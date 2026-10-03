//! What each page needs, loaded in one go from one snapshot.
//!
//! A page asks for its data once; server and browser run the same function, so the
//! server-rendered HTML and the hydrated page cannot disagree.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod graph;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::{clock, Day};
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::SharedPlan;
use folia_calendar::subscription::Subscription;
use folia_locale::Locale;
use folia_model::db::{Database, DbError};
use folia_model::labels::{Labelled, OfferStatus};
use folia_model::rows::{
    CatalogPage, CatalogRow, Department, Meta, Module, Prerequisite, Program, ProgramModule, SearchElsewhere, Semester,
};
use folia_model::rows_detail::{
    AreaNode, AreaPlacement, Counterpart, DateCount, DateRow, Document, EventDate, Lecturer, LecturerName, ModuleSws,
    ModuleTeachingForm, Plan, PlanEntry, PlanPlace, PlanTotal, ProgramDepartmentCount, ProgramLink, ProgramVersion,
    Successor, TextItem,
};
use folia_plans::plan::{self, SemesterPlan};
use folia_plans::variants::{self, PlanVariant, Supplement};
use folia_query as queries;
use folia_routes::filter::{
    CatalogQuery, FitsFilter, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey, TurnusFilter,
};
use folia_routes::url::{BookmarkSort, CatalogUrl, PAGE_SIZE, Season};
use folia_search as search;
use folia_timetable::clash;
use folia_timetable::exams::{self, ExamWarning, Termin, TerminAt, WarningKind};
use folia_timetable::export;
use folia_timetable::facts::SemesterFacts;
use folia_timetable::fit::{self, CandidateSet, Candidates, Verdict};
use folia_timetable::ics::{self, Calendar};
use folia_timetable::model::{Event, Input, Row, Timetable};
use folia_timetable::occur::Every;
use folia_timetable::views;
use serde::{Deserialize, Serialize};

pub use folia_plans::areas::{area_sections, catalog_areas, is_choice, is_structural, short_name, CatalogArea};

/// The ground at the end of every page: which Radix built the data, how fresh it is, and the
/// semester it is about. Cheap, so every page can have it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub meta: Meta,
    pub current_semester: Option<Semester>,
}

pub fn ground(db: &dyn Database) -> Result<Ground, DbError> {
    Ok(Ground { meta: queries::meta(db)?, current_semester: queries::semesters(db)?.into_iter().find(|s| s.is_current) })
}

/// The landing page: how big and how fresh the catalog is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overview {
    pub meta: Meta,
    pub current_semester: Option<Semester>,
    /// Modules the default catalog lists (what is no longer offered is hidden).
    pub modules: u64,
    /// Programs in their current PO version.
    pub programs: u64,
    /// Those of them whose study plan was read from their regulations and validated.
    pub plans: u64,
}

pub fn overview(db: &dyn Database) -> Result<Overview, DbError> {
    let programs = queries::programs(db)?;
    let current = || programs.iter().filter(|p| p.is_latest_po);
    Ok(Overview {
        meta: queries::meta(db)?,
        current_semester: queries::semesters(db)?.into_iter().find(|s| s.is_current),
        modules: queries::catalog_count(db, &CatalogQuery::default())?,
        programs: current().count() as u64,
        plans: current().filter(|p| p.has_plan).count() as u64,
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
/// The faculties are the derived ones of the program overview (`faculties`), grouped by their
/// number: only numbered departments are faculties, and a number the snapshot lists under two
/// names (before and after the restructuring) is named after the one with more modules.
pub fn program_map(db: &dyn Database) -> Result<crate::graph::ProgramMap, DbError> {
    use crate::graph::MapFaculty;
    let ProgramsData { programs, departments, faculties } = programs_overview(db)?;
    let programs: Vec<Program> = programs.into_iter().filter(|program| program.is_latest_po).collect();
    let code_of = |program: &Program| {
        let faculty = faculties.iter().find(|faculty| faculty.program_id == program.id)?;
        let department = departments.iter().find(|department| department.id == faculty.department_id)?;
        department.code.parse::<u32>().ok()
    };
    let codes: Vec<Option<u32>> = programs.iter().map(code_of).collect();
    let mut numbers: Vec<u32> = codes.iter().flatten().copied().collect();
    numbers.sort_unstable();
    numbers.dedup();
    let map_faculties: Vec<MapFaculty> = numbers
        .iter()
        .map(|number| {
            let named = departments.iter().filter(|d| d.code.parse::<u32>().ok() == Some(*number)).max_by_key(|d| (d.modules, -d.id));
            // „MINT - Mathematik, Informatik, …" is called „MINT".
            let name = named.map(|d| d.name_de.split(" - ").next().unwrap_or(&d.name_de).trim().to_string()).unwrap_or_default();
            MapFaculty { code: number.to_string(), name }
        })
        .collect();
    let faculty: Vec<Option<usize>> = codes.iter().map(|code| code.and_then(|code| numbers.iter().position(|n| *n == code))).collect();
    Ok(crate::graph::program_map(&programs, &queries::curriculum_links(db)?, map_faculties, &faculty))
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
    /// The areas of the selected program's module tree (empty without a program, or a tree).
    pub areas: Vec<CatalogArea>,
    /// With a semester of the program chosen: what the plan asks for in it besides the modules
    /// it places there, and what was listed for that (`plan::semester_plan`).
    pub semester_plan: Option<SemesterPlan>,
    /// The query the page ran: the URL's, with what the page derived filled in (the areas of the
    /// semester's requirements). Further pages of the list are loaded with this one.
    pub effective: CatalogQuery,
    /// For the name of the selected department (few rows; the long lists are `CatalogChoices`).
    pub departments: Vec<Department>,
    pub meta: Meta,
    /// With a search text: what it finds outside the list's other filters, said under the rows.
    pub elsewhere: Option<SearchElsewhere>,
}

/// The catalog of `url` for a page in `locale` (the credits of a semester's requirements are
/// written in it).
pub fn catalog(db: &dyn Database, url: &CatalogUrl, locale: Locale) -> Result<CatalogData, DbError> {
    let scope = catalog_scope(db, &url.query, locale)?;
    let elsewhere = if scope.effective.text.trim().is_empty() { None } else { Some(queries::search_elsewhere(db, &scope.effective)?) };
    Ok(CatalogData {
        elsewhere,
        page: queries::catalog_page(db, &scope.effective, url.offset(), PAGE_SIZE)?,
        plan_semesters: scope.plan_semesters,
        areas: scope.areas,
        semester_plan: scope.semester_plan,
        effective: scope.effective,
        program: scope.program,
        curricular_total: scope.curricular_total,
        fues_total: scope.fues_total,
        departments: queries::departments(db)?,
        meta: queries::meta(db)?,
    })
}

/// What the filter panel says about a filter, without the list: the program with the semesters
/// of its plan and its areas, the totals of the program's two lists, and how many modules the
/// filter holds. The filter sheet of a phone asks for this with every tap and has the list
/// loaded once, when it closes (`catalog`, which says the same about the same filter).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub program: Option<Program>,
    pub curricular_total: Option<u64>,
    pub fues_total: Option<u64>,
    pub plan_semesters: Vec<i64>,
    pub areas: Vec<CatalogArea>,
    pub total: u64,
}

pub fn catalog_summary(db: &dyn Database, query: &CatalogQuery) -> Result<CatalogSummary, DbError> {
    // The summary says nothing about a semester's requirements, only which areas they point at:
    // the language their credits are written in does not matter here.
    let scope = catalog_scope(db, query, Locale::default())?;
    // The list a program's filter chose has been counted with the program's two lists already.
    let counted = match (&scope.effective.program, &scope.program) {
        (Some(chosen), Some(_)) => match chosen.relation {
            ProgramRelation::Curricular => scope.curricular_total,
            ProgramRelation::Fues => scope.fues_total,
        },
        _ => None,
    };
    let total = match counted {
        Some(total) => total,
        None => queries::catalog_count(db, &scope.effective)?,
    };
    Ok(CatalogSummary {
        program: scope.program,
        curricular_total: scope.curricular_total,
        fues_total: scope.fues_total,
        plan_semesters: scope.plan_semesters,
        areas: scope.areas,
        total,
    })
}

/// How many of the modules closest to a search text the semantic search hands over for „Ähnliche
/// Module" (`similar`): the closest tenth of the catalog, of which a filter keeps its share.
pub const SIMILAR_CANDIDATES: usize = 500;
/// How many „Ähnliche Module" stand under the results at most (owner, 2026-10-01).
pub const SIMILAR_SHOWN: usize = 10;
/// From how many letters or digits on a search has „Ähnliche Module" (owner, 2026-10-01).
pub const SIMILAR_FROM: usize = 3;

/// Whether a search for `text` has „Ähnliche Module" under its results.
pub fn searches_similar(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphanumeric()).count() >= SIMILAR_FROM
}

/// What the semantic search is asked for the „Ähnliche Module" of `query` (the one the page ran):
/// its text as the list searched it, typos corrected („algoritmen" is „Algorithmen", which the
/// list says). `None` for a search that has none (`searches_similar`).
pub fn similar_text(query: &CatalogQuery) -> Option<String> {
    let text = query.text.trim();
    if !searches_similar(text) {
        return None;
    }
    Some(match &query.text_resolution {
        Some(resolution) if !resolution.corrected.is_empty() => resolution.searched_text(text),
        _ => text.to_string(),
    })
}

/// „Ähnliche Module" under the results of a search (owner, 2026-10-01: with every search, the
/// filters applying, at most 10): of the modules the semantic search finds closest to the text of
/// `query` (`hits`, the closest first), those the rest of its filter holds and the search itself
/// does not find (they are results already, and so is a module titled as one of them,
/// `queries::similar_rows`), the closest first, each title once (the closest of the modules that
/// bear it), at most `limit`. `query` is the one the page ran (`CatalogData::effective`); its text
/// is resolved as the list's where it is not.
pub fn similar(db: &dyn Database, query: &CatalogQuery, hits: &[String], limit: usize) -> Result<Vec<CatalogRow>, DbError> {
    if !searches_similar(&query.text) || hits.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut query = query.clone();
    if query.text_resolution.is_none() {
        query.text_resolution = Some(search::resolve(db, &query.text)?);
    }
    let mut rows: BTreeMap<String, CatalogRow> = queries::similar_rows(db, &query, hits)?.into_iter().map(|row| (row.id.clone(), row)).collect();
    let mut titles = BTreeSet::new();
    Ok(hits.iter().filter_map(|id| rows.remove(id)).filter(|row| titles.insert(row.title.clone())).take(limit).collect())
}

/// What a filter means before a row of the list is read (`catalog` and `catalog_summary`).
struct CatalogScope {
    program: Option<Program>,
    plan_semesters: Vec<i64>,
    areas: Vec<CatalogArea>,
    semester_plan: Option<SemesterPlan>,
    /// The query as it is run: the URL's, with what the program's plan says filled in.
    effective: CatalogQuery,
    curricular_total: Option<u64>,
    fues_total: Option<u64>,
}

fn catalog_scope(db: &dyn Database, query: &CatalogQuery, locale: Locale) -> Result<CatalogScope, DbError> {
    let program = match &query.program {
        Some(scope) => queries::program_by_slug(db, &scope.program_slug)?,
        None => None,
    };

    let plan_semesters = match &program {
        Some(program) if program.has_plan => queries::program_plan_semesters(db, &program.id)?,
        _ => Vec::new(),
    };
    let areas = match &program {
        Some(program) => catalog_areas(&queries::program_areas(db, &program.id)?, &queries::program_area_tree(db, &program.id)?),
        None => Vec::new(),
    };

    // A semester of the plan lists what the plan places there and what can be chosen for the
    // semester's requirements („Wahlpflichtmodule der Informatik"): the modules of the areas the
    // requirement's name points at, else every elective the plan places nowhere. The URL says
    // the semester; what that means is derived here and filled into the query (R12: the page
    // says that it is derived).
    let mut query = query.clone();
    // How the text is searched: as typed, or, where that finds nothing, with its typos corrected or
    // for the most of its words (`search::resolve`). The list says so above its rows.
    if query.text_resolution.is_none() && !query.text.trim().is_empty() {
        query.text_resolution = Some(search::resolve(db, &query.text)?);
    }
    let mut semester_plan = None;
    if let (Some(scope), Some(program)) = (query.program.as_mut(), program.as_ref()) {
        if let (Some(PlanSemesterFilter::Semester(semester)), true) = (scope.plan_semester, program.has_plan) {
            let plan = plan::semester_plan(semester, &queries::program_plan_entries(db, &program.id)?, &areas, locale);
            scope.semester_areas = plan.area_ids();
            scope.semester_electives = plan.any_elective();
            semester_plan = Some(plan);
        }
    }

    // The two lists of a program are shown as tabs, each with its exact total.
    let (mut curricular_total, mut fues_total) = (None, None);
    if let (Some(scope), Some(_)) = (&query.program, &program) {
        for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
            let other = CatalogQuery { program: Some(ProgramScope { relation, ..scope.clone() }), ..query.clone() };
            let total = Some(queries::catalog_count(db, &other)?);
            match relation {
                ProgramRelation::Curricular => curricular_total = total,
                ProgramRelation::Fues => fues_total = total,
            }
        }
    }

    Ok(CatalogScope { program, plan_semesters, areas, semester_plan, effective: query, curricular_total, fues_total })
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
    /// Where the validated study plans place the module (`plan_semesters`).
    pub plan_places: Vec<PlanPlace>,
    pub semesters: Vec<Semester>,
}

