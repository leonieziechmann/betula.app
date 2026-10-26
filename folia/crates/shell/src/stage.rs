//! Tabs that stay built (folia/design/tabwechsel/konzept.html, „Tabs, die stehen bleiben"; owner,
//! 2026-10-18: „Es ist nice zu sehen, dass die Seiten gleich da sind. Das muss auf jeden Fall
//! rein"). In the browser every tab has a stage in `main`, and its page stays built there while
//! Betula is open: a change of tab only swaps which stage is in front, without a skeleton and
//! without a frame's delay, and the page comes back as it was left, scrolled as far.
//!
//! - **A place for each stage.** The stage the router shows (`shown`) is where the router is; the
//!   others stay where they were left, the top of their tab's stack (`nav`). The pages read their
//!   stage's place (`folia_design::stage`, `i18n::use_location`), and a change within the tab
//!   (a filter, a module beside the list) changes the page on its stage as it always did.
//! - **In front** (`front`): the stage the router shows, or at once the stage that shows where the
//!   app is going (`Pending::to`): a tab whose stage stands there already comes in the frame of the
//!   tap, and the router follows (`Pending` takes the step at once, `nav` after the history is
//!   written). A stage that does not show it yet comes with the router, as a page did before.
//! - **Behind** is built and not drawn: `content-visibility: hidden`, and cut to nothing so that its
//!   box takes no click (app.css). Nothing there takes a finger or a reader (focus, find in page and
//!   the accessibility tree skip it), and the browser keeps what it worked out. Not `inert`: a
//!   browser works out the style of every element under a stage anew as it changes (Chrome: hundreds
//!   for a list), which a change of tab waited for, and on a phone the page that goes stays drawn
//!   while it moves away (the Achse, enhance.js). The document's title and the tags for search
//!   engines are those of the page in front.
//! - **The window's scroll on a phone** is the stage's: kept as it goes behind, given back in the
//!   frame it comes to the front. On a wide screen every page scrolls in its own areas, which keep
//!   their place.
//! - **Which stage** (`Key::of`): its tab's; a module's own page opened from a tab that shows its
//!   modules in place stays on that tab's stage (`nav::tab_of_page`, `Tabs`), and the pages of no
//!   tab (the profile, the legal pages, a page that does not exist) have a stage of their own.
//! - **Built at the start** (`prebuild`): after the first page, the other tabs' pages are built
//!   behind it while nothing happens, each where its tab leads; a stage stands once its answers are
//!   in (`ready`), and from then on its first visit only swaps as well.
//!
//! The server knows nothing of this: it renders the page of its address with `<Routes>`, in a
//! stage in front, so that the browser app's page stands as the server's does at the takeover.

use std::rc::Rc;

use folia_design::stage::{Stage, Titles};
use leptos::context::Provider;
use leptos::oco::Oco;
use leptos::prelude::*;
use leptos_router::location::Location;
use leptos_router::params::ParamsMap;

use crate::pending::Pending;
use crate::tabs::Area;

/// The stages: one for each tab, and one for the pages of none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Home,
    Catalog,
    Programs,
    Bookmarks,
    Studyplan,
    /// The profile, the legal pages, a page that does not exist: the one visited last stays.
    Other,
}

impl Key {
    pub const ALL: [Key; 6] = [Key::Home, Key::Catalog, Key::Programs, Key::Bookmarks, Key::Studyplan, Key::Other];

