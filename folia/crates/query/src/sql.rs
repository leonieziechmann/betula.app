//! The catalog's filter as SQL: what `folia_routes::filter::CatalogQuery` selects, over the facet
//! columns of the views (the one exception is the search, `folia_search::Plan` against
//! `v_module_folded`).

use folia_model::db::Value;
use folia_model::labels::{Campus, Labelled, OfferStatus, TeachingForm};
use folia_routes::filter::{CatalogQuery, ExamPart, FitIds, KindFilter, Language, PlanSemesterFilter, SortKey};
use folia_search::Plan;

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

/// The facet column of an exam part.
fn exam_part_column(part: ExamPart) -> &'static str {
    match part {
        ExamPart::Written => "f.exam_written",
        ExamPart::Oral => "f.exam_oral",
        ExamPart::Paper => "f.exam_paper",
        ExamPart::Presentation => "f.exam_presentation",
        ExamPart::Project => "f.exam_project",
        ExamPart::Practical => "f.exam_practical",
    }
}

/// The facet column of a language.
fn language_column(language: Language) -> &'static str {
    match language {
        Language::German => "f.teaches_german",
        Language::English => "f.teaches_english",
    }
}

/// The catalog's filter as SQL.
pub trait CatalogSql {
    /// The joins, conditions and parameters that select the matching modules.
    fn to_sql(&self) -> Sql;
    /// `ORDER BY` for the list; the module id makes every order total, so paging is stable.
    fn order_by(&self) -> String;
    /// The terms of that order, for a window (`ROW_NUMBER() OVER (ORDER BY …)`) as well.
    fn order_terms(&self) -> String;
}

