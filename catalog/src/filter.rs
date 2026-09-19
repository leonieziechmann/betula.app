//! The typed filter state of the module catalog and its translation to SQL.
//!
//! `CatalogQuery` is what a catalog URL encodes and the only thing that triggers a
//! catalog query. It filters on the facet columns of the views, never on free text
//! (the one exception is the search against `v_module_search`).

use serde::{Deserialize, Serialize};

use crate::db::Value;
use crate::labels::{Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};

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
    /// `v_program.id`
    pub program_id: String,
    pub relation: ProgramRelation,
    pub plan_semester: Option<PlanSemesterFilter>,
    /// Any of these; empty means all.
    pub kinds: Vec<KindFilter>,
}

/// Any of the ticked offers matches; nothing ticked means no turnus filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnusFilter {
    pub winter: bool,
    pub summer: bool,
    pub irregular: bool,
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

    fn column(self) -> &'static str {
        match self {
            ExamPart::Written => "f.exam_written",
            ExamPart::Oral => "f.exam_oral",
            ExamPart::Paper => "f.exam_paper",
            ExamPart::Presentation => "f.exam_presentation",
            ExamPart::Project => "f.exam_project",
            ExamPart::Practical => "f.exam_practical",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExamPart::Written => "Klausur",
            ExamPart::Oral => "mündliche Prüfung",
            ExamPart::Paper => "Hausarbeit / Beleg",
            ExamPart::Presentation => "Vortrag",
            ExamPart::Project => "Projektarbeit",
            ExamPart::Practical => "praktische Prüfung",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    German,
    English,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortKey {
    #[default]
    Title,
    Id,
    Credits,
    /// Number of teaching events in the module's newest semester.
    Events,
}

/// The filter state of the catalog. `Default` is "no filter".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogQuery {
    /// Search text: matches module ids and German or English titles.
    pub text: String,
    pub program: Option<ProgramScope>,
    /// Every one of these teaches or is responsible for the module.
    pub lecturers_include: Vec<String>,
    /// None of these teaches or is responsible for the module.
    pub lecturers_exclude: Vec<String>,
    pub department_id: Option<i64>,
    pub turnus: TurnusFilter,
    /// Any of these.
    pub teaching_forms: Vec<TeachingForm>,
    pub duration_semesters: Option<u8>,
    /// `Some(true)`: only modules with a participant limit, `Some(false)`: only without.
    pub limited: Option<bool>,
    /// `Some(true)`: only modules of the general FÜS list, `Some(false)`: none of them.
    pub fues: Option<bool>,
    /// Any of these.
    pub exam_forms: Vec<ExamForm>,
    /// Any of these.
    pub exam_parts: Vec<ExamPart>,
    pub graded: Option<bool>,
    /// Any of these; empty means all.
    pub offer: Vec<OfferStatus>,
    pub credits_min: Option<f64>,
    pub credits_max: Option<f64>,
    /// Any of these. Only modules with room data can match (campus is unknown otherwise).
    pub campuses: Vec<Campus>,
    /// Any of these.
    pub languages: Vec<Language>,
    /// Restrict to these module ids: the „Gemerkt" and „Bestanden" views.
    pub only_ids: Option<Vec<String>>,
    /// Keep modules whose mandatory prerequisites are all in this set of passed modules.
    pub prerequisites_met_by: Option<Vec<String>>,
    pub sort: SortKey,
    pub descending: bool,
}

/// A WHERE clause over `v_module_facets f JOIN v_module m` (and `v_program_module pm`
/// when a program is selected), with its parameters in order.
pub struct Sql {
    pub joins: String,
    pub conditions: Vec<String>,
    pub params: Vec<Value>,
}

impl Sql {
    pub fn where_clause(&self) -> String {
        if self.conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", self.conditions.join(" AND "))
        }
    }
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// Escapes `%`, `_` and the escape character itself for `LIKE … ESCAPE '\'`.
pub fn like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