    /// Its name in the page (`data-stage`), for app.css, enhance.js and the checks.
    pub fn name(self) -> &'static str {
        match self {
            Key::Home => "home",
            Key::Catalog => "catalog",
            Key::Programs => "programs",
            Key::Bookmarks => "bookmarks",
            Key::Studyplan => "studyplan",
            Key::Other => "other",
        }
    }

    fn index(self) -> usize {
        match self {
            Key::Home => 0,
            Key::Catalog => 1,
            Key::Programs => 2,
            Key::Bookmarks => 3,
            Key::Studyplan => 4,
            Key::Other => 5,
        }
    }

    fn area(self) -> Option<Area> {
        match self {
            Key::Home => Some(Area::Home),
            Key::Catalog => Some(Area::Catalog),
            Key::Programs => Some(Area::Programs),
            Key::Bookmarks => Some(Area::Bookmarks),
            Key::Studyplan => Some(Area::Studyplan),
            Key::Other => None,
        }
    }

    fn of_area(area: Area) -> Self {
        match area {
            Area::Home => Key::Home,
            Area::Catalog => Key::Catalog,
            Area::Programs => Key::Programs,
            Area::Bookmarks => Key::Bookmarks,
            Area::Studyplan => Key::Studyplan,
        }
    }

    /// The stage of `path`, come to from the stage `from`: its tab's, but a module's own page opened
    /// from a tab that shows its modules in place stays on that tab's stage, and a page of no tab
    /// is on the stage of its own.
    pub fn of(path: &str, from: Key) -> Self {
        match Page::of(path) {
            Page::About | Page::Profile | Page::Event | Page::Imprint | Page::Privacy | Page::NotFound => Key::Other,
            Page::Module if from.area().is_some_and(Area::shows_in_place) => from,
            _ => Key::of_area(Area::of(path)),
        }
    }
}

/// The pages of the app, by the routes of `folia_app::App` (the server's `<Routes>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    /// `/about`, „Über Betula".
    About,
    Catalog,
    /// `/catalog/module/:id`
    Module,
    Programs,
    /// `/programs/:slug`
    Program,
    /// `/programs/:slug/:tab`
    ProgramTab,
    Bookmarks,
    Studyplan,
    Study,
    Profile,
    /// `/event/:key` and `/event/:key/:day`, a Termin's own page (folia/design/termin).
    Event,
    Imprint,
    Privacy,
    NotFound,
}

impl Page {
    /// The page `path` shows, as the routes match it.
    pub fn of(path: &str) -> Self {
        let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
        match segments.as_slice() {
            [] => Page::Home,
            ["about"] => Page::About,
            ["catalog"] => Page::Catalog,
            ["catalog", "module", _] => Page::Module,
            ["programs"] => Page::Programs,
            ["programs", _] => Page::Program,
            ["programs", _, _] => Page::ProgramTab,
            ["bookmarks"] => Page::Bookmarks,
            ["studyplan"] => Page::Studyplan,
            ["study"] => Page::Study,
            ["profile"] => Page::Profile,
            ["event", _] | ["event", _, _] => Page::Event,
            ["impressum"] => Page::Imprint,
            ["datenschutz"] => Page::Privacy,
            _ => Page::NotFound,
        }
    }

    /// The params its route takes from `path`, as the router has them (the segments as written).
    pub fn params(self, path: &str) -> ParamsMap {
        let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
        let mut params = ParamsMap::new();
        match (self, segments.as_slice()) {
            (Page::Module, [_, _, id]) => params.insert("id", (*id).to_string()),
            (Page::Program, [_, slug]) => params.insert("slug", (*slug).to_string()),
            (Page::ProgramTab, [_, slug, tab]) => {
                params.insert("slug", (*slug).to_string());
                params.insert("tab", (*tab).to_string());
            }
            (Page::Event, [_, key]) => params.insert("key", (*key).to_string()),
            (Page::Event, [_, key, day]) => {
                params.insert("key", (*key).to_string());
                params.insert("day", (*day).to_string());
            }
            _ => {}
        }
        params
    }
}

/// Where a stage is: path (without the language's prefix), query (without `?`) and its pairs, and
/// the fragment, as the router has them.
#[derive(Clone, Debug, PartialEq)]
struct Place {
    path: String,
    search: String,
    query: ParamsMap,
    hash: String,
}

impl Place {
    fn of(pathname: Memo<String>, search: Memo<String>, query: Memo<ParamsMap>, hash: Memo<String>) -> Self {
        Place { path: pathname.get(), search: search.get(), query: query.get(), hash: hash.get() }
    }

