//! The Studienplan (`studyplan`): the semesters, the week, the exams, the export; and „Mein
//! Studium" (`study`), the whole study semester by semester, the first page of the Studium tab. A
//! feature of the app (docs/folia/folia-refactor.md §7.4); the plan itself is a store.

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
pub mod study;
pub mod studyplan;
