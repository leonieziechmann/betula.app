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

// annexPlanPDF draws the shape of the Bauingenieurwesen Lesefassung: Anlage 2.1
// prints a credit in every cell, and Anlage 3.1 below it lists the same rows but
// replaces the first two semesters with one merged cell pointing back at it. ref
// supplies that cell's text, so a reference the geometry contradicts can be
// drawn too.
func annexPlanPDF(ref string) string {
	var b strings.Builder
	xs := []float64{40, 190, 310, 430, 500}
	draw := func(top float64, title string, rows [][]string, merged bool) {
		fmt.Fprintf(&b, "BT /F1 9 Tf 1 0 0 1 40 %.1f Tm (%s) Tj ET\n", top+8, title)
		for ri, row := range rows {
			for ci, txt := range row {
				if merged && ri > 0 && ci >= 1 && ci <= 2 {
					continue
				}
				x, y, w, h := xs[ci], top-float64(ri+1)*22, xs[ci+1]-xs[ci], 22.0
				fmt.Fprintf(&b, "%.1f %.1f %.1f %.1f re S\n", x, y, w, h)
				if txt != "" {
					fmt.Fprintf(&b, "BT /F1 7 Tf 1 0 0 1 %.1f %.1f Tm (%s) Tj ET\n", x+2, y+h-13, txt)
				}
			}
		}
		if merged {
			// one cell over the first two semester columns of every module row,
			// carrying the reference
			h := float64(len(rows)-2) * 22
			y := top - float64(len(rows)-1)*22
			fmt.Fprintf(&b, "%.1f %.1f %.1f %.1f re S\n", xs[1], y, xs[3]-xs[1], h)
			fmt.Fprintf(&b, "BT /F1 6 Tf 1 0 0 1 %.1f %.1f Tm (%s) Tj ET\n", xs[1]+2, y+h-13, ref)
		}
	}
	draw(600, "Anlage 2.1 Regelstudienplan", [][]string{
		{"Modul", "1. Sem", "2. Sem", "3. Sem"},
		{"Hoehere Mathematik", "10", "", ""},
		{"Baustatik", "", "10", ""},
		{"Wasserbau", "", "", "10"},
		{"Summe", "10", "10", "10"},
	}, false)
	draw(420, "Anlage 3.1 Regelstudienplan dual", [][]string{
		{"Modul", "1. Sem", "2. Sem", "3. Sem"},
		{"Hoehere Mathematik", "", "", ""},
		{"Baustatik", "", "", ""},
		{"Wasserbau", "", "", "10"},
		{"Summe", "10", "10", "10"},
	}, true)
	return b.String()
}

// A cross reference is only followed where its text and the cell's own geometry
// say the same thing. The span the text names has to be exactly the columns the
// merged cell covers — otherwise the reader has understood neither, and the plan
// keeps its gap rather than inventing credits for it.
func TestAnAnnexReferenceMustAgreeWithTheCellItStandsIn(t *testing.T) {
	for _, c := range []struct {
		text      string
		semesters []int
		ok        bool
		from, to  int
		source    int
		annex     string
	}{
		{"1. bis 5. Fachsemester analog zu Anlage 2.1", []int{1, 2, 3, 4, 5}, true, 1, 5, 1, "2.1"},
		{"8. Fachsemester analog zu 6. Fachsemester gemäß Anlage 2.1", []int{8}, true, 8, 8, 6, "2.1"},
		{"3. Fachsemester analog zu Anlage 4", []int{3}, true, 3, 3, 3, "4"},
		// the text names more semesters than the cell covers, or fewer
		{"1. bis 5. Fachsemester analog zu Anlage 2.1", []int{1, 2, 3}, false, 0, 0, 0, ""},
		{"1. bis 2. Fachsemester analog zu Anlage 2.1", []int{3, 4}, false, 0, 0, 0, ""},
		{"2. Fachsemester analog zu Anlage 2.1", []int{1, 2}, false, 0, 0, 0, ""},
		// not a reference at all
		{"Höhere Mathematik T1", []int{1}, false, 0, 0, 0, ""},
		{"1. bis 5. Fachsemester siehe Anlage 2.1", []int{1, 2, 3, 4, 5}, false, 0, 0, 0, ""},
	} {
		ref, ok := parseAnnexReference(c.text, c.semesters)
		if ok != c.ok {
			t.Errorf("%q over %v: read=%t, want %t", c.text, c.semesters, ok, c.ok)
			continue
		}
		if !ok {
			continue
		}
		if ref.from != c.from || ref.to != c.to || ref.sourceFrom != c.source || ref.annex != c.annex {
			t.Errorf("%q: got %d-%d from Anlage %s semester %d, want %d-%d from %s semester %d",
				c.text, ref.from, ref.to, ref.annex, ref.sourceFrom, c.from, c.to, c.annex, c.source)
		}
	}
}

// A reference the plan's own geometry contradicts is not followed: the cell
// spans two semester columns, so a text naming three of them is not understood
// and the plan keeps its gap rather than inventing credits.
func TestAnAnnexReferenceTheGeometryContradictsIsRefused(t *testing.T) {
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, annexPlanPDF("1. bis 3. Fachsemester analog zu Anlage 2.1"), ""))
	if err != nil {
		return // refusing the whole table is a correct outcome too
	}
	for _, c := range l.Cells {
		if c.Table == "p1t2" && len(c.Semesters) == 1 && c.Semesters[0] <= 2 && c.Min > 0 {
			t.Errorf("credits invented from a contradicted reference: %+v", c)
		}
	}
}

// „entweder" opens a choice the same way „oder" separates one. Both are read
// only from a line that carries nothing else: a row naming a module as well is
// a requirement, not a choice marker, and reading it as one would drop every
// row of the branch it appears to open.
func TestAChoiceMarkerIsALineThatCarriesNothingElse(t *testing.T) {
	row := func(cells ...string) []*string {
		out := make([]*string, len(cells))
		for i := range cells {
			c := cells[i]
			out[i] = &c
		}
		return out
	}
	for _, c := range []struct {
		cells          []string
		entweder, oder bool
	}{
		{[]string{"entweder", "", ""}, true, false},
		{[]string{"", "Entweder", ""}, true, false},
		{[]string{"oder", "", ""}, false, true},
		{[]string{"", "", "or"}, false, true},
		// a line that says more than the marker is not a marker
		{[]string{"entweder", "6", ""}, false, false},
		{[]string{"entweder Modul A", "", ""}, false, false},
		{[]string{"oder Wahlpflicht", "", ""}, false, false},
		{[]string{"", "", ""}, false, false},
		{[]string{"Bachelor-Arbeit", "", "12"}, false, false},
	} {
		r := row(c.cells...)
		if got := isEntwederRow(r); got != c.entweder {
			t.Errorf("%v: isEntwederRow=%t, want %t", c.cells, got, c.entweder)
		}
		if got := isOderRow(r); got != c.oder {
			t.Errorf("%v: isOderRow=%t, want %t", c.cells, got, c.oder)
		}
	}
}
