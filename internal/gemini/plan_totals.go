package gemini

import (
	"fmt"
	"math"
	"sort"
	"strings"
)

// A regulation prints more than a list of modules: it prints what its rows have
// to add up to. „Summe Studium" under the whole table is what the degree costs;
// „Summe Komplexe des Fachstudiums 44" over three rows that each say „10-24" is
// the rule that ties those three ranges together. Without those sums a plan with
// elective budgets cannot be added up at all — three times „10-24" is anything
// between 30 and 72 LP, and only the printed 44 says which.
//
// PlanTotal is one such printed sum with the rows it counts. Nothing here is
// invented: a sum is kept only where the rows it is bound to actually reach it.

// PlanTotal is a sum the plan prints over a block of its own rows.
type PlanTotal struct {
	// ID is the source cell that printed the value.
	ID    string `json:"id"`
	Table string `json:"table"`
	// Label is the printed row label („Summe Studium").
	Label string `json:"label"`
	// WholePlan marks a total that counts the whole plan of its semesters, as
	// opposed to a section of it („Summe Komplex Mathematik").
	WholePlan bool `json:"whole_plan"`
	// Start and End are the semesters the sum covers; a merged cell over the
	// last two columns of the plan covers both.
	Start int `json:"start_semester"`
	End   int `json:"end_semester"`
	// Credits is what the plan prints.
	Credits float64 `json:"credits"`
	// Min and Max are what its rows come to: Min from the rows that lie entirely
	// inside these semesters, Max from those and whatever a row reaching into
	// them could add. Credits always lies between the two.
	Min float64 `json:"min_credits"`
	Max float64 `json:"max_credits"`
	// Choice marks a sum that is the only statement of how much its rows count
	// for: every row it names lies inside it, and at least one of them prints a
	// range („10-24 LP") instead of a number.
	Choice bool `json:"choice,omitempty"`
	// Members are the ids of the cells the sum counts, in plan order.
	Members []string `json:"members"`
	// Page is the PDF page the sum was printed on.
	Page int    `json:"page"`
	Raw  string `json:"raw"`
}

// DerivePlanTotals binds every printed sum of a layout to the rows above it.
//
// A study plan is printed like an account sheet: rows, then a line that sums
// them, then more rows and their line, and at the bottom a line over everything.
// A sum therefore counts the rows between it and the sum before it — and where
// that does not add up, the sum before it as well, because it is the line over a
// section that already has its own lines („Summe Grundstudium" over „Summe
// Komplex Informatik", „Summe Komplex Mathematik", „Summe Komplex Nebenfach").
//
// The binding is only kept where the source proves it: the rows must reach the
// printed value exactly, or, where they are elective budgets, contain it. A sum
// nothing explains is dropped rather than guessed at.
func DerivePlanTotals(layout *PDFLayout) []PlanTotal {
	if layout == nil {
		return nil
	}
	counted := map[string]SourceCell{}
	for _, c := range effectiveCells(layout) {
		counted[c.ID] = c
	}
	var out []PlanTotal
	for _, table := range tablesOf(layout) {
		out = append(out, planTotalsOfTable(layout, table, counted)...)
	}
	return out
}

// tablesOf lists the physical tables of a layout in the order they were read.
func tablesOf(layout *PDFLayout) []string {
	var order []string
	seen := map[string]bool{}
	for _, c := range layout.Cells {
		if !seen[c.Table] {
			seen[c.Table] = true
			order = append(order, c.Table)
		}
	}
	return order
}

// sumLine is one printed sum row: its label and the value it prints per
// semester span („Summe Studium" prints 32, 28, 30, 30 and 60 for 5-6).
type sumLine struct {
	row    int
	label  string
	values []SourceCell
}

