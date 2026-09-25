//! Paint first, then work (owner, 2026-09-23: switching pages and changing filters must not feel
//! slow, and where a result still takes a moment, skeletons should bridge it).
//!
//! A page of the app is a synchronous function of its address (docs/frontend.md, „Data flow"):
//! the router takes a click, and the queries and the new page run in the same task, before the
//! browser may show anything. Until they are done the click shows nothing at all, 50 ms on a
//! laptop and up to 800 ms on a phone. So the app lets a navigation reach the router one frame
//! later, and what can be shown at once is shown in that frame:
//!
//! - what was clicked is in its new state (the tab of the rail, a toggle of the filter panel, the
//!   row whose module opens), because those parts read where the app is going (`Pending::to`);
//! - what has to be computed stands there as a skeleton (`skeleton`), wherever the wait is
//!   expected to be seen: a change of the same kind took at least `SLOW_MS` the last time.
//!   A result that comes quicker is not preceded by a flash of grey.
//!
//! Then the router takes the address, and the page with it replaces the skeleton in one frame.
//! That costs the result at most one frame against before, and the click answers within one.
//!
//! What is taken over: clicks on links the router would take (the same checks), Back and Forward
//! (the browser's `popstate`, handed to the router again a frame later, `REPLAY`), and what the
//! app starts itself through `Pending::go` (the pickers, the credit slider, the draft of the
//! phone's filter sheet; the search of the top bar through `go_quietly`, without skeletons). A
//! link or a step whose address changes nothing the visitor sees (the fragment, the list's
//! `page`) goes to the router as before.

use std::collections::HashMap;
use std::rc::Rc;

use catalog::url::{self, BookmarksUrl, CatalogUrl, LocalView, PlanView, ProgramTab, ProgramUrl, StudyplanUrl};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::location::Location;
use leptos_router::NavigateOptions;

use crate::studyplan::PlanAddress;

/// A change of this kind that took this long the last time (smoothed) gets a skeleton: below it,
/// the result comes about as soon as a skeleton would, and the skeleton would only flash.
pub const SLOW_MS: f64 = 50.0;

/// Set on a `popstate` event the app hands to the router again, so that neither the app's own
/// listener nor `enhance.js` take it for a new step.
pub const REPLAY: &str = "betulaReplay";

/// The page whose frame a skeleton shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Home,
    Catalog,
    Module,
    Programs,
    Program,
    Bookmarks,
    /// The Studienplan: a semester of the visitor's plan, or all of them.
    Studyplan,
    /// Text: the legal pages, a page that does not exist.
    Text,
}

impl Shape {
    fn of(path: &str) -> Self {
        if path == url::HOME {
            Shape::Home
        } else if path == url::CATALOG {
            Shape::Catalog
        } else if path.starts_with("/catalog/module/") {
            Shape::Module
        } else if path == url::PROGRAMS {
            Shape::Programs
        } else if path.starts_with("/programs/") {
            Shape::Program
        } else if path == url::BOOKMARKS {
            Shape::Bookmarks
        } else if path == url::STUDYPLAN {
            Shape::Studyplan
        } else {
            Shape::Text
        }
    }
}

/// What a navigation changes, and so what waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    /// Another page (another area, another module or program): its frame stands in for it.
    Page(Shape),
    /// The same page shows something else in its column: another view of the program, the program
    /// overview or the marked modules filtered or ordered otherwise, another semester or view of
    /// the Studienplan. The sidebar stays.
    Column(Shape),
    /// The catalog's list: another filter, order or search. The filter panel shows the new
    /// filter at once, the rows wait.
    List,
    /// The module beside the catalog's list or beside the marked modules, opened or closed.
    Preview,
    /// What stands beside a program's page (a module, an area, a row of the plan) or beside the
    /// Studienplan (a planned module with its Termine).
    Aside,
}

/// How a navigation reaches the router when its turn comes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Via {
    Link { replace: bool, scroll: bool },
    /// Back or Forward: the browser has already moved; the router hears it now.
    History,
}

type Navigate = Rc<dyn Fn(&str, NavigateOptions)>;

struct Inner {
    via: Via,
    /// Set by `Bind` inside the router: its navigation and where it is.
    navigate: Option<Navigate>,
    location: Option<Location>,
    /// The turn that is due; a newer navigation takes the place of an older one.
    turn: u64,
    due: bool,
    /// How long the router took for a change of each kind the last times, in ms (smoothed).
    took: HashMap<Change, f64>,
}

