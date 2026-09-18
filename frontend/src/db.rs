use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::collections::HashSet;
use crate::models::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = initBtuDatabase, catch)]
    async fn js_init_btu_database(
        db_url: &str,
        status_url: &str,
        on_progress: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = dbQuery, catch)]
    fn js_db_query(sql: &str, params_json: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = dbQueryValue, catch)]
    fn js_db_query_value(sql: &str, params_json: &str) -> Result<JsValue, JsValue>;
}

pub async fn init_database<F>(on_progress: F) -> Result<String, String>
where
    F: Fn(i32, String) + 'static,
{
    let closure = Closure::wrap(Box::new(move |pct: i32, msg: String| {
        on_progress(pct, msg);
    }) as Box<dyn Fn(i32, String)>);

    let progress_fn = closure.as_ref().unchecked_ref();
    let res = js_init_btu_database("/api/db", "/api/status", progress_fn)
        .await
        .map_err(|e| format!("{:?}", e))?;

    closure.forget();
    res.as_string().ok_or_else(|| "Unbekannter Fehler beim Laden".to_string())
}

pub fn execute_query<T: serde::de::DeserializeOwned>(sql: &str, params: &[serde_json::Value]) -> Result<Vec<T>, String> {
    let params_json = serde_json::to_string(params).map_err(|e| e.to_string())?;
    let js_res = js_db_query(sql, &params_json).map_err(|e| format!("{:?}", e))?;
    let json_str = js_res.as_string().unwrap_or_else(|| "[]".to_string());
    serde_json::from_str(&json_str).map_err(|e| format!("Fehler beim Parsen von: {}", e))
}

pub fn get_total_count() -> i64 {
    if let Ok(val) = js_db_query_value("SELECT COUNT(*) FROM modules", "[]") {
        if let Some(s) = val.as_string() {
            return s.parse::<i64>().unwrap_or(0);
        }
    }
    0
}

pub fn get_all_study_programs() -> Vec<ProgramOption> {
    let sql = "SELECT id, program_name, degree, po_version FROM official_study_programs ORDER BY program_name ASC, po_version DESC";
    execute_query(sql, &[]).unwrap_or_default()
}

