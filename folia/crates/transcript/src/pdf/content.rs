//! What a page shows that a Notenübersicht needs (ISO 32000-1 §8.2–8.5, §9.3–9.4): every piece of
//! text, where it begins and ends, its size and whether its font is bold or italic, and every
//! horizontal line, which is what rules a table. In the default user space: points, y upwards.
//! Form XObjects are not followed (the Notenübersicht writes its text on the page itself), nor
//! curves, colours, clipping and images, which say nothing of what is written.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::font::Font;
use super::syntax::{Item, Object, Parser};
use super::{Document, PageRef};

/// How deep `q` nests; a page nests a few levels.
const MAX_STACK: usize = 256;

/// The most operands an operator takes; a longer list is not of this syntax.
const MAX_OPERANDS: usize = 64;

/// The most pieces of text and lines a page may yield.
const MAX_RUNS: usize = 100_000;

/// A line no thicker or more slanted than this many points is a rule.
const RULE_THICKNESS: f64 = 2.0;

/// A line shorter than this is no rule (a tick, a dot).
const RULE_LENGTH: f64 = 3.0;

/// A gap in a `TJ` of more than this part of the size is a space between words.
const WORD_GAP: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const ONE: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    fn of(numbers: &[f64]) -> Option<Matrix> {
        match numbers {
            [a, b, c, d, e, f] => Some(Matrix { a: *a, b: *b, c: *c, d: *d, e: *e, f: *f }),
            _ => None,
        }
    }

    fn translate(x: f64, y: f64) -> Matrix {
        Matrix { e: x, f: y, ..Matrix::ONE }
    }

    /// This one, then `next`: `self × next`.
    pub fn then(self, next: Matrix) -> Matrix {
        Matrix {
            a: self.a * next.a + self.b * next.c,
            b: self.a * next.b + self.b * next.d,
            c: self.c * next.a + self.d * next.c,
            d: self.c * next.b + self.d * next.d,
            e: self.e * next.a + self.f * next.c + next.e,
            f: self.e * next.b + self.f * next.d + next.f,
        }
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (x * self.a + y * self.c + self.e, x * self.b + y * self.d + self.f)
    }
}

/// A piece of text as one operator shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Where it begins and ends on its baseline.
    pub x: f64,
    pub end: f64,
    pub y: f64,
    /// Its size on the page.
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub text: String,
}

/// A horizontal line from `x0` to `x1` at `y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub y: f64,
    pub x0: f64,
    pub x1: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub runs: Vec<Run>,
    pub rules: Vec<Rule>,
}

#[derive(Clone)]
struct Text {
    tm: Matrix,
    tlm: Matrix,
    font: Option<Rc<Font>>,
    size: f64,
    char_spacing: f64,
    word_spacing: f64,
    scale: f64,
    leading: f64,
    rise: f64,
}

impl Default for Text {
    fn default() -> Self {
        Text { tm: Matrix::ONE, tlm: Matrix::ONE, font: None, size: 0.0, char_spacing: 0.0, word_spacing: 0.0, scale: 1.0, leading: 0.0, rise: 0.0 }
    }
}

/// The graphics state as far as it is read: the CTM, and the text state, which `q` and `Q` keep
/// too (§8.4.1).
#[derive(Clone)]
struct State {
    ctm: Matrix,
    text: Text,
}

