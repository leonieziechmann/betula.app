//! The catalog's search beside the page's thread (`app::data::CatalogWorker`; owner, 2026-10-02:
//! typing a search lagged): a Web Worker with a copy of the local catalog of its own and this
//! bundle, which runs the loaders of `catalog::pages` on it as the page would on its own copy.
//!
//! Both sides are here. In the worker (`folia/crates/client/js/search-worker.js`, which opens the catalog and
//! puts `betulaDb` on its global object) the functions `worker_catalog` and `worker_similar`
//! answer a question in JSON with JSON. On the page `BrowserWorker` asks them through
//! `window.betulaSearch` (`folia/assets/boot.js`, which starts the worker once the app runs).

use app::data::{CatalogWorker, DataError, Later};
use catalog::filter::CatalogQuery;
use catalog::pages::{self, CatalogData};
use catalog::rows::CatalogRow;
use catalog::url::CatalogUrl;
use catalog::Locale;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::LocalDatabase;

/// The list of an address, as the page asks for it: its query as the list runs it (the marks and
/// what fits the plan filled in, which no address carries), its page and placeholder, the language.
#[derive(Serialize, Deserialize)]
struct CatalogAsk {
    query: CatalogQuery,
    page: u64,
    fill: Option<u32>,
    locale: Locale,
}

/// „Ähnliche Module" of the query a list ran, of the semantic search's hits.
#[derive(Serialize, Deserialize)]
struct SimilarAsk {
    query: CatalogQuery,
    hits: Vec<String>,
    limit: usize,
}

fn failed(error: impl std::fmt::Display) -> DataError {
    DataError { unavailable: false, message: error.to_string() }
}

/// In the worker: `pages::catalog` of the address `ask` names, a `Result<CatalogData, DataError>`.
#[wasm_bindgen]
pub fn worker_catalog(ask: &str) -> String {
    let answer = serde_json::from_str::<CatalogAsk>(ask).map_err(failed).and_then(|ask| {
        let url = CatalogUrl { query: ask.query, page: ask.page, open: None, fill: ask.fill };
        pages::catalog(&LocalDatabase, &url, ask.locale).map_err(DataError::from)
    });
    serde_json::to_string(&answer).unwrap_or_default()
}

/// In the worker: `pages::similar` of `ask`, a `Result<Vec<CatalogRow>, DataError>`.
#[wasm_bindgen]
pub fn worker_similar(ask: &str) -> String {
    let answer = serde_json::from_str::<SimilarAsk>(ask)
        .map_err(failed)
        .and_then(|ask| pages::similar(&LocalDatabase, &ask.query, &ask.hits, ask.limit).map_err(DataError::from));
    serde_json::to_string(&answer).unwrap_or_default()
}

/// The worker as the page asks it: `window.betulaSearch`, asked anew on every call (like
/// `BrowserSemantic`), so it holds no JavaScript object of its own.
pub(crate) struct BrowserWorker;

impl BrowserWorker {
    fn search() -> Option<JsValue> {
        js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaSearch".into()).ok().filter(JsValue::is_object)
    }

    /// `window.betulaSearch[method](ask)`, awaited: the answer's JSON, or `None` without one.
    async fn ask(method: &str, ask: String) -> Option<String> {
        let search = Self::search()?;
        let function: js_sys::Function = js_sys::Reflect::get(&search, &method.into()).ok()?.dyn_into().ok()?;
        let promise = function.call1(&search, &JsValue::from_str(&ask)).ok()?;
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await.ok()?.as_string()
    }
}

impl CatalogWorker for BrowserWorker {
    fn ready(&self) -> bool {
        Self::search().and_then(|search| js_sys::Reflect::get(&search, &"ready".into()).ok()).and_then(|ready| ready.as_bool()).unwrap_or(false)
    }

    fn catalog(&self, url: &CatalogUrl, locale: Locale) -> Later<Option<Result<CatalogData, DataError>>> {
        let ask = serde_json::to_string(&CatalogAsk { query: url.query.clone(), page: url.page, fill: url.fill, locale });
        Box::pin(async move { serde_json::from_str(&Self::ask("catalog", ask.ok()?).await?).ok() })
    }

    fn similar(&self, query: &CatalogQuery, hits: &[String], limit: usize) -> Later<Option<Result<Vec<CatalogRow>, DataError>>> {
        let ask = serde_json::to_string(&SimilarAsk { query: query.clone(), hits: hits.to_vec(), limit });
        Box::pin(async move { serde_json::from_str(&Self::ask("similar", ask.ok()?).await?).ok() })
    }
}
