//! The map of the programs on the landing page: every current program is a dot, two programs
//! are linked when their curricula share modules, and the layout pulls linked programs together.
//! What comes out are the neighbourhoods of the university (the engineers around „Höhere
//! Mathematik", the business programs, health in Senftenberg …) without anybody having drawn them.
//!
//! The web server lays the map out once per snapshot and keeps it (owner decision 2026-09-20:
//! built on the backend and cached, the frontend only draws). Server-rendered pages get it through
//! context; the browser app gets the same thing as `/api/map.json`, fetched by `boot.js` next to
//! the database. `ProgramMap` is therefore complete in itself: what it says about a program does
//! not depend on the copy of the catalog a browser happens to have. It is a pure function of the
//! snapshot and has no randomness: the same snapshot always gives the same map.
//!
//! How the kinship of two programs is measured: shared modules / modules of either (Jaccard).
//! Modules that belong to more than `HUB_LIMIT` programs are left out of the measure: a container
//! module that 113 programs list says nothing about who is related to whom.
//!
//! The layout is a plain force simulation (Fruchterman and Reingold: all dots repel each other,
//! links pull, a pull towards the middle keeps unconnected groups on the sheet). It runs in a
//! space whose x axis is compressed by the aspect ratio of the sheet, so it fills a wide sheet as
//! well as a tall one; afterwards the dots are fitted to the sheet and pushed apart until none
//! overlap.
//!
//! The faculties (owner wish 2026-09-21): programs of one faculty are pulled a very little
//! towards each other, a hint and not the shape of the map; what programs share decides. After
//! the simulation the dots are drawn part of the way towards an even spread per axis (`EVEN`), so
//! the sheet is filled without dense clumps and empty stretches. Each
//! faculty has an outline with its name, which the app shows for the faculty of a picked program. A program's faculty is derived (`pages::faculties`, no source states it); a
//! program without one only follows its links. Faculties are grouped by their number: the
//! snapshot lists faculties 1 to 4 twice (the names before and after the restructuring), which
//! are one faculty each.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::labels::DegreeLevel;
use crate::rows::Program;

/// Modules in more programs than this say nothing about kinship.
const HUB_LIMIT: usize = 40;
/// Every program keeps the links to its closest relatives …
const NEAREST: usize = 3;
/// … if they are at least this similar, and every link of at least `STRONG` is kept anyway.
const WEAKEST: f64 = 0.04;
const STRONG: f64 = 0.30;
const ROUNDS: usize = 400;
/// The faculty is a small bias, not the shape of the map (owner 2026-09-21): how hard a program
/// is pulled to the middle of its faculty, how much harder dots of two faculties repel each other
/// than two of one faculty, and how much a link across faculties pulls compared to one within.
const FACULTY_PULL: f64 = 0.12;
const FACULTY_APART: f64 = 1.1;
const FACULTY_ACROSS: f64 = 0.9;
/// How far the dots are drawn towards an even spread after the simulation (0 = as simulated,
/// 1 = ranks only): dense clumps open up, empty stretches fill, the order stays.
const EVEN: f64 = 0.45;
/// Dots of a faculty closer than this many times the spacing of an even sheet share an outline.
const ISLAND: f64 = 1.1;
/// Room around the dots inside the outline of a faculty.
const REGION_PAD: f64 = 9.0;

/// The sheet a layout is made for. A phone gets a tall one: the same programs, laid out again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    Wide,
    Tall,
}

impl Sheet {
    fn size(self) -> (f64, f64) {
        match self {
            Sheet::Wide => (1200.0, 600.0),
            Sheet::Tall => (600.0, 860.0),
        }
    }

    /// Height of the names in units of the sheet (a tall sheet is shown much smaller).
    fn font(self) -> f64 {
        match self {
            Sheet::Wide => 12.0,
            Sheet::Tall => 17.0,
        }
    }

    fn names(self) -> usize {
        match self {
            Sheet::Wide => 20,
            Sheet::Tall => 12,
        }
    }

    /// Dots are larger on the tall sheet for the same reason the names are.
    fn dot(self) -> f64 {
        match self {
            Sheet::Wide => 1.0,
            Sheet::Tall => 1.25,
        }
    }
}

