package catalogbuild

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/model"
	"github.com/leonieziechmann/betula/radix/internal/normalize"
	"github.com/leonieziechmann/betula/radix/internal/parser"
)

// The modules of 2026W whose events stand at two campuses (canary's snapshot of 2026-10-19), each
// with what its description states and what QIS states of each event.
func TestTracksOf(t *testing.T) {
	ev := func(id, campus string, sws float64, lecture bool) trackEvent {
		return trackEvent{id: id, campus: campus, sws: sws, lecture: lecture}
	}
	cases := []struct {
		name         string
		events       []trackEvent
		sws, lecture float64
		want         map[string][]string
	}{{
		// Programmierpraktikum: 1 SWS lecture, 2 SWS practical. The lecture every other week and four
		// lab slots at the Zentralcampus, a lecture and a lab in Sachsendorf: two whole courses.
		name:   "12102 Programmierpraktikum",
		events: []trackEvent{ev("148008", "zentralcampus", 1, true), ev("148370", "zentralcampus", 2, false), ev("148455", "sachsendorf", 2, false), ev("149408", "sachsendorf", 1, true)},
		sws:    3, lecture: 1,
		want: map[string][]string{"zentralcampus": {"148008", "148370"}, "sachsendorf": {"148455", "149408"}},
	}, {
		// Betriebssysteme und Rechnernetze: one half in Sachsendorf, the other at the Zentralcampus.
		name:   "11760 Betriebssysteme und Rechnernetze",
		events: []trackEvent{ev("150029", "sachsendorf", 2, true), ev("150033", "zentralcampus", 2, true)},
		sws:    4, lecture: 2,
	}, {
		// Elektronische Bauelemente und Schaltungen: the lecture in Cottbus, the lab in Senftenberg.
		name:   "13693 Elektronische Bauelemente und Schaltungen",
		events: []trackEvent{ev("148521", "senftenberg", 2, false), ev("150164", "zentralcampus", 2, true)},
		sws:    4, lecture: 2,
	}, {
		// Medizinische Grundlagen: 5 SWS of lectures, two of them in Senftenberg, three in Sachsendorf.
		name:   "11777 Medizinische Grundlagen",
		events: []trackEvent{ev("148769", "sachsendorf", 2, true), ev("148839", "senftenberg", 2, true), ev("149528", "sachsendorf", 1, true), ev("149529", "sachsendorf", 1, false), ev("150585", "sachsendorf", 1, false)},
		sws:    6, lecture: 5,
	}, {
		// Softwarepraktikum: no lecture; 6 SWS at each campus.
		name:   "12202 Softwarepraktikum",
		events: []trackEvent{ev("148294", "zentralcampus", 6, false), ev("149333", "sachsendorf", 6, false)},
		sws:    6,
		want:   map[string][]string{"zentralcampus": {"148294"}, "sachsendorf": {"149333"}},
	}, {
		// A course without a lecture is none where the module has lectures.
		name:   "no lecture",
		events: []trackEvent{ev("1", "zentralcampus", 4, true), ev("2", "senftenberg", 4, false)},
		sws:    4, lecture: 2,
	}, {
		// An event at no campus, or at two, is in no course; the module's SWS unknown, no tracks.
		name:   "unknown",
		events: []trackEvent{ev("1", "zentralcampus", 4, true), ev("2", "senftenberg", 4, true), ev("3", "", 2, false)},
		want:   nil,
	}}
	for _, c := range cases {
		got := tracksOf(c.events, c.sws, c.lecture)
		for _, events := range got {
			sort.Strings(events)
		}
		if !reflect.DeepEqual(got, c.want) {
			t.Errorf("%s: %v, want %v", c.name, got, c.want)
		}
	}
	// The event at no campus stays everybody's.
	got := tracksOf([]trackEvent{ev("1", "zentralcampus", 4, true), ev("2", "senftenberg", 4, true), ev("3", "", 2, false)}, 4, 2)
	if len(got) != 2 || len(got["zentralcampus"]) != 1 || len(got["senftenberg"]) != 1 {
		t.Errorf("an event at no campus: %v", got)
	}
}

