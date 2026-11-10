//! „Als Bild speichern" of the Gesamtplan (owner, 2026-11-09: „ich möchte einen export button
//! haben für den gesamtplan, wo er als bild exportiert wird"): the plan as it is shown — its areas,
//! what of them is unfolded (subsections, lanes), „Bisher" folded or not — as a PNG made in the
//! browser and downloaded (`crate::draw`), so nothing of the plan reaches the server. Every
//! semester stands in it, however far the page is scrolled sideways.
//!
//! It is laid out here from the grid's model (`model::Layout`) as plain values, which the tests
//! read: the title, what it is of and the site; the band of the study years, the semesters' heads,
//! a row per area with its name, its figure and its bar, under an unfolded area its subsections
//! or its lanes, the line of today with its flag, and the legend. A cell's pieces stand side by
//! side as on the page, each as wide as its credits and never narrower than its label; where they
//! do not fit, the cell says „4 Module · 24 LP" as the page does. Its colours are the light
//! theme's, the areas' tones as the stylesheet defines them.

use std::collections::BTreeSet;

use folia_plans::choices::{Room, Stand};

use super::model::{AreaRow, Band, Cell, Column, Fill, Lane, Layout, Piece, SubRow};
use crate::draw::{self, oklch, Align, Color, Font, Op, Picture, LINE, LINE_STRONG, NO, OK, PANEL, PANEL_2, TEXT, TEXT_2, TEXT_3, WARN};
use crate::i18n::Texts;

/// The margin round the picture, the title's block, the column of the names, the narrowest a
/// semester's column is, „Bisher"'s column.
const PAD: f64 = 32.0;
const TITLE_BLOCK: f64 = 74.0;
const NAMES: f64 = 214.0;
const SEMESTER: f64 = 150.0;
const SO_FAR: f64 = 100.0;
/// The rows' heights: the band of the study years, the semesters' heads, an area, a subsection, a
/// lane (the last of a row the more), the line of today's flag, the legend.
const BAND: f64 = 22.0;
const HEADS: f64 = 54.0;
const AREA: f64 = 46.0;
const SUB: f64 = 42.0;
const LANE: f64 = 30.0;
const LANE_LAST: f64 = 6.0;
const FLAG: f64 = 28.0;
const LEGEND: f64 = 30.0;
/// A piece's height in an area's and a subsection's row, in a lane; the room inside it beside
/// its label, an icon's room, the gap between two pieces.
const PIECE: f64 = 26.0;
const LANE_PIECE: f64 = 22.0;
const INSET: f64 = 6.0;
const ICON: f64 = 12.0;
const GAP: f64 = 2.0;

const TITLE: Font = Font { size: 24.0, weight: 700 };
const SUBTITLE: Font = Font { size: 14.0, weight: 500 };
const SITE: Font = Font { size: 13.0, weight: 600 };
const BAND_FONT: Font = Font { size: 10.0, weight: 700 };
const HEAD_NAME: Font = Font { size: 12.5, weight: 650 };
const HEAD_SUB: Font = Font { size: 11.0, weight: 500 };
const HEAD_FIGURE: Font = Font { size: 11.0, weight: 650 };
const AREA_NAME: Font = Font { size: 13.0, weight: 650 };
const SUB_NAME: Font = Font { size: 12.0, weight: 600 };
const FIGURE: Font = Font { size: 12.0, weight: 600 };
const LANE_NAME: Font = Font { size: 11.5, weight: 500 };
const PIECE_FONT: Font = Font { size: 11.0, weight: 650 };
const FLAG_FONT: Font = Font { size: 10.0, weight: 700 };
const LEGEND_FONT: Font = Font { size: 11.5, weight: 500 };

/// Betula's green (`--accent`).
fn accent() -> Color {
    oklch(0.53, 0.07, 149.0)
}

/// The ground of a subsection and of a lane: the text's colour, a breath of it over the panel.
fn lane_ground() -> Color {
    TEXT.over(PANEL, 0.016)
}

/// An area's tone as the stylesheet defines it (`var(--fac-1)` …), in the light theme.
fn tone(tone: &str) -> Color {
    let (l, c, h) = match tone.trim_start_matches("var(").trim_end_matches(')') {
        "--fac-1" => (0.5, 0.12, 262.0),
        "--fac-2" => (0.52, 0.09, 158.0),
        "--fac-3" => (0.56, 0.11, 62.0),
        "--fac-4" => (0.5, 0.11, 330.0),
        "--fac-5" => (0.52, 0.08, 212.0),
        "--fac-6" => (0.52, 0.12, 30.0),
        "--t-ice" => (0.55, 0.09, 240.0),
        "--t-sun" => (0.64, 0.12, 75.0),
        "--t-violet" => (0.53, 0.1, 300.0),
        "--t-teal" => (0.56, 0.08, 190.0),
        "--t-green" => (0.56, 0.1, 150.0),
        "--t-coral" => (0.6, 0.12, 35.0),
        "--t-rose" => (0.56, 0.1, 355.0),
        "--t-slate" => (0.55, 0.04, 260.0),
        _ => return TEXT_3,
    };
    oklch(l, c, h)
}

/// What a swatch of the legend shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Swatch {
    Fill(Fill),
    Over,
    Failed,
    Retake,
    Unoffered,
    Today,
}