/// The columns of the program overview, as the map tells them apart: filled, ring, grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cycle {
    Bachelor,
    Master,
    Other,
}

impl Cycle {
    pub fn of(program: &Program) -> Self {
        match program.degree_level.known() {
            Some(DegreeLevel::Bachelor | DegreeLevel::TeachingBachelor) => Cycle::Bachelor,
            Some(DegreeLevel::Master | DegreeLevel::TeachingMaster) => Cycle::Master,
            _ => Cycle::Other,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Cycle::Bachelor => "bachelor",
            Cycle::Master => "master",
            Cycle::Other => "other",
        }
    }
}

/// What the map says about a program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapProgram {
    pub slug: String,
    pub name: String,
    /// „B.Sc.", else „Bachelor", else the raw text of the source (`Program::degree`).
    pub degree: String,
    /// The form of study where it is not the plain one („dual, praxisintegrierend"): what tells
    /// two programs of the same name and degree apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    pub cycle: Cycle,
    /// Modules of the curriculum.
    pub modules: usize,
    /// Index into `ProgramMap::faculties`; `None` where no faculty could be derived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faculty: Option<usize>,
}

/// A faculty on the map: „1" and „MINT", or „3" and „Maschinenbau, Elektro- und Energiesysteme".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapFaculty {
    pub code: String,
    pub name: String,
}

impl MapFaculty {
    pub fn title(&self) -> String {
        format!("Fakultät {} · {}", self.code, self.name)
    }
}

impl MapProgram {
    /// „Maschinenbau B.Sc." or „Maschinenbau - dual B.Sc. (dual, praxisintegrierend)".
    pub fn title(&self) -> String {
        match &self.variant {
            Some(variant) => format!("{} {} ({variant})", self.name, self.degree),
            None => format!("{} {}", self.name, self.degree),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub a: usize,
    pub b: usize,
    pub shared: usize,
    /// Shared modules / modules of either program, 0 to 1.
    pub similarity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    pub fn code(self) -> &'static str {
        match self {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        }
    }
}

/// Where the name of a program stands (the text is `MapProgram::name`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Name {
    pub program: usize,
    pub x: f64,
    pub y: f64,
    pub anchor: Anchor,
}

/// The outline of a faculty on one sheet and where its name stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub faculty: usize,
    /// A closed, smooth SVG path around the faculty's dots.
    pub path: String,
    /// The name: „Fakultät 1 · MINT", or only „Fakultät 1" where the long one does not fit.
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub anchor: Anchor,
}

/// The programs on one sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub width: f64,
    pub height: f64,
    /// Height of the names, in units of the sheet.
    pub font: f64,
    /// (x, y, radius) per program, in the order of `ProgramMap::programs`.
    pub dots: Vec<(f64, f64, f64)>,
    /// The names that fit without covering a dot or each other, the largest programs first.
    pub names: Vec<Name>,
    /// One per faculty with programs, in the order of `ProgramMap::faculties`.
    #[serde(default)]
    pub regions: Vec<Region>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramMap {
    pub programs: Vec<MapProgram>,
    /// The faculties that have programs on the map, by number.
    #[serde(default)]
    pub faculties: Vec<MapFaculty>,
    /// Sorted by (a, b), a < b.
    pub links: Vec<Link>,
    pub wide: Layout,
    /// For a phone: the same programs, laid out again on a tall sheet.
    pub tall: Layout,
}

impl ProgramMap {
    /// The relatives of a program, the closest first: (program, shared modules).
    pub fn relatives(&self, program: usize) -> Vec<(usize, usize)> {
        let mut found: Vec<(usize, usize, f64)> = self
            .links
            .iter()
            .filter_map(|link| match (link.a == program, link.b == program) {
                (true, _) => Some((link.b, link.shared, link.similarity)),
                (_, true) => Some((link.a, link.shared, link.similarity)),
                _ => None,
            })
            .collect();
        found.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        found.into_iter().map(|(other, shared, _)| (other, shared)).collect()
    }
}

