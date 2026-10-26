//! One date of a Termin, as its own view shows it (folia/design/termin, the fourth round; owner,
//! 2026-10-26: „Ja zu allen drei Vorschlägen, bau das jetzt in Folia"): the Termin of a row key on
//! a day, every other date of it and what differs on each, the options of its event's choice and
//! whether they fit the plan, and the plan's other Termine of the day.
//!
//! A Termin is the rows of one row key (the same slot in two rooms), and of a series of single
//! dates (`EventDate::series`) every date of the series at the same time, as the Stundenplan's
//! panel lists them: its view steps from date to date through the whole series. Pure functions of
//! a `Timetable`, as the rest of the crate; the page passes the day it is where one is to be
//! picked.

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::HiddenBy;

use crate::clash::{self, Weeks};
use crate::model::{Attendance, Event, Row, Timetable};
use crate::occur::Skipped;

/// What a date of a Termin is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DateState {
    /// Held as the Termin says.
    Held,
    /// Held, with a note of the room as QIS writes it („LV findet ersatzweise im ZHG HS A statt.").
    Note(String),
    /// QIS cancels it, with the reason as written (empty without one) and the day it names
    /// instead, where it names one.
    Cancelled(String, Option<Day>),
    /// Not held: the break or a holiday.
    Skipped(Skipped),
    /// Held, but the visitor hid it („Nur am … ausblenden", `Selection::hidden_dates`).
    Hidden,
}

impl DateState {
    /// Held, whatever is noted: a date of the series one counts.
    pub fn held(&self) -> bool {
        matches!(self, DateState::Held | DateState::Note(_) | DateState::Hidden)
    }

    /// What one steps to: every date but those of the break and the holidays.
    pub fn steppable(&self) -> bool {
        !matches!(self, DateState::Skipped(_))
    }

    /// Which of two states of one day of a Termin of two rows wins: what says more.
    fn rank(&self) -> u8 {
        match self {
            DateState::Note(_) => 5,
            DateState::Held => 4,
            DateState::Hidden => 3,
            DateState::Cancelled(..) => 2,
            DateState::Skipped(_) => 1,
        }
    }
}

/// One date of a Termin: the day, what it is, and the row it is a date of (by index into the
/// event's rows), whose key hides it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dated {
    pub day: Day,
    pub state: DateState,
    pub row: usize,
}

/// One date of a Termin, with the rest of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Occasion {
    /// The event, by index into `Timetable::events`.
    pub event: usize,
    /// The rows of the Termin, by index into the event's rows: the row of the key first, then the
    /// others of the key (other rooms) and of its series.
    pub rows: Vec<usize>,
    /// The key the Termin was asked for.
    pub key: RowKey,
    /// The date shown, one of `dates`.
    pub day: Day,
    pub state: DateState,
    /// Every date of the Termin, in order: held, cancelled, in the break, hidden.
    pub dates: Vec<Dated>,
    /// The times in minutes; `None` is „Zeit unklar".
    pub from: Option<u16>,
    pub to: Option<u16>,
    /// The week of the day where the Termin is held every other week: „A-Woche", „B-Woche".
    pub weeks: Option<Weeks>,
    /// Why the plan leaves the Termin out, where it does: the event or the Termin hidden, another
    /// group ticked, its kind switched off …
    pub hidden: Option<HiddenBy>,
}

impl Occasion {
    /// The Termin `key` of the timetable's events on `on`, else on the date nearest to it, and
    /// without a day on its next date from `today` (else its last). `None` when no row has the
    /// key, or the Termin has no date at all (a row „ohne feste Zeit").
    pub fn find(table: &Timetable, key: RowKey, on: Option<Day>, today: Option<Day>) -> Option<Occasion> {
        let (event, first) = table.events.iter().enumerate().find_map(|(index, event)| Some((index, event.rows.iter().position(|row| row.key == Some(key))?)))?;
        let of = table.events.get(event)?;
        let rows = termin_rows(of, first);
        let dates = dates_of(of, &rows);
        let day = pick(&dates, on, today)?;
        let dated = dates.iter().find(|dated| dated.day == day)?;
        let state = dated.state.clone();
        let row = of.rows.get(first)?;
        let weeks = Weeks::of(row.every).filter(|weeks| *weeks != Weeks::All).and(table.facts.ab_week(day));
        let hidden = of.hidden.or_else(|| of.rows.get(dated.row).and_then(|row| row.hidden));
        Some(Occasion { event, rows, key, day, state, dates, from: row.from, to: row.to, weeks, hidden })
    }

