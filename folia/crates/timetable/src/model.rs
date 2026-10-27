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
//! Eight modules of 2026W are held as two whole courses at two campuses or more, each with its
//! lecture and its Übung (docs/radix/schema-v2.md, „Tracks"): Entwicklung von Softwaresystemen
//! (12104) at the Zentralcampus and in Senftenberg, the Programmierpraktikum (12102) at the
//! Zentralcampus and in Sachsendorf. A student takes one of them, so the events of the others are
//! hidden: of the courses the plan's program takes (owner, 2026-10-19: „nur die termine anzeigen,
//! die den eingerichteten Studiengang entsprechend"), within the town the visitor chose where
//! one is; else of the town the visitor chose; else of the town most of the plan's other modules
//! are taught in. „Beide" shows every course. Radix reads the courses (`EventDate::track`): a module
//! whose Cottbus lecture and Senftenberg lab are both required (13693) has none, and neither has
//! 11760, whose two campuses hold its two required parts. Where the rows carry no courses at all
//! (a snapshot before schema 15), a module with a lecture and a non-lecture event in each town has
//! two towns' courses, as before.
//!
//! Radix also reads which events of a module one attends some of, not all (docs/radix/schema-v2.md,
//! „Choices"; `Input::choices`): one of the three exercises of Höhere Mathematik T3, each for other
//! programs, one of 14 projects, a group. Such a choice across a module's events is `Across`: what
//! the visitor chose („Nur diesen" on a Termin of an option) is taken, else, where the options are
//! for other programs, the one QIS lists the plan's program at (owner, 2026-10-19, the concept of
//! folia/design/wahlangebote/konzept.html: „Was QIS einem Studiengang zuordnet, wählt Betula für
//! ihn vor. Alles andere ist eine offene Wahl, bei der nichts verschwindet, bis man selbst gewählt
//! hat"); the options left out are hidden (`HiddenBy::Option`, `HiddenBy::Program`). An open choice
//! shows every option, and its options meet nothing as long as one of them is free (`clash`). An
//! extra beside a module's compulsory part (`Event::extra`) is attended by whoever wants to: it
//! meets nothing. A module only visited (`Selection::visits`, „nur besuchen") stands in the week
//! like any other, but its events meet nothing in a clash and give way where they meet the plan's
//! (`Event::visit`, `clash::yield_pairs`), and it brings no exam.
//!
//! The Termine of one's own the visitor added (`Selection::own`, folia/design/kalender/termine.html)
//! are events of their own (`Event::own`): rows as QIS's of their rhythm would be, keyed by their
//! own key, linked by no module, so that they meet every Termin of the plan, firm as a module's
//! (owner, 2026-10-27: „Passt alles", the third question's „fest, wie ein Modul").
//!
//! Then the visitor's selection: hidden events, kinds and Termine, and made choices. Every event
//! and row says why it is not shown (`HiddenBy`, the first rule that holds). The clashes of what
//! is left are `clash`'s, the exams and their warnings `exams`'.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::{clock, Day};
use folia_calendar::kind::{class_of, kinds_of, Class, EventKind, KindSet};
use folia_calendar::own::{OwnRhythm, OwnTermin};
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::{town_of, town_of_campus, HiddenBy, Selection, Town, TownChoice};
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Campus, ChoiceKind, Code, Rhythm};
use folia_model::rows::Semester;
use folia_model::rows_detail::{ChoiceFamily, ChoiceOption, DateRow, EventDate, ModuleSws, TrackProgram};

use crate::clash::{self, Clash, Weeks};
use crate::exams::{self, Exam, ExamWarning};
use crate::facts::SemesterFacts;
use crate::occur::{self, Every, Occurrences};

/// The group QIS gives a row that belongs to no group.
pub const UNNAMED_GROUP: &str = "[unbenannt]";

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
    /// The courses the plan's program takes of the modules held as courses at two campuses or more
    /// (`modules_tracks`); empty without a program.
    pub taken: &'a [TrackProgram],
    /// The choices among the planned modules' events (`modules_choices`), a row per event of an
    /// option; empty before schema 17.
    pub choices: &'a [ChoiceOption],
    /// The options QIS lists the plan's program at (`modules_choice_listed`): of a choice whose
    /// options are for other programs, those it takes. Empty without a program.
    pub listed: &'a [ChoiceFamily],
}

/// One semester of a Studienplan, with the visitor's selection applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Timetable {
    pub key: SemesterKey,
    pub facts: SemesterFacts,
    /// The planned module ids, in plan order, each once.
    pub modules: Vec<String>,
    /// Those of them only visited (`Selection::visits`): their events give way (`Event::visit`),
    /// and they bring no exam.
    pub visits: BTreeSet<String>,
    /// By the plan position of the first module that links them, then title, then event id as a
    /// number.
    pub events: Vec<Event>,
    /// The planned modules' exams (`exams::exams_of`).
    pub exams: Vec<Exam>,
    /// The event ids of the exams left out as those of visits alone: a subscription hides them
    /// (`export::subscription_of`), so that the feed shows what the page shows.
    pub visit_exams: Vec<String>,
    /// The planned modules held as courses at two places, of which a student attends one: those
    /// with courses (`courses`), and, where the rows carry none, those with a lecture and a
    /// non-lecture event in each town.
    pub tracks: BTreeSet<String>,
    /// The campuses of the courses of each planned module that Radix reads them for (two or more).
    pub courses: BTreeMap<String, BTreeSet<Campus>>,
    /// Of the modules with tracks, which courses are shown and what decided it; a module not in
    /// here shows all of them.
    pub shown: BTreeMap<String, Shown>,
    /// The town the visitor chose or most of the plan's other modules are taught in (the town a
    /// module of two towns' courses is taken in where nothing else decides); `None` for both.
    pub town: Option<Town>,
    /// The town was derived from the plan's other modules, not chosen.
    pub town_derived: bool,
    /// The choices among the planned modules' events, in the order of the modules and their
    /// choices (`Across`).
    pub choices: Vec<Across>,
    /// Hard teaching clashes (`clash::clashes`).
    pub clashes: Vec<Clash>,
    /// Open choices (`Event::unresolved`) that cannot take an option free of clashes, by event
    /// index (`clash::clashes`). A clash names each of them; the rows of all hard clashes are
    /// `clash::hard_rows`.
    pub blocked: Vec<usize>,
    /// The same of the open choices among a module's events, by index into `choices`.
    pub blocked_choices: Vec<usize>,
    /// Exam warnings (`exams::exam_warnings`).
    pub exam_warnings: Vec<ExamWarning>,
    /// Days on which a hop between exams cannot be judged, with their modules.
    pub place_unknown: Vec<(Day, Vec<String>)>,
    /// Planned modules without a dated teaching row this semester, in plan order.
    pub without_dates: Vec<String>,
}

/// A choice among the events of a planned module (docs/radix/schema-v2.md, „Choices"): of its
/// options a student attends `pick`, each option one event or more.
#[derive(Clone, Debug, PartialEq)]
pub struct Across {
    pub module: String,
    /// Its number in the module's semester (`v_module_choice.choice`).
    pub number: u32,
    pub kind: ChoiceKind,
    pub pick: usize,
    /// Per option its events, by index into `Timetable::events`, and its number
    /// (`v_module_choice.option`), in the order of the numbers.
    pub options: Vec<Vec<usize>>,
    pub numbers: Vec<u32>,
    /// The options taken, and what took them: the visitor's choice (a Termin of an option in
    /// `Selection::chosen_rows`), else the plan's program. Fewer than `pick` leave the choice open
    /// among the others.
    pub taken: BTreeSet<usize>,
    pub by: Option<TakenBy>,
    /// The options the plan's program narrows the choice to, where QIS lists it at more of them
    /// than one takes: the others are hidden (`HiddenBy::Program`).
    pub narrowed: Option<BTreeSet<usize>>,
}

/// What took the options of a choice among a module's events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakenBy {
    /// The visitor chose them („Nur diesen").
    Chosen,
    /// QIS lists the plan's program at them, and at no other option.
    Program,
}

impl Across {
    /// The options with a Termin shown, in order.
    pub fn visible(&self, events: &[Event]) -> Vec<usize> {
        let shown = |event: &usize| events.get(*event).is_some_and(|event| event.hidden.is_none() && (event.rows.is_empty() || event.rows.iter().any(|row| row.hidden.is_none())));
        self.options.iter().enumerate().filter(|(_, events)| events.iter().any(shown)).map(|(option, _)| option).collect()
    }

    /// Whether the student still picks among its options: more of them are shown than one takes,
    /// or than were ticked, where the visitor ticked more than one takes (all of them alike,
    /// owner, 2026-10-26).
    pub fn open(&self, events: &[Event]) -> bool {
        self.visible(events).len() > self.pick.max(self.taken.len())
    }

    /// The options still to choose among: shown, not taken; empty when the choice is not open.
    pub fn choosable(&self, events: &[Event]) -> Vec<usize> {
        match self.open(events) {
            true => self.visible(events).into_iter().filter(|option| !self.taken.contains(option)).collect(),
            false => Vec::new(),
        }
    }

    /// How many of the options to choose among one still takes („2 von 5"): 0 when the choice is
    /// not open.
    pub fn take(&self, events: &[Event]) -> usize {
        self.pick.saturating_sub(self.taken.len()).min(self.choosable(events).len())
    }
}

/// The option of a choice among its module's events an event is (`Timetable::choices`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alt {
    /// The choice, by index into `Timetable::choices`, and the option, by index into its options.
    pub choice: usize,
    pub option: usize,
    /// The choice is open and this option is not taken: one to choose among („1 von 3").
    pub open: bool,
    /// How many of the options to choose among one still takes; 0 where `open` is not.
    pub take: usize,
}

/// Which courses of a module with tracks a timetable shows, and what decided it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// The courses shown, by campus (`Event::course`); empty for a module of two towns' courses
    /// whose rows carry none, which `towns` decides.
    pub campuses: BTreeSet<Campus>,
    /// The towns of the courses shown: an exam of the module is shown where it is held in one.
    pub towns: BTreeSet<Town>,
    pub by: ShownBy,
}

/// What decided the courses of a module a timetable shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownBy {
    /// The plan's program takes them (`Input::taken`).
    Program,
    /// The town the visitor chose.
    Chosen,
    /// The town most of the plan's other modules are taught in.
    Derived,
}

