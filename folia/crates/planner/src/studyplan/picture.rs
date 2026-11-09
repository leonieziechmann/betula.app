//! „Als Bild speichern" in „Export" (owner, 2026-11-04: „In deinen Kalender soll zu export werden
//! und da soll auch noch der knopf als bild speichern rein"): the week as a picture, a PNG made in
//! the browser and downloaded, so nothing of the plan reaches the server.
//!
//! The picture is the week the grid shows (`week::picture_slots`: the Regelwoche of the plan, in
//! the A-week, the B-week or both as the head's switch has it), without what belongs to the page
//! (links, buttons, the calendars of one's own beside it, „Alle Termine"'s faint slots). It is laid
//! out here as plain values (`Picture`, a list of what to draw, in CSS pixels), which the tests
//! read, and drawn on a canvas at twice that size (`draw`), in the app's typeface, whose widths the
//! canvas measures. It keeps to the week's geometry: a day as wide as its lanes need, slots that
//! overlap side by side (`folia_timetable::grid`), 08:00 to 18:00 at least. Its colours are the
//! light theme's whatever the page shows (a picture is shared and printed): a module's card full in
//! its tone with white words, single dates lighter in it, an option to choose hatched and ringed in its
//! tone, an extra ringed with dashes, a module only visited ringed, a clash ringed red with its
//! „!".

use folia_calendar::day::clock;
use folia_timetable::grid::{self, Placed};
use folia_widgets::week::GridSlot;

use crate::draw::{self, lines_of, oklch, Align, Color, Font, Op, Picture, LINE, LINE_STRONG, NO, PANEL, PANEL_2, TEXT, TEXT_2, TEXT_3, WARN};

/// The picture's width in CSS pixels.
const WIDTH: f64 = 1200.0;
/// The margin round the picture, the title's block over the week, the row of the days' names,
/// an hour of the week, the column of the hours and the gap between two days.
const PAD: f64 = 36.0;
const HEAD: f64 = 70.0;
const DAY_HEAD: f64 = 30.0;
const HOUR: f64 = 56.0;
const HOURS: f64 = 46.0;
const GAP: f64 = 6.0;
/// The padding inside a slot, and the room each of its lines takes.
const INSET: f64 = 6.0;
const LABEL_LINE: f64 = 15.5;
const SMALL_LINE: f64 = 14.0;
/// The share of the week an empty day takes, what each lane after a day's first adds, and the
/// most a day takes: as the page's grid (`folia_widgets::week`).
const EMPTY_DAY: f64 = 0.6;
const MORE_LANE: f64 = 0.6;
const WIDEST_DAY: f64 = 3.4;
/// The fewest hours, and the hour the week shows the day until at least: as the page's grid.
const MIN_MINUTES: u16 = 4 * 60;
const UNTIL: u16 = 18 * 60;
// `lay_out` clamps the day's end between UNTIL and midnight: the bounds must stay in order.
const _: () = assert!(UNTIL <= 24 * 60);

/// A module's tone by its class (`head::hue`), as `app.css` defines `--t-…`: lightness, chroma,
/// hue. Betula's green without one.
fn tone(hue: Option<&str>) -> (f64, f64, f64) {
    match hue.unwrap_or_default() {
        "t-ice" => (0.55, 0.09, 240.0),
        "t-sun" => (0.64, 0.12, 75.0),
        "t-violet" => (0.53, 0.1, 300.0),
        "t-teal" => (0.56, 0.08, 190.0),
        "t-green" => (0.56, 0.1, 150.0),
        "t-coral" => (0.6, 0.12, 35.0),
        "t-rose" => (0.56, 0.1, 355.0),
        "t-slate" => (0.55, 0.04, 260.0),
        "t-own" => (0.5, 0.006, 250.0),
        _ => (0.53, 0.07, 149.0),
    }
}

/// A slot's colours, as the page's grid draws it: its ground, the stripes over it, its ring (its
/// colour, width, dashed), its words and the plate under them on stripes, and whether it is a card
/// with a shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    fill: Color,
    hatch: Option<Color>,
    ring: Option<(Color, f64, bool)>,
    ink: Color,
    plate: Option<Color>,
    card: bool,
}

