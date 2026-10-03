package catalogbuild

import (
	"context"
	"fmt"
	"path/filepath"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// semesterFixture builds a catalog whose modules have a dated teaching event in
// the given semester, and returns the semester the build calls the current one.
func semesterFixture(t *testing.T, semester string, modules int) string {
	t.Helper()
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "semester.db"))
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 21, 0, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage: %v", err)
		}
	}

	// One module per event, so the number of modules with a schedule is the number
	// of events: that is what the threshold counts.
	list := `<table><tbody class="list">`
	for i := 0; i < modules; i++ {
		id := fmt.Sprintf("2%04d", i)
		list += fmt.Sprintf(`<tr><td class="moduleNumber"><a href="/modul/%s">%s</a></td><td class="title">Modul %s</td></tr>`, id, id, id)
		put(catalogdb.SourceModulePage, id, modulePageDE(id, "Modul "+id, fmt.Sprintf(
			`<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul><li><a href="/rds?veranstaltung.veranstid=9%04d">Vorlesung</a></li></ul></td></tr>`, i)))
		put(catalogdb.SourceQISEvent, fmt.Sprintf("9%04d", i), eventPageOfSemester(semester))
	}
	put(catalogdb.SourceModuleCatalog, "list", list+`</tbody></table>`)

	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build: %v", err)
	}
	var current string
	if err := db.SQL().QueryRow("SELECT value FROM meta WHERE key = 'current_semester'").Scan(&current); err != nil {
		t.Fatalf("read current_semester: %v", err)
	}
	return current
}

// eventPageOfSemester is a lecture with dates inside the given semester.
func eventPageOfSemester(semester string) string {
	label, dates := "SS 2026", "14.04.2026 bis 21.07.2026"
	if semester == "2026W" {
		label, dates = "WS 2026/27", "05.10.2026 bis 25.01.2027"
	}
	return `<html><body><h1>Vorlesung - Einzelansicht</h1>
	<table summary="Grunddaten zur Veranstaltung"><tr><th>Veranstaltungsart</th><td>Vorlesung</td><th>Semester</th><td>` + label + `</td></tr></table>
	<table summary="Übersicht über alle Veranstaltungstermine"><caption>Termine Gruppe: 1</caption>
		<tr><th>Tag</th><th>Zeit</th><th>Rhythmus</th><th>Dauer</th><th>Raum</th></tr>
		<tr><td>Di.</td><td>09:15 bis 10:45</td><td>A/B</td><td>` + dates + `</td><td><a href="#">Hauptgebäude - HG 3.45 - Zentralcampus</a></td></tr></table>
	</body></html>`
}

// BTU publishes the next semester module by module over weeks. A handful of early
// events is not a published schedule: on 2026-09-19 exactly one winter event
// existed, and it must not move the catalog out of the running semester.
func TestAFewEarlyEventsDoNotMoveTheSemester(t *testing.T) {
	if got := semesterFixture(t, "2026W", 5); got != currentSemesterKey(time.Now()) {
		t.Errorf("current_semester = %q, want the calendar semester %q", got, currentSemesterKey(time.Now()))
	}
}

// Once the schedule of the next semester is out, students plan with it, so the
// catalog presents it even while the old semester runs out.
func TestAPublishedScheduleMovesTheSemesterForward(t *testing.T) {
	if got := semesterFixture(t, "2026W", minModulesOfAPublishedSchedule); got != "2026W" {
		t.Errorf("current_semester = %q, want 2026W once %d modules have a winter schedule", got, minModulesOfAPublishedSchedule)
	}
}

// It never moves back: a semester that has ended stays behind the calendar.
func TestThePastNeverBecomesTheCurrentSemester(t *testing.T) {
	got := semesterFixture(t, "2026S", minModulesOfAPublishedSchedule)
	if want := currentSemesterKey(time.Now()); got != want {
		t.Errorf("current_semester = %q, want %q", got, want)
	}
}
