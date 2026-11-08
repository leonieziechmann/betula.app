package normalize

import (
	"reflect"
	"testing"
)

// The raw values are the complete vocabularies found in btu_modules.db on 2026-09-19.

func TestTurnus(t *testing.T) {
	cases := map[string][2]string{
		"jedes Wintersemester":                 {SeasonWinter, ""},
		"Every winter semester":                {SeasonWinter, ""},
		"jedes Sommersemester":                 {SeasonSummer, ""},
		"Every summer semester":                {SeasonSummer, ""},
		"jedes Semester":                       {SeasonBoth, ""},
		"Every semester":                       {SeasonBoth, ""},
		"sporadisch nach Ankündigung":          {SeasonIrregular, ""},
		"On special announcement":              {SeasonIrregular, ""},
		"jedes Wintersemester gerader Jahre":   {SeasonWinter, ParityEven},
		"jedes Wintersemester ungerader Jahre": {SeasonWinter, ParityOdd},
		"jedes Sommersemester gerader Jahre":   {SeasonSummer, ParityEven},
		"jedes Sommersemester ungerader Jahre": {SeasonSummer, ParityOdd},
		"Each winter semester even year":       {SeasonWinter, ParityEven},
		"Each winter semester odd year":        {SeasonWinter, ParityOdd},
		"Each summer semester even year":       {SeasonSummer, ParityEven},
		"Each summer semester odd year":        {SeasonSummer, ParityOdd},
		"":                                     {"", ""},
		"alle drei Jahre":                      {"", ""},
	}
	for raw, want := range cases {
		season, parity := Turnus(raw)
		if season != want[0] || parity != want[1] {
			t.Errorf("Turnus(%q) = %q, %q; want %q, %q", raw, season, parity, want[0], want[1])
		}
	}
}

func TestLanguagesDurationLimitationGraded(t *testing.T) {
	if de, en := Languages("Deutsch"); !de || en {
		t.Errorf("Languages(Deutsch) = %v, %v", de, en)
	}
	for _, raw := range []string{"English", "Englisch"} {
		if de, en := Languages(raw); de || !en {
			t.Errorf("Languages(%q) = %v, %v", raw, de, en)
		}
	}
	if de, en := Languages("Deutsch/Englisch"); !de || !en {
		t.Errorf("Languages(both) = %v, %v", de, en)
	}

	durations := map[string]int{"1 Semester": 1, "1 semester": 1, "2 semesters": 2, "6 semesters": 6, "10 Wochen": 0, "": 0}
	for raw, want := range durations {
		got, ok := DurationSemesters(raw)
		if got != want || ok != (want > 0) {
			t.Errorf("DurationSemesters(%q) = %d, %v; want %d", raw, got, ok, want)
		}
	}

	type lim struct {
		known, limited bool
		limit          int
	}
	limits := map[string]lim{"": {}, "keine": {true, false, 0}, "None": {true, false, 0}, "80": {true, true, 80}, "max. 25 Teilnehmer": {true, true, 25}, "begrenzt": {true, true, 0}}
	for raw, want := range limits {
		known, limited, n := Limitation(raw)
		if (lim{known, limited, n}) != want {
			t.Errorf("Limitation(%q) = %v, %v, %d; want %+v", raw, known, limited, n, want)
		}
	}

	graded := map[string][2]bool{
		"Prüfungsleistung - benotet":        {true, true},
		"Performance Verification – graded": {true, true},
		"Studienleistung - unbenotet":       {false, true},
		"Study Performance – ungraded":      {false, true},
		"":                                  {false, false},
	}
	for raw, want := range graded {
		g, ok := Graded(raw)
		if g != want[0] || ok != want[1] {
			t.Errorf("Graded(%q) = %v, %v; want %v", raw, g, ok, want)
		}
	}
}