pub fn query_filtered_modules(filter: &FilterOptions, completed: &HashSet<String>) -> Vec<ModuleCardItem> {
    let mut sql = String::new();
    let mut params = Vec::new();

    let has_program = !filter.program_id.trim().is_empty();

    if has_program {
        sql.push_str(
            "SELECT DISTINCT m.id, m.code, m.title_de, m.title_en, m.department, \
             COALESCE(msp.credits, m.credits) as credits, \
             m.credits_raw, m.turnus, m.language, m.is_fues, m.cross_disciplinary, \
             m.is_phase_out, m.is_not_offered, m.limitation, m.exam_type, m.successor_modules, \
             m.prerequisites_mandatory, m.prerequisites_recommended, \
             (SELECT COUNT(*) FROM module_events me WHERE me.module_id = m.id) as events_count, \
             msp.recommended_semester, msp.module_type, msp.study_section, msp.subject_area, msp.area_rules, msp.specialization \
             FROM modules m \
             JOIN module_study_programs msp ON m.id = msp.module_id \
             WHERE msp.program_id = ? "
        );
        params.push(serde_json::Value::String(filter.program_id.clone()));

        if let Some(sem) = filter.semester {
            if sem > 0 {
                sql.push_str("AND msp.recommended_semester = ? ");
                params.push(serde_json::Value::Number(serde_json::Number::from(sem)));
            } else if sem == 0 {
                sql.push_str("AND (msp.recommended_semester IS NULL OR msp.recommended_semester <= 0) ");
            }
        }
    } else {
        sql.push_str(
            "SELECT m.id, m.code, m.title_de, m.title_en, m.department, m.credits, \
             m.credits_raw, m.turnus, m.language, m.is_fues, m.cross_disciplinary, \
             m.is_phase_out, m.is_not_offered, m.limitation, m.exam_type, m.successor_modules, \
             m.prerequisites_mandatory, m.prerequisites_recommended, \
             (SELECT COUNT(*) FROM module_events me WHERE me.module_id = m.id) as events_count, \
             NULL as recommended_semester, NULL as module_type, NULL as study_section, NULL as subject_area, NULL as area_rules, NULL as specialization \
             FROM modules m \
             WHERE 1=1 "
        );
    }

    // Search query
    let q = filter.query.trim();
    if !q.is_empty() {
        sql.push_str("AND (m.title_de LIKE ? OR m.title_en LIKE ? OR m.id LIKE ? OR m.code LIKE ? OR m.department LIKE ?) ");
        let pat = format!("%{}%", q);
        params.push(serde_json::Value::String(pat.clone()));
        params.push(serde_json::Value::String(pat.clone()));
        params.push(serde_json::Value::String(pat.clone()));
        params.push(serde_json::Value::String(pat.clone()));
        params.push(serde_json::Value::String(pat));
    }

    // Turnus filters
    if filter.turnus_next {
        // WiSe (or current semester)
        sql.push_str("AND (LOWER(m.turnus) LIKE '%winter%' OR LOWER(m.turnus) LIKE '%jede%') ");
    } else if !filter.turnus_all {
        let mut turnus_clauses = Vec::new();
        if filter.turnus_wise_even {
            turnus_clauses.push("(LOWER(m.turnus) LIKE '%winter%' AND LOWER(m.turnus) LIKE '%gerad%')");
            turnus_clauses.push("LOWER(m.turnus) LIKE '%jedes semester%'");
        }
        if filter.turnus_wise_odd {
            turnus_clauses.push("(LOWER(m.turnus) LIKE '%winter%' AND LOWER(m.turnus) LIKE '%ungerad%')");
            turnus_clauses.push("LOWER(m.turnus) LIKE '%jedes semester%'");
        }
        if filter.turnus_sose_even {
            turnus_clauses.push("(LOWER(m.turnus) LIKE '%sommer%' AND LOWER(m.turnus) LIKE '%gerad%')");
            turnus_clauses.push("LOWER(m.turnus) LIKE '%jedes semester%'");
        }
        if filter.turnus_sose_odd {
            turnus_clauses.push("(LOWER(m.turnus) LIKE '%sommer%' AND LOWER(m.turnus) LIKE '%ungerad%')");
            turnus_clauses.push("LOWER(m.turnus) LIKE '%jedes semester%'");
        }
        if filter.turnus_sporadic {
            turnus_clauses.push("(LOWER(m.turnus) LIKE '%sporadisch%' OR LOWER(m.turnus) LIKE '%ankündigung%')");
        }

        if !turnus_clauses.is_empty() {
            sql.push_str(&format!("AND ({}) ", turnus_clauses.join(" OR ")));
        }
    }

    // Modulart / Typ filter
    match filter.module_type.as_str() {
        "pflicht" => {
            if has_program {
                sql.push_str("AND (LOWER(msp.module_type) LIKE '%pflicht%' AND LOWER(msp.module_type) NOT LIKE '%wahlpflicht%' AND LOWER(msp.module_type) NOT LIKE '%wpm%') ");
            } else {
                sql.push_str("AND (EXISTS (SELECT 1 FROM program_curriculum_modules pcm WHERE pcm.module_id = m.id AND LOWER(pcm.module_type) LIKE '%pflicht%' AND LOWER(pcm.module_type) NOT LIKE '%wahlpflicht%')) ");
            }
        }
        "wahlpflicht" => {
            if has_program {
                sql.push_str("AND (LOWER(msp.module_type) LIKE '%wahlpflicht%' OR LOWER(msp.module_type) LIKE '%wpm%') ");
            } else {
                sql.push_str("AND (EXISTS (SELECT 1 FROM program_curriculum_modules pcm WHERE pcm.module_id = m.id AND (LOWER(pcm.module_type) LIKE '%wahlpflicht%' OR LOWER(pcm.module_type) LIKE '%wpm%'))) ");
            }
        }
        "fues" => {
            sql.push_str("AND (m.is_fues = 1 OR m.cross_disciplinary = 1) ");
        }
        _ => {}
    }

    // Dozierende Whitelist (+ Include)
    for prof in &filter.prof_includes {
        let p_trimmed = prof.trim();
        if !p_trimmed.is_empty() {
            sql.push_str("AND (m.responsible_persons LIKE ? OR EXISTS (SELECT 1 FROM module_events me_p JOIN event_schedules es_p ON me_p.event_id = es_p.event_id WHERE me_p.module_id = m.id AND es_p.instructor LIKE ?)) ");
            let pat = format!("%{}%", p_trimmed);
            params.push(serde_json::Value::String(pat.clone()));
            params.push(serde_json::Value::String(pat));
        }
    }

    // Dozierende Blacklist (− Exclude)
    for prof in &filter.prof_excludes {
        let p_trimmed = prof.trim();
        if !p_trimmed.is_empty() {
            sql.push_str("AND ((m.responsible_persons IS NULL OR m.responsible_persons NOT LIKE ?) AND NOT EXISTS (SELECT 1 FROM module_events me_px JOIN event_schedules es_px ON me_px.event_id = es_px.event_id WHERE me_px.module_id = m.id AND es_px.instructor LIKE ?)) ");
            let pat = format!("%{}%", p_trimmed);
            params.push(serde_json::Value::String(pat.clone()));
            params.push(serde_json::Value::String(pat));
        }
    }

    // Department / Fachgebiet filter
    let dept = filter.department.trim();
    if !dept.is_empty() {
        sql.push_str("AND m.department LIKE ? ");
        params.push(serde_json::Value::String(format!("%{}%", dept)));
    }

    // Prüfungsformen Multi-Filter
    let mut exam_clauses = Vec::new();
    if filter.exam_klausur {
        exam_clauses.push("LOWER(m.exam_type) LIKE '%klausur%'");
    }
    if filter.exam_muendlich {
        exam_clauses.push("LOWER(m.exam_type) LIKE '%mündlich%'");
    }
    if filter.exam_beleg {
        exam_clauses.push("(LOWER(m.exam_type) LIKE '%beleg%' OR LOWER(m.exam_type) LIKE '%hausarbeit%' OR LOWER(m.exam_type) LIKE '%projekt%')");
    }
    if !exam_clauses.is_empty() {
        sql.push_str(&format!("AND ({}) ", exam_clauses.join(" OR ")));
    }

    // Lehrformen Multi-Filter
    let mut teaching_clauses = Vec::new();
    if filter.teaching_vorlesung {
        teaching_clauses.push("(LOWER(m.teaching_forms) LIKE '%vorlesung%' OR EXISTS (SELECT 1 FROM module_events me_t JOIN events e_t ON me_t.event_id = e_t.id WHERE me_t.module_id = m.id AND LOWER(e_t.event_type) LIKE '%vorlesung%'))");
    }
    if filter.teaching_uebung {
        teaching_clauses.push("(LOWER(m.teaching_forms) LIKE '%übung%' OR EXISTS (SELECT 1 FROM module_events me_t JOIN events e_t ON me_t.event_id = e_t.id WHERE me_t.module_id = m.id AND LOWER(e_t.event_type) LIKE '%übung%'))");
    }
    if filter.teaching_praktikum {
        teaching_clauses.push("(LOWER(m.teaching_forms) LIKE '%praktikum%' OR EXISTS (SELECT 1 FROM module_events me_t JOIN events e_t ON me_t.event_id = e_t.id WHERE me_t.module_id = m.id AND LOWER(e_t.event_type) LIKE '%praktikum%'))");
    }
    if !teaching_clauses.is_empty() {
        sql.push_str(&format!("AND ({}) ", teaching_clauses.join(" OR ")));
    }

    // Benotung filter
    match filter.grading.as_str() {
        "benotet" => {
            sql.push_str("AND (LOWER(m.grading) LIKE '%benotet%' OR (m.grading IS NULL AND LOWER(m.exam_type) NOT LIKE '%unbenotet%')) ");
        }
        "unbenotet" => {
            sql.push_str("AND (LOWER(m.grading) LIKE '%unbenotet%' OR LOWER(m.grading) LIKE '%bestanden%' OR LOWER(m.exam_type) LIKE '%unbenotet%') ");
        }
        _ => {}
    }

    // Moduldauer filter
    match filter.duration.as_str() {
        "1" => {
            sql.push_str("AND (m.duration LIKE '%1%' OR m.duration IS NULL OR m.duration = '') ");
        }
        "2" => {
            sql.push_str("AND m.duration LIKE '%2%' ");
        }
        _ => {}
    }

    // Limitation filter: "ja" (all), "nein" (ohne Limit), "nur" (nur beschränkt)
    match filter.limitation.as_str() {
        "nein" => {
            sql.push_str("AND (m.limitation IS NULL OR m.limitation = '' OR LOWER(m.limitation) IN ('keine', 'nein', 'ohne', 'k.a.')) ");
        }
        "nur" => {
            sql.push_str("AND (m.limitation IS NOT NULL AND m.limitation != '' AND LOWER(m.limitation) NOT IN ('keine', 'nein', 'ohne', 'k.a.')) ");
        }
        _ => {}
    }

    // FÜS filter: "inkl" (all), "exkl" (ohne FÜS), "nur" (nur FÜS)
    match filter.fues.as_str() {
        "exkl" => {
            sql.push_str("AND (m.is_fues = 0 OR m.is_fues IS NULL) AND (m.cross_disciplinary = 0 OR m.cross_disciplinary IS NULL) ");
        }
        "nur" => {
            sql.push_str("AND (m.is_fues = 1 OR m.cross_disciplinary = 1) ");
        }
        _ => {}
    }

    // Hide phase out
    if filter.hide_phase_out {
        sql.push_str("AND (m.is_phase_out = 0 OR m.is_phase_out IS NULL) AND (m.is_not_offered = 0 OR m.is_not_offered IS NULL) ");
    }

    // ECTS Range filter
    if filter.min_credits > 0.0 {
        sql.push_str("AND m.credits >= ? ");
        params.push(serde_json::Value::from(filter.min_credits));
    }
    if filter.max_credits < 30.0 {
        sql.push_str("AND m.credits <= ? ");
        params.push(serde_json::Value::from(filter.max_credits));
    }

    // Campus filter
    let mut campus_clauses = Vec::new();
    if filter.campus_hauptcampus {
        campus_clauses.push("EXISTS (SELECT 1 FROM module_events me2 JOIN event_schedules es2 ON me2.event_id = es2.event_id WHERE me2.module_id = m.id AND (LOWER(es2.room) LIKE '%cottbus%' OR LOWER(es2.room) LIKE '%haupt%' OR LOWER(es2.room) LIKE '%lg%'))");
    }
    if filter.campus_sachsendorf {
        campus_clauses.push("EXISTS (SELECT 1 FROM module_events me2 JOIN event_schedules es2 ON me2.event_id = es2.event_id WHERE me2.module_id = m.id AND (LOWER(es2.room) LIKE '%sachsen%' OR LOWER(es2.room) LIKE '%sd%'))");
    }
    if filter.campus_senftenberg {
        campus_clauses.push("EXISTS (SELECT 1 FROM module_events me2 JOIN event_schedules es2 ON me2.event_id = es2.event_id WHERE me2.module_id = m.id AND (LOWER(es2.room) LIKE '%senftenberg%' OR LOWER(es2.room) LIKE '%sfb%'))");
    }
    if !campus_clauses.is_empty() {
        sql.push_str(&format!("AND ({}) ", campus_clauses.join(" OR ")));
    }

    // Language filter
    if filter.lang_de && !filter.lang_en {
        sql.push_str("AND (LOWER(m.language) LIKE '%deutsch%' OR m.language IS NULL OR m.language = '') ");
    } else if filter.lang_en && !filter.lang_de {
        sql.push_str("AND LOWER(m.language) LIKE '%engl%' ");
    }

    // Sort order
    match filter.sort_by.as_str() {
        "id" => {
            if filter.sort_asc {
                sql.push_str("ORDER BY CAST(m.id AS INTEGER) ASC ");
            } else {
                sql.push_str("ORDER BY CAST(m.id AS INTEGER) DESC ");
            }
        }
        "ects" => {
            if filter.sort_asc {
                sql.push_str("ORDER BY m.credits ASC, m.title_de ASC ");
            } else {
                sql.push_str("ORDER BY m.credits DESC, m.title_de ASC ");
            }
        }
        "events" => {
            if filter.sort_asc {
                sql.push_str("ORDER BY events_count ASC, m.title_de ASC ");
            } else {
                sql.push_str("ORDER BY events_count DESC, m.title_de ASC ");
            }
        }
        _ => {
            if filter.sort_asc {
                sql.push_str("ORDER BY m.title_de ASC ");
            } else {
                sql.push_str("ORDER BY m.title_de DESC ");
            }
        }
    }

    sql.push_str("LIMIT 300");

    let mut list: Vec<ModuleCardItem> = execute_query(&sql, &params).unwrap_or_default();

    // Filter by prerequisites fulfilled if toggle is active
    if filter.only_prereqs_met {
        list.retain(|m| {
            let (status, _, _) = evaluate_prerequisites(
                m.prerequisites_mandatory.as_deref(),
                m.prerequisites_recommended.as_deref(),
                completed,
            );
            status == PrereqStatus::Met || status == PrereqStatus::None
        });
    }

    list
}

