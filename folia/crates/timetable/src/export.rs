//! A timetable as calendar entries, and the subscription that describes it.
//!
//! The calendar a visitor downloads and the one a calendar service subscribes to are the same
//! text: the browser makes it from the page's own timetable, the server from the timetable it
//! builds anew for a code, and both call `calendar_of`. So nothing here may depend on what differs
//! between the two: the browser holds the modules in plan order, a code in ascending order.
//! Entries are sorted by their start and UID, and the modules of an entry are listed by id.
//!
//! A Termin is one entry, a series (owner, 2026-10-13: an entry per date was „ziemlich nervig"):
//! its rule (every week or every two to four weeks, every day of a block or its working days)
//! makes its dates from the first to the last, the dates the rule makes and the Termin does
//! not hold (the break, a holiday, a date QIS cancels) are left out, and a date that says more or
//! other than the series, a room note or another end, follows it in full (`ics`). Its UID is its
//! row key's, `148701-a2633@betula.app`, and stays when QIS changes a Termin's end or room, so a
//! subscribed calendar updates the series instead of adding a second. Rows that share a key (the
//! same slot in two rooms, or once more with a later end) are one series, each date from the
//! earliest start to the latest end, in both rooms. Such rows may differ in who teaches, in QIS's
//! comment, in their last date and in what they drop, so a date says what each row held on it
//! says, the range spans all of them, and a date one row drops is named as dropped only when no row
//! of the key holds it. Single dates Radix reads as one series (QIS lists a seminar's Tuesdays one
//! by one, `Row::series`) are one series as well, under the key of the row of its first date; a
//! week it has no date in is left out and named. A single date is an entry of its own, with a UID
//! of its key and its date as an exam date has (`148689-485e5-20270312@betula.app`), and so is
//! every date of a Termin that no rule makes.
//!
//! An entry is as short as a slot of the page's week (owner, 2026-09-25: on a phone a calendar
//! shows little more than the title): „VL EvS" in „ZHG/HS.C", the kinds in their few letters, the
//! modules as the slots call them (their abbreviations within the plan's program, which a code
//! carries) and the rooms in their short forms. Its description says it all in full: what QIS
//! calls the event, the modules with number and title, the rooms as QIS names them, and what the
//! rows say.
//!
//! What a student may not attend is marked tentative: a date in a range QIS does not state, an
//! option of a choice not made yet, an exam's second sitting and a retake. What is hidden, a row
//! without a clear time and a row that names no date are left out.
//!
//! A subscription's code carries the semester, the planned modules and what is hidden. It keeps
//! only what the loaded timetable has, so hide rules of Termine QIS has since removed do not make
//! the code longer, and such a removal does not make a subscription stale either.
//!
//! A calendar is written in one language (`Locale`): the page's for the download, the address's
//! for the feed (`/calendar/<code>.ics` German, `/en/calendar/<code>.ics` English). The language
//! decides the words around the data (`i18n::timetable`), the kinds, campuses, holidays and dates;
//! what QIS says (the type of an event, titles, rooms, people, its comments and the reasons it
//! gives) stays as QIS says it. UIDs do not depend on the language, so a subscription that changes
//! its language updates its entries.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::Day;
use folia_calendar::kind::{EventKind, KindSet};
use folia_calendar::rowkey::{fingerprint, RowKey};
use folia_calendar::select::{Selection, TownChoice};
use folia_calendar::semester::SemesterKey;
use folia_calendar::subscription::{date_of, date_value, Subscription};
use folia_locale::Locale;
use folia_model::ids::is_program_id;
use folia_model::labels::{Code, Rhythm};
use folia_model::rows::Meta;
use folia_model::rows_detail::EventDate;

use crate::exam_reading::Reason;
use crate::exams::{Exam, ExamRow, ExamShape};
use crate::i18n::Texts;
use crate::ics::{self, Calendar, Entry, Repeat, Rule, When};
use crate::model::{Event, Row, Timetable};
use crate::occur::{Every, Skipped};
use crate::views::{kind_and_title, kind_short, short_title, type_text};

/// The domain of every UID. The same for the download and the feed, so a calendar that holds both
/// holds each date once, and fixed: another domain would make every subscribed entry a new one.
pub const UID_DOMAIN: &str = "betula.app";

/// The most VEVENTs of one calendar, a series and each of its dates that differ counted, the
/// earliest entries kept. A full semester of 60 modules stays far below it; the cap only bounds
/// what a hand-made code can ask of the server.
pub const MAX_ENTRIES: usize = 5000;

/// The DTSTAMP when the snapshot names no moment: fixed, never the clock, so the bytes do not
/// change from one fetch to the next.
const EPOCH_STAMP: &str = "19700101T000000Z";

/// The group QIS gives a row that belongs to no group.
const UNNAMED_GROUP: &str = "[unbenannt]";

/// The DTSTAMP of a snapshot: when its data last changed, else when it was built, else the epoch.
/// Never the clock, so an unchanged plan in an unchanged snapshot is the same bytes.
pub fn snapshot_stamp(meta: &Meta) -> String {
    [&meta.data_changed_at, &meta.built_at]
        .into_iter()
        .flatten()
        .find_map(|moment| ics::stamp_of(moment))
        .unwrap_or_else(|| EPOCH_STAMP.to_string())
}

/// The calendar of a timetable: an entry for every Termin of a visible event that holds a date, with
/// a time or as a block of whole days, a series where its dates follow a rule (`series_of`), and one
/// for every visible exam date that names a day.
///
/// `titles` maps planned module ids to their titles for the descriptions, `names` to what the
/// week's slots call them for the entries' titles (`StudyplanData::slot_names`). `label` is the
/// semester's label („WiSe 2026/27"; empty: the key's in `locale`). `stamp` is every entry's
/// DTSTAMP, as `snapshot_stamp` makes it, or a moment in RFC 3339 and UTC; anything else is the
/// epoch. `locale` is the language the calendar is written in.
pub fn calendar_of(
    t: &Timetable,
    titles: &BTreeMap<String, String>,
    names: &BTreeMap<String, String>,
    label: &str,
    stamp: &str,
    locale: Locale,
) -> Calendar {
    let texts = crate::i18n::texts(locale);
    let stamp = dtstamp(stamp);
    let names = Names { titles, short: names };
    let mut entries: Vec<Entry> = Vec::new();
    for event in t.events.iter().filter(|event| event.hidden.is_none()) {
        entries.extend(teaching(t, event, &names, locale));
    }
    let mut exams = Entries::default();
    for exam in t.exams.iter().filter(|exam| exam.hidden.is_none()) {
        exam_dates(&mut exams, exam, &names, locale);
    }
    entries.extend(exams.finish(texts));
    entries.sort_by(|a, b| start(&a.when).cmp(&start(&b.when)).then_with(|| a.uid.cmp(&b.uid)));
    let cut = keep_earliest(&mut entries);

    let label = match label.trim() {
        "" => t.key.label(locale),
        label => label.to_string(),
    };
    let mut description = texts.feed_about.to_string();
    if let Some(day) = berlin_day(&stamp).filter(|_| stamp != EPOCH_STAMP) {
        description.push_str(", ");
        description.push_str(&(texts.feed_as_of)(&day.date(locale)));
    }
    if !published(t) {
        description.push_str(" · ");
        description.push_str(texts.feed_unpublished);
    }
    if cut {
        description.push_str(" · ");
        description.push_str(&(texts.feed_first_only)(MAX_ENTRIES));
    }
    Calendar { name: (texts.feed_name)(&label), description, stamp, entries }
}

/// The subscription of a semester's plan as the page shows it. Only numeric module ids, and
/// only hidden events and hidden or chosen rows that belong to `table` (all of them when
/// `table` is None), so hide rules the timetable no longer needs do not lengthen the code.
/// `program` is the one whose abbreviations the page names the modules by
/// (`StudyplanData::program`); an id of another shape is left out, as a reader leaves it out.
/// Second value: the ids that cannot be subscribed (not numeric), each once, in their order.
pub fn subscription_of(
    key: SemesterKey,
    modules: &[String],
    program: Option<&str>,
    selection: &Selection,
    table: Option<&Timetable>,
) -> (Subscription, Vec<String>) {
    let mut numeric = Vec::with_capacity(modules.len());
    let mut other: Vec<String> = Vec::new();
    for id in modules {
        match canonical(id) {
            Some(number) => numeric.push(number),
            None if !other.contains(id) => other.push(id.clone()),
            None => {}
        }
    }
    numeric.sort_unstable();
    numeric.dedup();
    let known = table.map(Known::of);
    let has_event = |id: &u32| known.as_ref().is_none_or(|known| known.events.contains(id));
    let rows = |keys: &BTreeSet<RowKey>| -> Vec<u64> {
        keys.iter()
            .filter(|key| known.as_ref().is_none_or(|known| known.rows.contains(key)))
            .map(|key| key.packed())
            .collect()
    };
    let subscription = Subscription {
        semester: key.index(),
        modules: numeric,
        program: program.filter(|id| is_program_id(id)).map(str::to_string),
        hidden_kinds: selection.hidden_kinds.known().0,
        hidden_events: selection.hidden_events.iter().copied().filter(has_event).collect(),
        hidden_rows: rows(&selection.hidden_rows),
        chosen_rows: rows(&selection.chosen_rows),
        town: town_of(selection, table).code(),
        hidden_dates: selection
            .hidden_dates
            .iter()
            .filter(|(row, _)| known.as_ref().is_none_or(|known| known.rows.contains(row)))
            .filter_map(|(row, day)| date_value(key, *row, *day))
            .collect(),
    };
    (subscription, other)
}

/// The town a code carries. A town the page derived from the modules taken over from the
/// Regelstudienplan (`Selection::town_from`) goes in as the town it is, or as both towns where it
/// derives none: the feed derives from every module of the code, and an elective planned beside
/// them could turn its town and make the calendar differ from the page. Otherwise the choice, and
/// „derive" stays „derive".
fn town_of(selection: &Selection, table: Option<&Timetable>) -> TownChoice {
    match (selection.town, table) {
        (TownChoice::Derive, Some(t)) if selection.town_from.is_some() && !t.tracks.is_empty() => t.town.map_or(TownChoice::Both, TownChoice::Only),
        (choice, _) => choice,
    }
}

/// Whether a stored code still describes the plan: equal semester, modules, program, kinds and
/// town, and equal hidden events and hidden or chosen rows among those `table` still has. An event
/// or a row QIS removed or re-keyed does not make a subscription stale: the feed, made anew from
/// the same data, shows what the page shows. Another program does, as the feed would name the
/// modules by its abbreviations. A code that does not decode describes nothing.
pub fn same_subscription(stored: &str, current: &Subscription, table: &Timetable) -> bool {
    let Some(stored) = Subscription::from_code(stored) else {
        return false;
    };
    let known = Known::of(table);
    let seen = |s: &Subscription| {
        let rows = |packed: &[u64]| -> BTreeSet<RowKey> {
            packed.iter().filter_map(|value| RowKey::unpack(*value)).filter(|key| known.rows.contains(key)).collect()
        };
        let dates: BTreeSet<(RowKey, Day)> = s
            .key()
            .map(|key| s.hidden_dates.iter().filter_map(|value| date_of(key, *value)).filter(|(row, _)| known.rows.contains(row)).collect())
            .unwrap_or_default();
        (
            s.semester,
            s.module_ids(),
            s.program.clone(),
            KindSet(s.hidden_kinds).known(),
            s.hidden_events.iter().copied().filter(|id| known.events.contains(id)).collect::<BTreeSet<u32>>(),
            rows(&s.hidden_rows),
            rows(&s.chosen_rows),
            TownChoice::from_code(s.town),
            dates,
        )
    };
    seen(&stored) == seen(current)
}

/// The events and row keys of a timetable, teaching and exams, hidden ones included.
struct Known {
    events: BTreeSet<u32>,
    rows: BTreeSet<RowKey>,
}

impl Known {
    fn of(t: &Timetable) -> Known {
        let ids =
            t.events.iter().map(|event| event.id.as_str()).chain(t.exams.iter().map(|exam| exam.event_id.as_str()));
        let teaching = t.events.iter().flat_map(|event| event.rows.iter().filter_map(|row| row.key));
        let exams = t.exams.iter().flat_map(|exam| exam.rows.iter().filter_map(|row| row.key));
        Known { events: ids.filter_map(canonical).collect(), rows: teaching.chain(exams).collect() }
    }
}

/// An id as a code carries it: digits without a leading zero that fit a `u32`, so that the number
/// reads back as the same id.
fn canonical(id: &str) -> Option<u32> {
    let digits = !id.is_empty() && !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit());
    digits.then(|| id.parse().ok()).flatten()
}

/// Where a line stands in a description. An entry gathers the lines of every row of its UID, each
/// once, and writes them in this order, so rows of one key that differ in who teaches or in what
/// QIS notes all say it, and a single row reads as it always did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    /// What the entry is, in full: what QIS calls it, and its title where that is not its
    /// module's („Übung · Übungsgruppe 3", „Prüfung · Klausur Statistik").
    What,
    /// The modules, the group, the rhythm and the range: what the rows of a key share.
    Head,
    /// The rooms as QIS names them, where the entry's place is their short forms.
    Room,
    /// One name of who teaches; the names make one line, „Lehrende: Bleicher / Freymann".
    Teacher,
    /// QIS's comment on a row, and what is odd about an exam date.
    Said,
    /// That the range is the lecture period's.
    Assumed,
    /// That the row is one option of a choice not made yet.
    Choice,
    /// The dates of the key that no row of it holds.
    Dropped,
    /// A note of the entry's date.
    Note,
}