func TestExamFormAndKinds(t *testing.T) {
	forms := map[string]string{
		"Continuous Assessment (MCA)":                                        ExamFormMCA,
		"Modulabschlussprüfung (MAP)":                                        ExamFormMAP,
		"Final Module Examination (MAP)":                                     ExamFormMAP,
		"Voraussetzung + Modulabschlussprüfung (MAP)":                        ExamFormPrereqMAP,
		"Prerequisite + Final Module Examination (MAP)":                      ExamFormPrereqMAP,
		"Keine Angabe - Angabe ab Wintersemester 2016/17 erforderlich!":      "",
		"Unspecified - Specification from winter semester 2016/17 required!": "",
		"Graduierungsarbeit/externes Praktikum/Exkursion":                    ExamFormOther,
		"": "",
	}
	for raw, want := range forms {
		if got := ExamForm(raw); got != want {
			t.Errorf("ExamForm(%q) = %q, want %q", raw, got, want)
		}
	}

	k := ParseExamKinds("Voraussetzung: erfolgreiche Bearbeitung von Hausaufgaben. Modulabschlussprüfung: Klausur, 90 min. ODER mündliche Prüfung, 30 min.")
	if !k.Written || !k.Oral || !k.Practical || k.Presentation || k.Project {
		t.Errorf("ParseExamKinds(german) = %+v", k)
	}
	k = ParseExamKinds("Prerequisite: Research Report (max. 10000 caracters) Final Module Examination: Presentation (max. 30 min)")
	if !k.Paper || !k.Presentation || k.Written || k.Oral {
		t.Errorf("ParseExamKinds(english) = %+v", k)
	}
}

func TestTeachingFormAndWorkload(t *testing.T) {
	forms := map[string][]string{
		"Übung":                          {FormExercise}, // the filter that matched 0 of 1,535 modules
		"Exercise":                       {FormExercise},
		"Vorlesung":                      {FormLecture},
		"Vorlesung/Übung":                {FormLecture, FormExercise},
		"Seminar/Praktikum":              {FormSeminar, FormPractical},
		"Laborausbildung":                {FormPractical},
		"Laboratory training":            {FormPractical},
		"Self organised studies":         {FormSelfStudy},
		"Selbststudium":                  {FormSelfStudy},
		"Short term design project":      {FormProject},
		"musikalischer Einzelunterricht": {FormOther},
		"•":                              nil,
		"":                               nil,
	}
	for raw, want := range forms {
		if got := TeachingForm(raw); !reflect.DeepEqual(got, want) {
			t.Errorf("TeachingForm(%q) = %v, want %v", raw, got, want)
		}
	}

	for raw, want := range map[string][2]float64{
		"4 SWS":                         {4, 0},
		"2 Hours per Week per Semester": {2, 0},
		"150 Stunden":                   {0, 150},
		"120 Hours":                     {0, 120},
		"1,5 SWS":                       {1.5, 0},
		"":                              {0, 0},
	} {
		sws, hours := Workload(raw)
		if sws != want[0] || hours != want[1] {
			t.Errorf("Workload(%q) = %v, %v; want %v", raw, sws, hours, want)
		}
	}
}

func TestCampus(t *testing.T) {
	cases := map[string]string{
		"Lehrgebäude 1A - 0.22 - Zentralcampus":                         CampusZentral,
		"Gebäude 7 - 7.135 - Campus Sachsendorf":                        CampusSachsendorf,
		"Allgemeine Elektrotechnik Labor - 14.117 - Campus Senftenberg": CampusSenftenberg, // "lg" in Allgemeine
		"Lehrgebäude 4/3 - 321 - Campus Nord":                           CampusNord,
		"":                                                              "",
		"Online":                                                        "",
	}
	for room, want := range cases {
		if got := Campus(room); got != want {
			t.Errorf("Campus(%q) = %q, want %q", room, got, want)
		}
	}
}

func TestSemesterKeyAndPOVersion(t *testing.T) {
	for raw, want := range map[string]string{
		"SS 2026": "2026S", "SoSe 2026": "2026S", "Sommersemester 2026": "2026S",
		"WS 2026/27": "2026W", "WiSe 2026/27": "2026W", "Wintersemester 2026/2027": "2026W", "WS 26/27": "2026W",
		"": "", "2026": "",
	} {
		if got := SemesterKey(raw); got != want {
			t.Errorf("SemesterKey(%q) = %q, want %q", raw, got, want)
		}
	}

	year, amendment := POVersion("PO 2008 - 2. SÄ 2024")
	if year != 2008 || amendment != "2. SÄ 2024" {
		t.Errorf("POVersion = %d, %q", year, amendment)
	}
	if year, amendment = POVersion("2024 - NF 2026"); year != 2024 || amendment != "NF 2026" {
		t.Errorf("POVersion = %d, %q", year, amendment)
	}
	if year, amendment = POVersion("keine PO"); year != 0 || amendment != "" {
		t.Errorf("POVersion(keine PO) = %d, %q", year, amendment)
	}
}