/// The map of these programs (the current ones). `curriculum`: `queries::curriculum_links`;
/// `faculty`: per program an index into `faculties` (see `pages::program_map`).
pub fn program_map(programs: &[Program], curriculum: &[(String, String)], faculties: Vec<MapFaculty>, faculty: &[Option<usize>]) -> ProgramMap {
    let faculty: Vec<Option<usize>> = (0..programs.len()).map(|i| faculty.get(i).copied().flatten().filter(|f| *f < faculties.len())).collect();
    let (modules, links) = links(programs, curriculum);
    let wide = layout(programs, &modules, &links, &faculties, &faculty, Sheet::Wide);
    let tall = layout(programs, &modules, &links, &faculties, &faculty, Sheet::Tall);
    let programs = programs
        .iter()
        .zip(modules.iter())
        .zip(faculty.iter())
        .map(|((program, modules), faculty)| MapProgram {
            slug: program.slug.clone(),
            name: program.name.trim().to_string(),
            degree: program.degree().to_string(),
            variant: program.study_variant.as_ref().map(|variant| variant.label().to_string()),
            cycle: Cycle::of(program),
            modules: *modules,
            faculty: *faculty,
        })
        .collect();
    ProgramMap { programs, faculties, links, wide, tall }
}

/// Who is related to whom: the modules per program and the links that are drawn.
fn links(programs: &[Program], curriculum: &[(String, String)]) -> (Vec<usize>, Vec<Link>) {
    let index: BTreeMap<&str, usize> = programs.iter().enumerate().map(|(i, program)| (program.id.as_str(), i)).collect();
    let mut by_module: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (program_id, module_id) in curriculum {
        if let Some(i) = index.get(program_id.as_str()) {
            let members = by_module.entry(module_id.as_str()).or_default();
            if !members.contains(i) {
                members.push(*i);
            }
        }
    }
    let mut modules = vec![0usize; programs.len()];
    let mut shared: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for members in by_module.values_mut() {
        members.sort_unstable();
        for i in members.iter() {
            if let Some(count) = modules.get_mut(*i) {
                *count += 1;
            }
        }
        if members.len() > HUB_LIMIT {
            continue;
        }
        for (n, a) in members.iter().enumerate() {
            for b in members.iter().skip(n + 1) {
                *shared.entry((*a, *b)).or_default() += 1;
            }
        }
    }

    let size = |i: usize| modules.get(i).copied().unwrap_or(0);
    // The closest relatives of every program, and every close pair.
    let mut keep: BTreeMap<(usize, usize), (usize, f64)> = BTreeMap::new();
    let mut of: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); programs.len()];
    for (&(a, b), &n) in &shared {
        let similarity = n as f64 / (size(a) + size(b)).saturating_sub(n).max(1) as f64;
        if let Some(list) = of.get_mut(a) {
            list.push((similarity, b, n));
        }
        if let Some(list) = of.get_mut(b) {
            list.push((similarity, a, n));
        }
        if similarity >= STRONG {
            keep.insert((a, b), (n, similarity));
        }
    }
    for (a, list) in of.iter_mut().enumerate() {
        list.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        for &(similarity, b, n) in list.iter().take(NEAREST).filter(|(similarity, ..)| *similarity >= WEAKEST) {
            keep.insert((a.min(b), a.max(b)), (n, similarity));
        }
    }
    let links = keep.into_iter().map(|((a, b), (shared, similarity))| Link { a, b, shared, similarity: (similarity * 100.0).round() / 100.0 }).collect();
    (modules, links)
}

