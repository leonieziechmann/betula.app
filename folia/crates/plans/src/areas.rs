//! The areas of a program's module tree as the catalog's filter offers them (`catalog_areas`),
//! and what a plan's rows and the pickers read of them.

use folia_model::labels::ModuleKind;
use folia_model::rows_detail::{AreaNode, AreaPlacement};
use serde::{Deserialize, Serialize};

use crate::plan;

/// An area of the selected program's module tree, as the filter panel offers it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogArea {
    /// `v_program_module_area.area_id`: what the URL carries (`area=<id>`).
    pub id: i64,
    pub label: String,
    /// The whole path („Fachstudium / Wahlpflichtmodule Praktische Informatik").
    pub path: String,
    pub depth: i64,
    /// How many modules the tree places directly in it.
    pub modules: usize,
    /// Whether a student chooses here: a module placed directly in it is not known to be
    /// compulsory, the thesis or the internship — by what the program's sources settle on for
    /// the module (`v_program_module.kind`: the plan, the module page, the tree's own label),
    /// unknown counting as a choice (R12: not known to be fixed is not fixed). A fixed area
    /// („Komplex Mathematik" with its Pflichtmodule, the thesis) is no filter worth offering —
    /// those modules are taken anyway — and no requirement row of a plan points at it (owner,
    /// 2026-09-21).
    pub choice: bool,
    /// The labels of the nodes above it, from the top of the tree down, from the tree itself
    /// (`program_area.parent_id`), never from splitting the path: a label may itself read
    /// „Maschinenbau / Elektrotechnik".
    pub ancestors: Vec<String>,
    /// The name the pickers show (`catalog_areas`): the label without a leading
    /// „Wahlpflichtmodule"; for a label that says nothing but that („Wahlpflichtmodule (KT)"),
    /// the name of the area it lies in; with what tells two apart where two would read the same.
    pub name: String,
    /// The heading the pickers put it under (`catalog_areas`); `None` for the areas that come
    /// first, without one.
    pub section: Option<String>,
}

impl CatalogArea {
    /// An area on its own, named by its label, under no heading; `catalog_areas` names and
    /// places the areas of a whole tree.
    pub fn new(id: i64, label: &str, ancestors: &[&str], modules: usize, choice: bool) -> Self {
        let path = ancestors.iter().chain([&label]).copied().collect::<Vec<_>>().join(" / ");
        Self {
            id,
            label: label.to_string(),
            path,
            depth: ancestors.len() as i64 + 1,
            modules,
            choice,
            ancestors: ancestors.iter().map(|ancestor| ancestor.to_string()).collect(),
            name: short_name(label).to_string(),
            section: None,
        }
    }

    /// The name the pickers show.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The label of the area directly above it, if any.
    pub fn parent(&self) -> Option<&str> {
        self.ancestors.last().map(String::as_str)
    }
}

/// „Wahlpflichtmodule Praktische Mathematik" → „Praktische Mathematik", „Wahlpflichtmodule der
/// Berufsfelder" → „Berufsfelder"; a label that is nothing but such a word, or such a word and
/// a study direction („Wahlpflicht (CIV)"), stays as it is.
pub fn short_name(label: &str) -> &str {
    const JOINS: &[&str] = &["aus", "dem", "der", "des", "den", "die", "für", "im", "in", "von", "zur", "zum"];
    for prefix in ["Wahlpflichtmodule ", "Wahlpflichtmodul ", "Wahlpflichtbereich ", "Wahlpflichtfach ", "Wahlpflicht ", "Wahlbereich "] {
        if label.len() > prefix.len() && label.is_char_boundary(prefix.len()) && label[..prefix.len()].eq_ignore_ascii_case(prefix) {
            let mut rest = label[prefix.len()..].trim_start_matches(|c: char| c == '-' || c == ':' || c.is_whitespace());
            while let Some((word, after)) = rest.split_once(' ') {
                if !JOINS.contains(&word.to_lowercase().as_str()) {
                    break;
                }
                rest = after.trim_start();
            }
            if rest.chars().count() >= 3 && !plan::distinctive(plan::without_direction(rest)).is_empty() {
                return rest;
            }
        }
    }
    label
}

