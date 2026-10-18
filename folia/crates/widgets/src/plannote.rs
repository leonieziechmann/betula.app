//! The note of what „Einplanen" did (owner, 2026-10-18, concept E of
//! folia/design/einplanen/oberflaeche.html: „Also ich liebe Option E, das ist mit Abstand die
//! eleganteste"), over the page at the bottom of the window (`folia_stores::studyplan::Planning`): a
//! check, the semester between ‹ and › with its Fachsemester and what is unusual there („kein
//! Angebot", „liegt zurück"; where there is room also what happened, „eingeplant", „verschoben"),
//! and „Rückgängig". The arrows move the module at once, into the past as well, to the first
//! Fachsemester (← and → do too while the note stands); once it was moved, a second row offers
//! „Weitere Module auch nach SoSe 27" with a switch, and while the session plans into a semester
//! chosen so it stands in every note, to switch it off where it is seen (owner, the same day: „die
//! Option mit, die nächsten Module auch in das Semester planen ist genial. Aber das Design von der
//! Box muss vielleicht nochmal etwas angepasst werden"). A module taken out says so, with
//! „Rückgängig".
//!
//! The semester and its arrows are one control (owner, 2026-10-18, of a phone: „Erstmal müssen die
//! Links und rechts Buttons virtuell über den komplette text da gehen, also wenn man auf der rechten
//! Seite drückt, dann nach rechts und wenn man auf der linken Seite drückt nach links. Dazu muss
//! auch swiping unterstützt werden"): a tap on its left half is ‹, on its right half ›, and a finger
//! swiped sideways over it moves the module a semester the way a page turns (to the left the next
//! one). What it says fits between the arrows (the same day: „Wichtig finde ich, dass das
//! eingeplante Semester (also fs. 12) und die Warnung da steht. Der Rest kann optional ausgeblendet
//! werden"): the semester, its Fachsemester and the warning; what happened only where there is room.
//!
//! It stands 4.5 s, 7 s once moved, and as long as the pointer or the keyboard is on it; a newer one
//! takes its place. It goes as soon as the student moves on (the same day: „sobald erkannt wurde,
//! dass damit nicht interagiert wurde und der Nutzer weiter macht soll es ausgeblendet werden. Aber
//! wir müssen aufpassen, dass versehebtliche Interaktionen, wie ein klein wenig scrollen kein
//! Problem sind"): another page or tab, another module opened or its panel or sheet closed, other
//! filters or another search, a longer scroll (a fifth of the window, `far`) — not a little one, not
//! the list writing its page into the address as it scrolls, and nothing the page does within
//! `GRACE` of the press, which is the page answering it. A finger swipes it down and away (the same
//! day: „Auf dem Handy soll es die Geste geben die notification nach unten weg zu wischen"); × and
//! Esc put it away on a wide screen.
//!
//! On a wide screen one bar across the page right of the rail, one line high, the module named in
//! it (the same day: „bei der pc Version kann die notification ruhig über die ganze Breite rechts
//! von der sidebar gehen und dann vllt etwas weniger hoch sein"); on a phone a card as wide as the
//! bottom bar over it, under it in the order of what lies on what, so it casts nothing on the bar
//! and comes up from behind it (the same day: „keinen schatten auf die navbar werfen"), the switch a
//! row over the semester, so that what is under the finger stays. Dark in both themes (the same
//! day: „dark modus muss natürlich auch dafür funktionieren"; app.css „The note of „Einplanen"",
//! `.plan-toast`).

use std::time::Duration;

use folia_calendar::semester::{fachsemester, SemesterKey};
use leptos::ev::{MouseEvent, PointerEvent};
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{NoteKind, Planning, Pressed, Studyplan};

use crate::i18n::{self, Texts};

