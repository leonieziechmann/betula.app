//! The workbench (folia/design/hinzufuegen/konzepte.html, third round; owner, 2026-11-03: „Passt
//! so, bau das jetzt in Folia"): „Module hinzufügen" makes planning a mode for one semester, which
//! „Fertig" or Esc end (`Bench`). Its bar stands over all it changes: „Planen", the semester with
//! ‹ › (another one without leaving the mode), what it holds against the plan, „Fertig".
//!
//! On a desktop three columns under it. On the left the areas of the plan as the overview has
//! them (owner, the same day: „dass da die Bereicht sind, die man auswählen kann und sich dann
//! ausklappt mit den untergruppen, wenn es gibt"): „Laut Plan im 3. FS" first, one row per area
//! with its dot and its LP, a click folding an area with subgroups out at the one that is due
//! (`folia_plans::bench`), one area open at a time; „Gemerkt" and „Im Katalog suchen" under them.
//! In the middle what is picked: „Laut Plan" with „Plan übernehmen" (it plans at once, with
//! „Rückgängig"), a choice with „Wählen ›", what is open from before and what the next Fachsemester
//! could take now; a subgroup's modules in groups by the student's week (`pages::fit`: they fit,
//! they meet it, they have no fixed dates, the semester does not offer them), with „Fach", „Im
//! WiSe angeboten", „Passt in meine Woche" and a search above them, which change only what stands
//! below them (CLAUDE.md, „What picks stands above what it changes"). Each row plans at once
//! („+ Einplanen") and counts for the subgroup it was planned from (`bench::plan_for`); a click on
//! it opens the module beside the page, over the column on the right. That column is the semester,
//! and only what is taken in it (owner: „Da sollte nur zu sehen sein, was man alles belegt hat"),
//! each module with the subgroup it counts for and × to take it out.
//!
//! On a phone the area is a button over the list that opens the same areas as a sheet, a tap
//! folding an area out; the bar of the mode takes the tab bar's place, as the bar of the rows
//! selected does, and a tap on it shows what the semester holds. Back leaves the mode there.
//!
//! What needs many filters (a language, an exam, the credits) is the catalog's: „Im Katalog
//! suchen" opens it with the subgroup's modules and the semester as its target
//! (`folia_stores::studyplan::Planning::set_bench`), and „Zurück zur Werkbank" comes back.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_model::labels::OfferStatus;
use folia_model::rows::CatalogRow;
use folia_pages::ask::{FitAsk, StudyModulesAsk};
use folia_pages::FitResult;
use folia_plans::bench::{self as model, Bench as Groups, Due, Group, GroupKind};
use folia_plans::choices::{self, PlanView, RowState};
use folia_plans::study::{self, Offer, Pick, Subject, When};
use folia_plans::studyplan::RowRef;
use folia_plans::variants;
use folia_routes::filter::{CatalogQuery, FitsFilter, ProgramScope, TurnusFilter};
use folia_routes::url::CatalogUrl;
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::combobox::{ComboItem, Select};
use folia_design::ui::{Icon, Shortcut};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::studyplan::BenchAim;

use super::dialog::DialogHead;
use super::picker::offer_note;
use super::{module_href, n, now_secs, Dialog, Plans, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// The workbench open over „Mein Studium": the semester it plans, what the list of the areas has
/// picked, and the area folded out there (one at a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bench {
    pub semester: SemesterKey,
    pub pick: BenchPick,
    pub folded: Option<usize>,
}

/// What the list of the areas has picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BenchPick {
    /// „Laut Plan im 3. FS".
    Plan,
    /// An area of the progress, at the subgroup it opens with (`Groups::opens_with`).
    Area(usize),
    /// A subgroup: its area and what it is.
    Group(usize, GroupKind),
    /// „Gemerkt".
    Marked,
}

/// Opens the workbench for semester `semester` at `pick`: what lay over the page goes, the
/// semester is the one in focus.
pub(super) fn enter(ctx: StudyCtx, semester: SemesterKey, pick: BenchPick) {
    ctx.dialog.set(None);
    ctx.menu.set(None);
    ctx.selection.set(super::Selection::default());
    ctx.focus.set(Some(semester));
    let folded = match pick {
        BenchPick::Area(area) | BenchPick::Group(area, _) => Some(area),
        BenchPick::Plan | BenchPick::Marked => None,
    };
    ctx.bench.set(Some(Bench { semester, pick, folded }));
    super::dom::page_to_top();
}

/// „Fertig": the mode ends, the semester planned in focus.
pub(super) fn leave(ctx: StudyCtx) {
    if let Some(bench) = ctx.bench.get_untracked() {
        ctx.focus.set(Some(bench.semester));
    }
    ctx.dialog.set(None);
    ctx.bench.set(None);
    super::dom::page_to_top();
}

/// What the workbench works with, as the plan stands: the plan's view of the study, its areas with
/// their subgroups, what the semester's Fachsemester asks.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Model {
    pub view: PlanView,
    pub groups: Groups,
    pub due: Due,
    /// The semester's Fachsemester.
    pub fs: Option<u8>,
}

fn model_of(ctx: StudyCtx, s: SemesterKey) -> Option<Model> {
    ctx.with_input(|input, ready| {
        let study = &ready.study;
        let view = choices::plan_view(input, study);
        let groups = Groups::of(study, &view);
        let fs = study.semester(s).and_then(|semester| semester.fs);
        let now_fs = study.semester(study.now).and_then(|semester| semester.fs);
        let due = Due::of(&view, fs, now_fs);
        Model { view, groups, due, fs }
    })
}

/// What the middle shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pane {
    Plan,
    /// A subgroup: an index of `Groups::groups`.
    Group(usize),
    Marked,
}

fn pane_of(model: &Model, pick: BenchPick) -> Pane {
    match pick {
        BenchPick::Plan => Pane::Plan,
        BenchPick::Marked => Pane::Marked,
        BenchPick::Area(area) => model.groups.opens_with(&model.view, area, model.fs).map_or(Pane::Plan, Pane::Group),
        BenchPick::Group(area, kind) => model.groups.find(area, kind).map_or(Pane::Plan, Pane::Group),
    }
}

/// A subgroup's name: „Pflichtmodule", or the choice's name, whole or short (`bench::short_name`).
fn group_name(model: &Model, group: &Group, short: bool, t: &Texts) -> String {
    match group.kind {
        GroupKind::Pflicht => t.study_bench.pflicht.to_string(),
        GroupKind::Choice(at) => model
            .view
            .choices
            .get(at)
            .map(|choice| match short {
                // „FÜS", as the overview calls the area.
                true if choice.fues => t.study.fues.to_string(),
                true => model::short_name(&choice.name),
                false => choice.name.clone(),
            })
            .unwrap_or_default(),
    }
}

/// What a subgroup's row says: „2 offen", „6/12", or a check once it is complete.
fn group_figure(model: &Model, group: &Group, t: &Texts) -> (String, bool) {
    match group.kind {
        GroupKind::Pflicht if group.open == 0 => ("✓".to_string(), true),
        GroupKind::Pflicht => ((t.study_bench.open_n)(group.open), false),
        GroupKind::Choice(at) => {
            let Some(choice) = model.view.choices.get(at) else { return (String::new(), false) };
            if choice.complete() {
                ("✓".to_string(), true)
            } else {
                (format!("{}/{}", n(choice.taken_credits(), t), folia_plans::plan::credits_in(&choice.credits_text, t.locale)), false)
            }
        }
    }
}

/// What an area's row says: what is passed and planned of what it asks, or a check.
fn area_figure(ready: &Ready, area: usize, t: &Texts) -> (String, bool) {
    let Some(progress) = ready.study.progress.get(area) else { return (String::new(), false) };
    let have = progress.passed + progress.planned;
    if model::reaches(have, progress.area.required) {
        ("✓".to_string(), true)
    } else {
        (format!("{}/{}", n(have, t), n(progress.area.required, t)), false)
    }
}

/// „3. FS" of the semester, else nothing.
fn fs_words(model: &Model, t: &Texts) -> Option<String> {
    model.fs.map(|fs| (t.study.fs)(fs))
}

/// The subgroup a row of the plan belongs to: its choice, or its area's Pflicht.
fn group_of_line(model: &Model, at: usize) -> Option<(usize, GroupKind)> {
    let line = model.view.lines.get(at)?;
    let index = match line.choice {
        Some(choice) => model.groups.of_choice(choice)?,
        None => model.groups.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(&at))?,
    };
    model.groups.groups.get(index).map(|group| (group.area, group.kind))
}

/// Picks `pick` in the list of the areas, the area `folded` folded out.
fn pick(ctx: StudyCtx, pick: BenchPick, folded: Option<usize>) {
    ctx.bench.update(|bench| {
        if let Some(bench) = bench {
            bench.pick = pick;
            bench.folded = folded;
        }
    });
}

/// Turns the workbench to semester `to`.
fn turn(ctx: StudyCtx, to: SemesterKey) {
    ctx.bench.update(|bench| {
        if let Some(bench) = bench {
            bench.semester = to;
        }
    });
    ctx.focus.set(Some(to));
}

/// Plans module `id` into `s` for the subgroup `group`, with the note „Rückgängig" takes it back
/// by: where it counts, where that is a subgroup of its own.
fn plan_module(ctx: StudyCtx, s: SemesterKey, id: String, title: String, group: Option<(usize, GroupKind)>, counts: Option<String>, t: &'static Texts) {
    let b = &t.study_bench;
    let note = match counts {
        Some(name) => (b.planned_for_note)(&title, &name),
        None => (b.planned_note)(&title, &s.label(t.locale)),
    };
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            let study = study::study(input);
            let view = choices::plan_view(input, &study);
            let groups = Groups::of(&study, &view);
            let group = group.and_then(|(area, kind)| groups.find(area, kind)).and_then(|at| groups.groups.get(at));
            model::plan_for(doc, input, &view, s, &id, group, now_secs());
        },
        || {},
    );
}

