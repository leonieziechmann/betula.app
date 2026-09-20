package gemini

import (
	"context"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/internal/model"
)

func TestMissingCatalogCoverageIsVisibleWithoutInventedLinks(t *testing.T) {
	res := &CurriculumExtractionResult{Modules: []ExtractedModule{{ModuleName: "Historical module", ModuleType: "Pflicht", StartSemester: 1, EndSemester: 1, Credits: 6}}, Layout: &PDFLayout{Cells: []SourceCell{{Table: "p1", Semesters: []int{1}, Min: 6, Max: 6}}, Totals: []SourceCell{{Table: "p1", Row: "Summe", Semesters: []int{1}, Min: 6, Max: 6}}}, TotalCredits: 6, StandardPeriodSemesters: 1}
	r := ValidateCurriculum(res, nil, "winter", 6)
	found := false
	for _, issue := range r.Issues {
		if issue.Code == "catalog_coverage" {
			found = issue.Severity == "warning"
		}
	}
	if !found || !r.Valid || r.Matched != 0 {
		t.Fatalf("missing coverage warning: %+v", r)
	}
	res.Modules[0].ModuleType = "Wahlpflicht"
	r = ValidateCurriculum(res, nil, "winter", 6)
	for _, issue := range r.Issues {
		if issue.Code == "catalog_coverage" {
			t.Fatal("elective budget should not require a concrete catalog link")
		}
	}
}

// Opt-in tests exercise the actual Go geometry extractor against local
// university PDFs, which are not committed to this repository.
func TestPDFLayoutIntegration(t *testing.T) {
	root := os.Getenv("RADIX_PDF_TEST_DIR")
	if root == "" {
		t.Skip("set RADIX_PDF_TEST_DIR to the downloaded statutes directory")
	}
	for _, tc := range []struct{ file, fixture string }{
		{"Informatik/6707_12_Informatik_B.Sc.pdf", "informatik_2024_layout.json"},
		{"Wirtschaftsinformatik/6749_21_Wirtschaftsinformatik.pdf", "wirtschaftsinformatik_2024_layout.json"},
	} {
		t.Run(tc.fixture, func(t *testing.T) {
			got, err := ReadPDFLayout(context.Background(), filepath.Join(root, tc.file))
			if err != nil {
				t.Fatal(err)
			}
			data, err := os.ReadFile(filepath.Join("testdata", tc.fixture))
			if err != nil {
				t.Fatal(err)
			}
			var want PDFLayout
			if err := json.Unmarshal(data, &want); err != nil {
				t.Fatal(err)
			}
			for _, pair := range [][2][]SourceCell{{got.Cells, want.Cells}, {got.Totals, want.Totals}} {
				if len(pair[0]) != len(pair[1]) {
					t.Fatalf("cell count got %d want %d", len(pair[0]), len(pair[1]))
				}
				for i, g := range pair[0] {
					w := pair[1][i]
					if len(g.BBox) != 4 || len(w.BBox) != 4 {
						t.Fatal("missing geometry")
					}
					// Different readers average overlapping border strokes slightly
					// differently. All semantic fields must still match exactly.
					for k := range g.BBox {
						if math.Abs(g.BBox[k]-w.BBox[k]) > 0.25 {
							t.Errorf("%s bbox[%d]: got %f want %f", g.ID, k, g.BBox[k], w.BBox[k])
						}
					}
					g.BBox = nil
					w.BBox = nil
					if !reflect.DeepEqual(g, w) {
						t.Errorf("cell got %+v; want %+v", g, w)
					}
				}
			}
			if got.StartTerm != want.StartTerm {
				t.Errorf("start term got %s want %s", got.StartTerm, want.StartTerm)
			}
		})
	}
}

func TestWirtschaftsinformatikGoldenColumns(t *testing.T) {
	data, err := os.ReadFile("testdata/wirtschaftsinformatik_2024_layout.json")
	if err != nil {
		t.Fatal(err)
	}
	var layout PDFLayout
	if err := json.Unmarshal(data, &layout); err != nil {
		t.Fatal(err)
	}
	// Row order and columns read visually from physical page 8, ABl. 21/2024.
	want := []int{1, 2, 3, 4, 5, 5, 1, 2, 3, 3, 5, 1, 1, 2, 5, 6, 4, 1, 2, 3, 4, 5, 6, 6, 4, 6}
	res := &CurriculumExtractionResult{StandardPeriodSemesters: 6, TotalCredits: 180}
	for _, c := range layout.Cells {
		name := c.Row
		if parts := sourceModuleCode.FindStringSubmatch(name); parts != nil {
			name = parts[2]
		}
		res.Modules = append(res.Modules, ExtractedModule{SourceCell: c.ID, ModuleName: name, RecommendedSemester: 99, Credits: 999})
	}
	if err := BindSourceCells(res, &layout); err != nil {
		t.Fatal(err)
	}
	if len(res.Modules) != len(want) {
		t.Fatalf("got %d entries, expected %d", len(res.Modules), len(want))
	}
	for i, m := range res.Modules {
		if m.RecommendedSemester != want[i] {
			t.Errorf("%s: %d, want %d", m.ModuleName, m.RecommendedSemester, want[i])
		}
	}
	if r := ValidateCurriculum(res, nil, "unknown", 6); !r.Valid {
		t.Fatalf("source rejected: %+v", r)
	}
}

