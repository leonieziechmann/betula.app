//! The Stundenplan (`/studyplan`): the visitor's timetable of the semester the catalog has dates
//! for (owner's redesign of 2026-09-25: the whole study is planned on the program's page).
//!
//! What is planned lives in this browser alone (`crate::studyplan`, R20): the server renders one
//! explanation for every address (R9, cached by the path), and the browser app renders the plan
//! from its store and the local copy of the catalog. The address says only how the plan is shown
//! (`StudyplanUrl`): which semester, which view, the module beside the plan and the Termin it
//! points at, and the Regelstudienplan being taken over; `full=1` lets that module fill the page
//! with its whole page, inside the plan's area (a local view, `crate::local`).
//!
//! The page is one frame (`ui::Frame`, R17): the sidebar „Anpassen" (`side.rs`: the program,
//! `import.rs`, view, what is shown, Standort, calendar, the plan as a whole and the saved plans),
//! the plan in the main column (`head.rs`, `week.rs`, `exams.rs`), and the module beside it
//! (`aside.rs`).
//! Its parts share one `PlanCtx`: the memos below, built once per page. On a phone the sidebar is
//! a sheet, opened by „Anpassen" (`SheetToggle`), and the module beside the plan is the page.
//!
//! The memo graph keeps to R16 (docs/frontend.md): no closure reads a source together with a memo
//! derived from it. The semester shown depends on the plan (the default semester, B.4), so it is
//! computed by the plain function `key_of` in each closure that reads the store (`wanted`,
//! `selection`), never read from the `key` memo there; `data` reads `wanted` alone and takes the
//! planned ids in plan order from it; `table` reads the siblings `data` and `selection` and is
//! `None` while their semesters differ. A change of what is hidden or chosen reaches `selection`
//! and `table` (pure Rust) and asks the catalog nothing; a change of what is planned reaches
//! `wanted` and `data` (the queries of `pages::studyplan`, answered from the visit's cache the
//! second time).

mod aside;
mod exams;
mod export;
mod head;
mod import;
mod side;
mod week;

use catalog::pages::{self, StudyplanData};
use catalog::queries;
use catalog::studyplan::PlanDoc;
use catalog::timetable::clash::Weeks;
use catalog::timetable::day::Day;
use catalog::timetable::model::Timetable;
use catalog::timetable::select::Selection;
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, PlanView, StudyplanUrl};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

use self::aside::PlanModulePanel;
use self::exams::ExamsView;
use self::head::{DerivedLine, ExamAlerts, FromBookmarks, ModuleLegend, Overlaps, SemesterHead};
use self::side::PlanSidebar;
use self::week::{DatesView, WeekView};
use crate::data::{use_source, DataError, Source};
use crate::local::{self, ModuleInPlace};
use crate::myprogram::{MineResolved, MyProgram};
use crate::nav;
use crate::pending::{Change, Pending};
use crate::seo::Seo;
use crate::skeleton::DetailSkeleton;
use crate::studyplan::{PlanAddress, Studyplan};
use crate::tabs::Area;
use crate::ui::{EmptyState, ErrorState, Frame, Icon};

/// The browser app (`csr`), or the server rendering the one explanation it has.
const APP: bool = cfg!(feature = "csr");

/// What the parts of the page share: the plan's address, the semester it shows, what that semester
/// holds and its timetable, the two stores, and the note „Rückgängig" answers.
///
/// A part reads the memos it needs, and a store only where it writes (the memos already follow
/// it): R16 forbids reading a source together with a memo derived from it.
#[derive(Clone, Copy)]
// The views of WP17–WP21 read every part; the stubs of WP16 read none of them yet.
#[allow(dead_code)]
pub(super) struct PlanCtx {
    /// The address without `full` (while a module fills the page, no view is shown).
    pub url: Memo<StudyplanUrl>,
    /// `meta.current_semester` of the local snapshot (a cached answer): „jetzt".
    pub current: Memo<Option<SemesterKey>>,
    /// The semester shown and its planned modules, in the order they were planned.
    pub wanted: Memo<(SemesterKey, Vec<String>)>,
    /// The semester shown (`wanted.0`), for the views.
    pub key: Memo<SemesterKey>,
    /// Everything the semester's timetable is made of (`pages::studyplan`), with the ids in plan
    /// order (`StudyplanData::ids`).
    pub data: Memo<Result<StudyplanData, DataError>>,
    /// The semester and what it hides and has chosen, with the town (`PlanDoc::selection`).
    pub selection: Memo<(SemesterKey, Selection)>,
    /// The semester's timetable as the selection shows it; `None` while `data` and `selection`
    /// are not of the same semester, or the data failed.
    pub table: Memo<Option<Timetable>>,
    pub plan: Option<Studyplan>,
    pub mine: Option<MyProgram>,
    pub source: StoredValue<Option<Source>>,
    /// The browser's date, read once when the page was built; `None` on the server. The page reads
    /// the clock for two things only: the default semester and the week „Termine" scrolls to.
    pub today: Option<Day>,
    /// The note of the last „Übernehmen" or „Plan leeren", and the plan before it („Rückgängig").
    pub undo: RwSignal<Option<(String, PlanDoc)>>,
    /// The week „Woche" shows: the A week, the B week, or both („A/B"). A view setting of the
    /// page, not stored.
    pub weeks: RwSignal<Weeks>,
}

