//! The week grid of a module and of the Studienplan.
//!
//! One component draws both: a module's page shows its own Termine in it, the Studienplan its
//! Regelwoche. Callers hand over slots in minutes; the grid finds the days and hours it needs and
//! puts slots that overlap on a day side by side (`folia_timetable::grid`), so that a single
//! date no longer covers the weekly group beneath it. The geometry is data only, written into
//! custom properties in half-hours (`--days`, `--first`, `--span` and, fitted, the widths of the
//! days `--cols` on `.week`; `--from`, `--to`,
//! `--lane`, `--lanes` and, where a lane is not one of equal ones, `--wide`, `--mine`, `--theirs`
//! and `--beside` per slot), and `app.css` turns it into places.
//!
//! A slot the page lets act on (the Stundenplan's) has buttons of its own beside its link, over its
//! bottom right corner (`div.slot-acts > button.slot-act`, each key in `data-act`): „✓" and „×",
//! what they do in their tooltip, a „✓" that is on filled (`pressed`). The page listens for their
//! clicks on the grid's box. They come when the slot is pointed at, and stay where a decision is
//! due or can be taken back (`asks`).
//!
//! Slots that stand together (`joint`: the Stundenplan's A-week and B-week Termine of a module at
//! one time, which never meet each other and clash together) are drawn in one red frame where their
//! lanes lie side by side (`div.slot-frame`), each without a frame of its own (`framed`).
//!
//! The plan beside a module's own slots (`planned`, folia/design/kalender/raster.html; owner,
//! 2026-11-03: „Passt alles, bau das jetzt in Folia") is the context, not the subject: it is the
//! ground of the module's week. Its slots have lanes among themselves in the day's whole width; the
//! module's own stand over them as cards, each narrowed by 14 px per lane of the plan under it
//! (`--free`), so that the plan shows at its right, its name upright there where the card covers
//! its start (`upright`); of more than one lane under a card the plan stands as strips at the
//! right instead (`edge`). The plan never makes the frame grow: it is cut to the module's hours,
//! its cut edge torn (`cutt`, `cutb`) and its time named, where it ends under its day („bis
//! 17:00"), where it begins in its box („ab 07:30"); a slot of the plan wholly after the frame is
//! named under its day („+ ABWL1 17:30", `--spill` lines on `.week`), one wholly before it left
//! out. The hours stand at their lines, the frame's last one too. A slot's first line breaks
//! between words only and holds two lines at most (one in a slot under an hour), then ends in
//! „…"; a first word too wide for the slot is cut with „…" and ends the label.
//!
//! A Termin of one's own (folia/design/kalender/termine.html) carries the mark „von dir" in its
//! corner (`own`); the grid names the minutes its hours begin and end at (`data-first`,
//! `data-last` on `.week`), so that the Stundenplan finds the time of a click on a free stretch of
//! a day (its „+ Termin" there).
//!
//! A calendar of one's own beside the Stundenplan's (folia/design/kalender, owner 2026-10-26: „A mit
//! Fokus und Spur aus C") stands in the week in full (`cal`: its Termine among the plan's, in their
//! lanes, outlined in its colour with its initial) or as a strip (`strip`): a band at the right edge
//! of each day that the day's own slots leave free (`--strip` on `.week`), its Termine thin bars in
//! it, side by side among themselves where they overlap, so that the plan keeps its width. Behind
//! the slots a day can have stretches of its own (`GridBand`): the time free in every calendar shown
//! („Gemeinsam frei").
//!
//! The Stundenplan's grid fits the room it is given (`fit`, owner 2026-09-25: the whole week on the
//! screen, and room where modules meet): its height is the page's, whatever hours it shows — more
//! hours squeeze it („einfach gesquished"), they never make it taller — and each day is as wide as
//! its lanes need (`--cols`): a day of one lane the unit, an empty one less, one where modules
//! stand side by side more. It shows 08:00 to 18:00 at least, so its frame stays put while slots
//! come and go, and without a slot it is that frame, an empty week. The slots are placed in
//! fractions of the day's height, so both grids share one geometry.

use std::collections::BTreeMap;

use folia_calendar::day::clock;
use folia_timetable::grid::{self, Placed, Span};
use leptos::prelude::*;

