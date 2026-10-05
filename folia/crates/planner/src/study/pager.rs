//! The semesters on a phone (owner, 2026-10-05: „die swipe bewegungen nach links und rechts
//! brauchen Animationen, damit man versteht was passiert. Die knöpfe können das einfach abspielen
//! und beim swipen selbst muss dass mit der bewegung funktionieren"): one semester as a card as
//! wide as the page, and while it turns the semesters before and after it beside it, out of view
//! until the turn brings them in. A finger drawn sideways takes the cards along: the neighbour
//! comes in from the edge of the screen as far as the finger goes. Let go past a fifth of the card,
//! or flicked that way, and the cards glide on until the neighbour stands in the card's place; let
//! go before, and they glide back. ‹ › in the card's head play the same glide. Before the first
//! semester there is nothing: the card follows less and less far, and goes back.
//!
//! The finger is read as a row's swipe reads it (`folia_widgets::swipe`): the first move past
//! `SLOP` decides, sideways the cards are the finger's, up or down the page scrolls; a swipe is no
//! tap, and its moves are the cards' alone. The neighbours are drawn only while the cards move
//! (`Cards::around`) and are inert, so that the card in place is the only one to find and to use.
//! While the cards glide, the pager grows or shrinks to the card that comes, so that what is under
//! it moves along instead of jumping once it is there. While rows are selected the finger selects
//! (`focus.rs`), and the cards stay where they are. Under the cards, dots: where the card stands
//! among the semesters.

use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use leptos::ev::{MouseEvent, PointerEvent, TouchEvent};
use leptos::html::Div;
use leptos::prelude::*;

use folia_design::nav;

use super::focus::{NewSemester, SemesterCard};
use super::{Selection, StudyCtx};

/// How far a finger moves, in px, before it is a swipe or a scroll (a row's long press gives up
/// past the same: `focus.rs`).
pub(super) const SLOP: f64 = 10.0;
/// A flick, in px per ms, whose last move came less than `FLICK_MS` before the finger went (the
/// sheet's and the bottom bar's, `enhance.js`).
pub(super) const FLICK: f64 = 0.45;
pub(super) const FLICK_MS: f64 = 100.0;
/// How far a card goes where nothing comes after it (px): held back from the start.
const EDGE: f64 = 56.0;
/// How long the cards glide (app.css, `.st-pager[data-phase="glide"]`), and a little more.
const GLIDE: Duration = Duration::from_millis(320);
/// How long after a swipe a click on the cards is the swipe's end and not a tap (ms).
pub(super) const NO_TAP: f64 = 400.0;

/// What the cards do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Rest,
    /// The neighbours stand beside the card; the glide of ‹ › begins in the next frame.
    Ready,
    /// The finger has the cards.
    Drag,
    /// The cards glide on, or back.
    Glide,
}

/// The finger on the cards, from where it came down.
#[derive(Clone, Copy, Debug)]
pub(super) struct Finger {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    /// It moved sideways first: the cards are the finger's.
    pub taken: bool,
    /// How far sideways past the slop, when (the event's time, ms), how fast (px per ms).
    pub pull: f64,
    pub at: f64,
    pub speed: f64,
    /// The card's width, measured when the finger took it.
    pub width: f64,
}

impl Finger {
    pub fn new(ev: &PointerEvent) -> Self {
        Finger { id: ev.pointer_id(), x: f64::from(ev.client_x()), y: f64::from(ev.client_y()), taken: false, pull: 0.0, at: ev.time_stamp(), speed: 0.0, width: 0.0 }
    }

    /// Whether the finger has moved past the slop, and if so, whether sideways: `None` while it
    /// has not, else `Some(true)` for sideways.
    pub fn sideways(&self, ev: &PointerEvent) -> Option<bool> {
        let (dx, dy) = (f64::from(ev.client_x()) - self.x, f64::from(ev.client_y()) - self.y);
        (dx.abs() > SLOP || dy.abs() > SLOP).then_some(dx.abs() > dy.abs())
    }

    /// The finger at `x` at the time `at`: how far it has pulled past the slop, and how fast it
    /// goes, mostly of its last moves. Where it stands still nothing changes, so that a finger
    /// lifted where it last moved keeps that move's speed and time (a flick).
    pub fn follow(&mut self, x: f64, at: f64) {
        let pull = beyond_slop(x - self.x);
        if (pull - self.pull).abs() < f64::EPSILON {
            return;
        }
        if at > self.at {
            self.speed = 0.7 * ((pull - self.pull) / (at - self.at)) + 0.3 * self.speed;
        }
        self.pull = pull;
        self.at = at;
    }
}

