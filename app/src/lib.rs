//! The web app of Betula (the pages of Folia). The same components render on the server (complete HTML for
//! search engines, shared links and browsers without JavaScript) and in the browser.
//!
//! Rules (docs/frontend.md §2): navigation state comes from the router as plain values (R1);
//! server HTML never depends on the user (R9); no panics (R3, enforced by the clippy lints of
//! this crate); styling only through the classes of `assets/app.css` (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod combobox;
pub mod data;
pub mod format;
pub mod icons;
pub mod nav;
pub mod pages;
pub mod tabs;
pub mod ui;

use catalog::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::{path, NavigateOptions, SsrMode};

use crate::pages::{catalog::CatalogPage, home::HomePage, module::ModulePage, program::ProgramPage, programs::ProgramsPage};
use crate::tabs::{Area, Tabs};
use crate::ui::Icon;

/// Where the host serves the files of `app/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
pub const FONT: &str = "/assets/inter-latin.woff2";
pub const ENHANCE_SCRIPT: &str = "/assets/enhance.js";
/// Loads the local database and the browser app, which then takes the page over.
pub const BOOT_SCRIPT: &str = "/assets/boot.js";

/// Runs before the first paint: marks the document as scripted and applies what this browser
/// remembers (theme, widths of the filter panel and the module preview), so nothing flashes or jumps. Such personal
/// view settings live in localStorage, never in the URL and never in server HTML (R9).
const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px')}catch(e){}";

/// The HTML document around the app (server side only).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    // The browser app does not hydrate this HTML: it mounts fresh once its local database is
    // ready (`assets/boot.js`), so no hydration scripts are needed here.
    let _ = options;
    view! {
        <!DOCTYPE html>
        <html lang="de">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="color-scheme" content="light dark"/>
                <script inner_html=HEAD_SCRIPT></script>
                <MetaTags/>
                <script defer src=ENHANCE_SCRIPT></script>
                <script type="module" src=BOOT_SCRIPT></script>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    Tabs::provide();
    view! {
        <Link rel="preload" href=FONT as_="font" type_="font/woff2" crossorigin="anonymous"/>
        <Stylesheet href=STYLESHEET/>
        <Link rel="icon" type_="image/svg+xml" href=FAVICON/>
        <Title formatter=|title: String| if title.is_empty() { "Betula · Modulkatalog für die BTU Cottbus-Senftenberg (inoffiziell)".to_string() } else { format!("{title} · Betula") }/>
        <Meta name="description" content="Betula ist ein inoffizieller Modulkatalog für die BTU Cottbus-Senftenberg: Module, Studiengänge und Regelstudienpläne, durchsuchbar, filterbar, aktuell."/>
        <Router>
            <FollowTabs/>
            <a class="skip-link" href="#content">"Zum Inhalt springen"</a>
            <Rail/>
            <div class="main">
                <TopBar/>
                <main class="content" id="content">
                    <Routes fallback=|| view! { <div class="page"><ui::NotFound title="Seite nicht gefunden" hint="Diese Adresse gibt es nicht (mehr)."/></div> }>
                        <Route path=path!("/") view=HomePage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog") view=CatalogPage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog/module/:id") view=ModulePage ssr=SsrMode::Async/>
                        <Route path=path!("/programs") view=ProgramsPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug") view=ProgramPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug/:tab") view=ProgramPage ssr=SsrMode::Async/>
                    </Routes>
                </main>
            </div>
            <nav class="bottomnav" aria-label="Navigation"><NavItems/></nav>
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
    let location = use_location();
    let tabs = Tabs::expect();
    let current = move |area: Area| (Area::of(&location.pathname.get()) == area).then_some("page");
    let href = move |area: Area| {
        let path = location.pathname.get();
        match tabs {
            Some(tabs) => tabs.href(area, &path),
            None => area.root().to_string(),
        }
    };
    view! {
        <a class="nav" data-area="home" href=url::HOME title="Start" aria-current=move || current(Area::Home)><span class="ind"><Icon name="house"/></span>"Start"</a>
        <a class="nav" data-area="catalog" href=move || href(Area::Catalog) title="Module" aria-current=move || current(Area::Catalog)><span class="ind"><Icon name="layout-list"/></span>"Module"</a>
        <a class="nav" data-area="programs" href=move || href(Area::Programs) title="Studiengänge" aria-current=move || current(Area::Programs)><span class="ind"><Icon name="graduation-cap"/></span>"Studium"</a>
    }
}

