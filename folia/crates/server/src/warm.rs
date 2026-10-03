//! The cache warmed up: after a snapshot is activated (and after a start), every page of the
//! sitemap is rendered into the cache, one at a time and only while the server has nothing else
//! to do (`busy::Places::enter_idle`), so that search engines, link previews and visitors find
//! them rendered — after a deploy and a restart too, when the cache is empty and a crawler
//! walking the sitemap would otherwise meet a render on every page. Measured 2026-09-26: 5,235
//! pages, 26 s of one processor of the workstation (about a minute on the server), 36 MiB of the
//! cache. A newer snapshot starts it over; `--warm-cache off` leaves it out.
//!
//! On its way it notes what each page says, for the dates of the sitemap (`lastmod`).
//!
//! And after a start, the files every page asks for are compressed before the first visitor asks
//! for them (`files`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::http::{header, Request};
use axum::Router;
use tower::ServiceExt;

use crate::cache::WarmUp;
use crate::lastmod::Changes;
use crate::snapshot::SnapshotStore;

/// Watches for new snapshots and warms the cache with each.
pub async fn run(pages: Router, store: Arc<SnapshotStore>, changes: Option<Arc<Changes>>) {
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
        if warm(&pages, &store, generation, &paths, changes.as_deref()).await {
            warmed = generation;
            if let Some(changes) = &changes {
                if let Err(error) = changes.finish(&paths) {
                    tracing::warn!(component = "cache", event = "lastmod.write_failed", error = %error, "the dates of the sitemap could not be written; they are kept until the next start");
                }
            }
        }
    }
}

/// Renders `paths` into the cache of `generation`, and notes what each page says in `changes`, as
/// of the snapshot's `data_changed_at`. False when a newer snapshot came in between.
pub async fn warm(pages: &Router, store: &SnapshotStore, generation: u64, paths: &[String], changes: Option<&Changes>) -> bool {
    let started = Instant::now();
    let since = store.current().and_then(|snapshot| snapshot.meta.data_changed_at.clone());
    let (mut rendered, mut kept, mut other, mut changed, mut failed) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for path in paths {
        loop {
            if store.generation() != generation {
                return false;
            }
            // Without an `Accept-Encoding`: the page as it was rendered, which `changes` reads.
            let Ok(request) = Request::builder().uri(path.as_str()).header(header::ACCEPT, "text/html").extension(WarmUp).body(Body::empty()) else {
                break;
            };
            // Each page in a task of its own: a render that panics costs this page, not the rest
            // of the warm-up. On 2026-09-26 one ended it after a few hundred pages, and no later
            // snapshot was warmed until a restart (a value of the browser's in `folia_app::pending`,
            // dropped on another thread). A page lost so keeps the date it had (`Changes::finish`).
            let (state, page) = match tokio::spawn(ask(pages.clone(), request)).await {
                Ok(answer) => answer,
                Err(error) => {
                    failed += 1;
                    tracing::error!(component = "cache", event = "cache.warm_page_failed", generation, path = %path, error = %error, "a page of the sitemap failed to render and is left out of the warm-up");
                    break;
                }
            };
            if let (Some(html), Some(changes), Some(since)) = (page, changes, since.as_deref()) {
                if changes.note(path, &html, since) {
                    changed += 1;
                }
            }
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
        changed,
        failed,
        ms = started.elapsed().as_millis() as u64,
        "the pages of the sitemap are in the cache"
    );
    true
}

/// Asks the pages for one of them and reads it to the end (the cache takes it on the way): what
/// the cache says it did (`x-cache`), and the page when it is one (`miss` or `hit`).
async fn ask(pages: Router, request: Request<Body>) -> (String, Option<Bytes>) {
    let Ok(response) = pages.oneshot(request).await;
    let state = response.headers().get("x-cache").and_then(|value| value.to_str().ok()).unwrap_or_default().to_string();
    let page = response.status().is_success() && matches!(state.as_str(), "miss" | "hit") && !response.headers().contains_key(header::CONTENT_ENCODING);
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024).await;
    (state, body.ok().filter(|_| page))
}

/// The masks of the birch a stylesheet names (`/assets/birch/<name>.svg`), quoted or not: the
/// minified one writes `url(/assets/birch/…)`.
pub fn masks(stylesheet: &str) -> impl Iterator<Item = &str> {
    stylesheet.match_indices("/assets/birch/").filter_map(|(at, _)| stylesheet.get(at..)?.split(['"', '\'', ')']).next())
}

/// The files every page asks for, in brotli before the first visitor asks for them: made once per
/// process (`encoding::Kept`), and the larger ones take a while at brotli's best — the stylesheet
/// 0.4 s, sql.js's WASM 1.1 s, the bundle 2 s — which the first request after a start would wait
/// for. One after the other, in the background; what a visitor asks for meanwhile is made for
/// them, once. With the build in the address, as a page links them: so it is the address the
/// browsers keep. Last the files of the semantic search, which a page loads once the app runs, and
/// its model (`model`, 15 MB, 1.8 s), whose address names its content and not the build.
pub async fn files(router: Router, build: Arc<str>, model: Option<String>) {
    let started = Instant::now();
    let masks: std::collections::BTreeSet<&str> = masks(crate::assets::text("app.css")).collect();
    let built = [folia_app::STYLESHEET, folia_app::icons::SPRITE, folia_app::ENHANCE_SCRIPT, folia_app::BOOT_SCRIPT, "/assets/sql-wasm.js", "/assets/sql-wasm.wasm", "/pkg/folia_client.js", "/pkg/folia_client_bg.wasm", "/pkg/data-worker.js"];
    let semantic = ["/pkg/semantic.js", "/pkg/semantic-worker.js", "/pkg/semantic.simd.wasm", "/pkg/semantic.relaxed.wasm"];
    let paths: Vec<String> = built
        .iter()
        .map(|path| format!("{path}?v={build}"))
        .chain(masks.iter().map(|path| path.to_string()))
        .chain(model.iter().flat_map(|model| semantic.iter().map(|path| format!("{path}?v={build}")).chain([model.clone()])))
        .collect();
    for path in &paths {
        let Ok(request) = Request::builder().uri(path.as_str()).header(header::ACCEPT_ENCODING, "br").body(Body::empty()) else { continue };
        let Ok(response) = router.clone().oneshot(request).await;
        drop(response);
    }
    tracing::info!(component = "http", event = "files.compressed", files = paths.len(), ms = started.elapsed().as_millis() as u64, "the files of the site are ready in brotli");
}
