//! The way back (owner, 2026-10-18: „am handy ist das ja eher wie eine App … der back graph beim
//! handy ist halt eher so dass man zurück zum main screen kommt und nicht die Seiten rückwärts
//! abzuspielen"; folia/design/zurueck/konzept.html). On a phone Back and the arrow at the head of a
//! page are one key: it closes first what lies over the page, then leads one level up. On a
//! desktop Back is the browser's history. Android first (owner, the same day: „auf gar keinen Fall
//! darf Android darunter leiden. Dann geht es halt nur auf Android."): what Safari cannot do, an
//! iPhone does without, and Android is not made worse for it.
//!
//! - **What lies over a page** (`layer_while`): a menu, the selection, a dialog, and on a phone the
//!   semester „Mein Studium"'s row shows. It has no entry in the history, so Forward never opens it
//!   again. Where the phone's layout is shown and the browser has `CloseWatcher` (Chrome on Android
//!   from 126, Firefox from 157) Back closes the newest one; without it (Safari, and so every
//!   browser on an iPhone) its ×, a tap beside it and the arrow do, and Back leads up with what is
//!   open going along. A `<dialog>` the page shows modally has the browser's own watcher. A
//!   watcher made without a tap may land in the group of the one before it, and one Back would
//!   close both: a layer opened while no tap is fresh (the selection, held until its timer fires)
//!   gets its watcher with the next tap or the finger's release. A new address closes what lies over
//!   the old one (`Follow`), except the semester of the row, which is the page's own state.
//! - **The step before** (`came_from`): which address the history has before the current entry,
//!   from the Navigation API where the browser has it (Chrome, Edge, Safari from 26.2, Firefox
//!   from 147), else as the memory of the tabs guesses it (`Tabs::before`). Closing what stands
//!   beside a page goes back there instead of writing a new step, so that Back never brings it
//!   again.
//! - **The arrow** (`arrow`): what Back does on Android — the newest layer first, else one step back.
//! - **A stack for each tab** (`tab`, `steer`): on a phone the history is Start and, right above it,
//!   the stack of the tab the visitor is in, so that Back leads up through the tab to Start and then
//!   out (owner, 2026-10-18: „Erst zur Startseite dann raus"). A change of tabs — the bar, or a link
//!   to a page of another tab („Tab Wechsel, das wird sonst giga irritierend") — writes the history
//!   in place: first what is missing, while the tap is fresh (an entry written without one the
//!   browsers skip on Back), then the entries in place (`history.go`, `replaceState`). What the tap
//!   changes shows at once (`Pending::hold`), the history is written after the frame that shows it
//!   (the tap stays fresh for seconds), and the router takes the top once it is written. The tab
//!   left keeps its stack, and comes back as it was; what is left over ahead is no way Forward. A
//!   page come to from outside (a link in a messenger) stays as it came while it is only read, so
//!   Back leads back out; its first use lays under it what lies above it, up to Start (`seed`).
//!   Where the browser lacks the Navigation API, or an entry below is of another load of a page (the
//!   site's pages before the app took over, another language: going there loads that page anew; a
//!   reload keeps the entries the page wrote), each link is a step of its own as before.

use leptos::prelude::*;

use crate::tabs::Area;

/// The phone's behaviour applies where the phone's layout is shown (owner, 2026-10-18: „wenn die
/// mobile version gezeigt wird, sollte auch das mobile verhalten verwendet werden").
pub fn phone() -> bool {
    folia_design::nav::is_phone()
}

/// An address of the app (path and query) without the query's `keys`, its other pairs in their order.
fn without(url: &str, keys: &[&str]) -> String {
    let Some((path, query)) = url.split_once('?') else { return url.to_string() };
    let kept: Vec<&str> = query.split('&').filter(|pair| !pair.is_empty() && !keys.contains(&pair.split('=').next().unwrap_or(pair))).collect();
    if kept.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", kept.join("&"))
    }
}

/// Whether the query of `url` has one of `keys`.
fn has(url: &str, keys: &[&str]) -> bool {
    url.split_once('?').is_some_and(|(_, query)| query.split('&').any(|pair| keys.contains(&pair.split('=').next().unwrap_or(pair))))
}

/// What lies above a page in its tab, the tab's first page first and the page last (empty for
/// Start, which is under every tab): a page come to from outside, or by a link from another tab,
/// gets it under it. What stands beside a page goes first (a module in full, then beside it, then
/// an area of a program); then the levels above the page: a module's own page under the catalog, a
/// view of „Mein Studium" under it, the programs under „Mein Studium", a program under them.
pub fn chain_of(url: &str) -> Vec<String> {
    let url = url.split('#').next().unwrap_or(url);
    let path = url.split('?').next().unwrap_or(url);
    if path == "/" {
        return Vec::new();
    }
    let mut chain = vec![url.to_string()];
    let mut now = url.to_string();
    for keys in [&["full"][..], &["open", "row"], &["area", "req"]] {
        if has(&now, keys) {
            now = without(&now, keys);
            chain.insert(0, now.clone());
        }
    }
    if path.starts_with("/catalog/module/") {
        chain.insert(0, folia_routes::url::CATALOG.to_string());
    } else if (path == folia_routes::url::STUDY && has(&now, &["view"])) || path == folia_routes::url::PROGRAMS {
        chain.insert(0, folia_routes::url::STUDY.to_string());
    } else if path.starts_with("/programs/") {
        chain.insert(0, folia_routes::url::PROGRAMS.to_string());
        chain.insert(0, folia_routes::url::STUDY.to_string());
    }
    chain
}

/// The tab a page belongs to, come to from the tab `from`: its area's, but a module's own page
/// opened out of a tab that shows its modules in place (a program, the Merkliste, the
/// Stundenplan) stays in that tab, as the memory of the tabs has it (`Area::shows_in_place`).
pub fn tab_of_page(path: &str, from: Area) -> Area {
    if path.starts_with("/catalog/module/") && from.shows_in_place() {
        from
    } else {
        Area::of(path)
    }
}

