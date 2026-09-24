//! A semester's timetable: the planned modules' events with every date, which slots are
//! alternatives of one another, which town's course a module is taken in, and what is shown.
//!
//! QIS lists an event's rows side by side without saying which of them a student attends. A
//! lecture's rows are all required (148109 meets on Tuesday and Thursday). An event held in named
//! groups („1-Gruppe", „2-Gruppe") is attended in one of them. An Übung without groups that QIS
//! lists at several times of one length is taken once when the module's SWS say one slot is
//! enough (148369: 2 SWS, four 90-minute slots), and at every time when they say more (148297:
//! 4 SWS, two slots, both required). Anything the rules do not decide stays required: a false
//! „1 von 2" hides a date a student needs, a false clash costs one click on „Veranstaltung
//! ausblenden".
//!
//! Five modules are taught in Cottbus and in Senftenberg as two whole courses, each town with its
//! lecture and its Übung (12104). A student takes one of them, so the other town's events of such
//! a module are hidden: those of the town the visitor chose, else of the town most of the plan's
//! other modules are taught in. A module whose Cottbus lecture and Senftenberg lab are both
//! required (13693) has no tracks, and Cottbus sites are never tracks (11760's two sites hold its
//! two required parts).
//!
//! Then the visitor's selection: hidden events, kinds and Termine, and made choices. Every event
//! and row says why it is not shown (`HiddenBy`, the first rule that holds). The clashes of what
//! is left are `clash`'s, the exams and their warnings `exams`'.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::clash::{self, Clash};
use super::day::Day;
use super::exams::{self, Exam, ExamWarning};
use super::facts::SemesterFacts;
use super::kind::{class_of, kinds_of, Class, EventKind, KindSet};
use super::occur::{self, Every, Occurrences};
use super::rowkey::RowKey;
use super::select::{town_of, HiddenBy, Selection, Town, TownChoice};
use super::semester::SemesterKey;
use crate::rows::Semester;
use crate::rows_detail::{DateRow, EventDate, ModuleSws};

/// The group QIS gives a row that belongs to no group.
const UNNAMED_GROUP: &str = "[unbenannt]";

/// Minutes of one SWS: a 90-minute slot every week is 2 SWS.
const SWS_MINUTES: f64 = 45.0;

/// One slot is all a student attends only when it holds at least this share of the SWS the module
/// states for the event's forms: two 90-minute slots of a 4-SWS Übung are both needed (148297).
const SWS_SLOT_SHARE: f64 = 0.75;

/// And only when the slots together offer clearly more than those SWS. With two or more slots
/// the share above already implies it (2 × 0.75 > 1.25); it is kept so the rule reads as stated.
const SWS_SURPLUS: f64 = 1.25;

/// The number of module tones (`t-1` … `t-8`); the ninth module takes the first again.
const TONES: usize = 8;

/// What a timetable is built from: one semester's rows of the planned modules, as the loaders
/// fetch them.
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    pub key: SemesterKey,
    /// The semester's `v_semester` row: the exam reading needs its bounds.
    pub semester: Option<&'a Semester>,
    pub facts: &'a SemesterFacts,
    /// The planned module ids of the semester, in plan order: tones and the order of events
    /// follow it.
    pub modules: &'a [String],
    /// The modules' teaching rows (`modules_schedule`), an event once per linking module.
    pub schedule: &'a [DateRow],
    /// The modules' exam rows (`modules_exams`).
    pub exams: &'a [DateRow],
    /// The modules' SWS per teaching form (`modules_teaching_sws`).
    pub sws: &'a [ModuleSws],
}

/// One semester of a Studienplan, with the visitor's selection applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Timetable {
    pub key: SemesterKey,
    pub facts: SemesterFacts,
    /// The planned module ids, in plan order, each once.
    pub modules: Vec<String>,
    /// By the plan position of the first module that links them, then title, then event id as a
    /// number.
    pub events: Vec<Event>,
    /// The planned modules' exams (`exams::exams_of`).
    pub exams: Vec<Exam>,
    /// The planned modules with city tracks: a lecture and a non-lecture event in each town.
    pub tracks: BTreeSet<String>,
    /// The town whose course a module with tracks is taken in; `None` for both.
    pub town: Option<Town>,
    /// The town was derived from the plan's other modules, not chosen.
    pub town_derived: bool,
    /// Hard teaching clashes (`clash::clashes`).
    pub clashes: Vec<Clash>,
    /// Open choices (`Event::unresolved`) that cannot take an option free of clashes, by event
    /// index (`clash::clashes`). A clash names each of them; the rows of all hard clashes are
    /// `clash::hard_rows`.
    pub blocked: Vec<usize>,
    /// Exam warnings (`exams::exam_warnings`).
    pub exam_warnings: Vec<ExamWarning>,
    /// Days on which a hop between exams cannot be judged, with their modules.
    pub place_unknown: Vec<(Day, Vec<String>)>,
    /// Planned modules without a dated teaching row this semester, in plan order.
    pub without_dates: Vec<String>,
}

/// One teaching event of the planned modules.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// QIS's `veranstid`.
    pub id: String,
    pub number: Option<String>,
    pub title: String,
    pub type_raw: Option<String>,
    pub kinds: KindSet,
    pub class: Class,
    /// The planned modules that link it, in plan order.
    pub modules: Vec<String>,
    /// 1..=8, from the plan position of its first module.
    pub tone: u8,
    pub attendance: Attendance,
    /// The option a made choice („Nur diesen") picked.
    pub chosen: Option<usize>,
    /// Its dated rows by `ord`; empty for an event without dates („ohne Termine").
    pub rows: Vec<Row>,
    pub hidden: Option<HiddenBy>,
    pub source_url: Option<String>,
}

/// Which of an event's rows a student attends.
#[derive(Clone, Debug, PartialEq)]
pub enum Attendance {
    /// Every row.
    All,
    /// One of the options, each a list of row indices; a row in no option is required.
    OneOf { options: Vec<Vec<usize>>, basis: Basis },
}

/// Why an event's rows are options: the rule, which the page names as derived (R12).
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// QIS names two or more groups.
    Groups,
    /// Parallel slots of one length, one of which covers the SWS the module states for the
    /// event's forms.
    Sws { stated: f64 },
}

/// One date row of an event, with its dates.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: Option<RowKey>,
    pub ord: Option<i64>,
    pub date: EventDate,
    pub cancelled_dates: Option<String>,
    pub occ: Occurrences,
    /// The times in minutes (`occur::times`); `None` is „Zeit unklar": listed, never in a clash.
    pub from: Option<u16>,
    pub to: Option<u16>,
    /// The option of `Attendance::OneOf` it belongs to; `None` for a required row.
    pub option: Option<usize>,
    pub hidden: Option<HiddenBy>,
}

impl Event {
    /// The options of an open or made choice that still have a visible row, in order. Empty for
    /// an event whose rows are all required, and for a hidden event.
    pub fn visible_options(&self) -> Vec<usize> {
        let Attendance::OneOf { options, .. } = &self.attendance else {
            return Vec::new();
        };
        options
            .iter()
            .enumerate()
            .filter(|(_, rows)| rows.iter().any(|index| self.rows.get(*index).is_some_and(|row| row.hidden.is_none())))
            .map(|(option, _)| option)
            .collect()
    }

    /// Whether the student still has to pick one of two or more visible options („1 von 4
    /// wählen").
    pub fn unresolved(&self) -> bool {
        self.visible_options().len() >= 2
    }

    /// The one town all its rows with a known campus are in; `None` when they are in both towns
    /// or no campus is known.
    pub fn town(&self) -> Option<Town> {
        let mut towns = self.rows.iter().filter_map(|row| row.date.campus.as_ref().and_then(town_of));
        let first = towns.next()?;
        towns.all(|town| town == first).then_some(first)
    }
}

