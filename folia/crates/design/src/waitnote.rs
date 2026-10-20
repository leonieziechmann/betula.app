//! A note that waits for interaction (owner, 2026-10-18: „Bau diese notification mal bitte als wait
//! for interaction notification. Vielleicht brauchen wir das später nochmal"): at the bottom of the
//! window, over the page, it says what was just done and offers what follows from it (its
//! children), and stays — it has no timer — until the student does something. Something done with
//! it keeps it; anything else is moving on, and it goes (the same day: „sobald erkannt wurde, dass
//! damit nicht interagiert wurde und der Nutzer weiter macht soll es ausgeblendet werden. Aber wir
//! müssen aufpassen, dass versehebtliche Interaktionen, wie ein klein wenig scrollen kein Problem
//! sind"):
//!
//! - a click or a tap elsewhere; a key that is not its own (`on_key`) — not a modifier, Tab, a
//!   function key or a key with a modifier, which are the browser's and the page's shortcuts;
//! - another page or tab, another module opened or its panel or sheet closed, other filters or
//!   another search: the address but its `page`, which the catalog's list writes as it scrolls;
//! - a scroll of anything further than a fifth of the window (140 to 260 px); a little one does not
//!   count;
//! - put away: × (a wide screen), Esc, a finger swiping it down (the same day: „Auf dem Handy soll es
//!   die Geste geben die notification nach unten weg zu wischen").
//!
//! What happens within `GRACE` of its coming, or of a change of what it says (`changed`), is the
//! page answering what was done, not the student moving on. A part of it may take a finger swiped
//! sideways (`Sideways`: the semester of the note of „Einplanen", `folia_widgets::plannote`); a
//! swipe is never a tap as well.
//!
//! It goes down out of sight, from where it stands, and then the host lets go of it (the same day:
//! „wenn man da die Modul Ansicht schließt sollte die notification auch mit eher Animation nach
//! unten raus geschlossen werden. Aktuell ist das ziemlich unangenehm anzusehen"). Its place is
//! kept from its coming (`placed`): what changes around it — a module's sheet that closes on a
//! phone, a page without a sidebar — never moves it on its way out. A module's sheet sliding away
//! (enhance.js `sheet-away` on the document) takes it along at once.
//!
//! On a wide screen a bar across the page right of the rail; on a phone a card as wide as the bottom
//! bar over it, under it in the order of what lies on what and flat, so it casts nothing on the bar
//! and comes up from behind it (the same day: „keinen schatten auf die navbar werfen"); dark in both
//! themes (app.css „A note that waits for interaction", `.wait-note`). The host shows it while it
//! has something to say, gives it what it says as one element, and takes it away on `on_gone`.

use std::time::Duration;

use leptos::ev::{KeyboardEvent, PointerEvent};
use leptos::prelude::*;

use crate::i18n;
use crate::ui::Icon;

/// What happens this soon (ms) after the note came or changed is the page answering what was done.
const GRACE: f64 = 400.0;
/// A scroll moves on once it went a fifth of the window, at least and at most so far (px).
#[cfg(feature = "csr")]
const SCROLLED: f64 = 0.2;
#[cfg(feature = "csr")]
const SCROLLED_MIN: f64 = 140.0;
#[cfg(feature = "csr")]
const SCROLLED_MAX: f64 = 260.0;
/// How far a finger moves (px) before it is a swipe: sideways over `Sideways::zone` it is that
/// part's, down it puts the note away.
const SLOP: f64 = 10.0;
/// A swipe sideways steps once it went this far (px), one down puts the note away from this far;
/// flicked (px per ms, as the rows of the lists measure it), from half of either.
const STEP_AT: f64 = 40.0;
const AWAY_AT: f64 = 36.0;
const FLICK: f64 = 0.45;
/// How long the note takes to go down and away (app.css, `.wait-note.leaving`).
const LEAVING: Duration = Duration::from_millis(200);
/// How long after a swipe a click is the swipe's end, not a tap (ms).
#[cfg(feature = "csr")]
const NO_TAP: f64 = 400.0;

