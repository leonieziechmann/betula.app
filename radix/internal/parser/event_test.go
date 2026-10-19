package parser

import (
	"bytes"
	"os"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/model"
)

func TestEventParser(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<h1>Einführung in die Programmierung - C++ (SFB) - Einzelansicht</h1>
		<table summary="Grunddaten zur Veranstaltung">
			<caption>Grunddaten</caption>
			<tr>
				<th>Veranstaltungsart</th>
				<td>Vorlesung</td>
			</tr>
			<tr>
				<th>Veranstaltungsnummer</th>
				<td>140037</td>
			</tr>
			<tr>
				<th>Semester</th>
				<td>SS 2026</td>
				<th>SWS</th>
				<td>2</td>
			</tr>
			<tr>
				<th>Erwartete Teilnehmer/-innen</th>
				<td>40</td>
			</tr>
		</table>

		<table summary="Übersicht über alle Veranstaltungstermine">
			<caption>Termine Gruppe: [unbenannt]</caption>
			<tr>
				<th>&nbsp;</th>
				<th>Tag</th>
				<th>Zeit</th>
				<th>Rhythmus</th>
				<th>Dauer</th>
				<th>Raum</th>
				<th>Raumplan</th>
				<th>Lehrperson</th>
				<th>Bemerkung</th>
				<th>fällt aus am</th>
			</tr>
			<tr>
				<td>Icon</td>
				<td>Di.</td>
				<td>10:00 bis 11:30</td>
				<td>wöchentlich</td>
				<td>14.04.2026 bis 21.07.2026</td>
				<td><a href="https://qis.b-tu.de/raum2922">Gebäude 6 - SFB - 6.210</a></td>
				<td>plan</td>
				<td><a href="https://qis.b-tu.de/person6132">Reichelt</a></td>
				<td>Hörsaalübung</td>
				<td>&nbsp;</td>
			</tr>
		</table>

		<table summary="Verantwortliche Dozenten">
			<caption>Zugeordnete Personen</caption>
			<tr>
				<td><a href="https://qis.b-tu.de/person6024">Irrgang, Kai-Uwe</a></td>
				<td>begleitend</td>
			</tr>
			<tr>
				<td><a href="https://qis.b-tu.de/person6132">Reichelt, Steffen</a></td>
				<td>verantwort</td>
			</tr>
		</table>

		<table summary="Übersicht über die zugehörigen Prüfungen">
			<caption>Gehört zu Modul</caption>
			<tr>
				<th>Modulnummer</th>
				<th>Modultitel</th>
			</tr>
			<tr>
				<td>12105</td>
				<td>Einführung in die Programmierung</td>
			</tr>
			<tr>
				<td>11826</td>
				<td>Informatik 1</td>
			</tr>
		</table>
	</body>
	</html>
	`

	p := NewEventParser()
	event, err := p.Parse(strings.NewReader(htmlSnippet), "147828", "https://b-tu.de/qis?veranstaltung.veranstid=147828")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if event.ID != "147828" {
		t.Errorf("expected ID 147828, got %s", event.ID)
	}
	if event.Title != "Einführung in die Programmierung - C++ (SFB)" {
		t.Errorf("expected title 'Einführung in die Programmierung - C++ (SFB)', got %s", event.Title)
	}
	if event.EventType != "Vorlesung" {
		t.Errorf("expected EventType 'Vorlesung', got %s", event.EventType)
	}
	if event.EventNumber != "140037" {
		t.Errorf("expected EventNumber '140037', got %s", event.EventNumber)
	}
	if event.Semester != "SS 2026" {
		t.Errorf("expected Semester 'SS 2026', got %s", event.Semester)
	}
	if event.SWS != "2" {
		t.Errorf("expected SWS '2', got %s", event.SWS)
	}
	if event.ExpectedParticipants != "40" {
		t.Errorf("expected ExpectedParticipants '40', got %s", event.ExpectedParticipants)
	}

	// Schedule
	if len(event.Schedules) != 1 {
		t.Fatalf("expected 1 schedule, got %d", len(event.Schedules))
	}
	sched := event.Schedules[0]
	if sched.DayOfWeek != "Di." {
		t.Errorf("expected DayOfWeek 'Di.', got %s", sched.DayOfWeek)
	}
	if sched.StartTime != "10:00" || sched.EndTime != "11:30" {
		t.Errorf("expected 10:00 - 11:30, got %s - %s", sched.StartTime, sched.EndTime)
	}
	if sched.Rhythm != "wöchentlich" {
		t.Errorf("expected rhythm 'wöchentlich', got %s", sched.Rhythm)
	}
	if sched.Room != "Gebäude 6 - SFB - 6.210" {
		t.Errorf("expected room 'Gebäude 6 - SFB - 6.210', got %s", sched.Room)
	}
	if sched.RoomURL != "https://qis.b-tu.de/raum2922" {
		t.Errorf("expected room URL, got %s", sched.RoomURL)
	}
	if sched.Instructor != "Reichelt" {
		t.Errorf("expected instructor 'Reichelt', got %s", sched.Instructor)
	}
	if sched.Comment != "Hörsaalübung" {
		t.Errorf("expected comment 'Hörsaalübung', got %s", sched.Comment)
	}

	// Responsible Persons
	if len(event.ResponsiblePersons) != 2 {
		t.Fatalf("expected 2 responsible persons, got %d", len(event.ResponsiblePersons))
	}
	if event.ResponsiblePersons[1].Name != "Reichelt, Steffen" || event.ResponsiblePersons[1].Role != "verantwort" {
		t.Errorf("unexpected responsible person 1: %+v", event.ResponsiblePersons[1])
	}

	// Associated Modules
	if len(event.AssociatedModules) != 2 {
		t.Fatalf("expected 2 associated modules, got %d", len(event.AssociatedModules))
	}
	if event.AssociatedModules[0] != "12105" || event.AssociatedModules[1] != "11826" {
		t.Errorf("unexpected associated modules: %v", event.AssociatedModules)
	}
}

// The two Programmierpraktika of 2026W (12102, owner 2026-10-19): QIS states on each event page
// whom it is for. testdata/qis_event_148008.html is the lecture at the Zentralcampus,
// testdata/qis_event_149408.html the one in Sachsendorf, both cut to the content (2026-10-19).
func TestEventPrograms(t *testing.T) {
	read := func(id string) *model.EventDetail {
		t.Helper()
		body, err := os.ReadFile("testdata/qis_event_" + id + ".html")
		if err != nil {
			t.Fatal(err)
		}
		d, err := NewEventParser().Parse(bytes.NewReader(body), id, "")
		if err != nil {
			t.Fatal(err)
		}
		return d
	}

	central := read("148008")
	want := []model.EventProgram{
		{Label: "Physik Bachelor (universitäres Profil)", Semesters: "5 - 5", QISID: "253"},
		{Label: "Informatik Bachelor (universitäres Profil)", Semesters: "1 - 1", QISID: "71"},
		{Label: "Künstliche Intelligenz Bachelor", Semesters: "1 - 1", PO: "2022", QISID: "448"},
		{Label: "Künstliche Intelligenz Technologie Bachelor", Semesters: "1 - 1", PO: "2022", QISID: "449"},
	}
	if len(central.Programs) != 7 {
		t.Fatalf("programs of 148008: %d, want 7: %+v", len(central.Programs), central.Programs)
	}
	for i, w := range want {
		if central.Programs[i] != w {
			t.Errorf("program %d of 148008: %+v, want %+v", i, central.Programs[i], w)
		}
	}
	if last := central.Programs[6]; last.Label != "Wirtschaftsinformatik Bachelor" || last.PO != "2024" {
		t.Errorf("last program of 148008: %+v", last)
	}

	sachsendorf := read("149408")
	if len(sachsendorf.Programs) != 1 || sachsendorf.Programs[0].Label != "Medizininformatik Bachelor (universitäres Profil)" || sachsendorf.Programs[0].Semesters != "1 - 1" {
		t.Errorf("programs of 149408: %+v", sachsendorf.Programs)
	}
	if sachsendorf.SWS != "1" || len(sachsendorf.Schedules) != 1 {
		t.Errorf("149408: SWS %q, %d dates", sachsendorf.SWS, len(sachsendorf.Schedules))
	}

	// A page whose table has no programs, and the rows of a page without the section, are none.
	empty := `<html><body><h1>X - Einzelansicht</h1><table summary="Übersicht über die zugehörigen Studiengänge"><caption>Studiengänge</caption>
<tr><th>Studiengang</th><th>Semester</th><th>PO</th><th>Bemerkung</th></tr></table></body></html>`
	d, err := NewEventParser().Parse(strings.NewReader(empty), "1", "")
	if err != nil || len(d.Programs) != 0 {
		t.Errorf("an empty table: %+v, %v", d.Programs, err)
	}
}
