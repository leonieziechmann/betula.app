//! A timetable as calendar entries, and the subscription that describes it.
//!
//! The calendar a visitor downloads and the one a calendar service subscribes to are the same
//! text: the browser makes it from the page's own timetable, the server from the timetable it
//! builds anew for a code, and both call `calendar_of`. So nothing here may depend on what differs
//! between the two: the browser holds the modules in plan order, a code in ascending order.
//! Entries are sorted by their start and UID, and the modules of an entry are listed by id.
//!
//! Every held date is an entry of its own, with a UID made of the row key and the date. A date
//! QIS cancels, a break and a holiday are simply not there, and a room note sits on its date; no
//! client has to get RRULE, EXDATE and a time zone right together. The UID stays when QIS changes
//! a Termin's end or room, so a subscribed calendar updates the entry instead of adding a second.
//! Rows that share a key (the same slot in two rooms, or once more with a later end) are one entry
//! per date, from the earliest start to the latest end, in both rooms. Such rows may differ in who
//! teaches, in QIS's comment, in their last date and in what they drop, so an entry says what each
//! row held on its date says, the range spans all of them, and a date one row drops is named as
//! dropped only when no row of the key holds it.
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

use std::collections::{BTreeMap, BTreeSet};

use super::day::Day;
use super::exams::{Exam, ExamRow, ExamShape};
use super::ics::{self, Calendar, Entry, When};
use super::kind::{EventKind, KindSet};
use super::model::{Event, Row, Timetable};
use super::occur::{Every, Skipped};
use super::rowkey::{fingerprint, RowKey};
use super::select::{Selection, TownChoice};
use super::semester::SemesterKey;
use super::subscription::Subscription;
use super::views::{kind_and_title, kind_short, short_title, type_text};
use crate::exam_reading::Reason;
use crate::i18n::timetable::Texts;
use crate::i18n::Locale;
use crate::labels::{Code, Rhythm};
use crate::rows::Meta;
use crate::rows_detail::EventDate;
use crate::url::is_program_id;

/// The domain of every UID. The same for the download and the feed, so a calendar that holds both
/// holds each date once, and fixed: another domain would make every subscribed entry a new one.
pub const UID_DOMAIN: &str = "betula.app";

/// The most entries of one calendar, the earliest kept. A full semester of 60 modules stays far
/// below it; the cap only bounds what a hand-made code can ask of the server.
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

/// The calendar of a timetable: one entry per held date of every visible row with a time or of a
/// block of whole days, and per visible exam date that names a day.
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
    let texts = &locale.texts().timetable;
    let stamp = dtstamp(stamp);
    let names = Names { titles, short: names };
    let mut entries = Entries::default();
    for event in t.events.iter().filter(|event| event.hidden.is_none()) {
        teaching(&mut entries, t, event, &names, locale);
    }
    for exam in t.exams.iter().filter(|exam| exam.hidden.is_none()) {
        exam_dates(&mut entries, exam, &names, locale);
    }
    let mut entries = entries.finish(texts);
    entries.sort_by(|a, b| start(&a.when).cmp(&start(&b.when)).then_with(|| a.uid.cmp(&b.uid)));
    let cut = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);

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