/// A part of the note a finger swipes sideways, as a page turns: to the left the next, to the right
/// the one before.
#[derive(Clone, Copy)]
pub struct Sideways {
    /// What it is, within the note (a selector).
    pub zone: &'static str,
    /// How far its words follow the finger while it swipes (px); 0 at rest.
    pub slide: RwSignal<f64>,
    /// A swipe past `STEP_AT`, or flicked: 1 for the next, -1 for the one before.
    pub step: Callback<i32>,
    /// Whether a step that way can be taken; the words hold back the way it cannot.
    pub can: Callback<i32, bool>,
}

/// How a finger on the note was read once it moved past `SLOP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    /// Not yet past `SLOP`: a tap, so far.
    Still,
    /// Sideways over `Sideways::zone`.
    Sideways,
    /// Down: it puts the note away.
    Down,
    /// Anything else (up, sideways elsewhere): nothing, and no tap.
    Astray,
}

/// A finger on the note, until it lets go.
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: i32,
    x: f64,
    y: f64,
    /// How far it went, and when it moved last (the event's time).
    dx: f64,
    dy: f64,
    at: f64,
    /// How fast it went at last, along its way (px per ms).
    speed: f64,
    way: Way,
    /// It came down on the part that takes a sideways swipe.
    in_zone: bool,
}

