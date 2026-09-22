package gemini

import (
	"fmt"
	"math"
	"regexp"
	"sort"
	"strings"
)

func tableBounds(t pdfTable) pdfBox {
	b := pdfBox{math.Inf(1), math.Inf(1), math.Inf(-1), math.Inf(-1), false, false}
	for _, row := range t.boxes {
		for _, c := range row {
			if c != nil {
				b.x0 = math.Min(b.x0, c.x0)
				b.y0 = math.Min(b.y0, c.y0)
				b.x1 = math.Max(b.x1, c.x1)
				b.y1 = math.Max(b.y1, c.y1)
			}
		}
	}
	return b
}

// A word such as "oder" can interrupt the table border without ending the
// study plan. Join only aligned fragments with a recognized semester header
// above and a total below; arbitrary neighboring tables stay independent.
func mergeStudyFragments(tables []pdfTable, g pdfPageGeometry) []pdfTable {
	var out []pdfTable
	for _, next := range tables {
		if len(out) > 0 {
			last := &out[len(out)-1]
			a, b := tableBounds(*last), tableBounds(next)
			prefix := ""
			for i, row := range last.rows {
				if i >= 4 {
					break
				}
				for _, s := range row {
					prefix += " " + cellText(s)
				}
			}
			lower := ""
			for _, row := range next.rows {
				for _, s := range row {
					lower += " " + cellText(s)
				}
			}
			gap := cleanPDFText(textInBox(g.glyphs, &pdfBox{a.x0, a.y1, a.x1, b.y0, false, false}))
			aligned := math.Abs(a.x0-b.x0) < 1 && math.Abs(a.x1-b.x1) < 1 && b.y0-a.y1 >= -1 && b.y0-a.y1 < 60
			if aligned && semesterHeading.MatchString(strings.ToLower(prefix)) && !semesterHeading.MatchString(strings.ToLower(lower)) && (subtotalLabel.MatchString(lower) || (!hasTotalRow(*last) && hasCreditCells(next))) && len(last.rows[0]) == len(next.rows[0]) {
				if (gap == "" || strings.EqualFold(gap, "oder")) && b.y0-a.y1 < 35 {
					last.origins = append(last.paddedOrigins(), next.paddedOrigins()...)
					last.rows = append(last.rows, next.rows...)
					last.boxes = append(last.boxes, next.boxes...)
					continue
				}
				// Rows whose rules are missing in the PDF drop out of the table
				// geometry although they are still printed between two fragments.
				if rows, boxes := gapRows(g, next, a.y1, b.y0); len(rows) > 0 {
					last.origins = append(append(last.paddedOrigins(), make([]rowOrigin, len(rows))...), next.paddedOrigins()...)
					last.rows = append(append(last.rows, rows...), next.rows...)
					last.boxes = append(append(last.boxes, boxes...), next.boxes...)
					continue
				}
			}
		}
		out = append(out, next)
	}
	return out
}

// namedTrackEnd is the row after the block of the track starting at `start`.
// The plan prints the extent of a section itself: its rows are the ones its own
// „Summe LP" cell covers — a merged cell spanning them, or the value on the
// heading row, which holds until the next section prints one. A heading that
// carries no value at all is closed by the next heading. Everything after that
// belongs to no track and is shared by both.
func namedTrackEnd(t pdfTable, start int) int {
	var own *pdfBox
	for ri := start; ri < len(t.rows); ri++ {
		row := t.rows[ri]
		if len(row) < 3 {
			continue
		}
		if ri > start && (isBareLabelRow(row) || totalRow.MatchString(strings.ToLower(cellText(row[0])))) {
			return ri
		}
		last := len(row) - 1
		if _, _, ok := parseCreditAmount(cellText(row[last])); !ok || t.boxes[ri][last] == nil {
			continue
		}
		if own == nil {
			own = t.boxes[ri][last]
			continue
		}
		if t.boxes[ri][last].y0 >= own.y1-1 {
			return ri
		}
	}
	return len(t.rows)
}

