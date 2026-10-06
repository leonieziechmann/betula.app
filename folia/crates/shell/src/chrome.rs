//! The chrome around every page: the rail with the main navigation and the languages, the top
//! bar with the page's title and search (and on a phone the way back of a page's step, `TopBack`),
//! the navigation at the bottom of a phone (`NavItems`) with what its tabs hold besides their pages
//! (`TabMenu`), and what lets the memory of the tabs follow the router (`FollowTabs`).

use folia_routes::url::{self, ProgramTab};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n::{self, use_location};
use crate::languages;
use crate::pending::Pending;
use crate::tabs::{Area, TabAgain, Tabs};
use folia_design::ui::{self, Icon};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::{program_href, program_name, MineResolved, MyProgram};
use folia_stores::studyplan::Studyplan;

/// The browser app (`csr`): only there is „Mein Studium", the first page of „Studium".
const APP: bool = cfg!(feature = "csr");

/// Lets the memory of the tabs follow the router (it needs the router's context).
#[component]
pub fn FollowTabs() -> impl IntoView {
    if let Some(tabs) = Tabs::expect() {
        tabs.follow();
    }
}

/// The main navigation. Its items are tabs: each leads to where its area was left (`tabs`).
#[component]
pub fn NavItems() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let tabs = Tabs::expect();
    // The tab of the page the app is going to is current at once, before the page is there.
    let pending = Pending::expect();
    let current = move |area: Area| (Area::of(&pending.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get())) == area).then_some("page");
    // A tab tapped where its area is (not the end of a swipe along the bar, enhance.js): its page
    // goes back to where it starts (`TabAgain`).
    let again = TabAgain::expect();
    let tap = move |area: Area| {
        move |ev: leptos::ev::MouseEvent| {
            let plain = ev.button() == 0 && !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key());
            if plain && !ev.default_prevented() && untrack(|| current(area)).is_some() {
                if let Some(again) = again {
                    again.tapped(area);
                }
            }
        }
    };
    // The catalog's first entry of a session is the catalog of „Mein Studiengang", while its PO is
    // in the snapshot (A.10); nothing is known of it on the server, whose tab is the plain link (R9).
    let mine = MineResolved::expect();
    // The catalog's tab held on a phone opens the catalog's list with its filters (enhance.js): the
    // list as it was left, else the catalog its tab starts at.
    let catalog_list = move || {
        APP.then(|| t.path(&tabs.and_then(Tabs::catalog_list).unwrap_or_else(|| mine.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href))))
    };
    let href = move |area: Area| {
        let path = location.pathname.get();
        t.path(&match (tabs, area) {
            // „Studium" is „Mein Studium" in the app; without it (the server's page, no JavaScript)
            // there is nothing to plan, and the tab is the overview of the programs.
            (_, Area::Programs) if !APP => url::PROGRAMS.to_string(),
            (Some(tabs), Area::Catalog) => tabs.href_with_root(area, &path, &mine.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href)),
            (Some(tabs), _) => tabs.href(area, &path),
            (None, _) => area.root().to_string(),
        })
    };
    // The Stundenplan is the timetable of the current semester: its tab counts what that semester
    // holds, not what „Mein Studium" places into later ones; every module planned until the
    // catalog has said which semester is the current one.
    let plan = Studyplan::expect();
    let meta = folia_data::use_ask(|| APP.then_some(folia_pages::ask::MetaAsk {}));
    let semester = Memo::new(move |_| meta.with(|meta| meta.as_ref().and_then(|meta| meta.as_ref().ok()).and_then(|meta| meta.current_semester.as_deref().and_then(folia_calendar::semester::SemesterKey::parse))));
    let planned = Memo::new(move |_| match (plan, semester.get()) {
        (Some(plan), Some(current)) => plan.count_in(current),
        (Some(plan), None) => plan.count(),
        (None, _) => 0,
    });
    view! {
        <a class="nav" data-area="home" href=t.path(url::HOME) on:click=tap(Area::Home) title=t.app.home aria-current=move || current(Area::Home)><span class="ind"><Icon name="house"/></span>{t.app.home}</a>
        <a class="nav" data-area="catalog" href=move || href(Area::Catalog) on:click=tap(Area::Catalog) data-hold=APP.then_some("filters") data-list=catalog_list title=t.app.modules aria-current=move || current(Area::Catalog)><span class="ind"><Icon name="layout-list"/></span>{t.app.modules}</a>
        <a class="nav" data-area="programs" href=move || href(Area::Programs) on:click=tap(Area::Programs) data-hold=APP.then_some("menu") title=t.app.programs aria-current=move || current(Area::Programs)><span class="ind"><Icon name="graduation-cap"/></span>{t.app.study}</a>
        // The marked modules exist in the browser app only (R15). How many there are is known
        // there alone, so the number is never part of server HTML (R9). What the visitor keeps is
        // no page for a crawler (`seo`), here and in the Studienplan.
        <a class="nav js-only" data-area="bookmarks" href=move || href(Area::Bookmarks) on:click=tap(Area::Bookmarks) rel="nofollow" title=t.app.bookmarks aria-current=move || current(Area::Bookmarks)>
            <span class="ind">
                <Icon name="bookmark"/>
                {move || {
                    let marked = Bookmarks::expect().map(|bookmarks| bookmarks.count()).unwrap_or(0);
                    (marked > 0).then(|| view! { <span class="nav-count num" aria-label=(t.app.marked_count)(marked)>{if marked > 99 { "99+".to_string() } else { marked.to_string() }}</span> })
                }}
            </span>
            {t.app.bookmarks}
        </a>
        // The Studienplan lives in the browser app alone, like the Merkliste (R15), and so does the
        // number of its modules (R9). The number is a memo of its own: most changes of the plan
        // (a hidden Termin, a move) leave it as it is.
        <a class="nav js-only" data-area="studyplan" href=move || href(Area::Studyplan) on:click=tap(Area::Studyplan) rel="nofollow" title=t.app.studyplan aria-current=move || current(Area::Studyplan)>
            <span class="ind">
                <Icon name="calendar-range"/>
                {move || {
                    let planned = planned.get();
                    (planned > 0).then(|| view! { <span class="nav-count num" aria-label=(t.app.planned_count)(planned)>{if planned > 99 { "99+".to_string() } else { planned.to_string() }}</span> })
                }}
            </span>
            {t.app.studyplan}
        </a>
    }
}