/// How far sideways the finger has pulled past the slop: the cards do not jump by it.
pub(super) fn beyond_slop(dx: f64) -> f64 {
    if dx > SLOP {
        dx - SLOP
    } else if dx < -SLOP {
        dx + SLOP
    } else {
        0.0
    }
}

/// Past its room a pull goes less and less far, never past `room` px (the bottom bar's `band`).
pub(super) fn band(over: f64, room: f64) -> f64 {
    if room > 0.0 {
        room * (1.0 - 1.0 / (1.0 + over / (3.0 * room)))
    } else {
        0.0
    }
}

/// Where the cards stand for a pull of `pull` px: with the finger, never further than a card is
/// wide; towards a side where nothing comes (`open` false) held back from the start.
fn card_at(pull: f64, width: f64, open: bool) -> f64 {
    let far = if open { pull.abs().min(width.max(0.0)) } else { band(pull.abs(), EDGE) };
    far.copysign(pull)
}

/// Where letting go of cards `pull` px from their places turns, with the finger's last `speed`
/// and whether its last move was `recent`: to the next (1) or the one before (−1) — pulled past a
/// fifth of the card, or flicked that way a little — or nowhere; never where nothing comes
/// (`open`: before, after).
pub(super) fn decide(pull: f64, speed: f64, recent: bool, width: f64, open: (bool, bool)) -> Option<i32> {
    let by = if pull < 0.0 { 1 } else { -1 };
    let there = if by > 0 { open.1 } else { open.0 };
    let flicked = recent && speed.abs() > FLICK && speed.signum() == pull.signum() && pull.abs() >= 24.0;
    (there && pull != 0.0 && (pull.abs() >= width / 5.0 || flicked)).then_some(by)
}

/// The cards: the semester in focus, its neighbours while they turn, and the finger on them.
/// `focus.rs` turns them by ‹ › through `turn_to`, found as context.
#[derive(Clone, Copy)]
pub(super) struct Cards {
    ctx: StudyCtx,
    /// The semesters in order, then the page that adds one.
    order: Memo<Vec<SemesterKey>>,
    focused: Memo<Option<SemesterKey>>,
    /// The neighbours are drawn.
    around: RwSignal<bool>,
    phase: RwSignal<Phase>,
    /// Where the cards stand: px from their places (the finger's), and slots on (−1: the next
    /// comes into the card's place).
    pull: RwSignal<f64>,
    turn: RwSignal<i32>,
    /// How tall the card is while the cards move: the one in place, then the one that comes.
    height: RwSignal<Option<f64>>,
    finger: StoredValue<Option<Finger>>,
    /// Until when a click on the cards is the end of a swipe (ms, the events' clock).
    swiped: StoredValue<f64>,
    node: NodeRef<Div>,
}

impl Cards {
    fn new(ctx: StudyCtx) -> Self {
        let order = Memo::new(move |_| {
            ctx.with_ready(|ready| {
                let mut keys: Vec<SemesterKey> = ready.study.semesters.iter().map(|semester| semester.key).collect();
                if let Some(end) = keys.last().and_then(|last| last.plus(1)) {
                    keys.push(end);
                }
                keys
            })
            .unwrap_or_default()
        });
        Cards {
            ctx,
            order,
            focused: Memo::new(move |_| ctx.focused()),
            around: RwSignal::new(false),
            phase: RwSignal::new(Phase::Rest),
            pull: RwSignal::new(0.0),
            turn: RwSignal::new(0),
            height: RwSignal::new(None),
            finger: StoredValue::new(None),
            swiped: StoredValue::new(f64::NEG_INFINITY),
            node: NodeRef::new(),
        }
    }

    /// The semester `by` places from the one in focus, if there is one (the page that adds one
    /// counts).
    fn neighbour(self, by: i32) -> Option<SemesterKey> {
        let focused = self.focused.get_untracked()?;
        self.order.with_untracked(|order| {
            let at = i32::try_from(order.iter().position(|key| *key == focused)?).ok()?;
            order.get(usize::try_from(at + by).ok()?).copied()
        })
    }

