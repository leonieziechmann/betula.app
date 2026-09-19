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

const PROGRAM_COLUMNS: &str = "id, slug, name, degree_level, study_variant, degree_label, degree_raw, \
     degree_display, po_version, po_year, family_key, is_latest_po, source_url, has_plan, plan_status, \
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
             f.teaching_events, {program_columns} \
             FROM v_module_facets f JOIN v_module m ON m.id = f.module_id{}{}{} LIMIT ? OFFSET ?",
            sql.joins,
            sql.where_clause(),
            query.order_by()
        ),
        &params,
    )?;
    Ok(CatalogPage { total, offset, rows })
}

pub fn module(db: &dyn Database, id: &str) -> Result<Option<Module>, DbError> {
    fetch_optional(
        db,
        "module",
        "SELECT id, title, title_de, title_en, credits, language_raw, duration_raw, duration_semesters, \
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
