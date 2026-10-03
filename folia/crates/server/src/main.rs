//! Folia, the web server of Betula.
//!
//! It knows Radix only through its HTTP snapshot endpoint (docs/radix/operations.md §1),
//! renders the app's pages from the active snapshot, caches them until the next snapshot,
//! and hands the snapshot file on to browsers as `/api/db`.
//!
//! Logging follows docs/radix/operations.md §2: stable `event` names, ERROR = a human has to act.

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

mod access;
mod api;
mod assets;
mod birch;
mod busy;
mod cache;
mod cards;
mod config;
mod encoding;
mod lastmod;
mod launch;
mod logo;
mod semantic;
mod snapshot;
mod texts;
#[cfg(test)]
mod tests;
mod warm;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use app::data::{CatalogSource, Source};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use catalog::{Database, DbError};
use clap::Parser;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, AxumRouteListing, LeptosRoutes};

use crate::cache::HtmlCache;
use crate::config::Config;
use crate::snapshot::SnapshotStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<SnapshotStore>,
    pub cache: Arc<HtmlCache>,
    /// The pictures of link previews, one per module and program (`cards`).
    pub cards: Arc<cards::Cards>,
    /// The launch screens of the installed app on iOS (`launch`).
    pub launch: Arc<launch::Launch>,
    /// Changes with every start of the process, so browsers drop pages and assets of an older build.
    pub build_id: Arc<str>,
    pub stale_after: Option<Duration>,
    pub leptos: LeptosOptions,
    /// The address of the site from outside, without a slash at the end (`--public-url`).
    pub public_url: Arc<str>,
    /// Where the built browser app lives (`<site-root>/pkg`).
    pub site_root: std::path::PathBuf,
    /// `folia/assets` while working on the site (`--live-assets`): the minified files are read from
    /// there on every request, as they are (`api::minified`), and nothing is kept as immutable.
    pub live_assets: Option<std::path::PathBuf>,
    /// The files of the browser app as they are served, by name.
    pub packages: Packages,
    /// The browser's model of the semantic search (`--semantic-model`); `None` without one.
    pub semantic: Option<Arc<semantic::Model>>,
    /// Closed testing: the password in front of the whole site (`access`); `None` when it is open.
    pub gate: Option<Arc<access::Gate>>,
    /// Where pages that are not in the cache are rendered, and how long a page waits for a place
    /// before it is answered 503 (`busy`).
    pub renders: Arc<busy::Places>,
    pub render_wait: Duration,
    /// Where calendar feeds are made.
    pub feeds: Arc<busy::Places>,
    /// When each page of the sitemap last changed (`lastmod`), recorded by the warm-up; `None`
    /// without it.
    pub changes: Option<Arc<lastmod::Changes>>,
}

/// The header that names the build of the server on every answer (`AppState::build_id`).
pub const BUILD_HEADER: &str = "x-build";

/// name → the file as it is served
pub type Packages = Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<api::Package>>>>;

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

async fn access_log(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    // A calendar feed's address carries somebody's plan (`/calendar/<code>.ics`), and so does the
    // card of a shared Stundenplan (`/cards/studyplan/<code>.png`): the log, kept 30 days, writes
    // every path under them as one fixed text, valid code or not. (A page's query, where a shared
    // plan's code travels, is never written.)
    let path = catalog::timetable::share::redacted_path(catalog::timetable::subscription::redacted_path(request.uri().path())).to_string();
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    // The build that answered: the service worker keeps a page or a file only when it comes from
    // its own build (`folia/assets/sw.js`), so it never mixes the files of two builds.
    if let Ok(build) = HeaderValue::from_str(&state.build_id) {
        headers.insert(BUILD_HEADER, build);
    }
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("referrer-policy", HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));

    let status = response.status().as_u16();
    let cache = response.headers().get("x-cache").and_then(|v| v.to_str().ok()).unwrap_or("-").to_string();
    let bytes = response.headers().get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    if cache == busy::BUSY {
        // Turned away on purpose, the server being busy (`busy`): no error of the server, but
        // worth seeing when it happens often.
        tracing::warn!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request turned away: the server is busy");
    } else if status >= 500 {
        tracing::error!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request failed");
    } else if path == api::LIVENESS {
        // The container's own probe, twice a minute: not part of the story of a run.
        tracing::debug!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request");
    } else {
        tracing::info!(component = "http", event = "http.request", %method, path, status, ms, cache, bytes, "request");
    }
    response
}