/// The note: what it says (`children`, one element), its × and all it waits for. Shown while the
/// host has it; `on_gone` when it should go — also from a note a newer one has taken the place of
/// in the moment between them (a key that makes the next note), so the host lets go of this one
/// alone.
#[component]
pub fn WaitNote(
    /// The student moved on, or put the note away: the host takes it away.
    on_gone: Callback<()>,
    /// Changes whenever what it says changes by the student's hand (a step): what follows this
    /// soon belongs to that.
    #[prop(optional, into)]
    changed: Option<Signal<u32>>,
    /// A key while it stands, before it counts as moving on: `true` where the note took it (← and
    /// → of its semester).
    #[prop(optional)]
    on_key: Option<Callback<KeyboardEvent, bool>>,
    /// A part that takes a finger swiped sideways.
    #[prop(optional_no_strip)]
    sideways: Option<Sideways>,
    /// While true something the note opened lies over the page (the menu of the note after
    /// marking, `folia_widgets::marknote`): nothing done there is moving on, and the note waits
    /// behind it; once it is closed the note waits as after a change (`changed`).
    #[prop(optional, into)]
    held: Option<Signal<bool>>,
    /// Once true the note goes, as if put away: what it offered is done (on a phone „Fertig" of
    /// the menu it opened).
    #[prop(optional, into)]
    away: Option<Signal<bool>>,
    /// Classes of its own besides `wait-note`.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let t = i18n::t();
    let root = NodeRef::<leptos::html::Div>::new();
    let gone = move || {
        let _ = on_gone.try_run(());
    };
    // Where it stands, as its coming put it: kept until it is gone.
    let placed = RwSignal::new(None::<String>);
    // When it came or changed last.
    let came = StoredValue::new(clock());
    if let Some(changed) = changed {
        Effect::new(move |_| {
            changed.track();
            came.set_value(clock());
        });
    }
    // While held, nothing is moving on; let go, it waits as after a change.
    let holding = move || held.is_some_and(|held| held.try_get_untracked().unwrap_or(false));
    if let Some(held) = held {
        Effect::new(move |_| {
            held.track();
            came.set_value(clock());
        });
    }
    let fresh = move || holding() || clock() - came.try_get_value().unwrap_or(f64::NEG_INFINITY) < GRACE;


    // A finger: the part that takes it sideways follows (`Sideways::slide`), the note down
    // (`lowered`); while it is on the note nothing eases (`dragging`). Swiped away, the note goes
    // down out of sight (`leaving`) before it is gone.
    let finger = StoredValue::new(None::<Finger>);
    let lowered = RwSignal::new(0.0_f64);
    let dragging = RwSignal::new(false);
    let leaving = RwSignal::new(false);
    let swiped = StoredValue::new(f64::NEG_INFINITY);
    // It goes: down from where it is to out of sight, and then the host lets go of it.
    let leave = move || {
        if leaving.try_get_untracked().unwrap_or(true) {
            return;
        }
        let down = lowered.get_untracked() + distance_out(root);
        dragging.set(false);
        leaving.set(true);
        lowered.set(down);
        set_timeout(gone, LEAVING);
    };

    // Done with: it goes.
    if let Some(away) = away {
        Effect::new(move |_| {
            if away.get() {
                leave();
            }
        });
    }

    // Another page, module, filter or search.
    let location = i18n::use_location();
    let here = Memo::new(move |_| (location.pathname.get(), steps_of(&location.search.get())));
    Effect::new(move |before: Option<(String, String)>| {
        let now = here.get();
        if before.is_some_and(|before| before != now) && !fresh() {
            leave();
        }
        now
    });

    #[cfg(feature = "csr")]
    {
        // Its place, once it is in the page; taken anew when the window changes its size.
        root.on_load(move |element| {
            request_animation_frame(move || placed.set(place_of(&element)));
        });
        let resized = window_event_listener(leptos::ev::resize, move |_| {
            placed.set(None);
            request_animation_frame(move || {
                if let Some(element) = root.get_untracked() {
                    placed.set(place_of(&element));
                }
            });
        });
        on_cleanup(move || resized.remove());
        let keys = window_event_listener(leptos::ev::keydown, move |ev| {
            if is_moving_on(&ev, root, on_key) && !fresh() {
                leave();
            }
        });
        on_cleanup(move || keys.remove());
        if let Some(watch) = watch(root, fresh, swiped, leave) {
            on_cleanup(move || drop(watch));
        }
    }
    // The server renders no note: nothing to hear there.
    #[cfg(not(feature = "csr"))]
    let _ = on_key;

    // The finger comes down: a mouse clicks, a finger or a pen may swipe.
    let down = move |ev: PointerEvent| {
        if ev.pointer_type() == "mouse" || leaving.get_untracked() || finger.get_value().is_some() {
            return;
        }
        let in_zone = sideways.is_some_and(|sideways| within(&ev, sideways.zone));
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        finger.set_value(Some(Finger { id: ev.pointer_id(), x, y, dx: 0.0, dy: 0.0, at: ev.time_stamp(), speed: 0.0, way: Way::Still, in_zone }));
    };
    let moved = move |ev: PointerEvent| {
        let Some(mut now) = finger.get_value().filter(|finger| finger.id == ev.pointer_id()) else { return };
        let (dx, dy) = (f64::from(ev.client_x()) - now.x, f64::from(ev.client_y()) - now.y);
        let elapsed = (ev.time_stamp() - now.at).max(1.0);
        if now.way == Way::Still && (dx.abs() > SLOP || dy.abs() > SLOP) {
            now.way = if dx.abs() > dy.abs() {
                if now.in_zone { Way::Sideways } else { Way::Astray }
            } else if dy > 0.0 {
                Way::Down
            } else {
                Way::Astray
            };
            if matches!(now.way, Way::Sideways | Way::Down) {
                if let Some(root) = root.get_untracked() {
                    let _ = root.set_pointer_capture(ev.pointer_id());
                }
                dragging.set(true);
            }
        }
        match (now.way, sideways) {
            (Way::Sideways, Some(sideways)) => {
                now.speed = (dx - now.dx) / elapsed;
                // The way it cannot go, the words hold back.
                let open = sideways.can.run(if dx < 0.0 { 1 } else { -1 });
                sideways.slide.set(if open { dx.clamp(-90.0, 90.0) } else { dx * 0.2 });
            }
            (Way::Down, _) => {
                now.speed = (dy - now.dy) / elapsed;
                lowered.set(dy.max(0.0));
            }
            _ => {}
        }
        now.dx = dx;
        now.dy = dy;
        now.at = ev.time_stamp();
        finger.set_value(Some(now));
    };
    let up = move |ev: PointerEvent| {
        let Some(now) = finger.get_value().filter(|finger| finger.id == ev.pointer_id()) else { return };
        finger.set_value(None);
        dragging.set(false);
        if now.way != Way::Still {
            swiped.set_value(clock());
        }
        match (now.way, sideways) {
            (Way::Sideways, Some(sideways)) => {
                sideways.slide.set(0.0);
                let way = if now.dx < 0.0 { 1 } else { -1 };
                let flicked = now.dx.abs() >= STEP_AT / 2.0 && now.speed.abs() >= FLICK && now.speed.signum() == now.dx.signum();
                if (now.dx.abs() >= STEP_AT || flicked) && sideways.can.run(way) {
                    sideways.step.run(way);
                }
            }
            (Way::Down, _) => {
                let flicked = now.dy >= AWAY_AT / 2.0 && now.speed >= FLICK;
                if now.dy >= AWAY_AT || flicked {
                    leave();
                } else {
                    lowered.set(0.0);
                }
            }
            _ => {}
        }
    };
    let cancel = move |ev: PointerEvent| {
        if finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
            finger.set_value(None);
            dragging.set(false);
            if let Some(sideways) = sideways {
                sideways.slide.set(0.0);
            }
            if !leaving.get_untracked() {
                lowered.set(0.0);
            }
        }
    };
    view! {
        <div
            class=format!("wait-note {class}")
            class:leaving=move || leaving.get()
            node_ref=root
            role="status"
            data-drag=move || dragging.get().then_some("")
            style=move || {
                let mut style = placed.get().unwrap_or_default();
                let down = lowered.get();
                if down != 0.0 {
                    style.push_str(&format!("transform:translateY({down:.1}px);"));
                }
                (!style.is_empty()).then_some(style)
            }
            on:pointerdown=down
            on:pointermove=moved
            on:pointerup=up
            on:pointercancel=cancel
        >
            {children()}
            <button class="wait-note-close" type="button" aria-label=t.common.close title=format!("{} (Esc)", t.common.close) on:click=move |_| leave()>
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// What of an address is a step of the student's: all of it but `page`, which the catalog's list
/// writes as it scrolls.
fn steps_of(search: &str) -> String {
    search.trim_start_matches('?').split('&').filter(|pair| !pair.is_empty() && pair.split('=').next() != Some("page")).collect::<Vec<_>>().join("&")
}

/// The time on the page's clock, in ms.
fn clock() -> f64 {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now())
    }
    #[cfg(not(feature = "csr"))]
    0.0
}