impl ModuleData {
    /// The semesters the validated plan of `program_id` places the module in, each once and in
    /// order: `[(1, 1)]`, `[(5, 6)]` for a span the regulation prints, `[(4, 4), (5, 5)]` where its
    /// study directions place it differently. Empty where no plan of the program names it.
    pub fn plan_semesters(&self, program_id: &str) -> Vec<(i64, i64)> {
        let mut spans: Vec<(i64, i64)> = self
            .plan_places
            .iter()
            .filter(|place| place.program_id == program_id)
            .filter_map(|place| plan::span_of(place.semester, place.start_semester, place.end_semester))
            .collect();
        spans.sort_unstable();
        spans.dedup();
        spans
    }
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
        plan_places: queries::module_plan_places(db, id)?,
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
    /// Every node of the module tree, those without modules included (`catalog_areas`).
    pub area_tree: Vec<AreaNode>,
    pub plan: Option<Plan>,
    pub plan_entries: Vec<PlanEntry>,
    /// What the regulation says its plan adds up to, and which rows each sum counts.
    pub plan_totals: Vec<PlanTotal>,
}

/// How many plans a program's validated study plan prints, one per study direction
/// (`variants::plan_variants`, as its page tells them apart): the pages of its plan
/// (`?variant=<n>`), as the sitemap lists them. 0 without a plan.
pub fn study_plans(db: &dyn Database, program_id: &str) -> Result<usize, DbError> {
    let entries = queries::program_plan_entries(db, program_id)?;
    if entries.is_empty() {
        return Ok(0);
    }
    // Only counted: the language of their labels does not matter.
    Ok(variants::plan_variants(&entries, &queries::program_plan_totals(db, program_id)?, Locale::default()).len())
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
        area_tree: queries::program_area_tree(db, &id)?,
        plan: queries::program_plan(db, &id)?,
        plan_entries: queries::program_plan_entries(db, &id)?,
        plan_totals: queries::program_plan_totals(db, &id)?,
        program,
    }))
}

/// How many marked modules a browser keeps. Far more than anybody marks, and well below what a
/// single `IN (…)` of the local SQLite takes.
pub const MAX_BOOKMARKS: usize = 2000;

/// The visitor's marked modules („Merkliste"). Which modules these are is personal: the list
/// lives in the browser, and only the browser app calls this, with the ids it has stored. The
/// server never sees them (R9).
///
/// The page gets every module once and decides itself what to show: a change of the half of the
/// year, and a mark given or taken away, need no further query.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BookmarksData {
    /// The modules the snapshot knows, in the order asked for.
    pub rows: Vec<CatalogRow>,
    /// Which of them are offered in winter and which in summer, as the catalog's turnus filter
    /// sees it (a module offered every semester is in both, one without a stated turnus in none).
    pub winter: Vec<String>,
    pub summer: Vec<String>,
    /// Ids the snapshot does not know (no longer part of the BTU's catalog, or marked with a
    /// newer snapshot than the local copy), in the order given.
    pub missing: Vec<String>,
}

impl BookmarksData {
    pub fn offered_in(&self, season: Season, id: &str) -> bool {
        let offered = match season {
            Season::Winter => &self.winter,
            Season::Summer => &self.summer,
        };
        offered.iter().any(|offered| offered == id)
    }
}

/// `ids` in the order of marking, the newest first: that is the order `BookmarkSort::Added` shows.
pub fn bookmarks(db: &dyn Database, ids: &[String], sort: BookmarkSort, descending: bool) -> Result<BookmarksData, DbError> {
    // What comes from a browser's storage is checked like what comes from a URL.
    let mut seen = std::collections::BTreeSet::new();
    let ids: Vec<String> = ids.iter().filter(|id| folia_model::ids::is_module_id(id) && seen.insert(id.as_str())).take(MAX_BOOKMARKS).cloned().collect();
    if ids.is_empty() {
        return Ok(BookmarksData::default());
    }
    let limit = ids.len() as u64;
    let marked = |season: Option<Season>| CatalogQuery {
        only_ids: Some(ids.clone()),
        // A marked module stays on the list when it is no longer offered.
        offer: Some(OfferStatus::ALL.to_vec()),
        turnus: TurnusFilter { winter: season == Some(Season::Winter), summer: season == Some(Season::Summer), ..Default::default() },
        sort: match sort {
            BookmarkSort::Added | BookmarkSort::Title => SortKey::Title,
            BookmarkSort::Credits => SortKey::Credits,
            BookmarkSort::Events => SortKey::Events,
        },
        descending: descending && sort != BookmarkSort::Added,
        ..Default::default()
    };
    let offered_in = |season: Season| -> Result<Vec<String>, DbError> {
        Ok(queries::catalog_page(db, &marked(Some(season)), 0, limit)?.rows.into_iter().map(|row| row.id).collect())
    };

    let mut rows = queries::catalog_page(db, &marked(None), 0, limit)?.rows;
    if sort == BookmarkSort::Added {
        let place = |id: &str| ids.iter().position(|marked| marked == id).unwrap_or(usize::MAX);
        rows.sort_by_key(|row| place(&row.id));
    }
    Ok(BookmarksData {
        missing: ids.iter().filter(|id| !rows.iter().any(|row| row.id == **id)).cloned().collect(),
        winter: offered_in(Season::Winter)?,
        summer: offered_in(Season::Summer)?,
        rows,
    })
}

/// One semester of a Studienplan: the planned modules' rows, and what the semester's timetable is
/// derived from. Which modules these are is personal, so only two callers load it: the browser
/// with the ids of its store, and the server with the ids a subscription code carries (R9).
#[derive(Clone, Debug, PartialEq)]
pub struct StudyplanData {
    pub key: SemesterKey,
    /// The language the data was loaded for: `label` is written in it, and so is what is written
    /// from the data (the lines of `overlay`).
    pub locale: Locale,
    /// The semester's name in `locale` („WiSe 2026/27", "Winter 2026/27").
    pub label: String,
    /// The semester's `v_semester` row; `None` for a semester the snapshot does not have.
    pub semester: Option<Semester>,
    pub meta: Meta,
    /// The requested ids that look like a module id, each once, in the order asked for: the
    /// plan's order, which the timetable's tones and events follow. Every query takes them sorted
    /// (`id_json`, `catalog_rows`), so a plan reordered asks the same questions.
    pub ids: Vec<String>,
    /// The catalog's rows of those ids, whatever their offer status, by title (`catalog_page`).
    pub modules: Vec<CatalogRow>,
    /// The ids the catalog does not know, in the order asked for.
    pub missing: Vec<String>,
    /// The modules' teaching rows of the semester (`modules_schedule`).
    pub schedule: Vec<DateRow>,
    /// Their exam rows of the semester (`modules_exams`).
    pub exams: Vec<DateRow>,
    /// Their SWS per teaching form (`modules_teaching_sws`).
    pub sws: Vec<ModuleSws>,
    /// The semester's dated teaching rows counted by rhythm and range: the evidence for its
    /// lecture period, breaks and A weeks (`SemesterFacts::derive`).
    pub counts: Vec<DateCount>,
    /// The modules' abbreviations („EvS", schema 9), by id: within the plan's program where it has
    /// one (`studyplan_in`), else each module's own. Only a week grid and the calendar's entries
    /// name modules by them (`slot_names`); everywhere else the title stands (owner, 2026-09-25).
    pub abbrevs: BTreeMap<String, String>,
    /// The program `abbrevs` are of, as `studyplan_in` was asked for it (an id of another shape
    /// left out). A subscription carries it, so the feed names the modules as the page does.
    pub program: Option<String>,
}

/// The Studienplan of `ids` in semester `key`, for a page in `locale`. For the browser (ids from
/// the store) and the server (ids from a code) only (R9). Without ids no id query runs; the
/// semester's facts are loaded all the same.
pub fn studyplan(db: &dyn Database, key: SemesterKey, ids: &[String], locale: Locale) -> Result<StudyplanData, DbError> {
    studyplan_in(db, key, ids, None, locale)
}

/// `studyplan` for a plan of `program_id`: the week grid and the calendar name its modules by the
/// abbreviations unique within that program (`queries::modules_abbrevs`), not only by each
/// module's own. An id that is no program id (`url::is_program_id`) is none.
pub fn studyplan_in(db: &dyn Database, key: SemesterKey, ids: &[String], program_id: Option<&str>, locale: Locale) -> Result<StudyplanData, DbError> {
    let ids = checked_ids(ids);
    let program_id = program_id.filter(|id| folia_model::ids::is_program_id(id));
    let semester_key = key.key();
    let semester = queries::semesters(db)?.into_iter().find(|semester| semester.key == semester_key);
    let (modules, missing) = catalog_rows(db, &ids)?;
    Ok(StudyplanData {
        key,
        locale,
        label: key.label(locale),
        meta: queries::meta(db)?,
        counts: queries::semester_date_counts(db, &semester_key)?,
        schedule: queries::modules_schedule(db, &ids, &semester_key)?,
        exams: queries::modules_exams(db, &ids, &semester_key)?,
        sws: queries::modules_teaching_sws(db, &ids)?,
        abbrevs: queries::modules_abbrevs(db, &ids, program_id)?.into_iter().map(|a| (a.module_id, a.abbrev)).collect(),
        program: program_id.map(str::to_string),
        semester,
        ids,
        modules,
        missing,
    })
}

impl StudyplanData {
    /// The semester's lecture period, breaks, A weeks and holidays, as its own rows say.
    pub fn facts(&self) -> SemesterFacts {
        SemesterFacts::derive(self.key, self.semester.as_ref(), &self.counts)
    }

    /// The semester's timetable of the loaded modules, in the order of `ids`, as `selection`
    /// shows it. Pure: a changed selection needs no query.
    pub fn timetable(&self, selection: &Selection) -> Timetable {
        let facts = self.facts();
        let input = Input {
            key: self.key,
            semester: self.semester.as_ref(),
            facts: &facts,
            modules: &self.ids,
            schedule: &self.schedule,
            exams: &self.exams,
            sws: &self.sws,
        };
        Timetable::build(&input, selection)
    }

    /// The title of each module the catalog knows, by id: what a calendar entry names its
    /// modules by. Built the same way on both paths, so the download and the feed agree.
    pub fn titles(&self) -> BTreeMap<String, String> {
        self.modules.iter().map(|row| (row.id.clone(), row.title.clone())).collect()
    }

    /// What a slot of a week grid names each module by, where no title fits: its abbreviation,
    /// else its title cut short (`views::short_title`). An abbreviation two planned modules share
    /// (each module's own list is unique only within a program) names neither.
    pub fn slot_names(&self) -> BTreeMap<String, String> {
        slot_names(&self.modules, &self.abbrevs)
    }

    /// The calendar of `table`, a timetable of this data, before it is written: the page asks
    /// whether it has entries at all before it offers a download. Its entries name the modules as
    /// the week's slots do (`slot_names`). Written in `locale`.
    pub fn calendar(&self, table: &Timetable, locale: Locale) -> Calendar {
        export::calendar_of(table, &self.titles(), &self.slot_names(), &self.label, &export::snapshot_stamp(&self.meta), locale)
    }