/// Takes module `id` out of `s`, and what was passed of it there.
fn unplan_module(ctx: StudyCtx, s: SemesterKey, id: String, title: String, t: &'static Texts) {
    ctx.change_noted(
        Some((t.study.removed)(&title)),
        move |doc, _| {
            doc.unplan(s, &id, &[]);
            if doc.passed_in(&id) == Some(s) {
                doc.unpass(&id);
            }
        },
        || {},
    );
}

/// The catalog's rows a pane asks for besides the ones the page has: a choice's lists, the FÜS
/// list, the Merkliste.
fn asked_ids(model: &Model, pane: Pane, fues: &[String], marked: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = match pane {
        Pane::Group(at) => model.groups.groups.get(at).map(|group| model::candidates(&model.view, group, fues).into_iter().map(|(id, _)| id).collect()).unwrap_or_default(),
        Pane::Marked => marked.to_vec(),
        Pane::Plan => Vec::new(),
    };
    ids.sort();
    ids.dedup();
    ids
}

/// The FÜS list of the program.
fn fues_of(ctx: StudyCtx) -> Vec<String> {
    ctx.plans.with(|plans| match plans {
        Plans::Found(source) => source.fues.clone(),
        _ => Vec::new(),
    })
}

/// What `pages::fit` says of the semester's week, as sets.
#[derive(Clone, Debug, Default, PartialEq)]
struct Week {
    has_data: bool,
    fitting: BTreeSet<String>,
    excluded: BTreeSet<String>,
    unknown: BTreeSet<String>,
    notes: BTreeMap<String, String>,
}

impl Week {
    fn of(result: &FitResult) -> Self {
        Week {
            has_data: result.has_data,
            fitting: result.fitting.iter().cloned().collect(),
            excluded: result.excluded.iter().cloned().collect(),
            unknown: result.unknown.clone(),
            notes: result.notes.clone(),
        }
    }
}

/// What a module means for the semester's week.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    Fits,
    /// Some of its events are free („Übung 1 von 3 frei").
    Partly(String),
    /// It cannot be checked („keine festen Termine").
    Unknown(String),
    Clash,
    /// No events in the semester.
    NoDates,
    /// The semester has no dates yet: nothing is checked.
    Unchecked,
    /// The semester does not offer it.
    Unoffered,
}

impl Fit {
    fn of(week: Option<&Week>, row: &CatalogRow, s: SemesterKey, planned: bool) -> Self {
        let id = row.id.as_str();
        let checked = week.filter(|week| week.has_data);
        if planned {
            return Fit::Fits;
        }
        // Its turnus first: a date of a semester that does not offer it is an exam or a retake.
        if !Offer::of(row).offered(s) {
            return Fit::Unoffered;
        }
        let Some(week) = checked else { return Fit::Unchecked };
        if week.unknown.contains(id) {
            return Fit::Unknown(week.notes.get(id).cloned().unwrap_or_default());
        }
        if week.fitting.contains(id) {
            return match week.notes.get(id) {
                Some(note) => Fit::Partly(note.clone()),
                None => Fit::Fits,
            };
        }
        if week.excluded.contains(id) {
            return Fit::Clash;
        }
        Fit::NoDates
    }

    /// The group of a list it stands in: 0 fits, 1 meets the week, 2 no fixed dates, 3 not
    /// offered.
    fn group(&self) -> u8 {
        match self {
            Fit::Fits | Fit::Partly(_) => 0,
            Fit::Clash => 1,
            Fit::Unknown(_) | Fit::NoDates | Fit::Unchecked => 2,
            Fit::Unoffered => 3,
        }
    }

    /// It can be taken in the week as it stands: „Passt in meine Woche" keeps it.
    fn fits(&self) -> bool {
        matches!(self, Fit::Fits | Fit::Partly(_) | Fit::Unknown(_))
    }

    /// The mark of a row: its class and words.
    fn mark(&self, s: SemesterKey, t: &Texts) -> Option<(&'static str, String)> {
        let b = &t.study_bench;
        match self {
            Fit::Fits => Some(("ok", b.fit_ok.to_string())),
            Fit::Partly(note) => Some(("part", note.clone())),
            Fit::Unknown(note) => Some(("quiet", if note.is_empty() { b.fit_none.to_string() } else { note.clone() })),
            Fit::Clash => Some(("warn", b.fit_clash.to_string())),
            Fit::NoDates => Some(("quiet", b.fit_none.to_string())),
            Fit::Unchecked => None,
            Fit::Unoffered => Some(("quiet", offer_note_short(s, t))),
        }
    }
}

/// „nicht im WiSe".
fn offer_note_short(s: SemesterKey, t: &Texts) -> String {
    (t.study_bench.group_unoffered)(season(s, t))
}

/// „WiSe", „SoSe".
fn season(s: SemesterKey, t: &Texts) -> &'static str {
    if s.winter {
        t.format.winter_short
    } else {
        t.format.summer_short
    }
}

/// A module of a list as the workbench draws it.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    id: String,
    title: String,
    credits: Option<f64>,
    /// What it is under its title: its „Fach", „Pflicht · Informatik", „laut Plan 4. FS".
    sub: Option<String>,
    /// „jedes WiSe · DE · 2 Termine".
    facts: String,
    fit: Fit,
    /// Passed somewhere; planned in the semester; planned in another one from now on.
    passed: bool,
    here: bool,
    elsewhere: Option<SemesterKey>,
    /// The subgroup a click plans it for, and where it counts then.
    group: Option<(usize, GroupKind)>,
    counts: Option<String>,
    /// On the Merkliste: the names of its collections, empty for none (folia/design/merken/
    /// konzept.html, „Von überall": a list says so wherever it lists a marked module).
    marked: Option<String>,
}