fn look(slot: &GridSlot) -> Look {
    let (l, c, h) = tone(slot.hue);
    let (mode, card, ink) = (oklch(l, c, h), oklch(0.5, c, h), oklch(0.42, c, h));
    let tint = mode.over(PANEL, 0.09);
    let mut look = if slot.extra {
        Look { fill: PANEL, hatch: None, ring: Some((mode.alpha(0.55), 1.5, true)), ink: TEXT_2, plate: None, card: true }
    } else if slot.visit {
        let hatch = slot.alt.then_some(mode.alpha(0.34));
        Look { fill: PANEL, hatch, ring: Some((mode, 1.5, false)), ink: TEXT, plate: hatch.map(|_| PANEL), card: true }
    } else if slot.alt {
        Look { fill: tint, hatch: Some(mode.alpha(0.34)), ring: Some((mode, 1.5, false)), ink, plate: Some(tint), card: true }
    } else if slot.class == "once" && slot.hue.is_some() {
        // Single dates of a module: its tone, lighter, with dark words (`--mod-once`).
        Look { fill: mode.over(PANEL, 0.4), hatch: None, ring: None, ink: TEXT, plate: None, card: true }
    } else if slot.class == "once" {
        Look { fill: PANEL_2, hatch: None, ring: Some((LINE_STRONG, 1.0, false)), ink: TEXT_2, plate: None, card: false }
    } else {
        Look { fill: card, hatch: None, ring: None, ink: PANEL, plate: None, card: true }
    };
    if slot.clash {
        look.ring = Some((NO, 2.0, false));
    } else if slot.gives || slot.conflict {
        look.ring = Some((WARN, 2.0, false));
    }
    look
}

const TITLE: Font = Font { size: 24.0, weight: 700 };
const SUBTITLE: Font = Font { size: 14.0, weight: 500 };
const SITE: Font = Font { size: 13.0, weight: 600 };
const DAY: Font = Font { size: 12.5, weight: 650 };
const HOUR_FONT: Font = Font { size: 11.0, weight: 500 };
const LABEL: Font = Font { size: 12.5, weight: 650 };
const SMALL: Font = Font { size: 11.0, weight: 500 };
const BADGE: Font = Font { size: 10.0, weight: 800 };

/// The words over the week: „Stundenplan", „WiSe 2026/27 · A-Woche", the site, and the days'
/// names, Monday first.
pub(super) struct Heading {
    pub title: String,
    pub subtitle: String,
    pub site: String,
    pub days: [String; 7],
}

/// The share of the week a day of `lanes` lanes takes (none: an empty day).
fn day_share(lanes: u8) -> f64 {
    match lanes.checked_sub(1) {
        None => EMPTY_DAY,
        Some(more) => (1.0 + MORE_LANE * f64::from(more)).min(WIDEST_DAY),
    }
}

