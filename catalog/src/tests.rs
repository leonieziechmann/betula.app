//! The data layer against a real snapshot.
//!
//! The snapshot is found through `BTU_TEST_SNAPSHOT` (a path to a `catalog-*.db`), else
//! through `../snapshot/current.json` as `scraper export` writes it. Without one the tests
//! fail: a green run must mean that every query ran against real data. Counts that depend
//! on the day's data are compared with direct SQL on the views; the exact numbers of
//! docs/frontend-rewrite.md §4 are asserted only for the snapshot they were taken from.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::db::{fetch, Database, DbError, FromRow, Row, Rows, Value};
use crate::filter::{
    CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey,
    TurnusFilter,
};
use crate::labels::{self, Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};
use crate::native::NativeDatabase;
use crate::queries;

/// `content_digest` of the snapshot the pinned numbers below were taken from (2026-09-19).
const PINNED_DIGEST: &str = "e20a744cfcbc2f475a56d7b47c8fffff69120c9f507d9cd288ffc15a6a695277";

const INFORMATIK_BSC: &str = "bachelor-informatik-2008";

fn snapshot_path() -> PathBuf {
    if let Ok(path) = std::env::var("BTU_TEST_SNAPSHOT") {
        return PathBuf::from(path);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("snapshot");
    let pointer = std::fs::read_to_string(dir.join("current.json")).unwrap_or_else(|e| {
        panic!(
            "no catalog snapshot for the tests ({e}). Run `scraper export`, or set BTU_TEST_SNAPSHOT \
             to a catalog-*.db (docs/operations.md)."
        )
    });
    // {"file":"catalog-<hash>.db", …}; not worth a JSON dependency.
    let file = pointer
        .split("\"file\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .unwrap_or_else(|| panic!("snapshot/current.json names no file: {pointer}"));
    dir.join(file)
}

fn open() -> NativeDatabase {
    let path = snapshot_path();
    NativeDatabase::open(&path).unwrap_or_else(|e| panic!("{e}"))
}

/// Direct SQL on the snapshot, to compare the query layer against.
fn scalar(db: &dyn Database, sql: &str) -> i64 {
    let rows = db.query("test", sql, &[]).unwrap_or_else(|e| panic!("{sql}: {e}"));
    match rows.rows.first().and_then(|r| r.first()) {
        Some(Value::Integer(n)) => *n,
        other => panic!("{sql}: expected one integer, got {other:?}"),
    }
}

fn column(db: &dyn Database, sql: &str) -> Vec<String> {
    let rows = db.query("test", sql, &[]).unwrap_or_else(|e| panic!("{sql}: {e}"));
    rows.rows
        .iter()
        .filter_map(|r| match r.first() {
            Some(Value::Text(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn is_pinned(db: &dyn Database) -> bool {
    let pinned = queries::meta(db).unwrap().content_digest.as_deref() == Some(PINNED_DIGEST);
    if !pinned {
        eprintln!("note: snapshot differs from the pinned one; exact numbers are not asserted");
    }
    pinned
}

/// No filter at all: every module, whatever its offer status.
fn everything() -> CatalogQuery {
    CatalogQuery { offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() }
}

/// Records which queries ran.
struct Recording {
    inner: NativeDatabase,
    ran: RefCell<BTreeSet<&'static str>>,
}

impl Database for Recording {
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
        self.ran.borrow_mut().insert(name);
        self.inner.query(name, sql, params)
    }
}

#[test]
fn every_query_runs_against_the_snapshot() {
    let db = Recording { inner: open(), ran: RefCell::new(BTreeSet::new()) };

    let meta = queries::meta(&db).unwrap();
    assert!(meta.data_changed_at.is_some() && meta.current_semester.is_some(), "{meta:?}");

    let semesters = queries::semesters(&db).unwrap();
    assert_eq!(semesters.iter().filter(|s| s.is_current).count(), 1, "exactly one current semester");
    assert!(!queries::departments(&db).unwrap().is_empty());

    let programs = queries::programs(&db).unwrap();
    assert!(!programs.is_empty());
    let program = queries::program_by_slug(&db, &programs[0].slug).unwrap().expect("slug resolves");
    assert_eq!(program, programs[0]);
    assert_eq!(queries::program_by_slug(&db, "no-such-program").unwrap(), None);
    for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
        queries::program_modules(&db, &program.id, relation).unwrap();
    }

    let page = queries::catalog_page(&db, &everything(), 0, 50).unwrap();
    assert_eq!(page.rows.len(), 50);
    let module = queries::module(&db, &page.rows[0].id).unwrap().expect("listed module has a page");
    assert_eq!(module.title, page.rows[0].title);
    assert_eq!(queries::module(&db, "00000").unwrap(), None);
    queries::module_prerequisites(&db, &module.id).unwrap();
    assert!(!queries::search_suggestions(&db, "Algebra", 10).unwrap().is_empty());

    // The satellites, each for a module or program that really has the data.
    let pick = |sql: &str| column(&db.inner, sql).pop().unwrap_or_else(|| panic!("no row for: {sql}"));
    assert!(queries::lecturer_names(&db).unwrap().len() > 100);
    let id = pick("SELECT module_id FROM v_module_lecturer LIMIT 1");
    assert!(!queries::module_lecturers(&db, &id).unwrap().is_empty());
    let id = pick("SELECT module_id FROM v_module_teaching_form WHERE sws IS NOT NULL LIMIT 1");
    assert!(queries::module_teaching_forms(&db, &id).unwrap().iter().any(|f| f.sws.is_some()));
    let id = pick("SELECT module_id FROM v_module_text_item WHERE kind = 'literature' LIMIT 1");
    assert!(!queries::module_text_items(&db, &id).unwrap().is_empty());
    let id = pick("SELECT module_id FROM v_module_successor LIMIT 1");
    assert!(!queries::module_successors(&db, &id).unwrap().is_empty());
    let id = pick("SELECT module_id FROM v_module_schedule WHERE weekday IS NOT NULL AND room IS NOT NULL LIMIT 1");
    let schedule = queries::module_schedule(&db, &id).unwrap();
    assert!(schedule.iter().any(|d| d.weekday.is_some() && d.room.is_some()), "{schedule:?}");
    let id = pick("SELECT module_id FROM v_module_exam LIMIT 1");
    assert!(!queries::module_exams(&db, &id).unwrap().is_empty());
    let id = pick("SELECT module_id FROM v_module_program_link WHERE program_slug IS NOT NULL LIMIT 1");
    assert!(queries::module_program_links(&db, &id).unwrap().iter().any(|l| l.program_slug.is_some()));

    let id = pick("SELECT program_id FROM v_program_version LIMIT 1");
    assert!(!queries::program_versions(&db, &id).unwrap().is_empty());
    let id = pick("SELECT program_id FROM v_program_counterpart LIMIT 1");
    assert!(queries::program_counterpart(&db, &id).unwrap().is_some());
    let id = pick("SELECT program_id FROM v_program_document LIMIT 1");
    assert!(!queries::program_documents(&db, &id).unwrap().is_empty());
    let id = pick("SELECT program_id FROM v_program_module_area LIMIT 1");
    assert!(!queries::program_areas(&db, &id).unwrap().is_empty());
    let id = pick("SELECT program_id FROM v_program_plan LIMIT 1");
    assert!(queries::program_plan(&db, &id).unwrap().is_some_and(|plan| plan.layout_json.starts_with('{')));
    assert!(!queries::program_plan_entries(&db, &id).unwrap().is_empty());

    // Every `pub fn` of queries.rs must have run above.
    let declared: BTreeSet<&str> = include_str!("queries.rs")
        .lines()
        .filter_map(|line| line.strip_prefix("pub fn "))
        .filter_map(|rest| rest.split(['(', '<']).next())
        .collect();
    let ran = db.ran.borrow();
    let never_ran: Vec<&&str> = declared.iter().filter(|name| !ran.contains(**name)).collect();
    assert!(never_ran.is_empty(), "queries the tests never ran against the snapshot: {never_ran:?}");
    let unnamed: Vec<&&str> = ran.iter().filter(|name| !declared.contains(**name)).collect();
    assert!(unnamed.is_empty(), "query names that are not the name of their function: {unnamed:?}");
}

#[test]
fn catalog_filters_match_direct_sql() {
    let db = open();
    let program_id = queries::program_by_slug(&db, INFORMATIK_BSC).unwrap().expect("Informatik B.Sc.").id;
    let scope = |relation, plan_semester, kinds: Vec<KindFilter>| {
        Some(ProgramScope { program_slug: INFORMATIK_BSC.into(), relation, plan_semester, kinds })
    };
    let pm = format!("v_module_facets f JOIN v_program_module pm ON pm.module_id = f.module_id AND pm.program_id = '{program_id}'");

    let cases: Vec<(&str, CatalogQuery, String)> = vec![
        ("no filter", everything(), "SELECT COUNT(*) FROM v_module_facets".into()),
        (
            "the default catalog hides what is no longer offered",
            CatalogQuery::default(),
            "SELECT COUNT(*) FROM v_module_facets WHERE offer_status IN ('active', 'phase_out')".into(),
        ),
        (
            "inside a program the default lists the whole curriculum",
            CatalogQuery { program: scope(ProgramRelation::Curricular, None, vec![]), ..Default::default() },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular'"),
        ),
        (
            "winter and exercise (the fixture of the acceptance criteria)",
            CatalogQuery {
                turnus: TurnusFilter { winter: true, ..Default::default() },
                teaching_forms: vec![TeachingForm::Exercise],
                ..everything()
            },
            "SELECT COUNT(*) FROM v_module_facets WHERE offered_winter = 1 AND has_exercise = 1".into(),
        ),
        (
            "summer or irregular, odd years",
            CatalogQuery {
                turnus: TurnusFilter { summer: true, irregular: true, year_parity: Some(TurnusParity::Odd), ..Default::default() },
                ..everything()
            },
            "SELECT COUNT(*) FROM v_module_facets WHERE (offered_summer = 1 OR turnus_season = 'irregular') \
             AND (turnus_parity IS NULL OR turnus_parity = 'odd')".into(),
        ),
        (
            "english, graded, active, 6 to 8 credits",
            CatalogQuery {
                languages: vec![Language::English],
                graded: Some(true),
                offer: Some(vec![OfferStatus::Active]),
                credits_min: Some(6.0),
                credits_max: Some(8.0),
                ..everything()
            },
            "SELECT COUNT(*) FROM v_module_facets WHERE teaches_english = 1 AND is_graded = 1 \
             AND offer_status = 'active' AND credits >= 6 AND credits <= 8".into(),
        ),
        (
            "ungraded: the filter that returned nothing in v1",
            CatalogQuery { graded: Some(false), ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE is_graded = 0".into(),
        ),
        (
            "exam: MCA or an oral part; limited; two semesters; not FÜS",
            CatalogQuery {
                exam_forms: vec![ExamForm::Mca],
                exam_parts: vec![ExamPart::Oral],
                limited: Some(true),
                duration_semesters: Some(2),
                fues: Some(false),
                ..everything()
            },
            "SELECT COUNT(*) FROM v_module_facets WHERE (exam_form = 'mca' OR exam_oral = 1) \
             AND is_limited = 1 AND duration_semesters = 2 AND is_fues = 0".into(),
        ),
        (
            "campus Senftenberg: only modules with room data can match",
            CatalogQuery { campuses: vec![Campus::Senftenberg], ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE at_senftenberg = 1".into(),
        ),
        (
            "curriculum of Informatik B.Sc.",
            CatalogQuery { program: scope(ProgramRelation::Curricular, None, vec![]), ..everything() },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular'"),
        ),
        (
            "FÜS list of Informatik B.Sc.",
            CatalogQuery { program: scope(ProgramRelation::Fues, None, vec![]), ..everything() },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'fues'"),
        ),
        (
            "Informatik B.Sc., third semester of the plan",
            CatalogQuery {
                program: scope(ProgramRelation::Curricular, Some(PlanSemesterFilter::Semester(3)), vec![]),
                ..everything()
            },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular' WHERE pm.plan_semester = 3"),
        ),
        (
            "Informatik B.Sc., compulsory or without a stated kind",
            CatalogQuery {
                program: scope(
                    ProgramRelation::Curricular,
                    None,
                    vec![KindFilter::Stated(ModuleKind::Compulsory), KindFilter::Unstated],
                ),
                ..everything()
            },
            format!(
                "SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular' WHERE (pm.kind = 'compulsory' OR pm.kind IS NULL)"
            ),
        ),
        (
            "search text",
            CatalogQuery { text: "Algebra".into(), ..everything() },
            "SELECT COUNT(DISTINCT module_id) FROM v_module_search WHERE term LIKE '%Algebra%'".into(),
        ),
        (
            "mandatory prerequisites met with nothing passed",
            CatalogQuery { prerequisites_met_by: Some(vec![]), ..everything() },
            "SELECT COUNT(*) FROM v_module_facets f WHERE NOT EXISTS (SELECT 1 FROM v_module_prerequisite p \
             WHERE p.module_id = f.module_id AND p.kind = 'mandatory')".into(),
        ),
    ];

    for (name, query, direct) in &cases {
        let expected = scalar(&db, direct) as u64;
        assert_eq!(queries::catalog_count(&db, query).unwrap(), expected, "{name}");

        // The page header is exact, and paging neither repeats nor skips a module.
        let mut seen = BTreeSet::new();
        let mut offset = 0;
        loop {
            let page = queries::catalog_page(&db, query, offset, 400).unwrap();
            assert_eq!(page.total, expected, "{name}: total of the page at {offset}");
            if page.rows.is_empty() {
                break;
            }
            offset += page.rows.len() as u64;
            for row in page.rows {
                assert!(seen.insert(row.id.clone()), "{name}: module {} listed twice", row.id);
            }
        }
        assert_eq!(seen.len() as u64, expected, "{name}: modules reached by paging");
    }

    if is_pinned(&db) {
        let (_, winter_exercise, _) = cases.iter().find(|(name, _, _)| name.starts_with("winter and exercise")).unwrap();
        assert_eq!(queries::catalog_count(&db, winter_exercise).unwrap(), 979);
        assert_eq!(queries::catalog_count(&db, &CatalogQuery::default()).unwrap(), 2864 + 340);
    }
}

#[test]
fn lecturer_and_bookmark_filters() {
    let db = open();
    let name = column(&db, "SELECT name FROM v_module_lecturer GROUP BY name ORDER BY COUNT(*) DESC LIMIT 1")
        .pop()
        .expect("a lecturer");
    let teaches = scalar(
        &db,
        &format!("SELECT COUNT(DISTINCT module_id) FROM v_module_lecturer WHERE name = '{}'", name.replace('\'', "''")),
    ) as u64;
    let all = queries::catalog_count(&db, &everything()).unwrap();

    let include = CatalogQuery { lecturers_include: vec![name.clone()], ..everything() };
    let exclude = CatalogQuery { lecturers_exclude: vec![name], ..everything() };
    assert_eq!(queries::catalog_count(&db, &include).unwrap(), teaches);
    assert_eq!(queries::catalog_count(&db, &exclude).unwrap(), all - teaches);

    let ids: Vec<String> = queries::catalog_page(&db, &everything(), 10, 3).unwrap().rows.into_iter().map(|r| r.id).collect();
    let bookmarks = CatalogQuery { only_ids: Some(ids.clone()), sort: SortKey::Id, ..everything() };
    let page = queries::catalog_page(&db, &bookmarks, 0, 50).unwrap();
    assert_eq!(page.total, 3);
    let mut sorted = ids;
    sorted.sort();
    assert_eq!(page.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), sorted);

    // An empty „Gemerkt" list is empty, not "no filter".
    let none = CatalogQuery { only_ids: Some(vec![]), ..everything() };
    assert_eq!(queries::catalog_count(&db, &none).unwrap(), 0);
}

#[test]
fn search_text_is_literal() {
    let db = open();
    let all = queries::catalog_count(&db, &everything()).unwrap();
    for wildcard in ["%", "_", "\\"] {
        let query = CatalogQuery { text: wildcard.into(), ..everything() };
        let literal = scalar(
            &db,
            &format!("SELECT COUNT(DISTINCT module_id) FROM v_module_search WHERE instr(term, '{wildcard}') > 0"),
        ) as u64;
        let found = queries::catalog_count(&db, &query).unwrap();
        assert_eq!(found, literal, "search for {wildcard:?} must match the character itself");
        assert!(found < all);
    }
}

#[test]
fn unknown_stays_unknown() {
    let db = open();
    let programs = queries::programs(&db).unwrap();

    // A degree label is never invented: without a stated one the display falls back to the level,
    // and where even that is unknown the raw QIS text is shown.
    let unlabelled = programs.iter().filter(|p| p.degree_label.is_none()).count() as i64;
    assert_eq!(unlabelled, scalar(&db, "SELECT COUNT(*) FROM v_program WHERE degree_label IS NULL"));
    for program in programs.iter().filter(|p| p.degree_display.is_none()) {
        assert_eq!(program.degree(), program.degree_raw);
    }

    // No source states the kind: it is None, never „Pflicht".
    let informatik = queries::program_by_slug(&db, INFORMATIK_BSC).unwrap().expect("Informatik B.Sc.");
    let curriculum = queries::program_modules(&db, &informatik.id, ProgramRelation::Curricular).unwrap();
    let without_kind = curriculum.iter().filter(|m| m.kind.is_none()).count() as i64;
    assert_eq!(
        without_kind,
        scalar(&db, &format!("SELECT COUNT(*) FROM v_program_module WHERE program_id = '{}' AND relation = 'curricular' AND kind IS NULL", informatik.id))
    );
    for module in &curriculum {
        assert_eq!(module.kind.is_none(), module.kind_basis.is_none(), "{}", module.module_id);
    }

    // Campus is tri-state.
    let row = queries::catalog_page(&db, &everything(), 0, 1).unwrap().rows.remove(0);
    let module = queries::module(&db, &row.id).unwrap().expect("module");
    let known = scalar(&db, &format!("SELECT at_zentralcampus IS NOT NULL FROM v_module WHERE id = '{}'", module.id));
    assert_eq!(module.at_zentralcampus.is_some(), known == 1);
}

#[test]
fn snapshot_facts_of_the_brief() {
    let db = open();
    let informatik = queries::program_by_slug(&db, INFORMATIK_BSC).unwrap().expect("Informatik B.Sc.");
    let curricular = queries::program_modules(&db, &informatik.id, ProgramRelation::Curricular).unwrap();
    let fues = queries::program_modules(&db, &informatik.id, ProgramRelation::Fues).unwrap();
    assert_eq!(curricular.len() as i64, informatik.curricular_modules);
    assert_eq!(fues.len() as i64, informatik.fues_modules);
    // Each program has its own FÜS list, and it never overlaps with its curriculum.
    let curricular_ids: BTreeSet<&str> = curricular.iter().map(|m| m.module_id.as_str()).collect();
    assert!(fues.iter().all(|m| !curricular_ids.contains(m.module_id.as_str())));

    if !is_pinned(&db) {
        return;
    }
    assert_eq!((curricular.len(), fues.len()), (109, 116));
    assert_eq!(queries::programs(&db).unwrap().len(), 182);
    assert_eq!(queries::departments(&db).unwrap().len(), 14);
    assert_eq!(queries::meta(&db).unwrap().current_semester.as_deref(), Some("2026S"));

    let count = |query: CatalogQuery| queries::catalog_count(&db, &query).unwrap();
    assert_eq!(count(everything()), 4908);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::Active]), ..everything() }), 2864);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::NotOffered]), ..everything() }), 1704);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::PhaseOut]), ..everything() }), 340);
    assert_eq!(count(CatalogQuery { languages: vec![Language::German], ..everything() }), 3928);
    assert_eq!(count(CatalogQuery { languages: vec![Language::English], ..everything() }), 980);
    assert_eq!(count(CatalogQuery { graded: Some(true), ..everything() }), 4623);
    assert_eq!(count(CatalogQuery { graded: Some(false), ..everything() }), 283);
    assert_eq!(count(CatalogQuery { fues: Some(true), ..everything() }), 288);
    let winter = TurnusFilter { winter: true, ..Default::default() };
    let summer = TurnusFilter { summer: true, ..Default::default() };
    let irregular = TurnusFilter { irregular: true, ..Default::default() };
    assert_eq!(count(CatalogQuery { turnus: winter, ..everything() }), 1950 + 604);
    assert_eq!(count(CatalogQuery { turnus: summer, ..everything() }), 1661 + 604);
    assert_eq!(count(CatalogQuery { turnus: irregular, ..everything() }), 691);
    let campus_known = scalar(&db, "SELECT COUNT(*) FROM v_module_facets WHERE at_zentralcampus IS NOT NULL");
    assert_eq!(campus_known, 234);
}