/// The words over the plan: „Gesamtplan", „Informatik B.Sc. · Stand 09.11.2026", the site, and
/// the legend under it.
pub(super) struct Heading {
    pub title: String,
    pub subtitle: String,
    pub site: String,
    pub legend: Vec<(Swatch, String)>,
}

/// The legend as the page has it under the plan.
pub(super) fn legend(t: &Texts) -> Vec<(Swatch, String)> {
    let s = &t.study;
    vec![
        (Swatch::Fill(Fill::Passed), s.legend_passed.to_string()),
        (Swatch::Fill(Fill::Running), s.legend_running.to_string()),
        (Swatch::Fill(Fill::Planned), s.legend_planned.to_string()),
        (Swatch::Fill(Fill::Open), s.legend_plan_open.to_string()),
        (Swatch::Failed, s.legend_failed.to_string()),
        (Swatch::Retake, s.legend_retake.to_string()),
        (Swatch::Unoffered, s.legend_offer.to_string()),
        (Swatch::Over, s.legend_over.to_string()),
        (Swatch::Today, s.legend_today.to_string()),
    ]
}

/// A piece's colours: its ground, its ring (colour, width, dashed), stripes, its words.
struct Look {
    fill: Color,
    ring: Option<(Color, f64, bool)>,
    hatch: Option<Color>,
    ink: Color,
}

fn look(fill: Fill, over: bool, c: Color, sun: bool) -> Look {
    let mix = |share: f64| c.over(PANEL, share);
    let mut look = match fill {
        Fill::Passed => Look { fill: c, ring: None, hatch: None, ink: if sun { TEXT } else { PANEL } },
        Fill::Running => Look { fill: mix(0.36), ring: None, hatch: None, ink: TEXT },
        Fill::Planned => Look { fill: mix(0.14), ring: Some((mix(0.42), 1.0, false)), hatch: None, ink: TEXT },
        Fill::Failed => Look { fill: NO.over(PANEL, 0.07), ring: Some((NO.over(PANEL, 0.6), 1.0, false)), hatch: None, ink: NO },
        Fill::Row => Look { fill: mix(0.14), ring: Some((mix(0.7), 1.5, true)), hatch: None, ink: TEXT },
        Fill::Open => Look { fill: PANEL, ring: Some((mix(0.62), 1.5, true)), hatch: None, ink: TEXT_2 },
    };
    if over {
        look = Look { fill: mix(0.09), ring: Some((mix(0.42), 1.0, false)), hatch: Some(mix(0.34)), ink: TEXT };
    }
    look
}

/// What a piece says before its label and after it: ✕, + or ? before, ↻ and ⚠ after.
fn marks(piece: &Piece) -> (Option<&'static str>, Vec<&'static str>) {
    let pre = match piece.fill {
        Fill::Failed => Some("✕"),
        Fill::Open if piece.retake => Some("↻"),
        // „+10" says its plus itself.
        Fill::Open if piece.label.starts_with('+') => None,
        Fill::Open => Some("+"),
        Fill::Row => Some("?"),
        _ => None,
    };
    let mut post = Vec::new();
    if piece.retake && piece.fill != Fill::Open {
        post.push("↻");
    }
    if piece.unoffered {
        post.push("⚠");
    }
    (pre, post)
}

/// What the picture is drawn with: where the columns stand, where the line of today runs, how
/// texts are measured, what is drawn.
struct Sheet<'a> {
    columns: Vec<(f64, f64)>,
    kinds: Vec<&'static str>,
    today: Option<f64>,
    measure: &'a dyn Fn(&str, Font) -> f64,
    ops: Vec<Op>,
    t: &'a Texts,
}

