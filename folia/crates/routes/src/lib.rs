//! Every address of the site: the types of the pages' addresses, reading and writing them, and the
//! catalog's filter as the address writes it (`filter`; its SQL is `folia_query::sql`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod filter;
pub mod local;
pub mod url;

pub use filter::CatalogQuery;
