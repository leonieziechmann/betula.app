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
    for link in ["rel=\"manifest\"", "rel=\"apple-touch-icon\"", "href=\"/favicon.ico\"", "name=\"theme-color\""] {
        assert_eq!(head(&home).matches(link).count(), 1, "{link}");
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

    // The map of the programs is laid out once per snapshot and handed on as it is.
    let (status, headers, body) = request(&router, "/api/map.json", &[]).await;
    assert_eq!((status, headers[header::ETAG].to_str().unwrap()), (StatusCode::OK, "\"aaaa1111\""));
    let map: catalog::graph::ProgramMap = serde_json::from_slice(&body).unwrap();
    assert_eq!(Some(&map), active.program_map.as_ref().map(|(map, ..)| map.as_ref()));
    assert!(map.programs.len() > 100 && map.wide.dots.len() == map.programs.len() && map.tall.dots.len() == map.programs.len());
    assert_eq!(request(&router, "/api/map.json", &[("if-none-match", "\"aaaa1111\"")]).await.0, StatusCode::NOT_MODIFIED);
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
