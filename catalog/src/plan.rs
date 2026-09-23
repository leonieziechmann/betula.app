//! What a study plan asks for beyond the modules it names: its requirement rows
//! („Wahlpflichtmodule der Studienrichtung", „Wahlpflichtmodul aus der Informatik") and the areas
//! of the program's module tree those rows point at. No source links a row to an area
//! (`area_rules` is prose about credits), so the name does the work, and everything here is
//! derived, never stated: the pages that use it say so (R12). The program page shows it beside a
//! row of the plan; the catalog uses it to list, with the modules the plan places in a semester,
//! the modules that can be chosen for that semester's requirements.

use serde::{Deserialize, Serialize};

use crate::labels::{Code, ModuleKind};
use crate::pages::CatalogArea;
use crate::rows_detail::{PlanEntry, PlanTotal};

/// Which semesters a row of the plan belongs to: one, a span, or none at all.
pub fn semester_span(entry: &PlanEntry) -> Option<(i64, i64)> {
    match (entry.semester, entry.start_semester, entry.end_semester) {
        (Some(n), _, _) => Some((n, n)),
        (None, Some(from), Some(to)) => Some((from.min(to), from.max(to))),
        (None, Some(n), None) | (None, None, Some(n)) => Some((n, n)),
        (None, None, None) => None,
    }
}

/// What a row of the plan says about its credits: a number, a range, or nothing.
pub fn credits_of(entry: &PlanEntry) -> Option<String> {
    let number = |n: f64| if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{n}").replace('.', ",") };
    match (entry.credits, entry.min_credits, entry.max_credits) {
        (Some(credits), _, _) => Some(number(credits)),
        (None, Some(min), Some(max)) if min != max => Some(format!("{}–{}", number(min), number(max))),
        (None, Some(value), _) | (None, None, Some(value)) => Some(number(value)),
        _ => None,
    }
}

/// A row stated as Pflicht, Abschlussarbeit or Praktikum means one module, not a choice: without
/// a link to the catalog, the catalog simply does not know it under this name.
pub fn is_single_module(entry: &PlanEntry) -> bool {
    entry
        .kind
        .as_ref()
        .is_some_and(|kind| kind.is(ModuleKind::Compulsory) || kind.is(ModuleKind::Thesis) || kind.is(ModuleKind::Internship))
}

/// A row that asks for a module of the Fachübergreifendes Studium: its modules are the
/// program's FÜS list, not its tree. The plan says so by the kind, or only by the name („FÜS",
/// „Fachübergreifendes Studium", „Modul aus dem FÜS-Katalog der BTU", „WPF FÜS"): in the plans of
/// 2026-09-21, 112 rows say so only by their name.
pub fn is_fues(entry: &PlanEntry) -> bool {
    let named = crate::search::fold(&entry.module_name)
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == "fus" || word.starts_with("fusmodul") || word.starts_with("fachubergreifend") || word.starts_with("facherubergreifend"));
    named || entry.kind.as_ref().is_some_and(|kind| kind.is(ModuleKind::Fues))
}

/// The study direction an area's label ends in („Informatik (MIT)" → „MIT").
fn direction_of(label: &str) -> Option<&str> {
    let (_, rest) = label.rsplit_once('(')?;
    let (inside, _) = rest.split_once(')')?;
    let inside = inside.trim();
    (!inside.is_empty() && inside.len() <= 12 && !inside.contains(char::is_whitespace)).then_some(inside)
}

/// Whether a plan printed for `direction` („PA", as a caption spells it) is about the areas of
/// `area_direction` („PAu", as the tree spells it).
fn same_direction(direction: &str, area_direction: &str) -> bool {
    direction.len() >= 2 && (area_direction.starts_with(direction) || direction.starts_with(area_direction))
}

/// An area's label without the study direction it ends in („Informatik (MIT)" → „Informatik").
pub(crate) fn without_direction(label: &str) -> &str {
    match direction_of(label) {
        Some(_) => label.rsplit_once('(').map(|(name, _)| name.trim_end()).unwrap_or(label),
        None => label,
    }
}

/// The words of a name that say which area it is: folded, without the words that say what kind
/// of thing it is („Komplex", „Wahlpflichtmodule", „WPF", „Compulsory Elective Modules", „Modul
/// aus dem"), without articles and the plan's references („gem. Anlage a5", „Prü/SL"), so that
/// „Komplex Praktische Informatik" and „Praktische Informatik" read the same. What the plans
/// print around a name falls away as well: a footnote number in front („5 Berufspraktische
/// Vorbereitung") or glued to the end („Wirtschaftswissenschaften2"). Roman numbers read as
/// digits („Schwerpunktmodul II" — „Schwerpunktmodul 2"), a plural as its singular
/// („Grundlagen der Statistik" — „Grundlage der Statistik"), and „Anwendungsfach",
/// „Anwendungen", „Anwendungsbereiche" as „Nebenfach", the other word the BTU's studies use
/// for the subject a student adds to their own. A name of nothing but numbers says nothing.
pub(crate) fn distinctive(text: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "komplex", "wahlpflicht", "wahlpflichtmodul", "wahlpflichtmodule", "wahlpflichtbereich", "wahlpflichtfach", "wahlpflichtkatalog", "wahlpflichtangebot",
        "wahl", "wahlbereich", "modul", "module", "moduls", "modulen", "modules", "pflicht", "pflichtmodul", "pflichtmodule", "wp", "wpf", "pf",
        "bereich", "bereiche", "bereichs", "studium", "katalog", "lp", "kp", "ects", "aus", "dem", "der", "des", "die", "das", "den", "im", "in", "und", "oder",
        "von", "vom", "zum", "zur", "mit", "ein", "eine", "einer", "eines", "einem", "einen", "fur", "gewahlten", "gewahlte", "wahlbar", "wahlen", "gemass", "gem", "anlage", "anlagen",
        "siehe", "pru", "sl", "bis", "je", "nach", "sowie", "zu", "compulsory", "elective", "optional", "mandatory", "of", "the", "and", "or", "from",
    ];
    const ROMAN: &[(&str, &str)] = &[("i", "1"), ("ii", "2"), ("iii", "3"), ("iv", "4"), ("vi", "6")];
    const NEBENFACH: &[&str] = &["nebenfach", "nebenfacher", "anwendungsfach", "anwendungsfacher", "anwendungen", "anwendungsbereich", "anwendungsbereiche"];
    const REFERENCES: &[&str] = &["anlage", "anlagen", "tab", "tabelle", "seite"];
    let mut words: Vec<String> = Vec::new();
    let mut after_reference = false;
    for word in crate::search::fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()) {
        let word = match ROMAN.iter().find(|(roman, _)| *roman == word) {
            Some((_, digit)) => digit.to_string(),
            // „Wirtschaftswissenschaften2": a footnote glued to the word.
            None if word.chars().filter(|c| c.is_alphabetic()).count() >= 4 => word.trim_end_matches(|c: char| c.is_ascii_digit()).to_string(),
            None => word.to_string(),
        };
        let number = word.chars().all(|c| c.is_ascii_digit());
        // „a5", „2d": a reference into the plan's appendices, not a name.
        let reference = !number && word.chars().any(|c| c.is_ascii_digit()) && word.chars().count() <= 3;
        if (word.len() < 2 && !number) || reference || NOISE.contains(&word.as_str()) {
            after_reference = REFERENCES.contains(&word.as_str());
            continue;
        }
        // A number in front of the name is the plan's numbering, one after „Anlage" or „Tab."
        // points into the appendices: neither is part of the name.
        if number && (words.is_empty() || after_reference) {
            after_reference = false;
            continue;
        }
        after_reference = REFERENCES.contains(&word.as_str());
        let word = if NEBENFACH.contains(&word.as_str()) { "nebenfach".to_string() } else { singular(&word) };
        words.push(word);
    }
    if words.iter().all(|word| word.chars().all(|c| c.is_ascii_digit())) {
        return Vec::new();
    }
    words
}