/// How long the note stands, and once the module was moved.
const STANDS: Duration = Duration::from_millis(4500);
const STANDS_MOVED: Duration = Duration::from_millis(7000);
/// What follows a change of the note this soon (ms) is the page answering the press, not the
/// student moving on.
const GRACE: f64 = 400.0;
/// A scroll moves on once it went a fifth of the window, at least and at most so far (px).
#[cfg(feature = "csr")]
const SCROLLED: f64 = 0.2;
#[cfg(feature = "csr")]
const SCROLLED_MIN: f64 = 140.0;
#[cfg(feature = "csr")]
const SCROLLED_MAX: f64 = 260.0;
/// How far a finger moves (px) before it is a swipe: sideways over the semester it moves the
/// module, down it puts the note away.
const SLOP: f64 = 10.0;
/// A swipe sideways moves the module once it went this far (px), one down puts the note away from
/// this far; flicked (px per ms, as the rows measure it), from half of either.
const STEP_AT: f64 = 40.0;
const AWAY_AT: f64 = 36.0;
const FLICK: f64 = 0.45;
/// How long the note takes to go down and away (app.css, `.plan-toast.leaving`).
const LEAVING: Duration = Duration::from_millis(200);
/// How long after a swipe a click is the swipe's end, not a tap (ms).
const NO_TAP: f64 = 400.0;

/// The note, once in the app (`folia_app::App`): nothing while there is none.
#[component]
pub fn PlanNote() -> impl IntoView {
    let (Some(planning), Some(plan)) = (Planning::expect(), Studyplan::expect()) else { return ().into_any() };
    let mine = MyProgram::expect();
    // The pointer or the keyboard on the note: it waits.
    let held = RwSignal::new(false);
    // Its while, from each change of it, unless it is held: a newer change leaves the older timer
    // without effect (`shown`).
    Effect::new(move |_| {
        let Some((shown, moved)) = planning.note().map(|note| (note.shown, matches!(note.kind, NoteKind::Planned { stepped: true, .. }))) else { return };
        if held.get() {
            return;
        }
        set_timeout(move || planning.dismiss(Some(shown)), if moved { STANDS_MOVED } else { STANDS });
    });
    // When the note changed last: what follows this soon belongs to the press.
    let changed = StoredValue::new(f64::NEG_INFINITY);
    Effect::new(move |_| {
        let _ = planning.note().map(|note| note.shown);
        changed.set_value(clock());
    });
    let fresh = move || clock() - changed.get_value() < GRACE;
    // Moving on: another page, another module or none, other filters, another search.
    let location = i18n::use_location();
    let here = Memo::new(move |_| (location.pathname.get(), steps_of(&location.search.get())));
    Effect::new(move |before: Option<(String, String)>| {
        let now = here.get();
        if before.is_some_and(|before| before != now) && !fresh() {
            planning.dismiss(None);
        }
        now
    });
    #[cfg(feature = "csr")]
    {
        let keys = window_event_listener(leptos::ev::keydown, move |ev| keys(&ev, planning, plan));
        on_cleanup(move || keys.remove());
        if let Some(scrolls) = watch_scrolls(planning, fresh) {
            on_cleanup(move || drop(scrolls));
        }
    }
    // A new press makes a new note; what changes on one changes inside it, so the focus stays.
    let turn = Memo::new(move |_| planning.note().map(|note| note.turn));
    (move || turn.get().map(|_| view! { <NoteView planning plan mine held/> })).into_any()
}

/// What of an address is a step of the student's: all of it but `page`, which the catalog's list
/// writes as it scrolls (`folia_catalog`).
fn steps_of(search: &str) -> String {
    search.trim_start_matches('?').split('&').filter(|pair| !pair.is_empty() && pair.split('=').next() != Some("page")).collect::<Vec<_>>().join("&")
}

/// The time on the page's clock, in ms.
fn clock() -> f64 {
    #[cfg(feature = "csr")]
    {
        leptos::web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now())
    }
    #[cfg(not(feature = "csr"))]
    0.0
}

/// ← and → while the note of a module planned stands: the module a semester earlier or later, as
/// its arrows; Esc puts the note away. Not where the keys mean something else: in a field, a picker,
/// a menu, a slider, at a panel's handle, in a dialog, or where the page took the key already.
#[cfg(feature = "csr")]
fn keys(ev: &leptos::web_sys::KeyboardEvent, planning: Planning, plan: Studyplan) {
    use leptos::wasm_bindgen::JsCast;
    if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || ev.shift_key() {
        return;
    }
    let Some(note) = planning.note_untracked() else { return };
    let by = match ev.key().as_str() {
        "ArrowLeft" => Some(-1),
        "ArrowRight" => Some(1),
        "Escape" => None,
        _ => return,
    };
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    let elsewhere = "input, textarea, select, [contenteditable], [role=slider], [role=tab], [role=menu], [role=menuitem], [role=listbox], [role=option], .combo, [data-action^='resize-'], dialog";
    if target.is_some_and(|target| target.closest(elsewhere).ok().flatten().is_some()) {
        return;
    }
    match by {
        Some(by) if matches!(note.kind, NoteKind::Planned { .. }) => {
            ev.prevent_default();
            planning.step(plan, by);
        }
        Some(_) => {}
        // Esc closes what else it closes as well (the module's panel): the note goes either way.
        None => planning.dismiss(None),
    }
}

