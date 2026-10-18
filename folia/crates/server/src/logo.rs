//! The icon of Betula and the wordmark as the server draws them with resvg, for the launch screens
//! of iOS (`launch`) and the link-preview cards (`cards`): the birch leaf in paper on a square of
//! ink with the bars of birch bark cut out of it, turned over on a dark page (the leaf of the page's
//! icon, `folia_design::ui::LEAF`), and the wordmark with the spacing of `design/logo/logo.html`
//! (display cut, offsets in em of its size, measured on Inter).

use folia_design::ui::{LEAF, LEAF_DY};

/// The ink of the text, and the ground of the icon.
pub const INK: [u8; 3] = [0x10, 0x15, 0x1f];
/// The leaf of the icon on ink.
pub const PAPER: [u8; 3] = [0xf4, 0xf5, 0xf7];
/// The ground of the icon on a dark page (the dark theme's `--bark`), the leaf in ink on it.
pub const PAPER_DARK: [u8; 3] = [0xe9, 0xed, 0xf5];

/// A colour as SVG writes it.
pub fn hex([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// The corner of the square on the 96 grid, as the icons of the manifest have it.
pub const RADIUS: f32 = 21.0;

/// The icon at (x, y), `size` wide: paper on ink, or `dark`, ink on paper.
pub fn app_icon(svg: &mut String, x: f32, y: f32, size: f32, dark: bool) {
    let scale = size / 96.0;
    let (ground, leaf) = if dark { (hex(PAPER_DARK), hex(INK)) } else { (hex(INK), hex(PAPER)) };
    svg.push_str(&format!(
        "<g transform=\"translate({x} {y}) scale({scale})\"><rect width=\"96\" height=\"96\" rx=\"{RADIUS}\" fill=\"{ground}\"/>\
         <path transform=\"translate(0 {LEAF_DY})\" d=\"{LEAF}\" fill=\"{leaf}\"/></g>"
    ));
}

/// A cut of the wordmark (`.wordmark` in app.css), in em of its size: where the letters of BᴇTUʟᴀ
/// stand (offset, letter, capital), how large and heavy the small capitals are, and how wide its
/// box is, as a page centres it.
pub struct Cut {
    letters: [(f32, &'static str, bool); 6],
    small: f32,
    weight: u16,
    pub width: f32,
}

/// The display cut, for sizes from 28 px on: the cards.
pub const DISPLAY: Cut = Cut {
    letters: [(0.0, "B", true), (0.63871, "E", false), (0.83563, "T", true), (1.47239, "U", true), (2.14984, "L", false), (2.57135, "A", false)],
    small: 0.72,
    weight: 400,
    width: 3.08375,
};

/// The small cut (`.wordmark.small`), for sizes under 28 px: the start page's at 22 px, and the
/// launch screens of iOS. Measured as Chromium sets it at 22 px.
pub const SMALL: Cut = Cut {
    letters: [(0.0, "B", true), (0.66832, "E", false), (0.87571, "T", true), (1.51278, "U", true), (2.21023, "L", false), (2.60795, "A", false)],
    small: 0.68,
    weight: 500,
    width: 3.10369,
};

/// The wordmark in `cut` with its baseline at `baseline`, from `left`, capitals `size` high in em.
pub fn wordmark(svg: &mut String, cut: &Cut, left: f32, baseline: f32, size: f32, fill: &str) {
    for (offset, letter, capital) in cut.letters {
        let (font_size, weight) = if capital { (size, 800) } else { (size * cut.small, cut.weight) };
        svg.push_str(&format!("<text x=\"{}\" y=\"{baseline}\" font-size=\"{font_size}\" font-weight=\"{weight}\" fill=\"{fill}\">{letter}</text>", left + offset * size));
    }
}