/// „grundlagen" → „grundlag", „grundlage" → „grundlag": one form for singular and plural.
fn singular(word: &str) -> String {
    if word.chars().count() > 6 {
        for ending in ["en", "e", "n", "s"] {
            if let Some(stem) = word.strip_suffix(ending) {
                return stem.to_string();
            }
        }
    }
    word.to_string()
}

/// How well the words of a row fit the words of a name: the same words (100), the one within
/// the other (60, a little more the more they share), some words in common (10 each; a word may
/// be the start of the other, „Studienrichtung" — „Studienrichtungsspezifische", or share a long
/// beginning, „Rechtswissenschaften" — „Rechtswissenschaftlicher"), or nothing (0). Words in
/// common count only where they are at least half of the row's words, and numbers do not count
/// there: „Physics of Modern Devices" is a module, not the area „Technology and Devices", and
/// „Schwerpunkt 1" is not „Konstruktiver Ingenieurbau - 1".
fn fit(row: &[String], name: &[String]) -> i32 {
    if row.is_empty() || name.is_empty() {
        return 0;
    }
    let within = |inner: &[String], outer: &[String]| inner.iter().all(|word| outer.contains(word));
    if row.len() == name.len() && within(row, name) && within(name, row) {
        return 100;
    }
    let shared = row.iter().filter(|word| name.contains(word)).count() as i32;
    if within(row, name) || within(name, row) {
        return 60 + 5 * shared;
    }
    let is_word = |word: &&String| !word.chars().all(|c| c.is_ascii_digit());
    let words: Vec<&String> = row.iter().filter(is_word).collect();
    let matched = words.iter().filter(|word| name.iter().filter(is_word).any(|other| akin(word, other))).count();
    if matched * 2 < words.len() {
        return 0;
    }
    10 * matched as i32
}

/// Two words that read as one: one the start of the other — an inflection („Kunst" — „Künste")
/// or a long word („Studienrichtung" — „Studienrichtungsspezifische"), never a short word at the
/// start of a compound („Stadt" — „Stadtbaugeschichte") — or with a long beginning in common
/// („Rechtswissenschaften" — „Rechtswissenschaftlicher").
fn akin(a: &str, b: &str) -> bool {
    let common = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let shorter = a.chars().count().min(b.chars().count());
    let longer = a.chars().count().max(b.chars().count());
    let prefix = common == shorter && (longer - shorter <= 2 || shorter >= 8);
    shorter >= 5 && (prefix || (common >= 8 && common * 4 >= shorter * 3))
}

/// The areas a row of the plan points at: those that fit best, equally well — one, or several,
/// and then none of them is the one (a page names them instead of picking one) — and those that
/// fit less well but not by far, named as also possible.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowAreas {
    pub areas: Vec<CatalogArea>,
    pub others: Vec<CatalogArea>,
}

impl RowAreas {
    pub fn ambiguous(&self) -> bool {
        self.areas.len() > 1
    }
}

/// The words an area is known by: its label's; for a label that says nothing but a kind
/// („Wahlpflichtmodule" below „Entwerfen"), the words of the nearest node above that names a
/// field — and then that node is the area's name, not a node above it.
fn area_words(area: &CatalogArea) -> (Vec<String>, Option<usize>) {
    let own = distinctive(without_direction(&area.label));
    if !own.is_empty() {
        return (own, None);
    }
    match area.ancestors.iter().enumerate().rev().find(|(_, ancestor)| !crate::pages::is_structural(ancestor)) {
        Some((index, ancestor)) => (distinctive(without_direction(ancestor)), Some(index)),
        None => (Vec::new(), None),
    }
}

/// The parts of a row that names several areas at once: „Wahlpflicht: Komplex Grundlagen der
/// Informatik / Komplex Praktische Informatik / …", „Schwerpunkt A oder Schwerpunkt B", „aus
/// dem gewählten Schwerpunkt „A“, „B“ oder „C“".
fn alternatives(name: &str) -> Vec<String> {
    let quoted: Vec<String> = name.split(['„', '“', '"']).skip(1).step_by(2).map(str::trim).filter(|part| part.chars().count() >= 3).map(str::to_string).collect();
    if quoted.len() >= 2 {
        return quoted;
    }
    let name = name.split_once(':').map(|(_, rest)| rest).unwrap_or(name);
    name.split(" / ").flat_map(|part| part.split(" oder ")).map(str::trim).filter(|part| !part.is_empty()).map(str::to_string).collect()
}

