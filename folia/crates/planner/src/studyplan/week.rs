//! The views „Woche" and „Termine" of one semester of the Studienplan.
//!
//! „Woche" is the Regelwoche (`Timetable::regular_week`): a slot per recurring Termin at its
//! weekday and time in the week grid (`folia_widgets::week`, fitted to the page's height), and the dates of
//! an event that do not recur gathered into one slot („3 Termine"). The week is there before
//! anything is planned, an empty frame with „Noch keine Termine" in its middle and its ways under
//! it while it is looked at (`NothingPlanned`, owner 2026-09-25; on a phone the words and the ways
//! alone, the page shorter than the screen). A slot says in three lines what and whose („VL EvS"),
//! when and in which weeks („07:30–09:00 A") and where („ZHG/HS.C"); its tooltip says the rest. A
//! phone has the grid too, as wide as its screen and as tall as it leaves it (owner, 2026-09-27), and
//! under it the same slots as a list of days, closed until asked for, whose rows are large enough
//! for a finger and its buttons. What has no fixed time stands under „Ohne feste Zeit", once for
//! the semester, under the fold (`WeekLoose`). Where the plan has Termine of A or B weeks only,
//! „A-Woche · B-Woche · A/B" in the head shows one kind of week or both (`PlanCtx::weeks`), on a
//! phone a carousel of the three (`WeekCarousel`), switched in the toolbar of the head („A · B ·
//! A/B"); a slot that overlaps another in the
//! week shown is red and names the other in its tooltip (owner's redesign of 2026-09-25).
//!
//! Each slot of the plan is a link to its Termin (`termin=`), in both states, a hatched group and a
//! faint slot too (owner, 2026-11-17: „dass wenn man auf einen termin klickt, dass dann infos kommen
//! die halt nicht nur für den Termin sind. Das muss auch angepasst werden so dass das mehr wie bei
//! google calendar."): what one clicks shows that Termin. While the
//! plan is edited (`edit.rs`) what to do with a Termin is decided right on it (owner, 2026-09-25:
//! „ja der fliegt raus, die Übung möchte ich", without a menu in between): „✓" takes an option of a
//! choice and stays on, filled, so that a second click takes the choice back (owner: „schnell
//! abwählen"); the eye leaves a Termin or an option out, the crossed eye of a faint one brings it
//! back, the bin deletes a Termin of one's own (`SlotAct`; owner, 2026-11-17: „Also da wo es nur
//! ausblendet das auge und da wo es entfernt eine Mülltonne."; written after the next frame, R21).
//! While looking no slot has a button. The buttons of a slot with a „✓" stand in view, the others
//! come when it is pointed at or has the keyboard. A module's A-week and B-week slots at one time
//! that clash stand together in one red frame (`join_ab`): together they compete with the other.
//! Editing shows besides what the plan leaves out, faint, in its place (`ghost_slots`; until
//! 2026-11-17 „Plan · Alle Termine"): a hidden event, kind or Termin, an option another was chosen
//! over — never drawn as a clash, what is left out meets nothing; its tooltip says what it would
//! meet — and marks a series some of whose dates are hidden (`GridSlot::hides`).
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

use folia_calendar::day::{clock, Day};
use folia_calendar::kind::EventKind;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::HiddenBy;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::Rhythm;
use folia_model::rows_detail::EventDate;
use folia_plans::studyplan::{CalendarView, PlanDoc};
use folia_routes::url::{self, StudyplanUrl};
use folia_timetable::choice;
use folia_timetable::clash::Weeks;
use folia_timetable::exams::{ExamShape, Termin};
use folia_timetable::facts::SemesterFacts;
use folia_timetable::model::{Across, Alt, Event, Row, TakenBy, Timetable};
use folia_timetable::occur::Every;
use folia_timetable::views::{
    kind_and_title, kind_short, short_title, type_text, AgendaItem, AgendaWeek, Reach, WeekItem, WeekLabel,
};
use leptos::prelude::*;

use folia_design::format;
use crate::i18n::{self, Locale};
use folia_design::nav;
use folia_shell::pending::Pending;
use folia_design::ui::Icon;
use folia_widgets::week::{slot_buttons, GridBand, GridSlot, SlotButton, WeekGrid};
use super::head::{hue, kind_word, tone_at, NothingPlanned};
use super::PlanCtx;

/// Where this browser tab remembers the link the visitor last left a view by (`Place`).
const LEFT_KEY: &str = "betula.studyplan.left";

/// The id of a phone's list of days, which its line opens.
const DAYS_ID: &str = "sp-days";

/// The module and the Termin the address puts beside the plan: what a slot or a row is marked by.
/// The module beside the plan and the Termin its panel points at, or (no module) the Termin in its
/// own view.
type Picked = (Option<String>, Option<RowKey>);

/// What the calendars of one's own add to a week: their slots and the stretches free in all of
/// them (`calendars::layer`).
type Layer = Memo<(Vec<GridSlot>, Vec<GridBand>)>;

