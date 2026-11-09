//! Pictures the browser draws and downloads as a PNG, so nothing of a plan reaches the server: the
//! Stundenplan's week („Als Bild speichern", `studyplan::picture`) and the Gesamtplan („Als Bild
//! speichern", `study::grid::picture`).
//!
//! A picture is laid out as plain values (`Picture`, a list of what to draw, in CSS pixels), which
//! the tests read, and drawn on a canvas at twice that size (`save`), in the app's typeface, whose
//! widths the canvas measures. Its colours are the light theme's whatever the page shows (a
//! picture is shared and printed).

/// How many pixels of the PNG a CSS pixel is.
#[cfg(feature = "csr")]
const SCALE: f64 = 2.0;

/// A colour with its opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: f64,
}

impl Color {
    pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 1.0 }
    }

    pub(crate) const fn alpha(self, a: f64) -> Self {
        Color { a, ..self }
    }

    /// The colour as the canvas takes it: „rgba(16,21,31,1)".
    pub(crate) fn css(self) -> String {
        format!("rgba({},{},{},{})", self.r, self.g, self.b, self.a)
    }

    /// `share` of this colour over `under`, in sRGB.
    pub(crate) fn over(self, under: Color, share: f64) -> Color {
        let mix = |a: u8, b: u8| channel(f64::from(a) * share + f64::from(b) * (1.0 - share));
        Color::rgb(mix(self.r, under.r), mix(self.g, under.g), mix(self.b, under.b))
    }
}

/// A channel of 0–255 from a number that may lie outside it.
fn channel(value: f64) -> u8 {
    // In range after the clamp, so the cast loses nothing but the fraction.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

/// An OKLCH colour in sRGB (Björn Ottosson's OKLab), the way the stylesheet writes the tones.
pub(crate) fn oklch(l: f64, c: f64, h: f64) -> Color {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    let linear = [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_,
    ];
    let encode = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        let v = if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        channel(v * 255.0)
    };
    Color::rgb(encode(linear[0]), encode(linear[1]), encode(linear[2]))
}

// The light theme's colours (app.css `:root`).
pub(crate) const PANEL: Color = Color::rgb(0xff, 0xff, 0xff);
pub(crate) const PANEL_2: Color = Color::rgb(0xf6, 0xf7, 0xf8);
pub(crate) const TEXT: Color = Color::rgb(0x10, 0x15, 0x1f);
pub(crate) const TEXT_2: Color = Color::rgb(0x4b, 0x55, 0x65);
pub(crate) const TEXT_3: Color = Color::rgb(0x87, 0x90, 0xa0);
pub(crate) const LINE: Color = Color::rgb(15, 23, 42).alpha(0.08);
pub(crate) const LINE_STRONG: Color = Color::rgb(15, 23, 42).alpha(0.15);
pub(crate) const NO: Color = Color::rgb(0xb3, 0x26, 0x1e);
pub(crate) const WARN: Color = Color::rgb(0x8f, 0x5a, 0x0a);
pub(crate) const OK: Color = Color::rgb(0x0b, 0x7f, 0x58);

/// A typeface's weight and size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Font {
    pub size: f64,
    pub weight: u16,
}

impl Font {
    /// The font as the canvas takes it, the app's typeface first.
    #[cfg(feature = "csr")]
    fn css(self) -> String {
        format!("{} {}px Inter, \"Inter Variable\", system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif", self.weight, self.size)
    }
}

/// How a text stands at its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Left,
    Center,
    Right,
}

/// What the picture draws, in order, in CSS pixels.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    /// A rounded box, with the shadow of a card where `shadow`.
    Fill { x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color, shadow: bool },
    /// Stripes across a rounded box.
    Hatch { x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color },
    /// A ring inside a rounded box.
    Ring { x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color, width: f64, dashed: bool },
    Line { x1: f64, y1: f64, x2: f64, y2: f64, color: Color },
    /// A line of text, its baseline at `y`.
    Text { x: f64, y: f64, text: String, font: Font, color: Color, align: Align },
}

/// The picture: its size and what it draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Picture {
    pub width: f64,
    pub height: f64,
    pub ops: Vec<Op>,
}

/// `text` in lines of at most `width` (by `measure`), at most `most` of them, broken between words;
/// what does not fit ends the last line in „…", and so does a word too wide for a line.
pub(crate) fn lines_of(text: &str, width: f64, most: usize, font: Font, measure: &dyn Fn(&str, Font) -> f64) -> Vec<String> {
    let fits = |line: &str| measure(line, font) <= width;
    let cut = |line: &str| {
        let mut chars: Vec<char> = line.chars().collect();
        loop {
            let text = format!("{}…", chars.iter().collect::<String>().trim_end());
            if chars.is_empty() || fits(&text) {
                return text;
            }
            chars.pop();
        }
    };
    let mut lines: Vec<String> = Vec::new();
    let mut words = text.split_whitespace().peekable();
    while lines.len() < most {
        let Some(word) = words.next() else { break };
        let mut line = word.to_string();
        if !fits(&line) {
            lines.push(cut(&line));
            // A word cut ends the label, as the page's grid ends it.
            return lines;
        }
        while let Some(word) = words.next_if(|word| fits(&format!("{line} {word}"))) {
            line = format!("{line} {word}");
        }
        lines.push(line);
    }
    if words.peek().is_some() {
        if let Some(last) = lines.last_mut() {
            let rest: Vec<&str> = words.collect();
            *last = cut(&format!("{last} {}", rest.join(" ")));
        }
    }
    lines
}

/// `text` in one line of at most `width`, cut with „…" where it is wider.
pub(crate) fn one_line(text: &str, width: f64, font: Font, measure: &dyn Fn(&str, Font) -> f64) -> String {
    if measure(text, font) <= width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    loop {
        let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
        if chars.is_empty() || measure(&cut, font) <= width {
            return cut;
        }
        chars.pop();
    }
}

