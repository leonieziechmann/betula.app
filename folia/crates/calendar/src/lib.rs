//! The calendar of a Studienplan: days and holidays, semesters, the kinds of events, the keys of a
//! plan's rows, a plan's selection, cancelled dates, and the codes of a shared plan and of a
//! calendar subscription. What the addresses and the filter need of the timetable; its engine is
//! `folia-timetable`.
//!
//! No re-exports: callers name the module (`folia_calendar::semester::SemesterKey`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod cancel;
pub mod day;
pub mod i18n;
pub mod kind;
pub mod rowkey;
pub mod select;
pub mod semester;
pub mod share;
pub mod subscription;
