package gemini

import "testing"

func TestFootnotedWorkload(t *testing.T) {
	for _, tc := range []struct {
		in, parts, total string
	}{
		{"(3+3) 61", "3+3", "6"},
		{"(3 + 2) 5", "3 + 2", "5"},
		{"12 1 (6 + 6)", "6 + 6", "12"},
		{"1 18 (9 + 9)", "9 + 9", "18"},
		{"12 5 (6+6)", "6+6", "12"},
	} {
		got := workloadParts(tc.in)
		if got == nil || got[1] != tc.parts || got[2] != tc.total {
			t.Errorf("%q: got %v, want parts %q total %q", tc.in, got, tc.parts, tc.total)
		}
	}
	// A total that does not match the bracket must not be guessed: the caller
	// compares the parts with the total and rejects the cell.
	for _, in := range []string{"(3+3) 8", "7 (3+3)", "12 34 (6+6)", "6 (3+3) 6 6"} {
		if got := workloadParts(in); got != nil && bracketSum(got[1]) == amountOf(got[2]) {
			t.Errorf("%q: unexpected %v", in, got)
		}
	}
}

func TestCreditRemarkAndOptionalPlacement(t *testing.T) {
	for in, want := range map[string]float64{"6(1+2)5)": 6, "6(3+4)⁵)": 6, "6 (1+2)": 6} {
		if got, ok := creditBeforeRemark(in); !ok || got != want {
			t.Errorf("%q: got %v %v", in, got, ok)
		}
	}
	if _, ok := creditBeforeRemark("6"); ok {
		t.Error("plain number is not a remark cell")
	}
	if got, ok := optionalPlacement("(12)"); !ok || got != 12 {
		t.Errorf("optional (12): %v %v", got, ok)
	}
	if _, ok := optionalPlacement("(6+6)"); ok {
		t.Error("workload bracket is not an optional placement")
	}
}

func TestMergeOptionalPlacements(t *testing.T) {
	l := &PDFLayout{Cells: []SourceCell{
		{ID: "a", Table: "t", Row: "Fixed", Semesters: []int{1}, Min: 6, Max: 6, BBox: []float64{0, 10, 5, 20}},
		{ID: "b", Table: "t", Row: "Wahl", Semesters: []int{5}, Min: 6, Max: 6, Raw: "(6)", Optional: true, BBox: []float64{50, 30, 60, 40}},
		{ID: "c", Table: "t", Row: "Wahl", Semesters: []int{6}, Min: 6, Max: 6, Raw: "(6)", Optional: true, BBox: []float64{60, 30, 70, 40}},
	}}
	mergeOptionalPlacements(l, 1)
	if len(l.Cells) != 2 {
		t.Fatalf("cells: %+v", l.Cells)
	}
	m := l.Cells[1]
	if m.Min != 6 || len(m.Semesters) != 2 || m.Semesters[0] != 5 || m.Semesters[1] != 6 {
		t.Errorf("merged: %+v", m)
	}
}

func TestTotalRowLabels(t *testing.T) {
	for _, yes := range []string{"Summe", "Summe Studium", "Summe der Leistungs- punkte", "Summe Aufwand", "LP Gesamt 210", "Teilsummen pro Semester", "Summe Masterstudium", "gesamt"} {
		if !totalRow.MatchString(yes) {
			t.Errorf("%q should be a whole-plan total", yes)
		}
	}
	for _, no := range []string{"Summe Grundstudium", "Summe Komplex Informatik", "Summe Fachstudium", "Mathematik"} {
		if totalRow.MatchString(no) {
			t.Errorf("%q must stay a partial subtotal", no)
		}
	}
}

func TestMarkedCredit(t *testing.T) {
	if v, opt, ok := markedCredit("12+"); !ok || opt || v != 12 {
		t.Errorf("12+: %v %v %v", v, opt, ok)
	}
	if v, opt, ok := markedCredit("(6+)"); !ok || !opt || v != 6 {
		t.Errorf("(6+): %v %v %v", v, opt, ok)
	}
	for _, in := range []string{"(6+", "6+)", "6", "(6)", "3+3"} {
		if _, _, ok := markedCredit(in); ok {
			t.Errorf("%q must not be a marked credit", in)
		}
	}
}
