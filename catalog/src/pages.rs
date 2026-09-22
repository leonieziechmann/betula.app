//! What each page needs, loaded in one go from one snapshot.
//!
//! A page asks for its data once; server and browser run the same function, so the
//! server-rendered HTML and the hydrated page cannot disagree.

use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError};
use crate::filter::{CatalogQuery, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey, TurnusFilter};
use crate::labels::{Labelled, ModuleKind, OfferStatus};
use crate::plan::{self, SemesterPlan};
use crate::queries;
use crate::rows::{CatalogPage, CatalogRow, Department, Meta, Module, Prerequisite, Program, ProgramModule, Semester};
use crate::rows_detail::{
    AreaNode, AreaPlacement, Counterpart, Document, EventDate, Lecturer, LecturerName, ModuleTeachingForm, Plan, PlanEntry,
    PlanTotal, ProgramDepartmentCount, ProgramLink, ProgramVersion, Successor, TextItem,
};
use crate::url::{BookmarkSort, CatalogUrl, Season, PAGE_SIZE};

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

/// An area of the selected program's module tree, as the filter panel offers it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogArea {
    /// `v_program_module_area.area_id`: what the URL carries (`area=<id>`).
    pub id: i64,
    pub label: String,
    /// The whole path („Fachstudium / Wahlpflichtmodule Praktische Informatik").
    pub path: String,
    pub depth: i64,
    /// How many modules the tree places directly in it.
    pub modules: usize,
    /// Whether a student chooses here: a module placed directly in it is not known to be
    /// compulsory, the thesis or the internship — by what the program's sources settle on for
    /// the module (`v_program_module.kind`: the plan, the module page, the tree's own label),
    /// unknown counting as a choice (R12: not known to be fixed is not fixed). A fixed area
    /// („Komplex Mathematik" with its Pflichtmodule, the thesis) is no filter worth offering —
    /// those modules are taken anyway — and no requirement row of a plan points at it (owner,
    /// 2026-09-21).
    pub choice: bool,
    /// The labels of the nodes above it, from the top of the tree down, from the tree itself
    /// (`program_area.parent_id`), never from splitting the path: a label may itself read
    /// „Maschinenbau / Elektrotechnik".
    pub ancestors: Vec<String>,
    /// The name the pickers show (`catalog_areas`): the label without a leading
    /// „Wahlpflichtmodule"; for a label that says nothing but that („Wahlpflichtmodule (KT)"),
    /// the name of the area it lies in; with what tells two apart where two would read the same.
    pub name: String,
    /// The heading the pickers put it under (`catalog_areas`); `None` for the areas that come
    /// first, without one.
    pub section: Option<String>,
}

impl CatalogArea {
    /// An area on its own, named by its label, under no heading; `catalog_areas` names and
    /// places the areas of a whole tree.
    pub fn new(id: i64, label: &str, ancestors: &[&str], modules: usize, choice: bool) -> Self {
        let path = ancestors.iter().chain([&label]).copied().collect::<Vec<_>>().join(" / ");
        Self {
            id,
            label: label.to_string(),
            path,
            depth: ancestors.len() as i64 + 1,
            modules,
            choice,
            ancestors: ancestors.iter().map(|ancestor| ancestor.to_string()).collect(),
            name: short_name(label).to_string(),
            section: None,
        }
    }

    /// The name the pickers show.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The label of the area directly above it, if any.
    pub fn parent(&self) -> Option<&str> {
        self.ancestors.last().map(String::as_str)
    }
}

/// „Wahlpflichtmodule Praktische Mathematik" → „Praktische Mathematik", „Wahlpflichtmodule der
/// Berufsfelder" → „Berufsfelder"; a label that is nothing but such a word, or such a word and
/// a study direction („Wahlpflicht (CIV)"), stays as it is.
pub fn short_name(label: &str) -> &str {
    const JOINS: &[&str] = &["aus", "dem", "der", "des", "den", "die", "für", "im", "in", "von", "zur", "zum"];
    for prefix in ["Wahlpflichtmodule ", "Wahlpflichtmodul ", "Wahlpflichtbereich ", "Wahlpflichtfach ", "Wahlpflicht ", "Wahlbereich "] {
        if label.len() > prefix.len() && label.is_char_boundary(prefix.len()) && label[..prefix.len()].eq_ignore_ascii_case(prefix) {
            let mut rest = label[prefix.len()..].trim_start_matches(|c: char| c == '-' || c == ':' || c.is_whitespace());
            while let Some((word, after)) = rest.split_once(' ') {
                if !JOINS.contains(&word.to_lowercase().as_str()) {
                    break;
                }
                rest = after.trim_start();
            }
            if rest.chars().count() >= 3 && !crate::plan::distinctive(crate::plan::without_direction(rest)).is_empty() {
                return rest;
            }
        }
    }
    label
}

