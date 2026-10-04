//! The data layer against a real snapshot.
//!
//! The snapshot is found through `FOLIA_TEST_SNAPSHOT` (a path to a `catalog-*.db`), else
//! through `../snapshot/current.json` as `radix export` writes it. Without one the tests
//! fail: a green run must mean that every query ran against real data. Counts that depend
//! on the day's data are compared with direct SQL on the views; the exact numbers of
//! docs/history/frontend-rewrite.md §4 are asserted only for the snapshot they were taken from.
//! The Studienplan's checks pin a snapshot of their own (`studyplan_db`).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

use folia_model::db::{fetch, Database, DbError, FromRow, Row, Rows, Value};
use folia_model::labels::{self, Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};
use folia_model::native::NativeDatabase;
use folia_model::rows_detail::DateRow;
use folia_query as queries;
use folia_routes::filter::{
    CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey,
    TurnusFilter,
};
use folia_test_support::{column, open, scalar, studyplan_db};

use crate as pages;

/// `content_digest` of the snapshot the pinned numbers below were taken from (2026-09-19).
const PINNED_DIGEST: &str = "14dc847aef858b5c61763a67a093279ee9d6d1570e23e692b0397c757c159fc0";

const INFORMATIK_BSC: &str = "bachelor-informatik-2008";

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
    // The place of a row in the list is the place the page lists it at.
    assert_eq!(queries::catalog_position(&db, &everything(), &page.rows[7].id).unwrap(), Some(7));
    assert_eq!(queries::catalog_position(&db, &everything(), "00000").unwrap(), None);
    assert_eq!(queries::module(&db, "00000").unwrap(), None);
    queries::module_prerequisites(&db, &module.id).unwrap();
    // The search, where a text finds nothing as typed (`search::resolve`), and under a filtered list.
    assert!(folia_search::search_count(&db, "Algebra", None).unwrap() > 0);
    assert_eq!(folia_search::search_words_found(&db, "algebra xqzvw").unwrap(), vec![true, false]);
    assert!(folia_search::search_titles(&db).unwrap().len() as u64 >= queries::catalog_count(&db, &everything()).unwrap());
    let elsewhere = queries::search_elsewhere(&db, &CatalogQuery { text: "Algebra".into(), ..Default::default() }).unwrap();
    assert_eq!(elsewhere.offered, 0, "the catalog's default lists every offered module");
    // „Ähnliche Module": of modules the filter holds, those the text does not find.
    let ids: Vec<String> = page.rows.iter().map(|row| row.id.clone()).collect();
    let algebra = CatalogQuery { text: "Algebra".into(), ..everything() };
    let found = queries::catalog_count(&db, &CatalogQuery { only_ids: Some(ids.clone()), ..algebra.clone() }).unwrap();
    assert_eq!(queries::similar_rows(&db, &algebra, &ids).unwrap().len() as u64 + found, ids.len() as u64);

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
    assert!(queries::module_program_links(&db, &id).unwrap().iter().any(|l| l.program_slug.is_some() && l.program_id.is_some()));
    let id = pick("SELECT module_id FROM v_program_plan_entry WHERE module_id IS NOT NULL LIMIT 1");
    let places = queries::module_plan_places(&db, &id).unwrap();
    assert!(!places.is_empty() && places.iter().all(|place| !place.program_id.is_empty()), "{places:?}");

    let id = pick("SELECT program_id FROM v_program_version LIMIT 1");
    assert!(!queries::program_versions(&db, &id).unwrap().is_empty());
    let id = pick("SELECT program_id FROM v_program_counterpart LIMIT 1");
    assert!(queries::program_counterpart(&db, &id).unwrap().is_some());
    let id = pick("SELECT program_id FROM v_program_document LIMIT 1");
    assert!(!queries::program_documents(&db, &id).unwrap().is_empty());
    let id = pick("SELECT program_id FROM v_program_module_area LIMIT 1");
    assert!(!queries::program_areas(&db, &id).unwrap().is_empty());
    let tree = queries::program_area_tree(&db, &id).unwrap();
    assert!(tree.iter().all(|node| node.parent_id.is_none_or(|parent| tree.iter().any(|above| above.id == parent))), "every parent is in the tree");
    let id = pick("SELECT program_id FROM v_program_plan LIMIT 1");
    assert!(queries::program_plan(&db, &id).unwrap().is_some_and(|plan| plan.layout_json.starts_with('{')));
    assert!(queries::program_department_counts(&db).unwrap().iter().any(|count| count.thesis_modules > 0));
    let semesters = queries::program_plan_semesters(&db, &id).unwrap();
    assert!(semesters.first().is_some_and(|first| *first >= 1) && semesters.windows(2).all(|pair| pair[0] < pair[1]), "{semesters:?}");
    let entries = queries::program_plan_entries(&db, &id).unwrap();
    assert!(!entries.is_empty());
    // A sum the regulation prints over rows of its plan names those rows, and they reach it.
    for total in queries::program_plan_totals(&db, &id).unwrap() {
        assert_eq!(total.entries.len() as i64, total.entry_count, "{total:?}");
        assert!(total.entries.iter().all(|ord| entries.iter().any(|entry| entry.ord == *ord)), "{total:?}");
        assert!(total.credits >= total.min_credits - 0.01 && total.credits <= total.max_credits + 0.01, "{total:?}");
    }

    let curriculum = queries::curriculum_links(&db).unwrap();
    assert!(curriculum.len() > 1000 && curriculum.iter().all(|(program, module)| !program.is_empty() && !module.is_empty()));
    let ids = queries::module_ids(&db).unwrap();
    assert!(ids.len() >= page.total as usize && ids.contains(&module.id));
    // The semantic search's vectors, as Radix computed them: 384 values of 4 bits (two a byte, the
    // nibble value + 8), of unit length up to their rounding, of modules the catalog has. A
    // snapshot of a Radix without a model has none.
    let vectors = queries::module_vectors(&db).unwrap();
    for v in &vectors {
        let values = v.vector.iter().flat_map(|b| [i16::from(b & 0x0f) - 8, i16::from(b >> 4) - 8]);
        let length = values.map(|c| (f32::from(c) * v.scale).powi(2)).sum::<f32>().sqrt();
        assert!(v.vector.len() == 192 && (length - 1.0).abs() < 0.1 && ids.contains(&v.module_id), "{} {length}", v.module_id);
    }

    the_studyplan_queries(&db, meta.current_semester.as_deref().expect("a current semester"));

    // „Mein Studium": what its modules ask for of others, for the modules asked and in order; a
    // list without a module id asks nothing.
    let asking = column(&db.inner, "SELECT module_id FROM v_module_prerequisite GROUP BY module_id ORDER BY module_id LIMIT 3");
    assert_eq!(asking.len(), 3);
    let study = crate::study_modules(&db, &[asking.clone(), vec!["1 OR 1=1".to_string()]].concat()).unwrap();
    let listed = asking.iter().map(|id| format!("'{id}'")).collect::<Vec<_>>().join(", ");
    assert_eq!(study.prerequisites.len() as i64, scalar(&db.inner, &format!("SELECT COUNT(*) FROM v_module_prerequisite WHERE module_id IN ({listed})")));
    assert!(study.prerequisites.iter().all(|p| asking.contains(&p.module_id) && !p.required_module_id.is_empty()));
    let order = |p: &folia_model::rows::Prerequisite| (p.module_id.clone(), p.kind.code().to_string(), p.required_module_id.clone());
    assert!(study.prerequisites.windows(2).all(|pair| order(&pair[0]) <= order(&pair[1])));
    assert_eq!(study.rows.len() + study.missing.len(), asking.len());
    assert!(queries::modules_prerequisites(&db, &["1 OR 1=1".to_string()]).unwrap().is_empty());

    // Every `pub fn` of folia-query must have run above, and the search's statements and `meta`.
    let functions = |source: &'static str| -> Vec<&'static str> {
        source.lines().filter_map(|line| line.strip_prefix("pub fn ")).filter_map(|rest| rest.split(['(', '<']).next()).collect()
    };
    let declared: BTreeSet<&str> = functions(include_str!("../../query/src/lib.rs"))
        .into_iter()
        .chain(functions(include_str!("../../search/src/lib.rs")).into_iter().filter(|name| name.starts_with("search_")))
        .chain(["meta"])
        .collect();
    let ran = db.ran.borrow();
    let never_ran: Vec<&&str> = declared.iter().filter(|name| !ran.contains(**name)).collect();
    assert!(never_ran.is_empty(), "queries the tests never ran against the snapshot: {never_ran:?}");
    let unnamed: Vec<&&str> = ran.iter().filter(|name| !declared.contains(**name)).collect();
    assert!(unnamed.is_empty(), "query names that are not the name of their function: {unnamed:?}");
}