    /// The date before (`back`) or after this one, of those one steps to.
    pub fn step(&self, back: bool) -> Option<Day> {
        let steppable = self.dates.iter().filter(|dated| dated.state.steppable());
        match back {
            true => steppable.filter(|dated| dated.day < self.day).map(|dated| dated.day).max(),
            false => steppable.filter(|dated| dated.day > self.day).map(|dated| dated.day).min(),
        }
    }

    /// Which of the dates held this one is (1-based; `None` for one not held), how many are held,
    /// and how many of them are still to come from `today` on (all without a day).
    pub fn place(&self, today: Option<Day>) -> (Option<usize>, usize, usize) {
        let held: Vec<Day> = self.dates.iter().filter(|dated| dated.state.held()).map(|dated| dated.day).collect();
        let nth = self.state.held().then(|| held.iter().position(|day| *day == self.day).map(|at| at + 1)).flatten();
        let left = held.iter().filter(|day| today.is_none_or(|today| **day >= today)).count();
        (nth, held.len(), left)
    }

    /// The key of the row the date shown is of: what hides it.
    pub fn date_key(&self, table: &Timetable) -> Option<RowKey> {
        let event = table.events.get(self.event)?;
        let row = self.dates.iter().find(|dated| dated.day == self.day)?.row;
        event.rows.get(row)?.key
    }
}

/// The rows of the Termin of `first` in `event`: those of its key, and of a series of single dates
/// every row of the series at the same times (as the panel's `by_termin` takes them), `first`
/// first.
pub fn termin_rows(event: &Event, first: usize) -> Vec<usize> {
    let Some(row) = event.rows.get(first) else { return Vec::new() };
    let series = row.date.series.map(|series| (series.first, row.from, row.to));
    let mut rows = vec![first];
    for (index, other) in event.rows.iter().enumerate() {
        let same_key = other.key.is_some() && other.key == row.key;
        let same_series = series.is_some() && other.date.series.map(|s| (s.first, other.from, other.to)) == series;
        if index != first && (same_key || same_series) {
            rows.push(index);
        }
    }
    rows
}

/// Every date of the rows, in order, one per day: what is held there, noted, cancelled, skipped
/// or hidden. A day two rows share takes what says more (`DateState::rank`).
pub fn dates_of(event: &Event, rows: &[usize]) -> Vec<Dated> {
    let mut dates: Vec<Dated> = Vec::new();
    let mut add = |day: Day, state: DateState, row: usize| match dates.iter_mut().find(|dated| dated.day == day) {
        Some(dated) if state.rank() > dated.state.rank() => *dated = Dated { day, state, row },
        Some(_) => {}
        None => dates.push(Dated { day, state, row }),
    };
    for index in rows {
        let Some(row) = event.rows.get(*index) else { continue };
        let occ = &row.occ;
        for day in &occ.days {
            let notes: Vec<&str> = occ.notes.iter().filter(|(on, _)| on == day).map(|(_, note)| note.as_str()).collect();
            let state = if notes.is_empty() { DateState::Held } else { DateState::Note(notes.join("; ")) };
            add(*day, state, *index);
        }
        for day in &occ.hidden {
            add(*day, DateState::Hidden, *index);
        }
        for (day, reason, instead) in &occ.cancelled {
            add(*day, DateState::Cancelled(reason.clone().unwrap_or_default(), *instead), *index);
        }
        for (day, why) in &occ.skipped {
            add(*day, DateState::Skipped(*why), *index);
        }
    }
    dates.sort_by_key(|dated| dated.day);
    dates
}

/// The date to show of `dates`: `on` where it is one, else the date to step to nearest to it
/// (the later of two as near); without `on`, the next date held from `today`, else the next one
/// to step to, else the last of those; the first of all where none can be stepped to.
fn pick(dates: &[Dated], on: Option<Day>, today: Option<Day>) -> Option<Day> {
    let steppable: Vec<Day> = dates.iter().filter(|dated| dated.state.steppable()).map(|dated| dated.day).collect();
    if let Some(on) = on {
        if dates.iter().any(|dated| dated.day == on) {
            return Some(on);
        }
        return steppable.iter().copied().min_by_key(|day| ((day.0 - on.0).abs(), on.0 - day.0)).or_else(|| dates.first().map(|dated| dated.day));
    }
    let from = |day: &Day| today.is_none_or(|today| *day >= today);
    dates
        .iter()
        .find(|dated| dated.state.held() && from(&dated.day))
        .map(|dated| dated.day)
        .or_else(|| steppable.iter().copied().find(from))
        .or_else(|| steppable.last().copied())
        .or_else(|| dates.first().map(|dated| dated.day))
}

