package abbrev

import (
	"fmt"
	"math/rand"
	"reflect"
	"strings"
	"testing"
	"unicode"
)

// vocabulary stands in for the catalog's titles: the splitter knows the words of these, and
// „nachrichten“ is a bound head because three of them start with it.
var vocabulary = []string{
	"Nachrichtentechnik", "Nachrichtenübertragung", "Nachrichtensysteme", "Softwaresysteme",
	"Hilfe zur Erziehung", "Jugend und Gesellschaft", "Kinder in der Stadt",
	"Rechnernetze", "Netze und Rechner", "Quanten und Elektronen", "Elektromaschinen",
	"Theoretische Physik", "Praktikum", "Programmierpraktikum", "Programmieren",
	"Software Engineering", "Digital Design", "Elektrotechnik", "Elektrochemie", "Geo-Informatik", "Mikro und Nano",
}

// derive runs Derive over modules given as "id title" lines, one program of them all
// (compulsory unless the title says FÜS in the id's first letter: F12345).
func deriveTitles(t *testing.T, overrides []Override, lines ...string) (map[string]Choice, map[string]string) {
	t.Helper()
	var modules []Module
	var members []Member
	titles := append([]string(nil), vocabulary...)
	byTitle := map[string]string{}
	for _, l := range lines {
		id, title, _ := strings.Cut(l, " ")
		tier := 0
		if strings.HasPrefix(id, "F") {
			tier = 2
		} else if strings.HasPrefix(id, "E") {
			tier = 1
		}
		modules = append(modules, Module{id, title})
		members = append(members, Member{"p", id, tier, 99})
		titles = append(titles, title)
		byTitle[title] = id
	}
	res := Derive(modules, members, titles, overrides)
	got := map[string]string{}
	for id, c := range res.Programs["p"] {
		got[id] = c.Abbrev
	}
	return res.Programs["p"], got
}

func defaultOf(t *testing.T, overrides []Override, title string) string {
	t.Helper()
	res := Derive([]Module{{"1", title}}, nil, append([]string{title}, vocabulary...), overrides)
	return res.Defaults["1"].Abbrev
}

func TestTheRulesAlone(t *testing.T) {
	for title, want := range map[string]string{
		// the owner's examples come out of the rules, without the override file
		"Algorithmieren und Programmieren":                        "AuP",
		"Elektrische und elektronische Grundlagen der Informatik": "EEG",
		"Entwicklung von Softwaresystemen":                        "ESS", // not EvS: Software|systeme
		"Softwarepraktikum":                                       "SWP", // SP would be two letters
		"Digitaltechnik":                                          "DT",
		"Mathematik IT-1 (Diskrete Mathematik)":                   "MIT1",
		"Grundlagen der Rechnernetze":                             "GRN",
		"Theoretische Informatik":                                 "ThI",
		"Kinder- und Jugendhilfe":                                 "KJH",
		"Nachrichtentechnik":                                      "NT",
		"Nachrichtenübertragung":                                  "NÜ",
		"Quantenelektrodynamik":                                   "QED",
		"Geoinformationssysteme (GIS) für Ingenieure":             "GIS",
		"Advanced Geophysical Methods in Natural Resource Investigation (ANRI)": "ANRI",
		"Laborpraktikum der Elektrotechnik (IMT)":                               "LET", // (IMT) names a program
		"Deutsch als Fremdsprache B1.1":                                         "DaF-B1.1",
		"Analysis II":                                                           "An2",
		"Kombinatorik":                                                          "Kom",
		"Einführung in die Volkswirtschaftslehre":                               "VWL",
		"Embedded Real-Time Systems":                                            "ERTS",
		"Analysis and Modelling of Human-Environment Systems":                   "A&M",
		"Operating Systems II (Multi-Level Memory Management)":                  "OS2",
		"EUNICE Individual Pathway Bachelor's Module":                           "IPBM", // the possessive goes
	} {
		if got := defaultOf(t, nil, title); got != want {
			t.Errorf("%q → %q, want %q", title, got, want)
		}
	}
}

