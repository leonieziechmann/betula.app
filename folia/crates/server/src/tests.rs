//! The server against a fake Radix that serves a real snapshot over HTTP, the way the
//! real one does (`ETag`, `If-None-Match` → 304). Needs a snapshot like the tests of the
//! domain crates (`folia-test-support`: `FOLIA_TEST_SNAPSHOT`, else `snapshot/current.json`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum::routing::get;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::{self, SharedPlan};
use folia_calendar::subscription::{self, Subscription};
use folia_model::native::NativeDatabase;
use leptos::prelude::LeptosOptions;
use tower::ServiceExt;

use crate::AppState;
use crate::cache::HtmlCache;
use crate::snapshot::{sync_once, SnapshotStore, Sync, SyncError};

fn snapshot_file() -> PathBuf {
    if let Ok(path) = std::env::var("FOLIA_TEST_SNAPSHOT") {
        return PathBuf::from(path);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../snapshot");
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
/// `folia_pack::to_versioned_code("calendar", 1, …)` of a `Subscription` for 2026W (`Subscription::code`
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
    let digest = |file: &PathBuf| NativeDatabase::open(file).and_then(|db| folia_query::meta(&db)).unwrap().content_digest;
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
        launch: Arc::default(),
        build_id: "test".into(),
        stale_after: None,
        public_url: "https://catalog.example".into(),
        site_root: "no-site".into(),
        live_assets: None,
        packages: Arc::default(),
        semantic: None,
        gate: None,
        renders: Arc::new(crate::busy::Places::new("render", 2, std::time::Duration::from_secs(3))),
        render_wait: std::time::Duration::from_secs(3),
        feeds: Arc::new(crate::busy::Places::new("calendar", 2, std::time::Duration::from_secs(10))),
        changes: None,
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

/// The launch screens of iOS (`folia_shell::launch`): the head script of every page names those of its
/// screen, and the server draws each one it can name, as large as its screen, and answers it again
/// with 304 to its ETag; no other name is there. Needs no snapshot.
/// The semantic search's model (`semantic`): served under the address its content names, kept
/// for good, named to the browser by `boot.js` and `/api/status`; nothing under another name, and
/// nothing at all, and `null` in both, without a model.
#[tokio::test(flavor = "multi_thread")]
async fn the_semantic_model_is_served_under_its_hash() {
    let without = crate::router(state(SnapshotStore::new(temp_dir("semantic-none")).unwrap()));
    let (_, _, status) = request(&without, "/api/status", &[]).await;
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&status).unwrap()["semantic_model"], serde_json::Value::Null);
    assert_eq!(request(&without, "/models/e5-de-en-0000000000000000.bin", &[]).await.0, StatusCode::NOT_FOUND);

    use crate::semantic::Model;
    assert!(Model::load(&temp_dir("semantic-missing").join("model.bin"), None).is_err());
    let store = temp_dir("semantic-store");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("model.bin"), b"PK\x03\x04").unwrap();
    assert!(Model::load(&store.join("model.bin"), None).is_err(), "not E5Q1");

    let bytes: Vec<u8> = b"E5Q1".iter().copied().chain((0..200_000u32).map(|i| (i % 7) as u8)).collect();
    // In the model store a file is named by its content's sha256: a damaged one is refused.
    use sha2::Digest;
    let sum: String = sha2::Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(store.join(&sum), &bytes).unwrap();
    std::fs::write(store.join("0".repeat(64)), &bytes).unwrap();
    let passage = Some("0539da78bb98e8bb");
    let loaded = Model::load(&store.join(&sum), passage).unwrap();
    assert_eq!((loaded.passage.as_deref(), loaded.path.clone()), (passage, format!("/models/e5-de-en-{}.bin", &sum[..16])));
    let damaged = Model::load(&store.join("0".repeat(64)), passage).err().unwrap().to_string();
    assert!(damaged.contains("damaged"), "{damaged}");
    for wrong in ["0539DA78BB98E8BB", "0539da78", "not-a-model-id!!"] {
        assert!(Model::load(&store.join(&sum), Some(wrong)).is_err(), "{wrong}");
    }
    let model = loaded;
    let path = model.path.clone();
    assert!(path.starts_with("/models/e5-de-en-") && path.ends_with(".bin") && path.len() == "/models/e5-de-en-.bin".len() + 16, "{path}");
    let mut with = state(SnapshotStore::new(temp_dir("semantic-model")).unwrap());
    with.semantic = Some(Arc::new(model));
    let router = crate::router(with);

    let (status, headers, body) = request(&router, &path, &[]).await;
    assert_eq!((status, body.len()), (StatusCode::OK, bytes.len()));
    assert!(body == bytes);
    assert_eq!(headers[header::CACHE_CONTROL], crate::api::Keep::IMMUTABLE, "its address names its content");
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(request(&router, &path, &[("if-none-match", &etag)]).await.0, StatusCode::NOT_MODIFIED);
    let (_, headers, brotli) = request(&router, &path, &[("accept-encoding", "br")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert_eq!(crate::encoding::unbrotli(&brotli).as_deref(), Some(&bytes[..]));
    for other in ["/models/e5-de-en-0000000000000000.bin", "/models/model.bin", "/models/"] {
        assert_eq!(request(&router, other, &[]).await.0, StatusCode::NOT_FOUND, "{other}");
    }

    let (_, _, status) = request(&router, "/api/status", &[]).await;
    let status = serde_json::from_slice::<serde_json::Value>(&status).unwrap();
    assert_eq!((&status["semantic_model"], &status["semantic_passage_model"]), (&serde_json::json!(path), &serde_json::json!("0539da78bb98e8bb")));
    let (_, _, boot) = request(&router, "/assets/boot.js?v=test", &[]).await;
    let boot = String::from_utf8(boot).unwrap();
    let written = serde_json::json!({ "url": path, "passage": "0539da78bb98e8bb" }).to_string();
    assert!(boot.contains(&written) && !boot.contains("__SEMANTIC_MODEL__"), "{boot}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_launch_screens_of_ios_are_drawn_as_large_as_their_screen() {
    let router = crate::router(state(SnapshotStore::new(temp_dir("launch")).unwrap()));
    let page = String::from_utf8(request(&router, "/", &[]).await.2).unwrap();
    let head = page.split("</head>").next().unwrap_or_default();
    assert!(head.contains(folia_app::HEAD_SCRIPT) && folia_app::HEAD_SCRIPT.contains("apple-touch-startup-image"), "{page}");
    let (status, headers, png) = request(&router, "/assets/launch/1179x2556-dark.png", &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"));
    assert!(png.starts_with(b"\x89PNG") && png[16..24] == [0, 0, 4, 155, 0, 0, 9, 252], "1179 x 2556");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(request(&router, "/assets/launch/1179x2556-dark.png", &[("if-none-match", &etag)]).await.0, StatusCode::NOT_MODIFIED);
    // A phone opens upright only; names that are no picture are not there.
    for missing in ["/assets/launch/2556x1179.png", "/assets/launch/1179x2556.jpg", "/assets/launch/100x100.png"] {
        assert_eq!(request(&router, missing, &[]).await.0, StatusCode::NOT_FOUND, "{missing}");
    }
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
    for path in [folia_app::STYLESHEET, "/assets/app.css?v=test", folia_app::FONT, folia_app::FAVICON, folia_app::FAVICON_ICO, folia_app::TOUCH_ICON, folia_app::ICON_192, folia_app::ICON_MASKABLE_LARGE, folia_app::ICON_MONOCHROME, folia_app::MANIFEST, "/assets/launch/750x1334.png"] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::OK, "{path}");
    }
    // Only the launch screens a page names: another name under their path stays behind the gate.
    assert_eq!(request(&router, "/assets/launch/100x100.png", &[]).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE, "answered by the health check (no snapshot here), not by the gate");
    // The container's own probe (`folia healthcheck`) has no password and needs no snapshot.
    let (status, headers, body) = request(&router, crate::api::LIVENESS, &[]).await;
    assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap(), body.as_slice()), (StatusCode::OK, "no-store", &b"ok\n"[..]));
    let (status, _, robots) = request(&router, "/robots.txt", &[]).await;
    assert_eq!((status, String::from_utf8(robots).unwrap().as_str()), (StatusCode::OK, "User-agent: *\nAllow: /calendar/\nAllow: /en/calendar/\nDisallow: /\n"));
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
    assert!(html.split("</head>").next().unwrap().contains(&format!("<style>{}</style>", folia_app::VIEW_TRANSITION_STYLE)), "the login page fades like the site: {html}");
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
    assert_eq!(request(&router, "/assets/boot.js?v=test", &with_cookie).await.1[header::CACHE_CONTROL], "private, max-age=31536000, immutable", "kept for a year, by this browser alone");
    assert_eq!(request(&router, folia_app::STYLESHEET, &with_cookie).await.1[header::CACHE_CONTROL], "public, no-cache", "what is open anyway stays shared");
    assert_eq!(request(&router, "/assets/app.css?v=test", &with_cookie).await.1[header::CACHE_CONTROL], crate::api::Keep::IMMUTABLE);
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