/// A Termin of an event as the grids of the Termin's view name it: its key, its first row and the
/// rows of its other rooms.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub key: RowKey,
    pub row: usize,
    /// Further rows of the key at the same time in other rooms („+1").
    pub rooms: usize,
}

/// The Termine of an event, each once (a series of single dates as one), by weekday and time and
/// then in the order of the rows; rows without a key are none.
pub fn sessions(event: &Event) -> Vec<Session> {
    let mut seen: Vec<usize> = Vec::new();
    let mut out: Vec<Session> = Vec::new();
    for (index, row) in event.rows.iter().enumerate() {
        let Some(key) = row.key else { continue };
        if seen.contains(&index) {
            continue;
        }
        let rows = termin_rows(event, index);
        let rooms = rows.iter().filter(|other| event.rows.get(**other).is_some_and(|other| other.key == Some(key))).count().saturating_sub(1);
        seen.extend(rows);
        out.push(Session { key, row: index, rooms });
    }
    out.sort_by_key(|session| event.rows.get(session.row).map(|row| (row.slot_day().unwrap_or(8), row.from.unwrap_or(u16::MAX))));
    out
}

/// An option of the event's choice as a tile to tick or to look up.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub option: usize,
    /// Its first Termin, which the tile names (weekday, time, room, group).
    pub session: Session,
    /// The keys of all its Termine: one of them ticks it, all of them untick it
    /// (`Event::option_keys`).
    pub keys: Vec<RowKey>,
    /// The visitor ticked it.
    pub ticked: bool,
    /// The first event of the plan a Termin of it meets (`meets`); `None` where it fits.
    pub meets: Option<usize>,
}

/// The options of the choice of the event `event` (`Attendance::OneOf`), in their order; empty
/// for an event whose rows are all required.
pub fn tiles(table: &Timetable, event: usize) -> Vec<Tile> {
    let Some(of) = table.events.get(event) else { return Vec::new() };
    let Attendance::OneOf { options, .. } = &of.attendance else { return Vec::new() };
    let sessions = sessions(of);
    options
        .iter()
        .enumerate()
        .filter_map(|(option, rows)| {
            let session = sessions.iter().find(|session| rows.contains(&session.row)).cloned().or_else(|| {
                let row = *rows.first()?;
                Some(Session { key: of.rows.get(row)?.key?, row, rooms: 0 })
            })?;
            let meets = rows.iter().find_map(|row| meets(table, event, *row));
            Some(Tile { option, session, keys: of.option_keys(option), ticked: of.chosen.contains(&option), meets })
        })
        .collect()
}

/// The first event of the plan the row `row` of `event` meets: a Termin of another event the plan
/// holds firmly (shown, no extra, no option of an open choice) on a day both are held, at times
/// that overlap. An option of the event's own choice is weighed the same whether it is ticked or
/// not, so that a tile says what ticking it would cost.
pub fn meets(table: &Timetable, event: usize, row: usize) -> Option<usize> {
    let mine = table.events.get(event)?.rows.get(row)?;
    let (from, to) = (mine.from?, mine.to?);
    table.events.iter().enumerate().find_map(|(index, other)| {
        if index == event || !holds(other) {
            return None;
        }
        let hit = other.rows.iter().any(|theirs| {
            let open = other.unresolved() && theirs.option.is_some();
            let overlap = theirs.from.zip(theirs.to).is_some_and(|(a, b)| a < to && from < b);
            theirs.hidden.is_none() && !open && overlap && clash::shared(mine, theirs).is_some()
        });
        hit.then_some(index)
    })
}

/// Whether an event takes part in the plan's clashes: shown, no extra beside its module, no
/// option of an open choice among its module's events.
fn holds(event: &Event) -> bool {
    event.hidden.is_none() && !event.extra && !event.across.is_some_and(|alt| alt.open)
}

/// A Termin of the plan on a day, as „Der Tag" draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Beside {
    pub event: usize,
    /// Its first row on the day, and its key.
    pub row: usize,
    pub key: Option<RowKey>,
    pub from: u16,
    pub to: u16,
    pub cancelled: bool,
    /// An option of an open choice: one of several, not yet the plan's.
    pub open: bool,
}

