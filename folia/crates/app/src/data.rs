//! Where a page gets its data from.
//!
//! The host provides a `Source` through context: the web server a pool of rusqlite
//! connections on the active snapshot, the browser the downloaded snapshot in sql.js.
//! Pages run the loaders of `folia_pages` through it and never see the difference.

use std::sync::Arc;

use folia_locale::Locale;
use folia_model::rows::CatalogRow;
use folia_model::{Database, DbError};
use folia_pages::ask::{Ask, Kept};
use folia_pages::CatalogData;
use folia_routes::filter::CatalogQuery;
use folia_routes::url::CatalogUrl;
use leptos::prelude::*;

pub trait CatalogSource: Send + Sync {
    /// Runs `job` with a database on one snapshot, so everything a page loads is consistent.
    fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError>;
}

#[derive(Clone)]
pub struct Source(pub Arc<dyn CatalogSource>);

/// The map of the programs on the landing page (`folia_pages::graph`). The web server lays it out
/// once per snapshot and hands it to the pages it renders; the browser app gets the same map as
/// `/api/map.json` (`boot.js`). Nobody computes it while a page renders; a host without a map
/// simply provides none and the page leaves the section out.
#[derive(Clone)]
pub struct ProgramMapHandle(pub Arc<folia_pages::graph::ProgramMap>);

/// One module the semantic search found: its id and how close its description is to the query
/// (the cosine of their vectors, higher is closer; folia/crates/semantic/README.md).
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticHit {
    pub module_id: String,
    pub score: f32,
}

/// What a search answers later: the semantic search runs in a Web Worker.
pub type Later<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T>>>;

/// The semantic search (folia/crates/semantic/README.md): the modules whose descriptions mean what a query
/// says, the catalog's „Ähnliche Module" under the results of a search. Only the browser app has
/// it (`client`, the model in a Web Worker that `boot.js` loads once the app runs); on the server,
/// and in a browser that has none, there is no `Semantic` in the context.
pub trait SemanticSearch: Send + Sync {
    /// Whether the search can answer: false for good when this browser has none (no model on the
    /// server, no vectors in the catalog yet, data saving, little memory). Resolves once loading
    /// has ended either way; until then a search waits.
    fn ready(&self) -> Later<bool>;
    /// The `k` modules closest to `query`, best first. `None` when a newer query took its place
    /// before this one ran (typing fast never piles up work), or when there is no search.
    fn search(&self, query: &str, k: usize) -> Later<Option<Vec<SemanticHit>>>;
}

#[derive(Clone)]
pub struct Semantic(pub Arc<dyn SemanticSearch>);

/// The catalog's search beside the page's thread (owner, 2026-10-02: typing a search lagged, „die
/// Suche muss auf jeden Fall asynchron"): in the browser app a Web Worker with a copy of the local
/// catalog of its own, which runs the same loaders of `folia_pages` as the page does (`client`,
/// `boot.js`: `window.betulaSearch`). The list of a search the visitor types is worked out there
/// before the address changes (`Pending::prepare_with`), and so is the list's „Ähnliche Module".
/// None on the server; in the browser it answers once it is loaded, and until then, or where it
/// failed, the page asks its `Source` as it always did.
pub trait CatalogWorker: Send + Sync {
    /// Whether it answers now: it is loaded once the app runs and the browser is idle, and is
    /// none for good where it failed or the device has little memory.
    fn ready(&self) -> bool;
    /// `pages::catalog` of `url` (its query as the list runs it, marks and what fits the plan filled
    /// in) in `locale`. `None` where it gave no answer: a newer question of the same kind took its
    /// place before this one ran, or the worker failed.
    fn catalog(&self, url: &CatalogUrl, locale: Locale) -> Later<Option<Result<CatalogData, DataError>>>;
    /// `pages::similar` of `query` and the semantic search's `hits`, at most `limit`; `None` as for
    /// `catalog`.
    fn similar(&self, query: &CatalogQuery, hits: &[String], limit: usize) -> Later<Option<Result<Vec<CatalogRow>, DataError>>>;
}