    /// The place of the address `to` (path, query and fragment as a link writes them), its query's
    /// pairs read as the router reads them (`UrlSearchParams`).
    #[cfg(feature = "csr")]
    fn parse(to: &str) -> Self {
        use leptos::wasm_bindgen::JsCast;
        let (without_fragment, hash) = match to.split_once('#') {
            Some((address, fragment)) => (address, format!("#{fragment}")),
            None => (to, String::new()),
        };
        let (path, search) = without_fragment.split_once('?').unwrap_or((without_fragment, ""));
        let query = leptos::web_sys::UrlSearchParams::new_with_str(search)
            .ok()
            .and_then(|params| leptos::web_sys::js_sys::try_iter(&params).ok().flatten())
            .map(|pairs| {
                pairs
                    .flatten()
                    .filter_map(|pair| {
                        let pair = pair.dyn_into::<leptos::web_sys::js_sys::Array>().ok()?;
                        Some((pair.get(0).as_string()?, pair.get(1).as_string()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Place { path: path.to_string(), search: search.to_string(), query, hash }
    }

    /// Whether this is the place of the address `to` (path, query and fragment as a link writes
    /// them): the same path and query.
    fn is(&self, to: &str) -> bool {
        let without_fragment = to.split('#').next().unwrap_or(to);
        let (path, search) = without_fragment.split_once('?').unwrap_or((without_fragment, ""));
        self.path == path && self.search == search
    }
}

/// The page on each stage: every route of the app as the stages show it.
pub type Pages = fn(Page) -> AnyView;

/// The stages in `main`, in the browser app: one for each tab and one for the pages of none, each
/// with the page of its place (`pages`), the one in front drawn.
#[component]
pub fn Stages(pages: Pages) -> impl IntoView {
    let router = folia_design::i18n::router_location();
    let (pathname, search, query, hash, state) = (router.pathname, router.search, router.query, router.hash, router.state);
    let pending = Pending::expect();
    // Where the router is, and which stage shows it.
    let here = Memo::new(move |_| Place::of(pathname, search, query, hash));
    let shown = Memo::new(move |before: Option<&Key>| Key::of(&pathname.get(), before.copied().unwrap_or(Key::Home)));
    let places: [RwSignal<Option<Place>>; 6] = std::array::from_fn(|_| RwSignal::new(None));
    let place = move |key: Key| places.get(key.index()).copied();
    // Whether a stage's page has its answers: one built at the start counts as standing only once
    // they are in (`prebuild`); the router's stage waits for its own with `Pending`.
    let ready: [RwSignal<bool>; 6] = std::array::from_fn(|_| RwSignal::new(true));
    let is_ready = move |key: Key| ready.get(key.index()).is_some_and(|ready| ready.get_untracked());
    // The page the app starts with is there with the first frame.
    if let Some(first) = place(shown.get_untracked()) {
        first.set(Some(here.get_untracked()));
    }
    // The stage the router shows is where the router is.
    Effect::new(move |_| {
        let (key, now) = (shown.get(), here.get());
        if let Some(slot) = place(key) {
            if slot.with_untracked(|was| was.as_ref() != Some(&now)) {
                slot.set(Some(now));
            }
        }
        if let Some(ready) = ready.get(key.index()).filter(|ready| !ready.get_untracked()) {
            ready.set(true);
        }
    });
    // Another stage than the router's that shows `to` already, with its answers: where a change
    // only swaps.
    let standing = move |to: &str| -> Option<Key> {
        let path = to.split(['?', '#']).next().unwrap_or(to);
        let from = shown.get_untracked();
        let key = Key::of(path, from);
        (key != from && is_ready(key) && place(key).is_some_and(|slot| slot.with_untracked(|place| place.as_ref().is_some_and(|place| place.is(to))))).then_some(key)
    };
    if let Some(pending) = pending {
        pending.bind_stages(Rc::new(move |to: &str| standing(to).is_some()));
    }
    // In front: where the app is going, if a stage shows it already; else the router's stage.
    let front = Memo::new(move |_| {
        let shown = shown.get();
        let going = pending.and_then(|pending| pending.to());
        // Tracked: a place the app is going to may come to stand on its stage meanwhile. Not its
        // answers: a tab tapped while it is built takes its step as before (`prebuild`).
        for key in Key::ALL {
            if let Some(slot) = place(key) {
                slot.track();
            }
        }
        going.and_then(|to| standing(&to)).unwrap_or(shown)
    });
    let nodes: [NodeRef<leptos::html::Div>; 6] = std::array::from_fn(|_| NodeRef::new());
    // The document's title is the one of the page in front (`folia_design::stage::Title`).
    let titles: [Titles; 6] = std::array::from_fn(|_| Titles::default());
    let title = move || titles.get(front.get().index()).map_or(Oco::Borrowed(""), |titles| titles.shown());
    let first = front.get_untracked();
    // How far the window was scrolled on each stage (a phone), as it scrolls: by the time another
    // stage comes to the front, the router may have taken the window to the top of the new page.
    let scrolled = StoredValue::new([None::<f64>; 6]);
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::scroll, move |_| {
            if !folia_design::nav::is_phone() {
                return;
            }
            let down = leptos::web_sys::window().and_then(|window| window.scroll_y().ok());
            let key = front.get_untracked();
            scrolled.update_value(|scrolled| {
                if let Some(slot) = scrolled.get_mut(key.index()) {
                    *slot = down;
                }
            });
        });
        on_cleanup(move || handle.remove());
    });
    // Where each stage was when it went behind: one that comes back there is scrolled as it was
    // left (at the top if it never scrolled); one at a new place is where the router puts it.
    let left = StoredValue::new(std::array::from_fn::<Option<Place>, 6, _>(|_| None));
    // The other tabs, built at the start while nothing happens (`prebuild`).
    #[cfg(feature = "csr")]
    prebuild::Builder {
        places,
        ready,
        left,
        scrolled,
        shown,
        router_path: pathname,
        pending,
        tabs: crate::tabs::Tabs::expect(),
        mine: folia_stores::myprogram::MineResolved::expect(),
        data: StoredValue::new_local(use_context::<folia_data::DataClient>()),
        down: StoredValue::new(0),
        last: StoredValue::new(0.0),
        turn: StoredValue::new(0),
    }
    .start();
    Effect::new(move |before: Option<Key>| {
        let now = front.get();
        let before = before.unwrap_or(first);
        let mut restore = None;
        // How far the stage that goes behind was scrolled, and whether `now` comes back where it was
        // left with its answers in, so that both pages stand (not a page the router has just built).
        let mut down = None;
        let mut step = false;
        if before != now {
            down = scrolled.with_value(|scrolled| scrolled.get(before.index()).copied().flatten());
            // Where `now` stands: the router's place if the router shows it, else its own.
            let at = if now == shown.get_untracked() { Some(here.get_untracked()) } else { place(now).and_then(|slot| slot.get_untracked()) };
            let back = at.is_some() && left.with_value(|left| left.get(now.index()).is_some_and(|was| *was == at));
            step = back && is_ready(now);
            scrolled.update_value(|scrolled| {
                if let Some(down) = scrolled.get_mut(now.index()) {
                    if back {
                        restore = Some(down.unwrap_or(0.0));
                    } else {
                        *down = None;
                    }
                }
            });
            let was = place(before).and_then(|slot| slot.get_untracked());
            left.update_value(|left| {
                if let Some(entry) = left.get_mut(before.index()) {
                    *entry = was;
                }
            });
        }
        #[cfg(feature = "csr")]
        browser::swap(before, now, &nodes, restore, down, step);
        #[cfg(not(feature = "csr"))]
        let _ = (restore, down, step);
        now
    });
    let stages = Key::ALL
        .into_iter()
        .filter_map(|key| {
            let slot = place(key)?;
            let node = nodes.get(key.index()).copied()?;
            let titles = titles.get(key.index()).copied()?;
            let location = Location {
                pathname: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.path.clone()).unwrap_or_default())),
                search: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.search.clone()).unwrap_or_default())),
                query: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.query.clone()).unwrap_or_default())),
                hash: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.hash.clone()).unwrap_or_default())),
                state,
            };
            let params = Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| Page::of(&place.path).params(&place.path)).unwrap_or_default()));
            let which = Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| Page::of(&place.path))));
            let in_front = Memo::new(move |_| front.get() == key);
            let stage = Stage { location, params, front: in_front.into(), node, titles };
            let answered = ready.get(key.index()).copied()?;
            Some(view! { <OneStage key stage which pages front=key == first answered/> })
        })
        .collect_view();
    view! {
        <leptos_meta::Title text=title/>
        {stages}
    }
}