/// The plan's Termine of `day` with a time, by start and then in the order of the events: held
/// or cancelled, shown in the plan (an option of an open choice among them, as the week draws it
/// dashed), each Termin once.
pub fn day_of(table: &Timetable, day: Day) -> Vec<Beside> {
    let mut out: Vec<Beside> = Vec::new();
    for (index, event) in table.events.iter().enumerate() {
        if event.hidden.is_some() {
            continue;
        }
        for (r, row) in event.rows.iter().enumerate() {
            let (Some(from), Some(to)) = (row.from, row.to) else { continue };
            if row.hidden.is_some() {
                continue;
            }
            let held = row.occ.days.contains(&day);
            let cancelled = row.occ.cancelled.iter().any(|(on, ..)| *on == day);
            if !held && !cancelled {
                continue;
            }
            if out.iter().any(|beside| beside.event == index && beside.key.is_some() && beside.key == row.key) {
                continue;
            }
            let open = (event.unresolved() && row.option.is_some()) || event.across.is_some_and(|alt| alt.open);
            out.push(Beside { event: index, row: r, key: row.key, from, to, cancelled: cancelled && !held, open });
        }
    }
    out.sort_by_key(|beside| (beside.from, beside.to, beside.event));
    out
}

/// What the date of `occasion` meets that day: the first Termin of another event the plan holds
/// firmly, held at a time that overlaps.
pub fn clash_on(table: &Timetable, occasion: &Occasion) -> Option<Beside> {
    let (from, to) = (occasion.from?, occasion.to?);
    if !occasion.state.held() || occasion.state == DateState::Hidden {
        return None;
    }
    day_of(table, occasion.day).into_iter().find(|beside| {
        let firm = table.events.get(beside.event).is_some_and(holds);
        beside.event != occasion.event && firm && !beside.open && !beside.cancelled && beside.from < to && from < beside.to
    })
}

/// What comes after the date of `occasion` that day: the next Termin held, of another Termin.
pub fn after(table: &Timetable, occasion: &Occasion) -> Option<Beside> {
    let to = occasion.to?;
    day_of(table, occasion.day).into_iter().find(|beside| beside.from >= to && !beside.cancelled && !(beside.event == occasion.event && beside.key == Some(occasion.key)))
}

