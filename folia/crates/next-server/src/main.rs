//! `folia-next`: serves the minimal version on a snapshot file.
//!
//!   folia-next [--addr 127.0.0.1:8090] [--snapshot ../snapshot/catalog.db]
//!
//! `/catalog`, `/catalog/module/<id>`: the site; `/bookmarks`: the app document; `/api/status`,
//! `/api/db`: the snapshot for the data worker; `/spike/next-snapshot`: the same bytes under a new
//! ETag, so that the switch to new data can be watched; `/pkg/…`: the bundles (folia/site/pkg);
//! `/assets/…`: today's files of `app/assets`, the crates' stylesheet and the boot scripts.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use catalog::native::NativeDatabase;
use folia_pages::Ask;
use folia_shell::Area;
use tower_http::services::ServeDir;

const STYLES: &str = include_str!(concat!(env!("OUT_DIR"), "/folia.css"));
const BOOT: &str = include_str!("../next-boot.js");
const WORKER: &str = include_str!("../../worker/worker.js");

struct AppState {
    db: Mutex<NativeDatabase>,
    bytes: Vec<u8>,
    generation: AtomicU64,
    build: String,
}

type Shared = Arc<AppState>;

fn etag(state: &AppState) -> String {
    format!("\"spike-{}-{}\"", state.bytes.len(), state.generation.load(Ordering::Relaxed))
}

fn html(result: Result<Option<String>, String>) -> Response {
    match result {
        Ok(Some(page)) => Html(page).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Nicht gefunden").into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

fn site(state: &AppState, ask: &Ask) -> Response {
    let Ok(db) = state.db.lock() else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    html(folia_site::page(&*db, ask, &state.build))
}

async fn catalog(State(state): State<Shared>, RawQuery(query): RawQuery) -> Response {
    let ask = folia_catalog_ui::ask_of("/catalog", query.as_deref().unwrap_or("")).unwrap_or(Ask::Catalog { page: 1 });
    site(&state, &ask)
}

async fn module(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    site(&state, &Ask::Module { id })
}

async fn bookmarks(State(state): State<Shared>) -> Response {
    Html(folia_site::app_document("Merkliste", Area::Bookmarks, &state.build)).into_response()
}

async fn status(State(state): State<Shared>) -> Response {
    let body = format!("{{\"snapshot\":{{\"etag\":{:?},\"bytes\":{}}}}}", etag(&state), state.bytes.len());
    ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")], body).into_response()
}

async fn database(State(state): State<Shared>) -> Response {
    let mut response = Response::new(Body::from(state.bytes.clone()));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.sqlite3"));
    if let Ok(value) = HeaderValue::from_str(&etag(&state)) {
        headers.insert(header::ETAG, value);
    }
    response
}

async fn next_snapshot(State(state): State<Shared>) -> String {
    state.generation.fetch_add(1, Ordering::Relaxed);
    etag(&state)
}

fn text(kind: &'static str, body: String) -> Response {
    ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut addr, mut snapshot) = ("127.0.0.1:8090".to_string(), PathBuf::from("../snapshot/catalog.db"));
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => addr = args.next().unwrap_or(addr),
            "--snapshot" => snapshot = args.next().map(PathBuf::from).unwrap_or(snapshot),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let bytes = std::fs::read(&snapshot).map_err(|e| format!("{}: {e}", snapshot.display()))?;
    let db = NativeDatabase::open(&snapshot).map_err(|e| e.to_string())?;
    let build = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    let state = Arc::new(AppState { db: Mutex::new(db), bytes, generation: AtomicU64::new(0), build });
    let app = Router::new()
        .route("/catalog", get(catalog))
        .route("/catalog/module/{id}", get(module))
        .route("/bookmarks", get(bookmarks))
        .route("/api/status", get(status))
        .route("/api/db", get(database))
        .route("/spike/next-snapshot", get(next_snapshot))
        .route("/assets/folia.css", get(|| async { text("text/css", STYLES.to_string()) }))
        .route("/assets/next-boot.js", get(|| async { text("text/javascript", BOOT.to_string()) }))
        .route("/assets/next-worker.js", get(|| async { text("text/javascript", WORKER.to_string()) }))
        .route("/assets/icons.svg", get(|| async { text("image/svg+xml", folia_design::icons::sprite()) }))
        .nest_service("/pkg", ServeDir::new("site/pkg"))
        .nest_service("/assets", ServeDir::new("../app/assets"))
        .with_state(state);
    let addr: SocketAddr = addr.parse().map_err(|e| format!("{addr}: {e}"))?;
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| e.to_string())?;
    println!("folia-next on http://{addr}");
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}