/// A node of the tree that only structures it and names no field of study: a phase of the
/// studies („Grundstudium", „Fachstudium", „Hauptstudium"), an account („Gesamtkonto Bachelor",
/// „Total Account - …", „Module an der …", „Modules at …"), or a label that says nothing but a
/// kind („Pflichtmodule", „Wahlpflichtmodule (KT)", „Compulsory Elective and Optional Modules").
/// Such a node is no heading of the pickers.
pub fn is_structural(label: &str) -> bool {
    const PHASES: &[&str] = &["grundstudium", "fachstudium", "hauptstudium", "basisstudium", "kernstudium", "vertiefungsstudium", "grundlagenstudium", "bachelorstudium", "masterstudium"];
    const ACCOUNTS: &[&str] = &["gesamtkonto", "total account", "module an der ", "module an den ", "modules at ", "studierende der ", "students of "];
    let folded = folia_search::fold(label.trim());
    PHASES.contains(&folded.as_str()) || ACCOUNTS.iter().any(|account| folded.starts_with(account)) || is_kind_only(label)
}

/// A label that says nothing but what kind of modules lie there („Wahlpflichtmodule (KT)").
fn is_kind_only(label: &str) -> bool {
    plan::distinctive(plan::without_direction(label)).is_empty()
}

/// A heading or a qualifier as the pickers show it: without „Komplex" in front, which only says
/// that it is one, without the account a double degree puts in front („Total Account - "), and
/// with a study direction the source printed twice („… (EM) (EM)") once.
fn display_label(label: &str) -> String {
    let mut shown = label.trim();
    for prefix in ["Komplex ", "Gesamtkonto - ", "Total Account - "] {
        if let Some(rest) = shown.strip_prefix(prefix).filter(|rest| rest.chars().count() >= 3) {
            shown = rest;
            break;
        }
    }
    if let Some((before, last)) = shown.rsplit_once(" (") {
        if before.ends_with(&format!("({last}")) {
            return before.to_string();
        }
    }
    shown.to_string()
}

/// Whether a student chooses among these modules (see `CatalogArea::choice`): any of them is
/// not known to be compulsory, the thesis or the internship.
pub fn is_choice<'a>(placements: impl IntoIterator<Item = &'a AreaPlacement>) -> bool {
    placements.into_iter().any(|placement| {
        let kind = placement.module_kind.as_ref().or(placement.kind.as_ref());
        !kind.is_some_and(|kind| kind.is(ModuleKind::Compulsory) || kind.is(ModuleKind::Thesis) || kind.is(ModuleKind::Internship))
    })
}

/// A section longer than this splits into the fields below its node, where there are such.
const LONG_SECTION: usize = 12;

