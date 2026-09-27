//! Every SQL statement of the web tier, one function per query.
//!
//! Rules (docs/frontend-rewrite.md §4): read only `v_*` views and `program_coverage`;
//! filter and sort on view columns; `LIKE` only against `v_module_search`; every list
//! comes with an exact total. `tests::every_query_runs_against_the_snapshot` fails for
//! a `pub fn` in this file that the tests never ran. The Studienplan's lists of module ids
//! go in as one parameter read by `json_each(?)`: a function over the parameter, which reads
//! no table.

use std::collections::BTreeSet;

use crate::db::{fetch, fetch_count, fetch_optional, Database, DbError, Value};
use crate::filter::{like_pattern, CatalogQuery, ProgramRelation};
use crate::rows::{
    CatalogPage, CatalogRow, Department, Meta, Module, Prerequisite, Program, ProgramModule, SearchTerm,
    Semester,
};
use crate::rows_detail::{
    AreaNode, AreaPlacement, Counterpart, DateCount, DateRow, Document, EventDate, Lecturer, LecturerName, ModuleAbbrev, ModuleSws,
    ModuleTeachingForm, Plan, PlanEntry, PlanPlace, PlanTotal, PlanTotalEntry, ProgramDepartmentCount, ProgramLink,
    ProgramVersion, Successor, TextItem,
};
use crate::url::is_module_id;

const PROGRAM_COLUMNS: &str = "id, slug, name, degree_level, study_variant, degree_label, degree_raw, \
     degree_display, po_version, po_year, family_key, name_key, is_latest_po, source_url, has_plan, plan_status, \
     curricular_modules, fues_modules, documents";

/// „Datenstand" and the current semester.
pub fn meta(db: &dyn Database) -> Result<Meta, DbError> {
    let rows = db.query("meta", "SELECT key, value FROM v_meta", &[])?;
    let mut meta = Meta::default();
    for row in &rows.rows {
        if let [Value::Text(key), Value::Text(value)] = row.as_slice() {
            let value = Some(value.clone());
            match key.as_str() {
                "built_at" => meta.built_at = value,
                "data_changed_at" => meta.data_changed_at = value,
                "current_semester" => meta.current_semester = value,
                "content_digest" => meta.content_digest = value,
                "radix_version" => meta.radix_version = value,
                _ => {}
            }
        }
    }
    Ok(meta)
}

pub fn semesters(db: &dyn Database) -> Result<Vec<Semester>, DbError> {
    fetch(
        db,
        "semesters",
        "SELECT key, season, year, label, starts_on, ends_on, is_current, teaching_events, exam_events \
         FROM v_semester ORDER BY key",
        &[],
    )
}

pub fn departments(db: &dyn Database) -> Result<Vec<Department>, DbError> {
    fetch(
        db,
        "departments",
        "SELECT id, code, label, name_de, name_en, modules FROM v_department ORDER BY label COLLATE NOCASE",
        &[],
    )
}

/// All programs, every PO version: the program overview and the program selector.
pub fn programs(db: &dyn Database) -> Result<Vec<Program>, DbError> {
    fetch(
        db,
        "programs",
        &format!("SELECT {PROGRAM_COLUMNS} FROM v_program ORDER BY name COLLATE NOCASE, degree_level, po_version DESC"),
        &[],
    )
}

pub fn program_by_slug(db: &dyn Database, slug: &str) -> Result<Option<Program>, DbError> {
    fetch_optional(
        db,
        "program_by_slug",
        &format!("SELECT {PROGRAM_COLUMNS} FROM v_program WHERE slug = ?"),
        &[Value::from(slug)],
    )
}

/// The curriculum of a program, or its FÜS list.
pub fn program_modules(
    db: &dyn Database,
    program_id: &str,
    relation: ProgramRelation,
) -> Result<Vec<ProgramModule>, DbError> {
    fetch(
        db,
        "program_modules",
        "SELECT module_id, relation, kind, kind_source, kind_basis, area, section, module_title, \
         module_credits, offer_status, turnus_season, plan_semester \
         FROM v_program_module WHERE program_id = ? AND relation = ? \
         ORDER BY plan_semester IS NULL, plan_semester, module_title COLLATE NOCASE, module_id",
        &[Value::from(program_id), Value::from(relation.code())],
    )
}

