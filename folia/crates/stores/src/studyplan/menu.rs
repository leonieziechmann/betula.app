//! The semesters „Einplanen" plans into besides its own (owner, 2026-10-17: „wenn man es lange
//! gedrückt hält (auf dem handy) soll dabei sich die border aufladen und wrnn das voll ist soll ein
//! dropdown mit so 3 bis 5 semestern kommen in die man das einplanen kann. Also das aktuelle und
//! dann die nächsten … wenn man das Semester noch nicht angelegt hat soll das trotzdem gehen. Beim
//! pc soll einfach on hover das dropdown kommen, oder wenn mann den shortcut lange gedrückt hält
//! oder mit modifier"): a menu under the switch beside „Merken" (`PlanLook::Hero`) with the
//! current semester and the four after it (`semesters`), each planning the module there or taking
//! it out. A semester nothing is planned into yet takes it all the same: the plan has a semester
//! once something is in it, and „Mein Studium" and the Stundenplan show it from then on.
//!
//! What opens it: a mouse resting on the switch (`HOVER_OPEN`; it goes once the mouse has left the
//! switch and the menu, `HOVER_CLOSE`), and what enhance.js hears and names with
//! `betula:plan-menu` on the switch — a finger held on it while its border fills (`hold`), P held
//! or Shift+P (`keys`), a right click (`click`). Opened by the keys, the semester P plans into has
//! the focus: the arrow keys go through the menu, Enter takes one, and so does its digit, which a
//! wide screen writes at its end. Esc, a click beside it, a scroll, another page close it; a
//! semester taken flips at once (R21), is written after the next frame (`press`, as the switch's
//! click) and the menu goes a moment later, so that what happened is seen.
//!
//! The menu stands over the page, fixed to the window (`PlanMenuHost`, once in the app): a module
//! beside the list is a panel that scrolls, and on a phone a sheet that moves, and neither may clip
//! it or carry it along. Under the pair of switches, so that it never covers „Merken", at their
//! right edge; on a phone across the screen, 8 px off its edges, as a picker's list is.

use std::collections::BTreeSet;
use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::TurnusSeason;
use folia_plans::study::Offer;
use folia_plans::studyplan::PlanDoc;
use leptos::prelude::*;

use super::{aim_of, press, semester_entry, Aim, PlanHint, Studyplan, APP};
use crate::i18n::{self, use_location, Texts};
use crate::myprogram::MyProgram;
use folia_data::use_data;
use folia_design::ui::Icon;

/// A mouse resting on the switch opens the menu this much later. Longer than at the way to the
/// profile page (160 ms): the switch is a button that is clicked, and a mouse on its way to the
/// click opens nothing.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const HOVER_OPEN: Duration = Duration::from_millis(320);

/// A menu the mouse opened goes this much later once the mouse has left it and the switch: enough
/// to cross from the switch to the menu (past „Merken" where the two stand one over the other).
const HOVER_CLOSE: Duration = Duration::from_millis(280);

/// How long a semester taken stays in the menu, flipped, before the menu goes.
const TAKEN: Duration = Duration::from_millis(220);

/// The semesters after the current one the menu offers: with it, five.
const AHEAD: i32 = 4;

/// The widest the menu gets on a wide screen (it is as wide as its semesters and their marks, at
/// least 232 px, app.css); a phone's spans the screen.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const WIDTH: f64 = 360.0;

/// Between the switches and the menu.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const GAP: f64 = 6.0;

/// What opened the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum By {
    /// A mouse resting on the switch: it goes when the mouse goes, and the focus stays where it is.
    Hover,
    /// A finger held on the switch (enhance.js `hold`).
    Hold,
    /// P held, or Shift+P (`keys`): the semester P plans into has the focus, the digits show.
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

