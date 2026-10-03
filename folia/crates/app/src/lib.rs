//! The web app of Betula (the pages of Folia). The same components render on the server (complete HTML for
//! search engines, shared links and browsers without JavaScript) and in the browser.
//!
//! Rules (docs/folia/frontend.md §2): navigation state comes from the router as plain values (R1);
//! server HTML never depends on the user (R9); no panics (R3, enforced by the clippy lints of
//! this crate); styling only through the classes of `assets/app.css` (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
pub mod local;
pub mod pages;
pub mod swipe;
pub mod week;

use folia_routes::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{path, NavigateOptions, SsrMode};

use folia_stores::bookmarks::Bookmarks;
use folia_shell::ground::{Crown, Ground, Wood};
use crate::i18n::use_location;
use folia_design::ui;
use folia_shell::{languages, pending, seo, skeleton};
use folia_stores::myprogram::{MineResolved, MyProgram};
use crate::pages::bookmarks::BookmarksPage;
use crate::pages::legal::{ImprintPage, PrivacyPage};
use crate::pages::studyplan::StudyplanPage;
use crate::pages::{catalog::CatalogPage, home::HomePage, module::ModulePage, program::ProgramPage, programs::ProgramsPage};
use folia_shell::pending::Pending;
use folia_stores::studyplan::Studyplan;
use folia_shell::tabs::{Area, Tabs};
use folia_design::ui::Icon;

pub use folia_design::{asset, BuildId};
/// The addresses and scripts of the document every page is in (`folia_shell::document`).
pub use folia_shell::document::*;


/// The HTML document around the app (server side only).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    // The browser app does not hydrate this HTML: it mounts fresh once its local database is
    // ready (`assets/boot.js`), so no hydration scripts are needed here.
    let _ = options;
    let t = i18n::t();
    view! {
        <!DOCTYPE html>
        <html lang=t.locale.code()>
            <head>
                <meta charset="utf-8"/>
                // First of all: the page in the visitor's language, before anything is loaded or drawn.
                <script inner_html=languages::language_script()></script>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="color-scheme" content="light dark"/>
                // Who the site is, for tabs, home screens and link previews. Static and the same
                // on every page, so it lives here and survives the takeover by the browser app.
                <meta name="application-name" content=seo::SITE_NAME/>
                <meta name="apple-mobile-web-app-title" content=seo::SITE_NAME/>
                // One tag: the head script turns it dark with the theme (crawlers read the light one).
                <meta name="theme-color" content=THEME_LIGHT/>
                <link rel="icon" href=FAVICON_ICO sizes="32x32"/>
                <link rel="icon" type="image/svg+xml" href=FAVICON/>
                // The icon of the app for Google Search; tabs keep the mark (`ICON_192`).
                <link rel="icon" type="image/png" sizes="192x192" href=ICON_192/>
                <link rel="apple-touch-icon" href=TOUCH_ICON/>
                // The app a home screen installs from this page: this language's (`/en/…`).
                <link rel="manifest" href=t.path(MANIFEST)/>
                <script inner_html=HEAD_SCRIPT></script>
                // The font and the stylesheet are the same on every page, so they are part of the
                // document and not of `App`: the browser app does not hydrate, it mounts fresh, and
                // leptos_meta would add a second `<link>` to the head for each. Two copies of the
                // stylesheet can come from different places (the service worker's cache, the
                // network) and mix an old sheet into a new one. `as` comes first: after a value
                // the macro would read it as a cast. The font is linked as it is: it never changes
                // with a build, and `app.css` names it by the same plain address.
                <link as="font" rel="preload" type="font/woff2" crossorigin="anonymous" href=FONT/>
                <link rel="stylesheet" href=asset(STYLESHEET)/>
                <style inner_html=VIEW_TRANSITION_STYLE></style>
                <MetaTags/>
                <script defer src=asset(ENHANCE_SCRIPT)></script>
                <script type="module" src=asset(BOOT_SCRIPT)></script>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // The language of the address, for the whole life of the app: another language is another
    // page load (`i18n`).
    let locale = i18n::locale();
    provide_context(locale);
    let t = i18n::texts(locale);
    provide_meta_context();
    Tabs::provide();
    // The visitor's marked modules, Studienplan and „Mein Studiengang": from this browser's
    // storage, empty on the server (R9). What the catalog knows of the program follows the store.
    Bookmarks::provide();
    Studyplan::provide();
    MyProgram::provide();
    MineResolved::provide();
    // A click answers in the next frame and the page follows (`pending`): its listeners have to
    // come before the router's, so before `<Router>` is built.
    let pending = Pending::provide();
    view! {
        // Description, canonical address and the rest of what search engines read belong to the
        // page (`seo::Seo`), not to the app: a page must not carry two descriptions.
        <Title formatter=move |title: String| if title.is_empty() { t.app.default_title.to_string() } else { format!("{title} · Betula") }/>
        // Below the language's prefix: the routes and every path inside the app are without it.
        <Router base=locale.prefix()>
            <pending::Bind/>
            <FollowTabs/>
            <a class="skip-link" href="#content">{t.app.skip_to_content}</a>
            <Crown/>
            <Wood/>
            <Rail/>
            <div class="main">
                <TopBar/>
                <main class="content" id="content" aria-busy=move || pending.busy().then_some("true")>
                    <Routes fallback=move || view! { <folia_shell::frame::Plain><ui::NotFound title=t.app.not_found_title hint=t.app.not_found_hint/></folia_shell::frame::Plain> }>
                        <Route path=path!("/") view=HomePage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog") view=CatalogPage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog/module/:id") view=ModulePage ssr=SsrMode::Async/>
                        <Route path=path!("/programs") view=ProgramsPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug") view=ProgramPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug/:tab") view=ProgramPage ssr=SsrMode::Async/>
                        <Route path=path!("/bookmarks") view=BookmarksPage ssr=SsrMode::Async/>
                        <Route path=path!("/studyplan") view=StudyplanPage ssr=SsrMode::Async/>
                        <Route path=path!("/impressum") view=ImprintPage ssr=SsrMode::Async/>
                        <Route path=path!("/datenschutz") view=PrivacyPage ssr=SsrMode::Async/>
                    </Routes>
                    <skeleton::PendingPage/>
                    <ui::ToTop/>
                </main>
            </div>
            <Ground/>
            <nav class="bottomnav" aria-label=t.app.navigation><NavItems/></nav>
        </Router>
    }
}

/// Lets the memory of the tabs follow the router (it needs the router's context).
#[component]
fn FollowTabs() -> impl IntoView {
    if let Some(tabs) = Tabs::expect() {
        tabs.follow();
    }
}

/// The main navigation. Its items are tabs: each leads to where its area was left (`tabs`).
#[component]
fn NavItems() -> impl IntoView {
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
fn Rail() -> impl IntoView {
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
fn TopBar() -> impl IntoView {
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
