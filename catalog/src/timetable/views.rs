//! The Regelwoche and the dated agenda of a timetable.
//!
//! A timetable (`model`) knows every date of every row; a student reads it in two ways. The
//! Regelwoche is the Stundenplan: one slot per recurring Termin at its weekday and time, however
//! often it meets, and the dates of an event that do not recur gathered per weekday and time into
//! one slot („3 Termine"), so a single date on 23.02. no longer reads as a weekly one (12229). The
//! agenda („Termine") lists every date by week and day: what is cancelled and why, which day is a
//! holiday, which weeks have no lectures, and the exams. What has no time or no date at all
//! („Ohne feste Zeit") stands apart, once for the semester.
//!
//! Only what is shown is in a view: hidden events and Termine, and the options a made choice left
//! out, are not. Neither are the recurring dates the break or a holiday takes: the agenda names
//! the holiday on its day and marks the break's weeks instead of listing every lecture that does
//! not take place. A row whose every date they take stands with what has no fixed time, so that
//! a Termin QIS set into the break on purpose is not lost.
//!
//! The agenda shows a Termin once per date, as the calendar feed does: rows that share a
//! `RowKey` (one slot in two rooms, C.5) are one item that names all of them, and an exam in two
//! rooms at one time is one sitting (C.11).

use std::collections::{BTreeMap, BTreeSet};

use super::clash::{self, Weeks};
use super::day::Day;
use super::exams::ExamShape;
use super::facts::SemesterFacts;
use super::model::{Event, Row, Timetable};
use super::occur::Every;
use super::rowkey::RowKey;

/// Days a recurring Termin's first or last week may lie from the lecture period's first or last
/// week and still run through it: many Übungen begin a week late or end a week early, and „ab
/// 12.10." on each of them would say nothing the student needs.
const PERIOD_SLACK_DAYS: i64 = 7;

/// The most weeks the agenda walks for a lecture period. A period lies inside its half-year
/// (`facts`), so it has at most 54 weeks; the bound only keeps the loop finite.
const MAX_WEEKS: usize = 60;

/// The most characters of a module's short name (`short_title`): two lines of a slot in a day of
/// the Studienplan's week on a laptop. Half the catalog's titles are 31 characters or shorter.
const SHORT_TITLE: usize = 32;

/// The fewest characters a cut takes off a title, its „ …" counted: one that takes off only
/// „II" or „1 B" drops what tells two modules apart and frees next to no room.
const SHORT_SAVES: usize = 6;

/// Words a cut title does not end on: „Entwicklung von …" says less than „Entwicklung …".
const DANGLING: [&str; 21] =
    ["und", "oder", "der", "die", "das", "des", "dem", "den", "von", "vom", "für", "in", "im", "mit", "zu", "zur", "zum", "and", "of", "the", "for"];

/// Whether a cut title may not end on `word`: a function word (`DANGLING`), the first half of a
/// pair („Wirtschafts-" of „Wirtschafts- und Finanzmathematik"), or a mark alone („–", „/").
fn dangling(word: &str) -> bool {
    DANGLING.contains(&word.to_lowercase().as_str()) || word.ends_with('-') || !word.chars().any(char::is_alphanumeric)
}

/// A module's name in the few words a slot of a week grid has room for: its title without a
/// trailing note in parentheses („Mathematik IT-1 (Diskrete Mathematik)" → „Mathematik IT-1"),
/// and a title longer than `SHORT_TITLE` characters cut after a whole word, with „…"
/// („Elektrische und elektronische …"). Every slot that names a module takes its name from here,
/// so the abbreviations Radix is to deliver for the modules replace this one guess in one place.
pub fn short_title(title: &str) -> String {
    let title = title.trim();
    let head = title.strip_suffix(')').and_then(|rest| rest.rfind(" (").and_then(|at| rest.get(..at))).map(str::trim_end);
    let bare = head.filter(|head| !head.is_empty()).unwrap_or(title);
    let length = bare.chars().count();
    if length <= SHORT_TITLE {
        return bare.to_string();
    }
    let all: Vec<&str> = bare.split_whitespace().collect();
    let chars = |words: &[&str]| words.iter().map(|word| word.chars().count()).sum::<usize>() + words.len().saturating_sub(1);
    // Whole words up to the limit, but at least half of it: „Grundlagen der …" would name no
    // module, „Grundlagen der Betriebswirtschaftslehre …" does (the slot cuts what it cannot hold).
    let mut taken = 0;
    let mut used = 0;
    for word in &all {
        let with = used + usize::from(taken > 0) + word.chars().count();
        if with > SHORT_TITLE && used >= SHORT_TITLE / 2 {
            break;
        }
        taken += 1;
        used = with;
    }
    while taken > 1 && all.get(taken - 1).is_some_and(|word| dangling(word)) {
        taken -= 1;
    }
    // What is left once the hanging words are gone must still name the module: of „Einführung in
    // die Volkswirtschaftslehre" or „Grundlagen Bau- und Planungsrecht" only „Einführung" and
    // „Grundlagen" would. Such a cut takes the words up to the next one that ends a name, past
    // the limit.
    let kept = all.get(..taken).unwrap_or_default();
    if chars(kept) < SHORT_TITLE / 2 || kept.last().is_some_and(|word| dangling(word)) {
        taken = all.iter().enumerate().skip(taken).find(|(_, word)| !dangling(word)).map_or(all.len(), |(at, _)| at + 1);
    }
    let cut = all.get(..taken).unwrap_or_default().join(" ");
    if taken >= all.len() || length < cut.chars().count() + 2 + SHORT_SAVES {
        return all.join(" ");
    }
    // „Recht II: Handels- und Gesellschaftsrecht …", not „…recht, …".
    format!("{} …", cut.trim_end_matches([',', ';', ':']))
}

/// A slot of the Regelwoche: a recurring Termin, or the dates of one event at one weekday and
/// time that do not recur.
#[derive(Clone, Debug, PartialEq)]
pub struct WeekItem {
    /// The event, and the row a click opens (`row=`): the row of the slot's earliest held day,
    /// else its first row.
    pub event: usize,
    pub row: usize,
    /// Weekday, 1 = Monday.
    pub day: u8,
    /// Minutes since midnight, `from < to`.
    pub from: u16,
    pub to: u16,
    pub label: WeekLabel,
    /// For an option of an open choice („1 von 4"): the number of visible options, and the
    /// option.
    pub alt: Option<(usize, usize)>,
    /// A row of the slot is in a hard clash (`clash::hard_rows`).
    pub clash: bool,
    /// The weeks of the A/B rhythm the slot is held in; `None` for dates that do not recur.
    pub weeks: Option<Weeks>,
    /// What its rows are in a hard clash with (`clash::hard_pairs`): the other event and the
    /// weeks they meet in (`None`: on single days), in the order of the events.
    pub against: Vec<(usize, Option<Weeks>)>,
}

impl WeekItem {
    /// Whether the slot is held in the week `shown` („A/B" shows every slot, and dates that do
    /// not recur stand in every week).
    pub fn in_week(&self, shown: Weeks) -> bool {
        self.weeks.is_none_or(|weeks| weeks.in_week(shown))
    }

    /// The events it clashes with in the week `shown`; one on single days in every week.
    pub fn against_in(&self, shown: Weeks) -> Vec<usize> {
        let meets = |weeks: &Option<Weeks>| weeks.is_none_or(|weeks| weeks.in_week(shown));
        self.against.iter().filter(|(_, weeks)| meets(weeks)).map(|(event, _)| *event).collect()
    }
}

/// What the small text of a Regelwoche slot says about its dates.
#[derive(Clone, Debug, PartialEq)]
pub enum WeekLabel {
    /// A recurring Termin through the lecture period, or one whose range is unknown (a pattern,
    /// or no period to compare with).
    Every(Every),
    /// A recurring Termin in a part of the lecture period: its first and last date, cancelled
    /// and skipped ones included. `reach` says which end differs; the rhythm is the row's
    /// (`Every::of`).
    Partial { first: Day, last: Day },
    /// Dates that do not recur (single dates, the days of a block), or the one date of a
    /// recurring row whose range holds no other: how many are held, and the first.
    Once { dates: usize, first: Day },
}

