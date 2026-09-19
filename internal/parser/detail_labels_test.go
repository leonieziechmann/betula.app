package parser

import (
	"strings"
	"testing"
)

// Labels and values as served by b-tu.de/modul/<id> on 2026-09-19.
const germanExamRows = `
<div class="tx-btusysteme"><h1>11101 - Lineare Algebra</h1><table>
	<tr><td>Modulprüfung:</td><td>Voraussetzung + Modulabschlussprüfung (MAP)</td></tr>
	<tr><td>Prüfungsleistung/en für Modulprüfung:</td><td><b>Modulabschlussprüfung:</b> Klausur, 90 min.</td></tr>
	<tr><td>Bewertung der Modulprüfung:</td><td>Prüfungsleistung - benotet</td></tr>
	<tr><td>Teilnehmerbeschränkung:</td><td>keine</td></tr>
	<tr><td>Zuordnung zu Studiengängen:</td><td><ul>
		<li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li>
		<li>LA Bachelor Grundstufe/Primarstufe / Lehramt Primarstufe Deutsch-Englisch / PO 2025</li>
		<li>Abschluss im Ausland / Informatik / keine PO</li>
	</ul></td></tr>
	<tr><td>&nbsp;</td><td>Das Modul ist für das Fachübergreifende Studium zugelassen.</td></tr>
</table></div>`

const englishPage = `
<div class="tx-btusysteme"><h1>11881 - Foundations of Data Mining</h1><table>
	<tr><td>Module Number:</td><td>11881</td></tr>
	<tr><td>Module Examination:</td><td>Prerequisite + Final Module Examination (MAP)</td></tr>
	<tr><td>Assessment Mode for Module Examination:</td><td>Written examination, 90 min.</td></tr>
	<tr><td>Evaluation of Module Examination:</td><td>Performance Verification – graded</td></tr>
	<tr><td>Limited Number of Participants:</td><td>80</td></tr>
	<tr><td>Part of the Study Programme:</td><td><ul>
		<li>Bachelor (research-oriented) / Informatik / PO 2008 - 2. SÄ 2024</li>
		<li>Master (research-oriented) / Artificial Intelligence / PO 2022</li>
	</ul></td></tr>
	<tr><td>&nbsp;</td><td>This module has been approved for the general studies.</td></tr>
	<tr><td>Remarks:</td><td>Study programme Informatik B.Sc.: Compulsory elective module</td></tr>
	<tr><td>Module Components:</td><td><ul><li>Lecture Foundations of Data Mining</li></ul></td></tr>
	<tr><td>Components to be offered in the Current Semester:</td><td><ul>
		<li><a href="https://www.b-tu.de/qisserver3/rds?veranstaltung.veranstid=120285">120285 Examination</a></li>
	</ul></td></tr>
</table></div>`

func TestDetailParser_ExamRowsStaySeparate(t *testing.T) {
	for name, tc := range map[string]struct {
		html                               string
		examType, examDetailsPart, grading string
	}{
		"german":  {germanExamRows, "Voraussetzung + Modulabschlussprüfung (MAP)", "Klausur, 90 min.", "Prüfungsleistung - benotet"},
		"english": {englishPage, "Prerequisite + Final Module Examination (MAP)", "Written examination", "Performance Verification – graded"},
	} {
		d, err := NewDetailParser().Parse(strings.NewReader(tc.html), "", "")
		if err != nil {
			t.Fatalf("%s: Parse failed: %v", name, err)
		}
		if d.ExamType != tc.examType {
			t.Errorf("%s: ExamType = %q, want %q", name, d.ExamType, tc.examType)
		}
		if !strings.Contains(d.ExamDetails, tc.examDetailsPart) {
			t.Errorf("%s: ExamDetails = %q, want it to contain %q", name, d.ExamDetails, tc.examDetailsPart)
		}
		if d.Grading != tc.grading {
			t.Errorf("%s: Grading = %q, want %q", name, d.Grading, tc.grading)
		}
	}
}

func TestDetailParser_EnglishLabels(t *testing.T) {
	d, err := NewDetailParser().Parse(strings.NewReader(englishPage), "11881", "")
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}
	if d.Limitation != "80" {
		t.Errorf("Limitation = %q, want 80", d.Limitation)
	}
	if !d.CrossDisciplinary {
		t.Error("CrossDisciplinary = false, want true (approved for the general studies)")
	}
	if len(d.StudyPrograms) != 2 || d.StudyPrograms[0].Degree != "Bachelor (research-oriented)" ||
		d.StudyPrograms[0].Program != "Informatik" || d.StudyPrograms[0].Regulation != "PO 2008 - 2. SÄ 2024" {
		t.Errorf("StudyPrograms = %+v", d.StudyPrograms)
	}
	if len(d.AssociatedCourses) != 1 {
		t.Errorf("AssociatedCourses = %v, want 1 entry", d.AssociatedCourses)
	}
	if len(d.CurrentSemesterEvents) != 1 || !strings.Contains(d.CurrentSemesterEvents[0].URL, "veranstid=120285") {
		t.Errorf("CurrentSemesterEvents = %+v", d.CurrentSemesterEvents)
	}
}

func TestDetailParser_NoAssignmentPlaceholders(t *testing.T) {
	page := `<div class="tx-btusysteme"><table>
		<tr><td>Components to be offered in the Current Semester:</td><td><ul><li>no assignment</li></ul></td></tr>
	</table></div>`
	d, err := NewDetailParser().Parse(strings.NewReader(page), "11377", "")
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}
	if len(d.CurrentSemesterEvents) != 0 {
		t.Errorf("CurrentSemesterEvents = %+v, want none", d.CurrentSemesterEvents)
	}
}

func TestParseStudyProgram_SlashInsideValues(t *testing.T) {
	d, err := NewDetailParser().Parse(strings.NewReader(germanExamRows), "11101", "")
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}
	if len(d.StudyPrograms) != 3 {
		t.Fatalf("StudyPrograms = %+v, want 3", d.StudyPrograms)
	}
	la := d.StudyPrograms[1]
	if la.Degree != "LA Bachelor Grundstufe/Primarstufe" || la.Program != "Lehramt Primarstufe Deutsch-Englisch" || la.Regulation != "PO 2025" {
		t.Errorf("Lehramt entry = %+v", la)
	}

	sp := parseStudyProgram("Master (universitär) / Wirtschaft / Recht / PO 2020")
	if sp.Degree != "Master (universitär)" || sp.Program != "Wirtschaft / Recht" || sp.Regulation != "PO 2020" {
		t.Errorf("program name with separator = %+v", sp)
	}
}