impl Sheet<'_> {
    fn text(&mut self, x: f64, y: f64, text: impl Into<String>, font: Font, color: Color, align: Align) {
        self.ops.push(Op::Text { x, y, text: text.into(), font, color, align });
    }

    fn fill(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        self.ops.push(Op::Fill { x, y, w, h, radius, color, shadow: false });
    }

    /// The line of today through a row from `y` of height `h`: over its ground, behind its pieces,
    /// as the page has it.
    fn today(&mut self, y: f64, h: f64) {
        if let Some(x) = self.today {
            self.fill(x - 1.0, y, 2.0, h, 0.0, accent());
        }
    }

    /// The grounds of a row's cells: the semesters that are over grey, the current one green, one
    /// of leave hatched, a line under them.
    fn grounds(&mut self, y: f64, h: f64, tint: bool) {
        let left = PAD;
        let right = self.columns.last().map_or(left + NAMES, |(x, w)| x + w);
        if tint {
            self.fill(left, y, right - left, h, 0.0, lane_ground());
        }
        for (at, (x, w)) in self.columns.clone().into_iter().enumerate() {
            match self.kinds.get(at).copied().unwrap_or_default() {
                "past" => self.fill(x, y, w, h, 0.0, PANEL_2),
                "is-now" => self.fill(x, y, w, h, 0.0, accent().over(PANEL, 0.06)),
                "leave" => {
                    self.fill(x, y, w, h, 0.0, PANEL_2);
                    self.ops.push(Op::Hatch { x, y, w, h, radius: 0.0, color: LINE_STRONG });
                }
                _ => {}
            }
            self.ops.push(Op::Line { x1: x, y1: y, x2: x, y2: y + h, color: LINE });
        }
        self.ops.push(Op::Line { x1: left, y1: y + h - 1.0, x2: right, y2: y + h - 1.0, color: LINE });
        self.today(y, h);
    }

    /// A piece at `x`, `y` of width `w` and height `h`: its marks and label (its word after it in a
    /// lane, where it fits; a row of the plan in its lane says only what it is, „? Platzhalter").
    #[allow(clippy::too_many_arguments)]
    fn piece(&mut self, piece: &Piece, x: f64, y: f64, w: f64, h: f64, c: Color, sun: bool, word: Option<&str>) {
        let (pre, post) = marks(piece);
        let label = match word {
            Some(word) if piece.row.is_some() => word.to_string(),
            Some(word) => format!("{} {word}", piece.label),
            None => piece.label.clone(),
        };
        let mut text = pre.map(|pre| format!("{pre} ")).unwrap_or_default();
        text.push_str(&label);
        for mark in post {
            text.push(' ');
            text.push_str(mark);
        }
        self.boxed(piece.fill, piece.over, &text, x, y, w, h, c, sun);
    }

    /// A box of a bar: its ground, its ring or stripes as its fill has them, `text` in it.
    #[allow(clippy::too_many_arguments)]
    fn boxed(&mut self, fill: Fill, over: bool, text: &str, x: f64, y: f64, w: f64, h: f64, c: Color, sun: bool) {
        let look = look(fill, over, c, sun);
        let radius = 4.0;
        self.fill(x, y, w, h, radius, look.fill);
        if let Some(color) = look.hatch {
            self.ops.push(Op::Hatch { x, y, w, h, radius, color });
        }
        if let Some((color, width, dashed)) = look.ring {
            self.ops.push(Op::Ring { x, y, w, h, radius, color, width, dashed });
        }
        let room = (w - 2.0 * INSET).max(0.0);
        let shown = draw::one_line(text, room, PIECE_FONT, self.measure);
        self.text(x + INSET, y + h / 2.0 + 4.0, shown, PIECE_FONT, look.ink, Align::Left);
    }

    /// The pieces of a cell side by side, as wide as their credits and never narrower than their
    /// labels; where they do not fit, one piece saying how many they are and their credits, and what
    /// is open beside it — in a row that does not unfold (`still`) the pieces by their credits alone,
    /// their labels cut, as the page has them (`grid::cell_view`).
    #[allow(clippy::too_many_arguments)]
    fn cell(&mut self, pieces: &[Piece], x: f64, y: f64, w: f64, h: f64, c: Color, sun: bool, now: bool, past: bool, still: bool) {
        if pieces.is_empty() {
            return;
        }
        let x = x + 2.0;
        let w = w - 4.0;
        let least = |piece: &Piece| {
            let (pre, post) = marks(piece);
            (self.measure)(&piece.label, PIECE_FONT) + 2.0 * INSET + ICON * f64::from(u8::from(pre.is_some()) + u8::try_from(post.len()).unwrap_or(0))
        };
        let mut needs: Vec<f64> = pieces.iter().map(least).collect();
        let gaps = GAP * (pieces.len().saturating_sub(1)) as f64;
        let fits = needs.iter().sum::<f64>() + gaps <= w;
        if fits || still {
            // Flex as the page: each its least, the rest by the credits.
            if !fits {
                needs.fill(0.0);
            }
            let spare = (w - gaps - needs.iter().sum::<f64>()).max(0.0);
            let grow: f64 = pieces.iter().map(|piece| piece.grow).sum::<f64>().max(1e-6);
            let mut at = x;
            for (piece, need) in pieces.iter().zip(&needs) {
                let width = need + spare * piece.grow / grow;
                self.piece(piece, at, y, width, h, c, sun, None);
                at += width + GAP;
            }
            return;
        }
        let real: Vec<&Piece> = pieces.iter().filter(|piece| piece.fill != Fill::Open).collect();
        let open: Vec<&Piece> = pieces.iter().filter(|piece| piece.fill == Fill::Open).collect();
        let s = &self.t.study;
        let mut open_w = 0.0;
        if !open.is_empty() {
            let label = match open.as_slice() {
                [one] => format!("+{}", one.label.trim_start_matches('+')),
                many => format!("+ {}", (s.n_open)(many.len())),
            };
            open_w = ((self.measure)(&label, PIECE_FONT) + 2.0 * INSET).min(w / 2.0);
            self.boxed(Fill::Open, false, &label, x + w - open_w, y, open_w, h, c, sun);
        }
        if !real.is_empty() {
            let places = real.iter().filter(|piece| piece.fill == Fill::Row).count();
            let modules = real.len() - places;
            let count = match (modules, places) {
                (_, 0) => folia_design::format::modules(i64::try_from(modules).unwrap_or(0), self.t.locale),
                (0, _) => (s.n_placeholders)(places),
                _ => format!("{} · {}", folia_design::format::modules(i64::try_from(modules).unwrap_or(0), self.t.locale), (s.n_placeholders)(places)),
            };
            let credits: f64 = real.iter().map(|piece| piece.grow).sum();
            let fill = if real.iter().all(|piece| piece.fill == Fill::Passed) || past {
                Fill::Passed
            } else if real.iter().all(|piece| piece.fill == Fill::Row) {
                Fill::Row
            } else if now {
                Fill::Running
            } else {
                Fill::Planned
            };
            let width = (w - open_w - if open_w > 0.0 { GAP } else { 0.0 }).max(0.0);
            let full = format!("{count} · {}", (s.credits)(&super::n(credits, self.t)));
            let label = if (self.measure)(&full, PIECE_FONT) + 2.0 * INSET <= width { full } else { count };
            let failed = real.iter().filter(|piece| piece.fill == Fill::Failed).count();
            let label = if failed > 0 { format!("{label} · ✕ {failed}") } else { label };
            self.boxed(fill, false, &label, x, y, width, h, c, sun);
        }
    }

    /// The cells of a row's bar, its pieces in the middle of its height; `still` for a row that
    /// does not unfold.
    fn bars(&mut self, cells: &[Cell], y: f64, h: f64, c: Color, sun: bool, still: bool) {
        for (at, cell) in cells.iter().enumerate() {
            let Some((x, w)) = self.columns.get(at).copied() else { continue };
            match cell {
                Cell::Past { passed, any_passed, failed, .. } => {
                    if *any_passed || *failed > 0 {
                        let label = if *any_passed { format!("✓ {}", super::n(*passed, self.t)) } else { String::new() };
                        let label = if *failed > 0 { format!("{label} ✕ {failed}").trim().to_string() } else { label };
                        let fill = if *any_passed { Fill::Passed } else { Fill::Failed };
                        self.boxed(fill, false, &label, x + 3.0, y + (h - PIECE) / 2.0, w - 6.0, PIECE, c, sun);
                    }
                }
                Cell::Semester(cell) => {
                    let now = cell.when == folia_plans::study::When::Now;
                    let past = cell.when == folia_plans::study::When::Past;
                    self.cell(&cell.pieces, x, y + (h - PIECE) / 2.0, w, PIECE, c, sun, now, past, still);
                }
            }
        }
    }

    /// A lane: its name and credits, its piece in each semester with what it is.
    fn lane(&mut self, lane: &Lane, y: f64, last: bool, indent: f64, c: Color, sun: bool) -> f64 {
        let h = LANE + if last { LANE_LAST } else { 0.0 };
        self.fill(PAD, y, self.columns.last().map_or(NAMES, |(x, w)| x + w) - PAD, h, 0.0, lane_ground());
        for (at, (x, w)) in self.columns.clone().into_iter().enumerate() {
            match self.kinds.get(at).copied().unwrap_or_default() {
                "past" => self.fill(x, y, w, h, 0.0, PANEL_2),
                "is-now" => self.fill(x, y, w, h, 0.0, accent().over(PANEL, 0.06)),
                _ => {}
            }
            self.ops.push(Op::Line { x1: x, y1: y, x2: x, y2: y + h, color: LINE });
        }
        if last {
            let right = self.columns.last().map_or(PAD + NAMES, |(x, w)| x + w);
            self.ops.push(Op::Line { x1: PAD, y1: y + h - 1.0, x2: right, y2: y + h - 1.0, color: LINE });
        }
        self.today(y, h);
        let credits = lane.credits.clone().unwrap_or_default();
        let credits_w = (self.measure)(&credits, LANE_NAME);
        let name = draw::one_line(&lane.name, NAMES - indent - credits_w - 20.0, LANE_NAME, self.measure);
        self.text(PAD + indent, y + 19.0, name, LANE_NAME, TEXT_2, Align::Left);
        self.text(PAD + NAMES - 10.0, y + 19.0, credits, LANE_NAME, TEXT_2, Align::Right);
        for (at, bar) in lane.bars.iter().enumerate() {
            let Some((x, w)) = self.columns.get(at).copied() else { continue };
            let count = bar.len().max(1) as f64;
            let width = (w - 6.0 - GAP * (count - 1.0)) / count;
            for (i, (piece, word)) in bar.iter().enumerate() {
                let px = x + 3.0 + (width + GAP) * i as f64;
                let fits = (self.measure)(&format!("{} {word}", piece.label), PIECE_FONT) + 2.0 * INSET + 2.0 * ICON <= width;
                self.piece(piece, px, y + 4.0, width, LANE_PIECE, c, sun, fits.then_some(*word));
            }
        }
        h
    }

    /// The Wahlpflicht's bar of a subsection's stand: passed, planned, beyond need hatched, the
    /// least marked, a member's room above its most.
    fn stand(&mut self, stand: &Stand, room: Option<(Room, f64)>, x: f64, y: f64, w: f64, c: Color) {
        let (state, extra) = room.unwrap_or((Room::None, 0.0));
        let cap = if state == Room::Kept { stand.most - extra } else { stand.most };
        let scale = (if state == Room::None { stand.most } else { cap + extra }).max(stand.taken()).max(stand.least).max(1e-6);
        let at = |v: f64| x + (v / scale).clamp(0.0, 1.0) * w;
        self.fill(x, y, w, 4.0, 2.0, Color::rgb(0xec, 0xee, 0xf1));
        if matches!(state, Room::Open | Room::Over) {
            self.ops.push(Op::Hatch { x: at(cap), y, w: at(cap + extra) - at(cap), h: 4.0, radius: 0.0, color: LINE_STRONG });
        }
        self.fill(x, y, at(stand.passed) - x, 4.0, 2.0, c);
        self.fill(at(stand.passed), y, at(stand.taken()) - at(stand.passed), 4.0, 2.0, c.over(PANEL, 0.36));
        let limit = if state == Room::Over { cap } else { stand.most.max(stand.counted()) };
        if stand.taken() > limit + 1e-6 {
            let color = if state == Room::Over { WARN } else { c };
            self.fill(at(limit), y, at(stand.taken()) - at(limit), 4.0, 0.0, PANEL);
            self.ops.push(Op::Hatch { x: at(limit), y, w: at(stand.taken()) - at(limit), h: 4.0, radius: 0.0, color });
        }
        if stand.least > 0.0 && stand.least + 1e-6 < cap {
            self.ops.push(Op::Line { x1: at(stand.least), y1: y - 2.0, x2: at(stand.least), y2: y + 5.0, color: TEXT_2 });
        }
    }

    /// An area's head and bar, then what of it is unfolded; returns where the next row begins.
    fn area(&mut self, row: &AreaRow, open: &BTreeSet<String>, y: f64) -> f64 {
        let c = tone(row.tone);
        let sun = row.tone == "var(--t-sun)";
        self.grounds(y, AREA, false);
        // The name, its figure, the thin bar under them.
        self.fill(PAD + 10.0, y + 15.0, 8.0, 8.0, 4.0, c);
        let figure_w = (self.measure)(&row.figure, FIGURE) + if row.done { 14.0 } else { 0.0 };
        let name = draw::one_line(&row.name, NAMES - 34.0 - figure_w - 12.0, AREA_NAME, self.measure);
        self.text(PAD + 24.0, y + 23.0, name, AREA_NAME, TEXT, Align::Left);
        let figure_color = if row.done { OK } else if row.over { WARN } else { TEXT_2 };
        let figure = if row.done { format!("✓ {}", row.figure) } else { row.figure.clone() };
        self.text(PAD + NAMES - 10.0, y + 23.0, figure, FIGURE, figure_color, Align::Right);
        let (passed, planned, rest, over) = row.bar;
        let all = (passed + planned + rest + over).max(1e-6);
        let (bx, bw) = (PAD + 24.0, NAMES - 34.0);
        self.fill(bx, y + 31.0, bw, 3.0, 1.5, Color::rgb(0xec, 0xee, 0xf1));
        let mut at = bx;
        for (value, color) in [(passed, c), (planned, c.over(PANEL, 0.36)), (rest, Color::rgb(0xec, 0xee, 0xf1)), (over, c.over(PANEL, 0.5))] {
            let w = bw * value / all;
            if w > 0.0 {
                self.fill(at, y + 31.0, w, 3.0, 0.0, color);
            }
            at += w;
        }
        self.bars(&row.cells, y, AREA, c, sun, !row.unfolds());
        let mut y = y + AREA;
        // A row of one lane alone does not unfold (`AreaRow::unfolds`), whatever was kept open.
        if !row.unfolds() || !open.contains(&row.name) {
            return y;
        }
        if row.subs.is_empty() {
            for (i, lane) in row.lanes.iter().enumerate() {
                y += self.lane(lane, y, i + 1 == row.lanes.len(), 30.0, c, sun);
            }
        } else {
            for sub in &row.subs {
                y = self.sub(sub, open, y, c, sun);
            }
        }
        y
    }

    /// A subsection's head and bar, then its lanes where it is unfolded: its name over the whole
    /// width, its bar and its figure under it.
    fn sub(&mut self, sub: &SubRow, open: &BTreeSet<String>, y: f64, c: Color, sun: bool) -> f64 {
        self.grounds(y, SUB, true);
        let name = draw::one_line(&sub.name, NAMES - 34.0, SUB_NAME, self.measure);
        self.text(PAD + 24.0, y + 18.0, name, SUB_NAME, TEXT, Align::Left);
        let figure = if sub.done { format!("✓ {}", sub.figure) } else { sub.figure.clone() };
        let figure_w = (self.measure)(&figure, FIGURE);
        self.text(PAD + NAMES - 10.0, y + 33.0, figure, FIGURE, if sub.done { OK } else { TEXT_2 }, Align::Right);
        if let Some((stand, room)) = &sub.stand {
            self.stand(stand, *room, PAD + 24.0, y + 27.0, (NAMES - 44.0 - figure_w).max(20.0), c);
        }
        self.bars(&sub.cells, y, SUB, c, sun, !sub.unfolds());
        let mut y = y + SUB;
        if sub.unfolds() && open.contains(&sub.key) {
            for (i, lane) in sub.lanes.iter().enumerate() {
                y += self.lane(lane, y, i + 1 == sub.lanes.len(), 40.0, c, sun);
            }
        }
        y
    }
}