/// Where a tab's menu stands (`TabMenu`): the tab's area, and its box on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Held {
    area: Area,
    /// The tab is one of the bar at the bottom of a phone (the menu stands above the bar), else
    /// one of the rail (the menu stands beside it).
    bar: bool,
    top: f64,
    right: f64,
    middle: f64,
}

/// What a tab holds besides its page (owner, 2026-10-06: „wenn man die buttons in der nav bar lange
/// gedrückt hält, dass dann eine special aktion kommt. Bei mein Studium währe das dann eine auswahl
/// von Regelstudienplan, Wahlpflicht, Alle Studiengänge. Das sollte natürlich auch mit
/// [Rechts]klick funktionieren."): a menu at the tab, over the page. „Studium"'s leads to the
/// Regelstudienplan and the electives of „Mein Studiengang" (where one is chosen; the plan of its
/// Studienrichtung, as the app links the program everywhere) and to all programs. enhance.js hears
/// the long press of a finger and the right click and names the tab (`betula:tab-menu` on it,
/// where `NavItems` marked it `data-hold="menu"`); a tap beside the menu, Esc, a way taken or
/// another page closes it. The browser app's alone. (The catalog's tab, held on a phone, opens
/// the catalog with its filters: enhance.js does that by itself.)
#[component]
pub fn TabMenu() -> impl IntoView {
    let t = i18n::t();
    let held = RwSignal::new(None::<Held>);
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        use leptos::wasm_bindgen::JsCast;
        let open = window_event_listener_untyped("betula:tab-menu", move |ev| {
            let Some(tab) = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()) else { return };
            let Some(area) = tab.get_attribute("data-area").as_deref().and_then(Area::from_name) else { return };
            let rect = tab.get_bounding_client_rect();
            let bar = tab.closest(".bottomnav").ok().flatten().is_some();
            held.set(Some(Held { area, bar, top: rect.top(), right: rect.right(), middle: rect.left() + rect.width() / 2.0 }));
        });
        let keys = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() == "Escape" && held.with_untracked(Option::is_some) {
                ev.prevent_default();
                held.set(None);
            }
        });
        on_cleanup(move || {
            open.remove();
            keys.remove();
        });
    });
    // Another page: the menu goes.
    let location = use_location();
    Effect::new(move |before: Option<()>| {
        location.pathname.track();
        location.search.track();
        if before.is_some() {
            held.set(None);
        }
    });
    let mine = MineResolved::expect();
    let stored = MyProgram::expect();
    let source = folia_data::use_data().ok();
    // „Mein Studiengang" while the catalog knows it, with the Studienrichtung stored for it.
    let program = Memo::new(move |_| mine.and_then(|mine| mine.0.with(|info| info.as_ref().map(|info| info.program.clone()))));
    let first = NodeRef::<leptos::html::A>::new();
    // The first way has the focus: Tab goes on from there, Esc closes the menu.
    Effect::new(move |_| {
        if let Some(first) = first.get() {
            let _ = first.focus();
        }
    });
    let close = move |_| held.set(None);
    move || {
        let at = held.get()?;
        let ways = match at.area {
            Area::Programs => {
                let mut ways = Vec::new();
                if let Some(program) = program.get() {
                    let (caption, direction) = stored.map(|stored| stored.with(|doc| (doc.caption.clone(), doc.direction.clone()))).unwrap_or_default();
                    ways.push(("file-check-2", t.app.to_plan, program_href(source.as_ref(), &program, caption.as_deref(), direction.as_deref())));
                    ways.push(("layout-list", t.app.to_areas, url::program_path(&program.slug, ProgramTab::Areas)));
                }
                ways.push(("graduation-cap", t.app.all_programs, url::PROGRAMS.to_string()));
                ways
            }
            _ => Vec::new(),
        };
        if ways.is_empty() {
            return None;
        }
        let name = (at.area == Area::Programs).then(|| program.get().map(|program| program_name(&program))).flatten();
        let label = (t.app.shortcuts)(match at.area {
            Area::Programs => t.app.study,
            Area::Catalog => t.app.modules,
            Area::Home => t.app.home,
            Area::Bookmarks => t.app.bookmarks,
            Area::Studyplan => t.app.studyplan,
        });
        // Over the bar, above the tab (app.css keeps it on the screen); beside the rail, at the tab.
        let place = if at.bar { format!("--tab-x: {:.0}px", at.middle) } else { format!("left: {:.0}px; top: {:.0}px", at.right + 8.0, at.top) };
        Some(view! {
            <div class="tab-scrim" on:click=close on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); held.set(None) }></div>
            <nav class="tab-menu" class:on-bar=at.bar aria-label=label style=place>
                {name.map(|name| view! { <p class="tab-menu-head">{name}</p> })}
                {ways.into_iter().enumerate().map(|(i, (icon, words, href))| {
                    let node = if i == 0 { first } else { NodeRef::new() };
                    view! { <a class="tab-menu-way" href=t.path(&href) node_ref=node on:click=close><Icon name=icon/><span>{words}</span></a> }
                }).collect_view()}
            </nav>
        })
    }
}

