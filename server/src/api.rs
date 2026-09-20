//! What is not a rendered page: the snapshot for browsers, status, health, static assets.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tokio_util::io::ReaderStream;

use crate::AppState;

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|tag| tag.trim().trim_start_matches("W/") == etag))
}

/// `GET /api/db`: the active snapshot with Radix's ETag. Browsers keep it in IndexedDB
/// and come back with `If-None-Match`, which is answered without touching the file.
pub async fn database(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    if if_none_match(&headers, &snapshot.etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, snapshot.etag.clone())]).into_response();
    }

    let wants_gzip = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")));
    let (path, length, compressed) = match (&snapshot.gzip, wants_gzip) {
        (Some((path, length)), true) => (path.clone(), *length, true),
        _ => (snapshot.path.clone(), snapshot.bytes, false),
    };
    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::error!(component = "http", event = "snapshot.unreadable", path = %path.display(), error = %error, "active snapshot file cannot be opened");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut response = Response::new(Body::from_stream(ReaderStream::with_capacity(file, 256 * 1024)));
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.sqlite3"));
    // Always revalidate: the 304 is cheap and a changed snapshot is picked up at once.
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, no-cache"));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    out.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    if let Ok(value) = HeaderValue::from_str(&snapshot.etag) {
        out.insert(header::ETAG, value);
    }
    if compressed {
        out.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

/// A body made once per snapshot, with the snapshot's ETag.
fn per_snapshot(headers: &HeaderMap, etag: &str, content_type: &'static str, body: &(axum::body::Bytes, axum::body::Bytes)) -> Response {
    if if_none_match(headers, etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag.to_string())]).into_response();
    }
    let wants_gzip = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")));
    let use_gzip = wants_gzip && !body.1.is_empty();
    let mut response = Response::new(Body::from(if use_gzip { body.1.clone() } else { body.0.clone() }));
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=300, stale-while-revalidate=86400"));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Ok(value) = HeaderValue::from_str(etag) {
        out.insert(header::ETAG, value);
    }
    if use_gzip {
        out.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

/// `GET /api/map.json`: the map of the programs for the landing page of the browser app. Laid out
/// when the snapshot was opened (`catalog::graph`); this only hands it on.
pub async fn program_map(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    match &snapshot.program_map {
        Some((_, json, compressed)) => per_snapshot(&headers, &snapshot.etag, "application/json", &(json.clone(), compressed.clone())),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /sitemap.xml`: every page a search engine should know: the three entrances, every module
/// and every current program with its views. Filters of the lists are not pages (`app::seo`).
pub async fn sitemap(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    if snapshot.sitemap.get().is_none() {
        let mut listed: Result<(Vec<String>, Vec<catalog::rows::Program>), catalog::DbError> = Err(catalog::DbError::Unavailable("not run".to_string()));
        let ran = snapshot.with_db(&mut |db| {
            listed = catalog::queries::module_ids(db).and_then(|modules| Ok((modules, catalog::queries::programs(db)?)));
        });
        let (modules, programs) = match ran.and(listed) {
            Ok(listed) => listed,
            Err(error) => {
                tracing::error!(component = "http", event = "sitemap.failed", error = %error, "the sitemap could not be read from the snapshot");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };
        let mut paths = vec![catalog::url::HOME.to_string(), catalog::url::CATALOG.to_string(), catalog::url::PROGRAMS.to_string()];
        for program in programs.iter().filter(|program| program.is_latest_po) {
            paths.extend(catalog::url::ProgramTab::ALL.iter().map(|tab| catalog::url::program_path(&program.slug, *tab)));
        }
        paths.extend(modules.iter().map(|id| catalog::url::module_path(id)));
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for path in paths {
            let address = format!("{}{path}", state.public_url).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            xml.push_str(&format!("<url><loc>{address}</loc></url>\n"));
        }
        xml.push_str("</urlset>\n");
        let compressed = crate::cache::gzip(xml.as_bytes());
        let _ = snapshot.sitemap.set((axum::body::Bytes::from(xml), compressed));
    }
    match snapshot.sitemap.get() {
        Some(body) => per_snapshot(&headers, &snapshot.etag, "application/xml; charset=utf-8", body),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /api/status`: what the browser compares its cached snapshot with, and what operators look at.
pub async fn status(State(state): State<AppState>) -> Response {
    let (cached_pages, cached_bytes) = state.cache.size();
    let snapshot = state.store.current().map(|snapshot| {
        json!({
            "etag": snapshot.etag,
            "bytes": snapshot.bytes,
            "gzip_bytes": snapshot.gzip.as_ref().map(|(_, bytes)| *bytes),
            "data_changed_at": snapshot.meta.data_changed_at,
            "current_semester": snapshot.meta.current_semester,
            "activated_seconds_ago": snapshot.activated_at.elapsed().map(|d| d.as_secs()).unwrap_or(0),
        })
    });
    let body = json!({
        "snapshot": snapshot,
        "radix_last_contact_seconds_ago": state.store.seconds_since_contact(),
        "html_cache": { "pages": cached_pages, "bytes": cached_bytes },
        "uptime_seconds": state.store.uptime().as_secs(),
        "build": state.build_id.as_ref(),
    });
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

/// `GET /healthz`: 200 while a snapshot is served and Radix was reachable recently.
pub async fn health(State(state): State<AppState>) -> Response {
    let problem = if state.store.current().is_none() {
        Some("no snapshot yet".to_string())
    } else {
        state.stale_after.and_then(|limit| {
            let silent_for = state.store.seconds_since_contact().unwrap_or_else(|| state.store.uptime().as_secs());
            (silent_for > limit.as_secs()).then(|| format!("no answer from Radix for {silent_for} s"))
        })
    };
    match problem {
        None => (StatusCode::OK, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "status": "ok" }))).into_response(),
        Some(reason) => {
            (StatusCode::SERVICE_UNAVAILABLE, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "status": "unhealthy", "reason": reason }))).into_response()
        }
    }
}

