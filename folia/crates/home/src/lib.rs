//! The start page (`home`) and the legal pages (`legal`): a feature of the app (docs/folia/folia-refactor.md §7.4).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
pub mod home;
pub mod legal;
