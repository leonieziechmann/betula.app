//! A switch's menu: what a switch offers besides its click, held at it (owner, 2026-10-17, of
//! „Einplanen": „wenn man es lange gedrückt hält (auf dem handy) soll dabei sich die border
//! aufladen und wrnn das voll ist soll ein dropdown … kommen … Beim pc soll einfach on hover das
//! dropdown kommen, oder wenn mann den shortcut lange gedrückt hält oder mit modifier"; and the same
//! day: „Bau das system gleich so, dass man das dann später auch für merken machen kann, so dass man
//! da mehrere gruppen haben kann … Und es soll halt einheitlich sein"). The click of the switch stays
//! what it was; the menu holds the other choices — the semesters of „Einplanen"
//! (`folia_stores::studyplan`), later the lists of „Merken" — and is opened the same way wherever a
//! switch has one:
//!
//! - a mouse resting on the switch (`HOVER_OPEN`); it goes once the mouse has left the switch and
//!   the menu (`HOVER_CLOSE`), and leaves the focus where it is;
//! - a finger held on it while its border fills (`HoldRing`), a right click, and the switch's key
//!   held or with Shift: enhance.js hears these on any `[data-hold-menu]` and names what opened it
//!   in `betula:hold-menu` on the switch (`hold`, `click`, `keys`), which `HoldMenu::listen` hears.
//!   Opened so, the entry the switch's click means (`data-default`) has the focus: the arrow keys go
//!   through the menu, Enter takes an entry, and so does its digit, which a wide screen writes at
//!   its end.
//!
//! An entry taken flips at once (R21) and the menu goes a moment later (`TAKEN`), so that what
//! happened is seen; Esc, a click beside it, a scroll, a window of another size and another page
//! close it, and the focus goes back to the switch where it was in the menu. The menu stands over
//! the page, fixed to the window (`HoldMenuHost`, once in the app: a module beside the list is a
//! panel that scrolls, and on a phone a sheet that moves, and neither may clip it or carry it along):
//! under the switch (`Anchor::Below`; under the pair of switches it is one of, so that it never
//! covers the other) or beside it (`Anchor::Beside`, a sidebar's action, whose neighbours stay
//! free); on a phone across the screen 8 px off its edges, as a picker's list is, under the switch
//! or above it where there is more room.
//!
//! What a switch gives: its menu, made when it opens (`MenuOf`): whose it is (the switch's
//! `aria-expanded`, `is_open_for`), its name and head, and its entries, `HoldItem`s. The browser
//! app's alone.

use std::time::Duration;

use leptos::prelude::*;

use crate::i18n::use_location;
use crate::ui::Icon;

/// A mouse resting on a switch opens its menu this much later. Longer than at the way to the
/// profile page (160 ms): a switch is a button that is clicked, and a mouse on its way to the
/// click opens nothing.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const HOVER_OPEN: Duration = Duration::from_millis(320);

/// A menu the mouse opened goes this much later once the mouse has left it and the switch: enough
/// to cross from the switch to the menu (past the other switch of a pair standing one over the
/// other).
const HOVER_CLOSE: Duration = Duration::from_millis(280);

/// How long an entry taken stays in the menu, flipped, before the menu goes.
const TAKEN: Duration = Duration::from_millis(220);

/// The widest a menu gets on a wide screen (it is as wide as what it says, at least 232 px,
/// app.css); a phone's spans the screen.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const WIDTH: f64 = 360.0;

/// Between the switch and its menu.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const GAP: f64 = 6.0;

/// Where a menu stands at its switch on a wide screen (a phone's spans the screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Under the switch at its right edge — under the pair of switches it is one of (`.switches`),
    /// so that it never covers the other — or above it where the window has more room there.
    Below,
    /// Beside the switch, at its right, its first entry at the switch's height: a sidebar's
    /// action, whose neighbours above and below stay free. Under it where the window has no room
    /// at the right.
    Beside,
}

