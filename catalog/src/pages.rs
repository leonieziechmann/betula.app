//! What each page needs, loaded in one go from one snapshot.
//!
//! A page asks for its data once; server and browser run the same function, so the
//! server-rendered HTML and the hydrated page cannot disagree.

use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError};
use crate::filter::{CatalogQuery, ProgramRelation, ProgramScope};
use crate::queries;
use crate::rows::{CatalogPage, Department, Meta, Module, Prerequisite, Program, ProgramModule, Semester};
use crate::rows_detail::{
    AreaPlacement, Counterpart, Document, EventDate, Lecturer, LecturerName, ModuleTeachingForm, Plan, PlanEntry,
    ProgramLink, ProgramVersion, Successor, TextItem,
};
use crate::url::{CatalogUrl, PAGE_SIZE};

/// The landing page and the footer: how big and how fresh the catalog is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overview {
    pub meta: Meta,
    pub current_semester: Option<Semester>,
    /// Modules the default catalog lists (what is no longer offered is hidden).
    pub modules: u64,
    /// Programs in their current PO version.
    pub programs: u64,
}

pub fn overview(db: &dyn Database) -> Result<Overview, DbError> {
    Ok(Overview {
        meta: queries::meta(db)?,
        current_semester: queries::semesters(db)?.into_iter().find(|s| s.is_current),
        modules: queries::catalog_count(db, &CatalogQuery::default())?,
        programs: queries::programs(db)?.iter().filter(|p| p.is_latest_po).count() as u64,
    })
}

/// What the pickers of the filter panel offer. The same for every filter, so the page loads it
/// once and not with every list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogChoices {
    pub programs: Vec<Program>,
    pub departments: Vec<Department>,
    pub lecturers: Vec<LecturerName>,
}

pub fn catalog_choices(db: &dyn Database) -> Result<CatalogChoices, DbError> {
    Ok(CatalogChoices {
        programs: queries::programs(db)?,
        departments: queries::departments(db)?,
        lecturers: queries::lecturer_names(db)?,
    })
}

/// The catalog: one page of modules plus what the list and the filter panel say about it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogData {
    pub page: CatalogPage,
    /// The selected program, if the URL names one that exists.
    pub program: Option<Program>,
    /// With a program selected: how many modules its curriculum and its FÜS list match.
    pub curricular_total: Option<u64>,
    pub fues_total: Option<u64>,
    /// The semesters of the selected program's study plan (empty without a plan).
    pub plan_semesters: Vec<i64>,
    /// For the name of the selected department (few rows; the long lists are `CatalogChoices`).
    pub departments: Vec<Department>,
    pub meta: Meta,
}

pub fn catalog(db: &dyn Database, url: &CatalogUrl) -> Result<CatalogData, DbError> {
    let program = match &url.query.program {
        Some(scope) => queries::program_by_slug(db, &scope.program_slug)?,
        None => None,
    };

    // The two lists of a program are shown as tabs, each with its exact total.
    let (mut curricular_total, mut fues_total) = (None, None);
    if let (Some(scope), Some(_)) = (&url.query.program, &program) {
        for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
            let other = CatalogQuery {
                program: Some(ProgramScope { relation, ..scope.clone() }),
                ..url.query.clone()
            };
            let total = Some(queries::catalog_count(db, &other)?);
            match relation {
                ProgramRelation::Curricular => curricular_total = total,
                ProgramRelation::Fues => fues_total = total,
            }
        }
    }

    let plan_semesters = match &program {
        Some(program) if program.has_plan => queries::program_plan_semesters(db, &program.id)?,
        _ => Vec::new(),
    };

    Ok(CatalogData {
        page: queries::catalog_page(db, &url.query, url.offset(), PAGE_SIZE)?,
        plan_semesters,
        program,
        curricular_total,
        fues_total,
        departments: queries::departments(db)?,
        meta: queries::meta(db)?,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleData {
    pub module: Module,
    pub lecturers: Vec<Lecturer>,
    pub teaching_forms: Vec<ModuleTeachingForm>,
    pub text_items: Vec<TextItem>,
    pub prerequisites: Vec<Prerequisite>,
    pub successors: Vec<Successor>,
    pub schedule: Vec<EventDate>,
    pub exams: Vec<EventDate>,
    pub programs: Vec<ProgramLink>,
    pub semesters: Vec<Semester>,
}

pub fn module(db: &dyn Database, id: &str) -> Result<Option<ModuleData>, DbError> {
    let Some(module) = queries::module(db, id)? else { return Ok(None) };
    Ok(Some(ModuleData {
        lecturers: queries::module_lecturers(db, id)?,
        teaching_forms: queries::module_teaching_forms(db, id)?,
        text_items: queries::module_text_items(db, id)?,
        prerequisites: queries::module_prerequisites(db, id)?,
        successors: queries::module_successors(db, id)?,
        schedule: queries::module_schedule(db, id)?,
        exams: queries::module_exams(db, id)?,
        programs: queries::module_program_links(db, id)?,
        semesters: queries::semesters(db)?,
        module,
    }))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramData {
    pub program: Program,
    pub versions: Vec<ProgramVersion>,
    pub counterpart: Option<Counterpart>,
    pub documents: Vec<Document>,
    pub curricular: Vec<ProgramModule>,
    pub fues: Vec<ProgramModule>,
    pub areas: Vec<AreaPlacement>,
    pub plan: Option<Plan>,
    pub plan_entries: Vec<PlanEntry>,
}

pub fn program(db: &dyn Database, slug: &str) -> Result<Option<ProgramData>, DbError> {
    let Some(program) = queries::program_by_slug(db, slug)? else { return Ok(None) };
    let id = program.id.clone();
    Ok(Some(ProgramData {
        versions: queries::program_versions(db, &id)?,
        counterpart: queries::program_counterpart(db, &id)?,
        documents: queries::program_documents(db, &id)?,
        curricular: queries::program_modules(db, &id, ProgramRelation::Curricular)?,
        fues: queries::program_modules(db, &id, ProgramRelation::Fues)?,
        areas: queries::program_areas(db, &id)?,
        plan: queries::program_plan(db, &id)?,
        plan_entries: queries::program_plan_entries(db, &id)?,
        program,
    }))
}