#[component]
pub fn Rail() -> impl IntoView {
    let t = i18n::t();
    view! {
        <aside class="rail">
            <a class="logo hit" href=t.path(url::HOME) aria-label=t.app.logo_label><ui::Mark/></a>
            <nav aria-label=t.app.main_navigation><NavItems/></nav>
            <div class="rail-end">
                <languages::Languages/>
                <button class="icon-btn theme-toggle js-only" type="button" data-action="theme" aria-label=t.app.theme_toggle>
                    <Icon name="moon" class="icon-moon"/><Icon name="sun" class="icon-sun"/>
                </button>
            </div>
        </aside>
    }
}

/// A way back at the head of a phone's page, left of the search (owner, 2026-10-05: „wenn man in
/// der semester ansicht ist, soll es oben links neben der search bar im gleichen style eine
/// quadratische box sein mit einem zurück pfeil"): a box as the search is and as wide as it is
/// high, an arrow in it. A page with a step below its first one puts it there while that step
/// shows (a semester of „Mein Studium", on either side of its overview) and takes it away when it
/// goes; the box slides in and out, and the search makes room for it (app.css). The browser app's
/// alone, and a wide screen has none. The page hears it pressed as well (`presses`): where it
/// leads where the app is already (the page's step not in the address yet), the page goes back
/// itself.
#[derive(Clone, Copy)]
pub struct TopBack {
    back: RwSignal<Option<Back>>,
    pressed: RwSignal<u32>,
}

