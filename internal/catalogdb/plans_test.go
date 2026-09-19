package catalogdb

import (
	"database/sql"
	"path/filepath"
	"testing"
)

func planCounts(t *testing.T, db *DB, programID string) (plans, entries int, sourceFile string) {
	t.Helper()
	_ = db.SQL().QueryRow("SELECT COUNT(*), COALESCE(MAX(source_file), '') FROM plan WHERE program_id = ?", programID).Scan(&plans, &sourceFile)
	_ = db.SQL().QueryRow("SELECT COUNT(*) FROM plan_entry WHERE program_id = ?", programID).Scan(&entries)
	return
}

func TestSavePlanReplacesAtomically(t *testing.T) {
	db := openTestDB(t)
	first := Plan{ProgramID: "079-82-2008", SourceFile: "old.pdf", LayoutJSON: "{}", Entries: []PlanEntry{
		{ModuleName: "A", Semester: 1, Credits: 6, KindRaw: "Pflicht"},
		{ModuleName: "B", Semester: 2, Credits: 6, KindRaw: "Pflicht"},
	}}
	if err := db.SavePlan(first); err != nil {
		t.Fatalf("SavePlan failed: %v", err)
	}

	// A broken replacement must leave the validated plan untouched.
	broken := Plan{ProgramID: "079-82-2008", SourceFile: "new.pdf", LayoutJSON: "{}", Entries: []PlanEntry{
		{ModuleName: "C", Semester: 1},
		{ModuleName: ""},
	}}
	if err := db.SavePlan(broken); err == nil {
		t.Fatal("SavePlan accepted an entry without a module name")
	}
	if plans, entries, source := planCounts(t, db, "079-82-2008"); plans != 1 || entries != 2 || source != "old.pdf" {
		t.Fatalf("after failed replace: plans=%d entries=%d source=%q", plans, entries, source)
	}

	for _, incomplete := range []Plan{
		{SourceFile: "x.pdf", LayoutJSON: "{}", Entries: first.Entries},
		{ProgramID: "p", LayoutJSON: "{}", Entries: first.Entries},
		{ProgramID: "p", SourceFile: "x.pdf", Entries: first.Entries},
		{ProgramID: "p", SourceFile: "x.pdf", LayoutJSON: "{}"},
	} {
		if err := db.SavePlan(incomplete); err == nil {
			t.Errorf("SavePlan accepted incomplete plan %+v", incomplete)
		}
	}

	replacement := Plan{ProgramID: "079-82-2008", SourceFile: "new.pdf", LayoutJSON: "{}", Entries: []PlanEntry{{ModuleName: "C", StartSemester: 5, EndSemester: 6, SemesterSpan: "5-6", MinCredits: 10, MaxCredits: 24, KindRaw: "Modul"}}}
	if err := db.SavePlan(replacement); err != nil {
		t.Fatalf("SavePlan replace failed: %v", err)
	}
	if plans, entries, source := planCounts(t, db, "079-82-2008"); plans != 1 || entries != 1 || source != "new.pdf" {
		t.Fatalf("after replace: plans=%d entries=%d source=%q", plans, entries, source)
	}

	// Unknown stays NULL: no exact semester, no single credit value, no kind for „Modul".
	var semester, credits, kind sql.NullString
	var span string
	var minCredits float64
	err := db.SQL().QueryRow("SELECT semester, credits, kind, semester_span, min_credits FROM plan_entry WHERE program_id = '079-82-2008'").
		Scan(&semester, &credits, &kind, &span, &minCredits)
	if err != nil || semester.Valid || credits.Valid || kind.Valid || span != "5-6" || minCredits != 10 {
		t.Fatalf("entry = semester %v credits %v kind %v span %q min %v (err %v)", semester, credits, kind, span, minCredits, err)
	}
}