func TestIncompleteTableIsRejected(t *testing.T) {
	layout, res := informatikFixture(t)
	layout.Totals = nil
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	if r := ValidateCurriculum(res, nil, "auto", 6); r.Valid {
		t.Fatal("table fragment accepted without whole-plan totals")
	}
}

func informatikFixture(t *testing.T) (*PDFLayout, *CurriculumExtractionResult) {
	t.Helper()
	data, err := os.ReadFile("testdata/informatik_2024_layout.json")
	if err != nil {
		t.Fatal(err)
	}
	var layout PDFLayout
	if err = json.Unmarshal(data, &layout); err != nil {
		t.Fatal(err)
	}
	res := &CurriculumExtractionResult{StandardPeriodSemesters: 6, TotalCredits: 180}
	for _, c := range layout.Cells {
		res.Modules = append(res.Modules, ExtractedModule{SourceCell: c.ID, ModuleName: c.Row, RecommendedSemester: 6, Credits: 999, ModuleType: "Pflicht"})
	}
	return &layout, res
}

// Golden values were read visually from Anlage 2, physical page 7, ABl. 12/2024.
// Deliberately corrupt model predictions must never determine these assignments.
func TestInformatikGoldenSemesterColumns(t *testing.T) {
	layout, res := informatikFixture(t)
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	expected := map[string]int{
		"Entwicklung von Softwaresystemen": 1, "Algorithmieren und Programmieren": 2,
		"Theoretische Informatik": 3, "Betriebssysteme I": 4,
		"Elektrische und elektronische Grundlagen der Informatik": 1, "Digitaltechnik": 2,
		"Programmierpraktikum": 1, "Softwarepraktikum": 3, "Digitaltechnik-Praktikum": 4,
		"Proseminar oder Praktikum": 2, "Mathematik IT-1 (Diskrete Mathematik)": 1,
		"Mathematik IT-2 (Lineare Algebra)": 2, "Mathematik IT-3 (Analysis)": 3,
		"Modul aus dem Bereich Praktische Mathematik": 4, "Fachübergreifendes Studium": 1,
	}
	seen := 0
	sums := map[int]float64{}
	for _, m := range res.Modules {
		if sem, ok := expected[m.ModuleName]; ok {
			seen++
			if sem != m.RecommendedSemester {
				t.Errorf("%s: got %d want %d", m.ModuleName, m.RecommendedSemester, sem)
			}
		}
		if m.RecommendedSemester > 0 {
			sums[m.RecommendedSemester] += m.Credits
		}
		if m.StartSemester == 5 {
			if m.RecommendedSemester != 0 || m.EndSemester != 6 {
				t.Errorf("invented semester for span: %+v", m)
			}
		}
		if m.MinCredits > 0 && (m.Credits != 0 || m.MinCredits != 10 || m.MaxCredits != 24) {
			t.Errorf("invented range credits: %+v", m)
		}
	}
	if seen != len(expected) {
		t.Fatalf("covered %d/%d fixed modules", seen, len(expected))
	}
	if !reflect.DeepEqual(sums, map[int]float64{1: 32, 2: 28, 3: 30, 4: 30}) {
		t.Errorf("wrong totals: %v", sums)
	}
	before, _ := json.Marshal(res)
	report := ValidateCurriculum(res, nil, "auto", 6)
	if !report.Valid {
		t.Fatalf("valid source rejected: %+v", report)
	}
	after, _ := json.Marshal(res)
	if string(before) != string(after) {
		t.Fatal("validation changed source assignments")
	}
}

func TestSourceBindingRejectsDuplicateAndUnknownCells(t *testing.T) {
	for _, kind := range []string{"duplicate", "unknown"} {
		t.Run(kind, func(t *testing.T) {
			layout, res := informatikFixture(t)
			switch kind {
			case "duplicate":
				res.Modules = append(res.Modules, res.Modules[0])
			case "unknown":
				res.Modules[0].SourceCell = "fiction"
			}
			if err := BindSourceCells(res, layout); err == nil {
				t.Fatal("unverified evidence accepted")
			}
		})
	}
}

func TestSourceBindingRecoversMissingEnrichmentAndOriginalTitles(t *testing.T) {
	layout, res := informatikFixture(t)
	res.Modules = res.Modules[1:]
	res.Modules[0].ModuleName = "Incorrect AI title"
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	if len(res.Modules) != len(layout.Cells) {
		t.Fatal("source requirements lost")
	}
	for _, c := range layout.Cells {
		found := false
		for _, m := range res.Modules {
			if m.SourceCell == c.ID {
				found = true
				if !c.SharedRows && m.ModuleName != c.Row {
					t.Fatalf("source title changed: %+v", m)
				}
				if m.StartSemester != c.Semesters[0] || m.EndSemester != c.Semesters[len(c.Semesters)-1] {
					t.Fatal("source semester changed")
				}
			}
		}
		if !found {
			t.Fatalf("lost %s", c.ID)
		}
	}
}

