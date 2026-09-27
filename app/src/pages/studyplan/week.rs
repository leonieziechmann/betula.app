//! The views „Woche" and „Termine" of one semester of the Studienplan.
//!
//! „Woche" is the Regelwoche (`Timetable::regular_week`): a slot per recurring Termin at its
//! weekday and time in the week grid (`crate::week`, fitted to the page's height), and the dates of
//! an event that do not recur gathered into one slot („3 Termine"). The week is there before
//! anything is planned, an empty frame with „Noch keine Termine" in its middle and the ways to
//! modules under it (`NothingPlanned`, owner 2026-09-25; on a phone the words and the ways alone,
//! the page shorter than the screen). A slot says in three lines what and whose („VL EvS"), when
//! and in which weeks („07:30–09:00 A") and where („ZHG/HS.C"); its tooltip says the rest. Each
//! slot is a link to its module beside the plan, pointing at that Termin (`open`, `row`). A phone
//! has the grid too, as wide as its screen and as tall as it leaves it (owner, 2026-09-27), and
//! under it the same slots as a list of days, closed until asked for, whose rows are large enough
//! for a finger and its buttons. What has no fixed time stands under „Ohne feste Zeit", once for
//! the semester, under the fold (`WeekLoose`). Where the plan has Termine of A or B weeks only,
//! „A-Woche · B-Woche · A/B" in the head shows one kind of week or both (`PlanCtx::weeks`), on a
//! phone the tabs of a carousel of the three (`WeekCarousel`); a slot that overlaps another in the
//! week shown is red and names the other in its tooltip (owner's redesign of 2026-09-25).
//!
//! What to do with a Termin is decided right on it (owner, 2026-09-25: „ja der fliegt raus, die
//! Übung möchte ich", without the module beside the plan, and without a menu in between): „✓"
//! takes an option of a choice and stays on, filled, so that a second click takes the choice back
//! (owner: „schnell abwählen"); „×" leaves a Termin or an option out (`SlotAct`, written after the
//! next frame, R21). The buttons of a slot with a „✓" stand in view, the „×" of any other slot
//! comes when it is pointed at. A module's A-week and B-week slots at one time that clash stand
//! together in one red frame (`join_ab`): together they compete with the other. „Plan · Alle Termine" in the head (`AllSwitch`, `PlanCtx::all`) shows
//! besides what the plan leaves out, faint (`ghost_slots`): a hidden event, kind or Termin, an
//! option another was chosen over — red-edged where it would meet a Termin of the plan, and its
//! „✓" takes it back.
//!
//! „Termine" is the agenda (`Timetable::agenda`): every date by week and day, what is cancelled
//! and why, the holidays, the exams, and the weeks of a break with nothing in them as one line. A
//! plan with A- or B-week Termine names each week of the lecture period „A-Woche" or „B-Woche".
//! What it cannot place on a day (a Termin „nach Vereinbarung", an exam whose date is open)
//! stands under „Ohne Datum". It opens at the current week (the page reads the clock once,
//! `PlanCtx::today`).
//!
//! A view comes back to where the visitor left it for a module (`Place`): after „Vollbild" and
//! „Zurück", and on a phone, where the module was the page, when it is closed.
//!
//! Both views read the timetable and the address without what stands beside the plan (`base`),
//! two siblings of the address (R16). The slot or date being opened is marked from where the app
//! is going (`Pending`), so a click answers in the next frame (R21). What they show is built as
//! plain values first (`PlanSlot`, `Block`, `LooseLine`), which the tests read without rendering;
//! the lists are keyed by those values, so a hidden event redraws the days it was on and no
//! other (R5).

use std::collections::{BTreeMap, BTreeSet};

use catalog::labels::Rhythm;
use catalog::rows_detail::EventDate;
use catalog::studyplan::PlanDoc;
use catalog::timetable::clash::Weeks;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{ExamShape, Termin};
use catalog::timetable::facts::SemesterFacts;
use catalog::timetable::kind::EventKind;
use catalog::timetable::model::{Event, Row, Timetable};
use catalog::timetable::occur::Every;
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::HiddenBy;
use catalog::timetable::semester::SemesterKey;
use catalog::timetable::views::{kind_and_title, kind_short, short_title, type_text, AgendaItem, AgendaWeek, Reach, WeekItem, WeekLabel};
use catalog::url::{self, StudyplanUrl};
use leptos::prelude::*;

use super::head::{hue, kind_word, tone_at, NothingPlanned};
use super::PlanCtx;
use crate::format;
use crate::i18n::{self, Locale};
use crate::nav;
use crate::pending::Pending;
use crate::ui::Icon;
use crate::week::{slot_buttons, GridSlot, SlotButton, WeekGrid};

/// Where this browser tab remembers the link the visitor last left a view by (`Place`).
const LEFT_KEY: &str = "betula.studyplan.left";

/// The id of a phone's list of days, which its line opens.
const DAYS_ID: &str = "sp-days";

/// The module and the Termin the address puts beside the plan: what a slot or a row is marked by.
type Picked = (Option<String>, Option<RowKey>);

/// „Woche": the Regelwoche as a grid that fits the page's height (`WeekGrid`'s `fit`, in
/// `div.sp-week`), on a phone as a grid that fits the screen over the list of its days (in
/// `div.sp-days`, closed until its line opens it, `PlanCtx::days`), a carousel of its weeks where
/// the plan has A or B weeks (`WeekCarousel`); with „Alle Termine" what the plan leaves out
/// besides, faint. A slot's buttons change the plan right there (`SlotAct`), on a phone a row's.
/// Without a planned module, the empty week and in its middle what is to do (`NothingPlanned`,
/// with the marked modules the semester could take, `marked`), on a phone what is to do alone.
/// What has no fixed time stands under the page's fold (`WeekLoose`), the switches of the Termine
/// shown and of A and B weeks in the head (`AllSwitch`, `WeekSwitch`; on a phone the weeks are the
/// carousel's tabs).
#[component]
pub(super) fn WeekView(ctx: PlanCtx, marked: Memo<Vec<(String, String)>>) -> impl IntoView {
    let t = i18n::t();
    let base = base_of(ctx);
    let picked = picked_of(ctx);
    // What the slots name the modules by, their abbreviations („EvS"; owner, 2026-09-25: only where
    // no title fits): a sibling of the timetable, both derived from the semester's data (R16).
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    // The slots of each week (`WEEKS`): a phone's carousel shows all three at once. A memo is
    // worked out only where it is read, so a wide screen works out the week it shows alone.
    let weeks = WEEKS.map(|week| {
        let slots = Memo::new(move |_| {
            let (base, all) = (base.get(), ctx.all.get());
            titles.with(|titles| ctx.table.with(|table| table.as_ref().map(|table| week_slots(table, &base, titles, week, all, t)).unwrap_or_default()))
        });
        (week, slots)
    });
    // The week shown (`PlanCtx::weeks`): the grid's, the list's and the buttons'.
    let slots = Memo::new(move |_| {
        let shown = ctx.weeks.get();
        weeks.iter().find(|(week, _)| *week == shown).map(|(_, slots)| slots.get()).unwrap_or_default()
    });
    let empty = Memo::new(move |_| ctx.wanted.with(|wanted| wanted.1.is_empty()));
    let nothing = move || empty.get().then(|| view! { <NothingPlanned ctx marked/> });
    // The buttons of the slots, one listener for all of them: a click changes the plan after the
    // next frame (R21), and the week follows.
    let act = move |ev: leptos::ev::MouseEvent| {
        let Some(key) = act_under(ev.target()) else { return };
        ev.stop_propagation();
        let (Some(plan), Some(act)) = (ctx.plan, slots.with_untracked(|slots| act_of(slots, &key))) else { return };
        let semester = ctx.key.get_untracked();
        plan.update_after_paint(move |doc| act.apply(doc, semester));
    };
    // On a phone the list of days is closed until asked for (owner, 2026-09-27): the grid over it
    // shows the week. Its line says how many Termine it holds.
    let rows = Memo::new(move |_| slots.with(Vec::len));
    let week = move || {
        // A phone with nothing planned says what to do where the week will stand, and the page
        // stays shorter than the screen: an empty week would take all of it.
        if ctx.phone.get() && empty.get() {
            return view! { <NothingPlanned ctx marked/> }.into_any();
        }
        if ctx.phone.get() {
            return view! {
                <WeekCarousel ctx weeks shown=slots picked/>
                {move || {
                    (rows.get() > 0).then(|| {
                        view! {
                            <button
                                class="sp-days-toggle"
                                type="button"
                                aria-expanded=move || if ctx.days.get() { "true" } else { "false" }
                                aria-controls=DAYS_ID
                                on:click=move |_| ctx.days.update(|open| *open = !*open)
                            >
                                {t.studyplan_week.days_toggle}
                                <span class="num">{move || format!("({})", rows.get())}</span>
                            </button>
                        }
                    })
                }}
                {move || {
                    (ctx.days.get() && rows.get() > 0).then(|| {
                        view! {
                            <div class="sp-days" id=DAYS_ID on:click=act>
                                <DayList slots picked/>
                            </div>
                        }
                    })
                }}
            }
            .into_any();
        }
        let grid = Signal::derive(move || {
            let picked = picked.get();
            slots.with(|slots| slots.iter().map(|slot| GridSlot { current: slot.row.is(&picked), href: linked(&slot.slot, t), ..slot.slot.clone() }).collect::<Vec<_>>())
        });
        view! {
            <div class="sp-week" on:click=act>
                <WeekGrid slots=grid fit=true/>
                {nothing}
            </div>
        }
        .into_any()
    };

    // Back from a module that filled the page („Vollbild", then „Zurück"; the address names it
    // still): at the line it was opened from. On a phone the module beside the plan is the page:
    // there the plan comes back when it is closed.
    Effect::new(move |_| {
        let here = ctx.url.get_untracked();
        if here.open.is_some() && !nav::is_phone() {
            come_back(&here, None, t.locale);
        }
    });
    back_on_phone(ctx, t.locale, || None);

    week
}