/// A node of the tree that only structures it and names no field of study: a phase of the
/// studies („Grundstudium", „Fachstudium", „Hauptstudium"), an account („Gesamtkonto Bachelor",
/// „Total Account - …", „Module an der …", „Modules at …"), or a label that says nothing but a
/// kind („Pflichtmodule", „Wahlpflichtmodule (KT)", „Compulsory Elective and Optional Modules").
/// Such a node is no heading of the pickers.
pub fn is_structural(label: &str) -> bool {
    const PHASES: &[&str] = &["grundstudium", "fachstudium", "hauptstudium", "basisstudium", "kernstudium", "vertiefungsstudium", "grundlagenstudium", "bachelorstudium", "masterstudium"];
    const ACCOUNTS: &[&str] = &["gesamtkonto", "total account", "module an der ", "module an den ", "modules at ", "studierende der ", "students of "];
    let folded = crate::search::fold(label.trim());
    PHASES.contains(&folded.as_str()) || ACCOUNTS.iter().any(|account| folded.starts_with(account)) || is_kind_only(label)
}

/// A label that says nothing but what kind of modules lie there („Wahlpflichtmodule (KT)").
fn is_kind_only(label: &str) -> bool {
    crate::plan::distinctive(crate::plan::without_direction(label)).is_empty()
}

/// A heading or a qualifier as the pickers show it: without „Komplex" in front, which only says
/// that it is one, without the account a double degree puts in front („Total Account - "), and
/// with a study direction the source printed twice („… (EM) (EM)") once.
fn display_label(label: &str) -> String {
    let mut shown = label.trim();
    for prefix in ["Komplex ", "Gesamtkonto - ", "Total Account - "] {
        if let Some(rest) = shown.strip_prefix(prefix).filter(|rest| rest.chars().count() >= 3) {
            shown = rest;
            break;
        }
    }
    if let Some((before, last)) = shown.rsplit_once(" (") {
        if before.ends_with(&format!("({last}")) {
            return before.to_string();
        }
    }
    shown.to_string()
}

/// Whether a student chooses among these modules (see `CatalogArea::choice`): any of them is
/// not known to be compulsory, the thesis or the internship.
pub fn is_choice<'a>(placements: impl IntoIterator<Item = &'a AreaPlacement>) -> bool {
    placements.into_iter().any(|placement| {
        let kind = placement.module_kind.as_ref().or(placement.kind.as_ref());
        !kind.is_some_and(|kind| kind.is(ModuleKind::Compulsory) || kind.is(ModuleKind::Thesis) || kind.is(ModuleKind::Internship))
    })
}

/// A section longer than this splits into the fields below its node, where there are such.
const LONG_SECTION: usize = 12;