/// Which departments offer the curriculum of each program: the evidence `pages::faculties`
/// derives a program's faculty from (no source states it).
pub fn program_department_counts(db: &dyn Database) -> Result<Vec<ProgramDepartmentCount>, DbError> {
    fetch(
        db,
        "program_department_counts",
        "SELECT pm.program_id, f.department_id, \
                SUM(CASE WHEN pm.kind = 'thesis' THEN 1 ELSE 0 END) AS thesis_modules, \
                SUM(CASE WHEN f.offer_status != 'not_offered' THEN 1 ELSE 0 END) AS offered_modules \
         FROM v_program_module pm JOIN v_module_facets f ON f.module_id = pm.module_id \
         WHERE pm.relation = 'curricular' AND f.department_id IS NOT NULL \
         GROUP BY pm.program_id, f.department_id ORDER BY pm.program_id, f.department_id",
        &[],
    )
}

/// (program id, module id) for the curriculum of every current program: what the map of the
/// programs (`graph::program_map`) measures their kinship with.
pub fn curriculum_links(db: &dyn Database) -> Result<Vec<(String, String)>, DbError> {
    let rows = db.query(
        "curriculum_links",
        "SELECT pm.program_id, pm.module_id FROM v_program_module pm JOIN v_program p ON p.id = pm.program_id \
         WHERE pm.relation = 'curricular' AND p.is_latest_po = 1 ORDER BY pm.module_id, pm.program_id",
        &[],
    )?;
    let text = |value: &Value| match value {
        Value::Text(text) => Some(text.clone()),
        Value::Integer(number) => Some(number.to_string()),
        _ => None,
    };
    Ok(rows.rows.iter().filter_map(|row| match row.as_slice() {
        [program, module] => text(program).zip(text(module)),
        _ => None,
    }).collect())
}

/// Every module that has a page, offered or not: the sitemap.
pub fn module_ids(db: &dyn Database) -> Result<Vec<String>, DbError> {
    let rows = db.query("module_ids", "SELECT module_id FROM v_module_facets ORDER BY module_id", &[])?;
    Ok(rows.rows.iter().filter_map(|row| match row.first() {
        Some(Value::Text(id)) => Some(id.clone()),
        Some(Value::Integer(id)) => Some(id.to_string()),
        _ => None,
    }).collect())
}

/// The semesters the validated study plan of a program places curriculum modules in: what the
/// semester filter offers. Empty without a plan.
pub fn program_plan_semesters(db: &dyn Database, program_id: &str) -> Result<Vec<i64>, DbError> {
    let rows = db.query(
        "program_plan_semesters",
        "SELECT DISTINCT plan_semester FROM v_program_module \
         WHERE program_id = ? AND relation = 'curricular' AND plan_semester IS NOT NULL ORDER BY plan_semester",
        &[Value::from(program_id)],
    )?;
    Ok(rows.rows.iter().filter_map(|row| match row.first() {
        Some(Value::Integer(n)) => Some(*n),
        _ => None,
    }).collect())
}

/// The exact number of modules a catalog query matches.
pub fn catalog_count(db: &dyn Database, query: &CatalogQuery) -> Result<u64, DbError> {
    let sql = query.to_sql();
    fetch_count(
        db,
        "catalog_count",
        &format!("SELECT COUNT(*) FROM v_module_facets f{}{}", sql.joins, sql.where_clause()),
        &sql.params,
    )
}

/// One page of the catalog table, with the exact total of the same filter.
pub fn catalog_page(
    db: &dyn Database,
    query: &CatalogQuery,
    offset: u64,
    limit: u64,
) -> Result<CatalogPage, DbError> {
    let total = catalog_count(db, query)?;

    let sql = query.to_sql();
    let program_columns = if query.program.is_some() {
        "pm.kind AS kind, pm.plan_semester AS plan_semester, pm.area AS area"
    } else {
        "NULL AS kind, NULL AS plan_semester, NULL AS area"
    };
    let mut params = sql.params.clone();
    params.push(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    params.push(Value::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));

    let rows: Vec<CatalogRow> = fetch(
        db,
        "catalog_page",
        &format!(
            "SELECT f.module_id, m.title, m.title_de, m.title_en, f.credits, f.turnus_season, f.turnus_parity, \
             f.offer_status, f.teaches_german, f.teaches_english, f.is_fues, f.is_limited, m.department, \
             f.teaching_events, f.exam_form, m.responsible, {program_columns} \
             FROM v_module_facets f JOIN v_module m ON m.id = f.module_id{}{}{} LIMIT ? OFFSET ?",
            sql.joins,
            sql.where_clause(),
            query.order_by()
        ),
        &params,
    )?;
    Ok(CatalogPage { total, offset, rows })
}

