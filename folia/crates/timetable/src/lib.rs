//! The timetable of a Studienplan: what the planned modules of one calendar semester ask of a
//! visitor's weeks.
//!
//! From the date rows of the planned modules it works out every date of a row, which slots are
//! alternatives of one another, which town's course a module is taken in, what overlaps, which
//! exams sit too close, which other modules still fit, and the calendar a visitor downloads or
//! subscribes to. Everything is a function of rows the caller loaded, without I/O and without a
//! clock, so the server's calendar feed and the browser compute the same answer.
//!
//! The days, semesters, kinds and selections it works on are `folia-calendar`'s.
//!
//! No re-exports: callers name the module (`folia_timetable::model::Timetable`), so no two names
//! can collide and this file stays a list.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod clash;
pub mod exam_reading;
pub mod exams;
pub mod export;
pub mod facts;
pub mod fit;
pub mod grid;
pub mod i18n;
pub mod ics;
pub mod model;
pub mod occur;
pub mod views;
