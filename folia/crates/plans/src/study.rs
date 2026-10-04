//! „Mein Studium" (owner, 2026-10-04): the study, planned semester by semester. „Primär soll man
//! da sein Studium planen können. Also das aktuelle und zukünftige Semester": the semesters begin
//! empty and hold what the student puts into them — the rows of a Fachsemester of the
//! Regelstudienplan, what was not passed („Wiederholer"), whatever else the catalog offers.
//! Betula blocks nothing („Die Nutzer sind erwachsene Menschen"): any module goes into any
//! semester, a semester may hold more than the plan puts into it, an area more than it asks. What
//! Betula knows it says instead: a semester that does not offer a module, a module taken again,
//! credits beyond what an area asks („über Bedarf").
//!
//! Stored is what the student says, nothing else: the modules and the rows of the plan put into a
//! semester (`m`, `p` of `PlanDoc`, the lines of the Stundenplan: a semester's timetable is its
//! part of the study), what is passed (`d`, `q`), and of „Mein Studiengang" the Studienbeginn,
//! the semesters of leave and the last semester (`MineDoc`). Everything else is worked out here:
//!
//! - the semesters, from the Studienbeginn to the end of the Regelstudienzeit, on to the last one
//!   that holds anything and to the one the student set; each with its Fachsemester, none for a
//!   semester of leave;
//! - what each semester holds: what was passed there, what was not passed in a semester that is
//!   over (it stays open as a „Wiederholer" until it is planned again), what is taken again;
//! - the progress by area: the plan's rows grouped as its regulation groups them (`areas`), and
//!   every module passed or planned counted towards the area it belongs to, beyond what the area
//!   asks as „über Bedarf".

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Code, ModuleKind, PrerequisiteKind, Season, TurnusParity, TurnusSeason};
use folia_model::rows::{CatalogRow, Prerequisite};
use folia_model::rows_detail::{PlanEntry, PlanTotal};
use folia_routes::url;

use crate::areas::{self, CatalogArea};
use crate::plan;
use crate::studyplan::{self, DoneRow, Import, PlanDoc, Placeholder};
use crate::variants::PlanVariant;

/// How many semesters after one a turnus is looked for: a module of every other year comes again
/// within four.
const LOOK_AHEAD: i32 = 4;

/// The most areas the progress is split into; the smallest beyond them go together („Weitere").
const MAX_AREAS: usize = 9;

/// The last Fachsemester counted.
const MAX_FS: u8 = 30;

/// Less than this is no credit left (sums of halves).
const EPSILON: f64 = 1e-6;

/// When a module is offered, as the catalog states its turnus. What it does not state does not
/// restrict (R12: not known to be limited is not limited).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub season: Option<TurnusSeason>,
    pub parity: Option<TurnusParity>,
}

impl Offer {
    pub fn of(row: &CatalogRow) -> Self {
        Offer { season: row.turnus_season.as_ref().and_then(Code::known), parity: row.turnus_parity.as_ref().and_then(Code::known) }
    }

    /// Whether semester `s` offers it. A turnus of one season names the half of the year; one of
    /// even or odd years counts by the year the semester begins in (WiSe 2027/28: 2027).
    pub fn offered(self, s: SemesterKey) -> bool {
        let season = match self.season {
            Some(TurnusSeason::Winter) => s.winter,
            Some(TurnusSeason::Summer) => !s.winter,
            _ => true,
        };
        let parity = match self.parity {
            Some(TurnusParity::Even) => s.year.is_multiple_of(2),
            Some(TurnusParity::Odd) => !s.year.is_multiple_of(2),
            None => true,
        };
        season && parity
    }

    /// The first semester from `from` on that offers it; `from` itself where none of the next
    /// ones does (a turnus no semester fits).
    pub fn next(self, from: SemesterKey) -> SemesterKey {
        (0..LOOK_AHEAD).filter_map(|n| from.plus(n)).find(|s| self.offered(*s)).unwrap_or(from)
    }

    /// The one half of the year it is offered in, where its turnus names one: „nur im WiSe".
    pub fn only(self) -> Option<Season> {
        match self.season {
            Some(TurnusSeason::Winter) => Some(Season::Winter),
            Some(TurnusSeason::Summer) => Some(Season::Summer),
            _ => None,
        }
    }
}

/// The Fachsemester semester `s` is of a study begun in `start`, the semesters of `leave` not
/// counted; `None` for a semester of leave, one before the start, and one past the 30th.
pub fn fs_of(start: SemesterKey, s: SemesterKey, leave: &BTreeSet<SemesterKey>) -> Option<u8> {
    if s < start || leave.contains(&s) {
        return None;
    }
    let mut n: u8 = 0;
    let mut at = start;
    while at <= s {
        if !leave.contains(&at) {
            n = n.checked_add(1)?;
        }
        at = at.plus(1)?;
    }
    (n <= MAX_FS).then_some(n)
}

/// The semester that is Fachsemester `fs` of a study begun in `start` (`fs_of` the other way).
pub fn semester_of_fs(start: SemesterKey, fs: u8, leave: &BTreeSet<SemesterKey>) -> Option<SemesterKey> {
    if fs == 0 || fs > MAX_FS {
        return None;
    }
    let mut n: u8 = 0;
    let mut at = start;
    // Every semester of leave adds one; there are at most `MAX_SEMESTERS` of them.
    for _ in 0..(usize::from(MAX_FS) + studyplan::MAX_SEMESTERS) {
        if !leave.contains(&at) {
            n += 1;
            if n == fs {
                return Some(at);
            }
        }
        at = at.plus(1)?;
    }
    None
}

/// A row of the plan the study counts: the plan it comes from and its first and last
/// Fachsemester.
#[derive(Clone, Copy, Debug)]
pub struct PlanRow<'a> {
    pub variant: &'a PlanVariant,
    pub entry: &'a PlanEntry,
    pub first: u8,
    pub last: u8,
}

/// The rows of the plan `core` with the page that fills a row of it (`variants::supplements`, with
/// that row's `ord`: the row it fills is no row of its own), in the order of the document. A row
/// of prose, with neither credits nor a kind, is no part of it.
pub fn plan_rows<'a>(core: Option<&'a PlanVariant>, page: Option<(&'a PlanVariant, i64)>) -> Vec<PlanRow<'a>> {
    let Some(core) = core else { return Vec::new() };
    let parts = std::iter::once((core, page.map(|(_, filled)| filled))).chain(page.map(|(page, _)| (page, None)));
    let mut rows = Vec::new();
    for (variant, filled) in parts {
        for entry in variant.entries.iter().filter(|entry| Some(entry.ord) != filled) {
            if plan::credits_of(entry).is_none() && entry.kind.is_none() {
                continue;
            }
            let Some((first, last)) = plan::semester_span(entry) else { continue };
            let (Ok(first), Ok(last)) = (u8::try_from(first), u8::try_from(last)) else { continue };
            if first == 0 {
                continue;
            }
            rows.push(PlanRow { variant, entry, first, last });
        }
    }
    rows
}

/// What kind of part of the progress an area is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaKind {
    /// A group of the plan's rows by their name.
    Plan,
    /// The FÜS („Fachübergreifendes Studium"), whatever the plan names it.
    Fues,
    /// The thesis.
    Thesis,
    /// What is left of a plan of many small groups (`MAX_AREAS`), or rows that name no group.
    Other,
}

/// A part of the progress: rows of the plan that its regulation groups together, with what they
/// ask for.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// What the bar and the cards call it („Informatik" for „Komplex Informatik"); empty for the
    /// kinds the page names itself (FÜS, thesis, the others).
    pub name: String,
    /// The plan's own name for it („Komplex Informatik"); empty as `name`.
    pub full: String,
    pub kind: AreaKind,
    /// What the plan asks of it, in credits: its rows', a choice between rows by the sum the
    /// regulation prints over them („Summe Komplexe des Fachstudiums 44").
    pub required: f64,
    /// Its rows: the caption of their plan and their `PlanEntry::ord`.
    pub rows: Vec<(String, i64)>,
    /// The areas of the program's module tree whose modules count here, with the areas below them.
    pub tree: BTreeSet<i64>,
}

/// What decides which area a row of the plan belongs to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Thesis,
    Fues,
    /// The row's `subject_area`, else the top of the module tree where its module or its choice
    /// lies, else its `study_section`.
    Named(String),
    Other,
}

/// „Komplex Informatik" → „Informatik", „Wahlpflichtmodule Praktische Mathematik" → „Praktische
/// Mathematik": the name of an area without the word that only says it is one.
pub fn area_name(full: &str) -> &str {
    let full = full.trim();
    for prefix in ["Komplex ", "Modulkomplex ", "Modulbereich ", "Bereich "] {
        if full.len() > prefix.len() && full.is_char_boundary(prefix.len()) && full[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return areas::short_name(full[prefix.len()..].trim_start());
        }
    }
    areas::short_name(full)
}

/// The label of the top of the module tree above `area` (itself at the top).
fn top_label(area: &CatalogArea) -> String {
    area.ancestors.first().cloned().unwrap_or_else(|| area.label.clone())
}

fn key_of(row: &PlanRow, tree: &[CatalogArea], by_id: &BTreeMap<i64, &CatalogArea>, placed: &BTreeMap<&str, Vec<i64>>) -> Key {
    let entry = row.entry;
    let kind = entry.kind.as_ref().and_then(Code::known);
    if kind == Some(ModuleKind::Thesis) {
        return Key::Thesis;
    }
    if plan::is_fues(entry) {
        return Key::Fues;
    }
    if let Some(subject) = entry.subject_area.as_deref().map(str::trim).filter(|subject| !subject.is_empty()) {
        return Key::Named(subject.to_string());
    }
    if let Some(top) = entry.module_id.as_deref().and_then(|id| placed.get(id)).and_then(|areas| areas.iter().find_map(|id| by_id.get(id).map(|area| top_label(area)))) {
        return Key::Named(top);
    }
    if entry.module_id.is_none() {
        if let Some(top) = plan::areas_for_row(entry, &row.variant.full, tree, &row.variant.entries).areas.first().map(top_label) {
            return Key::Named(top);
        }
    }
    match entry.study_section.as_deref().map(str::trim).filter(|section| !section.is_empty()) {
        Some(section) => Key::Named(section.to_string()),
        None => Key::Other,
    }
}

