//! The semesters on a phone (owner, 2026-10-05: „das swiping ist richtig komisch das stockt immer
//! wieder so mach das mal, so dass das flüssig läuft. Das ist ja mittlerweile eh ne onepage
//! application, da kann man das ja so umsetzen, dass das sich wie eine native app anfühlt"): every
//! semester a page of its own, side by side in a row that the browser scrolls by itself and stops
//! at a page, one page a swipe (`scroll-snap`, app.css). What is on two pages lies as far apart as
//! from the screen's edge (owner, the same day: „dass die Abstände zwischen den boxen genau der
//! Abstand zum rand ist, so dass sich das nicht doppelt, wenn man swiped"), so a swipe moves the row
//! by a page and that space, less than a screen. A finger moves the row as it moves any list, on
//! the browser's own thread, with the browser's fling and its bounce at the ends; nothing of the
//! app runs while the row moves. ‹ › in a semester's head scroll the row on to the neighbour,
//! smoothly, the same way. On the first semester a finger to the right goes on to the overview
//! (`phone.rs`, whose row of two pages this one lies in).
//!
//! The app only hears where the row is (`scroll`): the dots follow the page in the middle, which
//! alone can be used (the others are inert); and once the row rests (`scrollend`, or no move for
//! `QUIET` where a browser does not say), the semester there is the one in focus. The row is as
//! tall as the page it rests at (the others count for nothing, app.css) and at least as tall as
//! the screen below it, so that a page coming in shows as far down as the screen does; it changes
//! only at rest, never under a finger. A page comes in from its top wherever the window was
//! scrolled to in the one before, as the page of an app does (`dom::lift`), and at rest the window
//! is there (`dom::land`). While rows are selected the row does not move sideways: the finger
//! selects (`focus.rs`). After the last semester comes the page that adds one.
//!
//! The semesters come with the page they open at and its neighbours drawn; the others follow once
//! they are in view, two a step (`DRAW`), and any page stays drawn once a finger has brought it
//! near, so that they are there at once however many semesters there are.

use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use leptos::html::Div;
use leptos::prelude::*;

use super::focus::{NewSemester, SemesterCard};
use super::{dom, Selection, StudyCtx};

/// How long after its last move the row is taken to rest where the browser does not say so
/// (`scrollend`).
pub(super) const QUIET: Duration = Duration::from_millis(120);
/// When the pages beyond the neighbours are drawn: once the semesters are in view (and have slid
/// in, app.css), the next two after each step.
const DRAW: Duration = Duration::from_millis(320);
const DRAW_STEP: Duration = Duration::from_millis(16);

/// The page of `count` nearest to the middle of a row scrolled `x` px whose pages rest `step` px
/// apart, and whether the row rests there (to a px or two: scrolling rounds).
pub(super) fn card_at(x: f64, step: f64, count: usize) -> (usize, bool) {
    if step <= 0.0 || count == 0 {
        return (0, false);
    }
    let index = ((x / step).round().max(0.0) as usize).min(count - 1);
    (index, (x - index as f64 * step).abs() <= 2.0)
}

/// The row of semesters: the semesters in order, the page in view and the one the row rests at.
/// `focus.rs` turns it by ‹ › through `turn_to`, found as context.
#[derive(Clone, Copy)]
pub(super) struct Cards {
    ctx: StudyCtx,
    /// The semesters in order, then the page that adds one.
    order: Memo<Vec<SemesterKey>>,
    /// The page in the middle of the screen, as the row moves: the dots follow it, and it alone
    /// can be used.
    shown: RwSignal<usize>,
    /// The page the row rests at: the row is as tall as it.
    here: RwSignal<usize>,
    /// The row has been placed at the semester in focus (the first time at once).
    placed: StoredValue<bool>,
    quiet: StoredValue<Option<TimeoutHandle>>,
    /// The pages it does not rest at are drawn from their tops (`dom::lift`).
    lifted: StoredValue<bool>,
    /// A finger is on the row.
    touching: StoredValue<bool>,
    node: NodeRef<Div>,
    /// The semesters are the page of a phone the row of pages rests at (`phone.rs`): elsewhere the
    /// row goes to a semester at once, and draws no more than it opened with.
    visible: Signal<bool>,
    /// The page the row opened at, and how far from it the pages are drawn.
    start: usize,
    reach: RwSignal<usize>,
}

