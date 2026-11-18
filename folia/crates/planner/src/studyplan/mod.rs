//! The Stundenplan (`/studyplan`): the visitor's timetable of the semester the catalog has dates
//! for (owner's redesign of 2026-09-25: the whole study is planned on the program's page).
//!
//! What is planned lives in this browser alone (`folia_stores::studyplan`, R20): the server renders one
//! explanation for every address (R9, cached by the path), and the browser app renders the plan
//! from its store and the local copy of the catalog. The address says only how the plan is shown
//! (`StudyplanUrl`): which semester, which view, the module beside the plan and the Termin it
//! points at (`import=` and `variant=`, of the Regelstudienplan the sidebar took over until
//! 2026-11-04, are read past); `full=1` lets that module fill the page
//! with its whole page, inside the plan's area (a local view, `folia_widgets::local`).
//!
//! The page is one frame (`ui::Frame`, R17): the sidebar „Stundenplan" (`side.rs`: the calendars,
//! „Export", „Teilen …", and while editing the plans saved before),
//! the plan in the main column (`head.rs`, `week.rs`, `exams.rs`) with its modules in a column
//! beside it where the page is wide enough, else under it (`modules.rs`), and the module opened
//! from it floating over the page at its right edge, as the catalog's preview does (`aside.rs`).
//! The week is there before anything is planned (owner, 2026-09-25), with what to do in its middle.
//! The page is looked at or edited (`edit.rs`, `PlanCtx::edit`): only the mode adds the page's own
//! controls. Its parts share one `PlanCtx`: the memos below, built once per page. On a phone there
//! is no sidebar: the calendars and „Export" with „Teilen …" are sheets the head's toolbar opens
//! (`head::PhoneSheets`), the mode is a dock in the tab bar's place, and the module beside the plan
//! is the page.
//!
//! The memo graph keeps to R16 (docs/folia/frontend.md): no closure reads a source together with a memo
//! derived from it. The semester shown depends on the plan (the default semester, B.4), so it is
//! computed by the plain function `key_of` in each closure that reads the store (`wanted`,
//! `selection`), never read from the `key` memo there; `data` reads `wanted` alone and takes the
//! planned ids in plan order from it; `table` reads the siblings `data` and `selection` and is
//! `None` while their semesters differ. A change of what is hidden or chosen reaches `selection`
//! and `table` (pure Rust) and asks the catalog nothing; a change of what is planned reaches
//! `wanted` and `data` (the queries of `pages::studyplan`, answered from the visit's cache the
//! second time).

mod add;
mod aside;
mod calendars;
mod edit;
mod exams;
mod export;
mod head;
mod modules;
mod open;
mod own;
// The server lays out no picture (`picture::save` is the browser's).
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
mod picture;
mod share;
mod share_dialog;
mod side;
mod termin;
mod week;

use folia_calendar::day::Day;
use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::{self as shared_plan, SharedPlan};
use folia_pages::ask::{MetaAsk, SharedPlanAsk, StudyplanAsk};
use folia_pages::StudyplanData;
use folia_plans::studyplan::{PlanDoc, PlanUndo};
use folia_routes::url::{self, PlanView, StudyplanUrl};
use folia_timetable::clash::Weeks;
use folia_timetable::model::Timetable;
use leptos::prelude::*;
use folia_design::stage::Title;

use folia_data::{use_data, DataClient, DataError};
use crate::i18n::{self, use_location};
use folia_widgets::local::ModuleInPlace;
use folia_stores::myprogram::MyProgram;
use folia_widgets::list::phone_layout;
use folia_shell::pending::{Change, Pending, Shape};
use folia_shell::seo::Seo;
use folia_shell::skeleton::{AppStandin, DetailSkeleton};
use folia_stores::studyplan::Studyplan;
use folia_routes::url::PlanAddress;
use folia_shell::tabs::Area;
use folia_shell::frame::{ErrorState, Frame};
use folia_design::ui::Icon;
use self::aside::PlanModulePanel;
use self::calendars::CalendarModules;
use self::edit::EditDock;
use self::exams::ExamsView;
use self::head::{marked_offered, DerivedLine, ExamAlerts, NothingPlanned, Overlaps, PhoneBand, PhoneSheets, SemesterHead};
use self::modules::ModuleList;
use self::open::{OpenPanel, PickSheet};
use self::own::{OwnForm, OwnNote};
use self::side::{PlanSidebar, SavedPlans};
use self::termin::{PlanTerminFull, PlanTerminModal};
use self::week::{DatesView, WeekLoose, WeekView};

