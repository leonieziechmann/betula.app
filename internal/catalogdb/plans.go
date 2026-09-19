package catalogdb

import (
	"database/sql"
	"fmt"
	"time"

	"github.com/jakob/btu-scraper/internal/normalize"
	"github.com/jakob/btu-scraper/internal/qistree"
)

// Plan is a validated study plan of one program (source: statute PDF).
type Plan struct {
	ProgramID   string
	SourceFile  string
	LayoutJSON  string
	ValidatedAt time.Time
	Entries     []PlanEntry
}

// PlanEntry is one row of a study plan. Zero values mean "the PDF does not say".
type PlanEntry struct {
	ModuleID       string // catalog module the row was matched to, "" if none
	ModuleCodeRaw  string
	ModuleName     string
	Semester       int // exact semester; 0 when the PDF gives a span or nothing
	StartSemester  int
	EndSemester    int
	SemesterSpan   string
	Credits        float64
	MinCredits     float64
	MaxCredits     float64
	KindRaw        string // „Pflicht", „Wahlpflicht" … as extracted
	StudySection   string
	SubjectArea    string
	AreaRules      string
	Specialization string
	SourceEvidence string
}

// SavePlan replaces the validated plan of a program atomically: either the new
// plan with all its entries is stored, or the previous plan stays untouched.
func (db *DB) SavePlan(p Plan) error {
	tx, err := db.sql.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	if err := savePlanTx(tx, p); err != nil {
		return err
	}
	return tx.Commit()
}

