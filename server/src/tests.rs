//! The server against a fake scraper that serves a real snapshot over HTTP, the way the
//! real one does (`ETag`, `If-None-Match` → 304). Needs a snapshot like the tests of the
//! `catalog` crate (`BTU_TEST_SNAPSHOT`, else `snapshot/current.json`).

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
    if let Ok(path) = std::env::var("BTU_TEST_SNAPSHOT") {
        return PathBuf::from(path);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("snapshot");
    let pointer = std::fs::read_to_string(dir.join("current.json"))
        .unwrap_or_else(|e| panic!("no catalog snapshot for the tests ({e}); run `scraper export` or set BTU_TEST_SNAPSHOT"));
    let pointer: serde_json::Value = serde_json::from_str(&pointer).unwrap();
    dir.join(pointer["file"].as_str().unwrap())
}

type Served = Arc<std::sync::Mutex<Option<(String, Vec<u8>)>>>;

/// What the fake scraper serves next: (ETag, body).
#[derive(Clone)]
struct Scraper {
    current: Served,
    downloads: Arc<AtomicUsize>,
}

async fn serve_snapshot(State(scraper): State<Scraper>, headers: HeaderMap) -> Response {
    let Some((etag, body)) = scraper.current.lock().unwrap().clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, "5")]).into_response();
    };
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    scraper.downloads.fetch_add(1, Ordering::SeqCst);
    ([(header::ETAG, etag)], body).into_response()
}

async fn start_scraper() -> (Scraper, String) {
    let scraper = Scraper { current: Arc::default(), downloads: Arc::default() };
    let app = Router::new().route("/snapshot/catalog.db", get(serve_snapshot)).with_state(scraper.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/snapshot/catalog.db", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (scraper, url)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("btu-server-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn state(store: Arc<SnapshotStore>) -> AppState {
    AppState {
        store,
        cache: Arc::new(HtmlCache::new(32 * 1024 * 1024)),
        build_id: "test".into(),
        stale_after: None,
        site_root: "no-site".into(),
        packages: Arc::default(),
        leptos: LeptosOptions::builder().output_name("btu-app").site_root("no-site").build(),
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
    let (scraper, url) = start_scraper().await;
    let data_dir = temp_dir("sync");
    let store = SnapshotStore::new(data_dir.clone()).unwrap();
    let client = reqwest::Client::new();
    let router = crate::router(state(store.clone()));

    // The scraper has not exported anything yet: the site says so, and nothing is cached.
    assert!(matches!(sync_once(&store, &client, &url).await, Err(SyncError::NotReady)));
    let (status, headers, _) = request(&router, "/catalog", &[]).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(request(&router, "/api/db", &[]).await.0, StatusCode::SERVICE_UNAVAILABLE);

    // First export: downloaded, checked, compressed, active.
    *scraper.current.lock().unwrap() = Some(("\"aaaa1111\"".to_string(), real.clone()));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let active = store.current().expect("active snapshot");
    assert_eq!((active.etag.as_str(), active.bytes), ("\"aaaa1111\"", real.len() as u64));
    assert!(active.gzip.as_ref().is_some_and(|(_, bytes)| *bytes < active.bytes / 3), "the browser download is compressed once");
    assert_eq!(request(&router, "/healthz", &[]).await.0, StatusCode::OK);

    // Nothing new: a conditional request, no download.
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Unchanged)));
    assert_eq!(scraper.downloads.load(Ordering::SeqCst), 1);

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

    // Unknown things are 404 and never cached.
    for path in ["/catalog/module/00000", "/programs/no-such-program", "/no-such-page"] {
        let (status, headers, _) = request(&router, path, &[]).await;
        assert_eq!((status, headers[header::CACHE_CONTROL].to_str().unwrap()), (StatusCode::NOT_FOUND, "no-store"), "{path}");
    }

    // Browsers get the file with the scraper's ETag, compressed, and a 304 when they have it.
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
    *scraper.current.lock().unwrap() = Some(("\"bbbb2222\"".to_string(), b"this is not a database".to_vec()));
    assert!(matches!(sync_once(&store, &client, &url).await, Err(SyncError::Rejected(_))));
    assert_eq!(store.current().unwrap().etag, "\"aaaa1111\"");
    assert_eq!(request(&router, "/catalog", &[]).await.0, StatusCode::OK);

    // A new good export starts a new generation: old pages are gone, old ETags no longer match.
    *scraper.current.lock().unwrap() = Some(("\"cccc3333\"".to_string(), real.clone()));
    assert!(matches!(sync_once(&store, &client, &url).await, Ok(Sync::Activated)));
    let (status, headers, _) = request(&router, "/catalog?turnus=winter&form=exercise&status=all", &[("if-none-match", &etag)]).await;
    assert_eq!((status, headers["x-cache"].to_str().unwrap()), (StatusCode::OK, "miss"));

    // After a restart the last good snapshot is served even if the scraper is down.
    drop(router);
    let restarted = SnapshotStore::new(data_dir.clone()).unwrap();
    assert!(restarted.restore());
    assert_eq!(restarted.current().unwrap().etag, "\"cccc3333\"");
    let leftovers: Vec<String> = std::fs::read_dir(&data_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(leftovers.iter().all(|name| !name.starts_with("download-")), "{leftovers:?}");
}
