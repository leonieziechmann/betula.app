use std::path::PathBuf;
use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::services::ServeDir;
use crate::db::Database;
use crate::html::HtmlRenderer;
use crate::slug::{ProgramOption, resolve_program};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub renderer: HtmlRenderer,
    pub dist_dir: PathBuf,
    pub db_path: PathBuf,
    pub programs: Arc<Vec<ProgramOption>>,
}

pub fn create_router(state: AppState) -> Router {
    let static_dir = state.dist_dir.clone();

    Router::new()
        // Page routes
        .route("/", get(handle_index))
        .route("/catalog", get(handle_catalog))
        .route("/catalogue", get(handle_catalog))
        .route("/catalouge", get(handle_catalog))
        .route("/catalog/module/{id}", get(handle_module))
        .route("/course/{id}", get(handle_module))
        .route("/study-programm/{slug}", get(handle_program_default))
        .route("/study-programm/{slug}/{tab}", get(handle_program_tab))
        .route("/programs", get(handle_programs))
        .route("/studiengaenge", get(handle_coverage))
        // PWA specific endpoints
        .route("/manifest.json", get(handle_manifest))
        .route("/sw.js", get(handle_service_worker))
        // API endpoints
        .route("/api/status", get(handle_status))
        .route("/api/db", get(handle_db_file))
        .route("/api/programs", get(handle_programs_api))
        // Fallback for static assets in dist (WASM, JS, CSS, fonts, static/)
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}

// Handler: GET / -> Renders catalog directly
async fn handle_index(State(state): State<AppState>) -> Response {
    handle_catalog(State(state)).await
}

// Handler: GET /catalog -> Renders static catalog with all modules & study programs
async fn handle_catalog(State(state): State<AppState>) -> Response {
    let modules = match state.db.get_all_modules_summary() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to fetch modules: {}", e);
            Vec::new()
        }
    };

    let html = state.renderer.render_catalog(&modules, &state.programs);
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Html(html)).into_response()
}

// Handler: GET /catalog/module/:id and /course/:id -> Renders static module details
async fn handle_module(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let decoded_id = percent_encoding::percent_decode_str(&id)
        .decode_utf8_lossy()
        .to_string();

    match state.db.get_module_detail(&decoded_id, &state.programs) {
        Ok(Some(module)) => {
            let html = state.renderer.render_module(&module);
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Html(html)).into_response()
        }
        Ok(None) => {
            let html = state.renderer.render_not_found();
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                Html(html),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Error querying module {}: {}", decoded_id, e);
            let html = state.renderer.render_not_found();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                Html(html),
            )
                .into_response()
        }
    }
}

// Handler: GET /study-programm/:slug -> Renders plan view by default
async fn handle_program_default(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    handle_program_tab(State(state), Path((slug, "plan".to_string()))).await
}

// Handler: GET /study-programm/:slug/:tab -> Renders study program with specified tab
async fn handle_program_tab(
    State(state): State<AppState>,
    Path((slug, tab)): Path<(String, String)>,
) -> Response {
    let decoded_slug = percent_encoding::percent_decode_str(&slug)
        .decode_utf8_lossy()
        .to_string();

    let prog_id = resolve_program(&decoded_slug, &state.programs);

    match state.db.get_program_detail(&prog_id, &state.programs) {
        Ok(Some(prog)) => {
            let html = state.renderer.render_program(&prog, &tab);
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Html(html)).into_response()
        }
        Ok(None) => {
            let html = state.renderer.render_not_found();
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                Html(html),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Error querying program {}: {}", prog_id, e);
            let html = state.renderer.render_not_found();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                Html(html),
            )
                .into_response()
        }
    }
}

// Handler: GET /programs -> Renders list of all study programs
async fn handle_programs(State(state): State<AppState>) -> Response {
    let html = state.renderer.render_programs_list(&state.programs);
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Html(html)).into_response()
}

// Handler: GET /studiengaenge -> Overview of all study programs and their data coverage
async fn handle_coverage(State(state): State<AppState>) -> Response {
    match state.db.get_program_coverage() {
        Ok(rows) => {
            let html = crate::html::render_coverage_page(&rows, &state.programs);
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Html(html)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to read program coverage: {}", e);
            let html = state.renderer.render_not_found();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                Html(html),
            )
                .into_response()
        }
    }
}

// Handler: GET /manifest.json -> Serves PWA manifest with correct header
async fn handle_manifest(State(state): State<AppState>) -> Response {
    let manifest_path = state.dist_dir.join("manifest.json");
    let static_manifest_path = state.dist_dir.join("static").join("manifest.json");

    let content = tokio::fs::read_to_string(&manifest_path)
        .await
        .or_else(|_| std::fs::read_to_string(&static_manifest_path))
        .unwrap_or_else(|_| "{}".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/manifest+json; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );

    (headers, content).into_response()
}

// Handler: GET /sw.js -> Serves Service Worker with Service-Worker-Allowed header
async fn handle_service_worker(State(state): State<AppState>) -> Response {
    let sw_path = state.dist_dir.join("sw.js");
    let static_sw_path = state.dist_dir.join("static").join("sw.js");

    let content = tokio::fs::read_to_string(&sw_path)
        .await
        .or_else(|_| std::fs::read_to_string(&static_sw_path))
        .unwrap_or_else(|_| "// sw fallback\nself.addEventListener('fetch', () => {});".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    headers.insert(
        header::HeaderName::from_static("service-worker-allowed"),
        HeaderValue::from_static("/"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, must-revalidate"),
    );

    (headers, content).into_response()
}

// Handler: GET /api/status -> Returns JSON status for client sqlite bridge
async fn handle_status(State(state): State<AppState>) -> Response {
    let stats = state.db.get_stats().unwrap_or(crate::db::DbStats {
        total_modules: 0,
        total_programs: 0,
    });

    let file_size = tokio::fs::metadata(&state.db_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let etag = format!("\"btu-db-{file_size}\"");

    Json(json!({
        "status": "ok",
        "etag": etag,
        "database": {
            "modules_count": stats.total_modules,
            "programs_count": stats.total_programs,
            "size_bytes": file_size,
            "etag": etag
        }
    }))
    .into_response()
}

// Handler: GET /api/programs -> Returns JSON programs list
async fn handle_programs_api(State(state): State<AppState>) -> Response {
    Json(&*state.programs).into_response()
}

// Handler: GET /api/db -> Serves SQLite database file for SQL.js WASM
async fn handle_db_file(State(state): State<AppState>) -> Response {
    let path = state.db_path.clone();
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Database file not found").into_response();
    }

    match tokio::fs::read(&path).await {
        Ok(data) => {
            let file_size = data.len();
            let etag = format!("\"btu-db-{file_size}\"");

            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/vnd.sqlite3"),
            );
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            );
            headers.insert(
                header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("\"v1\"")),
            );
            headers.insert(
                header::HeaderName::from_static("access-control-allow-origin"),
                HeaderValue::from_static("*"),
            );

            (headers, data).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to read database file: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read database").into_response()
        }
    }
}