fn layout(programs: &[Program], modules: &[usize], links: &[Link], faculties: &[MapFaculty], faculty: &[Option<usize>], sheet: Sheet) -> Layout {
    let n = programs.len();
    let (width, height) = sheet.size();
    if n == 0 {
        return Layout { width, height, font: sheet.font(), dots: Vec::new(), names: Vec::new(), regions: Vec::new() };
    }
    let of = |i: usize| faculty.get(i).copied().flatten();
    let radius: Vec<f64> = modules.iter().map(|&m| sheet.dot() * (4.0 + 9.0 * ((m.min(160) as f64) / 160.0).sqrt())).collect();

    // The simulation runs in a space as high as the sheet whose x axis is compressed.
    let squeeze = (width / height).max(0.2) * 0.95;
    let (space_w, space_h) = (width / squeeze, height);
    let k = (space_w * space_h / n as f64).sqrt() * 0.5;
    // A sunflower: evenly spread, no two dots at the same place, no randomness. The faculty
    // plays no part here: where a program starts must not decide where it ends.
    let mut at: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let (distance, angle) = (((i as f64 + 0.5) / n as f64).sqrt(), i as f64 * 2.399_963_229_728_653);
            (space_w / 2.0 + distance * angle.cos() * space_w * 0.4, space_h / 2.0 + distance * angle.sin() * space_h * 0.4)
        })
        .collect();

    let mut push = vec![(0.0f64, 0.0f64); n];
    for round in 0..ROUNDS {
        let cooling = 1.0 - round as f64 / ROUNDS as f64;
        for p in push.iter_mut() {
            *p = (0.0, 0.0);
        }
        for (i, a) in at.iter().enumerate() {
            for (j, b) in at.iter().enumerate().skip(i + 1) {
                let (ux, uy) = (a.0 - b.0, a.1 - b.1);
                let apart = match (of(i), of(j)) {
                    (Some(x), Some(y)) if x != y => FACULTY_APART,
                    _ => 1.0,
                };
                let f = apart * k * k / (ux * ux + uy * uy + 0.01);
                if let Some(p) = push.get_mut(i) {
                    p.0 += ux * f;
                    p.1 += uy * f;
                }
                if let Some(p) = push.get_mut(j) {
                    p.0 -= ux * f;
                    p.1 -= uy * f;
                }
            }
        }
        for link in links {
            let (Some(a), Some(b)) = (at.get(link.a), at.get(link.b)) else { continue };
            let (ux, uy) = (a.0 - b.0, a.1 - b.1);
            // A link across faculties pulls less: it should show, not merge the regions.
            let across = if of(link.a) == of(link.b) { 1.0 } else { FACULTY_ACROSS };
            let f = ((ux * ux + uy * uy).sqrt() + 0.01) / k * (0.4 + 1.6 * link.similarity) * across;
            if let Some(p) = push.get_mut(link.a) {
                p.0 -= ux * f;
                p.1 -= uy * f;
            }
            if let Some(p) = push.get_mut(link.b) {
                p.0 += ux * f;
                p.1 += uy * f;
            }
        }
        // Every program towards the middle of its faculty.
        let mut middle = vec![(0.0f64, 0.0f64, 0usize); faculties.len()];
        for (i, place) in at.iter().enumerate() {
            if let Some(m) = of(i).and_then(|f| middle.get_mut(f)) {
                m.0 += place.0;
                m.1 += place.1;
                m.2 += 1;
            }
        }
        for (i, place) in at.iter().enumerate() {
            if let (Some(p), Some(&(x, y, count))) = (push.get_mut(i), of(i).and_then(|f| middle.get(f))) {
                let count = count.max(1) as f64;
                p.0 += (x / count - place.0) * FACULTY_PULL;
                p.1 += (y / count - place.1) * FACULTY_PULL;
            }
        }
        for (place, p) in at.iter_mut().zip(push.iter()) {
            let (fx, fy) = (p.0 + (space_w / 2.0 - place.0) * 0.5, p.1 + (space_h / 2.0 - place.1) * 0.5);
            let strength = (fx * fx + fy * fy).sqrt() + 0.01;
            let step = strength.min(1.0 + 30.0 * cooling * cooling);
            place.0 += fx / strength * step;
            place.1 += fy / strength * step;
        }
    }

    // Towards an even density (owner 2026-09-21): on each axis, every dot moves part of the way
    // from where it is to where its rank would put it if the dots were spread evenly.
    for axis in [0usize, 1] {
        let value = |p: &(f64, f64)| if axis == 0 { p.0 } else { p.1 };
        let (low, high) = at.iter().map(value).fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(v), h.max(v)));
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| at.get(*a).map(value).unwrap_or(0.0).total_cmp(&at.get(*b).map(value).unwrap_or(0.0)).then(a.cmp(b)));
        for (rank, i) in order.into_iter().enumerate() {
            let even = low + (high - low) * (rank as f64 + 0.5) / n as f64;
            if let Some(place) = at.get_mut(i) {
                let v = if axis == 0 { &mut place.0 } else { &mut place.1 };
                *v += (even - *v) * EVEN;
            }
        }
    }

    // Onto the sheet: both axes fill it, with room for the outlines of the faculties.
    let margin = 24.0 + REGION_PAD;
    let (mut left, mut right, mut top, mut bottom) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for place in &at {
        left = left.min(place.0);
        right = right.max(place.0);
        top = top.min(place.1);
        bottom = bottom.max(place.1);
    }
    let (scale_x, scale_y) = ((width - 2.0 * margin) / (right - left).max(1.0), (height - 2.0 * margin) / (bottom - top).max(1.0));
    for place in at.iter_mut() {
        place.0 = (place.0 - (left + right) / 2.0) * scale_x + width / 2.0;
        place.1 = (place.1 - (top + bottom) / 2.0) * scale_y + height / 2.0;
    }

    // No dot on another one.
    for _ in 0..200 {
        let mut moved = false;
        for i in 0..n {
            for j in (i + 1)..n {
                let (Some(a), Some(b)) = (at.get(i).copied(), at.get(j).copied()) else { continue };
                let (ux, uy) = (a.0 - b.0, a.1 - b.1);
                let distance = (ux * ux + uy * uy).sqrt() + 0.01;
                let needed = radius.get(i).copied().unwrap_or(4.0) + radius.get(j).copied().unwrap_or(4.0) + 3.0;
                if distance < needed {
                    let shift = (needed - distance) / distance * 0.5;
                    if let Some(place) = at.get_mut(i) {
                        place.0 += ux * shift;
                        place.1 += uy * shift;
                    }
                    if let Some(place) = at.get_mut(j) {
                        place.0 -= ux * shift;
                        place.1 -= uy * shift;
                    }
                    moved = true;
                }
            }
        }
        let edge = 4.0 + REGION_PAD;
        for (place, r) in at.iter_mut().zip(radius.iter()) {
            place.0 = place.0.clamp(r + edge, width - r - edge);
            place.1 = place.1.clamp(r + edge, height - r - edge);
        }
        if !moved {
            break;
        }
    }

    let dots: Vec<(f64, f64, f64)> = at.iter().zip(radius.iter()).map(|(place, r)| (round(place.0), round(place.1), round(*r))).collect();
    let names = names(programs, modules, &dots, sheet);
    let named: Vec<(f64, f64, f64, f64)> = names
        .iter()
        .filter_map(|name| programs.get(name.program).map(|program| text_box(name.x, name.y, text_width(program.name.trim(), sheet.font()), name.anchor, sheet.font())))
        .collect();
    let regions = regions(faculties, faculty, &dots, &named, sheet);
    Layout { width, height, font: sheet.font(), dots, names, regions }
}