/// „jedes WiSe · DE · 2 Termine".
fn facts_of(row: Option<&CatalogRow>, s: SemesterKey, t: &Texts) -> String {
    let Some(row) = row else { return String::new() };
    let (_, offer) = offer_note(Offer::of(row), s, t);
    let language = folia_design::format::languages(row.teaches_german, row.teaches_english);
    let events = if row.teaching_events > 0 { (t.catalog.events)(row.teaching_events) } else { t.catalog.events_none.to_string() };
    [Some(offer), language.map(str::to_string), Some(events)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// The line of module `id`, as the plan stands.
#[allow(clippy::too_many_arguments)]
fn line_of(ctx: StudyCtx, id: &str, row: Option<&CatalogRow>, sub: Option<String>, week: Option<&Week>, s: SemesterKey, group: Option<(usize, GroupKind)>, counts: Option<String>, t: &Texts) -> Line {
    let (passed, here, elsewhere) = ctx
        .plan
        .map(|plan| {
            plan.with_untracked(|doc| {
                let now = ctx.now.get_untracked();
                (doc.passed_in(id).is_some(), doc.is_planned(s, id), doc.planned_in(id).into_iter().filter(|at| *at != s && *at >= now).min())
            })
        })
        .unwrap_or_default();
    let fit = row.map_or(Fit::Unchecked, |row| Fit::of(week, row, s, here));
    let marked = Bookmarks::expect().filter(|bookmarks| bookmarks.is_marked_untracked(id)).map(|_| folia_stores::collections::Collections::expect().map(|shelf| shelf.with_untracked(|shelf| shelf.names_of(id))).unwrap_or_default());
    Line {
        id: id.to_string(),
        title: row.map_or_else(|| id.to_string(), |row| row.title.clone()),
        credits: row.and_then(|row| row.credits),
        sub,
        facts: facts_of(row, s, t),
        fit,
        passed,
        here,
        elsewhere,
        group,
        counts,
        marked,
    }
}

/// The workbench on a desktop: the bar of the mode over the areas, the pane and the semester.
#[component]
pub(super) fn BenchDesk(ctx: StudyCtx) -> impl IntoView {
    let parts = Parts::new(ctx);
    view! {
        <section class="st-wb" aria-label=i18n::t().study_bench.mode>
            <ModeBar ctx parts/>
            <div class="st-wb-cols">
                <nav class="panel st-wb-areas" aria-label=i18n::t().study_bench.areas>
                    <div class="st-wb-areas-h">{i18n::t().study_bench.areas}<small>{i18n::t().study_bench.areas_sub}</small></div>
                    <AreaList ctx parts phone=false/>
                </nav>
                <div class="panel st-wb-pane"><PaneView ctx parts/></div>
                <aside class="panel st-wb-sem"><SemesterColumn ctx parts/></aside>
            </div>
        </section>
    }
}

/// The workbench on a phone: the area as a button over the list, the bar of the mode in the tab
/// bar's place.
#[component]
pub(super) fn BenchPhone(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let parts = Parts::new(ctx);
    let picked = Memo::new(move |_| {
        let pane = parts.pane.get()?;
        let model = parts.model.get()?;
        let tone_of = |area: usize| ctx.with_ready(|ready| ready.tone(Some(area))).unwrap_or("var(--text-3)");
        Some(match pane {
            Pane::Plan => (None, model.fs.map_or_else(|| b.by_plan_plain.to_string(), |fs| (b.by_plan)(&(t.study.fs)(fs))), parts.plan_figure.get()),
            Pane::Marked => (None, b.marked.to_string(), None),
            Pane::Group(at) => {
                let group = model.groups.groups.get(at)?;
                let area = ctx.with_ready(|ready| ready.area_name(Some(group.area), t)).unwrap_or_default();
                let many = model.groups.area(group.area).is_some_and(|area| area.groups.len() > 1);
                let name = if many { format!("{area} › {}", group_name(&model, group, true, t)) } else { area };
                (Some(tone_of(group.area)), name, Some(group_figure(&model, group, t)))
            }
        })
    });
    view! {
        <section class="st-wb phone" aria-label=b.mode>
            <button class="st-wb-pick" type="button" aria-haspopup="dialog" on:click=move |_| ctx.open(Dialog::BenchAreas)>
                <span class="st-wb-pick-w">
                    <span class="k">{b.area_pick}</span>
                    {move || picked.get().map(|(tone, name, _)| view! {
                        <b>{tone.map(|tone| view! { <span class="st-dot" style=format!("--c: {tone}")></span> })}<span>{name}</span></b>
                    })}
                </span>
                <span class="st-wb-pick-f">
                    {move || picked.get().and_then(|(_, _, figure)| figure).map(|(text, ok)| view! { <small class:ok=ok>{text}</small> })}
                    <Icon name="chevron-down"/>
                </span>
            </button>
            <div class="st-wb-pane"><PaneView ctx parts/></div>
            <PhoneBar ctx parts/>
        </section>
    }
}

/// What the parts of the workbench share: its model, what is picked, the week.
#[derive(Clone, Copy)]
struct Parts {
    semester: Memo<Option<SemesterKey>>,
    model: Memo<Option<Model>>,
    pane: Memo<Option<Pane>>,
    /// The catalog's rows of what the pane lists, with the page's.
    rows: Memo<BTreeMap<String, CatalogRow>>,
    week: Memo<Option<Week>>,
    /// „24/30": what the semester holds against what the plan puts into it.
    plan_figure: Memo<Option<(String, bool)>>,
    marked: Memo<Vec<String>>,
    /// The switches of a list: „Im WiSe angeboten", „Passt in meine Woche", the search, the
    /// „Fach" (an index of the choice's lists).
    offered: RwSignal<bool>,
    fits: RwSignal<bool>,
    text: RwSignal<String>,
    list: RwSignal<Option<usize>>,
    /// The collection „Gemerkt" shows, all of them where none.
    collection: RwSignal<Option<u32>>,
}

impl Parts {
    /// Reads what the model is made of before the model (R16): its semester, and the program's
    /// plans, the rows and the page's state it is worked out from. A view that reads the model and
    /// then any of them calls this first.
    fn follow(&self, ctx: StudyCtx) {
        self.semester.track();
        ctx.plans.track();
        ctx.rows.track();
        ctx.state.track();
    }

    fn new(ctx: StudyCtx) -> Self {
        let t = i18n::t();
        let semester = Memo::new(move |_| ctx.bench.with(|bench| bench.map(|bench| bench.semester)));
        let model = Memo::new(move |_| semester.get().and_then(|s| model_of(ctx, s)));
        let pane = Memo::new(move |_| {
            let pick = ctx.bench.with(|bench| bench.map(|bench| bench.pick))?;
            model.with(|model| model.as_ref().map(|model| pane_of(model, pick)))
        });
        let bookmarks = Bookmarks::expect();
        let marked = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<Vec<String>>()).unwrap_or_default());
        let ids = Memo::new(move |_| {
            let fues = fues_of(ctx);
            let pane = pane.get();
            model.with(|model| match (model, pane) {
                (Some(model), Some(pane)) => marked.with(|marked| asked_ids(model, pane, &fues, marked)),
                _ => Vec::new(),
            })
        });
        let asked = use_ask(move || {
            let ids = ids.get();
            (!ids.is_empty()).then_some(StudyModulesAsk { ids })
        });
        let rows = Memo::new(move |_| {
            let mut rows: BTreeMap<String, CatalogRow> = ctx.rows.with(|rows| rows.iter().map(|row| (row.id.clone(), row.clone())).collect());
            asked.with(|asked| {
                if let Some(Ok(modules)) = asked {
                    rows.extend(modules.rows.iter().map(|row| (row.id.clone(), row.clone())));
                }
            });
            rows
        });
        // The week of the semester as the plan has it: what it plans, hides and has chosen there.
        let mine = folia_stores::myprogram::MyProgram::expect();
        let fit = use_ask(move || {
            let s = semester.get()?;
            let plan = ctx.plan?;
            let town = mine.map(|mine| mine.town()).unwrap_or_default();
            Some(FitAsk { filter: FitsFilter::all(&s.key()), plan: plan.modules_in(s), selection: plan.selection(s, town), locale: t.locale })
        });
        let week = Memo::new(move |_| fit.with(|fit| fit.as_ref().and_then(|fit| fit.as_ref().ok()).map(Week::of)));
        let plan_figure = Memo::new(move |_| {
            let s = semester.get()?;
            ctx.with_ready(|ready| {
                let semester = ready.study.semester(s)?;
                let planned = semester.planned?;
                Some((format!("{}/{}", n(semester.credits, t), n(planned, t)), semester.credits + 0.5 >= planned))
            })
            .flatten()
        });
        let parts = Parts {
            semester,
            model,
            pane,
            rows,
            week,
            plan_figure,
            marked,
            offered: RwSignal::new(true),
            fits: RwSignal::new(false),
            text: RwSignal::new(String::new()),
            list: RwSignal::new(None),
            collection: RwSignal::new(None),
        };
        // Another subgroup: its own „Fach", the search kept.
        Effect::new(move |before: Option<Option<Pane>>| {
            let now = pane.get();
            if before.is_some_and(|before| before != now) {
                parts.list.set(None);
            }
            now
        });
        parts
    }
}

/// The bar of the mode: „Planen", the semester with ‹ ›, its credits against the plan, „Fertig".
#[component]
fn ModeBar(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let head = Memo::new(move |_| {
        let s = parts.semester.get()?;
        ctx.with_ready(|ready| {
            let study = &ready.study;
            let at = study.semesters.iter().position(|semester| semester.key == s)?;
            let semester = study.semesters.get(at)?;
            let previous = at.checked_sub(1).and_then(|before| study.semesters.get(before)).map(|semester| semester.key);
            let next = study.semesters.get(at + 1).map(|semester| semester.key);
            let fs = ready.fs_label(s, t).map(|fs| match semester.fs {
                Some(number) => (t.study.fs_long)(number),
                None => fs,
            });
            let sub = fs.map(|fs| (b.semester_sub)(&fs, semester.when == When::Now)).unwrap_or_default();
            Some((s.label(t.locale), sub, semester.credits, semester.planned, previous, next))
        })
        .flatten()
    });
    view! {
        <div class="st-wb-bar" role="toolbar" aria-label=b.mode>
            <span class="st-wb-bar-ic" aria-hidden="true"><Icon name="calendar-plus"/></span>
            <b class="st-wb-bar-t">{b.mode}</b>
            {move || head.get().map(|(label, sub, credits, planned, previous, next)| {
                let over = planned.filter(|plan| credits > plan + 0.5).map(|plan| credits - plan);
                let share = planned.filter(|plan| *plan > 0.0).map_or(0.0, |plan| (credits / plan * 100.0).min(100.0));
                let figure = match planned {
                    Some(plan) => (b.of_planned)(&n(credits, t), &n(plan, t)),
                    None => (b.credits)(&n(credits, t)),
                };
                view! {
                    <span class="st-wb-sem-pick">
                        <button class="icon-btn hit" type="button" aria-label=b.previous title=b.previous disabled=previous.is_none() on:click=move |_| if let Some(to) = previous { turn(ctx, to) }><Icon name="chevron-left"/></button>
                        <em>{label}</em>
                        <button class="icon-btn hit" type="button" aria-label=b.next title=b.next disabled=next.is_none() on:click=move |_| if let Some(to) = next { turn(ctx, to) }><Icon name="chevron-right"/></button>
                    </span>
                    <small class="st-wb-bar-sub">{sub}</small>
                    <span class="st-wb-meter" class:heavy=over.is_some()>
                        <span aria-hidden="true"><i style=format!("--p: {share:.0}%")></i></span>
                        <b>{figure}</b>
                        {over.map(|over| view! { <span class="over">{(b.over)(&n(over, t))}</span> })}
                    </span>
                }
            })}
            <button class="btn primary st-wb-done" type="button" data-action="close-focus" title=b.done_hint on:click=move |_| leave(ctx)>
                {b.done}<Shortcut keys="Esc"/>
            </button>
        </div>
    }
}

