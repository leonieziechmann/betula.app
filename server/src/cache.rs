//! Rendered pages, cached on read.
//!
//! A page depends on nothing but its URL and the active snapshot (server HTML never depends
//! on the user), so the first request renders it and every later one is a memory copy, until
//! the next snapshot starts a new generation. Browsers and proxies get an ETag per generation
//! and revalidate with a 304 that costs no rendering at all.
//!
//! **Kept compressed only** (2026-09-26): nearly every client asks for gzip, and compressed a
//! page takes a fourth to a ninth of the memory (a module 27 → 6 kB with both copies before, a
//! filtered list 146 → 17 kB, the start page 516 → 106 kB). 128 MiB held about 1,200 pages
//! before — not even the modules of the sitemap — and now hold all 5,200 pages of the sitemap
//! and thousands of views besides. The rare client without gzip gets the page unpacked on the
//! way out. A full cache drops a tenth of itself at once, views (an address with a query:
//! filters, further pages, variants) before the pages of the sitemap, least recently used
//! first: a crawler walking through filters cannot push the site's own pages out.
//!
//! **One render per page** at a time: whoever asks for a page that is being rendered waits for
//! that render and is answered from the cache (after a new snapshot or a restart the start page
//! is asked for many times before its first render is done). A render needs one of the server's
//! places (`busy::Places`): without one within the wait, the page is answered 503.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use catalog::url::CatalogUrl;
use tokio::sync::watch;

use crate::AppState;

const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_KEY_BYTES: usize = 2048;
/// A full cache drops pages until it is at this share of its size, so that it does not sort its
/// entries for every page it takes.
const EVICT_TO_PERCENT: usize = 90;

/// Marks a request of the warm-up (`warm`): rendered only while the server has nothing else to
/// do, never waited for.
#[derive(Clone, Copy)]
pub struct WarmUp;

struct Entry {
    gzip: Bytes,
    last_used: u64,
    /// A page search engines list (`listed`), dropped only when no view is left.
    canonical: bool,
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
    /// The pages being rendered right now, by key. Whoever asks for one of them waits until the
    /// sender is dropped (the render is over, cached or not).
    rendering: Mutex<HashMap<String, watch::Receiver<()>>>,
}

/// The render of one key, held by the request that renders it: when it ends, however it ends,
/// the key is free again and whoever waited looks into the cache.
struct Rendering<'a> {
    cache: &'a HtmlCache,
    key: String,
    _done: watch::Sender<()>,
}

impl Drop for Rendering<'_> {
    fn drop(&mut self) {
        if let Ok(mut rendering) = self.cache.rendering.lock() {
            rendering.remove(&self.key);
        }
    }
}

enum Turn<'a> {
    Render(Rendering<'a>),
    Wait(watch::Receiver<()>),
}

impl HtmlCache {
    pub fn new(max_bytes: usize) -> Self {
        Self { inner: Mutex::new(Inner::default()), max_bytes, rendering: Mutex::new(HashMap::new()) }
    }

    fn get(&self, generation: u64, key: &str) -> Option<Bytes> {
        let mut inner = self.inner.lock().ok()?;
        if inner.generation != generation {
            return None;
        }
        inner.tick += 1;
        let tick = inner.tick;
        let entry = inner.entries.get_mut(key)?;
        entry.last_used = tick;
        Some(entry.gzip.clone())
    }

    fn put(&self, generation: u64, key: String, gzip: Bytes) {
        let size = key.len() + gzip.len();
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
        let canonical = listed(&key);
        if let Some(old) = inner.entries.insert(key.clone(), Entry { gzip, last_used: tick, canonical }) {
            inner.bytes = inner.bytes.saturating_sub(key.len() + old.gzip.len());
        }
        inner.bytes += size;
        if inner.bytes > self.max_bytes {
            evict(&mut inner, self.max_bytes / 100 * EVICT_TO_PERCENT);
        }
    }

    /// Whether this request renders `key` or waits for the request that does.
    fn turn(&self, key: &str) -> Option<Turn<'_>> {
        let mut rendering = self.rendering.lock().ok()?;
        if let Some(receiver) = rendering.get(key) {
            return Some(Turn::Wait(receiver.clone()));
        }
        let (done, receiver) = watch::channel(());
        rendering.insert(key.to_string(), receiver);
        Some(Turn::Render(Rendering { cache: self, key: key.to_string(), _done: done }))
    }

    /// (pages, bytes) for /api/status.
    pub fn size(&self) -> (usize, usize) {
        self.inner.lock().map(|inner| (inner.entries.len(), inner.bytes)).unwrap_or((0, 0))
    }
}

