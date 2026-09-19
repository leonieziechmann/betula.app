package gemini

import (
	"math"
	"regexp"
	"strings"
)

var headerNumber = regexp.MustCompile(`^\d{1,2}\.?$`)

// mergeContinuedTables joins a study table that runs over a page break. The
// next page repeats the header rows; a table already ending in a whole-plan
// total is complete and never absorbs its neighbor. Rows taken over from the
// later page are moved below the earlier rows so that label matching by
// vertical overlap keeps working, and remember their real page and offset.
func mergeContinuedTables(pages []layoutPage) {
	carry, merged := -1, false
	for i := 1; i < len(pages); i++ {
		prev := i - 1
		if len(pages[prev].tables) == 0 {
			if !merged || carry < 0 {
				merged = false
				continue
			}
			prev = carry
		}
		merged = false
		cur := &pages[i]
		holder := &pages[prev]
		if len(cur.tables) == 0 || pages[i-1].number+1 != cur.number {
			continue
		}
		// Small leftovers below the table (a wrapped label outside the rules)
		// do not end it.
		ai := len(holder.tables) - 1
		for ai > 0 && len(holder.tables[ai].rows) <= 3 {
			ai--
		}
		a := &holder.tables[ai]
		b := cur.tables[0]
		k := repeatedHeaderRows(*a, b)
		if k == 0 || hasTotalRow(*a) || len(b.rows) <= k {
			continue
		}
		ab, bb := tableBounds(*a), tableBounds(b)
		// Facing pages have mirrored margins: compare widths, then shift.
		if math.Abs((ab.x1-ab.x0)-(bb.x1-bb.x0)) > 1.5 {
			continue
		}
		dx := bb.x0 - ab.x0
		var top float64 = math.Inf(1)
		for _, box := range b.boxes[k] {
			if box != nil {
				top = math.Min(top, box.y0)
			}
		}
		if math.IsInf(top, 1) {
			continue
		}
		dy := top - ab.y1
		origins := a.paddedOrigins()
		for ri := k; ri < len(b.rows); ri++ {
			boxes := make([]*pdfBox, len(b.boxes[ri]))
			for ci, box := range b.boxes[ri] {
				if box != nil {
					moved := *box
					moved.x0, moved.y0, moved.x1, moved.y1 = box.x0-dx, box.y0-dy, box.x1-dx, box.y1-dy
					boxes[ci] = &moved
				}
			}
			a.rows = append(a.rows, b.rows[ri])
			a.boxes = append(a.boxes, boxes)
			o := b.origin(ri)
			if o.page == 0 {
				o = rowOrigin{cur.number, dx, dy}
			}
			origins = append(origins, o)
		}
		a.origins = origins
		cur.tables = cur.tables[1:]
		carry, merged = prev, len(cur.tables) == 0
	}
}

// repeatedHeaderRows counts the leading rows of b that repeat a's header. The
// repeated block must name semesters or credits, otherwise two unrelated tables
// with equal first rows could be glued together.
func repeatedHeaderRows(a, b pdfTable) int {
	if len(a.rows) < 3 || len(b.rows) < 2 {
		return 0
	}
	k, numbers, text := 0, 0, ""
	for k < 4 && k < len(a.rows) && k < len(b.rows) {
		ta, tb := nonEmptyTexts(a.rows[k]), nonEmptyTexts(b.rows[k])
		if len(ta) == 0 || len(ta) != len(tb) {
			break
		}
		same := true
		for i := range ta {
			if ta[i] != tb[i] {
				same = false
				break
			}
		}
		if !same {
			break
		}
		for _, cell := range ta {
			text += " " + strings.ToLower(cell)
			if headerNumber.MatchString(cell) {
				numbers++
			}
		}
		k++
	}
	if k == 0 || !(semesterHeading.MatchString(text) || creditHeading.MatchString(text) || numbers >= 2) {
		return 0
	}
	return k
}

// nonEmptyTexts ignores empty and merged (nil) cells, whose number differs
// between pages although the printed header is the same.
func nonEmptyTexts(row []*string) []string {
	var out []string
	for _, s := range row {
		if t := cellText(s); t != "" {
			out = append(out, t)
		}
	}
	return out
}

func hasTotalRow(t pdfTable) bool {
	// The header rows also print "Summe LP" over the total column.
	for ri := 3; ri < len(t.rows); ri++ {
		for _, s := range t.rows[ri] {
			if totalRow.MatchString(cellText(s)) {
				return true
			}
		}
	}
	return false
}