/// A line of a description and where it stands.
type Line = (Part, String);

/// The entries in the making, each UID once: an entry whose UID is taken widens the one there, and
/// its place and lines join those there.
#[derive(Default)]
struct Entries {
    list: Vec<Gathered>,
    /// The index of each UID in `list`.
    at: BTreeMap<String, usize>,
}

/// An entry and what its rows say of it, until `Entries::finish` writes its LOCATION and
/// DESCRIPTION.
struct Gathered {
    entry: Entry,
    /// The places of its rows, each once, in the order met.
    places: Vec<Place>,
    /// The lines of its rows, each once, in the order met.
    lines: Vec<Line>,
}

/// Where a row is, as a slot of the week names it („ZHG/HS.C") and as QIS does („Zentrales
/// Hörsaalgebäude - Hörsaal C - Zentralcampus"). A row without a room is at its campus, both ways.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    short: String,
    long: String,
}

impl Entries {
    fn add(&mut self, entry: Entry, place: Option<Place>, lines: Vec<Line>) {
        let known = self.at.get(&entry.uid).copied();
        let there = match known.and_then(|index| self.list.get_mut(index)) {
            Some(there) => {
                there.entry.when = widen(there.entry.when, entry.when);
                there.entry.tentative |= entry.tentative;
                there
            }
            None => {
                self.at.insert(entry.uid.clone(), self.list.len());
                self.list.push(Gathered { entry, places: Vec::new(), lines: Vec::new() });
                let Some(there) = self.list.last_mut() else {
                    return;
                };
                there
            }
        };
        if let Some(place) = place.filter(|place| !there.places.contains(place)) {
            there.places.push(place);
        }
        for line in lines {
            if !there.lines.contains(&line) {
                there.lines.push(line);
            }
        }
    }

    /// The entries, each with its places in their short forms as its LOCATION („ZHG/HS.A /
    /// ZHG/HS.B") and its lines as its DESCRIPTION, the places as QIS names them among them, in
    /// the language of `texts`.
    fn finish(self, texts: &Texts) -> Vec<Entry> {
        self.list
            .into_iter()
            .map(|Gathered { entry, places, mut lines }| {
                let short: Vec<&str> = places.iter().map(|place| place.short.as_str()).collect();
                let long: Vec<&str> = places.iter().map(|place| place.long.as_str()).collect();
                if long != short {
                    lines.push((Part::Room, (texts.feed_rooms)(places.len(), &long.join(" / "))));
                }
                Entry {
                    location: (!places.is_empty()).then(|| short.join(" / ")),
                    description: Some(description(lines, texts)),
                    ..entry
                }
            })
            .collect()
    }
}

/// What an entry calls the planned modules: in full in its description, and as the week's slots
/// do in its title.
struct Names<'a> {
    /// The titles, by module id.
    titles: &'a BTreeMap<String, String>,
    /// What the slots call them (`StudyplanData::slot_names`): the abbreviation, else the title
    /// cut short.
    short: &'a BTreeMap<String, String>,
}

impl Names<'_> {
    /// „EvS", „MIT1/DM": what the slots call the modules of an entry, by id and each once (the
    /// browser holds them in plan order, a code by id), else the entry's own title cut short, as
    /// a slot without its module's name has it.
    fn short_of(&self, modules: &[String], own: &str) -> String {
        let mut named: Vec<&str> = Vec::new();
        for id in by_id(modules) {
            if let Some(name) = self.short.get(id).map(|name| name.trim()).filter(|name| !name.is_empty() && !named.contains(name)) {
                named.push(name);
            }
        }
        match named.is_empty() {
            true => short_title(own),
            false => named.join("/"),
        }
    }

    /// What an entry is, in full: `kind` and the title QIS gives the event, as the page's lines
    /// say it (`views::kind_and_title`), or `kind` alone where that title is a module's, which the
    /// module's line names. `None` when neither says anything.
    fn what(&self, kind: &str, title: &str, modules: &[String]) -> Option<String> {
        let title = title.trim();
        let a_module = modules.iter().any(|id| self.titles.get(id).is_some_and(|own| own.trim() == title));
        let title = if a_module { "" } else { title };
        match (kind.trim(), title) {
            ("", "") => None,
            ("", title) => Some(title.to_string()),
            (kind, title) => Some(kind_and_title(kind, title)),
        }
    }
}

/// Module ids in the order a description lists them, each once: by length, then as text (the
/// numbers of the catalog by value).
fn by_id(modules: &[String]) -> Vec<&String> {
    let mut ids: Vec<&String> = modules.iter().collect();
    ids.sort_by_key(|id| (id.len(), *id));
    ids.dedup();
    ids
}

/// A title as short as a slot's: the kinds in their few letters, then the names („VL EvS").
fn slot_text(kinds: &str, names: &str) -> String {
    [kinds, names].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The lines in the order of their parts, the names of who teaches as one line, and the source.
fn description(mut lines: Vec<Line>, texts: &Texts) -> String {
    // A stable sort: within a part, the order met.
    lines.sort_by_key(|(part, _)| *part);
    let teachers: Vec<&str> =
        lines.iter().filter(|(part, _)| *part == Part::Teacher).map(|(_, name)| name.as_str()).collect();
    let teachers = (!teachers.is_empty()).then(|| (texts.feed_lecturers)(&teachers.join(" / ")));
    let before = lines.iter().filter(|(part, _)| *part < Part::Teacher).map(|(_, line)| line.clone());
    let after = lines.iter().filter(|(part, _)| *part > Part::Teacher).map(|(_, line)| line.clone());
    before.chain(teachers).chain(after).chain([texts.feed_source.to_string()]).collect::<Vec<_>>().join("\n")
}

/// Two dates of one UID as one: from the earlier start to the later end.
fn widen(a: When, b: When) -> When {
    match (a, b) {
        (When::Timed { day, from, to }, When::Timed { from: other_from, to: other_to, .. }) => {
            When::Timed { day, from: from.min(other_from), to: to.max(other_to) }
        }
        (When::AllDay { first, last }, When::AllDay { first: other_first, last: other_last }) => {
            When::AllDay { first: first.min(other_first), last: last.max(other_last) }
        }
        (a, _) => a,
    }
}

/// Where an entry sorts: its day, whole days first, then by start.
fn start(when: &When) -> (Day, u32) {
    match *when {
        When::Timed { day, from, .. } => (day, 1 + u32::from(from)),
        When::AllDay { first, .. } => (first, 0),
    }
}

/// Keeps the earliest entries whose VEVENTs, a series' dates that differ counted, are `MAX_ENTRIES`
/// at most; whether one was left out.
fn keep_earliest(entries: &mut Vec<Entry>) -> bool {
    let mut events = 0;
    let kept = entries
        .iter()
        .take_while(|entry| {
            events += 1 + entry.overrides.len();
            events <= MAX_ENTRIES
        })
        .count();
    let cut = kept < entries.len();
    entries.truncate(kept);
    cut
}

/// What the dates of one entry, or of one series, are of: the rows of a key (the same slot in two
/// rooms, or once more with a later end), or the single dates Radix reads as one series, by the
/// `ord` of its first date (`Row::series`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Unit {
    Key(String),
    Series(i64),
}

/// The entries of a visible teaching event: every held date of its visible rows, the dates of a
/// Termin as one series where a rule makes them (`series_of`). A row with an unclear time is left
/// out (its times say nothing a calendar can place), unless it is a block without times, whose
/// dates are whole days.
fn teaching(t: &Timetable, event: &Event, names: &Names, locale: Locale) -> Vec<Entry> {
    // An option of a choice among its module's events comes in once it is taken (owner,
    // 2026-10-19, the concept's proposal: „Eine offene Wahl kommt erst hinein, wenn gewählt ist").
    if event.across.is_some_and(|alt| alt.open) {
        return Vec::new();
    }
    let texts = crate::i18n::texts(locale);
    // As its slot in the week: „VL EvS".
    let label = slot_text(&kind_short(event, locale), &names.short_of(&event.modules, &event.title));
    let what = names.what(&type_text(event, locale), &event.title, &event.modules);
    let categories: Vec<String> = event.kinds.iter().map(|kind| kind.label(locale).to_string()).collect();
    let open = event.unresolved();
    let placed = |row: &&Row| row.hidden.is_none() && (row.from.zip(row.to).is_some() || row.occ.all_day);
    // The rows of each Termin, in the order met: its dates are one series, so what it says of the
    // whole Termin, its range and the dates it drops, is said of all of them.
    let mut units: Vec<(Unit, Vec<&Row>)> = Vec::new();
    for row in event.rows.iter().filter(placed) {
        let unit = match row.date.series {
            Some(series) => Unit::Series(series.first),
            None => Unit::Key(key_text(&event.id, row.key, &row.date)),
        };
        match units.iter_mut().find(|(seen, _)| *seen == unit) {
            Some((_, rows)) => rows.push(row),
            None => units.push((unit, vec![row])),
        }
    }
    let mut entries = Vec::new();
    for (unit, rows) in &units {
        let held: BTreeSet<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).collect();
        let rule = rule_of(rows, &held);
        let gaps = match unit {
            Unit::Series(first) => gaps(t, event, *first, &held, rule),
            Unit::Key(_) => Vec::new(),
        };
        let termin = termin_lines(event, rows, &gaps, what.as_deref(), names.titles, locale);
        let mut dates = Entries::default();
        for row in rows {
            let times = row.from.zip(row.to);
            let option = open && row.option.is_some();
            // An option of a choice not made yet says so, as its slot does: „Ü AuP · 1 von 3".
            let summary = match option {
                true => (texts.feed_one_of)(&label, event.visible_options().len()),
                false => label.clone(),
            };
            let lines: Vec<Line> = termin.iter().cloned().chain(row_lines(t, event, row, option, locale)).collect();
            let place = place(&row.date, locale);
            let key = key_text(&event.id, row.key, &row.date);
            for day in &row.occ.days {
                let when = match times {
                    Some((from, to)) => When::Timed { day: *day, from, to },
                    None => When::AllDay { first: *day, last: *day },
                };
                let notes = row
                    .occ
                    .notes
                    .iter()
                    .filter(|(on, _)| on == day)
                    .map(|(_, note)| (Part::Note, (texts.feed_note)(note.trim())));
                let entry = Entry {
                    uid: uid(&key, *day),
                    when,
                    repeat: None,
                    overrides: Vec::new(),
                    summary: summary.clone(),
                    location: None,
                    description: None,
                    url: event.source_url.clone(),
                    categories: categories.clone(),
                    tentative: row.occ.assumed || option,
                    transparent: times.is_none(),
                };
                dates.add(entry, place.clone(), lines.iter().cloned().chain(notes).collect());
            }
        }
        entries.extend(series_of(series_uid(event, unit, rows), rule, dates.finish(texts)));
    }
    entries
}

/// The rule a Termin's dates follow: its rows' rhythm (the rows of a key share it), the weeks of a
/// series of single dates, and for a block every day where it meets on a Saturday or a Sunday, else
/// the working days. `None` for a single date and a rhythm that makes no dates.
fn rule_of(rows: &[&Row], held: &BTreeSet<Day>) -> Option<Rule> {
    let row = rows.first()?;
    if let Some(series) = row.date.series {
        return Some(Rule::Weeks(series.weeks));
    }
    match Every::of(&row.date) {
        Some(Every::Week) => Some(Rule::Weeks(1)),
        Some(Every::AWeek | Every::BWeek) => Some(Rule::Weeks(2)),
        Some(Every::ThreeWeeks) => Some(Rule::Weeks(3)),
        Some(Every::FourWeeks) => Some(Rule::Weeks(4)),
        None if row.date.rhythm.as_ref().is_some_and(|rhythm| rhythm.is(Rhythm::Block)) => {
            Some(if held.iter().any(|day| day.weekday() >= 6) { Rule::Days } else { Rule::Workdays })
        }
        None => None,
    }
}

/// The UID of a Termin's series: its row key's, `148701-a2633@betula.app`; for a series of single
/// dates the key of the row that names it (`Series::first`), visible or not, so hiding its first
/// date does not make the series a new one.
fn series_uid(event: &Event, unit: &Unit, rows: &[&Row]) -> String {
    let key = match unit {
        Unit::Key(key) => key.clone(),
        Unit::Series(first) => {
            let named = event.rows.iter().find(|row| row.ord == Some(*first)).or(rows.first().copied());
            named.map(|row| key_text(&event.id, row.key, &row.date)).unwrap_or_default()
        }
    };
    format!("{key}@{UID_DOMAIN}")
}