func splitNamedTracks(t pdfTable) ([]pdfTable, []string) {
	var starts []int
	var names []string
	for ri, row := range t.rows {
		if len(row) == 0 {
			continue
		}
		label := strings.ToLower(cellText(row[0]))
		if strings.Contains(label, "praxisintegrierendes studium") {
			starts = append(starts, ri)
			names = append(names, "Dual praxisintegrierend")
		}
		if strings.Contains(label, "ausbildungsintegrierendes studium") {
			starts = append(starts, ri)
			names = append(names, "Dual ausbildungsintegrierend")
		}
		if strings.Contains(label, "fachspezifische module in der studienrichtung") {
			starts = append(starts, ri)
			names = append(names, cellText(row[0]))
		}
	}
	if len(starts) != 2 {
		return nil, nil
	}
	end := namedTrackEnd(t, starts[1])
	for ri := starts[1] + 1; ri < end; ri++ {
		label := strings.ToLower(cellText(t.rows[ri][0]))
		if strings.Contains(label, "bachelor-arbeit") || strings.Contains(label, "master-arbeit") || label == "wahlpflicht-module" || totalRow.MatchString(label) {
			end = ri
			break
		}
	}
	var out []pdfTable
	for v := 0; v < 2; v++ {
		copy := pdfTable{}
		for ri := range t.rows {
			if (v == 0 && ri >= starts[1] && ri < end) || (v == 1 && ri >= starts[0] && ri < starts[1]) {
				continue
			}
			copy.rows = append(copy.rows, t.rows[ri])
			copy.boxes = append(copy.boxes, t.boxes[ri])
			copy.origins = append(copy.origins, t.origin(ri))
		}
		out = append(out, copy)
	}
	return out, names
}

func appendTrackVariants(l *PDFLayout, t pdfTable, page, number int, workload, isPlan bool, prefix string) bool {
	variants, names := splitStudyDirections(t)
	if len(variants) == 0 {
		variants, names = splitNamedTracks(t)
	}
	if len(variants) == 0 {
		return false
	}
	found := false
	for i, v := range variants {
		id := (number)*1000 + i + 1
		table := fmt.Sprintf("p%dt%d", page, id)
		before := len(l.Cells)
		appendStudyTable(l, v, page, id, workload, isPlan)
		if len(l.Cells) == before {
			continue // an overview table with the same section rows, no plan
		}
		found = true
		if l.PlanNames == nil {
			l.PlanNames = map[string]string{}
		}
		// The prefix names the dual mode of the whole Anlage. Where the track
		// inside the table names one itself, it is the more precise of the two
		// and the prefix would only repeat it („Dual praxisintegrierend · Dual
		// praxisintegrierend").
		name := prefix + names[i]
		if dualMode(names[i]) != "" {
			name = names[i]
		}
		l.PlanNames[table] = name
	}
	return found
}

var studyDirectionKey = regexp.MustCompile(`(?i)^studienrichtung\s+`)

// isStudyDirectionHeader recognizes a section row "Studienrichtung X" that only
// carries a label and, at most, a sum in the last column.
func isStudyDirectionHeader(row []*string) bool {
	if len(row) < 3 || !studyDirectionKey.MatchString(cellText(row[0])) {
		return false
	}
	for ci := 1; ci < len(row)-1; ci++ {
		if row[ci] != nil && cellText(row[ci]) != "" {
			return false
		}
	}
	return true
}

// isSectionRow is a label row that only carries a sum in its last column.
func isSectionRow(row []*string) bool {
	if len(row) < 3 || cellText(row[0]) == "" {
		return false
	}
	for ci := 1; ci < len(row)-1; ci++ {
		if cellText(row[ci]) != "" {
			return false
		}
	}
	_, _, ok := parseCreditAmount(cellText(row[len(row)-1]))
	return ok
}