/// Width of a text in units of the sheet (Inter, a little generous).
fn text_width(text: &str, font: f64) -> f64 {
    text.chars().count() as f64 * font * 0.6 + 4.0
}

/// The box a text covers: (left, top, right, bottom).
fn text_box(x: f64, y: f64, width: f64, anchor: Anchor, font: f64) -> (f64, f64, f64, f64) {
    let x0 = match anchor {
        Anchor::Start => x,
        Anchor::End => x - width,
        Anchor::Middle => x - width / 2.0,
    };
    (x0, y - font, x0 + width, y + font * 0.3)
}

fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

/// The names of the faculties are a little smaller than those of the programs.
fn faculty_font(sheet: Sheet) -> f64 {
    sheet.font() * 0.95
}

/// The outline of every faculty: a smooth shape around the convex hull of its dots (each
/// widened by `REGION_PAD`), see `outline`. The app shows one at a time (the faculty of the picked
/// program), so outlines may overlap; its name stands at the edge of the outline where it covers
/// no dot and no name of a program, the long name where it fits, else the short.
fn regions(faculties: &[MapFaculty], faculty: &[Option<usize>], dots: &[(f64, f64, f64)], named: &[(f64, f64, f64, f64)], sheet: Sheet) -> Vec<Region> {
    let (width, height) = sheet.size();
    let font = faculty_font(sheet);
    // Two dots of a faculty belong to one island when they are closer than this: a little more
    // than the distance of neighbours on an evenly filled sheet.
    let reach_apart = (width * height / dots.len().max(1) as f64).sqrt() * ISLAND;
    let hulls: Vec<(usize, Vec<Hull>)> = (0..faculties.len())
        .filter_map(|f| {
            let members: Vec<(f64, f64, f64)> = dots.iter().zip(faculty.iter()).filter(|(_, of)| **of == Some(f)).map(|(dot, _)| *dot).collect();
            let islands: Vec<Hull> = islands(&members, reach_apart)
                .into_iter()
                .map(|island| {
                    let mut points: Vec<(f64, f64)> = Vec::new();
                    for (x, y, r) in island {
                        let reach = r + REGION_PAD;
                        for step in 0..16 {
                            let angle = std::f64::consts::TAU * step as f64 / 16.0;
                            points.push((x + angle.cos() * reach, y + angle.sin() * reach));
                        }
                    }
                    convex_hull(points)
                })
                .collect();
            (!islands.is_empty()).then_some((f, islands))
        })
        .collect();

    let mut taken: Vec<(f64, f64, f64, f64)> = dots.iter().map(|(x, y, r)| (x - r - 2.0, y - r - 2.0, x + r + 2.0, y + r + 2.0)).collect();
    taken.extend_from_slice(named);
    let mut regions = Vec::new();
    for (f, islands) in &hulls {
        let Some(about) = faculties.get(*f) else { continue };
        // One closed curve per island; the name goes to the largest (the first).
        let shapes: Vec<(String, Vec<(f64, f64)>)> = islands.iter().map(|hull| outline(hull, width, height)).collect();
        let path: String = shapes.iter().map(|(path, _)| path.as_str()).collect();
        let Some((_, hull)) = shapes.first() else { continue };
        let (left, right) = hull.iter().fold((f64::MAX, f64::MIN), |(l, r), p| (l.min(p.0), r.max(p.0)));
        let (top, bottom) = hull.iter().fold((f64::MAX, f64::MIN), |(t, b), p| (t.min(p.1), b.max(p.1)));
        let middle = (left + right) / 2.0;
        let short = format!("Fakultät {}", about.code);

        // The name stays at its island: above it or below it, the long name before the short; then
        // the short name beside it or inside along its top and bottom. The first place that
        // covers nothing wins; where every place covers something, the one that covers least (a
        // dot counts more than a name), the earlier one on a tie.
        let (long, gap, mid_y) = (about.title(), font * 0.5, (top + bottom) / 2.0 + font * 0.35);
        let (w_long, w_short) = (text_width(&long, font), text_width(&short, font));
        let around = |w: f64| [
            (middle, top - font * 0.35),
            (left + w / 2.0, top - font * 0.35),
            (right - w / 2.0, top - font * 0.35),
            (middle, bottom + font * 1.05),
            (left + w / 2.0, bottom + font * 1.05),
            (right - w / 2.0, bottom + font * 1.05),
        ];
        let mut places: Vec<(&str, f64, f64, f64)> = around(w_long).into_iter().map(|(x, y)| (long.as_str(), w_long, x, y)).collect();
        places.extend(around(w_short).into_iter().map(|(x, y)| (short.as_str(), w_short, x, y)));
        for (x, y) in [(left - gap - w_short / 2.0, mid_y), (right + gap + w_short / 2.0, mid_y), (middle, top + font * 1.3), (middle, bottom - font * 0.5)] {
            places.push((short.as_str(), w_short, x, y));
        }
        let mut best: Option<(usize, String, f64, f64)> = None;
        for (text, w, x, y) in places {
            let (x, y) = (x.clamp(w / 2.0 + 2.0, width - w / 2.0 - 2.0), y.clamp(font + 2.0, height - font * 0.3 - 2.0));
            let area = text_box(x, y, w, Anchor::Middle, font);
            let covered = taken.iter().enumerate().filter(|(_, b)| overlap(area, **b)).map(|(i, _)| if i < dots.len() { 3 } else { 1 }).sum::<usize>();
            if best.as_ref().is_none_or(|(least, ..)| covered < *least) {
                best = Some((covered, text.to_string(), x, y));
            }
            if covered == 0 {
                break;
            }
        }
        let Some((_, label, x, y)) = best else { continue };
        regions.push(Region { faculty: *f, path, label, x: round(x), y: round(y), anchor: Anchor::Middle });
    }
    regions
}