pub fn extract_module_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            if slice.len() == 5 && !ids.contains(&slice) {
                ids.push(slice);
            }
        } else {
            i += 1;
        }
    }
    ids
}

pub fn evaluate_prerequisites(
    mandatory: Option<&str>,
    recommended: Option<&str>,
    completed: &HashSet<String>,
) -> (PrereqStatus, Vec<String>, Vec<String>) {
    let mand_ids = mandatory.map(extract_module_ids).unwrap_or_default();
    let rec_ids = recommended.map(extract_module_ids).unwrap_or_default();

    let missing_mand: Vec<String> = mand_ids.into_iter().filter(|id| !completed.contains(id)).collect();
    let missing_rec: Vec<String> = rec_ids.into_iter().filter(|id| !completed.contains(id)).collect();

    if !missing_mand.is_empty() {
        (PrereqStatus::Missing(missing_mand.clone()), missing_mand, missing_rec)
    } else if !missing_rec.is_empty() {
        (PrereqStatus::RecommendedMissing(missing_rec.clone()), missing_mand, missing_rec)
    } else if mandatory.is_some() || recommended.is_some() {
        (PrereqStatus::Met, missing_mand, missing_rec)
    } else {
        (PrereqStatus::None, missing_mand, missing_rec)
    }
}