/// The areas of a program in tree order, one entry per area the tree places modules in, each
/// with the nodes above it (`tree`, `queries::program_area_tree`), its name and its heading in
/// the pickers. Headings and names are decided among the areas a student chooses from
/// (`CatalogArea::choice`), the ones the pickers show. The rule comes from the 179 trees of the
/// 2026-09-21 snapshot (docs/frontend.md, „The area picker"); the owner asked for few, stable
/// sections, never the same heading twice, no structural node as a heading (2026-09-21):
///
/// - An area's heading is the **highest node above it that names a field** — not a phase, an
///   account or a mere kind (`is_structural`): „Komplex Nebenfach" for Mathematik, Physik …,
///   „Komplex Berufsfeld" for its Schwerpunkte, however deep they lie. One level of headings,
///   so a section never splits into sections of its own — unless it would be longer than
///   `LONG_SECTION` and the fields below its node hold it: then those are the headings
///   („Ingenieurwissenschaftlicher Schwerpunkt" → „Produktionstechnik", „Umwelttechnik" …).
/// - An area whose label says nothing but a kind („Wahlpflichtmodule" below „Entwerfen") takes
///   the name of the field it lies in, and that field is then no heading of it — unless other
///   areas stand under that field as their heading: then it joins them under its own label.
///   An area that is itself the node of a heading („Fachspezifisches Studium" and its fields)
///   opens that section.
/// - A heading with a single area under it is none: the area joins the areas without a heading
///   (the section above it).
/// - Two headings that read the same are one section. The areas without a heading come first,
///   then the sections in the order of the tree (`area_sections`).
/// - Two areas that would read the same in one section (or one without a heading and any
///   other) are told apart by the node above them that the fewest of the others lie in, a
///   field before a structural node, the nearest first („Mathematik", „Mathematik
///   (Anwendungen)"); failing that, an area named after the field above goes by its label.
pub fn catalog_areas(placements: &[AreaPlacement], tree: &[AreaNode]) -> Vec<CatalogArea> {
    let mut areas: Vec<CatalogArea> = Vec::new();
    for placement in placements {
        let choice = is_choice([placement]);
        match areas.iter_mut().find(|area| area.id == placement.area_id) {
            Some(area) => {
                area.modules += 1;
                area.choice |= choice;
            }
            None => areas.push(CatalogArea {
                id: placement.area_id,
                label: placement.area_label.clone(),
                path: placement.area.clone(),
                depth: placement.depth,
                modules: 1,
                choice,
                ancestors: Vec::new(),
                name: short_name(&placement.area_label).to_string(),
                section: None,
            }),
        }
    }
    // The nodes above each area, from the top down (a broken tree ends the walk, never loops).
    let node = |id: i64| tree.iter().find(|node| node.id == id);
    let above = |id: i64| -> Vec<&AreaNode> {
        let mut chain: Vec<&AreaNode> = Vec::new();
        let mut next = node(id).and_then(|node| node.parent_id);
        while let Some(parent) = next.and_then(node) {
            if chain.len() > 32 || chain.iter().any(|known| known.id == parent.id) {
                break;
            }
            chain.push(parent);
            next = parent.parent_id;
        }
        chain.reverse();
        chain
    };
    let chains: Vec<Vec<&AreaNode>> = areas.iter().map(|area| above(area.id)).collect();
    for (area, chain) in areas.iter_mut().zip(&chains) {
        area.ancestors = chain.iter().map(|node| node.label.clone()).collect();
    }

    // Each area a student chooses from: the fields above it, the field that gave it its name,
    // the node of its heading.
    struct Placed<'a> {
        index: usize,
        fields: Vec<&'a AreaNode>,
        named_by: Option<i64>,
        heading: Option<i64>,
    }
    let mut placed: Vec<Placed> = Vec::new();
    for (index, (area, chain)) in areas.iter_mut().zip(&chains).enumerate() {
        if !area.choice {
            continue;
        }
        let fields: Vec<&AreaNode> = chain.iter().copied().filter(|node| !is_structural(&node.label)).collect();
        let named_by = if is_kind_only(&area.label) {
            area.name = match fields.last() {
                Some(field) => display_label(&field.label),
                None => area.label.clone(),
            };
            fields.last().map(|field| field.id)
        } else {
            None
        };
        let heading = fields.iter().find(|field| Some(field.id) != named_by).map(|field| field.id);
        placed.push(Placed { index, fields, named_by, heading });
    }
    // A long section splits into the fields below its node.
    let headings: Vec<i64> = placed.iter().filter_map(|area| area.heading).collect();
    for heading in headings.iter().copied().collect::<std::collections::BTreeSet<i64>>() {
        if headings.iter().filter(|other| **other == heading).count() <= LONG_SECTION {
            continue;
        }
        let below = |area: &Placed| -> Option<i64> {
            let at = area.fields.iter().position(|field| field.id == heading)?;
            area.fields.get(at + 1).map(|field| field.id).filter(|id| Some(*id) != area.named_by)
        };
        let members: Vec<Option<i64>> = placed.iter().filter(|area| area.heading == Some(heading)).map(below).collect();
        let fields: std::collections::BTreeSet<i64> = members.iter().flatten().copied().collect();
        let held = fields.iter().filter(|field| members.iter().filter(|member| **member == Some(**field)).count() >= 2).count();
        if held >= 2 {
            for area in placed.iter_mut().filter(|area| area.heading == Some(heading)) {
                if let Some(field) = below(area) {
                    area.heading = Some(field);
                }
            }
        }
    }
    // An area that is itself the node of a heading, or was named after one, joins that section.
    let heading_nodes: Vec<i64> = placed.iter().filter_map(|area| area.heading).collect();
    for area in placed.iter_mut() {
        let id = areas.get(area.index).map(|area| area.id);
        if area.heading.is_none() && id.is_some_and(|id| heading_nodes.contains(&id)) {
            area.heading = id;
        } else if area.named_by.is_some_and(|field| heading_nodes.contains(&field)) {
            area.heading = area.named_by;
            area.named_by = None;
            if let Some(own) = areas.get_mut(area.index) {
                own.name = own.label.clone();
            }
        }
    }
    let texts: Vec<Option<String>> = placed.iter().map(|area| area.heading.and_then(node).map(|node| display_label(&node.label))).collect();
    for (area, text) in placed.iter().zip(&texts) {
        // A heading over a single area is none.
        let alone = text.as_ref().is_some_and(|text| texts.iter().filter(|other| other.as_ref() == Some(text)).count() < 2);
        if let Some(own) = areas.get_mut(area.index) {
            own.section = if alone { None } else { text.clone() };
        }
    }

    // Two areas that read the same where they stand.
    let key = |area: &CatalogArea| crate::search::fold(&area.name);
    let mut renamed: Vec<(usize, String)> = Vec::new();
    for area in &placed {
        let (Some(own), Some(chain)) = (areas.get(area.index), chains.get(area.index)) else { continue };
        let alike: Vec<usize> = placed
            .iter()
            .map(|other| other.index)
            .filter(|other| *other != area.index)
            .filter(|other| areas.get(*other).is_some_and(|theirs| key(theirs) == key(own) && (own.section.is_none() || theirs.section == own.section)))
            .collect();
        if alike.is_empty() {
            continue;
        }
        let sharing = |label: &str| alike.iter().filter(|other| chains.get(**other).is_some_and(|theirs| theirs.iter().any(|node| node.label == label))).count();
        let heading = own.section.clone();
        // The node above that the fewest of the others lie in as well; a field before a
        // structural node, the nearest first.
        let distinct = chain
            .iter()
            .rev()
            .filter(|node| Some(node.id) != area.named_by && heading.as_deref() != Some(display_label(&node.label).as_str()))
            .map(|node| (sharing(&node.label), is_structural(&node.label), *node))
            .filter(|(shared, ..)| *shared < alike.len())
            .enumerate()
            .min_by_key(|(nearness, (shared, structural, _))| (*shared, *structural, *nearness))
            .map(|(_, (_, _, node))| node);
        match distinct {
            Some(distinct) => renamed.push((area.index, format!("{} ({})", own.name, display_label(&distinct.label)))),
            // Named after the field above, and nothing above tells them apart: their own labels do.
            None if area.named_by.is_some() => renamed.push((area.index, own.label.clone())),
            None => {}
        }
    }
    for (index, name) in renamed {
        if let Some(area) = areas.get_mut(index) {
            area.name = name;
        }
    }
    areas
}