/// The semester the page shows: the one the catalog has dates for, the snapshot's current one
/// (owner's redesign of 2026-09-25: a timetable of the current semester, no other). The address
/// and the plan no longer choose it; they stay in the signature for the sidebar's callers.
pub(super) fn key_of(_url: &StudyplanUrl, current: Option<SemesterKey>, _doc: &PlanDoc, today: Option<Day>) -> SemesterKey {
    // A snapshot always names its current semester; should one not, the date says which half of
    // the year it is. (The browser has a clock, so the last fallback is never shown.)
    current.or_else(|| today.and_then(semester_of)).unwrap_or(SemesterKey { year: 2000, winter: false })
}

/// The semester whose half of the year `day` lies in: April to September is the summer.
fn semester_of(day: Day) -> Option<SemesterKey> {
    let (year, month, _) = day.ymd();
    let year = u16::try_from(year).ok()?;
    match month {
        4..=9 => SemesterKey::new(year, false),
        10..=12 => SemesterKey::new(year, true),
        _ => SemesterKey::new(year.checked_sub(1)?, true),
    }
}

/// The browser's date. `None` on the server, which renders no plan and must not read a clock.
fn today() -> Option<Day> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The catalog as a way to find modules: filtered by „Mein Studiengang" where the stored program is
/// in the snapshot (the one kind of address it may stand in, A.10), else the whole catalog.
/// Tracked.
pub(super) fn catalog_href(resolved: Option<MineResolved>) -> String {
    resolved.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href)
}

/// Where „Modul ansehen" of the module beside the plan leads: the same page, filled with the
/// module's whole page (`full=1`, a local view); its „Zurück" returns here.
pub(super) fn full_href(url: &StudyplanUrl, id: &str) -> String {
    local::full_href(&PlanAddress { url: url.clone(), full: false }, id)
}