impl Shown {
    /// Whether the event is shown for this module: it is one of the courses shown, or in no
    /// course; of two towns' courses, held in a town shown or in none known.
    pub fn shows(&self, event: &Event) -> bool {
        match (event.course, self.campuses.is_empty()) {
            (Some(course), false) => self.campuses.contains(&course),
            (None, false) => true,
            (_, true) => event.town().is_none_or(|town| self.towns.contains(&town)),
        }
    }
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
    /// The campus of the course of its own it is part of for every planned module that links it
    /// (`EventDate::track`); `None` for an event in no course.
    pub course: Option<Campus>,
    /// 1..=8, from the plan position of its first module.
    pub tone: u8,
    pub attendance: Attendance,
    /// The option of a choice among its module's events it is (`Across`); `None` for most.
    pub across: Option<Alt>,
    /// An extra beside its module's compulsory part (`EventDate::extra`): attended by whoever
    /// wants to, it meets nothing (`clash`).
    pub extra: bool,
    /// Every planned module that links it is only visited (`Selection::visits`): it meets nothing
    /// in a clash, and where it meets the plan's Termine it gives way (`clash::yield_pairs`; owner,
    /// 2026-10-26: „ein Besuch gibt bei einer Überschneidung nach"). An event a planned module
    /// shares with a visit is the plan's.
    pub visit: bool,
    /// The options of `Attendance::OneOf` the visitor ticked (`Selection::chosen_rows`), as many as
    /// they like, each shown and counted alike (owner, 2026-10-26); empty while the choice is open.
    pub chosen: BTreeSet<usize>,
    /// Its dated rows by `ord`; empty for an event without dates („ohne Termine").
    pub rows: Vec<Row>,
    pub hidden: Option<HiddenBy>,
    pub source_url: Option<String>,
    /// The Termin of one's own it is (`Selection::own`): no module links it (`modules` is empty),
    /// so it meets every Termin of the plan and a choice takes it as one more; its `tone` is the
    /// tone of the module it belongs to where that is in the week, else 0, none. `None` for every
    /// event of QIS.
    pub own: Option<OwnTermin>,
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
    /// The rhythm the row is shown in (`Every::shown`): QIS's, or for a row of a series of single
    /// dates (`EventDate::series`) the series'. Its dates, clashes and choices follow QIS's rhythm
    /// (`Every::of`), in which every row of a series is a date of its own.
    pub every: Option<Every>,
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
    /// wählen"): nothing is ticked. Ticked options are the plan's, however many there are.
    pub fn unresolved(&self) -> bool {
        self.chosen.is_empty() && self.visible_options().len() >= 2
    }

    /// The keys of an option's Termine, each once, in order: one of them ticks the option
    /// (`Selection::chosen_rows`), and all of them taken away untick it.
    pub fn option_keys(&self, option: usize) -> Vec<RowKey> {
        let mut keys: Vec<RowKey> = Vec::new();
        for key in self.rows.iter().filter(|row| row.option == Some(option)).filter_map(|row| row.key) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
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
    /// The weekday a recurring row (`every`) stands on in the Regelwoche: its pattern's, else its
    /// first held day's; a row of a series of single dates whose date QIS cancels keeps its date's,
    /// so that its series' range holds it.
    pub fn slot_day(&self) -> Option<u8> {
        let held = self.occ.template.map(|pattern| pattern.weekday).or_else(|| self.occ.days.first().map(|day| day.weekday()));
        held.or_else(|| self.date.series.and(self.occ.cancelled.first()).map(|(day, ..)| day.weekday()))
    }

    fn of(row: &DateRow, facts: &SemesterFacts) -> Row {
        let times = occur::times(&row.date);
        Row {
            key: RowKey::of(&row.date),
            ord: row.ord,
            date: row.date.clone(),
            cancelled_dates: row.cancelled_dates.clone(),
            occ: occur::occurrences(row, facts),
            every: Every::shown(&row.date, facts),
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
        let visits: BTreeSet<String> = modules.iter().filter(|module| selection.visits.contains(*module)).cloned().collect();
        let mut events = gather(input.schedule, &modules, input.facts);
        for event in &mut events {
            decide(event, input.sws);
            event.visit = !event.modules.is_empty() && event.modules.iter().all(|module| visits.contains(module));
        }
        let mut choices = choices_of(input.choices, &mut events, &modules, input.key);
        let courses = courses(input.schedule, &modules);
        let tracks = tracks(&events, &modules, &courses);
        let (town, town_derived) = effective_town(selection.town, &events, &tracks, selection.town_from.as_ref());
        let shown = shown(&tracks, &courses, &taken(input.taken, &courses), selection.town, town, town_derived);
        for event in &mut events {
            event.hidden = event_hidden(event, selection, &shown);
            choose(event, selection);
        }
        take(&mut choices, &mut events, input.listed, selection);
        events.extend(selection.own.iter().filter(|own| own.is_valid()).map(|own| own_event(own, &modules, input.facts)));
        let (clashes, blocked, blocked_choices) = clash::clashes(&events);
        let towns: BTreeMap<String, BTreeSet<Town>> = shown.iter().map(|(module, shown)| (module.clone(), shown.towns.clone())).collect();
        // A visit brings no exam: nobody sits one of a module they only visit.
        let (visit_exams, exams): (Vec<Exam>, Vec<Exam>) =
            exams::exams_of(input.exams, input.semester, &modules, selection, &towns).into_iter().partition(|exam| exam.modules.iter().all(|module| visits.contains(module)));
        let visit_exams = visit_exams.into_iter().map(|exam| exam.event_id).collect();
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
            visits,
            events,
            exams,
            visit_exams,
            tracks,
            courses,
            shown,
            town,
            town_derived,
            choices,
            clashes,
            blocked,
            blocked_choices,
            exam_warnings,
            place_unknown,
            without_dates,
        }
    }

    /// The kinds of the semester's events and exams with the number of events of each, hidden
    /// ones included, in the kinds' order: the chips „Vorlesung 5", „Übung 4", „Prüfung 4". A
    /// „Vorlesung/Übung" counts for both. Another course of a module with tracks is not the
    /// visitor's, whatever else hides it: a chip counts what its click can show, and Senftenberg's
    /// student of 11107 has no Tutorium to show (only the Cottbus course has one).
    pub fn kinds_present(&self) -> Vec<(EventKind, usize)> {
        EventKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let count = match kind {
                    EventKind::Exam => self.exams.iter().filter(|exam| !self.theirs_exam(exam)).count(),
                    _ => self.events.iter().filter(|event| event.kinds.contains(kind) && !self.theirs(event)).count(),
                };
                (count > 0).then_some((kind, count))
            })
            .collect()
    }

    /// The planned modules with tracks in both towns between which nothing decides — not the
    /// program, not a town chosen or derived: what „Standort offen" asks about. A module whose
    /// courses are all in one town (12102 at the Zentralcampus and in Sachsendorf) shows them all
    /// until its program decides, and is not asked about: the visitor chooses towns, not campuses.
    pub fn towns_open(&self) -> Vec<&String> {
        self.tracks
            .iter()
            .filter(|module| !self.shown.contains_key(*module))
            .filter(|module| self.courses.get(*module).is_none_or(|held| held.iter().map(|campus| town_of_campus(*campus)).collect::<BTreeSet<_>>().len() >= 2))
            .collect()
    }

    /// Whether the event is another course than the one its modules are taken in, whatever else
    /// hides it: every module that links it has tracks, and none of them shows it.
    pub fn theirs(&self, event: &Event) -> bool {
        !event.modules.is_empty() && event.modules.iter().all(|module| self.shown.get(module).is_some_and(|shown| !shown.shows(event)))
    }

    /// Whether the exam is another course's: every module that links it has tracks, and the exam
    /// is held in a town none of them is taken in.
    pub fn theirs_exam(&self, exam: &Exam) -> bool {
        let Some(own) = exam.town() else { return false };
        !exam.modules.is_empty() && exam.modules.iter().all(|module| self.shown.get(module).is_some_and(|shown| !shown.towns.contains(&own)))
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
    /// The plan positions of an event's modules, its rows (each `ord` once), and the courses it is
    /// part of (`EventDate::track`) for each of them.
    type Gathered<'a> = (BTreeSet<usize>, Vec<&'a DateRow>, BTreeSet<Option<Campus>>);
    let mut by_event: BTreeMap<&str, Gathered<'_>> = BTreeMap::new();
    let mut seen: BTreeSet<(&str, Option<i64>)> = BTreeSet::new();
    for row in schedule {
        let Some(position) = positions.get(row.module_id.as_str()) else {
            continue;
        };
        let id = row.date.event_id.as_str();
        let (linked, rows, courses) = by_event.entry(id).or_default();
        linked.insert(*position);
        courses.insert(row.date.track.as_ref().and_then(|track| track.known()));
        if seen.insert((id, row.ord)) {
            rows.push(row);
        }
    }

    let mut events: Vec<(usize, Event)> = Vec::with_capacity(by_event.len());
    for (id, (linked, mut rows, courses)) in by_event {
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
            // One course for every module: the event is that course; one module without, anyone's.
            course: if courses.len() == 1 { courses.first().copied().flatten() } else { None },
            tone: tone(position),
            attendance: Attendance::All,
            across: None,
            extra: rows.iter().any(|row| row.date.extra),
            visit: false,
            chosen: BTreeSet::new(),
            // A row without `ord` is the view's line for an event without dates: no row at all.
            rows: rows.iter().filter(|row| row.ord.is_some()).map(|row| Row::of(row, facts)).collect(),
            hidden: None,
            source_url: date.source_url.clone(),
            own: None,
        };
        events.push((position, event));
    }
    events.sort_by(|(p, a), (q, b)| {
        p.cmp(q).then_with(|| a.title.cmp(&b.title)).then_with(|| (a.id.len(), &a.id).cmp(&(b.id.len(), &b.id)))
    });
    events.into_iter().map(|(_, event)| event).collect()
}

/// The event of a Termin of one's own: a row as QIS's of its rhythm would be — every week, or in
/// the A or the B weeks, from its first day to its last, else through the lecture period
/// (`occur`) — or a single date for each of its dates; keyed by its own key, the days it skips
/// taken out of its days as a hidden date is (`Occurrences::hidden`). Its tone is the module's it
/// belongs to where that is in the week (`modules`), else 0.
fn own_event(own: &OwnTermin, modules: &[String], facts: &SemesterFacts) -> Event {
    let key = own.key();
    let date = |weekday: u8, rhythm: Rhythm, range: Option<(Day, Day)>| EventDate {
        semester_key: facts.key.key(),
        semester_label: String::new(),
        event_id: key.event.to_string(),
        event_number: None,
        event_title: own.name.clone(),
        event_type: None,
        group_name: None,
        weekday: Some(i64::from(weekday)),
        start_time: Some(clock(own.from)),
        end_time: Some(clock(own.to)),
        rhythm: Some(Code::Known(rhythm)),
        rhythm_raw: None,
        first_date: range.map(|(first, _)| first.iso()),
        last_date: range.map(|(_, last)| last.iso()),
        room: Some(own.place.clone()).filter(|place| !place.is_empty()),
        campus: None,
        instructor: None,
        comment: None,
        source_url: None,
        room_short: None,
        series: None,
        track: None,
        choice: None,
        option: None,
        extra: false,
    };
    let dates = match own.rhythm {
        OwnRhythm::Dates => own.dates.iter().map(|day| date(day.weekday(), Rhythm::Single, Some((*day, *day)))).collect(),
        OwnRhythm::Week => vec![date(own.weekday, Rhythm::Weekly, own.period)],
        OwnRhythm::AWeek | OwnRhythm::BWeek => {
            let (weeks, rhythm) = if own.rhythm == OwnRhythm::AWeek { (Weeks::A, Rhythm::WeekA) } else { (Weeks::B, Rhythm::WeekB) };
            // A period of its own begins on its first day of its weeks; without the A weeks
            // known, it is a pattern (`occur`).
            let range = own.period.and_then(|(first, last)| Some((first_in(weeks, own.weekday, first, facts)?, last)));
            vec![date(own.weekday, rhythm, range)]
        }
    };
    let rows = dates
        .into_iter()
        .enumerate()
        .map(|(ord, date)| {
            let mut row = Row::of(&DateRow { module_id: String::new(), ord: i64::try_from(ord).ok(), cancelled_dates: None, date }, facts);
            row.key = Some(key);
            let (gone, kept): (Vec<Day>, Vec<Day>) = row.occ.days.iter().partition(|day| own.skip.contains(day));
            row.occ.days = kept;
            row.occ.hidden = gone;
            row
        })
        .collect();
    let tone = own.module.as_ref().and_then(|module| modules.iter().position(|planned| planned == module)).map_or(0, tone);
    Event {
        id: key.event.to_string(),
        number: None,
        title: own.name.clone(),
        type_raw: None,
        kinds: KindSet::default(),
        class: Class::Other,
        modules: Vec::new(),
        course: None,
        tone,
        attendance: Attendance::All,
        across: None,
        extra: false,
        visit: false,
        chosen: BTreeSet::new(),
        rows,
        hidden: None,
        source_url: None,
        own: Some(own.clone()),
    }
}