/// The list of the areas: „Laut Plan" first, each area with its dot and its LP, the one folded
/// out with its subgroups, „Gemerkt" and „Im Katalog suchen" after them. On a phone (`phone`) a
/// tap on an area with subgroups only folds it out: the sheet stays.
#[component]
fn AreaList(ctx: StudyCtx, parts: Parts, phone: bool) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let folded = Memo::new(move |_| ctx.bench.with(|bench| bench.and_then(|bench| bench.folded)));
    let close = move || {
        if phone {
            ctx.dialog.set(None);
        }
    };
    let catalog = Memo::new(move |_| {
        let s = parts.semester.get()?;
        semester_catalog(ctx, s, t)
    });
    move || {
        parts.follow(ctx);
        let Some(model) = parts.model.get() else { return ().into_any() };
        let pane = parts.pane.get();
        let open = folded.get();
        let plan_label = model.fs.map_or_else(|| b.by_plan_plain.to_string(), |fs| (b.by_plan)(&(t.study.fs)(fs)));
        let areas = model.groups.areas.clone();
        let rows = areas
            .into_iter()
            .map(|area| {
                let index = area.area;
                let (name, tone, (figure, ok)) = ctx.with_ready(|ready| (ready.area_name(Some(index), t), ready.tone(Some(index)), area_figure(ready, index, t))).unwrap_or_default();
                let many = area.groups.len() > 1;
                let is_open = many && open == Some(index);
                let only = (!many).then(|| area.groups.first().and_then(|at| model.groups.groups.get(*at)).map(|group| group.kind)).flatten();
                let current = !many && matches!(pane, Some(Pane::Group(at)) if area.groups.first() == Some(&at));
                let model_click = model.clone();
                let click = move |_| match only {
                    Some(kind) => {
                        pick(ctx, BenchPick::Group(index, kind), None);
                        close();
                    }
                    None if phone => ctx.bench.update(|bench| {
                        if let Some(bench) = bench {
                            bench.folded = if bench.folded == Some(index) { None } else { Some(index) };
                        }
                    }),
                    None => {
                        let opens = model_click.groups.opens_with(&model_click.view, index, model_click.fs).and_then(|at| model_click.groups.groups.get(at)).map(|group| group.kind);
                        match opens {
                            Some(kind) if open != Some(index) => pick(ctx, BenchPick::Group(index, kind), Some(index)),
                            _ => ctx.bench.update(|bench| {
                                if let Some(bench) = bench {
                                    bench.folded = if bench.folded == Some(index) { None } else { Some(index) };
                                }
                            }),
                        }
                    }
                };
                let subs = is_open.then(|| {
                    let entries = area
                        .groups
                        .iter()
                        .filter_map(|at| model.groups.groups.get(*at).map(|group| (*at, group.clone())))
                        .map(|(at, group)| {
                            let (figure, ok) = group_figure(&model, &group, t);
                            let name = group_name(&model, &group, true, t);
                            let whole = group_name(&model, &group, false, t);
                            let on = pane == Some(Pane::Group(at));
                            let kind = group.kind;
                            view! {
                                <button class="st-wb-s" type="button" aria-current=on.then_some("true") title=whole on:click=move |_| { pick(ctx, BenchPick::Group(index, kind), Some(index)); close(); }>
                                    <span>{name}</span>
                                    <small class:ok=ok>{figure}{ok.then(|| view! { <span class="visually-hidden">{b.complete}</span> })}</small>
                                </button>
                            }
                        })
                        .collect_view();
                    view! { <div class="st-wb-subs" role="group" aria-label=name.clone()>{entries}</div> }
                });
                view! {
                    <button class="st-wb-a" type="button" style=format!("--c: {tone}") aria-current=current.then_some("true") aria-expanded=many.then_some(if is_open { "true" } else { "false" }) on:click=click>
                        <span class="nm"><span class="st-dot"></span><span>{name.clone()}</span></span>
                        <small class:ok=ok>{figure}</small>
                        {if many { view! { <Icon name=if is_open { "chevron-down" } else { "chevron-right" } class="chev"/> }.into_any() } else { view! { <span class="chev"></span> }.into_any() }}
                    </button>
                    {subs}
                }
            })
            .collect_view();
        let marked = parts.marked.with(Vec::len);
        view! {
            <button class="st-wb-a top" type="button" aria-current=(pane == Some(Pane::Plan)).then_some("true") on:click=move |_| { pick(ctx, BenchPick::Plan, None); close(); }>
                <span class="nm"><Icon name="clipboard-list"/><span>{plan_label}</span></span>
                {move || parts.plan_figure.get().map(|(figure, ok)| view! { <small class:ok=ok>{figure}</small> })}
                <span class="chev"></span>
            </button>
            {rows}
            <hr class="st-wb-sep"/>
            <button class="st-wb-a" type="button" aria-current=(pane == Some(Pane::Marked)).then_some("true") on:click=move |_| { pick(ctx, BenchPick::Marked, None); close(); }>
                <span class="nm"><Icon name="bookmark"/><span>{b.marked}</span></span>
                <small>{(marked > 0).then(|| marked.to_string())}</small>
                <span class="chev"></span>
            </button>
            {move || catalog.get().map(|href| view! {
                <a class="st-wb-a way" href=t.path(&href) rel="nofollow">
                    <span class="nm"><Icon name="search"/><span>{b.in_catalog}</span></span>
                    <small></small>
                    <Icon name="chevron-right" class="chev"/>
                </a>
            })}
        }
        .into_any()
    }
}

/// The catalog with the semester as its target: the program's modules the semester offers.
fn semester_catalog(ctx: StudyCtx, s: SemesterKey, _t: &Texts) -> Option<String> {
    let slug = ctx.with_ready(|ready| ready.program.slug.clone())?;
    let query = CatalogQuery { program: Some(ProgramScope { program_slug: slug, ..Default::default() }), turnus: season_filter(s), ..Default::default() };
    Some(CatalogUrl { query, page: 1, open: None, fill: None }.path())
}

/// Offered in the half of the year of `s`.
fn season_filter(s: SemesterKey) -> TurnusFilter {
    TurnusFilter { winter: s.winter, summer: !s.winter, ..Default::default() }
}

/// The catalog of a subgroup with the semester as its target: the FÜS list (the program's, the
/// BTU's where it has none: `variants::fues_list_for`), a choice's row as the program's page has it
/// (`variants::row_query`), what the semester offers; with the placeholder of the choice standing
/// in the semester that a module found there fills (`fill=p<pid>`).
fn group_catalog(ctx: StudyCtx, model: &Model, group: &Group, s: SemesterKey, t: &Texts) -> Option<String> {
    let choice = model.view.choices.get(group.choice()?)?;
    let rows = model::rows_of(&model.view, group);
    ctx.plans.with_untracked(|plans| {
        let Plans::Found(source) = plans else { return None };
        let mut query = if choice.fues {
            variants::fues_list_for(&source.program, t.locale).0
        } else {
            let line = choice.lines.first().and_then(|at| model.view.lines.get(*at))?;
            let variant = source.variants.iter().find(|variant| variant.full == line.caption)?;
            let entry = variant.entries.iter().find(|entry| entry.ord == line.ord)?;
            variants::row_query(&source.program, variant, entry, &source.areas, t.locale).0
        };
        query.turnus = season_filter(s);
        let fill = ctx.plan.and_then(|plan| plan.with_untracked(|doc| study::open_placeholders(doc, &source.program.id, &rows).into_iter().find(|p| p.semester == s).map(|p| p.pid)));
        Some(CatalogUrl { query, page: 1, open: None, fill }.path())
    })
}

/// What the middle shows.
#[component]
fn PaneView(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    move || match parts.pane.get() {
        None => ().into_any(),
        Some(Pane::Plan) => view! { <PlanPane ctx parts/> }.into_any(),
        Some(Pane::Marked) => view! { <ListPane ctx parts group=None/> }.into_any(),
        Some(Pane::Group(at)) => {
            let pflicht = parts.model.with_untracked(|model| model.as_ref().and_then(|model| model.groups.groups.get(at)).is_some_and(|group| group.kind == GroupKind::Pflicht));
            if pflicht {
                view! { <PflichtPane ctx parts at/> }.into_any()
            } else {
                view! { <ListPane ctx parts group=Some(at)/> }.into_any()
            }
        }
    }
}

/// The head of a pane: the area's dot and the way up to it where it has subgroups, the title, the
/// figure; the rule under it.
fn pane_head(title: String, crumb: Option<String>, tone: Option<&'static str>, figure: Option<AnyView>, rule: Option<String>) -> impl IntoView {
    view! {
        <header class="st-wb-ph">
            <div class="st-wb-pt">
                {tone.map(|tone| view! { <span class="st-dot" style=format!("--c: {tone}")></span> })}
                {crumb.map(|crumb| view! { <span class="st-wb-crumb">{crumb}" ›"</span> })}
                <h3>{title}</h3>
                <span class="st-wb-fig">{figure}</span>
            </div>
            {rule.map(|rule| view! { <p class="st-wb-rule">{rule}</p> })}
        </header>
    }
}

/// A module of a list: its name and what it is, what it means for the week, its credits, and
/// „+ Einplanen" (or that it is planned or passed). A click on it opens the module beside the page.
fn line_view(ctx: StudyCtx, line: Line, s: SemesterKey, t: &'static Texts) -> impl IntoView {
    let b = &t.study_bench;
    let href = module_href(ctx, &line.id, t);
    let open = Memo::new({
        let id = line.id.clone();
        move |_| ctx.url.with(|url| url.open.as_deref() == Some(id.as_str()))
    });
    let mark = (!line.here && !line.passed).then(|| line.fit.mark(s, t)).flatten();
    let credits = line.credits.map(|credits| n(credits, t));
    let elsewhere = line.elsewhere.map(|at| (b.planned_in)(&at.short(t.locale)));
    let act = if line.passed {
        view! { <span class="st-wb-plus quiet"><Icon name="check"/><span>{b.passed}</span></span> }.into_any()
    } else if line.here {
        let (id, title) = (line.id.clone(), line.title.clone());
        let aria = (b.unplan_aria)(&line.title, &s.label(t.locale));
        view! {
            <button class="st-wb-plus done" type="button" aria-label=aria.clone() title=aria on:click=move |_| unplan_module(ctx, s, id.clone(), title.clone(), t)>
                <Icon name="check"/><span>{b.planned}</span>
            </button>
        }
        .into_any()
    } else {
        let (id, title, group, counts) = (line.id.clone(), line.title.clone(), line.group, line.counts.clone());
        let aria = (b.plan_aria)(&line.title, &s.label(t.locale));
        view! {
            <button class="st-wb-plus" type="button" aria-label=aria.clone() title=aria on:click=move |_| plan_module(ctx, s, id.clone(), title.clone(), group, counts.clone(), t)>
                <Icon name="plus"/><span>{b.plan}</span>
            </button>
        }
        .into_any()
    };
    view! {
        <li class="st-wb-row" class:is-open=open class:is-done=line.here || line.passed>
            <a class="st-wb-go" href=href aria-current=move || open.get().then_some("true")>
                <b>{line.title.clone()}</b>
                <small>
                    <span class="mono">{line.id.clone()}</span>
                    {line.marked.clone().map(|names| {
                        let words = if names.is_empty() { b.marked_mark.to_string() } else { format!("{}: ", b.marked_mark) };
                        view! { <span class="st-marked"><Icon name="bookmark"/><span class:visually-hidden=!names.is_empty()>{words}</span>{names}</span> }
                    })}
                    {line.sub.clone().map(|sub| view! { <span>{sub}</span> })}
                    {elsewhere.map(|text| view! { <span class="st-wb-else">{text}</span> })}
                    {(!line.facts.is_empty()).then(|| view! { <span class="st-wb-facts">{line.facts.clone()}</span> })}
                </small>
            </a>
            {match mark {
                Some((class, text)) => view! { <span class=format!("st-wb-fit {class}")><i></i><span>{text}</span></span> }.into_any(),
                None => view! { <span class="st-wb-fit"></span> }.into_any(),
            }}
            <span class="st-wb-lp">{credits.map(|credits| view! { <b>{credits}</b>{t.common.credits_unit} })}</span>
            {act}
        </li>
    }
}