// An event page abbreviates the day, the event search spells it out; both are the same date.
func TestWeekdayAndClock(t *testing.T) {
	days := map[string]int{
		"Mo.": 1, "Mi.": 3, "So.": 7, "Mittwoch": 3, "Sonntag": 7, "Wed": 3,
		"keine Angabe": 0, "": 0,
	}
	for raw, want := range days {
		if got := Weekday(raw); got != want {
			t.Errorf("Weekday(%q) = %d, want %d", raw, got, want)
		}
	}
	clocks := map[string]string{"07:30": "07:30", "7:30": "07:30", " 16:00 ": "16:00", "24:00": "24:00", "": "", "c.t.": ""}
	for raw, want := range clocks {
		if got := Clock(raw); got != want {
			t.Errorf("Clock(%q) = %q, want %q", raw, got, want)
		}
	}
}

func TestDegreeMeetsAcrossLanguages(t *testing.T) {
	pairs := [][2]string{
		{"Bachelor (universitär)", "Bachelor (research-oriented)"},
		{"Master (anwendungsbezogen)", "Master (applied)"},
		{"Bachelor (universitär) - Duales Studium, praxisintegrierend", "Bachelor (research-oriented) - Co-Op Programme with Practical Placement"},
		{"Master (universitär) - Duales Studium, praxisintegrierend", "Master (research-oriented) - Co-Op Programme with Practical Place"}, // truncated by QIS
		{"Bachelor (anwendungsbezogen) - Duales Studium, ausbildungsintegrierend", "Bachelor (applied) - Co-Op Programme with Integrated Vocational Training"},
		{"Master (universitär) - verringerte Fachsemester", "Master (research-oriented) - Reduced Semester"},
		{"Master (anwendungsbezogen) - erweiterte Fachsemester", "Master (applied) - Extended Semester"},
		{"Master (universitär) - Doppelabschluss", "Master (research-oriented) - Double Degree"},
		{"Master (universitär) - Fernstudium", "Master (research-oriented) - Distance Learning"},
		{"Strukturiertes Promotionsstudium", "Structural Doctoral Programme"},
		{"keine Abschlussprüfung möglich", "No Final Exam Possible"},
		{"LA Bachelor Grundstufe/Primarstufe", "Bachelor of Education"},
	}
	for _, p := range pairs {
		de, en := Degree(p[0]), Degree(p[1])
		if de != en || de.Level == "" || de.Level == LevelOther {
			t.Errorf("Degree(%q) = %+v, Degree(%q) = %+v", p[0], de, p[1], en)
		}
	}

	if d := Degree("Bachelor (universitär) - Duales Studium, praxisintegrierend"); d != (DegreeInfo{LevelBachelor, TypeUniversity, VariantDualPractice}) {
		t.Errorf("Degree(dual) = %+v", d)
	}
	if d := Degree("Abschluss im Ausland"); d.Level != LevelAbroad {
		t.Errorf("Degree(Ausland) = %+v", d)
	}
	// A degree nobody has seen yet still yields a usable value.
	if d := Degree("Zertifikat - Wochenendstudium"); d != (DegreeInfo{LevelOther, "", VariantOther}) {
		t.Errorf("Degree(unknown) = %+v", d)
	}
}

func TestShortDegreeLabels(t *testing.T) {
	got := ShortDegreeLabels("Studiengang Mathematik B.Sc.: Pflichtmodul. Study programme Artificial Intelligence M. Sc.; Wirtschaftsrecht LL.M.")
	if want := []string{"B.Sc.", "M.Sc.", "LL.M."}; !reflect.DeepEqual(got, want) {
		t.Errorf("ShortDegreeLabels = %v, want %v", got, want)
	}
	if !LabelMatchesLevel("B.Sc.", LevelBachelor) || LabelMatchesLevel("M.Sc.", LevelBachelor) || LabelMatchesLevel("B.A.", LevelDoctoral) {
		t.Error("LabelMatchesLevel is wrong")
	}
}

