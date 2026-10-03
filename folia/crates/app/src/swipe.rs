//! A module's row swiped sideways on a phone (owner, 2026-09-30: „Nach links wischen merken nach
//! rechts wischen planen. Mach das so, dass dann darunter freigelegt wird was die Aktion macht
//! (also Icon und Text)"): to the left „Merken", to the right „Einplanen" — the two switches of
//! the module's page (`bookmarks::MarkButton`, `studyplan::PlanButton`), pressed from the list
//! without opening the module. The catalog's list has it, and the Merkliste (owner, the same day:
//! „Mach das auch in der Merkliste"), in the browser app and in the phone layout: marking and
//! planning belong to the app (R9, R15), and a wide screen has the mark at the end of the row and
//! the preview beside the list. On the Merkliste a module swiped off it stays, dimmed, as one whose
//! mark is taken away there by its button, and a swipe to the left marks it again.
//!
//! The card follows the finger and uncovers what lies under it, at the side it leaves: the
//! action's icon and word, and a line of what it is done to — the semester „Einplanen" plans into,
//! aimed as the switch aims (`studyplan::target_semester`, from the module's own Termine as on its
//! page), and the placeholder the finder asked for. What is said is what the swipe does, not what
//! the module is: a marked module's row uncovers „Entfernen · von der Merkliste", a planned one's
//! „Entfernen · aus WiSe 2026/27". It is read once, when the finger starts. Past a third of the
//! card (at most `ARM`), or flicked that way, the action is armed: the ground takes the colour of
//! its side (the inverted look of a marked module, the accent for the plan) and the icon springs.
//! Let go armed, and the ground says what was done („Gemerkt", „Eingeplant", „Entfernt"), the card
//! goes aside as far as that takes and holds a moment before it glides back, and the action
//! follows after the next frame (R21); let go before, and the card glides back and nothing
//! happens. The row stays where it is either way: marking and
//! planning change what the row's buttons say, not the list (R5), unless the list is filtered by
//! them („Gemerkt", „Passt in meinen Stundenplan").
//!
//! The finger is read as along the bottom bar (`enhance.js`): the first move past `SLOP` decides —
//! sideways the card is the finger's, up or down the page scrolls (`touch-action: pan-y`), and a
//! swipe is no tap. Its moves are the row's alone (a quick one left to Chromium ends in a fling of
//! nothing, and the next tap only stops that fling); a flick is measured by the events' own times.
//! A mouse (a narrow window) drags the card the same way. The card moves inside its own place
//! (app.css: the row clips it, and draws the card's ring and shadow while it does), so it never
//! reaches past the page's edge, which would let the page scroll sideways.

use std::time::Duration;

use folia_model::labels::TurnusSeason;
use folia_pages::ask::ModuleSemestersAsk;
use leptos::ev::{DragEvent, MouseEvent, PointerEvent, TouchEvent};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

use crate::bookmarks::Bookmarks;
use crate::data::{use_data, DataClient};
use crate::i18n::{self, Texts};
use crate::nav;
use crate::pages::module::semesters_of;
use crate::studyplan::{self, Aim, PlanHint, Studyplan};
use crate::ui::Icon;

/// How far a finger moves, in px, before it is a swipe or a scroll: a little further than along the
/// bottom bar (8), a row being something to tap, and a tap may wobble.
const SLOP: f64 = 10.0;
/// How far the card goes before letting go does what its side says: a third of the card, at most
/// this far (px); a flick that way half as far.
const ARM: f64 = 120.0;
/// A flick, in px per ms, as the sheet and the bottom bar measure it (`enhance.js`), whose last move
/// came less than `FLICK_MS` before the finger went.
const FLICK: f64 = 0.45;
const FLICK_MS: f64 = 100.0;
/// How far the card goes towards a side that has nothing to do (px): held back from the start.
const CLOSED: f64 = 16.0;
/// Done, the card goes aside as far as the ground needs to say all of it: its words and this
/// margin (px), leaving at least `KEEP` px of the card in view; and it holds there for `HOLD`.
const ROOM: f64 = 32.0;
const KEEP: f64 = 56.0;
const HOLD: Duration = Duration::from_millis(300);
/// How long the card glides back (app.css, `.row-wrap[data-swipe="glide"]`), and a little more
/// before the row is at rest again.
const GLIDE: Duration = Duration::from_millis(380);
/// How long after a swipe a click on its row is the swipe's end and not a tap (ms).
const NO_TAP: f64 = 400.0;

