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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramMap {
    pub programs: Vec<MapProgram>,
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

/// The map of these programs (the current ones). `curriculum`: `queries::curriculum_links`.
pub fn program_map(programs: &[Program], curriculum: &[(String, String)]) -> ProgramMap {
    let (modules, links) = links(programs, curriculum);
    let wide = layout(programs, &modules, &links, Sheet::Wide);
    let tall = layout(programs, &modules, &links, Sheet::Tall);
    let programs = programs
        .iter()
        .zip(modules.iter())
        .map(|(program, modules)| MapProgram {
            slug: program.slug.clone(),
            name: program.name.trim().to_string(),
            degree: program.degree().to_string(),
            variant: program.study_variant.as_ref().map(|variant| variant.label().to_string()),
            cycle: Cycle::of(program),
            modules: *modules,
        })
        .collect();
    ProgramMap { programs, links, wide, tall }
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

fn layout(programs: &[Program], modules: &[usize], links: &[Link], sheet: Sheet) -> Layout {
    let n = programs.len();
    let (width, height) = sheet.size();
    if n == 0 {
        return Layout { width, height, font: sheet.font(), dots: Vec::new(), names: Vec::new() };
    }
    let radius: Vec<f64> = modules.iter().map(|&m| sheet.dot() * (4.0 + 9.0 * ((m.min(160) as f64) / 160.0).sqrt())).collect();

    // The simulation runs in a space as high as the sheet whose x axis is compressed.
    let squeeze = (width / height).max(0.2) * 0.95;
    let (space_w, space_h) = (width / squeeze, height);
    let k = (space_w * space_h / n as f64).sqrt() * 0.5;
    // A sunflower: evenly spread, no two dots at the same place, no randomness.
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
                let f = k * k / (ux * ux + uy * uy + 0.01);
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
            let f = ((ux * ux + uy * uy).sqrt() + 0.01) / k * (0.4 + 1.6 * link.similarity);
            if let Some(p) = push.get_mut(link.a) {
                p.0 -= ux * f;
                p.1 -= uy * f;
            }
            if let Some(p) = push.get_mut(link.b) {
                p.0 += ux * f;
                p.1 += uy * f;
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

    // Onto the sheet: both axes fill it.
    let margin = 24.0;
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
        for (place, r) in at.iter_mut().zip(radius.iter()) {
            place.0 = place.0.clamp(r + 4.0, width - r - 4.0);
            place.1 = place.1.clamp(r + 4.0, height - r - 4.0);
        }
        if !moved {
            break;
        }
    }

    let dots: Vec<(f64, f64, f64)> = at.iter().zip(radius.iter()).map(|(place, r)| (round(place.0), round(place.1), round(*r))).collect();
    let names = names(programs, modules, &dots, sheet);
    Layout { width, height, font: sheet.font(), dots, names }
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
