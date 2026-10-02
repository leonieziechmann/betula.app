//! The pages' questions to the data worker and their answers (docs/folia-refactor.md §6.3): one
//! request per loader, its answer the loader's data, in postcard across the thread boundary. The
//! worker answers them with `answer` on its database; the site calls the same loaders natively.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

use catalog::pages::{self, CatalogData, ModuleData};
use catalog::url::CatalogUrl;
use catalog::{Database, Locale};
use serde::{Deserialize, Serialize};

/// What a page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ask {
    /// A page of the unfiltered catalog (the minimal version has no filters).
    Catalog { page: u64 },
    Module { id: String },
}

impl Ask {
    /// Requests of one lane replace each other while they wait (docs/folia-refactor.md §6.3).
    pub fn lane(&self) -> Lane {
        Lane::Page
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Page,
}

/// What the worker answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    Catalog(Box<CatalogData>),
    Module(Option<Box<ModuleData>>),
}

/// A request as it crosses to the worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u32,
    pub ask: Ask,
}

/// The worker's answer to a request, and how long the loader took in the worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reply {
    pub id: u32,
    /// The snapshot the answer came from (its ETag): the UI drops what it kept of another.
    pub snapshot: String,
    pub ms: f64,
    pub result: Result<Answer, String>,
}

/// Runs the loader of `ask` on `db`.
pub fn answer(db: &dyn Database, ask: &Ask) -> Result<Answer, String> {
    match ask {
        Ask::Catalog { page } => {
            let url = CatalogUrl { page: (*page).max(1), ..CatalogUrl::default() };
            pages::catalog(db, &url, Locale::De).map(|data| Answer::Catalog(Box::new(data))).map_err(|e| e.to_string())
        }
        Ask::Module { id } => pages::module(db, id).map(|data| Answer::Module(data.map(Box::new))).map_err(|e| e.to_string()),
    }
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    postcard::to_stdvec(value).map_err(|e| e.to_string())
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, String> {
    postcard::from_bytes(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalog::native::NativeDatabase;

    fn snapshot() -> Option<NativeDatabase> {
        let path = std::env::var("FOLIA_TEST_SNAPSHOT").unwrap_or_else(|_| "../../../snapshot/catalog.db".to_string());
        NativeDatabase::open(std::path::Path::new(&path)).ok()
    }

    /// Every answer survives the trip through postcard as it was: a format that is not
    /// self-describing breaks on a field serde skips (`skip_serializing_if`).
    #[test]
    fn answers_cross_the_thread_boundary_unchanged() {
        let Some(db) = snapshot() else { return };
        let module_id = match answer(&db, &Ask::Catalog { page: 1 }).unwrap() {
            Answer::Catalog(data) => data.page.rows[0].id.clone(),
            other => panic!("{other:?}"),
        };
        for ask in [Ask::Catalog { page: 1 }, Ask::Catalog { page: 37 }, Ask::Module { id: module_id }] {
            let reply = Reply { id: 7, snapshot: "x".into(), ms: 1.0, result: answer(&db, &ask) };
            let bytes = encode(&reply).unwrap();
            let back: Reply = decode(&bytes).unwrap();
            assert_eq!(back.result, reply.result, "{ask:?}");
            eprintln!("{ask:?}: {} bytes", bytes.len());
        }
    }
}
