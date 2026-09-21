//! Rendered pages, cached on read.
//!
//! A page depends on nothing but its URL and the active snapshot (server HTML never depends
//! on the user), so the first request renders it and every later one is a memory copy, until
//! the next snapshot starts a new generation. Browsers and proxies get an ETag per generation
//! and revalidate with a 304 that costs no rendering at all.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use catalog::url::CatalogUrl;

use crate::AppState;

const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_KEY_BYTES: usize = 2048;

struct Entry {
    body: Bytes,
    gzip: Bytes,
    last_used: u64,
}

#[derive(Default)]
struct Inner {
    generation: u64,
    entries: HashMap<String, Entry>,
    bytes: usize,
    tick: u64,
}

pub struct HtmlCache {
    inner: Mutex<Inner>,
    max_bytes: usize,
}

impl HtmlCache {
    pub fn new(max_bytes: usize) -> Self {
        Self { inner: Mutex::new(Inner::default()), max_bytes }
    }

    fn get(&self, generation: u64, key: &str) -> Option<(Bytes, Bytes)> {
        let mut inner = self.inner.lock().ok()?;
        if inner.generation != generation {
            return None;
        }
        inner.tick += 1;
        let tick = inner.tick;
        let entry = inner.entries.get_mut(key)?;
        entry.last_used = tick;
        Some((entry.body.clone(), entry.gzip.clone()))
    }

    fn put(&self, generation: u64, key: String, body: Bytes, gzip: Bytes) {
        let size = key.len() + body.len() + gzip.len();
        if size > self.max_bytes {
            return;
        }
        let Ok(mut inner) = self.inner.lock() else { return };
        if inner.generation != generation {
            // A new snapshot: everything rendered from the old one is obsolete.
            inner.entries.clear();
            inner.bytes = 0;
            inner.generation = generation;
        }
        inner.tick += 1;
        let tick = inner.tick;
        if let Some(old) = inner.entries.insert(key.clone(), Entry { body, gzip, last_used: tick }) {
            inner.bytes -= key.len() + old.body.len() + old.gzip.len();
        }
        inner.bytes += size;
        while inner.bytes > self.max_bytes {
            let Some(oldest) = inner.entries.iter().min_by_key(|(_, entry)| entry.last_used).map(|(key, _)| key.clone()) else { break };
            if let Some(evicted) = inner.entries.remove(&oldest) {
                inner.bytes -= oldest.len() + evicted.body.len() + evicted.gzip.len();
            }
        }
    }

    /// (pages, bytes) for /api/status.
    pub fn size(&self) -> (usize, usize) {
        self.inner.lock().map(|inner| (inner.entries.len(), inner.bytes)).unwrap_or((0, 0))
    }
}

/// Equal pages get equal keys: the catalog by its canonical filter, every other page by its
/// path alone (tracking parameters and the like do not change what is rendered). What the app
/// lays beside a page or fills it with (`open`, `full`, `area`, `req`) changes nothing on the
/// server's page, so it is no part of the key either.
pub fn cache_key(uri: &Uri) -> String {
    let path = match uri.path().trim_end_matches('/') {
        "" => "/",
        path => path,
    };
    if path == catalog::url::CATALOG {
        // The list with its filters.
        CatalogUrl::parse(uri.query().unwrap_or_default()).with_open(None).path()
    } else if path == catalog::url::PROGRAMS {
        // The program overview with its filters; the search text folded, as the page matches it.
        let mut overview = catalog::url::ProgramsUrl::parse(uri.query().unwrap_or_default());
        overview.text = catalog::search::fold(&overview.text);
        overview.path()
    } else if path.starts_with("/programs/") {
        // A program's page shows one of its study plans.
        let tab = path.rsplit('/').next().and_then(catalog::url::ProgramTab::from_segment).unwrap_or_default();
        let url = catalog::url::ProgramUrl::parse("", tab, uri.query().unwrap_or_default());
        format!("{path}{}", catalog::url::ProgramUrl { open: None, full: false, area: None, req: None, ..url }.query())
    } else {
        path.to_string()
    }
}

fn accepts_gzip(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")))
}

pub fn gzip(body: &[u8]) -> Bytes {
    let mut encoder = flate2::write::GzEncoder::new(Vec::with_capacity(body.len() / 4), flate2::Compression::new(6));
    match encoder.write_all(body).and_then(|_| encoder.finish()) {
        Ok(compressed) => Bytes::from(compressed),
        Err(_) => Bytes::new(),
    }
}

