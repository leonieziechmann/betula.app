use std::sync::{Arc, Mutex};
use rusqlite::{Connection, OpenFlags, Result, params};
use serde::{Deserialize, Serialize};
use crate::slug::{ProgramOption, program_slug};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub id: String,
    pub code: String,
    pub title_de: String,
    pub title_en: Option<String>,
    pub department: Option<String>,
    pub credits: f64,
    pub credits_raw: Option<String>,
    pub turnus: Option<String>,
    pub language: Option<String>,
    pub is_fues: bool,
    pub exam_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkedProgram {
    pub program_id: String,
    pub program_name: String,
    pub degree: Option<String>,
    pub po_version: Option<String>,
    pub slug: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleDetail {
    pub id: String,
    pub code: String,
    pub title_de: String,
    pub title_en: Option<String>,
    pub department: Option<String>,
    pub credits: f64,
    pub credits_raw: Option<String>,
    pub turnus: Option<String>,
    pub language: Option<String>,
    pub duration: Option<String>,
    pub exam_type: Option<String>,
    pub exam_details: Option<String>,
    pub grading: Option<String>,
    pub limitation: Option<String>,
    pub learning_outcomes: Option<String>,
    pub contents: Option<String>,
    pub prerequisites_recommended: Option<String>,
    pub prerequisites_mandatory: Option<String>,
    pub teaching_forms: Option<String>,
    pub literature: Option<String>,
    pub responsible_persons: Option<String>,
    pub study_programs_raw: Option<String>,
    pub successor_modules: Option<String>,
    pub linked_programs: Vec<LinkedProgram>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurriculumEntry {
    pub module_id: String,
    pub module_code: String,
    pub module_title: String,
    pub semester: i64,
    pub credits: f64,
    pub module_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramDetail {
    pub id: String,
    pub program_name: String,
    pub degree: Option<String>,
    pub po_version: Option<String>,
    pub slug: String,
    pub curriculum: Vec<CurriculumEntry>,
    pub electives: Vec<ModuleSummary>,
    pub all_modules: Vec<ModuleSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbStats {
    pub total_modules: i64,
    pub total_programs: i64,
}

/// How much is known about one study program (row of the SQL view `program_coverage`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageRow {
    pub program_id: String,
    pub program_name: String,
    pub degree: String,
    pub po_version: String,
    /// "plan" (Prüfungsordnung geparst), "modules" (nur verknüpfte Module) or "none"
    pub level: String,
    pub linked_modules: i64,
    pub plan_requirements: i64,
    pub plan_linked: i64,
    pub plan_semesters: i64,
    pub scan_status: String,
    pub scan_message: String,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI,
        )?;
        // Enable WAL mode reading optimizations
        conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn get_stats(&self) -> Result<DbStats> {
        let conn = self.conn.lock().unwrap();
        let total_modules: i64 = conn.query_row("SELECT COUNT(*) FROM modules", [], |r| r.get(0))?;
        let total_programs: i64 = conn.query_row("SELECT COUNT(*) FROM official_study_programs", [], |r| r.get(0)).unwrap_or(0);
        Ok(DbStats {
            total_modules,
            total_programs,
        })
    }

    pub fn get_study_programs(&self) -> Result<Vec<ProgramOption>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, program_name, degree, po_version \
             FROM official_study_programs \
             ORDER BY program_name ASC, po_version DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProgramOption {
                id: row.get(0)?,
                program_name: row.get(1)?,
                degree: row.get(2)?,
                po_version: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_program_coverage(&self) -> Result<Vec<CoverageRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT program_id, program_name, COALESCE(degree, ''), COALESCE(po_version, ''), level, \
             linked_modules, plan_requirements, plan_linked, plan_semesters, scan_status, scan_message \
             FROM program_coverage \
             ORDER BY program_name ASC, degree ASC, po_version DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CoverageRow {
                program_id: row.get(0)?,
                program_name: row.get(1)?,
                degree: row.get(2)?,
                po_version: row.get(3)?,
                level: row.get(4)?,
                linked_modules: row.get(5)?,
                plan_requirements: row.get(6)?,
                plan_linked: row.get(7)?,
                plan_semesters: row.get(8)?,
                scan_status: row.get(9)?,
                scan_message: row.get(10)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_all_modules_summary(&self) -> Result<Vec<ModuleSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, code, title_de, title_en, department, credits, credits_raw, turnus, language, is_fues, exam_type \
             FROM modules \
             ORDER BY code ASC, title_de ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let is_fues_int: i64 = row.get(9).unwrap_or(0);
            Ok(ModuleSummary {
                id: row.get(0)?,
                code: row.get(1)?,
                title_de: row.get(2)?,
                title_en: row.get(3)?,
                department: row.get(4)?,
                credits: row.get(5).unwrap_or(0.0),
                credits_raw: row.get(6)?,
                turnus: row.get(7)?,
                language: row.get(8)?,
                is_fues: is_fues_int > 0,
                exam_type: row.get(10)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_module_detail(&self, id_or_code: &str, all_programs: &[ProgramOption]) -> Result<Option<ModuleDetail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, code, title_de, title_en, department, credits, credits_raw, turnus, language, duration, \
                    exam_type, exam_details, grading, limitation, learning_outcomes, contents, \
                    prerequisites_recommended, prerequisites_mandatory, teaching_forms, literature, \
                    responsible_persons, study_programs, successor_modules \
             FROM modules \
             WHERE id = ?1 OR code = ?1 \
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![id_or_code])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let id: String = row.get(0)?;
        let code: String = row.get(1)?;
        let title_de: String = row.get(2)?;
        let title_en: Option<String> = row.get(3)?;
        let department: Option<String> = row.get(4)?;
        let credits: f64 = row.get(5).unwrap_or(0.0);
        let credits_raw: Option<String> = row.get(6)?;
        let turnus: Option<String> = row.get(7)?;
        let language: Option<String> = row.get(8)?;
        let duration: Option<String> = row.get(9)?;
        let exam_type: Option<String> = row.get(10)?;
        let exam_details: Option<String> = row.get(11)?;
        let grading: Option<String> = row.get(12)?;
        let limitation: Option<String> = row.get(13)?;
        let learning_outcomes: Option<String> = row.get(14)?;
        let contents: Option<String> = row.get(15)?;
        let prerequisites_recommended: Option<String> = row.get(16)?;
        let prerequisites_mandatory: Option<String> = row.get(17)?;
        let teaching_forms: Option<String> = row.get(18)?;
        let literature: Option<String> = row.get(19)?;
        let responsible_persons: Option<String> = row.get(20)?;
        let study_programs_raw: Option<String> = row.get(21)?;
        let successor_modules: Option<String> = row.get(22)?;

        // Find linked programs
        let mut prog_stmt = conn.prepare(
            "SELECT DISTINCT osp.id, osp.program_name, osp.degree, osp.po_version \
             FROM module_study_programs msp \
             JOIN official_study_programs osp ON msp.program_id = osp.id \
             WHERE msp.module_id = ?1 \
             ORDER BY osp.program_name ASC"
        )?;

        let prog_rows = prog_stmt.query_map(params![id], |r| {
            let pid: String = r.get(0)?;
            let pname: String = r.get(1)?;
            let deg: Option<String> = r.get(2)?;
            let po: Option<String> = r.get(3)?;
            let slug = program_slug(&pid, all_programs);
            Ok(LinkedProgram {
                program_id: pid,
                program_name: pname,
                degree: deg,
                po_version: po,
                slug,
            })
        })?;

        let mut linked_programs = Vec::new();
        for p in prog_rows {
            linked_programs.push(p?);
        }

        Ok(Some(ModuleDetail {
            id,
            code,
            title_de,
            title_en,
            department,
            credits,
            credits_raw,
            turnus,
            language,
            duration,
            exam_type,
            exam_details,
            grading,
            limitation,
            learning_outcomes,
            contents,
            prerequisites_recommended,
            prerequisites_mandatory,
            teaching_forms,
            literature,
            responsible_persons,
            study_programs_raw,
            successor_modules,
            linked_programs,
        }))
    }

    pub fn get_program_detail(&self, program_id: &str, all_programs: &[ProgramOption]) -> Result<Option<ProgramDetail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, program_name, degree, po_version \
             FROM official_study_programs \
             WHERE id = ?1 \
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![program_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let id: String = row.get(0)?;
        let program_name: String = row.get(1)?;
        let degree: Option<String> = row.get(2)?;
        let po_version: Option<String> = row.get(3)?;
        let slug = program_slug(&id, all_programs);

        // Fetch curriculum plan entries from program_curriculum_modules
        let cur_stmt = conn.prepare(
            "SELECT COALESCE(m.id, 'curriculum_' || pcm.id) as id, \
                    COALESCE(m.code, pcm.module_code, '') as code, \
                    COALESCE(pcm.module_name, m.title_de, '') as title_de, \
                    COALESCE(pcm.recommended_semester, pcm.start_semester, 0) as semester, \
                    COALESCE(pcm.credits, m.credits, 0.0) as credits, \
                    COALESCE(pcm.module_type, 'Pflicht') as module_type \
             FROM program_curriculum_modules pcm \
             LEFT JOIN modules m ON pcm.module_id = m.id \
             WHERE pcm.program_id = ?1 \
             ORDER BY COALESCE(pcm.recommended_semester, pcm.start_semester, 0) ASC, pcm.module_name ASC"
        );

        let mut curriculum = Vec::new();
        if let Ok(mut c_stmt) = cur_stmt {
            if let Ok(c_rows) = c_stmt.query_map(params![id], |r| {
                Ok(CurriculumEntry {
                    module_id: r.get(0)?,
                    module_code: r.get(1)?,
                    module_title: r.get(2)?,
                    semester: r.get(3)?,
                    credits: r.get(4)?,
                    module_type: r.get(5)?,
                })
            }) {
                for c in c_rows {
                    if let Ok(entry) = c {
                        curriculum.push(entry);
                    }
                }
            }
        }

        // Fallback to module_study_programs if no pcm records exist
        if curriculum.is_empty() {
            let fallback_stmt = conn.prepare(
                "SELECT DISTINCT m.id, COALESCE(m.code, '') as code, m.title_de, \
                        COALESCE(msp.recommended_semester, 0) as semester, \
                        COALESCE(msp.credits, m.credits, 0.0) as credits, \
                        COALESCE(msp.module_type, 'Pflicht') as module_type \
                 FROM modules m \
                 JOIN module_study_programs msp ON m.id = msp.module_id \
                 WHERE msp.program_id = ?1 \
                 ORDER BY COALESCE(msp.recommended_semester, 0) ASC, m.title_de ASC"
            );
            if let Ok(mut fb_stmt) = fallback_stmt {
                if let Ok(fb_rows) = fb_stmt.query_map(params![id], |r| {
                    Ok(CurriculumEntry {
                        module_id: r.get(0)?,
                        module_code: r.get(1)?,
                        module_title: r.get(2)?,
                        semester: r.get(3)?,
                        credits: r.get(4)?,
                        module_type: r.get(5)?,
                    })
                }) {
                    for c in fb_rows {
                        if let Ok(entry) = c {
                            curriculum.push(entry);
                        }
                    }
                }
            }
        }

        // Fetch all assigned modules
        let mut mod_stmt = conn.prepare(
            "SELECT DISTINCT m.id, m.code, m.title_de, m.title_en, m.department, \
                    COALESCE(msp.credits, m.credits), m.credits_raw, m.turnus, m.language, m.is_fues, m.exam_type \
             FROM modules m \
             JOIN module_study_programs msp ON m.id = msp.module_id \
             WHERE msp.program_id = ?1 \
             ORDER BY m.code ASC, m.title_de ASC"
        )?;

        let m_rows = mod_stmt.query_map(params![id], |r| {
            let is_fues_int: i64 = r.get(9).unwrap_or(0);
            Ok(ModuleSummary {
                id: r.get(0)?,
                code: r.get(1)?,
                title_de: r.get(2)?,
                title_en: r.get(3)?,
                department: r.get(4)?,
                credits: r.get(5).unwrap_or(0.0),
                credits_raw: r.get(6)?,
                turnus: r.get(7)?,
                language: r.get(8)?,
                is_fues: is_fues_int > 0,
                exam_type: r.get(10)?,
            })
        })?;

        let mut all_modules = Vec::new();
        let mut electives = Vec::new();
        for m in m_rows {
            let module = m?;
            if module.is_fues || module.exam_type.as_deref().unwrap_or("").to_lowercase().contains("wahl") {
                electives.push(module.clone());
            }
            all_modules.push(module);
        }

        Ok(Some(ProgramDetail {
            id,
            program_name,
            degree,
            po_version,
            slug,
            curriculum,
            electives,
            all_modules,
        }))
    }
}