/// The module a menu plans, as its switch was given it (`PlanButton`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Planning {
    pub id: String,
    pub title: String,
    pub current: SemesterKey,
    pub newest: Option<SemesterKey>,
    pub turnus: Option<TurnusSeason>,
    pub offer: Offer,
    /// The page's hint as it was when the menu opened.
    pub hint: Option<PlanHint>,
}

/// Where the menu stands in the window.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
    /// Its right edge, from the window's, and how wide it may get from there leftwards.
    right: f64,
    max_width: f64,
    /// Under the switches (`top`), or above them where the window has more room there (`bottom`).
    top: Option<f64>,
    bottom: Option<f64>,
    max_height: f64,
    /// Across the screen, 8 px off its edges (app.css).
    phone: bool,
}

impl Place {
    fn style(self) -> String {
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("top:{top:.0}px"),
            (None, Some(bottom)) => format!("bottom:{bottom:.0}px"),
            (None, None) => String::new(),
        };
        // A phone's spans the screen (app.css).
        let across = if self.phone { String::new() } else { format!("right:{:.0}px;max-width:{:.0}px;", self.right, self.max_width) };
        format!("{across}max-height:{:.0}px;{vertical}", self.max_height)
    }
}

/// The menu that is open.
#[derive(Clone, Debug, PartialEq)]
struct Open {
    what: Planning,
    by: By,
    place: Place,
}

/// The menu of the semesters of „Einplanen" (see the module): one for the app, where its switch
/// asks for it. The browser app's alone.
#[derive(Clone, Copy)]
pub struct PlanMenu {
    open: RwSignal<Option<Open>>,
    /// What a mouse's coming or going, or a semester taken, does a moment later.
    timer: StoredValue<Option<TimeoutHandle>>,
    /// A semester was taken: the menu goes (`TAKEN`) whatever the mouse does meanwhile.
    taken: StoredValue<bool>,
}

