package catalogbuild

import (
	"context"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// qisModulePage is a module description as QIS serves it: label and value cells
// with their own classes, and the events of the semester that runs now.
func qisModulePage(id, title, credits, events string, rows ...string) string {
	return `<html><body><table cellpadding="5">
		<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">` + id + `</td></tr>
		<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">` + title + `</td></tr>
		<tr><td class="tabelle1_alignleft">&nbsp;</td><td class="tabelle2inhalt">` + title + ` (EN)</td></tr>
		<tr><td class="tabelle1_alignleft">Einrichtung:</td><td class="tabelle2inhalt">Fakultät 1 - MINT - Mathematik, Informatik</td></tr>
		<tr><td class="tabelle1_alignleft">Lehr- und Prüfungssprache:</td><td class="tabelle2inhalt">Deutsch</td></tr>
		<tr><td class="tabelle1_alignleft">Angebotsturnus:</td><td class="tabelle2inhalt">jedes Wintersemester</td></tr>
		<tr><td class="tabelle1_alignleft">Leistungspunkte:</td><td class="tabelle2inhalt">` + credits + `</td></tr>
		<tr><td class="tabelle1_alignleft">Veranstaltungen im aktuellen Semester:</td>
		    <td class="tabelle2inhalt"><ul>` + events + `</ul></td></tr>
		` + strings.Join(rows, "\n") + `
		</table></body></html>`
}

func qisEventLink(id, title string) string {
	return `<li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=` + id + `">` + title + `</a></li>`
}

// The catalog is maintained in QIS; b-tu.de/modul renders a copy that can be a
// semester behind. So the QIS description wins over the copy, its events are added
// to those of the copy rather than replacing them, and a module that only QIS lists
// is part of the catalog while one that only b-tu.de still lists stays in it.
func TestBuildPrefersTheQISDescription(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "qis.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 20, 21, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceModuleCatalog, "list", `<table><tbody class="list">
		<tr><td class="moduleNumber"><a href="/modul/11101">11101</a></td><td class="title">Lineare Algebra</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/13500">13500</a></td><td class="title">Nicht mehr angeboten</td></tr>
	</tbody></table>`)
	put(catalogdb.SourceQISModuleList, "rows-000000", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>11101</td><td><a href="/rds?pord.pordnr=6951">Lineare Algebra</a></td><td>Deutsch</td><td>8</td><td></td><td></td></tr>
		<tr><td>14999</td><td><a href="/rds?pord.pordnr=7777">Nur in QIS</a></td><td>Deutsch</td><td>5</td><td></td><td></td></tr>
	</table>`)

	// The copy still names the exam of the semester that is ending.
	put(catalogdb.SourceModulePage, "11101", modulePageDE("11101", "Lineare Algebra (alte Kopie)", `
		<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul>`+qisEventLink("120286", "130212 Prüfung")+`</ul></td></tr>`))
	put(catalogdb.SourceModulePage, "13500", modulePageDE("13500", "Nicht mehr angeboten", ``))

	// QIS names this semester's lecture, and states 9 credits where the copy says 8.
	put(catalogdb.SourceQISModulePage, "11101", qisModulePage("11101", "Lineare Algebra", "9", qisEventLink("120285", "130210 Vorlesung")))
	put(catalogdb.SourceQISModulePage, "14999", qisModulePage("14999", "Nur in QIS", "5", ""))

	put(catalogdb.SourceQISEvent, "120285", eventPageHTML("Vorlesung Lineare Algebra", "Vorlesung", "Hauptgebäude - HG 3.45 - Zentralcampus", "05.10.2026 bis 25.01.2027"))
	put(catalogdb.SourceQISEvent, "120286", eventPageHTML("Prüfung Lineare Algebra", "Prüfung", "Hauptgebäude - HG 0.18 - Zentralcampus", "am 16.09.2026"))

	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build failed: %v", err)
	}

	// QIS wins over the copy: its credits and its title. source_url states the page
	// the fields were read from, which is the QIS description, not the copy.
	want(t, db, "SELECT title, credits, page_lang, description_source, source_url FROM module WHERE id = '11101'",
		"Lineare Algebra|9|de|qis|11101")
	want(t, db, "SELECT description_source FROM module WHERE id = '13500'", "btu_cms")
	// A module only QIS lists is a module of the catalog, with its fields.
	want(t, db, "SELECT title, credits, detail_status FROM module WHERE id = '14999'", "Nur in QIS|5|ok")
	// A module only b-tu.de still lists is not lost.
	want(t, db, "SELECT detail_status FROM module WHERE id = '13500'", "ok")
	// Both descriptions decide which events belong to the module.
	want(t, db, "SELECT event_id FROM module_event WHERE module_id = '11101' ORDER BY event_id", "120285", "120286")
}

