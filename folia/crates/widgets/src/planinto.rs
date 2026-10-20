//! „Einplanen in …" (folia/design/tasten/konzept.html, „Knopf mit Pfeil"; owner, 2026-10-20: „Ja
//! bitte setze deine Vorschläge um"): the semesters a module not planned can go to, from the arrow
//! beside „Einplanen" or ⇧P at the module (`folia_stores::studyplan::PlanAsk`). Planned where the
//! switch is pressed for, the same arrow opens the menu of what is planned (`planmenu`).
//!
//! - The semesters from the current one, as far as the plan reaches and at least four, each with its
//!   Fachsemester and whether the module is offered then („angeboten", „nicht angeboten"), „jetzt"
//!   at the current one and a check where the module is planned already (a retake behind the
//!   student, a semester the page does not mean); folded under „Frühere Semester", the semesters
//!   behind the student back to the first Fachsemester, the latest first (owner, 2026-10-18:
//!   „Betula soll nie sagen. Nein, das geht nicht"). As the Merkliste's „Einplanen" of the rows
//!   selected has them, for one module.
//! - A semester chosen plans the module there, and the note of „Einplanen" says so, with ‹ › and
//!   „Rückgängig" (`Planning::place`); where no study is set up, the dialog asks first.
//!
//! On a wide screen at the switch, under it, never under the rail; on a phone a sheet from below
//! (app.css `.st-pop`). Its first entry with the focus is where the switch aims; ↑ ↓ go on, Esc
//! and a click beside it close it, and the focus goes back to the arrow.

use folia_calendar::semester::SemesterKey;
use folia_design::nav;
use folia_design::ui::Icon;
use folia_plans::study::fs_of;
use folia_stores::collections::Anchor;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{PlanAsk, Planning, Studyplan};
use leptos::prelude::*;

use crate::collect::FocusBack;
use crate::i18n;

/// The sizes of the menu on a wide screen (app.css `.st-pop`), to place it before it is drawn.
const WIDTH: f64 = 320.0;
const ITEM: f64 = 50.0;
const FOLD: f64 = 36.0;
const HEAD: f64 = 27.0;
const EDGE: f64 = 8.0;

/// The menu asked for, over the page. Once in the app (`folia_app::App`), inside the router.
#[component]
pub fn PlanIntoHost() -> impl IntoView {
    let Some(planning) = Planning::expect() else { return ().into_any() };
    let asked = Memo::new(move |_| planning.into_menu());
    (move || {
        let ask = asked.get()?;
        Some(view! { <PlanInto ask planning/> })
    })
    .into_any()
}