/// A file embedded in the binary. Revalidated on every use (the 304 costs nothing and a new
/// build shows up at once); compressed once per process.
fn asset(state: &AppState, headers: &HeaderMap, content_type: &'static str, body: &'static [u8]) -> Response {
    static COMPRESSED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, axum::body::Bytes>>> = std::sync::OnceLock::new();

    let etag = format!("\"{}\"", state.build_id);
    if if_none_match(headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    let wants_gzip = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")));
    // Fonts are compressed already.
    let compressed = (wants_gzip && content_type != "font/woff2" && content_type != "image/png" && body.len() > 1024)
        .then(|| {
            let mut cache = COMPRESSED.get_or_init(Default::default).lock().ok()?;
            Some(cache.entry(body.as_ptr() as usize).or_insert_with(|| crate::cache::gzip(body)).clone())
        })
        .flatten()
        .filter(|bytes| !bytes.is_empty());

    let mut response = match &compressed {
        Some(bytes) => Response::new(Body::from(bytes.clone())),
        None => Response::new(Body::from(body)),
    };
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, no-cache"));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Ok(value) = HeaderValue::from_str(&etag) {
        out.insert(header::ETAG, value);
    }
    if compressed.is_some() {
        out.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

pub async fn stylesheet(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/css; charset=utf-8", include_bytes!("../../app/assets/app.css"))
}

pub async fn favicon(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/svg+xml", include_bytes!("../../app/assets/favicon.svg"))
}

pub async fn og_image(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/og.png"))
}

pub async fn font(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "font/woff2", include_bytes!("../../app/assets/inter-latin.woff2"))
}

pub async fn enhance_script(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/javascript; charset=utf-8", include_bytes!("../../app/assets/enhance.js"))
}

pub async fn boot_script(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/javascript; charset=utf-8", include_bytes!("../../app/assets/boot.js"))
}

pub async fn sql_js(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/javascript; charset=utf-8", include_bytes!("../../app/assets/sql-wasm.js"))
}

pub async fn sql_wasm(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "application/wasm", include_bytes!("../../app/assets/sql-wasm.wasm"))
}

/// `GET /pkg/<file>`: the browser app built by scripts/build-client.sh, from `<site-root>/pkg`.
/// Compressed once per file version and kept in memory.
pub async fn package(State(state): State<AppState>, Path(file): Path<String>, headers: HeaderMap) -> Response {
    let content_type = match file.rsplit('.').next() {
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if !file.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = state.site_root.join("pkg").join(&file);
    let Ok(meta) = tokio::fs::metadata(&path).await else { return StatusCode::NOT_FOUND.into_response() };
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let etag = format!("\"{:x}-{:x}\"", modified, meta.len());
    if if_none_match(&headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }

    let cached = state.packages.lock().ok().and_then(|cache| cache.get(&file).filter(|(tag, ..)| *tag == etag).cloned());
    let (_, raw, compressed) = match cached {
        Some(entry) => entry,
        None => {
            let Ok(raw) = tokio::fs::read(&path).await else { return StatusCode::NOT_FOUND.into_response() };
            let raw = axum::body::Bytes::from(raw);
            let compressed = crate::cache::gzip(&raw);
            let entry = (etag.clone(), raw, compressed);
            if let Ok(mut cache) = state.packages.lock() {
                cache.insert(file.clone(), entry.clone());
            }
            entry
        }
    };
    let wants_gzip = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")));
    let use_gzip = wants_gzip && !compressed.is_empty();
    let mut response = Response::new(Body::from(if use_gzip { compressed } else { raw }));
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, no-cache"));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Ok(value) = HeaderValue::from_str(&etag) {
        out.insert(header::ETAG, value);
    }
    if use_gzip {
        out.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

pub async fn robots(State(state): State<AppState>) -> Response {
    let body = format!("User-agent: *\nAllow: /\nDisallow: /api/\n\nSitemap: {}/sitemap.xml\n", state.public_url);
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=86400")], body).into_response()
}
