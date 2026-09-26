//! The cache warmed up: after a snapshot is activated (and after a start), every page of the
//! sitemap is rendered into the cache, one at a time and only while the server has nothing else
//! to do (`busy::Places::enter_idle`), so that search engines, link previews and visitors find
//! them rendered — after a deploy and a restart too, when the cache is empty and a crawler
//! walking the sitemap would otherwise meet a render on every page. Measured 2026-09-26: 5,235
//! pages, 26 s of one processor of the workstation (about a minute on the server), 36 MiB of the
//! cache. A newer snapshot starts it over; `--warm-cache off` leaves it out.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Request};
use axum::Router;
use tower::ServiceExt;

use crate::cache::WarmUp;
use crate::snapshot::SnapshotStore;

/// Watches for new snapshots and warms the cache with each.
pub async fn run(pages: Router, store: Arc<SnapshotStore>) {
    let mut warmed = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let generation = store.generation();
        if generation == warmed {
            continue;
        }
        let Some(snapshot) = store.current() else { continue };
        let paths = match tokio::task::spawn_blocking(move || crate::api::sitemap_paths(&snapshot)).await {
            Ok(Ok(paths)) => paths,
            Ok(Err(error)) => {
                tracing::warn!(component = "cache", event = "cache.warm_failed", error = %error, "the pages of the sitemap could not be listed; the cache fills as they are asked for");
                warmed = generation;
                continue;
            }
            Err(_) => continue,
        };
        if warm(&pages, &store, generation, &paths).await {
            warmed = generation;
        }
    }
}

/// Renders `paths` into the cache of `generation`. False when a newer snapshot came in between.
pub async fn warm(pages: &Router, store: &SnapshotStore, generation: u64, paths: &[String]) -> bool {
    let started = Instant::now();
    let (mut rendered, mut kept, mut other) = (0usize, 0usize, 0usize);
    for path in paths {
        loop {
            if store.generation() != generation {
                return false;
            }
            let Ok(request) = Request::builder().uri(path.as_str()).header(header::ACCEPT, "text/html").header(header::ACCEPT_ENCODING, "gzip").extension(WarmUp).body(Body::empty()) else {
                break;
            };
            let Ok(response) = pages.clone().oneshot(request).await;
            let state = response.headers().get("x-cache").and_then(|value| value.to_str().ok()).unwrap_or_default().to_string();
            let _ = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024).await;
            match state.as_str() {
                // Visitors first: try again once the server is idle.
                crate::busy::BUSY => tokio::time::sleep(Duration::from_millis(200)).await,
                "miss" => {
                    rendered += 1;
                    break;
                }
                "hit" => {
                    kept += 1;
                    break;
                }
                _ => {
                    other += 1;
                    break;
                }
            }
        }
    }
    tracing::info!(
        component = "cache",
        event = "cache.warmed",
        generation,
        pages = paths.len(),
        rendered,
        kept,
        other,
        ms = started.elapsed().as_millis() as u64,
        "the pages of the sitemap are in the cache"
    );
    true
}