/// The root of `i` among the sets of `parent`.
fn root(parent: &[usize], mut i: usize) -> usize {
    while let Some(&up) = parent.get(i) {
        if up == i {
            break;
        }
        i = up;
    }
    i
}

/// One set of the sets `a` and `b` are in.
fn join(parent: &mut [usize], a: usize, b: usize) {
    let (a, b) = (root(parent, a), root(parent, b));
    if a != b {
        if let Some(slot) = parent.get_mut(a.max(b)) {
            *slot = a.min(b);
        }
    }
}

/// The sum the regulation prints over a row with a range of credits and the rows it is chosen
/// with (`plan::choice_of`).
fn choice_total<'a>(row: &PlanRow<'a>) -> Option<&'a PlanTotal> {
    plan::choice_of(&row.variant.totals.iter().collect::<Vec<_>>(), row.entry)
}

/// What `rows` ask for together: each row its credits (the least of a range), a choice between
/// rows the sum the regulation prints over them less the rows of fixed credits it counts.
fn required_of(rows: &[&PlanRow]) -> f64 {
    let mut sum = 0.0;
    let mut sums: Vec<(&str, i64)> = Vec::new();
    for row in rows {
        match choice_total(row) {
            Some(total) => {
                if !sums.contains(&(row.variant.full.as_str(), total.ord)) {
                    sums.push((row.variant.full.as_str(), total.ord));
                    let fixed: f64 = row
                        .variant
                        .entries
                        .iter()
                        .filter(|entry| total.entries.contains(&entry.ord) && entry.max_credits.is_none())
                        .filter_map(|entry| entry.credits)
                        .sum();
                    sum += (total.credits - fixed).max(0.0);
                }
            }
            None => sum += row.entry.credits.or(row.entry.min_credits).unwrap_or(0.0),
        }
    }
    sum
}

/// The areas the progress of a study of `rows` is split into, in the order of the plan.
///
/// A row belongs to the thesis, to the FÜS, or to a group by name: its `subject_area` (the plan of
/// Informatik B.Sc. names „Komplex Informatik", „Komplex Mathematik" …), else the top of the
/// program's module tree where its module or its choice lies (`placements`, `tree`), else its
/// `study_section`. Rows the regulation lets the student choose between („Komplex Grundlagen der
/// Informatik, 10–24 LP" and its two neighbours, „Summe Komplexe des Fachstudiums 44") are one
/// group, and that group joins the group of their section where there is one („Fachstudium"). A
/// plan of more than `MAX_AREAS` groups has its smallest ones together.
///
/// Each area holds the tree's areas whose modules count towards it: the ones named like it and the
/// ones its rows' modules and choices lie in, with every area below them.
pub fn areas(rows: &[PlanRow], tree: &[CatalogArea], placements: &[(String, i64)]) -> Vec<Area> {
    let by_id: BTreeMap<i64, &CatalogArea> = tree.iter().map(|area| (area.id, area)).collect();
    let mut placed: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (module, area) in placements {
        placed.entry(module.as_str()).or_default().push(*area);
    }
    let keys: Vec<Key> = rows.iter().map(|row| key_of(row, tree, &by_id, &placed)).collect();
    let mut distinct = keys.clone();
    distinct.sort();
    distinct.dedup();
    let index = |key: &Key| distinct.binary_search(key).ok();
    let mut parent: Vec<usize> = (0..distinct.len()).collect();
    let mergeable = |key: &Key| matches!(key, Key::Named(_) | Key::Other);
    for (i, row) in rows.iter().enumerate() {
        let Some(key) = keys.get(i).filter(|key| mergeable(key)) else { continue };
        let Some(total) = choice_total(row) else { continue };
        let Some(at) = index(key) else { continue };
        for (j, other) in rows.iter().enumerate() {
            let same_sum = std::ptr::eq(other.variant, row.variant) && total.entries.contains(&other.entry.ord);
            if let Some(there) = keys.get(j).filter(|key| same_sum && mergeable(key)).and_then(&index) {
                join(&mut parent, at, there);
            }
        }
        if let Some(section) = row.entry.study_section.as_deref().map(str::trim).filter(|section| !section.is_empty()) {
            if let Some(there) = index(&Key::Named(section.to_string())) {
                join(&mut parent, at, there);
            }
        }
    }

    // The groups, in the order of their first row.
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let Some(set) = index(key).map(|at| root(&parent, at)) else { continue };
        match groups.iter_mut().find(|(known, _)| *known == set) {
            Some((_, members)) => members.push(i),
            None => groups.push((set, vec![i])),
        }
    }
    let mut made: Vec<(Key, Vec<usize>)> = groups
        .into_iter()
        .map(|(_, members)| {
            // A group is named by the name most of its rows carry, the shorter of two.
            let mut counts: BTreeMap<&Key, usize> = BTreeMap::new();
            for i in &members {
                if let Some(key) = keys.get(*i) {
                    *counts.entry(key).or_default() += 1;
                }
            }
            let named = counts
                .iter()
                .max_by(|(a, n), (b, m)| n.cmp(m).then_with(|| key_len(b).cmp(&key_len(a))))
                .map(|(key, _)| (*key).clone())
                .unwrap_or(Key::Other);
            (named, members)
        })
        .collect();
    if made.len() > MAX_AREAS {
        let rows_of = |members: &[usize]| members.iter().filter_map(|i| rows.get(*i)).collect::<Vec<_>>();
        let mut by_size: Vec<usize> = (0..made.len()).collect();
        by_size.sort_by(|a, b| {
            let size = |i: &usize| made.get(*i).map_or(0.0, |(_, members)| required_of(&rows_of(members)));
            size(b).total_cmp(&size(a))
        });
        let small: BTreeSet<usize> = by_size.into_iter().skip(MAX_AREAS - 1).collect();
        let mut rest: Vec<usize> = Vec::new();
        let mut kept = Vec::new();
        for (i, group) in made.into_iter().enumerate() {
            if small.contains(&i) {
                rest.extend(group.1);
            } else {
                kept.push(group);
            }
        }
        rest.sort_unstable();
        kept.push((Key::Other, rest));
        made = kept;
    }

    made.into_iter()
        .map(|(key, members)| {
            let members: Vec<&PlanRow> = members.iter().filter_map(|i| rows.get(*i)).collect();
            let (kind, full) = match &key {
                Key::Thesis => (AreaKind::Thesis, String::new()),
                Key::Fues => (AreaKind::Fues, String::new()),
                Key::Named(name) => (AreaKind::Plan, name.clone()),
                Key::Other => (AreaKind::Other, String::new()),
            };
            let tree_of = claimed(&full, kind, &members, tree, &by_id, &placed);
            Area {
                name: area_name(&full).to_string(),
                full,
                kind,
                required: required_of(&members),
                rows: members.iter().map(|row| (row.variant.full.clone(), row.entry.ord)).collect(),
                tree: tree_of,
            }
        })
        .collect()
}

fn key_len(key: &Key) -> usize {
    match key {
        Key::Named(name) => name.chars().count(),
        _ => usize::MAX,
    }
}

/// The areas of the tree whose modules count towards the area named `full` of `rows`, of `kind`:
/// the ones named like it and the ones its choices lie in, each with the areas below it, and the
/// ones its modules are placed in. Not the areas below those: a thesis placed in the Fachstudium
/// does not take the Fachstudium's modules. Nor the areas of the FÜS's and the thesis's modules:
/// what counts there is said by the FÜS list and the kind of the row.
fn claimed(full: &str, kind: AreaKind, rows: &[&PlanRow], tree: &[CatalogArea], by_id: &BTreeMap<i64, &CatalogArea>, placed: &BTreeMap<&str, Vec<i64>>) -> BTreeSet<i64> {
    let mut roots: BTreeSet<i64> = BTreeSet::new();
    let mut modules: BTreeSet<i64> = BTreeSet::new();
    if !full.is_empty() {
        roots.extend(tree.iter().filter(|area| area.label.trim().eq_ignore_ascii_case(full)).map(|area| area.id));
    }
    for row in rows {
        match row.entry.module_id.as_deref() {
            Some(id) => {
                if matches!(kind, AreaKind::Plan | AreaKind::Other) {
                    modules.extend(placed.get(id).into_iter().flatten().copied());
                }
            }
            None => roots.extend(plan::areas_for_row(row.entry, &row.variant.full, tree, &row.variant.entries).areas.iter().map(|area| area.id)),
        }
    }
    let tops: Vec<&CatalogArea> = roots.iter().filter_map(|id| by_id.get(id).copied()).collect();
    let mut all = roots;
    all.extend(tree.iter().filter(|area| tops.iter().any(|top| below(area, top))).map(|area| area.id));
    all.extend(modules);
    all
}

/// Whether `area` lies below `top`, by the labels above it (a label may hold the path's „ / ").
fn below(area: &CatalogArea, top: &CatalogArea) -> bool {
    let n = top.ancestors.len();
    area.ancestors.get(..n) == Some(top.ancestors.as_slice()) && area.ancestors.get(n) == Some(&top.label)
}

/// What the study is worked out from.
#[derive(Clone, Copy)]
pub struct Input<'a> {
    pub program_id: &'a str,
    /// The plan of the chosen study direction; `None` for a program without one, whose study is
    /// what the student planned and passed.
    pub core: Option<&'a PlanVariant>,
    /// A page that fills a row of `core` (`variants::supplements`), with the `ord` of that row.
    pub page: Option<(&'a PlanVariant, i64)>,
    pub start: SemesterKey,
    /// The current semester.
    pub now: SemesterKey,
    /// The semesters of leave.
    pub leave: &'a BTreeSet<SemesterKey>,
    /// The last semester the student set, where it lies beyond the Regelstudienzeit.
    pub until: Option<SemesterKey>,
    pub doc: &'a PlanDoc,
    /// The catalog's rows of the plan's modules and of the ones planned and passed: their titles,
    /// credits and turnus.
    pub rows: &'a [CatalogRow],
    /// The program's module tree, where it places its modules, and its FÜS list.
    pub tree: &'a [CatalogArea],
    pub placements: &'a [(String, i64)],
    pub fues: &'a [String],
}

