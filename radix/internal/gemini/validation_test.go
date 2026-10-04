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

	"github.com/leonieziechmann/betula/radix/internal/model"
)

// wholeCatalog is the university catalog as a program that claims nothing of
// its own sees it: every module, no tie-break.
func wholeCatalog(mods ...model.CurriculumCatalogModule) model.CurriculumCatalog {
	return model.CurriculumCatalog{Modules: mods}
}

func TestMissingCatalogCoverageIsVisibleWithoutInventedLinks(t *testing.T) {
	res := &CurriculumExtractionResult{Modules: []ExtractedModule{{ModuleName: "Historical module", ModuleType: "Pflicht", StartSemester: 1, EndSemester: 1, Credits: 6}}, Layout: &PDFLayout{Cells: []SourceCell{{Table: "p1", Semesters: []int{1}, Min: 6, Max: 6}}, Totals: []SourceCell{{Table: "p1", Row: "Summe", Semesters: []int{1}, Min: 6, Max: 6}}}, TotalCredits: 6, StandardPeriodSemesters: 1}
	r := ValidateCurriculum(res, wholeCatalog(), "winter", 6)
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
	r = ValidateCurriculum(res, wholeCatalog(), "winter", 6)
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
	if r := ValidateCurriculum(res, wholeCatalog(), "unknown", 6); !r.Valid {
		t.Fatalf("source rejected: %+v", r)
	}
}

