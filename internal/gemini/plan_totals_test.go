package gemini

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"
)

// cell and total build the layout of a plan the way the readers do.
func cell(id string, row int, label string, semesters []int, lo, hi float64) SourceCell {
	return SourceCell{ID: id, Table: "t", Page: 1, RowIndex: row, Row: label, Semesters: semesters, Raw: fmt.Sprintf("%v", lo), Min: lo, Max: hi}
}

func total(id string, row int, label string, semesters []int, value float64) SourceCell {
	c := cell(id, row, label, semesters, value, value)
	return c
}

// The plan of Informatik 2008, shortened to what makes it hard: sections with
// their own sums, a grand total over all of them, and three elective budgets in
// the last two semesters that only a printed sum pins down.
func informatikLayout() *PDFLayout {
	return &PDFLayout{
		Cells: []SourceCell{
			cell("c1", 2, "Entwicklung von Softwaresystemen", []int{1}, 8, 8),
			cell("c2", 3, "Programmierpraktikum", []int{1}, 4, 4),
			cell("c3", 5, "Mathematik IT-1", []int{1}, 8, 8),
			cell("c4", 8, "Komplex Grundlagen der Informatik", []int{5, 6}, 10, 24),
			cell("c5", 9, "Komplex Praktische Informatik", []int{5, 6}, 10, 24),
			cell("c6", 10, "Komplex Angewandte und Technische Informatik", []int{5, 6}, 10, 24),
			cell("c7", 12, "Seminar oder Praktikum", []int{5, 6}, 4, 4),
			cell("c8", 13, "Bachelor-Arbeit", []int{5, 6}, 12, 12),
		},
		Totals: []SourceCell{
			total("t1", 4, "Summe Komplex Informatik", []int{1}, 12),
			total("t2", 6, "Summe Komplex Mathematik", []int{1}, 8),
			total("t3", 7, "Summe Grundstudium", []int{1}, 20),
			total("t4", 11, "Summe Komplexe des Fachstudiums", []int{5, 6}, 44),
			total("t5", 14, "Summe Studium", []int{1}, 20),
			total("t6", 14, "Summe Studium", []int{5, 6}, 60),
		},
		PlanNames: map[string]string{"t": "Regelstudienplan"},
	}
}

