//! Exam sittings as dates, their ranks, and warnings for overlaps and short hops between
//! campuses.
//!
//! An exam row is taken as a page shows it (`exam_reading`): QIS's placeholder loses its time, a
//! deadline reads „bis 24:00", everything else keeps what the source says. What is left has one of
//! five shapes, and only a sitting (one day with a start and an end) has a place in the day. So
//! only sittings are ranked and warned about; a window, a deadline, a day without a time and an
//! open date never are.
//!
//! A module's sittings in one semester are alternatives of each other: the first and the second
//! Termin, or a retake event shared by many modules. The Termine of a planned module are the
//! distinct days and times of its visible sittings, so two rooms or two events at the same time
//! are one Termin, and rows sharing a row key are one Termin until their latest end, as the feed
//! merges them. A sitting is rank 1 when it lies on the day of the earliest Termin of a planned
//! module it belongs to, else rank 2 („2. Termin"); the calendar feed marks rank 2 and retakes
//! tentative.
//!
//! Two modules' Termine on one day warn when their times overlap, or when the hop between their
//! campuses is too short: under `CITY_GAP` between Cottbus and Senftenberg, under `SITE_GAP`
//! between two Cottbus sites. Only an exam event both modules share is one exam; a shared room is
//! not, since the big halls hold several exams at once. A warning is hard only when no
//! combination of the two modules' Termine avoids it: a false warning costs a line, a missed exam
//! costs a semester. Where a campus is not known the hop cannot be judged, and the day gets a
//! muted line instead of a warning. `collision` is that rule for one pair of Termine, so the
//! finder checks a candidate's Termine by it too.

use std::collections::{BTreeMap, BTreeSet};

use super::day::{minutes, Day};
use super::kind::{fold, EventKind, KindSet};
use super::rowkey::RowKey;
use super::select::{town_of, HiddenBy, Selection, Town};
use crate::exam_reading::{self, ExamReading, Reason};
use crate::labels::{Campus, Code};
use crate::rows::Semester;
use crate::rows_detail::{DateRow, EventDate};

/// What an exam date is, as a page shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum ExamShape {
    /// One day with a start and an end, in minutes, `from ≤ to`: the only shape with a place in
    /// the day, so the only one that is ranked and warned about.
    Sitting { day: Day, from: u16, to: u16 },
    /// Something is due that day, „bis 24:00".
    Deadline { day: Day },
    /// Several days: oral exams „nach Absprache", mostly.
    Window { first: Day, last: Day },
    /// A day without a time, or with times that end before they start.
    DayOnly { day: Day },
    /// No day at all: „Termin offen".
    Open,
}

/// One exam event of the planned modules in a semester.
#[derive(Clone, Debug, PartialEq)]
pub struct Exam {
    pub event_id: String,
    pub title: String,
    /// The planned modules that link it, in plan order.
    pub modules: Vec<String>,
    /// Its dates, one per `ord`, in that order.
    pub rows: Vec<ExamRow>,
    pub hidden: Option<HiddenBy>,
    /// A retake by its title (`retake`): the feed marks its sittings tentative, since a student
    /// sits it only after failing the first.
    pub retake: bool,
    pub source_url: Option<String>,
}

/// One date of an exam event.
#[derive(Clone, Debug, PartialEq)]
pub struct ExamRow {
    pub key: Option<RowKey>,
    pub ord: Option<i64>,
    pub date: EventDate,
    pub reading: ExamReading,
    pub shape: ExamShape,
    /// The exam's reason when the whole exam is hidden, else `Row` when this Termin is.
    pub hidden: Option<HiddenBy>,
    /// 1 = on the day of the earliest Termin of a planned module it belongs to; 2 = on a later
    /// day; 0 = no visible sitting (another shape, or hidden). Visible sittings of one row key
    /// share day and start, so they always carry one rank.
    pub rank: u8,
}

/// Minutes between the end of one exam and the start of the next that are too few for a hop
/// between two Cottbus sites (Zentralcampus, Sachsendorf, Nord): 4–5 km, 20–25 minutes by tram or
/// bus, plus waiting and an exam that ends late.
pub const SITE_GAP: u16 = 45;

/// Minutes too few for a hop between Cottbus and Senftenberg: about 40 minutes by the hourly
/// regional train, 15–20 minutes of walking to and from each station, and up to an hour more when
/// the train is missed.
pub const CITY_GAP: u16 = 150;

/// Why two Termine of one day warn.
#[derive(Clone, Debug, PartialEq)]
pub enum WarningKind {
    /// Their times intersect.
    Overlap,
    /// `gap` minutes from the end of the earlier one at `from` to the start of the later one at
    /// `to` are too few for that hop.
    Tight { gap: u16, from: Code<Campus>, to: Code<Campus> },
}

/// Two planned modules whose exams collide on `day`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExamWarning {
    pub kind: WarningKind,
    pub day: Day,
    /// The Termin that starts first; at an equal start, the one of the module planned first.
    pub a: Termin,
    pub b: Termin,
    /// No combination of the two modules' Termine avoids an issue.
    pub hard: bool,
    /// For a soft warning, the day of a Termin that avoids it („Zweittermin 11.03. passt").
    pub avoid: Option<Day>,
}

/// A Termin of one planned module: its visible sittings at one day and time, wherever held. The
/// day is the warning's (or its `TerminAt`'s).
#[derive(Clone, Debug, PartialEq)]
pub struct Termin {
    pub module_id: String,
    /// The exam event of its first sitting.
    pub event_id: String,
    pub from: u16,
    /// The latest end of its sittings.
    pub to: u16,
    /// The campuses of its sittings, each once, in the order met; a sitting without a campus adds
    /// none.
    pub campus: Vec<Code<Campus>>,
    /// The room when every sitting names the same one.
    pub room: Option<String>,
}

/// A Termin at its day, with what `collision` compares beyond it.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminAt {
    pub day: Day,
    pub termin: Termin,
    /// Every exam event with a sitting in this Termin, each once: two modules that share one sit
    /// one exam.
    pub events: Vec<String>,
    /// Some sitting has no campus, or one in no known town: its hops cannot be judged.
    pub unplaced: bool,
}

/// In a folded exam title: a retake („Wiederholungsprüfung", „Nachprüfung", „Nachklausur",
/// „Nach-/Wiederholungsprüfung", „only for retake exams").
const RETAKE_WORDS: [&str; 5] = ["wiederhol", "nachprüf", "nachklausur", "nach-/", "retake"];

/// Whole words of a folded exam title that name a retake by an abbreviation: „WMAP", the
/// Wiederholungs-MAP of the health and teacher programs, and „Wdh.".
const RETAKE_TAGS: [&str; 2] = ["wmap", "wdh"];

/// The whole word of a first attempt, the Modulabschlussprüfung. A title that names it beside a
/// retake („(MAP+V) und WMAP"; „W-/MAP", first attempt or retake) is a first attempt too.
const FIRST_TAG: &str = "map";

/// Whether an exam title names a retake, and no first attempt beside it.
pub fn retake(title: &str) -> bool {
    let folded = fold(title);
    let words: Vec<&str> = folded.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).collect();
    if words.contains(&FIRST_TAG) {
        return false;
    }
    RETAKE_WORDS.iter().any(|word| folded.contains(word)) || words.iter().any(|word| RETAKE_TAGS.contains(word))
}

/// The shape of an exam date as the reading shows it: a deadline is one; two different days are a
/// window; one day is a sitting when it has a start and an end that is not before it, else a day
/// alone; no day is open. A placeholder whose date lies outside the semester has no shown date and
/// is open (148663's 01:00–02:30 on 27.12.2015 in the WiSe 2026/27).
pub fn shape_of(reading: &ExamReading) -> ExamShape {
    let shown = &reading.shown;
    let first = shown.first_date.as_deref().and_then(Day::parse);
    let last = shown.last_date.as_deref().and_then(Day::parse);
    let (first, last) = match (first, last) {
        (Some(first), Some(last)) => (first.min(last), first.max(last)),
        (Some(day), None) | (None, Some(day)) => (day, day),
        (None, None) => return ExamShape::Open,
    };
    if reading.has(Reason::Deadline) {
        return ExamShape::Deadline { day: last };
    }
    if first != last {
        return ExamShape::Window { first, last };
    }
    let from = shown.start_time.as_deref().and_then(minutes);
    let to = shown.end_time.as_deref().and_then(minutes);
    match from.zip(to) {
        // An end before the start is kept and marked by the reading, but it gives no time to
        // compare: unknown is never a conflict.
        Some((from, to)) if from <= to => ExamShape::Sitting { day: first, from, to },
        _ => ExamShape::DayOnly { day: first },
    }
}

