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

// writeTestPDFPages is writeTestPDF for a document of several pages.
func writeTestPDFPages(t *testing.T, pages ...string) string {
	t.Helper()
	objects := []string{"<< /Type /Catalog /Pages 2 0 R >>", "", testPDFFont}
	var kids []string
	for _, content := range pages {
		page := len(objects) + 1
		kids = append(kids, fmt.Sprintf("%d 0 R", page))
		objects = append(objects,
			fmt.Sprintf("<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 3 0 R >> >> /Contents %d 0 R >>", page+1),
			fmt.Sprintf("<< /Length %d >>\nstream\n%sendstream", len(content), content))
	}
	objects[1] = fmt.Sprintf("<< /Type /Pages /Kids [%s] /Count %d /MediaBox [0 0 500 500] >>", strings.Join(kids, " "), len(pages))
	return writePDFObjects(t, objects)
}

// pdfText prints one line of text; the strings are PDF strings in WinAnsi, so
// „ü" is written \374.
func pdfText(b *strings.Builder, x, y float64, text string) {
	fmt.Fprintf(b, "BT /F1 8 Tf %.0f %.0f Td (%s) Tj ET\n", x, y, text)
}

// boxPlan draws a box plan: a label column, one column per semester headed
// „n. Semester" from the first given, one row of module boxes and a „Summe LP"
// line. top is the upper edge of the header row.
func boxPlan(b *strings.Builder, top float64, first int, modules, sums []string) {
	box := func(x, y, w, h float64, text string) {
		fmt.Fprintf(b, "%.0f %.0f %.0f %.0f re S\n", x, y, w, h)
		if text != "" {
			pdfText(b, x+3, y+h-11, text)
		}
	}
	xs := []float64{130, 250, 370}
	box(10, top-30, 120, 30, "")
	box(10, top-70, 120, 40, "")
	box(10, top-100, 120, 30, "Summe LP")
	for i, x := range xs[:len(modules)] {
		box(x, top-30, xs[i+1]-x, 30, fmt.Sprintf("%d. Semester", first+i))
		box(x, top-70, xs[i+1]-x, 40, modules[i])
		box(x, top-100, xs[i+1]-x, 30, sums[i])
	}
}

// combinedIssue is an issue of the Amtliches Mitteilungsblatt as 17/2018 prints
// it: a cover listing the Bachelor's regulation from page 2 and the Master's
// from page 3, the Bachelor's box plan continued from its second semester in a
// second table, and the Master's own plan. The Bachelor's page carries the
// Master's footer, as two pages of 07/2014 do; masterFrom is the page number the
// Master's first page prints.
func combinedIssue(t *testing.T, masterFrom int) string {
	t.Helper()
	regulation := func(degree string) string {
		return "Fachspezifische Pr\\374fungs- und Studienordnung f\\374r den " + degree + "-Studiengang"
	}
	var cover strings.Builder
	pdfText(&cover, 40, 470, "Amtliches Mitteilungsblatt")
	pdfText(&cover, 200, 440, "I n h a l t")
	pdfText(&cover, 440, 425, "Seite")
	pdfText(&cover, 20, 410, "1.  "+regulation("Bachelor"))
	pdfText(&cover, 450, 410, "2")
	pdfText(&cover, 36, 398, "Materialchemie vom 17. September 2018")
	pdfText(&cover, 20, 380, "2.  "+regulation("Master"))
	pdfText(&cover, 450, 380, "3")
	pdfText(&cover, 36, 368, "Materialchemie vom 17. September 2018")
	pdfText(&cover, 20, 340, "Herausgeber:  BTU Cottbus-Senftenberg")

	var bachelor strings.Builder
	pdfText(&bachelor, 20, 485, "Seite 2  Amtliches Mitteilungsblatt")
	pdfText(&bachelor, 20, 470, "Anlage 3: Regelstudienplan")
	boxPlan(&bachelor, 460, 1, []string{"Physik \\(6 LP\\)", "Werkstoffe \\(6 LP\\)"}, []string{"6", "6"})
	boxPlan(&bachelor, 320, 3, []string{"Organische Chemie II \\(10 LP\\)", "Bachelor-Arbeit \\(12 LP\\)"}, []string{"10", "12"})
	pdfText(&bachelor, 20, 20, regulation("Master")+" Materialchemie")

	var master strings.Builder
	pdfText(&master, 20, 485, fmt.Sprintf("Seite %d  Amtliches Mitteilungsblatt", masterFrom))
	pdfText(&master, 20, 470, "Anlage 3: Regelstudienplan")
	boxPlan(&master, 460, 1, []string{"Analytik \\(6 LP\\)", "Master-Arbeit \\(30 LP\\)"}, []string{"6", "30"})
	pdfText(&master, 20, 20, regulation("Master")+" Materialchemie")
	return writeTestPDFPages(t, cover.String(), bachelor.String(), master.String())
}

