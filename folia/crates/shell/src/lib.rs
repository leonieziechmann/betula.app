//! The shell (docs/folia-refactor.md §5): the icon rail (the bottom bar on a phone), the header,
//! the footer and the background. The app mounts it once, as the parent route of every page; the
//! site renders the same markup around its pages. A page brings everything between them.

use folia_design::{Icon, Mark, Wordmark};
use leptos::prelude::*;

/// The areas of the rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Home,
    Catalog,
    Programs,
    Bookmarks,
    Studyplan,
}

impl Area {
    pub fn of(path: &str) -> Self {
        let under = |root: &str| path == root || path.starts_with(&format!("{root}/"));
        if under("/catalog") {
            Area::Catalog
        } else if under("/programs") {
            Area::Programs
        } else if under("/bookmarks") {
            Area::Bookmarks
        } else if under("/studyplan") {
            Area::Studyplan
        } else {
            Area::Home
        }
    }

    fn title(self) -> &'static str {
        match self {
            Area::Home => "Start",
            Area::Catalog => "Module",
            Area::Programs => "Studium",
            Area::Bookmarks => "Merkliste",
            Area::Studyplan => "Stundenplan",
        }
    }
}

const ITEMS: &[(Area, &str, &str)] = &[
    (Area::Home, "/", "house"),
    (Area::Catalog, "/catalog", "layout-list"),
    (Area::Programs, "/programs", "graduation-cap"),
    (Area::Bookmarks, "/bookmarks", "bookmark"),
    (Area::Studyplan, "/studyplan", "calendar-range"),
];

#[component]
fn NavItems(area: Signal<Area>) -> impl IntoView {
    ITEMS
        .iter()
        .map(|&(item, href, icon)| {
            view! {
                <a class="nav" href=href title=item.title() aria-current=move || (area.get() == item).then_some("page")>
                    <span class="ind"><Icon name=icon/></span>{item.title()}
                </a>
            }
        })
        .collect_view()
}

/// The shell around `children` (the page). `area` is the rail's current tab; `status` what the
/// header says about the data (its download, a new snapshot), nothing while all is well.
#[component]
pub fn Shell(#[prop(into)] area: Signal<Area>, #[prop(into)] status: Signal<Option<String>>, children: Children) -> impl IntoView {
    view! {
        <div class="crown" aria-hidden="true"></div>
        <div class="wood" aria-hidden="true"></div>
        <aside class="rail sh-rail">
            <a class="logo hit" href="/" aria-label="Betula, Start"><Mark/></a>
            <nav aria-label="Navigation"><NavItems area/></nav>
        </aside>
        <div class="main">
            <header class="topbar sh-header">
                <div class="crumb"><h1>{move || area.get().title()}</h1></div>
                <span class="pill db-status" id="db-status" hidden=move || status.get().is_none()>{move || status.get()}</span>
            </header>
            <main class="content sh-content" id="content">
                {children()}
                <footer class="ground sh-ground">
                    <div class="ground-top">
                        <div><p class="ground-name"><Wordmark/><small>"Modulkatalog · inoffiziell"</small></p></div>
                        <nav class="ground-legal" aria-label="Rechtliches">
                            <a href="/impressum">"Impressum"</a>
                            <a href="/datenschutz">"Datenschutz"</a>
                        </nav>
                    </div>
                </footer>
            </main>
        </div>
        <nav class="bottomnav" aria-label="Navigation"><NavItems area/></nav>
    }
}
