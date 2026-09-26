//! Pictures for link previews (Open Graph): 1200 × 630, drawn in this process by resvg from an
//! SVG that is put together here, set in the app's typeface. One per module and per program, one
//! for the Merkliste and for the Stundenplan, and one per shared Stundenplan with its modules.
//!
//! - **What a card says** comes from the snapshot or the page (`CardText`): the kind and number,
//!   the title (or a shared plan's modules as tags), a few facts. The logo is drawn as in
//!   `design/logo/logo.html` (same measured spacing).
//! - **The birch:** the crown hangs along the top of the card as it hangs along the top of the
//!   site (docs/frontend.md „The birch"), in the season the card is drawn in (`birch::Season`), so
//!   a card of October is golden and one of January bare.
//! - **Kept:** a finished card stays in memory (`--card-cache-mb`), under the hash of its text and
//!   season. Within one snapshot a kept card is answered without a look into the database; after a
//!   new snapshot its text is read once, and a card that still says the same is reused, so only
//!   changed ones are drawn again; a new season draws them anew. Browsers and the fetchers of
//!   messengers get an ETag.
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

use crate::birch::{self, Season};

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 630.0;
/// The card's white face on the grey of the picture.
const FACE_X: f32 = 40.0;
const FACE_Y: f32 = 40.0;
const FACE_WIDTH: f32 = 1120.0;
const FACE_HEIGHT: f32 = 550.0;
const FACE_RADIUS: f32 = 22.0;
/// The text column: from the left edge of the logo to the same distance from the right.
const LEFT: f32 = 104.0;
const RIGHT: f32 = 1096.0;
/// Between the logo row and the footer row.
const BODY_TOP: f32 = 184.0;
const BODY_BOTTOM: f32 = 486.0;
/// The crown on a card is half as large again as along the site's top: a preview is seen small.
const CROWN_SCALE: f32 = 1.5;

const INK: &str = "#10151f";
const INK_2: &str = "#4b5565";
const INK_3: &str = "#8790a0";
const GREY: &str = "#f1f2f4";

// The launch screens of iOS (`launch`) are set in the same cuts.
pub(crate) static INTER_400: &[u8] = include_bytes!("../assets/inter-400.ttf");
pub(crate) static INTER_500: &[u8] = include_bytes!("../assets/inter-500.ttf");
pub(crate) static INTER_600: &[u8] = include_bytes!("../assets/inter-600.ttf");
pub(crate) static INTER_800: &[u8] = include_bytes!("../assets/inter-800.ttf");

/// What a card says. Its hash, with the season, is the card's identity in the cache and its ETag.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CardText {
    /// „Modul 11101", „Studiengang", „Stundenplan · WiSe 2026/27".
    pub eyebrow: String,
    /// What the card is about, set large.
    pub headline: Headline,
    /// Short facts in the order of their weight; those that do not fit are left out from the end.
    pub facts: Vec<String>,
    /// One more line in a quieter colour (the department, the size of the curriculum); cut to fit.
    pub note: Option<String>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Headline {
    /// A title: at most four lines, as large as it fits.
    Title(String),
    /// Short names as tags in the tones of the Stundenplan, in its order (a shared plan's modules:
    /// „MIT-1", „AuP"); those that do not fit are counted in a last tag („+3").
    Tags(Vec<String>),
}