    /// The calendar text of `table`: the feed and the download both call this, so they are
    /// byte-identical in one language.
    pub fn ics(&self, table: &Timetable, locale: Locale) -> String {
        ics::write(&self.calendar(table, locale))
    }

    /// The same data with one more module's rows (a module page's overlay): no new SQL for the
    /// planned set. The module joins `ids` at the end; rows it had already are replaced, so its
    /// SWS count once, and rows of other modules in the arguments are left out. `modules` and
    /// `missing` stay those of the plan. An id that is no module id changes nothing.
    pub fn with_module(&self, id: &str, schedule: Vec<DateRow>, exams: Vec<DateRow>, sws: Vec<ModuleSws>) -> StudyplanData {
        let mut data = self.clone();
        if !folia_model::ids::is_module_id(id) {
            return data;
        }
        data.schedule.retain(|row| row.module_id != id);
        data.exams.retain(|row| row.module_id != id);
        data.sws.retain(|row| row.module_id != id);
        data.schedule.extend(schedule.into_iter().filter(|row| row.module_id == id));
        data.exams.extend(exams.into_iter().filter(|row| row.module_id == id));
        data.sws.extend(sws.into_iter().filter(|row| row.module_id == id));
        if !data.ids.iter().any(|known| known == id) {
            data.ids.push(id.to_string());
        }
        data
    }
}

/// `StudyplanData::slot_names` of `modules` with their `abbrevs`.
fn slot_names(modules: &[CatalogRow], abbrevs: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    for abbrev in abbrevs.values() {
        *uses.entry(abbrev.to_lowercase()).or_default() += 1;
    }
    let unique = |abbrev: &&String| uses.get(&abbrev.to_lowercase()) == Some(&1);
    modules
        .iter()
        .map(|row| {
            let name = abbrevs.get(&row.id).filter(unique).cloned().unwrap_or_else(|| views::short_title(&row.title));
            (row.id.clone(), name)
        })
        .collect()
}

/// The catalog's rows of planned modules outside the semester a page loaded (a module beside the
/// plan that the plan holds elsewhere, what counts for a placeholder's row in its other semesters),
/// by title, and the ids the catalog does not know.
pub fn studyplan_modules(db: &dyn Database, ids: &[String]) -> Result<(Vec<CatalogRow>, Vec<String>), DbError> {
    catalog_rows(db, &checked_ids(ids))
}

/// Module ids as a store or a code hands them in, checked like a URL's: those that look like one,
/// each once, in their order, at most as many as one query takes.
fn checked_ids(ids: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    ids.iter()
        .filter(|id| folia_model::ids::is_module_id(id) && seen.insert(id.as_str()))
        .take(queries::MAX_PLANNED)
        .cloned()
        .collect()
}

/// The catalog's rows of `ids` whatever their offer status (a planned module stays when it is no
/// longer offered), by title, as `bookmarks` loads them; and the ids without a row, in order.
fn catalog_rows(db: &dyn Database, ids: &[String]) -> Result<(Vec<CatalogRow>, Vec<String>), DbError> {
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // The ids go into the SQL sorted and each once: the browser's answer cache keys on the SQL and
    // its parameters, so a plan that is only reordered is answered from the visit's cache. The
    // rows come by title either way.
    let only: Vec<String> = ids.iter().collect::<BTreeSet<_>>().into_iter().cloned().collect();
    let limit = only.len() as u64;
    let query = CatalogQuery {
        only_ids: Some(only),
        offer: Some(OfferStatus::ALL.to_vec()),
        sort: SortKey::Title,
        ..Default::default()
    };
    let rows = queries::catalog_page(db, &query, 0, limit)?.rows;
    let missing = ids.iter().filter(|id| !rows.iter().any(|row| row.id == **id)).cloned().collect();
    Ok((rows, missing))
}

/// What the import and a placeholder need of a program: its study plans as the regulation prints
/// them, which of them fill a row of another, its areas, and the catalog's rows of the modules
/// its plans name.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanSource {
    pub program: Program,
    /// `variants::plan_variants`, in the order of the document; empty for a program without a plan.
    pub variants: Vec<PlanVariant>,
    /// `variants::supplements` of `variants`.
    pub supplements: Vec<Supplement>,
    /// The areas of the program's module tree (`catalog_areas`), for `plan::areas_for_row`.
    pub areas: Vec<CatalogArea>,
    /// The catalog's rows of every module a plan row names, whatever their offer status, by title
    /// (their turnus decides the intake season where the plan's caption does not name it,
    /// `studyplan::intake_season`).
    pub linked: Vec<CatalogRow>,
}

/// The plans of the program with `program_id` (`program.id`, never the slug: the store keeps
/// the id), labelled in `locale`, or `None` when the snapshot has no such program.
pub fn plan_source(db: &dyn Database, program_id: &str, locale: Locale) -> Result<Option<PlanSource>, DbError> {
    let Some(program) = queries::programs(db)?.into_iter().find(|program| program.id == program_id) else {
        return Ok(None);
    };
    let entries = queries::program_plan_entries(db, &program.id)?;
    let variants = variants::plan_variants(&entries, &queries::program_plan_totals(db, &program.id)?, locale);
    let areas = catalog_areas(&queries::program_areas(db, &program.id)?, &queries::program_area_tree(db, &program.id)?);
    let named: Vec<String> = entries.iter().filter_map(|entry| entry.module_id.clone()).collect();
    let (linked, _) = catalog_rows(db, &checked_ids(&named))?;
    Ok(Some(PlanSource { supplements: variants::supplements(&variants), program, variants, areas, linked }))
}

/// „Mein Studiengang" as the snapshot has it.
#[derive(Clone, Debug, PartialEq)]
pub struct MyProgramInfo {
    /// The stored program; when it is gone from the snapshot, the newest PO of its family.
    pub program: Program,
    /// The stored id is in the snapshot. Only then does the app set defaults from it (A.10).
    pub exact: bool,
    /// The newest PO of the family (the id's first two parts, `079-82`) when it is another
    /// program than the stored one: what „PO 2022 übernehmen" offers, and what a placeholder of a
    /// program gone from the snapshot looks its electives up in.
    pub latest: Option<Program>,
}

/// The program of a stored id, or of its family when the id is gone; `None` when neither is in
/// the snapshot.
pub fn my_program(db: &dyn Database, program_id: &str) -> Result<Option<MyProgramInfo>, DbError> {
    let programs = queries::programs(db)?;
    let exact = programs.iter().find(|program| program.id == program_id);
    let family = match exact {
        Some(program) => program.family_key.clone(),
        None => match program_id.split('-').collect::<Vec<_>>().as_slice() {
            [first, second, _, ..] => format!("{first}-{second}"),
            _ => return Ok(None),
        },
    };
    // Radix flags one PO per family as the newest; the year decides should it flag none or two.
    let latest = programs
        .iter()
        .filter(|program| program.family_key == family)
        .max_by_key(|program| (program.is_latest_po, program.po_year, program.id.as_str()));
    Ok(match exact {
        Some(program) => Some(MyProgramInfo {
            program: program.clone(),
            exact: true,
            latest: latest.filter(|latest| latest.id != program.id).cloned(),
        }),
        None => latest.map(|latest| MyProgramInfo { program: latest.clone(), exact: false, latest: Some(latest.clone()) }),
    })
}

/// What „Passt in meinen Stundenplan" leaves in the catalog (A.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FitResult {
    /// The semester has a dated teaching row. Without one nothing is checked, and only the
    /// planned modules are left out.
    pub has_data: bool,
    /// The modules checked and not clashing (`Verdict::Fits`, `Partly`, `Unknown`), without the
    /// planned ones, by id.
    pub fitting: Vec<String>,
    /// The modules that clash and the planned ones, by id, each once.
    pub excluded: Vec<String>,
    /// The row notes of partial fits and unknowns („Übung 1 von 3 frei", „keine festen Termine"),
    /// by module id, as the finder words them.
    pub notes: BTreeMap<String, String>,
    /// The listed modules that could not be checked (`Verdict::Unknown`: nothing with a time to
    /// compare, only retakes). Their notes say why, and a list shows them quietly: unlike a
    /// partial fit, they warn of nothing. By the verdict, so a note worded anew keeps its kind.
    pub unknown: BTreeSet<String>,
}

/// The finder for the plan of `plan_ids` (the planned modules of `filter.semester`) as
/// `selection` shows it. `cache`: the plan-independent `CandidateSet`, reused while its key
/// matches (the app keeps it between calls), so a changed plan re-runs only `fit::fits`. The
/// notes are in `locale`.
pub fn fit(
    db: &dyn Database,
    filter: &FitsFilter,
    plan_ids: &[String],
    selection: &Selection,
    cache: &mut Option<CandidateSet>,
    locale: Locale,
) -> Result<FitResult, DbError> {
    let mut seen = BTreeSet::new();
    let planned: Vec<String> = plan_ids.iter().filter(|id| seen.insert(id.as_str())).cloned().collect();
    let without_data = || FitResult { excluded: seen.iter().map(|id| id.to_string()).collect(), ..FitResult::default() };
    let Some(key) = SemesterKey::parse(&filter.semester) else {
        return Ok(without_data());
    };
    let semester_key = key.key();
    let schedule = queries::semester_schedule(db, &semester_key)?;
    if schedule.is_empty() {
        return Ok(without_data());
    }
    let exams = queries::semester_exams(db, &semester_key)?;
    let sws = queries::semester_teaching_sws(db, &semester_key)?;
    let semester = queries::semesters(db)?.into_iter().find(|semester| semester.key == semester_key);
    let facts = SemesterFacts::derive(key, semester.as_ref(), &queries::semester_date_counts(db, &semester_key)?);

    // The plan from the semester's own rows: the timetable keeps the rows of its modules only.
    let input = Input {
        key,
        semester: semester.as_ref(),
        facts: &facts,
        modules: &planned,
        schedule: &schedule,
        exams: &exams,
        sws: &sws,
    };
    let plan = Timetable::build(&input, selection);
    let options = filter.options();
    let wanted = (key, options, selection.hidden_kinds, plan.town);
    let set = match cache.take() {
        Some(set) if set.key == wanted => set,
        _ => {
            let rows = Candidates { schedule: &schedule, exams: &exams, sws: &sws };
            fit::candidates(&rows, &facts, semester.as_ref(), selection, plan.town, options)
        }
    };
    let verdicts = fit::fits(&plan, &set, locale);
    *cache = Some(set);

    let mut result = FitResult { has_data: true, ..FitResult::default() };
    let mut excluded: BTreeSet<String> = planned.into_iter().collect();
    for verdict in verdicts {
        if verdict.verdict == Verdict::Clashes {
            excluded.insert(verdict.module_id);
            continue;
        }
        if let Some(note) = verdict.note {
            result.notes.insert(verdict.module_id.clone(), note);
        }
        if verdict.verdict == Verdict::Unknown {
            result.unknown.insert(verdict.module_id.clone());
        }
        result.fitting.push(verdict.module_id);
    }
    result.excluded = excluded.into_iter().collect();
    Ok(result)
}

/// A module's week beside the Studienplan of its semester (A.9): the other planned modules'
/// Termine, the module's own Termine that clash with them, and the lines under its week and under
/// its exams.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// The planned modules' shown recurring Termine with a weekday and a time, each slot once, by
    /// weekday and time.
    pub planned: Vec<OverlaySlot>,
    /// The module's own rows in a hard clash with the plan.
    pub clashing: BTreeSet<RowKey>,
    /// The line under the week: `(true, „Überschneidet sich mit: …")` warns; a hint says „Passt in
    /// deinen Stundenplan (WiSe 2026/27)" or „Passt mit Übung Do 13:45". `None` when the module
    /// has no shown Termin with a time to compare.
    pub line: Option<(bool, String)>,
    /// The line under „Prüfungstermine": the module's exam warnings against the plan. `(true, …)`
    /// warns, as one of them is hard (no Termin of either module avoids it); soft ones alone
    /// („… · Erstermin 25.02. passt") are a quiet note (A.5).
    pub exam_line: Option<(bool, String)>,
}

/// A recurring slot of another planned module in a module's week: the module, its short name for
/// the slot's label (`StudyplanData::slot_names`: its abbreviation), the weekday (1 = Monday) and the minutes.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySlot {
    pub module: String,
    pub short: String,
    pub day: u8,
    pub from: u16,
    pub to: u16,
}