// Whom QIS lists at the courses of 2026W (canary's snapshot of 2026-10-19).
func TestSameAudience(t *testing.T) {
	of := func(keys ...string) map[string]bool {
		set := make(map[string]bool)
		for _, key := range keys {
			set[key] = true
		}
		return set
	}
	audiences := map[string]map[string]bool{
		// Programmierpraktikum: Informatik and six more at the Zentralcampus, Medizininformatik in
		// Sachsendorf.
		"148008": of("079-82", "105-82", "128-82"), "148370": of("079-82", "label:Physik Bachelor (universitäres Profil)"),
		"149408": of("247-82"), "148455": of("247-82"),
		// Research Module in Artificial Intelligence: every seminar for AI M.Sc.
		"150000": of("ARI-88"), "151375": of("ARI-88"), "150690": of("ARI-88"),
		// Höhere Mathematik - T1: Elektrotechnik at both campuses, the others at one.
		"150828": of("048-82", "017-82", "104-82"), "149440": of("048-82", "049-82", "216-82"),
	}
	cases := []struct {
		name    string
		courses map[string][]string
		want    bool
	}{
		{"12102: a program of its own at each campus", map[string][]string{"zentralcampus": {"148008", "148370"}, "sachsendorf": {"148455", "149408"}}, false},
		{"14060: AI M.Sc. at every seminar", map[string][]string{"zentralcampus": {"150000", "151375"}, "sachsendorf": {"150690"}}, true},
		{"11107: Elektrotechnik at both, the others at one", map[string][]string{"zentralcampus": {"150828"}, "senftenberg": {"149440"}}, false},
		{"a course QIS lists nobody at", map[string][]string{"zentralcampus": {"150000"}, "sachsendorf": {"1"}}, false},
		{"one audience a part of the other", map[string][]string{"zentralcampus": {"148008"}, "sachsendorf": {"148370"}}, false},
	}
	for _, c := range cases {
		if got := sameAudience(c.courses, audiences); got != c.want {
			t.Errorf("%s: %v, want %v", c.name, got, c.want)
		}
	}
}

