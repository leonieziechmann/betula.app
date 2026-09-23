//! `Database` on top of rusqlite: the web server's side of the seam, and the tests'.

use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use crate::db::{Database, DbError, Rows, Value};

/// A snapshot file opened read-only. Snapshots are replaced, never modified.
pub struct NativeDatabase {
    conn: Connection,
}

impl NativeDatabase {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)
            .map_err(|e| DbError::Unavailable(format!("{}: {e}", path.display())))?;
        // Fails here, not on the first page view, if the file is not a catalog snapshot.
        conn.query_row("SELECT COUNT(*) FROM v_meta", [], |row| row.get::<_, i64>(0))
            .map_err(|e| DbError::Unavailable(format!("{}: not a catalog snapshot: {e}", path.display())))?;
        Ok(Self { conn })
    }

    /// The schema of the file: its `PRAGMA user_version`, the number of the last migration of
    /// Radix it was built with (`crate::SCHEMA_VERSION` is the one the queries need).
    pub fn schema_version(&self) -> Result<i64, DbError> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| DbError::Sql { query: "schema_version", message: e.to_string() })
    }
}

impl Database for NativeDatabase {
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
        let fail = |e: rusqlite::Error| DbError::Sql { query: name, message: e.to_string() };

        let mut stmt = self.conn.prepare_cached(sql).map_err(fail)?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let bound = rusqlite::params_from_iter(params.iter().map(|p| match p {
            Value::Null => rusqlite::types::Value::Null,
            Value::Integer(i) => rusqlite::types::Value::Integer(*i),
            Value::Real(r) => rusqlite::types::Value::Real(*r),
            Value::Text(s) => rusqlite::types::Value::Text(s.clone()),
        }));

        let mut rows = Vec::new();
        let mut cursor = stmt.query(bound).map_err(fail)?;
        while let Some(row) = cursor.next().map_err(fail)? {
            let mut values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                values.push(match row.get_ref(i).map_err(fail)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => Value::Integer(n),
                    ValueRef::Real(r) => Value::Real(r),
                    ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Blob(_) => {
                        return Err(DbError::Decode {
                            query: name,
                            column: columns.get(i).cloned().unwrap_or_default(),
                            message: "unexpected BLOB".to_string(),
                        })
                    }
                });
            }
            rows.push(values);
        }
        Ok(Rows { columns, rows })
    }
}
