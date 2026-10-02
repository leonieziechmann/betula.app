//! The probe's worker half: the OPFS file system of SQLite (sahpool) installed in this worker, the
//! snapshot imported into it once, then opened read-only by rusqlite and queried.
use std::cell::RefCell;

use rusqlite::{Connection, OpenFlags};
use sqlite_wasm_vfs::sahpool::{install, OpfsSAHPoolCfg, OpfsSAHPoolUtil};
use wasm_bindgen::prelude::*;

thread_local! {
    static POOL: RefCell<Option<OpfsSAHPoolUtil>> = const { RefCell::new(None) };
    static DB: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

fn err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Installs the OPFS file system as SQLite's default. Fails where another tab holds it.
#[wasm_bindgen]
pub async fn init() -> Result<(), JsValue> {
    let pool = install::<sqlite_wasm_rs::WasmOsCallback>(&OpfsSAHPoolCfg::default(), true).await.map_err(err)?;
    POOL.with(|p| *p.borrow_mut() = Some(pool));
    Ok(())
}

/// Imports `bytes` into SQLite's memory file system as `name`.
#[wasm_bindgen]
pub fn import_memory(name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    sqlite_wasm_rs::MemVfsUtil::<sqlite_wasm_rs::WasmOsCallback>::new().import_db(name, bytes).map_err(err)
}

#[wasm_bindgen]
pub fn has(name: &str) -> Result<bool, JsValue> {
    POOL.with(|p| p.borrow().as_ref().map(|pool| pool.exists(name).map_err(err)).unwrap_or(Ok(false)))
}

#[wasm_bindgen]
pub fn import_db(name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    POOL.with(|p| p.borrow().as_ref().ok_or_else(|| err("no pool")).and_then(|pool| pool.import_db(name, bytes).map_err(err)))
}

/// Opens `name` read-only from OPFS, or from memory (`memory`: the bytes imported into SQLite's
/// memory file system, as sql.js holds them), with `cache_kib` of page cache.
#[wasm_bindgen]
pub fn open(name: &str, memory: bool, cache_kib: u32) -> Result<(), JsValue> {
    let conn = if memory {
        Connection::open_with_flags_and_vfs(name, OpenFlags::SQLITE_OPEN_READ_ONLY, c"memvfs").map_err(err)?
    } else {
        Connection::open_with_flags(name, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?
    };
    conn.execute_batch(&format!("PRAGMA cache_size = -{cache_kib};")).map_err(err)?;
    DB.with(|d| *d.borrow_mut() = Some(conn));
    Ok(())
}

/// Runs `sql` and returns how many rows it gave.
#[wasm_bindgen]
pub fn rows(sql: &str) -> Result<u32, JsValue> {
    DB.with(|d| {
        let d = d.borrow();
        let conn = d.as_ref().ok_or_else(|| err("not open"))?;
        let mut statement = conn.prepare_cached(sql).map_err(err)?;
        let columns = statement.column_count();
        let mut rows = statement.query([]).map_err(err)?;
        let mut n = 0u32;
        while let Some(row) = rows.next().map_err(err)? {
            for i in 0..columns {
                let _ = row.get_ref(i).map_err(err)?;
            }
            n += 1;
        }
        Ok(n)
    })
}
