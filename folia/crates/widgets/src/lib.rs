//! The widgets the pages share: a module's view (`module`: its page, beside a list, in place of a
//! page, `local`), a module as a row of a list (`list`), the chips and choices of the filters
//! (`choices`), the finder's memory (`finder`), a row swiped on a phone (`swipe`), the week of a
//! timetable (`week`), and what lies over a page: the menus at a module's switches (`collect`,
//! `planinto` and `planmenu` for „Einplanen" not planned and planned), the notes after them
//! (`marknote`, `plannote`). Above the stores, below the features
//! (docs/folia/folia-refactor.md §7.4).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod choices;
pub mod collect;
mod courses;
pub mod finder;
pub mod i18n;
pub mod list;
pub mod local;
pub mod marknote;
pub mod module;
mod options;
pub mod planinto;
pub mod planmenu;
pub mod plannote;
pub mod plansetup;
pub mod swipe;
pub mod termin;
pub mod week;