/// What an item of a semester is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    Module { id: String },
    /// A row of a plan without a module, put into the semester (a placeholder, `pid`) or done
    /// there without one (`q` alone). `span` and `credits` as the placeholder keeps them.
    Row { pid: Option<u32>, program_id: String, caption: String, ord: i64, name: String, span: (u8, u8), credits: Option<String> },
}

/// One module or row of a semester.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub subject: Subject,
    /// The catalog's title of a module, else the plan's name of it; a row's name.
    pub name: String,
    /// What it is worth: the plan's, else the catalog's; for a row of a range the least it takes.
    pub credits: Option<f64>,
    /// A row of a range of credits („10–24").
    pub range: bool,
    pub kind: Option<ModuleKind>,
    /// The area it counts towards (`Study::progress`).
    pub area: Option<usize>,
    /// Passed („bestanden") or done in this semester.
    pub passed: bool,
    /// In a semester that is over and not passed there („nicht bestanden").
    pub failed: bool,
    /// Taken again: the semester before this one it was not passed in.
    pub retake: Option<SemesterKey>,
    /// Not passed here, and planned or passed in this later semester.
    pub again: Option<SemesterKey>,
    pub offer: Offer,
    /// The semester does not offer it (its turnus).
    pub unoffered: bool,
    /// The credits it counts beyond what its area asks.
    pub over: f64,
    /// The Fachsemester the Regelstudienplan puts it into.
    pub plan_fs: Option<u8>,
    /// The modules that fill a row (`Planned::fills`): then they count, not the row.
    pub fillers: Vec<String>,
}

impl Item {
    pub fn module_id(&self) -> Option<&str> {
        match &self.subject {
            Subject::Module { id } => Some(id),
            Subject::Row { .. } => None,
        }
    }

    /// The same item in every semester it is in: `m:<id>` for a module, `r:<caption>:<ord>` for a
    /// row.
    pub fn key(&self) -> String {
        match &self.subject {
            Subject::Module { id } => format!("m:{id}"),
            Subject::Row { caption, ord, .. } => format!("r:{caption}:{ord}"),
        }
    }
}

/// Where a semester stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum When {
    Past,
    Now,
    Later,
}

/// One semester of the study.
#[derive(Clone, Debug, PartialEq)]
pub struct Semester {
    pub key: SemesterKey,
    /// Its Fachsemester; `None` for a semester of leave, one before the Studienbeginn, one past
    /// the 30th.
    pub fs: Option<u8>,
    pub leave: bool,
    pub when: When,
    /// What is taken again first, then in the order it was put in; modules before rows.
    pub items: Vec<Item>,
    /// What its items come to, each with one number of its own; what of it is passed.
    pub credits: f64,
    pub passed: f64,
    /// Some item states no single number (none, a range).
    pub partial: bool,
    /// What the Regelstudienplan puts into its Fachsemester.
    pub planned: Option<f64>,
    /// After the last Fachsemester of the Regelstudienplan.
    pub beyond: bool,
}

/// How far an area of the study is.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub area: Area,
    /// What counts towards it, passed and planned, up to what it asks.
    pub passed: f64,
    pub planned: f64,
    /// What is passed or planned beyond what it asks; what of it is passed.
    pub over: f64,
    pub over_passed: f64,
}

impl Progress {
    /// What it still asks for that is not planned.
    pub fn open(&self) -> f64 {
        (self.area.required - self.passed - self.planned).max(0.0)
    }
}

/// A module or row not passed in a semester that is over and not planned again („Wiederholer").
#[derive(Clone, Debug, PartialEq)]
pub struct Retake {
    pub item: Item,
    /// The last semester it was not passed in.
    pub failed_in: SemesterKey,
}

/// The study as the student planned it.
#[derive(Clone, Debug, PartialEq)]
pub struct Study {
    pub start: SemesterKey,
    /// The current semester, not before the Studienbeginn.
    pub now: SemesterKey,
    pub semesters: Vec<Semester>,
    /// By area, in the order of the plan; empty without a plan.
    pub progress: Vec<Progress>,
    /// What counts towards no area: passed, planned.
    pub outside: (f64, f64),
    /// What the plan asks for in all: what its areas ask; without a plan 0.
    pub total: f64,
    /// What counts towards the areas, passed and planned; what goes beyond them, and what of that
    /// is passed.
    pub passed: f64,
    pub planned: f64,
    pub over: f64,
    pub over_passed: f64,
    /// What the areas still ask for that is not planned.
    pub open: f64,
    pub retakes: Vec<Retake>,
    /// The semester of the Regelstudienplan's last Fachsemester.
    pub regular_end: Option<SemesterKey>,
}

impl Study {
    pub fn semester(&self, s: SemesterKey) -> Option<&Semester> {
        self.semesters.iter().find(|semester| semester.key == s)
    }

    /// The semesters from the current one on.
    pub fn ahead(&self) -> impl Iterator<Item = &Semester> {
        self.semesters.iter().filter(|semester| semester.when != When::Past)
    }
}

/// „6" → (6, 6), „10–24" → (10, 24).
fn credits_range(text: &str) -> Option<(f64, f64)> {
    let number = |part: &str| part.trim().replace(',', ".").parse::<f64>().ok();
    match text.split_once(['–', '-']) {
        Some((low, high)) => Some((number(low)?, number(high)?)),
        None => number(text).map(|n| (n, n)),
    }
}