/// A `scroll` anywhere in the page, heard on its way down (capture): each element's first position
/// since the note came is kept, and a scroll of one further than `far` from there puts the note
/// away. Within `GRACE` of a change of the note the positions are taken anew (the page answering
/// the press). Dropping it removes the listener.
#[cfg(feature = "csr")]
struct Scrolls {
    document: leptos::web_sys::Document,
    callback: leptos::wasm_bindgen::closure::Closure<dyn FnMut(leptos::web_sys::Event)>,
}

#[cfg(feature = "csr")]
impl Drop for Scrolls {
    fn drop(&mut self) {
        use leptos::wasm_bindgen::JsCast;
        let _ = self.document.remove_event_listener_with_callback_and_bool("scroll", self.callback.as_ref().unchecked_ref(), true);
    }
}

#[cfg(feature = "csr")]
fn watch_scrolls(planning: Planning, fresh: impl Fn() -> bool + 'static) -> Option<send_wrapper::SendWrapper<Scrolls>> {
    use leptos::wasm_bindgen::JsCast;
    use leptos::web_sys;
    let document = web_sys::window()?.document()?;
    // The note they were seen for, and where each element stood first.
    let mut seen: (u32, Vec<(web_sys::EventTarget, f64, f64)>) = (0, Vec::new());
    let callback = leptos::wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(turn) = planning.note_untracked().map(|note| note.turn) else { return };
        let Some(target) = ev.target() else { return };
        let window = web_sys::window();
        let (top, left) = match target.dyn_ref::<web_sys::Element>() {
            Some(element) => (f64::from(element.scroll_top()), f64::from(element.scroll_left())),
            None => window.as_ref().map_or((0.0, 0.0), |window| (window.scroll_y().unwrap_or(0.0), window.scroll_x().unwrap_or(0.0))),
        };
        let far = window.and_then(|window| window.inner_height().ok()?.as_f64()).map_or(SCROLLED_MIN, |height| (height * SCROLLED).clamp(SCROLLED_MIN, SCROLLED_MAX));
        if seen.0 != turn {
            seen = (turn, Vec::new());
        }
        let early = fresh();
        match seen.1.iter_mut().find(|(known, ..)| *known == target) {
            Some(first) if early => *first = (target, top, left),
            Some((_, top_first, left_first)) => {
                if (top - *top_first).abs() > far || (left - *left_first).abs() > far {
                    planning.dismiss(None);
                }
            }
            None => seen.1.push((target, top, left)),
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    options.set_passive(true);
    document.add_event_listener_with_callback_and_add_event_listener_options("scroll", callback.as_ref().unchecked_ref(), &options).ok()?;
    Some(send_wrapper::SendWrapper::new(Scrolls { document, callback }))
}

/// How a finger on the note was read once it moved past `SLOP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    /// Not yet past `SLOP`: a tap, so far.
    Still,
    /// Sideways over the semester: it moves the module.
    Sideways,
    /// Down: it puts the note away.
    Down,
    /// Anything else (up, sideways off the semester): nothing, and no tap.
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
    /// It came down on the semester (and the note is one of a module planned).
    on_semester: bool,
}