/// The beginning of the key of a shared Studienplan's card, which the code follows.
pub const SHARED_PLAN: &str = "s:";

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
    season: Season,
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
        let mut fonts = usvg::fontdb::Database::new();
        for cut in [INTER_400, INTER_500, INTER_600, INTER_800] {
            fonts.load_font_data(cut.to_vec());
        }
        fonts.set_sans_serif_family("Inter");
        Self {
            fonts: Arc::new(fonts),
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

    /// The card under `key` (`m:<id>`, `p:<slug>`, `s:<code>`, …) in the snapshot `generation`,
    /// drawn in `season`. `text` reads what the card says; it is only asked when the card is not
    /// known for this snapshot and season and a drawing place is free.
    pub async fn get(&self, key: &str, generation: u64, season: Season, text: impl FnOnce() -> Result<Option<CardText>, String>) -> Result<Card, String> {
        if let Some(card) = self.kept_card(key, |entry| entry.generation == generation && entry.season == season) {
            return Ok(card);
        }
        let Ok(place) = self.permits.clone().try_acquire_owned() else {
            self.report_turned_away();
            return Ok(Card::Busy);
        };
        let Some(text) = text()? else { return Ok(Card::Unknown) };
        let hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (&text, season).hash(&mut hasher);
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
            draw(&text, season, fonts)
        })
        .await
        .map_err(|error| error.to_string())??;
        let png = Bytes::from(drawn);
        // A shared Studienplan's key is its code, which names someone's modules: not for the log.
        let logged = if key.starts_with(SHARED_PLAN) { SHARED_PLAN } else { key };
        tracing::debug!(component = "cards", event = "card.drawn", key = logged, bytes = png.len(), ms = started.elapsed().as_millis() as u64, "drew a link-preview card");
        self.keep(key, Entry { hash, generation, season, png: png.clone(), used: 0 });
        Ok(Card::Drawn(etag(hash), png))
    }

    fn keep(&self, key: &str, mut entry: Entry) {
        let Ok(mut kept) = self.kept.lock() else { return };
        if entry.png.len() > self.budget {
            return;
        }
        kept.clock += 1;
        entry.used = kept.clock;
        kept.bytes += entry.png.len();
        if let Some(old) = kept.entries.insert(key.to_string(), entry) {
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

/// A list („A · B · C") that does not fit is cut after its last whole item that does, and ends in
/// „· …"; where not even the first fits, or there is no list, it is cut at a character (`fit`).
fn fit_list(face: &rustybuzz::Face<'_>, text: &str, size: f32, spacing: f32, width: f32) -> String {
    if Ruler::width(face, text, size, spacing) <= width {
        return text.to_string();
    }
    let items: Vec<&str> = text.split(LIST).collect();
    let mut shown = items.len().saturating_sub(1);
    while shown > 0 {
        let candidate = format!("{}{LIST}…", items.get(..shown).unwrap_or_default().join(LIST));
        if Ruler::width(face, &candidate, size, spacing) <= width {
            return candidate;
        }
        shown -= 1;
    }
    fit(face, text, size, spacing, width)
}

/// Between the items of a list on a card.
const LIST: &str = " · ";

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

// ---------- tags ----------

/// The tones of the Stundenplan's modules in the order they were planned (`t-ice` … `t-slate` in
/// app/assets/app.css, light theme; `studyplan::head::hue`): a tag's bar and its ground, as the
/// plan's legend draws a module's key (`.sp-key`: the ground is 16 % of the tone on white).
const TONES: [(&str, &str); 8] = [
    ("#3a78a1", "#dfe9f0"),
    ("#b68028", "#f4eade"),
    ("#755d9d", "#e7e4f0"),
    ("#2f837f", "#dfebe9"),
    ("#458554", "#e1ebe2"),
    ("#bc634d", "#f6e6e1"),
    ("#a25a77", "#f1e4e8"),
    ("#647289", "#e5e7ec"),
];

const TAG_SPACING: f32 = -0.01;
/// Tag sizes to try, largest first: (size of the text, height of a tag, rows at most). A tag's
/// padding, bar, corners and the gaps between tags grow with the size.
const TAG_CUTS: [(f32, f32, usize); 3] = [(44.0, 76.0, 2), (36.0, 62.0, 3), (30.0, 52.0, 3)];

/// A tag as it is set: the tone it has (`None`: the count of those left out), its text and width.
#[derive(Clone, Debug, PartialEq)]
struct Tag {
    tone: Option<usize>,
    text: String,
    width: f32,
}

fn tag_width(ruler: &Ruler, text: &str, size: f32) -> f32 {
    // Bar and space before the text, space after it: 30 and 26 at the largest size.
    Ruler::width(&ruler.heavy, text, size, TAG_SPACING) + size * (56.0 / 44.0)
}

fn tag_gap(size: f32) -> f32 {
    size * (14.0 / 44.0)
}

/// Tags into rows of at most the column's width, in their order.
fn tag_rows(tags: &[Tag], size: f32) -> Vec<Vec<Tag>> {
    let (width, gap) = (RIGHT - LEFT, tag_gap(size));
    let mut rows: Vec<Vec<Tag>> = Vec::new();
    let mut used = 0.0;
    for tag in tags {
        match rows.last_mut() {
            Some(row) if used + gap + tag.width <= width => {
                used += gap + tag.width;
                row.push(tag.clone());
            }
            _ => {
                used = tag.width;
                rows.push(vec![tag.clone()]);
            }
        }
    }
    rows
}

/// The tags in the largest cut whose rows they fit; in the smallest, what does not fit is counted
/// in a last tag („+3"). A single tag wider than the column ends in „…". Returns the cut and the
/// rows.
fn tag_layout(ruler: &Ruler, names: &[String]) -> ((f32, f32, usize), Vec<Vec<Tag>>) {
    let width = RIGHT - LEFT;
    let tags = |size: f32| -> Vec<Tag> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let room = width - tag_width(ruler, "", size);
                let text = fit(&ruler.heavy, name.trim(), size, TAG_SPACING, room);
                Tag { tone: Some(index % TONES.len()), width: tag_width(ruler, &text, size), text }
            })
            .collect()
    };
    for cut in TAG_CUTS {
        let rows = tag_rows(&tags(cut.0), cut.0);
        if rows.len() <= cut.2 {
            return (cut, rows);
        }
    }
    let cut = TAG_CUTS.last().copied().unwrap_or((30.0, 52.0, 3));
    let (size, _, most) = cut;
    let mut shown = tags(size);
    // Leave out from the end until what is left fits with the count of what is not shown.
    while !shown.is_empty() {
        let left_out = names.len() - shown.len() + 1;
        shown.pop();
        let text = format!("+{left_out}");
        let count = Tag { tone: None, width: tag_width(ruler, &text, size), text };
        let mut candidate = shown.clone();
        candidate.push(count);
        let rows = tag_rows(&candidate, size);
        if rows.len() <= most {
            return (cut, rows);
        }
    }
    (cut, Vec::new())
}