/// The first day on or after `from` on `weekday` in the A weeks or the B weeks (`weeks`), counted
/// as `SemesterFacts::ab_week` counts them; `None` where the A weeks are unknown.
fn first_in(weeks: Weeks, weekday: u8, from: Day, facts: &SemesterFacts) -> Option<Day> {
    let a_week = facts.a_week?.monday();
    let theirs = if weeks == Weeks::B { a_week.plus(7) } else { a_week };
    let monday = from.monday();
    let day = monday.plus((theirs.0 - monday.0).rem_euclid(14)).plus(i32::from(weekday) - 1);
    Some(if day < from { day.plus(14) } else { day })
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

/// The campuses of the courses of each planned module whose rows carry them (`EventDate::track`),
/// where they are two or more.
fn courses(schedule: &[DateRow], modules: &[String]) -> BTreeMap<String, BTreeSet<Campus>> {
    let mut courses: BTreeMap<String, BTreeSet<Campus>> = BTreeMap::new();
    for row in schedule.iter().filter(|row| modules.contains(&row.module_id)) {
        if let Some(campus) = row.date.track.as_ref().and_then(|track| track.known()) {
            courses.entry(row.module_id.clone()).or_default().insert(campus);
        }
    }
    courses.retain(|_, campuses| campuses.len() >= 2);
    courses
}

/// The planned modules with tracks: those with courses, and, of those whose rows carry none, those
/// whose two towns each hold a lecture event and a non-lecture event of the module (only events
/// whose rows are in one town count) — the reading before Radix read courses (schema 15). The
/// reading „both towns together hold both kinds" would also mark 13693, whose Cottbus lecture and
/// Senftenberg lab are both required.
fn tracks(events: &[Event], modules: &[String], courses: &BTreeMap<String, BTreeSet<Campus>>) -> BTreeSet<String> {
    let towns: Vec<Option<Town>> = events.iter().map(Event::town).collect();
    modules
        .iter()
        .filter(|module| {
            if courses.contains_key(*module) {
                return true;
            }
            let linking = || events.iter().zip(&towns).filter(|(event, _)| event.modules.contains(module));
            if linking().any(|(event, _)| event.course.is_some()) {
                return false;
            }
            let held: BTreeSet<(Town, bool)> = linking().filter_map(|(event, town)| Some(((*town)?, event.class == Class::Lecture))).collect();
            [Town::Cottbus, Town::Senftenberg]
                .into_iter()
                .all(|town| held.contains(&(town, true)) && held.contains(&(town, false)))
        })
        .cloned()
        .collect()
}

/// The courses the program takes, per module with courses (`Input::taken`).
fn taken(rows: &[TrackProgram], courses: &BTreeMap<String, BTreeSet<Campus>>) -> BTreeMap<String, BTreeSet<Campus>> {
    let mut taken: BTreeMap<String, BTreeSet<Campus>> = BTreeMap::new();
    for row in rows {
        if let Some(campus) = row.campus.known().filter(|campus| courses.get(&row.module_id).is_some_and(|held| held.contains(campus))) {
            taken.entry(row.module_id.clone()).or_default().insert(campus);
        }
    }
    taken
}

/// Which courses of each module with tracks are shown, and what decided it: „Beide" shows all; a
/// town chosen shows the courses in it, of them those the program takes where it takes one there;
/// else the courses the program takes; else those in the town most of the plan's other modules
/// are taught in. A module whose courses are all shown is not in the result, nor is one of two
/// towns' courses while no town decides.
fn shown(
    tracks: &BTreeSet<String>,
    courses: &BTreeMap<String, BTreeSet<Campus>>,
    taken: &BTreeMap<String, BTreeSet<Campus>>,
    choice: TownChoice,
    town: Option<Town>,
    derived: bool,
) -> BTreeMap<String, Shown> {
    let mut shown = BTreeMap::new();
    for module in tracks {
        let Some(held) = courses.get(module) else {
            // Two towns' courses without courses from Radix: the town decides.
            if let Some(town) = town {
                let by = if derived { ShownBy::Derived } else { ShownBy::Chosen };
                shown.insert(module.clone(), Shown { campuses: BTreeSet::new(), towns: BTreeSet::from([town]), by });
            }
            continue;
        };
        let in_town = |town: Town| -> BTreeSet<Campus> { held.iter().copied().filter(|campus| town_of_campus(*campus) == town).collect() };
        let program = taken.get(module).filter(|taken| !taken.is_empty());
        let decided = match choice {
            TownChoice::Both => None,
            TownChoice::Only(chosen) => {
                let there = in_town(chosen);
                let theirs: BTreeSet<Campus> = program.map(|taken| taken.intersection(&there).copied().collect()).unwrap_or_default();
                match (there.is_empty(), theirs.is_empty()) {
                    (true, _) => None,
                    (false, false) => Some((theirs, ShownBy::Program)),
                    (false, true) => Some((there, ShownBy::Chosen)),
                }
            }
            TownChoice::Derive => match program {
                Some(taken) => Some((taken.clone(), ShownBy::Program)),
                None => town.map(in_town).filter(|there| !there.is_empty()).map(|there| (there, ShownBy::Derived)),
            },
        };
        if let Some((campuses, by)) = decided.filter(|(campuses, _)| campuses != held) {
            let towns = campuses.iter().map(|campus| town_of_campus(*campus)).collect();
            shown.insert(module.clone(), Shown { campuses, towns, by });
        }
    }
    shown
}

/// The town a module with tracks is taken in, and whether it was derived: the chosen one; none
/// for both; derived, the town that holds strictly more of the located events of the plan's
/// modules without tracks, else none. It is derived even when no planned module has tracks: the
/// finder resolves a candidate's tracks with it. `from` narrows the modules it is derived from
/// to those taken over from the Regelstudienplan (`Selection::town_from`): one elective of the
/// other town planned beside them then does not turn it, nor does a candidate of the finder. A
/// visit's events (`Event::visit`) count for nothing: they give way, the town too.
fn effective_town(choice: TownChoice, events: &[Event], tracks: &BTreeSet<String>, from: Option<&BTreeSet<String>>) -> (Option<Town>, bool) {
    match choice {
        TownChoice::Only(town) => (Some(town), false),
        TownChoice::Both => (None, false),
        TownChoice::Derive => {
            let (mut cottbus, mut senftenberg) = (0usize, 0usize);
            let counts = |module: &String| !tracks.contains(module) && from.is_none_or(|from| from.contains(module));
            for event in events.iter().filter(|event| !event.visit && event.modules.iter().any(counts)) {
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
/// „Vorlesung/Übung" stays while lectures are shown); another course than the one its modules
/// are taken in (`Shown`), every module that links it having tracks and none showing it — named by
/// the campus of its course, or, of two towns' courses, by the town shown. An event in no course,
/// of mixed or unknown town, or one that also belongs to a module without tracks, stays.
fn event_hidden(event: &Event, selection: &Selection, shown: &BTreeMap<String, Shown>) -> Option<HiddenBy> {
    if event_number(&event.id).is_some_and(|id| selection.hidden_events.contains(&id)) {
        return Some(HiddenBy::Event);
    }
    if event.kinds.hidden_by(selection.hidden_kinds) {
        return Some(HiddenBy::Kinds);
    }
    let first = shown.get(event.modules.first()?)?;
    let theirs = event.modules.iter().all(|module| shown.get(module).is_some_and(|shown| !shown.shows(event)));
    if !theirs {
        return None;
    }
    match event.course {
        Some(course) if !first.campuses.is_empty() => Some(HiddenBy::Course(course)),
        _ => first.towns.first().copied().map(HiddenBy::Town),
    }
}

/// The choices among the planned modules' events of the semester (`Input::choices`), each event
/// marked as the option it is (`Event::across`), in the order of the modules and their numbers.
/// An event another planned module links as well stays required (that module attends it), so it is
/// no option; an option left without an event of the plan is none, and a choice left with fewer
/// than two options is none.
fn choices_of(rows: &[ChoiceOption], events: &mut [Event], modules: &[String], key: SemesterKey) -> Vec<Across> {
    let semester = key.key();
    /// A choice found: its module, number, kind, pick, and per option number its events.
    type Found = (String, u32, ChoiceKind, usize, BTreeMap<u32, Vec<usize>>);
    let mut found: Vec<Found> = Vec::new();
    for row in rows.iter().filter(|row| row.semester_key == semester) {
        let Some(at) = events.iter().position(|event| event.id == row.event_id && event.modules == [row.module_id.clone()]) else { continue };
        let index = match found.iter().position(|(module, number, ..)| *module == row.module_id && *number == row.choice) {
            Some(index) => index,
            None => {
                let kind = row.kind.known().unwrap_or(ChoiceKind::Topic);
                found.push((row.module_id.clone(), row.choice, kind, usize::try_from(row.pick).unwrap_or(1).max(1), BTreeMap::new()));
                found.len() - 1
            }
        };
        if let Some((.., options)) = found.get_mut(index) {
            let option = options.entry(row.option).or_default();
            if !option.contains(&at) {
                option.push(at);
            }
        }
    }
    let position = |module: &str| modules.iter().position(|known| known == module).unwrap_or(usize::MAX);
    found.sort_by_key(|(module, number, ..)| (position(module), *number));
    let mut choices = Vec::new();
    for (module, number, kind, pick, options) in found {
        if options.len() < 2 || options.len() <= pick {
            continue;
        }
        let index = choices.len();
        for (option, (_, held)) in options.iter().enumerate() {
            for event in held {
                if let Some(event) = events.get_mut(*event) {
                    event.across = Some(Alt { choice: index, option, open: false, take: 0 });
                }
            }
        }
        choices.push(Across {
            module,
            number,
            kind,
            pick,
            numbers: options.keys().copied().collect(),
            options: options.into_values().collect(),
            taken: BTreeSet::new(),
            by: None,
            narrowed: None,
        });
    }
    choices
}

/// What is decided of each choice among a module's events, and the events of the options it
/// leaves out hidden: the options a Termin of which the visitor chose (`Selection::chosen_rows`);
/// else, of a choice whose options are for other programs, those QIS lists the plan's program at
/// (`Input::listed`) — as many as one takes, or more, among which it then chooses. An event hidden
/// for another reason stays so. Then each option's events say whether they are one to choose
/// among (`Alt::open`).
fn take(choices: &mut [Across], events: &mut [Event], listed: &[ChoiceFamily], selection: &Selection) {
    for across in choices.iter_mut() {
        let chosen: BTreeSet<usize> = across
            .options
            .iter()
            .enumerate()
            .filter(|(_, held)| {
                held.iter().filter_map(|event| events.get(*event)).flat_map(|event| &event.rows).any(|row| row.key.is_some_and(|key| selection.chosen_rows.contains(&key)))
            })
            .map(|(option, _)| option)
            .collect();
        if !chosen.is_empty() {
            across.taken = chosen;
            across.by = Some(TakenBy::Chosen);
        } else if across.kind == ChoiceKind::Program {
            let own: BTreeSet<usize> = across
                .numbers
                .iter()
                .enumerate()
                .filter(|(_, number)| listed.iter().any(|row| row.module_id == across.module && row.choice == across.number && row.option == **number))
                .map(|(option, _)| option)
                .collect();
            if !own.is_empty() && own.len() < across.options.len() {
                match own.len() <= across.pick {
                    true => {
                        across.taken = own;
                        across.by = Some(TakenBy::Program);
                    }
                    false => across.narrowed = Some(own),
                }
            }
        }
        // The options left out: all but those taken once enough are, else all but those the
        // program narrows the choice to.
        let (out, by): (Vec<usize>, HiddenBy) = match (&across.by, &across.narrowed) {
            (Some(by), _) if across.taken.len() >= across.pick => {
                let by = if *by == TakenBy::Chosen { HiddenBy::Option } else { HiddenBy::Program };
                ((0..across.options.len()).filter(|option| !across.taken.contains(option)).collect(), by)
            }
            (None, Some(narrowed)) => ((0..across.options.len()).filter(|option| !narrowed.contains(option)).collect(), HiddenBy::Program),
            _ => (Vec::new(), HiddenBy::Option),
        };
        for option in out {
            for event in across.options.get(option).into_iter().flatten() {
                if let Some(event) = events.get_mut(*event) {
                    if event.hidden.is_none() {
                        event.hidden = Some(by);
                        for row in &mut event.rows {
                            row.hidden = Some(by);
                        }
                    }
                }
            }
        }
    }
    for (index, across) in choices.iter().enumerate() {
        let (choosable, take) = (across.choosable(events), across.take(events));
        for (option, held) in across.options.iter().enumerate() {
            let open = choosable.contains(&option);
            for event in held {
                if let Some(event) = events.get_mut(*event) {
                    event.across = Some(Alt { choice: index, option, open, take: if open { take } else { 0 } });
                }
            }
        }
    }
}

/// A `veranstid` as the selection keeps it: digits without a leading zero that fit a `u32`.
fn event_number(id: &str) -> Option<u32> {
    let canonical = !id.is_empty() && !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| id.parse().ok()).flatten()
}

/// Applies the ticked options, the hidden Termine and the hidden dates to the rows. Every option
/// a row of which has its key ticked is shown, however many (owner, 2026-10-26); a ticked key that
/// names no option's row (a required row, or one QIS changed) changes nothing, and with none left
/// the choice is open again. A row is hidden, first rule that holds, with its event, when other
/// options are ticked and its own is not, or when its key is hidden. A hidden date of a row's key
/// leaves its held days (`Occurrences::hidden`): the Termin stays, that day is gone from the week,
/// the day, the clashes and the feed.
fn choose(event: &mut Event, selection: &Selection) {
    event.chosen = match event.attendance {
        Attendance::OneOf { .. } => event
            .rows
            .iter()
            .filter_map(|row| row.option.filter(|_| row.key.is_some_and(|key| selection.chosen_rows.contains(&key))))
            .collect(),
        Attendance::All => BTreeSet::new(),
    };
    let chosen = event.chosen.clone();
    for row in &mut event.rows {
        let by_choice = !chosen.is_empty() && row.option.is_some_and(|option| !chosen.contains(&option));
        let by_row = row.key.is_some_and(|key| selection.hidden_rows.contains(&key));
        row.hidden = event.hidden.or(by_choice.then_some(HiddenBy::Choice)).or(by_row.then_some(HiddenBy::Row));
        if let Some(key) = row.key {
            let hidden: Vec<Day> = selection.hidden_dates.range((key, Day(i32::MIN))..=(key, Day(i32::MAX))).map(|(_, day)| *day).collect();
            if !hidden.is_empty() {
                let (gone, kept): (Vec<Day>, Vec<Day>) = row.occ.days.iter().partition(|day| hidden.contains(day));
                row.occ.days = kept;
                row.occ.hidden = gone;
            }
        }
    }
}

/// The fixtures of the timetable's tests, for the tests of the crates above it too (feature
/// `fixtures`). Built for tests only, so they may panic as tests do.
#[cfg(any(test, feature = "fixtures"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
pub mod tests {
    use super::*;
    #[cfg(test)]
    use folia_model::db::Database;
    use folia_model::labels::Code;
    #[cfg(test)]
    use folia_calendar::select::MAX_MODULES;
    #[cfg(test)]
    use folia_query as queries;

    pub fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    pub fn ids(modules: &[&str]) -> Vec<String> {
        modules.iter().map(|id| id.to_string()).collect()
    }

    /// 2026W as the data says it: lectures 05.10.2026–31.01.2027, the break 21.12.–03.01., A weeks
    /// from the first week.
    pub fn winter() -> SemesterFacts {
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
    pub struct Fixture(pub DateRow);

    pub fn teaching(
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
                room_short: None,
                series: None,
                track: None,
                choice: None,
                option: None,
                extra: false,
            },
        })
    }

    impl Fixture {
        pub fn group(mut self, name: &str) -> Self {
            self.0.date.group_name = Some(name.into());
            self
        }

        pub fn campus(mut self, campus: Option<&str>) -> Self {
            self.0.date.campus = campus.map(Code::parse);
            self
        }

        pub fn rhythm(mut self, rhythm: &str) -> Self {
            self.0.date.rhythm = Some(Code::parse(rhythm));
            self
        }

        pub fn range(mut self, first: &str, last: &str) -> Self {
            self.0.date.first_date = Some(first.into());
            self.0.date.last_date = Some(last.into());
            self
        }

        pub fn undated(mut self) -> Self {
            (self.0.date.first_date, self.0.date.last_date) = (None, None);
            self
        }

        pub fn title(mut self, title: &str) -> Self {
            self.0.date.event_title = title.into();
            self
        }

        /// The same row linked to another module.
        pub fn of(mut self, module: &str) -> Self {
            self.0.module_id = module.into();
            self
        }

        /// A row of an event that is a course of its module at `campus` (`EventDate::track`).
        pub fn track(mut self, campus: &str) -> Self {
            self.0.date.track = Some(Code::parse(campus));
            self
        }

        /// A row of an event that is option `option` of its module's choice 1 (`EventDate::choice`).
        pub fn option(mut self, option: u32) -> Self {
            (self.0.date.choice, self.0.date.option) = (Some(1), Some(option));
            self
        }

        /// A row of an extra beside the module's compulsory part (`EventDate::extra`).
        pub fn extra(mut self) -> Self {
            self.0.date.extra = true;
            self
        }

        /// A row of a series of single dates (`EventDate::series`) named by the row of `first`.
        pub fn series(mut self, first: i64, weeks: u8) -> Self {
            self.0.date.series = Some(folia_model::rows_detail::Series { first, weeks });
            self
        }

        pub fn key(&self) -> RowKey {
            RowKey::of(&self.0.date).unwrap()
        }
    }

    pub fn sws(module: &str, form: &str, sws: f64) -> ModuleSws {
        ModuleSws { module_id: module.into(), form: Code::parse(form), sws }
    }

    /// The timetable of synthetic rows of 2026W.
    pub fn table(rows: &[Fixture], modules: &[&str], sws: &[ModuleSws], selection: &Selection) -> Timetable {
        built(rows, modules, sws, &[], &[], &[], selection)
    }

    /// The timetable of synthetic rows of 2026W with the choices among their events `choices`
    /// (their rows' `choice` and `option` are the fixtures' own, `Fixture::option`), of whose
    /// options the plan's program is listed at `listed`.
    pub fn choosing(rows: &[Fixture], modules: &[&str], choices: &[ChoiceOption], listed: &[ChoiceFamily], selection: &Selection) -> Timetable {
        let t = built(rows, modules, &[], &[], choices, listed, selection);
        // The week of the choices holds too (in this crate's own tests).
        #[cfg(test)]
        crate::views::tests::invariants(&t);
        t
    }

    /// A row of `v_module_choice`: of `module`'s choice 1 of `kind` in 2026W, of which one takes
    /// `pick`, option `option` holds `event`.
    pub fn choice_row(module: &str, kind: &str, pick: u32, option: u32, event: &str) -> ChoiceOption {
        ChoiceOption {
            module_id: module.into(),
            semester_key: "2026W".into(),
            choice: 1,
            course: None,
            kind: Code::parse(kind),
            basis: Code::parse("form"),
            sws: Some(2.0),
            pick,
            option,
            event_id: event.into(),
            label: None,
            event_sws: Some(2.0),
        }
    }

    /// The plan's program listed at option `option` of `module`'s choice 1 (`Input::listed`).
    pub fn listed_at(module: &str, option: u32) -> ChoiceFamily {
        ChoiceFamily {
            module_id: module.into(),
            semester_key: "2026W".into(),
            choice: 1,
            option,
            family_key: "017-82".into(),
            program_name: "Bauingenieurwesen".into(),
            is_latest_po: true,
        }
    }

    /// A course the plan's program takes (`Input::taken`): of `module` the one at `campus`.
    pub fn takes(module: &str, campus: &str) -> TrackProgram {
        TrackProgram {
            module_id: module.into(),
            semester_key: "2026W".into(),
            campus: Code::parse(campus),
            program_id: "247-82-2016".into(),
            basis: Code::parse("qis"),
            family_key: "247-82".into(),
            program_name: "Medizininformatik".into(),
            degree_display: Some("B.Sc.".into()),
            is_latest_po: true,
        }
    }

    /// The timetable of synthetic rows of 2026W whose program takes the courses `taken`.
    pub fn taking(rows: &[Fixture], modules: &[&str], taken: &[TrackProgram], selection: &Selection) -> Timetable {
        built(rows, modules, &[], taken, &[], &[], selection)
    }

    #[allow(clippy::too_many_arguments)]
    fn built(rows: &[Fixture], modules: &[&str], sws: &[ModuleSws], taken: &[TrackProgram], choices: &[ChoiceOption], listed: &[ChoiceFamily], selection: &Selection) -> Timetable {
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
            taken,
            choices,
            listed,
        };
        let built = Timetable::build(&input, selection);
        invariants(&built);
        built
    }

    pub fn event<'t>(table: &'t Timetable, id: &str) -> &'t Event {
        table.events.iter().find(|event| event.id == id).unwrap_or_else(|| panic!("no event {id}"))
    }

    /// How many options an event's rows form, `None` when all are required.
    pub fn options(event: &Event) -> Option<usize> {
        match &event.attendance {
            Attendance::OneOf { options, .. } => Some(options.len()),
            Attendance::All => None,
        }
    }

    /// The timetable of `modules` in `semester` of a snapshot, loaded as the Studienplan loads it.
    #[cfg(test)]
    pub fn planned(db: &dyn Database, semester: &str, modules: &[String], selection: &Selection) -> Timetable {
        let semesters = queries::semesters(db).unwrap();
        let row = semesters.iter().find(|s| s.key == semester);
        let key = SemesterKey::parse(semester).unwrap();
        let facts = SemesterFacts::derive(key, row, &queries::semester_date_counts(db, semester).unwrap());
        let schedule = queries::modules_schedule(db, modules, semester).unwrap();
        let exams = queries::modules_exams(db, modules, semester).unwrap();
        let sws = queries::modules_teaching_sws(db, modules).unwrap();
        let input = Input { key, semester: row, facts: &facts, modules, schedule: &schedule, exams: &exams, sws: &sws, taken: &[], choices: &[], listed: &[] };
        let built = Timetable::build(&input, selection);
        invariants(&built);
        built
    }

    /// What holds for every timetable: events once and in plan order, rows once by `ord`, options
    /// that partition the rows they hold, a choice only of an option, the rows of a hidden event
    /// hidden with it, tones in range, chips that count, and clashes between visible, timed,
    /// overlapping rows of events without a common module.
    pub fn invariants(t: &Timetable) {
        let position =
            |id: &str| t.modules.iter().position(|m| m == id).unwrap_or_else(|| panic!("{id} is not planned"));
        let distinct: BTreeSet<&str> = t.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(distinct.len(), t.events.len(), "an event twice");
        // QIS's first, in plan order; then the Termine of one's own, which no module links.
        let qis = t.events.iter().take_while(|e| e.own.is_none()).count();
        assert!(t.events.iter().skip(qis).all(|e| e.own.is_some()), "the Termine of one's own last");
        let firsts: Vec<usize> = t.events.iter().take(qis).map(|e| position(&e.modules[0])).collect();
        assert!(firsts.windows(2).all(|w| w[0] <= w[1]), "events in plan order");
        for (i, e) in t.events.iter().enumerate() {
            if let Some(own) = &e.own {
                assert!(e.modules.is_empty() && e.across.is_none() && !e.visit && !e.extra && e.hidden.is_none(), "{}", e.id);
                assert!(e.rows.iter().all(|r| r.key == Some(own.key()) && r.hidden.is_none()), "{}: its rows keyed by its own key", e.id);
                let tone = own.module.as_ref().and_then(|m| t.modules.iter().position(|planned| planned == m)).map_or(0, |at| at % TONES + 1);
                assert_eq!(usize::from(e.tone), tone, "{}", e.id);
                assert!(own.skip.iter().all(|day| e.rows.iter().all(|r| !r.occ.days.contains(day))), "{}: a day it skips is no day of it", e.id);
            }
            let positions: Vec<usize> = e.modules.iter().map(|m| position(m)).collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{}: modules in plan order, each once", e.id);
            if let Some(first) = positions.first() {
                assert_eq!(usize::from(e.tone), first % TONES + 1, "{}", e.id);
            }
            assert_eq!(e.class, class_of(e.kinds));
            assert!(e.rows.windows(2).all(|w| w[0].ord < w[1].ord), "{}: rows once, by ord", e.id);
            assert!(e.rows.iter().all(|r| r.ord.is_some() && r.from.is_some() == r.to.is_some()));
            match &e.attendance {
                Attendance::All => {
                    assert!(e.rows.iter().all(|r| r.option.is_none()) && e.chosen.is_empty(), "{}", e.id);
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
                    assert!(e.chosen.iter().all(|chosen| *chosen < options.len()), "{}", e.id);
                    if !e.chosen.is_empty() {
                        assert!(e.visible_options().iter().all(|o| e.chosen.contains(o)), "{}", e.id);
                        assert!(!e.unresolved(), "{}: ticked options are the plan's", e.id);
                    }
                }
            }
            for r in &e.rows {
                if e.hidden.is_some() {
                    assert_eq!(r.hidden, e.hidden, "{}: a row of a hidden event", e.id);
                }
                if r.hidden == Some(HiddenBy::Choice) {
                    assert!(!e.chosen.is_empty() && r.option.is_some_and(|option| !e.chosen.contains(&option)));
                }
            }
            if e.hidden.is_some() {
                assert!(e.visible_options().is_empty() && !e.unresolved());
            }
            if let Some(HiddenBy::Town(shown)) = e.hidden {
                assert_eq!(t.town, Some(shown));
                assert!(e.modules.iter().all(|m| t.tracks.contains(m) && !t.courses.contains_key(m)) && e.town() != Some(shown), "{}", e.id);
            }
            if let Some(HiddenBy::Course(course)) = e.hidden {
                assert_eq!(e.course, Some(course));
                assert!(e.modules.iter().all(|m| t.courses.get(m).is_some_and(|held| held.contains(&course))), "{}", e.id);
                assert!(e.modules.iter().all(|m| t.shown.get(m).is_some_and(|shown| !shown.campuses.contains(&course))), "{}", e.id);
            }
            assert!(!t.blocked.contains(&i) || e.unresolved(), "{}", e.id);
        }
        assert!(t.blocked.windows(2).all(|w| w[0] < w[1]));
        // The choices among a module's events: each event of an option says so, an option taken
        // or narrowed to is one of them, and what they hide is an option left out.
        for (c, across) in t.choices.iter().enumerate() {
            assert!(across.options.len() >= 2 && across.options.len() > across.pick && across.numbers.len() == across.options.len(), "{}", across.module);
            for (o, held) in across.options.iter().enumerate() {
                assert!(!held.is_empty());
                for e in held {
                    let alt = t.events[*e].across.unwrap_or_else(|| panic!("{} is no option", t.events[*e].id));
                    assert_eq!((alt.choice, alt.option), (c, o));
                    assert!(t.events[*e].modules == [across.module.clone()], "{}", t.events[*e].id);
                    assert_eq!(alt.open, across.choosable(&t.events).contains(&o));
                }
            }
            assert!(across.taken.iter().all(|o| *o < across.options.len()) && across.taken.is_empty() == across.by.is_none());
            assert!(across.narrowed.as_ref().is_none_or(|narrowed| narrowed.len() > across.pick && across.by.is_none()));
        }
        for e in &t.events {
            if matches!(e.hidden, Some(HiddenBy::Option | HiddenBy::Program)) {
                let alt = e.across.unwrap_or_else(|| panic!("{} hidden as an option", e.id));
                let across = &t.choices[alt.choice];
                assert!(!across.taken.contains(&alt.option) && !across.narrowed.as_ref().is_some_and(|narrowed| narrowed.contains(&alt.option)), "{}", e.id);
                assert!(!alt.open, "{}", e.id);
            }
        }
        assert!(t.blocked_choices.windows(2).all(|w| w[0] < w[1]));
        for b in &t.blocked_choices {
            assert!(t.choices[*b].open(&t.events), "{} blocked, not open", t.choices[*b].module);
            assert!(
                t.clashes.iter().any(|c| [c.a.0, c.b.0].iter().any(|e| t.events[*e].across.is_some_and(|alt| alt.choice == *b))),
                "{} blocked, named by no clash",
                t.choices[*b].module
            );
        }
        for clash in &t.clashes {
            let (ea, eb) = (&t.events[clash.a.0], &t.events[clash.b.0]);
            let (ra, rb) = (&ea.rows[clash.a.1], &eb.rows[clash.b.1]);
            assert!(!ea.extra && !eb.extra, "{} × {}: an extra meets nothing", ea.id, eb.id);
            assert!(!ea.visit && !eb.visit, "{} × {}: a visit meets nothing", ea.id, eb.id);
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
            assert!(r.option.is_none() || !e.unresolved() || t.blocked.contains(event) || e.across.is_some_and(|alt| alt.open), "{}/{:?}", e.id, r.ord);
            assert!(e.across.is_none_or(|alt| !alt.open || t.blocked_choices.contains(&alt.choice)), "{}/{:?}", e.id, r.ord);
            assert!(t.clashes.iter().any(|c| c.a.0 == *event || c.b.0 == *event), "{}", e.id);
        }
        let chips = t.kinds_present();
        assert!(chips.iter().all(|(_, n)| *n > 0) && chips.windows(2).all(|w| w[0].0 < w[1].0));
        let exams_of_mine = t.exams.iter().any(|exam| !t.theirs_exam(exam));
        assert_eq!(chips.iter().any(|(k, _)| *k == EventKind::Exam), exams_of_mine);
        // What another course alone holds is no chip; everything shown has one.
        for event in t.events.iter().filter(|event| event.hidden.is_none()) {
            assert!(event.kinds.iter().all(|kind| chips.iter().any(|(k, _)| *k == kind)), "{}", event.id);
            assert!(!t.theirs(event), "{}", event.id);
        }
        for event in t.events.iter().filter(|event| matches!(event.hidden, Some(HiddenBy::Town(_) | HiddenBy::Course(_)))) {
            assert!(t.theirs(event), "{}", event.id);
        }
        assert!(!t.town_derived || t.town.is_some());
        assert!(t.courses.keys().all(|m| t.tracks.contains(m)) && t.shown.keys().all(|m| t.tracks.contains(m)));
        for (module, shown) in &t.shown {
            match t.courses.get(module) {
                Some(held) => assert!(!shown.campuses.is_empty() && shown.campuses.is_subset(held) && shown.campuses != *held, "{module}"),
                None => assert!(shown.campuses.is_empty() && shown.towns.len() == 1 && t.town == shown.towns.first().copied(), "{module}"),
            }
        }
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
        assert_eq!(e.chosen, BTreeSet::from([1]));
        let hidden: Vec<Option<HiddenBy>> = e.rows.iter().map(|r| r.hidden).collect();
        assert_eq!(hidden, [Some(HiddenBy::Choice), None, Some(HiddenBy::Choice), None], "the single stays");
        assert_eq!(e.visible_options(), [1]);
        assert!(!e.unresolved());
        // A key of no row (QIS changed the row), or of a required row: the choice is open again.
        for stale in [RowKey { event: 70, fp: 0x12345 }, rows[3].key(), RowKey { event: 71, fp: rows[0].key().fp }] {
            assert_eq!(pick(&[stale]), open, "{}", stale.text());
        }
        // Two ticked rows of one event: both options shown, alike, and the choice made (owner,
        // 2026-10-26: several groups); the third option stays out.
        let two = pick(&[rows[2].key(), rows[0].key()]);
        let e = &two.events[0];
        assert_eq!(e.chosen, BTreeSet::from([0, 2]));
        assert_eq!(e.visible_options(), [0, 2]);
        assert!(!e.unresolved());
        assert_eq!(e.rows[1].hidden, Some(HiddenBy::Choice));
        // An event whose rows are all required has no choice.
        let t = table(&rows, &["A"], &[], &Selection { chosen_rows: [rows[0].key()].into(), ..Selection::default() });
        assert!(t.events[0].chosen.is_empty() && t.events[0].rows.iter().all(|r| r.hidden.is_none()));
    }

    /// A Termin of one's own of 2026W: Friday 13:00–17:00 every week of the lecture period.
    fn job(number: u32) -> OwnTermin {
        OwnTermin {
            number,
            rhythm: OwnRhythm::Week,
            weekday: 5,
            dates: Vec::new(),
            from: 13 * 60,
            to: 17 * 60,
            period: None,
            module: None,
            name: "Arbeit".into(),
            place: "Café Mosaik".into(),
            skip: Vec::new(),
        }
    }

    #[test]
    fn a_termin_of_ones_own_stands_in_the_week_firm_as_a_modules() {
        let rows = [teaching("A", "80", 1, "Vorlesung", 5, "09:15", "10:45"), teaching("B", "81", 1, "Übung", 5, "14:00", "15:30")];
        let selection = Selection { own: vec![job(1), OwnTermin { weekday: 1, from: 8 * 60, to: 9 * 60, module: Some("B".into()), name: String::new(), ..job(2) }], ..Selection::default() };
        let t = table(&rows, &["A", "B"], &[], &selection);
        invariants(&t);
        let own: Vec<&Event> = t.events.iter().filter(|e| e.own.is_some()).collect();
        assert_eq!(own.iter().map(|e| (e.id.as_str(), e.title.as_str(), e.tone)).collect::<Vec<_>>(), [("4000000001", "Arbeit", 0), ("4000000002", "", 2)]);
        let row = &own[0].rows[0];
        assert_eq!((row.from, row.to, row.date.room.as_deref(), row.key.map(RowKey::text)), (Some(780), Some(1020), Some("Café Mosaik"), Some("4000000001-00000".into())));
        // Every Friday of the lecture period but the break; 30.10. is no holiday in Brandenburg's law,
        // so the job is held then, as a weekly row of QIS would be.
        assert_eq!((row.occ.days.first().copied(), row.occ.days.last().copied()), (Some(d("2026-10-09")), Some(d("2027-01-29"))));
        assert!(row.occ.assumed && !row.occ.days.contains(&d("2026-12-25")));
        // It meets B's Übung, which no module of it shares: a clash, as two modules' Termine are.
        assert_eq!(t.clashes.len(), 1);
        let clash = &t.clashes[0];
        assert_eq!((t.events[clash.a.0].id.as_str(), t.events[clash.b.0].id.as_str()), ("81", "4000000001"));
        // Its slot in the week, as any other.
        assert!(t.regular_week().iter().any(|item| t.events[item.event].own.is_some() && item.day == 5 && item.from == 780 && item.clash));
    }

    #[test]
    fn a_choice_of_groups_takes_a_termin_of_ones_own_as_the_plans() {
        // Two groups of an Übung, one at the job's time: the choice stays open, its group at the
        // job meets it, the other is free (`clash::option_meetings`).
        let rows = [teaching("A", "80", 1, "Übung", 5, "13:45", "15:15").group("1"), teaching("A", "80", 2, "Übung", 4, "09:15", "10:45").group("2")];
        let selection = Selection { own: vec![job(1)], ..Selection::default() };
        let t = table(&rows, &["A"], &[], &selection);
        invariants(&t);
        assert!(t.events[0].unresolved() && t.clashes.is_empty() && t.blocked.is_empty(), "the other group is free");
        let meetings = clash::option_meetings(&t.events);
        assert!(meetings.contains(&(0, 0)) && !meetings.contains(&(0, 1)), "{meetings:?}");
        // With the job on Thursday morning too, no group is free: the choice is blocked.
        let both = Selection { own: vec![job(1), OwnTermin { weekday: 4, from: 9 * 60, to: 11 * 60, ..job(2) }], ..Selection::default() };
        let blocked = table(&rows, &["A"], &[], &both);
        assert_eq!(blocked.blocked, [0]);
    }

    #[test]
    fn a_termin_of_ones_own_keeps_to_its_weeks_its_period_and_the_days_it_skips() {
        let a = OwnTermin { rhythm: OwnRhythm::AWeek, weekday: 2, period: Some((d("2026-10-14"), d("2026-12-18"))), skip: vec![d("2026-11-03")], ..job(1) };
        let b = OwnTermin { rhythm: OwnRhythm::BWeek, weekday: 2, ..job(2) };
        let block = OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![d("2026-12-23"), d("2027-02-12")], ..job(3) };
        let t = table(&[], &[], &[], &Selection { own: vec![a, b, block], ..Selection::default() });
        invariants(&t);
        let days = |e: usize| t.events[e].rows.iter().flat_map(|row| row.occ.days.iter().map(|day| day.iso())).collect::<Vec<_>>();
        // A weeks from 05.10. on, every other week: the first Tuesday of one from 14.10. on is 20.10.;
        // 03.11. skipped, held up to the period's end.
        assert_eq!(days(0), ["2026-10-20", "2026-11-17", "2026-12-01", "2026-12-15"]);
        assert_eq!(t.events[0].rows[0].occ.hidden, [d("2026-11-03")]);
        // B weeks through the lecture period, without the break.
        assert_eq!(days(1).first().map(String::as_str), Some("2026-10-13"));
        assert!(days(1).iter().all(|day| !day.starts_with("2026-12-2")));
        // Dates of its own, in the break and after the lectures too: a single date is never skipped.
        assert_eq!(days(2), ["2026-12-23", "2027-02-12"]);
        assert_eq!(t.events[2].rows.len(), 2);
    }

    #[test]
    fn a_termin_of_ones_own_without_the_weeks_known_is_a_pattern() {
        let facts = SemesterFacts::derive(SemesterKey::parse("2026W").unwrap(), None, &[]);
        let own = OwnTermin { rhythm: OwnRhythm::AWeek, period: Some((d("2026-10-14"), d("2026-12-18"))), ..job(1) };
        let event = own_event(&own, &[], &facts);
        assert_eq!(event.rows[0].occ.template.map(|t| (t.weekday, t.every)), Some((5, Every::AWeek)));
        assert!(event.rows[0].occ.days.is_empty());
    }

    #[test]
    fn a_hidden_date_leaves_its_day_and_keeps_its_termin() {
        let rows = [teaching("A", "80", 1, "Übung", 3, "09:15", "10:45"), teaching("B", "81", 1, "Vorlesung", 3, "10:00", "11:30")];
        let open = table(&rows, &["A", "B"], &[], &Selection::default());
        let days = open.events[0].rows[0].occ.days.clone();
        let day = d("2026-11-04");
        assert!(days.contains(&day));
        // 23.12. lies in the break: no held day of it, so nothing to hide.
        let selection = Selection { hidden_dates: [(rows[0].key(), day), (rows[0].key(), d("2026-12-23"))].into(), ..Selection::default() };
        let t = table(&rows, &["A", "B"], &[], &selection);
        let row = &t.events[0].rows[0];
        assert_eq!(row.occ.days.len(), days.len() - 1);
        assert!(!row.occ.days.contains(&day));
        assert_eq!(row.occ.hidden, [day]);
        assert!(row.hidden.is_none() && t.events[0].hidden.is_none(), "the Termin stays");
        // The week reads as before: the two still meet on the other Wednesdays.
        let slots = |t: &Timetable| t.regular_week().iter().map(|item| (item.event, item.day, item.from, item.label.clone())).collect::<Vec<_>>();
        assert_eq!(slots(&t), slots(&open));
        assert_eq!(t.clashes.len(), open.clashes.len());
        // The day has no item of it any more; the other module's Termin stays.
        let items = |t: &Timetable| -> Vec<usize> {
            t.agenda(folia_locale::Locale::De).iter().flat_map(|week| &week.days).filter(|d| d.day == day).flat_map(|d| d.items.iter().filter_map(|item| item.event)).collect()
        };
        assert_eq!(items(&open), [0, 1]);
        assert_eq!(items(&t), [1]);
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

        // Derived from the modules taken over from the Regelstudienplan alone, it holds when an
        // elective of the other town is planned beside them: S's Senftenberg Übung and seminar
        // outweigh C's one Cottbus event, but S was added later.
        let from = |ids: &[&str]| Selection { town_from: Some(ids.iter().map(|id| id.to_string()).collect()), ..Selection::default() };
        let mut elective = more.clone();
        elective.push(teaching("S", "140", 1, "Übung", 1, "13:45", "15:15").campus(senftenberg));
        elective.push(teaching("S", "141", 1, "Seminar", 2, "13:45", "15:15").campus(senftenberg));
        let all = table(&elective, &["T", "L", "M", "C", "S"], &[], &Selection::default());
        assert_eq!((all.town, all.town_derived), (Some(Town::Senftenberg), true), "every module decides");
        // A visit decides nothing: S only visited leaves it to the others.
        let visited = Selection { visits: BTreeSet::from(["S".to_string()]), ..Selection::default() };
        let t = table(&elective, &["T", "L", "M", "C", "S"], &[], &visited);
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true), "a visit of the other town");
        let t = table(&elective, &["T", "L", "M", "C", "S"], &[], &from(&["T", "L", "M", "C"]));
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true), "the imported ones decide");
        assert_eq!(event(&t, "102").hidden, Some(HiddenBy::Town(Town::Cottbus)));
        // What the imported modules leave open stays open (both courses shown), whatever else is
        // planned; a chosen town still wins.
        let t = table(&elective, &["T", "L", "M", "C", "S"], &[], &from(&["T", "L", "M"]));
        assert_eq!((t.town, t.town_derived), (None, false));
        let chosen = Selection { town: TownChoice::Only(Town::Senftenberg), ..from(&["T", "L", "M", "C"]) };
        assert_eq!(table(&elective, &["T", "L", "M", "C", "S"], &[], &chosen).town, Some(Town::Senftenberg));
    }

    /// The events the design quotes, on the pinned snapshot; on any snapshot, the invariants of the
    /// first sixty modules with a dated row in the current semester.
    #[test]
    fn alternatives_are_decided_by_groups_and_sws() {
        let pinned = folia_test_support::studyplan_db("alternatives_are_decided_by_groups_and_sws");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
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
        let linking = folia_test_support::column(
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
        assert_eq!((e.chosen.clone(), e.unresolved()), (BTreeSet::from([1]), false));
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
        let Some(db) = folia_test_support::studyplan_db("city_tracks_are_whole_courses") else {
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

    /// The chips count the visitor's course of a module with city tracks, not the other town's:
    /// with both courses counted, Elektrotechnik's first semester in Senftenberg read „Übung 7"
    /// and „Tutorium 2" for three Übungen and no Tutorium (the audit of 2026-09-24).
    #[test]
    fn the_chips_count_the_course_of_the_town_taken() {
        use EventKind::{Exam, Exercise, Lecture, Practical, Seminar, Tutorial};
        let senftenberg = Some("senftenberg");
        let rows = [
            teaching("T", "100", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("T", "101", 1, "Übung", 2, "09:15", "10:45"),
            teaching("T", "102", 1, "Vorlesung", 1, "09:15", "10:45").campus(senftenberg),
            teaching("T", "103", 1, "Übung", 2, "09:15", "10:45").campus(senftenberg),
            // Only the Senftenberg course has a Tutorium.
            teaching("T", "104", 1, "Tutorium", 3, "09:15", "10:45").campus(senftenberg),
            teaching("C", "130", 1, "Seminar", 5, "13:45", "15:15"),
        ];
        let chips = |selection: &Selection| table(&rows, &["T", "C"], &[], selection).kinds_present();
        // Cottbus, derived from C: the Senftenberg course counts for nothing, also while its kinds
        // are hidden as well (then `HiddenBy::Kinds` is the first reason, not the town).
        let cottbus = [(Lecture, 1), (Exercise, 1), (Seminar, 1)];
        assert_eq!(chips(&Selection::default()), cottbus);
        let hidden = KindSet::default().with(Exercise).with(Tutorial);
        assert_eq!(chips(&Selection { hidden_kinds: hidden, ..Selection::default() }), cottbus);
        let chosen = Selection { town: TownChoice::Only(Town::Senftenberg), ..Selection::default() };
        assert_eq!(chips(&chosen), [(Lecture, 1), (Exercise, 1), (Seminar, 1), (Tutorial, 1)]);
        let both = Selection { town: TownChoice::Both, ..Selection::default() };
        assert_eq!(chips(&both), [(Lecture, 2), (Exercise, 2), (Seminar, 1), (Tutorial, 1)]);

        let Some(db) = folia_test_support::studyplan_db("the_chips_count_the_course_of_the_town_taken") else {
            return;
        };
        // Informatik's first semester in Cottbus: 12104's Senftenberg lecture, Übung and exam
        // (149406, 149407, 150664) are not counted; with both towns they are.
        let fs1 = ids(&["12104", "12107", "12102", "11112"]);
        let t = planned(&db, "2026W", &fs1, &Selection::default());
        let mine = [(Lecture, 5), (Exercise, 3), (Practical, 2), (Tutorial, 1), (Exam, 3)];
        assert_eq!(t.kinds_present(), mine);
        let t = planned(&db, "2026W", &fs1, &both);
        assert_eq!(t.kinds_present(), [(Lecture, 6), (Exercise, 4), (Practical, 2), (Tutorial, 1), (Exam, 4)]);
        // Elektrotechnik 2022, PA und IoT, first semester: Senftenberg is derived, so the Cottbus
        // courses of 11107 and 12105 (with the only two Tutorien, 150018 and 152343) are not
        // counted.
        let et = ids(&["13694", "13693", "11107", "12761", "12105"]);
        let t = planned(&db, "2026W", &et, &Selection::default());
        assert_eq!((t.town, t.town_derived), (Some(Town::Senftenberg), true));
        let mine = [(Lecture, 5), (Exercise, 3), (Seminar, 2), (Practical, 2), (Exam, 7)];
        assert_eq!(t.kinds_present(), mine);
    }

    /// Courses Radix reads (schema 15, docs/radix/schema-v2.md „Tracks"): the Programmierpraktikum
    /// at the Zentralcampus and in Sachsendorf, Entwicklung von Softwaresystemen at the
    /// Zentralcampus and in Senftenberg. The program decides (owner, 2026-10-19: „nur die termine
    /// anzeigen, die den eingerichteten Studiengang entsprechend"), a town chosen first, „Beide"
    /// shows all.
    #[test]
    fn the_program_takes_its_courses() {
        let (sachsendorf, senftenberg) = (Some("sachsendorf"), Some("senftenberg"));
        let rows = [
            // P: a course at the Zentralcampus and one in Sachsendorf, both in Cottbus.
            teaching("P", "200", 1, "Vorlesung", 5, "11:30", "13:00").track("zentralcampus"),
            teaching("P", "201", 1, "Praktikum", 1, "11:30", "13:00").track("zentralcampus"),
            teaching("P", "202", 1, "Vorlesung", 2, "07:30", "09:00").campus(sachsendorf).track("sachsendorf"),
            teaching("P", "203", 1, "Laborausbildung", 2, "09:15", "10:45").campus(sachsendorf).track("sachsendorf"),
            // E: a course at the Zentralcampus and one in Senftenberg; a tutorial in no course.
            teaching("E", "210", 1, "Vorlesung", 3, "09:15", "10:45").track("zentralcampus"),
            teaching("E", "211", 1, "Übung", 3, "11:30", "13:00").track("zentralcampus"),
            teaching("E", "212", 1, "Vorlesung", 5, "10:00", "11:30").campus(senftenberg).track("senftenberg"),
            teaching("E", "213", 1, "Übung", 5, "14:30", "16:00").campus(senftenberg).track("senftenberg"),
            teaching("E", "214", 1, "Tutorium", 4, "13:45", "15:15").campus(None),
            // C: a module of the Zentralcampus without courses.
            teaching("C", "220", 1, "Seminar", 4, "09:15", "10:45"),
        ];
        let modules = ["P", "E", "C"];
        let hidden = |t: &Timetable| -> Vec<(String, HiddenBy)> {
            let mut hidden: Vec<(String, HiddenBy)> = t.events.iter().filter_map(|e| Some((e.id.clone(), e.hidden?))).collect();
            hidden.sort_by(|a, b| a.0.cmp(&b.0));
            hidden
        };
        let pair = |id: &str, by: HiddenBy| (id.to_string(), by);

        // Without a program: Cottbus derived from C, so E's Senftenberg course goes; P's two
        // courses are both in Cottbus and both shown.
        let t = table(&rows, &modules, &[], &Selection::default());
        assert_eq!(t.tracks, BTreeSet::from(["E".to_string(), "P".to_string()]));
        assert_eq!(t.courses["P"], BTreeSet::from([Campus::Zentralcampus, Campus::Sachsendorf]));
        assert_eq!((t.town, t.town_derived), (Some(Town::Cottbus), true));
        assert_eq!(hidden(&t), [pair("212", HiddenBy::Course(Campus::Senftenberg)), pair("213", HiddenBy::Course(Campus::Senftenberg))]);
        assert_eq!(t.shown["E"].by, ShownBy::Derived);
        assert!(!t.shown.contains_key("P"));
        assert_eq!(event(&t, "214").course, None);

        // Medizininformatik takes P in Sachsendorf and E in Senftenberg, whatever the plan's town.
        let taken = [takes("P", "sachsendorf"), takes("E", "senftenberg")];
        let t = taking(&rows, &modules, &taken, &Selection::default());
        let zc = HiddenBy::Course(Campus::Zentralcampus);
        assert_eq!(hidden(&t), [pair("200", zc), pair("201", zc), pair("210", zc), pair("211", zc)]);
        assert_eq!((t.shown["P"].by, t.shown["E"].by), (ShownBy::Program, ShownBy::Program));
        assert_eq!(t.shown["E"].towns, BTreeSet::from([Town::Senftenberg]));
        // The other courses count for no chip; the tutorial in no course stays everybody's.
        assert_eq!(t.kinds_present(), [(EventKind::Lecture, 2), (EventKind::Exercise, 1), (EventKind::Seminar, 1), (EventKind::Practical, 1), (EventKind::Tutorial, 1)]);

        // A town chosen wins over the program's course in the other town, and within the town the
        // program still decides: Cottbus shows P in Sachsendorf and E at the Zentralcampus.
        let cottbus = Selection { town: TownChoice::Only(Town::Cottbus), ..Selection::default() };
        let t = taking(&rows, &modules, &taken, &cottbus);
        let sfb = HiddenBy::Course(Campus::Senftenberg);
        assert_eq!(hidden(&t), [pair("200", zc), pair("201", zc), pair("212", sfb), pair("213", sfb)]);
        assert_eq!((t.shown["P"].by, t.shown["E"].by), (ShownBy::Program, ShownBy::Chosen));
        // Senftenberg chosen: P is held in no course there, so both of its courses are shown.
        let t = taking(&rows, &modules, &taken, &Selection { town: TownChoice::Only(Town::Senftenberg), ..Selection::default() });
        assert_eq!(hidden(&t), [pair("210", zc), pair("211", zc)]);
        // „Beide" shows every course.
        let both = Selection { town: TownChoice::Both, ..Selection::default() };
        assert!(hidden(&taking(&rows, &modules, &taken, &both)).is_empty());

        // A program that takes both courses of E (a family QIS lists on both) leaves E open; one
        // whose course is no course of the module decides nothing.
        let t = taking(&rows, &modules, &[takes("E", "zentralcampus"), takes("E", "senftenberg"), takes("P", "nord")], &Selection { town: TownChoice::Both, ..Selection::default() });
        assert!(t.shown.is_empty());
        // An event two planned modules link, one of them without courses, stays.
        let mut shared = rows.to_vec();
        shared.push(teaching("C", "210", 1, "Vorlesung", 3, "09:15", "10:45"));
        let t = taking(&shared, &modules, &taken, &Selection::default());
        assert_eq!(event(&t, "210").course, None);
        assert_eq!(event(&t, "210").hidden, None);
    }

    /// Höhere Mathematik T3 at the Zentralcampus (docs/radix/schema-v2.md, „Choices"): three
    /// exercises, each for other programs, and an Aufbaukurs beside them; a module M of the plan
    /// whose lecture meets the Tuesday exercise and the Aufbaukurs. Nothing chosen, every exercise
    /// is an option and the clash counts for nothing; Bauingenieurwesen takes its exercise, and the
    /// clash counts; the visitor's choice takes another; a program QIS lists at two chooses
    /// between them; options that each meet the plan block the choice.
    #[test]
    fn a_choice_among_a_modules_events() {
        let rows = [
            teaching("H", "148060", 1, "Vorlesung", 1, "15:30", "17:00").title("Höhere Mathematik - T3"),
            teaching("H", "150190", 1, "Übung", 1, "09:15", "10:45").title("Aufbaukurs Höhere Mathematik - T3").extra(),
            teaching("H", "148245", 1, "Übung", 4, "11:30", "13:00").option(1),
            teaching("H", "148246", 1, "Übung", 2, "07:30", "09:00").option(2),
            teaching("H", "148932", 1, "Übung", 3, "13:45", "15:15").option(3),
            teaching("M", "300", 1, "Vorlesung", 2, "07:30", "09:00"),
            teaching("M", "301", 1, "Vorlesung", 1, "09:15", "10:45"),
        ];
        let choices = [
            choice_row("H", "program", 1, 1, "148245"),
            choice_row("H", "program", 1, 2, "148246"),
            choice_row("H", "program", 1, 3, "148932"),
        ];
        let modules = ["H", "M"];
        let none = Selection::default();
        let hidden = |t: &Timetable| -> Vec<(String, HiddenBy)> { t.events.iter().filter_map(|e| Some((e.id.clone(), e.hidden?))).collect() };
        let pair = |id: &str, by: HiddenBy| (id.to_string(), by);

        // Nothing chosen: one choice of three open options; the Tuesday exercise meets M's
        // lecture, but another is free; the Aufbaukurs meets the other one and is an extra.
        let t = choosing(&rows, &modules, &choices, &[], &none);
        assert_eq!(t.choices.len(), 1);
        let across = &t.choices[0];
        assert_eq!((across.module.as_str(), across.kind, across.pick, across.options.len(), across.by), ("H", ChoiceKind::Program, 1, 3, None));
        assert!(across.open(&t.events) && hidden(&t).is_empty());
        assert!(event(&t, "148246").across.is_some_and(|alt| alt.open && alt.option == 1 && alt.take == 1));
        assert!(event(&t, "150190").extra && event(&t, "150190").across.is_none());
        assert!(t.clashes.is_empty() && t.blocked_choices.is_empty(), "{:?}", t.clashes);
        // The week: the options dashed („1 von 3"), the extra marked.
        let week = t.regular_week();
        let alt_of = |id: &str| week.iter().find(|item| t.events[item.event].id == id).map(|item| (item.alt, item.extra));
        assert_eq!(alt_of("148246"), Some((Some((3, 1)), false)));
        assert_eq!(alt_of("150190"), Some((None, true)));
        assert_eq!(alt_of("148060"), Some((None, false)));

        // Bauingenieurwesen, listed at the Tuesday exercise: it is taken, the others go, and its
        // clash with M's lecture counts.
        let t = choosing(&rows, &modules, &choices, &[listed_at("H", 2)], &none);
        assert_eq!((t.choices[0].by, t.choices[0].taken.clone()), (Some(TakenBy::Program), BTreeSet::from([1])));
        assert_eq!(hidden(&t), [pair("148245", HiddenBy::Program), pair("148932", HiddenBy::Program)]);
        assert!(!t.choices[0].open(&t.events) && event(&t, "148246").across.is_some_and(|alt| !alt.open));
        assert_eq!(t.clashes.len(), 1);
        assert_eq!((t.events[t.clashes[0].a.0].id.as_str(), t.events[t.clashes[0].b.0].id.as_str()), ("148246", "300"));

        // The visitor's choice of the Wednesday exercise wins over the program's.
        let wednesday = rows[4].key();
        let chosen = Selection { chosen_rows: [wednesday].into(), ..Selection::default() };
        let t = choosing(&rows, &modules, &choices, &[listed_at("H", 2)], &chosen);
        assert_eq!((t.choices[0].by, t.choices[0].taken.clone()), (Some(TakenBy::Chosen), BTreeSet::from([2])));
        assert_eq!(hidden(&t), [pair("148245", HiddenBy::Option), pair("148246", HiddenBy::Option)]);
        assert!(t.clashes.is_empty());

        // Listed at two: the third goes, the choice is open between the two.
        let t = choosing(&rows, &modules, &choices, &[listed_at("H", 1), listed_at("H", 3)], &none);
        assert_eq!(t.choices[0].narrowed, Some(BTreeSet::from([0, 2])));
        assert_eq!(hidden(&t), [pair("148246", HiddenBy::Program)]);
        assert!(t.choices[0].open(&t.events) && t.choices[0].choosable(&t.events) == [0, 2]);

        // Every option meets the plan: the choice is blocked, and its clashes count.
        let mut busy = rows.to_vec();
        busy.push(teaching("M", "302", 1, "Vorlesung", 4, "11:30", "13:00"));
        busy.push(teaching("M", "303", 1, "Vorlesung", 3, "13:45", "15:15"));
        let t = choosing(&busy, &modules, &choices, &[], &none);
        assert_eq!(t.blocked_choices, [0]);
        assert_eq!(t.clashes.len(), 3);

        // A choice of more options than the week shows stands in the list alone.
        let mut many = Vec::new();
        let mut rows_of_many = vec![teaching("P", "400", 1, "Vorlesung", 1, "08:00", "09:30")];
        for n in 1..=5_u32 {
            let id = format!("41{n}");
            rows_of_many.push(teaching("P", &id, 1, "Projekt", i64::from(n), "11:00", "12:30").option(n));
            many.push(choice_row("P", "topic", 1, n, &id));
        }
        let t = choosing(&rows_of_many, &["P"], &many, &[], &none);
        assert!(t.choices[0].open(&t.events));
        assert_eq!(t.regular_week().iter().map(|item| t.events[item.event].id.as_str()).collect::<Vec<_>>(), ["400"]);
    }

    /// A choice that takes two of four: open until two are chosen, one at a time; then the others
    /// go.
    #[test]
    fn a_choice_of_two_stays_open_until_both_are_taken() {
        let mut rows = Vec::new();
        let mut choices = Vec::new();
        for n in 1..=4_u32 {
            let id = format!("51{n}");
            rows.push(teaching("S", &id, 1, "Seminar", i64::from(n), "11:00", "12:30").option(n));
            choices.push(choice_row("S", "topic", 2, n, &id));
        }
        let take = |t: &Timetable| (t.choices[0].open(&t.events), t.choices[0].take(&t.events), t.choices[0].choosable(&t.events).len());
        let t = choosing(&rows, &["S"], &choices, &[], &Selection::default());
        assert_eq!(take(&t), (true, 2, 4));
        assert!(t.events.iter().all(|event| event.across.is_some_and(|alt| alt.open && alt.take == 2)));
        // One taken: one more to take of the three others, nothing hidden.
        let one = Selection { chosen_rows: [rows[0].key()].into(), ..Selection::default() };
        let t = choosing(&rows, &["S"], &choices, &[], &one);
        assert_eq!((take(&t), t.choices[0].by), ((true, 1, 3), Some(TakenBy::Chosen)));
        assert!(t.events.iter().all(|event| event.hidden.is_none()));
        assert!(event(&t, "511").across.is_some_and(|alt| !alt.open) && event(&t, "512").across.is_some_and(|alt| alt.open && alt.take == 1));
        // Two taken: the choice is made, the other two go.
        let two = Selection { chosen_rows: [rows[0].key(), rows[2].key()].into(), ..Selection::default() };
        let t = choosing(&rows, &["S"], &choices, &[], &two);
        assert_eq!(take(&t), (false, 0, 0));
        let hidden: Vec<(&str, Option<HiddenBy>)> = t.events.iter().map(|event| (event.id.as_str(), event.hidden)).collect();
        assert_eq!(hidden, [("511", None), ("512", Some(HiddenBy::Option)), ("513", None), ("514", Some(HiddenBy::Option))]);
    }

    #[test]
    fn a_visit_meets_nothing_gives_way_and_brings_no_exam() {
        // The plan's lecture on Tuesday 13:45 (12104); Denkmalpflege only visited, its lecture on
        // Tuesday 14:00 and its Übung on Thursday (13676); Biologie only visited, on Tuesday 13:45
        // too (41103).
        let rows = [
            teaching("12104", "1", 1, "Vorlesung", 2, "13:45", "15:15"),
            teaching("13676", "2", 1, "Vorlesung", 2, "14:00", "15:30"),
            teaching("13676", "3", 1, "Übung", 4, "13:45", "15:15"),
            teaching("41103", "4", 1, "Vorlesung", 2, "13:45", "15:15"),
        ];
        let modules = ["12104", "13676", "41103"];
        let visits = Selection { visits: ["13676".to_string(), "41103".to_string(), "99999".to_string()].into(), ..Selection::default() };
        let t = table(&rows, &modules, &[], &visits);
        assert_eq!(t.visits, BTreeSet::from(["13676".to_string(), "41103".to_string()]), "of the timetable's modules alone");
        assert_eq!(["1", "2", "3", "4"].map(|id| event(&t, id).visit), [false, true, true, true]);
        // No clash, nothing blocked: the visits give way where they meet the plan's lecture, and
        // leave each other alone.
        assert!(t.clashes.is_empty() && clash::overlaps(&t.events).is_empty() && clash::hard_rows(&t.events).is_empty());
        let at = |id: &str| t.events.iter().position(|event| event.id == id).unwrap();
        let yields: Vec<(usize, usize, u8, u16, u16)> = clash::yields(&t.events).iter().map(|y| (y.a.0, y.b.0, y.weekday, y.from, y.to)).collect();
        assert_eq!(yields, [(at("2"), at("1"), 2, 14 * 60, 15 * 60 + 15), (at("4"), at("1"), 2, 13 * 60 + 45, 15 * 60 + 15)]);
        // In the week at the visit's slot, the plan's slot as it was.
        let week = t.regular_week();
        let slot = |id: &str| week.iter().find(|item| item.event == at(id)).unwrap();
        assert!(slot("2").visit && !slot("2").clash && slot("2").yields_in(clash::Weeks::All) == [at("1")]);
        assert!(slot("3").visit && slot("3").yields.is_empty());
        assert!(!slot("1").visit && !slot("1").clash && slot("1").yields.is_empty());
        // Planned, the same modules clash as any.
        let planned = table(&rows, &modules, &[], &Selection::default());
        assert!(planned.visits.is_empty() && !planned.clashes.is_empty() && clash::yields(&planned.events).is_empty());

        // An exam of the visit is none of the timetable's; one of the plan's module stays.
        let exam = |module: &str, event: &str| {
            let mut row = teaching(module, event, 1, "Prüfung", 3, "10:00", "12:00").0;
            row.date.event_type = None;
            row.date.rhythm = None;
            row.date.rhythm_raw = None;
            row.date.first_date = Some("2027-02-10".into());
            row.date.last_date = Some("2027-02-10".into());
            row
        };
        let facts = winter();
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let (ids, exams) = (ids(&modules), [exam("12104", "8"), exam("13676", "9")]);
        let input = Input { key: facts.key, semester: None, facts: &facts, modules: &ids, schedule: &schedule, exams: &exams, sws: &[], taken: &[], choices: &[], listed: &[] };
        let exams_of = |selection: &Selection| Timetable::build(&input, selection).exams.iter().map(|exam| exam.event_id.clone()).collect::<Vec<_>>();
        assert_eq!(exams_of(&visits), ["8"]);
        assert_eq!(exams_of(&Selection::default()), ["8", "9"]);
    }
}
