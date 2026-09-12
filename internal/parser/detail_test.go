package parser

import (
	"strings"
	"testing"
)

func TestDetailParser_German(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<div class="tx-btusysteme">
			<h1>11101 - Lineare Algebra und analytische Geometrie I <small>Modulübersicht</small></h1>
			<table>
				<tr>
					<td>Modulnummer:</td>
					<td><b>11101</b></td>
				</tr>
				<tr>
					<td>Modultitel:</td>
					<td><b>Lineare Algebra und analytische Geometrie I</b></td>
				</tr>
				<tr>
					<td>&nbsp;</td>
					<td>Linear Algebra and Analytical Geometry I</td>
				</tr>
				<tr>
					<td>Einrichtung:</td>
					<td>Fakultät 1 - MINT</td>
				</tr>
				<tr>
					<td>Verantwortlich:</td>
					<td><ul><li>Prof. Dr. Köhler, Ekkehard</li></ul></td>
				</tr>
				<tr>
					<td>Lehr- und Prüfungssprache:</td>
					<td>Deutsch</td>
				</tr>
				<tr>
					<td>Dauer:</td>
					<td>1 Semester</td>
				</tr>
				<tr>
					<td>Angebotsturnus:</td>
					<td>jedes Wintersemester</td>
				</tr>
				<tr>
					<td>Leistungspunkte:</td>
					<td>8</td>
				</tr>
				<tr>
					<td>Lernziele:</td>
					<td>Die Studierenden sollen sichere Kenntnisse erwerben.</td>
				</tr>
				<tr>
					<td>Inhalte:</td>
					<td><ul><li>Vektorräume</li><li>Lineare Abbildungen</li></ul></td>
				</tr>
				<tr>
					<td>Empfohlene Voraussetzungen:</td>
					<td>Schulmathematik</td>
				</tr>
				<tr>
					<td>Zwingende Voraussetzungen:</td>
					<td>keine</td>
				</tr>
				<tr>
					<td>Lehrformen und Arbeitsumfang:</td>
					<td><ul><li>Vorlesung / 4 SWS</li><li>Übung / 2 SWS</li><li>Selbststudium / 150 Stunden</li></ul></td>
				</tr>
				<tr>
					<td>Unterrichtsmaterialien und Literaturhinweise:</td>
					<td><ul><li>Fischer, Gerd: Lineare Algebra</li></ul></td>
				</tr>
				<tr>
					<td>Modulprüfung:</td>
					<td>Voraussetzung + MAP</td>
				</tr>
				<tr>
					<td>Prüfungsleistung/en für Modulprüfung:</td>
					<td>Klausur, 90 min.</td>
				</tr>
				<tr>
					<td>Bewertung der Modulprüfung:</td>
					<td>benotet</td>
				</tr>
				<tr>
					<td>Teilnehmerbeschränkung:</td>
					<td>keine</td>
				</tr>
				<tr>
					<td>Zuordnung zu Studiengängen:</td>
					<td><ul><li>Bachelor (universitär) / Mathematik / PO 2023</li><li>Bachelor (universitär) / Physik / PO 2021</li></ul></td>
				</tr>
				<tr>
					<td>&nbsp;</td>
					<td>Das Modul ist für das Fachübergreifende Studium zugelassen.</td>
				</tr>
				<tr>
					<td>Veranstaltungen im aktuellen Semester:</td>
					<td><ul><li><a href="https://qis.b-tu.de/event1">130497 Prüfung Lineare Algebra</a></li></ul></td>
				</tr>
			</table>
		</div>
	</body>
	</html>
	`

	p := NewDetailParser()
	detail, err := p.Parse(strings.NewReader(htmlSnippet), "11101", "https://www.b-tu.de/modul/11101")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if detail.ID != "11101" {
		t.Errorf("expected ID 11101, got %s", detail.ID)
	}
	if detail.TitleDE != "Lineare Algebra und analytische Geometrie I" {
		t.Errorf("expected TitleDE 'Lineare Algebra und analytische Geometrie I', got %s", detail.TitleDE)
	}
	if detail.TitleEN != "Linear Algebra and Analytical Geometry I" {
		t.Errorf("expected TitleEN 'Linear Algebra and Analytical Geometry I', got %s", detail.TitleEN)
	}
	if detail.Department != "Fakultät 1 - MINT" {
		t.Errorf("expected Department 'Fakultät 1 - MINT', got %s", detail.Department)
	}
	if len(detail.ResponsiblePersons) != 1 || detail.ResponsiblePersons[0] != "Prof. Dr. Köhler, Ekkehard" {
		t.Errorf("unexpected responsible persons: %v", detail.ResponsiblePersons)
	}
	if detail.Credits != 8.0 {
		t.Errorf("expected Credits 8.0, got %f", detail.Credits)
	}
	if len(detail.TeachingForms) != 3 {
		t.Fatalf("expected 3 teaching forms, got %d", len(detail.TeachingForms))
	}
	if detail.TeachingForms[0].Type != "Vorlesung" || detail.TeachingForms[0].Workload != "4 SWS" {
		t.Errorf("unexpected teaching form 0: %+v", detail.TeachingForms[0])
	}
	if len(detail.StudyPrograms) != 2 {
		t.Fatalf("expected 2 study programs, got %d", len(detail.StudyPrograms))
	}
	if detail.StudyPrograms[0].Degree != "Bachelor (universitär)" || detail.StudyPrograms[0].Program != "Mathematik" || detail.StudyPrograms[0].Regulation != "PO 2023" {
		t.Errorf("unexpected study program 0: %+v", detail.StudyPrograms[0])
	}
	if !detail.CrossDisciplinary {
		t.Errorf("expected CrossDisciplinary to be true")
	}
	if len(detail.CurrentSemesterEvents) != 1 {
		t.Fatalf("expected 1 current event, got %d", len(detail.CurrentSemesterEvents))
	}
	if detail.CurrentSemesterEvents[0].URL != "https://qis.b-tu.de/event1" {
		t.Errorf("unexpected event url: %s", detail.CurrentSemesterEvents[0].URL)
	}
}

func TestDetailParser_EnglishAndPhaseOut(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<div class="tx-btusysteme">
			<h1>11155 - Embedded Software Technology <small>Modulübersicht</small></h1>
			<table>
				<tr>
					<td>Module Number:</td>
					<td><b>11155 - Phase-out Module</b></td>
				</tr>
				<tr>
					<td>Module Title:</td>
					<td><b>Embedded Software Technology</b></td>
				</tr>
				<tr>
					<td>&nbsp;</td>
					<td>Technologie eingebetteter Software</td>
				</tr>
				<tr>
					<td>Department:</td>
					<td>Faculty 1 - Mathematics and Computer Science</td>
				</tr>
				<tr>
					<td>Credits:</td>
					<td>6</td>
				</tr>
				<tr>
					<td>Language of Teaching / Examination:</td>
					<td>English</td>
				</tr>
			</table>
		</div>
	</body>
	</html>
	`

	p := NewDetailParser()
	detail, err := p.Parse(strings.NewReader(htmlSnippet), "11155", "https://www.b-tu.de/modul/11155")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if detail.ID != "11155" {
		t.Errorf("expected ID 11155, got %s", detail.ID)
	}
	if !detail.IsPhaseOut {
		t.Errorf("expected IsPhaseOut to be true")
	}
	if detail.Credits != 6.0 {
		t.Errorf("expected Credits 6.0, got %f", detail.Credits)
	}
	if detail.Language != "English" {
		t.Errorf("expected Language English, got %s", detail.Language)
	}
}