/// Whether the pointer came down within an element `selector` names.
fn within(ev: &PointerEvent, selector: &str) -> bool {
    use leptos::wasm_bindgen::JsCast;
    ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()).and_then(|target| target.closest(selector).ok().flatten()).is_some()
}

/// How far down the note has to go to be out of sight: from its bottom to the window's, and its
/// height.
fn distance_out(root: NodeRef<leptos::html::Div>) -> f64 {
    #[cfg(feature = "csr")]
    {
        let window_height = web_sys::window().and_then(|window| window.inner_height().ok()?.as_f64());
        if let (Some(element), Some(window_height)) = (root.get_untracked(), window_height) {
            let rect = element.get_bounding_client_rect();
            return (window_height - rect.bottom()).max(0.0) + rect.height() + 24.0;
        }
    }
    let _ = root;
    120.0
}

/// Where the note stands, from what its styles make of it now (the side a page's sidebar leaves,
/// over a phone's bottom bar or a module's sheet): kept in its own style, so that what changes
/// around it does not move it.
#[cfg(feature = "csr")]
fn place_of(element: &web_sys::HtmlDivElement) -> Option<String> {
    let style = web_sys::window()?.get_computed_style(element).ok()??;
    let value = |name: &str| style.get_property_value(name).ok().filter(|value| !value.is_empty());
    Some(format!("left:{};right:{};bottom:{};z-index:{};", value("left")?, value("right")?, value("bottom")?, value("z-index")?))
}

/// Whether a key is the student moving on: not taken by the note (`on_key`), not on the note itself
/// (Enter or the space bar on its buttons), not a modifier, Tab, a function key or a key with a
/// modifier. Esc puts the note away, also from the note.
#[cfg(feature = "csr")]
fn is_moving_on(ev: &KeyboardEvent, root: NodeRef<leptos::html::Div>, on_key: Option<Callback<KeyboardEvent, bool>>) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let key = ev.key();
    if on_key.is_some_and(|on_key| on_key.run(ev.clone())) {
        return false;
    }
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Node>().ok());
    if root.get_untracked().zip(target).is_some_and(|(root, target)| root.contains(Some(&target))) {
        return key == "Escape";
    }
    let function_key = key.strip_prefix('F').is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    let quiet = matches!(key.as_str(), "Shift" | "Control" | "Alt" | "AltGraph" | "Meta" | "CapsLock" | "Tab" | "Dead" | "Unidentified" | "Process");
    key == "Escape" || !(function_key || quiet || ev.ctrl_key() || ev.meta_key() || ev.alt_key())
}

