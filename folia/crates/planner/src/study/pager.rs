//! „Mein Studium" on a phone as one row of pages (owner, 2026-10-06: „als erstes muss die landing
//! page für mein Studium mit auf den pager, so dass man gleich durch swipen kann. Und dann sollten
//! alle vergangenen Semester auf die linke seite also so dass man nach rechts swipen muss, um die zu
//! sehen"): the semesters that are over, the overview (`phone.rs`), the current semester and the
//! ones to come, and after the last the page that adds one, side by side in a row that the browser
//! scrolls by itself and stops at a page, one page a swipe (`scroll-snap`, app.css). It opens at
//! the overview: a finger to the left brings the current semester, a finger to the right the last
//! one that is over. What is on two pages lies as far apart as from the screen's edge (owner,
//! 2026-10-05: „dass die Abstände zwischen den boxen genau der Abstand zum rand ist, so dass sich
//! das nicht doppelt, wenn man swiped"), so a swipe moves the row by a page and that space, less
//! than a screen. A finger moves the row as it moves any list, on the browser's own thread, with
//! the browser's fling and its bounce at the ends; nothing of the app runs while the row moves.
//! ‹ › in a semester's head scroll the row on to the semester before or after, past the overview
//! where it lies between them, smoothly, the same way.
//!
//! The app only hears where the row is (`scroll`): the dots follow the page in the middle. Once the
//! row rests (`scrollend`, or no move for `QUIET` where a browser does not say), the page there is
//! the one that can be used (the others are inert), the semester there the one in focus, and the
//! address follows: a semester is a step after the overview (`StudyUrl::plan`), the overview the
//! step before it again (back through the history where it came before). Back, the box „Studium
//! planen" and its columns, and the way back at the head move the row in turn. The row is as tall as
//! the page it rests at (the others count for nothing, app.css) and at least as tall as the screen
//! below it, so that a page coming in shows as far down as the screen does; it changes only at
//! rest, never under a finger. A page comes in from its top wherever the window was scrolled to in
//! the one before, as the page of an app does (`dom::lift`), and at rest the window is there
//! (`dom::land`). While rows are selected the row does not move sideways: the finger selects
//! (`focus.rs`).
//!
//! The row comes with the page it opens at and its neighbours drawn; the others follow once it is
//! there, two a step (`DRAW`), and the neighbours of a page the row comes to rest at at once: never
//! while a finger is on the row or it moves, whose frames the drawing would take.

use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use folia_routes::url::{self, StudyUrl};
use leptos::html::Div;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use folia_shell::pending::Pending;

use super::focus::{NewSemester, SemesterCard};
use super::phone::Overview;
use super::{dom, Selection, StudyCtx};

/// How long after its last move the row is taken to rest where the browser does not say so
/// (`scrollend`).
const QUIET: Duration = Duration::from_millis(120);
/// When the pages beyond the neighbours are drawn: once the page the row opened at is there, the
/// next two after each step.
const DRAW: Duration = Duration::from_millis(320);
const DRAW_STEP: Duration = Duration::from_millis(16);

/// A page of the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Page {
    /// The overview, between the semesters that are over and the current one.
    Overview,
    /// A semester; the one after the last is the page that adds it.
    Semester(SemesterKey),
}

/// The pages of a study whose semesters are `semesters` (each with whether it is over), in their
/// order: the semesters that are over, the overview, the current one and the ones to come, and the
/// page that adds one after the last.
fn order_of(semesters: &[(SemesterKey, bool)]) -> Vec<Page> {
    let over = semesters.iter().take_while(|(_, over)| *over).count();
    let mut pages: Vec<Page> = semesters.iter().map(|(key, _)| Page::Semester(*key)).collect();
    if let Some(end) = semesters.last().and_then(|(last, _)| last.plus(1)) {
        pages.push(Page::Semester(end));
    }
    pages.insert(over, Page::Overview);
    pages
}

