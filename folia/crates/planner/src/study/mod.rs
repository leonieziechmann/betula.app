//! „Mein Studium" (`/study`; owner, 2026-10-04: „Primär soll man da sein Studium planen können. Also
//! das aktuelle und zukünftige Semester"): the page the Studium tab leads to in the app. The
//! visitor's study semester by semester, as `folia_plans::study` works it out from „Mein
//! Studiengang" (program, study direction, Studienbeginn, semesters of leave, the last semester),
//! the program's Regelstudienplan and what the student put into the semesters and passed
//! (`folia_stores::studyplan`, the same lines the Stundenplan shows: one plan).
//!
//! The page (owner, mockup of 2026-10-04): an overview first, what is passed, planned and open in
//! all and by area (`overview.rs`), then the semesters. On a desktop one semester stands in focus
//! under a strip of all of them, with what fits it beside it (`focus.rs`, the view „Semester"), or
//! the whole plan as areas × semesters (`grid.rs`, „Gesamtplan"). A phone has one row of pages the
//! browser scrolls under a finger or by ‹ › (`pager.rs`): the semesters that are over, the overview
//! with the program and its ways (`phone.rs`), the current semester and the ones to come. The
//! semesters begin empty: „Module hinzufügen" (`picker.rs`) offers the rows of a Fachsemester of
//! the plan, the Wiederholer and the catalog; a row's menu (`menu.rs`) marks it as passed, moves it
//! and takes it out, and so does the bar of the rows selected for several at once (`focus.rs`).
//! Betula blocks nothing (owner: „Die Nutzer sind erwachsene Menschen"): it says what it knows
//! instead.
//!
//! The program is set up once (`setup.rs`, the first visit) and changed rarely, in a dialog that
//! says what of the plan counts in the new one (`side.rs`). The sidebar of a desktop holds it and
//! the ways to the program's pages.
//!
//! The page is the app's (R9, R20): the server writes the same stand-in for everybody, and nothing
//! of it reaches an address but how it is shown: a phone's semesters, the module beside them
//! (`StudyUrl`, a local view). What it shows is one memo (`State`), worked out anew when the
//! store, „Mein Studiengang", the program's plans or the current semester change; each part reads
//! what it needs of it through a memo of its own (R5). A click answers in the next frame and the
//! store follows after it (R21, `StudyCtx::change`).

mod dialog;
mod dom;
mod focus;
mod grid;
mod menu;
mod overview;
mod pager;
mod phone;
mod picker;
mod setup;
mod side;

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Prerequisite, Program};
use folia_pages::ask::{MetaAsk, ModuleAsk, PlanSourceAsk, StudyModulesAsk};
use folia_pages::PlanSource;
use folia_plans::plan;
use folia_plans::study::{self, AreaKind, Input, Item, Line, Study};
use folia_plans::studyplan::{self as stored, MineDoc, PlanDoc};
use folia_routes::url::{self, LocalView, StudyUrl};
use leptos::prelude::*;
use leptos_meta::Title;

use folia_data::{use_data, DataClient, DataError};
use folia_design::nav;
use folia_design::ui::Icon;
use folia_shell::frame::Frame;
use folia_shell::pending::{Change, Pending, Shape};
use folia_shell::seo::Seo;
use folia_shell::skeleton::{AppStandin, DetailSkeleton};
use folia_shell::tabs::Area;
use folia_stores::myprogram::{MyProgram, ProgramPlans};
use folia_stores::studyplan::Studyplan;
use folia_widgets::list::phone_layout;
use folia_widgets::local::ModuleInPlace;
use folia_widgets::module::ModulePanel;

use crate::i18n::{self, use_location, Texts};

/// The browser app (`csr`), or the server writing the page's stand-in.
const APP: bool = cfg!(feature = "csr");

/// Where the browser remembers the view of the semesters (`localStorage`, a view setting, R20).
const VIEW_KEY: &str = "betula.study.view";

/// The plans of the program „Mein Studiengang" keeps, as the catalog answers.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Plans {
    /// No program is kept.
    None,
    /// The answer is on its way.
    Waiting,
    /// The program is not in the snapshot.
    Gone,
    Found(Box<PlanSource>),
}

