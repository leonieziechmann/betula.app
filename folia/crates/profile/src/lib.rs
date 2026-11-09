//! The profile page (`profile`): the profiles of this browser and what the active one is and
//! keeps; Betula Sync of the active profile there (`sync`, its QR code `folia_design::qr`); the
//! page where devices find each other (`connect`: the scanner, `scan`); „Mit dem Handy verbinden",
//! where a computer shows the code a phone scans (`pair`); and what a phone asks before it sends a
//! profile there (`give`). A feature of the app
//! (docs/folia/folia-refactor.md §7.4); the profiles themselves are a store
//! (`folia_stores::profiles`), and so is Betula Sync (`folia_stores::sync`).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod confirm;
pub mod connect;
pub mod give;
pub mod i18n;
pub mod pair;
pub mod profile;
pub mod scan;
pub mod sync;
