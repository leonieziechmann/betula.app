package catalogdb

import (
	"testing"

	"github.com/leonieziechmann/betula/internal/model"
)

func TestScanAdaptersReadProgramsDocumentsAndCatalog(t *testing.T) {
	db := openTestDB(t)
	_, err := db.SQL().Exec(`
		INSERT INTO program (id, slug, name, stg_code, abschl_code, degree_raw, degree_level, po_version, family_key, name_key, is_latest_po, source_url, fetched_at) VALUES
			('079-82-2008', 'bachelor-informatik-2008', 'Informatik', '079', '82', 'Bachelor (universitär)', 'bachelor', '2008 - 2. SÄ 2024', '079-82', 'informatik', 1, 'https://qis/po1', '2026-09-17T18:00:00Z'),
			('G17-88-2022', 'master-ki-2022', 'Künstliche Intelligenz Technologie', 'G17', '88', 'Master (universitär)', 'master', '2022', 'G17-88', 'ki', 1, 'https://qis/po2', '2026-09-17T18:00:00Z');
		INSERT INTO program_document (program_id, ord, title, doc_type, url) VALUES
			('079-82-2008', 1, 'Prüfungsordnung ABl. 12/2024', 'statute', 'https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf'),
			('079-82-2008', 2, 'Satzungsänderung ABl. 13/2021', 'amendment', 'https://opus4.kobv.de/opus4-btu/files/5511/13.pdf');
		INSERT INTO module (id, title, title_de, title_en, detail_status, offer_status, is_fues, credits, turnus_raw, duration_raw) VALUES
			('11881', 'Foundations of Data Mining', 'Grundlagen des Data Mining', 'Foundations of Data Mining', 'ok', 'active', 0, 6, 'Every summer semester', '1 semester'),
			('14037', 'Nur auf der FÜS-Liste', NULL, NULL, 'missing', 'active', 1, NULL, NULL, NULL);`)
	if err != nil {
		t.Fatal(err)
	}

	all, err := db.ScanPrograms("", "", "")
	if err != nil || len(all) != 2 {
		t.Fatalf("ScanPrograms = %d programs, err %v", len(all), err)
	}
	// Filters fold case in Go: SQLite would not match „künstliche" against „Künstliche".
	ki, _ := db.ScanPrograms("künstliche", "master", "")
	if len(ki) != 1 || ki[0].ID != "G17-88-2022" || len(ki[0].Documents) != 0 {
		t.Fatalf("filtered programs = %+v", ki)
	}
	one, _ := db.ScanPrograms("", "", "079-82-2008")
	if len(one) != 1 || one[0].ProgramName != "Informatik" || one[0].Degree != "Bachelor (universitär)" || one[0].POVersion != "2008 - 2. SÄ 2024" ||
		len(one[0].Documents) != 2 || one[0].Documents[0].DocType != "statute" || one[0].Documents[1].DocType != "amendment" {
		t.Fatalf("program by id = %+v", one)
	}

	catalog, err := db.ScanCatalog()
	if err != nil || len(catalog) != 2 {
		t.Fatalf("ScanCatalog = %+v, err %v", catalog, err)
	}
	if m := catalog[0]; m.ID != "11881" || m.Code != "11881" || m.TitleDE != "Grundlagen des Data Mining" || m.TitleEN != "Foundations of Data Mining" || m.Credits != 6 {
		t.Errorf("catalog module = %+v", m)
	}
	if m := catalog[1]; m.TitleDE != "Nur auf der FÜS-Liste" || m.Credits != 0 {
		t.Errorf("module without a page = %+v", m)
	}

	if has, _ := db.HasPlan("079-82-2008"); has {
		t.Error("HasPlan before any scan")
	}
	plan := PlanFromModules("079-82-2008", "statutes/Informatik/6707_12_Informatik_B.Sc.pdf", `{"pages":[7]}`, []model.CurriculumModule{
		{ModuleID: "11881", ModuleName: "Foundations of Data Mining", StartSemester: 5, EndSemester: 6, SemesterSpan: "5-6", ModuleType: "Wahlpflicht", SubjectArea: "Grundlagen der Informatik", SourceEvidence: `{"id":"p7t1r5c2"}`},
		{ModuleName: "Bachelor-Arbeit", RecommendedSemester: 6, Credits: 12, ModuleType: "Abschlussarbeit"},
	})
	if err := db.SavePlan(plan); err != nil {
		t.Fatalf("SavePlan failed: %v", err)
	}
	if has, _ := db.HasPlan("079-82-2008"); !has {
		t.Error("HasPlan after SavePlan = false")
	}
	var kind, area, evidence string
	err = db.SQL().QueryRow("SELECT kind, subject_area, source_evidence FROM plan_entry WHERE program_id = '079-82-2008' AND module_id = '11881'").Scan(&kind, &area, &evidence)
	if err != nil || kind != "elective" || area != "Grundlagen der Informatik" || evidence != `{"id":"p7t1r5c2"}` {
		t.Errorf("stored entry: kind %q area %q evidence %q (err %v)", kind, area, evidence, err)
	}
}

// The claims are the program's own evidence: its module pages and its tree. A
// membership that only a study plan asserted is left out, so the next scan
// cannot confirm its own link.
func TestScanProgramClaimsLeavesOutWhatOnlyThePlanAsserted(t *testing.T) {
	db := openTestDB(t)
	_, err := db.SQL().Exec(`
		INSERT INTO program (id, slug, name, stg_code, abschl_code, degree_raw, degree_level, po_version, family_key, name_key, is_latest_po, source_url, fetched_at) VALUES
			('079-82-2008', 'bachelor-informatik-2008', 'Informatik', '079', '82', 'Bachelor (universitär)', 'bachelor', '2008', '079-82', 'informatik', 1, 'https://qis/po1', '2026-09-17T18:00:00Z');
		INSERT INTO module (id, title, detail_status, offer_status, is_fues) VALUES
			('12102', 'Programmierpraktikum', 'ok', 'active', 0),
			('11787', 'Theoretische Informatik', 'ok', 'active', 0),
			('11122', 'Bachelor-Arbeit', 'ok', 'active', 0);
		INSERT INTO program_module (program_id, module_id, relation, in_tree, on_module_page, in_plan) VALUES
			('079-82-2008', '12102', 'curricular', 1, 1, 0),
			('079-82-2008', '11787', 'curricular', 0, 1, 0),
			('079-82-2008', '11122', 'curricular', 0, 0, 1);`)
	if err != nil {
		t.Fatal(err)
	}
	claims, err := db.ScanProgramClaims()
	if err != nil {
		t.Fatal(err)
	}
	if got := claims["079-82-2008"]; len(got) != 2 || !got["12102"] || !got["11787"] || got["11122"] {
		t.Errorf("claims = %v", got)
	}
	if _, ok := claims["247-82-2016"]; ok {
		t.Error("a program without modules must not appear")
	}
}
