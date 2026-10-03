//! The typed filter state of the module catalog; its translation to SQL is `folia_query::sql`.
//!
//! `CatalogQuery` is what a catalog URL encodes and the only thing that triggers a
//! catalog query. It filters on the facet columns of the views, never on free text
//! (the one exception is the search, `folia_search::Plan` against `v_module_folded`).

use folia_calendar::select::FitOptions;
use folia_calendar::semester::SemesterKey;
use folia_locale::Locale;
use folia_model::labels::{Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};
use folia_search::{Plan, Resolution};
use serde::{Deserialize, Serialize};

/// Which modules of a program to list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgramRelation {
    /// The modules of the curriculum.
    #[default]
    Curricular,
    /// The program's own list of FÜS modules (never part of its curriculum).
    Fues,
}

impl ProgramRelation {
    pub fn code(self) -> &'static str {
        match self {
            ProgramRelation::Curricular => "curricular",
            ProgramRelation::Fues => "fues",
        }
    }
}

/// A kind to filter for; `Unstated` selects the modules no source states a kind for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KindFilter {
    Stated(ModuleKind),
    Unstated,
}

/// A semester of the validated study plan; `Unstated` selects modules the plan does not place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanSemesterFilter {
    Semester(u8),
    Unstated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProgramScope {
    /// `v_program.slug`: what URLs carry, so no lookup is needed to build the query.
    pub program_slug: String,
    pub relation: ProgramRelation,
    pub plan_semester: Option<PlanSemesterFilter>,
    /// Any of these; empty means all.
    pub kinds: Vec<KindFilter>,
    /// None of these.
    pub kinds_exclude: Vec<KindFilter>,
    /// Areas of the program's module tree (`v_program_module_area.area_id`): only the modules
    /// the tree places in one of them or in an area below one; empty means all. „Wahlpflicht­
    /// module Praktische Informatik" is such an area; the tree, not the plan, is the authority
    /// for structure. Several come from a row of the plan that means several areas
    /// („Anwendungsfach": Mathematik, Physik …), opened from the program's page.
    pub areas: Vec<i64>,
    /// Derived, never part of a URL (`pages::catalog` fills them in, like the marked modules):
    /// with a semester chosen, the areas whose modules can be chosen for the plan's requirement
    /// rows of that semester („Wahlpflichtmodule der Informatik", `plan::semester_plan`), listed
    /// with the modules the plan places there …
    pub semester_areas: Vec<i64>,
    /// … and whether every elective module the plan places nowhere counts as well (a row of the
    /// plan that points at no area).
    pub semester_electives: bool,
}

/// Any of the ticked offers matches; nothing ticked means no turnus filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnusFilter {
    pub winter: bool,
    pub summer: bool,
    pub irregular: bool,
    /// Leave out what is known to be offered then. Unknown stays: an exclusion only removes
    /// what the data states (the same holds for every `*_exclude` below).
    pub not_winter: bool,
    pub not_summer: bool,
    pub not_irregular: bool,
    /// Keep modules offered in years of this parity (modules without a parity always match).
    pub year_parity: Option<TurnusParity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExamPart {
    Written,
    Oral,
    Paper,
    Presentation,
    Project,
    Practical,
}

impl ExamPart {
    pub const ALL: &'static [Self] =
        &[Self::Written, Self::Oral, Self::Paper, Self::Presentation, Self::Project, Self::Practical];

    /// What the chips of the filter say.
    pub fn short_label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, ExamPart::Written) => "Klausur",
            (Locale::De, ExamPart::Oral) => "Mündlich",
            (Locale::De, ExamPart::Paper) => "Hausarbeit",
            (Locale::De, ExamPart::Presentation) => "Vortrag",
            (Locale::De, ExamPart::Project) => "Projekt",
            (Locale::De, ExamPart::Practical) => "Praktisch",
            (Locale::En, ExamPart::Written) => "Written",
            (Locale::En, ExamPart::Oral) => "Oral",
            (Locale::En, ExamPart::Paper) => "Paper",
            (Locale::En, ExamPart::Presentation) => "Presentation",
            (Locale::En, ExamPart::Project) => "Project",
            (Locale::En, ExamPart::Practical) => "Practical",
        }
    }

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, ExamPart::Written) => "Klausur",
            (Locale::De, ExamPart::Oral) => "mündliche Prüfung",
            (Locale::De, ExamPart::Paper) => "Hausarbeit / Beleg",
            (Locale::De, ExamPart::Presentation) => "Vortrag",
            (Locale::De, ExamPart::Project) => "Projektarbeit",
            (Locale::De, ExamPart::Practical) => "praktische Prüfung",
            (Locale::En, ExamPart::Written) => "written exam",
            (Locale::En, ExamPart::Oral) => "oral exam",
            (Locale::En, ExamPart::Paper) => "term paper / report",
            (Locale::En, ExamPart::Presentation) => "presentation",
            (Locale::En, ExamPart::Project) => "project work",
            (Locale::En, ExamPart::Practical) => "practical exam",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    German,
    English,
}