/// An entry a change of tabs writes: its address, and the semester „Mein Studium"'s row keeps there.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub url: String,
    pub semester: Option<String>,
}

/// What writes the history, in order.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    Push(Entry),
    Replace(Entry),
    Go(i32),
}

/// How a change of tabs writes the history: Start at `base`, the browser at `at`, the tab's
/// `stack` to stand right above Start with the browser at its top. What is missing is pushed first,
/// while the tap is fresh; then the browser walks down to Start's next entry and writes the
/// entries in place, one step up at a time, and on to the top. What lies ahead after that is left
/// over.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(crate) fn plan(base: usize, at: usize, stack: &[Entry]) -> Vec<Op> {
    let top = base + stack.len();
    let entry = |position: usize| stack.get(position - base - 1).cloned();
    let mut ops = Vec::new();
    let go = |ops: &mut Vec<Op>, delta: i64| {
        if delta != 0 {
            ops.push(Op::Go(delta as i32));
        }
    };
    if top > at {
        for position in (at + 1)..=top {
            if let Some(entry) = entry(position) {
                ops.push(Op::Push(entry));
            }
        }
        if at > base {
            go(&mut ops, base as i64 + 1 - top as i64);
            for position in (base + 1)..=at {
                if position > base + 1 {
                    go(&mut ops, 1);
                }
                if let Some(entry) = entry(position) {
                    ops.push(Op::Replace(entry));
                }
            }
            go(&mut ops, top as i64 - at as i64);
        }
    } else {
        let first = if stack.is_empty() { base } else { base + 1 };
        go(&mut ops, first as i64 - at as i64);
        for position in (base + 1)..=top {
            if position > base + 1 {
                go(&mut ops, 1);
            }
            if let Some(entry) = entry(position) {
                ops.push(Op::Replace(entry));
            }
        }
    }
    ops
}

/// What a layer is to the way back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Over the page (a menu, the selection): on a phone a `CloseWatcher` of its own, and a new
    /// address closes it.
    Over,
    /// A `<dialog>` the page shows modally: the browser's own watcher closes it; a new address
    /// closes it as well.
    Dialog,
    /// The page's own state that Back undoes before it leaves the page (the semester of „Mein
    /// Studium"'s row): a watcher of its own on a phone, but a new address leaves it alone.
    State,
}

/// What lies over a page while `open` says so (tracked); `close` closes it. Within a component: the
/// layer goes with it.
pub fn layer_while(open: impl Fn() -> bool + 'static, close: impl Fn() + 'static, kind: Kind) {
    #[cfg(feature = "csr")]
    {
        let close: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(close);
        let id = StoredValue::new_local(None::<u64>);
        Effect::new(move |_| {
            let now = open();
            match (now, id.get_value()) {
                (true, None) => id.set_value(Some(browser::open(close.clone(), kind))),
                (false, Some(layer)) => {
                    id.set_value(None);
                    browser::gone(layer);
                }
                _ => {}
            }
        });
        on_cleanup(move || {
            if let Some(Some(layer)) = id.try_get_value() {
                browser::gone(layer);
            }
        });
    }
    #[cfg(not(feature = "csr"))]
    let _ = (open, close, kind);
}

