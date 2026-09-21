//! Row structs of the module page and the program page (the satellites of `v_module` and `v_program`).

use serde::{Deserialize, Serialize};

use crate::db::{DbError, FromRow, Row};
use crate::labels::{
    Campus, Code, DegreeLevel, DocumentType, KindBasis, KindSource, LecturerRole, ModuleKind, Relation,
    ResolveStatus, Rhythm, TeachingForm, TextItemKind,
};

/// `v_module_lecturer`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lecturer {
    pub name: String,
    pub title: Option<String>,
    pub role: Code<LecturerRole>,
}

impl FromRow for Lecturer {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { name: row.text("name")?, title: row.opt_text("title")?, role: Code::parse(&row.text("role")?) })
    }
}

/// A lecturer of the filter list with the number of modules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LecturerName {
    pub name: String,
    /// The academic title as a module page states it; `None` for persons only known from events.
    pub title: Option<String>,
    pub modules: i64,
}

impl FromRow for LecturerName {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { name: row.text("name")?, title: row.opt_text("title")?, modules: row.int("modules")? })
    }
}

/// How much of a program's curriculum one department offers (for the derived faculty).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramDepartmentCount {
    pub program_id: String,
    pub department_id: i64,
    /// Thesis modules of the curriculum that belong to this department.
    pub thesis_modules: i64,
    /// Curriculum modules of this department that are still offered.
    pub offered_modules: i64,
}

impl FromRow for ProgramDepartmentCount {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            program_id: row.text("program_id")?,
            department_id: row.int("department_id")?,
            thesis_modules: row.int("thesis_modules")?,
            offered_modules: row.int("offered_modules")?,
        })
    }
}

/// `v_module_teaching_form`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleTeachingForm {
    pub form: Code<TeachingForm>,
    pub form_raw: String,
    pub workload_raw: Option<String>,
    pub sws: Option<f64>,
    pub hours: Option<f64>,
}

impl FromRow for ModuleTeachingForm {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            form: Code::parse(&row.text("form")?),
            form_raw: row.text("form_raw")?,
            workload_raw: row.opt_text("workload_raw")?,
            sws: row.opt_real("sws")?,
            hours: row.opt_real("hours")?,
        })
    }
}

/// `v_module_text_item`: one entry of the literature or course list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextItem {
    pub kind: Code<TextItemKind>,
    pub text: String,
}

impl FromRow for TextItem {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { kind: Code::parse(&row.text("kind")?), text: row.text("text")? })
    }
}

/// `v_module_successor`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Successor {
    pub successor_id: String,
    pub successor_title: Option<String>,
}

impl FromRow for Successor {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { successor_id: row.text("successor_id")?, successor_title: row.opt_text("successor_title")? })
    }
}

/// `v_module_schedule` and `v_module_exam`: one date of an event. Exams have no group or rhythm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDate {
    pub semester_key: String,
    pub semester_label: String,
    pub event_id: String,
    pub event_number: Option<String>,
    pub event_title: String,
    pub event_type: Option<String>,
    pub group_name: Option<String>,
    /// 1 = Monday
    pub weekday: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub rhythm: Option<Code<Rhythm>>,
    pub rhythm_raw: Option<String>,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub room: Option<String>,
    pub campus: Option<Code<Campus>>,
    pub instructor: Option<String>,
    pub comment: Option<String>,
    pub source_url: Option<String>,
}

impl FromRow for EventDate {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            semester_key: row.text("semester_key")?,
            semester_label: row.text("semester_label")?,
            event_id: row.text("event_id")?,
            event_number: row.opt_text("event_number")?,
            event_title: row.text("event_title")?,
            event_type: row.opt_text("event_type")?,
            group_name: row.opt_text("group_name")?,
            weekday: row.opt_int("weekday")?,
            start_time: row.opt_text("start_time")?,
            end_time: row.opt_text("end_time")?,
            rhythm: Code::parse_opt(row.opt_text("rhythm")?),
            rhythm_raw: row.opt_text("rhythm_raw")?,
            first_date: row.opt_text("first_date")?,
            last_date: row.opt_text("last_date")?,
            room: row.opt_text("room")?,
            campus: Code::parse_opt(row.opt_text("campus")?),
            instructor: row.opt_text("instructor")?,
            comment: row.opt_text("comment")?,
            source_url: row.opt_text("source_url")?,
        })
    }
}

/// `v_module_program_link`: a program a module page names, resolved to the catalog or not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramLink {
    pub degree_raw: Option<String>,
    pub program_raw: Option<String>,
    pub po_raw: Option<String>,
    pub resolve_status: Code<ResolveStatus>,
    pub program_slug: Option<String>,
    pub program_name: Option<String>,
    pub degree_display: Option<String>,
    pub po_version: Option<String>,
    pub is_latest_po: Option<bool>,
    pub relation: Option<Code<Relation>>,
    pub kind: Option<Code<ModuleKind>>,
    pub kind_source: Option<Code<KindSource>>,
    pub area: Option<String>,
}