    /// Turns to `to` (‹ ›): a neighbour by the glide, any other at once.
    pub fn turn_to(self, to: SemesterKey) {
        let by = [-1, 1].into_iter().find(|by| self.neighbour(*by) == Some(to));
        let Some(by) = by else {
            self.ctx.focus.set(Some(to));
            return;
        };
        if self.phase.get_untracked() != Phase::Rest {
            return;
        }
        self.hold_height();
        self.around.set(true);
        self.phase.set(Phase::Ready);
        // The neighbour stands beside the card first, so that the glide has somewhere to start.
        nav::after_paint(move || self.glide_to(by));
    }

    /// How tall the slot `place` is (0: the card in place), if it is drawn.
    fn slot_height(self, place: i32) -> Option<f64> {
        let node = self.node.get_untracked()?;
        let slot = node.query_selector(&format!(".st-slot[data-slot=\"{place}\"]")).ok()??;
        Some(f64::from(leptos::wasm_bindgen::JsCast::dyn_into::<leptos::web_sys::HtmlElement>(slot).ok()?.offset_height()))
    }

    /// The pager keeps the card's height while the cards move.
    fn hold_height(self) {
        self.height.set(self.slot_height(0));
    }

    fn down(self, ev: PointerEvent) {
        self.finger.set_value(None);
        // A new touch: its click is a tap.
        self.swiped.set_value(f64::NEG_INFINITY);
        let selecting = self.ctx.selection.with_untracked(|selection| !selection.keys.is_empty());
        if !ev.is_primary() || ev.button() != 0 || selecting || self.phase.get_untracked() != Phase::Rest || self.ctx.menu.with_untracked(Option::is_some) {
            return;
        }
        self.finger.set_value(Some(Finger::new(&ev)));
    }

    fn moving(self, ev: PointerEvent) {
        let Some(mut finger) = self.finger.get_value() else { return };
        if ev.pointer_id() != finger.id {
            return;
        }
        // Let go where the cards did not hear it (a mouse's button, outside the window).
        if ev.buttons() & 1 == 0 {
            self.let_go(None);
            return;
        }
        if !finger.taken {
            match finger.sideways(&ev) {
                None => return,
                // Up or down: the page scrolls.
                Some(false) => {
                    self.finger.set_value(None);
                    return;
                }
                Some(true) => {
                    let Some(node) = self.node.get_untracked() else { return };
                    // A mouse moved off the cards still moves them.
                    let _ = node.set_pointer_capture(finger.id);
                    finger.taken = true;
                    finger.width = card_width(&node);
                    self.hold_height();
                    self.around.set(true);
                    self.phase.set(Phase::Drag);
                }
            }
        }
        finger.follow(f64::from(ev.client_x()), ev.time_stamp());
        self.put(&finger);
        self.finger.set_value(Some(finger));
    }

    /// The cards where the finger has them.
    fn put(self, finger: &Finger) {
        let open = if finger.pull < 0.0 { self.neighbour(1).is_some() } else { self.neighbour(-1).is_some() };
        self.pull.set(card_at(finger.pull, finger.width, open));
    }

    /// The finger goes: lifted at `x` at the time `at`, or taken by the browser (`None`).
    fn let_go(self, lifted: Option<(f64, f64)>) {
        let Some(mut finger) = self.finger.get_value() else { return };
        self.finger.set_value(None);
        // A tap: the card's own.
        if !finger.taken {
            return;
        }
        if let Some(node) = self.node.get_untracked() {
            let _ = node.release_pointer_capture(finger.id);
        }
        let mut by = None;
        if let Some((x, at)) = lifted {
            self.swiped.set_value(at + NO_TAP);
            finger.follow(x, at);
            self.put(&finger);
            let open = (self.neighbour(-1).is_some(), self.neighbour(1).is_some());
            by = decide(self.pull.get_untracked(), finger.speed, at - finger.at < FLICK_MS, finger.width, open);
        }
        match by {
            Some(by) => self.glide_to(by),
            None => self.glide_back(),
        }
    }

    /// The cards glide one place on (`by` 1: the next comes in), then the neighbour is the
    /// semester in focus, standing where it stands, and the others go.
    fn glide_to(self, by: i32) {
        let Some(to) = self.neighbour(by) else { return self.glide_back() };
        if let Some(height) = self.slot_height(by) {
            self.height.try_set(Some(height));
        }
        self.phase.try_set(Phase::Glide);
        self.pull.try_set(0.0);
        self.turn.try_set(-by);
        set_timeout(
            move || {
                self.ctx.focus.try_set(Some(to));
                self.rest();
            },
            GLIDE,
        );
    }