// qisReplacementRow is the row in which a QIS description names the other module of a
// replacement by its internal number (parser.TestReplacementRows): „Auslaufmodul" on the
// successor, „Nachfolgemodul/e" on the module that phases out.
func qisReplacementRow(label, since, id, pordnr string) string {
	return `<tr><td class="tabelle1_alignleft">` + label + `:</td><td class="tabelle2inhalt">` + since + `
		<ul><li><a href="https://www.b-tu.de/qisserver3/rds?state=modulBeschrDetailInfo&amp;pord.pordnr=` + pordnr + `">` + id + ` Modul</a></li></ul></td></tr>`
}

// A replacement is one relation, whichever of its two modules states it: 12160 names the
// module it replaces, 38105 its successor, and 12917 does not name its successor 11162,
// which names 12917. The successors stay active.
func TestBuildReadsAReplacementFromBothModules(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "replacement.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 29, 7, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceQISModuleList, "rows-000000", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>11162</td><td><a href="/rds?pord.pordnr=11908">Wirtschaftsprüfung</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12160</td><td><a href="/rds?pord.pordnr=14364">Allgemeine Betriebswirtschaftslehre I: Grundlagen der BWL</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12917</td><td><a href="/rds?pord.pordnr=16532">Wirtschaftsprüfung und Rechnungslegung</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>38105</td><td><a href="/rds?pord.pordnr=7293">Allgemeine Betriebswirtschaftslehre I</a></td><td>Deutsch</td><td>4</td><td></td><td></td></tr>
	</table>`)
	put(catalogdb.SourceQISModulePage, "12160", qisModulePage("12160", "Allgemeine Betriebswirtschaftslehre I: Grundlagen der BWL", "6", "",
		qisReplacementRow("Auslaufmodul", "Nachfolgemodul seit: 21.04.2017", "38105", "7293")))
	put(catalogdb.SourceQISModulePage, "38105", qisModulePage("38105 - Auslaufmodul", "Allgemeine Betriebswirtschaftslehre I", "4", "",
		qisReplacementRow("Nachfolgemodul/e", "Auslaufmodul ab: 21.04.2017", "12160", "14364")))
	put(catalogdb.SourceQISModulePage, "11162", qisModulePage("11162", "Wirtschaftsprüfung", "6", "",
		qisReplacementRow("Auslaufmodul", "Nachfolgemodul seit: 20.01.2023", "12917", "16532")))
	put(catalogdb.SourceQISModulePage, "12917", qisModulePage("12917 - Auslaufmodul", "Wirtschaftsprüfung und Rechnungslegung", "6", ""))

	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build failed: %v", err)
	}

	want(t, db, "SELECT id, offer_status FROM module ORDER BY id",
		"11162|active", "12160|active", "12917|phase_out", "38105|phase_out")
	want(t, db, "SELECT module_id, successor_id, successor_title FROM v_module_successor ORDER BY module_id",
		"12917|11162|Wirtschaftsprüfung", "38105|12160|Allgemeine Betriebswirtschaftslehre I: Grundlagen der BWL")
}

// qisPrerequisiteRows are the two rows of a QIS description that state prerequisites.
func qisPrerequisiteRows(recommended, mandatory string) string {
	return `<tr><td class="tabelle1_alignleft">Empfohlene Voraussetzungen:</td><td class="tabelle2inhalt">` + recommended + `</td></tr>
		<tr><td class="tabelle1_alignleft">Zwingende Voraussetzungen:</td><td class="tabelle2inhalt">` + mandatory + `</td></tr>`
}

// A module that phases out is no prerequisite of the module that replaces it (owner,
// 2026-10-16): 11903 replaced 12212, which replaced 12103. 11903 rules out 12212 in its text
// („Keine erfolgreiche Teilnahme am zugehörigen Auslaufmodul 12212"), 12212 rules out 11903, and
// 12103 is left out even where a text names it without ruling it out. 12101 is a prerequisite.
func TestBuildKeepsReplacedModulesOutOfThePrerequisites(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "prerequisites.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 10, 16, 7, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceQISModuleList, "rows-000000", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>11903</td><td><a href="/rds?pord.pordnr=13000">Digitaltechnik</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12044</td><td><a href="/rds?pord.pordnr=13001">Rechnerarchitektur</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12101</td><td><a href="/rds?pord.pordnr=13002">Algorithmieren und Programmieren</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12103</td><td><a href="/rds?pord.pordnr=13003">Technische Informatik</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
		<tr><td>12212</td><td><a href="/rds?pord.pordnr=13971">Digitaltechnik</a></td><td>Deutsch</td><td>6</td><td></td><td></td></tr>
	</table>`)
	put(catalogdb.SourceQISModulePage, "11903", qisModulePage("11903", "Digitaltechnik", "6", "",
		qisPrerequisiteRows("Kenntnis des Stoffes der Module 12101 und 12103.",
			"Keine erfolgreiche Teilnahme am zugehörigen Auslaufmodul <em>12212 - Digitaltechnik.</em>")))
	put(catalogdb.SourceQISModulePage, "12212", qisModulePage("12212 - Auslaufmodul", "Digitaltechnik", "6", "",
		qisPrerequisiteRows("Kenntnis von elektrischen und elektronischen Grundlagen, Modul 12103.",
			"Keine erfolgreiche Teilnahme am zugehörigen Nachfolgemodul <em>11903 - Digitaltechnik</em>."),
		qisReplacementRow("Nachfolgemodul/e", "Auslaufmodul ab: 23.05.2019", "11903", "13000")))
	put(catalogdb.SourceQISModulePage, "12103", qisModulePage("12103 - Auslaufmodul", "Technische Informatik", "6", "",
		qisReplacementRow("Nachfolgemodul/e", "Auslaufmodul ab: 01.10.2015", "12212", "13971")))
	put(catalogdb.SourceQISModulePage, "12044", qisModulePage("12044", "Rechnerarchitektur", "6", "",
		qisPrerequisiteRows("Kenntnis des Stoffes von Modul 11903 Digitaltechnik.", "keine")))
	put(catalogdb.SourceQISModulePage, "12101", qisModulePage("12101", "Algorithmieren und Programmieren", "6", ""))

	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build failed: %v", err)
	}

	want(t, db, "SELECT module_id, successor_id FROM module_successor ORDER BY module_id", "12103|12212", "12212|11903")
	want(t, db, "SELECT module_id, required_module_id, kind FROM module_prerequisite ORDER BY 1, 2",
		"11903|12101|recommended", "12044|11903|recommended")
	// The texts stay as the page states them.
	want(t, db, "SELECT prerequisites_mandatory FROM module WHERE id = '11903'",
		"Keine erfolgreiche Teilnahme am zugehörigen Auslaufmodul *12212 - Digitaltechnik.*")
}