/// The picture of the plan `layout` with the rows of `open` unfolded, under `heading`, its texts
/// measured by `measure` (the canvas's widths in the browser).
pub(super) fn lay_out(layout: &Layout, open: &BTreeSet<String>, heading: &Heading, measure: &dyn Fn(&str, Font) -> f64, t: &Texts) -> Picture {
    // The columns: the names, „Bisher" where folded, the semesters.
    let mut columns = Vec::new();
    let mut kinds = Vec::new();
    let mut x = PAD + NAMES;
    for column in &layout.columns {
        let width = match column {
            Column::Past { .. } => SO_FAR,
            Column::Semester(_) => SEMESTER,
        };
        columns.push((x, width));
        kinds.push(match column {
            Column::Past { .. } => "past",
            Column::Semester(head) if head.leave => "leave",
            Column::Semester(head) if head.now => "is-now",
            Column::Semester(head) if head.past => "past",
            Column::Semester(_) => "",
        });
        x += width;
    }
    let right = x;
    let width = (right + PAD).max(720.0);
    let today = layout.today.and_then(|(at, gone)| columns.get(at).map(|(x, w)| x + w * gone));
    let mut sheet = Sheet { columns, kinds, today, measure, ops: Vec::new(), t };

    // The title, what it is of, and the site.
    sheet.text(PAD, PAD + 24.0, heading.title.clone(), TITLE, TEXT, Align::Left);
    sheet.text(PAD, PAD + 48.0, heading.subtitle.clone(), SUBTITLE, TEXT_2, Align::Left);
    sheet.text(width - PAD, PAD + 24.0, heading.site.clone(), SITE, TEXT_3, Align::Right);

    // The band of the study years over the semesters' heads.
    let top = PAD + TITLE_BLOCK;
    sheet.ops.push(Op::Line { x1: PAD, y1: top, x2: right, y2: top, color: LINE_STRONG });
    let mut at = 0;
    for Band { label, span } in &layout.bands {
        let Some((x, _)) = sheet.columns.get(at).copied() else { break };
        let end = sheet.columns.get(at + span - 1).map_or(x, |(x, w)| x + w);
        let label = draw::one_line(&label.to_uppercase(), end - x - 16.0, BAND_FONT, measure);
        sheet.text(x + 8.0, top + 15.0, label, BAND_FONT, TEXT_3, Align::Left);
        sheet.ops.push(Op::Line { x1: x, y1: top, x2: x, y2: top + BAND, color: LINE });
        at += span;
    }
    sheet.ops.push(Op::Line { x1: PAD, y1: top + BAND, x2: right, y2: top + BAND, color: LINE });

    // The semesters' heads: name, „jetzt", Fachsemester and figure, the load.
    let heads = top + BAND;
    for (at, column) in layout.columns.iter().enumerate() {
        let Some((x, w)) = sheet.columns.get(at).copied() else { continue };
        match sheet.kinds.get(at).copied().unwrap_or_default() {
            "past" => sheet.fill(x, heads, w, HEADS, 0.0, PANEL_2),
            "is-now" => sheet.fill(x, heads, w, HEADS, 0.0, accent().over(PANEL, 0.06)),
            _ => {}
        }
        sheet.ops.push(Op::Line { x1: x, y1: heads, x2: x, y2: heads + HEADS, color: LINE });
        match column {
            Column::Past { span, credits } => {
                sheet.text(x + 8.0, heads + 20.0, t.study.so_far, HEAD_NAME, TEXT, Align::Left);
                sheet.text(x + 8.0, heads + 37.0, span.clone(), HEAD_SUB, TEXT_2, Align::Left);
                sheet.text(x + w - 8.0, heads + 37.0, format!("✓ {credits}"), HEAD_FIGURE, OK, Align::Right);
            }
            Column::Semester(head) => {
                sheet.text(x + 8.0, heads + 20.0, head.label.clone(), HEAD_NAME, TEXT, Align::Left);
                if head.now {
                    let label_w = measure(&head.label, HEAD_NAME);
                    let now_w = measure(t.study.now, FLAG_FONT) + 10.0;
                    sheet.fill(x + 14.0 + label_w, heads + 9.0, now_w, 15.0, 7.5, accent());
                    sheet.text(x + 19.0 + label_w, heads + 20.0, t.study.now, FLAG_FONT, PANEL, Align::Left);
                }
                sheet.text(x + 8.0, heads + 37.0, head.fs.clone().unwrap_or_default(), HEAD_SUB, TEXT_2, Align::Left);
                sheet.text(x + w - 8.0, heads + 37.0, head.figure.clone(), HEAD_FIGURE, if head.heavy { WARN } else { TEXT }, Align::Right);
                let (done, all) = head.load;
                let bar_w = w - 16.0;
                sheet.fill(x + 8.0, heads + 44.0, bar_w, 3.0, 1.5, Color::rgb(0xec, 0xee, 0xf1));
                sheet.fill(x + 8.0, heads + 44.0, bar_w * (done / all.max(1e-6)).clamp(0.0, 1.0), 3.0, 1.5, TEXT_3);
            }
        }
    }
    sheet.ops.push(Op::Line { x1: PAD, y1: heads + HEADS, x2: right, y2: heads + HEADS, color: LINE_STRONG });
    // The line of today begins at the foot of its semester's head.
    sheet.today(heads + HEADS - 7.0, 7.0);

    // The areas, and what of them is unfolded.
    let rows_top = heads + HEADS;
    let mut y = rows_top;
    for row in &layout.rows {
        y = sheet.area(row, open, y);
    }

    // The line of today's flag under the plan.
    if let Some(line) = sheet.today {
        sheet.today(y, 6.0);
        let flag_w = measure(&layout.today_flag, FLAG_FONT) + 12.0;
        sheet.fill(line - flag_w / 2.0, y + 6.0, flag_w, 16.0, 5.0, accent());
        sheet.text(line, y + 18.0, layout.today_flag.clone(), FLAG_FONT, PANEL, Align::Center);
    }
    let y = y + FLAG;

    // The legend.
    let mut x = PAD;
    let legend_y = y + 6.0;
    for (swatch, label) in &heading.legend {
        let c = tone("var(--fac-1)");
        match swatch {
            Swatch::Fill(fill) => {
                let look = look(*fill, false, c, false);
                sheet.fill(x, legend_y, 18.0, 11.0, 3.0, look.fill);
                if let Some((color, width, dashed)) = look.ring {
                    sheet.ops.push(Op::Ring { x, y: legend_y, w: 18.0, h: 11.0, radius: 3.0, color, width, dashed });
                }
            }
            Swatch::Over => {
                let look = look(Fill::Planned, true, c, false);
                sheet.fill(x, legend_y, 18.0, 11.0, 3.0, look.fill);
                if let Some(color) = look.hatch {
                    sheet.ops.push(Op::Hatch { x, y: legend_y, w: 18.0, h: 11.0, radius: 3.0, color });
                }
            }
            Swatch::Failed => sheet.text(x + 9.0, legend_y + 10.0, "✕", LEGEND_FONT, NO, Align::Center),
            Swatch::Retake => sheet.text(x + 9.0, legend_y + 10.0, "↻", LEGEND_FONT, TEXT_2, Align::Center),
            Swatch::Unoffered => sheet.text(x + 9.0, legend_y + 10.0, "⚠", LEGEND_FONT, WARN, Align::Center),
            Swatch::Today => sheet.fill(x + 8.0, legend_y - 1.0, 2.0, 14.0, 1.0, accent()),
        }
        sheet.text(x + 24.0, legend_y + 10.0, label.clone(), LEGEND_FONT, TEXT_2, Align::Left);
        x += 24.0 + measure(label, LEGEND_FONT) + 18.0;
    }
    let height = legend_y + LEGEND + PAD;

    let mut ops = vec![Op::Fill { x: 0.0, y: 0.0, w: width, h: height, radius: 0.0, color: PANEL, shadow: false }];
    ops.append(&mut sheet.ops);
    Picture { width, height, ops }
}

