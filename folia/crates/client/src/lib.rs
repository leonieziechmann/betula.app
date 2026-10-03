//! The browser app: the same Leptos components as on the server, rendered in the browser. Their
//! questions go to the data worker (`worker`), which runs this same bundle with the catalog in
//! sql.js (`LocalDatabase`, opened by `js/data-worker.js`).
//!
//! `start()` is called once the data worker has opened the catalog. It replaces the
//! server-rendered page by the app; from then on every click is handled here and no page is
//! loaded again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use folia_data::{CatalogSource, Source};
use folia_model::db::Rows;
use folia_model::{Database, DbError, Value};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod worker;

#[wasm_bindgen]
extern "C" {
    /// `betulaDb.query(sql, params)` → `{ columns: string[], rows: any[][] }`: the catalog, in the
    /// data worker (`data-worker.js`).
    #[wasm_bindgen(js_namespace = betulaDb, js_name = query, catch)]
    fn db_query(sql: &str, params: js_sys::Array) -> Result<JsValue, JsValue>;
}

fn js_error(value: &JsValue) -> String {
    value
        .as_string()
        .or_else(|| js_sys::Reflect::get(value, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| format!("{value:?}"))
}

/// `Database` on top of sql.js. Queries are synchronous, like rusqlite on the server.
struct LocalDatabase;

/// What the catalog in the data worker has answered, by statement and parameters. The copy it
/// opened is never written to, and no query reads the clock, so an answer holds until the worker
/// opens a newer snapshot (`forget_statements`, from `worker::worker_forget`). Coming back to a
/// page, going back in the history or taking a filter back then asks sql.js nothing: the start
/// page's queries alone take 70 ms of a laptop's time, a phone's four times that. The oldest answers go once the kept ones reach `ANSWERS_BUDGET`.
struct Answers {
    kept: HashMap<String, Answer>,
    bytes: usize,
    clock: u64,
}

struct Answer {
    rows: Rows,
    bytes: usize,
    used: u64,
}

/// About what the kept answers take in memory (estimated from their cells), a small part of
/// the 37 MB of the catalog itself.
const ANSWERS_BUDGET: usize = 24 << 20;

thread_local! {
    static ANSWERS: RefCell<Answers> = RefCell::new(Answers { kept: HashMap::new(), bytes: 0, clock: 0 });
}

impl Answers {
    /// The statement and its parameters, each text with its length in front, so that no two
    /// different questions read the same (a search may contain anything).
    fn key(sql: &str, params: &[Value]) -> String {
        let mut key = String::with_capacity(sql.len() + 16 * params.len() + 8);
        let _ = write!(key, "{}:{sql}", sql.len());
        for param in params {
            let _ = match param {
                Value::Null => write!(key, "|n"),
                Value::Integer(i) => write!(key, "|i{i}"),
                Value::Real(r) => write!(key, "|r{r:?}"),
                Value::Text(s) => write!(key, "|t{}:{s}", s.len()),
            };
        }
        key
    }

    fn get(&mut self, key: &str) -> Option<Rows> {
        self.clock += 1;
        let clock = self.clock;
        self.kept.get_mut(key).map(|answer| {
            answer.used = clock;
            answer.rows.clone()
        })
    }

    fn keep(&mut self, key: String, rows: &Rows) {
        let bytes = key.len()
            + rows.columns.iter().map(|c| c.len() + 24).sum::<usize>()
            + rows.rows.iter().flatten().map(|cell| 24 + if let Value::Text(s) = cell { s.len() } else { 0 }).sum::<usize>();
        if bytes > ANSWERS_BUDGET / 4 {
            return;
        }
        while self.bytes + bytes > ANSWERS_BUDGET {
            let Some(oldest) = self.kept.iter().min_by_key(|(_, answer)| answer.used).map(|(key, _)| key.clone()) else { break };
            if let Some(answer) = self.kept.remove(&oldest) {
                self.bytes -= answer.bytes;
            }
        }
        self.clock += 1;
        self.bytes += bytes;
        if let Some(old) = self.kept.insert(key, Answer { rows: rows.clone(), bytes, used: self.clock }) {
            self.bytes -= old.bytes;
        }
    }
}

/// A newer snapshot answers from now on: no statement's answer of the one before counts.
pub(crate) fn forget_statements() {
    ANSWERS.with_borrow_mut(|answers| {
        answers.kept.clear();
        answers.bytes = 0;
    });
}

impl Database for LocalDatabase {
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
        let key = Answers::key(sql, params);
        if let Some(rows) = ANSWERS.with_borrow_mut(|answers| answers.get(&key)) {
            return Ok(rows);
        }
        let rows = ask(name, sql, params)?;
        ANSWERS.with_borrow_mut(|answers| answers.keep(key, &rows));
        Ok(rows)
    }
}