/// The planned modules' exam events of the semester, deduplicated by `(event_id, ord)`, with
/// shapes, ranks and visibility. Rows of modules that are not planned are left out; an event keeps
/// only its planned modules. Visibility follows the timetable's rules for exams, first that holds:
/// the event is hidden; the kind „Prüfung" is; `town` is the effective town, every linking module
/// has city tracks and the exam's town is the other one; and per row, its key is hidden.
pub fn exams_of(
    rows: &[DateRow],
    semester: Option<&Semester>,
    modules: &[String],
    selection: &Selection,
    town: Option<Town>,
    tracks: &BTreeSet<String>,
) -> Vec<Exam> {
    // Per event: the plan positions of its planned modules and its rows, each `ord` once (the
    // view lists an event once for every module that links it).
    let mut events: BTreeMap<&str, (BTreeSet<usize>, Vec<&DateRow>)> = BTreeMap::new();
    let mut seen: BTreeSet<(&str, Option<i64>)> = BTreeSet::new();
    for row in rows {
        let Some(position) = modules.iter().position(|module| *module == row.module_id) else {
            continue;
        };
        let event_id = row.date.event_id.as_str();
        let (positions, event_rows) = events.entry(event_id).or_default();
        positions.insert(position);
        if seen.insert((event_id, row.ord)) {
            event_rows.push(row);
        }
    }

    let mut exams: Vec<(usize, Exam)> = Vec::with_capacity(events.len());
    for (event_id, (positions, mut event_rows)) in events {
        event_rows.sort_by_key(|row| row.ord);
        let (Some(first_position), Some(first_row)) = (positions.first().copied(), event_rows.first()) else {
            continue;
        };
        let linked: Vec<String> = positions.iter().filter_map(|position| modules.get(*position).cloned()).collect();
        let hidden = exam_hidden(event_id, &event_rows, &linked, selection, town, tracks);
        let title = first_row.date.event_title.clone();
        let source_url = first_row.date.source_url.clone();
        let exam_rows = event_rows
            .iter()
            .map(|row| {
                let reading = exam_reading::read(&row.date, semester);
                let shape = shape_of(&reading);
                let key = RowKey::of(&row.date);
                let row_hidden = key.is_some_and(|key| selection.hidden_rows.contains(&key)).then_some(HiddenBy::Row);
                ExamRow { key, ord: row.ord, date: row.date.clone(), reading, shape, hidden: hidden.or(row_hidden), rank: 0 }
            })
            .collect();
        let exam = Exam {
            event_id: event_id.to_string(),
            retake: retake(&title),
            title,
            modules: linked,
            rows: exam_rows,
            hidden,
            source_url,
        };
        exams.push((first_position, exam));
    }
    // By the first linking module's place in the plan, then by title and event, as the events of
    // the timetable are.
    exams.sort_by(|(p, a), (q, b)| {
        p.cmp(q)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| (a.event_id.len(), &a.event_id).cmp(&(b.event_id.len(), &b.event_id)))
    });
    let mut exams: Vec<Exam> = exams.into_iter().map(|(_, exam)| exam).collect();
    rank(&mut exams);
    exams
}

/// Why a whole exam is hidden: the event, the kind, or the town, in this order.
fn exam_hidden(
    event_id: &str,
    rows: &[&DateRow],
    linked: &[String],
    selection: &Selection,
    town: Option<Town>,
    tracks: &BTreeSet<String>,
) -> Option<HiddenBy> {
    if event_number(event_id).is_some_and(|id| selection.hidden_events.contains(&id)) {
        return Some(HiddenBy::Event);
    }
    if KindSet::default().with(EventKind::Exam).hidden_by(selection.hidden_kinds) {
        return Some(HiddenBy::Kinds);
    }
    // Tracks are whole courses: the exam of a module taken in Cottbus is the Cottbus one. An exam
    // that also belongs to a module without tracks, or whose town is mixed or unknown, stays.
    let shown = town?;
    let all_tracks = !linked.is_empty() && linked.iter().all(|module| tracks.contains(module));
    (all_tracks && single_town(rows).is_some_and(|own| own != shown)).then_some(HiddenBy::Town(shown))
}

/// The one town of all rows with a known campus; `None` when they are in both or none is known.
fn single_town(rows: &[&DateRow]) -> Option<Town> {
    let mut towns = rows.iter().filter_map(|row| row.date.campus.as_ref().and_then(town_of));
    let first = towns.next()?;
    towns.all(|town| town == first).then_some(first)
}

/// A `veranstid` as the selection keeps it: digits without a leading zero that fit a `u32`.
fn event_number(id: &str) -> Option<u32> {
    let canonical = !id.is_empty() && !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| id.parse().ok()).flatten()
}

/// The day and times of a visible sitting.
fn visible_sitting(row: &ExamRow) -> Option<(Day, u16, u16)> {
    match row.shape {
        ExamShape::Sitting { day, from, to } if row.hidden.is_none() => Some((day, from, to)),
        _ => None,
    }
}

/// Sets the ranks: a visible sitting is 1 when it lies on the day of the earliest Termin of at
/// least one of its planned modules, else 2; every other row is 0. By day, not by time: the
/// sittings of one day are one attempt, whether rows of one key with two ends (150907's
/// 09:00–11:00 and 09:00–11:30 in one room), two parts (12128's 08:00 and 09:15) or the two towns'
/// exams of a track module no town was chosen for (11107's 11:00 in Senftenberg and 14:00 at
/// Zentralcampus). Hiding a module's first sitting makes its next one the first.
fn rank(exams: &mut [Exam]) {
    let mut first_day: BTreeMap<String, Day> = BTreeMap::new();
    for exam in exams.iter() {
        for (day, ..) in exam.rows.iter().filter_map(visible_sitting) {
            for module in &exam.modules {
                first_day.entry(module.clone()).and_modify(|first| *first = (*first).min(day)).or_insert(day);
            }
        }
    }
    for exam in exams.iter_mut() {
        let Exam { modules, rows, .. } = exam;
        for row in rows.iter_mut() {
            row.rank = match visible_sitting(row) {
                Some((day, ..)) if modules.iter().any(|module| first_day.get(module) == Some(&day)) => 1,
                Some(_) => 2,
                None => 0,
            };
        }
    }
}

/// The latest end of the visible sittings of each row key at a day and start. Rows sharing a key
/// are one calendar entry, which the feed merges to the latest end (149685's 09:00–13:30 and
/// 09:00–17:00), so they are one Termin until then: the longer sitting is the one that collides.
fn latest_ends(exams: &[Exam]) -> BTreeMap<(RowKey, Day, u16), u16> {
    let mut ends: BTreeMap<(RowKey, Day, u16), u16> = BTreeMap::new();
    for row in exams.iter().flat_map(|exam| &exam.rows) {
        if let (Some(key), Some((day, from, to))) = (row.key, visible_sitting(row)) {
            ends.entry((key, day, from)).and_modify(|end| *end = (*end).max(to)).or_insert(to);
        }
    }
    ends
}

impl TerminAt {
    fn new(module_id: &str, event_id: &str, day: Day, from: u16, to: u16) -> Self {
        let termin = Termin {
            module_id: module_id.to_string(),
            event_id: event_id.to_string(),
            from,
            to,
            campus: Vec::new(),
            room: None,
        };
        TerminAt { day, termin, events: Vec::new(), unplaced: false }
    }

    fn add(&mut self, event_id: &str, date: &EventDate) {
        if !self.events.iter().any(|event| event == event_id) {
            self.events.push(event_id.to_string());
        }
        match &date.campus {
            Some(campus) => {
                if !self.termin.campus.contains(campus) {
                    self.termin.campus.push(campus.clone());
                }
                self.unplaced |= town_of(campus).is_none();
            }
            None => self.unplaced = true,
        }
    }
}

