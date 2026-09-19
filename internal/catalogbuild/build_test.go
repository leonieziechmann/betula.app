package catalogbuild

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
)

// The fixture is a miniature BTU: two programs (Informatik B.Sc. and M.Sc.), a German
// compulsory module, an English elective, a FÜS module, a thesis and a list-only module.
// Every page uses the markup and the wording of the live pages from 2026-09-19.

const treeBase = "https://www.b-tu.de/qisserver3/rds?state=modulBeschrGast&nodeID="

func poNode(abschl string) string {
	return "auswahlBaum|studiengang:stg=079|abschluss:abschl=" + abschl + "|stgSpecials:vert=,schwp=,kzfa=H,pversion=2008"
}

func treePage(degree, po string, links ...string) string {
	var b strings.Builder
	b.WriteString(`<html><body><div class="Kruemelpfad">
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `auswahlBaum">Oberste Ebene</a></div>
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `x">Studiengang: Informatik</a></div>
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `y">Module für Abschluss: ` + degree + `</a></div>
		<div class="KruemelpfadEintrag"> PO-Version: ` + po + ` </div></div>
		<a href="https://opus4.kobv.de/opus4-btu/files/6700/po.pdf" target="_blank" title="Prüfungsordnung ABl. 12/2024"><img src="/QIS/images/pruefungsordnung.svg"></a>
		<ul class="treelist">`)
	for i := 0; i+1 < len(links); i += 2 {
		fmt.Fprintf(&b, `<li><a class="regular" href="%s%s">%s</a></li>`, treeBase, links[i], links[i+1])
	}
	b.WriteString(`</ul></body></html>`)
	return b.String()
}

func modulePageDE(id, title, extraRows string) string {
	return `<html><body><div class="tx-btusysteme"><h1>` + id + ` - ` + title + ` <small>Modulübersicht</small></h1><table>
		<tr><td>Modulnummer:</td><td>` + id + `</td></tr>
		<tr><td>Modultitel:</td><td>` + title + `</td></tr>
		<tr><td>&nbsp;</td><td>` + title + ` (EN)</td></tr>
		<tr><td>Einrichtung:</td><td>Fakultät 1 - MINT - Mathematik, Informatik</td></tr>
		<tr><td>Verantwortlich:</td><td><ul><li>Prof. Dr. rer. nat. Köhler, Ekkehard</li></ul></td></tr>
		<tr><td>Lehr- und Prüfungssprache:</td><td>Deutsch</td></tr>
		<tr><td>Dauer:</td><td>1 Semester</td></tr>
		<tr><td>Angebotsturnus:</td><td>jedes Wintersemester ungerader Jahre</td></tr>
		<tr><td>Leistungspunkte:</td><td>8</td></tr>
		<tr><td>Empfohlene Voraussetzungen:</td><td>Modul 11881 und 99999</td></tr>
		<tr><td>Zwingende Voraussetzungen:</td><td>keine</td></tr>
		<tr><td>Lehrformen und Arbeitsumfang:</td><td><ul><li>Vorlesung / 4 SWS</li><li>Übung / 2 SWS</li><li>Selbststudium / 150 Stunden</li></ul></td></tr>
		<tr><td>Modulprüfung:</td><td>Voraussetzung + Modulabschlussprüfung (MAP)</td></tr>
		<tr><td>Prüfungsleistung/en für Modulprüfung:</td><td>Klausur, 90 min.</td></tr>
		<tr><td>Bewertung der Modulprüfung:</td><td>Prüfungsleistung - benotet</td></tr>
		<tr><td>Teilnehmerbeschränkung:</td><td>keine</td></tr>
		` + extraRows + `
	</table></div></body></html>`
}

const englishModulePage = `<html><body><div class="tx-btusysteme"><h1>11881 - Foundations of Data Mining</h1><table>
	<tr><td>Module Number:</td><td>11881</td></tr>
	<tr><td>Module Title:</td><td>Foundations of Data Mining</td></tr>
	<tr><td>&nbsp;</td><td>Grundlagen des Data Mining</td></tr>
	<tr><td>Department:</td><td>Faculty 1 - Mathematics, Computer Science</td></tr>
	<tr><td>Responsible Staff Member:</td><td><ul><li>Prof. Dr. rer. nat. Köhler, Ekkehard</li></ul></td></tr>
	<tr><td>Language of Teaching / Examination:</td><td>English</td></tr>
	<tr><td>Duration:</td><td>1 semester</td></tr>
	<tr><td>Frequency of Offer:</td><td>Every summer semester</td></tr>
	<tr><td>Credits:</td><td>6</td></tr>
	<tr><td>Forms of Teaching and Proportion:</td><td><ul><li>Lecture / 2 Hours per Week per Semester</li><li>Exercise / 2 Hours per Week per Semester</li></ul></td></tr>
	<tr><td>Module Examination:</td><td>Continuous Assessment (MCA)</td></tr>
	<tr><td>Assessment Mode for Module Examination:</td><td>Presentation (max. 30 min)</td></tr>
	<tr><td>Evaluation of Module Examination:</td><td>Study Performance – ungraded</td></tr>
	<tr><td>Limited Number of Participants:</td><td>80</td></tr>
	<tr><td>Part of the Study Programme:</td><td><ul>
		<li>Abschluss im Ausland / Informatik / keine PO</li>
		<li>Bachelor (research-oriented) / Informatik / PO 2008 - 2. SÄ 2024</li>
		<li>Master (research-oriented) / Informatik / PO 2008</li>
		<li>Master (research-oriented) / Astrologie / PO 1999</li>
	</ul></td></tr>
	<tr><td>Remarks:</td><td>• Study programme Informatik B.Sc.: Compulsory elective module in complex „Grundlagen der Informatik” (level 300) • Study programme Informatik M.Sc.: Compulsory elective module</td></tr>
	<tr><td>Components to be offered in the Current Semester:</td><td><ul>
		<li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=120285">120285 Lecture</a></li>
		<li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=120286">120286 Examination</a></li>
		<li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=777777">777777 not archived</a></li>
	</ul></td></tr>
</table></div></body></html>`

func eventPageHTML(title, eventType, room, duration string) string {
	return `<html><body><h1>` + title + ` - Einzelansicht</h1>
	<table summary="Grunddaten zur Veranstaltung"><tr><th>Veranstaltungsart</th><td>` + eventType + `</td><th>Semester</th><td>SS 2026</td></tr>
		<tr><th>SWS</th><td>2</td><th>Max. Teilnehmer/-innen</th><td>80</td></tr></table>
	<table summary="Übersicht über alle Veranstaltungstermine"><caption>Termine Gruppe: 1</caption>
		<tr><th>Tag</th><th>Zeit</th><th>Rhythmus</th><th>Dauer</th><th>Raum</th><th>Lehrperson</th></tr>
		<tr><td>Di.</td><td>09:15 bis 10:45</td><td>A/B</td><td>` + duration + `</td><td><a href="#">` + room + `</a></td><td><a href="#">Meyer</a></td></tr></table>
	</body></html>`
}

func buildFixture(t *testing.T) (*catalogdb.DB, *Report) {
	t.Helper()
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	put := func(source, key, body string) {
		t.Helper()
		status := 200
		if body == "" {
			status = 404
		}
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: status, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 19, 15, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(catalogdb.SourceModuleCatalog, "list", `<table><tbody class="list">
		<tr><td class="moduleNumber"><a href="/modul/11101">11101</a></td><td class="title">Lineare Algebra</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/11881">11881</a></td><td class="title">Foundations of Data Mining</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/11152">11152</a></td><td class="title">ERP</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/12999">12999</a></td><td class="title">Bachelor-Arbeit</td></tr>
		<tr><td class="moduleNumber"><a href="/modul/13000">13000</a></td><td class="title">Verschwundenes Modul</td></tr>
	</tbody></table>`)
	put(catalogdb.SourceQISFUESList, "list", `<table summary="Suchergebnis">
		<tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>
		<tr><td>11152</td><td><a href="#">ERP</a></td><td>Deutsch</td><td>6</td><td>ja</td><td>25</td></tr>
		<tr><td>14037</td><td><a href="#">Nur auf der FÜS-Liste</a></td><td>Englisch</td><td>4</td><td>ja</td><td></td></tr>
	</table>`)

	put(catalogdb.SourceModulePage, "11101", modulePageDE("11101", "Lineare Algebra", `
		<tr><td>Zuordnung zu Studiengängen:</td><td><ul><li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>
		<tr><td>Bemerkungen:</td><td>Nachfolgemodul: 11881</td></tr>`))
	put(catalogdb.SourceModulePage, "11881", englishModulePage)
	put(catalogdb.SourceModulePage, "11152", modulePageDE("11152", "ERP", `
		<tr><td>Zuordnung zu Studiengängen:</td><td><ul>
			<li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li>
			<li>Master (universitär) / Informatik / PO 2008 - 3. SÄ 2024</li></ul></td></tr>
		<tr><td>&nbsp;</td><td>Das Modul ist für das Fachübergreifende Studium zugelassen.</td></tr>`))
	put(catalogdb.SourceModulePage, "12999", modulePageDE("12999", "Bachelor-Arbeit", `
		<tr><td>Zuordnung zu Studiengängen:</td><td><ul><li>keine Zuordnung vorhanden</li></ul></td></tr>`))
	put(catalogdb.SourceModulePage, "13000", "") // 404

	bsc, msc := poNode("82"), poNode("88")
	put(catalogdb.SourceQISTree, treeBase+bsc, treePage("Bachelor (universitär)", "2008 - 2. SÄ 2024", bsc+"|konto:1", "Gesamtkonto"))
	put(catalogdb.SourceQISTree, treeBase+bsc+"|konto:1", treePage("Bachelor (universitär)", "2008 - 2. SÄ 2024",
		bsc+"|konto:1|konto:2", "Grundstudium", bsc+"|konto:1|konto:3", "Fachstudium", bsc+"|konto:1|pruefung:9", "12999 Bachelor-Arbeit"))
	put(catalogdb.SourceQISTree, treeBase+bsc+"|konto:1|konto:2", treePage("Bachelor (universitär)", "2008 - 2. SÄ 2024",
		bsc+"|konto:1|konto:2|konto:4", "Pflichtmodule Mathematik"))
	put(catalogdb.SourceQISTree, treeBase+bsc+"|konto:1|konto:2|konto:4", treePage("Bachelor (universitär)", "2008 - 2. SÄ 2024",
		bsc+"|konto:1|konto:2|konto:4|pruefung:1", "11101 Lineare Algebra", bsc+"|konto:1|konto:2|konto:4|pruefung:2", "15011 Nicht im Katalog"))
	put(catalogdb.SourceQISTree, treeBase+bsc+"|konto:1|konto:3", treePage("Bachelor (universitär)", "2008 - 2. SÄ 2024",
		bsc+"|konto:1|konto:3|pruefung:3", "11881 Foundations of Data Mining"))
	put(catalogdb.SourceQISTree, treeBase+msc, treePage("Master (universitär)", "2008 - 3. SÄ 2024", msc+"|pruefung:4", "11881 Foundations of Data Mining"))

	put(catalogdb.SourceQISEvent, "120285", eventPageHTML("Foundations of Data Mining", "Vorlesung/Übung", "Allgemeine Elektrotechnik Labor - 14.117 - Campus Senftenberg", "14.04.2026 bis 21.07.2026"))
	put(catalogdb.SourceQISEvent, "120286", eventPageHTML("Prüfung Data Mining", "Prüfung", "Lehrgebäude 1A - 0.22 - Zentralcampus", "am 06.08.2026"))

	if err := db.SavePlan(catalogdb.Plan{ProgramID: "079-82-2008", SourceFile: "po.pdf", LayoutJSON: "{}", Entries: []catalogdb.PlanEntry{
		{ModuleID: "11101", ModuleName: "Lineare Algebra", Semester: 1, StartSemester: 1, EndSemester: 1, Credits: 6, KindRaw: "Pflicht", SubjectArea: "Mathematik"},
		{ModuleID: "11881", ModuleName: "Foundations of Data Mining", StartSemester: 5, EndSemester: 6, SemesterSpan: "5-6", KindRaw: "Modul"},
		{ModuleName: "Wahlpflicht Informatik", MinCredits: 10, MaxCredits: 24, KindRaw: "Wahlpflicht"},
	}}); err != nil {
		t.Fatalf("SavePlan failed: %v", err)
	}

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	return db, report
}

// want runs a query and compares the rows, rendered as "a|b|c" with NULL as "∅".
func want(t *testing.T, db *catalogdb.DB, query string, expected ...string) {
	t.Helper()
	rows, err := db.SQL().Query(query)
	if err != nil {
		t.Fatalf("query failed: %v\n%s", err, query)
	}
	defer rows.Close()
	cols, _ := rows.Columns()
	var got []string
	for rows.Next() {
		values := make([]sql.NullString, len(cols))
		ptrs := make([]any, len(cols))
		for i := range values {
			ptrs[i] = &values[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			t.Fatalf("scan failed: %v", err)
		}
		parts := make([]string, len(cols))
		for i, v := range values {
			parts[i] = "∅"
			if v.Valid {
				parts[i] = v.String
			}
		}
		got = append(got, strings.Join(parts, "|"))
	}
	if strings.Join(got, "\n") != strings.Join(expected, "\n") {
		t.Errorf("%s\n got: %q\nwant: %q", query, got, expected)
	}
}

func TestBuildReport(t *testing.T) {
	_, r := buildFixture(t)
	if r.Modules != 6 || r.ModulesWithoutPage != 2 || r.Programs != 2 || r.Departments != 1 || len(r.UnpairedEnglishDep) != 0 {
		t.Errorf("counts: %+v", r)
	}
	if r.TreeLeavesNoModule["15011"] != 1 || r.MissingTreePages != 0 {
		t.Errorf("tree: leaves without module %v, missing pages %d", r.TreeLeavesNoModule, r.MissingTreePages)
	}
	if r.PageRefs != 7 || r.PageRefsAbroad != 1 || r.PageRefsUnresolved["Master (research-oriented) / Astrologie / PO 1999"] != 1 {
		t.Errorf("page refs: %d, abroad %d, unresolved %v", r.PageRefs, r.PageRefsAbroad, r.PageRefsUnresolved)
	}
	if r.Events != 2 || r.EventLinksNoArchive != 1 {
		t.Errorf("events: %d, unarchived links %d", r.Events, r.EventLinksNoArchive)
	}
}

func TestModulesAreNormalizedAndUnknownIsNull(t *testing.T) {
	db, _ := buildFixture(t)

	want(t, db, `SELECT id, title, title_de, title_en, page_lang, detail_status FROM module ORDER BY id`,
		"11101|Lineare Algebra|Lineare Algebra|Lineare Algebra (EN)|de|ok",
		"11152|ERP|ERP|ERP (EN)|de|ok",
		"11881|Foundations of Data Mining|Grundlagen des Data Mining|Foundations of Data Mining|en|ok",
		"12999|Bachelor-Arbeit|Bachelor-Arbeit|Bachelor-Arbeit (EN)|de|ok",
		"13000|Verschwundenes Modul|∅|∅|∅|missing",
		"14037|Nur auf der FÜS-Liste|∅|∅|∅|missing")

	// P1: the three exam rows stay apart, in both languages.
	want(t, db, `SELECT id, exam_form, exam_form_raw, exam_details, is_graded, exam_written, exam_presentation FROM module WHERE id IN ('11101','11881') ORDER BY id`,
		"11101|prereq_map|Voraussetzung + Modulabschlussprüfung (MAP)|Klausur, 90 min.|1|1|0",
		"11881|mca|Continuous Assessment (MCA)|Presentation (max. 30 min)|0|0|1")

	want(t, db, `SELECT id, turnus_season, turnus_parity, duration_semesters, teaches_german, teaches_english, is_limited, participant_limit, credits FROM module ORDER BY id`,
		"11101|winter|odd|1|1|0|0|∅|8",
		"11152|winter|odd|1|1|0|0|∅|8",
		"11881|summer|∅|1|0|1|1|80|6",
		"12999|winter|odd|1|1|0|0|∅|8",
		"13000|∅|∅|∅|∅|∅|∅|∅|∅",
		"14037|∅|∅|∅|0|1|∅|∅|4") // FÜS list is the fallback for a module without a page

	// FÜS flag comes from the list; the page sentence is only a cross-check.
	want(t, db, `SELECT id, is_fues, page_states_fues FROM module WHERE is_fues = 1 OR page_states_fues = 1 ORDER BY id`,
		"11152|1|1", "14037|1|∅")

	// „keine" is not a prerequisite; only existing modules become links.
	want(t, db, `SELECT id, prerequisites_mandatory, prerequisites_recommended FROM module WHERE id = '11101'`, "11101|∅|Modul 11881 und 99999")
	want(t, db, `SELECT module_id, required_module_id, kind, required_title FROM v_module_prerequisite ORDER BY 1, 2`,
		"11101|11881|recommended|Foundations of Data Mining",
		"11152|11881|recommended|Foundations of Data Mining",
		"12999|11881|recommended|Foundations of Data Mining")
	want(t, db, `SELECT module_id, successor_id, successor_title FROM v_module_successor`, "11101|11881|Foundations of Data Mining")

	// German and English name of the same unit end up in one department.
	want(t, db, `SELECT code, name_de, name_en, modules FROM v_department`, "1|MINT - Mathematik, Informatik|Mathematics, Computer Science|4")
}

func TestFacetsNeedNoLike(t *testing.T) {
	db, _ := buildFixture(t)

	// The v1 frontend found 0 modules for „Übung" because sql.js cannot lowercase „Ü".
	want(t, db, `SELECT module_id FROM v_module_facets WHERE has_exercise = 1 ORDER BY 1`, "11101", "11152", "11881", "12999")
	want(t, db, `SELECT module_id FROM v_module_facets WHERE offered_summer = 1`, "11881")
	want(t, db, `SELECT module_id FROM v_module_facets WHERE turnus_parity = 'odd' AND offered_winter = 1 ORDER BY 1`, "11101", "11152", "12999")
	want(t, db, `SELECT module_id FROM v_module_facets WHERE is_graded = 0`, "11881")
	want(t, db, `SELECT module_id FROM v_module_facets WHERE is_limited = 1 AND participant_limit <= 100`, "11881")

	// Campus: „Allgemeine Elektrotechnik … Campus Senftenberg" is not the Zentralcampus; the exam room does not count.
	want(t, db, `SELECT module_id, teaching_events, at_zentralcampus, at_senftenberg FROM v_module_facets WHERE teaching_events > 0`, "11881|1|0|1")
	want(t, db, `SELECT module_id, at_zentralcampus FROM v_module_facets WHERE module_id = '11101'`, "11101|∅")

	want(t, db, `SELECT id, department, department_code, responsible, teaching_events FROM v_module WHERE id = '11881'`,
		"11881|Fakultät 1 - MINT - Mathematik, Informatik|1|Prof. Dr. rer. nat. Köhler, Ekkehard|1")
	want(t, db, `SELECT term, kind FROM v_module_search WHERE module_id = '11881' ORDER BY kind`,
		"11881|id", "Grundlagen des Data Mining|title_de", "Foundations of Data Mining|title_en")
	want(t, db, `SELECT form, sws, hours FROM v_module_teaching_form WHERE module_id = '11101' ORDER BY ord`,
		"lecture|4|∅", "exercise|2|∅", "self_study|∅|150")
	want(t, db, `SELECT module_id, name, role FROM v_module_lecturer WHERE module_id = '11881' ORDER BY role`,
		"11881|Meyer|instructor", "11881|Köhler, Ekkehard|responsible")
	want(t, db, `SELECT COUNT(*) FROM v_module_text_item`, "0")
}

func TestPrograms(t *testing.T) {
	db, _ := buildFixture(t)

	want(t, db, `SELECT id, slug, name, degree_level, degree_type, study_variant, degree_label, degree_display, po_year, po_amendment, is_latest_po, has_plan, curricular_modules, fues_modules, documents FROM v_program ORDER BY id`,
		"079-82-2008|bachelor-informatik-2008|Informatik|bachelor|university|∅|∅|Bachelor|2008|2. SÄ 2024|1|1|3|1|1",
		"079-88-2008|master-informatik-2008|Informatik|master|university|∅|∅|Master|2008|3. SÄ 2024|1|0|1|1|1")
	want(t, db, `SELECT program_id, counterpart_id, counterpart_level, match_score FROM v_program_counterpart ORDER BY 1`,
		"079-82-2008|079-88-2008|master|3", "079-88-2008|079-82-2008|bachelor|3")
	want(t, db, `SELECT COUNT(*) FROM v_program_version`, "0")
	want(t, db, `SELECT program_id, doc_type, title FROM v_program_document ORDER BY 1`,
		"079-82-2008|statute|Prüfungsordnung ABl. 12/2024", "079-88-2008|statute|Prüfungsordnung ABl. 12/2024")

	// Bookkeeping nodes („Gesamtkonto") are not areas; a label states a kind or nothing does.
	want(t, db, `SELECT path, depth, section, stated_kind FROM program_area WHERE program_id = '079-82-2008' ORDER BY ord`,
		"Grundstudium|1|basic|∅",
		"Grundstudium / Pflichtmodule Mathematik|2|basic|compulsory",
		"Fachstudium|1|main|∅")
}

func TestMembershipRelationAndKindPrecedence(t *testing.T) {
	db, _ := buildFixture(t)

	want(t, db, `SELECT module_id, relation, kind, kind_source, kind_basis, precedence, area, section, in_tree, on_module_page, in_plan, plan_semester
		FROM v_program_module WHERE program_id = '079-82-2008' ORDER BY module_id`,
		// plan and tree both say compulsory: the plan decides (precedence 1)
		"11101|curricular|compulsory|pdf_plan|stated|1|Grundstudium / Pflichtmodule Mathematik|basic|1|1|1|1",
		// FÜS module that only its page places here: the program's FÜS list, not its curriculum
		"11152|fues|fues|∅|∅|∅|∅|∅|0|1|0|∅",
		// the tree has no label (v1 defaulted to Pflicht), the plan says „Modul": the page's remark decides
		"11881|curricular|elective|module_page|stated|2|Fachstudium|main|1|1|1|∅",
		// nothing states a kind; the title lets us infer it, marked as inferred
		"12999|curricular|thesis|qis_tree|inferred|6|∅|∅|1|0|0|∅")

	want(t, db, `SELECT module_id, relation, kind, kind_source FROM v_program_module WHERE program_id = '079-88-2008' ORDER BY module_id`,
		"11152|fues|fues|∅",
		"11881|curricular|elective|module_page") // page says „PO 2008", which fits exactly one Master PO

	want(t, db, `SELECT program_id, area, kind FROM v_program_module_area WHERE module_id = '11881' ORDER BY 1`, "079-82-2008|Fachstudium|∅")

	want(t, db, `SELECT ord, resolve_status, program_id, degree_display, relation, kind FROM v_module_program_link WHERE module_id = '11881' ORDER BY ord`,
		"1|abroad|∅|∅|∅|∅",
		"2|resolved|079-82-2008|Bachelor|curricular|elective",
		"3|resolved|079-88-2008|Master|curricular|elective",
		"4|unresolved|∅|∅|∅|∅")

	want(t, db, `SELECT program_id, tree_modules, page_modules, plan_entries, plan_entries_linked, modules_without_kind, plan_status FROM program_coverage ORDER BY 1`,
		"079-82-2008|3|2|3|2|0|not_scanned", "079-88-2008|1|1|0|0|0|not_scanned")
}

func TestPlanEntriesKeepRangesAndUnknowns(t *testing.T) {
	db, _ := buildFixture(t)
	want(t, db, `SELECT ord, module_id, semester, start_semester, end_semester, semester_span, credits, min_credits, max_credits, kind, catalog_credits, credits_differ_from_catalog
		FROM v_program_plan_entry WHERE program_id = '079-82-2008' ORDER BY ord`,
		"1|11101|1|1|1|∅|6|∅|∅|compulsory|8|1",
		"2|11881|∅|5|6|5-6|∅|∅|∅|∅|6|0",
		"3|∅|∅|∅|∅|∅|∅|10|24|elective|∅|0")
	want(t, db, `SELECT program_id, source_file FROM v_program_plan`, "079-82-2008|po.pdf")
}

func TestEventsSeparateExamsFromSchedule(t *testing.T) {
	db, _ := buildFixture(t)
	want(t, db, `SELECT module_id, semester_key, semester_label, event_id, weekday, start_time, end_time, rhythm, first_date, last_date, campus, instructor FROM v_module_schedule`,
		"11881|2026S|SoSe 2026|120285|2|09:15|10:45|weekly|2026-04-14|2026-07-21|senftenberg|Meyer")
	want(t, db, `SELECT module_id, event_id, first_date, campus FROM v_module_exam`, "11881|120286|2026-08-06|zentralcampus")
	want(t, db, `SELECT form FROM event_form WHERE event_id = '120285' ORDER BY 1`, "exercise", "lecture")
	want(t, db, `SELECT key, label, teaching_events, exam_events FROM v_semester`, "2026S|SoSe 2026|1|1")
	want(t, db, `SELECT id, last_date FROM event ORDER BY id`, "120285|2026-07-21", "120286|2026-08-06")
	want(t, db, `SELECT COUNT(*) FROM v_meta WHERE key IN ('built_at', 'current_semester')`, "2")
}

func TestBuildIsRepeatableAndKeepsPlans(t *testing.T) {
	db, first := buildFixture(t)
	second, err := Build(context.Background(), db)
	if err != nil {
		t.Fatalf("second Build failed: %v", err)
	}
	if first.Modules != second.Modules || first.Assertions["qis_tree"] != second.Assertions["qis_tree"] || second.Assertions["pdf_plan"] != 2 {
		t.Errorf("second build differs: %+v vs %+v", first, second)
	}
	want(t, db, `SELECT COUNT(*) FROM plan_entry`, "3")
	want(t, db, `SELECT COUNT(*) FROM pragma_foreign_key_check`, "0")

	// The digest decides whether a new snapshot is published.
	if !first.ContentChanged || second.ContentChanged || first.ContentDigest != second.ContentDigest {
		t.Errorf("digest: first changed=%v, second changed=%v, equal=%v", first.ContentChanged, second.ContentChanged, first.ContentDigest == second.ContentDigest)
	}

	// Fetching a page again without any change must not count as new content …
	page, err := db.GetPage(catalogdb.SourceModulePage, "11101")
	if err != nil {
		t.Fatal(err)
	}
	page.FetchedAt = page.FetchedAt.Add(48 * time.Hour)
	if err := db.PutPage(*page); err != nil {
		t.Fatal(err)
	}
	refetched, err := Build(context.Background(), db)
	if err != nil || refetched.ContentChanged {
		t.Errorf("refetch without change: changed=%v err=%v", refetched != nil && refetched.ContentChanged, err)
	}
	want(t, db, `SELECT fetched_at FROM module WHERE id = '11101'`, "2026-09-21T15:00:00Z")
	want(t, db, `SELECT value FROM v_meta WHERE key = 'data_changed_at'`, first.BuiltAt.Format(time.RFC3339))

	// … but a changed page must.
	page.Body = []byte(strings.Replace(string(page.Body), "<td>8</td>", "<td>9</td>", 1))
	if err := db.PutPage(*page); err != nil {
		t.Fatal(err)
	}
	changed, err := Build(context.Background(), db)
	if err != nil || !changed.ContentChanged {
		t.Errorf("changed credits: changed=%v err=%v", changed != nil && changed.ContentChanged, err)
	}
}

func TestValidatePassesOnCleanBuildAndCatchesRegressions(t *testing.T) {
	db, _ := buildFixture(t)
	ctx := context.Background()

	checks, err := db.Validate(ctx, []catalogdb.Baseline{{Name: "modules", Query: "SELECT COUNT(*) FROM module", Min: 6}})
	if err != nil {
		t.Fatalf("Validate failed: %v", err)
	}
	if catalogdb.HasFailures(checks) {
		t.Fatalf("clean build has failures: %+v", checks)
	}
	status := make(map[string]catalogdb.Check)
	for _, c := range checks {
		status[c.Name] = c
	}
	if c := status["module page assignments that resolve to no program"]; c.Status != catalogdb.StatusWarn || c.Value != 1 || len(c.Samples) != 1 {
		t.Errorf("unresolved refs check = %+v", c)
	}
	if c := status["modules without a module page"]; c.Status != catalogdb.StatusWarn || c.Value != 2 {
		t.Errorf("missing page check = %+v", c)
	}

	// Regressions: a placeholder string, a stale materialized table, a missed baseline.
	for _, stmt := range []string{
		"UPDATE module SET remarks = '' WHERE id = '11101'",
		"DELETE FROM program_module WHERE module_id = '11152'",
	} {
		if _, err := db.SQL().Exec(stmt); err != nil {
			t.Fatal(err)
		}
	}
	checks, err = db.Validate(ctx, []catalogdb.Baseline{{Name: "modules", Query: "SELECT COUNT(*) FROM module", Min: 7}})
	if err != nil {
		t.Fatalf("Validate failed: %v", err)
	}
	failed := make(map[string]bool)
	for _, c := range checks {
		if c.Status == catalogdb.StatusFail {
			failed[c.Name] = true
		}
	}
	for _, name := range []string{"no empty-string or '-' placeholders in text columns", "program_module matches its source view", "baseline: modules"} {
		if !failed[name] {
			t.Errorf("expected check %q to fail; failed = %v", name, failed)
		}
	}
}

func TestExportWritesTrimmedSnapshotWithContentETag(t *testing.T) {
	db, _ := buildFixture(t)
	dir := filepath.Join(t.TempDir(), "snapshot")

	first, err := db.Export(context.Background(), dir)
	if err != nil {
		t.Fatalf("Export failed: %v", err)
	}
	body, err := os.ReadFile(filepath.Join(dir, first.File))
	if err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256(body)
	if want := `"` + hex.EncodeToString(sum[:])[:32] + `"`; first.ETag != want || first.Bytes != int64(len(body)) {
		t.Errorf("ETag = %s (%d bytes), want %s (%d bytes)", first.ETag, first.Bytes, want, len(body))
	}
	if pointer, err := catalogdb.ReadSnapshotPointer(dir); err != nil || *pointer != *first {
		t.Errorf("pointer = %+v (err %v), want %+v", pointer, err, first)
	}

	// A reader keeps the snapshot open, as the web server does.
	snap, err := sql.Open("sqlite", filepath.Join(dir, first.File)+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer snap.Close()
	var rawTables, journal, modules, fkViolations = 0, "", 0, 0
	_ = snap.QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE name = 'raw_page'").Scan(&rawTables)
	_ = snap.QueryRow("PRAGMA journal_mode").Scan(&journal)
	_ = snap.QueryRow("SELECT COUNT(*) FROM v_program_module WHERE program_id = '079-82-2008'").Scan(&modules)
	_ = snap.QueryRow("SELECT COUNT(*) FROM pragma_foreign_key_check").Scan(&fkViolations)
	if rawTables != 0 || journal != "delete" || modules != 4 || fkViolations != 0 {
		t.Errorf("snapshot: raw tables %d, journal %q, modules %d, fk violations %d", rawTables, journal, modules, fkViolations)
	}

	// Same size, different content: the v1 ETag (the file size) would not have noticed.
	if _, err := db.SQL().Exec("UPDATE module SET title = 'Lineare Algebrb' WHERE id = '11101'"); err != nil {
		t.Fatal(err)
	}
	second, err := db.Export(context.Background(), dir)
	if err != nil {
		t.Fatalf("Export while a reader holds the previous snapshot failed: %v", err)
	}
	if second.ETag == first.ETag || second.File == first.File {
		t.Errorf("snapshot identity did not change with the content: %+v → %+v", first, second)
	}
	if pointer, _ := catalogdb.ReadSnapshotPointer(dir); pointer == nil || pointer.File != second.File {
		t.Errorf("pointer = %+v, want file %s", pointer, second.File)
	}
	if err := snap.QueryRow("SELECT COUNT(*) FROM module").Scan(&modules); err != nil || modules != 6 {
		t.Errorf("the open reader lost its snapshot: %d modules, err %v", modules, err)
	}
}

// BTU publishes the next semester event by event. The first published winter event
// must not take the campus away from modules that are still on the summer semester.
func TestScheduleFacetsUseEachModulesOwnNewestSemester(t *testing.T) {
	db, _ := buildFixture(t)
	_, err := db.SQL().Exec(`
		INSERT INTO semester (key, season, year, label, starts_on, ends_on) VALUES ('2026W', 'winter', 2026, 'WiSe 2026/27', '2026-10-01', '2027-03-31');
		INSERT INTO event (id, title, category, semester_key, source_url, fetched_at) VALUES
			('w1', 'Lineare Algebra (neu)', 'teaching', '2026W', 'u', '2026-09-19T15:00:00Z'),
			('s1', 'Lineare Algebra (alt)', 'teaching', '2026S', 'u', '2026-09-19T15:00:00Z');
		INSERT INTO event_date (event_id, ord, campus) VALUES ('w1', 1, 'sachsendorf'), ('s1', 1, 'zentralcampus');
		INSERT INTO module_event (module_id, event_id) VALUES ('11101', 'w1'), ('11101', 's1');`)
	if err != nil {
		t.Fatal(err)
	}
	want(t, db, `SELECT module_id, teaching_events, at_zentralcampus, at_sachsendorf, at_senftenberg FROM v_module_facets_src WHERE teaching_events > 0 ORDER BY 1`,
		"11101|1|0|1|0", // already published for winter: only the winter event counts
		"11881|1|0|0|1") // still on the summer semester: keeps its campus
}