/// The corners of a convex hull.
type Hull = Vec<(f64, f64)>;

/// The dots in groups that are closer to each other than `apart` (single linkage), the group
/// with the most dots first (then by the first dot, so the order is fixed).
fn islands(dots: &[(f64, f64, f64)], apart: f64) -> Vec<Vec<(f64, f64, f64)>> {
    let mut group: Vec<usize> = (0..dots.len()).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while let Some(&up) = group.get(i) {
            if up == i {
                break;
            }
            i = up;
        }
        i
    }
    for (i, a) in dots.iter().enumerate() {
        for (j, b) in dots.iter().enumerate().skip(i + 1) {
            let gap = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() - a.2 - b.2;
            if gap < apart {
                let (ri, rj) = (root(&mut group, i), root(&mut group, j));
                if let Some(slot) = group.get_mut(ri.max(rj)) {
                    *slot = ri.min(rj);
                }
            }
        }
    }
    let mut found: BTreeMap<usize, Vec<(f64, f64, f64)>> = BTreeMap::new();
    for (i, dot) in dots.iter().enumerate() {
        let r = root(&mut group, i);
        found.entry(r).or_default().push(*dot);
    }
    let mut found: Vec<Vec<(f64, f64, f64)>> = found.into_values().collect();
    found.sort_by_key(|island| std::cmp::Reverse(island.len()));
    found
}