/// One note: what it says changes in place (the arrows, the switch), so that the focus and the
/// pointer stay where they are.
#[component]
fn NoteView(planning: Planning, plan: Studyplan, mine: Option<MyProgram>, held: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let Some(first) = planning.note_untracked() else { return ().into_any() };
    let root = NodeRef::<leptos::html::Div>::new();
    let hold = move || {
        if !held.get_untracked() {
            held.set(true);
            planning.renew();
        }
    };
    let free = move || held.set(false);
    // The keyboard leaving the note for something outside it.
    let focus_out = move |ev: leptos::ev::FocusEvent| {
        if !focus_stays(&ev) {
            free();
        }
    };
    let undo = move |_| {
        held.set(false);
        planning.undo(plan);
    };
    // The finger: the semester's words follow it sideways (`slide`), the note down (`lowered`);
    // while it is on the note nothing eases (`dragging`). Swiped away, the note goes down out of
    // sight (`leaving`) before it is gone.
    let finger = StoredValue::new(None::<Finger>);
    let slide = RwSignal::new(0.0_f64);
    let lowered = RwSignal::new(0.0_f64);
    let dragging = RwSignal::new(false);
    let leaving = RwSignal::new(false);
    let swiped = StoredValue::new(f64::NEG_INFINITY);
    let turn = first.turn;
    let away = move || {
        let height = root.get_untracked().map_or(80.0, |root| f64::from(root.offset_height()));
        dragging.set(false);
        leaving.set(true);
        lowered.set(height + 120.0);
        set_timeout(
            move || {
                if planning.note_untracked().is_some_and(|note| note.turn == turn) {
                    planning.dismiss(None);
                }
            },
            LEAVING,
        );
    };
    // „Rückgängig" at the end, after a line; × on a wide screen.
    let end = view! {
        <div class="plan-toast-end">
            <span class="plan-toast-sep" aria-hidden="true"></span>
            <button class="plan-toast-undo" type="button" on:click=undo>{t.common.undo}</button>
            <button class="plan-toast-close" type="button" aria-label=t.common.close title=format!("{} (Esc)", t.common.close) on:click=move |_| planning.dismiss(None)>
                <Icon name="x"/>
            </button>
        </div>
    };
    let (body, removed, floor_now) = match first.kind {
        NoteKind::Removed { .. } => (
            view! {
                <div class="plan-toast-main">
                    <span class="plan-toast-mark out" aria-hidden="true"><Icon name="calendar-minus"/></span>
                    <span class="plan-toast-said">{(t.planner.note_removed)(&first.what.title)}</span>
                </div>
                {end}
            }
            .into_any(),
            true,
            None,
        ),
        NoteKind::Planned { at, .. } => {
            let what = first.what;
            // Where the module is now, and whether it was moved: the note's alone.
            let now = Memo::new(move |_| match planning.note().map(|note| note.kind) {
                Some(NoteKind::Planned { at, stepped }) => (at, stepped),
                _ => (at, false),
            });
            let start = Memo::new(move |_| mine.and_then(MyProgram::start));
            let chosen = Memo::new(move |_| planning.chosen());
            let floor = planning.floor(what.current);
            // The way the semester came from at the last step: its words come in from that side.
            let came = RwSignal::new(0_i32);
            let go = move |by: i32| {
                if by < 0 && now.get_untracked().0 <= floor {
                    return;
                }
                came.set(by);
                planning.step(plan, by);
            };
            // A tap: the arrow's own way (a key on it as well), else the half of the semester.
            let tap = move |ev: MouseEvent| {
                if clock() - swiped.get_value() < NO_TAP {
                    return;
                }
                if let Some(by) = way_of_tap(&ev) {
                    go(by);
                }
            };
            let words = {
                let what = what.clone();
                move || {
                    let (at, stepped) = now.get();
                    let from = match came.get_untracked() {
                        by if by > 0 => "right",
                        by if by < 0 => "left",
                        _ => "",
                    };
                    let line = line_of(at, stepped, &what, start.get(), t);
                    view! {
                        <span class="plan-toast-now" data-from=from style=move || (slide.get() != 0.0).then(|| format!("transform:translateX({:.1}px);opacity:{:.2}", slide.get(), (1.0 - slide.get().abs() / 140.0).max(0.35)))>
                            <b>{at.short(t.locale)}</b>
                            <small>{line}</small>
                        </span>
                    }
                }
            };
            let keep = move || {
                let (at, stepped) = now.get();
                let on = chosen.get() == Some(at);
                (stepped || chosen.get().is_some()).then(|| view! {
                    <button class="plan-toast-keep" type="button" role="switch" aria-checked=if on { "true" } else { "false" } title=t.planner.keep_hint on:click=move |_| planning.keep(at)>
                        <Icon name="layers"/>
                        <span>{t.planner.keep_for}" "<b>{at.short(t.locale)}</b></span>
                        <span class="plan-toast-switch" aria-hidden="true"></span>
                    </button>
                })
            };
            let title = what.title.clone();
            (
                view! {
                    <div class="plan-toast-main">
                        <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                        // The module, where the bar has room for it (a wide screen).
                        <span class="plan-toast-title">{title}</span>
                        <div class="plan-toast-stepper" role="group" aria-label=(t.planner.semester_of)(&what.title) on:click=tap>
                            <button class="plan-toast-step" type="button" data-by="-1" aria-label=t.planner.earlier title=format!("{} (←)", t.planner.earlier) disabled=move || now.get().0 <= floor>
                                <Icon name="chevron-left"/>
                            </button>
                            <span class="plan-toast-at">{words}</span>
                            <button class="plan-toast-step" type="button" data-by="1" aria-label=t.planner.later title=format!("{} (→)", t.planner.later)>
                                <Icon name="chevron-right"/>
                            </button>
                        </div>
                    </div>
                    {keep}
                    {end}
                }
                .into_any(),
                false,
                Some((now, floor, go)),
            )
        }
    };
    // The finger comes down: a mouse clicks, a finger or a pen may swipe.
    let down = move |ev: PointerEvent| {
        if ev.pointer_type() == "mouse" || leaving.get_untracked() || finger.get_value().is_some() {
            return;
        }
        let on_semester = floor_now.is_some() && target_within(&ev, ".plan-toast-stepper");
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        finger.set_value(Some(Finger { id: ev.pointer_id(), x, y, dx: 0.0, dy: 0.0, at: ev.time_stamp(), speed: 0.0, way: Way::Still, on_semester }));
    };
    let moved = move |ev: PointerEvent| {
        let Some(mut finger_now) = finger.get_value().filter(|finger| finger.id == ev.pointer_id()) else { return };
        let (dx, dy) = (f64::from(ev.client_x()) - finger_now.x, f64::from(ev.client_y()) - finger_now.y);
        let elapsed = (ev.time_stamp() - finger_now.at).max(1.0);
        if finger_now.way == Way::Still && (dx.abs() > SLOP || dy.abs() > SLOP) {
            finger_now.way = if dx.abs() > dy.abs() {
                if finger_now.on_semester { Way::Sideways } else { Way::Astray }
            } else if dy > 0.0 {
                Way::Down
            } else {
                Way::Astray
            };
            if matches!(finger_now.way, Way::Sideways | Way::Down) {
                if let Some(root) = root.get_untracked() {
                    let _ = root.set_pointer_capture(ev.pointer_id());
                }
                dragging.set(true);
            }
        }
        match finger_now.way {
            Way::Sideways => {
                finger_now.speed = (dx - finger_now.dx) / elapsed;
                // At the first Fachsemester the words hold back towards the past.
                let at_floor = floor_now.is_some_and(|(now, floor, _)| now.get_untracked().0 <= floor);
                slide.set(if dx > 0.0 && at_floor { dx * 0.2 } else { dx.clamp(-90.0, 90.0) });
            }
            Way::Down => {
                finger_now.speed = (dy - finger_now.dy) / elapsed;
                lowered.set(dy.max(0.0));
            }
            Way::Still | Way::Astray => {}
        }
        finger_now.dx = dx;
        finger_now.dy = dy;
        finger_now.at = ev.time_stamp();
        finger.set_value(Some(finger_now));
    };
    let up = move |ev: PointerEvent| {
        let Some(finger_now) = finger.get_value().filter(|finger| finger.id == ev.pointer_id()) else { return };
        finger.set_value(None);
        dragging.set(false);
        if finger_now.way != Way::Still {
            swiped.set_value(clock());
        }
        match finger_now.way {
            Way::Sideways => {
                let flicked = finger_now.dx.abs() >= STEP_AT / 2.0 && finger_now.speed.abs() >= FLICK && finger_now.speed.signum() == finger_now.dx.signum();
                slide.set(0.0);
                if finger_now.dx.abs() >= STEP_AT || flicked {
                    if let Some((_, _, go)) = floor_now {
                        go(if finger_now.dx < 0.0 { 1 } else { -1 });
                    }
                }
            }
            Way::Down => {
                let flicked = finger_now.dy >= AWAY_AT / 2.0 && finger_now.speed >= FLICK;
                if finger_now.dy >= AWAY_AT || flicked {
                    away();
                } else {
                    lowered.set(0.0);
                }
            }
            Way::Still | Way::Astray => {}
        }
    };
    let cancel = move |ev: PointerEvent| {
        if finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
            finger.set_value(None);
            dragging.set(false);
            slide.set(0.0);
            if !leaving.get_untracked() {
                lowered.set(0.0);
            }
        }
    };
    view! {
        <div
            class="plan-toast"
            class:out=removed
            class:leaving=move || leaving.get()
            node_ref=root
            role="status"
            data-drag=move || dragging.get().then_some("")
            style=move || (lowered.get() != 0.0).then(|| format!("transform:translateY({:.1}px)", lowered.get()))
            on:pointerenter=move |_| hold()
            on:pointerleave=move |_| free()
            on:pointerdown=down
            on:pointermove=moved
            on:pointerup=up
            on:pointercancel=cancel
            on:focusin=move |_| hold()
            on:focusout=focus_out
        >
            {body}
        </div>
    }
    .into_any()
}