/// The overlay of `module_id` on `plan`, which is `studyplan(key, planned)` of the module's
/// semester with the module left out: the same ids for every preview, so its answers come from
/// the visit's cache, and only the module's own three single-id queries are new. The caller
/// decides whether an overlay is wanted at all (A.9: the semester is the current one or later,
/// and the plan holds other modules in it). Its lines are in the language of `plan`
/// (`StudyplanData::locale`), whose semester name they carry.
pub fn overlay(db: &dyn Database, plan: &StudyplanData, module_id: &str, selection: &Selection) -> Result<Overlay, DbError> {
    let id = [module_id.to_string()];
    let semester = plan.key.key();
    let data = plan.with_module(
        module_id,
        queries::modules_schedule(db, &id, &semester)?,
        queries::modules_exams(db, &id, &semester)?,
        queries::modules_teaching_sws(db, &id)?,
    );
    let table = data.timetable(selection);
    Ok(overlay_of(&table, &data.titles(), &data.slot_names(), &data.label, module_id, data.locale))
}

/// How many modules the clash line names before it counts the rest („+2").
const CLASH_NAMES: usize = 3;

/// The overlay of `module_id` in a timetable of the plan and the module together, its lines in
/// `locale`. Its own events are those it links; a clash is one the Studienplan would name once
/// the module is planned.
fn overlay_of(t: &Timetable, titles: &BTreeMap<String, String>, names: &BTreeMap<String, String>, label: &str, module_id: &str, locale: Locale) -> Overlay {
    let own = |event: &Event| event.modules.iter().any(|module| module == module_id);
    let title = |id: &str| titles.get(id).cloned().unwrap_or_else(|| id.to_string());

    let clashing = clash::hard_rows(&t.events)
        .into_iter()
        .filter_map(|(event, row)| t.events.get(event).filter(|event| own(event))?.rows.get(row)?.key)
        .collect();

    let mut planned: Vec<OverlaySlot> = Vec::new();
    for event in t.events.iter().filter(|event| event.hidden.is_none() && !own(event)) {
        let Some(module) = event.modules.first() else {
            continue;
        };
        for row in event.rows.iter().filter(|row| row.hidden.is_none() && held(row) && Every::of(&row.date).is_some()) {
            let (Some(day), Some(from), Some(to)) = (weekday_of(row), row.from, row.to) else {
                continue;
            };
            let short = names.get(module).cloned().unwrap_or_else(|| views::short_title(&title(module)));
            let slot = OverlaySlot { module: module.clone(), short, day, from, to };
            if !planned.contains(&slot) {
                planned.push(slot);
            }
        }
    }
    planned.sort_by_key(|slot| (slot.day, slot.from, slot.to));

    let timed = t
        .events
        .iter()
        .filter(|event| event.hidden.is_none() && own(event))
        .any(|event| event.rows.iter().any(|row| row.hidden.is_none() && row.from.is_some() && held(row)));
    let line = timed.then(|| match clash_line(t, &own, &title, locale) {
        Some(text) => (true, text),
        None => (false, choice_line(t, &own, locale).unwrap_or_else(|| (folia_plans::i18n::texts(locale).fits_plan)(label))),
    });

    let mut warnings: Vec<&ExamWarning> =
        t.exam_warnings.iter().filter(|w| w.a.module_id == module_id || w.b.module_id == module_id).collect();
    warnings.sort_by_key(|w| (!w.hard, w.day));
    let termine = if warnings.is_empty() { Vec::new() } else { exams::termine(&t.exams, &t.modules) };
    let hard = warnings.iter().any(|w| w.hard);
    let mut texts: Vec<String> = Vec::new();
    for warning in warnings {
        let text = exam_text(warning, module_id, &termine, &title, locale);
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    let exam_line = (!texts.is_empty()).then(|| (hard, texts.join("; ")));

    Overlay { planned, clashing, line, exam_line }
}

/// A row has a day to meet another on: held days, or a pattern of its weekday.
fn held(row: &Row) -> bool {
    !row.occ.days.is_empty() || row.occ.template.is_some()
}

/// A row's weekday as QIS states it, 1 = Monday.
fn weekday_of(row: &Row) -> Option<u8> {
    row.date.weekday.and_then(|weekday| u8::try_from(weekday).ok()).filter(|weekday| (1..=7).contains(weekday))
}

/// „Di" for 2 in German, "Tue" in English.
fn weekday_name(weekday: u8, locale: Locale) -> &'static str {
    locale.texts().weekday_short(i64::from(weekday)).unwrap_or("")
}

/// „Überschneidet sich mit: Analysis I (Di 09:15), Mathematik IT-1 (Di 13:45)": the planned
/// modules whose Termine the module's own meet in a hard clash, each once at its earliest time, at
/// most `CLASH_NAMES` of them and then how many more.
fn clash_line(t: &Timetable, own: &dyn Fn(&Event) -> bool, title: &dyn Fn(&str) -> String, locale: Locale) -> Option<String> {
    let mut met: Vec<(u8, u16, &str)> = Vec::new();
    for clash in &t.clashes {
        let (Some(a), Some(b)) = (t.events.get(clash.a.0), t.events.get(clash.b.0)) else {
            continue;
        };
        let theirs = match (own(a), own(b)) {
            (true, false) => clash.b,
            (false, true) => clash.a,
            _ => continue,
        };
        let Some(event) = t.events.get(theirs.0) else {
            continue;
        };
        let (Some(module), Some(from)) = (event.modules.first(), event.rows.get(theirs.1).and_then(|row| row.from)) else {
            continue;
        };
        met.push((clash.first.weekday(), from, module.as_str()));
    }
    met.sort();
    let mut names: Vec<String> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (weekday, from, module) in met {
        if seen.insert(module) {
            names.push(format!("{} ({} {})", title(module), weekday_name(weekday, locale), clock(from)));
        }
    }
    if names.is_empty() {
        return None;
    }
    let more = names.len().saturating_sub(CLASH_NAMES);
    names.truncate(CLASH_NAMES);
    let mut line = (folia_plans::i18n::texts(locale).clashes_with)(&names.join(", "));
    if more > 0 {
        line.push_str(&format!(" +{more}"));
    }
    Some(line)
}

/// „Passt mit Übung Do 13:45": the module fits only in some options of its own open choices, and
/// these are the free ones. An option is free when none of its shown Termine meets a shown Termin
/// of the plan, open options of the plan's choices included: this names the times that leave the
/// plan as it is. `None` when no choice of the module is cut short.
fn choice_line(t: &Timetable, own: &dyn Fn(&Event) -> bool, locale: Locale) -> Option<String> {
    let texts = folia_plans::i18n::texts(locale);
    let mut parts: Vec<String> = Vec::new();
    for event in t.events.iter().filter(|event| event.hidden.is_none() && own(event) && event.unresolved()) {
        let others: Vec<&Row> = t
            .events
            .iter()
            .filter(|other| other.hidden.is_none() && !other.modules.iter().any(|module| event.modules.contains(module)))
            .flat_map(|other| other.rows.iter().filter(|row| row.hidden.is_none()))
            .collect();
        let options = event.visible_options();
        let free: Vec<usize> = options
            .iter()
            .copied()
            .filter(|option| {
                !event
                    .rows
                    .iter()
                    .filter(|row| row.option == Some(*option) && row.hidden.is_none())
                    .any(|row| others.iter().any(|other| meets(row, other)))
            })
            .collect();
        if free.is_empty() || free.len() == options.len() {
            continue;
        }
        // By time, not by option: the options follow the order of their groups in the source.
        let times: BTreeSet<(u8, u16)> = free.into_iter().filter_map(|option| option_time(event, option)).collect();
        if !times.is_empty() {
            let times: Vec<String> =
                times.into_iter().map(|(weekday, from)| format!("{} {}", weekday_name(weekday, locale), clock(from))).collect();
            parts.push(format!("{} {}", kind_word(event, locale), times.join(&format!(" {} ", texts.or))));
        }
    }
    (!parts.is_empty()).then(|| (texts.fits_with)(&parts.join(" · ")))
}

/// Two timed rows overlap in time on a common day, as `clash` compares them.
fn meets(a: &Row, b: &Row) -> bool {
    match (a.from, a.to, b.from, b.to) {
        (Some(a_from), Some(a_to), Some(b_from), Some(b_to)) => {
            a_from < b_to && b_from < a_to && clash::shared(a, b).is_some()
        }
        _ => false,
    }
}

/// The weekday and start of the earliest shown Termin with a time of an option („Do 13:45").
fn option_time(event: &Event, option: usize) -> Option<(u8, u16)> {
    event
        .rows
        .iter()
        .filter(|row| row.option == Some(option) && row.hidden.is_none())
        .filter_map(|row| {
            let weekday = weekday_of(row).or_else(|| row.occ.days.first().map(|day| day.weekday()))?;
            Some((weekday, row.from?))
        })
        .min()
}

/// What an event is called in a line: its type as QIS writes it, else its first kind in `locale`.
fn kind_word(event: &Event, locale: Locale) -> String {
    match event.type_raw.as_deref().map(str::trim).filter(|kind| !kind.is_empty()) {
        Some(kind) => kind.to_string(),
        None => event.kinds.iter().next().map_or(folia_plans::i18n::texts(locale).session, |kind| kind.label(locale)).to_string(),
    }
}

/// An exam warning as the module page says it: „Prüfung gleichzeitig mit Mathematik IT-1
/// (10.03.2027 11:00)", „45 min bis Senftenberg nach Kraftwerkstechnik I" (the module's exam is
/// the later one) or „… zu Kraftwerkstechnik I" (the earlier one); an avoidable one adds what
/// avoids it (`avoid_text`). `termine`: the plan's Termine by module (`exams::termine`).
fn exam_text(
    warning: &ExamWarning,
    module_id: &str,
    termine: &[(String, Vec<TerminAt>)],
    title: &dyn Fn(&str) -> String,
    locale: Locale,
) -> String {
    let texts = folia_plans::i18n::texts(locale);
    let mine_first = warning.a.module_id == module_id;
    let (mine, other) = if mine_first { (&warning.a, &warning.b) } else { (&warning.b, &warning.a) };
    let name = title(&other.module_id);
    let mut text = match &warning.kind {
        WarningKind::Overlap => (texts.exam_overlap)(&name, &format!("{} {}", warning.day.date(locale), clock(other.from))),
        WarningKind::Tight { gap, to, .. } if mine_first => (texts.exam_tight_before)(*gap, to.label(locale), &name),
        WarningKind::Tight { gap, to, .. } => (texts.exam_tight_after)(*gap, to.label(locale), &name),
    };
    if let Some(avoid) = warning.avoid.filter(|_| !warning.hard) {
        let (mine, theirs) = ((mine, termine_of(termine, module_id)), (other, termine_of(termine, &other.module_id)));
        text.push_str(" · ");
        text.push_str(&avoid_text(warning.day, avoid, mine, theirs, &name, locale));
    }
    text
}

/// The Termine of `module` in `exams::termine`'s list, earliest first.
fn termine_of<'a>(termine: &'a [(String, Vec<TerminAt>)], module: &str) -> &'a [TerminAt] {
    termine.iter().find(|(id, _)| id == module).map_or(&[], |(_, list)| list.as_slice())
}

/// What avoids a soft exam warning on `day`, said from the module's side. `avoid` is only a day:
/// of the module's own Termin, of the other module's, or the later of both when only a change of
/// both avoids the issue (`exams::avoiding`); so the Termine on that day are looked up. The
/// module's own, free of the other's Termin in the warning, is its „Zweittermin 11.03. passt", or
/// „Erstermin 25.02. passt" when it is its earliest (the warning is about a later one); the other
/// module's, free of the module's, is „Mathematik IT-1 am 25.02. passt"; otherwise „andere Termine
/// passen".
fn avoid_text(
    day: Day,
    avoid: Day,
    mine: (&Termin, &[TerminAt]),
    theirs: (&Termin, &[TerminAt]),
    name: &str,
    locale: Locale,
) -> String {
    let texts = folia_plans::i18n::texts(locale);
    let both = || texts.other_dates_fit.to_string();
    let my_issue = mine.1.iter().find(|at| at.day == day && at.termin == *mine.0);
    let their_issue = theirs.1.iter().find(|at| at.day == day && at.termin == *theirs.0);
    let (Some(my_issue), Some(their_issue)) = (my_issue, their_issue) else {
        return both();
    };
    let instead = |list: &[TerminAt], issue: &TerminAt, against: &TerminAt| {
        list.iter().position(|at| at.day == avoid && at != issue && exams::collision(at, against).is_none())
    };
    if let Some(index) = instead(mine.1, my_issue, their_issue) {
        let fits = if index == 0 { texts.first_sitting_fits } else { texts.second_sitting_fits };
        return fits(&avoid.day_month(locale));
    }
    if instead(theirs.1, their_issue, my_issue).is_some() {
        return (texts.other_sitting_fits)(name, &avoid.day_month(locale));
    }
    both()
}

