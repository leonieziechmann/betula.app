package storage

// programCoverageSchema records how far each study program is covered and
// exposes the result as the view program_coverage, which the overview page reads.
//
// level = 'plan'    the Prüfungsordnung's semester plan is parsed and validated
//
//	'modules' no validated plan, but catalog modules are linked to the program
//	'none'    nothing is known beyond the program's existence
const programCoverageSchema = `
CREATE TABLE IF NOT EXISTS program_scan_status (
	program_id TEXT PRIMARY KEY,
	status TEXT NOT NULL,
	message TEXT,
	source_file TEXT,
	checked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
DROP VIEW IF EXISTS program_coverage;
CREATE VIEW program_coverage AS
SELECT p.id AS program_id, p.program_name AS program_name, p.degree AS degree, p.po_version AS po_version,
	CASE WHEN v.program_id IS NOT NULL THEN 'plan'
	     WHEN COALESCE(l.n, 0) > 0 THEN 'modules'
	     ELSE 'none' END AS level,
	COALESCE(l.n, 0) AS linked_modules,
	COALESCE(r.n, 0) AS plan_requirements,
	COALESCE(r.linked, 0) AS plan_linked,
	COALESCE(r.semesters, 0) AS plan_semesters,
	COALESCE(s.status, '') AS scan_status,
	COALESCE(s.message, '') AS scan_message
FROM official_study_programs p
LEFT JOIN validated_curriculum_plans v ON v.program_id = p.id
LEFT JOIN (SELECT program_id, COUNT(*) AS n FROM module_study_programs GROUP BY program_id) l ON l.program_id = p.id
LEFT JOIN (SELECT program_id, COUNT(*) AS n,
		SUM(CASE WHEN COALESCE(module_id, '') <> '' THEN 1 ELSE 0 END) AS linked,
		MAX(CASE WHEN COALESCE(end_semester, 0) > 0 THEN end_semester ELSE COALESCE(recommended_semester, 0) END) AS semesters
	FROM program_curriculum_modules WHERE COALESCE(source_file, '') <> 'qis_tree' GROUP BY program_id) r ON r.program_id = p.id
LEFT JOIN program_scan_status s ON s.program_id = p.id;
`

// EnsureProgramCoverage creates the status table and the coverage view.
func (s *Storage) EnsureProgramCoverage() error {
	_, err := s.db.Exec(programCoverageSchema)
	return err
}

// SaveProgramScanStatus remembers the outcome of the last scan of a program,
// including why a program has no study plan (for example a discontinued program).
func (s *Storage) SaveProgramScanStatus(programID, status, message, sourceFile string) error {
	_, err := s.db.Exec(`INSERT INTO program_scan_status(program_id, status, message, source_file, checked_at)
		VALUES(?, ?, ?, ?, CURRENT_TIMESTAMP)
		ON CONFLICT(program_id) DO UPDATE SET status=excluded.status, message=excluded.message,
		source_file=excluded.source_file, checked_at=excluded.checked_at`, programID, status, message, sourceFile)
	return err
}