/// The days a series of single dates (`first`, the `ord` of the row that names it) leaves out that
/// none of its rows explains: weeks QIS lists no date of it in, between its first date and its last.
/// Each says why where the semester says it, the break or a holiday (`None`: no date in QIS). A date
/// of the series that is hidden, or has no clear time, is no gap: QIS lists it.
fn gaps(t: &Timetable, event: &Event, first: i64, held: &BTreeSet<Day>, rule: Option<Rule>) -> Vec<(Day, Option<Skipped>)> {
    let (Some(rule), Some(from), Some(to)) = (rule, held.first(), held.last()) else {
        return Vec::new();
    };
    let listed: BTreeSet<Day> = event
        .rows
        .iter()
        .filter(|row| row.date.series.is_some_and(|series| series.first == first))
        .filter_map(|row| row.date.first_date.as_deref().and_then(Day::parse))
        .collect();
    rule.days(*from, *to)
        .into_iter()
        .filter(|day| !held.contains(day) && !listed.contains(day))
        .map(|day| {
            let why = match t.facts.in_break(day) {
                true => Some(Skipped::Break),
                false => t.facts.holiday(day).map(Skipped::Holiday),
            };
            (day, why)
        })
        .collect()
}

/// The dates of one Termin, an entry each as `Entries` gathers them, as one series under `uid`: its
/// rule makes them from the first to the last, the dates it makes and the Termin does not hold are
/// left out (EXDATE), and the series says what most of its dates say, the earliest of them breaking
/// a tie, while a date that says more or other follows it in full (RECURRENCE-ID). The dates stay
/// as they are where there is one, where no rule makes them, and where one of them is not of the
/// rule, begins at another time, or is another number of whole days.
fn series_of(uid: String, rule: Option<Rule>, mut dates: Vec<Entry>) -> Vec<Entry> {
    dates.sort_by(|a, b| start(&a.when).cmp(&start(&b.when)).then_with(|| a.uid.cmp(&b.uid)));
    let (Some(rule), Some(first), Some(last)) = (rule, dates.first(), dates.last()) else {
        return dates;
    };
    let (first_day, last_day) = (start(&first.when).0, start(&last.when).0);
    let fits = dates.iter().all(|date| rule.makes(first_day, start(&date.when).0) && same_shape(first.when, date.when));
    if dates.len() < 2 || !fits {
        return dates;
    }
    let forms: Vec<Entry> = dates.iter().map(form).collect();
    // How many dates say each, and the first that does.
    let mut said: BTreeMap<&Entry, (usize, usize)> = BTreeMap::new();
    for (index, own) in forms.iter().enumerate() {
        said.entry(own).or_insert((0, index)).0 += 1;
    }
    let most = said.values().max_by_key(|(count, first)| (*count, Reverse(*first))).map(|(_, first)| *first);
    let Some(master) = most.and_then(|first| forms.get(first)).cloned() else {
        return dates;
    };
    let held: BTreeSet<Day> = dates.iter().map(|date| start(&date.when).0).collect();
    let except = rule.days(first_day, last_day).into_iter().filter(|day| !held.contains(day)).collect();
    let overrides = dates
        .into_iter()
        .zip(&forms)
        .filter(|(_, own)| **own != master)
        .map(|(date, _)| Entry { uid: uid.clone(), ..date })
        .collect();
    let when = master.when.on(first_day);
    vec![Entry { uid, when, repeat: Some(Repeat { rule, until: last_day, except }), overrides, ..master }]
}

/// A date's entry without its UID and its day: what a series compares its dates by.
fn form(date: &Entry) -> Entry {
    Entry { uid: String::new(), when: date.when.on(Day(0)), ..date.clone() }
}

/// Whether two dates can be of one series: both begin at one time of their days, or both are the
/// same number of whole days.
fn same_shape(a: When, b: When) -> bool {
    match (a, b) {
        (When::Timed { from, .. }, When::Timed { from: other, .. }) => from == other,
        (When::AllDay { first, last }, When::AllDay { first: other_first, last: other_last }) => last.0 - first.0 == other_last.0 - other_first.0,
        _ => false,
    }
}

/// What the entries of one Termin say of it, whichever of its rows holds a date: what it is, its
/// modules, its group, its rhythm and the range its rows span, and the dates none of them holds, a
/// series' `gaps` among them.
fn termin_lines(
    event: &Event,
    rows: &[&Row],
    gaps: &[(Day, Option<Skipped>)],
    what: Option<&str>,
    titles: &BTreeMap<String, String>,
    locale: Locale,
) -> Vec<Line> {
    let texts = crate::i18n::texts(locale);
    let group = rows
        .first()
        .and_then(|row| text(&row.date.group_name))
        .filter(|group| *group != UNNAMED_GROUP)
        .map(texts.feed_group);
    let head = module_lines(&event.modules, titles, texts).into_iter().chain(group).chain(rhythm_line(rows, locale));
    let what = what.map(|what| (Part::What, what.to_string()));
    what.into_iter()
        .chain(head.map(|line| (Part::Head, line)))
        .chain(dropped(rows, gaps, locale).map(|line| (Part::Dropped, line)))
        .collect()
}

/// What a teaching row says of itself on each of its dates: who teaches, QIS's comment, an
/// assumed range and an open choice.
fn row_lines(t: &Timetable, event: &Event, row: &Row, option: bool, locale: Locale) -> Vec<Line> {
    let texts = crate::i18n::texts(locale);
    let date = &row.date;
    let mut lines: Vec<Line> = Vec::new();
    lines.extend(text(&date.instructor).map(|name| (Part::Teacher, name.to_string())));
    lines.extend(text(&date.comment).map(|comment| (Part::Said, comment.to_string())));
    if let Some((first, last)) = t.facts.lecture.filter(|_| row.occ.assumed) {
        lines.push((Part::Assumed, (texts.feed_assumed)(&first.date(locale), &last.date(locale))));
    }
    if option {
        let line = (texts.feed_choose)(event.visible_options().len(), groups_word(event.kinds, texts));
        lines.push((Part::Choice, line));
    }
    lines
}

/// „Modul 12104 Entwicklung von Softwaresystemen", one line per module, by id: the browser holds
/// the plan's order, a code does not.
fn module_lines(modules: &[String], titles: &BTreeMap<String, String>, texts: &Texts) -> Vec<String> {
    by_id(modules)
        .into_iter()
        .map(|id| (texts.feed_module)(id, titles.get(id).map(|title| title.trim()).filter(|title| !title.is_empty())))
        .collect()
}

/// The options of an open choice by what they are: „Übungsgruppen", else „Gruppen".
fn groups_word(kinds: KindSet, texts: &Texts) -> &'static str {
    let mut each = kinds.iter();
    let only = match (each.next(), each.next()) {
        (Some(kind), None) => Some(kind),
        _ => None,
    };
    match only {
        Some(EventKind::Exercise) => texts.exercise_groups,
        Some(EventKind::Practical) => texts.practical_groups,
        Some(EventKind::Seminar) => texts.seminar_groups,
        Some(EventKind::Tutorial) => texts.tutorial_groups,
        Some(EventKind::Project) => texts.project_groups,
        _ => texts.groups,
    }
}

/// The rhythm and the range the rows of a Termin state, from the first of their first dates to
/// the last of their last: „wöchentlich 13.10.2026–26.01.2027". An assumed range is not stated (a
/// line of its own says so), and a single date is the entry's own. The rows share their rhythm
/// (the key holds it), so the first one names it; a rhythm only QIS's words name is written as
/// QIS writes it. A series of single dates says so after its range: „wöchentlich
/// 13.10.2026–17.11.2026 (in QIS als Einzeltermine)".
fn rhythm_line(rows: &[&Row], locale: Locale) -> Option<String> {
    let texts = crate::i18n::texts(locale);
    let first = rows.first()?;
    let date = &first.date;
    let rhythm = date.rhythm.as_ref().and_then(Code::known);
    // A series of single dates every one to four weeks; else the rows' own rhythm.
    let series = date.series.map(|series| texts.series_rhythm(series.weeks));
    let name = match Every::of(date) {
        _ if series.is_some() => series.map(str::to_string),
        Some(Every::Week) => Some(texts.weekly.to_string()),
        Some(Every::AWeek) => Some(texts.fortnightly_a.to_string()),
        Some(Every::BWeek) => Some(texts.fortnightly_b.to_string()),
        Some(Every::ThreeWeeks) => Some(texts.every_three_weeks.to_string()),
        Some(Every::FourWeeks) => Some(texts.every_four_weeks.to_string()),
        None => match rhythm {
            Some(Rhythm::Single) => Some(texts.single_date.to_string()),
            Some(Rhythm::Block) => Some(texts.block_course.to_string()),
            _ => text(&date.rhythm_raw).map(str::to_string),
        },
    };
    let stated = rows
        .iter()
        .filter(|row| !row.occ.assumed)
        .filter_map(|row| {
            let first = row.date.first_date.as_deref().and_then(Day::parse)?;
            let last = row.date.last_date.as_deref().and_then(Day::parse)?;
            Some((first.min(last), first.max(last)))
        })
        .reduce(|(first, last), (other_first, other_last)| (first.min(other_first), last.max(other_last)));
    let range = stated
        .filter(|(first_day, last_day)| (rhythm != Some(Rhythm::Single) || first.date.series.is_some()) && first_day != last_day)
        .map(|(first_day, last_day)| format!("{}–{}", first_day.date(locale), last_day.date(locale)));
    let line = match (name, range) {
        (Some(name), Some(range)) => Some(format!("{name} {range}")),
        (name, range) => name.or(range),
    };
    match first.date.series {
        Some(_) => line.map(|line| format!("{line} ({})", texts.feed_singles)),
        None => line,
    }
}

/// „Entfällt: 22.12., 29.12. (vorlesungsfrei); 31.10. (Reformationstag); 05.11. (laut QIS)": the
/// dates the rows of a Termin drop, the break first, then the holidays, then what QIS cancels, and
/// last the weeks a series of single dates has no date in (`gaps`), each reason once with its dates.
/// A date another row of the Termin holds is not dropped: the calendar has it, in that row's room
/// and times. A date two rows drop is named once, for the first reason met. The reason QIS gives is
/// its own text; where it gives none, „laut QIS".
fn dropped(rows: &[&Row], gaps: &[(Day, Option<Skipped>)], locale: Locale) -> Option<String> {
    let texts = crate::i18n::texts(locale);
    let skipped = || {
        let gaps = gaps.iter().filter_map(|(day, why)| why.map(|why| (*day, why)));
        rows.iter().flat_map(|row| row.occ.skipped.iter().map(|(day, why)| (*day, *why))).chain(gaps)
    };
    let named_why = |(day, why): (Day, Skipped)| (day, why.text(locale));
    let mut breaks: Vec<(Day, &str)> = skipped().filter(|(_, why)| *why == Skipped::Break).map(named_why).collect();
    let mut holidays: Vec<(Day, &str)> = skipped().filter(|(_, why)| *why != Skipped::Break).map(named_why).collect();
    let mut cancelled: Vec<(Day, &str)> = rows
        .iter()
        .flat_map(|row| row.occ.cancelled.iter().map(|(day, note, _)| (*day, text(note).unwrap_or(texts.cancelled_by_qis))))
        .collect();
    let unlisted = gaps.iter().filter(|(_, why)| why.is_none()).map(|(day, _)| (*day, texts.no_date_in_qis));
    // Each list by day, as one row's lists are; a stable sort keeps the first row's reason first.
    for list in [&mut breaks, &mut holidays, &mut cancelled] {
        list.sort_by_key(|(day, _)| *day);
    }
    let mut named: BTreeSet<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).collect();
    let mut groups: Vec<(&str, Vec<Day>)> = Vec::new();
    for (day, why) in breaks.into_iter().chain(holidays).chain(cancelled).chain(unlisted) {
        if named.insert(day) {
            group(&mut groups, why, day);
        }
    }
    if groups.is_empty() {
        return None;
    }
    let parts: Vec<String> = groups
        .iter()
        .map(|(why, days)| format!("{} ({why})", days.iter().map(|day| day.day_month(locale)).collect::<Vec<_>>().join(", ")))
        .collect();
    Some((texts.feed_dropped)(&parts.join("; ")))
}

/// Adds a day to the group of its reason, a new group at the end for a new reason.
fn group<'a>(groups: &mut Vec<(&'a str, Vec<Day>)>, why: &'a str, day: Day) {
    match groups.iter_mut().find(|(seen, _)| *seen == why) {
        Some((_, days)) => days.push(day),
        None => groups.push((why, vec![day])),
    }
}