// splitStudyDirections separates a plan that prints several specializations
// ("Studienrichtung X") in one table. Rows belong to the direction whose section
// they follow; sections that do not name a direction (common modules, thesis)
// are shared. Detached sections such as "Wahlpflichtmodule ... Studienrichtung
// X" and per-direction total lines are assigned by the name they mention.
func splitStudyDirections(t pdfTable) ([]pdfTable, []string) {
	var names, keys []string
	starts := map[int]int{}
	for ri, row := range t.rows {
		if isStudyDirectionHeader(row) {
			starts[ri] = len(names)
			names = append(names, cellText(row[0]))
			keys = append(keys, strings.ToLower(studyDirectionKey.ReplaceAllString(cellText(row[0]), "")))
		}
	}
	if len(names) < 2 {
		return nil, nil
	}
	owner := make([]int, len(t.rows))
	cur, closable := -1, false
	for ri, row := range t.rows {
		label := strings.ToLower(cellText(row[0]))
		if k, ok := starts[ri]; ok {
			cur, closable = k, false
			for next := ri + 1; next < len(t.rows); next++ {
				if !isSectionRow(t.rows[next]) && cellText(t.rows[next][0]) != "" {
					closable = !rowHasLastValue(t.rows[next])
					break
				}
			}
			owner[ri] = k
			continue
		}
		if isSectionRow(row) {
			cur = matchDirection(label, keys)
			closable = false
			for next := ri + 1; next < len(t.rows); next++ {
				if !isSectionRow(t.rows[next]) && cellText(t.rows[next][0]) != "" {
					closable = !rowHasLastValue(t.rows[next])
					break
				}
			}
			owner[ri] = cur
			continue
		}
		if subtotalLabel.MatchString(label) || strings.Contains(label, "aufwand") {
			owner[ri] = matchDirection(label, keys)
			continue
		}
		// A bare label row -- a heading that carries nothing but its own text,
		// not even a sum -- opens a new section. Where it names no direction,
		// that section belongs to all of them ("Weitere Module" under the last
		// direction's rows), exactly as a heading before the first direction does.
		if isBareLabelRow(row) && matchDirection(label, keys) < 0 {
			cur, closable = -1, false
			owner[ri] = -1
			continue
		}
		if cur >= 0 && closable && rowHasLastValue(row) {
			cur = -1
		}
		owner[ri] = cur
	}
	// One unlabeled total line per direction after the last direction section
	// ("gesamt" under each "Arbeitsaufwand ..." line) belongs to them in order.
	lastStart := 0
	for ri := range starts {
		if ri > lastStart {
			lastStart = ri
		}
	}
	var unowned []int
	for ri, row := range t.rows {
		label := strings.ToLower(cellText(row[0]))
		if ri > lastStart && owner[ri] == -1 && subtotalLabel.MatchString(label) && !strings.Contains(label, "aufwand") && rowHasNumbers(row) {
			unowned = append(unowned, ri)
		}
	}
	if len(unowned) == len(names) {
		for i, ri := range unowned {
			owner[ri] = i
		}
	}
	var out []pdfTable
	for v := range names {
		copy := pdfTable{}
		for ri := range t.rows {
			if owner[ri] == -1 || owner[ri] == v {
				copy.rows = append(copy.rows, t.rows[ri])
				copy.boxes = append(copy.boxes, t.boxes[ri])
				copy.origins = append(copy.origins, t.origin(ri))
			}
		}
		out = append(out, copy)
	}
	return out, names
}

// isBareLabelRow is a heading row: a label in the first column and nothing at
// all in the others -- no per-semester value and no sum. isSectionRow is its
// sibling for a heading that does print its section's sum.
func isBareLabelRow(row []*string) bool {
	if len(row) < 3 || cellText(row[0]) == "" {
		return false
	}
	for ci := 1; ci < len(row); ci++ {
		if cellText(row[ci]) != "" {
			return false
		}
	}
	return true
}

func rowHasLastValue(row []*string) bool {
	return len(row) > 1 && cellText(row[len(row)-1]) != ""
}

// matchDirection finds the direction a label talks about. The printed names are
// not always spelled identically ("Energieökonomik"/"Energieökonomie"), so a
// long shared prefix counts as well.
func matchDirection(label string, keys []string) int {
	for i, key := range keys {
		if key != "" && strings.Contains(label, key) {
			return i
		}
	}
	for i, key := range keys {
		if r := []rune(key); len(r) >= 8 && strings.Contains(label, string(r[:len(r)*3/4])) {
			return i
		}
	}
	return -1
}

// hasCreditCells reports whether a headerless fragment carries credit numbers
// in more than its label column, as the rest of a study table does.
func hasCreditCells(t pdfTable) bool {
	n := 0
	for _, row := range t.rows {
		for ci := 1; ci < len(row); ci++ {
			if _, _, ok := parseCreditAmount(cellText(row[ci])); ok {
				n++
			}
		}
	}
	return n >= 2
}