/// What `study::study` was asked with.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Setup {
    /// The plan shown (an index into the program's plans): a core plan or a page of one.
    pub shown: Option<usize>,
    /// The core plan, and the page that fills a row of it with that row's `ord`.
    pub core: Option<usize>,
    pub page: Option<(usize, i64)>,
    pub start: SemesterKey,
    /// The Studienbeginn is stored; else it is assumed (`studyplan::intake_start`) and the page
    /// asks for it first (`setup.rs`).
    pub start_stored: bool,
    /// The current semester.
    pub now: SemesterKey,
    pub leave: BTreeSet<SemesterKey>,
    pub until: Option<SemesterKey>,
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum State {
    /// The app is starting, or the answers are on their way.
    Waiting,
    /// No program is kept: the page sets the study up.
    NoProgram,
    /// The program kept is not in the snapshot any more: its stored name.
    Gone(String),
    Ready(Box<Ready>),
}

impl State {
    pub fn ready(&self) -> Option<&Ready> {
        match self {
            State::Ready(ready) => Some(ready),
            _ => None,
        }
    }
}

/// An area as the page names and colours it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Named {
    /// „Informatik", „FÜS", „Bachelor-Arbeit".
    pub short: String,
    /// „Komplex Informatik", „Fachübergreifendes Studium".
    pub full: String,
    /// The colour, a value of a custom property (`var(--fac-1)`).
    pub tone: &'static str,
    /// The Fachsemester its rows lie in.
    pub span: Option<(u8, u8)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ready {
    pub program: Program,
    /// The program's plans by their labels, where there are two or more.
    pub plans: Vec<(usize, String)>,
    pub setup: Setup,
    pub study: Study,
    /// The areas of `study.progress`, named and coloured.
    pub named: Vec<Named>,
    /// What the plan has open, where it fits next (`study::open_lines`).
    pub lines: Vec<Line>,
}

impl Ready {
    /// The name of the area an item counts towards: „ohne Bereich" for none.
    pub fn area_name(&self, area: Option<usize>, t: &Texts) -> String {
        area.and_then(|area| self.named.get(area)).map_or_else(|| t.study.no_area.to_string(), |named| named.short.clone())
    }

    pub fn tone(&self, area: Option<usize>) -> &'static str {
        area.and_then(|area| self.named.get(area)).map_or("var(--text-3)", |named| named.tone)
    }

    /// „3. FS" of a semester, „Urlaubssemester" for one of leave.
    pub fn fs_label(&self, s: SemesterKey, t: &Texts) -> Option<String> {
        let semester = self.study.semester(s)?;
        match semester.fs {
            Some(fs) => Some((t.study.fs)(fs)),
            None if semester.leave => Some(t.study.leave.to_string()),
            None => None,
        }
    }
}

/// The view of the semesters on a desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum View {
    /// One semester in focus (the mockup's concept A).
    Semester,
    /// Areas × semesters (concept C).
    Grid,
}

/// The dialog open over the page, if any (`dialog.rs`).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Dialog {
    /// „Module hinzufügen" to a semester; `catalog` opens its search; `chosen` are picked
    /// already (`Item::key`).
    Add { semester: SemesterKey, catalog: bool, chosen: Vec<String> },
    /// A cell of the Gesamtplan: the modules of an area (`None`: of no area) for a semester.
    Cell { area: Option<usize>, semester: SemesterKey },
    /// An area of the progress (`Study::progress`); `None` for what counts towards none.
    Area(Option<usize>),
    /// The areas on a phone.
    Areas,
    /// „Studiengang wechseln".
    Switch,
}

/// The rows selected in a semester (`Item::key`), for what is done to several at once (owner,
/// 2026-10-04: „eine Multiselection für Bearbeitung"): marked as passed, moved, taken out.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Selection {
    pub semester: Option<SemesterKey>,
    pub keys: BTreeSet<String>,
    /// The row a range (Shift and a click) begins at.
    pub anchor: Option<String>,
}

impl Selection {
    pub fn has(&self, s: SemesterKey, key: &str) -> bool {
        self.semester == Some(s) && self.keys.contains(key)
    }

    /// Whether rows of semester `s` are selected.
    pub fn any_in(&self, s: SemesterKey) -> bool {
        self.semester == Some(s) && !self.keys.is_empty()
    }
}