/// A stage and the page of its place: the page is built anew when another page of the app comes to
/// the stage, not when its query changes (the router's routes did the same). The page has the
/// stage in an owner of its own (`Provider`): a component has none, and six stages given beside
/// each other would leave every page the last one.
#[component]
fn OneStage(key: Key, stage: Stage, which: Memo<Option<Page>>, pages: Pages, front: bool, answered: RwSignal<bool>) -> impl IntoView {
    let node = stage.node;
    view! {
        <Provider value=stage>
            // Busy while a page built at the start waits for its answers (`prebuild`).
            <div class=if front { "page-stage front" } else { "page-stage" } data-stage=key.name() aria-busy=move || (!answered.get()).then_some("true") node_ref=node>
                {move || which.get().map(|page| untrack(|| pages(page)))}
            </div>
        </Provider>
    }
}

/// The tabs built at the start (the concept, step 2; owner, 2026-10-18: „Am anfang darf die App
/// ruhig etwas laden/kurz Skeltons zeigen. Aber wenn die App dann einmal läuft, dann muss es sich
/// instant und einfach nur gut anfühlen"): once the app has taken the page over and its first
/// page's answers are in, Betula builds the other tabs' pages behind it, one after the other, the
/// cheapest first, each in a task of its own and only while nothing happens (no finger on the
/// screen, no key, wheel or scroll for a moment, no step on its way, no question of a page). Each at
/// the place its tab leads to (`chrome::tab_target`, `nav::tab_top`): its first visit only swaps,
/// as every later one does, and opens at its top. A stage counts as standing once its answers are
/// in (`ready`); a tab tapped before that comes as before, with a skeleton, and its page is the one
/// being built.
#[cfg(feature = "csr")]
mod prebuild {
    use std::time::Duration;