/// Where the box at the head leads (`TopBack`).
#[derive(Clone, Debug, PartialEq)]
pub struct Back {
    /// The path, without the language's prefix.
    pub href: String,
    /// What it is called (the arrow's words for those who do not see it).
    pub label: &'static str,
    /// The step before in the history is where it leads: it goes back there instead, the same
    /// entry as before, and the history does not grow.
    pub history: bool,
}

impl TopBack {
    /// Creates the box's place, empty, and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let back = TopBack { back: RwSignal::new(None), pressed: RwSignal::new(0) };
        provide_context(back);
        back
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Shows the box leading to `back`, or none.
    pub fn set(self, back: Option<Back>) {
        if self.back.with_untracked(|now| *now != back) {
            self.back.set(back);
        }
    }

    /// How often the box was pressed. Tracked.
    pub fn presses(self) -> u32 {
        self.pressed.get()
    }
}

/// The box of `TopBack`: there while a page puts it there. Where it went, it keeps its place and its
/// arrow and slides out, out of reach.
#[component]
fn TopBackBox() -> impl IntoView {
    let t = i18n::t();
    let back = TopBack::expect();
    let shown = Memo::new(move |_| back.and_then(|back| back.back.get()));
    let last = Memo::new(move |before: Option<&Option<Back>>| shown.get().or_else(|| before.cloned().flatten()));
    let on = move || shown.with(Option::is_some);
    let click = move |ev: leptos::ev::MouseEvent| {
        if let Some(back) = back {
            back.pressed.update(|presses| *presses = presses.wrapping_add(1));
        }
        let history = shown.with_untracked(|back| back.as_ref().is_some_and(|back| back.history));
        if history && ev.button() == 0 && !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key()) {
            ev.prevent_default();
            #[cfg(feature = "csr")]
            if let Some(history) = leptos::web_sys::window().and_then(|window| window.history().ok()) {
                let _ = history.back();
            }
        }
    };
    view! {
        <a
            class="top-back"
            class:on=on
            inert=move || !on()
            href=move || last.with(|back| back.as_ref().map(|back| t.path(&back.href)))
            aria-label=move || last.with(|back| back.as_ref().map(|back| back.label))
            title=move || last.with(|back| back.as_ref().map(|back| back.label))
            data-noscroll=""
            on:click=click
        >
            <Icon name="arrow-left"/>
        </a>
    }
}

