//! Pictures for link previews (Open Graph), one per module and per program: 1200 × 630, drawn in
//! this process by resvg from an SVG that is put together here, set in the app's typeface.
//!
//! - **What a card says** comes from the snapshot (`CardText`): the kind and number, the title, a
//!   few facts. The logo is drawn as in `design/logo/logo.html` (same measured spacing).
//! - **Kept:** a finished card stays in memory (`--card-cache-mb`), under the hash of its text.
//!   Within one snapshot a kept card is answered without a look into the database; after a new
//!   snapshot its text is read once, and a card that still says the same is reused, so only
//!   changed ones are drawn again. Browsers and the fetchers of messengers get an ETag.
//! - **Never in the way:** drawing is CPU work, so it runs on the blocking pool and only a few at
//!   a time (`permits`). When all are busy the answer is the site's standard picture, at once
//!   (before anything is read) and marked `no-store`, so the next fetch gets the real card. A
//!   preview is never worth a slow page.
//!
//! The typeface: static cuts of Inter (`server/assets/inter-*.ttf`, made once by
//! `design/cards/make-fonts.py` from the app's variable font; resvg reads plain TrueType).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use resvg::{tiny_skia, usvg};
use tokio::sync::Semaphore;

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 630.0;
/// The text column: from the left edge of the logo to the same distance from the right.
const LEFT: f32 = 104.0;
const RIGHT: f32 = 1096.0;
/// Between the logo row and the footer row.
const BODY_TOP: f32 = 184.0;
const BODY_BOTTOM: f32 = 486.0;

const INK: &str = "#10151f";
const INK_2: &str = "#4b5565";
const INK_3: &str = "#8790a0";

pub static INTER_400: &[u8] = include_bytes!("../assets/inter-400.ttf");
static INTER_500: &[u8] = include_bytes!("../assets/inter-500.ttf");
static INTER_600: &[u8] = include_bytes!("../assets/inter-600.ttf");
static INTER_800: &[u8] = include_bytes!("../assets/inter-800.ttf");

/// The typeface resvg sets the pictures of the server in (the cards, the launch screens): the
/// four cuts of Inter, loaded once.
pub fn typeface() -> Arc<usvg::fontdb::Database> {
    static FONTS: std::sync::OnceLock<Arc<usvg::fontdb::Database>> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            for cut in [INTER_400, INTER_500, INTER_600, INTER_800] {
                fonts.load_font_data(cut.to_vec());
            }
            fonts.set_sans_serif_family("Inter");
            Arc::new(fonts)
        })
        .clone()
}

/// What a card says. Its hash is the card's identity in the cache and its ETag.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CardText {
    /// „Modul 11101", „Studiengang".
    pub eyebrow: String,
    pub title: String,
    /// Short facts in the order of their weight; those that do not fit are left out from the end.
    pub facts: Vec<String>,
    /// One more line in a quieter colour (the department, the size of the curriculum); cut to fit.
    pub note: Option<String>,
}

pub enum Card {
    /// The picture and its ETag.
    Drawn(String, Bytes),
    /// There is no such module or program.
    Unknown,
    /// Every drawing place is taken: answer with the standard picture.
    Busy,
}

struct Entry {
    hash: u64,
    /// The snapshot (`SnapshotStore::generation`) this card was last checked against.
    generation: u64,
    png: Bytes,
    used: u64,
}

fn etag(hash: u64) -> String {
    format!("\"card-{hash:016x}\"")
}

#[derive(Default)]
struct Kept {
    entries: HashMap<String, Entry>,
    bytes: usize,
    clock: u64,
}

pub struct Cards {
    fonts: Arc<usvg::fontdb::Database>,
    kept: Mutex<Kept>,
    budget: usize,
    permits: Arc<Semaphore>,
    /// Cards not drawn because every place was taken, since the last log line about it.
    turned_away: AtomicU64,
    last_report: AtomicU64,
}

impl Cards {
    /// `budget`: bytes of finished cards to keep. `places`: cards drawn at the same time.
    pub fn new(budget: usize, places: usize) -> Self {
        Self {
            fonts: typeface(),
            kept: Mutex::default(),
            budget,
            permits: Arc::new(Semaphore::new(places)),
            turned_away: AtomicU64::new(0),
            last_report: AtomicU64::new(0),
        }
    }

