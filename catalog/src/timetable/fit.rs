//! The finder („Passt in meinen Plan"): the modules of a semester that fit a Studienplan without
//! taking its last free Übung.
//!
//! A candidate is every module with a dated row in the semester, teaching or exam, built as a plan
//! of its own: its own SWS decide its choices, the visitor's hidden kinds apply, and a module taught
//! as two city courses is taken in the plan's town, or in whichever town fits better when the plan
//! names none. That part does not depend on the plan, so it is built once per semester, compared
//! classes, hidden kinds and town (`CandidateSet`), and a change of the plan re-runs only `fits`.
//!
//! `fits` holds each candidate against what the plan already asks of the visitor's weeks. All its
//! lectures must be free, since every lecture is attended. Of its other events (Übungen, Seminare,
//! Praktika, Tutorien …) one free suffices: QIS does not mark which of a module's events are
//! alternatives of one another, and a false „passt nicht" hides a module for good while a false
//! „passt" costs one look. Of its exam Termine one must avoid the plan's fixed ones (a planned
//! module's only Termin), by the rule the exam warnings use. Only compared classes meet each other,
//! so a lecture is not held against an Übung while „Übungen" is off, and an event the plan already
//! holds is attended once and never clashes.
//!
//! The plan's open choices („1 von 4 wählen") are no obstacle as long as the plan can still take a
//! free option of each. Whether it can is `clash`'s own weighing, run over the plan and the
//! candidate together, so the finder and the Studienplan never disagree: a candidate that leaves
//! each choice one free option, but not all of them at once, does not fit.
//!
//! What cannot be compared is never a conflict and never a fit: a module whose rows have no time
//! in a compared class is kept as unknown („keine festen Termine"). A retake is not compared either:
//! it is sat only after a first attempt, so it says nothing about a module one plans to take.

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use super::clash;
use super::day::Day;
use super::exams::{self, Exam, TerminAt, WarningKind};
use super::facts::SemesterFacts;
use super::kind::{Class, KindSet};
use super::model::{Attendance, Event, Input, Row, Timetable};
use super::select::{FitOptions, Selection, Town, TownChoice};
use super::semester::SemesterKey;
use crate::rows::Semester;
use crate::rows_detail::{DateRow, ModuleSws};

/// The note of a module whose rows have no time to compare.
pub const UNKNOWN_NOTE: &str = "keine festen Termine";

/// The note of a module whose first exam Termin overlaps one the plan holds, while it has another
/// Termin, or whose exam meets the Erstermin of a planned module that has another.
pub const EXAM_OVERLAP_NOTE: &str = "Prüfung überschneidet sich mit Erstermin";

/// The same for a hop between campuses that is too short (`exams::CITY_GAP`, `exams::SITE_GAP`).
pub const EXAM_TIGHT_NOTE: &str = "Prüfung zu knapp am Erstermin";

/// How a module fits the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Everything compared is free.
    Fits,
    /// Nothing compared fails, but not everything is free („Übung 1 von 3 frei").
    Partly,
    /// A compared class fails, or the plan would lose an open choice: the module is not listed.
    Clashes,
    /// Dated rows, but none with a time in a compared class.
    Unknown,
}

/// The verdict on one module, with the note a catalog row shows for a partial fit or an unknown.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub module_id: String,
    pub verdict: Verdict,
    pub note: Option<String>,
}

/// A semester's rows of every module, as the semester queries deliver them (`semester_schedule`,
/// `semester_exams`, `semester_teaching_sws`).
#[derive(Clone, Copy, Debug)]
pub struct Candidates<'a> {
    pub schedule: &'a [DateRow],
    pub exams: &'a [DateRow],
    pub sws: &'a [ModuleSws],
}

/// Every module with a dated row (teaching or exam) in the semester, each built like a plan of
/// one: the part of the finder that does not depend on the plan, kept while its key holds
/// (semester, compared classes, hidden kinds, the plan's effective town).
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateSet {
    pub key: (SemesterKey, FitOptions, KindSet, Option<Town>),
    /// By module id.
    pub modules: Vec<Candidate>,
}

/// One module, reduced to what `fits` compares.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub module_id: String,
    /// One course, or one per town for a module with city tracks while the plan names no town.
    courses: Vec<Course>,
}

/// A module as it is taken in one town, or wherever it is taught.
#[derive(Clone, Debug, PartialEq)]
struct Course {
    /// Every teaching event of the module, shown or not. An event the plan holds as well is the
    /// plan's: attended once, it never clashes with the module's own events.
    ids: BTreeSet<String>,
    /// Its shown events of the compared teaching classes that have a Termin with a time.
    events: Vec<Event>,
    /// Its exam Termine, earliest first, when exams are compared; retakes are left out.
    termine: Vec<TerminAt>,
}

/// Builds every module of the semester's rows as a plan of one. `selection` gives the hidden kinds
/// (the plan's other choices are its own); `town` is the plan's effective town (`Timetable::town`),
/// which decides a module with city tracks, and with none each town's course is kept for `fits` to
/// take the better one. The SWS of a module are its own, as they would be once it is planned.
pub fn candidates(
    input: &Candidates<'_>,
    facts: &SemesterFacts,
    semester: Option<&Semester>,
    selection: &Selection,
    town: Option<Town>,
    options: FitOptions,
) -> CandidateSet {
    let schedule = by_module(input.schedule, |row| row.module_id.as_str());
    let exams = by_module(input.exams, |row| row.module_id.as_str());
    let sws = by_module(input.sws, |row| row.module_id.as_str());
    let ids: BTreeSet<&str> = schedule.keys().chain(exams.keys()).copied().collect();
    let hidden_kinds = selection.hidden_kinds;

    let modules = ids
        .into_iter()
        .map(|id| {
            let module = [id.to_string()];
            let input = Input {
                key: facts.key,
                semester,
                facts,
                modules: &module,
                schedule: part(&schedule, id),
                exams: part(&exams, id),
                sws: part(&sws, id),
            };
            let build =
                |town: TownChoice| Timetable::build(&input, &Selection { hidden_kinds, town, ..Selection::default() });
            let table = build(town.map_or(TownChoice::Both, TownChoice::Only));
            let courses: Vec<Course> = if town.is_none() && !table.tracks.is_empty() {
                [Town::Cottbus, Town::Senftenberg]
                    .into_iter()
                    .map(|t| course(build(TownChoice::Only(t)), options))
                    .collect()
            } else {
                vec![course(table, options)]
            };
            Candidate { module_id: id.to_string(), courses }
        })
        .collect();
    CandidateSet { key: (facts.key, options, hidden_kinds, town), modules }
}

