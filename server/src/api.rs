//! What is not a rendered page: the snapshot for browsers, status, health, static assets.

use axum::body::Body;
use axum::extract::State;
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

/// `GET /api/db`: the active snapshot with the scraper's ETag. Browsers keep it in IndexedDB
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
        "scraper_last_contact_seconds_ago": state.store.seconds_since_contact(),
        "html_cache": { "pages": cached_pages, "bytes": cached_bytes },
        "uptime_seconds": state.store.uptime().as_secs(),
        "build": state.build_id.as_ref(),
    });
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

/// `GET /healthz`: 200 while a snapshot is served and the scraper was reachable recently.
pub async fn health(State(state): State<AppState>) -> Response {
    let problem = if state.store.current().is_none() {
        Some("no snapshot yet".to_string())
    } else {
        state.stale_after.and_then(|limit| {
            let silent_for = state.store.seconds_since_contact().unwrap_or_else(|| state.store.uptime().as_secs());
            (silent_for > limit.as_secs()).then(|| format!("no answer from the scraper for {silent_for} s"))
        })
    };
    match problem {
        None => (StatusCode::OK, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "status": "ok" }))).into_response(),
        Some(reason) => {
            (StatusCode::SERVICE_UNAVAILABLE, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "status": "unhealthy", "reason": reason }))).into_response()
        }
    }
}

fn asset(state: &AppState, headers: &HeaderMap, content_type: &'static str, body: &'static [u8]) -> Response {
    let etag = format!("\"{}\"", state.build_id);
    if if_none_match(headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    // Revalidated on every use: the 304 costs nothing, and a new build shows up at once.
    ([(header::CONTENT_TYPE, content_type.to_string()), (header::CACHE_CONTROL, "public, no-cache".to_string()), (header::ETAG, etag)], body).into_response()
}

pub async fn stylesheet(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/css; charset=utf-8", include_bytes!("../../app/assets/app.css"))
}

pub async fn favicon(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/svg+xml", include_bytes!("../../app/assets/favicon.svg"))
}

pub async fn font(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "font/woff2", include_bytes!("../../app/assets/inter-latin.woff2"))
}

pub async fn enhance_script(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/javascript; charset=utf-8", include_bytes!("../../app/assets/enhance.js"))
}

pub async fn robots() -> Response {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=86400")], "User-agent: *\nAllow: /\nDisallow: /api/\n").into_response()
}
