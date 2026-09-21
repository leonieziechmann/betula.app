//! What the area pickers and the plan's requirement rows make of a real snapshot: for every
//! program with a module tree, the picker as the app shows it (sections, names, counts, and why
//! an area counts as a choice or as fixed), and for every row of a validated plan that names
//! no module, the areas `plan::areas_for_row` points it at. It calls the crate's own functions,
//! so it checks exactly what the app does.
//!
//! ```text
//! cargo run -p folia-catalog --features native --example area_survey -- <catalog-*.db> [slug…]
//! ```
//!
//! Without slugs it surveys every program and ends with the numbers docs/frontend.md quotes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use catalog::labels::ModuleKind;
use catalog::native::NativeDatabase;
use catalog::pages::{self, CatalogArea};
use catalog::plan;
use catalog::queries;
use catalog::rows_detail::AreaPlacement;

fn kinds(placements: &[AreaPlacement], area: i64) -> String {
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    for placement in placements.iter().filter(|placement| placement.area_id == area) {
        let kind = placement.module_kind.as_ref().or(placement.kind.as_ref()).map(|kind| kind.label().to_string()).unwrap_or_else(|| "?".to_string());
        *counted.entry(kind).or_default() += 1;
    }
    counted.iter().map(|(kind, n)| format!("{kind} {n}")).collect::<Vec<_>>().join(", ")
}

fn known_elective(placements: &[AreaPlacement], area: i64) -> bool {
    placements
        .iter()
        .filter(|placement| placement.area_id == area)
        .any(|placement| placement.module_kind.as_ref().or(placement.kind.as_ref()).is_some_and(|kind| kind.is(ModuleKind::Elective) || kind.is(ModuleKind::Fues)))
}

fn unknown_only(placements: &[AreaPlacement], area: i64) -> bool {
    placements.iter().filter(|placement| placement.area_id == area).all(|placement| placement.module_kind.is_none() && placement.kind.is_none())
}