/// What opened the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum By {
    /// A mouse resting on the switch: it goes when the mouse goes, and the focus stays where it is.
    Hover,
    /// A finger held on the switch (enhance.js `hold`).
    Hold,
    /// The switch's key held, or with Shift (`keys`): the digits show.
    Keys,
    /// A right click (`click`).
    Click,
}

impl By {
    /// What enhance.js names in the event's `detail`.
    #[cfg_attr(not(feature = "csr"), allow(dead_code))]
    fn named(detail: Option<&str>) -> Self {
        match detail {
            Some("hold") => By::Hold,
            Some("keys") => By::Keys,
            _ => By::Click,
        }
    }
}

/// A switch's menu, made when it opens.
#[derive(Clone)]
pub struct MenuOf {
    /// Whose menu it is, the switch's kind and its thing (`plan:12330`): what `is_open_for` asks.
    pub owner: String,
    /// The menu's name for screen readers („Analysis I: Semester").
    pub label: String,
    /// The line at its head („Einplanen in").
    pub head: String,
    /// How many entries it has: where it fits.
    pub rows: usize,
    /// Its entries, `HoldItem`s.
    pub entries: ViewFn,
}

/// Where a menu stands in the window.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
    /// From the left edge of the window (`Beside`), or its right edge from the window's (`Below`).
    left: Option<f64>,
    right: Option<f64>,
    max_width: f64,
    /// Its top, or its foot from the bottom of the window (above the switch).
    top: Option<f64>,
    bottom: Option<f64>,
    max_height: f64,
    /// Across the screen, 8 px off its edges (app.css).
    phone: bool,
}

impl Place {
    fn style(self) -> String {
        if self.phone {
            return match (self.top, self.bottom) {
                (Some(top), _) => format!("top:{top:.0}px;max-height:{:.0}px", self.max_height),
                (None, Some(bottom)) => format!("bottom:{bottom:.0}px;max-height:{:.0}px", self.max_height),
                (None, None) => String::new(),
            };
        }
        let across = match (self.left, self.right) {
            (Some(left), _) => format!("left:{left:.0}px;"),
            (None, Some(right)) => format!("right:{right:.0}px;"),
            (None, None) => String::new(),
        };
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("top:{top:.0}px"),
            (None, Some(bottom)) => format!("bottom:{bottom:.0}px"),
            (None, None) => String::new(),
        };
        format!("{across}max-width:{:.0}px;max-height:{:.0}px;{vertical}", self.max_width, self.max_height)
    }
}

/// The menu that is open.
#[derive(Clone)]
struct Opened {
    menu: MenuOf,
    by: By,
    place: Place,
}

/// The menu of the switches (see the module): one for the app.
#[derive(Clone, Copy)]
pub struct HoldMenu {
    open: RwSignal<Option<Opened>>,
    /// What a mouse's coming or going, or an entry taken, does a moment later.
    timer: StoredValue<Option<TimeoutHandle>>,
    /// An entry was taken: the menu goes (`TAKEN`) whatever the mouse does meanwhile.
    taken: StoredValue<bool>,
}

impl HoldMenu {
    /// Creates the menu's place, closed, and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let menu = HoldMenu { open: RwSignal::new(None), timer: StoredValue::new(None), taken: StoredValue::new(false) };
        provide_context(menu);
        menu
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Whether the menu of `owner` is open. Tracked: the switch's `aria-expanded`.
    pub fn is_open_for(self, owner: &str) -> bool {
        self.open.with(|open| open.as_ref().is_some_and(|open| open.menu.owner == owner))
    }

    /// Drops what a mouse's coming or going was about to do.
    fn cancel(self) {
        self.taken.set_value(false);
        if let Some(handle) = self.timer.get_value() {
            handle.clear();
        }
        self.timer.set_value(None);
    }

    /// Closes the menu. Where the focus was in it, it goes back to the switch (it goes with the
    /// menu).
    pub fn close(self) {
        self.cancel();
        if self.open.with_untracked(Option::is_some) {
            focus_switch();
            self.open.set(None);
        }
    }