#[test]
fn every_enum_code_has_a_label() {
    let db = open();
    let enums = labels::code_sets();
    let covered = |codes: &[String]| enums.iter().any(|(_, known)| codes.iter().all(|c| known.contains(&c.as_str())));

    // Columns that never reach a page.
    let internal = ["detail_status", "page_lang", "degree_label_basis"];

    let mut missing = Vec::new();
    let mut constraints = 0;
    for table_sql in column(&db, "SELECT sql FROM sqlite_master WHERE type = 'table' AND sql IS NOT NULL") {
        for (col, codes) in check_constraints(&table_sql) {
            constraints += 1;
            if !internal.contains(&col.as_str()) && !covered(&codes) {
                missing.push(format!("{col} IN {codes:?}"));
            }
        }
    }
    assert!(constraints >= 25, "the CHECK parser found only {constraints} enum constraints");

    // Codes that views define themselves.
    for (what, sql) in [
        ("v_module_lecturer.role", "SELECT DISTINCT role FROM v_module_lecturer"),
        ("v_program_counterpart.counterpart_level", "SELECT DISTINCT counterpart_level FROM v_program_counterpart"),
    ] {
        let codes = column(&db, sql);
        if !covered(&codes) {
            missing.push(format!("{what}: {codes:?}"));
        }
    }
    assert!(missing.is_empty(), "codes without a German label in labels.rs: {missing:#?}");
}

