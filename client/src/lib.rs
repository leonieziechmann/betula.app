//! The browser app: the same Leptos components as on the server, rendered in the browser
//! against the local copy of the catalog (sql.js, opened by `app/assets/boot.js`).
//!
//! `start()` is called once the database is open. It replaces the server-rendered page by
//! the app; from then on every click is handled here and no page is loaded again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use app::data::{CatalogSource, Source};
use catalog::db::Rows;
use catalog::{Database, DbError, Value};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// `window.betulaDb.query(sql, params)` → `{ columns: string[], rows: any[][] }`.
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

/// What the local catalog has answered in this visit, by statement and parameters. The copy
/// `boot.js` opened is never written to and stays the same until the next start (a newer
/// snapshot is only used from then on), and no query reads the clock, so an answer holds for the
/// whole visit. Coming back to a page, going back in the history or taking a filter back then
/// asks sql.js nothing: the start page's queries alone take 70 ms of a laptop's time, a phone's
/// four times that. The oldest answers go once the kept ones reach `ANSWERS_BUDGET`.
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
                let t = app::i18n::texts(app::i18n::of_address());
                banner.set_inner_html(&format!("{} <a href=\"\">{}</a>", t.ui.crashed, t.ui.reload_page));
                let _ = body.append_child(&banner);
            }
        }
    }));
}

/// The map of the programs as `boot.js` got it from the server (`window.betulaMap`, JSON). The
/// app does not lay anything out; without a map the landing page leaves the section out.
fn program_map() -> Option<app::data::ProgramMapHandle> {
    let text = js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaMap".into()).ok()?.as_string()?;
    serde_json::from_str(&text).ok().map(|map| app::data::ProgramMapHandle(Arc::new(map)))
}

/// The build the server wrote the page with: the `?v=` of its stylesheet (`app::BuildId`), which
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
    // The language of the address (`app::i18n`): the page the service worker kept for a start
    // without a network may be of another one.
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("lang", app::i18n::of_address().code());
    }
    // Not hydration: the local database may be older than the server's page, so the app renders
    // fresh. Same components, same markup, so nothing visibly changes.
    body.set_inner_html("");
    // The same goes for what the server wrote into the head for this page (`app::seo`): the app
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
    leptos::mount::mount_to_body(move || {
        provide_context(Source(Arc::new(LocalSource)));
        // The icons point into the sprite of this build (`app::icons`), as the server's page did.
        if let Some(build) = build.clone() {
            provide_context(app::BuildId(build.into()));
        }
        if let Some(map) = map.clone() {
            provide_context(map);
        }
        if let Some(site) = site.clone() {
            provide_context(app::seo::SiteUrl(site.into()));
        }
        view! { <app::App/> }
    });
}
