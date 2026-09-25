package catalogbuild

import (
	"context"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
)

// Rooms get their short form next to the full name, in the schedule and in the exams.
func TestRoomsGetAShortForm(t *testing.T) {
	db, r := buildFixture(t)
	want(t, db, `SELECT event_id, room_short, room FROM v_module_schedule`,
		"120285|SFB 14.117|Allgemeine Elektrotechnik Labor - 14.117 - Campus Senftenberg")
	want(t, db, `SELECT event_id, room_short, room FROM v_module_exam`,
		"120286|LG1A 0.22|Lehrgebäude 1A - 0.22 - Zentralcampus")
	if len(r.RoomsUnknownBuilding) != 0 || len(r.RoomShortCollisions) != 0 {
		t.Errorf("unknown buildings %v, collisions %v", r.RoomsUnknownBuilding, r.RoomShortCollisions)
	}
}

// Two rooms that would share a short form both keep their long form, and the report names them.
func TestRoomsThatWouldShareAShortFormKeepTheirLongForm(t *testing.T) {
	db, _ := buildFixture(t)
	tx, err := db.SQL().Begin()
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.Exec(`INSERT INTO event_date (event_id, ord, room) VALUES
		('120285', 2, 'Lehrgebäude 1A - 0.22 Labor - Zentralcampus'),
		('120285', 3, 'Neubau X - 1.01 - Zentralcampus')`); err != nil {
		t.Fatal(err)
	}
	b := &builder{tx: tx, report: &Report{RoomsUnknownBuilding: map[string]int{}}}
	if err := b.writeRoomShorts(); err != nil {
		t.Fatal(err)
	}
	if got, want := b.report.RoomShortCollisions, []string{"Lehrgebäude 1A - 0.22 - Zentralcampus", "Lehrgebäude 1A - 0.22 Labor - Zentralcampus"}; !reflect.DeepEqual(got, want) {
		t.Errorf("collisions = %q, want %q", got, want)
	}
	if got := b.report.RoomsUnknownBuilding; len(got) != 1 || got["Neubau X - 1.01 - Zentralcampus"] != 1 {
		t.Errorf("unknown buildings = %v", got)
	}
	rows, err := tx.Query("SELECT room_short FROM event_date ORDER BY room")
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	var got []string
	for rows.Next() {
		var s string
		if err := rows.Scan(&s); err != nil {
			t.Fatal(err)
		}
		got = append(got, s)
	}
	if want := []string{"SFB 14.117", "Lehrgebäude 1A - 0.22", "Lehrgebäude 1A - 0.22 Labor", "Neubau X 1.01"}; !reflect.DeepEqual(got, want) {
		t.Errorf("room_short = %q, want %q", got, want)
	}
}

