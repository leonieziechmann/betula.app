//! The workbench (folia/design/hinzufuegen/konzepte.html, third round; owner, 2026-11-03: „Passt
//! so, bau das jetzt in Folia"): „Module hinzufügen" makes planning a mode for one semester
//! (`Bench`). Its head stands over all it changes: what it plans into, what the semester holds
//! against the plan over the bar of it (`SemesterBar`: each item a part in its area's colour); on a
//! desktop with ‹ › (another semester without leaving the mode) and × to end it, as Esc does.
//!
//! The rows are the same on a desktop and on a phone (folia/design/hinzufuegen/handy.html, fifth
//! round; owner, 2026-11-04: „wir müssen an dem konzept arbeiten, wie module dargestellt werden";
//! 2026-11-05, of the desktop: „Mach das am Rechner bei der Werkbank auch so. Also mit ähnlichen
//! konzepten"): a module its name and one line, its credits as the button that adds them („+ 8 LP",
//! `module_line`), planned at once and counted for the subgroup it was planned from
//! (`bench::plan_for`), a click on it opening the module; a choice of the plan a dashed card with
//! its bar that goes into its list (`choice_card`). „Laut Plan im 3. FS" (`plan_rows`): „Plan
//! übernehmen" (at once, with „Rückgängig"), the Pflicht not yet planned, the choices, what is open
//! from before; „Aus dem 4. FS vorziehen" (`next_rows`): what the next Fachsemester puts in and the
//! semester can take (it offers it and has in place what it requires, `ahead`). A subgroup's
//! modules in groups by the student's week (`pages::fit`: they fit, they meet it, they have no fixed
//! dates, the semester does not offer them), the search and chips over them („Fach", „Im WiSe
//! angeboten", „Passt in meine Woche"), which change only what stands below them (CLAUDE.md, „What
//! picks stands above what it changes").
//!
//! On a desktop three columns under the head. On the left the areas of the plan as a tree, every
//! area unfolded with its subgroups, each with its figure and bar, one click into any of them (the
//! owner, 2026-11-04, of the phone: „als ausgeklappter baum … ohne drei weitere klicks direkt in
//! jeden bereich"); a choice that other choices of its area cover is none of them
//! (`shown_groups`). „Laut Plan im 3. FS" above them, „Gemerkt" and „Im Katalog suchen" under them.
//! In the middle what is picked; a click on a module opens it beside the page, over the column on
//! the right. That column is the semester, and only what is taken in it (owner, 2026-11-03: „Da
//! sollte nur zu sehen sein, was man alles belegt hat"), each module with the subgroup it counts
//! for and × to take it out.
//!
//! On a phone the page stays where it is (folia/design/hinzufuegen/handy.html, variant B; owner,
//! 2026-11-04: „Also ich finde Option B bedeutend besser, weil da leichter zu erkennen ist, was
//! jetzt geplant und was vorgeschlagen ist"): „Module hinzufügen" brings up a sheet over it
//! (`BenchSheet`), half up under the semester, which shows what is planned while the sheet shows
//! what is suggested; its root „Laut Plan im 3. FS", „Aus dem 4. FS vorziehen", then rows with ›
//! that go into a page of the sheet (the owner, the same day: „ein vertiefendes menu … mit > in das
//! quasi rein geht und das nicht ausklappt. Der modal soll offen bleiben. Oben einen Pfad haben und
//! auch einen back button"): „Alle Bereiche" as the tree, a subgroup's list, „Gemerkt" (`Page`, the
//! sheet's trail `StudyCtx::trail`), and „Im Katalog suchen". Over a page ‹ and the way to it, each
//! step of it a way back; Back and Esc take one step back. A module opens as its sheet over it.
//! Nothing behind it moves while it is open (the owner: „Das kann einfach gesperrt sein, so lange
//! der [offen] ist"): a tap there or a throw end the mode, Back once the root is in front.
//!
//! What needs many filters (a language, an exam, the credits) is the catalog's: „Im Katalog
//! suchen" opens it in the tab „Module" with the subgroup's modules and the semester as its target
//! (`folia_stores::studyplan::Planning::set_bench`; the owner, the same day, of a phone: „Mach das
//! beim Katalog mal bitte so, dass man in den Katalog Tab kommt und da die selection dann hat"),
//! and the bar of the target comes back: on a desktop over the list, on a phone in the tab bar's
//! place (`folia_stores::studyplan::BenchDock`).

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_model::labels::OfferStatus;
use folia_model::rows::{CatalogRow, Prerequisite};
use folia_pages::ask::{FitAsk, StudyModulesAsk};
use folia_pages::FitResult;
use folia_plans::bench::{self as model, Bench as Groups, Due, Group, GroupKind};
use folia_plans::choices::{self, PlanView, RowState};
use folia_plans::study::{self, Offer, Pick, Subject, When};
use folia_plans::studyplan::{PlanDoc, RowRef};
use folia_plans::variants;
use folia_routes::filter::{CatalogQuery, FitsFilter, ProgramScope, TurnusFilter};
use folia_routes::url::CatalogUrl;
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::combobox::{ComboItem, Select};
use folia_design::ui::Icon;
use folia_stores::bookmarks::Bookmarks;
use folia_stores::studyplan::BenchAim;

use super::picker::{lacks_required, offer_note};
use super::{module_href, n, now_secs, Plans, Ready, StudyCtx};
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
/// semester is the one in focus. On a phone the sheet opens at the page of what it picks, its root
/// behind it (a placeholder's „Modul wählen" at its choice's list).
pub(super) fn enter(ctx: StudyCtx, semester: SemesterKey, pick: BenchPick) {
    ctx.dialog.set(None);
    ctx.menu.set(None);
    ctx.selection.set(super::Selection::default());
    ctx.focus.set(Some(semester));
    let folded = match pick {
        BenchPick::Area(area) | BenchPick::Group(area, _) => Some(area),
        BenchPick::Plan | BenchPick::Marked => None,
    };
    let page = match pick {
        BenchPick::Plan => None,
        // An area (a cell of the Gesamtplan): its subgroup that is due.
        BenchPick::Area(area) => untrack(|| model_of(ctx, semester)).and_then(|model| model.groups.opens_with(&model.view, area, model.fs).and_then(|at| model.groups.groups.get(at)).map(|group| Page::Group(area, group.kind))),
        BenchPick::Group(area, kind) => Some(Page::Group(area, kind)),
        BenchPick::Marked => Some(Page::Marked),
    };
    // The trail before the mode: the sheet reads it as it comes.
    ctx.trail.set(page.map(|page| vec![Step { page, behind: 0 }]).unwrap_or_default());
    let pick = match (ctx.phone.get_untracked(), page) {
        (true, Some(page)) => page.pick(),
        _ => pick,
    };
    ctx.bench.set(Some(Bench { semester, pick, folded }));
    // On a desktop the workbench comes in the page's place; on a phone the page stays, the sheet
    // over it.
    if !ctx.phone.get_untracked() {
        super::dom::page_to_top();
    }
}

/// „Fertig": the mode ends, the semester planned in focus.
pub(super) fn leave(ctx: StudyCtx) {
    if let Some(bench) = ctx.bench.get_untracked() {
        ctx.focus.set(Some(bench.semester));
    }
    ctx.dialog.set(None);
    ctx.bench.set(None);
    ctx.trail.set(Vec::new());
    if !ctx.phone.get_untracked() {
        super::dom::page_to_top();
    }
}

/// A page of the phone's sheet under its root: what a row with › goes into (owner, 2026-11-04:
/// „Ich glaube es ist ne bessere idee, wenn man einfach ein vertiefendes menu hat").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    /// „Alle Bereiche": every area with its subgroups, unfolded (owner, 2026-11-04: „Alle bereiche
    /// muss eine eigene subpage werden, wo dann der modultree dargestellt ist").
    Areas,
    /// A subgroup's list: its area and what it is.
    Group(usize, GroupKind),
    /// „Gemerkt".
    Marked,
}

impl Page {
    /// What the workbench picks while the page is in front: the list it shows, else „Laut Plan".
    fn pick(self) -> BenchPick {
        match self {
            Page::Group(area, kind) => BenchPick::Group(area, kind),
            Page::Marked => BenchPick::Marked,
            Page::Areas => BenchPick::Plan,
        }
    }
}

/// A step of the sheet's trail: the page gone into, and how far the page behind it was scrolled,
/// which the way back finds again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    pub page: Page,
    pub behind: i32,
}

/// One step into `page` from the page in front: the sheet all up for it (`dom::raise_sheet`), the
/// page from its top.
fn go_in(ctx: StudyCtx, page: Page, ev: &leptos::ev::MouseEvent) {
    super::dom::raise_sheet(ev);
    let behind = super::dom::sheet_scrolled();
    ctx.trail.update(|trail| trail.push(Step { page, behind }));
    follow_trail(ctx);
    super::dom::sheet_scroll_to(0);
}

/// Into the list of subgroup `kind` of `area` from a row of it: on a phone a step into a page of the
/// sheet, on a desktop what the middle shows, from its top.
fn into_group(ctx: StudyCtx, area: usize, kind: GroupKind, ev: &leptos::ev::MouseEvent) {
    if ctx.phone.get_untracked() {
        go_in(ctx, Page::Group(area, kind), ev);
    } else {
        pick(ctx, BenchPick::Group(area, kind), Some(area));
        super::dom::page_to_top();
    }
}