/// Drops pages until the cache holds at most `target` bytes: views before the pages of the
/// sitemap, each group least recently used first.
fn evict(inner: &mut Inner, target: usize) {
    let mut order: Vec<(bool, u64, String)> = inner.entries.iter().map(|(key, entry)| (entry.canonical, entry.last_used, key.clone())).collect();
    order.sort_unstable();
    for (_, _, key) in order {
        if inner.bytes <= target {
            break;
        }
        if let Some(entry) = inner.entries.remove(&key) {
            inner.bytes = inner.bytes.saturating_sub(key.len() + entry.gzip.len());
        }
    }
}

/// Equal pages get equal keys: the catalog by its canonical filter, every other page by its
/// path alone (tracking parameters and the like do not change what is rendered). What the app
/// lays beside a page or fills it with (`open`, `full`, `area`, `req`) changes nothing on the
/// server's page, so it is no part of the key either; nor does the placeholder a catalog list
/// is looking for (`fill`), which only the app's plan button reads. A shared Stundenplan
/// (`share=<code>`) is: the page names its modules for link previews. The language is part of
/// the key (`/en/catalog?…`): the key of a page in a language is the page's key in its prefix.
pub fn cache_key(uri: &Uri) -> String {
    let (locale, path) = catalog::Locale::split(uri.path());
    locale.path(&page_key(path, uri.query().unwrap_or_default()))
}

/// `cache_key` of the app's path (without a language) and its query.
fn page_key(path: &str, query: &str) -> String {
    let path = match path.trim_end_matches('/') {
        "" => "/",
        path => path,
    };
    if path == catalog::url::CATALOG {
        // The list with its filters.
        CatalogUrl::parse(query).with_open(None).with_fill(None).path()
    } else if path == catalog::url::PROGRAMS {
        // The program overview with its filters; the search text folded, as the page matches it.
        let mut overview = catalog::url::ProgramsUrl::parse(query);
        overview.text = catalog::search::fold(&overview.text);
        overview.path()
    } else if path.starts_with("/programs/") {
        // A program's page shows one of its study plans.
        let tab = path.rsplit('/').next().and_then(catalog::url::ProgramTab::from_segment).unwrap_or_default();
        let url = catalog::url::ProgramUrl::parse("", tab, query);
        format!("{path}{}", catalog::url::ProgramUrl { open: None, full: false, area: None, req: None, ..url }.query())
    } else if path == catalog::url::STUDYPLAN {
        // One page for every view of the plan; one for each plan handed on by a link.
        match catalog::url::StudyplanUrl::parse(query).share {
            Some(code) => catalog::timetable::share::path(&code),
            None => path.to_string(),
        }
    } else {
        path.to_string()
    }
}