/// The Studienplan's queries, run through the recording database of
/// `every_query_runs_against_the_snapshot`: some modules of a semester, and the whole semester for
/// the finder. On any snapshot they say what direct SQL on the views says; the rows themselves are
/// asserted on the pinned snapshot only.
fn the_studyplan_queries(db: &Recording, semester: &str) {
    use std::collections::BTreeMap;

    let direct = |sql: String| scalar(&db.inner, &sql);
    // Four modules taught in the semester whose pages state SWS, so every answer has rows.
    let planned = column(
        &db.inner,
        &format!(
            "SELECT s.module_id FROM v_module_schedule s WHERE s.semester_key = '{semester}' AND s.ord IS NOT NULL \
             AND EXISTS (SELECT 1 FROM v_module_teaching_form f WHERE f.module_id = s.module_id \
             AND f.sws IS NOT NULL AND f.form IS NOT NULL) GROUP BY s.module_id ORDER BY s.module_id LIMIT 4"
        ),
    );
    assert_eq!(planned.len(), 4, "four modules taught in {semester}");
    let asked: BTreeSet<&str> = planned.iter().map(String::as_str).collect();
    let listed = planned.iter().map(|id| format!("'{id}'")).collect::<Vec<_>>().join(", ");

    let schedule = queries::modules_schedule(db, &planned, semester).unwrap();
    assert!(!schedule.is_empty(), "{planned:?} in {semester}");
    assert!(schedule.iter().all(|row| asked.contains(row.module_id.as_str()) && row.date.semester_key == semester), "{schedule:?}");
    assert_eq!(schedule.len() as i64, direct(format!("SELECT COUNT(*) FROM v_module_schedule WHERE semester_key = '{semester}' AND module_id IN ({listed})")));
    let order = |row: &DateRow| (row.date.event_id.clone(), row.ord, row.module_id.clone());
    assert!(schedule.windows(2).all(|pair| order(&pair[0]) <= order(&pair[1])), "by event, row and module");
    // The same set asks the same question, whatever else the list holds; a list without a module
    // id asks nothing (and never everything).
    let mut shuffled: Vec<String> = planned.iter().rev().cloned().collect();
    shuffled.extend([planned[0].clone(), "1 OR 1=1".to_string(), String::new()]);
    assert_eq!(queries::modules_schedule(db, &shuffled, semester).unwrap(), schedule);
    assert!(queries::modules_schedule(db, &[], semester).unwrap().is_empty());
    assert!(queries::modules_schedule(db, &["1 OR 1=1".to_string()], semester).unwrap().is_empty());

    // The finder's rows are the full ones without what it does not compare.
    let lean = |row: &DateRow, room: bool| {
        let mut row = row.clone();
        row.date.event_number = None;
        row.date.instructor = None;
        row.date.comment = None;
        row.date.source_url = None;
        if !room {
            row.date.room = None;
            row.date.room_short = None;
        }
        row
    };
    let whole = queries::semester_schedule(db, semester).unwrap();
    assert_eq!(whole.len() as i64, direct(format!("SELECT COUNT(*) FROM v_module_schedule WHERE semester_key = '{semester}' AND ord IS NOT NULL")));
    let first = &planned[0];
    let full: Vec<DateRow> = schedule.iter().filter(|row| &row.module_id == first && row.ord.is_some()).map(|row| lean(row, false)).collect();
    assert_eq!(whole.iter().filter(|row| &row.module_id == first).cloned().collect::<Vec<_>>(), full, "{first}");

    // Every dated row of the semester is counted once, however many modules link its event.
    let counts = queries::semester_date_counts(db, semester).unwrap();
    assert!(!counts.is_empty());
    assert_eq!(
        counts.iter().map(|count| count.dates).sum::<i64>(),
        direct(format!("SELECT COUNT(DISTINCT event_id || '/' || ord) FROM v_module_schedule WHERE semester_key = '{semester}' AND first_date IS NOT NULL"))
    );

    let sws = queries::modules_teaching_sws(db, &planned).unwrap();
    assert!(!sws.is_empty() && sws.iter().all(|row| asked.contains(row.module_id.as_str()) && row.sws >= 0.0), "{sws:?}");
    assert_eq!(sws.len() as i64, direct(format!("SELECT COUNT(*) FROM (SELECT 1 FROM v_module_teaching_form WHERE sws IS NOT NULL AND form IS NOT NULL AND module_id IN ({listed}) GROUP BY module_id, form)")));
    let taught: BTreeSet<String> = column(&db.inner, &format!("SELECT DISTINCT module_id FROM v_module_schedule WHERE semester_key = '{semester}'")).into_iter().collect();
    let semester_sws = queries::semester_teaching_sws(db, semester).unwrap();
    assert!(!semester_sws.is_empty() && semester_sws.iter().all(|row| taught.contains(&row.module_id)));
    assert!(sws.iter().all(|row| semester_sws.contains(row)), "the finder's answer holds the plan's");

    // Exams: the newest semester that has a dated one, which need not be the current.
    let pick = |sql: String| column(&db.inner, &sql).pop().unwrap_or_else(|| panic!("no row for: {sql}"));
    let exam_semester = pick("SELECT semester_key FROM v_module_exam WHERE first_date IS NOT NULL ORDER BY semester_key DESC LIMIT 1".into());
    let examined = pick(format!("SELECT module_id FROM v_module_exam WHERE semester_key = '{exam_semester}' AND first_date IS NOT NULL ORDER BY module_id LIMIT 1"));
    let exams = queries::modules_exams(db, std::slice::from_ref(&examined), &exam_semester).unwrap();
    assert_eq!(exams.len() as i64, direct(format!("SELECT COUNT(*) FROM v_module_exam WHERE semester_key = '{exam_semester}' AND module_id = '{examined}'")));
    assert!(
        exams.iter().all(|row| row.module_id == examined
            && row.date.event_type.is_none()
            && row.date.group_name.is_none()
            && row.date.rhythm.is_none()
            && row.cancelled_dates.is_none()),
        "an exam row has no type, group, rhythm or cancellations: {exams:?}"
    );
    let all_exams = queries::semester_exams(db, &exam_semester).unwrap();
    assert_eq!(all_exams.len() as i64, direct(format!("SELECT COUNT(*) FROM v_module_exam WHERE semester_key = '{exam_semester}' AND first_date IS NOT NULL")));
    let full: Vec<DateRow> = exams.iter().filter(|row| row.date.first_date.is_some()).map(|row| lean(row, true)).collect();
    assert_eq!(all_exams.iter().filter(|row| row.module_id == examined).cloned().collect::<Vec<_>>(), full, "{examined}: the room stays");
    assert!(queries::modules_exams(db, &[], &exam_semester).unwrap().is_empty());
    assert!(queries::modules_teaching_sws(db, &["1 OR 1=1".to_string()]).unwrap().is_empty());

    // Abbreviations (schema 9): a module's own without a program, and nothing for what is no id.
    let abbrevs = queries::modules_abbrevs(db, &planned, None).unwrap();
    assert!(abbrevs.iter().all(|a| asked.contains(a.module_id.as_str()) && !a.abbrev.trim().is_empty()), "{abbrevs:?}");
    assert!(queries::modules_abbrevs(db, &["1 OR 1=1".to_string()], Some("x")).unwrap().is_empty());

    // Informatik B.Sc.'s first semester on the snapshot the Studienplan's checks were pinned to.
    let Some(pinned) = studyplan_db("every_query_runs_against_the_snapshot") else { return };
    let fs1: Vec<String> = ["12104", "12107", "12102", "11112"].map(String::from).to_vec();
    let mut per_module: BTreeMap<String, usize> = BTreeMap::new();
    for row in queries::modules_schedule(&pinned, &fs1, "2026W").unwrap() {
        assert!(fs1.contains(&row.module_id), "{row:?}");
        *per_module.entry(row.module_id).or_default() += 1;
    }
    let expected: BTreeMap<String, usize> = [("11112", 6), ("12102", 7), ("12104", 9), ("12107", 3)].map(|(id, n)| (id.to_string(), n)).into();
    assert_eq!(per_module, expected);
    // 12104 is examined at Zentralcampus and in Senftenberg at the same hour: one sitting per town.
    let sittings = queries::modules_exams(&pinned, &["12104".to_string()], "2026W").unwrap();
    type Sitting<'a> = (&'a str, Option<i64>, Option<&'a str>, Option<&'a str>, Option<&'a str>);
    fn sitting(row: &DateRow) -> Sitting<'_> {
        let date = &row.date;
        let campus = date.campus.as_ref().map(|campus| campus.code());
        (date.event_id.as_str(), row.ord, date.first_date.as_deref(), date.start_time.as_deref(), campus)
    }
    assert_eq!(
        sittings.iter().map(sitting).collect::<Vec<_>>(),
        [
            ("148689", Some(1), Some("2027-03-12"), Some("11:00"), Some("zentralcampus")),
            ("150664", Some(1), Some("2027-03-12"), Some("11:00"), Some("senftenberg")),
        ]
    );
    assert_eq!(queries::semester_schedule(&pinned, "2026W").unwrap().len(), 4330);
    assert_eq!(queries::semester_exams(&pinned, "2026W").unwrap().len(), 911);
    assert_eq!(queries::semester_date_counts(&pinned, "2026W").unwrap().len(), 478);
    assert_eq!(queries::semester_teaching_sws(&pinned, "2026W").unwrap().len(), 2537);
    let sws: Vec<(String, String, f64)> = queries::modules_teaching_sws(&pinned, &fs1)
        .unwrap()
        .into_iter()
        .map(|row| (row.module_id, row.form.code().to_string(), row.sws))
        .collect();
    let stated = [
        ("11112", "exercise", 2.0),
        ("11112", "lecture", 4.0),
        ("12102", "lecture", 1.0),
        ("12102", "practical", 2.0),
        ("12104", "exercise", 2.0),
        ("12104", "lecture", 4.0),
        ("12107", "exercise", 1.0),
        ("12107", "lecture", 3.0),
    ]
    .map(|(id, form, sws)| (id.to_string(), form.to_string(), sws));
    assert_eq!(sws, stated);

    // What the week grid names them by within Informatik, and the rooms as students read them.
    let abbrevs: Vec<(String, String)> =
        queries::modules_abbrevs(&pinned, &fs1, Some("079-82-2008")).unwrap().into_iter().map(|a| (a.module_id, a.abbrev)).collect();
    let said = [("11112", "MIT1"), ("12102", "PP"), ("12104", "EvS"), ("12107", "EEG")];
    assert_eq!(abbrevs, said.map(|(id, abbrev)| (id.to_string(), abbrev.to_string())));
    let rooms: BTreeSet<(Option<String>, Option<String>)> =
        queries::modules_schedule(&pinned, &fs1, "2026W").unwrap().into_iter().map(|row| (row.date.room, row.date.room_short)).collect();
    let vg = (Some("Verfügungsgebäude 1C - 0.03 - Zentralcampus".to_string()), Some("VG1C/0.03".to_string()));
    assert!(rooms.contains(&vg) && rooms.iter().all(|(room, short)| room.is_some() == short.is_some()), "{rooms:?}");
}