/// Which ends of the lecture period a `Partial` slot leaves out, for its small text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// It begins with the period and ends early: „bis 23.11.".
    Until(Day),
    /// It begins late and ends with the period: „ab 07.12.".
    From(Day),
    /// Neither end is the period's: „12.10.–23.11.".
    Between(Day, Day),
}

impl WeekLabel {
    /// Which ends of the lecture period a partial slot leaves out; `None` for every other label.
    pub fn reach(&self, facts: &SemesterFacts) -> Option<Reach> {
        match self {
            WeekLabel::Partial { first, last } => reach_of(facts.lecture, *first, *last),
            WeekLabel::Every(_) | WeekLabel::Once { .. } => None,
        }
    }
}

/// One week of the agenda.
#[derive(Clone, Debug, PartialEq)]
pub struct AgendaWeek {
    pub monday: Day,
    /// ISO year and week of the Monday („KW 41"). The page's anchor needs both, `kw-2026-41`:
    /// rows can run for more than a year (147307's block of 2026S, 2026 to 2028), and then a
    /// week number comes twice.
    pub iso_week: (i32, u8),
    /// A week of a break (`SemesterFacts::breaks`): the page collapses it when it holds nothing.
    pub break_week: bool,
    /// The days with an item or a holiday, in order.
    pub days: Vec<AgendaDay>,
}

/// One day of the agenda.
#[derive(Clone, Debug, PartialEq)]
pub struct AgendaDay {
    pub day: Day,
    /// The name of the public holiday on this day.
    pub holiday: Option<&'static str>,
    /// Items without a time first, then by time.
    pub items: Vec<AgendaItem>,
}

/// One date of the agenda: a teaching Termin's (`event`) or an exam's (`exam`), never both.
#[derive(Clone, Debug, PartialEq)]
pub struct AgendaItem {
    /// Index into `Timetable::events`.
    pub event: Option<usize>,
    /// Index into `Timetable::exams`.
    pub exam: Option<usize>,
    /// Index into that event's or exam's rows: the first of `rows`, the one a click opens.
    pub row: usize,
    /// Every row behind the date, in order, for its rooms („Raum A / Raum B"): a teaching date
    /// gathers the rows of one Termin (`RowKey`) and option that are held, or that are cancelled,
    /// on the day; an exam date the rows of one exam with one shape and time.
    pub rows: Vec<usize>,
    /// The times in minutes, from the earliest start to the latest end of the rows (the feed's
    /// merge): a teaching row's, an exam sitting's. `None` for a row without a clear time, a
    /// block without times, and an exam that is no sitting (its shape says which: a window
    /// stands on its first day, a deadline and a day without a time on theirs).
    pub from: Option<u16>,
    pub to: Option<u16>,
    /// `Some` when QIS cancels this date, with the reason as written (empty when there is none;
    /// the rows' different reasons joined with „; ").
    pub cancelled: Option<String>,
    /// Room notes of a held date („Raumwechsel"), several joined with „; ".
    pub note: Option<String>,
}

/// Which teaching rows are one Termin on a date: those that share a `RowKey`, or a row without
/// a key alone.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Termin {
    Key(RowKey),
    Row(usize),
}

/// What makes the dates of one day one agenda item.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ItemKey {
    /// A teaching date: the event, its Termin, the option, and whether it is cancelled.
    Teaching(usize, Termin, Option<usize>, bool),
    /// An exam date: the exam, its times, and its shape (`shape_order`).
    Exam(usize, Option<u16>, Option<u16>, u8),
}

/// An agenda item while the rows of its date are gathered.
struct Gathered {
    item: AgendaItem,
    /// The cancellation reasons and the room notes of its rows, each once, in order.
    reasons: Vec<String>,
    notes: Vec<String>,
}

impl Gathered {
    fn new(event: Option<usize>, exam: Option<usize>, row: usize, cancelled: bool) -> Self {
        let cancelled = cancelled.then(String::new);
        let item = AgendaItem { event, exam, row, rows: Vec::new(), from: None, to: None, cancelled, note: None };
        Gathered { item, reasons: Vec::new(), notes: Vec::new() }
    }

    /// Adds row `r` with its times, the reason QIS gives when it cancels the date, and the row's
    /// room notes of the day.
    fn add(&mut self, r: usize, times: (Option<u16>, Option<u16>), reason: Option<&str>, notes: &[&str]) {
        self.item.rows.push(r);
        self.item.from = self.item.from.into_iter().chain(times.0).min();
        self.item.to = self.item.to.into_iter().chain(times.1).max();
        let once = |texts: &mut Vec<String>, text: &str| {
            if !text.is_empty() && !texts.iter().any(|known| known == text) {
                texts.push(text.to_string());
            }
        };
        if let Some(reason) = reason {
            once(&mut self.reasons, reason);
        }
        for note in notes {
            once(&mut self.notes, note);
        }
    }

    fn done(self) -> AgendaItem {
        let mut item = self.item;
        item.rows.sort_unstable();
        item.row = item.rows.first().copied().unwrap_or(item.row);
        if item.cancelled.is_some() {
            item.cancelled = Some(self.reasons.join("; "));
        }
        item.note = (!self.notes.is_empty()).then(|| self.notes.join("; "));
        item
    }
}

/// A Regelwoche slot while its rows are gathered: the rows of one event at one weekday and time,
/// with one rhythm and one option.
struct Slot {
    /// `None`: dates that do not recur.
    every: Option<Every>,
    /// The first row added; rows come in order.
    lowest: usize,
    /// The earliest held day and its row.
    first: Option<(Day, usize)>,
    /// The held days of dates that do not recur.
    held: BTreeSet<Day>,
    /// The first and last date of the recurring rows, cancelled and skipped ones included.
    span: Option<(Day, Day)>,
    /// A recurring row is only a pattern: the slot's range is unknown.
    pattern: bool,
    clash: bool,
    /// What its rows clash with, and in which weeks (`WeekItem::against`).
    against: BTreeSet<(usize, Option<Weeks>)>,
}

impl Slot {
    fn new(every: Option<Every>, row: usize) -> Self {
        Slot {
            every,
            lowest: row,
            first: None,
            held: BTreeSet::new(),
            span: None,
            pattern: false,
            clash: false,
            against: BTreeSet::new(),
        }
    }

    /// A held day of `row`: the earliest names the slot's row.
    fn held_on(&mut self, day: Day, row: usize) {
        if self.first.is_none_or(|(first, _)| day < first) {
            self.first = Some((day, row));
        }
    }

    /// A date of a recurring row, held or not: the span grows to hold it.
    fn spans(&mut self, day: Day) {
        self.span = Some(self.span.map_or((day, day), |(first, last)| (first.min(day), last.max(day))));
    }
}

/// The key of a Regelwoche slot, in the order of the slots: weekday, start, end, event, option
/// (required rows first), rhythm (`rhythm_order`).
type SlotKey = (u8, u16, u16, usize, Option<usize>, u8);

/// The rows in a hard clash with a row `(event, row)`: each other event and the weeks they meet in.
type Against = BTreeMap<(usize, usize), BTreeSet<(usize, Option<Weeks>)>>;

