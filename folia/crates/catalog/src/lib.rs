//! The module catalog (`catalog`): its list, its filters, the module beside it, and the module's
//! own page (`module`). A feature of the app (docs/folia/folia-refactor.md §7.4); its rows, chips and texts are widgets.

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
pub mod catalog;
pub mod module;
