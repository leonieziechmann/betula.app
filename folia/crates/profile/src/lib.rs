//! The profile page (`profile`): the profiles of this browser, what the active one keeps, and the
//! settings every profile shares; Betula Sync of the active profile there (`sync`, its QR code
//! `folia_design::qr`), the page a link of another device opens (`connect`), and „Mit dem Handy
//! verbinden", where a computer shows the code a phone scans (`pair`). A feature of the app
//! (docs/folia/folia-refactor.md §7.4); the profiles themselves are a store
//! (`folia_stores::profiles`), and so is Betula Sync (`folia_stores::sync`).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod connect;
pub mod i18n;
pub mod pair;
pub mod profile;
pub mod sync;