// The labels of the event pages of 2026W (2026-10-19) against programs as the catalog has them.
func TestResolveEventPrograms(t *testing.T) {
	prog := func(id, name, level, kind, variant string, po int) *program {
		return &program{id: id, name: name, degree: normalize.DegreeInfo{Level: level, Type: kind, Variant: variant}, poYear: po, familyKey: id[:len(id)-5]}
	}
	b, m := normalize.LevelBachelor, normalize.LevelMaster
	u, a := normalize.TypeUniversity, normalize.TypeApplied
	programs := []*program{
		prog("079-82-2008", "Informatik", b, u, "", 2008),
		prog("079-88-2008", "Informatik", m, u, "", 2008),
		prog("247-82-2016", "Medizininformatik", b, u, "", 2016),
		prog("G18-82-2022", "Künstliche Intelligenz", b, u, "", 2022),
		prog("G17-82-2022", "Künstliche Intelligenz Technologie", b, u, "", 2022),
		prog("048-82-2019", "Elektrotechnik", b, u, "", 2019),
		prog("048-82-2022", "Elektrotechnik", b, u, "", 2022),
		prog("048-84-2018", "Elektrotechnik", b, a, "", 2018),
		prog("048-90-2018", "Elektrotechnik", m, a, "", 2018),
		prog("048-F0-2018", "Elektrotechnik", m, a, normalize.VariantExtended, 2018),
		prog("G02-A2-2022", "Elektrotechnik - dual", b, u, normalize.VariantDualTraining, 2022),
		prog("G02-P2-2022", "Elektrotechnik - dual", b, u, normalize.VariantDualPractice, 2022),
		prog("216-82-2022", "Medizintechnik", b, u, "", 2022),
		prog("G19-P2-2022", "Medizintechnik - dual", b, u, normalize.VariantDualPractice, 2022),
		prog("017-82-2022", "Bauingenieurwesen", b, u, "", 2022),
		prog("017-F2-2022", "Bauingenieurwesen", b, u, normalize.VariantExtended, 2022),
		prog("796-A2-2022", "Bauingenieurwesen - dual", b, u, normalize.VariantDualTraining, 2022),
		prog("211-82-2021", "Energietechnik und Energiewirtschaft", b, u, "", 2021),
		prog("211-88-2021", "Energietechnik und Energiewirtschaft", m, u, "", 2021),
		prog("902-P4-2021", "Hebammenwissenschaft", b, a, normalize.VariantDualPractice, 2021),
		prog("C48-19-2026", "Lehramt Primarstufe Deutsch-Englisch", normalize.LevelTeachingBachelor, "", "", 2026),
	}
	names := newProgramNames(programs)
	cases := []struct {
		label, po string
		want      string
	}{
		{"Informatik Bachelor (universitäres Profil)", "", "079-82-2008"},
		{"Medizininformatik Bachelor (universitäres Profil)", "", "247-82-2016"},
		{"Künstliche Intelligenz Bachelor", "2022", "G18-82-2022"},
		{"Künstliche Intelligenz Technologie Bachelor", "2022", "G17-82-2022"},
		// A study direction names its program; the PO picks the version.
		{"Elektrotechnik - MIT - Bachelor", "2022", "048-82-2022"},
		{"Elektrotechnik - IoT - Bachelor", "", "048-82-2019 048-82-2022 048-84-2018"},
		{"Elektrotechnik - EET - Bachelor - dual", "2022", "G02-A2-2022 G02-P2-2022"},
		{"Elektrotechnik Master (fachhochschulisches Profil)", "2025", "048-90-2018"},
		{"Medizintechnik - EM - Bachelor", "2022", "216-82-2022"},
		{"Medizintechnik - dual: MM - Bachelor - P2", "2022", "G19-P2-2022"},
		{"Medizintechnik - dual: EM - Bachelor - A2", "2022", ""},
		{"Bauingenieurwesen Bachelor", "2022", "017-82-2022"},
		{"Bauingenieurwesen Bachelor erweiterte Fachsemester", "2022", "017-F2-2022"},
		{"Bauingenieurwesen Bachelor - dual ausbildungsintegrierend", "2022", "796-A2-2022"},
		// No degree: both of the PO.
		{"Energietechnik und Energiewirtschaft EET", "2021", "211-82-2021 211-88-2021"},
		// The catalog has Hebammenwissenschaft as a dual program only.
		{"Hebammenwissenschaft Bachelor (fachhochschulisches Profil)", "", "902-P4-2021"},
		{"Hebammenwissenschaft Bachelor (universitär) - Duales Studium, praxisintegrierend", "", ""},
		{"Lehramt Primarstufe Bachelor - Deutsch-Englisch", "2026", "C48-19-2026"},
		{"Informatikdidaktik Master", "", ""},
		{"Astrologie Bachelor", "", ""},
	}
	for _, c := range cases {
		var ids []string
		for _, p := range names.resolve(model.EventProgram{Label: c.label, PO: c.po}) {
			ids = append(ids, p.id)
		}
		sort.Strings(ids)
		if got := strings.Join(ids, " "); got != c.want {
			t.Errorf("%q (PO %q): %q, want %q", c.label, c.po, got, c.want)
		}
	}

	for raw, want := range map[string][2]int{"1 - 1": {1, 1}, "3 - 5": {3, 5}, "5": {5, 5}, "": {0, 0}, "5 - 3": {0, 0}, "WS": {0, 0}} {
		if from, to := semesterRange(raw); [2]int{from, to} != want {
			t.Errorf("semesterRange(%q) = %d, %d; want %v", raw, from, to, want)
		}
	}
}

