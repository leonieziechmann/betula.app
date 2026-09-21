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
use crate::rows_detail::PlanEntry;

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
/// program's FÜS list, not its tree.
pub fn is_fues(entry: &PlanEntry) -> bool {
    entry.kind.as_ref().is_some_and(|kind| kind.is(ModuleKind::Fues))
}

/// The study direction an area's label ends in („Informatik (MIT)" → „MIT").
fn direction_of(label: &str) -> Option<&str> {
    let (_, rest) = label.rsplit_once('(')?;
    let (inside, _) = rest.split_once(')')?;
    let inside = inside.trim();
    (!inside.is_empty() && inside.len() <= 12).then_some(inside)
}

/// An area's label without the study direction it ends in („Informatik (MIT)" → „Informatik").
fn without_direction(label: &str) -> &str {
    match direction_of(label) {
        Some(_) => label.rsplit_once('(').map(|(name, _)| name.trim_end()).unwrap_or(label),
        None => label,
    }
}

/// The words of a name that say which area it is: folded, without the words that say what kind
/// of thing it is („Komplex", „Wahlpflichtmodule", „Modul aus dem") and without articles, so that
/// „Komplex Praktische Informatik" and „Praktische Informatik" read the same.
fn distinctive(text: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "komplex", "wahlpflicht", "wahlpflichtmodul", "wahlpflichtmodule", "wahlpflichtbereich", "wahlpflichtfach", "wahl", "modul", "module", "moduls",
        "modulen", "pflicht", "pflichtmodul", "pflichtmodule", "bereich", "bereiche", "bereichs", "studium", "katalog", "lp", "ects", "aus", "dem",
        "der", "des", "die", "das", "den", "im", "in", "und", "oder", "von", "vom", "zum", "zur", "mit", "ein", "eine", "einer",
    ];
    crate::search::fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| word.len() >= 2 && !NOISE.contains(word)).map(str::to_string).collect()
}

