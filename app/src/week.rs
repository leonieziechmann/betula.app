//! The week grid of a module and of the Studienplan.
//!
//! One component draws both: a module's page shows its own Termine in it, the Studienplan its
//! Regelwoche. Callers hand over slots in minutes; the grid finds the days and hours it needs and
//! puts slots that overlap on a day side by side (`catalog::timetable::grid`), so that a single
//! date no longer covers the weekly group beneath it. The geometry is data only, written into
//! custom properties in half-hours (`--days`, `--first`, `--span` on `.week`; `--from`, `--to`,
//! `--lane`, `--lanes` and, where a lane is not one of equal ones, `--wide`, `--mine`, `--theirs`
//! and `--beside` per slot), and `app.css` turns it into places.
//!
//! Another planned module beside a module's own slots (`planned`) is the context, not the
//! subject: where it meets the module's own slots it takes a slim lane at the right, half as wide
//! as one of theirs, and in a narrow day less, down to a sliver, so that an own slot keeps room
//! for „Prak". A slot's first line breaks between words only and holds two lines at most (one in
//! a slot under an hour), then ends in „…"; a first word too wide for the slot is cut with „…"
//! and ends the label.

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
    /// The first line („VL Entwicklung von Softwaresystemen", „Prak"), at most two lines of whole
    /// words, then „…"; the smaller line under it („09:15", „3 Termine"; left out when empty),
    /// and the tooltip (left out when empty).
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

    /// The slot's classes: `slot`, then its look, its tint, `alt` and `clash`, and `brief` for
    /// one shorter than an hour (its first line holds one line of words, not two).
    pub fn classes(&self) -> String {
        let mut classes = String::from("slot");
        let brief = self.to.saturating_sub(self.from) < 60;
        let parts = [self.class, self.hue.unwrap_or_default(), if self.alt { "alt" } else { "" }, if self.clash { "clash" } else { "" }, if brief { "brief" } else { "" }];
        for part in parts.into_iter().filter(|part| !part.is_empty()) {
            classes.push(' ');
            classes.push_str(part);
        }
        classes
    }

    fn drawn(&self) -> bool {
        (1..=7).contains(&self.day)
    }

    /// Another planned module beside the grid's own slots: it takes a slim lane (see the module's
    /// doc).
    fn beside(&self) -> bool {
        self.class == "planned"
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
    let week = format!("--days:{};--first:{};--span:{}", span.days, halves(span.first), halves(span.last.saturating_sub(span.first)));
    let styles = slots
        .iter()
        .zip(lanes_of(slots))
        .map(|(slot, lane)| {
            // The frame ends at 24:00; what would reach past it stops there.
            let to = slot.to.max(slot.from).min(span.last);
            let from = slot.from.min(to);
            format!("--from:{};--to:{};{lane}", halves(from), halves(to))
        })
        .collect();
    Some(Layout { span, week, slots: styles })
}