/// The queries follow every migration of Radix, and the snapshot of the tests has them all. A
/// migration that `SCHEMA_VERSION` did not follow would let browsers keep a copy the queries fail
/// on: `boot.js` only refuses what is older than `SCHEMA_VERSION`.
#[test]
fn the_queries_are_written_for_the_newest_schema() {
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../radix/internal/catalogdb/migrations");
    let newest = std::fs::read_dir(&migrations)
        .unwrap_or_else(|e| panic!("{}: {e}", migrations.display()))
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".sql")?.split_once('_')?.0.parse::<i64>().ok()
        })
        .max();
    assert_eq!(newest, Some(folia_model::SCHEMA_VERSION), "the newest migration of Radix is not the schema the queries are written for (folia_model::SCHEMA_VERSION)");
    let snapshot = open().schema_version().unwrap();
    assert!(snapshot >= folia_model::SCHEMA_VERSION, "the snapshot of the tests has schema {snapshot}, the queries are written for {}: export a new one", folia_model::SCHEMA_VERSION);
}

#[test]
fn catalog_filters_match_direct_sql() {
    let db = open();
    let program_id = queries::program_by_slug(&db, INFORMATIK_BSC).unwrap().expect("Informatik B.Sc.").id;
    let scope = |relation, plan_semester, kinds: Vec<KindFilter>| {
        Some(ProgramScope { program_slug: INFORMATIK_BSC.into(), relation, plan_semester, kinds, kinds_exclude: vec![], areas: vec![], semester_areas: vec![], semester_electives: false })
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
            "with published teaching events: what the list shows as „Termine“",
            CatalogQuery { scheduled: Some(true), ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE teaching_events > 0".into(),
        ),
        (
            "none published yet („noch keine“)",
            CatalogQuery { scheduled: Some(false), ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE teaching_events = 0".into(),
        ),
        (
            "Informatik B.Sc., third semester, with published teaching events",
            CatalogQuery {
                program: scope(ProgramRelation::Curricular, Some(PlanSemesterFilter::Semester(3)), vec![]),
                scheduled: Some(true),
                ..everything()
            },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular' WHERE pm.plan_semester = 3 AND f.teaching_events > 0"),
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
            "exclusions: no presentation, no seminar, not in English, not in summer, not irregular",
            CatalogQuery {
                exam_parts_exclude: vec![ExamPart::Presentation],
                teaching_forms_exclude: vec![TeachingForm::Seminar],
                languages_exclude: vec![Language::English],
                turnus: TurnusFilter { not_summer: true, not_irregular: true, ..Default::default() },
                ..everything()
            },
            "SELECT COUNT(*) FROM v_module_facets WHERE IFNULL(exam_presentation, 0) = 0 AND IFNULL(has_seminar, 0) = 0 \
             AND IFNULL(teaches_english, 0) = 0 AND IFNULL(offered_summer, 0) = 0 \
             AND (turnus_season IS NULL OR turnus_season != 'irregular')".into(),
        ),
        (
            "a written exam but no presentation",
            CatalogQuery { exam_parts: vec![ExamPart::Written], exam_parts_exclude: vec![ExamPart::Presentation], ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE exam_written = 1 AND IFNULL(exam_presentation, 0) = 0".into(),
        ),
        (
            "not in Senftenberg: modules without room data stay",
            CatalogQuery { campuses_exclude: vec![Campus::Senftenberg], ..everything() },
            "SELECT COUNT(*) FROM v_module_facets WHERE at_senftenberg IS NULL OR at_senftenberg = 0".into(),
        ),
        (
            "Informatik B.Sc. without compulsory modules: a module without a stated kind stays",
            CatalogQuery {
                program: Some(ProgramScope {
                    program_slug: INFORMATIK_BSC.into(),
                    kinds_exclude: vec![KindFilter::Stated(ModuleKind::Compulsory)],
                    ..Default::default()
                }),
                ..everything()
            },
            format!("SELECT COUNT(*) FROM {pm} AND pm.relation = 'curricular' WHERE pm.kind IS NULL OR pm.kind != 'compulsory'"),
        ),
        (
            "search text: a word of seven letters, in a title, inside a word or as an abbreviation",
            CatalogQuery { text: "Algebra".into(), ..everything() },
            "SELECT COUNT(*) FROM v_module_folded WHERE instr(title_de, 'algebra') > 0 OR instr(title_en, 'algebra') > 0 \
             OR instr(' ' || abbrevs || ' ', ' algebra ') > 0 OR instr(initials, 'algebra') > 0".into(),
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
        assert_eq!(queries::catalog_count(&db, winter_exercise).unwrap(), 984);
        assert_eq!(queries::catalog_count(&db, &CatalogQuery::default()).unwrap(), 2781 + 451);
    }
}

/// A semester of a program lists what the plan places there and what can be chosen for the
/// semester's requirement rows: the areas their names point at, else every unplaced elective.
#[test]
fn a_semester_lists_what_the_plan_asks_for() {
    let db = open();
    // Any program whose plan has a requirement row in a semester.
    let found = db.query("test", "SELECT p.slug, e.semester FROM v_program_plan_entry e JOIN v_program p ON p.id = e.program_id WHERE e.module_id IS NULL AND e.semester IS NOT NULL ORDER BY p.slug, e.semester LIMIT 1", &[]).unwrap();
    let Some(row) = found.rows.first() else {
        eprintln!("note: no plan of the snapshot has a requirement row in a semester; nothing to check");
        return;
    };
    let (slug, semester) = match row.as_slice() {
        [Value::Text(slug), Value::Integer(semester)] => (slug.clone(), u8::try_from(*semester).unwrap()),
        other => panic!("unexpected row {other:?}"),
    };
    let url = folia_routes::url::CatalogUrl {
        query: CatalogQuery {
            program: Some(ProgramScope { program_slug: slug.clone(), plan_semester: Some(PlanSemesterFilter::Semester(semester)), ..Default::default() }),
            ..Default::default()
        },
        ..Default::default()
    };
    let data = crate::catalog(&db, &url, folia_locale::Locale::De).unwrap();
    let plan = data.semester_plan.expect("the page says what the plan asks for");
    assert_eq!(plan.semester, semester);
    assert!(!plan.requirements.is_empty(), "the requirement row of the plan is part of it");
    let scope = data.effective.program.as_ref().unwrap();
    assert_eq!(scope.semester_areas, plan.area_ids());
    assert_eq!(scope.semester_electives, plan.any_elective());
    // The URL's query knows nothing of it; the page's does, and the total is that of the page's.
    assert!(url.query.program.as_ref().unwrap().semester_areas.is_empty());
    let program_id = queries::program_by_slug(&db, &slug).unwrap().unwrap().id;
    let placed = scalar(&db, &format!("SELECT COUNT(*) FROM v_program_module pm WHERE pm.program_id = '{program_id}' AND pm.relation = 'curricular' AND pm.plan_semester = {semester}"));
    let mut expected = format!("pm.plan_semester = {semester}");
    if !scope.semester_areas.is_empty() {
        let ids = scope.semester_areas.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        expected = format!("{expected} OR (pm.plan_semester IS NULL AND EXISTS (SELECT 1 FROM v_program_module_area a WHERE a.program_id = pm.program_id AND a.module_id = pm.module_id AND a.area_id IN ({ids})))");
    }
    if scope.semester_electives {
        expected = format!("{expected} OR (pm.plan_semester IS NULL AND IFNULL(pm.kind, '') NOT IN ('compulsory', 'thesis', 'internship'))");
    }
    let total = scalar(&db, &format!("SELECT COUNT(*) FROM v_module_facets f JOIN v_program_module pm ON pm.module_id = f.module_id AND pm.program_id = '{program_id}' AND pm.relation = 'curricular' WHERE ({expected})"));
    assert_eq!(data.page.total as i64, total, "the list is what the plan asks for");
    assert!(data.page.total as i64 >= placed, "at least what the plan places in the semester");
    assert_eq!(queries::catalog_count(&db, &data.effective).unwrap(), data.page.total, "further pages come with the same query");
    // The semesters the plan does not place anything in stay what they were.
    let unstated = folia_routes::url::CatalogUrl { query: CatalogQuery { program: Some(ProgramScope { program_slug: slug, plan_semester: Some(PlanSemesterFilter::Unstated), ..Default::default() }), ..Default::default() }, ..Default::default() };
    assert!(crate::catalog(&db, &unstated, folia_locale::Locale::De).unwrap().semester_plan.is_none());
}

#[test]
fn lecturer_and_bookmark_filters() {
    let db = open();
    let busiest = column(&db, "SELECT name FROM v_module_lecturer GROUP BY name ORDER BY COUNT(*) DESC, name LIMIT 4");
    let [name, b, c, d] = busiest.as_slice() else { panic!("four lecturers") };
    let name = name.clone();
    let teaches = scalar(
        &db,
        &format!("SELECT COUNT(DISTINCT module_id) FROM v_module_lecturer WHERE name = '{}'", name.replace('\'', "''")),
    ) as u64;
    let all = queries::catalog_count(&db, &everything()).unwrap();

    let include = CatalogQuery { lecturers_include: vec![name.clone()], ..everything() };
    let exclude = CatalogQuery { lecturers_exclude: vec![name.clone()], ..everything() };
    assert_eq!(queries::catalog_count(&db, &include).unwrap(), teaches);
    assert_eq!(queries::catalog_count(&db, &exclude).unwrap(), all - teaches);

    // (A or B) and not (C or D): wanted persons are alternatives, unwanted ones all have to be absent.
    let quote = |name: &str| format!("'{}'", name.replace('\'', "''"));
    let expected = scalar(
        &db,
        &format!(
            "SELECT COUNT(*) FROM v_module_facets f WHERE \
             EXISTS (SELECT 1 FROM v_module_lecturer l WHERE l.module_id = f.module_id AND l.name IN ({}, {})) \
             AND NOT EXISTS (SELECT 1 FROM v_module_lecturer l WHERE l.module_id = f.module_id AND l.name IN ({}, {}))",
            quote(&name), quote(b), quote(c), quote(d)
        ),
    ) as u64;
    let either = CatalogQuery { lecturers_include: vec![name.clone(), b.clone()], ..everything() };
    let mixed = CatalogQuery { lecturers_exclude: vec![c.clone(), d.clone()], ..either.clone() };
    assert!(queries::catalog_count(&db, &either).unwrap() > teaches, "a second wanted person adds modules");
    assert_eq!(queries::catalog_count(&db, &mixed).unwrap(), expected);

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

    // „ohne Gemerkte": the rest of the same list.
    let ids: Vec<String> = queries::catalog_page(&db, &everything(), 0, 4).unwrap().rows.into_iter().map(|r| r.id).collect();
    let without = CatalogQuery { without_ids: ids.clone(), ..everything() };
    assert_eq!(queries::catalog_count(&db, &without).unwrap(), all - 4);
    assert!(!queries::catalog_page(&db, &without, 0, 50).unwrap().rows.iter().any(|row| ids.contains(&row.id)));

    // The switch alone (a page that does not know the marks) matches nothing, never everything.
    let only_marked = CatalogQuery { marked: Some(true), ..everything() };
    assert_eq!(queries::catalog_count(&db, &only_marked).unwrap(), 0);
    let filled = CatalogQuery { only_ids: Some(ids.clone()), ..only_marked.clone() };
    assert_eq!(queries::catalog_count(&db, &filled).unwrap(), 4);
    let unmarked = CatalogQuery { marked: Some(false), without_ids: ids, ..everything() };
    assert_eq!(queries::catalog_count(&db, &unmarked).unwrap(), all - 4);
}

/// The page of the marked modules: whatever is marked is listed (also what is no longer offered),
/// in the order of marking unless another one is asked for, and what the snapshot does not know
/// is named instead of dropped.
#[test]
fn the_marked_modules_page() {
    use folia_routes::url::{BookmarkSort, Season};

    use crate::BookmarksData;

    let db = open();
    let pick = |sql: &str| column(&db, sql);
    let winter = pick("SELECT module_id FROM v_module_facets WHERE offered_winter = 1 AND IFNULL(offered_summer, 0) = 0 AND credits IS NOT NULL ORDER BY module_id LIMIT 2");
    let summer = pick("SELECT module_id FROM v_module_facets WHERE offered_summer = 1 AND IFNULL(offered_winter, 0) = 0 AND credits IS NOT NULL ORDER BY module_id LIMIT 1");
    let gone = pick("SELECT module_id FROM v_module_facets WHERE offer_status = 'not_offered' ORDER BY module_id LIMIT 1");
    assert_eq!((winter.len(), summer.len(), gone.len()), (2, 1, 1), "the snapshot has modules of every kind the test needs");

    // Newest mark first; an id nobody knows, one that is no id at all, and one marked twice.
    let marked: Vec<String> = [gone[0].as_str(), "99999999", summer[0].as_str(), "1 OR 1=1", winter[1].as_str(), winter[0].as_str(), summer[0].as_str()].iter().map(|id| id.to_string()).collect();
    let known: Vec<String> = [&gone[0], &summer[0], &winter[1], &winter[0]].iter().map(|id| id.to_string()).collect();
    let ids = |data: &BookmarksData| data.rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();

    let data = pages::bookmarks(&db, &marked, BookmarkSort::Added, false).unwrap();
    assert_eq!(ids(&data), known, "the order of marking, and the module that is no longer offered stays on the list");
    assert_eq!(data.missing, vec!["99999999".to_string()]);

    // The halves of the year are those of the catalog's turnus filter.
    let quoted = known.iter().map(|id| format!("'{id}'")).collect::<Vec<_>>().join(", ");
    for (season, column_name) in [(Season::Winter, "offered_winter"), (Season::Summer, "offered_summer")] {
        let expected: BTreeSet<String> = column(&db, &format!("SELECT module_id FROM v_module_facets WHERE {column_name} = 1 AND module_id IN ({quoted})")).into_iter().collect();
        let offered: BTreeSet<String> = known.iter().filter(|id| data.offered_in(season, id)).cloned().collect();
        assert_eq!(offered, expected, "{column_name}");
    }
    assert!(data.offered_in(Season::Winter, &winter[0]) && !data.offered_in(Season::Summer, &winter[0]) && data.offered_in(Season::Summer, &summer[0]));
    assert!(!data.offered_in(Season::Winter, "99999999"));

    // Another order comes from the same query as the catalog's.
    let by_credits = pages::bookmarks(&db, &marked, BookmarkSort::Credits, true).unwrap();
    let credits: Vec<f64> = by_credits.rows.iter().filter_map(|row| row.credits).collect();
    assert!(credits.windows(2).all(|pair| pair[0] >= pair[1]), "descending credits: {credits:?}");
    assert_eq!((by_credits.rows.len(), by_credits.missing.clone(), by_credits.winter.len()), (4, data.missing.clone(), data.winter.len()));
    // The order of marking has one direction.
    assert_eq!(ids(&pages::bookmarks(&db, &marked, BookmarkSort::Added, true).unwrap()), known);

    // Nothing marked: nothing listed, and no query that matches everything.
    assert_eq!(pages::bookmarks(&db, &[], BookmarkSort::Added, false).unwrap(), BookmarksData::default());
    assert_eq!(pages::bookmarks(&db, &["1 OR 1=1".to_string()], BookmarkSort::Title, false).unwrap(), BookmarksData::default());
}

/// The search takes words, never patterns: a wildcard of `LIKE` alone finds nothing, and next to
/// a word it parts words like any other mark.
#[test]
fn search_text_is_words() {
    let db = open();
    let count = |text: &str| queries::catalog_count(&db, &CatalogQuery { text: text.into(), ..everything() }).unwrap();
    for wildcard in ["%", "_", "\\"] {
        assert_eq!(count(wildcard), 0, "{wildcard:?} alone finds nothing");
        assert_eq!(count(&format!("Algebra{wildcard}")), count("Algebra"), "{wildcard:?} next to a word");
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
    // The catalog moves to a semester as soon as its schedule is published, even
    // while the old one runs out (radix: minModulesOfAPublishedSchedule).
    assert_eq!(queries::meta(&db).unwrap().current_semester.as_deref(), Some("2026W"));

    let count = |query: CatalogQuery| queries::catalog_count(&db, &query).unwrap();
    assert_eq!(count(everything()), 4936);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::Active]), ..everything() }), 2781);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::NotOffered]), ..everything() }), 1704);
    assert_eq!(count(CatalogQuery { offer: Some(vec![OfferStatus::PhaseOut]), ..everything() }), 451);
    assert_eq!(count(CatalogQuery { languages: vec![Language::German], ..everything() }), 3950);
    assert_eq!(count(CatalogQuery { languages: vec![Language::English], ..everything() }), 986);
    assert_eq!(count(CatalogQuery { graded: Some(true), ..everything() }), 4654);
    assert_eq!(count(CatalogQuery { graded: Some(false), ..everything() }), 282);
    assert_eq!(count(CatalogQuery { fues: Some(true), ..everything() }), 288);
    let winter = TurnusFilter { winter: true, ..Default::default() };
    let summer = TurnusFilter { summer: true, ..Default::default() };
    let irregular = TurnusFilter { irregular: true, ..Default::default() };
    assert_eq!(count(CatalogQuery { turnus: winter, ..everything() }), 1965 + 607);
    assert_eq!(count(CatalogQuery { turnus: summer, ..everything() }), 1671 + 607);
    assert_eq!(count(CatalogQuery { turnus: irregular, ..everything() }), 693);
    // The winter schedule reached the catalog on 2026-09-21; before it, only the
    // modules with a room in the summer semester were placed (234).
    let campus_known = scalar(&db, "SELECT COUNT(*) FROM v_module_facets WHERE at_zentralcampus IS NOT NULL");
    assert_eq!(campus_known, 1119);
}