/// What the note hears on the page, on its way down (capture): a click elsewhere, and a scroll of
/// anything — each element's first position since the note came kept, one further than `far` from
/// there moving on; within `GRACE` the positions are taken anew. A click on the note right after a
/// swipe is the swipe's end and goes no further. A module's sheet sliding away (`sheet-away`,
/// enhance.js) takes the note along. Dropping it removes the listeners.
#[cfg(feature = "csr")]
struct Watch {
    document: web_sys::Document,
    click: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
    scroll: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
    sheet_away: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(feature = "csr")]
impl Drop for Watch {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;
        let _ = self.document.remove_event_listener_with_callback_and_bool("click", self.click.as_ref().unchecked_ref(), true);
        let _ = self.document.remove_event_listener_with_callback_and_bool("scroll", self.scroll.as_ref().unchecked_ref(), true);
        let _ = self.document.remove_event_listener_with_callback("sheet-away", self.sheet_away.as_ref().unchecked_ref());
    }
}

#[cfg(feature = "csr")]
fn watch(root: NodeRef<leptos::html::Div>, fresh: impl Fn() -> bool + Copy + 'static, swiped: StoredValue<f64>, leave: impl Fn() + Copy + 'static) -> Option<send_wrapper::SendWrapper<Watch>> {
    use wasm_bindgen::JsCast;
    let document = web_sys::window()?.document()?;
    let on_note = move |target: Option<web_sys::EventTarget>| {
        let target = target.and_then(|target| target.dyn_into::<web_sys::Node>().ok());
        root.get_untracked().zip(target).is_some_and(|(root, target)| {
            let root: &web_sys::Node = root.as_ref();
            root.contains(Some(&target))
        })
    };
    let click = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        if on_note(ev.target()) {
            if clock() - swiped.try_get_value().unwrap_or(f64::NEG_INFINITY) < NO_TAP {
                ev.stop_immediate_propagation();
                ev.prevent_default();
            }
        } else if !fresh() {
            leave();
        }
    });
    // The note they were seen for, and where each element stood first.
    let mut seen: Vec<(web_sys::EventTarget, f64, f64)> = Vec::new();
    let scroll = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(target) = ev.target() else { return };
        let window = web_sys::window();
        let (top, left) = match target.dyn_ref::<web_sys::Element>() {
            Some(element) => (f64::from(element.scroll_top()), f64::from(element.scroll_left())),
            None => window.as_ref().map_or((0.0, 0.0), |window| (window.scroll_y().unwrap_or(0.0), window.scroll_x().unwrap_or(0.0))),
        };
        let far = window.and_then(|window| window.inner_height().ok()?.as_f64()).map_or(SCROLLED_MIN, |height| (height * SCROLLED).clamp(SCROLLED_MIN, SCROLLED_MAX));
        let early = fresh();
        match seen.iter_mut().find(|(known, ..)| *known == target) {
            Some(first) if early => *first = (target, top, left),
            Some((_, top_first, left_first)) => {
                if (top - *top_first).abs() > far || (left - *left_first).abs() > far {
                    leave();
                }
            }
            None => seen.push((target, top, left)),
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    options.set_passive(true);
    document.add_event_listener_with_callback_and_add_event_listener_options("scroll", scroll.as_ref().unchecked_ref(), &options).ok()?;
    document.add_event_listener_with_callback_and_bool("click", click.as_ref().unchecked_ref(), true).ok()?;
    let sheet_away = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| leave());
    document.add_event_listener_with_callback("sheet-away", sheet_away.as_ref().unchecked_ref()).ok()?;
    Some(send_wrapper::SendWrapper::new(Watch { document, click, scroll, sheet_away }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_writing_its_page_is_no_step() {
        assert_eq!(steps_of("?q=Analysis&page=3&open=11103"), "q=Analysis&open=11103");
        assert_eq!(steps_of("?page=2"), "");
        assert_eq!(steps_of(""), "");
        assert_ne!(steps_of("?q=Analysis"), steps_of("?q=Analysis&open=11103"));
    }
}