/// Per slot, where it stands in its day's column: `--lane:<n>;--lanes:<n>` for one of equal
/// lanes, as `grid::lanes` has them. In a group of overlapping slots where the module's own slots
/// meet planned ones, the column has two parts: the own slots' `--mine` lanes on the left, the
/// planned slots' `--theirs` on the right (`--beside:1`); `--lane`, `--lanes` and `--wide` count
/// lanes of the slot's part. `app.css` sizes the parts: a planned lane half as wide as an own
/// one, and in a narrow day less, so that an own lane keeps room for its label.
fn lanes_of(slots: &[GridSlot]) -> Vec<String> {
    let placed: Vec<Placed> = slots.iter().map(GridSlot::placed).collect();
    // Lanes are per day, so a slot that is not drawn shares a group with none that is; a slot
    // put on no day (0) is out of the other set's lanes.
    let only = |beside: bool| -> Vec<(u8, u8)> {
        let placed: Vec<Placed> = slots.iter().map(|slot| Placed { day: if slot.beside() == beside { slot.day } else { 0 }, ..slot.placed() }).collect();
        grid::lanes(&placed)
    };
    let (all, own, planned) = (grid::lanes(&placed), only(false), only(true));
    let groups = groups_of(&placed);
    // Per group: the most lanes its own slots and its planned slots need among themselves.
    let mut need: Vec<(u8, u8)> = vec![(0, 0); groups.iter().copied().max().map_or(0, |last| last + 1)];
    for (i, slot) in slots.iter().enumerate() {
        let (Some(group), Some(own), Some(planned)) = (groups.get(i), own.get(i), planned.get(i)) else { continue };
        if let Some(need) = need.get_mut(*group) {
            match slot.beside() {
                false => need.0 = need.0.max(own.1),
                true => need.1 = need.1.max(planned.1),
            }
        }
    }
    slots
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let group = groups.get(i).and_then(|group| need.get(*group)).copied().unwrap_or_default();
            let (lane, lanes) = all.get(i).copied().unwrap_or((0, 1));
            let (mine, theirs) = group;
            if mine == 0 || theirs == 0 {
                return format!("--lane:{lane};--lanes:{lanes}");
            }
            // A slot of fewer lanes than its part has spans as many of them as its share.
            let (part, (lane, lanes), beside) = match slot.beside() {
                false => (mine, own.get(i).copied().unwrap_or((0, 1)), ""),
                true => (theirs, planned.get(i).copied().unwrap_or((0, 1)), ";--beside:1"),
            };
            let unit = f64::from(part) / f64::from(lanes.max(1));
            format!("--lane:{};--lanes:{part};--wide:{};--mine:{mine};--theirs:{theirs}{beside}", number(unit * f64::from(lane)), number(unit))
        })
        .collect()
}

/// Per slot, the group of slots it overlaps transitively on its day (numbered from 0), as
/// `grid::lanes` groups them.
fn groups_of(placed: &[Placed]) -> Vec<usize> {
    let mut order: Vec<(usize, &Placed)> = placed.iter().enumerate().collect();
    order.sort_by_key(|(i, slot)| (slot.day, slot.from, std::cmp::Reverse(slot.to.saturating_sub(slot.from)), *i));
    let mut groups = vec![0; placed.len()];
    let (mut group, mut day, mut end) = (0usize, None, 0u16);
    for (n, (i, slot)) in order.into_iter().enumerate() {
        if n > 0 && (day != Some(slot.day) || slot.from >= end) {
            group += 1;
            end = 0;
        }
        day = Some(slot.day);
        end = end.max(slot.to.max(slot.from));
        if let Some(entry) = groups.get_mut(i) {
            *entry = group;
        }
    }
    groups
}

/// A number of the grid's CSS as short as it goes: „2", „0.667".
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
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
    let label = view! { <span class="l">{words(&slot.label)}</span> };
    let small = (!slot.small.is_empty()).then(|| view! { <small>{slot.small.clone()}</small> });
    match slot.href.clone() {
        Some(href) => view! { <a class=class href=href data-noscroll="" style=style title=title aria-current=current>{label}{small}</a> }.into_any(),
        None => view! { <div class=class style=style title=title aria-current=current>{label}{small}</div> }.into_any(),
    }
}

/// A label word by word, each a box of its own (`span.w`): lines break between words only, and a
/// word wider than its slot ends in „…" instead of being cut off at the slot's edge. The first
/// word's text has a box of its own inside it: its outer box keeps the word's whole width, which
/// `app.css` compares with the slot's to end the label at a first word it has to cut („Mathe…",
/// not „Mathe…" over „IT-1", review 2026-09-25).
fn words(text: &str) -> impl IntoView {
    let units = units(text);
    let last = units.len().saturating_sub(1);
    units
        .into_iter()
        .enumerate()
        .map(|(i, unit)| {
            let space = (i < last).then_some(" ");
            match i {
                0 => view! { <span class="w"><span>{unit}</span></span>{space} }.into_any(),
                _ => view! { <span class="w">{unit}</span>{space} }.into_any(),
            }
        })
        .collect_view()
}