impl Timetable {
    /// The Regelwoche, by weekday, time and event. A visible row with a time is a slot when it
    /// has a date: a recurring row with a held day or only its pattern is one slot at its
    /// weekday, and the held dates of a row that does not recur are gathered per weekday and
    /// time. Rows of one event at one weekday, time and rhythm and of one option are one slot:
    /// one slot in two rooms, or in two ranges one after the other. A recurring slot reads as
    /// running through the lecture period, as a part of it, or as one date when its range holds
    /// no other. A recurring row whose every date is cancelled has no slot (the agenda lists its
    /// cancellations); one whose every date the break or a holiday takes, and what has no time
    /// or no date at all, is `loose`.
    pub fn regular_week(&self) -> Vec<WeekItem> {
        // One pairwise comparison for the whole week, not one per slot.
        let mut hard: Against = BTreeMap::new();
        for (a, b, weeks) in clash::hard_pairs(&self.events) {
            hard.entry(a).or_default().insert((b.0, weeks));
            hard.entry(b).or_default().insert((a.0, weeks));
        }
        let open: Vec<Option<usize>> =
            self.events.iter().map(|event| event.unresolved().then(|| event.visible_options().len())).collect();
        let mut slots: BTreeMap<SlotKey, Slot> = BTreeMap::new();
        for (e, _, r, row) in shown(&self.events) {
            let (Some(from), Some(to)) = (row.from, row.to) else {
                continue;
            };
            let against = hard.get(&(e, r));
            let clashes = against.is_some();
            let every = Every::of(&row.date);
            if every.is_some() {
                let day = match (row.occ.template, row.occ.days.first()) {
                    (Some(pattern), _) => pattern.weekday,
                    (None, Some(held)) => held.weekday(),
                    (None, None) => continue,
                };
                let key = (day, from, to, e, row.option, rhythm_order(every));
                let slot = slots.entry(key).or_insert_with(|| Slot::new(every, r));
                slot.pattern |= row.occ.template.is_some();
                slot.clash |= clashes;
                slot.against.extend(against.into_iter().flatten().copied());
                if let Some(held) = row.occ.days.first() {
                    slot.held_on(*held, r);
                }
                for day in dates(row) {
                    slot.spans(day);
                }
            } else {
                for held in &row.occ.days {
                    let key = (held.weekday(), from, to, e, row.option, rhythm_order(None));
                    let slot = slots.entry(key).or_insert_with(|| Slot::new(None, r));
                    slot.clash |= clashes;
                    slot.against.extend(against.into_iter().flatten().copied());
                    slot.held.insert(*held);
                    slot.held_on(*held, r);
                }
            }
        }

        slots
            .into_iter()
            .filter_map(|((day, from, to, event, option, _), slot)| {
                let label = match slot.every {
                    None => {
                        let (first, _) = slot.first?;
                        WeekLabel::Once { dates: slot.held.len(), first }
                    }
                    Some(every) => match (slot.span.filter(|_| !slot.pattern), slot.first) {
                        // A recurring row whose range holds one date is a single date (143849's
                        // 29.04. of 2026S), not „29.04.–29.04." every week.
                        (Some((first, last)), Some((held, _))) if first == last => {
                            WeekLabel::Once { dates: 1, first: held }
                        }
                        (Some((first, last)), _) if reach_of(self.facts.lecture, first, last).is_some() => {
                            WeekLabel::Partial { first, last }
                        }
                        _ => WeekLabel::Every(every),
                    },
                };
                Some(WeekItem {
                    event,
                    row: slot.first.map_or(slot.lowest, |(_, row)| row),
                    day,
                    from,
                    to,
                    label,
                    alt: open.get(event).copied().flatten().zip(option),
                    clash: slot.clash,
                    weeks: Weeks::of(slot.every),
                    against: slot.against.into_iter().collect(),
                })
            })
            .collect()
    }

    /// The dated agenda: a week per week of the lecture period, so the page can scroll to the
    /// current one, and per week with an item outside it; in each week the days with an item or
    /// a holiday. Items are the held and the cancelled dates of the visible rows, and the visible
    /// exam dates that have a day: a sitting at its time, a deadline and a day without a time on
    /// their day, a window on its first day. Dates the break or a holiday takes are no items.
    /// A Termin is one item per date however many rooms it has: the rows of one `RowKey` and
    /// option held on a day are one item, those cancelled on it another, and the rows of an exam
    /// with one shape and time on a day are one sitting.
    pub fn agenda(&self) -> Vec<AgendaWeek> {
        let mut gathered: BTreeMap<(Day, ItemKey), Gathered> = BTreeMap::new();
        for (e, _, r, row) in shown(&self.events) {
            let termin = row.key.map_or(Termin::Row(r), Termin::Key);
            let times = (row.from, row.to);
            for day in &row.occ.days {
                let notes: Vec<&str> =
                    row.occ.notes.iter().filter(|(on, _)| on == day).map(|(_, note)| note.as_str()).collect();
                let key = ItemKey::Teaching(e, termin, row.option, false);
                let item = gathered.entry((*day, key)).or_insert_with(|| Gathered::new(Some(e), None, r, false));
                item.add(r, times, None, &notes);
            }
            for (day, reason, _) in &row.occ.cancelled {
                let key = ItemKey::Teaching(e, termin, row.option, true);
                let item = gathered.entry((*day, key)).or_insert_with(|| Gathered::new(Some(e), None, r, true));
                item.add(r, times, Some(reason.as_deref().unwrap_or_default()), &[]);
            }
        }
        for (x, exam) in self.exams.iter().enumerate().filter(|(_, exam)| exam.hidden.is_none()) {
            for (r, row) in exam.rows.iter().enumerate().filter(|(_, row)| row.hidden.is_none()) {
                let Some((day, from, to)) = exam_date(&row.shape) else {
                    continue;
                };
                let key = ItemKey::Exam(x, from, to, shape_order(&row.shape));
                let item = gathered.entry((day, key)).or_insert_with(|| Gathered::new(None, Some(x), r, false));
                item.add(r, (from, to), None, &[]);
            }
        }
        let mut by_day: BTreeMap<Day, Vec<AgendaItem>> = BTreeMap::new();
        for ((day, _), item) in gathered {
            by_day.entry(day).or_default().push(item.done());
        }

        let mut mondays: BTreeSet<Day> = by_day.keys().map(|day| day.monday()).collect();
        if let Some((start, end)) = self.facts.lecture {
            mondays.extend(
                std::iter::successors(Some(start.monday()), |monday| Some(monday.plus(7)))
                    .take(MAX_WEEKS)
                    .take_while(|monday| *monday <= end),
            );
        }
        mondays
            .into_iter()
            .map(|monday| {
                let days = (0..7)
                    .map(|offset| monday.plus(offset))
                    .filter_map(|day| {
                        let holiday = self.facts.holiday(day);
                        let mut items = by_day.remove(&day).unwrap_or_default();
                        if items.is_empty() && holiday.is_none() {
                            return None;
                        }
                        items.sort_by_key(item_order);
                        Some(AgendaDay { day, holiday, items })
                    })
                    .collect();
                AgendaWeek { monday, iso_week: monday.iso_week(), break_week: self.facts.in_break(monday), days }
            })
            .collect()
    }

    /// „Ohne feste Zeit": the visible events without any dated row (`(event, None)`), and the
    /// visible rows that neither view can place: without a clear time, or without a held or
    /// cancelled date or a pattern (a rhythm like „nach Absprache", a recurring row without a
    /// weekday, a block without a range, a weekly row whose every date the break or a holiday
    /// takes). By event, then row. A row with dates but no time is in the agenda too.
    pub fn loose(&self) -> Vec<(usize, Option<usize>)> {
        let mut loose = Vec::new();
        for (e, event) in self.events.iter().enumerate().filter(|(_, event)| event.hidden.is_none()) {
            if event.rows.is_empty() {
                loose.push((e, None));
            }
            for (r, row) in event.rows.iter().enumerate().filter(|(_, row)| row.hidden.is_none()) {
                if row.from.is_none() || unplaced(row) {
                    loose.push((e, Some(r)));
                }
            }
        }
        loose
    }
}

/// Whether a row has nothing either view shows: no held or cancelled date, and no pattern. Its
/// dates may all be skipped: QIS can set a weekly Termin into the break on purpose, and a row
/// the app hid would be missed.
fn unplaced(row: &Row) -> bool {
    row.occ.days.is_empty() && row.occ.cancelled.is_empty() && row.occ.template.is_none()
}

/// The day and times an exam row stands at in the agenda, `None` for an open one.
fn exam_date(shape: &ExamShape) -> Option<(Day, Option<u16>, Option<u16>)> {
    match *shape {
        ExamShape::Sitting { day, from, to } => Some((day, Some(from), Some(to))),
        ExamShape::Deadline { day } | ExamShape::DayOnly { day } => Some((day, None, None)),
        ExamShape::Window { first, .. } => Some((first, None, None)),
        ExamShape::Open => None,
    }
}

