//! The week grid of a module and of the Studienplan.
//!
//! One component draws both: a module's page shows its own Termine in it, the Studienplan its
//! Regelwoche. Callers hand over slots in minutes; the grid finds the days and hours it needs and
//! puts slots that overlap on a day side by side (`catalog::timetable::grid`), so that a single
//! date no longer covers the weekly group beneath it. The geometry is data only, written into
//! custom properties in half-hours (`--days`, `--first`, `--span` on `.week`; `--from`, `--to`,
//! `--lane`, `--lanes` per slot), and `app.css` turns it into places.

use catalog::timetable::grid::{self, Placed, Span};
use leptos::prelude::*;

/// The heads of the days, Monday first; a grid shows as many as it has days.
const DAY_HEADS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

/// The fewest hours a grid spans, so that one short slot still reads as a time of day.
pub const MIN_HOURS: u16 = 4;

/// One slot of the grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridSlot {
    /// 1 = Monday … 7 = Sunday. A slot on any other day is not drawn.
    pub day: u8,
    /// Minutes since midnight, `to > from`; `to` may be 24:00 (1440).
    pub from: u16,
    pub to: u16,
    /// The first line („Vorlesung"), the smaller second one („09:15", „3 Termine"; left out when
    /// empty), and the tooltip (left out when empty).
    pub label: String,
    pub small: String,
    pub title: String,
    /// The look: "" (a lecture), "other" (any other teaching), "tinted" (a module of the
    /// Studienplan, in its `hue`), "planned" (another planned module beside a module's own
    /// slots), "once" (single dates).
    pub class: &'static str,
    /// The tint of a `tinted` slot: one of the `t-…` classes of `app.css` („t-ice" … „t-slate").
    pub hue: Option<&'static str>,
    /// An option of a choice not yet made (dashed), and a slot in a hard clash.
    pub alt: bool,
    pub clash: bool,
    /// Where a click leads: `Some` draws a link that keeps the page's scroll, `None` a plain box.
    pub href: Option<String>,
    /// The slot of what is shown beside the grid (`aria-current`).
    pub current: bool,
}

impl GridSlot {
    /// Where the slot sits, as the grid's arithmetic takes it.
    pub fn placed(&self) -> Placed {
        Placed { day: self.day, from: self.from, to: self.to }
    }

    /// The slot's classes: `slot`, then its look, its tint, `alt` and `clash`.
    pub fn classes(&self) -> String {
        let mut classes = String::from("slot");
        let parts = [self.class, self.hue.unwrap_or_default(), if self.alt { "alt" } else { "" }, if self.clash { "clash" } else { "" }];
        for part in parts.into_iter().filter(|part| !part.is_empty()) {
            classes.push(' ');
            classes.push_str(part);
        }
        classes
    }

    fn drawn(&self) -> bool {
        (1..=7).contains(&self.day)
    }
}

/// What the grid of a set of slots needs: its frame, the style of `.week` and, per slot in the
/// order of the input, the slot's style.
struct Layout {
    span: Span,
    week: String,
    slots: Vec<String>,
}

fn layout(slots: &[GridSlot], min_hours: u16) -> Option<Layout> {
    let drawn: Vec<Placed> = slots.iter().filter(|slot| slot.drawn()).map(GridSlot::placed).collect();
    if drawn.is_empty() {
        return None;
    }
    let span = grid::span(&drawn, min_hours.saturating_mul(60));
    // Lanes are per day, so a slot that is not drawn shares a group with none that is.
    let lanes = grid::lanes(&slots.iter().map(GridSlot::placed).collect::<Vec<_>>());
    let week = format!("--days:{};--first:{};--span:{}", span.days, halves(span.first), halves(span.last.saturating_sub(span.first)));
    let styles = slots
        .iter()
        .zip(lanes)
        .map(|(slot, (lane, lanes))| {
            // The frame ends at 24:00; what would reach past it stops there.
            let to = slot.to.max(slot.from).min(span.last);
            let from = slot.from.min(to);
            format!("--from:{};--to:{};--lane:{lane};--lanes:{lanes}", halves(from), halves(to))
        })
        .collect();
    Some(Layout { span, week, slots: styles })
}

/// The geometry the grid writes, as plain data: the style of `.week` and one style per slot, in
/// the order of `slots`. `None` when no slot is drawn, and then there is no grid at all (a frame
/// of empty hours says nothing).
pub fn geometry(slots: &[GridSlot], min_hours: u16) -> Option<(String, Vec<String>)> {
    layout(slots, min_hours).map(|layout| (layout.week, layout.slots))
}

/// Minutes in the grid's CSS unit, half-hours: 480 → „16", 555 → „18.5", 620 → „20.667".
fn halves(minutes: u16) -> String {
    if minutes.is_multiple_of(30) {
        return (minutes / 30).to_string();
    }
    // A fraction of k/30 (k = 1…29) never rounds to .000 or to the next whole number at three
    // places, so trimming the zeros never reaches the point.
    format!("{:.3}", f64::from(minutes) / 30.0).trim_end_matches('0').to_string()
}