/// Where a module stands in the list a query orders (0-based), or `None` if it is not in it: the
/// list scrolls to the row the visitor comes back to without loading every page before it.
pub fn catalog_position(db: &dyn Database, query: &CatalogQuery, id: &str) -> Result<Option<u64>, DbError> {
    let sql = query.to_sql();
    let mut params = sql.params.clone();
    params.push(Value::from(id));
    let rows = db.query(
        "catalog_position",
        &format!(
            "SELECT n FROM (SELECT f.module_id AS module_id, ROW_NUMBER() OVER (ORDER BY {}) AS n \
             FROM v_module_facets f JOIN v_module m ON m.id = f.module_id{}{}) WHERE module_id = ?",
            query.order_terms(),
            sql.joins,
            sql.where_clause()
        ),
        &params,
    )?;
    Ok(match rows.rows.first().and_then(|row| row.first()) {
        Some(Value::Integer(n)) if *n >= 1 => Some((*n - 1) as u64),
        _ => None,
    })
}

pub fn module(db: &dyn Database, id: &str) -> Result<Option<Module>, DbError> {
    fetch_optional(
        db,
        "module",
        "SELECT id, title, title_de, title_en, credits, language_raw, teaches_german, teaches_english, \
         duration_raw, duration_semesters, \
         turnus_raw, turnus_season, turnus_parity, offer_status, limitation_raw, is_limited, participant_limit, \
         exam_form, exam_form_raw, exam_details, grading_raw, is_graded, is_fues, department, \
         learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory, remarks, \
         source_url, fetched_at, at_zentralcampus, at_sachsendorf, at_senftenberg \
         FROM v_module WHERE id = ?",
        &[Value::from(id)],
    )
}

/// The modules a module requires or recommends, already resolved to module ids.
pub fn module_prerequisites(db: &dyn Database, module_id: &str) -> Result<Vec<Prerequisite>, DbError> {
    fetch(
        db,
        "module_prerequisites",
        "SELECT module_id, required_module_id, kind, required_title, required_offer_status \
         FROM v_module_prerequisite WHERE module_id = ? ORDER BY kind, required_module_id",
        &[Value::from(module_id)],
    )
}

/// Search suggestions: ids first, then titles; at most one row per module.
pub fn search_suggestions(db: &dyn Database, text: &str, limit: u64) -> Result<Vec<SearchTerm>, DbError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    fetch(
        db,
        "search_suggestions",
        "SELECT module_id, term, kind FROM v_module_search WHERE term LIKE ? ESCAPE '\\' \
         GROUP BY module_id ORDER BY MIN(CASE kind WHEN 'id' THEN 0 ELSE 1 END), term COLLATE NOCASE LIMIT ?",
        &[Value::from(like_pattern(text)), Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX))],
    )
}

/// Everyone who teaches or is responsible for a module, for the lecturer filter.
pub fn lecturer_names(db: &dyn Database) -> Result<Vec<LecturerName>, DbError> {
    fetch(
        db,
        "lecturer_names",
        // Two persons are listed with two spellings of their title; the longer one is the fuller one.
        "SELECT name, MAX(title) AS title, COUNT(DISTINCT module_id) AS modules FROM v_module_lecturer \
         GROUP BY name ORDER BY name COLLATE NOCASE",
        &[],
    )
}

pub fn module_lecturers(db: &dyn Database, module_id: &str) -> Result<Vec<Lecturer>, DbError> {
    fetch(
        db,
        "module_lecturers",
        "SELECT DISTINCT name, title, role FROM v_module_lecturer WHERE module_id = ? \
         ORDER BY CASE role WHEN 'responsible' THEN 0 ELSE 1 END, name COLLATE NOCASE",
        &[Value::from(module_id)],
    )
}

pub fn module_teaching_forms(db: &dyn Database, module_id: &str) -> Result<Vec<ModuleTeachingForm>, DbError> {
    fetch(
        db,
        "module_teaching_forms",
        "SELECT form, form_raw, workload_raw, sws, hours FROM v_module_teaching_form WHERE module_id = ? ORDER BY ord",
        &[Value::from(module_id)],
    )
}

