//! „Mein Studium" (`/study`; owner, 2026-10-04: „Primär soll man da sein Studium planen können. Also
//! das aktuelle und zukünftige Semester"): the page the Studium tab leads to in the app. The
//! visitor's study semester by semester, as `folia_plans::study` works it out from „Mein
//! Studiengang" (its program, study direction and Studienbeginn), the Regelstudienplan and what is
//! passed and placed (`folia_stores::studyplan`): from the current semester on what is left over
//! first („nachholen"), then what is due, then what the student planned besides; the semesters
//! before it folded, to tick off what was passed.
//!
//! The page is the app's (R9, R20): the server writes the same stand-in for everybody, and nothing
//! of it reaches an address but the module beside it (`StudyUrl`, a local view). It is framed
//! (R17): the sidebar (`side.rs`) holds the program, its study direction, the Studienbeginn, the
//! ways to the program's pages and the timetable, and the legend; the page (`terms.rs`) the head
//! with the progress, the semesters before the current one and the semesters from it on. A module
//! opened from it stands beside the page (`?open=`) and fills it after „Vollbild"; on a phone it
//! is the page.
//!
//! What it shows is one memo (`State`), worked out anew when the store, „Mein Studiengang", the
//! program's plans or the current semester change. It reads the stores (signals) and the memos of
//! the catalog's answers, never a memo together with the source it is derived from (R16). Each part
//! of the page reads what it needs of it through a memo of its own (R5), so a row ticked off changes
//! that row and its semester's numbers, keeps its place and the focus. A click answers in the next
//! frame and the store follows after it (R21, `StudyCtx::change`).

mod side;
mod terms;

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Program};
use folia_pages::ask::{MetaAsk, ModuleAsk, PlanSourceAsk, StudyplanModulesAsk};
use folia_pages::PlanSource;
use folia_plans::study::{self, Input, Study};
use folia_plans::studyplan::{self as stored, PlanDoc};
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

use crate::i18n::{self, use_location};

/// The browser app (`csr`), or the server writing the page's stand-in.
const APP: bool = cfg!(feature = "csr");

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

/// Where the study stands: what `study::study` was asked with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Setup {
    /// The plan shown (an index into the program's plans): a core plan or a page of one.
    pub shown: Option<usize>,
    /// The core plan, and the page that fills a row of it with that row's `ord`.
    pub core: Option<usize>,
    pub page: Option<(usize, i64)>,
    pub start: SemesterKey,
    /// The Studienbeginn is stored; else it is assumed (`studyplan::intake_start`).
    pub start_stored: bool,
    /// The current semester.
    pub now: SemesterKey,
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum State {
    /// The app is starting, or the answers are on their way.
    Waiting,
    /// No program is kept: the page asks for one.
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ready {
    pub program: Program,
    /// The program's plans by their labels, where there are two or more.
    pub plans: Vec<(usize, String)>,
    pub setup: Setup,
    pub study: Study,
    /// What „In den Stundenplan" adds to the current semester's timetable: modules, placeholders.
    pub to_take: (usize, usize),
    /// The modules in the current semester's timetable, and the plan's rows it holds a placeholder of.
    pub in_timetable: BTreeSet<String>,
    pub rows_in_timetable: BTreeSet<i64>,
}

/// What the parts of the page share.
#[derive(Clone, Copy)]
pub(super) struct StudyCtx {
    /// The address: the module beside the page.
    pub url: Memo<StudyUrl>,
    pub state: Memo<State>,
    pub plans: Memo<Plans>,
    pub plan: Option<Studyplan>,
    pub mine: Option<MyProgram>,
    pub source: StoredValue<Option<DataClient>>,
    /// The phone's layout: the sidebar is a sheet there, a module opened is the page.
    pub phone: RwSignal<bool>,
    /// The note of the last „In den Stundenplan" and the plan before it („Rückgängig").
    pub undo: RwSignal<Option<(String, PlanDoc)>>,
}

impl StudyCtx {
    /// Changes the plan after the next frame (R21), then runs `then`: `change` gets the plan and
    /// what the study is worked out from as it stands then (the plan's modules with their turnus;
    /// a module of one's own is placed where it is planned whatever its turnus). Nothing changes
    /// while the study is not there.
    pub fn change(self, change: impl FnOnce(&mut PlanDoc, &Input) + 'static, then: impl FnOnce() + 'static) {
        let Some(plan) = self.plan else {
            then();
            return;
        };
        let (plans, state) = (self.plans, self.state);
        nav::after_paint(move || {
            plan.update(|doc| {
                let Some(setup) = state.with_untracked(|state| state.ready().map(|ready| ready.setup)) else { return };
                let before = doc.clone();
                plans.with_untracked(|plans| {
                    let Plans::Found(source) = plans else { return };
                    let input = input_of(source, setup, &before, &source.linked);
                    change(doc, &input);
                });
            });
            then();
        });
    }
}

/// What the study is worked out from: the program's plans as `setup` chose them, the plan, and the
/// catalog's rows of its modules.
pub(super) fn input_of<'a>(source: &'a PlanSource, setup: Setup, doc: &'a PlanDoc, rows: &'a [CatalogRow]) -> Input<'a> {
    Input {
        program_id: &source.program.id,
        core: setup.core.and_then(|core| source.variants.get(core)),
        page: setup.page.and_then(|(page, ord)| Some((source.variants.get(page)?, ord))),
        start: setup.start,
        now: setup.now,
        doc,
        rows,
    }
}

