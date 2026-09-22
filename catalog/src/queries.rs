//! Every SQL statement of the web tier, one function per query.
//!
//! Rules (docs/frontend-rewrite.md §4): read only `v_*` views and `program_coverage`;
//! filter and sort on view columns; `LIKE` only against `v_module_search`; every list
//! comes with an exact total. `tests::every_query_runs_against_the_snapshot` fails for
//! a `pub fn` in this file that the tests never ran.

use crate::db::{fetch, fetch_count, fetch_optional, Database, DbError, Value};
use crate::filter::{like_pattern, CatalogQuery, ProgramRelation};
use crate::rows::{
    CatalogPage, CatalogRow, Department, Meta, Module, Prerequisite, Program, ProgramModule, SearchTerm,
    Semester,
};
use crate::rows_detail::{
    AreaNode, AreaPlacement, Counterpart, Document, EventDate, Lecturer, LecturerName, ModuleTeachingForm, Plan, PlanEntry,
    PlanTotal, PlanTotalEntry, ProgramDepartmentCount, ProgramLink, ProgramVersion, Successor, TextItem,
};

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

/// The teaching events of a module, newest semester first. Exams are not in here.
pub fn module_schedule(db: &dyn Database, module_id: &str) -> Result<Vec<EventDate>, DbError> {
    fetch(
        db,
        "module_schedule",
        &format!(
            "SELECT semester_key, semester_label, event_id, event_number, event_title, event_type, group_name, \
             weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, room, campus, instructor, \
             comment, source_url FROM v_module_schedule WHERE module_id = ? {EVENT_ORDER}"
        ),
        &[Value::from(module_id)],
    )
}

/// The exam dates of a module, newest semester first.
pub fn module_exams(db: &dyn Database, module_id: &str) -> Result<Vec<EventDate>, DbError> {
    fetch(
        db,
        "module_exams",
        &format!(
            "SELECT semester_key, semester_label, event_id, event_number, event_title, NULL AS event_type, \
             NULL AS group_name, weekday, start_time, end_time, NULL AS rhythm, NULL AS rhythm_raw, first_date, \
             last_date, room, campus, NULL AS instructor, comment, source_url \
             FROM v_module_exam WHERE module_id = ? {EVENT_ORDER}"
        ),
        &[Value::from(module_id)],
    )
}

/// The programs a module page names: resolved ones first, the newest PO of each first.
pub fn module_program_links(db: &dyn Database, module_id: &str) -> Result<Vec<ProgramLink>, DbError> {
    fetch(
        db,
        "module_program_links",
        "SELECT degree_raw, program_raw, po_raw, resolve_status, program_slug, program_name, degree_display, \
         po_version, is_latest_po, relation, kind, kind_source, area \
         FROM v_module_program_link WHERE module_id = ? \
         ORDER BY program_slug IS NULL, relation, program_name COLLATE NOCASE, po_version DESC, ord",
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
        "SELECT source_file, layout_json, validated_at FROM v_program_plan WHERE program_id = ?",
        &[Value::from(program_id)],
    )
}

pub fn program_plan_entries(db: &dyn Database, program_id: &str) -> Result<Vec<PlanEntry>, DbError> {
    fetch(
        db,
        "program_plan_entries",
        "SELECT ord, module_id, module_name, semester, start_semester, end_semester, semester_span, credits, \
         min_credits, max_credits, kind, kind_raw, study_section, subject_area, specialization, catalog_title, \
         credits_differ_from_catalog FROM v_program_plan_entry WHERE program_id = ? ORDER BY ord",
        &[Value::from(program_id)],
    )
}

/// The sums the regulation prints over the rows of its plan, each with the rows it counts. They
/// are what a plan with elective budgets adds up to; its rows alone only give a range.
pub fn program_plan_totals(db: &dyn Database, program_id: &str) -> Result<Vec<PlanTotal>, DbError> {
    let mut totals: Vec<PlanTotal> = fetch(
        db,
        "program_plan_totals",
        "SELECT ord, label, scope, specialization, start_semester, end_semester, credits, \
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
