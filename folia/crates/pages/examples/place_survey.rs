//! What the Regelstudienpläne leave a student to choose, on a real snapshot: for every current
//! program with a study plan and every plan of it (one per study direction), each row that names
//! no module of the catalog — a place the student fills („Anwendungsfach", „FÜS", „Komplex
//! Praktische Informatik") — with the modules „Modul wählen" leads to (`variants::row_query`, the
//! catalog query of the row), how many of them there are and how many a winter and a summer offer
//! (`study::Offer`). It calls the crates' own functions, so it counts what the app shows. The
//! numbers of folia/design/hinzufuegen/konzepte.html come from it.
//!
//! ```text
//! cargo run -p folia-pages --example place_survey -- <catalog-*.db> [2026W] > places.tsv
//! ```
//!
//! One line per row of a plan, tab-separated with a header; the summary on stderr.

use std::collections::BTreeMap;
use std::path::PathBuf;

use folia_calendar::semester::SemesterKey;
use folia_locale::Locale;
use folia_model::labels::{Labelled, ModuleKind};
use folia_model::native::NativeDatabase;
use folia_pages as pages;
use folia_plans::plan;
use folia_plans::study::Offer;
use folia_plans::variants;
use folia_query as queries;
use folia_routes::url;

const COLUMNS: &[&str] = &["program", "degree", "plan", "ord", "row", "kind", "what", "credits", "fs", "candidates", "offered_winter", "offered_summer", "query"];

/// What a row of a plan is to the student.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum What {
    /// It names a module of the catalog: taken over as it is.
    Module,
    /// A module the plan names without a link to the catalog (a compulsory module, the thesis,
    /// an internship): one to find, not to choose.
    Named,
    /// A place of the Fachübergreifendes Studium: the program's FÜS list.
    Fues,
    /// A place to fill from a list of modules (an area of the module tree, the program's
    /// electives).
    Choice,
}

impl What {
    fn word(self) -> &'static str {
        match self {
            What::Module => "module",
            What::Named => "named",
            What::Fues => "fues",
            What::Choice => "choice",
        }
    }
}

/// The share of `part` in `all`, in whole per cent.
fn share(part: f64, all: f64) -> String {
    if all <= 0.0 { "–".to_string() } else { format!("{:.0} %", 100.0 * part / all) }
}