    /// As many places as half the processors, at least one, at most four: the pages come first.
    pub fn places_for_this_machine() -> usize {
        std::thread::available_parallelism().map(|n| (n.get() / 2).clamp(1, 4)).unwrap_or(1)
    }

    /// A kept card, if `fresh` says it may be answered as it is.
    fn kept_card(&self, key: &str, fresh: impl FnOnce(&mut Entry) -> bool) -> Option<Card> {
        let mut kept = self.kept.lock().ok()?;
        kept.clock += 1;
        let now = kept.clock;
        let entry = kept.entries.get_mut(key)?;
        if !fresh(entry) {
            return None;
        }
        entry.used = now;
        Some(Card::Drawn(etag(entry.hash), entry.png.clone()))
    }

    /// The card under `key` (`m:<id>`, `p:<slug>`) in the snapshot `generation`. `text` reads
    /// what the card says; it is only asked when the card is not known for this snapshot and
    /// a drawing place is free.
    pub async fn get(&self, key: &str, generation: u64, text: impl FnOnce() -> Result<Option<CardText>, String>) -> Result<Card, String> {
        if let Some(card) = self.kept_card(key, |entry| entry.generation == generation) {
            return Ok(card);
        }
        let Ok(place) = self.permits.clone().try_acquire_owned() else {
            self.report_turned_away();
            return Ok(Card::Busy);
        };
        let Some(text) = text()? else { return Ok(Card::Unknown) };
        let hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            hasher.finish()
        };
        // A new snapshot that says the same: the card stands, now for this snapshot too.
        let same = |entry: &mut Entry| {
            let same = entry.hash == hash;
            if same {
                entry.generation = generation;
            }
            same
        };
        if let Some(card) = self.kept_card(key, same) {
            return Ok(card);
        }
        let fonts = self.fonts.clone();
        let started = std::time::Instant::now();
        let drawn = tokio::task::spawn_blocking(move || {
            let _place = place;
            draw(&text, fonts)
        })
        .await
        .map_err(|error| error.to_string())??;
        let png = Bytes::from(drawn);
        tracing::debug!(component = "cards", event = "card.drawn", key, bytes = png.len(), ms = started.elapsed().as_millis() as u64, "drew a link-preview card");
        self.keep(key, hash, generation, png.clone());
        Ok(Card::Drawn(etag(hash), png))
    }

    fn keep(&self, key: &str, hash: u64, generation: u64, png: Bytes) {
        let Ok(mut kept) = self.kept.lock() else { return };
        if png.len() > self.budget {
            return;
        }
        kept.clock += 1;
        let used = kept.clock;
        kept.bytes += png.len();
        if let Some(old) = kept.entries.insert(key.to_string(), Entry { hash, generation, png, used }) {
            kept.bytes = kept.bytes.saturating_sub(old.png.len());
        }
        // Over the budget: the cards that were asked for longest ago go first.
        while kept.bytes > self.budget {
            let Some(oldest) = kept.entries.iter().min_by_key(|(_, entry)| entry.used).map(|(key, _)| key.clone()) else { break };
            if let Some(gone) = kept.entries.remove(&oldest) {
                kept.bytes = kept.bytes.saturating_sub(gone.png.len());
            }
        }
    }

    /// One line a minute at most, with the count: a burst of previews must not flood the log.
    fn report_turned_away(&self) {
        let waiting = self.turned_away.fetch_add(1, Ordering::Relaxed) + 1;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let last = self.last_report.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= 60 && self.last_report.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            self.turned_away.store(0, Ordering::Relaxed);
            tracing::warn!(component = "cards", event = "card.busy", count = waiting, "every drawing place was taken: answered with the standard picture");
        }
    }

    #[cfg(test)]
    pub fn kept(&self) -> (usize, usize) {
        let kept = self.kept.lock().unwrap();
        (kept.entries.len(), kept.bytes)
    }
}

// ---------- drawing ----------

