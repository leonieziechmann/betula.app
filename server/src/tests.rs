//! The server against a fake Radix that serves a real snapshot over HTTP, the way the
//! real one does (`ETag`, `If-None-Match` → 304). Needs a snapshot like the tests of the
//! `catalog` crate (`FOLIA_TEST_SNAPSHOT`, else `snapshot/current.json`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use catalog::native::NativeDatabase;
use catalog::timetable::semester::SemesterKey;
use catalog::timetable::subscription::{self, Subscription};
use leptos::prelude::LeptosOptions;
use tower::ServiceExt;

use crate::cache::HtmlCache;
use crate::snapshot::{sync_once, SnapshotStore, Sync, SyncError};
use crate::AppState;

fn snapshot_file() -> PathBuf {
    if let Ok(path) = std::env::var("FOLIA_TEST_SNAPSHOT") {
        return PathBuf::from(path);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("snapshot");
    let pointer = std::fs::read_to_string(dir.join("current.json"))
        .unwrap_or_else(|e| panic!("no catalog snapshot for the tests ({e}); run `radix export` or set FOLIA_TEST_SNAPSHOT"));
    let pointer: serde_json::Value = serde_json::from_str(&pointer).unwrap();
    dir.join(pointer["file"].as_str().unwrap())
}

/// `content_digest` of the snapshot the Studienplan's checks were pinned to
/// (`catalog-41bcde83e1bbcaab.db` in the main checkout's `target/studyplan-snapshot/`, schema 9,
/// the catalog crate's `STUDYPLAN_DIGEST`): what the feed of a code holds event by event is
/// asserted on this one only.
const STUDYPLAN_DIGEST: &str = "8700613779415164c03d36c53966137e574a6e2b7ef2bf40542ad05c8f29b68b";

/// Informatik B.Sc. in WiSe 2026/27, the first semester of its plan: 11112, 12102, 12104 and
/// 12107 by Informatik's abbreviations, the Sachsendorf lecture 149408 hidden and „Nur diesen" on
/// the Übung 148369-a4d12. The catalog crate pins this code (`subscription.rs`); subscribed codes
/// never change.
const FIRST_SEMESTER_CODE: &str = "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0";

/// The address of `FIRST_SEMESTER_CODE` with its first character escaped (`b`, `%62`), as a
/// calendar service may write it.
const ESCAPED_PATH: &str = "/calendar/%623MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0.ics";

/// Codes `pack` writes with the kind `calendar` that no calendar reads, made once with
/// `pack::to_versioned_code("calendar", 1, …)` of a `Subscription` for 2026W (`Subscription::code`
/// refuses to write them): 61 modules (11100 to 11160), one more than a semester of a plan holds;
/// no module.
const TOO_MANY_MODULES_CODE: &str = "DbEGRK-jjZ5";
const NO_MODULE_CODE: &str = "JKXwB75";

/// The snapshot for the feed's checks and whether it is the pinned one. As in the catalog crate:
/// `FOLIA_STUDYPLAN_SNAPSHOT` names the pinned file and fails the test when it is another one;
/// without it the tests' own snapshot serves, and the event-level checks are skipped out loud
/// unless it happens to be the pinned one.
fn feed_snapshot(test: &str) -> (PathBuf, bool) {
    use std::io::Write;
    let digest = |file: &PathBuf| NativeDatabase::open(file).and_then(|db| catalog::queries::meta(&db)).unwrap().content_digest;
    if let Some(path) = std::env::var("FOLIA_STUDYPLAN_SNAPSHOT").ok().filter(|path| !path.is_empty()) {
        let file = PathBuf::from(&path);
        assert_eq!(digest(&file).as_deref(), Some(STUDYPLAN_DIGEST), "FOLIA_STUDYPLAN_SNAPSHOT={path} is not the snapshot the Studienplan's checks were pinned to");
        return (file, true);
    }
    let file = snapshot_file();
    let found = digest(&file);
    if found.as_deref() == Some(STUDYPLAN_DIGEST) {
        return (file, true);
    }
    // Straight to the handle: libtest swallows `eprintln!` of a test that passes.
    let _ = writeln!(
        std::io::stderr(),
        "studyplan: pinned checks of {test} skipped: snapshot digest {}, pinned {STUDYPLAN_DIGEST}; set FOLIA_STUDYPLAN_SNAPSHOT=…/target/studyplan-snapshot/catalog-41bcde83e1bbcaab.db",
        found.as_deref().unwrap_or("none")
    );
    (file, false)
}

/// A store with `file` active, the way a restart finds its snapshot.
fn store_with(name: &str, file: &PathBuf) -> Arc<SnapshotStore> {
    let dir = temp_dir(name);
    let store = SnapshotStore::new(dir.clone()).unwrap();
    std::fs::copy(file, dir.join("catalog-test.db")).unwrap();
    std::fs::write(dir.join("current.json"), r#"{"file":"catalog-test.db","etag":"\"test\""}"#).unwrap();
    assert!(store.restore());
    store
}

/// The text of a calendar with its folded lines joined again (RFC 5545 §3.1), so a check does not
/// depend on where a long line was folded.
fn unfolded(ics: &str) -> String {
    ics.replace("\r\n ", "")
}

/// What the log of this thread says, for the checks of the log.
#[derive(Clone, Default)]
struct Captured(Arc<std::sync::Mutex<Vec<u8>>>);

thread_local! {
    static CAPTURING: std::cell::RefCell<Option<Captured>> = const { std::cell::RefCell::new(None) };
}

/// Where a line of the tests' log goes: into the capture running on this thread, else nowhere.
struct ThisThread;

impl std::io::Write for ThisThread {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        CAPTURING.with(|capturing| {
            if let Some(captured) = capturing.borrow().as_ref() {
                captured.0.lock().unwrap().extend_from_slice(bytes);
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Ends a capture when dropped.
struct Capturing;

impl Drop for Capturing {
    fn drop(&mut self) {
        CAPTURING.with(|capturing| *capturing.borrow_mut() = None);
    }
}

impl Captured {
    /// Everything down to DEBUG that this thread logs, until the guard is dropped: a request sent
    /// with `request` runs its middleware and handler here. One subscriber for the whole process,
    /// set once: scoped subscribers of tests running side by side race on tracing's global cache
    /// of which levels are wanted, and lose lines.
    fn start(&self) -> Capturing {
        static LOGGING: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        LOGGING.get_or_init(|| {
            let subscriber = tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).with_ansi(false).with_writer(|| ThisThread).finish();
            tracing::subscriber::set_global_default(subscriber).unwrap();
        });
        CAPTURING.with(|capturing| *capturing.borrow_mut() = Some(self.clone()));
        Capturing
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

type Served = Arc<std::sync::Mutex<Option<(String, Vec<u8>)>>>;

/// What the fake Radix serves next: (ETag, body).
#[derive(Clone)]
struct FakeRadix {
    current: Served,
    downloads: Arc<AtomicUsize>,
}

async fn serve_snapshot(State(radix): State<FakeRadix>, headers: HeaderMap) -> Response {
    let Some((etag, body)) = radix.current.lock().unwrap().clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "5")]).into_response();
    };
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    radix.downloads.fetch_add(1, Ordering::SeqCst);
    ([(header::ETAG, etag)], body).into_response()
}

async fn start_radix() -> (FakeRadix, String) {
    let radix = FakeRadix { current: Arc::default(), downloads: Arc::default() };
    let app = Router::new().route("/snapshot/catalog.db", get(serve_snapshot)).with_state(radix.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/snapshot/catalog.db", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (radix, url)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("folia-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn state(store: Arc<SnapshotStore>) -> AppState {
    AppState {
        store,
        cache: Arc::new(HtmlCache::new(32 * 1024 * 1024)),
        cards: Arc::new(crate::cards::Cards::new(8 * 1024 * 1024, 1)),
        build_id: "test".into(),
        stale_after: None,
        public_url: "https://catalog.example".into(),
        site_root: "no-site".into(),
        packages: Arc::default(),
        gate: None,
        leptos: LeptosOptions::builder().output_name("folia-app").site_root("no-site").build(),
    }
}

async fn request(router: &Router, path: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().uri(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let (parts, body) = response.into_parts();
    let body = axum::body::to_bytes(body, 64 * 1024 * 1024).await.unwrap().to_vec();
    (parts.status, parts.headers, body)
}

/// A form sent to the server, the way the login page sends its own.
async fn post(router: &Router, path: &str, headers: &[(&str, &str)], form: &str) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::builder().method("POST").uri(path).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router.clone().oneshot(request.body(Body::from(form.to_string())).unwrap()).await.unwrap();
    let (parts, body) = response.into_parts();
    let body = axum::body::to_bytes(body, 1024 * 1024).await.unwrap().to_vec();
    (parts.status, parts.headers, String::from_utf8(body).unwrap())
}

/// Closed testing (`access`): nothing but the login page and what it needs, and a calendar
/// subscription with a valid code, answers without the password; with it the site is what it
/// was. Needs no snapshot.
#[tokio::test(flavor = "multi_thread")]
async fn closed_testing_asks_for_the_password_before_anything_else() {
    let gated = |name: &str| {
        let mut state = state(SnapshotStore::new(temp_dir(name)).unwrap());
        state.gate = Some(Arc::new(crate::access::Gate::new("birke im tagebau")));
        crate::router(state)
    };
    let router = gated("gate");
    let page = [("accept", "text/html,application/xhtml+xml,*/*;q=0.8")];

    // Whoever opens a page is led to the login page, which remembers where they wanted to go.
    // Everything else just hears "no", and nobody may keep either answer.
    let (status, headers, _) = request(&router, "/catalog?q=mathe&open=11101", &page).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!((headers[header::LOCATION].to_str().unwrap(), headers[header::CACHE_CONTROL].to_str().unwrap()), ("/access?next=%2Fcatalog%3Fq%3Dmathe%26open%3D11101", "no-store"));
    assert_eq!(request(&router, "/", &page).await.1[header::LOCATION], "/access");
    for path in ["/api/db", "/api/status", "/api/map.json", "/sitemap.xml", "/pkg/folia_client.js", "/assets/boot.js", "/assets/og.png", "/cards/module/11101.png", "/catalog", "/healthz/", "/no-such-page"] {
        let (status, headers, _) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::UNAUTHORIZED, "no-store"), "{path}");
    }
    assert_eq!(post(&router, "/api/db", &[], "").await.0, StatusCode::UNAUTHORIZED);

    // Open stays what the login page, a home screen and a supervisor need; crawlers are sent away
    // from everything but the calendar feeds (Google Calendar asks robots.txt before fetching one).
    for path in [app::STYLESHEET, "/assets/app.css?v=test", app::FONT, app::FAVICON, app::FAVICON_ICO, app::TOUCH_ICON, app::ICON_192, app::MANIFEST] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::OK, "{path}");
    }
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE, "answered by the health check (no snapshot here), not by the gate");
    // The container's own probe (`folia healthcheck`) has no password and needs no snapshot.
    let (status, headers, body) = request(&router, crate::api::LIVENESS, &[]).await;
    assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap(), body.as_slice()), (StatusCode::OK, "no-store", &b"ok\n"[..]));
    let (status, _, robots) = request(&router, "/robots.txt", &[]).await;
    assert_eq!((status, String::from_utf8(robots).unwrap().as_str()), (StatusCode::OK, "User-agent: *\nAllow: /calendar/\nDisallow: /\n"));
    // A calendar service has no password: a subscription whose code decodes passes, also with a
    // character of it escaped (here it meets no snapshot, so the feed itself answers 503; the
    // feed's own test serves one). Anything else under `/calendar/` stays behind the gate.
    for path in [subscription::path(FIRST_SEMESTER_CODE), ESCAPED_PATH.to_string()] {
        assert_eq!(request(&router, &path, &[]).await.0, StatusCode::SERVICE_UNAVAILABLE, "{path}");
    }
    for path in ["/calendar/abc".to_string(), "/calendar/Ab.ics.ics".to_string(), "/calendar/x.ics".to_string(), "/calendar/a/b.ics".to_string(), subscription::path(TOO_MANY_MODULES_CODE)] {
        let (status, headers, _) = request(&router, &path, &[("accept", "*/*")]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::UNAUTHORIZED, "no-store"), "{path}");
    }

    // The login page: a form that works without JavaScript, carries the way back as text (never
    // as markup) and is nothing a search engine or a cache may keep.
    let (status, headers, body) = request(&router, "/access?next=%2Fcatalog%3Fq%3D%22%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E", &page).await;
    let html = String::from_utf8(body).unwrap();
    assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap(), headers["x-robots-tag"].to_str().unwrap()), (StatusCode::OK, "no-store", "noindex, nofollow"));
    assert!(html.starts_with("<!DOCTYPE html>") && html.contains("<form method=\"post\" action=\"/access\">") && html.contains("type=\"password\""), "{html}");
    assert!(html.contains("name=\"next\"") && !html.contains("<script>alert") && !html.contains("role=\"alert\""), "{html}");
    assert!(html.split("</head>").next().unwrap().contains(&format!("<style>{}</style>", app::VIEW_TRANSITION_STYLE)), "the login page fades like the site: {html}");
    assert!(html.contains("rel=\"stylesheet\" href=\"/assets/app.css?v=test\""), "the stylesheet of this build: {html}");

    // A wrong password stays on the form and says so; the right one opens the gate for this
    // browser and leads to where the visitor wanted to go, and nowhere outside the site.
    let (status, headers, html) = post(&router, "/access", &[], "password=birke&next=%2Fprograms").await;
    assert_eq!((status, headers.get(header::SET_COOKIE)), (StatusCode::UNAUTHORIZED, None));
    assert!(html.contains("Das Passwort stimmt nicht.") && html.contains("value=\"/programs\""), "{html}");
    let (status, headers, _) = post(&router, "/access", &[], "password=+birke+im+tagebau%0A&next=%2Fprograms%3Fq%3Dinfo").await;
    assert_eq!((status, headers[header::LOCATION].to_str().unwrap()), (StatusCode::SEE_OTHER, "/programs?q=info"));
    let cookie = headers[header::SET_COOKIE].to_str().unwrap().to_string();
    assert!(cookie.starts_with("betula_access=v1.") && cookie.ends_with("; Path=/; Max-Age=7776000; HttpOnly; SameSite=Lax"), "{cookie}");
    let (_, headers, _) = post(&router, "/access", &[("x-forwarded-proto", "https")], "password=birke+im+tagebau&next=%2F%2Fevil.example%2F").await;
    assert_eq!(headers[header::LOCATION], "/");
    assert!(headers[header::SET_COOKIE].to_str().unwrap().ends_with("; SameSite=Lax; Secure"), "behind the proxy the cookie travels over HTTPS only");

    // With the cookie the site is what it was, only private to this browser. A made-up or
    // altered cookie is no cookie.
    let visit = cookie.split(';').next().unwrap().to_string();
    let with_cookie = [("cookie", visit.as_str()), ("accept", "text/html")];
    assert_eq!(request(&router, "/api/status", &with_cookie).await.0, StatusCode::OK);
    assert_eq!(request(&router, "/catalog", &with_cookie).await.0, StatusCode::SERVICE_UNAVAILABLE, "the page itself answers (no snapshot here)");
    let (status, headers, _) = request(&router, "/assets/boot.js", &with_cookie).await;
    assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::OK, "private, no-cache"));
    assert_eq!(request(&router, app::STYLESHEET, &with_cookie).await.1[header::CACHE_CONTROL], "public, no-cache", "what is open anyway stays shared");
    let (status, headers, _) = request(&router, "/access?next=%2Fcatalog", &with_cookie).await;
    assert_eq!((status, headers[header::LOCATION].to_str().unwrap()), (StatusCode::SEE_OTHER, "/catalog"));
    let forged = format!("{}{}", &visit[..visit.len() - 1], if visit.ends_with('0') { '1' } else { '0' });
    for cookie in [forged.as_str(), "betula_access=v1.99999999999.00", "betula_access=", "other=1"] {
        assert_eq!(request(&router, "/api/status", &[("cookie", cookie)]).await.0, StatusCode::UNAUTHORIZED, "{cookie}");
    }

    // Ten wrong passwords close the form for the rest of the minute, for the right one too.
    let router = gated("gate-closed");
    for _ in 0..10 {
        assert_eq!(post(&router, "/access", &[], "password=geraten").await.0, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, html) = post(&router, "/access", &[], "password=birke+im+tagebau").await;
    assert_eq!((status, headers.get(header::SET_COOKIE)), (StatusCode::TOO_MANY_REQUESTS, None));
    assert!(headers.contains_key(header::RETRY_AFTER) && html.contains("Zu viele falsche Versuche"), "{html}");
    assert_eq!(request(&router, "/api/status", &with_cookie).await.0, StatusCode::OK, "who is in stays in");

    // Without the gate the login page has nothing to ask and leads on.
    let open = crate::router(state(SnapshotStore::new(temp_dir("gate-off")).unwrap()));
    let (status, headers, _) = request(&open, "/access?next=%2Fprograms", &page).await;
    assert_eq!((status, headers[header::LOCATION].to_str().unwrap()), (StatusCode::SEE_OTHER, "/programs"));
    assert_eq!(request(&open, "/api/status", &[]).await.0, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshots_come_over_http_and_bad_ones_are_rejected() {
    let real = std::fs::read(snapshot_file()).unwrap();
    let (radix, url) = start_radix().await;
    let data_dir = temp_dir("sync");
    let store = SnapshotStore::new(data_dir.clone()).unwrap();
    let client = reqwest::Client::new();
    let router = crate::router(state(store.clone()));

    // Radix has not exported anything yet: the site says so, and nothing is cached.
    assert!(matches!(sync_once(&store, &client, &url).await, Err(SyncError::NotReady)));
    let (status, headers, _) = request(&router, "/catalog", &[]).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(request(&router, "/api/db", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE);

    // First export: downloaded, checked, compressed, active.
    *radix.current.lock().unwrap() = Some(("\"aaaa1111\"".to_string(), real.clone()));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let active = store.current().expect("active snapshot");
    assert_eq!((active.etag.as_str(), active.bytes), ("\"aaaa1111\"", real.len() as u64));
    assert!(active.gzip.as_ref().is_some_and(|(_, bytes)| *bytes < active.bytes / 3), "the browser download is compressed once");
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::OK);

    // Nothing new: a conditional request, no download.
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Unchanged)));
    assert_eq!(radix.downloads.load(Ordering::SeqCst), 1);

    // Pages render from the snapshot, are cached, and revalidate without rendering.
    let (status, headers, body) = request(&router, "/catalog?form=exercise&turnus=winter&status=all", &[]).await;
    let html = String::from_utf8(body).unwrap();
    assert_eq!((status, headers["x-cache"].to_str().unwrap()), (StatusCode::OK, "miss"));
    assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache", "a page of the old build must not outlive a deploy in the browser");
    let expected = {
        let db = catalog::native::NativeDatabase::open(&snapshot_file()).unwrap();
        let url = catalog::url::CatalogUrl::parse("form=exercise&turnus=winter&status=all");
        catalog::queries::catalog_count(&db, &url.query).unwrap()
    };
    assert!(expected > 100);
    assert!(html.replace("<!>", "").contains(&format!("class=\"count num\">{}</span>", app::format::count(expected))), "the header shows the exact total {expected}");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    // The same filter written differently is the same page.
    let (_, headers, _) = request(&router, "/catalog?status=all&turnus=winter&form=exercise&q=", &[]).await;
    assert_eq!(headers["x-cache"], "hit");
    assert_eq!(request(&router, "/catalog?turnus=winter&form=exercise&status=all", &[("if-none-match", &etag)]).await.0, StatusCode::NOT_MODIFIED);
    let (_, headers, body) = request(&router, "/programs", &[("accept-encoding", "gzip, br")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    assert_eq!(body[..2], [0x1f, 0x8b]);

    // What search engines read: one description and one address per page, absolute, with the name
    // the site has from outside; views of the lists are not listed; the sitemap names every page.
    let head = |html: &str| html.split("</head>").next().unwrap_or_default().to_string();
    let (_, _, body) = request(&router, "/catalog/module/11101", &[]).await;
    let module = head(&String::from_utf8(body).unwrap());
    assert_eq!(module.matches("name=\"description\"").count(), 1, "{module}");
    assert!(module.contains("href=\"https://catalog.example/catalog/module/11101\" rel=\"canonical\""), "{module}");
    assert!(module.contains("application/ld+json") && module.contains("\"@type\":\"Course\"") && !module.contains("noindex"), "{module}");
    assert!(head(&html).contains("content=\"noindex, follow\""), "a filtered list is a view of /catalog");
    // The placeholder a list looks for (`fill`) is the app's, and the cache keeps the page under
    // the address without it: the server's page must be that address's page, or the first
    // `?fill=` would make the one listed catalog page unlisted for everybody.
    let (_, headers, body) = request(&router, "/catalog?fill=p3", &[]).await;
    let filled = head(&String::from_utf8(body).unwrap());
    assert_eq!(headers["x-cache"], "miss");
    assert!(filled.contains("href=\"https://catalog.example/catalog\" rel=\"canonical\"") && !filled.contains("noindex"), "{filled}");
    let (_, headers, body) = request(&router, "/catalog", &[]).await;
    assert_eq!((headers["x-cache"].to_str().unwrap(), head(&String::from_utf8(body).unwrap())), ("hit", filled));
    let (_, _, body) = request(&router, "/catalog?turnus=winter&fill=p9", &[]).await;
    let filtered = head(&String::from_utf8(body).unwrap());
    assert!(filtered.contains("href=\"https://catalog.example/catalog?turnus=winter\" rel=\"canonical\"") && !filtered.contains("fill="), "{filtered}");
    let (_, _, body) = request(&router, "/", &[]).await;
    let home = String::from_utf8(body).unwrap();
    assert!(head(&home).contains("href=\"https://catalog.example/\" rel=\"canonical\"") && !head(&home).contains("noindex"));
    assert!(home.contains("class=\"map map-wide\"") && home.contains("class=\"map map-tall\""), "the landing page draws the map the snapshot was opened with");
    // Impressum and Datenschutz: linked from the start page, and while they are placeholders they
    // say so and are not indexed (deploy/ship.sh keeps them off an instance open to everybody).
    for path in [catalog::url::IMPRINT, catalog::url::PRIVACY] {
        assert!(home.contains(&format!("href=\"{path}\"")), "the start page links {path}");
        let (status, _, body) = request(&router, path, &[]).await;
        let page = String::from_utf8(body).unwrap();
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(page.contains("Platzhalter"), app::pages::legal::PLACEHOLDER, "{path}");
        assert_eq!(head(&page).contains("noindex"), app::pages::legal::PLACEHOLDER, "{path}");
    }

    // What a link preview and a home screen read: the card of the page with an absolute picture,
    // and the icons and the manifest of the site, each served as what it is.
    for tag in ["property=\"og:title\"", "property=\"og:description\"", "name=\"twitter:card\"", "name=\"twitter:image\"", "property=\"og:image:alt\""] {
        assert_eq!(module.matches(tag).count(), 1, "{tag} in {module}");
    }
    for link in ["rel=\"manifest\"", "rel=\"apple-touch-icon\"", "href=\"/favicon.ico\"", "name=\"theme-color\"", "rel=\"stylesheet\"", "rel=\"preload\""] {
        assert_eq!(head(&home).matches(link).count(), 1, "{link}");
    }
    // The fade between pages is opted into in the head itself: from the stylesheet alone the
    // browser may learn of it too late (`app::VIEW_TRANSITION_STYLE`).
    assert_eq!(head(&home).matches(&format!("<style>{}</style>", app::VIEW_TRANSITION_STYLE)).count(), 1, "{home}");
    // The stylesheet and the scripts are linked with the build that wrote the page, so that a
    // service worker of another build never answers them from its cache; the address still
    // leads to the file, whatever build it names.
    for link in ["rel=\"stylesheet\" href=\"/assets/app.css?v=test\"", "src=\"/assets/enhance.js?v=test\"", "src=\"/assets/boot.js?v=test\""] {
        assert_eq!(head(&home).matches(link).count(), 1, "{link} in {home}");
    }
    for path in ["/assets/app.css?v=test", "/assets/app.css?v=an-older-build", "/assets/boot.js?v=test", "/assets/sql-wasm.wasm?v=test"] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::OK, "{path}");
    }
    // The worker knows the build too: it keeps the files under the addresses this build links.
    let (_, _, worker) = request(&router, app::SERVICE_WORKER, &[]).await;
    let worker = String::from_utf8(worker).unwrap();
    assert!(worker.contains("const VERSION = \"test\";") && !worker.contains("__BUILD__"), "{worker}");
    // The boot knows the schema its build reads, and opens no local copy of an older one.
    let (_, _, boot) = request(&router, "/assets/boot.js?v=test", &[]).await;
    let boot = String::from_utf8(boot).unwrap();
    assert!(boot.contains(&format!("const SCHEMA = Number(\"{}\");", catalog::SCHEMA_VERSION)) && !boot.contains("__SCHEMA__"), "{boot}");
    // Every answer names the build that gave it; the worker keeps only the answers of its own.
    for path in ["/", "/catalog", "/assets/app.css?v=test", app::SERVICE_WORKER, "/manifest.webmanifest"] {
        let (_, headers, _) = request(&router, path, &[]).await;
        assert_eq!(headers.get(crate::BUILD_HEADER).and_then(|value| value.to_str().ok()), Some("test"), "{path}");
    }
    for (path, content_type, magic) in [
        ("/assets/og.png", "image/png", &b"\x89PNG"[..]),
        ("/apple-touch-icon.png", "image/png", &b"\x89PNG"[..]),
        ("/apple-touch-icon-precomposed.png", "image/png", &b"\x89PNG"[..]),
        ("/assets/icon-192.png", "image/png", &b"\x89PNG"[..]),
        ("/assets/icon-512.png", "image/png", &b"\x89PNG"[..]),
        ("/assets/icon-maskable-512.png", "image/png", &b"\x89PNG"[..]),
        ("/favicon.ico", "image/x-icon", &[0, 0, 1, 0][..]),
    ] {
        let (status, headers, body) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, content_type), "{path}");
        assert!(body.starts_with(magic), "{path}");
    }
    // The birch: every mask the stylesheet names is served (a name it misses would leave a hole
    // in the crown or the ground without any error).
    let stylesheet = include_str!("../../app/assets/app.css");
    let masks: std::collections::BTreeSet<&str> = stylesheet.split("url(\"").skip(1).filter_map(|rest| rest.split('"').next()).filter(|url| url.starts_with("/assets/birch/")).collect();
    assert!(masks.len() >= 12, "{masks:?}");
    for path in masks {
        let (status, headers, body) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/svg+xml"), "{path}");
        assert!(body.starts_with(b"<svg"), "{path}");
    }
    assert_eq!(request(&router, "/assets/birch/no-such-season.svg", &[]).await.0, StatusCode::NOT_FOUND);
    // A module and a program have their own picture: named in the head with the site's outside
    // address, drawn on the first request, kept after that, and answered with 304 to its ETag.
    assert!(module.contains("content=\"https://catalog.example/cards/module/11101.png\"") && !module.contains("/assets/og.png"), "{module}");
    assert!(head(&home).contains("content=\"https://catalog.example/assets/og.png\""), "the landing page keeps the standard picture");
    let (status, headers, card) = request(&router, "/cards/module/11101.png", &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"));
    assert!(card.starts_with(b"\x89PNG") && card[16..24] == [0, 0, 4, 176, 0, 0, 2, 118], "1200 x 630");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    let (status, again, _) = request(&router, "/cards/module/11101.png", &[("if-none-match", &etag)]).await;
    assert_eq!((status, again[header::ETAG].to_str().unwrap()), (StatusCode::NOT_MODIFIED, etag.as_str()));
    let program = String::from_utf8(request(&router, "/programs", &[]).await.2).unwrap();
    let slug = program.split("href=\"/programs/").nth(1).and_then(|rest| rest.split(['/', '"', '?']).next()).unwrap().to_string();
    let (status, headers, card) = request(&router, &format!("/cards/program/{slug}.png"), &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"), "{slug}");
    assert!(card.starts_with(b"\x89PNG"));
    for missing in ["/cards/module/00000.png", "/cards/module/11101", "/cards/program/no-such-program.png"] {
        assert_eq!(request(&router, missing, &[]).await.0, StatusCode::NOT_FOUND, "{missing}");
    }

    let (status, headers, body) = request(&router, "/manifest.webmanifest", &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "application/manifest+json"));
    let manifest: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(manifest["short_name"], "Betula");
    for icon in manifest["icons"].as_array().unwrap() {
        let (status, _, _) = request(&router, icon["src"].as_str().unwrap(), &[]).await;
        assert_eq!(status, StatusCode::OK, "the manifest names an icon that is not served: {icon}");
    }

    let (status, headers, body) = request(&router, "/sitemap.xml", &[]).await;
    let sitemap = String::from_utf8(body).unwrap();
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "application/xml; charset=utf-8"));
    assert!(sitemap.contains("<loc>https://catalog.example/</loc>") && sitemap.contains("<loc>https://catalog.example/catalog/module/11101</loc>"));
    assert!(sitemap.matches("<loc>").count() > 3000 && sitemap.lines().all(|line| !line.starts_with("<url>") || !line.contains('?')), "pages only, no filters");
    // „Mein Plan" is the visitor's (a placeholder so far): not listed, not indexed.
    assert!(sitemap.contains("/areas</loc>") && !sitemap.contains("/my-plan</loc>"));
    let (status, _, body) = request(&router, &format!("/programs/{slug}/my-plan"), &[]).await;
    let mine = String::from_utf8(body).unwrap();
    assert!(status == StatusCode::OK && head(&mine).contains("noindex") && mine.contains(&format!("href=\"/catalog?program={slug}\"")), "{mine}");
    let (_, _, robots) = request(&router, "/robots.txt", &[]).await;
    assert!(String::from_utf8(robots).unwrap().contains("Sitemap: https://catalog.example/sitemap.xml"));

    // The map of the programs is laid out once per snapshot and handed on as it is. Its ETag is
    // its content's: a new layout of the same catalog must not be a „304" from a browser's cache.
    let (status, headers, body) = request(&router, "/api/map.json", &[]).await;
    let map_etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(status, StatusCode::OK);
    assert!(map_etag.starts_with("\"map-") && map_etag != "\"aaaa1111\"", "{map_etag}");
    let map: catalog::graph::ProgramMap = serde_json::from_slice(&body).unwrap();
    assert_eq!(Some(&map), active.program_map.as_ref().map(|(map, ..)| map.as_ref()));
    assert!(map.programs.len() > 100 && map.wide.dots.len() == map.programs.len() && map.tall.dots.len() == map.programs.len());
    assert_eq!(request(&router, "/api/map.json", &[("if-none-match", map_etag.as_str())]).await.0, StatusCode::NOT_MODIFIED);
    assert_eq!(request(&router, "/api/map.json", &[("if-none-match", "\"aaaa1111\"")]).await.0, StatusCode::OK);
    assert_eq!(request(&router, app::OG_IMAGE, &[]).await.0, StatusCode::OK);

    // Unknown things are 404 and never cached.
    for path in ["/catalog/module/00000", "/programs/no-such-program", "/no-such-page"] {
        let (status, headers, _) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::NOT_FOUND, "no-store"), "{path}");
    }

    // Browsers get the file with Radix's ETag, compressed, and a 304 when they have it.
    let (status, headers, body) = request(&router, "/api/db", &[("accept-encoding", "gzip")]).await;
    assert_eq!((status, headers[header::ETAG].to_str().unwrap()), (StatusCode::OK, "\"aaaa1111\""));
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    assert_eq!(body.len() as u64, active.gzip.as_ref().unwrap().1);
    let (status, _, body) = request(&router, "/api/db", &[]).await;
    assert_eq!((status, body.len()), (StatusCode::OK, real.len()));
    assert_eq!(request(&router, "/api/db", &[("if-none-match", "\"aaaa1111\"")]).await.0, StatusCode::NOT_MODIFIED);
    let (_, _, status_body) = request(&router, "/api/status", &[]).await;
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["snapshot"]["etag"], "\"aaaa1111\"");
    assert!(status_json["html_cache"]["pages"].as_u64().unwrap() >= 2);
    // And its schema, which a browser compares with the one its build reads before it fetches it.
    let schema = catalog::native::NativeDatabase::open(&snapshot_file()).unwrap().schema_version().unwrap();
    assert_eq!((status_json["snapshot"]["schema_version"].as_i64(), active.schema_version), (Some(schema), schema));

    // A broken export must not replace a good snapshot.
    *radix.current.lock().unwrap() = Some(("\"bbbb2222\"".to_string(), b"this is not a database".to_vec()));
    assert!(matches!(sync_once(&store, &client, &url).await, Err(SyncError::Rejected(_))));
    assert_eq!(store.current().unwrap().etag, "\"aaaa1111\"");
    assert_eq!(request(&router, "/catalog", &[]).await.0, StatusCode::OK);

    // An export of an older schema than this build reads is served all the same: the pages that
    // do not need the newer columns work, and a refused one would leave the server without data
    // after a restart where Radix does not export again. The status names its schema, so that no
    // browser fetches it for the app.
    let mut older = real.clone();
    older[60..64].copy_from_slice(&((catalog::SCHEMA_VERSION - 1) as u32).to_be_bytes());
    *radix.current.lock().unwrap() = Some(("\"dddd4444\"".to_string(), older));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let (_, _, status_body) = request(&router, "/api/status", &[]).await;
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!((status_json["snapshot"]["etag"].as_str(), status_json["snapshot"]["schema_version"].as_i64()), (Some("\"dddd4444\""), Some(catalog::SCHEMA_VERSION - 1)));
    assert_eq!(request(&router, "/catalog", &[]).await.0, StatusCode::OK);

    // A new good export starts a new generation: old pages are gone, old ETags no longer match.
    *radix.current.lock().unwrap() = Some(("\"cccc3333\"".to_string(), real.clone()));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let (status, headers, _) = request(&router, "/catalog?turnus=winter&form=exercise&status=all", &[("if-none-match", &etag)]).await;
    assert_eq!((status, headers["x-cache"].to_str().unwrap()), (StatusCode::OK, "miss"));

    // After a restart the last good snapshot is served even if Radix is down.
    drop(router);
    let restarted = SnapshotStore::new(data_dir.clone()).unwrap();
    assert!(restarted.restore());
    assert_eq!(restarted.current().unwrap().etag, "\"cccc3333\"");
    let leftovers: Vec<String> = std::fs::read_dir(&data_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(leftovers.iter().all(|name| !name.starts_with("download-")), "{leftovers:?}");
}