func TestSharedElectiveCellIsRecoveredOnce(t *testing.T) {
	layout, res := informatikFixture(t)
	for i, c := range layout.Cells {
		if c.SharedRows {
			res.Modules = append(res.Modules[:i], res.Modules[i+1:]...)
			break
		}
	}
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	n := 0
	for _, m := range res.Modules {
		if strings.HasPrefix(m.ModuleName, "Wahlpflicht:") {
			n++
			if m.ModuleCode != "" || m.RecommendedSemester != 4 || m.Credits != 6 {
				t.Fatalf("wrong shared slot: %+v", m)
			}
		}
	}
	if n != 1 {
		t.Fatalf("got %d shared slots", n)
	}
}

func TestSeasonValidationAndAmbiguousIdentity(t *testing.T) {
	for _, tt := range []struct {
		term, offering, duration string
		sem                      int
		conflict                 bool
	}{
		{"winter", "jedes Sommersemester", "1 Semester", 1, true},
		{"winter", "Every summer semester", "1 Semester", 2, false},
		{"summer", "jedes Sommersemester", "1 Semester", 1, false},
		{"summer", "Every winter semester", "1 Semester", 1, true},
		{"unknown", "jedes Sommersemester", "1 Semester", 1, false},
		{"winter", "Every semester", "1 Semester", 1, false},
		{"winter", "jedes Sommersemester", "2 Semester", 1, false},
	} {
		res := &CurriculumExtractionResult{Modules: []ExtractedModule{{ModuleCode: "42", ModuleName: "Test", StartSemester: tt.sem, EndSemester: tt.sem, Credits: 6}}, Layout: &PDFLayout{}}
		cat := []model.CurriculumCatalogModule{{ID: "42", Turnus: tt.offering, Duration: tt.duration, Credits: 6}}
		report := ValidateCurriculum(res, cat, tt.term, 6)
		conflict := false
		for _, i := range report.Issues {
			if i.Code == "season_conflict" {
				conflict = true
			}
		}
		if conflict != tt.conflict {
			t.Errorf("%+v: %+v", tt, report)
		}
	}
	catalog := []model.CurriculumCatalogModule{{ID: "1", TitleDE: "Mathematik"}, {ID: "2", TitleDE: "Mathematik"}, {ID: "3", TitleDE: "Mathematik II"}}
	if MatchCatalogModule(ExtractedModule{ModuleName: "Mathematik"}, catalog) != nil {
		t.Fatal("ambiguous title matched")
	}
	if MatchCatalogModule(ExtractedModule{ModuleName: "Mathe"}, catalog) != nil {
		t.Fatal("substring matched")
	}
	if MatchCatalogModule(ExtractedModule{ModuleCode: "404", ModuleName: "Mathematik II"}, catalog) != nil {
		t.Fatal("unknown code fell back to different identity")
	}
}

func TestSourceTotalsDetectMissingCreditsAndLoad(t *testing.T) {
	layout, res := informatikFixture(t)
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	for i := range layout.Totals {
		if layout.Totals[i].Row == "Summe Studium" && layout.Totals[i].Semesters[0] == 1 {
			layout.Totals[i].Min = 45
			layout.Totals[i].Max = 45
		}
	}
	r := ValidateCurriculum(res, nil, "winter", 6)
	if r.Valid {
		t.Fatal("inconsistent totals accepted")
	}
	codes := map[string]bool{}
	for _, i := range r.Issues {
		codes[i.Code] = true
	}
	if !codes["source_total_conflict"] || !codes["semester_load"] {
		t.Fatalf("missing checks: %+v", r)
	}
}

func TestLegacyBalanceDoesNotInventAssignments(t *testing.T) {
	res := CurriculumExtractionResult{Modules: []ExtractedModule{{RecommendedSemester: 1, Credits: 30}, {RecommendedSemester: 1, Credits: 6, ModuleType: "Wahlpflicht"}, {RecommendedSemester: 2, Credits: 24}}}
	before, _ := json.Marshal(res)
	BalanceCurriculumSemesters(&res)
	after, _ := json.Marshal(res)
	if string(before) != string(after) {
		t.Fatal("balance changed source")
	}
}

func TestStandardPeriodComesFromSource(t *testing.T) {
	layout, res := informatikFixture(t)
	// A model may under- or overstate the standard period; the plan decides.
	res.StandardPeriodSemesters = 2
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	highest := 0
	for _, c := range layout.Cells {
		for _, sem := range c.Semesters {
			if sem > highest {
				highest = sem
			}
		}
	}
	if res.StandardPeriodSemesters != highest {
		t.Fatalf("standard period = %d, want %d from the source cells", res.StandardPeriodSemesters, highest)
	}
	for _, i := range ValidateCurriculum(res, nil, "winter", 6).Issues {
		if i.Code == "semester_bounds" {
			t.Fatalf("source semesters reported as out of bounds: %+v", i)
		}
	}
}