/// Seconds since 1970, for when a module is planned (here and in „Mein Studium"); 0 outside the
/// browser.
pub(crate) fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    {
        let millis = web_sys::js_sys::Date::now();
        if millis.is_finite() && millis > 0.0 {
            // Whole seconds of a positive, finite number of milliseconds.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            return (millis / 1000.0) as u64;
        }
    }
    0
}

/// The browser app (`csr`), or the server rendering the one explanation it has.
const APP: bool = cfg!(feature = "csr");

/// What the parts of the page share: the plan's address, the semester it shows, what that semester
/// holds and its timetable, the two stores, and the note „Rückgängig" answers.
///
/// A part reads the memos it needs, and a store only where it writes (the memos already follow
/// it): R16 forbids reading a source together with a memo derived from it.
#[derive(Clone, Copy)]
pub(super) struct PlanCtx {
    /// The address without `full` (while a module fills the page, no view is shown).
    pub url: Memo<StudyplanUrl>,
    /// `meta.current_semester` of the local snapshot (a cached answer): „jetzt".
    pub current: Memo<Option<SemesterKey>>,
    /// The semester shown, the modules of its week (the planned ones in the order they were
    /// planned, then those only visited, `PlanDoc::week_in`), and the program the timetable is for
    /// (the plan's, else „Mein Studiengang"), whose abbreviations the week grid names the modules
    /// by.
    pub wanted: Memo<(SemesterKey, Vec<String>, Option<String>)>,
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
    pub source: StoredValue<Option<DataClient>>,
    /// The browser's date, read once when the page was built; `None` on the server. The page reads
    /// the clock for two things only: the default semester and the week „Termine" scrolls to.
    pub today: Option<Day>,
    /// The note of the last change that answers with „Rückgängig" (a share taken in, a Termin of
    /// one's own deleted, a calendar or a module of one taken out, „Alle einblenden", what a
    /// Termin's card did), and what „Rückgängig" takes back (`PlanUndo`: the change alone, not what
    /// another tab or device changed meanwhile).
    pub undo: RwSignal<Option<(String, PlanUndo)>>,
    /// The week „Woche" shows: the A week, the B week, or both („A/B"). A view setting of the
    /// page, not stored.
    pub weeks: RwSignal<Weeks>,
    /// The plan is being edited (owner, 2026-11-17: „bei dem Kalender einen edit modus hinzuzufügen, weil
    /// man dadurch unnötiges ui ausblenden kann, und dann im edit modus auch Termine leicht anzeigen kann,
    /// die man ausgeblendet hat"): the address says `edit=1` or holds a choice in focus (`wahl`), read of
    /// the address the app is going to while a step is on its way (`Pending`), so that the head's button,
    /// the status row and what the mode reveals answer in the frame of the click (R21). Made of `url`: no
    /// closure reads it and `url` (R16). The week's slots follow the address itself (`week::base_of`).
    pub edit: Memo<bool>,
    /// „Offen"'s rows stand unfolded over the week (a page without the column beside it): set by the
    /// way „3 Wahlen offen ›" into the mode, flipped by the bar's figure, off again once the mode is
    /// left. A view setting of the page, not stored.
    pub unfolded: RwSignal<bool>,
    /// The overlaps' list stands unfolded under the status row: flipped by their figure there and in
    /// the mode's bar (`head::ClashFigure`), never by what the plan holds. A view setting of the
    /// page, not stored.
    pub clashes: RwSignal<bool>,
    /// The phone's layout is in use (`nav::is_phone`, following the window): there the week is a
    /// carousel of its weeks over the list of its days, and the calendar, „Export" and „Teilen …"
    /// stand in the page instead of in a sidebar.
    pub phone: RwSignal<bool>,
    /// On a phone the list of the week's days under its grid is open (owner, 2026-09-27: closed
    /// until asked for). A view setting of the page, not stored; it outlasts the module opened
    /// from the list, so that closing the module finds its row.
    pub days: RwSignal<bool>,
    /// The calendars of one's own of the semester shown, each with its timetable (`calendars.rs`;
    /// owner, 2026-10-26: „Kallender, die die nutzer selbst erstellen können").
    pub calendars: Memo<Vec<calendars::Shown>>,
    /// „Gemeinsam frei": the week draws the time every calendar in it has free. A view setting of
    /// the page, off until switched on, not stored.
    pub together: RwSignal<bool>,
    /// The calendar whose „+ Modul" is open (`add.rs`). A view setting of the page, not stored.
    pub adding: RwSignal<Option<add::AddTo>>,
    /// The form of a Termin of one's own while it is open, and what its fields hold (`own.rs`;
    /// folia/design/kalender/termine.html). Not stored.
    pub own: RwSignal<Option<own::OwnAsk>>,
    /// The dialog „Teilen" while it is open: for which calendar, on which side
    /// (`share_dialog.rs`; folia/design/teilen/teilen.html). Not stored.
    pub sharing: RwSignal<Option<share_dialog::ShareOpen>>,
    /// The span a drag draws on a free stretch of the week while it lasts (`own::Drag`), drawn as
    /// the form's draft is. Not stored.
    pub sketch: RwSignal<Option<own::Sketch>>,
    /// The sheet a phone's head opened: the calendars or „Export" (`head::PhoneSheets`). Not stored.
    pub sheet: RwSignal<Option<head::PhoneSheet>>,
    /// Where the keyboard goes once „Rückgängig" put back the Termin of one's own a deletion took:
    /// the slot it stood on, as a selector of the week (`own::remove`). Not stored.
    pub back_to: StoredValue<Option<String>>,
    /// When the mode was left last: „3 Wahlen offen ›" comes where „Fertig" was clicked, and takes no
    /// click of the pointer for a moment (`folia_design::nav::Newborn`; `edit.rs` starts it).
    pub left: folia_design::nav::Newborn,
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

/// The host a picture names as where it is from („betula.app"), as the browser has it.
pub(crate) fn site_host() -> String {
    export::site().map_or_else(|| "betula.app".to_string(), |(_, host)| host)
}

/// The browser's date. `None` on the server, which renders no plan and must not read a clock.
pub(crate) fn today() -> Option<Day> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Where „Modul ansehen" of the module beside the plan leads: the same page, filled with the
/// module's whole page (`full=1`, a local view); its „Zurück" returns here.
pub(super) fn full_href(url: &StudyplanUrl, id: &str) -> String {
    folia_routes::local::full_href(&PlanAddress { url: url.clone(), full: false }, id)
}

/// Where „Zurück" leads from a module that fills the page, as the app's path (`ModuleInPlace` writes
/// it as a link of the page's language): the plan with the module beside it again where it is one of
/// the study's week (`ours`), else the plan without it — a calendar's module, which the panel would
/// only call „Nicht im Studienplan.".
fn back_of(address: &PlanAddress, ours: bool) -> String {
    match ours {
        true => folia_routes::local::back_href(address),
        false => address.url.with_open(None, None).path(),
    }
}

#[component]
pub fn StudyplanPage() -> impl IntoView {
    if !APP {
        return server_page().into_any();
    }
    let t = i18n::t();
    let location = use_location();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let source = StoredValue::new(use_data().ok());
    let today = today();

    let address = Memo::new(move |_| PlanAddress::parse(&location.search.get()));
    let url = Memo::new(move |_| address.with(|address| address.url.clone()));
    let current = Memo::new(move |before| {
        let now = source.with_value(|source| source.as_ref().map(|source| source.now(&MetaAsk {})))?;
        folia_pages::ask::unless_pending(now, before, |meta| meta.ok().and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse)))
    });
    let wanted = Memo::new(move |_| {
        let (url, current) = (url.get(), current.get());
        match plan {
            Some(plan) => plan.with(|doc| {
                let key = key_of(&url, current, doc, today);
                let program = doc.program.clone().or_else(|| mine.and_then(|mine| mine.with(|mine| mine.program.clone())));
                (key, doc.week_in(key), program)
            }),
            None => (key_of(&url, current, &PlanDoc::default(), today), Vec::new(), None),
        }
    });
    let key = Memo::new(move |_| wanted.with(|wanted| wanted.0));
    let data = Memo::new(move |_| {
        let (key, ids, program) = wanted.get();
        source.with_value(|source| match source {
            Some(source) => source.now(&StudyplanAsk { key, ids: ids.clone(), program: program.clone(), locale: t.locale }),
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
    // Where the app is going: the mode follows a click on its way (`PlanCtx::edit`), the module
    // beside the plan as well (`target`, below).
    let going = Pending::expect();
    let edit = Memo::new(move |_| match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
        Some(search) => edit::on(&StudyplanUrl::parse(&search)),
        None => url.with(edit::on),
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
        edit,
        unfolded: RwSignal::new(false),
        clashes: RwSignal::new(false),
        phone: phone_layout(),
        days: RwSignal::new(false),
        calendars: calendars::shown_calendars(url, current, plan, source, today),
        together: RwSignal::new(false),
        adding: RwSignal::new(None),
        own: RwSignal::new(None),
        sharing: RwSignal::new(None),
        sketch: RwSignal::new(None),
        sheet: RwSignal::new(None),
        back_to: StoredValue::new(None),
        left: folia_design::nav::Newborn::grown(),
    };
    // What „Rückgängig" puts back is the plan of the profile it was taken from: with another
    // profile (chosen here or in another tab, `folia_stores::profiles`) there is nothing to undo.
    Effect::new(move |before: Option<u16>| {
        let profile = plan.map_or(0, Studyplan::profile);
        if before.is_some_and(|before| before != profile) {
            ctx.undo.set(None);
        }
        profile
    });
    // „Offen" folds again once the mode is left: the next way in decides anew.
    Effect::new(move |_| {
        if !edit.get() {
            ctx.unfolded.set(false);
        }
    });
    // A sheet of a phone's head closes once the page is no longer a phone's: the sidebar holds what
    // it held, and it must not come back with the next phone.
    Effect::new(move |_| {
        if !ctx.phone.get() && ctx.sheet.with_untracked(Option::is_some) {
            ctx.sheet.set(None);
        }
    });

    // What fills the page: the plan, or the module after „Vollbild" (until then the plan's panel of
    // it stands beside the plan, on a phone a sheet over it), or a Termin's overview (`full=1`).
    let filling = Memo::new(move |_| address.with(folia_routes::local::filling));
    let termin_full = Memo::new(move |_| address.with(PlanAddress::termin_full));
    // A page of its own opened from the plan (the overview, a module's whole page) opens at its
    // top; leaving it finds the plan where it was left, with the keyboard on what opened it
    // (`left`).
    #[cfg(feature = "csr")]
    {
        let stop = left::watch();
        on_cleanup(stop);
        let filled = Memo::new(move |_| address.with(|address| address.full));
        Effect::new(move |was: Option<bool>| {
            let now = filled.get();
            if was == Some(true) && !now {
                left::restore(&location.search.get_untracked());
            }
            now
        });
    }
    let empty = Memo::new(move |_| plan.is_none_or(Studyplan::is_empty));

    // The module beside the plan, and a pick on its way there (`pending`): the panel follows the
    // click in the next frame, there at once for a pick and gone at once for a close.
    let target = Memo::new(move |_| {
        let going = going.filter(|going| going.change() == Some(Change::Aside))?;
        Some(StudyplanUrl::parse(&going.search_on(url::STUDYPLAN)?))
    });
    // Beside the plan a module (`open`); a Termin in its own view (`termin`) stands over the plan in
    // the app's dialog (`PlanTerminModal`), unless it fills the page.
    let beside = |url: &StudyplanUrl| url.open.is_some();
    let open = Memo::new(move |_| url.with(beside));
    let termin = Memo::new(move |_| url.with(|url| url.termin.is_some()));
    let picked = Signal::derive(move || {
        let shown = match target.get() {
            Some(to) => beside(&to),
            None => open.get(),
        };
        shown && !empty.get()
    });
    // The module beside the plan by its id: entering or leaving the mode with a module beside the
    // plan is a step beside the same module, which shows no skeleton.
    let open_id = Memo::new(move |_| url.with(|url| url.open.clone()));
    let aside = move || {
        let there = open.get();
        let coming = target.with(|to| to.as_ref().is_some_and(beside));
        let other = target.with(|to| to.as_ref().is_some_and(|to| open_id.with(|id| to.open != *id)));
        if coming && (!there || (other && going.is_some_and(|going| going.waits(Change::Aside)))) {
            return view! { <DetailSkeleton aside=true calm=there module=true/> }.into_any();
        }
        view! { <PlanModulePanel ctx/> }.into_any()
    };

    let page = move || {
        if termin_full.get() {
            return view! { <PlanTerminFull ctx/> }.into_any();
        }
        if let Some(id) = filling.get() {
            let ours = wanted.with_untracked(|wanted| wanted.1.contains(&id));
            let back = address.with_untracked(|address| back_of(address, ours));
            return view! { <ModuleInPlace id area=Area::Studyplan back/> }.into_any();
        }
        view! {
            <Title text=t.studyplan.title/>
            // The sidebar „Stundenplan" beside the page; a phone has its groups in the page and,
            // after it, the storage hint and the saved plans (`side.rs`).
            <Frame title=t.studyplan.title sidebar=move || view! { <PlanSidebar ctx/><StorageHint/><SavedPlans ctx/> } aside aside_picked=picked>
                <PlanSeo/>
                // The page in the mode: what it reveals and hides is styled under `.sp-edit`.
                <div class="page-inner sp" class:sp-edit=move || ctx.edit.get()>
                    <section class="panel sp-body">
                        <SemesterView ctx/>
                    </section>
                </div>
            </Frame>
        }
        .into_any()
    };
    // The Termin's dialog stands with the page, so that it closes by its address alone.
    let over = Signal::derive(move || termin.get() && !termin_full.get());
    view! {
        {page}
        <PlanTerminModal ctx open=over/>
    }
    .into_any()
}

/// The semester, in three parts (`.sp-body` lays them out). Above the fold (`.sp-fold`): its head
/// and the status row under it, the exams that collide (red, in every view), the line of overlaps,
/// and the view. Beside it where the page is wide enough, else under it: the planned
/// modules (`.sp-side`), a column of its own, so that however long their list is the week keeps
/// its height. Under the fold (`.sp-below`): what has no fixed time and the line that says what is
/// derived. The fold is as tall as the page shows, so that the week fits into it (owner,
/// 2026-09-25: the whole week on the screen, smaller on a small one, larger on a large one). The
/// week is there with nothing planned too, and says so in its middle; „Termine" and „Prüfungen"
/// say only that.
#[component]
fn SemesterView(ctx: PlanCtx) -> impl IntoView {
    // A module in the week, or a Termin of one's own: „Termine" and „Prüfungen" have something
    // to say (siblings of the plan, R16). The calendar file has modules' alone.
    let modules = Memo::new(move |_| ctx.wanted.with(|wanted| !wanted.1.is_empty()));
    let planned = Memo::new(move |_| modules.get() || ctx.selection.with(|(_, selection)| !selection.own.is_empty()));
    let failed = Memo::new(move |_| ctx.data.with(|data| data.as_ref().err().cloned()));
    let fine = Memo::new(move |_| failed.with(Option::is_none));
    let shown = Memo::new(move |_| ctx.url.with(|url| url.view));
    let weekly = Memo::new(move |_| matches!(shown.get(), PlanView::Week | PlanView::Overview));
    // The marked modules, for „+ Modul"'s „Merkliste" of each calendar of one's own (`add.rs`): one
    // query of the Merkliste for all of them.
    let marked = marked_offered(ctx);
    let view = move || match shown.get() {
        PlanView::Week | PlanView::Overview => view! { <WeekView ctx/> }.into_any(),
        PlanView::Dates => view! {
            {move || match planned.get() {
                true => view! { <DatesView ctx/> }.into_any(),
                false => view! { <NothingPlanned ctx/> }.into_any(),
            }}
        }
        .into_any(),
        PlanView::Exams => view! {
            {move || match planned.get() {
                true => view! { <ExamsView ctx/> }.into_any(),
                false => view! { <NothingPlanned ctx/> }.into_any(),
            }}
        }
        .into_any(),
    };
    view! {
        <div class="sp-fold">
            <share::ShareOffer ctx/>
            <share::LiveOffer ctx/>
            <SemesterHead ctx/>
            {move || match failed.get() {
                Some(error) => view! { <ErrorState error/> }.into_any(),
                None => view! {
                    <ExamAlerts ctx/>
                    // On a phone one band over the week (`head.rs`): what is left to choose — a way
                    // into the mode while looking, its rows while editing — and the overlaps'
                    // figure. Wider, the status row under the head says both, and the choice in
                    // focus stands there (`edit.rs`).
                    {move || ctx.phone.get().then(|| view! { <PhoneBand ctx/> })}
                    // What the overlaps' figure unfolds, under it.
                    <Overlaps ctx/>
                    <div class="sp-view">{view}</div>
                    // The choice in focus on a phone: a sheet under the week (H1 of
                    // folia/design/kalender/handy.html).
                    {move || ctx.phone.get().then(|| view! { <PickSheet ctx/> })}
                    // On a phone the mode is a dock in the tab bar's place (`edit.rs`).
                    {move || ctx.phone.get().then(|| view! { <EditDock ctx/> })}
                }
                .into_any(),
            }}
            // The form of a Termin of one's own and the note after it (`own.rs`).
            <OwnForm ctx/>
            <OwnNote ctx/>
            // „Teilen" (`share_dialog.rs`).
            <share_dialog::ShareDialog ctx/>
            // On a phone the sheets of the head's toolbar: the calendars and „Export" (`head.rs`).
            {move || ctx.phone.get().then(|| view! { <PhoneSheets ctx marked/> })}
        </div>
        {move || {
            fine.get().then(|| {
                view! {
                    <div class="sp-side">
                        <OpenPanel ctx/>
                        <ModuleList ctx/>
                        <CalendarModules ctx marked/>
                    </div>
                }
            })
        }}
        <div class="sp-below">
            {move || {
                (fine.get() && planned.get()).then(|| {
                    view! {
                        {move || weekly.get().then(|| view! { <WeekLoose ctx/> })}
                        <DerivedLine ctx/>
                    }
                })
            }}
        </div>
    }
}

/// Where the plan was when a page of its own was opened from it (the overview of a Termin from a row
/// of the list or from a card, a module's whole page from a calendar's line): the window's scroll,
/// the page's scroller's (`#page-scroll`, a narrow page), the column's beside the week (`.sp-side`)
/// and the link clicked. That page opens at its top (it is a page, not a part of the plan); leaving
/// it by „Zurück", Esc or Back builds the plan anew, which this puts back where it was, the keyboard
/// on the link (owner, 2026-09-25, of the plan: „Da sollte sich das Layout nicht viel shiften").
/// Kept in the browser app's memory for this one way there and back.
#[cfg(feature = "csr")]
mod left {
    use std::cell::RefCell;

    use folia_routes::url::{self, PlanAddress};
    use leptos::wasm_bindgen::{closure::Closure, JsCast};

    use crate::i18n::Locale;
    use leptos::web_sys;

    #[derive(Clone)]
    struct Left {
        /// The plan's address without its Termin (a card open or not, the way back is the plan).
        plan: String,
        top: f64,
        work: i32,
        side: i32,
        link: Option<String>,
    }

    thread_local! {
        static LEFT: RefCell<Option<Left>> = const { RefCell::new(None) };
    }

    fn plan_of(search: &str) -> String {
        PlanAddress::parse(search.trim_start_matches('?')).url.with_termin(None).path()
    }

    fn scroll_top(selector: &str) -> i32 {
        folia_design::stage::select(selector).map_or(0, |element| element.scroll_top())
    }

    /// Notes the place of the plan as a link to a page of its own is clicked, before the router
    /// takes the click (a listener of the capturing phase); the returned function takes it off.
    pub(super) fn watch() -> impl FnOnce() + Send + Sync + 'static {
        let listener = Closure::<dyn Fn(web_sys::MouseEvent)>::new(|ev: web_sys::MouseEvent| {
            if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
                return;
            }
            let Some(link) = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).and_then(|element| element.closest("a[href]").ok().flatten()) else { return };
            let Some(window) = web_sys::window() else { return };
            let here = window.location().search().unwrap_or_default();
            let Some(href) = link.get_attribute("href") else { return };
            let (path, to) = href.split_once('?').unwrap_or((href.as_str(), ""));
            // Only from the plan itself — the page is built behind the others, and the start
            // page's card links to the plan's overview too —, to a page of its own of the plan's
            // address.
            let on_plan = Locale::split(&window.location().pathname().unwrap_or_default()).1 == url::STUDYPLAN;
            if !on_plan || PlanAddress::parse(here.trim_start_matches('?')).full || !PlanAddress::parse(to).full || Locale::split(path).1 != url::STUDYPLAN {
                return;
            }
            let left = Left { plan: plan_of(&here), top: window.scroll_y().unwrap_or(0.0), work: scroll_top("#page-scroll"), side: scroll_top(".sp-side"), link: Some(href) };
            LEFT.with(|stored| *stored.borrow_mut() = Some(left));
        });
        let window = web_sys::window();
        if let Some(window) = &window {
            let _ = window.add_event_listener_with_callback_and_bool("click", listener.as_ref().unchecked_ref(), true);
        }
        let function: web_sys::js_sys::Function = listener.into_js_value().unchecked_into();
        let held = send_wrapper::SendWrapper::new(function);
        move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback_and_bool("click", &held, true);
            }
        }
    }

    /// The plan built anew at `search` after a page of its own: back where it was left, if it was
    /// left from this address — in the next frame and once more after the browser has put its own
    /// idea of the scroll in place (a step back through the history).
    pub(super) fn restore(search: &str) {
        let Some(left) = LEFT.with(|stored| stored.borrow_mut().take()) else { return };
        if left.plan != plan_of(search) {
            return;
        }
        let put = move |focus: bool| {
            if let Some(window) = web_sys::window() {
                window.scroll_to_with_x_and_y(0.0, left.top);
            }
            if let Some(work) = folia_design::stage::select("#page-scroll") {
                work.set_scroll_top(left.work);
            }
            if let Some(side) = folia_design::stage::select(".sp-side") {
                side.set_scroll_top(left.side);
            }
            let link = left.link.as_ref().filter(|_| focus).and_then(|href| folia_design::stage::select(&format!("a[href=\"{href}\"]")));
            if let Some(link) = link.and_then(|link| link.dyn_into::<web_sys::HtmlElement>().ok()) {
                let options = web_sys::FocusOptions::new();
                options.set_prevent_scroll(true);
                let _ = link.focus_with_options(&options);
            }
        };
        let again = put.clone();
        leptos::prelude::request_animation_frame(move || put(true));
        leptos::prelude::set_timeout(move || again(false), std::time::Duration::from_millis(220));
    }
}