/// What the parts of the page share.
#[derive(Clone, Copy)]
pub(super) struct StudyCtx {
    /// The address: the module beside the page.
    pub url: Memo<StudyUrl>,
    pub state: Memo<State>,
    /// The current semester.
    pub now: Memo<SemesterKey>,
    pub plans: Memo<Plans>,
    /// The catalog's rows of the plan's modules and of every module planned or passed.
    pub rows: Memo<Vec<CatalogRow>>,
    /// What those modules ask for of others.
    pub needs: Memo<Vec<Prerequisite>>,
    pub plan: Option<Studyplan>,
    pub mine: Option<MyProgram>,
    pub source: StoredValue<Option<DataClient>>,
    /// The phone's layout: one semester as a card, dialogs as sheets.
    pub phone: RwSignal<bool>,
    /// The semester in focus; `None` the current one. One after the last is the page that adds a
    /// semester.
    pub focus: RwSignal<Option<SemesterKey>>,
    pub view: RwSignal<View>,
    pub dialog: RwSignal<Option<Dialog>>,
    /// The menu open, if any (`menu.rs`).
    pub menu: RwSignal<Option<menu::Menu>>,
    pub selection: RwSignal<Selection>,
    /// The rows being dragged to another semester: their semester and `Item::key`s.
    pub drag: RwSignal<Option<(SemesterKey, Vec<String>)>>,
    /// The note of the last change that „Rückgängig" takes back, and the plan before it.
    pub undo: RwSignal<Option<(String, PlanDoc)>>,
    /// A phone's semester was come to from its overview: the step before it in the history is the
    /// overview, and the way back goes back there (`phone.rs`).
    pub from_overview: RwSignal<bool>,
}

impl StudyCtx {
    /// Changes the plan after the next frame (R21), then runs `then`: `change` gets the plan and
    /// what the study is worked out from as it stands then. Nothing changes while the study is not
    /// there. With a `note`, „Rückgängig" takes the change back by it, where it changed anything.
    pub fn change_noted(self, note: Option<String>, change: impl FnOnce(&mut PlanDoc, &Input) + 'static, then: impl FnOnce() + 'static) {
        let Some(plan) = self.plan else {
            then();
            return;
        };
        let (plans, state, rows, undo) = (self.plans, self.state, self.rows, self.undo);
        nav::after_paint(move || {
            let before = plan.update(|doc| {
                let before = doc.clone();
                state.with_untracked(|state| {
                    let Some(ready) = state.ready() else { return };
                    plans.with_untracked(|plans| {
                        let Plans::Found(source) = plans else { return };
                        rows.with_untracked(|rows| {
                            let input = input_of(source, &ready.setup, &before, rows);
                            change(doc, &input);
                        });
                    });
                });
                (*doc != before).then_some(before)
            });
            if let (Some(note), Some(before)) = (note, before) {
                undo.try_set(Some((note, before)));
            }
            then();
        });
    }

    /// `f` with what the study is worked out from and the page's study, while there is one.
    /// Tracked: the plan, the program's plans, the catalog's rows, the state.
    pub fn with_input<R>(self, f: impl FnOnce(&Input, &Ready) -> R) -> Option<R> {
        let doc = self.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
        self.state.with(|state| {
            let ready = state.ready()?;
            self.plans.with(|plans| {
                let Plans::Found(source) = plans else { return None };
                self.rows.with(|rows| Some(f(&input_of(source, &ready.setup, &doc, rows), ready)))
            })
        })
    }

    /// Reads the page's study. Tracked.
    pub fn with_ready<R>(self, f: impl FnOnce(&Ready) -> R) -> Option<R> {
        self.state.with(|state| state.ready().map(f))
    }

    /// The semester in focus: the one chosen, else the current one; one after the last for the
    /// page that adds a semester. Tracked.
    pub fn focused(self) -> Option<SemesterKey> {
        let chosen = self.focus.get();
        self.with_ready(|ready| {
            let study = &ready.study;
            let last = study.semesters.last()?.key;
            let end = last.plus(1)?;
            Some(match chosen {
                Some(s) if s == end || study.semester(s).is_some() => s,
                _ => study.now.min(last),
            })
        })
        .flatten()
    }

    pub fn open(self, dialog: Dialog) {
        self.dialog.set(Some(dialog));
    }
}

/// What the study is worked out from: the program's plans as `setup` chose them, the plan, and the
/// catalog's rows of its modules.
pub(super) fn input_of<'a>(source: &'a PlanSource, setup: &'a Setup, doc: &'a PlanDoc, rows: &'a [CatalogRow]) -> Input<'a> {
    Input {
        program_id: &source.program.id,
        core: setup.core.and_then(|core| source.variants.get(core)),
        page: setup.page.and_then(|(page, ord)| Some((source.variants.get(page)?, ord))),
        start: setup.start,
        now: setup.now,
        leave: &setup.leave,
        until: setup.until,
        doc,
        rows,
        tree: &source.areas,
        placements: &source.placements,
        fues: &source.fues,
    }
}