impl Cards {
    pub fn new(ctx: StudyCtx, visible: Signal<bool>) -> Self {
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
        // Where the row opens: the semester in focus, drawn as the page in view from the start.
        let start = untrack(|| ctx.focused().and_then(|focused| order.with(|order| order.iter().position(|key| *key == focused)))).unwrap_or(0);
        Cards {
            ctx,
            order,
            shown: RwSignal::new(start),
            here: RwSignal::new(start),
            placed: StoredValue::new(false),
            quiet: StoredValue::new(None),
            lifted: StoredValue::new(false),
            touching: StoredValue::new(false),
            node: NodeRef::new(),
            visible,
            start,
            reach: RwSignal::new(1),
        }
    }

    /// Whether the page at `index` is drawn: near the one the row opened at, or near the page in
    /// view. Tracked.
    fn near(self, index: usize) -> bool {
        index.abs_diff(self.start) <= self.reach.get() || index.abs_diff(self.shown.get()) <= 1
    }

    /// The pages further out, two a step, until all are drawn; not while a finger is on the row or
    /// it moves, whose frames the drawing would take.
    fn draw_on(self) {
        if self.touching.get_value() || self.quiet.with_value(Option::is_some) {
            set_timeout(move || self.draw_on(), QUIET);
            return;
        }
        let more = self.reach.try_update(|reach| {
            *reach += 1;
            *reach
        });
        if more.is_some_and(|reach| reach < self.order.with_untracked(Vec::len)) {
            set_timeout(move || self.draw_on(), DRAW_STEP);
        }
    }

    fn index_of(self, key: SemesterKey) -> Option<usize> {
        self.order.with_untracked(|order| order.iter().position(|other| *other == key))
    }

    /// The semester at `index` in the row, the page that adds one after the last.
    fn key_at(self, index: usize) -> Option<SemesterKey> {
        self.order.with_untracked(|order| order.get(index).copied())
    }

    /// Where the row is: the page in the middle, whether it rests there.
    fn at(self) -> Option<(usize, bool)> {
        let row = self.node.get_untracked()?;
        let (x, step) = dom::scrolled_cards(&row)?;
        Some(card_at(x, step, self.order.with_untracked(Vec::len)))
    }

    /// A finger comes down on the row (the pages coming in are drawn from their tops), or goes.
    fn finger(self, down: bool) {
        if down {
            self.lift();
        }
        self.touching.set_value(down);
    }

    /// The pages coming in are drawn from their tops (a finger on the row, a turn by ‹ ›).
    fn lift(self) {
        if let Some(row) = self.node.get_untracked() {
            dom::lift(&row);
            self.lifted.set_value(true);
        }
    }

    /// Turns to `to` (‹ ›): the row scrolls there as a finger would have it.
    pub fn turn_to(self, to: SemesterKey) {
        let (Some(row), Some(index)) = (self.node.get_untracked(), self.index_of(to)) else {
            self.ctx.focus.set(Some(to));
            return;
        };
        if let Some((_, step)) = dom::scrolled_cards(&row) {
            self.lift();
            dom::scroll_row_to(&row, index as f64 * step, true);
        }
    }

    /// The row moved: the page in the middle, and the rest once nothing moves for a while. A row
    /// moved by what is no finger (a trackpad, the keys) draws the pages coming in from their tops
    /// now.
    fn moved(self) {
        if !self.lifted.get_value() {
            self.lift();
        }
        if let Some((index, _)) = self.at() {
            if index != self.shown.get_untracked() {
                self.shown.set(index);
            }
        }
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
        }
        self.quiet.set_value(set_timeout_with_handle(move || self.settle(), QUIET).ok());
    }

    /// The row rests: the semester there is the one in focus, shown from its top. Between two
    /// pages a finger still holds it, and the row's own snap comes after.
    fn settle(self) {
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
            self.quiet.set_value(None);
        }
        let Some((index, true)) = self.at() else { return };
        if index != self.shown.get_untracked() {
            self.shown.set(index);
        }
        let moved = index != self.here.get_untracked();
        if let Some(row) = self.node.get_untracked() {
            dom::land(&row, moved);
            self.lifted.set_value(false);
        }
        if !moved {
            return;
        }
        self.here.set(index);
        if let Some(key) = self.key_at(index) {
            self.ctx.focus.set(Some(key));
        }
    }

    /// The row at the semester in focus: where it opens (at once), and where a semester is chosen
    /// otherwise, a row's „Einplanen", one added, a column of the overview's box (smoothly while
    /// the semesters are in view, else at once). A row not laid out yet is placed in a frame to
    /// come (`tries` more).
    fn place(self, index: usize, tries: u8) {
        let Some(row) = self.node.get_untracked() else { return };
        let first = !self.placed.get_value();
        if !first && index == self.here.get_untracked() {
            return;
        }
        let Some((_, step)) = dom::scrolled_cards(&row) else {
            if tries > 0 {
                request_animation_frame(move || self.place(index, tries - 1));
            }
            return;
        };
        let smooth = !first && self.visible.get_untracked();
        if smooth {
            self.lift();
        }
        dom::scroll_row_to(&row, index as f64 * step, smooth);
        if !smooth {
            self.placed.set_value(true);
            self.shown.set(index);
            self.here.set(index);
        }
    }
}