/// How well the words of a row fit the words of a name: the same words (100), the one within
/// the other (60, a little more the more they share), some words in common (10 each; a word may
/// be the start of the other: „Studienrichtung", „Studienrichtungsspezifische"), or nothing (0).
fn fit(row: &[String], name: &[String]) -> i32 {
    if row.is_empty() || name.is_empty() {
        return 0;
    }
    let within = |inner: &[String], outer: &[String]| inner.iter().all(|word| outer.contains(word));
    if row.len() == name.len() && within(row, name) {
        return 100;
    }
    let shared = row.iter().filter(|word| name.contains(word)).count() as i32;
    if within(row, name) || within(name, row) {
        return 60 + 5 * shared;
    }
    let starts = |a: &String, b: &String| a.len() >= 5 && b.len() >= 5 && (a.starts_with(b.as_str()) || b.starts_with(a.as_str()));
    10 * row.iter().filter(|word| name.iter().any(|other| starts(word, other))).count() as i32
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

/// Which areas of the program a row of the plan is about. Only areas a student chooses from
/// come into question (`CatalogArea::choice`): a requirement row never means the Pflichtmodule.
/// The name decides: the same name as an area („Komplex Praktische Informatik" — „Praktische
/// Informatik") is that area and no other; a name within an area's name or the other way round
/// („Wahlpflichtmodul aus der Informatik" — every „… Informatik" area) fits next best, and all
/// that fit equally well are meant; some words in common count least. A name that is an area
/// above the leaves („Nebenfach" — „Komplex Nebenfach", with Mathematik, Physik … below it) means
/// the areas below it. The study direction the plan is printed for keeps the other directions
/// out („Wahlpflichtmodul aus der **Informatik**" in the plan of MIT and EET → „Informatik (MIT)",
/// „Informatik (EET)", never the areas of PA or IoT). `plan` is the caption of the plan
/// („Regelstudienplan der Studienrichtungen MIT und EET …"), empty where a program has one plan.
pub fn areas_for_row(entry: &PlanEntry, plan: &str, areas: &[CatalogArea]) -> RowAreas {
    let row_words = distinctive(&entry.module_name);
    if row_words.is_empty() {
        return RowAreas::default();
    }
    // The directions this plan is printed for, as its caption spells them („MIT und EET").
    let plan_parts: Vec<&str> = plan.split(|c: char| !c.is_alphanumeric()).filter(|part| !part.is_empty()).collect();
    let directions: Vec<&str> = areas.iter().filter_map(|area| direction_of(&area.label)).filter(|direction| plan_parts.contains(direction)).collect();

    let mut scored: Vec<(i32, &CatalogArea)> = areas
        .iter()
        .filter(|area| area.choice)
        .filter_map(|area| {
            // Where the plans are printed per study direction, only this plan's areas can be meant.
            let direction = direction_of(&area.label);
            if !directions.is_empty() && direction.is_some_and(|direction| !directions.contains(&direction)) {
                return None;
            }
            let by_name = fit(&row_words, &distinctive(without_direction(&area.label)));
            // A row that names an area above this one: it means what lies below („Nebenfach").
            let by_ancestor = area
                .path
                .rsplit_once(" / ")
                .map(|(above, _)| above.split(" / ").map(|ancestor| fit(&row_words, &distinctive(ancestor))).filter(|score| *score >= 60).max().unwrap_or(0))
                .unwrap_or(0);
            let mut score = by_name + by_ancestor / 2;
            if score > 0 && direction.is_some() {
                score += 5;
            }
            (score > 0).then_some((score, area))
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
    /// Further areas the name fits, less well: named, not listed.
    pub others: Vec<CatalogArea>,
}

impl SemesterRequirement {
    /// Several areas fit equally well: all of them are meant, none is the one.
    pub fn ambiguous(&self) -> bool {
        self.areas.len() > 1
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
        let found = if single || fues { RowAreas::default() } else { areas_for_row(entry, entry.specialization.as_deref().unwrap_or_default(), areas) };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area(id: i64, label: &str, modules: usize) -> CatalogArea {
        CatalogArea { id, label: label.to_string(), path: format!("Grundstudium / {label}"), depth: 2, modules, choice: true, parent: Some("Grundstudium".to_string()) }
    }

    fn under(id: i64, path: &str, modules: usize, choice: bool) -> CatalogArea {
        let mut segments: Vec<&str> = path.split(" / ").collect();
        let label = segments.pop().unwrap_or(path).to_string();
        CatalogArea { id, label, path: path.to_string(), depth: segments.len() as i64 + 1, modules, choice, parent: segments.last().map(|parent| parent.to_string()) }
    }

    fn row(name: &str, semester: Option<i64>, kind: Option<ModuleKind>, specialization: Option<&str>) -> PlanEntry {
        PlanEntry {
            module_id: None,
            module_name: name.to_string(),
            semester,
            start_semester: None,
            end_semester: None,
            semester_span: None,
            credits: Some(6.0),
            min_credits: None,
            max_credits: None,
            kind: kind.map(Code::Known),
            kind_raw: None,
            study_section: None,
            subject_area: None,
            specialization: specialization.map(str::to_string),
            catalog_title: None,
            credits_differ_from_catalog: false,
        }
    }

    fn labels(areas: &[CatalogArea]) -> Vec<&str> {
        areas.iter().map(|area| area.label.as_str()).collect()
    }

    /// The tree of Informatik B.Sc. as the owner described it (2026-09-21), with what is fixed
    /// and what is chosen.
    fn informatik() -> Vec<CatalogArea> {
        vec![
            under(1, "Grundstudium / Komplex Informatik", 9, false),
            under(2, "Grundstudium / Komplex Informatik / Proseminar oder Praktikum", 3, true),
            under(3, "Grundstudium / Komplex Mathematik", 3, false),
            under(4, "Grundstudium / Komplex Nebenfach / Mathematik", 8, true),
            under(5, "Grundstudium / Komplex Nebenfach / Physik", 4, true),
            under(6, "Grundstudium / Komplex Nebenfach / Maschinenbau/Elektrotechnik", 6, true),
            under(7, "Grundstudium / Komplex Nebenfach / Wirtschaftswissenschaften", 5, true),
            under(8, "Grundstudium / Komplex Nebenfach / Bauingenieurwesen", 4, true),
            under(9, "Fachstudium", 1, false),
            under(10, "Fachstudium / Grundlagen der Informatik", 7, true),
            under(11, "Fachstudium / Praktische Informatik", 10, true),
            under(12, "Fachstudium / Angewandte und Technische Informatik", 9, true),
            under(13, "Fachstudium / Seminar oder Praktikum (aus der Informatik)", 4, true),
            under(14, "Fachstudium / Wahlpflichtmodule Praktische Mathematik", 5, true),
        ]
    }

    #[test]
    fn a_row_that_names_an_area_means_that_area_and_no_other() {
        let areas = informatik();
        // „Komplex" says nothing about which area; the rest is the area's name.
        let found = areas_for_row(&row("Komplex Praktische Informatik", Some(5), Some(ModuleKind::Elective), None), "", &areas);
        assert_eq!(labels(&found.areas), vec!["Praktische Informatik"]);
        assert!(!found.ambiguous());
        assert!(found.others.is_empty(), "nothing else comes near: {:?}", labels(&found.others));

        let found = areas_for_row(&row("Komplex Angewandte und Technische Informatik", Some(5), None, None), "", &areas);
        assert_eq!(labels(&found.areas), vec!["Angewandte und Technische Informatik"]);
        let found = areas_for_row(&row("Proseminar oder Praktikum", Some(3), None, None), "", &areas);
        assert_eq!(labels(&found.areas), vec!["Proseminar oder Praktikum"]);
        assert!(found.others.is_empty());
        // The leading „Wahlpflichtmodule" of the area's label says nothing either.
        let found = areas_for_row(&row("Praktische Mathematik", Some(6), None, None), "", &areas);
        assert_eq!(labels(&found.areas), vec!["Wahlpflichtmodule Praktische Mathematik"]);

        // A fixed area is never meant, however well its name fits: what is left of this name is
        // the Nebenfach of the same name, which is a choice.
        let found = areas_for_row(&row("Komplex Mathematik", Some(1), None, None), "", &areas);
        assert!(found.areas.iter().all(|area| area.id != 3), "{:?}", labels(&found.areas));
        assert_eq!(found.areas.iter().map(|area| area.path.as_str()).collect::<Vec<_>>(), vec!["Grundstudium / Komplex Nebenfach / Mathematik"]);
    }

    #[test]
    fn a_row_that_names_a_part_of_the_tree_means_all_that_fit_it() {
        let areas = informatik();
        // „aus der Informatik": every area to choose from whose name says Informatik, none of them the one.
        let found = areas_for_row(&row("Wahlpflichtmodul aus der Informatik", Some(4), None, None), "", &areas);
        assert!(found.ambiguous());
        assert_eq!(labels(&found.areas), vec!["Praktische Informatik", "Angewandte und Technische Informatik", "Grundlagen der Informatik", "Seminar oder Praktikum (aus der Informatik)"]);
        // The Proseminar lies in „Komplex Informatik", so it comes into question, less.
        assert_eq!(labels(&found.others), vec!["Proseminar oder Praktikum"]);
        assert!(!labels(&found.areas).contains(&"Komplex Informatik"), "the fixed complex is no choice");

        // The name of an area above the leaves means the leaves below it.
        let found = areas_for_row(&row("Nebenfach", Some(2), None, None), "", &areas);
        assert!(found.ambiguous());
        assert_eq!(labels(&found.areas), vec!["Mathematik", "Maschinenbau/Elektrotechnik", "Wirtschaftswissenschaften", "Physik", "Bauingenieurwesen"]);
        // … and a leaf named with its area above is that leaf.
        let found = areas_for_row(&row("Nebenfach Physik", Some(2), None, None), "", &areas);
        assert_eq!(labels(&found.areas), vec!["Physik"]);
        assert!(found.others.is_empty(), "{:?}", labels(&found.others));

        // A name that fits nothing keeps quiet: every elective may be meant.
        assert_eq!(areas_for_row(&row("Freie Wahl", Some(6), None, None), "", &areas), RowAreas::default());
        assert_eq!(areas_for_row(&row("Bachelor-Arbeit", Some(6), None, None), "", &areas), RowAreas::default());
    }

    #[test]
    fn a_row_of_the_plan_points_at_the_areas_of_its_own_study_direction() {
        let areas = vec![
            area(1, "Informatik (MIT)", 1),
            area(2, "Informatik (EET)", 1),
            area(3, "Informatik (PAu)", 2),
            area(4, "Informatik (IoT)", 2),
            area(5, "Studienrichtungsspezifische Vertiefungsmodule (MIT)", 23),
            area(6, "Mathematik und Physik (MIT)", 7),
        ];
        let plan = "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium";

        // Only the directions of this plan, and with two of them nothing is picked as the one.
        let found = areas_for_row(&row("Wahlpflichtmodul aus der Informatik", Some(2), None, None), plan, &areas);
        assert!(found.ambiguous(), "two directions fit, so none is shown as the one");
        assert_eq!(labels(&found.areas), vec!["Informatik (MIT)", "Informatik (EET)"]);

        // „der Studienrichtung" finds the area whose name starts the same way.
        let found = areas_for_row(&row("Wahlpflichtmodule der Studienrichtung", Some(2), None, None), plan, &areas);
        assert_eq!(labels(&found.areas), vec!["Studienrichtungsspezifische Vertiefungsmodule (MIT)"]);
        assert!(!found.ambiguous());
        assert!(found.others.is_empty(), "{:?}", labels(&found.others));

        // Without a caption (one plan) the directions do not narrow anything down.
        let found = areas_for_row(&row("Wahlpflichtmodul aus der Informatik", Some(2), None, None), "", &areas);
        assert_eq!(found.areas.len(), 4);
    }

    #[test]
    fn a_semester_asks_for_what_its_rows_name() {
        let areas = informatik();
        let entries = vec![
            row("Komplex Praktische Informatik", Some(5), Some(ModuleKind::Elective), None),
            row("Komplex Praktische Informatik", Some(5), Some(ModuleKind::Elective), Some("Regelstudienplan der Studienrichtung B")),
            row("Komplex Praktische Informatik", Some(6), Some(ModuleKind::Elective), None),
            row("Wahlpflichtmodul aus der Informatik", Some(5), Some(ModuleKind::Elective), None),
            row("Freie Wahl", Some(5), Some(ModuleKind::Elective), None),
            row("Bachelor-Arbeit", Some(5), Some(ModuleKind::Thesis), None),
            row("Modul aus dem FÜS", Some(5), Some(ModuleKind::Fues), None),
            PlanEntry { module_id: Some("11101".into()), ..row("Lineare Algebra", Some(5), Some(ModuleKind::Compulsory), None) },
        ];
        let plan = semester_plan(5, &entries, &areas);
        let names: Vec<&str> = plan.requirements.iter().map(|row| row.name.as_str()).collect();
        // The same row of two study directions is one; the linked row and the 6th semester are not here.
        assert_eq!(names, vec!["Komplex Praktische Informatik", "Wahlpflichtmodul aus der Informatik", "Freie Wahl", "Bachelor-Arbeit", "Modul aus dem FÜS"]);
        assert_eq!(plan.requirements[0].areas.iter().map(|area| area.id).collect::<Vec<_>>(), vec![11]);
        assert!(plan.requirements[0].others.is_empty());
        assert_eq!(plan.requirements[1].areas.len(), 4);
        assert_eq!(plan.requirements[1].others.iter().map(|area| area.id).collect::<Vec<_>>(), vec![2], "the Proseminar in the complex is named, not listed");
        assert_eq!(plan.area_ids(), vec![11, 12, 10, 13], "the areas of the rows, each once, never the thesis or the FÜS row");
        assert!(plan.any_elective(), "a row that points at no area means every elective");
        assert!(plan.requirements.iter().any(|row| row.single && row.name == "Bachelor-Arbeit"));
        assert!(plan.requirements.iter().any(|row| row.fues));
        assert_eq!(plan.requirements[0].credits.as_deref(), Some("6"));

        // A span of semesters lies in each of them.
        let spanning = PlanEntry { semester: None, start_semester: Some(5), end_semester: Some(6), ..row("Wahlpflichtmodule", None, None, None) };
        assert_eq!(semester_plan(6, &[spanning.clone()], &areas).requirements.len(), 1);
        assert_eq!(semester_plan(4, &[spanning], &areas).requirements.len(), 0);
        assert_eq!(credits_of(&PlanEntry { credits: None, min_credits: Some(10.0), max_credits: Some(24.0), ..row("x", None, None, None) }).as_deref(), Some("10–24"));
        assert_eq!(credits_of(&PlanEntry { credits: Some(7.5), ..row("x", None, None, None) }).as_deref(), Some("7,5"));
    }

    #[test]
    fn the_name_of_an_area_drops_the_word_that_says_it_is_a_choice() {
        assert_eq!(crate::pages::short_name("Wahlpflichtmodule Praktische Mathematik"), "Praktische Mathematik");
        assert_eq!(crate::pages::short_name("Wahlpflichtmodul aus dem Nebenfach"), "aus dem Nebenfach");
        assert_eq!(crate::pages::short_name("Wahlpflichtmodule"), "Wahlpflichtmodule");
        assert_eq!(crate::pages::short_name("Praktische Informatik"), "Praktische Informatik");
        assert_eq!(under(1, "Grundstudium / Komplex Nebenfach / Physik", 1, true).parent(), Some("Komplex Nebenfach"));
        assert_eq!(under(1, "Fachstudium", 1, true).parent(), None);
        // The parent comes from the tree, never from the path: a label may hold the separator.
        let slashed = CatalogArea { label: "Maschinenbau / Elektrotechnik".to_string(), path: "Grundstudium / Komplex Nebenfach / Maschinenbau / Elektrotechnik".to_string(), ..under(1, "Grundstudium / Komplex Nebenfach / x", 1, true) };
        assert_eq!(slashed.parent(), Some("Komplex Nebenfach"));
        assert_eq!(slashed.name(), "Maschinenbau / Elektrotechnik");
    }
}
