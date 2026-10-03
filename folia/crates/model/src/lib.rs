//! The data contract of Betula's catalog snapshot (docs/radix/schema-v2.md §3): the database seam
//! (`Database`, `Value`, `DbError`, `SCHEMA_VERSION`), the rows the read views give, the codes with
//! their labels, what an id may look like, and the module texts' Markdown reader.
//!
//! The crate has no I/O of its own: callers hand in a `Database`, rusqlite behind the feature
//! `sqlite` (`native`: the web server, the tests) and sql.js in the browser.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod db;
pub mod ids;
pub mod labels;
pub mod rows;
pub mod rows_detail;
pub mod text;

#[cfg(any(feature = "sqlite", test))]
pub mod native;

pub use db::{Database, DbError, Value, SCHEMA_VERSION};
