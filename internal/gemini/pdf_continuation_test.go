package gemini

import "testing"

func testTable(x0 float64, rows [][]string, top float64, withTotal bool) pdfTable {
	t := pdfTable{}
	y := top
	for _, r := range rows {
		var cells []*string
		var boxes []*pdfBox
		for ci, s := range r {
			s := s
			cells = append(cells, &s)
			boxes = append(boxes, &pdfBox{x0 + float64(ci)*50, y, x0 + float64(ci+1)*50, y + 10, false, false})
		}
		t.rows, t.boxes = append(t.rows, cells), append(t.boxes, boxes)
		y += 10
	}
	return t
}

func TestMergeContinuedTablesJoinsRepeatedHeader(t *testing.T) {
	head := [][]string{{"Modul", "Semester", "", ""}, {"", "1", "2", "3"}}
	first := testTable(40, append(append([][]string{}, head...), []string{"Analysis", "6", "", ""}, []string{"Physik", "", "6", ""}), 100, false)
	second := testTable(55, append(append([][]string{}, head...), []string{"Chemie", "", "", "6"}, []string{"Summe", "6", "6", "6"}), 60, true)
	pages := []layoutPage{{number: 3, tables: []pdfTable{first}}, {number: 4, tables: []pdfTable{second}}}
	mergeContinuedTables(pages)
	if len(pages[1].tables) != 0 || len(pages[0].tables[0].rows) != 6 {
		t.Fatalf("not merged: %d tables on page 4, %d rows", len(pages[1].tables), len(pages[0].tables[0].rows))
	}
	m := pages[0].tables[0]
	if m.origin(3).page != 0 || m.origin(4).page != 4 || m.origin(5).page != 4 {
		t.Fatalf("origins: %+v", m.origins)
	}
	// Moved rows sit directly below the first fragment and use its columns.
	if got := m.boxes[4][0]; got.x0 != 40 || got.y0 != 140 {
		t.Errorf("moved box: %+v", got)
	}
	// The original coordinates are recoverable from the recorded offsets.
	if o := m.origin(4); m.boxes[4][0].y0+o.dy != 80 || m.boxes[4][0].x0+o.dx != 55 {
		t.Errorf("origin offsets do not restore the page position: %+v", o)
	}
}

func TestMergeContinuedTablesKeepsCompleteTablesApart(t *testing.T) {
	head := [][]string{{"Modul", "Semester", "", ""}, {"", "1", "2", "3"}}
	done := testTable(40, append(append([][]string{}, head...), []string{"Analysis", "6", "", ""}, []string{"Summe", "6", "", ""}), 100, true)
	next := testTable(40, append(append([][]string{}, head...), []string{"Chemie", "", "", "6"}, []string{"Summe", "", "", "6"}), 60, true)
	pages := []layoutPage{{number: 3, tables: []pdfTable{done}}, {number: 4, tables: []pdfTable{next}}}
	mergeContinuedTables(pages)
	if len(pages[1].tables) != 1 {
		t.Fatal("a table that already has its total row must not absorb the next plan")
	}
}

func TestMergeContinuedTablesNeedsSameHeader(t *testing.T) {
	a := testTable(40, [][]string{{"Modul", "Semester", "", ""}, {"", "1", "2", "3"}, {"Analysis", "6", "", ""}}, 100, false)
	b := testTable(40, [][]string{{"Anderes", "Semester", "", ""}, {"", "1", "2", "3"}, {"Chemie", "", "", "6"}}, 60, false)
	pages := []layoutPage{{number: 3, tables: []pdfTable{a}}, {number: 4, tables: []pdfTable{b}}}
	mergeContinuedTables(pages)
	if len(pages[1].tables) != 1 {
		t.Fatal("different headers were merged")
	}
}
