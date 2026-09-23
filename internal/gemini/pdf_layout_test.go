package gemini

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/ledongthuc/pdf"
)

// Generate a real, self-contained PDF with Go so the default suite exercises
// decoding, graphics transforms and table reconstruction without external tools.
func writeTestPDF(t *testing.T, content, resources string, extraObjects ...string) string {
	t.Helper()
	if !strings.Contains(resources, "/Font") {
		resources = "/Font << /F1 4 0 R >> " + resources
	}
	objects := []string{
		"<< /Type /Catalog /Pages 2 0 R >>",
		"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 500 500] >>",
		"<< /Type /Page /Parent 2 0 R /Resources << " + resources + " >> /Contents 5 0 R >>",
		testPDFFont,
		fmt.Sprintf("<< /Length %d >>\nstream\n%sendstream", len(content), content),
	}
	return writePDFObjects(t, append(objects, extraObjects...))
}

var testPDFFont = "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding /FirstChar 0 /LastChar 255 /Widths [" + strings.Repeat("500 ", 256) + "] >>"

// writePDFObjects writes objects 1, 2, … with their cross-reference table.
func writePDFObjects(t *testing.T, objects []string) string {
	t.Helper()
	var b strings.Builder
	b.WriteString("%PDF-1.4\n")
	offsets := []int{0}
	for i, o := range objects {
		offsets = append(offsets, b.Len())
		fmt.Fprintf(&b, "%d 0 obj\n%s\nendobj\n", i+1, o)
	}
	start := b.Len()
	fmt.Fprintf(&b, "xref\n0 %d\n0000000000 65535 f \n", len(offsets))
	for _, off := range offsets[1:] {
		fmt.Fprintf(&b, "%010d 00000 n \n", off)
	}
	fmt.Fprintf(&b, "trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n", len(offsets), start)
	path := filepath.Join(t.TempDir(), "plan.pdf")
	if err := os.WriteFile(path, []byte(b.String()), 0600); err != nil {
		t.Fatal(err)
	}
	return path
}

func studyPlanPDFContent(total string) string {
	var b strings.Builder
	// Translation exercises CTM handling. Coordinates below are local points.
	b.WriteString("q 1 0 0 1 5 -7 cm\nBT /F1 8 Tf 40 460 Td (Studium kann nur im Wintersemester begonnen werden.) Tj ET\n")
	xs := []float64{40, 250, 310, 370, 430}
	rows := [][]string{{"Semester LP", "1", "2", "Summe LP"}, {"Alpha", "6", "", "6"}, {"Beta", "", "6", "6"}, {"Optional", "4-8", "", "4-8"}, {"Choice A", "6", "", "6"}, {"Choice B", "", "", ""}, {total, "12", "6", "18"}}
	for ri, row := range rows {
		for ci, txt := range row {
			if (ri == 3 && ci == 2) || (ri == 5 && ci == 1) {
				continue
			}
			x, y, w, h := xs[ci], 420-float64(ri+1)*24, xs[ci+1]-xs[ci], 24.0
			if ri == 3 && ci == 1 {
				w = 120
			}
			if ri == 4 && ci == 1 {
				h = 48
				y -= 24
			}
			fmt.Fprintf(&b, "%.1f %.1f %.1f %.1f re S\n", x, y, w, h)
			if txt != "" {
				fmt.Fprintf(&b, "BT /F1 8 Tf 1 0 0 1 %.1f %.1f Tm ", x+4, y+h-16)
				if txt == "Alpha" {
					b.WriteString("[(Al)] TJ [(pha)] TJ")
				} else {
					fmt.Fprintf(&b, "(%s) Tj", txt)
				}
				b.WriteString(" ET\n")
			}
		}
	}
	b.WriteString("Q\n")
	return b.String()
}

func TestGoPDFLayoutMergedCells(t *testing.T) {
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, studyPlanPDFContent("Summe"), ""))
	if err != nil {
		t.Fatal(err)
	}
	if l.StartTerm != "winter" || len(l.Cells) != 4 || len(l.Totals) != 2 {
		t.Fatalf("unexpected layout: %+v", l)
	}
	want := []SourceCell{
		{ID: "p1t1r2c2", Table: "p1t1", Page: 1, RowIndex: 2, Row: "Alpha", Semesters: []int{1}, Raw: "6", Min: 6, Max: 6},
		{ID: "p1t1r3c3", Table: "p1t1", Page: 1, RowIndex: 3, Row: "Beta", Semesters: []int{2}, Raw: "6", Min: 6, Max: 6},
		{ID: "p1t1r4c2", Table: "p1t1", Page: 1, RowIndex: 4, Row: "Optional", Semesters: []int{1, 2}, Raw: "4-8", Min: 4, Max: 8},
		{ID: "p1t1r5c2", Table: "p1t1", Page: 1, RowIndex: 5, Row: "Choice A / Choice B", Semesters: []int{1}, Raw: "6", Min: 6, Max: 6, SharedRows: true},
	}
	for i, g := range l.Cells {
		if len(g.BBox) != 4 {
			t.Fatal("missing coordinates")
		}
		g.BBox = nil
		if !reflect.DeepEqual(g, want[i]) {
			t.Errorf("got %+v want %+v", g, want[i])
		}
	}
}