    /// Closes the menu a moment later (`after`), unless something opens or keeps it meanwhile.
    fn close_later(self, after: Duration) {
        self.cancel();
        let close = move || {
            self.timer.set_value(None);
            self.close();
        };
        self.timer.set_value(set_timeout_with_handle(close, after).ok());
    }

    /// An entry was taken: the menu goes a moment later (`TAKEN`), so that its flip is seen.
    pub fn taken(self) {
        self.close_later(TAKEN);
        self.taken.set_value(true);
    }

    /// A mouse came onto a switch (its `pointerenter`): an open menu stays, and a closed one opens
    /// a moment later (`HOVER_OPEN`) at the switch, made by `make` (`None`: nothing to offer now),
    /// unless the mouse leaves before.
    pub fn pointer_on(self, ev: &leptos::ev::PointerEvent, at: Anchor, make: impl FnOnce() -> Option<MenuOf> + 'static) {
        if ev.pointer_type() != "mouse" || self.taken.get_value() {
            return;
        }
        self.cancel();
        if self.open.with_untracked(Option::is_some) {
            return;
        }
        #[cfg(feature = "csr")]
        {
            use wasm_bindgen::JsCast;
            let Some(switch) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return };
            let open = move || {
                self.timer.set_value(None);
                if let Some(menu) = make() {
                    self.open_at(menu, By::Hover, at, &switch);
                }
            };
            self.timer.set_value(set_timeout_with_handle(open, HOVER_OPEN).ok());
        }
        #[cfg(not(feature = "csr"))]
        let _ = (at, make);
    }

    /// A mouse left a switch or the menu (their `pointerleave`): a menu the mouse opened goes a
    /// moment later (`HOVER_CLOSE`), unless the mouse comes back onto either.
    pub fn pointer_off(self, ev: &leptos::ev::PointerEvent) {
        if ev.pointer_type() != "mouse" || self.taken.get_value() {
            return;
        }
        self.cancel();
        if self.open.with_untracked(|open| open.as_ref().is_some_and(|open| open.by == By::Hover)) {
            self.close_later(HOVER_CLOSE);
        }
    }

    /// A mouse came onto the menu: it stays.
    fn pointer_in(self, ev: &leptos::ev::PointerEvent) {
        if ev.pointer_type() == "mouse" && !self.taken.get_value() {
            self.cancel();
        }
    }

    /// Hears what enhance.js names on the switch `node` (`betula:hold-menu`: a finger held, the
    /// keys, a right click) and opens its menu, made by `make` (`None`: nothing to offer now), at
    /// it. For the life of the switch.
    #[allow(unused_variables)]
    pub fn listen(self, node: NodeRef<leptos::html::Button>, at: Anchor, make: impl Fn() -> Option<MenuOf> + Clone + 'static) {
        #[cfg(feature = "csr")]
        Effect::new(move |_| {
            let make = make.clone();
            let handle = window_event_listener_untyped("betula:hold-menu", move |ev| {
                use wasm_bindgen::JsCast;
                let Some(button) = node.get_untracked() else { return };
                let own: &web_sys::Element = &button;
                if ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).as_ref() != Some(own) {
                    return;
                }
                let detail = ev.dyn_ref::<web_sys::CustomEvent>().and_then(|ev| ev.detail().as_string());
                if let Some(menu) = make() {
                    self.open_at(menu, By::named(detail.as_deref()), at, own);
                }
            });
            on_cleanup(move || handle.remove());
        });
    }

    #[cfg(feature = "csr")]
    fn open_at(self, menu: MenuOf, by: By, at: Anchor, switch: &web_sys::Element) {
        let place = place_of(switch, at, menu.rows);
        self.cancel();
        self.open.set(Some(Opened { menu, by, place }));
    }

    /// The keys while the menu is open: Esc closes it, a digit takes its entry; inside it, the
    /// arrows go from entry to entry and Tab leaves it. What the page does with them waits
    /// (enhance.js steps aside while the focus is in the menu, and for Esc while it is open).
    #[cfg(feature = "csr")]
    fn key(self, ev: &web_sys::KeyboardEvent) {
        if self.open.with_untracked(Option::is_none) || ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
            return;
        }
        let inside = inside_menu();
        match ev.key().as_str() {
            "Escape" => {
                ev.prevent_default();
                self.close();
            }
            "Tab" if inside => self.close(),
            "ArrowDown" | "ArrowUp" | "Home" | "End" if inside => {
                ev.prevent_default();
                step_focus(&ev.key());
            }
            digit => {
                let Some(n) = digit.parse::<usize>().ok().filter(|n| (1..=9).contains(n)) else { return };
                if typing() {
                    return;
                }
                ev.prevent_default();
                click_entry(n);
            }
        }
    }
}

