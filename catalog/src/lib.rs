//! Read access to the BTU catalog snapshot (docs/schema-v2.md §3 is the contract).
//!
//! The web server and the browser app share this crate, so there is exactly one place
//! with SQL (`queries`), one typed filter state (`filter`), one set of row structs
//! (`rows`) and one German label per enum code (`labels`). The crate has no I/O of its
//! own: callers hand in a `Database`, rusqlite on the server (`native`, behind the
//! feature of the same name) and sql.js in the browser.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod db;
pub mod filter;
pub mod labels;
pub mod queries;
pub mod rows;

#[cfg(any(feature = "native", test))]
pub mod native;

#[cfg(test)]
mod tests;

pub use db::{Database, DbError, Value};
pub use filter::CatalogQuery;