use crate::i18n;
use folia_design::ui::Icon;

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
    /// A third line, where the slot is held („ZHG/HS.C"); left out when empty, and in a slot too
    /// short for three lines.
    pub place: String,
    /// The look: "" (a lecture), "other" (any other teaching), "tinted" (a module of the
    /// Studienplan, in its `hue`), "planned" (another planned module beside a module's own
    /// slots), "once" (single dates), "faint" (what the Stundenplan leaves out, in „Alle
    /// Termine"), "cal" (a calendar of one's own in full, its colour the `hue`) and "strip" (one as
    /// a strip at the day's edge, see the module's doc).
    pub class: &'static str,
    /// The tint of a `tinted` slot: one of the `t-…` classes of `app.css` („t-ice" … „t-slate").
    pub hue: Option<&'static str>,
    /// The course of a module held at two campuses or more the slot belongs to, where its page
    /// tells them apart (`course-1` … `course-4`, app.css): a stripe in the course's colour.
    pub mark: Option<&'static str>,
    /// An option of a choice not yet made (dashed), and a slot in a hard clash.
    pub alt: bool,
    pub clash: bool,
    /// An extra beside a module's compulsory part (dotted and lighter, app.css `.slot.extra`).
    pub extra: bool,
    /// A module only visited (hatched in its tone, app.css `.slot.visit`), and one that gives way
    /// where it meets the plan (`gives`, a yellow edge instead of a clash's red).
    pub visit: bool,
    pub gives: bool,
    /// Where a click leads: `Some` draws a link that keeps the page's scroll, `None` a plain box.
    pub href: Option<String>,
    /// The lines of the list under the grid that the slot stands for, by their ids (a module's
    /// Termine): the slot is a link to the first of them, and a click shows them there instead of
    /// following it (`data-action="termin"`, enhance.js). Empty for a slot that shows nothing below.
    pub lines: Vec<String>,
    /// The slot of what is shown beside the grid (`aria-current`).
    pub current: bool,
    /// The page's buttons on the slot, in their order (`slot_buttons`); none for a slot to look at.
    pub acts: Vec<SlotButton>,
    /// A decision about the slot is due: its buttons show without pointing at it.
    pub asks: bool,
    /// The slots of one `joint` on a day stand together: where they clash, one red frame is drawn
    /// around them (`frames_of`).
    pub joint: Option<u32>,
    /// A group of the Stundenplan's choice in focus (folia/design/kalender): ticked (`Some(true)`)
    /// or not, drawn outlined over the rest. Its `tick` is the key a click on it acts by.
    pub pick: Option<bool>,
    /// The key of what a click on the slot does (`data-act`, as a button's): the slot itself is
    /// the box that ticks its group, a button, not a link.
    pub tick: Option<String>,
    /// What a choice in focus leaves behind it: the rest of the plan, which steps back.
    pub dim: bool,
    /// The key of what a click on the slot's link does besides following it (`data-pre`): a group
    /// of a choice still open is ticked as its link takes the choice into focus (owner,
    /// 2026-10-26: „Wenn man auf eine Übung klickt, dann soll die Preselected sein").
    pub pre: Option<String>,
    /// The initial of the calendar of one's own a slot in full belongs to („L"), in its corner.
    pub badge: Option<String>,
    /// A Termin of one's own (folia/design/kalender/termine.html): the mark „von dir" in its corner
    /// (`.slot-own`), and the slot `own`.
    pub own: bool,
    /// A Termin of QIS changed by hand (step 2): the pen in its corner (`.slot-pen`), and amber
    /// where QIS says something else now, which the student decides (`conflict`).
    pub edited: bool,
    pub conflict: bool,
    /// A group of an event of groups, one of which a student attends (`Attendance::OneOf`, „eine
    /// von 4"): to choose (`alt`), or one's own, ticked in the plan (`chosen`: full, with a tick in
    /// its corner; folia/design/kalender/raster.html).
    pub group: bool,
    pub chosen: bool,
}

/// A stretch of a day drawn behind the slots, from `from` to `to` (minutes): the time every
/// calendar shown has free („Gemeinsam frei"), with its words (`label`, in it where there is room)
/// and its tooltip.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridBand {
    pub day: u8,
    pub from: u16,
    pub to: u16,
    pub label: String,
    pub title: String,
}

/// A button the page puts on a slot: its icon („check", „x"), what it does in words (its name and
/// tooltip), the key the page knows it by (`data-act`), and whether it is on (`aria-pressed`: a
/// „✓" of what was taken, whose click takes it back).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlotButton {
    pub icon: &'static str,
    pub label: String,
    pub key: String,
    pub pressed: bool,
}

/// The buttons of a slot or of a line of the Stundenplan: `button.slot-act[data-act][data-icon]`.
pub fn slot_buttons(buttons: &[SlotButton]) -> AnyView {
    buttons
        .iter()
        .map(|button| {
            view! {
                <button
                    class="slot-act"
                    type="button"
                    data-act=button.key.clone()
                    data-icon=button.icon
                    aria-pressed=button.pressed.then_some("true")
                    aria-label=button.label.clone()
                    title=button.label.clone()
                >
                    <Icon name=button.icon/>
                </button>
            }
        })
        .collect_view()
        .into_any()
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
        let parts = [
            self.class,
            self.hue.unwrap_or_default(),
            self.mark.unwrap_or_default(),
            if self.alt { "alt" } else { "" },
            if self.clash { "clash" } else { "" },
            if self.extra { "extra" } else { "" },
            if self.visit { "visit" } else { "" },
            if self.gives { "gives" } else { "" },
            if brief { "brief" } else { "" },
            if self.pick.is_some() { "pick" } else { "" },
            if self.pick == Some(true) { "on" } else { "" },
            if self.dim { "dim" } else { "" },
            if self.own { "own" } else { "" },
            if self.edited { "edited" } else { "" },
            if self.conflict { "conflict" } else { "" },
            if self.group { "group" } else { "" },
            if self.chosen { "chosen" } else { "" },
        ];
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

    /// A calendar of one's own as a strip: it takes no lane of the day's, but one of the band at
    /// the day's edge (see the module's doc).
    fn strip(&self) -> bool {
        self.class == "strip"
    }

    /// Where it sits among the day's lanes: nowhere for a strip, which is put on no day (0).
    fn in_lanes(&self) -> Placed {
        Placed { day: if self.strip() { 0 } else { self.day }, ..self.placed() }
    }
}