/// The verdict on every candidate that is not planned, in the order of the set. The plan is the
/// semester's `Timetable` with the visitor's selection; the classes compared are the set's.
pub fn fits(plan: &Timetable, set: &CandidateSet) -> Vec<Fit> {
    let options = set.key.1;
    let shown: Vec<(usize, &Event)> = plan
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.hidden.is_none() && compared(event.class, options))
        .collect();
    let (fixed, firsts) = plan_termine(plan, options);
    let context = Context {
        index: plan.events.iter().enumerate().map(|(i, event)| (event.id.as_str(), i)).collect(),
        hard: Busy::of(&shown, |open, row| !(open && row.option.is_some())),
        soft: Busy::of(&shown, |open, row| open && row.option.is_some()),
        fixed,
        firsts,
    };
    let mut choices = Choices::of(&shown);
    let planned: BTreeSet<&str> = plan.modules.iter().map(String::as_str).collect();

    set.modules
        .iter()
        .filter(|candidate| !planned.contains(candidate.module_id.as_str()))
        .map(|candidate| {
            // With two city courses, the one that fits better; at a tie, the first.
            let best = candidate
                .courses
                .iter()
                .map(|course| judge(&candidate.module_id, course, &context, &mut choices))
                .min_by_key(|(verdict, _)| preference(*verdict));
            let (verdict, note) = best.unwrap_or_else(|| (Verdict::Unknown, Some(UNKNOWN_NOTE.to_string())));
            Fit { module_id: candidate.module_id.clone(), verdict, note }
        })
        .collect()
}

/// Whether the finder compares events of `class`.
fn compared(class: Class, options: FitOptions) -> bool {
    match class {
        Class::Lecture => options.lectures,
        Class::Other => options.exercises,
        Class::Exam => options.exams,
    }
}

/// The better of two verdicts comes first: a checked fit before an unknown, an unknown before a
/// clash.
fn preference(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::Fits => 0,
        Verdict::Partly => 1,
        Verdict::Unknown => 2,
        Verdict::Clashes => 3,
    }
}

/// The rows of each module. The semester queries deliver a module's rows in one piece (`ORDER BY
/// module_id`), which is borrowed as it is; rows of one module in several pieces are put together.
fn by_module<'a, T: Clone>(rows: &'a [T], module: fn(&T) -> &str) -> BTreeMap<&'a str, Cow<'a, [T]>> {
    let mut modules: BTreeMap<&'a str, Cow<'a, [T]>> = BTreeMap::new();
    for piece in rows.chunk_by(|a, b| module(a) == module(b)) {
        let Some(first) = piece.first() else {
            continue;
        };
        match modules.entry(module(first)) {
            Entry::Vacant(entry) => {
                entry.insert(Cow::Borrowed(piece));
            }
            Entry::Occupied(mut entry) => entry.get_mut().to_mut().extend_from_slice(piece),
        }
    }
    modules
}

/// A module's rows of `by_module`, none when it has none.
fn part<'m, T: Clone>(modules: &'m BTreeMap<&str, Cow<'_, [T]>>, id: &str) -> &'m [T] {
    modules.get(id).map(|rows| rows.as_ref()).unwrap_or_default()
}

/// What `fits` keeps of a candidate's timetable.
fn course(table: Timetable, options: FitOptions) -> Course {
    let ids = table.events.iter().map(|event| event.id.clone()).collect();
    let events = table
        .events
        .into_iter()
        .filter(|event| event.hidden.is_none() && compared(event.class, options))
        .filter(|event| event.rows.iter().any(|row| placed(row).is_some()))
        .collect();
    let termine = if options.exams {
        let exams: Vec<Exam> = table.exams.into_iter().filter(|exam| !exam.retake).collect();
        exams::termine(&exams, &table.modules).into_iter().flat_map(|(_, termine)| termine).collect()
    } else {
        Vec::new()
    };
    Course { ids, events, termine }
}

/// A shown row's times when it has a day to be compared on: held days, or a pattern of its
/// weekday (`clash` takes a pattern to meet every row of that weekday).
fn placed(row: &Row) -> Option<(u16, u16)> {
    let dated = !row.occ.days.is_empty() || row.occ.template.is_some();
    (row.hidden.is_none() && dated).then_some(row.from.zip(row.to)).flatten()
}

/// The exam Termine of the plan that a candidate is held against: the fixed ones (a module's only
/// Termin, which it cannot avoid) and the first of each module that has another (it could move,
/// but then it no longer sits its Erstermin).
fn plan_termine(plan: &Timetable, options: FitOptions) -> (Vec<TerminAt>, Vec<TerminAt>) {
    let (mut fixed, mut firsts) = (Vec::new(), Vec::new());
    if !options.exams {
        return (fixed, firsts);
    }
    for (_, termine) in exams::termine(&plan.exams, &plan.modules) {
        let mut termine = termine.into_iter();
        match (termine.next(), termine.next()) {
            (Some(only), None) => fixed.push(only),
            (Some(first), Some(_)) => firsts.push(first),
            _ => {}
        }
    }
    (fixed, firsts)
}

/// What `fits` looks a candidate up in, built once per call.
struct Context<'t> {
    /// The plan's events by id, shown or not, to their index.
    index: BTreeMap<&'t str, usize>,
    /// The plan's rows a candidate must not meet: the shown rows of its shown events of the
    /// compared classes, except the options of open choices.
    hard: Busy<'t>,
    /// The options of the plan's open choices: a candidate that meets one may leave a choice
    /// without a free option.
    soft: Busy<'t>,
    /// The plan's fixed exam Termine.
    fixed: Vec<TerminAt>,
    /// The first Termin of each planned module that has another.
    firsts: Vec<TerminAt>,
}

/// A row of the plan in a `Busy`: its event and times.
#[derive(Clone, Copy)]
struct Spot {
    event: usize,
    from: u16,
    to: u16,
}