/// The plan „Mein Studiengang" keeps among the program's plans (by its caption and direction, as
/// `ProgramPlans` reads them), else the first; with its core plan and the page of it.
pub(super) fn chosen(source: &PlanSource, caption: Option<&str>, direction: Option<&str>) -> (Option<usize>, Option<usize>, Option<(usize, i64)>) {
    if source.variants.is_empty() {
        return (None, None, None);
    }
    let plans = ProgramPlans::new(&source.variants, source.supplements.clone());
    let shown = caption.and_then(|caption| plans.shown_index(caption, direction)).unwrap_or(0).min(source.variants.len() - 1);
    match source.supplements.iter().find(|supplement| supplement.page == shown) {
        Some(supplement) => (Some(shown), Some(supplement.core), Some((supplement.page, supplement.ord))),
        None => (Some(shown), Some(shown), None),
    }
}

/// The Studienbeginn a study of `source` is assumed to have begun in: the current semester, or the
/// one before where the plan begins in the other half of the year.
pub(super) fn assumed_start(source: &PlanSource, core: Option<usize>, now: SemesterKey) -> SemesterKey {
    let intake = core.and_then(|core| source.variants.get(core)).and_then(|core| stored::intake_season(core, &source.linked));
    stored::intake_start(now, intake)
}

/// The colours of the areas of a plan, in turn; the FÜS, the thesis and the rest have their own.
const TONES: [&str; 9] = ["var(--fac-1)", "var(--fac-4)", "var(--t-sun)", "var(--fac-6)", "var(--fac-2)", "var(--t-violet)", "var(--fac-5)", "var(--t-rose)", "var(--t-ice)"];

/// The areas of `study` as the page names and colours them.
fn named(source: &PlanSource, input: &Input, study: &Study, t: &Texts) -> Vec<Named> {
    let rows = study::plan_rows(input.core, input.page);
    let mut plain = 0;
    study
        .progress
        .iter()
        .map(|progress| {
            let area = &progress.area;
            let span = area
                .rows
                .iter()
                .filter_map(|(caption, ord)| rows.iter().find(|row| row.variant.full == *caption && row.entry.ord == *ord))
                .fold(None, |span: Option<(u8, u8)>, row| Some(span.map_or((row.first, row.last), |(from, to)| (from.min(row.first), to.max(row.last)))));
            let named = match area.kind {
                AreaKind::Plan => {
                    let tone = TONES.get(plain % TONES.len()).copied().unwrap_or("var(--fac-1)");
                    plain += 1;
                    Named { short: area.name.clone(), full: area.full.clone(), tone, span: None }
                }
                AreaKind::Fues => Named { short: t.study.fues.to_string(), full: t.study.fues_full.to_string(), tone: "var(--t-teal)", span: None },
                AreaKind::Thesis => {
                    // The thesis by the name its row has („Bachelor-Arbeit").
                    let name = area
                        .rows
                        .first()
                        .and_then(|(caption, ord)| source.variants.iter().find(|variant| variant.full == *caption)?.entries.iter().find(|entry| entry.ord == *ord))
                        .map(|entry| plan::shown_name(&entry.module_name).to_string())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| t.study.thesis.to_string());
                    Named { short: name.clone(), full: name, tone: "var(--t-slate)", span: None }
                }
                AreaKind::Other => Named { short: t.study.other.to_string(), full: t.study.other.to_string(), tone: "var(--text-3)", span: None },
            };
            Named { span, ..named }
        })
        .collect()
}

/// What the page shows of a program's study.
fn ready(source: &PlanSource, mine: &MineDoc, now: SemesterKey, doc: &PlanDoc, rows: &[CatalogRow], t: &Texts) -> Ready {
    let (shown, core, page) = chosen(source, mine.caption.as_deref(), mine.direction.as_deref());
    let setup = Setup {
        shown,
        core,
        page,
        start: mine.start.unwrap_or_else(|| assumed_start(source, core, now)),
        start_stored: mine.start.is_some(),
        now,
        leave: mine.leave.clone(),
        until: mine.until,
    };
    let input = input_of(source, &setup, doc, rows);
    let study = study::study(&input);
    let lines = study::open_lines(&input, &study);
    let plans = match source.variants.len() {
        0 | 1 => Vec::new(),
        _ => source.variants.iter().enumerate().map(|(index, plan)| (index, plan.label.clone())).collect(),
    };
    let named = named(source, &input, &study, t);
    Ready { program: source.program.clone(), plans, named, setup, study, lines }
}

/// The catalog's rows of `source`'s modules and of `extra`, each once.
fn rows_of(source: &PlanSource, extra: &[CatalogRow]) -> Vec<CatalogRow> {
    source.linked.iter().chain(extra.iter().filter(|row| !source.linked.iter().any(|linked| linked.id == row.id))).cloned().collect()
}