pub fn get_module_detail(id: &str) -> Option<ModuleDetail> {
    let sql = "SELECT id, code, title_de, title_en, is_phase_out, is_not_offered, department, \
               responsible_persons, language, duration, turnus, credits, credits_raw, \
               learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory, \
               teaching_forms, literature, exam_type, exam_details, grading, limitation, \
               is_fues, cross_disciplinary, successor_modules, raw_url, study_programs, remarks \
               FROM modules WHERE id = ? LIMIT 1";
    let res: Result<Vec<ModuleDetail>, _> = execute_query(sql, &[serde_json::Value::String(id.to_string())]);
    res.ok().and_then(|mut list| if !list.is_empty() { Some(list.remove(0)) } else { None })
}

pub fn get_module_events(module_id: &str) -> Vec<EventItem> {
    let events_sql = "SELECT DISTINCT e.id, e.event_number, e.title, e.event_type, e.semester, e.sws, e.raw_url \
                      FROM events e \
                      INNER JOIN module_events me ON me.event_id = e.id \
                      WHERE me.module_id = ? \
                      ORDER BY e.event_type, e.title";
    let events_res: Result<Vec<EventRow>, _> = execute_query(events_sql, &[serde_json::Value::String(module_id.to_string())]);
    let events = match events_res {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };

    let mut result = Vec::new();
    for e in events {
        let sched_sql = "SELECT day_of_week, time_slot, start_time, end_time, rhythm, room, instructor \
                         FROM event_schedules \
                         WHERE event_id = ? \
                         ORDER BY day_of_week, start_time";
        let sched_res: Result<Vec<ScheduleItem>, _> = execute_query(sched_sql, &[serde_json::Value::String(e.id.clone())]);
        let schedules = sched_res.unwrap_or_default();
        result.push(EventItem {
            id: e.id,
            event_number: e.event_number,
            title: e.title,
            event_type: e.event_type,
            semester: e.semester,
            sws: e.sws,
            raw_url: e.raw_url,
            schedules,
        });
    }
    result
}

