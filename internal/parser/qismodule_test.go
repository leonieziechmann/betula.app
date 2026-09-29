package parser

import (
	"io"
	"os"
	"slices"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/internal/model"
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

// The English view of the pair below, 2026-09-29: the same rows under their own labels.
const (
	qisSuccessorEN = `<html><body><table>
<tr><td class="tabelle1_alignleft" valign="top" width="20%">Module Number:</td>
<td class="tabelle2inhalt" valign="top" width="80%" style="text-transform: uppercase;"><b>12160
</b></td></tr>
<TR>
<TD class="tabelle1_alignleft" width="30%" valign="top">Phase-out Module:</TD>
<TD class="tabelle2inhalt" width="70%">
Follow-up Module since: 21.04.2017
<UL>
<li>
<a class="regular" title="See details on 38105 Allgemeine Betriebswirtschaftslehre I" href="https://www.b-tu.de/qisserver3/rds?state=modulBeschrDetailInfo&nodeID=auswahlBaum%7Cmodul:pordnr=7293&pord.pordnr=7293" style="background-color:lightpink;">
38105 Allgemeine Betriebswirtschaftslehre I
</a>
</li>
</li>
</UL>
</TD>
</TR>
</table></body></html>`
	qisPhaseOutEN = `<html><body><table>
<tr><td class="tabelle1_alignleft" valign="top" width="20%">Module Number:</td>
<td class="tabelle2inhalt" valign="top" width="80%" style="text-transform: uppercase;"><b>38105
- Phase-out Module
</b></td></tr>
<TR>
<TD class="tabelle1_alignleft" width="30%" valign="top">Follow-up Module/s:</TD>
<TD class="tabelle2inhalt" width="70%">
Phase-out module since: 21.04.2017
<UL>
<li>
<a class="regular" title="See details on 12160 Allgemeine Betriebswirtschaftslehre I: Grundlagen der BWL" href="https://www.b-tu.de/qisserver3/rds?state=modulBeschrDetailInfo&nodeID=auswahlBaum%7Cmodul:pordnr=14364&pord.pordnr=14364">
12160 Allgemeine Betriebswirtschaftslehre I: Grundlagen der BWL
</a>
</li>
</li>
</UL>
</TD>
</TR>
</table></body></html>`
)

// A replacement is stated on both modules, each naming the other, and a row's label says
// what the module it links is. 12160 „Allgemeine Betriebswirtschaftslehre I: Grundlagen der
// BWL" replaces 38105 „Allgemeine Betriebswirtschaftslehre I" since 21.04.2017: 12160 links
// 38105 under „Auslaufmodul" and marks the link in pink, 38105 („38105 - Auslaufmodul")
// links 12160 under „Nachfolgemodul/e". testdata/qis_module_<id>.html are their German
// descriptions as QIS served them on 2026-09-29, cut to the description; the English view
// and the copy on b-tu.de carry the same rows. A QIS link names the other module by its
// internal number (pordnr 7293 and 14364), which is not a module number.
func TestReplacementRows(t *testing.T) {
	fixture := func(id string) string {
		t.Helper()
		body, err := os.ReadFile("testdata/qis_module_" + id + ".html")
		if err != nil {
			t.Fatalf("read fixture: %v", err)
		}
		return string(body)
	}
	type moduleParser interface {
		Parse(r io.Reader, fallbackID, pageURL string) (*model.ModuleDetail, error)
	}
	for _, tc := range []struct {
		name                     string
		parser                   moduleParser
		id, page                 string
		phaseOut                 bool
		successors, predecessors []string
	}{
		{"successor", NewQISModuleParser(), "12160", fixture("12160"), false, nil, []string{"38105"}},
		{"phase-out", NewQISModuleParser(), "38105", fixture("38105"), true, []string{"12160"}, nil},
		{"successor, English view", NewQISModuleParser(), "12160", qisSuccessorEN, false, nil, []string{"38105"}},
		{"phase-out, English view", NewQISModuleParser(), "38105", qisPhaseOutEN, true, []string{"12160"}, nil},
		{"successor, copy", NewDetailParser(), "12160", `<div class="tx-btusysteme"><table>
			<tr><td>Modulnummer:</td><td><b>12160</b></td></tr>
			<tr><td>Auslaufmodul:</td><td>Nachfolgemodul seit: 21.04.2017
				<ul><li><a title="Details ansehen zu 38105 Allgemeine Betriebswirtschaftslehre I" href="https://www.b-tu.de/qisserver3/rds?state=modulBeschrDetailInfo&amp;pord.pordnr=7293">
					38105 Allgemeine Betriebswirtschaftslehre I</a></li></ul></td></tr>
			</table></div>`, false, nil, []string{"38105"}},
	} {
		d, err := tc.parser.Parse(strings.NewReader(tc.page), tc.id, "u")
		if err != nil {
			t.Fatalf("%s: Parse: %v", tc.name, err)
		}
		if d.IsPhaseOut != tc.phaseOut {
			t.Errorf("%s: IsPhaseOut = %v, want %v", tc.name, d.IsPhaseOut, tc.phaseOut)
		}
		if !slices.Equal(d.SuccessorModules, tc.successors) || !slices.Equal(d.PredecessorModules, tc.predecessors) {
			t.Errorf("%s: successors %v, replaces %v; want %v, %v", tc.name, d.SuccessorModules, d.PredecessorModules, tc.successors, tc.predecessors)
		}
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