func TestDepartmentAndSlug(t *testing.T) {
	type dep struct {
		code, name string
		english    bool
	}
	for raw, want := range map[string]dep{
		"Fakultät 1 - MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik": {"1", "MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik", false},
		"Faculty 2 - Environment and Natural Sciences":                                         {"2", "Environment and Natural Sciences", true},
		"Fak. GW - Fakultät für Gesundheitswissenschaften Brandenburg":                         {"GW", "Fakultät für Gesundheitswissenschaften Brandenburg", false},
		"Faculty GW - Faculty of Health Sciences Brandenburg":                                  {"GW", "Faculty of Health Sciences Brandenburg", true},
		"ZES - Language Centre":               {"ZES", "Language Centre", true},
		"ZES - Zentrale Einrichtung Sprachen": {"ZES", "Zentrale Einrichtung Sprachen", false},
		"-":                                   {},
		"":                                    {},
	} {
		code, name, english := Department(raw)
		if (dep{code, name, english}) != want {
			t.Errorf("Department(%q) = %q, %q, %v", raw, code, name, english)
		}
	}

	if got := Slug("Künstliche Intelligenz Technologie – dual & Überblick"); got != "kuenstliche-intelligenz-technologie-dual-und-ueberblick" {
		t.Errorf("Slug = %q", got)
	}
}

func TestIsNoneAndModuleIDs(t *testing.T) {
	for _, raw := range []string{"", "-", "keine", "Keine.", "None", "k.A.", "entfällt"} {
		if !IsNone(raw) {
			t.Errorf("IsNone(%q) = false", raw)
		}
	}
	if IsNone("Schulmathematik") {
		t.Error("IsNone(Schulmathematik) = true")
	}
	if got := ModuleIDs("Module 11101 und 11102, siehe 11101; Raum 123456"); !reflect.DeepEqual(got, []string{"11101", "11102"}) {
		t.Errorf("ModuleIDs = %v", got)
	}
}

