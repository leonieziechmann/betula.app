package gemini

import (
	"fmt"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

var panelSemester = regexp.MustCompile(`^(\d{1,2})\.\s*Semester$`)
var panelTotal = regexp.MustCompile(`(?i)^LP\s+(\d+(?:[.,]\d+)?)$`)

// Some plans use one miniature module/LP table per semester instead of one
// matrix. Only accept a complete, consecutive set, each with a printed total.
// Full-width rules delimit rows; text background rectangles must not split them.
func appendSemesterPanels(layout *PDFLayout, tables []pdfTable, g pdfPageGeometry, page int) {
	type panel struct {
		semester       int
		label, credits *pdfBox
		total          float64
		bottom         float64
		boxes          [][]*pdfBox
	}
	var panels []panel
	for _, t := range tables {
		if len(t.rows) == 0 {
			continue
		}
		bottom := 0.0
		for _, row := range t.boxes {
			for _, b := range row {
				if b != nil {
					bottom = math.Max(bottom, b.y1)
				}
			}
		}
		for ri := 0; ri < len(t.rows) && ri < 2; ri++ {
			for ci, s := range t.rows[ri] {
				match := panelSemester.FindStringSubmatch(cellText(s))
				if match == nil {
					continue
				}
				n, _ := strconv.Atoi(match[1])
				label := t.boxes[ri][ci]
				if label == nil {
					continue
				}
				for cj := ci + 1; cj < len(t.rows[ri]); cj++ {
					if t.boxes[ri][cj] == nil {
						continue
					}
					m := panelTotal.FindStringSubmatch(cellText(t.rows[ri][cj]))
					if m == nil {
						break
					}
					total, _, ok := parseCreditAmount(m[1])
					if ok {
						panels = append(panels, panel{n, label, t.boxes[ri][cj], total, bottom, t.boxes})
					}
					break
				}
			}
		}
	}
	if len(panels) < 2 {
		return
	}
	sort.Slice(panels, func(i, j int) bool { return panels[i].semester < panels[j].semester })
	for i, p := range panels {
		if p.semester != i+1 {
			return
		}
	}
	edges := snapEdges(append([]pdfEdge(nil), g.edges...))
	id := fmt.Sprintf("p%dpanels", page)
	for _, p := range panels {
		var boundaries []float64
		for _, e := range edges {
			if e.horizontal && e.start <= p.label.x0+geometryTolerance && e.end >= p.credits.x1-geometryTolerance && e.position > p.label.y1+geometryTolerance && e.position <= p.bottom+geometryTolerance {
				boundaries = append(boundaries, e.position)
			}
		}
		sort.Float64s(boundaries)
		previous := p.label.y1
		for ri, y := range boundaries {
			if y-previous < 3 {
				continue
			}
			labelBox := pdfBox{p.label.x0, previous, p.credits.x0, y, false, false}
			// A title may span the category column (e.g. a thesis or project).
			for _, row := range p.boxes {
				for _, b := range row {
					if b != nil && b.x0 < labelBox.x0 && b.x1 > p.label.x0+1 && b.x1 <= p.credits.x0+1 && b.y0 < y-1 && b.y1 > previous+1 {
						labelBox.x0 = b.x0
					}
				}
			}
			creditBox := pdfBox{p.credits.x0, previous, p.credits.x1, y, false, false}
			label := cleanPDFText(textInBox(g.glyphs, &labelBox))
			value := cleanPDFText(textInBox(g.glyphs, &creditBox))
			value = strings.TrimSpace(strings.TrimRight(value, " →\uf0e0"))
			previous = y
			if value == "" && label == "" {
				continue
			}
			lo, hi, ok := parseCreditAmount(value)
			if !ok || label == "" {
				layout.Issues = append(layout.Issues, fmt.Sprintf("%s semester %d row %d: unresolved module/LP cell %q / %q", id, p.semester, ri+1, label, value))
				continue
			}
			layout.Cells = append(layout.Cells, SourceCell{ID: fmt.Sprintf("p%ds%dr%d", page, p.semester, ri+1), Table: id, Page: page, Row: label, Semesters: []int{p.semester}, Raw: value, Min: lo, Max: hi, BBox: []float64{labelBox.x0, labelBox.y0, creditBox.x1, creditBox.y1}})
		}
		layout.Totals = append(layout.Totals, SourceCell{ID: fmt.Sprintf("p%ds%dtotal", page, p.semester), Table: id, Page: page, Row: "Summe", Semesters: []int{p.semester}, Raw: fmt.Sprint(p.total), Min: p.total, Max: p.total, BBox: []float64{p.credits.x0, p.credits.y0, p.credits.x1, p.credits.y1}})
	}
	if layout.PlanNames == nil {
		layout.PlanNames = map[string]string{}
	}
	layout.PlanNames[id] = "Regelstudienplan"
}

func nameStudyPlans(layout *PDFLayout, tables []pdfTable, g pdfPageGeometry, page int) {
	for ti, t := range tables {
		id := fmt.Sprintf("p%dt%d", page, ti+1)
		found := false
		for _, c := range layout.Cells {
			if c.Table == id {
				found = true
				break
			}
		}
		if !found {
			continue
		}
		top := math.Inf(1)
		for _, r := range t.boxes {
			for _, b := range r {
				if b != nil {
					top = math.Min(top, b.y0)
				}
			}
		}
		box := pdfBox{0, math.Max(0, top-95), 2000, top, false, false}
		heading := cleanPDFText(textInBox(g.glyphs, &box))
		name := fmt.Sprintf("Studienplan · Seite %d", page)
		if i := strings.LastIndex(heading, "Regelstudienplan"); i >= 0 {
			name = strings.TrimSpace(strings.TrimRight(heading[i:], ") "))
		}
		// Regular and dual plans are often printed as consecutive Anlagen whose
		// headings both end in "(Regelstudienplan)". The dual mode is named only
		// in the Anlage title itself, so it has to be part of the plan name.
		title := heading
		if i := lastAnlageTitle(heading); i >= 0 {
			title = heading[i:]
		}
		if mode := dualMode(title); mode != "" {
			name = mode + " · " + name
		}
		if layout.PlanNames == nil {
			layout.PlanNames = map[string]string{}
		}
		layout.PlanNames[id] = name
	}
}

// Amendments often repeat part of the same plan before the consolidated text.
// Remove a copy only when every requirement and printed total occurs verbatim
// (apart from whitespace/line wrapping) in the more complete/later table.
func deduplicatePlanTables(layout *PDFLayout) {
	sets := map[string]map[string]int{}
	pages := map[string]int{}
	add := func(c SourceCell, kind string) {
		if sets[c.Table] == nil {
			sets[c.Table] = map[string]int{}
		}
		key := fmt.Sprintf("%s|%s|%v|%g|%g|%v|%d", kind, normalizedTitle(c.Row), c.Semesters, c.Min, c.Max, c.Workload, c.CreditSemester)
		sets[c.Table][key]++
		pages[c.Table] = c.Page
	}
	for _, c := range layout.Cells {
		add(c, "cell")
	}
	for _, c := range layout.Totals {
		add(c, "total")
	}
	drop := map[string]bool{}
	for a, sa := range sets {
		for b, sb := range sets {
			if a == b || len(sa) > len(sb) || (len(sa) == len(sb) && pages[a] >= pages[b]) {
				continue
			}
			subset := true
			for key, count := range sa {
				if sb[key] < count {
					subset = false
					break
				}
			}
			if subset {
				drop[a] = true
				break
			}
		}
	}
	filter := func(cells []SourceCell) []SourceCell {
		out := cells[:0]
		for _, c := range cells {
			if !drop[c.Table] {
				out = append(out, c)
			}
		}
		return out
	}
	layout.Cells = filter(layout.Cells)
	layout.Totals = filter(layout.Totals)
	for table := range drop {
		delete(layout.PlanNames, table)
	}
}

var (
	anlageTitle        = regexp.MustCompile(`Anlage\s*\d+(?:\.\d+)*\s*:`)
	practiceIntegrated = regexp.MustCompile(`(?i)praxis\s*-?\s*integrier`)
	trainingIntegrated = regexp.MustCompile(`(?i)ausbildungs\s*-?\s*integrier`)
	dualStudy          = regexp.MustCompile(`(?i)\bdual`)
)

// lastAnlageTitle returns the start of the last "Anlage N:" title in a heading.
func lastAnlageTitle(heading string) int {
	all := anlageTitle.FindAllStringIndex(heading, -1)
	if len(all) == 0 {
		return -1
	}
	return all[len(all)-1][0]
}

// dualMode names the dual variant announced in a plan title, or "" for a regular
// plan. A title that announces both variants („im dualen praxisintegrierenden und
// im dualen ausbildungsintegrierenden Studium", Elektrotechnik 2022) names the
// dual study as such: the variants are told apart inside the table, not here.
func dualMode(title string) string {
	practice, training := practiceIntegrated.MatchString(title), trainingIntegrated.MatchString(title)
	switch {
	case practice && training:
		return "Dual"
	case practice:
		return "Dual praxisintegrierend"
	case training:
		return "Dual ausbildungsintegrierend"
	case dualStudy.MatchString(title):
		return "Dual"
	}
	return ""
}

// planModePrefix names the dual mode ("Dual praxisintegrierend · ") from the
// Anlage title above a table, or returns "" for a regular plan.
func planModePrefix(t pdfTable, g pdfPageGeometry) string {
	top := math.Inf(1)
	for _, r := range t.boxes {
		for _, b := range r {
			if b != nil {
				top = math.Min(top, b.y0)
			}
		}
	}
	heading := cleanPDFText(textInBox(g.glyphs, &pdfBox{0, math.Max(0, top-95), 2000, top, false, false}))
	title := heading
	if i := lastAnlageTitle(heading); i >= 0 {
		title = heading[i:]
	}
	if mode := dualMode(title); mode != "" {
		return mode + " · "
	}
	return ""
}
