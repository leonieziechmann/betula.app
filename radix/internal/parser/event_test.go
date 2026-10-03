package parser

import (
	"strings"
	"testing"
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