/// The navigation that is on its way to the router, provided by `App` for the whole app. Empty on
/// the server, which never navigates.
#[derive(Clone, Copy)]
pub struct Pending {
    to: RwSignal<Option<String>>,
    change: RwSignal<Option<Change>>,
    slow: RwSignal<bool>,
    inner: StoredValue<Inner, LocalStorage>,
}

impl Pending {
    /// Provides the context and, in the browser, takes clicks and `popstate` before the router
    /// does: call it before `<Router>` is built, whose own listeners come after these then.
    pub fn provide() -> Self {
        let pending = Pending {
            to: RwSignal::new(None),
            change: RwSignal::new(None),
            slow: RwSignal::new(false),
            inner: StoredValue::new_local(Inner { via: Via::Link { replace: false, scroll: true }, navigate: None, location: None, turn: 0, due: false, took: HashMap::new() }),
        };
        provide_context(pending);
        #[cfg(feature = "csr")]
        browser::listen(pending);
        pending
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Where the app is going (path, query and fragment, as a link writes it), while it is on its way.
    pub fn to(&self) -> Option<String> {
        self.to.get()
    }

    /// What the navigation on its way changes.
    pub fn change(&self) -> Option<Change> {
        self.change.get()
    }

    /// Whether the parts that wait for `change` show a skeleton now: it is on its way, and its
    /// kind took long enough the last time to be seen.
    pub fn waits(&self, change: Change) -> bool {
        self.slow.get() && self.change.get() == Some(change)
    }

    /// Whether anything is on its way.
    pub fn busy(&self) -> bool {
        self.to.with(Option::is_some)
    }

    /// The search of the address the app is going to, if it goes to `path` (without `?`).
    pub fn search_on(&self, path: &str) -> Option<String> {
        self.to.with(|to| to.as_deref().filter(|to| path_of(to) == path).map(|to| search_of(to).to_string()))
    }

    /// The path of the address the app is going to.
    pub fn path(&self) -> Option<String> {
        self.to.with(|to| to.as_deref().map(|to| path_of(to).to_string()))
    }

    /// Where the visitor is headed, else where the router is: path and query (without `?`),
    /// read without tracking (for event handlers that build the next address on top of it).
    pub fn shown_untracked(pending: Option<Self>, location: &Location) -> (String, String) {
        Self::shown_of(pending, location.pathname, location.search)
    }

    /// `shown_untracked` for a closure that holds the router's memos rather than its location.
    pub fn shown_of(pending: Option<Self>, pathname: Memo<String>, search: Memo<String>) -> (String, String) {
        match pending.and_then(|pending| pending.to.get_untracked()) {
            Some(to) => (path_of(&to).to_string(), search_of(&to).to_string()),
            None => (pathname.get_untracked(), search.get_untracked()),
        }
    }

    /// Navigation started by the app itself (a picker, the slider): like a click on a link to
    /// `to`. Nothing waits for what changes nothing, it goes to the router at once.
    pub fn go(&self, to: &str, options: NavigateOptions) {
        self.go_with(to, options, false);
    }

    /// What the visitor is typing (the search of the top bar): the same, but the result there is
    /// stays until the next one comes, without a skeleton, which would flicker with every letter.
    pub fn go_quietly(&self, to: &str, options: NavigateOptions) {
        self.go_with(to, options, true);
    }

    fn go_with(&self, to: &str, options: NavigateOptions, quiet: bool) {
        let via = Via::Link { replace: options.replace, scroll: options.scroll };
        match self.change_to(to) {
            Some(change) => self.start(to.to_string(), via, change, quiet),
            None => {
                if let Some(navigate) = self.inner.with_value(|inner| inner.navigate.clone()) {
                    navigate(to, options);
                }
            }
        }
    }

    /// What going from where the router is to `to` changes; `None` without the router.
    fn change_to(&self, to: &str) -> Option<Change> {
        let from = self.inner.with_value(|inner| inner.location.as_ref().map(|location| (location.pathname.get_untracked(), location.search.get_untracked())))?;
        change(&from.0, &from.1, path_of(to), search_of(to), crate::nav::is_phone())
    }

    fn start(&self, to: String, via: Via, change: Change, quiet: bool) {
        // A step back or forward that has not reached the router yet goes first, so that its
        // idea of the history stays right. Not when this is another such step: the router reads
        // where the browser is when it hears of it, and handing it the first one from inside the
        // listener of the second would call that listener again while it runs.
        if via != Via::History && self.inner.with_value(|inner| inner.due && inner.via == Via::History) {
            self.commit();
        }
        let slow = !quiet && self.inner.with_value(|inner| inner.took.get(&change).is_none_or(|took| *took >= SLOW_MS));
        self.inner.update_value(|inner| inner.via = via);
        self.to.set(Some(to));
        self.change.set(Some(change));
        self.slow.set(slow);
        let turn = self.inner.with_value(|inner| inner.turn).wrapping_add(1);
        self.inner.update_value(|inner| {
            inner.turn = turn;
            inner.due = true;
        });
        // A newer navigation takes the place of this one: it runs only while it is still due.
        let pending = *self;
        crate::nav::after_paint(move || {
            if pending.inner.with_value(|inner| inner.due && inner.turn == turn) {
                pending.commit();
            }
        });
    }

    /// The router takes the address now; the skeleton goes in the same frame as the page comes.
    fn commit(&self) {
        let Some(to) = self.to.get_untracked() else { return };
        let (via, navigate) = self.inner.with_value(|inner| (inner.via, inner.navigate.clone()));
        let change = self.change.get_untracked();
        self.inner.update_value(|inner| inner.due = false);
        // What replaces a skeleton comes as it is: without the entrance of a panel that opens.
        #[cfg(feature = "csr")]
        if self.slow.get_untracked() {
            browser::settle();
        }
        #[cfg(feature = "csr")]
        let started = browser::now();
        match via {
            Via::Link { replace, scroll } => {
                if let Some(navigate) = navigate {
                    navigate(&to, NavigateOptions { replace, scroll, ..Default::default() });
                }
            }
            Via::History => {
                #[cfg(feature = "csr")]
                browser::replay_popstate();
            }
        }
        self.to.set(None);
        self.change.set(None);
        self.slow.set(false);
        // How long it took: until everything the router set off has run.
        #[cfg(feature = "csr")]
        if let Some(change) = change {
            let inner = self.inner;
            set_timeout(
                move || {
                    let took = browser::now() - started;
                    inner.update_value(|inner| {
                        let smoothed = inner.took.get(&change).map_or(took, |before| (before + took) / 2.0);
                        inner.took.insert(change, smoothed);
                    });
                },
                std::time::Duration::ZERO,
            );
        }
        #[cfg(not(feature = "csr"))]
        let _ = change;
    }
}

/// Hands the router's navigation and location to `Pending`; lives inside `<Router>`.
#[component]
pub fn Bind() -> impl IntoView {
    if let Some(pending) = Pending::expect() {
        let navigate = use_navigate();
        let location = use_location();
        pending.inner.update_value(|inner| {
            inner.navigate = Some(Rc::new(navigate));
            inner.location = Some(location);
        });
    }
}

fn path_of(to: &str) -> &str {
    to.split(['?', '#']).next().unwrap_or(to)
}

/// The query of an address, without `?` and without the fragment.
fn search_of(to: &str) -> &str {
    let without_fragment = to.split('#').next().unwrap_or(to);
    without_fragment.split_once('?').map_or("", |(_, search)| search)
}

/// What a step from one address (the router's) to another changes for the visitor; `None` if
/// nothing they see: the same address, another fragment, another `page` of the same list.
/// `phone`: nothing stands beside a page there, what is picked is the page.
pub fn change(from_path: &str, from_search: &str, to_path: &str, to_search: &str, phone: bool) -> Option<Change> {
    let from_search = from_search.trim_start_matches('?');
    let to_search = to_search.trim_start_matches('?');
    let program = |path: &str| -> Option<(String, ProgramTab)> {
        let rest = path.strip_prefix("/programs/")?;
        let (slug, tab) = match rest.split_once('/') {
            Some((slug, tab)) => (slug, ProgramTab::from_segment(tab)?),
            None => (rest, ProgramTab::default()),
        };
        Some((slug.to_string(), tab))
    };
    if from_path != to_path {
        // Another view of the same program is the same page with another column.
        if let (Some((from_slug, from_tab)), Some((to_slug, to_tab))) = (program(from_path), program(to_path)) {
            if from_slug == to_slug {
                let (from, to) = (ProgramUrl::parse(&from_slug, from_tab, from_search), ProgramUrl::parse(&to_slug, to_tab, to_search));
                return program_change(&from, &to, phone);
            }
        }
        return Some(Change::Page(Shape::of(to_path)));
    }
    if from_search == to_search {
        return None;
    }
    match Shape::of(to_path) {
        Shape::Catalog => {
            let (from, to) = (CatalogUrl::parse(from_search), CatalogUrl::parse(to_search));
            if from.query != to.query {
                Some(Change::List)
            } else if from.open != to.open {
                // On a phone a module is its own page (the catalog turns `open` into it).
                Some(if phone { Change::Page(Shape::Module) } else { Change::Preview })
            } else {
                None
            }
        }
        Shape::Bookmarks => {
            let (from, to) = (BookmarksUrl::parse(from_search), BookmarksUrl::parse(to_search));
            if let Some(change) = local_change(&from, &to, phone, Change::Column(Shape::Bookmarks)) {
                Some(change)
            } else if (from.season, from.sort, from.descending) != (to.season, to.sort, to.descending) {
                Some(Change::Column(Shape::Bookmarks))
            } else if from.open != to.open {
                Some(Change::Preview)
            } else {
                None
            }
        }
        Shape::Studyplan => studyplan_change(&PlanAddress::parse(from_search), &PlanAddress::parse(to_search), phone),
        Shape::Programs => Some(Change::Column(Shape::Programs)),
        Shape::Program => {
            let (slug, tab) = program(to_path)?;
            program_change(&ProgramUrl::parse(&slug, tab, from_search), &ProgramUrl::parse(&slug, tab, to_search), phone)
        }
        Shape::Home | Shape::Module | Shape::Text => None,
    }
}

/// Within one program: another view or study plan is its column; the module in full, or on a
/// phone whatever is picked, is a page; what stands beside the page is the aside. `None` for the
/// same view by another address (`/programs/<slug>` is its plan).
fn program_change(from: &ProgramUrl, to: &ProgramUrl, phone: bool) -> Option<Change> {
    if from == to {
        return None;
    }
    // On a phone an area or a row of the plan is the page as well: what comes back where a module
    // filled the page, or what takes its place.
    let picked_page = phone && (to.area.is_some() || to.req.is_some());
    let after_module = if picked_page { Change::Page(Shape::Text) } else { Change::Column(Shape::Program) };
    if let Some(change) = local_change(from, to, phone, after_module) {
        return Some(change);
    }
    if picked_page {
        return Some(Change::Page(Shape::Text));
    }
    if from.tab != to.tab || from.variant != to.variant || from.full != to.full || (phone && (from.area.is_some() || from.req.is_some())) {
        return Some(Change::Column(Shape::Program));
    }
    Some(Change::Aside)
}

/// Within the Studienplan: another semester, view or Regelstudienplan being taken over is its
/// column; the module beside it is the aside, and on a phone, where nothing stands beside a page,
/// the plan's panel of the module is the page until it is closed. „Vollbild" fills the page with
/// the module's whole page, on a phone as well (`PlanAddress`). The Übersicht shows every
/// semester: there `sem` only says which of its semesters a module planned twice is shown in
/// beside it, so it is the aside's.
fn studyplan_change(from: &PlanAddress, to: &PlanAddress, phone: bool) -> Option<Change> {
    let after_module = if phone && to.url.open.is_some() { Change::Page(Shape::Text) } else { Change::Column(Shape::Studyplan) };
    if let Some(change) = local_change(from, to, false, after_module) {
        return Some(change);
    }
    let (from, to) = (&from.url, &to.url);
    fn column_sem(url: &StudyplanUrl) -> Option<&str> {
        if url.view == PlanView::Overview { None } else { url.sem.as_deref() }
    }
    fn beside(url: &StudyplanUrl) -> (Option<&str>, Option<&str>, Option<&str>) {
        (url.open.as_deref(), url.row.as_deref(), url.open.as_ref().and(url.sem.as_deref()))
    }
    if (column_sem(from), from.view, &from.import, from.variant) != (column_sem(to), to.view, &to.import, to.variant) {
        Some(Change::Column(Shape::Studyplan))
    } else if beside(from) != beside(to) {
        Some(match (phone, &to.open) {
            (false, _) => Change::Aside,
            (true, Some(_)) => Change::Page(Shape::Text),
            (true, None) => Change::Column(Shape::Studyplan),
        })
    } else {
        None
    }
}

/// A step within a page that shows its modules in place (`crate::local`), as far as what fills
/// the page decides it: a module coming to fill it is a page of its own, the module's, and the
/// page coming back where a module filled it is `after_module` (the page's column, or on a phone
/// whatever else is picked there). `None` where the same fills the page before and after: the
/// step is the page's own business then (another view or order, what stands beside the page).
fn local_change(from: &impl LocalView, to: &impl LocalView, phone: bool, after_module: Change) -> Option<Change> {
    match (crate::local::filling(from, phone), crate::local::filling(to, phone)) {
        (before, Some(now)) if before.as_ref() != Some(&now) => Some(Change::Page(Shape::Module)),
        (Some(_), None) => Some(after_module),
        _ => None,
    }
}

#[cfg(feature = "csr")]
mod browser {
    use leptos::prelude::*;
    use leptos::wasm_bindgen::{JsCast, JsValue};
    use leptos::web_sys;