func TestDerivePlanTotalsBindsEachSumToItsOwnRows(t *testing.T) {
	got := map[string][]string{}
	credits := map[string]float64{}
	whole := map[string]bool{}
	for _, g := range DerivePlanTotals(informatikLayout()) {
		key := fmt.Sprintf("%s %d-%d", g.Label, g.Start, g.End)
		got[key] = g.Members
		credits[key] = g.Credits
		whole[key] = g.WholePlan
	}
	want := map[string][]string{
		"Summe Komplex Informatik 1-1":        {"c1", "c2"},
		"Summe Komplex Mathematik 1-1":        {"c3"},
		"Summe Grundstudium 1-1":              {"c1", "c2", "c3"},
		"Summe Komplexe des Fachstudiums 5-6": {"c4", "c5", "c6"},
		"Summe Studium 1-1":                   {"c1", "c2", "c3"},
		"Summe Studium 5-6":                   {"c4", "c5", "c6", "c7", "c8"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("bound rows\n got %v\nwant %v", got, want)
	}
	// A section sum is not the plan's own total, however much it adds up to.
	if whole["Summe Grundstudium 1-1"] || !whole["Summe Studium 5-6"] {
		t.Errorf("scope of the sums: %v", whole)
	}
	// The three budgets are 30 to 72 LP on their own; the plan says 44.
	for _, g := range DerivePlanTotals(informatikLayout()) {
		if g.Label != "Summe Komplexe des Fachstudiums" {
			continue
		}
		if g.Credits != 44 || g.Min != 30 || g.Max != 72 || !g.Choice {
			t.Fatalf("elective group: %+v", g)
		}
	}
}

// The rows of a plan with budgets are a lower bound; its own lines are the plan.
func TestThePlansOwnLinesSayMoreThanItsRows(t *testing.T) {
	layout := informatikLayout()
	rows := 0.0
	for _, c := range layout.Cells {
		rows += c.Min
	}
	stated := 0.0
	for _, g := range DerivePlanTotals(layout) {
		if g.WholePlan {
			stated += g.Credits
		}
	}
	if stated != 80 || rows != 66 {
		t.Fatalf("the plan states %v LP, its rows give %v (want 80 and 66)", stated, rows)
	}
}

// A sum nothing explains is dropped rather than bound to whatever is above it.
func TestDerivePlanTotalsDropsASumItsRowsCannotReach(t *testing.T) {
	layout := &PDFLayout{
		Cells:  []SourceCell{cell("c1", 2, "Alpha", []int{1}, 6, 6)},
		Totals: []SourceCell{total("t1", 3, "Summe Studium", []int{1}, 30)},
	}
	if got := DerivePlanTotals(layout); len(got) != 0 {
		t.Fatalf("kept an unexplained sum: %+v", got)
	}
}

// What a row carries about the sum it belongs to is the narrowest one.
func TestAreaRulesNameTheNarrowestSum(t *testing.T) {
	layout := informatikLayout()
	res := &CurriculumExtractionResult{}
	for _, c := range layout.Cells {
		res.Modules = append(res.Modules, ExtractedModule{SourceCell: c.ID, ModuleName: c.Row})
	}
	if err := BindSourceCells(res, layout); err != nil {
		t.Fatal(err)
	}
	for _, m := range res.Modules {
		switch m.SourceCell {
		case "c4":
			if !strings.Contains(m.AreaRules, "Summe Komplexe des Fachstudiums: 3 requirements together 44 LP in semesters 5-6; their own ranges allow 30–72 LP.") {
				t.Errorf("elective row says %q", m.AreaRules)
			}
		case "c1":
			if m.AreaRules != "" {
				t.Errorf("a row with one semester and one value needs no constraint, got %q", m.AreaRules)
			}
		}
	}
	if len(res.Totals) == 0 {
		t.Error("the extraction result does not carry the plan's sums")
	}
}

// A plan printed as one panel per semester column, with a phase caption over the
// columns and a bare "Leistungspunkte" line instead of a "Summe" line: the
// Physics master. Neither the caption nor that line is a module.
func TestBoxPlanReadsCaptionsAndACreditLine(t *testing.T) {
	var b strings.Builder
	xs := []float64{130, 250, 370, 490}
	box := func(x, y, w, h float64, text string) {
		fmt.Fprintf(&b, "%.0f %.0f %.0f %.0f re S\n", x, y, w, h)
		if text != "" {
			fmt.Fprintf(&b, "BT /F1 8 Tf %.0f %.0f Td (%s) Tj ET\n", x+3, y+h-11, text)
		}
	}
	// The label column, then one column per semester.
	box(10, 400, 120, 30, "")
	for i, x := range xs[:3] {
		box(x, 400, xs[i+1]-x, 30, fmt.Sprintf("%d. Semester", i+1))
	}
	// A caption over the first two semesters and one over the third.
	box(10, 370, 120, 30, "")
	box(xs[0], 370, xs[2]-xs[0], 30, "Specialization Phase")
	box(xs[2], 370, xs[3]-xs[2], 30, "Research Phase")
	box(10, 330, 120, 40, "")
	for i, x := range xs[:3] {
		box(x, 330, xs[i+1]-x, 40, fmt.Sprintf("Module %d \\(30 LP\\)", i+1))
	}
	// The line of credits, named after the column and not after a sum.
	box(10, 300, 120, 30, "Leistungspunkte")
	for i, x := range xs[:3] {
		box(x, 300, xs[i+1]-x, 30, "30")
	}

	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, b.String(), ""))
	if err != nil {
		t.Fatal(err)
	}
	if len(l.Cells) != 3 || len(l.Totals) != 3 || len(l.Issues) != 0 {
		t.Fatalf("cells %d totals %d issues %v", len(l.Cells), len(l.Totals), l.Issues)
	}
	sections := map[string]string{}
	for _, c := range l.Cells {
		sections[c.Row] = c.Section
	}
	want := map[string]string{"Module 1": "Specialization Phase", "Module 2": "Specialization Phase", "Module 3": "Research Phase"}
	if !reflect.DeepEqual(sections, want) {
		t.Fatalf("sections %v want %v", sections, want)
	}
}