/// Lays out a picture with the canvas's widths of its texts (`lay_out`), draws it on a canvas and
/// downloads it as `name` (a PNG). Only in the browser; nothing happens where the canvas is
/// missing.
#[cfg(feature = "csr")]
pub(crate) fn save(name: &str, lay_out: impl FnOnce(&dyn Fn(&str, Font) -> f64) -> Picture) {
    use leptos::wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use web_sys::{CanvasRenderingContext2d, HtmlAnchorElement, HtmlCanvasElement};

    let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
    let Some(canvas) = document.create_element("canvas").ok().and_then(|canvas| canvas.dyn_into::<HtmlCanvasElement>().ok()) else { return };
    let Some(context) = canvas.get_context("2d").ok().flatten().and_then(|context| context.dyn_into::<CanvasRenderingContext2d>().ok()) else { return };
    let measure = |text: &str, font: Font| {
        context.set_font(&font.css());
        context.measure_text(text).map(|metrics| metrics.width()).unwrap_or_default()
    };
    let picture = lay_out(&measure);
    // Whole pixels of the PNG, as large as the picture asks.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = ((picture.width * SCALE).ceil() as u32, (picture.height * SCALE).ceil() as u32);
    canvas.set_width(width);
    canvas.set_height(height);
    let _ = context.scale(SCALE, SCALE);
    let rounded = |x: f64, y: f64, w: f64, h: f64, r: f64| {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        context.begin_path();
        context.move_to(x + r, y);
        let _ = context.arc_to(x + w, y, x + w, y + h, r);
        let _ = context.arc_to(x + w, y + h, x, y + h, r);
        let _ = context.arc_to(x, y + h, x, y, r);
        let _ = context.arc_to(x, y, x + w, y, r);
        context.close_path();
    };
    for op in &picture.ops {
        match op {
            Op::Fill { x, y, w, h, radius, color, shadow } => {
                context.save();
                if *shadow {
                    context.set_shadow_color("rgba(15,23,42,0.22)");
                    context.set_shadow_blur(6.0);
                    context.set_shadow_offset_y(2.0);
                }
                context.set_fill_style_str(&color.css());
                rounded(*x, *y, *w, *h, *radius);
                context.fill();
                context.restore();
            }
            Op::Hatch { x, y, w, h, radius, color } => {
                context.save();
                rounded(*x, *y, *w, *h, *radius);
                context.clip();
                context.set_stroke_style_str(&color.css());
                context.set_line_width(1.6);
                let mut at = -h;
                while at < *w {
                    context.begin_path();
                    context.move_to(x + at, y + h);
                    context.line_to(x + at + h, *y);
                    context.stroke();
                    at += 6.0;
                }
                context.restore();
            }
            Op::Ring { x, y, w, h, radius, color, width, dashed } => {
                context.save();
                context.set_stroke_style_str(&color.css());
                context.set_line_width(*width);
                if *dashed {
                    let _ = context.set_line_dash(&web_sys::js_sys::Array::of2(&JsValue::from_f64(4.0), &JsValue::from_f64(3.0)));
                }
                // Inside the box, as the page's inset ring.
                let half = width / 2.0;
                rounded(x + half, y + half, w - width, h - width, (radius - half).max(0.0));
                context.stroke();
                context.restore();
            }
            Op::Line { x1, y1, x2, y2, color } => {
                context.set_stroke_style_str(&color.css());
                context.set_line_width(1.0);
                context.begin_path();
                context.move_to(*x1, *y1 + 0.5);
                context.line_to(*x2, *y2 + 0.5);
                context.stroke();
            }
            Op::Text { x, y, text, font, color, align } => {
                context.set_font(&font.css());
                context.set_fill_style_str(&color.css());
                context.set_text_align(match align {
                    Align::Left => "left",
                    Align::Center => "center",
                    Align::Right => "right",
                });
                let _ = context.fill_text(text, *x, *y);
            }
        }
    }
    let name = name.to_string();
    let done = Closure::once_into_js(move |blob: JsValue| {
        let Some(blob) = blob.dyn_ref::<web_sys::Blob>() else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(blob) else { return };
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        if let Some(link) = document.create_element("a").ok().and_then(|link| link.dyn_into::<HtmlAnchorElement>().ok()) {
            link.set_href(&url);
            link.set_download(&name);
            let body = document.body();
            if let Some(body) = &body {
                let _ = body.append_child(&link);
            }
            link.click();
            link.remove();
        }
        // The browser has the file once the download began; the address can go a moment later.
        leptos::prelude::set_timeout(move || { let _ = web_sys::Url::revoke_object_url(&url); }, std::time::Duration::from_secs(30));
    });
    let _ = canvas.to_blob_with_type(done.unchecked_ref(), "image/png");
}

/// The server draws no picture: it renders no plan.
#[cfg(not(feature = "csr"))]
pub(crate) fn save(_name: &str, _lay_out: impl FnOnce(&dyn Fn(&str, Font) -> f64) -> Picture) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A width of 0.6 of the size per character, as Inter's letters are on average.
    fn measure(text: &str, font: Font) -> f64 {
        text.chars().count() as f64 * font.size * 0.6
    }

    #[test]
    fn a_text_is_cut_where_it_is_wider() {
        let font = Font { size: 10.0, weight: 600 };
        assert_eq!(one_line("Datenbanken", 100.0, font, &measure), "Datenbanken");
        assert_eq!(one_line("Datenbanken", 40.0, font, &measure), "Daten…");
        assert_eq!(one_line("Datenbanken", 0.0, font, &measure), "…");
    }
}
