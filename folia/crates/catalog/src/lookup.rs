//! A Termin to look up (`TerminMode::LookUp`; folia/design/termin, „Zum Nachschlagen"), wherever it
//! stands: its own page (`event.rs`) and beside its module's page (`module.rs`, `?termin=<key>
//! &on=<day>`).
//!
//! What it reads is the timetable of the modules that link the event, in the newest semester it is
//! held in (`EventAsk`): one answer for every Termin and date of the event. The app adds the
//! visitor's plan of that semester where they have one (the Stundenplan's own `StudyplanAsk`; R9:
//! the server knows no plan and says nothing of it): a module in it is read in the plan, its groups
//! ticked as there, with „Anpassen ›" into the Stundenplan of that semester (one place to adjust;
//! owner, 2026-10-26, „Ja zu allen drei Vorschlägen"); one outside it is measured against the plan
//! (`Against::Plan`).

use std::collections::BTreeMap;

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_data::{use_data, DataError};
use folia_pages::ask::{EventAsk, StudyplanAsk};
use folia_pages::StudyplanData;
use folia_routes::url::StudyplanUrl;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::Studyplan;
use folia_timetable::model::Timetable;
use folia_widgets::termin::{module_facts, Against, ModuleFacts, TerminCtx, TerminLinks, TerminMode, TerminPlace};
use leptos::prelude::*;

use crate::i18n;

/// The browser app (`csr`), or the server rendering the page the same for everybody.
const APP: bool = cfg!(feature = "csr");

/// What there is to show of the Termin asked for: none (an address of no Termin, an event no module
/// links), an error, or the Termin.
#[derive(Clone, Debug, PartialEq)]
enum Loaded {
    Missing,
    Failed(DataError),
    Found(Box<Found>),
}

/// The Termin and day asked for, with the timetable of the event's modules and the visitor's plan
/// of its semester where they have one (the app's alone, R9). They stay together while the answer
/// for another event is on its way, so the view never reads one Termin in another event's
/// timetable (R16).
#[derive(Clone, Debug, PartialEq)]
struct Found {
    key: RowKey,
    day: Option<Day>,
    event: StudyplanData,
    plan: Option<StudyplanData>,
}

impl Found {
    /// The Termin's module is in the visitor's plan of the event's semester.
    fn planned(&self, plan: Option<Studyplan>) -> bool {
        APP && plan.is_some_and(|plan| self.event.ids.iter().any(|id| plan.is_planned(self.event.key, id)))
    }

    /// The plan's timetable, as the Stundenplan shows it.
    fn plan_table(&self, plan: Option<Studyplan>, mine: Option<MyProgram>) -> Option<Timetable> {
        let (data, plan) = (self.plan.as_ref()?, plan?);
        Some(data.timetable(&plan.selection(self.event.key, mine.map(MyProgram::town).unwrap_or_default())))
    }
}

/// What a view to look up a Termin reads, each a memo of the one answer (`loaded`).
#[derive(Clone, Copy)]
pub(crate) struct LookUp {
    /// `None` while the first answer is on its way; then whether there is such a Termin, or why
    /// there is no answer.
    pub phase: Memo<Option<Result<bool, DataError>>>,
    /// A module of the plan is read in the plan, one outside it in its own timetable, as QIS has
    /// it.
    pub table: Memo<Option<Timetable>>,
    pub against: Memo<Against>,
    /// Whether it is in the plan: on the server not known (R9).
    pub planned: Memo<Option<bool>>,
    /// The Termin and day the view shows.
    pub asked: Memo<Option<(RowKey, Option<Day>)>>,
    /// The modules of both, by id: the chips of the plan's Termine too.
    pub modules: Memo<BTreeMap<String, ModuleFacts>>,
    /// The event's semester.
    pub semester: Memo<Option<SemesterKey>>,
}