impl Subscription {
    /// The subscription of a semester's plan as the page shows it. Only numeric module ids, and
    /// only hidden events and hidden or chosen rows that belong to `table` (all of them when
    /// `table` is None), so hide rules the timetable no longer needs do not lengthen the code.
    /// `program` is the one whose abbreviations the page names the modules by
    /// (`StudyplanData::program`); an id of another shape is left out, as a reader leaves it out.
    /// Second value: the ids that cannot be subscribed (not numeric), each once, in their order.
    pub fn of(
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
        };
        (subscription, other)
    }
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
        (
            s.semester,
            s.module_ids(),
            s.program.clone(),
            KindSet(s.hidden_kinds).known(),
            s.hidden_events.iter().copied().filter(|id| known.events.contains(id)).collect::<BTreeSet<u32>>(),
            rows(&s.hidden_rows),
            rows(&s.chosen_rows),
            TownChoice::from_code(s.town),
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

/// The entries of a visible teaching event: every held date of its visible rows. A row with an
/// unclear time is left out (its times say nothing a calendar can place), unless it is a block
/// without times, whose dates are whole days.
fn teaching(entries: &mut Entries, t: &Timetable, event: &Event, names: &Names, locale: Locale) {
    let texts = &locale.texts().timetable;
    // As its slot in the week: „VL EvS".
    let label = slot_text(&kind_short(event, locale), &names.short_of(&event.modules, &event.title));
    let what = names.what(&type_text(event, locale), &event.title, &event.modules);
    let categories: Vec<String> = event.kinds.iter().map(|kind| kind.label(locale).to_string()).collect();
    let open = event.unresolved();
    let placed = |row: &&Row| row.hidden.is_none() && (row.from.zip(row.to).is_some() || row.occ.all_day);
    // The rows of each key, in the order met: they are one entry per date, so what that entry
    // says of the whole Termin, its range and the dates it drops, is said of all of them.
    let mut keys: Vec<(String, Vec<&Row>)> = Vec::new();
    for row in event.rows.iter().filter(placed) {
        let key = key_text(&event.id, row.key, &row.date);
        match keys.iter_mut().find(|(seen, _)| *seen == key) {
            Some((_, rows)) => rows.push(row),
            None => keys.push((key, vec![row])),
        }
    }
    for (key, rows) in &keys {
        let termin = termin_lines(event, rows, what.as_deref(), names.titles, locale);
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
                    uid: uid(key, *day),
                    when,
                    summary: summary.clone(),
                    location: None,
                    description: None,
                    url: event.source_url.clone(),
                    categories: categories.clone(),
                    tentative: row.occ.assumed || option,
                    transparent: times.is_none(),
                };
                entries.add(entry, place.clone(), lines.iter().cloned().chain(notes).collect());
            }
        }
    }
}

/// What the entries of one Termin say of it, whichever of its rows holds a date: what it is, its
/// modules, its group, its rhythm and the range its rows span, and the dates none of them holds.
fn termin_lines(
    event: &Event,
    rows: &[&Row],
    what: Option<&str>,
    titles: &BTreeMap<String, String>,
    locale: Locale,
) -> Vec<Line> {
    let texts = &locale.texts().timetable;
    let group = rows
        .first()
        .and_then(|row| text(&row.date.group_name))
        .filter(|group| *group != UNNAMED_GROUP)
        .map(texts.feed_group);
    let head = module_lines(&event.modules, titles, texts).into_iter().chain(group).chain(rhythm_line(rows, locale));
    let what = what.map(|what| (Part::What, what.to_string()));
    what.into_iter()
        .chain(head.map(|line| (Part::Head, line)))
        .chain(dropped(rows, locale).map(|line| (Part::Dropped, line)))
        .collect()
}