#[test]
fn every_enum_code_has_a_label() {
    let db = open();
    let enums = labels::code_sets();
    let covered = |codes: &[String]| enums.iter().any(|(_, known)| codes.iter().all(|c| known.contains(&c.as_str())));

    // Columns that never reach a page.
    let internal = ["detail_status", "page_lang", "degree_label_basis", "description_source"];

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

/// The phone's filter sheet counts with `catalog_summary` and loads the list with `catalog` when
/// it closes: the two must say the same about a filter, the semester of a plan (whose query the
/// loader fills in) and a program's FÜS list included.
#[test]
fn the_summary_of_a_filter_is_what_its_page_says() {

    use folia_routes::url::CatalogUrl;

    let db = open();
    let searches = [
        String::new(),
        "turnus=winter&form=lecture&exam=written".to_string(),
        format!("program={INFORMATIK_BSC}"),
        format!("program={INFORMATIK_BSC}&list=fues"),
        format!("program={INFORMATIK_BSC}&semester=1"),
        format!("program={INFORMATIK_BSC}&semester=none&turnus=summer"),
        "program=no-such-program".to_string(),
        "q=informatik".to_string(),
        format!("q=python&program={INFORMATIK_BSC}"),
        "q=algoritmen".to_string(),
        "q=algorithmen+xqzvw".to_string(),
    ];
    for search in searches {
        let url = CatalogUrl::parse(&search);
        let page = pages::catalog(&db, &url, folia_locale::Locale::De).unwrap();
        let summary = pages::catalog_summary(&db, &url.query).unwrap();
        assert_eq!(summary.total, page.page.total, "{search}: the total");
        assert_eq!(
            (summary.program, summary.curricular_total, summary.fues_total, summary.plan_semesters, summary.areas),
            (page.program, page.curricular_total, page.fues_total, page.plan_semesters, page.areas),
            "{search}: what the panel shows"
        );
    }
}

#[test]
fn page_loaders_return_everything_a_page_shows() {

    use folia_routes::url::CatalogUrl;

    let db = open();
    let started = std::time::Instant::now();

    let overview = pages::overview(&db).unwrap();
    assert!(overview.current_semester.is_some() && overview.modules > 0 && overview.programs > 0);
    // The start page names how many of the current programs have a checked plan.
    assert!(overview.plans > 0 && overview.plans <= overview.programs, "{} of {}", overview.plans, overview.programs);
    let ground = pages::ground(&db).unwrap();
    assert_eq!((&ground.meta, &ground.current_semester), (&overview.meta, &overview.current_semester));

    let url = CatalogUrl::parse(&format!("program={INFORMATIK_BSC}&list=fues"));
    let catalog = pages::catalog(&db, &url, folia_locale::Locale::De).unwrap();
    let program = catalog.program.as_ref().expect("the selected program");
    assert_eq!(catalog.curricular_total, Some(program.curricular_modules as u64));
    assert_eq!(catalog.fues_total, Some(program.fues_modules as u64));
    assert_eq!(Some(catalog.page.total), catalog.fues_total);
    assert!(!catalog.departments.is_empty());
    let choices = pages::catalog_choices(&db).unwrap();
    assert!(!choices.departments.is_empty() && !choices.programs.is_empty());
    assert!(choices.lecturers.iter().any(|l| l.title.is_some()) && choices.lecturers.iter().any(|l| l.title.is_none()));

    // A program that does not exist selects nothing; it is not an error.
    let unknown = pages::catalog(&db, &CatalogUrl::parse("program=no-such-program"), folia_locale::Locale::De).unwrap();
    assert_eq!((unknown.program, unknown.page.total), (None, 0));

    let module_id = &catalog.page.rows[0].id;
    let module = pages::module(&db, module_id).unwrap().expect("module page");
    assert!(module.programs.iter().any(|link| link.program_slug.as_deref() == Some(INFORMATIK_BSC)));
    assert_eq!(pages::module(&db, "00000").unwrap(), None);

    // Faculties are derived: most programs get one, none gets two, and the rest stays unknown.
    let overview = pages::programs_overview(&db).unwrap();
    let current: Vec<_> = overview.programs.iter().filter(|p| p.is_latest_po).collect();
    let with_faculty = current.iter().filter(|p| overview.faculties.iter().any(|f| f.program_id == p.id)).count();
    assert!(with_faculty * 10 >= current.len() * 9 && with_faculty < current.len(), "{with_faculty} of {}", current.len());
    let mut ids: Vec<&String> = overview.faculties.iter().map(|f| &f.program_id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), overview.faculties.len(), "one faculty per program");
    let informatik = overview.programs.iter().find(|p| p.slug == INFORMATIK_BSC).unwrap();
    let faculty = overview.faculties.iter().find(|f| f.program_id == informatik.id).expect("Informatik has a faculty");
    let department = overview.departments.iter().find(|d| d.id == faculty.department_id).unwrap();
    assert_eq!(department.code, "1", "{department:?}");
    assert!(overview.faculties.iter().any(|f| f.basis == pages::FacultyBasis::Thesis));
    assert!(overview.faculties.iter().any(|f| f.basis == pages::FacultyBasis::Majority));
    // A faculty taken from the programs of the same subject is checked on its own
    // (`pages::faculty_tests`): since a thesis is what its name says, no current program needs it.

    let page = pages::program(&db, INFORMATIK_BSC).unwrap().expect("program page");
    assert_eq!(page.curricular.len() as i64, page.program.curricular_modules);
    assert_eq!(page.plan.is_some(), page.program.has_plan);
    assert_eq!(pages::program(&db, "no-such-program").unwrap(), None);

    eprintln!("all page loaders: {:?}", started.elapsed());
}