/// The calendar text of a subscription (the server's feed): the timetable is made anew from the
/// snapshot for the code's semester, modules, program and hide rules, so exams QIS publishes later
/// arrive by themselves. A semester the snapshot does not have is a calendar without entries that
/// says so. Written in `locale`, the language of the feed's address.
pub fn calendar(db: &dyn Database, subscription: &Subscription, locale: Locale) -> Result<String, DbError> {
    let Some(key) = subscription.key() else {
        // A code decodes only with a semester; a subscription made by hand without one has none.
        let stamp = export::snapshot_stamp(&queries::meta(db)?);
        let name = (folia_timetable::i18n::texts(locale).feed_name)("");
        return Ok(ics::write(&Calendar { name, stamp, ..Calendar::default() }));
    };
    let data = studyplan_in(db, key, &subscription.module_ids(), subscription.program.as_deref(), locale)?;
    let table = data.timetable(&subscription.selection());
    Ok(data.ics(&table, locale))
}

/// A plan handed on by a link (`timetable::share`) as its link preview and the offer to take it
/// over name it.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedPlanData {
    pub key: SemesterKey,
    /// The semester's name in the language `shared_plan` was asked for („WiSe 2026/27", "Winter
    /// 2026/27").
    pub label: String,
    /// The modules the catalog knows, in the order they were planned.
    pub modules: Vec<SharedModule>,
    /// The code's modules the catalog does not know (any more), in the plan's order.
    pub missing: Vec<String>,
    /// The program whose abbreviations name the modules, while the snapshot has it.
    pub program: Option<Program>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SharedModule {
    pub id: String,
    /// What the week grid names the module by: its abbreviation (the program's where the code names
    /// one, else its own), else its title cut short (`StudyplanData::slot_names`).
    pub name: String,
    pub title: String,
    pub credits: Option<f64>,
}

impl SharedPlanData {
    /// The credits of the modules that state them.
    pub fn credits(&self) -> f64 {
        self.modules.iter().filter_map(|module| module.credits).sum()
    }
}

/// What a shared plan's code names, from the snapshot: its semester's name in `locale`, its
/// modules as the week grid names them, with title and credits, and its program. `None` for a
/// plan without a semester (a code decodes only with one).
pub fn shared_plan(db: &dyn Database, plan: &SharedPlan, locale: Locale) -> Result<Option<SharedPlanData>, DbError> {
    let Some(key) = plan.key() else { return Ok(None) };
    let ids = checked_ids(&plan.module_ids());
    let (rows, missing) = catalog_rows(db, &ids)?;
    let abbrevs: BTreeMap<String, String> = queries::modules_abbrevs(db, &ids, plan.program.as_deref())?.into_iter().map(|abbrev| (abbrev.module_id, abbrev.abbrev)).collect();
    let names = slot_names(&rows, &abbrevs);
    let modules = ids
        .iter()
        .filter_map(|id| rows.iter().find(|row| row.id == *id))
        .map(|row| SharedModule { id: row.id.clone(), name: names.get(&row.id).cloned().unwrap_or_else(|| views::short_title(&row.title)), title: row.title.clone(), credits: row.credits })
        .collect();
    let label = key.label(locale);
    let program = match plan.program.as_deref() {
        Some(id) => queries::programs(db)?.into_iter().find(|program| program.id == id),
        None => None,
    };
    Ok(Some(SharedPlanData { key, label, modules, missing, program }))
}

#[cfg(test)]
mod faculty_tests {
    use folia_model::labels::Code;

    use super::*;

    fn program(id: &str, name_key: &str) -> Program {
        Program {
            id: id.to_string(),
            slug: id.to_string(),
            name: name_key.to_string(),
            degree_level: Code::parse("bachelor"),
            study_variant: None,
            degree_label: None,
            degree_raw: String::new(),
            degree_display: None,
            po_version: "2022".to_string(),
            po_year: Some(2022),
            family_key: name_key.to_string(),
            name_key: name_key.to_string(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: true,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        }
    }

    fn count(program: &str, department_id: i64, thesis_modules: i64, offered_modules: i64) -> ProgramDepartmentCount {
        ProgramDepartmentCount { program_id: program.to_string(), department_id, thesis_modules, offered_modules }
    }

    /// The thesis's department, else the one offering at least half, else what the programs of the
    /// subject agree on. Checked on its own because a day's snapshot need not hold every case:
    /// since a thesis is what its name says (2026-09-23), no current program depends on its
    /// subject any more.
    #[test]
    fn a_faculty_rests_on_the_thesis_the_majority_or_the_subject() {
        let programs = [
            program("b", "elektrotechnik"),
            program("m", "elektrotechnik"),
            program("d", "elektrotechnik-dual"),
            program("x", "x"),
            program("y1", "y"),
            program("y2", "y"),
            program("y3", "y"),
        ];
        let counts = [
            // The thesis decides, although most of the curriculum is offered elsewhere.
            count("b", 1, 1, 10),
            count("b", 3, 0, 30),
            // No thesis: the department that offers at least half.
            count("m", 1, 0, 6),
            count("m", 3, 0, 4),
            // Two thesis departments and no majority: no answer of its own, and nothing of its
            // subject to agree on either.
            count("x", 1, 1, 4),
            count("x", 2, 1, 3),
            count("x", 3, 0, 3),
            // The programs of a subject that disagree say nothing about the one that is silent.
            count("y1", 1, 1, 5),
            count("y2", 2, 1, 5),
        ];
        let mut found: Vec<(String, i64, FacultyBasis)> =
            faculties(&programs, &counts).into_iter().map(|f| (f.program_id, f.department_id, f.basis)).collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            found,
            vec![
                ("b".to_string(), 1, FacultyBasis::Thesis),
                // „Elektrotechnik - dual" is the subject „Elektrotechnik", whose programs agree.
                ("d".to_string(), 1, FacultyBasis::Counterpart),
                ("m".to_string(), 1, FacultyBasis::Majority),
                ("y1".to_string(), 1, FacultyBasis::Thesis),
                ("y2".to_string(), 2, FacultyBasis::Thesis),
            ]
        );
    }
}

#[cfg(test)]
mod studyplan_tests {
    use folia_calendar::day::Day;
    use folia_calendar::select::{FitOptions, Town};
    use folia_model::labels::Code;
    use folia_model::native::NativeDatabase;
    use folia_timetable::model::tests::{ids, invariants, teaching, winter, Fixture};

    use super::*;

    // The finder's notes in German (`i18n::timetable`), as `fit` writes them for `Locale::De`.
    const RETAKE_NOTE: &str = folia_timetable::i18n::DE.retake_only;
    const UNCOMPARED_NOTE: &str = folia_timetable::i18n::DE.not_compared;
    const UNKNOWN_NOTE: &str = folia_timetable::i18n::DE.no_fixed_dates;

    /// Informatik B.Sc., first semester, in plan order (the import's).
    const FS1: [&str; 4] = ["12104", "12107", "12102", "11112"];

    /// The pinned code of `folia_calendar::subscription`: FS1 in 2026W with Informatik's abbreviations, 149408
    /// hidden, „Nur diesen" on 148369-a4d12.
    const FIRST_SEMESTER_CODE: &str = "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0";

    /// Informatik B.Sc. (PO 2008), the program of FS1.
    const INFORMATIK: &str = "079-82-2008";

    /// The pinned snapshot for `test`, else the tests' own; the semester the checks look at: 2026W
    /// on the pinned one, the current one on any other.
    fn snapshot(test: &str) -> (NativeDatabase, bool, SemesterKey) {
        let pinned = folia_test_support::studyplan_db(test);
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        let semester = if is_pinned { "2026W".to_string() } else { queries::meta(&db).unwrap().current_semester.unwrap() };
        (db, is_pinned, SemesterKey::parse(&semester).unwrap())
    }

    /// FS1 with 12102's second offering at Sachsendorf hidden (H.2): the finder's pinned plan.
    fn sachsendorf_hidden() -> Selection {
        Selection { hidden_events: [149408, 148455].into(), ..Selection::default() }
    }

    /// Lines of an iCalendar text as RFC 5545 wants them: CRLF only, the last one too, none longer
    /// than 75 octets.
    fn well_formed(text: &str) {
        assert!(text.starts_with("BEGIN:VCALENDAR\r\n") && text.ends_with("END:VCALENDAR\r\n"), "{text}");
        assert!(!text.replace("\r\n", "").contains(['\r', '\n']), "a line break other than CRLF");
        assert!(text.split("\r\n").all(|line| line.len() <= 75), "a line over 75 octets");
        assert_eq!(text.matches("BEGIN:VTIMEZONE\r\n").count(), 1);
    }

    /// A plan handed on by a link names its modules as the week grid of the same plan does, in the
    /// plan's order; a module the catalog does not know is counted, not named.
    #[test]
    fn a_shared_plan_names_its_modules_as_the_week_does() {
        let (db, _, key) = snapshot("a_shared_plan_names_its_modules_as_the_week_does");
        let plan = SharedPlan::of(key, &ids(&FS1), Some(INFORMATIK)).unwrap();
        let shared = shared_plan(&db, &plan, Locale::De).unwrap().unwrap();
        let data = studyplan_in(&db, key, &ids(&FS1), Some(INFORMATIK), Locale::De).unwrap();
        let names = data.slot_names();
        let known: Vec<&str> = FS1.iter().copied().filter(|id| !data.missing.iter().any(|missing| missing == id)).collect();
        assert_eq!(shared.modules.iter().map(|module| module.id.as_str()).collect::<Vec<_>>(), known, "the plan's order");
        for module in &shared.modules {
            assert_eq!(names.get(&module.id), Some(&module.name), "{}", module.id);
        }
        assert_eq!((shared.label.as_str(), shared.program.as_ref().map(|program| program.id.as_str())), (data.label.as_str(), Some(INFORMATIK)));
        let credits: f64 = data.modules.iter().filter_map(|row| row.credits).sum();
        assert!((shared.credits() - credits).abs() < 1e-9);

        let unknown = SharedPlan::of(key, &ids(&["99999", "12104"]), None).unwrap();
        let shared = shared_plan(&db, &unknown, Locale::De).unwrap().unwrap();
        assert_eq!((shared.missing, shared.program), (ids(&["99999"]), None));
    }

