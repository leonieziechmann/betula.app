package gemini

import (
	"context"
	"fmt"
	"strings"
	"testing"
)

// choicePlanPDF prints the shape of Kultur und Technik 2017, Anlage 3: a ruled
// matrix whose specialization section marks a footnote, with both tracks below
// it one under the other. blockB is the second track's credit row, so a test
// can make the two tracks differ.
func choicePlanPDF(footnote string, blockB []string) string {
	var b strings.Builder
	xs := []float64{40, 250, 310, 370, 430}
	rows := [][]string{
		{"Komplex bzw. Modul", "1.Sem.", "2.Sem.", "LP"},
		{"Mathematik", "6", "", "6"},
		{"Schwerpunktbereich*", "", "", ""},
		{"Schwerpunkt Alpha", "", "", ""},
		{"Module aus dem Schwerpunkt", "", "12", "12"},
		{"Schwerpunkt Beta", "", "", ""},
		blockB,
		{"Abschlussarbeit", "", "18", "18"},
		{"Summe der Leistungspunkte", "6", "30", "36"},
	}
	for ri, row := range rows {
		for ci, txt := range row {
			x, y, w, h := xs[ci], 460-float64(ri+1)*24, xs[ci+1]-xs[ci], 24.0
			fmt.Fprintf(&b, "%.1f %.1f %.1f %.1f re S\n", x, y, w, h)
			if txt != "" {
				fmt.Fprintf(&b, "BT /F1 8 Tf 1 0 0 1 %.1f %.1f Tm (%s) Tj ET\n", x+4, y+h-16, txt)
			}
		}
	}
	// The footnote sits under the table, as the regulation prints it.
	fmt.Fprintf(&b, "BT /F1 8 Tf 1 0 0 1 40 200 Tm (%s) Tj ET\n", footnote)
	return b.String()
}

const belegen = "* siehe \\247 6 (3) - Ein Schwerpunkt ist zu belegen."

// Both tracks are printed, both are stored, and the plan's own semester sums
// count one of them -- which is what the footnote says.
func TestChoiceFootnoteCountsOneTrack(t *testing.T) {
	same := []string{"Module aus dem Schwerpunkt", "", "12", "12"}
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, choicePlanPDF(belegen, same), ""))
	if err != nil {
		t.Fatal(err)
	}
	tracks := 0
	for _, c := range l.Cells {
		if strings.Contains(c.Row, "Schwerpunkt") {
			tracks++
			if c.AltGroup == 0 {
				t.Errorf("track cell %s is not an alternative: %+v", c.ID, c)
			}
		}
	}
	if tracks != 2 {
		t.Fatalf("both tracks must stay in the plan, got %d", tracks)
	}
	counted := 0.0
	for _, c := range effectiveCells(l) {
		counted += c.Min
	}
	if counted != 36 {
		t.Fatalf("the plan's own sum is 36 LP; its counted rows give %g", counted)
	}
}

// Tracks that carry different credits do not say how the choice moves them.
// Nothing is marked, and the document reads exactly as it did before.
func TestChoiceFootnoteRefusesUnequalTracks(t *testing.T) {
	other := []string{"Module aus dem Schwerpunkt", "6", "6", "12"}
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, choicePlanPDF(belegen, other), ""))
	if err != nil {
		t.Fatal(err)
	}
	for _, c := range l.Cells {
		if c.AltGroup != 0 {
			t.Fatalf("unequal tracks must not be marked as alternatives: %+v", c)
		}
	}
	if len(l.Notes) == 0 {
		t.Error("the refusal must be reported")
	}
}

// Without the footnote the same layout is two required sections, and every row
// keeps counting -- the structure alone never means a choice.
func TestChoiceNeedsTheFootnote(t *testing.T) {
	same := []string{"Module aus dem Schwerpunkt", "", "12", "12"}
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, choicePlanPDF("* Das Modul wird nur im Wintersemester angeboten.", same), ""))
	if err != nil {
		t.Fatal(err)
	}
	for _, c := range l.Cells {
		if c.AltGroup != 0 {
			t.Fatalf("no choice is stated, so nothing is an alternative: %+v", c)
		}
	}
}