    fn glide_back(self) {
        self.phase.try_set(Phase::Glide);
        self.pull.try_set(0.0);
        self.turn.try_set(0);
        set_timeout(move || self.rest(), GLIDE);
    }

    fn rest(self) {
        self.turn.try_set(0);
        self.pull.try_set(0.0);
        self.height.try_set(None);
        self.phase.try_set(Phase::Rest);
        self.around.try_set(false);
    }

    /// Whatever the browser makes of a finger that swiped, it is no tap (heard before the cards'
    /// own clicks, in the capture phase).
    fn click(self, ev: MouseEvent) {
        if ev.time_stamp() < self.swiped.get_value() {
            ev.prevent_default();
            ev.stop_propagation();
        }
    }

    /// The moves of a swipe are the cards' alone: left to the browser, a quick one ends in a
    /// fling of nothing that eats the next tap (`folia_widgets::swipe`).
    fn touch_move(self, ev: TouchEvent) {
        if self.phase.get_untracked() == Phase::Drag && ev.cancelable() {
            ev.prevent_default();
        }
    }

    /// `data-phase`: while the cards move.
    fn phase_attr(self) -> Option<&'static str> {
        match self.phase.get() {
            Phase::Rest => None,
            Phase::Ready => Some("ready"),
            Phase::Drag => Some("drag"),
            Phase::Glide => Some("glide"),
        }
    }

    /// Where the cards stand while they move (`--pull`, `--turn`), and how tall the card is (with
    /// the pager's padding, app.css).
    fn style(self) -> Option<String> {
        (self.phase.get() != Phase::Rest).then(|| {
            let height = self.height.get().map(|height| format!(";height:{:.1}px", height + 24.0)).unwrap_or_default();
            format!("--pull:{:.1}px;--turn:{}{height}", self.pull.get(), self.turn.get())
        })
    }

    /// The slots drawn: the semester in focus at 0, its neighbours at −1 and 1 while the cards
    /// move; each with whether it is the page that adds a semester.
    fn slots(self) -> Vec<(SemesterKey, bool, i32)> {
        let Some(focused) = self.focused.get() else { return Vec::new() };
        let around = self.around.get();
        self.order.with(|order| {
            let Some(at) = order.iter().position(|key| *key == focused) else { return Vec::new() };
            let last = order.len().saturating_sub(1);
            let from = if around { at.saturating_sub(1) } else { at };
            let to = if around { (at + 1).min(last) } else { at };
            (from..=to)
                .filter_map(|i| {
                    let key = *order.get(i)?;
                    let place = i32::try_from(i).ok()? - i32::try_from(at).ok()?;
                    Some((key, i == last, place))
                })
                .collect()
        })
    }
}

/// How wide a card is: the slot in place.
fn card_width(node: &leptos::web_sys::HtmlDivElement) -> f64 {
    node.query_selector(".st-slot[data-slot=\"0\"]")
        .ok()
        .flatten()
        .and_then(|slot| leptos::wasm_bindgen::JsCast::dyn_into::<leptos::web_sys::HtmlElement>(slot).ok())
        .map_or_else(|| f64::from(node.offset_width()), |slot| f64::from(slot.offset_width()))
}