/// What „Mein Studium" takes into the timetable of `semester` (the Stundenplan's „Importieren" from
/// „Mein Studium"): its Wiederholer, what was not passed and is not planned again
/// (`study::retake_import`), seen from the current semester `now`. `None` before the
/// Studienbeginn. The rest of a semester's timetable is its part of the study already: the two
/// pages share one plan.
pub(crate) fn current_import(source: &PlanSource, mine: &MineDoc, doc: &PlanDoc, semester: SemesterKey, now: SemesterKey) -> Option<stored::Import> {
    let ready = ready(source, mine, now, doc, &source.linked, i18n::texts(folia_locale::Locale::De));
    (semester >= ready.setup.start).then(|| study::retake_import(&ready.study, doc, semester))
}

#[component]
pub fn StudyPage() -> impl IntoView {
    let t = i18n::t();
    if !APP {
        return server_page().into_any();
    }
    let location = use_location();
    let url = Memo::new(move |_| StudyUrl::parse(&location.search.get()));
    let mine = MyProgram::expect();
    let plan = Studyplan::expect();
    let source = StoredValue::new(use_data().ok());
    let today = crate::studyplan::today();

    // The program kept, and its plans.
    let program_id = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    let plans = Memo::new(move |before| {
        let Some(id) = program_id.get() else { return Plans::None };
        let Some(now) = source.with_value(|source| source.as_ref().map(|source| source.now(&PlanSourceAsk { program_id: id.clone(), locale: t.locale }))) else {
            return Plans::Waiting;
        };
        folia_pages::ask::unless_pending(now, before, |now| match now {
            Ok(Some(plans)) => Plans::Found(Box::new(plans)),
            Ok(None) => Plans::Gone,
            Err(_) => Plans::Waiting,
        })
    });
    // „jetzt": the snapshot's current semester, as the Stundenplan has it.
    let current = Memo::new(move |before| {
        let now = source.with_value(|source| source.as_ref().map(|source| source.now(&MetaAsk {})))?;
        folia_pages::ask::unless_pending(now, before, |meta| meta.ok().and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse)))
    });
    // The modules of the plan and of the semesters: their rows (for the ones the plan does not
    // name) and what they ask for of others.
    let ids = Memo::new(move |_| {
        let mut ids: Vec<String> = plan.map(|plan| plan.with(|doc| doc.modules.iter().map(|m| m.module_id.clone()).chain(doc.passed.iter().map(|p| p.module_id.clone())).collect())).unwrap_or_default();
        plans.with(|plans| {
            if let Plans::Found(source) = plans {
                ids.extend(source.linked.iter().map(|row| row.id.clone()));
            }
        });
        ids.sort();
        ids.dedup();
        ids
    });
    let modules = Memo::new(move |before: Option<&(Vec<CatalogRow>, Vec<Prerequisite>)>| {
        let ids = ids.get();
        if ids.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let Some(now) = source.with_value(|source| source.as_ref().map(|source| source.now(&StudyModulesAsk { ids }))) else { return (Vec::new(), Vec::new()) };
        folia_pages::ask::unless_pending(now, before, |now| now.map(|modules| (modules.rows, modules.prerequisites)).unwrap_or_default())
    });
    let rows = Memo::new(move |_| {
        plans.with(|plans| match plans {
            Plans::Found(source) => modules.with(|(extra, _)| rows_of(source, extra)),
            _ => Vec::new(),
        })
    });
    let needs = Memo::new(move |_| modules.with(|(_, needs)| needs.clone()));
    let now = Memo::new(move |_| crate::studyplan::key_of(&Default::default(), current.get(), &PlanDoc::default(), today));
    let state = Memo::new(move |_| {
        let now = now.get();
        let kept = mine.map(|mine| mine.get()).unwrap_or_default();
        plans.with(|plans| match plans {
            Plans::None => State::NoProgram,
            Plans::Waiting => State::Waiting,
            Plans::Gone => State::Gone(kept.name.clone().unwrap_or_default()),
            Plans::Found(source) => {
                let doc = plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
                rows.with(|rows| State::Ready(Box::new(ready(source, &kept, now, &doc, rows, t))))
            }
        })
    });
    let view = RwSignal::new(match nav::local_get(VIEW_KEY).as_deref() {
        Some("grid") => View::Grid,
        _ => View::Semester,
    });
    Effect::new(move |before: Option<View>| {
        let now = view.get();
        if before.is_some_and(|before| before != now) {
            nav::local_set(VIEW_KEY, if now == View::Grid { "grid" } else { "semester" });
        }
        now
    });
    let ctx = StudyCtx {
        url,
        state,
        now,
        plans,
        rows,
        needs,
        plan,
        mine,
        source,
        phone: phone_layout(),
        focus: RwSignal::new(None),
        view,
        dialog: RwSignal::new(None),
        menu: RwSignal::new(None),
        selection: RwSignal::new(Selection::default()),
        drag: RwSignal::new(None),
        undo: RwSignal::new(None),
        from_overview: RwSignal::new(false),
    };
    // Escape closes the menu open, wherever the focus is; else it lets go of the rows selected,
    // where no dialog takes it first.
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() != "Escape" || ev.default_prevented() {
                return;
            }
            if ctx.menu.with_untracked(Option::is_some) {
                ctx.menu.set(None);
                dom::give_focus_back();
            } else if ctx.dialog.with_untracked(Option::is_none) && ctx.selection.with_untracked(|selection| !selection.keys.is_empty()) {
                ctx.selection.set(Selection::default());
            }
        });
        on_cleanup(move || handle.remove());
    });

    // What fills the page: the study, or the module opened from it after „Vollbild" (on a phone
    // whatever is opened).
    let filling = Memo::new(move |_| url.with(|url| folia_routes::local::filling(url, ctx.phone.get())));
    // The module beside the page, and a pick on its way there (`pending`).
    let going = Pending::expect();
    let target = Memo::new(move |_| {
        let going = going.filter(|going| going.change() == Some(Change::Aside))?;
        Some(StudyUrl::parse(&going.search_on(url::STUDY)?))
    });
    let open = Memo::new(move |_| url.with(|url| url.open.clone()));
    let picked = Signal::derive(move || match target.get() {
        Some(to) => to.open.is_some(),
        None => open.with(Option::is_some),
    });
    let aside = move || view! { <StudyAside ctx open target/> };

    (move || {
        if let Some(id) = filling.get() {
            let back = url.with_untracked(|url| folia_routes::local::back_href(url, ctx.phone.get_untracked()));
            return view! { <ModuleInPlace id area=Area::Programs back/> }.into_any();
        }
        view! {
            <Title text=t.study.title/>
            <Frame title=t.study.title sheet=true sidebar=move || view! { <side::StudySidebar ctx/> } aside aside_picked=picked>
                <StudySeo/>
                <div class="page-inner study">
                    <StudyMain ctx/>
                </div>
                <dialog::DialogHost ctx/>
                <menu::MenuHost ctx/>
                <UndoNote ctx/>
            </Frame>
        }
        .into_any()
    })
    .into_any()
}