/// A Studienplan as a calendar subscription (`GET /calendar/<code>.ics`): the loader's calendar of
/// the code, made anew from the snapshot, revalidated by its content, compressed on request,
/// reachable in closed testing without a cookie, and in the log by its size, never by its code.
#[tokio::test(flavor = "multi_thread")]
async fn a_studyplan_is_a_calendar_feed() {
    use std::io::Read;
    let (file, pinned) = feed_snapshot("a_studyplan_is_a_calendar_feed");
    let store = store_with("feed", &file);
    let router = crate::router(state(store.clone()));
    let log = Captured::default();
    let logging = log.start();
    let path = subscription::path(FIRST_SEMESTER_CODE);

    let (status, headers, body) = request(&router, &path, &[]).await;
    let ics = String::from_utf8(body).unwrap();
    assert_eq!(status, StatusCode::OK, "{ics}");
    let named = |name: &str| headers.get(name).and_then(|value| value.to_str().ok()).unwrap_or_default().to_string();
    assert_eq!(named("content-type"), "text/calendar; charset=utf-8");
    assert_eq!(named("cache-control"), "private, max-age=900", "one person's plan: no shared cache keeps it");
    assert_eq!((named("vary"), named("x-robots-tag")), ("Accept-Encoding".to_string(), "noindex, nofollow".to_string()));
    assert_eq!(named("content-disposition"), "inline; filename=\"studienplan-2026W.ics\"");
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n") && ics.ends_with("END:VCALENDAR\r\n"), "{ics}");
    // The feed is the loader's calendar of the code, byte for byte: the text the page offers as a
    // download is made by the same function from the same rows.
    let db = NativeDatabase::open(&file).unwrap();
    assert_eq!(ics, catalog::pages::calendar(&db, &Subscription::from_code(FIRST_SEMESTER_CODE).unwrap()).unwrap());
    if pinned {
        let text = unfolded(&ics);
        assert!(text.contains("UID:148701-a2633-20261013@betula.app") && text.contains("UID:148369-a4d12-") && text.contains("Entwicklung von Softwaresystemen"), "{text}");
        // As short as the page's week, with Informatik's abbreviations; in full in the description.
        assert!(text.contains("SUMMARY:VL EvS\r\nLOCATION:ZHG/HS.C\r\nDESCRIPTION:Vorlesung\\nModul 12104 Entwicklung von Softwaresystemen\\n"), "{text}");
        assert!(text.contains("\\nRaum: Zentrales Hörsaalgebäude - Hörsaal C - Zentralcampus\\n"), "{text}");
        // Not 12104's Senftenberg track (the plan's other modules make it Cottbus), not the hidden
        // Sachsendorf lecture, not the Übungen beside the chosen one.
        for absent in ["UID:149406-", "UID:149408-", "UID:148369-aaf38-"] {
            assert!(!text.contains(absent), "{absent}");
        }
        assert_eq!(text.matches("BEGIN:VEVENT").count(), 249);
    }

    // An unchanged plan in an unchanged snapshot has the same tag: a calendar that asks again
    // hears „304".
    let etag = named("etag");
    assert!(etag.starts_with("\"ics-"), "{etag}");
    let (status, again, body) = request(&router, &path, &[("if-none-match", &etag)]).await;
    assert_eq!((status, again[header::ETAG].to_str().unwrap(), again[header::CACHE_CONTROL].to_str().unwrap(), body.len()), (StatusCode::NOT_MODIFIED, etag.as_str(), "private, max-age=900", 0));
    // Compressed here on request (the edge's compression does not take `text/calendar`).
    let (status, zipped, body) = request(&router, &path, &[("accept-encoding", "gzip, br")]).await;
    assert_eq!((status, zipped[header::CONTENT_ENCODING].to_str().unwrap()), (StatusCode::OK, "gzip"));
    let mut unzipped = String::new();
    flate2::read::GzDecoder::new(body.as_slice()).read_to_string(&mut unzipped).unwrap();
    assert_eq!(unzipped, ics);
    // A calendar that escapes a character of the code asks for the same feed.
    let escaped = ESCAPED_PATH.to_string();
    assert_eq!(request(&router, &escaped, &[]).await.2, ics.as_bytes());

    // A semester without Termine in the snapshot yet: a calendar without entries that says so,
    // which fills by itself once QIS publishes them.
    let later = Subscription { semester: SemesterKey::parse("2027S").unwrap().index(), modules: vec![12104], ..Subscription::default() }.code().unwrap();
    let (status, headers, body) = request(&router, &subscription::path(&later), &[]).await;
    let empty = unfolded(&String::from_utf8(body).unwrap());
    assert_eq!((status, headers[header::CONTENT_DISPOSITION].to_str().unwrap()), (StatusCode::OK, "inline; filename=\"studienplan-2027S.ics\""));
    assert!(empty.starts_with("BEGIN:VCALENDAR\r\n"), "{empty}");
    if pinned {
        assert!(!empty.contains("BEGIN:VEVENT") && empty.contains("Noch keine Termine veröffentlicht"), "{empty}");
    }

    // In closed testing the calendar service has no password: the feed passes the gate without a
    // cookie and stays what it is, private.
    let mut closed = state(store);
    closed.gate = Some(Arc::new(crate::access::Gate::new("birke im tagebau")));
    let closed = crate::router(closed);
    for path in [path.as_str(), escaped.as_str()] {
        let (status, headers, body) = request(&closed, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap(), body.as_slice()), (StatusCode::OK, "private, max-age=900", ics.as_bytes()), "{path}");
    }

    // The log says that feeds were made, how large and how fast, and never which.
    drop(logging);
    let log = log.text();
    // Seven feeds were made above: the 304 is made too, since its tag is the content's.
    assert_eq!(log.matches("calendar.served").count(), 7, "{log}");
    assert!(log.contains("/calendar/….ics"), "{log}");
    for code in [FIRST_SEMESTER_CODE, &FIRST_SEMESTER_CODE[1..], later.as_str()] {
        assert!(!log.contains(code), "{code} in {log}");
    }
}