/// One statement on sql.js.
fn ask(name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
    let fail = |message: String| DbError::Sql { query: name, message };
    let bound = js_sys::Array::new();
    for param in params {
        bound.push(&match param {
            Value::Null => JsValue::NULL,
            Value::Integer(i) => JsValue::from_f64(*i as f64),
            Value::Real(r) => JsValue::from_f64(*r),
            Value::Text(s) => JsValue::from_str(s),
        });
    }
    let result = db_query(sql, bound).map_err(|e| fail(js_error(&e)))?;
    let get = |key: &str| js_sys::Reflect::get(&result, &key.into()).map_err(|e| fail(js_error(&e)));

    let columns: Vec<String> = js_sys::Array::from(&get("columns")?).iter().filter_map(|c| c.as_string()).collect();
    let mut rows = Vec::new();
    for row in js_sys::Array::from(&get("rows")?).iter() {
        let mut values = Vec::with_capacity(columns.len());
        for cell in js_sys::Array::from(&row).iter() {
            values.push(if cell.is_null() || cell.is_undefined() {
                Value::Null
            } else if let Some(text) = cell.as_string() {
                Value::Text(text)
            } else if let Some(number) = cell.as_f64() {
                // sql.js hands out every number as a double.
                if number.fract() == 0.0 && number.abs() < 9.0e15 {
                    Value::Integer(number as i64)
                } else {
                    Value::Real(number)
                }
            } else {
                return Err(DbError::Decode { query: name, column: String::new(), message: "unexpected value type".to_string() });
            });
        }
        rows.push(values);
    }
    Ok(Rows { columns, rows })
}

struct LocalSource;

impl CatalogSource for LocalSource {
    fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
        job(&LocalDatabase);
        Ok(())
    }
}

/// A panic cannot unwind in WASM and would leave a frozen page: say so and offer a reload.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let (Ok(banner), Some(body)) = (document.create_element("div"), document.body()) {
                banner.set_class_name("fatal");
                let t = folia_app::i18n::texts(folia_app::i18n::of_address());
                banner.set_inner_html(&format!("{} <a href=\"\">{}</a>", t.ui.crashed, t.ui.reload_page));
                let _ = body.append_child(&banner);
            }
        }
    }));
}

/// The map of the programs as `boot.js` got it from the server (`window.betulaMap`, JSON). The
/// app does not lay anything out; without a map the landing page leaves the section out.
fn program_map() -> Option<folia_data::ProgramMapHandle> {
    let text = js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaMap".into()).ok()?.as_string()?;
    serde_json::from_str(&text).ok().map(|map| folia_data::ProgramMapHandle(Arc::new(map)))
}

/// The semantic search as `boot.js` loads it (`window.betulaSemantic`): asked anew on every call,
/// so it holds no JavaScript object of its own.
struct BrowserSemantic;

impl BrowserSemantic {
    /// `window.betulaSemantic[method](...args)`, awaited; `None` when there is no such thing.
    async fn call(method: &str, args: &[JsValue]) -> Option<JsValue> {
        let semantic = js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaSemantic".into()).ok().filter(|v| v.is_object())?;
        let promise = match method {
            "ready" => js_sys::Reflect::get(&semantic, &"ready".into()).ok()?,
            _ => {
                let function: js_sys::Function = js_sys::Reflect::get(&semantic, &method.into()).ok()?.dyn_into().ok()?;
                function.apply(&semantic, &args.iter().collect::<js_sys::Array>()).ok()?
            }
        };
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await.ok().filter(|answer| !answer.is_null() && !answer.is_undefined())
    }
}

impl folia_data::SemanticSearch for BrowserSemantic {
    fn ready(&self) -> folia_data::Later<bool> {
        Box::pin(async { BrowserSemantic::call("ready", &[]).await.is_some() })
    }

