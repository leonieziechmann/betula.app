//! The widgets the pages share: a module's view (`module`: its page, beside a list, in place of a
//! page, `local`), a module as a row of a list (`list`), the chips and choices of the filters
//! (`choices`), the finder's memory (`finder`), a row swiped on a phone (`swipe`) and the week of
//! a timetable (`week`). Above the stores, below the features (docs/folia/folia-refactor.md §7.4).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod choices;
pub mod finder;
pub mod i18n;
pub mod list;
pub mod local;
pub mod module;
pub mod swipe;
pub mod week;
