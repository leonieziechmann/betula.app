//! The database seam. Above it everything is plain Rust; below it is SQLite, either
//! rusqlite (web server, tests) or sql.js (browser). Both run the same SQL text.

use std::collections::HashMap;
use std::fmt;

/// The schema of the snapshot the queries of this crate are written for: its `PRAGMA
/// user_version`, the number of the last migration of Radix (`internal/catalogdb/migrations`,
/// docs/schema-v2.md §1). A copy of an older schema lacks what they select (before 0008,
/// `v_program_plan` had no `source_pages`), so the browser does not start the app on one
/// (`app/assets/boot.js`, into which the server writes this number), and the server reports it
/// when it serves one. A test holds it to the newest migration.
pub const SCHEMA_VERSION: i64 = 8;

/// A SQLite value, as a parameter or as a result cell.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
impl From<&String> for Value {
    fn from(v: &String) -> Self {
        Value::Text(v.clone())
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Real(v)
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Integer(i64::from(v))
    }
}

/// Why a query produced no result. Never swallowed: pages render it as an error state.
#[derive(Clone, Debug, PartialEq)]
pub enum DbError {
    /// No snapshot is loaded (yet, or the download failed).
    Unavailable(String),
    /// SQLite rejected or failed the statement.
    Sql { query: &'static str, message: String },
    /// A row does not have the shape its row struct expects: the contract changed.
    Decode { query: &'static str, column: String, message: String },
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Unavailable(why) => write!(f, "catalog not available: {why}"),
            DbError::Sql { query, message } => write!(f, "query {query} failed: {message}"),
            DbError::Decode { query, column, message } => {
                write!(f, "query {query}: column {column}: {message}")
            }
        }
    }
}

impl std::error::Error for DbError {}

/// The result of one statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// Runs read-only SQL against a catalog snapshot.
pub trait Database {
    /// `name` is the stable name of the query: it shows up in errors and logs, and the
    /// test suite uses it to prove that every query ran against a real snapshot.
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError>;
}

/// One result row with access by column name.
pub struct Row<'a> {
    query: &'static str,
    index: &'a HashMap<&'a str, usize>,
    values: &'a [Value],
}

impl Row<'_> {
    fn cell(&self, column: &str) -> Result<&Value, DbError> {
        self.index
            .get(column)
            .and_then(|i| self.values.get(*i))
            .ok_or_else(|| self.error(column, "no such column in the result"))
    }

    fn error(&self, column: &str, message: &str) -> DbError {
        DbError::Decode { query: self.query, column: column.to_string(), message: message.to_string() }
    }

    pub fn opt_text(&self, column: &str) -> Result<Option<String>, DbError> {
        match self.cell(column)? {
            Value::Null => Ok(None),
            Value::Text(s) => Ok(Some(s.clone())),
            Value::Integer(i) => Ok(Some(i.to_string())),
            Value::Real(_) => Err(self.error(column, "expected text, found a real")),
        }
    }

    pub fn text(&self, column: &str) -> Result<String, DbError> {
        self.opt_text(column)?.ok_or_else(|| self.error(column, "unexpected NULL"))
    }

    pub fn opt_int(&self, column: &str) -> Result<Option<i64>, DbError> {
        match self.cell(column)? {
            Value::Null => Ok(None),
            Value::Integer(i) => Ok(Some(*i)),
            // Exact integers only; SUM() over an integer column may come back as a real.
            Value::Real(r) if r.fract() == 0.0 && r.abs() < 9.0e15 => Ok(Some(*r as i64)),
            Value::Real(_) => Err(self.error(column, "expected an integer, found a fraction")),
            Value::Text(_) => Err(self.error(column, "expected an integer, found text")),
        }
    }

    pub fn int(&self, column: &str) -> Result<i64, DbError> {
        self.opt_int(column)?.ok_or_else(|| self.error(column, "unexpected NULL"))
    }

    pub fn opt_real(&self, column: &str) -> Result<Option<f64>, DbError> {
        match self.cell(column)? {
            Value::Null => Ok(None),
            Value::Real(r) => Ok(Some(*r)),
            Value::Integer(i) => Ok(Some(*i as f64)),
            Value::Text(_) => Err(self.error(column, "expected a number, found text")),
        }
    }

    pub fn real(&self, column: &str) -> Result<f64, DbError> {
        self.opt_real(column)?.ok_or_else(|| self.error(column, "unexpected NULL"))
    }

    /// A 0/1 column where NULL means "unknown". Unknown stays unknown.
    pub fn opt_flag(&self, column: &str) -> Result<Option<bool>, DbError> {
        match self.opt_int(column)? {
            None => Ok(None),
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            Some(_) => Err(self.error(column, "expected 0 or 1")),
        }
    }

    /// A 0/1 column that is never NULL.
    pub fn flag(&self, column: &str) -> Result<bool, DbError> {
        self.opt_flag(column)?.ok_or_else(|| self.error(column, "unexpected NULL"))
    }
}

/// A struct that mirrors the columns one query selects.
pub trait FromRow: Sized {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError>;
}

pub fn fetch<T: FromRow>(
    db: &dyn Database,
    name: &'static str,
    sql: &str,
    params: &[Value],
) -> Result<Vec<T>, DbError> {
    let result = db.query(name, sql, params)?;
    let index: HashMap<&str, usize> =
        result.columns.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    result
        .rows
        .iter()
        .map(|values| T::from_row(&Row { query: name, index: &index, values }))
        .collect()
}

pub fn fetch_optional<T: FromRow>(
    db: &dyn Database,
    name: &'static str,
    sql: &str,
    params: &[Value],
) -> Result<Option<T>, DbError> {
    Ok(fetch::<T>(db, name, sql, params)?.into_iter().next())
}

/// The single integer a `SELECT COUNT(*)` returns.
pub fn fetch_count(
    db: &dyn Database,
    name: &'static str,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let result = db.query(name, sql, params)?;
    match result.rows.first().and_then(|row| row.first()) {
        Some(Value::Integer(n)) if *n >= 0 => Ok(*n as u64),
        _ => Err(DbError::Decode {
            query: name,
            column: "COUNT(*)".to_string(),
            message: "expected one non-negative integer".to_string(),
        }),
    }
}
