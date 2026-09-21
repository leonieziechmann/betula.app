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

/// The areas a row of the plan points at, most fitting first (at most four), and how many of
/// them share the best fit: more than one, and none of them is the one — a page names them
/// instead of picking one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowAreas {
    pub areas: Vec<CatalogArea>,
    pub tied: usize,
}

impl RowAreas {
    pub fn ambiguous(&self) -> bool {
        self.tied > 1
    }
}

/// Which areas of the program a row of the plan is about. What the row and an area have in
/// common decides („Wahlpflichtmodul" and the like count little, a distinctive word counts), and
/// the study direction the plan is printed for keeps the other directions out („Wahlpflichtmodul
/// aus der **Informatik**" in the plan of MIT and EET → „Informatik (MIT)", „Informatik (EET)",
/// never the areas of PA or IoT). `plan` is the caption of the plan („Regelstudienplan der
/// Studienrichtungen MIT und EET …"), empty where a program has one plan.
pub fn areas_for_row(entry: &PlanEntry, plan: &str, areas: &[CatalogArea]) -> RowAreas {
    let words = |text: &str| -> Vec<String> {
        crate::search::fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| word.len() >= 4).map(str::to_string).collect()
    };
    // „Wahlpflichtmodul" says what kind it is, not which area: it may match, but weighs less —
    // on either side, so „Freie Wahl" does not land in every „Wahlpflichtmodule …" area.
    let generic = ["wahl", "wahlpflicht", "wahlpflichtmodul", "wahlpflichtmodule", "modul", "module", "pflicht", "pflichtmodul", "pflichtmodule", "studium", "katalog"];
    let row_words = words(&entry.module_name);
    // The directions this plan is printed for, as its caption spells them („MIT und EET").
    let plan_parts: Vec<&str> = plan.split(|c: char| !c.is_alphanumeric()).filter(|part| !part.is_empty()).collect();
    let directions: Vec<&str> = areas.iter().filter_map(|area| direction_of(&area.label)).filter(|direction| plan_parts.contains(direction)).collect();

    let mut scored: Vec<(i32, &CatalogArea)> = areas
        .iter()
        .filter_map(|area| {
            let area_words = words(&area.label);
            let matches = |a: &String, b: &String| a.starts_with(b.as_str()) || b.starts_with(a.as_str());
            // The name has to carry the match: a word of the row that is not „Wahlpflichtmodul"
            // and the like. The study direction only ranks what the name already found.
            let mut score = 0;
            for word in &row_words {
                if let Some(other) = area_words.iter().find(|other| matches(word, other)) {
                    score += if generic.contains(&word.as_str()) || generic.contains(&other.as_str()) { 1 } else { 3 };
                }
            }
            if score < 3 {
                return None;
            }
            // Where the plans are printed per study direction, only this plan's areas can be meant.
            if !directions.is_empty() {
                match direction_of(&area.label) {
                    Some(direction) if directions.contains(&direction) => score += 3,
                    Some(_) => return None,
                    None => {}
                }
            }
            Some((score, area))
        })
        .collect();
    scored.sort_by(|(a, left), (b, right)| b.cmp(a).then(right.modules.cmp(&left.modules)));
    let best = scored.first().map(|(score, _)| *score).unwrap_or(0);
    let tied = scored.iter().filter(|(score, _)| *score == best).count();
    RowAreas { areas: scored.into_iter().take(4).map(|(_, area)| area.clone()).collect(), tied }
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
        let mut listed = found.areas;
        let others = listed.split_off(found.tied.min(listed.len()));
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
        CatalogArea { id, label: label.to_string(), path: format!("Grundstudium / {label}"), depth: 2, modules }
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
        assert_eq!(found.areas.iter().map(|area| area.label.as_str()).collect::<Vec<_>>(), vec!["Informatik (MIT)", "Informatik (EET)"]);

        // „der Studienrichtung" finds the area whose name starts the same way.
        let found = areas_for_row(&row("Wahlpflichtmodule der Studienrichtung", Some(2), None, None), plan, &areas);
        assert_eq!(found.areas.first().map(|area| area.label.as_str()), Some("Studienrichtungsspezifische Vertiefungsmodule (MIT)"));
        assert!(!found.ambiguous());

        // A row that fits nothing keeps quiet.
        assert_eq!(areas_for_row(&row("Bachelor-Arbeit", Some(6), None, None), plan, &areas), RowAreas::default());
    }

    #[test]
    fn a_semester_asks_for_what_its_rows_name() {
        let areas = vec![area(1, "Wahlpflichtmodule Praktische Informatik", 30), area(2, "Wahlpflichtmodule Theoretische Informatik", 20), area(3, "Pflichtmodule Mathematik", 10)];
        let entries = vec![
            row("Wahlpflichtmodule Praktische Informatik", Some(3), Some(ModuleKind::Elective), None),
            row("Wahlpflichtmodule Praktische Informatik", Some(3), Some(ModuleKind::Elective), Some("Regelstudienplan der Studienrichtung B")),
            row("Wahlpflichtmodule Praktische Informatik", Some(4), Some(ModuleKind::Elective), None),
            row("Freie Wahl", Some(3), Some(ModuleKind::Elective), None),
            row("Bachelor-Arbeit", Some(3), Some(ModuleKind::Thesis), None),
            row("Modul aus dem FÜS", Some(3), Some(ModuleKind::Fues), None),
            PlanEntry { module_id: Some("11101".into()), ..row("Lineare Algebra", Some(3), Some(ModuleKind::Compulsory), None) },
        ];
        let plan = semester_plan(3, &entries, &areas);
        let names: Vec<&str> = plan.requirements.iter().map(|row| row.name.as_str()).collect();
        // The same row of two study directions is one; the linked row and the 4th semester are not here.
        assert_eq!(names, vec!["Wahlpflichtmodule Praktische Informatik", "Freie Wahl", "Bachelor-Arbeit", "Modul aus dem FÜS"]);
        assert_eq!(plan.area_ids(), vec![1], "only the area the choice fits best, not the thesis or the FÜS row");
        assert_eq!(plan.requirements[0].others.iter().map(|area| area.id).collect::<Vec<_>>(), vec![2], "the area that fits less well is named, not listed");
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
}
