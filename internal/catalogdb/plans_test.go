package catalogdb

import (
	"database/sql"
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