/// Rows of the plan by their held days, so a candidate's row is looked up day by day.
struct Busy<'t> {
    days: BTreeMap<Day, Vec<Spot>>,
    /// The rows known only by their pattern (no lecture period to place them in).
    patterns: Vec<(Spot, &'t Row)>,
    /// Every row, for a candidate's row that is a pattern itself.
    rows: Vec<(Spot, &'t Row)>,
}

impl<'t> Busy<'t> {
    /// The placed rows of `events` that `keep` takes, given whether their event is an open
    /// choice.
    fn of(events: &[(usize, &'t Event)], keep: fn(bool, &Row) -> bool) -> Self {
        let mut busy = Busy { days: BTreeMap::new(), patterns: Vec::new(), rows: Vec::new() };
        for (index, event) in events {
            let open = event.unresolved();
            for row in &event.rows {
                let Some((from, to)) = placed(row).filter(|_| keep(open, row)) else {
                    continue;
                };
                let spot = Spot { event: *index, from, to };
                busy.rows.push((spot, row));
                if row.occ.template.is_some() {
                    busy.patterns.push((spot, row));
                }
                for day in &row.occ.days {
                    busy.days.entry(*day).or_default().push(spot);
                }
            }
        }
        busy
    }

    /// Whether `row` meets one of these rows as `clash` has two rows meet: a common held day (or
    /// a pattern's weekday) and a strict overlap of times. Rows of the events in `shared` do not
    /// count.
    fn meets(&self, row: &Row, shared: &BTreeSet<usize>) -> bool {
        let Some((from, to)) = placed(row) else {
            return false;
        };
        let overlaps = |spot: &Spot| from < spot.to && spot.from < to && !shared.contains(&spot.event);
        let pattern = |(spot, other): &(Spot, &Row)| overlaps(spot) && clash::shared(other, row).is_some();
        if row.occ.template.is_some() {
            return self.rows.iter().any(pattern);
        }
        row.occ.days.iter().any(|day| self.days.get(day).is_some_and(|spots| spots.iter().any(overlaps)))
            || self.patterns.iter().any(pattern)
    }
}

/// The plan's open choices, weighed by `clash` with a candidate's events added.
struct Choices {
    /// The plan's shown events of the compared classes, as `clash::clashes` takes them.
    base: Vec<Event>,
    /// Their open choices that the plan alone leaves open (not blocked), by index into `base`.
    open: Vec<usize>,
}

impl Choices {
    fn of(shown: &[(usize, &Event)]) -> Self {
        let base: Vec<Event> = shown.iter().map(|(_, event)| (*event).clone()).collect();
        let (_, blocked) = clash::clashes(&base);
        let open = base
            .iter()
            .enumerate()
            .filter(|(i, event)| event.unresolved() && !blocked.contains(i))
            .map(|(i, _)| i)
            .collect();
        Choices { base, open }
    }

    /// Whether planning `module` with its `own` events (those the plan does not hold) blocks one of
    /// the plan's open choices. The plan's events that are the module's too (`ids`) gain it as a
    /// linking module for the time of the check, as they would once it is planned, so they do not
    /// meet its own events.
    fn blocked_by(&mut self, module: &str, ids: &BTreeSet<String>, own: &[&Event]) -> bool {
        if self.open.is_empty() {
            return false;
        }
        let len = self.base.len();
        let mut linked = Vec::new();
        for (i, event) in self.base.iter_mut().enumerate() {
            if ids.contains(&event.id) {
                event.modules.push(module.to_string());
                linked.push(i);
            }
        }
        self.base.extend(own.iter().map(|event| (*event).clone()));
        let (_, blocked) = clash::clashes(&self.base);
        self.base.truncate(len);
        for i in linked {
            if let Some(event) = self.base.get_mut(i) {
                event.modules.pop();
            }
        }
        self.open.iter().any(|choice| blocked.contains(choice))
    }
}

/// Free and taken among a class's events: `units` are what a student picks among (an event, or
/// each option of a choice).
#[derive(Default)]
struct Tally {
    events: usize,
    free_events: usize,
    units: usize,
    free_units: usize,
}

impl Tally {
    /// „Übung 1 von 3 frei" when some but not all units are free.
    fn note(&self, label: &str) -> Option<String> {
        (self.free_units > 0 && self.free_units < self.units)
            .then(|| format!("{label} {} von {} frei", self.free_units, self.units))
    }
}

/// What a candidate's exam Termine say against the plan's.
enum ExamFit {
    /// No Termin to compare.
    Unknown,
    Free,
    /// One is free, but not the first, or the one taken meets a planned module's Erstermin.
    Note(&'static str),
    /// Every Termin collides with a fixed one.
    Fails,
}

/// The verdict on one course of a candidate.
fn judge(module: &str, course: &Course, context: &Context<'_>, choices: &mut Choices) -> (Verdict, Option<String>) {
    // The plan's events that are the module's too: attended once, they never clash.
    let shared: BTreeSet<usize> = course.ids.iter().filter_map(|id| context.index.get(id.as_str()).copied()).collect();
    let (mut lectures, mut others) = (Tally::default(), Tally::default());
    let mut own: Vec<&Event> = Vec::new();
    for event in &course.events {
        let in_plan = context.index.contains_key(event.id.as_str());
        if !in_plan {
            own.push(event);
        }
        let free = |row: &Row| in_plan || !context.hard.meets(row, &shared);
        let (units, free_units, choice) = match &event.attendance {
            Attendance::All => (1, usize::from(event.rows.iter().all(free)), false),
            Attendance::OneOf { .. } => {
                let options = event.visible_options();
                let required = event.rows.iter().filter(|row| row.option.is_none()).all(free);
                let taken = |option: &&usize| event.rows.iter().filter(|row| row.option == Some(**option)).all(free);
                let free_options = if required { options.iter().filter(taken).count() } else { 0 };
                (options.len(), free_options, true)
            }
        };
        let lecture = event.class == Class::Lecture;
        let tally = if lecture { &mut lectures } else { &mut others };
        tally.events += 1;
        tally.free_events += usize::from(free_units > 0);
        // A lecture's units count only for a choice: the others must all be free anyway.
        if !lecture || choice {
            tally.units += units;
            tally.free_units += free_units;
        }
    }

    let exam = exam_fit(&course.termine, context);
    let fails = lectures.free_events < lectures.events
        || (others.events > 0 && others.free_events == 0)
        || matches!(exam, ExamFit::Fails);
    if fails {
        return (Verdict::Clashes, None);
    }
    if lectures.events + others.events == 0 && matches!(exam, ExamFit::Unknown) {
        return (Verdict::Unknown, Some(UNKNOWN_NOTE.to_string()));
    }
    let touches = own.iter().any(|event| event.rows.iter().any(|row| context.soft.meets(row, &shared)));
    if touches && choices.blocked_by(module, &course.ids, &own) {
        return (Verdict::Clashes, None);
    }
    let exam_note = match exam {
        ExamFit::Note(note) => Some(note.to_string()),
        ExamFit::Unknown | ExamFit::Free | ExamFit::Fails => None,
    };
    let notes: Vec<String> =
        [lectures.note("Vorlesung"), others.note("Übung"), exam_note].into_iter().flatten().collect();
    if notes.is_empty() {
        (Verdict::Fits, None)
    } else {
        (Verdict::Partly, Some(notes.join(" · ")))
    }
}

/// A candidate's exam Termine against the plan's: the first that avoids every fixed Termin is the
/// one taken, and when none does, the module fails. When the taken one is not the first, or it
/// meets a planned module's Erstermin, a note names the kind of that collision.
fn exam_fit(termine: &[TerminAt], context: &Context<'_>) -> ExamFit {
    let Some(first) = termine.first() else {
        return ExamFit::Unknown;
    };
    let collision =
        |termin: &TerminAt, against: &[TerminAt]| against.iter().find_map(|other| exams::collision(termin, other));
    let Some(taken) = termine.iter().position(|termin| collision(termin, &context.fixed).is_none()) else {
        return ExamFit::Fails;
    };
    let issue = if taken == 0 { collision(first, &context.firsts) } else { collision(first, &context.fixed) };
    match issue {
        Some(WarningKind::Overlap) => ExamFit::Note(EXAM_OVERLAP_NOTE),
        Some(WarningKind::Tight { .. }) => ExamFit::Note(EXAM_TIGHT_NOTE),
        None => ExamFit::Free,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::Instant;

    use super::*;
    use crate::labels::Code;
    use crate::queries;
    use crate::rows_detail::EventDate;
    use crate::timetable::kind::EventKind;
    use crate::timetable::model::tests::{ids, invariants, sws, teaching, winter, Fixture};

    const ALL: FitOptions = FitOptions { lectures: true, exercises: true, exams: true };
    const NO_LECTURES: FitOptions = FitOptions { lectures: false, ..ALL };
    const NO_EXERCISES: FitOptions = FitOptions { exercises: false, ..ALL };
    const NO_EXAMS: FitOptions = FitOptions { exams: false, ..ALL };

    type Verdicts = Vec<(String, Verdict, Option<String>)>;

    /// The plan of `planned` in the semester of `rows`, as the Studienplan builds it.
    fn plan_of(
        facts: &SemesterFacts,
        rows: &[DateRow],
        exams: &[DateRow],
        sws: &[ModuleSws],
        planned: &[String],
        selection: &Selection,
    ) -> Timetable {
        let input = Input { key: facts.key, semester: None, facts, modules: planned, schedule: rows, exams, sws };
        let table = Timetable::build(&input, selection);
        invariants(&table);
        table
    }

    /// The finder's verdicts on every module of the rows but the planned ones, for a plan of
    /// `planned` with `selection`, in `facts`' semester.
    fn finder_in(
        facts: &SemesterFacts,
        rows: &[Fixture],
        exams: &[DateRow],
        sws: &[ModuleSws],
        planned: &[&str],
        selection: &Selection,
        options: FitOptions,
    ) -> Verdicts {
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let plan = plan_of(facts, &schedule, exams, sws, &ids(planned), selection);
        let set =
            candidates(&Candidates { schedule: &schedule, exams, sws }, facts, None, selection, plan.town, options);
        fits(&plan, &set).into_iter().map(|fit| (fit.module_id, fit.verdict, fit.note)).collect()
    }

    /// The same in 2026W as the data has it.
    fn finder(
        rows: &[Fixture],
        exams: &[DateRow],
        planned: &[&str],
        selection: &Selection,
        options: FitOptions,
    ) -> Verdicts {
        finder_in(&winter(), rows, exams, &[], planned, selection, options)
    }

    fn fit(module: &str, verdict: Verdict, note: Option<&str>) -> (String, Verdict, Option<String>) {
        (module.to_string(), verdict, note.map(str::to_string))
    }

    fn fits_(module: &str) -> (String, Verdict, Option<String>) {
        fit(module, Verdict::Fits, None)
    }

    fn clashes(module: &str) -> (String, Verdict, Option<String>) {
        fit(module, Verdict::Clashes, None)
    }

    fn unknown(module: &str) -> (String, Verdict, Option<String>) {
        fit(module, Verdict::Unknown, Some(UNKNOWN_NOTE))
    }

    fn partly(module: &str, note: &str) -> (String, Verdict, Option<String>) {
        fit(module, Verdict::Partly, Some(note))
    }

    /// The Studienplan's blocked choices after planning `modules`, by event id.
    fn blocked_when_planned(rows: &[Fixture], modules: &[&str]) -> Vec<String> {
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let table = plan_of(&winter(), &schedule, &[], &[], &ids(modules), &Selection::default());
        table.blocked.iter().map(|i| table.events[*i].id.clone()).collect()
    }

    /// Named groups of one Übung, one row each: `(weekday, from, to)`.
    fn groups(module: &str, event: &str, slots: &[(i64, &str, &str)]) -> Vec<Fixture> {
        slots
            .iter()
            .zip(1i64..)
            .map(|((weekday, from, to), ord)| {
                teaching(module, event, ord, "Übung", *weekday, from, to).group(&format!("{ord}-Gruppe"))
            })
            .collect()
    }

    /// An exam row as `semester_exams` delivers it: one day with its times, at `campus`.
    fn exam(module: &str, event: &str, ord: i64, on: &str, from: &str, to: &str, campus: &str) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: format!("Prüfung {event}"),
                event_type: None,
                group_name: None,
                weekday: Day::parse(on).map(|day| i64::from(day.weekday())),
                start_time: Some(from.into()),
                end_time: Some(to.into()),
                rhythm: None,
                rhythm_raw: None,
                first_date: Some(on.into()),
                last_date: Some(on.into()),
                room: None,
                campus: Some(Code::parse(campus)),
                instructor: None,
                comment: None,
                source_url: None,
            },
        }
    }

    #[test]
    fn lectures_must_all_be_free_and_one_other_event_suffices() {
        let mut untimed = teaching("H", "18", 1, "Vorlesung", 3, "09:15", "10:45");
        untimed.0.date.start_time = None;
        let rows = [
            teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("P", "2", 1, "Übung", 2, "09:15", "10:45"),
            // A meets P's lecture; B only touches it.
            teaching("A", "10", 1, "Vorlesung", 1, "10:00", "11:30"),
            teaching("B", "11", 1, "Vorlesung", 1, "10:45", "12:15"),
            // C's Übung meets P's, its Seminar is free; D's only Übung meets it.
            teaching("C", "12", 1, "Übung", 2, "09:15", "10:45"),
            teaching("C", "13", 1, "Seminar", 3, "09:15", "10:45"),
            teaching("D", "14", 1, "Übung", 2, "09:00", "10:00"),
            // E's lecture meets P's Übung.
            teaching("E", "15", 1, "Vorlesung", 2, "09:30", "11:00"),
            // F's groups: Tuesday taken, Thursday free; G's: Monday and Tuesday taken.
            teaching("F", "16", 1, "Übung", 2, "09:15", "10:45").group("1-Gruppe"),
            teaching("F", "16", 2, "Übung", 4, "09:15", "10:45").group("2-Gruppe"),
            teaching("G", "17", 1, "Übung", 1, "09:15", "10:45").group("1-Gruppe"),
            teaching("G", "17", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"),
            untimed,
        ];
        let plan = ["P"];
        let selection = Selection::default();
        assert_eq!(
            finder(&rows, &[], &plan, &selection, ALL),
            [
                clashes("A"),
                fits_("B"),
                partly("C", "Übung 1 von 2 frei"),
                clashes("D"),
                clashes("E"),
                partly("F", "Übung 1 von 2 frei"),
                clashes("G"),
                unknown("H")
            ]
        );
        // Übungen not compared: E's lecture is not held against P's Übung, and a module of Übungen
        // alone has nothing to compare.
        assert_eq!(
            finder(&rows, &[], &plan, &selection, NO_EXERCISES),
            [
                clashes("A"),
                fits_("B"),
                unknown("C"),
                unknown("D"),
                fits_("E"),
                unknown("F"),
                unknown("G"),
                unknown("H")
            ]
        );
        // Lectures not compared: G's Monday is free of P's lecture.
        assert_eq!(
            finder(&rows, &[], &plan, &selection, NO_LECTURES),
            [
                unknown("A"),
                unknown("B"),
                partly("C", "Übung 1 von 2 frei"),
                clashes("D"),
                unknown("E"),
                partly("F", "Übung 1 von 2 frei"),
                partly("G", "Übung 1 von 2 frei"),
                unknown("H")
            ]
        );
    }

    #[test]
    fn an_event_the_plan_holds_never_clashes() {
        let rows = [
            teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            // S shares P's lecture and has its own Übung at that time; T is P's lecture alone.
            teaching("S", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("S", "20", 1, "Übung", 1, "09:15", "10:45"),
            teaching("T", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("U", "21", 1, "Übung", 1, "09:15", "10:45"),
        ];
        assert_eq!(finder(&rows, &[], &["P"], &Selection::default(), ALL), [fits_("S"), fits_("T"), clashes("U")]);
        // The Studienplan agrees: planned with P, S has no clash.
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let table = plan_of(&winter(), &schedule, &[], &[], &ids(&["P", "S"]), &Selection::default());
        assert!(table.clashes.is_empty());
    }

    #[test]
    fn the_plans_open_choices_stay_open() {
        // 148304: an Übung at Mi 09:15, Mi 13:45 and Do 09:15.
        let choice = groups("P", "30", &[(3, "09:15", "10:45"), (3, "13:45", "15:15"), (4, "09:15", "10:45")]);
        let covering = [
            teaching("K", "31", 1, "Vorlesung", 3, "09:15", "10:45"),
            teaching("K", "31", 2, "Vorlesung", 3, "13:45", "15:15"),
            teaching("K", "31", 3, "Vorlesung", 4, "09:15", "10:45"),
        ];
        let others = [
            // L leaves Thursday.
            teaching("L", "32", 1, "Vorlesung", 3, "09:15", "10:45"),
            teaching("L", "32", 2, "Vorlesung", 3, "13:45", "15:15"),
            // M's own choice can take Friday.
            teaching("M", "33", 1, "Übung", 3, "09:15", "10:45").group("1-Gruppe"),
            teaching("M", "33", 2, "Übung", 5, "09:15", "10:45").group("2-Gruppe"),
            // N's three Übungen are each free of the plan's required Termine, but take all three.
            teaching("N", "34", 1, "Übung", 3, "09:15", "10:45"),
            teaching("N", "35", 1, "Übung", 3, "13:45", "15:15"),
            teaching("N", "36", 1, "Übung", 4, "09:15", "10:45"),
        ];
        let rows = [choice, covering.to_vec(), others.to_vec()].concat();
        let selection = Selection::default();
        assert_eq!(finder(&rows, &[], &["P"], &selection, ALL), [clashes("K"), fits_("L"), fits_("M"), clashes("N")]);
        assert_eq!(blocked_when_planned(&rows, &["P", "K"]), ["30"]);
        assert!(blocked_when_planned(&rows, &["P", "L"]).is_empty());
        assert!(blocked_when_planned(&rows, &["P", "M"]).is_empty());
        // With Übungen not compared, the plan's choice is not either.
        assert_eq!(
            finder(&rows, &[], &["P"], &selection, NO_EXERCISES),
            [fits_("K"), fits_("L"), unknown("M"), unknown("N")]
        );
        // A choice the plan made is a required Termin like any other.
        let chosen = Selection { chosen_rows: [rows[0].key()].into(), ..Selection::default() };
        assert_eq!(finder(&rows, &[], &["P"], &chosen, ALL)[1], clashes("L"));
    }

    #[test]
    fn open_choices_are_weighed_together() {
        // X can take Monday or Tuesday, Y Tuesday or Wednesday; their Tuesdays meet.
        let x = groups("P", "40", &[(1, "09:15", "10:45"), (2, "09:15", "10:45")]);
        let y = groups("Q", "41", &[(2, "09:15", "10:45"), (3, "09:15", "10:45")]);
        let candidates = [
            // N takes Wednesday: Y goes to Tuesday, X to Monday.
            teaching("N", "42", 1, "Vorlesung", 3, "09:15", "10:45"),
            // O takes Monday and Wednesday: each choice keeps Tuesday, but not both at once.
            teaching("O", "43", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("O", "43", 2, "Vorlesung", 3, "09:15", "10:45"),
        ];
        let rows = [x, y, candidates.to_vec()].concat();
        assert!(blocked_when_planned(&rows, &["P", "Q"]).is_empty());
        assert_eq!(finder(&rows, &[], &["P", "Q"], &Selection::default(), ALL), [fits_("N"), clashes("O")]);
        assert!(blocked_when_planned(&rows, &["P", "Q", "N"]).is_empty());
        assert!(!blocked_when_planned(&rows, &["P", "Q", "O"]).is_empty());
    }

    #[test]
    fn a_choice_the_plan_cannot_place_is_not_held_against_a_candidate() {
        // P's lecture meets both of R's groups: R is blocked before any candidate comes, and Z,
        // which meets both groups too, takes nothing from it.
        let lecture = [
            teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("P", "1", 2, "Vorlesung", 2, "09:15", "10:45"),
        ];
        let rest = [
            groups("R", "2", &[(1, "10:00", "11:30"), (2, "10:00", "11:30")]),
            vec![
                teaching("Z", "3", 1, "Vorlesung", 1, "11:00", "12:30"),
                teaching("Z", "3", 2, "Vorlesung", 2, "11:00", "12:30"),
            ],
        ]
        .concat();
        let rows = [lecture.to_vec(), rest.clone()].concat();
        assert_eq!(blocked_when_planned(&rows, &["P", "R"]), ["2"]);
        assert_eq!(finder(&rows, &[], &["P", "R"], &Selection::default(), ALL), [fits_("Z")]);
        // Without P, R can take either group, and Z would take both.
        assert_eq!(finder(&rest, &[], &["R"], &Selection::default(), ALL), [clashes("Z")]);
    }

    #[test]
    fn one_exam_termin_must_be_free() {
        let zc = "zentralcampus";
        let sfb = "senftenberg";
        let mut retake = exam("F", "65", 1, "2027-02-08", "11:00", "13:00", zc);
        retake.date.event_title = "Wiederholungsprüfung 65".into();
        let exams = [
            // P's only Termin is fixed; R has two, the first on 10.02.
            exam("P", "50", 1, "2027-02-08", "10:00", "12:00", zc),
            exam("R", "51", 1, "2027-02-10", "10:00", "12:00", zc),
            exam("R", "51", 2, "2027-03-10", "10:00", "12:00", zc),
            exam("A", "60", 1, "2027-02-08", "11:00", "13:00", zc),
            exam("B", "61", 1, "2027-02-08", "11:00", "13:00", zc),
            exam("B", "61", 2, "2027-03-09", "11:00", "13:00", zc),
            exam("C", "62", 1, "2027-02-10", "11:00", "13:00", zc),
            exam("D", "63", 1, "2027-02-08", "12:30", "14:00", sfb),
            exam("E", "64", 1, "2027-02-08", "12:30", "14:00", sfb),
            exam("E", "64", 2, "2027-03-01", "12:30", "14:00", sfb),
            // F's only exam in the winter is a retake.
            retake,
            exam("G", "66", 1, "2027-02-15", "10:00", "12:00", zc),
            // H sits P's exam with it: one exam, not two.
            exam("H", "50", 1, "2027-02-08", "10:00", "12:00", zc),
            // I's is QIS's placeholder: no Termin.
            exam("I", "67", 1, "2015-12-27", "01:00", "02:30", zc),
        ];
        let expected = [
            clashes("A"),
            partly("B", EXAM_OVERLAP_NOTE),
            partly("C", EXAM_OVERLAP_NOTE),
            clashes("D"),
            partly("E", EXAM_TIGHT_NOTE),
            unknown("F"),
            fits_("G"),
            fits_("H"),
            unknown("I"),
        ];
        assert_eq!(finder(&[], &exams, &["P", "R"], &Selection::default(), ALL), expected);
        let off: Vec<_> = expected.iter().map(|(module, ..)| unknown(module)).collect();
        assert_eq!(finder(&[], &exams, &["P", "R"], &Selection::default(), NO_EXAMS), off);
        // Exams hidden as a kind: the plan's and the candidates' alike.
        let hidden = Selection { hidden_kinds: KindSet::default().with(EventKind::Exam), ..Selection::default() };
        assert_eq!(finder(&[], &exams, &["P", "R"], &hidden, ALL), off);
        // A teaching Termin and an exam are never compared: G's lecture on P's exam day is free.
        let rows = [teaching("G", "70", 1, "Vorlesung", 1, "10:00", "12:00").range("2027-02-08", "2027-02-08")];
        let only_g: Vec<DateRow> =
            exams.iter().filter(|row| ["P", "G"].contains(&row.module_id.as_str())).cloned().collect();
        assert_eq!(finder(&rows, &only_g, &["P"], &Selection::default(), ALL), [fits_("G")]);
    }

    #[test]
    fn a_module_in_two_towns_is_taken_where_the_plan_is() {
        let sfb = Some("senftenberg");
        let rows = [
            teaching("C", "70", 1, "Vorlesung", 2, "09:15", "10:45"),
            // T: a whole course in each town; Cottbus' lecture meets C's.
            teaching("T", "71", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("T", "72", 1, "Übung", 3, "09:15", "10:45"),
            teaching("T", "73", 1, "Vorlesung", 1, "11:30", "13:00").campus(sfb),
            teaching("T", "74", 1, "Übung", 4, "09:15", "10:45").campus(sfb),
        ];
        let town = |choice: TownChoice| Selection { town: choice, ..Selection::default() };
        let verdict = |choice| finder(&rows, &[], &["C"], &town(choice), ALL);
        // The plan's town, derived or chosen, takes T's course there.
        assert_eq!(verdict(TownChoice::Derive), [clashes("T")]);
        assert_eq!(verdict(TownChoice::Only(Town::Cottbus)), [clashes("T")]);
        assert_eq!(verdict(TownChoice::Only(Town::Senftenberg)), [fits_("T")]);
        // Both towns: the course that fits.
        assert_eq!(verdict(TownChoice::Both), [fits_("T")]);
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let input = Candidates { schedule: &schedule, exams: &[], sws: &[] };
        let courses = |town| {
            let set = candidates(&input, &winter(), None, &Selection::default(), town, ALL);
            set.modules.iter().map(|c| (c.module_id.clone(), c.courses.len())).collect::<Vec<_>>()
        };
        assert_eq!(courses(None), [("C".to_string(), 1), ("T".to_string(), 2)]);
        assert_eq!(courses(Some(Town::Cottbus)), [("C".to_string(), 1), ("T".to_string(), 1)]);
    }

    #[test]
    fn hidden_kinds_and_events_are_not_compared() {
        let rows = [
            teaching("P", "80", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("P", "81", 1, "Tutorium", 2, "09:15", "10:45"),
            // V's Tutorium meets P's lecture, its Übung is free.
            teaching("V", "82", 1, "Übung", 3, "09:15", "10:45"),
            teaching("V", "83", 1, "Tutorium", 1, "09:30", "11:00"),
            // W's only Übung meets P's Tutorium.
            teaching("W", "84", 1, "Übung", 2, "09:15", "10:45"),
        ];
        assert_eq!(
            finder(&rows, &[], &["P"], &Selection::default(), ALL),
            [partly("V", "Übung 1 von 2 frei"), clashes("W")]
        );
        let no_tutorials =
            Selection { hidden_kinds: KindSet::default().with(EventKind::Tutorial), ..Selection::default() };
        assert_eq!(finder(&rows, &[], &["P"], &no_tutorials, ALL), [fits_("V"), fits_("W")]);
        let hidden = Selection { hidden_events: [81].into(), ..Selection::default() };
        assert_eq!(finder(&rows, &[], &["P"], &hidden, ALL), [partly("V", "Übung 1 von 2 frei"), fits_("W")]);
        let hidden = Selection { hidden_rows: [rows[1].key()].into(), ..Selection::default() };
        assert_eq!(finder(&rows, &[], &["P"], &hidden, ALL)[1], fits_("W"));
    }

    #[test]
    fn patterns_meet_as_the_studienplan_has_them_meet() {
        // Without a lecture period, rows without a range are patterns of their weekday.
        let facts = SemesterFacts::derive(SemesterKey { year: 2026, winter: true }, None, &[]);
        let rows = [
            teaching("P", "90", 1, "Vorlesung", 1, "09:15", "10:45").undated(),
            teaching("P", "94", 1, "Vorlesung", 4, "09:15", "10:45").rhythm("week_a").undated(),
            teaching("A", "91", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("B", "92", 1, "Vorlesung", 1, "10:00", "11:30").undated(),
            teaching("C", "93", 1, "Vorlesung", 2, "09:15", "10:45").undated(),
            teaching("E", "95", 1, "Vorlesung", 4, "09:15", "10:45").rhythm("week_b").undated(),
            teaching("F", "96", 1, "Vorlesung", 4, "09:15", "10:45").undated(),
        ];
        let verdicts = finder_in(&facts, &rows, &[], &[], &["P"], &Selection::default(), ALL);
        assert_eq!(verdicts, [clashes("A"), clashes("B"), fits_("C"), fits_("E"), clashes("F")]);
    }

    #[test]
    fn the_set_is_the_plans_to_compare_against_and_its_own_sws_decide() {
        // Q's Übung at four times is a choice by its own 2 SWS (148369), so one free time is
        // enough; with 4 SWS all four are required and two of them meet P's lecture.
        let rows = [
            teaching("P", "1", 1, "Vorlesung", 1, "15:30", "17:00"),
            teaching("Q", "2", 1, "Übung", 1, "15:30", "17:00"),
            teaching("Q", "2", 2, "Übung", 1, "17:30", "19:00"),
            teaching("Q", "2", 3, "Übung", 2, "15:30", "17:00"),
            teaching("Q", "2", 4, "Übung", 2, "17:30", "19:00"),
        ];
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let two = [sws("Q", "exercise", 2.0)];
        let four = [sws("Q", "exercise", 4.0)];
        let with = |sws: &[ModuleSws], planned: &[&str]| {
            finder_in(&winter(), &rows, &[], sws, planned, &Selection::default(), ALL)
        };
        assert_eq!(with(&two, &["P"]), [partly("Q", "Übung 3 von 4 frei")]);
        assert_eq!(with(&four, &["P"]), [clashes("Q")]);
        // One set serves every plan of the semester; a planned module is no candidate.
        let input = Candidates { schedule: &schedule, exams: &[], sws: &two };
        let hidden_kinds = KindSet::default().with(EventKind::Seminar);
        let selection = Selection { hidden_kinds, ..Selection::default() };
        let set = candidates(&input, &winter(), None, &selection, Some(Town::Cottbus), NO_EXAMS);
        assert_eq!(set.key, (winter().key, NO_EXAMS, hidden_kinds, Some(Town::Cottbus)));
        assert_eq!(set.modules.iter().map(|c| c.module_id.as_str()).collect::<Vec<_>>(), ["P", "Q"]);
        let plan = |planned: &[&str]| plan_of(&winter(), &schedule, &[], &two, &ids(planned), &selection);
        let verdicts = |planned: &[&str]| -> Vec<(String, Verdict)> {
            fits(&plan(planned), &set).into_iter().map(|fit| (fit.module_id, fit.verdict)).collect()
        };
        assert_eq!(verdicts(&["P"]), [("Q".to_string(), Verdict::Partly)]);
        assert_eq!(verdicts(&["Q"]), [("P".to_string(), Verdict::Fits)]);
        assert_eq!(verdicts(&[]), [("P".to_string(), Verdict::Fits), ("Q".to_string(), Verdict::Fits)]);
        // Rows of one module in two pieces are one module.
        let apart = [&schedule[1..3], &schedule[..1], &schedule[3..]].concat();
        let pieces = candidates(
            &Candidates { schedule: &apart, ..input },
            &winter(),
            None,
            &selection,
            Some(Town::Cottbus),
            NO_EXAMS,
        );
        assert_eq!(pieces, set);
    }

    /// The design's pinned plan: Informatik's first semester with 12102's second offering at
    /// Sachsendorf hidden (H.2), every class compared. On any snapshot, what holds for every plan:
    /// the finder and the Studienplan agree about the plan's open choices.
    #[test]
    fn the_finder_lists_what_was_checked_and_fits() {
        let pinned = crate::tests::studyplan_db("the_finder_lists_what_was_checked_and_fits");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let semester =
            if is_pinned { "2026W".to_string() } else { queries::meta(&db).unwrap().current_semester.unwrap() };
        let semesters = queries::semesters(&db).unwrap();
        let row = semesters.iter().find(|s| s.key == semester);
        let key = SemesterKey::parse(&semester).unwrap();
        let facts = SemesterFacts::derive(key, row, &queries::semester_date_counts(&db, &semester).unwrap());
        let schedule = queries::semester_schedule(&db, &semester).unwrap();
        let exams = queries::semester_exams(&db, &semester).unwrap();
        let sws = queries::semester_teaching_sws(&db, &semester).unwrap();

        let fs1 = ids(&["12104", "12107", "12102", "11112"]);
        let selection = Selection { hidden_events: [149408, 148455].into(), ..Selection::default() };
        let plan_input =
            Input { key, semester: row, facts: &facts, modules: &fs1, schedule: &schedule, exams: &exams, sws: &sws };
        let plan = Timetable::build(&plan_input, &selection);
        let input = Candidates { schedule: &schedule, exams: &exams, sws: &sws };
        let started = Instant::now();
        let set = candidates(&input, &facts, row, &selection, plan.town, ALL);
        let built = started.elapsed();
        let started = Instant::now();
        let verdicts = fits(&plan, &set);
        let checked = started.elapsed();
        let _ = writeln!(
            std::io::stderr(),
            "fit: {} candidates of {semester} built in {built:?}, checked against FS1 in {checked:?}",
            set.modules.len()
        );

        // The candidates are exactly the modules with a dated row; the planned ones get no verdict.
        let dated: BTreeSet<&str> = schedule.iter().chain(&exams).map(|r| r.module_id.as_str()).collect();
        let listed: BTreeSet<&str> = set.modules.iter().map(|c| c.module_id.as_str()).collect();
        assert_eq!(listed, dated);
        let judged: BTreeSet<&str> = verdicts.iter().map(|f| f.module_id.as_str()).collect();
        assert_eq!(judged, dated.iter().copied().filter(|id| !fs1.iter().any(|m| m == id)).collect());
        for f in &verdicts {
            match f.verdict {
                Verdict::Fits | Verdict::Clashes => assert_eq!(f.note, None, "{}", f.module_id),
                Verdict::Partly => assert!(f.note.is_some(), "{}", f.module_id),
                Verdict::Unknown => assert_eq!(f.note.as_deref(), Some(UNKNOWN_NOTE), "{}", f.module_id),
            }
        }
        // The Studienplan with a listed candidate planned too has no open choice more blocked
        // than the plan alone, and none of the listed module's lectures is in a hard clash. Both
        // hold while the town stays (the module's own events may tip a derived one) and, for the
        // lectures, while the plan blocks no choice (a blocked choice's options are no obstacle to
        // the finder, but the Studienplan names their clashes).
        let open: BTreeSet<String> = plan.events.iter().filter(|e| e.unresolved()).map(|e| e.id.clone()).collect();
        let blocked: BTreeSet<String> = plan.blocked.iter().map(|i| plan.events[*i].id.clone()).collect();
        for f in
            verdicts.iter().filter(|f| f.verdict != Verdict::Clashes).take(if is_pinned { usize::MAX } else { 200 })
        {
            let modules = [fs1.clone(), vec![f.module_id.clone()]].concat();
            let both = Timetable::build(&Input { modules: &modules, ..plan_input }, &selection);
            if both.town != plan.town {
                assert!(!is_pinned, "{}: the town moved", f.module_id);
                continue;
            }
            let now: BTreeSet<String> = both.blocked.iter().map(|i| both.events[*i].id.clone()).collect();
            assert!(now.iter().all(|id| blocked.contains(id) || !open.contains(id)), "{}: {now:?}", f.module_id);
            for (event, _) in clash::hard_rows(&both.events).into_iter().filter(|_| blocked.is_empty()) {
                let e = &both.events[event];
                let theirs = e.modules.iter().all(|m| *m == f.module_id);
                assert!(!(theirs && e.class == Class::Lecture), "{}: {} in a hard clash", f.module_id, e.id);
            }
        }
        if !is_pinned {
            return;
        }

        let verdict = |verdicts: &[Fit], id: &str| verdicts.iter().find(|f| f.module_id == id).map(|f| f.verdict);
        assert_eq!(plan.town, Some(Town::Cottbus));
        assert!(plan.blocked.is_empty() && plan.clashes.is_empty());
        // Analysis I's lecture meets 12107's on Tuesdays, Theoretische Informatik's meets
        // 148701/1, Deutsch als Fremdsprache's Übung the Tutorium 150132.
        assert_eq!(verdict(&verdicts, "11103"), Some(Verdict::Clashes));
        assert_eq!(verdict(&verdicts, "11787"), Some(Verdict::Clashes));
        assert_eq!(verdict(&verdicts, "13583"), Some(Verdict::Clashes));
        // Datenbanken's Monday Übung takes only one of 148369's four times.
        assert_eq!(verdict(&verdicts, "12330"), Some(Verdict::Fits));
        // Algorithmieren und Programmieren has only a retake in the winter.
        assert_eq!(verdict(&verdicts, "12101"), Some(Verdict::Unknown));
        // Grundlagen der Rechnernetze has no dated row in 2026W.
        assert_eq!(verdict(&verdicts, "11454"), None);
        let lenient = candidates(&input, &facts, row, &selection, plan.town, NO_EXERCISES);
        let without = fits(&plan, &lenient);
        assert_ne!(verdict(&without, "13583"), Some(Verdict::Clashes));
        assert_eq!(verdict(&without, "11103"), Some(Verdict::Clashes));
    }
}
