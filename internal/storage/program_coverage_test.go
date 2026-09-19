package storage

import (
	"path/filepath"
	"testing"
)

func TestProgramCoverageView(t *testing.T) {
	s, err := NewStorage(filepath.Join(t.TempDir(), "t.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	for _, q := range []string{
		`INSERT INTO official_study_programs(id,program_name,degree,po_version) VALUES('a','A','Bachelor','2020'),('b','B','Bachelor','2020'),('c','C','Master','2020')`,
		`INSERT INTO validated_curriculum_plans(program_id,source_file,layout_json) VALUES('a','a.pdf','{}')`,
		`INSERT INTO modules(id,code,title_de) VALUES('m1','1','M')`,
		`INSERT INTO module_study_programs(module_id,program_id,program_name) VALUES('m1','b','B')`,
	} {
		if _, err := s.db.Exec(q); err != nil {
			t.Fatal(q, err)
		}
	}
	if err := s.SaveProgramScanStatus("c", "no_plan", "kein Studienplan", "c.pdf"); err != nil {
		t.Fatal(err)
	}
	got := map[string]string{}
	rows, err := s.db.Query(`SELECT program_id, level FROM program_coverage`)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	for rows.Next() {
		var id, level string
		if err := rows.Scan(&id, &level); err != nil {
			t.Fatal(err)
		}
		got[id] = level
	}
	if got["a"] != "plan" || got["b"] != "modules" || got["c"] != "none" {
		t.Fatalf("%v", got)
	}
}