impl PlanMenu {
    /// Creates the menu's place, closed, and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let menu = PlanMenu { open: RwSignal::new(None), timer: StoredValue::new(None), taken: StoredValue::new(false) };
        provide_context(menu);
        menu
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Whether the menu is open for the module. Tracked: the switch's `aria-expanded`.
    pub(super) fn is_open_for(self, id: &str) -> bool {
        self.open.with(|open| open.as_ref().is_some_and(|open| open.what.id == id))
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
    pub(super) fn close(self) {
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

    /// A mouse came onto the switch (`what` its module): an open menu stays, and a closed one
    /// opens a moment later (`HOVER_OPEN`), unless the mouse leaves before.
    pub(super) fn pointer_on(self, ev: &leptos::ev::PointerEvent, what: impl FnOnce() -> Planning + 'static) {
        if ev.pointer_type() != "mouse" || self.taken.get_value() {
            return;
        }
        self.cancel();
        if self.open.with_untracked(Option::is_some) {
            return;
        }
        #[cfg(feature = "csr")]
        {
            use leptos::wasm_bindgen::JsCast;
            let Some(switch) = ev.current_target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()) else { return };
            let open = move || {
                self.timer.set_value(None);
                self.open_at(what(), By::Hover, &switch);
            };
            self.timer.set_value(set_timeout_with_handle(open, HOVER_OPEN).ok());
        }
        #[cfg(not(feature = "csr"))]
        let _ = what;
    }

    /// A mouse left the switch or the menu: a menu the mouse opened goes a moment later
    /// (`HOVER_CLOSE`), unless the mouse comes back onto either.
    pub(super) fn pointer_off(self, ev: &leptos::ev::PointerEvent) {
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

    /// A semester was taken: the menu goes a moment later (`TAKEN`), so that its flip is seen.
    fn taken(self) {
        self.close_later(TAKEN);
        self.taken.set_value(true);
    }

    /// Opens the menu for `what` under `switch` (enhance.js names how in `detail`).
    #[cfg(feature = "csr")]
    pub(super) fn open_named(self, what: Planning, detail: Option<&str>, switch: &leptos::web_sys::Element) {
        self.open_at(what, By::named(detail), switch);
    }

    #[cfg(feature = "csr")]
    fn open_at(self, what: Planning, by: By, switch: &leptos::web_sys::Element) {
        let count = semesters(&what, &PlanDoc::default()).len();
        let place = place_of(switch, count);
        self.cancel();
        self.open.set(Some(Open { what, by, place }));
    }

    /// The keys while the menu is open: Esc closes it, a digit takes its semester; inside it, the
    /// arrows go from semester to semester and Tab leaves it. What the page does with them waits
    /// (enhance.js steps aside while the focus is in the menu, and for Esc while it is open).
    #[cfg(feature = "csr")]
    fn key(self, ev: &leptos::ev::KeyboardEvent) {
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

/// The semesters the menu offers: the current one and the four after it, and the one the switch
/// plans into should it lie beyond them (the finder's, a placeholder's).
pub(super) fn semesters(what: &Planning, doc: &PlanDoc) -> Vec<SemesterKey> {
    let target = aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc).target;
    let mut all: BTreeSet<SemesterKey> = (0..=AHEAD).filter_map(|n| what.current.plus(n)).collect();
    all.insert(target);
    all.into_iter().collect()
}

/// What taking a semester of the menu does: in the semester the switch aims at, what a click on
/// the switch does (for its placeholder, too); in any other, planning the module there or taking
/// it out of it.
pub(super) fn aim_in(what: &Planning, semester: SemesterKey, doc: &PlanDoc) -> Aim {
    let aim = aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc);
    if aim.target == semester {
        return aim;
    }
    Aim { target: semester, pressed: doc.is_planned(semester, &what.id), fill: None, elsewhere: Vec::new() }
}

/// What a semester says at its end: „geplant" where the module is planned, else „nicht
/// angeboten" where its turnus leaves the semester out (R12: what the catalog does not state does
/// not restrict).
fn note_of(what: &Planning, semester: SemesterKey, planned: bool, t: &'static Texts) -> Option<(&'static str, &'static str)> {
    if planned {
        Some((t.planner.planned_mark, "on"))
    } else if !what.offer.offered(semester) {
        Some((t.planner.not_offered, "warn"))
    } else {
        None
    }
}

/// The menu where `PlanMenu` opened it: once in the app, over the page.
#[component]
pub fn PlanMenuHost() -> impl IntoView {
    let t = i18n::t();
    let menu = PlanMenu::expect().filter(|_| APP);
    let plan = Studyplan::expect().filter(|_| APP);
    let mine = MyProgram::expect();
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let Some(menu) = menu else { return };
        let keys = window_event_listener(leptos::ev::keydown, move |ev| menu.key(&ev));
        // A window of another size: the menu goes, it stands where the switches were.
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
        let (menu, plan) = (menu?, plan?);
        let Open { what, by, place } = menu.open.get()?;
        let start = mine.and_then(MyProgram::start);
        let (semesters, target) = plan.with_untracked(|doc| (semesters(&what, doc), aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc).target));
        // Opened by the keys, or by a finger or a right click: the semester P plans into has the
        // focus. A mouse resting on the switch leaves it where it is.
        let first = NodeRef::<leptos::html::Button>::new();
        if by != By::Hover {
            Effect::new(move |_| {
                if let Some(first) = first.get() {
                    focus_quietly(&first);
                }
            });
        }
        let entries = semesters
            .into_iter()
            .enumerate()
            .map(|(i, semester)| {
                let node = if semester == target { first } else { NodeRef::new() };
                let label = semester_entry(semester, start, t);
                view! { <Entry menu plan what=what.clone() semester n=i + 1 label node/> }
            })
            .collect_view();
        let close = move |_| menu.close();
        Some(view! {
            {(by != By::Hover).then(|| view! { <div class="plan-menu-shade" on:click=close on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); menu.close() }></div> })}
            <div
                class="plan-menu"
                class:on-phone=place.phone
                class:by-keys=by == By::Keys
                class:above=place.top.is_none()
                role="menu"
                aria-label=(t.planner.semesters_of)(&what.title)
                style=place.style()
                on:pointerenter=move |ev| menu.pointer_in(&ev)
                on:pointerleave=move |ev| menu.pointer_off(&ev)
                on:contextmenu=|ev: leptos::ev::MouseEvent| ev.prevent_default()
            >
                <p class="plan-menu-head" aria-hidden="true">{t.planner.plan_into}</p>
                {entries}
            </div>
        })
    }
}

