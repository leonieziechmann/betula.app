package storage

import (
	"path/filepath"
	"testing"

	"github.com/jakob/btu-scraper/internal/model"
)

func TestCatalogRowsDoNotSuppressCurriculumScan(t *testing.T) {
	s, err := NewStorage(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	if _, err = s.db.Exec("INSERT INTO official_study_programs(id,program_name,degree,po_version) VALUES ('p','Test','Bachelor','2024')"); err != nil {
		t.Fatal(err)
	}
	for _, source := range []string{"qis_tree", "old-ai.pdf"} {
		if _, err = s.db.Exec("INSERT INTO program_curriculum_modules(program_id,program_name,module_name,module_type,source_file) VALUES ('p','Test','Test','Pflicht',?)", source); err != nil {
			t.Fatal(err)
		}
	}
	if has, err := s.HasValidatedCurriculum("p"); err != nil || has {
		t.Fatalf("catalog rows must not count as validated: %v %v", has, err)
	}
	if _, err = s.db.Exec("INSERT INTO validated_curriculum_plans(program_id,source_file,layout_json) VALUES ('p','verified.pdf','{}')"); err != nil {
		t.Fatal(err)
	}
	if has, err := s.HasValidatedCurriculum("p"); err != nil || !has {
		t.Fatalf("validated snapshot not recognized: %v %v", has, err)
	}
}

func TestValidatedCurriculumAtomicReplacement(t *testing.T) {
	s, err := NewStorage(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	_, err = s.db.Exec(`INSERT INTO modules(id,title_de,turnus,credits) VALUES ('1','Test','jedes Sommersemester',6)`)
	if err != nil {
		t.Fatal(err)
	}
	_, err = s.db.Exec(`INSERT INTO official_study_programs(id,program_name,degree,po_version) VALUES ('p','Program','Bachelor','2024')`)
	if err != nil {
		t.Fatal(err)
	}
	first := []model.CurriculumModule{{ModuleID: "1", ModuleCode: "1", ModuleName: "Test", RecommendedSemester: 1, Credits: 6, SourceEvidence: `{"semesters":[2,3],"workload":[3,3],"credit_semester":3}`}}
	save := func(ms []model.CurriculumModule) error {
		return s.SaveValidatedCurriculumModules("p", "Program", "Bachelor", "2024", ms, "test.pdf", `{"cells":[],"plan_names":{"p1t1":"Plan"}}`)
	}
	if err := save(first); err != nil {
		t.Fatal(err)
	}
	first[0].RecommendedSemester = 2
	if err := save(first); err != nil {
		t.Fatal(err)
	}
	var sem int
	var source string
	if err := s.db.QueryRow(`SELECT recommended_semester,source FROM module_study_programs WHERE module_id='1'`).Scan(&sem, &source); err != nil {
		t.Fatal(err)
	}
	if sem != 2 || source != "verified_pdf_cells" {
		t.Fatalf("stale link: %d %s", sem, source)
	}
	invalid := []model.CurriculumModule{{ModuleID: "missing", ModuleName: "Bad", RecommendedSemester: 3}}
	if err := save(invalid); err == nil {
		t.Fatal("invalid link succeeded")
	}
	rows, err := s.GetProgramCurriculum("p")
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) != 1 || rows[0].RecommendedSemester != 2 {
		t.Fatalf("previous source was lost: %+v", rows)
	}
	if rows[0].SourceEvidence != first[0].SourceEvidence {
		t.Fatal("workload evidence was lost")
	}
	var layoutJSON string
	if err := s.db.QueryRow("SELECT layout_json FROM validated_curriculum_plans WHERE program_id='p'").Scan(&layoutJSON); err != nil || layoutJSON != `{"cells":[],"plan_names":{"p1t1":"Plan"}}` {
		t.Fatalf("plan metadata lost during rollback: %s %v", layoutJSON, err)
	}
	// A module assigned across two semesters must not collapse to the last one.
	first = append(first, model.CurriculumModule{ModuleID: "1", ModuleName: "Test", RecommendedSemester: 3, Credits: 2})
	if err := save(first); err != nil {
		t.Fatal(err)
	}
	if err := s.db.QueryRow(`SELECT recommended_semester FROM module_study_programs WHERE module_id='1'`).Scan(&sem); err != nil {
		t.Fatal(err)
	}
	if sem != 0 {
		t.Fatalf("invented single semester for multi-semester module: %d", sem)
	}
	// Removing the match must clear its stale semester while keeping membership.
	if err := save([]model.CurriculumModule{{ModuleName: "Elective", RecommendedSemester: 4, Credits: 6}}); err != nil {
		t.Fatal(err)
	}
	var count int
	if err := s.db.QueryRow(`SELECT COUNT(*) FROM module_study_programs WHERE module_id='1' AND recommended_semester IS NULL`).Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != 1 {
		t.Fatal("stale semester survived replacement")
	}
	first = first[:1]
	if err := save(first); err != nil {
		t.Fatal(err)
	}
	qis := []model.CurriculumModule{{ModuleCode: "1", ModuleName: "Test", RecommendedSemester: 5}}
	if err := s.SaveCurriculumModules("p", "Program", "Bachelor", "2024", qis, "qis_tree"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := s.MatchAndLinkCurriculumModules("p"); err != nil {
		t.Fatal(err)
	}
	if err := s.db.QueryRow(`SELECT recommended_semester FROM module_study_programs WHERE module_id='1'`).Scan(&sem); err != nil {
		t.Fatal(err)
	}
	if sem != 2 {
		t.Fatal("legacy/QIS linking overwrote a verified semester")
	}
	if err := s.SaveValidatedCurriculumModules("p", "Program", "Bachelor", "2024", first, "updated.pdf"); err != nil {
		t.Fatal(err)
	}
	rows, err = s.GetProgramCurriculum("p")
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) != 2 {
		t.Fatalf("expected one PDF row plus QIS, got %d", len(rows))
	}
	for _, row := range rows {
		if row.SourceFile == "test.pdf" {
			t.Fatal("old PDF survived replacement by effective plan")
		}
	}
	catalog, err := s.GetCurriculumCatalog()
	if err != nil || len(catalog) != 1 || catalog[0].Turnus != "jedes Sommersemester" {
		t.Fatalf("catalog: %+v %v", catalog, err)
	}
}
