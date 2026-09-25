//! Row structs of the module page, the program page and the Studienplan (the satellites of
//! `v_module` and `v_program`).

use serde::{Deserialize, Serialize};

use crate::db::{DbError, FromRow, Row};
use crate::labels::{
    Campus, Code, DegreeLevel, DocumentType, KindBasis, KindSource, LecturerRole, ModuleKind, PlanTotalScope,
    Relation, ResolveStatus, Rhythm, TeachingForm, TextItemKind,
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

/// One row of `v_module_schedule` or `v_module_exam` with what `EventDate` leaves out: the module
/// it was asked for, its place in the event and QIS's cancellations. The Studienplan keys, dates
/// and hides rows by them (`timetable`); `EventDate` stays what the module page reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DateRow {
    /// The module the row was asked for: an event of two planned modules comes once for each.
    pub module_id: String,
    /// The row's place among the dates of its event; `None` for an event that has no dates.
    pub ord: Option<i64>,
    /// QIS's „fällt aus am" column as it stands, read by `timetable::cancel::parse`. Exam rows do
    /// not have the column, so theirs is always `None`.
    pub cancelled_dates: Option<String>,
    pub date: EventDate,
}

impl FromRow for DateRow {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            ord: row.opt_int("ord")?,
            cancelled_dates: row.opt_text("cancelled_dates")?,
            date: EventDate::from_row(row)?,
        })
    }
}

/// How many dated rows of a semester share a rhythm and a date range: what the lecture period,
/// its breaks and the A/B weeks are read from (`timetable::facts`), since no source states them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DateCount {
    pub rhythm: Option<Code<Rhythm>>,
    pub first_date: String,
    pub last_date: Option<String>,
    /// Distinct rows (event and `ord`), however many modules link the event.
    pub dates: i64,
}

impl FromRow for DateCount {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            rhythm: Code::parse_opt(row.opt_text("rhythm")?),
            first_date: row.text("first_date")?,
            last_date: row.opt_text("last_date")?,
            dates: row.int("dates")?,
        })
    }
}

/// The SWS a module page states for one teaching form, its entries summed: whether the parallel
/// slots of an event are alternatives or all required (`timetable::model`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleSws {
    pub module_id: String,
    pub form: Code<TeachingForm>,
    pub sws: f64,
}

impl FromRow for ModuleSws {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { module_id: row.text("module_id")?, form: Code::parse(&row.text("form")?), sws: row.real("sws")? })
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

/// `program_area`: one node of the program's module tree, with or without modules of its own.
/// The areas the tree places modules in (`AreaPlacement`) hang below nodes that hold none
/// („Grundstudium", „Komplex Nebenfach"); the pickers need those to know what an area lies in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaNode {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub depth: i64,
    pub label: String,
    /// What a label on the path states about the kind (`program_area.stated_kind`).
    pub stated_kind: Option<Code<ModuleKind>>,
}

impl FromRow for AreaNode {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.int("id")?,
            parent_id: row.opt_int("parent_id")?,
            depth: row.int("depth")?,
            label: row.text("label")?,
            stated_kind: Code::parse_opt(row.opt_text("stated_kind")?),
        })
    }
}

/// `v_program_plan`: the validated study plan of a program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub source_file: String,
    /// The pages of the regulation the plan stands on, as a reader would write them („9", „9–11").
    /// A regulation is dozens of pages of legal text with the plan somewhere in an appendix, so the
    /// file alone does not let anyone check what is shown here.
    pub source_pages: Option<String>,
    /// The heading the plan stands under („Anlage 2.1 Regelstudienplan … – grundlagenorientiert").
    /// A Lesefassung may print one plan per study branch; this says which of them was read.
    pub source_label: Option<String>,
    /// The table layout of the plan as extracted from the PDF (input of the plan grid).
    pub layout_json: String,
    pub validated_at: Option<String>,
}

