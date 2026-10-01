//! Where a page gets its data from.
//!
//! The host provides a `Source` through context: the web server a pool of rusqlite
//! connections on the active snapshot, the browser the downloaded snapshot in sql.js.
//! Pages run the loaders of `catalog::pages` through it and never see the difference.

use std::sync::Arc;

use catalog::{Database, DbError};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

pub trait CatalogSource: Send + Sync {
    /// Runs `job` with a database on one snapshot, so everything a page loads is consistent.
    fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError>;
}

#[derive(Clone)]
pub struct Source(pub Arc<dyn CatalogSource>);

/// The map of the programs on the landing page (`catalog::graph`). The web server lays it out
/// once per snapshot and hands it to the pages it renders; the browser app gets the same map as
/// `/api/map.json` (`boot.js`). Nobody computes it while a page renders; a host without a map
/// simply provides none and the page leaves the section out.
#[derive(Clone)]
pub struct ProgramMapHandle(pub Arc<catalog::graph::ProgramMap>);

/// One module the semantic search found: its id and how close its description is to the query
/// (the cosine of their vectors, higher is closer; semantic/README.md).
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticHit {
    pub module_id: String,
    pub score: f32,
}

/// What a search answers later: the semantic search runs in a Web Worker.
pub type Later<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T>>>;

/// The semantic search (semantic/README.md): the modules whose descriptions mean what a query
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

/// What a page shows instead of data. Serializable, because the server hands the
/// outcome of its queries to the browser for hydration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataError {
    /// No snapshot yet: worth retrying. Otherwise the query itself failed.
    pub unavailable: bool,
    pub message: String,
}

impl From<DbError> for DataError {
    fn from(error: DbError) -> Self {
        Self { unavailable: matches!(error, DbError::Unavailable(_)), message: error.to_string() }
    }
}

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