func TestImportLegacyPlansOnlyTakesValidatedRows(t *testing.T) {
	legacyPath := filepath.Join(t.TempDir(), "v1.db")
	legacy, err := sql.Open("sqlite", legacyPath)
	if err != nil {
		t.Fatal(err)
	}
	poURL := "https://www.b-tu.de/qisserver3/rds?state=modulBeschrGast&nodeID=auswahlBaum%7Cstudiengang%3Astg%3D079%7Cabschluss%3Aabschl%3D82%7CstgSpecials%3Avert%3D%2Cschwp%3D%2Ckzfa%3DH%2Cpversion%3D2008"
	_, err = legacy.Exec(`
		CREATE TABLE official_study_programs (id TEXT PRIMARY KEY, qis_url TEXT);
		CREATE TABLE validated_curriculum_plans (program_id TEXT PRIMARY KEY, source_file TEXT, layout_json TEXT, validated_at TEXT);
		CREATE TABLE program_curriculum_modules (id INTEGER PRIMARY KEY, program_id TEXT, module_id TEXT, module_code TEXT, module_name TEXT,
			recommended_semester INTEGER, start_semester INTEGER, end_semester INTEGER, semester_span TEXT,
			credits REAL, min_credits REAL, max_credits REAL, module_type TEXT, study_section TEXT, subject_area TEXT,
			area_rules TEXT, specialization TEXT, source_evidence TEXT, source_file TEXT);
		CREATE TABLE program_scan_status (program_id TEXT PRIMARY KEY, status TEXT, message TEXT, source_file TEXT, checked_at TEXT);

		INSERT INTO official_study_programs VALUES ('stg_079_abschl_82_po_2008_-_2._SÄ_2024', '` + poURL + `'), ('stg_no_url', '');
		INSERT INTO validated_curriculum_plans VALUES ('stg_079_abschl_82_po_2008_-_2._SÄ_2024', 'po.pdf', '{"pages":[7]}', '2026-09-18 14:53:17');
		INSERT INTO program_curriculum_modules (program_id, module_id, module_name, recommended_semester, start_semester, end_semester, credits, min_credits, max_credits, module_type, source_file, source_evidence) VALUES
			('stg_079_abschl_82_po_2008_-_2._SÄ_2024', '12104', 'Entwicklung von Softwaresystemen', 1, 1, 1, 8, 0, 0, 'Pflicht', 'po.pdf', '{"id":"p7t1r5c2"}'),
			('stg_079_abschl_82_po_2008_-_2._SÄ_2024', '',      'Theoretische Informatik',          3, 3, 3, 8, 0, 0, 'Pflicht', 'po.pdf', NULL),
			('stg_079_abschl_82_po_2008_-_2._SÄ_2024', '11861', 'Operating Systems II',             0, 0, 0, 0, 0, 0, 'Pflicht', 'qis_tree', NULL),
			('stg_079_abschl_82_po_2008_-_2._SÄ_2024', '99999', 'Unverified AI row',                2, 2, 2, 6, 0, 0, 'Pflicht', 'older-scan.pdf', NULL);
		INSERT INTO program_scan_status VALUES ('stg_079_abschl_82_po_2008_-_2._SÄ_2024', 'saved_with_warnings', 'low coverage', 'po.pdf', '2026-09-18 14:53:17');
	`)
	if err != nil {
		t.Fatalf("failed to create legacy fixture: %v", err)
	}
	_ = legacy.Close()

	db := openTestDB(t)
	result, err := db.ImportLegacyPlans(legacyPath)
	if err != nil {
		t.Fatalf("ImportLegacyPlans failed: %v", err)
	}
	if result.Plans != 1 || result.Entries != 2 || result.ScanStatuses != 1 || len(result.SkippedNoQISID) != 1 {
		t.Fatalf("result = %+v", result)
	}

	var moduleID sql.NullString
	var minCredits sql.NullFloat64
	var kind, validatedAt string
	err = db.SQL().QueryRow(`SELECT e.module_id, e.min_credits, e.kind, p.validated_at FROM plan_entry e JOIN plan p ON p.program_id = e.program_id
		WHERE e.program_id = '079-82-2008' AND e.module_name = 'Theoretische Informatik'`).Scan(&moduleID, &minCredits, &kind, &validatedAt)
	if err != nil || moduleID.Valid || minCredits.Valid || kind != "compulsory" || validatedAt != "2026-09-18T14:53:17Z" {
		t.Fatalf("entry: module %v min %v kind %q validated %q (err %v)", moduleID, minCredits, kind, validatedAt, err)
	}
}