#[component]
fn PlanInto(ask: PlanAsk, planning: Planning) -> impl IntoView {
    let t = i18n::t();
    let p = &t.planner;
    let Some(plan) = Studyplan::expect() else { return ().into_any() };
    let back = FocusBack::take();
    let close = move || {
        planning.close_into();
        back.give();
    };
    folia_shell::nav::layer_while(move || planning.into_menu().is_some(), move || planning.close_into(), folia_shell::nav::Kind::Over);
    let PlanAsk { what, aim, at } = ask;
    let current = what.current;
    let planned = aim.elsewhere().to_vec();
    let mine = MyProgram::expect();
    let (start, leave) = mine.map(|mine| (mine.start_untracked(), mine.leave())).unwrap_or_default();
    // From the current semester as far as the plan reaches, at least four.
    let planned_last = plan.with_untracked(|doc| doc.semesters().into_iter().max());
    let last = [current.plus(3), planned_last, planned.iter().copied().max()].into_iter().flatten().max().unwrap_or(current);
    let mut ahead = Vec::new();
    let mut s = Some(current);
    while let Some(at) = s.filter(|at| *at <= last) {
        ahead.push(at);
        s = at.plus(1);
    }
    // Behind the student, back to the first Fachsemester (six without a Studienbeginn).
    let floor = planning.floor(current);
    let mut behind = Vec::new();
    let mut s = current.plus(-1);
    while let Some(at) = s.filter(|at| *at >= floor) {
        behind.push(at);
        s = at.plus(-1);
    }
    let earlier = RwSignal::new(false);
    let item = {
        let what = what.clone();
        move |s: SemesterKey| {
            let fs = start.and_then(|start| fs_of(start, s, &leave)).map(|fs| (p.fs_short)(fs));
            let offered = what.offer.offered(s) || what.newest == Some(s);
            let here = planned.contains(&s);
            let what = what.clone();
            let choose = move |_| {
                planning.place(plan, what.clone(), s);
                back.give();
            };
            view! {
                <button class="st-pop-item with-hint" class:on=here type="button" role="menuitemradio" aria-checked=if here { "true" } else { "false" } data-semester=s.key() on:click=choose>
                    <span class="plan-into-check" aria-hidden="true">{here.then(|| view! { <Icon name="check"/> })}</span>
                    <span class="st-pop-label">
                        <span>{s.short(t.locale)}{(s == current).then(|| view! { " " <span class="st-now">{p.now}</span> })}</span>
                        <small>{match (fs, here) {
                            (Some(fs), true) => format!("{fs} · {}", p.planned_there),
                            (Some(fs), false) => fs,
                            (None, true) => p.planned_there.to_string(),
                            (None, false) => String::new(),
                        }}</small>
                    </span>
                    <span class=if offered { "st-pop-note ok" } else { "st-pop-note warn" }>
                        {(!offered).then(|| view! { <Icon name="triangle-alert"/> })}
                        {if offered { p.offered } else { p.not_offered }}
                    </span>
                </button>
            }
        }
    };
    let has_behind = !behind.is_empty();
    let (shown_items, behind_items) = (ahead.len(), behind.len());
    let style = move || {
        let count = shown_items + if earlier.get() { behind_items } else { 0 };
        let height = HEAD + 12.0 + count as f64 * ITEM + if has_behind { if earlier.get() { HEAD } else { FOLD } } else { 0.0 };
        let (left, top) = place(at, height, nav::room());
        format!("--x:{left:.0}px;--y:{top:.0}px;--w:{WIDTH:.0}px")
    };
    let keys = move |ev: leptos::ev::KeyboardEvent| {
        let by = match ev.key().as_str() {
            "ArrowDown" => 1,
            "ArrowUp" => -1,
            "Escape" | "Tab" => {
                ev.prevent_default();
                ev.stop_propagation();
                close();
                return;
            }
            _ => return,
        };
        ev.prevent_default();
        nav::step_focus(".plan-into :is([role=menuitem], [role=menuitemradio])", by);
    };
    // The focus on where the switch aims as it opens, else on the first.
    let aimed = aim.target().key();
    request_animation_frame(move || {
        if !nav::focus_selector(&format!(".plan-into [data-semester=\"{aimed}\"]")) {
            nav::focus_selector(".plan-into [role=menuitemradio]");
        }
    });
    let ahead_view = ahead.into_iter().map(item.clone()).collect_view();
    let behind_view = move || {
        if !has_behind {
            return None;
        }
        Some(if earlier.get() {
            let item = item.clone();
            view! {
                <p class="pick-menu-label">{p.earlier_semesters}</p>
                {behind.iter().copied().map(item).collect_view()}
            }
            .into_any()
        } else {
            view! {
                <button class="st-pop-item" type="button" role="menuitem" on:click=move |_| {
                    earlier.set(true);
                    request_animation_frame(|| {
                        nav::focus_selector(".plan-into .pick-menu-label + [role=menuitemradio]");
                    });
                }>
                    <Icon name="chevron-down"/><span class="st-pop-label"><span>{p.earlier_semesters}</span></span>
                </button>
            }
            .into_any()
        })
    };
    let label = format!("{}: {}", p.plan_into, what.title);
    view! {
        <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close() on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); close(); }></div>
        <div class="st-pop root view-menu plan-into" role="menu" aria-label=label style=style on:keydown=keys>
            <p class="st-pop-title st-phone">{what.title.clone()}</p>
            <p class="view-menu-head">{p.plan_into}</p>
            {ahead_view}
            {behind_view}
        </div>
    }
    .into_any()
}

/// Where the menu `height` tall stands for the switch at `at` in the `room` of the window: under it,
/// its right edge at the switch's, above it where there is no room below; never past the window's
/// edges, nor under the rail. Without a box in the middle, near the top.
fn place(at: Option<Anchor>, height: f64, room: nav::Room) -> (f64, f64) {
    let least = room.left + EDGE;
    let Some(at) = at else {
        return (((room.width - WIDTH) / 2.0).max(least), (room.height * 0.12).round());
    };
    let left = (at.right - WIDTH).clamp(least, (room.width - WIDTH - EDGE).max(least));
    let below = at.bottom + 4.0;
    let top = if below + height <= room.height - EDGE { below } else { (at.top - 4.0 - height).max(EDGE) };
    (left, top)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under the switch, the right edges flush; above it at the window's bottom; never under the
    /// rail.
    #[test]
    fn the_menu_stands_under_the_switch() {
        let switch = Anchor { left: 1100.0, top: 200.0, right: 1410.0, bottom: 232.0 };
        let room = nav::Room { left: 52.0, ..nav::Room::window(1440.0, 900.0) };
        assert_eq!(place(Some(switch), 300.0, room), (1090.0, 236.0));
        let low = Anchor { top: 800.0, bottom: 832.0, ..switch };
        assert_eq!(place(Some(low), 300.0, room), (1090.0, 496.0));
        let narrow = Anchor { left: 60.0, right: 200.0, ..switch };
        assert_eq!(place(Some(narrow), 300.0, room).0, 60.0);
        assert_eq!(place(None, 300.0, room).0, 560.0);
    }
}
