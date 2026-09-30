//! Structs that mirror what the queries select from the read views.
//!
//! `Option` means the source does not say: unknown stays unknown all the way to the page.

use serde::{Deserialize, Serialize};

use crate::db::{DbError, FromRow, Row};
use crate::labels::{
    Code, DegreeLevel, ExamForm, KindBasis, KindSource, ModuleKind, OfferStatus, PlanStatus,
    PrerequisiteKind, Relation, Season, StudySection, StudyVariant, TurnusParity, TurnusSeason,
};

/// `v_meta`, the keys the UI shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub built_at: Option<String>,
    pub data_changed_at: Option<String>,
    pub current_semester: Option<String>,
    pub content_digest: Option<String>,
    /// The Radix that built the snapshot (`internal/version`); older snapshots do not say.
    pub radix_version: Option<String>,
}

/// `v_semester`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Semester {
    pub key: String,
    pub season: Code<Season>,
    pub year: i64,
    pub label: String,
    pub starts_on: String,
    pub ends_on: String,
    pub is_current: bool,
    pub teaching_events: i64,
    pub exam_events: i64,
}

impl FromRow for Semester {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            key: row.text("key")?,
            season: Code::parse(&row.text("season")?),
            year: row.int("year")?,
            label: row.text("label")?,
            starts_on: row.text("starts_on")?,
            ends_on: row.text("ends_on")?,
            is_current: row.flag("is_current")?,
            teaching_events: row.int("teaching_events")?,
            exam_events: row.int("exam_events")?,
        })
    }
}

/// `v_department`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Department {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub name_de: String,
    pub name_en: Option<String>,
    pub modules: i64,
}

impl FromRow for Department {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.int("id")?,
            code: row.text("code")?,
            label: row.text("label")?,
            name_de: row.text("name_de")?,
            name_en: row.opt_text("name_en")?,
            modules: row.int("modules")?,
        })
    }
}

/// `v_program`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub degree_level: Code<DegreeLevel>,
    pub study_variant: Option<Code<StudyVariant>>,
    /// „B.Sc." only where a source states it.
    pub degree_label: Option<String>,
    pub degree_raw: String,
    /// `degree_label`, else „Bachelor" / „Master"; `None` where even that is unknown.
    pub degree_display: Option<String>,
    pub po_version: String,
    pub po_year: Option<i64>,
    pub family_key: String,
    /// The same subject across degree levels (Bachelor and Master of one name).
    pub name_key: String,
    pub is_latest_po: bool,
    pub source_url: String,
    pub has_plan: bool,
    pub plan_status: Option<Code<PlanStatus>>,
    pub curricular_modules: i64,
    pub fues_modules: i64,
    pub documents: i64,
}

impl Program {
    /// What to print as the degree: never invented, falls back to the raw QIS text.
    pub fn degree(&self) -> &str {
        self.degree_display.as_deref().unwrap_or(&self.degree_raw)
    }
}

impl FromRow for Program {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("id")?,
            slug: row.text("slug")?,
            name: row.text("name")?,
            degree_level: Code::parse(&row.text("degree_level")?),
            study_variant: Code::parse_opt(row.opt_text("study_variant")?),
            degree_label: row.opt_text("degree_label")?,
            degree_raw: row.text("degree_raw")?,
            degree_display: row.opt_text("degree_display")?,
            po_version: row.text("po_version")?,
            po_year: row.opt_int("po_year")?,
            family_key: row.text("family_key")?,
            name_key: row.text("name_key")?,
            is_latest_po: row.flag("is_latest_po")?,
            source_url: row.text("source_url")?,
            has_plan: row.flag("has_plan")?,
            plan_status: Code::parse_opt(row.opt_text("plan_status")?),
            curricular_modules: row.int("curricular_modules")?,
            fues_modules: row.int("fues_modules")?,
            documents: row.int("documents")?,
        })
    }
}

/// `v_program_module`: one module of one program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramModule {
    pub module_id: String,
    pub relation: Code<Relation>,
    /// `None`: no source states the kind. Never shown as „Pflicht".
    pub kind: Option<Code<ModuleKind>>,
    pub kind_source: Option<Code<KindSource>>,
    pub kind_basis: Option<Code<KindBasis>>,
    pub area: Option<String>,
    pub section: Option<Code<StudySection>>,
    pub module_title: String,
    pub module_credits: Option<f64>,
    pub offer_status: Code<OfferStatus>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    /// Only from a validated study plan.
    pub plan_semester: Option<i64>,
}

impl FromRow for ProgramModule {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            relation: Code::parse(&row.text("relation")?),
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_source: Code::parse_opt(row.opt_text("kind_source")?),
            kind_basis: Code::parse_opt(row.opt_text("kind_basis")?),
            area: row.opt_text("area")?,
            section: Code::parse_opt(row.opt_text("section")?),
            module_title: row.text("module_title")?,
            module_credits: row.opt_real("module_credits")?,
            offer_status: Code::parse(&row.text("offer_status")?),
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            plan_semester: row.opt_int("plan_semester")?,
        })
    }
}

/// One row of the catalog table: `v_module_facets` joined with `v_module`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogRow {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    pub turnus_parity: Option<Code<TurnusParity>>,
    pub offer_status: Code<OfferStatus>,
    pub teaches_german: Option<bool>,
    pub teaches_english: Option<bool>,
    pub is_fues: bool,
    pub is_limited: Option<bool>,
    pub department: Option<String>,
    /// Teaching events in the module's newest semester.
    pub teaching_events: i64,
    pub exam_form: Option<Code<ExamForm>>,
    /// The responsible persons as the module page names them.
    pub responsible: Option<String>,
    /// Present when a program is selected.
    pub kind: Option<Code<ModuleKind>>,
    pub plan_semester: Option<i64>,
    pub area: Option<String>,
}