/// A side of the card: what it uncovers going to the left (the mark, at the right edge) and to the
/// right (the plan, at the left edge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Mark,
    Plan,
}

impl Side {
    /// The side a card `offset` px from its place uncovers.
    fn of(offset: f64) -> Option<Self> {
        if offset < 0.0 {
            Some(Side::Mark)
        } else if offset > 0.0 {
            Some(Side::Plan)
        } else {
            None
        }
    }

    fn name(self) -> &'static str {
        match self {
            Side::Mark => "mark",
            Side::Plan => "plan",
        }
    }
}

/// What a side says: the action while the finger is down, and what it did once it is done.
#[derive(Clone, Debug, PartialEq)]
struct Act {
    icon: &'static str,
    word: &'static str,
    line: Option<String>,
    done_icon: &'static str,
    done: &'static str,
    /// The icon is filled once it is done: the bookmark of a module that is marked now.
    fills: bool,
}

impl Act {
    /// „Merken" for a module that is not marked, else „Entfernen" from the Merkliste.
    fn mark(marked: bool, t: &'static Texts) -> Self {
        if marked {
            Act { icon: "bookmark-minus", word: t.marks.remove, line: Some(t.marks.from_list.to_string()), done_icon: "bookmark-minus", done: t.marks.removed, fills: false }
        } else {
            Act { icon: "bookmark", word: t.marks.save, line: None, done_icon: "bookmark", done: t.marks.saved, fills: true }
        }
    }

    /// „Einplanen" into the semester the switch aims at, else „Entfernen" out of it.
    fn plan(aim: &Aim, t: &'static Texts) -> Self {
        let (word, line, done) = studyplan::swipe_words(aim, t);
        if aim.pressed() {
            Act { icon: "calendar-minus", word, line: Some(line), done_icon: "calendar-minus", done, fills: false }
        } else {
            Act { icon: "calendar-plus", word, line: Some(line), done_icon: "calendar-check-2", done, fills: false }
        }
    }
}

/// What lies under the card while the row is swiped, read when the finger starts: what each side
/// says (none where it has nothing to do), and what their actions start from.
#[derive(Clone)]
struct Ground {
    mark: Option<Act>,
    plan: Option<Act>,
    /// The module was marked, and „Einplanen" aimed so, when the finger started.
    marked: bool,
    aim: Option<Aim>,
}

/// The finger on the row, from where it came down.
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: i32,
    x: f64,
    y: f64,
    /// It moved sideways first: the card is the finger's.
    taken: bool,
    /// How far along the row, past the slop, when (the event's time, ms), and how fast (px per ms).
    pull: f64,
    at: f64,
    speed: f64,
    /// The card's width, measured when the finger took it.
    width: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Rest,
    /// The finger has the card.
    Drag,
    /// The card goes back to its place.
    Glide,
}

/// Where the card stands: px from its place, and how far towards arming (0–1, what the ground's
/// words fade in with). A card gliding back keeps the second, so its words stay while it goes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Card {
    offset: f64,
    open: f64,
}

/// A row that can be swiped: what the row's element needs (`Row` in `pages::catalog`: its
/// reference, its handlers, its attributes and the ground under the card) and the state behind it.
/// Every value is the row's own and reactive only where the row shows it (R5).
#[derive(Clone, Copy)]
pub struct RowSwipe {
    t: &'static Texts,
    id: StoredValue<String>,
    turnus: Option<TurnusSeason>,
    hint: Option<Memo<Option<PlanHint>>>,
    phone: RwSignal<bool>,
    wrap: NodeRef<leptos::html::Div>,
    source: StoredValue<Option<DataClient>>,
    bookmarks: Option<Bookmarks>,
    plan: Option<Studyplan>,
    finger: StoredValue<Option<Finger>>,
    /// Until when a click on the row is the end of a swipe (ms, the events' clock).
    swiped: StoredValue<f64>,
    phase: RwSignal<Phase>,
    card: RwSignal<Card>,
    side: RwSignal<Option<Side>>,
    armed: RwSignal<bool>,
    done: RwSignal<bool>,
    ground: RwSignal<Option<Ground>>,
}