impl FromRow for Plan {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            source_file: row.text("source_file")?,
            source_pages: row.opt_text("source_pages")?,
            source_label: row.opt_text("source_label")?,
            layout_json: row.text("layout_json")?,
            validated_at: row.opt_text("validated_at")?,
        })
    }
}

/// `v_program_plan_entry`: one row of the validated study plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanEntry {
    /// The row's place in the plan, counted from 1. A printed sum names its rows by it.
    pub ord: i64,
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
    /// The page of the regulation this row stands on. A plan continued across a page break has
    /// rows on both, so it is kept per row and not only for the plan.
    pub source_page: Option<i64>,
}

impl FromRow for PlanEntry {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            ord: row.int("ord")?,
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
            source_page: row.opt_int("source_page")?,
        })
    }
}

/// `v_program_plan_total`: a sum the regulation prints over rows of its own plan.
///
/// A plan with elective budgets cannot be added up from its rows: „Komplex Praktische Informatik,
/// 10–24 LP" three times is anything between 30 and 72 LP. The regulation prints what they come
/// to — „Summe Komplexe des Fachstudiums 44" over exactly those three rows, „Summe Studium" over
/// the whole table — and `scope` says which of the two a sum is: the plan of these semesters
/// (`plan`), or a part of it (`section`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanTotal {
    pub ord: i64,
    /// As the regulation prints it: „Summe Studium", „Summe Komplex Mathematik".
    pub label: String,
    /// `plan`: everything these semesters hold. `section`: a named part of them.
    pub scope: Code<PlanTotalScope>,
    /// The plan variant this sum belongs to, where the document prints several.
    pub specialization: Option<String>,
    pub start_semester: i64,
    /// The same as `start_semester` unless the sum stands over a merged column.
    pub end_semester: i64,
    /// What the regulation prints. Some print a span instead of a number wherever a semester
    /// holds an elective budget („28 – 32 LP"); `credits_max` is then the upper end of it, and
    /// equal to `credits` otherwise.
    pub credits: f64,
    pub credits_max: f64,
    /// What its rows come to: `min_credits` from the rows that lie entirely inside these
    /// semesters, `max_credits` from those and whatever a row reaching into them could add.
    /// `credits` always lies between the two.
    pub min_credits: f64,
    pub max_credits: f64,
    /// Whether this sum is the only statement of how much its rows count for: every row it names
    /// lies inside it, and at least one of them prints a range („10–24 LP") instead of a number.
    pub is_choice: bool,
    pub entry_count: i64,
    /// `ord` of every plan row this sum counts, in plan order.
    pub entries: Vec<i64>,
}

impl PlanTotal {
    /// Whether the regulation prints a span here instead of a number.
    pub fn is_span(&self) -> bool {
        self.credits_max - self.credits > 0.01
    }

    /// Whether this sum counts everything its semesters hold.
    pub fn is_whole_plan(&self) -> bool {
        self.scope.is(PlanTotalScope::Plan)
    }
}

impl FromRow for PlanTotal {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            ord: row.int("ord")?,
            label: row.text("label")?,
            scope: Code::parse(&row.text("scope")?),
            specialization: row.opt_text("specialization")?,
            start_semester: row.int("start_semester")?,
            end_semester: row.int("end_semester")?,
            credits: row.real("credits")?,
            credits_max: row.real("credits_max")?,
            min_credits: row.real("min_credits")?,
            max_credits: row.real("max_credits")?,
            is_choice: row.opt_flag("is_choice")?.unwrap_or(false),
            entry_count: row.int("entry_count")?,
            entries: Vec::new(),
        })
    }
}

/// `v_program_plan_total_entry`: which row of the plan a sum counts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanTotalEntry {
    pub total_ord: i64,
    pub entry_ord: i64,
}

impl FromRow for PlanTotalEntry {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self { total_ord: row.int("total_ord")?, entry_ord: row.int("entry_ord")? })
    }
}