func TestTheOverrideFile(t *testing.T) {
	overrides, err := Overrides()
	if err != nil {
		t.Fatal(err)
	}
	if len(overrides) < 11 {
		t.Fatalf("%d override lines", len(overrides))
	}
	for title, want := range map[string]string{
		"Betriebssysteme I": "BS1",
		"Datenbanken":       "DB",
		"Allgemeine Betriebswirtschaftslehre III":              "ABWL3",
		"Allgemeine Betriebswirtschaftslehre III: Investition": "ABWL3",
		"Bachelor-Arbeit":                  "BA",
		"Master's Thesis":                  "MA",
		"Objektorientierte Programmierung": "OOP",
	} {
		if got := defaultOf(t, overrides, title); got != want {
			t.Errorf("%q → %q, want %q", title, got, want)
		}
	}
	res := Derive([]Module{{"12101", "Algorithmieren und Programmieren"}, {"38316", "Planung und Innovationsmanagement"}}, nil, vocabulary, overrides)
	if c := res.Defaults["12101"]; c.Abbrev != "AuP" || !c.Override {
		t.Errorf("12101 → %+v, want AuP from the override file", c)
	}
	if c := res.Defaults["38316"]; c.Abbrev != "PuI" || !c.Override {
		t.Errorf("38316 → %+v, want PuI from the override file", c)
	}
	if len(res.UnusedOverrides) == 0 || !strings.Contains(strings.Join(res.UnusedOverrides, "\n"), "12107 → EEG") {
		t.Errorf("unused overrides = %v, want the module numbers this catalog lacks", res.UnusedOverrides)
	}
}

func TestParseOverrides(t *testing.T) {
	good := "# comment\r\n12101\t\tAuP\towner\tnote\r\n/^Datenbanken$/\t079-82-2008\tDB\tcommon\r\n"
	o, err := ParseOverrides(good)
	if err != nil || len(o) != 2 || o[0].ModuleID != "12101" || o[1].Pattern == nil || o[1].Program != "079-82-2008" {
		t.Fatalf("ParseOverrides = %+v, %v", o, err)
	}
	for _, bad := range []string{
		"12101\t\tA\towner",                         // too short
		"12101\t\tAu P\towner",                      // a space
		"12101\t\tABCDEFGHIJK\towner",               // too long
		"12101\t\tAuP\tsomeone",                     // unknown source
		"/[/\t\tAuP\towner",                         // bad pattern
		"Algorithmieren\t\tAuP\towner",              // neither number nor pattern
		"12101\t\tAuP\towner\n12101\t\tAP\tpage",    // twice for every program
		"12101\tAuP\towner",                         // a column missing
		"12101\t\tAuP\towner\tnote\textra",          // a column too many
		"12101\t079-82-2008 \tAuP\towner",           // a program with a trailing space
		"12101\tnot-a-program\tAuP\towner",          // no program id
		"/^Foo/\t\tFo\tcommon\n/^Foo/\t\tFoo\tpage", // one pattern twice
	} {
		if _, err := ParseOverrides(bad); err == nil {
			t.Errorf("ParseOverrides(%q) accepted it", bad)
		}
	}
	// the same module in another program is fine
	if _, err := ParseOverrides("12101\t\tAuP\towner\n12101\t079-82-2008\tAP\towner"); err != nil {
		t.Errorf("a program's own line: %v", err)
	}
}