/// What the grid of a set of slots needs: its frame, the style of `.week` and, per slot in the
/// order of the input, the slot's style and what the frame and the lanes made of it; the red
/// frames around slots that stand together, by day, and per slot whether it is in one.
struct Layout {
    span: Span,
    week: String,
    slots: Vec<String>,
    marks: Vec<Marks>,
    frames: Vec<(u8, String)>,
    framed: Vec<bool>,
}

/// What the frame and the lanes made of a slot: its classes (`cutt`, `cutb` where the frame cuts a
/// slot of the plan; `upright`, `edge` where the module's own stand over it), the times of a cut
/// slot (`since`, where it begins before the frame; `until`, where it ends after it), and a slot
/// of the plan wholly after the frame, named under its day (`after`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Marks {
    classes: Vec<&'static str>,
    since: Option<u16>,
    until: Option<u16>,
    after: bool,
}

/// The width a lane of the plan takes beside a card of the module's own: 13 px and the gap.
#[cfg(test)]
const PLAN_LANE: u16 = 14;

/// The hour a fitted grid shows the day until at least (it starts at 08:00 at the latest,
/// `grid::span`).
const FIT_UNTIL: u16 = 18 * 60;

fn layout(slots: &[GridSlot], min_hours: u16, fit: bool) -> Option<Layout> {
    // The frame is the module's own: the plan beside it (`planned`) never makes it grow, and is
    // cut to it (`frame_cut`). A grid of the plan's slots alone takes their frame.
    let framing: Vec<Placed> = slots.iter().filter(|slot| slot.drawn() && !slot.beside()).map(GridSlot::placed).collect();
    let drawn: Vec<Placed> = match framing.is_empty() {
        true => slots.iter().filter(|slot| slot.drawn()).map(GridSlot::placed).collect(),
        false => framing,
    };
    if drawn.is_empty() && !fit {
        return None;
    }
    let mut span = grid::span(&drawn, min_hours.saturating_mul(60));
    if fit {
        span.last = span.last.max(FIT_UNTIL);
    }
    let (cut, mut marks) = frame_cut(slots, &span);
    let mut week = format!("--days:{};--first:{};--span:{}", span.days, halves(span.first), halves(span.last.saturating_sub(span.first)));
    if fit {
        week.push_str(";--cols:");
        week.push_str(&columns(&cut, span.days));
    }
    // The band of the strips, at the edge of every day, where any is drawn.
    if cut.iter().any(|slot| slot.strip() && slot.drawn()) {
        week.push_str(";--strip:1");
    }
    // The lines under the days that name what of the plan the frame cuts or leaves out after it.
    let spill = (1..=span.days).map(|day| slots.iter().zip(&marks).filter(|(slot, marks)| slot.day == day && (marks.after || marks.until.is_some())).count()).max().unwrap_or(0);
    if spill > 0 {
        week.push_str(&format!(";--spill:{spill}"));
    }
    let (lanes, classes) = lanes_of(&cut);
    for (marks, more) in marks.iter_mut().zip(classes) {
        marks.classes.extend(more);
    }
    let styles = cut
        .iter()
        .zip(lanes)
        .map(|(slot, lane)| {
            // The frame ends at 24:00; what would reach past it stops there.
            let to = slot.to.max(slot.from).min(span.last);
            let from = slot.from.min(to);
            format!("--from:{};--to:{};{lane}", halves(from), halves(to))
        })
        .collect();
    let (frames, framed) = frames_of(&cut, span.last);
    Some(Layout { span, week, slots: styles, marks, frames, framed })
}

/// The slots as the frame shows them: a slot of the plan beside a module's week (`planned`) cut to
/// the frame's hours, with what the cut did (`Marks`); one wholly outside them, or on a day the
/// frame does not have, put on no day (0), and noted where it lies after the frame.
fn frame_cut(slots: &[GridSlot], span: &Span) -> (Vec<GridSlot>, Vec<Marks>) {
    slots
        .iter()
        .map(|slot| {
            let mut marks = Marks::default();
            if !slot.beside() || !slot.drawn() {
                return (slot.clone(), marks);
            }
            if slot.day > span.days || slot.to <= span.first || slot.from >= span.last {
                marks.after = slot.day <= span.days && slot.from >= span.last;
                return (GridSlot { day: 0, ..slot.clone() }, marks);
            }
            if slot.from < span.first {
                marks.classes.push("cutt");
                marks.since = Some(slot.from);
            }
            if slot.to > span.last {
                marks.classes.push("cutb");
                marks.until = Some(slot.to);
            }
            (GridSlot { from: slot.from.max(span.first), to: slot.to.min(span.last), ..slot.clone() }, marks)
        })
        .unzip()
}