/// Which areas of the program a row of the plan is about. Only areas a student chooses from
/// come into question (`CatalogArea::choice`): a requirement row never means the Pflichtmodule.
/// The name decides: the same name as an area („Komplex Praktische Informatik" — „Praktische
/// Informatik") is that area and no other; a row that lists areas („Komplex A / Komplex B",
/// „Schwerpunkt A oder Schwerpunkt B") means each of them; a name within an area's name or the
/// other way round („Wahlpflichtmodul aus der Informatik" — every „… Informatik" area) fits next
/// best, and all that fit equally well are meant; some words in common count least. A name that
/// is an area above the leaves („Anwendungsfach" — „Komplex Nebenfach", with Mathematik,
/// Physik … below it) means the areas below it, except those another row of the same plan
/// (`rows`) names on their own („Modul aus dem Bereich Praktische Mathematik"). The study
/// direction the plan is printed for keeps the other directions out („Wahlpflichtmodul aus der
/// **Informatik**" in the plan of MIT and EET → „Informatik (MIT)", „Informatik (EET)", never
/// the areas of PAu or IoT). `plan` is the caption of the plan („Regelstudienplan der
/// Studienrichtungen MIT und EET …"), empty where a program has one plan.
pub fn areas_for_row(entry: &PlanEntry, plan: &str, areas: &[CatalogArea], rows: &[PlanEntry]) -> RowAreas {
    let row_words = distinctive(&entry.module_name);
    if row_words.is_empty() {
        return RowAreas::default();
    }
    // The directions this plan is printed for, as its caption spells them: by their short names
    // („MIT und EET", „PA und IoT" for PAu) or by the name an area of the tree gives in front of
    // one („Studienrichtung „Umwelttechnik“" — „Umwelttechnik (UMT)").
    let plan_parts: Vec<&str> = plan.split(|c: char| !c.is_alphanumeric()).filter(|part| part.len() >= 2).collect();
    let caption = crate::search::fold(plan);
    let directions: Vec<&str> = areas
        .iter()
        .filter_map(|area| direction_of(&area.label).map(|direction| (direction, crate::search::fold(without_direction(&area.label)))))
        .filter(|(direction, name)| plan_parts.iter().any(|part| same_direction(part, direction)) || (name.chars().count() >= 8 && caption.contains(name.as_str())))
        .map(|(direction, _)| direction)
        .collect();
    let candidates: Vec<(&CatalogArea, Vec<String>, Option<usize>)> = areas
        .iter()
        .filter(|area| area.choice)
        // Where the plans are printed per study direction, only this plan's areas can be meant.
        .filter(|area| directions.is_empty() || direction_of(&area.label).is_none_or(|direction| directions.contains(&direction)))
        .map(|area| {
            let (words, named_by) = area_words(area);
            (area, words, named_by)
        })
        .collect();

    // A row that lists areas by their names: each of them (unless the whole row is one name,
    // „Maschinenbau / Elektrotechnik").
    let parts = alternatives(&entry.module_name);
    if parts.len() >= 2 && !candidates.iter().any(|(_, name, _)| fit(&row_words, name) == 100) {
        let named: Vec<Vec<&CatalogArea>> = parts
            .iter()
            .map(|part| distinctive(part))
            .map(|words| candidates.iter().filter(|(_, name, _)| fit(&words, name) == 100).map(|(area, ..)| *area).collect())
            .filter(|found: &Vec<&CatalogArea>| !found.is_empty())
            .collect();
        if named.len() >= 2 {
            let mut listed: Vec<CatalogArea> = Vec::new();
            for area in named.into_iter().flatten() {
                if !listed.iter().any(|known| known.id == area.id) {
                    listed.push(area.clone());
                }
            }
            return RowAreas { areas: listed, others: Vec::new() };
        }
    }

    // What the other rows of the plan name on their own is not what this row means by a name
    // above it.
    let claimed = |words: &[String]| {
        rows.iter()
            .filter(|other| other.module_id.is_none() && other.module_name != entry.module_name)
            .any(|other| fit(&distinctive(&other.module_name), words) == 100)
    };
    let mut scored: Vec<(i32, &CatalogArea)> = candidates
        .iter()
        .filter_map(|(area, words, named_by)| {
            // An area named after the field above it is that field's list: a row that fits the
            // field fits it as it fits the areas below that field, unless it names it exactly.
            let by_name = match (fit(&row_words, words), named_by) {
                (100, _) | (_, None) => fit(&row_words, words),
                (score, Some(_)) => score / 2,
            };
            // A row that names an area above this one: it means what lies below („Nebenfach").
            let by_ancestor = area
                .ancestors
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != *named_by)
                .map(|(_, ancestor)| fit(&row_words, &distinctive(without_direction(ancestor))))
                .filter(|score| *score >= 60)
                .max()
                .unwrap_or(0);
            if by_name == 0 && by_ancestor > 0 && claimed(words) {
                return None;
            }
            let mut score = by_name + by_ancestor / 2;
            if score > 0 && direction_of(&area.label).is_some_and(|direction| directions.contains(&direction)) {
                score += 5;
            }
            (score > 0).then_some((score, *area))
        })
        .collect();
    scored.sort_by(|(a, left), (b, right)| b.cmp(a).then(right.modules.cmp(&left.modules)));
    let best = scored.first().map(|(score, _)| *score).unwrap_or(0);
    let tied = scored.iter().filter(|(score, _)| *score == best).count();
    RowAreas {
        areas: scored.iter().take(tied.min(8)).map(|(_, area)| (*area).clone()).collect(),
        others: scored.iter().skip(tied).filter(|(score, _)| *score * 2 >= best).take(3).map(|(_, area)| (*area).clone()).collect(),
    }
}

/// A row of the plan in a semester that names no module of the catalog, and what can be chosen
/// for it. Rows of several study plans (one per study direction) that read the same are one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemesterRequirement {
    pub name: String,
    /// „12", „10–24"; `None` where the plan states none.
    pub credits: Option<String>,
    pub kind: Option<Code<ModuleKind>>,
    /// One module the catalog does not know under this name, not a choice.
    pub single: bool,
    /// A module of the Fachübergreifendes Studium: the program's FÜS list, not its tree.
    pub fues: bool,
    /// The areas whose modules are listed for the row: the one the name fits best, or all of
    /// those that fit equally well (none of them is then the one); empty where none fits, and
    /// then every elective module of the program may be meant.
    pub areas: Vec<CatalogArea>,
    /// Further areas the name fits, less well: not listed, and since 2026-09-23 no longer named
    /// in the catalog's note either (the owner's short form of it leaves them out).
    pub others: Vec<CatalogArea>,
}