/// One step back, to where the page behind was scrolled to: ‹, Back and Esc.
pub(super) fn back(ctx: StudyCtx) {
    let Some(step) = ctx.trail.try_update(Vec::pop).flatten() else { return };
    follow_trail(ctx);
    super::dom::sheet_scroll_to(step.behind);
}

/// Back to the page at `depth` of the trail, the root at 0: a step of the path.
fn back_to(ctx: StudyCtx, depth: usize) {
    let gone = ctx.trail.try_update(|trail| (depth < trail.len()).then(|| trail.drain(depth..).next()).flatten()).flatten();
    if let Some(step) = gone {
        follow_trail(ctx);
        super::dom::sheet_scroll_to(step.behind);
    }
}

/// What the workbench picks follows the page in front of the sheet.
fn follow_trail(ctx: StudyCtx) {
    let page = ctx.trail.with_untracked(|trail| trail.last().map(|step| step.page));
    let folded = match page {
        Some(Page::Group(area, _)) => Some(area),
        _ => None,
    };
    pick(ctx, page.map_or(BenchPick::Plan, Page::pick), folded);
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
/// list, the Merkliste; for „Laut Plan" the lists of the next Fachsemester's choices, which
/// „vorziehen" suggests where one of their modules can be taken (`ahead`).
fn asked_ids(model: &Model, pane: Pane, fues: &[String], marked: &[String]) -> Vec<String> {
    let lists_of = |group: &Group| model::candidates(&model.view, group, fues).into_iter().map(|(id, _)| id);
    let mut ids: Vec<String> = match pane {
        Pane::Group(at) => model.groups.groups.get(at).map(|group| lists_of(group).collect()).unwrap_or_default(),
        Pane::Marked => marked.to_vec(),
        Pane::Plan => model.due.next_choices.iter().filter_map(|choice| model.groups.of_choice(*choice)).filter_map(|at| model.groups.groups.get(at)).flat_map(lists_of).collect(),
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
                    <AreaList ctx parts/>
                </nav>
                <div class="panel st-wb-pane"><PaneView ctx parts/></div>
                <aside class="panel st-wb-sem"><SemesterColumn ctx parts/></aside>
            </div>
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
    /// What they ask for of other modules, with what the page's ask for.
    needs: Memo<Vec<Prerequisite>>,
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
        let needs = Memo::new(move |_| {
            let mut needs = ctx.needs.get();
            asked.with(|asked| {
                if let Some(Ok(modules)) = asked {
                    needs.extend(modules.prerequisites.iter().cloned());
                }
            });
            needs
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
            needs,
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

/// What the semester holds against the plan, for the bar of the workbench's head: its credits, what
/// the plan puts into it, and its items as the parts of the bar.
type Held = Option<(f64, Option<f64>, Vec<Piece>)>;

/// A part of the bar of the workbench's head: an item of the semester, as many credits wide as it
/// counts, in its area's colour; a placeholder hatched (`kind` "place").
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    key: String,
    credits: f64,
    kind: &'static str,
    tone: &'static str,
}

/// What the workbench's semester holds (`Held`), as the plan stands.
fn held_of(ctx: StudyCtx, parts: Parts) -> Memo<Held> {
    Memo::new(move |_| {
        let s = parts.semester.get()?;
        ctx.with_ready(|ready| {
            let semester = ready.study.semester(s)?;
            let pieces: Vec<Piece> = semester
                .items
                .iter()
                .filter_map(|item| {
                    let credits = item.counted().filter(|credits| *credits > 0.0)?;
                    let kind = match &item.subject {
                        Subject::Row { pid: Some(_), .. } => "place",
                        _ if item.passed => "passed",
                        _ => "planned",
                    };
                    Some(Piece { key: format!("{}:{credits}", item.key()), credits, kind, tone: ready.tone(item.area) })
                })
                .collect();
            Some((semester.credits, semester.planned, pieces))
        })
        .flatten()
    })
}

/// „22 von 30 LP" over the right end of the bar: what the semester holds against the plan, a
/// warning where it is more.
fn held_figure(held: Memo<Held>, t: &'static Texts) -> impl IntoView {
    let b = &t.study_bench;
    move || {
        held.get().map(|(credits, planned, _)| {
            let heavy = planned.is_some_and(|plan| credits > plan + 0.5);
            let rest = match planned {
                Some(plan) => (b.of_planned)("", &n(plan, t)).trim_start().to_string(),
                None => t.common.credits_unit.to_string(),
            };
            view! { <span class="st-wb-hfig" class:heavy=heavy><b>{n(credits, t)}</b>" "{rest}</span> }
        })
    }
}

/// The bar of what the semester holds (owner, 2026-11-04: „wir arbeiten hier in betula viel mit
/// progress bars, weil die das ganze system gut intuitiv veranschaulichen können"): each item a part
/// in its area's colour, as wide as the credits it counts, a placeholder hatched, what the plan still
/// puts in grey, more than the plan with a mark where the plan ends. Made once: an item planned comes
/// in as a part of its own (`For`), the rest gives way.
#[component]
fn SemesterBar(held: Memo<Held>) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let pieces = Memo::new(move |_| held.with(|held| held.as_ref().map(|(_, _, pieces)| pieces.clone()).unwrap_or_default()));
    let has_plan = Memo::new(move |_| held.with(|held| held.as_ref().is_some_and(|(_, planned, _)| planned.is_some())));
    let words = Memo::new(move |_| held.with(|held| held.as_ref().and_then(|(credits, planned, _)| planned.map(|plan| (b.of_planned)(&n(*credits, t), &n(plan, t))))).unwrap_or_default());
    let rest = Memo::new(move |_| held.with(|held| held.as_ref().and_then(|(credits, planned, _)| planned.map(|plan| (plan - credits).max(0.0))).unwrap_or_default()));
    let need = Memo::new(move |_| held.with(|held| held.as_ref().and_then(|(credits, planned, _)| planned.filter(|plan| *credits > plan + 0.5).map(|plan| plan / credits * 100.0))));
    move || {
        has_plan.get().then(|| {
            view! {
                <div class="st-wb-hbar" role="img" aria-label=move || words.get()>
                    <For each=move || pieces.get() key=|piece| piece.key.clone() let:piece>
                        <span class=piece.kind style=format!("flex-grow: {}; --c: {}", piece.credits, piece.tone)></span>
                    </For>
                    <span class="rest" style=move || format!("flex-grow: {}", rest.get())></span>
                    {move || need.get().map(|at| view! { <i class="st-wb-need" style=format!("left: {at:.2}%")></i> })}
                </div>
            }
        })
    }
}

/// The head of the mode on a desktop, as the phone's sheet has it (owner, 2026-11-05: „Mach das am
/// Rechner bei der Werkbank auch so. Also mit ähnlichen konzepten"): „Hinzufügen zu" the semester
/// with ‹ › (another one without leaving the mode) and its Fachsemester, the figure of what it holds
/// at the right end over the bar of it (`SemesterBar`), and × to end the mode, as Esc does (until
/// 1.75.0 „Fertig", of which the owner said on a phone: „der btw nur für verwirrung sorgt").
#[component]
fn ModeBar(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let held = held_of(ctx, parts);
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
            Some((s.label(t.locale), sub, previous, next))
        })
        .flatten()
    });
    view! {
        <div class="st-wb-bar" role="toolbar" aria-label=b.mode>
            <div class="st-wb-hrow">
                <span class="st-wb-bar-ic" aria-hidden="true"><Icon name="calendar-plus"/></span>
                <b class="st-wb-bar-t">{b.add_to}</b>
                {move || head.get().map(|(label, sub, previous, next)| view! {
                    <span class="st-wb-sem-pick">
                        <button class="icon-btn hit" type="button" aria-label=b.previous title=b.previous disabled=previous.is_none() on:click=move |_| if let Some(to) = previous { turn(ctx, to) }><Icon name="chevron-left"/></button>
                        <em>{label}</em>
                        <button class="icon-btn hit" type="button" aria-label=b.next title=b.next disabled=next.is_none() on:click=move |_| if let Some(to) = next { turn(ctx, to) }><Icon name="chevron-right"/></button>
                    </span>
                    <small class="st-wb-bar-sub">{sub}</small>
                })}
                {held_figure(held, t)}
                <button class="icon-btn hit st-wb-done" type="button" data-action="close-focus" aria-label=b.done_hint title=b.done_hint on:click=move |_| leave(ctx)>
                    <Icon name="x"/>
                </button>
            </div>
            <SemesterBar held/>
        </div>
    }
}

