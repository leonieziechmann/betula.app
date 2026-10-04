//! The study plans: the rows of a Regelstudienplan and their areas (`plan`, `areas`), its study
//! directions (`variants`), the Stundenplan's stored documents with the import of a plan
//! (`studyplan`), and „Mein Studium", the whole study as it will go (`study`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod areas;
pub mod i18n;
pub mod plan;
pub mod study;
pub mod studyplan;
pub mod variants;

#[cfg(any(test, feature = "fixtures"))]
pub mod area_fixtures;