/// The entries of a visible exam: one per visible row that names a day. A sitting has its times;
/// a window, a deadline and a day without a time are whole days. An open date is left out.
///
/// The title says what it is and whose, as short as the rest („Prüfung EvS", „Abgabe DB"); a
/// retake and a later sitting say so after it („· Wdh.", „· 2. Termin"), since each may be one of
/// two entries of the module that a student does not both attend. The description says when a
/// deadline ends and that a day has no time, which the entry's whole days only suggest.
fn exam_dates(entries: &mut Entries, exam: &Exam, names: &Names, locale: Locale) {
    let texts = crate::i18n::texts(locale);
    let modules: Vec<Line> = module_lines(&exam.modules, names.titles, texts).into_iter().map(|line| (Part::Head, line)).collect();
    let name = names.short_of(&exam.modules, &exam.title);
    for row in exam.rows.iter().filter(|row| row.hidden.is_none()) {
        let (when, what, when_said) = match row.shape {
            ExamShape::Sitting { day, from, to } => (When::Timed { day, from, to }, texts.exam, None),
            ExamShape::Deadline { day } => (When::AllDay { first: day, last: day }, texts.submission, Some(texts.by_midnight)),
            ExamShape::Window { first, last } => (When::AllDay { first, last }, texts.exam_period, None),
            ExamShape::DayOnly { day } => (When::AllDay { first: day, last: day }, texts.exam, Some(texts.time_open)),
            ExamShape::Open => continue,
        };
        let mut summary = slot_text(what, &name);
        if exam.retake {
            summary.push_str(" · ");
            summary.push_str(texts.retake);
        }
        if row.rank == 2 {
            summary.push_str(" · ");
            summary.push_str(texts.second_sitting);
        }
        let full = names.what(what, &exam.title, &exam.modules).map(|full| match when_said {
            Some(said) => format!("{full} ({said})"),
            None => full,
        });
        let said = odd(row, texts).into_iter().chain(text(&row.date.comment).map(str::to_string));
        let lines: Vec<Line> = full
            .map(|full| (Part::What, full))
            .into_iter()
            .chain(modules.iter().cloned())
            .chain(said.map(|line| (Part::Said, line)))
            .collect();
        let entry = Entry {
            uid: uid(&key_text(&exam.event_id, row.key, &row.date), start(&when).0),
            when,
            repeat: None,
            overrides: Vec::new(),
            summary,
            location: None,
            description: None,
            url: exam.source_url.clone(),
            categories: vec![EventKind::Exam.label(locale).to_string()],
            tentative: row.rank == 2 || exam.retake,
            transparent: matches!(when, When::AllDay { .. }),
        };
        entries.add(entry, place(&row.date, locale), lines);
    }
}

/// What is odd about an exam date shown as QIS states it, in the module page's words.
fn odd(row: &ExamRow, texts: &Texts) -> Option<String> {
    let odd: Vec<&str> = [
        (Reason::UnusualTime, texts.unusual_time),
        (Reason::EndsBeforeStart, texts.ends_before_start),
        (Reason::DateOutsideSemester, texts.outside_semester),
    ]
    .into_iter()
    .filter(|(reason, _)| row.reading.has(*reason))
    .map(|(_, text)| text)
    .collect();
    (!odd.is_empty()).then(|| (texts.qis_states)(&odd.join(", ")))
}

/// The row key as text, `148369-aaf38`; for an event id that is no number (none is), the id and
/// the row's fingerprint.
fn key_text(event_id: &str, key: Option<RowKey>, date: &EventDate) -> String {
    key.map(RowKey::text).unwrap_or_else(|| format!("{event_id}-{:05x}", fingerprint(date)))
}

/// `148701-a2633-20261013@betula.app`: a row's date.
fn uid(key: &str, day: Day) -> String {
    format!("{key}-{}@{UID_DOMAIN}", day.compact())
}

/// Where a row is: its room in the short form of schema 9 („HG/0.20", `EventDate::room_shown`)
/// and as QIS names it, with its campus („Hauptgebäude - HG 0.20 - Zentralcampus"); else the
/// campus alone, both ways, in `locale`.
fn place(date: &EventDate, locale: Locale) -> Option<Place> {
    match text(&date.room) {
        Some(room) => Some(Place { short: date.room_shown().unwrap_or(room).to_string(), long: room.to_string() }),
        None => {
            let campus = date.campus.as_ref().map(|campus| campus.label(locale).trim().to_string()).filter(|label| !label.is_empty())?;
            Some(Place { short: campus.clone(), long: campus })
        }
    }
}

/// A text of the source, trimmed; `None` when there is none.
fn text(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|value| !value.is_empty())
}

/// Whether the snapshot has anything of the semester: a lecture period, a dated row of a planned
/// module or an exam. A semester QIS has not published yet (SoSe 2027 in September 2026) has
/// none, and its calendar says so rather than looking like a plan without dates.
fn published(t: &Timetable) -> bool {
    t.facts.lecture.is_some()
        || t.events.iter().any(|event| !event.rows.is_empty())
        || t.exams.iter().any(|exam| !exam.rows.is_empty())
}

/// A DTSTAMP as the writer takes it: a DTSTAMP given is kept, a moment in RFC 3339 and UTC made
/// one, and anything else is the epoch.
fn dtstamp(stamp: &str) -> String {
    let stamp = stamp.trim();
    if utc_moment(stamp).is_some() {
        return stamp.to_string();
    }
    ics::stamp_of(stamp).unwrap_or_else(|| EPOCH_STAMP.to_string())
}

