//! What is not a rendered page: the snapshot for browsers, status, health, static assets, and a
//! Studienplan as a calendar feed.

use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use catalog::timetable::subscription::{self, Subscription};
use serde_json::json;
use tokio_util::io::ReaderStream;

use crate::cards::{Card, CardText};
use crate::AppState;

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|tag| tag.trim().trim_start_matches("W/") == etag))
}

fn accepts_gzip(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|encoding| encoding.trim().starts_with("gzip")))
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

    let (path, length, compressed) = match (&snapshot.gzip, accepts_gzip(&headers)) {
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
    let use_gzip = accepts_gzip(headers) && !body.1.is_empty();
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
        Some((_, json, compressed, etag)) => per_snapshot(&headers, etag, "application/json", &(json.clone(), compressed.clone())),
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
    let gone = || (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, "no-store")], "Kein Kalender unter dieser Adresse.\n").into_response();
    let Some(subscription) = subscription::code_of_path(uri.path()).and_then(|code| Subscription::from_code(&code)) else { return gone() };
    let Some(snapshot) = state.store.current() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "30")], "no snapshot yet").into_response();
    };
    let started = Instant::now();
    let key = subscription.key().map(|key| key.key()).unwrap_or_default();
    // A semester's rows and a few hundred entries: made off the threads that answer requests.
    let built = tokio::task::spawn_blocking(move || {
        let mut out: Result<String, catalog::DbError> = Err(catalog::DbError::Unavailable("not run".to_string()));
        let ran = snapshot.with_db(&mut |db| out = catalog::pages::calendar(db, &subscription));
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
/// with the plan or the data (the calendar never reads the clock). Compressed here, because the
/// edge's compression does not take `text/calendar`. No search engine is to list it.
fn calendar_response(headers: &HeaderMap, etag: &str, ics: String, key: &str) -> Response {
    let shared = |out: &mut HeaderMap| {
        out.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=900"));
        out.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
        if let Ok(value) = HeaderValue::from_str(etag) {
            out.insert(header::ETAG, value);
        }
    };
    if if_none_match(headers, etag) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        shared(response.headers_mut());
        return response;
    }
    let compressed = accepts_gzip(headers).then(|| crate::cache::gzip(ics.as_bytes())).filter(|bytes| !bytes.is_empty());
    let gzipped = compressed.is_some();
    let mut response = Response::new(match compressed {
        Some(bytes) => Body::from(bytes),
        None => Body::from(ics),
    });
    let out = response.headers_mut();
    shared(out);
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/calendar; charset=utf-8"));
    out.insert(header::HeaderName::from_static("x-robots-tag"), HeaderValue::from_static("noindex, nofollow"));
    let name = if key.is_empty() { "studienplan.ics".to_string() } else { format!("studienplan-{key}.ics") };
    if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{name}\"")) {
        out.insert(header::CONTENT_DISPOSITION, value);
    }
    if gzipped {
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
            "schema_version": snapshot.schema_version,
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

/// A file embedded in the binary. Revalidated on every use (the 304 costs nothing and a new
/// build shows up at once); compressed once per process.
fn asset(state: &AppState, headers: &HeaderMap, content_type: &'static str, body: &'static [u8]) -> Response {
    static COMPRESSED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, axum::body::Bytes>>> = std::sync::OnceLock::new();

    let etag = format!("\"{}\"", state.build_id);
    if if_none_match(headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    // Fonts are compressed already.
    let compressed = (accepts_gzip(headers) && !matches!(content_type, "font/woff2" | "image/png" | "image/webp") && body.len() > 1024)
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

/// `GET /sw.js`: the service worker, with the build of this process written into it, so that a
/// new build installs a new worker and drops the shell the old one kept. Revalidated on every use
/// like the other assets (browsers check a worker for updates on their own as well).
pub async fn service_worker(State(state): State<AppState>, headers: HeaderMap) -> Response {
    static SOURCE: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
    let body = SOURCE.get_or_init(|| {
        let source = include_str!("../../app/assets/sw.js").replace("__BUILD__", &state.build_id);
        Box::leak(source.into_boxed_str()).as_bytes()
    });
    asset(&state, &headers, "text/javascript; charset=utf-8", body)
}

pub async fn stylesheet(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/css; charset=utf-8", include_bytes!("../../app/assets/app.css"))
}

pub async fn favicon(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/svg+xml", include_bytes!("../../app/assets/favicon.svg"))
}

/// `GET /assets/shots/<name>.webp`: the screenshots in the start page's carousel, light and dark,
/// wide and for phones (`e2e/showcase-shots.mjs` takes them). Embedded like every other asset.
pub async fn showcase_shot(State(state): State<AppState>, Path(file): Path<String>, headers: HeaderMap) -> Response {
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
    asset(&state, &headers, "image/webp", body)
}

pub async fn og_image(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/og.png"))
}

/// The mark as pictures (`design/logo/render-icons.mjs`): `/favicon.ico` for what asks for it
/// unprompted, the icon of iOS, and the icons the manifest names.
pub async fn favicon_ico(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/x-icon", include_bytes!("../../app/assets/favicon.ico"))
}

pub async fn touch_icon(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/apple-touch-icon.png"))
}

pub async fn icon_192(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/icon-192.png"))
}

pub async fn icon_512(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/icon-512.png"))
}

pub async fn icon_maskable(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "image/png", include_bytes!("../../app/assets/icon-maskable-512.png"))
}

/// `GET /cards/module/<id>.png`: the picture of a module's link preview (`cards`).
pub async fn module_card(State(state): State<AppState>, Path(file): Path<String>, headers: HeaderMap) -> Response {
    let Some(id) = file.strip_suffix(".png").filter(|id| !id.is_empty() && id.len() <= 32) else { return StatusCode::NOT_FOUND.into_response() };
    let id = id.to_string();
    card(&state, &headers, format!("m:{id}"), move |db| {
        Ok(catalog::queries::module(db, &id)?.map(|module| {
            let mut facts = Vec::new();
            if !module.offer_status.is(catalog::labels::OfferStatus::Active) {
                facts.push(module.offer_status.label().to_string());
            }
            if module.credits.is_some() {
                facts.push(app::format::credits(module.credits));
            }
            if let Some(season) = &module.turnus_season {
                facts.push(match &module.turnus_parity {
                    Some(parity) => format!("{} ({})", season.label(), parity.label()),
                    None => season.label().to_string(),
                });
            }
            match (module.teaches_german, module.teaches_english) {
                (Some(true), Some(true)) => facts.push("Deutsch und Englisch".to_string()),
                (Some(true), _) => facts.push("Deutsch".to_string()),
                (_, Some(true)) => facts.push("Englisch".to_string()),
                _ => {}
            }
            if let Some(exam) = &module.exam_form {
                facts.push(app::format::exam_short(exam));
            }
            CardText { eyebrow: format!("Modul {}", module.id), title: module.title, facts, note: module.department }
        }))
    })
    .await
}

/// `GET /cards/program/<slug>.png`: the picture of a program's link preview.
pub async fn program_card(State(state): State<AppState>, Path(file): Path<String>, headers: HeaderMap) -> Response {
    let Some(slug) = file.strip_suffix(".png").filter(|slug| !slug.is_empty() && slug.len() <= 200) else { return StatusCode::NOT_FOUND.into_response() };
    let slug = slug.to_string();
    card(&state, &headers, format!("p:{slug}"), move |db| {
        Ok(catalog::queries::program_by_slug(db, &slug)?.map(|program| {
            let mut facts = vec![program.degree().to_string()];
            if let Some(variant) = &program.study_variant {
                facts.push(variant.label().to_string());
            }
            facts.push(match program.po_year {
                Some(year) => format!("Prüfungsordnung {year}"),
                None => format!("Prüfungsordnung {}", program.po_version),
            });
            let mut note = vec![format!("{} Module im Curriculum", app::format::count(program.curricular_modules.max(0) as u64))];
            if program.has_plan {
                note.push("mit Regelstudienplan".to_string());
            }
            CardText { eyebrow: "Studiengang".to_string(), title: program.name, facts, note: Some(note.join("  ·  ")) }
        }))
    })
    .await
}

/// A card: what it says is read from the snapshot, the picture is kept or drawn. When the server
/// has no free place to draw (or no snapshot yet), the site's standard picture answers instead,
/// not to be kept, so the next fetch gets the real one.
async fn card(state: &AppState, headers: &HeaderMap, key: String, read: impl FnOnce(&dyn catalog::Database) -> Result<Option<CardText>, catalog::DbError>) -> Response {
    let standard = || {
        let mut response = Response::new(Body::from(&include_bytes!("../../app/assets/og.png")[..]));
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
    match state.cards.get(&key, state.store.generation(), text).await {
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

/// `GET /manifest.webmanifest`: name, colours and icons of the site for a home screen.
pub async fn manifest(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "application/manifest+json", include_bytes!("../../app/assets/manifest.webmanifest"))
}

pub async fn font(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "font/woff2", include_bytes!("../../app/assets/inter-latin.woff2"))
}

pub async fn enhance_script(State(state): State<AppState>, headers: HeaderMap) -> Response {
    asset(&state, &headers, "text/javascript; charset=utf-8", include_bytes!("../../app/assets/enhance.js"))
}

/// `GET /assets/boot.js`, with the schema this build reads written into it
/// (`catalog::SCHEMA_VERSION`): it refuses a local copy of the catalog of an older one.
pub async fn boot_script(State(state): State<AppState>, headers: HeaderMap) -> Response {
    static SOURCE: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
    let body = SOURCE.get_or_init(|| {
        let source = include_str!("../../app/assets/boot.js").replace("__SCHEMA__", &catalog::SCHEMA_VERSION.to_string());
        Box::leak(source.into_boxed_str()).as_bytes()
    });
    asset(&state, &headers, "text/javascript; charset=utf-8", body)
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
    let use_gzip = accepts_gzip(&headers) && !compressed.is_empty();
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
    // In closed testing (`access`) there is nothing for a crawler but a login page.
    if state.gate.is_some() {
        return ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], "User-agent: *\nDisallow: /\n").into_response();
    }
    // A calendar feed is somebody's plan, not a page (its answer says `noindex` as well).
    let body = format!("User-agent: *\nAllow: /\nDisallow: /api/\nDisallow: {}\n\nSitemap: {}/sitemap.xml\n", subscription::CALENDAR_PREFIX, state.public_url);
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=86400")], body).into_response()
}