#[component]
pub fn StudyplanPage() -> impl IntoView {
    if !APP {
        return server_page().into_any();
    }
    let location = use_location();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let source = StoredValue::new(use_source().ok());
    let today = today();

    let address = Memo::new(move |_| PlanAddress::parse(&location.search.get()));
    let url = Memo::new(move |_| address.with(|address| address.url.clone()));
    let current = Memo::new(move |_| {
        let meta = source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| queries::meta(db)).ok()));
        meta.and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse))
    });
    let wanted = Memo::new(move |_| {
        let (url, current) = (url.get(), current.get());
        match plan {
            Some(plan) => plan.with(|doc| {
                let key = key_of(&url, current, doc, today);
                (key, doc.modules_in(key))
            }),
            None => (key_of(&url, current, &PlanDoc::default(), today), Vec::new()),
        }
    });
    let key = Memo::new(move |_| wanted.with(|wanted| wanted.0));
    let data = Memo::new(move |_| {
        let (key, ids) = wanted.get();
        source.with_value(|source| match source {
            Some(source) => source.run(|db| pages::studyplan(db, key, &ids)),
            None => Err(DataError { unavailable: true, message: "no data source was provided".to_string() }),
        })
    });
    let selection = Memo::new(move |_| {
        let (url, current) = (url.get(), current.get());
        let town = mine.map(MyProgram::town).unwrap_or_default();
        match plan {
            Some(plan) => plan.with(|doc| {
                let key = key_of(&url, current, doc, today);
                (key, doc.selection(key, town))
            }),
            None => (key_of(&url, current, &PlanDoc::default(), today), Selection { town, ..Default::default() }),
        }
    });
    let table = Memo::new(move |_| {
        let (key, selection) = selection.get();
        data.with(|data| data.as_ref().ok().filter(|data| data.key == key).map(|data| data.timetable(&selection)))
    });
    let ctx = PlanCtx {
        url,
        current,
        wanted,
        key,
        data,
        selection,
        table,
        plan,
        mine,
        source,
        today,
        undo: RwSignal::new(None),
        weeks: RwSignal::new(Weeks::All),
    };

    // What fills the page: the plan, or the module after „Vollbild" (on a phone as well: the
    // module beside the plan is the plan's panel of it, which is the page there anyway).
    let filling = Memo::new(move |_| address.with(|address| local::filling(address, false)));
    let empty = Memo::new(move |_| plan.is_none_or(Studyplan::is_empty));

    // The module beside the plan, and a pick on its way there (`pending`): the panel follows the
    // click in the next frame, there at once for a pick and gone at once for a close.
    let going = Pending::expect();
    let target = Memo::new(move |_| {
        let going = going.filter(|going| going.change() == Some(Change::Aside))?;
        Some(StudyplanUrl::parse(&going.search_on(url::STUDYPLAN)?))
    });
    let open = Memo::new(move |_| url.with(|url| url.open.is_some()));
    let picked = Signal::derive(move || {
        let shown = match target.get() {
            Some(to) => to.open.is_some(),
            None => open.get(),
        };
        shown && !empty.get()
    });
    let aside = move || {
        let there = open.get();
        let coming = target.with(|to| to.as_ref().is_some_and(|to| to.open.is_some()));
        if coming && (!there || going.is_some_and(|going| going.waits(Change::Aside))) {
            return view! { <DetailSkeleton aside=true calm=there/> }.into_any();
        }
        view! { <PlanModulePanel ctx/> }.into_any()
    };

    let page = move || {
        if let Some(id) = filling.get() {
            let back = address.with_untracked(|address| local::back_href(address, false));
            return view! { <ModuleInPlace id area=Area::Studyplan back/> }.into_any();
        }
        view! {
            <Title text=TITLE/>
            <Frame title="Anpassen" sheet=true sidebar=move || view! { <PlanSidebar ctx/><StorageHint/> } aside aside_picked=picked>
                <PlanSeo/>
                <div class="page-inner sp">
                    <section class="panel sp-body">
                        {move || match empty.get() {
                            true => view! { <EmptyPlan/> }.into_any(),
                            false => view! { <SemesterView ctx/> }.into_any(),
                        }}
                    </section>
                </div>
            </Frame>
        }
        .into_any()
    };
    page.into_any()
}

/// The semester: its head, and, where anything is planned into it, the modules, the exams that
/// collide (red, in every view), the one line of overlaps and open choices, the view, and under
/// it the line that says what is derived.
#[component]
fn SemesterView(ctx: PlanCtx) -> impl IntoView {
    let planned = Memo::new(move |_| ctx.wanted.with(|wanted| !wanted.1.is_empty()));
    let failed = Memo::new(move |_| ctx.data.with(|data| data.as_ref().err().cloned()));
    let shown = Memo::new(move |_| ctx.url.with(|url| url.view));
    view! {
        <SemesterHead ctx/>
        {move || match (failed.get(), planned.get()) {
            (Some(error), _) => view! { <ErrorState error/> }.into_any(),
            (None, false) => view! { <FromBookmarks ctx/> }.into_any(),
            (None, true) => view! {
                <ModuleLegend ctx/>
                <ExamAlerts ctx/>
                <Overlaps ctx/>
                <FromBookmarks ctx/>
                {move || match shown.get() {
                    PlanView::Dates => view! { <DatesView ctx/> }.into_any(),
                    PlanView::Exams => view! { <ExamsView ctx/> }.into_any(),
                    PlanView::Week | PlanView::Overview => view! { <WeekView ctx/> }.into_any(),
                }}
                <DerivedLine ctx/>
            }
            .into_any(),
        }}
    }
}

