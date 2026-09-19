//! Web server of the BTU catalog.
//!
//! It knows the scraper only through its HTTP snapshot endpoint (docs/operations.md §1),
//! renders the app's pages from the active snapshot, caches them until the next snapshot,
//! and hands the snapshot file on to browsers as `/api/db`.
//!
//! Logging follows docs/operations.md §2: stable `event` names, ERROR = a human has to act.

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

mod api;
mod cache;
mod config;
mod snapshot;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use app::data::{CatalogSource, Source};
use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use catalog::{Database, DbError};
use clap::Parser;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};

use crate::cache::HtmlCache;
use crate::config::Config;
use crate::snapshot::SnapshotStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<SnapshotStore>,
    pub cache: Arc<HtmlCache>,
    /// Changes with every start of the process, so browsers drop pages and assets of an older build.
    pub build_id: Arc<str>,
    pub stale_after: Option<Duration>,
    pub leptos: LeptosOptions,
}

impl axum::extract::FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos.clone()
    }
}

/// Pages query whatever snapshot is active when they start rendering.
struct ActiveSnapshot(Arc<SnapshotStore>);

impl CatalogSource for ActiveSnapshot {
    fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
        match self.0.current() {
            Some(snapshot) => snapshot.with_db(job),
            None => Err(DbError::Unavailable("no snapshot has been loaded yet".to_string())),
        }
    }
}

fn init_logging(config: &Config) {
    let filter = tracing_subscriber::EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(false);
    if config.log_format.eq_ignore_ascii_case("json") {
        builder.json().flatten_event(true).init();
    } else {
        builder.init();
    }
}

async fn access_log(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("referrer-policy", HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));

    let status = response.status().as_u16();
    let cache = response.headers().get("x-cache").and_then(|v| v.to_str().ok()).unwrap_or("-").to_string();
    let bytes = response.headers().get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    if status >= 500 {
        tracing::error!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request failed");
    } else {
        tracing::info!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request");
    }
    response
}

pub fn router(state: AppState) -> Router {
    let source = Source(Arc::new(ActiveSnapshot(state.store.clone())));
    let routes = generate_route_list(app::App);
    let options = state.leptos.clone();

    let pages = Router::new()
        .leptos_routes_with_context(
            &state,
            routes,
            {
                let source = source.clone();
                move || provide_context(source.clone())
            },
            {
                let options = options.clone();
                move || app::shell(options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler_with_context::<AppState, _>(
            move || provide_context(source.clone()),
            app::shell,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), cache::html_cache));

    Router::new()
        .route("/api/db", get(api::database))
        .route("/api/status", get(api::status))
        .route("/healthz", get(api::health))
        .route(app::STYLESHEET, get(api::stylesheet))
        .route(app::FAVICON, get(api::favicon))
        .route(app::FONT, get(api::font))
        .route(app::ENHANCE_SCRIPT, get(api::enhance_script))
        .route("/favicon.ico", get(api::favicon))
        .route("/robots.txt", get(api::robots))
        .merge(pages)
        .layer(middleware::from_fn(access_log))
        .with_state(state)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!(component = "server", event = "server.shutdown", "shutting down");
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let config = Config::parse();
    init_logging(&config);

    let store = match SnapshotStore::new(config.data_dir.clone()) {
        Ok(store) => store,
        Err(error) => {
            tracing::error!(component = "server", event = "server.start_failed", data_dir = %config.data_dir.display(), error = %error, "cannot use the data directory");
            return std::process::ExitCode::FAILURE;
        }
    };
    store.restore();
    tokio::spawn(snapshot::run(store.clone(), config.snapshot_url.clone(), config.poll_interval(), config.stale_after()));

    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let state = AppState {
        store,
        cache: Arc::new(HtmlCache::new(config.html_cache_mb * 1024 * 1024)),
        build_id: format!("{}-{started_at:x}", env!("CARGO_PKG_VERSION")).into(),
        stale_after: config.stale_after(),
        leptos: LeptosOptions::builder()
            .output_name("btu-app")
            .site_root(config.site_root.to_string_lossy().into_owned())
            .site_pkg_dir("pkg")
            .site_addr(config.addr)
            .build(),
    };

    let listener = match tokio::net::TcpListener::bind(config.addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(component = "server", event = "server.start_failed", addr = %config.addr, error = %error, "cannot listen");
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!(component = "server", event = "server.listening", addr = %config.addr, snapshot_url = %config.snapshot_url, "web server started");

    match axum::serve(listener, router(state)).with_graceful_shutdown(shutdown_signal()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(component = "server", event = "server.failed", error = %error, "server stopped");
            std::process::ExitCode::FAILURE
        }
    }
}
