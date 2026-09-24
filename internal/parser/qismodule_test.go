package parser

import (
	"os"
	"strings"
	"testing"
)

// The shape of a QIS module description: rows of a label cell and a value cell,
// lists in <ul>, the English title in the row below the German one without a
// label of its own, and the events of the current semester as links.
const qisModuleFixture = `<html><body>
<div><table cellpadding="5">
 <tr><td class="tabelle1_alignleft" width="20%">Modulnummer:</td>
     <td class="tabelle2inhalt" width="80%"><b>11289
       </b></td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">Modultitel:</td>
     <td class="tabelle2inhalt" width="80%"><b>Softwaretechnik</b></td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">&nbsp;</td>
     <td class="tabelle2inhalt" width="80%">
       Software Engineering
     </td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">Einrichtung:</td>
     <td class="tabelle2inhalt" width="80%">Fakultät 1 - MINT</td></tr>
 <TR><TD class="tabelle1_alignleft" width="30%">Verantwortlich:</TD>
     <TD class="tabelle2inhalt" width="70%"><UL><li>Prof. Dr. rer. nat. Lambers, Leen</li></UL></TD></TR>
 <tr><td class="tabelle1_alignleft" width="20%">Lehr- und Prüfungssprache:</td>
     <td class="tabelle2inhalt" width="80%">Deutsch</td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">Dauer:</td>
     <td class="tabelle2inhalt" width="80%">1 Semester</td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">Angebotsturnus:</td>
     <td class="tabelle2inhalt" width="80%">jedes Sommersemester</td></tr>
 <tr><td class="tabelle1_alignleft" width="20%">Leistungspunkte:</td>
     <td class="tabelle2inhalt" width="80%">8</td></tr>
 <TR><TD class="tabelle1_alignleft" width="30%">Lehrformen und Arbeitsumfang:</TD>
     <TD class="tabelle2inhalt" width="70%"><UL><li>Vorlesung / 4 SWS</li><li>Übung / 2 SWS</li></UL></TD></TR>
 <tr><td class="tabelle1_alignleft" width="30%">Modulprüfung:</td>
     <td class="tabelle2inhalt" width="70%">Voraussetzung + Modulabschlussprüfung (MAP)</td></tr>
 <tr><td class="tabelle1_alignleft" width="30%">Teilnehmerbeschränkung:</td>
     <td class="tabelle2inhalt" width="70%">keine</td></tr>
 <TR><TD class="tabelle1_alignleft" width="30%">Zuordnung zu Studiengängen:</TD>
     <TD class="tabelle2inhalt" width="70%"><UL>
       <li>Master (universitär) / Artificial Intelligence / PO 2022 - 1. SÄ 2024</li>
       <li>Abschluss im Ausland / Informatik / keine PO</li></UL></TD></TR>
 <TR><TD class="tabelle1_alignleft" width="30%">Veranstaltungen im aktuellen Semester:</TD>
     <TD class="tabelle2inhalt" width="70%"><UL>
       <li><a class="regular" href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;veranstaltung.veranstid=153213">
         120632 Prüfung Softwaretechnik (Wiederholung)</a></li></UL></TD></TR>
</table></div>
<table><tr><td>a navigation table</td><td>without the classes</td></tr></table>
</body></html>`