/// Every address under `/calendar/` that is not a calendar's is a plain 404 that nothing keeps:
/// never a 5xx, which the log would report as an error a human has to act on.
#[tokio::test(flavor = "multi_thread")]
async fn broken_calendar_codes_are_404() {
    let router = crate::router(state(SnapshotStore::new(temp_dir("calendar-404")).unwrap()));
    // The Merkliste's code of the same kind of list: its kind is part of the check characters.
    let bookmarks = app::bookmarks::transfer_fragment(&["11112".to_string(), "12104".to_string()]).unwrap();
    let bookmarks = bookmarks.strip_prefix("m=").unwrap();
    // These have the shape of a feed's address; what they carry is what no calendar reads.
    for code in [bookmarks, TOO_MANY_MODULES_CODE, NO_MODULE_CODE, "Ab.ics"] {
        assert!(subscription::code_of_path(&subscription::path(code)).is_some(), "{code}");
    }
    let paths = [
        "/calendar/x.ics".to_string(),
        "/calendar/Ab.ics.ics".to_string(),
        subscription::path(bookmarks),
        format!("/calendar/{}.ics", "A".repeat(subscription::MAX_CODE + 1)),
        subscription::path(TOO_MANY_MODULES_CODE),
        subscription::path(NO_MODULE_CODE),
        "/calendar/%3Cx%3E.ics".to_string(),
        "/calendar/%FF.ics".to_string(),
        "/calendar/abc".to_string(),
        format!("/calendar/{FIRST_SEMESTER_CODE}"),
    ];
    for path in &paths {
        let (status, headers, _) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::NOT_FOUND, "no-store"), "{path}");
    }
    // The address is checked first: a good code meets no snapshot here and hears so.
    assert_eq!(request(&router, &subscription::path(FIRST_SEMESTER_CODE), &[]).await.0, StatusCode::SERVICE_UNAVAILABLE);
}

