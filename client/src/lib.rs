//! The browser app: the same Leptos components as on the server, rendered in the browser
//! against the local copy of the catalog (sql.js, opened by `app/assets/boot.js`).
//!
//! `start()` is called once the database is open. It replaces the server-rendered page by
//! the app; from then on every click is handled here and no page is loaded again.

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

impl Database for LocalDatabase {
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
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
                banner.set_inner_html("Etwas ist schiefgelaufen. <a href=\"\">Seite neu laden</a>");
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

/// Called by `boot.js` when the local database is open.
#[wasm_bindgen]
pub fn start() {
    install_panic_hook();
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(body) = document.body() else { return };
    // Not hydration: the local database may be older than the server's page, so the app renders
    // fresh. Same components, same markup, so nothing visibly changes.
    body.set_inner_html("");
    // The same goes for what the server wrote into the head for this page (`app::seo`): the app
    // writes its own, and what stayed would describe the first page on every later one.
    let stale = "meta[name=description], meta[name=robots], link[rel=canonical], meta[property^='og:'], meta[name^='twitter:'], script[type='application/ld+json']";
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
    leptos::mount::mount_to_body(move || {
        provide_context(Source(Arc::new(LocalSource)));
        if let Some(map) = map.clone() {
            provide_context(map);
        }
        if let Some(site) = site.clone() {
            provide_context(app::seo::SiteUrl(site.into()));
        }
        view! { <app::App/> }
    });
}