/// The study of `input`.
pub fn study(input: &Input) -> Study {
    let start = input.start;
    let now = input.now.max(start);
    let doc = input.doc;
    let rows = plan_rows(input.core, input.page);
    let areas = areas(&rows, input.tree, input.placements);
    let catalog: BTreeMap<&str, &CatalogRow> = input.rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let mut module_rows: BTreeMap<&str, &PlanRow> = BTreeMap::new();
    for row in &rows {
        if let Some(id) = row.entry.module_id.as_deref().filter(|id| url::is_module_id(id)) {
            module_rows.entry(id).or_insert(row);
        }
    }
    let regular_end = rows.iter().map(|row| row.last).max().and_then(|last| semester_of_fs(start, last, input.leave));

    // From the Studienbeginn, or the first semester anything is in, to the last one.
    let used: Vec<SemesterKey> = doc
        .modules
        .iter()
        .map(|m| m.semester)
        .chain(doc.placeholders.iter().map(|p| p.semester))
        .chain(doc.passed.iter().map(|p| p.semester))
        .chain(doc.done_rows.iter().map(|q| q.semester))
        .collect();
    let first = used.iter().copied().min().map_or(start, |first| first.min(start));
    let last = [Some(now), regular_end, used.iter().copied().max(), input.until].into_iter().flatten().max().unwrap_or(now);

    let module_item = |id: &str, s: SemesterKey| -> Item {
        let linked = catalog.get(id).copied();
        let row = module_rows.get(id).copied();
        let offer = linked.map(Offer::of).unwrap_or_default();
        let passed_in = doc.passed_in(id);
        Item {
            subject: Subject::Module { id: id.to_string() },
            name: linked.map(|row| row.title.clone()).or_else(|| row.map(|row| plan::shown_name(&row.entry.module_name).to_string())).unwrap_or_else(|| id.to_string()),
            credits: row.and_then(|row| row.entry.credits.or(row.entry.min_credits)).or(linked.and_then(|row| row.credits)),
            range: false,
            kind: row.and_then(|row| row.entry.kind.as_ref().and_then(Code::known)).or(linked.and_then(|row| row.kind.as_ref().and_then(Code::known))),
            area: None,
            passed: passed_in == Some(s),
            failed: s < now && passed_in != Some(s),
            retake: doc.planned_in(id).into_iter().filter(|earlier| *earlier < s && passed_in != Some(*earlier)).max(),
            again: doc.planned_in(id).into_iter().chain(passed_in).filter(|later| *later > s).min(),
            offer,
            unoffered: !offer.offered(s),
            over: 0.0,
            plan_fs: row.map(|row| row.first),
            fillers: Vec::new(),
        }
    };
    let row_item = |pid: Option<u32>, program_id: &str, caption: &str, ord: i64, name: &str, span: (u8, u8), credits: Option<String>, kind: Option<&str>, s: SemesterKey| -> Item {
        let (low, high) = credits.as_deref().and_then(credits_range).map_or((None, false), |(low, high)| (Some(low), high > low));
        let done = doc.done_row(program_id, ord).map(|done| done.semester);
        Item {
            subject: Subject::Row { pid, program_id: program_id.to_string(), caption: caption.to_string(), ord, name: name.to_string(), span, credits },
            name: name.to_string(),
            credits: low,
            range: high,
            kind: kind.and_then(|code| Code::<ModuleKind>::parse(code).known()),
            area: None,
            passed: done == Some(s),
            failed: s < now && done != Some(s),
            retake: doc
                .placeholders
                .iter()
                .filter(|p| p.program_id == program_id && p.ord == ord && p.semester < s && done != Some(p.semester))
                .map(|p| p.semester)
                .max(),
            again: doc
                .placeholders
                .iter()
                .filter(|p| p.program_id == program_id && p.ord == ord)
                .map(|p| p.semester)
                .chain(done)
                .filter(|later| *later > s)
                .min(),
            offer: Offer::default(),
            unoffered: false,
            over: 0.0,
            plan_fs: (span.0 > 0).then_some(span.0),
            fillers: pid.map(|pid| doc.fillers(pid).into_iter().filter(|m| m.semester == s).map(|m| m.module_id.clone()).collect()).unwrap_or_default(),
        }
    };

    let mut semesters = Vec::new();
    let mut s = first;
    while s <= last {
        let mut items: Vec<Item> = doc.modules.iter().filter(|m| m.semester == s).map(|m| module_item(&m.module_id, s)).collect();
        for passed in doc.passed.iter().filter(|p| p.semester == s && !doc.is_planned(s, &p.module_id)) {
            items.push(module_item(&passed.module_id, s));
        }
        for p in doc.placeholders.iter().filter(|p| p.semester == s) {
            items.push(row_item(Some(p.pid), &p.program_id, &p.caption, p.ord, &p.name, p.span, p.credits.clone(), p.kind.as_deref(), s));
        }
        for done in doc.done_rows.iter().filter(|q| q.semester == s) {
            if !doc.placeholders.iter().any(|p| p.semester == s && p.program_id == done.program_id && p.ord == done.ord) {
                let row = rows.iter().find(|row| row.variant.full == done.caption && row.entry.ord == done.ord);
                let span = row.map_or((0, 0), |row| (row.first, row.last));
                let credits = row.and_then(|row| plan::credits_of(row.entry));
                items.push(row_item(None, &done.program_id, &done.caption, done.ord, &done.name, span, credits, None, s));
            }
        }
        // What is taken again first; the rest keeps the order it was put in.
        items.sort_by_key(|item| item.retake.is_none() || s < now);
        let fs = fs_of(start, s, input.leave);
        semesters.push(Semester {
            key: s,
            fs,
            leave: input.leave.contains(&s),
            when: match s.cmp(&now) {
                std::cmp::Ordering::Less => When::Past,
                std::cmp::Ordering::Equal => When::Now,
                std::cmp::Ordering::Greater => When::Later,
            },
            items,
            credits: 0.0,
            passed: 0.0,
            partial: false,
            planned: fs.zip(input.core).filter(|(fs, core)| i64::from(*fs) <= core.semesters).and_then(|(fs, core)| core.semester_credits(i64::from(fs))),
            beyond: regular_end.is_some_and(|end| s > end),
        });
        let Some(next) = s.plus(1) else { break };
        s = next;
    }

    // Each item's area: the one of its plan row, else the ones the tree places its module in (the
    // first with room left), else the FÜS for a module of the FÜS list or the catalog's FÜS.
    let fues_area = areas.iter().position(|area| area.kind == AreaKind::Fues);
    let fues: BTreeSet<&str> = input.fues.iter().map(String::as_str).collect();
    let row_area = |caption: &str, ord: i64| areas.iter().position(|area| area.rows.iter().any(|(c, o)| c == caption && *o == ord));
    let candidates = |item: &Item| -> Vec<usize> {
        match &item.subject {
            Subject::Row { caption, ord, .. } => row_area(caption, *ord).into_iter().collect(),
            Subject::Module { id } => {
                if let Some(row) = module_rows.get(id.as_str()) {
                    return row_area(&row.variant.full, row.entry.ord).into_iter().collect();
                }
                let placed: Vec<i64> = input.placements.iter().filter(|(module, _)| module == id).map(|(_, area)| *area).collect();
                let found: Vec<usize> = areas.iter().enumerate().filter(|(_, area)| placed.iter().any(|at| area.tree.contains(at))).map(|(i, _)| i).collect();
                if !found.is_empty() {
                    return found;
                }
                let is_fues = fues.contains(id.as_str()) || catalog.get(id.as_str()).is_some_and(|row| row.is_fues);
                fues_area.filter(|_| is_fues).into_iter().collect()
            }
        }
    };
    let mut used = vec![0.0_f64; areas.len()];
    let mut progress: Vec<Progress> = areas.iter().map(|area| Progress { area: area.clone(), passed: 0.0, planned: 0.0, over: 0.0, over_passed: 0.0 }).collect();
    let mut outside = (0.0, 0.0);
    let mut order: Vec<(bool, usize, usize)> = Vec::new();
    for (si, semester) in semesters.iter().enumerate() {
        for (ii, item) in semester.items.iter().enumerate() {
            order.push((!item.passed, si, ii));
        }
    }
    order.sort_unstable();
    for (_, si, ii) in order {
        let Some(item) = semesters.get_mut(si).and_then(|semester| semester.items.get_mut(ii)) else { continue };
        let found = candidates(item);
        item.area = found.first().copied();
        let counts = item.passed || !item.failed;
        let credits = if item.fillers.is_empty() { item.credits.unwrap_or(0.0) } else { 0.0 };
        if !counts || credits <= 0.0 {
            continue;
        }
        let Some(&fallback) = found.first() else {
            if item.passed {
                outside.0 += credits;
            } else {
                outside.1 += credits;
            }
            continue;
        };
        let room = |at: usize| areas.get(at).map_or(0.0, |area| area.required) - used.get(at).copied().unwrap_or(0.0);
        let target = found.iter().copied().find(|at| room(*at) > EPSILON).unwrap_or(fallback);
        let counted = credits.min(room(target).max(0.0));
        if let Some(slot) = used.get_mut(target) {
            *slot += counted;
        }
        if let Some(progress) = progress.get_mut(target) {
            if item.passed {
                progress.passed += counted;
            } else {
                progress.planned += counted;
            }
            progress.over += credits - counted;
            if item.passed {
                progress.over_passed += credits - counted;
            }
        }
        item.area = Some(target);
        item.over = credits - counted;
    }
    for semester in &mut semesters {
        semester.credits = semester.items.iter().filter_map(|item| item.credits).sum();
        semester.passed = semester.items.iter().filter(|item| item.passed).filter_map(|item| item.credits).sum();
        semester.partial = semester.items.iter().any(|item| item.credits.is_none() || item.range);
    }

    // What was not passed and is not planned again.
    let mut retakes: Vec<Retake> = Vec::new();
    for semester in semesters.iter().rev() {
        for item in semester.items.iter().filter(|item| item.failed) {
            let again = match &item.subject {
                Subject::Module { id } => doc.passed_in(id).is_some() || doc.planned_in(id).into_iter().any(|s| s >= now),
                Subject::Row { program_id, ord, .. } => {
                    doc.done_row(program_id, *ord).is_some() || doc.placeholders.iter().any(|p| p.program_id == *program_id && p.ord == *ord && p.semester >= now)
                }
            };
            if !again && !retakes.iter().any(|known| known.item.key() == item.key()) {
                retakes.push(Retake { item: item.clone(), failed_in: semester.key });
            }
        }
    }
    retakes.reverse();

    let total = areas.iter().map(|area| area.required).sum();
    Study {
        start,
        now,
        passed: progress.iter().map(|p| p.passed).sum(),
        planned: progress.iter().map(|p| p.planned).sum(),
        over: progress.iter().map(|p| p.over).sum(),
        over_passed: progress.iter().map(|p| p.over_passed).sum(),
        open: progress.iter().map(Progress::open).sum(),
        semesters,
        progress,
        outside,
        total,
        retakes,
        regular_end,
    }
}

/// What a pick puts into a semester.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A module; `from_plan` when it comes from a row of the Regelstudienplan.
    Module { id: String, from_plan: bool },
    /// A row of the plan without a module, as a placeholder: its plan's caption and its `ord`.
    Row { caption: String, ord: i64 },
}

/// Where a row of the plan stands in the study.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    Open,
    /// Put into these semesters, not passed.
    Planned(Vec<SemesterKey>),
    Passed(SemesterKey),
}

/// A row of the plan, as the picker offers it.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub pick: Pick,
    pub name: String,
    pub credits: Option<f64>,
    /// What the plan says of its credits („6", „10–24").
    pub credits_text: Option<String>,
    pub area: Option<usize>,
    pub kind: Option<ModuleKind>,
    pub offer: Offer,
    pub standing: Standing,
}

/// The rows of the plan in Fachsemester `fs`, as the picker offers them for a semester: each
/// module once, with where it stands.
pub fn plan_semester(input: &Input, study: &Study, fs: u8) -> Vec<Suggestion> {
    let doc = input.doc;
    let catalog: BTreeMap<&str, &CatalogRow> = input.rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let mut named: BTreeSet<&str> = BTreeSet::new();
    let area_of = |caption: &str, ord: i64| study.progress.iter().position(|p| p.area.rows.iter().any(|(c, o)| c == caption && *o == ord));
    let mut out = Vec::new();
    for row in plan_rows(input.core, input.page).into_iter().filter(|row| row.first <= fs && fs <= row.last) {
        let entry = row.entry;
        let area = area_of(&row.variant.full, entry.ord);
        let credits_text = plan::credits_of(entry);
        match entry.module_id.as_deref().filter(|id| url::is_module_id(id)) {
            Some(id) => {
                if !named.insert(id) {
                    continue;
                }
                let linked = catalog.get(id).copied();
                let standing = match doc.passed_in(id) {
                    Some(s) => Standing::Passed(s),
                    None => match doc.planned_in(id) {
                        planned if planned.is_empty() => Standing::Open,
                        planned => Standing::Planned(planned),
                    },
                };
                out.push(Suggestion {
                    pick: Pick::Module { id: id.to_string(), from_plan: true },
                    name: linked.map(|row| row.title.clone()).unwrap_or_else(|| plan::shown_name(&entry.module_name).to_string()),
                    credits: entry.credits.or(entry.min_credits).or(linked.and_then(|row| row.credits)),
                    credits_text,
                    area,
                    kind: entry.kind.as_ref().and_then(Code::known),
                    offer: linked.map(Offer::of).unwrap_or_default(),
                    standing,
                });
            }
            None => {
                let program = input.program_id;
                let standing = match doc.done_row(program, entry.ord) {
                    Some(done) => Standing::Passed(done.semester),
                    None => {
                        let planned: Vec<SemesterKey> = doc.placeholders.iter().filter(|p| p.program_id == program && p.ord == entry.ord && p.caption == row.variant.full).map(|p| p.semester).collect();
                        if planned.is_empty() { Standing::Open } else { Standing::Planned(planned) }
                    }
                };
                out.push(Suggestion {
                    pick: Pick::Row { caption: row.variant.full.clone(), ord: entry.ord },
                    name: plan::shown_name(&entry.module_name).to_string(),
                    credits: entry.credits.or(entry.min_credits),
                    credits_text,
                    area,
                    kind: entry.kind.as_ref().and_then(Code::known),
                    offer: Offer::default(),
                    standing,
                });
            }
        }
    }
    out
}