// The texts are those of the catalog (canary, 2026-10-16), as the parser reads them: a list item
// on a line of its own, starting with „• ".
func TestPrerequisiteIDs(t *testing.T) {
	cases := []struct {
		module, text string
		want         []string
	}{
		// The modules a text rules out are none of its prerequisites.
		{"11903", "Keine erfolgreiche Teilnahme am zugehörigen Auslaufmodul 12212 - Digitaltechnik.", nil},
		{"12212", "Keine erfolgreiche Teilnahme am zugehörigen Nachfolgemodul 11903 - Digitaltechnik.", nil},
		{"11107", "Keine erfolgreiche Teilnahme an den Modulen:\n• 11281- Höhere Mathematik T1 – BI\n• 11116 - Höhere Mathematik K", nil},
		{"11195", "No successful participation in associated follow-up module\n• 11689 Power Generation from Wind Energy AS WELL AS\n" +
			"in associated phase-out modules\n• 35415 Technology for Decentralized Power Generation and Storage 1 AND\n" +
			"• 35435 Renewable Generation and Storage of Electrical Energy.", nil},
		{"12164", "Keine erfolgreiche Teilnahme an zugehörigen Auslaufmodulen\n• 38407 Strategie und Umsetzung von Gründung und Wachstum SOWIE\n" +
			"• 38206 Gründungsmanagement\nALS AUCH am zugehörigen Nachfolgemodul\n• 12231 Gründungsmanagement.", nil},
		{"11999", "Keine Doppelbelegung mit Modul:\n• 13966 - Internetrecht und Datenschutzrecht", nil},
		{"43420", "Doppelbelegung mit zugehörigem Auslaufmodul 43402 - Behandlung fester Abfälle nicht zulässig.", nil},
		{"25106", "No double occupancy with Module 13676 - Denkmalpflege.", nil},
		{"12276", "Kein erfolgreicher Abschluss von Modul 14293 Porous Materials.", nil},
		{"13733", "• Kein Abschluss des Moduls 37105 Kultur und Gesellschaft", nil},
		{"13909", "• kein paralleler Besuch des Moduls 13947 - Seminar Empirische Wirtschaftsforschung (Hinweis: beide Seminare können in jeweils unterschiedlichen Semestern belegt werden)", nil},
		{"14358", "Keine erfolgreiche Belegung von Modul 12637 Digitale Fabrikplanung.", nil},
		{"13952", "No successfull completion of module 35463 - Labor Regelungstechnik", nil},
		{"14723", "No successful enrolment for 35443 International Management.", nil},
		{"14995", "Students must not have previously taken and passed Module 14297 (Polymer Chemistry)", nil},
		{"14293", "Module 12276 Porous Materials was not successfully completed.", nil},
		{"11626", "• Für Studierende, die bereits am Modul 12110 Mediendesign erfolgreich teilgenommen haben, ist eine Belegung nicht mehr möglich.", nil},
		// Only the statement that rules out a module loses it.
		{"11863", "• Successful participation in module 11862 - Pervasive System Security.\n• No successful participation in module 14478 - Praxis der Seitenkanal-Angriffe.", []string{"11862"}},
		{"", "Keine erfolgreiche Teilnahme am Modul 11111; Kenntnisse aus Modul 22222", []string{"22222"}},
		{"", "Keine erfolgreiche Teilnahme an 11111, aber Kenntnisse aus 22222 sind hilfreich.", []string{"22222"}},
		{"", "Keine erfolgreiche Teilnahme am Modul\n• 11111\nKenntnisse aus Modul 22222", []string{"22222"}},
		{"", "Keine Teilnahme an:\n• 11111\n• Erfolgreicher Abschluss von 22222", []string{"22222"}},
		// A text that negates something else keeps its modules.
		{"11293", "Kenntnis des Stoffes des Moduls\n• 11113: Mathematik IT-2 (Lineare Algebra)\nist wünschenswert, aber nicht zwingend erforderlich.", []string{"11113"}},
		{"11361", "Kenntnisse der Theoretischen Physik auf dem Niveau eines Bachelor-Studienganges, das Modul Computational Physics 13408 ist empfehlenswert, aber nicht Bedingung", []string{"13408"}},
		{"11975", "Knowledge of the contents of module\n• 12144 Personalökonomie und Industrielle Beziehungen\nis helpful but not essential.", []string{"12144"}},
		{"22406", "Das Modul 11444 (Grundlagen der Numerischen Mathematik) oder das Modul 11202 (Numerische Mathematik I) sollte als Voraussetzung vorher oder zeitlich parallel belegt werden. " +
			"Ein Abschluss ist für die Anmeldung zum Modul nicht erforderlich.", []string{"11444", "11202"}},
		{"23436", "11144 (MM1), 23427 (MM3)\n(This courses should be taken before or simultaneously with the module. A certificate is not required for the registration.)", []string{"11144", "23427"}},
		{"33327", "• Modul 33328 Grundlagen der Hochfrequenztechnik oder\n• Modul 13210 Hochfrequenztechnik 1 (nicht mehr im Angebot ab SS 2011)", []string{"33328", "13210"}},
		{"11881", "The module cannot be successfully completed without knowledge of the content of\n• 11112 Mathematik IT-1 (Diskrete Mathematik)\n• 11113 Mathematik IT-2 (Lineare Algebra)", []string{"11112", "11113"}},
		{"13565", "Kenntnis des Stoffes der Module\n• 11112 : Mathematik IT-1 (Diskrete Mathematik)\nOhne grundlegende Kenntnisse in diesem Gebiet ist ein Bestehen des Moduls schwer möglich.", []string{"11112"}},
		{"11605", "• Schwingungsanfällige Tragwerke (23503)\n• Nichtlineare Berechnungen und Stabilität (11512)", []string{"23503", "11512"}},
		{"11454", "Kenntnis des Stoffes von Modul:\n• 12101: Algorithmieren und Programmieren\nalternativ für Nicht-IT-Studiengänge:\n• 12105: Einführung in die Programmierung", []string{"12101", "12105"}},
		{"14315", "für Bachelor-Studierende:\nKenntnisse der Inhalte eines der folgenden Module\n• 12283 – Elektrische und magnetische Felder, ODER\n• 11866 – Allgemeine Physik II (Elektrizität und Magnetismus)\nfür Masterstudierende: keine", []string{"12283", "11866"}},
		{"", "Keine Teilnahmevoraussetzungen, Kenntnisse aus Modul 22222 sind hilfreich.", []string{"22222"}},
		{"", "Kein Abschluss erforderlich, Teilnahme an Modul 22222 genügt.", []string{"22222"}},
	}
	for _, c := range cases {
		if got := PrerequisiteIDs(c.text); !reflect.DeepEqual(got, c.want) {
			t.Errorf("%s: PrerequisiteIDs(%q) = %v, want %v", c.module, c.text, got, c.want)
		}
	}
}