    use folia_data::DataClient;
    use folia_stores::myprogram::MineResolved;
    use leptos::prelude::*;
    use leptos::wasm_bindgen::closure::Closure;
    use leptos::wasm_bindgen::{JsCast, JsValue};
    use leptos::web_sys;

    use super::{Key, Place};
    use crate::pending::Pending;
    use crate::tabs::Tabs;

    /// The cheapest first: the Merkliste and the Stundenplan take a few tenths of a second of a
    /// phone's thread, Start more, Module and Studium the most (the concept, „Was das Vorbauen
    /// kostet").
    const ORDER: [Key; 5] = [Key::Bookmarks, Key::Studyplan, Key::Home, Key::Catalog, Key::Programs];
    /// How long nothing has to happen before a tab is built, in ms: no finger, key, wheel or scroll.
    const CALM_MS: f64 = 600.0;
    /// How long the browser may keep a tab waiting for an idle moment, in ms (one that is never
    /// idle, an animation running, still builds it then).
    const IDLE_MS: u64 = 2000;
    /// How long a tab's answers may take, in ms, before it counts as standing anyway.
    const ANSWERS_MS: f64 = 8000.0;
    /// How long a press may last, in ms, without anything heard of it, before it counts as over.
    const STALE_MS: f64 = 10000.0;

    #[derive(Clone, Copy)]
    pub(super) struct Builder {
        pub places: [RwSignal<Option<Place>>; 6],
        pub ready: [RwSignal<bool>; 6],
        pub left: StoredValue<[Option<Place>; 6]>,
        pub scrolled: StoredValue<[Option<f64>; 6]>,
        pub shown: Memo<Key>,
        pub router_path: Memo<String>,
        pub pending: Option<Pending>,
        pub tabs: Option<Tabs>,
        pub mine: Option<MineResolved>,
        pub data: StoredValue<Option<DataClient>, LocalStorage>,
        /// Fingers (and buttons) down, and when anything last happened (`performance.now()`).
        pub down: StoredValue<u32>,
        pub last: StoredValue<f64>,
        /// The tab whose idle moment is awaited: the idle callback and its timeout build it once.
        pub turn: StoredValue<usize>,
    }

    #[derive(Clone, Copy)]
    enum Phase {
        /// Until the app has taken the page over.
        Start,
        /// The tab `ORDER[i]`, once nothing happens.
        Calm(usize),
        /// The tab `ORDER[i]` built: until its answers are in (since when, quiet looks in a row).
        Answers(usize, f64, u8),
    }

    impl Builder {
        pub(super) fn start(self) {
            self.listen();
            self.later(Phase::Start, 300);
        }

