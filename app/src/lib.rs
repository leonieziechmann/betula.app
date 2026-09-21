//! The web app of Betula (the pages of Folia). The same components render on the server (complete HTML for
//! search engines, shared links and browsers without JavaScript) and in the browser.
//!
//! Rules (docs/frontend.md §2): navigation state comes from the router as plain values (R1);
//! server HTML never depends on the user (R9); no panics (R3, enforced by the clippy lints of
//! this crate); styling only through the classes of `assets/app.css` (R7).

// Leptos view types nest deeply.
#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod bookmarks;
pub mod combobox;
pub mod data;
pub mod format;
pub mod icons;
pub mod nav;
pub mod pages;
pub mod seo;
pub mod tabs;
pub mod ui;

use catalog::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::{path, NavigateOptions, SsrMode};

use crate::bookmarks::Bookmarks;
use crate::pages::bookmarks::BookmarksPage;
use crate::pages::{catalog::CatalogPage, home::HomePage, module::ModulePage, program::ProgramPage, programs::ProgramsPage};
use crate::tabs::{Area, Tabs};
use crate::ui::Icon;

/// Where the host serves the files of `app/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
/// The mark as pictures, for what cannot read the SVG (`design/logo/render-icons.mjs` makes them):
/// the classic `/favicon.ico`, the icon of iOS (home screen, link previews of Messages) and the
/// icons of the web app manifest.
pub const FAVICON_ICO: &str = "/favicon.ico";
pub const TOUCH_ICON: &str = "/apple-touch-icon.png";
pub const ICON_192: &str = "/assets/icon-192.png";
pub const ICON_512: &str = "/assets/icon-512.png";
pub const ICON_MASKABLE: &str = "/assets/icon-maskable-512.png";
/// Name, colours and icons of the site for a home screen or an installed window.
pub const MANIFEST: &str = "/manifest.webmanifest";
/// The page background of the light and the dark theme (`--bg`), for the browser's own chrome.
pub const THEME_LIGHT: &str = "#f1f2f4";
pub const THEME_DARK: &str = "#0a0c11";
pub const FONT: &str = "/assets/inter-latin.woff2";
/// The picture of link previews (1200 × 630, made from `design/og/og.html`).
pub const OG_IMAGE: &str = "/assets/og.png";
pub const ENHANCE_SCRIPT: &str = "/assets/enhance.js";
/// Loads the local database and the browser app, which then takes the page over.
pub const BOOT_SCRIPT: &str = "/assets/boot.js";
/// The service worker: keeps the shell of the app for a start without a network (`boot.js`
/// registers it; the web server writes its build into it).
pub const SERVICE_WORKER: &str = "/sw.js";

/// The build of the server that writes the page (given by the host, server side only). The
/// document links the stylesheet and the scripts with it (`/assets/app.css?v=<build>`), and
/// `boot.js` hands the same `?v=` on to the bundle and to sql.js. A service worker of another
/// build has nothing under such an address and asks the network, so a page always gets the
/// stylesheet, the scripts and the bundle of its own build — also on the first load after a
/// deploy, which the worker of the old build still answers. Every visitor gets the same build,
/// so the server's HTML stays the same for everybody (R9).
#[derive(Clone)]
pub struct BuildId(pub std::sync::Arc<str>);

impl BuildId {
    /// The address under which a page of this build asks for `path`.
    pub fn asset(&self, path: &str) -> String {
        format!("{path}?v={}", self.0)
    }
}

/// `path` as the page being rendered links it: with the build its host gave (`BuildId`), plain
/// where no host gave one.
pub fn asset(path: &str) -> String {
    use_context::<BuildId>().map_or_else(|| path.to_string(), |build| build.asset(path))
}

/// Runs before the first paint: marks the document as scripted and applies what this browser
/// remembers (theme, widths of the filter panel and the module preview), so nothing flashes or jumps; the
/// colour of the browser's own chrome (`theme-color`, `THEME_DARK`) follows the theme. Such personal
/// view settings live in localStorage, never in the URL and never in server HTML (R9). Public for
/// the one document the server writes without the app: the login page of closed testing.
pub const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;else t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';if(t==='dark'){var m=document.querySelector('meta[name=theme-color]');if(m)m.content='#0a0c11'}var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px')}catch(e){}";

/// The opt-in to the fade between pages, in the head of every document the server writes (this
/// shell and the login page of closed testing). Not in app.css: Chromium decides whether a new
/// page takes part in the transition when it shows it for the first time, from the style sheets it
/// has applied by then, and the stylesheet, revalidated on every page load, often arrives after the
/// parser has reached `<body>`. The page then came without the fade and with "Transition was
/// aborted because of invalid state. ViewTransition opt-in disabled" in the console. Written
/// inline, the rule is there before the body is. The duration stays in app.css.
pub const VIEW_TRANSITION_STYLE: &str = "@view-transition{navigation:auto}";

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
                // Who the site is, for tabs, home screens and link previews. Static and the same
                // on every page, so it lives here and survives the takeover by the browser app.
                <meta name="application-name" content=seo::SITE_NAME/>
                <meta name="apple-mobile-web-app-title" content=seo::SITE_NAME/>
                // One tag: the head script turns it dark with the theme (crawlers read the light one).
                <meta name="theme-color" content=THEME_LIGHT/>
                <link rel="icon" href=FAVICON_ICO sizes="32x32"/>
                <link rel="icon" type="image/svg+xml" href=FAVICON/>
                <link rel="apple-touch-icon" href=TOUCH_ICON/>
                <link rel="manifest" href=MANIFEST/>
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
    provide_meta_context();
    Tabs::provide();
    // The visitor's marked modules: from this browser's storage, empty on the server (R9).
    Bookmarks::provide();
    view! {
        // Description, canonical address and the rest of what search engines read belong to the
        // page (`seo::Seo`), not to the app: a page must not carry two descriptions.
        <Title formatter=|title: String| if title.is_empty() { "Modulkatalog der BTU Cottbus-Senftenberg · Betula (inoffiziell)".to_string() } else { format!("{title} · Betula") }/>
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
                        <Route path=path!("/bookmarks") view=BookmarksPage ssr=SsrMode::Async/>
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
        // The marked modules exist in the browser app only (R15). How many there are is known
        // there alone, so the number is never part of server HTML (R9).
        <a class="nav js-only" data-area="bookmarks" href=move || href(Area::Bookmarks) title="Merkliste" aria-current=move || current(Area::Bookmarks)>
            <span class="ind">
                <Icon name="bookmark"/>
                {move || {
                    let marked = Bookmarks::expect().map(|bookmarks| bookmarks.count()).unwrap_or(0);
                    (marked > 0).then(|| view! { <span class="nav-count num" aria-label=format!("{marked} gemerkt")>{if marked > 99 { "99+".to_string() } else { marked.to_string() }}</span> })
                }}
            </span>
            "Merkliste"
        </a>
    }
}

#[component]
fn Rail() -> impl IntoView {
    view! {
        <aside class="rail">
            <a class="logo hit" href=url::HOME aria-label="Betula, zur Startseite"><ui::Mark/></a>
            <nav aria-label="Hauptnavigation"><NavItems/></nav>
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
                    Area::Bookmarks => ("Merkliste", url::CATALOG, "Modul, Nummer oder Thema suchen"),
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