/// Impressum and Datenschutz are there without a catalog, name who runs Betula and how to reach
/// them, and every page leads to them (§ 5 DDG): the ground at its end. The privacy notice has
/// every part its sidebar lists. Needs no snapshot.
#[tokio::test(flavor = "multi_thread")]
async fn legal_pages_are_one_step_from_every_page() {
    let router = crate::router(state(SnapshotStore::new(temp_dir("legal")).unwrap()));
    for path in [folia_routes::url::IMPRINT, folia_routes::url::PRIVACY] {
        let (status, _, body) = request(&router, path, &[]).await;
        let page = String::from_utf8(body).unwrap();
        assert_eq!(status, StatusCode::OK, "{path} needs no snapshot");
        for text in [folia_app::pages::legal::NAME, "Querstraße 23", "14656 Brieselang", &format!("href=\"mailto:{}\"", folia_app::pages::legal::EMAIL)] {
            assert!(page.contains(text), "{path}: {text}");
        }
        let head = page.split("</head>").next().unwrap_or_default();
        assert_eq!(head.contains("noindex"), folia_app::pages::legal::PLACEHOLDER, "{path}");
    }
    let (_, _, body) = request(&router, folia_routes::url::PRIVACY, &[]).await;
    let privacy = String::from_utf8(body).unwrap();
    for part in &folia_app::pages::legal::PRIVACY {
        assert!(privacy.contains(&format!("id=\"{}\"", part.id)) && privacy.contains(&format!("href=\"#{}\"", part.id)), "{}: {}", part.id, (part.heading)(&folia_app::i18n::legal::DE));
    }

    // Any other page, here the program overview, which says that it has no catalog: the ground at
    // its end (`folia_shell::ground`) links both.
    let (status, _, body) = request(&router, folia_routes::url::PROGRAMS, &[]).await;
    let page = String::from_utf8(body).unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let ground = page.split("<footer class=\"ground\">").nth(1).and_then(|rest| rest.split("</footer>").next()).unwrap_or_default();
    for path in [folia_routes::url::IMPRINT, folia_routes::url::PRIVACY] {
        assert!(ground.contains(&format!("href=\"{path}\"")), "the ground: {ground}");
    }
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
    assert!(active.brotli.get().is_none(), "brotli follows once it is active");
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
        let db = folia_model::native::NativeDatabase::open(&snapshot_file()).unwrap();
        let url = folia_routes::url::CatalogUrl::parse("form=exercise&turnus=winter&status=all");
        folia_query::catalog_count(&db, &url.query).unwrap()
    };
    assert!(expected > 100);
    assert!(html.replace("<!>", "").contains(&format!("class=\"count num\">{}</span>", folia_design::format::count(expected, folia_locale::Locale::De))), "the header shows the exact total {expected}");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    // The same filter written differently is the same page.
    let (_, headers, _) = request(&router, "/catalog?status=all&turnus=winter&form=exercise&q=", &[]).await;
    assert_eq!(headers["x-cache"], "hit");
    let (status, headers, body) = request(&router, "/catalog?turnus=winter&form=exercise&status=all", &[("if-none-match", &etag)]).await;
    let kept = |headers: &HeaderMap| (headers[header::CACHE_CONTROL].to_str().unwrap().to_string(), headers[header::VARY].to_str().unwrap().to_string());
    assert_eq!((status, body.len()), (StatusCode::NOT_MODIFIED, 0));
    assert_eq!(kept(&headers), ("public, no-cache".to_string(), "Accept-Encoding".to_string()), "„unchanged\" says how to keep the page, as its 200 did");
    // In brotli to a browser, as the cache keeps it; gzipped anew for a client that takes only
    // gzip, plain for one that takes neither.
    let (_, headers, body) = request(&router, "/programs", &[("accept-encoding", "gzip, deflate, br, zstd")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    let programs = crate::encoding::unbrotli(&body).expect("brotli");
    assert!(programs.starts_with(b"<!DOCTYPE html>"));
    let (_, headers, body) = request(&router, "/programs", &[("accept-encoding", "gzip, deflate")]).await;
    assert_eq!((headers[header::CONTENT_ENCODING].to_str().unwrap(), &body[..2]), ("gzip", &[0x1f, 0x8b][..]));
    assert_eq!(crate::encoding::gunzip(&body), Some(programs.clone()));
    let (_, headers, body) = request(&router, "/programs", &[]).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(body, programs.to_vec());

    // What search engines read: one description and one address per page, absolute, with the name
    // the site has from outside; views of the lists are not listed; the sitemap names every page.
    let head = |html: &str| html.split("</head>").next().unwrap_or_default().to_string();
    let (_, _, body) = request(&router, "/catalog/module/11101", &[]).await;
    let module_page = String::from_utf8(body).unwrap();
    // The wood stands behind every page, once (`ground::Wood`), not only behind the start page. Its
    // attributes in either order: Leptos writes the class after the others, whatever the view says.
    let woods = |html: &str| ["<div class=\"wood\" aria-hidden=\"true\"></div>", "<div aria-hidden=\"true\" class=\"wood\"></div>"].iter().map(|wood| html.matches(wood).count()).sum::<usize>();
    assert_eq!(woods(&module_page), 1, "the module page has no wood behind it");
    let module = head(&module_page);
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
    // What search engines, and the assistants that answer with them, read of Betula itself: the
    // app with its abilities, the questions, and „Betula im Detail" with every filter (2026-09-28).
    assert!(home.contains("\"@type\":\"WebApplication\"") && home.contains("\"@type\":\"FAQPage\""), "the start page's structured data");
    assert!(home.contains("id=\"im-detail\"") && home.matches("class=\"panel feature t-").count() == 8 && home.matches("class=\"bgroup").count() == 12, "the start page's „Betula im Detail\"");
    // No sidebar (owner, 2026-09-28): the page is one scroll area without a frame (`#page-scroll`,
    // app.css „one scroll area"), its attributes in either order as the wood's. The way in for a
    // first visit: three steps, the first the next one, since the server knows nothing of the
    // visitor (R9); „Studiengang wählen" there and in the first panel a link to all programs (the
    // picker is the app's); the figures on a birch; the wood behind the page, and no branches out
    // of the panels any more.
    let scroll_area = ["<div class=\"work flowing solo\" id=\"page-scroll\">", "<div id=\"page-scroll\" class=\"work flowing solo\">"].iter().map(|area| home.matches(&format!("{area}<div class=\"page home-page\">")).count()).sum::<usize>();
    assert!(!home.contains("id=\"sidebar\"") && home.matches("id=\"page-scroll\"").count() == 1 && scroll_area == 1, "the start page has no frame");
    assert!(home.contains("id=\"loslegen\"") && home.matches("class=\"start-step ").count() + home.matches("class=\"start-step\"").count() == 3 && home.matches("is-next").count() == 1 && !home.contains("is-done"), "the start page's way in");
    assert!(["home-program", "start-program"].iter().all(|id| home.contains(&format!("<a id=\"{id}\" href=\"/programs\""))) && home.matches("program-pick\"").count() == 2 && home.contains("<dl class=\"tree-figures\">") && home.contains("class=\"hero-trunk\"") && woods(&home) == 1 && !home.contains("class=\"branch"), "the start page's buttons, figures and wood");
    // Impressum and Datenschutz: linked from the ground at the end of every page, the start page's
    // included (`legal_pages_are_one_step_from_every_page`), indexed once they are final
    // (deploy/ship.sh keeps an instance open to everybody from shipping while `PLACEHOLDER` is true).
    for path in [folia_routes::url::IMPRINT, folia_routes::url::PRIVACY] {
        assert!(home.contains(&format!("href=\"{path}\"")), "the start page links {path}");
        let (status, _, body) = request(&router, path, &[]).await;
        let page = String::from_utf8(body).unwrap();
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(page.contains(folia_app::pages::legal::NAME), "{path} names who runs Betula");
        assert_eq!(head(&page).contains("noindex"), folia_app::pages::legal::PLACEHOLDER, "{path}");
    }

    // What a link preview and a home screen read: the card of the page with an absolute picture,
    // and the icons and the manifest of the site, each served as what it is.
    for tag in ["property=\"og:title\"", "property=\"og:description\"", "name=\"twitter:card\"", "name=\"twitter:image\"", "property=\"og:image:alt\""] {
        assert_eq!(module.matches(tag).count(), 1, "{tag} in {module}");
    }
    for link in ["rel=\"manifest\"", "rel=\"apple-touch-icon\"", "href=\"/favicon.ico\"", "name=\"theme-color\"", "rel=\"stylesheet\"", "rel=\"preload\""] {
        assert_eq!(head(&home).matches(link).count(), 1, "{link}");
    }
    // What Google Search shows beside a result: of the pictures the start page links as `icon` or
    // `apple-touch-icon`, the largest it can read (no SVG). That has to be the icon of the app,
    // square and larger than 48 px (`folia_app::ICON_192`); the mark (`/favicon.ico`) is 48 at most.
    let mut largest = (0, String::new());
    for link in head(&home).split("<link ").skip(1).filter_map(|tag| tag.split('>').next()) {
        if !(link.contains("rel=\"icon\"") || link.contains("rel=\"apple-touch-icon\"")) || link.contains("image/svg+xml") {
            continue;
        }
        let href = link.split("href=\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap();
        let (status, _, picture) = request(&router, href, &[]).await;
        assert_eq!(status, StatusCode::OK, "{href}");
        let (width, height) = if picture.starts_with(b"\x89PNG") {
            (u32::from_be_bytes(picture[16..20].try_into().unwrap()), u32::from_be_bytes(picture[20..24].try_into().unwrap()))
        } else {
            // An ICO: its widest entry (a width of 0 means 256).
            let side = |byte: u8| if byte == 0 { 256 } else { u32::from(byte) };
            let entries = usize::from(u16::from_le_bytes([picture[4], picture[5]]));
            (0..entries).map(|entry| (side(picture[6 + 16 * entry]), side(picture[7 + 16 * entry]))).max().unwrap()
        };
        assert_eq!(width, height, "{href} is square");
        if width > largest.0 {
            largest = (width, href.to_string());
        }
    }
    assert_eq!(largest, (192, folia_app::ICON_192.to_string()), "{}", head(&home));
    // The fade between pages is opted into in the head itself: from the stylesheet alone the
    // browser may learn of it too late (`folia_app::VIEW_TRANSITION_STYLE`).
    assert_eq!(head(&home).matches(&format!("<style>{}</style>", folia_app::VIEW_TRANSITION_STYLE)).count(), 1, "{home}");
    // The stylesheet and the scripts are linked with the build that wrote the page, so that a
    // service worker of another build never answers them from its cache; the address still
    // leads to the file, whatever build it names.
    for link in ["rel=\"stylesheet\" href=\"/assets/app.css?v=test\"", "src=\"/assets/enhance.js?v=test\"", "src=\"/assets/boot.js?v=test\""] {
        assert_eq!(head(&home).matches(link).count(), 1, "{link} in {home}");
    }
    for path in ["/assets/app.css?v=test", "/assets/app.css?v=an-older-build", "/assets/boot.js?v=test", "/assets/sql-wasm.wasm?v=test"] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::OK, "{path}");
    }
    // Under exactly the address of this build a file is kept for a year and never asked for
    // again: nothing the process serves there changes while it runs. Under any other it is asked
    // for again every time (a 304 while it is the same), since the server answers every `?v=`
    // with the file it has; so are the worker and the season's picture, whatever they are asked
    // with, and the page that names the build.
    let immutable = crate::api::Keep::IMMUTABLE;
    let revalidate = crate::cache::REVALIDATE;
    for (path, keep) in [
        ("/assets/app.css?v=test", immutable),
        ("/assets/enhance.js?v=test", immutable),
        ("/assets/boot.js?v=test", immutable),
        ("/assets/icons.svg?v=test", immutable),
        ("/assets/sql-wasm.js?v=test", immutable),
        ("/assets/sql-wasm.wasm?v=test", immutable),
        ("/assets/app.css", revalidate),
        ("/assets/app.css?v=an-older-build", revalidate),
        ("/assets/app.css?v=test&v=another", revalidate),
        ("/assets/app.css?x=1&v=test", revalidate),
        ("/assets/inter-latin.woff2", revalidate),
        ("/manifest.webmanifest", revalidate),
        ("/sw.js", revalidate),
        ("/sw.js?v=test", revalidate),
        ("/assets/og.png?v=test", revalidate),
        ("/", revalidate),
    ] {
        let (status, headers, _) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::OK, keep), "{path}");
        let etag = headers[header::ETAG].to_str().unwrap().to_string();
        let (status, headers, _) = request(&router, path, &[("if-none-match", &etag)]).await;
        assert_eq!((status, kept(&headers)), (StatusCode::NOT_MODIFIED, (keep.to_string(), "Accept-Encoding".to_string())), "{path}");
    }
    // Every file of the app goes out in brotli to a browser, made once at its best: the same bytes
    // for everybody.
    let (_, headers, css) = request(&router, "/assets/app.css?v=test", &[("accept-encoding", "gzip, deflate, br, zstd")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert_eq!(crate::encoding::unbrotli(&css).as_deref(), Some(crate::assets::text("app.css").as_bytes()), "minified by the build");
    assert_eq!(request(&router, "/assets/app.css", &[("accept-encoding", "br")]).await.2, css, "made once");
    assert!(css.len() < crate::assets::text("app.css").len() / 4);
    let (_, headers, wasm) = request(&router, "/assets/sql-wasm.wasm?v=test", &[("accept-encoding", "br")]).await;
    assert_eq!((headers[header::CONTENT_ENCODING].to_str().unwrap(), headers[header::CONTENT_TYPE].to_str().unwrap()), ("br", "application/wasm"));
    assert_eq!(crate::encoding::unbrotli(&wasm).as_deref(), Some(&include_bytes!("../../../assets/sql-wasm.wasm")[..]));
    // What is compressed already goes out as it is.
    let (_, headers, font) = request(&router, folia_app::FONT, &[("accept-encoding", "br")]).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none() && font == include_bytes!("../../../assets/inter-latin.woff2"));
    // The worker knows the build too: it keeps the files under the addresses this build links.
    // Both are served minified (`assets`), with what the server knows written in.
    let (_, _, worker) = request(&router, folia_app::SERVICE_WORKER, &[]).await;
    let worker = String::from_utf8(worker).unwrap();
    assert_eq!(worker, crate::assets::text("sw.js").replace("__BUILD__", "test"));
    assert!(!worker.contains("__BUILD__") && ["\"test\"", "'test'", "`test`"].iter().any(|build| worker.contains(build)), "{worker}");
    // The boot knows the schema its build reads, and opens no local copy of an older one.
    let (_, _, boot) = request(&router, "/assets/boot.js?v=test", &[]).await;
    let boot = String::from_utf8(boot).unwrap();
    assert_eq!(boot, crate::assets::text("boot.js").replace("__SCHEMA__", &folia_model::SCHEMA_VERSION.to_string()).replace("__SEMANTIC_MODEL__", "null"));
    assert!(!boot.contains("__SCHEMA__") && !boot.contains("__SEMANTIC_MODEL__"), "{boot}");
    // Every answer names the build that gave it; the worker keeps only the answers of its own.
    for path in ["/", "/catalog", "/assets/app.css?v=test", folia_app::SERVICE_WORKER, "/manifest.webmanifest"] {
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
    let (_, _, stylesheet) = request(&router, "/assets/app.css?v=test", &[]).await;
    let stylesheet = String::from_utf8(stylesheet).unwrap();
    let masks: std::collections::BTreeSet<&str> = crate::warm::masks(&stylesheet).collect();
    assert!(masks.len() >= 12, "{masks:?}");
    for path in masks {
        let (status, headers, body) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/svg+xml"), "{path}");
        assert!(body.starts_with(b"<svg"), "{path}");
    }
    assert_eq!(request(&router, "/assets/birch/no-such-season.svg", &[]).await.0, StatusCode::NOT_FOUND);
    // The wood goes out as the brotli it was drawn with to a browser that takes it, gzipped to one
    // that takes only gzip, and plain to one that gives `br` no weight.
    let wood = "/assets/birch/summer-wood-front.svg";
    let (status, headers, body) = request(&router, wood, &[("accept-encoding", "gzip, deflate, br, zstd")]).await;
    assert_eq!((status, headers[header::CONTENT_ENCODING].to_str().unwrap(), headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "br", "image/svg+xml"));
    assert_eq!(body, include_bytes!("../../../assets/birch/summer-wood-front.svg.br"));
    let (_, headers, _) = request(&router, wood, &[("accept-encoding", "gzip")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING].to_str().unwrap(), "gzip");
    let (_, headers, body) = request(&router, wood, &[("accept-encoding", "br;q=0")]).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none() && body.starts_with(b"<svg"));
    // The other masks in the brotli the server makes of them.
    let (_, headers, body) = request(&router, "/assets/birch/summer-crown.svg", &[("accept-encoding", "gzip, br")]).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert_eq!(crate::encoding::unbrotli(&body).as_deref(), crate::birch::file("summer-crown.svg").map(str::as_bytes));
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
    // The Merkliste and the Stundenplan: a picture each, the same for everybody (what a visitor
    // keeps lives in the browser), named by their pages.
    for (page, card) in [(folia_routes::url::BOOKMARKS, folia_shell::seo::BOOKMARKS_CARD), (folia_routes::url::STUDYPLAN, folia_shell::seo::STUDYPLAN_CARD)] {
        let body = String::from_utf8(request(&router, page, &[]).await.2).unwrap();
        assert!(head(&body).contains(&format!("content=\"https://catalog.example{card}\"")), "{page}: {body}");
        let (status, headers, png) = request(&router, card, &[]).await;
        assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"), "{card}");
        assert!(png.starts_with(b"\x89PNG"), "{card}");
    }
    // A Stundenplan handed on by a link: its page (one per code) names its modules and its own
    // picture, which shows them; a code whose modules the catalog does not know has none.
    let semester = SemesterKey::parse("2026W").unwrap();
    let code = SharedPlan::of(semester, &["11101".to_string()], None).unwrap().code().unwrap();
    let shared = String::from_utf8(request(&router, &folia_routes::url::share_path(&code), &[]).await.2).unwrap();
    let card = share::card_path(&code);
    assert!(head(&shared).contains(&format!("content=\"https://catalog.example{card}\"")) && head(&shared).contains("1 Modul: "), "{shared}");
    let plain = String::from_utf8(request(&router, folia_routes::url::STUDYPLAN, &[]).await.2).unwrap();
    assert!(!plain.contains(&code), "the plain page is another page than the shared one");
    let (status, headers, png) = request(&router, &card, &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"));
    assert!(png.starts_with(b"\x89PNG") && png[16..24] == [0, 0, 4, 176, 0, 0, 2, 118], "1200 x 630");
    let unknown = SharedPlan::of(semester, &["99999".to_string()], None).unwrap().code().unwrap();
    for missing in [share::card_path(&unknown), "/cards/studyplan/not-a-code.png".to_string(), format!("/cards/studyplan/{code}")] {
        assert_eq!(request(&router, &missing, &[]).await.0, StatusCode::NOT_FOUND, "{missing}");
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
    // Pages only, no filters: an address with a query is the plan of a further study direction.
    let urls: Vec<&str> = sitemap.lines().filter(|line| line.starts_with("<url>")).collect();
    assert!(urls.len() > 3000 && urls.iter().all(|line| !line.contains('?') || line.contains("/plan?variant=")), "pages only, no filters");
    assert!(sitemap.contains("/plan?variant=2</loc>"), "the plan of every study direction is listed (Elektrotechnik B.Sc. has two)");
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
    let map: folia_pages::graph::ProgramMap = serde_json::from_slice(&body).unwrap();
    assert_eq!(Some(&map), active.program_map.as_ref().map(|(map, ..)| map.as_ref()));
    assert!(map.programs.len() > 100 && map.wide.dots.len() == map.programs.len() && map.tall.dots.len() == map.programs.len());
    assert_eq!(request(&router, "/api/map.json", &[("if-none-match", map_etag.as_str())]).await.0, StatusCode::NOT_MODIFIED);
    assert_eq!(request(&router, "/api/map.json", &[("if-none-match", "\"aaaa1111\"")]).await.0, StatusCode::OK);
    assert_eq!(request(&router, folia_app::OG_IMAGE, &[]).await.0, StatusCode::OK);

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
    let (status, headers, _) = request(&router, "/api/db", &[("if-none-match", "\"aaaa1111\"")]).await;
    assert_eq!((status, kept(&headers)), (StatusCode::NOT_MODIFIED, ("public, no-cache".to_string(), "Accept-Encoding".to_string())));
    // Brotli once it is made, in the background after the switch (here at a quicker quality than
    // the server's own): beside the file for the next start, and in memory. Gzip until then.
    assert_eq!(request(&router, "/api/db", &[("accept-encoding", "gzip, deflate, br, zstd")]).await.1[header::CONTENT_ENCODING], "gzip");
    assert!(crate::snapshot::compress_active(&store, crate::encoding::FAST).await);
    let (status, headers, body) = request(&router, "/api/db", &[("accept-encoding", "gzip, deflate, br, zstd")]).await;
    assert_eq!((status, headers[header::CONTENT_ENCODING].to_str().unwrap()), (StatusCode::OK, "br"));
    assert_eq!(headers[header::CONTENT_LENGTH].to_str().unwrap(), body.len().to_string());
    assert!(body.len() < active.gzip.as_ref().unwrap().1 as usize, "smaller than gzip");
    assert_eq!(crate::encoding::unbrotli(&body).as_deref(), Some(real.as_slice()));
    let mut beside = active.path.clone().into_os_string();
    beside.push(".br");
    assert_eq!(std::fs::read(&beside).unwrap(), body);
    assert!(crate::snapshot::compress_active(&store, crate::encoding::FAST).await, "made once");
    let (_, _, status_body) = request(&router, "/api/status", &[]).await;
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["snapshot"]["etag"], "\"aaaa1111\"");
    assert!(status_json["html_cache"]["pages"].as_u64().unwrap() >= 2);
    // And its schema, which a browser compares with the one its build reads before it fetches it.
    let schema = folia_model::native::NativeDatabase::open(&snapshot_file()).unwrap().schema_version().unwrap();
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
    older[60..64].copy_from_slice(&((folia_model::SCHEMA_VERSION - 1) as u32).to_be_bytes());
    *radix.current.lock().unwrap() = Some(("\"dddd4444\"".to_string(), older));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let (_, _, status_body) = request(&router, "/api/status", &[]).await;
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!((status_json["snapshot"]["etag"].as_str(), status_json["snapshot"]["schema_version"].as_i64()), (Some("\"dddd4444\""), Some(folia_model::SCHEMA_VERSION - 1)));
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
    assert_eq!(ics, folia_pages::calendar(&db, &Subscription::from_code(FIRST_SEMESTER_CODE).unwrap(), folia_locale::Locale::De).unwrap());
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
    // Compressed here on request (the edge's compression does not take `text/calendar`): in
    // brotli to whoever takes it, else in gzip.
    let (status, compressed, body) = request(&router, &path, &[("accept-encoding", "gzip, br")]).await;
    assert_eq!((status, compressed[header::CONTENT_ENCODING].to_str().unwrap()), (StatusCode::OK, "br"));
    assert_eq!(crate::encoding::unbrotli(&body).as_deref(), Some(ics.as_bytes()));
    let (status, zipped, body) = request(&router, &path, &[("accept-encoding", "gzip")]).await;
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
    // Eight feeds were made above: the 304 is made too, since its tag is the content's.
    assert_eq!(log.matches("calendar.served").count(), 8, "{log}");
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
    let bookmarks = folia_stores::bookmarks::transfer_fragment(&["11112".to_string(), "12104".to_string()]).unwrap();
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

/// A shared Stundenplan's code names someone's modules too: Folia's log writes the path of its
/// picture as one fixed text, and a page's query, where the code travels, never.
#[tokio::test(flavor = "multi_thread")]
async fn the_log_keeps_no_shared_plan() {
    let semester = SemesterKey::parse("2026W").unwrap();
    let code = SharedPlan::of(semester, &["12104".to_string(), "11101".to_string()], Some("079-82-2008")).unwrap().code().unwrap();
    let router = crate::router(state(SnapshotStore::new(temp_dir("log-share")).unwrap()));
    let asked = [share::card_path(&code), folia_routes::url::share_path(&code), "/cards/studyplan/secret.png".to_string()];
    let log = Captured::default();
    let logging = log.start();
    for path in &asked {
        request(&router, path, &[]).await;
    }
    drop(logging);
    let log = log.text();
    assert_eq!(log.matches("http.request").count(), asked.len(), "one line per request: {log}");
    assert_eq!(log.matches("/cards/studyplan/….png").count(), 2, "{log}");
    for secret in [code.as_str(), "secret"] {
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
    let (_, _, worker) = request(&router, folia_app::SERVICE_WORKER, &[]).await;
    // The rule it goes by, as the minified worker writes it.
    assert!(String::from_utf8(worker).unwrap().contains("=/^\\/(api\\/|access|sw\\.js$|([a-z]{2}\\/)?(cards|calendar)\\/)/"));
}

/// Whether robots.txt lets `agent` fetch `address`, read as Google reads it (RFC 9309): the group
/// that names the agent, else the one for everybody; of its rules that match, the longest decides,
/// and `Allow` wins a tie; `*` is any run of characters, `$` the end of the address.
fn robots_allow(robots: &str, agent: &str, address: &str) -> bool {
    fn matches(pattern: &str, address: &str) -> bool {
        let (pattern, to_end) = pattern.strip_suffix('$').map_or((pattern, false), |pattern| (pattern, true));
        let mut parts = pattern.split('*');
        let Some(mut rest) = parts.next().and_then(|first| address.strip_prefix(first)) else { return false };
        let parts: Vec<&str> = parts.collect();
        for (i, part) in parts.iter().enumerate() {
            if to_end && i + 1 == parts.len() {
                return rest.ends_with(part);
            }
            let Some(at) = rest.find(part) else { return false };
            rest = &rest[at + part.len()..];
        }
        !to_end || rest.is_empty()
    }
    // The agents a group names and its rules (`Allow` or not, the pattern): the lines
    // `User-agent` in a row open a group, its rules follow.
    type Group = (Vec<String>, Vec<(bool, String)>);
    let mut groups: Vec<Group> = Vec::new();
    let mut naming = false;
    for line in robots.lines() {
        let Some((field, value)) = line.split_once(':') else { continue };
        let (field, value) = (field.trim().to_ascii_lowercase(), value.trim().to_string());
        match field.as_str() {
            "user-agent" => {
                if !naming {
                    groups.push((Vec::new(), Vec::new()));
                }
                naming = true;
                groups.last_mut().unwrap().0.push(value.to_ascii_lowercase());
            }
            "allow" | "disallow" => {
                naming = false;
                if let Some(group) = groups.last_mut().filter(|_| !value.is_empty()) {
                    group.1.push((field == "allow", value));
                }
            }
            _ => naming = false,
        }
    }
    let agent = agent.to_ascii_lowercase();
    let group = groups.iter().find(|(agents, _)| agents.contains(&agent)).or_else(|| groups.iter().find(|(agents, _)| agents.iter().any(|named| named == "*")));
    let Some((_, rules)) = group else { return true };
    rules.iter().filter(|(_, pattern)| matches(pattern, address)).max_by_key(|(allow, pattern)| (pattern.len(), *allow)).is_none_or(|(allow, _)| *allow)
}

/// The links of a page: where each leads (`&amp;` read back) and whether it says `nofollow`.
fn links(html: &str) -> Vec<(String, bool)> {
    html.split("<a ")
        .skip(1)
        .filter_map(|rest| {
            let tag = rest.split('>').next().unwrap_or_default();
            let value = |name: &str| {
                let start = format!("{name}=\"");
                tag.strip_prefix(&start).or_else(|| tag.split(&format!(" {start}")).nth(1)).and_then(|value| value.split('"').next())
            };
            let nofollow = value("rel").is_some_and(|rel| rel.split_whitespace().any(|part| part == "nofollow"));
            value("href").map(|href| (href.replace("&amp;", "&"), nofollow))
        })
        .collect()
}

/// Googlebot had fetched 250,000 views of the catalog by 2026-09-30: every filter it followed had
/// links to more. A crawler that keeps to robots.txt and follows no `nofollow` meets pages only
/// (`folia_routes::url::listed`), and every one of them: every link it may follow leads to a page it
/// may fetch, robots.txt lets it fetch every page of the sitemap and every page of the catalog
/// (`/catalog?page=<n>`, the way to every module) and keeps it out of the views of the lists. The
/// link previews of X, LinkedIn and Facebook fetch a filtered list somebody shares all the same.
#[tokio::test(flavor = "multi_thread")]
async fn crawlers_are_led_to_pages_and_kept_out_of_views() {
    let router = crate::router(state(store_with("crawl", &snapshot_file())));
    let (status, _, robots) = request(&router, "/robots.txt", &[]).await;
    let robots = String::from_utf8(robots).unwrap();
    assert_eq!(status, StatusCode::OK);
    // The reading of the rules, on Google's own examples.
    let example = "User-agent: *\nDisallow: /*.php$\nDisallow: /fish*\nAllow: /fish/salmon\n";
    assert!(!robots_allow(example, "Googlebot", "/index.php") && robots_allow(example, "Googlebot", "/index.php?x=1"));
    assert!(!robots_allow(example, "Googlebot", "/fish.html") && robots_allow(example, "Googlebot", "/fish/salmon/1") && robots_allow(example, "Googlebot", "/Fish"));

    for page in ["/", "/en", "/catalog", "/catalog?page=2", "/catalog?page=24", "/en/catalog?page=2", "/catalog/module/11101", "/programs", "/programs/x/plan?variant=2", "/bookmarks", "/studyplan", "/studyplan?share=x", "/calendar/x.ics", "/en/calendar/x.ics", "/cards/module/11101.png", "/sitemap.xml"] {
        assert!(robots_allow(&robots, "Googlebot", page), "{page}: {robots}");
    }
    for view in ["/catalog?turnus=winter", "/catalog?turnus=winter&page=2", "/catalog?sort=title", "/catalog?q=analysis", "/catalog?page=2&open=11101", "/en/catalog?form=lecture", "/en/catalog?page=2&sort=ects", "/programs?level=master", "/en/programs?plan=1", "/bookmarks?turnus=winter", "/api/db", "/api/status"] {
        assert!(!robots_allow(&robots, "Googlebot", view), "{view}: {robots}");
    }
    for agent in ["Twitterbot", "LinkedInBot", "facebookexternalhit"] {
        assert!(robots_allow(&robots, agent, "/catalog?turnus=winter") && robots_allow(&robots, agent, "/en/programs?level=master") && !robots_allow(&robots, agent, "/api/status"), "{agent}");
    }
    let (_, _, sitemap) = request(&router, "/sitemap.xml", &[]).await;
    let sitemap = String::from_utf8(sitemap).unwrap();
    let listed: Vec<String> = sitemap.split("<loc>").skip(1).filter_map(|rest| rest.split("</loc>").next()).map(|loc| loc.replace("https://catalog.example", "").replace("&amp;", "&")).collect();
    assert!(listed.len() > 1000);
    for page in &listed {
        assert!(robots_allow(&robots, "Googlebot", page), "{page} is in the sitemap");
    }

    // A program with the plans of two study directions: its second plan is a page of its own.
    let slug = listed.iter().find_map(|page| page.strip_prefix("/programs/")?.strip_suffix("/plan?variant=2")).unwrap().to_string();
    // A program whose sidebar names its other examination regulations: from „Mein Plan“ each leads
    // to the other one's „Mein Plan“ (ae431c9). The program above need not have any (with the
    // snapshot of 2026-09-30 it has, which is how the links were found); where the snapshot has such
    // a program, it is checked as well (then Bauingenieurwesen B.Sc. 2022 and 2017). The synthetic
    // snapshot has none.
    let versioned = {
        let db = NativeDatabase::open(&snapshot_file()).unwrap();
        let programs = folia_query::programs(&db).unwrap();
        programs.into_iter().filter(|p| p.is_latest_po).find(|p| !folia_query::program_versions(&db, &p.id).unwrap().is_empty()).map(|p| p.slug)
    };
    let mut pages = vec![
        "/".to_string(),
        "/catalog".to_string(),
        "/catalog?page=2".to_string(),
        "/catalog?turnus=winter".to_string(),
        "/catalog?turnus=winter&page=2".to_string(),
        format!("/catalog?program={slug}"),
        format!("/catalog?program={slug}&semester=1"),
        "/catalog/module/11101".to_string(),
        "/programs".to_string(),
        "/programs?level=master".to_string(),
        format!("/programs/{slug}/plan"),
        format!("/programs/{slug}/plan?variant=2"),
        format!("/programs/{slug}/areas"),
        format!("/programs/{slug}/my-plan"),
        "/bookmarks".to_string(),
        "/studyplan".to_string(),
        folia_routes::url::IMPRINT.to_string(),
        folia_routes::url::PRIVACY.to_string(),
    ];
    pages.extend(versioned.iter().flat_map(|versioned| [format!("/programs/{versioned}/plan"), format!("/programs/{versioned}/my-plan")]));
    // Every page is checked before the test fails, so that it lists all there is.
    let mut problems = Vec::new();
    let mut followed = std::collections::BTreeSet::new();
    for page in pages.iter().flat_map(|page| folia_locale::Locale::ALL.iter().map(move |locale| locale.path(page))) {
        let (status, _, body) = request(&router, &page, &[]).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        for (href, nofollow) in links(&String::from_utf8(body).unwrap()) {
            let address = href.split('#').next().unwrap_or_default();
            if nofollow || !address.starts_with('/') || address.starts_with("//") {
                continue;
            }
            if !folia_routes::url::listed(folia_locale::Locale::split(address).1) || !robots_allow(&robots, "Googlebot", address) {
                problems.push(format!("{page} lets a crawler follow {href}"));
            }
            followed.insert(address.to_string());
        }
    }
    problems.dedup();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // What it follows: the pages of the catalog one after the other, in both languages; the plan
    // of each study direction; modules and programs.
    let (second_plan, areas) = (format!("/programs/{slug}/plan?variant=2"), format!("/programs/{slug}/areas"));
    for page in ["/catalog?page=2", "/catalog?page=3", "/en/catalog?page=2", "/en/catalog?page=3", second_plan.as_str(), areas.as_str(), "/catalog/module/11101"] {
        assert!(followed.contains(page), "no link a crawler follows leads to {page}");
    }
    // The pager as a crawler reads it (e2e/crawl.mjs as well): the unfiltered list's is followed,
    // a filtered list's is not.
    let (_, _, body) = request(&router, "/catalog?page=2", &[]).await;
    let second = String::from_utf8(body).unwrap();
    assert!(second.contains("rel=\"prev\" href=\"/catalog\"") && second.contains("rel=\"next\" href=\"/catalog?page=3\""), "{second}");
    let (_, _, body) = request(&router, "/catalog?turnus=winter", &[]).await;
    assert!(String::from_utf8(body).unwrap().contains("rel=\"next nofollow\" href=\"/catalog?turnus=winter&amp;page=2\""));
}

/// A server with more work than places (`busy`): a page that finds no place within the wait is
/// answered 503 with `Retry-After`, and so is a calendar feed; what needs no place — `/livez`, a
/// cached page, a file — answers as always. (Measured before, on one processor: a queue that
/// only grew, `/livez` included, 6 to 13 seconds after half a minute.)
#[tokio::test(flavor = "multi_thread")]
async fn a_busy_server_turns_work_away_and_stays_alive() {
    let mut state = state(store_with("busy", &snapshot_file()));
    state.renders = Arc::new(crate::busy::Places::new("render", 1, std::time::Duration::from_millis(50)));
    state.feeds = Arc::new(crate::busy::Places::new("calendar", 1, std::time::Duration::from_millis(50)));
    let router = crate::router(state.clone());
    assert_eq!(request(&router, "/catalog/module/11101", &[]).await.1["x-cache"], "miss");

    // Every place taken, for longer than the wait.
    let _render = state.renders.enter().await.unwrap();
    let _feed = state.feeds.enter().await.unwrap();
    let (status, headers, body) = request(&router, "/catalog?turnus=winter", &[("accept", "text/html")]).await;
    assert_eq!((status, headers["x-cache"].to_str().unwrap(), headers[header::RETRY_AFTER].to_str().unwrap()), (StatusCode::SERVICE_UNAVAILABLE, "busy", "10"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert!(String::from_utf8(body).unwrap().contains("in ein paar Sekunden"));
    let head = Request::builder().method("HEAD").uri("/catalog?turnus=summer").body(Body::empty()).unwrap();
    assert_eq!(router.clone().oneshot(head).await.unwrap().status(), StatusCode::SERVICE_UNAVAILABLE, "HEAD renders under the same rules");
    let (status, headers, _) = request(&router, &subscription::path(FIRST_SEMESTER_CODE), &[]).await;
    assert_eq!((status, headers[header::RETRY_AFTER].to_str().unwrap()), (StatusCode::SERVICE_UNAVAILABLE, "120"));

    assert_eq!(request(&router, crate::api::LIVENESS, &[]).await.0, StatusCode::OK);
    assert_eq!(request(&router, "/catalog/module/11101", &[]).await.1["x-cache"], "hit");
    assert_eq!(request(&router, folia_app::STYLESHEET, &[]).await.0, StatusCode::OK);
}

/// A page asked for many times at once is rendered once: the others wait for that render and
/// are answered from the cache.
#[tokio::test(flavor = "multi_thread")]
async fn a_page_asked_for_at_once_is_rendered_once() {
    let router = crate::router(state(store_with("once", &snapshot_file())));
    let asks: Vec<_> = (0..8)
        .map(|_| {
            let router = router.clone();
            tokio::spawn(async move { request(&router, "/", &[("accept-encoding", "gzip")]).await.1["x-cache"].to_str().unwrap().to_string() })
        })
        .collect();
    let mut answers = Vec::new();
    for ask in asks {
        answers.push(ask.await.unwrap());
    }
    assert_eq!(answers.iter().filter(|answer| *answer == "miss").count(), 1, "{answers:?}");
    assert_eq!(answers.iter().filter(|answer| *answer == "hit").count(), 7, "{answers:?}");
}

/// The warm-up renders the pages of the sitemap into the cache, past the gate, and says how many.
#[tokio::test(flavor = "multi_thread")]
async fn the_warm_up_renders_the_pages_of_the_sitemap() {
    let store = store_with("warm", &snapshot_file());
    let mut state = state(store.clone());
    state.gate = Some(Arc::new(crate::access::Gate::new("birke im tagebau")));
    let paths = crate::api::sitemap_paths(&store.current().unwrap()).unwrap();
    assert!(paths.len() > 1000 && paths.first().map(String::as_str) == Some("/"));
    let some: Vec<String> = paths.iter().take(3).chain(paths.iter().rev().take(2)).cloned().collect();
    let pages = crate::pages(&state).with_state(state.clone());
    // On its way it notes what each page says: the dates of the sitemap.
    let changes = crate::lastmod::Changes::load(&temp_dir("warm-lastmod"));
    assert!(crate::warm::warm(&pages, &store, store.generation(), &some, Some(&changes)).await);
    assert_eq!(state.cache.size().0, some.len());
    let since = store.current().unwrap().meta.data_changed_at.clone().unwrap();
    assert!(some.iter().all(|path| changes.since(path).as_deref() == Some(since.as_str())), "{some:?}");
    // A newer snapshot stops it.
    assert!(!crate::warm::warm(&pages, &store, store.generation() + 1, &some, None).await);
}

/// A render that panics costs the warm-up that page, not the pages after it; its place and its
/// key are free again, and the log names it. (On 2026-09-26 one ended the warm-up after a few
/// hundred pages, and no later snapshot was warmed until a restart.)
#[tokio::test(flavor = "multi_thread")]
async fn the_warm_up_goes_on_after_a_render_that_panics() {
    use futures_util::StreamExt;

    let store = store_with("warm-panic", &snapshot_file());
    let state = state(store.clone());
    let page = |text: &'static str| move || async move { axum::response::Html(text) };
    // Half a page, then a panic, where the render panicked: at the end of its stream, when its
    // reactive owner was cleaned up.
    let panics = || async {
        let half = futures_util::stream::iter([Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"<!DOCTYPE html><p>half a page"))]);
        let end = futures_util::stream::poll_fn(|_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> { panic!("a render that panics") });
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], Body::from_stream(half.chain(end)))
    };
    let pages = Router::new()
        .route("/a", get(page("<p>a</p>")))
        .route("/b", get(panics))
        .route("/c", get(page("<p>c</p>")))
        .layer(axum::middleware::from_fn_with_state(state.clone(), crate::cache::html_cache))
        .with_state(state.clone());
    let paths = ["/a", "/b", "/c"].map(String::from);
    let log = Captured::default();
    let logging = log.start();
    assert!(crate::warm::warm(&pages, &store, store.generation(), &paths, None).await);
    // Neither its place nor its key is held: asked for again, it is rendered again (and panics).
    let again = tokio::time::timeout(std::time::Duration::from_secs(10), crate::warm::warm(&pages, &store, store.generation(), &paths[1..2], None)).await;
    drop(logging);
    assert_eq!(again.ok(), Some(true));
    assert_eq!(state.cache.size().0, 2, "the pages before and after it are in the cache");
    let log = log.text();
    assert_eq!(log.matches("cache.warm_page_failed").count(), 2, "{log}");
    assert!(log.contains("path=/b") && log.contains("a render that panics") && log.contains("rendered=2") && log.contains("failed=1"), "{log}");
}

/// While working on the site (`--live-assets`) the stylesheet, the scripts and the SVGs come from
/// disk as they are: an edit is there with the next request, even under the address of the build
/// that a page links, the service worker keeps nothing, and no file is read but those the server
/// serves anyway.
#[tokio::test(flavor = "multi_thread")]
async fn live_assets_come_from_disk() {
    let dir = temp_dir("live-assets");
    std::fs::create_dir_all(dir.join("birch")).unwrap();
    std::fs::write(dir.join("app.css"), "body { color: red; }\n").unwrap();
    std::fs::write(dir.join("boot.js"), "const SCHEMA = __SCHEMA__;\n").unwrap();
    std::fs::write(dir.join("birch/roots.svg"), "<svg/>").unwrap();
    std::fs::write(dir.join("secret.txt"), "not an asset").unwrap();
    let mut live = state(SnapshotStore::new(temp_dir("live-assets-data")).unwrap());
    live.live_assets = Some(dir.clone());
    let router = crate::router(live);

    let (status, headers, body) = request(&router, "/assets/app.css?v=test", &[("accept-encoding", "br, gzip")]).await;
    assert_eq!((status, body.as_slice()), (StatusCode::OK, &b"body { color: red; }\n"[..]), "as it is on disk");
    assert!(headers.get(header::CONTENT_ENCODING).is_none(), "not compressed: it goes to this machine");
    assert_eq!(headers[header::CACHE_CONTROL], crate::cache::REVALIDATE, "not immutable under the build's address");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(request(&router, "/assets/app.css?v=test", &[("if-none-match", &etag)]).await.0, StatusCode::NOT_MODIFIED);
    std::fs::write(dir.join("app.css"), "body { color: green; }\n").unwrap();
    let (status, headers, body) = request(&router, "/assets/app.css?v=test", &[("if-none-match", &etag)]).await;
    assert_eq!((status, body.as_slice()), (StatusCode::OK, &b"body { color: green; }\n"[..]), "an edit is there with the next request");
    assert_ne!(headers[header::ETAG].to_str().unwrap(), etag);
    // What is not read from disk is not kept as immutable either: the browser app is built again
    // under the same address.
    let (_, headers, _) = request(&router, &format!("{}?v=test", folia_app::FONT), &[]).await;
    assert_eq!(headers[header::CACHE_CONTROL], crate::cache::REVALIDATE);

    let (_, _, boot) = request(&router, "/assets/boot.js?v=test", &[]).await;
    assert_eq!(String::from_utf8(boot).unwrap(), format!("const SCHEMA = {};\n", folia_model::SCHEMA_VERSION));
    let (_, _, worker) = request(&router, folia_app::SERVICE_WORKER, &[]).await;
    assert_eq!(worker, crate::api::LIVE_SERVICE_WORKER.as_bytes(), "the worker keeps nothing");
    assert_eq!(request(&router, "/assets/birch/roots.svg", &[]).await.2, b"<svg/>");
    // Nothing but the files of the server, and one of them missing on disk is not found.
    for path in ["/assets/birch/..%2Fsecret.txt", "/assets/birch/secret.txt", "/assets/enhance.js"] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::NOT_FOUND, "{path}");
    }
}