/// The red frames around slots that stand together (`GridSlot::joint`): per day and joint, where
/// two or more of its slots lie in lanes side by side, one box from the first lane to the last and
/// from the earliest start to the latest end (up to `last`), as a slot's style; and per slot
/// whether it is in one, which takes the place of its own frame and mark.
fn frames_of(slots: &[GridSlot], last: u16) -> (Vec<(u8, String)>, Vec<bool>) {
    let lanes = grid::lanes(&slots.iter().map(GridSlot::in_lanes).collect::<Vec<_>>());
    let mut groups: BTreeMap<(u8, u32), Vec<usize>> = BTreeMap::new();
    for (i, slot) in slots.iter().enumerate() {
        if let Some(joint) = slot.joint.filter(|_| slot.drawn()) {
            groups.entry((slot.day, joint)).or_default().push(i);
        }
    }
    let mut framed = vec![false; slots.len()];
    let mut frames = Vec::new();
    for ((day, _), members) in groups {
        let placed: Vec<((u8, u8), &GridSlot)> = members.iter().filter_map(|i| Some((*lanes.get(*i)?, slots.get(*i)?))).collect();
        let (Some(first), Some(end), Some(((_, total), _))) =
            (placed.iter().map(|((lane, _), _)| *lane).min(), placed.iter().map(|((lane, _), _)| *lane).max(), placed.first())
        else {
            continue;
        };
        let side_by_side = placed.len() >= 2 && placed.iter().all(|((_, lanes), _)| lanes == total) && usize::from(end.saturating_sub(first)) + 1 == placed.len();
        if !side_by_side {
            continue;
        }
        let to = placed.iter().map(|(_, slot)| slot.to.max(slot.from)).max().unwrap_or_default().min(last);
        let from = placed.iter().map(|(_, slot)| slot.from).min().unwrap_or_default().min(to);
        frames.push((day, format!("--from:{};--to:{};--lane:{first};--lanes:{total};--wide:{}", halves(from), halves(to), placed.len())));
        for i in members {
            if let Some(mark) = framed.get_mut(i) {
                *mark = true;
            }
        }
    }
    (frames, framed)
}

/// How long after its start a slot of the plan still names itself at its top: where a card of the
/// module's own begins later, the plan's name stands above it, where it begins earlier, upright in
/// the plan's strip beside it.
const NAME_ROOM: u16 = 20;

/// Per slot, where it stands in its day's column (`--lane:<n>;--lanes:<n>`, as `grid::lanes` has
/// them), and the classes that come of it. Where the plan stands beside a module's week (`planned`,
/// see the module's doc) it is the ground: its slots have lanes among themselves in the day's
/// whole width, the module's own lanes among theirs over it, each narrowed by a lane of the plan
/// per lane of it under the slot (`--free`). A slot of the plan whose start a card covers names
/// itself upright at its right (`upright`); one of more than one lane under a card stands as a
/// strip at the right (`edge`), where the card would cover it.
fn lanes_of(slots: &[GridSlot]) -> (Vec<String>, Vec<Vec<&'static str>>) {
    let placed: Vec<Placed> = slots.iter().map(GridSlot::in_lanes).collect();
    // Lanes are per day, so a slot that is not drawn shares a group with none that is; a slot
    // put on no day (0) is out of the other set's lanes.
    let only = |ground: bool| -> Vec<(u8, u8)> {
        let placed: Vec<Placed> = slots.iter().map(|slot| Placed { day: if slot.beside() == ground { slot.in_lanes().day } else { 0 }, ..slot.placed() }).collect();
        grid::lanes(&placed)
    };
    let (all, own, ground) = (grid::lanes(&placed), only(false), only(true));
    // The strips' lanes are among the strips, in the band at the day's edge.
    let strips = grid::lanes(&slots.iter().map(|slot| Placed { day: if slot.strip() { slot.day } else { 0 }, ..slot.placed() }).collect::<Vec<_>>());
    let plan = slots.iter().any(|slot| slot.beside() && slot.drawn());
    let meet = |a: &GridSlot, b: &GridSlot| a.drawn() && b.drawn() && a.day == b.day && a.from < b.to && b.from < a.to;
    slots
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            if slot.strip() {
                let (lane, lanes) = strips.get(i).copied().unwrap_or((0, 1));
                return (format!("--lane:{lane};--lanes:{lanes}"), Vec::new());
            }
            if !plan {
                let (lane, lanes) = all.get(i).copied().unwrap_or((0, 1));
                return (format!("--lane:{lane};--lanes:{lanes}"), Vec::new());
            }
            if slot.beside() {
                let (lane, lanes) = ground.get(i).copied().unwrap_or((0, 1));
                let cards: Vec<&GridSlot> = slots.iter().filter(|card| !card.beside() && !card.strip() && meet(card, slot)).collect();
                let classes = match (cards.is_empty(), lanes > 1) {
                    (false, true) => vec!["edge", "upright"],
                    (false, false) if cards.iter().any(|card| card.from <= slot.from.saturating_add(NAME_ROOM)) => vec!["upright"],
                    _ => Vec::new(),
                };
                return (format!("--lane:{lane};--lanes:{lanes}"), classes);
            }
            let (lane, lanes) = own.get(i).copied().unwrap_or((0, 1));
            let free = slots.iter().zip(&ground).filter(|(under, _)| under.beside() && meet(under, slot)).map(|(_, (_, lanes))| *lanes).max().unwrap_or(0);
            let free = if free > 0 { format!(";--free:{free}") } else { String::new() };
            (format!("--lane:{lane};--lanes:{lanes}{free}"), Vec::new())
        })
        .unzip()
}