/// Puts `picks` into semester `s`; how many it added. A module planned already into a semester
/// from the current one on moves here (the picker says so); one planned only into semesters that
/// are over is taken again. A row comes as a placeholder of its own name, credits and span.
pub fn add(doc: &mut PlanDoc, input: &Input, s: SemesterKey, picks: &[Pick], at: u64) -> usize {
    let now = input.now.max(input.start);
    let mut added = 0;
    for pick in picks {
        match pick {
            Pick::Module { id, from_plan } => {
                if !url::is_module_id(id) || doc.is_planned(s, id) {
                    continue;
                }
                let ahead = doc.planned_in(id).into_iter().find(|planned| *planned >= now && *planned != s);
                match ahead {
                    Some(from) if doc.passed_in(id).is_none() => doc.move_to(from, s, id),
                    _ => {
                        doc.plan(s, id, at, None);
                    }
                }
                if doc.is_planned(s, id) {
                    added += 1;
                    if *from_plan {
                        doc.mark_from_plan(s, id);
                    }
                }
            }
            Pick::Row { caption, ord } => {
                let rows = plan_rows(input.core, input.page);
                let Some(row) = rows.iter().find(|row| row.variant.full == *caption && row.entry.ord == *ord) else { continue };
                let name = plan::shown_name(&row.entry.module_name);
                let Some(template) = studyplan::row_placeholder(input.program_id, row.variant, row.entry, s, name) else { continue };
                let pid = doc.next_pid();
                if doc.add_placeholder(Placeholder { pid, ..template }) {
                    added += 1;
                }
            }
        }
    }
    added
}

/// Takes `item` out of semester `s`, and what was passed of it there.
pub fn remove(doc: &mut PlanDoc, s: SemesterKey, item: &Item) {
    match &item.subject {
        Subject::Module { id } => {
            doc.unplan(s, id, &[]);
            if doc.passed_in(id) == Some(s) {
                doc.unpass(id);
            }
        }
        Subject::Row { pid, program_id, ord, .. } => {
            if let Some(pid) = pid {
                doc.remove_placeholder(*pid);
            }
            if doc.done_row(program_id, *ord).is_some_and(|done| done.semester == s) {
                doc.reopen_row(program_id, *ord);
            }
        }
    }
}

/// Moves `item` from semester `from` to `to`, with what was passed of it there; whether it is in
/// `to` then.
pub fn move_item(doc: &mut PlanDoc, from: SemesterKey, to: SemesterKey, item: &Item) -> bool {
    if from == to {
        return false;
    }
    match &item.subject {
        Subject::Module { id } => {
            let passed_here = doc.passed_in(id) == Some(from);
            doc.move_to(from, to, id);
            if passed_here {
                doc.unpass(id);
                doc.pass(to, id);
            }
            doc.is_planned(to, id) || doc.passed_in(id) == Some(to)
        }
        Subject::Row { pid, program_id, caption, ord, name, .. } => {
            if let Some(pid) = pid {
                let Some(moved) = doc.placeholders.iter().find(|p| p.pid == *pid).cloned() else { return false };
                doc.remove_placeholder(*pid);
                let there = doc.placeholders.iter().any(|p| p.program_id == moved.program_id && p.ord == moved.ord && p.semester == to);
                if !there && !doc.add_placeholder(Placeholder { semester: to, ..moved.clone() }) {
                    doc.add_placeholder(moved);
                    return false;
                }
            }
            if doc.done_row(program_id, *ord).is_some_and(|done| done.semester == from) {
                doc.reopen_row(program_id, *ord);
                doc.finish_row(DoneRow { semester: to, program_id: program_id.clone(), ord: *ord, caption: caption.clone(), name: name.clone() });
            }
            true
        }
    }
}

/// Ticks `item` of semester `s` off as passed, or opens it again; whether the plan holds that.
pub fn set_passed(doc: &mut PlanDoc, s: SemesterKey, item: &Item, passed: bool) -> bool {
    match &item.subject {
        Subject::Module { id } => {
            if passed {
                doc.pass(s, id)
            } else {
                doc.unpass(id);
                true
            }
        }
        Subject::Row { program_id, caption, ord, name, .. } => {
            if passed {
                doc.finish_row(DoneRow { semester: s, program_id: program_id.clone(), ord: *ord, caption: caption.clone(), name: name.clone() })
            } else {
                doc.reopen_row(program_id, *ord);
                true
            }
        }
    }
}

/// Fills the semesters before the current one with the rows of their Fachsemester (the first
/// visit's „bisherige Semester nach Regelstudienplan füllen"): modules and placeholders as the
/// Stundenplan's import takes them, what is there already left as it is. How many modules and
/// placeholders it added.
pub fn fill_past(doc: &mut PlanDoc, input: &Input, at: u64) -> (usize, usize) {
    let Some(core) = input.core else { return (0, 0) };
    let now = input.now.max(input.start);
    let mut added = (0, 0);
    let mut s = input.start;
    while s < now {
        if let Some(fs) = fs_of(input.start, s, input.leave) {
            let import = studyplan::import_fs(doc, input.program_id, core, input.page, s, fs);
            let (modules, placeholders) = doc.apply(&import, at);
            added = (added.0 + modules, added.1 + placeholders);
        }
        let Some(next) = s.plus(1) else { break };
        s = next;
    }
    added
}

/// Where a module that another asks for („Voraussetzung", „empfohlen") stands, seen from the
/// semester of the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stand {
    /// Passed before, in that semester.
    Passed(SemesterKey),
    /// Planned before, in that semester.
    Before(SemesterKey),
    /// In the same semester.
    Same,
    /// Only later, from that semester on.
    After(SemesterKey),
    /// Planned only in a semester that is over, there not passed.
    Failed(SemesterKey),
    /// Not planned.
    Open,
}

/// A module another asks for, and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    pub id: String,
    /// The catalog's title, else the id.
    pub title: String,
    /// Asked for („zwingend"), else recommended.
    pub mandatory: bool,
    pub stand: Stand,
}

/// What module `id` asks for of other modules (`prerequisites`: the catalog's), each once, and
/// where each stands seen from semester `s` while `now` is the current one: what was planned in a
/// semester that is over and not passed there counts as not passed. Betula says it and blocks
/// nothing.
pub fn needs(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, s: SemesterKey, now: SemesterKey) -> Vec<Need> {
    let mut out: Vec<Need> = Vec::new();
    for p in prerequisites.iter().filter(|p| p.module_id == id && p.required_module_id != id) {
        let mandatory = p.kind.known() == Some(PrerequisiteKind::Mandatory);
        if let Some(known) = out.iter_mut().find(|need| need.id == p.required_module_id) {
            known.mandatory |= mandatory;
            continue;
        }
        let required = p.required_module_id.as_str();
        let planned = doc.planned_in(required);
        let ahead: Vec<SemesterKey> = planned.iter().copied().filter(|at| *at >= now).collect();
        let stand = match doc.passed_in(required) {
            Some(passed) if passed < s => Stand::Passed(passed),
            Some(passed) if passed == s => Stand::Same,
            Some(passed) => Stand::After(passed),
            None => match ahead.iter().copied().filter(|at| *at < s).max() {
                Some(before) => Stand::Before(before),
                None if ahead.contains(&s) => Stand::Same,
                None => match ahead.iter().copied().filter(|at| *at > s).min() {
                    Some(after) => Stand::After(after),
                    None => planned.iter().copied().max().map_or(Stand::Open, Stand::Failed),
                },
            },
        };
        let title = p.required_title.clone().filter(|title| !title.trim().is_empty()).unwrap_or_else(|| required.to_string());
        out.push(Need { id: required.to_string(), title, mandatory, stand });
    }
    out
}

/// A row of the plan neither planned nor passed, or what was not passed and is not planned again,
/// in the semester it fits next: what the Gesamtplan shows in grey, one click from being planned.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub suggestion: Suggestion,
    pub semester: SemesterKey,
    /// The Fachsemester the plan puts it into first.
    pub plan_fs: Option<u8>,
    /// A Wiederholer: the semester it was not passed in.
    pub failed_in: Option<SemesterKey>,
}

/// The open rows of the plan, each once at its first Fachsemester (or the current semester, where
/// that is over; then the next one that offers it), and the Wiederholer in the next semester that
/// offers them. A semester the study does not reach does not move a row.
pub fn open_lines(input: &Input, study: &Study) -> Vec<Line> {
    let now = study.now;
    let fit = |at: SemesterKey, offer: Offer| {
        let at = at.max(now);
        let next = offer.next(at);
        if study.semester(next).is_some() { next } else { at }
    };
    let last = plan_rows(input.core, input.page).iter().map(|row| row.last).max().unwrap_or(0);
    let mut seen: Vec<Pick> = Vec::new();
    let mut out = Vec::new();
    for fs in 1..=last {
        let Some(at) = semester_of_fs(input.start, fs, input.leave) else { continue };
        for suggestion in plan_semester(input, study, fs) {
            if seen.contains(&suggestion.pick) {
                continue;
            }
            seen.push(suggestion.pick.clone());
            if suggestion.standing == Standing::Open {
                out.push(Line { semester: fit(at, suggestion.offer), suggestion, plan_fs: Some(fs), failed_in: None });
            }
        }
    }
    for retake in &study.retakes {
        let item = &retake.item;
        let pick = match &item.subject {
            Subject::Module { id } => Pick::Module { id: id.clone(), from_plan: item.plan_fs.is_some() },
            Subject::Row { caption, ord, .. } => Pick::Row { caption: caption.clone(), ord: *ord },
        };
        let suggestion = Suggestion {
            pick,
            name: item.name.clone(),
            credits: item.credits,
            credits_text: match &item.subject {
                Subject::Row { credits, .. } => credits.clone(),
                Subject::Module { .. } => None,
            },
            area: item.area,
            kind: item.kind,
            offer: item.offer,
            standing: Standing::Open,
        };
        out.push(Line { semester: fit(now, item.offer), suggestion, plan_fs: item.plan_fs, failed_in: Some(retake.failed_in) });
    }
    out
}

/// What taking the Wiederholer (`Study::retakes`) into semester `s` adds: each module, and each
/// row as the placeholder it was. What the semester holds already is counted as skipped.
pub fn retake_import(study: &Study, doc: &PlanDoc, s: SemesterKey) -> Import {
    let mut import = Import::default();
    for retake in &study.retakes {
        match &retake.item.subject {
            Subject::Module { id } => {
                if doc.is_planned(s, id) {
                    import.skipped += 1;
                } else {
                    import.modules.push((s, id.clone()));
                }
            }
            Subject::Row { pid: Some(pid), program_id, ord, .. } => {
                if doc.placeholders.iter().any(|p| p.semester == s && p.program_id == *program_id && p.ord == *ord) {
                    import.skipped += 1;
                } else if let Some(p) = doc.placeholders.iter().find(|p| p.pid == *pid) {
                    import.placeholders.push(Placeholder { pid: 0, semester: s, ..p.clone() });
                }
            }
            Subject::Row { pid: None, .. } => {}
        }
    }
    import
}

