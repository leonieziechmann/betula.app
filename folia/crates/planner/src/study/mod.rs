//! „Mein Studium" (`/study`; owner, 2026-10-04: „Primär soll man da sein Studium planen können. Also
//! das aktuelle und zukünftige Semester"): the page the Studium tab leads to in the app. The
//! visitor's study semester by semester, as `folia_plans::study` works it out from „Mein
//! Studiengang" (program, study direction, Studienbeginn, semesters of leave, the last semester),
//! the program's Regelstudienplan and what the student put into the semesters and passed
//! (`folia_stores::studyplan`, the same lines the Stundenplan shows: one plan).
//!
//! The page (owner, mockup of 2026-10-04): an overview first, what is passed, planned and open in
//! all and by area (`overview.rs`), then the semesters. On a desktop one semester stands in focus,
//! its name the way to any other, in a box that is a list as the catalog's (`focus.rs`, the view
//! „Semester"), or the whole plan as areas × semesters (`grid/`, „Gesamtplan"), which the sidebar
//! switches between (`side.rs`). A phone has one row of pages the
//! browser scrolls under a finger or by ‹ › (`pager.rs`): the semesters that are over, the overview
//! with the program and its ways (`phone.rs`), the current semester and the ones to come. The
//! semesters begin empty: „Module hinzufügen" opens the workbench (`bench.rs`; owner, 2026-11-03:
//! „Passt so, bau das jetzt in Folia"), a mode for one semester with the areas of the plan and
//! their subgroups, what the plan puts into its Fachsemester, and the semester with what it holds;
//! a row's menu (`menu.rs`) marks it as passed, moves it, lets it count for another subgroup and
//! takes it out, and so does the bar of the rows selected for several at once (`focus.rs`).
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

mod bench;
mod card;
mod choose;
mod count;
mod dialog;
mod dom;
mod focus;
mod grid;
mod menu;
mod overview;
mod pager;
mod phone;
mod picker;
mod regular;
mod setup;
mod side;
mod size;

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Prerequisite, Program};
use folia_pages::ask::{MetaAsk, ModuleAsk, PlanSourceAsk, StudyModulesAsk};
use folia_pages::PlanSource;
use folia_plans::plan;
use folia_plans::study::{self, AreaKind, Input, Item, Line, Study};
use folia_plans::studyplan::{MineDoc, PlanDoc};
use folia_routes::url::{self, LocalView, StudyUrl, StudyView};
use leptos::prelude::*;
use folia_design::stage::Title;

use folia_data::{use_data, DataClient, DataError};
use folia_design::nav;
use folia_design::ui::Icon;
use folia_design::waitnote::WaitNote;
use folia_shell::frame::Frame;
use folia_shell::pending::{Change, Pending, Shape};
use folia_shell::seo::Seo;
use folia_shell::skeleton::{AppStandin, DetailSkeleton};
use folia_shell::tabs::Area;
use folia_stores::myprogram::MyProgram;
// What the study is worked out from, as the start page works it out too (`folia_stores::study`).
pub(super) use folia_stores::study::{assumed_start, chosen, input_of, rows_of, Setup};
use folia_stores::studyplan::{PlanHint, Studyplan};
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
    /// An area of the progress (`Study::progress`); `None` for what counts towards none.
    Area(Option<usize>),
    /// The areas on a phone.
    Areas,
    /// „Studiengang wechseln".
    Switch,
    /// „Zählt für" › „Anderer Bereich …" of a module of a semester (`count.rs`).
    CountFor { semester: SemesterKey, id: String },
    /// „Umfang …" of a placeholder of a semester (`size.rs`).
    Size { semester: SemesterKey, pid: u32 },
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
    /// The entries of the Gesamtplan being dragged to another semester: their semester and
    /// `Item::key`s.
    pub drag: RwSignal<Option<(SemesterKey, Vec<String>)>>,
    /// The last change, which „Rückgängig" takes back (`Undo`).
    pub undo: RwSignal<Option<Undo>>,
    /// The same for a view (the Regelstudienplan, the Wahlpflicht) on a phone.
    pub view_back: RwSignal<bool>,
    /// The choice the Wahlpflicht shows on a wide screen (an index of `PlanView::choices`;
    /// `choose.rs`). A phone's is in the address (`StudyUrl::choice`).
    pub choice: RwSignal<Option<usize>>,
    /// A phone's choice of the Wahlpflicht was opened from its list, the step before in the
    /// history: its way back goes back there.
    pub choice_back: RwSignal<bool>,
    /// What „Einplanen" of the module beside the page aims at where the Wahlpflicht opened it: the
    /// module and the choice's placeholder and row (`PlanHint`, `choose.rs`). For that module
    /// alone.
    pub aside_hint: RwSignal<Option<(String, PlanHint)>>,
    /// The workbench, while it is open (`bench.rs`): planning one semester is a mode.
    pub bench: RwSignal<Option<bench::Bench>>,
    /// The pages of the workbench's sheet on a phone gone into from its root (`bench::Page`).
    pub trail: RwSignal<Vec<bench::Step>>,
    /// The Gesamtplan fills the page, the sidebar and the overview aside, and the browser the
    /// screen with it (`grid::full`; owner, 2026-11-09: „einen Fullscreen knopf oben rechts … Das
    /// soll so sein, dass die nav bar noch funktioniert und oben auch noch die suchleiste ist").
    /// Not kept: a reload shows the page as it is.
    pub full: RwSignal<bool>,
}

/// The last change that „Rückgängig" takes back: what its note says, the plan before it, and its
/// turn, which tells its note from the note of a change after it.
#[derive(Clone, Debug)]
pub struct Undo {
    pub note: String,
    pub before: PlanDoc,
    pub turn: u32,
}