impl CatalogSql for CatalogQuery {
    /// The joins, conditions and parameters that select the matching modules.
    fn to_sql(&self) -> Sql {
        let mut joins = String::new();
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        // The matches of the search first: its parameters precede those of every other join.
        let plan = self.search_plan();
        if let Some((table, table_params)) = plan.as_ref().and_then(Plan::table) {
            joins.push_str(&format!(" JOIN {table} sr ON sr.module_id = f.module_id"));
            params.extend(table_params);
        }
        if let Some(plan) = &plan {
            conditions.push(plan.condition("sr"));
        }

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
            if !scope.areas.is_empty() {
                // An area itself, or one below it: the path of a placement starts with the chosen
                // area's path and a separator. A parent area without modules of its own has no row
                // in the view, so the path (not a parent id) is what the tree is walked by.
                let one = "(a.area_id = ? OR SUBSTR(a.area, 1, LENGTH((SELECT MIN(c.area) FROM v_program_module_area c WHERE c.area_id = ?)) + 3) \
                     = (SELECT MIN(c.area) FROM v_program_module_area c WHERE c.area_id = ?) || ' / ')";
                conditions.push(format!(
                    "EXISTS (SELECT 1 FROM v_program_module_area a WHERE a.program_id = pm.program_id AND a.module_id = f.module_id AND ({}))",
                    vec![one; scope.areas.len()].join(" OR ")
                ));
                for area in &scope.areas {
                    params.extend([Value::Integer(*area), Value::Integer(*area), Value::Integer(*area)]);
                }
            }
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
        exams.extend(self.exam_parts.iter().map(|part| format!("{} = 1", exam_part_column(*part))));
        if !exams.is_empty() {
            conditions.push(format!("({})", exams.join(" OR ")));
        }
        conditions.extend(self.exam_parts_exclude.iter().map(|part| unless(exam_part_column(*part))));

        if let Some(graded) = self.graded {
            conditions.push("f.is_graded = ?".to_string());
            params.push(Value::from(graded));
        }
        if let Some(scheduled) = self.scheduled {
            conditions.push(if scheduled { "f.teaching_events > 0" } else { "f.teaching_events = 0" }.to_string());
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

        let languages: Vec<String> = self.languages.iter().map(|language| format!("{} = 1", language_column(*language))).collect();
        if !languages.is_empty() {
            conditions.push(format!("({})", languages.join(" OR ")));
        }
        conditions.extend(self.languages_exclude.iter().map(|language| unless(language_column(*language))));

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
        if self.fits.is_some() {
            match &self.fits_ids {
                // The plan is unknown here: the server's page.
                None => conditions.push("0".to_string()),
                // Nothing fits.
                Some(FitIds::Only(ids)) if ids.is_empty() => conditions.push("0".to_string()),
                Some(FitIds::Only(ids)) => {
                    conditions.push(format!("f.module_id IN ({})", placeholders(ids.len())));
                    params.extend(ids.iter().map(Value::from));
                }
                Some(FitIds::Without(ids)) if ids.is_empty() => {}
                Some(FitIds::Without(ids)) => {
                    conditions.push(format!("f.module_id NOT IN ({})", placeholders(ids.len())));
                    params.extend(ids.iter().map(Value::from));
                }
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
    fn order_by(&self) -> String {
        format!(" ORDER BY {}", self.order_terms())
    }

    /// The terms of that order, for a window (`ROW_NUMBER() OVER (ORDER BY …)`) as well. While
    /// searching, the best matches first: the most words found, then how well, then by title.
    fn order_terms(&self) -> String {
        if self.by_relevance() {
            return "sr.matched DESC, sr.score DESC, m.title COLLATE NOCASE, f.module_id".to_string();
        }
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

#[cfg(test)]
mod tests {
    use folia_calendar::select::FitOptions;
    use folia_routes::filter::FitsFilter;

    use super::*;

    fn fitting(ids: Option<FitIds>) -> CatalogQuery {
        CatalogQuery { fits: Some(FitsFilter::all("2026W")), fits_ids: ids, ..Default::default() }
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    /// The switch adds one condition to what the query asks without it, or none.
    fn added(query: &CatalogQuery) -> (Vec<String>, Vec<Value>) {
        let plain = CatalogQuery::default().to_sql();
        let sql = query.to_sql();
        assert_eq!(sql.conditions.get(..plain.conditions.len()), Some(plain.conditions.as_slice()));
        assert_eq!(sql.params.get(..plain.params.len()), Some(plain.params.as_slice()));
        (sql.conditions[plain.conditions.len()..].to_vec(), sql.params[plain.params.len()..].to_vec())
    }

    #[test]
    fn the_fit_filter_is_a_switch() {
        let two = ids(&["12330", "11103"]);
        let texts = two.iter().map(Value::from).collect::<Vec<_>>();
        // Off: nothing is asked, whatever ids are around.
        assert_eq!(added(&CatalogQuery { fits_ids: Some(FitIds::Only(Vec::new())), ..Default::default() }), (vec![], vec![]));
        // On where nobody filled in the plan (the server's page): nothing is listed.
        assert_eq!(added(&fitting(None)), (vec!["0".to_string()], vec![]));
        assert_eq!(added(&fitting(Some(FitIds::Only(two.clone())))), (vec!["f.module_id IN (?, ?)".to_string()], texts.clone()));
        assert_eq!(added(&fitting(Some(FitIds::Only(Vec::new())))), (vec!["0".to_string()], vec![]));
        assert_eq!(added(&fitting(Some(FitIds::Without(two)))), (vec!["f.module_id NOT IN (?, ?)".to_string()], texts));
        assert_eq!(added(&fitting(Some(FitIds::Without(Vec::new())))), (vec![], vec![]));
        // One filter, however it is set.
        assert_eq!(fitting(None).active_filters(), 1);
        assert_eq!(CatalogQuery { fits: Some(FitsFilter { undated: true, exams: false, ..FitsFilter::all("2026W") }), ..Default::default() }.active_filters(), 1);
    }

    #[test]
    fn the_fit_filter_asks_the_snapshot() {
        let db = folia_test_support::open();
        let count = |query: &CatalogQuery| crate::catalog_count(&db, query).unwrap();
        let everything = || CatalogQuery { offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() };
        let all = count(&everything());
        let two = ids(&["12104", "12107"]);
        let with = |fits_ids: Option<FitIds>| CatalogQuery { fits: Some(FitsFilter::all("2026W")), fits_ids, ..everything() };
        assert_eq!(count(&with(None)), 0);
        assert_eq!(count(&with(Some(FitIds::Only(two.clone())))), 2);
        assert_eq!(count(&with(Some(FitIds::Without(two)))), all - 2);
        assert_eq!(count(&with(Some(FitIds::Without(Vec::new())))), all);
    }

    #[test]
    fn a_fit_filter_names_what_it_leaves_out() {
        let all = FitsFilter::all(" 2026w ");
        assert_eq!(all, FitsFilter { semester: "2026W".into(), lectures: true, exercises: true, exams: true, undated: false });
        assert_eq!(all.options(), FitOptions { lectures: true, exercises: true, exams: true });
        assert!(all.skipped().is_empty());
        let fewer = FitsFilter { lectures: false, exams: false, ..all };
        assert_eq!(fewer.skipped(), vec!["lecture", "exam"]);
        assert_eq!(fewer.options(), FitOptions { lectures: false, exercises: true, exams: false });
    }
}