/// Which of the page's states is shown; the parts read the rest themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Waiting,
    Setup,
    Study,
}

#[component]
fn StudyMain(ctx: StudyCtx) -> impl IntoView {
    let kind = Memo::new(move |_| {
        ctx.state.with(|state| match state {
            State::Waiting => Kind::Waiting,
            State::NoProgram | State::Gone(_) => Kind::Setup,
            State::Ready(ready) if !ready.setup.start_stored => Kind::Setup,
            State::Ready(_) => Kind::Study,
        })
    });
    move || match kind.get() {
        Kind::Waiting => view! {
            <div class="panel sk-sweep sk-head" aria-hidden="true">
                <i class="sk sk-w2"></i>
                <div class="sk-title-row"><i class="sk sk-w4 sk-big"></i><i class="sk sk-w3"></i></div>
                <i class="sk sk-w6"></i>
            </div>
        }
        .into_any(),
        Kind::Setup => view! { <setup::Setup ctx/> }.into_any(),
        Kind::Study => view! { <StudyBody ctx/> }.into_any(),
    }
}

/// The study: on a desktop the overview and the semesters, on a phone a row of pages.
#[component]
fn StudyBody(ctx: StudyCtx) -> impl IntoView {
    move || {
        if ctx.phone.get() {
            view! { <phone::Phone ctx/> }.into_any()
        } else {
            view! {
                <overview::Overview ctx/>
                <Semesters ctx/>
            }
            .into_any()
        }
    }
}

/// The semesters on a desktop: one in focus, or the Gesamtplan.
#[component]
fn Semesters(ctx: StudyCtx) -> impl IntoView {
    let grid = Memo::new(move |_| ctx.view.get() == View::Grid);
    move || if grid.get() { view! { <grid::Grid ctx/> }.into_any() } else { view! { <focus::Focus ctx/> }.into_any() }
}

/// „Semester | Gesamtplan": the view of the semesters on a desktop.
#[component]
fn ViewSwitch(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let button = move |view: View, label: &'static str, icon: &'static str| {
        view! {
            <button type="button" aria-pressed=move || if ctx.view.get() == view { "true" } else { "false" } on:click=move |_| {
                ctx.selection.set(Selection::default());
                ctx.view.set(view);
            }>
                <Icon name=icon/>{label}
            </button>
        }
    };
    view! {
        <div class="st-views" role="group" aria-label=s.view_label>
            {button(View::Semester, s.view_semester, "layout-list")}
            {button(View::Grid, s.view_grid, "layout-grid")}
        </div>
    }
}