/// Whether the pointer came down within an element `selector` names.
fn target_within(ev: &PointerEvent, selector: &str) -> bool {
    use leptos::wasm_bindgen::JsCast;
    ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()).and_then(|target| target.closest(selector).ok().flatten()).is_some()
}

/// The way a click on the semester moves it: an arrow's own (`data-by`, also a key on the arrow),
/// else -1 on the left half of the semester, 1 on the right.
fn way_of_tap(ev: &MouseEvent) -> Option<i32> {
    use leptos::wasm_bindgen::JsCast;
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    if let Some(by) = target.and_then(|target| target.closest("[data-by]").ok().flatten()).and_then(|arrow| arrow.get_attribute("data-by")) {
        return by.parse().ok();
    }
    #[cfg(feature = "csr")]
    {
        let area = ev.current_target().and_then(|area| area.dyn_into::<leptos::web_sys::Element>().ok())?;
        let rect = area.get_bounding_client_rect();
        Some(if f64::from(ev.client_x()) < rect.left() + rect.width() / 2.0 { -1 } else { 1 })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Whether the focus that leaves an element of the note goes to another of it.
fn focus_stays(ev: &leptos::ev::FocusEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let note = ev.current_target().and_then(|note| note.dyn_into::<leptos::web_sys::Node>().ok());
    let next = ev.related_target().and_then(|next| next.dyn_into::<leptos::web_sys::Node>().ok());
    note.zip(next).is_some_and(|(note, next)| note.contains(Some(&next)))
}

/// What the line under the semester says, most needed last to go: what happened there
/// („eingeplant", „verschoben"; only where there is room, app.css), its Fachsemester where the
/// Studienbeginn is known, and the warning — „liegt zurück" in a semester behind the student, „kein
/// Angebot" where the module's turnus leaves it out.
fn line_of(at: SemesterKey, stepped: bool, what: &Pressed, start: Option<SemesterKey>, t: &'static Texts) -> AnyView {
    let p = &t.planner;
    let past = at < what.current;
    let said = (!past).then_some(if stepped { p.note_moved } else { p.note_planned });
    let fs = start.and_then(|start| fachsemester(at, start)).map(|fs| (p.fs_short)(fs));
    let warning = if past {
        Some(p.note_behind)
    } else if !what.offer.offered(at) {
        Some(p.note_not_offered)
    } else {
        None
    };
    let parts: Vec<(&'static str, String)> = [said.map(|said| ("plan-toast-what", said.to_string())), fs.map(|fs| ("plan-toast-fs", fs)), warning.map(|warning| ("plan-toast-warn", warning.to_string()))].into_iter().flatten().collect();
    let last = parts.len().saturating_sub(1);
    parts
        .into_iter()
        .enumerate()
        .map(|(i, (class, text))| {
            view! {
                <span class=class>{text}</span>
                {(i < last).then(|| view! { <span class="plan-toast-dot" aria-hidden="true">" · "</span> })}
            }
        })
        .collect_view()
        .into_any()
}