// Materialchemie B.Sc. and M.Sc. 2018 stand in one issue, and each program was
// given both plans: the Bachelor a Master-Arbeit, the Master the Bachelor's
// first semesters. Each now reads the pages its own regulation stands on — the
// ones the cover names, whatever a footer says — and the Bachelor its whole
// box plan, whose second table begins at the third semester.
func TestACombinedIssueGivesEachProgramItsOwnRegulation(t *testing.T) {
	path := combinedIssue(t, 3)
	for _, tc := range []struct {
		hint   string
		rows   []string
		sems   [][]int
		totals map[int]float64
	}{
		{"Materialchemie / Bachelor (universitär) / PO 2018",
			[]string{"Physik", "Werkstoffe", "Organische Chemie II", "Bachelor-Arbeit"},
			[][]int{{1}, {2}, {3}, {4}}, map[int]float64{1: 6, 2: 6, 3: 10, 4: 12}},
		{"Materialchemie / Master (universitär) / PO 2018",
			[]string{"Analytik", "Master-Arbeit"},
			[][]int{{1}, {2}}, map[int]float64{1: 6, 2: 30}},
	} {
		t.Run(tc.hint, func(t *testing.T) {
			res, err := NewClient("", "").ExtractCurriculumOffline(context.Background(), path, tc.hint)
			if err != nil {
				t.Fatal(err)
			}
			var rows []string
			var sems [][]int
			for _, c := range res.Layout.Cells {
				rows, sems = append(rows, c.Row), append(sems, c.Semesters)
			}
			if !reflect.DeepEqual(rows, tc.rows) || !reflect.DeepEqual(sems, tc.sems) {
				t.Fatalf("read %v in %v, want %v in %v", rows, sems, tc.rows, tc.sems)
			}
			totals := map[int]float64{}
			for _, c := range res.Layout.Totals {
				totals[c.Semesters[0]] = c.Min
			}
			if !reflect.DeepEqual(totals, tc.totals) {
				t.Errorf("printed sums %v, want %v", totals, tc.totals)
			}
			if res.StandardPeriodSemesters != len(tc.sems) {
				t.Errorf("standard period %d, want %d", res.StandardPeriodSemesters, len(tc.sems))
			}
			if report := ValidateCurriculum(res, wholeCatalog(), "unknown", 6); !report.Valid {
				t.Errorf("the plan does not verify: %+v", report.Issues)
			}
			if len(res.Layout.Notes) != 1 || !strings.Contains(res.Layout.Notes[0], "several degrees") {
				t.Errorf("the pages left out are not named: %v", res.Layout.Notes)
			}
		})
	}
	// A program of neither degree has no plan in this document.
	_, err := ReadPDFLayoutForProgram(context.Background(), path, nil, "Materialchemie / Strukturiertes Promotionsstudium / PO 2018")
	if err == nil || !IsLayoutError(err) || !strings.Contains(err.Error(), "names none") {
		t.Errorf("a doctoral program was given a plan: %v", err)
	}
}

// The contents name printed page numbers. Where a page does not print the one
// the contents give it, which pages belong to whom is not known, and the
// document goes to review rather than to a program.
func TestAContentsPageTheDocumentContradictsIsRefused(t *testing.T) {
	_, err := ReadPDFLayoutForProgram(context.Background(), combinedIssue(t, 4), nil, "Materialchemie / Master (universitär) / PO 2018")
	if err == nil || !IsLayoutError(err) || !strings.Contains(err.Error(), "page 3 does not print") {
		t.Fatalf("got %v, want a refusal naming page 3", err)
	}
}

// A box plan whose columns begin at a later semester is the rest of a plan,
// never a plan of its own: where the table before it does not end at the
// semester before, the plan has a hole, and the document goes to review rather
// than storing a plan without its third semester.
func TestAContinuedBoxPlanNeedsItsBeginning(t *testing.T) {
	var b strings.Builder
	boxPlan(&b, 460, 1, []string{"Physik \\(6 LP\\)", "Werkstoffe \\(6 LP\\)"}, []string{"6", "6"})
	boxPlan(&b, 320, 4, []string{"Organische Chemie II \\(10 LP\\)", "Bachelor-Arbeit \\(12 LP\\)"}, []string{"10", "12"})
	_, err := ReadPDFLayout(context.Background(), writeTestPDF(t, b.String(), ""))
	if err == nil || !strings.Contains(err.Error(), "columns begin at semester 4, and no plan read before it ends at semester 3") {
		t.Fatalf("got %v, want the continuation refused", err)
	}
}

