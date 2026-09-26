//! The logo as the server draws it with resvg: the mark (a birch leaf with the marks of birch
//! bark, on the green of the leaf) and the wordmark. The link-preview cards (`cards`) and the
//! launch screens of iOS (`launch`) put it into their pictures. The paths are those of
//! `design/logo/mark.mjs` on the 96 grid (`node design/logo/render-icons.mjs` prints them); the
//! wordmark keeps the spacing of `design/logo/logo.html` (display cut, offsets in em of its size,
//! measured on Inter).

pub const LEAF: &str = "M48 14.7C49.6 23.6 59.9 37 67.6 49.2C71.7 55.3 71.5 60.4 67.6 63.1C61.7 65.9 53.9 67 48 68.2C42.1 67 34.3 65.9 28.4 63.1C24.5 60.4 24.3 55.3 28.4 49.2C36.1 37 46.4 23.6 48 14.7Z";
/// The four rows, each cut to the leaf.
pub const ROWS: &str = "M48.9 31.3H40.12C39.15 32.85 38.13 34.42 37.08 36H48.9ZM50.4 40.2H61.7C62.75 41.77 63.8 43.34 64.82 44.9H50.4ZM38.8 49.1H28.46C28.44 49.13 28.42 49.17 28.4 49.2C27.32 50.81 26.53 52.35 26.04 53.8H38.8ZM42.7 58H70.57C70.42 59.9 69.59 61.5 68.13 62.7H42.7Z";
pub const STEM: &str = "M48 65.9Q48.8 72.6 45.5 78.2";
pub const STEM_WIDTH: f32 = 3.6;
/// The green of the birch leaf, from the top to the bottom of the square; the ink of the rows.
pub const GREEN_TOP: [u8; 3] = [0x6d, 0xac, 0x78];
pub const GREEN_BOTTOM: [u8; 3] = [0x2d, 0x69, 0x45];
pub const INK: [u8; 3] = [0x10, 0x15, 0x1f];

/// A colour as SVG writes it.
pub fn hex([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}
/// The corner of the square on the 96 grid, as the icons of the site have it.
pub const RADIUS: f32 = 21.0;

/// The mark with its square at (x, y), `size` wide. `id` names its gradient, unique in the picture.
pub fn mark(svg: &mut String, x: f32, y: f32, size: f32, id: &str) {
    let scale = size / 96.0;
    let (top, bottom, ink) = (hex(GREEN_TOP), hex(GREEN_BOTTOM), hex(INK));
    svg.push_str(&format!(
        "<g transform=\"translate({x} {y}) scale({scale})\">\
         <linearGradient id=\"{id}\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"96\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"{top}\"/><stop offset=\"1\" stop-color=\"{bottom}\"/></linearGradient>\
         <rect width=\"96\" height=\"96\" rx=\"{RADIUS}\" fill=\"url(#{id})\"/>\
         <path d=\"{STEM}\" fill=\"none\" stroke=\"#fff\" stroke-width=\"{STEM_WIDTH}\" stroke-linecap=\"round\"/>\
         <path d=\"{LEAF}\" fill=\"#fff\"/><path d=\"{ROWS}\" fill=\"{ink}\"/></g>"
    ));
}

/// Where the letters of BᴇTUʟᴀ stand, in em of the wordmark's size: (offset, letter, capital).
const LETTERS: [(f32, &str, bool); 6] = [(0.0, "B", true), (0.63871, "E", false), (0.83563, "T", true), (1.47239, "U", true), (2.14984, "L", false), (2.57135, "A", false)];
/// The small capitals, in em of the wordmark's size.
const SMALL: f32 = 0.72;

/// The wordmark with its baseline at `baseline`, from `left`, capitals `size` high in em.
pub fn wordmark(svg: &mut String, left: f32, baseline: f32, size: f32, fill: &str) {
    for (offset, letter, capital) in LETTERS {
        let (font_size, weight) = if capital { (size, 800) } else { (size * SMALL, 400) };
        svg.push_str(&format!("<text x=\"{}\" y=\"{baseline}\" font-size=\"{font_size}\" font-weight=\"{weight}\" fill=\"{fill}\">{letter}</text>", left + offset * size));
    }
}

/// How wide the wordmark is at `size`: up to the end of its last letter, the small capital A of
/// Inter Regular (`regular`, the face resvg sets it in).
pub fn wordmark_width(regular: &rustybuzz::Face<'_>, size: f32) -> f32 {
    let Some(&(offset, letter, _)) = LETTERS.last() else { return 0.0 };
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(letter);
    let shaped = rustybuzz::shape(regular, &[], buffer);
    let advance: i32 = shaped.glyph_positions().iter().map(|glyph| glyph.x_advance).sum();
    offset * size + advance as f32 * size * SMALL / regular.units_per_em() as f32
}