// A box whose credits stand in an appendix („Entwurfsprojekt 1 (Gemäß Anlage 1,
// Nr. 1)", Architektur M.Sc.) is a requirement, not a caption: a row of them must
// not disappear as a section label, whether or not the appendix was read.
func TestBoxPlanKeepsARowThatPointsAtItsCredits(t *testing.T) {
	var b strings.Builder
	xs := []float64{130, 310, 490}
	box := func(x, y, w, h float64, text string) {
		fmt.Fprintf(&b, "%.0f %.0f %.0f %.0f re S\n", x, y, w, h)
		if text != "" {
			fmt.Fprintf(&b, "BT /F1 8 Tf %.0f %.0f Td (%s) Tj ET\n", x+3, y+h-11, text)
		}
	}
	box(10, 400, 120, 30, "")
	for i, x := range xs[:2] {
		box(x, 400, xs[i+1]-x, 30, fmt.Sprintf("%d. Semester", i+1))
	}
	// A row whose credits stand in an appendix this document does not carry.
	box(10, 360, 120, 40, "")
	for i, x := range xs[:2] {
		box(x, 360, xs[i+1]-x, 40, fmt.Sprintf("P%d \\(Gem\\344\\337 Anlage 1, Nr. %d\\)", i+1, i+1))
	}
	// A row that prints its own credits, so the table is read at all.
	box(10, 320, 120, 40, "")
	for i, x := range xs[:2] {
		box(x, 320, xs[i+1]-x, 40, fmt.Sprintf("Vertiefung %d \\(6 LP\\)", i+1))
	}
	box(10, 290, 120, 30, "Leistungspunkte")
	for i, x := range xs[:2] {
		box(x, 290, xs[i+1]-x, 30, "30")
	}
	l, err := ReadPDFLayout(context.Background(), writeTestPDF(t, b.String(), ""))
	// The rows stay, as something to review; none of them becomes a section label.
	if err == nil || !strings.Contains(err.Error(), "unsupported module box P1 (Gemäß Anlage 1, Nr. 1)") {
		t.Fatalf("a row pointing at its credits was not kept as a requirement: %v (%+v)", err, l)
	}
}

// The regulation of Elektrotechnik 2022 heads one table with „im dualen
// praxisintegrierenden und im dualen ausbildungsintegrierenden Studium" and
// tells the two apart inside it. Reading the heading as the first of the two
// named modes threw away the plan of the other.
func TestDualModeOfATitleThatNamesBothVariants(t *testing.T) {
	both := "Anlage b.2: Regelstudienplan für die Studienrichtungen PA, IoT und EET im dualen praxisintegrierenden und im dualen ausbildungsintegrierenden Studium"
	if got := dualMode(both); got != "Dual" {
		t.Errorf("dualMode(both) = %q, want %q", got, "Dual")
	}
	if got := dualModeOf("Dual · Dual ausbildungsintegrierend"); got != "Dual ausbildungsintegrierend" {
		t.Errorf("dualModeOf = %q", got)
	}
	l := &PDFLayout{
		Cells: []SourceCell{
			{ID: "a", Table: "practice", Semesters: []int{1}, Min: 6, Max: 6},
			{ID: "b", Table: "training", Semesters: []int{1}, Min: 6, Max: 6},
			{ID: "c", Table: "regular", Semesters: []int{1}, Min: 6, Max: 6},
		},
		PlanNames: map[string]string{
			"practice": "Dual · Dual praxisintegrierend",
			"training": "Dual · Dual ausbildungsintegrierend",
			"regular":  "Regelstudienplan im grundständigen Studium",
		},
	}
	selectProgramMode(l, strings.ToLower("Elektrotechnik - dual / Bachelor (universitär) - Duales Studium, ausbildungsintegrierend / PO 2022"))
	if len(l.Cells) != 1 || l.Cells[0].Table != "training" {
		t.Fatalf("kept %d tables: %v", len(l.PlanNames), l.PlanNames)
	}
}