/// The icons of a page point into one sprite, served once, with the build of the page.
#[tokio::test(flavor = "multi_thread")]
async fn icons_point_into_the_sprite() {
    let router = crate::router(state(store_with("icons", &snapshot_file())));
    let (_, _, body) = request(&router, "/catalog", &[]).await;
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("<use href=\"/assets/icons.svg?v=test#"), "icons link the sprite of the build");
    assert!(!html.contains("<path d=\"M20 6 9 17l-5-5\"/>"), "no icon carries its path data any more");
    assert!(!html.contains("<datalist"), "no list of every person on the catalog");
    let (status, headers, sprite) = request(&router, "/assets/icons.svg?v=test", &[]).await;
    let sprite = String::from_utf8(sprite).unwrap();
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/svg+xml"));
    assert!(sprite.contains("<symbol id=\"check\" viewBox=\"0 0 24 24\"><path d=\"M20 6 9 17l-5-5\"/></symbol>"));
    let (_, _, worker) = request(&router, folia_app::SERVICE_WORKER, &[]).await;
    assert!(String::from_utf8(worker).unwrap().contains("/assets/icons.svg"), "the service worker keeps the sprite");
}

/// `/api/db` hands every browser the same compressed bytes from memory.
#[tokio::test(flavor = "multi_thread")]
async fn the_catalog_download_comes_from_memory() {
    // A restart finds the compressed copies next to the snapshot, as the download and
    // `snapshot::compress_active` left them.
    let dir = temp_dir("db-memory");
    let raw = std::fs::read(snapshot_file()).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("catalog-test.db"), &raw).unwrap();
    std::fs::write(dir.join("catalog-test.db.gz"), crate::encoding::gzip(&raw)).unwrap();
    std::fs::write(dir.join("catalog-test.db.br"), crate::encoding::brotli(&raw, 1)).unwrap();
    std::fs::write(dir.join("current.json"), r#"{"file":"catalog-test.db","etag":"\"test\""}"#).unwrap();
    let store = SnapshotStore::new(dir).unwrap();
    assert!(store.restore());
    let router = crate::router(state(store.clone()));
    let snapshot = store.current().unwrap();
    let kept = snapshot.gzip_bytes.clone().expect("the compressed snapshot is kept in memory");
    let brotli = snapshot.brotli.get().cloned().expect("its brotli too");
    let (status, headers, body) = request(&router, "/api/db", &[("accept-encoding", "gzip, br")]).await;
    assert_eq!((status, headers[header::CONTENT_ENCODING].to_str().unwrap()), (StatusCode::OK, "br"));
    assert_eq!(body, brotli.to_vec());
    let (status, headers, body) = request(&router, "/api/db", &[("accept-encoding", "gzip")]).await;
    assert_eq!((status, headers[header::CONTENT_ENCODING].to_str().unwrap()), (StatusCode::OK, "gzip"));
    assert_eq!(body, kept.to_vec());
    let (_, headers, body) = request(&router, "/api/db", &[]).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(body.len() as u64, snapshot.bytes);
    // A client that takes brotli but not gzip, before the brotli is made: the file as it is.
    let bare = store_with("db-bare", &snapshot_file());
    let router = crate::router(state(bare.clone()));
    let (_, headers, body) = request(&router, "/api/db", &[("accept-encoding", "br")]).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(body.len() as u64, bare.current().unwrap().bytes);
}