    /// The server's feed of the pinned code and the browser's download of the same plan are one
    /// text, made twice the same. On the pinned snapshot: the Termine the design names, the town
    /// derived, the choice made, 149408 hidden.
    #[test]
    fn the_feed_of_informatik_first_semester() {
        let (db, is_pinned, key) = snapshot("the_feed_of_informatik_first_semester");
        let subscription = Subscription::from_code(FIRST_SEMESTER_CODE).unwrap();
        let subscription = Subscription { semester: key.index(), ..subscription };
        let feed = calendar(&db, &subscription, Locale::De).unwrap();
        well_formed(&feed);
        assert_eq!(calendar(&db, &subscription, Locale::De).unwrap(), feed, "two fetches, the same bytes");
        let uids: Vec<&str> = feed.split("\r\n").filter_map(|line| line.strip_prefix("UID:")).collect();
        assert_eq!(uids.iter().collect::<BTreeSet<_>>().len(), uids.len(), "every UID once");

        // The browser: its plan order, its store's selection, its own data, loaded for the program
        // the code carries.
        assert_eq!(subscription.program.as_deref(), Some(INFORMATIK));
        let data = studyplan_in(&db, key, &ids(&FS1), Some(INFORMATIK), Locale::De).unwrap();
        assert_eq!(data.program.as_deref(), Some(INFORMATIK));
        assert_eq!(data.ids, ids(&FS1), "the plan's order");
        let table = data.timetable(&subscription.selection());
        invariants(&table);
        assert_eq!(table.modules, ids(&FS1));
        assert_eq!(data.ics(&table, Locale::De), feed, "the download is the feed");
        assert_eq!(data.calendar(&table, Locale::De).entries.len(), uids.len());
        // Without a program the modules go by their own abbreviations, on both sides alike.
        let own = Subscription { program: None, ..subscription.clone() };
        let plain = studyplan(&db, key, &ids(&FS1), Locale::De).unwrap();
        assert_eq!(plain.program, None);
        assert_eq!(plain.ics(&plain.timetable(&own.selection()), Locale::De), calendar(&db, &own, Locale::De).unwrap(), "the download is the feed");
        // A subscription made by hand without a semester: a calendar without entries.
        let nowhere = calendar(&db, &Subscription { semester: 0, ..subscription.clone() }, Locale::De).unwrap();
        well_formed(&nowhere);
        assert!(nowhere.contains("X-WR-CALNAME:Studienplan\r\n") && !nowhere.contains("BEGIN:VEVENT"));
        // The feed of an English address names itself in English, and so does the semester.
        let english = calendar(&db, &Subscription { semester: 0, ..subscription.clone() }, Locale::En).unwrap();
        assert!(english.contains("X-WR-CALNAME:Timetable\r\n"), "{english}");
        let data_en = studyplan_in(&db, key, &ids(&FS1), Some(INFORMATIK), Locale::En).unwrap();
        assert_eq!((data_en.locale, data_en.label.as_str()), (Locale::En, key.label(Locale::En).as_str()));
        assert_eq!(data_en.ids, data.ids, "the same plan in every language");
        if !is_pinned {
            return;
        }

        assert_eq!(data.label, "WiSe 2026/27");
        assert_eq!(table.town, Some(Town::Cottbus));
        assert!(table.town_derived);
        assert_eq!((uids.len(), feed.len()), (249, 177_900));
        // As short as the week's slots: the kinds' letters, Informatik's abbreviations, short rooms.
        assert!(feed.contains("\r\nSUMMARY:VL EvS\r\nLOCATION:ZHG/HS.C\r\n") && feed.contains("\r\nSUMMARY:Prak PP · 1 von 4\r\n"));
        assert!(feed.contains(
            "UID:148701-a2633-20261013@betula.app\r\nDTSTAMP:20260925T083015Z\r\n\
             DTSTART;TZID=Europe/Berlin:20261013T113000\r\n"
        ));
        assert!(feed.contains("\r\nUID:148369-a4d12-"));
        assert!(feed.contains("X-WR-CALNAME:Studienplan WiSe 2026/27\r\n"));
        // Senftenberg's course of 12104 and its exam are not the student's; 149408 is hidden; the
        // other options of 148369 are not chosen; 148663's 2015 date is no Termin.
        for gone in ["UID:149406-", "UID:149407-", "UID:150664-", "UID:149408-", "UID:148369-aaf38-", "20151227"] {
            assert!(!feed.contains(gone), "{gone}");
        }
        // The exams of 11112, 12104 and 12107.
        for exam in ["UID:148097-", "UID:148664-", "UID:148689-485e5-20270312@"] {
            assert!(feed.contains(exam), "{exam}");
        }
        // A code made before QIS publishes the semester: a valid calendar that says so, and fills
        // once the dates are there.
        let early = Subscription { semester: SemesterKey::parse("2027S").unwrap().index(), ..subscription };
        let empty = calendar(&db, &early, Locale::De).unwrap();
        well_formed(&empty);
        assert!(!empty.contains("BEGIN:VEVENT"));
        assert!(empty.contains("X-WR-CALNAME:Studienplan SoSe 2027\r\n"));
        assert!(empty.contains("Noch keine Termine veröffentlicht"));
    }

    /// Elektrotechnik B.Sc. 2022, PA/IoT, first semester (the core audit's case): its located
    /// events make Senftenberg by four to three. Mathematik IT-1 planned beside them would turn the
    /// town to Cottbus and bring back the Cottbus courses of 11107 and 12105; derived from the
    /// modules the Regelstudienplan placed, it stays Senftenberg, and the code says so.
    #[test]
    fn an_elective_does_not_turn_the_derived_town() {
        let (db, is_pinned, key) = snapshot("an_elective_does_not_turn_the_derived_town");
        if !is_pinned {
            return;
        }
        let imported = ["13694", "13693", "11107", "12761", "12105"];
        let mut doc = folia_plans::studyplan::PlanDoc::default();
        let import = folia_plans::studyplan::Import { modules: imported.iter().map(|id| (key, id.to_string())).collect(), ..Default::default() };
        doc.apply(&import, 1);
        assert!(doc.plan(key, "11112", 2, None));
        let data = studyplan(&db, key, &doc.modules_in(key), Locale::De).unwrap();
        let every = data.timetable(&Selection::default());
        assert_eq!((every.town, every.town_derived), (Some(Town::Cottbus), true), "every module decides: the elective turns it");
        let selection = doc.selection(key, folia_calendar::select::TownChoice::Derive);
        let table = data.timetable(&selection);
        assert_eq!((table.town, table.town_derived), (Some(Town::Senftenberg), true));
        assert!(table.tracks.contains("11107") && table.tracks.contains("12105"));
        let (code, _) = folia_timetable::export::subscription_of(key, &table.modules, None, &selection, Some(&table));
        assert_eq!(code.town, folia_calendar::select::TownChoice::Only(Town::Senftenberg).code(), "the feed shows the page's town");
        // Without the elective, the imported modules alone: the same town, as before.
        let before = studyplan(&db, key, &ids(&imported), Locale::De).unwrap().timetable(&Selection::default());
        assert_eq!(before.town, Some(Town::Senftenberg));
    }

    /// The loaders keep the plan's order, check what a store hands in, and add a module's rows to
    /// a plan without asking for the plan again.
    #[test]
    fn a_plan_is_loaded_in_its_order_and_with_one_module_more() {
        let (db, is_pinned, key) = snapshot("a_plan_is_loaded_in_its_order_and_with_one_module_more");
        let asked = ids(&["12107", "1 OR 1=1", "12104", "12107", "00000", "11112"]);
        let data = studyplan(&db, key, &asked, Locale::De).unwrap();
        assert_eq!(data.ids, ids(&["12107", "12104", "00000", "11112"]));
        assert_eq!(data.missing, ids(&["00000"]));
        assert!(data.modules.windows(2).all(|pair| pair[0].title <= pair[1].title), "by title");
        let known: BTreeSet<&str> = data.modules.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(known, ["12104", "12107", "11112"].into());
        assert!(data.schedule.iter().chain(&data.exams).all(|row| known.contains(row.module_id.as_str())));
        assert_eq!(studyplan_modules(&db, &asked).unwrap(), (data.modules.clone(), data.missing.clone()));

        // Nothing planned: no rows, the semester all the same.
        let empty = studyplan(&db, key, &[], Locale::De).unwrap();
        assert!(empty.ids.is_empty() && empty.modules.is_empty() && empty.schedule.is_empty() && empty.sws.is_empty());
        assert_eq!(empty.counts, data.counts);
        assert_eq!(studyplan_modules(&db, &[]).unwrap(), (Vec::new(), Vec::new()));

        // One module more, as a module page's overlay adds it.
        let plan = studyplan(&db, key, &ids(&FS1), Locale::De).unwrap();
        let id = ids(&["11103"]);
        let rows = queries::modules_schedule(&db, &id, &key.key()).unwrap();
        let exams = queries::modules_exams(&db, &id, &key.key()).unwrap();
        let sws = queries::modules_teaching_sws(&db, &id).unwrap();
        let with = plan.with_module("11103", rows.clone(), exams.clone(), sws.clone());
        assert_eq!(with.ids, ids(&["12104", "12107", "12102", "11112", "11103"]));
        assert_eq!(with.schedule, [plan.schedule.clone(), rows.clone()].concat());
        assert_eq!(with.exams, [plan.exams.clone(), exams.clone()].concat());
        assert_eq!(with.sws, [plan.sws.clone(), sws.clone()].concat());
        assert_eq!((&with.modules, &with.missing), (&plan.modules, &plan.missing));
        // A planned module added again replaces its rows: its SWS count once.
        let own = |data: &StudyplanData, id: &str| data.sws.iter().filter(|s| s.module_id == id).count();
        let twice = with.with_module("11103", rows, exams, sws);
        assert_eq!(twice.ids, with.ids);
        assert_eq!(own(&twice, "11103"), own(&with, "11103"));
        assert_eq!(twice.schedule.len(), with.schedule.len());
        assert_eq!(plan.with_module("1 OR 1=1", Vec::new(), Vec::new(), Vec::new()), plan);

        if is_pinned {
            // A semester the snapshot does not have: no row, no Termin.
            let later = studyplan(&db, SemesterKey::parse("2027S").unwrap(), &ids(&FS1), Locale::De).unwrap();
            assert_eq!(later.semester, None);
            assert_eq!(later.label, "SoSe 2027");
            assert!(later.schedule.is_empty() && later.exams.is_empty() && later.counts.is_empty());
            assert_eq!(later.modules.len(), 4, "the modules are the catalog's, whatever the semester");
            let table = later.timetable(&Selection::default());
            assert_eq!(table.without_dates, ids(&FS1));
            let feed = later.ics(&table, Locale::De);
            well_formed(&feed);
            assert!(!feed.contains("BEGIN:VEVENT"));
            assert!(feed.contains("Noch keine Termine veröffentlicht"));
        }
    }

