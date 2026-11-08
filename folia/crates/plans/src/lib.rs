//! The study plans: the rows of a Regelstudienplan and their areas (`plan`, `areas`), its study
//! directions (`variants`), the Stundenplan's stored documents with the import of a plan
//! (`studyplan`) with what it shares and follows (`shares`), and „Mein Studium", the study planned
//! semester by semester with its progress by area (`study`). New numbers come from the device's
//! own class while a profile syncs (`ids`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod areas;
pub mod bench;
pub mod choices;
pub mod i18n;
pub mod ids;
pub mod plan;
pub mod shares;
pub mod study;
pub mod studyplan;
pub mod variants;

#[cfg(any(test, feature = "fixtures"))]
pub mod area_fixtures;