/// Whether the page of a cache key is one search engines list (and the sitemap names), not a view
/// of one: an address without a query, the plan of a further study direction
/// (`/programs/<slug>/plan?variant=<n>`) and a further page of the unfiltered catalog
/// (`/catalog?page=<n>`). Keys are canonical spellings (`cache_key`), so these stand alone.
fn listed(key: &str) -> bool {
    let number = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    let key = catalog::Locale::split(key).1;
    match key.split_once('?') {
        None => true,
        Some((path, query)) if path.starts_with("/programs/") && path.ends_with("/plan") => query.strip_prefix("variant=").is_some_and(number),
        Some((path, query)) if path == catalog::url::CATALOG => query.strip_prefix("page=").is_some_and(number),
        Some(_) => false,
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

/// `body` compressed with Brotli at its best quality, for the small files the server writes itself
/// (every larger one gets its copy from the build, `build/main.rs`). The window is as large as the
/// body needs; browsers read windows up to 16 MB.
pub fn brotli(body: &[u8]) -> Bytes {
    let window = (usize::BITS - body.len().leading_zeros()).clamp(10, 24);
    let mut writer = brotli::CompressorWriter::new(Vec::with_capacity(body.len() / 4), 1 << 16, 11, window);
    match writer.write_all(body) {
        Ok(()) => Bytes::from(writer.into_inner()),
        Err(_) => Bytes::new(),
    }
}

pub fn gunzip(compressed: &[u8]) -> Option<Bytes> {
    let mut body = Vec::with_capacity(compressed.len() * 5);
    flate2::read::GzDecoder::new(compressed).read_to_end(&mut body).ok()?;
    Some(Bytes::from(body))
}

/// A page as the client can take it: compressed if it asked for gzip, else unpacked (or as it
/// came from the render, when that is at hand).
fn page(compressed: Bytes, plain: Option<Bytes>, etag: &str, cache_state: &'static str, wants_gzip: bool) -> Response {
    let use_gzip = wants_gzip && !compressed.is_empty();
    let body = match (use_gzip, plain) {
        (true, _) => compressed,
        (false, Some(plain)) => plain,
        (false, None) => match gunzip(&compressed) {
            Some(plain) => plain,
            None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
    };
    let mut response = Response::new(Body::from(body));
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
    // HEAD is a GET without the body (hyper leaves it out): some crawlers ask so, and it must not
    // render past the cache and the places.
    if request.method() != Method::GET && request.method() != Method::HEAD {
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
    let warm_up = request.extensions().get::<WarmUp>().is_some();
    let key = cache_key(request.uri());
    let cached = |state: &AppState| state.cache.get(generation, &key);
    let answer_cached = |compressed: Bytes, cache_state: &'static str| {
        if revalidates {
            return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag.clone())]).into_response();
        }
        page(compressed, None, &etag, cache_state, wants_gzip)
    };

    if let Some(compressed) = cached(&state) {
        return answer_cached(compressed, "hit");
    }

    // Somebody renders this page already: wait for that render, then answer from the cache.
    // (A render that ends without a page for the cache — a 404, a 503 — leaves the waiting ones
    // to render it themselves.)
    let mut rendering = None;
    if key.len() <= MAX_KEY_BYTES {
        match state.cache.turn(&key) {
            Some(Turn::Wait(mut done)) if !warm_up => {
                let _ = tokio::time::timeout(state.render_wait + Duration::from_secs(5), done.changed()).await;
                if let Some(compressed) = cached(&state) {
                    return answer_cached(compressed, "hit");
                }
            }
            Some(Turn::Wait(_)) => return crate::busy::busy(10, true),
            Some(Turn::Render(turn)) => rendering = Some(turn),
            None => {}
        }
    }

    // A place to render in: a visitor waits for one (a while), the warm-up only takes a free one.
    let place = if warm_up { state.renders.enter_idle() } else { state.renders.enter().await };
    let Some(_place) = place else {
        return crate::busy::busy(10, true);
    };

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
    if !compressed.is_empty() && key.len() <= MAX_KEY_BYTES && state.store.generation() == generation {
        state.cache.put(generation, key, compressed.clone());
    }
    drop(rendering);
    if revalidates {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    page(compressed, Some(body), &etag, "miss", wants_gzip)
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
        // Nor is the placeholder a list looks for; the fit switch is the page's (it says why the
        // list is empty on the server).
        assert_eq!(key("/catalog?fits=2026W&fill=p3"), key("/catalog?fits=2026W"));
        assert_eq!(key("/catalog?fill=p3&open=11101"), "/catalog");
        assert_eq!(key("/catalog?fits=2026w&fits-skip=exam"), "/catalog?fits=2026W&fits-skip=exam");
        assert_eq!(key("/catalog/module/12104?plan=2026W&fill=p3"), "/catalog/module/12104");
        // The Studienplan is one explanation for every address; the plan is the browser's.
        assert_eq!(key("/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38&import=mine"), "/studyplan");
        let semester = catalog::timetable::semester::SemesterKey::parse("2026W").unwrap();
        let code = catalog::timetable::share::SharedPlan::of(semester, &["12104".to_string()], None).unwrap().code().unwrap();
        assert_eq!(key(&format!("/studyplan?view=dates&share={code}&open=12104")), format!("/studyplan?share={code}"));
        assert_eq!(key("/studyplan?share=not-a-code"), "/studyplan");
        assert_eq!(key("/programs/x/plan?variant=2&open=11101&full=1&area=3&req=4"), "/programs/x/plan?variant=2");
        assert_eq!(key("/programs?q=+%C3%96ko"), "/programs?q=oko");
        assert_eq!(key("/programs/x/plan?utm_source=x"), "/programs/x/plan");
        // Which study plan of a program is shown belongs to the page, so also to its key.
        assert_eq!(key("/programs/x/plan?variant=2"), "/programs/x/plan?variant=2");
        assert_eq!(key("/programs/x/plan?variant=1"), key("/programs/x/plan?variant=nonsense"));
        assert_eq!(key("/programs/x/plan?open=../etc"), "/programs/x/plan");
        assert_eq!(key("/programs/"), "/programs");
        assert_eq!(key("/"), "/");
        // A page in another language is another page, with the same canonical spelling.
        assert_eq!(key("/en/catalog?form=exercise&turnus=winter&q="), "/en/catalog?turnus=winter&form=exercise");
        assert_ne!(key("/en/catalog?turnus=winter"), key("/catalog?turnus=winter"));
        assert_eq!(key("/en/programs/x/plan?variant=2&open=11101"), "/en/programs/x/plan?variant=2");
        assert_eq!((key("/en"), key("/en/"), key("/en?utm_source=x")), ("/en".to_string(), "/en".to_string(), "/en".to_string()));
    }

    #[test]
    fn the_cache_is_bounded_and_forgets_old_generations() {
        let cache = HtmlCache::new(1000);
        let body = |n: usize| Bytes::from(vec![b'x'; n]);
        cache.put(1, "/a".into(), body(400));
        cache.put(1, "/b".into(), body(400));
        assert!(cache.get(1, "/a").is_some());
        cache.put(1, "/c".into(), body(400));
        // "/b" was used least recently.
        assert!(cache.get(1, "/b").is_none() && cache.get(1, "/a").is_some() && cache.get(1, "/c").is_some());
        assert!(cache.size().1 <= 1000);

        assert!(cache.get(2, "/a").is_none(), "a new snapshot must not see old pages");
        cache.put(2, "/d".into(), body(10));
        assert_eq!(cache.size().0, 1);
        cache.put(2, "/huge".into(), body(5000));
        assert!(cache.get(2, "/huge").is_none());
    }

    #[test]
    fn views_go_before_the_pages_of_the_sitemap() {
        let cache = HtmlCache::new(10_000);
        let body = |n: usize| Bytes::from(vec![b'x'; n]);
        cache.put(1, "/catalog/module/1".into(), body(2000));
        cache.put(1, "/catalog/module/2".into(), body(2000));
        // A crawler walks through filters: many views, each used once, all of them newer.
        for n in 0..20 {
            cache.put(1, format!("/catalog?q={n}"), body(900));
        }
        assert!(cache.size().1 <= 10_000);
        assert!(cache.get(1, "/catalog/module/1").is_some() && cache.get(1, "/catalog/module/2").is_some(), "the sitemap's pages stay");
        assert!(cache.get(1, "/catalog?q=19").is_some(), "the newest view stays");
        assert!(cache.get(1, "/catalog?q=0").is_none(), "the oldest view went");
        // With no view left, the least recently used page of the sitemap goes.
        let only_pages = HtmlCache::new(5000);
        only_pages.put(1, "/a".into(), body(2000));
        only_pages.put(1, "/b".into(), body(2000));
        assert!(only_pages.get(1, "/a").is_some());
        only_pages.put(1, "/c".into(), body(2000));
        assert!(only_pages.get(1, "/b").is_none() && only_pages.get(1, "/a").is_some());
    }

    #[test]
    fn a_study_direction_and_a_page_of_the_catalog_are_pages_too() {
        for key in ["/", "/catalog/module/11101", "/programs/bachelor-elektrotechnik-2022/plan?variant=2", "/catalog?page=3", "/en", "/en/catalog?page=3", "/en/programs/x/plan?variant=2"] {
            assert!(listed(key), "{key}");
        }
        for key in ["/en/catalog?q=analysis", "/catalog?q=analysis", "/catalog?page=2&q=analysis", "/catalog?turnus=winter&page=2", "/programs/x/areas?variant=2", "/programs/x/plan?variant=2&area=7", "/programs?level=master"] {
            assert!(!listed(key), "{key}");
        }
    }

    #[test]
    fn a_page_is_kept_compressed_and_unpacked_for_who_asks() {
        let html = "<!DOCTYPE html><p>Grundlagen der Informatik</p>".repeat(50);
        let compressed = gzip(html.as_bytes());
        assert!(compressed.len() < html.len() / 5);
        assert_eq!(gunzip(&compressed).as_deref(), Some(html.as_bytes()));
    }
}