#[derive(Clone)]
pub struct Worker(pub Arc<dyn CatalogWorker>);

pub use folia_pages::ask::DataError;

impl Source {
    pub fn run<T>(&self, load: impl FnOnce(&dyn Database) -> Result<T, DbError>) -> Result<T, DataError> {
        let mut load = Some(load);
        let mut outcome = None;
        self.0.with_db(&mut |db| {
            if let Some(load) = load.take() {
                outcome = Some(load(db));
            }
        })?;
        match outcome {
            Some(result) => Ok(result?),
            None => Err(DbError::Unavailable("the data source did not run the query".to_string()).into()),
        }
    }
}

/// The source the host provided. Call it in the component body, not inside a future.
pub fn use_source() -> Result<Source, DataError> {
    use_context::<Source>()
        .ok_or_else(|| DbError::Unavailable("no data source was provided".to_string()).into())
}

/// Where a page asks its questions (`folia_pages::ask`, docs/folia/folia-refactor.md §6.4): every
/// page and component asks through it, never a `Database` itself. For now it answers on the page's
/// thread from the host's `Source` (the server's snapshot, the browser's local catalog); the data
/// worker takes its place without a page noticing.
#[derive(Clone)]
pub struct DataClient {
    source: Source,
    /// What the answering side keeps besides answers (the finder's candidates).
    kept: Arc<std::sync::Mutex<Kept>>,
}

impl DataClient {
    pub fn new(source: Source) -> Self {
        Self { source, kept: Arc::new(std::sync::Mutex::new(Kept::default())) }
    }

    /// The answer to `ask`, now.
    pub fn now<A: Ask>(&self, ask: &A) -> Result<A::Answer, DataError> {
        let kept = &self.kept;
        self.source.run(|db| match kept.lock() {
            Ok(mut kept) => ask.run(db, &mut kept),
            Err(_) => ask.run(db, &mut Kept::default()),
        })
    }
}

/// The client the host provided, as `use_source` (call it in the component body). A host that
/// provides a `Source` and no client gets one over that source.
pub fn use_data() -> Result<DataClient, DataError> {
    if let Some(client) = use_context::<DataClient>() {
        return Ok(client);
    }
    use_source().map(DataClient::new)
}

/// The answer to the question `ask` makes, following what it reads: `None` where it asks nothing
/// (and while an answer is on its way, once questions travel to the data worker).
pub fn use_ask<A: Ask + Send + Sync>(ask: impl Fn() -> Option<A> + Send + Sync + 'static) -> Memo<Option<Result<A::Answer, DataError>>>
where
    A::Answer: Send + Sync,
{
    let client = use_data();
    Memo::new(move |_| {
        let question = ask()?;
        Some(client.as_ref().map_err(Clone::clone).and_then(|client| client.now(&question)))
    })
}

/// Sets the HTTP status of a server-rendered page (404 for an unknown module, 503 without
/// a snapshot). Captured in the component body; does nothing in the browser.
#[derive(Clone)]
pub struct PageStatus {
    #[cfg(feature = "ssr")]
    response: Option<leptos_axum::ResponseOptions>,
}

impl PageStatus {
    pub fn capture() -> Self {
        Self {
            #[cfg(feature = "ssr")]
            response: use_context::<leptos_axum::ResponseOptions>(),
        }
    }

    #[allow(unused_variables)]
    pub fn set(&self, code: u16) {
        #[cfg(feature = "ssr")]
        if let (Some(response), Ok(status)) = (&self.response, http::StatusCode::from_u16(code)) {
            response.set_status(status);
        }
    }

    pub fn for_error(&self, error: &DataError) {
        self.set(if error.unavailable { 503 } else { 500 });
    }
}