/// Which kind of shape an exam row has, so that a window and a deadline of one exam on one day
/// stay two items.
fn shape_order(shape: &ExamShape) -> u8 {
    match shape {
        ExamShape::Sitting { .. } => 0,
        ExamShape::Deadline { .. } => 1,
        ExamShape::DayOnly { .. } => 2,
        ExamShape::Window { .. } => 3,
        ExamShape::Open => 4,
    }
}

/// The visible rows of the visible events, as `(event index, event, row index, row)`.
fn shown(events: &[Event]) -> impl Iterator<Item = (usize, &Event, usize, &Row)> {
    events.iter().enumerate().filter(|(_, event)| event.hidden.is_none()).flat_map(|(e, event)| {
        event.rows.iter().enumerate().filter(|(_, row)| row.hidden.is_none()).map(move |(r, row)| (e, event, r, row))
    })
}

/// Every date of a row: held, cancelled and skipped.
fn dates(row: &Row) -> impl Iterator<Item = Day> + '_ {
    let occ = &row.occ;
    occ.days
        .iter()
        .copied()
        .chain(occ.cancelled.iter().map(|(day, ..)| *day))
        .chain(occ.skipped.iter().map(|(day, _)| *day))
}

/// The order of the rhythms among slots of one weekday, time, event and option: weekly, A, B,
/// four-weekly, then dates that do not recur.
fn rhythm_order(every: Option<Every>) -> u8 {
    match every {
        Some(Every::Week) => 0,
        Some(Every::AWeek) => 1,
        Some(Every::BWeek) => 2,
        Some(Every::FourWeeks) => 3,
        None => 4,
    }
}

/// Which ends of `lecture` the dates from `first` to `last` leave out, comparing their weeks with
/// `PERIOD_SLACK_DAYS` to spare; `None` when they run through it, or when there is no period to
/// compare with.
fn reach_of(lecture: Option<(Day, Day)>, first: Day, last: Day) -> Option<Reach> {
    let (start, end) = lecture?;
    let near = |a: Day, b: Day| (i64::from(a.monday().0) - i64::from(b.monday().0)).abs() <= PERIOD_SLACK_DAYS;
    match (near(first, start), near(last, end)) {
        (true, true) => None,
        (true, false) => Some(Reach::Until(last)),
        (false, true) => Some(Reach::From(first)),
        (false, false) => Some(Reach::Between(first, last)),
    }
}

