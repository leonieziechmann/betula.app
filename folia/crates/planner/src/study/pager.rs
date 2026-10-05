//! The semesters on a phone (owner, 2026-10-05: „das swiping ist richtig komisch das stockt immer
//! wieder so mach das mal, so dass das flüssig läuft. Das ist ja mittlerweile eh ne onepage
//! application, da kann man das ja so umsetzen, dass das sich wie eine native app anfühlt"): every
//! semester a card as wide as the screen, side by side in a row that the browser scrolls by itself
//! and stops at a card, one card a swipe (`scroll-snap`, app.css). A finger moves the row as it
//! moves any list, on the browser's own thread, with the browser's fling and its bounce at the
//! ends; nothing of the app runs while the row moves. ‹ › in a card's head scroll the row on to the
//! neighbour, smoothly, the same way.
//!
//! The app only hears where the row is (`scroll`): the dots follow the card in the middle, which
//! alone can be used (the others are inert); and once the row rests (`scrollend`, or no move for
//! `QUIET` where a browser does not say), the card there is the semester in focus. The row is as
//! tall as the card it rests at (the others count for nothing, app.css) and at least as tall as the
//! screen below it, so that a card coming in shows as far down as the screen does; it changes only
//! at rest, never under a finger. A card that comes after a long one the window was scrolled into
//! shows from its head. While rows are selected the row does not move sideways: the finger selects
//! (`focus.rs`). After the last semester comes the page that adds one.
//!
//! The page comes with the card it opens at and its neighbours drawn; the others follow once it
//! has come, two a step (`DRAW`), and any card stays drawn once a finger has brought it near, so
//! that the page is there at once however many semesters it has.

use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use leptos::html::Div;
use leptos::prelude::*;

use super::focus::{NewSemester, SemesterCard};
use super::{dom, Selection, StudyCtx};

/// How long after its last move the row is taken to rest where the browser does not say so
/// (`scrollend`).
const QUIET: Duration = Duration::from_millis(120);
/// When the cards beyond the neighbours are drawn: once the page has slid in (app.css, .28 s), the
/// next two after each step.
const DRAW: Duration = Duration::from_millis(320);
const DRAW_STEP: Duration = Duration::from_millis(16);

/// The card of `count` nearest to the middle of a row of cards `width` px wide scrolled `x` px,
/// and whether the row rests there (to a px or two: scrolling rounds).
fn card_at(x: f64, width: f64, count: usize) -> (usize, bool) {
    if width <= 0.0 || count == 0 {
        return (0, false);
    }
    let index = ((x / width).round().max(0.0) as usize).min(count - 1);
    (index, (x - index as f64 * width).abs() <= 2.0)
}

/// The row of cards: the semesters in order, the card in view and the one the row rests at.
/// `focus.rs` turns it by ‹ › through `turn_to`, found as context.
#[derive(Clone, Copy)]
pub(super) struct Cards {
    ctx: StudyCtx,
    /// The semesters in order, then the page that adds one.
    order: Memo<Vec<SemesterKey>>,
    /// The card in the middle of the screen, as the row moves: the dots follow it, and it alone
    /// can be used.
    shown: RwSignal<usize>,
    /// The card the row rests at: the row is as tall as it.
    here: RwSignal<usize>,
    /// The row has been placed at the semester in focus (the first time at once).
    placed: StoredValue<bool>,
    quiet: StoredValue<Option<TimeoutHandle>>,
    node: NodeRef<Div>,
    /// The card the page opened at, and how far from it the cards are drawn.
    start: usize,
    reach: RwSignal<usize>,
}

impl Cards {
    pub fn new(ctx: StudyCtx) -> Self {
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
        // Where the page opens: the semester in focus, drawn as the card in view from the start.
        let start = untrack(|| ctx.focused().and_then(|focused| order.with(|order| order.iter().position(|key| *key == focused)))).unwrap_or(0);
        Cards {
            ctx,
            order,
            shown: RwSignal::new(start),
            here: RwSignal::new(start),
            placed: StoredValue::new(false),
            quiet: StoredValue::new(None),
            node: NodeRef::new(),
            start,
            reach: RwSignal::new(1),
        }
    }