    /// „Passt in meinen Stundenplan" as the catalog asks for it: what was checked and does not
    /// clash, the clashing and the planned ones left out, the notes of partial fits and unknowns;
    /// a second question with the same key does not rebuild the candidates.
    #[test]
    fn the_finder_lists_what_was_checked_and_fits() {
        let (db, is_pinned, key) = snapshot("the_finder_lists_what_was_checked_and_fits (pages)");
        let plan = ids(&FS1);
        let selection = sachsendorf_hidden();
        let all = FitsFilter::all(&key.key());
        let mut cache = None;
        let found = fit(&db, &all, &plan, &selection, &mut cache, Locale::De).unwrap();

        // What holds on any snapshot: the planned ones are left out, nothing is both listed and
        // left out, and a note belongs to a listed module, so no unknown (each has one) is left
        // out.
        let fitting: BTreeSet<&str> = found.fitting.iter().map(String::as_str).collect();
        let excluded: BTreeSet<&str> = found.excluded.iter().map(String::as_str).collect();
        assert!(found.fitting.windows(2).all(|pair| pair[0] < pair[1]), "by id, each once");
        assert!(found.excluded.windows(2).all(|pair| pair[0] < pair[1]), "by id, each once");
        assert!(FS1.iter().all(|id| excluded.contains(id) && !fitting.contains(id)));
        assert!(fitting.is_disjoint(&excluded));
        assert!(found.notes.keys().all(|id| fitting.contains(id.as_str())));
        // What could not be checked is listed and says why.
        assert!(found.unknown.iter().all(|id| fitting.contains(id.as_str()) && found.notes.contains_key(id)));
        let set = cache.as_ref().expect("the candidates are kept");
        assert_eq!(set.key.0, key);
        assert_eq!(set.key.1, all.options());
        let checked: BTreeSet<&str> = set.modules.iter().map(|candidate| candidate.module_id.as_str()).collect();
        assert!(fitting.is_subset(&checked), "only what was checked is listed");
        assert!(excluded.iter().all(|id| checked.contains(id) || FS1.contains(id)));
        let judged: BTreeSet<&str> = fitting.union(&excluded).copied().filter(|id| checked.contains(id)).collect();
        assert_eq!(judged, checked, "every candidate is listed or left out");
        for (id, note) in &found.notes {
            assert!(!note.is_empty(), "{id}");
        }

        // The same key again: the kept candidates answer. Emptied, they list nothing.
        let mut kept = cache.clone();
        if let Some(set) = kept.as_mut() {
            set.modules.clear();
        }
        let again = fit(&db, &all, &plan, &selection, &mut kept, Locale::De).unwrap();
        assert!(again.fitting.is_empty() && again.notes.is_empty(), "rebuilt although the key held");
        let planned: Vec<String> = found.excluded.iter().filter(|id| FS1.contains(&id.as_str())).cloned().collect();
        assert_eq!(again.excluded, planned);
        assert!(kept.as_ref().is_some_and(|set| set.modules.is_empty()));
        // Another key builds them anew.
        let lenient = FitsFilter { exercises: false, ..all.clone() };
        let without_exercises = fit(&db, &lenient, &plan, &selection, &mut kept, Locale::De).unwrap();
        assert!(kept.as_ref().is_some_and(|set| set.key.1 == FitOptions { exercises: false, ..all.options() }));
        assert_eq!(without_exercises.has_data, found.has_data);
        if found.has_data {
            assert!(kept.as_ref().is_some_and(|set| !set.modules.is_empty()));
        }
        // An address the semester parser refuses checks nothing.
        let broken = fit(&db, &FitsFilter::all("2026X"), &plan, &selection, &mut None, Locale::De).unwrap();
        assert_eq!(broken, FitResult { excluded: ids(&["11112", "12102", "12104", "12107"]), ..FitResult::default() });
        if !is_pinned {
            return;
        }

        assert!(found.has_data);
        // Analysis I's lecture meets 12107's on Tuesdays, Theoretische Informatik's meets 148701/1,
        // Deutsch als Fremdsprache's Übung the Tutorium 150132.
        for clashing in ["11103", "11787", "13583"] {
            assert!(excluded.contains(clashing) && !fitting.contains(clashing), "{clashing}");
        }
        // Datenbanken fits; Algorithmieren und Programmieren has only a free retake in the winter.
        assert!(fitting.contains("12330") && !found.notes.contains_key("12330"));
        assert!(fitting.contains("12101"));
        assert_eq!(found.notes.get("12101").map(String::as_str), Some(RETAKE_NOTE));
        assert!(found.notes.values().any(|note| note.starts_with(UNKNOWN_NOTE)));
        // Every kind of „not checked" is an unknown, and a partial fit is none.
        assert!(found.unknown.contains("12101") && !found.unknown.contains("12330"));
        let not_checked = |note: &str| [UNKNOWN_NOTE, RETAKE_NOTE, UNCOMPARED_NOTE].iter().any(|kind| note.starts_with(kind));
        for result in [&found, &without_exercises] {
            assert!(result.notes.iter().all(|(id, note)| result.unknown.contains(id) == not_checked(note)), "an unknown by its note");
        }
        // Grundlagen der Rechnernetze has no dated row in 2026W: neither checked nor left out.
        assert!(!fitting.contains("11454") && !excluded.contains("11454"));
        // Without Übungen compared, Deutsch als Fremdsprache fits; Analysis I still does not. A
        // module of Übungen alone was not compared, which it says instead of having no times.
        assert!(without_exercises.fitting.iter().any(|id| id == "13583"));
        assert!(without_exercises.notes.values().any(|note| note == UNCOMPARED_NOTE));
        assert!(!found.notes.values().any(|note| note.starts_with(UNCOMPARED_NOTE)), "everything compared");
        assert!(without_exercises.excluded.iter().any(|id| id == "11103"));

        // SoSe 2027 has no Termine yet: nothing is checked, the plan is left out.
        let summer = fit(&db, &FitsFilter::all("2027S"), &plan, &selection, &mut cache, Locale::De).unwrap();
        assert_eq!(summer, FitResult { excluded: ids(&["11112", "12102", "12104", "12107"]), ..FitResult::default() });
        assert_eq!(cache.as_ref().map(|set| set.key.0), Some(key), "a semester without data keeps the candidates");
    }

    /// Analysis I beside the first semester: its Tuesday lecture meets 12107's, so the line warns
    /// and names the module; the planned modules' slots are drawn, its own are not among them.
    #[test]
    fn the_overlay_of_analysis_on_the_first_semester() {
        let (db, is_pinned, key) = snapshot("the_overlay_of_analysis_on_the_first_semester");
        let plan = studyplan(&db, key, &ids(&FS1), Locale::De).unwrap();
        let selection = sachsendorf_hidden();
        let overlay = overlay(&db, &plan, "11103", &selection).unwrap();
        let fs1: BTreeSet<&str> = FS1.into();
        assert!(overlay.planned.iter().all(|slot| fs1.contains(slot.module.as_str())));
        assert!(overlay.planned.windows(2).all(|pair| (pair[0].day, pair[0].from) <= (pair[1].day, pair[1].from)));
        assert!(overlay.planned.iter().all(|slot| (1..=7).contains(&slot.day) && slot.from < slot.to));
        let own: BTreeSet<RowKey> = queries::modules_schedule(&db, &ids(&["11103"]), &key.key())
            .unwrap()
            .iter()
            .filter_map(|row| RowKey::of(&row.date))
            .collect();
        assert!(overlay.clashing.is_subset(&own));
        assert_eq!(overlay.clashing.is_empty(), !overlay.line.as_ref().is_some_and(|(warn, _)| *warn));
        if !is_pinned {
            return;
        }

        let line = "Überschneidet sich mit: Elektrische und elektronische Grundlagen der Informatik (Di 09:15)";
        assert_eq!(overlay.line, Some((true, line.to_string())));
        // Its lecture 148109, Tuesday and Thursday; the Thursday is free.
        assert_eq!(overlay.clashing.iter().map(|key| key.event).collect::<Vec<_>>(), [148109]);
        assert_eq!(overlay.exam_line, None);
        let slots: BTreeSet<&str> = overlay.planned.iter().map(|slot| slot.module.as_str()).collect();
        assert_eq!(slots, fs1);
        assert!(overlay.planned.iter().all(|slot| slot.short == plan.slot_names()[&slot.module]));
    }

    /// What avoids an exam overlap, as the pinned snapshot has it: the module's own earlier sitting
    /// or the planned module's; and the free Übungen of an open choice by time.
    #[test]
    fn the_overlay_says_which_termin_avoids_an_exam() {
        let (db, is_pinned, key) = snapshot("the_overlay_says_which_termin_avoids_an_exam");
        if !is_pinned {
            return;
        }
        let line = |plan: &[&str], module: &str| {
            overlay(&db, &studyplan(&db, key, &ids(plan), Locale::De).unwrap(), module, &sachsendorf_hidden()).unwrap()
        };

        // Algorithmische Graphentheorie (152760) sits on 25.02., 10.03. and 11.03.; its 10.03. meets
        // Mathematik IT-1's only sitting (148664), its first does not.
        let own = "Prüfung gleichzeitig mit Mathematik IT-1 (Diskrete Mathematik) (10.03.2027 11:00) · \
                   Erstermin 25.02. passt";
        assert_eq!(line(&FS1, "11405").exam_line, Some((false, own.to_string())));
        // The other way round, Mathematik IT-1 has no other sitting: the planned module's avoids it.
        let theirs = "Prüfung gleichzeitig mit Algorithmische Graphentheorie (10.03.2027 10:00) · \
                      Algorithmische Graphentheorie am 25.02. passt";
        assert_eq!(line(&["11405"], "11112").exam_line, Some((false, theirs.to_string())));
        // 12079 (150766) sits on 12.03. and 16.03.; the second meets Theoretische Informatik.
        assert_eq!(
            line(&["11787", "12202", "11213"], "12079").exam_line,
            Some((false, "Prüfung gleichzeitig mit Theoretische Informatik (16.03.2027 11:00) · Erstermin 12.03. passt".to_string()))
        );
        // Two free Übungen of one Thursday, the later one first among the options.
        assert_eq!(line(&FS1, "12112").line, Some((false, "Passt mit Übung Do 14:30 oder Do 16:30".to_string())));
    }

    /// An exam row as `modules_exams` delivers it: one day with its times, at `campus`.
    fn exam(module: &str, event: &str, on: &str, from: &str, to: &str, campus: &str) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(1),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: format!("Prüfung {event}"),
                event_type: None,
                group_name: None,
                weekday: Day::parse(on).map(|day| i64::from(day.weekday())),
                start_time: Some(from.into()),
                end_time: Some(to.into()),
                rhythm: None,
                rhythm_raw: None,
                first_date: Some(on.into()),
                last_date: Some(on.into()),
                room: None,
                campus: Some(Code::parse(campus)),
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    /// The overlay of M in a plan of P and Q in 2026W as the data has it.
    fn overlay_in(rows: &[Fixture], exams: &[DateRow], modules: &[&str]) -> Overlay {
        overlay_in_language(rows, exams, modules, Locale::De)
    }

    /// The same on a page in `locale`.
    fn overlay_in_language(rows: &[Fixture], exams: &[DateRow], modules: &[&str], locale: Locale) -> Overlay {
        let facts = winter();
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let modules = ids(modules);
        let input =
            Input { key: facts.key, semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams, sws: &[] };
        let table = Timetable::build(&input, &Selection::default());
        invariants(&table);
        let titles: BTreeMap<String, String> =
            [("P", "Analysis I"), ("Q", "Mathematik IT-1")].map(|(id, title)| (id.to_string(), title.to_string())).into();
        overlay_of(&table, &titles, &BTreeMap::new(), &facts.key.label(locale), "M", locale)
    }

