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
    ProgramDepartmentCount, ProgramLink, ProgramVersion, Successor, TextItem,
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

/// The landing page: the overview, how many modules each of its entry links leads to, and the
/// faculties with the number of their current programs (in the order of the program overview:
/// 1 to 6, then the others; `None` = programs no faculty could be derived for).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HomeData {
    pub overview: Overview,
    /// One count per query handed to `home`, in their order.
    pub entry_counts: Vec<u64>,
    pub faculties: Vec<(Option<Department>, usize)>,
}

pub fn home(db: &dyn Database, entries: &[CatalogQuery]) -> Result<HomeData, DbError> {
    let overview = overview(db)?;
    let entry_counts = entries.iter().map(|query| queries::catalog_count(db, query)).collect::<Result<Vec<_>, _>>()?;
    let ProgramsData { programs, departments, faculties } = programs_overview(db)?;
    let mut counted: Vec<(Option<Department>, usize)> = Vec::new();
    for program in programs.iter().filter(|program| program.is_latest_po) {
        let department = faculties
            .iter()
            .find(|faculty| faculty.program_id == program.id)
            .and_then(|faculty| departments.iter().find(|department| department.id == faculty.department_id));
        match counted.iter_mut().find(|(known, _)| known.as_ref().map(|d| d.id) == department.map(|d| d.id)) {
            Some((_, count)) => *count += 1,
            None => counted.push((department.cloned(), 1)),
        }
    }
    counted.sort_by_key(|(department, _)| match department {
        Some(d) => (d.code.parse::<u32>().map_or(1, |_| 0), d.code.parse::<u32>().unwrap_or(0), d.code.clone()),
        None => (2, 0, String::new()),
    });
    Ok(HomeData { overview, entry_counts, faculties: counted })
}

/// The map of the current programs (`graph`). The web server calls this once per snapshot.
pub fn program_map(db: &dyn Database) -> Result<crate::graph::ProgramMap, DbError> {
    let programs: Vec<Program> = queries::programs(db)?.into_iter().filter(|program| program.is_latest_po).collect();
    Ok(crate::graph::program_map(&programs, &queries::curriculum_links(db)?))
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

/// What a derived faculty rests on, from strongest to weakest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FacultyBasis {
    /// The department of the program's thesis module.
    Thesis,
    /// The department that offers at least half of the curriculum's offered modules.
    Majority,
    /// The faculty of the programs of the same subject (Bachelor and Master), where they agree.
    Counterpart,
}

/// The faculty of a program. **Derived, not stated:** neither the module catalog nor the
/// lecture directory of the BTU names a program's faculty. A program without a clear answer has
/// no entry, and the page says so („unknown stays unknown").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramFaculty {
    pub program_id: String,
    pub department_id: i64,
    pub basis: FacultyBasis,
}

/// Derives the faculties: the thesis module's department if there is exactly one; else the
/// department offering at least half of the offered curriculum; else what the other programs
/// of the same subject agree on.
pub fn faculties(programs: &[Program], counts: &[ProgramDepartmentCount]) -> Vec<ProgramFaculty> {
    let mut found: Vec<ProgramFaculty> = Vec::new();
    for program in programs {
        let own: Vec<&ProgramDepartmentCount> = counts.iter().filter(|c| c.program_id == program.id).collect();
        let thesis: Vec<i64> = own.iter().filter(|c| c.thesis_modules > 0).map(|c| c.department_id).collect();
        let offered: i64 = own.iter().map(|c| c.offered_modules).sum();
        let largest = own.iter().max_by_key(|c| c.offered_modules);
        let derived = match (thesis.as_slice(), largest) {
            ([department], _) => Some((*department, FacultyBasis::Thesis)),
            (_, Some(largest)) if offered > 0 && largest.offered_modules * 2 >= offered => Some((largest.department_id, FacultyBasis::Majority)),
            _ => None,
        };
        if let Some((department_id, basis)) = derived {
            found.push(ProgramFaculty { program_id: program.id.clone(), department_id, basis });
        }
    }
    // The subject of „Wirtschaftsingenieurwesen - dual" is „Wirtschaftsingenieurwesen".
    let subject = |program: &Program| program.name_key.trim_end_matches("-dual").to_string();
    let by_subject: Vec<ProgramFaculty> = programs
        .iter()
        .filter(|program| !found.iter().any(|f| f.program_id == program.id))
        .filter_map(|program| {
            let mut agreed: Vec<i64> = programs
                .iter()
                .filter(|other| subject(other) == subject(program))
                .filter_map(|other| found.iter().find(|f| f.program_id == other.id).map(|f| f.department_id))
                .collect();
            agreed.sort_unstable();
            agreed.dedup();
            match agreed.as_slice() {
                [department] => Some(ProgramFaculty { program_id: program.id.clone(), department_id: *department, basis: FacultyBasis::Counterpart }),
                _ => None,
            }
        })
        .collect();
    found.extend(by_subject);
    found
}

/// The program overview: every program, the departments, and each program's derived faculty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramsData {
    pub programs: Vec<Program>,
    pub departments: Vec<Department>,
    pub faculties: Vec<ProgramFaculty>,
}

pub fn programs_overview(db: &dyn Database) -> Result<ProgramsData, DbError> {
    let programs = queries::programs(db)?;
    let faculties = faculties(&programs, &queries::program_department_counts(db)?);
    Ok(ProgramsData { departments: queries::departments(db)?, faculties, programs })
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