/// The literature and course lists of a module, in page order.
pub fn module_text_items(db: &dyn Database, module_id: &str) -> Result<Vec<TextItem>, DbError> {
    fetch(
        db,
        "module_text_items",
        "SELECT kind, text FROM v_module_text_item WHERE module_id = ? ORDER BY kind, ord",
        &[Value::from(module_id)],
    )
}

pub fn module_successors(db: &dyn Database, module_id: &str) -> Result<Vec<Successor>, DbError> {
    fetch(
        db,
        "module_successors",
        "SELECT successor_id, successor_title FROM v_module_successor WHERE module_id = ? ORDER BY successor_id",
        &[Value::from(module_id)],
    )
}

const EVENT_ORDER: &str = "ORDER BY semester_key DESC, event_title COLLATE NOCASE, event_id, ord";

/// The teaching events of a module, newest semester first. Exams are not in here. A date without a
/// semester is left out rather than failing the page, which shows the dates of a semester: the schema
/// allows one (Radix leaves `event.semester_key` NULL where it cannot read it, and `validate` warns),
/// and a build of 2026-09-27 wrote two events QIS had emptied without one.
pub fn module_schedule(db: &dyn Database, module_id: &str) -> Result<Vec<EventDate>, DbError> {
    fetch(
        db,
        "module_schedule",
        &format!(
            "SELECT semester_key, semester_label, event_id, event_number, event_title, event_type, group_name, \
             weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, room, campus, instructor, \
             comment, source_url, room_short FROM v_module_schedule WHERE module_id = ? AND semester_key IS NOT NULL \
             {EVENT_ORDER}"
        ),
        &[Value::from(module_id)],
    )
}

/// The exam dates of a module, newest semester first; one without a semester is left out, as in
/// `module_schedule`.
pub fn module_exams(db: &dyn Database, module_id: &str) -> Result<Vec<EventDate>, DbError> {
    fetch(
        db,
        "module_exams",
        &format!(
            "SELECT semester_key, semester_label, event_id, event_number, event_title, NULL AS event_type, \
             NULL AS group_name, weekday, start_time, end_time, NULL AS rhythm, NULL AS rhythm_raw, first_date, \
             last_date, room, campus, NULL AS instructor, comment, source_url, room_short \
             FROM v_module_exam WHERE module_id = ? AND semester_key IS NOT NULL {EVENT_ORDER}"
        ),
        &[Value::from(module_id)],
    )
}

/// The programs a module page names: resolved ones first, the newest PO of each first.
pub fn module_program_links(db: &dyn Database, module_id: &str) -> Result<Vec<ProgramLink>, DbError> {
    fetch(
        db,
        "module_program_links",
        "SELECT degree_raw, program_raw, po_raw, resolve_status, program_id, program_slug, program_name, degree_display, \
         po_version, is_latest_po, relation, kind, kind_source, area \
         FROM v_module_program_link WHERE module_id = ? \
         ORDER BY program_slug IS NULL, relation, program_name COLLATE NOCASE, po_version DESC, ord",
        &[Value::from(module_id)],
    )
}

/// Where the validated study plans place a module: every row of a plan that names it, in the
/// plan's order (docs/schema-v2.md, `get_curriculum_entries`).
pub fn module_plan_places(db: &dyn Database, module_id: &str) -> Result<Vec<PlanPlace>, DbError> {
    fetch(
        db,
        "module_plan_places",
        "SELECT program_id, semester, start_semester, end_semester FROM v_program_plan_entry \
         WHERE module_id = ? ORDER BY program_id, ord",
        &[Value::from(module_id)],
    )
}

/// The other PO versions of a program, newest first.
pub fn program_versions(db: &dyn Database, program_id: &str) -> Result<Vec<ProgramVersion>, DbError> {
    fetch(
        db,
        "program_versions",
        "SELECT other_slug, po_version, is_latest_po FROM v_program_version WHERE program_id = ? \
         ORDER BY po_year DESC, po_version DESC",
        &[Value::from(program_id)],
    )
}

/// The best-matching Bachelor of a Master, or Master of a Bachelor.
pub fn program_counterpart(db: &dyn Database, program_id: &str) -> Result<Option<Counterpart>, DbError> {
    fetch_optional(
        db,
        "program_counterpart",
        "SELECT counterpart_slug, counterpart_name, counterpart_level, counterpart_po_version \
         FROM v_program_counterpart WHERE program_id = ? \
         ORDER BY match_score DESC, counterpart_po_version DESC LIMIT 1",
        &[Value::from(program_id)],
    )
}

