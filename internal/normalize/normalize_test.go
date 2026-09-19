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