// A program-limited line applies in its program only.
func TestProgramOverride(t *testing.T) {
	const p, q = "P01-82-2026", "Q01-82-2026"
	overrides, err := ParseOverrides("12101\t" + q + "\tALP\towner")
	if err != nil {
		t.Fatal(err)
	}
	modules := []Module{{"12101", "Algorithmieren und Programmieren"}}
	res := Derive(modules, []Member{{p, "12101", 0, 1}, {q, "12101", 0, 1}}, vocabulary, overrides)
	if res.Defaults["12101"].Abbrev != "AuP" || res.Programs[p]["12101"].Abbrev != "AuP" || res.Programs[q]["12101"].Abbrev != "ALP" {
		t.Errorf("default %v, p %v, q %v", res.Defaults["12101"], res.Programs[p]["12101"], res.Programs[q]["12101"])
	}
	// A program's own line that comes out longer than 10 characters with the designator is
	// no candidate: the module keeps its derived form.
	overrides, err = ParseOverrides("11104\t" + q + "\tABCDEFGHIJ{n}\towner")
	if err != nil {
		t.Fatal(err)
	}
	res = Derive([]Module{{"11104", "Analysis II"}}, []Member{{q, "11104", 0, 1}}, vocabulary, overrides)
	if got := res.Programs[q]["11104"]; got.Abbrev != "An2" || got.Override {
		t.Errorf("an overlong program line → %+v, want the derived An2", got)
	}
}

// Lines that apply to no module are reported: a module the catalog lacks, a program without
// the module, a pattern that matches nothing, and one that an earlier pattern shadows.
func TestUnusedOverrides(t *testing.T) {
	overrides, err := ParseOverrides(strings.Join([]string{
		"12101\t\tAuP\towner",
		"12101\tQ01-82-2026\tALP\towner",
		"99999\t\tXY\tcommon",
		"/^Nichts$/\t\tNi\tcommon",
		"/^Algorithmieren/\t\tAlg\tcommon",
		"/^Algorithmieren und/\t\tAuP2\tcommon",
	}, "\n"))
	if err != nil {
		t.Fatal(err)
	}
	res := Derive([]Module{{"12101", "Algorithmieren und Programmieren"}}, []Member{{"P01-82-2026", "12101", 0, 1}}, vocabulary, overrides)
	got := strings.Join(res.UnusedOverrides, "\n")
	for _, want := range []string{
		"line 2: 12101 in Q01-82-2026 → ALP (the program has no such module)",
		"line 3: 99999 → XY (names no module)",
		"line 4: /^Nichts$/ → Ni (matches no module an earlier line does not take)",
		"line 6: /^Algorithmieren und/ → AuP2 (matches no module an earlier line does not take)",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("unused overrides lack %q:\n%s", want, got)
		}
	}
	if len(res.UnusedOverrides) != 4 {
		t.Errorf("unused overrides:\n%s", got)
	}
}

func TestResolutionWithinAProgram(t *testing.T) {
	// A candidate two modules want is nobody's: both fall back (the owner's rule).
	_, got := deriveTitles(t, nil, "1 Grundzüge der Makroökonomik", "2 Grundzüge der Mikroökonomik")
	if got["1"] != "GMa" || got["2"] != "GMi" {
		t.Errorf("Makro/Mikro → %v, want GMa / GMi", got)
	}
	_, got = deriveTitles(t, nil, "1 Personalmanagement", "2 Projektmanagement")
	if got["1"] != "Per" || got["2"] != "Pro" {
		t.Errorf("Personal/Projekt → %v, want Per / Pro", got)
	}
	// A FÜS module never takes a curricular module's form.
	choices, got := deriveTitles(t, nil, "F1 Datentechnik", "2 Digitaltechnik")
	if got["2"] != "DT" || got["F1"] == "DT" || choices["F1"].Choice == 1 {
		t.Errorf("FÜS vs curriculum → %v (%+v)", got, choices)
	}
	// Siblings: the same head, told apart by the subtitle.
	_, got = deriveTitles(t, nil, "1 Dynamik der Kraftfahrzeuge - Längsdynamik", "2 Dynamik der Kraftfahrzeuge - Querdynamik")
	if got["1"] != "DKL" || got["2"] != "DKQ" {
		t.Errorf("siblings → %v, want DKL / DKQ", got)
	}
	// Identical titles: the first in priority order keeps the form, the other gets -b.
	choices, got = deriveTitles(t, nil, "1 Häusliche Gewalt", "2 Häusliche Gewalt", "3 Häusliche Gewalt")
	if got["1"] != "HäG" || got["2"] != "HäG-b" || got["3"] != "HäG-c" || !choices["2"].Twin || choices["1"].Twin {
		t.Errorf("twins → %v", got)
	}
}

