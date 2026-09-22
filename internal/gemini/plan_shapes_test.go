package gemini

import (
	"context"
	"fmt"
	"strings"
	"testing"
)

// sitePlanPDF draws a four-semester matrix whose semester headings carry, beside
// the number, the place that semester is spent — the shape Control of Renewable
// Energy Systems and Transfers-Fluids-Materials print. head supplies the four
// headings so the same plan can be drawn with a heading that means something
// else.
func sitePlanPDF(head [4]string) string {
	var b strings.Builder
	xs := []float64{40, 230, 290, 350, 410, 470, 530}
	rows := [][]string{
		{"Modul", "Leistungspunkte (LP) im Semester", "", "", "", "Summe LP"},
		{"", head[0], head[1], head[2], head[3], ""},
		{"Renewable Energy Systems", "30", "", "", "", "30"},
		{"Control and Grid Integration", "", "30", "", "", "30"},
		{"Geothermal Energy", "", "", "30", "", "30"},
		{"Master Thesis", "", "", "", "30", "30"},
		{"Summe", "30", "30", "30", "30", "120"},
	}
	for ri, row := range rows {
		for ci, txt := range row {
			x, y, w, h := xs[ci], 420-float64(ri+1)*24, xs[ci+1]-xs[ci], 24.0
			fmt.Fprintf(&b, "%.1f %.1f %.1f %.1f re S\n", x, y, w, h)
			if txt != "" {
				fmt.Fprintf(&b, "BT /F1 7 Tf 1 0 0 1 %.1f %.1f Tm (%s) Tj ET\n", x+2, y+h-14, txt)
			}
		}
	}
	return b.String()
}

// A semester column may be headed „1 ECN" — the number and the partner
// university the semester is spent at. The number decides the column, exactly as
// a bare „1" does, and the label is kept as what the plan says about that
// semester.
func TestASemesterHeaderMayNameWhereTheSemesterIsSpent(t *testing.T) {
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, sitePlanPDF([4]string{"1 ECN", "2 UNIZG", "3 BTU", "4 Thesis"}), ""))
	if err != nil {
		t.Fatal(err)
	}
	if len(l.Cells) != 4 {
		t.Fatalf("got %d cells, want the four modules: %+v", len(l.Cells), l.Cells)
	}
	want := []string{"ECN", "UNIZG", "BTU", "Thesis"}
	for i, c := range l.Cells {
		if len(c.Semesters) != 1 || c.Semesters[0] != i+1 {
			t.Errorf("cell %d sits in %v, want semester %d", i, c.Semesters, i+1)
		}
		if c.Section != want[i] {
			t.Errorf("cell %d names %q, want %q", i, c.Section, want[i])
		}
	}
	totals := DerivePlanTotals(l)
	if len(totals) != 4 {
		t.Fatalf("got %d sums, want one per semester: %+v", len(totals), totals)
	}
	for i, g := range totals {
		if g.Credits != 30 || g.Start != i+1 || g.End != i+1 {
			t.Errorf("sum %d is %v over %d-%d, want 30 in semester %d", i, g.Credits, g.Start, g.End, i+1)
		}
	}
}

// The same reading must refuse a heading that counts something other than
// semesters. „1. Studienjahr" spans two semesters, so reading it as the first
// one puts every module in the wrong semester — and the columns still add up,
// which is why ValidateCurriculum would let it pass.
func TestAYearHeaderIsNotASemesterHeader(t *testing.T) {
	for _, head := range [][4]string{
		{"1. Studienjahr", "2. Studienjahr", "3. Studienjahr", "4. Studienjahr"},
		{"1 Jahr", "2 Jahr", "3 Jahr", "4 Jahr"},
		{"1 LP", "2 LP", "3 LP", "4 LP"},
	} {
		l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, sitePlanPDF(head), ""))
		if err == nil && len(l.Cells) > 0 {
			t.Errorf("%q was read as a semester header: %+v", head[0], l.Cells)
		}
	}
}