/// Nothing planned yet: the two ways to begin, and on a phone the way to the sidebar.
#[component]
fn EmptyPlan() -> impl IntoView {
    let resolved = MineResolved::expect();
    view! {
        <EmptyState title="Noch nichts geplant." hint="Übernimm den Regelstudienplan oder plane Module aus dem Katalog ein.">
            // „Importieren" in the sidebar: on a phone the sheet opens, on a desktop „Übernehmen"
            // takes the focus.
            <a class="btn primary" href="#sidebar" data-action="sheet-open" on:click=|_| nav::focus_by_id(import::GO_ID)>"Regelstudienplan übernehmen"</a>
            <a class="btn secondary" href=move || catalog_href(resolved)>"Module suchen"</a>
            <SheetToggle/>
        </EmptyState>
    }
}

/// „Anpassen": opens the sidebar, which on a phone is a sheet from below. Shown on a phone only
/// (`.sheet-toggle`); the heads of the views carry it at their right end, the empty plan among its
/// actions, and the page itself while an empty plan takes a Regelstudienplan over.
#[component]
pub(super) fn SheetToggle() -> impl IntoView {
    view! { <a class="sheet-toggle" href="#sidebar" data-action="sheet-open"><Icon name="sliders-horizontal"/>"Anpassen"</a> }
}

/// The last group of the sidebar, on the server as in the app: where the plan lives.
#[component]
pub(super) fn StorageHint() -> impl IntoView {
    view! {
        <div class="fgroup">
            <p class="hint storage-hint"><Icon name="shield-check"/><span>"Dein Studienplan liegt nur in diesem Browser."</span></p>
        </div>
    }
}

/// A page of one visitor: the same address and the same explanation for everybody, for no index.
#[component]
fn PlanSeo() -> impl IntoView {
    view! {
        <Seo title=TITLE description="Dein Stundenplan: Termine, Prüfungen und Kalender-Abo der geplanten Module." path=url::STUDYPLAN noindex=true/>
    }
}

/// What the server renders for every address of the plan (R9): the frame, and the explanation in
/// the place of the plan. The app replaces it with the plan once it runs.
///
/// The frame is the app's, a sheet on a phone included: a sidebar in the page that the takeover
/// turns into a closed sheet would vanish from under the explanation (R15). The sidebar says where
/// the plan lives (`StorageHint`, as in the app), the explanation what it takes to see it; each
/// says it once (owner review 2026-09-25).
fn server_page() -> impl IntoView {
    view! {
        <Title text=TITLE/>
        <Frame title="Anpassen" sheet=true sidebar=|| view! { <StorageHint/> }>
            <PlanSeo/>
            <div class="page-inner sp">
                <section class="panel sp-body">
                    <EmptyState title=SERVER_TITLE hint="Dafür braucht es JavaScript."/>
                </section>
            </div>
        </Frame>
    }
}

/// The page's name (owner's redesign of 2026-09-25): a timetable of one semester.
const TITLE: &str = "Stundenplan";

/// What the server's page says in the place of the plan.
const SERVER_TITLE: &str = "Dein Stundenplan erscheint, sobald die App geladen ist.";

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn the_page_shows_the_current_semester_alone() {
        let url = |query: &str| StudyplanUrl::parse(query);
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        let march = Day::from_ymd(2027, 3, 1);
        // Neither the address nor a plan of another semester moves it.
        assert_eq!(key_of(&url("sem=2027W"), Some(key("2026W")), &doc, march), key("2026W"));
        assert_eq!(key_of(&url(""), Some(key("2027S")), &doc, march), key("2027S"));
        // Without the snapshot's word the date decides the half of the year.
        assert_eq!(key_of(&url(""), None, &PlanDoc::default(), Day::from_ymd(2026, 11, 5)), key("2026W"));
        assert_eq!(key_of(&url(""), None, &PlanDoc::default(), Day::from_ymd(2027, 2, 5)), key("2026W"));
        assert_eq!(key_of(&url(""), None, &PlanDoc::default(), Day::from_ymd(2027, 5, 5)), key("2027S"));
    }

    #[test]
    fn modul_ansehen_fills_the_plan_with_the_module() {
        let url = StudyplanUrl::parse("sem=2026W&view=dates&open=12104&row=148369-aaf38");
        assert_eq!(full_href(&url, "12104"), "/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38&full=1");
        assert_eq!(full_href(&url, "12107"), "/studyplan?sem=2026W&view=dates&open=12107&full=1");
    }
}