/// A week grid: a column per day (Monday to Friday, with Saturday and Sunday when a slot needs
/// them), the hours the slots need (at least `min_hours`, `MIN_HOURS` by default), and the slots,
/// side by side where they overlap. `heads` names the days instead of „Mo" … „So". Without any
/// slot to draw it renders nothing.
#[component]
pub fn WeekGrid(#[prop(into)] slots: Signal<Vec<GridSlot>>, #[prop(optional)] heads: Option<Vec<String>>, #[prop(optional)] min_hours: Option<u16>) -> impl IntoView {
    let min_hours = min_hours.unwrap_or(MIN_HOURS);
    move || {
        slots.with(|slots| {
            let Layout { span, week, slots: styles } = layout(slots, min_hours)?;
            let heads: Vec<String> = (0..usize::from(span.days))
                .map(|i| heads.as_ref().and_then(|heads| heads.get(i).cloned()).or_else(|| DAY_HEADS.get(i).map(|head| head.to_string())).unwrap_or_default())
                .collect();
            let columns = (1..=span.days)
                .map(|day| {
                    let slots = slots.iter().zip(&styles).filter(|(slot, _)| slot.day == day).map(|(slot, style)| slot_view(slot, style.clone())).collect_view();
                    view! { <div class="col">{slots}</div> }
                })
                .collect_view();
            Some(
                view! {
                    <div class="week" style=week>
                        <span></span>
                        {heads.into_iter().map(|head| view! { <span class="d">{head}</span> }).collect_view()}
                        <div class="hours">{(span.first / 60..span.last / 60).map(|hour| view! { <span>{hour}</span> }).collect_view()}</div>
                        {columns}
                    </div>
                }
                .into_any(),
            )
        })
    }
}

fn slot_view(slot: &GridSlot, style: String) -> AnyView {
    let class = slot.classes();
    let title = (!slot.title.is_empty()).then(|| slot.title.clone());
    let current = slot.current.then_some("true");
    let label = slot.label.clone();
    let small = (!slot.small.is_empty()).then(|| view! { <small>{slot.small.clone()}</small> });
    match slot.href.clone() {
        Some(href) => view! { <a class=class href=href data-noscroll="" style=style title=title aria-current=current>{label}{small}</a> }.into_any(),
        None => view! { <div class=class style=style title=title aria-current=current>{label}{small}</div> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(day: u8, from: u16, to: u16) -> GridSlot {
        GridSlot { day, from, to, label: "Vorlesung".into(), ..GridSlot::default() }
    }

    #[test]
    fn overlapping_slots_get_lanes() {
        // Tuesday 11:45–15:00 and 13:45–15:15 (module 12229): side by side, the longer one left.
        let (week, slots) = geometry(&[slot(2, 705, 900), slot(2, 825, 915)], MIN_HOURS).unwrap();
        assert_eq!(week, "--days:5;--first:16;--span:16");
        assert_eq!(slots, ["--from:23.5;--to:30;--lane:0;--lanes:2", "--from:27.5;--to:30.5;--lane:1;--lanes:2"]);
        // Apart, each has the whole column.
        let (_, slots) = geometry(&[slot(1, 555, 645), slot(1, 645, 735)], MIN_HOURS).unwrap();
        assert!(slots.iter().all(|style| style.ends_with("--lane:0;--lanes:1")), "{slots:?}");
    }

    #[test]
    fn no_slot_no_grid() {
        assert_eq!(geometry(&[], MIN_HOURS), None);
        // A slot on no weekday is not drawn, and alone it makes no grid either.
        assert_eq!(geometry(&[slot(0, 555, 645), slot(8, 555, 645)], MIN_HOURS), None);
    }

    #[test]
    fn the_frame_follows_the_slots() {
        // At least four hours from 08:00 at the latest, whole hours.
        assert_eq!(geometry(&[slot(1, 555, 645)], MIN_HOURS).unwrap().0, "--days:5;--first:16;--span:8");
        assert_eq!(geometry(&[slot(1, 555, 645)], 1).unwrap().0, "--days:5;--first:16;--span:6");
        // A Sunday slot draws the whole week; one that ends at 24:00 ends the frame there.
        let (week, slots) = geometry(&[slot(7, 1380, 1440), slot(3, 450, 540)], MIN_HOURS).unwrap();
        assert_eq!(week, "--days:7;--first:14;--span:34");
        assert_eq!(slots.first().map(String::as_str), Some("--from:46;--to:48;--lane:0;--lanes:1"));
        // Minutes that are no whole half-hour keep three places.
        assert_eq!(halves(620), "20.667");
        assert_eq!(halves(1), "0.033");
        assert_eq!(halves(1439), "47.967");
        // A slot that would reach past the frame stops at 24:00.
        assert_eq!(geometry(&[slot(1, 1380, 1500)], MIN_HOURS).unwrap().1, ["--from:46;--to:48;--lane:0;--lanes:1"]);
    }

    #[test]
    fn a_slot_names_its_look() {
        assert_eq!(slot(1, 555, 645).classes(), "slot");
        let tinted = GridSlot { class: "tinted", hue: Some("t-ice"), alt: true, clash: true, ..slot(1, 555, 645) };
        assert_eq!(tinted.classes(), "slot tinted t-ice alt clash");
        assert_eq!(GridSlot { class: "once", ..slot(1, 555, 645) }.classes(), "slot once");
    }
}