/// A choice in „Laut Plan": its name, what is taken of it and its span, „Wählen ›" (or „Ändern"
/// once it is complete), which opens its subgroup.
fn choice_view(ctx: StudyCtx, model: &Model, choice: usize, t: &'static Texts) -> Option<AnyView> {
    let b = &t.study_bench;
    let at = model.groups.of_choice(choice)?;
    let group = model.groups.groups.get(at)?;
    let item = model.view.choices.get(choice)?;
    let (area, kind) = (group.area, group.kind);
    let (area_name, tone) = ctx.with_ready(|ready| (ready.area_name(Some(area), t), ready.tone(Some(area))))?;
    let complete = item.complete();
    let span = (b.plan_fs)(&(t.study.fs_span)(item.first, item.last));
    let line = (b.choice_line)(&n(item.taken_credits(), t), &folia_plans::plan::credits_in(&item.credits_text, t.locale), &span);
    let taken: Vec<String> = item.taken.iter().map(|taken| taken.name.clone()).collect();
    let credits = folia_plans::plan::credits_in(&item.credits_text, t.locale);
    Some(
        view! {
            <li class="st-wb-row choice" class:is-done=complete>
                <button class="st-wb-go" type="button" on:click=move |_| pick(ctx, BenchPick::Group(area, kind), Some(area))>
                    <b>{complete.then(|| view! { <Icon name="check"/> })}{item.name.clone()}</b>
                    <small>
                        <span class="st-area"><span class="st-dot" style=format!("--c: {tone}")></span>{area_name}</span>
                        <span>{line}</span>
                        {(!taken.is_empty()).then(|| view! { <span class="st-wb-else">{taken.join(", ")}</span> })}
                    </small>
                </button>
                <span class="st-wb-fit"></span>
                <span class="st-wb-lp"><b>{credits}</b>{t.common.credits_unit}</span>
                <button class="st-wb-plus go" type="button" on:click=move |_| pick(ctx, BenchPick::Group(area, kind), Some(area))>
                    <span>{if complete { b.change } else { b.choose }}</span><Icon name="chevron-right"/>
                </button>
            </li>
        }
        .into_any(),
    )
}

/// The Pflicht's row of the plan at `at`: its module, or a row without one, which can be put into
/// the semester for later („Vormerken").
fn plan_line_view(ctx: StudyCtx, parts: Parts, model: &Model, at: usize, sub: Option<String>, s: SemesterKey, t: &'static Texts) -> Option<AnyView> {
    let b = &t.study_bench;
    let line = model.view.lines.get(at)?;
    let group = group_of_line(model, at);
    match &line.module_id {
        Some(id) => {
            let row = parts.rows.with_untracked(|rows| rows.get(id).cloned());
            let week = parts.week.get_untracked();
            let sub = sub.or_else(|| match line.state {
                RowState::Failed(failed) => Some((b.failed_in)(&failed.short(t.locale))),
                _ => None,
            });
            Some(line_view(ctx, line_of(ctx, id, row.as_ref(), sub, week.as_ref(), s, group, None, t), s, t).into_any())
        }
        None => {
            let (caption, ord, name) = (line.caption.clone(), line.ord, line.name.clone());
            let credits = line.credits_text.clone().map(|text| folia_plans::plan::credits_in(&text, t.locale));
            let placed = ctx.plan.is_some_and(|plan| plan.with_untracked(|doc| doc.placeholders.iter().any(|p| p.semester == s && p.caption == caption && p.ord == ord)));
            let state = line.state;
            Some(
                view! {
                    <li class="st-wb-row" class:is-done=placed || !matches!(state, RowState::Open | RowState::Failed(_))>
                        <span class="st-wb-go">
                            <b>{name.clone()}</b>
                            <small>{sub.map(|sub| view! { <span>{sub}</span> })}{placed.then(|| view! { <span>{b.reserved_line}</span> })}</small>
                        </span>
                        <span class="st-wb-fit"></span>
                        <span class="st-wb-lp">{credits.map(|credits| view! { <b>{credits}</b>{t.common.credits_unit} })}</span>
                        {(!placed && matches!(state, RowState::Open | RowState::Failed(_))).then(|| {
                            let (caption, name) = (caption.clone(), name.clone());
                            view! {
                                <button class="st-wb-plus" type="button" title=b.reserve_hint on:click=move |_| reserve(ctx, s, caption.clone(), ord, name.clone(), t)>
                                    <Icon name="bookmark-plus"/><span>{b.reserve}</span>
                                </button>
                            }
                        })}
                    </li>
                }
                .into_any(),
            )
        }
    }
}

/// „Vormerken": a row of the plan put into `s` without a module (a placeholder), for later.
fn reserve(ctx: StudyCtx, s: SemesterKey, caption: String, ord: i64, name: String, t: &'static Texts) {
    let note = (t.study_bench.reserved_note)(&name, &s.label(t.locale));
    ctx.change_noted(Some(note), move |doc, input| {
        study::add(doc, input, s, &[Pick::Row { caption: caption.clone(), ord }], now_secs());
    }, || {});
}

/// „Laut Plan im 3. FS": what the plan puts into the semester's Fachsemester and what is open from
/// before it.
#[component]
fn PlanPane(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let more = RwSignal::new(false);
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        // Read so that a new answer redraws the rows.
        parts.rows.track();
        parts.week.track();
        let fs = fs_words(&model, t);
        let title = fs.as_deref().map_or_else(|| b.plan_title_plain.to_string(), |fs| (b.plan_title)(fs));
        let figure = parts.plan_figure.get().map(|(figure, _)| view! { <span><b>{figure}</b>" "{t.common.credits_unit}</span> }.into_any());
        let take = model.due.takeover(&model.view, s);
        let action = if !take.is_empty() {
            let count = take.len();
            view! {
                <div class="st-wb-act">
                    <button class="btn primary" type="button" id="st-wb-takeover" on:click=move |_| take_over(ctx, s, take.clone(), t)>
                        <Icon name="list-checks"/><span>{(b.take_over)(count)}</span>
                    </button>
                    <small>{b.take_over_hint}</small>
                </div>
            }
            .into_any()
        } else if !model.due.pflicht.is_empty() {
            fs.as_deref().map(|fs| view! { <div class="st-wb-act"><span class="st-wb-fit ok"><i></i><span>{(b.taken_over)(fs)}</span></span></div> }).into_any()
        } else {
            ().into_any()
        };
        let pflicht_credits: f64 = model.due.pflicht.iter().filter_map(|at| model.view.lines.get(*at)).filter_map(|line| line.credits).sum();
        let area_of = |at: usize| model.view.lines.get(at).and_then(|line| line.area).and_then(|area| ctx.with_ready(|ready| ready.area_name(Some(area), t)));
        let pflicht = model.due.pflicht.iter().filter_map(|at| plan_line_view(ctx, parts, &model, *at, area_of(*at).map(|area| (b.of_area)(&area)), s, t)).collect_view();
        let chosen = model.due.choices.iter().filter_map(|at| choice_view(ctx, &model, *at, t)).collect_view();
        // Stored placeholders of the semester that nothing fills: vorgemerkt.
        let reserved: Vec<(String, Option<(usize, GroupKind)>)> = ctx
            .plan
            .map(|plan| {
                plan.with_untracked(|doc| {
                    doc.placeholders
                        .iter()
                        .filter(|p| p.semester == s && doc.fillers(p.pid).is_empty())
                        .map(|p| {
                            let at = model.view.lines.iter().position(|line| line.caption == p.caption && line.ord == p.ord);
                            (p.name.clone(), at.and_then(|at| group_of_line(&model, at)))
                        })
                        .collect()
                })
            })
            .unwrap_or_default();
        let reserved_views = reserved
            .into_iter()
            .map(|(name, group)| {
                view! {
                    <li class="st-wb-row">
                        <span class="st-wb-go"><b>{name}</b><small><span>{b.reserved_line}</span></small></span>
                        <span class="st-wb-fit"></span>
                        <span class="st-wb-lp"></span>
                        {group.map(|(area, kind)| view! {
                            <button class="st-wb-plus go" type="button" on:click=move |_| pick(ctx, BenchPick::Group(area, kind), Some(area))>
                                <span>{b.choose}</span><Icon name="chevron-right"/>
                            </button>
                        })}
                    </li>
                }
            })
            .collect::<Vec<_>>();
        let plan_fs_of = |at: usize| model.view.lines.get(at).map(|line| (b.plan_fs)(&(t.study.fs)(line.first)));
        let mut earlier: Vec<AnyView> = model.due.earlier.iter().filter_map(|at| plan_line_view(ctx, parts, &model, *at, plan_fs_of(*at), s, t)).collect();
        earlier.extend(model.due.earlier_choices.iter().filter_map(|at| choice_view(ctx, &model, *at, t)));
        // What was not passed and is not planned again, where the plan names it nowhere else.
        let named: BTreeSet<String> = model.due.earlier.iter().chain(model.due.pflicht.iter()).filter_map(|at| model.view.lines.get(*at)?.module_id.clone()).collect();
        let retakes: Vec<(String, String)> = ctx
            .with_ready(|ready| {
                ready
                    .study
                    .retakes
                    .iter()
                    .filter_map(|retake| retake.item.module_id().map(|id| (id.to_string(), (b.failed_in)(&retake.failed_in.short(t.locale)))))
                    .filter(|(id, _)| !named.contains(id))
                    .collect()
            })
            .unwrap_or_default();
        let fues = fues_of(ctx);
        for (id, sub) in retakes {
            let row = parts.rows.with_untracked(|rows| rows.get(&id).cloned());
            let week = parts.week.get_untracked();
            let group = model.groups.group_of(&model.view, &id, None, "", &fues).and_then(|at| model.groups.groups.get(at)).map(|group| (group.area, group.kind));
            earlier.push(line_view(ctx, line_of(ctx, &id, row.as_ref(), Some(sub), week.as_ref(), s, group, None, t), s, t).into_any());
        }
        let next_count = model.due.next.len() + model.due.next_choices.len();
        let next_fs = model.fs.map(|fs| (t.study.fs)(fs + 1));
        let next = more.get().then(|| {
            let mut views: Vec<AnyView> = model.due.next.iter().filter_map(|at| plan_line_view(ctx, parts, &model, *at, area_of(*at).map(|area| (b.of_area)(&area)), s, t)).collect();
            views.extend(model.due.next_choices.iter().filter_map(|at| choice_view(ctx, &model, *at, t)));
            views
        });
        view! {
            {pane_head(title, None, None, figure, Some(b.plan_rule.to_string()))}
            {action}
            <ul class="st-wb-list">
                {(!model.due.pflicht.is_empty()).then(|| view! { <li class="st-wb-g">{b.g_pflicht}<small>{format!(" · {}", (t.study.credits)(&n(pflicht_credits, t)))}</small></li> })}
                {pflicht}
                {(!model.due.choices.is_empty()).then(|| view! { <li class="st-wb-g">{b.g_choices}</li> })}
                {chosen}
                {(!reserved_views.is_empty()).then(|| view! { <li class="st-wb-g">{b.g_reserved}</li> })}
                {reserved_views}
                {(!earlier.is_empty()).then(|| view! { <li class="st-wb-g">{b.g_earlier}</li> })}
                {earlier}
                {(next_count > 0).then(|| next_fs.clone().map(|fs| view! {
                    <li class="st-wb-g">
                        <button class="st-wb-fold" type="button" aria-expanded=move || if more.get() { "true" } else { "false" } on:click=move |_| more.update(|more| *more = !*more)>
                            {(b.g_next)(&fs)}<small>{format!(" · {next_count}")}</small><Icon name=if more.get_untracked() { "chevron-up" } else { "chevron-down" }/>
                        </button>
                    </li>
                }))}
                {next}
            </ul>
        }
        .into_any()
    }
}