/// `CHECK (col IN ('a', 'b'))` constraints with text codes, from a CREATE TABLE statement.
fn check_constraints(table_sql: &str) -> Vec<(String, Vec<String>)> {
    let mut found = Vec::new();
    for (start, _) in table_sql.match_indices("CHECK (") {
        let rest = &table_sql[start + "CHECK (".len()..];
        let Some((col, list)) = rest.split_once(" IN (") else { continue };
        if col.is_empty() || !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let Some((list, _)) = list.split_once(')') else { continue };
        let codes: Vec<String> = list.split('\'').skip(1).step_by(2).map(str::to_string).collect();
        if !codes.is_empty() {
            found.push((col.to_string(), codes));
        }
    }
    found
}

#[test]
fn errors_are_reported_not_swallowed() {
    struct Wanted;
    impl FromRow for Wanted {
        fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
            row.int("no_such_column").map(|_| Wanted)
        }
    }

    let db = open();
    let sql_error = fetch::<Wanted>(&db, "test", "SELECT * FROM no_such_view", &[]);
    assert!(matches!(sql_error, Err(DbError::Sql { query: "test", .. })), "{:?}", sql_error.err());
    let decode_error = fetch::<Wanted>(&db, "test", "SELECT 1 AS x", &[]);
    assert!(matches!(decode_error, Err(DbError::Decode { .. })), "{:?}", decode_error.err());

    let missing = NativeDatabase::open(std::path::Path::new("no-such-snapshot.db"));
    assert!(matches!(missing, Err(DbError::Unavailable(_))));
}