func TestIncompleteTableIsRejected(t *testing.T) {
	layout, res := informatikFixture(t)
	layout.Totals = nil
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	if r := ValidateCurriculum(res, wholeCatalog(), "auto", 6); r.Valid {
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
	report := ValidateCurriculum(res, wholeCatalog(), "auto", 6)
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

// offlineBinding is what the reader makes of a layout without the model.
func offlineBinding(t *testing.T, layout func() *PDFLayout) *CurriculumExtractionResult {
	t.Helper()
	pdf := filepath.Join(t.TempDir(), "plan.pdf")
	if err := os.WriteFile(pdf, []byte("%PDF"), 0600); err != nil {
		t.Fatal(err)
	}
	client := NewClient("", "")
	client.layoutLoader = func(context.Context, string) (*PDFLayout, error) { return layout(), nil }
	res, err := client.ExtractCurriculumOffline(context.Background(), pdf, "")
	if err != nil {
		t.Fatal(err)
	}
	return res
}

// The model labels cells; which rows a plan has, in which order they stand and
// which variants the plan is split into is the document's. Whatever the model
// answers, the rows bind to what the offline reader makes of the same layout.
func TestModelAnswerBindsToTheOfflineOrderAndVariants(t *testing.T) {
	for _, tc := range []struct {
		name   string
		layout func() *PDFLayout
		model  []ExtractedModule
	}{{
		// Energietechnik und Energiewirtschaft prints a plan per study direction,
		// each opening with the same rows. The model answered the second
		// direction first and left the shared rows out: in that order „Höhere
		// Mathematik - T1" stood after „Wahlpflicht-Modul 4", and the plan took
		// its heading from the second table.
		name: "a plan per study direction",
		layout: func() *PDFLayout {
			return &PDFLayout{
				Cells: []SourceCell{
					{ID: "p16t1r4c2", Table: "p16t1", Page: 16, Row: "Höhere Mathematik - T1", Semesters: []int{1}, Min: 8, Max: 8},
					{ID: "p16t1r5c3", Table: "p16t1", Page: 16, Row: "Energiewirtschaft", Semesters: []int{2}, Min: 6, Max: 6},
					{ID: "p16t1r6c7", Table: "p16t1", Page: 16, Row: "Wahlpflicht-Modul 4", Semesters: []int{6}, Min: 6, Max: 6},
					{ID: "p16t2r4c2", Table: "p16t2", Page: 16, Row: "Höhere Mathematik - T1", Semesters: []int{1}, Min: 8, Max: 8},
					{ID: "p16t2r5c3", Table: "p16t2", Page: 16, Row: "Elektrische Maschinen", Semesters: []int{2}, Min: 6, Max: 6},
					{ID: "p16t2r6c7", Table: "p16t2", Page: 16, Row: "Wahlpflicht-Modul 4", Semesters: []int{6}, Min: 6, Max: 6},
				},
				PlanNames: map[string]string{"p16t1": "Studienrichtung Energieökonomie", "p16t2": "Studienrichtung Elektrische Energietechnik"},
			}
		},
		model: []ExtractedModule{
			{SourceCell: "p16t2r5c3", ModuleName: "Elektrische Maschinen", ModuleType: "Pflicht"},
			{SourceCell: "p16t2r6c7", ModuleName: "Wahlpflicht-Modul 4", ModuleType: "Wahlpflicht"},
			{SourceCell: "p16t1r5c3", ModuleName: "Energiewirtschaft", ModuleType: "Pflicht"},
			{SourceCell: "p16t1r6c7", ModuleName: "Wahlpflicht-Modul 4", ModuleType: "Wahlpflicht"},
		},
	}, {
		// Wirtschaftsmathematik prints one plan, and the model put „Komplex
		// Vertiefung" on the two cells of its row „Module (gemäß Anlage 3)": a
		// variant of two rows the document does not print. A track the plan
		// prints over an alternative names one, whatever the model says.
		name: "one plan",
		layout: func() *PDFLayout {
			return &PDFLayout{
				Cells: []SourceCell{
					{ID: "p8t1r4c3", Table: "p8t1", Page: 8, Row: "11101 Lineare Algebra und analytische Geometrie I", Semesters: []int{1}, Min: 8, Max: 8},
					{ID: "p8t1r12c5", Table: "p8t1", Page: 8, Row: "Ethik", Semesters: []int{3}, Min: 6, Max: 6, AltGroup: 1, Track: "Schwerpunkt Philosophie"},
					{ID: "p8t1r13c5", Table: "p8t1", Page: 8, Row: "Soziologie", Semesters: []int{3}, Min: 6, Max: 6, AltGroup: 1, AltIndex: 1, Track: "Schwerpunkt Gesellschaft"},
					{ID: "p8t1r16c7", Table: "p8t1", Page: 8, Row: "Module (gemäß Anlage 3)", Semesters: []int{5}, Min: 10, Max: 10},
					{ID: "p8t1r16c8", Table: "p8t1", Page: 8, Row: "Module (gemäß Anlage 3)", Semesters: []int{6}, Min: 10, Max: 10},
				},
				PlanNames: map[string]string{"p8t1": "Grundständig"},
			}
		},
		model: []ExtractedModule{
			{SourceCell: "p8t1r4c3", ModuleName: "Lineare Algebra und analytische Geometrie I", ModuleType: "Pflicht"},
			{SourceCell: "p8t1r12c5", ModuleName: "Ethik", ModuleType: "Wahlpflicht", Specialization: "Komplex Vertiefung"},
			{SourceCell: "p8t1r13c5", ModuleName: "Soziologie", ModuleType: "Wahlpflicht", Specialization: "Komplex Vertiefung"},
			{SourceCell: "p8t1r16c7", ModuleName: "Module (gemäß Anlage 3)", ModuleType: "Wahlpflicht", Specialization: "Komplex Vertiefung"},
			{SourceCell: "p8t1r16c8", ModuleName: "Module (gemäß Anlage 3)", ModuleType: "Wahlpflicht", Specialization: "Komplex Vertiefung"},
		},
	}} {
		t.Run(tc.name, func(t *testing.T) {
			offline := offlineBinding(t, tc.layout)
			layout := tc.layout()
			for i, c := range layout.Cells {
				if offline.Modules[i].SourceCell != c.ID {
					t.Fatalf("the offline reader left the order of the document at row %d", i+1)
				}
			}
			res := &CurriculumExtractionResult{Modules: tc.model}
			if err := BindSourceCells(res, layout); err != nil {
				t.Fatal(err)
			}
			if len(res.Modules) != len(offline.Modules) {
				t.Fatalf("%d rows, the offline reader has %d", len(res.Modules), len(offline.Modules))
			}
			for i, m := range res.Modules {
				o := offline.Modules[i]
				if m.SourceCell != o.SourceCell || m.Specialization != o.Specialization || m.SourcePlanLabel != o.SourcePlanLabel {
					t.Errorf("row %d: %s in %q under %q; offline %s in %q under %q", i+1,
						m.SourceCell, m.Specialization, m.SourcePlanLabel, o.SourceCell, o.Specialization, o.SourcePlanLabel)
				}
			}
		})
	}
}

// A thesis and the FÜS say what they are in their name, and the name wins over
// the model's label: the model called Elektrotechnik's „Bachelor-Arbeit"
// „Pflicht", which took the program its thesis and with it its faculty. A lab
// course named „…praktikum" is left to the model, and so is every row whose
// name says nothing about its kind.
func TestThesisAndFUESKindsComeFromTheName(t *testing.T) {
	layout := &PDFLayout{Cells: []SourceCell{
		{ID: "a", Table: "t", Row: "11477 Bachelor-Arbeit", Semesters: []int{6}, Min: 12, Max: 12},
		{ID: "b", Table: "t", Row: "Fachübergreifendes Studium (FÜS)", Semesters: []int{3}, Min: 6, Max: 6},
		{ID: "c", Table: "t", Row: "Programmierpraktikum", Semesters: []int{1}, Min: 4, Max: 4},
		{ID: "d", Table: "t", Row: "Betriebliche Phase 1", Semesters: []int{2}, Min: 15, Max: 15},
		{ID: "e", Table: "t", Row: "PhD Thesis Writing Skills", Semesters: []int{1}, Min: 3, Max: 3},
		{ID: "f", Table: "t", Row: "Master Thesis oder Master Thesis (Online)", Semesters: []int{4}, Min: 30, Max: 30},
	}}
	res := &CurriculumExtractionResult{Modules: []ExtractedModule{
		{SourceCell: "a", ModuleName: "Bachelor-Arbeit", ModuleType: "Pflicht"},
		{SourceCell: "b", ModuleName: "Fachübergreifendes Studium (FÜS)", ModuleType: "Wahlpflicht"},
		{SourceCell: "c", ModuleName: "Programmierpraktikum", ModuleType: "Pflicht"},
		{SourceCell: "d", ModuleName: "Betriebliche Phase 1", ModuleType: "Praktikum"},
		{SourceCell: "e", ModuleName: "PhD Thesis Writing Skills", ModuleType: "Pflicht"},
		{SourceCell: "f", ModuleName: "Master Thesis oder Master Thesis (Online)", ModuleType: "Abschlussarbeit"},
	}}
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"a": "Abschlussarbeit", // the name, over the model's „Pflicht"
		"b": "FÜS",             // the name, over the model's „Wahlpflicht"
		"c": "Pflicht",         // a lab course: the model's
		"d": "Praktikum",       // the name says nothing: the model's
		"e": "Pflicht",         // a course about a thesis is not one
		"f": "Wahlpflicht",     // a choice between two theses is a choice
	}
	for _, m := range res.Modules {
		if m.ModuleType != want[m.SourceCell] {
			t.Errorf("%s: %s, want %s", m.ModuleName, m.ModuleType, want[m.SourceCell])
		}
	}
}

// An internship says what it is in its name as well, and the name wins over the
// model's label: the model called „Bachelor-Praktikum" and „Industrial
// Internship" „Pflicht" and left „Praxis Musikschule (Praktikum Dual)" without a
// kind. A lab course and a Lehramt module with a school practicum carry the
// word too and keep the model's kind, and so does an internship whose name does
// not say what it is.
func TestInternshipKindComesFromTheName(t *testing.T) {
	layout := &PDFLayout{Cells: []SourceCell{
		{ID: "a", Table: "t", Row: "Bachelor-Praktikum", Semesters: []int{6}, Min: 18, Max: 18},
		{ID: "b", Table: "t", Row: "Industrial Internship (siehe § 32 Abs. 5)", Semesters: []int{3}, Min: 12, Max: 12},
		{ID: "c", Table: "t", Row: "14627 Praxis Musikschule (Praktikum Dual) I", Semesters: []int{1, 2}, Min: 12, Max: 12},
		{ID: "d", Table: "t", Row: "Programmierpraktikum", Semesters: []int{1}, Min: 4, Max: 4},
		{ID: "e", Table: "t", Row: "Fachdidaktik Mathematik (beinhaltet fachdidaktisches Tagespraktikum, fTP)", Semesters: []int{2}, Min: 6, Max: 6},
		{ID: "f", Table: "t", Row: "Praktikum Maschinelles Lernen", Semesters: []int{5}, Min: 4, Max: 4},
		{ID: "g", Table: "t", Row: "Betriebliche Phase 1", Semesters: []int{2}, Min: 15, Max: 15},
		{ID: "h", Table: "t", Row: "14257 11920 Wirtschaftspraktikum Wirtschaftsingenieurwesen oder Ingenieurpraktikum Wirtschaftsingenieurwesen", Semesters: []int{2}, Min: 6, Max: 6, Elective: true},
	}}
	res := &CurriculumExtractionResult{Modules: []ExtractedModule{
		{SourceCell: "a", ModuleName: "Bachelor-Praktikum", ModuleType: "Pflicht"},
		{SourceCell: "b", ModuleName: "Industrial Internship", ModuleType: "Pflicht"},
		{SourceCell: "c", ModuleName: "Praxis Musikschule (Praktikum Dual) I", ModuleType: "Modul"},
		{SourceCell: "d", ModuleName: "Programmierpraktikum", ModuleType: "Pflicht"},
		{SourceCell: "e", ModuleName: "Fachdidaktik Mathematik", ModuleType: "Pflicht"},
		{SourceCell: "f", ModuleName: "Praktikum Maschinelles Lernen", ModuleType: "Pflicht"},
		{SourceCell: "g", ModuleName: "Betriebliche Phase 1", ModuleType: "Praktikum"},
		{SourceCell: "h", ModuleName: "Wirtschaftspraktikum oder Ingenieurpraktikum", ModuleType: "Praktikum"},
	}}
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"a": "Praktikum",   // the name, over the model's „Pflicht"
		"b": "Praktikum",   // the name, over the model's „Pflicht"
		"c": "Praktikum",   // the name, where the model gave no kind
		"d": "Pflicht",     // a lab course: the model's
		"e": "Pflicht",     // a module with a school practicum in it: the model's
		"f": "Pflicht",     // a lab course: the model's
		"g": "Praktikum",   // the name says nothing: the model's
		"h": "Wahlpflicht", // a choice between two internships is a choice
	}
	for _, m := range res.Modules {
		if m.ModuleType != want[m.SourceCell] {
			t.Errorf("%s: %s, want %s", m.ModuleName, m.ModuleType, want[m.SourceCell])
		}
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
		report := ValidateCurriculum(res, wholeCatalog(cat...), tt.term, 6)
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
	if MatchCatalogModule(ExtractedModule{ModuleName: "Mathematik"}, wholeCatalog(catalog...)) != nil {
		t.Fatal("ambiguous title matched")
	}
	if MatchCatalogModule(ExtractedModule{ModuleName: "Mathe"}, wholeCatalog(catalog...)) != nil {
		t.Fatal("substring matched")
	}
	if MatchCatalogModule(ExtractedModule{ModuleCode: "404", ModuleName: "Mathematik II"}, wholeCatalog(catalog...)) != nil {
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
	r := ValidateCurriculum(res, wholeCatalog(), "winter", 6)
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
	for _, i := range ValidateCurriculum(res, wholeCatalog(), "winter", 6).Issues {
		if i.Code == "semester_bounds" {
			t.Fatalf("source semesters reported as out of bounds: %+v", i)
		}
	}
}