    fn search(&self, query: &str, k: usize) -> folia_data::Later<Option<Vec<folia_data::SemanticHit>>> {
        let args = [JsValue::from_str(query), JsValue::from_f64(k as f64)];
        Box::pin(async move {
            let answer = BrowserSemantic::call("search", &args).await?;
            let hits = js_sys::Reflect::get(&answer, &"hits".into()).ok()?;
            let hits = js_sys::Array::from(&hits)
                .iter()
                .filter_map(|hit| {
                    let module_id = js_sys::Reflect::get(&hit, &"id".into()).ok()?.as_string()?;
                    let score = js_sys::Reflect::get(&hit, &"score".into()).ok()?.as_f64()? as f32;
                    Some(folia_data::SemanticHit { module_id, score })
                })
                .collect();
            Some(hits)
        })
    }
}

/// The build the server wrote the page with: the `?v=` of its stylesheet (`folia_app::BuildId`), which
/// stays in the head when the app takes the body over.
fn build_of_page(document: &web_sys::Document) -> Option<String> {
    let href = document.query_selector("link[rel=stylesheet]").ok()??.get_attribute("href")?;
    let (_, build) = href.split_once("?v=")?;
    Some(build.split(['&', '#']).next()?.to_string()).filter(|build| !build.is_empty())
}

/// Called by `boot.js` when the local database is open.
#[wasm_bindgen]
pub fn start() {
    install_panic_hook();
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(body) = document.body() else { return };
    // The language of the address (`folia_app::i18n`): the page the service worker kept for a start
    // without a network may be of another one.
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("lang", folia_app::i18n::of_address().code());
    }
    // The page's questions go to the data worker (`worker`), where `boot.js` started one;
    // otherwise to the local copy on this thread.
    let data = worker::client();
    // Not hydration: the local database may be older than the server's page, so the app renders
    // fresh. Same components, same markup, so nothing visibly changes; and while the app's first
    // answers are on their way, the server's page stays in front of it as a picture (`boot.js`
    // took it before it called this; `answered` says when it can go).
    body.set_inner_html("");
    // The same goes for what the server wrote into the head for this page (`folia_shell::seo`): the app
    // writes its own, and what stayed would describe the first page on every later one.
    let stale = "meta[name=description], meta[name=robots], link[rel=canonical], link[rel=alternate][hreflang], meta[property^='og:'], meta[name^='twitter:'], script[type='application/ld+json']";
    if let Ok(tags) = document.query_selector_all(stale) {
        for i in 0..tags.length() {
            if let Some(tag) = tags.item(i) {
                if let Some(parent) = tag.parent_node() {
                    let _ = parent.remove_child(&tag);
                }
            }
        }
    }
    let map = program_map();
    let site = web_sys::window().and_then(|w| w.location().origin().ok());
    let build = build_of_page(&document);
    let client = data.clone();
    leptos::mount::mount_to_body(move || {
        provide_context(Source(Arc::new(LocalSource)));
        if let Some(data) = client.clone() {
            provide_context(data);
        }
        // Loaded by `boot.js` once the app runs; nothing waits for it. The catalog's „Ähnliche Module"
        // ask it (`folia_app::pages::catalog`).
        provide_context(folia_data::Semantic(Arc::new(BrowserSemantic)));
        // The icons point into the sprite of this build (`folia_design::icons`), as the server's page did.
        if let Some(build) = build.clone() {
            provide_context(folia_app::BuildId(build.into()));
        }
        if let Some(map) = map.clone() {
            provide_context(map);
        }
        if let Some(site) = site.clone() {
            provide_context(folia_shell::seo::SiteUrl(site.into()));
        }
        view! { <folia_app::App/> }
    });
    answered(data);
}

/// Tells `boot.js` (`window.betulaAnswered`) once the app's first answers are in: no question on
/// its way in two looks a frame apart, or after `TAKEOVER_MS` whatever is still on its way.
fn answered(data: Option<folia_data::DataClient>) {
    const TAKEOVER_MS: f64 = 4000.0;
    let tell = || {
        if let Some(window) = web_sys::window() {
            if let Ok(function) = js_sys::Reflect::get(&window, &"betulaAnswered".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
                let _ = function.call0(&window);
            }
        }
    };
    let Some(data) = data else { return tell() };
    let started = js_sys::Date::now();
    wasm_bindgen_futures::spawn_local(async move {
        let mut quiet = 0;
        loop {
            sleep(16).await;
            quiet = if untrack(|| data.waiting()) == 0 { quiet + 1 } else { 0 };
            if quiet >= 2 || js_sys::Date::now() - started > TAKEOVER_MS {
                break;
            }
        }
        tell();
    });
}

/// A timer as a future (no crate for it: one promise).
async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