/// The Regelwoche on a phone and a small screen (owner, 2026-09-27: „Es gibt keine Wochenansicht
/// beim Kalender auf dem Smartphone"): the grid of a wide screen, as wide as the screen and as tall
/// as it leaves it, over the list of its days (closed until asked for). Where the plan has Termine
/// of A or B weeks only (`has_ab`), a carousel of the three weeks „A-Woche", „B-Woche" and „A/B",
/// as the pictures of the start page are one (`home.rs`), with the tabs under it: the week shown
/// fills the screen's width, another one comes with a swipe, the finger carrying the weeks, or with
/// its tab. They stand in the order of the head's switch and do not go round: the three are no
/// cycle. The week shown is the page's (`PlanCtx::weeks`), which the list under the carousel
/// follows. Without A or B weeks the three would be one and the same: the grid alone. Its slots are
/// links to their module (on a phone the module is the page); their buttons are the list's, as
/// large as a finger (in a day's narrow column they would cover the slot). With nothing planned
/// there is no week (`WeekView`).
#[component]
fn WeekCarousel(ctx: PlanCtx, weeks: [(Weeks, Memo<Vec<PlanSlot>>); 3], shown: Memo<Vec<PlanSlot>>, picked: Memo<Picked>) -> impl IntoView {
    let t = i18n::t();
    let ab = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(has_ab)));
    let current = Memo::new(move |_| week_at(ctx.weeks.get()));
    // A grid of the carousel: its slots, the one beside the plan marked, without buttons.
    let grid = move |slots: Memo<Vec<PlanSlot>>| {
        Signal::derive(move || {
            let picked = picked.get();
            slots.with(|slots| slots.iter().map(|slot| GridSlot { current: slot.row.is(&picked), href: linked(&slot.slot, t), acts: Vec::new(), asks: false, ..slot.slot.clone() }).collect::<Vec<_>>())
        })
    };
    // To the week at `to` of `WEEKS`; the ends stay where they are.
    let go = move |to: usize| {
        if let Some(week) = WEEKS.get(to) {
            ctx.weeks.set(*week);
        }
    };
    // A finger that moves sideways further than up or down carries the weeks with it (`drag`, in
    // pixels; the page scrolls under any other move), less where no week lies beyond. Let go
    // further than `SWIPE`, the next week or the one before comes, else the week goes back. It
    // ends in a click on what lay under the finger, and that click opens nothing.
    let start = StoredValue::new(None::<(i32, i32)>);
    let carried = StoredValue::new(false);
    let drag = RwSignal::new(0i32);
    let dragging = RwSignal::new(false);
    let down = move |ev: leptos::ev::PointerEvent| {
        carried.set_value(false);
        start.set_value(Some((ev.client_x(), ev.client_y())));
    };
    let moving = move |ev: leptos::ev::PointerEvent| {
        let Some((x, y)) = start.get_value() else { return };
        let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
        if !dragging.get_untracked() {
            if dx.abs() < DRAG || dx.abs() <= dy.abs() {
                return;
            }
            carried.set_value(true);
            dragging.set(true);
        }
        let now = current.get_untracked();
        let beyond = (dx > 0 && now == 0) || (dx < 0 && now + 1 >= WEEKS.len());
        drag.set(if beyond { dx / 3 } else { dx });
    };
    let up = move |ev: leptos::ev::PointerEvent| {
        let Some((x, y)) = start.get_value() else { return };
        start.set_value(None);
        dragging.set(false);
        drag.set(0);
        let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
        if dx.abs() > SWIPE && dx.abs() > dy.abs() {
            carried.set_value(true);
            let now = current.get_untracked();
            go(if dx < 0 { now + 1 } else { now.wrapping_sub(1) });
        }
    };
    let cancel = move |_| {
        start.set_value(None);
        dragging.set(false);
        drag.set(0);
    };
    let click = move |ev: leptos::ev::MouseEvent| {
        if carried.get_value() {
            carried.set_value(false);
            ev.prevent_default();
            return;
        }
        remember(Within::Grid, &ev);
    };
    // A week of the carousel, named by `label`; the grid alone has no name but the page's.
    let slide = move |at: Signal<i32>, label: Option<&'static str>, slots: Memo<Vec<PlanSlot>>| {
        view! {
            <div
                class="sp-slide"
                class:is-current=move || at.get() == 0
                class:is-side=move || at.get().abs() == 1
                class:is-far=move || { at.get().abs() > 1 }
                style=move || format!("--at:{};--drag:{}px", at.get(), drag.get())
                role=label.map(|_| "group")
                aria-roledescription=label.map(|_| t.studyplan_week.week_role)
                aria-label=label
                inert=move || (at.get() != 0).then_some("")
            >
                <div class="sp-week">
                    <WeekGrid slots=grid(slots) fit=true/>
                </div>
            </div>
        }
    };
    move || {
        if !ab.get() {
            // One week: the grid alone.
            return view! {
                <div class="sp-carousel" on:click=move |ev| remember(Within::Grid, &ev)>
                    {slide(Signal::stored(0), None, shown)}
                </div>
            }
            .into_any();
        }
        let slides = WEEKS
            .into_iter()
            .zip(weeks)
            .enumerate()
            .map(|(i, (week, (_, slots)))| {
                // Its place counted from the week shown: 0 in the middle, ±1 at the sides, ±2 out of
                // sight at the side it lies on.
                let at = Signal::derive(move || place_of(i, current.get()));
                slide(at, Some(week_label(week, t)), slots)
            })
            .collect_view();
        view! {
            // A link dragged with a mouse would leave the page's hands (the browser's own drag
            // cancels the pointer): the slots are not dragged.
            <div
                class="sp-carousel"
                class:dragging=move || dragging.get()
                aria-roledescription=t.studyplan_week.carousel_role
                aria-label=t.studyplan_week.weeks
                on:pointerdown=down
                on:pointermove=moving
                on:pointerup=up
                on:pointerleave=up
                on:pointercancel=cancel
                on:click=click
                on:dragstart=|ev| ev.prevent_default()
            >
                {slides}
            </div>
            // The tabs name the weeks; the mark of the one shown slides to it (`--i`).
            <div class="seg sp-weektabs" role="radiogroup" aria-label=t.studyplan_week.week style=move || format!("--i:{}", current.get())>
                <i class="sp-weekmark" aria-hidden="true"></i>
                {WEEKS
                    .into_iter()
                    .enumerate()
                    .map(|(i, week)| {
                        view! {
                            <button type="button" role="radio" aria-checked=move || if current.get() == i { "true" } else { "false" } on:click=move |_| ctx.weeks.set(week)>
                                {week_label(week, t)}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    }
}

/// The weeks a Regelwoche can show, in the order of the head's switch and of a phone's carousel.
const WEEKS: [Weeks; 3] = [Weeks::A, Weeks::B, Weeks::All];

/// A week's name on the head's switch and a phone's tab: „A-Woche", „B-Woche", „A/B".
fn week_label(week: Weeks, t: &'static i18n::Texts) -> &'static str {
    match week {
        Weeks::A => t.studyplan_week.tab_a,
        Weeks::B => t.studyplan_week.tab_b,
        Weeks::All => t.studyplan_week.tab_ab,
    }
}

/// Where a slot of the plan leads, as the grid writes the link: in the page's language
/// (`PlanSlot` keeps the app's path, which the page's other lines and the tests read).
fn linked(slot: &GridSlot, t: &i18n::Texts) -> Option<String> {
    slot.href.as_deref().map(|href| t.path(href))
}

/// How far a finger moves sideways, in pixels, before it carries the carousel's weeks, and how far
/// it carries them before letting go brings another week.
const DRAG: i32 = 8;
const SWIPE: i32 = 40;

/// The place of `week` among `WEEKS`.
fn week_at(week: Weeks) -> usize {
    WEEKS.iter().position(|shown| *shown == week).unwrap_or(WEEKS.len() - 1)
}

/// Where the slide `i` of the carousel stands while the one at `current` is shown: 0 in the
/// middle, ±1 at its sides, ±2 for any further out, out of sight on the side it lies on.
fn place_of(i: usize, current: usize) -> i32 {
    let signed = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    (signed(i) - signed(current)).clamp(-2, 2)
}

/// „Ohne feste Zeit": what of the Regelwoche has no fixed time, once for the semester. It stands
/// under the fold, so that the week keeps the room above it.
#[component]
pub(super) fn WeekLoose(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let base = base_of(ctx);
    let loose = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| loose_lines(table, &base, &table.loose(), t)).unwrap_or_default())
    });
    view! { <Loose title=t.studyplan_week.no_fixed_time lines=loose/> }
}

/// „A-Woche · B-Woche · A/B": which kind of week the Regelwoche shows. The click answers at once
/// (R21): a signal of the page, the week is worked out again in Rust.
#[component]
pub(super) fn WeekSwitch(weeks: RwSignal<Weeks>) -> impl IntoView {
    let t = i18n::t();
    let choice = move |week: Weeks| {
        view! {
            <button type="button" role="radio" aria-checked=move || if weeks.get() == week { "true" } else { "false" } on:click=move |_| weeks.set(week)>
                {week_label(week, t)}
            </button>
        }
    };
    view! {
        <div class="seg sp-weeks" role="radiogroup" aria-label=t.studyplan_week.week>
            {WEEKS.into_iter().map(choice).collect_view()}
        </div>
    }
}

/// „Plan · Alle Termine": the Regelwoche of the plan, or every Termin of its modules, what the plan
/// leaves out faint (owner, 2026-09-25: „zwischen der tatsächlichen Ansicht und allen zu den
/// Modulen gehörenden Terminen wechseln"). A signal of the page, as the switch of the weeks (R21).
#[component]
pub(super) fn AllSwitch(all: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let choice = move |(value, label): (bool, &'static str)| {
        view! {
            <button type="button" role="radio" aria-checked=move || if all.get() == value { "true" } else { "false" } on:click=move |_| all.set(value)>
                {label}
            </button>
        }
    };
    view! {
        <div class="seg sp-all" role="radiogroup" aria-label=t.studyplan_week.sessions_switch>
            {[(false, t.studyplan_week.plan), (true, t.studyplan_week.all_sessions)].into_iter().map(choice).collect_view()}
        </div>
    }
}

/// The key of the slot button a click hit (`button.slot-act[data-act]`); `None` for a click
/// anywhere else.
fn act_under(target: Option<leptos::web_sys::EventTarget>) -> Option<String> {
    use leptos::wasm_bindgen::JsCast;
    let element = target?.dyn_into::<leptos::web_sys::Element>().ok()?;
    element.closest(".slot-act").ok()??.get_attribute("data-act")
}

/// What the button `key` does: `<its slot's key>/<its place among the slot's buttons>` (`keyed`).
fn act_of(slots: &[PlanSlot], key: &str) -> Option<Act> {
    let (slot, at) = key.rsplit_once('/')?;
    let at: usize = at.parse().ok()?;
    slots.iter().find(|entry| entry.key == slot)?.acts.get(at).map(|act| act.act.clone())
}

/// Whether a shown Termin of the plan is held in A or B weeks only: then the week can show one
/// kind of week, and the agenda names its weeks.
pub(super) fn has_ab(table: &Timetable) -> bool {
    table
        .events
        .iter()
        .filter(|event| event.hidden.is_none())
        .flat_map(|event| event.rows.iter().filter(|row| row.hidden.is_none()))
        .any(|row| matches!(Every::of(&row.date), Some(Every::AWeek | Every::BWeek)))
}

/// „Termine": the agenda by week, opened at the current one, and what has no date.
#[component]
pub(super) fn DatesView(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let base = base_of(ctx);
    let today = ctx.today;
    let blocks = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| agenda_blocks(table, &table.agenda(t.locale), &base, today, t)).unwrap_or_default())
    });
    let loose = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| {
            table
                .as_ref()
                .map(|table| loose_lines(table, &base, &undated(table), t).into_iter().chain(open_exams(table, &base, t)).collect())
                .unwrap_or_default()
        })
    });
    // The current week, as the agenda has it now; `None` without a clock.
    let current = move |blocks: &[Block]| today.and_then(|today| scroll_target(blocks, today));

    // The agenda opens once, when it first has something to show: at the current week, or back
    // from a module that filled the page (the address names it still) at the date it was opened
    // from. Later changes of what is shown leave the page where the visitor put it. On a phone,
    // while the module beside the plan is the page, the plan is not shown: it opens when the
    // module is closed (`back_on_phone`).
    let done = StoredValue::new(false);
    Effect::new(move |_| {
        if done.get_value() {
            return;
        }
        let Some(week) = blocks.with(|blocks| (!blocks.is_empty()).then(|| current(blocks))) else { return };
        let here = ctx.url.get_untracked();
        if here.open.is_some() && nav::is_phone() {
            return;
        }
        done.set_value(true);
        match (&here.open, week) {
            (Some(_), week) => come_back(&here, week, t.locale),
            (None, Some(week)) => reveal(week),
            (None, None) => {}
        }
    });
    back_on_phone(ctx, t.locale, move || {
        done.set_value(true);
        blocks.with_untracked(|blocks| current(blocks))
    });

    view! {
        <div class="agenda">
            <For each=move || blocks.get() key=|block| block.clone() children=move |block| block_view(block, t)/>
        </div>
        <Loose title=t.studyplan_week.no_date lines=loose/>
    }
}

/// The address of the view without the module beside it: what every link of a view starts from.
/// A sibling of the timetable (both follow the address), never read with the address itself (R16).
fn base_of(ctx: PlanCtx) -> Memo<StudyplanUrl> {
    Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)))
}

/// The module and Termin beside the plan, or where a click is taking them: marked in the next
/// frame (R21), before the router has the address.
fn picked_of(ctx: PlanCtx) -> Memo<Picked> {
    let going = Pending::expect();
    Memo::new(move |_| {
        let (open, row) = match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
            Some(search) => {
                let to = StudyplanUrl::parse(&search);
                (to.open, to.row)
            }
            None => ctx.url.with(|url| (url.open.clone(), url.row.clone())),
        };
        (open, row.as_deref().and_then(RowKey::parse))
    })
}

/// Scrolls the week `id` to the top a frame later: after the router has put a page it opened at
/// its top.
#[allow(unused_variables)]
fn reveal(id: String) {
    #[cfg(feature = "csr")]
    request_animation_frame(move || {
        week_to_top(&id);
    });
}

/// Scrolls the week `id` to the top of what scrolls around it (the page's column, on a phone the
/// window). `false` if the agenda has no such week.
#[allow(unused_variables)]
fn week_to_top(id: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return false };
        let Some(document) = window.document() else { return false };
        let Some(element) = document.get_element_by_id(id) else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        // On a phone the window scrolls under the top bar, which stays: the week goes below it.
        if nav::is_phone() {
            if let Some(bar) = document.query_selector(".topbar").ok().flatten() {
                window.scroll_by_with_x_and_y(0.0, -bar.get_bounding_client_rect().bottom());
            }
        }
        true
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// On a phone the module beside the plan is the page, and the plan waits unseen (with the window
/// scrolled for the module). Closing the module brings the view back where the visitor left it
/// (`come_back`, the page's links in `locale`), else at `week`.
fn back_on_phone(ctx: PlanCtx, locale: Locale, week: impl Fn() -> Option<String> + 'static) {
    Effect::new(move |before: Option<StudyplanUrl>| {
        let here = ctx.url.get();
        if let Some(left) = before.filter(|before| before.open.is_some()) {
            if here.open.is_none() && nav::is_phone() {
                come_back(&left, week(), locale);
            }
        }
        here
    });
}

/// Scrolls back to where the visitor left `left` (an address with the module beside the view):
/// to the link they opened, else to its week, else to `week`. Once in the next frame and once more
/// after the browser has restored its own idea of the scroll position (a step back through the
/// history), as the Merkliste does. `locale`: the language of the page's links.
fn come_back(left: &StudyplanUrl, week: Option<String>, locale: Locale) {
    let place = nav::session_get(LEFT_KEY).as_deref().and_then(Place::restored).filter(|place| place.left(left));
    let pass = move || {
        if let Some(place) = &place {
            if nav::reveal_selector(&place.selector(locale)) || place.week().is_some_and(week_to_top) {
                return;
            }
        }
        if let Some(week) = &week {
            week_to_top(week);
        }
    };
    #[cfg(feature = "csr")]
    {
        let again = pass.clone();
        request_animation_frame(pass);
        set_timeout(again, std::time::Duration::from_millis(220));
    }
    // The server renders no plan and scrolls nothing.
    #[cfg(not(feature = "csr"))]
    let _ = pass;
}

/// What holds a link a view is left by: a week of the agenda (its anchor), the phone's list of
/// days, the phone's grid of the week, the lines without a fixed time or date.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Within {
    Week(String),
    Days,
    Grid,
    Loose,
}

/// The link the visitor last opened a module by, and what holds it: where the view comes back to
/// (`come_back`). Kept for the browser tab (`LEFT_KEY`), since a view that the module's whole page
/// replaced is built anew. `href` is the app's path (`/studyplan?…`), whatever the page's language:
/// the language changes only with a new page, and the tab keeps what the site keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    within: Within,
    href: String,
}

impl Place {
    /// „kw-2026-50 /studyplan?…", „days /studyplan?…", „grid /studyplan?…", „loose /studyplan?…".
    fn stored(&self) -> String {
        let within = match &self.within {
            Within::Week(id) => id.as_str(),
            Within::Days => "days",
            Within::Grid => "grid",
            Within::Loose => "loose",
        };
        format!("{within} {}", self.href)
    }

    /// What `stored` wrote, checked like anything read from storage: an anchor as the agenda
    /// writes it, and a link of the plan made of the characters its addresses have (it becomes
    /// part of a selector).
    fn restored(text: &str) -> Option<Place> {
        let (within, href) = text.split_once(' ')?;
        let within = match within {
            "days" => Within::Days,
            "grid" => Within::Grid,
            "loose" => Within::Loose,
            id if is_week_id(id) => Within::Week(id.to_string()),
            _ => return None,
        };
        let plain = |c: char| c.is_ascii_alphanumeric() || "/?&=-_.%+".contains(c);
        let ours = href.strip_prefix(url::STUDYPLAN).is_some_and(|rest| rest.starts_with('?')) && href.chars().all(plain);
        ours.then(|| Place { within, href: href.to_string() })
    }

    /// What the page shows again: the link, `#kw-2026-50 a[href="…"]`, `.sp-daylist a[href="…"]`,
    /// as a page in `locale` writes it (`/en/studyplan?…`); of the phone's grid the whole week it
    /// was on (a slot in its middle would leave the rest of the week above or under the screen).
    fn selector(&self, locale: Locale) -> String {
        let within = match &self.within {
            Within::Week(id) => format!("#{id}"),
            Within::Days => ".sp-daylist".to_string(),
            Within::Grid => return ".sp-carousel".to_string(),
            Within::Loose => ".sp-loose".to_string(),
        };
        format!("{within} a[href=\"{}\"]", locale.path(&self.href))
    }

    fn week(&self) -> Option<&str> {
        match &self.within {
            Within::Week(id) => Some(id),
            Within::Days | Within::Grid | Within::Loose => None,
        }
    }

    /// Whether the visitor left `url` here: the same semester and view, the same module beside it.
    /// (Another module opened since, from the legend or the notes, is not left from here.)
    fn left(&self, url: &StudyplanUrl) -> bool {
        let there = StudyplanUrl::parse(self.href.split_once('?').map_or("", |(_, query)| query));
        (&there.sem, there.view, &there.open) == (&url.sem, url.view, &url.open)
    }
}

/// `kw-2026-41`, as `week_id` writes it.
fn is_week_id(id: &str) -> bool {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    id.strip_prefix("kw-").and_then(|rest| rest.split_once('-')).is_some_and(|(year, week)| digits(year) && digits(week))
}

/// Remembers the link a click in `within` follows (`Place`), as the app's path: the page writes it
/// in its language (`/en/studyplan?…`).
fn remember(within: Within, ev: &leptos::ev::MouseEvent) {
    if let Some(href) = nav::link_under(ev.target()) {
        if let Some(path) = i18n::app_path(&href) {
            nav::session_set(LEFT_KEY, &Place { within, href: path.to_string() }.stored());
        }
    }
}

// ---------- the Regelwoche ----------

/// A slot of the Regelwoche: as the grid draws it, and as a row of the phone's list of days; the
/// key its buttons are known by, and what they do.
#[derive(Clone, Debug, PartialEq)]
struct PlanSlot {
    /// Never `current`: the page marks the slot from the address.
    slot: GridSlot,
    row: DayRow,
    key: String,
    acts: Vec<SlotAct>,
    /// The weeks of the A/B rhythm it is held in; `None` for dates that do not recur.
    weeks: Option<Weeks>,
}

/// A slot as a row of the list of days: the time, then what it is and what else to know.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DayRow {
    day: u8,
    /// The tone, and `alt` and `clash` as in the grid.
    class: String,
    /// „15:30–17:00"
    time: String,
    /// „Übung · Entwicklung von Softwaresystemen"
    text: String,
    /// „1 von 4", „A-Woche · bis 23.11.", „3 Termine"; empty when there is nothing to add.
    note: String,
    /// The app's path of its link; the page writes it in its language.
    href: String,
    /// The planned modules of its event and its Termine: what the address's `open` and `row` name.
    modules: Vec<String>,
    keys: Vec<RowKey>,
    /// Its buttons, as the grid's slot has them.
    acts: Vec<SlotButton>,
}

