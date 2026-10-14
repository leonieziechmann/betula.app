//! The profile page (`profile`): the profiles of this browser, what the active one keeps, and the
//! settings every profile shares. A feature of the app (docs/folia/folia-refactor.md §7.4); the
//! profiles themselves are a store (`folia_stores::profiles`).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
pub mod profile;