/// The page of `count` nearest to the middle of a row scrolled `x` px whose pages rest `step` px
/// apart, and whether the row rests there (to a px or two: scrolling rounds).
fn card_at(x: f64, step: f64, count: usize) -> (usize, bool) {
    if step <= 0.0 || count == 0 {
        return (0, false);
    }
    let index = ((x / step).round().max(0.0) as usize).min(count - 1);
    (index, (x - index as f64 * step).abs() <= 2.0)
}

/// The address of a semester's page.
pub(super) fn plan_path() -> String {
    StudyUrl::default().with_plan(true).path()
}

/// The page the address and the focus put the row at: the semester in focus where the address
/// says a semester, else the overview. Tracked.
fn wanted(ctx: StudyCtx) -> Option<Page> {
    if ctx.url.with(|url| url.plan) {
        ctx.focused().map(Page::Semester)
    } else {
        Some(Page::Overview)
    }
}

/// The row of pages: the pages in order, the page in view and the one the row rests at. `focus.rs`
/// turns it by ‹ › through `turn_to`, found as context.
#[derive(Clone, Copy)]
pub(super) struct Pages {
    ctx: StudyCtx,
    order: Memo<Vec<Page>>,
    /// The page in the middle of the screen, as the row moves: the dots follow it.
    shown: RwSignal<usize>,
    /// The page the row rests at: it alone can be used, the row is as tall as it, and the address
    /// says it.
    here: RwSignal<usize>,
    /// The row has been placed where the address says (the first time at once).
    placed: StoredValue<bool>,
    quiet: StoredValue<Option<TimeoutHandle>>,
    /// The pages it does not rest at are drawn from their tops (`dom::lift`).
    lifted: StoredValue<bool>,
    /// A finger is on the row.
    touching: StoredValue<bool>,
    node: NodeRef<Div>,
    /// The page the row opened at, and how far from it the pages are drawn.
    start: usize,
    reach: RwSignal<usize>,
    going: Option<Pending>,
}

impl Pages {
    pub fn new(ctx: StudyCtx) -> Self {
        let order = Memo::new(move |_| {
            ctx.with_ready(|ready| order_of(&ready.study.semesters.iter().map(|semester| (semester.key, semester.when == When::Past)).collect::<Vec<_>>())).unwrap_or_default()
        });
        // Where the row opens: where the address says, drawn as the page in view from the start.
        let start = untrack(|| wanted(ctx).and_then(|page| order.with(|order| order.iter().position(|other| *other == page)))).unwrap_or(0);
        Pages {
            ctx,
            order,
            shown: RwSignal::new(start),
            here: RwSignal::new(start),
            placed: StoredValue::new(false),
            quiet: StoredValue::new(None),
            lifted: StoredValue::new(false),
            touching: StoredValue::new(false),
            node: NodeRef::new(),
            start,
            reach: RwSignal::new(1),
            going: Pending::expect(),
        }
    }

    /// The page the row rests at. Tracked.
    pub fn here(self) -> Option<Page> {
        let here = self.here.get();
        self.order.with(|order| order.get(here).copied())
    }

    /// Whether the page at `index` is drawn: near the one the row opened at, or next to the one it
    /// rests at. Tracked.
    fn near(self, index: usize) -> bool {
        index.abs_diff(self.start) <= self.reach.get() || index.abs_diff(self.here.get()) <= 1
    }

