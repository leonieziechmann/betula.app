//! The data worker's Rust half. `worker.js` opens the snapshot with sql.js (as `betulaDb`, in the
//! worker's global scope) and hands every message to `answer`, which decodes the request, runs
//! its loader and encodes the reply. Nothing here touches the page.

use catalog::db::Rows;
use catalog::{Database, DbError, Value};
use folia_pages::{decode, encode, Reply, Request};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// `betulaDb.query(sql, params)` → `{ columns, rows }`, sql.js in this worker.
    #[wasm_bindgen(js_namespace = betulaDb, js_name = query, catch)]
    fn db_query(sql: &str, params: js_sys::Array) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn date_now() -> f64;
}

struct WorkerDatabase;

fn js_error(value: &JsValue) -> String {
    value.as_string().or_else(|| js_sys::Reflect::get(value, &"message".into()).ok().and_then(|m| m.as_string())).unwrap_or_else(|| format!("{value:?}"))
}

impl Database for WorkerDatabase {
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
                    if number.fract() == 0.0 && number.abs() < 9.0e15 { Value::Integer(number as i64) } else { Value::Real(number) }
                } else {
                    return Err(DbError::Decode { query: name, column: String::new(), message: "unexpected value type".to_string() });
                });
            }
            rows.push(values);
        }
        Ok(Rows { columns, rows })
    }
}

/// One request in, its reply out (postcard both ways). `snapshot` names the copy that answers.
#[wasm_bindgen]
pub fn answer(request: &[u8], snapshot: &str) -> Vec<u8> {
    let started = date_now();
    let reply = match decode::<Request>(request) {
        Ok(request) => {
            let result = folia_pages::answer(&WorkerDatabase, &request.ask);
            Reply { id: request.id, snapshot: snapshot.to_string(), ms: date_now() - started, result }
        }
        Err(error) => Reply { id: 0, snapshot: snapshot.to_string(), ms: 0.0, result: Err(error) },
    };
    encode(&reply).unwrap_or_default()
}