/// The last group of the sidebar, on the server as in the app: where the plan lives, by whether
/// the active profile is synced (`active_synced`; the server's page says the private text).
#[component]
pub(super) fn StorageHint() -> impl IntoView {
    let t = i18n::t();
    let synced = folia_stores::sync::active_synced();
    view! {
        <div class="fgroup">
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{move || t.studyplan.storage_hint(synced.get())}</span></p>
        </div>
    }
}

/// A page of one visitor: the same address and the same explanation for everybody, for no index,
/// with the Stundenplan's own picture. A plan handed on by a link (`share=`, `share.rs`) is the
/// exception: its tags name its semester and modules, and its picture shows them
/// (`/cards/studyplan/<code>.png`), so the preview of the link says what was shared. What the code
/// names comes from the snapshot, on the server as in the app.
#[component]
fn PlanSeo() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let source = use_data().ok();
    // The tags change with the code alone, not with every view of the plan.
    let code = Memo::new(move |_| StudyplanUrl::parse(&location.search.get()).share);
    move || {
        let shared = code.get().and_then(|code| {
            let plan = SharedPlan::from_code(&code)?;
            let data = source.as_ref()?.now(&SharedPlanAsk { plan, locale: t.locale }).ok()??;
            (!data.modules.is_empty()).then_some((code, data))
        });
        match shared {
            Some((code, data)) => {
                let names = data.modules.iter().map(|module| module.name.as_str()).collect::<Vec<_>>().join(", ");
                let count = folia_design::format::modules(i64::try_from(data.modules.len()).unwrap_or(i64::MAX), t.locale);
                let description = (t.studyplan.shared_description)(&count, &names);
                view! {
                    <Seo title=(t.studyplan.shared_title)(&data.label) description path=url::share_path(&code) card=shared_plan::card_path(&code) noindex=true/>
                }
                .into_any()
            }
            None => view! {
                <Seo title=t.studyplan.title description=t.studyplan.seo_description path=url::STUDYPLAN card=folia_shell::seo::STUDYPLAN_CARD noindex=true/>
            }
            .into_any(),
        }
    }
}