/// The row of `event` a click on one of its Termine opens: its first with a key.
pub fn first_keyed(event: &Event) -> Option<&Row> {
    event.rows.iter().find(|row| row.key.is_some())
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;

    use super::*;
    use crate::model::tests::{d, table, teaching, winter};
    use crate::occur::Skipped;

    /// Two exercises and a lecture of 12104 in 2026W: the lecture Tue 09:15, group 1 Mon 11:30,
    /// group 2 Wed 11:30 — and group 3 on Tue 09:15, under the lecture.
    fn module() -> Vec<crate::model::tests::Fixture> {
        vec![
            teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"),
            teaching("12104", "200", 2, "Übung", 3, "11:30", "13:00").group("2-Gruppe"),
            teaching("12104", "200", 3, "Übung", 2, "09:15", "10:45").group("3-Gruppe"),
        ]
    }

    #[test]
    fn a_termin_is_found_on_its_next_date_and_steps_through_its_series() {
        let rows = module();
        let key = rows[0].key();
        let t = table(&rows, &["12104"], &[], &Selection::default());
        assert_eq!(t.facts, winter());
        // Without a day: the next date from today; Tuesdays of the lecture period, the break left out.
        let o = Occasion::find(&t, key, None, Some(d("2026-11-04"))).unwrap();
        assert_eq!((o.day, o.from, o.to, o.state.clone()), (d("2026-11-10"), Some(555), Some(645), DateState::Held));
        assert_eq!((o.step(true), o.step(false)), (Some(d("2026-11-03")), Some(d("2026-11-17"))));
        let (nth, count, left) = o.place(Some(d("2026-11-04")));
        assert_eq!((nth, count), (Some(6), o.dates.iter().filter(|dated| dated.state.held()).count()));
        assert_eq!(left, count - 5);
        // The break: skipped, never stepped to; from the last date before it the next one is after it.
        let before = Occasion::find(&t, key, Some(d("2026-12-15")), None).unwrap();
        assert!(o.dates.iter().any(|dated| dated.day == d("2026-12-22") && dated.state == DateState::Skipped(Skipped::Break)));
        assert_eq!(before.step(false), Some(d("2027-01-05")));
        // A day that is none of its dates: the nearest one, the later of two as near.
        assert_eq!(Occasion::find(&t, key, Some(d("2026-11-07")), None).unwrap().day, d("2026-11-10"));
        assert_eq!(Occasion::find(&t, key, Some(d("2026-11-06")), None).unwrap().day, d("2026-11-03"));
        // Past the last date: the last.
        let last = o.dates.iter().rfind(|dated| dated.state.steppable()).unwrap().day;
        assert_eq!(Occasion::find(&t, key, None, Some(d("2027-06-01"))).unwrap().day, last);
        // An unknown key: none.
        assert!(Occasion::find(&t, RowKey { event: 999, fp: 1 }, None, None).is_none());
    }

    #[test]
    fn a_cancelled_a_noted_and_a_hidden_date_say_so() {
        let mut rows = module();
        rows[0].0.cancelled_dates = Some("10.11.2026: Krankheit 17.11.2026: LV findet ersatzweise im ZHG HS A statt.".into());
        let key = rows[0].key();
        let selection = Selection { hidden_dates: [(key, d("2026-11-24"))].into_iter().collect(), ..Selection::default() };
        let t = table(&rows, &["12104"], &[], &selection);
        let state = |on: &str| Occasion::find(&t, key, Some(d(on)), None).unwrap().state;
        assert_eq!(state("2026-11-10"), DateState::Cancelled("Krankheit".into(), None));
        assert!(matches!(state("2026-11-17"), DateState::Note(note) if note.contains("ZHG HS A")), "{:?}", state("2026-11-17"));
        assert_eq!(state("2026-11-24"), DateState::Hidden);
        // A cancelled date is stepped to, and counts as none of those held; a hidden one does.
        let o = Occasion::find(&t, key, Some(d("2026-11-10")), None).unwrap();
        assert_eq!(o.place(None).0, None);
        let hidden = Occasion::find(&t, key, Some(d("2026-11-24")), None).unwrap();
        assert_eq!(hidden.place(None).0, Some(7));
        assert_eq!(hidden.date_key(&t), Some(key));
    }

    #[test]
    fn the_groups_say_what_they_meet_and_what_is_ticked() {
        let rows = module();
        let (one, three) = (rows[1].key(), rows[3].key());
        let open = table(&rows, &["12104"], &[], &Selection::default());
        let event = open.events.iter().position(|event| event.id == "200").unwrap();
        let tiles = tiles(&open, event);
        assert_eq!(tiles.len(), 3);
        // Group 3 meets the lecture; the others fit.
        let lecture = open.events.iter().position(|event| event.id == "100").unwrap();
        assert_eq!(tiles.iter().map(|tile| tile.meets).collect::<Vec<_>>(), [None, None, Some(lecture)]);
        assert!(tiles.iter().all(|tile| !tile.ticked) && tiles[0].keys == [one]);
        // Two ticked, all alike; the third still says what it would meet.
        let ticked = Selection { chosen_rows: [one, three].into_iter().collect(), ..Selection::default() };
        let t = table(&rows, &["12104"], &[], &ticked);
        let tiles = super::tiles(&t, event);
        assert_eq!(tiles.iter().map(|tile| (tile.ticked, tile.meets.is_some())).collect::<Vec<_>>(), [(true, false), (false, false), (true, true)]);
        // The date of group 3 meets the lecture that day; the lecture's meets group 3 once ticked.
        let o = Occasion::find(&t, three, Some(d("2026-11-10")), None).unwrap();
        assert_eq!(clash_on(&t, &o).map(|beside| beside.event), Some(lecture));
        let l = Occasion::find(&t, rows[0].key(), Some(d("2026-11-10")), None).unwrap();
        assert_eq!(clash_on(&t, &l).map(|beside| beside.event), Some(event));
        // Not while group 3 is one option of an open choice: dashed, it is none of the plan's yet.
        let l = Occasion::find(&open, rows[0].key(), Some(d("2026-11-10")), None).unwrap();
        assert_eq!(clash_on(&open, &l), None);
    }

    #[test]
    fn the_day_holds_the_plans_termine_and_what_comes_after() {
        let rows = module();
        let one = rows[1].key();
        let t = table(&rows, &["12104"], &[], &Selection { chosen_rows: [one].into_iter().collect(), ..Selection::default() });
        // Tuesday: the lecture alone (group 3 is not ticked); Monday group 1.
        let tuesday = day_of(&t, d("2026-11-10"));
        assert_eq!(tuesday.iter().map(|beside| t.events[beside.event].id.as_str()).collect::<Vec<_>>(), ["100"]);
        assert_eq!(day_of(&t, d("2026-11-09")).len(), 1);
        let monday = Occasion::find(&t, one, Some(d("2026-11-09")), None).unwrap();
        assert_eq!(after(&t, &monday), None);
        // The sessions of the exercise, by weekday: Mon, Tue, Wed.
        let event = t.events.iter().find(|event| event.id == "200").unwrap();
        let days: Vec<u8> = sessions(event).iter().map(|session| event.rows[session.row].slot_day().unwrap()).collect();
        assert_eq!(days, [1, 2, 3]);
    }
}
