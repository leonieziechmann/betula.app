package fuescatalog

import (
	"bufio"
	"compress/gzip"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

// fixture reads rows written from the BTU's catalogs (page, y, then x and text of each run; the
// column of whom a module is with left out): FÜS_Katalog_WiSe_2026_27.pdf of 2026-10-14 and
// FÜS_Katalog_SoSe_2026.pdf of 2026-03-24, as ReadRows reads them.
func fixture(t *testing.T, name string) []Row {
	t.Helper()
	f, err := os.Open(filepath.Join("testdata", name))
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	zr, err := gzip.NewReader(f)
	if err != nil {
		t.Fatal(err)
	}
	var rows []Row
	sc := bufio.NewScanner(zr)
	sc.Buffer(make([]byte, 1<<20), 1<<20)
	for sc.Scan() {
		cells := strings.Split(sc.Text(), "\t")
		page, _ := strconv.Atoi(cells[0])
		y, _ := strconv.ParseFloat(cells[1], 64)
		row := Row{Page: page, Y: y}
		for i := 2; i+1 < len(cells); i += 2 {
			x, _ := strconv.ParseFloat(cells[i], 64)
			row.Texts = append(row.Texts, Text{X: x, S: cells[i+1]})
		}
		rows = append(rows, row)
	}
	if err := sc.Err(); err != nil {
		t.Fatal(err)
	}
	return rows
}

func parsed(t *testing.T, name string) *Catalog {
	t.Helper()
	c, err := Parse(fixture(t, name))
	if err != nil {
		t.Fatal(err)
	}
	return c
}

func excluded(c *Catalog, program, scope string) map[string]bool {
	ids := map[string]bool{}
	for _, e := range c.Exclusions {
		if e.Program == program && e.Scope == scope {
			ids[e.ModuleID] = true
		}
	}
	return ids
}

func TestTheWintersCatalog(t *testing.T) {
	c := parsed(t, "katalog_wise_2026_27.tsv.gz")
	if c.Semester != "2026W" || c.Issued != "2026-10-14" {
		t.Errorf("semester %q, issued %q", c.Semester, c.Issued)
	}
	if len(c.Modules) != 152 || len(c.Programs) != 50 || len(c.Exclusions) != 1617 {
		t.Errorf("%d modules, %d programs, %d exclusions", len(c.Modules), len(c.Programs), len(c.Exclusions))
	}
	campus := map[string]string{}
	titles := map[string]string{}
	for _, m := range c.Modules {
		campus[m.ID], titles[m.ID] = m.Campus, m.Title
	}
	for id, want := range map[string]string{"15031": "sachsendorf", "11812": "senftenberg", "11101": "zentralcampus", "14886": "zentralcampus"} {
		if campus[id] != want {
			t.Errorf("%s at %q, want %q", id, campus[id], want)
		}
	}
	if titles["11812"] != "Energieeffiziente Fahrzeuge" || titles["11828"] != "Kolben- und Strömungsmaschinen" {
		t.Errorf("titles %q, %q", titles["11812"], titles["11828"])
	}
	if campus["12045"] != "" {
		t.Error("12045 is taught in the summer: not in the winter's catalog")
	}

	// Informatik: for every student, and for one Anwendungsfach each.
	if all := excluded(c, "Informatik", "Unabhängig vom Anwendungsfach"); len(all) != 12 || !all["12330"] || !all["36303"] {
		t.Errorf("Informatik for every Anwendungsfach: %v", all)
	}
	if physics := excluded(c, "Informatik", "Anwendungsfach Physik"); len(physics) != 3 || !physics["13102"] || !physics["31102"] {
		t.Errorf("Informatik, Anwendungsfach Physik (the list runs over a page): %v", physics)
	}
	if wiwi := excluded(c, "Informatik", "Anwendungsfach Wirtschaftswissenschaften"); len(wiwi) != 13 || !wiwi["14037"] {
		t.Errorf("Informatik, Anwendungsfach Wirtschaftswissenschaften: %v", wiwi)
	}
	// A program the heading of which names three; the degree and the profile as parts.
	city := "Städtebau und Stadtplanung/ Stadtplanung und Städtebau; Stadt- und Regionalplanung"
	if len(excluded(c, city, "")) == 0 {
		t.Errorf("%s has no exclusions; headings: %q", city, c.Programs)
	}
	if len(excluded(c, "Bauingenieurwesen", "für B.Sc")) == 0 || len(excluded(c, "Elektrotechnik", "mit anwendungsbezogenem Studienprofil")) == 0 {
		t.Error("the parts of Bauingenieurwesen and Elektrotechnik")
	}

	notes := map[string]string{}
	for _, n := range c.Notes {
		notes[n.Program+" | "+n.Scope] = n.Text
	}
	if notes["Bauen und Erhalten | "] != "Bitte bei der Studiengangleitung erfragen!" || notes["Informatik | Anwendungsfach Mathematik"] != "Keine weiteren Einschränkungen" {
		t.Errorf("notes: %v", notes)
	}
	for _, e := range c.Exclusions {
		if ScopeKind(e.Scope) == ScopeChoice && !strings.Contains(e.Scope, "Anwendungs") && !strings.HasPrefix(e.Scope, "Schwerpunkt") &&
			!strings.HasPrefix(e.Scope, "Studienrichtung") && e.Scope != "Studium in der Breite" {
			t.Errorf("a part no one chooses: %+v", e)
		}
	}
}

func TestTheSummersCatalog(t *testing.T) {
	c := parsed(t, "katalog_sose_2026.tsv.gz")
	if c.Semester != "2026S" || c.Issued != "2026-03-24" {
		t.Errorf("semester %q, issued %q", c.Semester, c.Issued)
	}
	if len(c.Modules) != 161 || len(c.Programs) != 51 || len(c.Exclusions) != 1390 {
		t.Errorf("%d modules, %d programs, %d exclusions", len(c.Modules), len(c.Programs), len(c.Exclusions))
	}
	found := false
	for _, m := range c.Modules {
		found = found || m.ID == "12045"
	}
	if !found {
		t.Error("Einführung in die Amateurfunktechnik (12045) is in the summer's catalog")
	}
	for _, e := range c.Exclusions {
		if e.Program == "Informatik" && e.ModuleID == "12045" {
			t.Errorf("Informatik does not exclude 12045: %+v", e)
		}
	}
}

func TestScopes(t *testing.T) {
	for scope, want := range map[string]string{
		"":                              ScopeAll,
		"Unabhängig vom Anwendungsfach": ScopeAll,
		"unabhängig vom Studienprofil":  ScopeAll,
		"Unabhängig vom Abschlussgrad":  ScopeAll,
		"für B.Sc":                      ScopeBachelor,
		"Nur für M.Sc":                  ScopeMaster,
		"mit universitärem und dualem Studienprofil":           ScopeProfile,
		"mit anwendungsbezogenem und dualem Studiengangprofil": ScopeProfile,
		"Anwendungsfach Physik":                                ScopeChoice,
		"Bei Anwendungsfach Economics and Social Sciences":     ScopeChoice,
		"Schwerpunkt Wirtschaftsmathematik":                    ScopeChoice,
		"Studienrichtung Umwelttechnik":                        ScopeChoice,
		"Studium in der Breite":                                ScopeChoice,
	} {
		if got := ScopeKind(scope); got != want {
			t.Errorf("ScopeKind(%q) = %q, want %q", scope, got, want)
		}
	}
	for _, c := range []struct {
		scope, degreeType string
		dual, want        bool
	}{
		{"mit anwendungsbezogenem Studienprofil", "applied", false, true},
		{"mit anwendungsbezogenem Studienprofil", "applied", true, false},
		{"mit anwendungsbezogenem Studienprofil", "university", false, false},
		{"mit anwendungsbezogenem und dualem Studienprofil", "applied", true, true},
		{"mit universitärem Studienprofil", "university", false, true},
		{"mit universitärem Studienprofil", "", false, true},
		{"mit universitärem Studienprofil", "university", true, false},
		{"mit universitärem und dualem Studienprofil", "university", true, true},
		{"mit universitärem und dualem Studienprofil", "applied", true, true},
		{"mit universitärem und dualem Studienprofil", "applied", false, false},
	} {
		if got := ProfileHolds(c.scope, c.degreeType, c.dual); got != c.want {
			t.Errorf("ProfileHolds(%q, %q, %v) = %v", c.scope, c.degreeType, c.dual, got)
		}
	}
	if got := ChoiceLabel("Bei Anwendungsfach Natural Sciences and Engineering:"); got != "Anwendungsfach Natural Sciences and Engineering" {
		t.Errorf("ChoiceLabel: %q", got)
	}
}

func TestLinksAndNames(t *testing.T) {
	page := []byte(`<div><h3>FÜS-Modulkatalog zum Wintersemester 2026/2027</h3>
		<p><a href="https://www-docs.b-tu.de/studierende/public/files/Informationen/F%c3%9cS_Katalog_WiSe_2026_27.pdf" target="_blank">FÜS-Modulkatalog zum Wintersemester 2026/2027</a></p>
		<p><a href="https://www-docs.b-tu.de/studierende/public/files/Formulare/Fues_Richtlinie.pdf">Richtlinie für das Fachübergreifende Studium</a></p>
		<p><a href="/files/F%c3%9cS_Katalog_SoSe_2026.pdf">Katalog</a> <a href="#c406102">FÜS-Modulkatalog zum Wintersemester 2026/2027</a></p></div>`)
	links := Links(page, "https://www.b-tu.de/studium/im-studium/studienorganisation/quicklinks/aktuelle-infos")
	want := []Link{
		{URL: "https://www-docs.b-tu.de/studierende/public/files/Informationen/F%c3%9cS_Katalog_WiSe_2026_27.pdf", Semester: "2026W"},
		{URL: "https://www.b-tu.de/files/F%c3%9cS_Katalog_SoSe_2026.pdf", Semester: "2026S"},
	}
	if len(links) != len(want) || links[0] != want[0] || links[1] != want[1] {
		t.Errorf("links:\n got %+v\nwant %+v", links, want)
	}

	files := "https://www-docs.b-tu.de/studierende/public/files/Informationen/"
	if got := CatalogURL(files, "2026W"); got != files+"F%C3%9CS_Katalog_WiSe_2026_27.pdf" {
		t.Errorf("CatalogURL 2026W: %s", got)
	}
	if got := CatalogURL(files, "2026S"); got != files+"F%C3%9CS_Katalog_SoSe_2026.pdf" {
		t.Errorf("CatalogURL 2026S: %s", got)
	}
	if got := CatalogURL(files, "2099W"); got != files+"F%C3%9CS_Katalog_WiSe_2099_00.pdf" {
		t.Errorf("CatalogURL 2099W: %s", got)
	}
	if Previous("2026W") != "2026S" || Previous("2026S") != "2025W" || Next("2026W") != "2027S" || Next("2026S") != "2026W" || Previous("nonsense") != "" {
		t.Error("Previous, Next")
	}
	day := func(s string) time.Time { d, _ := time.Parse("2006-01-02", s); return d }
	for date, want := range map[string]string{"2026-11-17": "2026W", "2027-02-01": "2026W", "2026-04-01": "2026S", "2026-09-30": "2026S"} {
		if got := SemesterAt(day(date)); got != want {
			t.Errorf("SemesterAt(%s) = %s, want %s", date, got, want)
		}
	}
	if SemesterOf("Für das Fachübergreifende Studium im Sommersemester 2026") != "2026S" || SemesterOf("im Wintersemester 2026/2027") != "2026W" || SemesterOf("FÜS") != "" {
		t.Error("SemesterOf")
	}
}

// TestWriteFixtures writes the fixtures from the BTU's PDFs; it runs only when asked to:
//
//	RADIX_FUES_CATALOG_PDFS=/path/FÜS_Katalog_WiSe_2026_27.pdf=katalog_wise_2026_27.tsv.gz,/path/FÜS_Katalog_SoSe_2026.pdf=katalog_sose_2026.tsv.gz \
//	  go test ./internal/fuescatalog -run TestWriteFixtures
//
// A row is its page, its y, then x and text of each run of pieces that start at one x. Above the
// list of programs the column of whom a module is with (x 395 to 495 of a module's row) is left out:
// the parse needs no names.
func TestWriteFixtures(t *testing.T) {
	spec := os.Getenv("RADIX_FUES_CATALOG_PDFS")
	if spec == "" {
		t.Skip("RADIX_FUES_CATALOG_PDFS not set")
	}
	for _, pair := range strings.Split(spec, ",") {
		pdfPath, name, ok := strings.Cut(pair, "=")
		if !ok {
			t.Fatalf("%q is no pdf=fixture pair", pair)
		}
		body, err := os.ReadFile(pdfPath)
		if err != nil {
			t.Fatal(err)
		}
		rows, err := ReadRows(body)
		if err != nil {
			t.Fatal(err)
		}
		out, err := os.Create(filepath.Join("testdata", name))
		if err != nil {
			t.Fatal(err)
		}
		zw := gzip.NewWriter(out)
		beforeIndex := true
		for _, r := range rows {
			text := r.String()
			if strings.HasPrefix(text, "Liste der Studiengänge") {
				beforeIndex = false
			}
			moduleRow := reModuleRow.MatchString(text)
			var runs []Text
			for _, piece := range r.Texts {
				if beforeIndex && moduleRow && piece.X >= 395 && piece.X < 495 {
					continue
				}
				if n := len(runs); n > 0 && runs[n-1].X == piece.X {
					runs[n-1].S += piece.S
					continue
				}
				runs = append(runs, piece)
			}
			line := strconv.Itoa(r.Page) + "\t" + strconv.FormatFloat(r.Y, 'f', 2, 64)
			for _, run := range runs {
				line += "\t" + strconv.FormatFloat(run.X, 'f', 2, 64) + "\t" + strings.ReplaceAll(run.S, "\t", " ")
			}
			if _, err := zw.Write([]byte(line + "\n")); err != nil {
				t.Fatal(err)
			}
		}
		if err := zw.Close(); err != nil {
			t.Fatal(err)
		}
		if err := out.Close(); err != nil {
			t.Fatal(err)
		}
	}
}