/// The Termin `asked` to look up.
pub(crate) fn look_up(asked: Memo<Option<(RowKey, Option<Day>)>>) -> LookUp {
    let t = i18n::t();
    let client = use_data();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let loaded = Memo::new(move |before: Option<&Option<Loaded>>| {
        let Some((key, day)) = asked.get() else { return Some(Loaded::Missing) };
        // On its way: what was shown stays, its own Termin with it.
        let kept = || before.cloned().flatten();
        let client = match client.as_ref() {
            Ok(client) => client,
            Err(error) => return Some(Loaded::Failed(error.clone())),
        };
        let event = match client.get(&EventAsk { event: key.event.to_string(), locale: t.locale }) {
            None => return kept(),
            Some(Ok(Some(event))) => event,
            Some(Ok(None)) => return Some(Loaded::Missing),
            Some(Err(error)) => return Some(Loaded::Failed(error)),
        };
        // The visitor's plan of the event's semester, asked as the Stundenplan asks it.
        let ids = match (APP, plan) {
            (true, Some(plan)) => plan.modules_in(event.key),
            _ => Vec::new(),
        };
        if ids.is_empty() {
            return Some(Loaded::Found(Box::new(Found { key, day, event, plan: None })));
        }
        let program = plan.and_then(|plan| plan.with(|doc| doc.program.clone())).or_else(|| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
        match client.get(&StudyplanAsk { key: event.key, ids, program, locale: t.locale }) {
            None => kept(),
            Some(Ok(data)) => Some(Loaded::Found(Box::new(Found { key, day, event, plan: Some(data) }))),
            Some(Err(error)) => Some(Loaded::Failed(error)),
        }
    });
    let phase = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            None => None,
            Some(Loaded::Missing) => Some(Ok(false)),
            Some(Loaded::Failed(error)) => Some(Err(error.clone())),
            Some(Loaded::Found(_)) => Some(Ok(true)),
        })
    });
    let found = move |read: &dyn Fn(&Found) -> Option<Timetable>| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { read(found) } else { None });
    let table = Memo::new(move |_| found(&|found| if found.planned(plan) { found.plan_table(plan, mine) } else { Some(found.event.timetable(&Selection::default())) }));
    let against = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Some(Loaded::Found(found)) if found.planned(plan) => Against::Itself,
            Some(Loaded::Found(found)) => found.plan_table(plan, mine).map_or(Against::Nothing, |table| Against::Plan(Box::new(table))),
            _ => Against::Nothing,
        })
    });
    let planned = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { APP.then(|| found.planned(plan)) } else { None }));
    let shown_asked = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { Some((found.key, found.day)) } else { None }));
    let modules = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Some(Loaded::Found(found)) => {
                let mut all: BTreeMap<_, _> = found.plan.as_ref().map(module_facts).unwrap_or_default();
                for (id, facts) in module_facts(&found.event) {
                    all.entry(id).or_insert(facts);
                }
                all
            }
            _ => BTreeMap::new(),
        })
    });
    let semester = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { Some(found.event.key) } else { None }));
    LookUp { phase, table, against, planned, asked: shown_asked, modules, semester }
}

impl LookUp {
    /// „Anpassen ›": the Termin in the Stundenplan of the event's semester.
    pub fn adjust(self) -> Callback<(RowKey, Option<Day>), String> {
        let t = i18n::t();
        let semester = self.semester;
        Callback::new(move |(key, on): (RowKey, Option<Day>)| {
            let at = StudyplanUrl { sem: semester.get_untracked().map(|semester| semester.key()), ..StudyplanUrl::default() };
            t.path(&at.with_termin(Some((key, on))).path())
        })
    }

    /// The view of it with these links, standing there, at the time `now` says.
    pub fn ctx(self, links: TerminLinks, place: TerminPlace, now: Signal<Option<(Day, u16)>>) -> TerminCtx {
        TerminCtx {
            table: self.table,
            asked: self.asked,
            modules: self.modules,
            mode: TerminMode::LookUp,
            plan: None,
            now,
            planned: self.planned.into(),
            against: self.against.into(),
            links,
            place,
        }
    }
}