/// The order of a day's items: without a time first, then by start and end, teaching before
/// exams, then by event or exam and row.
fn item_order(item: &AgendaItem) -> (bool, Option<u16>, Option<u16>, bool, Option<usize>, usize) {
    (item.from.is_some(), item.from, item.to, item.exam.is_some(), item.event.or(item.exam), item.row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Code;
    use crate::queries;
    use crate::rows_detail::{DateRow, EventDate};
    use crate::timetable::day::clock;
    use crate::timetable::model::tests::{d, ids, planned, teaching, winter, Fixture};
    use crate::timetable::model::{self, Input};
    use crate::timetable::rowkey::RowKey;
    use crate::timetable::select::{Selection, TownChoice, MAX_MODULES};

    /// A date of 2026W that does not recur, at its weekday and times.
    fn single(module: &str, event: &str, ord: i64, on: &str, from: &str, to: &str) -> Fixture {
        let weekday = i64::from(d(on).weekday());
        teaching(module, event, ord, "Übung", weekday, from, to).rhythm("single").range(on, on)
    }

    /// An exam row of 2026W as `modules_exams` delivers it (no type, group or rhythm), at
    /// Zentralcampus; an empty time is none.
    fn exam(module: &str, event: &str, ord: i64, range: Option<(&str, &str)>, from: &str, to: &str) -> DateRow {
        let time = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
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
                weekday: None,
                start_time: time(from),
                end_time: time(to),
                rhythm: None,
                rhythm_raw: None,
                first_date: range.map(|(first, _)| first.into()),
                last_date: range.map(|(_, last)| last.into()),
                room: Some(format!("Raum {event}")),
                campus: Some(Code::parse("zentralcampus")),
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    /// The timetable of synthetic teaching and exam rows of 2026W, as `selection` shows it.
    fn build(rows: &[Fixture], exams: &[DateRow], modules: &[&str], selection: &Selection) -> Timetable {
        let facts = winter();
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let modules = ids(modules);
        let input = Input {
            key: facts.key,
            semester: None,
            facts: &facts,
            modules: &modules,
            schedule: &schedule,
            exams,
            sws: &[],
        };
        let t = Timetable::build(&input, selection);
        model::tests::invariants(&t);
        invariants(&t);
        t
    }

    type Slotted = (String, i64, u8, String, WeekLabel, Option<(usize, usize)>, bool);

    /// The Regelwoche as `(event id, ord of the slot's row, weekday, times, label, alt, clash)`.
    fn slots(t: &Timetable) -> Vec<Slotted> {
        t.regular_week()
            .into_iter()
            .map(|item| {
                let event = &t.events[item.event];
                let times = format!("{}–{}", clock(item.from), clock(item.to));
                (event.id.clone(), event.rows[item.row].ord.unwrap(), item.day, times, item.label, item.alt, item.clash)
            })
            .collect()
    }

    /// „Ohne feste Zeit" as `(event id, ord)`.
    fn loose(t: &Timetable) -> Vec<(String, Option<i64>)> {
        t.loose().into_iter().map(|(e, r)| (t.events[e].id.clone(), r.and_then(|r| t.events[e].rows[r].ord))).collect()
    }

    /// The days of the agenda's week of `monday`: the day, its holiday, and its items as „10/1
    /// 09:15" (teaching), „Prüfung 90/1 11:00" (an exam), „31/1+2" for the rows of one item,
    /// „—" without a time, with „fällt aus: …" and „Raum: …".
    fn week_of(t: &Timetable, monday: &str) -> Vec<(String, Option<&'static str>, Vec<String>)> {
        let agenda = t.agenda();
        let week = agenda.iter().find(|w| w.monday == d(monday)).unwrap_or_else(|| panic!("no week {monday}"));
        week.days
            .iter()
            .map(|day| {
                let items = day
                    .items
                    .iter()
                    .map(|item| {
                        let (what, ords): (String, Vec<Option<i64>>) = match (item.event, item.exam) {
                            (Some(e), None) => {
                                (t.events[e].id.clone(), item.rows.iter().map(|r| t.events[e].rows[*r].ord).collect())
                            }
                            (None, Some(x)) => (
                                format!("Prüfung {}", t.exams[x].event_id),
                                item.rows.iter().map(|r| t.exams[x].rows[*r].ord).collect(),
                            ),
                            _ => panic!("an item of neither or both"),
                        };
                        let ords: Vec<String> = ords.iter().map(|ord| ord.unwrap().to_string()).collect();
                        let from = item.from.map_or("—".to_string(), clock);
                        let mut text = format!("{what}/{} {from}", ords.join("+"));
                        if let Some(reason) = &item.cancelled {
                            text.push_str(&format!(" fällt aus: {reason}"));
                        }
                        if let Some(note) = &item.note {
                            text.push_str(&format!(" Raum: {note}"));
                        }
                        text
                    })
                    .collect();
                (day.day.iso(), day.holiday, items)
            })
            .collect()
    }

    fn texts(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    /// What holds for the views of every timetable: slots of visible, timed rows with a date, in
    /// order, labels that fit their rows, choices and clashes marked as the timetable has them,
    /// and every such row in a slot; loose rows that no view can place; an agenda by week and
    /// day with the lecture period's weeks, the breaks and the holidays, that holds every held
    /// and every cancelled date of a visible row and every dated visible exam row once, one item
    /// per Termin (or sitting) and day; and every visible row in some view.
    pub(crate) fn invariants(t: &Timetable) {
        let week = t.regular_week();
        let hard = clash::hard_rows(&t.events);
        let order: Vec<(u8, u16, u16, usize)> = week.iter().map(|i| (i.day, i.from, i.to, i.event)).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "slots in order");
        for item in &week {
            let e = &t.events[item.event];
            let r = &e.rows[item.row];
            assert!(e.hidden.is_none() && r.hidden.is_none(), "{}", e.id);
            assert!((1..=7).contains(&item.day) && item.from < item.to, "{}", e.id);
            assert_eq!((r.from, r.to), (Some(item.from), Some(item.to)), "{}", e.id);
            match item.alt {
                Some((n, o)) => assert!(e.unresolved() && n == e.visible_options().len() && r.option == Some(o)),
                None => assert!(!e.unresolved() || r.option.is_none(), "{}", e.id),
            }
            if hard.contains(&(item.event, item.row)) {
                assert!(item.clash, "{}/{:?}", e.id, r.ord);
            }
            if item.clash {
                let times = (Some(item.from), Some(item.to));
                assert!(hard.iter().any(|(he, hr)| *he == item.event && (e.rows[*hr].from, e.rows[*hr].to) == times));
            }
            match &item.label {
                WeekLabel::Once { dates, first } => {
                    assert!(*dates >= 1 && (Every::of(&r.date).is_none() || *dates == 1), "{}", e.id);
                    assert!(r.occ.days.contains(first) && first.weekday() == item.day, "{}", e.id);
                }
                WeekLabel::Every(every) => assert_eq!(Every::of(&r.date), Some(*every), "{}", e.id),
                WeekLabel::Partial { first, last } => {
                    assert!(Every::of(&r.date).is_some() && first <= last, "{}", e.id);
                    assert!(item.label.reach(&t.facts).is_some(), "{}", e.id);
                }
            }
        }
        for (e, _, _, row) in shown(&t.events) {
            let (Some(from), Some(to)) = (row.from, row.to) else {
                continue;
            };
            // A recurring row's slot may read as one date; dates that do not recur always do.
            let slotted = |day: u8, recurring: bool| {
                week.iter().any(|i| {
                    let once = matches!(i.label, WeekLabel::Once { .. });
                    i.event == e && (i.day, i.from, i.to) == (day, from, to) && (recurring || once)
                })
            };
            match (Every::of(&row.date), row.occ.template, row.occ.days.first()) {
                (Some(_), Some(pattern), _) => assert!(slotted(pattern.weekday, true)),
                (Some(_), None, Some(held)) => assert!(slotted(held.weekday(), true)),
                (Some(_), None, None) => {}
                (None, ..) => assert!(row.occ.days.iter().all(|held| slotted(held.weekday(), false))),
            }
        }

        let loose = t.loose();
        for (e, r) in &loose {
            let event = &t.events[*e];
            assert!(event.hidden.is_none());
            match r {
                None => assert!(event.rows.is_empty()),
                Some(r) => {
                    let row = &event.rows[*r];
                    assert!(row.hidden.is_none());
                    assert!(row.from.is_none() || unplaced(row));
                }
            }
        }

        let agenda = t.agenda();
        assert!(agenda.windows(2).all(|w| w[0].monday < w[1].monday), "weeks once, in order");
        if let Some((start, end)) = t.facts.lecture {
            let mut monday = start.monday();
            while monday <= end {
                assert!(agenda.iter().any(|w| w.monday == monday), "no week {}", monday.iso());
                monday = monday.plus(7);
            }
        }
        let anchors: BTreeSet<(i32, u8)> = agenda.iter().map(|w| w.iso_week).collect();
        assert_eq!(anchors.len(), agenda.len(), "the page's anchors kw-<year>-<week> once");
        // A row's date is in one item, and the rows of one Termin held (or cancelled) on a day, or
        // of one exam sitting, are in one.
        let (mut teaching, mut exams) = (0, 0);
        let mut items: BTreeSet<(Day, ItemKey)> = BTreeSet::new();
        let mut placed: BTreeSet<(usize, usize)> = BTreeSet::new();
        for w in &agenda {
            assert_eq!(w.monday.weekday(), 1);
            assert_eq!(w.iso_week, w.monday.iso_week());
            assert_eq!(w.break_week, t.facts.in_break(w.monday));
            assert!(w.days.windows(2).all(|p| p[0].day < p[1].day));
            for day in &w.days {
                assert!(w.monday <= day.day && day.day <= w.monday.plus(6));
                assert_eq!(day.holiday, t.facts.holiday(day.day));
                assert!(!day.items.is_empty() || day.holiday.is_some(), "an empty day {}", day.day.iso());
                assert!(day.items.windows(2).all(|p| item_order(&p[0]) <= item_order(&p[1])));
                for item in &day.items {
                    assert!(item.rows.windows(2).all(|p| p[0] < p[1]) && item.rows.first() == Some(&item.row));
                    match (item.event, item.exam) {
                        (Some(e), None) => {
                            let event = &t.events[e];
                            let rows: Vec<&Row> = item.rows.iter().map(|r| &event.rows[*r]).collect();
                            let first = rows[0];
                            let termin = first.key.map_or(Termin::Row(item.row), Termin::Key);
                            let key = ItemKey::Teaching(e, termin, first.option, item.cancelled.is_some());
                            assert!(items.insert((day.day, key)), "{} twice on {}", event.id, day.day.iso());
                            assert!(event.hidden.is_none(), "{}", event.id);
                            for (r, row) in item.rows.iter().zip(&rows) {
                                assert!(row.hidden.is_none() && row.option == first.option, "{}", event.id);
                                assert!((row.key.is_some() && row.key == first.key) || item.rows.len() == 1);
                                match &item.cancelled {
                                    Some(_) => assert!(row.occ.cancelled.iter().any(|c| c.0 == day.day)),
                                    None => assert!(row.occ.days.contains(&day.day)),
                                }
                                placed.insert((e, *r));
                            }
                            assert!(item.cancelled.is_none() || item.note.is_none());
                            let froms = rows.iter().filter_map(|row| row.from);
                            let tos = rows.iter().filter_map(|row| row.to);
                            assert_eq!((item.from, item.to), (froms.min(), tos.max()), "{}", event.id);
                            teaching += item.rows.len();
                        }
                        (None, Some(x)) => {
                            let exam = &t.exams[x];
                            assert!(exam.hidden.is_none() && item.cancelled.is_none() && item.note.is_none());
                            let shape = shape_order(&exam.rows[item.row].shape);
                            let key = ItemKey::Exam(x, item.from, item.to, shape);
                            assert!(items.insert((day.day, key)), "{} twice on {}", exam.event_id, day.day.iso());
                            for r in &item.rows {
                                let row = &exam.rows[*r];
                                assert!(row.hidden.is_none(), "{}", exam.event_id);
                                assert_eq!(exam_date(&row.shape), Some((day.day, item.from, item.to)));
                                assert_eq!(shape_order(&row.shape), shape);
                            }
                            exams += item.rows.len();
                        }
                        _ => panic!("an item of neither or both"),
                    }
                }
            }
        }
        let held: usize = shown(&t.events).map(|(.., row)| row.occ.days.len() + row.occ.cancelled.len()).sum();
        assert_eq!(teaching, held, "every held and cancelled date once");
        // Every visible row is in a view: with a date in the agenda, with only its pattern in the
        // Regelwoche, else loose.
        for (e, event, r, row) in shown(&t.events) {
            let patterned = row.occ.template.is_some() && row.from.is_some();
            let seen = placed.contains(&(e, r)) || patterned || loose.contains(&(e, Some(r)));
            assert!(seen, "{}/{:?} in no view", event.id, row.ord);
        }
        let dated = t
            .exams
            .iter()
            .filter(|exam| exam.hidden.is_none())
            .flat_map(|exam| &exam.rows)
            .filter(|row| row.hidden.is_none() && row.shape != ExamShape::Open)
            .count();
        assert_eq!(exams, dated, "every dated exam once");
    }

    #[test]
    fn a_slot_names_a_module_in_a_few_whole_words() {
        // Short enough: as it is, without a note in parentheses at its end.
        assert_eq!(short_title("Entwicklung von Softwaresystemen"), "Entwicklung von Softwaresystemen");
        assert_eq!(short_title(" Programmierpraktikum "), "Programmierpraktikum");
        assert_eq!(short_title("Mathematik IT-1 (Diskrete Mathematik)"), "Mathematik IT-1");
        assert_eq!(short_title("Carbon Capture and Storage (CCS)"), "Carbon Capture and Storage");
        // A parenthesis that is the whole title, or not at its end, stays.
        assert_eq!(short_title("(Studium generale)"), "(Studium generale)");
        assert_eq!(short_title("Analysis (Teil 1) und Algebra"), "Analysis (Teil 1) und Algebra");
        // Longer: cut after a whole word, never after „und", „der", „von" …, and marked.
        assert_eq!(short_title("Elektrische und elektronische Grundlagen der Informatik"), "Elektrische und elektronische …");
        assert_eq!(short_title("Einführung in die Programmierung mit Python und Java"), "Einführung in die Programmierung …");
        // At least half the limit, so the name still names the module; a word stays whole (the
        // slot cuts what it cannot hold).
        assert_eq!(short_title("Grundlagen der Betriebswirtschaftslehre für Ingenieure"), "Grundlagen der Betriebswirtschaftslehre …");
        assert_eq!(short_title("Donaudampfschifffahrtsgesellschaftskapitänsmütze Teil 2 und 3"), "Donaudampfschifffahrtsgesellschaftskapitänsmütze …");
        assert_eq!(short_title("Kurzwort Donaudampfschifffahrtsgesellschaftskapitän"), "Kurzwort Donaudampfschifffahrtsgesellschaftskapitän");
        // What is left once „in die" or „Bau- und" go would name no module: the cut takes the
        // words up to the next one that ends a name, here the whole title (review 2026-09-25: the
        // Woche read „VL Einführung …" in a slot with room for all of it).
        assert_eq!(short_title("Einführung in die Volkswirtschaftslehre"), "Einführung in die Volkswirtschaftslehre");
        assert_eq!(short_title("Einführung in das wissenschaftliche Arbeiten"), "Einführung in das wissenschaftliche …");
        assert_eq!(short_title("Wirtschafts- und Finanzmathematik"), "Wirtschafts- und Finanzmathematik");
        assert_eq!(short_title("Grundlagen Bau- und Planungsrecht"), "Grundlagen Bau- und Planungsrecht");
        assert_eq!(short_title("Informationstechnik- und Kommunikationssysteme im Verbund"), "Informationstechnik- und Kommunikationssysteme …");
        assert_eq!(short_title("Recht II: Handels- und Gesellschaftsrecht, Arbeitsrecht"), "Recht II: Handels- und Gesellschaftsrecht …");
        // A cut that would take off only what tells modules apart leaves the title whole.
        assert_eq!(short_title("Rechnernetze und Kommunikationssysteme II"), "Rechnernetze und Kommunikationssysteme II");
        assert_eq!(short_title("Forensischer Vorbereitungskurs 1 B"), "Forensischer Vorbereitungskurs 1 B");
    }

    /// Two modules' events: a weekly lecture with a cancelled date and a room note, one in a part
    /// of the period and one on Saturdays over Reformationstag, beside an Übung in A weeks with
    /// single dates, one of which meets the lecture; and two exams, one written in two rooms.
    #[test]
    fn the_week_and_the_dates_of_two_events() {
        let mut monday = teaching("A", "10", 1, "Vorlesung", 1, "09:15", "10:45");
        monday.0.cancelled_dates = Some("12.10.2026: Krankheit 19.10.2026: Raumwechsel".into());
        let rows = [
            monday,
            teaching("A", "10", 2, "Vorlesung", 4, "09:15", "10:45").range("2026-10-08", "2026-11-19"),
            teaching("A", "10", 3, "Vorlesung", 6, "09:15", "10:45").range("2026-10-24", "2026-11-07"),
            teaching("A", "10", 4, "Vorlesung", 3, "09:15", "10:45").range("2026-12-02", "2027-01-27"),
            teaching("B", "20", 1, "Übung", 2, "11:30", "13:00").rhythm("week_a"),
            single("B", "20", 2, "2026-10-09", "13:45", "15:15"),
            single("B", "20", 3, "2026-10-16", "13:45", "15:15"),
            single("B", "20", 4, "2026-11-02", "09:15", "10:45"),
            single("B", "20", 5, "2027-02-10", "10:00", "11:00"),
        ];
        let mut second_hall = exam("A", "90", 2, Some(("2027-02-11", "2027-02-11")), "11:00", "13:00");
        second_hall.date.room = Some("Audimax 2".into());
        let exams = [
            exam("A", "90", 1, Some(("2027-02-11", "2027-02-11")), "11:00", "13:00"),
            second_hall,
            exam("B", "91", 1, Some(("2027-02-15", "2027-02-19")), "", ""),
            exam("B", "91", 2, None, "", ""),
        ];
        let t = build(&rows, &exams, &["A", "B"], &Selection::default());

        let lecture = "09:15–10:45".to_string();
        let every = WeekLabel::Every(Every::Week);
        let thursdays = WeekLabel::Partial { first: d("2026-10-08"), last: d("2026-11-19") };
        let saturdays = WeekLabel::Partial { first: d("2026-10-24"), last: d("2026-11-07") };
        let wednesdays = WeekLabel::Partial { first: d("2026-12-02"), last: d("2027-01-27") };
        let once = |dates: usize, first: &str| WeekLabel::Once { dates, first: d(first) };
        assert_eq!(
            slots(&t),
            [
                // The single Monday of the Übung meets the lecture: both slots are marked.
                ("10".into(), 1, 1, lecture.clone(), every, None, true),
                ("20".into(), 4, 1, lecture.clone(), once(1, "2026-11-02"), None, true),
                ("20".into(), 1, 2, "11:30–13:00".into(), WeekLabel::Every(Every::AWeek), None, false),
                ("10".into(), 4, 3, lecture.clone(), wednesdays.clone(), None, false),
                ("20".into(), 5, 3, "10:00–11:00".into(), once(1, "2027-02-10"), None, false),
                ("10".into(), 2, 4, lecture.clone(), thursdays.clone(), None, false),
                // Two single Fridays at one time are one slot.
                ("20".into(), 2, 5, "13:45–15:15".into(), once(2, "2026-10-09"), None, false),
                ("10".into(), 3, 6, lecture, saturdays.clone(), None, false),
            ]
        );
        assert_eq!(thursdays.reach(&t.facts), Some(Reach::Until(d("2026-11-19"))));
        assert_eq!(wednesdays.reach(&t.facts), Some(Reach::From(d("2026-12-02"))));
        assert_eq!(saturdays.reach(&t.facts), Some(Reach::Between(d("2026-10-24"), d("2026-11-07"))));
        assert_eq!(WeekLabel::Every(Every::Week).reach(&t.facts), None);
        assert!(t.loose().is_empty());

        let agenda = t.agenda();
        // Every week of the lecture period, then the weeks of the February dates.
        let lecture_weeks = (0..17).map(|week| d("2026-10-05").plus(7 * week));
        let mondays: Vec<Day> = agenda.iter().map(|w| w.monday).collect();
        let expected: Vec<Day> = lecture_weeks.chain([d("2027-02-08"), d("2027-02-15")]).collect();
        assert_eq!(mondays, expected);
        assert_eq!((agenda[0].iso_week, agenda[12].iso_week), ((2026, 41), (2026, 53)));
        let breaks: Vec<Day> = agenda.iter().filter(|w| w.break_week).map(|w| w.monday).collect();
        assert_eq!(breaks, [d("2026-12-21"), d("2026-12-28")]);
        assert_eq!(
            week_of(&t, "2026-10-05"),
            [
                ("2026-10-05".into(), None, texts(&["10/1 09:15"])),
                ("2026-10-06".into(), None, texts(&["20/1 11:30"])),
                ("2026-10-08".into(), None, texts(&["10/2 09:15"])),
                ("2026-10-09".into(), None, texts(&["20/2 13:45"])),
            ]
        );
        // A cancelled date with its reason; a room note on a held one. Tuesday 13.10. is a B week.
        assert_eq!(
            week_of(&t, "2026-10-12"),
            [
                ("2026-10-12".into(), None, texts(&["10/1 09:15 fällt aus: Krankheit"])),
                ("2026-10-15".into(), None, texts(&["10/2 09:15"])),
                ("2026-10-16".into(), None, texts(&["20/3 13:45"])),
            ]
        );
        assert_eq!(week_of(&t, "2026-10-19")[0], ("2026-10-19".into(), None, texts(&["10/1 09:15 Raum: Raumwechsel"])));
        // Reformationstag names itself; the Saturday lecture does not take place.
        assert_eq!(
            week_of(&t, "2026-10-26"),
            [
                ("2026-10-26".into(), None, texts(&["10/1 09:15"])),
                ("2026-10-29".into(), None, texts(&["10/2 09:15"])),
                ("2026-10-31".into(), Some("Reformationstag"), vec![]),
            ]
        );
        assert_eq!(week_of(&t, "2026-11-02")[0], ("2026-11-02".into(), None, texts(&["10/1 09:15", "20/4 09:15"])));
        // The break: its holidays only, no lecture and no A week (29.12.).
        assert_eq!(
            week_of(&t, "2026-12-21"),
            [
                ("2026-12-25".into(), Some("1. Weihnachtstag"), vec![]),
                ("2026-12-26".into(), Some("2. Weihnachtstag"), vec![]),
            ]
        );
        assert_eq!(week_of(&t, "2026-12-28"), [("2027-01-01".into(), Some("Neujahr"), vec![])]);
        // After the lecture period: a single date and an exam sitting, once for its two rooms; a
        // window on its first day; the open exam date nowhere.
        assert_eq!(
            week_of(&t, "2027-02-08"),
            [
                ("2027-02-10".into(), None, texts(&["20/5 10:00"])),
                ("2027-02-11".into(), None, texts(&["Prüfung 90/1+2 11:00"])),
            ]
        );
        assert_eq!(week_of(&t, "2027-02-15"), [("2027-02-15".into(), None, texts(&["Prüfung 91/1 —"]))]);
    }

    /// Rows of one slot gather, choices and patterns are marked, what has no time or no date
    /// stands apart, and a Termin in three rooms is one agenda item per date.
    #[test]
    fn slots_gather_rows_and_the_rest_stands_apart() {
        // One Termin (one `RowKey`) in three rooms, the third until 12:00: the first two are
        // cancelled on 21.10., the second alone on 14.10., and the second has a room note on
        // 28.10.
        let mut first_room = teaching("A", "31", 1, "Vorlesung", 3, "09:15", "10:45");
        first_room.0.date.room = Some("Raum 1".into());
        first_room.0.cancelled_dates = Some("21.10.2026: Krankheit".into());
        let mut second_room = teaching("A", "31", 2, "Vorlesung", 3, "09:15", "10:45");
        second_room.0.date.room = Some("Raum 2".into());
        second_room.0.cancelled_dates =
            Some("14.10.2026: Krankheit 21.10.2026: Krankheit 28.10.2026: Raumwechsel".into());
        let mut third_room = teaching("A", "31", 3, "Vorlesung", 3, "09:15", "12:00");
        third_room.0.date.room = Some("Raum 3".into());
        assert!(first_room.key() == second_room.key() && second_room.key() == third_room.key());
        let mut pattern = teaching("B", "33", 1, "Seminar", 4, "11:30", "13:00").rhythm("other").undated();
        pattern.0.date.rhythm_raw = Some("vierwöch.".into());
        let mut untimed = single("B", "33", 2, "2026-10-05", "09:15", "10:45");
        (untimed.0.date.start_time, untimed.0.date.end_time) = (None, None);
        let mut by_arrangement = teaching("B", "33", 3, "Seminar", 5, "08:00", "09:00").rhythm("other");
        by_arrangement.0.date.rhythm_raw = Some("nach Absprache".into());
        let mut no_rows = teaching("B", "34", 1, "Seminar", 1, "09:15", "10:45").undated();
        no_rows.0.ord = None;
        (no_rows.0.date.weekday, no_rows.0.date.start_time, no_rows.0.date.end_time) = (None, None, None);
        let rows = [
            // Two named groups, the first with a single date beside its weekly one.
            teaching("A", "30", 1, "Übung", 1, "13:45", "15:15").group("1-Gruppe"),
            teaching("A", "30", 2, "Übung", 2, "13:45", "15:15").group("2-Gruppe"),
            single("A", "30", 3, "2026-10-15", "13:45", "15:15").group("1-Gruppe"),
            // One slot in two rooms (the third room's end makes another), and one in two ranges
            // one after the other.
            first_room,
            second_room,
            third_room,
            teaching("A", "32", 1, "Vorlesung", 5, "09:15", "10:45").range("2026-10-09", "2026-11-27"),
            teaching("A", "32", 2, "Vorlesung", 5, "09:15", "10:45").range("2026-12-04", "2027-01-29"),
            pattern,
            untimed,
            by_arrangement,
            // Every date in the break: no slot and no date, so it stands apart.
            teaching("B", "33", 4, "Seminar", 1, "11:30", "13:00").range("2026-12-21", "2026-12-28"),
            no_rows,
            teaching("B", "35", 1, "Tutorium", 5, "13:45", "15:15"),
            // A weekly row with one date in its range.
            teaching("B", "36", 1, "Seminar", 3, "15:30", "17:00").range("2026-10-14", "2026-10-14"),
        ];
        let hidden = Selection { hidden_events: [35].into(), ..Selection::default() };
        let t = build(&rows, &[], &["A", "B"], &hidden);
        let every = |rhythm: Every| WeekLabel::Every(rhythm);
        let group = "13:45–15:15".to_string();
        let first_group_once = WeekLabel::Once { dates: 1, first: d("2026-10-15") };
        assert_eq!(
            slots(&t),
            [
                ("30".into(), 1, 1, group.clone(), every(Every::Week), Some((2, 0)), false),
                ("30".into(), 2, 2, group.clone(), every(Every::Week), Some((2, 1)), false),
                ("31".into(), 1, 3, "09:15–10:45".into(), every(Every::Week), None, false),
                ("31".into(), 3, 3, "09:15–12:00".into(), every(Every::Week), None, false),
                (
                    "36".into(),
                    1,
                    3,
                    "15:30–17:00".into(),
                    WeekLabel::Once { dates: 1, first: d("2026-10-14") },
                    None,
                    false
                ),
                ("33".into(), 1, 4, "11:30–13:00".into(), every(Every::FourWeeks), None, false),
                ("30".into(), 3, 4, group.clone(), first_group_once, Some((2, 0)), false),
                ("32".into(), 1, 5, "09:15–10:45".into(), every(Every::Week), None, false),
            ]
        );
        assert_eq!(
            loose(&t),
            [("33".into(), Some(2)), ("33".into(), Some(3)), ("33".into(), Some(4)), ("34".into(), None)]
        );
        // The untimed date is in the agenda too, before the timed ones of its day; the row in the
        // break has no date there.
        assert_eq!(week_of(&t, "2026-10-05")[0], ("2026-10-05".into(), None, texts(&["33/2 —", "30/1 13:45"])));
        // The Termin in three rooms: one item a date, from the first start to the last end; the
        // rooms held and those cancelled on one date apart, the cancelled ones together.
        let wednesday = |monday: &str| week_of(&t, monday).into_iter().find(|day| d(&day.0).weekday() == 3);
        assert_eq!(wednesday("2026-10-05"), Some(("2026-10-07".into(), None, texts(&["31/1+2+3 09:15"]))));
        let lecture = t.events.iter().position(|e| e.id == "31").unwrap();
        let agenda = t.agenda();
        let first = &agenda[0].days.iter().find(|day| day.day == d("2026-10-07")).unwrap().items[0];
        assert_eq!((first.event, first.row, first.rows.as_slice()), (Some(lecture), 0, [0, 1, 2].as_slice()));
        assert_eq!((first.from, first.to), (Some(555), Some(720)));
        let fourteenth = texts(&["31/2 09:15 fällt aus: Krankheit", "31/1+3 09:15", "36/1 15:30"]);
        assert_eq!(wednesday("2026-10-12"), Some(("2026-10-14".into(), None, fourteenth)));
        assert_eq!(
            wednesday("2026-10-19"),
            Some(("2026-10-21".into(), None, texts(&["31/1+2 09:15 fällt aus: Krankheit", "31/3 09:15"])))
        );
        assert_eq!(
            wednesday("2026-10-26"),
            Some(("2026-10-28".into(), None, texts(&["31/1+2+3 09:15 Raum: Raumwechsel"])))
        );
        let seminar = t.events.iter().position(|e| e.id == "33").unwrap();
        let agenda = t.agenda();
        let items: Vec<&AgendaItem> = agenda.iter().flat_map(|w| &w.days).flat_map(|day| &day.items).collect();
        assert!(!items.is_empty());
        assert!(items
            .iter()
            .all(|item| item.event != Some(seminar) || t.events[seminar].rows[item.row].ord != Some(4)));

        // „Nur diesen" on the second group: its slot stays, without „1 von 2".
        let chosen = Selection { chosen_rows: [rows[1].key()].into(), ..hidden };
        let t = build(&rows, &[], &["A", "B"], &chosen);
        let groups: Vec<Slotted> = slots(&t).into_iter().filter(|slot| slot.0 == "30").collect();
        assert_eq!(groups, [("30".into(), 2, 2, group, every(Every::Week), None, false)]);
    }

    /// Informatik's first semester on the pinned snapshot; on any snapshot, the invariants of the
    /// first sixty modules with a dated row in the current semester, in the derived town and in
    /// both.
    #[test]
    fn the_first_semester_as_a_week_and_as_dates() {
        let pinned = crate::tests::studyplan_db("the_first_semester_as_a_week_and_as_dates");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let current = queries::meta(&db).unwrap().current_semester.unwrap();
        let mut many: Vec<String> = Vec::new();
        for row in queries::semester_schedule(&db, &current).unwrap() {
            if many.len() < MAX_MODULES && !many.contains(&row.module_id) {
                many.push(row.module_id);
            }
        }
        let both = Selection { town: TownChoice::Both, ..Selection::default() };
        for selection in [Selection::default(), both] {
            let t = planned(&db, &current, &many, &selection);
            invariants(&t);
            assert!(!t.regular_week().is_empty() && !t.agenda().is_empty());
        }
        if !is_pinned {
            return;
        }

        let fs1 = ids(&["12104", "12107", "12102", "11112"]);
        let t = planned(&db, "2026W", &fs1, &Selection::default());
        invariants(&t);
        let agenda = t.agenda();
        // A week per KW of the lecture period, the break from 21.12., and the weeks of the exams.
        let mondays: Vec<String> = agenda.iter().map(|w| w.monday.iso()).collect();
        let lecture_weeks = (0..17).map(|week| d("2026-10-05").plus(7 * week).iso());
        let expected: Vec<String> = lecture_weeks.chain(["2027-02-08".into(), "2027-03-08".into()]).collect();
        assert_eq!(mondays, expected);
        let breaks: Vec<String> = agenda.iter().filter(|w| w.break_week).map(|w| w.monday.iso()).collect();
        assert_eq!(breaks, ["2026-12-21", "2026-12-28"]);
        assert!(agenda.iter().filter(|w| w.break_week).all(|w| w.days.iter().all(|day| day.items.is_empty())));
        // The exam sittings; Senftenberg's 150664 goes with the town.
        let sittings: Vec<(String, String, String)> = agenda
            .iter()
            .flat_map(|w| &w.days)
            .flat_map(|day| {
                day.items.iter().filter_map(|item| {
                    let x = item.exam?;
                    Some((day.day.iso(), t.exams[x].event_id.clone(), clock(item.from?)))
                })
            })
            .collect();
        let at = |day: &str, event: &str| (day.to_string(), event.to_string(), "11:00".to_string());
        assert_eq!(sittings, [at("2027-02-11", "148097"), at("2027-03-10", "148664"), at("2027-03-12", "148689")]);

        // The Regelwoche: every visible row a weekly or A/B slot, 148701's Tuesday from the
        // second week (13.10.) through the period; the choices „1 von n"; the clashes of 149408
        // (Sachsendorf) and 148455 with 148134/148135, and 148369's Tuesday option against the
        // Tutorium 150132 dropped.
        let week = slots(&t);
        assert_eq!(week.len(), 22);
        assert!(loose(&t).is_empty());
        assert!(week.iter().all(|slot| !matches!(slot.4, WeekLabel::Partial { .. } | WeekLabel::Once { .. })));
        assert!(week.iter().all(|slot| !["149406", "149407"].contains(&slot.0.as_str())));
        let labelled =
            |event: &str, ord: i64| week.iter().find(|slot| slot.0 == event && slot.1 == ord).unwrap().4.clone();
        assert_eq!(labelled("148701", 1), WeekLabel::Every(Every::Week));
        assert_eq!(labelled("148008", 1), WeekLabel::Every(Every::AWeek));
        assert_eq!(labelled("148135", 1), WeekLabel::Every(Every::BWeek));
        let mut alternatives: Vec<(&str, i64, (usize, usize))> =
            week.iter().filter_map(|slot| Some((slot.0.as_str(), slot.1, slot.5?))).collect();
        alternatives.sort_unstable();
        let mut expected: Vec<(&str, i64, (usize, usize))> = Vec::new();
        for (event, n) in [("148304", 3), ("148369", 4), ("148370", 4)] {
            expected.extend((0..n).map(|o| (event, i64::try_from(o).unwrap() + 1, (n, o))));
        }
        assert_eq!(alternatives, expected);
        let mut clashing: Vec<(&str, i64)> =
            week.iter().filter(|slot| slot.6).map(|slot| (slot.0.as_str(), slot.1)).collect();
        clashing.sort_unstable();
        assert_eq!(clashing, [("148134", 1), ("148134", 2), ("148135", 1), ("148455", 1), ("149408", 1)]);

        // Hiding 149408 and 148455 leaves nothing marked.
        let hidden = Selection { hidden_events: [149408, 148455].into(), ..Selection::default() };
        let t = planned(&db, "2026W", &fs1, &hidden);
        invariants(&t);
        assert!(slots(&t).iter().all(|slot| !slot.6));
        // „Nur diesen" on Monday 17:30 leaves one of 148369's slots, without „1 von 4".
        let chosen = Selection { chosen_rows: [RowKey::parse("148369-a4d12").unwrap()].into(), ..hidden };
        let t = planned(&db, "2026W", &fs1, &chosen);
        invariants(&t);
        let options: Vec<Slotted> = slots(&t).into_iter().filter(|slot| slot.0 == "148369").collect();
        assert_eq!(
            options,
            [("148369".into(), 2, 1, "17:30–19:00".into(), WeekLabel::Every(Every::Week), None, false)]
        );

        // One Termin in four rooms is one date a week (13262's 149674, Wednesdays at 09:15), and
        // an exam in two halls one sitting (13102's 148703 and 11212's 148325 at 08:00).
        let dates_of = |plan: &str, id: &str| -> Vec<(String, usize)> {
            let t = planned(&db, "2026W", &ids(&[plan]), &Selection::default());
            let mut dates = Vec::new();
            for day in t.agenda().into_iter().flat_map(|w| w.days) {
                for item in &day.items {
                    let of = item.event.map(|e| &t.events[e].id).or(item.exam.map(|x| &t.exams[x].event_id));
                    if of.is_some_and(|of| of == id) {
                        dates.push((day.day.iso(), item.rows.len()));
                    }
                }
            }
            dates
        };
        let wednesdays = dates_of("13262", "149674");
        assert_eq!(wednesdays.len(), 15);
        assert!(wednesdays.iter().all(|(day, rows)| d(day).weekday() == 3 && *rows == 4));
        assert_eq!(dates_of("13102", "148703"), [("2027-02-12".to_string(), 2)]);
        assert_eq!(dates_of("11212", "148325"), [("2027-02-17".to_string(), 2)]);
    }
}