/// The menu where `HoldMenu` opened it: once in the app, over the page.
#[component]
pub fn HoldMenuHost() -> impl IntoView {
    let menu = HoldMenu::expect().filter(|_| cfg!(feature = "csr"));
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let Some(menu) = menu else { return };
        let keys = window_event_listener(leptos::ev::keydown, move |ev| menu.key(&ev));
        // A window of another size: the menu goes, it stands where the switch was.
        let resize = window_event_listener(leptos::ev::resize, move |_| {
            if menu.open.with_untracked(Option::is_some) {
                menu.close();
            }
        });
        close_on_scroll(menu);
        on_cleanup(move || {
            keys.remove();
            resize.remove();
        });
    });
    // Another page: the menu goes.
    let location = use_location();
    Effect::new(move |before: Option<()>| {
        location.pathname.track();
        location.search.track();
        if let Some(menu) = menu.filter(|_| before.is_some()) {
            menu.close();
        }
    });
    move || {
        let menu = menu?;
        let Opened { menu: of, by, place } = menu.open.get()?;
        // Opened by the keys, a finger or a right click: the entry the switch's click means has the
        // focus. A mouse resting on the switch leaves it where it is.
        if by != By::Hover {
            Effect::new(move |_| focus_default());
        }
        let close = move |_| menu.close();
        Some(view! {
            {(by != By::Hover).then(|| view! { <div class="hold-menu-shade" on:click=close on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); menu.close() }></div> })}
            <div
                class="hold-menu"
                class:on-phone=place.phone
                class:by-keys=by == By::Keys
                class:above=place.top.is_none()
                class:beside=place.left.is_some()
                role="menu"
                aria-label=of.label
                style=place.style()
                on:pointerenter=move |ev| menu.pointer_in(&ev)
                on:pointerleave=move |ev| menu.pointer_off(&ev)
                on:contextmenu=|ev: leptos::ev::MouseEvent| ev.prevent_default()
            >
                <p class="hold-menu-head" aria-hidden="true">{of.head}</p>
                {of.entries.run()}
            </div>
        })
    }
}

/// An entry of a switch's menu: a switch of its own, checked or not, with its icon for either
/// (`on`, `off`), its words, what it says at its end (`note`: the words and their look, a class of
/// `.hold-menu-note`), and its digit (`n`, from 1), which a wide screen writes. `default`: the
/// entry the switch's click means, which has the focus when the menu opens by the keys, a finger
/// or a right click. Taken, it runs `take` and the menu goes a moment later; `take` flips what
/// `checked` reads at once (R21).
#[component]
pub fn HoldItem<F>(
    n: usize,
    #[prop(into)] label: String,
    #[prop(into)] checked: Signal<bool>,
    on: &'static str,
    off: &'static str,
    #[prop(optional, into)] note: Signal<Option<(&'static str, &'static str)>>,
    #[prop(optional)] default: bool,
    take: F,
) -> impl IntoView
where
    F: Fn() + 'static,
{
    let menu = HoldMenu::expect();
    let icon = move || match checked.get() {
        true => view! { <Icon name=on/> }.into_any(),
        false => view! { <Icon name=off/> }.into_any(),
    };
    let note = move || note.get().map(|(text, class)| view! { <span class=format!("hold-menu-note {class}")>{text}</span> });
    view! {
        <button
            class="hold-menu-item"
            type="button"
            role="menuitemcheckbox"
            aria-checked=move || if checked.get() { "true" } else { "false" }
            data-n=n
            data-default=default.then_some("")
            on:click=move |_| {
                take();
                if let Some(menu) = menu {
                    menu.taken();
                }
            }
        >
            {icon}<span class="hold-menu-label">{label}</span>{note}<kbd aria-hidden="true">{n}</kbd>
        </button>
    }
}