#[component]
pub fn TopBar() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    // Title and search belong to the page the app is at or going to (`pending`).
    let going = Pending::expect();
    let path_now = Memo::new(move |_| going.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get()));
    let area_now = Memo::new(move |_| Area::of(&path_now.get()));
    // „Mein Studium" is the visitor's study, not a list of programs: its search is the modules'.
    let study_now = Memo::new(move |_| path_now.get() == url::STUDY);
    let pending = StoredValue::new(None::<TimeoutHandle>);

    // The search belongs to the page: programs on the program overview, modules everywhere else.
    // In the browser app it filters while typing (replacing the history entry, not adding one).
    let on_input = move |ev: leptos::ev::Event| {
        let text = event_target_value(&ev);
        if let Some(handle) = pending.get_value() {
            handle.clear();
        }
        // The list of a text typed before is not built any more once it is worked out: this key's
        // takes its place (`Pending::typed`).
        if let Some(going) = going {
            going.typed();
        }
        let run = move || {
            // On top of where the visitor is headed: a filter clicked a moment ago stays.
            let (path, search) = Pending::shown_of(going, location.pathname, location.search);
            let target = if area_now.get_untracked() == Area::Programs && !study_now.get_untracked() {
                let mut next = if path == url::PROGRAMS { url::ProgramsUrl::parse(&search) } else { Default::default() };
                next.text = text.trim().to_string();
                next.path()
            } else {
                let mut next = if path == url::CATALOG { url::CatalogUrl::parse(&search) } else { Default::default() };
                next.query.text = text.clone();
                next.page = 1;
                // What is typed is searched for the best matches first: an order chosen before
                // gives way to relevance (owner, 2026-09-30), and a column orders the matches again.
                next.query.sort = folia_routes::filter::SortKey::Default;
                next.query.descending = false;
                next.path()
            };
            if let Some(going) = going {
                going.go_quietly(&target, NavigateOptions { replace: true, scroll: false, ..Default::default() });
            }
        };
        pending.set_value(set_timeout_with_handle(run, std::time::Duration::from_millis(140)).ok());
    };

    view! {
        <header class="topbar">
            {APP.then(|| view! { <TopBackBox/> })}
            {move || {
                let study = study_now.get();
                let programs = area_now.get() == Area::Programs && !study;
                let modules = t.app.search_modules_placeholder;
                let (title, action, placeholder) = match area_now.get() {
                    Area::Programs if study => (t.app.my_studies, url::CATALOG, modules),
                    Area::Programs => (t.app.programs, url::PROGRAMS, t.app.search_programs_placeholder),
                    Area::Catalog => (t.app.modules, url::CATALOG, modules),
                    Area::Bookmarks => (t.app.bookmarks, url::CATALOG, modules),
                    Area::Studyplan => (t.app.studyplan, url::CATALOG, modules),
                    Area::Home => (t.app.home, url::CATALOG, modules),
                };
                let initial = url::parse_pairs(&Pending::shown_of(going, location.pathname, location.search).1)
                    .into_iter()
                    .find(|(key, _)| key == "q")
                    .map(|(_, value)| value)
                    .unwrap_or_default();
                // The start page carries the name: next to the mark in the rail it reads as the logo.
                let home = area_now.get() == Area::Home;
                let heading = if home {
                    view! { <h1><ui::Wordmark small=true/></h1><small>{t.common.tagline}</small> }.into_any()
                } else {
                    view! { <h1>{title}</h1> }.into_any()
                };
                view! {
                    <div class="crumb" class:brand=home>{heading}</div>
                    <form class="search" role="search" method="get" action=t.path(action) data-live-search="">
                        <Icon name="search"/>
                        <label class="visually-hidden" for="topsearch">{if programs { t.app.search_programs } else { t.app.search_modules }}</label>
                        <input id="topsearch" type="search" name="q" value=initial placeholder=placeholder autocomplete="off" on:input=on_input/>
                        <ui::Shortcut keys=t.common.search_shortcut/>
                    </form>
                    <span class="pill db-status" id="db-status" hidden></span>
                }
            }}
        </header>
    }
}
