//! A Notenübersicht taken into „Mein Studium" (owner, 2026-11-18: „die Option den Studiengang aus
//! der Notenübersicht automatisch befüllen zu lassen"): each of its modules into the semester it
//! names, as the student would put it there by hand. Passed („BE") planned there and marked passed;
//! not passed („NB", „EN") planned there and not marked, which makes it what was not passed in a
//! semester that is over (a Wiederholer, `study`); registered („AN") planned where it is under way:
//! in its semester, or in the current one where that is over (a thesis registered in the summer and
//! handed in in November is under way now, not failed). A module recognised from elsewhere, without
//! a semester, goes into the Studienbeginn.
//!
//! The Notenübersicht writes each module's last attempt. Where that is its first, the module was
//! taken in no semester before it: an entry of it in an earlier one (the Regelstudienplan's guess,
//! `study::fill_past`) is moved into the semester it was taken in, with what it counts for, and any
//! other earlier one goes. From the second attempt on the earlier entries stay: they are the
//! attempts before. After a module passed, an entry of it in a later semester goes (nothing is left
//! to take); after one not passed it stays (the next attempt). A mark „bestanden" the Notenübersicht
//! does not have is taken away. Nothing else of the study is touched: modules the Notenübersicht does
//! not list, the plan's placeholders, the visits, what the Stundenplan has chosen.

use folia_calendar::semester::SemesterKey;
use folia_transcript::Status;

use crate::studyplan::PlanDoc;

/// A row of the Notenübersicht as the study takes it: its module in the catalog, its status, the
/// semester it names and its attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub module_id: String,
    pub status: Status,
    pub term: Option<SemesterKey>,
    pub attempt: Option<u8>,
}

/// What a row did to the study.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Taken {
    /// The semester it is in now.
    pub semester: Option<SemesterKey>,
    /// It was so already: in that semester alone, with the mark it has.
    pub kept: bool,
    /// The semester it was moved from.
    pub moved_from: Option<SemesterKey>,
    /// The other semesters it was taken out of.
    pub removed: Vec<SemesterKey>,
    /// Its mark „bestanden" taken away (or moved from another semester).
    pub unmarked: bool,
    /// Not put in: the semester or the study is full (the caps of `PlanDoc::plan`).
    pub full: bool,
}

/// The semester a row goes into: `now` the current one, `start` the Studienbeginn.
pub fn semester_of(row: &Row, now: SemesterKey, start: SemesterKey) -> SemesterKey {
    let named = row.term.unwrap_or(start);
    match row.status {
        Status::Registered => named.max(now),
        _ => named,
    }
}

/// Takes `rows` into `doc`, `at` the time it is done (seconds since 1970); what each row did, in
/// their order. A row of a status the legend does not have is left out (`None`).
pub fn take(doc: &mut PlanDoc, rows: &[Row], now: SemesterKey, start: SemesterKey, at: u64) -> Vec<Option<Taken>> {
    rows.iter().map(|row| take_row(doc, row, now, start, at)).collect()
}