/// Andrew's monotone chain: the hull counter-clockwise, without repeating the first point.
fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut hull: Vec<(f64, f64)> = Vec::new();
    for upper in [false, true] {
        let start = hull.len();
        let mut add = |p: (f64, f64)| {
            while let [.., a, b] = hull.as_slice() {
                if hull.len() < start + 2 || cross(*a, *b, p) > 0.0 {
                    break;
                }
                hull.pop();
            }
            hull.push(p);
        };
        if upper {
            points.iter().rev().for_each(|p| add(*p));
        } else {
            points.iter().for_each(|p| add(*p));
        }
        hull.pop();
    }
    hull
}

/// Around how many directions the outline of a faculty is measured, and how far it is smoothed.
const OUTLINE_STEPS: usize = 90;
const OUTLINE_BLUR: usize = 4;

/// The outline of a faculty: its hull, seen from the middle as a distance per direction, that
/// distance smoothed (a blob, not a polygon), widened until it holds the whole hull again, and
/// drawn as one closed curve (Catmull-Rom as cubic Béziers). It takes in a little more than the
/// hull on purpose: a calm shape reads as drawn on purpose, a tight one as a mistake.
/// Returns the path and the points it passes through.
fn outline(hull: &[(f64, f64)], width: f64, height: f64) -> (String, Vec<(f64, f64)>) {
    if hull.is_empty() {
        return (String::new(), Vec::new());
    }
    let n = hull.len() as f64;
    let middle = (hull.iter().map(|p| p.0).sum::<f64>() / n, hull.iter().map(|p| p.1).sum::<f64>() / n);
    let edges: Vec<((f64, f64), (f64, f64))> = hull.iter().zip(hull.iter().cycle().skip(1)).map(|(a, b)| (*a, *b)).collect();
    // Where the ray from the middle in direction `d` leaves the hull (it is convex).
    let reach = |d: (f64, f64)| {
        edges
            .iter()
            .filter_map(|(a, b)| {
                let (ex, ey) = (b.0 - a.0, b.1 - a.1);
                let det = d.0 * ey - d.1 * ex;
                if det.abs() < 1e-9 {
                    return None;
                }
                let (wx, wy) = (a.0 - middle.0, a.1 - middle.1);
                let t = (wx * ey - wy * ex) / det;
                let u = (wx * d.1 - wy * d.0) / det;
                (t > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
            })
            .fold(0.0f64, f64::max)
    };
    let directions: Vec<(f64, f64)> = (0..OUTLINE_STEPS)
        .map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / OUTLINE_STEPS as f64;
            (angle.cos(), angle.sin())
        })
        .collect();
    let exact: Vec<f64> = directions.iter().map(|d| reach(*d).max(1.0)).collect();
    let mut smooth = exact.clone();
    for _ in 0..3 {
        smooth = (0..OUTLINE_STEPS)
            .map(|k| {
                let window = (0..=2 * OUTLINE_BLUR).map(|j| (k + OUTLINE_STEPS + j - OUTLINE_BLUR) % OUTLINE_STEPS);
                window.filter_map(|j| smooth.get(j)).sum::<f64>() / (2 * OUTLINE_BLUR + 1) as f64
            })
            .collect();
    }
    let widen = exact.iter().zip(smooth.iter()).map(|(e, s)| e / s).fold(1.0f64, f64::max);
    let points: Vec<(f64, f64)> = directions
        .iter()
        .zip(smooth.iter())
        .map(|(d, r)| ((middle.0 + d.0 * r * widen).clamp(2.0, width - 2.0), (middle.1 + d.1 * r * widen).clamp(2.0, height - 2.0)))
        .collect();

    let count = points.len();
    let at = |i: usize| points.get(i % count).copied().unwrap_or(middle);
    let start = at(0);
    let mut path = format!("M{} {}", round(start.0), round(start.1));
    for i in 0..count {
        let (p0, p1, p2, p3) = (at(i + count - 1), at(i), at(i + 1), at(i + 2));
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        path.push_str(&format!("C{} {} {} {} {} {}", round(c1.0), round(c1.1), round(c2.0), round(c2.1), round(p2.0), round(p2.1)));
    }
    path.push('Z');
    (path, points)
}

