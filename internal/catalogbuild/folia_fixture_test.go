package catalogbuild

// A synthetic catalog for developing the web tier without a crawl: several programs with
// trees, areas and a plan, hundreds of modules with varied facets, a few events. Written only
// when BETULA_FIXTURE_DIR names a directory:
//
//	BETULA_FIXTURE_DIR=$PWD/snapshot go test ./internal/catalogbuild -run TestWriteFoliaFixture -count=1
//
// The numbers are made up; nothing here says anything about the BTU.

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
)

type fixtureProgram struct {
	name, stg, abschl, degree, pversion, po string
	areas                                   []fixtureArea
}

type fixtureArea struct {
	label    string
	children []fixtureArea
	modules  []string
}

func fixtureTreePage(p fixtureProgram, links ...string) string {
	var b strings.Builder
	b.WriteString(`<html><body><div class="Kruemelpfad">
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `auswahlBaum">Oberste Ebene</a></div>
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `x">Studiengang: ` + p.name + `</a></div>
		<div class="KruemelpfadEintrag"><a class="regular" href="` + treeBase + `y">Module für Abschluss: ` + p.degree + `</a></div>
		<div class="KruemelpfadEintrag"> PO-Version: ` + p.po + ` </div></div>
		<a href="https://opus4.kobv.de/opus4-btu/files/` + p.stg + `/po.pdf" target="_blank" title="Prüfungsordnung ABl. 12/2024"><img src="/QIS/images/pruefungsordnung.svg"></a>
		<ul class="treelist">`)
	for i := 0; i+1 < len(links); i += 2 {
		fmt.Fprintf(&b, `<li><a class="regular" href="%s%s">%s</a></li>`, treeBase, links[i], links[i+1])
	}
	b.WriteString(`</ul></body></html>`)
	return b.String()
}