pub fn program_documents(db: &dyn Database, program_id: &str) -> Result<Vec<Document>, DbError> {
    fetch(
        db,
        "program_documents",
        "SELECT title, doc_type, url FROM v_program_document WHERE program_id = ? ORDER BY ord",
        &[Value::from(program_id)],
    )
}

/// The module tree of a program: every placement of a module in an area, in tree order.
pub fn program_areas(db: &dyn Database, program_id: &str) -> Result<Vec<AreaPlacement>, DbError> {
    // The program's own rows of `v_program_module` first (the same rows the join on program and
    // module finds): joined as the view itself, SQLite read the whole view for every placement,
    // 120 ms instead of 2 in the browser with every filter of a program.
    fetch(
        db,
        "program_areas",
        "SELECT a.module_id, m.title AS module_title, m.credits AS module_credits, a.area_id, a.area, \
         a.area_label, a.depth, a.area_ord, a.kind, a.kind_basis, pm.kind AS module_kind \
         FROM v_program_module_area a JOIN v_module m ON m.id = a.module_id \
         LEFT JOIN (SELECT module_id, kind FROM v_program_module WHERE program_id = ?) pm ON pm.module_id = a.module_id \
         WHERE a.program_id = ? \
         ORDER BY a.area_ord, a.area_id, m.title COLLATE NOCASE, a.module_id",
        &[Value::from(program_id), Value::from(program_id)],
    )
}

/// Every node of the program's module tree in tree order, those without modules included.
pub fn program_area_tree(db: &dyn Database, program_id: &str) -> Result<Vec<AreaNode>, DbError> {
    fetch(
        db,
        "program_area_tree",
        "SELECT id, parent_id, depth, label, stated_kind FROM program_area WHERE program_id = ? ORDER BY ord, id",
        &[Value::from(program_id)],
    )
}

pub fn program_plan(db: &dyn Database, program_id: &str) -> Result<Option<Plan>, DbError> {
    fetch_optional(
        db,
        "program_plan",
        "SELECT source_file, source_pages, source_label, layout_json, validated_at \
         FROM v_program_plan WHERE program_id = ?",
        &[Value::from(program_id)],
    )
}

pub fn program_plan_entries(db: &dyn Database, program_id: &str) -> Result<Vec<PlanEntry>, DbError> {
    fetch(
        db,
        "program_plan_entries",
        "SELECT ord, module_id, module_name, semester, start_semester, end_semester, semester_span, credits, \
         min_credits, max_credits, kind, kind_raw, study_section, subject_area, specialization, catalog_title, \
         credits_differ_from_catalog, source_page FROM v_program_plan_entry WHERE program_id = ? ORDER BY ord",
        &[Value::from(program_id)],
    )
}

/// The sums the regulation prints over the rows of its plan, each with the rows it counts. They
/// are what a plan with elective budgets adds up to; its rows alone only give a range.
pub fn program_plan_totals(db: &dyn Database, program_id: &str) -> Result<Vec<PlanTotal>, DbError> {
    let mut totals: Vec<PlanTotal> = fetch(
        db,
        "program_plan_totals",
        "SELECT ord, label, scope, specialization, start_semester, end_semester, credits, credits_max, \
         min_credits, max_credits, is_choice, entry_count FROM v_program_plan_total WHERE program_id = ? ORDER BY ord",
        &[Value::from(program_id)],
    )?;
    for member in program_plan_total_entries(db, program_id)? {
        if let Some(total) = totals.iter_mut().find(|total| total.ord == member.total_ord) {
            total.entries.push(member.entry_ord);
        }
    }
    Ok(totals)
}

/// Which row of the plan each of its sums counts. `program_plan_totals` joins the two.
pub fn program_plan_total_entries(db: &dyn Database, program_id: &str) -> Result<Vec<PlanTotalEntry>, DbError> {
    fetch(
        db,
        "program_plan_total_entries",
        "SELECT total_ord, entry_ord FROM v_program_plan_total_entry WHERE program_id = ? ORDER BY total_ord, entry_ord",
        &[Value::from(program_id)],
    )
}

/// The most module ids one Studienplan query takes: the store's cap of planned modules in all
/// semesters (`studyplan`), so a whole plan is one answer.
pub const MAX_PLANNED: usize = 400;

/// What `id_json` gives for a list without a single module id.
const NO_IDS: &str = "[]";