#[cfg(test)]
mod tests {
    use folia_locale::Locale;
    use folia_model::labels::{Labelled, OfferStatus};
    use folia_model::rows_detail::PlanTotal;

    use super::*;
    use crate::area_fixtures::row;
    use crate::variants::plan_variants;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    const PROGRAM: &str = "079-82-2008";

    /// A module row of a plan in an area.
    fn module(ord: i64, id: &str, fs: i64, credits: f64, area: &str) -> PlanEntry {
        PlanEntry {
            ord,
            module_id: Some(id.to_string()),
            credits: Some(credits),
            subject_area: Some(area.to_string()),
            study_section: Some("Grundstudium".to_string()),
            ..row(&format!("Modul {id}"), fs, Some(ModuleKind::Compulsory), None)
        }
    }

    /// A row without a module over the Fachsemester `from`–`to`.
    #[allow(clippy::too_many_arguments)]
    fn choice(ord: i64, name: &str, from: i64, to: i64, credits: (f64, f64), area: &str, section: &str, kind: ModuleKind) -> PlanEntry {
        let (min, max) = credits;
        PlanEntry {
            ord,
            semester: None,
            start_semester: Some(from),
            end_semester: Some(to),
            credits: (min == max).then_some(min),
            min_credits: (min != max).then_some(min),
            max_credits: (min != max).then_some(max),
            subject_area: Some(area.to_string()),
            study_section: Some(section.to_string()),
            ..row(name, from, Some(kind), None)
        }
    }

    fn total(ord: i64, label: &str, credits: f64, entries: &[i64]) -> PlanTotal {
        PlanTotal {
            ord,
            label: label.to_string(),
            scope: Code::parse("section"),
            specialization: None,
            start_semester: 3,
            end_semester: 4,
            credits,
            credits_max: credits,
            min_credits: 20.0,
            max_credits: 48.0,
            is_choice: true,
            entry_count: entries.len() as i64,
            entries: entries.to_vec(),
        }
    }

