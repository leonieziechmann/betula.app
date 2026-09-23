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
			if line := semesterTotalRow(t, ri, headers); line != nil {
				for _, s := range line {
					s.ID = fmt.Sprintf("%sr%dc%d", id, ri+1, s.column+1)
					s.Table, s.Page, s.RowIndex, s.Row = id, page, ri+1, "Summe"
					totals = append(totals, s.SourceCell)
				}
				continue
			}
			for ci, b := range t.boxes[ri] {
				if b == nil || b.x0 < headers[0].x0-1 {
					continue
				}
				value := cellText(t.rows[ri][ci])
				if value == "" || legendBox.MatchString(value) {
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
					parts := splitReferencedModules(value)
					if len(parts) > 0 {
						resolved := make([]SourceCell, 0, len(parts))
						ok := true
						for pi, part := range parts {
							ref, found := refs[part.annex+":"+part.number]
							if !found || part.name == "" {
								ok = false
								break
							}
							p := c
							if len(parts) > 1 {
								p.ID = fmt.Sprintf("%sm%d", c.ID, pi+1)
							}
							p.Row = part.name
							if ref.code != "" && !strings.ContainsRune(ref.code, ' ') {
								p.Row = ref.code + " " + part.name
							}
							p.Min, p.Max = ref.amount, ref.amount
							p.CreditEvidence = ref.evidence
							p.AltGroup, p.AltIndex = part.altGroup, part.altIndex
							if part.altGroup > 0 {
								p.AltGroup = ri*1000 + ci + 1
							}
							resolved = append(resolved, p)
						}
						if ok {
							cells = append(cells, resolved...)
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

// legendBox is a box that explains the plan instead of requiring something of it:
// the footnotes a box plan prints under its own total line. Such a box may well
// name a credit value („Abweichungen in der Summe der LP von 30 LP je Semester"),
// which would otherwise be read as a requirement of every semester it spans.
var legendBox = regexp.MustCompile(`(?i)^\s*(erläuterung|legende|hinweis|anmerkung|fußnote|schattierung)`)

// referencedModule is one module of a box that prints no credits of its own:
// its name and the appendix row its credits stand in. „oder" between two of them
// makes them alternatives, of which the plan's sums count one.
type referencedModule struct {
	name, annex, number string
	altGroup, altIndex  int
}

// splitReferencedModules cuts a module box into the modules it names. A box may
// hold more than one („Entwurfsprojekt 1 (Gemäß Anlage 1, Nr. 1) Integrationsmodul
// (Gemäß Anlage 1, Nr. 7)"), and each name stands before its own reference.
func splitReferencedModules(value string) []referencedModule {
	locs := creditReferencePattern.FindAllStringSubmatchIndex(value, -1)
	if len(locs) == 0 {
		return nil
	}
	var out []referencedModule
	prev := 0
	for _, loc := range locs {
		name := strings.TrimSpace(value[prev:loc[0]])
		alt := 0
		if i := strings.LastIndex(strings.ToLower(name), " oder "); i >= 0 {
			name = strings.TrimSpace(name[i+6:])
			alt = 1
		} else if strings.HasPrefix(strings.ToLower(name), "oder ") {
			name = strings.TrimSpace(name[5:])
			alt = 1
		}
		out = append(out, referencedModule{name: name, annex: value[loc[2]:loc[3]], number: value[loc[4]:loc[5]], altGroup: alt})
		prev = loc[1]
	}
	// „A (Nr. 1) oder B (Nr. 2)": every part of the box is one alternative of
	// the same choice, numbered in printing order.
	group := 0
	for _, p := range out {
		group += p.altGroup
	}
	if group > 0 {
		for i := range out {
			out[i].altGroup, out[i].altIndex = 1, i
		}
	} else {
		for i := range out {
			out[i].altGroup = 0
		}
	}
	return out
}

// semesterTotalRow reads the unlabeled line a box plan prints under its columns:
// one bare credit amount per semester („30 LP  30 LP  30 LP  30 LP"). It is a
// sum, not a requirement, and only that: every column of the plan must hold
// exactly one box, and every one of those boxes nothing but a credit value. A
// row that names anything at all, or that misses a column, is read as modules.
type semesterSum struct {
	SourceCell
	column int
}

func semesterTotalRow(t pdfTable, ri int, headers []*pdfBox) []*semesterSum {
	out := make([]*semesterSum, len(headers))
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
		if len(semesters) != 1 {
			return nil
		}
		amount := strings.TrimSpace(creditUnitSuffix.ReplaceAllString(value, ""))
		lo, hi, ok := parseCreditAmount(amount)
		if !ok {
			return nil
		}
		si := semesters[0] - 1
		if out[si] != nil {
			return nil
		}
		out[si] = &semesterSum{SourceCell{Semesters: semesters, Raw: value, Min: lo, Max: hi,
			BBox: []float64{b.x0, b.y0, b.x1, b.y1}}, ci}
	}
	for _, s := range out {
		if s == nil {
			return nil
		}
	}
	return out
}