/// The ids as a JSON array for `json_each(?)`: `url::is_module_id` only, sorted, deduplicated,
/// capped. Constant SQL text (rusqlite's statement cache); equal sets give equal parameters
/// (the client's answer cache). The id charset needs no JSON escaping.
fn id_json(module_ids: &[String]) -> String {
    let ids: BTreeSet<&str> = module_ids.iter().map(String::as_str).filter(|id| is_module_id(id)).collect();
    let mut json = String::from("[");
    for (n, id) in ids.into_iter().take(MAX_PLANNED).enumerate() {
        if n > 0 {
            json.push(',');
        }
        json.push('"');
        json.push_str(id);
        json.push('"');
    }
    json.push(']');
    json
}

/// The columns of `DateRow` from `v_module_schedule`.
const DATE_ROW_COLUMNS: &str = "module_id, semester_key, semester_label, event_id, event_number, event_title, \
     event_type, ord, group_name, weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, room, \
     campus, instructor, comment, cancelled_dates, source_url, room_short";

/// The columns of `DateRow` from `v_module_exam`, which has no type, group, rhythm, instructor or
/// cancellations: the same row struct reads teaching and exam rows.
const EXAM_ROW_COLUMNS: &str = "module_id, semester_key, semester_label, event_id, event_number, event_title, \
     NULL AS event_type, ord, NULL AS group_name, weekday, start_time, end_time, NULL AS rhythm, NULL AS rhythm_raw, \
     first_date, last_date, room, campus, NULL AS instructor, comment, NULL AS cancelled_dates, source_url, room_short";

/// The teaching rows of the planned modules in one semester, events without dates included (the
/// Studienplan lists them „ohne feste Zeit"). An event linked to two of the modules comes once for
/// each; the timetable merges them.
pub fn modules_schedule(db: &dyn Database, module_ids: &[String], semester_key: &str) -> Result<Vec<DateRow>, DbError> {
    let ids = id_json(module_ids);
    if ids == NO_IDS {
        return Ok(Vec::new());
    }
    fetch(
        db,
        "modules_schedule",
        &format!(
            "SELECT {DATE_ROW_COLUMNS} FROM v_module_schedule \
             WHERE semester_key = ? AND module_id IN (SELECT value FROM json_each(?)) ORDER BY event_id, ord, module_id"
        ),
        &[Value::from(semester_key), Value::from(ids)],
    )
}

/// The exam rows of the planned modules in one semester, undated sittings included.
pub fn modules_exams(db: &dyn Database, module_ids: &[String], semester_key: &str) -> Result<Vec<DateRow>, DbError> {
    let ids = id_json(module_ids);
    if ids == NO_IDS {
        return Ok(Vec::new());
    }
    fetch(
        db,
        "modules_exams",
        &format!(
            "SELECT {EXAM_ROW_COLUMNS} FROM v_module_exam \
             WHERE semester_key = ? AND module_id IN (SELECT value FROM json_each(?)) ORDER BY event_id, ord, module_id"
        ),
        &[Value::from(semester_key), Value::from(ids)],
    )
}

/// Every dated teaching row of a semester, with only what the finder compares („Passt in meinen
/// Plan"): no rooms, persons or links, so the whole semester stays one answer of about a megabyte.
pub fn semester_schedule(db: &dyn Database, semester_key: &str) -> Result<Vec<DateRow>, DbError> {
    fetch(
        db,
        "semester_schedule",
        "SELECT module_id, semester_key, semester_label, event_id, NULL AS event_number, event_title, event_type, ord, \
         group_name, weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, NULL AS room, campus, \
         NULL AS instructor, NULL AS comment, cancelled_dates, NULL AS source_url, NULL AS room_short \
         FROM v_module_schedule WHERE semester_key = ? AND ord IS NOT NULL ORDER BY module_id, event_id, ord",
        &[Value::from(semester_key)],
    )
}

/// Every dated exam row of a semester, as lean as `semester_schedule`. The room stays: two
/// modules examined in the same room at the same time sit one joint exam, not two that clash.
pub fn semester_exams(db: &dyn Database, semester_key: &str) -> Result<Vec<DateRow>, DbError> {
    fetch(
        db,
        "semester_exams",
        "SELECT module_id, semester_key, semester_label, event_id, NULL AS event_number, event_title, \
         NULL AS event_type, ord, NULL AS group_name, weekday, start_time, end_time, NULL AS rhythm, \
         NULL AS rhythm_raw, first_date, last_date, room, campus, NULL AS instructor, NULL AS comment, \
         NULL AS cancelled_dates, NULL AS source_url, room_short \
         FROM v_module_exam WHERE semester_key = ? AND first_date IS NOT NULL ORDER BY module_id, event_id, ord",
        &[Value::from(semester_key)],
    )
}