/// The Termine of each planned module, in plan order, each earliest first.
fn held(exams: &[Exam], modules: &[String]) -> Vec<Vec<TerminAt>> {
    let ends = latest_ends(exams);
    modules
        .iter()
        .map(|module| {
            // Each Termin with the rooms its sittings name, each once (`None` for a sitting
            // without one).
            let mut termine: Vec<(TerminAt, Vec<Option<&str>>)> = Vec::new();
            for exam in exams.iter().filter(|exam| exam.hidden.is_none() && exam.modules.contains(module)) {
                for row in &exam.rows {
                    let Some((day, from, to)) = visible_sitting(row) else {
                        continue;
                    };
                    let to = row.key.and_then(|key| ends.get(&(key, day, from)).copied()).unwrap_or(to);
                    let at = (day, from, to);
                    let index = match termine.iter().position(|(t, _)| (t.day, t.termin.from, t.termin.to) == at) {
                        Some(index) => index,
                        None => {
                            termine.push((TerminAt::new(module, &exam.event_id, day, from, to), Vec::new()));
                            termine.len() - 1
                        }
                    };
                    if let Some((termin, rooms)) = termine.get_mut(index) {
                        termin.add(&exam.event_id, &row.date);
                        let room = row.date.room.as_deref().map(str::trim).filter(|room| !room.is_empty());
                        if !rooms.contains(&room) {
                            rooms.push(room);
                        }
                    }
                }
            }
            termine.sort_by_key(|(t, _)| (t.day, t.termin.from, t.termin.to));
            termine
                .into_iter()
                .map(|(mut termin, rooms)| {
                    termin.termin.room = match rooms.as_slice() {
                        [Some(room)] => Some(room.to_string()),
                        _ => None,
                    };
                    termin
                })
                .collect()
        })
        .collect()
}

/// The Termine of each planned module, in plan order: the distinct days and times of its visible
/// sittings, earliest first. The finder takes a plan's fixed Termine (a module with exactly one)
/// from them and checks a candidate's against those with `collision`.
pub fn termine(exams: &[Exam], modules: &[String]) -> Vec<(String, Vec<TerminAt>)> {
    modules.iter().cloned().zip(held(exams, modules)).collect()
}

/// Whether two Termine of different modules collide, by the rule `exam_warnings` warns with: an
/// overlap, or a hop too short for its campuses, on one day, unless they share an exam event.
/// `None` also when a campus is unknown and the hop may be too short: unknown is never a conflict.
pub fn collision(a: &TerminAt, b: &TerminAt) -> Option<WarningKind> {
    match judge(a, b) {
        Judged::Issue { kind, .. } => Some(kind),
        Judged::Unplaced | Judged::Fine => None,
    }
}

/// What one combination of two modules' Termine says.
enum Judged {
    /// They collide; `a_first` when the first Termin of the pair starts first.
    Issue { kind: WarningKind, a_first: bool },
    /// No collision known, but a campus is unknown and the hop may be too short.
    Unplaced,
    Fine,
}

/// Two Termine of different modules. Other days and one exam event (a shared exam) never collide.
/// Two events at one time in one room do: the big halls hold several exams at once (148689
/// „Entwicklung von Softwaresystemen" and 152385 „Softwaresystemtechnik" in Audimax 1 on
/// 12.03.2027 at 11:00, and three programs hold both modules), while one exam held under two
/// titles costs a student who plans both only a line.
fn judge(a: &TerminAt, b: &TerminAt) -> Judged {
    let (ta, tb) = (&a.termin, &b.termin);
    if a.day != b.day || a.events.iter().any(|event| b.events.contains(event)) {
        return Judged::Fine;
    }
    let a_first = ta.from <= tb.from;
    if ta.from == tb.from || (ta.from < tb.to && tb.from < ta.to) {
        return Judged::Issue { kind: WarningKind::Overlap, a_first };
    }
    let (earlier, later) = if a_first { (a, b) } else { (b, a) };
    let gap = later.termin.from.saturating_sub(earlier.termin.to);
    if let Some((from, to)) = too_close(earlier, later, gap) {
        return Judged::Issue { kind: WarningKind::Tight { gap, from, to }, a_first };
    }
    if (earlier.unplaced || later.unplaced) && gap < CITY_GAP {
        Judged::Unplaced
    } else {
        Judged::Fine
    }
}

/// The campuses of a hop that `gap` minutes do not allow: every pairing of the earlier Termin's
/// campuses with the later one's must be far enough. A hop between the towns is named before one
/// between Cottbus sites.
fn too_close(earlier: &TerminAt, later: &TerminAt, gap: u16) -> Option<(Code<Campus>, Code<Campus>)> {
    let pairs = || {
        earlier.termin.campus.iter().flat_map(|from| later.termin.campus.iter().map(move |to| (from, to)))
    };
    let towns = |from: &Code<Campus>, to: &Code<Campus>| town_of(from).zip(town_of(to));
    let city = || pairs().find(|(from, to)| gap < CITY_GAP && towns(from, to).is_some_and(|(x, y)| x != y));
    let site = || pairs().find(|(from, to)| gap < SITE_GAP && from != to && towns(from, to).is_some_and(|(x, y)| x == y));
    city().or_else(site).map(|(from, to)| (from.clone(), to.clone()))
}

/// The day of the Termin that avoids a soft warning's issue: of the combinations without one,
/// those that change one module's Termin come first, then the earliest day.
fn avoiding(mine: &[TerminAt], theirs: &[TerminAt], issue: (usize, usize), free: &[(usize, usize)]) -> Option<Day> {
    free.iter()
        .filter_map(|&(x, y)| {
            let (a, b) = (mine.get(x)?, theirs.get(y)?);
            let changed = (x != issue.0, y != issue.1);
            let day = match changed {
                (true, false) => a.day,
                (false, true) => b.day,
                _ => a.day.max(b.day),
            };
            Some((usize::from(changed.0) + usize::from(changed.1), day))
        })
        .min()
        .map(|(_, day)| day)
}

/// Where a warning stands in the list: its day, the starts of its earlier and its later Termin,
/// and the plan positions of the two modules.
type Order = (Day, u16, u16, usize, usize);