// Every abbreviation of a program is unique, whatever the titles, and 2 to 10 characters
// without spaces.
func TestUniqueWithinAProgram(t *testing.T) {
	var lines []string
	for i, title := range []string{
		"Projekt", "Projekt", "Projekt A", "Projektarbeit", "Projektmanagement", "Projektseminar",
		"Praktikum", "Praxisprojekt", "Proseminar", "Programmierpraktikum", "P", "X", "", "12",
		"Mathematik I", "Mathematik II", "Mathematik I", "Mathematik für Ingenieure I",
		"Grundlagen der Informatik", "Grundlagen der Informatik 1", "Einführung in die Informatik",
		"Deutsch als Fremdsprache A1", "Deutsch als Fremdsprache A1.1", "Deutsch als Fremdsprache A1.2",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul", "Wahlpflichtmodul",
		"Interdisziplinäres Projekt zur Technikfolgenabschätzung und Technikbewertung in der Praxis der Energiewirtschaft",
	} {
		prefix := ""
		if i%3 == 1 {
			prefix = "F"
		}
		lines = append(lines, fmt.Sprintf("%s%05d %s", prefix, i, title))
	}
	choices, _ := deriveTitles(t, nil, lines...)
	seen := map[string]string{}
	for id, c := range choices {
		k := strings.ToLower(c.Abbrev)
		if other, dup := seen[k]; dup {
			t.Errorf("%s and %s both get %q", other, id, c.Abbrev)
		}
		seen[k] = id
		if n := runes(c.Abbrev); n < 2 || n > 10 || strings.IndexFunc(c.Abbrev, unicode.IsSpace) >= 0 {
			t.Errorf("%s gets %q", id, c.Abbrev)
		}
	}
	if len(choices) != len(lines) {
		t.Errorf("%d choices for %d modules", len(choices), len(lines))
	}
}

// The same catalog always gives the same result, in whatever order it is read.
func TestDeterministic(t *testing.T) {
	var modules []Module
	var members []Member
	titles := append([]string(nil), vocabulary...)
	for i, title := range []string{
		"Grundzüge der Makroökonomik", "Grundzüge der Mikroökonomik", "Personalmanagement", "Projektmanagement",
		"Häusliche Gewalt", "Häusliche Gewalt", "Algorithmieren und Programmieren", "Datenbanken",
		"Dynamik der Kraftfahrzeuge - Längsdynamik", "Dynamik der Kraftfahrzeuge - Querdynamik",
		"Vertiefendes Integrationsmodul Stadt, Region, Landschaft 1", "Vertiefendes Integrationsmodul Architektur 1",
		"Vertiefendes Integrationsmodul Bauingenieurwesen 1", "Projekt", "Projektarbeit", "Proseminar",
	} {
		id := fmt.Sprintf("%05d", 10000+i)
		modules = append(modules, Module{id, title})
		titles = append(titles, title)
		for j, p := range []string{"a", "b", "c"} {
			if (i+j)%4 != 0 {
				members = append(members, Member{p, id, (i + j) % 3, 1 + i%5})
			}
		}
	}
	overrides, _ := Overrides()
	first := Derive(modules, members, titles, overrides)
	r := rand.New(rand.NewSource(1))
	for n := 0; n < 5; n++ {
		r.Shuffle(len(modules), func(i, j int) { modules[i], modules[j] = modules[j], modules[i] })
		r.Shuffle(len(members), func(i, j int) { members[i], members[j] = members[j], members[i] })
		r.Shuffle(len(titles), func(i, j int) { titles[i], titles[j] = titles[j], titles[i] })
		again := Derive(modules, members, titles, overrides)
		if !reflect.DeepEqual(first, again) {
			t.Fatalf("shuffle %d changed the result", n)
		}
	}
}