/// The areas of a program in tree order, one entry per area the tree places modules in, each
/// with the nodes above it (`tree`, `queries::program_area_tree`), its name and its heading in
/// the pickers. Headings and names are decided among the areas a student chooses from
/// (`CatalogArea::choice`), the ones the pickers show. The rule comes from the 179 trees of the
/// 2026-09-21 snapshot (docs/folia/frontend.md, „The area picker"); the owner asked for few, stable
/// sections, never the same heading twice, no structural node as a heading (2026-09-21):
///
/// - An area's heading is the **highest node above it that names a field** — not a phase, an
///   account or a mere kind (`is_structural`): „Komplex Nebenfach" for Mathematik, Physik …,
///   „Komplex Berufsfeld" for its Schwerpunkte, however deep they lie. One level of headings,
///   so a section never splits into sections of its own — unless it would be longer than
///   `LONG_SECTION` and the fields below its node hold it: then those are the headings
///   („Ingenieurwissenschaftlicher Schwerpunkt" → „Produktionstechnik", „Umwelttechnik" …).
/// - An area whose label says nothing but a kind („Wahlpflichtmodule" below „Entwerfen") takes
///   the name of the field it lies in, and that field is then no heading of it — unless other
///   areas stand under that field as their heading: then it joins them under its own label.
///   An area that is itself the node of a heading („Fachspezifisches Studium" and its fields)
///   opens that section.
/// - A heading with a single area under it is none: the area joins the areas without a heading
///   (the section above it).
/// - Two headings that read the same are one section. The areas without a heading come first,
///   then the sections in the order of the tree (`area_sections`).
/// - Two areas that would read the same in one section (or one without a heading and any
///   other) are told apart by the node above them that the fewest of the others lie in, a
///   field before a structural node, the nearest first („Mathematik", „Mathematik
///   (Anwendungen)"); failing that, an area named after the field above goes by its label.
pub fn catalog_areas(placements: &[AreaPlacement], tree: &[AreaNode]) -> Vec<CatalogArea> {
    let mut areas: Vec<CatalogArea> = Vec::new();
    for placement in placements {
        let choice = is_choice([placement]);
        match areas.iter_mut().find(|area| area.id == placement.area_id) {
            Some(area) => {
                area.modules += 1;
                area.choice |= choice;
            }
            None => areas.push(CatalogArea {
                id: placement.area_id,
                label: placement.area_label.clone(),
                path: placement.area.clone(),
                depth: placement.depth,
                modules: 1,
                choice,
                ancestors: Vec::new(),
                name: short_name(&placement.area_label).to_string(),
                section: None,
            }),
        }
    }
    // The nodes above each area, from the top down (a broken tree ends the walk, never loops).
    let node = |id: i64| tree.iter().find(|node| node.id == id);
    let above = |id: i64| -> Vec<&AreaNode> {
        let mut chain: Vec<&AreaNode> = Vec::new();
        let mut next = node(id).and_then(|node| node.parent_id);
        while let Some(parent) = next.and_then(node) {
            if chain.len() > 32 || chain.iter().any(|known| known.id == parent.id) {
                break;
            }
            chain.push(parent);
            next = parent.parent_id;
        }
        chain.reverse();
        chain
    };
    let chains: Vec<Vec<&AreaNode>> = areas.iter().map(|area| above(area.id)).collect();
    for (area, chain) in areas.iter_mut().zip(&chains) {
        area.ancestors = chain.iter().map(|node| node.label.clone()).collect();
    }

    // Each area a student chooses from: the fields above it, the field that gave it its name,
    // the node of its heading.
    struct Placed<'a> {
        index: usize,
        fields: Vec<&'a AreaNode>,
        named_by: Option<i64>,
        heading: Option<i64>,
    }
    let mut placed: Vec<Placed> = Vec::new();
    for (index, (area, chain)) in areas.iter_mut().zip(&chains).enumerate() {
        if !area.choice {
            continue;
        }
        let fields: Vec<&AreaNode> = chain.iter().copied().filter(|node| !is_structural(&node.label)).collect();
        let named_by = if is_kind_only(&area.label) {
            area.name = match fields.last() {
                Some(field) => display_label(&field.label),
                None => area.label.clone(),
            };
            fields.last().map(|field| field.id)
        } else {
            None
        };
        let heading = fields.iter().find(|field| Some(field.id) != named_by).map(|field| field.id);
        placed.push(Placed { index, fields, named_by, heading });
    }
    // A long section splits into the fields below its node.
    let headings: Vec<i64> = placed.iter().filter_map(|area| area.heading).collect();
    for heading in headings.iter().copied().collect::<std::collections::BTreeSet<i64>>() {
        if headings.iter().filter(|other| **other == heading).count() <= LONG_SECTION {
            continue;
        }
        let below = |area: &Placed| -> Option<i64> {
            let at = area.fields.iter().position(|field| field.id == heading)?;
            area.fields.get(at + 1).map(|field| field.id).filter(|id| Some(*id) != area.named_by)
        };
        let members: Vec<Option<i64>> = placed.iter().filter(|area| area.heading == Some(heading)).map(below).collect();
        let fields: std::collections::BTreeSet<i64> = members.iter().flatten().copied().collect();
        let held = fields.iter().filter(|field| members.iter().filter(|member| **member == Some(**field)).count() >= 2).count();
        if held >= 2 {
            for area in placed.iter_mut().filter(|area| area.heading == Some(heading)) {
                if let Some(field) = below(area) {
                    area.heading = Some(field);
                }
            }
        }
    }
    // An area that is itself the node of a heading, or was named after one, joins that section.
    let heading_nodes: Vec<i64> = placed.iter().filter_map(|area| area.heading).collect();
    for area in placed.iter_mut() {
        let id = areas.get(area.index).map(|area| area.id);
        if area.heading.is_none() && id.is_some_and(|id| heading_nodes.contains(&id)) {
            area.heading = id;
        } else if area.named_by.is_some_and(|field| heading_nodes.contains(&field)) {
            area.heading = area.named_by;
            area.named_by = None;
            if let Some(own) = areas.get_mut(area.index) {
                own.name = own.label.clone();
            }
        }
    }
    let texts: Vec<Option<String>> = placed.iter().map(|area| area.heading.and_then(node).map(|node| display_label(&node.label))).collect();
    for (area, text) in placed.iter().zip(&texts) {
        // A heading over a single area is none.
        let alone = text.as_ref().is_some_and(|text| texts.iter().filter(|other| other.as_ref() == Some(text)).count() < 2);
        if let Some(own) = areas.get_mut(area.index) {
            own.section = if alone { None } else { text.clone() };
        }
    }

    // Two areas that read the same where they stand.
    let key = |area: &CatalogArea| folia_search::fold(&area.name);
    let mut renamed: Vec<(usize, String)> = Vec::new();
    for area in &placed {
        let (Some(own), Some(chain)) = (areas.get(area.index), chains.get(area.index)) else { continue };
        let alike: Vec<usize> = placed
            .iter()
            .map(|other| other.index)
            .filter(|other| *other != area.index)
            .filter(|other| areas.get(*other).is_some_and(|theirs| key(theirs) == key(own) && (own.section.is_none() || theirs.section == own.section)))
            .collect();
        if alike.is_empty() {
            continue;
        }
        let sharing = |label: &str| alike.iter().filter(|other| chains.get(**other).is_some_and(|theirs| theirs.iter().any(|node| node.label == label))).count();
        let heading = own.section.clone();
        // The node above that the fewest of the others lie in as well; a field before a
        // structural node, the nearest first.
        let distinct = chain
            .iter()
            .rev()
            .filter(|node| Some(node.id) != area.named_by && heading.as_deref() != Some(display_label(&node.label).as_str()))
            .map(|node| (sharing(&node.label), is_structural(&node.label), *node))
            .filter(|(shared, ..)| *shared < alike.len())
            .enumerate()
            .min_by_key(|(nearness, (shared, structural, _))| (*shared, *structural, *nearness))
            .map(|(_, (_, _, node))| node);
        match distinct {
            Some(distinct) => renamed.push((area.index, format!("{} ({})", own.name, display_label(&distinct.label)))),
            // Named after the field above, and nothing above tells them apart: their own labels do.
            None if area.named_by.is_some() => renamed.push((area.index, own.label.clone())),
            None => {}
        }
    }
    for (index, name) in renamed {
        if let Some(area) = areas.get_mut(index) {
            area.name = name;
        }
    }
    areas
}

