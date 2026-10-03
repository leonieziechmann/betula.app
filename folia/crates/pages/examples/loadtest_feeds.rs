//! Calendar-feed addresses for a load test: the Studienpläne people would really subscribe to.
//! For every current program with a study plan and every Fachsemester of it, the plan's modules of
//! that semester (what „Regelstudienplan übernehmen" plans), as `/calendar/<code>.ics` for one
//! semester; then `--heavy` codes of `MAX_MODULES` modules with events, the dearest feed there is.
//!
//! ```text
//! cargo run --release -p folia-catalog --features native --example loadtest_feeds -- <catalog-*.db> [2026W] [--heavy N]
//! ```
//!
//! One line per feed: path, program slug, Fachsemester (0 = heavy), number of modules.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use folia_calendar::select::MAX_MODULES;
use folia_calendar::semester::SemesterKey;
use folia_calendar::subscription::{self, Subscription};
use folia_model::native::NativeDatabase;
use folia_query as queries;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: loadtest_feeds <catalog-*.db> [semester] [--heavy N]");
        return std::process::ExitCode::FAILURE;
    };
    let semester = args.get(1).filter(|arg| !arg.starts_with("--")).and_then(|key| SemesterKey::parse(key)).or_else(|| SemesterKey::parse("2026W"));
    let heavy = args.iter().position(|arg| arg == "--heavy").and_then(|at| args.get(at + 1)).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let (Some(semester), Ok(db)) = (semester, NativeDatabase::open(&PathBuf::from(path))) else {
        eprintln!("cannot open {path} or read the semester");
        return std::process::ExitCode::FAILURE;
    };

    let mut written = 0usize;
    let mut emit = |subscription: Subscription, slug: &str, fachsemester: i64| match subscription.code() {
        Ok(code) => {
            println!("{}\t{slug}\t{fachsemester}\t{}", subscription::path(&code), subscription.modules.len());
            written += 1;
        }
        Err(error) => eprintln!("{slug} {fachsemester}: {error:?}"),
    };

    let programs = queries::programs(&db).unwrap_or_default();
    for program in programs.iter().filter(|program| program.is_latest_po && program.has_plan) {
        let mut by_semester: BTreeMap<i64, BTreeSet<u32>> = BTreeMap::new();
        for entry in queries::program_plan_entries(&db, &program.id).unwrap_or_default() {
            let (Some(id), Some(fachsemester)) = (entry.module_id.as_deref().and_then(|id| id.parse::<u32>().ok()), entry.semester.or(entry.start_semester)) else { continue };
            by_semester.entry(fachsemester).or_default().insert(id);
        }
        for (fachsemester, modules) in by_semester {
            let modules: Vec<u32> = modules.into_iter().take(MAX_MODULES).collect();
            emit(Subscription { semester: semester.index(), modules, program: Some(program.id.clone()), ..Subscription::default() }, &program.slug, fachsemester);
        }
    }

    if heavy > 0 {
        let with_events: Vec<u32> = queries::semester_schedule(&db, &semester.key())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|row| row.module_id.parse::<u32>().ok())
            .collect::<BTreeSet<u32>>()
            .into_iter()
            .collect();
        // Evenly spread windows over the modules with events, so the heavy feeds differ.
        let step = (with_events.len() / heavy.max(1)).max(1);
        for n in 0..heavy {
            let start = (n * step) % with_events.len().max(1);
            let modules: Vec<u32> = with_events.iter().cycle().skip(start).take(MAX_MODULES.min(with_events.len())).copied().collect();
            emit(Subscription { semester: semester.index(), modules, ..Subscription::default() }, "heavy", 0);
        }
    }
    eprintln!("{written} feeds for {}", semester.key());
    std::process::ExitCode::SUCCESS
}
