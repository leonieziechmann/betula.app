package gemini

import (
	"context"
	"fmt"
	"github.com/leonieziechmann/betula/internal/model"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCatalogTextMatchingDoesNotInventIdentity(t *testing.T) {
	catalog := []model.CurriculumCatalogModule{{ID: "1", TitleDE: "Angewandte Programmierung I"}, {ID: "2", TitleDE: "Angewandte Programmierung II"}, {ID: "3", TitleDE: "Fachdidaktik Deutsch"}}
	for _, tc := range []struct{ name, want string }{{"Angewandte Programierung I", "1"}, {"Angewandte Programmierung III", ""}, {"Fachdidaktik Deutsch (beinhaltet fachdidaktisches Tagespraktikum, fTP)", "3"}, {"Programmierung", ""}} {
		m := MatchCatalogModule(ExtractedModule{ModuleName: tc.name}, wholeCatalog(catalog...))
		got := ""
		if m != nil {
			got = m.ID
		}
		if got != tc.want {
			t.Errorf("%q got %s want %s", tc.name, got, tc.want)
		}
	}
	catalog = append(catalog, model.CurriculumCatalogModule{ID: "4", TitleDE: "Fachdidaktik Deutsch"})
	if MatchCatalogModule(ExtractedModule{ModuleName: "Fachdidaktik Deutsch"}, wholeCatalog(catalog...)) != nil {
		t.Fatal("duplicate title must remain unresolved")
	}
}

// One title names several modules of the university: both „Grundlagen der
// Elektrotechnik" are current, Maschinenbau reads the one and Elektrotechnik
// the other. What the program itself claims tells them apart — and only that.
func TestProgramClaimResolvesModulesOfTheSameTitle(t *testing.T) {
	twins := []model.CurriculumCatalogModule{
		{ID: "12537", TitleDE: "Grundlagen der Elektrotechnik"},
		{ID: "12696", TitleDE: "Grundlagen der Elektrotechnik"},
	}
	row := ExtractedModule{ModuleName: "Grundlagen der Elektrotechnik"}
	for _, tc := range []struct {
		name   string
		claims map[string]bool
		want   string
	}{
		{"the program claims one of them", map[string]bool{"12696": true}, "12696"},
		{"another program claims the other", map[string]bool{"12537": true}, "12537"},
		{"the program claims neither", map[string]bool{"11903": true}, ""},
		{"the program claims both", map[string]bool{"12537": true, "12696": true}, ""},
		{"nothing is known about the program", nil, ""},
	} {
		got := ""
		if m := MatchCatalogModule(row, model.CurriculumCatalog{Modules: twins, Claims: tc.claims}); m != nil {
			got = m.ID
		}
		if got != tc.want {
			t.Errorf("%s: got %q want %q", tc.name, got, tc.want)
		}
	}
	// The claim breaks a tie; it never withholds a link that is unique anyway.
	lone := model.CurriculumCatalog{Modules: twins[:1], Claims: map[string]bool{"11903": true}}
	if m := MatchCatalogModule(row, lone); m == nil || m.ID != "12537" {
		t.Errorf("unique title withheld: %+v", m)
	}
}

// A plan cell prints the slot in front of the title. The printed code and the
// module it names still mean the same module.
func TestSlotMarkerIsNotAnIdentityConflict(t *testing.T) {
	c := model.CurriculumCatalogModule{ID: "11922", TitleDE: "Numerik & Simulation"}
	for _, tc := range []struct {
		printed  string
		conflict bool
	}{
		{"Numerik & Simulation", false},
		{"KI P Numerik & Simulation", false},
		{"SPB3 Numerik & Simulation", false},
		{"12 Numerik & Simulation", false},
		{"Vertiefung Numerik & Simulation", true},
		{"Numerik & Simulation II", true},
		{"KI P", true},
	} {
		if got := IdentityConflict(ExtractedModule{ModuleCode: "11922", ModuleName: tc.printed}, c); got != tc.conflict {
			t.Errorf("%q: conflict %v, want %v", tc.printed, got, tc.conflict)
		}
	}
}

func TestCatalogReconciliationDetectsLayoutError(t *testing.T) {
	l := &PDFLayout{Cells: []SourceCell{{ID: "a", Table: "t", Row: "Alpha", Semesters: []int{1}, Min: 8, Max: 8}}, Totals: []SourceCell{{Table: "t", Row: "Summe", Semesters: []int{1}, Min: 6, Max: 6}}}
	r := &CurriculumExtractionResult{Modules: []ExtractedModule{{SourceCell: "a"}}}
	if e := BindSourceCells(r, l); e != nil {
		t.Fatal(e)
	}
	v := ValidateCurriculum(r, wholeCatalog(model.CurriculumCatalogModule{ID: "1", TitleDE: "Alpha", Credits: 6}), "winter", 6)
	found := false
	for _, i := range v.Issues {
		found = found || i.Code == "catalog_suggests_layout_error"
	}
	if !found || v.Valid || r.Modules[0].Credits != 8 {
		t.Fatalf("%+v", v)
	}
}

func TestFormAndVerticalTextPreservePlan(t *testing.T) {
	content := studyPlanPDFContent("Summe")
	form := fmt.Sprintf("<< /Type /XObject /Subtype /Form /BBox [0 0 500 500] /Length %d >>\nstream\n%sendstream", len(content), content)
	path := writeTestPDF(t, "/X1 Do\nBT /F1 8 Tf 0 1 -1 0 490 30 Tm (Vertical annotation) Tj ET\n", "/XObject << /X1 6 0 R >>", form)
	l, e := ReadPDFLayout(context.Background(), path)
	if e != nil || len(l.Cells) != 4 {
		t.Fatalf("layout=%+v error=%v", l, e)
	}
}

func TestUnknownAmendmentIsRecordedAndPlanStaysAuthoritative(t *testing.T) {
	file := filepath.Join(t.TempDir(), "amendment.pdf")
	os.WriteFile(file, []byte("unknown amendment"), 0600)
	s, e := SelectRegulationSources(model.OfficialStudyProgram{}, []model.ProgramRegulationDocument{{DocType: "statute", LocalPath: "base.pdf"}, {DocType: "amendment", LocalPath: file}})
	if e != nil || s.Plan.LocalPath != "base.pdf" || len(s.Issues) != 0 || s.Reviews[0].SHA256 == "" || s.Reviews[0].Decision != "not_applied" {
		t.Fatalf("%+v %v", s, e)
	}
}

func TestRecoveredPDFLayouts(t *testing.T) {
	root := os.Getenv("RADIX_PDF_TEST_DIR")
	if root == "" {
		t.Skip("local statute PDFs required")
	}
	for _, tc := range []struct {
		file, hint string
		count      int
	}{
		{"Artificial_Intelligence/6704_09_AI.pdf", "Artificial Intelligence Master", 11},
		{"Lehramt_Primarstufe_Deutsch-Englisch/7004_AMbl-14_25.pdf", "Lehramt Bachelor", 25},
		{"Betriebswirtschaftslehre/4307_AMbl-28_2017_PStO_BWL_MSc_Uni.pdf", "BWL Master", 18},
		{"Physik/5634_AMbl-18_2021_NF_Physik_B.Sc..pdf", "Physik Bachelor", 25},
		{"Mathematik/6428_14_NF_Mathe_B.Sc..pdf", "Mathematik Bachelor", 22},
	} {
		t.Run(tc.file, func(t *testing.T) {
			l, e := ReadPDFLayoutForProgram(context.Background(), filepath.Join(root, tc.file), nil, tc.hint)
			if e != nil {
				t.Fatal(e)
			}
			if len(l.Cells) != tc.count {
				t.Fatalf("got %d cells want %d", len(l.Cells), tc.count)
			}
			r := &CurriculumExtractionResult{Modules: []ExtractedModule{{SourceCell: l.Cells[0].ID}}}
			if e = BindSourceCells(r, l); e != nil {
				t.Fatal(e)
			}
			v := ValidateCurriculum(r, wholeCatalog(), "auto", 6)
			if !v.Valid {
				for _, i := range v.Issues {
					if i.Severity == "error" {
						t.Error(i.Message)
					}
				}
			}
			for _, c := range l.Cells {
				if strings.Contains(tc.hint, "Mathematik") && c.Semesters[len(c.Semesters)-1] > 6 {
					t.Fatal("dual semester leaked")
				}
			}
		})
	}
}