/// The addresses a page writes into its HTML (`href`, `action`, `src`, `content` of a URL), each
/// with whether its tag names another language (`hreflang`: the language switch and the
/// alternates in the head).
fn addresses(html: &str) -> Vec<(String, bool)> {
    let mut found = Vec::new();
    for tag in html.split('<').skip(1).map(|rest| rest.split('>').next().unwrap_or_default()) {
        let other_language = tag.contains(" hreflang=");
        for attribute in [" href=\"", " action=\"", " src=\""] {
            let mut rest = tag;
            while let Some(at) = rest.find(attribute) {
                rest = &rest[at + attribute.len()..];
                let value = rest.split('"').next().unwrap_or_default();
                found.push((value.replace("&amp;", "&"), other_language));
            }
        }
    }
    found
}

/// Whether `address`, written into a page in English, leads to an English page or to what has
/// no language (a file, the site's API, another site, a place on the same page). Only a path of
/// this site can lead into German; what is no such path (another site, the synthetic snapshot's
/// source addresses, which are bare numbers) is not the page's to say.
fn stays_in_english(address: &str) -> bool {
    let in_english = address == "/en" || ["/en/", "/en?", "/en#"].iter().any(|prefix| address.starts_with(prefix));
    let no_language = ["/assets/", "/pkg/", "/api/", "/favicon", "/apple-touch-icon", "/sw.js"].iter().any(|prefix| address.starts_with(prefix));
    let site_path = address.starts_with('/') && !address.starts_with("//");
    in_english || no_language || !site_path
}