impl SemesterRequirement {
    /// Several areas fit equally well: all of them are meant, none is the one.
    pub fn ambiguous(&self) -> bool {
        self.areas.len() > 1
    }

    /// Whether the name says nothing that the names of its areas do not, so that a page may name
    /// the areas alone (owner, 2026-09-23: the note of a semester said everything twice): „Modul
    /// aus dem Bereich Praktische Mathematik" beside „Praktische Mathematik", „Wahlpflicht:
    /// Komplex A / Komplex B" beside A and B, „- Wahlpflicht Wirtschaftswissenschaften (gemäß
    /// Anlage 3)" beside „Wirtschaftswissenschaftlicher Bereich". A word the areas lack says
    /// more: „Anwendungsfach" what „Mathematik", „Physik" … are for, „Wahlpflichtmodule der
    /// gewählten Studienrichtung" what „Wahlpflichtmodule (KT)" and „Zweite Fremdsprache" are.
    /// Only the kind of group may be missing where the name spells the groups out („Module aus
    /// dem gewählten Schwerpunkt „A“ oder „B“"). Where it is all the name has, it counts like
    /// any word: „Module aus dem Schwerpunkt" says no more than „Schwerpunkt Marketing", and
    /// „Schwerpunkt 1" more than „Geotechnik".
    pub fn named_by_areas(&self) -> bool {
        const GROUPS: &[&str] = &["schwerpunkt", "studienrichtung", "vertiefung"];
        let names: Vec<String> = self.areas.iter().flat_map(|area| distinctive(area.name())).collect();
        let known = |word: &String| names.iter().any(|name| name == word || akin(word, name));
        let words = distinctive(self.shown_name());
        let spelled: Vec<&String> = words.iter().filter(|word| !GROUPS.contains(&word.as_str())).collect();
        if spelled.iter().any(|word| !word.chars().all(|c| c.is_ascii_digit())) {
            spelled.into_iter().all(known)
        } else {
            !words.is_empty() && words.iter().all(known)
        }
    }

    /// The name as a page shows it: without the dash a plan leads into a row or out of it with
    /// („- Wahlpflicht Energiesysteme", „… (Sport, Musik oder Kunst) -") and without the marks of
    /// its footnotes („Integrationsmodule**", „Informatik¹").
    pub fn shown_name(&self) -> &str {
        let trimmed = self.name.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '—' | '*' | '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹' | '⁰'));
        if trimmed.is_empty() {
            self.name.trim()
        } else {
            trimmed
        }
    }
}

/// What the plan asks for in one semester besides the modules it places there.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemesterPlan {
    pub semester: u8,
    pub requirements: Vec<SemesterRequirement>,
}

impl SemesterPlan {
    /// The areas whose modules can be chosen for the semester's rows: every area a row points
    /// at, ties included (where two fit equally well, both are offered).
    pub fn area_ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = Vec::new();
        for id in self.requirements.iter().filter(|row| !row.single && !row.fues).flat_map(|row| row.areas.iter().map(|area| area.id)) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// A choice the name points nowhere with: every elective module of the program may be meant.
    pub fn any_elective(&self) -> bool {
        self.requirements.iter().any(|row| !row.single && !row.fues && row.areas.is_empty())
    }
}

/// The rows of the plan that lie in `semester` (a row over several semesters lies in each of
/// them) and name no module, with the areas they point at.
pub fn semester_plan(semester: u8, entries: &[PlanEntry], areas: &[CatalogArea]) -> SemesterPlan {
    let mut plan = SemesterPlan { semester, requirements: Vec::new() };
    for entry in entries.iter().filter(|entry| entry.module_id.is_none()) {
        let in_semester = semester_span(entry).is_some_and(|(from, to)| from <= i64::from(semester) && i64::from(semester) <= to);
        if !in_semester || entry.module_name.trim().is_empty() {
            continue;
        }
        let (single, fues) = (is_single_module(entry), is_fues(entry));
        let found = if single || fues {
            RowAreas::default()
        } else {
            // The rows of the same plan (one per study direction) tell what this row does not mean.
            let rows: Vec<PlanEntry> = entries.iter().filter(|other| other.specialization == entry.specialization).cloned().collect();
            areas_for_row(entry, entry.specialization.as_deref().unwrap_or_default(), areas, &rows)
        };
        let credits = credits_of(entry);
        let (listed, others) = (found.areas, found.others);
        let add = |into: &mut Vec<CatalogArea>, areas: Vec<CatalogArea>| {
            for area in areas {
                if !into.iter().any(|known| known.id == area.id) {
                    into.push(area);
                }
            }
        };
        match plan.requirements.iter_mut().find(|row| row.name == entry.module_name && row.credits == credits && row.kind == entry.kind) {
            Some(row) => {
                // The same row in the plan of another study direction: its areas count as well.
                add(&mut row.areas, listed);
                add(&mut row.others, others);
                let listed: Vec<i64> = row.areas.iter().map(|area| area.id).collect();
                row.others.retain(|area| !listed.contains(&area.id));
            }
            None => plan.requirements.push(SemesterRequirement { name: entry.module_name.clone(), credits, kind: entry.kind.clone(), single, fues, areas: listed, others }),
        }
    }
    plan
}

// ---------- what the plan adds up to ----------
//
// A plan row that says „10–24 LP" does not say how many of them count. Adding the lower bounds
// made the Bachelor Informatik a degree of 166 LP instead of 180, and the last two semesters,
// printed as one merged column, had no sum at all. The regulation does say: it prints a line per
// semester and a line per section, and `plan_total` keeps those lines with the rows each of them
// counts. What follows reads them; where a plan prints none, the caller falls back to its rows.

/// The sums of one plan variant. A document that prints several plans keeps them apart by the
/// caption above each („Regelstudienplan der Studienrichtungen MIT und EET"), the same name the
/// rows carry as their specialization.
pub fn totals_of_variant<'a>(totals: &'a [PlanTotal], specialization: &str) -> Vec<&'a PlanTotal> {
    let named = totals.iter().any(|total| total.specialization.is_some());
    totals
        .iter()
        .filter(|total| !named || total.specialization.as_deref().unwrap_or_default() == specialization)
        .collect()
}

