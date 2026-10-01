//! What is not a rendered page: the snapshot for browsers, status, health, static assets, and a
//! Studienplan as a calendar feed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use catalog::timetable::subscription::{self, Subscription};
use catalog::Locale;
use serde_json::json;
use tokio_util::io::ReaderStream;

use crate::birch::Season;
use crate::cache::{not_modified, REVALIDATE};
use crate::cards::{Card, CardText, Headline};
use crate::encoding::{self, Coding, Kept};
use crate::texts::texts;
use crate::AppState;

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|tag| tag.trim().trim_start_matches("W/") == etag))
}

/// How long a browser keeps a file of the app before it asks for it again (`Cache-Control`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// A year, and never asked for again, not even on a reload (`immutable`): the address names
    /// the build of this process (`?v=<build>`, `app::BuildId`), and nothing the process serves
    /// under it changes while it runs. A new build is a new address, which the page names: the
    /// page itself is asked for again every time (`cache::REVALIDATE`).
    Immutable,
    /// Asked for again on every use, and answered with a 304 while it is unchanged (the ETag is
    /// the build): an address without a build, or with another one than this process's — the
    /// server answers every `?v=` with the file it has, which is not that build's.
    Revalidate,
}

impl Keep {
    /// Fresh for a year, as long as is usual: `immutable` holds while an answer is fresh (RFC 8246).
    pub const IMMUTABLE: &'static str = "public, max-age=31536000, immutable";

    /// How the file at `uri` is kept: `Immutable` under exactly the address a page of this build
    /// links it with (`/assets/app.css?v=<build>`). Never while the server serves `app/assets`
    /// live (`--live-assets`): an edited file, or a browser app built again, is a new file under
    /// the same address.
    pub fn of(state: &AppState, uri: &Uri) -> Keep {
        if state.live_assets.is_none() && uri.query().and_then(|query| query.strip_prefix("v=")) == Some(&*state.build_id) {
            Keep::Immutable
        } else {
            Keep::Revalidate
        }
    }

    pub fn header(self) -> &'static str {
        match self {
            Keep::Immutable => Keep::IMMUTABLE,
            Keep::Revalidate => REVALIDATE,
        }
    }
}

/// A file as it goes out: its type, how long it may be kept, its ETag and its coding.
fn respond(body: Bytes, coding: Coding, content_type: &'static str, cache_control: &'static str, etag: &str) -> Response {
    let mut response = Response::new(Body::from(body));
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Ok(value) = HeaderValue::from_str(etag) {
        out.insert(header::ETAG, value);
    }
    if let Some(value) = coding.header() {
        out.insert(header::CONTENT_ENCODING, value);
    }
    response
}

/// `GET /api/db`: the active snapshot with Radix's ETag. Browsers keep it in IndexedDB
/// and come back with `If-None-Match`, which is answered without touching the file.
pub async fn database(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    if if_none_match(&headers, &snapshot.etag) {
        return not_modified(&snapshot.etag, REVALIDATE);
    }

    // Compressed from memory: every browser gets the same bytes, and a download holds no file and
    // no buffer of its own. Brotli once it is made (in the background after a new snapshot,
    // `snapshot::compress_active`), gzip until then and for a client without brotli.
    // Uncompressed (hardly anybody) streamed from the file.
    let coding = Coding::of(&headers);
    let brotli = snapshot.brotli.get().filter(|_| coding == Coding::Brotli).map(|bytes| (bytes.clone(), Coding::Brotli));
    let gzip = || snapshot.gzip_bytes.clone().filter(|_| Coding::Gzip.taken_by(&headers)).map(|bytes| (bytes, Coding::Gzip));
    let (body, length, coding) = match brotli.or_else(gzip) {
        Some((bytes, coding)) => (Body::from(bytes.clone()), bytes.len() as u64, coding),
        None => {
            let file = match tokio::fs::File::open(&snapshot.path).await {
                Ok(file) => file,
                Err(error) => {
                    tracing::error!(component = "http", event = "snapshot.unreadable", path = %snapshot.path.display(), error = %error, "active snapshot file cannot be opened");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            (Body::from_stream(ReaderStream::with_capacity(file, 256 * 1024)), snapshot.bytes, Coding::Identity)
        }
    };

    let mut response = Response::new(body);
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.sqlite3"));
    // Always revalidate: the 304 is cheap and a changed snapshot is picked up at once.
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE));
    out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    out.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    if let Ok(value) = HeaderValue::from_str(&snapshot.etag) {
        out.insert(header::ETAG, value);
    }
    if let Some(value) = coding.header() {
        out.insert(header::CONTENT_ENCODING, value);
    }
    response
}

/// A body made once per snapshot, with its own ETag: fresh for five minutes.
async fn per_snapshot(headers: &HeaderMap, etag: &str, content_type: &'static str, body: &Kept) -> Response {
    const KEEP: &str = "public, max-age=300, stale-while-revalidate=86400";
    if if_none_match(headers, etag) {
        return not_modified(etag, KEEP);
    }
    let (bytes, coding) = body.get(Coding::of(headers)).await;
    respond(bytes, coding, content_type, KEEP, etag)
}