        /// Hears what happens anywhere (capturing, so a scroll of a page's own area as well).
        fn listen(self) {
            let Some(window) = web_sys::window() else { return };
            let options = web_sys::AddEventListenerOptions::new();
            options.set_capture(true);
            options.set_passive(true);
            for (kind, finger) in [("pointerdown", Some(true)), ("pointerup", Some(false)), ("pointercancel", Some(false)), ("keydown", None), ("wheel", None), ("scroll", None)] {
                let listener = Closure::<dyn Fn()>::new(move || {
                    match finger {
                        Some(true) => {
                            let _ = self.down.try_update_value(|down| *down += 1);
                        }
                        Some(false) => {
                            let _ = self.down.try_set_value(0);
                        }
                        None => {}
                    }
                    let _ = self.last.try_set_value(now());
                });
                let _ = window.add_event_listener_with_callback_and_add_event_listener_options(kind, listener.as_ref().unchecked_ref(), &options);
                // As long as the app: the stages are never taken down.
                listener.forget();
            }
        }

        fn later(self, phase: Phase, ms: u64) {
            set_timeout(move || self.step(phase), Duration::from_millis(ms));
        }

        fn step(self, phase: Phase) {
            // The stages are gone (never while the app runs).
            if self.shown.try_get_untracked().is_none() {
                return;
            }
            match phase {
                Phase::Start => self.later(if app_runs() { Phase::Calm(0) } else { Phase::Start }, 200),
                Phase::Calm(i) => {
                    let Some(key) = ORDER.get(i).copied() else { return };
                    if !self.wanted(key) {
                        return self.later(Phase::Calm(i + 1), 0);
                    }
                    if !self.calm() {
                        return self.later(Phase::Calm(i), 250);
                    }
                    // In an idle moment where the browser tells one (Safari does not: a moment
                    // later then), at the latest after `IDLE_MS`; once either way.
                    let _ = self.turn.try_set_value(i + 1);
                    let build = move || {
                        if self.turn.try_get_value() == Some(i + 1) {
                            let _ = self.turn.try_set_value(0);
                            self.build(i);
                        }
                    };
                    let idle = request_idle_callback_with_handle(build).is_ok();
                    set_timeout(build, Duration::from_millis(if idle { IDLE_MS } else { 50 }));
                }
                Phase::Answers(i, since, quiet) => {
                    let quiet = if self.waiting() == 0 { quiet.saturating_add(1) } else { 0 };
                    if quiet >= 3 || now() - since > ANSWERS_MS {
                        if let Some(ready) = ORDER.get(i).and_then(|key| self.ready.get(key.index())) {
                            ready.set(true);
                        }
                        self.later(Phase::Calm(i + 1), 0);
                    } else {
                        self.later(Phase::Answers(i, since, quiet), 50);
                    }
                }
            }
        }

        /// Builds the tab `ORDER[i]` at the place its tab leads to, unless something happened
        /// meanwhile; then waits for its answers.
        fn build(self, i: usize) {
            let Some(key) = ORDER.get(i).copied() else { return };
            if !self.wanted(key) {
                return self.later(Phase::Calm(i + 1), 0);
            }
            if !self.calm() {
                return self.later(Phase::Calm(i), 250);
            }
            let Some(area) = key.area() else { return self.later(Phase::Calm(i + 1), 0) };
            let from = self.router_path.get_untracked();
            let top = untrack(|| crate::nav::tab_top(area, &crate::chrome::tab_target(self.tabs, self.mine, area, &from)));
            let place = Place::parse(&top);
            if Key::of(&place.path, key) != key {
                return self.later(Phase::Calm(i + 1), 0);
            }
            // As if it was left there at its top: its first visit opens it there (`Stages`).
            self.left.update_value(|left| {
                if let Some(entry) = left.get_mut(key.index()) {
                    *entry = Some(place.clone());
                }
            });
            self.scrolled.update_value(|scrolled| {
                if let Some(down) = scrolled.get_mut(key.index()) {
                    *down = None;
                }
            });
            if let Some(ready) = self.ready.get(key.index()) {
                ready.set(false);
            }
            if let Some(slot) = self.places.get(key.index()) {
                slot.set(Some(place));
            }
            self.later(Phase::Answers(i, now(), 0), 50);
        }

        /// Whether `key` is still to be built: not the router's stage, and nothing on it yet.
        fn wanted(self, key: Key) -> bool {
            key != self.shown.get_untracked() && self.places.get(key.index()).is_some_and(|slot| slot.with_untracked(Option::is_none))
        }