/// The dated teaching rows of a semester, counted by rhythm and date range: the evidence for its
/// lecture period, breaks and A/B weeks (`timetable::facts`).
pub fn semester_date_counts(db: &dyn Database, semester_key: &str) -> Result<Vec<DateCount>, DbError> {
    fetch(
        db,
        "semester_date_counts",
        "SELECT rhythm, first_date, last_date, COUNT(DISTINCT event_id || '/' || ord) AS dates \
         FROM v_module_schedule WHERE semester_key = ? AND first_date IS NOT NULL \
         GROUP BY rhythm, first_date, last_date ORDER BY rhythm, first_date, last_date",
        &[Value::from(semester_key)],
    )
}

/// The SWS per teaching form of the planned modules, whatever the semester: a module page states
/// them once.
pub fn modules_teaching_sws(db: &dyn Database, module_ids: &[String]) -> Result<Vec<ModuleSws>, DbError> {
    let ids = id_json(module_ids);
    if ids == NO_IDS {
        return Ok(Vec::new());
    }
    fetch(
        db,
        "modules_teaching_sws",
        "SELECT module_id, form, SUM(sws) AS sws FROM v_module_teaching_form \
         WHERE sws IS NOT NULL AND form IS NOT NULL AND module_id IN (SELECT value FROM json_each(?)) \
         GROUP BY module_id, form ORDER BY module_id, form",
        &[Value::from(ids)],
    )
}

/// The abbreviations of modules („AuP", schema 9): within `program_id` the one unique among that
/// program's modules (`v_program_module`), else the module's own (`v_module`). Only modules that
/// have one; a week grid names the others by a short title.
pub fn modules_abbrevs(db: &dyn Database, module_ids: &[String], program_id: Option<&str>) -> Result<Vec<ModuleAbbrev>, DbError> {
    let ids = id_json(module_ids);
    if ids == NO_IDS {
        return Ok(Vec::new());
    }
    fetch(
        db,
        "modules_abbrevs",
        "SELECT m.id AS module_id, COALESCE(p.abbrev, m.abbrev) AS abbrev FROM v_module m \
         LEFT JOIN v_program_module p ON p.module_id = m.id AND p.program_id = ? \
         WHERE m.id IN (SELECT value FROM json_each(?)) AND COALESCE(p.abbrev, m.abbrev) IS NOT NULL ORDER BY m.id",
        &[program_id.map_or(Value::Null, Value::from), Value::from(ids)],
    )
}

/// The SWS per teaching form of every module taught in a semester: the finder's candidates.
pub fn semester_teaching_sws(db: &dyn Database, semester_key: &str) -> Result<Vec<ModuleSws>, DbError> {
    fetch(
        db,
        "semester_teaching_sws",
        "SELECT module_id, form, SUM(sws) AS sws FROM v_module_teaching_form \
         WHERE sws IS NOT NULL AND form IS NOT NULL \
         AND module_id IN (SELECT module_id FROM v_module_schedule WHERE semester_key = ?) \
         GROUP BY module_id, form ORDER BY module_id, form",
        &[Value::from(semester_key)],
    )
}

#[cfg(test)]
mod id_tests {
    use super::{id_json, MAX_PLANNED};

    fn json(ids: &[&str]) -> String {
        id_json(&ids.iter().map(|id| id.to_string()).collect::<Vec<_>>())
    }