/// One decimal is finer than a screen pixel and keeps the map short.
fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Names for the largest programs: a name once (Bachelor and Master of a subject sit together),
/// the plain program rather than its dual form, and only where the text covers nothing.
fn names(programs: &[Program], modules: &[usize], dots: &[(f64, f64, f64)], sheet: Sheet) -> Vec<Name> {
    let (width, height) = sheet.size();
    let font = sheet.font();
    let mut taken: Vec<(f64, f64, f64, f64)> = dots.iter().map(|(x, y, r)| (x - r - 2.0, y - r - 2.0, x + r + 2.0, y + r + 2.0)).collect();
    let mut order: Vec<usize> = (0..programs.len()).collect();
    order.sort_by_key(|i| (std::cmp::Reverse(modules.get(*i).copied().unwrap_or(0)), *i));

    let mut names: Vec<Name> = Vec::new();
    let mut named: Vec<&str> = Vec::new();
    for i in order {
        let (Some(program), Some(&(x, y, r))) = (programs.get(i), dots.get(i)) else { continue };
        let text = program.name.trim();
        let letters = text.chars().count();
        if program.study_variant.is_some() || program.name_key.ends_with("-dual") || letters > 32 || named.contains(&text) {
            continue;
        }
        let (text_w, gap) = (letters as f64 * font * 0.6 + 4.0, font * 0.45);
        let candidates = [
            (x + r + gap, y + font * 0.35, Anchor::Start),
            (x - r - gap, y + font * 0.35, Anchor::End),
            (x, y + r + font * 1.1, Anchor::Middle),
            (x, y - r - font * 0.5, Anchor::Middle),
            // Beside the dot, a line higher or lower: where the dots are spread evenly, the
            // straight places are often taken.
            (x + r * 0.7 + gap, y - r * 0.7, Anchor::Start),
            (x - r * 0.7 - gap, y - r * 0.7, Anchor::End),
            (x + r * 0.7 + gap, y + r * 0.7 + font * 0.7, Anchor::Start),
            (x - r * 0.7 - gap, y + r * 0.7 + font * 0.7, Anchor::End),
        ];
        for (at_x, at_y, anchor) in candidates {
            let x0 = match anchor {
                Anchor::Start => at_x,
                Anchor::End => at_x - text_w,
                Anchor::Middle => at_x - text_w / 2.0,
            };
            let area = (x0, at_y - font, x0 + text_w, at_y + font * 0.3);
            let inside = area.0 >= 2.0 && area.2 <= width - 2.0 && area.1 >= 2.0 && area.3 <= height - 2.0;
            let free = taken.iter().enumerate().all(|(other, b)| other == i || !(area.0 < b.2 && area.2 > b.0 && area.1 < b.3 && area.3 > b.1));
            if inside && free {
                taken.push(area);
                named.push(text);
                names.push(Name { program: i, x: round(at_x), y: round(at_y), anchor });
                break;
            }
        }
        if names.len() >= sheet.names() {
            break;
        }
    }
    names
}