/// „Woche": the Regelwoche as a grid that fits the page's height (`WeekGrid`'s `fit`, in
/// `div.sp-week`), on a phone as a grid that fits the screen over the list of its days (in
/// `div.sp-days`, closed until its line opens it, `PlanCtx::days`), a carousel of its weeks where
/// the plan has A or B weeks (`WeekCarousel`); while the plan is edited what it leaves out
/// besides, faint. A slot's buttons change the plan right there (`SlotAct`), on a phone a row's.
/// Where the week has nothing to draw (no module, no Termin of one's own, no calendar of one's own
/// in it) and only while it is looked at, the empty week and in its middle what is to do
/// (`NothingPlanned`), on a phone what is to do alone; while editing the week is drawn bare.
/// What has no fixed time stands under the page's fold (`WeekLoose`), the switch of A and B weeks
/// in the head (`WeekSwitch`; on a phone in the head's toolbar).
#[component]
pub(super) fn WeekView(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let base = base_of(ctx);
    let picked = picked_of(ctx);
    // What the slots name the modules by, their abbreviations („EvS"; owner, 2026-09-25: only where
    // no title fits): a sibling of the timetable, both derived from the semester's data (R16).
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    // The choice in focus (`wahl=`, folia/design/kalender), by its address: a sibling of `base`,
    // both of the address (R16); the timetable says which event it is.
    let wahl = Memo::new(move |_| ctx.url.with(|url| url.wahl.clone()));
    // The slots of each week (`WEEKS`): a phone's carousel shows all three at once. A memo is
    // worked out only where it is read, so a wide screen works out the week it shows alone.
    let weeks = WEEKS.map(|week| {
        let slots = Memo::new(move |_| {
            let (base, wahl) = (base.get(), wahl.get());
            titles.with(|titles| {
                ctx.table.with(|table| {
                    table
                        .as_ref()
                        .map(|table| {
                            let focus = wahl.as_deref().and_then(|code| choice::find(table, code));
                            week_slots(table, &base, titles, week, focus, t)
                        })
                        .unwrap_or_default()
                })
            })
        });
        (week, slots)
    });
    // The week shown (`PlanCtx::weeks`): the grid's, the list's and the buttons'.
    let slots = Memo::new(move |_| {
        let shown = ctx.weeks.get();
        weeks.iter().find(|(week, _)| *week == shown).map(|(_, slots)| slots.get()).unwrap_or_default()
    });
    // What the calendars of one's own add to each week (`calendars::layer`): their slots, and with
    // „Gemeinsam frei" the stretches every calendar in the week has free. Siblings of the plan's
    // slots, of the same sources (R16); behind a choice in focus they step back with the plan.
    let layers: [Layer; 3] = WEEKS.map(|week| {
        Memo::new(move |_| {
            let (together, focused) = (ctx.together.get(), wahl.with(Option::is_some));
            ctx.calendars.with(|calendars| ctx.table.with(|table| super::calendars::layer(calendars, table.as_ref(), week, together, focused, t)))
        })
    });
    let layer = Memo::new(move |_| {
        let shown = ctx.weeks.get();
        WEEKS.iter().position(|week| *week == shown).and_then(|at| layers.get(at)).map(|layer| layer.get()).unwrap_or_default()
    });
    // Nothing to draw: no module, no Termin of one's own and no calendar of one's own in the week
    // (siblings of the plan, R16). The card „Noch keine Termine" stands in it only then and only
    // while looking: while editing the week is drawn bare, where a click makes a Termin.
    let empty = Memo::new(move |_| {
        nothing_to_draw(
            ctx.wanted.with(|wanted| wanted.1.len()),
            ctx.selection.with(|(_, selection)| selection.own.len()),
            ctx.calendars.with(|calendars| calendars.iter().any(super::calendars::Shown::in_week)),
        )
    });
    let card = Memo::new(move |_| empty_card(empty.get(), ctx.edit.get()));
    let nothing = move || card.get().then(|| view! { <NothingPlanned ctx/> });
    // The buttons of the slots, one listener for all of them: a click changes the plan after the
    // next frame (R21), and the week follows; the bin of a Termin of one's own deletes it with the
    // note and „Rückgängig" (`own::remove`).
    // A span drawn on a free stretch while the plan is edited (`own::Drag`): the click that ends
    // it opens nothing more.
    let drag = super::own::Drag::new(ctx);
    let act = move |ev: leptos::ev::MouseEvent| {
        if drag.swallow(&ev) {
            return;
        }
        let Some(key) = act_under(ev.target()) else {
            // While editing, a click on a free stretch of a day: a Termin of one's own there (owner,
            // the first question of folia/design/kalender/termine.html: „ein Klick auf eine freie
            // Stelle der Woche füllt Tag und Zeit vor"). While looking it does nothing, and with a
            // choice in focus neither: the week is the choice's boxes then, and a click a few
            // pixels beside one must not open a form over them.
            if ctx.edit.get_untracked() && wahl.with_untracked(Option::is_none) {
                if let Some((day, minute)) = super::own::free_at(&ev) {
                    super::own::ask_new(ctx, 0, day, minute, None);
                }
            }
            return;
        };
        ev.stop_propagation();
        let (Some(plan), Some(act)) = (ctx.plan, slots.with_untracked(|slots| act_of(slots, &key))) else { return };
        if let Act::RemoveOwn(number) = act {
            super::own::remove(ctx, number);
            hush_after_bin();
            return;
        }
        let semester = ctx.key.get_untracked();
        plan.update_after_paint(move |doc| act.apply(doc, semester));
    };
    // On a phone the list of days is closed until asked for (owner, 2026-09-27): the grid over it
    // shows the week. Its line says how many Termine it holds.
    let rows = Memo::new(move |_| slots.with(Vec::len));
    // Whether there are any: the toggle and the list are built anew only when that changes, never
    // with every Termin deleted (rebuilt, the list took the browser's hold of the scroll with it,
    // and the bin threw the page 600 px under the finger).
    let any_rows = Memo::new(move |_| rows.get() > 0);
    let week = move || {
        // A phone with nothing planned says what to do where the week will stand, and the page
        // stays shorter than the screen: an empty week would take all of it. While editing the
        // week is drawn, where a finger held makes a Termin.
        if ctx.phone.get() && card.get() {
            return view! { <NothingPlanned ctx/> }.into_any();
        }
        if ctx.phone.get() {
            return view! {
                <WeekCarousel ctx weeks layers shown=slots layer picked drag/>
                {move || {
                    any_rows.get().then(|| {
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
                    (ctx.days.get() && any_rows.get()).then(|| {
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
            // While the form of a Termin of one's own is open, its draft stands where it will, in
            // the place of the Termin it changes.
            let (draft, editing) = ctx.own.with(|ask| ask.as_ref().map(|ask| (super::own::draft_slots(ask, ctx.weeks.get(), t), super::own::editing_key(ask))).unwrap_or_default());
            let mut grid: Vec<GridSlot> = slots.with(|slots| {
                slots.iter().filter(|slot| editing.is_none_or(|key| !slot.row.keys.contains(&key))).map(|slot| GridSlot { current: slot.row.is(&picked), href: linked(&slot.slot, t), ..slot.slot.clone() }).collect()
            });
            layer.with(|(calendars, _)| grid.extend(calendars.iter().cloned()));
            grid.extend(draft);
            grid
        });
        let bands = Signal::derive(move || layer.with(|(_, bands)| bands.clone()));
        view! {
            // The week holds the pointer of a press on a free stretch (`setPointerCapture`), and
            // the span drawn lies over it (`SketchView`).
            // For the keyboard a table of its Termine (enhance.js, `data-stop="grid"`): one stop of
            // Tab, the arrows from Termin to Termin as the eye sees them (← → to the day beside,
            // at the time nearest), Enter its card; `?` says what the keys do here.
            <div
                class="sp-week"
                data-stop="grid"
                data-keys-place="week"
                class:drawing=move || ctx.sketch.with(Option::is_some)
                on:click=act
                on:pointerdown=move |ev| drag.down(&ev)
                on:pointermove=move |ev| {
                    drag.moved(&ev);
                }
                on:pointerup=move |ev| {
                    drag.up(&ev);
                }
                on:pointercancel=move |_| drag.cancel()
                on:lostpointercapture=move |ev| drag.lost(&ev)
            >
                <WeekGrid slots=grid bands fit=true/>
                {nothing}
                <super::own::SketchView drag/>
            </div>
        }
        .into_any()
    };

    // Back from a module that filled the page („Vollbild", then „Zurück"; the address names it
    // still): at the line it was opened from.
    Effect::new(move |_| {
        let here = ctx.url.get_untracked();
        if here.open.is_some() {
            come_back(&here, None, t.locale);
        }
    });

    week
}

/// The Regelwoche on a phone and a small screen (owner, 2026-09-27: „Es gibt keine Wochenansicht
/// beim Kalender auf dem Smartphone"): the grid of a wide screen, as wide as the screen and as tall
/// as it leaves it, over the list of its days (closed until asked for). Where the plan has Termine
/// of A or B weeks only (`has_ab`), a carousel of the three weeks „A-Woche", „B-Woche" and „A/B",
/// as the pictures of the start page are one (`home.rs`): the week shown fills the screen's width,
/// another one comes with a swipe, the finger carrying the weeks, or with the switch in the head's
/// toolbar (`WeekSwitch`'s `short`, „A · B · A/B"; owner, 2026-10-26: in the head, no longer tabs
/// under the week). They stand in the order of that switch and do not go
/// round: the three are no cycle. The week shown is the page's (`PlanCtx::weeks`), which the switch
/// and the list under the carousel follow. Without A or B weeks the three would be one and the same: the grid alone. Its slots are
/// links to their module (on a phone the module is the page); their buttons are the list's, as
/// large as a finger (in a day's narrow column they would cover the slot). With nothing planned
/// there is no week (`WeekView`).
#[component]
fn WeekCarousel(ctx: PlanCtx, weeks: [(Weeks, Memo<Vec<PlanSlot>>); 3], layers: [Layer; 3], shown: Memo<Vec<PlanSlot>>, layer: Layer, picked: Memo<Picked>, drag: super::own::Drag) -> impl IntoView {
    let t = i18n::t();
    let ab = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(has_ab)));
    let current = Memo::new(move |_| week_at(ctx.weeks.get()));
    // A grid of the carousel: its slots, the one beside the plan marked, without buttons, and what
    // the calendars of one's own add to its week.
    let grid = move |slots: Memo<Vec<PlanSlot>>, layer: Layer, week: Weeks| {
        Signal::derive(move || {
            let picked = picked.get();
            let (draft, editing) = ctx.own.with(|ask| ask.as_ref().map(|ask| (super::own::draft_slots(ask, week, t), super::own::editing_key(ask))).unwrap_or_default());
            let mut grid: Vec<GridSlot> = slots.with(|slots| {
                slots
                    .iter()
                    .filter(|slot| editing.is_none_or(|key| !slot.row.keys.contains(&key)))
                    .map(|slot| GridSlot { current: slot.row.is(&picked), href: linked(&slot.slot, t), acts: Vec::new(), asks: false, ..slot.slot.clone() })
                    .collect()
            });
            layer.with(|(calendars, _)| grid.extend(calendars.iter().cloned()));
            grid.extend(draft);
            grid
        })
    };
    // A finger held on a free stretch of the week: a Termin of one's own there, its span stretched
    // by the finger before it lifts (owner: „am Handy langes Drücken"; 2026-11-18: „drag placement um
    // gleich eine Zeitspanne zu machen").
    let hold = super::own::Hold::new(drag);
    // The week holds still under a finger that draws: one listener on it for as long as it is there.
    let node = NodeRef::<leptos::html::Div>::new();
    #[cfg(feature = "csr")]
    drag.keep_still(node);
    // While a finger is held on the week or draws, its long press opens no menu of the browser's (a
    // mouse's right click keeps its menu: the hold is a finger's alone).
    let menu = move |ev: leptos::ev::MouseEvent| {
        if hold.pressing() {
            ev.prevent_default();
        }
    };
    let bands = |layer: Layer| Signal::derive(move || layer.with(|(_, bands)| bands.clone()));
    // To the week at `to` of `WEEKS`; the ends stay where they are.
    let go = move |to: usize| {
        if let Some(week) = WEEKS.get(to) {
            ctx.weeks.set(*week);
        }
    };
    // A finger that moves sideways further than up or down carries the weeks with it (`drag_x`, in
    // pixels; the page scrolls under any other move), less where no week lies beyond. Let go
    // further than `SWIPE`, the next week or the one before comes, else the week goes back. It
    // ends in a click on what lay under the finger, and that click opens nothing.
    let start = StoredValue::new(None::<(i32, i32)>);
    let carried = StoredValue::new(false);
    let drag_x = RwSignal::new(0i32);
    let dragging = RwSignal::new(false);
    let down = move |ev: leptos::ev::PointerEvent| {
        carried.set_value(false);
        start.set_value(Some((ev.client_x(), ev.client_y())));
        // A mouse or a pen draws here as on a wide week; a finger holds first.
        drag.down(&ev);
        hold.down(&ev);
    };
    let moving = move |ev: leptos::ev::PointerEvent| {
        // A finger drawing a span carries no week.
        if drag.moved(&ev) {
            return;
        }
        hold.moved(&ev);
        // Nor does a finger whose hold is still pending: one of the two has the finger (`Hold`).
        if hold.pending() {
            return;
        }
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
        drag_x.set(if beyond { dx / 3 } else { dx });
    };
    let up = move |ev: leptos::ev::PointerEvent| {
        hold.up();
        if drag.up(&ev) {
            start.set_value(None);
            dragging.set(false);
            drag_x.set(0);
            return;
        }
        let Some((x, y)) = start.get_value() else { return };
        start.set_value(None);
        dragging.set(false);
        drag_x.set(0);
        let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
        if dx.abs() > SWIPE && dx.abs() > dy.abs() {
            carried.set_value(true);
            let now = current.get_untracked();
            go(if dx < 0 { now + 1 } else { now.wrapping_sub(1) });
        }
    };
    let cancel = move |_| {
        hold.up();
        drag.cancel();
        start.set_value(None);
        dragging.set(false);
        drag_x.set(0);
    };
    let click = move |ev: leptos::ev::MouseEvent| {
        if hold.swallow(&ev) | drag.swallow(&ev) {
            return;
        }
        if carried.get_value() {
            carried.set_value(false);
            ev.prevent_default();
            return;
        }
        remember(Within::Grid, &ev);
    };
    // A week of the carousel, named by `label`; the grid alone has no name but the page's.
    let slide = move |at: Signal<i32>, label: Option<&'static str>, slots: Memo<Vec<PlanSlot>>, layer: Layer, week: Weeks| {
        view! {
            <div
                class="sp-slide"
                class:is-current=move || at.get() == 0
                class:is-side=move || at.get().abs() == 1
                class:is-far=move || { at.get().abs() > 1 }
                style=move || format!("--at:{};--drag:{}px", at.get(), drag_x.get())
                role=label.map(|_| "group")
                aria-roledescription=label.map(|_| t.studyplan_week.week_role)
                aria-label=label
                inert=move || (at.get() != 0).then_some("")
            >
                <div class="sp-week">
                    <WeekGrid slots=grid(slots, layer, week) bands=bands(layer) fit=true/>
                </div>
            </div>
        }
    };
    move || {
        if !ab.get() {
            // One week: the grid alone.
            return view! {
                <div
                    class="sp-carousel"
                    node_ref=node
                    class:drawing=move || ctx.sketch.with(Option::is_some)
                    on:pointerdown=move |ev| {
                        drag.down(&ev);
                        hold.down(&ev);
                    }
                    on:pointermove=move |ev| {
                        if !drag.moved(&ev) {
                            hold.moved(&ev);
                        }
                    }
                    on:pointerup=move |ev| {
                        hold.up();
                        drag.up(&ev);
                    }
                    on:pointerleave=move |_| {
                        if !drag.live() {
                            hold.up();
                        }
                    }
                    on:pointercancel=move |_| {
                        hold.up();
                        drag.cancel();
                    }
                    on:lostpointercapture=move |ev| drag.lost(&ev)
                    on:contextmenu=menu
                    on:click=move |ev| {
                        if hold.swallow(&ev) | drag.swallow(&ev) {
                            return;
                        }
                        remember(Within::Grid, &ev);
                    }
                >
                    {slide(Signal::stored(0), None, shown, layer, ctx.weeks.get_untracked())}
                    <super::own::SketchView drag/>
                </div>
            }
            .into_any();
        }
        let slides = WEEKS
            .into_iter()
            .zip(weeks)
            .zip(layers)
            .enumerate()
            .map(|(i, ((week, (_, slots)), layer))| {
                // Its place counted from the week shown: 0 in the middle, ±1 at the sides, ±2 out of
                // sight at the side it lies on.
                let at = Signal::derive(move || place_of(i, current.get()));
                slide(at, Some(week_label(week, t)), slots, layer, week)
            })
            .collect_view();
        view! {
            // A link dragged with a mouse would leave the page's hands (the browser's own drag
            // cancels the pointer): the slots are not dragged.
            <div
                class="sp-carousel"
                node_ref=node
                class:dragging=move || dragging.get()
                // A span drawn on it by a mouse: the pointer of moving (app.css, „A span drawn").
                class:drawing=move || ctx.sketch.with(Option::is_some)
                aria-roledescription=t.studyplan_week.carousel_role
                aria-label=t.studyplan_week.weeks
                on:pointerdown=down
                on:pointermove=moving
                on:pointerup=up
                on:pointerleave=move |ev| {
                    if !drag.live() {
                        up(ev);
                    }
                }
                on:pointercancel=cancel
                on:lostpointercapture=move |ev| drag.lost(&ev)
                on:contextmenu=menu
                on:click=click
                on:dragstart=|ev| ev.prevent_default()
            >
                {slides}
                <super::own::SketchView drag/>
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
/// it carries them before letting go brings another week. While the plan is edited a finger held on
/// the week is the hold's as far as this (`own::HOLD_SLOP`, the same 8 px), and only then the
/// carousel's.
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
    // What the lines name the modules by: a sibling of the timetable (R16).
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    let loose = Memo::new(move |_| {
        let base = base.get();
        titles.with(|titles| ctx.table.with(|table| table.as_ref().map(|table| loose_lines(table, &base, titles, &table.loose(), t)).unwrap_or_default()))
    });
    view! { <Loose title=t.studyplan_week.no_fixed_time lines=loose/> }
}

/// „A-Woche · B-Woche · A/B": which kind of week the Regelwoche shows. The click answers at once
/// (R21): a signal of the page, the week is worked out again in Rust. `short`: on a phone, at the
/// left of the head's toolbar, „A", „B", „A/B" (owner, 2026-10-26: „Dann kann oben in die gleiche
/// Reihe mit dem Anpassen Knopf die Handles. Woche: A, B, A/B"; the word „Woche" went on
/// 2026-11-18, its name says it); the carousel under it follows the switch, and a swipe of the
/// carousel moves it (`PlanCtx::weeks`).
#[component]
pub(super) fn WeekSwitch(weeks: RwSignal<Weeks>, #[prop(optional)] short: bool) -> impl IntoView {
    let t = i18n::t();
    let w = &t.studyplan_week;
    let choice = move |week: Weeks| {
        let brief = match week {
            Weeks::A => w.short_a,
            Weeks::B => w.short_b,
            Weeks::All => w.tab_ab,
        };
        let (label, name) = match (short, week) {
            (false, _) => (week_label(week, t), week_label(week, t)),
            (true, Weeks::A) => (w.short_a, w.tab_a),
            (true, Weeks::B) => (w.short_b, w.tab_b),
            (true, Weeks::All) => (w.tab_ab, w.both_weeks),
        };
        // Beside the week on a wider screen both words stand, and the head shows „A · B · A/B"
        // (the phone's) where it has no room for the long ones, so that it never wraps (app.css).
        view! {
            <button type="button" role="radio" aria-checked=move || if weeks.get() == week { "true" } else { "false" } aria-label=name data-command="" data-command-state=move || (weeks.get() == week).then_some(t.studyplan_keys.shown) on:click=move |_| weeks.set(week)>
                <span class="seg-label">{label}</span>
                {(!short).then(|| view! { <span class="seg-brief" aria-hidden="true">{brief}</span> })}
            </button>
        }
    };
    let switch = view! {
        // One stop of Tab, entered at the week shown, ← → choose (enhance.js, an option group that
        // is a stop).
        <div class="seg hug sp-weeks" role="radiogroup" aria-label=w.week data-stop="x">
            {WEEKS.into_iter().map(choice).collect_view()}
        </div>
    };
    match short {
        // On a phone the switch alone, „A · B · A/B", at the left of the head's toolbar: its name says
        // „Woche" (owner, 2026-11-18, of the head: „eine absolute Katastrophe"; until then the word
        // stood before it).
        true => view! { <div class="sp-weekpick">{switch}</div> }.into_any(),
        false => switch.into_any(),
    }
}

/// The key of the slot button a click hit (`button.slot-act[data-act]`), or of a group of the
/// choice in focus, which is a button itself (`button.slot[data-act]`); `None` for a click
/// anywhere else.
fn act_under(target: Option<leptos::web_sys::EventTarget>) -> Option<String> {
    use leptos::wasm_bindgen::JsCast;
    let element = target?.dyn_into::<leptos::web_sys::Element>().ok()?;
    element.closest(".slot-act, .slot[data-act]").ok()??.get_attribute("data-act")
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
        .any(|row| matches!(row.every, Some(Every::AWeek | Every::BWeek)))
}

/// „Termine": the agenda by week, opened at the current one, and what has no date.
#[component]
pub(super) fn DatesView(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let base = base_of(ctx);
    let today = ctx.today;
    // What the lines name the modules by: a sibling of the timetable (R16).
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    let blocks = Memo::new(move |_| {
        let base = base.get();
        titles.with(|titles| ctx.table.with(|table| table.as_ref().map(|table| agenda_blocks(table, &table.agenda(t.locale), &base, titles, today, t)).unwrap_or_default()))
    });
    let loose = Memo::new(move |_| {
        let base = base.get();
        titles.with(|titles| {
            ctx.table.with(|table| {
                table
                    .as_ref()
                    .map(|table| loose_lines(table, &base, titles, &undated(table), t).into_iter().chain(open_exams(table, &base, t)).collect())
                    .unwrap_or_default()
            })
        })
    });
    // The current week, as the agenda has it now; `None` without a clock.
    let current = move |blocks: &[Block]| today.and_then(|today| scroll_target(blocks, today));

    // The agenda opens once, when it first has something to show: at the current week, or back
    // from a module that filled the page (the address names it still) at the date it was opened
    // from. Later changes of what is shown leave the page where the visitor put it (on a phone the
    // module beside the plan is a sheet over it, and the plan stays where it is under it).
    let done = StoredValue::new(false);
    Effect::new(move |_| {
        if done.get_value() {
            return;
        }
        let Some(week) = blocks.with(|blocks| (!blocks.is_empty()).then(|| current(blocks))) else { return };
        let here = ctx.url.get_untracked();
        done.set_value(true);
        match (&here.open, week) {
            (Some(_), week) => come_back(&here, week, t.locale),
            (None, Some(week)) => reveal(week),
            (None, None) => {}
        }
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
        let picked = |url: &StudyplanUrl| (url.open.clone(), url.row.clone().or_else(|| url.termin.clone()));
        let (open, row) = match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
            Some(search) => picked(&StudyplanUrl::parse(&search)),
            None => ctx.url.with(picked),
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
/// window, below its top bar: the window's `scroll-padding`, app.css „phone and tablet"). `false`
/// if the agenda has no such week.
#[allow(unused_variables)]
fn week_to_top(id: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(element) = folia_design::stage::by_id(id) else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        true
    }
    #[cfg(not(feature = "csr"))]
    false
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
    /// one of its rows, or it is in its own view.
    fn is(&self, (open, row): &Picked) -> bool {
        match (open, row) {
            (Some(open), Some(row)) => self.modules.contains(open) && self.keys.contains(row),
            (None, Some(row)) => self.keys.contains(row),
            _ => false,
        }
    }
}

/// The slots of the Regelwoche held in the week `shown`, by weekday, time and their first Termin's
/// key (so that a faint slot keeps its lane);
/// `titles` are the planned modules' titles by id. While the plan is edited (`edit::on(base)`; the
/// mode shows what the plan leaves out, faint, in its place), what it leaves out besides (`ghost_slots`), in the
/// order of the week. With a choice in focus (`focus`, its event), its groups and the rest behind
/// them (`focus_slots`). In the words of `t`.
fn week_slots(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks, focus: Option<usize>, t: &i18n::Texts) -> Vec<PlanSlot> {
    if let Some(event) = focus {
        return focus_slots(table, event, base, titles, shown, t);
    }
    let items = table.regular_week();
    let mut slots: Vec<PlanSlot> = items.iter().filter(|item| item.in_week(shown)).filter_map(|item| plan_slot(table, base, item, titles, shown, &items, t)).collect();
    if super::edit::on(base) {
        slots.extend(ghost_slots(table, base, titles, shown, &items, t));
    }
    // Slots at one time share the day's lanes in one order, whether the plan shows them or leaves
    // them out (faint): by their first Termin, never by being shown. A Termin hidden by its eye
    // stays in its lane, and the slot beside it does not slide under the pointer.
    slots.sort_by_key(|slot| (slot.slot.day, slot.slot.from, slot.slot.to, slot.row.keys.first().copied()));
    join_ab(slots)
}

/// The slots of the week `shown` as „Als Bild speichern" draws them (`picture.rs`): the plan's, as
/// the week looked at shows them, without what is faint and without a choice in focus; a plan
/// without A or B weeks shows them all whatever the switch said.
pub(super) fn picture_slots(table: &Timetable, titles: &BTreeMap<String, String>, shown: Weeks, t: &i18n::Texts) -> Vec<GridSlot> {
    let shown = if has_ab(table) { shown } else { Weeks::All };
    week_slots(table, &StudyplanUrl::default(), titles, shown, None, t).into_iter().map(|slot| slot.slot).collect()
}

/// The week with a choice in focus (folia/design/kalender; owner, 2026-10-26: „A mit Fokus"): its
/// groups all there, ticked or not, each the box that ticks it (`GridSlot::tick`), outlined and
/// red-marked where it meets the plan; the rest of the plan behind them, without links or buttons
/// (`GridSlot::dim`). A tick acts as a group's „✓" does (`Act::Tick`, `Act::Untick`).
fn focus_slots(table: &Timetable, event: usize, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks, t: &i18n::Texts) -> Vec<PlanSlot> {
    let words = &t.studyplan_week;
    let drawn = choice::focus_table(table, event);
    let Some(real) = table.events.get(event) else { return Vec::new() };
    let Ok(id) = real.id.parse::<u32>() else { return Vec::new() };
    let slots = drawn
        .regular_week()
        .iter()
        .filter(|item| item.in_week(shown))
        .filter_map(|item| {
            let mut slot = drawn_slot(&drawn, base, item, titles, &item.against_in(shown), &item.yields_in(shown), t)?;
            slot.slot.href = None;
            slot.slot.asks = false;
            if item.event != event {
                slot.acts = Vec::new();
                slot.slot.dim = true;
                return Some(keyed(slot, 'p', item));
            }
            let option = drawn.events.get(event)?.rows.get(item.row)?.option?;
            let ticked = real.chosen.contains(&option);
            let keys = real.option_keys(option);
            let row = *keys.first()?;
            let meets = drawn.events.get(event)?.rows.iter().enumerate().filter(|(_, r)| r.option == Some(option)).find_map(|(r, _)| folia_timetable::termin::meets(table, event, r));
            slot.acts = vec![match ticked {
                true => SlotAct::taken(words.drop_group, Act::Untick(keys.clone())),
                false => SlotAct::take(words.take_group, Act::Tick { event: id, row, rows: keys }),
            }];
            slot.slot.alt = false;
            // A group of a visit gives way where it meets the plan: yellow, never a clash's red.
            let visit = drawn.events.get(event).is_some_and(|event| event.visit);
            slot.slot.clash = meets.is_some() && !visit;
            slot.slot.gives = meets.is_some() && visit;
            slot.slot.pick = Some(ticked);
            slot.slot.title = format!("{} · {}", slot.slot.title, if ticked { words.ticked_group } else { words.tick_group });
            let mut slot = keyed(slot, 'f', item);
            slot.slot.tick = Some(format!("{}/0", slot.key));
            slot.slot.acts = Vec::new();
            Some(slot)
        })
        .collect();
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
/// and while the plan is edited with its buttons (`plan_acts`; while looking nothing on a slot
/// changes the plan). `items` is the plan's whole Regelwoche, which says how many slots the slot's
/// event has.
fn plan_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem, titles: &BTreeMap<String, String>, week: Weeks, items: &[WeekItem], t: &i18n::Texts) -> Option<PlanSlot> {
    let mut slot = drawn_slot(table, base, item, titles, &item.against_in(week), &item.yields_in(week), t)?;
    if !super::edit::on(base) {
        return Some(keyed(slot, 'p', item));
    }
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
/// events it overlaps, which draw it red and which its tooltip names, and of a visit `gives` the
/// plan's events it gives way to, a yellow edge and a line of its tooltip (owner, 2026-10-26: „ein
/// Besuch gibt bei einer Überschneidung nach"). Its link is the app's path (`linked` writes it in
/// the page's language).
fn drawn_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem, titles: &BTreeMap<String, String>, against: &[usize], gives: &[usize], t: &i18n::Texts) -> Option<PlanSlot> {
    let event = table.events.get(item.event)?;
    let shown = event.rows.get(item.row)?;
    // A Termin of one's own belongs to no module; it has its key.
    let module = event.modules.first();
    let every = shown.every;
    let once = matches!(item.label, WeekLabel::Once { .. });
    let rows: Vec<&Row> = slot_rows(event, item, every).into_iter().filter_map(|r| event.rows.get(r)).collect();
    // An option of an open choice: how many to take of how many („1 von 3").
    let take = event.across.filter(|alt| alt.open).map_or(1, |alt| alt.take);
    let notes = qualifiers(&item.label, every, item.alt.map(|(options, _)| (take, options)), &table.facts, t);
    let hue = hue(event.tone);
    // A click opens the Termin, in both states, a hatched group and a faint slot too (owner,
    // 2026-11-17: „dass wenn man auf einen termin klickt, dass dann infos kommen die halt nicht nur
    // für den Termin sind. Das muss auch angepasst werden so dass das mehr wie bei google
    // calendar."): one click, one meaning. A choice is taken into focus from
    // „Offen" and from the Termin's card. One without a key opens its module.
    let href = match shown.key {
        Some(key) => base.with_termin(Some((key, None))).path(),
        None => base.with_open(Some(module?.as_str()), None).path(),
    };
    let place = rooms(rows.iter().map(|row| &row.date));

    // The tooltip says in full what the slot shortens: the kind as QIS names it, the whole time,
    // the rooms, and the dates of single ones and of a series of single dates (its range spans
    // the weeks it leaves out).
    let mut title = vec![event_text(event, titles, t.locale), format!("{} {}–{}", day_short(item.day, t), clock(item.from), clock(item.to))];
    title.extend(notes.iter().cloned());
    title.extend(rooms_long(rows.iter().map(|row| &row.date)));
    let series = !once && rows.iter().any(|row| row.date.series.is_some());
    if series || matches!(item.label, WeekLabel::Once { dates, .. } if dates > 1) {
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
    if item.extra {
        title.push(t.studyplan_week.extra.to_string());
    }
    if item.visit {
        title.push(t.studyplan_week.visit.to_string());
    }
    let own = event.own.is_some();
    if own {
        title.push(t.studyplan_own.by_you.to_string());
    }
    // While the plan is edited, a series some of whose dates the student hid carries a mark in its
    // corner, as the pen of a changed one has its own: a single date has no slot of its own.
    let hidden_dates = folia_timetable::views::hidden_days(rows.iter().copied()).into_iter().filter(|day| day.weekday() == item.day).count();
    let hides = hidden_dates > 0 && super::edit::on(base);
    if hides {
        title.push((t.studyplan_edit.dates_hidden)(hidden_dates));
    }
    // A Termin of QIS changed by hand: the pen, amber where QIS says something else now.
    let edited = rows.iter().any(|row| row.changed.is_some());
    let conflict = rows.iter().any(|row| row.changed.as_ref().is_some_and(|changed| changed.differs));
    if edited {
        title.push(if conflict { t.studyplan_own.changed_differs } else { t.studyplan_own.changed }.to_string());
    }
    let mut yielded: Vec<String> = Vec::new();
    for other in gives.iter().filter_map(|e| table.events.get(*e)) {
        let label = slot_label(other, titles, t.locale);
        if !yielded.contains(&label) {
            yielded.push(label);
        }
    }
    let yields = !yielded.is_empty();
    if yields {
        title.push((t.studyplan_week.gives_way)(&yielded.join(", ")));
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
        mark: None,
        alt: item.alt.is_some(),
        clash,
        extra: item.extra,
        visit: item.visit,
        gives: yields,
        href: Some(href.clone()),
        lines: Vec::new(),
        current: false,
        acts: Vec::new(),
        asks: false,
        joint: None,
        pick: None,
        tick: None,
        dim: false,
        hides,
        badge: None,
        own,
        edited,
        conflict,
        // The Stundenplan draws a choice open hatched and a group chosen as any (`alt`, its focus);
        // the module's page alone marks groups (folia/design/kalender/raster.html).
        group: false,
        chosen: false,
    };
    let row = DayRow {
        day: item.day,
        class: classes(hue, &[(item.alt.is_some(), "alt"), (clash, "clash"), (item.extra, "extra"), (item.visit, "visit"), (yields, "gives"), (own, "own"), (edited, "edited"), (conflict, "conflict"), (hides, "hides")]),
        time: format!("{}–{}", clock(item.from), clock(item.to)),
        text: event_text(event, titles, t.locale),
        note: place.into_iter().chain(notes).collect::<Vec<_>>().join(" · "),
        href,
        modules: event.modules.clone(),
        keys: rows.iter().filter_map(|row| row.key).collect(),
        acts: Vec::new(),
    };
    Some(PlanSlot { slot, row, key: String::new(), acts: Vec::new(), weeks: item.weeks })
}

/// The slots of a calendar of one's own in the week `shown` (`calendars.rs`): its Termine drawn
/// as the plan's are, in its colour, without links or buttons; in full outlined with its initial
/// (`cal`), as a strip at the edge of each day (`strip`) only what stands firm (no group of a
/// choice still open, no extra). Behind a choice in focus they step back with the plan (`dim`).
/// Their tooltip begins with the calendar's name and says they are to look at only („Lena · nur zum
/// Ansehen"): a click on one opens nothing yet.
pub(super) fn calendar_slots(table: &Timetable, titles: &BTreeMap<String, String>, shown: Weeks, calendar: &super::calendars::Shown, dim: bool, t: &i18n::Texts) -> Vec<GridSlot> {
    let strip = calendar.view == CalendarView::Strip;
    let base = StudyplanUrl::default();
    table
        .regular_week()
        .iter()
        .filter(|item| item.in_week(shown) && (!strip || (item.alt.is_none() && !item.extra)))
        .filter_map(|item| {
            let slot = drawn_slot(table, &base, item, titles, &[], &[], t)?.slot;
            // A Termin of one's own is one's own in whichever calendar („Chor · in „Lena“ · von
            // dir"); what came with the calendar is there to look at.
            let own = table.events.get(item.event).is_some_and(|event| event.own.is_some());
            let said = slot.title.replace(t.studyplan_own.changed_differs, t.studyplan_own.changed_elsewhere).replace(t.studyplan_own.changed, t.studyplan_own.changed_elsewhere);
            let title = match (own, said.split_once(" · ")) {
                (true, Some((name, rest))) => format!("{name} · {} · {rest}", (t.studyplan_calendars.own_in)(&calendar.name)),
                (true, None) => format!("{said} · {}", (t.studyplan_calendars.own_in)(&calendar.name)),
                (false, _) => format!("{} · {said}", (t.studyplan_calendars.look_only)(&calendar.name)),
            };
            Some(GridSlot {
                class: if strip { "strip" } else { "cal" },
                hue: Some(calendar.hue),
                mark: None,
                href: None,
                title,
                clash: false,
                // Its changes came with its link, which carries nothing of what QIS said: none of
                // them asks to be decided, the pen alone says they are changed.
                conflict: false,
                dim,
                badge: (!strip).then(|| calendar.initial.clone()),
                ..slot
            })
        })
        .collect()
}

/// While the plan is edited: the slots of what the plan leaves out, held in the week `shown` — a hidden
/// event, kind or Termin, an option another was chosen over (the other town's course stays out).
/// Each is faint (`faint`) and says why in its tooltip; it is red-edged where it would meet a slot
/// of the plan (`items`, the plan's Regelwoche) in a week both are held in, and its buttons, in
/// view, take it back (`ghost_acts`). Its link opens its card, which says why.
fn ghost_slots(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks, items: &[WeekItem], t: &i18n::Texts) -> Vec<PlanSlot> {
    let all = unhidden(table);
    let plan: Vec<&WeekItem> = items.iter().filter(|item| item.in_week(shown)).collect();
    all.regular_week()
        .iter()
        .filter(|item| item.in_week(shown))
        .filter_map(|item| {
            let event = all.events.get(item.event)?;
            let every = event.rows.get(item.row)?.every;
            // Left out: every Termin of the slot is hidden in the plan. Why the first one is.
            let hidden = table.events.get(item.event)?;
            let reasons: Vec<HiddenBy> = slot_rows(event, item, every).into_iter().map(|r| hidden.rows.get(r)?.hidden).collect::<Option<_>>()?;
            let reason = *reasons.first()?;
            // What it would meet of the plan: a visit's Termin gives way, so it meets nothing, and
            // one of a visit gives way where it meets the plan (yellow, not red).
            let meets: Vec<usize> = plan
                .iter()
                .filter(|other| other.event != item.event && !other.visit && other.day == item.day && other.from < item.to && item.from < other.to)
                .filter(|other| same_weeks(other.weeks, item.weeks))
                .map(|other| other.event)
                .collect();
            let (against, gives) = if event.visit { (Vec::new(), meets) } else { (meets, Vec::new()) };
            let mut slot = drawn_slot(&all, base, item, titles, &against, &gives, t)?;
            let why = reason_text(reason, hidden, t);
            let hue = slot.slot.hue.unwrap_or_default();
            slot.slot.class = "faint";
            slot.slot.alt = false;
            slot.slot.asks = true;
            // What the plan leaves out meets nothing: no red edge and no „!" on a faint slot, the
            // loudest thing of the week otherwise; its tooltip still says what it would meet.
            slot.slot.clash = false;
            slot.slot.title = format!("{why} · {}", slot.slot.title);
            slot.row.class = classes(hue, &[(true, "faint"), (slot.slot.gives, "gives")]);
            slot.row.note = std::iter::once(why).chain(Some(slot.row.note.clone()).filter(|note| !note.is_empty())).collect::<Vec<_>>().join(" · ");
            slot.acts = ghost_acts(table, hidden, item, &slot.row.keys, reason, t);
            Some(keyed(slot, 'g', item))
        })
        .collect()
}

/// The timetable with what the plan hides shown again, but another course of a module: every Termin
/// of the planned modules, as the mode draws them faint. Its events and rows are the plan's, at
/// the same places.
fn unhidden(table: &Timetable) -> Timetable {
    let town = |hidden: Option<HiddenBy>| hidden.filter(|by| matches!(by, HiddenBy::Town(_) | HiddenBy::Course(_)));
    let mut all = table.clone();
    for event in &mut all.events {
        event.hidden = town(event.hidden);
        event.chosen.clear();
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
        HiddenBy::Option => words.other_option_chosen.to_string(),
        HiddenBy::Program => words.other_programs.to_string(),
        HiddenBy::Event | HiddenBy::Row | HiddenBy::Town(_) | HiddenBy::Course(_) => words.hidden.to_string(),
    }
}

// ---------- the buttons of a slot ----------

/// A button of a slot while the plan is edited (owner, 2026-09-25: „ja der fliegt raus, die Übung
/// möchte ich", on the Termin itself): „✓" takes, „+" takes beside what was taken, the eye says
/// what is and changes it (owner, 2026-11-17: „Ich will übrigens dass wir vom x weg kommen und eher
/// ein augen symbol verwneden"): `eye` on what the week shows, a click hides it, `eye-off` on what
/// is faint, a click shows it; the bin deletes a Termin of one's own („da wo es nur ausblendet das
/// auge und da wo es entfernt eine Mülltonne"). Its words (its name and tooltip) say what. A „✓"
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

    /// The eye of a Termin the week shows: a click hides it.
    fn hide(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "eye", label: label.into(), act, pressed: false }
    }

    /// The crossed eye of a Termin the plan leaves out: a click shows it again.
    fn show(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "eye-off", label: label.into(), act, pressed: false }
    }

    /// The bin of a Termin of one's own: a click deletes it.
    fn remove(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "trash-2", label: label.into(), act, pressed: false }
    }

    /// „+": takes it beside what was taken.
    fn also(label: impl Into<String>, act: Act) -> Self {
        SlotAct { icon: "plus", label: label.into(), act, pressed: false }
    }
}

/// What a button of a slot does to what the semester shows (the plan's `Selection`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Act {
    /// Hides the slot's Termine.
    HideRows(Vec<RowKey>),
    /// Hides the whole event.
    HideEvent(u32),
    /// Takes the slot's option of the event's choice instead of what it took („Nur diesen", `row`
    /// a Termin of it): the event and the option's Termine shown again where they were hidden.
    Choose { event: u32, row: RowKey, rows: Vec<RowKey> },
    /// Ticks the slot's option beside those ticked (owner, 2026-10-26: several groups, all alike),
    /// shown again as `Choose`.
    Tick { event: u32, row: RowKey, rows: Vec<RowKey> },
    /// Takes the tick of an option away, by the keys of its Termine (`Event::option_keys`): the
    /// others ticked stay, and without any the choice is open again.
    Untick(Vec<RowKey>),
    /// Leaves out an option ticked: its tick taken away (`ticks`), the slot's Termine hidden.
    Drop { ticks: Vec<RowKey>, rows: Vec<RowKey> },
    /// Shows the slot again: its event, a kind of it, its Termine.
    Show { event: Option<u32>, kind: Option<EventKind>, rows: Vec<RowKey> },
    /// Takes the slot's event, an option of the choice among its module's events, by a Termin of
    /// it (`row`): the choices of `events` undone first (all of the choice's where it takes this
    /// one instead of what it took, „Nur diese"; none where it is taken beside, „+", or the choice
    /// is open), the event and the slot's Termine shown again.
    ChooseAcross { events: Vec<u32>, event: u32, row: RowKey, rows: Vec<RowKey> },
    /// Takes the event, an option the visitor took, back alone: the others taken stay, and
    /// without any the choice is open again.
    UnchooseAcross(u32),
    /// Takes the event back as `UnchooseAcross` and leaves it out.
    DropAcross(u32),
    /// Deletes the Termin of one's own of this number (the page does it through `own::remove`, with
    /// the note and „Rückgängig").
    RemoveOwn(u32),
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
            Act::Tick { event, row, rows } => {
                doc.set_event(key, *event, false);
                rows.iter().for_each(|row| doc.set_row(key, *row, false));
                doc.tick(key, *row, true);
            }
            Act::Untick(ticks) => ticks.iter().for_each(|row| doc.tick(key, *row, false)),
            Act::Drop { ticks, rows } => {
                ticks.iter().for_each(|row| doc.tick(key, *row, false));
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
            Act::ChooseAcross { events, event, row, rows } => {
                events.iter().for_each(|other| doc.choose(key, *other, None));
                doc.set_event(key, *event, false);
                rows.iter().for_each(|row| doc.set_row(key, *row, false));
                doc.choose(key, *event, Some(*row));
            }
            Act::UnchooseAcross(event) => doc.choose(key, *event, None),
            Act::DropAcross(event) => {
                doc.choose(key, *event, None);
                doc.set_event(key, *event, true);
            }
            Act::RemoveOwn(number) => {
                doc.remove_own(*number);
            }
        }
    }
}

/// The choice among its module's events that `event` is an option of (`Timetable::choices`): the
/// ids of all its events, the choice, and what the event is of it.
fn across_of<'t>(table: &'t Timetable, event: &Event) -> Option<(Vec<u32>, &'t Across, Alt)> {
    let alt = event.across?;
    let across = table.choices.get(alt.choice)?;
    let events = across.options.iter().flatten().filter_map(|index| table.events.get(*index)?.id.parse::<u32>().ok()).collect();
    Some((events, across, alt))
}

/// The buttons of a slot of the plan. An option of an open choice: „✓" takes it, the eye leaves it
/// out. An option ticked: its „✓", on, takes its tick back (owner, 2026-09-25: „schnell
/// abwählen"), the others ticked stay, without any the choice is open again; its eye leaves it
/// out besides. A Termin of one's own: the bin, which deletes it (it is not hidden: the bin where
/// something is removed, the eye where it is only hidden, owner 2026-11-17). Any other
/// Termin: the eye leaves it out, and an event of one slot (`slots`) as a whole. `keys` are the
/// slot's Termine.
fn plan_acts(table: &Timetable, item: &WeekItem, keys: &[RowKey], slots: usize, t: &i18n::Texts) -> Vec<SlotAct> {
    let words = &t.studyplan_week;
    let Some(event) = table.events.get(item.event) else { return Vec::new() };
    // A Termin of one's own is not hidden but deleted: the bin, never an eye.
    if let Some(own) = event.own.as_ref() {
        return vec![SlotAct::remove(words.delete_own, Act::RemoveOwn(own.number))];
    }
    let Ok(id) = event.id.parse::<u32>() else { return Vec::new() };
    let shown = event.rows.get(item.row);
    let (option, row) = (shown.and_then(|row| row.option), shown.and_then(|row| row.key));
    // An option of a choice among the module's events: taken („✓") or left out (the eye) as a whole
    // event, beside what the open choice took so far; one the visitor took takes itself back.
    if let Some((_, across, alt)) = across_of(table, event) {
        let chosen = across.by == Some(TakenBy::Chosen) && across.taken.contains(&alt.option);
        match row {
            Some(row) if alt.open => {
                return vec![
                    SlotAct::take(words.take_option, Act::ChooseAcross { events: Vec::new(), event: id, row, rows: keys.to_vec() }),
                    SlotAct::hide(hide_event(event, t), Act::HideEvent(id)),
                ];
            }
            _ if chosen => {
                return vec![
                    SlotAct::taken(words.drop_option, Act::UnchooseAcross(id)),
                    SlotAct::hide(hide_event(event, t), Act::DropAcross(id)),
                ];
            }
            _ => {}
        }
    }
    match (item.alt, row) {
        (Some(_), Some(row)) => vec![
            SlotAct::take(words.take_group, Act::Choose { event: id, row, rows: keys.to_vec() }),
            SlotAct::hide(words.not_this_group, Act::HideRows(keys.to_vec())),
        ],
        _ if option.is_some_and(|option| event.chosen.contains(&option)) => {
            let ticks = option.map(|option| event.option_keys(option)).unwrap_or_default();
            vec![SlotAct::taken(words.drop_group, Act::Untick(ticks.clone())), SlotAct::hide(words.not_this_group, Act::Drop { ticks, rows: keys.to_vec() })]
        }
        _ if slots > 1 && !keys.is_empty() => vec![SlotAct::hide(words.hide_session, Act::HideRows(keys.to_vec()))],
        _ => vec![SlotAct::hide(hide_event(event, t), Act::HideEvent(id))],
    }
}

/// The buttons of a slot the plan leaves out. An option of a choice another option of which is
/// ticked is taken instead with „✓" („Nur diese Gruppe"), or with „+" beside it („Auch diese
/// Gruppe", owner, 2026-10-26: several groups, all alike); an option of an open choice is taken
/// („Diese Gruppe nehmen"); a kind switched off comes back with every event of it, by the crossed
/// eye („„Übung“ wieder einblenden"); any other Termin is shown again by it, its event with it
/// („Einblenden"). `event` is the plan's, `keys` the slot's Termine.
fn ghost_acts(table: &Timetable, event: &Event, item: &WeekItem, keys: &[RowKey], reason: HiddenBy, t: &i18n::Texts) -> Vec<SlotAct> {
    let words = &t.studyplan_week;
    let id = event.id.parse::<u32>().ok();
    let shown = event.rows.get(item.row);
    let (option, row) = (shown.and_then(|row| row.option), shown.and_then(|row| row.key));
    // Another option of a choice among the module's events, left out by a choice or the program:
    // its „✓" takes it instead (owner, 2026-10-19, the concept: „Ein Tipp auf einen anderen Termin
    // wählt um"), and of one the visitor made „+" takes it beside.
    if let (HiddenBy::Option | HiddenBy::Program, Some(number), Some(row)) = (reason, id, row) {
        if let Some((events, ..)) = across_of(table, event) {
            let instead = Act::ChooseAcross { events, event: number, row, rows: keys.to_vec() };
            return match reason {
                HiddenBy::Option => vec![
                    SlotAct::take(words.only_option, instead),
                    SlotAct::also(words.also_option, Act::ChooseAcross { events: Vec::new(), event: number, row, rows: keys.to_vec() }),
                ],
                _ => vec![SlotAct::take(words.take_option, instead)],
            };
        }
    }
    match (reason, id, row) {
        (HiddenBy::Kinds, _, _) => {
            let kind = event.kinds.iter().next();
            vec![SlotAct::show((words.show_kind)(kind.map_or("", |kind| kind.label(t.locale))), Act::Show { event: None, kind, rows: keys.to_vec() })]
        }
        (HiddenBy::Choice, Some(number), Some(row)) if option.is_some() => vec![
            SlotAct::take(words.only_group, Act::Choose { event: number, row, rows: keys.to_vec() }),
            SlotAct::also(words.also_group, Act::Tick { event: number, row, rows: keys.to_vec() }),
        ],
        // A group of an open choice the student hid (its eye, or its event's): the crossed eye
        // brings it back as it was (hatched, the choice still open), and „✓" beside it takes it.
        // The eye says what is (owner, 2026-11-17: „eher ein augen symbol verwneden"), so a group
        // hidden is never shown again only by deciding the choice.
        (HiddenBy::Row | HiddenBy::Event, Some(number), Some(row)) if option.is_some() => vec![
            SlotAct::take(words.take_group, Act::Choose { event: number, row, rows: keys.to_vec() }),
            SlotAct::show(words.show_again, Act::Show { event: id.filter(|_| reason == HiddenBy::Event), kind: None, rows: keys.to_vec() }),
        ],
        (_, Some(number), Some(row)) if option.is_some() => vec![SlotAct::take(words.take_group, Act::Choose { event: number, row, rows: keys.to_vec() })],
        _ => vec![SlotAct::show(words.show_again, Act::Show { event: id.filter(|_| reason == HiddenBy::Event), kind: None, rows: keys.to_vec() })],
    }
}

/// Whether the week has nothing to draw: no module, no Termin of one's own and no calendar of one's
/// own in it (one that is off draws nothing).
fn nothing_to_draw(modules: usize, own: usize, calendar_in_week: bool) -> bool {
    modules == 0 && own == 0 && !calendar_in_week
}

/// Whether the card „Noch keine Termine" stands in the week: where it has nothing to draw, and only
/// while it is looked at; while editing the week is drawn bare, where a click makes a Termin.
fn empty_card(nothing: bool, editing: bool) -> bool {
    nothing && !editing
}

/// What the eye of a whole event calls it, and whether that is Betula's own word (which English
/// says with its article): in German QIS's word („Vorlesung ausblenden", „Laborausbildung
/// ausblenden"), in another language the kind's word of that language, which QIS does not write
/// („Hide the lecture", not „Hide Vorlesung"); QIS's word as written where it names no kind Betula
/// knows but „Sonstiges" („Hide Kolloquium", never „Hide the other").
fn eye_word(event: &Event, locale: Locale) -> (String, bool) {
    let first = event.kinds.iter().find(|kind| *kind != EventKind::Other);
    match (locale, first) {
        (Locale::De, _) | (_, None) => (kind_word(event, locale), false),
        (_, Some(kind)) => (kind.label(locale).to_lowercase(), true),
    }
}

/// The words of the eye of a whole event („Vorlesung ausblenden", "Hide the lecture", "Hide
/// Kolloquium").
fn hide_event(event: &Event, t: &i18n::Texts) -> String {
    let (word, own) = eye_word(event, t.locale);
    (t.studyplan_week.hide_event)(&word, own)
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
        .filter(|(_, row)| row.every == every)
        .filter(|(_, row)| match every {
            Some(_) => row.slot_day() == Some(item.day),
            None => row.occ.days.iter().any(|day| day.weekday() == item.day),
        })
        .map(|(r, _)| r)
        .collect()
}

/// What a slot says besides its time: that it is an option of a choice not yet made (`choice`: how
/// many to take of how many), the weeks it meets in, the part of the lecture period, or how many
/// single dates it gathers.
fn qualifiers(label: &WeekLabel, every: Option<Every>, choice: Option<(usize, usize)>, facts: &SemesterFacts, t: &i18n::Texts) -> Vec<String> {
    let words = &t.studyplan_week;
    let mut parts = Vec::new();
    if let Some((take, options)) = choice {
        parts.push((words.one_of)(take, options));
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
        Some(Every::ThreeWeeks) => format!("{time} · {}", t.studyplan_week.every_three_weeks),
        Some(Every::FourWeeks) => format!("{time} · {}", t.studyplan_week.every_four_weeks),
        Some(Every::Week) | None => time,
    }
}

/// The weeks a Termin meets in, in words: „A-Woche", „B-Woche", „3-wöch.", „4-wöch."; nothing for
/// every week.
fn rhythm_word(every: Every, t: &i18n::Texts) -> Option<String> {
    let words = &t.studyplan_week;
    match every {
        Every::Week => None,
        Every::AWeek => Some(words.in_week_a.to_string()),
        Every::BWeek => Some(words.in_week_b.to_string()),
        Every::ThreeWeeks => Some(words.every_three_weeks.to_string()),
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
/// finger, with its buttons. Days and rows are kept by what they are, so that a button tapped
/// changes its own row alone: rebuilt, a whole day took the browser's hold of the scroll with it,
/// and a bin threw the page 600 px under the finger.
#[component]
fn DayList(slots: Memo<Vec<PlanSlot>>, picked: Memo<Picked>) -> impl IntoView {
    let t = i18n::t();
    let groups = Memo::new(move |_| slots.with(|slots| day_groups(slots.iter().map(|slot| slot.row.clone()))));
    let days = Memo::new(move |_| groups.with(|groups| groups.iter().map(|(day, _)| *day).collect::<Vec<u8>>()));
    let row = move |row: DayRow| {
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
    };
    let day = move |day: u8| {
        let rows = Memo::new(move |_| groups.with(|groups| groups.iter().find(|(of, _)| *of == day).map(|(_, rows)| rows.clone()).unwrap_or_default()));
        view! {
            <section>
                <h3 class="label">{weekday_name(day, t)}</h3>
                <For each=move || rows.get() key=|row| row.clone() children=row/>
            </section>
        }
    };
    view! {
        <div class="sp-daylist" on:click=|ev| remember(Within::Days, &ev)>
            <For each=move || days.get() key=|day| *day children=day/>
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
    /// A Termin of one's own: the mark „von dir" after its name, as its slot has.
    own: bool,
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
fn agenda_blocks(table: &Timetable, weeks: &[AgendaWeek], base: &StudyplanUrl, titles: &BTreeMap<String, String>, today: Option<Day>, t: &i18n::Texts) -> Vec<Block> {
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
                    items.extend(agenda_item(table, base, titles, day.day, item, t));
                    continue;
                };
                let dates = choices.get(&e).map_or(&[][..], Vec::as_slice);
                // A cancelled date is no day to attend, unless the week has no other.
                if (item.cancelled.is_none() || dates.is_empty()) && !said.contains_key(&e) {
                    said.insert(e, day.day);
                    items.extend(choice_item(table, base, titles, e, dates, t));
                } else if is_today && said.get(&e) != Some(&day.day) {
                    // Today keeps its line: the options that meet today stand as they are
                    // („1 von 4 · LG 10/214"), below the week's line on an earlier day.
                    items.extend(agenda_item(table, base, titles, day.day, item, t));
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
            head.push_str(t.timetable_data.lecture_break);
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

fn agenda_item(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, day: Day, item: &AgendaItem, t: &i18n::Texts) -> Option<ItemLine> {
    match (item.event, item.exam) {
        (Some(e), _) => teaching_item(table, base, titles, day, item, e, t),
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
fn choice_item(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, e: usize, dates: &[(Day, &AgendaItem)], t: &i18n::Texts) -> Option<ItemLine> {
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
        true => format!("{} · {}", (words.one_of)(1, options), words.cancelled),
        false => format!("{} ({})", (words.one_of)(1, options), days.join(", ")),
    };
    let text = event_text(event, titles, t.locale);
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
        href: match (first.and_then(|row| row.key), dates.first()) {
            (Some(key), Some((day, _))) => base.with_termin(Some((key, Some(*day)))).path(),
            (key, _) => base.with_open(Some(module.as_str()), key).path(),
        },
        warn: false,
        own: false,
    })
}

fn teaching_item(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, day: Day, item: &AgendaItem, e: usize, t: &i18n::Texts) -> Option<ItemLine> {
    let words = &t.studyplan_week;
    let event = table.events.get(e)?;
    let first = event.rows.get(item.row)?;
    // A Termin of one's own belongs to no module of the timetable: it leads to its view alone.
    let module = event.modules.first();
    let own = event.own.is_some();
    let rows: Vec<&Row> = item.rows.iter().filter_map(|r| event.rows.get(*r)).collect();
    let alt = event.unresolved() && first.option.is_some();
    let time = match (item.from, item.to) {
        (Some(from), Some(to)) => format!("{}–{}", clock(from), clock(to)),
        _ if rows.iter().any(|row| row.occ.all_day) => words.all_day.to_string(),
        _ => words.time_open.to_string(),
    };
    let text = event_text(event, titles, t.locale);
    let rooms = rooms(rows.iter().map(|row| &row.date));

    let mut small = Vec::new();
    if alt {
        small.push((words.one_of)(1, event.visible_options().len()));
    }
    // An option of an open choice among the module's events.
    if let Some(alt) = event.across.filter(|alt| alt.open) {
        let options = table.choices.get(alt.choice).map_or(0, |across| across.choosable(&table.events).len());
        small.push((words.one_of)(alt.take, options));
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
    if own {
        title.push(t.studyplan_own.by_you.to_string());
    }
    Some(ItemLine {
        class: classes(hue(event.tone), &[(alt, "alt"), (item.cancelled.is_some(), "cancelled"), (own, "own")]),
        time,
        text,
        small: small.join(" · "),
        title: title.join(" · "),
        href: match (first.key, module) {
            (Some(key), _) => base.with_termin(Some((key, Some(day)))).path(),
            (None, Some(module)) => base.with_open(Some(module.as_str()), None).path(),
            (None, None) => return None,
        },
        warn: false,
        own,
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
    let text = kind_and_title(t.timetable_data.exam, &exam.title);
    let rooms = rooms(item.rows.iter().filter_map(|r| exam.rows.get(*r)).map(|row| &row.date));
    let small: Vec<String> = (first.rank == 2).then(|| t.timetable_data.second_sitting.to_string()).into_iter().chain(rooms).collect();
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
        own: false,
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
            <span>{item.text}{item.own.then(|| view! { <b class="ag-own" aria-hidden="true"><Icon name="user-round"/></b> })}</span>
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
fn loose_lines(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, entries: &[(usize, Option<usize>)], t: &i18n::Texts) -> Vec<LooseLine> {
    entries
        .iter()
        .filter_map(|(e, r)| {
            let event = table.events.get(*e)?;
            let module = event.modules.first()?;
            let row = r.and_then(|r| event.rows.get(r));
            Some(LooseLine {
                hue: hue(event.tone),
                text: event_text(event, titles, t.locale),
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
                text: kind_and_title(t.timetable_data.exam, &exam.title),
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
/// in `locale`), then its title; a Termin of one's own by its name, else by its module as `titles`
/// name it.
fn event_text(event: &Event, titles: &BTreeMap<String, String>, locale: Locale) -> String {
    if let Some(own) = &event.own {
        return super::own::shown_name(own, titles, i18n::texts(locale));
    }
    kind_and_title(&type_text(event, locale), &event.title)
}

/// A slot's label, its first line (owner, 2026-09-25: „<type> <short-tag>"): the kinds in their
/// few letters, then what `titles` names the event's module by — in the plan its abbreviation,
/// else its short name (`StudyplanData::slot_names`): „VL EvS", „Prak Programmierpraktikum",
/// „Ü Mathematik IT-1". The event's own title where the plan's data has no title of its module.
fn slot_label(event: &Event, titles: &BTreeMap<String, String>, locale: Locale) -> String {
    // A Termin of one's own by its name, else by the module it belongs to, else „Eigener Termin".
    if let Some(own) = &event.own {
        return super::own::shown_name(own, titles, i18n::texts(locale));
    }
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
    t.data.weekday(i64::from(day)).unwrap_or_default()
}

/// „Mo" for 1, "Mon" in English; nothing for what is no weekday.
fn day_short(day: u8, t: &i18n::Texts) -> &'static str {
    t.data.weekday_short(i64::from(day)).unwrap_or_default()
}

/// How long (ms) the week takes no click after the bin of a slot deleted a Termin of one's own.
#[cfg(feature = "csr")]
const HUSH_MS: u64 = 500;

/// After the bin of a slot deleted a Termin of one's own, the slot beside it takes the lane it
/// stood in, its eye under the pointer: for half a second a click on the week (or the day list of
/// a phone) does nothing — it hides no neighbour — and does not take the note with „Rückgängig"
/// away either (the note hears clicks on the document; this listener on the window comes first).
fn hush_after_bin() {
    #[cfg(feature = "csr")]
    {
        use leptos::wasm_bindgen::closure::Closure;
        use leptos::wasm_bindgen::JsCast;
        let Some(window) = leptos::web_sys::window() else { return };
        let hush = Closure::<dyn Fn(leptos::web_sys::Event)>::new(|ev: leptos::web_sys::Event| {
            let on_week = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()).is_some_and(|element| element.closest(".sp-week, .sp-days").ok().flatten().is_some());
            if on_week {
                ev.stop_immediate_propagation();
                ev.prevent_default();
            }
        });
        if window.add_event_listener_with_callback_and_bool("click", hush.as_ref().unchecked_ref(), true).is_err() {
            return;
        }
        set_timeout(
            move || {
                let _ = window.remove_event_listener_with_callback_and_bool("click", hush.as_ref().unchecked_ref(), true);
            },
            std::time::Duration::from_millis(HUSH_MS),
        );
    }
}

#[cfg(test)]
pub(super) mod tests {
    use folia_calendar::day::holidays;
    use folia_calendar::select::{Selection, TownChoice};
    use folia_calendar::semester::SemesterKey;
    use folia_model::labels::Code;
    use folia_model::rows_detail::{ChoiceOption, DateRow};
    use folia_timetable::model::Input;
    use folia_timetable::views::AgendaDay;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    pub(crate) fn key() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    /// 2026W as the snapshot derives it: lectures 05.10.2026–31.01.2027, a break over the turn
    /// of the year, A weeks from the first.
    pub(crate) fn winter() -> SemesterFacts {
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
    pub(crate) fn row(event: &str, ord: i64, kind: &str, rhythm: &str, weekday: i64, time: (&str, &str), dates: (&str, &str)) -> DateRow {
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
                series: None,
                track: None,
                choice: None,
                option: None,
                extra: false,
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
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule, exams, sws: &[], taken: &[], choices: &[], listed: &[] };
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

    /// A Termin of one's own stands in „Termine" as in the week: by its name, its place under it,
    /// the mark „von dir", leading to its view; a plan of one alone has its agenda.
    #[test]
    fn a_termin_of_ones_own_stands_in_the_agenda_with_its_mark() {
        use folia_calendar::own::{OwnRhythm, OwnTermin};
        let facts = winter();
        let input = Input { key: key(), semester: None, facts: &facts, modules: &[], schedule: &[], exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
        let chor = OwnTermin {
            number: 1,
            rhythm: OwnRhythm::Week,
            weekday: 2,
            dates: Vec::new(),
            from: 17 * 60,
            to: 19 * 60,
            period: None,
            module: None,
            name: "Chor".into(),
            place: "Aula".into(),
            skip: Vec::new(),
            kind: None,
        };
        let table = Timetable::build(&input, &Selection { own: vec![chor], ..Selection::default() });
        let blocks = agenda_blocks(&table, &table.agenda(Locale::De), &plain(), &titles(), None, &i18n::DE);
        let items = agenda_items(&blocks);
        let lines: Vec<&ItemLine> = items.iter().map(|(_, item)| *item).filter(|item| item.text == "Chor").collect();
        assert!(lines.len() > 10, "{items:?}");
        for line in lines {
            assert!(line.own && line.class.split(' ').any(|class| class == "own") && line.class.starts_with("t-own"), "{line:?}");
            assert_eq!((line.time.as_str(), line.small.as_str()), ("17:00–19:00", "Aula"));
            assert!(line.title.ends_with("von dir, nicht in Datei und festem Abo") && line.href.contains("termin="), "{line:?}");
        }
    }

    pub(crate) fn plain() -> StudyplanUrl {
        StudyplanUrl::parse("sem=2026W")
    }

    /// The same address while the plan is edited.
    pub(crate) fn editing() -> StudyplanUrl {
        plain().with_edit(true)
    }

    #[test]
    fn a_slot_says_its_time_and_its_weeks() {
        let small = |label: WeekLabel, every: Option<Every>| slot_small(&label, every, 690, 780, &i18n::DE);
        assert_eq!(small(WeekLabel::Every(Every::Week), Some(Every::Week)), "11:30–13:00");
        assert_eq!(small(WeekLabel::Every(Every::AWeek), Some(Every::AWeek)), "11:30–13:00 A");
        assert_eq!(small(WeekLabel::Every(Every::BWeek), Some(Every::BWeek)), "11:30–13:00 B");
        assert_eq!(small(WeekLabel::Every(Every::FourWeeks), Some(Every::FourWeeks)), "11:30–13:00 · 4-wöch.");
        assert_eq!(small(WeekLabel::Every(Every::ThreeWeeks), Some(Every::ThreeWeeks)), "11:30–13:00 · 3-wöch.");
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
        assert_eq!(said(WeekLabel::Every(Every::Week), Some(Every::Week), Some((1, 4))), "1 von 4");
        assert_eq!(said(WeekLabel::Every(Every::Week), Some(Every::Week), Some((2, 5))), "2 von 5");
        assert_eq!(said(WeekLabel::Every(Every::BWeek), Some(Every::BWeek), None), "B-Woche");
        let until = WeekLabel::Partial { first: d("2026-10-06"), last: d("2026-11-24") };
        assert_eq!(said(until, Some(Every::AWeek), None), "A-Woche · bis 24.11.");
        assert_eq!(said(WeekLabel::Partial { first: d("2026-12-07"), last: d("2027-01-25") }, Some(Every::Week), None), "ab 07.12.");
        assert_eq!(said(WeekLabel::Partial { first: d("2026-10-26"), last: d("2026-11-23") }, Some(Every::Week), None), "26.10.–23.11.");
        assert_eq!(said(WeekLabel::Once { dates: 1, first: d("2027-02-23") }, None, None), "1 Termin · 23.02.");
    }

    /// A series of single dates is one slot in its rhythm (owner, 2026-10-13: „sehr
    /// unübersichtlich, wenn da Einzeltermine sind"), not one that counts its dates: weekly its
    /// time alone, every three weeks „3-wöch."; its tooltip names the part of the period and its
    /// dates, which the range only spans.
    #[test]
    fn a_series_of_single_dates_is_one_slot() {
        let single = |event: &str, ord: i64, weekday: i64, time: (&str, &str), on: &str, weeks: u8| {
            let mut row = row(event, ord, "Seminar", "single", weekday, time, (on, on));
            row.date.series = Some(folia_model::rows_detail::Series { first: 1, weeks });
            row
        };
        let schedule = [
            single("150001", 1, 2, ("13:45", "15:15"), "2026-10-13", 1),
            single("150001", 2, 2, ("13:45", "15:15"), "2026-10-20", 1),
            single("150001", 3, 2, ("13:45", "15:15"), "2026-11-03", 1),
            single("150002", 1, 5, ("08:00", "09:30"), "2026-10-16", 3),
            single("150002", 2, 5, ("08:00", "09:30"), "2026-11-06", 3),
            single("150002", 3, 5, ("08:00", "09:30"), "2026-11-27", 3),
        ];
        let slots = week_slots(&table(&schedule), &plain(), &titles(), Weeks::All, None, &i18n::DE);
        let shown: Vec<(&str, &str)> = slots.iter().map(|slot| (slot.slot.small.as_str(), slot.slot.class)).collect();
        assert_eq!(shown, [("13:45–15:15", "tinted"), ("08:00–09:30 · 3-wöch.", "tinted")]);
        let titles: Vec<&str> = slots.iter().map(|slot| slot.slot.title.as_str()).collect();
        assert!(titles[0].contains("bis 03.11.") && titles[0].ends_with("13.10.2026, 20.10.2026, 03.11.2026"), "{}", titles[0]);
        assert!(titles[1].contains("3-wöch. · bis 27.11.") && titles[1].ends_with("16.10.2026, 06.11.2026, 27.11.2026"), "{}", titles[1]);
    }

    #[test]
    fn the_regelwoche_links_each_slot_to_its_termin() {
        let schedule = [
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")),
            row("148134", 1, "Übung", "week_a", 2, ("07:30", "09:00"), ("2026-10-06", "2026-11-17")),
            row("148019", 1, "Übung", "single", 2, ("11:45", "13:15"), ("2027-02-23", "2027-02-23")),
        ];
        let table = table(&schedule);
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, None, &i18n::DE);
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
        assert_eq!(week_slots(&table, &plain(), &short, Weeks::All, None, &i18n::DE).get(1).map(|slot| slot.slot.label.clone()).as_deref(), Some("VL EvS"));
        // The label names the module in its short name, whatever the event is called.
        let long = BTreeMap::from([("12104".to_string(), "Elektrische und elektronische Grundlagen der Informatik".to_string())]);
        let slots = week_slots(&table, &plain(), &long, Weeks::All, None, &i18n::DE);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Elektrische und elektronische …"));
        assert_eq!(slots.get(1).map(|slot| slot.row.text.as_str()), Some("Vorlesung · Entwicklung von Softwaresystemen"));
        // Without the module's title, the event's stands in.
        let slots = week_slots(&table, &plain(), &BTreeMap::new(), Weeks::All, None, &i18n::DE);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Entwicklung von Softwaresystemen"));
        let lecture = slots.get(1).unwrap();
        let key = table.events.iter().find(|event| event.id == "148701").and_then(|event| event.rows.first()).and_then(|row| row.key).unwrap();
        // A click opens the Termin in its own view, on its next date.
        assert_eq!(lecture.slot.href.as_deref(), Some(format!("/studyplan?sem=2026W&termin={}", key.text()).as_str()));
        assert_eq!((lecture.slot.hue, lecture.row.class.as_str()), (Some("t-ice"), "t-ice"));
        assert_eq!((lecture.row.time.as_str(), lecture.row.text.as_str()), ("11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!(lecture.slot.title, "Vorlesung · Entwicklung von Softwaresystemen · Di 11:30–13:00 · HG 0.20");
        // The slot is the one beside the plan when its module is open and the address names it, or
        // when it is in its own view.
        assert!(lecture.row.is(&(Some("12104".into()), Some(key))));
        assert!(lecture.row.is(&(None, Some(key))) && !lecture.row.is(&(Some("11112".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12107".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12104".into()), None)));
        assert!(!slots.first().unwrap().row.is(&(Some("12104".into()), Some(key))));
    }

    /// The week of the lecture 148701 (Di 11:30) and the groups of 148370 (Mo, Mi 11:30), a group of
    /// the other two hidden, with the Termin of one's own „Chor" (Mi 17:00).
    fn mode_table(doc: &PlanDoc) -> Timetable {
        use folia_calendar::own::{OwnRhythm, OwnTermin};
        let group = |ord: i64, weekday: i64, first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, ("11:30", "13:00"), (first, "2027-01-29"));
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        let schedule = [row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")), group(1, 1, "2026-10-05"), group(2, 3, "2026-10-07")];
        let facts = winter();
        let modules = vec!["12104".to_string()];
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
        let chor = OwnTermin { number: 1, rhythm: OwnRhythm::Week, weekday: 3, dates: Vec::new(), from: 17 * 60, to: 19 * 60, period: None, module: None, name: "Chor".into(), place: "Aula".into(), skip: Vec::new(), kind: None };
        let mut selection = doc.selection(key(), TownChoice::Derive);
        selection.own.push(chor);
        Timetable::build(&input, &selection)
    }

    #[test]
    fn looking_no_slot_has_a_button_and_every_slot_opens_its_termin() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let slots = week_slots(&mode_table(&doc), &plain(), &titles(), Weeks::All, None, &i18n::DE);
        assert_eq!(slots.len(), 4);
        for slot in &slots {
            assert!(slot.acts.is_empty() && slot.slot.acts.is_empty() && slot.row.acts.is_empty() && !slot.slot.asks && slot.slot.class != "faint", "{:?}", slot.slot.label);
            assert!(slot.slot.href.as_deref().is_some_and(|href| href.starts_with("/studyplan?sem=2026W&termin=")), "{:?}", slot.slot.href);
        }
        // A hatched group too: its card, not the choice in focus.
        assert!(slots.iter().any(|slot| slot.slot.alt));
    }

    #[test]
    fn editing_a_hatched_group_still_opens_its_termin() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let slots = week_slots(&mode_table(&doc), &editing(), &titles(), Weeks::All, None, &i18n::DE);
        let group = slots.iter().find(|slot| slot.slot.alt).unwrap();
        assert!(group.slot.href.as_deref().is_some_and(|href| href.contains("edit=1&termin=148370-")), "{:?}", group.slot.href);
        assert_eq!(group.acts.iter().map(|act| act.icon).collect::<Vec<_>>(), ["check", "eye"]);
        // No „×" on any slot: the eye hides, the bin deletes.
        assert!(slots.iter().flat_map(|slot| &slot.acts).all(|act| act.icon != "x"));
    }

    #[test]
    fn a_termin_of_ones_own_has_the_bin_and_no_eye() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let slots = week_slots(&mode_table(&doc), &editing(), &titles(), Weeks::All, None, &i18n::DE);
        let chor = slots.iter().find(|slot| slot.slot.own).unwrap();
        assert_eq!(chor.acts.iter().map(|act| (act.icon, act.label.as_str(), act.act.clone())).collect::<Vec<_>>(), [("trash-2", "Löschen", Act::RemoveOwn(1))]);
    }

    #[test]
    fn a_faint_slot_opens_its_card() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        doc.set_event(key(), 148701, true);
        let table = mode_table(&doc);
        // Looking it is not drawn; editing it stands faint, with the crossed eye, and its link opens
        // its card.
        assert!(week_slots(&table, &plain(), &titles(), Weeks::All, None, &i18n::DE).iter().all(|slot| slot.slot.class != "faint"));
        let slots = week_slots(&table, &editing(), &titles(), Weeks::All, None, &i18n::DE);
        let faint = slots.iter().find(|slot| slot.slot.class == "faint").unwrap();
        assert!(faint.slot.href.as_deref().is_some_and(|href| href.contains("edit=1&termin=148701-")), "{:?}", faint.slot.href);
        assert_eq!(faint.acts.iter().map(|act| (act.icon, act.label.as_str())).collect::<Vec<_>>(), [("eye-off", "Einblenden")]);
    }

    /// A group of an open choice hidden by its eye, or with its whole event: faint while editing,
    /// with „✓" that takes it and the crossed eye that shows it again as it was.
    #[test]
    fn a_hidden_group_has_the_crossed_eye_beside_its_tick() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let group = week_slots(&mode_table(&doc), &plain(), &titles(), Weeks::All, None, &i18n::DE).into_iter().find(|slot| slot.slot.alt).unwrap().row.keys[0];
        let acts = |doc: &PlanDoc| -> Vec<Vec<(&'static str, String)>> {
            week_slots(&mode_table(doc), &editing(), &titles(), Weeks::All, None, &i18n::DE)
                .into_iter()
                .filter(|slot| slot.slot.class == "faint")
                .map(|slot| slot.acts.iter().map(|act| (act.icon, act.label.clone())).collect())
                .collect()
        };
        let both = vec![("check", "Diese Gruppe nehmen".to_string()), ("eye-off", "Einblenden".to_string())];
        doc.set_row(key(), group, true);
        assert_eq!(acts(&doc), vec![both.clone()]);
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        doc.set_event(key(), 148370, true);
        assert_eq!(acts(&doc), [both.clone(), both]);
    }

    #[test]
    fn a_series_with_a_hidden_date_is_marked_while_editing() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let lecture = |url: &StudyplanUrl, doc: &PlanDoc| week_slots(&mode_table(doc), url, &titles(), Weeks::All, None, &i18n::DE).into_iter().find(|slot| slot.row.text.starts_with("Vorlesung")).unwrap();
        let key_of = lecture(&plain(), &doc).row.keys[0];
        assert!(!lecture(&editing(), &doc).slot.hides);
        doc.set_date(key(), key_of, Day::parse("2026-11-03").unwrap(), true);
        assert!(!lecture(&plain(), &doc).slot.hides);
        let marked = lecture(&editing(), &doc);
        assert!(marked.slot.hides && marked.row.class.contains("hides") && marked.slot.title.ends_with("1 Termin ausgeblendet"), "{}", marked.slot.title);
    }

    /// The empty week's card: only where the week has nothing to draw (a Termin of one's own or a
    /// calendar in the week is something), and only while looking.
    #[test]
    fn the_empty_weeks_card_stands_only_where_nothing_is_drawn() {
        assert!(nothing_to_draw(0, 0, false));
        assert!(!nothing_to_draw(1, 0, false) && !nothing_to_draw(0, 1, false) && !nothing_to_draw(0, 0, true));
        assert!(empty_card(true, false));
        assert!(!empty_card(true, true) && !empty_card(false, false));
    }

    #[test]
    fn an_old_link_to_a_choice_is_the_mode() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        doc.set_event(key(), 148701, true);
        // `wahl=` of a code no event has: no focus, the editing week with what is faint.
        let slots = week_slots(&mode_table(&doc), &plain().with_wahl(Some("e999")), &titles(), Weeks::All, None, &i18n::DE);
        assert!(slots.iter().any(|slot| slot.slot.class == "faint") && slots.iter().any(|slot| !slot.acts.is_empty()));
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
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, None, en);
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
        // The kind in English as the slot's short word has it („Lec"), never QIS's „Vorlesung" beside it.
        let lecture = slots.get(1).unwrap();
        assert_eq!(lecture.slot.title, "Lecture · Entwicklung von Softwaresystemen · Tue 11:30–13:00 · HG 0.20");
        // While looking no slot has a button; while editing the eye says what it does.
        assert!(lecture.acts.is_empty());
        let edited = week_slots(&table, &editing(), &titles(), Weeks::All, None, en);
        // The eye names the kind in English, not QIS's German word.
        assert_eq!(edited.get(1).unwrap().acts.iter().map(|act| act.label.as_str()).collect::<Vec<_>>(), ["Hide the lecture"]);
        // A type QIS names that is no kind Betula knows („Kolloquium"): QIS's word as written,
        // never „Hide the other".
        let colloquium = self::table(&[row("148701", 1, "Kolloquium", "weekly", 5, ("13:45", "15:15"), ("2026-10-06", "2027-01-26"))]);
        let edited = week_slots(&colloquium, &editing(), &titles(), Weeks::All, None, en);
        assert_eq!(edited.first().unwrap().acts.iter().map(|act| act.label.as_str()).collect::<Vec<_>>(), ["Hide Kolloquium"]);
        let event = table.events.iter().find(|event| event.id == "148701").unwrap();
        assert_eq!((reason_text(HiddenBy::Kinds, event, en), reason_text(HiddenBy::Choice, event, en)), ("“Lecture” hidden".to_string(), "another group chosen".to_string()));
        // The slot keeps the app's path; the grid writes its link under `/en`.
        assert!(lecture.slot.href.as_deref().is_some_and(|href| href.starts_with("/studyplan?")));
        assert!(linked(&lecture.slot, en).is_some_and(|href| href.starts_with("/en/studyplan?sem=2026W&termin=")));
        assert_eq!(linked(&lecture.slot, &i18n::DE), lecture.slot.href);

        // The agenda: the heads of its weeks, a day, the break.
        let blocks = agenda_blocks(&table, &table.agenda(Locale::En), &plain(), &titles(), None, en);
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
        assert_eq!((when.as_str(), first.text.as_str(), first.title.as_str()), ("Tue 6 Oct", "Exercise · Entwicklung von Softwaresystemen", "Exercise · Entwicklung von Softwaresystemen · 6 Oct 2026 07:30–09:00 · HG 0.20"));
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
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All, None, &i18n::DE);
        // The A and the B slot of 12104 stand together, next to each other; the other lecture alone.
        let together: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| slot.slot.joint == Some(0)).map(|(at, _)| at).collect();
        assert_eq!(together.len(), 2, "{:?}", joints(&slots));
        assert_eq!(together[1], together[0] + 1);
        assert!(slots.iter().all(|slot| slot.slot.clash));
        assert!(slots.iter().filter(|slot| slot.slot.joint.is_some()).all(|slot| slot.row.modules.first().map(String::as_str) == Some("12104")));
        assert_eq!(slots.iter().filter(|slot| slot.slot.joint.is_none()).count(), 1);
        // The A weeks alone: nothing to stand with, a frame of its own.
        let a_weeks = week_slots(&table, &plain(), &titles(), Weeks::A, None, &i18n::DE);
        assert!(a_weeks.iter().all(|slot| slot.slot.joint.is_none() && slot.slot.clash) && a_weeks.len() == 2);
        // Of two modules, an A-week and a B-week slot stand apart.
        let mut exercise = row("148136", 1, "Übung", "week_b", 2, ("07:30", "09:00"), ("2026-10-13", "2027-01-19"));
        exercise.module_id = "12107".into();
        let schedule = [schedule[0].clone(), schedule[1].clone(), exercise];
        let apart = week_slots(&planned(&["12104", "12107"], &schedule, &[]), &plain(), &titles(), Weeks::All, None, &i18n::DE);
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
        // The week while the plan is edited, without its faint slots unless `all`.
        let plan_week = |doc: &PlanDoc, all: bool| {
            let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
            let slots = week_slots(&Timetable::build(&input, &doc.selection(key(), TownChoice::Derive)), &editing(), &titles(), Weeks::All, None, &i18n::DE);
            slots.into_iter().filter(|slot| all || slot.slot.class != "faint").collect::<Vec<_>>()
        };
        // Each slot as „Mo 11:30" with its buttons, „✓ …", „+ …", the eye „◉ …", the crossed eye
        // „◌ …" or the bin „▣ …" (no „×").
        let said = |slot: &PlanSlot| -> Vec<String> { slot.acts.iter().map(|act| format!("{} {}", sign(act.icon), act.label)).collect() };
        let buttons = |slots: &[PlanSlot]| -> Vec<(String, Vec<String>)> { slots.iter().map(|slot| (format!("{} {}", day_short(slot.slot.day, &i18n::DE), clock(slot.slot.from)), said(slot))).collect() };
        let at = |slots: &[PlanSlot], day: u8, from: &str| slots.iter().find(|slot| slot.slot.day == day && clock(slot.slot.from) == from).cloned().unwrap();
        let practicals = |slots: &[PlanSlot]| -> Vec<u8> { slots.iter().filter(|slot| slot.row.text.starts_with("Praktikum")).map(|slot| slot.slot.day).collect() };
        let entry = |when: &str, labels: &[&str]| (when.to_string(), labels.iter().map(|label| label.to_string()).collect::<Vec<_>>());
        let group_buttons = ["✓ Diese Gruppe nehmen", "◉ Diese Gruppe ausblenden"];
        let open = plan_week(&doc, false);
        assert_eq!(
            buttons(&open),
            [
                entry("Mo 09:15", &["◉ Ausblenden"]),
                entry("Mo 11:30", &group_buttons),
                entry("Di 11:30", &["◉ Vorlesung ausblenden"]),
                entry("Mi 11:30", &group_buttons),
                entry("Do 11:00", &["◉ Ausblenden"]),
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
        // A hatched group's link opens its Termin, while editing too (one click, one meaning).
        assert!(monday.slot.href.as_deref().is_some_and(|href| href.contains("edit=1&termin=148370-")), "{:?}", monday.slot.href);

        // „✓" on Wednesday: the other groups leave the plan. Its „✓" stays, on and in view, and
        // takes the choice back at once; its eye leaves the group out.
        at(&open, 3, "11:30").acts[0].act.apply(&mut doc, key());
        let chosen = plan_week(&doc, false);
        assert_eq!(practicals(&chosen), [3]);
        let wednesday = at(&chosen, 3, "11:30");
        assert_eq!(said(&wednesday), ["✓ Gruppe abwählen", "◉ Diese Gruppe ausblenden"]);
        let pressed: Vec<bool> = wednesday.slot.acts.iter().map(|button| button.pressed).collect();
        assert!(wednesday.slot.asks && pressed == [true, false] && wednesday.row.acts == wednesday.slot.acts);
        wednesday.acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [1, 3, 4, 5]);
        at(&plan_week(&doc, false), 3, "11:30").acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [3]);
        // While editing the others stand beside the plan, faint, each with its „✓" in view; the one under
        // Thursday's lecture says it would meet it, and is not drawn as a clash.
        let all = plan_week(&doc, true);
        let faint: Vec<(String, bool, bool, Vec<String>)> = all
            .iter()
            .filter(|slot| slot.slot.class == "faint")
            .map(|slot| (format!("{} {}", day_short(slot.slot.day, &i18n::DE), clock(slot.slot.from)), slot.slot.clash, slot.slot.asks, said(slot)))
            .collect();
        let taken = |when: &str, clash: bool| (when.to_string(), clash, true, vec!["✓ Nur diese Gruppe".to_string(), "+ Auch diese Gruppe".to_string()]);
        assert_eq!(faint, [taken("Mo 11:30", false), taken("Do 11:30", false), taken("Fr 13:45", false)]);
        let thursday = at(&all, 4, "11:30");
        assert!(thursday.slot.title.starts_with("andere Gruppe gewählt · ") && thursday.slot.title.ends_with("überschneidet sich mit VL Entwicklung von Softwaresystemen"), "{}", thursday.slot.title);
        assert!(thursday.key.starts_with('g') && thursday.row.class.contains("faint"));
        assert_eq!(all.len(), chosen.len() + 3);
        // „+" on a faint one ticks it beside (owner, 2026-10-26): both groups in the plan, each
        // with its „✓" on, which takes back that one alone.
        at(&all, 1, "11:30").acts[1].act.apply(&mut doc, key());
        let two = plan_week(&doc, false);
        assert_eq!(practicals(&two), [1, 3]);
        assert!([1, 3].iter().all(|day| said(&at(&two, *day, "11:30")) == ["✓ Gruppe abwählen", "◉ Diese Gruppe ausblenden"] && !at(&two, *day, "11:30").slot.alt));
        at(&two, 1, "11:30").acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [3]);
        // „✓" on a faint one switches the choice.
        at(&all, 5, "13:45").acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [5]);
        // The eye on the one taken: the choice is open again, without it; faint, its „✓" takes it
        // again and its crossed eye shows it again, the choice still open.
        at(&plan_week(&doc, false), 5, "13:45").acts[1].act.apply(&mut doc, key());
        let reopened = plan_week(&doc, false);
        assert_eq!(practicals(&reopened), [1, 3, 4]);
        assert!(reopened.iter().filter(|slot| slot.row.text.starts_with("Praktikum")).all(|slot| slot.slot.asks));
        let friday = at(&plan_week(&doc, true), 5, "13:45");
        assert_eq!(said(&friday), ["✓ Diese Gruppe nehmen", "◌ Einblenden"]);
        friday.acts[0].act.apply(&mut doc, key());
        assert_eq!(practicals(&plan_week(&doc, false)), [5]);

        // The eye on a Termin of the lecture of two: it leaves the plan; faint, its crossed eye
        // brings it back.
        at(&plan_week(&doc, false), 1, "09:15").acts[0].act.apply(&mut doc, key());
        assert!(plan_week(&doc, false).iter().all(|slot| clock(slot.slot.from) != "09:15"));
        let hidden = at(&plan_week(&doc, true), 1, "09:15");
        assert!(hidden.slot.title.starts_with("ausgeblendet · "));
        assert_eq!(said(&hidden), ["◌ Einblenden"]);
        hidden.acts[0].act.apply(&mut doc, key());
        assert!(plan_week(&doc, false).iter().any(|slot| clock(slot.slot.from) == "09:15"));

        // A kind switched off comes back with every event of it.
        doc.set_kind(key(), EventKind::Lecture, true);
        assert!(plan_week(&doc, false).iter().all(|slot| !slot.row.text.starts_with("Vorlesung")));
        let lecture = at(&plan_week(&doc, true), 2, "11:30");
        assert!(lecture.slot.title.starts_with("„Vorlesung“ ausgeblendet · "), "{}", lecture.slot.title);
        assert_eq!(said(&lecture), ["◌ „Vorlesung“ wieder einblenden"]);
        lecture.acts[0].act.apply(&mut doc, key());
        assert_eq!(plan_week(&doc, false).iter().filter(|slot| slot.row.text.starts_with("Vorlesung")).count(), 3);
    }

    /// A slot button's sign as the slot shows it: „✓", „+", the eye, the crossed eye, the bin.
    fn sign(icon: &str) -> &'static str {
        match icon {
            "check" => "✓",
            "plus" => "+",
            "eye" => "◉",
            "eye-off" => "◌",
            "trash-2" => "▣",
            _ => "×",
        }
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
        let blocks = agenda_blocks(&table, &weeks, &plain(), &titles(), Some(d("2026-12-15")), &i18n::DE);
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
        let blocks = agenda_blocks(&table, &busy, &plain(), &titles(), None, &i18n::DE);
        assert!(matches!(blocks.first(), Some(Block::Week { head, .. }) if head == "KW 52 · 21.–27.12.2026 · vorlesungsfrei"), "{blocks:?}");
        assert!(matches!(blocks.get(1), Some(Block::Break { text, id, .. }) if text == "28.12.–03.01. vorlesungsfrei" && id == "kw-2026-53"), "{blocks:?}");
    }

    #[test]
    fn the_agenda_opens_at_the_current_week() {
        let table = table(&[]);
        let weeks: Vec<AgendaWeek> = ["2026-10-05", "2026-10-12", "2026-10-26"].into_iter().map(|monday| week(monday, false, Vec::new())).collect();
        let blocks = agenda_blocks(&table, &weeks, &plain(), &titles(), None, &i18n::DE);
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
        let blocks = agenda_blocks(&table, &table.agenda(folia_locale::Locale::De), &plain(), &titles(), Some(d("2026-10-06")), &i18n::DE);
        let items = agenda_items(&blocks);
        let (when, first) = items.first().unwrap();
        assert_eq!((when.as_str(), first.time.as_str(), first.text.as_str()), ("Di 06.10.", "11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!((first.class.as_str(), first.small.as_str()), ("t-ice", "HG 0.20"));
        assert!(first.href.starts_with("/studyplan?sem=2026W&termin=148701-") && first.href.ends_with("&on=2026-10-06"), "{}", first.href);
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
        let blocks = agenda_blocks(&table, &table.agenda(folia_locale::Locale::De), &plain(), &titles(), None, &i18n::DE);
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
        let blocks = agenda_blocks(&table, &table.agenda(Locale::En), &plain(), &titles(), None, &i18n::EN);
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
        let labels: Vec<(String, String)> = table.events.iter().map(|event| (slot_label(event, &math, Locale::De), event_text(event, &math, Locale::De))).collect();
        // A slot names the module in its short name after the kind's few letters.
        assert!(labels.contains(&("Tut Höhere Mathematik W-1".to_string(), "Tutorium Höhere Mathematik W-1".to_string())), "{labels:?}");
        assert!(labels.contains(&("VL Höhere Mathematik W-1".to_string(), "Vorlesung · Entwicklung von Softwaresystemen".to_string())), "{labels:?}");
    }

    /// A choice among the module's events, decided in the week: „✓" takes an option, beside what
    /// a choice of two took so far, else instead; „✓" on the one taken takes it back, of a choice
    /// of one the whole choice.
    #[test]
    fn an_option_among_the_modules_events_is_taken_in_the_week() {
        let option = |event: &str, weekday: i64, from: &str, to: &str, first: &str| row(event, 1, "Übung", "weekly", weekday, (from, to), (first, "2027-01-29"));
        let schedule = [
            option("201", 2, "07:30", "09:00", "2026-10-06"),
            option("202", 3, "13:45", "15:15", "2026-10-07"),
            option("203", 4, "11:30", "13:00", "2026-10-08"),
            option("204", 5, "09:15", "10:45", "2026-10-09"),
        ];
        let choice = |pick: u32, options: usize| -> Vec<ChoiceOption> {
            (1..=options as u32)
                .map(|n| ChoiceOption {
                    module_id: "12104".into(),
                    semester_key: "2026W".into(),
                    choice: 1,
                    course: None,
                    kind: Code::parse("topic"),
                    basis: Code::parse("form"),
                    sws: Some(2.0),
                    pick,
                    option: n,
                    event_id: format!("20{n}"),
                    label: None,
                    event_sws: Some(2.0),
                })
                .collect()
        };
        let facts = winter();
        let modules = vec!["12104".to_string()];
        // The week while the plan is edited, without its faint slots unless `all`.
        let plan_week = |doc: &PlanDoc, choices: &[ChoiceOption], all: bool| {
            let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices, listed: &[] };
            let slots = week_slots(&Timetable::build(&input, &doc.selection(key(), TownChoice::Derive)), &editing(), &titles(), Weeks::All, None, &i18n::DE);
            slots.into_iter().filter(|slot| all || slot.slot.class != "faint").collect::<Vec<_>>()
        };
        let said = |slot: &PlanSlot| -> Vec<String> { slot.acts.iter().map(|act| format!("{} {}", sign(act.icon), act.label)).collect() };
        let at = |slots: &[PlanSlot], day: u8| slots.iter().find(|slot| slot.slot.day == day).cloned().unwrap();
        let days = |slots: &[PlanSlot]| -> Vec<u8> { slots.iter().filter(|slot| slot.slot.class != "faint").map(|slot| slot.slot.day).collect() };
        let open_buttons = ["✓ Diese nehmen", "◉ Übung ausblenden"];
        let taken_buttons = ["✓ Wahl aufheben", "◉ Übung ausblenden"];

        // One of three: each hatched, „1 von 3" in its tooltip, „✓" and the eye.
        let three = choice(1, 3);
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let open = plan_week(&doc, &three, false);
        assert_eq!(days(&open), [2, 3, 4, 5]);
        for day in [2, 3, 4] {
            let slot = at(&open, day);
            assert!(slot.slot.alt && slot.slot.title.contains("1 von 3") && said(&slot) == open_buttons, "{day}: {:?} {}", said(&slot), slot.slot.title);
        }
        // „✓" on Wednesday: the others leave the week; its „✓", on, takes the choice back.
        at(&open, 3).acts[0].act.apply(&mut doc, key());
        let chosen = plan_week(&doc, &three, false);
        assert_eq!(days(&chosen), [3, 5]);
        let wednesday = at(&chosen, 3);
        assert!(!wednesday.slot.alt && said(&wednesday) == taken_buttons && wednesday.slot.acts[0].pressed);
        wednesday.acts[0].act.apply(&mut doc, key());
        assert_eq!(days(&plan_week(&doc, &three, false)), [2, 3, 4, 5]);
        // Tuesday taken, then Thursday's faint „✓" beside the plan takes it instead.
        at(&plan_week(&doc, &three, false), 2).acts[0].act.apply(&mut doc, key());
        let all = plan_week(&doc, &three, true);
        let thursday = at(&all, 4);
        assert!(thursday.slot.class == "faint" && said(&thursday) == ["✓ Nur diese", "+ Auch diese"] && thursday.slot.title.starts_with("andere Option gewählt"), "{}", thursday.slot.title);
        thursday.acts[0].act.apply(&mut doc, key());
        assert_eq!(days(&plan_week(&doc, &three, false)), [4, 5]);
        // Tuesday's faint „+" takes it beside Thursday, one more than the choice takes; the „✓" of
        // either takes back that one alone.
        at(&plan_week(&doc, &three, true), 2).acts[1].act.apply(&mut doc, key());
        let two = plan_week(&doc, &three, false);
        assert_eq!(days(&two), [2, 4, 5]);
        assert!(said(&at(&two, 2)) == taken_buttons && said(&at(&two, 4)) == taken_buttons);
        at(&two, 2).acts[0].act.apply(&mut doc, key());
        assert_eq!(days(&plan_week(&doc, &three, false)), [4, 5]);
        // The eye on the one taken: open again without it.
        at(&plan_week(&doc, &three, false), 4).acts[1].act.apply(&mut doc, key());
        let reopened = plan_week(&doc, &three, false);
        assert_eq!(days(&reopened), [2, 3, 5]);
        assert!(at(&reopened, 2).slot.title.contains("1 von 2"), "{}", at(&reopened, 2).slot.title);

        // Two of four: „2 von 4"; a first „✓" leaves the choice open for one more, a second makes
        // it; „✓" on one of them takes back that one alone.
        let four = choice(2, 4);
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let open = plan_week(&doc, &four, false);
        assert!(open.iter().all(|slot| slot.slot.alt && slot.slot.title.contains("2 von 4")), "{:?}", open.iter().map(|slot| &slot.slot.title).collect::<Vec<_>>());
        at(&open, 2).acts[0].act.apply(&mut doc, key());
        let one = plan_week(&doc, &four, false);
        assert_eq!(days(&one), [2, 3, 4, 5]);
        assert_eq!(said(&at(&one, 2)), taken_buttons);
        assert!([3, 4, 5].iter().all(|day| at(&one, *day).slot.title.contains("1 von 3") && said(&at(&one, *day)) == open_buttons));
        at(&one, 4).acts[0].act.apply(&mut doc, key());
        let both = plan_week(&doc, &four, false);
        assert_eq!(days(&both), [2, 4]);
        at(&both, 2).acts[0].act.apply(&mut doc, key());
        let back = plan_week(&doc, &four, false);
        assert_eq!(days(&back), [2, 3, 4, 5]);
        assert_eq!(said(&at(&back, 4)), taken_buttons);
        assert!([2, 3, 5].iter().all(|day| at(&back, *day).slot.title.contains("1 von 3")));
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
        let build = |selection: &Selection| Timetable::build(&Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] }, selection);

        let open = build(&Selection::default());
        let blocks = agenda_blocks(&open, &open.agenda(folia_locale::Locale::De), &plain(), &titles(), None, &i18n::DE);
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
        assert!(first.href.starts_with("/studyplan?sem=2026W&termin=148370-") && first.href.ends_with("&on=2026-10-05"), "{}", first.href);
        assert!(first.class.ends_with(" alt") && first.title.contains("Mo 11:30–13:00 · Mi 11:30–13:00"), "{first:?}");

        // Today keeps its line (review 2026-09-25): its options stand as they are, and on the day
        // of the week's line nothing comes twice.
        let on = |today: &str| {
            let blocks = agenda_blocks(&open, &open.agenda(folia_locale::Locale::De), &plain(), &titles(), Some(d(today)), &i18n::DE);
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
        let blocks = agenda_blocks(&decided, &decided.agenda(folia_locale::Locale::De), &plain(), &titles(), None, &i18n::DE);
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
        let lines = loose_lines(&table, &plain(), &titles(), &table.loose(), &i18n::DE);
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
        let english: Vec<String> = loose_lines(&table, &plain(), &titles(), &table.loose(), &i18n::EN).into_iter().map(|line| line.small).collect();
        assert_eq!(english, ["Mon · nach Vereinbarung · 12 Oct–25 Jan · HG 0.20", "no dates"]);
        // The agenda places neither, so its „Ohne Datum" has both.
        assert_eq!(undated(&table), table.loose());
        // A row that says nothing says so (R12).
        let mut nothing = row("150002", 1, "Seminar", "other", 1, ("", ""), ("", ""));
        nothing.date = EventDate { weekday: None, start_time: None, end_time: None, rhythm: None, first_date: None, last_date: None, room: None, ..nothing.date };
        assert_eq!(row_facts(&nothing.date, &i18n::DE), "nicht angegeben");
        assert_eq!(row_facts(&nothing.date, &i18n::EN), "not stated");
    }

    #[test]
    fn a_visit_is_ringed_and_gives_way_in_the_week() {
        // The plan's lecture on Tuesday 13:45; Denkmalpflege only visited, its lecture on Tuesday
        // 14:00 and its Übung on Thursday.
        let lecture = row("148702", 1, "Vorlesung", "weekly", 2, ("13:45", "15:15"), ("2026-10-06", "2027-01-26"));
        let mut visit = row("150500", 1, "Vorlesung", "weekly", 2, ("14:00", "15:30"), ("2026-10-06", "2027-01-26"));
        visit.module_id = "13676".into();
        visit.date.event_title = "Denkmalpflege".into();
        let mut exercise = row("150501", 1, "Übung", "weekly", 4, ("13:45", "15:15"), ("2026-10-08", "2027-01-28"));
        exercise.module_id = "13676".into();
        exercise.date.event_title = "Denkmalpflege".into();
        let facts = winter();
        let modules: Vec<String> = vec!["12104".into(), "13676".into()];
        let schedule = [lecture, visit, exercise];
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
        let selection = Selection { visits: ["13676".to_string()].into(), ..Selection::default() };
        let table = Timetable::build(&input, &selection);
        let titles = BTreeMap::from([("12104".to_string(), "EvS".to_string()), ("13676".to_string(), "Denkmalpflege".to_string())]);
        let slots = week_slots(&table, &plain(), &titles, Weeks::All, None, &i18n::DE);
        let of = |event: &str| slots.iter().find(|slot| slot.key.starts_with(&format!("p{}-", table.events.iter().position(|e| e.id == event).unwrap()))).unwrap();
        let (plan, visit, exercise) = (of("148702"), of("150500"), of("150501"));
        // The visit: ringed, giving way, never red; its tooltip says so.
        assert!(visit.slot.visit && visit.slot.gives && !visit.slot.clash, "{:?}", visit.slot);
        assert!(visit.slot.title.contains("Besuch, zählt nicht") && visit.slot.title.ends_with("trifft VL EvS, der Besuch gibt nach"), "{}", visit.slot.title);
        assert!(visit.slot.classes().contains("visit gives") && visit.row.class.contains("visit") && visit.row.class.contains("gives"));
        assert!(exercise.slot.visit && !exercise.slot.gives);
        // The plan's lecture as it was.
        assert!(!plan.slot.visit && !plan.slot.gives && !plan.slot.clash);
        // „Offen" says where it gives way, to the visit's Termin.
        let lines = super::super::open::lines(&table, &titles, &plain(), None, &i18n::DE);
        let gives: Vec<(&str, &str, bool)> = lines.iter().filter(|line| line.gives).map(|line| (line.name.as_str(), line.say.as_str(), line.href.contains("termin=150500-"))).collect();
        assert_eq!(gives, [("Denkmalpflege trifft VL EvS", "Di 14:00 · der Besuch gibt nach", true)]);
    }
}