/// The tag rows from `top`: each tag a rounded ground in its tone with the tone's bar at its left
/// edge (the bar cut to the tag's corners) and the name in ink.
fn draw_tags(svg: &mut String, rows: &[Vec<Tag>], (size, height, _): (f32, f32, usize), top: f32) {
    let (gap, radius, bar, before) = (tag_gap(size), size * (12.0 / 44.0), size * (7.0 / 44.0), size * (30.0 / 44.0));
    let mut y = top;
    let mut clip = 0;
    for row in rows {
        let mut x = LEFT;
        for tag in row {
            let (tone, ground, ink) = match tag.tone.and_then(|tone| TONES.get(tone)) {
                Some((tone, ground)) => (*tone, *ground, INK),
                None => (GREY, GREY, INK_2),
            };
            svg.push_str(&format!(
                "<clipPath id=\"tag{clip}\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{height}\" rx=\"{radius}\"/></clipPath>\
                 <g clip-path=\"url(#tag{clip})\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{height}\" fill=\"{tone}\"/>\
                 <rect x=\"{}\" y=\"{y}\" width=\"{w}\" height=\"{height}\" rx=\"{radius}\" fill=\"{ground}\"/></g>\
                 <text x=\"{}\" y=\"{}\" font-size=\"{size}\" font-weight=\"800\" fill=\"{ink}\" letter-spacing=\"{}\">{}</text>",
                x + bar,
                x + before,
                // The capitals' middle on the tag's middle (caps are .728 em high).
                y + height / 2.0 + size * 0.364,
                TAG_SPACING * size,
                escape(&tag.text),
                w = tag.width,
            ));
            clip += 1;
            x += tag.width + gap;
        }
        y += height + gap;
    }
}