/// The border that fills while a finger holds a switch with a menu (enhance.js, app.css
/// `.hold-charge`): a ring on the switch's edge, its corners as round as the switch's (`radius`,
/// px), its length 100. Part of the switch's server HTML, out of the way of everything.
#[component]
pub fn HoldRing(radius: f64) -> impl IntoView {
    view! { <svg class="hold-charge" aria-hidden="true"><rect width="100%" height="100%" rx=radius pathLength="100"/></svg> }
}

/// Where a menu stands for a switch (`Anchor`), and on a phone across the screen under or above
/// it.
#[cfg(feature = "csr")]
fn place_of(switch: &web_sys::Element, at: Anchor, rows: usize) -> Place {
    let size = |value: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>| value.ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
    let (width, height) = web_sys::window().map_or((0.0, 0.0), |window| (size(window.inner_width()), size(window.inner_height())));
    let phone = crate::nav::is_phone();
    // As tall as its entries and its head make it (app.css).
    let tall = if phone { 50.0 } else { 38.0 } * rows as f64 + 44.0;
    let own = switch.get_bounding_client_rect();
    if at == Anchor::Beside && !phone && own.right() + GAP + 240.0 <= width - 8.0 {
        // Its first entry at the switch's height (the menu's padding and head above it).
        let top = (own.top() - 34.0).clamp(8.0, (height - tall - 8.0).max(8.0));
        let left = own.right() + GAP;
        return Place { left: Some(left), right: None, max_width: (width - left - 8.0).min(WIDTH), top: Some(top), bottom: None, max_height: height - top - 8.0, phone };
    }
    let pair = switch.closest(".switches").ok().flatten();
    let rect = pair.as_ref().map_or(own.clone(), |pair| pair.get_bounding_client_rect());
    let (below, above) = (height - rect.bottom() - GAP - 8.0, rect.top() - GAP - 8.0);
    let right = (width - rect.right()).max(8.0);
    let max_width = (width - right - 8.0).min(WIDTH);
    if below >= tall || below >= above {
        Place { left: None, right: Some(right), max_width, top: Some(rect.bottom() + GAP), bottom: None, max_height: below.max(120.0), phone }
    } else {
        Place { left: None, right: Some(right), max_width, top: None, bottom: Some(height - rect.top() + GAP), max_height: above.max(120.0), phone }
    }
}

/// The focus back on the switch the menu stood at, where the focus was in the menu.
fn focus_switch() {
    #[cfg(feature = "csr")]
    if inside_menu() {
        crate::nav::focus_selector("[data-hold-menu][aria-expanded=\"true\"]");
    }
}

/// Whether the focus is in the menu.
#[cfg(feature = "csr")]
fn inside_menu() -> bool {
    let document = web_sys::window().and_then(|window| window.document());
    document.and_then(|document| document.active_element()).is_some_and(|active| active.closest(".hold-menu").ok().flatten().is_some())
}