/// The picture of the week of `slots` (the page's grid's slots: day, minutes, label, look) under
/// `heading`, its texts measured by `measure` (the canvas's widths in the browser).
pub(super) fn lay_out(slots: &[GridSlot], heading: &Heading, measure: &dyn Fn(&str, Font) -> f64) -> Picture {
    let drawn: Vec<&GridSlot> = slots.iter().filter(|slot| (1..=7).contains(&slot.day) && slot.to > slot.from).collect();
    let placed: Vec<Placed> = drawn.iter().map(|slot| slot.placed()).collect();
    let mut span = grid::span(&placed, MIN_MINUTES);
    span.last = span.last.clamp(UNTIL, 24 * 60);
    let lanes = grid::lanes(&placed);
    let days = usize::from(span.days);
    let mut most = [0u8; 7];
    for (slot, (_, of)) in drawn.iter().zip(&lanes) {
        if let Some(day) = usize::from(slot.day).checked_sub(1).and_then(|day| most.get_mut(day)) {
            *day = (*day).max(*of);
        }
    }
    let shares: Vec<f64> = most.iter().take(days).map(|lanes| day_share(*lanes)).collect();
    let total: f64 = shares.iter().sum();
    let left = PAD + HOURS;
    let room = WIDTH - PAD - left - GAP * (shares.len().saturating_sub(1) as f64);
    let mut columns: Vec<(f64, f64)> = Vec::new();
    let mut x = left;
    for share in &shares {
        let width = room * share / total;
        columns.push((x, width));
        x += width + GAP;
    }
    let top = PAD + HEAD + DAY_HEAD;
    let hours = f64::from(span.last.saturating_sub(span.first)) / 60.0;
    let bottom = top + hours * HOUR;
    let y_of = |minutes: u16| top + f64::from(minutes.saturating_sub(span.first)) / 60.0 * HOUR;

    let mut ops = vec![Op::Fill { x: 0.0, y: 0.0, w: WIDTH, h: bottom + PAD, radius: 0.0, color: PANEL, shadow: false }];
    // The title, what it is of, and the site.
    ops.push(Op::Text { x: PAD, y: PAD + 24.0, text: heading.title.clone(), font: TITLE, color: TEXT, align: Align::Left });
    ops.push(Op::Text { x: PAD, y: PAD + 48.0, text: heading.subtitle.clone(), font: SUBTITLE, color: TEXT_2, align: Align::Left });
    ops.push(Op::Text { x: WIDTH - PAD, y: PAD + 24.0, text: heading.site.clone(), font: SITE, color: TEXT_3, align: Align::Right });
    // The week's ground, the days' names over it, an hour's line each with its time.
    ops.push(Op::Fill { x: left - 8.0, y: top - 8.0, w: WIDTH - PAD - left + 16.0, h: bottom - top + 16.0, radius: 9.0, color: PANEL_2, shadow: false });
    ops.push(Op::Ring { x: left - 8.0, y: top - 8.0, w: WIDTH - PAD - left + 16.0, h: bottom - top + 16.0, radius: 9.0, color: LINE, width: 1.0, dashed: false });
    for (i, (x, width)) in columns.iter().enumerate() {
        let name = heading.days.get(i).cloned().unwrap_or_default();
        let name = lines_of(&name, *width, 1, DAY, measure).into_iter().next().unwrap_or_default();
        ops.push(Op::Text { x: x + width / 2.0, y: top - 16.0, text: name, font: DAY, color: TEXT_3, align: Align::Center });
    }
    let mut hour = span.first;
    while hour <= span.last {
        let y = y_of(hour);
        ops.push(Op::Line { x1: left, y1: y, x2: WIDTH - PAD, y2: y, color: LINE });
        ops.push(Op::Text { x: left - 14.0, y: y + 4.0, text: clock(hour), font: HOUR_FONT, color: TEXT_3, align: Align::Right });
        hour += 60;
    }
    // The slots, each in its lane of its day.
    for (slot, (lane, of)) in drawn.iter().zip(&lanes) {
        let Some((x, width)) = columns.get(usize::from(slot.day) - 1).copied() else { continue };
        let lane_width = width / f64::from((*of).max(1));
        let (sx, sw) = (x + lane_width * f64::from(*lane) + 1.0, lane_width - 2.0);
        let (sy, sh) = (y_of(slot.from) + 1.0, y_of(slot.to.min(span.last)) - y_of(slot.from) - 2.0);
        if sw <= 0.0 || sh <= 0.0 {
            continue;
        }
        let look = look(slot);
        let radius = 6.0_f64.min(sh / 2.0);
        ops.push(Op::Fill { x: sx, y: sy, w: sw, h: sh, radius, color: look.fill, shadow: look.card });
        if let Some(color) = look.hatch {
            ops.push(Op::Hatch { x: sx, y: sy, w: sw, h: sh, radius, color });
        }
        if let Some((color, ring, dashed)) = look.ring {
            ops.push(Op::Ring { x: sx, y: sy, w: sw, h: sh, radius, color, width: ring, dashed });
        }
        // Its lines: the label (two lines where the slot is an hour or longer and has the room),
        // the time, where it is held; each only where it fits.
        let inner = sw - 2.0 * INSET - if slot.clash { 14.0 } else { 0.0 };
        let mut room = sh - 2.0 * INSET;
        let mut y = sy + INSET;
        let mut texts: Vec<(String, Font)> = Vec::new();
        let most = if slot.to.saturating_sub(slot.from) < 60 { 1 } else { 2 };
        let label_lines = most.min((room / LABEL_LINE).floor().max(1.0) as usize);
        for line in lines_of(&slot.label, inner.max(1.0), label_lines, LABEL, measure) {
            texts.push((line, LABEL));
        }
        room -= LABEL_LINE * texts.len() as f64;
        for small in [&slot.small, &slot.place].into_iter().filter(|small| !small.is_empty()) {
            if room < SMALL_LINE {
                break;
            }
            if let Some(line) = lines_of(small, inner.max(1.0), 1, SMALL, measure).into_iter().next() {
                texts.push((line, SMALL));
                room -= SMALL_LINE;
            }
        }
        for (text, font) in texts {
            let height = if font == LABEL { LABEL_LINE } else { SMALL_LINE };
            if let Some(plate) = look.plate {
                let w = measure(&text, font) + 4.0;
                ops.push(Op::Fill { x: sx + INSET - 2.0, y: y + 1.0, w, h: height - 1.0, radius: 3.0, color: plate, shadow: false });
            }
            let color = if font == SMALL && look.plate.is_some() { TEXT_2 } else { look.ink };
            ops.push(Op::Text { x: sx + INSET, y: y + height - 4.0, text, font, color, align: Align::Left });
            y += height;
        }
        // A clash carries its „!" in the top right corner.
        if slot.clash && sw > 20.0 && sh > 18.0 {
            ops.push(Op::Fill { x: sx + sw - 16.0, y: sy + 3.0, w: 13.0, h: 13.0, radius: 6.5, color: NO, shadow: false });
            ops.push(Op::Text { x: sx + sw - 9.5, y: sy + 13.0, text: "!".into(), font: BADGE, color: PANEL, align: Align::Center });
        }
    }
    Picture { width: WIDTH, height: bottom + PAD, ops }
}

