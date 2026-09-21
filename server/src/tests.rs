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

/// Closed testing (`access`): nothing but the login page and what it needs answers without the
/// password; with it the site is what it was. Needs no snapshot.
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

    // Open stays what the login page, a home screen and a supervisor need; crawlers are sent away.
    for path in [app::STYLESHEET, "/assets/app.css?v=test", app::FONT, app::FAVICON, app::FAVICON_ICO, app::TOUCH_ICON, app::ICON_192, app::MANIFEST] {
        assert_eq!(request(&router, path, &[]).await.0, StatusCode::OK, "{path}");
    }
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE, "answered by the health check (no snapshot here), not by the gate");
    // The container's own probe (`folia healthcheck`) has no password and needs no snapshot.
    let (status, headers, body) = request(&router, crate::api::LIVENESS, &[]).await;
    assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap(), body.as_slice()), (StatusCode::OK, "no-store", &b"ok\n"[..]));
    let (status, _, robots) = request(&router, "/robots.txt", &[]).await;
    assert_eq!((status, String::from_utf8(robots).unwrap().as_str()), (StatusCode::OK, "User-agent: *\nDisallow: /\n"));

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
    let (_, _, body) = request(&router, "/", &[]).await;
    let home = String::from_utf8(body).unwrap();
    assert!(head(&home).contains("href=\"https://catalog.example/\" rel=\"canonical\"") && !head(&home).contains("noindex"));
    assert!(home.contains("class=\"map map-wide\"") && home.contains("class=\"map map-tall\""), "the landing page draws the map the snapshot was opened with");

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

    // A broken export must not replace a good snapshot.
    *radix.current.lock().unwrap() = Some(("\"bbbb2222\"".to_string(), b"this is not a database".to_vec()));
    assert!(matches!(sync_once(&store, &client, &url).await, Err(SyncError::Rejected(_))));
    assert_eq!(store.current().unwrap().etag, "\"aaaa1111\"");
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