/// The value at quantile `q` of sorted `values`.
fn quantile(values: &[u64], q: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let at = ((values.len() - 1) as f64 * q).round() as usize;
    values.get(at).copied().unwrap_or_default()
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: place_survey <catalog-*.db> [winter semester, e.g. 2026W]");
        return std::process::ExitCode::FAILURE;
    };
    let winter = args.get(1).and_then(|key| SemesterKey::parse(key)).or_else(|| SemesterKey::parse("2026W"));
    let (Some(winter), Ok(db)) = (winter, NativeDatabase::open(&PathBuf::from(path))) else {
        eprintln!("cannot open {path} or read the semester");
        return std::process::ExitCode::FAILURE;
    };
    let Some(summer) = winter.plus(1) else { return std::process::ExitCode::FAILURE };

    println!("{}", COLUMNS.join("\t"));
    // Credits by what a row is, per plan; candidates per place.
    let mut plans = 0usize;
    let mut programs_seen = 0usize;
    let mut credits_by: BTreeMap<What, f64> = BTreeMap::new();
    let mut rows_by: BTreeMap<What, usize> = BTreeMap::new();
    let mut plan_shares: Vec<u64> = Vec::new();
    let mut places_per_plan: Vec<u64> = Vec::new();
    let mut choice_candidates: Vec<u64> = Vec::new();
    let mut choice_winter: Vec<u64> = Vec::new();
    let mut fues_candidates: Vec<u64> = Vec::new();
    let mut by_degree: BTreeMap<String, (usize, f64, f64)> = BTreeMap::new();
    let programs = queries::programs(&db).unwrap_or_default();
    for program in programs.iter().filter(|program| program.is_latest_po && program.has_plan) {
        let Ok(Some(source)) = pages::plan_source(&db, &program.id, Locale::De) else { continue };
        programs_seen += 1;
        // A page that fills a row of a larger plan is counted with that plan only.
        let pages_of: Vec<usize> = source.supplements.iter().map(|supplement| supplement.page).collect();
        for (at, variant) in source.variants.iter().enumerate() {
            if pages_of.contains(&at) {
                continue;
            }
            plans += 1;
            let mut plan_credits: BTreeMap<What, f64> = BTreeMap::new();
            let mut places = 0u64;
            for entry in &variant.entries {
                let credits = entry.credits.or(entry.min_credits).unwrap_or(0.0);
                let linked = entry.module_id.as_deref().is_some_and(url::is_module_id);
                let what = if linked {
                    What::Module
                } else if plan::is_fues(entry) {
                    What::Fues
                } else if plan::is_single_module(entry) {
                    What::Named
                } else {
                    What::Choice
                };
                *plan_credits.entry(what).or_default() += credits;
                *rows_by.entry(what).or_default() += 1;
                let (mut candidates, mut offered_winter, mut offered_summer, mut query_text) = (String::new(), String::new(), String::new(), String::new());
                if matches!(what, What::Fues | What::Choice) {
                    places += 1;
                    let (query, label) = variants::row_query(&program.slug, variant, entry, &source.areas, Locale::De);
                    let count = queries::catalog_count(&db, &query).unwrap_or(0);
                    let rows = queries::catalog_page(&db, &query, 0, 5000).map(|page| page.rows).unwrap_or_default();
                    let in_winter = rows.iter().filter(|row| Offer::of(row).offered(winter)).count() as u64;
                    let in_summer = rows.iter().filter(|row| Offer::of(row).offered(summer)).count() as u64;
                    match what {
                        What::Fues => fues_candidates.push(count),
                        _ => {
                            choice_candidates.push(count);
                            choice_winter.push(in_winter);
                        }
                    }
                    candidates = count.to_string();
                    offered_winter = in_winter.to_string();
                    offered_summer = in_summer.to_string();
                    query_text = label;
                }
                let fs = match (entry.start_semester.or(entry.semester), entry.end_semester.or(entry.semester)) {
                    (Some(from), Some(to)) if from != to => format!("{from}-{to}"),
                    (Some(from), _) => from.to_string(),
                    _ => String::new(),
                };
                let kind = entry.kind.as_ref().and_then(|kind| kind.known()).map(|kind: ModuleKind| kind.label(Locale::De).to_string()).unwrap_or_default();
                let cells = [
                    program.slug.clone(),
                    program.degree().to_string(),
                    variant.label.clone(),
                    entry.ord.to_string(),
                    plan::shown_name(&entry.module_name).replace(['\t', '\n'], " "),
                    kind,
                    what.word().to_string(),
                    credits.to_string(),
                    fs,
                    candidates,
                    offered_winter,
                    offered_summer,
                    query_text,
                ];
                println!("{}", cells.join("\t"));
            }
            let total: f64 = plan_credits.values().sum();
            let open = plan_credits.get(&What::Fues).copied().unwrap_or(0.0) + plan_credits.get(&What::Choice).copied().unwrap_or(0.0);
            if total > 0.0 {
                plan_shares.push((100.0 * open / total).round() as u64);
            }
            places_per_plan.push(places);
            for (what, credits) in &plan_credits {
                *credits_by.entry(*what).or_default() += credits;
            }
            let degree = by_degree.entry(program.degree().to_string()).or_default();
            degree.0 += 1;
            degree.1 += open;
            degree.2 += total;
        }
    }
    for list in [&mut plan_shares, &mut places_per_plan, &mut choice_candidates, &mut choice_winter, &mut fues_candidates] {
        list.sort_unstable();
    }
    let all: f64 = credits_by.values().sum();
    eprintln!("{programs_seen} programs, {plans} plans");
    for (what, credits) in &credits_by {
        eprintln!("  {:<7} {:>5} rows  {:>7.0} LP  {}", what.word(), rows_by.get(what).copied().unwrap_or(0), credits, share(*credits, all));
    }
    eprintln!(
        "credits a plan leaves to choose (FÜS and places): median {} %, quartiles {}–{} %; plans with none: {}",
        quantile(&plan_shares, 0.5),
        quantile(&plan_shares, 0.25),
        quantile(&plan_shares, 0.75),
        plan_shares.iter().filter(|share| **share == 0).count()
    );
    eprintln!("places per plan: median {}, quartiles {}–{}, most {}", quantile(&places_per_plan, 0.5), quantile(&places_per_plan, 0.25), quantile(&places_per_plan, 0.75), places_per_plan.last().copied().unwrap_or(0));
    let buckets = [(0, 0), (1, 1), (2, 10), (11, 30), (31, 100), (101, 300), (301, u64::MAX)];
    let bucket_line = |values: &[u64]| {
        buckets
            .iter()
            .map(|(from, to)| {
                let n = values.iter().filter(|v| **v >= *from && **v <= *to).count();
                let name = match (from, to) {
                    (a, b) if a == b => a.to_string(),
                    (a, &u64::MAX) => format!("{a}+"),
                    (a, b) => format!("{a}–{b}"),
                };
                format!("{name}: {n} ({})", share(n as f64, values.len() as f64))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    eprintln!("choice places: {}; candidates median {}, quartiles {}–{}, p90 {}", choice_candidates.len(), quantile(&choice_candidates, 0.5), quantile(&choice_candidates, 0.25), quantile(&choice_candidates, 0.75), quantile(&choice_candidates, 0.9));
    eprintln!("  by count: {}", bucket_line(&choice_candidates));
    eprintln!("  offered in {}: median {}, quartiles {}–{}", winter.key(), quantile(&choice_winter, 0.5), quantile(&choice_winter, 0.25), quantile(&choice_winter, 0.75));
    eprintln!("FÜS places: {}; candidates median {}, quartiles {}–{}", fues_candidates.len(), quantile(&fues_candidates, 0.5), quantile(&fues_candidates, 0.25), quantile(&fues_candidates, 0.75));
    eprintln!("  by count: {}", bucket_line(&fues_candidates));
    for (degree, (n, open, total)) in &by_degree {
        eprintln!("  {degree:<12} {n:>3} plans, to choose {}", share(*open, *total));
    }
    std::process::ExitCode::SUCCESS
}