pub fn get_linked_programs(module_id: &str) -> Vec<ProgramOption> {
    let sql = "SELECT DISTINCT p.id, p.program_name, p.degree, p.po_version \
               FROM official_study_programs p \
               INNER JOIN module_study_programs msp ON msp.program_id = p.id \
               WHERE msp.module_id = ? AND (msp.degree IS NULL OR msp.degree != 'Abschluss im Ausland') \
               ORDER BY p.degree, p.program_name, p.po_version";
    let list: Vec<ProgramOption> = execute_query(sql, &[serde_json::Value::String(module_id.to_string())]).unwrap_or_default();
    if !list.is_empty() {
        return list;
    }
    let sql_legacy = "SELECT DISTINCT p.id, p.program_name, p.degree, p.po_version \
                      FROM study_programs p \
                      INNER JOIN program_modules pm ON pm.program_id = p.id \
                      WHERE pm.module_id = ? \
                      ORDER BY p.degree, p.program_name, p.po_version";
    execute_query(sql_legacy, &[serde_json::Value::String(module_id.to_string())]).unwrap_or_default()
}

pub fn get_curriculum_entries(module_id: &str) -> Vec<CurriculumModuleItem> {
    let sql = "SELECT DISTINCT p.id as program_id, p.program_name, p.degree, p.po_version, \
               pcm.recommended_semester, pcm.module_type, pcm.study_section, pcm.subject_area, pcm.area_rules, \
               pcm.credits, pcm.specialization \
               FROM program_curriculum_modules pcm \
               JOIN official_study_programs p ON p.id = pcm.program_id \
               WHERE pcm.module_id = ? \
               ORDER BY p.program_name, pcm.recommended_semester ASC, p.po_version DESC";
    execute_query(sql, &[serde_json::Value::String(module_id.to_string())]).unwrap_or_default()
}

