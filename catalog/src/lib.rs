//! Read access to the catalog snapshot of Betula (docs/schema-v2.md §3 is the contract).
//!
//! The web server and the browser app share this crate, so there is exactly one place
//! with SQL (`queries`), one typed filter state (`filter`), one set of row structs
//! (`rows`) and one German label per enum code (`labels`). The crate has no I/O of its
//! own: callers hand in a `Database`, rusqlite on the server (`native`, behind the
//! feature of the same name) and sql.js in the browser.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod db;
pub mod exam_reading;
pub mod filter;
pub mod fuzzy;
pub mod graph;
pub mod labels;
pub mod pages;
pub mod plan;
pub mod queries;
pub mod rows;
pub mod rows_detail;
pub mod search;
pub mod url;

#[cfg(any(feature = "native", test))]
pub mod native;

#[cfg(test)]
mod area_fixtures;
#[cfg(test)]
mod tests;

pub use db::{Database, DbError, Value, SCHEMA_VERSION};
pub use filter::CatalogQuery;