// ---------- the picture ----------

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn hex([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// The logo at (x, y of its top): the mark on its 96 grid and the wordmark with the spacing of
/// `design/logo/logo.html` (display cut; offsets in em of the wordmark's size, measured on Inter).
fn logo(svg: &mut String, x: f32, y: f32) {
    let mark = 52.0;
    let scale = mark / 96.0;
    svg.push_str(&format!(
        "<g transform=\"translate({x} {y}) scale({scale})\"><rect x=\".5\" y=\".5\" width=\"95\" height=\"95\" rx=\"20.5\" fill=\"#fff\" stroke=\"{INK}\" stroke-opacity=\".14\" stroke-width=\"1.6\"/>\
         <path fill=\"{INK}\" d=\"M0 20h40v8H0zM68 36h28v8H68zM0 52h16v8H0zM52 68h44v8H52z\"/></g>"
    ));
    let size = 44.0;
    let small = size * 0.72;
    let left = x + mark + 18.0;
    // Capitals are .728 em high: their middle on the middle of the mark.
    let baseline = y + mark / 2.0 + size * 0.364;
    for (offset, letter, capital) in [(0.0, "B", true), (0.63871, "E", false), (0.83563, "T", true), (1.47239, "U", true), (2.14984, "L", false), (2.57135, "A", false)] {
        let (font_size, weight) = if capital { (size, 800) } else { (small, 400) };
        svg.push_str(&format!("<text x=\"{}\" y=\"{baseline}\" font-size=\"{font_size}\" font-weight=\"{weight}\" fill=\"{INK}\">{letter}</text>", left + offset * size));
    }
}

/// A mask of the birch at (x, y), `scale` times its size and cut to its box, as the stylesheet
/// places it: `width` is its own (420 for a head, 1200 for a tile). Every file names its leaves
/// alike, so its ids get `prefix`; its black is `tone`, and what it leaves unfilled takes the
/// group's fill.
fn mask(svg: &mut String, file: &str, prefix: &str, tone: &str, (x, y): (f32, f32), width: f32, scale: f32) {
    let start = file.find('>').map_or(0, |end| end + 1);
    let end = file.rfind("</svg>").unwrap_or(file.len());
    let inside = file.get(start..end).unwrap_or_default().replace("id=\"", &format!("id=\"{prefix}")).replace("href=\"#", &format!("href=\"#{prefix}")).replace("#000", tone);
    svg.push_str(&format!("<svg x=\"{x}\" y=\"{y}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {width} 64\">{inside}</svg>", width * scale, 64.0 * scale));
}

/// The crown along the top of the card's face, as the site hangs it along the top of the view
/// (`.crown` in app/assets/app.css): the card's head at the left edge — dense where the mark
/// stands in front of it, then a clearing just as wide as the wordmark, which stands free there as
/// the page's title does — and the site's tile after it to the right edge, spring's catkins in
/// their own tone; all of it cut to the face's rounded corners.
fn crown(svg: &mut String, season: Season) {
    let crown = birch::card_crown(season);
    svg.push_str(&format!(
        "<clipPath id=\"face\"><rect x=\"{FACE_X}\" y=\"{FACE_Y}\" width=\"{FACE_WIDTH}\" height=\"{FACE_HEIGHT}\" rx=\"{FACE_RADIUS}\"/></clipPath><g clip-path=\"url(#face)\">"
    ));
    let catkins = crown.catkins.zip(season.catkins_tone());
    let layers = [(crown.head, crown.tile, season.tone())].into_iter().chain(catkins.map(|((head, tile), tone)| (head, tile, tone)));
    for (layer, (head, tile, tone)) in layers.enumerate() {
        let tone = hex(tone);
        svg.push_str(&format!("<g fill=\"{tone}\">"));
        mask(svg, head, &format!("c{layer}h-"), &tone, (FACE_X, FACE_Y), crown.head_width, CROWN_SCALE);
        let mut x = FACE_X + crown.head_width * CROWN_SCALE;
        let mut part = 0;
        while x < FACE_X + FACE_WIDTH {
            mask(svg, tile, &format!("c{layer}t{part}-"), &tone, (x, FACE_Y), 1200.0, CROWN_SCALE);
            x += 1200.0 * CROWN_SCALE;
            part += 1;
        }
        svg.push_str("</g>");
    }
    svg.push_str("</g>");
}

/// What every card of a season stands on: the grey, the white face, the crown and the face's
/// hairline over the crown's cut edge (as the ring around a panel on the site).
fn ground_svg(season: Season) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" viewBox=\"0 0 {WIDTH} {HEIGHT}\">\
         <rect width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"{GREY}\"/>\
         <rect x=\"{FACE_X}\" y=\"{FACE_Y}\" width=\"{FACE_WIDTH}\" height=\"{FACE_HEIGHT}\" rx=\"{FACE_RADIUS}\" fill=\"#fff\"/>"
    );
    crown(&mut svg, season);
    svg.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{FACE_RADIUS}\" fill=\"none\" stroke=\"{INK}\" stroke-opacity=\".08\"/></svg>",
        FACE_X + 0.5,
        FACE_Y + 0.5,
        FACE_WIDTH - 1.0,
        FACE_HEIGHT - 1.0
    ));
    svg
}