fn take_row(doc: &mut PlanDoc, row: &Row, now: SemesterKey, start: SemesterKey, at: u64) -> Option<Taken> {
    if matches!(row.status, Status::Other(_)) {
        return None;
    }
    let id = row.module_id.as_str();
    let s = semester_of(row, now, start);
    let passed = row.status == Status::Passed;
    let first = row.attempt.is_none_or(|attempt| attempt <= 1);
    let mark = doc.passed_in(id);
    let planned = doc.planned_in(id);
    let here = planned.contains(&s);
    // What does not belong: an earlier entry of a first attempt, a later one of a module passed.
    let earlier: Vec<SemesterKey> = planned.iter().copied().filter(|p| first && *p < s).collect();
    let later: Vec<SemesterKey> = planned.iter().copied().filter(|p| passed && *p > s).collect();
    let mut taken = Taken { semester: Some(s), ..Taken::default() };
    if !here {
        // The entry nearest before it, else the nearest after it, is the one taken there.
        match earlier.iter().max().or(later.iter().min()).copied() {
            Some(from) => {
                doc.move_to(from, s, id);
                if doc.is_planned(s, id) {
                    taken.moved_from = Some(from);
                }
            }
            None => {
                doc.plan(s, id, at, None);
            }
        }
    }
    for other in earlier.iter().chain(&later) {
        if doc.is_planned(*other, id) && doc.is_planned(s, id) {
            doc.unplan(*other, id, &[]);
            taken.removed.push(*other);
        }
    }
    if passed {
        // Its mark is enough to show it passed there, should the semester be full.
        doc.pass(s, id);
    } else if mark.is_some() {
        doc.unpass(id);
    }
    taken.full = !doc.is_planned(s, id) && !(passed && doc.passed_in(id) == Some(s));
    taken.unmarked = mark.is_some_and(|mark| !passed || mark != s);
    taken.kept = here && taken.removed.is_empty() && !taken.unmarked && (!passed || mark == Some(s));
    Some(taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn row(id: &str, status: Status, term: &str, attempt: u8) -> Row {
        Row { module_id: id.into(), status, term: SemesterKey::parse(term), attempt: Some(attempt) }
    }

    const NOW: &str = "2026W";
    const START: &str = "2022W";

    fn take_one(doc: &mut PlanDoc, row: Row) -> Taken {
        take(doc, &[row], key(NOW), key(START), 7).remove(0).unwrap()
    }

    #[test]
    fn passed_is_planned_and_marked_in_its_semester() {
        let mut doc = PlanDoc::default();
        let taken = take_one(&mut doc, row("12330", Status::Passed, "2024W", 1));
        assert_eq!(doc.planned_in("12330"), vec![key("2024W")]);
        assert_eq!(doc.passed_in("12330"), Some(key("2024W")));
        assert_eq!(taken, Taken { semester: Some(key("2024W")), ..Taken::default() });
        // Again: nothing changes.
        let again = take_one(&mut doc.clone(), row("12330", Status::Passed, "2024W", 1));
        assert!(again.kept);
    }

    #[test]
    fn not_passed_is_planned_and_not_marked() {
        let mut doc = PlanDoc::default();
        doc.plan(key("2023S"), "11113", 1, None);
        doc.pass(key("2023S"), "11113");
        let taken = take_one(&mut doc, row("11113", Status::Failed, "2023S", 1));
        assert_eq!(doc.planned_in("11113"), vec![key("2023S")]);
        assert_eq!(doc.passed_in("11113"), None);
        assert!(taken.unmarked && !taken.kept);
    }

    #[test]
    fn registered_in_a_semester_that_is_over_is_under_way_now() {
        let mut doc = PlanDoc::default();
        let taken = take_one(&mut doc, row("12333", Status::Registered, "2026S", 1));
        assert_eq!(doc.planned_in("12333"), vec![key(NOW)]);
        assert_eq!(taken.semester, Some(key(NOW)));
        // One registered for a semester to come stays there.
        take_one(&mut doc, row("12204", Status::Registered, "2027S", 1));
        assert_eq!(doc.planned_in("12204"), vec![key("2027S")]);
    }

    #[test]
    fn a_first_attempt_moves_the_plans_guess_into_its_semester() {
        let mut doc = PlanDoc::default();
        // The Regelstudienplan put it into the 3rd FS, it was taken in the 5th; and once more later.
        doc.plan(key("2023W"), "12341", 1, None);
        doc.plan(key("2024S"), "12341", 1, None);
        let taken = take_one(&mut doc, row("12341", Status::Passed, "2024W", 1));
        assert_eq!(doc.planned_in("12341"), vec![key("2024W")]);
        assert_eq!(taken.moved_from, Some(key("2024S")));
        assert_eq!(taken.removed, vec![key("2023W")]);
    }

    #[test]
    fn a_second_attempt_keeps_the_one_before() {
        let mut doc = PlanDoc::default();
        doc.plan(key("2023W"), "11787", 1, None);
        take_one(&mut doc, row("11787", Status::Passed, "2024W", 2));
        assert_eq!(doc.planned_in("11787"), vec![key("2023W"), key("2024W")]);
        assert_eq!(doc.passed_in("11787"), Some(key("2024W")));
    }

    #[test]
    fn after_passing_a_later_plan_goes_after_failing_it_stays() {
        let mut doc = PlanDoc::default();
        doc.plan(key("2027S"), "11918", 1, None);
        doc.plan(key("2027S"), "11811", 1, None);
        let passed = take_one(&mut doc, row("11918", Status::Passed, "2025S", 1));
        assert_eq!(doc.planned_in("11918"), vec![key("2025S")]);
        assert_eq!(passed.moved_from, Some(key("2027S")));
        take_one(&mut doc, row("11811", Status::Failed, "2025S", 1));
        assert_eq!(doc.planned_in("11811"), vec![key("2025S"), key("2027S")]);
    }

    #[test]
    fn recognised_without_a_semester_goes_into_the_studienbeginn() {
        let mut doc = PlanDoc::default();
        let taken = take_one(&mut doc, Row { module_id: "11684".into(), status: Status::Passed, term: None, attempt: None });
        assert_eq!(taken.semester, Some(key(START)));
        assert_eq!(doc.passed_in("11684"), Some(key(START)));
    }

    #[test]
    fn a_status_the_legend_does_not_have_is_left_out() {
        let mut doc = PlanDoc::default();
        assert_eq!(take(&mut doc, &[row("12330", Status::Other("XY".into()), "2024W", 1)], key(NOW), key(START), 7), vec![None]);
        assert!(doc.is_empty());
    }
}