/// Draws the plan `layout` (the rows of `open` unfolded) under `heading` on a canvas and
/// downloads it as `name` (a PNG, `draw::save`). Only in the browser.
pub(super) fn save(layout: &Layout, open: &BTreeSet<String>, heading: &Heading, name: &str, t: &Texts) {
    draw::save(name, |measure| lay_out(layout, open, heading, measure, t));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n;
    use folia_calendar::semester::SemesterKey;
    use folia_plans::study::When;

    use super::super::model::{Head, SemesterCell};

    /// A width of 0.6 of the size per character, as Inter's letters are on average.
    fn measure(text: &str, font: Font) -> f64 {
        text.chars().count() as f64 * font.size * 0.6
    }

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn piece(label: &str, credits: f64, fill: Fill) -> Piece {
        Piece { key: format!("m:{label}"), semester: key("2026W"), label: label.to_string(), title: String::new(), grow: credits, fill, now: false, retake: false, unoffered: false, over: false, row: None, picks: Vec::new() }
    }

    fn lane(name: &str, piece: Piece, word: &'static str) -> Lane {
        Lane { name: name.to_string(), title: String::new(), credits: Some("6".to_string()), bars: vec![vec![(piece, word)], Vec::new()], keys: BTreeSet::new(), through: Vec::new(), into: None }
    }

    fn column(s: &str, now: bool) -> Column {
        Column::Semester(Head { key: key(s), label: s.to_string(), now, past: false, leave: false, fs: Some("3. FS".to_string()), figure: "12 / 30 LP".to_string(), heavy: false, load: (12.0, 30.0) })
    }

    fn texts(picture: &Picture) -> Vec<String> {
        picture.ops.iter().filter_map(|op| match op {
            Op::Text { text, .. } => Some(text.clone()),
            _ => None,
        }).collect()
    }

    fn layout(pieces: Vec<Piece>, subs: Vec<SubRow>) -> Layout {
        let cells = vec![Cell::Semester(SemesterCell { key: key("2026W"), when: When::Now, leave: false, pieces }), Cell::Semester(SemesterCell { key: key("2027S"), when: When::Later, leave: false, pieces: Vec::new() })];
        Layout {
            columns: vec![column("2026W", true), column("2027S", false)],
            bands: vec![Band { label: "2. Studienjahr · 2026/27".to_string(), span: 2 }],
            rows: vec![AreaRow { area: Some(0), name: "Fachstudium".to_string(), tone: "var(--fac-6)", figure: "34/54".to_string(), done: false, over: false, bar: (12.0, 22.0, 20.0, 0.0), cells, lanes: Vec::new(), subs }],
            today: Some((0, 0.25)),
            today_flag: "heute, 09.11.".to_string(),
        }
    }

    fn heading() -> Heading {
        Heading { title: "Gesamtplan".to_string(), subtitle: "Informatik B.Sc. · Stand 09.11.2026".to_string(), site: "betula.app".to_string(), legend: legend(i18n::texts(folia_locale::Locale::De)) }
    }

    /// Every semester stands in the picture with its pieces by their labels; the title, the band,
    /// the heads, the line of today and the legend round them.
    #[test]
    fn the_picture_shows_the_plan() {
        let t = i18n::texts(folia_locale::Locale::De);
        let plan = layout(vec![piece("DB", 6.0, Fill::Passed), piece("SE", 8.0, Fill::Running)], Vec::new());
        let picture = lay_out(&plan, &BTreeSet::new(), &heading(), &measure, t);
        let said = texts(&picture);
        for text in ["Gesamtplan", "Informatik B.Sc. · Stand 09.11.2026", "betula.app", "2. STUDIENJAHR · 2026/27", "2026W", "jetzt", "Fachstudium", "34/54", "DB", "SE", "heute, 09.11.", "Bestanden", "Über Bedarf"] {
            assert!(said.iter().any(|known| known == text), "{text} in {said:?}");
        }
        assert_eq!(picture.width, (PAD + NAMES + 2.0 * SEMESTER + PAD).max(720.0));
        assert!(picture.height > PAD + TITLE_BLOCK + BAND + HEADS + AREA);
    }

    /// Too many for their cell, the pieces are one saying how many they are.
    #[test]
    fn a_full_cell_says_how_many() {
        let t = i18n::texts(folia_locale::Locale::De);
        let many: Vec<Piece> = (0..8).map(|i| piece(&format!("Modul{i}"), 6.0, Fill::Planned)).collect();
        let mut plan = layout(many.clone(), Vec::new());
        if let Some(row) = plan.rows.first_mut() {
            row.lanes = many.into_iter().map(|piece| lane(&piece.label.clone(), piece, "eingeplant")).collect();
        }
        let picture = lay_out(&plan, &BTreeSet::new(), &heading(), &measure, t);
        let said = texts(&picture);
        assert!(said.iter().any(|text| text.starts_with("8 Module")), "{said:?}");
        assert!(!said.iter().any(|text| text == "Modul0"));
    }

    /// A row that does not unfold (`AreaRow::unfolds`: one lane alone; owner, 2026-11-10: „dass ein
    /// platzhalter, der nur für ein modul ist kein ausklappbares element wird") has no „1
    /// Platzhalter" for a piece too wide for its cell, as the page has none: the piece's label is
    /// cut. Kept open from before, its lane is not drawn.
    #[test]
    fn a_still_row_cuts_its_label() {
        let t = i18n::texts(folia_locale::Locale::De);
        let long = "Wahlpflichtmodul aus dem Bereich der Informatik";
        let place = piece(long, 6.0, Fill::Row);
        let mut plan = layout(vec![place.clone()], Vec::new());
        if let Some(row) = plan.rows.first_mut() {
            row.lanes = vec![lane(long, place, "Platzhalter")];
        }
        let open: BTreeSet<String> = ["Fachstudium".to_string()].into();
        let said = texts(&lay_out(&plan, &open, &heading(), &measure, t));
        assert!(!said.iter().any(|text| text.ends_with("Platzhalter")), "{said:?}");
        assert!(said.iter().any(|text| text.starts_with("? Wahlpflichtmodul") && text.ends_with('…')), "{said:?}");
    }

    /// An area unfolded shows its subsections with what they hold against what they ask, and a
    /// subsection unfolded its lanes; folded, neither.
    #[test]
    fn what_is_unfolded_is_in_the_picture() {
        let t = i18n::texts(folia_locale::Locale::De);
        let sub = SubRow {
            key: "Fachstudium\tPraktische Informatik".to_string(),
            group: Some(3),
            kind: Some(folia_plans::bench::GroupKind::Choice(7)),
            name: "Praktische Informatik".to_string(),
            title: String::new(),
            figure: "18/10–24".to_string(),
            done: true,
            stand: Some((Stand { passed: 6.0, planned: 12.0, least: 10.0, most: 24.0, spare: 0.0 }, Some((Room::Open, 6.0)))),
            cells: vec![Cell::Semester(SemesterCell { key: key("2026W"), when: When::Now, leave: false, pieces: vec![piece("DB", 6.0, Fill::Passed), piece("SE", 6.0, Fill::Passed)] }), Cell::Semester(SemesterCell { key: key("2027S"), when: When::Later, leave: false, pieces: Vec::new() })],
            lanes: vec![lane("Datenbanken", piece("DB", 6.0, Fill::Passed), "bestanden"), lane("Softwaretechnik", piece("SE", 6.0, Fill::Passed), "bestanden")],
        };
        let mut plan = layout(vec![piece("DB", 6.0, Fill::Passed), piece("SE", 6.0, Fill::Passed)], vec![sub]);
        let folded = texts(&lay_out(&plan, &BTreeSet::new(), &heading(), &measure, t));
        assert!(!folded.iter().any(|text| text == "Praktische Informatik"));
        let area: BTreeSet<String> = ["Fachstudium".to_string()].into();
        let unfolded = texts(&lay_out(&plan, &area, &heading(), &measure, t));
        assert!(unfolded.iter().any(|text| text == "Praktische Informatik") && unfolded.iter().any(|text| text == "✓ 18/10–24"));
        assert!(!unfolded.iter().any(|text| text == "Datenbanken"));
        let both: BTreeSet<String> = ["Fachstudium".to_string(), "Fachstudium\tPraktische Informatik".to_string()].into();
        let lanes = texts(&lay_out(&plan, &both, &heading(), &measure, t));
        assert!(lanes.iter().any(|text| text == "Datenbanken") && lanes.iter().any(|text| text == "Softwaretechnik"));
        // Of one lane alone it does not unfold, whatever was kept open.
        if let Some(sub) = plan.rows.first_mut().and_then(|row| row.subs.first_mut()) {
            sub.lanes.truncate(1);
        }
        let single = texts(&lay_out(&plan, &both, &heading(), &measure, t));
        assert!(single.iter().any(|text| text == "Praktische Informatik") && !single.iter().any(|text| text == "Datenbanken"));
    }
}