/// Whether the newest layer has a watcher of its own that Esc reaches (the phone's layout in a
/// browser with `CloseWatcher`): a page's own handler of Esc leaves the layer to it then, or one
/// Esc would close two.
pub fn watched() -> bool {
    #[cfg(feature = "csr")]
    {
        browser::watched()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Closes the newest layer, if one is open: true where it did.
pub fn close_top() -> bool {
    #[cfg(feature = "csr")]
    {
        browser::close_top()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// The address of the step before the current one in this tab's history (path and query, without
/// the language's prefix), where the browser says it (the Navigation API); `None` where it does not,
/// where that step is no page of this app, or where it is of another load of a page (the site's
/// pages before the app took over, another language): going back there would load that page anew.
pub fn before() -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let (entries, at) = browser::seen()?;
        let before = entries.into_iter().nth(at.checked_sub(1)?)?;
        if before.same {
            before.url
        } else {
            None
        }
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Whether the browser says which entries the history has (the Navigation API).
pub fn knows_history() -> bool {
    #[cfg(feature = "csr")]
    {
        browser::seen().is_some()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Whether the step before in the history is `target` (path and query, written as the app writes
/// links; a program's plan by either of its addresses): Back leads there, the same entry as before.
/// Where the browser does not say, `guess` answers (the memory of the tabs).
pub fn came_from(target: &str, guess: impl FnOnce() -> bool) -> bool {
    if knows_history() {
        before().is_some_and(|before| crate::pending::same_page(&before, target))
    } else {
        guess()
    }
}

/// Closes what stands beside a page (a module the plan no longer holds): to `target`, the page
/// without it, back through the history where that is the step before (`came_from`; `guess` where
/// the browser does not say), else in place of the current entry, so that Back does not bring it.
pub fn close_to(going: Option<crate::pending::Pending>, target: &str, guess: impl FnOnce() -> bool) {
    if came_from(target, guess) {
        back();
    } else if let Some(going) = going {
        going.go(target, leptos_router::NavigateOptions { replace: true, scroll: false, ..Default::default() });
    }
}

/// What a page keeps in its own entry of the history (the semester „Mein Studium"'s row shows):
/// set in the state of the entry the browser is at (`history.replaceState`), which writes no entry
/// and needs no tap; a reload, Back and Forward bring it with the entry. `None` takes it away.
pub fn keep(name: &str, value: Option<&str>) {
    #[cfg(feature = "csr")]
    browser::keep(name, value);
    #[cfg(not(feature = "csr"))]
    let _ = (name, value);
}

/// What the page keeps under `name` in the entry the browser is at (`keep`).
pub fn kept(name: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        browser::kept(name)
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = name;
        None
    }
}

/// Back, as the app takes it (`history.back()`).
pub fn back() {
    #[cfg(feature = "csr")]
    if let Some(history) = leptos::web_sys::window().and_then(|window| window.history().ok()) {
        let _ = history.back();
    }
}

/// Whether the step before is the level above within the tab on a phone: its stack has Start under
/// it, the browser is above the tab's first page, and that step is of this load of the page.
pub fn up_in_tab() -> bool {
    #[cfg(feature = "csr")]
    {
        phone() && browser::stacked().is_some_and(|(entries, at, base)| at > base + 1 && entries.get(at - 1).is_some_and(|entry| entry.same))
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// The arrow at the head of a phone's page (`chrome::TopBack`), which is Back: the newest layer
/// closes, else the history goes one step back — on a phone's stack the step before is the level
/// above (`up_in_tab`), elsewhere where it is where the arrow leads (`history`). `false` where
/// neither: the arrow's link takes the visitor up then.
pub fn arrow(history: bool) -> bool {
    if close_top() {
        return true;
    }
    if up_in_tab() || history {
        back();
        return true;
    }
    false
}

/// Whether a change of tabs is writing the history: the router waits for it (`Pending`).
pub fn rewriting() -> bool {
    #[cfg(feature = "csr")]
    {
        browser::rewriting()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Runs `then` once the history is written (at once where nothing writes it).
pub fn after_rewrite(then: impl FnOnce() + 'static) {
    #[cfg(feature = "csr")]
    browser::after_rewrite(Box::new(then));
    #[cfg(not(feature = "csr"))]
    then();
}

/// Back or Forward, heard before the router (`Pending`): true where it is no step for the router — a
/// step of a change of tabs writing the history, or Forward into what one left over, which goes
/// back at once.
pub fn on_popstate() -> bool {
    #[cfg(feature = "csr")]
    {
        browser::on_popstate()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// The first use of a page come to from outside on a phone (a link, the arrow, a tab): what lies
/// above it goes under it, up to Start, so that Back leads up through the tab and out (owner,
/// 2026-10-18: „wenn es keine Interaktion mit dem back graphen gab dann in den massanger, aber
/// wenn die app verwendet wurde dann über start menu"). Nothing where the history has Start already,
/// on a wide screen, or without the Navigation API. True where it laid something.
pub fn seed() -> bool {
    #[cfg(feature = "csr")]
    {
        phone() && browser::seed()
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// A link followed on a phone, from where the router is (`from_path`, `from_search`) to `to`: its
/// first use seeds a page come to from outside (`seed`), and a page of another tab is a change of
/// tabs, with what lies above it under it (`chain_of`). True where the change of tabs took the step.
pub fn steer(pending: crate::pending::Pending, from_path: &str, from_search: &str, to: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        if !phone() {
            return false;
        }
        if rewriting() {
            return true;
        }
        let to = to.split('#').next().unwrap_or(to);
        let Some((entries, at, base, landing)) = browser::stacked_or_seeded() else { return false };
        let from = browser::tab_at(&entries, base, at);
        let into = tab_of_page(to.split('?').next().unwrap_or(to), from);
        if into == from {
            // The link takes its own step, above the way up its first use lays under the page.
            if let Some(landing) = landing {
                browser::seed_now(landing);
            }
            return false;
        }
        let stack = chain_of(to).into_iter().map(|url| Entry { url, semester: None }).collect();
        browser::tab_to(pending, (from_path, from_search), entries, at, base, stack, landing)
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (pending, from_path, from_search, to);
        false
    }
}

/// A tab of the bar tapped on a phone: in another tab, to its stack as it was left (else to `href`,
/// where its link leads, with what lies above it); in its own, back to its first page. True where it
/// did; false where the link has to take the step itself (a wide screen, no Navigation API, an entry
/// below of another load of a page).
pub fn tab(pending: crate::pending::Pending, area: Area, href: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        if !phone() {
            return false;
        }
        if rewriting() {
            return true;
        }
        let Some((entries, at, base, landing)) = browser::stacked_or_seeded() else { return false };
        let now = browser::tab_at(&entries, base, at);
        if now == area {
            let first = if area == Area::Home { base } else { base + 1 };
            if at > first {
                // An entry of another load of a page would load that page anew: the link then.
                if !entries.get(first).is_some_and(|entry| entry.same) {
                    return false;
                }
                if let Some(landing) = landing {
                    browser::seed_now(landing);
                }
                browser::go(first as i32 - at as i32);
            }
            return true;
        }
        // Start is the entry under every tab, and its own tab nothing above it.
        let stack = if area == Area::Home {
            Vec::new()
        } else {
            browser::saved(area).unwrap_or_else(|| chain_of(href).into_iter().map(|url| Entry { url, semester: None }).collect())
        };
        let Some(from) = pending.router_at() else { return false };
        browser::tab_to(pending, (&from.0, &from.1), entries, at, base, stack, landing)
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (pending, area, href);
        false
    }
}

/// A new address closes what lies over the old one, but not the page's own state: on a phone any
/// new address (Back on an iPhone leaves the page, and what is open goes with it), on a wide screen
/// another page (the rows selected stay while a module opens beside them). Lives inside the router.
#[component]
pub fn Follow() -> impl IntoView {
    let location = crate::i18n::use_location();
    Effect::new(move |before: Option<(String, String)>| {
        let now = (location.pathname.get(), location.search.get());
        if let Some(before) = before {
            if before.0 != now.0 || (before.1 != now.1 && phone()) {
                #[cfg(feature = "csr")]
                browser::close_over();
            }
        }
        now
    });
}

#[cfg(feature = "csr")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use leptos::wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use leptos::web_sys::{
        self,
        js_sys::{Array, Function, Reflect},
    };

    use super::{Entry, Kind};
    use crate::tabs::Area;

    /// A layer that is open: its number, what closes it, what it is, and its watcher.
    struct Open {
        id: u64,
        close: Rc<dyn Fn()>,
        kind: Kind,
        watch: Option<Watch>,
    }

    thread_local! {
        static LAYERS: RefCell<Vec<Open>> = const { RefCell::new(Vec::new()) };
        static NEXT: Cell<u64> = const { Cell::new(0) };
    }

    fn get(target: &JsValue, key: &str) -> Option<JsValue> {
        Reflect::get(target, &JsValue::from_str(key)).ok().filter(|value| !value.is_undefined() && !value.is_null())
    }

    /// The browser's `CloseWatcher`, where the phone's layout is shown and the browser has it.
    fn watcher_class() -> Option<Function> {
        if !folia_design::nav::is_phone() {
            return None;
        }
        let window: JsValue = web_sys::window()?.into();
        get(&window, "CloseWatcher")?.dyn_into::<Function>().ok()
    }

    /// Whether a tap (or a key) is fresh: a watcher made now has a group of its own. Where the
    /// browser does not say, it is taken to be.
    fn fresh() -> bool {
        let Some(activation) = web_sys::window().and_then(|window| get(&window.navigator(), "userActivation")) else { return true };
        get(&activation, "isActive").is_none_or(|active| active.is_truthy())
    }

    /// A layer's watcher: made at once where a tap is fresh, else with the next one (a finger
    /// lifted, a key; not Esc, which gives the page no activation). Dropped, it is destroyed
    /// (`destroy()`: no `close` then).
    struct Watch {
        watcher: Rc<RefCell<Option<JsValue>>>,
        alive: Rc<Cell<bool>>,
        _close: Closure<dyn FnMut()>,
        waiting: Option<Closure<dyn FnMut(web_sys::Event)>>,
    }

    /// The events after which the page may have a fresh activation (a finger lifted, a key).
    const ACTIVATIONS: [&str; 3] = ["pointerup", "keydown", "mousedown"];

    impl Watch {
        fn new(id: u64, now: bool) -> Option<Self> {
            let class = watcher_class()?;
            let watcher = Rc::new(RefCell::new(None::<JsValue>));
            let alive = Rc::new(Cell::new(true));
            let on_close = Closure::<dyn FnMut()>::new(move || close(id));
            let handler: JsValue = on_close.as_ref().clone();
            let make = {
                let (watcher, alive) = (watcher.clone(), alive.clone());
                move || {
                    if !alive.get() || watcher.borrow().is_some() {
                        return;
                    }
                    if let Ok(made) = Reflect::construct(&class, &Array::new()) {
                        let _ = Reflect::set(&made, &JsValue::from_str("onclose"), &handler);
                        *watcher.borrow_mut() = Some(made);
                    }
                }
            };
            let mut waiting = None;
            if now || fresh() {
                make();
            } else if let Some(window) = web_sys::window() {
                // The next tap or key: a finger lifted after a long press gives the page a
                // user's activation, and with it a group of its own for the watcher. Heard in
                // the capture phase, after the browser has counted the activation.
                let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                    if fresh() {
                        make();
                    }
                });
                for event in ACTIVATIONS {
                    let _ = window.add_event_listener_with_callback_and_bool(event, listener.as_ref().unchecked_ref(), true);
                }
                waiting = Some(listener);
            }
            Some(Watch { watcher, alive, _close: on_close, waiting })
        }

        fn made(&self) -> bool {
            self.watcher.borrow().is_some()
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            self.alive.set(false);
            if let (Some(listener), Some(window)) = (self.waiting.take(), web_sys::window()) {
                for event in ACTIVATIONS {
                    let _ = window.remove_event_listener_with_callback_and_bool(event, listener.as_ref().unchecked_ref(), true);
                }
            }
            if let Some(watcher) = self.watcher.borrow_mut().take() {
                if let Some(destroy) = get(&watcher, "destroy").and_then(|destroy| destroy.dyn_into::<Function>().ok()) {
                    let _ = destroy.call0(&watcher);
                }
            }
        }
    }

    /// How many layers are open, on the root for `enhance.js` (`data-layers`; `data-state` for the
    /// page's own state): its Esc closes the module beside a page or leaves a page only where none
    /// is. The browser checks read them as well (e2e/back.mjs).
    fn mark() {
        let (over, state) = LAYERS.with(|layers| {
            let layers = layers.borrow();
            (layers.iter().filter(|open| open.kind != Kind::State).count(), layers.iter().filter(|open| open.kind == Kind::State).count())
        });
        let Some(root) = web_sys::window().and_then(|window| window.document()).and_then(|document| document.document_element()) else { return };
        for (name, count) in [("data-layers", over), ("data-state", state)] {
            if count == 0 {
                let _ = root.remove_attribute(name);
            } else {
                let _ = root.set_attribute(name, &count.to_string());
            }
        }
    }

    pub(super) fn open(close: Rc<dyn Fn()>, kind: Kind) -> u64 {
        let id = NEXT.with(|next| {
            let id = next.get().wrapping_add(1);
            next.set(id);
            id
        });
        let watch = match kind {
            Kind::Dialog => None,
            // The semester of the row comes with a swipe, which is no tap; nothing else is open
            // then (a menu's scrim, a dialog and the selection hold the row), so its watcher has a
            // group of its own all the same.
            Kind::State => Watch::new(id, true),
            Kind::Over => Watch::new(id, false),
        };
        LAYERS.with(|layers| layers.borrow_mut().push(Open { id, close, kind, watch }));
        mark();
        id
    }

    /// The layer went (closed, or its page gone): its watcher with it.
    pub(super) fn gone(id: u64) {
        let open = LAYERS.with(|layers| {
            let mut layers = layers.borrow_mut();
            layers.iter().position(|open| open.id == id).map(|at| layers.remove(at))
        });
        drop(open);
        mark();
    }

    /// Closes the layer `id` (its watcher heard Back): what it closes with, outside the list.
    fn close(id: u64) {
        let close = LAYERS.with(|layers| layers.borrow().iter().find(|open| open.id == id).map(|open| open.close.clone()));
        if let Some(close) = close {
            close();
        }
    }

    pub(super) fn close_top() -> bool {
        let close = LAYERS.with(|layers| layers.borrow().last().map(|open| open.close.clone()));
        match close {
            Some(close) => {
                close();
                true
            }
            None => false,
        }
    }

    pub(super) fn watched() -> bool {
        LAYERS.with(|layers| layers.borrow().last().is_some_and(|open| open.watch.as_ref().is_some_and(Watch::made)))
    }

    /// A new address: what lies over the page goes, newest first; the page's own state stays.
    pub(super) fn close_over() {
        let closes: Vec<Rc<dyn Fn()>> = LAYERS.with(|layers| layers.borrow().iter().rev().filter(|open| open.kind != Kind::State).map(|open| open.close.clone()).collect());
        for close in closes {
            close();
        }
    }

    /// The state of the entry the browser is at, as an object of its own.
    fn state() -> Option<(web_sys::History, JsValue)> {
        let history = web_sys::window()?.history().ok()?;
        let state = history.state().ok().filter(|state| state.is_object()).unwrap_or_else(|| leptos::web_sys::js_sys::Object::new().into());
        let copy = leptos::web_sys::js_sys::Object::assign(&leptos::web_sys::js_sys::Object::new(), state.unchecked_ref());
        Some((history, copy.into()))
    }

    pub(super) fn keep(name: &str, value: Option<&str>) {
        let Some((history, state)) = state() else { return };
        let key = JsValue::from_str(&format!("betula.{name}"));
        let now = Reflect::get(&state, &key).ok().and_then(|now| now.as_string());
        if now.as_deref() == value {
            return;
        }
        match value {
            Some(value) => {
                let _ = Reflect::set(&state, &key, &JsValue::from_str(value));
            }
            None => {
                let _ = Reflect::delete_property(state.unchecked_ref::<leptos::web_sys::js_sys::Object>(), &key);
            }
        }
        let _ = history.replace_state(&state, "");
    }

    pub(super) fn kept(name: &str) -> Option<String> {
        let state = web_sys::window()?.history().ok()?.state().ok()?;
        get(&state, &format!("betula.{name}"))?.as_string()
    }

    /// An entry of the history as the Navigation API has it: its address (as an app path, `None`
    /// for one not of this app), its key, and whether it is of the document the page is (an entry
    /// of another load is loaded again when the browser goes there; the entries a page wrote stay
    /// its own across a reload).
    pub(super) struct Seen {
        pub url: Option<String>,
        pub key: String,
        pub same: bool,
    }

    fn address(entry: &JsValue, origin: &str) -> Option<String> {
        let url = web_sys::Url::new(&get(entry, "url")?.as_string()?).ok()?;
        if url.origin() != origin {
            return None;
        }
        let path = crate::i18n::app_path(&url.pathname())?.to_string();
        Some(format!("{path}{}", url.search()))
    }

    pub(super) fn seen() -> Option<(Vec<Seen>, usize)> {
        let window: JsValue = web_sys::window()?.into();
        let navigation = get(&window, "navigation")?;
        let list = get(&navigation, "entries")?.dyn_into::<Function>().ok()?.call0(&navigation).ok()?.dyn_into::<Array>().ok()?;
        let index = get(&get(&navigation, "currentEntry")?, "index")?.as_f64()?;
        if index < 0.0 {
            return None;
        }
        let origin = web_sys::window()?.location().origin().ok()?;
        let entries = list
            .iter()
            .map(|entry| Seen {
                url: address(&entry, &origin),
                key: get(&entry, "key").and_then(|key| key.as_string()).unwrap_or_default(),
                same: get(&entry, "sameDocument").is_some_and(|same| same.is_truthy()),
            })
            .collect();
        Some((entries, index as usize))
    }

    /// The history as a phone's stacks have it: the entries, the current one, and Start under it
    /// (the last entry with the start page's address at or below the current one).
    pub(super) fn stacked() -> Option<(Vec<Seen>, usize, usize)> {
        let (entries, at) = seen()?;
        let base = (0..=at).rev().find(|&i| entries.get(i).and_then(|entry| entry.url.as_deref()) == Some("/"))?;
        Some((entries, at, base))
    }

    /// The tab the history is in: the area of the first page above Start, Start's own on it.
    pub(super) fn tab_at(entries: &[Seen], base: usize, at: usize) -> Area {
        if at <= base {
            return Area::Home;
        }
        entries.get(base + 1).and_then(|entry| entry.url.as_deref()).map_or(Area::Home, |url| Area::of(url.split('?').next().unwrap_or(url)))
    }

    /// A page come to from outside, not used yet: the first entry of the app, not Start.
    /// A page come to from outside, not used yet (`seed`): its address, the first of the app's
    /// entries and not Start.
    pub(super) struct Landing(String);

    pub(super) fn landing() -> Option<Landing> {
        let (entries, at) = seen()?;
        match entries.first() {
            Some(Seen { url: Some(url), same: true, .. }) if at == 0 && url.as_str() != "/" => Some(Landing(url.clone())),
            _ => None,
        }
    }

    pub(super) fn seed() -> bool {
        landing().is_some_and(seed_now)
    }

    /// Lays under the page come to from outside what lies above it, up to Start: Start in its
    /// entry's place, then the way up to it and the page itself, which keeps what its entry kept
    /// (the row's semester).
    pub(super) fn seed_now(Landing(landing): Landing) -> bool {
        let Some(history) = web_sys::window().and_then(|window| window.history().ok()) else { return false };
        let locale = crate::i18n::locale();
        let state = history.state().unwrap_or(JsValue::NULL);
        if history.replace_state_with_url(&JsValue::NULL, "", Some(&locale.path("/"))).is_err() {
            return false;
        }
        for url in super::chain_of(&landing) {
            let kept = if url == landing { state.clone() } else { JsValue::NULL };
            let _ = history.push_state_with_url(&kept, "", Some(&locale.path(&url)));
        }
        true
    }

    /// The history as a phone's stacks have it (`stacked`), or, where the first use of a page come
    /// to from outside is to lay the way up under it (`landing`), as it will be then: Start, the
    /// way up and the page, the browser at the page.
    pub(super) fn stacked_or_seeded() -> Option<(Vec<Seen>, usize, usize, Option<Landing>)> {
        match landing() {
            Some(landing) => {
                let entries: Vec<Seen> = std::iter::once("/".to_string()).chain(super::chain_of(&landing.0)).map(|url| Seen { url: Some(url), key: String::new(), same: true }).collect();
                let at = entries.len().checked_sub(1)?;
                Some((entries, at, 0, Some(landing)))
            }
            None => stacked().map(|(entries, at, base)| (entries, at, base, None)),
        }
    }

    // ---- a change of tabs writing the history ----

    struct Rewrite {
        ops: std::collections::VecDeque<super::Op>,
        done: Option<Box<dyn FnOnce()>>,
        /// What the history's own scrolling was before (`scrollRestoration`), back at the end.
        scrolling: Option<String>,
        /// Ends a step whose `popstate` never came.
        guard: Option<leptos::prelude::TimeoutHandle>,
        /// The entry a step of `history.go` leads to: a `popstate` elsewhere is the visitor's own.
        expect: Option<usize>,
        /// The way up to lay under a page come to from outside first (`seed_now`).
        seed: Option<Landing>,
        /// The history is being written: it waits for the frame that shows the tap.
        started: bool,
    }

    thread_local! {
        static REWRITE: RefCell<Option<Rewrite>> = const { RefCell::new(None) };
        static AFTER: RefCell<Vec<Box<dyn FnOnce()>>> = const { RefCell::new(Vec::new()) };
        static STALE: RefCell<std::collections::HashSet<String>> = RefCell::new(std::collections::HashSet::new());
    }

    pub(super) fn rewriting() -> bool {
        REWRITE.with(|rewrite| rewrite.borrow().is_some())
    }

    pub(super) fn after_rewrite(then: Box<dyn FnOnce()>) {
        if rewriting() {
            AFTER.with(|after| after.borrow_mut().push(then));
        } else {
            then();
        }
    }

    pub(super) fn go(delta: i32) {
        if delta != 0 {
            if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
                let _ = history.go_with_delta(delta);
            }
        }
    }

    fn write(push: bool, entry: &super::Entry) {
        let Some(history) = web_sys::window().and_then(|window| window.history().ok()) else { return };
        let state: JsValue = match &entry.semester {
            Some(semester) => {
                let object = leptos::web_sys::js_sys::Object::new();
                let _ = Reflect::set(&object, &JsValue::from_str("betula.semester"), &JsValue::from_str(semester));
                object.into()
            }
            None => JsValue::NULL,
        };
        let url = crate::i18n::locale().path(&entry.url);
        let _ = if push { history.push_state_with_url(&state, "", Some(&url)) } else { history.replace_state_with_url(&state, "", Some(&url)) };
    }

    /// Runs the steps of the rewrite until one waits for the browser (`Go`) or none is left.
    fn run() {
        loop {
            let op = REWRITE.with(|rewrite| rewrite.borrow_mut().as_mut().and_then(|rewrite| rewrite.ops.pop_front()));
            match op {
                Some(super::Op::Push(entry)) => write(true, &entry),
                Some(super::Op::Replace(entry)) => write(false, &entry),
                Some(super::Op::Go(delta)) => {
                    // A step whose `popstate` does not come within a second ends the rewrite where
                    // the browser is.
                    let guard = leptos::prelude::set_timeout_with_handle(finish, std::time::Duration::from_millis(1000)).ok();
                    let expect = seen().and_then(|(_, at)| at.checked_add_signed(delta as isize));
                    REWRITE.with(|rewrite| {
                        if let Some(rewrite) = rewrite.borrow_mut().as_mut() {
                            rewrite.guard = guard;
                            rewrite.expect = expect;
                        }
                    });
                    go(delta);
                    return;
                }
                None => {
                    finish();
                    return;
                }
            }
        }
    }

    fn finish() {
        let Some(rewrite) = REWRITE.with(|rewrite| rewrite.borrow_mut().take()) else { return };
        if let Some(guard) = rewrite.guard {
            guard.clear();
        }
        // What lies ahead now is what the change of tabs left over: Forward does not lead there.
        // Not where the visitor's own step came before anything was written: what lies ahead is
        // theirs then.
        if rewrite.started {
            if let Some((entries, at)) = seen() {
                STALE.with(|stale| stale.borrow_mut().extend(entries.into_iter().skip(at + 1).map(|entry| entry.key)));
            }
        }
        if let (Some(scrolling), Some(history)) = (rewrite.scrolling, web_sys::window().and_then(|window| window.history().ok())) {
            let _ = Reflect::set(&history, &JsValue::from_str("scrollRestoration"), &JsValue::from_str(&scrolling));
        }
        // Not from inside the listener of `popstate` that ended the rewrite: the router hears its
        // own `popstate` then (`Pending::release`), and that listener would be called again while
        // it runs.
        let done = rewrite.done;
        leptos::prelude::set_timeout(
            move || {
                if let Some(done) = done {
                    done();
                }
                let after = AFTER.with(|after| std::mem::take(&mut *after.borrow_mut()));
                for then in after {
                    then();
                }
            },
            std::time::Duration::ZERO,
        );
    }

    pub(super) fn on_popstate() -> bool {
        if rewriting() {
            // Not the entry the step led to, or no step taken yet: the visitor went back or forward
            // meanwhile. The rewrite ends where the browser is, and the router hears of the step as
            // of any other.
            let (started, expected) = REWRITE.with(|rewrite| rewrite.borrow().as_ref().map_or((true, None), |rewrite| (rewrite.started, rewrite.expect)));
            let at = seen().map(|(_, at)| at);
            if !started || (expected.is_some() && at.is_some() && expected != at) {
                finish();
                return false;
            }
            REWRITE.with(|rewrite| {
                if let Some(guard) = rewrite.borrow_mut().as_mut().and_then(|rewrite| rewrite.guard.take()) {
                    guard.clear();
                }
            });
            run();
            return true;
        }
        let key = web_sys::window().and_then(|window| get(&window.into(), "navigation")).and_then(|navigation| get(&navigation, "currentEntry")).and_then(|entry| get(&entry, "key")).and_then(|key| key.as_string());
        if key.is_some_and(|key| STALE.with(|stale| stale.borrow().contains(&key))) {
            go(-1);
            return true;
        }
        false
    }

    /// Writes the history for a change of tabs: Start and `stack` above it, the browser at its
    /// top (`super::plan`), and first the way up under a page come to from outside (`landing`). The
    /// tab left keeps its stack (`save`). What the tap changes shows at once (`Pending::hold`); the
    /// history is written after the frame that shows it, so that the tap answers as quickly as a
    /// link's, and the router takes the top at the end. False where the history cannot be written
    /// in place: an entry it would walk through is of another load of a page.
    pub(super) fn tab_to(pending: crate::pending::Pending, from: (&str, &str), entries: Vec<Seen>, at: usize, base: usize, stack: Vec<super::Entry>, landing: Option<Landing>) -> bool {
        if entries.get(base..=at).is_none_or(|walked| walked.iter().any(|entry| !entry.same)) {
            return false;
        }
        save(&entries, base, at);
        let top = stack.last().map_or_else(|| "/".to_string(), |entry| entry.url.clone());
        let (top_path, top_search) = top.split_once('?').unwrap_or((top.as_str(), ""));
        let turn = crate::pending::change(from.0, from.1, top_path, top_search, true).and_then(|change| pending.hold(top.clone(), change));
        let ops = super::plan(base, at, &stack).into();
        // The page the router then shows starts at its top, as a link's does.
        let top_of_page = || {
            if let Some(window) = web_sys::window() {
                window.scroll_to_with_x_and_y(0.0, 0.0);
            }
        };
        let done: Box<dyn FnOnce()> = Box::new(move || match turn {
            Some(turn) => pending.release(turn, top_of_page),
            None => top_of_page(),
        });
        REWRITE.with(|rewrite| *rewrite.borrow_mut() = Some(Rewrite { ops, done: Some(done), scrolling: None, guard: None, expect: None, seed: landing, started: false }));
        folia_design::nav::after_paint(begin);
        true
    }

    /// The frame that shows the tap is there: the history is written now (`tab_to`), unless the
    /// visitor's own step ended the rewrite meanwhile.
    fn begin() {
        let Some(seed) = REWRITE.with(|rewrite| {
            rewrite.borrow_mut().as_mut().map(|rewrite| {
                rewrite.started = true;
                rewrite.seed.take()
            })
        }) else {
            return;
        };
        if let Some(landing) = seed {
            seed_now(landing);
        }
        // The page the router then shows starts at its top: the browser's own scrolling waits.
        if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
            let scrolling = Reflect::get(&history, &JsValue::from_str("scrollRestoration")).ok().and_then(|value| value.as_string());
            let _ = Reflect::set(&history, &JsValue::from_str("scrollRestoration"), &JsValue::from_str("manual"));
            REWRITE.with(|rewrite| {
                if let Some(rewrite) = rewrite.borrow_mut().as_mut() {
                    rewrite.scrolling = scrolling;
                }
            });
        }
        run();
    }

    // ---- the stacks the tabs were left with ----

    const STACKS: &str = "betula.nav.stacks";

    fn name(area: Area) -> &'static str {
        match area {
            Area::Home => "home",
            Area::Catalog => "catalog",
            Area::Programs => "programs",
            Area::Bookmarks => "bookmarks",
            Area::Studyplan => "studyplan",
        }
    }

    /// The stacks as kept: a line each, the tab's name, the row's semester, then the addresses.
    fn stacks() -> Vec<(String, Option<String>, Vec<String>)> {
        folia_design::nav::session_get(STACKS)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let area = parts.next()?.to_string();
                let semester = parts.next().filter(|semester| !semester.is_empty()).map(str::to_string);
                // Only addresses of this app: what is kept ends up in the history.
                let urls: Vec<String> = parts.filter(|url| url.starts_with('/') && !url.starts_with("//")).map(str::to_string).collect();
                (!urls.is_empty()).then_some((area, semester, urls))
            })
            .collect()
    }

    /// The tab of the history keeps its stack, from its first page to where the browser is; the
    /// row's semester where the browser is at „Mein Studium".
    fn save(entries: &[Seen], base: usize, at: usize) {
        let area = tab_at(entries, base, at);
        // Start has no stack of its own (the profile, the legal pages are only above it for a while).
        if area == Area::Home {
            return;
        }
        let urls: Vec<String> = entries.iter().take(at + 1).skip(base + 1).filter_map(|entry| entry.url.clone()).collect();
        let semester = (urls.last().map(String::as_str) == Some(folia_routes::url::STUDY)).then(|| kept("semester")).flatten();
        let mut all: Vec<_> = stacks().into_iter().filter(|(other, _, _)| other != name(area)).collect();
        if !urls.is_empty() {
            all.push((name(area).to_string(), semester, urls));
        }
        let text: Vec<String> = all.into_iter().map(|(area, semester, urls)| format!("{area}\t{}\t{}", semester.unwrap_or_default(), urls.join("\t"))).collect();
        folia_design::nav::session_set(STACKS, &text.join("\n"));
    }

    /// The stack the tab `area` was left with, the row's semester with „Mein Studium".
    pub(super) fn saved(area: Area) -> Option<Vec<super::Entry>> {
        let (_, semester, urls) = stacks().into_iter().find(|(other, _, _)| other == name(area))?;
        Some(urls.into_iter().map(|url| Entry { semester: (url == folia_routes::url::STUDY).then(|| semester.clone()).flatten(), url }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str) -> Entry {
        Entry { url: url.to_string(), semester: None }
    }

    /// What lies above a page in its tab, as a link from outside or from another tab gets it under it.
    #[test]
    fn what_lies_above_a_page() {
        assert_eq!(chain_of("/"), Vec::<String>::new());
        assert_eq!(chain_of("/catalog?turnus=winter"), ["/catalog?turnus=winter"]);
        assert_eq!(chain_of("/catalog?turnus=winter&open=11103"), ["/catalog?turnus=winter", "/catalog?turnus=winter&open=11103"]);
        assert_eq!(chain_of("/catalog/module/11103"), ["/catalog", "/catalog/module/11103"]);
        assert_eq!(chain_of("/catalog/module/11103#termine"), ["/catalog", "/catalog/module/11103"]);
        assert_eq!(chain_of("/study"), ["/study"]);
        assert_eq!(chain_of("/study?view=plan"), ["/study", "/study?view=plan"]);
        assert_eq!(chain_of("/study?view=plan&open=11103&full=1"), ["/study", "/study?view=plan", "/study?view=plan&open=11103", "/study?view=plan&open=11103&full=1"]);
        assert_eq!(chain_of("/programs"), ["/study", "/programs"]);
        assert_eq!(chain_of("/programs?level=master"), ["/study", "/programs?level=master"]);
        assert_eq!(
            chain_of("/programs/bachelor-informatik-2008/areas?area=4&open=11103"),
            ["/study", "/programs", "/programs/bachelor-informatik-2008/areas", "/programs/bachelor-informatik-2008/areas?area=4", "/programs/bachelor-informatik-2008/areas?area=4&open=11103"]
        );
        assert_eq!(chain_of("/bookmarks?sort=title&open=11103"), ["/bookmarks?sort=title", "/bookmarks?sort=title&open=11103"]);
        assert_eq!(chain_of("/studyplan?sem=2026W&open=12104&row=1"), ["/studyplan?sem=2026W", "/studyplan?sem=2026W&open=12104&row=1"]);
        assert_eq!(chain_of("/profile"), ["/profile"]);
    }

    /// A module's own page stays in a tab that shows its modules in place; every other page is its
    /// area's.
    #[test]
    fn the_tab_of_a_page() {
        assert_eq!(tab_of_page("/catalog/module/11103", Area::Programs), Area::Programs);
        assert_eq!(tab_of_page("/catalog/module/11103", Area::Bookmarks), Area::Bookmarks);
        assert_eq!(tab_of_page("/catalog/module/11103", Area::Home), Area::Catalog);
        assert_eq!(tab_of_page("/programs/bachelor-informatik-2008/plan", Area::Catalog), Area::Programs);
        assert_eq!(tab_of_page("/study", Area::Home), Area::Programs);
        assert_eq!(tab_of_page("/profile", Area::Catalog), Area::Home);
    }

    /// The history a change of tabs writes: what is missing pushed first (the tap is fresh), then
    /// the entries in place from Start's next one up, the browser ending at the top.
    #[test]
    fn a_change_of_tabs_writes_the_history() {
        let study = [entry("/study"), entry("/programs")];
        // On Start: the stack pushed, nothing else.
        assert_eq!(plan(0, 0, &study), [Op::Push(entry("/study")), Op::Push(entry("/programs"))]);
        // From a shorter stack: the missing one pushed, then down to Start's next and up again.
        assert_eq!(plan(0, 1, &study), [Op::Push(entry("/programs")), Op::Go(-1), Op::Replace(entry("/study")), Op::Go(1)]);
        // From a longer one: down to Start's next, written in place; what lies ahead is left over.
        let catalog = [entry("/catalog")];
        assert_eq!(plan(0, 2, &catalog), [Op::Go(-1), Op::Replace(entry("/catalog"))]);
        assert_eq!(plan(0, 3, &study), [Op::Go(-2), Op::Replace(entry("/study")), Op::Go(1), Op::Replace(entry("/programs"))]);
        // As long: in place, from the bottom.
        assert_eq!(plan(0, 2, &study), [Op::Go(-1), Op::Replace(entry("/study")), Op::Go(1), Op::Replace(entry("/programs"))]);
        // To Start: down to it, nothing written.
        assert_eq!(plan(0, 2, &[]), [Op::Go(-2)]);
        // Start further up the history (a page before it in the messenger's tab): the same above it.
        assert_eq!(plan(3, 4, &study), [Op::Push(entry("/programs")), Op::Go(-1), Op::Replace(entry("/study")), Op::Go(1)]);
        // Never a step of nothing (`history.go(0)` reloads the page).
        for at in 0..5 {
            for len in 0..4 {
                let stack: Vec<Entry> = (0..len).map(|k| entry(&format!("/x{k}"))).collect();
                assert!(plan(0, at, &stack).iter().all(|op| *op != Op::Go(0)), "{at} {len}");
            }
        }
    }

    #[test]
    fn a_query_without_some_of_its_keys() {
        assert_eq!(without("/catalog?turnus=winter&open=11103&form=lecture", &["open"]), "/catalog?turnus=winter&form=lecture");
        assert_eq!(without("/catalog?open=11103", &["open"]), "/catalog");
        assert_eq!(without("/catalog", &["open"]), "/catalog");
        assert!(has("/catalog?turnus=winter&open=1", &["open"]) && !has("/catalog?opened=1", &["open"]));
    }
}