/// What the server writes for every address of the plan, a route of the app (§4.2): its tags (a
/// shared plan's name its modules: a link preview runs no script) and the plan's skeleton.
fn server_page() -> impl IntoView {
    let t = i18n::t();
    view! {
        <Title text=t.studyplan.title/>
        <PlanSeo/>
        <AppStandin shape=Shape::Studyplan title=t.studyplan.server_title hint=t.studyplan.server_hint/>
    }
}

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

    /// „Zurück" from a module filling the page is the app's path, never with the language's prefix,
    /// which the link writes itself; a module of a calendar of one's own goes back to the plan alone.
    #[test]
    fn back_from_a_module_filling_the_page_is_the_apps_path() {
        let address = |query: &str| PlanAddress { url: StudyplanUrl::parse(query), full: true };
        assert_eq!(back_of(&address("open=99999"), false), "/studyplan");
        assert_eq!(back_of(&address("edit=1&open=99999"), false), "/studyplan?edit=1");
        assert_eq!(back_of(&address("open=12104"), true), "/studyplan?open=12104");
    }

    #[test]
    fn modul_ansehen_fills_the_plan_with_the_module() {
        let url = StudyplanUrl::parse("sem=2026W&view=dates&open=12104&row=148369-aaf38");
        assert_eq!(full_href(&url, "12104"), "/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38&full=1");
        assert_eq!(full_href(&url, "12107"), "/studyplan?sem=2026W&view=dates&open=12107&full=1");
    }
}