    use super::{Pending, Via, REPLAY};

    pub(super) fn now() -> f64 {
        web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
    }

    /// `data-settling` on the document for the frame in which the page comes (app.css).
    pub(super) fn settle() {
        let Some(root) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.document_element()) else { return };
        let _ = root.set_attribute("data-settling", "");
        request_animation_frame(move || {
            set_timeout(move || { let _ = root.remove_attribute("data-settling"); }, std::time::Duration::ZERO);
        });
    }

    /// The browser's own Back or Forward, handed to the router now: it reads the address
    /// from the browser, as it would have a frame earlier.
    pub(super) fn replay_popstate() {
        let Some(window) = web_sys::window() else { return };
        let init = web_sys::PopStateEventInit::new();
        init.set_state(&window.history().and_then(|h| h.state()).unwrap_or(JsValue::NULL));
        if let Ok(event) = web_sys::PopStateEvent::new_with_event_init_dict("popstate", &init) {
            let _ = js_sys_set(&event, REPLAY);
            let _ = window.dispatch_event(&event);
        }
    }

    fn js_sys_set(target: &web_sys::PopStateEvent, key: &str) -> Result<bool, JsValue> {
        leptos::web_sys::js_sys::Reflect::set(target, &JsValue::from_str(key), &JsValue::TRUE)
    }