/// The areas the pickers offer, as they show them: those without a heading first, then one
/// section per heading in the order of the tree, each area in the order of the tree.
pub fn area_sections(areas: &[CatalogArea]) -> Vec<(Option<String>, Vec<CatalogArea>)> {
    let mut sections: Vec<(Option<String>, Vec<CatalogArea>)> = vec![(None, Vec::new())];
    for area in areas.iter().filter(|area| area.choice) {
        match sections.iter_mut().find(|(heading, _)| *heading == area.section) {
            Some((_, members)) => members.push(area.clone()),
            None => sections.push((area.section.clone(), vec![area.clone()])),
        }
    }
    sections.retain(|(_, members)| !members.is_empty());
    sections
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
}

pub fn catalog(db: &dyn Database, url: &CatalogUrl) -> Result<CatalogData, DbError> {
    let scope = catalog_scope(db, &url.query)?;
    Ok(CatalogData {
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
    let scope = catalog_scope(db, query)?;
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

fn catalog_scope(db: &dyn Database, query: &CatalogQuery) -> Result<CatalogScope, DbError> {
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
    let mut semester_plan = None;
    if let (Some(scope), Some(program)) = (query.program.as_mut(), program.as_ref()) {
        if let (Some(PlanSemesterFilter::Semester(semester)), true) = (scope.plan_semester, program.has_plan) {
            let plan = plan::semester_plan(semester, &queries::program_plan_entries(db, &program.id)?, &areas);
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
    /// Every node of the module tree, those without modules included (`catalog_areas`).
    pub area_tree: Vec<AreaNode>,
    pub plan: Option<Plan>,
    pub plan_entries: Vec<PlanEntry>,
    /// What the regulation says its plan adds up to, and which rows each sum counts.
    pub plan_totals: Vec<PlanTotal>,
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
    let ids: Vec<String> = ids.iter().filter(|id| crate::url::is_module_id(id) && seen.insert(id.as_str())).take(MAX_BOOKMARKS).cloned().collect();
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

#[cfg(test)]
mod area_tests {
    use super::*;
    use crate::area_fixtures as real;

    /// The picker as it reads: (heading, names).
    fn picker(nodes: &[real::Node]) -> Vec<(Option<String>, Vec<String>)> {
        area_sections(&real::areas(nodes)).into_iter().map(|(heading, areas)| (heading, areas.iter().map(|area| area.name().to_string()).collect())).collect()
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn the_picker_of_informatik_offers_its_own_electives_and_the_nebenfach() {
        // Owner, 2026-09-21: the own electives and the Anwendungs-/Nebenfach, no fixed complex,
        // no heading twice, no „Grundstudium"/„Fachstudium". Praktische Mathematik lies in the
        // Komplex Nebenfach of the tree, so it stands there (docs/frontend.md says why).
        assert_eq!(
            picker(real::INFORMATIK_BSC),
            vec![
                (None, strings(&["Proseminar oder Praktikum", "Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Seminar oder Praktikum aus der Informatik"])),
                (Some("Nebenfach".to_string()), strings(&["Praktische Mathematik", "Mathematik", "Physik", "Maschinenbau / Elektrotechnik", "Wirtschaftswissenschaften", "Bauingenieurwesen"])),
            ]
        );
        let areas = real::areas(real::INFORMATIK_BSC);
        let fixed: Vec<&str> = areas.iter().filter(|area| !area.choice).map(|area| area.label.as_str()).collect();
        assert_eq!(fixed, vec!["Komplex Informatik", "Komplex Mathematik", "Fachstudium"]);
        // The parent comes from the tree, never from the path: a label may hold the separator.
        let slashed = areas.iter().find(|area| area.label == "Maschinenbau / Elektrotechnik").map(|area| (area.parent(), area.ancestors.clone()));
        assert_eq!(slashed, Some((Some("Komplex Nebenfach"), strings(&["Grundstudium", "Komplex Nebenfach"]))));

        // The Master: two fields, and the two areas called Mathematik told apart.
        assert_eq!(
            picker(real::INFORMATIK_MSC),
            vec![
                (Some("Informatik-Vertiefung".to_string()), strings(&["Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Seminare oder Praktika"])),
                (Some("Nebenfach".to_string()), strings(&["Mathematik", "Mathematik (Anwendungen)", "Physik", "Maschinenbau / Elektrotechnik", "Wirtschaftsingenieurwesen", "Bauingenieurwesen"])),
            ]
        );
    }

    #[test]
    fn an_area_named_only_by_its_kind_takes_the_name_of_its_field() {
        // Architektur: every field has its „Wahlpflichtmodule"; the account on top is no heading.
        assert_eq!(picker(real::ARCHITEKTUR_BSC), vec![(None, strings(&["Entwerfen", "Bautechnik und Ökologie", "Geschichte und Theorie"]))]);
        // Elektrotechnik B.Sc.: „Wahlpflichtmodul (MIT)" is the list of „Informatik (MIT)".
        assert_eq!(
            picker(real::ELEKTROTECHNIK_BSC),
            vec![(
                None,
                strings(&[
                    "Informatik (MIT)",
                    "Informatik (EET)",
                    "Hauptstudium",
                    "Studienrichtungsspezifische Vertiefungsmodule (EET)",
                    "Studienrichtungsspezifische Vertiefungsmodule (MIT)",
                    "Studienrichtungsspezifische Vertiefungsmodule (PAu)",
                    "Studienrichtungsspezifische Vertiefungsmodule (IoT)",
                ])
            )]
        );
        // Elektrotechnik M.Sc.: the field holds another area as well, so it is the heading, and
        // the list keeps its own label.
        assert_eq!(
            picker(real::ELEKTROTECHNIK_MSC),
            vec![
                (Some("Studienrichtung Kommunikationstechnik (KT)".to_string()), strings(&["Wahlpflichtmodule (KT)", "Zweite Fremdsprache"])),
                (Some("Studienrichtung Prozessautomatisierung (PAu)".to_string()), strings(&["Wahlpflichtmodule (PAu)", "Zweite Fremdsprache"])),
                (Some("Studienrichtung Energiesysteme (ES)".to_string()), strings(&["Wahlpflichtmodule (ES)", "Zweite Fremdsprache"])),
            ]
        );
    }

    #[test]
    fn a_long_section_splits_into_the_fields_below_it() {
        let sections = picker(real::WIRTSCHAFTSINGENIEURWESEN_DUAL);
        let headings: Vec<Option<&str>> = sections.iter().map(|(heading, _)| heading.as_deref()).collect();
        assert_eq!(
            headings,
            vec![None, Some("Wirtschaftswissenschaftlicher Bereich"), Some("Produktionstechnik"), Some("Umwelttechnik"), Some("Energiesysteme"), Some("Bauingenieurwesen"), Some("Elektro- und Informationstechnik")]
        );
        assert_eq!(sections[0].1, strings(&["Mathematisch-Methodischer Bereich", "Praxisintegrierendes Studium"]));
        assert_eq!(sections[3].1, strings(&["Pflichtbereich Umwelttechnik", "Umwelttechnik", "Kreislauf und Entsorgung", "Wassertechnik"]));
    }

    #[test]
    fn no_heading_twice_none_over_one_area_none_that_only_structures() {
        for tree in [real::INFORMATIK_BSC, real::INFORMATIK_MSC, real::ELEKTROTECHNIK_BSC, real::ELEKTROTECHNIK_MSC, real::ARCHITEKTUR_BSC, real::WIRTSCHAFTSINGENIEURWESEN_DUAL] {
            let sections = picker(tree);
            let headings: Vec<&String> = sections.iter().filter_map(|(heading, _)| heading.as_ref()).collect();
            for (i, heading) in headings.iter().enumerate() {
                assert!(!headings.iter().skip(i + 1).any(|other| other == heading), "{heading} twice");
                assert!(!is_structural(heading), "{heading}");
            }
            assert!(sections.iter().all(|(heading, names)| heading.is_none() || names.len() >= 2), "{sections:?}");
            assert!(sections.iter().skip(1).all(|(heading, _)| heading.is_some()), "the areas without a heading come first: {sections:?}");
        }
        for label in ["Grundstudium", "Fachstudium", "Hauptstudium", "Gesamtkonto Bachelor", "Total Account - Home BTU", "Modules at the Deakin University", "Pflichtmodule", "Wahlpflichtmodule (KT)", "Compulsory Elective and Optional Modules", "Mandatory Modules"] {
            assert!(is_structural(label), "{label}");
        }
        for label in ["Komplex Nebenfach", "Studienrichtung Kommunikationstechnik (KT)", "Schwerpunkte", "Anwendungen", "Informatik-Vertiefung"] {
            assert!(!is_structural(label), "{label}");
        }
    }

    #[test]
    fn the_name_of_an_area_drops_the_word_that_says_it_is_a_choice() {
        assert_eq!(short_name("Wahlpflichtmodule Praktische Mathematik"), "Praktische Mathematik");
        assert_eq!(short_name("Wahlpflichtmodul aus dem Nebenfach"), "Nebenfach");
        assert_eq!(short_name("Wahlpflichtmodule der Berufsfelder"), "Berufsfelder");
        assert_eq!(short_name("Wahlbereich Grundlage der Statistik"), "Grundlage der Statistik");
        assert_eq!(short_name("Wahlpflichtmodule"), "Wahlpflichtmodule");
        assert_eq!(short_name("Wahlpflicht (CIV)"), "Wahlpflicht (CIV)");
        assert_eq!(short_name("Wahlpflichtbereich II (SUR)"), "Wahlpflichtbereich II (SUR)");
        assert_eq!(short_name("Praktische Informatik"), "Praktische Informatik");
        assert_eq!(display_label("Komplex Nebenfach"), "Nebenfach");
        assert_eq!(display_label("Studienrichtung Elektrische Medizintechnik (EM) (EM)"), "Studienrichtung Elektrische Medizintechnik (EM)");
        assert_eq!(display_label("Total Account - Home BTU"), "Home BTU");
    }
}