#[component]
fn Rail() -> impl IntoView {
    view! {
        <aside class="rail">
            <a class="logo hit" href=url::HOME aria-label="Betula, zur Startseite"><ui::Mark/></a>
            <nav aria-label="Hauptnavigation"><NavItems/></nav>
            <span class="nav soon" title="Merkliste (in Arbeit)"><span class="ind"><Icon name="bookmark"/></span>"Merkliste"</span>
            <span class="nav soon" title="Semesterplaner (geplant)"><span class="ind"><Icon name="calendar-range"/></span>"Planer"</span>
            <div class="rail-end">
                <button class="icon-btn theme-toggle js-only" type="button" data-action="theme" aria-label="Hell oder dunkel">
                    <Icon name="moon" class="icon-moon"/><Icon name="sun" class="icon-sun"/>
                </button>
            </div>
        </aside>
    }
}

#[component]
fn TopBar() -> impl IntoView {
    let location = use_location();
    let area_now = Memo::new(move |_| Area::of(&location.pathname.get()));
    let navigate = use_navigate();
    let pending = StoredValue::new(None::<TimeoutHandle>);

    // The search belongs to the page: programs on the program overview, modules everywhere else.
    // In the browser app it filters while typing (replacing the history entry, not adding one).
    let on_input = move |ev: leptos::ev::Event| {
        let text = event_target_value(&ev);
        let navigate = navigate.clone();
        if let Some(handle) = pending.get_value() {
            handle.clear();
        }
        let run = move || {
            let target = if area_now.get_untracked() == Area::Programs {
                let on_overview = location.pathname.get_untracked() == url::PROGRAMS;
                let mut next = if on_overview { url::ProgramsUrl::parse(&location.search.get_untracked()) } else { Default::default() };
                next.text = text.trim().to_string();
                next.path()
            } else {
                let on_catalog = location.pathname.get_untracked() == url::CATALOG;
                let mut next = if on_catalog { url::CatalogUrl::parse(&location.search.get_untracked()) } else { Default::default() };
                next.query.text = text.clone();
                next.page = 1;
                next.path()
            };
            navigate(&target, NavigateOptions { replace: true, scroll: false, ..Default::default() });
        };
        pending.set_value(set_timeout_with_handle(run, std::time::Duration::from_millis(140)).ok());
    };

    view! {
        <header class="topbar">
            {move || {
                let programs = area_now.get() == Area::Programs;
                let (title, action, placeholder) = match area_now.get() {
                    Area::Programs => ("Studiengänge", url::PROGRAMS, "Studiengang suchen"),
                    Area::Catalog => ("Module", url::CATALOG, "Modul, Nummer oder Thema suchen"),
                    Area::Home => ("Start", url::CATALOG, "Modul, Nummer oder Thema suchen"),
                };
                let initial = url::parse_pairs(&location.search.get_untracked())
                    .into_iter()
                    .find(|(key, _)| key == "q")
                    .map(|(_, value)| value)
                    .unwrap_or_default();
                let on_input = on_input.clone();
                // The start page carries the name: next to the mark in the rail it reads as the logo.
                let home = area_now.get() == Area::Home;
                let heading = if home {
                    view! { <h1><ui::Wordmark small=true/></h1><small>"Modulkatalog · inoffiziell"</small> }.into_any()
                } else {
                    view! { <h1>{title}</h1> }.into_any()
                };
                view! {
                    <div class="crumb" class:brand=home>{heading}</div>
                    <form class="search" role="search" method="get" action=action data-live-search="">
                        <Icon name="search"/>
                        <label class="visually-hidden" for="topsearch">{if programs { "Studiengänge suchen" } else { "Module suchen" }}</label>
                        <input id="topsearch" type="search" name="q" value=initial placeholder=placeholder autocomplete="off" on:input=on_input/>
                        <ui::Shortcut keys="Strg K"/>
                    </form>
                    <span class="pill db-status" id="db-status" hidden></span>
                }
            }}
        </header>
    }
}