/// What a teaching row says of itself on each of its dates: who teaches, QIS's comment, an
/// assumed range and an open choice.
fn row_lines(t: &Timetable, event: &Event, row: &Row, option: bool, locale: Locale) -> Vec<Line> {
    let texts = &locale.texts().timetable;
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
/// QIS writes it.
fn rhythm_line(rows: &[&Row], locale: Locale) -> Option<String> {
    let texts = &locale.texts().timetable;
    let date = &rows.first()?.date;
    let rhythm = date.rhythm.as_ref().and_then(Code::known);
    let name = match Every::of(date) {
        Some(Every::Week) => Some(texts.weekly.to_string()),
        Some(Every::AWeek) => Some(texts.fortnightly_a.to_string()),
        Some(Every::BWeek) => Some(texts.fortnightly_b.to_string()),
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
        .filter(|(first, last)| rhythm != Some(Rhythm::Single) && first != last)
        .map(|(first, last)| format!("{}–{}", first.date(locale), last.date(locale)));
    match (name, range) {
        (Some(name), Some(range)) => Some(format!("{name} {range}")),
        (name, range) => name.or(range),
    }
}

/// „Entfällt: 22.12., 29.12. (vorlesungsfrei); 31.10. (Reformationstag); 05.11. (laut QIS)": the
/// dates the rows of a Termin drop, the break first, then the holidays, then what QIS cancels, each
/// reason once with its dates. A date another row of the Termin holds is not dropped: the calendar
/// has an entry on it, in that row's room and times. A date two rows drop is named once, for the
/// first reason met. The reason QIS gives is its own text; where it gives none, „laut QIS".
fn dropped(rows: &[&Row], locale: Locale) -> Option<String> {
    let texts = &locale.texts().timetable;
    let skipped = || rows.iter().flat_map(|row| row.occ.skipped.iter().map(|(day, why)| (*day, *why)));
    let named_why = |(day, why): (Day, Skipped)| (day, why.text(locale));
    let mut breaks: Vec<(Day, &str)> = skipped().filter(|(_, why)| *why == Skipped::Break).map(named_why).collect();
    let mut holidays: Vec<(Day, &str)> = skipped().filter(|(_, why)| *why != Skipped::Break).map(named_why).collect();
    let mut cancelled: Vec<(Day, &str)> = rows
        .iter()
        .flat_map(|row| row.occ.cancelled.iter().map(|(day, note, _)| (*day, text(note).unwrap_or(texts.cancelled_by_qis))))
        .collect();
    // Each list by day, as one row's lists are; a stable sort keeps the first row's reason first.
    for list in [&mut breaks, &mut holidays, &mut cancelled] {
        list.sort_by_key(|(day, _)| *day);
    }
    let mut named: BTreeSet<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).collect();
    let mut groups: Vec<(&str, Vec<Day>)> = Vec::new();
    for (day, why) in breaks.into_iter().chain(holidays).chain(cancelled) {
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
    let texts = &locale.texts().timetable;
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
    use super::*;
    use crate::queries;
    use crate::rows_detail::{DateRow, ModuleSws};
    use crate::timetable::facts::SemesterFacts;
    use crate::timetable::model::tests::{d, ids, invariants, planned, sws, teaching, winter, Fixture};
    use crate::timetable::model::Input;
    use crate::timetable::select::Town;

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
        let input = Input { key: facts.key, semester: None, facts, modules: &modules, schedule: &schedule, exams, sws };
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

    fn entry<'c>(calendar: &'c Calendar, uid: &str) -> &'c Entry {
        calendar.entries.iter().find(|entry| entry.uid == uid).unwrap_or_else(|| panic!("no entry {uid}"))
    }

    fn of_event<'c>(calendar: &'c Calendar, event: &str) -> Vec<&'c Entry> {
        calendar.entries.iter().filter(|entry| entry.uid.starts_with(&format!("{event}-"))).collect()
    }

    fn uid_of(row: &Fixture, day: &str) -> String {
        format!("{}-{}@betula.app", row.key().text(), d(day).compact())
    }

    fn exam_uid(row: &DateRow) -> String {
        let first = d(row.date.first_date.as_deref().unwrap());
        format!("{}-{}@betula.app", RowKey::of(&row.date).unwrap().text(), first.compact())
    }

    fn day_of(entry: &Entry) -> Day {
        start(&entry.when).0
    }

    #[test]
    fn rows_of_one_key_are_one_entry_per_date() {
        let rows = rows();
        let c = calendar(&semester());
        let lecture = entry(&c, &uid_of(&rows[0], "2026-10-06"));
        assert_eq!(lecture.when, When::Timed { day: d("2026-10-06"), from: 690, to: 810 }, "until the later end");
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
        // 17 Tuesdays but the two in the break, each once.
        assert_eq!(of_event(&c, "10").len(), 15);

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

    #[test]
    fn what_may_not_happen_is_tentative() {
        let rows = rows();
        let c = calendar(&semester());
        // An open choice: both options' dates, tentative, with the prompt.
        let options = of_event(&c, "20");
        assert_eq!(options.len(), 30);
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
            (assumed.first().map(|e| day_of(e)), assumed.last().map(|e| day_of(e))),
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
        // A block without times: its days, whole.
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
        let first = entry(&c, &uid_of(&rows[3], "2026-10-10"));
        assert_eq!(first.description.as_deref(), Some(format!("{lines}\nQuelle: QIS").as_str()));
        let noted = entry(&c, &uid_of(&rows[3], "2026-11-14"));
        assert_eq!(noted.description.as_deref(), Some(format!("{lines}\nHinweis: Raumwechsel\nQuelle: QIS").as_str()));
        for gone in ["2026-10-31", "2026-11-07", "2026-12-26"] {
            assert!(c.entries.iter().all(|entry| entry.uid != uid_of(&rows[3], gone)), "{gone}");
        }
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
        assert_eq!((unknown.stamp.as_str(), unknown.description.as_str()), (EPOCH_STAMP, Locale::De.texts().timetable.feed_about));
        // A snapshot changed late in the evening is of the next day in Cottbus.
        let late = calendar_of(&t, &titles(), &names(), "WiSe 2026/27", "2026-09-23T22:30:00Z", Locale::De);
        assert_eq!(late.description, "Betula (inoffiziell) · Termine laut QIS, Stand 24.09.2026");
        let text = ics::write(&c);
        assert!(text.starts_with("BEGIN:VCALENDAR\r\n") && text.ends_with("END:VCALENDAR\r\n"));
        assert!(text.contains("\r\nX-WR-CALNAME:Studienplan WiSe 2026/27\r\n"));
        assert!(text.contains("\r\nX-WR-CALDESC:Betula (inoffiziell) · Termine laut QIS\\, Stand 23.09.2026\r\n"));
        assert_eq!(text.matches("BEGIN:VEVENT\r\n").count(), c.entries.len());
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
        assert_eq!(c.name, "Study plan Winter 2026/27");
        assert_eq!(c.description, "Betula (unofficial) · dates according to QIS, as of 23 Sep 2026");
        let uids = |c: &Calendar| c.entries.iter().map(|entry| (entry.uid.clone(), entry.when, entry.tentative)).collect::<Vec<_>>();
        assert_eq!(uids(&c), uids(&de), "a subscription that changes its language updates its entries");

        // The lecture: „Vorlesung" is what QIS calls it, the rest is English.
        let lecture = entry(&c, &uid_of(&rows[0], "2026-10-06"));
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
        let option = &of_event(&c, "20")[0];
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
        let noted = entry(&c, &uid_of(&rows[3], "2026-11-14"));
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
        assert!(text.contains("\r\nX-WR-CALNAME:Study plan Winter 2026/27\r\n"));
        assert!(text.contains("\r\nX-WR-CALDESC:Betula (unofficial) · dates according to QIS\\, as of 23 Sep 2026\r\n"));

        // A semester without dates says so in English, and a name given is kept.
        let summer = SemesterFacts::derive(SemesterKey::parse("2027S").unwrap(), None, &[]);
        let empty = build_in(&summer, &[], &[], &[A], &[], &Selection::default());
        let c = calendar_of(&empty, &titles(), &names(), "Summer 2027", "2026-09-23T12:35:16Z", Locale::En);
        assert_eq!(c.name, "Study plan Summer 2027");
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
        // 14 blocks of the 400 days one row walks at most, less the 14 holidays among them.
        let blocks: Vec<Fixture> = (0..14).map(|n| block(&(900 + n).to_string(), "2026-10-01", "2028-12-31")).collect();
        let t = build(&blocks, &[], &[A], &[], &Selection::default());
        let days = &t.events[0].rows[0].occ.days;
        assert_eq!(days.len(), 386);
        let c = calendar(&t);
        assert_eq!(c.entries.len(), MAX_ENTRIES);
        assert!(c.description.ends_with(", Stand 23.09.2026 · nur die ersten 5000 Termine"), "{}", c.description);
        let english = calendar_of(&t, &titles(), &names(), "", "2026-09-23T12:35:16Z", Locale::En).description;
        assert!(english.ends_with(", as of 23 Sep 2026 · only the first 5,000 dates"), "{english}");
        // 14 entries a day: 357 whole days and two of the next.
        assert_eq!(c.entries.first().map(day_of), days.first().copied());
        assert_eq!(c.entries.last().map(day_of), days.get(357).copied());
        assert_eq!(c.entries.iter().filter(|entry| Some(day_of(entry)) == days.get(357).copied()).count(), 2);
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
            town: TownChoice::Only(Town::Senftenberg),
            town_from: None,
        };
        let modules = ids(&[B, "FÜS", A, B, "0123", "FÜS"]);
        let (subscription, other) = Subscription::of(t.key, &modules, Some("079-82-2008"), &selection, Some(&t));
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
            }
        );
        assert_eq!(other, ["FÜS", "0123"]);
        assert_eq!(Subscription::from_code(&subscription.code().unwrap()), Some(subscription));
        // Without a timetable, everything the selection holds.
        let (all, _) = Subscription::of(t.key, &modules, None, &selection, None);
        assert_eq!(all.hidden_events, [50, 999_999]);
        assert_eq!((all.hidden_rows.len(), all.chosen_rows.len()), (3, 2));
        // A program id of another shape is left out, as a reader leaves it out.
        let (odd, _) = Subscription::of(t.key, &modules, Some("079_82_2008"), &selection, Some(&t));
        assert_eq!(odd.program, None);
        // Only ids that are no numbers: nothing a code can carry.
        let (none, other) = Subscription::of(t.key, &ids(&["FÜS"]), None, &Selection::default(), Some(&t));
        assert_eq!((none.modules.len(), other), (0, vec!["FÜS".to_string()]));
        assert_eq!(none.code(), Err(pack::Error::Malformed));
    }

    #[test]
    fn a_town_derived_from_the_import_is_written_as_it_is() {
        let with_tracks = |town: Option<Town>| Timetable { tracks: [A.to_string()].into(), town, town_derived: town.is_some(), ..semester() };
        let imported = Selection { town_from: Some([A.to_string()].into()), ..Selection::default() };
        let town = |selection: &Selection, t: &Timetable| Subscription::of(t.key, &ids(&[A, B]), None, selection, Some(t)).0.town;
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
        let (current, _) = Subscription::of(t.key, &modules, informatik, &selection(), Some(&t));
        let stored = current.code().unwrap();
        assert!(same_subscription(&stored, &current, &t));

        // QIS removes the hidden Termin's row and the hidden event: the plan is the same.
        let fewer: Vec<Fixture> =
            rows.iter().filter(|row| !["50", "60"].contains(&row.0.date.event_id.as_str())).cloned().collect();
        let now = build(&fewer, &exams(), &[A, B], &two_sws(), &selection());
        let (after, _) = Subscription::of(now.key, &modules, informatik, &selection(), Some(&now));
        assert_eq!((after.hidden_events.len(), after.hidden_rows.len()), (0, 1), "the new code is shorter");
        assert!(same_subscription(&stored, &after, &now));
        // A chosen row QIS no longer has, kept in a code made without the timetable: the same.
        let with_stale = Selection { chosen_rows: [RowKey { event: 20, fp: 0x12345 }].into(), ..selection() };
        let (all, _) = Subscription::of(t.key, &modules, informatik, &with_stale, None);
        assert!(same_subscription(&all.code().unwrap(), &current, &t));

        // A module added, a Termin hidden, a choice made, a kind, the town, an event: stale.
        let (more, _) = Subscription::of(t.key, &ids(&[A, B, "33333"]), informatik, &selection(), Some(&t));
        assert!(!same_subscription(&stored, &more, &t));
        // Another program, or none: the feed would name the modules otherwise.
        for program in [Some("C38-82-2024"), None] {
            let (other, _) = Subscription::of(t.key, &modules, program, &selection(), Some(&t));
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
            let (changed_now, _) = Subscription::of(t.key, &modules, informatik, &changed, Some(&t));
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
            assert!(c.entries.iter().all(|entry| entry.uid != uid_of(&short, gone)), "{gone}");
        }
        let dropped = "Entfällt: 23.12., 30.12. (vorlesungsfrei); 20.01. (Exkursion)";
        let both = entry(&c, &uid_of(&short, "2026-10-07"));
        assert_eq!(both.when, When::Timed { day: d("2026-10-07"), from: 600, to: 960 });
        assert_eq!(both.location.as_deref(), Some("HS 105 / Labor 213"));
        let both_lines = format!(
            "Praktikum 90\nModul 11111 Softwaretechnik\nwöchentlich 07.10.2026–27.01.2027\nLehrende: Bleicher / \
             Freymann\nEinweisung\n{dropped}"
        );
        assert_eq!(both.description.as_deref(), Some(format!("{both_lines}\nQuelle: QIS").as_str()));
        let noted = entry(&c, &uid_of(&short, "2026-10-21"));
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
            let only = entry(&c, &uid_of(&short, day));
            assert_eq!(only.when, When::Timed { day: d(day), from: 600, to: 960 }, "{day}");
            assert_eq!(only.location.as_deref(), Some("Labor 213"), "{day}");
            assert_eq!(only.description.as_deref(), Some(alone.as_str()), "{day}");
        }
    }

    /// Informatik B.Sc., FS1 in WiSe 2026/27, with the pinned subscription's selection: the
    /// Termine the design names. On any snapshot: the same calendar in plan order and by id, and
    /// every UID once.
    #[test]
    fn the_calendar_of_informatik_first_semester() {
        let pinned = crate::tests::studyplan_db("the_calendar_of_informatik_first_semester");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let meta = queries::meta(&db).unwrap();
        let semester = if is_pinned { "2026W".to_string() } else { meta.current_semester.clone().unwrap() };
        // The pinned code of subscription.rs: Informatik's abbreviations, 149408 hidden, „Nur
        // diesen" on 148369-a4d12.
        let code = crate::timetable::subscription::tests::FIRST_SEMESTER_CODE;
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
        let uids: Vec<&str> = feed.split("\r\n").filter_map(|line| line.strip_prefix("UID:")).collect();
        assert_eq!(uids.iter().collect::<BTreeSet<_>>().len(), uids.len(), "every UID once");
        if !is_pinned {
            return;
        }

        assert_eq!(stamp, "20260925T083015Z");
        let (of_plan, _) = Subscription::of(in_plan.key, &plan, Some("079-82-2008"), &selection, Some(&in_plan));
        assert_eq!(of_plan.code().as_deref(), Ok(code));
        let c = calendar_of(&in_plan, &titles, &names, "", &stamp, Locale::De);
        assert_eq!(c.name, "Studienplan WiSe 2026/27");
        // The stand is the build's, which wrote the short names on 25.09. (the data is of 23.09.).
        assert_eq!(c.description, "Betula (inoffiziell) · Termine laut QIS, Stand 25.09.2026");
        // 12104's Tuesday lecture, as C.14 shows it.
        let lecture = entry(&c, "148701-a2633-20261013@betula.app");
        assert_eq!(lecture.when, When::Timed { day: d("2026-10-13"), from: 690, to: 780 });
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
            "UID:148701-a2633-20261013@betula.app\r\nDTSTAMP:20260925T083015Z\r\n\
             DTSTART;TZID=Europe/Berlin:20261013T113000\r\nDTEND;TZID=Europe/Berlin:20261013T130000\r\n"
        ));
        // Senftenberg's course of 12104 and its exam are not the student's; 149408 is hidden.
        for gone in ["UID:149406-", "UID:149407-", "UID:150664-", "UID:149408-", "UID:148369-aaf38-", "20151227"] {
            assert!(!feed.contains(gone), "{gone}");
        }
        // „Nur diesen" on Monday 17:30: its 16 Mondays from 12.10. but the two in the break, firm.
        let chosen = of_event(&c, "148369");
        assert_eq!(chosen.len(), 14);
        assert!(chosen.iter().all(|entry| entry.uid.starts_with("148369-a4d12-") && !entry.tentative));
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