impl Language {
    pub const ALL: &'static [Self] = &[Self::German, Self::English];

    pub fn code(self) -> &'static str {
        match self {
            Language::German => "de",
            Language::English => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|language| language.code() == code)
    }

    /// The language a module is taught in, as a page in `locale` names it.
    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, Language::German) => "Deutsch",
            (Locale::De, Language::English) => "Englisch",
            (Locale::En, Language::German) => "German",
            (Locale::En, Language::English) => "English",
        }
    }
}

/// „Passt in meinen Stundenplan": the semester whose timetable a module must fit, which classes
/// are compared, and whether modules without dated rows are listed too. The URL carries it
/// (`fits=2026W&fits-skip=exercise&fits-undated=1`), since a semester is public and says nothing
/// about the visitor; which modules fit is worked out by the browser from the plan it keeps
/// (`CatalogQuery::fits_ids`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FitsFilter {
    /// `SemesterKey::key()`: `2026W`.
    pub semester: String,
    /// Lectures are compared (all of a module's lectures must be free).
    pub lectures: bool,
    /// Everything taught that is not a lecture is compared (one of a module's events must be free).
    pub exercises: bool,
    /// Exams are compared (one sitting must avoid the plan's fixed exams).
    pub exams: bool,
    /// Modules without a dated row in the semester are listed too. They cannot be checked, so
    /// by default they are not: 2,013 of the 3,232 modules on offer have none in WiSe 2026/27, and
    /// listing them would say they fit.
    pub undated: bool,
}

impl FitsFilter {
    /// The codes of `fits-skip`, in the order the address writes them.
    pub const CLASSES: [&'static str; 3] = ["lecture", "exercise", "exam"];

    /// What the switch turns on: every class compared, modules without dates left out. A key is
    /// written as `SemesterKey` writes it (`2026w` → `2026W`), so equal filters give equal
    /// addresses.
    pub fn all(semester: &str) -> Self {
        let semester = SemesterKey::parse(semester).map(SemesterKey::key).unwrap_or_else(|| semester.trim().to_string());
        Self { semester, lectures: true, exercises: true, exams: true, undated: false }
    }

    /// The classes the finder compares.
    pub fn options(&self) -> FitOptions {
        FitOptions { lectures: self.lectures, exercises: self.exercises, exams: self.exams }
    }

    /// The codes of the classes that are not compared, in canonical order (`fits-skip`).
    pub fn skipped(&self) -> Vec<&'static str> {
        let [lecture, exercise, exam] = Self::CLASSES;
        [(self.lectures, lecture), (self.exercises, exercise), (self.exams, exam)]
            .into_iter()
            .filter(|(compared, _)| !compared)
            .map(|(_, code)| code)
            .collect()
    }
}