// A module whose every candidate is banned or taken gets its first one with a letter:
// -b, not a digit that would read as a series number.
func TestAnExhaustedListGetsALetter(t *testing.T) {
	d := &deriver{parsed: map[string]*parsed{"1": {title: "Eins"}, "2": {title: "Zwei"}}}
	one := []candidate{{text: "XY"}}
	got := d.contrast([]entry{{"1", 0, 1}, {"2", 0, 1}}, [][]candidate{one, one})
	if got["1"].Abbrev != "XY" || got["2"].Abbrev != "XY-b" || got["2"].Choice != 2 {
		t.Errorf("exhausted → %+v", got)
	}
}

func TestTheBlockedFile(t *testing.T) {
	m, err := parseBlocked(blockedTSV)
	if err != nil {
		t.Fatal(err)
	}
	for _, f := range []string{"SS", "SA", "NS", "KZ", "KKK", "NPD", "AfD", "THC", "NSA", "IBM", "BBC", "PO"} {
		if _, ok := m[strings.ToUpper(f)]; !ok {
			t.Errorf("%s is not on blocked.tsv", f)
		}
	}
	for _, bad := range []string{"SS", "SS\treason\textra", "S\ttoo short", "SS1\ta designator", "SS\tx\nss\ttwice"} {
		if _, err := parseBlocked(bad); err == nil {
			t.Errorf("parseBlocked(%q) accepted it", bad)
		}
	}
}

func TestStem(t *testing.T) {
	for form, want := range map[string]string{
		"ST1": "ST", "ST": "ST", "SS-A1": "SS", "DaF-B1.1": "DaF", "DaF-A1-A2": "DaF", "MIT1": "MIT",
		"KT1.1": "KT", "HäG-b": "HäG-b", "12": "12", "3D": "3D", "B&B": "B&B",
	} {
		if got := Stem(form); got != want {
			t.Errorf("Stem(%q) = %q, want %q", form, got, want)
		}
	}
}

func TestBlocked(t *testing.T) {
	for _, c := range []struct {
		form, title string
		blocked     bool
	}{
		{"KKK", "Krisen, Konflikte und Kommunikation", true},
		{"SS1", "Studienbezogene Schlüsselkompetenzen I", true},
		{"ss-A1", "Spanisch Start A1", true},
		{"Meth", "Methods", true},
		{"HG", "Hydrogeology", true}, // the Hauptgebäude of short room names
		{"ZHG", "Zukunft Hochschule Gestalten", true},
		{"SFB", "Sonderforschungsbereich", true},
		{"IBM", "IBM Watson in der Praxis", false}, // the title's own word
		{"NS", "Die NS-Zeit in der Lausitz", false},
		{"Po-A2", "Portugiesisch A2", true},
		{"SAP", "SAP-Grundlagen", false},
		{"AuP", "Algorithmieren und Programmieren", false},
		{"SR", "Schulrecht", false}, // room kinds follow a building: ZHG SR.4
	} {
		if _, got := Blocked(c.form, c.title); got != c.blocked {
			t.Errorf("Blocked(%q, %q) = %v", c.form, c.title, got)
		}
	}
	// derived forms fall back; an override may name a blocked form on purpose
	for title, blocked := range map[string]string{
		"Krisen, Konflikte und Kommunikation": "KKK",
		"Steuerungssysteme":                   "SS",
		"Nachrichtensysteme":                  "NS",
		"Hydrogeology":                        "HG",
	} {
		if got := defaultOf(t, nil, title); Stem(got) == blocked {
			t.Errorf("%q → %q, a blocked form", title, got)
		}
	}
	overrides, _ := ParseOverrides("/^Studienarbeit$/\t\tSA\tcommon")
	if got := defaultOf(t, overrides, "Studienarbeit"); got != "SA" {
		t.Errorf("an override line for SA → %q", got)
	}
}