impl Row {
    fn of(row: &DateRow, facts: &SemesterFacts) -> Row {
        let times = occur::times(&row.date);
        Row {
            key: RowKey::of(&row.date),
            ord: row.ord,
            date: row.date.clone(),
            cancelled_dates: row.cancelled_dates.clone(),
            occ: occur::occurrences(row, facts),
            from: times.map(|(from, _)| from),
            to: times.map(|(_, to)| to),
            option: None,
            hidden: None,
        }
    }
}

impl Timetable {
    /// The timetable of `input` as `selection` shows it. Rows of modules that are not planned are
    /// left out, and an event keeps only its planned modules.
    pub fn build(input: &Input<'_>, selection: &Selection) -> Timetable {
        let modules = plan_order(input.modules);
        let mut events = gather(input.schedule, &modules, input.facts);
        for event in &mut events {
            decide(event, input.sws);
        }
        let tracks = tracks(&events, &modules);
        let (town, town_derived) = effective_town(selection.town, &events, &tracks);
        for event in &mut events {
            event.hidden = event_hidden(event, selection, town, &tracks);
            choose(event, selection);
        }
        let (clashes, blocked) = clash::clashes(&events);
        let exams = exams::exams_of(input.exams, input.semester, &modules, selection, town, &tracks);
        let (exam_warnings, place_unknown) = exams::exam_warnings(&exams, &modules);
        let without_dates = modules
            .iter()
            .filter(|module| !events.iter().any(|event| !event.rows.is_empty() && event.modules.contains(module)))
            .cloned()
            .collect();
        Timetable {
            key: input.key,
            facts: input.facts.clone(),
            modules,
            events,
            exams,
            tracks,
            town,
            town_derived,
            clashes,
            blocked,
            exam_warnings,
            place_unknown,
            without_dates,
        }
    }

    /// The kinds of the semester's events and exams with the number of events of each, hidden
    /// ones included, in the kinds' order: the chips „Vorlesung 5", „Übung 4", „Prüfung 4". A
    /// „Vorlesung/Übung" counts for both.
    pub fn kinds_present(&self) -> Vec<(EventKind, usize)> {
        EventKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let count = match kind {
                    EventKind::Exam => self.exams.len(),
                    _ => self.events.iter().filter(|event| event.kinds.contains(kind)).count(),
                };
                (count > 0).then_some((kind, count))
            })
            .collect()
    }
}

/// The planned ids in plan order, each once.
fn plan_order(modules: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    modules.iter().filter(|module| seen.insert(module.as_str())).cloned().collect()
}

/// The events of the planned modules' rows, each row once (the view lists an event once for every
/// module that links it), sorted by the plan position of the first module that links them, then
/// title, then event id as a number.
fn gather(schedule: &[DateRow], modules: &[String], facts: &SemesterFacts) -> Vec<Event> {
    let positions: BTreeMap<&str, usize> =
        modules.iter().enumerate().map(|(position, module)| (module.as_str(), position)).collect();
    let mut by_event: BTreeMap<&str, (BTreeSet<usize>, Vec<&DateRow>)> = BTreeMap::new();
    let mut seen: BTreeSet<(&str, Option<i64>)> = BTreeSet::new();
    for row in schedule {
        let Some(position) = positions.get(row.module_id.as_str()) else {
            continue;
        };
        let id = row.date.event_id.as_str();
        let (linked, rows) = by_event.entry(id).or_default();
        linked.insert(*position);
        if seen.insert((id, row.ord)) {
            rows.push(row);
        }
    }

    let mut events: Vec<(usize, Event)> = Vec::with_capacity(by_event.len());
    for (id, (linked, mut rows)) in by_event {
        rows.sort_by_key(|row| row.ord);
        let (Some(position), Some(first)) = (linked.first().copied(), rows.first()) else {
            continue;
        };
        let date = &first.date;
        let kinds = kinds_of(date.event_type.as_deref());
        let event = Event {
            id: id.to_string(),
            number: date.event_number.clone(),
            title: date.event_title.clone(),
            type_raw: date.event_type.clone(),
            kinds,
            class: class_of(kinds),
            modules: linked.iter().filter_map(|position| modules.get(*position).cloned()).collect(),
            tone: tone(position),
            attendance: Attendance::All,
            chosen: None,
            // A row without `ord` is the view's line for an event without dates: no row at all.
            rows: rows.iter().filter(|row| row.ord.is_some()).map(|row| Row::of(row, facts)).collect(),
            hidden: None,
            source_url: date.source_url.clone(),
        };
        events.push((position, event));
    }
    events.sort_by(|(p, a), (q, b)| {
        p.cmp(q).then_with(|| a.title.cmp(&b.title)).then_with(|| (a.id.len(), &a.id).cmp(&(b.id.len(), &b.id)))
    });
    events.into_iter().map(|(_, event)| event).collect()
}

/// The tone of the module at `position` in the plan: 1..=8, then round again.
fn tone(position: usize) -> u8 {
    u8::try_from(position % TONES).unwrap_or(0) + 1
}

/// Decides the event's attendance and marks the rows of each option.
fn decide(event: &mut Event, sws: &[ModuleSws]) {
    let decided = match by_groups(&event.rows) {
        Some(options) => Some((options, Basis::Groups)),
        None => by_sws(event, sws).map(|(options, stated)| (options, Basis::Sws { stated })),
    };
    let Some((options, basis)) = decided else {
        event.attendance = Attendance::All;
        return;
    };
    for (option, rows) in options.iter().enumerate() {
        for index in rows {
            if let Some(row) = event.rows.get_mut(*index) {
                row.option = Some(option);
            }
        }
    }
    event.attendance = Attendance::OneOf { options, basis };
}

/// The row's named group; QIS's „[unbenannt]" and an empty name are none.
fn group(row: &Row) -> Option<&str> {
    row.date.group_name.as_deref().map(str::trim).filter(|name| !name.is_empty() && *name != UNNAMED_GROUP)
}

/// One option per named group when QIS names two or more (in the order the groups first appear),
/// each with all its rows. Rows without a group stay required: every group attends them.
fn by_groups(rows: &[Row]) -> Option<Vec<Vec<usize>>> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(name) = group(row) else {
            continue;
        };
        match groups.iter_mut().find(|(seen, _)| *seen == name) {
            Some((_, members)) => members.push(index),
            None => groups.push((name, vec![index])),
        }
    }
    (groups.len() >= 2).then(|| groups.into_iter().map(|(_, members)| members).collect())
}

/// A recurring slot of an event: its weekday and times, its rows (one slot may run in two ranges
/// or two rooms), and the hull of their date ranges (`None`: a row has no range, so the lecture
/// period is assumed, which meets every other range).
struct Slot {
    at: (u8, u16, u16),
    rows: Vec<usize>,
    range: Option<(Day, Day)>,
}