fn page(body: Bytes, compressed: Bytes, etag: &str, cache_state: &'static str, wants_gzip: bool) -> Response {
    let use_gzip = wants_gzip && !compressed.is_empty();
    let mut response = Response::new(Body::from(if use_gzip { compressed } else { body }));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    // Asked again every time (the ETag makes it a 304 from memory): a page names the build of its
    // stylesheet and scripts, and a page of the old build kept by the browser after a deploy would
    // meet the files of the new one, which the server serves under every build's address.
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, no-cache"));
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    headers.insert("x-cache", HeaderValue::from_static(cache_state));
    if let Ok(value) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, value);
    }
    if use_gzip {
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

/// Middleware around the rendered routes.
pub async fn html_cache(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.method() != Method::GET {
        return next.run(request).await;
    }
    let Some(snapshot) = state.store.current() else {
        // No data yet: render the error page, tell clients to come back, cache nothing.
        let mut response = next.run(request).await;
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
        return response;
    };

    let generation = state.store.generation();
    let etag = format!("W/\"{}.{}\"", snapshot.etag.trim_matches('"'), state.build_id);
    let wants_gzip = accepts_gzip(request.headers());
    let revalidates = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|tag| tag.trim() == etag));
    let key = cache_key(request.uri());

    if let Some((body, compressed)) = state.cache.get(generation, &key) {
        if revalidates {
            return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
        }
        return page(body, compressed, &etag, "hit", wants_gzip);
    }

    let response = next.run(request).await;
    let (parts, body) = response.into_parts();
    let body = match axum::body::to_bytes(body, MAX_PAGE_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            tracing::error!(component = "http", event = "render.failed", path = %key, error = %error, "rendered page could not be read");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let is_html = parts.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/html"));
    if parts.status != StatusCode::OK || !is_html {
        // 404, 5xx: never cached, by nobody.
        let mut response = Response::from_parts(parts, Body::from(body));
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return response;
    }

    let compressed = gzip(&body);
    if key.len() <= MAX_KEY_BYTES && state.store.generation() == generation {
        state.cache.put(generation, key, body.clone(), compressed.clone());
    }
    if revalidates {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    page(body, compressed, &etag, "miss", wants_gzip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_pages_share_a_key() {
        let key = |uri: &str| cache_key(&uri.parse::<Uri>().unwrap());
        assert_eq!(key("/catalog?form=exercise&turnus=winter&q="), key("/catalog?turnus=winter&form=exercise"));
        assert_eq!(key("/catalog?page=1"), "/catalog");
        assert_ne!(key("/catalog?page=2"), key("/catalog"));
        assert_eq!(key("/catalog/module/11101?utm_source=x"), "/catalog/module/11101");
        // What the app shows beside a page is not the server's: the page is the same without it.
        assert_eq!(key("/catalog?open=11101&form=exercise&turnus=winter"), "/catalog?turnus=winter&form=exercise");
        assert_eq!(key("/programs/x/plan?variant=2&open=11101&full=1&area=3&req=4"), "/programs/x/plan?variant=2");
        assert_eq!(key("/programs?q=+%C3%96ko"), "/programs?q=oko");
        assert_eq!(key("/programs/x/plan?utm_source=x"), "/programs/x/plan");
        // Which study plan of a program is shown belongs to the page, so also to its key.
        assert_eq!(key("/programs/x/plan?variant=2"), "/programs/x/plan?variant=2");
        assert_eq!(key("/programs/x/plan?variant=1"), key("/programs/x/plan?variant=nonsense"));
        assert_eq!(key("/programs/x/plan?open=../etc"), "/programs/x/plan");
        assert_eq!(key("/programs/"), "/programs");
        assert_eq!(key("/"), "/");
    }

    #[test]
    fn the_cache_is_bounded_and_forgets_old_generations() {
        let cache = HtmlCache::new(1000);
        let body = |n: usize| Bytes::from(vec![b'x'; n]);
        cache.put(1, "/a".into(), body(400), body(30));
        cache.put(1, "/b".into(), body(400), body(30));
        assert!(cache.get(1, "/a").is_some());
        cache.put(1, "/c".into(), body(400), body(30));
        // "/b" was used least recently.
        assert!(cache.get(1, "/b").is_none() && cache.get(1, "/a").is_some() && cache.get(1, "/c").is_some());
        assert!(cache.size().1 <= 1000);

        assert!(cache.get(2, "/a").is_none(), "a new snapshot must not see old pages");
        cache.put(2, "/d".into(), body(10), body(1));
        assert_eq!(cache.size().0, 1);
        cache.put(2, "/huge".into(), body(5000), body(1));
        assert!(cache.get(2, "/huge").is_none());
    }
}
