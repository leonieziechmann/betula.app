//! The BTU catalog web app. The same components render on the server (complete HTML for
//! search engines, shared links and browsers without JavaScript) and in the browser.
//!
//! Rules (docs/frontend.md §2): navigation state comes from the router as plain values (R1);
//! server HTML never depends on the user (R9); no panics (R3, enforced by the clippy lints of
//! this crate); styling only through the classes of `assets/app.css` (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod data;
pub mod format;
pub mod icons;
pub mod pages;
pub mod ui;

use catalog::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::{path, SsrMode};

use crate::pages::{catalog::CatalogPage, home::HomePage, program::ProgramPage, programs::ProgramsPage};
use crate::ui::Icon;

/// Where the host serves the files of `app/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
pub const FONT: &str = "/assets/inter-latin.woff2";
pub const ENHANCE_SCRIPT: &str = "/assets/enhance.js";

/// Runs before the first paint: marks the document as scripted and applies a remembered theme,
/// so neither the filter button nor the colors flash.
const HEAD_SCRIPT: &str = "document.documentElement.classList.add('js');try{var t=localStorage.getItem('btu.theme');if(t==='dark'||t==='light')document.documentElement.dataset.theme=t}catch(e){}";

/// The HTML document around the app (server side only).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    // The browser bundle joins in the next step; until then `options` only configures the server.
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
    view! {
        <Link rel="preload" href=FONT as_="font" type_="font/woff2" crossorigin="anonymous"/>
        <Stylesheet href=STYLESHEET/>
        <Link rel="icon" type_="image/svg+xml" href=FAVICON/>
        <Title formatter=|title: String| if title.is_empty() { "BTU Modulkatalog".to_string() } else { format!("{title} · BTU Modulkatalog") }/>
        <Meta name="description" content="Module, Studiengänge und Regelstudienpläne der BTU Cottbus-Senftenberg: durchsuchbar, filterbar, aktuell."/>
        <Router>
            <a class="skip-link" href="#content">"Zum Inhalt springen"</a>
            <Rail/>
            <div class="main">
                <TopBar/>
                <main class="content" id="content">
                    <Routes fallback=|| view! { <div class="page"><ui::NotFound title="Seite nicht gefunden" hint="Diese Adresse gibt es nicht (mehr)."/></div> }>
                        <Route path=path!("/") view=HomePage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog") view=CatalogPage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog/module/:id") view=CatalogPage ssr=SsrMode::Async/>
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

/// Which main area a path belongs to (for the navigation's current item and the page title).
fn area(path: &str) -> &'static str {
    if path.starts_with(url::PROGRAMS) {
        "programs"
    } else if path.starts_with(url::CATALOG) {
        "catalog"
    } else {
        "home"
    }
}

#[component]
fn NavItems() -> impl IntoView {
    let location = use_location();
    let current = move |name: &'static str| (area(&location.pathname.get()) == name).then_some("page");
    view! {
        <a class="nav" href=url::HOME aria-current=move || current("home")><span class="ind"><Icon name="house"/></span>"Start"</a>
        <a class="nav" href=url::CATALOG aria-current=move || current("catalog")><span class="ind"><Icon name="layout-list"/></span>"Module"</a>
        <a class="nav" href=url::PROGRAMS aria-current=move || current("programs")><span class="ind"><Icon name="graduation-cap"/></span>"Studium"</a>
    }
}

#[component]
fn Rail() -> impl IntoView {
    view! {
        <aside class="rail">
            <a class="logo" href=url::HOME aria-label="BTU Modulkatalog"><Icon name="layout-list"/></a>
            <nav aria-label="Hauptnavigation"><NavItems/></nav>
            <span class="nav soon"><span class="ind"><Icon name="bookmark"/></span>"Merkliste"</span>
            <span class="nav soon"><span class="ind"><Icon name="calendar-range"/></span>"Planer"</span>
            <div class="rail-end">
                <button class="icon-btn theme-toggle" type="button" data-action="theme" aria-label="Hell oder dunkel">
                    <Icon name="moon" class="icon-moon"/><Icon name="sun" class="icon-sun"/>
                </button>
            </div>
        </aside>
    }
}

#[component]
fn TopBar() -> impl IntoView {
    let location = use_location();
    let title = move || match area(&location.pathname.get()) {
        "programs" => "Studiengänge",
        "catalog" => "Module",
        _ => "Start",
    };
    view! {
        <header class="topbar">
            <div class="crumb"><h1>{title}</h1></div>
            <form class="search" role="search" method="get" action=url::CATALOG>
                <Icon name="search"/>
                <label class="visually-hidden" for="topsearch">"Module suchen"</label>
                <input id="topsearch" type="search" name="q" placeholder="Modul, Nummer oder Thema suchen" autocomplete="off"/>
                <kbd>"Strg K"</kbd>
            </form>
        </header>
    }
}