/// One option per slot when the event's parallel slots are alternatives by the module's SWS: the
/// event is no lecture and names no group; it has two or more recurring slots with a weekday and
/// a time, all of one length, whose date ranges meet pairwise (slots one after another are all
/// needed); and one slot is at least 0.75 of the stated SWS while all together exceed 1.25 of
/// them. Singles, blocks and rows without a time stay required. The stated SWS are the most any
/// linking planned module states for the event's forms; without any the rule does not apply.
fn by_sws(event: &Event, sws: &[ModuleSws]) -> Option<(Vec<Vec<usize>>, f64)> {
    if event.kinds.contains(EventKind::Lecture) || event.rows.iter().any(|row| group(row).is_some()) {
        return None;
    }
    let mut slots: Vec<Slot> = Vec::new();
    for (index, row) in event.rows.iter().enumerate() {
        let Some(at) = slot_of(row) else {
            continue;
        };
        let range = range_of(&row.date);
        match slots.iter_mut().find(|slot| slot.at == at) {
            Some(slot) => {
                slot.rows.push(index);
                slot.range = slot.range.zip(range).map(|(a, b)| (a.0.min(b.0), a.1.max(b.1)));
            }
            None => slots.push(Slot { at, rows: vec![index], range }),
        }
    }
    let length = |slot: &Slot| slot.at.2.saturating_sub(slot.at.1);
    let minutes = length(slots.first()?);
    if slots.len() < 2 || slots.iter().any(|slot| length(slot) != minutes) {
        return None;
    }
    let meet = |a: &Slot, b: &Slot| match (a.range, b.range) {
        (Some(a), Some(b)) => a.0 <= b.1 && b.0 <= a.1,
        _ => true,
    };
    if slots.iter().enumerate().any(|(i, a)| slots.iter().skip(i + 1).any(|b| !meet(a, b))) {
        return None;
    }
    let stated = stated_sws(event, sws)?;
    let slot = f64::from(minutes) / SWS_MINUTES;
    let count = f64::from(u32::try_from(slots.len()).unwrap_or(u32::MAX));
    (slot >= SWS_SLOT_SHARE * stated && slot * count > SWS_SURPLUS * stated)
        .then(|| (slots.into_iter().map(|slot| slot.rows).collect(), stated))
}

/// Weekday and times of a recurring row that has both.
fn slot_of(row: &Row) -> Option<(u8, u16, u16)> {
    Every::of(&row.date)?;
    let weekday = row.date.weekday.and_then(|w| u8::try_from(w).ok()).filter(|w| (1..=7).contains(w))?;
    Some((weekday, row.from?, row.to?))
}

/// A row's date range as stated, earlier day first.
fn range_of(date: &EventDate) -> Option<(Day, Day)> {
    let first = date.first_date.as_deref().and_then(Day::parse)?;
    let last = date.last_date.as_deref().and_then(Day::parse)?;
    Some((first.min(last), first.max(last)))
}

/// The most SWS a linking planned module states for the event's forms, when one states any. The
/// most, not the least: a module that asks for more time makes the slots less likely to be
/// alternatives, the safe direction.
fn stated_sws(event: &Event, sws: &[ModuleSws]) -> Option<f64> {
    event
        .modules
        .iter()
        .map(|module| {
            sws.iter()
                .filter(|s| s.module_id == *module)
                .filter(|s| s.form.known().is_some_and(|form| event.kinds.contains(EventKind::of_form(form))))
                .map(|s| s.sws)
                .sum::<f64>()
        })
        .filter(|sum| sum.is_finite() && *sum > 0.0)
        .fold(None, |most: Option<f64>, sum| Some(most.map_or(sum, |most| most.max(sum))))
}

/// The planned modules with city tracks: each town holds a lecture event and a non-lecture event
/// of the module (only events whose rows are in one town count). The reading „both towns together
/// hold both kinds" would also mark 13693, whose Cottbus lecture and Senftenberg lab are both
/// required.
fn tracks(events: &[Event], modules: &[String]) -> BTreeSet<String> {
    let towns: Vec<Option<Town>> = events.iter().map(Event::town).collect();
    modules
        .iter()
        .filter(|module| {
            let held: BTreeSet<(Town, bool)> = events
                .iter()
                .zip(&towns)
                .filter(|(event, _)| event.modules.contains(module))
                .filter_map(|(event, town)| Some(((*town)?, event.class == Class::Lecture)))
                .collect();
            [Town::Cottbus, Town::Senftenberg]
                .into_iter()
                .all(|town| held.contains(&(town, true)) && held.contains(&(town, false)))
        })
        .cloned()
        .collect()
}

/// The town a module with tracks is taken in, and whether it was derived: the chosen one; none
/// for both; derived, the town that holds strictly more of the located events of the plan's
/// modules without tracks, else none. It is derived even when no planned module has tracks: the
/// finder resolves a candidate's tracks with it.
fn effective_town(choice: TownChoice, events: &[Event], tracks: &BTreeSet<String>) -> (Option<Town>, bool) {
    match choice {
        TownChoice::Only(town) => (Some(town), false),
        TownChoice::Both => (None, false),
        TownChoice::Derive => {
            let (mut cottbus, mut senftenberg) = (0usize, 0usize);
            for event in events.iter().filter(|event| event.modules.iter().any(|module| !tracks.contains(module))) {
                match event.town() {
                    Some(Town::Cottbus) => cottbus += 1,
                    Some(Town::Senftenberg) => senftenberg += 1,
                    None => {}
                }
            }
            let town = match cottbus.cmp(&senftenberg) {
                Ordering::Greater => Some(Town::Cottbus),
                Ordering::Less => Some(Town::Senftenberg),
                Ordering::Equal => None,
            };
            (town, town.is_some())
        }
    }
}

/// Why a whole event is not shown, first rule that holds: hidden itself; every kind hidden (a
/// „Vorlesung/Übung" stays while lectures are shown); the other town's course of modules that all
/// have tracks, carrying the town that is shown. An event of mixed or unknown town, or one that
/// also belongs to a module without tracks, stays.
fn event_hidden(
    event: &Event,
    selection: &Selection,
    town: Option<Town>,
    tracks: &BTreeSet<String>,
) -> Option<HiddenBy> {
    if event_number(&event.id).is_some_and(|id| selection.hidden_events.contains(&id)) {
        return Some(HiddenBy::Event);
    }
    if event.kinds.hidden_by(selection.hidden_kinds) {
        return Some(HiddenBy::Kinds);
    }
    let shown = town?;
    let all_tracks = !event.modules.is_empty() && event.modules.iter().all(|module| tracks.contains(module));
    (all_tracks && event.town().is_some_and(|own| own != shown)).then_some(HiddenBy::Town(shown))
}

/// A `veranstid` as the selection keeps it: digits without a leading zero that fit a `u32`.
fn event_number(id: &str) -> Option<u32> {
    let canonical = !id.is_empty() && !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| id.parse().ok()).flatten()
}

