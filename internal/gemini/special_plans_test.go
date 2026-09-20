package gemini

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestSemesterPanelPDF(t *testing.T) {
	var b strings.Builder
	for sem := 1; sem <= 2; sem++ {
		x := float64(30 + (sem-1)*230)
		fmt.Fprintf(&b, "%.0f 300 200 100 re S %.0f 380 m %.0f 380 l S %.0f 300 m %.0f 400 l S\n", x, x, x+200, x+160, x+160)
		fmt.Fprintf(&b, "BT /F1 8 Tf %.0f 390 Td (%d. Semester) Tj ET BT /F1 8 Tf %.0f 393 Td (LP) Tj 0 -9 Td (6) Tj ET\n", x+4, sem, x+170)
		fmt.Fprintf(&b, "BT /F1 8 Tf %.0f 340 Td (Module %d) Tj ET BT /F1 8 Tf %.0f 340 Td (6) Tj ET\n", x+4, sem, x+170)
	}
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, b.String(), ""))
	if err != nil {
		t.Fatal(err)
	}
	if len(l.Cells) != 2 || len(l.Totals) != 2 {
		t.Fatalf("incorrect panels: %+v", l)
	}
	for i, c := range l.Cells {
		if !reflect.DeepEqual(c.Semesters, []int{i + 1}) || c.Min != 6 || c.Row != fmt.Sprintf("Module %d", i+1) {
			t.Fatalf("wrong panel: %+v", c)
		}
	}
}

func TestSpecialPlanPDFIntegration(t *testing.T) {
	root := os.Getenv("RADIX_PDF_TEST_DIR")
	if root == "" {
		t.Skip("set RADIX_PDF_TEST_DIR")
	}
	for _, tc := range []struct {
		name, file string
		pages      []int
		count      int
	}{{"Medizininformatik", "Medizininformatik/6497_06_1_Medinfo.pdf", nil, 25}, {"Elektrotechnik", "Elektrotechnik/6097_AMbl-21_2022_ET_B.Sc..pdf", []int{7, 9}, 50}} {
		t.Run(tc.name, func(t *testing.T) {
			l, err := ReadPDFLayoutPages(context.Background(), filepath.Join(root, tc.file), tc.pages)
			if err != nil {
				t.Fatal(err)
			}
			if len(l.Cells) != tc.count {
				t.Fatalf("got %d cells want %d", len(l.Cells), tc.count)
			}
			res := &CurriculumExtractionResult{StandardPeriodSemesters: 6, TotalCredits: 180}
			for _, c := range l.Cells {
				res.Modules = append(res.Modules, ExtractedModule{SourceCell: c.ID, ModuleName: c.Row})
			}
			if err := BindSourceCells(res, l); err != nil {
				t.Fatal(err)
			}
			if report := ValidateCurriculum(res, nil, "unknown", 6); !report.Valid {
				t.Fatalf("rejected plan: %+v", report)
			}
			if tc.name == "Medizininformatik" {
				totals := map[int]float64{}
				for _, c := range l.Totals {
					totals[c.Semesters[0]] = c.Min
				}
				if !reflect.DeepEqual(totals, map[int]float64{1: 28, 2: 32, 3: 32, 4: 32, 5: 26, 6: 30}) {
					t.Fatalf("wrong totals %v", totals)
				}
				for _, c := range l.Cells {
					if c.Page != 13 {
						t.Fatal("duplicate partial amendment plan retained")
					}
				}
			} else {
				found := false
				for _, c := range l.Cells {
					if c.Row == "Laborpraktikum der Elektrotechnik" {
						found = true
						if !reflect.DeepEqual(c.Semesters, []int{2, 3}) || !reflect.DeepEqual(c.Workload, []float64{3, 3}) || c.CreditSemester != 3 || c.Min != 6 {
							t.Fatalf("wrong laboratory assignment %+v", c)
						}
					}
				}
				if !found {
					t.Fatal("laboratory missing")
				}
				if len(l.PlanNames) != 2 {
					t.Fatal("study variants mixed")
				}
			}
		})
	}
}
