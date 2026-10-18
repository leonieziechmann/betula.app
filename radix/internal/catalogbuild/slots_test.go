package catalogbuild

import (
	"bufio"
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	_ "modernc.org/sqlite"
)

// TestSlotWords holds distinctive and fit to testdata/slot_words.tsv, which Folia's tests hold
// plans::plan to as well: the two ports of the rule read a name alike.
func TestSlotWords(t *testing.T) {
	f, err := os.Open("testdata/slot_words.tsv")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	lines := 0
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		lines++
		fields := strings.Split(line, "\t")
		switch {
		case fields[0] == "words" && len(fields) == 3:
			got := strings.Join(distinctive(fields[1]), " ")
			if got == "" {
				got = "∅"
			}
			if got != fields[2] {
				t.Errorf("distinctive(%q) = %q, want %q", fields[1], got, fields[2])
			}
		case fields[0] == "fit" && len(fields) == 4:
			want, err := strconv.Atoi(fields[3])
			if err != nil {
				t.Fatalf("testdata/slot_words.tsv: %q", line)
			}
			if got := fit(distinctive(fields[1]), distinctive(fields[2])); got != want {
				t.Errorf("fit(%q, %q) = %d, want %d", fields[1], fields[2], got, want)
			}
		default:
			t.Fatalf("testdata/slot_words.tsv: %q is neither words<TAB>name<TAB>words nor fit<TAB>row<TAB>name<TAB>score", line)
		}
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	if lines < 20 {
		t.Fatalf("testdata/slot_words.tsv: only %d cases", lines)
	}
}

// informatikAreas is the tree of Informatik B.Sc. (PO 2008) as the snapshot of 2026-10-18 has it,
// with the areas a student chooses in (folia plans::area_fixtures::INFORMATIK_BSC).
func informatikAreas() []slotArea {
	area := func(id int64, label string, choice bool, modules int, ancestors ...string) slotArea {
		return slotArea{id: id, label: label, path: strings.Join(append(append([]string{}, ancestors...), label), " / "), ancestors: ancestors, modules: modules, choice: choice}
	}
	return []slotArea{
		area(1, "Komplex Informatik", false, 9, "Grundstudium"),
		area(2, "Proseminar oder Praktikum", true, 3, "Grundstudium", "Komplex Informatik"),
		area(3, "Komplex Mathematik", false, 3, "Grundstudium"),
		area(4, "Wahlpflichtmodule Praktische Mathematik", true, 17, "Grundstudium", "Komplex Nebenfach"),
		area(5, "Mathematik", true, 23, "Grundstudium", "Komplex Nebenfach"),
		area(6, "Physik", true, 4, "Grundstudium", "Komplex Nebenfach"),
		area(7, "Maschinenbau / Elektrotechnik", true, 16, "Grundstudium", "Komplex Nebenfach"),
		area(8, "Wirtschaftswissenschaften", true, 9, "Grundstudium", "Komplex Nebenfach"),
		area(9, "Bauingenieurwesen", true, 7, "Grundstudium", "Komplex Nebenfach"),
		area(10, "Grundlagen der Informatik", true, 7, "Fachstudium"),
		area(11, "Praktische Informatik", true, 10, "Fachstudium"),
		area(12, "Angewandte und Technische Informatik", true, 9, "Fachstudium"),
		area(13, "Seminar oder Praktikum aus der Informatik", true, 6, "Fachstudium"),
	}
}

// TestAreasForRowInformatik: the slots of Informatik B.Sc.'s plan point where Folia's do
// (area_survey on the snapshot of 2026-10-18).
func TestAreasForRowInformatik(t *testing.T) {
	areas := informatikAreas()
	names := []string{"Proseminar oder Praktikum", "Modul aus dem Bereich Praktische Mathematik", "Anwendungsfach", "Fachübergreifendes Studium",
		"Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik",
		"Komplex Praktische Informatik", "Seminar oder Praktikum"}
	var rows []slotRow
	for _, name := range names {
		rows = append(rows, slotRow{name: name, kind: kindElective})
	}
	rows = append(rows, slotRow{moduleID: "11101", name: "Lineare Algebra und analytische Geometrie I", kind: kindCompulsory})
	labels := func(found []slotArea) string {
		var l []string
		for _, area := range found {
			l = append(l, area.label)
		}
		return strings.Join(l, " | ")
	}
	for _, c := range []struct{ row, want string }{
		{"Proseminar oder Praktikum", "Proseminar oder Praktikum"},
		{"Modul aus dem Bereich Praktische Mathematik", "Wahlpflichtmodule Praktische Mathematik"},
		// A name above the leaves: the areas below it, but not what another row names alone.
		{"Anwendungsfach", "Mathematik | Maschinenbau / Elektrotechnik | Wirtschaftswissenschaften | Bauingenieurwesen | Physik"},
		{"Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik",
			"Grundlagen der Informatik | Praktische Informatik | Angewandte und Technische Informatik"},
		{"Komplex Praktische Informatik", "Praktische Informatik"},
		{"Seminar oder Praktikum", "Seminar oder Praktikum aus der Informatik"},
	} {
		if got := labels(areasForRow(slotRow{name: c.row, kind: kindElective}, "", areas, rows)); got != c.want {
			t.Errorf("%q → %q, want %q", c.row, got, c.want)
		}
	}
	if !isFUESRow(slotRow{name: "Fachübergreifendes Studium", kind: kindElective}) || isFUESRow(slotRow{name: "Anwendungsfach", kind: kindElective}) {
		t.Error("isFUESRow")
	}
}

