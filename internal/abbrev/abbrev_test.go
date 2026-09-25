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
		"12101\t\tA\towner",                      // too short
		"12101\t\tAu P\towner",                   // a space
		"12101\t\tABCDEFGHIJK\towner",            // too long
		"12101\t\tAuP\tsomeone",                  // unknown source
		"/[/\t\tAuP\towner",                      // bad pattern
		"Algorithmieren\t\tAuP\towner",           // neither number nor pattern
		"12101\t\tAuP\towner\n12101\t\tAP\tpage", // twice for every program
		"12101\tAuP\towner",                      // a column missing
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
	overrides, err := ParseOverrides("12101\tq\tALP\towner")
	if err != nil {
		t.Fatal(err)
	}
	modules := []Module{{"12101", "Algorithmieren und Programmieren"}}
	res := Derive(modules, []Member{{"p", "12101", 0, 1}, {"q", "12101", 0, 1}}, vocabulary, overrides)
	if res.Defaults["12101"].Abbrev != "AuP" || res.Programs["p"]["12101"].Abbrev != "AuP" || res.Programs["q"]["12101"].Abbrev != "ALP" {
		t.Errorf("default %v, p %v, q %v", res.Defaults["12101"], res.Programs["p"]["12101"], res.Programs["q"]["12101"])
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
