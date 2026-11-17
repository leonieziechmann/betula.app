package catalogbuild

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// TestFUESListIsWhatThePageAdmits: a FÜS module is on a program's FÜS list where its page admits
// the program and it is not curricular there, and nowhere else. One whose page admits no program
// („keine Zuordnung vorhanden", only „Abschluss im Ausland", or no page) is on none: its page names
// none because the FÜS catalog of the current semester does not hold it, as „Einführung in die
// Amateurfunktechnik" in the winter (migration 0022, which withdraws 0021's rule).
func TestFUESListIsWhatThePageAdmits(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "fues.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	tx, err := db.SQL().BeginTx(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	exec := func(query string, args ...any) {
		t.Helper()
		if _, err := tx.Exec(query, args...); err != nil {
			t.Fatalf("%v\n%s", err, query)
		}
	}

	// Informatik B.Sc. and M.Sc. have a FÜS list (ERP's page admits both); Power Engineering has none.
	bsc, msc, power := "079-82-2008", "079-88-2008", "853-88-2016"
	for _, p := range []string{bsc, msc, power} {
		exec(`INSERT INTO program (id, slug, name, stg_code, abschl_code, degree_raw, degree_level, po_version, family_key, name_key, is_latest_po, source_url, fetched_at)
			VALUES (?, ?, 'P', 's', 'a', 'Bachelor', 'bachelor', '2008', ?, ?, 1, 'u', 'now')`, p, "slug-"+p, p, p)
	}
	for _, m := range []struct {
		id    string
		title string
		fues  int
	}{
		{"11101", "Lineare Algebra", 0},
		{"11152", "ERP", 1},
		{"12045", "Einführung in die Amateurfunktechnik", 1},
		{"12396", "Thermische Systembetrachtungen", 1},
		{"13999", "Nur für einen unbekannten Studiengang", 1},
		{"14037", "Nur auf der FÜS-Liste", 1},
		{"14077", "Studierstrategien & Fachdeutsch Architektur und Stadtplanung", 1},
	} {
		exec(`INSERT INTO module (id, title, detail_status, offer_status, is_fues) VALUES (?, ?, 'ok', 'active', ?)`, m.id, m.title, m.fues)
	}
	exec(`UPDATE module SET detail_status = 'missing' WHERE id = '14037'`) // no page at all

	ref := func(module string, ord int, degree, program string, programID any, status string) {
		exec(`INSERT INTO module_program_ref (module_id, ord, degree_raw, program_raw, po_raw, program_id, resolve_status) VALUES (?, ?, ?, ?, 'PO 2008', ?, ?)`,
			module, ord, degree, program, programID, status)
		if status == "resolved" {
			exec(`INSERT INTO program_module_assertion (program_id, module_id, source) VALUES (?, ?, 'module_page')`, programID, module)
		}
	}
	ref("11152", 1, "Bachelor (universitär)", "Informatik", bsc, "resolved")
	ref("11152", 2, "Master (universitär)", "Informatik", msc, "resolved")
	ref("12396", 1, "Master (universitär)", "Informatik", msc, "resolved") // its page names the program it is curricular in, no other
	ref("13999", 1, "Master (universitär)", "Astrologie", nil, "unresolved")
	ref("14077", 1, "Abschluss im Ausland", "Architektur", nil, "abroad")
	// 12045: „keine Zuordnung vorhanden", no line at all

	exec(`INSERT INTO program_area (id, program_id, parent_id, ord, depth, label, path, source_url) VALUES (1, ?, NULL, 0, 1, 'Fachstudium', 'Fachstudium', 'u')`, msc)
	exec(`INSERT INTO program_area (id, program_id, parent_id, ord, depth, label, path, source_url) VALUES (2, ?, NULL, 0, 1, 'Pflichtmodule', 'Pflichtmodule', 'u')`, power)
	exec(`INSERT INTO program_module_assertion (program_id, module_id, source, area_id) VALUES (?, '12045', 'qis_tree', 1)`, msc) // the M.Sc.'s own module
	exec(`INSERT INTO program_module_assertion (program_id, module_id, source, area_id) VALUES (?, '12396', 'qis_tree', 1)`, msc)
	exec(`INSERT INTO program_module_assertion (program_id, module_id, source, area_id) VALUES (?, '11101', 'qis_tree', 2)`, power)

	rows, err := tx.Query(`SELECT program_id, module_id, relation, IFNULL(kind, '∅'), IFNULL(kind_source, '∅'), IFNULL(area, '∅'), in_tree, on_module_page, in_plan
		FROM v_program_module_src ORDER BY program_id, module_id`)
	if err != nil {
		t.Fatal(err)
	}
	var got []string
	for rows.Next() {
		var program, module, relation, kind, source, area string
		var inTree, onPage, inPlan int
		if err := rows.Scan(&program, &module, &relation, &kind, &source, &area, &inTree, &onPage, &inPlan); err != nil {
			t.Fatal(err)
		}
		got = append(got, fmt.Sprintf("%s %s %s %s %s %s %d %d %d", program, module, relation, kind, source, area, inTree, onPage, inPlan))
	}
	rows.Close()
	want := []string{
		"079-82-2008 11152 fues fues ∅ ∅ 0 1 0",
		"079-88-2008 11152 fues fues ∅ ∅ 0 1 0",
		"079-88-2008 12045 curricular ∅ ∅ Fachstudium 1 0 0",
		"079-88-2008 12396 curricular ∅ ∅ Fachstudium 1 1 0",
		"853-88-2016 11101 curricular ∅ ∅ Pflichtmodule 1 0 0",
	}
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Errorf("v_program_module_src\n got: %q\nwant: %q", got, want)
	}
}

// What validate reports is what the pages state: in the fixture 14037, known from the FÜS list
// alone, while ERP's page admits both programs.
func TestValidateCountsFUESModulesWithoutPrograms(t *testing.T) {
	db, _ := buildFixture(t)
	if c := validated(t, db)["FÜS modules whose page admits no program"]; c.Status != catalogdb.StatusInfo || c.Value != 1 {
		t.Errorf("%+v", c)
	}
}