/// Which modules the fit switch lists, filled in by the browser from the plan it keeps and never
/// part of a URL (R20, like the marked modules): only these (the modules that were checked and
/// fit), or every module but these (the clashing and the planned ones, when modules without dates
/// are listed too or the semester has no dates yet).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FitIds {
    Only(Vec<String>),
    Without(Vec<String>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortKey {
    /// By title; inside a program in the order of its study plan (semester, then title).
    #[default]
    Default,
    Title,
    Id,
    Credits,
    /// Number of teaching events in the module's newest semester.
    Events,
}

/// The filter state of the catalog. `Default` is "no filter".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogQuery {
    /// Search text: its words are found in the numbers, titles, initials and abbreviations of
    /// the modules (`search`).
    pub text: String,
    /// Derived from `text` by the page (`pages::catalog`, `search::resolve`), never part of a
    /// URL: how the text was searched where the text as typed found nothing (its typos corrected,
    /// or the modules with the most of its words). `None` searches the text as typed.
    /// Always written: what crosses to the data worker is read back by a format that does not
    /// describe itself (postcard), which cannot skip a field (R29).
    #[serde(default)]
    pub text_resolution: Option<Resolution>,
    pub program: Option<ProgramScope>,
    /// At least one of these teaches or is responsible for the module (owner decision
    /// 2026-09-20: „Meer oder Köhler", not „Meer und Köhler").
    pub lecturers_include: Vec<String>,
    /// None of these teaches or is responsible for the module.
    pub lecturers_exclude: Vec<String>,
    pub department_id: Option<i64>,
    pub turnus: TurnusFilter,
    /// Any of these.
    pub teaching_forms: Vec<TeachingForm>,
    /// None of these.
    pub teaching_forms_exclude: Vec<TeachingForm>,
    pub duration_semesters: Option<u8>,
    /// `Some(true)`: only modules with a participant limit, `Some(false)`: only without.
    pub limited: Option<bool>,
    /// `Some(true)`: only modules of the general FÜS list, `Some(false)`: none of them.
    pub fues: Option<bool>,
    /// Any of these.
    pub exam_forms: Vec<ExamForm>,
    /// Any of these.
    pub exam_parts: Vec<ExamPart>,
    /// None of these: „keine Vorträge".
    pub exam_parts_exclude: Vec<ExamPart>,
    pub graded: Option<bool>,
    /// `Some(true)`: only modules with published teaching events (the list's „Termine"; owner,
    /// 2026-09-23: hide what probably does not take place), `Some(false)`: only those with none
    /// yet („noch keine"). The events are those of the module's newest semester that has any.
    pub scheduled: Option<bool>,
    /// Any of these. `None` is the default, see `effective_offer`.
    pub offer: Option<Vec<OfferStatus>>,
    pub credits_min: Option<f64>,
    pub credits_max: Option<f64>,
    /// Any of these. Only modules with room data can match (campus is unknown otherwise).
    pub campuses: Vec<Campus>,
    /// None of these (modules without room data stay: their campus is unknown).
    pub campuses_exclude: Vec<Campus>,
    /// Any of these.
    pub languages: Vec<Language>,
    /// None of these.
    pub languages_exclude: Vec<Language>,
    /// Restrict to these module ids: the „Gemerkt" and „Bestanden" views.
    pub only_ids: Option<Vec<String>>,
    /// Leave these module ids out („ohne Gemerkte").
    pub without_ids: Vec<String>,
    /// „Gemerkt": `Some(true)` only marked modules, `Some(false)` none of them. What is marked
    /// lives in the browser (R20), so the URL carries only the switch and the browser app fills
    /// `only_ids` / `without_ids` before it asks. Where nothing filled them, „only marked"
    /// matches nothing: a page that does not know the marks must not answer as if there were none.
    pub marked: Option<bool>,
    /// Keep modules whose mandatory prerequisites are all in this set of passed modules.
    pub prerequisites_met_by: Option<Vec<String>>,
    pub sort: SortKey,
    pub descending: bool,
    /// „Passt in meinen Stundenplan": the URL carries the switch, the browser fills `fits_ids`
    /// from the plan before it asks. Where nothing filled them, nothing is listed: a page that does
    /// not know the plan (the server's) must not answer as if every module fit.
    pub fits: Option<FitsFilter>,
    /// Derived from the plan, never part of a URL; ignored without `fits`.
    pub fits_ids: Option<FitIds>,
}

impl CatalogQuery {
    /// Which offer states are listed. By default modules that are no longer offered are
    /// hidden (1,649 of the 1,704 are in no curriculum at all), except inside a program:
    /// there the list shows everything the curriculum names. Phase-out modules stay visible.
    pub fn effective_offer(&self) -> Vec<OfferStatus> {
        match (&self.offer, &self.program) {
            (Some(chosen), _) => chosen.clone(),
            (None, Some(_)) => OfferStatus::ALL.to_vec(),
            (None, None) => vec![OfferStatus::Active, OfferStatus::PhaseOut],
        }
    }

    /// How many filters are active (for „Filter zurücksetzen (3)"). Sorting is not a filter.
    pub fn active_filters(&self) -> usize {
        let t = &self.turnus;
        let program = self.program.as_ref();
        [
            !self.text.trim().is_empty(),
            program.is_some(),
            program.is_some_and(|p| p.plan_semester.is_some()),
            program.is_some_and(|p| !p.kinds.is_empty() || !p.kinds_exclude.is_empty()),
            program.is_some_and(|p| !p.areas.is_empty()),
            self.department_id.is_some(),
            t.winter || t.summer || t.irregular || t.not_winter || t.not_summer || t.not_irregular || t.year_parity.is_some(),
            !self.teaching_forms.is_empty() || !self.teaching_forms_exclude.is_empty(),
            self.duration_semesters.is_some(),
            self.limited.is_some(),
            self.fues.is_some(),
            !self.exam_forms.is_empty() || !self.exam_parts.is_empty() || !self.exam_parts_exclude.is_empty(),
            self.graded.is_some(),
            self.scheduled.is_some(),
            self.offer.is_some(),
            self.credits_min.is_some() || self.credits_max.is_some(),
            !self.campuses.is_empty() || !self.campuses_exclude.is_empty(),
            !self.languages.is_empty() || !self.languages_exclude.is_empty(),
            self.prerequisites_met_by.is_some(),
            self.marked.is_some(),
            self.fits.is_some(),
        ]
        .iter()
        .filter(|active| **active)
        .count()
            + self.lecturers_include.len()
            + self.lecturers_exclude.len()
    }

    /// The plan of the search text, as its resolution has it searched; `None` without a text.
    pub fn search_plan(&self) -> Option<Plan> {
        Plan::new(&self.text, self.text_resolution.as_ref())
    }

    /// Whether the list is ordered by how well the modules match the search text: while there
    /// is one and no column was chosen to order by (owner, 2026-09-30).
    pub fn by_relevance(&self) -> bool {
        self.sort == SortKey::Default && self.search_plan().is_some_and(|plan| plan.len() > 0)
    }

}