impl FromRow for CatalogRow {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("module_id")?,
            title: row.text("title")?,
            title_de: row.opt_text("title_de")?,
            title_en: row.opt_text("title_en")?,
            credits: row.opt_real("credits")?,
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            turnus_parity: Code::parse_opt(row.opt_text("turnus_parity")?),
            offer_status: Code::parse(&row.text("offer_status")?),
            teaches_german: row.opt_flag("teaches_german")?,
            teaches_english: row.opt_flag("teaches_english")?,
            is_fues: row.flag("is_fues")?,
            is_limited: row.opt_flag("is_limited")?,
            department: row.opt_text("department")?,
            teaching_events: row.int("teaching_events")?,
            exam_form: Code::parse_opt(row.opt_text("exam_form")?),
            responsible: row.opt_text("responsible")?,
            kind: Code::parse_opt(row.opt_text("kind")?),
            plan_semester: row.opt_int("plan_semester")?,
            area: row.opt_text("area")?,
        })
    }
}

/// One page of the catalog with the exact number of matches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogPage {
    pub total: u64,
    pub offset: u64,
    pub rows: Vec<CatalogRow>,
}

/// `v_module`: everything the module page states about the module itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    pub language_raw: Option<String>,
    pub teaches_german: Option<bool>,
    pub teaches_english: Option<bool>,
    pub duration_raw: Option<String>,
    pub duration_semesters: Option<i64>,
    pub turnus_raw: Option<String>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    pub turnus_parity: Option<Code<TurnusParity>>,
    pub offer_status: Code<OfferStatus>,
    pub limitation_raw: Option<String>,
    pub is_limited: Option<bool>,
    pub participant_limit: Option<i64>,
    pub exam_form: Option<Code<ExamForm>>,
    pub exam_form_raw: Option<String>,
    pub exam_details: Option<String>,
    pub grading_raw: Option<String>,
    pub is_graded: Option<bool>,
    pub is_fues: bool,
    pub department: Option<String>,
    pub learning_outcomes: Option<String>,
    pub contents: Option<String>,
    pub prerequisites_recommended: Option<String>,
    pub prerequisites_mandatory: Option<String>,
    pub remarks: Option<String>,
    pub source_url: Option<String>,
    pub fetched_at: Option<String>,
    pub at_zentralcampus: Option<bool>,
    pub at_sachsendorf: Option<bool>,
    pub at_senftenberg: Option<bool>,
}

impl FromRow for Module {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("id")?,
            title: row.text("title")?,
            title_de: row.opt_text("title_de")?,
            title_en: row.opt_text("title_en")?,
            credits: row.opt_real("credits")?,
            language_raw: row.opt_text("language_raw")?,
            teaches_german: row.opt_flag("teaches_german")?,
            teaches_english: row.opt_flag("teaches_english")?,
            duration_raw: row.opt_text("duration_raw")?,
            duration_semesters: row.opt_int("duration_semesters")?,
            turnus_raw: row.opt_text("turnus_raw")?,
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            turnus_parity: Code::parse_opt(row.opt_text("turnus_parity")?),
            offer_status: Code::parse(&row.text("offer_status")?),
            limitation_raw: row.opt_text("limitation_raw")?,
            is_limited: row.opt_flag("is_limited")?,
            participant_limit: row.opt_int("participant_limit")?,
            exam_form: Code::parse_opt(row.opt_text("exam_form")?),
            exam_form_raw: row.opt_text("exam_form_raw")?,
            exam_details: row.opt_text("exam_details")?,
            grading_raw: row.opt_text("grading_raw")?,
            is_graded: row.opt_flag("is_graded")?,
            is_fues: row.flag("is_fues")?,
            department: row.opt_text("department")?,
            learning_outcomes: row.opt_text("learning_outcomes")?,
            contents: row.opt_text("contents")?,
            prerequisites_recommended: row.opt_text("prerequisites_recommended")?,
            prerequisites_mandatory: row.opt_text("prerequisites_mandatory")?,
            remarks: row.opt_text("remarks")?,
            source_url: row.opt_text("source_url")?,
            fetched_at: row.opt_text("fetched_at")?,
            at_zentralcampus: row.opt_flag("at_zentralcampus")?,
            at_sachsendorf: row.opt_flag("at_sachsendorf")?,
            at_senftenberg: row.opt_flag("at_senftenberg")?,
        })
    }
}

/// `v_module_prerequisite`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prerequisite {
    pub module_id: String,
    pub required_module_id: String,
    pub kind: Code<PrerequisiteKind>,
    pub required_title: Option<String>,
    pub required_offer_status: Option<Code<OfferStatus>>,
}

impl FromRow for Prerequisite {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            required_module_id: row.text("required_module_id")?,
            kind: Code::parse(&row.text("kind")?),
            required_title: row.opt_text("required_title")?,
            required_offer_status: Code::parse_opt(row.opt_text("required_offer_status")?),
        })
    }
}

/// `v_module_search`: one searchable term of a module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchTerm {
    pub module_id: String,
    pub term: String,
    /// `id`, `title_de` or `title_en`
    pub kind: String,
}

impl FromRow for SearchTerm {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { module_id: row.text("module_id")?, term: row.text("term")?, kind: row.text("kind")? })
    }
}

/// What the search of a list finds outside its filters (`queries::search_elsewhere`): modules
/// that are offered, and modules that are no longer offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchElsewhere {
    pub offered: u64,
    pub not_offered: u64,
}

impl FromRow for SearchElsewhere {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let count = |column: &str| row.int(column).map(|n| u64::try_from(n).unwrap_or(0));
        Ok(Self { offered: count("offered")?, not_offered: count("not_offered")? })
    }
}