/// The warnings between the planned modules' visible Termine, one per pair of modules that
/// collide (its earliest issue), by day; and the days whose hops cannot be judged because a campus
/// is unknown, each with its modules in plan order („Ort offen: 2 Prüfungen am …").
pub fn exam_warnings(exams: &[Exam], modules: &[String]) -> (Vec<ExamWarning>, Vec<(Day, Vec<String>)>) {
    let termine = held(exams, modules);
    let mut warnings: Vec<(Order, ExamWarning)> = Vec::new();
    let mut unplaced: BTreeMap<Day, BTreeSet<usize>> = BTreeMap::new();
    for (i, mine) in termine.iter().enumerate() {
        for (j, theirs) in termine.iter().enumerate().skip(i + 1) {
            let mut issues: Vec<(usize, usize, WarningKind, bool)> = Vec::new();
            let mut free: Vec<(usize, usize)> = Vec::new();
            for (x, a) in mine.iter().enumerate() {
                for (y, b) in theirs.iter().enumerate() {
                    match judge(a, b) {
                        Judged::Issue { kind, a_first } => issues.push((x, y, kind, a_first)),
                        Judged::Unplaced => {
                            unplaced.entry(a.day).or_default().extend([i, j]);
                            free.push((x, y));
                        }
                        Judged::Fine => free.push((x, y)),
                    }
                }
            }
            // The earliest issue: by day, then by the start of its earlier Termin, then the later's.
            let earliest = issues
                .iter()
                .filter_map(|(x, y, kind, a_first)| {
                    let (a, b) = (mine.get(*x)?, theirs.get(*y)?);
                    let (first, second) = if *a_first { (a, b) } else { (b, a) };
                    Some(((first.day, first.termin.from, second.termin.from), (*x, *y), kind, first, second))
                })
                .min_by_key(|(key, ..)| *key);
            let Some(((day, from, then), issue, kind, first, second)) = earliest else {
                continue;
            };
            let hard = free.is_empty();
            let avoid = if hard { None } else { avoiding(mine, theirs, issue, &free) };
            let warning =
                ExamWarning { kind: kind.clone(), day, a: first.termin.clone(), b: second.termin.clone(), hard, avoid };
            warnings.push(((day, from, then, i, j), warning));
        }
    }
    warnings.sort_by_key(|(order, _)| *order);
    let unplaced = unplaced
        .into_iter()
        .map(|(day, positions)| (day, positions.into_iter().filter_map(|i| modules.get(i).cloned()).collect()))
        .collect();
    (warnings.into_iter().map(|(_, warning)| warning).collect(), unplaced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::queries;
    use crate::timetable::select::MAX_MODULES;

    const ZC: &str = "zentralcampus";
    const SD: &str = "sachsendorf";
    const SFB: &str = "senftenberg";

    fn winter() -> Semester {
        Semester {
            key: "2026W".into(),
            season: Code::parse("winter"),
            year: 2026,
            label: "WiSe 2026/27".into(),
            starts_on: "2026-10-01".into(),
            ends_on: "2027-03-31".into(),
            is_current: true,
            teaching_events: 0,
            exam_events: 0,
        }
    }

    fn day(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn ids(modules: &[&str]) -> Vec<String> {
        modules.iter().map(|id| id.to_string()).collect()
    }

    /// An exam row as `modules_exams` delivers it (no type, group, rhythm or cancellations), on
    /// one day; an empty time is none. The room is the event's, at its campus.
    fn row(module: &str, event: &str, ord: i64, on: &str, from: &str, to: &str, campus: Option<&str>) -> DateRow {
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
                weekday: Day::parse(on).map(|d| i64::from(d.weekday())),
                start_time: time(from),
                end_time: time(to),
                rhythm: None,
                rhythm_raw: None,
                first_date: Some(on.into()),
                last_date: Some(on.into()),
                room: campus.map(|c| format!("Raum {event} - {c}")),
                campus: campus.map(Code::parse),
                instructor: None,
                comment: None,
                source_url: Some(format!("https://qis.example/{event}")),
            },
        }
    }

    fn in_room(mut row: DateRow, room: &str) -> DateRow {
        row.date.room = Some(room.into());
        row
    }

    fn plan(rows: &[DateRow], modules: &[&str], selection: &Selection) -> Vec<Exam> {
        exams_of(rows, Some(&winter()), &ids(modules), selection, None, &BTreeSet::new())
    }

    type Warned = (Vec<ExamWarning>, Vec<(Day, Vec<String>)>);

    fn warn(rows: &[DateRow], modules: &[&str]) -> Warned {
        let exams = plan(rows, modules, &Selection::default());
        let result = exam_warnings(&exams, &ids(modules));
        invariants(&exams, &ids(modules), &result);
        result
    }

    fn shape(date: DateRow) -> ExamShape {
        shape_of(&exam_reading::read(&date.date, Some(&winter())))
    }

    #[test]
    fn shapes_of_what_a_page_shows() {
        let sitting = row("12104", "148689", 1, "2027-03-12", "11:00", "13:00", Some(ZC));
        assert_eq!(shape(sitting), ExamShape::Sitting { day: day("2027-03-12"), from: 660, to: 780 });
        // Odd times are shown as stated, so they are a sitting as stated.
        let early = row("1", "1", 1, "2027-02-15", "05:00", "07:00", None);
        assert_eq!(shape(early), ExamShape::Sitting { day: day("2027-02-15"), from: 300, to: 420 });
        let deadline = row("1", "1", 1, "2027-02-14", "23:45", "24:00", None);
        assert_eq!(shape(deadline), ExamShape::Deadline { day: day("2027-02-14") });

        let mut window = row("1", "1", 1, "2027-02-08", "", "", None);
        window.date.last_date = Some("2027-02-19".into());
        assert_eq!(shape(window), ExamShape::Window { first: day("2027-02-08"), last: day("2027-02-19") });
        let untimed = row("1", "1", 1, "2027-02-17", "", "", None);
        assert_eq!(shape(untimed), ExamShape::DayOnly { day: day("2027-02-17") });
        let start_only = row("1", "1", 1, "2027-02-17", "10:00", "", None);
        assert_eq!(shape(start_only), ExamShape::DayOnly { day: day("2027-02-17") });
        let reversed = row("1", "1", 1, "2027-02-17", "13:00", "11:00", None);
        assert_eq!(shape(reversed), ExamShape::DayOnly { day: day("2027-02-17") });

        // QIS's placeholder: on a date from another year it is open (148663), inside the semester
        // it keeps its day but no time; a block keeps its days.
        let mut placeholder = row("11103", "148663", 1, "2015-12-27", "01:00", "02:30", None);
        placeholder.date.weekday = Some(7);
        assert_eq!(shape(placeholder.clone()), ExamShape::Open);
        placeholder.date.first_date = Some("2027-02-14".into());
        placeholder.date.last_date = Some("2027-02-14".into());
        assert_eq!(shape(placeholder.clone()), ExamShape::DayOnly { day: day("2027-02-14") });
        placeholder.date.last_date = Some("2027-02-19".into());
        assert_eq!(shape(placeholder), ExamShape::Window { first: day("2027-02-14"), last: day("2027-02-19") });

        let mut undated = row("1", "151016", 1, "2027-02-17", "", "", None);
        (undated.date.first_date, undated.date.last_date, undated.date.weekday) = (None, None, None);
        assert_eq!(shape(undated.clone()), ExamShape::Open);
        undated.date.first_date = Some("demnächst".into());
        assert_eq!(shape(undated), ExamShape::Open);
    }

    #[test]
    fn retakes_by_their_titles() {
        for title in [
            "Nach-/Wiederholungsprüfung schriftliche Ausarbeitungen",
            "Betriebssysteme I / Wiederholungsprüfung",
            "Statistik(Service)/Statistik für Anwender - Wiederholung",
            "Nachklausur  Analysis",
            "NACHPRÜFUNG Physik",
            // The retakes of the health and teacher programs, and English and short ones (2026W).
            "BP36c Statistik in den Gesundheits- und Sozialberufen - 13199 (WMAP) - Wahlpflicht",
            "BT11 Physiotherapie im orthopädischen Handlungsfeld -12112 (WMAP)",
            "PW1 Steuerung und Gestaltung von Pflegeprozessen (14695) WMAP",
            "BP15 Praxismodul Pflege III - 12061 (WMAP+V)",
            "Power Plant Technology 1 - only for retake exams",
            "Wdh. Klimaschutzrecht und das Recht der Energiewende",
            "Wdh.Prüfung Unternehmensnachfolge",
        ] {
            assert!(retake(title), "{title}");
        }
        for title in [
            "Analysis I",
            "Nachhaltigkeit im Bauwesen",
            "Nach Absprache",
            "Entwicklung von Softwaresystemen",
            "BT27 Gesundheits-/ Sozial- und Berufspolitik, Recht - 12128 (MAP) BAP SP (FG 1,3)",
            // A first attempt beside the retake, or either of them.
            "BP7 Pflegephänomene im Kontext von Stoffwechselprozessen - Ausscheidung - 12053 (MAP+V) und WMAP",
            "6. Schulpraktische Studien (SPS) / Praktisches Studiensemester - 11670 (MAP / WMAP)",
            "1.1/2.1/3.1 Pflegewissenschaft und Pflegeforschung I - 11652 (WMAP) / Therapiewissenschaft und \
             Therapieforschung I - 12296 (W-/MAP)",
            "Bildungswissenschaften II (14007) W-/MAP",
        ] {
            assert!(!retake(title), "{title}");
        }
    }

    #[test]
    fn exams_are_the_planned_modules_events_once() {
        let mut rows = Vec::new();
        for module in ["A", "B", "C"] {
            rows.push(row(module, "200", 1, "2027-02-11", "14:00", "15:30", Some(ZC)));
            rows.push(row(module, "200", 2, "2027-03-11", "14:00", "15:30", Some(ZC)));
        }
        rows.push(row("C", "300", 1, "2027-02-12", "09:00", "11:00", Some(ZC)));
        rows.push(row("B", "100", 1, "2027-02-15", "09:00", "11:00", Some(ZC)));
        let exams = plan(&rows, &["B", "A"], &Selection::default());
        let events: Vec<(&str, Vec<String>)> = exams.iter().map(|e| (e.event_id.as_str(), e.modules.clone())).collect();
        // Event 300 belongs only to C, which is not planned; 100 sorts before 200 by its title.
        assert_eq!(events, [("100", ids(&["B"])), ("200", ids(&["B", "A"]))]);
        let shared = &exams[1];
        assert_eq!(shared.rows.iter().map(|r| r.ord).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(shared.title, "Prüfung 200");
        assert_eq!(shared.source_url.as_deref(), Some("https://qis.example/200"));
        assert!(!shared.retake);
        assert_eq!(shared.rows[0].key, RowKey::of(&rows[0].date));
        assert!(exams.iter().all(|e| e.hidden.is_none()));
        assert!(plan(&rows, &[], &Selection::default()).is_empty());
    }

    #[test]
    fn ranks_follow_each_modules_earliest_termin() {
        let mut retake_row = row("M", "900", 1, "2027-03-20", "09:00", "11:00", Some(ZC));
        retake_row.date.event_title = "Nach-/Wiederholungsprüfung schriftliche Ausarbeitungen".into();
        let mut shared_retake = retake_row.clone();
        shared_retake.module_id = "N".into();
        let mut window = row("M", "500", 1, "2027-02-08", "", "", None);
        window.date.last_date = Some("2027-02-19".into());
        let rows = [
            row("M", "148005", 1, "2027-02-11", "14:00", "15:30", Some(ZC)),
            row("M", "148005", 2, "2027-03-11", "14:00", "15:30", Some(ZC)),
            retake_row,
            shared_retake,
            window,
        ];
        let rank_of = |exams: &[Exam], event: &str, ord: i64| -> u8 {
            let exam = exams.iter().find(|e| e.event_id == event).unwrap();
            exam.rows.iter().find(|r| r.ord == Some(ord)).unwrap().rank
        };
        let exams = plan(&rows, &["M", "N"], &Selection::default());
        assert_eq!((rank_of(&exams, "148005", 1), rank_of(&exams, "148005", 2)), (1, 2));
        assert_eq!(rank_of(&exams, "500", 1), 0, "a window has no rank");
        // The retake is M's third Termin but N's only one: rank 1 for N's sake.
        assert_eq!(rank_of(&exams, "900", 1), 1);
        let retake_exam = exams.iter().find(|e| e.event_id == "900").unwrap();
        assert!(retake_exam.retake);
        assert_eq!(retake_exam.modules, ids(&["M", "N"]));
        // Without N, it is a later Termin of M.
        let exams = plan(&rows, &["M"], &Selection::default());
        assert_eq!(rank_of(&exams, "900", 1), 2);

        // Hiding the first sitting makes the second one M's first.
        let first = RowKey::of(&rows[0].date).unwrap();
        let selection = Selection { hidden_rows: BTreeSet::from([first]), ..Selection::default() };
        let exams = plan(&rows, &["M"], &selection);
        let e148005 = exams.iter().find(|e| e.event_id == "148005").unwrap();
        let rows: Vec<(Option<HiddenBy>, u8)> = e148005.rows.iter().map(|r| (r.hidden, r.rank)).collect();
        assert_eq!(rows, [(Some(HiddenBy::Row), 0), (None, 1)]);
    }

    #[test]
    fn the_sittings_of_one_day_are_one_attempt() {
        // Two rows of one key with two ends (150907), two parts of one exam (12128's 150494), the
        // two towns' exams of a track module (11107), and a second attempt on a later day.
        let rows = [
            row("P", "150907", 1, "2027-03-12", "09:00", "11:00", Some(SFB)),
            row("P", "150907", 2, "2027-03-12", "09:00", "11:30", Some(SFB)),
            row("P", "150907", 3, "2027-03-26", "09:00", "11:00", Some(SFB)),
            row("Q", "150494", 1, "2027-02-04", "08:00", "08:45", Some(SFB)),
            row("Q", "150494", 2, "2027-02-04", "09:15", "12:15", Some(SFB)),
            row("T", "149694", 1, "2027-02-08", "11:00", "13:00", Some(SFB)),
            row("T", "149541", 1, "2027-02-08", "14:00", "16:00", Some(ZC)),
        ];
        assert_eq!(RowKey::of(&rows[0].date), RowKey::of(&rows[1].date));
        let modules = ["P", "Q", "T"];
        let exams = plan(&rows, &modules, &Selection::default());
        let ranks: Vec<(&str, Vec<u8>)> =
            exams.iter().map(|e| (e.event_id.as_str(), e.rows.iter().map(|r| r.rank).collect())).collect();
        let expected: [(&str, Vec<u8>); 4] =
            [("150907", vec![1, 1, 2]), ("150494", vec![1, 1]), ("149541", vec![1]), ("149694", vec![1])];
        assert_eq!(ranks, expected);

        // Rows of one key are one Termin until the later end, as the feed merges them; so a
        // sitting at 11:15 in the same hall overlaps it, and only the second attempt avoids that.
        let found = termine(&exams, &ids(&modules));
        let p: Vec<(Day, u16, u16)> = found[0].1.iter().map(|t| (t.day, t.termin.from, t.termin.to)).collect();
        assert_eq!(p, [(day("2027-03-12"), 540, 690), (day("2027-03-26"), 540, 660)]);
        let mut next = rows.to_vec();
        next.push(row("R", "1", 1, "2027-03-12", "11:15", "12:00", Some(SFB)));
        let (warnings, _) = warn(&next, &["P", "R"]);
        assert_eq!(warnings.len(), 1);
        let w = &warnings[0];
        assert_eq!((w.kind.clone(), w.hard, w.avoid), (WarningKind::Overlap, false, Some(day("2027-03-26"))));
    }

    #[test]
    fn exams_are_hidden_by_the_first_rule_that_holds() {
        let rows = [
            row("T", "700", 1, "2027-02-10", "09:00", "11:00", Some(SFB)),
            row("T", "701", 1, "2027-02-10", "09:00", "11:00", Some(ZC)),
            row("T", "702", 1, "2027-02-11", "09:00", "11:00", None),
            row("T", "703", 1, "2027-02-12", "09:00", "11:00", Some(SFB)),
            row("T", "703", 2, "2027-03-12", "09:00", "11:00", Some(ZC)),
            row("T", "704", 1, "2027-02-15", "09:00", "11:00", Some(SFB)),
            row("U", "704", 1, "2027-02-15", "09:00", "11:00", Some(SFB)),
        ];
        let modules = ids(&["T", "U"]);
        let tracks = BTreeSet::from(["T".to_string()]);
        let hidden = |selection: &Selection, town: Option<Town>| -> Vec<(String, Option<HiddenBy>)> {
            exams_of(&rows, Some(&winter()), &modules, selection, town, &tracks)
                .into_iter()
                .map(|e| (e.event_id, e.hidden))
                .collect()
        };
        let none = Selection::default();
        // T has tracks and is taken in Cottbus: its Senftenberg exam goes. An exam of mixed or
        // unknown town stays, and so does one U (no tracks) shares.
        assert_eq!(
            hidden(&none, Some(Town::Cottbus)),
            [
                ("700".to_string(), Some(HiddenBy::Town(Town::Cottbus))),
                ("701".to_string(), None),
                ("702".to_string(), None),
                ("703".to_string(), None),
                ("704".to_string(), None)
            ]
        );
        assert_eq!(hidden(&none, Some(Town::Senftenberg))[1], ("701".to_string(), Some(HiddenBy::Town(Town::Senftenberg))));
        assert!(hidden(&none, None).iter().all(|(_, h)| h.is_none()), "no town, no track rule");

        let kinds = Selection { hidden_kinds: KindSet::default().with(EventKind::Exam), ..Selection::default() };
        assert!(hidden(&kinds, Some(Town::Cottbus)).iter().all(|(_, h)| *h == Some(HiddenBy::Kinds)));
        let teaching = Selection { hidden_kinds: KindSet::default().with(EventKind::Exercise), ..Selection::default() };
        assert!(hidden(&teaching, None).iter().all(|(_, h)| h.is_none()), "other kinds leave exams alone");
        let events = Selection { hidden_events: BTreeSet::from([700, 702]), ..kinds.clone() };
        let by_event = hidden(&events, Some(Town::Cottbus));
        assert_eq!(by_event[0].1, Some(HiddenBy::Event));
        assert_eq!(by_event[1].1, Some(HiddenBy::Kinds));
        assert_eq!(by_event[2].1, Some(HiddenBy::Event));

        // A hidden exam hides every row with its reason; a hidden key only its row.
        let key = RowKey::of(&rows[4].date).unwrap();
        let selection = Selection { hidden_rows: BTreeSet::from([key]), hidden_events: BTreeSet::from([700]), ..none };
        let exams = exams_of(&rows, Some(&winter()), &modules, &selection, None, &tracks);
        assert_eq!(exams[0].rows[0].hidden, Some(HiddenBy::Event));
        let e703: Vec<Option<HiddenBy>> = exams[3].rows.iter().map(|r| r.hidden).collect();
        assert_eq!(e703, [None, Some(HiddenBy::Row)]);
        assert_eq!(exams[3].hidden, None);
    }

    #[test]
    fn two_rooms_at_one_time_are_one_termin() {
        let rows = [
            in_room(row("12104", "148689", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), "Audimax 1"),
            in_room(row("12104", "150664", 1, "2027-03-12", "11:00", "13:00", Some(SFB)), "6.210"),
            in_room(row("12104", "150665", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), "Audimax 1"),
            in_room(row("12204", "148696", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), "Seminarraum 2"),
            in_room(row("12204", "148696", 2, "2027-03-26", "11:00", "13:00", Some(ZC)), "Seminarraum 2"),
        ];
        let modules = ids(&["12104", "12204"]);
        let exams = exams_of(&rows, Some(&winter()), &modules, &Selection::default(), None, &BTreeSet::new());
        let found = termine(&exams, &modules);
        assert_eq!(found[0].0, "12104");
        assert_eq!(found[0].1.len(), 1);
        let TerminAt { day: on, termin, events, unplaced } = &found[0].1[0];
        assert_eq!(*on, day("2027-03-12"));
        assert_eq!((termin.event_id.as_str(), termin.from, termin.to), ("148689", 660, 780));
        assert_eq!(termin.campus, [Code::parse(ZC), Code::parse(SFB)]);
        assert_eq!(termin.room, None, "two rooms");
        assert_eq!(*events, ["148689", "150664", "150665"]);
        assert!(!unplaced);
        let second: Vec<Day> = found[1].1.iter().map(|t| t.day).collect();
        assert_eq!(second, [day("2027-03-12"), day("2027-03-26")]);
        assert_eq!(found[1].1[0].termin.room.as_deref(), Some("Seminarraum 2"));
    }

    #[test]
    fn an_overlap_is_hard_when_no_termin_avoids_it() {
        // Mathematik W-1 and ERP on Mo 08.02.2027, both at 11:00.
        let rows = [
            row("11109", "148084", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("11152", "148616", 1, "2027-02-08", "11:00", "12:30", Some(ZC)),
        ];
        let (warnings, unknown) = warn(&rows, &["11109", "11152"]);
        assert_eq!(warnings.len(), 1);
        let w = &warnings[0];
        assert_eq!((w.kind.clone(), w.day, w.hard, w.avoid), (WarningKind::Overlap, day("2027-02-08"), true, None));
        assert_eq!((w.a.module_id.as_str(), w.b.module_id.as_str()), ("11109", "11152"), "equal start: plan order");
        assert!(unknown.is_empty());
        // Planned the other way round, the other one comes first.
        let (warnings, _) = warn(&rows, &["11152", "11109"]);
        assert_eq!(warnings[0].a.module_id, "11152");
        // One that starts inside the other one's time.
        let rows = [
            row("A", "1", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("B", "2", 1, "2027-02-08", "09:00", "11:30", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!((warnings[0].a.module_id.as_str(), warnings[0].kind.clone()), ("B", WarningKind::Overlap));
    }

    #[test]
    fn an_avoidable_pair_is_soft_and_names_the_day_that_avoids_it() {
        let rows = [
            row("A", "1", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("B", "2", 1, "2027-02-08", "12:00", "14:00", Some(ZC)),
            row("B", "2", 2, "2027-03-11", "12:00", "14:00", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!(warnings.len(), 1);
        let w = &warnings[0];
        let expected = (WarningKind::Overlap, day("2027-02-08"), false, Some(day("2027-03-11")));
        assert_eq!((w.kind.clone(), w.day, w.hard, w.avoid), expected);
        assert_eq!((w.a.from, w.b.from), (660, 720));

        // Both modules have several Termine and collide twice: one warning, the earliest issue,
        // and a day that avoids it by changing one module's Termin only.
        let rows = [
            row("A", "1", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("A", "1", 2, "2027-03-10", "11:00", "13:00", Some(ZC)),
            row("B", "2", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("B", "2", 2, "2027-03-10", "12:00", "14:00", Some(ZC)),
            row("B", "3", 1, "2027-03-12", "12:00", "14:00", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!(warnings.len(), 1);
        assert_eq!((warnings[0].day, warnings[0].hard, warnings[0].avoid), (day("2027-02-08"), false, Some(day("2027-03-10"))));

        // A pair that collides only on A's second Termin: A's first fits.
        let rows = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", Some(ZC)),
            row("A", "1", 2, "2027-03-10", "11:00", "13:00", Some(ZC)),
            row("B", "2", 1, "2027-03-10", "12:00", "14:00", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!((warnings[0].day, warnings[0].hard, warnings[0].avoid), (day("2027-03-10"), false, Some(day("2027-02-08"))));
    }

    #[test]
    fn short_hops_between_towns_and_sites() {
        let tight = |first: (&str, &str, &str), second: (&str, &str, &str)| -> Vec<WarningKind> {
            let rows = [
                row("A", "1", 1, "2027-02-08", first.0, first.1, Some(first.2)),
                row("B", "2", 1, "2027-02-08", second.0, second.1, Some(second.2)),
            ];
            warn(&rows, &["A", "B"]).0.into_iter().map(|w| w.kind).collect()
        };
        let hop = |gap: u16, from: &str, to: &str| WarningKind::Tight { gap, from: Code::parse(from), to: Code::parse(to) };
        // Cottbus ↔ Senftenberg: 149 minutes are too few, 150 are enough; either way round.
        assert_eq!(tight(("08:00", "10:00", ZC), ("12:29", "14:00", SFB)), [hop(149, ZC, SFB)]);
        assert_eq!(tight(("08:00", "10:00", ZC), ("12:30", "14:00", SFB)), []);
        assert_eq!(tight(("14:00", "16:00", ZC), ("10:00", "12:00", SFB)), [hop(120, SFB, ZC)]);
        assert_eq!(tight(("08:00", "10:00", ZC), ("10:00", "12:00", SFB)), [hop(0, ZC, SFB)]);
        // Between Cottbus sites: 44 too few, 45 enough.
        assert_eq!(tight(("09:00", "11:00", ZC), ("11:44", "13:00", SD)), [hop(44, ZC, SD)]);
        assert_eq!(tight(("09:00", "11:00", ZC), ("11:45", "13:00", SD)), []);
        assert_eq!(tight(("12:15", "13:45", SD), ("14:00", "17:00", ZC)), [hop(15, SD, ZC)]);
        // One campus: no hop at all.
        assert_eq!(tight(("08:00", "10:00", ZC), ("10:00", "12:00", ZC)), []);
        assert_eq!(tight(("08:00", "10:00", SFB), ("10:00", "12:00", SFB)), []);
        // Other days never warn.
        let rows = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", Some(ZC)),
            row("B", "2", 1, "2027-02-09", "08:00", "10:00", Some(SFB)),
        ];
        assert_eq!(warn(&rows, &["A", "B"]), (vec![], vec![]));
    }

    #[test]
    fn with_several_campuses_every_pairing_must_be_far() {
        // A sits at Zentralcampus and in Senftenberg at once (12104's two events); B at
        // Zentralcampus an hour later: the Senftenberg room is too far.
        let rows = [
            row("A", "1", 1, "2027-03-12", "09:00", "11:00", Some(ZC)),
            row("A", "2", 1, "2027-03-12", "09:00", "11:00", Some(SFB)),
            row("B", "3", 1, "2027-03-12", "12:00", "13:00", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!(warnings[0].kind, WarningKind::Tight { gap: 60, from: Code::parse(SFB), to: Code::parse(ZC) });
        assert_eq!(warnings[0].a.campus, [Code::parse(ZC), Code::parse(SFB)]);
        // A hop between the towns is named before one between sites.
        let rows = [
            row("A", "1", 1, "2027-03-12", "09:00", "11:00", Some(SD)),
            row("A", "2", 1, "2027-03-12", "09:00", "11:00", Some(SFB)),
            row("B", "3", 1, "2027-03-12", "11:30", "13:00", Some(ZC)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!(warnings[0].kind, WarningKind::Tight { gap: 30, from: Code::parse(SFB), to: Code::parse(ZC) });
    }

    #[test]
    fn an_unknown_campus_is_only_a_line() {
        let rows = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", None),
            row("B", "2", 1, "2027-02-08", "11:00", "12:00", Some(ZC)),
            row("C", "3", 1, "2027-02-08", "12:29", "13:00", Some("mars")),
            row("D", "4", 1, "2027-02-09", "08:00", "10:00", None),
            row("E", "5", 1, "2027-02-09", "12:30", "14:00", Some(SFB)),
        ];
        let (warnings, unknown) = warn(&rows, &["A", "B", "C", "D", "E"]);
        assert_eq!(warnings, []);
        // A–B (60 minutes), A–C and B–C (a campus in no known town); D–E are 150 minutes apart.
        assert_eq!(unknown, [(day("2027-02-08"), ids(&["A", "B", "C"]))]);
        // An overlap needs no campus.
        let rows = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", None),
            row("B", "2", 1, "2027-02-08", "09:00", "12:00", None),
        ];
        let (warnings, unknown) = warn(&rows, &["A", "B"]);
        assert_eq!((warnings[0].kind.clone(), warnings[0].hard), (WarningKind::Overlap, true));
        assert_eq!(unknown, []);
        // A known hop that is too short warns, even when another room of the Termin is unknown.
        let rows = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", Some(ZC)),
            row("A", "2", 1, "2027-02-08", "08:00", "10:00", None),
            row("B", "3", 1, "2027-02-08", "10:30", "12:00", Some(SFB)),
        ];
        let (warnings, _) = warn(&rows, &["A", "B"]);
        assert_eq!(warnings[0].kind, WarningKind::Tight { gap: 30, from: Code::parse(ZC), to: Code::parse(SFB) });
    }

    #[test]
    fn shared_exams_never_warn_and_shared_rooms_do() {
        // One event of two modules is one exam.
        let shared = [
            row("A", "1", 1, "2027-02-08", "08:00", "10:00", Some(ZC)),
            row("B", "1", 1, "2027-02-08", "08:00", "10:00", Some(ZC)),
        ];
        assert_eq!(warn(&shared, &["A", "B"]), (vec![], vec![]));
        // Two events at one time in one hall are two exams („Entwicklung von Softwaresystemen"
        // and „Softwaresystemtechnik" in Audimax 1).
        let room = "Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus";
        let hall = [
            in_room(row("A", "1", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), room),
            in_room(row("B", "2", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), room),
        ];
        let (warnings, unknown) = warn(&hall, &["A", "B"]);
        assert_eq!(warnings.len(), 1);
        assert_eq!((warnings[0].kind.clone(), warnings[0].hard), (WarningKind::Overlap, true));
        assert_eq!((warnings[0].a.room.as_deref(), warnings[0].b.room.as_deref()), (Some(room), Some(room)));
        assert_eq!(unknown, []);
        // A Termin of B that shares A's event is still one exam with it, whatever else it holds.
        let mut both = hall.to_vec();
        both.push(in_room(row("B", "1", 1, "2027-03-12", "11:00", "13:00", Some(ZC)), room));
        assert_eq!(warn(&both, &["A", "B"]), (vec![], vec![]));
    }

    #[test]
    fn the_finder_asks_one_pair_of_termine_at_a_time() {
        // A candidate C with two Termine, each colliding with another fixed Termin of the plan:
        // no pair of modules is hard, yet no Termin of C avoids both.
        let rows = [
            row("P", "1", 1, "2027-02-08", "09:00", "11:00", Some(ZC)),
            row("Q", "2", 1, "2027-03-10", "09:00", "11:00", Some(ZC)),
            row("C", "3", 1, "2027-02-08", "10:00", "12:00", Some(ZC)),
            row("C", "3", 2, "2027-03-10", "11:30", "13:00", Some(SFB)),
            row("S", "4", 1, "2027-02-08", "13:00", "14:00", None),
            row("U", "3", 1, "2027-02-08", "10:00", "12:00", Some(ZC)),
        ];
        let modules = ["P", "Q", "C", "S", "U"];
        let (warnings, _) = warn(&rows, &["P", "Q", "C"]);
        assert!(warnings.iter().all(|w| !w.hard), "{warnings:?}");
        let exams = plan(&rows, &modules, &Selection::default());
        let found = termine(&exams, &ids(&modules));
        let at = |module: usize, index: usize| &found[module].1[index];
        let (p, q, c1, c2, s, u) = (at(0, 0), at(1, 0), at(2, 0), at(2, 1), at(3, 0), at(4, 0));
        assert_eq!(collision(c1, p), Some(WarningKind::Overlap));
        assert_eq!(collision(p, c1), Some(WarningKind::Overlap), "either way round");
        assert_eq!(collision(c1, q), None, "another day");
        assert_eq!(collision(c2, p), None);
        let hop = WarningKind::Tight { gap: 30, from: Code::parse(ZC), to: Code::parse(SFB) };
        assert_eq!(collision(q, c2), Some(hop.clone()));
        assert_eq!(collision(c2, q), Some(hop));
        assert!([c1, c2].iter().all(|c| [p, q].iter().any(|fixed| collision(c, fixed).is_some())));
        // An unknown campus is no conflict, and a shared exam event is one exam.
        assert_eq!((collision(s, c1), collision(s, p)), (None, None));
        assert!(s.unplaced);
        assert_eq!(collision(u, c1), None);
    }

    #[test]
    fn only_visible_sittings_warn() {
        let mut placeholder = row("A", "1", 1, "2027-02-14", "01:00", "02:30", Some(ZC));
        placeholder.date.weekday = Some(7);
        let mut window = row("B", "2", 1, "2027-02-08", "", "", Some(ZC));
        window.date.last_date = Some("2027-02-19".into());
        let rows = [
            placeholder,
            window,
            row("C", "3", 1, "2027-02-14", "23:45", "24:00", Some(SFB)),
            row("D", "4", 1, "2027-02-14", "", "", Some(SFB)),
            row("E", "5", 1, "2027-02-14", "01:00", "03:00", Some(ZC)),
            row("E", "5", 2, "2027-02-10", "22:00", "23:50", Some(SFB)),
        ];
        let modules = ["A", "B", "C", "D", "E"];
        let exams = plan(&rows, &modules, &Selection::default());
        let shapes: Vec<&ExamShape> = exams.iter().flat_map(|e| e.rows.iter().map(|r| &r.shape)).collect();
        assert!(
            matches!(
                shapes.as_slice(),
                [
                    ExamShape::DayOnly { .. },
                    ExamShape::Window { .. },
                    ExamShape::Deadline { .. },
                    ExamShape::DayOnly { .. },
                    ExamShape::Sitting { .. },
                    ExamShape::Sitting { .. }
                ]
            ),
            "{shapes:?}"
        );
        // E's sittings, the only ones, meet no other sitting.
        assert_eq!(warn(&rows, &modules), (vec![], vec![]));

        // Hiding the kind „Prüfung" silences everything; hiding one Termin chooses the other.
        let rows = [
            row("A", "1", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("B", "2", 1, "2027-02-08", "11:00", "13:00", Some(ZC)),
            row("B", "2", 2, "2027-03-11", "11:00", "13:00", Some(ZC)),
        ];
        let modules = ids(&["A", "B"]);
        assert_eq!(warn(&rows, &["A", "B"]).0.len(), 1);
        let quiet = Selection { hidden_kinds: KindSet::default().with(EventKind::Exam), ..Selection::default() };
        let exams = exams_of(&rows, None, &modules, &quiet, None, &BTreeSet::new());
        assert_eq!(exam_warnings(&exams, &modules), (vec![], vec![]));
        let first = RowKey::of(&rows[1].date).unwrap();
        let chosen = Selection { hidden_rows: BTreeSet::from([first]), ..Selection::default() };
        let exams = exams_of(&rows, None, &modules, &chosen, None, &BTreeSet::new());
        assert_eq!(exam_warnings(&exams, &modules), (vec![], vec![]));
    }

    /// What holds for any plan: modules and rows once, a rank exactly for visible sittings (1 on
    /// the day of a module's first Termin), one rank per row key, Termine in order, and warnings
    /// between two planned modules.
    fn invariants(exams: &[Exam], modules: &[String], (warnings, unknown): &Warned) {
        let position = |id: &str| modules.iter().position(|m| m == id).unwrap_or_else(|| panic!("{id} is not planned"));
        let found = termine(exams, modules);
        let firsts: BTreeMap<&str, Day> =
            found.iter().filter_map(|(module, termine)| termine.first().map(|t| (module.as_str(), t.day))).collect();
        for (module, termine) in &found {
            let at = |t: &TerminAt| (t.day, t.termin.from, t.termin.to);
            assert!(termine.windows(2).all(|w| at(&w[0]) < at(&w[1])), "{module}");
            assert!(termine.iter().all(|t| t.termin.module_id == *module && t.events.contains(&t.termin.event_id)));
        }
        let mut ranks: BTreeMap<RowKey, u8> = BTreeMap::new();
        for exam in exams {
            assert!(!exam.modules.is_empty(), "{}", exam.event_id);
            let positions: Vec<usize> = exam.modules.iter().map(|m| position(m)).collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{}: plan order, each once", exam.event_id);
            let ords: Vec<Option<i64>> = exam.rows.iter().map(|r| r.ord).collect();
            assert!(ords.windows(2).all(|w| w[0] < w[1]), "{}: rows once, by ord", exam.event_id);
            for row in &exam.rows {
                assert!(exam.hidden.is_none() || row.hidden == exam.hidden);
                match (&row.shape, row.hidden) {
                    (ExamShape::Sitting { day, from, to }, None) => {
                        assert!(from <= to);
                        let first = exam.modules.iter().any(|m| firsts.get(m.as_str()) == Some(day));
                        assert_eq!(row.rank, if first { 1 } else { 2 }, "{}/{:?}", exam.event_id, row.ord);
                        if let Some(key) = row.key {
                            let rank = *ranks.entry(key).or_insert(row.rank);
                            assert_eq!(row.rank, rank, "{}: one rank per key", key.text());
                        }
                    }
                    _ => assert_eq!(row.rank, 0, "{}/{:?}", exam.event_id, row.ord),
                }
            }
        }
        for w in warnings {
            let (a, b) = (&w.a, &w.b);
            assert_ne!(position(&a.module_id), position(&b.module_id));
            assert!(a.from <= b.from, "{w:?}");
            assert_eq!(w.hard, w.avoid.is_none(), "{w:?}");
            match &w.kind {
                WarningKind::Overlap => assert!(a.from == b.from || (a.from < b.to && b.from < a.to), "{w:?}"),
                WarningKind::Tight { gap, .. } => assert!(*gap < CITY_GAP && a.to + gap == b.from, "{w:?}"),
            }
        }
        assert!(warnings.windows(2).all(|w| w[0].day <= w[1].day));
        assert!(unknown.windows(2).all(|w| w[0].0 < w[1].0));
        for (_, ids) in unknown {
            let positions: Vec<usize> = ids.iter().map(|m| position(m)).collect();
            assert!(positions.len() >= 2 && positions.windows(2).all(|w| w[0] < w[1]));
        }
    }

    /// A plan of these modules in one semester, fed as the Studienplan feeds it: its exam rows,
    /// an empty selection, no town, no tracks.
    fn planned(db: &dyn Database, semester: &Semester, modules: &[String]) -> (Vec<Exam>, Warned) {
        let rows = queries::modules_exams(db, modules, &semester.key).unwrap();
        let exams = exams_of(&rows, Some(semester), modules, &Selection::default(), None, &BTreeSet::new());
        let warned = exam_warnings(&exams, modules);
        invariants(&exams, modules, &warned);
        (exams, warned)
    }

    /// The pairs quoted in the design, on the pinned snapshot; on any snapshot, the invariants of a
    /// plan of the first sixty modules with a dated exam in the current semester.
    #[test]
    fn exam_warnings_on_real_pairs() {
        let pinned = crate::tests::studyplan_db("exam_warnings_on_real_pairs");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let semesters = queries::semesters(&db).unwrap();
        let current = queries::meta(&db).unwrap().current_semester.unwrap();
        let semester = semesters.iter().find(|s| s.key == current).unwrap();
        let mut many: Vec<String> = Vec::new();
        for row in queries::semester_exams(&db, &current).unwrap() {
            if many.len() < MAX_MODULES && !many.contains(&row.module_id) {
                many.push(row.module_id);
            }
        }
        assert!(!many.is_empty());
        let (exams, _) = planned(&db, semester, &many);
        assert!(exams.iter().any(|e| e.rows.iter().any(|r| r.rank == 1)));
        if !is_pinned {
            return;
        }

        let winter = semesters.iter().find(|s| s.key == "2026W").unwrap();
        let pair = |modules: &[&str]| -> ExamWarning {
            let (_, (warnings, unknown)) = planned(&db, winter, &ids(modules));
            assert_eq!(warnings.len(), 1, "{modules:?}: {warnings:?}");
            assert_eq!(unknown, [], "{modules:?}");
            warnings[0].clone()
        };
        let hop = |gap: u16, from: Campus, to: Campus| WarningKind::Tight { gap, from: Code::Known(from), to: Code::Known(to) };

        let w = pair(&["11109", "11152"]);
        assert_eq!((w.kind, w.day, w.hard), (WarningKind::Overlap, day("2027-02-08"), true));
        for modules in [["35320", "12737"], ["11675", "12175"]] {
            let w = pair(&modules);
            let expected = (hop(0, Campus::Zentralcampus, Campus::Senftenberg), day("2027-02-08"), true);
            assert_eq!((w.kind, w.day, w.hard), expected, "{modules:?}");
            assert_eq!([w.a.module_id.as_str(), w.b.module_id.as_str()], modules);
        }
        let w = pair(&["11970", "12737"]);
        assert_eq!((w.kind, w.a.module_id.as_str()), (hop(120, Campus::Senftenberg, Campus::Zentralcampus), "12737"));
        let w = pair(&["23503", "11758"]);
        assert_eq!((w.kind, w.day), (hop(0, Campus::Zentralcampus, Campus::Sachsendorf), day("2027-02-09")));
        let w = pair(&["11595", "13361"]);
        assert_eq!((w.kind, w.day), (hop(15, Campus::Sachsendorf, Campus::Zentralcampus), day("2027-02-15")));

        // 12104 sits at Zentralcampus and in Senftenberg at the same time: one Termin, no warning.
        let alone = ids(&["12104"]);
        let (exams, warned) = planned(&db, winter, &alone);
        assert_eq!(exams.iter().map(|e| e.event_id.as_str()).collect::<Vec<_>>(), ["148689", "150664"]);
        let found = termine(&exams, &alone);
        assert_eq!(found[0].1.len(), 1);
        let campus = [Code::Known(Campus::Zentralcampus), Code::Known(Campus::Senftenberg)];
        assert_eq!(found[0].1[0].termin.campus, campus);
        assert!(exams.iter().all(|e| e.rows.iter().all(|r| r.rank == 1)));
        assert_eq!(warned, (vec![], vec![]));
        // Betriebssysteme I's retake at the same hour.
        let w = pair(&["12104", "12204"]);
        assert_eq!((w.kind, w.day, w.a.from, w.hard), (WarningKind::Overlap, day("2027-03-12"), 660, true));
        let (exams, _) = planned(&db, winter, &ids(&["12204"]));
        assert!(exams.iter().any(|e| e.retake), "{exams:?}");
        // Softwaresystemtechnik in the same hall at the same hour is another exam: three programs
        // hold both modules.
        let w = pair(&["12104", "12209"]);
        assert_eq!((w.kind, w.day, w.a.from, w.hard), (WarningKind::Overlap, day("2027-03-12"), 660, true));
        assert_eq!((w.a.event_id.as_str(), w.b.event_id.as_str()), ("148689", "152385"));
        assert_eq!(w.a.room, None, "two rooms");
        assert_eq!(w.b.room.as_deref(), Some("Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus"));

        // Pharmazeutische Chemie's two rows of one key, 09:00–11:00 and 09:00–11:30 in one room:
        // one rank, one Termin until 11:30.
        let (exams, _) = planned(&db, winter, &ids(&["12749"]));
        let e150907 = exams.iter().find(|e| e.event_id == "150907").unwrap();
        let rows: Vec<(Option<String>, u8)> = e150907.rows.iter().map(|r| (r.key.map(RowKey::text), r.rank)).collect();
        let key = Some("150907-61a1a".to_string());
        assert_eq!(rows, [(key.clone(), 1), (key, 1)]);
        let found = termine(&exams, &ids(&["12749"]));
        let at: Vec<(Day, u16, u16)> = found[0].1.iter().map(|t| (t.day, t.termin.from, t.termin.to)).collect();
        assert_eq!(at, [(day("2027-03-12"), 540, 690)]);
        // No town chosen for a track module: its Senftenberg and its Cottbus exam of one day are
        // both its first Termin.
        let (exams, _) = planned(&db, winter, &ids(&["11107"]));
        let ranks: Vec<(&str, Vec<u8>)> =
            exams.iter().map(|e| (e.event_id.as_str(), e.rows.iter().map(|r| r.rank).collect())).collect();
        let expected: [(&str, Vec<u8>); 2] = [("149694", vec![1]), ("149541", vec![1, 1])];
        assert_eq!(ranks, expected);
        // A Wiederholungs-MAP, the only dated exam of its module.
        let (exams, _) = planned(&db, winter, &ids(&["13199"]));
        let e152826 = exams.iter().find(|e| e.event_id == "152826").unwrap();
        assert!(e152826.retake, "{}", e152826.title);

        // Switching Technologies sits twice: the second is „2. Termin".
        let (exams, _) = planned(&db, winter, &ids(&["11473"]));
        let e148005 = exams.iter().find(|e| e.event_id == "148005").unwrap();
        let ranks: Vec<(ExamShape, u8)> = e148005.rows.iter().map(|r| (r.shape.clone(), r.rank)).collect();
        assert_eq!(
            ranks,
            [
                (ExamShape::Sitting { day: day("2027-02-11"), from: 840, to: 930 }, 1),
                (ExamShape::Sitting { day: day("2027-03-11"), from: 840, to: 930 }, 2)
            ]
        );

        // The „Nach-/Wiederholungsprüfung schriftliche Ausarbeitungen" of 23 modules, without a
        // date yet.
        let (exams, _) = planned(&db, winter, &ids(&["12682", "12684"]));
        let e151016 = exams.iter().find(|e| e.event_id == "151016").unwrap();
        assert!(e151016.retake);
        assert_eq!(e151016.modules, ids(&["12682", "12684"]));
        assert!(e151016.rows.iter().all(|r| r.shape == ExamShape::Open && r.rank == 0));

        // Hiding the kind „Prüfung" silences a real overlap.
        let modules = ids(&["11109", "11152"]);
        let rows = queries::modules_exams(&db, &modules, "2026W").unwrap();
        let quiet = Selection { hidden_kinds: KindSet::default().with(EventKind::Exam), ..Selection::default() };
        let exams = exams_of(&rows, Some(winter), &modules, &quiet, None, &BTreeSet::new());
        assert_eq!(exam_warnings(&exams, &modules), (vec![], vec![]));
    }
}