func TestQISModuleParser(t *testing.T) {
	d, err := NewQISModuleParser().Parse(strings.NewReader(qisModuleFixture), "11289", "https://qis.example/module")
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}

	if d.ID != "11289" {
		t.Errorf("ID = %q", d.ID)
	}
	if d.TitleDE != "Softwaretechnik" || d.TitleEN != "Software Engineering" {
		t.Errorf("titles = %q / %q", d.TitleDE, d.TitleEN)
	}
	if d.Department != "Fakultät 1 - MINT" || d.Language != "Deutsch" || d.Duration != "1 Semester" {
		t.Errorf("department/language/duration = %q / %q / %q", d.Department, d.Language, d.Duration)
	}
	if d.Turnus != "jedes Sommersemester" || d.Credits != 8 {
		t.Errorf("turnus/credits = %q / %v", d.Turnus, d.Credits)
	}
	if len(d.ResponsiblePersons) != 1 || !strings.Contains(d.ResponsiblePersons[0].Name, "Lambers") {
		t.Errorf("responsible = %+v", d.ResponsiblePersons)
	}
	if len(d.TeachingForms) != 2 || d.TeachingForms[0].Type != "Vorlesung" || d.TeachingForms[0].Workload != "4 SWS" {
		t.Errorf("teaching forms = %+v", d.TeachingForms)
	}
	if d.ExamType != "Voraussetzung + Modulabschlussprüfung (MAP)" || d.Limitation != "keine" {
		t.Errorf("exam/limitation = %q / %q", d.ExamType, d.Limitation)
	}
	if len(d.StudyPrograms) != 2 {
		t.Fatalf("study programs = %+v", d.StudyPrograms)
	}
	if got := d.StudyPrograms[0]; got.Degree != "Master (universitär)" || got.Program != "Artificial Intelligence" || got.Regulation != "PO 2022 - 1. SÄ 2024" {
		t.Errorf("study program = %+v", got)
	}

	// What the switch to QIS is about: the events of the semester that runs now.
	if len(d.CurrentSemesterEvents) != 1 {
		t.Fatalf("events = %+v", d.CurrentSemesterEvents)
	}
	if e := d.CurrentSemesterEvents[0]; !strings.Contains(e.URL, "veranstid=153213") || !strings.Contains(e.Title, "120632") {
		t.Errorf("event = %+v", e)
	}
}

// QIS states a phase-out in a row of its own and links the successor by its
// internal number, which must not be mistaken for a module number.
func TestQISModulePhaseOut(t *testing.T) {
	const page = `<html><body><table>
		<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">11162</td></tr>
		<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">Wirtschaftsprüfung</td></tr>
		<tr><td class="tabelle1_alignleft">Auslaufmodul:</td><td class="tabelle2inhalt">Nachfolgemodul seit: 20.01.2023
			<ul><li><a href="https://www.b-tu.de/qisserver3/rds?state=modulBeschrDetailInfo&amp;pord.pordnr=16532">
				12917 Wirtschaftsprüfung und Rechnungslegung</a></li></ul></td></tr>
		</table></body></html>`
	d, err := NewQISModuleParser().Parse(strings.NewReader(page), "11162", "u")
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if !d.IsPhaseOut {
		t.Error("a module with an Auslaufmodul row is phasing out")
	}
	if len(d.SuccessorModules) != 1 || d.SuccessorModules[0] != "12917" {
		t.Errorf("successors = %v, want [12917] and not the pordnr 16532", d.SuccessorModules)
	}
}

// A page whose rows are missing is a layout change, not a module without fields.
func TestQISModuleParserRejectsAPageWithoutRows(t *testing.T) {
	const page = `<html><body><table><tr><td>Modulnummer:</td><td>11289</td></tr></table></body></html>`
	if _, err := NewQISModuleParser().Parse(strings.NewReader(page), "11289", "u"); err == nil {
		t.Fatal("expected an error for a page without description rows")
	}
}

// A module whose description states no English title keeps the German one.
func TestQISModuleParserSingleTitle(t *testing.T) {
	const page = `<html><body><table>
		<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">12345</td></tr>
		<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">Bachelor-Arbeit</td></tr>
		<tr><td class="tabelle1_alignleft">&nbsp;</td><td class="tabelle2inhalt"></td></tr>
		</table></body></html>`
	d, err := NewQISModuleParser().Parse(strings.NewReader(page), "12345", "u")
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if d.TitleDE != "Bachelor-Arbeit" || d.TitleEN != "" {
		t.Errorf("titles = %q / %q", d.TitleDE, d.TitleEN)
	}
}

// Every QIS page names the semester QIS calls current in its head, a module description
// and the module table as much as the event search (2026-09-24).
func TestQISSemester(t *testing.T) {
	head := `<div class="services"><ol><li>
		<a href="https://www.b-tu.de/qisserver3/rds?state=change&amp;type=6&amp;moduleParameter=semesterSelect" id="choosesemester" title="Semester wählen ...">
		   WiSe 2026/27
		</a></li></ol></div>`
	if got := QISSemester([]byte(head)); got != "WiSe 2026/27" {
		t.Errorf("QISSemester = %q", got)
	}
	list, err := os.ReadFile("testdata/qis_event_list.html")
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	if got := QISSemester(list); got != "WiSe 2026/27" {
		t.Errorf("QISSemester(event search) = %q", got)
	}
	if got := QISSemester([]byte(`<html><body>no head</body></html>`)); got != "" {
		t.Errorf("QISSemester without a head = %q", got)
	}
}