/// One semester of the menu: planned there or not, flipped at once by a click (R21) and written
/// after the next frame as the switch's click is (`press`); the menu goes a moment later.
#[component]
fn Entry(menu: PlanMenu, plan: Studyplan, what: Planning, semester: SemesterKey, n: usize, label: String, node: NodeRef<leptos::html::Button>) -> impl IntoView {
    let t = i18n::t();
    let source = use_data().ok();
    let planned = {
        let what = what.clone();
        Memo::new(move |_| plan.with(|doc| aim_in(&what, semester, doc).pressed))
    };
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| planned.get()));
    let take = {
        let what = what.clone();
        move |_: leptos::ev::MouseEvent| {
            if said.get_untracked().is_some() {
                return;
            }
            let aim = plan.with_untracked(|doc| aim_in(&what, semester, doc));
            let was = aim.pressed;
            said.set(Some(!was));
            press(plan, source.clone(), what.id.clone(), aim, was, move || {
                said.try_set(None);
            });
            menu.taken();
        }
    };
    let icon = move || match pressed.get() {
        true => view! { <Icon name="calendar-check-2"/> }.into_any(),
        false => view! { <Icon name="calendar-plus"/> }.into_any(),
    };
    let note = move || {
        note_of(&what, semester, pressed.get(), t).map(|(text, class)| view! { <span class=format!("plan-menu-note {class}")>{text}</span> })
    };
    view! {
        <button
            class="plan-menu-item"
            type="button"
            role="menuitemcheckbox"
            aria-checked=move || if pressed.get() { "true" } else { "false" }
            data-n=n
            node_ref=node
            on:click=take
        >
            {icon}<span class="plan-menu-label">{label}</span>{note}<kbd aria-hidden="true">{n}</kbd>
        </button>
    }
}

/// Where the menu stands for a switch: under the pair of switches it is one of (so that it never
/// covers „Merken"), at their right edge, or above them where the window has more room there.
#[cfg(feature = "csr")]
fn place_of(switch: &leptos::web_sys::Element, entries: usize) -> Place {
    let pair = switch.closest(".switches").ok().flatten();
    let rect = pair.as_ref().unwrap_or(switch).get_bounding_client_rect();
    let size = |value: Result<leptos::wasm_bindgen::JsValue, leptos::wasm_bindgen::JsValue>| value.ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
    let (width, height) = leptos::web_sys::window().map_or((0.0, 0.0), |window| (size(window.inner_width()), size(window.inner_height())));
    let phone = folia_design::nav::is_phone();
    // As tall as its entries and its head make it (app.css).
    let tall = if phone { 50.0 } else { 38.0 } * entries as f64 + 44.0;
    let (below, above) = (height - rect.bottom() - GAP - 8.0, rect.top() - GAP - 8.0);
    let right = (width - rect.right()).max(8.0);
    let max_width = (width - right - 8.0).min(WIDTH);
    if below >= tall || below >= above {
        Place { right, max_width, top: Some(rect.bottom() + GAP), bottom: None, max_height: below.max(120.0), phone }
    } else {
        Place { right, max_width, top: None, bottom: Some(height - rect.top() + GAP), max_height: above.max(120.0), phone }
    }
}

/// The focus back on the switch the menu stood at, where the focus was in the menu.
fn focus_switch() {
    #[cfg(feature = "csr")]
    if inside_menu() {
        folia_design::nav::focus_selector(".plan-toggle[aria-expanded=\"true\"]");
    }
}