func planTotalsOfTable(layout *PDFLayout, table string, counted map[string]SourceCell) []PlanTotal {
	// The rows of the table, in printing order, and the sums between them.
	rows := map[int][]SourceCell{}
	var cellRows []int
	for _, c := range layout.Cells {
		if c.Table != table || c.RowIndex == 0 {
			continue
		}
		if _, seen := rows[c.RowIndex]; !seen {
			cellRows = append(cellRows, c.RowIndex)
		}
		rows[c.RowIndex] = append(rows[c.RowIndex], c)
	}
	sort.Ints(cellRows)

	byRow := map[int]*sumLine{}
	var lines []*sumLine
	for _, t := range layout.Totals {
		// „Summe Aufwand" counts the work of a semester, not the credits booked
		// in it — a plan may print both lines with different numbers — and a sum
		// printed as a range says nothing exact enough to bind rows to.
		if t.Table != table || t.RowIndex == 0 || t.Min != t.Max || isAufwandRow(t) {
			continue
		}
		line := byRow[t.RowIndex]
		if line == nil {
			line = &sumLine{row: t.RowIndex, label: strings.TrimSpace(t.Row)}
			byRow[t.RowIndex] = line
			lines = append(lines, line)
		}
		line.values = append(line.values, t)
	}
	sort.Slice(lines, func(i, j int) bool { return lines[i].row < lines[j].row })

	// Blocks of rows that a sum has already been bound to, oldest first. A sum
	// that its own rows do not explain takes the block before it as well.
	type block struct{ from, to int }
	var blocks []block
	var out []PlanTotal
	boundary := 0
	for _, line := range lines {
		from, absorbed := boundary+1, 0
		var members []SourceCell
		for {
			members = members[:0]
			for _, row := range cellRows {
				// A row is either requirements or a sum, never both, so a sum
				// printed at the end of the row it sums (one row per semester,
				// its total in the last column) counts that row as well.
				if row >= from && row <= line.row {
					members = append(members, rows[row]...)
				}
			}
			if explains(members, line.values, counted) {
				break
			}
			if absorbed >= len(blocks) {
				members = nil
				break
			}
			absorbed++
			from = blocks[len(blocks)-absorbed].from
		}
		if members == nil {
			continue
		}
		blocks = blocks[:len(blocks)-absorbed]
		blocks = append(blocks, block{from, line.row})
		boundary = line.row
		whole := totalRow.MatchString(line.label)
		for _, value := range line.values {
			start, end := value.Semesters[0], value.Semesters[len(value.Semesters)-1]
			total := PlanTotal{ID: value.ID, Table: table, Label: line.label, WholePlan: whole,
				Start: start, End: end, Credits: value.Min, Page: value.Page, Raw: value.Raw}
			ranged, spilling := false, false
			for _, m := range members {
				c, ok := counted[m.ID]
				if !ok {
					continue
				}
				switch {
				case within(m, start, end):
					total.Members = append(total.Members, m.ID)
					total.Min += c.Min
					total.Max += c.Max
					ranged = ranged || c.Max-c.Min > 0.01
				case reaches(m, start, end):
					// The plan does not say how a module over several semesters
					// splits, so it can only raise what these semesters may hold.
					total.Max += c.Max
					spilling = true
				}
			}
			// A semester whose modules all reach into it from a span of their own
			// has no row to name, and the printed sum is all the plan says about
			// it. That is worth keeping: without it the semester has no figure.
			total.Choice = ranged && !spilling
			out = append(out, total)
		}
	}
	return out
}

// explains reports whether the rows of a block reach every value the sum line
// prints: exactly, or within the range elective budgets leave open.
//
// A module over several semesters belongs to none of them alone — the plan does
// not say how it splits — so it can only raise the upper bound of a semester it
// reaches into, exactly as the validation of the printed totals treats it. It
// does belong to the line as a whole, and a block that holds a row outside the
// semesters of the whole line reaches further than that line sums: such a block
// is not what this sum is about.
func explains(members []SourceCell, values []SourceCell, counted map[string]SourceCell) bool {
	if len(members) == 0 || len(values) == 0 {
		return false
	}
	first, last := values[0].Semesters[0], values[0].Semesters[0]
	for _, value := range values {
		first = min(first, value.Semesters[0])
		last = max(last, value.Semesters[len(value.Semesters)-1])
	}
	for _, m := range members {
		if !within(m, first, last) {
			return false
		}
	}
	for _, value := range values {
		start, end := value.Semesters[0], value.Semesters[len(value.Semesters)-1]
		low, high := 0.0, 0.0
		for _, m := range members {
			c, ok := counted[m.ID]
			if !ok {
				continue
			}
			switch {
			case within(m, start, end):
				low += c.Min
				high += c.Max
			case reaches(m, start, end):
				high += c.Max
			}
		}
		if value.Min < low-0.01 || value.Min > high+0.01 {
			return false
		}
	}
	return true
}

// reaches reports whether a cell overlaps the semesters of a printed sum without
// lying inside them: a module over semesters 3 and 4 next to a sum for 3 alone.
func reaches(c SourceCell, start, end int) bool {
	a, b := c.Semesters[0], c.Semesters[len(c.Semesters)-1]
	if c.CreditSemester > 0 {
		a, b = c.CreditSemester, c.CreditSemester
	}
	return a <= end && b >= start
}

// within reports whether a cell lies inside the semesters of a printed sum. A
// module that is credited in one semester of its span counts there.
func within(c SourceCell, start, end int) bool {
	a, b := c.Semesters[0], c.Semesters[len(c.Semesters)-1]
	if c.CreditSemester > 0 {
		a, b = c.CreditSemester, c.CreditSemester
	}
	return a >= start && b <= end
}

// Describe is the sentence a plan row carries about the sum it belongs to.
func (t PlanTotal) Describe() string {
	where := fmt.Sprintf("semester %d", t.Start)
	if t.End != t.Start {
		where = fmt.Sprintf("semesters %d-%d", t.Start, t.End)
	}
	if t.Choice {
		return fmt.Sprintf("%s: %s together %s LP in %s; their own ranges allow %s–%s LP.", t.Label,
			plural(len(t.Members), "requirement"), trimFloat(t.Credits), where, trimFloat(t.Min), trimFloat(t.Max))
	}
	return fmt.Sprintf("%s: %s together %s LP in %s.", t.Label, plural(len(t.Members), "requirement"), trimFloat(t.Credits), where)
}

func plural(n int, word string) string {
	if n == 1 {
		return fmt.Sprintf("1 %s", word)
	}
	return fmt.Sprintf("%d %ss", n, word)
}

func trimFloat(v float64) string {
	if math.Abs(v-math.Round(v)) < 0.01 {
		return fmt.Sprintf("%d", int(math.Round(v)))
	}
	return strings.TrimRight(fmt.Sprintf("%.1f", v), "0")
}