/// The ground of the cards of `season` as pixels. The crown is some 1,500 leaves, which take
/// most of the time a card is drawn in; they are the same on every card of a season, so they are
/// drawn once and each card is drawn onto a copy (the last season's alone is kept, 3 MB).
fn ground(season: Season) -> Result<tiny_skia::Pixmap, String> {
    static KEPT: Mutex<Option<(Season, tiny_skia::Pixmap)>> = Mutex::new(None);
    if let Some(pixmap) = KEPT.lock().ok().and_then(|kept| kept.as_ref().filter(|(kept, _)| *kept == season).map(|(_, pixmap)| pixmap.clone())) {
        return Ok(pixmap);
    }
    let tree = usvg::Tree::from_str(&ground_svg(season), &usvg::Options::default()).map_err(|error| error.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH as u32, HEIGHT as u32).ok_or("no pixmap")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((season, pixmap.clone()));
    }
    Ok(pixmap)
}

/// What a card says, on a transparent picture: laid over the ground (`ground`).
fn svg(text: &CardText, ruler: &Ruler) -> String {
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" font-family=\"Inter\">");
    logo(&mut svg, LEFT, 92.0);
    svg.push_str(&format!(
        "<text x=\"{RIGHT}\" y=\"134\" text-anchor=\"end\" font-size=\"26\" font-weight=\"600\" fill=\"{INK_2}\" letter-spacing=\"-.3\">{}</text>",
        escape(&fit(&ruler.regular, &text.eyebrow, 26.0, -0.012, 560.0))
    ));

    // The body: title or tags, facts, note; as a block in the middle between logo and footer.
    let width = RIGHT - LEFT;
    let mut facts: Vec<&str> = text.facts.iter().map(String::as_str).filter(|fact| !fact.trim().is_empty()).collect();
    while facts.len() > 1 && Ruler::width(&ruler.regular, &facts.join(" · "), 30.0, -0.012) > width {
        facts.pop();
    }
    let facts = fit(&ruler.regular, &facts.join(" · "), 30.0, -0.012, width);
    let note = text.note.as_deref().map(str::trim).filter(|note| !note.is_empty()).map(|note| fit_list(&ruler.regular, note, 25.0, -0.012, width));
    let below = if facts.is_empty() { 0.0 } else { 22.0 + 36.0 } + if note.is_some() { 12.0 + 32.0 } else { 0.0 };
    let top = |height: f32| (BODY_TOP + ((BODY_BOTTOM - BODY_TOP) - height - below) / 2.0).max(BODY_TOP);
    let mut y = match &text.headline {
        Headline::Title(title) => {
            let (size, leading, lines) = title_lines(ruler, title);
            let mut y = top(lines.len() as f32 * leading);
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
            y
        }
        Headline::Tags(names) => {
            let (cut, rows) = tag_layout(ruler, names);
            let height = rows.len() as f32 * (cut.1 + tag_gap(cut.0)) - tag_gap(cut.0);
            let y = top(height.max(0.0));
            draw_tags(&mut svg, &rows, cut, y);
            y + height.max(0.0)
        }
    };
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