// gapRows rebuilds table rows from text lying between two study-plan fragments.
// Columns come from the widest row of the template table. A line without text
// outside the first column is only a wrapped label and joins the nearest row
// with numbers; if nothing numeric is printed the gap is not a table row.
func gapRows(g pdfPageGeometry, next pdfTable, y0, y1 float64) ([][]*string, [][]*pdfBox) {
	var template []*pdfBox
	for _, row := range next.boxes {
		count := 0
		for _, b := range row {
			if b != nil {
				count++
			}
		}
		if count > len(template) {
			template = row
		}
	}
	if len(template) < 3 || template[0] == nil {
		return nil, nil
	}
	firstEnd := template[0].x1
	left, right := template[0].x0, template[0].x1
	for _, b := range template {
		if b != nil {
			right = math.Max(right, b.x1)
		}
	}
	var glyphs []pdfGlyph
	for _, gl := range g.glyphs {
		if gl.rotation != 0 {
			continue
		}
		if gl.y >= y0 && gl.y < y1 && gl.x >= left && gl.x < right {
			glyphs = append(glyphs, gl)
		}
	}
	if len(glyphs) == 0 {
		return nil, nil
	}
	sort.SliceStable(glyphs, func(i, j int) bool { return glyphs[i].y < glyphs[j].y })
	type line struct {
		y       float64
		glyphs  []pdfGlyph
		numeric bool
	}
	var lines []*line
	for _, gl := range glyphs {
		if n := len(lines); n > 0 && gl.y-lines[n-1].y <= 3 {
			lines[n-1].glyphs = append(lines[n-1].glyphs, gl)
		} else {
			lines = append(lines, &line{y: gl.y, glyphs: []pdfGlyph{gl}})
		}
		if gl.x >= firstEnd && strings.TrimSpace(gl.text) != "" {
			lines[len(lines)-1].numeric = true
		}
	}
	var anchors []*line
	for _, l := range lines {
		if l.numeric {
			anchors = append(anchors, l)
		}
	}
	if len(anchors) == 0 {
		return nil, nil
	}
	rows := make([][]pdfGlyph, len(anchors))
	for _, l := range lines {
		best := 0
		for i, a := range anchors {
			if math.Abs(a.y-l.y) < math.Abs(anchors[best].y-l.y) {
				best = i
			}
		}
		// A label line far from every numeric line is prose, not a wrapped label.
		if math.Abs(anchors[best].y-l.y) > 12 {
			continue
		}
		rows[best] = append(rows[best], l.glyphs...)
	}
	var outRows [][]*string
	var outBoxes [][]*pdfBox
	for i, rg := range rows {
		top, bottom := y0, y1
		if i > 0 {
			top = (anchors[i-1].y + anchors[i].y) / 2
		}
		if i+1 < len(anchors) {
			bottom = (anchors[i].y + anchors[i+1].y) / 2
		}
		var cells []*string
		var boxes []*pdfBox
		for _, c := range template {
			if c == nil {
				cells, boxes = append(cells, nil), append(boxes, nil)
				continue
			}
			text := textInBox(rg, &pdfBox{c.x0, top, c.x1, bottom, false, false})
			cells = append(cells, &text)
			boxes = append(boxes, &pdfBox{c.x0, top, c.x1, bottom, false, false})
		}
		for ci := 1; ci < len(cells); ci++ {
			if cells[ci] != nil && !numberLike.MatchString(cleanPDFText(*cells[ci])) {
				return nil, nil
			}
		}
		outRows, outBoxes = append(outRows, cells), append(outBoxes, boxes)
	}
	return outRows, outBoxes
}

var numberLike = regexp.MustCompile(`^[\d\s.,+()/⁰¹²³⁴⁵⁶⁷⁸⁹-]*$`)

// withTailRows appends rows printed below a study table whose rules stop early
// (typically the last rows and the total lines). Lines are taken while they
// follow each other closely; anything that is not a row of numbers under the
// table's columns makes the whole attempt void.
func withTailRows(t pdfTable, g pdfPageGeometry) pdfTable {
	if len(t.rows) < 3 || hasTotalRow(t) {
		return t
	}
	head := ""
	for i := 0; i < len(t.rows) && i < 4; i++ {
		for _, s := range t.rows[i] {
			head += " " + strings.ToLower(cellText(s))
		}
	}
	if !semesterHeading.MatchString(head) && !creditHeading.MatchString(head) {
		return t
	}
	b := tableBounds(t)
	var ys []float64
	for _, gl := range g.glyphs {
		if gl.rotation == 0 && gl.y >= b.y1 && gl.x >= b.x0 && gl.x < b.x1 && strings.TrimSpace(gl.text) != "" {
			ys = append(ys, gl.y)
		}
	}
	sort.Float64s(ys)
	end, prev := b.y1, b.y1
	for _, y := range ys {
		if y-prev > 40 {
			break
		}
		prev, end = y, y+1
	}
	if end == b.y1 {
		return t
	}
	rows, boxes := gapRows(g, t, b.y1, end)
	if len(rows) == 0 {
		return t
	}
	out := pdfTable{origins: append(t.paddedOrigins(), make([]rowOrigin, len(rows))...)}
	out.rows = append(append(out.rows, t.rows...), rows...)
	out.boxes = append(append(out.boxes, t.boxes...), boxes...)
	return out
}

func rowHasNumbers(row []*string) bool {
	for ci := 1; ci < len(row); ci++ {
		if _, _, ok := parseCreditAmount(cellText(row[ci])); ok {
			return true
		}
	}
	return false
}
