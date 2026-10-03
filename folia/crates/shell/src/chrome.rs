//! The chrome around every page: the rail with the main navigation and the languages, the top
//! bar with the page's title and search, the navigation at the bottom of a phone (`NavItems`),
//! and what lets the memory of the tabs follow the router (`FollowTabs`).

use folia_routes::url;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n::{self, use_location};
use crate::languages;
use crate::pending::Pending;
use crate::tabs::{Area, Tabs};
use folia_design::ui::{self, Icon};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::MineResolved;
use folia_stores::studyplan::Studyplan;

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
    // The catalog's first entry of a session is the catalog of „Mein Studiengang", while its PO is
    // in the snapshot (A.10); nothing is known of it on the server, whose tab is the plain link (R9).
    let mine = MineResolved::expect();
    let href = move |area: Area| {
        let path = location.pathname.get();
        t.path(&match (tabs, area) {
            (Some(tabs), Area::Catalog) => tabs.href_with_root(area, &path, &mine.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href)),
            (Some(tabs), _) => tabs.href(area, &path),
            (None, _) => area.root().to_string(),
        })
    };
    let plan = Studyplan::expect();
    let planned = Memo::new(move |_| plan.map(Studyplan::count).unwrap_or(0));
    view! {
        <a class="nav" data-area="home" href=t.path(url::HOME) title=t.app.home aria-current=move || current(Area::Home)><span class="ind"><Icon name="house"/></span>{t.app.home}</a>
        <a class="nav" data-area="catalog" href=move || href(Area::Catalog) title=t.app.modules aria-current=move || current(Area::Catalog)><span class="ind"><Icon name="layout-list"/></span>{t.app.modules}</a>
        <a class="nav" data-area="programs" href=move || href(Area::Programs) title=t.app.programs aria-current=move || current(Area::Programs)><span class="ind"><Icon name="graduation-cap"/></span>{t.app.study}</a>
        // The marked modules exist in the browser app only (R15). How many there are is known
        // there alone, so the number is never part of server HTML (R9). What the visitor keeps is
        // no page for a crawler (`seo`), here and in the Studienplan.
        <a class="nav js-only" data-area="bookmarks" href=move || href(Area::Bookmarks) rel="nofollow" title=t.app.bookmarks aria-current=move || current(Area::Bookmarks)>
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
        <a class="nav js-only" data-area="studyplan" href=move || href(Area::Studyplan) rel="nofollow" title=t.app.studyplan aria-current=move || current(Area::Studyplan)>
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

#[component]
pub fn TopBar() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    // Title and search belong to the page the app is at or going to (`pending`).
    let going = Pending::expect();
    let area_now = Memo::new(move |_| Area::of(&going.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get())));
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
            let target = if area_now.get_untracked() == Area::Programs {
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
            {move || {
                let programs = area_now.get() == Area::Programs;
                let modules = t.app.search_modules_placeholder;
                let (title, action, placeholder) = match area_now.get() {
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