// ST for Steuerungstechnik next to ST1 and ST2 for Systemtheorie I and II reads as one series:
// the stem is contested like a form. A series keeps one stem.
func TestStemsWithinAProgram(t *testing.T) {
	_, got := deriveTitles(t, nil, "1 Steuerungstechnik", "2 Systemtheorie I", "3 Systemtheorie II")
	if Stem(got["1"]) == Stem(got["2"]) || Stem(got["1"]) == Stem(got["3"]) {
		t.Errorf("Steuerungstechnik %q, Systemtheorie I %q, II %q share a stem", got["1"], got["2"], got["3"])
	}
	if Stem(got["2"]) != Stem(got["3"]) {
		t.Errorf("Systemtheorie I %q and II %q are one series", got["2"], got["3"])
	}
	// the stem contest follows the tiers: a FÜS module gives way
	_, got = deriveTitles(t, nil, "1 Physikalisches Praktikum I", "2 Physikalisches Praktikum II", "F3 Programmierpraktikum")
	if got["1"] != "PP1" || got["2"] != "PP2" || Stem(got["F3"]) == "PP" {
		t.Errorf("PP1 / PP2 / Programmierpraktikum → %v", got)
	}
}

// B&B and BB read as one form.
func TestNearDuplicates(t *testing.T) {
	d := &deriver{parsed: map[string]*parsed{"1": {title: "Biomass and Bioenergy", seriesKey: "a"}, "2": {title: "Brückenbau", seriesKey: "b"}}}
	got := d.contrast([]entry{{"1", 1, 1}, {"2", 1, 1}}, [][]candidate{{{text: "B&B"}, {text: "BiB", cost: 80}}, {{text: "BB"}, {text: "Brü", cost: 80}}})
	if got["1"].Abbrev != "BiB" || got["2"].Abbrev != "Brü" {
		t.Errorf("B&B / BB → %+v", got)
	}
}

// An override line beats a derived form, whatever the tier, and the forms of the owner's and
// the common lines are reserved: no other title derives AuP.
func TestOverridesComeFirst(t *testing.T) {
	overrides, err := Overrides()
	if err != nil {
		t.Fatal(err)
	}
	modules := []Module{{"12101", "Algorithmieren und Programmieren"}, {"14330", "Außeruniversitäres Praktikum"}, {"14331", "Algorithmieren und Programmieren II"}}
	res := Derive(modules, []Member{{"G29-82-2025", "14330", 0, 1}, {"G29-82-2025", "12101", 1, 2}}, vocabulary, overrides)
	if got := res.Programs["G29-82-2025"]; got["12101"].Abbrev != "AuP" || got["14330"].Abbrev == "AuP" {
		t.Errorf("the internship against the owner's AuP → %+v", got)
	}
	if got := res.Defaults["14330"].Abbrev; got == "AuP" {
		t.Errorf("Außeruniversitäres Praktikum derives the reserved AuP")
	}
	if got := res.Defaults["14331"].Abbrev; got != "AuP2" {
		t.Errorf("Algorithmieren und Programmieren II → %q, want AuP2 (one head)", got)
	}
	// siblings with one override line are told apart by their subtitle
	_, got := deriveTitles(t, overrides,
		"1 Allgemeine Betriebswirtschaftslehre III: Investition und Finanzierung",
		"E2 Allgemeine Betriebswirtschaftslehre III: Beschaffung, Produktion und Absatz",
		"3 Allgemeine Betriebswirtschaftslehre II: Betriebliche Sachfunktionen")
	if got["1"] != "ABWL3I" || got["E2"] != "ABWL3B" || got["3"] != "ABWL2" {
		t.Errorf("ABWL siblings → %v", got)
	}
}

func TestFunctionLettersAndCompounds(t *testing.T) {
	for title, want := range map[string]string{
		// a first-two-letters form must not read as a function word between capitals
		"Numerische Mathematik":  "NMa", // not NuM
		"Effiziente Algorithmen": "EAl", // not EfA, next to SfA „Statistik für Anwender“
		"Corporate Finance":      "CFi",
		"Statistik für Anwender": "SfA", // a real function word keeps its letter
		// an unknown part before an open head, and words written in parts
		"Deponietechnik":    "DT",
		"Umsatzbesteuerung": "Ums", // not Umsatzbe|steuerung
		"CampusTV":          "CTV",
		"eBusiness":         "EB",
	} {
		if got := defaultOf(t, nil, title); got != want {
			t.Errorf("%q → %q, want %q", title, got, want)
		}
	}
}