/// What a page shows: its text and its rules.
pub fn read(document: &Document, page: &PageRef) -> Page {
    let data = document.contents(page);
    let font_dict = document.entry(&page.resources, b"Font");
    let mut fonts: BTreeMap<Vec<u8>, Rc<Font>> = BTreeMap::new();
    let mut load = |name: &[u8]| -> Option<Rc<Font>> {
        if let Some(font) = fonts.get(name) {
            return Some(font.clone());
        }
        let dict = document.entry(font_dict.dict()?, name);
        let font = Rc::new(Font::load(document, dict.dict()?));
        fonts.insert(name.to_vec(), font.clone());
        Some(font)
    };

    let mut out = Page::default();
    let mut parser = Parser::new(&data, 0, false);
    let mut operands: Vec<Object> = Vec::new();
    let mut stack: Vec<State> = Vec::new();
    let mut state = State { ctm: Matrix::ONE, text: Text::default() };
    // The path being built, in the page's space: its segments and its rectangles.
    let mut segments: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut rectangles: Vec<[(f64, f64); 4]> = Vec::new();
    let mut current: Option<(f64, f64)> = None;
    let mut start: Option<(f64, f64)> = None;

    while let Some(item) = parser.item() {
        let op = match item {
            Item::Object(object) => {
                if operands.len() < MAX_OPERANDS {
                    operands.push(object);
                }
                continue;
            }
            Item::Keyword(op) => op,
        };
        let numbers: Vec<f64> = operands.iter().filter_map(Object::number).collect();
        let point = |at: usize| -> Option<(f64, f64)> { Some((*numbers.get(at)?, *numbers.get(at + 1)?)) };
        match op.as_slice() {
            b"q" => {
                if stack.len() < MAX_STACK {
                    stack.push(state.clone());
                }
            }
            b"Q" => {
                if let Some(before) = stack.pop() {
                    state = before;
                }
            }
            b"cm" => {
                if let Some(m) = Matrix::of(&numbers) {
                    state.ctm = m.then(state.ctm);
                }
            }
            b"BT" => {
                state.text.tm = Matrix::ONE;
                state.text.tlm = Matrix::ONE;
            }
            b"Tf" => {
                if let (Some(name), Some(size)) = (operands.first().and_then(Object::name), operands.get(1).and_then(Object::number)) {
                    state.text.font = load(name);
                    state.text.size = size;
                }
            }
            b"Tc" => state.text.char_spacing = numbers.first().copied().unwrap_or(0.0),
            b"Tw" => state.text.word_spacing = numbers.first().copied().unwrap_or(0.0),
            b"Tz" => state.text.scale = numbers.first().copied().unwrap_or(100.0) / 100.0,
            b"TL" => state.text.leading = numbers.first().copied().unwrap_or(0.0),
            b"Ts" => state.text.rise = numbers.first().copied().unwrap_or(0.0),
            b"Td" | b"TD" => {
                if let Some((x, y)) = point(0) {
                    if op == b"TD" {
                        state.text.leading = -y;
                    }
                    state.text.tlm = Matrix::translate(x, y).then(state.text.tlm);
                    state.text.tm = state.text.tlm;
                }
            }
            b"Tm" => {
                if let Some(m) = Matrix::of(&numbers) {
                    state.text.tm = m;
                    state.text.tlm = m;
                }
            }
            b"T*" => next_line(&mut state.text),
            b"Tj" | b"'" | b"\"" | b"TJ" => {
                if op == b"\"" {
                    state.text.word_spacing = numbers.first().copied().unwrap_or(state.text.word_spacing);
                    state.text.char_spacing = numbers.get(1).copied().unwrap_or(state.text.char_spacing);
                }
                if op == b"'" || op == b"\"" {
                    next_line(&mut state.text);
                }
                let parts: Vec<Object> = match operands.last() {
                    Some(Object::Array(parts)) if op == b"TJ" => parts.clone(),
                    Some(string @ Object::Str(_)) => vec![string.clone()],
                    _ => Vec::new(),
                };
                if let Some(run) = show(&mut state, &parts) {
                    if out.runs.len() < MAX_RUNS {
                        out.runs.push(run);
                    }
                }
            }
            b"m" => {
                if let Some((x, y)) = point(0) {
                    let at = state.ctm.apply(x, y);
                    current = Some(at);
                    start = Some(at);
                }
            }
            b"l" => {
                if let Some((x, y)) = point(0) {
                    let to = state.ctm.apply(x, y);
                    if let Some(from) = current {
                        segments.push((from, to));
                    }
                    current = Some(to);
                }
            }
            b"c" => current = point(4).map(|(x, y)| state.ctm.apply(x, y)).or(current),
            b"v" | b"y" => current = point(2).map(|(x, y)| state.ctm.apply(x, y)).or(current),
            b"re" => {
                if let (Some((x, y)), Some((w, h))) = (point(0), point(2)) {
                    let corners = [state.ctm.apply(x, y), state.ctm.apply(x + w, y), state.ctm.apply(x + w, y + h), state.ctm.apply(x, y + h)];
                    rectangles.push(corners);
                    let [a, b, c, d] = corners;
                    segments.extend([(a, b), (b, c), (c, d), (d, a)]);
                    current = Some(a);
                    start = Some(a);
                }
            }
            b"h" => {
                if let (Some(from), Some(to)) = (current, start) {
                    segments.push((from, to));
                    current = Some(to);
                }
            }
            b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                if op == b"s" || op == b"b" || op == b"b*" {
                    if let (Some(from), Some(to)) = (current, start) {
                        segments.push((from, to));
                    }
                }
                let stroked = matches!(op.as_slice(), b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
                let filled = matches!(op.as_slice(), b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*");
                if stroked {
                    for (from, to) in &segments {
                        if let Some(rule) = rule_of(&[*from, *to]) {
                            out.rules.push(rule);
                        }
                    }
                }
                if filled {
                    for corners in &rectangles {
                        if let Some(rule) = rule_of(corners) {
                            out.rules.push(rule);
                        }
                    }
                }
                if out.rules.len() > MAX_RUNS {
                    out.rules.truncate(MAX_RUNS);
                }
                segments.clear();
                rectangles.clear();
                current = None;
                start = None;
            }
            b"BI" => parser.skip_inline_image(),
            _ => {}
        }
        operands.clear();
    }
    out
}

/// `T*`: the start of the next line.
fn next_line(text: &mut Text) {
    text.tlm = Matrix::translate(0.0, -text.leading).then(text.tlm);
    text.tm = text.tlm;
}

/// The run of a text-showing operator: its strings, and in a `TJ` the numbers that move between
/// them (a move wide enough is a space). The text matrix moves on by what was shown (§9.4.4).
fn show(state: &mut State, parts: &[Object]) -> Option<Run> {
    let font = state.text.font.clone()?;
    let text = &mut state.text;
    let size = text.size;
    let render = |tm: Matrix, ctm: Matrix, size: f64, scale: f64, rise: f64| Matrix { a: size * scale, b: 0.0, c: 0.0, d: size, e: 0.0, f: rise }.then(tm).then(ctm);
    let first = render(text.tm, state.ctm, size, text.scale, text.rise);
    let (x, y) = first.apply(0.0, 0.0);
    let mut shown = String::new();
    for part in parts {
        match part {
            Object::Str(bytes) => {
                for code in font.codes(bytes) {
                    shown.push_str(&font.text(code));
                    let spacing = if !font.wide && code == 32 { text.word_spacing } else { 0.0 };
                    let advance = (font.width(code) / 1000.0 * size + text.char_spacing + spacing) * text.scale;
                    text.tm = Matrix::translate(advance, 0.0).then(text.tm);
                }
            }
            other => {
                let Some(n) = other.number() else { continue };
                let advance = -n / 1000.0 * size * text.scale;
                text.tm = Matrix::translate(advance, 0.0).then(text.tm);
                if -n / 1000.0 > WORD_GAP && !shown.is_empty() && !shown.ends_with(' ') {
                    shown.push(' ');
                }
            }
        }
    }
    let last = render(text.tm, state.ctm, size, text.scale, text.rise);
    let (end, _) = last.apply(0.0, 0.0);
    if shown.trim().is_empty() {
        return None;
    }
    Some(Run { x, end, y, size: first.c.hypot(first.d), bold: font.bold, italic: font.italic, text: shown })
}

/// A rule, where the points (a segment's two, a rectangle's four) lie along a thin horizontal
/// band long enough.
fn rule_of(points: &[(f64, f64)]) -> Option<Rule> {
    let xs = points.iter().map(|(x, _)| *x);
    let ys = points.iter().map(|(_, y)| *y);
    let (x0, x1) = xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
    let (y0, y1) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| (lo.min(y), hi.max(y)));
    (x1 - x0 >= RULE_LENGTH && y1 - y0 <= RULE_THICKNESS && x0.is_finite() && y0.is_finite()).then_some(Rule { y: (y0 + y1) / 2.0, x0, x1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices_compose_as_the_spec_writes_them() {
        // Translate, then flip y on a page 800 high: (10, 20) → (10, 780).
        let flip = Matrix { a: 1.0, b: 0.0, c: 0.0, d: -1.0, e: 0.0, f: 800.0 };
        let m = Matrix::translate(10.0, 20.0).then(flip);
        assert_eq!(m.apply(0.0, 0.0), (10.0, 780.0));
    }

    #[test]
    fn a_rule_is_long_and_flat() {
        assert_eq!(rule_of(&[(10.0, 5.0), (100.0, 5.4)]), Some(Rule { y: 5.2, x0: 10.0, x1: 100.0 }));
        assert_eq!(rule_of(&[(10.0, 5.0), (10.0, 50.0)]), None);
        assert_eq!(rule_of(&[(10.0, 5.0), (11.0, 5.0)]), None);
        assert_eq!(rule_of(&[(0.0, 0.0), (200.0, 0.0), (200.0, 0.5), (0.0, 0.5)]), Some(Rule { y: 0.25, x0: 0.0, x1: 200.0 }));
    }
}