    fn offered(id: &str, turnus: &str, credits: f64) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            title: format!("Titel {id}"),
            title_de: None,
            title_en: None,
            credits: Some(credits),
            turnus_season: Some(Code::parse(turnus)),
            turnus_parity: None,
            offer_status: Code::Known(OfferStatus::Active),
            teaches_german: None,
            teaches_english: None,
            is_fues: false,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            plan_semester: None,
            area: None,
        }
    }

    /// A plan of four Fachsemester for a winter start in the shape of Informatik B.Sc.: two
    /// modules of „Komplex Informatik" and one of „Komplex Mathematik" in the first, the FÜS, a
    /// summer module in the second, a choice between two complexes of the Fachstudium over the
    /// third and fourth (one sum of 20), a seminar of the Fachstudium, the thesis.
    fn plan() -> PlanVariant {
        let entries = vec![
            module(1, "11001", 1, 6.0, "Komplex Informatik"),
            module(2, "11002", 1, 6.0, "Komplex Informatik"),
            module(3, "11003", 2, 8.0, "Komplex Informatik"),
            module(4, "11004", 1, 8.0, "Komplex Mathematik"),
            choice(5, "Fachübergreifendes Studium", 2, 2, (6.0, 6.0), "Komplex Nebenfach", "Grundstudium", ModuleKind::Fues),
            choice(6, "Komplex Grundlagen der Informatik", 3, 4, (6.0, 14.0), "Komplex Grundlagen der Informatik", "Fachstudium", ModuleKind::Elective),
            choice(7, "Komplex Praktische Informatik", 3, 4, (6.0, 14.0), "Komplex Praktische Informatik", "Fachstudium", ModuleKind::Elective),
            choice(8, "Seminar oder Praktikum", 3, 3, (4.0, 4.0), "Fachstudium", "Fachstudium", ModuleKind::Elective),
            PlanEntry { module_id: Some("11009".to_string()), ..choice(9, "Bachelor-Arbeit", 4, 4, (12.0, 12.0), "Fachstudium", "Fachstudium", ModuleKind::Thesis) },
            PlanEntry { ord: 10, credits: None, kind: None, ..row("Summe", 4, None, None) },
        ];
        plan_variants(&entries, &[total(1, "Summe Komplexe des Fachstudiums", 20.0, &[6, 7])], Locale::De).remove(0)
    }

    fn catalog() -> Vec<CatalogRow> {
        let mut fues = offered("14011", "both", 6.0);
        fues.is_fues = true;
        vec![
            offered("11001", "winter", 6.0),
            offered("11002", "winter", 6.0),
            offered("11003", "summer", 8.0),
            offered("11004", "winter", 8.0),
            offered("11009", "both", 12.0),
            offered("12330", "both", 6.0),
            offered("12160", "winter", 6.0),
            fues,
        ]
    }

    /// The module tree: Grundstudium with its complexes, Fachstudium with two areas to choose.
    fn tree() -> Vec<CatalogArea> {
        vec![
            CatalogArea::new(1, "Grundstudium", &[], 0, false),
            CatalogArea::new(2, "Komplex Informatik", &["Grundstudium"], 3, false),
            CatalogArea::new(3, "Komplex Nebenfach", &["Grundstudium"], 0, false),
            CatalogArea::new(4, "Wirtschaftswissenschaften", &["Grundstudium", "Komplex Nebenfach"], 1, true),
            CatalogArea::new(5, "Fachstudium", &[], 0, false),
            CatalogArea::new(6, "Grundlagen der Informatik", &["Fachstudium"], 1, true),
            CatalogArea::new(7, "Praktische Informatik", &["Fachstudium"], 1, true),
        ]
    }

    fn placements() -> Vec<(String, i64)> {
        vec![("11001".into(), 2), ("11002".into(), 2), ("11003".into(), 2), ("12160".into(), 4), ("12330".into(), 7), ("11009".into(), 5)]
    }

    struct World {
        core: PlanVariant,
        rows: Vec<CatalogRow>,
        tree: Vec<CatalogArea>,
        placements: Vec<(String, i64)>,
        fues: Vec<String>,
        leave: BTreeSet<SemesterKey>,
    }

    fn world() -> World {
        World { core: plan(), rows: catalog(), tree: tree(), placements: placements(), fues: vec![], leave: BTreeSet::new() }
    }

    fn input<'a>(w: &'a World, doc: &'a PlanDoc, start: &str, now: &str) -> Input<'a> {
        Input {
            program_id: PROGRAM,
            core: Some(&w.core),
            page: None,
            start: key(start),
            now: key(now),
            leave: &w.leave,
            until: None,
            doc,
            rows: &w.rows,
            tree: &w.tree,
            placements: &w.placements,
            fues: &w.fues,
        }
    }

    fn names(study: &Study) -> Vec<(String, AreaKind, f64)> {
        study.progress.iter().map(|p| (p.area.name.clone(), p.area.kind, p.area.required)).collect()
    }

    fn keys(study: &Study, s: &str) -> Vec<String> {
        study.semester(key(s)).map(|semester| semester.items.iter().map(Item::key).collect()).unwrap_or_default()
    }

    #[test]
    fn leave_counts_as_no_fachsemester() {
        let leave: BTreeSet<SemesterKey> = [key("2026S")].into_iter().collect();
        let start = key("2025W");
        assert_eq!(fs_of(start, key("2025W"), &leave), Some(1));
        assert_eq!(fs_of(start, key("2026S"), &leave), None);
        assert_eq!(fs_of(start, key("2026W"), &leave), Some(2));
        assert_eq!(fs_of(start, key("2025S"), &leave), None);
        assert_eq!(semester_of_fs(start, 2, &leave), Some(key("2026W")));
        assert_eq!(semester_of_fs(start, 2, &BTreeSet::new()), Some(key("2026S")));
        assert_eq!(semester_of_fs(start, 0, &leave), None);
    }

    #[test]
    fn the_areas_are_the_plans_groups() {
        let (w, doc) = (world(), PlanDoc::default());
        let study = study(&input(&w, &doc, "2025W", "2025W"));
        // The choice between the complexes is one group with the seminar of its section; the FÜS
        // and the thesis stand apart.
        assert_eq!(
            names(&study),
            [
                ("Informatik".to_string(), AreaKind::Plan, 20.0),
                ("Mathematik".to_string(), AreaKind::Plan, 8.0),
                (String::new(), AreaKind::Fues, 6.0),
                ("Fachstudium".to_string(), AreaKind::Plan, 24.0),
                (String::new(), AreaKind::Thesis, 12.0),
            ]
        );
        assert_eq!(study.total, 70.0);
        // The Fachstudium holds the areas of the tree below it.
        let fach = &study.progress[3].area;
        assert!([5, 6, 7].iter().all(|id| fach.tree.contains(id)), "{:?}", fach.tree);
        assert_eq!(area_name("Komplex Informatik"), "Informatik");
        assert_eq!(area_name("Wahlpflichtmodule Praktische Mathematik"), "Praktische Mathematik");
    }

    #[test]
    fn semesters_begin_empty() {
        let (w, doc) = (world(), PlanDoc::default());
        let study = study(&input(&w, &doc, "2025W", "2026W"));
        assert_eq!(study.semesters.iter().map(|s| (s.key.key(), s.fs, s.when)).collect::<Vec<_>>(), [
            ("2025W".to_string(), Some(1), When::Past),
            ("2026S".to_string(), Some(2), When::Past),
            ("2026W".to_string(), Some(3), When::Now),
            ("2027S".to_string(), Some(4), When::Later),
        ]);
        assert!(study.semesters.iter().all(|s| s.items.is_empty()));
        assert_eq!((study.passed, study.planned, study.open, study.regular_end), (0.0, 0.0, 70.0, Some(key("2027S"))));
        // The choice spans the third and fourth: the third alone asks for the seminar.
        assert_eq!(study.semester(key("2026W")).unwrap().planned, Some(4.0));
    }

    #[test]
    fn what_was_not_passed_is_a_wiederholer_until_it_is_planned_again() {
        let w = world();
        let mut doc = PlanDoc::default();
        let at = |w: &World, doc: &PlanDoc| study(&input(w, doc, "2025W", "2026W"));
        // The first semester: two modules, one passed.
        let picks = [Pick::Module { id: "11001".into(), from_plan: true }, Pick::Module { id: "11002".into(), from_plan: true }];
        let before = doc.clone();
        assert_eq!(add(&mut doc, &input(&w, &before, "2025W", "2026W"), key("2025W"), &picks, 1), 2);
        assert!(doc.pass(key("2025W"), "11001"));
        let study = at(&w, &doc);
        let first = study.semester(key("2025W")).unwrap();
        assert_eq!(first.items.iter().map(|i| (i.passed, i.failed)).collect::<Vec<_>>(), [(true, false), (false, true)]);
        assert_eq!((first.credits, first.passed), (12.0, 6.0));
        assert_eq!(study.retakes.iter().map(|r| (r.item.key(), r.failed_in)).collect::<Vec<_>>(), [("m:11002".to_string(), key("2025W"))]);
        assert_eq!((study.passed, study.planned), (6.0, 0.0));
        // Planned again in this winter: no longer left over, and taken again there, first.
        let before = doc.clone();
        add(&mut doc, &input(&w, &before, "2025W", "2026W"), key("2026W"), &[Pick::Module { id: "11004".into(), from_plan: true }, Pick::Module { id: "11002".into(), from_plan: true }], 2);
        let study = at(&w, &doc);
        assert!(study.retakes.is_empty());
        assert_eq!(keys(&study, "2026W"), ["m:11002", "m:11004"]);
        assert_eq!(study.semester(key("2026W")).unwrap().items[0].retake, Some(key("2025W")));
        assert_eq!((study.passed, study.planned), (6.0, 14.0));
        // The past attempt stays there, not passed, and says where it is taken again.
        let failed = &study.semester(key("2025W")).unwrap().items[1];
        assert_eq!((failed.failed, failed.again), (true, Some(key("2026W"))));
    }

    #[test]
    fn what_a_module_asks_for_stands_where_it_is_planned() {
        let mut doc = PlanDoc::default();
        doc.plan(key("2025W"), "11001", 1, None);
        assert!(doc.pass(key("2025W"), "11001"));
        doc.plan(key("2026S"), "11003", 1, None);
        doc.plan(key("2027S"), "11004", 1, None);
        let need = |id: &str, kind: &str| Prerequisite {
            module_id: "12330".into(),
            required_module_id: id.into(),
            kind: Code::parse(kind),
            required_title: Some(format!("Titel {id}")),
            required_offer_status: None,
        };
        let prerequisites = [
            need("11001", "recommended"),
            need("11003", "mandatory"),
            need("11004", "recommended"),
            need("11002", "recommended"),
            need("11001", "mandatory"),
            Prerequisite { module_id: "other".into(), ..need("11009", "recommended") },
        ];
        let found = needs(&doc, &prerequisites, "12330", key("2026S"), key("2025W"));
        assert_eq!(
            found.iter().map(|n| (n.id.as_str(), n.mandatory, n.stand.clone())).collect::<Vec<_>>(),
            [
                ("11001", true, Stand::Passed(key("2025W"))),
                ("11003", true, Stand::Same),
                ("11004", false, Stand::After(key("2027S"))),
                ("11002", false, Stand::Open),
            ]
        );
        assert_eq!(found[0].title, "Titel 11001");
        // Seen from the summer after: what was planned in the winter before and not passed is not.
        doc.plan(key("2025W"), "11002", 1, None);
        let later = needs(&doc, &prerequisites, "12330", key("2027S"), key("2027S"));
        assert_eq!(later.iter().find(|n| n.id == "11002").map(|n| n.stand.clone()), Some(Stand::Failed(key("2025W"))));
        assert_eq!(later.iter().find(|n| n.id == "11004").map(|n| n.stand.clone()), Some(Stand::Same));
    }

    #[test]
    fn the_open_rows_and_the_wiederholer_stand_where_they_fit_next() {
        let w = world();
        let mut doc = PlanDoc::default();
        // In the third semester: of the first, 11001 passed and 11002 not; 11003 planned.
        doc.plan(key("2025W"), "11001", 1, None);
        doc.plan(key("2025W"), "11002", 1, None);
        assert!(doc.pass(key("2025W"), "11001"));
        doc.plan(key("2027S"), "11003", 1, None);
        let input = input(&w, &doc, "2025W", "2026W");
        let study = study(&input);
        let lines = open_lines(&input, &study);
        let at = |pick: Pick| lines.iter().find(|line| line.suggestion.pick == pick).map(|line| (line.semester, line.failed_in));
        let module = |id: &str| Pick::Module { id: id.into(), from_plan: true };
        let row = |ord: i64| Pick::Row { caption: String::new(), ord };
        // A winter module of the first: the current winter. The FÜS of the second: now.
        assert_eq!(at(module("11004")), Some((key("2026W"), None)));
        assert_eq!(at(row(5)), Some((key("2026W"), None)));
        // The choice over the third and fourth: once, in the third. The thesis in the fourth.
        assert_eq!(lines.iter().filter(|line| line.suggestion.pick == row(6)).count(), 1);
        assert_eq!(at(row(6)), Some((key("2026W"), None)));
        assert_eq!(at(module("11009")), Some((key("2027S"), None)));
        // What was not passed: a Wiederholer, in the next winter.
        assert_eq!(at(module("11002")), Some((key("2026W"), Some(key("2025W")))));
        // What is planned or passed is no line.
        assert_eq!((at(module("11001")), at(module("11003"))), (None, None));
        // The Stundenplan takes the Wiederholer.
        let import = retake_import(&study, &doc, key("2026W"));
        assert_eq!((import.modules, import.skipped), (vec![(key("2026W"), "11002".to_string())], 0));
    }

    #[test]
    fn betula_blocks_nothing_and_says_what_it_knows() {
        let w = world();
        let mut doc = PlanDoc::default();
        let before = doc.clone();
        // A winter module into a summer, the FÜS, a module the catalog does not know.
        let picks = [
            Pick::Module { id: "11004".into(), from_plan: true },
            Pick::Module { id: "14011".into(), from_plan: false },
            Pick::Module { id: "99999".into(), from_plan: false },
        ];
        assert_eq!(add(&mut doc, &input(&w, &before, "2025W", "2025W"), key("2026S"), &picks, 1), 3);
        // The same module twice into one semester: once. The FÜS into the winter: a module planned
        // ahead moves.
        let before = doc.clone();
        let again = [Pick::Module { id: "11004".into(), from_plan: true }];
        assert_eq!(add(&mut doc, &input(&w, &before, "2025W", "2025W"), key("2026S"), &again, 2), 0);
        let before = doc.clone();
        let fues = [Pick::Module { id: "14011".into(), from_plan: false }];
        assert_eq!(add(&mut doc, &input(&w, &before, "2025W", "2025W"), key("2026W"), &fues, 2), 1);
        let planned = study(&input(&w, &doc, "2025W", "2025W"));
        assert_eq!(keys(&planned, "2026S"), ["m:11004", "m:99999"]);
        assert_eq!(keys(&planned, "2026W"), ["m:14011"]);
        let summer = &planned.semester(key("2026S")).unwrap().items;
        assert!(summer[0].unoffered, "a winter module in the summer: allowed, and said");
        assert_eq!((summer[0].area, summer[1].area), (Some(1), None));
        assert_eq!(planned.semester(key("2026W")).unwrap().items[0].area, Some(2), "the catalog's FÜS counts for the FÜS");
        assert_eq!(planned.outside, (0.0, 0.0), "a module the catalog does not know has no credits");
        // Two FÜS modules: the second is beyond what the area asks.
        let mut doc = PlanDoc::default();
        doc.plan(key("2026S"), "14011", 1, None);
        let mut twice = w.rows.clone();
        let mut other = offered("14012", "both", 6.0);
        other.is_fues = true;
        twice.push(other);
        let w2 = World { rows: twice, ..world() };
        doc.plan(key("2026W"), "14012", 2, None);
        let over = study(&input(&w2, &doc, "2025W", "2025W"));
        let fues = &over.progress[2];
        assert_eq!((fues.planned, fues.over), (6.0, 6.0));
        assert_eq!(over.semester(key("2026W")).unwrap().items[0].over, 6.0);
        assert_eq!(over.over, 6.0);
    }

    #[test]
    fn modules_count_where_the_tree_places_them() {
        let w = world();
        let mut doc = PlanDoc::default();
        // Datenbanken lies in an area of the Fachstudium, BWL in one of the Nebenfach, which the
        // plan names as no area of its own: outside.
        doc.plan(key("2027S"), "12330", 1, None);
        doc.plan(key("2027S"), "12160", 1, None);
        let study = study(&input(&w, &doc, "2025W", "2025W"));
        let items = &study.semester(key("2027S")).unwrap().items;
        assert_eq!(items[0].area, Some(3));
        assert_eq!(items[1].area, None);
        assert_eq!(study.outside, (0.0, 6.0));
        assert_eq!(study.progress[3].planned, 6.0);
    }

    #[test]
    fn rows_are_placeholders_and_count_until_modules_fill_them() {
        let w = world();
        let mut doc = PlanDoc::default();
        let before = doc.clone();
        let picks = [Pick::Row { caption: String::new(), ord: 6 }, Pick::Row { caption: String::new(), ord: 8 }];
        assert_eq!(add(&mut doc, &input(&w, &before, "2025W", "2025W"), key("2026W"), &picks, 1), 2);
        let study_now = study(&input(&w, &doc, "2025W", "2025W"));
        let items = &study_now.semester(key("2026W")).unwrap().items;
        assert_eq!(items.iter().map(Item::key).collect::<Vec<_>>(), [":6", ":8"].map(|k| format!("r{k}").replacen("r:", "r::", 1)));
        assert_eq!((items[0].credits, items[0].range), (Some(6.0), true));
        assert_eq!(study_now.progress[3].planned, 10.0);
        // A module filling the choice counts instead of it.
        let pid = doc.placeholders[0].pid;
        doc.plan(key("2026W"), "12330", 2, Some(pid));
        let filled = study(&input(&w, &doc, "2025W", "2025W"));
        assert_eq!(filled.semester(key("2026W")).unwrap().items[1].fillers, ["12330"]);
        assert_eq!(filled.progress[3].planned, 4.0 + 6.0);
        // Done, the row is passed where it was done.
        let item = filled.semester(key("2026W")).unwrap().items[2].clone();
        assert!(set_passed(&mut doc, key("2026W"), &item, true));
        let done = study(&input(&w, &doc, "2025W", "2025W"));
        assert!(done.semester(key("2026W")).unwrap().items[2].passed);
        assert_eq!(done.progress[3].passed, 4.0);
    }

    #[test]
    fn the_picker_offers_a_fachsemester_with_where_each_row_stands() {
        let w = world();
        let mut doc = PlanDoc::default();
        doc.plan(key("2026W"), "11002", 1, None);
        assert!(doc.pass(key("2025W"), "11001"));
        let input = input(&w, &doc, "2025W", "2026W");
        let study = study(&input);
        let first = plan_semester(&input, &study, 1);
        assert_eq!(first.iter().map(|s| (s.name.clone(), s.standing.clone())).collect::<Vec<_>>(), [
            ("Titel 11001".to_string(), Standing::Passed(key("2025W"))),
            ("Titel 11002".to_string(), Standing::Planned(vec![key("2026W")])),
            ("Titel 11004".to_string(), Standing::Open),
        ]);
        assert_eq!(first[0].area, Some(0));
        let third = plan_semester(&input, &study, 3);
        assert_eq!(third.iter().map(|s| (s.pick.clone(), s.credits_text.clone())).collect::<Vec<_>>(), [
            (Pick::Row { caption: String::new(), ord: 6 }, Some("6–14".to_string())),
            (Pick::Row { caption: String::new(), ord: 7 }, Some("6–14".to_string())),
            (Pick::Row { caption: String::new(), ord: 8 }, Some("4".to_string())),
        ]);
    }

    #[test]
    fn moved_removed_and_filled_from_the_plan() {
        let w = world();
        let mut doc = PlanDoc::default();
        doc.plan(key("2026W"), "11004", 1, None);
        assert!(doc.pass(key("2026W"), "11004"));
        let study_now = study(&input(&w, &doc, "2025W", "2026W"));
        let item = study_now.semester(key("2026W")).unwrap().items[0].clone();
        // Moved, what was passed goes with it.
        assert!(move_item(&mut doc, key("2026W"), key("2027S"), &item));
        assert_eq!((doc.is_planned(key("2027S"), "11004"), doc.passed_in("11004")), (true, Some(key("2027S"))));
        let moved = study(&input(&w, &doc, "2025W", "2026W"));
        let item = moved.semester(key("2027S")).unwrap().items[0].clone();
        remove(&mut doc, key("2027S"), &item);
        assert!(doc.modules.is_empty() && doc.passed.is_empty());
        // The first visit in the third semester: the first two filled from the plan.
        let before = doc.clone();
        let (modules, placeholders) = fill_past(&mut doc, &input(&w, &before, "2025W", "2026W"), 1);
        assert_eq!((modules, placeholders), (4, 1));
        let filled = study(&input(&w, &doc, "2025W", "2026W"));
        assert_eq!(keys(&filled, "2025W"), ["m:11001", "m:11002", "m:11004"]);
        assert_eq!(keys(&filled, "2026S"), ["m:11003", "r::5"]);
        assert_eq!(filled.retakes.len(), 5, "nothing ticked off: all of it is left over");
        // Twice adds nothing.
        let before = doc.clone();
        assert_eq!(fill_past(&mut doc, &input(&w, &before, "2025W", "2026W"), 2), (0, 0));
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);
    }

    #[test]
    fn a_semester_of_leave_and_one_added_beyond_the_plan() {
        let mut w = world();
        w.leave = [key("2026S")].into_iter().collect();
        let doc = PlanDoc::default();
        let mut input = input(&w, &doc, "2025W", "2026W");
        input.until = Some(key("2028W"));
        let study = study(&input);
        assert_eq!(study.semesters.iter().map(|s| (s.key.key(), s.fs, s.leave)).collect::<Vec<_>>(), [
            ("2025W".to_string(), Some(1), false),
            ("2026S".to_string(), None, true),
            ("2026W".to_string(), Some(2), false),
            ("2027S".to_string(), Some(3), false),
            ("2027W".to_string(), Some(4), false),
            ("2028S".to_string(), Some(5), false),
            ("2028W".to_string(), Some(6), false),
        ]);
        assert_eq!(study.regular_end, Some(key("2027W")));
        assert!(study.semester(key("2028S")).unwrap().beyond);
    }

    /// Informatik B.Sc. as the snapshot has it: the areas of the mockup (owner, 2026-10-04).
    #[test]
    fn the_areas_of_informatik() {
        let db = folia_test_support::open();
        let entries = folia_query::program_plan_entries(&db, PROGRAM).unwrap();
        let variants = plan_variants(&entries, &folia_query::program_plan_totals(&db, PROGRAM).unwrap(), Locale::De);
        let placed = folia_query::program_areas(&db, PROGRAM).unwrap();
        let tree = crate::areas::catalog_areas(&placed, &folia_query::program_area_tree(&db, PROGRAM).unwrap());
        let placements: Vec<(String, i64)> = placed.iter().map(|p| (p.module_id.clone(), p.area_id)).collect();
        let rows = plan_rows(variants.first(), None);
        let found = areas(&rows, &tree, &placements);
        assert_eq!(
            found.iter().map(|a| (a.name.as_str(), a.kind, a.required)).collect::<Vec<_>>(),
            [
                ("Informatik", AreaKind::Plan, 66.0),
                ("Mathematik", AreaKind::Plan, 24.0),
                ("Nebenfach", AreaKind::Plan, 18.0),
                ("", AreaKind::Fues, 6.0),
                ("Fachstudium", AreaKind::Plan, 54.0),
                ("", AreaKind::Thesis, 12.0),
            ]
        );
        // BWL counts for the Nebenfach, Datenbanken for the Fachstudium.
        let of = |id: &str| placements.iter().filter(|(m, _)| m == id).map(|(_, a)| *a).collect::<Vec<_>>();
        assert!(of("12160").iter().any(|a| found[2].tree.contains(a)));
        assert!(of("12330").iter().any(|a| found[4].tree.contains(a)));
    }

    /// Every plan of the snapshot: its areas take every row once, ask for what the plan comes to,
    /// and a study filled from the plan to the current semester counts all of it.
    #[test]
    fn every_plan_of_the_snapshot_has_its_areas() {
        let db = folia_test_support::open();
        let mut studied = 0;
        for program in folia_query::programs(&db).unwrap().into_iter().filter(|program| program.has_plan) {
            let entries = folia_query::program_plan_entries(&db, &program.id).unwrap();
            let variants = plan_variants(&entries, &folia_query::program_plan_totals(&db, &program.id).unwrap(), Locale::De);
            let pages = crate::variants::supplements(&variants);
            let placed = folia_query::program_areas(&db, &program.id).unwrap();
            let tree = crate::areas::catalog_areas(&placed, &folia_query::program_area_tree(&db, &program.id).unwrap());
            let placements: Vec<(String, i64)> = placed.iter().map(|p| (p.module_id.clone(), p.area_id)).collect();
            let fues: Vec<String> = folia_query::program_modules(&db, &program.id, folia_routes::filter::ProgramRelation::Fues).unwrap().into_iter().map(|m| m.module_id).collect();
            let ids: Vec<String> = entries.iter().filter_map(|entry| entry.module_id.clone()).filter(|id| url::is_module_id(id)).collect();
            let query = folia_routes::filter::CatalogQuery { only_ids: Some(ids.clone()), offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() };
            let linked = folia_query::catalog_page(&db, &query, 0, ids.len() as u64 + 1).unwrap().rows;
            let leave = BTreeSet::new();
            for (i, core) in variants.iter().enumerate().filter(|(i, _)| !pages.iter().any(|page| page.page == *i)) {
                let page = pages.iter().find(|page| page.core == i).map(|page| (&variants[page.page], page.ord));
                let context = format!("{} {}", program.id, core.full);
                let rows = plan_rows(Some(core), page);
                let found = areas(&rows, &tree, &placements);
                assert!(!rows.is_empty() || found.is_empty(), "{context}");
                assert!(found.len() <= MAX_AREAS, "{context}: {} areas", found.len());
                let taken: usize = found.iter().map(|area| area.rows.len()).sum();
                assert_eq!(taken, rows.len(), "{context}: every row in one area");
                assert!(found.iter().all(|area| area.required >= 0.0), "{context}");

                let mut doc = PlanDoc::default();
                let make = |doc: &PlanDoc| -> Study {
                    let input = Input { program_id: &program.id, core: Some(core), page, start: key("2025W"), now: key("2027W"), leave: &leave, until: None, doc, rows: &linked, tree: &tree, placements: &placements, fues: &fues };
                    study(&input)
                };
                let before = doc.clone();
                let input = Input { program_id: &program.id, core: Some(core), page, start: key("2025W"), now: key("2027W"), leave: &leave, until: None, doc: &before, rows: &linked, tree: &tree, placements: &placements, fues: &fues };
                fill_past(&mut doc, &input, 1);
                let filled = make(&doc);
                assert_eq!(filled.passed, 0.0, "{context}");
                assert!(filled.semesters.iter().filter(|s| s.when == When::Past).all(|s| s.items.iter().all(|item| item.failed)), "{context}");
                // All of the past ticked off.
                for semester in filled.semesters.iter().filter(|s| s.when == When::Past) {
                    for item in &semester.items {
                        set_passed(&mut doc, semester.key, item, true);
                    }
                }
                let ticked = make(&doc);
                assert!(ticked.retakes.is_empty(), "{context}");
                assert!(ticked.passed + ticked.over + ticked.outside.0 > 0.0 || filled.semesters.iter().all(|s| s.items.is_empty()), "{context}");
                assert_eq!(PlanDoc::restored(&doc.stored()), doc, "{context}");
                studied += 1;
            }
        }
        assert!(studied > 100, "{studied} plans");
    }
}
