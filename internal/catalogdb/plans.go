package catalogdb

import (
	"database/sql"
	"fmt"
	"time"

	"github.com/leonieziechmann/betula/internal/normalize"
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