/// Whether the focus is in a field that takes the digits.
#[cfg(feature = "csr")]
fn typing() -> bool {
    let document = web_sys::window().and_then(|window| window.document());
    document.and_then(|document| document.active_element()).is_some_and(|active| matches!(active.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
}

/// Focuses an entry without scrolling anything (a scroll would close the menu).
#[cfg(feature = "csr")]
fn focus_quietly(element: &web_sys::HtmlElement) {
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    let _ = element.focus_with_options(&options);
}

/// The entries of the menu, in order.
#[cfg(feature = "csr")]
fn entries() -> Vec<web_sys::HtmlElement> {
    use wasm_bindgen::JsCast;
    let document = web_sys::window().and_then(|window| window.document());
    let Some(list) = document.and_then(|document| document.query_selector_all(".hold-menu .hold-menu-item").ok()) else { return Vec::new() };
    (0..list.length()).filter_map(|i| list.item(i)).filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok()).collect()
}

/// The entry the switch's click means, else the first, takes the focus.
fn focus_default() {
    #[cfg(feature = "csr")]
    {
        let all = entries();
        let chosen = all.iter().find(|entry| entry.has_attribute("data-default")).or(all.first());
        if let Some(entry) = chosen {
            focus_quietly(entry);
        }
    }
}

/// The arrow keys, Home and End in the menu: from entry to entry, round at either end.
#[cfg(feature = "csr")]
fn step_focus(key: &str) {
    let all = entries();
    let Some(last) = all.len().checked_sub(1) else { return };
    let active = web_sys::window().and_then(|window| window.document()).and_then(|document| document.active_element());
    let at = active.and_then(|active| {
        all.iter().position(|entry| {
            let entry: &web_sys::Element = entry;
            *entry == active
        })
    });
    let next = match (key, at) {
        ("Home", _) => 0,
        ("End", _) => last,
        ("ArrowDown", Some(at)) if at < last => at + 1,
        ("ArrowDown", _) => 0,
        ("ArrowUp", Some(at)) if at > 0 => at - 1,
        _ => last,
    };
    if let Some(entry) = all.get(next) {
        focus_quietly(entry);
    }
}

/// A digit: the entry it is written at, taken as a click takes it.
#[cfg(feature = "csr")]
fn click_entry(n: usize) {
    if let Some(entry) = entries().get(n.wrapping_sub(1)) {
        entry.click();
    }
}

/// What scrolls closes the menu (a panel, the page, a phone's sheet): it stands where the switch
/// was, fixed to the window. A scroll inside the menu (a short window) is its own. Scrolls do not
/// bubble, so the window hears them on their way down (capture). The host is the app's and lives
/// as long as the page: so does the listener.
#[cfg(feature = "csr")]
fn close_on_scroll(menu: HoldMenu) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else { return };
    let callback = Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
        if menu.open.with_untracked(Option::is_none) {
            return;
        }
        let own = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).is_some_and(|el| el.closest(".hold-menu").ok().flatten().is_some());
        if !own {
            menu.close();
        }
    });
    if window.add_event_listener_with_callback_and_bool("scroll", callback.as_ref().unchecked_ref(), true).is_ok() {
        callback.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enhance_js_names_what_opened_it() {
        assert_eq!([Some("hold"), Some("keys"), Some("click"), None, Some("x")].map(By::named), [By::Hold, By::Keys, By::Click, By::Click, By::Click]);
    }

    #[test]
    fn a_menu_stands_where_its_place_says() {
        let below = Place { left: None, right: Some(24.0), max_width: 360.0, top: Some(200.0), bottom: None, max_height: 400.0, phone: false };
        assert_eq!(below.style(), "right:24px;max-width:360px;max-height:400px;top:200px");
        let beside = Place { left: Some(300.0), right: None, top: Some(90.0), ..below };
        assert_eq!(beside.style(), "left:300px;max-width:360px;max-height:400px;top:90px");
        // A phone's spans the screen: only how high it stands.
        let above = Place { phone: true, top: None, bottom: Some(310.0), ..below };
        assert_eq!(above.style(), "bottom:310px;max-height:400px");
    }
}