func TestWriteFoliaFixture(t *testing.T) {
	dir := os.Getenv("BETULA_FIXTURE_DIR")
	if dir == "" {
		t.Skip("BETULA_FIXTURE_DIR not set")
	}
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "fixture.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })
	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 9, 19, 15, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	// ---- modules ----
	prefixes := []string{"Grundlagen der", "Einführung in die", "Vertiefung", "Angewandte", "Theoretische", "Praktische", "Numerische", "Verteilte", "Digitale", "Moderne", "Experimentelle", "Höhere"}
	subjects := []string{"Informatik", "Mathematik", "Physik", "Elektrotechnik", "Regelungstechnik", "Thermodynamik", "Datenanalyse", "Softwaretechnik", "Rechnernetze", "Betriebssysteme", "Künstliche Intelligenz", "Robotik", "Signalverarbeitung", "Werkstoffkunde", "Baukonstruktion", "Stadtplanung", "Energiesysteme", "Optimierung", "Statistik", "Strömungsmechanik", "Bildverarbeitung", "Datenbanken", "Kryptographie", "Mechanik", "Chemie"}
	suffixes := []string{"", "", " I", " II", " für Ingenieurinnen und Ingenieure", " und ihre Anwendungen", " in der Praxis"}
	departments := []string{
		"Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik",
		"Fakultät 2 - Umwelt und Naturwissenschaften",
		"Fakultät 3 - Maschinenbau, Elektro- und Energiesysteme",
		"Fakultät 5 - Wirtschaft, Recht und Gesellschaft",
		"Fakultät 6 - Architektur, Bauingenieurwesen und Stadtplanung",
	}
	persons := []string{"Prof. Dr. rer. nat. Köhler, Ekkehard", "Prof. Dr.-Ing. Meer, Klaus", "Prof. Dr. Lambers, Leen", "Prof. Dr. Hofstedt, Petra", "Dr. Wachsmuth, Gerd", "Prof. Dr.-Ing. Schmidt, Anna", "Prof. Dr. Neumann, Jonas"}
	surnames := []string{"Bauer", "Fischer", "Weber", "Wagner", "Becker", "Schulz", "Hoffmann", "Koch", "Richter", "Klein", "Wolf", "Schröder", "Zimmermann", "Braun", "Krüger", "Hartmann", "Lange", "Werner", "Krause", "Lehmann", "Huber", "Mayer", "Herrmann", "König", "Walter", "Peters", "Möller", "Kaiser", "Fuchs", "Lang"}
	forenames := []string{"Anna", "Jonas", "Mia", "Lukas", "Lena", "Paul"}
	for i, surname := range surnames {
		for j := 0; j < 4; j++ {
			title := []string{"Prof. Dr.", "Dr.", "Prof. Dr.-Ing.", "Dr. rer. nat."}[(i+j)%4]
			persons = append(persons, fmt.Sprintf("%s %s, %s", title, surname, forenames[(i*3+j)%len(forenames)]))
		}
	}
	turnus := []string{"jedes Wintersemester", "jedes Sommersemester", "jedes Semester", "jedes Wintersemester ungerader Jahre", "unregelmäßig"}
	languages := []string{"Deutsch", "Deutsch", "Englisch", "Deutsch / Englisch"}
	exams := []string{"Klausur, 90 min.", "mündliche Prüfung, 30 min.", "Hausarbeit", "Klausur, 120 min. oder mündliche Prüfung", "Vortrag und schriftliche Ausarbeitung"}
	credits := []string{"6", "6", "5", "8", "4", "10", "3", "12"}

	const total = 1200
	seed := uint32(7)
	next := func(n int) int {
		seed = seed*1664525 + 1013904223
		return int(seed>>8) % n
	}
	type mod struct {
		id, title string
		programs  []string // "degree / name / PO po"
	}
	modules := make([]*mod, 0, total)
	titles := make(map[string]int)
	for i := 0; i < total; i++ {
		title := prefixes[next(len(prefixes))] + " " + subjects[next(len(subjects))] + suffixes[next(len(suffixes))]
		titles[title]++
		if titles[title] > 1 {
			title = fmt.Sprintf("%s (%d)", title, titles[title])
		}
		id := fmt.Sprintf("%05d", 20001+i)
		// Ids and a title the tests of the web tier and its browser checks look for.
		switch i {
		case 0:
			id, title = "11101", "Lineare Algebra"
		case 1:
			id, title = "11112", "Algorithmen und Datenstrukturen"
		}
		modules = append(modules, &mod{id: id, title: title})
	}

	// ---- programs and their trees ----
	informatik := fixtureProgram{name: "Informatik", stg: "079", abschl: "82", degree: "Bachelor (universitär)", pversion: "2008", po: "2008 - 2. SÄ 2024"}
	informatikMaster := fixtureProgram{name: "Informatik", stg: "079", abschl: "88", degree: "Master (universitär)", pversion: "2008", po: "2008 - 3. SÄ 2024"}
	maschinenbau := fixtureProgram{name: "Maschinenbau", stg: "055", abschl: "82", degree: "Bachelor (universitär)", pversion: "2020", po: "2020"}
	architektur := fixtureProgram{name: "Architektur", stg: "010", abschl: "82", degree: "Bachelor (universitär)", pversion: "2015", po: "2015"}

	cursor := 0
	take := func(n int) []string {
		ids := make([]string, 0, n)
		for i := 0; i < n && cursor < total; i++ {
			ids = append(ids, modules[cursor].id)
			cursor++
		}
		return ids
	}
	// The tree as the real one of Informatik B.Sc. reads (snapshot of 2026-09-21): complexes
	// whose labels say nothing about Pflicht or Wahlpflicht — the plan says what is compulsory —
	// and the Komplex Nebenfach with Praktische Mathematik and the subjects to choose from below
	// it. The thesis sits directly in the Fachstudium.
	informatik.areas = []fixtureArea{
		{label: "Grundstudium", children: []fixtureArea{
			{label: "Komplex Informatik", modules: take(9), children: []fixtureArea{
				{label: "Proseminar oder Praktikum", modules: take(3)},
			}},
			{label: "Komplex Mathematik", modules: take(3)},
			{label: "Komplex Nebenfach", children: []fixtureArea{
				{label: "Wahlpflichtmodule Praktische Mathematik", modules: take(5)},
				{label: "Mathematik", modules: take(8)},
				{label: "Physik", modules: take(4)},
				{label: "Maschinenbau / Elektrotechnik", modules: take(6)}, // the separator in a label, as the real tree has it
				{label: "Wirtschaftswissenschaften", modules: take(5)},
				{label: "Bauingenieurwesen", modules: take(4)},
			}},
		}},
		{label: "Fachstudium", modules: take(1), children: []fixtureArea{
			{label: "Grundlagen der Informatik", modules: take(7)},
			{label: "Praktische Informatik", modules: take(10)},
			{label: "Angewandte und Technische Informatik", modules: take(9)},
			{label: "Seminar oder Praktikum aus der Informatik", modules: take(4)},
		}},
	}
	// Informatik M.Sc. as the real tree reads: the Informatik-Vertiefung, and the Komplex
	// Nebenfach with Mathematik and the Anwendungen below it — Mathematik twice.
	informatikMaster.areas = []fixtureArea{
		{label: "Informatik-Vertiefung", children: []fixtureArea{
			{label: "Grundlagen der Informatik", modules: take(6)},
			{label: "Praktische Informatik", modules: take(11)},
			{label: "Angewandte und Technische Informatik", modules: take(16)},
			{label: "Seminare oder Praktika", modules: take(8)},
		}},
		{label: "Komplex Nebenfach", children: []fixtureArea{
			{label: "Mathematik", modules: take(6)},
			{label: "Anwendungen", children: []fixtureArea{
				{label: "Mathematik", modules: take(8)},
				{label: "Physik", modules: take(4)},
				{label: "Maschinenbau / Elektrotechnik", modules: take(6)},
				{label: "Wirtschaftsingenieurwesen", modules: take(4)},
				{label: "Bauingenieurwesen", modules: take(3)},
			}},
		}},
	}
	maschinenbau.areas = []fixtureArea{
		{label: "Grundlagen", children: []fixtureArea{
			{label: "Pflichtmodule Mathematik und Naturwissenschaften", modules: take(12)},
			{label: "Pflichtmodule Ingenieurwissenschaften", modules: take(18)},
		}},
		{label: "Vertiefung", children: []fixtureArea{
			{label: "Wahlpflichtmodule Energietechnik", modules: take(22)},
			{label: "Wahlpflichtmodule Produktionstechnik", modules: take(22)},
			{label: "Wahlpflichtmodule Fahrzeugtechnik", modules: take(18)},
		}},
		{label: "Bachelor-Arbeit", modules: take(1)},
	}
	// Architektur as the real tree reads: an account on top, and in each field its
	// Pflichtmodule and Wahlpflichtmodule, labels that say nothing but the kind.
	architektur.areas = []fixtureArea{
		{label: "Gesamtkonto Bachelor", children: []fixtureArea{
			{label: "Entwerfen", children: []fixtureArea{{label: "Pflichtmodule", modules: take(14)}}},
			{label: "Städtebau", children: []fixtureArea{{label: "Wahlpflichtmodule", modules: take(16)}}},
			{label: "Baukonstruktion", children: []fixtureArea{{label: "Wahlpflichtmodule", modules: take(14)}}},
		}},
		{label: "Bachelor-Arbeit", modules: take(1)},
	}
	// 60 of the modules are on the FÜS list.
	fuesIDs := take(60)

	byID := make(map[string]*mod, total)
	for _, m := range modules {
		byID[m.id] = m
	}
	programs := []fixtureProgram{informatik, informatikMaster, maschinenbau, architektur}
	// Many more programs, drawing on the modules of the pool (a module can be in several), so
	// that the pickers of the filter panel have long lists and the overview has its faculties.
	subjectNames := []string{"Physik", "Mathematik", "Bauingenieurwesen", "Stadtplanung", "Umweltwissenschaften", "Chemie", "Biotechnologie", "Wirtschaftsingenieurwesen", "Betriebswirtschaftslehre", "Wirtschaftsinformatik", "Medizininformatik", "Verfahrenstechnik", "Energietechnik", "Cyber Security", "Künstliche Intelligenz", "Landnutzung", "Kultur und Technik", "Pflegewissenschaft", "Soziale Arbeit", "Musikpädagogik", "Materialwissenschaft", "Nachhaltige Technik", "Robotik", "Data Science", "Umweltingenieurwesen", "Technologien Biogener Rohstoffe", "Wirtschaftsrecht", "Industrial Engineering", "Angewandte Mathematik", "Geoinformatik", "Weltkulturerbe", "Bauen und Erhalten", "Elektrische Energiesysteme", "Mikroelektronik", "Photonik", "Wirtschaftsmathematik", "Physiotherapie", "Hebammenwissenschaft", "Sicherheit und Gefahrenabwehr", "Digitale Medien", "Automotive Engineering", "Luftfahrttechnik", "Prozessinformatik", "Betriebliche Bildung", "Public Management", "Umweltrecht", "Wasserwirtschaft", "Bergbau", "Geologie", "Ökologie", "Biomedizintechnik", "Sportwissenschaft", "Europäische Studien", "Regionalentwicklung", "Textiltechnik"}
	pool := func(from, n int) []string {
		ids := make([]string, 0, n)
		for i := 0; i < n; i++ {
			ids = append(ids, modules[(from+i*7)%total].id)
		}
		return ids
	}
	for i, name := range subjectNames {
		stg := fmt.Sprintf("%03d", 100+i)
		bachelor := fixtureProgram{name: name, stg: stg, abschl: "82", degree: "Bachelor (universitär)", pversion: "2021", po: "2021"}
		if i%7 == 3 {
			bachelor.degree = "Bachelor (universitär) - Dual, praxisintegrierend"
		}
		bachelor.areas = []fixtureArea{
			{label: "Pflichtmodule " + name, modules: pool(i*40+60, 12)},
			{label: "Wahlpflichtmodule " + name, modules: pool(i*40+300, 16)},
			{label: "Bachelor-Arbeit", modules: pool(i*40+599, 1)},
		}
		master := fixtureProgram{name: name, stg: stg, abschl: "88", degree: "Master (universitär)", pversion: "2022", po: "2022"}
		if i%11 == 5 {
			master.degree = "Master (universitär) - Doppelabschluss"
		}
		master.areas = []fixtureArea{
			{label: "Pflichtmodule", modules: pool(i*40+700, 6)},
			{label: "Wahlpflichtmodule Vertiefung " + name, modules: pool(i*40+900, 14)},
			{label: "Master-Arbeit", modules: pool(i*40+1150, 1)},
		}
		programs = append(programs, bachelor, master)
	}
	poKey := func(p fixtureProgram) string {
		return "auswahlBaum|studiengang:stg=" + p.stg + "|abschluss:abschl=" + p.abschl + "|stgSpecials:vert=,schwp=,kzfa=H,pversion=" + p.pversion
	}
	for _, p := range programs {
		ref := p.degree + " / " + p.name + " / PO " + p.po
		var putArea func(key string, area fixtureArea, konto *int)
		putArea = func(key string, area fixtureArea, konto *int) {
			var links []string
			for _, child := range area.children {
				*konto++
				childKey := key + fmt.Sprintf("|konto:%d", *konto)
				links = append(links, childKey, child.label)
				putArea(childKey, child, konto)
			}
			for i, id := range area.modules {
				links = append(links, key+fmt.Sprintf("|pruefung:%d", i+1), id+" "+byID[id].title)
				byID[id].programs = append(byID[id].programs, ref)
			}
			put(catalogdb.SourceQISTree, treeBase+key, fixtureTreePage(p, links...))
		}
		konto := 0
		root := poKey(p)
		var links []string
		for _, area := range p.areas {
			konto++
			childKey := root + fmt.Sprintf("|konto:%d", konto)
			links = append(links, childKey, area.label)
			putArea(childKey, area, &konto)
		}
		put(catalogdb.SourceQISTree, treeBase+root, fixtureTreePage(p, links...))
	}

	// ---- module pages, the lists ----
	var list strings.Builder
	list.WriteString(`<table><tbody class="list">`)
	var fues strings.Builder
	fues.WriteString(`<table summary="Suchergebnis"><tr><th>Nr.</th><th>Modultitel</th><th>Sprache</th><th>LP</th><th>FÜS</th><th>Teilnehmerbeschränkung</th></tr>`)
	isFues := make(map[string]bool)
	for _, id := range fuesIDs {
		isFues[id] = true
	}
	eventNo := 200000
	var events []string
	for i, m := range modules {
		fmt.Fprintf(&list, `<tr><td class="moduleNumber"><a href="/modul/%s">%s</a></td><td class="title">%s</td></tr>`, m.id, m.id, m.title)
		if isFues[m.id] {
			fmt.Fprintf(&fues, `<tr><td>%s</td><td><a href="#">%s</a></td><td>Deutsch</td><td>6</td><td>ja</td><td></td></tr>`, m.id, m.title)
		}
		assignment := "<li>keine Zuordnung vorhanden</li>"
		if len(m.programs) > 0 {
			assignment = ""
			for _, ref := range m.programs {
				assignment += "<li>" + ref + "</li>"
			}
		}
		extra := `<tr><td>Zuordnung zu Studiengängen:</td><td><ul>` + assignment + `</ul></td></tr>`
		if isFues[m.id] {
			extra += `<tr><td>&nbsp;</td><td>Das Modul ist für das Fachübergreifende Studium zugelassen.</td></tr>`
		}
		if i%9 == 0 {
			eventNo++
			extra += fmt.Sprintf(`<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul><li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&veranstaltung.veranstid=%d">%d Vorlesung</a></li></ul></td></tr>`, eventNo, eventNo)
			// Room names as QIS prints them, one after the other (i is a multiple of 9 here);
			// their short forms are ZHG/HS.A, SD/11.301 and SFB/14C.103.
			rooms := []string{"Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus", "Gebäude 11 - Hörsaal SD - 11.301 Hörsaal C - Campus Sachsendorf", "Gebäude 14.C - SFB - 14C.103 Hörsaal - Campus Senftenberg"}
			events = append(events, fmt.Sprintf("%d", eventNo), eventPageHTML(m.title, "Vorlesung", rooms[(i/9)%3], "14.04.2026 bis 21.07.2026"))
		}
		// The remarks name the programs with their degree label, as the live pages do; that is
		// where „B.Sc." and „M.Sc." come from. No kind is stated here: the tree states it.
		var remarks []string
		for _, ref := range m.programs {
			parts := strings.Split(ref, " / ")
			// Not every program is named with its label (Architektur reads „Bachelor" then).
			if len(parts) < 3 || parts[1] == "Architektur" {
				continue
			}
			label := "B.Sc."
			if strings.HasPrefix(parts[0], "Master") {
				label = "M.Sc."
			}
			remarks = append(remarks, fmt.Sprintf("• Studiengang %s %s: Modul des Curriculums", parts[1], label))
		}
		if i%13 == 0 {
			remarks = append(remarks, "• Das Modul wird in Kooperation mit der Praxis angeboten.")
		}
		if len(remarks) > 0 {
			extra += `<tr><td>Bemerkungen:</td><td>` + strings.Join(remarks, " ") + `</td></tr>`
		}
		var page strings.Builder
		page.WriteString(`<html><body><div class="tx-btusysteme"><h1>` + m.id + ` - ` + m.title + ` <small>Modulübersicht</small></h1><table>
		<tr><td>Modulnummer:</td><td>` + m.id + `</td></tr>
		<tr><td>Modultitel:</td><td>` + m.title + `</td></tr>
		<tr><td>&nbsp;</td><td>` + m.title + ` (EN)</td></tr>
		<tr><td>Einrichtung:</td><td>` + departments[next(len(departments))] + `</td></tr>
		<tr><td>Verantwortlich:</td><td><ul><li>` + persons[next(len(persons))] + `</li></ul></td></tr>
		<tr><td>Lehr- und Prüfungssprache:</td><td>` + languages[next(len(languages))] + `</td></tr>
		<tr><td>Dauer:</td><td>1 Semester</td></tr>
		<tr><td>Angebotsturnus:</td><td>` + turnus[next(len(turnus))] + `</td></tr>
		<tr><td>Leistungspunkte:</td><td>` + credits[next(len(credits))] + `</td></tr>
		<tr><td>Empfohlene Voraussetzungen:</td><td>keine</td></tr>
		<tr><td>Zwingende Voraussetzungen:</td><td>keine</td></tr>
		<tr><td>Lehrformen und Arbeitsumfang:</td><td><ul><li>Vorlesung / 2 SWS</li><li>Übung / 2 SWS</li><li>Selbststudium / 120 Stunden</li></ul></td></tr>
		<tr><td>Modulprüfung:</td><td>Modulabschlussprüfung (MAP)</td></tr>
		<tr><td>Prüfungsleistung/en für Modulprüfung:</td><td>` + exams[next(len(exams))] + `</td></tr>
		<tr><td>Bewertung der Modulprüfung:</td><td>Prüfungsleistung - benotet</td></tr>
		<tr><td>Teilnehmerbeschränkung:</td><td>keine</td></tr>
		<tr><td>Inhalte:</td><td>Dieses Modul behandelt die Grundlagen von ` + m.title + `. Es vermittelt Begriffe, Methoden und Werkzeuge und übt sie an Beispielen ein.

Im zweiten Teil werden Anwendungen aus Forschung und Praxis vorgestellt.</td></tr>
		<tr><td>Lernziele:</td><td>Die Studierenden können die Konzepte von ` + m.title + ` erklären und auf neue Aufgaben anwenden.</td></tr>
		` + extra + `
	</table></div></body></html>`)
		put(catalogdb.SourceModulePage, m.id, page.String())
	}
	list.WriteString(`</tbody></table>`)
	fues.WriteString(`</table>`)
	put(catalogdb.SourceModuleCatalog, "list", list.String())
	put(catalogdb.SourceQISFUESList, "list", fues.String())
	for i := 0; i+1 < len(events); i += 2 {
		put(catalogdb.SourceQISEvent, events[i], events[i+1])
	}

	// ---- a validated plan for Informatik B.Sc.: the compulsory modules of the two complexes over
	// the first three semesters, then the rows the real plan has that name no module ----
	var entries []catalogdb.PlanEntry
	compulsory := append(append([]string{}, informatik.areas[0].children[0].modules...), informatik.areas[0].children[1].modules...)
	for i, id := range compulsory {
		semester := i/4 + 1
		if semester > 3 {
			semester = 3
		}
		entries = append(entries, catalogdb.PlanEntry{ModuleID: id, ModuleName: byID[id].title, Semester: semester, StartSemester: semester, EndSemester: semester, Credits: 6, KindRaw: "Pflicht", SubjectArea: "Grundstudium"})
	}
	requirement := func(name string, semester int, credits float64, area string) {
		entries = append(entries, catalogdb.PlanEntry{ModuleName: name, Semester: semester, StartSemester: semester, EndSemester: semester, Credits: credits, KindRaw: "Wahlpflicht", SubjectArea: area})
	}
	requirement("Anwendungsfach", 2, 6, "Grundstudium")
	requirement("Proseminar oder Praktikum", 3, 6, "Grundstudium")
	requirement("Modul aus dem Bereich Praktische Mathematik", 3, 6, "Grundstudium")
	requirement("Anwendungsfach", 3, 6, "Grundstudium")
	requirement("Fachübergreifendes Studium", 3, 6, "Grundstudium")
	requirement("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik", 4, 6, "Fachstudium")
	requirement("Komplex Grundlagen der Informatik", 4, 12, "Fachstudium")
	requirement("Anwendungsfach", 4, 6, "Grundstudium")
	requirement("Komplex Praktische Informatik", 5, 12, "Fachstudium")
	requirement("Komplex Angewandte und Technische Informatik", 5, 6, "Fachstudium")
	requirement("Komplex Praktische Informatik", 6, 6, "Fachstudium")
	requirement("Seminar oder Praktikum", 6, 6, "Fachstudium")
	thesis := informatik.areas[1].modules[0]
	entries = append(entries, catalogdb.PlanEntry{ModuleID: thesis, ModuleName: byID[thesis].title, Semester: 6, StartSemester: 6, EndSemester: 6, Credits: 12, KindRaw: "Abschlussarbeit"})
	if err := db.SavePlan(catalogdb.Plan{ProgramID: "079-82-2008", SourceFile: "po.pdf", LayoutJSON: "{}", Entries: entries}); err != nil {
		t.Fatalf("SavePlan failed: %v", err)
	}

	// Elektrotechnik B.Sc. 2022: one study plan per study direction, as the browser checks
	// expect it (two plans of 180 LP, 30 LP in the first semester, rows that name no module).
	elektrotechnik := fixtureProgram{name: "Elektrotechnik", stg: "042", abschl: "82", degree: "Bachelor (universitär)", pversion: "2022", po: "2022"}
	etPool := func(from, n int) []string { return pool(from, n) }
	// The tree as the real one reads: Grundstudium and Hauptstudium (phases, no headings), the
	// electives of Informatik in a node below it that says nothing but its kind and direction.
	elektrotechnik.areas = []fixtureArea{
		{label: "Grundstudium", children: []fixtureArea{
			{label: "Pflichtmodule Mathematik und Physik", modules: etPool(5, 12)},
			{label: "Pflichtmodule Elektrotechnik", modules: etPool(105, 18)},
			{label: "Informatik (MIT)", children: []fixtureArea{{label: "Wahlpflichtmodul (MIT)", modules: etPool(205, 8)}}},
			{label: "Informatik (EET)", children: []fixtureArea{{label: "Wahlpflichtmodul (EET)", modules: etPool(305, 8)}}},
			{label: "Informatik (PAu)", modules: etPool(605, 6)},
			{label: "Informatik (IoT)", modules: etPool(705, 6)},
		}},
		{label: "Hauptstudium", modules: etPool(805, 1), children: []fixtureArea{
			{label: "Studienrichtungsspezifische Vertiefungsmodule (MIT)", modules: etPool(405, 23)},
			{label: "Studienrichtungsspezifische Vertiefungsmodule (EET)", modules: etPool(505, 20)},
		}},
	}
	programs = append(programs, elektrotechnik)
	{
		p := elektrotechnik
		ref := p.degree + " / " + p.name + " / PO " + p.po
		var putArea func(key string, area fixtureArea, konto *int)
		putArea = func(key string, area fixtureArea, konto *int) {
			var links []string
			for _, child := range area.children {
				*konto++
				childKey := key + fmt.Sprintf("|konto:%d", *konto)
				links = append(links, childKey, child.label)
				putArea(childKey, child, konto)
			}
			for i, id := range area.modules {
				links = append(links, key+fmt.Sprintf("|pruefung:%d", i+1), id+" "+byID[id].title)
				if !strings.Contains(strings.Join(byID[id].programs, "\n"), ref) {
					byID[id].programs = append(byID[id].programs, ref)
				}
			}
			put(catalogdb.SourceQISTree, treeBase+key, fixtureTreePage(p, links...))
		}
		konto := 0
		root := poKey(p)
		var links []string
		for _, area := range p.areas {
			konto++
			childKey := root + fmt.Sprintf("|konto:%d", konto)
			links = append(links, childKey, area.label)
			putArea(childKey, area, &konto)
		}
		put(catalogdb.SourceQISTree, treeBase+root, fixtureTreePage(p, links...))
		var etEntries []catalogdb.PlanEntry
		common := append(append([]string{}, elektrotechnik.areas[0].children[0].modules...), elektrotechnik.areas[0].children[1].modules...)
		for v, spec := range []string{"Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium", "Regelstudienplan der Studienrichtungen PA und IoT im grundständigen Studium"} {
			// 20 modules of 6 LP over four semesters (30 LP each), two semesters of electives
			// (24 LP each) and the thesis (12 LP): 180 LP per study direction.
			for i, id := range common[:20] {
				semester := i/5 + 1
				etEntries = append(etEntries, catalogdb.PlanEntry{ModuleID: id, ModuleName: byID[id].title, Semester: semester, StartSemester: semester, EndSemester: semester, Credits: 6, KindRaw: "Pflicht", StudySection: "Grundstudium", Specialization: spec})
			}
			for semester := 5; semester <= 6; semester++ {
				etEntries = append(etEntries, catalogdb.PlanEntry{ModuleName: "Wahlpflichtmodule der Studienrichtung", Semester: semester, StartSemester: semester, EndSemester: semester, Credits: 18, KindRaw: "Wahlpflicht", StudySection: "Studienrichtung", Specialization: spec})
				etEntries = append(etEntries, catalogdb.PlanEntry{ModuleName: "Wahlpflichtmodul aus der Informatik", Semester: semester, StartSemester: semester, EndSemester: semester, Credits: 6, KindRaw: "Wahlpflicht", StudySection: "Studienrichtung", Specialization: spec})
			}
			thesis := elektrotechnik.areas[1].modules[0]
			etEntries = append(etEntries, catalogdb.PlanEntry{ModuleID: thesis, ModuleName: byID[thesis].title, Semester: 6, StartSemester: 6, EndSemester: 6, Credits: 12, KindRaw: "Abschlussarbeit", Specialization: spec})
			_ = v
		}
		if err := db.SavePlan(catalogdb.Plan{ProgramID: "042-82-2022", SourceFile: "po-et.pdf", LayoutJSON: "{}", Entries: etEntries}); err != nil {
			t.Fatalf("SavePlan (Elektrotechnik) failed: %v", err)
		}
	}

	if _, err := Build(context.Background(), db); err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	// The fixture is a catalog like any other: it passes validate (without the BTU baselines),
	// short names included.
	checks, err := db.Validate(context.Background(), nil)
	if err != nil {
		t.Fatalf("Validate failed: %v", err)
	}
	for _, c := range checks {
		if c.Status == catalogdb.StatusFail {
			t.Errorf("validate: %s = %d %v", c.Name, c.Value, c.Samples)
		}
	}
	snap, err := db.Export(context.Background(), dir)
	if err != nil {
		t.Fatalf("Export failed: %v", err)
	}
	t.Logf("fixture snapshot: %s (%d bytes)", filepath.Join(dir, snap.File), snap.Bytes)
}