impl CatalogQuery {
    /// How many filters are active (for „Filter zurücksetzen (3)"). Sorting is not a filter.
    pub fn active_filters(&self) -> usize {
        let t = &self.turnus;
        let program = self.program.as_ref();
        [
            !self.text.trim().is_empty(),
            program.is_some(),
            program.is_some_and(|p| p.plan_semester.is_some()),
            program.is_some_and(|p| !p.kinds.is_empty()),
            self.department_id.is_some(),
            t.winter || t.summer || t.irregular || t.year_parity.is_some(),
            !self.teaching_forms.is_empty(),
            self.duration_semesters.is_some(),
            self.limited.is_some(),
            self.fues.is_some(),
            !self.exam_forms.is_empty() || !self.exam_parts.is_empty(),
            self.graded.is_some(),
            !self.offer.is_empty(),
            self.credits_min.is_some() || self.credits_max.is_some(),
            !self.campuses.is_empty(),
            !self.languages.is_empty(),
            self.prerequisites_met_by.is_some(),
        ]
        .iter()
        .filter(|active| **active)
        .count()
            + self.lecturers_include.len()
            + self.lecturers_exclude.len()
    }

    /// The joins, conditions and parameters that select the matching modules.
    pub fn to_sql(&self) -> Sql {
        let mut joins = String::new();
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        if let Some(scope) = &self.program {
            joins.push_str(
                " JOIN v_program_module pm ON pm.module_id = f.module_id AND pm.program_id = ? AND pm.relation = ?",
            );
            params.push(Value::from(&scope.program_id));
            params.push(Value::from(scope.relation.code()));

            match scope.plan_semester {
                Some(PlanSemesterFilter::Semester(n)) => {
                    conditions.push("pm.plan_semester = ?".to_string());
                    params.push(Value::Integer(i64::from(n)));
                }
                Some(PlanSemesterFilter::Unstated) => conditions.push("pm.plan_semester IS NULL".to_string()),
                None => {}
            }

            let mut kinds: Vec<String> = Vec::new();
            for kind in &scope.kinds {
                match kind {
                    KindFilter::Stated(kind) => {
                        kinds.push("pm.kind = ?".to_string());
                        params.push(Value::from(kind.code()));
                    }
                    KindFilter::Unstated => kinds.push("pm.kind IS NULL".to_string()),
                }
            }
            if !kinds.is_empty() {
                conditions.push(format!("({})", kinds.join(" OR ")));
            }
        }

        let text = self.text.trim();
        if !text.is_empty() {
            conditions.push(
                "EXISTS (SELECT 1 FROM v_module_search s WHERE s.module_id = f.module_id AND s.term LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
            params.push(Value::from(like_pattern(text)));
        }

        for name in &self.lecturers_include {
            conditions.push(
                "EXISTS (SELECT 1 FROM v_module_lecturer l WHERE l.module_id = f.module_id AND l.name = ?)".to_string(),
            );
            params.push(Value::from(name));
        }
        if !self.lecturers_exclude.is_empty() {
            conditions.push(format!(
                "NOT EXISTS (SELECT 1 FROM v_module_lecturer l WHERE l.module_id = f.module_id AND l.name IN ({}))",
                placeholders(self.lecturers_exclude.len())
            ));
            params.extend(self.lecturers_exclude.iter().map(Value::from));
        }

        if let Some(id) = self.department_id {
            conditions.push("f.department_id = ?".to_string());
            params.push(Value::Integer(id));
        }

        let mut turnus: Vec<&str> = Vec::new();
        if self.turnus.winter {
            turnus.push("f.offered_winter = 1");
        }
        if self.turnus.summer {
            turnus.push("f.offered_summer = 1");
        }
        if self.turnus.irregular {
            turnus.push("f.turnus_season = 'irregular'");
        }
        if !turnus.is_empty() {
            conditions.push(format!("({})", turnus.join(" OR ")));
        }
        if let Some(parity) = self.turnus.year_parity {
            conditions.push("(f.turnus_parity IS NULL OR f.turnus_parity = ?)".to_string());
            params.push(Value::from(parity.code()));
        }

        let forms: Vec<&str> = self
            .teaching_forms
            .iter()
            .filter_map(|form| match form {
                TeachingForm::Lecture => Some("f.has_lecture = 1"),
                TeachingForm::Exercise => Some("f.has_exercise = 1"),
                TeachingForm::Seminar => Some("f.has_seminar = 1"),
                TeachingForm::Practical => Some("f.has_practical = 1"),
                TeachingForm::Project => Some("f.has_project = 1"),
                TeachingForm::Excursion => Some("f.has_excursion = 1"),
                // No facet column: not offered as a filter.
                TeachingForm::Tutorial
                | TeachingForm::Consultation
                | TeachingForm::SelfStudy
                | TeachingForm::Paper
                | TeachingForm::Other => None,
            })
            .collect();
        if !forms.is_empty() {
            conditions.push(format!("({})", forms.join(" OR ")));
        }

        if let Some(n) = self.duration_semesters {
            conditions.push("f.duration_semesters = ?".to_string());
            params.push(Value::Integer(i64::from(n)));
        }
        if let Some(limited) = self.limited {
            conditions.push("f.is_limited = ?".to_string());
            params.push(Value::from(limited));
        }
        if let Some(fues) = self.fues {
            conditions.push("f.is_fues = ?".to_string());
            params.push(Value::from(fues));
        }

        let mut exams: Vec<String> = Vec::new();
        if !self.exam_forms.is_empty() {
            exams.push(format!("f.exam_form IN ({})", placeholders(self.exam_forms.len())));
            params.extend(self.exam_forms.iter().map(|form| Value::from(form.code())));
        }
        exams.extend(self.exam_parts.iter().map(|part| format!("{} = 1", part.column())));
        if !exams.is_empty() {
            conditions.push(format!("({})", exams.join(" OR ")));
        }

        if let Some(graded) = self.graded {
            conditions.push("f.is_graded = ?".to_string());
            params.push(Value::from(graded));
        }
        if !self.offer.is_empty() {
            conditions.push(format!("f.offer_status IN ({})", placeholders(self.offer.len())));
            params.extend(self.offer.iter().map(|status| Value::from(status.code())));
        }
        if let Some(min) = self.credits_min {
            conditions.push("f.credits >= ?".to_string());
            params.push(Value::Real(min));
        }
        if let Some(max) = self.credits_max {
            conditions.push("f.credits <= ?".to_string());
            params.push(Value::Real(max));
        }

        let campuses: Vec<&str> = self
            .campuses
            .iter()
            .filter_map(|campus| match campus {
                Campus::Zentralcampus => Some("f.at_zentralcampus = 1"),
                Campus::Sachsendorf => Some("f.at_sachsendorf = 1"),
                Campus::Senftenberg => Some("f.at_senftenberg = 1"),
                Campus::Nord => None,
            })
            .collect();
        if !campuses.is_empty() {
            conditions.push(format!("({})", campuses.join(" OR ")));
        }

        let languages: Vec<&str> = self
            .languages
            .iter()
            .map(|language| match language {
                Language::German => "f.teaches_german = 1",
                Language::English => "f.teaches_english = 1",
            })
            .collect();
        if !languages.is_empty() {
            conditions.push(format!("({})", languages.join(" OR ")));
        }

        if let Some(ids) = &self.only_ids {
            if ids.is_empty() {
                conditions.push("0".to_string());
            } else {
                conditions.push(format!("f.module_id IN ({})", placeholders(ids.len())));
                params.extend(ids.iter().map(Value::from));
            }
        }

        if let Some(passed) = &self.prerequisites_met_by {
            let not_passed = if passed.is_empty() {
                String::new()
            } else {
                format!(" AND p.required_module_id NOT IN ({})", placeholders(passed.len()))
            };
            conditions.push(format!(
                "NOT EXISTS (SELECT 1 FROM v_module_prerequisite p WHERE p.module_id = f.module_id AND p.kind = 'mandatory'{not_passed})"
            ));
            params.extend(passed.iter().map(Value::from));
        }

        Sql { joins, conditions, params }
    }

    /// `ORDER BY` for the list; the module id makes every order total, so paging is stable.
    pub fn order_by(&self) -> String {
        let direction = if self.descending { "DESC" } else { "ASC" };
        match self.sort {
            SortKey::Title => format!(" ORDER BY m.title COLLATE NOCASE {direction}, f.module_id"),
            SortKey::Id => format!(" ORDER BY f.module_id {direction}"),
            SortKey::Credits => format!(" ORDER BY f.credits {direction}, m.title COLLATE NOCASE, f.module_id"),
            SortKey::Events => {
                format!(" ORDER BY f.teaching_events {direction}, m.title COLLATE NOCASE, f.module_id")
            }
        }
    }
}
