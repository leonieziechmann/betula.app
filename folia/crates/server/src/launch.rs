//! The launch screens of the installed app on iPhones and iPads (`app::launch`), drawn in this
//! process like the link-preview cards: the icon of the installed app (the leaf) in the middle of
//! the page's background, the wordmark and what Betula is at the bottom, in the light and in the
//! dark scheme. The same picture as Android's splash screen, which shows the maskable icon on the
//! manifest's background; and what iOS shows when the icon on the home screen opens into the app.
//!
//! iOS fetches the picture of its screen when the app is added to the home screen and keeps it.
//! So a picture is drawn on its first request, one at a time (a large one takes a moment of a
//! processor, and only a home screen waits for it), and kept: sixty pictures of 20–60 kB at most.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use app::launch::Picture;
use axum::body::Bytes;
use resvg::{tiny_skia, usvg};
use tokio::sync::Semaphore;

use crate::{cards, logo};

/// The page's background and its text, as in `folia/assets/app.css` (`--bg`, `--text`, `--text-3`).
const LIGHT: (&str, &str, &str) = ("#f1f2f4", "#10151f", "#8790a0");
const DARK: (&str, &str, &str) = ("#0a0c11", "#eceff5", "#6a748a");

pub struct Launch {
    kept: Mutex<HashMap<Picture, Bytes>>,
    drawing: Semaphore,
}

impl Default for Launch {
    fn default() -> Self {
        Self { kept: Mutex::default(), drawing: Semaphore::new(1) }
    }
}

impl Launch {
    fn kept(&self, picture: &Picture) -> Option<Bytes> {
        self.kept.lock().ok()?.get(picture).cloned()
    }

    /// The PNG of `picture`: kept, or drawn now.
    pub async fn get(&self, picture: Picture) -> Result<Bytes, String> {
        if let Some(png) = self.kept(&picture) {
            return Ok(png);
        }
        let _turn = self.drawing.acquire().await.map_err(|error| error.to_string())?;
        // Drawn while this one waited for its turn.
        if let Some(png) = self.kept(&picture) {
            return Ok(png);
        }
        let started = std::time::Instant::now();
        let png = Bytes::from(tokio::task::spawn_blocking(move || draw(picture)).await.map_err(|error| error.to_string())??);
        tracing::debug!(component = "launch", event = "launch.drawn", file = picture.file(), bytes = png.len(), ms = started.elapsed().as_millis() as u64, "drew a launch screen");
        if let Ok(mut kept) = self.kept.lock() {
            kept.insert(picture, png.clone());
        }
        Ok(png)
    }
}

/// The picture in CSS pixels; `draw` scales it to the device's.
fn svg(picture: &Picture, regular: &rustybuzz::Face<'_>) -> String {
    let (width, height) = picture.points();
    let (w, h) = (width as f32, height as f32);
    let (background, text, quiet) = if picture.dark { DARK } else { LIGHT };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"Inter\"><rect width=\"{w}\" height=\"{h}\" fill=\"{background}\"/>"
    );
    // The icon in the middle, as large as twice an icon of the home screen on a phone (a quarter
    // of the shorter side), no larger on a tablet.
    let size = (0.28 * w.min(h)).clamp(96.0, 128.0);
    let (x, y) = ((w - size) / 2.0, (h - size) / 2.0);
    if !picture.dark {
        // On the light page it lies on a soft shadow, like the panels of the app.
        svg.push_str(&format!(
            "<filter id=\"shadow\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"{}\"/></filter>\
             <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"#0f172a\" opacity=\".2\" filter=\"url(#shadow)\"/>",
            size * 0.07,
            x + size * 0.08,
            y + size * 0.1,
            size * 0.84,
            size * 0.9,
            size * 0.22
        ));
    }
    logo::app_icon(&mut svg, x, y, size, "icon");
    // At the bottom, clear of the home indicator: the wordmark, and under it what Betula is.
    let last = h - (0.075 * h).max(44.0);
    let em = 28.0;
    logo::wordmark(&mut svg, (w - logo::wordmark_width(regular, em)) / 2.0, last - 24.0, em, text);
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{last}\" text-anchor=\"middle\" font-size=\"13\" font-weight=\"500\" letter-spacing=\"-.08\" fill=\"{quiet}\">Modulkatalog · inoffiziell</text></svg>",
        w / 2.0
    ));
    svg
}

/// The cuts of Inter the cards are set in (`cards`), loaded once, when the first picture is drawn.
fn typeface() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            for cut in [cards::INTER_400, cards::INTER_500, cards::INTER_600, cards::INTER_800] {
                fonts.load_font_data(cut.to_vec());
            }
            fonts.set_sans_serif_family("Inter");
            Arc::new(fonts)
        })
        .clone()
}

fn draw(picture: Picture) -> Result<Vec<u8>, String> {
    let regular = rustybuzz::Face::from_slice(cards::INTER_400, 0).ok_or("the typeface of the launch screens cannot be read")?;
    let options = usvg::Options { fontdb: typeface(), font_family: "Inter".to_string(), ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(&svg(&picture, &regular), &options).map_err(|error| error.to_string())?;
    let (width, height) = picture.pixels();
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("no pixmap")?;
    let ratio = f32::from(picture.screen.ratio);
    resvg::render(&tree, tiny_skia::Transform::from_scale(ratio, ratio), &mut pixmap.as_mut());
    encode(&pixmap)
}

/// A true-colour PNG without alpha: the background fills the picture, so premultiplied colours
/// are the plain ones. Most rows are one colour, which the PNG filter makes a row of zeros.
fn encode(pixmap: &tiny_skia::Pixmap) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity((pixmap.width() * pixmap.height() * 3) as usize);
    for &[red, green, blue, _] in pixmap.data().as_chunks::<4>().0 {
        rgb.extend_from_slice(&[red, green, blue]);
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Default);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer.write_image_data(&rgb).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(png: &[u8]) -> (u32, u32) {
        (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()))
    }

    /// A picture is drawn once, as large as its screen, and kept.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_launch_screen_is_drawn_once_as_large_as_its_screen() {
        let launch = Launch::default();
        let picture = Picture::from_file("1179x2556-dark.png").unwrap();
        let png = launch.get(picture).await.unwrap();
        assert!(png.starts_with(b"\x89PNG") && size(&png) == (1179, 2556), "{:?}", size(&png));
        assert!(png.len() < 120 * 1024, "{} bytes", png.len());
        assert_eq!(launch.get(picture).await.unwrap(), png, "kept");
        let tablet = launch.get(Picture::from_file("2388x1668.png").unwrap()).await.unwrap();
        assert_eq!(size(&tablet), (2388, 1668));
    }

    /// Every picture, into a directory to look at:
    /// `FOLIA_LAUNCH_OUT=<dir> cargo test -p folia-server launch_screens_for_review -- --nocapture`.
    #[test]
    fn launch_screens_for_review() {
        let Ok(dir) = std::env::var("FOLIA_LAUNCH_OUT") else { return };
        std::fs::create_dir_all(&dir).unwrap();
        for picture in Picture::all() {
            let started = std::time::Instant::now();
            let png = draw(picture).unwrap();
            println!("{}: {} bytes, {} ms", picture.file(), png.len(), started.elapsed().as_millis());
            std::fs::write(format!("{dir}/{}", picture.file()), png).unwrap();
        }
    }
}