/// Every page in English (`/en/…`): the document says so, every address it writes leads to an
/// English page or to what has no language (only the switch and the alternates in the head name
/// another language), it names the same page in every language, and the frame speaks English.
/// The German page of the same address keeps its plain links. `/de/…` leads to the plain address.
#[tokio::test(flavor = "multi_thread")]
async fn a_page_in_english_stays_in_english() {
    let store = store_with("english", &snapshot_file());
    let router = crate::router(state(store.clone()));
    let (_, _, overview) = request(&router, "/programs", &[]).await;
    let overview = String::from_utf8(overview).unwrap();
    let slug = overview.split("href=\"/programs/").nth(1).and_then(|rest| rest.split(['/', '"', '?']).next()).unwrap().to_string();
    let (_, _, catalog) = request(&router, "/catalog", &[]).await;
    let catalog = String::from_utf8(catalog).unwrap();
    let module = catalog.split("href=\"/catalog/module/").nth(1).and_then(|rest| rest.split(['"', '?']).next()).unwrap().to_string();
    let pages = [
        "/".to_string(),
        "/catalog".to_string(),
        "/catalog?turnus=winter&form=lecture&sort=title".to_string(),
        format!("/catalog/module/{module}"),
        "/programs".to_string(),
        "/programs?level=master".to_string(),
        format!("/programs/{slug}/plan"),
        format!("/programs/{slug}/areas"),
        format!("/programs/{slug}/my-plan"),
        "/bookmarks".to_string(),
        "/studyplan".to_string(),
        folia_routes::url::IMPRINT.to_string(),
        folia_routes::url::PRIVACY.to_string(),
    ];
    // Every page is checked before the test fails, so that it lists what is left.
    let mut problems = Vec::new();
    for page in &pages {
        let english = folia_locale::Locale::En.path(page);
        let (status, _, body) = request(&router, &english, &[]).await;
        let html = String::from_utf8(body).unwrap();
        assert_eq!(status, StatusCode::OK, "{english}");
        assert!(html.contains("<html lang=\"en\""), "{english}: the document's language");
        for (address, other_language) in addresses(&html) {
            if !other_language && !stays_in_english(&address) {
                problems.push(format!("{english} links {address} out of English"));
            }
        }
        let head = html.split("</head>").next().unwrap_or_default();
        for (language, address) in [("de", page.clone()), ("en", english.clone()), ("x-default", page.clone())] {
            // (leptos_meta writes the attributes of a link in its own order.)
            let alternate = format!("href=\"https://catalog.example{}\" hreflang=\"{language}\"", address.replace('&', "&amp;"));
            if !head.contains(&alternate) {
                problems.push(format!("{english}: no {alternate}"));
            }
        }
        for german in ["Zum Inhalt springen", "Hauptnavigation", "Modulkatalog · inoffiziell", "Rechtliches"] {
            if html.contains(german) {
                problems.push(format!("{english} says „{german}“"));
            }
        }
        if !html.contains("Skip to content") || !html.contains("href=\"/en/catalog\"") {
            problems.push(format!("{english}: not the English frame"));
        }

        // The same page in German: the links it writes are the plain ones.
        let (status, _, body) = request(&router, page, &[]).await;
        let html = String::from_utf8(body).unwrap();
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(html.contains("<html lang=\"de\"") && html.contains("Zum Inhalt springen"), "{page}");
        for (address, other_language) in addresses(&html) {
            if !other_language && (address == "/en" || address.starts_with("/en/") || address.starts_with("/en?")) {
                problems.push(format!("{page} links {address} into English"));
            }
        }
    }
    problems.dedup();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // A page that does not exist does not exist in English either.
    let (status, _, body) = request(&router, "/en/no-such-page", &[]).await;
    assert!(status == StatusCode::NOT_FOUND && String::from_utf8(body).unwrap().contains("Page not found"));
    // German has no prefix: `/de/…` is the plain address, the query kept; `/en/` is `/en`.
    for (asked, target) in [("/de", "/"), ("/de/", "/"), ("/de/catalog?turnus=winter", "/catalog?turnus=winter"), ("/en/", "/en")] {
        let (status, headers, _) = request(&router, asked, &[]).await;
        assert_eq!((status, headers[header::LOCATION].to_str().unwrap()), (StatusCode::PERMANENT_REDIRECT, target), "{asked}");
    }
    // The installed app of each language starts in it.
    let (status, _, body) = request(&router, "/en/manifest.webmanifest", &[]).await;
    let manifest: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!((status, manifest["lang"].as_str(), manifest["start_url"].as_str()), (StatusCode::OK, Some("en"), Some("/en")));
    let (_, _, body) = request(&router, folia_app::MANIFEST, &[]).await;
    let manifest: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!((manifest["lang"].as_str(), manifest["start_url"].as_str()), (Some("de"), Some("/")));
    // The sitemap names every page in every language, each with its alternates.
    let (_, _, body) = request(&router, "/sitemap.xml", &[]).await;
    let sitemap = String::from_utf8(body).unwrap();
    assert!(sitemap.contains("<loc>https://catalog.example/en</loc>") && sitemap.contains(&format!("<loc>https://catalog.example/en/catalog/module/{module}</loc>")));
    assert!(sitemap.contains(&format!("<xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://catalog.example/catalog/module/{module}\"/>")));
    // A card in English is drawn apart from the German one.
    let (status, headers, card) = request(&router, &format!("/en/cards/module/{module}.png"), &[]).await;
    assert_eq!((status, headers[header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "image/png"));
    let (_, _, german) = request(&router, &format!("/cards/module/{module}.png"), &[]).await;
    assert_ne!(card, german, "the card says „Modul“ in German and \"Module\" in English");
    let (status, _, picture) = request(&router, "/en/assets/og.png", &[]).await;
    let (_, _, german) = request(&router, folia_app::OG_IMAGE, &[]).await;
    assert!(status == StatusCode::OK && picture.starts_with(b"\x89PNG") && picture != german, "the standard picture in English");
}
