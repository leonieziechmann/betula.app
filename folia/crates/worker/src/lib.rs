//! The data worker's bundle (docs/folia/folia-refactor.md §6.2): the questions of the app's pages
//! (`folia_pages::ask`) answered from the catalog in sql.js, beside the page's thread. No Leptos,
//! no page: `js/data-worker.js` opens the catalog, puts `betulaDb` on its global object and asks
//! `worker_answer`; the page's bundle (`folia-client`) only sends the questions.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;

use folia_model::db::Rows;
use folia_model::{Database, DbError, Value};
use folia_pages::ask::{self, Kept};
use wasm_bindgen::prelude::*;

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
/// opens a newer snapshot (`forget_statements`, from `worker_forget`). Coming back to a
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
fn forget_statements() {
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


thread_local! {
    /// What the worker keeps between questions besides the answers (the finder's candidates).
    static KEPT: RefCell<Kept> = RefCell::new(Kept::default());
}

/// In the worker: the answer to the question `name` (`Ask::NAME`) with the fields `question` (JSON),
/// as the JSON of its `Result<Answer, DataError>`; empty for a name no question has.
#[wasm_bindgen]
pub fn worker_answer(name: &str, question: &str) -> String {
    KEPT.with_borrow_mut(|kept| ask::answer_json(name, question, &LocalDatabase, kept)).unwrap_or_default()
}

/// In the worker: a newer snapshot answers from now on, so nothing kept of the one before counts,
/// neither what the questions keep nor the statements' answers.
#[wasm_bindgen]
pub fn worker_forget() {
    KEPT.with_borrow_mut(|kept| *kept = Kept::default());
    forget_statements();
}