/// „Plan übernehmen": the Pflicht of the Fachsemester into the semester, at once, with the note.
fn take_over(ctx: StudyCtx, s: SemesterKey, ids: Vec<String>, t: &'static Texts) {
    let note = (t.study_bench.took_over)(ids.len(), &s.label(t.locale));
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            let picks: Vec<Pick> = ids.iter().map(|id| Pick::Module { id: id.clone(), from_plan: true }).collect();
            study::add(doc, input, s, &picks, now_secs());
        },
        || {},
    );
}

/// The Pflicht of an area: every row of it, what stands nowhere yet first, by the Fachsemester the
/// plan puts it into.
#[component]
fn PflichtPane(ctx: StudyCtx, parts: Parts, at: usize) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        parts.rows.track();
        parts.week.track();
        let Some(group) = model.groups.groups.get(at).cloned() else { return ().into_any() };
        let (area_name, tone) = ctx.with_ready(|ready| (ready.area_name(Some(group.area), t), ready.tone(Some(group.area)))).unwrap_or_default();
        let many = model.groups.area(group.area).is_some_and(|area| area.groups.len() > 1);
        let title = if many { b.pflicht.to_string() } else { format!("{area_name} · {}", b.pflicht) };
        let (figure, _) = group_figure(&model, &group, t);
        let mut lines: Vec<usize> = group.lines.clone();
        lines.sort_by_key(|at| model.view.lines.get(*at).map_or((true, u8::MAX), |line| (!matches!(line.state, RowState::Open | RowState::Failed(_)), line.first)));
        let rows = lines
            .iter()
            .filter_map(|at| {
                let line = model.view.lines.get(*at)?;
                let fs = (t.study.fs)(line.first);
                let sub = match line.state {
                    RowState::Passed(_) => (b.passed_fs)(&fs),
                    _ => (b.plan_fs)(&fs),
                };
                plan_line_view(ctx, parts, &model, *at, Some(sub), s, t)
            })
            .collect_view();
        view! {
            {pane_head(title, many.then(|| area_name.clone()), Some(tone), Some(view! { <span>{figure}</span> }.into_any()), Some(b.rule_pflicht.to_string()))}
            <ul class="st-wb-list">{rows}</ul>
        }
        .into_any()
    }
}

/// A choice's modules, or the Merkliste's (`group` `None`): the switches over them, the modules
/// in groups by the week, the catalog under them. The switches, the selects and the search are
/// made once: what the list shows changes below them as the answers come, and an open select or
/// the search keeps its place and its focus.
#[component]
fn ListPane(ctx: StudyCtx, parts: Parts, group: Option<usize>) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let fues = Memo::new(move |_| fues_of(ctx));
    // The subgroup as the list shows it: its choice (its lists, whether it is the FÜS).
    let choice = Memo::new(move |_| {
        parts.model.with(|model| {
            let model = model.as_ref()?;
            let at = group.and_then(|at| model.groups.groups.get(at)).and_then(Group::choice)?;
            model.view.choices.get(at).cloned()
        })
    });
    let lists = Memo::new(move |_| choice.with(|choice| choice.as_ref().map(|choice| choice.lists.iter().map(|list| list.name.clone()).collect::<Vec<String>>()).unwrap_or_default()));
    let shelf = Memo::new(move |_| folia_stores::collections::Collections::expect().map(|collections| collections.with(Clone::clone)).unwrap_or_default());
    let season_word = move || parts.semester.get().map(|s| season(s, t)).unwrap_or_default();

    // The head and the rule.
    let head = move || {
        let model = parts.model.get()?;
        Some(match (group.and_then(|at| model.groups.groups.get(at)), choice.get()) {
            (Some(the_group), Some(choice)) => {
                let (area_name, tone) = ctx.with_ready(|ready| (ready.area_name(Some(the_group.area), t), ready.tone(Some(the_group.area)))).unwrap_or_default();
                let many = model.groups.area(the_group.area).is_some_and(|area| area.groups.len() > 1);
                let asked = folia_plans::plan::credits_in(&choice.credits_text, t.locale);
                let figure = view! { <span><b>{n(choice.taken_credits(), t)}</b>" "{(b.of_planned)("", &asked).trim_start().to_string()}</span> }.into_any();
                let mut rule = vec![(b.rule_choice)(&asked, &(b.plan_fs)(&(t.study.fs_span)(choice.first, choice.last)))];
                if choice.fues {
                    rule.push(b.rule_fues.to_string());
                }
                if choice.lists.len() > 1 {
                    rule.push((b.rule_lists)(choice.lists.len()));
                }
                pane_head(choice.name.clone(), many.then_some(area_name), Some(tone), Some(figure), Some(rule.join(" · "))).into_any()
            }
            _ => pane_head(b.marked.to_string(), None, None, None, Some(b.rule_marked.to_string())).into_any(),
        })
    };

    // The switches of a subgroup: „Fach" where it has several lists, the offer, the week, the search.
    let search = move || {
        view! {
            <label class="st-search-box small">
                <Icon name="search"/>
                <input type="search" id="st-wb-search" placeholder=b.search aria-label=b.search prop:value=move || parts.text.get() on:input=move |ev| parts.text.set(event_target_value(&ev))/>
            </label>
        }
    };
    let several = Memo::new(move |_| lists.with(|lists| lists.len() > 1));
    let fach = move || {
        several.get().then(|| {
            view! {
                <div class="st-inline">
                    <span class="label">{b.fach}</span>
                    <Select
                        id="st-wb-fach"
                        label=b.fach
                        items=Signal::derive(move || lists.with(|lists| std::iter::once(ComboItem::new("all", (b.all_lists)(lists.len()), "", 0)).chain(lists.iter().enumerate().map(|(at, name)| ComboItem::new(at.to_string(), name.clone(), "", 0))).collect::<Vec<_>>()))
                        selected=Signal::derive(move || parts.list.get().map_or_else(|| "all".to_string(), |at| at.to_string()))
                        on_select=Callback::new(move |value: String| parts.list.set(value.parse().ok()))
                        small=true
                    />
                </div>
            }
        })
    };
    // „Gemerkt": its collections, where there are any.
    let kept = Memo::new(move |_| {
        let marked: BTreeSet<String> = parts.marked.with(|ids| ids.iter().cloned().collect());
        shelf.with(|shelf| {
            let collections: Vec<(u32, String, usize)> = shelf.collections().iter().map(|kept| (kept.id, kept.name.clone(), shelf.count_in(kept.id, |id| marked.contains(id)))).collect();
            (marked.len(), collections)
        })
    });
    let any_kept = Memo::new(move |_| kept.with(|(_, collections)| !collections.is_empty()));
    let collections = move || {
        any_kept.get().then(|| {
            view! {
                <div class="st-inline">
                    <span class="label">{b.collection}</span>
                    <Select
                        id="st-wb-collection"
                        label=b.collection
                        items=Signal::derive(move || kept.with(|(all, collections)| std::iter::once(ComboItem::new("all", format!("{} · {all}", b.all_collections), "", 0)).chain(collections.iter().map(|(id, name, count)| ComboItem::new(id.to_string(), format!("{name} · {count}"), "", 0))).collect::<Vec<_>>()))
                        selected=Signal::derive(move || parts.collection.get().map_or_else(|| "all".to_string(), |id| id.to_string()))
                        on_select=Callback::new(move |value: String| parts.collection.set(value.parse().ok()))
                        small=true
                    />
                </div>
            }
        })
    };
    let controls = if group.is_some() {
        view! {
            <div class="st-wb-ctl" role="group">
                {fach}
                <button class="st-check" type="button" role="switch" aria-checked=move || if parts.offered.get() { "true" } else { "false" } on:click=move |_| parts.offered.update(|on| *on = !*on)>
                    <span class="st-box" class:with=move || parts.offered.get() aria-hidden="true"><Icon name="check"/></span>{move || (b.offered_in)(season_word())}
                </button>
                <button class="st-check" type="button" role="switch" aria-checked=move || if parts.fits.get() { "true" } else { "false" } on:click=move |_| parts.fits.update(|on| *on = !*on)>
                    <span class="st-box" class:with=move || parts.fits.get() aria-hidden="true"><Icon name="check"/></span>{b.fits_week}
                </button>
                {search}
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="st-wb-ctl" role="group">
                {collections}
                {search}
            </div>
        }
        .into_any()
    };

    // The list, as the answers and the switches have it.
    let body = move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        let the_group = group.and_then(|at| model.groups.groups.get(at)).cloned();
        let choice = choice.get();
        let fues = fues.get();
        let shelf = shelf.get();
        let collection = parts.collection.get().filter(|id| shelf.collection(*id).is_some());
        let candidates: Vec<(String, Option<usize>)> = match &the_group {
            Some(group) => model::candidates(&model.view, group, &fues),
            None => parts.marked.get().into_iter().filter(|id| collection.is_none_or(|chosen| shelf.holds(chosen, id))).map(|id| (id, None)).collect(),
        };
        let week = parts.week.get();
        let rows = parts.rows.get();
        let text = parts.text.get();
        let list = parts.list.get();
        // The switches are a subgroup's; „Gemerkt" lists every marked module, what the semester
        // does not offer last (it was kept for a reason).
        let (offered_only, fits_only) = match the_group {
            Some(_) => (parts.offered.get(), parts.fits.get()),
            None => (false, false),
        };
        let lists = lists.get();
        let program_id = ctx.plans.with(|plans| match plans {
            Plans::Found(source) => source.program.id.clone(),
            _ => String::new(),
        });
        let mut lines: Vec<Line> = candidates
            .iter()
            .filter(|(_, on)| list.is_none() || *on == list)
            .filter_map(|(id, on)| {
                let row = rows.get(id);
                let gone = row.is_some_and(|row| row.offer_status.known() == Some(OfferStatus::NotOffered));
                let (group_key, counts, sub) = match &the_group {
                    Some(group) => {
                        let counts = choice.as_ref().map(|choice| model::short_name(&choice.name));
                        let sub = on.and_then(|at| lists.get(at)).filter(|_| lists.len() > 1).cloned();
                        (Some((group.area, group.kind)), counts, sub)
                    }
                    None => {
                        // The Merkliste: the subgroup each counts for.
                        let at = model.groups.group_of(&model.view, id, None, &program_id, &fues);
                        let found = at.and_then(|at| model.groups.groups.get(at));
                        let name = found.map(|group| group_name(&model, group, true, t));
                        (found.map(|group| (group.area, group.kind)), name.clone().filter(|_| found.is_some_and(|group| group.choice().is_some())), name.map(|name| (b.counts_for)(&name)))
                    }
                };
                let mut line = line_of(ctx, id, row, sub, week.as_ref(), s, group_key, counts, t);
                // In „Gemerkt", where every one is marked, only its collections.
                if the_group.is_none() && line.marked.as_deref() == Some("") {
                    line.marked = None;
                }
                (!gone || line.here || line.passed).then_some(line)
            })
            .filter(|line| text.trim().is_empty() || folia_search::matches(&format!("{} {} {}", line.title, line.id, shelf.kept_text(&line.id).unwrap_or_default()), &text))
            .collect();
        let total = lines.len();
        lines.retain(|line| !offered_only || line.fit != Fit::Unoffered || line.here || line.passed);
        let checked = week.as_ref().is_some_and(|week| week.has_data);
        if fits_only && checked {
            lines.retain(|line| line.fit.fits() || line.here);
        }
        // In each group what is marked first (konzept.html of the Merkliste, „Von überall").
        lines.sort_by_cached_key(|line| (line.fit.group(), line.marked.is_none(), line.fit.clone(), line.title.to_lowercase()));
        let mut groups: Vec<(String, Vec<Line>)> = Vec::new();
        for line in lines {
            let name = match line.fit.group() {
                0 => b.group_fit.to_string(),
                1 => b.group_clash.to_string(),
                2 if checked => b.group_undated.to_string(),
                2 => (b.group_offered)(season(s, t)),
                _ => (b.group_unoffered)(season(s, t)),
            };
            match groups.last_mut() {
                Some((known, members)) if *known == name => members.push(line),
                _ => groups.push((name, vec![line])),
            }
        }
        let shown: usize = groups.iter().map(|(_, members)| members.len()).sum();
        let no_dates = (fits_only && !checked && week.is_some()).then(|| view! { <p class="st-wb-rule pad">{(b.no_dates)(&s.label(t.locale))}</p> });
        let list = if total == 0 && the_group.is_some() && text.trim().is_empty() {
            view! { <p class="st-wb-rule pad">{b.no_list}</p> }.into_any()
        } else if shown == 0 {
            let words = if total == 0 && !text.trim().is_empty() { b.no_hits } else { b.nothing };
            view! { <p class="st-wb-rule pad">{words}</p> }.into_any()
        } else {
            view! {
                <ul class="st-wb-list">
                    {groups.into_iter().map(|(name, members)| view! {
                        <li class="st-wb-g">{name}<small>{format!(" · {}", members.len())}</small></li>
                        {members.into_iter().map(|line| line_view(ctx, line, s, t)).collect_view()}
                    }).collect_view()}
                </ul>
            }
            .into_any()
        };
        view! { {no_dates}{list} }.into_any()
    };

    // Under the list: the catalog with the subgroup and the semester, „Vormerken".
    let foot = move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return None };
        let the_group = group.and_then(|at| model.groups.groups.get(at)).cloned()?;
        let choice = choice.get();
        let catalog = group_catalog(ctx, &model, &the_group, s, t);
        let reserve_row = choice.as_ref().and_then(|choice| {
            let line = choice.lines.first().and_then(|at| model.view.lines.get(*at))?;
            Some((line.caption.clone(), line.ord, choice.name.clone()))
        });
        let fues_choice = choice.as_ref().is_some_and(|choice| choice.fues);
        Some(view! {
            <footer class="st-wb-foot">
                {catalog.map(|href| view! {
                    <span>{if fues_choice { b.fues_filters } else { b.more_filters }}</span>
                    <a class="btn secondary" href=t.path(&href) rel="nofollow"><Icon name="search"/>{b.in_catalog}<Icon name="chevron-right"/></a>
                })}
                {reserve_row.map(|(caption, ord, name)| view! {
                    <button class="st-link" type="button" title=b.reserve_hint on:click=move |_| reserve(ctx, s, caption.clone(), ord, name.clone(), t)>
                        <Icon name="bookmark-plus"/>{b.reserve}
                    </button>
                })}
            </footer>
        })
    };

    view! {
        {head}
        {controls}
        {body}
        {foot}
    }
}