impl RowSwipe {
    /// For the row of module `id`: its turnus and the hint of the list („Passt in meinen
    /// Stundenplan") aim „Einplanen" as on the module's page; `phone` says whether a swipe is one.
    pub fn new(id: String, turnus: Option<TurnusSeason>, hint: Option<Memo<Option<PlanHint>>>, phone: RwSignal<bool>) -> Self {
        Self {
            t: i18n::t(),
            id: StoredValue::new(id),
            turnus,
            hint,
            phone,
            wrap: NodeRef::new(),
            source: StoredValue::new(use_data().ok()),
            bookmarks: Bookmarks::expect(),
            plan: Studyplan::expect(),
            finger: StoredValue::new(None),
            swiped: StoredValue::new(f64::NEG_INFINITY),
            phase: RwSignal::new(Phase::Rest),
            card: RwSignal::new(Card::default()),
            side: RwSignal::new(None),
            armed: RwSignal::new(false),
            done: RwSignal::new(false),
            ground: RwSignal::new(None),
        }
    }

    /// The row's element (`node_ref`).
    pub fn wrap(self) -> NodeRef<leptos::html::Div> {
        self.wrap
    }

    /// A finger (or the mouse's button) comes down on the row: watched until it moves.
    pub fn down(self, ev: PointerEvent) {
        self.finger.set_value(None);
        // A new touch: its click is a tap.
        self.swiped.set_value(f64::NEG_INFINITY);
        // The card still on its way back takes taps only.
        if !self.phone.get_untracked() || !ev.is_primary() || ev.button() != 0 || self.phase.get_untracked() != Phase::Rest {
            return;
        }
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        self.finger.set_value(Some(Finger { id: ev.pointer_id(), x, y, taken: false, pull: 0.0, at: ev.time_stamp(), speed: 0.0, width: 0.0 }));
    }

    pub fn moving(self, ev: PointerEvent) {
        let Some(mut finger) = self.finger.get_value() else { return };
        if ev.pointer_id() != finger.id {
            return;
        }
        // Let go where the row did not hear it (a mouse's button, outside the window).
        if ev.buttons() & 1 == 0 {
            self.let_go(None);
            return;
        }
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        if !finger.taken {
            let (dx, dy) = (x - finger.x, y - finger.y);
            if dx.abs() < SLOP && dy.abs() < SLOP {
                return;
            }
            // The first move past the slop decides: along the row the finger has the card, else
            // the page scrolls.
            match (dx.abs() > dy.abs()).then(|| self.take(finger.id)).flatten() {
                Some(width) => {
                    finger.taken = true;
                    finger.width = width;
                }
                None => {
                    self.finger.set_value(None);
                    return;
                }
            }
        }
        self.follow(&mut finger, x, ev.time_stamp());
        self.finger.set_value(Some(finger));
    }