/// Draws the picture of `slots` on a canvas and downloads it as `name` (a PNG, `draw::save`).
/// Only in the browser.
pub(super) fn save(slots: &[GridSlot], heading: &Heading, name: &str) {
    draw::save(name, |measure| lay_out(slots, heading, measure));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A width of 0.6 of the size per character, as Inter's letters are on average.
    fn measure(text: &str, font: Font) -> f64 {
        text.chars().count() as f64 * font.size * 0.6
    }

    fn slot(day: u8, from: &str, to: &str, label: &str) -> GridSlot {
        let minutes = |at: &str| {
            let (h, m) = at.split_once(':').unwrap();
            h.parse::<u16>().unwrap() * 60 + m.parse::<u16>().unwrap()
        };
        GridSlot { day, from: minutes(from), to: minutes(to), label: label.into(), small: format!("{from}–{to}"), place: "ZHG/HS.C".into(), class: "tinted", hue: Some("t-ice"), ..Default::default() }
    }

    fn heading() -> Heading {
        Heading {
            title: "Stundenplan".into(),
            subtitle: "WiSe 2026/27".into(),
            site: "betula.app".into(),
            days: ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"].map(String::from),
        }
    }

    fn texts(picture: &Picture) -> Vec<String> {
        picture.ops.iter().filter_map(|op| if let Op::Text { text, .. } = op { Some(text.clone()) } else { None }).collect()
    }

    /// The fills of the slots: every fill after the week's ground (the page, the ground).
    fn cards(picture: &Picture) -> Vec<(f64, f64, f64, f64)> {
        picture.ops.iter().filter_map(|op| if let Op::Fill { x, y, w, h, shadow: true, .. } = op { Some((*x, *y, *w, *h)) } else { None }).collect()
    }

    #[test]
    fn the_tones_are_the_stylesheets() {
        // oklch(.53 .07 149), Betula's green; white and black stay.
        assert_eq!(oklch(0.53, 0.07, 149.0), Color::rgb(0x4e, 0x77, 0x55));
        assert_eq!(oklch(1.0, 0.0, 0.0), Color::rgb(255, 255, 255));
        assert_eq!(oklch(0.0, 0.0, 0.0), Color::rgb(0, 0, 0));
        assert_eq!(Color::rgb(16, 21, 31).alpha(0.5).css(), "rgba(16,21,31,0.5)");
    }

    #[test]
    fn the_week_is_monday_to_friday_from_eight_to_six_at_least() {
        let picture = lay_out(&[slot(2, "09:15", "10:45", "VL EvS")], &heading(), &measure);
        let said = texts(&picture);
        for word in ["Stundenplan", "WiSe 2026/27", "betula.app", "Montag", "Freitag", "08:00", "18:00", "VL EvS", "09:15–10:45", "ZHG/HS.C"] {
            assert!(said.iter().any(|text| text == word), "{word}: {said:?}");
        }
        assert!(!said.iter().any(|text| text == "Samstag" || text == "07:00" || text == "19:00"), "{said:?}");
        // 08:00 to 18:00: ten hours under the title and the days' names.
        assert_eq!(picture.height, PAD + HEAD + DAY_HEAD + 10.0 * HOUR + PAD);
        // An empty picture is the empty week.
        let empty = lay_out(&[], &heading(), &measure);
        assert!(cards(&empty).is_empty() && texts(&empty).iter().any(|text| text == "Freitag"));
    }

    #[test]
    fn slots_that_overlap_stand_side_by_side_and_their_day_is_wider() {
        let slots = [slot(1, "08:00", "10:00", "VL Mathe"), slot(1, "09:00", "11:00", "Ü Mathe"), slot(2, "08:00", "10:00", "VL EvS")];
        let picture = lay_out(&slots, &heading(), &measure);
        let cards = cards(&picture);
        assert_eq!(cards.len(), 3);
        let (monday, tuesday) = ((cards[0], cards[1]), cards[2]);
        // Side by side on Monday, each half the day; Monday's two lanes take 1.6 of Tuesday's one.
        assert!(monday.0.0 + monday.0.2 <= monday.1.0, "{monday:?}");
        assert!((monday.0.2 - monday.1.2).abs() < 0.01);
        assert!(((monday.0.2 + 2.0) * 2.0 / (tuesday.2 + 2.0) - 1.6).abs() < 0.01, "{cards:?}");
        // Two hours tall, less the gap.
        assert!((tuesday.3 - (2.0 * HOUR - 2.0)).abs() < 0.01);
    }

    #[test]
    fn a_long_label_breaks_between_words_and_ends_in_an_ellipsis() {
        let font = LABEL;
        let width = 12.0 * font.size * 0.6;
        assert_eq!(lines_of("Entwicklung von Softwaresystemen", width, 2, font, &measure), vec!["Entwicklung", "von Softwar…"]);
        assert_eq!(lines_of("VL EvS", width, 2, font, &measure), vec!["VL EvS"]);
        // A word too wide is cut and ends the label.
        assert_eq!(lines_of("Softwaresystementwicklung II", width, 2, font, &measure), vec!["Softwaresys…"]);
        assert_eq!(lines_of("", width, 2, font, &measure), Vec::<String>::new());
    }

    #[test]
    fn a_slot_says_what_fits_and_a_clash_carries_its_mark() {
        // Half an hour: the label alone, on one line.
        let short = lay_out(&[slot(3, "10:00", "10:30", "Tutorium Mathematik")], &heading(), &measure);
        let said = texts(&short);
        assert!(!said.iter().any(|text| text == "10:00–10:30" || text == "ZHG/HS.C"), "{said:?}");
        let mut clash = slot(4, "10:00", "12:00", "VL EvS");
        clash.clash = true;
        let marked = lay_out(&[clash], &heading(), &measure);
        assert!(texts(&marked).iter().any(|text| text == "!"));
        assert!(marked.ops.iter().any(|op| matches!(op, Op::Ring { color, .. } if *color == NO)));
    }

    #[test]
    fn each_look_has_its_colours() {
        let mut once = slot(1, "08:00", "10:00", "x");
        once.class = "once";
        assert_eq!((look(&once).fill, look(&once).ink), (oklch(0.55, 0.09, 240.0).over(PANEL, 0.4), TEXT));
        once.hue = None;
        assert_eq!(look(&once).fill, PANEL_2);
        let mut alt = slot(1, "08:00", "10:00", "x");
        alt.alt = true;
        let alt = look(&alt);
        assert!(alt.hatch.is_some() && alt.plate == Some(alt.fill) && alt.ring.is_some());
        let card = look(&slot(1, "08:00", "10:00", "x"));
        assert_eq!((card.fill, card.ink, card.ring), (oklch(0.5, 0.09, 240.0), PANEL, None));
        let mut extra = slot(1, "08:00", "10:00", "x");
        extra.extra = true;
        assert!(matches!(look(&extra).ring, Some((_, _, true))));
    }
}
