//! The changes of Termine of QIS in the timetable (`Selection::changes`; folia/design/kalender/
//! termine.html, its second step; owner, 2026-11-03: „Passt, bau Schritt 2 jetzt in Folia").
//!
//! A change keeps its Termin's key on every row it makes, so the group stays ticked, a hidden date
//! stays hidden and the calendar entry keeps its UID. For every date it rewrites the Termin's rows
//! in place; from a date on it ends them the day before and adds a row for the rest; on one date it
//! takes that day out of its row (`Occurrences::moved`) and adds a row of that one date. A weekday
//! changed moves each date within its week, and the dates QIS cancels and its notes of a room move
//! with them. The changes are applied before the ticks and the hidden dates: those name the Termin
//! by its key and a day as the week shows it. What the change says is the Termin then: the week,
//! the day, the clashes, a choice of groups and the feed take it as they take QIS's.
//!
//! Each change is measured against what QIS says now (`folia_calendar::change::Change::against`):
//! it holds, it holds while QIS says something else (the student decides), it is done (QIS says
//! what it says, and nothing is changed), its Termin is gone while its event is there (QIS moved it:
//! another day, start, group or first date makes another key), or both are gone. A change of a
//! module not in the semester's plan says nothing.

use std::collections::BTreeMap;

use folia_calendar::change::{Against, Change, Scope, Values};
use folia_calendar::day::{clock, Day};
use folia_calendar::rowkey::RowKey;
use folia_model::labels::{Code, Rhythm};
use folia_model::rows_detail::DateRow;

use crate::facts::SemesterFacts;
use crate::model::{Attendance, Event, Row};

/// A change of a Termin of QIS as the timetable holds it: the change, how it stands against what
/// QIS says now, and of a Termin that is there its event and what QIS says of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Changed {
    pub change: Change,
    pub state: ChangeState,
    pub event: Option<usize>,
    pub qis: Option<Values>,
}

/// How a change stands (`Changed`).
#[derive(Clone, Debug, PartialEq)]
pub enum ChangeState {
    /// It holds: its Termin shows it, with the pen.
    Holds,
    /// QIS says something else now of a field it changes: it holds until the student decides
    /// („QIS übernehmen", „Meine behalten"), and „Offen" names it.
    Differs,
    /// QIS says what it says: it is done, and changes nothing („Offen" says so once).
    Agrees,
    /// Its Termin is gone and its event is there: QIS gave it another day, start, group or first
    /// date. `now`: the one Termin the event has, where it has one, and what QIS says of it.
    Moved { now: Option<(RowKey, Values)> },
    /// Its Termin and its event are gone.
    Gone,
}

impl ChangeState {
    /// It stands for something the student decides („Offen").
    pub fn asks(&self) -> bool {
        !matches!(self, ChangeState::Holds)
    }
}

/// What a row shows of the change that made it (`Row::changed`).
#[derive(Clone, Debug, PartialEq)]
pub struct RowChange {
    pub scope: Scope,
    /// What QIS says of the Termin.
    pub qis: Values,
    /// QIS says something else now than when the change was made (`ChangeState::Differs`).
    pub differs: bool,
    /// It moves its date to another weekday or time: a date on its own is a date of the week, one
    /// of a room changed for a day stands in its series' slot.
    pub moves: bool,
}