    pub fn up(self, ev: PointerEvent) {
        if self.finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
            self.let_go(Some((f64::from(ev.client_x()), ev.time_stamp())));
        }
    }

    /// The browser took the finger (a scroll, a pinch, a gesture of the system): nothing is done.
    pub fn cancel(self, ev: PointerEvent) {
        if self.finger.get_value().is_some_and(|finger| finger.id == ev.pointer_id()) {
            self.let_go(None);
        }
    }

    /// The moves of a swipe are the row's alone: left to the browser, a quick one ends in a fling
    /// of nothing, and the next tap anywhere only stops that fling instead of being a tap. The
    /// listener is the row's own (the app is built without Leptos's delegation of events), so the
    /// browser lets it cancel a move, as it would not a listener of the window.
    pub fn touch_move(self, ev: TouchEvent) {
        if self.phase.get_untracked() == Phase::Drag && ev.cancelable() {
            ev.prevent_default();
        }
    }

    /// Whatever the browser makes of a finger that swiped, it is no tap: not the row's link, not
    /// its mark (so this is heard before either, in the capture phase).
    pub fn click(self, ev: MouseEvent) {
        if ev.time_stamp() < self.swiped.get_value() {
            ev.prevent_default();
            ev.stop_propagation();
        }
    }

    /// A link dragged with a mouse would leave the page's hands (the browser's own drag cancels
    /// the pointer): in the phone layout, the row is not dragged.
    pub fn drag_start(self, ev: DragEvent) {
        if self.phone.get_untracked() {
            ev.prevent_default();
        }
    }

    /// `data-swipe`: while the finger has the card („drag") and while it goes back („glide").
    pub fn phase_attr(self) -> Option<&'static str> {
        match self.phase.get() {
            Phase::Rest => None,
            Phase::Drag => Some("drag"),
            Phase::Glide => Some("glide"),
        }
    }

    /// `data-side`: the side the card uncovers.
    pub fn side_attr(self) -> Option<&'static str> {
        self.side.get().map(Side::name)
    }

    /// `data-armed`: letting go does what the side says.
    pub fn armed_attr(self) -> Option<&'static str> {
        self.armed.get().then_some("")
    }

    /// `data-done`: it has been done, and the ground says so.
    pub fn done_attr(self) -> Option<&'static str> {
        self.done.get().then_some("")
    }

    /// Where the card stands (`--swipe`) and how far its words have come in (`--open`).
    pub fn style(self) -> Option<String> {
        (self.phase.get() != Phase::Rest).then(|| self.card.with(|card| format!("--swipe:{:.1}px;--open:{:.3}", card.offset, card.open)))
    }

    /// The ground under the card while the row is swiped: each side's icon, its word and its line.
    /// Nobody reads it but the eye: the switches it stands for are the module's.
    pub fn ground_view(self) -> impl IntoView {
        let done = self.done;
        let side = move |act: Option<Act>, side: Side| {
            act.map(|act| {
                let Act { icon, word, line, done_icon, done: said, fills } = act;
                let icon = move || if done.get() { view! { <Icon name=done_icon/> }.into_any() } else { view! { <Icon name=icon/> }.into_any() };
                view! {
                    <span class="swipe-act" data-act=side.name() class:fills=fills>
                        {icon}
                        <span class="swipe-words">
                            <b>{move || if done.get() { said } else { word }}</b>
                            {line.map(|line| view! { <small>{line}</small> })}
                        </span>
                    </span>
                }
            })
        };
        move || {
            self.ground.get().map(|ground| {
                view! {
                    <div class="swipe-ground" aria-hidden="true">
                        {side(ground.plan, Side::Plan)}
                        {side(ground.mark, Side::Mark)}
                    </div>
                }
            })
        }
    }

    /// The finger takes the card: what its sides do is read now, once. `None` where the row has
    /// nothing to offer (no marks, no plan in this browser), which leaves the finger to the page.
    fn take(self, pointer: i32) -> Option<f64> {
        let wrap = self.wrap.get_untracked()?;
        let ground = self.read()?;
        let width = f64::from(wrap.offset_width());
        // A mouse moved off the row still moves the card.
        let _ = wrap.set_pointer_capture(pointer);
        self.ground.set(Some(ground));
        self.done.set(false);
        self.phase.set(Phase::Drag);
        Some(width)
    }

    /// What the two sides of the row say and start from.
    fn read(self) -> Option<Ground> {
        let (t, id) = (self.t, self.id.get_value());
        let marked = self.bookmarks.map(|bookmarks| bookmarks.is_marked_untracked(&id));
        let aim = self.plan.and_then(|plan| {
            // The semesters the switch aims with, asked of the local catalog as the module's page
            // asks them; without a current semester (a snapshot always names one) its newest.
            let source = self.source.get_value()?;
            let (current, newest) = source.now(&ModuleSemestersAsk { id: id.clone() }).map(|(semesters, schedule)| semesters_of(&semesters, &schedule)).ok()?;
            let hint = self.hint.and_then(|hint| hint.get_untracked());
            Some(studyplan::aim_now(plan, &id, current.or(newest)?, newest, self.turnus, hint.as_ref()))
        });
        (marked.is_some() || aim.is_some()).then(|| Ground {
            mark: marked.map(|marked| Act::mark(marked, t)),
            plan: aim.as_ref().map(|aim| Act::plan(aim, t)),
            marked: marked.unwrap_or(false),
            aim,
        })
    }

    /// The finger at `x` at the time `at` (the event's): the card follows, and how fast the finger
    /// goes is kept, mostly of the last moves, as the sheet measures a flick.
    fn follow(self, finger: &mut Finger, x: f64, at: f64) {
        let pull = beyond_slop(x - finger.x);
        if (pull - finger.pull).abs() < f64::EPSILON {
            return;
        }
        if at > finger.at {
            finger.speed = 0.7 * ((pull - finger.pull) / (at - finger.at)) + 0.3 * finger.speed;
        }
        finger.pull = pull;
        finger.at = at;
        let open = self.ground.with_untracked(|ground| ground.as_ref().map(|ground| (ground.mark.is_some(), ground.plan.is_some()))).unwrap_or_default();
        let offset = card_at(pull, finger.width, open);
        let arm = arm_at(finger.width);
        self.card.set(Card { offset, open: (offset.abs() / arm).min(1.0) });
        put(self.side, Side::of(offset));
        put(self.armed, offset.abs() >= arm);
    }

    /// The finger goes: lifted at `x` at the time `at`, or taken by the browser (`None`).
    fn let_go(self, lifted: Option<(f64, f64)>) {
        let Some(mut finger) = self.finger.get_value() else { return };
        self.finger.set_value(None);
        // A tap: the link's own.
        if !finger.taken {
            return;
        }
        if let Some(wrap) = self.wrap.get_untracked() {
            let _ = wrap.release_pointer_capture(finger.id);
        }
        let mut act = None;
        if let Some((x, at)) = lifted {
            self.swiped.set_value(at + NO_TAP);
            self.follow(&mut finger, x, at);
            let offset = self.card.with_untracked(|card| card.offset);
            act = decide(offset, finger.speed, at - finger.at < FLICK_MS, finger.width);
        }
        match act {
            Some(side) => self.act(side, finger.width),
            None => self.glide(),
        }
    }

    /// Let go armed: the ground says what was done, and it is done after the next frame (R21: the
    /// row has answered by then). What the ground said is what happens, also should another tab
    /// have changed the module meanwhile. The card makes room for all the ground says, holds a
    /// moment, and glides back.
    fn act(self, side: Side, width: f64) {
        let Some(ground) = self.ground.get_untracked() else { return self.glide() };
        put(self.side, Some(side));
        put(self.armed, true);
        self.done.set(true);
        self.phase.set(Phase::Glide);
        // Measured once the ground says it: the words of what was done differ from the action's.
        nav::after_paint(move || {
            if let Some(room) = self.room(side, width) {
                self.card.try_update(|card| {
                    if card.offset.abs() < room {
                        card.offset = room.copysign(card.offset);
                    }
                });
            }
            set_timeout(move || self.glide(), HOLD);
        });
        let id = self.id.get_value();
        match side {
            Side::Mark => {
                let (bookmarks, was) = (self.bookmarks, ground.marked);
                nav::after_paint(move || {
                    if let Some(bookmarks) = bookmarks.filter(|bookmarks| bookmarks.is_marked_untracked(&id) == was) {
                        bookmarks.toggle(&id);
                    }
                });
            }
            Side::Plan => {
                if let (Some(plan), Some(aim)) = (self.plan, ground.aim) {
                    let was = aim.pressed();
                    studyplan::press(plan, self.source.get_value(), id, aim, was, || {});
                }
            }
        }
    }

    /// How far aside the card goes for the ground to say all of what `side` did: the side's words
    /// with their margins, never so far that less than `KEEP` px of the card stays in view.
    fn room(self, side: Side, width: f64) -> Option<f64> {
        let wrap = self.wrap.try_get_untracked().flatten()?;
        let act = wrap.query_selector(&format!(".swipe-act[data-act=\"{}\"]", side.name())).ok().flatten()?;
        let words = f64::from(act.dyn_into::<leptos::web_sys::HtmlElement>().ok()?.offset_width());
        Some((words + ROOM).min(width - KEEP))
    }

    /// The card goes back to its place, and the row is at rest once it is there. Its words stay
    /// while it goes (`Card::open`). Also after the row has gone (the list changed meanwhile),
    /// when there is nothing left to set.
    fn glide(self) {
        self.phase.try_set(Phase::Glide);
        self.card.try_update(|card| card.offset = 0.0);
        set_timeout(
            move || {
                self.phase.try_set(Phase::Rest);
                self.side.try_set(None);
                self.armed.try_set(false);
                self.done.try_set(false);
                self.ground.try_set(None);
                self.card.try_set(Card::default());
            },
            GLIDE,
        );
    }
}