/// What the whole plan comes to, as the regulation prints it: its own semester sums, each counted
/// once. The two ends differ where the regulation prints a span for a semester instead of a number.
/// `None` where the plan prints no sums at all — the rows are then all there is.
pub fn stated_credits(totals: &[&PlanTotal]) -> Option<(f64, f64)> {
    let mut covered: Vec<(i64, i64)> = Vec::new();
    let (mut low, mut high) = (0.0, 0.0);
    for total in whole_plan_totals(totals) {
        let overlaps = covered.iter().any(|(from, to)| total.start_semester <= *to && total.end_semester >= *from);
        if overlaps {
            continue;
        }
        covered.push((total.start_semester, total.end_semester));
        low += total.credits;
        high += total.credits_max;
    }
    (!covered.is_empty()).then_some((low, high))
}

/// The plan's own sums, widest span first and, for the same span, the one that counts the most
/// rows: a plan printing both „1.–6." and one line per semester is counted once, and one printing
/// two lines over the same semesters is read by the line that covers the whole plan.
fn whole_plan_totals<'a>(totals: &[&'a PlanTotal]) -> Vec<&'a PlanTotal> {
    let mut out: Vec<&PlanTotal> = totals.iter().copied().filter(|total| total.is_whole_plan()).collect();
    out.sort_by_key(|total| (total.start_semester - total.end_semester, total.start_semester, -total.entry_count));
    out
}

/// What the plan states for exactly these semesters, if it states anything: a number, or the span
/// the regulation prints in its place.
pub fn stated_for_span(totals: &[&PlanTotal], from: i64, to: i64) -> Option<(f64, f64)> {
    whole_plan_totals(totals)
        .into_iter()
        .find(|total| total.start_semester == from && total.end_semester == to)
        .map(|total| (total.credits, total.credits_max))
}

