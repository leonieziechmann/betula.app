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
pub mod ground;
pub mod i18n;
pub mod icons;
pub mod languages;
pub mod launch;
pub mod local;
pub mod myprogram;
pub mod nav;
pub mod pages;
pub mod pending;
pub mod seo;
pub mod skeleton;
pub mod studyplan;
pub mod swipe;
pub mod tabs;
pub mod ui;
pub mod week;

use catalog::url;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{path, NavigateOptions, SsrMode};

use crate::bookmarks::Bookmarks;
use crate::ground::{Crown, Ground, Wood};
use crate::i18n::use_location;
use crate::myprogram::{MineResolved, MyProgram};
use crate::pages::bookmarks::BookmarksPage;
use crate::pages::legal::{ImprintPage, PrivacyPage};
use crate::pages::studyplan::StudyplanPage;
use crate::pages::{catalog::CatalogPage, home::HomePage, module::ModulePage, program::ProgramPage, programs::ProgramsPage};
use crate::pending::Pending;
use crate::studyplan::Studyplan;
use crate::tabs::{Area, Tabs};
use crate::ui::Icon;

/// The release of Folia as the owner names it (2026-09-21: Folia and Radix are both
/// alpha-0.2.0; 2026-09-22: Folia alpha-0.2.1 with the phone's filter sheet; 2026-09-23:
/// Folia alpha-0.2.2, Radix alpha-0.3.0; 2026-09-27, after the public release: Folia 1.0.1,
/// Radix 0.5.0, no stage in front any more; 2026-09-29: Folia 1.0.2, the same day 1.0.3 with the
/// wood behind every page): this crate's version, the same as the server's.
/// Radix's is in the snapshot (`Meta::radix_version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the host serves the files of `app/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
/// The pictures for what cannot read the SVG (`design/logo/render-icons.mjs` makes them): the
/// classic `/favicon.ico`, the mark like the SVG; and the icon of the installed app, the birch leaf
/// that carries the mark's bars (`design/logo/app-icon.mjs`): the icon of iOS (home screen, link
/// previews of Messages) and the icons of the web app manifest, plain ones for desktops, maskable
/// ones for Android's launchers and the splash screen of the installed app (the large one keeps it
/// sharp there), and the monochrome one that Android's themed icons tint in the colours of the
/// wallpaper. The site keeps its mark; the leaf stands whole in whatever shape a launcher cuts.
pub const FAVICON_ICO: &str = "/favicon.ico";
pub const TOUCH_ICON: &str = "/apple-touch-icon.png";
pub const ICON_192: &str = "/assets/icon-192.png";
pub const ICON_512: &str = "/assets/icon-512.png";
pub const ICON_MASKABLE: &str = "/assets/icon-maskable-512.png";
pub const ICON_MASKABLE_LARGE: &str = "/assets/icon-maskable-1024.png";
pub const ICON_MONOCHROME: &str = "/assets/icon-monochrome-512.png";
/// Name, colours and icons of the site for a home screen or an installed window.
pub const MANIFEST: &str = "/manifest.webmanifest";
/// The page background of the light and the dark theme (`--bg`), for the browser's own chrome.
pub const THEME_LIGHT: &str = "#f1f2f4";
pub const THEME_DARK: &str = "#0a0c11";
pub const FONT: &str = "/assets/inter-latin.woff2";
/// The picture of link previews (1200 × 630, made from `design/og/og.html`).
pub const OG_IMAGE: &str = "/assets/og.png";
/// The screenshots of the start page's carousel: `<SHOTS>/<name>[-phone][-dark].webp`.
pub const SHOTS: &str = "/assets/shots";
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

/// Runs before the first paint: marks the document as scripted, names the season the birch is
/// drawn in (`data-season`: March–May spring, June–August summer, September–November autumn,
/// December–February winter; the server's page is the same all year, R9), and applies what this browser
/// remembers (theme, widths of the filter panel and the module preview), so nothing flashes or jumps; the
/// colour of the browser's own chrome (`theme-color`, `THEME_DARK`) follows the theme. Such personal
/// view settings live in localStorage, never in the URL and never in server HTML (R9). A browser
/// that keeps „Mein Studiengang" marks the document with `mine`: the program overview's line about
/// it is the app's, and the page keeps its room from the first paint, so the list does not move
/// when the app takes over (R15). On an iPhone or iPad (`navigator.standalone` exists there only)
/// it names the launch screens of this screen, upright and, on a tablet, turned, light and dark
/// (`launch`): iOS takes them when the app is added to the home screen, from the page as it is
/// then. Written here and not as sixty `<link>`s into every page, which would cost every visitor
/// almost a kilobyte for what only a home screen of iOS reads. Public for the one document the
/// server writes without the app: the login page of closed testing.
pub const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');var n=new Date().getMonth();d.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn';try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;else t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';if(t==='dark'){var m=document.querySelector('meta[name=theme-color]');if(m)m.content='#0a0c11'}var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px');if(/^program\\t[0-9A-Za-z]/m.test(localStorage.getItem('betula.myprogram.v1')||''))d.classList.add('mine')}catch(e){}if('standalone'in navigator)(function(){var s=screen,r=Math.round(devicePixelRatio),a=Math.min(s.width,s.height)*r,b=Math.max(s.width,s.height)*r;(a<1400?[[a,b,'portrait']]:[[a,b,'portrait'],[b,a,'landscape']]).forEach(function(o){['light','dark'].forEach(function(c){var l=document.createElement('link');l.rel='apple-touch-startup-image';l.media='(orientation: '+o[2]+') and (prefers-color-scheme: '+c+')';l.href='/assets/launch/'+o[0]+'x'+o[1]+(c==='dark'?'-dark':'')+'.png';document.head.appendChild(l)})})})()";

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
                    <Routes fallback=move || view! { <ui::Plain><ui::NotFound title=t.app.not_found_title hint=t.app.not_found_hint/></ui::Plain> }>
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
        // there alone, so the number is never part of server HTML (R9).
        <a class="nav js-only" data-area="bookmarks" href=move || href(Area::Bookmarks) title=t.app.bookmarks aria-current=move || current(Area::Bookmarks)>
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
        <a class="nav js-only" data-area="studyplan" href=move || href(Area::Studyplan) title=t.app.studyplan aria-current=move || current(Area::Studyplan)>
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