        /// Whether nothing happens: no finger on the screen, nothing for a moment, no step on its
        /// way, no question of a page. A press nothing was heard of for `STALE_MS` is over (a
        /// button let go outside the window sends no `pointerup`).
        fn calm(self) -> bool {
            let since = now() - self.last.try_get_value().unwrap_or(0.0);
            let still = (self.down.try_get_value() == Some(0) || since > STALE_MS) && since > CALM_MS;
            let stepping = self.pending.is_some_and(|pending| untrack(|| pending.busy())) || crate::nav::rewriting();
            still && !stepping && self.waiting() == 0
        }

        /// How many questions of the pages are on their way.
        fn waiting(self) -> usize {
            self.data.try_with_value(|data| data.as_ref().map_or(0, |data| untrack(|| data.waiting()))).unwrap_or(0)
        }
    }

    /// The app has taken the page over (`boot.js`, `__betulaApp`): its first page's answers are in.
    fn app_runs() -> bool {
        web_sys::window().and_then(|window| web_sys::js_sys::Reflect::get(&window, &JsValue::from_str("__betulaApp")).ok()).is_some_and(|runs| runs.as_bool() == Some(true))
    }

    fn now() -> f64 {
        web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now())
    }
}

#[cfg(feature = "csr")]
mod browser {
    use leptos::prelude::*;
    use leptos::wasm_bindgen::{JsCast, JsValue};
    use leptos::web_sys;

    use super::Key;

    /// The stage `now` comes to the front, `before` goes behind and is not drawn, and on a phone
    /// the window scrolled to `restore` (where `now` was left, if it comes back there), in this
    /// frame. On a phone the stage that goes behind keeps how far its page was scrolled (`down`)
    /// and how high it was (`data-scroll`, `data-height`): enhance.js draws it so while a finger on
    /// the bar brings it back (the Achse). `step`: `now` comes back where it was left, its answers
    /// in, so both pages stand. enhance.js hears of the change before it (`betula:stage-going`:
    /// the page that goes becomes a layer of its own while it is still drawn; hidden and drawn
    /// again in the same task, a stage's every element works out its style anew) and after it
    /// (`betula:stage`).
    pub(super) fn swap(before: Key, now: Key, nodes: &[NodeRef<leptos::html::Div>; 6], restore: Option<f64>, down: Option<f64>, step: bool) {
        let element = |key: Key| nodes.get(key.index()).and_then(|node| node.get_untracked()).map(|node| node.unchecked_into::<web_sys::Element>());
        let window = web_sys::window();
        if before != now {
            announce(window.as_ref(), "betula:stage-going", before, now, step);
            let phone = folia_design::nav::is_phone();
            if let Some(stage) = element(before) {
                // The page that goes behind is left, as a new address left it: a dialog open on it
                // goes with it (its `close` tells the page).
                if let Ok(open) = stage.query_selector_all("dialog[open]") {
                    for dialog in (0..open.length()).filter_map(|i| open.item(i)?.dyn_into::<web_sys::HtmlDialogElement>().ok()) {
                        dialog.close();
                    }
                }
                if phone {
                    let down = down.or_else(|| window.as_ref().and_then(|window| window.scroll_y().ok())).unwrap_or(0.0);
                    let _ = stage.set_attribute("data-scroll", &down.to_string());
                    // A stage on its way (`moving`) is as high as the screen, not as its page.
                    if !stage.class_list().contains("moving") {
                        let _ = stage.set_attribute("data-height", &stage.get_bounding_client_rect().height().to_string());
                    }
                }
                let _ = stage.class_list().remove_1("front");
            }
            if let Some(stage) = element(now) {
                let _ = stage.class_list().add_1("front");
            }
            if let (true, Some(window), Some(top)) = (phone, window.as_ref(), restore) {
                let options = web_sys::ScrollToOptions::new();
                options.set_top(top);
                options.set_behavior(web_sys::ScrollBehavior::Instant);
                window.scroll_to_with_scroll_to_options(&options);
            }
        }
        folia_design::stage::set_front(element(now));
        // enhance.js follows the page in front (its scroll area, „Nach oben", a sheet left open),
        // and on a phone moves the two pages where both stand.
        if before != now {
            announce(window.as_ref(), "betula:stage", before, now, step);
        }
    }