/// The note of the last change with „Rückgängig", at the bottom of the window until the next one
/// or a while; over a phone's dots (`phone.rs`).
#[component]
fn UndoNote(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone())));
    // Each note stands for a while; a newer one starts the while anew.
    Effect::new(move |_| {
        if let Some(note) = note.get() {
            set_timeout(
                move || {
                    if ctx.undo.with_untracked(|undo| undo.as_ref().is_some_and(|(now, _)| *now == note)) {
                        ctx.undo.try_set(None);
                    }
                },
                std::time::Duration::from_secs(8),
            );
        }
    });
    let over_dock = Memo::new(move |_| ctx.phone.get());
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        ctx.undo.set(None);
        plan.update_after_paint(move |doc| *doc = before);
    };
    move || {
        note.get().map(|note| {
            view! {
                <div class="st-undo" class:over-dock=move || over_dock.get() role="status">
                    <Icon name="check"/>
                    <span>{note}</span>
                    <button class="mini hit" type="button" on:click=restore>{t.common.undo}</button>
                    <button class="icon-btn hit" type="button" aria-label=t.study.close on:click=move |_| ctx.undo.set(None)><Icon name="x"/></button>
                </div>
            }
        })
    }
}

/// The module beside the page: the catalog's preview, docked, with „Vollbild" in place.
#[component]
fn StudyAside(ctx: StudyCtx, open: Memo<Option<String>>, target: Memo<Option<StudyUrl>>) -> impl IntoView {
    let t = i18n::t();
    let source = use_data();
    let module = Memo::new(move |before| {
        let now = match open.get() {
            None => Ok(None),
            Some(id) => source.clone().and_then(|source| source.now(&ModuleAsk { id })).map(Some),
        };
        DataError::or_before(now, before)
    });
    let going = Pending::expect();
    move || {
        let there = open.with(Option::is_some);
        let coming = target.with(|to| to.as_ref().is_some_and(|to| to.open.is_some()));
        if coming && (!there || going.is_some_and(|going| going.waits(Change::Aside))) {
            return view! { <DetailSkeleton aside=true calm=there/> }.into_any();
        }
        let here = ctx.url.get();
        match module.get() {
            Ok(Some(Some(data))) => {
                let full_href = folia_routes::local::full_href(&here, &data.module.id);
                view! { <ModulePanel data close_href=here.with_open(None).path() docked=true full_href=Some(full_href)/> }.into_any()
            }
            Ok(Some(None)) | Err(_) => view! {
                <section class="panel detail aside" id="preview">
                    <div class="state">
                        <p class="state-title">{t.catalog.module_not_found}</p>
                        <p>{t.catalog.module_not_in_catalog}</p>
                        <a class="btn secondary" href=t.path(&here.with_open(None).path())>{t.catalog.close_preview}</a>
                    </div>
                </section>
            }
            .into_any(),
            Ok(None) => ().into_any(),
        }
    }
}

/// The address of a module beside the page.
pub(super) fn module_href(ctx: StudyCtx, id: &str, t: &Texts) -> String {
    t.path(&ctx.url.with_untracked(|url| url.with_open(Some(id)).path()))
}

/// A page of one visitor: the same address and explanation for everybody, for no index.
#[component]
fn StudySeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.study.title description=t.study.seo_description path=url::STUDY card=folia_shell::seo::STUDYPLAN_CARD noindex=true/> }
}

/// What the server writes for „Mein Studium", a route of the app (§4.2): its tags and the page's
/// skeleton, and without JavaScript what it needs instead.
fn server_page() -> impl IntoView {
    let t = i18n::t();
    view! {
        <Title text=t.study.title/>
        <StudySeo/>
        <AppStandin shape=Shape::Study title=t.study.server_title hint=t.study.server_hint/>
    }
}

/// Seconds since 1970, for when a module is planned; 0 outside the browser.
pub(super) fn now_secs() -> u64 {
    crate::studyplan::now_secs()
}

/// The credits of a number, as the page writes them: „6", „7,5".
pub(super) fn n(value: f64, t: &Texts) -> String {
    folia_design::format::number(value, t.locale)
}