/// The plan „Mein Studiengang" keeps among the program's plans (by its caption and direction, as
/// `ProgramPlans` reads them), else the first; with its core plan and the page of it.
fn chosen(source: &PlanSource, caption: Option<&str>, direction: Option<&str>) -> (Option<usize>, Option<usize>, Option<(usize, i64)>) {
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

/// What the page shows of a program's study.
#[allow(clippy::too_many_arguments)]
fn ready(source: &PlanSource, caption: Option<&str>, direction: Option<&str>, start: Option<SemesterKey>, now: SemesterKey, doc: &PlanDoc, extra: &[CatalogRow]) -> Ready {
    let (shown, core, page) = chosen(source, caption, direction);
    let assumed = || {
        let intake = core.and_then(|core| source.variants.get(core)).and_then(|core| stored::intake_season(core, &source.linked));
        stored::intake_start(now, intake)
    };
    let setup = Setup { shown, core, page, start: start.unwrap_or_else(assumed), start_stored: start.is_some(), now };
    let rows: Vec<CatalogRow> = source.linked.iter().chain(extra.iter().filter(|row| !source.linked.iter().any(|linked| linked.id == row.id))).cloned().collect();
    let input = input_of(source, setup, doc, &rows);
    let study = study::study(&input);
    // The timetable is the current semester's; a study that begins later has none yet.
    let to_take = study.term(now).filter(|_| study.now == now).map(|term| study::timetable_import(&input, term)).map_or((0, 0), |import| (import.modules.len(), import.placeholders.len()));
    let plans = match source.variants.len() {
        0 | 1 => Vec::new(),
        _ => source.variants.iter().enumerate().map(|(index, plan)| (index, plan.label.clone())).collect(),
    };
    let rows_in_timetable = doc.placeholders_in(now).into_iter().filter(|p| p.program_id == source.program.id).map(|p| p.ord).collect();
    Ready { program: source.program.clone(), plans, setup, study, to_take, in_timetable: doc.modules_in(now).into_iter().collect(), rows_in_timetable }
}

/// What „Mein Studium" takes into the timetable of semester `now` (the Stundenplan's „Importieren"
/// from „Mein Studium"): the study of the program „Mein Studiengang" keeps (`mine`, whose plans
/// `source` are), and of it the semester `now` — what is left over and what is due there, not yet
/// in the timetable (`study::timetable_import`). `None` where the study does not reach `now` (a
/// Studienbeginn after it).
pub(crate) fn current_import(source: &PlanSource, mine: &stored::MineDoc, doc: &PlanDoc, now: SemesterKey) -> Option<stored::Import> {
    let ready = ready(source, mine.caption.as_deref(), mine.direction.as_deref(), mine.start, now, doc, &[]);
    let input = input_of(source, ready.setup, doc, &source.linked);
    let term = ready.study.term(now).filter(|_| ready.study.now == now)?;
    Some(study::timetable_import(&input, term))
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
    // The catalog's rows of what is planned and passed, for the modules the plan does not name.
    let ids = Memo::new(move |_| {
        let mut ids: Vec<String> = plan.map(|plan| plan.with(|doc| doc.modules.iter().map(|m| m.module_id.clone()).chain(doc.passed.iter().map(|p| p.module_id.clone())).collect())).unwrap_or_default();
        ids.sort();
        ids.dedup();
        ids
    });
    let extra = Memo::new(move |before: Option<&Vec<CatalogRow>>| {
        let ids = ids.get();
        if ids.is_empty() {
            return Vec::new();
        }
        let Some(now) = source.with_value(|source| source.as_ref().map(|source| source.now(&StudyplanModulesAsk { ids }))) else { return Vec::new() };
        folia_pages::ask::unless_pending(now, before, |now| now.map(|(rows, _)| rows).unwrap_or_default())
    });
    let state = Memo::new(move |_| {
        let now = crate::studyplan::key_of(&Default::default(), current.get(), &PlanDoc::default(), today);
        let kept = mine.map(|mine| mine.with(|doc| (doc.caption.clone(), doc.direction.clone(), doc.start, doc.name.clone()))).unwrap_or_default();
        let (caption, direction, start, name) = kept;
        plans.with(|plans| match plans {
            Plans::None => State::NoProgram,
            Plans::Waiting => State::Waiting,
            Plans::Gone => State::Gone(name.unwrap_or_default()),
            Plans::Found(source) => {
                let doc = plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
                extra.with(|extra| State::Ready(Box::new(ready(source, caption.as_deref(), direction.as_deref(), start, now, &doc, extra))))
            }
        })
    });
    let ctx = StudyCtx { url, state, plans, plan, mine, source, phone: phone_layout(), undo: RwSignal::new(None) };

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
                    <terms::StudyMain ctx/>
                </div>
            </Frame>
        }
        .into_any()
    })
    .into_any()
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