    /// Tells enhance.js of a change of stage (`name`): from which stage to which (`from`, `to`), and
    /// whether both pages stand (`step`).
    fn announce(window: Option<&web_sys::Window>, name: &str, before: Key, now: Key, step: bool) {
        let Some(window) = window else { return };
        let detail = web_sys::js_sys::Object::new();
        for (key, value) in [("from", JsValue::from_str(before.name())), ("to", JsValue::from_str(now.name())), ("step", JsValue::from_bool(step))] {
            let _ = web_sys::js_sys::Reflect::set(&detail, &JsValue::from_str(key), &value);
        }
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
            let _ = window.dispatch_event(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route of the app's is a page here, and what is none is the page that does not exist.
    #[test]
    fn the_page_of_a_path() {
        assert_eq!(Page::of("/"), Page::Home);
        assert_eq!(Page::of("/about"), Page::About);
        assert_eq!(Page::of("/catalog"), Page::Catalog);
        assert_eq!(Page::of("/catalog/"), Page::Catalog);
        assert_eq!(Page::of("/catalog/module/11103"), Page::Module);
        assert_eq!(Page::of("/catalog/module"), Page::NotFound);
        assert_eq!(Page::of("/programs"), Page::Programs);
        assert_eq!(Page::of("/programs/bachelor-informatik-2008"), Page::Program);
        assert_eq!(Page::of("/programs/bachelor-informatik-2008/areas"), Page::ProgramTab);
        assert_eq!(Page::of("/programs/a/b/c"), Page::NotFound);
        assert_eq!((Page::of("/bookmarks"), Page::of("/studyplan"), Page::of("/study")), (Page::Bookmarks, Page::Studyplan, Page::Study));
        assert_eq!((Page::of("/profile"), Page::of("/impressum"), Page::of("/datenschutz")), (Page::Profile, Page::Imprint, Page::Privacy));
        assert_eq!(Page::of("/nichts"), Page::NotFound);
        assert_eq!(Page::Module.params("/catalog/module/11103").get("id").as_deref(), Some("11103"));
        let program = Page::ProgramTab.params("/programs/bachelor-informatik-2008/areas");
        assert_eq!((program.get("slug").as_deref(), program.get("tab").as_deref()), (Some("bachelor-informatik-2008"), Some("areas")));
        assert_eq!(Page::Program.params("/programs/x").get("tab"), None);
        assert_eq!((Page::of("/event/148370-eb017"), Page::of("/event/148370-eb017/2026-11-02"), Page::of("/event")), (Page::Event, Page::Event, Page::NotFound));
        let event = Page::Event.params("/event/148370-eb017/2026-11-02");
        assert_eq!((event.get("key").as_deref(), event.get("day").as_deref()), (Some("148370-eb017"), Some("2026-11-02")));
        assert_eq!(Page::Event.params("/event/148370-eb017").get("day"), None);
    }

    /// A page is on its tab's stage; a module's own page stays on a tab that shows its modules in
    /// place; the pages of no tab have a stage of their own.
    #[test]
    fn the_stage_of_a_page() {
        assert_eq!(Key::of("/", Key::Catalog), Key::Home);
        assert_eq!(Key::of("/catalog", Key::Home), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Catalog), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Home), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Bookmarks), Key::Bookmarks);
        assert_eq!(Key::of("/catalog/module/11103", Key::Programs), Key::Programs);
        assert_eq!(Key::of("/catalog/module/11103", Key::Studyplan), Key::Studyplan);
        assert_eq!(Key::of("/catalog/module/11103", Key::Other), Key::Catalog);
        assert_eq!((Key::of("/study", Key::Home), Key::of("/programs/x/plan", Key::Catalog)), (Key::Programs, Key::Programs));
        assert_eq!((Key::of("/profile", Key::Catalog), Key::of("/impressum", Key::Home), Key::of("/nichts", Key::Home), Key::of("/about", Key::Home)), (Key::Other, Key::Other, Key::Other, Key::Other));
        assert_eq!(Key::of("/event/148370-eb017/2026-11-02", Key::Catalog), Key::Other);
    }

    #[test]
    fn a_place_and_an_address() {
        let place = Place { path: "/catalog".into(), search: "turnus=winter".into(), query: ParamsMap::new(), hash: String::new() };
        assert!(place.is("/catalog?turnus=winter") && place.is("/catalog?turnus=winter#x"));
        assert!(!place.is("/catalog") && !place.is("/catalog?turnus=sommer") && !place.is("/bookmarks?turnus=winter"));
    }
}