/// The areas the pickers offer, as they show them: those without a heading first, then one
/// section per heading in the order of the tree, each area in the order of the tree.
pub fn area_sections(areas: &[CatalogArea]) -> Vec<(Option<String>, Vec<CatalogArea>)> {
    let mut sections: Vec<(Option<String>, Vec<CatalogArea>)> = vec![(None, Vec::new())];
    for area in areas.iter().filter(|area| area.choice) {
        match sections.iter_mut().find(|(heading, _)| *heading == area.section) {
            Some((_, members)) => members.push(area.clone()),
            None => sections.push((area.section.clone(), vec![area.clone()])),
        }
    }
    sections.retain(|(_, members)| !members.is_empty());
    sections
}

#[cfg(test)]
mod tests {
    use crate::area_fixtures as real;
    use super::*;

    /// The picker as it reads: (heading, names).
    fn picker(nodes: &[real::Node]) -> Vec<(Option<String>, Vec<String>)> {
        area_sections(&real::areas(nodes)).into_iter().map(|(heading, areas)| (heading, areas.iter().map(|area| area.name().to_string()).collect())).collect()
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn the_picker_of_informatik_offers_its_own_electives_and_the_nebenfach() {
        // Owner, 2026-09-21: the own electives and the Anwendungs-/Nebenfach, no fixed complex,
        // no heading twice, no „Grundstudium"/„Fachstudium". Praktische Mathematik lies in the
        // Komplex Nebenfach of the tree, so it stands there (docs/folia/frontend.md says why).
        assert_eq!(
            picker(real::INFORMATIK_BSC),
            vec![
                (None, strings(&["Proseminar oder Praktikum", "Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Seminar oder Praktikum aus der Informatik"])),
                (Some("Nebenfach".to_string()), strings(&["Praktische Mathematik", "Mathematik", "Physik", "Maschinenbau / Elektrotechnik", "Wirtschaftswissenschaften", "Bauingenieurwesen"])),
            ]
        );
        let areas = real::areas(real::INFORMATIK_BSC);
        let fixed: Vec<&str> = areas.iter().filter(|area| !area.choice).map(|area| area.label.as_str()).collect();
        assert_eq!(fixed, vec!["Komplex Informatik", "Komplex Mathematik", "Fachstudium"]);
        // The parent comes from the tree, never from the path: a label may hold the separator.
        let slashed = areas.iter().find(|area| area.label == "Maschinenbau / Elektrotechnik").map(|area| (area.parent(), area.ancestors.clone()));
        assert_eq!(slashed, Some((Some("Komplex Nebenfach"), strings(&["Grundstudium", "Komplex Nebenfach"]))));

        // The Master: two fields, and the two areas called Mathematik told apart.
        assert_eq!(
            picker(real::INFORMATIK_MSC),
            vec![
                (Some("Informatik-Vertiefung".to_string()), strings(&["Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Seminare oder Praktika"])),
                (Some("Nebenfach".to_string()), strings(&["Mathematik", "Mathematik (Anwendungen)", "Physik", "Maschinenbau / Elektrotechnik", "Wirtschaftsingenieurwesen", "Bauingenieurwesen"])),
            ]
        );
    }

    #[test]
    fn an_area_named_only_by_its_kind_takes_the_name_of_its_field() {
        // Architektur: every field has its „Wahlpflichtmodule"; the account on top is no heading.
        assert_eq!(picker(real::ARCHITEKTUR_BSC), vec![(None, strings(&["Entwerfen", "Bautechnik und Ökologie", "Geschichte und Theorie"]))]);
        // Elektrotechnik B.Sc.: „Wahlpflichtmodul (MIT)" is the list of „Informatik (MIT)".
        assert_eq!(
            picker(real::ELEKTROTECHNIK_BSC),
            vec![(
                None,
                strings(&[
                    "Informatik (MIT)",
                    "Informatik (EET)",
                    "Hauptstudium",
                    "Studienrichtungsspezifische Vertiefungsmodule (EET)",
                    "Studienrichtungsspezifische Vertiefungsmodule (MIT)",
                    "Studienrichtungsspezifische Vertiefungsmodule (PAu)",
                    "Studienrichtungsspezifische Vertiefungsmodule (IoT)",
                ])
            )]
        );
        // Elektrotechnik M.Sc.: the field holds another area as well, so it is the heading, and
        // the list keeps its own label.
        assert_eq!(
            picker(real::ELEKTROTECHNIK_MSC),
            vec![
                (Some("Studienrichtung Kommunikationstechnik (KT)".to_string()), strings(&["Wahlpflichtmodule (KT)", "Zweite Fremdsprache"])),
                (Some("Studienrichtung Prozessautomatisierung (PAu)".to_string()), strings(&["Wahlpflichtmodule (PAu)", "Zweite Fremdsprache"])),
                (Some("Studienrichtung Energiesysteme (ES)".to_string()), strings(&["Wahlpflichtmodule (ES)", "Zweite Fremdsprache"])),
            ]
        );
    }

    #[test]
    fn a_long_section_splits_into_the_fields_below_it() {
        let sections = picker(real::WIRTSCHAFTSINGENIEURWESEN_DUAL);
        let headings: Vec<Option<&str>> = sections.iter().map(|(heading, _)| heading.as_deref()).collect();
        assert_eq!(
            headings,
            vec![None, Some("Wirtschaftswissenschaftlicher Bereich"), Some("Produktionstechnik"), Some("Umwelttechnik"), Some("Energiesysteme"), Some("Bauingenieurwesen"), Some("Elektro- und Informationstechnik")]
        );
        assert_eq!(sections[0].1, strings(&["Mathematisch-Methodischer Bereich", "Praxisintegrierendes Studium"]));
        assert_eq!(sections[3].1, strings(&["Pflichtbereich Umwelttechnik", "Umwelttechnik", "Kreislauf und Entsorgung", "Wassertechnik"]));
    }

    #[test]
    fn no_heading_twice_none_over_one_area_none_that_only_structures() {
        for tree in [real::INFORMATIK_BSC, real::INFORMATIK_MSC, real::ELEKTROTECHNIK_BSC, real::ELEKTROTECHNIK_MSC, real::ARCHITEKTUR_BSC, real::WIRTSCHAFTSINGENIEURWESEN_DUAL] {
            let sections = picker(tree);
            let headings: Vec<&String> = sections.iter().filter_map(|(heading, _)| heading.as_ref()).collect();
            for (i, heading) in headings.iter().enumerate() {
                assert!(!headings.iter().skip(i + 1).any(|other| other == heading), "{heading} twice");
                assert!(!is_structural(heading), "{heading}");
            }
            assert!(sections.iter().all(|(heading, names)| heading.is_none() || names.len() >= 2), "{sections:?}");
            assert!(sections.iter().skip(1).all(|(heading, _)| heading.is_some()), "the areas without a heading come first: {sections:?}");
        }
        for label in ["Grundstudium", "Fachstudium", "Hauptstudium", "Gesamtkonto Bachelor", "Total Account - Home BTU", "Modules at the Deakin University", "Pflichtmodule", "Wahlpflichtmodule (KT)", "Compulsory Elective and Optional Modules", "Mandatory Modules"] {
            assert!(is_structural(label), "{label}");
        }
        for label in ["Komplex Nebenfach", "Studienrichtung Kommunikationstechnik (KT)", "Schwerpunkte", "Anwendungen", "Informatik-Vertiefung"] {
            assert!(!is_structural(label), "{label}");
        }
    }

    #[test]
    fn the_name_of_an_area_drops_the_word_that_says_it_is_a_choice() {
        assert_eq!(short_name("Wahlpflichtmodule Praktische Mathematik"), "Praktische Mathematik");
        assert_eq!(short_name("Wahlpflichtmodul aus dem Nebenfach"), "Nebenfach");
        assert_eq!(short_name("Wahlpflichtmodule der Berufsfelder"), "Berufsfelder");
        assert_eq!(short_name("Wahlbereich Grundlage der Statistik"), "Grundlage der Statistik");
        assert_eq!(short_name("Wahlpflichtmodule"), "Wahlpflichtmodule");
        assert_eq!(short_name("Wahlpflicht (CIV)"), "Wahlpflicht (CIV)");
        assert_eq!(short_name("Wahlpflichtbereich II (SUR)"), "Wahlpflichtbereich II (SUR)");
        assert_eq!(short_name("Praktische Informatik"), "Praktische Informatik");
        assert_eq!(display_label("Komplex Nebenfach"), "Nebenfach");
        assert_eq!(display_label("Studienrichtung Elektrische Medizintechnik (EM) (EM)"), "Studienrichtung Elektrische Medizintechnik (EM)");
        assert_eq!(display_label("Total Account - Home BTU"), "Home BTU");
    }
}

