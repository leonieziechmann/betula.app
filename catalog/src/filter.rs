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
    /// `v_program.slug`: what URLs carry, so no lookup is needed to build the query.
    pub program_slug: String,
    pub relation: ProgramRelation,
    pub plan_semester: Option<PlanSemesterFilter>,
    /// Any of these; empty means all.
    pub kinds: Vec<KindFilter>,
    /// None of these.
    pub kinds_exclude: Vec<KindFilter>,
    /// An area of the program's module tree (`v_program_module_area.area_id`): only the modules
    /// the tree places in it or in an area below it. „Wahlpflichtmodule Praktische Informatik"
    /// is such an area; the tree, not the plan, is the authority for structure.
    pub area: Option<i64>,
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

    /// What the chips of the filter say.
    pub fn short_label(self) -> &'static str {
        match self {
            ExamPart::Written => "Klausur",
            ExamPart::Oral => "Mündlich",
            ExamPart::Paper => "Hausarbeit",
            ExamPart::Presentation => "Vortrag",
            ExamPart::Project => "Projekt",
            ExamPart::Practical => "Praktisch",
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

    pub fn label(self) -> &'static str {
        match self {
            Language::German => "Deutsch",
            Language::English => "Englisch",
        }
    }

    fn column(self) -> &'static str {
        match self {
            Language::German => "f.teaches_german",
            Language::English => "f.teaches_english",
        }
    }
}

/// The facet column of a teaching form; `None` for forms the catalog cannot filter by.
fn teaching_form_column(form: TeachingForm) -> Option<&'static str> {
    match form {
        TeachingForm::Lecture => Some("f.has_lecture"),
        TeachingForm::Exercise => Some("f.has_exercise"),
        TeachingForm::Seminar => Some("f.has_seminar"),
        TeachingForm::Practical => Some("f.has_practical"),
        TeachingForm::Project => Some("f.has_project"),
        TeachingForm::Excursion => Some("f.has_excursion"),
        TeachingForm::Tutorial | TeachingForm::Consultation | TeachingForm::SelfStudy | TeachingForm::Paper | TeachingForm::Other => None,
    }
}

/// The facet column of a campus; only campuses with room data have one.
fn campus_column(campus: Campus) -> Option<&'static str> {
    match campus {
        Campus::Zentralcampus => Some("f.at_zentralcampus"),
        Campus::Sachsendorf => Some("f.at_sachsendorf"),
        Campus::Senftenberg => Some("f.at_senftenberg"),
        Campus::Nord => None,
    }
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
    /// Search text: matches module ids and German or English titles.
    pub text: String,
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