/// The boxes of a label: its words, a short lowercase word („und", „von", „der") together with
/// the word after it, so no line ends on one („Entwicklung" / „von Software…", not „Entwicklung
/// von" / „Softwaresystemen" cut to „von…").
fn units(text: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    let mut open: Option<String> = None;
    for word in text.split_whitespace() {
        // The „…" of a cut name stays with the word before it.
        if word == "…" && open.is_none() {
            if let Some(last) = units.last_mut() {
                last.push_str(" …");
                continue;
            }
        }
        let unit = match open.take() {
            Some(before) => format!("{before} {word}"),
            None => word.to_string(),
        };
        let small = word.chars().count() <= 4 && word.chars().all(char::is_lowercase);
        if small {
            open = Some(unit);
        } else {
            units.push(unit);
        }
    }
    units.extend(open);
    units
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
    fn a_planned_module_beside_the_own_slots_is_slim() {
        let planned = |day: u8, from: u16, to: u16| GridSlot { class: "planned", ..slot(day, from, to) };
        let lanes = |slots: &[GridSlot]| {
            let (_, styles) = geometry(slots, MIN_HOURS).unwrap();
            styles.into_iter().map(|style| style.split_once(";--lane").map(|(_, lane)| format!("--lane{lane}")).unwrap_or_default()).collect::<Vec<_>>()
        };
        // One own lane, one planned: the own part and the planned part at the right, each a lane.
        assert_eq!(
            lanes(&[slot(2, 555, 645), planned(2, 555, 645)]),
            ["--lane:0;--lanes:1;--wide:1;--mine:1;--theirs:1", "--lane:0;--lanes:1;--wide:1;--mine:1;--theirs:1;--beside:1"]
        );
        // Two planned beside one own: they share the planned part, the own slot keeps its own.
        assert_eq!(
            lanes(&[planned(2, 555, 645), slot(2, 555, 645), planned(2, 600, 700)]),
            [
                "--lane:0;--lanes:2;--wide:1;--mine:1;--theirs:2;--beside:1",
                "--lane:0;--lanes:1;--wide:1;--mine:1;--theirs:2",
                "--lane:1;--lanes:2;--wide:1;--mine:1;--theirs:2;--beside:1"
            ]
        );
        // Two own lanes beside a planned one, and an own slot of the same group that meets only
        // the planned one: the whole own part.
        assert_eq!(
            lanes(&[slot(3, 555, 645), slot(3, 555, 645), planned(3, 600, 720), slot(3, 690, 780)]),
            [
                "--lane:0;--lanes:2;--wide:1;--mine:2;--theirs:1",
                "--lane:1;--lanes:2;--wide:1;--mine:2;--theirs:1",
                "--lane:0;--lanes:1;--wide:1;--mine:2;--theirs:1;--beside:1",
                "--lane:0;--lanes:2;--wide:2;--mine:2;--theirs:1"
            ]
        );
        // Alone, or among themselves, planned slots take lanes as any do.
        assert_eq!(lanes(&[planned(4, 555, 645), planned(4, 555, 645), slot(5, 555, 645)]), ["--lane:0;--lanes:2", "--lane:1;--lanes:2", "--lane:0;--lanes:1"]);
        assert_eq!(number(2.0), "2");
        assert_eq!(number(2.0 / 3.0), "0.667");
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
        // A label breaks between words, never after „und" or „von".
        assert_eq!(units("VL Entwicklung von Softwaresystemen"), ["VL", "Entwicklung", "von Softwaresystemen"]);
        assert_eq!(units("VL Elektrische und elektronische …"), ["VL", "Elektrische", "und elektronische …"]);
        assert_eq!(units("Grundlagen der und"), ["Grundlagen", "der und"]);
        assert_eq!(units("Ü Mathematik IT-1"), ["Ü", "Mathematik", "IT-1"]);
        // Under an hour its first line holds one line.
        assert_eq!(slot(1, 555, 600).classes(), "slot brief");
        assert_eq!(GridSlot { class: "planned", ..slot(1, 555, 614) }.classes(), "slot planned brief");
        assert_eq!(slot(1, 555, 615).classes(), "slot");
    }
}