/// The rendered pages with their cache, and nothing in front of them: the site's pages inside
/// `router`, and what the warm-up asks for directly (`warm`), past the gate and the access log.
pub fn pages(state: &AppState) -> Router<AppState> {
    let source = Source(Arc::new(ActiveSnapshot(state.store.clone())));
    let routes = localized_routes(generate_route_list(app::App));
    let options = state.leptos.clone();
    // What every rendered page gets from its host: the data, the name of the site from outside,
    // the build its stylesheet and scripts are linked with, and the map of the programs the
    // active snapshot was opened with.
    let provide = {
        let (store, site, build) = (state.store.clone(), app::seo::SiteUrl(state.public_url.clone()), app::BuildId(state.build_id.clone()));
        move || {
            provide_context(source.clone());
            provide_context(site.clone());
            provide_context(build.clone());
            if let Some(snapshot) = store.current() {
                if let Some((map, ..)) = &snapshot.program_map {
                    provide_context(app::data::ProgramMapHandle(map.clone()));
                }
                if let Some(pickers) = snapshot.pickers.clone() {
                    provide_context(pickers);
                }
                if let Some(programs) = snapshot.programs.clone() {
                    provide_context(programs);
                }
            }
        }
    };

    Router::new()
        .leptos_routes_with_context(
            state,
            routes,
            provide.clone(),
            {
                let options = options.clone();
                move || app::shell(options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler_with_context::<AppState, _>(provide, app::shell))
        .layer(middleware::from_fn_with_state(state.clone(), cache::html_cache))
}

/// The app's routes in every language: as they are for the default language, under its prefix for
/// every other (`/en/catalog`; the start page is `/en`). Each page learns its language from its
/// address (`app::i18n`).
fn localized_routes(routes: Vec<AxumRouteListing>) -> Vec<AxumRouteListing> {
    let mut all = Vec::with_capacity(routes.len() * catalog::Locale::ALL.len());
    for locale in catalog::Locale::ALL.iter().copied().filter(|locale| !locale.prefix().is_empty()) {
        for route in &routes {
            let regenerate: Vec<leptos_router::static_routes::RegenerationFn> = Vec::new();
            all.push(AxumRouteListing::new(locale.path(route.path()), route.mode().clone(), route.methods(), regenerate));
        }
    }
    all.extend(routes);
    all
}

/// `/de/…` is no address of the site (German, the default, has no prefix), but it is the one
/// people guess: it leads to the plain address, with its query. `/en/` leads to `/en`, the one
/// address of the English start page.
async fn language_redirect(uri: axum::http::Uri) -> Response {
    use axum::response::IntoResponse;
    let path = uri.path();
    let target = match path.strip_prefix("/de").filter(|rest| rest.is_empty() || rest.starts_with('/')) {
        Some(rest) => catalog::Locale::default().path(if rest.is_empty() { "/" } else { rest }),
        None => path.trim_end_matches('/').to_string(),
    };
    let target = match uri.query() {
        Some(query) => format!("{target}?{query}"),
        None => target,
    };
    axum::response::Redirect::permanent(&target).into_response()
}

/// What the server draws or writes in the language of its address, as the pages are: the
/// manifest, the cards of link previews and the calendar feed (`/cards/…`, `/en/cards/…`).
fn in_every_language() -> Router<AppState> {
    let mut router = Router::new();
    for locale in catalog::Locale::ALL.iter().copied() {
        let at = |path: &str| locale.path(path);
        router = router
            .route(&at(app::MANIFEST), get(api::manifest))
            .route(&at(app::OG_IMAGE), get(api::og_image))
            .route(&at("/cards/module/{file}"), get(api::module_card))
            .route(&at("/cards/program/{file}"), get(api::program_card))
            .route(&at(app::seo::BOOKMARKS_CARD), get(api::bookmarks_card_png))
            .route(&at(app::seo::STUDYPLAN_CARD), get(api::studyplan_card_png))
            .route(&at("/cards/studyplan/{file}"), get(api::shared_plan_card))
            // A Studienplan as a calendar subscription. No page of the app lives under `/calendar/`
            // (axum refuses two routes for one path at startup).
            .route(&at("/calendar/{file}"), get(api::calendar));
    }
    router
}

/// The files of the site: its stylesheet, scripts, pictures and the browser app, and nothing in
/// front of them: inside `router`, and what the warm-up of their compressed forms asks for
/// directly (`warm::files`), past the gate and the access log.
pub fn files() -> Router<AppState> {
    Router::new()
        .route(app::STYLESHEET, get(api::stylesheet))
        .route(app::icons::SPRITE, get(api::icons))
        .route(app::FAVICON, get(api::favicon))
        .route(app::FONT, get(api::font))
        .route("/assets/shots/{file}", get(api::showcase_shot))
        .route("/assets/birch/{file}", get(api::birch))
        .route(app::ENHANCE_SCRIPT, get(api::enhance_script))
        .route(app::BOOT_SCRIPT, get(api::boot_script))
        .route(app::SERVICE_WORKER, get(api::service_worker))
        .route("/assets/sql-wasm.js", get(api::sql_js))
        .route("/assets/sql-wasm.wasm", get(api::sql_wasm))
        .route("/pkg/{file}", get(api::package))
        .route("/models/{file}", get(api::semantic_model))
        .route(app::FAVICON_ICO, get(api::favicon_ico))
        .route(app::TOUCH_ICON, get(api::touch_icon))
        // iOS asks for this name too before it reads the page.
        .route("/apple-touch-icon-precomposed.png", get(api::touch_icon))
        .route(app::ICON_192, get(api::icon_192))
        .route(app::ICON_512, get(api::icon_512))
        .route(app::ICON_MASKABLE, get(api::icon_maskable))
        .route(app::ICON_MASKABLE_LARGE, get(api::icon_maskable_large))
        .route(app::ICON_MONOCHROME, get(api::icon_monochrome))
        .route("/assets/launch/{file}", get(api::launch_screen))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/de", get(language_redirect))
        .route("/de/", get(language_redirect))
        .route("/de/{*rest}", get(language_redirect))
        .route("/en/", get(language_redirect))
        .route("/api/db", get(api::database))
        .route("/api/status", get(api::status))
        .route("/api/map.json", get(api::program_map))
        .route("/healthz", get(api::health))
        .route(api::LIVENESS, get(api::alive))
        .merge(files())
        .route("/robots.txt", get(api::robots))
        .route("/sitemap.xml", get(api::sitemap))
        .route(access::PATH, get(access::page).post(access::enter))
        .merge(in_every_language())
        .merge(pages(&state))
        // Around everything above, the page cache included; the access log sees what it turns away.
        .layer(middleware::from_fn_with_state(state.clone(), access::gate))
        .layer(middleware::from_fn_with_state(state.clone(), access_log))
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

/// `folia healthcheck`: is the server of this container alive? Says nothing unless it is not
/// (Docker keeps the output of a failed probe with the container, not in its log).
async fn healthcheck(addr: std::net::SocketAddr) -> std::process::ExitCode {
    // The server may listen on every address; the probe asks the loopback one.
    let ip = match addr.ip() {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() => std::net::Ipv4Addr::LOCALHOST.into(),
        std::net::IpAddr::V6(ip) if ip.is_unspecified() => std::net::Ipv6Addr::LOCALHOST.into(),
        ip => ip,
    };
    let url = format!("http://{}{}", std::net::SocketAddr::new(ip, addr.port()), api::LIVENESS);
    let answer = match reqwest::Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(client) => client.get(&url).send().await,
        Err(error) => Err(error),
    };
    match answer {
        Ok(response) if response.status() == reqwest::StatusCode::OK => std::process::ExitCode::SUCCESS,
        Ok(response) => {
            eprintln!("{url} answered {}", response.status());
            std::process::ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("{url}: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() -> std::process::ExitCode {
    let config = Config::parse();
    // The processors the container may use (Rust counts the CPU limit of its cgroup): renders and
    // calendar feeds run on as many at once (`busy`). The runtime gets one worker thread more, so
    // that one is always free to accept connections and answer what needs no render — cached
    // pages, files, `/livez` — while every processor renders. With one thread for everything (the
    // default of a container limited to one processor) a queue of renders starved the accept loop:
    // in the load test of 2026-09-26 connections were refused before any of them could be told 503.
    let cpus = std::thread::available_parallelism().map(std::num::NonZeroUsize::get).unwrap_or(1);
    let workers = if config.workers > 0 { config.workers } else { cpus + 1 };
    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(workers).enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("folia: cannot start the runtime: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    runtime.block_on(serve(config, cpus, workers))
}

async fn serve(config: Config, cpus: usize, workers: usize) -> std::process::ExitCode {
    match config.command {
        Some(config::Command::Healthcheck) => return healthcheck(config.addr).await,
        Some(config::Command::Assets) => {
            return match assets::report(&config.site_root) {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("folia assets: {error}");
                    std::process::ExitCode::FAILURE
                }
            };
        }
        None => {}
    }
    init_logging(&config);

    // Before anything else: a gate that cannot close must not leave the site open.
    let gate = match access::Gate::from_environment(config.access_gate) {
        Ok(Some((gate, source))) => {
            tracing::info!(component = "access", event = "access.gate_on", source, "closed testing: the site asks for the access password");
            Some(Arc::new(gate))
        }
        Ok(None) => None,
        Err(error) => {
            tracing::error!(component = "server", event = "server.start_failed", error = %error, "the access gate cannot be set up");
            return std::process::ExitCode::FAILURE;
        }
    };

    let store = match SnapshotStore::new(config.data_dir.clone()) {
        Ok(store) => store,
        Err(error) => {
            tracing::error!(component = "server", event = "server.start_failed", data_dir = %config.data_dir.display(), error = %error, "cannot use the data directory");
            return std::process::ExitCode::FAILURE;
        }
    };
    store.restore();
    tokio::spawn(snapshot::run(store.clone(), config.snapshot_url.clone(), config.poll_interval(), config.stale_after()));

    // To the nanosecond: browsers keep what an address of a build names for a year without asking
    // (`api::Keep`), so two processes of different files must never share one, not even two
    // colours of the site that start in the same second when the host boots.
    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    // One place per processor unless configured.
    let places = |configured: usize| if configured > 0 { configured } else { cpus };
    let render_wait = Duration::from_millis(config.render_wait_ms);
    // Optional: without its model the app simply has no semantic search, so a model that cannot
    // be read is an error in the log, not a site that does not start.
    // An empty value is no value: the stack file hands over an empty one without models.
    let passage = config.semantic_passage_model.as_deref().filter(|id| !id.is_empty());
    let semantic = config.semantic_model.as_deref().filter(|path| !path.as_os_str().is_empty()).and_then(|path| match semantic::Model::load(path, passage) {
        Ok(model) => {
            tracing::info!(component = "server", event = "semantic.model", path = %path.display(), served_at = %model.path, passage_model = model.passage.as_deref().unwrap_or("any"), "the browser's model of the semantic search is served");
            Some(Arc::new(model))
        }
        Err(error) => {
            tracing::error!(component = "server", event = "semantic.model_unreadable", path = %path.display(), error = %error, "the browser's model of the semantic search cannot be read; the app runs without the semantic search");
            None
        }
    });
    let state = AppState {
        store,
        cache: Arc::new(HtmlCache::new(config.html_cache_mb * 1024 * 1024)),
        cards: Arc::new(cards::Cards::new(config.card_cache_mb * 1024 * 1024, cards::Cards::places_for_this_machine())),
        launch: Arc::default(),
        build_id: format!("{}-{started_at:x}", env!("CARGO_PKG_VERSION")).into(),
        stale_after: config.stale_after(),
        public_url: config.public_url.trim_end_matches('/').into(),
        site_root: config.site_root.clone(),
        live_assets: config.live_assets.clone(),
        packages: Arc::default(),
        semantic,
        gate,
        renders: Arc::new(busy::Places::new("render", places(config.render_places), render_wait)),
        render_wait,
        feeds: Arc::new(busy::Places::new("calendar", places(config.feed_places), Duration::from_secs(10))),
        changes: config.warm_cache.then(|| Arc::new(lastmod::Changes::load(&config.data_dir))),
        leptos: LeptosOptions::builder()
            .output_name("folia-app")
            .site_root(config.site_root.to_string_lossy().into_owned())
            .site_pkg_dir("pkg")
            .site_addr(config.addr)
            .build(),
    };

    if let Some(dir) = &state.live_assets {
        if !dir.join("app.css").is_file() {
            tracing::error!(component = "server", event = "server.start_failed", live_assets = %dir.display(), "--live-assets names no copy of folia/assets (there is no app.css in it)");
            return std::process::ExitCode::FAILURE;
        }
        tracing::warn!(component = "server", event = "server.live_assets", dir = %dir.display(), "the stylesheet, the scripts and the SVGs come from disk as they are, nothing is kept as immutable, and the service worker keeps nothing: for working on the site, never in production");
    }

    let listener = match tokio::net::TcpListener::bind(config.addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(component = "server", event = "server.start_failed", addr = %config.addr, error = %error, "cannot listen");
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!(
        component = "server",
        event = "server.listening",
        addr = %config.addr,
        snapshot_url = %config.snapshot_url,
        cpus,
        workers,
        render_places = state.renders.count(),
        render_wait_ms = config.render_wait_ms,
        feed_places = state.feeds.count(),
        html_cache_mb = config.html_cache_mb,
        warm_cache = config.warm_cache,
        "web server started"
    );
    if config.warm_cache {
        tokio::spawn(warm::run(pages(&state).with_state(state.clone()), state.store.clone(), state.changes.clone()));
    }
    tokio::spawn(warm::files(files().with_state(state.clone()), state.build_id.clone(), state.semantic.as_ref().map(|model| model.path.clone())));

    match axum::serve(listener, router(state)).with_graceful_shutdown(shutdown_signal()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(component = "server", event = "server.failed", error = %error, "server stopped");
            std::process::ExitCode::FAILURE
        }
    }
}