/// The areas on a desktop, as the phone's „Alle Bereiche" has them (owner, 2026-11-04: „die
/// container, dass die da als ausgeklappter baum sind und man dann damit ohne drei weitere klicks
/// direkt in jeden bereich kommt"): „Laut Plan im 3. FS" first, then every area unfolded — its head
/// with what it holds of what it asks (a click picks the subgroup that is due), each subgroup with
/// its figure and its bar, an area of one subgroup one row with its bar; a choice that other choices
/// of its area cover is none of them (`shown_groups`) —, „Gemerkt" and „Im Katalog suchen" after
/// them. Until 1.75.0 one area at a time folded out.
#[component]
fn AreaList(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let catalog = Memo::new(move |_| {
        let s = parts.semester.get()?;
        semester_catalog(ctx, s, t)
    });
    move || {
        parts.follow(ctx);
        let Some(model) = parts.model.get() else { return ().into_any() };
        let pane = parts.pane.get();
        let plan_label = model.fs.map_or_else(|| b.by_plan_plain.to_string(), |fs| (b.by_plan)(&(t.study.fs)(fs)));
        let rows = model
            .groups
            .areas
            .iter()
            .map(|area| {
                let index = area.area;
                let (name, tone, fill) = ctx.with_ready(|ready| (ready.area_name(Some(index), t), ready.tone(Some(index)), area_fill(ready, index))).unwrap_or_default();
                let shown = shown_groups(&model, index);
                if let [only] = shown.as_slice() {
                    let kind = model.groups.groups.get(*only).map_or(GroupKind::Pflicht, |group| group.kind);
                    let current = pane == Some(Pane::Group(*only));
                    return view! {
                        <button class="st-wb-a one" type="button" style=format!("--c: {tone}") aria-current=current.then_some("true") on:click=move |_| pick(ctx, BenchPick::Group(index, kind), Some(index))>
                            <span class="nm"><span class="st-dot"></span><span>{name}</span></span>
                            {fill_figure(fill, t)}
                            {fill_bar(fill)}
                        </button>
                    }
                    .into_any();
                }
                let opens = model.groups.opens_with(&model.view, index, model.fs).and_then(|at| model.groups.groups.get(at)).map(|group| group.kind);
                let subs = shown
                    .iter()
                    .filter_map(|at| model.groups.groups.get(*at).map(|group| (*at, group)))
                    .map(|(at, group)| {
                        let fill = group_fill(&model, group);
                        let short = group_name(&model, group, true, t);
                        let whole = group_name(&model, group, false, t);
                        let on = pane == Some(Pane::Group(at));
                        let kind = group.kind;
                        view! {
                            <button class="st-wb-s" type="button" aria-current=on.then_some("true") title=whole on:click=move |_| pick(ctx, BenchPick::Group(index, kind), Some(index))>
                                <span>{short}</span>
                                {fill_figure(fill, t)}
                                {fill_bar(fill)}
                            </button>
                        }
                    })
                    .collect_view();
                let label = name.clone();
                view! {
                    <button class="st-wb-a area" type="button" style=format!("--c: {tone}") on:click=move |_| if let Some(kind) = opens { pick(ctx, BenchPick::Group(index, kind), Some(index)) }>
                        <span class="nm"><span class="st-dot"></span><span>{name}</span></span>
                        {fill_figure(fill, t)}
                    </button>
                    <div class="st-wb-subs" role="group" aria-label=label style=format!("--c: {tone}")>{subs}</div>
                }
                .into_any()
            })
            .collect_view();
        let marked = parts.marked.with(Vec::len);
        view! {
            <button class="st-wb-a top" type="button" aria-current=(pane == Some(Pane::Plan)).then_some("true") on:click=move |_| pick(ctx, BenchPick::Plan, None)>
                <span class="nm"><Icon name="clipboard-list"/><span>{plan_label}</span></span>
                {move || parts.plan_figure.get().map(|(figure, ok)| view! { <small class:ok=ok>{figure}</small> })}
                <span class="chev"></span>
            </button>
            <hr class="st-wb-sep"/>
            {rows}
            <hr class="st-wb-sep"/>
            <button class="st-wb-a" type="button" aria-current=(pane == Some(Pane::Marked)).then_some("true") on:click=move |_| pick(ctx, BenchPick::Marked, None)>
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

/// The head of a pane: the area's dot and the way up to it where it has subgroups, the title, a
/// subgroup's figure and its bar under them, as the path of the phone's sheet has it; the rule
/// under it.
fn pane_head(title: String, crumb: Option<String>, tone: Option<&'static str>, fill: Option<Fill>, rule: Option<String>, t: &'static Texts) -> impl IntoView {
    view! {
        <header class="st-wb-ph" style=tone.map(|tone| format!("--c: {tone}"))>
            <div class="st-wb-pt">
                {tone.map(|_| view! { <span class="st-dot"></span> })}
                {crumb.map(|crumb| view! { <span class="st-wb-crumb">{crumb}" ›"</span> })}
                <h3>{title}</h3>
                {fill.map(|fill| fill_figure(fill, t))}
            </div>
            {fill.map(fill_bar)}
            {rule.map(|rule| view! { <p class="st-wb-rule">{rule}</p> })}
        </header>
    }
}

/// „Vormerken": a row of the plan put into `s` without a module (a placeholder), then its module
/// choice in front (`choose_for`; on a phone the sheet all up for it, as a step into a list is).
fn reserve(ctx: StudyCtx, s: SemesterKey, caption: String, ord: i64, name: String, ev: &leptos::ev::MouseEvent, t: &'static Texts) {
    let note = (t.study_bench.reserved_note)(&name, &s.label(t.locale));
    super::dom::raise_sheet(ev);
    let row = caption.clone();
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            study::add(doc, input, s, &[Pick::Row { caption: caption.clone(), ord }], now_secs());
        },
        move || choose_for(ctx, s, &row, ord),
    );
}

/// „Laut Regelstudienplan im 3. FS" on a desktop, as the root of the phone's sheet: „Plan
/// übernehmen", the Pflicht not yet planned (what is planned stands in the semester on the right),
/// the choices dashed with their bars, what is reserved and what is open from before; then „Aus
/// dem 4. FS vorziehen" with what the next Fachsemester could take now (until 1.75.0 folded under
/// its row, and the Pflicht planned stood in the list with ✓).
#[component]
fn PlanPane(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        // Read so that a new answer redraws the rows.
        parts.rows.track();
        parts.needs.track();
        parts.week.track();
        let fs = fs_words(&model, t);
        let title = fs.as_deref().map_or_else(|| b.plan_title_plain.to_string(), |fs| (b.plan_title)(fs));
        let (action, rows) = plan_rows(ctx, parts, &model, s, t);
        let next = next_rows(ctx, parts, &model, s, t);
        view! {
            {pane_head(title, None, None, None, Some(b.plan_rule.to_string()), t)}
            {action}
            {(!rows.is_empty()).then(|| view! { <ul class="st-wb-list">{rows}</ul> })}
            {next.map(|(title, rows)| view! {
                <h4 class="st-wb-sk" id="st-wb-next">{title}</h4>
                <ul class="st-wb-list">{rows}</ul>
            })}
        }
        .into_any()
    }
}

/// What „Laut Plan im 3. FS" lists, at the root of the phone's sheet and in the middle of a
/// desktop's workbench: „Plan übernehmen" (or that the Pflicht is in), and as rows the Pflicht not
/// yet planned, the choices, what is reserved where its choice is not among them, and what is open
/// from before, saying the Fachsemester it was planned for.
fn plan_rows(ctx: StudyCtx, parts: Parts, model: &Model, s: SemesterKey, t: &'static Texts) -> (AnyView, Vec<AnyView>) {
    let b = &t.study_bench;
    let fs = fs_words(model, t);
    let action = take_action(ctx, model, s, fs.as_deref(), t);
    let area_of = |at: usize| model.view.lines.get(at).and_then(|line| line.area).and_then(|area| ctx.with_ready(|ready| ready.area_name(Some(area), t)));
    let mut rows: Vec<AnyView> = model.due.pflicht.iter().filter(|at| still_open(ctx, model, **at, s)).filter_map(|at| plan_line(ctx, parts, model, *at, area_of(*at).map(|area| (b.of_area)(&area)), s, t)).collect();
    rows.extend(model.due.choices.iter().filter_map(|at| choice_card(ctx, model, *at, s, t)));
    // A choice listed here says itself that it is reserved.
    let listed: Vec<usize> = model.due.choices.iter().chain(&model.due.earlier_choices).copied().collect();
    rows.extend(
        reserved_of(ctx, model, s)
            .into_iter()
            .filter(|(_, group)| group.and_then(|(area, kind)| model.groups.find(area, kind)).and_then(|at| model.groups.groups.get(at)).and_then(Group::choice).is_none_or(|choice| !listed.contains(&choice)))
            .map(|(name, group)| reserved_card(ctx, name, group, t)),
    );
    let plan_fs_of = |at: usize| model.view.lines.get(at).map(|line| (b.plan_fs)(&(t.study.fs)(line.first)));
    rows.extend(model.due.earlier.iter().filter(|at| still_open(ctx, model, **at, s)).filter_map(|at| plan_line(ctx, parts, model, *at, plan_fs_of(*at), s, t)));
    rows.extend(retake_lines(ctx, parts, model, s, t));
    rows.extend(model.due.earlier_choices.iter().filter_map(|at| choice_card(ctx, model, *at, s, t)));
    (action, rows)
}