impl FromRow for ProgramLink {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            degree_raw: row.opt_text("degree_raw")?,
            program_raw: row.opt_text("program_raw")?,
            po_raw: row.opt_text("po_raw")?,
            resolve_status: Code::parse(&row.text("resolve_status")?),
            program_slug: row.opt_text("program_slug")?,
            program_name: row.opt_text("program_name")?,
            degree_display: row.opt_text("degree_display")?,
            po_version: row.opt_text("po_version")?,
            is_latest_po: row.opt_flag("is_latest_po")?,
            relation: Code::parse_opt(row.opt_text("relation")?),
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_source: Code::parse_opt(row.opt_text("kind_source")?),
            area: row.opt_text("area")?,
        })
    }
}

/// `v_program_version`: another PO version of the same program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramVersion {
    pub slug: String,
    pub po_version: String,
    pub is_latest_po: bool,
}

impl FromRow for ProgramVersion {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            slug: row.text("other_slug")?,
            po_version: row.text("po_version")?,
            is_latest_po: row.flag("is_latest_po")?,
        })
    }
}

/// `v_program_counterpart`: the Master of a Bachelor, or the other way round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Counterpart {
    pub slug: String,
    pub name: String,
    pub level: Code<DegreeLevel>,
    pub po_version: String,
}

impl FromRow for Counterpart {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            slug: row.text("counterpart_slug")?,
            name: row.text("counterpart_name")?,
            level: Code::parse(&row.text("counterpart_level")?),
            po_version: row.text("counterpart_po_version")?,
        })
    }
}

/// `v_program_document`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub doc_type: Code<DocumentType>,
    pub url: String,
}

impl FromRow for Document {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { title: row.text("title")?, doc_type: Code::parse(&row.text("doc_type")?), url: row.text("url")? })
    }
}

/// `v_program_module_area`: where the module tree of the program places a module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaPlacement {
    pub module_id: String,
    pub module_title: String,
    pub module_credits: Option<f64>,
    pub area_id: i64,
    /// The full path, e.g. „Gesamtkonto Bachelor / Entwerfen".
    pub area: String,
    pub area_label: String,
    pub depth: i64,
    pub area_ord: i64,
    /// What the tree's own labels say about the kind (`v_program_module_area.kind`).
    pub kind: Option<Code<ModuleKind>>,
    pub kind_basis: Option<Code<KindBasis>>,
    /// What the program's sources settle on for the module (`v_program_module.kind`: the plan,
    /// the module page, the tree's label, the strongest statement first); `None` where none says.
    pub module_kind: Option<Code<ModuleKind>>,
}

impl FromRow for AreaPlacement {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            module_title: row.text("module_title")?,
            module_credits: row.opt_real("module_credits")?,
            area_id: row.int("area_id")?,
            area: row.text("area")?,
            area_label: row.text("area_label")?,
            depth: row.int("depth")?,
            area_ord: row.int("area_ord")?,
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_basis: Code::parse_opt(row.opt_text("kind_basis")?),
            module_kind: Code::parse_opt(row.opt_text("module_kind")?),
        })
    }
}

/// `v_program_plan`: the validated study plan of a program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub source_file: String,
    /// The table layout of the plan as extracted from the PDF (input of the plan grid).
    pub layout_json: String,
    pub validated_at: Option<String>,
}

impl FromRow for Plan {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            source_file: row.text("source_file")?,
            layout_json: row.text("layout_json")?,
            validated_at: row.opt_text("validated_at")?,
        })
    }
}

/// `v_program_plan_entry`: one row of the validated study plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanEntry {
    /// Set when the plan row is linked to a module of the catalog.
    pub module_id: Option<String>,
    pub module_name: String,
    pub semester: Option<i64>,
    pub start_semester: Option<i64>,
    pub end_semester: Option<i64>,
    pub semester_span: Option<String>,
    pub credits: Option<f64>,
    pub min_credits: Option<f64>,
    pub max_credits: Option<f64>,
    pub kind: Option<Code<ModuleKind>>,
    pub kind_raw: Option<String>,
    pub study_section: Option<String>,
    pub subject_area: Option<String>,
    pub specialization: Option<String>,
    pub catalog_title: Option<String>,
    pub credits_differ_from_catalog: bool,
}

impl FromRow for PlanEntry {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.opt_text("module_id")?,
            module_name: row.text("module_name")?,
            semester: row.opt_int("semester")?,
            start_semester: row.opt_int("start_semester")?,
            end_semester: row.opt_int("end_semester")?,
            semester_span: row.opt_text("semester_span")?,
            credits: row.opt_real("credits")?,
            min_credits: row.opt_real("min_credits")?,
            max_credits: row.opt_real("max_credits")?,
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_raw: row.opt_text("kind_raw")?,
            study_section: row.opt_text("study_section")?,
            subject_area: row.opt_text("subject_area")?,
            specialization: row.opt_text("specialization")?,
            catalog_title: row.opt_text("catalog_title")?,
            credits_differ_from_catalog: row.opt_flag("credits_differ_from_catalog")?.unwrap_or(false),
        })
    }
}