/// Whether the focus is in the menu.
#[cfg(feature = "csr")]
fn inside_menu() -> bool {
    let document = leptos::web_sys::window().and_then(|window| window.document());
    document.and_then(|document| document.active_element()).is_some_and(|active| active.closest(".plan-menu").ok().flatten().is_some())
}

/// Whether the focus is in a field that takes the digits.
#[cfg(feature = "csr")]
fn typing() -> bool {
    let document = leptos::web_sys::window().and_then(|window| window.document());
    document.and_then(|document| document.active_element()).is_some_and(|active| matches!(active.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
}

/// Focuses an entry without scrolling anything (a scroll would close the menu).
#[cfg(feature = "csr")]
fn focus_quietly(element: &leptos::web_sys::HtmlElement) {
    let options = leptos::web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    let _ = element.focus_with_options(&options);
}

#[cfg(not(feature = "csr"))]
fn focus_quietly<T>(_: &T) {}

/// The entries of the menu, in order.
#[cfg(feature = "csr")]
fn entries() -> Vec<leptos::web_sys::HtmlElement> {
    use leptos::wasm_bindgen::JsCast;
    let document = leptos::web_sys::window().and_then(|window| window.document());
    let Some(list) = document.and_then(|document| document.query_selector_all(".plan-menu .plan-menu-item").ok()) else { return Vec::new() };
    (0..list.length()).filter_map(|i| list.item(i)).filter_map(|node| node.dyn_into::<leptos::web_sys::HtmlElement>().ok()).collect()
}

/// The arrow keys, Home and End in the menu: from entry to entry, round at either end.
#[cfg(feature = "csr")]
fn step_focus(key: &str) {
    let all = entries();
    let Some(last) = all.len().checked_sub(1) else { return };
    let active = leptos::web_sys::window().and_then(|window| window.document()).and_then(|document| document.active_element());
    let at = active.and_then(|active| {
        all.iter().position(|entry| {
            let entry: &leptos::web_sys::Element = entry;
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

/// What scrolls closes the menu (a panel, the page, a phone's sheet): it stands where the switches
/// were, fixed to the window. A scroll inside the menu (a short window) is its own. Scrolls do not
/// bubble, so the window hears them on their way down (capture). The host is the app's and lives
/// as long as the page: so does the listener.
#[cfg(feature = "csr")]
fn close_on_scroll(menu: PlanMenu) {
    use leptos::wasm_bindgen::closure::Closure;
    use leptos::wasm_bindgen::JsCast;
    let Some(window) = leptos::web_sys::window() else { return };
    let callback = Closure::<dyn Fn(leptos::web_sys::Event)>::new(move |ev: leptos::web_sys::Event| {
        if menu.open.with_untracked(Option::is_none) {
            return;
        }
        let own = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()).is_some_and(|el| el.closest(".plan-menu").ok().flatten().is_some());
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
    use folia_model::labels::TurnusParity;

    use super::*;
    use crate::i18n::{DE, EN};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn planning(turnus: Option<TurnusSeason>, hint: Option<PlanHint>) -> Planning {
        Planning {
            id: "12330".into(),
            title: "Datenbanken".into(),
            current: key("2026W"),
            newest: Some(key("2026W")),
            turnus,
            offer: Offer { season: turnus, parity: None },
            hint,
        }
    }

    #[test]
    fn the_menu_offers_the_current_semester_and_the_four_after_it() {
        let doc = PlanDoc::default();
        let what = planning(None, None);
        assert_eq!(semesters(&what, &doc), ["2026W", "2027S", "2027W", "2028S", "2028W"].map(key));
        // The semester the switch plans into stands among them where it lies beyond them.
        let far = planning(None, Some(PlanHint { semester: Some(key("2030S")), fill: None }));
        assert_eq!(semesters(&far, &doc), ["2026W", "2027S", "2027W", "2028S", "2028W", "2030S"].map(key));
        // What the plan holds elsewhere does not lengthen it: the switch's small line says that.
        let mut held = PlanDoc::default();
        assert!(held.plan(key("2025W"), "12330", 1, None));
        assert_eq!(semesters(&what, &held).len(), 5);
    }

    /// The owner, 2026-10-17: „wenn man das Semester noch nicht angelegt hat soll das trotzdem
    /// gehen". A semester the plan has nothing in takes the module, and is the plan's from then on.
    #[test]
    fn a_semester_nothing_is_planned_into_yet_takes_the_module() {
        let what = planning(None, None);
        let mut doc = PlanDoc::default();
        let later = key("2028S");
        let aim = aim_in(&what, later, &doc);
        assert_eq!((aim.target, aim.pressed, aim.fill.clone()), (later, false, None));
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 5);
        assert!(doc.is_planned(later, "12330"));
        assert_eq!(doc.semesters(), [later]);
        // Taken again: out of it, and the semester with it.
        let aim = aim_in(&what, later, &doc);
        assert!(aim.pressed);
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 6);
        assert!(doc.semesters().is_empty());
    }

    #[test]
    fn the_semester_the_switch_aims_at_does_what_the_switch_does() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(folia_plans::studyplan::Placeholder {
            pid: 3,
            semester: now,
            program_id: "079-82-2008".into(),
            ord: 17,
            span: (3, 3),
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: "Fachübergreifendes Studium".into(),
        });
        assert!(doc.plan(now, "12330", 1, None));
        let what = planning(None, Some(PlanHint { semester: None, fill: Some(3) }));
        // Planned there, but not for the placeholder: as on the switch, taking it counts it.
        let aim = aim_in(&what, now, &doc);
        assert_eq!((aim.pressed, aim.fill.as_ref().map(|(pid, _)| *pid)), (false, Some(3)));
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 2);
        assert_eq!(doc.fillers(3).len(), 1);
        // Another semester: planned there plainly, for no placeholder.
        let next = aim_in(&what, key("2027S"), &doc);
        assert_eq!((next.pressed, next.fill.clone()), (false, None));
        super::super::toggle_in(&mut doc, &what.id, &next, next.pressed, &[], 3);
        assert!(doc.is_planned(key("2027S"), "12330") && doc.fillers(3).len() == 1);
    }

    #[test]
    fn a_semester_says_whether_the_module_is_planned_or_offered_there() {
        let winter = planning(Some(TurnusSeason::Winter), None);
        assert_eq!(note_of(&winter, key("2027S"), false, &DE), Some(("nicht angeboten", "warn")));
        assert_eq!(note_of(&winter, key("2027W"), false, &DE), None);
        assert_eq!(note_of(&winter, key("2027S"), true, &DE), Some(("geplant", "on")));
        assert_eq!(note_of(&winter, key("2027S"), false, &EN), Some(("not offered", "warn")));
        // Every semester, or nothing stated: nothing to say.
        assert_eq!(note_of(&planning(Some(TurnusSeason::Both), None), key("2027S"), false, &DE), None);
        assert_eq!(note_of(&planning(None, None), key("2027S"), false, &DE), None);
        // Odd years only: the winter of 2027 is one, that of 2028 is not.
        let odd = Planning { offer: Offer { season: Some(TurnusSeason::Winter), parity: Some(TurnusParity::Odd) }, ..winter };
        assert_eq!((note_of(&odd, key("2027W"), false, &DE), note_of(&odd, key("2028W"), false, &DE)), (None, Some(("nicht angeboten", "warn"))));
    }

    #[test]
    fn enhance_js_names_what_opened_it() {
        assert_eq!([Some("hold"), Some("keys"), Some("click"), None, Some("x")].map(By::named), [By::Hold, By::Keys, By::Click, By::Click, By::Click]);
    }
}