/// The sum that ties a row to the other rows it is chosen with: the one over the fewest rows that
/// leaves a choice („Summe Komplexe des Fachstudiums 44" over three rows of „10–24"). A row whose
/// own credits are already fixed is tied to nothing, however many sums count it.
pub fn choice_of<'a>(totals: &[&'a PlanTotal], entry: &PlanEntry) -> Option<&'a PlanTotal> {
    entry.max_credits?;
    totals
        .iter()
        .copied()
        .filter(|total| total.is_choice && total.entries.contains(&entry.ord))
        .min_by_key(|total| total.entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area_fixtures::{self as real, row};

    const E: Option<ModuleKind> = Some(ModuleKind::Elective);

    fn labels(areas: &[CatalogArea]) -> Vec<&str> {
        areas.iter().map(|area| area.label.as_str()).collect()
    }

    fn paths(areas: &[CatalogArea]) -> Vec<&str> {
        areas.iter().map(|area| area.path.as_str()).collect()
    }

    const MIT_EET: &str = "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium";
    const PA_IOT: &str = "Regelstudienplan der Studienrichtungen PA und IoT im grundständigen Studium";

    fn total(label: &str, scope: &str, from: i64, to: i64, credits: f64, min: f64, max: f64, entries: &[i64]) -> PlanTotal {
        PlanTotal {
            ord: entries.first().copied().unwrap_or(0),
            label: label.to_string(),
            scope: Code::parse(scope),
            specialization: None,
            start_semester: from,
            end_semester: to,
            credits,
            min_credits: min,
            max_credits: max,
            credits_max: credits,
            is_choice: max - min > 0.01,
            entry_count: entries.len() as i64,
            entries: entries.to_vec(),
        }
    }

    // The plan of Informatik B.Sc.: „Summe Studium" 32 28 30 30 and 60 for the two semesters it
    // prints as one column, and „Summe Komplexe des Fachstudiums 44" over three rows of „10–24".
    #[test]
    fn a_plan_is_added_up_the_way_its_regulation_does() {
        let totals = vec![
            total("Summe Studium", "plan", 1, 1, 32.0, 32.0, 32.0, &[1]),
            total("Summe Studium", "plan", 2, 2, 28.0, 28.0, 28.0, &[2]),
            total("Summe Studium", "plan", 3, 3, 30.0, 30.0, 30.0, &[3]),
            total("Summe Studium", "plan", 4, 4, 30.0, 30.0, 30.0, &[4]),
            total("Summe Studium", "plan", 5, 6, 60.0, 46.0, 88.0, &[5, 6, 7, 8, 9]),
            total("Summe Komplexe des Fachstudiums", "section", 5, 6, 44.0, 30.0, 72.0, &[5, 6, 7]),
        ];
        let all: Vec<&PlanTotal> = totals.iter().collect();
        assert_eq!(stated_credits(&all), Some((180.0, 180.0)));
        assert_eq!(stated_for_span(&all, 5, 6), Some((60.0, 60.0)));
        assert_eq!(stated_for_span(&all, 5, 5), None, "the plan sums 5 and 6 together and says nothing about either alone");
        assert_eq!(stated_credits(&[]), None, "a plan that prints no sums is added up from its rows");

        // The narrowest sum that leaves a choice is the one that ties an elective row to the others.
        let elective = PlanEntry { ord: 6, min_credits: Some(10.0), max_credits: Some(24.0), credits: None, ..row("Komplex Praktische Informatik", 5, E, None) };
        assert_eq!(choice_of(&all, &elective).map(|total| total.credits), Some(44.0));
        let thesis = PlanEntry { ord: 9, ..row("Bachelor-Arbeit", 6, Some(ModuleKind::Thesis), None) };
        assert!(choice_of(&all, &thesis).is_none(), "a row with a number of its own is tied to nothing");
    }

    // A document that prints one plan per study direction keeps their sums apart.
    #[test]
    fn the_sums_of_a_plan_belong_to_that_plan() {
        let mine = PlanTotal { specialization: Some(MIT_EET.to_string()), ..total("Summe Studium", "plan", 1, 1, 30.0, 30.0, 30.0, &[1]) };
        let other = PlanTotal { specialization: Some(PA_IOT.to_string()), ..total("Summe Studium", "plan", 1, 1, 28.0, 28.0, 28.0, &[2]) };
        let totals = vec![mine, other];
        assert_eq!(stated_credits(&totals_of_variant(&totals, MIT_EET)), Some((30.0, 30.0)));
        assert_eq!(stated_credits(&totals_of_variant(&totals, PA_IOT)), Some((28.0, 28.0)));
    }

    #[test]
    fn a_row_that_names_an_area_means_that_area_and_no_other() {
        // The rows of the plan of Informatik B.Sc. against its tree (snapshot of 2026-09-21).
        let areas = real::areas(real::INFORMATIK_BSC);
        let rows = real::informatik_bsc_rows();
        let find = |name: &str| areas_for_row(&row(name, 5, E, None), "", &areas, &rows);

        // „Komplex" says nothing about which area; the rest is the area's name.
        let found = find("Komplex Praktische Informatik");
        assert_eq!(labels(&found.areas), vec!["Praktische Informatik"]);
        assert!(!found.ambiguous());
        assert!(found.others.is_empty(), "nothing else comes near: {:?}", labels(&found.others));
        assert_eq!(labels(&find("Komplex Angewandte und Technische Informatik").areas), vec!["Angewandte und Technische Informatik"]);
        assert_eq!(labels(&find("Komplex Grundlagen der Informatik").areas), vec!["Grundlagen der Informatik"]);
        let found = find("Proseminar oder Praktikum");
        assert_eq!(labels(&found.areas), vec!["Proseminar oder Praktikum"]);
        assert!(found.others.is_empty());
        // A name within an area's name: the one it lies in.
        assert_eq!(labels(&find("Seminar oder Praktikum").areas), vec!["Seminar oder Praktikum aus der Informatik"]);
        // The leading „Wahlpflichtmodule" of the area's label and „Modul aus dem Bereich" of the
        // row say nothing either.
        let found = find("Modul aus dem Bereich Praktische Mathematik");
        assert_eq!(labels(&found.areas), vec!["Wahlpflichtmodule Praktische Mathematik"]);
        assert!(!found.ambiguous());

        // A fixed area is never meant, however well its name fits: what is left of this name is
        // the Nebenfach of the same name, which is a choice.
        let found = find("Komplex Mathematik");
        assert_eq!(paths(&found.areas), vec!["Grundstudium / Komplex Nebenfach / Mathematik"]);

        // The Master of the same subject reads the same.
        let master = real::areas(real::INFORMATIK_MSC);
        let found = areas_for_row(&row("Komplex Praktische Informatik", 1, E, None), "", &master, &[]);
        assert_eq!(paths(&found.areas), vec!["Informatik-Vertiefung / Praktische Informatik"]);
        assert_eq!(labels(&areas_for_row(&row("Seminare oder Praktika", 2, E, None), "", &master, &[]).areas), vec!["Seminare oder Praktika"]);
    }

    #[test]
    fn a_row_that_lists_areas_means_each_of_them() {
        let areas = real::areas(real::INFORMATIK_BSC);
        let rows = real::informatik_bsc_rows();
        let name = "Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik";
        let found = areas_for_row(&row(name, 4, E, None), "", &areas, &rows);
        assert!(found.ambiguous());
        assert_eq!(labels(&found.areas), vec!["Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik"]);
        assert!(found.others.is_empty());
    }

    #[test]
    fn a_row_that_names_a_node_above_means_the_areas_below_it_that_no_other_row_names() {
        let areas = real::areas(real::INFORMATIK_BSC);
        let rows = real::informatik_bsc_rows();
        // „Anwendungsfach" is the other word for „Nebenfach"; Praktische Mathematik lies in the
        // Komplex Nebenfach as well, but the plan asks for it in a row of its own.
        let found = areas_for_row(&row("Anwendungsfach", 3, E, None), "", &areas, &rows);
        assert!(found.ambiguous());
        assert_eq!(labels(&found.areas), vec!["Mathematik", "Maschinenbau / Elektrotechnik", "Wirtschaftswissenschaften", "Bauingenieurwesen", "Physik"]);
        // Without the other rows of the plan, all of the complex.
        let alone = areas_for_row(&row("Anwendungsfach", 3, E, None), "", &areas, &[]);
        assert_eq!(alone.areas.len(), 6);
        // … and a leaf named with its node above is that leaf.
        let found = areas_for_row(&row("Nebenfach Physik", 3, E, None), "", &areas, &rows);
        assert_eq!(labels(&found.areas), vec!["Physik"]);
        assert!(found.others.is_empty(), "{:?}", labels(&found.others));

        // In the Master, the Nebenfach holds Mathematik and the Anwendungen below it.
        let master = real::areas(real::INFORMATIK_MSC);
        let found = areas_for_row(&row("Anwendungsfach", 1, E, None), "", &master, &[]);
        assert_eq!(found.areas.len(), 6, "{:?}", paths(&found.areas));

        // A name that fits nothing keeps quiet: every elective may be meant.
        assert_eq!(areas_for_row(&row("Freie Wahl", 6, E, None), "", &areas, &rows), RowAreas::default());
        assert_eq!(areas_for_row(&row("Wahlpflichtmodul 3", 6, E, None), "", &areas, &rows), RowAreas::default());
    }

    #[test]
    fn a_row_of_the_plan_points_at_the_areas_of_its_own_study_direction() {
        // Elektrotechnik B.Sc. (PO 2022): one plan for MIT and EET, one for PA and IoT; the tree
        // spells PA „PAu".
        let areas = real::areas(real::ELEKTROTECHNIK_BSC);
        let find = |name: &str, plan: &str| areas_for_row(&row(name, 5, E, Some(plan)), plan, &areas, &[]);

        // Only the directions of this plan, and with two of them nothing is picked as the one.
        // „Wahlpflichtmodul (MIT)" says nothing but its kind: it is the list of „Informatik (MIT)".
        let found = find("Wahlpflichtmodul aus der Informatik", MIT_EET);
        assert!(found.ambiguous(), "two directions fit, so none is shown as the one");
        assert_eq!(labels(&found.areas), vec!["Wahlpflichtmodul (MIT)", "Wahlpflichtmodul (EET)"]);

        // „der Studienrichtung" finds the areas whose name starts the same way.
        let found = find("Wahlpflichtmodule der Studienrichtung", MIT_EET);
        assert_eq!(labels(&found.areas), vec!["Studienrichtungsspezifische Vertiefungsmodule (EET)", "Studienrichtungsspezifische Vertiefungsmodule (MIT)"]);
        let found = find("Wahlpflichtmodule der Studienrichtung", PA_IOT);
        assert_eq!(labels(&found.areas), vec!["Studienrichtungsspezifische Vertiefungsmodule (IoT)", "Studienrichtungsspezifische Vertiefungsmodule (PAu)"]);
        // Without a caption (one plan) the directions do not narrow anything down.
        assert_eq!(find("Wahlpflichtmodule der Studienrichtung", "").areas.len(), 4);
        // Two named modules, one of them to take: no area.
        assert!(find("Industriefachpraktikum oder Praxisorientiertes Studienprojekt", MIT_EET).areas.is_empty());

        // A caption that spells a direction out finds it by the name in front of its short name.
        let umwelt = real::areas(&[
            (1, "Studienrichtungsspezifische Module (UMT)", &[]),
            (2, "Umwelttechnik (UMT)", &[(18, None)]),
            (2, "Methoden (UMT)", &[(3, E)]),
            (1, "Studienrichtungsspezifische Module (WAM)", &[]),
            (2, "Wassermanagement (WAM)", &[(12, None)]),
            (2, "Methoden (WAM)", &[(3, E)]),
        ]);
        let plan = "Regelstudienplan für die Studienrichtung „Umwelttechnik“";
        let found = areas_for_row(&row("Wahlpflichtmodul Grundlagen Methoden gem. Anlage a5", 5, E, Some(plan)), plan, &umwelt, &[]);
        assert_eq!(labels(&found.areas), vec!["Methoden (UMT)"]);

        // Elektrotechnik M.Sc.: „Wahlpflichtmodule (KT)" is the list of its study direction.
        let master = real::areas(real::ELEKTROTECHNIK_MSC);
        let found = areas_for_row(&row("Wahlpflichtmodule der gewählten Studienrichtung", 2, E, None), "", &master, &[]);
        assert!(found.ambiguous());
        for label in ["Wahlpflichtmodule (KT)", "Wahlpflichtmodule (PAu)", "Wahlpflichtmodule (ES)"] {
            assert!(labels(&found.areas).contains(&label), "{label}: {:?}", labels(&found.areas));
        }
    }

    #[test]
    fn a_row_that_names_the_fues_is_the_fues() {
        for name in ["Fachübergreifendes Studium", "FÜS", "Wahlpflichtmodul aus dem FÜS-Katalog der BTU", "Modul zum fachübergreifenden Studium", "- wählbar aus dem FÜSModulangebot der BTU", "Fächerübergreifendes Studium (gemäß BTU-FÜSModulangebot)"] {
            assert!(is_fues(&row(name, 1, E, None)), "{name}");
        }
        assert!(is_fues(&row("Modul", 1, Some(ModuleKind::Fues), None)));
        assert!(!is_fues(&row("Wahlpflichtmodul aus der Informatik", 1, E, None)));
        assert!(!is_fues(&row("Fusionsforschung", 1, E, None)));
    }

    #[test]
    fn the_words_of_a_name_are_what_tells_it_apart() {
        // What the plans print around a name: numbering, footnotes, references into appendices.
        assert_eq!(distinctive("5 Berufspraktische Vorbereitung (Wahl eines Modules aus Anlage 2a)"), distinctive("Berufspraktische Vorbereitung"));
        assert_eq!(distinctive("Wirtschaftswissenschaften2"), distinctive("Wirtschaftswissenschaften"));
        assert_eq!(distinctive("- Wahlpflicht Rechtswissenschaften (gemäß Anlage 2) Prü/SL"), distinctive("Rechtswissenschaft"));
        // Roman numbers, plural and singular.
        assert_eq!(distinctive("2 Schwerpunktmodul II"), distinctive("Schwerpunktmodul 2"));
        assert_ne!(distinctive("Schwerpunktmodul I"), distinctive("Schwerpunktmodul 2"));
        assert_eq!(distinctive("Wahlbereich Grundlagen der Statistik"), distinctive("Wahlbereich Grundlage der Statistik"));
        // A name of nothing but numbers or kinds says nothing.
        assert!(distinctive("Wahlpflichtmodul 3").is_empty());
        assert!(distinctive("Compulsory Elective and Optional Modules").is_empty());
        assert!(distinctive("WPF").is_empty());
        // Anwendungsfach, Anwendungen, Nebenfach are one word.
        assert_eq!(distinctive("Anwendungsfach"), distinctive("Komplex Nebenfach"));
        // Words in common count where they are half of the row, numbers never.
        assert_eq!(fit(&distinctive("Physics of Modern Devices oder Introduction to Microwave Electronics"), &distinctive("Technology and Devices")), 0);
        assert!(fit(&distinctive("Herrschaftsstrukturen und Diskriminierung"), &distinctive("Herrschaftsverhältnisse und Diskriminierung")) > 0);
        assert_eq!(fit(&distinctive("Schwerpunkt 1"), &distinctive("Konstruktiver Ingenieurbau - 1")), 0);
        assert_eq!(fit(&distinctive("Bau- und Stadtbaugeschichte 1"), &distinctive("Stadt und Landschaft")), 0);
        assert!(fit(&distinctive("Rechtswissenschaften"), &distinctive("Rechtswissenschaftlicher Bereich")) > 0);
    }

    #[test]
    fn a_semester_asks_for_what_its_rows_name() {
        let areas = real::areas(real::INFORMATIK_BSC);
        let id = |label: &str| areas.iter().find(|area| area.label == label).map(|area| area.id).unwrap_or_default();
        let entries = vec![
            row("Komplex Praktische Informatik", 5, E, None),
            row("Komplex Praktische Informatik", 5, E, Some("Regelstudienplan der Studienrichtung B")),
            row("Komplex Praktische Informatik", 6, E, None),
            row("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik", 5, E, None),
            row("Freie Wahl", 5, E, None),
            row("Bachelor-Arbeit", 5, Some(ModuleKind::Thesis), None),
            row("Fachübergreifendes Studium", 5, E, None),
            PlanEntry { module_id: Some("11101".into()), ..row("Lineare Algebra", 5, Some(ModuleKind::Compulsory), None) },
        ];
        let plan = semester_plan(5, &entries, &areas);
        let names: Vec<&str> = plan.requirements.iter().map(|row| row.name.as_str()).collect();
        // The same row of two study directions is one; the linked row and the 6th semester are not here.
        assert_eq!(names, vec!["Komplex Praktische Informatik", entries[3].module_name.as_str(), "Freie Wahl", "Bachelor-Arbeit", "Fachübergreifendes Studium"]);
        assert_eq!(plan.requirements[0].areas.iter().map(|area| area.id).collect::<Vec<_>>(), vec![id("Praktische Informatik")]);
        assert!(plan.requirements[0].others.is_empty());
        assert!(plan.requirements[1].ambiguous());
        assert_eq!(
            plan.area_ids(),
            vec![id("Praktische Informatik"), id("Grundlagen der Informatik"), id("Angewandte und Technische Informatik")],
            "the areas of the rows, each once, never the thesis or the FÜS row"
        );
        assert!(plan.any_elective(), "a row that points at no area means every elective");
        assert!(plan.requirements.iter().any(|row| row.single && row.name == "Bachelor-Arbeit"));
        assert!(plan.requirements.iter().any(|row| row.fues && row.name == "Fachübergreifendes Studium"), "the FÜS by its name");
        assert_eq!(plan.requirements[0].credits.as_deref(), Some("6"));

        // A span of semesters lies in each of them.
        let spanning = PlanEntry { semester: None, start_semester: Some(5), end_semester: Some(6), ..row("Wahlpflichtmodule", 5, None, None) };
        assert_eq!(semester_plan(6, std::slice::from_ref(&spanning), &areas).requirements.len(), 1);
        assert_eq!(semester_plan(4, &[spanning], &areas).requirements.len(), 0);
        assert_eq!(credits_of(&PlanEntry { credits: None, min_credits: Some(10.0), max_credits: Some(24.0), ..row("x", 1, None, None) }).as_deref(), Some("10–24"));
        assert_eq!(credits_of(&PlanEntry { credits: Some(7.5), ..row("x", 1, None, None) }).as_deref(), Some("7,5"));
    }

    // The note of a semester names the areas alone where the row's name says no more.
    #[test]
    fn a_name_that_only_repeats_its_areas_says_nothing() {
        let areas = real::areas(real::INFORMATIK_BSC);
        let rows = real::informatik_bsc_rows();
        let requirement = |name: &str, areas: &[CatalogArea], rows: &[PlanEntry]| {
            let found = areas_for_row(&row(name, 4, E, None), "", areas, rows);
            SemesterRequirement { name: name.to_string(), credits: Some("6".into()), kind: None, single: false, fues: false, areas: found.areas, others: found.others }
        };
        // Informatik B.Sc., 3rd and 4th semester (owner, 2026-09-23).
        assert!(requirement("Modul aus dem Bereich Praktische Mathematik", &areas, &rows).named_by_areas());
        assert!(requirement("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik", &areas, &rows).named_by_areas());
        assert!(!requirement("Anwendungsfach", &areas, &rows).named_by_areas(), "what the areas are for");
        assert!(!requirement("Wahlpflichtmodul 3", &areas, &rows).named_by_areas(), "no area at all");

        // A word of the name that is a word of an area's name, or its start; a footnote says nothing.
        let economics = real::areas(&[(1, "Wirtschaftswissenschaftlicher Bereich", &[(6, E)]), (1, "Rechtswissenschaftlicher Bereich", &[(4, E)])]);
        assert!(requirement("- Wahlpflicht Wirtschaftswissenschaften (gemäß Anlage 3) Prü/SL", &economics, &[]).named_by_areas());
        let own = real::areas(&[(1, "Fachspezifischer Wahlpflichtbereich", &[(6, E)])]);
        assert!(requirement("Fachspezifischer Wahlpflichtbereich¹", &own, &[]).named_by_areas());

        // The kind of group may go unnamed where the groups are spelled out, not where it is all
        // the name says.
        let focus = real::areas(&[(1, "Schwerpunkt Marketing", &[(5, E)]), (1, "Schwerpunkt Controlling", &[(5, E)])]);
        assert!(requirement("Module aus dem Schwerpunkt", &focus, &[]).named_by_areas());
        let spelled = real::areas(&[(1, "Wirtschaft, Arbeit und Unternehmenspraxis", &[(5, E)]), (1, "Kommunikation, Medien und Technologiegestaltung", &[(5, E)])]);
        assert!(requirement("Module aus dem gewählten Schwerpunkt „Wirtschaft, Arbeit und Unternehmenspraxis“ oder „Kommunikation, Medien und Technologiegestaltung“", &spelled, &[]).named_by_areas());
        let civil = real::areas(&[(1, "Geotechnik (BIW)", &[(5, E)]), (1, "Konstruktiver Ingenieurbau - 1 (BIW)", &[(5, E)])]);
        let first = SemesterRequirement { areas: civil, ..requirement("- Schwerpunkt 1", &[], &[]) };
        assert!(!first.named_by_areas(), "the plan's name for the row");
        let master = real::areas(real::ELEKTROTECHNIK_MSC);
        assert!(!requirement("Wahlpflichtmodule der gewählten Studienrichtung", &master, &[]).named_by_areas());

        // What a plan prints around a name is not shown with it.
        let shown = |name: &str| SemesterRequirement { name: name.to_string(), ..requirement("x", &[], &[]) }.shown_name().to_string();
        assert_eq!(shown("- Wahlpflicht Energiesysteme"), "Wahlpflicht Energiesysteme");
        assert_eq!(shown("Teilbereich Ästhetische Bildung (ÄB) - Wahlpflichtmodul 1 (Sport, Musik oder Kunst) -"), "Teilbereich Ästhetische Bildung (ÄB) - Wahlpflichtmodul 1 (Sport, Musik oder Kunst)");
        assert_eq!(shown("Integrationsmodule**"), "Integrationsmodule");
        assert_eq!(shown("Schwerpunktmodule³"), "Schwerpunktmodule");
        assert_eq!(shown("Wahlpflichtmodul 3"), "Wahlpflichtmodul 3");
    }
}