/// Folia's own log, kept 30 days in Loki, writes every path under `/calendar/` as one fixed text,
/// whatever the answer and whoever gave it (the feed, the gate, the pages' fallback).
#[tokio::test(flavor = "multi_thread")]
async fn the_log_keeps_no_calendar_code() {
    assert_eq!(subscription::redacted_path(&subscription::path(FIRST_SEMESTER_CODE)), "/calendar/….ics");
    for other in ["/calendar", "/calendarx/a.ics", "/catalog/module/12104", "/"] {
        assert_eq!(subscription::redacted_path(other), other);
    }

    let open = crate::router(state(SnapshotStore::new(temp_dir("log-open")).unwrap()));
    let mut gated = state(SnapshotStore::new(temp_dir("log-gated")).unwrap());
    gated.gate = Some(Arc::new(crate::access::Gate::new("birke im tagebau")));
    let gated = crate::router(gated);
    let asked = [subscription::path(FIRST_SEMESTER_CODE), ESCAPED_PATH.to_string(), "/calendar/Ab.ics.ics".to_string(), "/calendar/secret-a/b.ics".to_string()];
    let log = Captured::default();
    let logging = log.start();
    for router in [&open, &gated] {
        for path in &asked {
            request(router, path, &[]).await;
        }
    }
    drop(logging);
    let log = log.text();
    assert_eq!(log.matches("http.request").count(), 2 * asked.len(), "one line per request: {log}");
    assert_eq!(log.matches("/calendar/….ics").count(), 2 * asked.len(), "{log}");
    for secret in [&FIRST_SEMESTER_CODE[1..], "Ab.ics", "secret", "%62"] {
        assert!(!log.contains(secret), "{secret} in {log}");
    }
}