/// The share of the week an empty day takes (a day of one lane takes 1): it keeps its head.
const EMPTY_DAY: f64 = 0.6;

/// What each lane after a day's first adds to its share, and the most a day takes: the modules
/// that stand side by side get room, and the days of one lane give it (owner, 2026-09-25: „da wo
/// es mehrere Module gleichzeitig gibt, etwas mehr Platz").
const MORE_LANE: f64 = 0.6;
const WIDEST_DAY: f64 = 3.4;

/// The widths of the days of a fitted grid (`--cols`), Monday first: per day a share of the week
/// by the most lanes a group of its slots needs (`grid::lanes`).
fn columns(slots: &[GridSlot], days: u8) -> String {
    let placed: Vec<Placed> = slots.iter().map(GridSlot::in_lanes).collect();
    let mut most = [0u8; 7];
    for (slot, (_, lanes)) in slots.iter().zip(grid::lanes(&placed)) {
        let day = usize::from(slot.day).checked_sub(1).and_then(|day| most.get_mut(day));
        if let (true, Some(day)) = (slot.drawn() && !slot.strip(), day) {
            *day = (*day).max(lanes);
        }
    }
    most.iter().take(usize::from(days)).map(|lanes| format!("minmax(0,{}fr)", number(day_share(*lanes)))).collect::<Vec<_>>().join(" ")
}

/// The share of the week a day of `lanes` lanes takes (none: an empty day).
fn day_share(lanes: u8) -> f64 {
    match lanes.checked_sub(1) {
        None => EMPTY_DAY,
        Some(more) => (1.0 + MORE_LANE * f64::from(more)).min(WIDEST_DAY),
    }
}

