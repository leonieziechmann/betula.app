package catalogbuild

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/parser"
)

// fixture is a real answer of QIS, kept with the parsers (internal/parser/testdata).
func fixture(t *testing.T, name string) []byte {
	t.Helper()
	body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", name))
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	return body
}

// entries returns a function that gives the markup of an event's entry in the answer of the
// event search among the fixtures.
func entries(t *testing.T) func(id string) string {
	t.Helper()
	list, err := parser.SplitEventList(bytes.NewReader(fixture(t, "qis_event_list.html")))
	if err != nil {
		t.Fatalf("SplitEventList: %v", err)
	}
	return func(id string) string {
		t.Helper()
		for _, e := range list.Entries {
			if e.ID == id {
				return string(e.HTML)
			}
		}
		t.Fatalf("no entry for %s", id)
		return ""
	}
}

// The event search states the dates of an event every night; the page of the event adds
// the remarks of its dates and the campus of its rooms. Where the two agree the page is
// used, where they differ the newer one. The fixtures are real answers of QIS, kept with
// the parsers (internal/parser/testdata).
func TestBuildReadsEventsFromTheListAndThePage(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "events.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	entry := entries(t)
	pageURL := func(id string) string {
		return "https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=" + id
	}
	night := time.Date(2026, 9, 24, 1, 0, 0, 0, time.UTC)
	put := func(source, key, body string, at time.Time) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: pageURL(key), HTTPStatus: 200, Body: []byte(body), FetchedAt: at}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceQISModuleList, "rows-000000", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>13921</td><td><a href="/rds?pord.pordnr=9001">Lightweight Design and Construction</a></td><td>Englisch</td><td>6</td><td></td><td></td></tr>
	</table>`, night)
	put(catalogdb.SourceQISModulePage, "13921", qisModulePage("13921", "Lightweight Design and Construction", "6",
		qisEventLink("151102", "Vorlesung")+qisEventLink("147988", "Vorlesung/Übung")+qisEventLink("151278", "Konsultation")+
			qisEventLink("152864", "Vorlesung")+qisEventLink("151296", "Übung/Praktikum")), night)

	// The list confirms the page of 151102: the page is used, remark and campus included.
	put(catalogdb.SourceQISEvent, "151102", string(fixture(t, "qis_event_151102.html")), night.Add(-72*time.Hour))
	put(catalogdb.SourceQISEventEntry, "151102", entry("151102"), night)
	// 147988 has no page yet: its dates come from the list, and the room 352 gets the name
	// the page of 151102 gives it.
	put(catalogdb.SourceQISEventEntry, "147988", entry("147988"), night)
	// The list moved a date of 151278 after its page was fetched: the list wins.
	put(catalogdb.SourceQISEvent, "151278", string(fixture(t, "qis_event_151278.html")), night.Add(-72*time.Hour))
	put(catalogdb.SourceQISEventEntry, "151278", strings.Replace(entry("151278"), "07:30", "08:00", 1), night)
	// The page of 152864 is newer than the list: the page wins.
	put(catalogdb.SourceQISEventEntry, "152864", strings.Replace(entry("152864"), "16:30", "17:00", 1), night)
	put(catalogdb.SourceQISEvent, "152864", string(fixture(t, "qis_event_152864.html")), night.Add(time.Hour))
	// The page of 151296 was fetched after its entry last changed, and the entry was read
	// again since, unchanged. What counts is the change: the page stays, although it differs.
	moved := strings.Replace(entry("151296"), "07:30", "08:15", 1)
	put(catalogdb.SourceQISEventEntry, "151296", moved, night.Add(-2*time.Hour))
	put(catalogdb.SourceQISEvent, "151296", string(fixture(t, "qis_event_151296.html")), night.Add(-time.Hour))
	put(catalogdb.SourceQISEventEntry, "151296", moved, night)

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	if report.Events != 5 || report.EventsFromList != 2 {
		t.Errorf("events = %d, from the list %d; want 5 and 2", report.Events, report.EventsFromList)
	}

	want(t, db, "SELECT start_time, room, campus, comment FROM event_date WHERE event_id = '151102' ORDER BY ord",
		"10:00|Lehrgebäude 3A - 352 - Zentralcampus|zentralcampus|• MCA-Teilleistung",
		"11:30|Lehrgebäude 3A - 352 - Zentralcampus|zentralcampus|∅")
	// Confirmed by the list, the page is as current as the list.
	want(t, db, "SELECT fetched_at, source_url FROM event WHERE id = '151102'", "2026-09-24T01:00:00Z|"+pageURL("151102"))

	want(t, db, "SELECT weekday, start_time, rhythm, first_date, last_date, room, campus FROM event_date WHERE event_id = '147988' ORDER BY ord",
		"4|11:30|weekly|2026-10-22|2027-01-28|Lehrgebäude 3A - 324|∅",
		"4|15:30|week_a|2026-10-22|2027-01-28|Lehrgebäude 3A - 352 - Zentralcampus|zentralcampus",
		"4|15:30|single|2026-12-10|2026-12-10|Lehrgebäude 3A - 352 - Zentralcampus|zentralcampus",
		"4|11:30|single|2027-02-04|2027-02-04|Lehrgebäude 3A - 324|∅")
	want(t, db, "SELECT cancelled_dates FROM event_date WHERE event_id = '147988' AND cancelled_dates IS NOT NULL", "17.12.2026: takes place on 10.12.2026")
	want(t, db, "SELECT title, number, type_raw, category, semester_key, sws, source_url FROM event WHERE id = '147988'",
		"Fundamentals of Engine Technology|350301|Vorlesung/Übung|teaching|2026W|4|"+pageURL("147988"))
	want(t, db, "SELECT name, role FROM event_person WHERE event_id = '147988' ORDER BY ord", "Höschler|∅", "Maier|∅")

	// The moved date, without the remarks of the page it replaces.
	want(t, db, "SELECT start_time, comment FROM event_date WHERE event_id = '151278' ORDER BY ord", "08:00|∅", "07:30|∅")
	want(t, db, "SELECT start_time, cancelled_dates FROM event_date WHERE event_id = '152864' ORDER BY ord",
		"16:30|∅", "16:30|14.10.2026: findet ersatzweise im HS 11.301 statt.")

	want(t, db, "SELECT start_time FROM event_date WHERE event_id = '151296' AND ord = 1", "11:30")
	want(t, db, "SELECT COUNT(*) FROM event_date WHERE event_id = '151296' AND start_time = '08:15'", "0")

	want(t, db, "SELECT event_id FROM module_event WHERE module_id = '13921' ORDER BY event_id", "147988", "151102", "151278", "151296", "152864")
}

// When BTU removes an event, the event search no longer shows it and QIS answers its page
// with HTTP 200 and its empty frame, while a module description, read again within a month,
// may still name it (2026-09-27: 149396 and 152211, which the build of that day wrote as
// events titled with their ID, without a semester or a date). Such an event is not built,
// nor is the link to it. Every row the archive holds of it is unused and goes once nothing
// has fetched it for the grace period: while a description names the event, the crawler
// keeps asking about it. An empty page of an event the search still shows is no page
// either: its entry states the event.
func TestBuildLeavesOutEventsQISRemoved(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "removed.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	entry := entries(t)
	emptied := string(fixture(t, "qis_event_149396.html"))
	night := time.Date(2026, 9, 27, 1, 0, 0, 0, time.UTC)
	put := func(source, key, body string, at time.Time) {
		t.Helper()
		status := 200
		if body == "" {
			status = 404 // an event the search does not show, or a page QIS answers with 404
		}
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: "https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=" + key,
			HTTPStatus: status, Body: []byte(body), FetchedAt: at}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceQISModuleList, "rows-000000", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>13921</td><td><a href="/rds?pord.pordnr=9001">Lightweight Design and Construction</a></td><td>Englisch</td><td>6</td><td></td><td></td></tr>
	</table>`, night)
	put(catalogdb.SourceQISModulePage, "13921", qisModulePage("13921", "Lightweight Design and Construction", "6",
		qisEventLink("151102", "Vorlesung")+qisEventLink("149396", "Seminar")+qisEventLink("152864", "Vorlesung")+
			qisEventLink("900001", "Übung")+qisEventLink("900002", "Übung")), night)

	// An event as it should be.
	put(catalogdb.SourceQISEvent, "151102", string(fixture(t, "qis_event_151102.html")), night)
	put(catalogdb.SourceQISEventEntry, "151102", entry("151102"), night)
	// Removed: the search does not show it, and its page is the empty frame.
	put(catalogdb.SourceQISEventEntry, "149396", "", night)
	put(catalogdb.SourceQISEvent, "149396", emptied, night)
	// The same, with a page QIS answers with 404.
	put(catalogdb.SourceQISEventEntry, "900001", "", night)
	put(catalogdb.SourceQISEvent, "900001", "", night)
	// Not shown by the search, and its page not fetched yet: merely not archived.
	put(catalogdb.SourceQISEventEntry, "900002", "", night)
	// The empty frame, newer than the entry, for an event the search shows.
	put(catalogdb.SourceQISEventEntry, "152864", entry("152864"), night.Add(-time.Hour))
	put(catalogdb.SourceQISEvent, "152864", emptied, night)
	// Removed a while ago, and no description names it any more: nothing fetched it since.
	put(catalogdb.SourceQISEventEntry, "152211", "", night.Add(-8*24*time.Hour))
	put(catalogdb.SourceQISEvent, "152211", emptied, night.Add(-8*24*time.Hour))

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	want(t, db, "SELECT id, title, type_raw, semester_key FROM event ORDER BY id",
		"151102|Lightweight Design and Construction|Vorlesung|2026W",
		"152864|Einführung in die Erziehungswissenschaft|Vorlesung|2026W")
	want(t, db, "SELECT COUNT(*) FROM event_date WHERE event_id = '152864'", "2")
	want(t, db, "SELECT event_id FROM module_event WHERE module_id = '13921' ORDER BY event_id", "151102", "152864")
	want(t, db, "SELECT DISTINCT event_id FROM v_module_schedule WHERE module_id = '13921' ORDER BY 1", "151102", "152864")
	if report.Events != 2 || report.EventsFromList != 1 || report.EventLinksNoArchive != 1 ||
		strings.Join(report.EventLinksGone, ", ") != "13921 → 149396, 13921 → 900001" {
		t.Errorf("events %d, from the list %d, not archived %d, removed %q", report.Events, report.EventsFromList, report.EventLinksNoArchive, report.EventLinksGone)
	}
	if got := strings.Join(report.Unused[catalogdb.SourceQISEvent], ","); got != "149396,152211,900001" {
		t.Errorf("unused event pages = %s", got)
	}
	if got := strings.Join(report.Unused[catalogdb.SourceQISEventEntry], ","); got != "149396,152211,900001,900002" {
		t.Errorf("unused event entries = %s", got)
	}

	// The archive stage removes what nothing fetched within its grace period: 152211. The
	// crawler keeps asking about 149396 while a description names it.
	if n, err := db.PruneArchive(report.Unused, night.Add(-7*24*time.Hour)); err != nil || n != 2 {
		t.Errorf("PruneArchive removed %d rows (err %v), want the page and the entry of 152211", n, err)
	}
	for _, source := range []string{catalogdb.SourceQISEvent, catalogdb.SourceQISEventEntry} {
		if _, err := db.GetPage(source, "152211"); err != catalogdb.ErrNotFound {
			t.Errorf("%s of 152211 still archived (%v)", source, err)
		}
		if _, err := db.GetPage(source, "149396"); err != nil {
			t.Errorf("%s of 149396 removed within the grace period: %v", source, err)
		}
	}

	withoutSemester := func() catalogdb.Check {
		t.Helper()
		checks, err := db.Validate(context.Background(), nil)
		if err != nil {
			t.Fatalf("Validate failed: %v", err)
		}
		if catalogdb.HasFailures(checks) {
			t.Fatalf("the build fails validation: %+v", checks)
		}
		for _, c := range checks {
			if c.Name == "events a module links that have no semester" {
				return c
			}
		}
		t.Fatalf("no check of linked events without a semester")
		return catalogdb.Check{}
	}
	if c := withoutSemester(); c.Status != catalogdb.StatusOK {
		t.Errorf("check = %+v", c)
	}

	// A semester Radix cannot read („2026/27", without its season), on the page of an event
	// the search does not show: the event is built without a semester, Folia leaves it off
	// the module page, and validate names it.
	page := strings.Replace(string(fixture(t, "qis_event_151102.html")), `headers="basic_5">WS 2026/27<`, `headers="basic_5">2026/27<`, 1)
	if !strings.Contains(page, `headers="basic_5">2026/27<`) {
		t.Fatal("the fixture no longer states its semester where this test expects it")
	}
	put(catalogdb.SourceQISEventEntry, "151102", "", night.Add(time.Hour))
	put(catalogdb.SourceQISEvent, "151102", page, night.Add(time.Hour))
	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	want(t, db, "SELECT title, semester_key FROM event WHERE id = '151102'", "Lightweight Design and Construction|∅")
	if c := withoutSemester(); c.Status != catalogdb.StatusWarn || c.Value != 1 ||
		strings.Join(c.Samples, "; ") != "151102 Lightweight Design and Construction → 13921" {
		t.Errorf("check = %+v", c)
	}
}