impl DayRow {
    /// Whether this is the Termin beside the plan: its module is open and the address points at
    /// one of its rows.
    fn is(&self, (open, row): &Picked) -> bool {
        match (open, row) {
            (Some(open), Some(row)) => self.modules.contains(open) && self.keys.contains(row),
            _ => false,
        }
    }
}

/// The slots of the Regelwoche held in the week `shown`, in its order (weekday, time, event);
/// `titles` are the planned modules' titles by id. With `all` („Alle Termine"), what the plan
/// leaves out besides (`ghost_slots`), in the order of the week. In the words of `t`.
fn week_slots(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks, all: bool, t: &i18n::Texts) -> Vec<PlanSlot> {
    let items = table.regular_week();
    let mut slots: Vec<PlanSlot> = items.iter().filter(|item| item.in_week(shown)).filter_map(|item| plan_slot(table, base, item, titles, shown, &items, t)).collect();
    if all {
        slots.extend(ghost_slots(table, base, titles, shown, &items, t));
        slots.sort_by_key(|slot| (slot.slot.day, slot.slot.from, slot.slot.to));
    }
    join_ab(slots)
}

/// A module's slots that clash together (owner, 2026-09-25: „VL A und Ü B"): one held in A weeks
/// and one in B weeks, of one module, on one day at times that meet, both in a clash. They never
/// meet each other and compete together with the other, so the grid draws one red frame around
/// them (`GridSlot::joint`); each second one is put right after its first, so that they get lanes
/// side by side. What the plan leaves out stands with none.
fn join_ab(slots: Vec<PlanSlot>) -> Vec<PlanSlot> {
    let together = |a: &PlanSlot, b: &PlanSlot| {
        let ab = matches!((a.weeks, b.weeks), (Some(Weeks::A), Some(Weeks::B)) | (Some(Weeks::B), Some(Weeks::A)));
        let clash = a.slot.clash && b.slot.clash && a.slot.class != "faint" && b.slot.class != "faint";
        let meet = a.slot.day == b.slot.day && a.slot.from < b.slot.to && b.slot.from < a.slot.to;
        ab && clash && meet && a.row.modules.first().is_some_and(|module| b.row.modules.first() == Some(module))
    };
    // Per slot its joint and the place of its first slot.
    let mut joints: Vec<Option<(u32, usize)>> = vec![None; slots.len()];
    let mut next = 0;
    for (i, a) in slots.iter().enumerate() {
        for (j, b) in slots.iter().enumerate().skip(i + 1) {
            if matches!((joints.get(i), joints.get(j)), (Some(None), Some(None))) && together(a, b) {
                for at in [i, j] {
                    if let Some(joint) = joints.get_mut(at) {
                        *joint = Some((next, i));
                    }
                }
                next += 1;
            }
        }
    }
    let mut placed: Vec<(usize, PlanSlot)> = slots
        .into_iter()
        .zip(joints)
        .enumerate()
        .map(|(i, (mut slot, joint))| {
            slot.slot.joint = joint.map(|(id, _)| id);
            (joint.map_or(i, |(_, first)| first), slot)
        })
        .collect();
    placed.sort_by_key(|(first, slot)| (slot.slot.day, slot.slot.from, slot.slot.to, *first));
    placed.into_iter().map(|(_, slot)| slot).collect()
}

/// A slot of the plan: as `drawn_slot` draws it, red where it overlaps another in the week shown,
/// with its menu (`plan_acts`). `items` is the plan's whole Regelwoche, which says how many slots
/// the slot's event has.
fn plan_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem, titles: &BTreeMap<String, String>, week: Weeks, items: &[WeekItem], t: &i18n::Texts) -> Option<PlanSlot> {
    let mut slot = drawn_slot(table, base, item, titles, &item.against_in(week), t)?;
    let slots = items.iter().filter(|other| other.event == item.event).count();
    slot.acts = plan_acts(table, item, &slot.row.keys, slots, t);
    // A slot with a „✓" stands with its buttons in view: an open choice asks for a decision, and
    // the option it took can be taken back at once.
    slot.slot.asks = slot.acts.iter().any(|act| act.icon == "check");
    Some(keyed(slot, 'p', item))
}

/// A slot with its key, `p…` of the plan or `g…` of what it leaves out (its event, row, day and
/// start), and its buttons, each `<key>/<n>`, in the grid and in the line of a phone.
fn keyed(mut slot: PlanSlot, kind: char, item: &WeekItem) -> PlanSlot {
    slot.key = format!("{kind}{}-{}-{}-{}", item.event, item.row, item.day, item.from);
    let buttons: Vec<SlotButton> =
        slot.acts.iter().enumerate().map(|(n, act)| SlotButton { icon: act.icon, label: act.label.clone(), key: format!("{}/{n}", slot.key), pressed: act.pressed }).collect();
    slot.row.acts = buttons.clone();
    slot.slot.acts = buttons;
    slot
}

/// A slot as the grid draws it and a phone lists it, without a key or buttons; `against` are the
/// events it overlaps, which draw it red and which its tooltip names. Its link is the app's path
/// (`linked` writes it in the page's language).
fn drawn_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem, titles: &BTreeMap<String, String>, against: &[usize], t: &i18n::Texts) -> Option<PlanSlot> {
    let event = table.events.get(item.event)?;
    let shown = event.rows.get(item.row)?;
    let module = event.modules.first()?;
    let every = Every::of(&shown.date);
    let once = matches!(item.label, WeekLabel::Once { .. });
    let rows: Vec<&Row> = slot_rows(event, item, every).into_iter().filter_map(|r| event.rows.get(r)).collect();
    let notes = qualifiers(&item.label, every, item.alt, &table.facts, t);
    let hue = hue(event.tone);
    let href = base.with_open(Some(module.as_str()), shown.key).path();
    let place = rooms(rows.iter().map(|row| &row.date));

    // The tooltip says in full what the slot shortens: the kind as QIS names it, the whole time,
    // the rooms, and the dates of single ones.
    let mut title = vec![event_text(event, t.locale), format!("{} {}–{}", day_short(item.day, t), clock(item.from), clock(item.to))];
    title.extend(notes.iter().cloned());
    title.extend(rooms_long(rows.iter().map(|row| &row.date)));
    if matches!(item.label, WeekLabel::Once { dates, .. } if dates > 1) {
        let mut days: Vec<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).filter(|day| day.weekday() == item.day).collect();
        days.sort_unstable();
        days.dedup();
        title.push(days.iter().map(|day| day.date(t.locale)).collect::<Vec<_>>().join(", "));
    }
    // What it overlaps, by the other slot's label.
    let mut others: Vec<String> = Vec::new();
    for other in against.iter().filter_map(|e| table.events.get(*e)) {
        let label = slot_label(other, titles, t.locale);
        if !others.contains(&label) {
            others.push(label);
        }
    }
    let clash = !others.is_empty();
    if clash {
        title.push((t.studyplan_week.clashes_with)(&others.join(", ")));
    }

    let slot = GridSlot {
        day: item.day,
        from: item.from,
        to: item.to,
        label: slot_label(event, titles, t.locale),
        small: slot_small(&item.label, every, item.from, item.to, t),
        title: title.join(" · "),
        place: place.clone().unwrap_or_default(),
        class: if once { "once" } else { "tinted" },
        hue: Some(hue),
        alt: item.alt.is_some(),
        clash,
        href: Some(href.clone()),
        current: false,
        acts: Vec::new(),
        asks: false,
        joint: None,
    };
    let row = DayRow {
        day: item.day,
        class: classes(hue, &[(item.alt.is_some(), "alt"), (clash, "clash")]),
        time: format!("{}–{}", clock(item.from), clock(item.to)),
        text: event_text(event, t.locale),
        note: place.into_iter().chain(notes).collect::<Vec<_>>().join(" · "),
        href,
        modules: event.modules.clone(),
        keys: rows.iter().filter_map(|row| row.key).collect(),
        acts: Vec::new(),
    };
    Some(PlanSlot { slot, row, key: String::new(), acts: Vec::new(), weeks: item.weeks })
}

/// „Alle Termine": the slots of what the plan leaves out, held in the week `shown` — a hidden
/// event, kind or Termin, an option another was chosen over (the other town's course stays out).
/// Each is faint (`faint`) and says why in its tooltip; it is red-edged where it would meet a slot
/// of the plan (`items`, the plan's Regelwoche) in a week both are held in, and its „✓", in view,
/// takes it back (`ghost_acts`).
fn ghost_slots(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks, items: &[WeekItem], t: &i18n::Texts) -> Vec<PlanSlot> {
    let all = unhidden(table);
    let plan: Vec<&WeekItem> = items.iter().filter(|item| item.in_week(shown)).collect();
    all.regular_week()
        .iter()
        .filter(|item| item.in_week(shown))
        .filter_map(|item| {
            let event = all.events.get(item.event)?;
            let every = Every::of(&event.rows.get(item.row)?.date);
            // Left out: every Termin of the slot is hidden in the plan. Why the first one is.
            let hidden = table.events.get(item.event)?;
            let reasons: Vec<HiddenBy> = slot_rows(event, item, every).into_iter().map(|r| hidden.rows.get(r)?.hidden).collect::<Option<_>>()?;
            let reason = *reasons.first()?;
            let meets: Vec<usize> = plan
                .iter()
                .filter(|other| other.event != item.event && other.day == item.day && other.from < item.to && item.from < other.to)
                .filter(|other| same_weeks(other.weeks, item.weeks))
                .map(|other| other.event)
                .collect();
            let mut slot = drawn_slot(&all, base, item, titles, &meets, t)?;
            let why = reason_text(reason, hidden, t);
            let hue = slot.slot.hue.unwrap_or_default();
            slot.slot.class = "faint";
            slot.slot.alt = false;
            slot.slot.asks = true;
            slot.slot.title = format!("{why} · {}", slot.slot.title);
            slot.row.class = classes(hue, &[(true, "faint"), (slot.slot.clash, "clash")]);
            slot.row.note = std::iter::once(why).chain(Some(slot.row.note.clone()).filter(|note| !note.is_empty())).collect::<Vec<_>>().join(" · ");
            slot.acts = ghost_acts(hidden, item, &slot.row.keys, reason, t);
            Some(keyed(slot, 'g', item))
        })
        .collect()
}

/// The timetable with what the plan hides shown again, but the other town's course: every Termin
/// of the planned modules, as „Alle Termine" draws them. Its events and rows are the plan's, at
/// the same places.
fn unhidden(table: &Timetable) -> Timetable {
    let town = |hidden: Option<HiddenBy>| hidden.filter(|by| matches!(by, HiddenBy::Town(_)));
    let mut all = table.clone();
    for event in &mut all.events {
        event.hidden = town(event.hidden);
        event.chosen = None;
        for row in &mut event.rows {
            row.hidden = town(row.hidden);
        }
    }
    all
}

/// Whether two slots can meet: not one of A weeks and one of B weeks (`None`: single dates).
fn same_weeks(a: Option<Weeks>, b: Option<Weeks>) -> bool {
    !matches!((a, b), (Some(Weeks::A), Some(Weeks::B)) | (Some(Weeks::B), Some(Weeks::A)))
}

/// Why the plan leaves a slot out, first in its tooltip and its line: „ausgeblendet",
/// „„Tutorium“ ausgeblendet", „andere Gruppe gewählt".
fn reason_text(reason: HiddenBy, event: &Event, t: &i18n::Texts) -> String {
    let words = &t.studyplan_week;
    match reason {
        HiddenBy::Kinds => (words.kinds_hidden)(&event.kinds.iter().map(|kind| kind.label(t.locale)).collect::<Vec<_>>().join("/")),
        HiddenBy::Choice => words.other_group_chosen.to_string(),
        HiddenBy::Event | HiddenBy::Row | HiddenBy::Town(_) => words.hidden.to_string(),
    }
}

// ---------- the buttons of a slot ----------

/// A button of a slot (owner, 2026-09-25: „ja der fliegt raus, die Übung möchte ich", on the
/// Termin itself): „✓" takes, „×" leaves out; its words (its name and tooltip) say what. A „✓"
/// that is on (`pressed`) belongs to what was taken, and takes it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SlotAct {
    icon: &'static str,
    label: String,
    act: Act,
    pressed: bool,
}

impl SlotAct {
    /// „✓": takes the slot.
    fn take(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "check", label: label.into(), act, pressed: false }
    }

    /// „✓", on: the slot was taken, and the click takes it back.
    fn taken(label: impl Into<String>, act: Act) -> Self {
        SlotAct { pressed: true, ..SlotAct::take(label, act) }
    }

    /// „×": leaves it out.
    fn leave(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "x", label: label.into(), act, pressed: false }
    }
}

/// What a button of a slot does to what the semester shows (the plan's `Selection`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Act {
    /// Hides the slot's Termine.
    HideRows(Vec<RowKey>),
    /// Hides the whole event.
    HideEvent(u32),
    /// Takes the slot's option of the event's choice („Nur diesen", `row` a Termin of it): the
    /// event and the option's Termine shown again where they were hidden.
    Choose { event: u32, row: RowKey, rows: Vec<RowKey> },
    /// Takes the choice back: every option shown again, the one it took among them.
    Unchoose(u32),
    /// Leaves out the option a choice took: the choice is open again, without it.
    Drop { event: u32, rows: Vec<RowKey> },
    /// Shows the slot again: its event, a kind of it, its Termine.
    Show { event: Option<u32>, kind: Option<EventKind>, rows: Vec<RowKey> },
}

impl Act {
    /// Makes the change in the plan, for the semester `key`.
    fn apply(&self, doc: &mut PlanDoc, key: SemesterKey) {
        match self {
            Act::HideRows(rows) => rows.iter().for_each(|row| doc.set_row(key, *row, true)),
            Act::HideEvent(event) => doc.set_event(key, *event, true),
            Act::Choose { event, row, rows } => {
                doc.set_event(key, *event, false);
                rows.iter().for_each(|row| doc.set_row(key, *row, false));
                doc.choose(key, *event, Some(*row));
            }
            Act::Unchoose(event) => doc.choose(key, *event, None),
            Act::Drop { event, rows } => {
                doc.choose(key, *event, None);
                rows.iter().for_each(|row| doc.set_row(key, *row, true));
            }
            Act::Show { event, kind, rows } => {
                if let Some(event) = event {
                    doc.set_event(key, *event, false);
                }
                if let Some(kind) = kind {
                    doc.set_kind(key, *kind, false);
                }
                rows.iter().for_each(|row| doc.set_row(key, *row, false));
            }
        }
    }
}

/// The buttons of a slot of the plan. An option of an open choice: „✓" takes it, „×" leaves it
/// out. The option a made choice took: its „✓", on, takes the choice back (owner, 2026-09-25:
/// „schnell abwählen"), its „×" leaves it out, and the choice is open again without it. Any other
/// Termin: „×" leaves it out, and an event of one slot (`slots`) as a whole. `keys` are the slot's
/// Termine.
fn plan_acts(table: &Timetable, item: &WeekItem, keys: &[RowKey], slots: usize, t: &i18n::Texts) -> Vec<SlotAct> {
    let words = &t.studyplan_week;
    let Some(event) = table.events.get(item.event) else { return Vec::new() };
    let Ok(id) = event.id.parse::<u32>() else { return Vec::new() };
    let shown = event.rows.get(item.row);
    let (option, row) = (shown.and_then(|row| row.option), shown.and_then(|row| row.key));
    match (item.alt, row) {
        (Some(_), Some(row)) => vec![
            SlotAct::take(words.take_group, Act::Choose { event: id, row, rows: keys.to_vec() }),
            SlotAct::leave(words.not_this_group, Act::HideRows(keys.to_vec())),
        ],
        _ if option.is_some() && event.chosen == option => vec![
            SlotAct::taken(words.drop_group, Act::Unchoose(id)),
            SlotAct::leave(words.not_this_group, Act::Drop { event: id, rows: keys.to_vec() }),
        ],
        _ if slots > 1 && !keys.is_empty() => vec![SlotAct::leave(words.hide_session, Act::HideRows(keys.to_vec()))],
        _ => vec![SlotAct::leave((words.hide_event)(&kind_word(event, t.locale)), Act::HideEvent(id))],
    }
}