/// A number of the grid's CSS as short as it goes: „2", „0.667".
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The geometry the grid writes, as plain data: the style of `.week` and one style per slot, in
/// the order of `slots`. `None` when no slot is drawn, and then a module's grid is not there at
/// all (a frame of empty hours says nothing of a module).
pub fn geometry(slots: &[GridSlot], min_hours: u16) -> Option<(String, Vec<String>)> {
    layout(slots, min_hours, false).map(|layout| (layout.week, layout.slots))
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
/// side by side where they overlap. The heads of the days are the weekdays in a few letters, in
/// the page's language („Mo" … „So", "Mon" … "Sun"), Monday first; `heads` names the days
/// instead. `fit`: the grid fits the room it is given (see the module's doc); `bands`: stretches of
/// days behind the slots (`GridBand`). Without any slot to draw, a module's grid renders nothing and
/// a fitted one its frame, an empty week.
#[component]
pub fn WeekGrid(
    #[prop(into)] slots: Signal<Vec<GridSlot>>,
    #[prop(optional)] heads: Option<Vec<String>>,
    #[prop(optional)] min_hours: Option<u16>,
    #[prop(optional)] fit: bool,
    #[prop(optional, into)] bands: Option<Signal<Vec<GridBand>>>,
) -> impl IntoView {
    let t = i18n::t();
    let min_hours = min_hours.unwrap_or(MIN_HOURS);
    move || {
        let bands = bands.map(|bands| bands.get()).unwrap_or_default();
        slots.with(|slots| {
            let Layout { span, week, slots: styles, marks, frames, framed } = layout(slots, min_hours, fit)?;
            let days = &t.data.weekdays_short;
            let heads: Vec<String> = (0..usize::from(span.days))
                .map(|i| heads.as_ref().and_then(|heads| heads.get(i).cloned()).or_else(|| days.get(i).map(|head| head.to_string())).unwrap_or_default())
                .collect();
            let columns = (1..=span.days)
                .map(|day| {
                    let on: Vec<(&GridSlot, &String, &Marks, bool)> = slots
                        .iter()
                        .zip(&styles)
                        .zip(&marks)
                        .zip(&framed)
                        .filter(|(((slot, _), _), _)| slot.day == day)
                        .map(|(((slot, style), marks), framed)| (slot, style, marks, *framed))
                        .collect();
                    let drawn = on.iter().filter(|(_, _, marks, _)| !marks.after).map(|(slot, style, marks, framed)| slot_view(slot, style.to_string(), marks, *framed)).collect_view();
                    let frames = frames.iter().filter(|(on, _)| *on == day).map(|(_, style)| view! { <div class="slot-frame" style=style.clone()></div> }).collect_view();
                    let bands = bands.iter().filter(|band| band.day == day).map(|band| band_view(band, span.last)).collect_view();
                    // Under the day: until when what the frame cuts goes on, and what of the plan
                    // lies wholly after it, line by line.
                    let spills = on
                        .iter()
                        .filter_map(|(slot, style, marks, _)| match (marks.after, marks.until) {
                            (true, _) => Some(((t.week.later)(&slot.label, &clock(slot.from)), "--lane:0;--lanes:1".to_string(), slot.title.clone(), false)),
                            (false, Some(until)) => Some(((t.week.until)(&clock(until)), style.to_string(), slot.title.clone(), marks.classes.contains(&"edge"))),
                            _ => None,
                        })
                        .enumerate()
                        .map(|(row, (text, style, title, edge))| view! { <span class="spill" class:edge=edge style=format!("{style};--row:{row}") title=title>{text}</span> })
                        .collect_view();
                    // Each day in its own column of the grid (`--col`): the lines behind the days take
                    // the row's cells, which auto-placement would leave the days beside.
                    view! { <div class="col" style=format!("--col:{}", day + 1)>{bands}{drawn}{frames}{spills}</div> }
                })
                .collect_view();
            Some(
                view! {
                    <div class="week" class:fit=fit style=week data-first=span.first data-last=span.last>
                        <span></span>
                        {heads.into_iter().map(|head| view! { <span class="d">{head}</span> }).collect_view()}
                        <div class="hours">{(span.first.div_ceil(60)..=span.last / 60).map(|hour| view! { <span style=format!("--at:{}", halves(hour * 60 - span.first))>{hour}</span> }).collect_view()}</div>
                        <div class="lines" aria-hidden="true"></div>
                        {columns}
                    </div>
                }
                .into_any(),
            )
        })
    }
}

/// A stretch of a day behind the slots, within the frame that ends at `last`.
fn band_view(band: &GridBand, last: u16) -> impl IntoView {
    let to = band.to.min(last);
    let from = band.from.min(to);
    let style = format!("--from:{};--to:{}", halves(from), halves(to));
    let title = (!band.title.is_empty()).then(|| band.title.clone());
    view! { <div class="band free" style=style title=title><span>{band.label.clone()}</span></div> }
}

/// A slot, and its buttons; `marks`: what the frame and the lanes made of it; `framed`: it stands in
/// a red frame with others, which is its own.
fn slot_view(slot: &GridSlot, style: String, marks: &Marks, framed: bool) -> AnyView {
    let t = i18n::t();
    let mut class = slot.classes();
    for more in marks.classes.iter().copied().chain(framed.then_some("framed")) {
        class.push(' ');
        class.push_str(more);
    }
    let title = (!slot.title.is_empty()).then(|| slot.title.clone());
    let current = slot.current.then_some("true");
    let label = view! { <span class="l">{words(&slot.label)}</span> };
    // A slot of the plan cut at the frame's start says since when, in its box.
    let small = match marks.since {
        Some(since) => Some((t.week.since)(&clock(since))),
        None => (!slot.small.is_empty()).then(|| slot.small.clone()),
    };
    let small = small.map(|small| view! { <small>{small}</small> });
    let place = (!slot.place.is_empty()).then(|| view! { <small class="p">{slot.place.clone()}</small> });
    let upright = marks.classes.contains(&"upright").then(|| view! { <span class="vt" aria-hidden="true"><span>{slot.label.clone()}</span></span> });
    let place = view! {
        {place}
        {upright}
        {slot.badge.clone().map(|badge| view! { <i class="slot-who" aria-hidden="true">{badge}</i> })}
        {slot.own.then(|| view! { <i class="slot-own" aria-hidden="true"><Icon name="user-round"/></i> })}
        {slot.edited.then(|| view! { <i class="slot-pen" aria-hidden="true"><Icon name="pencil"/></i> })}
        {slot.chosen.then(|| view! { <i class="slot-tick" aria-hidden="true"><Icon name="check"/></i> })}
    };
    // Beside the link, not inside it, in a box of the slot's size (its style), which lets every
    // click but those on its buttons through to the link.
    let act = (!slot.acts.is_empty()).then(|| view! { <div class="slot-acts" class:asks=slot.asks style=style.clone()>{slot_buttons(&slot.acts)}</div> });
    // A group of the choice in focus: the whole slot is the box that ticks it.
    if let Some(key) = slot.tick.clone() {
        let on = slot.pick == Some(true);
        return view! {
            <button class=class type="button" role="checkbox" aria-checked=if on { "true" } else { "false" } data-act=key style=style title=title.clone() aria-label=title>
                {label}{small}{place}
                <i class="pick-box" aria-hidden="true">{on.then(|| view! { <Icon name="check"/> })}</i>
            </button>
        }
        .into_any();
    }
    let drawn = match (slot.lines.first(), slot.href.clone()) {
        // A link to its lines in the list under the grid, which a click shows there (enhance.js);
        // without JavaScript the address's fragment leads to the first.
        (Some(first), _) => view! {
            <a class=class href=format!("#{first}") data-action="termin" data-lines=slot.lines.join(" ") style=style title=title aria-current=current>{label}{small}{place}</a>
        }
        .into_any(),
        (None, Some(href)) => view! { <a class=class href=href data-noscroll="" data-pre=slot.pre.clone() style=style title=title aria-current=current>{label}{small}{place}</a> }.into_any(),
        (None, None) => view! { <div class=class style=style title=title aria-current=current>{label}{small}{place}</div> }.into_any(),
    };
    view! { {drawn}{act} }.into_any()
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
    fn the_plan_is_the_ground_under_a_module_s_own_slots() {
        let planned = |day: u8, from: u16, to: u16| GridSlot { class: "planned", label: "ThI".into(), ..slot(day, from, to) };
        let lanes = |slots: &[GridSlot]| {
            let (_, styles) = geometry(slots, MIN_HOURS).unwrap();
            styles.into_iter().map(|style| style.split_once(";--lane").map(|(_, lane)| format!("--lane{lane}")).unwrap_or_default()).collect::<Vec<_>>()
        };
        // One own slot on the plan: the plan in the day's whole width, the card over it narrowed by
        // a lane of the plan.
        assert_eq!(lanes(&[slot(2, 555, 645), planned(2, 555, 645)]), ["--lane:0;--lanes:1;--free:1", "--lane:0;--lanes:1"]);
        // Two of the plan at different times under one card: their lanes are their own, the card
        // leaves room for both.
        assert_eq!(lanes(&[planned(2, 555, 645), slot(2, 555, 645), planned(2, 600, 700)]), ["--lane:0;--lanes:2", "--lane:0;--lanes:1;--free:2", "--lane:1;--lanes:2"]);
        // Two own lanes over one of the plan, each narrowed.
        assert_eq!(lanes(&[slot(3, 555, 645), slot(3, 555, 645), planned(3, 600, 720)]), ["--lane:0;--lanes:2;--free:1", "--lane:1;--lanes:2;--free:1", "--lane:0;--lanes:1"]);
        // Among themselves the plan's slots take lanes as any do; an own slot apart from them is whole.
        assert_eq!(lanes(&[planned(4, 555, 645), planned(4, 555, 645), slot(5, 555, 645)]), ["--lane:0;--lanes:2", "--lane:1;--lanes:2", "--lane:0;--lanes:1"]);
        // The plan names itself upright where a card covers its start, stands as strips at the
        // right where it has two lanes under a card, and is plain where it is free.
        let marks = |slots: &[GridSlot]| layout(slots, MIN_HOURS, false).unwrap().marks.into_iter().map(|marks| marks.classes).collect::<Vec<_>>();
        assert_eq!(marks(&[slot(2, 555, 645), planned(2, 555, 645)]), [vec![], vec!["upright"]]);
        assert_eq!(marks(&[slot(2, 600, 690), planned(2, 555, 645)]), [Vec::<&str>::new(), vec![]], "a card 45 minutes later: the name stands above it");
        assert_eq!(marks(&[planned(2, 555, 645), slot(2, 555, 645), planned(2, 600, 700)]), [vec!["edge", "upright"], vec![], vec!["edge", "upright"]]);
        assert_eq!(marks(&[planned(4, 555, 645), slot(5, 555, 645)]), [Vec::<&str>::new(), vec![]]);
        assert_eq!(PLAN_LANE, 14, "app.css: a lane of the plan beside a card, 13 px and the gap");
    }

    #[test]
    fn the_frame_cuts_the_plan_and_says_so() {
        let planned = |day: u8, from: u16, to: u16| GridSlot { class: "planned", label: "ThI".into(), ..slot(day, from, to) };
        // Programmierpraktikum's own Termine (Mo 11:30–13:00, Fr 13:45–15:15) frame its week
        // 08:00–16:00. Of the plan, ThI's tutorial Mo 15:30–17:00 is cut at 16:00, a lab Di
        // 07:30–09:00 at 08:00; an Übung Di 17:30–19:00 lies after the frame, one Mi 06:00–07:30
        // before it, one on a Saturday off the week.
        let slots = [slot(1, 690, 780), slot(5, 825, 915), planned(1, 930, 1020), planned(2, 450, 540), planned(2, 1050, 1140), planned(3, 360, 450), planned(6, 600, 690)];
        let cut = layout(&slots, MIN_HOURS, false).unwrap();
        let own = layout(&slots[..2], MIN_HOURS, false).unwrap();
        assert_eq!(own.week, "--days:5;--first:16;--span:16");
        assert_eq!(cut.week, "--days:5;--first:16;--span:16;--spill:1", "the module's frame, one line under a day");
        assert_eq!(cut.marks[2], Marks { classes: vec!["cutb"], until: Some(1020), ..Marks::default() });
        assert_eq!(cut.marks[3], Marks { classes: vec!["cutt"], since: Some(450), ..Marks::default() });
        assert!(cut.marks[4].after && !cut.marks[5].after && !cut.marks[6].after, "{:?}", cut.marks);
        assert!(cut.slots[2].starts_with("--from:31;--to:32;") && cut.slots[3].starts_with("--from:16;--to:18;"), "{:?}", cut.slots);
        // What lies after the frame is on no day: it takes no lane, and no slot stands for it.
        assert_eq!(cut.slots[4], "--from:32;--to:32;--lane:0;--lanes:1");
    }

    #[test]
    fn a_calendar_as_a_strip_leaves_the_plans_lanes_as_they_are() {
        let strip = |day: u8, from: u16, to: u16| GridSlot { class: "strip", hue: Some("k-1"), ..slot(day, from, to) };
        let full = |day: u8, from: u16, to: u16| GridSlot { class: "cal", hue: Some("k-1"), badge: Some("L".into()), ..slot(day, from, to) };
        // Tuesday: one own slot and two strips that overlap it and each other; Thursday a strip alone.
        let slots = [slot(2, 555, 645), strip(2, 555, 645), strip(2, 600, 690), strip(4, 480, 570)];
        let (week, styles) = geometry(&slots, MIN_HOURS).unwrap();
        assert!(week.ends_with(";--strip:1"), "{week}");
        assert!(styles[0].ends_with("--lane:0;--lanes:1"), "the own slot keeps its lane: {}", styles[0]);
        assert!(styles[1].ends_with("--lane:0;--lanes:2") && styles[2].ends_with("--lane:1;--lanes:2"), "{styles:?}");
        assert!(styles[3].ends_with("--lane:0;--lanes:1"));
        // In full, a calendar's slot takes its lane beside the plan's, with its initial.
        let (week, styles) = geometry(&[slot(2, 555, 645), full(2, 555, 645)], MIN_HOURS).unwrap();
        assert!(!week.contains("--strip") && styles[1].ends_with("--lane:1;--lanes:2"), "{week} {styles:?}");
        assert_eq!(full(2, 555, 645).classes(), "slot cal k-1");
        // A fitted grid gives a day no more room for its strips.
        let fitted = layout(&slots, MIN_HOURS, true).unwrap();
        assert!(fitted.week.contains("--cols:minmax(0,0.6fr) minmax(0,1fr) minmax(0,0.6fr) minmax(0,0.6fr) minmax(0,0.6fr)"), "{}", fitted.week);
    }

    #[test]
    fn a_fitted_grid_gives_room_where_modules_meet() {
        // Monday one lane, Tuesday three side by side, Wednesday nothing, Thursday two, Friday one.
        let slots = [slot(1, 480, 570), slot(2, 480, 570), slot(2, 480, 570), slot(2, 500, 600), slot(4, 600, 690), slot(4, 630, 720), slot(5, 480, 570)];
        let widths = "minmax(0,1fr) minmax(0,2.2fr) minmax(0,0.6fr) minmax(0,1.6fr) minmax(0,1fr)";
        assert_eq!(columns(&slots, 5), widths);
        // Its frame reaches 18:00 however early the last slot ends.
        let week = layout(&slots, MIN_HOURS, true).map(|layout| layout.week);
        assert_eq!(week, Some(format!("--days:5;--first:16;--span:20;--cols:{widths}")));
        // A module's grid keeps its equal days; a day of many lanes takes at most its share.
        assert!(!geometry(&slots, MIN_HOURS).is_some_and(|(week, _)| week.contains("--cols")));
        assert_eq!((day_share(0), day_share(1), day_share(9)), (EMPTY_DAY, 1.0, WIDEST_DAY));
        // A slot on no weekday widens nothing.
        assert_eq!(columns(&[slot(0, 480, 570), slot(1, 480, 570)], 5), "minmax(0,1fr) minmax(0,0.6fr) minmax(0,0.6fr) minmax(0,0.6fr) minmax(0,0.6fr)");
    }

    #[test]
    fn slots_that_stand_together_share_a_frame() {
        let joint = |day: u8, id: u32| GridSlot { joint: Some(id), clash: true, ..slot(day, 450, 540) };
        // Tuesday: two of one joint side by side and another slot beside them; Thursday a joint alone.
        let together = layout(&[joint(2, 1), joint(2, 1), slot(2, 450, 540), joint(4, 2)], MIN_HOURS, true).unwrap();
        assert_eq!(together.frames, [(2, "--from:15;--to:18;--lane:0;--lanes:3;--wide:2".to_string())]);
        assert_eq!(together.framed, [true, true, false, false]);
        // Another slot between their lanes: no frame, each keeps its own.
        let apart = layout(&[joint(2, 1), slot(2, 450, 540), joint(2, 1)], MIN_HOURS, true).unwrap();
        assert!(apart.frames.is_empty() && apart.framed == [false, false, false]);
    }

    #[test]
    fn no_slot_no_grid() {
        assert_eq!(geometry(&[], MIN_HOURS), None);
        // A slot on no weekday is not drawn, and alone it makes no grid either.
        assert_eq!(geometry(&[slot(0, 555, 645), slot(8, 555, 645)], MIN_HOURS), None);
        // A fitted grid is the frame of an empty week: Monday to Friday, 08:00 to 18:00, the days
        // alike.
        let empty = layout(&[], MIN_HOURS, true).map(|layout| (layout.week, layout.slots));
        let days = ["minmax(0,0.6fr)"; 5].join(" ");
        assert_eq!(empty, Some((format!("--days:5;--first:16;--span:20;--cols:{days}"), Vec::new())));
        // A late slot takes the frame past 18:00, an early one before 08:00.
        let late = layout(&[slot(3, 1035, 1125), slot(1, 450, 540)], MIN_HOURS, true).map(|layout| layout.week);
        assert!(late.is_some_and(|week| week.starts_with("--days:5;--first:14;--span:24;")));
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
        assert_eq!(GridSlot { class: "other", mark: Some("course-2"), ..slot(1, 555, 645) }.classes(), "slot other course-2");
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
