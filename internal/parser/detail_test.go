package parser

import (
	"slices"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/internal/model"
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
	if len(detail.ResponsiblePersons) != 1 {
		t.Fatalf("unexpected responsible persons length: %d", len(detail.ResponsiblePersons))
	}
	if detail.ResponsiblePersons[0].Title != "Prof. Dr." || detail.ResponsiblePersons[0].Name != "Köhler, Ekkehard" {
		t.Errorf("unexpected responsible person: %+v", detail.ResponsiblePersons[0])
	}
	if detail.PrerequisitesMandatory != "-" {
		t.Errorf("expected PrerequisitesMandatory '-', got %q", detail.PrerequisitesMandatory)
	}
	if detail.PrerequisitesRecommended != "Schulmathematik" {
		t.Errorf("expected PrerequisitesRecommended 'Schulmathematik', got %q", detail.PrerequisitesRecommended)
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

func TestDetailParser_NotOffered_Successors_Events_Titles(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<div class="tx-btusysteme">
			<h1>12345 - Ausgelaufene Vorlesung <small>Modulübersicht</small></h1>
			<table>
				<tr>
					<td>Modulnummer:</td>
					<td><b>12345 - Modul nicht mehr im Angebot</b></td>
				</tr>
				<tr>
					<td>Modultitel:</td>
					<td><b>Ausgelaufene Vorlesung</b></td>
				</tr>
				<tr>
					<td>Verantwortlich:</td>
					<td><ul>
						<li>Schmid, Reiner , Prof. Dr. rer. nat.</li>
						<li>Glemser, Wolfgang, Prof.</li>
					</ul></td>
				</tr>
				<tr>
					<td>Angebotsturnus:</td>
					<td>kein Lehrangebot mehr</td>
				</tr>
				<tr>
					<td>Empfohlene Voraussetzungen:</td>
					<td>keine</td>
				</tr>
				<tr>
					<td>Zwingende Voraussetzungen:</td>
					<td></td>
				</tr>
				<tr>
					<td>Nachfolgemodul:</td>
					<td>12938</td>
				</tr>
				<tr>
					<td>Bemerkungen:</td>
					<td>Das Modul wird ersetzt durch Nachfolgemodul 12939.</td>
				</tr>
				<tr>
					<td>Veranstaltungen im aktuellen Semester:</td>
					<td>keine Zuordnung vorhanden</td>
				</tr>
			</table>
		</div>
	</body>
	</html>
	`

	p := NewDetailParser()
	detail, err := p.Parse(strings.NewReader(htmlSnippet), "12345", "https://www.b-tu.de/modul/12345")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !detail.IsNotOffered {
		t.Errorf("expected IsNotOffered to be true")
	}
	if !detail.IsPhaseOut {
		t.Errorf("expected IsPhaseOut to be true")
	}

	// Responsible persons parsing
	if len(detail.ResponsiblePersons) != 2 {
		t.Fatalf("expected 2 responsible persons, got %d", len(detail.ResponsiblePersons))
	}
	if detail.ResponsiblePersons[0].Name != "Schmid, Reiner" || detail.ResponsiblePersons[0].Title != "Prof. Dr. rer. nat." {
		t.Errorf("unexpected responsible person 0: %+v", detail.ResponsiblePersons[0])
	}
	if detail.ResponsiblePersons[1].Name != "Glemser, Wolfgang" || detail.ResponsiblePersons[1].Title != "Prof." {
		t.Errorf("unexpected responsible person 1: %+v", detail.ResponsiblePersons[1])
	}

	// Prerequisites defaults
	if detail.PrerequisitesRecommended != "-" {
		t.Errorf("expected PrerequisitesRecommended '-', got %q", detail.PrerequisitesRecommended)
	}
	if detail.PrerequisitesMandatory != "-" {
		t.Errorf("expected PrerequisitesMandatory '-', got %q", detail.PrerequisitesMandatory)
	}

	// Successor modules from both row and remarks
	if len(detail.SuccessorModules) != 2 {
		t.Fatalf("expected 2 successor modules, got %d: %v", len(detail.SuccessorModules), detail.SuccessorModules)
	}
	if detail.SuccessorModules[0] != "12938" || detail.SuccessorModules[1] != "12939" {
		t.Errorf("unexpected successor modules: %v", detail.SuccessorModules)
	}

	// Current semester events should be empty due to "keine Zuordnung vorhanden" and lack of <a> tags
	if len(detail.CurrentSemesterEvents) != 0 {
		t.Errorf("expected 0 current semester events, got %d: %+v", len(detail.CurrentSemesterEvents), detail.CurrentSemesterEvents)
	}
}

// A remark names a successor („Siehe Nachfolge-Modul 11523", „stattdessen Nachfolgemodul
// 11787") or the module this one succeeds („Nachfolgemodul zu 31423", „Nachfolgemodul für
// Modul 24410"). The remarks of b-tu.de/modul/11240, 12215, 11364 and 12046, 2026-09-29; read
// the second way as well, 11364 and 31423 named each other as successor.
func TestDetailParser_RemarksNameSuccessorOrPredecessor(t *testing.T) {
	for _, tc := range []struct {
		id, remarks              string
		phaseOut                 bool
		successors, predecessors []string
	}{
		{"11240", "B.Sc. Bauingenieurwesen PO 2011: BDGI 13\nLehrveranstaltungen (Vorlesung/ Übung) werden nur noch im SS angeboten.\nSiehe Nachfolge-Modul 11523\nBei Fragen am Fachgebiet Bauphysik und Gebäudetechnik melden.",
			true, []string{"11523"}, nil},
		{"12215", "Modul wird nicht im WS 17/18 angeboten, stattdessen Nachfolgemodul 11787 Theoretische Informatik besuchen.",
			true, []string{"11787"}, nil},
		{"11364", "Nachfolgemodul zu 31423: Technische Akustik und Strömungsakustik,\nergänzt sich mit dem Modul 11365: Technische Akustik - Schallfelder (im Wintersemester)\nKein Lehrangebot im Sommersemester 2019!",
			false, nil, []string{"31423"}},
		{"12046", "Nachfolgemodul für Modul 24410 \"Stadtmanagement\".",
			false, nil, []string{"24410"}},
	} {
		page := `<div class="tx-btusysteme"><table>
			<tr><td>Modulnummer:</td><td>` + tc.id + `</td></tr>
			<tr><td>Bemerkungen:</td><td>` + tc.remarks + `</td></tr>
			</table></div>`
		d, err := NewDetailParser().Parse(strings.NewReader(page), tc.id, "u")
		if err != nil {
			t.Fatalf("%s: Parse: %v", tc.id, err)
		}
		if d.IsPhaseOut != tc.phaseOut {
			t.Errorf("%s: IsPhaseOut = %v, want %v", tc.id, d.IsPhaseOut, tc.phaseOut)
		}
		if !slices.Equal(d.SuccessorModules, tc.successors) || !slices.Equal(d.PredecessorModules, tc.predecessors) {
			t.Errorf("%s: successors %v, replaces %v; want %v, %v", tc.id, d.SuccessorModules, d.PredecessorModules, tc.successors, tc.predecessors)
		}
	}
}

func TestSplitResponsiblePerson_ComplexTitles(t *testing.T) {
	tests := []struct {
		input     string
		wantTitle string
		wantName  string
	}{
		{"apl. Prof. Dr. rer. nat. habil. Felgenhauer, Ursula", "apl. Prof. Dr. rer. nat. habil.", "Felgenhauer, Ursula"},
		{"nat. habil. Müller, Peter", "nat. habil.", "Müller, Peter"},
		{"Prof. Dr. rer. publ . Dr. h. c. Knopp, Lothar", "Prof. Dr. rer. publ. Dr. h. c.", "Knopp, Lothar"},
		{"apl. Prof. PD Dr. rer. nat. habil. Schaaf, Wolfgang", "apl. Prof. PD Dr. rer. nat. habil.", "Schaaf, Wolfgang"},
		{"Prof. Dr. -Ing. Woll, Ralf", "Prof. Dr.-Ing.", "Woll, Ralf"},
		{"Prof. Dr. Dr.h.c. (NMU, UA) Schmidt, Michael", "Prof. Dr. Dr.h.c. (NMU, UA)", "Schmidt, Michael"},
		{"apl. Prof. Dr. sc. nat. Kittler, Martin", "apl. Prof. Dr. sc. nat.", "Kittler, Martin"},
		{"Prof. Dr. -Ing. habil. König, Hartmut", "Prof. Dr.-Ing. habil.", "König, Hartmut"},
		{"PD Dr. -Ing. Müller, Hans", "PD Dr.-Ing.", "Müller, Hans"},
		{"Gastprofessor Dr.-Ing. Wagener-Lohse, Georg", "Gastprofessor Dr.-Ing.", "Wagener-Lohse, Georg"},
		{"Dr. rer. nat . Will, Andreas", "Dr. rer. nat.", "Will, Andreas"},
		{"Prof. Dipl.-Ing. Nagler, Heinz", "Prof. Dipl.-Ing.", "Nagler, Heinz"},
		{"Prof. Dr. rer. nat. habil Meer, Klaus", "Prof. Dr. rer. nat. habil", "Meer, Klaus"},
		{"Prof.Dr.rer.nat.habil.Dr.h.c. Sigmund, Ernst", "Prof.Dr.rer.nat.habil.Dr.h.c.", "Sigmund, Ernst"},
	}

	for _, tc := range tests {
		got := model.SplitResponsiblePerson(tc.input)
		if got.Title != tc.wantTitle {
			t.Errorf("input %q: expected Title %q, got %q", tc.input, tc.wantTitle, got.Title)
		}
		if got.Name != tc.wantName {
			t.Errorf("input %q: expected Name %q, got %q", tc.input, tc.wantName, got.Name)
		}
	}
}