/// What the semester holds, and only that (owner, 2026-11-03: „Da sollte nur zu sehen sein, was
/// man alles belegt hat"): each module with the subgroup it counts for and ×, its credits against
/// the plan above them.
#[component]
fn SemesterColumn(
    ctx: StudyCtx,
    parts: Parts,
    /// In the phone's sheet, whose head names the semester: no head of its own, ‹ › beside the
    /// credits.
    #[prop(optional)]
    sheet: bool,
) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let (Some(s), Some(model)) = (parts.semester.get(), parts.model.get()) else { return ().into_any() };
        let Some((label, sub, now, credits, planned, items, visits)) = ctx
            .with_ready(|ready| {
                let semester = ready.study.semester(s)?;
                let sub = match semester.fs {
                    Some(fs) => (b.sem_sub)(&(t.study.fs_long)(fs)),
                    None => b.sem_sub_plain.to_string(),
                };
                let items: Vec<(study::Item, String, &'static str)> = semester
                    .items
                    .iter()
                    .filter(|item| match &item.subject {
                        Subject::Module { .. } => true,
                        Subject::Row { pid, .. } => pid.is_none() || !item.fillers.is_empty(),
                    })
                    .map(|item| (item.clone(), ready.area_name(item.area, t), ready.tone(item.area)))
                    .collect();
                let visits: Vec<(String, String)> = semester.visits.iter().map(|visit| (visit.id.clone(), visit.name.clone())).collect();
                Some((s.label(t.locale), sub, semester.when == When::Now, semester.credits, semester.planned, items, visits))
            })
            .flatten()
        else {
            return ().into_any();
        };
        let share = planned.filter(|plan| *plan > 0.0).map_or(0.0, |plan| (credits / plan * 100.0).min(100.0));
        let over = planned.filter(|plan| credits > plan + 0.5).map(|plan| credits - plan);
        let figure = match planned {
            Some(plan) => (b.of_planned)(&n(credits, t), &n(plan, t)),
            None => (b.credits)(&n(credits, t)),
        };
        let empty = items.is_empty() && visits.is_empty();
        let rows = items
            .into_iter()
            .map(|(item, area, tone)| {
                let tag = item.module_id().and_then(|id| model::counted_in(&model.groups, &model.view, id, s)).and_then(|at| model.groups.groups.get(at)).map(|group| match group.kind {
                    GroupKind::Pflicht => b.pflicht_tag.to_string(),
                    GroupKind::Choice(_) => (b.for_group)(&group_name(&model, group, true, t)),
                });
                let credits = item.counted().or(item.credits).map(|credits| n(credits, t));
                let name = item.name.clone();
                let href = item.module_id().map(|id| module_href(ctx, id, t));
                let remove = {
                    let item = item.clone();
                    move |_| {
                        let item = item.clone();
                        let note = (t.study.removed)(&item.name);
                        ctx.change_noted(Some(note), move |doc, _| study::remove(doc, s, &item), || {});
                    }
                };
                view! {
                    <li class="st-wb-sr">
                        {match href {
                            Some(href) => view! { <a class="st-wb-go" href=href><b>{name.clone()}</b><small><span class="st-area"><span class="st-dot" style=format!("--c: {tone}")></span>{area}</span>{tag.map(|tag| view! { <span class="st-wb-tag">{tag}</span> })}</small></a> }.into_any(),
                            None => view! { <span class="st-wb-go"><b>{name.clone()}</b><small><span class="st-area"><span class="st-dot" style=format!("--c: {tone}")></span>{area}</span></small></span> }.into_any(),
                        }}
                        <span class="st-wb-lp">{credits.map(|credits| view! { <b>{credits}</b>{t.common.credits_unit} })}</span>
                        <button class="icon-btn hit st-wb-rm" type="button" aria-label=(b.remove)(&name) title=(b.remove)(&name) on:click=remove><Icon name="x"/></button>
                    </li>
                }
            })
            .collect_view();
        let visit_rows = visits
            .into_iter()
            .map(|(id, name)| {
                let href = module_href(ctx, &id, t);
                let remove = {
                    let (id, name) = (id.clone(), name.clone());
                    move |_| {
                        let id = id.clone();
                        ctx.change_noted(Some((t.study.removed)(&name)), move |doc, _| doc.unvisit(s, &id, &[]), || {});
                    }
                };
                view! {
                    <li class="st-wb-sr visit">
                        <a class="st-wb-go" href=href><b>{name.clone()}</b><small><span class="st-wb-tag">{b.visit}</span></small></a>
                        <span class="st-wb-lp"></span>
                        <button class="icon-btn hit st-wb-rm" type="button" aria-label=(b.remove)(&name) title=(b.remove)(&name) on:click=remove><Icon name="x"/></button>
                    </li>
                }
            })
            .collect_view();
        let turns = sheet.then(|| {
            ctx.with_ready(|ready| {
                let study = &ready.study;
                let at = study.semesters.iter().position(|semester| semester.key == s)?;
                Some((at.checked_sub(1).and_then(|before| study.semesters.get(before)).map(|semester| semester.key), study.semesters.get(at + 1).map(|semester| semester.key)))
            })
            .flatten()
        });
        view! {
            <header class="st-wb-sh">
                {(!sheet).then(|| view! {
                    <h3>{label}{now.then(|| view! { <span class="st-now">{t.study.now}</span> })}</h3>
                    <small>{sub}</small>
                })}
                <div class="st-wb-meter" class:heavy=over.is_some()>
                    <span aria-hidden="true"><i style=format!("--p: {share:.0}%")></i></span>
                    <b>{figure}</b>
                    {turns.flatten().map(|(previous, next)| view! {
                        <span class="st-wb-turns">
                            <button class="icon-btn hit" type="button" aria-label=b.previous title=b.previous disabled=previous.is_none() on:click=move |_| if let Some(to) = previous { turn(ctx, to) }><Icon name="chevron-left"/></button>
                            <button class="icon-btn hit" type="button" aria-label=b.next title=b.next disabled=next.is_none() on:click=move |_| if let Some(to) = next { turn(ctx, to) }><Icon name="chevron-right"/></button>
                        </span>
                    })}
                </div>
                {over.map(|over| view! { <p class="st-wb-over">{(b.over_note)(&n(over, t))}</p> })}
            </header>
            {if empty {
                view! { <p class="st-wb-rule pad">{b.sem_empty}</p> }.into_any()
            } else {
                view! { <ul class="st-wb-sl">{rows}{visit_rows}</ul> }.into_any()
            }}
        }
        .into_any()
    }
}