/// `GET /api/map.json`: the map of the programs for the landing page of the browser app. Laid out
/// when the snapshot was opened (`catalog::graph`); this only hands it on.
pub async fn program_map(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    match &snapshot.program_map {
        Some((_, json, etag)) => per_snapshot(&headers, etag, "application/json", json).await,
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /sitemap.xml`: every page a search engine should know: the three entrances, every module
/// and every current program with its views. Filters of the lists are not pages (`app::seo`).
/// Each page with the time it last changed where the warm-up has seen it (`lastmod`); the sitemap
/// is made anew once a round of the warm-up has finished, and its ETag is its content's.
pub async fn sitemap(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    let round = state.changes.as_ref().map_or(0, |changes| changes.rounds());
    let made = snapshot.sitemap.lock().ok().and_then(|made| made.clone()).filter(|(made_in, ..)| *made_in == round);
    let (etag, body) = match made {
        Some((_, etag, body)) => (etag, body),
        None => {
            // A few hundred queries (the study directions of every program): off the threads that
            // answer requests, as the warm-up does.
            let (from, changes, public_url) = (snapshot.clone(), state.changes.clone(), state.public_url.clone());
            let xml = match tokio::task::spawn_blocking(move || sitemap_xml(&from, changes.as_deref(), &public_url)).await {
                Ok(Ok(xml)) => xml,
                Ok(Err(error)) => {
                    tracing::error!(component = "http", event = "sitemap.failed", error = %error, "the sitemap could not be read from the snapshot");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
                Err(error) => {
                    tracing::error!(component = "http", event = "sitemap.failed", error = %error, "the sitemap task failed");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            let etag = crate::snapshot::content_etag("sitemap", xml.as_bytes());
            let body = Arc::new(Kept::new(Bytes::from(xml)));
            if let Ok(mut made) = snapshot.sitemap.lock() {
                *made = Some((round, etag.clone(), body.clone()));
            }
            (etag, body)
        }
    };
    per_snapshot(&headers, &etag, "application/xml; charset=utf-8", &body).await
}

/// The sitemap's XML: every page of `sitemap_paths`, with the time it last changed where
/// `changes` knows it, and the same page in every language of the site (`hreflang`; the
/// default language's is also the page for everybody else, `x-default`).
fn sitemap_xml(snapshot: &crate::snapshot::Snapshot, changes: Option<&crate::lastmod::Changes>, public_url: &str) -> Result<String, catalog::DbError> {
    use catalog::Locale;
    let escape = |text: &str| text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n");
    let pages = sitemap_pages(snapshot)?;
    for locale in Locale::ALL.iter().copied() {
        for page in &pages {
            let path = locale.path(page);
            let address = escape(&format!("{public_url}{path}"));
            let mut alternates: String = Locale::ALL
                .iter()
                .map(|other| format!("<xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>", other.code(), escape(&format!("{public_url}{}", other.path(page)))))
                .collect();
            alternates.push_str(&format!("<xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\"/>", escape(&format!("{public_url}{}", Locale::default().path(page)))));
            match changes.and_then(|changes| changes.since(&path)) {
                Some(since) => xml.push_str(&format!("<url><loc>{address}</loc><lastmod>{}</lastmod>{alternates}</url>\n", escape(&since))),
                None => xml.push_str(&format!("<url><loc>{address}</loc>{alternates}</url>\n")),
            }
        }
    }
    xml.push_str("</urlset>\n");
    Ok(xml)
}

/// The pages of the sitemap in every language (`sitemap_pages`), the default language's first.
/// The warm-up of the cache renders the same list (`warm`).
pub fn sitemap_paths(snapshot: &crate::snapshot::Snapshot) -> Result<Vec<String>, catalog::DbError> {
    let pages = sitemap_pages(snapshot)?;
    Ok(catalog::Locale::ALL.iter().flat_map(|locale| pages.iter().map(move |page| locale.path(page))).collect())
}

/// The pages of the sitemap as paths of the app (without a language), in its order: the three
/// entrances, every current program with its views (the plan of each further study direction
/// after the first's), every module.
pub fn sitemap_pages(snapshot: &crate::snapshot::Snapshot) -> Result<Vec<String>, catalog::DbError> {
    use catalog::url::{ProgramTab, ProgramUrl};
    type Listed = (Vec<String>, Vec<(catalog::rows::Program, usize)>);
    let mut listed: Result<Listed, catalog::DbError> = Err(catalog::DbError::Unavailable("not run".to_string()));
    snapshot.with_db(&mut |db| {
        listed = catalog::queries::module_ids(db).and_then(|modules| {
            let mut programs = Vec::new();
            for program in catalog::queries::programs(db)?.into_iter().filter(|program| program.is_latest_po) {
                let plans = if program.has_plan { catalog::pages::study_plans(db, &program.id)? } else { 0 };
                programs.push((program, plans));
            }
            Ok((modules, programs))
        });
    })?;
    let (modules, programs) = listed?;
    let mut paths = vec![catalog::url::HOME.to_string(), catalog::url::CATALOG.to_string(), catalog::url::PROGRAMS.to_string()];
    for (program, plans) in &programs {
        for tab in ProgramTab::ALL.iter().copied().filter(|tab| tab.indexed()) {
            paths.push(catalog::url::program_path(&program.slug, tab));
            if tab == ProgramTab::Plan {
                paths.extend((2..=*plans).map(|variant| ProgramUrl::new(&program.slug, tab).with_variant(variant).path()));
            }
        }
    }
    paths.extend(modules.iter().map(|id| catalog::url::module_path(id)));
    Ok(paths)
}

/// `GET /calendar/<code>.ics`: a Studienplan as a calendar feed. The code carries semester, modules
/// and what is hidden (`catalog::timetable::subscription`); the timetable is made anew from the
/// active snapshot on every fetch, so exams the BTU publishes later arrive by themselves. Nothing is
/// kept, neither the code nor the calendar.
///
/// The code is read from the raw path with the function the gate uses (`subscription::code_of_path`),
/// so the two never disagree about what an address names, and an escape that spells no character
/// of a code is a 404 like every other wrong address (axum's `Path` would answer invalid UTF-8
/// with a 400).
pub async fn calendar(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    // Never a 5xx for a wrong address: the access log reports those as errors a human has to act on.
    let locale = language_of(&uri);
    let gone = || (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, "no-store")], texts(locale).no_calendar).into_response();
    let Some(subscription) = subscription::code_of_path(uri.path()).and_then(|code| Subscription::from_code(&code)) else { return gone() };
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    // A place to make it in (`busy`): a calendar service that finds none within the wait hears 503
    // and asks again later; the feeds of many subscribers never queue up without bound.
    let Some(_place) = state.feeds.enter().await else {
        return crate::busy::busy(120, false);
    };
    let started = Instant::now();
    let key = subscription.key().map(|key| key.key()).unwrap_or_default();
    // The feed speaks the language of its address: `/en/calendar/<code>.ics` is English.
    let locale = catalog::Locale::split(uri.path()).0;
    // A semester's rows and a few hundred entries: made off the threads that answer requests.
    let built = tokio::task::spawn_blocking(move || {
        let mut out: Result<String, catalog::DbError> = Err(catalog::DbError::Unavailable("not run".to_string()));
        let ran = snapshot.with_db(&mut |db| out = catalog::pages::calendar(db, &subscription, locale));
        ran.and(out)
    })
    .await;
    let ics = match built {
        Ok(Ok(ics)) => ics,
        Ok(Err(error)) => {
            tracing::error!(component = "http", event = "calendar.failed", error = %error, "a calendar feed could not be read from the snapshot");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(error) => {
            tracing::error!(component = "http", event = "calendar.failed", error = %error, "the calendar task failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // Size and time only: the code names what someone plans (`subscription::redacted_path`).
    tracing::debug!(component = "http", event = "calendar.served", bytes = ics.len(), ms = started.elapsed().as_millis() as u64, "a calendar feed was made");
    calendar_response(&headers, &crate::snapshot::content_etag("ics", ics.as_bytes()), ics, &key)
}

/// The answer of a feed. `private`: it is one person's plan, and a shared cache must not keep it.
/// A quarter of an hour fresh, then revalidated against the content's ETag, which only changes
/// with the plan or the data (the calendar never reads the clock). Compressed here, as it is made
/// (`encoding::FAST`), because the edge's compression does not take `text/calendar`. No search
/// engine is to list it.
fn calendar_response(headers: &HeaderMap, etag: &str, ics: String, key: &str) -> Response {
    const KEEP: &str = "private, max-age=900";
    if if_none_match(headers, etag) {
        return not_modified(etag, KEEP);
    }
    let coding = Coding::of(headers);
    let (body, coding) = encoding::smaller(Bytes::from(ics), coding, |ics| match coding {
        Coding::Brotli => encoding::brotli(ics, encoding::FAST),
        _ => encoding::gzip(ics),
    });
    let mut response = respond(body, coding, "text/calendar; charset=utf-8", KEEP, etag);
    let out = response.headers_mut();
    out.insert(header::HeaderName::from_static("x-robots-tag"), HeaderValue::from_static("noindex, nofollow"));
    let name = if key.is_empty() { "studienplan.ics".to_string() } else { format!("studienplan-{key}.ics") };
    if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{name}\"")) {
        out.insert(header::CONTENT_DISPOSITION, value);
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
            "schema_version": snapshot.schema_version,
            "gzip_bytes": snapshot.gzip.as_ref().map(|(_, bytes)| *bytes),
            "brotli_bytes": snapshot.brotli.get().map(Bytes::len),
            "data_changed_at": snapshot.meta.data_changed_at,
            "current_semester": snapshot.meta.current_semester,
            "activated_seconds_ago": snapshot.activated_at.elapsed().map(|d| d.as_secs()).unwrap_or(0),
        })
    });
    let semantic_model = state.semantic.as_ref().map(|model| model.path.as_str());
    let body = json!({
        "snapshot": snapshot,
        "radix_last_contact_seconds_ago": state.store.seconds_since_contact(),
        "html_cache": { "pages": cached_pages, "bytes": cached_bytes },
        "uptime_seconds": state.store.uptime().as_secs(),
        "build": state.build_id.as_ref(),
        "semantic_model": semantic_model,
    });
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

/// Where `folia healthcheck` asks whether the server is alive.
pub const LIVENESS: &str = "/livez";

/// `GET /livez`: the process answers requests, nothing more. For the container's HEALTHCHECK:
/// a supervisor that restarted the server over `/healthz` would take the site down exactly when
/// it could still serve its last snapshot (no answer from Radix), or before the first one arrived.
pub async fn alive() -> Response {
    ([(header::CACHE_CONTROL, "no-store")], "ok\n").into_response()
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

/// Whether a file of this type is worth compressing: not what is compressed already.
fn compressible(content_type: &str) -> bool {
    !matches!(content_type, "font/woff2" | "image/png" | "image/webp")
}

/// An embedded file with its compressed forms (`Kept`), one for the whole process: made when a
/// client first asks for them, the same bytes for everybody after that. `brotli` is its form made
/// ahead of time, where there is one (`birch::brotli`).
fn kept(body: &'static [u8], brotli: Option<&'static [u8]>) -> Arc<Kept> {
    /// By the address and the length of the bytes.
    type Files = Mutex<HashMap<(usize, usize), Arc<Kept>>>;
    static KEPT: OnceLock<Files> = OnceLock::new();
    let make = || {
        Arc::new(match brotli {
            Some(brotli) => Kept::with_brotli(Bytes::from_static(body), Bytes::from_static(brotli)),
            None => Kept::new(Bytes::from_static(body)),
        })
    };
    match KEPT.get_or_init(Default::default).lock() {
        Ok(mut kept) => kept.entry((body.as_ptr() as usize, body.len())).or_insert_with(make).clone(),
        Err(_) => make(),
    }
}

/// A file embedded in the binary, the same for as long as the process runs: the build is its ETag,
/// `keep` says how long a browser keeps it, and it goes out as the client takes it — brotli at
/// its best, made once per process off the threads that answer requests (`Kept`).
async fn embedded(state: &AppState, headers: &HeaderMap, keep: Keep, content_type: &'static str, body: &'static [u8], brotli: Option<&'static [u8]>) -> Response {
    let etag = format!("\"{}\"", state.build_id);
    if if_none_match(headers, &etag) {
        return not_modified(&etag, keep.header());
    }
    let (bytes, coding) = if compressible(content_type) { kept(body, brotli).get(Coding::of(headers)).await } else { (Bytes::from_static(body), Coding::Identity) };
    respond(bytes, coding, content_type, keep.header(), &etag)
}

/// `embedded`, kept as its address says (`Keep::of`).
async fn asset(state: &AppState, uri: &Uri, headers: &HeaderMap, content_type: &'static str, body: &'static [u8]) -> Response {
    embedded(state, headers, Keep::of(state, uri), content_type, body, None).await
}

/// A file of `app/assets` the build minifies (`assets`), by its path there; while the server
/// serves them live (`--live-assets`), the file as it is on disk right now.
async fn minified(state: &AppState, uri: &Uri, headers: &HeaderMap, path: &str) -> Response {
    let Some(file) = crate::assets::get(path) else { return StatusCode::NOT_FOUND.into_response() };
    match &state.live_assets {
        Some(dir) => live(headers, file.path, tokio::fs::read(dir.join(file.path)).await),
        None => asset(state, uri, headers, crate::assets::content_type(file.path), file.bytes).await,
    }
}

/// A file read from disk for this request (`--live-assets`): tagged by what it holds and asked
/// for again on every use, so that an edit is there with the next reload, and never compressed
/// (it goes to this machine).
fn live(headers: &HeaderMap, path: &str, file: std::io::Result<Vec<u8>>) -> Response {
    use std::hash::{Hash, Hasher};
    let body = match file {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(component = "http", event = "assets.live_failed", path, error = %error, "cannot read the file of a live asset");
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut hasher);
    let etag = format!("\"live-{:016x}\"", hasher.finish());
    if if_none_match(headers, &etag) {
        return not_modified(&etag, REVALIDATE);
    }
    respond(Bytes::from(body), Coding::Identity, crate::assets::content_type(path), REVALIDATE, &etag)
}

/// The service worker while the server serves `app/assets` live (`--live-assets`): it keeps
/// nothing and listens to no request, so an edited file, or a browser app built again, is there
/// with the next reload; and it drops what the worker of an earlier run kept.
pub const LIVE_SERVICE_WORKER: &str = "self.addEventListener(\"install\",()=>self.skipWaiting());self.addEventListener(\"activate\",e=>e.waitUntil(caches.keys().then(k=>Promise.all(k.map(n=>caches.delete(n)))).then(()=>self.clients.claim())));\n";

/// `GET /sw.js`: the service worker, with the build of this process written into it, so that a
/// new build installs a new worker and drops the shell the old one kept. Revalidated on every use
/// whatever its address (browsers check a worker for updates on their own as well): a worker
/// kept for a year would keep its build's shell for a year.
pub async fn service_worker(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.live_assets.is_some() {
        return live(&headers, "sw.js", Ok(LIVE_SERVICE_WORKER.as_bytes().to_vec()));
    }
    static SOURCE: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
    let body = SOURCE.get_or_init(|| {
        let source = crate::assets::text("sw.js").replace("__BUILD__", &state.build_id);
        Box::leak(source.into_boxed_str()).as_bytes()
    });
    embedded(&state, &headers, Keep::Revalidate, "text/javascript; charset=utf-8", body, None).await
}

pub async fn stylesheet(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    minified(&state, &uri, &headers, "app.css").await
}

pub async fn favicon(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    minified(&state, &uri, &headers, "favicon.svg").await
}

/// `GET /assets/shots/<name>.webp`: the screenshots in the start page's carousel, light and dark,
/// wide and for phones (`e2e/showcase-shots.mjs` takes them). Embedded like every other asset.
pub async fn showcase_shot(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let body: &'static [u8] = match file.as_str() {
        "catalog.webp" => include_bytes!("../../app/assets/shots/catalog.webp"),
        "catalog-dark.webp" => include_bytes!("../../app/assets/shots/catalog-dark.webp"),
        "catalog-phone.webp" => include_bytes!("../../app/assets/shots/catalog-phone.webp"),
        "catalog-phone-dark.webp" => include_bytes!("../../app/assets/shots/catalog-phone-dark.webp"),
        "program.webp" => include_bytes!("../../app/assets/shots/program.webp"),
        "program-dark.webp" => include_bytes!("../../app/assets/shots/program-dark.webp"),
        "program-phone.webp" => include_bytes!("../../app/assets/shots/program-phone.webp"),
        "program-phone-dark.webp" => include_bytes!("../../app/assets/shots/program-phone-dark.webp"),
        "module.webp" => include_bytes!("../../app/assets/shots/module.webp"),
        "module-dark.webp" => include_bytes!("../../app/assets/shots/module-dark.webp"),
        "module-phone.webp" => include_bytes!("../../app/assets/shots/module-phone.webp"),
        "module-phone-dark.webp" => include_bytes!("../../app/assets/shots/module-phone-dark.webp"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    asset(&state, &uri, &headers, "image/webp", body).await
}

/// `GET /assets/birch/<name>.svg`: the birch around the app — the crown along the top in each
/// season and the roots of the ground (`design/birch/birch.mjs` draws them; the stylesheet colours
/// them). Embedded once (`birch::file`), minified by the build: the link-preview cards draw the
/// same crown. The wood goes out as it was drawn, with the brotli it was drawn with
/// (`birch::brotli`), the others as brotli made here. Read from disk while the server serves
/// `app/assets` live (`--live-assets`).
pub async fn birch(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(body) = crate::birch::file(&file) else { return StatusCode::NOT_FOUND.into_response() };
    match &state.live_assets {
        Some(dir) => live(&headers, &file, tokio::fs::read(dir.join("birch").join(&file)).await),
        None => embedded(&state, &headers, Keep::of(&state, &uri), "image/svg+xml", body.as_bytes(), crate::birch::brotli(&file)).await,
    }
}

/// `GET /assets/icons.svg`: the icons of the app as one sprite (`app::icons`), which every icon
/// on a page points at.
pub async fn icons(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    static SPRITE: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
    let body = SPRITE.get_or_init(|| Box::leak(app::icons::sprite().into_boxed_str()).as_bytes());
    asset(&state, &uri, &headers, "image/svg+xml", body).await
}

/// `GET /assets/og.png`: the site's standard picture for link previews, in the season's crown
/// (`design/og/og.html` in four pictures), like every card the server draws; `/en/assets/og.png`
/// the same in English. The picture changes with the season within a build: revalidated whatever
/// its address, with the season in its ETag.
pub async fn og_image(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let (season, locale) = (Season::now(), language_of(&uri));
    let etag = format!("\"{}-{}\"", state.build_id, season.name());
    if if_none_match(&headers, &etag) {
        return not_modified(&etag, REVALIDATE);
    }
    respond(Bytes::from_static(standard_picture(season, locale)), Coding::Identity, "image/png", REVALIDATE, &etag)
}

/// The standard picture of a season in a language.
pub fn standard_picture(season: Season, locale: Locale) -> &'static [u8] {
    match (locale, season) {
        (Locale::De, Season::Spring) => include_bytes!("../../app/assets/og-spring.png"),
        (Locale::De, Season::Summer) => include_bytes!("../../app/assets/og-summer.png"),
        (Locale::De, Season::Autumn) => include_bytes!("../../app/assets/og-autumn.png"),
        (Locale::De, Season::Winter) => include_bytes!("../../app/assets/og-winter.png"),
        (Locale::En, Season::Spring) => include_bytes!("../../app/assets/og-spring-en.png"),
        (Locale::En, Season::Summer) => include_bytes!("../../app/assets/og-summer-en.png"),
        (Locale::En, Season::Autumn) => include_bytes!("../../app/assets/og-autumn-en.png"),
        (Locale::En, Season::Winter) => include_bytes!("../../app/assets/og-winter-en.png"),
    }
}

/// The mark as pictures (`design/logo/render-icons.mjs`): `/favicon.ico` for what asks for it
/// unprompted, the icon of iOS, and the icons the manifest names.
pub async fn favicon_ico(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/x-icon", include_bytes!("../../app/assets/favicon.ico")).await
}

pub async fn touch_icon(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/apple-touch-icon.png")).await
}

pub async fn icon_192(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/icon-192.png")).await
}

pub async fn icon_512(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/icon-512.png")).await
}

pub async fn icon_maskable(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/icon-maskable-512.png")).await
}

pub async fn icon_maskable_large(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/icon-maskable-1024.png")).await
}

pub async fn icon_monochrome(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "image/png", include_bytes!("../../app/assets/icon-monochrome-512.png")).await
}

/// `GET /assets/launch/<width>x<height>[-dark].png`: a launch screen of the installed app on iOS
/// (`app::launch`), for the screens a page names; drawn on its first request and kept (`launch`).
/// Kept like the other assets: it changes with the build at most.
pub async fn launch_screen(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(picture) = app::launch::Picture::from_file(&file) else { return StatusCode::NOT_FOUND.into_response() };
    let (etag, keep) = (format!("\"{}\"", state.build_id), Keep::of(&state, &uri));
    if if_none_match(&headers, &etag) {
        return not_modified(&etag, keep.header());
    }
    match state.launch.get(picture).await {
        Ok(png) => respond(png, Coding::Identity, "image/png", keep.header(), &etag),
        Err(error) => {
            tracing::error!(component = "launch", event = "launch.failed", file, error = %error, "a launch screen could not be drawn");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// The language of a card or a feed: the prefix of its address, as for a page (`/en/cards/…`).
fn language_of(uri: &Uri) -> Locale {
    Locale::split(uri.path()).0
}

/// The key of a card in the cache of cards: what it shows and its language. The language comes
/// last, so that the key of a shared plan still begins with `cards::SHARED_PLAN` (the log names
/// such keys without their code).
fn card_key(key: String, locale: Locale) -> String {
    format!("{key}@{}", locale.code())
}

/// `GET /cards/module/<id>.png`: the picture of a module's link preview (`cards`).
pub async fn module_card(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(id) = file.strip_suffix(".png").filter(|id| !id.is_empty() && id.len() <= 32) else { return StatusCode::NOT_FOUND.into_response() };
    let id = id.to_string();
    let locale = language_of(&uri);
    let t = texts(locale);
    card(&state, &headers, card_key(format!("m:{id}"), locale), move |db| {
        Ok(catalog::queries::module(db, &id)?.map(|module| {
            let mut facts = Vec::new();
            if !module.offer_status.is(catalog::labels::OfferStatus::Active) {
                facts.push(module.offer_status.label(locale).to_string());
            }
            if module.credits.is_some() {
                facts.push(app::format::credits(module.credits, locale));
            }
            if let Some(season) = &module.turnus_season {
                facts.push(match &module.turnus_parity {
                    Some(parity) => format!("{} ({})", season.label(locale), parity.label(locale)),
                    None => season.label(locale).to_string(),
                });
            }
            match (module.teaches_german, module.teaches_english) {
                (Some(true), Some(true)) => facts.push(t.teaches_both.to_string()),
                (Some(true), _) => facts.push(t.teaches_german.to_string()),
                (_, Some(true)) => facts.push(t.teaches_english.to_string()),
                _ => {}
            }
            if let Some(exam) = &module.exam_form {
                facts.push(app::format::exam_short(exam, locale));
            }
            CardText { eyebrow: (t.card_module)(&module.id), headline: Headline::Title(module.title), facts, note: module.department }
        }))
    })
    .await
}

/// `GET /cards/program/<slug>.png`: the picture of a program's link preview.
pub async fn program_card(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(slug) = file.strip_suffix(".png").filter(|slug| !slug.is_empty() && slug.len() <= 200) else { return StatusCode::NOT_FOUND.into_response() };
    let slug = slug.to_string();
    let locale = language_of(&uri);
    let t = texts(locale);
    card(&state, &headers, card_key(format!("p:{slug}"), locale), move |db| {
        Ok(catalog::queries::program_by_slug(db, &slug)?.map(|program| {
            let mut facts = vec![program.degree().to_string()];
            if let Some(variant) = &program.study_variant {
                facts.push(variant.label(locale).to_string());
            }
            facts.push(match program.po_year {
                Some(year) => (t.regulations)(&year.to_string()),
                None => (t.regulations)(&program.po_version),
            });
            let mut note = vec![(t.curricular_modules)(&app::format::count(program.curricular_modules.max(0) as u64, locale))];
            if program.has_plan {
                note.push(t.with_plan.to_string());
            }
            CardText { eyebrow: t.card_program.to_string(), headline: Headline::Title(program.name), facts, note: Some(note.join("  ·  ")) }
        }))
    })
    .await
}

/// `GET /cards/bookmarks.png`: the picture of the Merkliste's link preview. The same for everybody:
/// what is marked lives in the visitor's browser (R20), so it says what the Merkliste is.
pub async fn bookmarks_card_png(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let locale = language_of(&uri);
    card(&state, &headers, card_key("b".to_string(), locale), move |_| Ok(Some(bookmarks_card(locale)))).await
}

pub fn bookmarks_card(locale: Locale) -> CardText {
    let t = texts(locale);
    CardText {
        eyebrow: t.bookmarks_eyebrow.to_string(),
        headline: Headline::Title(t.bookmarks_title.to_string()),
        facts: t.bookmarks_facts.iter().map(|fact| fact.to_string()).collect(),
        note: Some(t.bookmarks_note.to_string()),
    }
}

/// `GET /cards/studyplan.png`: the picture of the Stundenplan's link preview, the same for
/// everybody (a plan lives in the browser, R20), with the semester the catalog has dates for. A
/// shared plan has its own (`shared_plan_card`).
pub async fn studyplan_card_png(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let locale = language_of(&uri);
    card(&state, &headers, card_key("s".to_string(), locale), move |db| {
        let meta = catalog::queries::meta(db)?;
        let semester = meta.current_semester.as_deref().and_then(catalog::timetable::semester::SemesterKey::parse).map(|key| key.label(locale));
        Ok(Some(studyplan_card(semester.as_deref(), locale)))
    })
    .await
}

/// `GET /cards/studyplan/<code>.png`: the picture of a shared Stundenplan's link preview
/// (`timetable::share`): its modules as tags in the tones of the plan, by the names the week grid
/// gives them („MIT-1", „AuP"), how many and how many credits, and their titles. A code that does
/// not decode, or names no module the catalog knows, is a 404.
pub async fn shared_plan_card(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
    let Some((code, plan)) = catalog::timetable::share::code_of_card(&file).and_then(|code| Some((code, catalog::timetable::share::SharedPlan::from_code(code)?))) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let locale = language_of(&uri);
    let key = card_key(format!("{}{code}", crate::cards::SHARED_PLAN), locale);
    card(&state, &headers, key, move |db| Ok(catalog::pages::shared_plan(db, &plan, locale)?.and_then(|shared| shared_plan_text(&shared, locale)))).await
}

/// What the card of a shared plan says; `None` without a module the catalog knows.
pub fn shared_plan_text(shared: &catalog::pages::SharedPlanData, locale: Locale) -> Option<CardText> {
    if shared.modules.is_empty() {
        return None;
    }
    let t = texts(locale);
    let mut facts = vec![app::format::modules(i64::try_from(shared.modules.len()).unwrap_or(i64::MAX), locale)];
    if shared.modules.iter().any(|module| module.credits.is_some()) {
        facts.push(app::format::credits(Some(shared.credits()), locale));
    }
    if let Some(program) = &shared.program {
        facts.push(format!("{} ({})", program.name, program.degree()));
    }
    Some(CardText {
        eyebrow: (t.studyplan_of)(&shared.label),
        headline: Headline::Tags(shared.modules.iter().map(|module| module.name.clone()).collect()),
        facts,
        note: Some(shared.modules.iter().map(|module| module.title.as_str()).collect::<Vec<_>>().join(" · ")),
    })
}

pub fn studyplan_card(semester: Option<&str>, locale: Locale) -> CardText {
    let t = texts(locale);
    CardText {
        eyebrow: semester.map_or_else(|| t.studyplan_eyebrow.to_string(), t.studyplan_of),
        headline: Headline::Title(t.studyplan_title.to_string()),
        facts: t.studyplan_facts.iter().map(|fact| fact.to_string()).collect(),
        note: Some(t.studyplan_note.to_string()),
    }
}

/// A card: what it says is read from the snapshot, the picture is kept or drawn. When the server
/// has no free place to draw (or no snapshot yet), the site's standard picture answers instead,
/// not to be kept, so the next fetch gets the real one.
async fn card(state: &AppState, headers: &HeaderMap, key: String, read: impl FnOnce(&dyn catalog::Database) -> Result<Option<CardText>, catalog::DbError>) -> Response {
    // The language is the key's last part (`card_key`).
    let locale = key.rsplit_once('@').and_then(|(_, code)| Locale::from_code(code)).unwrap_or_default();
    let standard = || {
        let mut response = Response::new(Body::from(standard_picture(Season::now(), locale)));
        let out = response.headers_mut();
        out.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
        out.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    };
    let Some(snapshot) = state.store.current() else { return standard() };
    // Only asked for a card that is not known for this snapshot, and only with a free place.
    let text = || {
        let mut read = Some(read);
        let mut text = Ok(None);
        let ran = snapshot.with_db(&mut |db| {
            if let Some(read) = read.take() {
                text = read(db);
            }
        });
        ran.and(text).map_err(|error| error.to_string())
    };
    match state.cards.get(&key, state.store.generation(), Season::now(), text).await {
        Ok(Card::Drawn(etag, png)) => {
            let mut response = if if_none_match(headers, &etag) { StatusCode::NOT_MODIFIED.into_response() } else { Response::new(Body::from(png)) };
            let out = response.headers_mut();
            out.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
            // A day for whoever keeps it; the ETag settles the rest. Fetchers of messengers keep
            // pictures far longer than that anyway.
            out.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"));
            if let Ok(value) = HeaderValue::from_str(&etag) {
                out.insert(header::ETAG, value);
            }
            response
        }
        Ok(Card::Unknown) => StatusCode::NOT_FOUND.into_response(),
        Ok(Card::Busy) => standard(),
        Err(error) => {
            tracing::error!(component = "cards", event = "card.failed", key, error = %error, "a card could not be read from the snapshot or drawn");
            standard()
        }
    }
}

/// `GET /manifest.webmanifest`: name, colours and icons of the site for a home screen; `/en/…` the
/// same in English, an app of its own that starts at `/en` (a home screen keeps each language's).
pub async fn manifest(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    static MANIFESTS: std::sync::OnceLock<Vec<(Locale, &'static [u8])>> = std::sync::OnceLock::new();
    let locale = language_of(&uri);
    let manifests = MANIFESTS.get_or_init(|| Locale::ALL.iter().map(|locale| (*locale, &*Box::leak(localized_manifest(*locale).into_bytes().into_boxed_slice()))).collect());
    let body = manifests.iter().find(|(language, _)| *language == locale).map_or(&include_bytes!("../../app/assets/manifest.webmanifest")[..], |(_, body)| body);
    asset(&state, &uri, &headers, "application/manifest+json", body).await
}

/// `app/assets/manifest.webmanifest` in a language: its name, description, language, and where the
/// app starts. The file is the default language's; a manifest the server cannot read stays as it is.
pub fn localized_manifest(locale: Locale) -> String {
    let file = include_str!("../../app/assets/manifest.webmanifest");
    let Ok(mut manifest) = serde_json::from_str::<serde_json::Value>(file) else { return file.to_string() };
    let t = texts(locale);
    let home = locale.path("/");
    if let Some(fields) = manifest.as_object_mut() {
        fields.insert("name".to_string(), json!(t.app_name));
        fields.insert("description".to_string(), json!(t.app_description));
        fields.insert("lang".to_string(), json!(locale.code()));
        fields.insert("id".to_string(), json!(home));
        fields.insert("start_url".to_string(), json!(home));
    }
    serde_json::to_string(&manifest).unwrap_or_else(|_| file.to_string())
}

pub async fn font(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "font/woff2", include_bytes!("../../app/assets/inter-latin.woff2")).await
}

pub async fn enhance_script(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    minified(&state, &uri, &headers, "enhance.js").await
}

/// `GET /assets/boot.js`, with the schema this build reads written into it
/// (`catalog::SCHEMA_VERSION`): it refuses a local copy of the catalog of an older one.
pub async fn boot_script(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    // And the address of the semantic search's model, or `null` without one: fixed while the
    // process runs, as is the build the script is kept under.
    let model = serde_json::to_string(&state.semantic.as_ref().map(|model| model.path.as_str())).unwrap_or_else(|_| "null".into());
    let with_schema = |source: &str| source.replace("__SCHEMA__", &catalog::SCHEMA_VERSION.to_string()).replace("__SEMANTIC_MODEL__", &model);
    if let Some(dir) = &state.live_assets {
        return live(&headers, "boot.js", tokio::fs::read_to_string(dir.join("boot.js")).await.map(|source| with_schema(&source).into_bytes()));
    }
    // Made once per model, which is one per process (the tests serve several).
    type Sources = Mutex<HashMap<String, &'static [u8]>>;
    static SOURCES: OnceLock<Sources> = OnceLock::new();
    let make = || -> &'static [u8] { Box::leak(with_schema(crate::assets::text("boot.js")).into_boxed_str()).as_bytes() };
    let body = *SOURCES.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner()).entry(model.clone()).or_insert_with(make);
    asset(&state, &uri, &headers, "text/javascript; charset=utf-8", body).await
}

pub async fn sql_js(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    minified(&state, &uri, &headers, "sql-wasm.js").await
}

pub async fn sql_wasm(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    asset(&state, &uri, &headers, "application/wasm", include_bytes!("../../app/assets/sql-wasm.wasm")).await
}

/// `GET /models/e5-de-en-<hash>.bin`: the browser's model of the semantic search (`semantic`),
/// when the server has one and the address names it. The address names its content, so it is kept
/// for good whatever the build; the service worker keeps it apart from the shell of a build.
pub async fn semantic_model(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(model) = state.semantic.as_ref().filter(|model| model.path == uri.path()) else { return StatusCode::NOT_FOUND.into_response() };
    if if_none_match(&headers, &model.etag) {
        return not_modified(&model.etag, Keep::IMMUTABLE);
    }
    let (bytes, coding) = model.body.get(Coding::of(&headers)).await;
    respond(bytes, coding, "application/octet-stream", Keep::IMMUTABLE, &model.etag)
}

/// A file of the browser app as it is served: its ETag, and its bytes with their compressed
/// forms (`Kept`; the bundle, 34 MB, is brotli 1.7 MB, made in 2 s).
pub struct Package {
    pub etag: String,
    pub body: Kept,
}

/// `GET /pkg/<file>`: the browser app built by scripts/build-client.sh, from `<site-root>/pkg`.
/// Read and compressed once per file version and kept in memory; kept by browsers like the files
/// embedded in the binary (`Keep`): `boot.js` asks for it with the build of its page. (After
/// building the app again, start the server again, as its worker has it under that address too.)
pub async fn package(State(state): State<AppState>, Path(file): Path<String>, uri: Uri, headers: HeaderMap) -> Response {
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
    let keep = Keep::of(&state, &uri);
    if if_none_match(&headers, &etag) {
        return not_modified(&etag, keep.header());
    }

    let current = |state: &AppState| state.packages.lock().ok().and_then(|packages| packages.get(&file).filter(|package| package.etag == etag).cloned());
    let package = match current(&state) {
        Some(package) => package,
        None => {
            let Ok(raw) = tokio::fs::read(&path).await else { return StatusCode::NOT_FOUND.into_response() };
            let read = Arc::new(Package { etag: etag.clone(), body: Kept::new(Bytes::from(raw)) });
            // Whoever read it first keeps it: its compressed forms are made once.
            match state.packages.lock() {
                Ok(mut packages) => match packages.get(&file).filter(|package| package.etag == etag) {
                    Some(package) => package.clone(),
                    None => {
                        packages.insert(file.clone(), read.clone());
                        read
                    }
                },
                Err(_) => read,
            }
        }
    };
    let (bytes, coding) = package.body.get(Coding::of(&headers)).await;
    respond(bytes, coding, content_type, keep.header(), &etag)
}

pub async fn robots(State(state): State<AppState>) -> Response {
    // Calendar feeds are allowed in both answers: Google Calendar reads robots.txt before it
    // fetches a subscription and gives up on a disallowed one („robots.txt prevents us from
    // crawling the url"). A feed stays out of search indexes by its own `X-Robots-Tag: noindex`,
    // which a crawler only sees when it may fetch the address.
    // In closed testing (`access`) there is nothing else for a crawler but a login page. The
    // longer rule wins (RFC 9309); `Allow` comes first for crawlers that take the first match.
    if state.gate.is_some() {
        // Every language's feeds (`/calendar/`, `/en/calendar/`).
        let allowed: String = Locale::ALL.iter().map(|locale| format!("Allow: {}\n", locale.path(subscription::CALENDAR_PREFIX))).collect();
        let body = format!("User-agent: *\n{allowed}Disallow: /\n");
        return ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response();
    }
    // Disallowing `/api/` also keeps a crawler that runs JavaScript (Googlebot) on the server's page:
    // without `/api/status` the browser app does not start (`app/assets/boot.js`), so it indexes the
    // page as the server wrote it and never downloads the catalog to let the app replace it.
    //
    // The views of the lists are no pages (`catalog::url::listed`): every filter, order and search
    // of the catalog, the program overview and the Merkliste, and every page of a filtered list.
    // Their links carry `rel="nofollow"`, but that is a hint, and a crawler keeps asking for the
    // addresses it knows: Googlebot had fetched 250,000 of them by 2026-09-30, walking the filters.
    // So they are off limits here, as Google advises for filters. The pages of the unfiltered
    // catalog stay open (`/catalog?page=<n>`, the way to every module): the longer rule wins, so
    // `Allow: /catalog?page=` beats `Disallow: /catalog?`, and `Disallow: /catalog?page=*&` beats
    // it again for a page with more behind it, a view (the canonical address writes `page` after
    // every filter: `/catalog?turnus=winter&page=2`). Each rule stands before the one it beats,
    // for crawlers that take the first match. The Stundenplan keeps its query open: a plan handed
    // on by a link (`?share=`) is a page of its own for link previews, and says `noindex` itself.
    let mut body = String::from("User-agent: *\nDisallow: /api/\n");
    for locale in Locale::ALL {
        let (list, overview, marked) = (locale.path(catalog::url::CATALOG), locale.path(catalog::url::PROGRAMS), locale.path(catalog::url::BOOKMARKS));
        body.push_str(&format!("Disallow: {list}?page=*&\nAllow: {list}?page=\nDisallow: {list}?\nDisallow: {overview}?\nDisallow: {marked}?\n"));
    }
    // A link preview fetches the one address somebody shares, a filtered list as well, and never
    // walks the filters: the crawlers of X, LinkedIn and Facebook read robots.txt before they draw
    // a card (Slack's does not read it at all), so they have a group of their own.
    body.push_str("\nUser-agent: Twitterbot\nUser-agent: LinkedInBot\nUser-agent: facebookexternalhit\nDisallow: /api/\n");
    body.push_str(&format!("\nSitemap: {}/sitemap.xml\n", state.public_url));
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=86400")], body).into_response()
}