#[test]
fn the_program_map_is_stable_tidy_and_on_the_sheet() {

    let db = open();
    let started = std::time::Instant::now();
    let map = pages::program_map(&db).unwrap();
    eprintln!("program map: {:?}, {} programs, {} links", started.elapsed(), map.programs.len(), map.links.len());
    if let Ok(path) = std::env::var("FOLIA_MAP_DUMP") {
        std::fs::write(path, serde_json::to_vec(&map).unwrap()).unwrap();
    }
    assert_eq!(map, pages::program_map(&db).unwrap(), "the same snapshot gives the same map");

    let current = queries::programs(&db).unwrap().iter().filter(|p| p.is_latest_po).count();
    assert_eq!(map.programs.len(), current);
    assert!(map.links.len() > current, "{} links", map.links.len());
    assert!(map.links.iter().all(|l| l.a < l.b && l.b < current && l.shared > 0 && l.similarity > 0.0 && l.similarity <= 1.0));
    // Informatik B.Sc. is related to somebody, its closest relative first.
    let informatik = map.programs.iter().position(|p| p.slug == INFORMATIK_BSC);
    assert!(informatik.is_none_or(|i| map.programs[i].modules > 10));

    // The six faculties, by number, and nearly every program in one of them.
    let codes: Vec<&str> = map.faculties.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["1", "2", "3", "4", "5", "6"]);
    assert!(map.faculties.iter().all(|f| !f.name.is_empty() && !f.name.contains(" - ")), "{:?}", map.faculties);
    let placed = map.programs.iter().filter(|p| p.faculty.is_some()).count();
    assert!(placed * 10 >= current * 9, "{placed} of {current} programs have a faculty");

    for layout in [&map.wide, &map.tall] {
        assert_eq!(layout.regions.iter().map(|r| r.faculty).collect::<Vec<_>>(), (0..map.faculties.len()).collect::<Vec<_>>());
        for region in &layout.regions {
            assert!(region.path.starts_with('M') && region.path.ends_with('Z'), "{region:?}");
            assert!(region.x > 0.0 && region.x < layout.width && region.y > 0.0 && region.y < layout.height, "{region:?}");
        }
        assert_eq!(layout.dots.len(), current);
        for (i, a) in layout.dots.iter().enumerate() {
            assert!(a.0 - a.2 >= 0.0 && a.0 + a.2 <= layout.width && a.1 - a.2 >= 0.0 && a.1 + a.2 <= layout.height, "dot {i} leaves the sheet: {a:?}");
            for b in layout.dots.iter().skip(i + 1) {
                let distance = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(distance >= a.2 + b.2, "dots overlap: {a:?} {b:?}");
            }
        }
        assert!(layout.names.len() >= 8, "{} names", layout.names.len());
    }

    let home = pages::home(&db, &[CatalogQuery::default(), everything()]).unwrap();
    assert_eq!(home.entry_counts.len(), 2);
    assert!(home.entry_counts[0] > 0 && home.entry_counts[0] < home.entry_counts[1]);
    assert_eq!(home.faculties.iter().map(|(_, programs)| programs).sum::<usize>(), current);
    assert!(home.faculties.iter().filter(|(department, _)| department.is_some()).count() >= 4);
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