/// What a semester holds of an item key, as it stands now.
pub(super) fn item_of(ready: &Ready, s: SemesterKey, key: &str) -> Option<Item> {
    ready.study.semester(s)?.items.iter().find(|item| item.key() == key).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The snapshot the tests read: `FOLIA_TEST_SNAPSHOT`, else the one `snapshot/current.json`
    /// names.
    fn plans_of(id: &str) -> PlanSource {
        use std::sync::Mutex;

        use folia_data::CatalogSource;
        use folia_model::native::NativeDatabase;
        use folia_model::{Database, DbError};

        struct Snapshot(Mutex<NativeDatabase>);
        impl CatalogSource for Snapshot {
            fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
                let db = self.0.lock().map_err(|_| DbError::Unavailable("poisoned".to_string()))?;
                job(&*db);
                Ok(())
            }
        }
        let path = std::env::var("FOLIA_TEST_SNAPSHOT").map(std::path::PathBuf::from).unwrap_or_else(|_| {
            let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../snapshot");
            let pointer = std::fs::read_to_string(dir.join("current.json")).unwrap();
            dir.join(pointer.split("\"file\"").nth(1).and_then(|rest| rest.split('"').nth(1)).unwrap())
        });
        let source = DataClient::new(folia_data::Source(std::sync::Arc::new(Snapshot(Mutex::new(NativeDatabase::open(&path).unwrap())))));
        source.now(&PlanSourceAsk { program_id: id.to_string(), locale: folia_locale::Locale::De }).unwrap().unwrap()
    }

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// Informatik B.Sc. in its third semester: the areas of the mockup, the semesters empty, the
    /// Studienbeginn assumed until one is stored; filled from the plan, the first two are
    /// Wiederholer until ticked off, and the Stundenplan's import takes them.
    #[test]
    fn mein_studium_of_informatik() {
        let source = plans_of("079-82-2008");
        let t = i18n::texts(folia_locale::Locale::De);
        let mut mine = MineDoc { caption: Some(String::new()), ..MineDoc::default() };
        let doc = PlanDoc::default();
        let assumed = ready(&source, &mine, key("2026W"), &doc, &source.linked, t);
        assert_eq!((assumed.setup.start, assumed.setup.start_stored), (key("2026W"), false), "a winter intake starts in the current winter");
        let names: Vec<&str> = assumed.named.iter().map(|named| named.short.as_str()).collect();
        assert_eq!(names[..5], ["Informatik", "Mathematik", "Nebenfach", "FÜS", "Fachstudium"]);
        assert!(names[5].to_lowercase().contains("arbeit"), "the thesis by its row's name: {names:?}");
        assert_eq!(assumed.named[4].span, Some((4, 6)), "the Fachstudium's rows lie in the 4th to 6th");
        assert_eq!(assumed.study.total, 180.0);
        mine.start = Some(key("2025W"));
        let third = ready(&source, &mine, key("2026W"), &doc, &source.linked, t);
        assert!(third.setup.start_stored);
        assert!(third.study.semesters.iter().all(|semester| semester.items.is_empty()));
        assert_eq!(third.study.semester(key("2026W")).and_then(|semester| semester.fs), Some(3));
        // The first visit fills the first two from the plan.
        let mut filled = doc.clone();
        let input = input_of(&source, &third.setup, &doc, &source.linked);
        let (modules, _) = study::fill_past(&mut filled, &input, 1);
        assert!(modules > 5);
        let after = ready(&source, &mine, key("2026W"), &filled, &source.linked, t);
        assert_eq!(after.study.retakes.len(), after.study.semesters.iter().filter(|s| s.key < key("2026W")).map(|s| s.items.len()).sum::<usize>());
        let import = current_import(&source, &mine, &filled, key("2026W"), key("2026W")).unwrap();
        assert_eq!(import.modules.len(), modules);
        assert!(current_import(&source, &mine, &filled, key("2025S"), key("2026W")).is_none(), "before the Studienbeginn");
    }

    /// Elektrotechnik B.Sc. 2022 has a plan per study direction: the one kept is the one studied.
    #[test]
    fn the_study_direction_kept_is_studied() {
        let source = plans_of("048-82-2022");
        assert!(source.variants.len() >= 2);
        let second = source.variants[1].full.clone();
        let (shown, core, _) = chosen(&source, Some(&second), None);
        assert_eq!((shown, core), (Some(1), Some(1)));
        assert_eq!(chosen(&source, Some("Vertiefung B"), None).0, Some(0));
        let mine = MineDoc { caption: Some(second), start: Some(key("2026W")), ..MineDoc::default() };
        let ready = ready(&source, &mine, key("2026W"), &PlanDoc::default(), &source.linked, i18n::texts(folia_locale::Locale::De));
        assert_eq!(ready.plans.len(), source.variants.len());
        assert_eq!(ready.setup.shown, Some(1));
    }
}