// TestSlotKinds: a module of a Wahlpflicht slot that no source gives a kind is Wahlpflicht,
// inferred from the plan; a stated kind stays, a Pflicht slot and a FÜS row infer nothing, and
// the plan does not name the module (in_plan 0).
func TestSlotKinds(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "slots.db"))
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
	for _, p := range []string{"079-82-2008", "966-88-2021"} {
		exec(`INSERT INTO program (id, slug, name, stg_code, abschl_code, degree_raw, degree_level, po_version, family_key, name_key, is_latest_po, source_url, fetched_at)
			VALUES (?, ?, 'P', 's', 'a', 'Bachelor', 'bachelor', '2008', ?, ?, 1, 'u', 'now')`, p, "slug-"+p, p, p)
		exec(`INSERT INTO plan (program_id, source_file, layout_json, validated_at) VALUES (?, 'po.pdf', '{}', 'now')`, p)
	}
	for _, m := range []string{"11101", "12111", "11102", "11865", "11908", "35305", "16001", "16002"} {
		exec(`INSERT INTO module (id, title, detail_status, offer_status, is_fues) VALUES (?, ?, 'ok', 'active', 0)`, m, "Modul "+m)
	}
	areas := []struct {
		id     int64
		parent any
		label  string
	}{
		{1, nil, "Grundstudium"}, {2, int64(1), "Komplex Informatik"}, {3, int64(2), "Proseminar oder Praktikum"},
		{4, int64(1), "Komplex Nebenfach"}, {5, int64(4), "Wahlpflichtmodule Praktische Mathematik"},
		{6, int64(4), "Physik"}, {7, int64(4), "Maschinenbau / Elektrotechnik"},
	}
	for i, a := range areas {
		exec(`INSERT INTO program_area (id, program_id, parent_id, ord, depth, label, path, source_url) VALUES (?, '079-82-2008', ?, ?, 1, ?, ?, 'u')`, a.id, a.parent, i, a.label, a.label)
	}
	exec(`INSERT INTO program_area (id, program_id, parent_id, ord, depth, label, path, source_url) VALUES (20, '966-88-2021', NULL, 0, 1, 'Minor Subject', 'Minor Subject', 'u')`)
	exec(`INSERT INTO program_area (id, program_id, parent_id, ord, depth, label, path, source_url) VALUES (21, '966-88-2021', NULL, 1, 1, 'Fachübergreifende Module', 'Fachübergreifende Module', 'u')`)
	tree := func(program, module string, area int64, kind any) {
		basis := any(nil)
		if kind != nil {
			basis = "stated"
		}
		exec(`INSERT INTO program_module_assertion (program_id, module_id, source, area_id, kind, kind_basis) VALUES (?, ?, 'qis_tree', ?, ?, ?)`, program, module, area, kind, basis)
	}
	tree("079-82-2008", "11101", 2, nil)
	tree("079-82-2008", "12111", 3, nil)        // Proseminar: nothing states a kind
	tree("079-82-2008", "11102", 5, "elective") // the tree's label says Wahlpflicht
	tree("079-82-2008", "11865", 6, nil)        // Physik: nothing states a kind
	tree("079-82-2008", "11908", 7, nil)        // Systemtheorie I: nothing states a kind
	tree("079-82-2008", "35305", 7, nil)        // its page states Pflicht (below)
	tree("966-88-2021", "16001", 20, nil)       // a Pflicht slot: nothing to infer
	tree("966-88-2021", "16002", 21, nil)       // a FÜS row: its modules are the FÜS list
	exec(`INSERT INTO program_module_assertion (program_id, module_id, source, kind, kind_basis) VALUES ('079-82-2008', '35305', 'module_page', 'compulsory', 'stated')`)
	exec(`INSERT INTO program_module_assertion (program_id, module_id, source, kind, kind_basis) VALUES ('079-82-2008', '11101', 'pdf_plan', 'compulsory', 'stated')`)
	plan := func(program string, ord int, module any, name, kind string) {
		exec(`INSERT INTO plan_entry (program_id, ord, module_id, module_name, kind) VALUES (?, ?, ?, ?, ?)`, program, ord, module, name, kind)
	}
	plan("079-82-2008", 1, "11101", "Lineare Algebra", "compulsory")
	plan("079-82-2008", 2, nil, "Proseminar oder Praktikum", "elective")
	plan("079-82-2008", 3, nil, "Modul aus dem Bereich Praktische Mathematik", "elective")
	plan("079-82-2008", 4, nil, "Anwendungsfach", "elective")
	plan("966-88-2021", 1, nil, "Minor Subject (Anlage 1b)", "compulsory")
	plan("966-88-2021", 2, nil, "Fachübergreifendes Studium", "elective")

	b := &builder{tx: tx, report: &Report{Assertions: map[string]int{}}}
	if err := b.writeSlotKinds(); err != nil {
		t.Fatal(err)
	}
	if b.report.Assertions["pdf_plan"] != 3 {
		t.Errorf("%d inferred statements, want 3", b.report.Assertions["pdf_plan"])
	}
	rows, err := tx.Query(`SELECT program_id, module_id, IFNULL(kind, '∅'), IFNULL(kind_source, '∅'), IFNULL(kind_basis, '∅'), IFNULL(precedence, '∅'), in_plan
		FROM v_program_module_src ORDER BY program_id, module_id`)
	if err != nil {
		t.Fatal(err)
	}
	var got []string
	for rows.Next() {
		var program, module, kind, source, basis, precedence string
		var inPlan int
		if err := rows.Scan(&program, &module, &kind, &source, &basis, &precedence, &inPlan); err != nil {
			t.Fatal(err)
		}
		got = append(got, fmt.Sprintf("%s %s %s %s %s %s %d", program, module, kind, source, basis, precedence, inPlan))
	}
	rows.Close()
	want := []string{
		"079-82-2008 11101 compulsory pdf_plan stated 1 1",
		"079-82-2008 11102 elective qis_tree stated 3 0",
		"079-82-2008 11865 elective pdf_plan inferred 4 0",
		"079-82-2008 11908 elective pdf_plan inferred 4 0",
		"079-82-2008 12111 elective pdf_plan inferred 4 0",
		"079-82-2008 35305 compulsory module_page stated 2 0",
		"966-88-2021 16001 ∅ ∅ ∅ ∅ 0",
		"966-88-2021 16002 ∅ ∅ ∅ ∅ 0",
	}
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Errorf("v_program_module_src\n got: %q\nwant: %q", got, want)
	}
}