/// Sets `signal` only when the value changes: what reads it hears of it only then.
fn put<T: PartialEq + Send + Sync + 'static>(signal: RwSignal<T>, value: T) {
    if signal.with_untracked(|now| *now != value) {
        signal.set(value);
    }
}

/// How far along the row the finger has pulled past the slop: the card does not jump by it.
fn beyond_slop(dx: f64) -> f64 {
    if dx > SLOP {
        dx - SLOP
    } else if dx < -SLOP {
        dx + SLOP
    } else {
        0.0
    }
}

/// Where the card stands, in px from its place, for a pull of `pull` px: with the finger, never
/// further than its own width; towards a side with nothing to do (`open`: the mark's side, the
/// plan's) held back from the start, never further than `CLOSED`.
fn card_at(pull: f64, width: f64, open: (bool, bool)) -> f64 {
    let open = if pull < 0.0 { open.0 } else { open.1 };
    let far = if open { pull.abs().min(width.max(0.0)) } else { band(pull.abs(), CLOSED) };
    far.copysign(pull)
}

/// Past its room a pull goes less and less far, never past `room` px (the bottom bar's `band`).
fn band(over: f64, room: f64) -> f64 {
    if room > 0.0 {
        room * (1.0 - 1.0 / (1.0 + over / (3.0 * room)))
    } else {
        0.0
    }
}