    fn is_replay(event: &web_sys::Event) -> bool {
        leptos::web_sys::js_sys::Reflect::get(event, &JsValue::from_str(REPLAY)).is_ok_and(|v| v.is_truthy())
    }

    /// Where the router is, as `(path, search)`.
    fn here(pending: &Pending) -> Option<(String, String)> {
        pending.inner.with_value(|inner| inner.location.as_ref().map(|location| (location.pathname.get_untracked(), location.search.get_untracked())))
    }

    pub(super) fn listen(pending: Pending) {
        // Clicks on links: the same checks the router makes before it takes one, and the link it
        // would take (the outermost on the event's path). Whatever another handler has claimed
        // (`preventDefault`: the sheet's draft, „Zurück" through the history, the map's dots)
        // is left alone.
        let click = window_event_listener(leptos::ev::click, move |ev: web_sys::MouseEvent| {
            if ev.default_prevented() || ev.button() != 0 || ev.meta_key() || ev.alt_key() || ev.ctrl_key() || ev.shift_key() {
                return;
            }
            let path = ev.composed_path();
            let mut link: Option<web_sys::HtmlAnchorElement> = None;
            for i in 0..path.length() {
                if let Ok(anchor) = path.get(i).dyn_into::<web_sys::HtmlAnchorElement>() {
                    link = Some(anchor);
                }
            }
            let Some(link) = link else { return };
            let href = link.href();
            let rel = link.get_attribute("rel").unwrap_or_default();
            if href.is_empty() || !link.target().is_empty() || link.has_attribute("download") || rel.split([' ', '\t']).any(|part| part == "external") {
                return;
            }
            let (Ok(target), Some(origin)) = (web_sys::Url::new(&href), web_sys::window().and_then(|w| w.location().origin().ok())) else { return };
            if target.origin() != origin {
                return;
            }
            let Some((from_path, from_search)) = here(&pending) else { return };
            let to = format!("{}{}{}", target.pathname(), target.search(), target.hash());
            let Some(change) = super::change(&from_path, &from_search, &target.pathname(), &target.search(), crate::nav::is_phone()) else { return };
            ev.prevent_default();
            let scroll = !link.has_attribute("noscroll") && !link.has_attribute("data-noscroll");
            pending.start(to, Via::Link { replace: false, scroll }, change, false);
        });
        // Back and Forward: the browser has moved already, the router hears of it a frame later.
        let step = window_event_listener(leptos::ev::popstate, move |ev: web_sys::PopStateEvent| {
            if is_replay(&ev) {
                return;
            }
            let Some(location) = web_sys::window().map(|w| w.location()) else { return };
            let (Ok(path), Ok(search), Ok(hash)) = (location.pathname(), location.search(), location.hash()) else { return };
            let Some((from_path, from_search)) = here(&pending) else { return };
            let Some(change) = super::change(&from_path, &from_search, &path, &search, crate::nav::is_phone()) else { return };
            ev.stop_immediate_propagation();
            pending.start(format!("{path}{search}{hash}"), Via::History, change, false);
        });
        // For the life of the app: `App` is mounted once.
        std::mem::forget(click);
        std::mem::forget(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_step_changes() {
        assert_eq!(change("/catalog", "", "/programs", "", false), Some(Change::Page(Shape::Programs)));
        assert_eq!(change("/catalog", "turnus=winter", "/catalog", "turnus=winter&form=lecture", false), Some(Change::List));
        assert_eq!(change("/catalog", "turnus=winter", "/catalog", "turnus=winter&open=11103", false), Some(Change::Preview));
        assert_eq!(change("/catalog", "turnus=winter", "/catalog", "turnus=winter&open=11103", true), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/catalog", "turnus=winter&page=2", "/catalog", "turnus=winter&page=3", false), None);
        assert_eq!(change("/catalog", "", "/catalog", "", false), None);
        assert_eq!(change("/", "", "/", "", false), None);
        assert_eq!(change("/programs/informatik/plan", "", "/programs/informatik/areas", "", false), Some(Change::Column(Shape::Program)));
        assert_eq!(change("/programs/informatik", "", "/programs/informatik/plan", "", false), None);
        assert_eq!(change("/programs/informatik/plan", "", "/programs/physik/plan", "", false), Some(Change::Page(Shape::Program)));
        assert_eq!(change("/programs/informatik/plan", "", "/programs/informatik/plan", "open=11103", false), Some(Change::Aside));
        assert_eq!(change("/programs/informatik/plan", "open=11103", "/programs/informatik/plan", "open=11103&full=1", false), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/programs/informatik/plan", "", "/programs/informatik/plan", "area=4", true), Some(Change::Page(Shape::Text)));
        assert_eq!(change("/programs", "", "/programs", "level=bachelor", false), Some(Change::Column(Shape::Programs)));
        assert_eq!(change("/bookmarks", "", "/bookmarks", "sort=title", false), Some(Change::Column(Shape::Bookmarks)));
        assert_eq!(change("/catalog/module/11103", "", "/impressum", "", false), Some(Change::Page(Shape::Text)));
    }

    #[test]
    fn a_module_filling_a_page_in_place_is_a_page() {
        // The marked modules: the preview beside the list, „Vollbild" in place, and back.
        assert_eq!(change("/bookmarks", "sort=title", "/bookmarks", "sort=title&open=11103", false), Some(Change::Preview));
        assert_eq!(change("/bookmarks", "sort=title&open=11103", "/bookmarks", "sort=title&open=11104", false), Some(Change::Preview));
        assert_eq!(change("/bookmarks", "sort=title&open=11103", "/bookmarks", "sort=title&open=11103&full=1", false), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/bookmarks", "sort=title&open=11103&full=1", "/bookmarks", "sort=title&open=11103", false), Some(Change::Column(Shape::Bookmarks)));
        // On a phone a tap on a row is the module's page, and „Zurück" the list again.
        assert_eq!(change("/bookmarks", "sort=title", "/bookmarks", "sort=title&open=11103", true), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/bookmarks", "sort=title&open=11103", "/bookmarks", "sort=title", true), Some(Change::Column(Shape::Bookmarks)));
        // A program: the same, and on a phone what the module was picked from comes back as the page.
        assert_eq!(change("/programs/informatik/plan", "open=11103&full=1", "/programs/informatik/plan", "open=11103", false), Some(Change::Column(Shape::Program)));
        assert_eq!(change("/programs/informatik/plan", "open=11103&full=1", "/programs/informatik/plan", "open=11104&full=1", false), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/programs/informatik/areas", "area=4", "/programs/informatik/areas", "area=4&open=11103", true), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/programs/informatik/areas", "area=4&open=11103", "/programs/informatik/areas", "area=4", true), Some(Change::Page(Shape::Text)));
        assert_eq!(change("/programs/informatik/plan", "open=11103", "/programs/informatik/plan", "", true), Some(Change::Column(Shape::Program)));
        assert_eq!(change("/programs/informatik/areas", "area=4", "/programs/informatik/areas", "", true), Some(Change::Column(Shape::Program)));
    }

    #[test]
    fn a_step_of_the_studyplan() {
        // Another page, and within the plan another view, semester or import: the plan's column.
        assert_eq!(change("/catalog", "", "/studyplan", "", false), Some(Change::Page(Shape::Studyplan)));
        assert_eq!(change("/studyplan", "", "/studyplan", "view=dates", false), Some(Change::Column(Shape::Studyplan)));
        assert_eq!(change("/studyplan", "sem=2026W", "/studyplan", "sem=2027S", true), Some(Change::Column(Shape::Studyplan)));
        assert_eq!(change("/studyplan", "", "/studyplan", "view=all&import=mine", false), Some(Change::Column(Shape::Studyplan)));
        // The module beside the plan, and the Termin it points at.
        assert_eq!(change("/studyplan", "", "/studyplan", "open=12104", false), Some(Change::Aside));
        assert_eq!(change("/studyplan", "open=12104", "/studyplan", "open=12104&row=148369-aaf38", false), Some(Change::Aside));
        assert_eq!(change("/studyplan", "open=12104", "/studyplan", "", false), Some(Change::Aside));
        // On the Übersicht `sem` names the semester of the module beside it, not the column's.
        assert_eq!(change("/studyplan", "view=all", "/studyplan", "sem=2027W&view=all&open=12204", false), Some(Change::Aside));
        assert_eq!(change("/studyplan", "sem=2027S&view=all&open=12204", "/studyplan", "sem=2027W&view=all&open=12204", false), Some(Change::Aside));
        assert_eq!(change("/studyplan", "sem=2027W&view=all&open=12204", "/studyplan", "sem=2027W&view=all", false), Some(Change::Aside));
        assert_eq!(change("/studyplan", "sem=2027S&view=all", "/studyplan", "sem=2027W&view=all", false), None);
        assert_eq!(change("/studyplan", "sem=2027W&view=all", "/studyplan", "sem=2027W", false), Some(Change::Column(Shape::Studyplan)));
        assert_eq!(change("/studyplan", "sem=2027S&open=12204", "/studyplan", "sem=2027W&open=12204", false), Some(Change::Column(Shape::Studyplan)));
        // On a phone the plan's panel of the module is the page, and closing it is the plan again.
        assert_eq!(change("/studyplan", "", "/studyplan", "open=12104", true), Some(Change::Page(Shape::Text)));
        assert_eq!(change("/studyplan", "open=12104", "/studyplan", "", true), Some(Change::Column(Shape::Studyplan)));
        // „Vollbild": the module fills the plan's page, and „Zurück" brings the plan with it beside.
        assert_eq!(change("/studyplan", "open=12104", "/studyplan", "open=12104&full=1", false), Some(Change::Page(Shape::Module)));
        assert_eq!(change("/studyplan", "open=12104&full=1", "/studyplan", "open=12104", false), Some(Change::Column(Shape::Studyplan)));
        assert_eq!(change("/studyplan", "open=12104&full=1", "/studyplan", "open=12104", true), Some(Change::Page(Shape::Text)));
        // Unknown names change nothing.
        assert_eq!(change("/studyplan", "sem=2026W", "/studyplan", "sem=2026W&week=2026-10-14", false), None);
    }

    #[test]
    fn parts_of_an_address() {
        assert_eq!(path_of("/catalog?turnus=winter#x"), "/catalog");
        assert_eq!(search_of("/catalog?turnus=winter#x"), "turnus=winter");
        assert_eq!(search_of("/catalog#x?y"), "");
        assert_eq!(search_of("/catalog"), "");
    }
}