/// Applies `changes` to the rows of `events` (QIS's, before the Termine of one's own come), the
/// modules of the semester's plan `modules`. Returns every change of a planned module with how it
/// stands, in the order of the Termine and their scopes.
pub(crate) fn apply(events: &mut [Event], changes: &[Change], modules: &[String], facts: &SemesterFacts) -> Vec<Changed> {
    let mut sorted: Vec<&Change> = changes.iter().filter(|change| change.is_valid()).collect();
    sorted.sort_by_key(|change| (change.key, change.scope));
    sorted.dedup_by_key(|change| (change.key, change.scope));
    // What QIS says of each Termin, before the first change of it rewrites its rows.
    let mut qis: BTreeMap<RowKey, Option<(usize, Values)>> = BTreeMap::new();
    let mut out = Vec::new();
    for change in sorted {
        let found = qis.entry(change.key).or_insert_with(|| {
            let e = events.iter().position(|event| event.rows.iter().any(|row| row.key == Some(change.key)))?;
            let event = events.get(e)?;
            Some((e, values_of(event.rows.iter().filter(|row| row.key == Some(change.key)))?))
        });
        let Some((e, values)) = found.clone() else {
            if !modules.contains(&change.module) {
                continue;
            }
            let id = change.key.event.to_string();
            let state = match events.iter().find(|event| event.id == id) {
                Some(event) => ChangeState::Moved { now: only_termin(event) },
                None => ChangeState::Gone,
            };
            out.push(Changed { change: change.clone(), state, event: None, qis: None });
            continue;
        };
        let state = match change.against(&values) {
            Against::Agrees => ChangeState::Agrees,
            Against::Holds => ChangeState::Holds,
            Against::Differs => ChangeState::Differs,
        };
        if state != ChangeState::Agrees {
            if let Some(event) = events.get_mut(e) {
                let mark = RowChange { scope: change.scope, qis: values.clone(), differs: state == ChangeState::Differs, moves: change.weekday.is_some() || change.time.is_some() };
                change_rows(event, change, &values, &mark, facts);
            }
        }
        out.push(Changed { change: change.clone(), state, event: Some(e), qis: Some(values) });
    }
    out
}

/// What QIS says of the Termin of `rows` (its rows, of one key): the weekday of its slot, its time,
/// and its rooms as a student reads them, each once (`EventDate::room_shown`). `None` for one
/// without a weekday or a time, which has nothing a change could change.
fn values_of<'a>(rows: impl Iterator<Item = &'a Row>) -> Option<Values> {
    let rows: Vec<&Row> = rows.collect();
    let first = rows.first()?;
    let weekday = first.slot_day()?;
    let (from, to) = (first.from?, first.to?);
    let mut rooms: Vec<&str> = Vec::new();
    for room in rows.iter().filter_map(|row| row.date.room_shown()) {
        if !rooms.contains(&room) {
            rooms.push(room);
        }
    }
    Some(Values { weekday, from, to, room: rooms.join(", ") })
}

/// The one Termin an event has and what QIS says of it, where it has one.
fn only_termin(event: &Event) -> Option<(RowKey, Values)> {
    let mut keys = event.rows.iter().filter_map(|row| row.key);
    let key = keys.next()?;
    if keys.any(|other| other != key) {
        return None;
    }
    Some((key, values_of(event.rows.iter().filter(|row| row.key == Some(key)))?))
}

/// Rewrites the rows of `change`'s Termin in `event` as its scope says, QIS saying `qis` of it. A
/// row it adds stands right after the one it comes of, with its `ord`, so the rows stay by `ord`.
fn change_rows(event: &mut Event, change: &Change, qis: &Values, mark: &RowChange, facts: &SemesterFacts) {
    let of_key: Vec<usize> = event.rows.iter().enumerate().filter(|(_, row)| row.key == Some(change.key)).map(|(at, _)| at).collect();
    let mut added: Vec<(usize, Row)> = Vec::new();
    // From the last, so that a row added after one leaves the places of those before it.
    for at in of_key.into_iter().rev() {
        let Some(row) = event.rows.get(at) else { continue };
        match change.scope {
            Scope::All => {
                // A row whose range QIS leaves to the lecture period keeps it so.
                let range = row.date.first_date.as_deref().and_then(Day::parse).zip(row.date.last_date.as_deref().and_then(Day::parse));
                let new = changed_row(row, change, qis, range, false, mark, facts);
                if let Some(slot) = event.rows.get_mut(at) {
                    *slot = new;
                }
            }
            Scope::From(day) => {
                let Some((first, last)) = range_of(row, facts) else { continue };
                if last < day {
                    continue;
                }
                if first >= day {
                    let new = changed_row(row, change, qis, Some((first, last)), false, mark, facts);
                    if let Some(slot) = event.rows.get_mut(at) {
                        *slot = new;
                    }
                    continue;
                }
                let later = changed_row(row, change, qis, Some((day, last)), false, mark, facts);
                let before = ranged_row(row, (first, day.plus(-1)), facts);
                if let Some(slot) = event.rows.get_mut(at) {
                    *slot = before;
                }
                added.push((at, later));
            }
            Scope::On(day) => {
                if !row.occ.days.contains(&day) {
                    continue;
                }
                let moved = changed_row(row, change, qis, Some((day, day)), true, mark, facts);
                if let Some(slot) = event.rows.get_mut(at) {
                    slot.occ.days.retain(|held| *held != day);
                    slot.occ.moved.push(day);
                }
                added.push((at, moved));
            }
        }
    }
    // Each after the one it comes of, the last first; a row added beside one of an option is of
    // that option, and the rows after it move one place on.
    for (of, row) in added {
        let index = of + 1;
        if let Attendance::OneOf { options, .. } = &mut event.attendance {
            for rows in options.iter_mut() {
                for held in rows.iter_mut().filter(|held| **held >= index) {
                    *held += 1;
                }
            }
            if let Some(rows) = row.option.and_then(|option| options.get_mut(option)) {
                if rows.contains(&of) {
                    rows.push(index);
                    rows.sort_unstable();
                }
            }
        }
        event.rows.insert(index.min(event.rows.len()), row);
    }
}