    /// Whether the card at `index` is drawn: near the one the page opened at, or near the card in
    /// view. Tracked.
    fn near(self, index: usize) -> bool {
        index.abs_diff(self.start) <= self.reach.get() || index.abs_diff(self.shown.get()) <= 1
    }

    /// The cards further out, two a step, until all are drawn.
    fn draw_on(self) {
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

    /// Where the row is: the card in the middle, whether it rests there, how wide a card is.
    fn at(self) -> Option<(usize, bool, f64)> {
        let row = self.node.get_untracked()?;
        let (x, width) = dom::scrolled_x(&row)?;
        let (index, rests) = card_at(x, width, self.order.with_untracked(Vec::len));
        Some((index, rests, width))
    }

    /// Turns to `to` (‹ ›): the row scrolls there as a finger would have it.
    pub fn turn_to(self, to: SemesterKey) {
        let (Some(row), Some(index)) = (self.node.get_untracked(), self.index_of(to)) else {
            self.ctx.focus.set(Some(to));
            return;
        };
        if let Some((_, width)) = dom::scrolled_x(&row) {
            dom::scroll_row_to(&row, index as f64 * width, true);
        }
    }

    /// The row moved: the card in the middle, and the rest once nothing moves for a while.
    fn moved(self) {
        if let Some((index, ..)) = self.at() {
            if index != self.shown.get_untracked() {
                self.shown.set(index);
            }
        }
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
        }
        self.quiet.set_value(set_timeout_with_handle(move || self.settle(), QUIET).ok());
    }

    /// The row rests: the card there is the semester in focus. Between two cards a finger still
    /// holds it, and the row's own snap comes after.
    fn settle(self) {
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
            self.quiet.set_value(None);
        }
        let Some((index, true, _)) = self.at() else { return };
        if index != self.shown.get_untracked() {
            self.shown.set(index);
        }
        if index == self.here.get_untracked() {
            return;
        }
        if let Some(row) = self.node.get_untracked() {
            dom::show_top(&row);
        }
        self.here.set(index);
        if let Some(key) = self.order.with_untracked(|order| order.get(index).copied()) {
            self.ctx.focus.set(Some(key));
        }
    }

    /// The row at the semester in focus: where the page opens (at once), and where a semester is
    /// chosen otherwise, a row's „Einplanen" or one added (smoothly). A row not laid out yet is
    /// placed in a frame to come (`tries` more).
    fn place(self, index: usize, tries: u8) {
        let Some(row) = self.node.get_untracked() else { return };
        let first = !self.placed.get_value();
        if !first && index == self.here.get_untracked() {
            return;
        }
        let Some((_, width)) = dom::scrolled_x(&row) else {
            if tries > 0 {
                request_animation_frame(move || self.place(index, tries - 1));
            }
            return;
        };
        dom::scroll_row_to(&row, index as f64 * width, !first);
        if first {
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
    let slots = Memo::new(move |_| {
        cards.order.with(|order| {
            let last = order.len().saturating_sub(1);
            order.iter().enumerate().map(|(i, key)| (*key, i == last)).collect::<Vec<_>>()
        })
    });
    let selecting = Memo::new(move |_| ctx.selection.with(|selection| !selection.keys.is_empty()));
    set_timeout(move || cards.draw_on(), DRAW);
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

/// Where the card in view stands among the semesters: a dot each, the current one ringed.
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

    /// The card nearest to the middle, and whether the row rests at it: to a px or two, as
    /// scrolling rounds; never past the last.
    #[test]
    fn the_row_knows_its_card() {
        assert_eq!(card_at(0.0, 390.0, 7), (0, true));
        assert_eq!(card_at(194.0, 390.0, 7), (0, false), "a finger short of half way");
        assert_eq!(card_at(196.0, 390.0, 7), (1, false), "past half way: the next");
        assert_eq!(card_at(781.0, 390.0, 7), (2, true));
        assert_eq!(card_at(2340.0, 390.0, 7), (6, true));
        assert_eq!(card_at(2400.0, 390.0, 7), (6, false), "bounced past the last");
        assert_eq!(card_at(-30.0, 390.0, 7), (0, false), "bounced before the first");
        assert_eq!(card_at(100.0, 0.0, 7), (0, false), "no width yet");
    }
}