/// Several areas list the modules of any of them: what a row of the plan that means several
/// areas opens in the catalog („Anwendungsfach": the Nebenfächer).
#[test]
fn several_areas_list_the_modules_of_any_of_them() {
    let db = open();
    let program_id = queries::program_by_slug(&db, INFORMATIK_BSC).unwrap().expect("Informatik B.Sc.").id;
    let areas = folia_plans::areas::catalog_areas(&queries::program_areas(&db, &program_id).unwrap(), &queries::program_area_tree(&db, &program_id).unwrap());
    let physik = areas.iter().find(|area| area.label == "Physik").expect("Physik").id;
    let mathe = areas.iter().find(|area| area.label == "Mathematik").expect("Mathematik").id;
    let count = |ids: Vec<i64>| {
        let query = CatalogQuery {
            program: Some(ProgramScope { program_slug: INFORMATIK_BSC.into(), areas: ids, ..Default::default() }),
            offer: Some(OfferStatus::ALL.to_vec()),
            ..Default::default()
        };
        queries::catalog_count(&db, &query).unwrap()
    };
    let (one, other, both) = (count(vec![physik]), count(vec![mathe]), count(vec![physik, mathe]));
    let direct = scalar(
        &db,
        &format!("SELECT COUNT(DISTINCT module_id) FROM v_program_module_area WHERE program_id = '{program_id}' AND area_id IN ({physik}, {mathe})"),
    ) as u64;
    assert!(one > 0 && other > 0, "{one} {other}");
    assert_eq!(both, direct, "the modules of either area, each once");
    assert_eq!(count(vec![mathe, physik]), both);
}