/// Google Calendar reads robots.txt before it fetches a subscription, so neither answer of
/// robots.txt may disallow a feed (the feed keeps itself out of indexes with `X-Robots-Tag:
/// noindex`). The service worker never keeps one (in Cache Storage, or as the shell offline).
#[tokio::test(flavor = "multi_thread")]
async fn calendar_services_may_fetch_feeds() {
    let mut gated = state(SnapshotStore::new(temp_dir("robots-gated")).unwrap());
    gated.gate = Some(Arc::new(crate::access::Gate::new("birke im tagebau")));
    let (status, _, robots) = request(&crate::router(gated), "/robots.txt", &[]).await;
    let robots = String::from_utf8(robots).unwrap();
    let (allow, disallow) = (robots.find("\nAllow: /calendar/\n"), robots.find("\nDisallow: /\n"));
    assert!(status == StatusCode::OK && allow.is_some() && disallow.is_some() && allow < disallow, "{robots}");

    let router = crate::router(state(SnapshotStore::new(temp_dir("robots")).unwrap()));
    let (status, _, robots) = request(&router, "/robots.txt", &[]).await;
    let robots = String::from_utf8(robots).unwrap();
    assert!(status == StatusCode::OK && !robots.contains("calendar") && robots.contains("\nDisallow: /api/\n"), "{robots}");
    let (_, _, worker) = request(&router, app::SERVICE_WORKER, &[]).await;
    assert!(String::from_utf8(worker).unwrap().contains("const NEVER = /^\\/(api\\/|access|sw\\.js$|cards\\/|calendar\\/)/;"));
}
