//! What a visitor keeps in this browser, and the buttons that change it: the marked modules
//! (`bookmarks`), the Studienplan (`studyplan`) and „Mein Studiengang" (`myprogram`). Nothing of
//! it reaches the server (R9, R15). Above the shell, below the widgets and the features
//! (docs/folia/folia-refactor.md §7.4).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod bookmarks;
pub mod i18n;
pub mod myprogram;
pub mod studyplan;