fn draw(text: &CardText, season: Season, fonts: Arc<usvg::fontdb::Database>) -> Result<Vec<u8>, String> {
    let ruler = Ruler::new().ok_or("the typeface of the cards cannot be read")?;
    let options = usvg::Options { fontdb: fonts, font_family: "Inter".to_string(), ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(&svg(text, &ruler), &options).map_err(|error| error.to_string())?;
    let mut pixmap = ground(season)?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    match text.headline {
        Headline::Title(_) => encode(&pixmap, Some(&palette(season))),
        // Eight tones with their grounds and the ink on them are more than a palette holds.
        Headline::Tags(_) => encode(&pixmap, None),
    }
}

/// The colours a card of a title is made of: white towards each ink, towards the page's grey and
/// towards the crown's tone (and spring's catkins), in as many steps each as antialiasing needs
/// there, 256 in all, so the picture fits a palette. A palette PNG is a fifth of the size of the
/// true-colour one and is packed faster.
fn palette(season: Season) -> Vec<[u8; 3]> {
    let rgb = |hex: &str| -> [u8; 3] {
        let channel = |at: usize| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()).unwrap_or(0);
        [channel(1), channel(3), channel(5)]
    };
    let mut ramps = vec![(rgb(INK), 63), (rgb(INK_2), 40), (rgb(INK_3), 40), (rgb(GREY), 16)];
    match season.catkins_tone() {
        Some(catkins) => ramps.extend([(season.tone(), 64), (catkins, 32)]),
        None => ramps.push((season.tone(), 96)),
    }
    let mut colours = vec![[255u8; 3]];
    for (target, steps) in ramps {
        for step in 1..=steps {
            let share = step as f32 / steps as f32;
            let mut colour = [0u8; 3];
            for (out, to) in colour.iter_mut().zip(target) {
                *out = (255.0 + (f32::from(to) - 255.0) * share).round() as u8;
            }
            colours.push(colour);
        }
    }
    colours
}

