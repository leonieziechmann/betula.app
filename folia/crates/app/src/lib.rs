//! The web app of Betula (the pages of Folia), composed: the document every page is in (`shell`)
//! and the app with its routes (`App`), of the shell, the stores and the features' pages. The same
//! components render on the server (complete HTML for search engines, shared links and browsers
//! without JavaScript) and in the browser (docs/folia/folia-refactor.md §7).
//!
//! Rules (docs/folia/frontend.md §2): navigation state comes from the router as plain values (R1);
//! server HTML never depends on the user (R9); no panics (R3, enforced by the clippy lints of
//! this crate); styling only through the classes of `assets/app.css` (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod i18n;
mod quotes;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{path, SsrMode};

use folia_bookmarks::bookmarks::BookmarksPage;
use folia_catalog::catalog::CatalogPage;
use folia_design::ui;
use folia_home::home::HomePage;
use folia_home::legal::{ImprintPage, PrivacyPage};
use folia_planner::study::StudyPage;
use folia_planner::studyplan::StudyplanPage;
use folia_programs::{program::ProgramPage, programs::ProgramsPage};
use folia_shell::chrome::{FollowTabs, NavItems, Rail, TabMenu, TopBack, TopBar};
use folia_shell::frame::Plain;
use folia_shell::ground::{Crown, Ground, Wood};
use folia_shell::pending::Pending;
use folia_shell::tabs::{TabAgain, Tabs};
use folia_shell::{languages, pending, seo, skeleton};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::{MineResolved, MyProgram};
use folia_stores::studyplan::Studyplan;
use folia_catalog::module::ModulePage;

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
    // A tab tapped again where its area is: the area's page back to where it starts.
    TabAgain::provide();
    // The visitor's marked modules, Studienplan and „Mein Studiengang": from this browser's
    // storage, empty on the server (R9). What the catalog knows of the program follows the store.
    Bookmarks::provide();
    Studyplan::provide();
    MyProgram::provide();
    MineResolved::provide();
    // The way back a page's step puts at the head of a phone's page (`chrome::TopBack`).
    TopBack::provide();
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
                    <Routes fallback=move || view! { <Plain><ui::NotFound title=t.app.not_found_title hint=t.app.not_found_hint/></Plain> }>
                        <Route path=path!("/") view=HomePage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog") view=CatalogPage ssr=SsrMode::Async/>
                        <Route path=path!("/catalog/module/:id") view=ModulePage ssr=SsrMode::Async/>
                        <Route path=path!("/programs") view=ProgramsPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug") view=ProgramPage ssr=SsrMode::Async/>
                        <Route path=path!("/programs/:slug/:tab") view=ProgramPage ssr=SsrMode::Async/>
                        <Route path=path!("/bookmarks") view=BookmarksPage ssr=SsrMode::Async/>
                        <Route path=path!("/studyplan") view=StudyplanPage ssr=SsrMode::Async/>
                        <Route path=path!("/study") view=StudyPage ssr=SsrMode::Async/>
                        <Route path=path!("/impressum") view=ImprintPage ssr=SsrMode::Async/>
                        <Route path=path!("/datenschutz") view=PrivacyPage ssr=SsrMode::Async/>
                    </Routes>
                    <skeleton::PendingPage/>
                    <ui::ToTop/>
                </main>
            </div>
            <Ground/>
            <nav class="bottomnav" aria-label=t.app.navigation><NavItems/></nav>
            <TabMenu/>
        </Router>
    }
}