/// The first and last day of `row` as QIS has them: its range, else the lecture period it is
/// assumed to run through; `None` for a pattern without days.
fn range_of(row: &Row, facts: &SemesterFacts) -> Option<(Day, Day)> {
    let first = row.date.first_date.as_deref().and_then(Day::parse);
    let last = row.date.last_date.as_deref().and_then(Day::parse);
    match (first, last) {
        (Some(first), Some(last)) => Some((first, last.max(first))),
        _ if row.occ.assumed => facts.lecture,
        _ => None,
    }
}

/// `row` with its dates made anew from `range` alone (a part of it before a change from a day).
fn ranged_row(row: &Row, (first, last): (Day, Day), facts: &SemesterFacts) -> Row {
    let mut date = row.date.clone();
    date.first_date = Some(first.iso());
    date.last_date = Some(last.iso());
    let mut new = Row::of(&DateRow { module_id: String::new(), ord: row.ord, cancelled_dates: row.cancelled_dates.clone(), date }, facts);
    new.key = row.key;
    new.option = row.option;
    new.hidden = row.hidden;
    new.changed = row.changed.clone();
    new
}

/// `row` as `change` says it, its dates those of `range` (QIS's days, `None`: QIS's pattern) moved
/// to the changed weekday within their weeks, as one date where `single`. The dates QIS cancels and
/// its notes of a room in the range move with them; the key, the option and what hides the row stay.
fn changed_row(row: &Row, change: &Change, qis: &Values, range: Option<(Day, Day)>, single: bool, mark: &RowChange, facts: &SemesterFacts) -> Row {
    let shift = |day: Day| change.moved(day, qis.weekday);
    let mut date = row.date.clone();
    if let Some(weekday) = change.weekday {
        date.weekday = Some(i64::from(weekday));
    }
    if let Some((from, to)) = change.time {
        date.start_time = Some(clock(from));
        date.end_time = Some(clock(to));
    }
    if let Some(room) = &change.room {
        date.room = Some(room.clone());
        date.room_short = Some(room.clone());
    }
    if let Some((first, last)) = range {
        date.first_date = Some(shift(first).iso());
        date.last_date = Some(shift(last).iso());
    }
    if single {
        date.rhythm = Some(Code::Known(Rhythm::Single));
        date.rhythm_raw = None;
        date.series = None;
    }
    let within = |day: Day| range.is_none_or(|(first, last)| (first..=last).contains(&day));
    let mut new = Row::of(&DateRow { module_id: String::new(), ord: row.ord, cancelled_dates: None, date }, facts);
    new.key = row.key;
    new.option = row.option;
    new.hidden = row.hidden;
    new.changed = Some(mark.clone());
    // What QIS says of a date in the range follows it to its new day: a cancellation, a note.
    let cancelled: Vec<(Day, Option<String>, Option<Day>)> =
        row.occ.cancelled.iter().filter(|(day, ..)| within(*day)).map(|(day, why, instead)| (shift(*day), why.clone(), *instead)).collect();
    if !cancelled.is_empty() {
        new.occ.days.retain(|held| !cancelled.iter().any(|(day, ..)| day == held));
        new.occ.cancelled = cancelled;
    }
    if change.room.is_none() {
        new.occ.notes = row.occ.notes.iter().filter(|(day, _)| within(*day)).map(|(day, note)| (shift(*day), note.clone())).collect();
    }
    new
}