#[component]
pub(super) fn Pager(ctx: StudyCtx, cards: Cards) -> impl IntoView {
    // What is selected belongs to the semester in focus: turning to another lets go of it.
    Effect::new(move |_| {
        let now = ctx.focused();
        if ctx.selection.with_untracked(|selection| selection.semester.is_some() && selection.semester != now) {
            ctx.selection.set(Selection::default());
        }
    });
    Effect::new(move |_| {
        if cards.node.get().is_none() {
            return;
        }
        let index = ctx.focused().and_then(|focused| cards.order.with(|order| order.iter().position(|key| *key == focused)));
        if let Some(index) = index {
            cards.place(index, 10);
        }
    });
    // A finger on the row: the pages coming in are drawn from their tops before it moves.
    Effect::new(move |_| {
        if let Some(row) = cards.node.get() {
            dom::on_finger(&row, move |down| cards.finger(down));
        }
    });
    // The pages further out once the semesters are in view.
    Effect::new(move |started: Option<bool>| {
        if started == Some(true) || !cards.visible.get() {
            return started.unwrap_or(false);
        }
        set_timeout(move || cards.draw_on(), DRAW);
        true
    });
    let slots = Memo::new(move |_| {
        cards.order.with(|order| {
            let last = order.len().saturating_sub(1);
            order.iter().enumerate().map(|(i, key)| (*key, i == last)).collect::<Vec<_>>()
        })
    });
    let selecting = Memo::new(move |_| ctx.selection.with(|selection| !selection.keys.is_empty()));
    view! {
        <div class="st-pager" class:is-selecting=selecting node_ref=cards.node on:scroll=move |_| cards.moved() on:scrollend=move |_| cards.settle()>
            <For
                each=move || slots.get()
                key=|(key, end)| (*key, *end)
                children=move |(key, end)| {
                    let index = Memo::new(move |_| cards.order.with(|order| order.iter().position(|other| *other == key)));
                    let shown = Memo::new(move |_| index.get() == Some(cards.shown.get()));
                    // Once drawn, drawn.
                    let drawn = Memo::new(move |before: Option<&bool>| before.copied().unwrap_or(false) || index.get().is_some_and(|index| cards.near(index)));
                    view! {
                        <div
                            class="st-slot"
                            data-here=move || (index.get() == Some(cards.here.get())).then_some("")
                            inert=move || !shown.get()
                            aria-hidden=move || (!shown.get()).then_some("true")
                        >
                            {move || drawn.get().then(|| if end { view! { <NewSemester ctx/> }.into_any() } else { view! { <SemesterCard ctx semester=key/> }.into_any() })}
                        </div>
                    }
                }
            />
        </div>
    }
}

/// Where the page in view stands among the semesters: a dot each, the current one ringed.
#[component]
pub(super) fn Dots(ctx: StudyCtx, cards: Cards) -> impl IntoView {
    let dots = Memo::new(move |_| {
        let shown = cards.shown.get();
        ctx.with_ready(|ready| {
            let mut dots: Vec<(bool, bool)> = ready.study.semesters.iter().enumerate().map(|(i, semester)| (i == shown, semester.when == When::Now)).collect();
            dots.push((shown == dots.len(), false));
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

    /// The page nearest to the middle, and whether the row rests at it: to a px or two, as
    /// scrolling rounds; never past the last. A screen 390 px wide: pages 366 px wide, their
    /// content 12 px from its edges and 12 px apart, 378 px a step.
    #[test]
    fn the_row_knows_its_card() {
        assert_eq!(card_at(0.0, 378.0, 7), (0, true));
        assert_eq!(card_at(188.0, 378.0, 7), (0, false), "a finger short of half way");
        assert_eq!(card_at(190.0, 378.0, 7), (1, false), "past half way: the next");
        assert_eq!(card_at(757.0, 378.0, 7), (2, true));
        assert_eq!(card_at(2268.0, 378.0, 7), (6, true));
        assert_eq!(card_at(2330.0, 378.0, 7), (6, false), "bounced past the last");
        assert_eq!(card_at(-30.0, 378.0, 7), (0, false), "bounced before the first");
        assert_eq!(card_at(100.0, 0.0, 7), (0, false), "no width yet");
        // A screen of a fraction of a px: the row rests at a page all the same, far along.
        assert_eq!(card_at(3994.0, 399.43, 12), (10, true));
    }
}