func TestMostOf(t *testing.T) {
	counts := map[string]int{"zentralcampus": 5, "sachsendorf": 4, "senftenberg": 1}
	if got := mostOf(counts, nil); got != "zentralcampus" {
		t.Errorf("mostOf = %q", got)
	}
	if got := mostOf(counts, map[string][]string{"sachsendorf": nil, "senftenberg": nil}); got != "sachsendorf" {
		t.Errorf("mostOf among Sachsendorf and Senftenberg = %q", got)
	}
	if got := mostOf(map[string]int{"zentralcampus": 2, "senftenberg": 2}, nil); got != "" {
		t.Errorf("a tie = %q", got)
	}
	if got := mostOf(counts, map[string][]string{"nord": nil}); got != "" {
		t.Errorf("none among = %q", got)
	}

	// Wirtschaftsingenieurwesen B.Sc., FH profile: QIS lists it for 30 SWS in Senftenberg and 3 at
	// the Zentralcampus; three of its four compulsory modules are held at the Zentralcampus.
	wing := campusEvidence{sws: map[string]float64{"senftenberg": 30, "zentralcampus": 3}, modules: map[string]int{"zentralcampus": 3, "senftenberg": 1}}
	if got := wing.main(nil); got != "senftenberg" {
		t.Errorf("QIS first: %q", got)
	}
	if got := wing.main(map[string][]string{"zentralcampus": nil, "sachsendorf": nil}); got != "zentralcampus" {
		t.Errorf("among the Zentralcampus and Sachsendorf: %q", got)
	}
	// Where QIS lists the program nowhere among the courses, its compulsory modules decide.
	plan := campusEvidence{sws: map[string]float64{"senftenberg": 12}, modules: map[string]int{"sachsendorf": 2, "zentralcampus": 1}}
	if got := plan.main(map[string][]string{"zentralcampus": nil, "sachsendorf": nil}); got != "sachsendorf" {
		t.Errorf("the compulsory modules after QIS: %q", got)
	}
	if got := mostSWS(map[string]float64{"zentralcampus": 2, "senftenberg": 2}, nil); got != "" {
		t.Errorf("a tie of SWS = %q", got)
	}
	if got := (campusEvidence{}).main(nil); got != "" {
		t.Errorf("no evidence = %q", got)
	}
}