/// The picture as a PNG: indexed to the nearest colour of `palette`, or in true colour without one.
fn encode(pixmap: &tiny_skia::Pixmap, palette: Option<&[[u8; 3]]>) -> Result<Vec<u8>, String> {
    // Opaque everywhere (the background fills the picture), so premultiplied = plain.
    let pixels = pixmap.data().as_chunks::<4>().0;
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pixmap.width(), pixmap.height());
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Default);
    let data = match palette {
        Some(palette) => {
            // The card has a few hundred different colours: look each one up once.
            let mut nearest: HashMap<[u8; 3], u8> = HashMap::new();
            let mut indexed = Vec::with_capacity(pixels.len());
            // Most pixels repeat their neighbour (white, the grey around the card): ask the table
            // only when the colour changes.
            let mut last = ([0u8; 3], 0u8, false);
            for &[red, green, blue, _] in pixels {
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
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_palette(palette.concat());
            indexed
        }
        None => {
            encoder.set_color(png::ColorType::Rgb);
            pixels.iter().flat_map(|&[red, green, blue, _]| [red, green, blue]).collect()
        }
    };
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer.write_image_data(&data).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(title: &str) -> CardText {
        CardText { eyebrow: "Modul 11101".into(), headline: Headline::Title(title.into()), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into()], note: Some("Fakultät 1".into()) }
    }

    fn size(png: &[u8]) -> (u32, u32) {
        (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()))
    }

    #[tokio::test]
    async fn a_card_is_drawn_once_and_kept_until_its_text_or_season_changes() {
        let cards = Cards::new(8 * 1024 * 1024, 1);
        let title = "Lineare Algebra und analytische Geometrie I";
        let autumn = Season::Autumn;
        let Card::Drawn(etag, png) = cards.get("m:11101", 1, autumn, || Ok(Some(text(title)))).await.unwrap() else { panic!("not drawn") };
        assert!(png.starts_with(b"\x89PNG") && size(&png) == (1200, 630));
        // Same snapshot: answered from memory, the text is not even read.
        let Card::Drawn(again, same) = cards.get("m:11101", 1, autumn, || panic!("a kept card needs no text")).await.unwrap() else { panic!("not kept") };
        assert!(etag == again && same == png && cards.kept().0 == 1);
        // A new snapshot that says the same: read once, not drawn again (no place is needed twice).
        let Card::Drawn(next, same) = cards.get("m:11101", 2, autumn, || Ok(Some(text(title)))).await.unwrap() else { panic!("not kept") };
        assert!(etag == next && same == png);
        let Card::Drawn(..) = cards.get("m:11101", 2, autumn, || panic!("known for this snapshot now")).await.unwrap() else { panic!("not kept") };
        // A snapshot that says something else: a new card replaces the old one.
        let Card::Drawn(changed, _) = cards.get("m:11101", 3, autumn, || Ok(Some(text("Lineare Algebra II")))).await.unwrap() else { panic!("not drawn") };
        assert!(changed != etag && cards.kept().0 == 1);
        // Winter comes: the same text in bare twigs, under another ETag.
        let Card::Drawn(winter, bare) = cards.get("m:11101", 3, Season::Winter, || Ok(Some(text("Lineare Algebra II")))).await.unwrap() else { panic!("not drawn") };
        assert!(winter != changed && bare != png && cards.kept().0 == 1);
        assert!(matches!(cards.get("m:0", 3, autumn, || Ok(None)).await.unwrap(), Card::Unknown));
    }

    #[tokio::test]
    async fn without_a_free_place_the_answer_is_busy_and_the_budget_holds() {
        let none = Cards::new(8 * 1024 * 1024, 0);
        assert!(matches!(none.get("m:1", 1, Season::Summer, || panic!("a busy server reads nothing")).await.unwrap(), Card::Busy));

        let small = Cards::new(1, 1);
        assert!(matches!(small.get("m:1", 1, Season::Summer, || Ok(Some(text("Analysis I")))).await.unwrap(), Card::Drawn(..)));
        assert_eq!(small.kept(), (0, 0), "a card larger than the budget is not kept");
    }

    fn samples() -> Vec<(&'static str, CardText)> {
        let tags = |names: &[&str]| Headline::Tags(names.iter().map(|name| name.to_string()).collect());
        vec![
            ("short", CardText { eyebrow: "Modul 11103".into(), headline: Headline::Title("Analysis I".into()), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into(), "MAP".into()], note: Some("Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik".into()) }),
            ("two-lines", CardText { eyebrow: "Modul 11101".into(), headline: Headline::Title("Lineare Algebra und analytische Geometrie I".into()), facts: vec!["8 LP".into(), "Wintersemester".into(), "Deutsch".into(), "Vorleistung + MAP".into()], note: Some("Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik".into()) }),
            ("long", CardText { eyebrow: "Modul 13849".into(), headline: Headline::Title("Advanced Geophysical Methods in Natural Resource Investigation and Environmental Monitoring of Post-Mining Landscapes (ANRI)".into()), facts: vec!["wird nicht mehr angeboten".into(), "6 LP".into(), "jedes Semester".into(), "Englisch".into(), "MCA".into()], note: None }),
            ("program", CardText { eyebrow: "Studiengang".into(), headline: Headline::Title("Informatik".into()), facts: vec!["B.Sc.".into(), "Prüfungsordnung 2021".into()], note: Some("64 Module im Curriculum  ·  mit Regelstudienplan".into()) }),
            ("program-long", CardText { eyebrow: "Studiengang".into(), headline: Headline::Title("Umweltingenieurwesen – Verfahrenstechnik und Kreislaufwirtschaft".into()), facts: vec!["M.Sc.".into(), "dual, praxisintegrierend".into(), "Prüfungsordnung 2019".into()], note: Some("38 Module im Curriculum".into()) }),
            ("bookmarks", crate::api::bookmarks_card()),
            ("studyplan", crate::api::studyplan_card(Some("WiSe 2026/27"))),
            ("shared-plan", CardText { eyebrow: "Stundenplan · WiSe 2026/27".into(), headline: tags(&["MIT-1", "AuP", "EEG", "LinA", "PhyA", "SWT"]), facts: vec!["6 Module".into(), "36 LP".into(), "Informatik (B.Sc.)".into()], note: Some("Mathematik für Ingenieure 1 · Algorithmen und Programmierung · Elektrotechnik und Elektronik für Informatiker · Lineare Algebra · Physik · Softwaretechnik".into()) }),
            ("shared-plan-many", CardText { eyebrow: "Stundenplan · SoSe 2027".into(), headline: tags(&["MIT-2", "DB", "RN", "BS", "TheoInf", "Mathe II", "IT-Sich", "KI", "SWT-P", "Stat", "WiMa", "Proj", "Sem", "Engl B2", "ProgP"]), facts: vec!["15 Module".into(), "84 LP".into(), "Informatik (B.Sc.)".into()], note: None }),
        ]
    }

    #[test]
    fn tags_shrink_into_rows_and_count_what_is_left_out() {
        let ruler = Ruler::new().unwrap();
        let names = |n: usize| (1..=n).map(|i| format!("MOD-{i}")).collect::<Vec<_>>();
        let ((size, _, most), rows) = tag_layout(&ruler, &names(5));
        assert!(size == 44.0 && rows.len() <= most && rows.iter().flatten().count() == 5);
        // Thirty never fit: the last tag counts those left out, and they add up.
        let (_, rows) = tag_layout(&ruler, &names(30));
        let tags: Vec<&Tag> = rows.iter().flatten().collect();
        let count = tags.last().unwrap();
        assert!(count.tone.is_none() && rows.len() <= 3, "{rows:?}");
        assert_eq!(tags.len() - 1 + count.text.trim_start_matches('+').parse::<usize>().unwrap(), 30);
        for row in &rows {
            let used: f32 = row.iter().map(|tag| tag.width).sum::<f32>() + tag_gap(30.0) * (row.len() - 1) as f32;
            assert!(used <= RIGHT - LEFT);
        }
        // The tones follow the plan's order, the ninth module in the first tone again.
        let (_, rows) = tag_layout(&ruler, &names(9));
        let tones: Vec<Option<usize>> = rows.iter().flatten().map(|tag| tag.tone).collect();
        assert_eq!(tones, [0, 1, 2, 3, 4, 5, 6, 7, 0].map(Some));
    }

    #[test]
    fn every_card_is_drawn_in_every_season() {
        let fonts = Cards::new(0, 1).fonts.clone();
        for season in Season::ALL {
            for (name, text) in samples() {
                let png = draw(&text, season, fonts.clone()).unwrap();
                assert!(png.starts_with(b"\x89PNG") && size(&png) == (1200, 630), "{name} in {season:?}");
            }
        }
    }

    /// For looking at the design: `FOLIA_CARD_OUT=<dir> cargo test -p folia-server cards_for_review`
    /// (`FOLIA_CARD_SEASON=spring` for another season than the one of today).
    #[test]
    fn cards_for_review() {
        let Ok(dir) = std::env::var("FOLIA_CARD_OUT") else { return };
        let season = std::env::var("FOLIA_CARD_SEASON").ok().and_then(|name| Season::ALL.into_iter().find(|season| season.name() == name)).unwrap_or_else(Season::now);
        let fonts = Cards::new(0, 1).fonts.clone();
        for (name, text) in samples() {
            let started = std::time::Instant::now();
            let png = draw(&text, season, fonts.clone()).unwrap();
            println!("{name} ({}): {} bytes, {} ms", season.name(), png.len(), started.elapsed().as_millis());
            std::fs::write(format!("{dir}/card-{name}-{}.png", season.name()), png).unwrap();
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

    #[test]
    fn a_list_is_cut_between_its_items() {
        let ruler = Ruler::new().unwrap();
        let titles = "Mathematik für Ingenieure 1 · Algorithmen und Programmierung · Elektrotechnik und Elektronik für Informatiker · Lineare Algebra · Physik";
        let cut = fit_list(&ruler.regular, titles, 25.0, -0.012, RIGHT - LEFT);
        assert_eq!(cut, "Mathematik für Ingenieure 1 · Algorithmen und Programmierung · …");
        assert_eq!(fit_list(&ruler.regular, "Physik · Chemie", 25.0, -0.012, RIGHT - LEFT), "Physik · Chemie");
        // No list, or its first item alone too long: cut at a character.
        let department = "Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik und noch viel mehr";
        assert!(fit_list(&ruler.regular, department, 25.0, -0.012, RIGHT - LEFT).ends_with(" …"));
    }

    #[test]
    fn the_palette_has_the_crown_of_every_season() {
        for season in Season::ALL {
            let palette = palette(season);
            assert_eq!(palette.len(), 256, "{season:?}");
            assert!(palette.contains(&season.tone()) && season.catkins_tone().is_none_or(|tone| palette.contains(&tone)));
        }
    }
}