/// The turn of the next change with a note.
static UNDO_TURNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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
                let turn = UNDO_TURNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                undo.try_set(Some(Undo { note, before, turn }));
            }
            then();
        });
    }

    /// `f` with what the study is worked out from and the page's study, while there is one.
    /// Tracked: the plan, the program's plans, the catalog's rows, the state.
    pub fn with_input<R>(self, f: impl FnOnce(&Input, &Ready) -> R) -> Option<R> {
        let doc = self.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
        // The program's plans and the rows before the state they make (R16): an effect that calls
        // this would miss a change of them that leaves the state as it was (the workbench's aim).
        self.plans.track();
        self.rows.track();
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

    /// The same without subscribing: for what a key decides.
    pub fn focused_untracked(self) -> Option<SemesterKey> {
        untrack(|| self.focused())
    }

    pub fn open(self, dialog: Dialog) {
        self.dialog.set(Some(dialog));
    }
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
    let setup = Setup::of(source, mine, now);
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
    // The modules of the plan and of the semesters, the visits among them: their rows (for the
    // ones the plan does not name) and what they ask for of others.
    let ids = Memo::new(move |_| {
        let mut ids: Vec<String> = plan.map(|plan| plan.with(folia_stores::study::module_ids)).unwrap_or_default();
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
        view_back: RwSignal::new(false),
        choice: RwSignal::new(None),
        choice_back: RwSignal::new(false),
        aside_hint: RwSignal::new(None),
        bench: RwSignal::new(None),
        trail: RwSignal::new(Vec::new()),
        full: RwSignal::new(false),
    };
    // While the workbench is open, „Einplanen" elsewhere aims at its semester and subgroup
    // (`Planning::bench`); its end ends that.
    if let Some(planning) = folia_stores::studyplan::Planning::expect() {
        Effect::new(move |_| planning.set_bench(bench::aim_of(ctx)));
        on_cleanup(move || planning.set_bench(None));
    }
    // On a desktop the workbench is the semester's card opened: its view comes with it and goes
    // back with its end, and another view ends it (`bench::follow_view`).
    let steps = Pending::expect();
    bench::follow_view(
        ctx,
        Callback::new(move |view: StudyView| {
            if let Some(steps) = steps {
                steps.go(&StudyUrl::default().with_view(view).path(), leptos_router::NavigateOptions::default());
            }
        }),
    );
    // On a phone Back leaves the mode, as it closes what lies over the page: the page's own state,
    // which a module opened from its sheet (a new address) leaves alone. While a module's sheet lies
    // over it, that is closed first (a place of its own): the layer waits meanwhile. A page gone
    // into in the sheet is a step that Back and Esc take back first (`bench::back`), one at a time:
    // the layer is the sheet's alone, its watcher made anew after each step, for a watcher hears one
    // Back, and two made by one tap (a placeholder's „Modul wählen" opening the sheet at a page) are
    // one group of the browser's, which one Back closes at once (`folia_shell::nav`).
    let armed = RwSignal::new(true);
    folia_shell::nav::layer_while(
        move || armed.get() && ctx.phone.get() && ctx.bench.with(Option::is_some) && ctx.url.with(|url| url.open.is_none()),
        move || {
            if ctx.trail.with_untracked(Vec::is_empty) {
                bench::leave(ctx);
            } else {
                bench::back(ctx);
                armed.set(false);
                request_animation_frame(move || {
                    armed.try_set(true);
                });
            }
        },
        folia_shell::nav::Kind::State,
    );
    // A view come to from the overview (a phone's way to it): the step before it in the history.
    Effect::new(move |before: Option<(StudyView, bool)>| {
        let now = url.with(|url| (url.view, url.plan));
        if let Some(before) = before.filter(|before| before.0 != now.0) {
            ctx.view_back.set(before == (StudyView::Semesters, false));
        }
        now
    });
    // Until its tab is left: the page stays built behind (`folia_shell::stage`), and when the tab
    // comes back the step before is the other tab's page. Where the browser says what the step
    // before is (the Navigation API), its way back asks it; where it does not, every change of
    // tabs is a new entry, and the way back went there instead of to the overview.
    let front = folia_design::stage::front();
    Effect::new(move |_| {
        if !front.get() && ctx.view_back.get_untracked() {
            ctx.view_back.set(false);
        }
    });
    // What lies over the page (folia_shell::nav): the menu, the rows selected, the dialog. On a
    // phone whose browser has `CloseWatcher` (Android) Back closes the newest, as Esc does
    // elsewhere; a new address closes them.
    folia_shell::nav::layer_while(move || ctx.dialog.with(Option::is_some), move || ctx.dialog.set(None), folia_shell::nav::Kind::Dialog);
    folia_shell::nav::layer_while(move || ctx.selection.with(|selection| !selection.keys.is_empty()), move || ctx.selection.set(Selection::default()), folia_shell::nav::Kind::Over);
    folia_shell::nav::layer_while(move || ctx.menu.with(Option::is_some), move || ctx.menu.set(None), folia_shell::nav::Kind::Over);
    // Escape closes the menu open, wherever the focus is; else it lets go of the rows selected,
    // where no dialog takes it first. Where Back's watcher has the newest of them, it does. Ctrl
    // or ⌘ and A selects every row of the semester in view (oberflaeche.html of the Merkliste:
    // „Strg A nimmt alle sichtbaren"), not in a field, where it is the field's.
    // Only the page in front: a page kept built behind (`folia_design::stage`) takes no key.
    let in_front = folia_design::stage::front();
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if (ev.ctrl_key() || ev.meta_key()) && !ev.alt_key() && !ev.shift_key() && ev.key().eq_ignore_ascii_case("a") {
                let shown = ctx.url.with_untracked(|url| url.view == StudyView::Semesters) && ctx.bench.with_untracked(Option::is_none);
                let free = shown && !ev.default_prevented() && in_front.get_untracked() && ctx.menu.with_untracked(Option::is_none) && ctx.dialog.with_untracked(Option::is_none) && !dom::typing(&ev);
                let semester = ctx.focused_untracked().filter(|semester| untrack(|| ctx.with_ready(|ready| ready.study.semester(*semester).is_some_and(|semester| !semester.items.is_empty()))).unwrap_or(false));
                if let Some(semester) = semester.filter(|_| free) {
                    ev.prevent_default();
                    menu::act(ctx, menu::Act::SelectAll(semester), i18n::t());
                }
                return;
            }
            if ev.key() != "Escape" || ev.default_prevented() || folia_shell::nav::watched() || !in_front.get_untracked() {
                return;
            }
            // What this Esc closes is all it does: no close request after it reaches the page's
            // state (the semester of a phone's row) as well.
            if ctx.menu.with_untracked(Option::is_some) {
                ev.prevent_default();
                ctx.menu.set(None);
                dom::give_focus_back();
            } else if ctx.dialog.with_untracked(Option::is_none) && ctx.selection.with_untracked(|selection| !selection.keys.is_empty()) {
                ev.prevent_default();
                ctx.selection.set(Selection::default());
            } else if ctx.dialog.with_untracked(Option::is_none) && ctx.bench.with_untracked(Option::is_none) && ctx.full.get_untracked() && ctx.url.with_untracked(|url| url.open.is_none()) {
                // The Gesamtplan's full screen, as every full screen ends.
                ev.prevent_default();
                grid::full(ctx, false);
            }
        });
        on_cleanup(move || handle.remove());
    });

    // The Gesamtplan's full screen ends where the browser's ends (Esc, which the browser takes
    // itself there) and where the plan is not shown any more: another view, a phone.
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("fullscreenchange", move |_| {
            if !dom::in_fullscreen() && ctx.full.get_untracked() {
                ctx.full.set(false);
            }
        });
        on_cleanup(move || handle.remove());
    });
    Effect::new(move |_| {
        let away = ctx.view.get() != View::Grid || ctx.url.with(|url| url.view != StudyView::Semesters) || ctx.phone.get();
        if away && ctx.full.get_untracked() {
            grid::full(ctx, false);
        }
    });
    on_cleanup(move || {
        if ctx.full.try_get_untracked() == Some(true) {
            dom::leave_fullscreen();
        }
    });

    // What fills the page: the study, or the module opened from it after „Vollbild" (until then it
    // stands beside the page, on a phone a sheet over it).
    let filling = Memo::new(move |_| url.with(folia_routes::local::filling));
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
    // A module moved on two devices of the profile at once (Betula Sync) stands in each semester it
    // was moved to; the hint over the study names it by its title where the page has it.
    let title = Callback::new(move |id: String| modules.with(|(rows, _)| rows.iter().find(|row| row.id == id).map(|row| row.title.clone())).unwrap_or(id));

    (move || {
        if let Some(id) = filling.get() {
            let back = url.with_untracked(folia_routes::local::back_href);
            return view! { <ModuleInPlace id area=Area::Programs back/> }.into_any();
        }
        view! {
            <Title text=t.study.title/>
            <Frame title=t.study.title sheet=true sidebar=move || view! { <side::StudySidebar ctx/> } aside aside_picked=picked>
                <StudySeo/>
                <div class="page-inner study">
                    <folia_widgets::syncnote::MovedNotes title/>
                    <StudyMain ctx/>
                </div>
                <dialog::DialogHost ctx/>
                <bench::BenchSheet ctx/>
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

/// The study: on a desktop the overview and the semesters, on a phone a row of pages; or a view of
/// it, the Regelstudienplan or the Wahlpflicht, without the overview (owner, 2026-11-04: „Beim
/// regelstudienplan und bei dem wahlpflicht teil soll die progress bar entfernt werden. Die ist
/// nicht relevant für die beiden subpages"): on a desktop the view alone in the page, on a phone a
/// page of its own.
#[component]
fn StudyBody(ctx: StudyCtx) -> impl IntoView {
    let view = Memo::new(move |_| ctx.url.with(|url| url.view));
    let full = Memo::new(move |_| ctx.full.get() && ctx.view.get() == View::Grid);
    // The workbench: on a desktop the semester's card opened (`bench::BenchHalf`), on a phone its
    // sheet over the page (`bench::BenchSheet`).
    move || {
        let phone = ctx.phone.get();
        match view.get() {
            StudyView::Semesters if phone => view! { <phone::Phone ctx/> }.into_any(),
            // The Gesamtplan in full screen fills the page without the overview (`grid::full`).
            StudyView::Semesters => view! {
                {move || (!full.get()).then(|| view! { <overview::Overview ctx/> })}
                <Semesters ctx/>
            }
            .into_any(),
            StudyView::Plan => view! { <regular::Regular ctx/> }.into_any(),
            StudyView::Choices => view! { <choose::Choose ctx/> }.into_any(),
        }
    }
}

/// The semesters on a desktop: one in focus, or the Gesamtplan.
#[component]
fn Semesters(ctx: StudyCtx) -> impl IntoView {
    let grid = Memo::new(move |_| ctx.view.get() == View::Grid);
    move || if grid.get() { view! { <grid::Grid ctx/> }.into_any() } else { view! { <focus::Focus ctx/> }.into_any() }
}

/// The note of the last change with „Rückgängig": a note that waits for interaction
/// (`folia_design::waitnote::WaitNote`; owner, 2026-11-04: „Bitte stelle den auch mal auf diese
/// wait notification um … dass er angezeigt bleibt, wenn nichts passiert man es aber auch schließen
/// kann und es allgemein besser mit modals funktioniert"): it stands until the student does
/// something else, × (a wide screen) or a finger swiping it down puts it away. On a phone over the
/// workbench's sheet at the bottom of the screen, as over a module's (app.css, `.study-note`).
#[component]
fn UndoNote(ctx: StudyCtx) -> impl IntoView {
    // A new change makes a new note.
    let turn = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().map(|undo| undo.turn)));
    move || turn.get().map(|turn| view! { <UndoView ctx turn/> })
}

/// One note: a check, what was done, „Rückgängig" after a line.
#[component]
fn UndoView(ctx: StudyCtx, turn: u32) -> impl IntoView {
    let t = i18n::t();
    let note = ctx.undo.with_untracked(|undo| undo.as_ref().map(|undo| undo.note.clone())).unwrap_or_default();
    // This note goes, not a newer one that took its place.
    let gone = Callback::new(move |()| {
        if ctx.undo.with_untracked(|undo| undo.as_ref().is_some_and(|undo| undo.turn == turn)) {
            ctx.undo.set(None);
        }
    });
    // „Rückgängig": the plan as it was, and the note goes down and away.
    let done = RwSignal::new(false);
    let restore = move |_| {
        let (Some(plan), Some(undo)) = (ctx.plan, ctx.undo.get_untracked().filter(|undo| undo.turn == turn)) else { return };
        done.set(true);
        plan.update_after_paint(move |doc| *doc = undo.before);
    };
    view! {
        <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note study-note">
            <div class="mark-toast">
                <span class="plan-toast-mark out" aria-hidden="true"><Icon name="check"/></span>
                <span class="mark-toast-said">{note}</span>
                <span class="mark-toast-gap"></span>
                <div class="plan-toast-end">
                    <span class="plan-toast-sep" aria-hidden="true"></span>
                    <button class="plan-toast-undo" type="button" on:click=restore>{t.common.undo}</button>
                </div>
            </div>
        </WaitNote>
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
            return view! { <DetailSkeleton aside=true calm=there module=true/> }.into_any();
        }
        let here = ctx.url.get();
        match module.get() {
            Ok(Some(Some(data))) => {
                let full_href = folia_routes::local::full_href(&here, &data.module.id);
                // Opened from a choice of the Wahlpflicht: „Einplanen" plans for it.
                let id = data.module.id.clone();
                let hint = Signal::derive(move || ctx.aside_hint.with(|hint| hint.as_ref().filter(|(of, _)| *of == id).map(|(_, hint)| hint.clone())));
                view! { <ModulePanel data close_href=here.with_open(None).path() docked=true full_href=Some(full_href) hint/> }.into_any()
            }
            Ok(Some(None)) | Err(_) => view! {
                <section class="panel detail aside is-module" id="preview">
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

    /// The catalog's rows of `ids`, as „Mein Studium" asks for them.
    fn plans_of_rows(ids: &[&str]) -> Vec<CatalogRow> {
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
        source.now(&StudyModulesAsk { ids: ids.iter().map(|id| id.to_string()).collect() }).unwrap().rows
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

    /// The views of Informatik B.Sc. in its third semester (owner, 2026-10-14, concept A): every row
    /// of the plan with where it stands, and what is left to choose. The first two semesters filled
    /// and passed but Mathematik IT-1, taken again now; the FÜS and the Proseminar done as rows;
    /// Analysis II planned out of the Anwendungsfach's list.
    #[test]
    fn the_views_of_informatik() {
        use folia_plans::choices::{plan_view, RowState};

        let source = plans_of("079-82-2008");
        let plan = [
            "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
            "m\t2026S\t12101\t1\t\tplan", "m\t2026S\t11903\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan",
            "m\t2026W\t11787\t1\t\tplan", "m\t2026W\t11112\t1\t\t", "m\t2027S\t11104\t1\t\t",
            "p\t1\t2025W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium",
            "q\t2025W\t079-82-2008\t17\t\tFachübergreifendes Studium", "q\t2026S\t079-82-2008\t10\t\tProseminar oder Praktikum",
            "d\t2025W\t12104", "d\t2025W\t12107", "d\t2025W\t12102", "d\t2026S\t12101", "d\t2026S\t11903", "d\t2026S\t11113",
        ]
        .join("\n");
        let doc = PlanDoc::restored(&plan);
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2025W")), ..MineDoc::default() };
        let (shown, core, page) = chosen(&source, mine.caption.as_deref(), None);
        let setup = Setup { shown, core, page, start: key("2025W"), start_stored: true, now: key("2026W"), leave: BTreeSet::new(), until: None };
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&["11104"]));
        let input = input_of(&source, &setup, &doc, &rows);
        let study = study::study(&input);
        let view = plan_view(&input, &study);
        assert_eq!(view.lines.len(), 23);
        let state = |name: &str| view.lines.iter().find(|line| line.name == name).map(|line| line.state).unwrap();
        assert_eq!(state("Entwicklung von Softwaresystemen"), RowState::Passed(key("2025W")));
        assert_eq!(state("Mathematik IT-1 (Diskrete Mathematik)"), RowState::Again { failed: key("2025W"), planned: key("2026W") });
        assert_eq!(state("Theoretische Informatik"), RowState::Planned(key("2026W")));
        assert_eq!(state("Betriebssysteme I"), RowState::Open);
        assert_eq!(state("Fachübergreifendes Studium"), RowState::Passed(key("2025W")), "a row marked as done");
        assert_eq!(state("Proseminar oder Praktikum"), RowState::Passed(key("2026S")));
        // The choices: the two rows of the Anwendungsfach one, the three Komplexe tied by their sum.
        let names: Vec<&str> = view.choices.iter().map(|choice| choice.name.as_str()).collect();
        assert_eq!(names.len(), 9, "{names:?}");
        let anwendung = view.choices.iter().find(|choice| choice.name == "Anwendungsfach").unwrap();
        assert_eq!((anwendung.lines.len(), anwendung.least, anwendung.first, anwendung.last), (2, 12.0, 3, 4));
        assert_eq!(anwendung.lists.iter().map(|list| list.name.as_str()).collect::<Vec<_>>()[..2], ["Mathematik", "Maschinenbau / Elektrotechnik"]);
        assert_eq!(anwendung.taken.iter().map(|taken| (taken.id.as_str(), taken.passed)).collect::<Vec<_>>(), [("11104", false)], "Analysis II counts towards the Anwendungsfach");
        assert!(!anwendung.complete() && anwendung.open() == 4.0);
        let komplex = view.choices.iter().find(|choice| choice.name == "Komplex Praktische Informatik").unwrap();
        assert_eq!((komplex.least, komplex.most, komplex.credits_text.as_str()), (10.0, 24.0, "10–24"));
        assert_eq!(komplex.together.as_ref().map(|(credits, others)| (*credits, others.len())), Some((44.0, 2)));
        let fues = view.choices.iter().find(|choice| choice.fues).unwrap();
        assert!(fues.complete() && fues.lists.is_empty());
        // The three Komplexe and the Wahlpflicht im 4. FS, whose lists are theirs: one group of 50.
        assert_eq!(view.groups.len(), 1);
        let group = &view.groups[0];
        let named = |ats: &[usize]| ats.iter().map(|at| view.choices[*at].name.clone()).collect::<Vec<_>>();
        assert_eq!(named(&group.members), ["Komplex Grundlagen der Informatik", "Komplex Praktische Informatik", "Komplex Angewandte und Technische Informatik"]);
        assert_eq!(named(&group.extras).len(), 1);
        assert!(named(&group.extras)[0].starts_with("Wahlpflicht: Komplex"));
        assert_eq!((group.label.as_str(), group.together, group.target(&view.choices)), ("Komplexe des Fachstudiums", 44.0, 50.0));
        assert!(group.members(&view).iter().all(|member| member.room == folia_plans::choices::Room::Open && member.stand.most == 24.0));
        // Left to choose: Praktische Mathematik 6, the Anwendungsfach 4 more, the Komplexe 50
        // (the 4. FS 6 and 44 together), the Seminar 4.
        assert_eq!(view.to_choose, 64.0);
    }

    /// The Komplexe of Informatik B.Sc.: 24 each, one of them to 30 (owner, 2026-11-04: „nur eins
    /// davon darf 30 haben"). Praktische Informatik with five modules of 6 is the one over 24 and
    /// keeps the room to 30, whichever of the group's choices counted which; Grundlagen der
    /// Informatik over 24 as well is over; a second Proseminar counts for none and stays spare.
    #[test]
    fn the_komplexe_take_24_one_of_them_30() {
        use folia_plans::choices::{plan_view, Room};

        let source = plans_of("079-82-2008");
        let pi = ["14012", "13569", "12330", "14460", "12311"];
        let lines: Vec<String> = pi.iter().map(|id| format!("m\t2028W\t{id}\t1\t\t")).chain(["m\t2027S\t12111\t1\t\t".to_string(), "m\t2027S\t12216\t1\t\t".to_string()]).collect();
        let doc = PlanDoc::restored(&lines.join("\n"));
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2026W")), ..MineDoc::default() };
        let (shown, core, page) = chosen(&source, mine.caption.as_deref(), None);
        let setup = Setup { shown, core, page, start: key("2026W"), start_stored: true, now: key("2026W"), leave: BTreeSet::new(), until: None };
        let gdi = ["12329", "12350", "11450", "13565"];
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&pi.iter().chain(gdi.iter()).chain(["12111", "12216"].iter()).copied().collect::<Vec<_>>()));
        let view_of = |doc: &PlanDoc| {
            let input = input_of(&source, &setup, doc, &rows);
            let study = study::study(&input);
            plan_view(&input, &study)
        };
        let view = view_of(&doc);
        let group = &view.groups[0];
        let stands = |view: &folia_plans::choices::PlanView| view.groups[0].members(view).iter().map(|member| (member.stand.taken(), member.stand.most, member.room)).collect::<Vec<_>>();
        assert_eq!(stands(&view), [(0.0, 24.0, Room::Gone), (30.0, 30.0, Room::Kept), (0.0, 24.0, Room::Gone)]);
        // The 50 less 30, and at least the 10 each of the other two lack.
        assert_eq!(group.open(&view), 20.0);
        // Proseminar and Praktikum: the second counts for none, beyond the 4 asked.
        let pro = view.choices.iter().position(|choice| choice.name == "Proseminar oder Praktikum").unwrap();
        assert_eq!(view.spare_of(pro).len(), 1);
        assert_eq!(view.stand_of(pro).taken(), 8.0);
        // Grundlagen der Informatik with 30 as well: it is over, and counts to 24.
        let mut doc = doc;
        for id in gdi {
            doc.plan(key("2029S"), id, 2, None);
        }
        let view = view_of(&doc);
        assert_eq!(stands(&view), [(30.0, 24.0, Room::Over), (30.0, 30.0, Room::Kept), (0.0, 24.0, Room::Gone)]);
        assert_eq!(view.groups[0].open(&view), 10.0, "the Angewandte und Technische Informatik's 10");
    }

    /// The workbench of Informatik B.Sc. in its third semester (folia/design/hinzufuegen/
    /// konzepte.html, third round): the six areas of the overview with their subgroups, what the
    /// plan puts into the 3. FS, and a module counted for the subgroup it was planned from — one
    /// that two choices list as well, and „Zählt für" changing it.
    #[test]
    fn the_workbench_of_informatik() {
        use folia_plans::bench::{self, Bench, Due, GroupKind};
        use folia_plans::choices::plan_view;

        let source = plans_of("079-82-2008");
        let w = key("2026W");
        let plan = [
            "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
            "m\t2026S\t12101\t1\t\tplan", "m\t2026S\t11903\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan",
            "d\t2025W\t12104", "d\t2025W\t12107", "d\t2025W\t12102", "d\t2025W\t11112", "d\t2026S\t12101", "d\t2026S\t11903", "d\t2026S\t11113",
            "q\t2026S\t079-82-2008\t10\t\tProseminar oder Praktikum",
        ]
        .join("\n");
        let mut doc = PlanDoc::restored(&plan);
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2025W")), ..MineDoc::default() };
        let (shown, core, page) = chosen(&source, mine.caption.as_deref(), None);
        let setup = Setup { shown, core, page, start: key("2025W"), start_stored: true, now: w, leave: BTreeSet::new(), until: None };
        let view_of = |doc: &PlanDoc| {
            let input = input_of(&source, &setup, doc, &source.linked);
            let study = study::study(&input);
            let view = plan_view(&input, &study);
            (Bench::of(&study, &view), view, study)
        };
        let (bench, view, study) = view_of(&doc);
        // One row per area, as the overview has them; the Pflicht first, then the choices.
        assert_eq!(bench.areas.len(), 6);
        let kinds = |area: usize| bench.area(area).unwrap().groups.iter().map(|at| bench.groups[*at].kind).collect::<Vec<_>>();
        let names = |area: usize| bench.area(area).unwrap().groups.iter().filter_map(|at| bench.groups[*at].choice()).map(|at| view.choices[at].name.clone()).collect::<Vec<_>>();
        assert_eq!(kinds(0).first(), Some(&GroupKind::Pflicht), "Informatik: its Pflicht");
        assert_eq!(names(0), ["Proseminar oder Praktikum"]);
        assert_eq!(kinds(1), [GroupKind::Pflicht], "Mathematik: its Pflicht alone");
        eprintln!("{:?}", (0..6).map(names).collect::<Vec<_>>());
        assert_eq!(names(2), ["Anwendungsfach", "Modul aus dem Bereich Praktische Mathematik"], "Nebenfach: two choices, no Pflicht, by their Fachsemester");
        assert_eq!(kinds(3).len(), 1, "the FÜS: one choice");
        assert_eq!(names(4).len(), 5, "Fachstudium: the Wahlpflicht-Komplex, the three Komplexe, the Seminar: {:?}", names(4));
        // The Nebenfach opens with what the 3. FS asks for: the Anwendungsfach.
        let anw = bench.opens_with(&view, 2, Some(3)).unwrap();
        assert_eq!(bench.groups[anw].choice().map(|at| view.choices[at].name.as_str()), Some("Anwendungsfach"));
        // Laut Plan im 3. FS: three Pflichtmodule to take over, the Anwendungsfach, the FÜS of the
        // 1. FS still open, the 4. FS to take now.
        let due = Due::of(&view, Some(3), Some(3));
        let pflicht: Vec<&str> = due.pflicht.iter().filter_map(|at| view.lines[*at].module_id.as_deref()).collect();
        assert_eq!(pflicht, ["11787", "12202", "11213"]);
        assert_eq!(due.takeover(&view, w), ["11787", "12202", "11213"]);
        assert!(due.choices.iter().any(|at| view.choices[*at].name == "Anwendungsfach"));
        assert!(due.earlier_choices.iter().any(|at| view.choices[*at].fues), "the FÜS of the 1. FS");
        assert!(!due.next.is_empty(), "the 4. FS's Pflicht");
        assert!(due.earlier.is_empty(), "the 1. and 2. FS are passed");
        // A module both the Anwendungsfach and Praktische Mathematik list, planned from the second:
        // it counts there, not in the first choice that lists it.
        let pm = view.choices.iter().position(|choice| choice.name.ends_with("Praktische Mathematik")).unwrap();
        let anw_choice = bench.groups[anw].choice().unwrap();
        let in_pm: BTreeSet<&str> = view.choices[pm].modules().into_iter().collect();
        let both = view.choices[anw_choice].modules().into_iter().find(|id| in_pm.contains(id)).expect("a module both list");
        let pm_group = bench.of_choice(pm).unwrap();
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&[both]));
        let before = doc.clone();
        let input = input_of(&source, &setup, &before, &rows);
        assert_eq!(bench::plan_for(&mut doc, &input, &view, w, both, Some(&bench.groups[pm_group]), 7), 1);
        assert_eq!(doc.counts_of(w, both).map(|row| row.ord), view.choices[pm].lines.first().map(|at| view.lines[*at].ord));
        let after = doc.clone();
        let input = input_of(&source, &setup, &after, &rows);
        let study_after = study::study(&input);
        let view_after = plan_view(&input, &study_after);
        let bench_after = Bench::of(&study_after, &view_after);
        assert_eq!(bench::counted_in(&bench_after, &view_after, both, w), Some(pm_group), "counted for Praktische Mathematik");
        assert!(view_after.choices[anw_choice].taken.is_empty(), "not for the Anwendungsfach");
        let item = study_after.semester(w).unwrap().items.iter().find(|item| item.module_id() == Some(both)).unwrap();
        assert_eq!(item.area, Some(2), "in the Nebenfach");
        // „Zählt für ›": the Anwendungsfach.
        bench::count_for(&mut doc, &view_after, w, both, bench_after.of_choice(anw_choice).and_then(|at| bench_after.groups.get(at)), "079-82-2008");
        let moved = doc.clone();
        let input = input_of(&source, &setup, &moved, &rows);
        let study_moved = study::study(&input);
        let view_moved = plan_view(&input, &study_moved);
        assert_eq!(bench::counted_in(&Bench::of(&study_moved, &view_moved), &view_moved, both, w), Some(anw));
        // From the Pflicht a module comes from the plan; a stored placeholder of a choice in the
        // semester is filled, not remembered.
        let tool = bench.find(0, GroupKind::Pflicht).unwrap();
        assert_eq!(bench::plan_for(&mut doc, &input, &view_moved, w, "11787", Some(&bench.groups[tool]), 8), 1);
        assert!(doc.modules.iter().any(|m| m.module_id == "11787" && m.from_plan && m.counts.is_none()));
        let fues = view.choices.iter().position(|choice| choice.fues).unwrap();
        let fues_line = &view.lines[view.choices[fues].lines[0]];
        let before = doc.clone();
        let input = input_of(&source, &setup, &before, &rows);
        study::add(&mut doc, &input, w, &[study::Pick::Row { caption: fues_line.caption.clone(), ord: fues_line.ord }], 9);
        let pid = doc.placeholders.iter().find(|p| p.semester == w).map(|p| p.pid).unwrap();
        let fues_id = source.fues.first().unwrap().clone();
        let before = doc.clone();
        let input = input_of(&source, &setup, &before, &rows);
        let fues_group = bench.of_choice(fues).unwrap();
        assert_eq!(bench::plan_for(&mut doc, &input, &view_moved, w, &fues_id, Some(&bench.groups[fues_group]), 10), 1);
        let planned = doc.modules.iter().find(|m| m.module_id == fues_id).unwrap();
        assert_eq!((planned.fills, planned.counts.as_ref()), (Some(pid), None));
        let _ = study;
    }

    /// „Zählt für" › „Anderer Bereich …" (owner, 2026-11-08: „jedes modul zu jedem möglichen bereich
    /// zuzuordnen … dass es angezeigt wird, wenn man das gemacht hat"): Datenbanken, of the Komplex
    /// Praktische Informatik, counts for the Anwendungsfach or for Mathematik's Pflichtmodule as the
    /// student says, marked as an exception there; „Automatisch" takes it back to its Komplex.
    #[test]
    fn any_module_counts_for_any_subgroup_as_an_exception() {
        use folia_plans::bench::{self, Bench, GroupKind};
        use folia_plans::choices::plan_view;

        let source = plans_of("079-82-2008");
        let w = key("2026W");
        let mut doc = PlanDoc::restored("m\t2026W\t12330\t1\t\t");
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2025W")), ..MineDoc::default() };
        let (shown, core, page) = chosen(&source, mine.caption.as_deref(), None);
        let setup = Setup { shown, core, page, start: key("2025W"), start_stored: true, now: w, leave: BTreeSet::new(), until: None };
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&["12330"]));
        let state = |doc: &PlanDoc| {
            let input = input_of(&source, &setup, doc, &rows);
            let study = study::study(&input);
            let view = plan_view(&input, &study);
            let bench = Bench::of(&study, &view);
            let item = study.semester(w).unwrap().items.iter().find(|item| item.module_id() == Some("12330")).unwrap().clone();
            (study, view, bench, item)
        };
        let (study, view, groups, item) = state(&doc);
        let komplex = view.choices.iter().position(|choice| choice.name == "Komplex Praktische Informatik").unwrap();
        assert_eq!((item.area, item.exception), (Some(4), false), "in the Fachstudium, where it belongs");
        assert_eq!(bench::counted_in(&groups, &view, "12330", w), groups.of_choice(komplex));
        // It fits its Komplex, not the Anwendungsfach, not Mathematik's Pflichtmodule.
        let input = input_of(&source, &setup, &doc, &rows);
        let anw = view.choices.iter().position(|choice| choice.name == "Anwendungsfach").unwrap();
        let anw_group = groups.of_choice(anw).unwrap();
        let mathe = groups.find(1, GroupKind::Pflicht).unwrap();
        assert!(bench::fits(&input, &study, &view, &groups.groups[groups.of_choice(komplex).unwrap()], "12330"));
        assert!(!bench::fits(&input, &study, &view, &groups.groups[anw_group], "12330"));
        assert!(!bench::fits(&input, &study, &view, &groups.groups[mathe], "12330"));

        // For the Anwendungsfach: it counts there, in the Nebenfach, as an exception.
        bench::count_for(&mut doc, &view, w, "12330", Some(&groups.groups[anw_group]), "079-82-2008");
        let (_, view, groups, item) = state(&doc);
        assert_eq!((item.area, item.exception), (Some(2), true));
        let taken = &view.choices[anw].taken;
        assert_eq!(taken.iter().map(|taken| (taken.id.as_str(), taken.exception)).collect::<Vec<_>>(), [("12330", true)]);
        assert!(view.choices[komplex].taken.is_empty());
        assert_eq!(bench::counted_in(&groups, &view, "12330", w), Some(anw_group));

        // For Mathematik's Pflichtmodule: its area counts it, no choice does.
        bench::count_for(&mut doc, &view, w, "12330", Some(&groups.groups[mathe]), "079-82-2008");
        let (study, view, groups, item) = state(&doc);
        assert_eq!((item.area, item.exception), (Some(1), true));
        assert!(view.choices.iter().all(|choice| choice.taken.is_empty()));
        assert_eq!(view.pflicht.iter().map(|(taken, _)| taken.id.as_str()).collect::<Vec<_>>(), ["12330"]);
        assert_eq!(bench::counted_in(&groups, &view, "12330", w), Some(mathe));
        assert_eq!(study.progress[1].planned, 6.0);
        assert_eq!(PlanDoc::restored(&doc.stored()), doc, "what it counts for is kept");

        // Automatisch: back to its Komplex, no exception.
        bench::count_for(&mut doc, &view, w, "12330", None, "079-82-2008");
        let (_, view, _, item) = state(&doc);
        assert_eq!((item.area, item.exception), (Some(4), false));
        assert_eq!(view.choices[komplex].taken.iter().map(|taken| taken.id.as_str()).collect::<Vec<_>>(), ["12330"]);
    }

    /// „Aus dem 4. FS vorziehen" (owner, 2026-11-05: „das ergibt ja nur sinn für module, die in dem
    /// ausgewählten semester auch angeboten werden und wofür die voraussetzungen erfüllt sind"):
    /// Informatik's 4. FS seen from the 3. in a WiSe suggests no Pflicht — Betriebssysteme I and the
    /// Digitaltechnik-Praktikum are offered in the SoSe only —, from a SoSe both; a module whose
    /// required module is not in place by then is no suggestion until that is planned into the
    /// semester as well; a choice stays while a module of its lists can be taken.
    #[test]
    fn vorziehen_suggests_what_the_semester_can_take() {
        use folia_model::labels::Code;
        use folia_model::rows::Prerequisite;
        use folia_plans::bench::{self, Bench, Due};
        use folia_plans::choices::plan_view;
        use folia_plans::study::Offer;

        let source = plans_of("079-82-2008");
        let (w, summer) = (key("2026W"), key("2027S"));
        let plan = [
            "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
            "m\t2026S\t12101\t1\t\tplan", "m\t2026S\t11903\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan",
            "d\t2025W\t12104", "d\t2025W\t12107", "d\t2025W\t12102", "d\t2025W\t11112", "d\t2026S\t12101", "d\t2026S\t11903", "d\t2026S\t11113",
        ]
        .join("\n");
        let doc = PlanDoc::restored(&plan);
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2025W")), ..MineDoc::default() };
        let (shown, core, page) = chosen(&source, mine.caption.as_deref(), None);
        let setup = Setup { shown, core, page, start: key("2025W"), start_stored: true, now: w, leave: BTreeSet::new(), until: None };
        let input = input_of(&source, &setup, &doc, &source.linked);
        let study = study::study(&input);
        let view = plan_view(&input, &study);
        let groups = Bench::of(&study, &view);
        let due = Due::of(&view, Some(3), Some(3));
        let ids = |rows: &[usize]| rows.iter().filter_map(|at| view.lines[*at].module_id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&due.next), ["12204", "12214"], "the 4. FS's Pflicht");
        assert!(!due.next_choices.is_empty(), "the 4. FS's choices");
        // The catalog's rows of the choices' lists, as the root asks for them.
        let lists_of = |choice: usize| bench::candidates(&view, &groups.groups[groups.of_choice(choice).unwrap()], &source.fues).into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        let listed: Vec<String> = due.next_choices.iter().flat_map(|choice| lists_of(*choice)).collect();
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&listed.iter().map(String::as_str).collect::<Vec<_>>()));
        let offered_in = |s: SemesterKey, id: &str| rows.iter().find(|row| row.id == id).map_or_else(Offer::default, Offer::of).offered(s);

        // In the WiSe no Pflicht of the 4. FS; a choice where a module of its lists is offered then.
        let (next, choices) = due.ahead(&view, &groups, w, &source.fues, |id| offered_in(w, id));
        assert!(next.is_empty(), "offered in the SoSe only: {:?}", ids(&next));
        for choice in &due.next_choices {
            let modules = lists_of(*choice);
            let any = modules.is_empty() || modules.iter().any(|id| offered_in(w, id));
            assert_eq!(choices.contains(choice), any, "{}", view.choices[*choice].name);
        }
        // From a SoSe both rows. What takes nothing keeps a choice only where it has no lists.
        let (next, _) = due.ahead(&view, &groups, summer, &source.fues, |id| offered_in(summer, id));
        assert_eq!(ids(&next), ["12204", "12214"]);
        let (none, kept) = due.ahead(&view, &groups, w, &source.fues, |_| false);
        assert!(none.is_empty());
        assert!(kept.iter().all(|choice| lists_of(*choice).is_empty()));

        // What a module requires („setzt voraus"), as the semester warns of it: the
        // Digitaltechnik-Praktikum asking for Algorithmen und Datenstrukturen (11756), which stands
        // nowhere, is no suggestion until that is in the semester too; asking for Digitaltechnik
        // (11903), passed, it is. Only a recommendation keeps it as well.
        let asks = |required: &str, kind: &str| vec![Prerequisite { module_id: "12214".to_string(), required_module_id: required.to_string(), kind: Code::parse(kind), choice: 1, required_title: None, required_offer_status: None }];
        let title_of = |id: &str| rows.iter().find(|row| row.id == id).map_or_else(|| id.to_string(), |row| row.title.clone());
        let takes = |doc: &PlanDoc, needs: &[Prerequisite], id: &str| offered_in(summer, id) && !picker::lacks_required(doc, needs, id, &title_of(id), summer, w);
        let (next, _) = due.ahead(&view, &groups, summer, &source.fues, |id| takes(&doc, &asks("11756", "mandatory"), id));
        assert_eq!(ids(&next), ["12204"], "Algorithmen und Datenstrukturen is not in place");
        let mut with = doc.clone();
        assert!(with.plan(summer, "11756", 1, None));
        let (next, _) = due.ahead(&view, &groups, summer, &source.fues, |id| takes(&with, &asks("11756", "mandatory"), id));
        assert_eq!(ids(&next), ["12204", "12214"], "planned into the same semester");
        let (next, _) = due.ahead(&view, &groups, summer, &source.fues, |id| takes(&doc, &asks("11903", "mandatory"), id));
        assert_eq!(ids(&next), ["12204", "12214"], "Digitaltechnik is passed");
        let (next, _) = due.ahead(&view, &groups, summer, &source.fues, |id| takes(&doc, &asks("11756", "recommended"), id));
        assert_eq!(ids(&next), ["12204", "12214"], "only recommended");
    }

    /// The Gesamtplan's subsections of Informatik B.Sc. (owner, 2026-11-09: „subsections für
    /// bereiche, wie zum beispiel bei informatik wo im fachstudium den bereichen praktische
    /// informatik, grundlagen der informatik und technische informatik zugeordnet wird"): the
    /// Fachstudium unfolds into its three Komplexe and its Seminar, each with what it holds against
    /// its 10–24 and the room to 30, its „Wahlpflicht: Komplex …" none of them; the Nebenfach into
    /// its two choices, where Lineare Algebra II's 8 LP for Praktische Mathematik's 6 are no „über
    /// Bedarf"; a second Proseminar is.
    #[test]
    fn the_gesamtplan_s_subsections() {
        use folia_plans::choices::Room;
        use grid::model::{Cell, Layout, Subs};

        let source = plans_of("079-82-2008");
        let ids = ["14012", "13569", "12330", "12329", "12350", "11102", "11104", "12111", "12216"];
        let plan = [
            "m\t2027S\t11102\t1\t\t", "m\t2027S\t11104\t2\t\t", "m\t2027S\t12111\t3\t\t", "m\t2027S\t12216\t4\t\t",
            "m\t2027W\t14012\t1\t\t", "m\t2027W\t13569\t2\t\t", "m\t2027W\t12330\t3\t\t",
            "m\t2028S\t12329\t1\t\t", "m\t2028S\t12350\t2\t\t",
        ]
        .join("\n");
        let doc = PlanDoc::restored(&plan);
        let mine = MineDoc { caption: Some(String::new()), start: Some(key("2025W")), ..MineDoc::default() };
        let mut rows = source.linked.clone();
        rows.extend(plans_of_rows(&ids));
        let t = i18n::texts(folia_locale::Locale::De);
        let ready = ready(&source, &mine, key("2026W"), &doc, &rows, t);
        let input = input_of(&source, &ready.setup, &doc, &rows);
        let subs = Subs::of(&input, &ready);
        let layout = Layout::of(&ready, &subs, false, &Default::default(), None, t);
        let area = |name: &str| layout.rows.iter().find(|row| row.name == name).unwrap();
        let said = |name: &str| area(name).subs.iter().map(|sub| format!("{} {}", sub.name, sub.figure)).collect::<Vec<_>>();
        assert_eq!(said("Fachstudium"), ["Grundlagen der Informatik 16/10–24", "Praktische Informatik 18/10–24", "Angewandte und Technische Informatik 0/10–24", "Seminar oder Praktikum 0/4"]);
        let komplexe = &area("Fachstudium").subs;
        assert!(komplexe.iter().take(3).all(|sub| sub.stand.is_some_and(|(stand, room)| stand.least == 10.0 && stand.most == 24.0 && room == Some((Room::Open, 6.0)))), "{komplexe:?}");
        assert_eq!(said("Nebenfach"), ["Anwendungsfach 8/12", "Praktische Mathematik 8/6"]);
        assert_eq!(said("Informatik"), ["Pflichtmodule 0/62", "Proseminar oder Praktikum 8/4"]);
        assert!(said("Mathematik").is_empty(), "an area of its Pflicht alone has none");
        // A subsection's bar holds its modules, its lanes theirs; the area unfolds into them.
        let praktische = komplexe.iter().find(|sub| sub.name == "Praktische Informatik").unwrap();
        let pieces: Vec<String> = praktische.cells.iter().flat_map(|cell| match cell {
            Cell::Semester(cell) => cell.pieces.iter().map(|piece| piece.key.clone()).collect::<Vec<_>>(),
            Cell::Past { .. } => Vec::new(),
        }).collect();
        assert_eq!(pieces, ["m:14012", "m:13569", "m:12330"]);
        assert_eq!(praktische.lanes.len(), 3);
        assert!(area("Fachstudium").lanes.is_empty());
        // 8 of 6 LP is no „über Bedarf"; the second Proseminar is.
        let over: Vec<String> = layout.rows.iter().flat_map(|row| row.cells.iter()).flat_map(|cell| match cell {
            Cell::Semester(cell) => cell.pieces.iter().filter(|piece| piece.over).map(|piece| piece.key.clone()).collect::<Vec<_>>(),
            Cell::Past { .. } => Vec::new(),
        }).collect();
        assert_eq!(over.len(), 1, "{over:?}");
        assert!(["m:12111", "m:12216"].contains(&over[0].as_str()));
        assert!(!area("Nebenfach").over);
        assert!(layout.keys().contains("Fachstudium\tPraktische Informatik"));
        // A row of one lane alone does not unfold, its lane only its bar again (owner, 2026-11-10:
        // „dass ein platzhalter, der nur für ein modul ist kein ausklappbares element wird. Also zum
        // Beispiel seminar oder Praktikum Oder BA"); neither does the one button count it.
        let seminar = komplexe.iter().find(|sub| sub.name == "Seminar oder Praktikum").unwrap();
        assert_eq!(seminar.lanes.len(), 1, "{:?}", seminar.lanes);
        assert!(!seminar.unfolds() && praktische.unfolds());
        assert!(!layout.keys().contains("Fachstudium\tSeminar oder Praktikum"));
        let single: Vec<&str> = layout.rows.iter().filter(|row| !row.unfolds()).map(|row| row.name.as_str()).collect();
        assert!(single.iter().any(|name| name.starts_with("Bachelor")), "{single:?}");
        assert!(single.iter().all(|name| !layout.keys().contains(*name)));
        assert!(layout.rows.iter().filter(|row| !row.unfolds()).all(|row| row.subs.is_empty() && row.lanes.len() <= 1));
        assert!(area("Fachstudium").unfolds() && area("Nebenfach").unfolds() && layout.keys().contains("Fachstudium"));
    }

    /// Maschinenbau numbers its electives („Wahlpflichtmodul 1" … „4"): one choice of 24 LP, whose
    /// list is the tree of its area, the plan's rows naming none.
    #[test]
    fn numbered_rows_are_one_choice() {
        use folia_plans::choices::plan_view;

        let source = plans_of("104-82-2021");
        let (shown, core, page) = chosen(&source, None, None);
        let setup = Setup { shown, core, page, start: key("2025W"), start_stored: true, now: key("2026W"), leave: BTreeSet::new(), until: None };
        let doc = PlanDoc::default();
        let input = input_of(&source, &setup, &doc, &source.linked);
        let view = plan_view(&input, &study::study(&input));
        let electives: Vec<_> = view.choices.iter().filter(|choice| choice.name == "Wahlpflichtmodul").collect();
        assert_eq!(electives.len(), 2, "the Fachspezifisches and the Praxisorientiertes Studium each have theirs");
        let first = electives.iter().find(|choice| choice.lines.len() == 4).unwrap();
        assert_eq!((first.least, first.first, first.last), (24.0, 5, 6));
        assert!(!first.lists.is_empty() && first.lists.iter().all(|list| !list.modules.is_empty()));
    }
}