/// What the next Fachsemester puts in, is not in the semester yet and the semester can take („Aus
/// dem 4. FS vorziehen", `ahead`): its heading and its rows, where it has any. The callers track
/// `parts.rows` and `parts.needs`.
fn next_rows(ctx: StudyCtx, parts: Parts, model: &Model, s: SemesterKey, t: &'static Texts) -> Option<(String, Vec<AnyView>)> {
    let b = &t.study_bench;
    let fs = model.fs?;
    let (next, next_choices) = ahead(ctx, parts, model, s);
    let area_of = |at: usize| model.view.lines.get(at).and_then(|line| line.area).and_then(|area| ctx.with_ready(|ready| ready.area_name(Some(area), t)));
    let mut rows: Vec<AnyView> = next.iter().filter(|at| still_open(ctx, model, **at, s)).filter_map(|at| plan_line(ctx, parts, model, *at, area_of(*at).map(|area| (b.of_area)(&area)), s, t)).collect();
    rows.extend(next_choices.iter().filter_map(|at| choice_card(ctx, model, *at, s, t)));
    (!rows.is_empty()).then(|| ((b.g_next)(&(t.study.fs)(fs + 1)), rows))
}

/// „Plan übernehmen · 3 Pflichtmodule" where the Pflicht of the Fachsemester is not all in the
/// semester, else that it is („Die Pflicht des 3. FS steht im Semester.").
fn take_action(ctx: StudyCtx, model: &Model, s: SemesterKey, fs: Option<&str>, t: &'static Texts) -> AnyView {
    let b = &t.study_bench;
    let take = model.due.takeover(&model.view, s);
    if !take.is_empty() {
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
        fs.map(|fs| view! { <div class="st-wb-act"><span class="st-wb-fit ok"><i></i><span>{(b.taken_over)(fs)}</span></span></div> }).into_any()
    } else {
        ().into_any()
    }
}