// The texts are those of the catalog (betula.app, 2026-11-08), as the parser reads them.
func TestPrerequisiteChoices(t *testing.T) {
	cases := []struct {
		module, text string
		want         [][]string
	}{
		// What the owner found (2026-11-08): 12202 asks for one of the two Programmierpraktika and for
		// 12104, not for all three.
		{"12202", "Erfolgreiche Modulprüfung von:\n• Modul 12102 Programmierpraktikum ODER\n• Modul 11900 Programmierpraktikum (IMT)\nUND\n• Modul 12104 Entwicklung von Softwaresystemen",
			[][]string{{"12102", "11900"}, {"12104"}}},
		{"13616", "Erfolgreicher Abschluss der Module:\n• 11572 - Baustoffe und Tragwerke: Struktur und Material des Bauens oder\n• 13612 - Baustoffe und Tragwerke\nund\n" +
			"• 11573 - Bau- und Tragkonstruktion: Struktur des Gebäudes und der Gebäudehülle oder\n• 13614 - Bau- und Tragkonstruktion\nund\n" +
			"• 11574 - Baukonstruktion und Bauphysik: Eigenschaften der Gebäudehülle oder\n• 13613 - Baukonstruktion und Bauphysik",
			[][]string{{"11572", "13612"}, {"11573", "13614"}, {"11574", "13613"}}},
		// An „und" within a title, in brackets or not, joins nothing.
		{"13627", "erfolgreicher Abschluss folgender Module:\n• 13633 - Gebäudekunde und Entwerfen (GK1-B)\n• 25102 - Bau- und Stadtbaugeschichte 1 (GT1-B) ODER 25201 - Bau- und Stadtbaugeschichte 2 (GT2-B)\n• 13621 - Plastisches Gestalten, Grundlagen (K2-B)",
			[][]string{{"13633"}, {"25102", "25201"}, {"13621"}}},
		{"14212", "• Modul 11827 (Einführung in die Laborarbeit)\n• Modul 12287 (Organische Chemie I) ODER Modul 12725 (Organische Chemie)",
			[][]string{{"11827"}, {"12287", "12725"}}},
		{"14185", "• 12287 - Organische Chemie I\nODER\n• 12725 - Organische Chemie", [][]string{{"12287", "12725"}}},
		{"24301", "erfolgreicher Abschluss des Moduls 24101 Städtebau 1 (Grundlagen) Landschaftsplanung 1 (Grundlagen)\noder\nerfolgreicher Abschluss des Moduls 11555 Grundlagen des Städtebaus",
			[][]string{{"24101", "11555"}}},
		{"41313", "Erfolgreicher Abschluss des Moduls\n• 41207 Privatrecht I\nbzw. des Moduls:\n• 12156 Privatrecht I", [][]string{{"41207", "12156"}}},
		{"33436", "• erfolgreicher Abschluss des Moduls 13206 Hochfrequenztechnik 1 (nicht mehr im Angebot ab SS 2011)\noder\n• erfolgreicher Abschluss des Moduls 33328 Grundlagen der Hochfrequenztechnik",
			[][]string{{"13206", "33328"}}},
		{"11454", "Kenntnis des Stoffes von Modul:\n• 12101: Algorithmieren und Programmieren\nalternativ für Nicht-IT-Studiengänge:\n• 12105: Einführung in die Programmierung",
			[][]string{{"12101", "12105"}}},
		{"11214", "Kenntnis des Stoffes der Module\n• 11101: Lineare Algebra und analytische Geometrie I\n• 11118: Programmierkurs (Mathematik)\n• oder 11121: Programmierkurs (Wirtschaftsmathematik)",
			[][]string{{"11101"}, {"11118", "11121"}}},
		{"42438", "Module 12187 \"Ökologie und Management von Gewässern\" und/oder 12744 \"Gewässerschutz\" oder vergleichbare Kenntnisse.", [][]string{{"12187", "12744"}}},
		{"23444", "Teilnahme am Modul 23302 bzw. 12688 - Geschichte ist erforderlich!", [][]string{{"23302", "12688"}}},
		// „eines der Module": one of the list is enough.
		{"11170", "Vorheriger erfolgreicher Abschluss eines der Module:\n• 22401 „Baukonstruktion“ (BT A1)\n• 22403 „Tragwerkslehre“ (BT A2)\n• 22404 „Gebäudetechnik“ (BT A3)\n• 22405 „Baustoffe/Bauphysik/Bautenschutz“ (BT A4)",
			[][]string{{"22401", "22403", "22404", "22405"}}},
		{"13927", "Students have knowledge of at least one of the following modules\n• 13922 Introduction to Transformation Studies\n• 13932 Transformation Theories",
			[][]string{{"13922", "13932"}}},
		// A list without a conjunction asks for all of it.
		{"12326", "erfolgreicher Abschluss der Module\n• 12201 Hardwarepraktikum\n• 12202 Softwarepraktikum", [][]string{{"12201"}, {"12202"}}},
		{"41302", "Erfolgreicher Abschluss der Module 41207 Privatrecht I und 41213 Privatrecht II", [][]string{{"41207"}, {"41213"}}},
		// An „oder" that goes on with something other than the next module.
		{"12895", "• Kenntnisse aus Regelungstechnik 1 (Modul 12894) oder Grundlagen der Regelungstechnik oder vergleichbare Module\n• 11108 Höhere Mathematik - T2",
			[][]string{{"12894"}, {"11108"}}},
		{"23506", "Die Module 35453 (MM5), 35321 (HI6), 23401 (HI8.1) sollten vorher oder zeitlich parallel belegt werden. Ein Abschluss ist aber für die Anmeldung zu Modul 23506 nicht erforderlich.",
			[][]string{{"35453"}, {"35321"}, {"23401"}, {"23506"}}},
		// An „oder" between two lists, or between pairs, opposes them: no choice of single modules
		// says that, so every module is needed.
		{"11202", "Kenntnis des Stoffes der Module\n• 11103: Analysis I und\n• 11101: Lineare Algebra und analytische Geometrie I\noder gute Kenntnis des Stoffes der Module\n" +
			"• 11213: Mathematik IT-3 (Analysis) und\n• 11113: Mathematik IT-2 (Lineare Algebra)",
			[][]string{{"11103"}, {"11101"}, {"11213"}, {"11113"}}},
		{"13297", "Strongly recommended:\n• 12894 Regelungstechnik 1 and 12895 Regelungstechnik 2 or\n• 11494 Control Engineering 1 and 11747 Control Engineering 2",
			[][]string{{"12894"}, {"12895"}, {"11494"}, {"11747"}}},
		// What a text rules out is no alternative.
		{"", "Erfolgreicher Abschluss von Modul 11111 oder 22222. Keine erfolgreiche Teilnahme am Auslaufmodul 33333.", [][]string{{"11111", "22222"}}},
		// The same said twice is said once; a module alone and among alternatives is needed alone,
		// and so is every module of its choice.
		{"", "Modul 11111 oder 22222 / Module 11111 or 22222", [][]string{{"11111", "22222"}}},
		{"", "Modul 11111 und Modul 33333\n• 11111 oder\n• 22222", [][]string{{"11111"}, {"33333"}, {"22222"}}},
		{"", "-", nil},
	}
	for _, c := range cases {
		got := PrerequisiteChoices(c.text)
		if !reflect.DeepEqual(got, c.want) {
			t.Errorf("%s: PrerequisiteChoices(%q) = %v, want %v", c.module, c.text, got, c.want)
		}
		var flat []string
		for _, choice := range got {
			flat = append(flat, choice...)
		}
		if ids := PrerequisiteIDs(c.text); !reflect.DeepEqual(flat, ids) {
			t.Errorf("%s: the choices %v name other modules than PrerequisiteIDs, %v", c.module, got, ids)
		}
	}
}