/// The button of a slot the plan leaves out: „✓" takes it back. An option of a choice is taken
/// („Diese Gruppe nehmen"); a kind switched off comes back with every event of it („„Übung“ wieder
/// einblenden"); any other Termin is shown again, its event with it. `event` is the plan's, `keys`
/// the slot's Termine.
fn ghost_acts(event: &Event, item: &WeekItem, keys: &[RowKey], reason: HiddenBy, t: &i18n::Texts) -> Vec<SlotAct> {
    let words = &t.studyplan_week;
    let id = event.id.parse::<u32>().ok();
    let shown = event.rows.get(item.row);
    let (option, row) = (shown.and_then(|row| row.option), shown.and_then(|row| row.key));
    let act = match (reason, id, row) {
        (HiddenBy::Kinds, _, _) => {
            let kind = event.kinds.iter().next();
            SlotAct::take((words.show_kind)(kind.map_or("", |kind| kind.label(t.locale))), Act::Show { event: None, kind, rows: keys.to_vec() })
        }
        (_, Some(number), Some(row)) if option.is_some() => SlotAct::take(words.take_group, Act::Choose { event: number, row, rows: keys.to_vec() }),
        _ => SlotAct::take(words.show_again, Act::Show { event: id.filter(|_| reason == HiddenBy::Event), kind: None, rows: keys.to_vec() }),
    };
    vec![act]
}

/// The rows behind a slot of the Regelwoche: the shown rows of its event at its weekday and
/// time, of its option and its rhythm (one slot in two rooms, or in two ranges one after the
/// other, as `regular_week` merges them).
fn slot_rows(event: &Event, item: &WeekItem, every: Option<Every>) -> Vec<usize> {
    let option = event.rows.get(item.row).and_then(|row| row.option);
    event
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.hidden.is_none() && row.from == Some(item.from) && row.to == Some(item.to) && row.option == option)
        .filter(|(_, row)| Every::of(&row.date) == every)
        .filter(|(_, row)| match every {
            Some(_) => row.occ.template.map(|pattern| pattern.weekday).or_else(|| row.occ.days.first().map(|day| day.weekday())) == Some(item.day),
            None => row.occ.days.iter().any(|day| day.weekday() == item.day),
        })
        .map(|(r, _)| r)
        .collect()
}

/// What a slot says besides its time: which option of a choice not yet made, the weeks it meets
/// in, the part of the lecture period, or how many single dates it gathers.
fn qualifiers(label: &WeekLabel, every: Option<Every>, alt: Option<(usize, usize)>, facts: &SemesterFacts, t: &i18n::Texts) -> Vec<String> {
    let words = &t.studyplan_week;
    let mut parts = Vec::new();
    if let Some((options, _)) = alt {
        parts.push((words.one_of)(options));
    }
    match label {
        WeekLabel::Every(every) => parts.extend(rhythm_word(*every, t)),
        WeekLabel::Partial { .. } => {
            parts.extend(every.and_then(|every| rhythm_word(every, t)));
            parts.extend(label.reach(facts).map(|reach| reach_text(reach, t)));
        }
        WeekLabel::Once { dates: 1, first } => parts.push(format!("{} · {}", (words.sessions)(1), first.day_month(t.locale))),
        WeekLabel::Once { dates, .. } => parts.push((words.sessions)(*dates)),
    }
    parts
}

/// The second line of a slot in the grid (owner, 2026-09-25: „<zeit> <A/B>"): its time, and the
/// weeks it meets in where that is not every week („07:30–09:00 A"); single dates say their day,
/// or how many they are. The rest of what `qualifiers` says (an option of a choice, a part of the
/// lecture period) is in the tooltip.
fn slot_small(label: &WeekLabel, every: Option<Every>, from: u16, to: u16, t: &i18n::Texts) -> String {
    let time = format!("{}–{}", clock(from), clock(to));
    let every = match label {
        WeekLabel::Every(every) => Some(*every),
        WeekLabel::Partial { .. } => every,
        WeekLabel::Once { dates: 1, first } => return format!("{time} · {}", first.day_month(t.locale)),
        WeekLabel::Once { dates, .. } => return format!("{time} · {}", (t.studyplan_week.sessions)(*dates)),
    };
    match every {
        Some(Every::AWeek) => format!("{time} A"),
        Some(Every::BWeek) => format!("{time} B"),
        Some(Every::FourWeeks) => format!("{time} · {}", t.studyplan_week.every_four_weeks),
        Some(Every::Week) | None => time,
    }
}

/// The weeks a Termin meets in, in words: „A-Woche", „B-Woche", „4-wöch."; nothing for every week.
fn rhythm_word(every: Every, t: &i18n::Texts) -> Option<String> {
    let words = &t.studyplan_week;
    match every {
        Every::Week => None,
        Every::AWeek => Some(words.in_week_a.to_string()),
        Every::BWeek => Some(words.in_week_b.to_string()),
        Every::FourWeeks => Some(words.every_four_weeks.to_string()),
    }
}

/// The part of the lecture period a Termin meets in: „bis 24.11.", „ab 07.12.", „26.10.–23.11.".
fn reach_text(reach: Reach, t: &i18n::Texts) -> String {
    let day = |day: Day| day.day_month(t.locale);
    match reach {
        Reach::Until(last) => (t.studyplan_week.until)(&day(last)),
        Reach::From(first) => (t.studyplan_week.from)(&day(first)),
        Reach::Between(first, last) => format!("{}–{}", day(first), day(last)),
    }
}

/// The rows of the list of days, one group per weekday in the order of the week; each day's rows
/// in the order they come (the Regelwoche's: by time).
fn day_groups(rows: impl IntoIterator<Item = DayRow>) -> Vec<(u8, Vec<DayRow>)> {
    let mut days: BTreeMap<u8, Vec<DayRow>> = BTreeMap::new();
    for row in rows {
        days.entry(row.day).or_default().push(row);
    }
    days.into_iter().collect()
}

/// The Regelwoche on a phone, under its grid: a list of days, each Termin a row as tall as a
/// finger, with its buttons.
#[component]
fn DayList(slots: Memo<Vec<PlanSlot>>, picked: Memo<Picked>) -> impl IntoView {
    let t = i18n::t();
    let groups = Memo::new(move |_| slots.with(|slots| day_groups(slots.iter().map(|slot| slot.row.clone()))));
    let day = move |(day, rows): (u8, Vec<DayRow>)| {
        let rows = rows
            .into_iter()
            .map(|row| {
                let marked = row.clone();
                let current = move || picked.with(|picked| marked.is(picked)).then_some("true");
                let note = (!row.note.is_empty()).then(|| view! { <small>{row.note}</small> });
                // The Termin's buttons beside its link, as in the grid, in view (a finger has no
                // pointer to show them).
                let count = row.acts.len();
                let act = (count > 0).then(|| view! { <div class="slot-acts">{slot_buttons(&row.acts)}</div> });
                view! {
                    <div class="sp-dayrow" style=format!("--acts:{count}")>
                        <a class=row.class href=t.path(&row.href) data-noscroll="" aria-current=current>
                            <span class="t">{row.time}</span>
                            <span>{row.text}{note}</span>
                        </a>
                        {act}
                    </div>
                }
            })
            .collect_view();
        view! {
            <section>
                <h3 class="label">{weekday_name(day, t)}</h3>
                {rows}
            </section>
        }
    };
    view! {
        <div class="sp-daylist" on:click=|ev| remember(Within::Days, &ev)>
            <For each=move || groups.get() key=|group| group.clone() children=day/>
        </div>
    }
}

// ---------- the agenda ----------

/// A part of the agenda as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Block {
    /// A week: its anchor (`kw-<year>-<week>`), its head, and the days with something in them;
    /// no day at all for a week of the lecture period with nothing in it.
    Week { id: String, head: String, days: Vec<DayLine>, from: Day, to: Day },
    /// Weeks of a break with nothing in them, as one line („21.12.–03.01. vorlesungsfrei"),
    /// anchored as the first of them.
    Break { id: String, text: String, from: Day, to: Day },
}

impl Block {
    fn id(&self) -> &str {
        match self {
            Block::Week { id, .. } | Block::Break { id, .. } => id,
        }
    }

    /// The Monday of its first week and the Sunday of its last.
    fn span(&self) -> (Day, Day) {
        match self {
            Block::Week { from, to, .. } | Block::Break { from, to, .. } => (*from, *to),
        }
    }
}

/// A day of the agenda: „Mo 05.10." (today marked), the holiday's name, the dates.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DayLine {
    when: String,
    today: bool,
    holiday: Option<&'static str>,
    items: Vec<ItemLine>,
}

/// A date of the agenda: the time, what it is, where (or why it does not take place).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ItemLine {
    /// The tone, and `alt`, `cancelled` or `exam`.
    class: String,
    /// „09:15–10:45", „ganztägig", „Zeit offen", „bis 19.02." (an exam's window or deadline).
    time: String,
    /// „Übung · Entwicklung von Softwaresystemen", „Prüfung · …"
    text: String,
    /// The rooms, a room note as QIS writes it, „fällt aus · <reason>", „2. Termin"; before them
    /// „1 von 4" for an option of a choice not yet made.
    small: String,
    title: String,
    /// The app's path of its link; the page writes it in its language.
    href: String,
    /// An exam the plan's exam warnings name.
    warn: bool,
}

/// The anchor of a week: `kw-2026-41`. The year belongs to it: rows can run for more than a year,
/// and then a week number comes twice (`AgendaWeek::iso_week`).
fn week_id((year, week): (i32, u8)) -> String {
    format!("kw-{year}-{week}")
}

/// „KW 41 · 05.–11.10.2026", „KW 44 · 26.10.–01.11.2026", „KW 53 · 28.12.2026–03.01.2027"
fn week_head(monday: Day, week: u8, t: &i18n::Texts) -> String {
    let words = &t.studyplan_week;
    (words.week_head)(week, &(words.days)(monday, monday.plus(6), true))
}

/// The agenda as the page shows it: a block per week, and the weeks of a break that hold nothing
/// (not even a single date; a holiday alone does not count) as one line. In the words of `t`.
fn agenda_blocks(table: &Timetable, weeks: &[AgendaWeek], base: &StudyplanUrl, today: Option<Day>, t: &i18n::Texts) -> Vec<Block> {
    let words = &t.studyplan_week;
    let mut blocks = Vec::new();
    let ab = has_ab(table);
    // The quiet weeks of a break not yet written: the first one's anchor and Monday, the last
    // one's Monday.
    let mut quiet: Option<(String, Day, Day)> = None;
    let flush = |quiet: &mut Option<(String, Day, Day)>, blocks: &mut Vec<Block>| {
        if let Some((id, from, last)) = quiet.take() {
            let to = last.plus(6);
            let days = format!("{}–{}", from.day_month(t.locale), to.day_month(t.locale));
            blocks.push(Block::Break { id, text: (words.break_line)(&days), from, to });
        }
    };
    for week in weeks {
        if week.break_week && week.days.iter().all(|day| day.items.is_empty()) {
            match &mut quiet {
                Some((_, _, last)) if last.plus(7) == week.monday => *last = week.monday,
                _ => {
                    flush(&mut quiet, &mut blocks);
                    quiet = Some((week_id(week.iso_week), week.monday, week.monday));
                }
            }
            continue;
        }
        flush(&mut quiet, &mut blocks);
        // An open choice is one line in the week, on the first day it could be attended, instead
        // of a line per option and day (owner review 2026-09-25).
        let choices = open_choices(table, week);
        // Per choice, the day its line stands on.
        let mut said: BTreeMap<usize, Day> = BTreeMap::new();
        let mut days = Vec::new();
        for day in &week.days {
            let is_today = today == Some(day.day);
            let mut items = Vec::new();
            for item in &day.items {
                let Some(e) = open_option(table, item) else {
                    items.extend(agenda_item(table, base, day.day, item, t));
                    continue;
                };
                let dates = choices.get(&e).map_or(&[][..], Vec::as_slice);
                // A cancelled date is no day to attend, unless the week has no other.
                if (item.cancelled.is_none() || dates.is_empty()) && !said.contains_key(&e) {
                    said.insert(e, day.day);
                    items.extend(choice_item(table, base, e, dates, t));
                } else if is_today && said.get(&e) != Some(&day.day) {
                    // Today keeps its line: the options that meet today stand as they are
                    // („1 von 4 · LG 10/214"), below the week's line on an earlier day.
                    items.extend(agenda_item(table, base, day.day, item, t));
                }
            }
            if items.is_empty() && day.holiday.is_none() {
                continue;
            }
            days.push(DayLine {
                when: format!("{} {}", day_short(day.day.weekday(), t), day.day.day_month(t.locale)),
                today: today == Some(day.day),
                holiday: day.holiday,
                items,
            });
        }
        let mut head = week_head(week.monday, week.iso_week.1, t);
        if week.break_week {
            head.push_str(" · ");
            head.push_str(t.data.timetable.lecture_break);
        }
        match table.facts.ab_week(week.monday).filter(|_| ab) {
            Some(Weeks::A) => head.push_str(&format!(" · {}", words.in_week_a)),
            Some(Weeks::B) => head.push_str(&format!(" · {}", words.in_week_b)),
            _ => {}
        }
        blocks.push(Block::Week { id: week_id(week.iso_week), head, days, from: week.monday, to: week.monday.plus(6) });
    }
    flush(&mut quiet, &mut blocks);
    blocks
}

fn agenda_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem, t: &i18n::Texts) -> Option<ItemLine> {
    match (item.event, item.exam) {
        (Some(e), _) => teaching_item(table, base, day, item, e, t),
        (None, Some(x)) => exam_item(table, base, day, item, x, t),
        (None, None) => None,
    }
}

/// The event of `item` when the item is a date of an option of a choice not yet made („1 von 4").
fn open_option(table: &Timetable, item: &AgendaItem) -> Option<usize> {
    let e = item.event?;
    let event = table.events.get(e)?;
    let row = event.rows.get(item.row)?;
    (event.unresolved() && row.option.is_some()).then_some(e)
}

/// A week's open choices: per event, its options' dates in the week that take place, in order.
fn open_choices<'a>(table: &Timetable, week: &'a AgendaWeek) -> BTreeMap<usize, Vec<(Day, &'a AgendaItem)>> {
    let mut choices: BTreeMap<usize, Vec<(Day, &AgendaItem)>> = BTreeMap::new();
    for day in &week.days {
        for item in &day.items {
            if let Some(e) = open_option(table, item) {
                let dates = choices.entry(e).or_default();
                if item.cancelled.is_none() {
                    dates.push((day.day, item));
                }
            }
        }
    }
    choices
}