/// Measures text the way resvg will set it (same shaper, same fonts).
struct Ruler {
    regular: rustybuzz::Face<'static>,
    heavy: rustybuzz::Face<'static>,
}

impl Ruler {
    fn new() -> Option<Self> {
        Some(Self { regular: rustybuzz::Face::from_slice(INTER_500, 0)?, heavy: rustybuzz::Face::from_slice(INTER_800, 0)? })
    }

    /// Width of `text` at `size`, with `spacing` (em) after every character.
    fn width(face: &rustybuzz::Face<'_>, text: &str, size: f32, spacing: f32) -> f32 {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let shaped = rustybuzz::shape(face, &[], buffer);
        let advance: i32 = shaped.glyph_positions().iter().map(|glyph| glyph.x_advance).sum();
        advance as f32 * size / face.units_per_em() as f32 + spacing * size * text.chars().count() as f32
    }
}

const TITLE_SPACING: f32 = -0.025;
/// Title sizes to try, largest first: (size, line height, lines at most).
const TITLE_CUTS: [(f32, f32, usize); 4] = [(68.0, 76.0, 2), (58.0, 66.0, 3), (50.0, 58.0, 3), (42.0, 50.0, 4)];

/// Greedy lines of at most `width`; `None` if a single word is wider.
fn wrap(face: &rustybuzz::Face<'_>, text: &str, size: f32, width: f32) -> Option<Vec<String>> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if Ruler::width(face, word, size, TITLE_SPACING) > width {
            return None;
        }
        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if Ruler::width(face, &candidate, size, TITLE_SPACING) <= width {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    Some(lines)
}

/// `text` cut at a character so that it fits `width` with „…" at its end.
fn fit(face: &rustybuzz::Face<'_>, text: &str, size: f32, spacing: f32, width: f32) -> String {
    if Ruler::width(face, text, size, spacing) <= width {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while cut.pop().is_some() {
        let candidate = format!("{} …", cut.trim_end_matches([' ', ',', ';', ':', '-', '–', '.']));
        if Ruler::width(face, &candidate, size, spacing) <= width {
            return candidate;
        }
    }
    "…".to_string()
}

/// The title in the largest cut it fits; in the smallest one its last line is cut short.
fn title_lines(ruler: &Ruler, title: &str) -> (f32, f32, Vec<String>) {
    let width = RIGHT - LEFT;
    for (size, leading, most) in TITLE_CUTS {
        if let Some(lines) = wrap(&ruler.heavy, title, size, width).filter(|lines| lines.len() <= most) {
            return (size, leading, lines);
        }
    }
    let (size, leading, most) = TITLE_CUTS.last().copied().unwrap_or((42.0, 50.0, 4));
    // Too many lines even in the smallest cut: keep the first ones, the last ends in „…".
    if let Some(mut lines) = wrap(&ruler.heavy, title, size, width) {
        lines.truncate(most);
        if let Some(last) = lines.last_mut() {
            *last = fit(&ruler.heavy, &format!("{last} …"), size, TITLE_SPACING, width);
        }
        return (size, leading, lines);
    }
    // A word wider than the column: break where the line is full.
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for ch in title.split_whitespace().collect::<Vec<_>>().join(" ").chars() {
        let mut candidate = line.clone();
        candidate.push(ch);
        if Ruler::width(&ruler.heavy, &candidate, size, TITLE_SPACING) > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line).trim().to_string());
            if lines.len() == most {
                break;
            }
            line = ch.to_string();
        } else {
            line = candidate;
        }
    }
    if lines.len() < most && !line.trim().is_empty() {
        lines.push(line.trim().to_string());
    } else if let Some(last) = lines.last_mut() {
        *last = fit(&ruler.heavy, &format!("{last} …"), size, TITLE_SPACING, width);
    }
    (size, leading, lines)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The logo at (x, y of its top): the mark and the wordmark beside it (`crate::logo`).
fn logo(svg: &mut String, x: f32, y: f32) {
    let mark = 52.0;
    crate::logo::mark(svg, x, y, mark);
    let size = 44.0;
    // Capitals are .728 em high: their middle on the middle of the mark.
    crate::logo::wordmark(svg, x + mark + 18.0, y + mark / 2.0 + size * 0.364, size, INK);
}