/// The day and the minute of the day, in UTC, of a DTSTAMP `YYYYMMDDTHHMMSSZ`.
fn utc_moment(stamp: &str) -> Option<(Day, u32)> {
    let shaped = stamp.len() == 16 && stamp.get(8..9) == Some("T") && stamp.get(15..) == Some("Z");
    if !shaped {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<u32> {
        stamp.get(from..to).filter(|digits| digits.bytes().all(|b| b.is_ascii_digit())).and_then(|d| d.parse().ok())
    };
    let day = Day::from_ymd(i32::try_from(number(0, 4)?).ok()?, number(4, 6)?, number(6, 8)?)?;
    let (hour, minute, second) = (number(9, 11)?, number(11, 13)?, number(13, 15)?);
    (hour < 24 && minute < 60 && second <= 60).then_some((day, hour * 60 + minute))
}

/// The day in Cottbus of a DTSTAMP: UTC plus one hour, plus two from the last Sunday of March
/// 01:00 UTC to the last Sunday of October 01:00 UTC. „Stand" is a date the student reads, so a
/// snapshot changed at 22:30 UTC in September is of the next day.
fn berlin_day(stamp: &str) -> Option<Day> {
    let (day, minute) = utc_moment(stamp)?;
    let (year, ..) = day.ymd();
    let at = i64::from(day.0) * 1440 + i64::from(minute);
    let switch = |month: u32| last_sunday(year, month).map(|sunday| i64::from(sunday.0) * 1440 + 60);
    let summer = matches!((switch(3), switch(10)), (Some(from), Some(to)) if (from..to).contains(&at));
    let local = at + if summer { 120 } else { 60 };
    i32::try_from(local.div_euclid(1440)).ok().map(Day)
}

/// The last Sunday of a month of 31 days.
fn last_sunday(year: i32, month: u32) -> Option<Day> {
    let last = Day::from_ymd(year, month, 31)?;
    Some(last.plus(-i32::from(last.weekday() % 7)))
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Town;
    use folia_model::rows_detail::{DateRow, ModuleSws, Series};
    use folia_query as queries;

    use crate::facts::SemesterFacts;
    use crate::model::Input;
    use crate::model::tests::{d, ids, invariants, planned, sws, teaching, winter, Fixture};
    use super::*;

    /// The two planned modules of the synthetic semester.
    const A: &str = "11111";
    const B: &str = "22222";

    fn titles() -> BTreeMap<String, String> {
        BTreeMap::from([(A.to_string(), "Softwaretechnik".to_string()), (B.to_string(), "Datenbanken".to_string())])
    }

    /// What the week's slots call the two modules: their abbreviations.
    fn names() -> BTreeMap<String, String> {
        BTreeMap::from([(A.to_string(), "SWT".to_string()), (B.to_string(), "DB".to_string())])
    }

    fn room(mut row: Fixture, room: &str) -> Fixture {
        row.0.date.room = Some(room.into());
        row
    }

    /// A lecture hall as QIS names it, and its short form (schema 9).
    fn hall(row: Fixture, letter: char) -> Fixture {
        let mut row = room(row, &format!("Zentrales Hörsaalgebäude - Hörsaal {letter} - Zentralcampus"));
        row.0.date.room_short = Some(format!("ZHG/HS.{letter}"));
        row
    }

    /// An exam row as `modules_exams` delivers it: no type, group or rhythm; the weekday of its
    /// first day; in the Audimax at Zentralcampus.
    fn exam(
        module: &str,
        event: &str,
        ord: i64,
        title: &str,
        days: Option<(&str, &str)>,
        time: Option<(&str, &str)>,
    ) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: title.into(),
                event_type: None,
                group_name: None,
                weekday: days.map(|(first, _)| i64::from(d(first).weekday())),
                start_time: time.map(|(from, _)| from.into()),
                end_time: time.map(|(_, to)| to.into()),
                rhythm: None,
                rhythm_raw: None,
                first_date: days.map(|(first, _)| first.into()),
                last_date: days.map(|(_, last)| last.into()),
                room: Some("Audimax".into()),
                campus: Some(Code::parse("zentralcampus")),
                instructor: None,
                comment: None,
                source_url: Some(format!("https://qis.example/{event}")),
                room_short: None,
                series: None,
                track: None,
                choice: None,
                option: None,
                extra: false,
            },
        }
    }

    /// The timetable of synthetic rows of 2026W, with exams.
    fn build(
        rows: &[Fixture],
        exams: &[DateRow],
        modules: &[&str],
        sws: &[ModuleSws],
        selection: &Selection,
    ) -> Timetable {
        build_in(&winter(), rows, exams, modules, sws, selection)
    }

    fn build_in(
        facts: &SemesterFacts,
        rows: &[Fixture],
        exams: &[DateRow],
        modules: &[&str],
        sws: &[ModuleSws],
        selection: &Selection,
    ) -> Timetable {
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let modules = ids(modules);
        let input = Input { key: facts.key, semester: None, facts, modules: &modules, schedule: &schedule, exams, sws, taken: &[], choices: &[], listed: &[] };
        let built = Timetable::build(&input, selection);
        invariants(&built);
        built
    }

    /// A block of whole days, Saturdays and Sundays included.
    fn block(event: &str, first: &str, last: &str) -> Fixture {
        let mut block = teaching(A, event, 1, "Blockseminar", 6, "09:00", "17:00").rhythm("block").range(first, last);
        block.0.date.rhythm_raw = Some("Block+SaSo".into());
        (block.0.date.start_time, block.0.date.end_time) = (None, None);
        block
    }

    /// A semester of two modules with every case the calendar tells apart. The indices matter:
    /// 5 is the Wednesday option of 20, 9 the Termin of 60 that is hidden.
    fn rows() -> Vec<Fixture> {
        let mut saturday = teaching(A, "35", 1, "Übung", 6, "09:15", "10:45").title("Softwaretechnik");
        saturday.0.cancelled_dates = Some("07.11.2026: 14.11.2026: Raumwechsel".into());
        saturday.0.date.instructor = Some("Robel".into());
        let mut unclear = teaching(A, "61", 1, "Übung", 4, "13:45", "15:15");
        unclear.0.date.end_time = Some("12:00".into());
        let mut pattern = teaching(A, "62", 1, "Übung", 7, "08:00", "09:30").rhythm("other").undated();
        pattern.0.date.rhythm_raw = Some("vierwöch.".into());
        vec![
            // One key in two rooms, and once more until 13:30: one entry per date.
            hall(teaching(A, "10", 1, "Vorlesung", 2, "11:30", "13:00").title("Softwaretechnik"), 'A'),
            hall(teaching(A, "10", 2, "Vorlesung", 2, "11:30", "13:30").title("Softwaretechnik"), 'B'),
            hall(teaching(A, "10", 3, "Vorlesung", 2, "11:30", "13:00").title("Softwaretechnik"), 'A'),
            saturday,
            // An Übung of 2 SWS at two times: „1 von 2".
            teaching(B, "20", 1, "Übung", 1, "15:30", "17:00").title("Datenbanken"),
            teaching(B, "20", 2, "Übung", 3, "15:30", "17:00").title("Datenbanken"),
            // No range: the lecture period is assumed.
            teaching(A, "30", 1, "Seminar", 4, "09:15", "10:45").undated(),
            block("40", "2026-11-14", "2026-11-15"),
            teaching(A, "50", 1, "Übung", 5, "09:15", "10:45"),
            teaching(A, "60", 1, "Übung", 5, "11:30", "13:00"),
            unclear,
            pattern,
            // One event of both modules.
            teaching(B, "80", 1, "Tutorium", 5, "15:30", "17:00").title("Gemeinsam"),
            teaching(A, "80", 1, "Tutorium", 5, "15:30", "17:00").title("Gemeinsam"),
        ]
    }

    /// The exams of the two modules. The indices matter: 9 is the sitting that is hidden.
    fn exams() -> Vec<DateRow> {
        let day = |iso: &'static str| Some((iso, iso));
        let in_room = |mut row: DateRow, room: &str| {
            row.date.room = Some(room.into());
            row
        };
        let mut audimax = in_room(exam(A, "70", 1, "Softwaretechnik", day("2027-02-11"), Some(("11:00", "13:00"))), "Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus");
        audimax.date.room_short = Some("ZHG/AM.1".into());
        vec![
            audimax,
            exam(A, "70", 2, "Softwaretechnik", day("2027-03-11"), Some(("11:00", "13:00"))),
            exam(B, "71", 1, "Datenbanken", Some(("2027-02-08", "2027-02-19")), None),
            exam(B, "72", 1, "Hausarbeit Datenbanken", day("2027-02-14"), Some(("23:45", "24:00"))),
            exam(B, "73", 1, "Mündliche Prüfung Datenbanken", day("2027-02-16"), None),
            exam(A, "74", 1, "Offen", None, None),
            exam(B, "75", 1, "Wiederholungsprüfung Datenbanken", day("2027-02-12"), Some(("13:00", "15:00"))),
            in_room(exam(B, "76", 1, "Datenbanken", day("2027-02-12"), Some(("09:00", "11:00"))), "HS 1"),
            in_room(exam(B, "76", 2, "Datenbanken", day("2027-02-12"), Some(("09:00", "11:30"))), "HS 2"),
            exam(A, "77", 1, "Softwaretechnik Zusatz", day("2027-02-20"), Some(("10:00", "12:00"))),
        ]
    }

    fn two_sws() -> Vec<ModuleSws> {
        vec![sws(B, "exercise", 2.0)]
    }

    fn hidden_exam() -> RowKey {
        RowKey::of(&exams()[9].date).unwrap()
    }

    /// Event 50 hidden, the Termin of 60 and the sitting of 77 hidden.
    fn selection() -> Selection {
        Selection {
            hidden_events: [50].into(),
            hidden_rows: [rows()[9].key(), hidden_exam()].into(),
            ..Selection::default()
        }
    }

    fn semester() -> Timetable {
        build(&rows(), &exams(), &[A, B], &two_sws(), &selection())
    }

    fn calendar(t: &Timetable) -> Calendar {
        calendar_of(t, &titles(), &names(), "WiSe 2026/27", "2026-09-23T12:35:16Z", Locale::De)
    }

    /// The entry under `uid`: a series, a single date or an exam date.
    fn entry<'c>(calendar: &'c Calendar, uid: &str) -> &'c Entry {
        calendar.entries.iter().find(|entry| entry.uid == uid).unwrap_or_else(|| panic!("no entry {uid}"))
    }

    /// The series of the Termin of `row`, under its key.
    fn series<'c>(calendar: &'c Calendar, row: &Fixture) -> &'c Entry {
        entry(calendar, &format!("{}@betula.app", row.key().text()))
    }

    /// Every date of a calendar as a client shows it (`Entry::dates`), in the calendar's order.
    fn dated(calendar: &Calendar) -> Vec<Entry> {
        calendar.entries.iter().flat_map(Entry::dates).collect()
    }

    /// The dates of an event's Termine.
    fn of_event(calendar: &Calendar, event: &str) -> Vec<Entry> {
        dated(calendar).into_iter().filter(|date| date.uid.starts_with(&format!("{event}-"))).collect()
    }

    /// The date `day` of the Termin of `row`: of its series, or an entry of its own.
    fn find_on(calendar: &Calendar, row: &Fixture, day: &str) -> Option<Entry> {
        let key = row.key().text();
        let own = |uid: &str| uid.starts_with(&format!("{key}@")) || uid.starts_with(&format!("{key}-"));
        dated(calendar).into_iter().find(|date| own(&date.uid) && date.when.day() == d(day))
    }

    fn on(calendar: &Calendar, row: &Fixture, day: &str) -> Entry {
        find_on(calendar, row, day).unwrap_or_else(|| panic!("no date {day} of {}", row.key().text()))
    }

    fn exam_uid(row: &DateRow) -> String {
        let first = d(row.date.first_date.as_deref().unwrap());
        format!("{}-{}@betula.app", RowKey::of(&row.date).unwrap().text(), first.compact())
    }

    fn day_of(entry: &Entry) -> Day {
        entry.when.day()
    }

    /// The VEVENTs a calendar writes: an entry, and a series' dates that differ.
    fn vevents(calendar: &Calendar) -> usize {
        calendar.entries.iter().map(|entry| 1 + entry.overrides.len()).sum()
    }

    #[test]
    fn a_hidden_date_is_left_out_of_its_series() {
        let rows = rows();
        // The Tuesday lecture of 13.10. hidden („Nur am 13.10. ausblenden"): the series leaves the day
        // out as it leaves out the break, and says nothing of it, since QIS cancels nothing.
        let mut hidden = selection();
        hidden.hidden_dates = [(rows[0].key(), d("2026-10-13"))].into();
        let c = calendar(&build(&rows, &exams(), &[A, B], &two_sws(), &hidden));
        let lecture = series(&c, &rows[0]);
        assert_eq!(lecture.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-26"), except: vec![d("2026-10-13"), d("2026-12-22"), d("2026-12-29")] }));
        assert_eq!(lecture.description, series(&calendar(&semester()), &rows[0]).description);
        // Its first date hidden, the series begins a week later under the same UID.
        hidden.hidden_dates = [(rows[0].key(), d("2026-10-06"))].into();
        let c = calendar(&build(&rows, &exams(), &[A, B], &two_sws(), &hidden));
        assert_eq!(series(&c, &rows[0]).when.day(), d("2026-10-13"));
    }

    #[test]
    fn rows_of_one_key_are_one_series() {
        let rows = rows();
        let c = calendar(&semester());
        // Every Tuesday from 06.10.2026 to 26.01.2027, the two of the break left out, each in both
        // rooms until 13:30: one series that no date differs from.
        let lecture = series(&c, &rows[0]);
        assert_eq!(lecture.when, When::Timed { day: d("2026-10-06"), from: 690, to: 810 }, "until the later end");
        assert_eq!(lecture.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-26"), except: vec![d("2026-12-22"), d("2026-12-29")] }));
        assert!(lecture.overrides.is_empty(), "{:?}", lecture.overrides);
        // As short as its slot in the week, both rooms in their short forms; all of it in full in
        // the description, the event's title only where it is not its module's.
        assert_eq!(lecture.summary, "VL SWT");
        assert_eq!(lecture.location.as_deref(), Some("ZHG/HS.A / ZHG/HS.B"));
        assert_eq!(lecture.categories, ["Vorlesung"]);
        assert_eq!(lecture.url.as_deref(), Some("https://qis.example/10"));
        assert_eq!(
            lecture.description.as_deref(),
            Some(
                "Vorlesung\nModul 11111 Softwaretechnik\nwöchentlich 06.10.2026–26.01.2027\nRäume: Zentrales \
                 Hörsaalgebäude - Hörsaal A - Zentralcampus / Zentrales Hörsaalgebäude - Hörsaal B - \
                 Zentralcampus\nEntfällt: 22.12., 29.12. (vorlesungsfrei)\nQuelle: QIS"
            )
        );
        assert!(!lecture.tentative && !lecture.transparent);
        // 17 Tuesdays but the two in the break, each once, each as the series says.
        let dates = of_event(&c, "10");
        assert_eq!(dates.len(), 15);
        assert!(dates.iter().all(|date| date.description == lecture.description && date.location == lecture.location));
        assert_eq!(on(&c, &rows[1], "2027-01-26").when, When::Timed { day: d("2027-01-26"), from: 690, to: 810 });

        // Two rooms and two ends of one exam sitting.
        let exams = exams();
        let sitting = entry(&c, &exam_uid(&exams[7]));
        assert_eq!(sitting.when, When::Timed { day: d("2027-02-12"), from: 540, to: 690 });
        assert_eq!(sitting.summary, "Prüfung DB");
        // Rooms without a short form: as QIS names them, and no line repeats them.
        assert_eq!(sitting.location.as_deref(), Some("HS 1 / HS 2"));
        assert_eq!(sitting.description.as_deref(), Some("Prüfung\nModul 22222 Datenbanken\nQuelle: QIS"));
        assert_eq!(of_event(&c, "76").len(), 1);

        // Every UID once, the entries by start.
        let uids: BTreeSet<&str> = c.entries.iter().map(|entry| entry.uid.as_str()).collect();
        assert_eq!(uids.len(), c.entries.len());
        assert!(c
            .entries
            .windows(2)
            .all(|pair| (start(&pair[0].when), &pair[0].uid) < (start(&pair[1].when), &pair[1].uid)));
        assert!(c.entries.iter().all(|entry| entry.uid.ends_with("@betula.app")));
    }

    /// An option of a choice among a module's events comes into the calendar once it is taken
    /// (owner, 2026-10-19, the concept: „Eine offene Wahl kommt erst hinein, wenn gewählt ist").
    #[test]
    fn an_open_choice_among_events_waits_for_its_pick() {
        use crate::model::tests::{choice_row, choosing};
        let rows = [
            teaching(A, "60", 1, "Vorlesung", 1, "15:30", "17:00"),
            teaching(A, "61", 1, "Übung", 2, "07:30", "09:00").option(1),
            teaching(A, "62", 1, "Übung", 3, "13:45", "15:15").option(2),
        ];
        let choices = [choice_row(A, "group", 1, 1, "61"), choice_row(A, "group", 1, 2, "62")];
        let open = calendar(&choosing(&rows, &[A], &choices, &[], &Selection::default()));
        assert!(!of_event(&open, "60").is_empty());
        assert!(of_event(&open, "61").is_empty() && of_event(&open, "62").is_empty());
        let chosen = Selection { chosen_rows: [rows[2].key()].into(), ..Selection::default() };
        let decided = calendar(&choosing(&rows, &[A], &choices, &[], &chosen));
        assert!(of_event(&decided, "61").is_empty() && !of_event(&decided, "62").is_empty());
        assert!(of_event(&decided, "62").iter().all(|entry| !entry.tentative));
    }

    #[test]
    fn what_may_not_happen_is_tentative() {
        let rows = rows();
        let c = calendar(&semester());
        // An open choice: both options' dates, tentative, with the prompt; a series each.
        let options = of_event(&c, "20");
        assert_eq!(options.len(), 30);
        assert_eq!(c.entries.iter().filter(|entry| entry.uid.starts_with("20-") && entry.repeat.is_some()).count(), 2);
        for option in &options {
            assert!(option.tentative, "{}", option.uid);
            assert_eq!(option.summary, "Ü DB · 1 von 2", "as its slot says");
            assert!(option.description.as_deref().unwrap().contains("\nEine von 2 Übungsgruppen; in Betula wählen\n"));
        }
        // The choice made: the other option's dates go, and the chosen ones are firm.
        let chosen = Selection { chosen_rows: [rows[5].key()].into(), ..selection() };
        let decided = calendar(&build(&rows, &exams(), &[A, B], &two_sws(), &chosen));
        let left = of_event(&decided, "20");
        assert_eq!(left.len(), 15);
        assert!(left.iter().all(|entry| !entry.tentative && entry.uid.starts_with(&rows[5].key().text())));
        assert!(!left[0].description.as_deref().unwrap().contains("Eine von"));
        assert_eq!(left[0].summary, "Ü DB");

        // A range QIS does not state: the lecture period, named.
        let assumed = of_event(&c, "30");
        assert_eq!(assumed.len(), 15);
        assert_eq!(
            (assumed.first().map(day_of), assumed.last().map(day_of)),
            (Some(d("2026-10-08")), Some(d("2027-01-28")))
        );
        for entry in &assumed {
            assert!(entry.tentative);
            assert_eq!(
                entry.description.as_deref(),
                Some(
                    "Seminar 30\nModul 11111 Softwaretechnik\nwöchentlich\nZeitraum in QIS nicht angegeben; \
                     angenommen: Vorlesungszeit 05.10.2026–31.01.2027\nEntfällt: 24.12., 31.12. (vorlesungsfrei)\n\
                     Quelle: QIS"
                )
            );
        }

        // Exams: the second sitting and the retake are tentative, the first sitting is not.
        let exams = exams();
        let first = entry(&c, &exam_uid(&exams[0]));
        assert_eq!((first.summary.as_str(), first.tentative), ("Prüfung SWT", false));
        assert_eq!(first.when, When::Timed { day: d("2027-02-11"), from: 660, to: 780 });
        assert_eq!(first.categories, ["Prüfung"]);
        assert_eq!(first.location.as_deref(), Some("ZHG/AM.1"));
        assert_eq!(
            first.description.as_deref(),
            Some("Prüfung\nModul 11111 Softwaretechnik\nRaum: Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus\nQuelle: QIS")
        );
        // The later sitting and the retake say so after the module: a student attends one of two.
        let second = entry(&c, &exam_uid(&exams[1]));
        assert_eq!((second.summary.as_str(), second.tentative), ("Prüfung SWT · 2. Termin", true));
        let retake = entry(&c, &exam_uid(&exams[6]));
        assert_eq!((retake.summary.as_str(), retake.tentative), ("Prüfung DB · Wdh.", true));
        assert!(retake.description.as_deref().unwrap().starts_with("Prüfung · Wiederholungsprüfung Datenbanken\n"));
        assert!(!entry(&c, &exam_uid(&exams[7])).tentative, "B's first sitting, that day");
    }

    #[test]
    fn whole_days_and_what_is_left_out() {
        let c = calendar(&semester());
        let exams = exams();
        // An exam window, a deadline and a day without a time are whole days that keep the time free.
        let window = entry(&c, &exam_uid(&exams[2]));
        assert_eq!(window.when, When::AllDay { first: d("2027-02-08"), last: d("2027-02-19") });
        assert_eq!(window.summary, "Prüfungszeitraum DB");
        let deadline = entry(&c, &exam_uid(&exams[3]));
        assert_eq!(deadline.when, When::AllDay { first: d("2027-02-14"), last: d("2027-02-14") });
        assert_eq!(deadline.summary, "Abgabe DB");
        let day_only = entry(&c, &exam_uid(&exams[4]));
        assert_eq!(day_only.when, When::AllDay { first: d("2027-02-16"), last: d("2027-02-16") });
        assert_eq!(day_only.summary, "Prüfung DB");
        // The description says what the whole days leave open, and the exam's own title.
        let first_line = |entry: &Entry| entry.description.as_deref().unwrap().lines().next().unwrap().to_string();
        assert_eq!(first_line(window), "Prüfungszeitraum");
        assert_eq!(first_line(deadline), "Abgabe · Hausarbeit Datenbanken (bis 24:00)");
        assert_eq!(first_line(day_only), "Prüfung · Mündliche Prüfung Datenbanken (Uhrzeit offen)");
        for whole in [window, deadline, day_only] {
            assert!(whole.transparent && !whole.tentative, "{}", whole.uid);
            assert_eq!(whole.location.as_deref(), Some("Audimax"));
        }
        // A block without times: its days, whole, every day from Saturday to Sunday.
        let rows = rows();
        assert_eq!(series(&c, &rows[7]).repeat, Some(Repeat { rule: Rule::Days, until: d("2026-11-15"), except: vec![] }));
        let block = of_event(&c, "40");
        let days: Vec<When> = block.iter().map(|entry| entry.when).collect();
        assert_eq!(
            days,
            [
                When::AllDay { first: d("2026-11-14"), last: d("2026-11-14") },
                When::AllDay { first: d("2026-11-15"), last: d("2026-11-15") }
            ]
        );
        assert!(block.iter().all(|entry| entry.transparent && !entry.tentative));
        assert!(block[0].description.as_deref().unwrap().starts_with("Blockseminar 40\nModul 11111 Softwaretechnik\n"));
        assert!(block[0].description.as_deref().unwrap().contains("\nBlockveranstaltung 14.11.2026–15.11.2026\n"));
        assert_eq!(block[0].summary, "Sem SWT");
        assert_eq!(block[0].categories, ["Seminar"]);
        assert_eq!(block[0].location.as_deref(), Some("Zentralcampus Cottbus"), "no room: the campus");

        // Left out: a hidden event, a hidden Termin, an unclear time, a pattern without a day, an
        // open exam date and a hidden exam sitting.
        for event in ["50", "60", "61", "62", "74", "77"] {
            assert!(of_event(&c, event).is_empty(), "{event}");
        }
        assert_eq!(of_event(&c, "70").len(), 2);
    }

    #[test]
    fn a_date_tells_what_does_not_take_place() {
        let rows = rows();
        let c = calendar(&semester());
        // Saturdays: the break, Reformationstag, a date QIS cancels; a room note on its day.
        assert_eq!(of_event(&c, "35").len(), 13);
        let lines = "Übung\nModul 11111 Softwaretechnik\nwöchentlich 10.10.2026–30.01.2027\nLehrende: Robel\nEntfällt: \
                     26.12., 02.01. (vorlesungsfrei); 31.10. (Reformationstag); 07.11. (laut QIS)";
        let first = on(&c, &rows[3], "2026-10-10");
        assert_eq!(first.description.as_deref(), Some(format!("{lines}\nQuelle: QIS").as_str()));
        let noted = on(&c, &rows[3], "2026-11-14");
        assert_eq!(noted.description.as_deref(), Some(format!("{lines}\nHinweis: Raumwechsel\nQuelle: QIS").as_str()));
        for gone in ["2026-10-31", "2026-11-07", "2026-12-26"] {
            assert_eq!(find_on(&c, &rows[3], gone), None, "{gone}");
        }
        // In the series the dates that are not held are left out, and the noted one differs.
        let saturdays = series(&c, &rows[3]);
        let except = ["2026-10-31", "2026-11-07", "2026-12-26", "2027-01-02"].map(d).to_vec();
        assert_eq!(saturdays.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-30"), except }));
        assert_eq!(saturdays.overrides, [noted]);
        // An event of both modules names both, by id, its title too.
        let shared = of_event(&c, "80");
        assert_eq!(shared.len(), 15);
        let description = shared[0].description.as_deref().unwrap();
        assert!(description.starts_with("Tutorium · Gemeinsam\nModul 11111 Softwaretechnik\nModul 22222 Datenbanken\n"), "{description}");
        assert_eq!(shared[0].summary, "Tut SWT/DB");
    }

    /// The browser holds the modules in plan order, a code in ascending order: the same calendar.
    #[test]
    fn the_order_of_the_plan_changes_nothing() {
        let forward = calendar(&semester());
        let backward = calendar(&build(&rows(), &exams(), &[B, A], &two_sws(), &selection()));
        assert_eq!(forward, backward);
        assert_eq!(ics::write(&forward), ics::write(&backward));
    }

    #[test]
    fn the_calendar_names_its_semester_and_date() {
        let t = semester();
        let c = calendar(&t);
        assert_eq!(c.name, "Studienplan WiSe 2026/27");
        assert_eq!(c.stamp, "20260923T123516Z");
        assert_eq!(c.description, "Betula (inoffiziell) · Termine laut QIS, Stand 23.09.2026");
        assert_eq!(
            calendar_of(&t, &titles(), &names(), "", "20260923T123516Z", Locale::De),
            c,
            "a DTSTAMP is kept, the key names the semester"
        );
        let unknown = calendar_of(&t, &titles(), &names(), "WiSe 2026/27", "gestern", Locale::De);
        assert_eq!((unknown.stamp.as_str(), unknown.description.as_str()), (EPOCH_STAMP, crate::i18n::texts(Locale::De).feed_about));
        // A snapshot changed late in the evening is of the next day in Cottbus.
        let late = calendar_of(&t, &titles(), &names(), "WiSe 2026/27", "2026-09-23T22:30:00Z", Locale::De);
        assert_eq!(late.description, "Betula (inoffiziell) · Termine laut QIS, Stand 24.09.2026");
        let text = ics::write(&c);
        assert!(text.starts_with("BEGIN:VCALENDAR\r\n") && text.ends_with("END:VCALENDAR\r\n"));
        assert!(text.contains("\r\nX-WR-CALNAME:Studienplan WiSe 2026/27\r\n"));
        assert!(text.contains("\r\nX-WR-CALDESC:Betula (inoffiziell) · Termine laut QIS\\, Stand 23.09.2026\r\n"));
        assert_eq!(text.matches("BEGIN:VEVENT\r\n").count(), vevents(&c));
    }

    /// The feed of `/en/calendar/<code>.ics`: the same entries with the same UIDs, the words around
    /// the data in English, and what QIS says as QIS says it.
    #[test]
    fn the_calendar_in_english() {
        let rows = rows();
        let exams = exams();
        let t = semester();
        let de = calendar_of(&t, &titles(), &names(), "", "2026-09-23T12:35:16Z", Locale::De);
        let c = calendar_of(&t, &titles(), &names(), "", "2026-09-23T12:35:16Z", Locale::En);
        assert_eq!(c.name, "Timetable Winter 2026/27");
        assert_eq!(c.description, "Betula (unofficial) · dates according to QIS, as of 23 Sep 2026");
        let uids = |c: &Calendar| c.entries.iter().map(|entry| (entry.uid.clone(), entry.when, entry.tentative)).collect::<Vec<_>>();
        assert_eq!(uids(&c), uids(&de), "a subscription that changes its language updates its entries");

        // The lecture: „Vorlesung" is what QIS calls it, the rest is English.
        let lecture = on(&c, &rows[0], "2026-10-06");
        assert_eq!((lecture.summary.as_str(), lecture.categories.as_slice()), ("Lec SWT", ["Lecture".to_string()].as_slice()));
        assert_eq!(
            lecture.description.as_deref(),
            Some(
                "Vorlesung\nModule 11111 Softwaretechnik\nweekly 6 Oct 2026–26 Jan 2027\nRooms: Zentrales \
                 Hörsaalgebäude - Hörsaal A - Zentralcampus / Zentrales Hörsaalgebäude - Hörsaal B - \
                 Zentralcampus\nCancelled: 22 Dec, 29 Dec (no lectures)\nSource: QIS"
            )
        );
        // An open choice, and a range QIS does not state.
        let options = of_event(&c, "20");
        let option = &options[0];
        assert_eq!(option.summary, "Ex DB · 1 of 2");
        assert!(option.description.as_deref().unwrap().contains("\nOne of 2 exercise groups; choose in Betula\n"));
        assert_eq!(
            of_event(&c, "30")[0].description.as_deref(),
            Some(
                "Seminar 30\nModule 11111 Softwaretechnik\nweekly\nPeriod not stated in QIS; assumed: lecture period \
                 5 Oct 2026–31 Jan 2027\nCancelled: 24 Dec, 31 Dec (no lectures)\nSource: QIS"
            )
        );
        // The Saturdays: the break, a holiday, a date QIS cancels without a reason, a room note.
        let lines = "Übung\nModule 11111 Softwaretechnik\nweekly 10 Oct 2026–30 Jan 2027\nLecturers: Robel\nCancelled: \
                     26 Dec, 2 Jan (no lectures); 31 Oct (Reformation Day); 7 Nov (according to QIS)";
        let noted = on(&c, &rows[3], "2026-11-14");
        assert_eq!(noted.description.as_deref(), Some(format!("{lines}\nNote: Raumwechsel\nSource: QIS").as_str()));

        // Exams, and the whole days of a window, a deadline and a day without a time.
        let summary = |row: &DateRow| entry(&c, &exam_uid(row)).summary.clone();
        assert_eq!(
            [&exams[0], &exams[1], &exams[6], &exams[2], &exams[3], &exams[4]].map(summary),
            ["Exam SWT", "Exam SWT · 2nd sitting", "Exam DB · Retake", "Exam period DB", "Submission DB", "Exam DB"]
        );
        let first = entry(&c, &exam_uid(&exams[0]));
        assert_eq!(first.categories, ["Exam"]);
        assert_eq!(
            first.description.as_deref(),
            Some("Exam\nModule 11111 Softwaretechnik\nRoom: Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus\nSource: QIS")
        );
        let first_line = |row: &DateRow| entry(&c, &exam_uid(row)).description.as_deref().unwrap().lines().next().unwrap().to_string();
        assert_eq!(first_line(&exams[3]), "Submission · Hausarbeit Datenbanken (by 24:00)");
        assert_eq!(first_line(&exams[4]), "Exam · Mündliche Prüfung Datenbanken (time to be announced)");
        // A block without a room is at its campus, in English.
        assert_eq!(of_event(&c, "40")[0].location.as_deref(), Some("Central Campus Cottbus"));

        // Longer than the German, the description is folded: read it unfolded.
        let text = ics::write(&c).replace("\r\n ", "");
        assert!(text.contains("\r\nX-WR-CALNAME:Timetable Winter 2026/27\r\n"));
        assert!(text.contains("\r\nX-WR-CALDESC:Betula (unofficial) · dates according to QIS\\, as of 23 Sep 2026\r\n"));

        // A semester without dates says so in English, and a name given is kept.
        let summer = SemesterFacts::derive(SemesterKey::parse("2027S").unwrap(), None, &[]);
        let empty = build_in(&summer, &[], &[], &[A], &[], &Selection::default());
        let c = calendar_of(&empty, &titles(), &names(), "Summer 2027", "2026-09-23T12:35:16Z", Locale::En);
        assert_eq!(c.name, "Timetable Summer 2027");
        assert_eq!(c.description, "Betula (unofficial) · dates according to QIS, as of 23 Sep 2026 · No dates published yet");
    }

    #[test]
    fn days_in_cottbus() {
        let day = |stamp: &str| berlin_day(stamp).map(Day::iso);
        assert_eq!(day("20260923T123516Z").as_deref(), Some("2026-09-23"));
        assert_eq!(day("20260923T215959Z").as_deref(), Some("2026-09-23"), "23:59 in summer");
        assert_eq!(day("20260923T220000Z").as_deref(), Some("2026-09-24"));
        assert_eq!(day("20261231T225959Z").as_deref(), Some("2026-12-31"), "23:59 in winter");
        assert_eq!(day("20261231T230000Z").as_deref(), Some("2027-01-01"));
        // The switches, at 01:00 UTC: 2027-03-28 and 2026-10-25.
        assert_eq!(day("20270327T225959Z").as_deref(), Some("2027-03-27"));
        assert_eq!(day("20270328T220000Z").as_deref(), Some("2027-03-29"), "summer time since 01:00 UTC");
        assert_eq!(day("20261024T220000Z").as_deref(), Some("2026-10-25"), "still summer time");
        assert_eq!(day("20261025T225959Z").as_deref(), Some("2026-10-25"), "winter time again");
        assert_eq!((last_sunday(2026, 3), last_sunday(2026, 10)), (Some(d("2026-03-29")), Some(d("2026-10-25"))));
        assert_eq!((last_sunday(2027, 3), last_sunday(2027, 10)), (Some(d("2027-03-28")), Some(d("2027-10-31"))));
        assert_eq!(dtstamp(" 2026-09-23T12:35:16Z "), "20260923T123516Z");
        for bad in ["", "20260923T123516", "20260230T123516Z", "20260923T243516Z", "20260923X123516Z", "gestern"] {
            assert_eq!(utc_moment(bad), None, "{bad}");
            assert_eq!(dtstamp(bad), EPOCH_STAMP, "{bad}");
        }
    }

    #[test]
    fn the_stamp_of_a_snapshot() {
        let meta = |changed: Option<&str>, built: Option<&str>| Meta {
            built_at: built.map(Into::into),
            data_changed_at: changed.map(Into::into),
            current_semester: None,
            content_digest: None,
            radix_version: None,
        };
        let stamp = snapshot_stamp(&meta(Some("2026-09-23T12:35:16Z"), Some("2026-09-24T01:00:00Z")));
        assert_eq!(stamp, "20260923T123516Z");
        assert_eq!(snapshot_stamp(&meta(Some("kaputt"), Some("2026-09-24T01:00:00Z"))), "20260924T010000Z");
        assert_eq!(snapshot_stamp(&meta(None, None)), EPOCH_STAMP);
    }

    #[test]
    fn a_semester_without_data_is_an_empty_calendar() {
        let summer = SemesterFacts::derive(SemesterKey::parse("2027S").unwrap(), None, &[]);
        let t = build_in(&summer, &[], &[], &[A], &[], &Selection::default());
        let c = calendar_of(&t, &titles(), &names(), "SoSe 2027", "2026-09-23T12:35:16Z", Locale::De);
        assert!(c.entries.is_empty());
        assert_eq!(
            c.description,
            "Betula (inoffiziell) · Termine laut QIS, Stand 23.09.2026 · Noch keine Termine veröffentlicht"
        );
        let text = ics::write(&c);
        assert!(text.starts_with("BEGIN:VCALENDAR\r\n") && !text.contains("BEGIN:VEVENT"));
        assert_eq!(text.matches("BEGIN:VTIMEZONE").count(), 1);
        // Everything hidden in a semester with data: no entries, but no such note either.
        let hidden = Selection { hidden_kinds: KindSet(0x0fff), ..Selection::default() };
        let c = calendar(&build(&rows(), &exams(), &[A, B], &two_sws(), &hidden));
        assert!(c.entries.is_empty());
        assert!(!c.description.contains("Noch keine"), "{}", c.description);
    }

    #[test]
    fn a_calendar_keeps_its_earliest_entries() {
        // 14 blocks of the 400 days one row walks at most, less the 14 holidays among them: a
        // series each, every day, the holidays left out.
        let blocks: Vec<Fixture> = (0..14).map(|n| block(&(900 + n).to_string(), "2026-10-01", "2028-12-31")).collect();
        let t = build(&blocks, &[], &[A], &[], &Selection::default());
        let days = &t.events[0].rows[0].occ.days;
        assert_eq!(days.len(), 386);
        let c = calendar(&t);
        assert_eq!((c.entries.len(), vevents(&c)), (14, 14));
        let repeat = c.entries[0].repeat.as_ref().unwrap();
        assert_eq!((repeat.rule, Some(repeat.until), repeat.except.len()), (Rule::Days, days.last().copied(), 14));
        assert!(!c.description.contains("nur die ersten"), "{}", c.description);

        // With a room note of its own on every day, each date but the first differs from its
        // series: 386 VEVENTs a block, and a thirteenth block would pass 5,000.
        let notes: String = (0..400).map(|n| format!("{}: Raumwechsel {n} ", d("2026-10-01").plus(n).german())).collect();
        let noted: Vec<Fixture> = blocks
            .into_iter()
            .map(|mut block| {
                block.0.cancelled_dates = Some(notes.clone());
                block
            })
            .collect();
        let t = build(&noted, &[], &[A], &[], &Selection::default());
        let c = calendar(&t);
        assert_eq!((c.entries.len(), vevents(&c)), (12, 12 * 386));
        assert!(c.entries.iter().all(|entry| entry.overrides.len() == 385 && Some(day_of(entry)) == days.first().copied()));
        assert!(c.description.ends_with(", Stand 23.09.2026 · nur die ersten 5000 Einträge"), "{}", c.description);
        let english = calendar_of(&t, &titles(), &names(), "", "2026-09-23T12:35:16Z", Locale::En).description;
        assert!(english.ends_with(", as of 23 Sep 2026 · only the first 5,000 entries"), "{english}");
    }

    #[test]
    fn a_subscription_keeps_what_the_timetable_has() {
        let rows = rows();
        let t = semester();
        let selection = Selection {
            hidden_kinds: KindSet::default().with(EventKind::Tutorial),
            hidden_events: [50, 999_999].into(),
            hidden_rows: [rows[9].key(), RowKey { event: 999, fp: 1 }, hidden_exam()].into(),
            chosen_rows: [rows[5].key(), RowKey { event: 20, fp: 0x12345 }].into(),
            hidden_dates: [(rows[5].key(), d("2026-11-04")), (RowKey { event: 999, fp: 1 }, d("2026-11-04"))].into(),
            town: TownChoice::Only(Town::Senftenberg),
            town_from: None,
        };
        let modules = ids(&[B, "FÜS", A, B, "0123", "FÜS"]);
        let (subscription, other) = subscription_of(t.key, &modules, Some("079-82-2008"), &selection, Some(&t));
        let mut hidden = vec![rows[9].key().packed(), hidden_exam().packed()];
        hidden.sort_unstable();
        assert_eq!(
            subscription,
            Subscription {
                semester: 4053,
                modules: vec![11111, 22222],
                program: Some("079-82-2008".to_string()),
                hidden_kinds: EventKind::Tutorial.bit(),
                hidden_events: vec![50],
                hidden_rows: hidden,
                chosen_rows: vec![rows[5].key().packed()],
                town: 2,
                hidden_dates: vec![date_value(t.key, rows[5].key(), d("2026-11-04")).unwrap()],
            }
        );
        assert_eq!(other, ["FÜS", "0123"]);
        assert_eq!(Subscription::from_code(&subscription.code().unwrap()), Some(subscription));
        // Without a timetable, everything the selection holds.
        let (all, _) = subscription_of(t.key, &modules, None, &selection, None);
        assert_eq!(all.hidden_events, [50, 999_999]);
        assert_eq!((all.hidden_rows.len(), all.chosen_rows.len(), all.hidden_dates.len()), (3, 2, 2));
        // A program id of another shape is left out, as a reader leaves it out.
        let (odd, _) = subscription_of(t.key, &modules, Some("079_82_2008"), &selection, Some(&t));
        assert_eq!(odd.program, None);
        // Only ids that are no numbers: nothing a code can carry.
        let (none, other) = subscription_of(t.key, &ids(&["FÜS"]), None, &Selection::default(), Some(&t));
        assert_eq!((none.modules.len(), other), (0, vec!["FÜS".to_string()]));
        assert_eq!(none.code(), Err(folia_pack::Error::Malformed));
    }

    #[test]
    fn a_town_derived_from_the_import_is_written_as_it_is() {
        let with_tracks = |town: Option<Town>| Timetable { tracks: [A.to_string()].into(), town, town_derived: town.is_some(), ..semester() };
        let imported = Selection { town_from: Some([A.to_string()].into()), ..Selection::default() };
        let town = |selection: &Selection, t: &Timetable| subscription_of(t.key, &ids(&[A, B]), None, selection, Some(t)).0.town;
        // The page derived it from the imported modules: the feed must not derive it anew.
        assert_eq!(town(&imported, &with_tracks(Some(Town::Cottbus))), TownChoice::Only(Town::Cottbus).code());
        assert_eq!(town(&imported, &with_tracks(None)), TownChoice::Both.code());
        // Derived from every module, or nothing to decide: „derive", as the feed does it too.
        assert_eq!(town(&Selection::default(), &with_tracks(Some(Town::Cottbus))), 0);
        assert_eq!(town(&imported, &semester()), 0);
        // A chosen town is the choice.
        let chosen = Selection { town: TownChoice::Only(Town::Senftenberg), ..imported.clone() };
        assert_eq!(town(&chosen, &with_tracks(Some(Town::Cottbus))), TownChoice::Only(Town::Senftenberg).code());
    }

    #[test]
    fn a_subscription_is_stale_only_when_the_plan_changed() {
        let rows = rows();
        let t = semester();
        let modules = ids(&[A, B]);
        let informatik = Some("079-82-2008");
        let (current, _) = subscription_of(t.key, &modules, informatik, &selection(), Some(&t));
        let stored = current.code().unwrap();
        assert!(same_subscription(&stored, &current, &t));

        // QIS removes the hidden Termin's row and the hidden event: the plan is the same.
        let fewer: Vec<Fixture> =
            rows.iter().filter(|row| !["50", "60"].contains(&row.0.date.event_id.as_str())).cloned().collect();
        let now = build(&fewer, &exams(), &[A, B], &two_sws(), &selection());
        let (after, _) = subscription_of(now.key, &modules, informatik, &selection(), Some(&now));
        assert_eq!((after.hidden_events.len(), after.hidden_rows.len()), (0, 1), "the new code is shorter");
        assert!(same_subscription(&stored, &after, &now));
        // A chosen row QIS no longer has, kept in a code made without the timetable: the same.
        let with_stale = Selection { chosen_rows: [RowKey { event: 20, fp: 0x12345 }].into(), ..selection() };
        let (all, _) = subscription_of(t.key, &modules, informatik, &with_stale, None);
        assert!(same_subscription(&all.code().unwrap(), &current, &t));

        // A module added, a Termin hidden, a choice made, a kind, the town, an event: stale.
        let (more, _) = subscription_of(t.key, &ids(&[A, B, "33333"]), informatik, &selection(), Some(&t));
        assert!(!same_subscription(&stored, &more, &t));
        // Another program, or none: the feed would name the modules otherwise.
        for program in [Some("C38-82-2024"), None] {
            let (other, _) = subscription_of(t.key, &modules, program, &selection(), Some(&t));
            assert!(!same_subscription(&stored, &other, &t), "{program:?}");
        }
        let changes = [
            Selection { hidden_rows: [rows[9].key(), rows[0].key()].into(), ..selection() },
            Selection { chosen_rows: [rows[4].key()].into(), ..selection() },
            Selection { hidden_kinds: KindSet::default().with(EventKind::Seminar), ..selection() },
            Selection { town: TownChoice::Both, ..selection() },
            Selection { hidden_events: [50, 30].into(), ..selection() },
        ];
        for changed in changes {
            let (changed_now, _) = subscription_of(t.key, &modules, informatik, &changed, Some(&t));
            assert!(!same_subscription(&stored, &changed_now, &t), "{changed:?}");
        }
        // Another semester, and codes that do not decode.
        let later = Subscription { semester: 4054, ..current.clone() };
        assert!(!same_subscription(&later.code().unwrap(), &current, &t));
        assert!(!same_subscription("", &current, &t));
        assert!(!same_subscription("x", &current, &t));
    }

    /// Two rows of one Termin as QIS has them for 149467 and 148732: other teachers, rooms, ends
    /// and last dates, the first cancelled on a date the second holds, the second on a date
    /// neither holds, and a room note on a date both hold.
    #[test]
    fn rows_of_one_key_say_what_each_says() {
        let short = teaching(A, "90", 1, "Praktikum", 3, "10:00", "11:30").range("2026-10-07", "2027-01-06");
        let mut short = room(short, "HS 105");
        short.0.date.instructor = Some("Bleicher".into());
        short.0.date.comment = Some("Einweisung".into());
        short.0.cancelled_dates = Some("16.12.2026: MHB".into());
        let long = teaching(A, "90", 2, "Praktikum", 3, "10:00", "16:00").range("2026-10-07", "2027-01-27");
        let mut long = room(long, "Labor 213");
        long.0.date.instructor = Some("Freymann".into());
        long.0.cancelled_dates = Some("20.01.2027: Exkursion 21.10.2026: Raumwechsel HS 3".into());
        assert_eq!(short.key(), long.key());
        let c = calendar(&build(&[short.clone(), long], &[], &[A], &[], &Selection::default()));

        // 17 Wednesdays but the two in the break and the one neither row holds.
        assert_eq!(of_event(&c, "90").len(), 14);
        for gone in ["2026-12-23", "2026-12-30", "2027-01-20"] {
            assert_eq!(find_on(&c, &short, gone), None, "{gone}");
        }
        let dropped = "Entfällt: 23.12., 30.12. (vorlesungsfrei); 20.01. (Exkursion)";
        let both = on(&c, &short, "2026-10-07");
        assert_eq!(both.when, When::Timed { day: d("2026-10-07"), from: 600, to: 960 });
        assert_eq!(both.location.as_deref(), Some("HS 105 / Labor 213"));
        let both_lines = format!(
            "Praktikum 90\nModul 11111 Softwaretechnik\nwöchentlich 07.10.2026–27.01.2027\nLehrende: Bleicher / \
             Freymann\nEinweisung\n{dropped}"
        );
        assert_eq!(both.description.as_deref(), Some(format!("{both_lines}\nQuelle: QIS").as_str()));
        let noted = on(&c, &short, "2026-10-21");
        assert_eq!(
            noted.description.as_deref(),
            Some(format!("{both_lines}\nHinweis: Raumwechsel HS 3\nQuelle: QIS").as_str())
        );

        // A date only the second row holds: its room, its end, its teacher; the series as above.
        let alone = format!(
            "Praktikum 90\nModul 11111 Softwaretechnik\nwöchentlich 07.10.2026–27.01.2027\nLehrende: Freymann\n\
             {dropped}\nQuelle: QIS"
        );
        for day in ["2026-12-16", "2027-01-13"] {
            let only = on(&c, &short, day);
            assert_eq!(only.when, When::Timed { day: d(day), from: 600, to: 960 }, "{day}");
            assert_eq!(only.location.as_deref(), Some("Labor 213"), "{day}");
            assert_eq!(only.description.as_deref(), Some(alone.as_str()), "{day}");
        }

        // One series: what both rows say on the dates they both hold, and after it, in full, the
        // date with the note and the three only the second row holds.
        let termin = series(&c, &short);
        let except = ["2026-12-23", "2026-12-30", "2027-01-20"].map(d).to_vec();
        assert_eq!(termin.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-27"), except }));
        assert_eq!((termin.when, &termin.description, &termin.location), (both.when, &both.description, &both.location));
        let changed: Vec<Day> = termin.overrides.iter().map(|date| date.when.day()).collect();
        assert_eq!(changed, ["2026-10-21", "2026-12-16", "2027-01-13", "2027-01-27"].map(d));
    }

    /// A single date of QIS: on `day` at 13:45, in room A, of the series Radix names by `first`
    /// (`None`: of no series).
    fn single(event: &str, ord: i64, day: &str, series: Option<(i64, u8)>) -> Fixture {
        let weekday = i64::from(d(day).weekday());
        let mut row = room(teaching(A, event, ord, "Seminar", weekday, "13:45", "15:15").rhythm("single").range(day, day), "Raum A");
        row.0.date.rhythm_raw = Some("Einzel".into());
        row.0.date.series = series.map(|(first, weeks)| Series { first, weeks });
        row
    }

    /// Single dates Radix reads as one series (`Row::series`): one entry under the key of the row
    /// that names it, the weeks it has no date in left out and named, a date in another room
    /// after it in full; a date that is hidden or cancelled is left out as well.
    #[test]
    fn single_dates_of_a_series_are_one_series() {
        let weekly = Some((2, 1));
        let mut moved = single("95", 4, "2026-11-03", weekly);
        moved.0.date.room = Some("Raum B".into());
        let mut ill = single("95", 8, "2026-12-01", weekly);
        ill.0.cancelled_dates = Some("01.12.2026: Krankheit".into());
        let hidden = single("95", 7, "2026-11-24", weekly);
        let rows = vec![
            // Listed by QIS in any order: the series is named by the row of its first date.
            single("95", 1, "2026-10-20", weekly),
            single("95", 2, "2026-10-13", weekly),
            single("95", 3, "2026-10-27", weekly),
            moved,
            // No date on 10.11.: a week of no date in QIS.
            single("95", 5, "2026-11-17", weekly),
            hidden.clone(),
            ill,
            single("95", 9, "2026-12-08", weekly),
            single("95", 10, "2026-12-15", weekly),
            // The break: 22.12. and 29.12.
            single("95", 11, "2027-01-05", weekly),
            // Every other Thursday, and a date of no series.
            single("96", 1, "2026-10-15", Some((1, 2))),
            single("96", 2, "2026-10-29", Some((1, 2))),
            single("96", 3, "2026-11-12", Some((1, 2))),
            single("97", 1, "2026-10-14", None),
            // Every three weeks.
            single("98", 1, "2026-10-15", Some((1, 3))),
            single("98", 2, "2026-11-05", Some((1, 3))),
            single("98", 3, "2026-11-26", Some((1, 3))),
        ];
        let selection = Selection { hidden_rows: [hidden.key()].into(), ..Selection::default() };
        let c = calendar(&build(&rows, &[], &[A], &[], &selection));

        let termin = series(&c, &rows[1]);
        let except = ["2026-11-10", "2026-11-24", "2026-12-01", "2026-12-22", "2026-12-29"].map(d).to_vec();
        assert_eq!(termin.when, When::Timed { day: d("2026-10-13"), from: 825, to: 915 });
        assert_eq!(termin.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-05"), except }));
        assert_eq!((termin.summary.as_str(), termin.location.as_deref()), ("Sem SWT", Some("Raum A")));
        assert_eq!(
            termin.description.as_deref(),
            Some(
                "Seminar 95\nModul 11111 Softwaretechnik\nwöchentlich 13.10.2026–05.01.2027 (in QIS als Einzeltermine)\n\
                 Entfällt: 22.12., 29.12. (vorlesungsfrei); 01.12. (Krankheit); 10.11. (kein Termin in QIS)\nQuelle: QIS"
            )
        );
        let other_room = Entry { when: When::Timed { day: d("2026-11-03"), from: 825, to: 915 }, location: Some("Raum B".into()), repeat: None, overrides: vec![], ..termin.clone() };
        assert_eq!(termin.overrides, [other_room]);
        assert_eq!(of_event(&c, "95").len(), 8);
        assert_eq!(c.entries.iter().filter(|entry| entry.uid.starts_with("95-")).count(), 1);

        // Every other week, every three weeks; and in English.
        let fortnightly = series(&c, &rows[10]);
        assert_eq!(fortnightly.repeat, Some(Repeat { rule: Rule::Weeks(2), until: d("2026-11-12"), except: vec![] }));
        assert!(fortnightly.description.as_deref().unwrap().contains("\n14-täglich 15.10.2026–12.11.2026 (in QIS als Einzeltermine)\n"));
        let third = series(&c, &rows[14]);
        assert_eq!(third.repeat, Some(Repeat { rule: Rule::Weeks(3), until: d("2026-11-26"), except: vec![] }));
        assert!(third.description.as_deref().unwrap().contains("\nalle drei Wochen 15.10.2026–26.11.2026 (in QIS als Einzeltermine)\n"));
        assert!(ics::write(&c).contains("\r\nRRULE:FREQ=WEEKLY;INTERVAL=3;UNTIL=20261126T225959Z\r\n"));
        let english = calendar_of(&build(&rows, &[], &[A], &[], &selection), &titles(), &names(), "", "2026-09-23T12:35:16Z", Locale::En);
        let lines = series(&english, &rows[1]).description.clone().unwrap();
        assert!(lines.contains("\nweekly 13 Oct 2026–5 Jan 2027 (single dates in QIS)\n"), "{lines}");
        assert!(lines.contains("Cancelled: 22 Dec, 29 Dec (no lectures); 1 Dec (Krankheit); 10 Nov (no date in QIS)"), "{lines}");

        // A date of no series is an entry of its own, under its key and its date, as ever.
        let lone = entry(&c, &format!("{}-20261014@betula.app", rows[13].key().text()));
        assert_eq!((lone.repeat.as_ref(), lone.when.day()), (None, d("2026-10-14")));
        assert!(lone.description.as_deref().unwrap().contains("\nEinzeltermin\n"));
    }

    /// Dates that their series' rule does not make (a series of every other week whose dates are a
    /// week apart): an entry each, as without a series.
    #[test]
    fn dates_no_rule_makes_stay_dates() {
        let rows: Vec<Fixture> = ["2026-10-13", "2026-10-20", "2026-10-27"]
            .into_iter()
            .zip(1..)
            .map(|(day, ord)| single("95", ord, day, Some((1, 2))))
            .collect();
        let c = calendar(&build(&rows, &[], &[A], &[], &Selection::default()));
        let uids: Vec<&str> = c.entries.iter().map(|entry| entry.uid.as_str()).collect();
        let own: Vec<String> = rows.iter().zip(["20261013", "20261020", "20261027"]).map(|(row, day)| format!("{}-{day}@betula.app", row.key().text())).collect();
        assert_eq!(uids, own);
        assert!(c.entries.iter().all(|entry| entry.repeat.is_none()));
    }

    /// Informatik B.Sc., FS1 in WiSe 2026/27, with the pinned subscription's selection: the
    /// Termine the design names. On any snapshot: the same calendar in plan order and by id, and
    /// every UID once.
    #[test]
    fn the_calendar_of_informatik_first_semester() {
        let pinned = folia_test_support::studyplan_db("the_calendar_of_informatik_first_semester");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        let meta = queries::meta(&db).unwrap();
        let semester = if is_pinned { "2026W".to_string() } else { meta.current_semester.clone().unwrap() };
        // The pinned code of folia-calendar's subscription.rs: Informatik's abbreviations, 149408 hidden, „Nur
        // diesen" on 148369-a4d12.
        let code = "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0";
        let subscription = Subscription::from_code(code).unwrap();
        let selection = subscription.selection();
        let plan = ids(&["12104", "12107", "12102", "11112"]);
        let titles: BTreeMap<String, String> = [
            ("12104", "Entwicklung von Softwaresystemen"),
            ("12107", "Elektrische und elektronische Grundlagen der Informatik"),
            ("12102", "Programmierpraktikum"),
            ("11112", "Mathematik IT-1 (Diskrete Mathematik)"),
        ]
        .into_iter()
        .map(|(id, title)| (id.to_string(), title.to_string()))
        .collect();
        // Their abbreviations in Informatik B.Sc. (079-82-2008), as the page's slots show them.
        let names: BTreeMap<String, String> =
            [("12104", "EvS"), ("12107", "EEG"), ("12102", "PP"), ("11112", "MIT1")].into_iter().map(|(id, name)| (id.to_string(), name.to_string())).collect();
        let stamp = snapshot_stamp(&meta);
        let in_plan = planned(&db, &semester, &plan, &selection);
        let by_id = planned(&db, &semester, &subscription.module_ids(), &selection);
        let browser = ics::write(&calendar_of(&in_plan, &titles, &names, "", &stamp, Locale::De));
        let feed = ics::write(&calendar_of(&by_id, &titles, &names, "", &stamp, Locale::De));
        assert_eq!(browser, feed, "the download is the feed");
        // Every UID once, and every date of a series that differs once under it.
        let named: Vec<(&str, Option<&str>)> = feed
            .split("BEGIN:VEVENT\r\n")
            .skip(1)
            .map(|event| {
                let property = |name: &str| event.split("\r\n").find_map(|line| line.strip_prefix(name));
                (property("UID:").unwrap_or_default(), property("RECURRENCE-ID;"))
            })
            .collect();
        assert_eq!(named.iter().collect::<BTreeSet<_>>().len(), named.len(), "every UID once");
        if !is_pinned {
            return;
        }

        assert_eq!(stamp, "20260925T083015Z");
        let (of_plan, _) = subscription_of(in_plan.key, &plan, Some("079-82-2008"), &selection, Some(&in_plan));
        assert_eq!(of_plan.code().as_deref(), Ok(code));
        let c = calendar_of(&in_plan, &titles, &names, "", &stamp, Locale::De);
        assert_eq!(c.name, "Studienplan WiSe 2026/27");
        // The stand is the build's, which wrote the short names on 25.09. (the data is of 23.09.).
        assert_eq!(c.description, "Betula (inoffiziell) · Termine laut QIS, Stand 25.09.2026");
        // The 249 dates the calendar had as an entry each until 2026-10-13.
        assert_eq!(dated(&c).len(), 249);
        // 12104's Tuesday lecture, as C.14 shows it: every Tuesday to 26.01., the break left out.
        let lecture = entry(&c, "148701-a2633@betula.app");
        assert_eq!(lecture.when, When::Timed { day: d("2026-10-13"), from: 690, to: 780 });
        let except = vec![d("2026-12-22"), d("2026-12-29")];
        assert_eq!(lecture.repeat, Some(Repeat { rule: Rule::Weeks(1), until: d("2027-01-26"), except }));
        assert_eq!(lecture.summary, "VL EvS");
        assert_eq!(lecture.location.as_deref(), Some("ZHG/HS.C"));
        assert_eq!(
            lecture.description.as_deref(),
            Some(
                "Vorlesung\nModul 12104 Entwicklung von Softwaresystemen\nwöchentlich 13.10.2026–26.01.2027\nRaum: \
                 Zentrales Hörsaalgebäude - Hörsaal C - Zentralcampus\nEntfällt: 22.12., 29.12. (vorlesungsfrei)\n\
                 Quelle: QIS"
            )
        );
        assert!(feed.contains(
            "UID:148701-a2633@betula.app\r\nDTSTAMP:20260925T083015Z\r\n\
             DTSTART;TZID=Europe/Berlin:20261013T113000\r\nDTEND;TZID=Europe/Berlin:20261013T130000\r\n\
             RRULE:FREQ=WEEKLY;UNTIL=20270126T225959Z\r\nEXDATE;TZID=Europe/Berlin:20261222T113000\r\n\
             EXDATE;TZID=Europe/Berlin:20261229T113000\r\nSUMMARY:VL EvS\r\n"
        ));
        // Senftenberg's course of 12104 and its exam are not the student's; 149408 is hidden.
        for gone in ["UID:149406-", "UID:149407-", "UID:150664-", "UID:149408-", "UID:148369-aaf38", "20151227"] {
            assert!(!feed.contains(gone), "{gone}");
        }
        // „Nur diesen" on Monday 17:30: its 16 Mondays from 12.10. but the two in the break, firm.
        let chosen = of_event(&c, "148369");
        assert_eq!(chosen.len(), 14);
        assert!(chosen.iter().all(|entry| entry.uid == "148369-a4d12@betula.app" && !entry.tentative));
        // 148304 is still open: three options, tentative.
        let open = of_event(&c, "148304");
        assert!(!open.is_empty() && open.iter().all(|entry| entry.tentative));
        assert!(open[0].description.as_deref().unwrap().contains("\nEine von 3 Übungsgruppen; in Betula wählen\n"));
        // The exams of 11112, 12104 and 12107, each once.
        let exam_days: Vec<Day> =
            c.entries.iter().filter(|entry| entry.categories == ["Prüfung"]).map(day_of).collect();
        assert_eq!(exam_days, [d("2027-02-11"), d("2027-03-10"), d("2027-03-12")]);
        assert!(feed.split("\r\n").all(|line| line.len() <= 75));
    }
}