/// The one line of a week for a choice not yet made: „Praktikum · Programmierpraktikum" and
/// „1 von 4 (Mo, Mi, Do, Fr)", the days its options take place on in the week; the time where all
/// of them share one. It opens the module at the first of them, where „Nur diesen" makes the
/// choice. `dates` are the week's dates of its options (`open_choices`); none when all of them
/// are cancelled.
fn choice_item(table: &Timetable, base: &StudyplanUrl, e: usize, dates: &[(Day, &AgendaItem)], t: &i18n::Texts) -> Option<ItemLine> {
    let words = &t.studyplan_week;
    let event = table.events.get(e)?;
    let module = event.modules.first()?;
    let options = event.visible_options().len();
    let mut days: Vec<&str> = Vec::new();
    for (day, _) in dates {
        let short = day_short(day.weekday(), t);
        if !days.contains(&short) {
            days.push(short);
        }
    }
    let times: BTreeSet<(Option<u16>, Option<u16>)> = dates.iter().map(|(_, item)| (item.from, item.to)).collect();
    let time = match times.into_iter().collect::<Vec<_>>().as_slice() {
        [(Some(from), Some(to))] => format!("{}–{}", clock(*from), clock(*to)),
        _ => String::new(),
    };
    let small = match days.is_empty() {
        true => format!("{} · {}", (words.one_of)(options), words.cancelled),
        false => format!("{} ({})", (words.one_of)(options), days.join(", ")),
    };
    let text = event_text(event, t.locale);
    let whens = dates.iter().map(|(day, item)| match (item.from, item.to) {
        (Some(from), Some(to)) => format!("{} {}–{}", day_short(day.weekday(), t), clock(from), clock(to)),
        _ => day_short(day.weekday(), t).to_string(),
    });
    let title: Vec<String> = [text.clone(), small.clone()].into_iter().chain(whens).collect();
    let first = dates.first().and_then(|(_, item)| event.rows.get(item.row)).or_else(|| event.rows.iter().find(|row| row.option.is_some()));
    Some(ItemLine {
        class: classes(hue(event.tone), &[(true, "alt")]),
        time,
        text,
        small,
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.and_then(|row| row.key)).path(),
        warn: false,
    })
}

fn teaching_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem, e: usize, t: &i18n::Texts) -> Option<ItemLine> {
    let words = &t.studyplan_week;
    let event = table.events.get(e)?;
    let first = event.rows.get(item.row)?;
    let module = event.modules.first()?;
    let rows: Vec<&Row> = item.rows.iter().filter_map(|r| event.rows.get(*r)).collect();
    let alt = event.unresolved() && first.option.is_some();
    let time = match (item.from, item.to) {
        (Some(from), Some(to)) => format!("{}–{}", clock(from), clock(to)),
        _ if rows.iter().any(|row| row.occ.all_day) => words.all_day.to_string(),
        _ => words.time_open.to_string(),
    };
    let text = event_text(event, t.locale);
    let rooms = rooms(rows.iter().map(|row| &row.date));

    let mut small = Vec::new();
    if alt {
        small.push((words.one_of)(event.visible_options().len()));
    }
    // A room note says where the date is instead of the row's room, so it takes the room's place.
    let said = match (&item.cancelled, &item.note) {
        (Some(reason), _) if reason.is_empty() => Some(words.cancelled.to_string()),
        (Some(reason), _) => Some(format!("{} · {reason}", words.cancelled)),
        (None, Some(note)) => Some(note.clone()),
        (None, None) => None,
    };
    small.extend(said.clone().or_else(|| rooms.clone()));

    let mut title = vec![text.clone(), format!("{} {time}", day.date(t.locale))];
    title.extend(rooms);
    title.extend(said);
    Some(ItemLine {
        class: classes(hue(event.tone), &[(alt, "alt"), (item.cancelled.is_some(), "cancelled")]),
        time,
        text,
        small: small.join(" · "),
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.key).path(),
        warn: false,
    })
}

fn exam_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem, x: usize, t: &i18n::Texts) -> Option<ItemLine> {
    let words = &t.studyplan_week;
    let exam = table.exams.get(x)?;
    let first = exam.rows.get(item.row)?;
    let module = exam.modules.first()?;
    let time = match first.shape {
        ExamShape::Sitting { from, to, .. } => format!("{}–{}", clock(from), clock(to)),
        ExamShape::Deadline { .. } => (words.due_by)(first.date.end_time.as_deref().unwrap_or("24:00")),
        ExamShape::Window { last, .. } => (words.until)(&last.day_month(t.locale)),
        ExamShape::DayOnly { .. } => words.time_open.to_string(),
        // The agenda has no day for it: it stands under „Ohne Datum" (`open_exams`).
        ExamShape::Open => return None,
    };
    let text = kind_and_title(t.data.timetable.exam, &exam.title);
    let rooms = rooms(item.rows.iter().filter_map(|r| exam.rows.get(*r)).map(|row| &row.date));
    let small: Vec<String> = (first.rank == 2).then(|| t.data.timetable.second_sitting.to_string()).into_iter().chain(rooms).collect();
    // A warning names a module's Termin by its day and start; this sitting is that Termin.
    let named = |termin: &Termin| exam.modules.contains(&termin.module_id) && Some(termin.from) == item.from;
    let warn = table.exam_warnings.iter().any(|warning| warning.day == day && (named(&warning.a) || named(&warning.b)));
    let title: Vec<String> = [text.clone(), format!("{} {time}", day.date(t.locale))].into_iter().chain(small.iter().cloned()).collect();
    Some(ItemLine {
        class: classes(exam_hue(table, module), &[(true, "exam")]),
        time,
        text,
        small: small.join(" · "),
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.key).path(),
        warn,
    })
}

/// The week to open the agenda at: the current one, or the next one shown after a gap between
/// weeks. None before the agenda begins and after it ends; none either when that is the first
/// block, where the agenda begins anyway and the semester's head and notes stay in view.
fn scroll_target(blocks: &[Block], today: Day) -> Option<String> {
    let first = blocks.first()?;
    if today < first.span().0 {
        return None;
    }
    let target = blocks.iter().find(|block| block.span().1 >= today)?;
    (target != first).then(|| target.id().to_string())
}

fn block_view(block: Block, t: &'static i18n::Texts) -> impl IntoView {
    match block {
        Block::Break { id, text, .. } => view! { <p class="agenda-break" id=id>{text}</p> }.into_any(),
        Block::Week { id, head, days, .. } => {
            let empty = days.is_empty().then(|| view! { <small>{format!(" · {}", t.studyplan_week.no_dates)}</small> });
            let within = Within::Week(id.clone());
            view! {
                <section class="agenda-week" id=id on:click=move |ev| remember(within.clone(), &ev)>
                    <h3>{head}{empty}</h3>
                    {days.into_iter().map(|day| day_view(day, t)).collect_view()}
                </section>
            }
            .into_any()
        }
    }
}

fn day_view(day: DayLine, t: &'static i18n::Texts) -> impl IntoView {
    view! {
        <div class="agenda-day" class:today=day.today>
            <span class="when">{day.when}</span>
            <div>
                {day.holiday.map(|name| view! { <span class="holiday">{name}</span> })}
                {day.items.into_iter().map(|item| item_view(item, t)).collect_view()}
            </div>
        </div>
    }
}

fn item_view(item: ItemLine, t: &'static i18n::Texts) -> impl IntoView {
    let small = (!item.small.is_empty()).then(|| view! { <small>{item.small}</small> });
    view! {
        <a class=format!("agenda-item {}", item.class) href=t.path(&item.href) data-noscroll="" title=item.title>
            <i></i>
            <span class="t">{item.time}</span>
            {item.warn.then(|| view! { <Icon name="triangle-alert"/> })}
            <span>{item.text}</span>
            {small}
        </a>
    }
}

// ---------- what has no fixed time ----------

/// A Termin without a fixed time or date, or an event without any.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LooseLine {
    hue: &'static str,
    /// „Praktikum · Programmierpraktikum"
    text: String,
    /// What QIS says of it: „Mo · nach Vereinbarung · 12.10.–25.01. · HG 0.20", „ohne Termine".
    small: String,
    /// The app's path of its link; the page writes it in its language.
    href: String,
}

/// The lines of `entries` (`(event, row)`, `None` for an event without dates), in their order,
/// which is the plan's: by module.
fn loose_lines(table: &Timetable, base: &StudyplanUrl, entries: &[(usize, Option<usize>)], t: &i18n::Texts) -> Vec<LooseLine> {
    entries
        .iter()
        .filter_map(|(e, r)| {
            let event = table.events.get(*e)?;
            let module = event.modules.first()?;
            let row = r.and_then(|r| event.rows.get(r));
            Some(LooseLine {
                hue: hue(event.tone),
                text: event_text(event, t.locale),
                small: row.map_or_else(|| t.studyplan_week.without_dates.to_string(), |row| row_facts(&row.date, t)),
                href: base.with_open(Some(module.as_str()), row.and_then(|row| row.key)).path(),
            })
        })
        .collect()
}

/// What the agenda cannot place on a day: the shown events without any date, and their shown rows
/// with neither a held nor a cancelled date (a rhythm like „nach Absprache", a pattern without a
/// lecture period, a weekly row the break takes whole).
fn undated(table: &Timetable) -> Vec<(usize, Option<usize>)> {
    let mut entries = Vec::new();
    for (e, event) in table.events.iter().enumerate().filter(|(_, event)| event.hidden.is_none()) {
        if event.rows.is_empty() {
            entries.push((e, None));
        }
        for (r, row) in event.rows.iter().enumerate().filter(|(_, row)| row.hidden.is_none()) {
            if row.occ.days.is_empty() && row.occ.cancelled.is_empty() {
                entries.push((e, Some(r)));
            }
        }
    }
    entries
}

/// The shown exam Termine without a date (QIS lists the exam, its date is open), in the plan's
/// order: the agenda has no day for them. „Prüfungen" names them as well.
fn open_exams(table: &Timetable, base: &StudyplanUrl, t: &i18n::Texts) -> Vec<LooseLine> {
    let mut lines = Vec::new();
    for exam in table.exams.iter().filter(|exam| exam.hidden.is_none()) {
        let Some(module) = exam.modules.first() else { continue };
        for row in exam.rows.iter().filter(|row| row.hidden.is_none() && matches!(row.shape, ExamShape::Open)) {
            let small: Vec<String> = std::iter::once(t.studyplan_week.date_open.to_string()).chain(rooms(std::iter::once(&row.date))).collect();
            let line = LooseLine {
                hue: exam_hue(table, module),
                text: kind_and_title(t.data.timetable.exam, &exam.title),
                small: small.join(" · "),
                href: base.with_open(Some(module.as_str()), row.key).path(),
            };
            // Two open dates of one exam that say the same are one line (and one key of the list).
            if !lines.contains(&line) {
                lines.push(line);
            }
        }
    }
    lines
}

/// A row as QIS gives it, in what it has: weekday and time, rhythm, dates, room.
fn row_facts(date: &EventDate, t: &i18n::Texts) -> String {
    let locale = t.locale;
    let said = date.rhythm_raw.clone().filter(|raw| !raw.trim().is_empty());
    let rhythm = match &date.rhythm {
        // Radix's `other` is whatever QIS wrote („nach Absprache" when it wrote nothing else).
        Some(code) if code.is(Rhythm::Other) => said.or_else(|| Some(code.label(locale).to_string())),
        Some(code) => Some(code.label(locale).to_string()),
        None => said,
    };
    let first = date.first_date.as_deref().and_then(Day::parse);
    let last = date.last_date.as_deref().and_then(Day::parse);
    let dates = match (first, last) {
        (Some(first), Some(last)) if first == last => Some(first.day_month(locale)),
        (Some(first), Some(last)) => Some(format!("{}–{}", first.day_month(locale), last.day_month(locale))),
        (Some(first), None) => Some((t.studyplan_week.from)(&first.day_month(locale))),
        (None, _) => None,
    };
    let parts: Vec<String> = format::time_slot(date.weekday, date.start_time.as_deref(), date.end_time.as_deref(), locale)
        .into_iter()
        .chain(rhythm)
        .chain(dates)
        .chain(date.room.clone().filter(|room| !room.trim().is_empty()))
        .collect();
    if parts.is_empty() {
        return t.common.not_stated.to_string();
    }
    parts.join(" · ")
}

/// „Ohne feste Zeit" (or „Ohne Datum") and its lines; nothing when there are none.
#[component]
fn Loose(title: &'static str, lines: Memo<Vec<LooseLine>>) -> impl IntoView {
    let t = i18n::t();
    let some = Memo::new(move |_| lines.with(|lines| !lines.is_empty()));
    // What QIS says stands in the line's text, where it wraps with it: a line has no time for
    // the agenda's column of times to align it with.
    let line = move |line: LooseLine| {
        view! {
            <a class=format!("agenda-item {}", line.hue) href=t.path(&line.href) data-noscroll="">
                <i></i>
                <span>{line.text}" "<small>{line.small}</small></span>
            </a>
        }
    };
    move || {
        some.get().then(|| {
            view! {
                <h3 class="label">{title}</h3>
                <div class="sp-loose" on:click=|ev| remember(Within::Loose, &ev)>
                    <For each=move || lines.get() key=|line| line.clone() children=line/>
                </div>
            }
        })
    }
}

// ---------- words ----------

/// An exam takes the tone of its first planned module, as that module's events do.
fn exam_hue(table: &Timetable, module: &str) -> &'static str {
    hue(tone_at(table.modules.iter().position(|planned| planned == module).unwrap_or_default()))
}

/// The tone first, then the names of the flags that are on.
fn classes(hue: &'static str, flags: &[(bool, &'static str)]) -> String {
    std::iter::once(hue).chain(flags.iter().filter(|(on, _)| *on).map(|(_, name)| *name)).collect::<Vec<_>>().join(" ")
}

/// „Übung · Entwicklung von Softwaresystemen": what an event is (as QIS types it, else its kinds
/// in `locale`), then its title.
fn event_text(event: &Event, locale: Locale) -> String {
    kind_and_title(&type_text(event, locale), &event.title)
}

/// A slot's label, its first line (owner, 2026-09-25: „<type> <short-tag>"): the kinds in their
/// few letters, then what `titles` names the event's module by — in the plan its abbreviation,
/// else its short name (`StudyplanData::slot_names`): „VL EvS", „Prak Programmierpraktikum",
/// „Ü Mathematik IT-1". The event's own title where the plan's data has no title of its module.
fn slot_label(event: &Event, titles: &BTreeMap<String, String>, locale: Locale) -> String {
    let title = event.modules.first().and_then(|module| titles.get(module)).map_or(event.title.as_str(), String::as_str);
    let name = short_title(title);
    match kind_short(event, locale) {
        short if short.is_empty() => name,
        short => format!("{short} {name}"),
    }
}

/// The rooms of rows, each once, as a student reads them at a glance: „HG/0.20 / HG/0.19" (the
/// short forms of schema 9, QIS's names where a row has none).
fn rooms<'a>(dates: impl Iterator<Item = &'a EventDate>) -> Option<String> {
    let mut rooms: Vec<&str> = Vec::new();
    for room in dates.filter_map(EventDate::room_shown) {
        if !rooms.contains(&room) {
            rooms.push(room);
        }
    }
    (!rooms.is_empty()).then(|| rooms.join(" / "))
}

/// The rooms of rows, each once, as QIS writes them („Hauptgebäude - HG 0.20 - Zentralcampus"):
/// for a tooltip, which has the room for them.
fn rooms_long<'a>(dates: impl Iterator<Item = &'a EventDate>) -> Option<String> {
    let mut rooms: Vec<&str> = Vec::new();
    for room in dates.filter_map(|date| date.room.as_deref()).map(str::trim).filter(|room| !room.is_empty()) {
        if !rooms.contains(&room) {
            rooms.push(room);
        }
    }
    (!rooms.is_empty()).then(|| rooms.join(" / "))
}

/// „Montag" for 1, "Monday" in English; nothing for what is no weekday.
fn weekday_name(day: u8, t: &i18n::Texts) -> &'static str {
    t.data.common.weekday(i64::from(day)).unwrap_or_default()
}