/// How a page reads the exam dates of the whole snapshot (`exam_reading`): it never states a time
/// or a date the source does not, and what lies outside 06:00–22:00 is either read (placeholder,
/// deadline) or marked. The numbers go to stderr for the report; they move with the data.
#[test]
fn exam_readings_never_state_what_the_source_does_not() {
    use std::collections::BTreeMap;

    use folia_timetable::exam_reading::{self, Reason};

    let db = open();
    let semesters = queries::semesters(&db).unwrap();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for id in column(&db, "SELECT DISTINCT module_id FROM v_module_exam") {
        for date in queries::module_exams(&db, &id).unwrap() {
            let semester = semesters.iter().find(|semester| semester.key == date.semester_key);
            let reading = exam_reading::read(&date, semester);
            let (stated, shown) = (&reading.stated, &reading.shown);
            for (shown, stated) in [(&shown.start_time, &stated.start_time), (&shown.end_time, &stated.end_time), (&shown.first_date, &stated.first_date), (&shown.last_date, &stated.last_date)] {
                assert!(shown.is_none() || shown == stated, "{id}: {reading:?}");
            }
            assert!(shown.weekday.is_none() || shown.weekday == stated.weekday, "{id}: {reading:?}");
            assert_eq!(reading.reasons.is_empty(), shown == stated, "{id}: {reading:?}");

            let minutes = |time: &Option<String>| time.as_deref().and_then(|t| Some(t.get(..2)?.parse::<i64>().ok()? * 60 + t.get(3..5)?.parse::<i64>().ok()?));
            let outside = [minutes(&stated.start_time), minutes(&stated.end_time)].into_iter().flatten().any(|m| !(360..=1320).contains(&m));
            if outside {
                assert!(reading.has(Reason::PlaceholderTime) || reading.has(Reason::Deadline) || reading.has(Reason::UnusualTime), "{id}: {reading:?}");
            }
            if reading.has(Reason::PlaceholderTime) {
                assert_eq!((&shown.start_time, &shown.end_time, shown.weekday), (&None, &None, None), "{id}");
            }
            *counts.entry(format!("{:?}", reading.reasons)).or_default() += 1;
        }
    }
    eprintln!("exam readings by reasons (rows of v_module_exam, per module): {counts:#?}");

    // Analysis I: two placeholders dated 27.12.2015 in the WiSe 2026/27, shown without either.
    let semester = semesters.iter().find(|semester| semester.key == "2026W");
    let analysis: Vec<_> = queries::module_exams(&db, "11103").unwrap().into_iter().filter(|date| date.semester_key == "2026W").collect();
    if analysis.iter().any(|date| date.first_date.as_deref() == Some("2015-12-27")) {
        for date in &analysis {
            let reading = exam_reading::read(date, semester);
            assert_eq!(reading.reasons, [Reason::PlaceholderTime, Reason::PlaceholderDate]);
            assert_eq!(reading.shown, exam_reading::Slot::default());
            assert_eq!(reading.stated.start_time.as_deref(), Some("01:00"));
        }
    } else {
        eprintln!("note: 11103 no longer carries the 2015 placeholder; its reading is not asserted");
    }
}

/// The search of the catalog (`search`, docs/folia/frontend.md „Search“), against the snapshot: it
/// finds words in any order, folded, in the names of the modules.
#[test]
fn the_search_folds_and_takes_words_in_any_order() {
    let db = open();
    let count = |text: &str| queries::catalog_count(&db, &CatalogQuery { text: text.into(), ..everything() }).unwrap();
    // LIKE found none of the modules on „Ökologie“ for „okologie“, and 5 of 16 for „ökologie“.
    assert!(count("okologie") > 0);
    for same in ["Ökologie", "ÖKOLOGIE", "ökologie"] {
        assert_eq!(count(same), count("okologie"), "{same}");
    }
    assert_eq!(count("strasse"), count("Straße"));
    assert!(count("Maschinelles Lernen") > 0);
    assert_eq!(count("lernen maschinelles"), count("Maschinelles Lernen"));
    assert_eq!(count("Mathematik für Ingenieure"), count("mathematik ingenieure"), "fillers do not count");
    assert!(count("Analysis I") > 0);
    assert_eq!(count("Analysis 1"), count("Analysis I"), "a number is its Roman numeral");
    // „ki“ is too short to be found inside a word („Schlüsselqualifikationen“, „Kinetik“ is the start of one).
    let ki = scalar(
        &db,
        "SELECT COUNT(*) FROM v_module_folded WHERE instr(' ' || title_de, ' ki') > 0 OR instr(' ' || title_en, ' ki') > 0 \
         OR instr(' ' || abbrevs || ' ', ' ki ') > 0 OR instr(initials, 'ki') > 0",
    ) as u64;
    assert_eq!(count("KI"), ki);
    // The known short form of a word of a title (radix/internal/abbrev: BWL) finds every title with the
    // word; three letters are found as a word, the start of one, an abbreviation or initials.
    let bwl = scalar(
        &db,
        "SELECT COUNT(*) FROM v_module_folded WHERE instr(' ' || title_de, ' bwl') > 0 OR instr(' ' || title_en, ' bwl') > 0 \
         OR instr(' ' || abbrevs || ' ', ' bwl ') > 0 OR instr(initials, 'bwl') > 0",
    ) as u64;
    let titled = scalar(&db, "SELECT COUNT(*) FROM v_module_folded WHERE instr(title_de, 'betriebswirtschaftslehre') > 0") as u64;
    assert!(titled > 0 && bwl >= titled);
    assert_eq!(count("BWL"), bwl);
    let with_word = CatalogQuery { text: "BWL".into(), only_ids: Some(column(&db, "SELECT module_id FROM v_module_folded WHERE instr(title_de, 'betriebswirtschaftslehre') > 0")), ..everything() };
    assert_eq!(queries::catalog_count(&db, &with_word).unwrap(), titled);
}

/// The best match comes first while nothing else is chosen to order by: the module of the number,
/// the one of the abbreviation, then whole words before words that only contain the query.
#[test]
fn the_search_orders_by_relevance() {
    let db = open();
    let listed = |query: CatalogQuery| queries::catalog_page(&db, &query, 0, 400).unwrap().rows;
    let search = |text: &str| CatalogQuery { text: text.into(), ..everything() };

    let id = column(&db, "SELECT module_id FROM v_module_folded ORDER BY module_id LIMIT 1 OFFSET 100").remove(0);
    assert_eq!(listed(search(&id)).first().map(|row| row.id.clone()), Some(id.clone()), "a module by its number");
    let prefix: String = id.chars().take(3).collect();
    assert!(listed(search(&prefix)).iter().all(|row| row.id.starts_with(&prefix)), "a number from its start");

    let abbrev = column(&db, &format!("SELECT abbrev FROM v_module WHERE id = '{id}'")).remove(0);
    let first = listed(search(&abbrev)).first().map(|row| row.id.clone()).unwrap();
    let folded = folia_search::words(&abbrev).concat();
    assert!(
        scalar(&db, &format!("SELECT COUNT(*) FROM v_module_folded WHERE module_id = '{first}' AND instr(' ' || abbrevs || ' ', ' {folded} ') > 0")) == 1,
        "{abbrev}: the first module has the abbreviation"
    );

    // „informatik“: every title with the word comes before every title that has it inside a word.
    let whole = |row_id: &str| {
        scalar(
            &db,
            &format!("SELECT COUNT(*) FROM v_module_folded WHERE module_id = '{row_id}' AND (instr(' ' || title_de || ' ', ' informatik ') > 0 OR instr(' ' || title_en || ' ', ' informatik ') > 0)"),
        ) == 1
    };
    let order: Vec<bool> = listed(search("informatik")).iter().map(|row| whole(&row.id)).collect();
    assert!(order.contains(&true) && order.contains(&false), "{order:?}");
    assert!(order.windows(2).all(|pair| pair[0] || !pair[1]), "whole words first: {order:?}");

    // A column chosen to order by orders the matches.
    let by_title = listed(CatalogQuery { sort: SortKey::Title, ..search("informatik") });
    assert!(by_title.windows(2).all(|pair| pair[0].title.to_lowercase() <= pair[1].title.to_lowercase()), "by title");
    assert!(CatalogQuery { text: "informatik".into(), ..Default::default() }.by_relevance());
    assert!(!CatalogQuery { text: "informatik".into(), sort: SortKey::Credits, ..Default::default() }.by_relevance());
    assert!(!CatalogQuery { text: "%".into(), ..Default::default() }.by_relevance());
    assert!(!CatalogQuery::default().by_relevance());
}

/// Where the words as typed find nothing, the page searches for the word of a title a typo stands
/// for, and else for the most of the words; the page and the filter panel agree.
#[test]
fn the_search_corrects_typos_and_falls_back_to_the_most_words() {

    use folia_routes::url::CatalogUrl;
    use folia_search::{Correction, Resolution};

    let db = open();
    let page = |search: &str| pages::catalog(&db, &CatalogUrl::parse(search), folia_locale::Locale::De).unwrap();
    let count = |text: &str| queries::catalog_count(&db, &CatalogQuery { text: text.into(), ..Default::default() }).unwrap();

    let typo = page("q=algoritmen");
    assert_eq!(
        typo.effective.text_resolution,
        Some(Resolution { corrected: vec![Correction { typed: "algoritmen".into(), word: "algorithmen".into(), shown: "Algorithmen".into() }], most_words: false })
    );
    assert_eq!(typo.page.total, count("algorithmen"));
    assert!(typo.page.total > 0);

    // A word no title knows, next to one that finds modules: the modules with that one.
    let most = page("q=algorithmen+xqzvw");
    assert_eq!(most.effective.text_resolution, Some(Resolution { corrected: vec![], most_words: true }));
    assert_eq!(most.page.total, count("algorithmen"));

    // As typed where that finds something, and a text that finds nothing stays as it is.
    assert_eq!(page("q=algorithmen").effective.text_resolution, Some(Resolution::default()));
    let nothing = page("q=xqzvw");
    assert_eq!((nothing.effective.text_resolution, nothing.page.total), (Some(Resolution::default()), 0));
    assert_eq!(page("").effective.text_resolution, None);
}