/// Applies a made choice and the hidden Termine to the rows. The first row (by `ord`) of an
/// option whose key is chosen decides; a chosen key that names no option's row (a required row,
/// or one QIS changed) changes nothing, and the choice is open again. A row is hidden, first rule
/// that holds, with its event, when another option is chosen, or when its key is hidden.
fn choose(event: &mut Event, selection: &Selection) {
    event.chosen = match event.attendance {
        Attendance::OneOf { .. } => event
            .rows
            .iter()
            .find_map(|row| row.option.filter(|_| row.key.is_some_and(|key| selection.chosen_rows.contains(&key)))),
        Attendance::All => None,
    };
    let chosen = event.chosen;
    for row in &mut event.rows {
        let by_choice = chosen.is_some() && row.option.is_some() && row.option != chosen;
        let by_row = row.key.is_some_and(|key| selection.hidden_rows.contains(&key));
        row.hidden = event.hidden.or(by_choice.then_some(HiddenBy::Choice)).or(by_row.then_some(HiddenBy::Row));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::Database;
    use crate::labels::Code;
    use crate::queries;
    use crate::timetable::select::MAX_MODULES;

    pub(crate) fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    pub(crate) fn ids(modules: &[&str]) -> Vec<String> {
        modules.iter().map(|id| id.to_string()).collect()
    }

    /// 2026W as the data says it: lectures 05.10.2026–31.01.2027, the break 21.12.–03.01., A weeks
    /// from the first week.
    pub(crate) fn winter() -> SemesterFacts {
        let key = SemesterKey::parse("2026W").unwrap();
        SemesterFacts {
            lecture: Some((d("2026-10-05"), d("2027-01-31"))),
            breaks: vec![(d("2026-12-21"), d("2027-01-03"))],
            a_week: Some(d("2026-10-05")),
            ..SemesterFacts::derive(key, None, &[])
        }
    }

    /// A teaching row of 2026W, as `modules_schedule` delivers it: weekly on its weekday through
    /// the lecture period, unnamed, at Zentralcampus. The methods change what a case needs.
    #[derive(Clone, Debug)]
    pub(crate) struct Fixture(pub(crate) DateRow);

    pub(crate) fn teaching(
        module: &str,
        event: &str,
        ord: i64,
        kind: &str,
        weekday: i64,
        from: &str,
        to: &str,
    ) -> Fixture {
        let first = d("2026-10-05").plus(i32::try_from(weekday).unwrap() - 1);
        Fixture(DateRow {
            module_id: module.into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: Some(format!("N{event}")),
                event_title: format!("{kind} {event}"),
                event_type: Some(kind.into()),
                group_name: Some(UNNAMED_GROUP.into()),
                weekday: Some(weekday),
                start_time: Some(from.into()),
                end_time: Some(to.into()),
                rhythm: Some(Code::parse("weekly")),
                rhythm_raw: Some("wöch.".into()),
                first_date: Some(first.iso()),
                last_date: Some(first.plus(7 * 16).iso()),
                room: None,
                campus: Some(Code::parse("zentralcampus")),
                instructor: None,
                comment: None,
                source_url: Some(format!("https://qis.example/{event}")),
            },
        })
    }

    impl Fixture {
        pub(crate) fn group(mut self, name: &str) -> Self {
            self.0.date.group_name = Some(name.into());
            self
        }

        pub(crate) fn campus(mut self, campus: Option<&str>) -> Self {
            self.0.date.campus = campus.map(Code::parse);
            self
        }

        pub(crate) fn rhythm(mut self, rhythm: &str) -> Self {
            self.0.date.rhythm = Some(Code::parse(rhythm));
            self
        }

        pub(crate) fn range(mut self, first: &str, last: &str) -> Self {
            self.0.date.first_date = Some(first.into());
            self.0.date.last_date = Some(last.into());
            self
        }

        pub(crate) fn undated(mut self) -> Self {
            (self.0.date.first_date, self.0.date.last_date) = (None, None);
            self
        }

        pub(crate) fn title(mut self, title: &str) -> Self {
            self.0.date.event_title = title.into();
            self
        }

        /// The same row linked to another module.
        pub(crate) fn of(mut self, module: &str) -> Self {
            self.0.module_id = module.into();
            self
        }

        pub(crate) fn key(&self) -> RowKey {
            RowKey::of(&self.0.date).unwrap()
        }
    }

    pub(crate) fn sws(module: &str, form: &str, sws: f64) -> ModuleSws {
        ModuleSws { module_id: module.into(), form: Code::parse(form), sws }
    }

    /// The timetable of synthetic rows of 2026W.
    pub(crate) fn table(rows: &[Fixture], modules: &[&str], sws: &[ModuleSws], selection: &Selection) -> Timetable {
        let facts = winter();
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let modules = ids(modules);
        let input = Input {
            key: facts.key,
            semester: None,
            facts: &facts,
            modules: &modules,
            schedule: &schedule,
            exams: &[],
            sws,
        };
        let built = Timetable::build(&input, selection);
        invariants(&built);
        built
    }

    pub(crate) fn event<'t>(table: &'t Timetable, id: &str) -> &'t Event {
        table.events.iter().find(|event| event.id == id).unwrap_or_else(|| panic!("no event {id}"))
    }

    /// How many options an event's rows form, `None` when all are required.
    pub(crate) fn options(event: &Event) -> Option<usize> {
        match &event.attendance {
            Attendance::OneOf { options, .. } => Some(options.len()),
            Attendance::All => None,
        }
    }

    /// The timetable of `modules` in `semester` of a snapshot, loaded as the Studienplan loads it.
    pub(crate) fn planned(db: &dyn Database, semester: &str, modules: &[String], selection: &Selection) -> Timetable {
        let semesters = queries::semesters(db).unwrap();
        let row = semesters.iter().find(|s| s.key == semester);
        let key = SemesterKey::parse(semester).unwrap();
        let facts = SemesterFacts::derive(key, row, &queries::semester_date_counts(db, semester).unwrap());
        let schedule = queries::modules_schedule(db, modules, semester).unwrap();
        let exams = queries::modules_exams(db, modules, semester).unwrap();
        let sws = queries::modules_teaching_sws(db, modules).unwrap();
        let input = Input { key, semester: row, facts: &facts, modules, schedule: &schedule, exams: &exams, sws: &sws };
        let built = Timetable::build(&input, selection);
        invariants(&built);
        built
    }

    /// What holds for every timetable: events once and in plan order, rows once by `ord`, options
    /// that partition the rows they hold, a choice only of an option, the rows of a hidden event
    /// hidden with it, tones in range, chips that count, and clashes between visible, timed,
    /// overlapping rows of events without a common module.
    pub(crate) fn invariants(t: &Timetable) {
        let position =
            |id: &str| t.modules.iter().position(|m| m == id).unwrap_or_else(|| panic!("{id} is not planned"));
        let distinct: BTreeSet<&str> = t.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(distinct.len(), t.events.len(), "an event twice");
        let firsts: Vec<usize> = t.events.iter().map(|e| position(&e.modules[0])).collect();
        assert!(firsts.windows(2).all(|w| w[0] <= w[1]), "events in plan order");
        for (i, e) in t.events.iter().enumerate() {
            let positions: Vec<usize> = e.modules.iter().map(|m| position(m)).collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{}: modules in plan order, each once", e.id);
            assert_eq!(usize::from(e.tone), positions[0] % TONES + 1, "{}", e.id);
            assert_eq!(e.class, class_of(e.kinds));
            assert!(e.rows.windows(2).all(|w| w[0].ord < w[1].ord), "{}: rows once, by ord", e.id);
            assert!(e.rows.iter().all(|r| r.ord.is_some() && r.from.is_some() == r.to.is_some()));
            match &e.attendance {
                Attendance::All => {
                    assert!(e.rows.iter().all(|r| r.option.is_none()) && e.chosen.is_none(), "{}", e.id);
                }
                Attendance::OneOf { options, basis } => {
                    assert!(options.len() >= 2, "{}", e.id);
                    let mut held = BTreeSet::new();
                    for (o, rows) in options.iter().enumerate() {
                        assert!(!rows.is_empty());
                        for r in rows {
                            assert!(held.insert(*r), "{}: row {r} in two options", e.id);
                            assert_eq!(e.rows[*r].option, Some(o));
                        }
                    }
                    assert_eq!(held.len(), e.rows.iter().filter(|r| r.option.is_some()).count());
                    if let Basis::Sws { stated } = basis {
                        assert!(*stated > 0.0 && !e.kinds.contains(EventKind::Lecture), "{}", e.id);
                    }
                    if let Some(chosen) = e.chosen {
                        assert!(chosen < options.len());
                        assert!(e.visible_options().iter().all(|o| *o == chosen), "{}", e.id);
                    }
                }
            }
            for r in &e.rows {
                if e.hidden.is_some() {
                    assert_eq!(r.hidden, e.hidden, "{}: a row of a hidden event", e.id);
                }
                if r.hidden == Some(HiddenBy::Choice) {
                    assert!(e.chosen.is_some() && r.option != e.chosen);
                }
            }
            if e.hidden.is_some() {
                assert!(e.visible_options().is_empty() && !e.unresolved());
            }
            if let Some(HiddenBy::Town(shown)) = e.hidden {
                assert_eq!(t.town, Some(shown));
                assert!(e.modules.iter().all(|m| t.tracks.contains(m)) && e.town() != Some(shown), "{}", e.id);
            }
            assert!(!t.blocked.contains(&i) || e.unresolved(), "{}", e.id);
        }
        assert!(t.blocked.windows(2).all(|w| w[0] < w[1]));
        for clash in &t.clashes {
            let (ea, eb) = (&t.events[clash.a.0], &t.events[clash.b.0]);
            let (ra, rb) = (&ea.rows[clash.a.1], &eb.rows[clash.b.1]);
            assert!(clash.a.0 < clash.b.0);
            assert!(ea.modules.iter().all(|m| !eb.modules.contains(m)), "{} × {}", ea.id, eb.id);
            assert!(ea.hidden.is_none() && eb.hidden.is_none() && ra.hidden.is_none() && rb.hidden.is_none());
            let (from, to) = (ra.from.unwrap(), ra.to.unwrap());
            let (other_from, other_to) = (rb.from.unwrap(), rb.to.unwrap());
            assert!(from < other_to && other_from < to, "{} × {}", ea.id, eb.id);
            if clash.days > 0 {
                assert!(ra.occ.days.contains(&clash.first) || rb.occ.days.contains(&clash.first));
            } else {
                assert!(ra.occ.template.is_some() && rb.occ.template.is_some());
            }
        }
        // Every blocked choice is named by a clash; a clash names an option of an open choice only
        // when that choice is blocked; the rows the clashes name are hard rows.
        for b in &t.blocked {
            assert!(
                t.clashes.iter().any(|c| c.a.0 == *b || c.b.0 == *b),
                "{} blocked, named by no clash",
                t.events[*b].id
            );
        }
        let hard = clash::hard_rows(&t.events);
        assert!(t.clashes.iter().all(|c| hard.contains(&c.a) && hard.contains(&c.b)));
        for (event, row) in &hard {
            let e = &t.events[*event];
            let r = &e.rows[*row];
            assert!(e.hidden.is_none() && r.hidden.is_none() && r.from.is_some());
            assert!(r.option.is_none() || !e.unresolved() || t.blocked.contains(event), "{}/{:?}", e.id, r.ord);
            assert!(t.clashes.iter().any(|c| c.a.0 == *event || c.b.0 == *event), "{}", e.id);
        }
        let chips = t.kinds_present();
        assert!(chips.iter().all(|(_, n)| *n > 0) && chips.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(chips.iter().any(|(k, _)| *k == EventKind::Exam), !t.exams.is_empty());
        assert!(!t.town_derived || t.town.is_some());
        for m in &t.without_dates {
            assert!(t.events.iter().all(|e| e.rows.is_empty() || !e.modules.contains(m)), "{m}");
        }
    }

    #[test]
    fn events_follow_the_plan_with_their_rows_once() {
        let rows = [
            teaching("B", "20", 1, "Vorlesung", 1, "09:15", "10:45").title("Zweite"),
            teaching("A", "20", 1, "Vorlesung", 1, "09:15", "10:45").title("Zweite"),
            teaching("A", "20", 2, "Vorlesung", 3, "09:15", "10:45").title("Zweite"),
            teaching("A", "3", 1, "Übung", 2, "09:15", "10:45").title("Zweite"),
            teaching("B", "10", 1, "Übung", 4, "09:15", "10:45").title("Erste"),
            teaching("X", "30", 1, "Vorlesung", 4, "09:15", "10:45"),
        ];
        let t = table(&rows, &["B", "A", "B"], &[], &Selection::default());
        assert_eq!(t.modules, ids(&["B", "A"]));
        let events: Vec<(&str, Vec<String>, u8)> =
            t.events.iter().map(|e| (e.id.as_str(), e.modules.clone(), e.tone)).collect();
        // B's events first, by title; event 30 belongs to no planned module.
        assert_eq!(events, [("10", ids(&["B"]), 1), ("20", ids(&["B", "A"]), 1), ("3", ids(&["A"]), 2)]);
        let shared = event(&t, "20");
        assert_eq!(shared.rows.iter().map(|r| r.ord).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!((shared.number.as_deref(), shared.class), (Some("N20"), Class::Lecture));
        assert_eq!(shared.source_url.as_deref(), Some("https://qis.example/20"));
        assert_eq!(shared.rows[0].occ.days.len(), 15);
        assert_eq!((shared.rows[0].from, shared.rows[0].to), (Some(555), Some(645)));
        assert_eq!(shared.rows[0].key, RowKey::of(&rows[1].0.date));
        assert_eq!(t.kinds_present(), [(EventKind::Lecture, 1), (EventKind::Exercise, 2)]);
        // Ties of title are broken by the event id as a number; the ninth module's tone is the first.
        let t = table(
            &rows[3..5].iter().cloned().map(|r| r.title("Gleich").of("A")).collect::<Vec<_>>(),
            &["A"],
            &[],
            &Selection::default(),
        );
        assert_eq!(t.events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["3", "10"]);
        assert_eq!((tone(0), tone(7), tone(8), tone(9)), (1, 8, 1, 2));
    }

    #[test]
    fn an_event_without_dates_has_no_rows() {
        let mut undated = teaching("A", "40", 1, "Seminar", 1, "09:15", "10:45").undated();
        undated.0.ord = None;
        (undated.0.date.weekday, undated.0.date.start_time, undated.0.date.end_time) = (None, None, None);
        let dated = teaching("B", "41", 1, "Seminar", 1, "09:15", "10:45");
        let t = table(&[undated, dated.clone()], &["A", "B", "C"], &[], &Selection::default());
        assert!(event(&t, "40").rows.is_empty());
        assert_eq!(t.without_dates, ids(&["A", "C"]));
        assert_eq!(t.kinds_present(), [(EventKind::Seminar, 2)]);
        // Unclear times keep the row, without a place in the day.
        let mut unclear = dated;
        unclear.0.date.end_time = Some("08:00".into());
        let t = table(&[unclear], &["B"], &[], &Selection::default());
        assert_eq!((t.events[0].rows[0].from, t.events[0].rows[0].to), (None, None));
        assert_eq!(t.events[0].rows[0].occ.days.len(), 15);
        assert!(t.without_dates.is_empty());
    }

    #[test]
    fn named_groups_are_options_and_unnamed_rows_stay_required() {
        let rows = [
            teaching("A", "50", 1, "Übung", 1, "09:15", "10:45").group("1-Gruppe"),
            teaching("A", "50", 2, "Übung", 1, "11:30", "13:00").group("2-Gruppe"),
            teaching("A", "50", 3, "Übung", 3, "11:30", "13:00").group("1-Gruppe"),
            teaching("A", "50", 4, "Übung", 5, "07:30", "09:00"),
        ];
        let t = table(&rows, &["A"], &[], &Selection::default());
        let e = event(&t, "50");
        assert_eq!(e.attendance, Attendance::OneOf { options: vec![vec![0, 2], vec![1]], basis: Basis::Groups });
        assert_eq!(e.rows[3].option, None);
        assert_eq!(e.visible_options(), [0, 1]);
        assert!(e.unresolved());
        // One named group is no choice, with or without unnamed rows.
        assert_eq!(table(&rows[..1], &["A"], &[], &Selection::default()).events[0].attendance, Attendance::All);
        let one = [rows[0].clone(), rows[3].clone()];
        assert_eq!(table(&one, &["A"], &[], &Selection::default()).events[0].attendance, Attendance::All);
        // Groups decide even for a „Vorlesung/Übung" (145529 of 2026S).
        let mixed: Vec<Fixture> = rows
            .iter()
            .cloned()
            .map(|mut r| {
                r.0.date.event_type = Some("Vorlesung/Übung".into());
                r
            })
            .collect();
        assert_eq!(options(&table(&mixed, &["A"], &[], &Selection::default()).events[0]), Some(2));
    }

    #[test]
    fn parallel_slots_are_options_when_one_covers_the_sws() {
        // 148369: an Übung of 2 SWS at four times of 90 minutes.
        let four = [
            teaching("A", "60", 1, "Übung", 1, "15:30", "17:00"),
            teaching("A", "60", 2, "Übung", 1, "17:30", "19:00"),
            teaching("A", "60", 3, "Übung", 2, "15:30", "17:00"),
            teaching("A", "60", 4, "Übung", 2, "17:30", "19:00"),
        ];
        let choice = |rows: &[Fixture], modules: &[&str], sws: &[ModuleSws]| {
            options(&table(rows, modules, sws, &Selection::default()).events[0])
        };
        let two = [sws("A", "exercise", 2.0), sws("A", "lecture", 4.0)];
        let t = table(&four, &["A"], &two, &Selection::default());
        let expected =
            Attendance::OneOf { options: vec![vec![0], vec![1], vec![2], vec![3]], basis: Basis::Sws { stated: 2.0 } };
        assert_eq!(t.events[0].attendance, expected);
        // 4 SWS: one 90-minute slot is not enough (148297). Unknown SWS decide nothing.
        assert_eq!(choice(&four, &["A"], &[sws("A", "exercise", 4.0)]), None);
        assert_eq!(choice(&four, &["A"], &[]), None);
        assert_eq!(choice(&four, &["A"], &[sws("A", "lecture", 2.0)]), None, "not the event's form");
        assert_eq!(choice(&four, &["A"], &[sws("A", "exercise", 0.0)]), None);
        // Two 90-minute slots: of 3 SWS, 2 ≥ 2.25 fails; of 1.5 SWS, 2 ≥ 1.125 and 4 > 1.875.
        assert_eq!(choice(&four[..2], &["A"], &[sws("A", "exercise", 3.0)]), None);
        assert_eq!(choice(&four[..2], &["A"], &[sws("A", "exercise", 1.5)]), Some(2));
        // Two 45-minute slots: of 1 SWS, 1 ≥ 0.75 and 2 > 1.25; of 1.6 SWS, 1 ≥ 1.2 fails.
        let short = [
            teaching("A", "61", 1, "Übung", 1, "15:30", "16:15"),
            teaching("A", "61", 2, "Übung", 2, "15:30", "16:15"),
        ];
        assert_eq!(choice(&short, &["A"], &[sws("A", "exercise", 1.0)]), Some(2));
        assert_eq!(choice(&short, &["A"], &[sws("A", "exercise", 1.6)]), None);
        // A combined type sums its forms: „Übung/Tutorium" of 1 + 1 SWS is 2.
        let combined: Vec<Fixture> = four
            .iter()
            .cloned()
            .map(|mut r| {
                r.0.date.event_type = Some("Übung/Tutorium".into());
                r
            })
            .collect();
        let parts = [sws("A", "exercise", 1.0), sws("A", "tutorial", 1.0)];
        assert_eq!(choice(&combined, &["A"], &parts), Some(4));
        let t = table(&combined, &["A"], &parts, &Selection::default());
        assert!(
            matches!(t.events[0].attendance, Attendance::OneOf { basis: Basis::Sws { stated }, .. } if stated == 2.0)
        );

        // The most any linking module states counts: 4 SWS of B keep them all required.
        let shared: Vec<Fixture> = four.iter().flat_map(|r| [r.clone(), r.clone().of("B")]).collect();
        assert_eq!(choice(&shared, &["A", "B"], &[sws("A", "exercise", 2.0), sws("B", "exercise", 4.0)]), None);
        assert_eq!(choice(&shared, &["A", "B"], &[sws("A", "exercise", 1.0), sws("B", "exercise", 2.0)]), Some(4));
        // A module that is not planned says nothing.
        assert_eq!(choice(&shared, &["A"], &[sws("A", "exercise", 2.0), sws("B", "exercise", 4.0)]), Some(4));
    }

    #[test]
    fn slots_that_are_not_parallel_stay_required() {
        let two = [sws("A", "exercise", 2.0), sws("A", "practical", 2.0), sws("A", "lecture", 2.0)];
        let choice = |rows: &[Fixture]| options(&table(rows, &["A"], &two, &Selection::default()).events[0]);
        let base = [
            teaching("A", "62", 1, "Übung", 1, "09:15", "10:45"),
            teaching("A", "62", 2, "Übung", 3, "09:15", "10:45"),
        ];
        assert_eq!(choice(&base), Some(2));
        // Lectures are never alternatives (148109, Tuesday and Thursday).
        let lecture: Vec<Fixture> = base
            .iter()
            .cloned()
            .map(|mut r| {
                r.0.date.event_type = Some("Vorlesung".into());
                r
            })
            .collect();
        assert_eq!(choice(&lecture), None);
        // Slots of two lengths.
        let mut longer = base.to_vec();
        longer[1].0.date.end_time = Some("11:30".into());
        assert_eq!(choice(&longer), None);
        // One slot after the other: October–November, then December–January.
        let apart =
            [base[0].clone().range("2026-10-05", "2026-11-30"), base[1].clone().range("2026-12-02", "2027-01-27")];
        assert_eq!(choice(&apart), None);
        // Ranges that meet, and a slot without a range (the period assumed), are parallel.
        let meeting =
            [base[0].clone().range("2026-10-05", "2026-12-07"), base[1].clone().range("2026-12-02", "2027-01-27")];
        assert_eq!(choice(&meeting), Some(2));
        assert_eq!(choice(&[base[0].clone().range("2026-10-05", "2026-11-30"), base[1].clone().undated()]), Some(2));
        // One slot in two ranges is one option; singles and rows without a time stay required.
        let mut parts = base.to_vec();
        parts.push(teaching("A", "62", 3, "Übung", 1, "09:15", "10:45").range("2026-12-07", "2027-01-25"));
        parts.push(
            teaching("A", "62", 4, "Übung", 5, "09:15", "10:45").rhythm("single").range("2027-02-05", "2027-02-05"),
        );
        let mut untimed = teaching("A", "62", 5, "Übung", 2, "09:15", "10:45");
        untimed.0.date.start_time = None;
        parts.push(untimed);
        let t = table(&parts, &["A"], &two, &Selection::default());
        let e = &t.events[0];
        assert_eq!(
            e.attendance,
            Attendance::OneOf { options: vec![vec![0, 2], vec![1]], basis: Basis::Sws { stated: 2.0 } }
        );
        assert_eq!((e.rows[3].option, e.rows[4].option), (None, None));
        // A single slot, or a named group beside the slots, is no SWS choice.
        assert_eq!(choice(&base[..1]), None);
        assert_eq!(choice(&[base[0].clone(), base[1].clone().group("1-Gruppe")]), None);
    }

    #[test]
    fn a_choice_shows_its_option_and_a_stale_one_changes_nothing() {
        let rows = [
            teaching("A", "70", 1, "Übung", 1, "15:30", "17:00"),
            teaching("A", "70", 2, "Übung", 2, "15:30", "17:00"),
            teaching("A", "70", 3, "Übung", 3, "15:30", "17:00"),
            teaching("A", "70", 4, "Übung", 5, "09:15", "10:45").rhythm("single").range("2026-10-09", "2026-10-09"),
        ];
        let two = [sws("A", "exercise", 2.0)];
        let open = table(&rows, &["A"], &two, &Selection::default());
        assert_eq!(open.events[0].visible_options(), [0, 1, 2]);
        let pick = |keys: &[RowKey]| {
            let selection = Selection { chosen_rows: keys.iter().copied().collect(), ..Selection::default() };
            table(&rows, &["A"], &two, &selection)
        };
        let t = pick(&[rows[1].key()]);
        let e = &t.events[0];
        assert_eq!(e.chosen, Some(1));
        let hidden: Vec<Option<HiddenBy>> = e.rows.iter().map(|r| r.hidden).collect();
        assert_eq!(hidden, [Some(HiddenBy::Choice), None, Some(HiddenBy::Choice), None], "the single stays");
        assert_eq!(e.visible_options(), [1]);
        assert!(!e.unresolved());
        // A key of no row (QIS changed the row), or of a required row: the choice is open again.
        for stale in [RowKey { event: 70, fp: 0x12345 }, rows[3].key(), RowKey { event: 71, fp: rows[0].key().fp }] {
            assert_eq!(pick(&[stale]), open, "{}", stale.text());
        }
        // Two chosen rows of one event (a store written by hand): the first option in row order.
        assert_eq!(pick(&[rows[2].key(), rows[0].key()]).events[0].chosen, Some(0));
        // An event whose rows are all required has no choice.
        let t = table(&rows, &["A"], &[], &Selection { chosen_rows: [rows[0].key()].into(), ..Selection::default() });
        assert_eq!((t.events[0].chosen, t.events[0].rows.iter().all(|r| r.hidden.is_none())), (None, true));
    }

    #[test]
    fn hidden_by_follows_the_order_of_the_rules() {
        let rows = [
            teaching("T", "80", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("T", "81", 1, "Übung", 2, "09:15", "10:45"),
            teaching("T", "82", 1, "Vorlesung", 3, "09:15", "10:45").campus(Some("senftenberg")),
            teaching("T", "83", 1, "Übung", 4, "09:15", "10:45").campus(Some("senftenberg")),
            teaching("T", "83", 2, "Übung", 5, "09:15", "10:45").campus(Some("senftenberg")),
        ];
        // By event id (the timetable sorts them by title).
        let reasons = |selection: &Selection| -> Vec<(String, Option<HiddenBy>, Vec<Option<HiddenBy>>)> {
            let t = table(&rows, &["T"], &[sws("T", "exercise", 2.0)], selection);
            let mut reasons: Vec<_> =
                t.events.iter().map(|e| (e.id.clone(), e.hidden, e.rows.iter().map(|r| r.hidden).collect())).collect();
            reasons.sort_by(|a, b| a.0.cmp(&b.0));
            reasons
        };
        let cottbus = Selection { town: TownChoice::Only(Town::Cottbus), ..Selection::default() };
        let town = Some(HiddenBy::Town(Town::Cottbus));
        assert_eq!(
            reasons(&cottbus),
            [
                ("80".into(), None, vec![None]),
                ("81".into(), None, vec![None]),
                ("82".into(), town, vec![town]),
                ("83".into(), town, vec![town, town])
            ]
        );
        // The event before the kinds, the kinds before the town.
        let exercise = KindSet::default().with(EventKind::Exercise);
        let all = Selection { hidden_kinds: exercise, hidden_events: [81, 82].into(), ..cottbus.clone() };
        let event = Some(HiddenBy::Event);
        let kinds = Some(HiddenBy::Kinds);
        assert_eq!(
            reasons(&all),
            [
                ("80".into(), None, vec![None]),
                ("81".into(), event, vec![event]),
                ("82".into(), event, vec![event]),
                ("83".into(), kinds, vec![kinds, kinds])
            ]
        );
        // Rows: the choice before the hidden Termin.
        let rows_only = Selection {
            town: TownChoice::Both,
            hidden_rows: [rows[3].key(), rows[4].key(), rows[0].key()].into(),
            chosen_rows: [rows[3].key()].into(),
            ..Selection::default()
        };
        let reasons = reasons(&rows_only);
        assert_eq!(reasons[0], ("80".into(), None, vec![Some(HiddenBy::Row)]));
        assert_eq!(reasons[3], ("83".into(), None, vec![Some(HiddenBy::Row), Some(HiddenBy::Choice)]));
    }

    #[test]
    fn kinds_hide_an_event_only_when_all_are_hidden() {
        let rows = [
            teaching("A", "90", 1, "Vorlesung/Übung", 1, "09:15", "10:45"),
            teaching("A", "91", 1, "Übung", 2, "09:15", "10:45"),
            teaching("A", "92", 1, "Blockseminar", 3, "09:15", "10:45"),
            teaching("A", "93", 1, "", 4, "09:15", "10:45"),
        ];
        let hidden = |kinds: &[EventKind]| -> Vec<bool> {
            let selection = Selection {
                hidden_kinds: kinds.iter().copied().fold(KindSet::default(), KindSet::with),
                ..Selection::default()
            };
            let t = table(&rows, &["A"], &[], &selection);
            rows.iter().map(|row| event(&t, &row.0.date.event_id).hidden == Some(HiddenBy::Kinds)).collect()
        };
        assert_eq!(hidden(&[EventKind::Exercise]), [false, true, false, false]);
        assert_eq!(hidden(&[EventKind::Exercise, EventKind::Lecture]), [true, true, false, false]);
        assert_eq!(
            hidden(&[EventKind::Seminar, EventKind::Other]),
            [false, false, true, true],
            "no type is „Sonstiges“"
        );
        assert_eq!(hidden(&[EventKind::Exam]), [false; 4]);
        let t = table(&rows, &["A"], &[], &Selection::default());
        let chips = t.kinds_present();
        assert_eq!(
            chips,
            [(EventKind::Lecture, 1), (EventKind::Exercise, 2), (EventKind::Seminar, 1), (EventKind::Other, 1)]
        );
    }

    #[test]
    fn a_module_has_tracks_when_each_town_holds_a_whole_course() {
        let senftenberg = Some("senftenberg");
        let rows = [
            // T: lecture and Übung in each town.
            teaching("T", "100", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("T", "101", 1, "Übung", 2, "09:15", "10:45"),
            teaching("T", "102", 1, "Vorlesung", 1, "09:15", "10:45").campus(senftenberg),
            teaching("T", "103", 1, "Übung", 2, "09:15", "10:45").campus(senftenberg),
            // L: a Cottbus lecture and a Senftenberg lab, both required (13693).
            teaching("L", "110", 1, "Vorlesung", 3, "09:15", "10:45"),
            teaching("L", "111", 1, "Laborausbildung", 3, "13:45", "15:15").campus(senftenberg),
            // M: its rows in both towns make one event of no town, and a lecture alone.
            teaching("M", "120", 1, "Vorlesung", 4, "09:15", "10:45"),
            teaching("M", "120", 2, "Vorlesung", 4, "11:30", "13:00").campus(senftenberg),
            teaching("M", "121", 1, "Übung", 5, "09:15", "10:45").campus(None),
        ];
        let t = table(&rows, &["T", "L", "M"], &[], &Selection::default());
        assert_eq!(t.tracks, BTreeSet::from(["T".to_string()]));
        assert_eq!(event(&t, "120").town(), None);
        assert_eq!(event(&t, "121").town(), None);
        // L's Cottbus lecture against its Senftenberg lab: a tie, so no town is derived.
        assert_eq!((t.town, t.town_derived), (None, false));
        assert!(t.events.iter().all(|e| e.hidden.is_none()));
        // Another Cottbus module decides: T's Senftenberg course goes.
        let mut more = rows.to_vec();
        more.push(teaching("C", "130", 1, "Seminar", 5, "13:45", "15:15"));
        let t = table(&more, &["T", "L", "M", "C"], &[], &Selection::default());
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true));
        let hidden: Vec<&str> = t.events.iter().filter(|e| e.hidden.is_some()).map(|e| e.id.as_str()).collect();
        assert_eq!(hidden, ["102", "103"]);
        // A chosen town wins and is not derived; both towns show everything.
        let chosen = Selection { town: TownChoice::Only(Town::Senftenberg), ..Selection::default() };
        let t = table(&more, &["T", "L", "M", "C"], &[], &chosen);
        assert_eq!((t.town, t.town_derived), (Some(Town::Senftenberg), false));
        let hidden: Vec<&str> = t.events.iter().filter(|e| e.hidden.is_some()).map(|e| e.id.as_str()).collect();
        assert_eq!(hidden, ["100", "101"]);
        let both = Selection { town: TownChoice::Both, ..Selection::default() };
        assert!(table(&more, &["T", "L", "M", "C"], &[], &both).events.iter().all(|e| e.hidden.is_none()));
        // An event T shares with a module without tracks stays in either town.
        let mut shared = more.clone();
        shared.push(teaching("C", "103", 1, "Übung", 2, "09:15", "10:45").campus(senftenberg));
        let cottbus = Selection { town: TownChoice::Only(Town::Cottbus), ..Selection::default() };
        let t = table(&shared, &["T", "L", "M", "C"], &[], &cottbus);
        assert_eq!(event(&t, "103").hidden, None);
        assert_eq!(event(&t, "102").hidden, Some(HiddenBy::Town(Town::Cottbus)));
        // The town is derived without any module with tracks too (the finder needs it).
        let t = table(&more[4..], &["L", "M", "C"], &[], &Selection::default());
        assert!(t.tracks.is_empty());
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true));
    }

    /// The events the design quotes, on the pinned snapshot; on any snapshot, the invariants of the
    /// first sixty modules with a dated row in the current semester.
    #[test]
    fn alternatives_are_decided_by_groups_and_sws() {
        let pinned = crate::tests::studyplan_db("alternatives_are_decided_by_groups_and_sws");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let current = queries::meta(&db).unwrap().current_semester.unwrap();
        let mut many: Vec<String> = Vec::new();
        for row in queries::semester_schedule(&db, &current).unwrap() {
            if many.len() < MAX_MODULES && !many.contains(&row.module_id) {
                many.push(row.module_id);
            }
        }
        let t = planned(&db, &current, &many, &Selection::default());
        assert!(t.events.iter().any(|e| e.rows.iter().any(|r| !r.occ.days.is_empty())));
        if !is_pinned {
            return;
        }

        let events = [
            "148369", "148370", "148304", "148130", "148701", "148109", "148297", "152844", "149058", "151919",
            "149132",
        ];
        let linking = crate::tests::column(
            &db,
            &format!(
                "SELECT DISTINCT module_id FROM v_module_schedule WHERE semester_key = '2026W' AND event_id IN ({}) \
                 ORDER BY module_id",
                events.map(|e| format!("'{e}'")).join(",")
            ),
        );
        assert!(!linking.is_empty() && linking.len() <= MAX_MODULES, "{linking:?}");
        let t = planned(&db, "2026W", &linking, &Selection::default());
        let decided: Vec<(&str, Option<usize>)> = events.iter().map(|id| (*id, options(event(&t, id)))).collect();
        assert_eq!(
            decided,
            [
                ("148369", Some(4)),
                ("148370", Some(4)),
                ("148304", Some(3)),
                ("148130", Some(2)),
                ("148701", None),
                ("148109", None),
                ("148297", None),
                ("152844", None),
                ("149058", None),
                ("151919", None),
                ("149132", None)
            ]
        );
        assert!(matches!(event(&t, "148130").attendance, Attendance::OneOf { basis: Basis::Groups, .. }));
        let Attendance::OneOf { basis, .. } = &event(&t, "148369").attendance else {
            panic!("148369 is a choice");
        };
        assert_eq!(*basis, Basis::Sws { stated: 2.0 });
        assert!(["148369", "148370", "148304", "148130"].iter().all(|id| event(&t, id).unresolved()));

        // „Nur diesen" on Monday 17:30 (148369-a4d12): that option, the other three rows hidden.
        let chosen = RowKey::parse("148369-a4d12").unwrap();
        let selection = Selection { chosen_rows: [chosen].into(), ..Selection::default() };
        let picked = planned(&db, "2026W", &linking, &selection);
        let e = event(&picked, "148369");
        let rows: Vec<(String, Option<HiddenBy>)> = e.rows.iter().map(|r| (r.key.unwrap().text(), r.hidden)).collect();
        let choice = Some(HiddenBy::Choice);
        assert_eq!(
            rows,
            [
                ("148369-aaf38".into(), choice),
                ("148369-a4d12".into(), None),
                ("148369-467cf".into(), choice),
                ("148369-f09d2".into(), choice)
            ]
        );
        assert_eq!((e.chosen, e.unresolved()), (Some(1), false));
        assert_eq!(picked.events.len(), t.events.len());
        for (before, after) in t.events.iter().zip(&picked.events).filter(|(e, _)| e.id != "148369") {
            assert_eq!(before, after, "only 148369 changes");
        }
        // A chosen key of no row changes nothing.
        let stale = Selection { chosen_rows: [RowKey { event: 148369, fp: 0x12345 }].into(), ..Selection::default() };
        assert_eq!(planned(&db, "2026W", &linking, &stale), t);
    }

    #[test]
    fn city_tracks_are_whole_courses() {
        let Some(db) = crate::tests::studyplan_db("city_tracks_are_whole_courses") else {
            return;
        };
        let with = ["11107", "11206", "11826", "12104", "12105"];
        let without = ["13693", "11777", "11779", "11760", "12102", "11536"];
        let modules: Vec<String> = ids(&with).into_iter().chain(ids(&without)).collect();
        let t = planned(&db, "2026W", &modules, &Selection::default());
        assert_eq!(t.tracks, ids(&with).into_iter().collect::<BTreeSet<_>>());

        // Informatik's first semester: taught in Cottbus, so 12104's Senftenberg course goes.
        let fs1 = ids(&["12104", "12107", "12102", "11112"]);
        let t = planned(&db, "2026W", &fs1, &Selection::default());
        assert_eq!(t.tracks, BTreeSet::from(["12104".to_string()]));
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true));
        let hidden: Vec<(&str, Option<HiddenBy>)> =
            t.events.iter().filter(|e| e.hidden.is_some()).map(|e| (e.id.as_str(), e.hidden)).collect();
        let town = Some(HiddenBy::Town(Town::Cottbus));
        assert_eq!(hidden, [("149406", town), ("149407", town)]);
        let exam = t.exams.iter().find(|e| e.event_id == "150664").unwrap();
        assert_eq!(exam.hidden, town);
        assert!(t.exams.iter().filter(|e| e.event_id != "150664").all(|e| e.hidden.is_none()));
        // Senftenberg chosen: the Cottbus course and exam go instead; both towns show all.
        let senftenberg = Selection { town: TownChoice::Only(Town::Senftenberg), ..Selection::default() };
        let t = planned(&db, "2026W", &fs1, &senftenberg);
        let hidden: Vec<&str> = t.events.iter().filter(|e| e.hidden.is_some()).map(|e| e.id.as_str()).collect();
        assert_eq!(hidden, ["148369", "148701"]);
        let both = Selection { town: TownChoice::Both, ..Selection::default() };
        let t = planned(&db, "2026W", &fs1, &both);
        assert!(t.events.iter().all(|e| e.hidden.is_none()) && t.exams.iter().all(|e| e.hidden.is_none()));
        assert_eq!(t.town, None);
    }
}