/// „Not known to be so": the flag is 0 or the source does not say.
fn unless(column: &str) -> String {
    format!("IFNULL({column}, 0) = 0")
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
            program.is_some_and(|p| p.area.is_some()),
            self.department_id.is_some(),
            t.winter || t.summer || t.irregular || t.not_winter || t.not_summer || t.not_irregular || t.year_parity.is_some(),
            !self.teaching_forms.is_empty() || !self.teaching_forms_exclude.is_empty(),
            self.duration_semesters.is_some(),
            self.limited.is_some(),
            self.fues.is_some(),
            !self.exam_forms.is_empty() || !self.exam_parts.is_empty() || !self.exam_parts_exclude.is_empty(),
            self.graded.is_some(),
            self.offer.is_some(),
            self.credits_min.is_some() || self.credits_max.is_some(),
            !self.campuses.is_empty() || !self.campuses_exclude.is_empty(),
            !self.languages.is_empty() || !self.languages_exclude.is_empty(),
            self.prerequisites_met_by.is_some(),
            self.marked.is_some(),
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
                " JOIN v_program_module pm ON pm.module_id = f.module_id \
                 AND pm.program_id = (SELECT id FROM v_program WHERE slug = ?) AND pm.relation = ?",
            );
            params.push(Value::from(&scope.program_slug));
            params.push(Value::from(scope.relation.code()));

            match scope.plan_semester {
                Some(PlanSemesterFilter::Semester(n)) => {
                    // What the plan places in the semester, and what can be chosen for the
                    // semester's requirements: the modules of the areas those point at, or every
                    // elective the plan places nowhere. What the plan places elsewhere is no choice.
                    let mut asked = vec!["pm.plan_semester = ?".to_string()];
                    params.push(Value::Integer(i64::from(n)));
                    if !scope.semester_areas.is_empty() {
                        asked.push(format!(
                            "(pm.plan_semester IS NULL AND EXISTS (SELECT 1 FROM v_program_module_area a \
                             WHERE a.program_id = pm.program_id AND a.module_id = f.module_id AND a.area_id IN ({})))",
                            placeholders(scope.semester_areas.len())
                        ));
                        params.extend(scope.semester_areas.iter().map(|id| Value::Integer(*id)));
                    }
                    if scope.semester_electives {
                        asked.push("(pm.plan_semester IS NULL AND IFNULL(pm.kind, '') NOT IN ('compulsory', 'thesis', 'internship'))".to_string());
                    }
                    conditions.push(if asked.len() == 1 { asked.remove(0) } else { format!("({})", asked.join(" OR ")) });
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
            for kind in &scope.kinds_exclude {
                match kind {
                    KindFilter::Stated(kind) => {
                        conditions.push("(pm.kind IS NULL OR pm.kind != ?)".to_string());
                        params.push(Value::from(kind.code()));
                    }
                    KindFilter::Unstated => conditions.push("pm.kind IS NOT NULL".to_string()),
                }
            }
            if let Some(area) = scope.area {
                // The area itself, or one below it: the path of a placement starts with the
                // chosen area's path and a separator. A parent area without modules of its own has
                // no row in the view, so the path (not a parent id) is what the tree is walked by.
                conditions.push(
                    "EXISTS (SELECT 1 FROM v_program_module_area a WHERE a.program_id = pm.program_id AND a.module_id = f.module_id \
                     AND (a.area_id = ? OR SUBSTR(a.area, 1, LENGTH((SELECT MIN(c.area) FROM v_program_module_area c WHERE c.area_id = ?)) + 3) \
                     = (SELECT MIN(c.area) FROM v_program_module_area c WHERE c.area_id = ?) || ' / '))"
                        .to_string(),
                );
                params.extend([Value::Integer(area), Value::Integer(area), Value::Integer(area)]);
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

        if !self.lecturers_include.is_empty() {
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM v_module_lecturer l WHERE l.module_id = f.module_id AND l.name IN ({}))",
                placeholders(self.lecturers_include.len())
            ));
            params.extend(self.lecturers_include.iter().map(Value::from));
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
        if self.turnus.not_winter {
            conditions.push(unless("f.offered_winter"));
        }
        if self.turnus.not_summer {
            conditions.push(unless("f.offered_summer"));
        }
        if self.turnus.not_irregular {
            conditions.push("IFNULL(f.turnus_season, '') != 'irregular'".to_string());
        }
        if let Some(parity) = self.turnus.year_parity {
            conditions.push("(f.turnus_parity IS NULL OR f.turnus_parity = ?)".to_string());
            params.push(Value::from(parity.code()));
        }

        let forms: Vec<String> =
            self.teaching_forms.iter().filter_map(|form| teaching_form_column(*form)).map(|column| format!("{column} = 1")).collect();
        if !forms.is_empty() {
            conditions.push(format!("({})", forms.join(" OR ")));
        }
        conditions.extend(self.teaching_forms_exclude.iter().filter_map(|form| teaching_form_column(*form)).map(unless));

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
        conditions.extend(self.exam_parts_exclude.iter().map(|part| unless(part.column())));

        if let Some(graded) = self.graded {
            conditions.push("f.is_graded = ?".to_string());
            params.push(Value::from(graded));
        }
        let offer = self.effective_offer();
        if !offer.is_empty() && offer.len() < OfferStatus::ALL.len() {
            conditions.push(format!("f.offer_status IN ({})", placeholders(offer.len())));
            params.extend(offer.iter().map(|status| Value::from(status.code())));
        }
        if let Some(min) = self.credits_min {
            conditions.push("f.credits >= ?".to_string());
            params.push(Value::Real(min));
        }
        if let Some(max) = self.credits_max {
            conditions.push("f.credits <= ?".to_string());
            params.push(Value::Real(max));
        }

        let campuses: Vec<String> =
            self.campuses.iter().filter_map(|campus| campus_column(*campus)).map(|column| format!("{column} = 1")).collect();
        if !campuses.is_empty() {
            conditions.push(format!("({})", campuses.join(" OR ")));
        }
        conditions.extend(self.campuses_exclude.iter().filter_map(|campus| campus_column(*campus)).map(unless));

        let languages: Vec<String> = self.languages.iter().map(|language| format!("{} = 1", language.column())).collect();
        if !languages.is_empty() {
            conditions.push(format!("({})", languages.join(" OR ")));
        }
        conditions.extend(self.languages_exclude.iter().map(|language| unless(language.column())));

        if let Some(ids) = &self.only_ids {
            if ids.is_empty() {
                conditions.push("0".to_string());
            } else {
                conditions.push(format!("f.module_id IN ({})", placeholders(ids.len())));
                params.extend(ids.iter().map(Value::from));
            }
        }
        if !self.without_ids.is_empty() {
            conditions.push(format!("f.module_id NOT IN ({})", placeholders(self.without_ids.len())));
            params.extend(self.without_ids.iter().map(Value::from));
        }
        if self.marked == Some(true) && self.only_ids.is_none() {
            conditions.push("0".to_string());
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
        format!(" ORDER BY {}", self.order_terms())
    }

    /// The terms of that order, for a window (`ROW_NUMBER() OVER (ORDER BY …)`) as well.
    pub fn order_terms(&self) -> String {
        let direction = if self.descending { "DESC" } else { "ASC" };
        match self.sort {
            SortKey::Default if self.program.is_some() => {
                format!("pm.plan_semester IS NULL, pm.plan_semester {direction}, m.title COLLATE NOCASE, f.module_id")
            }
            SortKey::Default | SortKey::Title => format!("m.title COLLATE NOCASE {direction}, f.module_id"),
            SortKey::Id => format!("f.module_id {direction}"),
            SortKey::Credits => format!("f.credits {direction}, m.title COLLATE NOCASE, f.module_id"),
            SortKey::Events => format!("f.teaching_events {direction}, m.title COLLATE NOCASE, f.module_id"),
        }
    }
}