    /// The pages further out, two a step, until all are drawn; not while a finger is on the row or
    /// it moves, whose frames the drawing would take.
    fn draw_on(self) {
        let Some(busy) = self.touching.try_get_value().zip(self.quiet.try_with_value(Option::is_some)).map(|(touching, moving)| touching || moving) else { return };
        if busy {
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

    fn index_of(self, page: Page) -> Option<usize> {
        self.order.with_untracked(|order| order.iter().position(|other| *other == page))
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

    /// The pages coming in are drawn from their tops (a finger on the row, the row moved by ‹ › or
    /// the address).
    fn lift(self) {
        if let Some(row) = self.node.get_untracked() {
            dom::lift(&row);
            self.lifted.set_value(true);
        }
    }

    /// Turns to semester `to` (‹ ›): the row scrolls there as a finger would have it.
    pub fn turn_to(self, to: SemesterKey) {
        match self.index_of(Page::Semester(to)) {
            Some(index) => self.go(index),
            None => self.ctx.focus.set(Some(to)),
        }
    }

    /// The row scrolls to the page at `index`, smoothly, the pages coming in drawn from their tops.
    fn go(self, index: usize) {
        let Some(row) = self.node.get_untracked() else { return };
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

    /// The row rests: at another page, the window shows it from its top, the semester there is the
    /// one in focus, and the address follows. Between two pages a finger still holds it, and the
    /// row's own snap comes after.
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
        let Some(page) = self.order.with_untracked(|order| order.get(index).copied()) else { return };
        if let Page::Semester(key) = page {
            self.ctx.focus.set(Some(key));
        }
        self.follow(page);
    }

    /// The address follows the page the row came to rest at: a semester a step after the overview,
    /// the overview back through the history where it was the step before.
    fn follow(self, page: Page) {
        let plan = page != Page::Overview;
        if self.ctx.url.with_untracked(|url| url.plan) == plan {
            return;
        }
        if !plan && self.ctx.from_overview.get_untracked() {
            dom::history_back();
            return;
        }
        let to = if plan { plan_path() } else { url::STUDY.to_string() };
        if let Some(going) = self.going {
            going.go(&to, NavigateOptions { scroll: false, ..Default::default() });
        }
    }

    /// The row at the page at `index`: at once where it opens, smoothly where the address or the
    /// focus moves it (Back, the box „Studium planen", the way back at the head, a row's „Einplanen",
    /// a semester added). A row not laid out yet is placed in a frame to come (`tries` more).
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
        if first {
            dom::scroll_row_to(&row, index as f64 * step, false);
            self.placed.set_value(true);
            self.shown.set(index);
            self.here.set(index);
        } else {
            self.lift();
            dom::scroll_row_to(&row, index as f64 * step, true);
        }
    }
}

#[component]
pub(super) fn Pager(ctx: StudyCtx, pages: Pages) -> impl IntoView {
    // What is selected belongs to the semester in focus: turning to another lets go of it.
    Effect::new(move |_| {
        let now = ctx.focused();
        if ctx.selection.with_untracked(|selection| selection.semester.is_some() && selection.semester != now) {
            ctx.selection.set(Selection::default());
        }
    });
    // The row follows the address: to the semester in focus where it says a semester (Back, the
    // box „Studium planen" and its columns), to the overview where it does not (the way back).
    let plan = Memo::new(move |_| ctx.url.with(|url| url.plan));
    Effect::new(move |_| {
        let plan = plan.get();
        if pages.node.get().is_none() {
            return;
        }
        let page = untrack(|| if plan { ctx.focused().map(Page::Semester) } else { Some(Page::Overview) });
        if let Some(index) = page.and_then(|page| pages.order.with_untracked(|order| order.iter().position(|other| *other == page))) {
            pages.place(index, 10);
        }
    });
    // And the focus, where the row rests at a semester (a row's „Einplanen", a semester added); at
    // the overview the address moves it.
    Effect::new(move |before: Option<Option<usize>>| {
        let index = ctx.focused().and_then(|focused| pages.order.with(|order| order.iter().position(|other| *other == Page::Semester(focused))));
        if before.is_some() && before != Some(index) && untrack(|| pages.here()).is_some_and(|here| here != Page::Overview) {
            if let Some(index) = index {
                pages.place(index, 10);
            }
        }
        index
    });
    // A finger on the row: the pages coming in are drawn from their tops before it moves.
    Effect::new(move |_| {
        if let Some(row) = pages.node.get() {
            dom::on_finger(&row, move |down| pages.finger(down));
        }
    });
    // The pages further out, once the page the row opened at is there.
    set_timeout(move || pages.draw_on(), DRAW);
    on_cleanup(move || {
        if let Some(Some(handle)) = pages.quiet.try_get_value() {
            handle.clear();
        }
    });
    let slots = Memo::new(move |_| {
        pages.order.with(|order| {
            let last = order.len().saturating_sub(1);
            order.iter().enumerate().map(|(i, page)| (*page, i == last)).collect::<Vec<_>>()
        })
    });
    let selecting = Memo::new(move |_| ctx.selection.with(|selection| !selection.keys.is_empty()));
    view! {
        <div class="st-pager" class:is-selecting=selecting node_ref=pages.node on:scroll=move |_| pages.moved() on:scrollend=move |_| pages.settle()>
            <For
                each=move || slots.get()
                key=|(page, end)| (*page, *end)
                children=move |(page, end)| {
                    let index = Memo::new(move |_| pages.order.with(|order| order.iter().position(|other| *other == page)));
                    let here = Memo::new(move |_| index.get().is_some_and(|index| index == pages.here.get()));
                    // Once drawn, drawn.
                    let drawn = Memo::new(move |before: Option<&bool>| before.copied().unwrap_or(false) || index.get().is_some_and(|index| pages.near(index)));
                    view! {
                        <div
                            class="st-slot"
                            class:is-overview=page == Page::Overview
                            data-here=move || here.get().then_some("")
                            inert=move || !here.get()
                            aria-hidden=move || (!here.get()).then_some("true")
                        >
                            {move || drawn.get().then(|| match page {
                                Page::Overview => view! { <Overview ctx/> }.into_any(),
                                Page::Semester(_) if end => view! { <NewSemester ctx/> }.into_any(),
                                Page::Semester(key) => view! { <SemesterCard ctx semester=key/> }.into_any(),
                            })}
                        </div>
                    }
                }
            />
        </div>
    }
}

/// Where the page in view stands in the row: a dot each, the overview's a little square between the
/// semesters that are over and the current one, the current one's ringed.
#[component]
pub(super) fn Dots(ctx: StudyCtx, pages: Pages) -> impl IntoView {
    let now = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.iter().find(|semester| semester.when == When::Now).map(|semester| semester.key)).flatten());
    let dots = Memo::new(move |_| {
        let shown = pages.shown.get();
        let now = now.get();
        pages.order.with(|order| order.iter().enumerate().map(|(i, page)| (i == shown, *page == Page::Overview, now.is_some_and(|now| *page == Page::Semester(now)))).collect::<Vec<_>>())
    });
    view! {
        <div class="st-dots" aria-hidden="true">
            {move || dots.get().into_iter().map(|(here, overview, now)| view! { <span class:here=here class:overview=overview class:now=now></span> }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

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

    /// The semesters that are over to the left of the overview, the current one and the ones to
    /// come to its right, the page that adds one at the end (owner, 2026-10-06).
    #[test]
    fn the_overview_lies_between_what_is_over_and_what_comes() {
        let semesters = [(key("2025W"), true), (key("2026S"), true), (key("2026W"), false), (key("2027S"), false)];
        assert_eq!(
            order_of(&semesters),
            [Page::Semester(key("2025W")), Page::Semester(key("2026S")), Page::Overview, Page::Semester(key("2026W")), Page::Semester(key("2027S")), Page::Semester(key("2027W"))]
        );
        assert_eq!(order_of(&semesters[2..])[0], Page::Overview, "nothing over yet: the overview first");
        assert_eq!(order_of(&semesters[..2]), [Page::Semester(key("2025W")), Page::Semester(key("2026S")), Page::Overview, Page::Semester(key("2026W"))], "all over: the page that adds one after it");
        assert_eq!(order_of(&[]), [Page::Overview]);
    }

    #[test]
    fn a_semester_has_an_address_of_its_own() {
        assert_eq!(plan_path(), "/study?plan=1");
    }
}
