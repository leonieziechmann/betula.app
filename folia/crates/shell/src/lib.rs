//! The shell of the app: what every page stands in and how the app moves between pages. The chrome
//! (`chrome`: rail, top bar, the navigation of a phone), the frame
//! of a page (`frame`: sidebar, page, panel, the ground after them), the memory of the areas
//! (`tabs`), a click answered in the next frame with the page before held until its data is there
//! (`pending`, `skeleton`), the ground under every page (`ground`), the languages (`languages`),
//! what search engines read (`seo`), the launch screens of iOS (`launch`) and what the document
//! links (`document`). Above the design, the data and the stores, below the widgets and the
//! features: it knows no page of its own (docs/folia/folia-refactor.md §7.4).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod chrome;
pub mod document;
pub mod frame;
pub mod ground;
pub mod i18n;
pub mod languages;
pub mod launch;
pub mod pending;
pub mod seo;
pub mod skeleton;
pub mod tabs;