/// „Mo" for 1, "Mon" in English; nothing for what is no weekday.
fn day_short(day: u8, t: &i18n::Texts) -> &'static str {
    t.data.common.weekday_short(i64::from(day)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use catalog::labels::Code;
    use catalog::rows_detail::DateRow;
    use catalog::timetable::day::holidays;
    use catalog::timetable::model::Input;
    use catalog::timetable::select::{Selection, TownChoice};
    use catalog::timetable::semester::SemesterKey;
    use catalog::timetable::views::AgendaDay;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn key() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    /// 2026W as the snapshot derives it: lectures 05.10.2026–31.01.2027, a break over the turn
    /// of the year, A weeks from the first.
    fn winter() -> SemesterFacts {
        let bounds = key().bounds();
        let holidays = [2026, 2027].into_iter().flat_map(holidays).filter(|(day, _)| (bounds.0..=bounds.1).contains(day)).collect();
        SemesterFacts {
            key: key(),
            bounds,
            lecture: Some((d("2026-10-05"), d("2027-01-31"))),
            breaks: vec![(d("2026-12-21"), d("2027-01-03"))],
            a_week: Some(d("2026-10-05")),
            holidays,
        }
    }

    /// A dated row of module 12104 in 2026W: event, ord, QIS's type, rhythm, weekday, times, dates.
    fn row(event: &str, ord: i64, kind: &str, rhythm: &str, weekday: i64, time: (&str, &str), dates: (&str, &str)) -> DateRow {
        DateRow {
            module_id: "12104".into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: "Entwicklung von Softwaresystemen".into(),
                event_type: Some(kind.into()),
                group_name: None,
                weekday: Some(weekday),
                start_time: Some(time.0.into()),
                end_time: Some(time.1.into()),
                rhythm: Some(Code::parse(rhythm)),
                rhythm_raw: None,
                first_date: Some(dates.0.into()),
                last_date: Some(dates.1.into()),
                room: Some("HG 0.20".into()),
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    fn table(schedule: &[DateRow]) -> Timetable {
        planned(&["12104"], schedule, &[])
    }

    /// The title of the module the rows belong to.
    fn titles() -> BTreeMap<String, String> {
        BTreeMap::from([("12104".to_string(), "Entwicklung von Softwaresystemen".to_string())])
    }

    /// The timetable of `modules` planned in this order, with teaching and exam rows.
    fn planned(modules: &[&str], schedule: &[DateRow], exams: &[DateRow]) -> Timetable {
        let facts = winter();
        let modules: Vec<String> = modules.iter().map(|id| id.to_string()).collect();
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule, exams, sws: &[] };
        Timetable::build(&input, &Selection::default())
    }

    /// An exam row: module, event, ord, title, its dates (none: the date is open), its times
    /// (empty: none), its room.
    fn exam(module: &str, event: &str, ord: i64, title: &str, dates: Option<(&str, &str)>, time: (&str, &str), room: Option<&str>) -> DateRow {
        let time_of = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
        let mut exam = row(event, ord, "Prüfung", "single", 1, ("", ""), ("", ""));
        exam.module_id = module.into();
        exam.date.event_title = title.into();
        exam.date.event_type = None;
        (exam.date.weekday, exam.date.rhythm) = (None, None);
        (exam.date.start_time, exam.date.end_time) = (time_of(time.0), time_of(time.1));
        exam.date.first_date = dates.map(|(first, _)| first.into());
        exam.date.last_date = dates.map(|(_, last)| last.into());
        exam.date.room = room.map(str::to_string);
        exam
    }

    /// The items of the agenda with their day („Do 11.02.").
    fn agenda_items(blocks: &[Block]) -> Vec<(String, &ItemLine)> {
        blocks
            .iter()
            .filter_map(|block| match block {
                Block::Week { days, .. } => Some(days),
                Block::Break { .. } => None,
            })
            .flatten()
            .flat_map(|day| day.items.iter().map(move |item| (day.when.clone(), item)))
            .collect()
    }

    fn plain() -> StudyplanUrl {
        StudyplanUrl::parse("sem=2026W")
    }

    #[test]
    fn a_slot_says_its_time_and_its_weeks() {
        let small = |label: WeekLabel, every: Option<Every>| slot_small(&label, every, 690, 780, &i18n::DE);
        assert_eq!(small(WeekLabel::Every(Every::Week), Some(Every::Week)), "11:30–13:00");
        assert_eq!(small(WeekLabel::Every(Every::AWeek), Some(Every::AWeek)), "11:30–13:00 A");
        assert_eq!(small(WeekLabel::Every(Every::BWeek), Some(Every::BWeek)), "11:30–13:00 B");
        assert_eq!(small(WeekLabel::Every(Every::FourWeeks), Some(Every::FourWeeks)), "11:30–13:00 · 4-wöch.");
        // A part of the lecture period keeps its weeks; which end it leaves out is the tooltip's.
        let until = WeekLabel::Partial { first: d("2026-10-06"), last: d("2026-11-24") };
        assert_eq!(small(until.clone(), Some(Every::Week)), "11:30–13:00");
        assert_eq!(small(until, Some(Every::AWeek)), "11:30–13:00 A");
        // Single dates: the lone one's day, else how many.
        assert_eq!(small(WeekLabel::Once { dates: 3, first: d("2026-11-04") }, None), "11:30–13:00 · 3 Termine");
        assert_eq!(small(WeekLabel::Once { dates: 1, first: d("2027-02-23") }, None), "11:30–13:00 · 23.02.");

        // What the tooltip and a phone's line say besides: the option of a choice not made, the
        // weeks in words, the part of the lecture period.
        let facts = winter();
        let said = |label: WeekLabel, every: Option<Every>, alt| qualifiers(&label, every, alt, &facts, &i18n::DE).join(" · ");
        assert_eq!(said(WeekLabel::Every(Every::Week), Some(Every::Week), Some((4, 1))), "1 von 4");
        assert_eq!(said(WeekLabel::Every(Every::BWeek), Some(Every::BWeek), None), "B-Woche");
        let until = WeekLabel::Partial { first: d("2026-10-06"), last: d("2026-11-24") };
        assert_eq!(said(until, Some(Every::AWeek), None), "A-Woche · bis 24.11.");
        assert_eq!(said(WeekLabel::Partial { first: d("2026-12-07"), last: d("2027-01-25") }, Some(Every::Week), None), "ab 07.12.");
        assert_eq!(said(WeekLabel::Partial { first: d("2026-10-26"), last: d("2026-11-23") }, Some(Every::Week), None), "26.10.–23.11.");
        assert_eq!(said(WeekLabel::Once { dates: 1, first: d("2027-02-23") }, None, None), "1 Termin · 23.02.");
    }

    #[test]
    fn the_regelwoche_links_each_slot_to_its_termin() {
        let schedule = [
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")),
            row("148134", 1, "Übung", "week_a", 2, ("07:30", "09:00"), ("2026-10-06", "2026-11-17")),
            row("148019", 1, "Übung", "single", 2, ("11:45", "13:15"), ("2027-02-23", "2027-02-23")),
        ];
        let table = table(&schedule);
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, false, &i18n::DE);
        let shown: Vec<(&str, &str, &str, &str, &str)> = slots
            .iter()
            .map(|slot| (slot.slot.label.as_str(), slot.slot.small.as_str(), slot.slot.place.as_str(), slot.slot.class, slot.row.note.as_str()))
            .collect();
        // Three lines (owner, 2026-09-25): what and whose, when and in which weeks, where. A phone's
        // line says where first, then the rest in words.
        assert_eq!(
            shown,
            [
                ("Ü Entwicklung von Softwaresystemen", "07:30–09:00 A", "HG 0.20", "tinted", "HG 0.20 · A-Woche · bis 17.11."),
                ("VL Entwicklung von Softwaresystemen", "11:30–13:00", "HG 0.20", "tinted", "HG 0.20"),
                ("Ü Entwicklung von Softwaresystemen", "11:45–13:15 · 23.02.", "HG 0.20", "once", "HG 0.20 · 1 Termin · 23.02."),
            ]
        );
        // The abbreviation stands for the module where the plan's data has one.
        let short = BTreeMap::from([("12104".to_string(), "EvS".to_string())]);
        assert_eq!(week_slots(&table, &plain(), &short, Weeks::All, false, &i18n::DE).get(1).map(|slot| slot.slot.label.clone()).as_deref(), Some("VL EvS"));
        // The label names the module in its short name, whatever the event is called.
        let long = BTreeMap::from([("12104".to_string(), "Elektrische und elektronische Grundlagen der Informatik".to_string())]);
        let slots = week_slots(&table, &plain(), &long, Weeks::All, false, &i18n::DE);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Elektrische und elektronische …"));
        assert_eq!(slots.get(1).map(|slot| slot.row.text.as_str()), Some("Vorlesung · Entwicklung von Softwaresystemen"));
        // Without the module's title, the event's stands in.
        let slots = week_slots(&table, &plain(), &BTreeMap::new(), Weeks::All, false, &i18n::DE);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Entwicklung von Softwaresystemen"));
        let lecture = slots.get(1).unwrap();
        let key = table.events.iter().find(|event| event.id == "148701").and_then(|event| event.rows.first()).and_then(|row| row.key).unwrap();
        assert_eq!(lecture.slot.href.as_deref(), Some(format!("/studyplan?sem=2026W&open=12104&row={}", key.text()).as_str()));
        assert_eq!((lecture.slot.hue, lecture.row.class.as_str()), (Some("t-ice"), "t-ice"));
        assert_eq!((lecture.row.time.as_str(), lecture.row.text.as_str()), ("11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!(lecture.slot.title, "Vorlesung · Entwicklung von Softwaresystemen · Di 11:30–13:00 · HG 0.20");
        // The slot is the one beside the plan when its module is open and the address names it.
        assert!(lecture.row.is(&(Some("12104".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12107".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12104".into()), None)));
        assert!(!slots.first().unwrap().row.is(&(Some("12104".into()), Some(key))));
    }

    #[test]
    fn the_week_and_the_agenda_read_in_english_too() {
        let en = &i18n::EN;
        let schedule = [
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")),
            row("148134", 1, "Übung", "week_a", 2, ("07:30", "09:00"), ("2026-10-06", "2026-11-17")),
            row("148019", 1, "Übung", "single", 2, ("11:45", "13:15"), ("2027-02-23", "2027-02-23")),
        ];
        let table = table(&schedule);
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, false, en);
        let shown: Vec<(&str, &str, &str)> = slots.iter().map(|slot| (slot.slot.label.as_str(), slot.slot.small.as_str(), slot.row.note.as_str())).collect();
        // The kinds' few letters, the weeks and the dates in English; rooms and titles as QIS has them.
        assert_eq!(
            shown,
            [
                ("Ex Entwicklung von Softwaresystemen", "07:30–09:00 A", "HG 0.20 · week A · until 17 Nov"),
                ("Lec Entwicklung von Softwaresystemen", "11:30–13:00", "HG 0.20"),
                ("Ex Entwicklung von Softwaresystemen", "11:45–13:15 · 23 Feb", "HG 0.20 · 1 session · 23 Feb"),
            ]
        );
        // QIS's type of the event („Vorlesung") is the data's word, in every language.
        let lecture = slots.get(1).unwrap();
        assert_eq!(lecture.slot.title, "Vorlesung · Entwicklung von Softwaresystemen · Tue 11:30–13:00 · HG 0.20");
        assert_eq!(lecture.acts.iter().map(|act| act.label.as_str()).collect::<Vec<_>>(), ["Hide Vorlesung"]);
        let event = table.events.iter().find(|event| event.id == "148701").unwrap();
        assert_eq!((reason_text(HiddenBy::Kinds, event, en), reason_text(HiddenBy::Choice, event, en)), ("“Lecture” hidden".to_string(), "another group chosen".to_string()));
        // The slot keeps the app's path; the grid writes its link under `/en`.
        assert!(lecture.slot.href.as_deref().is_some_and(|href| href.starts_with("/studyplan?")));
        assert!(linked(&lecture.slot, en).is_some_and(|href| href.starts_with("/en/studyplan?sem=2026W&open=12104&row=")));
        assert_eq!(linked(&lecture.slot, &i18n::DE), lecture.slot.href);

        // The agenda: the heads of its weeks, a day, the break.
        let blocks = agenda_blocks(&table, &table.agenda(Locale::En), &plain(), None, en);
        let heads: Vec<&str> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Week { head, .. } => Some(head.as_str()),
                Block::Break { .. } => None,
            })
            .take(2)
            .collect();
        assert_eq!(heads, ["Week 41 · 5–11 Oct 2026 · week A", "Week 42 · 12–18 Oct 2026 · week B"]);
        assert!(blocks.iter().any(|block| matches!(block, Block::Break { text, .. } if text == "21 Dec–3 Jan · no lectures")), "{blocks:?}");
        let items = agenda_items(&blocks);
        let (when, first) = items.first().unwrap();
        assert_eq!((when.as_str(), first.text.as_str(), first.title.as_str()), ("Tue 6 Oct", "Übung · Entwicklung von Softwaresystemen", "Übung · Entwicklung von Softwaresystemen · 6 Oct 2026 07:30–09:00 · HG 0.20"));
    }

    #[test]
    fn a_phone_lists_the_slots_by_day() {
        let at = |day: u8, text: &str| DayRow {
            day,
            class: "t-ice".into(),
            time: String::new(),
            text: text.into(),
            note: String::new(),
            href: String::new(),
            modules: Vec::new(),
            keys: Vec::new(),
            acts: Vec::new(),
        };
        let groups = day_groups([at(2, "a"), at(1, "b"), at(2, "c"), at(5, "d")]);
        let shown: Vec<(u8, Vec<&str>)> = groups.iter().map(|(day, rows)| (*day, rows.iter().map(|row| row.text.as_str()).collect())).collect();
        assert_eq!(shown, [(1, vec!["b"]), (2, vec!["a", "c"]), (5, vec!["d"])]);
        let de = &i18n::DE;
        assert_eq!((weekday_name(1, de), weekday_name(7, de), weekday_name(0, de), weekday_name(8, de)), ("Montag", "Sonntag", "", ""));
        assert_eq!((day_short(3, de), day_short(9, de)), ("Mi", ""));
        assert_eq!((weekday_name(1, &i18n::EN), day_short(3, &i18n::EN)), ("Monday", "Wed"));
    }

    #[test]
    fn a_weeks_and_b_weeks_of_a_module_clash_together() {
        // 12104: its lecture in A weeks and its exercise in B weeks, Tuesdays 07:30; 12107's lecture
        // every Tuesday 07:30 meets one of them each week.
        let mut other = row("149408", 1, "Vorlesung", "weekly", 2, ("07:30", "09:00"), ("2026-10-06", "2027-01-26"));
        other.module_id = "12107".into();
        let schedule = [
            other,
            row("148134", 1, "Vorlesung", "week_a", 2, ("07:30", "09:00"), ("2026-10-06", "2027-01-26")),
            row("148135", 1, "Übung", "week_b", 2, ("07:30", "09:00"), ("2026-10-13", "2027-01-19")),
        ];
        let table = planned(&["12104", "12107"], &schedule, &[]);
        let joints = |slots: &[PlanSlot]| -> Vec<(String, Option<u32>, bool)> {
            slots.iter().map(|slot| (format!("{} {}", slot.row.modules.first().cloned().unwrap_or_default(), slot.slot.label), slot.slot.joint, slot.slot.clash)).collect()
        };
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, false, &i18n::DE);
        // The A and the B slot of 12104 stand together, next to each other; the other lecture alone.
        let together: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| slot.slot.joint == Some(0)).map(|(at, _)| at).collect();
        assert_eq!(together.len(), 2, "{:?}", joints(&slots));
        assert_eq!(together[1], together[0] + 1);
        assert!(slots.iter().all(|slot| slot.slot.clash));
        assert!(slots.iter().filter(|slot| slot.slot.joint.is_some()).all(|slot| slot.row.modules.first().map(String::as_str) == Some("12104")));
        assert_eq!(slots.iter().filter(|slot| slot.slot.joint.is_none()).count(), 1);
        // The A weeks alone: nothing to stand with, a frame of its own.
        let a_weeks = week_slots(&table, &plain(), &titles(), Weeks::A, false, &i18n::DE);
        assert!(a_weeks.iter().all(|slot| slot.slot.joint.is_none() && slot.slot.clash) && a_weeks.len() == 2);
        // Of two modules, an A-week and a B-week slot stand apart.
        let mut exercise = row("148136", 1, "Übung", "week_b", 2, ("07:30", "09:00"), ("2026-10-13", "2027-01-19"));
        exercise.module_id = "12107".into();
        let schedule = [schedule[0].clone(), schedule[1].clone(), exercise];
        let apart = week_slots(&planned(&["12104", "12107"], &schedule, &[]), &plain(), &titles(), Weeks::All, false, &i18n::DE);
        assert!(apart.iter().all(|slot| slot.slot.joint.is_none()));
    }

    #[test]
    fn a_termin_is_decided_right_in_the_week() {
        // Programmierpraktikum (148370): four groups, Mo, Mi, Do 11:30 and Fr 13:45; a lecture of
        // one slot (148701, Di) and one of two (148702, Mo and Do, the Thursday one over the
        // Thursday group).
        let group = |ord: i64, weekday: i64, time: (&str, &str), first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, time, (first, "2027-01-29"));
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        let schedule = [
            group(1, 1, ("11:30", "13:00"), "2026-10-05"),
            group(2, 3, ("11:30", "13:00"), "2026-10-07"),
            group(3, 4, ("11:30", "13:00"), "2026-10-08"),
            group(4, 5, ("13:45", "15:15"), "2026-10-09"),
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")),
            row("148702", 1, "Vorlesung", "weekly", 1, ("09:15", "10:45"), ("2026-10-05", "2027-01-25")),
            row("148702", 2, "Vorlesung", "weekly", 4, ("11:00", "12:30"), ("2026-10-08", "2027-01-28")),
        ];
        let facts = winter();
        let modules = vec!["12104".to_string()];
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let plan_week = |doc: &PlanDoc, all: bool| {
            let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[] };
            week_slots(&Timetable::build(&input, &doc.selection(key(), TownChoice::Derive)), &plain(), &titles(), Weeks::All, all, &i18n::DE)
        };
        // Each slot as „Mo 11:30" with its buttons, „✓ …" or „× …".
        let said = |slot: &PlanSlot| -> Vec<String> { slot.acts.iter().map(|act| format!("{} {}", if act.icon == "check" { "✓" } else { "×" }, act.label)).collect() };
        let buttons = |slots: &[PlanSlot]| -> Vec<(String, Vec<String>)> { slots.iter().map(|slot| (format!("{} {}", day_short(slot.slot.day, &i18n::DE), clock(slot.slot.from)), said(slot))).collect() };
        let at = |slots: &[PlanSlot], day: u8, from: &str| slots.iter().find(|slot| slot.slot.day == day && clock(slot.slot.from) == from).cloned().unwrap();
        let practicals = |slots: &[PlanSlot]| -> Vec<u8> { slots.iter().filter(|slot| slot.row.text.starts_with("Praktikum")).map(|slot| slot.slot.day).collect() };
        let entry = |when: &str, labels: &[&str]| (when.to_string(), labels.iter().map(|label| label.to_string()).collect::<Vec<_>>());
        let group_buttons = ["✓ Diese Gruppe nehmen", "× Nicht diese Gruppe"];
        let open = plan_week(&doc, false);
        assert_eq!(
            buttons(&open),
            [
                entry("Mo 09:15", &["× Termin ausblenden"]),
                entry("Mo 11:30", &group_buttons),
                entry("Di 11:30", &["× Vorlesung ausblenden"]),
                entry("Mi 11:30", &group_buttons),
                entry("Do 11:00", &["× Termin ausblenden"]),
                entry("Do 11:30", &group_buttons),
                entry("Fr 13:45", &group_buttons),
            ]
        );
        // Each button has its key, in the grid and on a phone; the open choice asks for a decision.
        assert!(open.iter().all(|slot| slot.row.acts == slot.slot.acts && slot.slot.acts.iter().enumerate().all(|(n, button)| button.key == format!("{}/{n}", slot.key))));
        assert_eq!(open.iter().filter(|slot| slot.slot.asks).count(), 4);
        let monday = at(&open, 1, "11:30");
        assert_eq!(act_of(&open, &format!("{}/1", monday.key)), Some(monday.acts[1].act.clone()));
        assert_eq!((act_of(&open, &format!("{}/2", monday.key)), act_of(&open, "p9-9-9-999/0")), (None, None));

        // „✓" on Wednesday: the other groups leave the plan. Its „✓" stays, on and in view, and
        // takes the choice back at once; its „×" leaves the group out.
        at(&open, 3, "11:30").acts[0].act.apply(&mut doc, key());
        let chosen = plan_week(&doc, false);
        assert_eq!(practicals(&chosen), [3]);
        let wednesday = at(&chosen, 3, "11:30");
        assert_eq!(said(&wednesday), ["✓ Gruppe abwählen", "× Nicht diese Gruppe"]);
        let pressed: Vec<bool> = wednesday.slot.acts.iter().map(|button| button.pressed).collect();
        assert!(wednesday.slot.asks && pressed == [true, false] && wednesday.row.acts == wednesday.slot.acts);
        wednesday.acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [1, 3, 4, 5]);
        at(&plan_week(&doc, false), 3, "11:30").acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [3]);
        // „Alle Termine": the others beside the plan, faint, each with its „✓" in view; the one under
        // Thursday's lecture says it would meet it.
        let all = plan_week(&doc, true);
        let faint: Vec<(String, bool, bool, Vec<String>)> = all
            .iter()
            .filter(|slot| slot.slot.class == "faint")
            .map(|slot| (format!("{} {}", day_short(slot.slot.day, &i18n::DE), clock(slot.slot.from)), slot.slot.clash, slot.slot.asks, said(slot)))
            .collect();
        let taken = |when: &str, clash: bool| (when.to_string(), clash, true, vec!["✓ Diese Gruppe nehmen".to_string()]);
        assert_eq!(faint, [taken("Mo 11:30", false), taken("Do 11:30", true), taken("Fr 13:45", false)]);
        let thursday = at(&all, 4, "11:30");
        assert!(thursday.slot.title.starts_with("andere Gruppe gewählt · ") && thursday.slot.title.ends_with("überschneidet sich mit VL Entwicklung von Softwaresystemen"), "{}", thursday.slot.title);
        assert!(thursday.key.starts_with('g') && thursday.row.class.contains("faint"));
        assert_eq!(all.len(), chosen.len() + 3);
        // „✓" on a faint one switches the choice.
        at(&all, 5, "13:45").acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [5]);
        // „×" on the one taken: the choice is open again, without it; faint, its „✓" takes it again.
        at(&plan_week(&doc, false), 5, "13:45").acts[1].act.apply(&mut doc, key());
        let reopened = plan_week(&doc, false);
        assert_eq!(practicals(&reopened), [1, 3, 4]);
        assert!(reopened.iter().filter(|slot| slot.row.text.starts_with("Praktikum")).all(|slot| slot.slot.asks));
        let friday = at(&plan_week(&doc, true), 5, "13:45");
        assert_eq!(said(&friday), ["✓ Diese Gruppe nehmen"]);
        friday.acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [5]);

        // „×" on a Termin of the lecture of two: it leaves the plan; faint, its „✓" brings it back.
        at(&plan_week(&doc, false), 1, "09:15").acts[0].act.apply(&mut doc, key());
        assert!(plan_week(&doc, false).iter().all(|slot| clock(slot.slot.from) != "09:15"));
        let hidden = at(&plan_week(&doc, true), 1, "09:15");
        assert!(hidden.slot.title.starts_with("ausgeblendet · "));
        assert_eq!(said(&hidden), ["✓ Wieder einblenden"]);
        hidden.acts[0].act.apply(&mut doc, key());
        assert!(plan_week(&doc, false).iter().any(|slot| clock(slot.slot.from) == "09:15"));

        // A kind switched off comes back with every event of it.
        doc.set_kind(key(), EventKind::Lecture, true);
        assert!(plan_week(&doc, false).iter().all(|slot| !slot.row.text.starts_with("Vorlesung")));
        let lecture = at(&plan_week(&doc, true), 2, "11:30");
        assert!(lecture.slot.title.starts_with("„Vorlesung“ ausgeblendet · "), "{}", lecture.slot.title);
        assert_eq!(said(&lecture), ["✓ „Vorlesung“ wieder einblenden"]);
        lecture.acts[0].act.apply(&mut doc, key());
        assert_eq!(plan_week(&doc, false).iter().filter(|slot| slot.row.text.starts_with("Vorlesung")).count(), 3);
    }

    fn week(monday: &str, break_week: bool, days: Vec<AgendaDay>) -> AgendaWeek {
        let monday = d(monday);
        AgendaWeek { monday, iso_week: monday.iso_week(), break_week, days }
    }

    fn holiday(day: &str, name: &'static str) -> AgendaDay {
        AgendaDay { day: d(day), holiday: Some(name), items: Vec::new() }
    }

    #[test]
    fn the_agenda_is_anchored_by_week_and_folds_a_quiet_break() {
        assert_eq!(week_id((2026, 41)), "kw-2026-41");
        assert_eq!(week_id((2027, 1)), "kw-2027-1");
        assert_eq!(week_head(d("2026-10-05"), 41, &i18n::DE), "KW 41 · 05.–11.10.2026");
        assert_eq!(week_head(d("2026-10-26"), 44, &i18n::DE), "KW 44 · 26.10.–01.11.2026");
        assert_eq!(week_head(d("2026-12-28"), 53, &i18n::DE), "KW 53 · 28.12.2026–03.01.2027");

        let table = table(&[]);
        let weeks = [
            week("2026-12-14", false, Vec::new()),
            week("2026-12-21", true, vec![holiday("2026-12-25", "1. Weihnachtstag"), holiday("2026-12-26", "2. Weihnachtstag")]),
            week("2026-12-28", true, vec![holiday("2027-01-01", "Neujahr")]),
            week("2027-01-04", false, Vec::new()),
        ];
        let blocks = agenda_blocks(&table, &weeks, &plain(), Some(d("2026-12-15")), &i18n::DE);
        let shown: Vec<(&str, String)> = blocks
            .iter()
            .map(|block| match block {
                Block::Week { id, head, days, .. } => (id.as_str(), format!("{head} ({} Tage)", days.len())),
                Block::Break { id, text, .. } => (id.as_str(), text.clone()),
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("kw-2026-51", "KW 51 · 14.–20.12.2026 (0 Tage)".to_string()),
                ("kw-2026-52", "21.12.–03.01. vorlesungsfrei".to_string()),
                ("kw-2027-1", "KW 1 · 04.–10.01.2027 (0 Tage)".to_string()),
            ]
        );

        // A break week that holds a date stays a week of its own and says it is one of the break;
        // the quiet one after it is a line.
        let mut busy = [week("2026-12-21", true, vec![holiday("2026-12-25", "1. Weihnachtstag")]), week("2026-12-28", true, Vec::new())];
        if let Some(day) = busy.get_mut(0).and_then(|week| week.days.get_mut(0)) {
            day.items.push(AgendaItem { event: None, exam: None, row: 0, rows: Vec::new(), from: None, to: None, cancelled: None, note: None });
        }
        let blocks = agenda_blocks(&table, &busy, &plain(), None, &i18n::DE);
        assert!(matches!(blocks.first(), Some(Block::Week { head, .. }) if head == "KW 52 · 21.–27.12.2026 · vorlesungsfrei"), "{blocks:?}");
        assert!(matches!(blocks.get(1), Some(Block::Break { text, id, .. }) if text == "28.12.–03.01. vorlesungsfrei" && id == "kw-2026-53"), "{blocks:?}");
    }

    #[test]
    fn the_agenda_opens_at_the_current_week() {
        let table = table(&[]);
        let weeks: Vec<AgendaWeek> = ["2026-10-05", "2026-10-12", "2026-10-26"].into_iter().map(|monday| week(monday, false, Vec::new())).collect();
        let blocks = agenda_blocks(&table, &weeks, &plain(), None, &i18n::DE);
        // Before the semester, and in its first week, the agenda stays at its start.
        assert_eq!(scroll_target(&blocks, d("2026-09-24")), None);
        assert_eq!(scroll_target(&blocks, d("2026-10-07")), None);
        // In a later week, that week; in a gap between the weeks shown, the next one.
        assert_eq!(scroll_target(&blocks, d("2026-10-18")).as_deref(), Some("kw-2026-42"));
        assert_eq!(scroll_target(&blocks, d("2026-10-20")).as_deref(), Some("kw-2026-44"));
        // After its last week nothing is current.
        assert_eq!(scroll_target(&blocks, d("2026-11-02")), None);
        assert_eq!(scroll_target(&[], d("2026-10-20")), None);
    }

    #[test]
    fn the_agenda_says_what_each_date_is() {
        let mut lecture = row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26"));
        lecture.cancelled_dates = Some("13.10.2026: Projektwoche 19.01.2027: Raumwechsel".into());
        let table = table(&[lecture]);
        let blocks = agenda_blocks(&table, &table.agenda(catalog::Locale::De), &plain(), Some(d("2026-10-06")), &i18n::DE);
        let items = agenda_items(&blocks);
        let (when, first) = items.first().unwrap();
        assert_eq!((when.as_str(), first.time.as_str(), first.text.as_str()), ("Di 06.10.", "11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!((first.class.as_str(), first.small.as_str()), ("t-ice", "HG 0.20"));
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=148701-"), "{}", first.href);
        let (_, gone) = items.iter().find(|(when, _)| when == "Di 13.10.").unwrap();
        assert_eq!((gone.class.as_str(), gone.small.as_str()), ("t-ice cancelled", "fällt aus · Projektwoche"));
        let (_, moved) = items.iter().find(|(when, _)| when == "Di 19.01.").unwrap();
        assert_eq!((moved.small.as_str(), moved.title.as_str()), ("Raumwechsel", "Vorlesung · Entwicklung von Softwaresystemen · 19.01.2027 11:30–13:00 · HG 0.20 · Raumwechsel"));
        // Today is marked, the break is one line, a holiday names itself.
        assert!(blocks.iter().any(|block| matches!(block, Block::Week { days, .. } if days.iter().any(|day| day.today && day.when == "Di 06.10."))));
        assert!(blocks.iter().any(|block| matches!(block, Block::Break { text, .. } if text == "21.12.–03.01. vorlesungsfrei")));
        assert!(blocks.iter().any(|block| matches!(block, Block::Week { days, .. } if days.iter().any(|day| day.holiday == Some("Reformationstag") && day.when == "Sa 31.10."))));
    }

    #[test]
    fn the_agenda_says_when_each_exam_is_and_which_collide() {
        let exams = [
            // 12104: a first sitting that overlaps 12107's, a second one a month later, a
            // deadline; 12107: that sitting, a window, a day without a time, an open date.
            exam("12104", "90", 1, "Entwicklung von Softwaresystemen", Some(("2027-02-11", "2027-02-11")), ("11:00", "13:00"), Some("Audimax 1")),
            exam("12104", "90", 2, "Entwicklung von Softwaresystemen", Some(("2027-03-12", "2027-03-12")), ("11:00", "13:00"), Some("Audimax 1")),
            exam("12104", "94", 1, "Abgabe Softwareprojekt", Some(("2027-02-14", "2027-02-14")), ("", "24:00"), None),
            exam("12107", "91", 1, "Prüfung Mathematik", Some(("2027-02-11", "2027-02-11")), ("12:00", "14:00"), Some("HG 0.20")),
            exam("12107", "92", 1, "Hausarbeit", Some(("2027-02-01", "2027-02-05")), ("", ""), None),
            exam("12107", "93", 1, "Kolloquium", Some(("2027-02-15", "2027-02-15")), ("", ""), None),
            exam("12107", "95", 1, "Klausur Statistik", None, ("", ""), Some("HG 0.19")),
            exam("12107", "95", 2, "Klausur Statistik", None, ("", ""), Some("HG 0.19")),
        ];
        let table = planned(&["12104", "12107"], &[], &exams);
        let blocks = agenda_blocks(&table, &table.agenda(catalog::Locale::De), &plain(), None, &i18n::DE);
        let shown: Vec<(String, &str, &str, &str, &str, bool)> = agenda_items(&blocks)
            .into_iter()
            .map(|(when, item)| (when, item.time.as_str(), item.text.as_str(), item.small.as_str(), item.class.as_str(), item.warn))
            .collect();
        assert_eq!(
            shown,
            [
                // A window stands on its first day only.
                ("Mo 01.02.".to_string(), "bis 05.02.", "Prüfung · Hausarbeit", "", "t-sun exam", false),
                // The two sittings that overlap both carry the warning; a title that says it is
                // an exam stands alone.
                ("Do 11.02.".to_string(), "11:00–13:00", "Prüfung · Entwicklung von Softwaresystemen", "Audimax 1", "t-ice exam", true),
                ("Do 11.02.".to_string(), "12:00–14:00", "Prüfung Mathematik", "HG 0.20", "t-sun exam", true),
                ("So 14.02.".to_string(), "bis 24:00", "Prüfung · Abgabe Softwareprojekt", "", "t-ice exam", false),
                ("Mo 15.02.".to_string(), "Zeit offen", "Prüfung · Kolloquium", "", "t-sun exam", false),
                // The later sitting of a module says so first, and on its own day it warns of nothing.
                ("Fr 12.03.".to_string(), "11:00–13:00", "Prüfung · Entwicklung von Softwaresystemen", "2. Termin · Audimax 1", "t-ice exam", false),
            ]
        );
        let (_, first) = agenda_items(&blocks).into_iter().find(|(when, _)| when == "Do 11.02.").unwrap();
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=90-"), "{}", first.href);

        // The open date has no day: it stands under „Ohne Datum", in its module's tone, once
        // however often QIS lists it.
        let open = open_exams(&table, &plain(), &i18n::DE);
        let lines: Vec<(&str, &str, &str)> = open.iter().map(|line| (line.hue, line.text.as_str(), line.small.as_str())).collect();
        assert_eq!(lines, [("t-sun", "Prüfung · Klausur Statistik", "Termin offen · HG 0.19")]);
        assert!(open.iter().all(|line| line.href.starts_with("/studyplan?sem=2026W&open=12107")), "{open:?}");

        // In English.
        let blocks = agenda_blocks(&table, &table.agenda(Locale::En), &plain(), None, &i18n::EN);
        let english: Vec<(String, String, String, String)> = agenda_items(&blocks).into_iter().map(|(when, item)| (when, item.time.clone(), item.text.clone(), item.small.clone())).collect();
        let line = |when: &str, time: &str, text: &str, small: &str| (when.to_string(), time.to_string(), text.to_string(), small.to_string());
        assert_eq!(english.first(), Some(&line("Mon 1 Feb", "until 5 Feb", "Exam · Hausarbeit", "")));
        assert_eq!(english.get(3), Some(&line("Sun 14 Feb", "by 24:00", "Exam · Abgabe Softwareprojekt", "")));
        assert_eq!(english.get(4), Some(&line("Mon 15 Feb", "time TBA", "Exam · Kolloquium", "")));
        assert_eq!(english.last(), Some(&line("Fri 12 Mar", "11:00–13:00", "Exam · Entwicklung von Softwaresystemen", "2nd sitting · Audimax 1")));
        let open: Vec<String> = open_exams(&table, &plain(), &i18n::EN).into_iter().map(|line| format!("{} · {}", line.text, line.small)).collect();
        assert_eq!(open, ["Exam · Klausur Statistik · date TBA · HG 0.19"]);
    }

    /// `views::kind_and_title` has its own test; here a slot and a line of real events.
    #[test]
    fn a_title_that_says_what_it_is_stands_alone() {
        let mut tutorial = row("150001", 1, "Tutorium", "weekly", 3, ("09:15", "10:45"), ("2026-10-07", "2027-01-27"));
        tutorial.date.event_title = "Tutorium Höhere Mathematik W-1".into();
        let table = table(&[tutorial, row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26"))]);
        let math = BTreeMap::from([("12104".to_string(), "Höhere Mathematik W-1 (Analysis)".to_string())]);
        let labels: Vec<(String, String)> = table.events.iter().map(|event| (slot_label(event, &math, Locale::De), event_text(event, Locale::De))).collect();
        // A slot names the module in its short name after the kind's few letters.
        assert!(labels.contains(&("Tut Höhere Mathematik W-1".to_string(), "Tutorium Höhere Mathematik W-1".to_string())), "{labels:?}");
        assert!(labels.contains(&("VL Höhere Mathematik W-1".to_string(), "Vorlesung · Entwicklung von Softwaresystemen".to_string())), "{labels:?}");
    }

    #[test]
    fn an_open_choice_is_one_line_a_week() {
        // Programmierpraktikum (148370): four groups, Mo, Mi, Do 11:30 and Fr 13:45.
        let group = |ord: i64, weekday: i64, time: (&str, &str), first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, time, (first, "2027-01-29"));
            row.date.event_title = "Programmierpraktikum".into();
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        let mut monday = group(1, 1, ("11:30", "13:00"), "2026-10-05");
        monday.cancelled_dates = Some("12.10.2026: Krankheit".into());
        let schedule = [monday, group(2, 3, ("11:30", "13:00"), "2026-10-07"), group(3, 4, ("11:30", "13:00"), "2026-10-08"), group(4, 5, ("13:45", "15:15"), "2026-10-09")];
        let facts = winter();
        let modules = vec!["12104".to_string()];
        let build = |selection: &Selection| Timetable::build(&Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[] }, selection);

        let open = build(&Selection::default());
        let blocks = agenda_blocks(&open, &open.agenda(catalog::Locale::De), &plain(), None, &i18n::DE);
        let week = |id: &str| -> Vec<(String, String, String, String)> {
            blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Week { id: at, days, .. } if at == id => Some(days),
                    _ => None,
                })
                .flatten()
                .flat_map(|day| day.items.iter().map(move |item| (day.when.clone(), item.time.clone(), item.text.clone(), item.small.clone())))
                .collect()
        };
        let line = |when: &str, time: &str, small: &str| (when.to_string(), time.to_string(), "Praktikum · Programmierpraktikum".to_string(), small.to_string());
        // One line on the first day the choice could be attended, not four; the times differ.
        assert_eq!(week("kw-2026-41"), [line("Mo 05.10.", "", "1 von 4 (Mo, Mi, Do, Fr)")]);
        // A cancelled option is no day to attend: the line stands on the next.
        assert_eq!(week("kw-2026-42"), [line("Mi 14.10.", "", "1 von 4 (Mi, Do, Fr)")]);
        let (_, first) = agenda_items(&blocks).into_iter().next().unwrap();
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=148370-"), "{}", first.href);
        assert!(first.class.ends_with(" alt") && first.title.contains("Mo 11:30–13:00 · Mi 11:30–13:00"), "{first:?}");

        // Today keeps its line (review 2026-09-25): its options stand as they are, and on the day
        // of the week's line nothing comes twice.
        let on = |today: &str| {
            let blocks = agenda_blocks(&open, &open.agenda(catalog::Locale::De), &plain(), Some(d(today)), &i18n::DE);
            let days: Vec<(String, bool, Vec<String>)> = blocks
                .into_iter()
                .filter_map(|block| match block {
                    Block::Week { id, days, .. } if id == "kw-2026-41" || id == "kw-2026-42" => Some(days),
                    _ => None,
                })
                .flatten()
                .map(|day| (day.when, day.today, day.items.into_iter().map(|item| format!("{} {}", item.time, item.small)).collect()))
                .collect();
            days
        };
        let day = |when: &str, today: bool, items: &[&str]| (when.to_string(), today, items.iter().map(|item| item.to_string()).collect::<Vec<_>>());
        assert_eq!(
            on("2026-10-08"),
            [
                day("Mo 05.10.", false, &[" 1 von 4 (Mo, Mi, Do, Fr)"]),
                day("Do 08.10.", true, &["11:30–13:00 1 von 4 · HG 0.20"]),
                day("Mi 14.10.", false, &[" 1 von 4 (Mi, Do, Fr)"]),
            ]
        );
        assert_eq!(on("2026-10-05").first(), Some(&day("Mo 05.10.", true, &[" 1 von 4 (Mo, Mi, Do, Fr)"])));
        // Today's option is cancelled: it says so, and the week's line stands on the next day.
        assert_eq!(
            on("2026-10-12").get(1..),
            Some(&[day("Mo 12.10.", true, &["11:30–13:00 1 von 4 · fällt aus · Krankheit"]), day("Mi 14.10.", false, &[" 1 von 4 (Mi, Do, Fr)"])][..])
        );

        // Chosen: that group's dates, one line each, as every other Termin.
        let chosen = open.events.iter().flat_map(|event| &event.rows).find(|row| row.date.weekday == Some(3)).and_then(|row| row.key).unwrap();
        let decided = build(&Selection { chosen_rows: [chosen].into(), ..Selection::default() });
        let blocks = agenda_blocks(&decided, &decided.agenda(catalog::Locale::De), &plain(), None, &i18n::DE);
        let items = agenda_items(&blocks);
        assert!(items.iter().all(|(when, item)| when.starts_with("Mi") && item.time == "11:30–13:00" && !item.small.starts_with("1 von")), "{items:?}");
        assert!(items.len() > 10);
    }

    #[test]
    fn the_carousel_of_a_phone_holds_its_weeks_in_a_row() {
        // „A-Woche", „B-Woche", „A/B", as the head's switch has them; a week it does not know is
        // „A/B", the page's default.
        assert_eq!(WEEKS.map(week_at), [0, 1, 2]);
        assert_eq!(WEEKS.map(|week| week_label(week, &i18n::DE)), ["A-Woche", "B-Woche", "A/B"]);
        assert_eq!(WEEKS.map(|week| week_label(week, &i18n::EN)), ["Week A", "Week B", "A/B"]);
        // The week shown in the middle, its neighbours at its sides; they do not go round, so a
        // week two away waits out of sight on its own side.
        assert_eq!([0, 1, 2].map(|i| place_of(i, 2)), [-2, -1, 0]);
        assert_eq!([0, 1, 2].map(|i| place_of(i, 1)), [-1, 0, 1]);
        assert_eq!([0, 1, 2].map(|i| place_of(i, 0)), [0, 1, 2]);
    }

    #[test]
    fn a_view_remembers_the_link_it_was_left_by() {
        let href = "/studyplan?sem=2026W&view=dates&open=12104&row=148701-b7025";
        let place = Place { within: Within::Week("kw-2026-50".into()), href: href.into() };
        assert_eq!(Place::restored(&place.stored()), Some(place.clone()));
        assert_eq!(place.selector(Locale::De), format!("#kw-2026-50 a[href=\"{href}\"]"));
        // An English page writes its links under `/en`: the selector finds them there, while the
        // tab keeps the app's path.
        assert_eq!(place.selector(Locale::En), format!("#kw-2026-50 a[href=\"/en{href}\"]"));
        assert_eq!(place.week(), Some("kw-2026-50"));
        let days = Place { within: Within::Days, href: "/studyplan?open=12104".into() };
        assert_eq!(Place::restored(&days.stored()), Some(days.clone()));
        assert_eq!((days.selector(Locale::De).as_str(), days.week()), (".sp-daylist a[href=\"/studyplan?open=12104\"]", None));
        assert_eq!(Place::restored("loose /studyplan?view=dates&open=12107").map(|place| place.within), Some(Within::Loose));
        // Of the phone's grid the whole week comes back, not the slot.
        let grid = Place { within: Within::Grid, href: "/studyplan?open=12104&row=148701-b7025".into() };
        assert_eq!(Place::restored(&grid.stored()), Some(grid.clone()));
        assert_eq!((grid.selector(Locale::En).as_str(), grid.week()), (".sp-carousel", None));
        // What is read back is checked: a week's anchor, a link of the plan, nothing that would
        // break out of the selector.
        for bad in ["kw-2026 /studyplan?open=1", "week /studyplan?open=1", "days /catalog?open=1", "days /studyplan?open=1\"]", "days /studyplanx", "days"] {
            assert_eq!(Place::restored(bad), None, "{bad}");
        }
        // It is where the visitor left the view only for the same semester, view and module.
        assert!(place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12104")));
        assert!(place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12104&row=148701-aaaaa")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12107")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2026W&open=12104")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2027S&view=dates&open=12104")));
    }

    #[test]
    fn what_has_no_fixed_time_says_what_is_known() {
        let mut arranged = row("148455", 1, "Praktikum", "other", 1, ("", ""), ("2026-10-12", "2027-01-25"));
        arranged.date.start_time = None;
        arranged.date.end_time = None;
        arranged.date.rhythm_raw = Some("nach Vereinbarung".into());
        let mut dateless = row("150000", 1, "Seminar", "single", 1, ("09:15", "10:45"), ("", ""));
        dateless.ord = None;
        dateless.date.first_date = None;
        dateless.date.last_date = None;
        let table = table(&[arranged, dateless]);
        let lines = loose_lines(&table, &plain(), &table.loose(), &i18n::DE);
        let shown: Vec<(&str, &str)> = lines.iter().map(|line| (line.text.as_str(), line.small.as_str())).collect();
        assert_eq!(
            shown,
            [
                ("Praktikum · Entwicklung von Softwaresystemen", "Mo · nach Vereinbarung · 12.10.–25.01. · HG 0.20"),
                ("Seminar · Entwicklung von Softwaresystemen", "ohne Termine"),
            ]
        );
        assert_eq!(lines.last().map(|line| line.href.as_str()), Some("/studyplan?sem=2026W&open=12104"));
        // In English: QIS's own words for the rhythm stay.
        let english: Vec<String> = loose_lines(&table, &plain(), &table.loose(), &i18n::EN).into_iter().map(|line| line.small).collect();
        assert_eq!(english, ["Mon · nach Vereinbarung · 12 Oct–25 Jan · HG 0.20", "no dates"]);
        // The agenda places neither, so its „Ohne Datum" has both.
        assert_eq!(undated(&table), table.loose());
        // A row that says nothing says so (R12).
        let mut nothing = row("150002", 1, "Seminar", "other", 1, ("", ""), ("", ""));
        nothing.date = EventDate { weekday: None, start_time: None, end_time: None, rhythm: None, first_date: None, last_date: None, room: None, ..nothing.date };
        assert_eq!(row_facts(&nothing.date, &i18n::DE), "nicht angegeben");
        assert_eq!(row_facts(&nothing.date, &i18n::EN), "not stated");
    }
}