// TestTracksOnSnapshot reads the tracks of a real snapshot: RADIX_TRACKS_SNAPSHOT names a copy of
// one, which it migrates and writes the tracks into. A snapshot has no pages: the programs QIS
// lists at the events are the snapshot's own (schema 15 or newer, canary's /api/db), or those of
// RADIX_TRACKS_PAGES, a directory of QIS event pages saved as <veranstid>.html (also in its
// subdirectories), whose „Studiengänge" it reads as the build does:
//
//	RADIX_TRACKS_SNAPSHOT=$PWD/../snapshot/catalog.db go test ./internal/catalogbuild -run TestTracksOnSnapshot -count=1 -v
func TestTracksOnSnapshot(t *testing.T) {
	path := os.Getenv("RADIX_TRACKS_SNAPSHOT")
	if path == "" {
		t.Skip("RADIX_TRACKS_SNAPSHOT not set")
	}
	db, err := catalogdb.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	var programs []*program
	rows, err := db.SQL().Query("SELECT id, name, degree_level, IFNULL(degree_type, ''), IFNULL(study_variant, ''), IFNULL(po_year, 0), family_key FROM program ORDER BY id")
	if err != nil {
		t.Fatal(err)
	}
	for rows.Next() {
		p := &program{}
		if err := rows.Scan(&p.id, &p.name, &p.degree.Level, &p.degree.Type, &p.degree.Variant, &p.poYear, &p.familyKey); err != nil {
			t.Fatal(err)
		}
		programs = append(programs, p)
	}
	rows.Close()

	tx, err := db.SQL().Begin()
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	// The programs of the events stay as the snapshot has them unless pages are given to read them
	// from (a snapshot of schema 15 or newer has them).
	pages := os.Getenv("RADIX_TRACKS_PAGES")
	tables := []string{"module_track_program", "module_track", "program_campus"}
	if pages != "" {
		tables = append(tables, "event_program_ref", "event_program")
	}
	for _, table := range tables {
		if _, err := tx.Exec("DELETE FROM " + table); err != nil {
			t.Fatal(err)
		}
	}
	report := &Report{EventProgramsUnresolved: make(map[string]int)}
	if dir := pages; dir != "" {
		events := make(map[string]bool)
		rows, err := tx.Query("SELECT id FROM event")
		if err != nil {
			t.Fatal(err)
		}
		for rows.Next() {
			var id string
			if err := rows.Scan(&id); err != nil {
				t.Fatal(err)
			}
			events[id] = true
		}
		rows.Close()
		names := newProgramNames(programs)
		read := make(map[string]bool)
		err = filepath.WalkDir(dir, func(file string, entry os.DirEntry, err error) error {
			if err != nil || entry.IsDir() || !strings.HasSuffix(file, ".html") {
				return err
			}
			id := strings.TrimSuffix(entry.Name(), ".html")
			if !events[id] || read[id] {
				return nil
			}
			read[id] = true
			body, err := os.ReadFile(file)
			if err != nil {
				return err
			}
			d, err := parser.NewEventParser().Parse(bytes.NewReader(body), id, "")
			if err != nil {
				return err
			}
			return insertEventPrograms(tx, names, id, d.Programs, report)
		})
		if err != nil {
			t.Fatal(err)
		}
		t.Logf("%d event pages: %d rows resolved, %d not (%d labels): %v", len(read), report.EventProgramsResolved,
			total(report.EventProgramsUnresolved), len(report.EventProgramsUnresolved), examples(report.EventProgramsUnresolved, 40))
	}
	if err := writeTracks(tx, programs, report); err != nil {
		t.Fatal(err)
	}
	if err := tx.Commit(); err != nil {
		t.Fatal(err)
	}
	t.Logf("%d modules × semesters with tracks, %d with one audience at every course; %d programs with a main campus, %d of them by QIS; %d track programs by QIS, %d by campus",
		report.ModulesWithTracks, report.TracksOneAudience, report.ProgramsWithCampus, report.ProgramsCampusByQIS, report.TrackProgramsByQIS, report.TrackProgramsByCampus)

	var listed string
	if err := db.SQL().QueryRow(`SELECT IFNULL(GROUP_CONCAT(m, ' '), '') FROM (SELECT DISTINCT module_id || ':' || semester_key AS m FROM module_track ORDER BY 1)`).Scan(&listed); err != nil {
		t.Fatal(err)
	}
	t.Logf("tracks: %s", listed)

	// The case of 2026-10-19: Programmierpraktikum, at the Zentralcampus and in Sachsendorf.
	var campuses string
	if err := db.SQL().QueryRow(`SELECT IFNULL(GROUP_CONCAT(DISTINCT campus), '') FROM (SELECT campus FROM module_track WHERE module_id = '12102' AND semester_key = '2026W' ORDER BY campus)`).Scan(&campuses); err != nil {
		t.Fatal(err)
	}
	if campuses != "sachsendorf,zentralcampus" {
		t.Errorf("tracks of 12102 in 2026W: %q", campuses)
	}
	var none int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM module_track WHERE module_id IN ('11760', '13693', '11777')`).Scan(&none); err != nil || none != 0 {
		t.Errorf("modules whose parts are all required have tracks: %d, %v", none, err)
	}
	of := func(program string) string {
		var campus string
		_ = db.SQL().QueryRow(`SELECT IFNULL(GROUP_CONCAT(campus || '/' || basis), '') FROM module_track_program WHERE module_id = '12102' AND semester_key = '2026W' AND program_id = ?`, program).Scan(&campus)
		return campus
	}
	if got := of("079-82-2008"); !strings.HasPrefix(got, "zentralcampus/") {
		t.Errorf("Informatik B.Sc. takes 12102 at %q", got)
	}
	var programRows int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM event_program`).Scan(&programRows); err != nil {
		t.Fatal(err)
	}
	if programRows > 0 {
		if got := of("247-82-2016"); got != "sachsendorf/qis" {
			t.Errorf("Medizininformatik B.Sc. takes 12102 at %q", got)
		}
	}
}

// trackEventPage is an event page of 2026W with its SWS, one weekly date in a room and the
// programs QIS lists it for.
func trackEventPage(title, eventType, sws, room string, programs ...string) string {
	var rows strings.Builder
	for _, p := range programs {
		rows.WriteString(`<tr><td headers="curricular_2"><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;k_abstgv.abstgvnr=71">` + p +
			`</a></td><td headers="curricular_3">1 - 1</td><td headers="curricular_4"></td><td headers="curricular_5"></td></tr>`)
	}
	return `<html><body><h1>` + title + ` - Einzelansicht</h1>
	<table summary="Grunddaten zur Veranstaltung"><tr><th>Veranstaltungsart</th><td>` + eventType + `</td><th>Semester</th><td>WS 2026/27</td></tr>
		<tr><th>SWS</th><td>` + sws + `</td></tr></table>
	<table summary="Übersicht über alle Veranstaltungstermine"><caption>Termine Gruppe: [unbenannt]</caption>
		<tr><th>Tag</th><th>Zeit</th><th>Rhythmus</th><th>Dauer</th><th>Raum</th><th>Lehrperson</th></tr>
		<tr><td>Di.</td><td>09:15 bis 10:45</td><td>A/B</td><td>06.10.2026 bis 26.01.2027</td><td><a href="#">` + room + `</a></td><td><a href="#">Meyer</a></td></tr></table>
	<table summary="Übersicht über die zugehörigen Studiengänge"><caption>Studiengänge</caption>
		<tr><th>Studiengang</th><th>Semester</th><th>PO</th><th>Bemerkung</th></tr>` + rows.String() + `</table>
	</body></html>`
}