    /// One parameter per set of modules: the same set in another order, twice or beside what is no
    /// module id asks the same question, and what is no module id never reaches the SQL.
    #[test]
    fn a_set_of_modules_is_one_parameter() {
        assert_eq!(json(&[]), "[]");
        assert_eq!(json(&["12107", "12104", "12107"]), r#"["12104","12107"]"#);
        assert_eq!(json(&["12104", "12107"]), json(&["12107", "1 OR 1=1", "12104", ""]));
        assert_eq!(json(&["1 OR 1=1", "\"]", "a\\b", "x'y"]), "[]");

        let many: Vec<String> = (0..MAX_PLANNED + 10).rev().map(|n| format!("{n:05}")).collect();
        let capped = id_json(&many);
        assert_eq!(capped.matches(',').count(), MAX_PLANNED - 1);
        assert!(capped.starts_with(r#"["00000","00001","#) && capped.ends_with(r#""00399"]"#), "{capped}");
    }
}

/// What the views allow and a page must survive, in a catalog of its own: the snapshot of the other
/// tests holds only what Radix builds today.
#[cfg(test)]
mod view_edge_tests {
    use std::path::{Path, PathBuf};

    use crate::native::NativeDatabase;
    use crate::rows_detail::EventDate;
    use crate::{pages, queries};

    /// A catalog at `path` made by Radix's own migrations (`internal/catalogdb/migrations`), with `rows`.
    fn catalog(path: &Path, rows: &str) -> NativeDatabase {
        let _ = std::fs::remove_file(path);
        let conn = rusqlite::Connection::open(path).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../internal/catalogdb/migrations");
        let mut migrations: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
            .collect();
        migrations.sort();
        for migration in &migrations {
            let sql = std::fs::read_to_string(migration).unwrap();
            conn.execute_batch(&sql).unwrap_or_else(|e| panic!("{}: {e}", migration.display()));
        }
        conn.execute_batch(&format!("PRAGMA user_version = {};\n{rows}", migrations.len())).unwrap();
        drop(conn);
        NativeDatabase::open(path).unwrap()
    }

    /// The build of 2026-09-27 wrote two events QIS had emptied as events titled with their ID,
    /// without a semester or a date, and the module descriptions still linked them: the pages of
    /// those modules failed on the NULL. Radix no longer builds such an event, and a date without a
    /// semester, which the schema allows, leaves the page with its other dates.
    #[test]
    fn a_date_without_a_semester_leaves_the_module_page_with_the_others() {
        let path = std::env::temp_dir().join(format!("folia-catalog-no-semester-{}.db", std::process::id()));
        let db = catalog(
            &path,
            "INSERT INTO meta (key, value) VALUES ('current_semester', '2026W');
             INSERT INTO semester (key, season, year, label, starts_on, ends_on)
                 VALUES ('2026W', 'winter', 2026, 'WiSe 2026/27', '2026-10-01', '2027-03-31');
             INSERT INTO module (id, title, detail_status, offer_status, is_fues)
                 VALUES ('21501', 'Internationales Bau- und Planungsrecht', 'ok', 'active', 0);
             INSERT INTO event (id, title, type_raw, category, semester_key, source_url, fetched_at) VALUES
                 ('149001', 'Internationales Bau- und Planungsrecht', 'Seminar', 'teaching', '2026W', 'https://qis/149001', '2026-09-27T08:00:00Z'),
                 ('149002', 'Prüfung Internationales Bau- und Planungsrecht', 'Prüfung', 'exam', '2026W', 'https://qis/149002', '2026-09-27T08:00:00Z'),
                 ('149396', '149396', NULL, 'other', NULL, 'https://qis/149396', '2026-09-27T08:41:57Z'),
                 ('149397', 'Mündliche Prüfung', 'Prüfung', 'exam', NULL, 'https://qis/149397', '2026-09-27T08:41:57Z');
             INSERT INTO event_date (event_id, ord, weekday, start_time, end_time, rhythm, first_date, last_date) VALUES
                 ('149001', 1, 1, '13:45', '15:15', 'weekly', '2026-10-05', '2027-01-25'),
                 ('149002', 1, 5, '10:00', '12:00', 'single', '2027-02-12', '2027-02-12'),
                 ('149397', 1, 3, '09:00', '09:30', 'single', '2027-02-17', '2027-02-17');
             INSERT INTO module_event (module_id, event_id)
                 VALUES ('21501', '149001'), ('21501', '149002'), ('21501', '149396'), ('21501', '149397');",
        );
        let ids = |dates: &[EventDate]| dates.iter().map(|date| date.event_id.clone()).collect::<Vec<_>>();

        assert_eq!(ids(&queries::module_schedule(&db, "21501").unwrap()), ["149001"]);
        assert_eq!(ids(&queries::module_exams(&db, "21501").unwrap()), ["149002"]);
        let page = pages::module(&db, "21501").unwrap().expect("the module has a page");
        assert_eq!((ids(&page.schedule), ids(&page.exams)), (vec!["149001".to_string()], vec!["149002".to_string()]));

        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