func TestTableOfContentsJoinsWhatTheLinesBreak(t *testing.T) {
	cover := strings.Join([]string{
		"Amtliches Mitteilungsblatt",
		"I n h a l t",
		"Seite",
		"1.  Berichtigung der fachspezifischen Prüfungs- und Studienordnung für den Ba-  2",
		"chelor-Studiengang Soziale Arbeit vom",
		"17. September 2020",
		"2.  Erste Änderungssatzung zur Prüfungs- und Studienordnung für den Master-Stu-  4",
		"diengang Soziale Arbeit vom 09. September 2021",
		"3.  Ordnung für den Bachelor- und Master-Studiengang X  6",
		"Herausgeber:  BTU",
	}, "\n")
	regs := tableOfContents(cover)
	want := []regulation{
		{title: "Berichtigung der fachspezifischen Prüfungs- und Studienordnung für den Bachelor-Studiengang Soziale Arbeit vom 17. September 2020", degrees: []string{"bachelor"}, from: 2},
		{title: "Erste Änderungssatzung zur Prüfungs- und Studienordnung für den Master-Studiengang Soziale Arbeit vom 09. September 2021", degrees: []string{"master"}, from: 4},
		{title: "Ordnung für den Bachelor- und Master-Studiengang X", degrees: []string{"bachelor", "master"}, from: 6},
	}
	if !reflect.DeepEqual(regs, want) {
		t.Fatalf("got %#v\nwant %#v", regs, want)
	}
}

// The corpus documents that print two regulations. Before, each program was
// given the plans of both, and Bauingenieurwesen M.Sc. 2014 the three plans
// of the Bachelor alone.
func TestCombinedIssuesInTheCorpus(t *testing.T) {
	root := os.Getenv("RADIX_PDF_TEST_DIR")
	if root == "" {
		t.Skip("set RADIX_PDF_TEST_DIR")
	}
	for _, tc := range []struct {
		file, hint string
		cells      int
		pages      []int
		credits    float64
	}{
		{"Materialchemie/4615_AMbl-17_2018-PSO_Materialchemie_BA_MA.pdf", "Materialchemie / Bachelor (universitär) / PO 2018", 24, []int{5}, 180},
		{"Materialchemie/4615_AMbl-17_2018-PSO_Materialchemie_BA_MA.pdf", "Materialchemie / Master (universitär) / PO 2018", 10, []int{9}, 120},
		{"Künstliche_Intelligenz/6011_AMbl-13_2022_KI_BA_AI_MA.pdf", "Künstliche Intelligenz / Bachelor (universitär) / PO 2022", 21, []int{7}, 180},
		{"Künstliche_Intelligenz_Technologie/6012_AMbl-14_2022_KIT_BA-MA.pdf", "Künstliche Intelligenz Technologie / Bachelor (universitär) / PO 2022", 23, []int{8}, 180},
		// Its own plan, a box plan on page 13, is one the reader cannot read yet.
		{"Bauingenieurwesen/3159_AMbl_08_10_14.pdf", "Bauingenieurwesen / Master (universitär) / PO 2014", 0, nil, 0},
	} {
		t.Run(tc.hint, func(t *testing.T) {
			l, err := ReadPDFLayoutForProgram(context.Background(), filepath.Join(root, tc.file), nil, tc.hint)
			if tc.cells == 0 {
				if err == nil {
					t.Fatalf("read %d cells of another regulation", len(l.Cells))
				}
				if !strings.Contains(err.Error(), "pages 9–15") {
					t.Errorf("the refusal does not say which pages were read: %v", err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			pages, credits := map[int]bool{}, 0.0
			for _, c := range l.Cells {
				pages[c.Page] = true
				credits += c.Min
			}
			want := map[int]bool{}
			for _, p := range tc.pages {
				want[p] = true
			}
			if len(l.Cells) != tc.cells || !reflect.DeepEqual(pages, want) || credits != tc.credits {
				t.Fatalf("read %d cells on pages %v worth %v LP, want %d on %v worth %v", len(l.Cells), pages, credits, tc.cells, tc.pages, tc.credits)
			}
		})
	}
}
