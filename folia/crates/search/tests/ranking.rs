//! The order of the catalog's search on the snapshot of the tests (folia/design/suche/konzept.html,
//! „Wo das Gemeinte landet"): what a student most likely means by a query stands first, or among
//! the first. A case whose module the snapshot does not offer is left out, so the checks hold on
//! any snapshot.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use folia_model::db::{Database, Value};
use folia_search::Plan;
use folia_test_support::open;

/// The offered modules `text` finds, in the order of the list: (id, title).
fn found(db: &dyn Database, text: &str, program: Option<&str>) -> Vec<(String, String)> {
    let resolution = folia_search::resolve(db, text).unwrap();
    let plan = Plan::new(db, text, Some(&resolution), program).unwrap().unwrap();
    let Some((table, params)) = plan.table() else { return Vec::new() };
    let sql = format!(
        "SELECT m.id, m.title FROM {table} sr JOIN v_module m ON m.id = sr.module_id \
         WHERE {} AND m.offer_status != 'not_offered' ORDER BY sr.rank",
        plan.condition("sr")
    );
    let text = |value: Option<&Value>| match value {
        Some(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    db.query("test_found", &sql, &params).unwrap().rows.iter().map(|row| (text(row.first()), text(row.get(1)))).collect()
}

/// Whether the snapshot offers a module whose title `meant` takes.
fn offers(db: &dyn Database, meant: &dyn Fn(&str) -> bool) -> bool {
    let rows = db.query("test_offers", "SELECT title FROM v_module WHERE offer_status != 'not_offered'", &[]).unwrap();
    rows.rows.iter().any(|row| matches!(row.first(), Some(Value::Text(title)) if meant(title)))
}

/// The place (1-based) of the first module `text` finds whose title `meant` takes, where the
/// snapshot offers one; `Some(None)`: offered, but not found.
fn place(db: &dyn Database, text: &str, meant: &dyn Fn(&str) -> bool) -> Option<Option<usize>> {
    if !offers(db, meant) {
        eprintln!("ranking: the snapshot offers nothing „{text}“ means; not checked");
        return None;
    }
    Some(found(db, text, None).iter().position(|(_, title)| meant(title)).map(|i| i + 1))
}

#[test]
fn what_is_meant_stands_first() {
    let db = open();
    let is = |title: &'static str| move |t: &str| t == title;
    let starts = |start: &'static str| move |t: &str| t.starts_with(start);
    let has = |part: &'static str| move |t: &str| t.contains(part);
    // query, how its module is recognised, the place it must at least have
    type Meant = Box<dyn Fn(&str) -> bool>;
    let cases: Vec<(&str, Meant, usize)> = vec![
        ("TI", Box::new(is("Theoretische Informatik")), 1),
        ("ti", Box::new(is("Theoretische Informatik")), 1),
        ("theo inf", Box::new(is("Theoretische Informatik")), 1),
        ("TheoInf", Box::new(is("Theoretische Informatik")), 1),
        ("theoinf", Box::new(is("Theoretische Informatik")), 1),
        ("KI", Box::new(has("Künstliche Intelligenz")), 1),
        ("LinA", Box::new(starts("Lineare Algebra")), 1),
        ("mathe 1", Box::new(is("Mathematik 1")), 1),
        ("AuD", Box::new(is("Algorithmen und Datenstrukturen")), 1),
        ("DB", Box::new(is("Datenbanken")), 1),
        ("BS", Box::new(is("Betriebssysteme I")), 3),
        ("TM", Box::new(starts("Technische Mechanik 1")), 1),
        ("ET", Box::new(is("Elektrotechnik 1")), 2),
        ("Analysis", Box::new(is("Analysis I")), 1),
        ("Masterarbeit", Box::new(is("Master-Arbeit")), 1),
        ("Physik", Box::new(is("Physik")), 1),
        ("algoritmen", Box::new(has("Algorithmen")), 1),
    ];
    let mut failed = Vec::new();
    for (text, meant, at_most) in &cases {
        if let Some(at) = place(&db, text, meant.as_ref()) {
            if at.is_none_or(|at| at > *at_most) {
                failed.push(format!("„{text}“: {at:?}, expected at most {at_most}"));
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn capitals_as_typed_count_and_the_other_title_counts_less() {
    let db = open();
    if !offers(&db, &|t| t.starts_with("TIN*")) || !offers(&db, &|t| t == "Tissue Engineering") {
        return;
    }
    let at = |text: &str, title: &dyn Fn(&str) -> bool| found(&db, text, None).iter().position(|(_, t)| title(t)).unwrap();
    // „TI" is written so in „TIN*", not in „Tissue" (owner, 2026-10-19: „Tissue Engineering < TIN*")
    assert!(at("TI", &|t| t.starts_with("TIN*")) < at("TI", &|t| t == "Tissue Engineering"));
    assert!(at("TI", &|t| t.starts_with("TIN*")) < 4, "„TIN*“ eher so an stelle 3/4");
    assert!(at("Ti", &|t| t == "Tissue Engineering") < at("Ti", &|t| t.starts_with("TIN*")));
    // „ti" finds no word's start in the title one does not see: not „Time-discrete Systems"
    assert!(!found(&db, "TI", None).iter().any(|(_, t)| t == "Zeitdiskrete Systeme und Regelungen"));
}

#[test]
fn lecturers_are_found_by_their_names_written_alike() {
    let db = open();
    let names = folia_search::search_lecturers(&db).unwrap();
    if !names.iter().any(|l| l.name == "Hofstedt, Petra") {
        return;
    }
    let ids = |text: &str| found(&db, text, None).into_iter().map(|(id, _)| id).collect::<BTreeSet<String>>();
    let hers = ids("Hofstedt");
    assert!(!hers.is_empty());
    for text in ["hofstedt", "Prof. Hofstedt", "Hoffstedt", "Hofstätt", "Hofstet", "Petra Hofstedt", "P. Hofstedt", "Hofstedt Petra"] {
        assert_eq!(ids(text), hers, "„{text}“");
    }
    // a first name only with the surname („Petra" alone is corrected to a word of a title)
    assert!(ids("Petra").is_disjoint(&hers), "a first name only with the surname");
    // the start of a name from four letters on: „Hofs“ is hers, „Hof“ is no find in a name
    let hers: Vec<String> = hers.into_iter().collect();
    let marks = folia_search::marks(&db, "Hofs", None, &hers).unwrap();
    assert!(hers.iter().all(|id| marks.get(id).is_some_and(|m| m.lecturer.is_some())));
    let marks = folia_search::marks(&db, "Hof", None, &hers).unwrap();
    assert!(marks.values().all(|m| m.lecturer.is_none()));
}

/// The newest regulations of a program whose slug starts with `slug`.
fn program(db: &dyn Database, slug: &str) -> Option<String> {
    let rows = db
        .query("test_program", "SELECT p.id FROM v_program p WHERE p.slug LIKE ? AND p.is_latest_po = 1 LIMIT 1", &[Value::from(format!("{slug}-%").as_str())])
        .unwrap();
    match rows.rows.first().and_then(|row| row.first()) {
        Some(Value::Text(id)) => Some(id.clone()),
        _ => None,
    }
}

#[test]
fn a_search_finds_the_same_for_everyone_and_orders_by_the_program() {
    let db = open();
    let Some(informatik) = program(&db, "master-informatik") else { return };
    for text in ["SE", "TI", "Mathe", "Hofstedt", "algoritmen", "Masterarbeit"] {
        let plain = found(&db, text, None);
        let mine = found(&db, text, Some(&informatik));
        let set = |list: &[(String, String)]| list.iter().map(|(id, _)| id.clone()).collect::<BTreeSet<String>>();
        assert_eq!(set(&plain), set(&mine), "„{text}“ finds the same modules with a program");
    }
    // One's own module named in the other language before another's named in the language shown,
    // not before one named by its own abbreviation (`rank::points::PROGRAM`).
    let first_own = |text: &str, program: &str| {
        let theirs = folia_search::search_program_modules(&db, program).unwrap();
        found(&db, text, Some(program)).iter().position(|(id, _)| theirs.contains(id)).map(|i| i + 1)
    };
    if offers(&db, &|t| t == "Softwaretechnik") {
        assert!(first_own("SE", &informatik).is_some_and(|at| at <= 2), "„SE“: {:?}", first_own("SE", &informatik));
    }
    if let Some(ai) = program(&db, "master-artificial-intelligence") {
        assert_eq!(first_own("Masterarbeit", &ai), Some(1), "one's own „Master Thesis“ first");
    }
}

#[test]
fn a_text_without_a_word_finds_nothing() {
    let db = open();
    let plan = Plan::new(&db, "%", None, None).unwrap().unwrap();
    assert!(plan.table().is_none());
    assert_eq!(plan.condition("sr"), "0");
    assert!(Plan::new(&db, "   ", None, None).unwrap().is_none());
}

/// Writes what the search finds for the queries of a file, one per line, for the prototype
/// (folia/design/suche/engine.mjs) to compare with: `SEARCH_DUMP_QUERIES=<file>
/// SEARCH_DUMP_OUT=<file> cargo test -p folia-search --test ranking -- --ignored`, ordered for the
/// program whose slug `SEARCH_DUMP_PROGRAM` names, if set.
#[test]
#[ignore = "a tool: compares the search with its prototype"]
fn dump_for_the_prototype() {
    let db = open();
    let queries = std::fs::read_to_string(std::env::var("SEARCH_DUMP_QUERIES").unwrap()).unwrap();
    let program = std::env::var("SEARCH_DUMP_PROGRAM").ok().map(|slug| {
        let rows = db.query("test_dump_program", "SELECT id FROM v_program WHERE slug = ?", &[Value::from(slug.as_str())]).unwrap();
        match rows.rows.first().and_then(|row| row.first()) {
            Some(Value::Text(id)) => id.clone(),
            _ => panic!("no program {slug}"),
        }
    });
    let mut out = String::new();
    for text in queries.lines().filter(|line| !line.trim().is_empty()) {
        let resolution = folia_search::resolve(&db, text).unwrap();
        let plan = Plan::new(&db, text, Some(&resolution), program.as_deref()).unwrap().unwrap();
        let ids: Vec<String> = match plan.table() {
            Some((table, params)) => db
                .query("test_dump", &format!("SELECT sr.module_id FROM {table} sr WHERE {} ORDER BY sr.rank", plan.condition("sr")), &params)
                .unwrap()
                .rows
                .iter()
                .map(|row| match row.first() {
                    Some(Value::Text(id)) => id.clone(),
                    _ => String::new(),
                })
                .collect(),
            None => Vec::new(),
        };
        out.push_str(&format!("{text}\t{}\n", ids.join(" ")));
    }
    std::fs::write(std::env::var("SEARCH_DUMP_OUT").unwrap(), out).unwrap();
}