/// The placeholders stored in `s` that nothing fills („vorgemerkt"): each name, and the subgroup of
/// its row.
fn reserved_of(ctx: StudyCtx, model: &Model, s: SemesterKey) -> Vec<(String, Option<(usize, GroupKind)>)> {
    ctx.plan
        .map(|plan| {
            plan.with_untracked(|doc| {
                doc.placeholders
                    .iter()
                    .filter(|p| p.semester == s && doc.fillers(p.pid).is_empty())
                    .map(|p| {
                        let at = model.view.lines.iter().position(|line| line.caption == p.caption && line.ord == p.ord);
                        (p.name.clone(), at.and_then(|at| group_of_line(model, at)))
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
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
/// plan puts it into. In the phone's sheet (`inline`) under the row that names it: its rule alone
/// over the rows.
#[component]
fn PflichtPane(
    ctx: StudyCtx,
    parts: Parts,
    at: usize,
    #[prop(optional)]
    inline: bool,
) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        parts.rows.track();
        parts.week.track();
        let Some(group) = model.groups.groups.get(at).cloned() else { return ().into_any() };
        let (area_name, tone) = ctx.with_ready(|ready| (ready.area_name(Some(group.area), t), ready.tone(Some(group.area)))).unwrap_or_default();
        let many = shown_groups(&model, group.area).len() > 1;
        let title = if many { b.pflicht.to_string() } else { format!("{area_name} · {}", b.pflicht) };
        let fill = group_fill(&model, &group);
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
                plan_line(ctx, parts, &model, *at, Some(sub), s, t)
            })
            .collect_view();
        let head = if inline {
            view! { <p class="st-wb-rule">{b.rule_pflicht}</p> }.into_any()
        } else {
            pane_head(title, many.then(|| area_name.clone()), Some(tone), Some(fill), Some(b.rule_pflicht.to_string()), t).into_any()
        };
        view! {
            {head}
            <ul class="st-wb-list">{rows}</ul>
        }
        .into_any()
    }
}

/// A choice's modules, or the Merkliste's (`group` `None`): the switches over them, the modules
/// in groups by the week, the catalog under them. The switches, the selects and the search are
/// made once: what the list shows changes below them as the answers come, and an open select or
/// the search keeps its place and its focus. In the phone's sheet (`inline`) under the row that
/// names it: its rule alone over the switches.
#[component]
fn ListPane(
    ctx: StudyCtx,
    parts: Parts,
    group: Option<usize>,
    #[prop(optional)]
    inline: bool,
) -> impl IntoView {
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
                let many = shown_groups(&model, the_group.area).len() > 1;
                let fill = group_fill(&model, the_group);
                // What it asks stands over the list with its bar: in the head, on a phone in the path.
                let mut rule = vec![(b.plan_fs)(&(t.study.fs_span)(choice.first, choice.last))];
                if choice.fues {
                    rule.push(b.rule_fues.to_string());
                }
                if choice.lists.len() > 1 {
                    rule.push((b.rule_lists)(choice.lists.len()));
                }
                if inline {
                    view! { <p class="st-wb-rule">{rule.join(" · ")}</p> }.into_any()
                } else {
                    pane_head(choice.name.clone(), many.then_some(area_name), Some(tone), Some(fill), Some(rule.join(" · ")), t).into_any()
                }
            }
            _ if inline => view! { <p class="st-wb-rule">{b.rule_marked}</p> }.into_any(),
            _ => pane_head(b.marked.to_string(), None, None, None, Some(b.rule_marked.to_string()), t).into_any(),
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
                <div class="st-wb-sel" class:on=move || parts.list.get().is_some()>
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
                <div class="st-wb-sel" class:on=move || parts.collection.get().is_some()>
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
    // On a phone (owner, 2026-11-04, of a choice's list there: „da ist ja wirklich alles kaputt. Das
    // muss nochmal neu designd werden"): the search over a row of chips. „Fach" and the collection
    // are chips whose list is the phone's own (a `<select>` over the chip), each switch a chip that
    // is on or off.
    let fach_chip = move || {
        several.get().then(|| {
            let label = move || {
                let at = parts.list.get();
                lists.with(|lists| at.and_then(|at| lists.get(at)).cloned().unwrap_or_else(|| (b.all_lists)(lists.len())))
            };
            view! {
                <label class="st-wb-chip pick" class:on=move || parts.list.get().is_some()>
                    <span>{label}</span>
                    <Icon name="chevron-down"/>
                    <select id="st-wb-fach" aria-label=b.fach on:change=move |ev| parts.list.set(event_target_value(&ev).parse().ok())>
                        <option value="all" prop:selected=move || parts.list.get().is_none()>{move || lists.with(|lists| (b.all_lists)(lists.len()))}</option>
                        {move || lists.get().into_iter().enumerate().map(|(at, name)| view! { <option value=at.to_string() prop:selected=move || parts.list.get() == Some(at)>{name}</option> }).collect_view()}
                    </select>
                </label>
            }
        })
    };
    let collection_chip = move || {
        any_kept.get().then(|| {
            let label = move || {
                let chosen = parts.collection.get();
                kept.with(|(all, collections)| match chosen.and_then(|id| collections.iter().find(|(known, _, _)| *known == id)) {
                    Some((_, name, count)) => format!("{name} · {count}"),
                    None => format!("{} · {all}", b.all_collections),
                })
            };
            view! {
                <label class="st-wb-chip pick" class:on=move || parts.collection.get().is_some()>
                    <span>{label}</span>
                    <Icon name="chevron-down"/>
                    <select id="st-wb-collection" aria-label=b.collection on:change=move |ev| parts.collection.set(event_target_value(&ev).parse().ok())>
                        <option value="all" prop:selected=move || parts.collection.get().is_none()>{move || kept.with(|(all, _)| format!("{} · {all}", b.all_collections))}</option>
                        {move || kept.get().1.into_iter().map(|(id, name, count)| view! { <option value=id.to_string() prop:selected=move || parts.collection.get() == Some(id)>{format!("{name} · {count}")}</option> }).collect_view()}
                    </select>
                </label>
            }
        })
    };
    let switch_chip = move |on: RwSignal<bool>, words: Signal<String>| {
        view! {
            <button class="st-wb-chip" type="button" role="switch" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| on.update(|on| *on = !*on)>
                <Icon name="check"/>{move || words.get()}
            </button>
        }
    };
    let controls = if inline && group.is_some() {
        view! {
            <div class="st-wb-ctl phone" role="group">
                {search}
                <div class="st-wb-chips">
                    {fach_chip}
                    {switch_chip(parts.offered, Signal::derive(move || (b.offered_in)(season_word())))}
                    {switch_chip(parts.fits, Signal::derive(move || b.fits_week.to_string()))}
                </div>
            </div>
        }
        .into_any()
    } else if inline {
        view! {
            <div class="st-wb-ctl phone" role="group">
                {search}
                {move || any_kept.get().then(|| view! { <div class="st-wb-chips">{collection_chip}</div> })}
            </div>
        }
        .into_any()
    } else if group.is_some() {
        view! {
            <div class="st-wb-ctl" role="group">
                {search}
                <div class="st-wb-chips">
                    {fach}
                    {switch_chip(parts.offered, Signal::derive(move || (b.offered_in)(season_word())))}
                    {switch_chip(parts.fits, Signal::derive(move || b.fits_week.to_string()))}
                </div>
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="st-wb-ctl" role="group">
                {search}
                {move || any_kept.get().then(|| view! { <div class="st-wb-chips">{collections}</div> })}
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
                        {members.into_iter().map(|line| module_line(ctx, line, s, true, t)).collect_view()}
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
        // Two ways at the list's end, as the root of the phone's sheet has them: the catalog with
        // every filter, and „Vormerken" for later (until 1.75.0 on a desktop a button and a link).
        Some(
            view! {
                <footer class="st-wb-foot st-wb-ways">
                    {catalog.map(|href| view! {
                        <a class="st-wb-a way" href=t.path(&href) rel="nofollow">
                            <span class="nm"><Icon name="search"/><span>{b.in_catalog}<small>{b.catalog_sub}</small></span></span>
                            <small></small>
                            <Icon name="chevron-right" class="chev"/>
                        </a>
                    })}
                    {reserve_row.map(|(caption, ord, name)| view! {
                        <button class="st-wb-a" type="button" on:click=move |ev| reserve(ctx, s, caption.clone(), ord, name.clone(), &ev, t)>
                            <span class="nm"><Icon name="bookmark-plus"/><span>{b.reserve}<small>{b.reserve_hint}</small></span></span>
                            <small></small>
                            <span class="chev"></span>
                        </button>
                    })}
                </footer>
            }
            .into_any(),
        )
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
fn SemesterColumn(ctx: StudyCtx, parts: Parts) -> impl IntoView {
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
        let over = planned.filter(|plan| credits > plan + 0.5).map(|plan| credits - plan);
        let empty = items.is_empty() && visits.is_empty();
        let rows = items
            .into_iter()
            .map(|(item, area, tone)| {
                let tag = item.module_id().and_then(|id| model::counted_in(&model.groups, &model.view, id, s)).and_then(|at| model.groups.groups.get(at)).map(|group| match group.kind {
                    GroupKind::Pflicht => b.pflicht_tag.to_string(),
                    GroupKind::Choice(_) => (b.for_group)(&group_name(&model, group, true, t)),
                });
                let exception = item.exception.then_some(t.study.exception);
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
                            Some(href) => view! { <a class="st-wb-go" href=href><b>{name.clone()}</b><small><span class="st-area"><span class="st-dot" style=format!("--c: {tone}")></span>{area}</span>{tag.map(|tag| view! { <span class="st-wb-tag">{tag}</span> })}{exception.map(|text| view! { <span class="flag st-exception">{text}</span> })}</small></a> }.into_any(),
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
        view! {
            <header class="st-wb-sh">
                <h3>{label}{now.then(|| view! { <span class="st-now">{t.study.now}</span> })}</h3>
                <small>{sub}</small>
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

/// The workbench on a phone: a sheet over the page while the mode is open (enhance.js „the sheets
/// of a phone": a page's own sheet, under a module's, its scrim before it).
#[component]
pub(super) fn BenchSheet(ctx: StudyCtx) -> impl IntoView {
    let open = Memo::new(move |_| ctx.phone.get() && ctx.bench.with(Option::is_some));
    move || open.get().then(|| view! { <Sheet ctx/> })
}

/// The sheet: its head (what it plans into, and the bar of what the semester holds against the
/// plan with its figure over the bar's right end: owner, 2026-11-04, „beim header vom modal statt
/// dem text und dem Fertig button, der btw nur für verwirrung sorgt lieber eine progressbar mit dem
/// text dann halt oben rechts dadrüber"), over a page gone into the way to it (`SheetPath`), then
/// the page in front — the root or a page of the trail. Opened at a page (a placeholder's „Modul
/// wählen"), it comes all up (`data-sheet-start`, enhance.js). It has no button to end it: a throw,
/// a tap above it or Back does, as every sheet of a phone.
#[component]
fn Sheet(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let parts = Parts::new(ctx);
    let title = Memo::new(move |_| parts.semester.get().map(|s| (b.sheet_title)(&s.label(t.locale))));
    let held = held_of(ctx, parts);
    let deep = untrack(|| ctx.trail.with(|trail| !trail.is_empty()));
    let depth = Memo::new(move |_| ctx.trail.with(Vec::len));
    let gone_in = Memo::new(move |_| depth.get() > 0);
    let front = Memo::new(move |_| ctx.trail.with(|trail| trail.last().map(|step| step.page)));
    // Another page: its own search.
    Effect::new(move |before: Option<usize>| {
        let now = depth.get();
        if before.is_some_and(|before| before != now) {
            parts.text.set(String::new());
        }
        now
    });
    // It takes the focus as it comes, for a screen reader. A region of the page, not a dialog: Esc
    // and Back take what lies over the page in their order (enhance.js keeps the keys of a dialog
    // to it), a module's sheet over it first, then a step back, then the mode (`nav::Kind::State`).
    let node = NodeRef::<leptos::html::Section>::new();
    Effect::new(move |_| {
        if let Some(sheet) = node.get() {
            let _ = sheet.focus();
        }
    });
    // The page in front, coming in from the side it was gone to: from the right going in, from the
    // left coming back (app.css, `.st-wb-page`).
    let shown = StoredValue::new(None::<usize>);
    let page = move || {
        let page = front.get();
        let now = ctx.trail.with_untracked(Vec::len);
        let way = match shown.get_value() {
            Some(was) if now > was => Some("in"),
            Some(was) if now < was => Some("back"),
            _ => None,
        };
        shown.set_value(Some(now));
        let content = match page {
            None => view! { <SheetRoot ctx parts/> }.into_any(),
            Some(Page::Areas) => view! { <AreasPage ctx parts/> }.into_any(),
            Some(Page::Group(..)) => view! { <GroupPage ctx parts/> }.into_any(),
            Some(Page::Marked) => view! { <ListPane ctx parts group=None inline=true/> }.into_any(),
        };
        view! { <div class="st-wb-page" data-way=way>{content}</div> }
    };
    view! {
        <div class="page-sheet-shade st-wb-shade" aria-hidden="true" on:click=move |_| leave(ctx)></div>
        <section class="page-sheet st-wb-sheet" data-sheet="half" data-sheet-start=deep.then_some("full") role="region" aria-labelledby="st-wb-title" tabindex="-1" node_ref=node>
            <header class="st-wb-sheet-h">
                <div class="st-wb-hrow">
                    <h2 id="st-wb-title">{move || title.get()}</h2>
                    {held_figure(held, t)}
                </div>
                <SemesterBar held/>
            </header>
            {move || gone_in.get().then(|| view! { <SheetPath ctx parts/> })}
            <div class="st-wb-sbody" data-sheet-body>{page}</div>
        </section>
    }
}

/// Over a page gone into the way to it (owner, 2026-11-04: „Oben einen Pfad haben und auch einen
/// back button, so dass man wieder zurück kommt"): ‹ one step back, the root and the pages behind
/// the one in front, each a way back to it, and under them the page's name with its area's dot, a
/// subgroup's figure and its bar. It stands above the page it changes, and does not scroll with it.
#[component]
fn SheetPath(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let pages = Memo::new(move |_| ctx.trail.with(|trail| trail.iter().map(|step| step.page).collect::<Vec<Page>>()));
    let behind = move || {
        parts.follow(ctx);
        let model = parts.model.get()?;
        let pages = pages.get();
        let (_, behind) = pages.split_last()?;
        Some(
            behind
                .iter()
                .enumerate()
                .map(|(at, page)| {
                    let name = page_name(ctx, &model, *page, true, t);
                    view! { <li><button type="button" on:click=move |_| back_to(ctx, at + 1)>{name}</button></li> }
                })
                .collect_view(),
        )
    };
    let title = move || {
        parts.follow(ctx);
        let model = parts.model.get()?;
        let front = pages.with(|pages| pages.last().copied())?;
        let name = page_name(ctx, &model, front, false, t);
        let (tone, fill) = page_fill(ctx, &model, front);
        Some(view! {
            <div class="st-wb-pth" style=tone.map(|tone| format!("--c: {tone}"))>
                <p class="st-wb-pt" id="st-wb-page">
                    {tone.map(|_| view! { <span class="st-dot"></span> })}
                    <b>{name}</b>
                    {fill.map(|fill| fill_figure(fill, t))}
                </p>
                {fill.map(fill_bar)}
            </div>
        })
    };
    view! {
        <nav class="st-wb-path" aria-label=b.path>
            <button class="st-wb-back" type="button" aria-label=b.back title=b.back on:click=move |_| back(ctx)><Icon name="chevron-left"/></button>
            <div class="st-wb-trail">
                <ol class="st-wb-crumbs">
                    <li><button type="button" on:click=move |_| back_to(ctx, 0)>{b.overview}</button></li>
                    {behind}
                </ol>
                {title}
            </div>
        </nav>
    }
}

/// A page's name: „Alle Bereiche"; a subgroup's, short in the path (the area's where it is the
/// area's only one and no choice); „Gemerkt".
fn page_name(ctx: StudyCtx, model: &Model, page: Page, short: bool, t: &Texts) -> String {
    let b = &t.study_bench;
    let area_name = |area: usize| ctx.with_ready(|ready| ready.area_name(Some(area), t)).unwrap_or_default();
    match page {
        Page::Areas => b.all_areas.to_string(),
        Page::Group(area, kind) => {
            let many = shown_groups(model, area).len() > 1;
            match model.groups.find(area, kind).and_then(|at| model.groups.groups.get(at)) {
                Some(group) if many || group.choice().is_some() => group_name(model, group, short, t),
                _ => area_name(area),
            }
        }
        Page::Marked => b.marked.to_string(),
    }
}

/// What the head of a page shows beside its name: a subgroup's area's colour, and how far it is.
fn page_fill(ctx: StudyCtx, model: &Model, page: Page) -> (Option<&'static str>, Option<Fill>) {
    match page {
        Page::Group(area, kind) => {
            let tone = ctx.with_ready(|ready| ready.tone(Some(area)));
            let fill = model.groups.find(area, kind).and_then(|at| model.groups.groups.get(at)).map(|group| group_fill(model, group));
            (tone, fill)
        }
        Page::Areas | Page::Marked => (None, None),
    }
}

/// How far a subgroup or an area is, for its bar: what is passed and planned of it, the least it
/// asks and the most that counts („10–24": a tick at the least).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Fill {
    passed: f64,
    planned: f64,
    least: f64,
    most: f64,
}

/// A subgroup's fill: the credits of its Pflicht's rows passed and planned, or the modules a choice
/// takes; a choice marked as done is full.
fn group_fill(model: &Model, group: &Group) -> Fill {
    match group.kind {
        GroupKind::Pflicht => {
            let rows = group.lines.iter().filter_map(|at| model.view.lines.get(*at));
            let asked: f64 = rows.clone().filter_map(|line| line.credits).sum();
            let passed: f64 = rows.clone().filter(|line| matches!(line.state, RowState::Passed(_))).filter_map(|line| line.credits).sum();
            let planned: f64 = rows.filter(|line| matches!(line.state, RowState::Planned(_) | RowState::Again { .. })).filter_map(|line| line.credits).sum();
            Fill { passed, planned, least: asked, most: asked }
        }
        GroupKind::Choice(at) => {
            let Some(choice) = model.view.choices.get(at) else { return Fill::default() };
            let mut passed = choice.passed_credits();
            if choice.done.is_some() {
                passed = passed.max(choice.least);
            }
            let planned = (choice.taken_credits() - choice.passed_credits()).max(0.0);
            Fill { passed, planned, least: choice.least, most: choice.most.max(choice.least) }
        }
    }
}

/// An area's fill, as the overview counts it.
fn area_fill(ready: &Ready, area: usize) -> Fill {
    ready.study.progress.get(area).map_or_else(Fill::default, |progress| Fill { passed: progress.passed, planned: progress.planned, least: progress.area.required, most: progress.area.required })
}

/// „6 / 12 LP", „0 / 10–24 LP": what a fill holds against what it asks.
fn fill_figure(fill: Fill, t: &Texts) -> impl IntoView {
    let asked = if fill.most > fill.least + 0.01 { format!("{}–{}", n(fill.least, t), n(fill.most, t)) } else { n(fill.least, t) };
    view! { <small class="st-wb-fig"><b>{n(fill.passed + fill.planned, t)}</b>{format!(" / {asked} {}", t.common.credits_unit)}</small> }
}

/// A fill's bar in its area's colour (`--c` around it): what is passed full, what is planned light,
/// and a tick at the least where it asks a range. Done looks as open does, only full (the owner, of
/// the Wahlpflicht, 2026-11-04: „Mach doch einfach die progress bar voll").
fn fill_bar(fill: Fill) -> impl IntoView {
    let whole = fill.most.max(fill.least).max(fill.passed + fill.planned).max(0.01);
    let at = move |value: f64| format!("{:.2}%", (value / whole * 100.0).clamp(0.0, 100.0));
    view! {
        <span class="st-wb-fill" aria-hidden="true">
            <i class="passed" style=format!("width: {}", at(fill.passed))></i>
            <i class="planned" style=format!("width: {}", at(fill.planned))></i>
            {(fill.most > fill.least + 0.01).then(|| view! { <b class="need" style=format!("left: {}", at(fill.least))></b> })}
        </span>
    }
}

/// The subgroups of an area the tree shows (`folia_plans::bench::Bench::shown`): a choice whose
/// modules other choices of the area list all of is none of them.
fn shown_groups(model: &Model, area: usize) -> Vec<usize> {
    model.groups.shown(&model.view, area)
}

/// Whether the plan's row at `at` still asks something of `s`: its module neither planned there
/// nor passed, or, a row without one, nothing put into `s` for it. What is planned stands in the
/// semester over the sheet, not in it.
fn still_open(ctx: StudyCtx, model: &Model, at: usize, s: SemesterKey) -> bool {
    let Some(line) = model.view.lines.get(at) else { return false };
    ctx.plan.is_none_or(|plan| {
        plan.with_untracked(|doc| match &line.module_id {
            Some(id) => !doc.is_planned(s, id) && doc.passed_in(id).is_none(),
            None => matches!(line.state, RowState::Open | RowState::Failed(_)) && !doc.placeholders.iter().any(|p| p.semester == s && p.caption == line.caption && p.ord == line.ord),
        })
    })
}

/// What „Aus dem 4. FS vorziehen" suggests for semester `s` (`Due::ahead`): the next
/// Fachsemester's rows and choices with a module `s` can take — not passed, offered in `s`, what it
/// requires („setzt voraus") in place by then (`picker::lacks_required`, what the semester warns
/// of). A module whose catalog row has not come yet counts as offered, as a row of the plan
/// without one does. The callers track `parts.rows` and `parts.needs`.
fn ahead(ctx: StudyCtx, parts: Parts, model: &Model, s: SemesterKey) -> (Vec<usize>, Vec<usize>) {
    let now = ctx.now.get_untracked();
    let fues = fues_of(ctx);
    let judge = |doc: &PlanDoc| {
        parts.rows.with_untracked(|rows| {
            parts.needs.with_untracked(|needs| {
                let takes = |id: &str| {
                    let row = rows.get(id);
                    let own = row.map_or(id, |row| row.title.as_str());
                    doc.passed_in(id).is_none() && row.map_or_else(Offer::default, Offer::of).offered(s) && !lacks_required(doc, needs, id, own, s, now)
                };
                model.due.ahead(&model.view, &model.groups, s, &fues, takes)
            })
        })
    };
    match ctx.plan {
        Some(plan) => plan.with_untracked(judge),
        None => judge(&PlanDoc::default()),
    }
}

/// The root of the sheet (owner, 2026-11-04: „Der aufbau soll also jetzt sein: Module aus dem
/// semester, Module vorziehen, Page für alle Bereiche, Page gemerkt, Im katalog suchen"): what the
/// plan puts into the semester, what the next Fachsemester could take now, and the ways to all the
/// areas, to „Gemerkt" and to the catalog.
#[component]
fn SheetRoot(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    view! {
        <SheetPlan ctx parts/>
        <SheetNext ctx parts/>
        <SheetWays ctx parts/>
    }
}

/// „Laut Plan im 3. FS" at the root: „Plan übernehmen" and the Pflicht not yet planned, each
/// planned at once; the choices, what is reserved where its choice is not among them, and what is
/// open from before, each choice going into its list.
#[component]
fn SheetPlan(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        // Read so that a new answer redraws the rows.
        parts.rows.track();
        parts.week.track();
        let fs = fs_words(&model, t);
        let title = fs.as_deref().map_or_else(|| b.by_plan_plain.to_string(), |fs| (b.by_plan)(fs));
        let (action, rows) = plan_rows(ctx, parts, &model, s, t);
        view! {
            <section class="st-wb-ss">
                <h3 class="st-wb-sk">{title}</h3>
                {action}
                {(!rows.is_empty()).then(|| view! { <ul class="st-wb-list">{rows}</ul> })}
            </section>
        }
        .into_any()
    }
}

/// „Aus dem 4. FS vorziehen" at the root, under what the semester's own Fachsemester asks: what the
/// plan puts into the next one, is not in the semester yet and the semester can take (`ahead`),
/// each planned at once, its choices going into their lists.
#[component]
fn SheetNext(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    move || {
        parts.follow(ctx);
        let (Some(model), Some(s)) = (parts.model.get(), parts.semester.get()) else { return ().into_any() };
        parts.rows.track();
        parts.needs.track();
        parts.week.track();
        let Some((title, rows)) = next_rows(ctx, parts, &model, s, t) else { return ().into_any() };
        view! {
            <section class="st-wb-ss" id="st-wb-next">
                <h3 class="st-wb-sk">{title}</h3>
                <ul class="st-wb-list">{rows}</ul>
            </section>
        }
        .into_any()
    }
}

/// The ways at the root's end: all the areas and „Gemerkt", each a page of the sheet, and the
/// catalog with the semester as its target.
#[component]
fn SheetWays(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let catalog = Memo::new(move |_| {
        let s = parts.semester.get()?;
        semester_catalog(ctx, s, t)
    });
    view! {
        <nav class="st-wb-ways" aria-label=b.areas>
            <button class="st-wb-a" type="button" id="st-wb-areas" on:click=move |ev| go_in(ctx, Page::Areas, &ev)>
                <span class="nm"><Icon name="list-tree"/><span>{b.all_areas}</span></span>
                <small></small>
                <Icon name="chevron-right" class="chev"/>
            </button>
            <button class="st-wb-a" type="button" id="st-wb-marked" on:click=move |ev| go_in(ctx, Page::Marked, &ev)>
                <span class="nm"><Icon name="bookmark"/><span>{b.marked}</span></span>
                <small>{move || { let count = parts.marked.with(Vec::len); (count > 0).then(|| count.to_string()) }}</small>
                <Icon name="chevron-right" class="chev"/>
            </button>
            {move || catalog.get().map(|href| view! {
                <a class="st-wb-a way" href=t.path(&href) rel="nofollow">
                    <span class="nm"><Icon name="search"/><span>{b.in_catalog}</span></span>
                    <small></small>
                    <Icon name="chevron-right" class="chev"/>
                </a>
            })}
        </nav>
    }
}

/// „Alle Bereiche" gone into: every area, unfolded, with its subgroups as the overview counts them
/// (owner, 2026-11-04: „die container, dass die da als ausgeklappter baum sind und man dann damit
/// ohne drei weitere klicks direkt in jeden bereich kommt"). An area's head says what it holds of
/// what it asks; each subgroup its own figure and bar, and goes into its list. An area of one
/// subgroup is one row that goes into it.
#[component]
fn AreasPage(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    move || {
        parts.follow(ctx);
        let model = parts.model.get()?;
        let areas = model
            .groups
            .areas
            .iter()
            .map(|area| {
                let index = area.area;
                let (name, tone, fill) = ctx.with_ready(|ready| (ready.area_name(Some(index), t), ready.tone(Some(index)), area_fill(ready, index))).unwrap_or_default();
                let shown = shown_groups(&model, index);
                match shown.as_slice() {
                    [only] => {
                        let kind = model.groups.groups.get(*only).map_or(GroupKind::Pflicht, |group| group.kind);
                        view! {
                            <section class="st-wb-ta" style=format!("--c: {tone}")>
                                <button class="st-wb-th one" type="button" on:click=move |ev| go_in(ctx, Page::Group(index, kind), &ev)>
                                    <span class="st-wb-tn"><span class="st-dot"></span><span>{name}</span>{fill_figure(fill, t)}</span>
                                    {fill_bar(fill)}
                                    <Icon name="chevron-right" class="chev"/>
                                </button>
                            </section>
                        }
                        .into_any()
                    }
                    _ => {
                        let subs = shown
                            .iter()
                            .filter_map(|at| model.groups.groups.get(*at))
                            .map(|group| {
                                let fill = group_fill(&model, group);
                                let short = group_name(&model, group, true, t);
                                let whole = group_name(&model, group, false, t);
                                let kind = group.kind;
                                view! {
                                    <li>
                                        <button class="st-wb-ts" type="button" title=whole on:click=move |ev| go_in(ctx, Page::Group(index, kind), &ev)>
                                            <span class="st-wb-tn"><span>{short}</span>{fill_figure(fill, t)}</span>
                                            {fill_bar(fill)}
                                            <Icon name="chevron-right" class="chev"/>
                                        </button>
                                    </li>
                                }
                            })
                            .collect_view();
                        let label = name.clone();
                        view! {
                            <section class="st-wb-ta" style=format!("--c: {tone}") aria-label=label>
                                <h4 class="st-wb-th"><span class="st-wb-tn"><span class="st-dot"></span><span>{name}</span>{fill_figure(fill, t)}</span></h4>
                                <ul class="st-wb-tree">{subs}</ul>
                            </section>
                        }
                        .into_any()
                    }
                }
            })
            .collect_view();
        Some(view! { <div class="st-wb-areas-all" aria-label=b.all_areas>{areas}</div> })
    }
}

/// A subgroup gone into: the Pflicht's rows, or a choice's modules with their switches; what the
/// workbench picks is the page's (`Page::pick`).
#[component]
fn GroupPage(ctx: StudyCtx, parts: Parts) -> impl IntoView {
    move || match parts.pane.get() {
        Some(Pane::Group(at)) => view! { <GroupList ctx parts at/> }.into_any(),
        _ => ().into_any(),
    }
}

/// A subgroup's list in the sheet: the Pflicht's rows, or a choice's modules.
#[component]
fn GroupList(ctx: StudyCtx, parts: Parts, at: usize) -> impl IntoView {
    let pflicht = parts.model.with_untracked(|model| model.as_ref().and_then(|model| model.groups.groups.get(at)).is_some_and(|group| group.kind == GroupKind::Pflicht));
    if pflicht {
        view! { <PflichtPane ctx parts at inline=true/> }.into_any()
    } else {
        view! { <ListPane ctx parts group=Some(at) inline=true/> }.into_any()
    }
}

/// A module in the phone's sheet (owner, 2026-11-04: „wir müssen an dem konzept arbeiten, wie module
/// dargestellt werden. Hier ist auch wieder das problem visuell unordentlich und unbalanciert"): its
/// name and one line of what it is — its number, its „Fach" or area, what it means for the week
/// where its group does not say it (`grouped`), where else it is planned, that it is marked —, and
/// on the right, in the middle of the row, its credits as the button that adds them („+ 8 LP"), ✓
/// once it is in the semester (a tap takes it out again). A tap elsewhere on the row opens the
/// module's sheet.
fn module_line(ctx: StudyCtx, line: Line, s: SemesterKey, grouped: bool, t: &'static Texts) -> impl IntoView {
    let b = &t.study_bench;
    let href = module_href(ctx, &line.id, t);
    let open = Memo::new({
        let id = line.id.clone();
        move |_| ctx.url.with(|url| url.open.as_deref() == Some(id.as_str()))
    });
    let mark = (!line.here && !line.passed).then(|| line.fit.mark(s, t)).flatten().filter(|_| !grouped || matches!(line.fit, Fit::Partly(_)));
    let credits = line.credits.map(|credits| n(credits, t));
    let figure = move || credits.clone().map(|credits| view! { <b>{credits}</b><small>{t.common.credits_unit}</small> });
    let elsewhere = line.elsewhere.map(|at| (b.planned_in)(&at.short(t.locale)));
    let act = if line.passed {
        view! { <span class="st-wb-plus quiet"><Icon name="check"/><span>{b.passed}</span></span> }.into_any()
    } else if line.here {
        let (id, title) = (line.id.clone(), line.title.clone());
        let aria = (b.unplan_aria)(&line.title, &s.label(t.locale));
        view! {
            <button class="st-wb-plus done" type="button" aria-label=aria.clone() title=aria on:click=move |_| unplan_module(ctx, s, id.clone(), title.clone(), t)>
                <Icon name="check"/>{figure()}
            </button>
        }
        .into_any()
    } else {
        let (id, title, group, counts) = (line.id.clone(), line.title.clone(), line.group, line.counts.clone());
        let aria = (b.plan_aria)(&line.title, &s.label(t.locale));
        view! {
            <button class="st-wb-plus" type="button" aria-label=aria.clone() title=aria on:click=move |_| plan_module(ctx, s, id.clone(), title.clone(), group, counts.clone(), t)>
                <Icon name="plus"/>{figure()}
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
                    {line.sub.clone().map(|sub| view! { <span>{sub}</span> })}
                    {mark.map(|(class, text)| view! { <span class=format!("st-wb-fit {class}")><i></i><span>{text}</span></span> })}
                    {elsewhere.map(|text| view! { <span class="st-wb-else">{text}</span> })}
                    {line.marked.clone().map(|names| {
                        let words = if names.is_empty() { b.marked_mark.to_string() } else { format!("{}: ", b.marked_mark) };
                        view! { <span class="st-marked"><Icon name="bookmark"/><span class:visually-hidden=!names.is_empty()>{words}</span>{names}</span> }
                    })}
                </small>
            </a>
            {act}
        </li>
    }
}

/// A row of the plan in the sheet: its module (`module_line`), or a row without one, a place as a
/// placeholder is in the semester, which „Vormerken" puts into the semester for later.
fn plan_line(ctx: StudyCtx, parts: Parts, model: &Model, at: usize, sub: Option<String>, s: SemesterKey, t: &'static Texts) -> Option<AnyView> {
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
            Some(module_line(ctx, line_of(ctx, id, row.as_ref(), sub, week.as_ref(), s, group, None, t), s, false, t).into_any())
        }
        None => {
            let (caption, ord, name) = (line.caption.clone(), line.ord, line.name.clone());
            let credits = line.credits_text.clone().map(|text| folia_plans::plan::credits_in(&text, t.locale));
            let placed = ctx.plan.is_some_and(|plan| plan.with_untracked(|doc| doc.placeholders.iter().any(|p| p.semester == s && p.caption == caption && p.ord == ord)));
            let open = matches!(line.state, RowState::Open | RowState::Failed(_));
            let tone = ctx.with_ready(|ready| ready.tone(line.area)).unwrap_or_default();
            Some(
                view! {
                    <li class="st-wb-row place" class:is-done=placed || !open style=format!("--c: {tone}")>
                        <span class="st-wb-go">
                            <b>{name.clone()}</b>
                            <small>
                                {sub.map(|sub| view! { <span>{sub}</span> })}
                                {credits.map(|credits| view! { <span>{format!("{credits} {}", t.common.credits_unit)}</span> })}
                                {placed.then(|| view! { <span class="st-wb-tag">{b.reserved_mark}</span> })}
                            </small>
                        </span>
                        {(!placed && open).then(|| {
                            let (caption, name) = (caption.clone(), name.clone());
                            view! {
                                <button class="st-wb-plus reserve" type="button" title=b.reserve_hint on:click=move |ev| reserve(ctx, s, caption.clone(), ord, name.clone(), &ev, t)>
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

/// What was not passed and is not planned again, where the plan names it nowhere else: its module.
fn retake_lines(ctx: StudyCtx, parts: Parts, model: &Model, s: SemesterKey, t: &'static Texts) -> Vec<AnyView> {
    let b = &t.study_bench;
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
    retakes
        .into_iter()
        .map(|(id, sub)| {
            let row = parts.rows.with_untracked(|rows| rows.get(&id).cloned());
            let week = parts.week.get_untracked();
            let group = model.groups.group_of(&model.view, &id, None, "", &fues).and_then(|at| model.groups.groups.get(at)).map(|group| (group.area, group.kind));
            module_line(ctx, line_of(ctx, &id, row.as_ref(), Some(sub), week.as_ref(), s, group, None, t), s, false, t).into_any()
        })
        .collect()
}

/// A choice of the plan in the sheet: a place to fill, dashed as a placeholder is in the semester
/// (owner, 2026-11-04: „Das gleiche beim wp placeholder"): its name and what it holds of what it
/// asks over its bar, its area and its span under it, that it is reserved in the semester; › at its
/// end. The row goes into its list.
fn choice_card(ctx: StudyCtx, model: &Model, choice: usize, s: SemesterKey, t: &'static Texts) -> Option<AnyView> {
    let b = &t.study_bench;
    let at = model.groups.of_choice(choice)?;
    let group = model.groups.groups.get(at)?;
    let item = model.view.choices.get(choice)?;
    let (area, kind) = (group.area, group.kind);
    let (area_name, tone) = ctx.with_ready(|ready| (ready.area_name(Some(area), t), ready.tone(Some(area))))?;
    let fill = group_fill(model, group);
    let span = (b.plan_fs)(&(t.study.fs_span)(item.first, item.last));
    let reserved = reserved_of(ctx, model, s).iter().any(|(_, of)| *of == Some((area, kind)));
    let name = model::short_name(&item.name);
    Some(
        view! {
            <li class="st-wb-row choice" class:is-done=item.complete() style=format!("--c: {tone}")>
                <button class="st-wb-go" type="button" title=item.name.clone() on:click=move |ev| into_group(ctx, area, kind, &ev)>
                    <span class="st-wb-ct"><b>{name}</b>{fill_figure(fill, t)}</span>
                    {fill_bar(fill)}
                    <small>
                        <span class="st-area"><span class="st-dot"></span>{area_name}</span>
                        <span>{span}</span>
                        {reserved.then(|| view! { <span class="st-wb-tag">{b.reserved_mark}</span> })}
                    </small>
                </button>
                <Icon name="chevron-right" class="chev"/>
            </li>
        }
        .into_any(),
    )
}

/// A placeholder reserved in the semester whose choice the root does not list: dashed as a choice
/// is, „vorgemerkt", and › into the list of its row's subgroup.
fn reserved_card(ctx: StudyCtx, name: String, group: Option<(usize, GroupKind)>, t: &'static Texts) -> AnyView {
    let b = &t.study_bench;
    let tone = group.and_then(|(area, _)| ctx.with_ready(|ready| ready.tone(Some(area)))).unwrap_or_default();
    match group {
        Some((area, kind)) => view! {
            <li class="st-wb-row choice" style=format!("--c: {tone}")>
                <button class="st-wb-go" type="button" on:click=move |ev| into_group(ctx, area, kind, &ev)>
                    <span class="st-wb-ct"><b>{name}</b></span>
                    <small><span class="st-wb-tag">{b.reserved_line}</span></small>
                </button>
                <Icon name="chevron-right" class="chev"/>
            </li>
        }
        .into_any(),
        None => view! {
            <li class="st-wb-row choice">
                <span class="st-wb-go">
                    <span class="st-wb-ct"><b>{name}</b></span>
                    <small><span class="st-wb-tag">{b.reserved_line}</span></small>
                </span>
            </li>
        }
        .into_any(),
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
    // The way back is the page as it stands, without a module beside it; on a phone the semesters'
    // row (`plan=1`), which stands at the semester in focus — the workbench's —, not the overview
    // of `/study`: the tab „Module" left for „Mein Studium" is a change of tabs, which keeps no
    // semester of the row's entry (`folia_shell::nav::tab`).
    let phone = ctx.phone.get();
    let study_path = ctx.url.with(|url| super::StudyUrl { open: None, full: false, ..url.clone() }.with_plan(phone || url.plan).path());
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

/// The module choice for a placeholder of row `caption`/`ord` just put into semester `s` (owner,
/// 2026-11-04: „wenn man einen platzhalter slot erstellt auch gleich in die modulauswahl dafür
/// geschmissen werden"): its row's subgroup in front, whose „+ Einplanen" fills it
/// (`study::add_for`); back from there it stays for later. In the workbench of `s` a step into the
/// subgroup where it is not in front already; anywhere else the workbench at it (`enter_row`).
/// Until Folia 1.73.3 the placeholder stood where it was put in, and „Modul wählen" at it led there.
pub(super) fn choose_for(ctx: StudyCtx, s: SemesterKey, caption: &str, ord: i64) {
    let Some(bench) = ctx.bench.get_untracked().filter(|bench| bench.semester == s) else {
        enter_row(ctx, s, caption, ord);
        return;
    };
    let group = untrack(|| {
        let model = model_of(ctx, s)?;
        let at = model.view.lines.iter().position(|line| line.caption == caption && line.ord == ord)?;
        group_of_line(&model, at)
    });
    let Some((area, kind)) = group else { return };
    if bench.pick == BenchPick::Group(area, kind) {
        return;
    }
    if ctx.phone.get_untracked() {
        let behind = super::dom::sheet_scrolled();
        ctx.trail.update(|trail| trail.push(Step { page: Page::Group(area, kind), behind }));
        follow_trail(ctx);
        super::dom::sheet_scroll_to(0);
    } else {
        pick(ctx, BenchPick::Group(area, kind), Some(area));
    }
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

/// A subgroup „Zählt für" offers a module: its area and kind (as `BenchPick::Group` names it), its
/// short name, whether the module counts for it now, and whether it would be an exception there
/// (`model::fits`: not on its lists, not in its area's tree).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub area: usize,
    pub kind: GroupKind,
    pub name: String,
    pub current: bool,
    pub exception: bool,
    /// What it has of what it asks (`model::figures`).
    pub figures: (f64, f64),
}

/// What „Zählt für ›" offers: the subgroups, and whether the module counts where the study puts it
/// (nothing stored).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CountsFor {
    pub targets: Vec<Target>,
    pub auto: bool,
}

/// „Zählt für" for module `id` of semester `s`: every subgroup of every area (`all`), or those it
/// usually counts for — the choices whose lists hold it, the FÜS where its list holds it — and the
/// one it counts for now, whatever that is. Nothing for a module the plan names in a row of its
/// own: it counts there.
pub(super) fn counts_choices(ctx: StudyCtx, s: SemesterKey, id: &str, all: bool, t: &Texts) -> Option<CountsFor> {
    untrack(|| {
        ctx.with_input(|input, ready| {
            let study = &ready.study;
            let view = choices::plan_view(input, study);
            if view.lines.iter().any(|line| line.module_id.as_deref() == Some(id)) {
                return None;
            }
            let groups = Groups::of(study, &view);
            if groups.groups.is_empty() {
                return None;
            }
            let model = Model { due: Due::default(), fs: None, view, groups };
            let now = model::counted_in(&model.groups, &model.view, id, s);
            let stored = ctx.plan.is_some_and(|plan| plan.with(|doc| doc.counts_of(s, id).is_some() || doc.modules.iter().any(|m| m.semester == s && m.module_id == id && m.fills.is_some())));
            let targets: Vec<Target> = model
                .groups
                .areas
                .iter()
                .flat_map(|area| area.groups.iter().copied())
                .filter_map(|at| model.groups.groups.get(at).map(|group| (at, group)))
                .filter_map(|(at, group)| {
                    let exception = !model::fits(input, study, &model.view, group, id);
                    let current = Some(at) == now;
                    // As the menu offered them before any could be chosen: a choice whose lists have
                    // it, the FÜS where its list has it.
                    let usual = group.choice().and_then(|at| model.view.choices.get(at)).is_some_and(|choice| if choice.fues { !exception } else { choice.list_of(id).is_some() });
                    (all || current || usual).then(|| {
                        let name = match group.kind {
                            GroupKind::Pflicht if !all => format!("{} · {}", ready.area_name(Some(group.area), t), t.study_bench.pflicht),
                            _ => group_name(&model, group, true, t),
                        };
                        Target { area: group.area, kind: group.kind, name, current, exception, figures: model::figures(&model.view, group) }
                    })
                })
                .collect();
            Some(CountsFor { targets, auto: !stored })
        })
        .flatten()
    })
}

/// „Zählt für": module `id` of semester `s` counts for the subgroup `target` (its area and kind,
/// its name, whether that is an exception), or where the study puts it (`None`), with the note
/// „Rückgängig" takes it back by.
pub(super) fn count_for(ctx: StudyCtx, s: SemesterKey, id: String, title: String, target: Option<(usize, GroupKind, String, bool)>, t: &'static Texts) {
    let b = &t.study_bench;
    let note = match &target {
        Some((.., name, true)) => (b.counted_exception_note)(&title, name),
        Some((.., name, false)) => (b.counted_note)(&title, name),
        None => (b.counted_tree_note)(&title),
    };
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            let study = study::study(input);
            let view = choices::plan_view(input, &study);
            let groups = Groups::of(&study, &view);
            let group = target.and_then(|(area, kind, ..)| groups.find(area, kind)).and_then(|at| groups.groups.get(at));
            model::count_for(doc, &view, s, &id, group, input.program_id);
        },
        || {},
    );
}