func savePlanTx(tx *sql.Tx, p Plan) error {
	if p.ProgramID == "" || p.SourceFile == "" || p.LayoutJSON == "" || len(p.Entries) == 0 {
		return fmt.Errorf("a validated plan requires program, source file, layout and entries")
	}
	if p.ValidatedAt.IsZero() {
		p.ValidatedAt = time.Now()
	}
	// plan_entry rows go with their plan (ON DELETE CASCADE).
	if _, err := tx.Exec("DELETE FROM plan WHERE program_id = ?", p.ProgramID); err != nil {
		return err
	}
	if _, err := tx.Exec("INSERT INTO plan (program_id, source_file, layout_json, validated_at) VALUES (?, ?, ?, ?)",
		p.ProgramID, p.SourceFile, p.LayoutJSON, p.ValidatedAt.UTC().Format(time.RFC3339)); err != nil {
		return err
	}

	stmt, err := tx.Prepare(`
		INSERT INTO plan_entry (
			program_id, ord, module_id, module_code_raw, module_name,
			semester, start_semester, end_semester, semester_span,
			credits, min_credits, max_credits, kind, kind_raw,
			study_section, subject_area, area_rules, specialization, source_evidence
		) VALUES (?,?,?,?,?, ?,?,?,?, ?,?,?,?,?, ?,?,?,?,?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for i, e := range p.Entries {
		if e.ModuleName == "" {
			return fmt.Errorf("plan %s: entry %d has no module name", p.ProgramID, i+1)
		}
		_, err := stmt.Exec(
			p.ProgramID, i+1, nullIfZero(e.ModuleID), nullIfZero(e.ModuleCodeRaw), e.ModuleName,
			nullIfZero(e.Semester), nullIfZero(e.StartSemester), nullIfZero(e.EndSemester), nullIfZero(e.SemesterSpan),
			nullIfZero(e.Credits), nullIfZero(e.MinCredits), nullIfZero(e.MaxCredits),
			nullIfZero(normalize.PlanKind(e.KindRaw)), nullIfZero(e.KindRaw),
			nullIfZero(e.StudySection), nullIfZero(e.SubjectArea), nullIfZero(e.AreaRules),
			nullIfZero(e.Specialization), nullIfZero(e.SourceEvidence))
		if err != nil {
			return fmt.Errorf("plan %s: entry %d: %w", p.ProgramID, i+1, err)
		}
	}
	return nil
}

// SetPlanScanStatus records the outcome of the last scan of a program's statutes.
func (db *DB) SetPlanScanStatus(programID, status, message, source string) error {
	_, err := db.sql.Exec(`
		INSERT INTO plan_scan_status (program_id, status, message, source, updated_at)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(program_id) DO UPDATE SET
			status = excluded.status, message = excluded.message,
			source = excluded.source, updated_at = excluded.updated_at`,
		programID, status, nullIfZero(message), nullIfZero(source), time.Now().UTC().Format(time.RFC3339))
	return err
}

func nullIfZero[T comparable](v T) any {
	var zero T
	if v == zero {
		return nil
	}
	return v
}

// LegacyImport summarises ImportLegacyPlans.
type LegacyImport struct {
	Plans          int
	Entries        int
	ScanStatuses   int
	SkippedNoQISID []string // legacy programs whose QIS URL does not identify a PO
}

// ImportLegacyPlans copies the validated study plans of a schema v1 database. They are
// the only v1 data worth keeping: every plan was checked against the PDF geometry, and
// reproducing them costs Gemini calls. Rows of unvalidated scans are not imported, and
// the v1 placeholders (0 and the empty string) become NULL. Program IDs are translated through the QIS
// URL of the PO, which both schemas know.
func (db *DB) ImportLegacyPlans(legacyPath string) (*LegacyImport, error) {
	legacy, err := sql.Open("sqlite", legacyPath+"?mode=ro&_pragma=query_only(1)")
	if err != nil {
		return nil, err
	}
	defer legacy.Close()

	result := &LegacyImport{}
	idMap := make(map[string]string) // v1 program id → v2 program id
	err = queryRows(legacy, "SELECT id, COALESCE(qis_url, '') FROM official_study_programs ORDER BY id", nil,
		func(scan func(...any) error) error {
			var id, qisURL string
			if err := scan(&id, &qisURL); err != nil {
				return err
			}
			if node := qistree.ParseNodeID(qisURL); node.IsPO() {
				idMap[id] = node.ProgramID()
			} else {
				result.SkippedNoQISID = append(result.SkippedNoQISID, id)
			}
			return nil
		})
	if err != nil {
		return nil, fmt.Errorf("failed to read legacy programs: %w", err)
	}

	var plans []Plan
	legacyIDs := make(map[string]string) // v2 id → v1 id
	err = queryRows(legacy, "SELECT program_id, source_file, layout_json, validated_at FROM validated_curriculum_plans ORDER BY program_id", nil,
		func(scan func(...any) error) error {
			var legacyID, validatedAt string
			var p Plan
			if err := scan(&legacyID, &p.SourceFile, &p.LayoutJSON, &validatedAt); err != nil {
				return err
			}
			if newID, ok := idMap[legacyID]; ok {
				p.ProgramID = newID
				p.ValidatedAt = parseLegacyTime(validatedAt)
				legacyIDs[newID] = legacyID
				plans = append(plans, p)
			}
			return nil
		})
	if err != nil {
		return nil, fmt.Errorf("failed to read legacy plans: %w", err)
	}

	for i := range plans {
		p := &plans[i]
		err := queryRows(legacy, `
			SELECT COALESCE(module_id, ''), COALESCE(module_code, ''), module_name,
			       COALESCE(recommended_semester, 0), COALESCE(start_semester, 0), COALESCE(end_semester, 0), COALESCE(semester_span, ''),
			       COALESCE(credits, 0), COALESCE(min_credits, 0), COALESCE(max_credits, 0), COALESCE(module_type, ''),
			       COALESCE(study_section, ''), COALESCE(subject_area, ''), COALESCE(area_rules, ''),
			       COALESCE(specialization, ''), COALESCE(source_evidence, '')
			FROM program_curriculum_modules
			WHERE program_id = ? AND source_file = ?
			ORDER BY id`, []any{legacyIDs[p.ProgramID], p.SourceFile},
			func(scan func(...any) error) error {
				var e PlanEntry
				if err := scan(&e.ModuleID, &e.ModuleCodeRaw, &e.ModuleName,
					&e.Semester, &e.StartSemester, &e.EndSemester, &e.SemesterSpan,
					&e.Credits, &e.MinCredits, &e.MaxCredits, &e.KindRaw,
					&e.StudySection, &e.SubjectArea, &e.AreaRules, &e.Specialization, &e.SourceEvidence); err != nil {
					return err
				}
				p.Entries = append(p.Entries, e)
				return nil
			})
		if err != nil {
			return nil, fmt.Errorf("failed to read legacy plan rows of %s: %w", p.ProgramID, err)
		}
	}

	type scanStatus struct{ programID, status, message, source, checkedAt string }
	var statuses []scanStatus
	err = queryRows(legacy, "SELECT program_id, status, COALESCE(message, ''), COALESCE(source_file, ''), checked_at FROM program_scan_status ORDER BY program_id", nil,
		func(scan func(...any) error) error {
			var s scanStatus
			if err := scan(&s.programID, &s.status, &s.message, &s.source, &s.checkedAt); err != nil {
				return err
			}
			if newID, ok := idMap[s.programID]; ok {
				s.programID = newID
				statuses = append(statuses, s)
			}
			return nil
		})
	if err != nil {
		return nil, fmt.Errorf("failed to read legacy scan status: %w", err)
	}

	// One transaction: a failed import leaves the existing plans as they were.
	tx, err := db.sql.Begin()
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()

	for _, p := range plans {
		if len(p.Entries) == 0 {
			continue
		}
		if err := savePlanTx(tx, p); err != nil {
			return nil, err
		}
		result.Plans++
		result.Entries += len(p.Entries)
	}
	for _, s := range statuses {
		_, err := tx.Exec(`
			INSERT INTO plan_scan_status (program_id, status, message, source, updated_at) VALUES (?, ?, ?, ?, ?)
			ON CONFLICT(program_id) DO UPDATE SET status = excluded.status, message = excluded.message,
				source = excluded.source, updated_at = excluded.updated_at`,
			s.programID, s.status, nullIfZero(s.message), nullIfZero(s.source), parseLegacyTime(s.checkedAt).UTC().Format(time.RFC3339))
		if err != nil {
			return nil, fmt.Errorf("scan status %s: %w", s.programID, err)
		}
		result.ScanStatuses++
	}
	return result, tx.Commit()
}

func queryRows(db *sql.DB, query string, args []any, each func(scan func(...any) error) error) error {
	rows, err := db.Query(query, args...)
	if err != nil {
		return err
	}
	defer rows.Close()
	for rows.Next() {
		if err := each(rows.Scan); err != nil {
			return err
		}
	}
	return rows.Err()
}

// parseLegacyTime reads the two timestamp formats v1 used.
func parseLegacyTime(s string) time.Time {
	for _, layout := range []string{time.RFC3339, "2006-01-02 15:04:05"} {
		if t, err := time.Parse(layout, s); err == nil {
			return t
		}
	}
	return time.Now()
}
