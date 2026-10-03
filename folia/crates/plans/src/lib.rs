//! The study plans: the rows of a Regelstudienplan and their areas (`plan`, `areas`), its study
//! directions (`variants`), and the Stundenplan's stored documents with the import of a plan
//! (`studyplan`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod areas;
pub mod i18n;
pub mod plan;
pub mod studyplan;
pub mod variants;

#[cfg(any(test, feature = "fixtures"))]
pub mod area_fixtures;