/// „Anpassen": opens the sidebar, which on a phone is a sheet from below (`.sheet-toggle` shows
/// on a phone only).
#[component]
pub(super) fn SheetToggle() -> impl IntoView {
    let t = i18n::t();
    view! { <a class="sheet-toggle" href="#sidebar" data-action="sheet-open"><Icon name="sliders-horizontal"/>{t.studyplan.customise}</a> }
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

    /// Informatik B.Sc. in its third semester: what is left over of the first two comes first, the
    /// Studienbeginn is assumed until one is stored, and the timetable is offered what is open.
    #[test]
    fn mein_studium_of_informatik() {
        let source = plans_of("079-82-2008");
        let doc = PlanDoc::default();
        let assumed = ready(&source, None, None, None, key("2026W"), &doc, &[]);
        assert_eq!((assumed.setup.start, assumed.setup.start_stored), (key("2026W"), false), "a winter intake starts in the current winter");
        assert!(assumed.study.past.is_empty());
        let third = ready(&source, Some(""), None, Some(key("2025W")), key("2026W"), &doc, &[]);
        assert!(third.setup.start_stored);
        assert_eq!(third.study.past.len(), 2);
        let now = third.study.term(key("2026W")).unwrap();
        assert!(matches!(now.items.first().map(|item| item.place), Some(study::Place::Overdue { .. })), "what is left over comes first");
        assert_eq!(third.to_take.0, now.open().filter(|item| item.module_id().is_some()).count());
        assert!(third.in_timetable.is_empty());
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
        let ready = ready(&source, Some(&second), None, Some(key("2026W")), key("2026W"), &PlanDoc::default(), &[]);
        assert_eq!(ready.plans.len(), source.variants.len());
        assert_eq!(ready.setup.shown, Some(1));
    }
}
