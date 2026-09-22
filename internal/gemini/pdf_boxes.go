package gemini

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

// creditColumnLabel is the label of a row that prints nothing but the credits of
// each semester („Leistungspunkte", „LP", „Credits"). Unlike the „Summe …" rows
// it names no sum, so totalRow does not know it.
var creditColumnLabel = regexp.MustCompile(`^(?:lp|kp|cp|ects|credits?|credit points|leistungspunkte|kreditpunkte)$`)

// Plans made of named module boxes underneath "n. Semester" headings.
// Merged boxes keep their complete horizontal span; vertical height never
// changes credit values or duplicates a module.
func appendBoxStudyTable(l *PDFLayout, t pdfTable, page, number int, refs map[string]creditReference) {
	for hr, row := range t.rows {
		if hr > 4 {
			break
		}
		var headers []*pdfBox
		for ci, s := range row {
			m := panelSemester.FindStringSubmatch(cellText(s))
			if m == nil {
				continue
			}
			n, _ := strconv.Atoi(m[1])
			if n != len(headers)+1 {
				return
			}
			headers = append(headers, t.boxes[hr][ci])
		}
		if len(headers) < 2 {
			continue
		}
		id := fmt.Sprintf("p%dt%dboxes", page, number)
		var cells, totals []SourceCell
		var issues []string
		sections := map[int]string{}
		for ri := hr + 1; ri < len(t.rows); ri++ {
			// A row that names no credits anywhere is a caption over the
			// semesters it spans („Specialization Phase", „Research Phase" in
			// the Physics plan), not a requirement that lost its LP.
			if caption := captionBoxes(t, ri, headers); caption != nil {
				for semester, label := range caption {
					sections[semester] = label
				}
				continue
			}
			for ci, b := range t.boxes[ri] {
				if b == nil || b.x0 < headers[0].x0-1 {
					continue
				}
				value := cellText(t.rows[ri][ci])
				if value == "" {
					continue
				}
				var semesters []int
				for si, h := range headers {
					if h != nil && (h.x0+h.x1)/2 >= b.x0 && (h.x0+h.x1)/2 < b.x1 {
						semesters = append(semesters, si+1)
					}
				}
				if len(semesters) == 0 {
					continue
				}
				c := SourceCell{ID: fmt.Sprintf("%sr%dc%d", id, ri+1, ci+1), Table: id, Page: page, RowIndex: ri + 1, Semesters: semesters, Raw: value, BBox: []float64{b.x0, b.y0, b.x1, b.y1}}
				label := strings.ToLower(cellText(t.rows[ri][0]))
				sigmaValue := strings.HasPrefix(value, "Σ") || strings.HasPrefix(value, "∑")
				if creditColumnLabel.MatchString(label) || (totalRow.MatchString(label) && !sigmaValue) {
					lo, hi, ok := parseCreditAmount(value)
					if !ok {
						issues = append(issues, c.ID+": invalid total")
						continue
					}
					c.Row, c.Min, c.Max = "Summe", lo, hi
					totals = append(totals, c)
					continue
				}
				matches := inlineLP.FindAllStringSubmatch(value, -1)
				if len(matches) == 1 && (strings.HasPrefix(value, "Σ") || strings.HasPrefix(value, "∑")) {
					amount, _, ok := parseCreditAmount(matches[0][1])
					if ok {
						c.Row, c.Min, c.Max = "Summe", amount, amount
						totals = append(totals, c)
					}
					continue
				}
				if len(matches) == 0 {
					if m := creditReferencePattern.FindStringSubmatch(value); m != nil {
						if ref, found := refs[m[1]+":"+m[2]]; found {
							c.Row = strings.TrimSpace(creditReferencePattern.ReplaceAllString(value, ""))
							if ref.code != "" {
								c.Row = ref.code + " " + c.Row
							}
							c.Min, c.Max = ref.amount, ref.amount
							c.CreditEvidence = ref.evidence
							cells = append(cells, c)
							continue
						}
					}
				}
				if len(matches) != 1 {
					issues = append(issues, c.ID+": unsupported module box "+value)
					continue
				}
				amount, _, ok := parseCreditAmount(matches[0][1])
				if !ok {
					continue
				}
				c.Row = strings.TrimSpace(inlineLP.ReplaceAllString(value, ""))
				c.Min, c.Max = amount, amount
				cells = append(cells, c)
			}
		}
		for i := range cells {
			cells[i].Section = sections[cells[i].Semesters[0]]
		}
		// A box table whose cells carry no module name is the same ruled table
		// read a second time (credit values only); it adds nothing.
		for _, c := range cells {
			if strings.TrimSpace(c.Row) == "" {
				cells = nil
				break
			}
		}
		if len(cells) > 0 {
			l.Cells = append(l.Cells, cells...)
			l.Totals = append(l.Totals, totals...)
			l.Issues = append(l.Issues, issues...)
		}
		return
	}
}

// captionBoxes reads a row of the box plan that carries labels but no credits
// anywhere: the phases a plan prints over its semester columns. It returns the
// label of each semester the row's boxes span, or nil when the row names any
// credit value, or points at one, and is therefore read as requirements.
func captionBoxes(t pdfTable, ri int, headers []*pdfBox) map[int]string {
	labels := map[int]string{}
	for ci, b := range t.boxes[ri] {
		if b == nil || b.x0 < headers[0].x0-1 {
			continue
		}
		value := cellText(t.rows[ri][ci])
		if value == "" {
			continue
		}
		var semesters []int
		for si, h := range headers {
			if h != nil && (h.x0+h.x1)/2 >= b.x0 && (h.x0+h.x1)/2 < b.x1 {
				semesters = append(semesters, si+1)
			}
		}
		if len(semesters) == 0 {
			continue
		}
		if len(inlineLP.FindAllStringSubmatch(value, -1)) > 0 {
			return nil
		}
		if _, _, ok := parseCreditAmount(value); ok {
			return nil
		}
		// „Entwurfsprojekt 1 (Gemäß Anlage 1, Nr. 1)" is a module whose credits
		// stand in an appendix — a requirement even where that appendix was not
		// read, and never a caption.
		if creditReferencePattern.MatchString(value) {
			return nil
		}
		for _, semester := range semesters {
			labels[semester] = value
		}
	}
	if len(labels) == 0 {
		return nil
	}
	return labels
}