/// How far the card goes before letting go does what its side says: a third of it, at most `ARM`.
fn arm_at(width: f64) -> f64 {
    (width / 3.0).min(ARM)
}

/// What letting go of a card `offset` px from its place does, with the finger's last `speed` and
/// whether its last move was `recent`: the side whose action is armed — pulled that far, or
/// flicked that way half as far — or nothing.
fn decide(offset: f64, speed: f64, recent: bool, width: f64) -> Option<Side> {
    let arm = arm_at(width);
    let flicked = recent && speed.abs() > FLICK && speed.signum() == offset.signum() && offset.abs() >= arm / 2.0;
    if offset.abs() >= arm || flicked {
        Side::of(offset)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{DE, EN};
    use super::*;

    #[test]
    fn the_card_follows_the_finger_past_the_slop_and_is_held_back_where_nothing_is() {
        assert_eq!(beyond_slop(6.0), 0.0);
        assert_eq!(beyond_slop(-10.0), 0.0);
        assert_eq!(beyond_slop(25.0), 15.0);
        assert_eq!(beyond_slop(-25.0), -15.0);
        // With the finger, never further than the card is wide.
        assert_eq!(card_at(-84.0, 366.0, (true, true)), -84.0);
        assert_eq!(card_at(84.0, 366.0, (true, true)), 84.0);
        assert_eq!(card_at(500.0, 366.0, (true, true)), 366.0);
        // A side with nothing to do: held back from the start, never past its room.
        let held = card_at(84.0, 366.0, (true, false));
        assert!(held > 0.0 && held < CLOSED, "{held}");
        assert!(card_at(-2000.0, 366.0, (false, true)) > -CLOSED);
        assert_eq!(card_at(0.0, 366.0, (true, true)), 0.0);
    }

    #[test]
    fn letting_go_does_what_the_side_says_only_when_it_is_armed() {
        // A card of a phone (366 px) is armed at 120 px, a narrow one at a third of it.
        assert_eq!(arm_at(366.0), ARM);
        assert_eq!(arm_at(270.0), 90.0);
        // Pulled far enough: to the left the mark, to the right the plan.
        assert_eq!(decide(-130.0, 0.0, false, 366.0), Some(Side::Mark));
        assert_eq!(decide(125.0, 0.0, false, 366.0), Some(Side::Plan));
        // Not far enough, and slow: nothing.
        assert_eq!(decide(-100.0, -0.1, true, 366.0), None);
        // Flicked that way, half as far: done; flicked back, or long ago, or not far: nothing.
        assert_eq!(decide(-70.0, -0.9, true, 366.0), Some(Side::Mark));
        assert_eq!(decide(70.0, 0.9, true, 366.0), Some(Side::Plan));
        assert_eq!(decide(-70.0, 0.9, true, 366.0), None);
        assert_eq!(decide(-70.0, -0.9, false, 366.0), None);
        assert_eq!(decide(-40.0, -0.9, true, 366.0), None);
        assert_eq!(decide(0.0, 0.9, true, 366.0), None);
    }

    #[test]
    fn the_ground_says_what_the_swipe_does_and_then_what_it_did() {
        let save = Act::mark(false, &DE);
        assert_eq!((save.icon, save.word, save.line.as_deref(), save.done, save.fills), ("bookmark", "Merken", None, "Gemerkt", true));
        let remove = Act::mark(true, &DE);
        assert_eq!((remove.icon, remove.word, remove.line.as_deref(), remove.done), ("bookmark-minus", "Entfernen", Some("von der Merkliste"), "Entfernt"));
        let remove = Act::mark(true, &EN);
        assert_eq!((remove.word, remove.line.as_deref(), remove.done), ("Remove", Some("from saved modules"), "Removed"));
        // Every icon the ground shows is one of the set.
        for icon in ["bookmark", "bookmark-minus", "calendar-plus", "calendar-minus", "calendar-check-2"] {
            assert!(crate::icons::markup(icon).is_some(), "{icon}");
        }
    }
}