// A miniature of 2026-10-19: the Programmierpraktikum as a course at the Zentralcampus for
// Informatik and one in Sachsendorf for Medizininformatik; a module whose two halves are held at
// the two campuses, both required; and one compulsory module of each program at its own campus.
// (The module pages state 4 SWS of lectures and 2 of exercises, as modulePageDE does.)
func TestBuildTracks(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "tracks.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 10, 19, 9, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatal(err)
		}
	}
	const (
		central     = "Zentrales Hörsaalgebäude - Hörsaal C - Zentralcampus"
		sachsendorf = "Gebäude 9 - SD - 9.117 PC-Pool - Campus Sachsendorf"
		informatik  = "Informatik Bachelor (universitäres Profil)"
		medinf      = "Medizininformatik Bachelor (universitäres Profil)"
	)
	modules := map[string]string{"12102": "Programmierpraktikum", "11760": "Betriebssysteme und Rechnernetze", "11112": "Mathematik IT-1", "11777": "Medizinische Grundlagen", "14060": "Research Module"}
	events := map[string][]string{"12102": {"148008", "148370", "149408", "148455"}, "11760": {"150029", "150033"}, "11112": {"150100"}, "11777": {"148769"}, "14060": {"150000", "150690"}}
	var list strings.Builder
	list.WriteString(`<table><tbody class="list">`)
	for _, id := range sortedKeys(modules) {
		list.WriteString(`<tr><td class="moduleNumber"><a href="/modul/` + id + `">` + id + `</a></td><td class="title">` + modules[id] + `</td></tr>`)
		var links strings.Builder
		for _, event := range events[id] {
			links.WriteString(qisEventLink(event, event))
		}
		put(catalogdb.SourceModulePage, id, modulePageDE(id, modules[id], `<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul>`+links.String()+`</ul></td></tr>`))
	}
	list.WriteString(`</tbody></table>`)
	put(catalogdb.SourceModuleCatalog, "list", list.String())

	put(catalogdb.SourceQISEvent, "148008", trackEventPage("Programmierpraktikum", "Vorlesung", "4", central, informatik))
	put(catalogdb.SourceQISEvent, "148370", trackEventPage("Programmierpraktikum", "Übung", "2", central, informatik, "Physik Bachelor (universitäres Profil)"))
	put(catalogdb.SourceQISEvent, "149408", trackEventPage("Programmierpraktikum", "Vorlesung", "4", sachsendorf, medinf))
	put(catalogdb.SourceQISEvent, "148455", trackEventPage("Programmierpraktikum", "Übung", "2", sachsendorf, medinf))
	put(catalogdb.SourceQISEvent, "150029", trackEventPage("Betriebssysteme und Rechnernetze - Betriebssysteme", "Vorlesung/Übung", "3", sachsendorf, medinf))
	put(catalogdb.SourceQISEvent, "150033", trackEventPage("Betriebssysteme und Rechnernetze - Rechnernetze", "Vorlesung/Übung", "3", central, medinf))
	put(catalogdb.SourceQISEvent, "150100", trackEventPage("Mathematik IT-1", "Vorlesung", "6", central))
	put(catalogdb.SourceQISEvent, "148769", trackEventPage("Medizinische Grundlagen", "Vorlesung", "6", sachsendorf, "Wirtschaftsinformatik Bachelor"))
	// Two whole seminars, at each campus one, both for Medizininformatik: topics to choose from, no
	// choice of place.
	put(catalogdb.SourceQISEvent, "150000", trackEventPage("Research Module", "Vorlesung", "6", central, medinf))
	put(catalogdb.SourceQISEvent, "150690", trackEventPage("Projektmodul", "Vorlesung", "6", sachsendorf, medinf))

	for _, p := range []fixtureProgram{
		{name: "Informatik", stg: "079", abschl: "82", degree: "Bachelor (universitär)", pversion: "2008", po: "2008 - 2. SÄ 2024"},
		{name: "Medizininformatik", stg: "247", abschl: "82", degree: "Bachelor (universitär)", pversion: "2016", po: "2016 - 1. SÄ 2024"},
		{name: "Wirtschaftsinformatik", stg: "C38", abschl: "82", degree: "Bachelor (universitär)", pversion: "2024", po: "2024"},
	} {
		node := "auswahlBaum|studiengang:stg=" + p.stg + "|abschluss:abschl=" + p.abschl + "|stgSpecials:vert=,schwp=,kzfa=H,pversion=" + p.pversion
		put(catalogdb.SourceQISTree, treeBase+node, fixtureTreePage(p, node+"|pruefung:1", "12102 Programmierpraktikum"))
	}
	plan := func(program string, modules ...string) {
		t.Helper()
		var entries []catalogdb.PlanEntry
		for _, id := range modules {
			entries = append(entries, catalogdb.PlanEntry{ModuleID: id, ModuleName: id, Semester: 1, StartSemester: 1, EndSemester: 1, Credits: 6, KindRaw: "Pflicht"})
		}
		if err := db.SavePlan(catalogdb.Plan{ProgramID: program, SourceFile: "po.pdf", LayoutJSON: "{}", Entries: entries}); err != nil {
			t.Fatal(err)
		}
	}
	plan("079-82-2008", "12102", "11112")
	plan("247-82-2016", "12102", "11760", "11777")
	plan("C38-82-2024", "12102", "11112")

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if report.ModulesWithTracks != 1 || report.TracksOneAudience != 1 || report.EventProgramsResolved != 9 || total(report.EventProgramsUnresolved) != 1 {
		t.Errorf("report: %d modules with tracks, %d with one audience, %d rows resolved, unresolved %v", report.ModulesWithTracks, report.TracksOneAudience, report.EventProgramsResolved, report.EventProgramsUnresolved)
	}
	if report.ProgramsWithCampus != 3 || report.ProgramsCampusByQIS != 3 {
		t.Errorf("report: %d programs with a campus, %d of them by QIS", report.ProgramsWithCampus, report.ProgramsCampusByQIS)
	}

	want(t, db, `SELECT event_id, ord, label, semester_min, semester_max, resolve_status, IFNULL(program_id, '∅'), IFNULL(family_key, '∅') FROM v_event_program WHERE event_id = '148370' ORDER BY ord`,
		"148370|1|Informatik Bachelor (universitäres Profil)|1|1|resolved|079-82-2008|079-82",
		"148370|2|Physik Bachelor (universitäres Profil)|1|1|unresolved|∅|∅")
	// Two courses of the Programmierpraktikum; 11760's halves are no courses, nor are 14060's
	// seminars, which QIS lists for the same program.
	want(t, db, `SELECT DISTINCT module_id, event_id, track FROM v_module_schedule WHERE track IS NOT NULL ORDER BY 1, 3, 2`,
		"12102|148455|sachsendorf", "12102|149408|sachsendorf", "12102|148008|zentralcampus", "12102|148370|zentralcampus")
	// Where QIS lists each program's events (Medizininformatik: 4 + 2 + 3 + 6 SWS in Sachsendorf,
	// 3 + 6 at the Zentralcampus), and its compulsory modules but 12102. Wirtschaftsinformatik's at
	// the Zentralcampus, QIS lists it in Sachsendorf alone: there it is taught.
	want(t, db, `SELECT program_id, campus, modules, sws, is_main FROM v_program_campus ORDER BY 1, 2`,
		"079-82-2008|zentralcampus|1|6|1",
		"247-82-2016|sachsendorf|2|15|1", "247-82-2016|zentralcampus|1|9|0",
		"C38-82-2024|sachsendorf|0|6|1", "C38-82-2024|zentralcampus|1|0|0")
	// Informatik and Medizininformatik as QIS lists them; Wirtschaftsinformatik, which QIS does not
	// name at 12102, where QIS lists its other events.
	want(t, db, `SELECT module_id, semester_key, program_id, campus, basis, family_key FROM v_module_track_program ORDER BY program_id`,
		"12102|2026W|079-82-2008|zentralcampus|qis|079-82",
		"12102|2026W|247-82-2016|sachsendorf|qis|247-82",
		"12102|2026W|C38-82-2024|sachsendorf|campus|C38-82")

	// A second build replaces what the first wrote, to the same content.
	again, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if again.ContentDigest != report.ContentDigest {
		t.Errorf("the second build changed the content: %s, %s", report.ContentDigest, again.ContentDigest)
	}
}