#[component]
pub(super) fn Pager(ctx: StudyCtx) -> impl IntoView {
    let cards = Cards::new(ctx);
    provide_context(cards);
    // What is selected belongs to the semester in focus: turning to another lets go of it.
    Effect::new(move |_| {
        let now = cards.focused.get();
        if ctx.selection.with_untracked(|selection| selection.semester.is_some() && selection.semester != now) {
            ctx.selection.set(Selection::default());
        }
    });
    let slots = Memo::new(move |_| cards.slots());
    view! {
        <div
            class="st-pager"
            node_ref=cards.node
            data-phase=move || cards.phase_attr()
            style=move || cards.style()
            on:pointerdown=move |ev| cards.down(ev)
            on:pointermove=move |ev| cards.moving(ev)
            on:pointerup=move |ev: PointerEvent| {
                if cards.finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
                    cards.let_go(Some((f64::from(ev.client_x()), ev.time_stamp())));
                }
            }
            on:pointercancel=move |ev: PointerEvent| {
                if cards.finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
                    cards.let_go(None);
                }
            }
            on:touchmove=move |ev| cards.touch_move(ev)
            on:click:capture=move |ev| cards.click(ev)
        >
            <For
                each=move || slots.get()
                key=|(key, end, _)| (*key, *end)
                children=move |(key, end, _)| {
                    let place = Memo::new(move |_| slots.with(|slots| slots.iter().find(|(other, last, _)| *other == key && *last == end).map_or(0, |(.., place)| *place)));
                    view! {
                        <div
                            class="st-slot"
                            data-slot=move || place.get().to_string()
                            style=move || format!("--slot:{}", place.get())
                            inert=move || place.get() != 0
                            aria-hidden=move || (place.get() != 0).then_some("true")
                        >
                            {if end { view! { <NewSemester ctx/> }.into_any() } else { view! { <SemesterCard ctx semester=key/> }.into_any() }}
                        </div>
                    }
                }
            />
        </div>
        <Dots ctx/>
    }
}

/// Where the card stands among the semesters: a dot each, the current one ringed.
#[component]
fn Dots(ctx: StudyCtx) -> impl IntoView {
    let dots = Memo::new(move |_| {
        let focused = ctx.focused();
        ctx.with_ready(|ready| {
            let mut dots: Vec<(bool, bool)> = ready.study.semesters.iter().map(|semester| (Some(semester.key) == focused, semester.when == When::Now)).collect();
            let end = ready.study.semesters.last().and_then(|last| last.key.plus(1));
            dots.push((end.is_some() && end == focused, false));
            dots
        })
        .unwrap_or_default()
    });
    view! {
        <div class="st-dots" aria-hidden="true">
            {move || dots.get().into_iter().map(|(here, now)| view! { <span class:here=here class:now=now></span> }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cards follow the finger past the slop, never further than a card is wide; where nothing
    /// comes they are held back from the start.
    #[test]
    fn the_cards_follow_the_finger() {
        assert_eq!(beyond_slop(6.0), 0.0);
        assert_eq!(beyond_slop(-25.0), -15.0);
        assert_eq!(card_at(-84.0, 366.0, true), -84.0);
        assert_eq!(card_at(-500.0, 366.0, true), -366.0);
        let held = card_at(84.0, 366.0, false);
        assert!(held > 0.0 && held < EDGE, "{held}");
        assert!(card_at(2000.0, 366.0, false) < EDGE);
    }

    /// A finger lifted where it last moved keeps that move's speed and time: a flick stays one.
    #[test]
    fn a_finger_lifted_where_it_moved_keeps_its_speed() {
        let mut finger = Finger { id: 1, x: 300.0, y: 100.0, taken: true, pull: 0.0, at: 0.0, speed: 0.0, width: 366.0 };
        finger.follow(250.0, 16.0);
        finger.follow(200.0, 32.0);
        let (speed, at) = (finger.speed, finger.at);
        assert!(speed < -FLICK, "{speed}");
        finger.follow(200.0, 90.0);
        assert_eq!((finger.speed, finger.at, finger.pull), (speed, at, -90.0));
    }

    /// Let go past a fifth of the card, or flicked, the cards turn that way; never where nothing
    /// comes, never for a pull too short or a flick back.
    #[test]
    fn letting_go_turns_where_the_finger_went() {
        let both = (true, true);
        assert_eq!(decide(-80.0, 0.0, false, 366.0, both), Some(1), "to the left: the next");
        assert_eq!(decide(80.0, 0.0, false, 366.0, both), Some(-1), "to the right: the one before");
        assert_eq!(decide(-60.0, -0.1, true, 366.0, both), None, "not far enough, slow");
        assert_eq!(decide(-30.0, -0.9, true, 366.0, both), Some(1), "flicked");
        assert_eq!(decide(-30.0, 0.9, true, 366.0, both), None, "flicked back");
        assert_eq!(decide(-30.0, -0.9, false, 366.0, both), None, "flicked long ago");
        assert_eq!(decide(-12.0, -0.9, true, 366.0, both), None, "too short a flick");
        assert_eq!(decide(80.0, 0.0, false, 366.0, (false, true)), None, "nothing before the first");
        assert_eq!(decide(0.0, 0.0, false, 366.0, both), None);
    }
}