pub fn get_autocomplete_suggestions(query: &str) -> Vec<ModuleCardItem> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let sql = "SELECT id, code, title_de, title_en, department, credits, credits_raw, turnus, language \
               FROM modules \
               WHERE title_de LIKE ? OR title_en LIKE ? OR id LIKE ? OR code LIKE ? \
               LIMIT 10";
    let pat = format!("%{}%", q);
    execute_query(sql, &[
        serde_json::Value::String(pat.clone()),
        serde_json::Value::String(pat.clone()),
        serde_json::Value::String(pat.clone()),
        serde_json::Value::String(pat),
    ]).unwrap_or_default()
}

pub fn get_study_program_detail(program_id: &str) -> Option<OfficialStudyProgramDetail> {
    let sql = "SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at \
               FROM official_study_programs \
               WHERE id = ? LIMIT 1";
    let list: Vec<OfficialStudyProgramDetail> = execute_query(sql, &[serde_json::Value::String(program_id.to_string())]).unwrap_or_default();
    list.into_iter().next()
}

pub fn get_study_program_all_regulations(program_name: &str, degree: &str) -> Vec<OfficialStudyProgramDetail> {
    let sql = "SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at \
               FROM official_study_programs \
               WHERE program_name = ? AND degree = ? \
               ORDER BY po_version DESC";
    execute_query(sql, &[
        serde_json::Value::String(program_name.to_string()),
        serde_json::Value::String(degree.to_string()),
    ]).unwrap_or_default()
}

pub fn get_study_program_counterpart(program_name: &str, current_degree: &str) -> Option<ProgramOption> {
    let p_clean = program_name.trim();
    let d_low = current_degree.to_lowercase();

    // Identify if currently Bachelor or Master
    let is_bachelor = d_low.contains("bachelor") || d_low.contains("b.sc") || d_low.contains("b.a") || d_low.contains("b.eng");
    let is_master = d_low.contains("master") || d_low.contains("m.sc") || d_low.contains("m.a") || d_low.contains("m.eng");

    let target_deg_clause = if is_bachelor {
        "(LOWER(degree) LIKE '%master%' OR LOWER(degree) LIKE '%m.sc%' OR LOWER(degree) LIKE '%m.a%' OR LOWER(degree) LIKE '%m.eng%')"
    } else if is_master {
        "(LOWER(degree) LIKE '%bachelor%' OR LOWER(degree) LIKE '%b.sc%' OR LOWER(degree) LIKE '%b.a%' OR LOWER(degree) LIKE '%b.eng%')"
    } else {
        return None;
    };

    // Find direct counterpart with matching or similar program name
    let sql = format!(
        "SELECT id, program_name, degree, po_version \
         FROM official_study_programs \
         WHERE {} \
           AND (LOWER(program_name) = LOWER(?) \
                OR LOWER(program_name) LIKE LOWER(?) \
                OR LOWER(?) LIKE '%' || LOWER(program_name) || '%') \
         ORDER BY po_version DESC LIMIT 1",
        target_deg_clause
    );

    let pat = format!("%{}%", p_clean);
    let list: Vec<ProgramOption> = execute_query(&sql, &[
        serde_json::Value::String(p_clean.to_string()),
        serde_json::Value::String(pat),
        serde_json::Value::String(p_clean.to_string()),
    ]).unwrap_or_default();

    list.into_iter().next()
}