/// The bar of the mode on a phone, in the tab bar's place: the semester and its credits (a tap
/// shows what it holds), „Fertig".
#[component]
fn PhoneBar(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let head = Memo::new(move |_| {
        let s = parts.semester.get()?;
        ctx.with_ready(|ready| {
            let semester = ready.study.semester(s)?;
            Some((s.short(t.locale), semester.credits, semester.planned))
        })
        .flatten()
    });
    view! {
        <div class="st-wb-pbar" role="toolbar" aria-label=b.mode>
            <button class="st-wb-pbar-w" type="button" aria-haspopup="dialog" on:click=move |_| ctx.open(Dialog::BenchSemester)>
                {move || head.get().map(|(label, credits, planned)| {
                    let share = planned.filter(|plan| *plan > 0.0).map_or(0.0, |plan| (credits / plan * 100.0).min(100.0));
                    let figure = match planned {
                        Some(plan) => (b.of_planned)(&n(credits, t), &n(plan, t)),
                        None => (b.credits)(&n(credits, t)),
                    };
                    view! {
                        <b>{format!("{label} · {figure}")}<Icon name="chevron-up"/></b>
                        <small>{b.tap_semester}</small>
                        <span class="st-wb-pbar-m" aria-hidden="true"><i style=format!("--p: {share:.0}%")></i></span>
                    }
                })}
            </button>
            <button class="btn primary" type="button" on:click=move |_| leave(ctx)>{b.done}</button>
        </div>
    }
}

/// The areas on a phone: a sheet with the same list, a tap folding an area out.
#[component]
pub(super) fn AreasSheet(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let parts = Parts::new(ctx);
    view! {
        <DialogHead ctx title=b.areas sub=b.areas_sub.to_string()/>
        <div class="st-dlg-body st-wb-areas sheet"><AreaList ctx parts phone=true/></div>
    }
}

/// The semester on a phone: what it holds, and only that, with ‹ › to another one beside its
/// credits.
#[component]
pub(super) fn SemesterSheet(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let parts = Parts::new(ctx);
    let head = Memo::new(move |_| {
        let s = parts.semester.get()?;
        ctx.with_ready(|ready| {
            let semester = ready.study.semester(s)?;
            let sub = match semester.fs {
                Some(fs) => (b.semester_sub)(&(b.sem_sub)(&(t.study.fs_long)(fs)), semester.when == When::Now),
                None => b.sem_sub_plain.to_string(),
            };
            Some((s.label(t.locale), sub))
        })
        .flatten()
    });
    move || {
        let (title, sub) = head.get().unwrap_or_default();
        view! {
            <DialogHead ctx title sub/>
            <div class="st-dlg-body st-wb-sem sheet"><SemesterColumn ctx parts sheet=true/></div>
        }
    }
}

/// What „Einplanen" elsewhere aims at while the workbench is open (`Planning::set_bench`): its
/// semester, and the subgroup picked where it is a choice — the placeholder of it standing in the
/// semester, or its first row — with its name.
pub(super) fn aim_of(ctx: StudyCtx) -> Option<BenchAim> {
    let bench = ctx.bench.get()?;
    let s = bench.semester;
    let model = model_of(ctx, s)?;
    let group = match pane_of(&model, bench.pick) {
        Pane::Group(at) => model.groups.groups.get(at).cloned(),
        _ => None,
    };
    let program_id = ctx.plans.with(|plans| match plans {
        Plans::Found(source) => source.program.id.clone(),
        _ => String::new(),
    });
    let (counts, fill, name): (Option<RowRef>, Option<u32>, Option<String>) = match group.as_ref().and_then(|group| group.choice().map(|choice| (group, choice))) {
        Some((group, choice)) => {
            let rows = model::rows_of(&model.view, group);
            let fill = ctx.plan.and_then(|plan| plan.with(|doc| study::open_placeholders(doc, &program_id, &rows).into_iter().find(|p| p.semester == s).map(|p| p.pid)));
            let name = model.view.choices.get(choice).map(|choice| choice.name.clone());
            (model::row_of(&model.view, choice, &program_id), fill, name)
        }
        None => (None, None, None),
    };
    let study_path = super::StudyUrl::default().path();
    Some(BenchAim { semester: s, counts, fill, name, back: study_path })
}

/// „Modul wählen" at a row of the plan without a module (a placeholder): the workbench for its
/// semester at the subgroup of its row (owner, 2026-11-03: „„Modul wählen“ an einem Platzhalter
/// öffnet die Werkbank in seiner Untergruppe").
pub(super) fn enter_row(ctx: StudyCtx, s: SemesterKey, caption: &str, ord: i64) {
    let pick = untrack(|| {
        let model = model_of(ctx, s)?;
        let at = model.view.lines.iter().position(|line| line.caption == caption && line.ord == ord)?;
        group_of_line(&model, at).map(|(area, kind)| BenchPick::Group(area, kind))
    });
    enter(ctx, s, pick.unwrap_or(BenchPick::Plan));
}

/// The workbench for semester `s` at the choice `choice` of the plan (`PlanView::choices`): „Wählen ›"
/// in the Regelstudienplan, „Planen ›" in the Wahlpflicht.
pub(super) fn enter_choice(ctx: StudyCtx, s: SemesterKey, choice: usize) {
    let pick = untrack(|| {
        let model = model_of(ctx, s)?;
        let at = model.groups.of_choice(choice)?;
        model.groups.groups.get(at).map(|group| BenchPick::Group(group.area, group.kind))
    });
    enter(ctx, s, pick.unwrap_or(BenchPick::Plan));
}

/// What „Zählt für ›" offers: each choice (its index of `PlanView::choices`, its short name, whether
/// the module counts for it now), and whether it counts where the tree places it.
pub(super) type CountsFor = (Vec<(usize, String, bool)>, bool);

/// „Zählt für ›" in the menu of module `id` of semester `s`: the choices it can count for — those
/// whose lists hold it, the one it counts for now, the FÜS where its list holds it — each with
/// whether it is the one, and whether it counts where the tree places it. Nothing for a module the
/// plan names in a row of its own: it counts there.
pub(super) fn counts_choices(ctx: StudyCtx, s: SemesterKey, id: &str, t: &Texts) -> Option<CountsFor> {
    untrack(|| {
        let model = model_of(ctx, s)?;
        if model.view.lines.iter().any(|line| line.module_id.as_deref() == Some(id)) {
            return None;
        }
        let fues = fues_of(ctx);
        let now = model::counted_in(&model.groups, &model.view, id, s).and_then(|at| model.groups.groups.get(at)).and_then(Group::choice);
        let stored = ctx.plan.is_some_and(|plan| plan.with(|doc| doc.counts_of(s, id).is_some() || doc.modules.iter().any(|m| m.semester == s && m.module_id == id && m.fills.is_some())));
        let entries: Vec<(usize, String, bool)> = model
            .view
            .choices
            .iter()
            .enumerate()
            .filter(|(at, choice)| Some(*at) == now || choice.list_of(id).is_some() || (choice.fues && fues.iter().any(|known| known == id)))
            .map(|(at, choice)| (at, model::short_name(&choice.name), Some(at) == now))
            .collect();
        let _ = t;
        (!entries.is_empty()).then_some((entries, !stored))
    })
}

/// „Zählt für": module `id` of semester `s` counts for choice `choice`, or where the tree places it
/// (`None`), with the note „Rückgängig" takes it back by.
pub(super) fn count_for(ctx: StudyCtx, s: SemesterKey, id: String, title: String, choice: Option<(usize, String)>, t: &'static Texts) {
    let b = &t.study_bench;
    let note = match &choice {
        Some((_, name)) => (b.counted_note)(&title, name),
        None => (b.counted_tree_note)(&title),
    };
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            let study = study::study(input);
            let view = choices::plan_view(input, &study);
            model::count_for(doc, &view, s, &id, choice.map(|(at, _)| at), input.program_id);
        },
        || {},
    );
}
