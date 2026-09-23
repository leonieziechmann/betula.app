package gemini

import (
	"context"
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/ledongthuc/pdf"
)

// TestProbe shows what the reader makes of one regulation PDF. It is the tool for
// the question „why does this program have no plan?" or „why does this plan not
// state what it adds up to?": it prints the page text, the ruled tables the
// geometry reader finds, every cell and sum the layout reader took from them, and
// the sums plan_totals.go bound to their rows.
//
//	RADIX_PROBE_PDF=statutes/Informatik/6707_12_Informatik_B.Sc.pdf \
//	RADIX_PROBE_HINT="Informatik / Bachelor (universitär) / PO 2008" \
//	go test ./internal/gemini -run TestProbe -v
//
// RADIX_PROBE_PAGES=7,9 narrows it to those pages; without it, every page with a
// table or an „Anlage" is shown. The hint is what curriculumscan passes as the
// program hint, and it decides which plan variant of a document is selected — a
// document that prints a regular and a dual plan reads differently without it.
//
// Skipped without RADIX_PROBE_PDF, so it costs the normal suite nothing
// (`debug_layout_test.go` dumps the raw tables alone, as JSON, for diffing).
func TestProbe(t *testing.T) {
	path := os.Getenv("RADIX_PROBE_PDF")
	if path == "" {
		t.Skip("set RADIX_PROBE_PDF")
	}
	only := map[int]bool{}
	for _, p := range strings.Split(os.Getenv("RADIX_PROBE_PAGES"), ",") {
		var n int
		if _, err := fmt.Sscanf(strings.TrimSpace(p), "%d", &n); err == nil {
			only[n] = true
		}
	}

	f, r, err := pdf.Open(path)
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	defer f.Close()
	fmt.Printf("=== %s (%d pages)\n", path, r.NumPage())
	for page := 1; page <= r.NumPage(); page++ {
		if len(only) > 0 && !only[page] {
			continue
		}
		g, err := readPageGeometry(context.Background(), r.Page(page))
		if err != nil {
			fmt.Printf("--- PAGE %d: geometry error %v\n", page, err)
			continue
		}
		text := textInBox(g.glyphs, nil)
		tables, err := tableGeometry(context.Background(), g.edges, g.glyphs)
		if err != nil {
			fmt.Printf("--- PAGE %d: table error %v\n", page, err)
			continue
		}
		// A plan drawn without ruling lines leaves no table behind but stands in
		// the page text, so a page that only names an Anlage is worth showing.
		low := strings.ToLower(text)
		if len(only) == 0 && len(tables) == 0 && !strings.Contains(low, "studienplan") && !strings.Contains(low, "anlage") {
			continue
		}
		fmt.Printf("--- PAGE %d (%d tables)\n", page, len(tables))
		fmt.Printf("TEXT: %s\n", clipRunes(strings.Join(strings.Fields(text), " "), 1400))
		for ti, tab := range tables {
			fmt.Printf("TABLE %d.%d  %d rows\n", page, ti+1, len(tab.rows))
			for ri, row := range tab.rows {
				cells := make([]string, 0, len(row))
				for _, c := range row {
					cells = append(cells, clipRunes(cellText(c), 34))
				}
				fmt.Printf("   r%-3d %v\n", ri+1, cells)
			}
		}
	}

	fmt.Println("=== what the reader makes of it")
	layout, err := ReadPDFLayoutForProgram(context.Background(), path, nil, os.Getenv("RADIX_PROBE_HINT"))
	if err != nil {
		// The rejection is the answer, but what the reader saw before the program
		// filter dropped the other variants is usually what explains it.
		fmt.Printf("REJECTED: %v\n", err)
		raw, rawErr := extractPDFLayout(context.Background(), path)
		if rawErr != nil {
			fmt.Printf("even before the program filter: %v\n", rawErr)
			return
		}
		fmt.Printf("before the program filter: %d cells, %d totals, names %v, issues %v\n",
			len(raw.Cells), len(raw.Totals), raw.PlanNames, raw.Issues)
		layout = raw
	} else {
		fmt.Printf("ACCEPTED: %d cells, %d totals, names %v, issues %v\n", len(layout.Cells), len(layout.Totals), layout.PlanNames, layout.Issues)
	}
	for _, c := range layout.Cells {
		fmt.Printf("CELL  %-22s r%-3d sem=%v %v-%v raw=%q workload=%v credited=%d elective=%t alt=%d/%d  %s\n",
			c.ID, c.RowIndex, c.Semesters, c.Min, c.Max, c.Raw, c.Workload, c.CreditSemester, c.Elective, c.AltGroup, c.AltIndex, clipRunes(c.Row, 44))
	}
	for _, c := range layout.Totals {
		fmt.Printf("TOTAL %-22s r%-3d sem=%v %v-%v  %q (totalRow=%t workload=%t)\n",
			c.ID, c.RowIndex, c.Semesters, c.Min, c.Max, c.Row, totalRow.MatchString(c.Row), isAufwandRow(c))
	}
	// Which cells the printed sums count: an alternative, a grey „possible plan"
	// cell or a budget printed on top of a sum is not one of them.
	effective := map[string]bool{}
	for _, c := range effectiveCells(layout) {
		effective[c.ID] = true
	}
	var uncounted []string
	for _, c := range layout.Cells {
		if !effective[c.ID] {
			uncounted = append(uncounted, c.ID)
		}
	}
	fmt.Printf("COUNTED %d of %d cells; not counted: %v\n", len(effective), len(layout.Cells), uncounted)
	for _, g := range DerivePlanTotals(layout) {
		fmt.Printf("SUM   %-34s whole=%t choice=%t sem %d-%d = %v (its rows give %v-%v over %d rows)\n",
			clipRunes(g.Label, 34), g.WholePlan, g.Choice, g.Start, g.End, g.Credits, g.Min, g.Max, len(g.Members))
	}
}

func clipRunes(s string, n int) string {
	r := []rune(strings.Join(strings.Fields(s), " "))
	if len(r) > n {
		return string(r[:n]) + "…"
	}
	return string(r)
}