pub fn get_study_program_curriculum_modules(program_id: &str) -> Vec<ModuleCardItem> {
    // 1. Try fetching directly from program_curriculum_modules (extracted from statute PO with slots & semesters)
    let pcm_sql = "SELECT \
                   COALESCE(m.id, 'curriculum_' || pcm.id) as id, \
                   COALESCE(m.code, pcm.module_code, '') as code, \
                   COALESCE(pcm.module_name, m.title_de, '') as title_de, \
                   COALESCE(pcm.module_name_en, m.title_en, '') as title_en, \
                   m.department, \
                   COALESCE(pcm.credits, m.credits) as credits, \
                   m.credits_raw, m.turnus, m.language, \
                   CASE WHEN LOWER(pcm.module_type) = 'füs' OR m.is_fues = 1 THEN 1 ELSE 0 END as is_fues, \
                   m.cross_disciplinary, m.is_phase_out, m.is_not_offered, m.limitation, \
                   COALESCE(pcm.exam_type, m.exam_type) as exam_type, \
                   m.successor_modules, \
                   COALESCE(pcm.prerequisites, m.prerequisites_mandatory) as prerequisites_mandatory, \
                   m.prerequisites_recommended, \
                   (SELECT COUNT(*) FROM module_events me WHERE me.module_id = m.id) as events_count, \
                   pcm.recommended_semester, pcm.semester_span, pcm.start_semester, pcm.end_semester, pcm.min_credits, pcm.max_credits, \
                   pcm.module_type, pcm.study_section, pcm.subject_area, pcm.area_rules, pcm.specialization \
                   FROM program_curriculum_modules pcm \
                   LEFT JOIN modules m ON pcm.module_id = m.id \
                   WHERE pcm.program_id = ? \
                   ORDER BY pcm.recommended_semester ASC, pcm.module_name ASC";
    let list: Vec<ModuleCardItem> = execute_query(pcm_sql, &[serde_json::Value::String(program_id.to_string())]).unwrap_or_default();
    if !list.is_empty() {
        return list;
    }

    // 2. Fallback to module_study_programs
    let sql = "SELECT DISTINCT m.id, m.code, m.title_de, m.title_en, m.department, \
               COALESCE(msp.credits, m.credits) as credits, \
               m.credits_raw, m.turnus, m.language, m.is_fues, m.cross_disciplinary, \
               m.is_phase_out, m.is_not_offered, m.limitation, m.exam_type, m.successor_modules, \
               m.prerequisites_mandatory, m.prerequisites_recommended, \
               (SELECT COUNT(*) FROM module_events me WHERE me.module_id = m.id) as events_count, \
               msp.recommended_semester, msp.module_type, msp.study_section, msp.subject_area, msp.area_rules, msp.specialization \
               FROM modules m \
               JOIN module_study_programs msp ON m.id = msp.module_id \
               WHERE msp.program_id = ? \
               ORDER BY msp.recommended_semester ASC, m.title_de ASC";
    execute_query(sql, &[serde_json::Value::String(program_id.to_string())]).unwrap_or_default()
}

#[derive(serde::Deserialize)]
struct DepartmentRow {
    department: Option<String>,
}

pub fn get_all_departments() -> Vec<String> {
    let sql = "SELECT DISTINCT department FROM modules WHERE department IS NOT NULL AND department != '' ORDER BY department ASC";
    let rows: Vec<DepartmentRow> = execute_query(sql, &[]).unwrap_or_default();
    let mut set = std::collections::BTreeSet::new();
    for r in rows {
        if let Some(d) = r.department {
            let trimmed = d.trim();
            if !trimmed.is_empty() {
                // Get clean last component or whole name
                if let Some(idx) = trimmed.rfind('/') {
                    if idx + 1 < trimmed.len() {
                        set.insert(trimmed[idx + 1..].trim().to_string());
                        continue;
                    }
                }
                set.insert(trimmed.to_string());
            }
        }
    }
    set.into_iter().collect()
}