fn svg(text: &CardText, ruler: &Ruler) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" font-family=\"Inter\">\
         <rect width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"#f1f2f4\"/>\
         <rect x=\"40.5\" y=\"40.5\" width=\"1119\" height=\"549\" rx=\"22\" fill=\"#fff\" stroke=\"{INK}\" stroke-opacity=\".08\"/>"
    );
    logo(&mut svg, LEFT, 92.0);
    svg.push_str(&format!(
        "<text x=\"{RIGHT}\" y=\"134\" text-anchor=\"end\" font-size=\"26\" font-weight=\"600\" fill=\"{INK_2}\" letter-spacing=\"-.3\">{}</text>",
        escape(&fit(&ruler.regular, &text.eyebrow, 26.0, -0.012, 560.0))
    ));

    // The body: title, facts, note; as a block in the middle between logo and footer.
    let width = RIGHT - LEFT;
    let (size, leading, lines) = title_lines(ruler, &text.title);
    let mut facts: Vec<&str> = text.facts.iter().map(String::as_str).filter(|fact| !fact.trim().is_empty()).collect();
    while facts.len() > 1 && Ruler::width(&ruler.regular, &facts.join(" · "), 30.0, -0.012) > width {
        facts.pop();
    }
    let facts = fit(&ruler.regular, &facts.join(" · "), 30.0, -0.012, width);
    let note = text.note.as_deref().map(str::trim).filter(|note| !note.is_empty()).map(|note| fit(&ruler.regular, note, 25.0, -0.012, width));
    let height = lines.len() as f32 * leading + if facts.is_empty() { 0.0 } else { 22.0 + 36.0 } + if note.is_some() { 12.0 + 32.0 } else { 0.0 };
    let mut y = (BODY_TOP + ((BODY_BOTTOM - BODY_TOP) - height) / 2.0).max(BODY_TOP);
    for line in &lines {
        // The baseline sits about .8 of the size below the top of its line.
        svg.push_str(&format!(
            "<text x=\"{LEFT}\" y=\"{}\" font-size=\"{size}\" font-weight=\"800\" fill=\"{INK}\" letter-spacing=\"{}\">{}</text>",
            y + (leading - size) / 2.0 + size * 0.8,
            TITLE_SPACING * size,
            escape(line)
        ));
        y += leading;
    }
    if !facts.is_empty() {
        y += 22.0;
        svg.push_str(&format!("<text x=\"{LEFT}\" y=\"{}\" font-size=\"30\" font-weight=\"500\" fill=\"{INK_2}\" letter-spacing=\"-.36\">{}</text>", y + 28.0, escape(&facts)));
        y += 36.0;
    }
    if let Some(note) = note {
        y += 12.0;
        svg.push_str(&format!("<text x=\"{LEFT}\" y=\"{}\" font-size=\"25\" font-weight=\"500\" fill=\"{INK_3}\" letter-spacing=\"-.3\">{}</text>", y + 24.0, escape(&note)));
    }

    svg.push_str(&format!(
        "<text x=\"{LEFT}\" y=\"546\" font-size=\"22\" letter-spacing=\"-.26\"><tspan font-weight=\"600\" fill=\"{INK}\">betula.app</tspan>\
         <tspan font-weight=\"500\" fill=\"{INK_3}\"> · </tspan><tspan font-weight=\"500\" fill=\"{INK_2}\">Modulkatalog</tspan>\
         <tspan font-weight=\"500\" fill=\"{INK_3}\"> · </tspan><tspan font-weight=\"500\" fill=\"{INK_2}\">inoffiziell</tspan></text>\
         <text x=\"{RIGHT}\" y=\"546\" text-anchor=\"end\" font-size=\"22\" font-weight=\"500\" fill=\"{INK_3}\" letter-spacing=\"-.26\">BTU Cottbus-Senftenberg</text></svg>"
    ));
    svg
}