// TestSlotKindsOnSnapshot runs the slot rule on a real catalog, so that a change of it shows
// what it moves; it runs only with a snapshot:
//
//	RADIX_SLOT_SNAPSHOT=/path/to/catalog-*.db go test ./internal/catalogbuild -run TestSlotKindsOnSnapshot -v
//
// With RADIX_SLOT_ROWS=<file> it writes every Wahlpflicht slot and the areas it is filled
// from, one line each („<slug>\t<row>\t<caption>\t<path> | <path>"), as Folia's
// `cargo run -p folia-pages --example area_survey` prints them, for a diff of the two ports.
func TestSlotKindsOnSnapshot(t *testing.T) {
	path := os.Getenv("RADIX_SLOT_SNAPSHOT")
	if path == "" {
		t.Skip("RADIX_SLOT_SNAPSHOT not set")
	}
	db, err := sql.Open("sqlite", "file:"+filepath.ToSlash(path)+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	slugs := make(map[string]string)
	rows, err := db.Query("SELECT id, slug FROM program")
	if err != nil {
		t.Fatal(err)
	}
	for rows.Next() {
		var id, slug string
		if err := rows.Scan(&id, &slug); err != nil {
			t.Fatal(err)
		}
		slugs[id] = slug
	}
	rows.Close()

	var lines []string
	seen := make(map[string]bool)
	statements, err := slotKinds(db, func(program string, row slotRow, areas []slotArea) {
		key := program + "\t" + row.name + "\t" + row.specialization
		if seen[key] {
			return
		}
		seen[key] = true
		paths := make([]string, len(areas))
		for i, area := range areas {
			paths[i] = area.path
		}
		found := strings.Join(paths, " | ")
		if found == "" {
			found = "—"
		}
		lines = append(lines, fmt.Sprintf("%s\t%s\t%s\t%s", slugs[program], row.name, row.specialization, found))
	})
	if err != nil {
		t.Fatal(err)
	}
	if out := os.Getenv("RADIX_SLOT_ROWS"); out != "" {
		sort.Strings(lines)
		f, err := os.Create(out)
		if err != nil {
			t.Fatal(err)
		}
		w := bufio.NewWriter(f)
		for _, line := range lines {
			fmt.Fprintln(w, line)
		}
		if err := w.Flush(); err != nil {
			t.Fatal(err)
		}
		f.Close()
	}

	programs := make(map[string]int)
	for _, s := range statements {
		programs[s.program]++
	}
	t.Logf("%d Wahlpflicht slots, %d modules without a kind become Wahlpflicht in %d programs", len(lines), len(statements), len(programs))

	// The case of 2026-10-18: Systemtheorie I and II in Informatik B.Sc., the Anwendungsfach.
	for _, module := range []string{"11908", "11909"} {
		found := false
		for _, s := range statements {
			if s.program == "079-82-2008" && s.module == module && s.row == "Anwendungsfach" {
				found = true
			}
		}
		if !found {
			t.Errorf("Informatik B.Sc.: %s is not inferred from the Anwendungsfach", module)
		}
	}
}
