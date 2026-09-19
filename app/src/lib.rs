//! The BTU catalog web app. The same components render on the server (complete HTML for
//! search engines, shared links and browsers without JavaScript) and hydrate in the browser.
//!
//! Rules (docs/frontend-rewrite.md §5, docs/frontend-phase0.md §2.3): navigation state comes
//! from the router as plain values (R1); server HTML never depends on the user (R9); no
//! panics (R3, enforced by the clippy lints of this crate); no inline styles (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod data;
pub mod format;
pub mod pages;
pub mod ui;

use catalog::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{path, SsrMode};

use crate::pages::{catalog::CatalogPage, home::HomePage, module::ModulePage, program::ProgramPage, programs::ProgramsPage};

/// Where the host serves the files of `app/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";

/// The HTML document around the app (server side only).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    // Phase 1 serves plain HTML. The hydration scripts join in phase 2, together with the
    // browser's own data source; until then `options` only configures the server.
    let _ = options;
    view! {
        <!DOCTYPE html>
        <html lang="de">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="theme-color" content="#0b3a53"/>
                <MetaTags/>
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
        <Stylesheet href=STYLESHEET/>
        <Link rel="icon" type_="image/svg+xml" href=FAVICON/>
        <Title formatter=|title: String| if title.is_empty() { "BTU Modulkatalog".to_string() } else { format!("{title} · BTU Modulkatalog") }/>
        <Meta name="description" content="Module, Studiengänge und Regelstudienpläne der BTU Cottbus-Senftenberg: durchsuchbar, filterbar, aktuell."/>
        <Router>
            <a class="skip-link" href="#content">"Zum Inhalt springen"</a>
            <header class="topbar">
                <a class="brand" href=url::HOME>"BTU Modulkatalog"</a>
                <nav class="topnav" aria-label="Hauptnavigation">
                    <a href=url::CATALOG>"Module"</a>
                    <a href=url::PROGRAMS>"Studiengänge"</a>
                </nav>
                <form class="topsearch" role="search" method="get" action=url::CATALOG>
                    <label class="visually-hidden" for="topsearch">"Module suchen"</label>
                    <input id="topsearch" type="search" name="q" placeholder="Modul, Nummer oder Thema suchen" autocomplete="off"/>
                </form>
            </header>
            <main id="content">
                <Routes fallback=|| view! { <ui::NotFound title="Seite nicht gefunden" hint="Diese Adresse gibt es nicht (mehr)."/> }>
                    <Route path=path!("/") view=HomePage ssr=SsrMode::Async/>
                    <Route path=path!("/catalog") view=CatalogPage ssr=SsrMode::Async/>
                    <Route path=path!("/catalog/module/:id") view=ModulePage ssr=SsrMode::Async/>
                    <Route path=path!("/programs") view=ProgramsPage ssr=SsrMode::Async/>
                    <Route path=path!("/programs/:slug") view=ProgramPage ssr=SsrMode::Async/>
                    <Route path=path!("/programs/:slug/:tab") view=ProgramPage ssr=SsrMode::Async/>
                </Routes>
            </main>
            <nav class="bottomnav" aria-label="Navigation">
                <a href=url::HOME>"Start"</a>
                <a href=url::CATALOG>"Module"</a>
                <a href=url::PROGRAMS>"Studiengänge"</a>
            </nav>
        </Router>
    }
}