// TestChoicesOnSnapshot reads the choices and extras of a real snapshot of schema 15 or newer
// (canary's /api/db): RADIX_CHOICES_SNAPSHOT names a copy of one, which it migrates and writes them
// into; RADIX_CHOICES_OUT, where given, a file it writes them to as JSON lines, one per module and
// semester, for research/wahlangebote/python/evaluate.py:
//
//	RADIX_CHOICES_SNAPSHOT=/tmp/copy.db RADIX_CHOICES_OUT=/tmp/choices.jsonl go test ./internal/catalogbuild -run TestChoicesOnSnapshot -count=1 -v
func TestChoicesOnSnapshot(t *testing.T) {
	path := os.Getenv("RADIX_CHOICES_SNAPSHOT")
	if path == "" {
		t.Skip("RADIX_CHOICES_SNAPSHOT not set")
	}
	db, err := catalogdb.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	tx, err := db.SQL().Begin()
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	for _, table := range []string{"module_extra", "module_choice_option", "module_choice"} {
		if _, err := tx.Exec("DELETE FROM " + table); err != nil {
			t.Fatal(err)
		}
	}
	report := &Report{}
	if err := writeChoices(tx, report); err != nil {
		t.Fatal(err)
	}
	if err := tx.Commit(); err != nil {
		t.Fatal(err)
	}
	t.Logf("%d modules × semesters with choices: %d by program, %d in groups, %d of topics; %d extras",
		report.ModulesWithChoices, report.ChoicesByProgram, report.ChoicesInGroups, report.ChoicesOfTopics, report.ChoiceExtras)

	// Höhere Mathematik T3 at the Zentralcampus: three exercises, one per program; its Aufbaukurs an extra.
	want(t, db, `SELECT kind, basis, sws, pick, COUNT(DISTINCT option) FROM v_module_choice WHERE module_id = '11206' AND semester_key = '2026W' AND course = 'zentralcampus' GROUP BY choice`,
		"program|form|2|1|3")
	want(t, db, `SELECT event_id, basis FROM module_extra WHERE module_id = '11206'`, "150190|title")
	// The Programmierpraktikum has its courses and nothing else.
	want(t, db, `SELECT COUNT(*) FROM module_choice WHERE module_id = '12102'`, "0")

	if out := os.Getenv("RADIX_CHOICES_OUT"); out != "" {
		f, err := os.Create(out)
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		rows, err := db.SQL().Query(`
			SELECT m.module_id, m.semester_key,
			       IFNULL((SELECT GROUP_CONCAT(c.kind || ':' || c.basis || ':' || c.pick || ':' ||
			              (SELECT COUNT(DISTINCT o.option) FROM module_choice_option o WHERE o.module_id = c.module_id AND o.semester_key = c.semester_key AND o.choice = c.choice), ';')
			               FROM module_choice c WHERE c.module_id = m.module_id AND c.semester_key = m.semester_key), ''),
			       IFNULL((SELECT GROUP_CONCAT(x.event_id, ';') FROM module_extra x WHERE x.module_id = m.module_id AND x.semester_key = m.semester_key), '')
			FROM (SELECT module_id, semester_key FROM module_choice UNION SELECT module_id, semester_key FROM module_extra) m
			ORDER BY 1, 2`)
		if err != nil {
			t.Fatal(err)
		}
		defer rows.Close()
		for rows.Next() {
			var module, semester, choices, extras string
			if err := rows.Scan(&module, &semester, &choices, &extras); err != nil {
				t.Fatal(err)
			}
			if _, err := f.WriteString(module + "\t" + semester + "\t" + choices + "\t" + extras + "\n"); err != nil {
				t.Fatal(err)
			}
		}
	}
}