    #[test]
    fn the_overlay_names_what_the_module_meets() {
        let plan = [
            teaching("P", "1", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("Q", "2", 1, "Vorlesung", 2, "13:45", "15:15"),
            teaching("Q", "2", 2, "Vorlesung", 4, "07:30", "09:00").rhythm("single").range("2026-10-08", "2026-10-08"),
        ];
        let slot = |module: &str, short: &str, day: u8, from: u16, to: u16| OverlaySlot {
            module: module.into(),
            short: short.into(),
            day,
            from,
            to,
        };
        let recurring = vec![slot("P", "Analysis I", 2, 555, 645), slot("Q", "Mathematik IT-1", 2, 825, 915)];

        // Its lecture meets P's and its Übung Q's: both named, at their times; the single date
        // of Q is no slot of the week.
        let lecture = teaching("M", "3", 1, "Vorlesung", 2, "09:15", "10:45");
        let exercise = teaching("M", "4", 1, "Übung", 2, "14:00", "15:30");
        let meets = overlay_in(&[plan.as_slice(), &[lecture.clone(), exercise.clone()]].concat(), &[], &["P", "Q", "M"]);
        assert_eq!(meets.planned, recurring);
        assert_eq!(meets.clashing, [lecture.key(), exercise.key()].into());
        assert_eq!(
            meets.line,
            Some((true, "Überschneidet sich mit: Analysis I (Di 09:15), Mathematik IT-1 (Di 13:45)".to_string()))
        );
        assert_eq!(meets.exam_line, None);

        // More than three modules: the rest are counted.
        let many: Vec<Fixture> = ["A", "B", "C", "D", "E"]
            .iter()
            .zip(10..)
            .map(|(module, event)| teaching(module, &event.to_string(), 1, "Vorlesung", 3, "11:30", "13:00"))
            .chain([teaching("M", "3", 1, "Vorlesung", 3, "11:30", "13:00")])
            .collect();
        let crowded = overlay_in(&many, &[], &["A", "B", "C", "D", "E", "M"]);
        assert_eq!(
            crowded.line,
            Some((true, "Überschneidet sich mit: A (Mi 11:30), B (Mi 11:30), C (Mi 11:30) +2".to_string()))
        );

        // Its Übung in three groups, two of them at P's and Q's lectures: it fits with the third.
        let friday_lecture = teaching("M", "3", 1, "Vorlesung", 5, "11:30", "13:00");
        let groups = [(1, "09:15", "10:45"), (2, "09:15", "10:45"), (4, "13:45", "15:15")]
            .into_iter()
            .zip(1..)
            .map(|((weekday, from, to), ord)| {
                teaching("M", "5", ord, "Übung", weekday, from, to).group(&format!("{ord}-Gruppe"))
            });
        let rows: Vec<Fixture> = [
            teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"),
            teaching("Q", "2", 1, "Vorlesung", 2, "09:15", "10:45"),
            friday_lecture.clone(),
        ]
        .into_iter()
        .chain(groups)
        .collect();
        let choice = overlay_in(&rows, &[], &["P", "Q", "M"]);
        assert!(choice.clashing.is_empty());
        assert_eq!(choice.line, Some((false, "Passt mit Übung Do 13:45".to_string())));
        // Two free groups, the later one first in the source: they are named by time.
        let groups = [(4, "16:30", "18:00"), (1, "09:15", "10:45"), (4, "14:30", "16:00")]
            .into_iter()
            .zip(1..)
            .map(|((weekday, from, to), ord)| {
                teaching("M", "5", ord, "Übung", weekday, from, to).group(&format!("{ord}-Gruppe"))
            });
        let rows: Vec<Fixture> = [teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"), friday_lecture.clone()]
            .into_iter()
            .chain(groups)
            .collect();
        let by_time = overlay_in(&rows, &[], &["P", "M"]);
        assert_eq!(by_time.line, Some((false, "Passt mit Übung Do 14:30 oder Do 16:30".to_string())));

        // Nothing meets: it fits.
        let friday = overlay_in(&[plan.as_slice(), &[friday_lecture]].concat(), &[], &["P", "Q", "M"]);
        assert_eq!(friday.line, Some((false, "Passt in deinen Stundenplan (WiSe 2026/27)".to_string())));
        assert!(friday.clashing.is_empty());

        // Without a Termin of its own that has a time there is nothing to say under its week.
        let mut untimed = teaching("M", "3", 1, "Vorlesung", 5, "11:30", "13:00");
        untimed.0.date.start_time = None;
        let silent = overlay_in(&[plan.as_slice(), &[untimed]].concat(), &[], &["P", "Q", "M"]);
        assert_eq!(silent.line, None);
        assert_eq!(silent.planned, recurring);
        assert_eq!(overlay_in(&plan, &[], &["P", "Q", "M"]).line, None);
    }

    #[test]
    fn the_overlay_names_the_exams_the_module_meets() {
        let p = exam("P", "90", "2027-02-11", "11:00", "13:00", "zentralcampus");
        let exam_line = |mine: DateRow| overlay_in(&[], &[p.clone(), mine], &["P", "M"]).exam_line;
        // One Termin each: nothing avoids it, a warning.
        let warns = |text: &str| Some((true, text.to_string()));
        let quiet = |text: &str| Some((false, text.to_string()));
        assert_eq!(
            exam_line(exam("M", "91", "2027-02-11", "12:00", "14:00", "zentralcampus")),
            warns("Prüfung gleichzeitig mit Analysis I (11.02.2027 11:00)")
        );
        assert_eq!(
            exam_line(exam("M", "91", "2027-02-11", "13:30", "15:00", "senftenberg")),
            warns("30 min bis Senftenberg nach Analysis I")
        );
        let q = exam("P", "92", "2027-02-11", "11:00", "13:00", "senftenberg");
        let earlier = overlay_in(&[], &[q, exam("M", "91", "2027-02-11", "08:00", "10:00", "zentralcampus")], &["P", "M"]);
        assert_eq!(earlier.exam_line, warns("60 min bis Senftenberg zu Analysis I"));
        // A second Termin of M avoids it: the warning is soft, says so, and is quiet.
        let second = exam("M", "93", "2027-03-11", "11:00", "13:00", "zentralcampus");
        let soft = overlay_in(
            &[],
            &[p.clone(), exam("M", "91", "2027-02-11", "11:00", "13:00", "zentralcampus"), second],
            &["P", "M"],
        );
        let avoided = "Prüfung gleichzeitig mit Analysis I (11.02.2027 11:00) · Zweittermin 11.03. passt";
        assert_eq!(soft.exam_line, quiet(avoided));
        assert_eq!(exam_line(exam("M", "91", "2027-02-12", "11:00", "13:00", "zentralcampus")), None);

        // M's earlier sitting avoids the overlap of its later one: that is its Erstermin.
        let first = exam("M", "93", "2027-02-04", "11:00", "13:00", "zentralcampus");
        let earlier = overlay_in(
            &[],
            &[p.clone(), exam("M", "91", "2027-02-11", "11:00", "13:00", "zentralcampus"), first],
            &["P", "M"],
        );
        let avoided = "Prüfung gleichzeitig mit Analysis I (11.02.2027 11:00) · Erstermin 04.02. passt";
        assert_eq!(earlier.exam_line, quiet(avoided));
        // Only P's other sitting avoids it: P's is named.
        let theirs = overlay_in(
            &[],
            &[
                p.clone(),
                exam("P", "94", "2027-02-18", "11:00", "13:00", "zentralcampus"),
                exam("M", "91", "2027-02-11", "11:00", "13:00", "zentralcampus"),
            ],
            &["P", "M"],
        );
        let avoided = "Prüfung gleichzeitig mit Analysis I (11.02.2027 11:00) · Analysis I am 18.02. passt";
        assert_eq!(theirs.exam_line, quiet(avoided));
        // Only a change of both avoids it: P at 08:00 meets M at 08:00 and M at 09:00, M at 08:00
        // meets P at 09:00, and the two at 09:00 are one exam of both.
        let both = overlay_in(
            &[],
            &[
                exam("P", "90", "2027-02-11", "08:00", "10:00", "zentralcampus"),
                exam("M", "91", "2027-02-11", "08:00", "10:00", "zentralcampus"),
                exam("P", "95", "2027-02-11", "09:00", "11:00", "zentralcampus"),
                exam("M", "95", "2027-02-11", "09:00", "11:00", "zentralcampus"),
            ],
            &["P", "M"],
        );
        let avoided = "Prüfung gleichzeitig mit Analysis I (11.02.2027 08:00) · andere Termine passen";
        assert_eq!(both.exam_line, quiet(avoided));
    }

    /// The lines of the overlay on an English page: the words, weekdays and dates in English, the
    /// titles of the modules and QIS's words for an event as the data has them.
    #[test]
    fn the_overlay_speaks_the_language_of_its_page() {
        let english = |rows: &[Fixture], exams: &[DateRow], modules: &[&str]| overlay_in_language(rows, exams, modules, Locale::En);
        let plan = [teaching("P", "1", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("Q", "2", 1, "Vorlesung", 2, "13:45", "15:15")];
        let lecture = teaching("M", "3", 1, "Vorlesung", 2, "09:15", "10:45");
        let exercise = teaching("M", "4", 1, "Übung", 2, "14:00", "15:30");
        let meets = english(&[plan.as_slice(), &[lecture, exercise]].concat(), &[], &["P", "Q", "M"]);
        assert_eq!(meets.line, Some((true, "Clashes with: Analysis I (Tue 09:15), Mathematik IT-1 (Tue 13:45)".to_string())));

        let friday_lecture = teaching("M", "3", 1, "Vorlesung", 5, "11:30", "13:00");
        let friday = english(&[plan.as_slice(), std::slice::from_ref(&friday_lecture)].concat(), &[], &["P", "Q", "M"]);
        assert_eq!(friday.line, Some((false, "Fits your timetable (Winter 2026/27)".to_string())));
        let groups = [(4, "16:30", "18:00"), (1, "09:15", "10:45"), (4, "14:30", "16:00")]
            .into_iter()
            .zip(1..)
            .map(|((weekday, from, to), ord)| teaching("M", "5", ord, "Übung", weekday, from, to).group(&format!("{ord}-Gruppe")));
        let rows: Vec<Fixture> = [teaching("P", "1", 1, "Vorlesung", 1, "09:15", "10:45"), friday_lecture].into_iter().chain(groups).collect();
        assert_eq!(english(&rows, &[], &["P", "M"]).line, Some((false, "Fits with Übung Thu 14:30 or Thu 16:30".to_string())));

        let p = exam("P", "90", "2027-02-11", "11:00", "13:00", "zentralcampus");
        let exam_line = |exams: &[DateRow]| english(&[], exams, &["P", "M"]).exam_line;
        assert_eq!(
            exam_line(&[p.clone(), exam("M", "91", "2027-02-11", "12:00", "14:00", "zentralcampus")]),
            Some((true, "Exam at the same time as Analysis I (11 Feb 2027 11:00)".to_string()))
        );
        assert_eq!(
            exam_line(&[p.clone(), exam("M", "91", "2027-02-11", "13:30", "15:00", "senftenberg")]),
            Some((true, "30 min to Senftenberg after Analysis I".to_string()))
        );
        assert_eq!(
            exam_line(&[exam("P", "92", "2027-02-11", "11:00", "13:00", "senftenberg"), exam("M", "91", "2027-02-11", "08:00", "10:00", "zentralcampus")]),
            Some((true, "60 min to Senftenberg for Analysis I".to_string()))
        );
        let clash = exam("M", "91", "2027-02-11", "11:00", "13:00", "zentralcampus");
        assert_eq!(
            exam_line(&[p.clone(), clash.clone(), exam("M", "93", "2027-03-11", "11:00", "13:00", "zentralcampus")]),
            Some((false, "Exam at the same time as Analysis I (11 Feb 2027 11:00) · second sitting on 11 Mar fits".to_string()))
        );
        assert_eq!(
            exam_line(&[p.clone(), clash.clone(), exam("M", "93", "2027-02-04", "11:00", "13:00", "zentralcampus")]),
            Some((false, "Exam at the same time as Analysis I (11 Feb 2027 11:00) · first sitting on 4 Feb fits".to_string()))
        );
        assert_eq!(
            exam_line(&[p, exam("P", "94", "2027-02-18", "11:00", "13:00", "zentralcampus"), clash]),
            Some((false, "Exam at the same time as Analysis I (11 Feb 2027 11:00) · Analysis I on 18 Feb fits".to_string()))
        );
    }

    /// A program's plans for the import, and „Mein Studiengang" with an id the snapshot has or
    /// lost.
    #[test]
    fn the_plans_of_a_program_and_the_program_of_mine() {
        let (db, is_pinned, _) = snapshot("the_plans_of_a_program_and_the_program_of_mine");
        let programs = queries::programs(&db).unwrap();
        assert_eq!(plan_source(&db, "no-such-program", Locale::De).unwrap(), None);
        assert_eq!(my_program(&db, "x").unwrap(), None);
        for program in programs.iter().filter(|program| program.has_plan).take(20) {
            let source = plan_source(&db, &program.id, Locale::De).unwrap().unwrap();
            assert_eq!(&source.program, program);
            assert!(!source.variants.is_empty(), "{}", program.id);
            assert!(source.supplements.iter().all(|s| s.core < source.variants.len() && s.page < source.variants.len()));
            let named: BTreeSet<&str> =
                source.variants.iter().flat_map(|v| &v.entries).filter_map(|e| e.module_id.as_deref()).collect();
            assert!(source.linked.iter().all(|row| named.contains(row.id.as_str())), "{}", program.id);
            let mine = my_program(&db, &program.id).unwrap().unwrap();
            assert!(mine.exact && mine.program == *program);
            let newest = |latest: &Program| latest.is_latest_po && latest.family_key == program.family_key;
            assert!(mine.latest.as_ref().is_none_or(newest));
            assert_eq!(mine.latest.is_none(), program.is_latest_po);
        }
        if !is_pinned {
            return;
        }

        let informatik = plan_source(&db, "079-82-2008", Locale::De).unwrap().unwrap();
        assert_eq!(informatik.program.slug, "bachelor-informatik-2008");
        assert_eq!(informatik.variants.len(), 1);
        assert!(informatik.supplements.is_empty());
        assert!(informatik.linked.iter().any(|row| row.id == "12104"));
        assert!(!informatik.areas.is_empty());
        let direction = plan_source(&db, "370-82-2023", Locale::De).unwrap().unwrap();
        assert_eq!(direction.supplements.len(), 5);
        assert!(direction.supplements.iter().all(|s| s.ord == 16 && s.core == direction.supplements[0].core));

        // A PO gone from the snapshot: its family's newest.
        let gone = my_program(&db, "079-82-1999").unwrap().unwrap();
        assert!(!gone.exact);
        assert_eq!(gone.program.id, "079-82-2008");
        assert_eq!(gone.latest.as_ref().map(|p| p.id.as_str()), Some("079-82-2008"));
        let older = my_program(&db, "048-82-2019").unwrap().unwrap();
        assert!(older.exact);
        assert_eq!(older.latest.map(|p| p.id), Some("048-82-2022".to_string()));
        assert_eq!(my_program(&db, "999-82-2020").unwrap(), None);
    }
}

/// What the views allow and a page must survive, in a catalog of its own: the snapshot of the other
/// tests holds only what Radix builds today.
#[cfg(test)]
mod view_edge_tests {
    use std::path::{Path, PathBuf};

    use folia_model::native::NativeDatabase;
    use folia_model::rows_detail::EventDate;
    use folia_query as queries;

    use crate as pages;

    /// A catalog at `path` made by Radix's own migrations (`radix/internal/catalogdb/migrations`), with `rows`.
    fn catalog(path: &Path, rows: &str) -> NativeDatabase {
        let _ = std::fs::remove_file(path);
        let conn = rusqlite::Connection::open(path).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../radix/internal/catalogdb/migrations");
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