fn draw(text: &CardText, fonts: Arc<usvg::fontdb::Database>) -> Result<Vec<u8>, String> {
    let ruler = Ruler::new().ok_or("the typeface of the cards cannot be read")?;
    let options = usvg::Options { fontdb: fonts, font_family: "Inter".to_string(), ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(&svg(text, &ruler), &options).map_err(|error| error.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH as u32, HEIGHT as u32).ok_or("no pixmap")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    encode(&pixmap)
}

/// The colours a card is made of: white towards each ink and towards the page's grey, in 64
/// steps each (what antialiasing produces), so the picture fits a palette of 256 colours. A
/// palette PNG is a fifth of the size of the true-colour one and is packed faster.
fn palette() -> Vec<[u8; 3]> {
    let white = [255.0, 255.0, 255.0];
    let mut colours = Vec::with_capacity(256);
    for target in [[0x10u8, 0x15, 0x1f], [0x4b, 0x55, 0x65], [0x87, 0x90, 0xa0], [0xf1, 0xf2, 0xf4]] {
        for step in 0..64 {
            let share = step as f32 / 63.0;
            let mut colour = [0u8; 3];
            for (out, (from, to)) in colour.iter_mut().zip(white.iter().zip(target)) {
                *out = (from + (f32::from(to) - from) * share).round() as u8;
            }
            colours.push(colour);
        }
    }
    colours
}

fn encode(pixmap: &tiny_skia::Pixmap) -> Result<Vec<u8>, String> {
    let palette = palette();
    // The card has a few hundred different colours: look each one up once.
    let mut nearest: HashMap<[u8; 3], u8> = HashMap::new();
    let mut indexed = Vec::with_capacity((pixmap.width() * pixmap.height()) as usize);
    // Most pixels repeat their neighbour (white, the grey around the card): ask the table only
    // when the colour changes.
    let mut last = ([0u8; 3], 0u8, false);
    for &[red, green, blue, _] in pixmap.data().as_chunks::<4>().0 {
        // Opaque everywhere (the background fills the picture), so premultiplied = plain.
        let colour = [red, green, blue];
        if !(last.2 && last.0 == colour) {
            let index = *nearest.entry(colour).or_insert_with(|| {
                let distance = |entry: &[u8; 3]| entry.iter().zip(colour).map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2)).sum::<i32>();
                palette.iter().enumerate().min_by_key(|(_, entry)| distance(entry)).map(|(index, _)| index as u8).unwrap_or(0)
            });
            last = (colour, index, true);
        }
        indexed.push(last.1);
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette.concat());
    encoder.set_compression(png::Compression::Default);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer.write_image_data(&indexed).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(title: &str) -> CardText {
        CardText { eyebrow: "Modul 11101".into(), title: title.into(), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into()], note: Some("Fakultät 1".into()) }
    }

    fn size(png: &[u8]) -> (u32, u32) {
        (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()))
    }

    #[tokio::test]
    async fn a_card_is_drawn_once_and_kept_until_its_text_changes() {
        let cards = Cards::new(8 * 1024 * 1024, 1);
        let title = "Lineare Algebra und analytische Geometrie I";
        let Card::Drawn(etag, png) = cards.get("m:11101", 1, || Ok(Some(text(title)))).await.unwrap() else { panic!("not drawn") };
        assert!(png.starts_with(b"\x89PNG") && size(&png) == (1200, 630));
        // Same snapshot: answered from memory, the text is not even read.
        let Card::Drawn(again, same) = cards.get("m:11101", 1, || panic!("a kept card needs no text")).await.unwrap() else { panic!("not kept") };
        assert!(etag == again && same == png && cards.kept().0 == 1);
        // A new snapshot that says the same: read once, not drawn again (no place is needed twice).
        let Card::Drawn(next, same) = cards.get("m:11101", 2, || Ok(Some(text(title)))).await.unwrap() else { panic!("not kept") };
        assert!(etag == next && same == png);
        let Card::Drawn(..) = cards.get("m:11101", 2, || panic!("known for this snapshot now")).await.unwrap() else { panic!("not kept") };
        // A snapshot that says something else: a new card replaces the old one.
        let Card::Drawn(changed, _) = cards.get("m:11101", 3, || Ok(Some(text("Lineare Algebra II")))).await.unwrap() else { panic!("not drawn") };
        assert!(changed != etag && cards.kept().0 == 1);
        assert!(matches!(cards.get("m:0", 3, || Ok(None)).await.unwrap(), Card::Unknown));
    }

    #[tokio::test]
    async fn without_a_free_place_the_answer_is_busy_and_the_budget_holds() {
        let none = Cards::new(8 * 1024 * 1024, 0);
        assert!(matches!(none.get("m:1", 1, || panic!("a busy server reads nothing")).await.unwrap(), Card::Busy));

        let small = Cards::new(1, 1);
        assert!(matches!(small.get("m:1", 1, || Ok(Some(text("Analysis I")))).await.unwrap(), Card::Drawn(..)));
        assert_eq!(small.kept(), (0, 0), "a card larger than the budget is not kept");
    }

    /// For looking at the design: `FOLIA_CARD_OUT=<dir> cargo test -p folia-server cards_for_review`.
    #[test]
    fn cards_for_review() {
        let Ok(dir) = std::env::var("FOLIA_CARD_OUT") else { return };
        let fonts = Cards::new(0, 1).fonts.clone();
        let samples = [
            ("short", CardText { eyebrow: "Modul 11103".into(), title: "Analysis I".into(), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into(), "MAP".into()], note: Some("Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik".into()) }),
            ("two-lines", CardText { eyebrow: "Modul 11101".into(), title: "Lineare Algebra und analytische Geometrie I".into(), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into(), "Vorleistung + MAP".into()], note: Some("Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik".into()) }),
            ("long", CardText { eyebrow: "Modul 13849".into(), title: "Advanced Geophysical Methods in Natural Resource Investigation and Environmental Monitoring of Post-Mining Landscapes (ANRI)".into(), facts: vec!["wird nicht mehr angeboten".into(), "6 LP".into(), "jedes Semester".into(), "Englisch".into(), "MCA".into()], note: None }),
            ("program", CardText { eyebrow: "Studiengang".into(), title: "Informatik".into(), facts: vec!["B.Sc.".into(), "Prüfungsordnung 2021".into()], note: Some("64 Module im Curriculum  ·  mit Regelstudienplan".into()) }),
            ("program-long", CardText { eyebrow: "Studiengang".into(), title: "Umweltingenieurwesen – Verfahrenstechnik und Kreislaufwirtschaft".into(), facts: vec!["M.Sc.".into(), "dual, praxisintegrierend".into(), "Prüfungsordnung 2019".into()], note: Some("38 Module im Curriculum".into()) }),
        ];
        for (name, text) in samples {
            let started = std::time::Instant::now();
            let png = draw(&text, fonts.clone()).unwrap();
            println!("{name}: {} bytes, {} ms", png.len(), started.elapsed().as_millis());
            std::fs::write(format!("{dir}/card-{name}.png"), png).unwrap();
        }
    }

    #[test]
    fn long_titles_shrink_wrap_and_end_in_an_ellipsis() {
        let ruler = Ruler::new().unwrap();
        let (size, _, lines) = title_lines(&ruler, "Analysis I");
        assert!(size == 68.0 && lines == ["Analysis I"]);
        let long = "Ausgewählte Kapitel der Siedlungswasserwirtschaft und des Gewässerschutzes unter besonderer Berücksichtigung der Bergbaufolgelandschaften der Lausitz einschließlich ihrer wasserrechtlichen Grundlagen und Verfahren";
        let (size, _, lines) = title_lines(&ruler, long);
        assert!(size == 42.0 && lines.len() == 4 && lines[3].ends_with('…'), "{lines:?}");
        assert!(lines.iter().all(|line| Ruler::width(&ruler.heavy, line, size, TITLE_SPACING) <= RIGHT - LEFT));
        let (_, _, lines) = title_lines(&ruler, &"Donaudampfschifffahrtsgesellschaftskapitänsmützenabzeichen".repeat(3));
        assert!(!lines.is_empty() && lines.iter().all(|line| Ruler::width(&ruler.heavy, line, 42.0, TITLE_SPACING) <= RIGHT - LEFT));
    }
}