#[derive(Default)]
struct Numbers {
    programs: usize,
    no_kind_at_all: Vec<String>,
    choice_areas: usize,
    fixed_areas: usize,
    choice_with_elective: usize,
    choice_unknown_only: usize,
    choice_unknown_beside_fixed: Vec<String>,
    sections: BTreeMap<usize, usize>,
    headings: BTreeMap<String, usize>,
    repeated_headings: Vec<String>,
    lone_sections: Vec<String>,
    structural_headings: Vec<String>,
    same_names: Vec<String>,
    old_repeated_headings: Vec<String>,
    old_lone_groups: usize,
    old_structural_headings: usize,
    rows: usize,
    rows_single: usize,
    rows_fues: usize,
    rows_none: Vec<String>,
    rows_one: usize,
    rows_ambiguous: Vec<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: area_survey <catalog-*.db> [slug…]");
        std::process::exit(2);
    };
    let only: Vec<String> = args.collect();
    let db = match NativeDatabase::open(&PathBuf::from(path)) {
        Ok(db) => db,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let programs = match queries::programs(&db) {
        Ok(programs) => programs,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let mut n = Numbers::default();
    for program in programs.iter().filter(|program| only.is_empty() || only.contains(&program.slug)) {
        let (Ok(placements), Ok(tree), Ok(entries)) = (queries::program_areas(&db, &program.id), queries::program_area_tree(&db, &program.id), queries::program_plan_entries(&db, &program.id)) else {
            eprintln!("{}: query failed", program.slug);
            continue;
        };
        if placements.is_empty() {
            continue;
        }
        let tag = format!("{} ({})", program.name, program.slug);
        n.programs += 1;
        let areas = pages::catalog_areas(&placements, &tree);
        println!("\n### {tag}");
        if placements.iter().all(|placement| placement.module_kind.is_none() && placement.kind.is_none()) {
            n.no_kind_at_all.push(tag.clone());
            println!("  (no module of the tree has a kind: every area counts as a choice)");
        }

        // The picker as the app shows it.
        let sections = pages::area_sections(&areas);
        *n.sections.entry(sections.iter().filter(|(heading, _)| heading.is_some()).count()).or_default() += 1;
        for (heading, members) in &sections {
            println!("  [{}]", heading.as_deref().unwrap_or("—"));
            if let Some(heading) = heading {
                *n.headings.entry(heading.clone()).or_default() += 1;
                if members.len() < 2 {
                    n.lone_sections.push(format!("{tag}: {heading}"));
                }
                if pages::is_structural(heading) {
                    n.structural_headings.push(format!("{tag}: {heading}"));
                }
            }
            for area in members {
                let why = if known_elective(&placements, area.id) { "" } else if unknown_only(&placements, area.id) { "  ← only unknown" } else { "  ← unknown beside fixed" };
                println!("    {} · {} Module   [{}]{why}   <{}>", area.name(), area.modules, kinds(&placements, area.id), area.path);
            }
        }
        let headings: Vec<&String> = sections.iter().filter_map(|(heading, _)| heading.as_ref()).collect();
        for (i, heading) in headings.iter().enumerate() {
            if headings.iter().skip(i + 1).any(|other| other == heading) {
                n.repeated_headings.push(format!("{tag}: {heading}"));
            }
        }
        let shown: Vec<&CatalogArea> = areas.iter().filter(|area| area.choice).collect();
        for (i, area) in shown.iter().enumerate() {
            // Where the picker shows them: in one section, or one of them without a heading.
            let together = |other: &&&CatalogArea| other.section == area.section || other.section.is_none() || area.section.is_none();
            if shown.iter().skip(i + 1).filter(together).any(|other| other.name() == area.name()) {
                n.same_names.push(format!("{tag}: {}", area.name()));
            }
        }
        let fixed: Vec<&CatalogArea> = areas.iter().filter(|area| !area.choice).collect();
        if !fixed.is_empty() {
            println!("  hidden as fixed: {}", fixed.iter().map(|area| format!("{} [{}]", area.label, kinds(&placements, area.id))).collect::<Vec<_>>().join("; "));
        }
        for area in &shown {
            n.choice_areas += 1;
            if known_elective(&placements, area.id) {
                n.choice_with_elective += 1;
            } else if unknown_only(&placements, area.id) {
                n.choice_unknown_only += 1;
            } else {
                n.choice_unknown_beside_fixed.push(format!("{tag}: {} [{}]", area.label, kinds(&placements, area.id)));
            }
        }
        n.fixed_areas += fixed.len();

        // The picker as it was before (a run of areas under the label of the node above).
        let mut old: Vec<(Option<String>, usize)> = Vec::new();
        for area in &shown {
            let parent = area.parent().map(str::to_string);
            match old.last_mut() {
                Some((group, count)) if *group == parent => *count += 1,
                _ => old.push((parent, 1)),
            }
        }
        for (i, (group, count)) in old.iter().enumerate() {
            if let Some(group) = group {
                if old.iter().skip(i + 1).any(|(other, _)| other.as_ref() == Some(group)) {
                    n.old_repeated_headings.push(format!("{tag}: {group}"));
                }
                if *count < 2 {
                    n.old_lone_groups += 1;
                }
                if pages::is_structural(group) {
                    n.old_structural_headings += 1;
                }
            }
        }

        // The rows of the plan that name no module.
        let rows: Vec<_> = entries.iter().filter(|entry| entry.module_id.is_none() && !entry.module_name.trim().is_empty()).collect();
        if !rows.is_empty() {
            println!("  plan rows:");
        }
        let mut seen: Vec<(String, String)> = Vec::new();
        for entry in rows {
            let caption = entry.specialization.clone().unwrap_or_default();
            if seen.contains(&(entry.module_name.clone(), caption.clone())) {
                continue;
            }
            seen.push((entry.module_name.clone(), caption.clone()));
            n.rows += 1;
            let kind = entry.kind.as_ref().map(|kind| kind.label().to_string()).unwrap_or_else(|| "?".to_string());
            let head = format!("    „{}\" [{kind}]{}", entry.module_name, if caption.is_empty() { String::new() } else { format!(" ‹{caption}›") });
            if plan::is_single_module(entry) {
                n.rows_single += 1;
                println!("{head} → one module");
                continue;
            }
            if plan::is_fues(entry) {
                n.rows_fues += 1;
                println!("{head} → FÜS");
                continue;
            }
            let plan_rows: Vec<_> = entries.iter().filter(|other| other.specialization == entry.specialization).cloned().collect();
            let found = plan::areas_for_row(entry, &caption, &areas, &plan_rows);
            let names = |list: &[CatalogArea]| list.iter().map(|area| area.path.clone()).collect::<Vec<_>>().join(" | ");
            match found.areas.len() {
                0 => n.rows_none.push(format!("{tag}: {}", entry.module_name)),
                1 => n.rows_one += 1,
                _ => n.rows_ambiguous.push(format!("{tag}: {} → {}", entry.module_name, found.areas.len())),
            }
            println!("{head} → {}{}", if found.areas.is_empty() { "—".to_string() } else { names(&found.areas) }, if found.others.is_empty() { String::new() } else { format!("   (also: {})", names(&found.others)) });
        }
    }

    println!("\n==== numbers");
    println!("programs with a tree: {}", n.programs);
    println!("  without any kind in the tree (every area a choice): {}", n.no_kind_at_all.len());
    for tag in &n.no_kind_at_all {
        println!("    {tag}");
    }
    println!("areas offered: {} (known elective module: {}, only unknown kinds: {}, unknown beside fixed only: {}); hidden as fixed: {}", n.choice_areas, n.choice_with_elective, n.choice_unknown_only, n.choice_unknown_beside_fixed.len(), n.fixed_areas);
    for line in &n.choice_unknown_beside_fixed {
        println!("    {line}");
    }
    println!("sections per picker (headings → programs): {:?}", n.sections);
    println!("headings, most frequent: {:?}", {
        let mut list: Vec<(&String, &usize)> = n.headings.iter().collect();
        list.sort_by(|a, b| b.1.cmp(a.1));
        list.into_iter().take(25).collect::<Vec<_>>()
    });
    println!("repeated headings: {} {:?}", n.repeated_headings.len(), n.repeated_headings);
    println!("sections of one area: {} {:?}", n.lone_sections.len(), n.lone_sections);
    println!("structural headings: {} {:?}", n.structural_headings.len(), n.structural_headings);
    println!("areas that read the same where they stand: {} {:?}", n.same_names.len(), n.same_names);
    println!("before (heading = node above, runs): repeated headings {}, groups of one {}, structural headings {}", n.old_repeated_headings.len(), n.old_lone_groups, n.old_structural_headings);
    for line in &n.old_repeated_headings {
        println!("    {line}");
    }
    println!("plan rows naming no module (per plan caption): {} — one module {}, FÜS {}, to an area: one {}, several {}, none {}", n.rows, n.rows_single, n.rows_fues, n.rows_one, n.rows_ambiguous.len(), n.rows_none.len());
    for line in &n.rows_ambiguous {
        println!("    several: {line}");
    }
    for line in &n.rows_none {
        println!("    none: {line}");
    }
}