func TestGoPDFRejectsUnverifiedLayouts(t *testing.T) {
	for _, tt := range []struct {
		name, content, resources, expected string
		extra                              []string
	}{
		{"missing totals", studyPlanPDFContent("Other"), "", "no whole-plan total", nil},
		{"invalid credits", strings.Replace(studyPlanPDFContent("Summe"), "(4-8)", "(8-4)", 1), "", "unsupported semester cell", nil},
		{"clipping only", "40 200 300 200 re W n\n", "", "no supported ruled", nil},
		{"empty form XObject", "/X1 Do\n", "/XObject << /X1 6 0 R >>", "no supported ruled", []string{"<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /Length 0 >>\nstream\nendstream"}},
		{"bad graphics state", "Q\n", "", "unbalanced graphics state", nil},
	} {
		t.Run(tt.name, func(t *testing.T) {
			_, err := ReadPDFLayout(context.Background(), writeTestPDF(t, tt.content, tt.resources, tt.extra...))
			if err == nil || !strings.Contains(err.Error(), tt.expected) {
				t.Fatalf("got %v, want %s", err, tt.expected)
			}
		})
	}
}

func TestGoPDFCancellationAndMalformedInput(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := ReadPDFLayout(ctx, "unused.pdf"); err != context.Canceled {
		t.Fatalf("got %v", err)
	}
	path := filepath.Join(t.TempDir(), "broken.pdf")
	if err := os.WriteFile(path, []byte("not a PDF"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := ReadPDFLayout(context.Background(), path); err == nil {
		t.Fatal("malformed PDF accepted")
	}
}

func TestGoPDFCharacterPositions(t *testing.T) {
	// CID widths differ deliberately. Decoding UTF-8 and then indexing raw bytes
	// would put the second character at the wrong horizontal coordinate.
	cmap := "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Test def /CMapType 2 def\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n2 beginbfchar <0001> <0041> <0002> <0042> endbfchar\nendcmap CMapName currentdict /CMap defineresource pop end end\n"
	path := writeTestPDF(t,
		"BT /F1 10 Tf 1 0 0 1 40 400 Tm 2 Tw 200 Tz (A B) Tj ET\nBT /F2 10 Tf 100 Tz 0 Tw 1 0 0 1 100 350 Tm <00010002> Tj ET\n",
		"/Font << /F1 4 0 R /F2 6 0 R >>",
		"<< /Type /Font /Subtype /Type0 /BaseFont /Test /Encoding /Identity-H /DescendantFonts [7 0 R] /ToUnicode 8 0 R >>",
		"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /DW 1000 /W [1 [400 600]] >>",
		fmt.Sprintf("<< /Length %d >>\nstream\n%sendstream", len(cmap), cmap))
	f, r, err := pdf.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	g, err := readPageGeometry(context.Background(), r.Page(1))
	if err != nil {
		t.Fatal(err)
	}
	if len(g.glyphs) != 5 {
		t.Fatalf("unexpected glyphs %+v", g.glyphs)
	}
	for i, want := range []struct {
		x, width float64
		text     string
	}{{40, 10, "A"}, {50, 10, " "}, {64, 10, "B"}, {100, 4, "A"}, {104, 6, "B"}} {
		got := g.glyphs[i]
		if got.x != want.x || got.width != want.width || got.text != want.text {
			t.Errorf("glyph %d got %+v want %+v", i, got, want)
		}
	}
}

func TestEffectiveCellsUseGreySamplePlan(t *testing.T) {
	l := &PDFLayout{Cells: []SourceCell{
		{ID: "a", Table: "t", Semesters: []int{1}, Min: 9, Max: 9, InPlan: true},
		{ID: "b", Table: "t", Semesters: []int{1}, Min: 6, Max: 6},
		{ID: "c", Table: "t", Semesters: []int{5, 6}, Min: 6, Max: 6, Optional: true, InPlan: true, PlanSemester: 6},
		{ID: "d", Table: "other", Semesters: []int{1}, Min: 3, Max: 3},
	}}
	got := effectiveCells(l)
	if len(got) != 3 || got[0].ID != "a" || got[1].ID != "c" || len(got[1].Semesters) != 1 || got[1].Semesters[0] != 6 || got[2].ID != "d" {
		t.Fatalf("effective cells: %+v", got)
	}
}

func TestApplyStyleLegends(t *testing.T) {
	l := &PDFLayout{Cells: []SourceCell{
		{ID: "a", Table: "t", Bold: true, Shaded: true},
		{ID: "b", Table: "t"},
		{ID: "c", Table: "u"},
	}}
	applyStyleLegends(l, "fett geschriebene lp-zahlen sind pflichtmodule; grau ange- + die entwurfsmodule sind legte zellen stellen einen möglichen studienplan dar")
	if l.Cells[0].Elective || !l.Cells[0].InPlan || !l.Cells[1].Elective || l.Cells[1].InPlan || l.Cells[2].Elective || l.Cells[2].InPlan {
		t.Fatalf("legend styles: %+v", l.Cells)
	}
	plain := &PDFLayout{Cells: []SourceCell{{ID: "a", Table: "t", Bold: true}, {ID: "b", Table: "t"}}}
	applyStyleLegends(plain, "keine legende")
	if plain.Cells[1].Elective {
		t.Fatal("bold text without a legend must not change module types")
	}
}