/// Under the list: what the search finds outside its filters, offered and no longer offered.
/// „Ähnliche Module" (`pages::similar`, owner, 2026-10-01): of the semantic search's hits, in
/// their order, those the filters hold besides the text, without the results, as the list's rows.
#[test]
fn the_similar_modules_are_the_hits_the_filters_hold_besides_the_results() {

    use folia_model::rows::CatalogRow;
    use folia_routes::url::CatalogUrl;

    let db = open();
    let ids = |rows: Vec<CatalogRow>| rows.into_iter().map(|row| row.id).collect::<Vec<_>>();
    let found: BTreeSet<String> = ids(queries::catalog_page(&db, &CatalogQuery { text: "python".into(), ..everything() }, 0, 10_000).unwrap().rows).into_iter().collect();
    let besides = |status: &str| -> Vec<String> {
        column(&db, &format!("SELECT module_id FROM v_module_facets WHERE offer_status = '{status}' ORDER BY module_id"))
            .into_iter()
            .filter(|id| !found.contains(id))
            .collect()
    };
    let (offered, not_offered) = (besides("active"), besides("not_offered"));
    let data = pages::catalog(&db, &CatalogUrl::parse("q=python"), folia_locale::Locale::De).unwrap();
    let query = data.effective.clone();
    let result = data.page.rows.first().expect("python finds a module").id.clone();
    let hits = vec![offered[0].clone(), result.clone(), not_offered[0].clone(), offered[1].clone(), offered[2].clone()];

    // The catalog lists offered modules: those of the hits, in their order, without the result.
    assert_eq!(ids(pages::similar(&db, &query, &hits, 10).unwrap()), [&offered[..3]].concat());
    assert_eq!(ids(pages::similar(&db, &query, &hits, 2).unwrap()), [&offered[..2]].concat());
    // Every module: the one no longer offered as well.
    let all = CatalogQuery { offer: Some(OfferStatus::ALL.to_vec()), ..query.clone() };
    assert_eq!(ids(pages::similar(&db, &all, &hits, 10).unwrap()), [offered[0].clone(), not_offered[0].clone(), offered[1].clone(), offered[2].clone()]);
    // „Gemerkt": the marked ones among them.
    let marked = CatalogQuery { marked: Some(true), only_ids: Some(vec![offered[1].clone(), result.clone()]), ..query.clone() };
    assert_eq!(ids(pages::similar(&db, &marked, &hits, 10).unwrap()), [offered[1].clone()]);
    // The address's query, whose text is not resolved yet, has the same.
    assert_eq!(ids(pages::similar(&db, &CatalogUrl::parse("q=python").query, &hits, 10).unwrap()), [&offered[..3]].concat());
    // No search, or one of fewer than three letters or digits: nothing.
    assert!(pages::similar(&db, &CatalogQuery::default(), &hits, 10).unwrap().is_empty());
    assert!(pages::similar(&db, &CatalogQuery { text: "py".into(), ..Default::default() }, &hits, 10).unwrap().is_empty());
    assert!(!pages::searches_similar("C++") && pages::searches_similar("BWL") && pages::searches_similar(" ki 2 "));
    // The semantic search is asked for the text as the list searched it.
    let text = |address: &str| pages::similar_text(&pages::catalog(&db, &CatalogUrl::parse(address), folia_locale::Locale::De).unwrap().effective);
    assert_eq!(text("q=python"), Some("python".to_string()));
    assert_eq!(text("q=algoritmen%20graphen"), Some("algorithmen graphen".to_string()));
    assert_eq!(text("q=py"), None);

    // Inside a program: its modules alone, each the row its list has.
    let program = pages::catalog(&db, &CatalogUrl::parse(&format!("q=python&program={INFORMATIK_BSC}")), folia_locale::Locale::De).unwrap();
    let curriculum = queries::catalog_page(&db, &CatalogQuery { text: String::new(), text_resolution: None, ..program.effective.clone() }, 0, 10_000).unwrap().rows;
    let inside: Vec<CatalogRow> = curriculum.iter().filter(|row| !found.contains(&row.id)).take(2).cloned().collect();
    let outside = offered.iter().find(|id| !curriculum.iter().any(|row| &row.id == *id)).expect("a module outside the program");
    let hits = vec![outside.clone(), inside[1].id.clone(), inside[0].id.clone()];
    assert_eq!(pages::similar(&db, &program.effective, &hits, 10).unwrap(), [inside[1].clone(), inside[0].clone()]);

    // A title borne by several offered modules (a module per program, an old and a new number):
    // the search for one by its number finds that one, and the others are not „Ähnliche Module"
    // (owner, 2026-10-02: what the search finds is not shown again); among the similar ones a
    // title stands once, the closest of its modules.
    let shared = column(
        &db,
        "SELECT m.title FROM v_module m JOIN v_module_facets f ON f.module_id = m.id WHERE f.offer_status IN ('active', 'phase_out') \
         GROUP BY m.title HAVING COUNT(*) > 1 ORDER BY m.title",
    );
    let bearing = |title: &str| -> Vec<String> {
        let ids = column(&db, &format!("SELECT f.module_id FROM v_module_facets f JOIN v_module m ON m.id = f.module_id WHERE f.offer_status IN ('active', 'phase_out') AND m.title = '{}' ORDER BY f.module_id", title.replace('\'', "''")));
        assert!(ids.len() > 1, "{title} is borne by several offered modules");
        ids
    };
    let (twins, more) = (bearing(&shared[0]), bearing(&shared[1]));
    let by_number = pages::catalog(&db, &CatalogUrl::parse(&format!("q={}", twins[0])), folia_locale::Locale::De).unwrap();
    let found = ids(by_number.page.rows.clone());
    assert!(found.contains(&twins[0]) && !found.contains(&twins[1]), "{} finds itself, not {} of the same title", twins[0], twins[1]);
    let hits = vec![twins[1].clone(), more[0].clone(), more[1].clone(), offered[0].clone()];
    assert!(!found.contains(&more[0]) && !found.contains(&offered[0]));
    assert_eq!(ids(pages::similar(&db, &by_number.effective, &hits, 10).unwrap()), [more[0].clone(), offered[0].clone()]);
    // Without that title among the results, the closest module of it stands there.
    let other = pages::catalog(&db, &CatalogUrl::parse("q=python"), folia_locale::Locale::De).unwrap();
    assert_eq!(ids(pages::similar(&db, &other.effective, &[twins[1].clone(), twins[0].clone()], 10).unwrap()), [twins[1].clone()]);
}

#[test]
fn the_search_says_what_it_finds_outside_the_filters() {

    use folia_routes::url::CatalogUrl;

    let db = open();
    let data = pages::catalog(&db, &CatalogUrl::parse(&format!("q=python&program={INFORMATIK_BSC}")), folia_locale::Locale::De).unwrap();
    let elsewhere = data.elsewhere.unwrap();
    let anywhere = folia_search::search_count(&db, "python", None).unwrap();
    assert_eq!(elsewhere.offered + elsewhere.not_offered + data.page.total, anywhere);
    let offered = queries::catalog_count(&db, &CatalogQuery { text: "python".into(), ..Default::default() }).unwrap();
    assert!(elsewhere.offered > 0 && elsewhere.offered <= offered);

    // The catalog without filters lists every offered module the text finds; the rest is not offered.
    let plain = pages::catalog(&db, &CatalogUrl::parse("q=python"), folia_locale::Locale::De).unwrap();
    assert_eq!(plain.elsewhere.unwrap().offered, 0);
    assert_eq!(plain.elsewhere.unwrap().not_offered + plain.page.total, anywhere);
    assert_eq!(pages::catalog(&db, &CatalogUrl::parse(""), folia_locale::Locale::De).unwrap().elsewhere, None);
}