// Every module has an abbreviation, and every module of a program one that is unique there.
// The owner's example comes out of the build: „Algorithmieren und Programmieren“ is AuP.
func TestModulesGetAnAbbreviation(t *testing.T) {
	db, _ := buildFixture(t)
	want(t, db, `SELECT id, abbrev FROM v_module ORDER BY id`, // LAl: LiA would read as „L in A“
		"11101|LAl", "11152|ERP", "11881|FDM", "12999|BA", "13000|VeM", "14037|NFL")
	want(t, db, `SELECT COUNT(*) FROM v_program_module WHERE abbrev IS NULL`, "0")

	// A new module of Informatik B.Sc.: the list names it, and its page assigns it.
	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 19, 15, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatal(err)
		}
	}
	list, err := db.GetPage(catalogdb.SourceModuleCatalog, "list")
	if err != nil {
		t.Fatal(err)
	}
	put(catalogdb.SourceModuleCatalog, "list", strings.Replace(string(list.Body), "</tbody>",
		`<tr><td class="moduleNumber"><a href="/modul/12101">12101</a></td><td class="title">Algorithmieren und Programmieren</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/12102">12102</a></td><td class="title">Algorithmen und Programme</td></tr></tbody>`, 1))
	for _, m := range [][2]string{{"12101", "Algorithmieren und Programmieren"}, {"12102", "Algorithmen und Programme"}} {
		put(catalogdb.SourceModulePage, m[0], modulePageDE(m[0], m[1], `
			<tr><td>Zuordnung zu Studiengängen:</td><td><ul><li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>`))
	}
	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	// The owner's AuP is 12101's; 12102 would derive it too, but the form is reserved for
	// the title the override line names, so AlP is its first candidate.
	want(t, db, `SELECT module_id, abbrev FROM v_program_module WHERE program_id = '079-82-2008' AND module_id IN ('12101', '12102') ORDER BY 1`,
		"12101|AuP", "12102|AlP")
	want(t, db, `SELECT module_id, is_override, choice, is_twin FROM program_module_abbrev WHERE module_id IN ('12101', '12102') ORDER BY 1`,
		"12101|1|1|0", "12102|0|1|0")
	want(t, db, `SELECT COUNT(*) FROM (SELECT 1 FROM program_module_abbrev GROUP BY program_id, abbrev COLLATE NOCASE HAVING COUNT(*) > 1)`, "0")
	if report.AbbrevFellBack != 0 || strings.Contains(strings.Join(report.AbbrevOverridesUnused, " "), "12101") {
		t.Errorf("fell back %d, unused overrides %v", report.AbbrevFellBack, report.AbbrevOverridesUnused)
	}
	if report.AbbrevChanged != 0 {
		t.Errorf("%d pairs changed; new modules are no change", report.AbbrevChanged)
	}

	// build.finished counts the pairs whose form moved since the build before.
	if _, err := db.SQL().Exec("UPDATE program_module_abbrev SET abbrev = 'Zz' WHERE module_id = '12101'"); err != nil {
		t.Fatal(err)
	}
	if report, err = Build(context.Background(), db); err != nil {
		t.Fatal(err)
	}
	if report.AbbrevChanged != 1 {
		t.Errorf("%d pairs changed, want 1", report.AbbrevChanged)
	}
}

// A database without short names fails validate, so that it is never exported: a migrated
// database that was not built again.
func TestValidateWantsShortNames(t *testing.T) {
	db, _ := buildFixture(t)
	for _, stmt := range []string{
		"UPDATE event_date SET room_short = NULL WHERE event_id = '120285'",
		"DELETE FROM module_abbrev WHERE module_id = '11101'",
		"DELETE FROM program_module_abbrev WHERE module_id = '11881'",
		"UPDATE module_abbrev SET abbrev = 'E R P' WHERE module_id = '11152'",
		"UPDATE module_abbrev SET abbrev = 'KKK', is_override = 0 WHERE module_id = '11881'",
		// two modules of one program: N-FL and NFL
		`UPDATE program_module_abbrev SET abbrev = 'N-FL' WHERE (program_id, module_id) = (SELECT program_id, MIN(module_id)
		 FROM program_module_abbrev GROUP BY program_id HAVING COUNT(*) > 1 ORDER BY program_id LIMIT 1)`,
		`UPDATE program_module_abbrev SET abbrev = 'NFL' WHERE (program_id, module_id) = (SELECT program_id, MAX(module_id)
		 FROM program_module_abbrev GROUP BY program_id HAVING COUNT(*) > 1 ORDER BY program_id LIMIT 1)`,
	} {
		if _, err := db.SQL().Exec(stmt); err != nil {
			t.Fatal(err)
		}
	}
	checks, err := db.Validate(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	failed := map[string]bool{}
	for _, c := range checks {
		if c.Status == catalogdb.StatusFail {
			failed[c.Name] = true
		}
	}
	for _, name := range []string{
		"every event date with a room has a short form",
		"every module has an abbreviation",
		"every module of a program has an abbreviation",
		"abbreviations are 2 to 10 characters without spaces",
		"no derived abbreviation is on the blocked list",
		"abbreviations are unique within a program", // N-FL and NFL read as one
	} {
		if !failed[name] {
			t.Errorf("expected check %q to fail; failed = %v", name, failed)
		}
	}
}
