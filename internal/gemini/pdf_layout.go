package gemini

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/ledongthuc/pdf"
)

var (
	creditAmount     = regexp.MustCompile(`^(\d+(?:[.,]\d+)?)(?:\s*[-–−]\s*(\d+(?:[.,]\d+)?))?$`)
	semesterNumber   = regexp.MustCompile(`^\d{1,2}\.?$`)
	semesterHeading  = regexp.MustCompile(`semester|se-\s*mester`)
	creditHeading    = regexp.MustCompile(`\blp\b|\bkp\b|\bcp\b|ects|leistungspunkte|credit`)
	creditUnitSuffix = regexp.MustCompile(`(?i)\s*(?:lp|kp|cp|ects)$`)
	subtotalLabel    = regexp.MustCompile(`(?i)\bsumme\b|\bteilsummen?\b|\btotal\b|\bsubtotal\b|\bgesamt\b|\bgesamtsumme\b|\binsgesamt\b|^arbeitsaufwand\b|\bstudentischer\s+aufwand\b|\baufteilung nach\b|^[Σ∑]?\s*=\s*\d+\s*(?:lp|kp|cp|ects)`)
	winterIntake     = regexp.MustCompile(`studium kann nur im wintersemester|studienbeginn[^.]{0,70}wintersemester|studium (?:beginnt|kann)[^.]{0,80}wintersemester[^.]{0,30}(?:aufgenommen|begonnen)|studienaufnahme[^.]{0,50}wintersemester`)
	summerIntake     = regexp.MustCompile(`studium kann nur im sommersemester|studienbeginn[^.]{0,70}sommersemester|studium (?:beginnt|kann)[^.]{0,80}sommersemester[^.]{0,30}(?:aufgenommen|begonnen)|studienaufnahme[^.]{0,50}sommersemester`)
	workloadAmount   = regexp.MustCompile(`^\((\d+(?:[.,]\d+)?(?:\s*\+\s*\d+(?:[.,]\d+)?)+)\)\s*(\d+(?:[.,]\d+)?)$`)
)

func cleanPDFText(s string) string {
	s = strings.Map(symbolFontRune, s)
	return strings.Join(strings.Fields(strings.ReplaceAll(s, "-\n", "")), " ")
}

// symbolFontRune maps the private-use code points of the Symbol font (used for
// the sum sign in total rows) to their meaning and drops the rest.
func symbolFontRune(r rune) rune {
	if r < 0xF000 || r > 0xF0FF {
		return r
	}
	switch r - 0xF000 {
	case 0x20:
		return ' '
	case 0x53:
		return 'Σ'
	}
	return -1
}
func cellText(s *string) string {
	if s == nil {
		return ""
	}
	return cleanPDFText(*s)
}
func parseCreditAmount(s string) (lo, hi float64, ok bool) {
	s = strings.TrimSpace(strings.TrimRight(s, "⁰¹²³⁴⁵⁶⁷⁸⁹"))
	// The unit may be printed without a separating space ("6LP") and in either case.
	s = strings.TrimSpace(creditUnitSuffix.ReplaceAllString(s, ""))
	if strings.Contains(s, "+") {
		sum := 0.0
		for _, part := range strings.Split(s, "+") {
			a, b, valid := parseCreditAmount(strings.TrimSpace(part))
			if !valid || a != b {
				return 0, 0, false
			}
			sum += a
		}
		return sum, sum, true
	}
	p := creditAmount.FindStringSubmatch(s)
	if p == nil {
		return 0, 0, false
	}
	lo, err := strconv.ParseFloat(strings.ReplaceAll(p[1], ",", "."), 64)
	if err != nil {
		return 0, 0, false
	}
	hi = lo
	if p[2] != "" {
		hi, err = strconv.ParseFloat(strings.ReplaceAll(p[2], ",", "."), 64)
	}
	return lo, hi, err == nil && hi >= lo
}

func extractPDFLayout(ctx context.Context, path string, selectedPages ...int) (layout *PDFLayout, err error) {
	defer func() {
		if r := recover(); r != nil {
			layout = nil
			err = fmt.Errorf("read PDF layout: %v", r)
		}
	}()
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	f, reader, err := pdf.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open PDF: %w", err)
	}
	defer f.Close()
	selected := map[int]bool{}
	for _, p := range selectedPages {
		if p < 1 || p > reader.NumPage() {
			return nil, fmt.Errorf("plan page %d outside PDF", p)
		}
		selected[p] = true
	}
	layout = &PDFLayout{Cells: []SourceCell{}, Totals: []SourceCell{}, Tables: []json.RawMessage{}, Issues: []string{}, StartTerm: "unknown"}
	var fullText strings.Builder
	var pages []layoutPage
	pageTexts := map[int]string{}
	for pageNo := 1; pageNo <= reader.NumPage(); pageNo++ {
		if len(selected) > 0 && !selected[pageNo] {
			continue
		}
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		geometry, err := readPageGeometry(ctx, reader.Page(pageNo))
		if err != nil {
			return nil, fmt.Errorf("PDF page %d: %w", pageNo, err)
		}
		pageTexts[pageNo] = cleanPDFText(textInBox(geometry.glyphs, nil))
		fullText.WriteString(textInBox(geometry.glyphs, nil))
		fullText.WriteByte('\n')
		tables, err := tableGeometry(ctx, geometry.edges, geometry.glyphs)
		if err != nil {
			return nil, fmt.Errorf("PDF page %d: %w", pageNo, err)
		}
		annotateCellStyles(tables, geometry)
		tables = mergeStudyFragments(tables, geometry)
		if n := len(tables); n > 0 {
			tables[n-1] = withTailRows(tables[n-1], geometry)
		}
		pages = append(pages, layoutPage{pageNo, geometry, tables})
	}
	mergeContinuedTables(pages)
	refs := readCreditReferences(pages)
	section := ""
	option := ""
	for _, pg := range pages {
		pageNo, geometry, tables := pg.number, pg.geometry, pg.tables
		if o := studyOptionOf(textInBox(geometry.glyphs, nil)); o != "" {
			option = o
		}
		firstCell, firstTotal := len(layout.Cells), len(layout.Totals)
		for ti, t := range tables {
			pageText := textInBox(geometry.glyphs, nil)
			isPlan := strings.Contains(strings.ToLower(pageText), "studienplan") || strings.Contains(strings.ToLower(pageText), "studienpläne") || strings.Contains(strings.ToLower(pageText), "studienverlaufsplan")
			workload := strings.Contains(pageText, "Arbeitsaufwand") && (strings.Contains(pageText, "Gutschrift") || strings.Contains(pageText, "Anrechnung"))
			if appendTrackVariants(layout, t, pageNo, ti+1, workload, isPlan, planModePrefix(t, geometry)) {
				continue
			}
			variants := splitStudyVariants(t)
			if len(variants) > 0 {
				for vi, v := range variants {
					appendStudyTable(layout, v, pageNo, (ti+1)*100+vi+1, workload, isPlan)
					if layout.PlanNames == nil {
						layout.PlanNames = map[string]string{}
					}
					name := "Grundständig"
					if vi == 1 {
						name = "Dual praxisintegrierend"
					}
					layout.PlanNames[fmt.Sprintf("p%dt%d", pageNo, (ti+1)*100+vi+1)] = name
				}
			} else {
				appendStudyTable(layout, t, pageNo, ti+1, workload, isPlan)
			}
			appendRowStudyTable(layout, t, pageNo, ti+1)
			appendBoxStudyTable(layout, t, pageNo, ti+1, refs)
		}
		appendSemesterPanels(layout, tables, geometry, pageNo)
		if c := planSectionCaption.FindAllString(cleanPDFText(textInBox(geometry.glyphs, nil)), -1); len(c) > 0 {
			section = c[len(c)-1]
		}
		nameStudyPlans(layout, tables, geometry, pageNo, section)
		if option != "" {
			if layout.PlanOptions == nil {
				layout.PlanOptions = map[string]string{}
			}
			for _, c := range layout.Cells[firstCell:] {
				layout.PlanOptions[c.Table] = option
			}
			for _, c := range layout.Totals[firstTotal:] {
				layout.PlanOptions[c.Table] = option
			}
		}
	}
	resolveAnnexReferences(layout, pages)
	text := strings.ToLower(cleanPDFText(fullText.String()))
	deduplicatePlanTables(layout)
	applyChoiceFootnotes(layout, pageTexts)
	applyStyleLegends(layout, strings.ToLower(cleanPDFText(fullText.String())))
	winter, summer := winterIntake.MatchString(text), summerIntake.MatchString(text)
	if winter && !summer {
		layout.StartTerm = "winter"
	} else if summer && !winter {
		layout.StartTerm = "summer"
	}
	return layout, nil
}

func appendStudyTable(layout *PDFLayout, t pdfTable, pageNo, tableNo int, workloadNotation ...bool) {
	hi := -1
	var header []*pdfBox
	headerFrom, headerTo := 0, 0
	headerTerm := ""
	var headerCaptions []string
	// Two passes: a table headed by plain semester numbers is read exactly as
	// before, and only one that has no such header at all is offered the reading
	// that allows a site label beside the number.
	for _, withSite := range []bool{false, true} {
		if hi >= 0 {
			break
		}
		var prefix strings.Builder
		for ri := 0; ri < len(t.rows) && ri < 5; ri++ {
			for _, s := range t.rows[ri] {
				prefix.WriteString(cellText(s))
				prefix.WriteByte(' ')
			}
			p := strings.ToLower(prefix.String())
			var boxes []*pdfBox
			var captions []string
			valid, explicit, named := true, 0, 0
			first, last := -1, -1
			term := ""
			for ci, s := range t.rows[ri] {
				n, isExplicit, season, caption, ok := parseSemesterHeaderSite(cellText(s), withSite)
				if !ok {
					continue
				}
				if n != len(boxes)+1 || t.boxes[ri][ci] == nil {
					valid = false
					break
				}
				if isExplicit {
					explicit++
				}
				if n == 1 {
					term = season
				}
				if first < 0 {
					first = ci
				}
				last = ci
				captions = append(captions, caption)
				if caption != "" {
					named++
				}
				boxes = append(boxes, t.boxes[ri][ci])
			}
			// A caption is only kept where the plan gives every semester column
			// one: a single labelled column among bare numbers names a column
			// that is not a semester at all.
			if named != len(boxes) {
				captions = nil
			}
			headingOK := semesterHeading.MatchString(p) || explicit >= 2 || (len(workloadNotation) > 1 && workloadNotation[1] && (creditHeading.MatchString(p) || subtotalLabel.MatchString(p))) || captionedBelow(t, ri, boxes)
			if headingOK && valid && len(boxes) >= 2 {
				hi = ri
				header = boxes
				headerCaptions = captions
				headerFrom, headerTo, headerTerm = first, last, term
				break
			}
		}
	}
	if hi < 0 {
		return
	}
	if headerTerm != "" && layout.StartTerm == "unknown" {
		layout.StartTerm = headerTerm
	}
	id := fmt.Sprintf("p%dt%d", pageNo, tableNo)
	firstNew := len(layout.Cells)
	// A table whose "semester" boxes only hold module names (one panel per
	// semester) is read by appendSemesterPanels; leave no trace of this attempt
	// when it found no credit values at all.
	startTotals, startIssues, startTables := len(layout.Totals), len(layout.Issues), len(layout.Tables)
	startRefs := len(layout.annexRefs)
	textIssues := 0
	defer func() {
		accepted := len(layout.Cells) - firstNew
		if (accepted == 0 && len(layout.Totals) == startTotals) || (textIssues > 0 && textIssues >= accepted) {
			layout.Cells, layout.Totals = layout.Cells[:firstNew], layout.Totals[:startTotals]
			layout.Issues = layout.Issues[:startIssues]
			layout.Tables = layout.Tables[:startTables]
			layout.annexRefs = layout.annexRefs[:startRefs]
		}
	}()
	data, _ := json.Marshal(struct {
		ID   string      `json:"id"`
		Page int         `json:"page"`
		Rows [][]*string `json:"rows"`
	}{id, pageNo, t.rows})
	layout.Tables = append(layout.Tables, data)
	left, right := headerBlock(t, hi, header)
	alternatives, branchOf := alternativeRows(t, hi, left, right)
	for ri := hi + 1; ri < len(t.rows); ri++ {
		if isCreditSubHeader(t.rows[ri], headerFrom, headerTo) {
			continue
		}
		for ci, b := range t.boxes[ri] {
			if b == nil || b.x0 < left-1 {
				continue
			}
			var semesters []int
			for i, h := range header {
				mid := (h.x0 + h.x1) / 2
				if b.x0 <= mid && mid <= b.x1 {
					semesters = append(semesters, i+1)
				}
			}
			value := cellText(t.rows[ri][ci])
			if len(semesters) == 0 || value == "" || value == "-" || value == "–" {
				continue
			}
			// A merged block that names another Anlage instead of printing
			// credits: it is resolved once every page has been read.
			if ref, isRef := parseAnnexReference(value, semesters); isRef {
				origin := t.origin(ri)
				ref.table, ref.cell = id, fmt.Sprintf("%sr%dc%d", id, ri+1, ci+1)
				ref.page, ref.rowIndex = pageNo, ri+1
				ref.bbox = []float64{b.x0 + origin.dx, b.y0 + origin.dy, b.x1 + origin.dx, b.y1 + origin.dy}
				layout.annexRefs = append(layout.annexRefs, ref)
				continue
			}
			var labels, statusOnly []string
			seen := map[string]bool{}
			for rj := hi + 1; rj < len(t.rows); rj++ {
				var pieces []string
				for cj, rb := range t.boxes[rj] {
					if rb == nil || rb.y0 >= b.y1-1 || rb.y1 <= b.y0+1 {
						continue
					}
					piece := cellText(t.rows[rj][cj])
					// Some plans print the status column to the right of the
					// semester block instead of next to the module name.
					if rb.x0 >= right-1 {
						if statusCell.MatchString(piece) {
							statusOnly = append(statusOnly, piece)
						}
						continue
					}
					if rb.x1 > left+1 {
						continue
					}
					if statusCell.MatchString(piece) {
						statusOnly = append(statusOnly, piece)
						continue
					}
					pieces = append(pieces, piece)
				}
				label := cleanPDFText(strings.Join(pieces, " "))
				if label != "" && !seen[label] {
					labels = append(labels, label)
					seen[label] = true
				}
			}
			// "Wahlpflicht" is a status marker next to a module name, but on its own
			// it is the name of an elective budget row with its own credits. Dropping
			// it would silently remove those credits from the plan.
			if len(labels) == 0 && len(statusOnly) == 1 {
				labels = statusOnly
			}
			label := strings.Join(labels, " / ")
			if label == "" {
				// Semester columns may print ranges ("28 - 32"), so the row is
				// verified as an interval: the grand total has to be reachable.
				sumLo, sumHi, count := 0.0, 0.0, 0
				rightLo, rightHi := math.NaN(), math.NaN()
				for cj, rb := range t.boxes[ri] {
					if rb == nil {
						continue
					}
					clo, chi, ok := parseCreditAmount(cellText(t.rows[ri][cj]))
					if !ok {
						continue
					}
					if rb.x0 >= right-1 {
						rightLo, rightHi = clo, chi
					} else if rb.x0 >= left-1 {
						sumLo += clo
						sumHi += chi
						count++
					}
				}
				if count > 0 && rightLo <= sumHi+0.01 && rightHi >= sumLo-0.01 {
					label = "Subtotal"
					// An unlabeled final row with every semester and a matching
					// grand-total column is a verifiable whole-plan total.
					if ri == len(t.rows)-1 && count == len(header) {
						label = "Summe"
					}
				} else {
					// A cell without a label in the row's name column is also what a
					// semester panel looks like to the ruled reader, so this counts
					// towards discarding this reading in favour of the panel one.
					if prose.MatchString(value) {
						textIssues++
					}
					layout.Issues = append(layout.Issues, fmt.Sprintf("%sr%d: numeric row without a module label or verifiable subtotal", id, ri+1))
					continue
				}
			}
			lo, hi, ok := parseCreditAmount(value)
			if !ok {
				if sum, joined := joinedCreditAmounts(value); joined {
					lo, hi, ok = sum, sum, true
				}
			}
			var workload []float64
			creditSemester := 0
			if !ok {
				if parts := workloadParts(value); parts != nil {
					total, _, valid := parseCreditAmount(parts[2])
					sum := 0.0
					for _, piece := range strings.Split(parts[1], "+") {
						v, _, yes := parseCreditAmount(strings.TrimSpace(piece))
						valid = valid && yes
						workload = append(workload, v)
						sum += v
					}
					if valid && math.Abs(sum-total) < 0.01 {
						lo, hi, ok = total, total, true
						if len(workload) == len(semesters) && len(workloadNotation) > 0 && workloadNotation[0] {
							creditSemester = semesters[len(semesters)-1]
						} else {
							workload = nil
						}
					} else {
						workload = nil
					}
				}
			}
			optional, marked, additional := false, false, false
			if !ok {
				if v, yes := additionalCredit(value); yes {
					lo, hi, ok, additional = v, v, true, true
				} else if v, yes := creditBeforeRemark(value); yes {
					lo, hi, ok = v, v, true
				} else if v, yes := optionalPlacement(value); yes {
					lo, hi, ok, optional = v, v, true, true
				} else if v, opt, yes := markedCredit(value); yes {
					lo, hi, ok, optional, marked = v, v, true, opt, true
				}
			}
			if !ok {
				if prose.MatchString(value) {
					textIssues++
				}
				layout.Issues = append(layout.Issues, fmt.Sprintf("%sr%dc%d: unsupported semester cell %q", id, ri+1, ci+1, value))
				continue
			}
			origin := t.origin(ri)
			cellPage := pageNo
			if origin.page != 0 {
				cellPage = origin.page
			}
			c := SourceCell{ID: fmt.Sprintf("%sr%dc%d", id, ri+1, ci+1), Table: id, Page: cellPage, RowIndex: ri + 1, Row: label, Semesters: semesters, Raw: value, Min: lo, Max: hi, BBox: []float64{b.x0 + origin.dx, b.y0 + origin.dy, b.x1 + origin.dx, b.y1 + origin.dy}, SharedRows: len(labels) > 1}
			c.Workload, c.CreditSemester = workload, creditSemester
			c.Optional, c.Additional = optional, additional
			c.Elective = electiveStatus(statusOnly)
			if alt, ok := alternatives[ri]; ok {
				c.AltGroup, c.AltIndex, c.AltLabel = alt[0], alt[1], branchOf[ri]
			}
			c.Bold, c.Shaded = b.bold, b.shaded
			if optional && b.shaded {
				c.PlanSemester = semesters[0]
			}
			if marked {
				c.Marker = "+"
			}
			// The caption of the semester column („ECN", „BTU") names where that
			// semester is spent. A cell over several semesters belongs to no
			// single caption, so it keeps none.
			if len(semesters) == 1 && semesters[0] <= len(headerCaptions) {
				c.Section = headerCaptions[semesters[0]-1]
			}
			if subtotalLabel.MatchString(label) {
				layout.Totals = append(layout.Totals, c)
			} else if hi > 0 {
				layout.Cells = append(layout.Cells, c)
			}
		}
	}
	mergeOptionalPlacements(layout, firstNew)
}

// mergeOptionalPlacements joins the bracketed cells "(6) (6)" of one row into a
// single credit that may fall into any of the listed semesters, so the module
// is counted once and never pinned to a semester the plan does not name.
var prose = regexp.MustCompile(`[A-Za-zÄÖÜäöüß]{4,}`)

func mergeOptionalPlacements(layout *PDFLayout, from int) {
	var out []SourceCell
	out = append(out, layout.Cells[:from]...)
	index := map[string]int{}
	for _, c := range layout.Cells[from:] {
		// "(6)" repeated across semesters names one module whose placement is
		// open. "(6)+(6)" already states a quantity for this semester, so two
		// such cells are two obligations and must stay apart.
		if !c.Optional || strings.Contains(c.Raw, "+") {
			out = append(out, c)
			continue
		}
		key := fmt.Sprintf("%s|%s|%.1f|%.0f", c.Table, c.Row, c.Min, c.BBox[1])
		i, seen := index[key]
		if !seen {
			index[key] = len(out)
			out = append(out, c)
			continue
		}
		m := &out[i]
		if c.PlanSemester > 0 {
			if m.PlanSemester > 0 {
				m.PlanSemester = -1 // two grey cells: the plan does not choose one
			} else if m.PlanSemester == 0 {
				m.PlanSemester = c.PlanSemester
			}
		}
		m.Shaded = m.Shaded || c.Shaded
		m.Bold = m.Bold && c.Bold
		m.Semesters = append(m.Semesters, c.Semesters...)
		sort.Ints(m.Semesters)
		lo, hi := m.Semesters[0], m.Semesters[len(m.Semesters)-1]
		m.Semesters = m.Semesters[:0]
		for s := lo; s <= hi; s++ {
			m.Semesters = append(m.Semesters, s)
		}
		m.Raw += " " + c.Raw
		m.BBox[0], m.BBox[2] = math.Min(m.BBox[0], c.BBox[0]), math.Max(m.BBox[2], c.BBox[2])
	}
	for i := range out {
		if out[i].PlanSemester < 0 {
			out[i].PlanSemester = 0
		}
	}
	layout.Cells = out
}

var (
	boldMeansRequired = regexp.MustCompile(`fett\s+geschriebene\s+lp-zahlen\s+sind\s+pflichtmodul`)
	// The legend often sits in a two-column text block, so words of the other
	// column may be interleaved after the hyphenated "ange-".
	greyMeansPlan = regexp.MustCompile(`grau\s+ange-?.{0,400}?legte\s+zellen\s+stellen\s+einen\s+möglichen\s+studienplan|grau\s+angelegte\s+zellen\s+stellen\s+einen\s+möglichen\s+studienplan`)
)

// applyStyleLegends gives bold and grey cells their meaning when the regulation
// says so in its legend. Without the legend the styles stay plain observations.
func applyStyleLegends(layout *PDFLayout, text string) {
	bold, grey := boldMeansRequired.MatchString(text), greyMeansPlan.MatchString(text)
	if !bold && !grey {
		return
	}
	hasBold, hasShaded := map[string]bool{}, map[string]bool{}
	for _, c := range layout.Cells {
		hasBold[c.Table] = hasBold[c.Table] || c.Bold
		hasShaded[c.Table] = hasShaded[c.Table] || c.Shaded
	}
	for i := range layout.Cells {
		c := &layout.Cells[i]
		if bold && hasBold[c.Table] {
			c.Elective = !c.Bold
		}
		if grey && hasShaded[c.Table] {
			c.InPlan = c.Shaded
		}
	}
}

func isOderRow(row []*string) bool { return isChoiceRow(row, "oder", "or") }

// isEntwederRow opens an explicit choice: a line whose only word is „entweder".
func isEntwederRow(row []*string) bool { return isChoiceRow(row, "entweder") }

func isChoiceRow(row []*string, words ...string) bool {
	seen := false
	for _, s := range row {
		t := strings.ToLower(cellText(s))
		if t == "" {
			continue
		}
		ok := false
		for _, w := range words {
			ok = ok || t == w
		}
		if !ok {
			return false
		}
		seen = true
	}
	return seen
}

// alternativeRows numbers the module rows on both sides of an "oder" line:
// group id and alternative index (0 for the first option). The options extend
// to the neighboring section header, total line or next "oder".
func alternativeRows(t pdfTable, header int, left, right float64) (map[int][2]int, map[int]string) {
	out, labels := map[int][2]int{}, map[int]string{}
	group := 0
	claimed := entwederBlocks(t, header, left, right, out, labels, &group)
	for ro, row := range t.rows {
		if ro <= header || !isOderRow(row) || claimed[ro] {
			continue
		}
		g, idx := 0, 0
		if prev, ok := out[ro-1]; ok {
			g, idx = prev[0], prev[1]
		} else {
			group++
			g = group
			for r := ro - 1; r > header && !alternativeBreak(t.rows[r]) && !isOderRow(t.rows[r]); r-- {
				out[r] = [2]int{g, 0}
			}
		}
		for r := ro + 1; r < len(t.rows) && !alternativeBreak(t.rows[r]) && !isOderRow(t.rows[r]); r++ {
			out[r] = [2]int{g, idx + 1}
		}
	}
	return out, labels
}

// alternativeBreak ends a chain of "oder" alternatives. Besides a section row
// with its own group total and a totals line, any heading that carries a label
// but no credit value starts a new group of requirements: without this, the
// alternatives of every later block would be counted as alternatives of the
// first one and their credits would drop out of the plan.
func alternativeBreak(row []*string) bool {
	if len(row) == 0 {
		return true
	}
	if isSectionRow(row) || subtotalLabel.MatchString(cellText(row[0])) {
		return true
	}
	label, credit := false, false
	for _, s := range row {
		text := cellText(s)
		if text == "" {
			continue
		}
		if _, _, ok := parseCreditAmount(text); ok {
			credit = true
		} else {
			label = true
		}
	}
	return label && !credit
}

// electiveStatus reads the plan's own status column ("P", "WP"). A plan that
// lists every elective as its own row uses it to separate the compulsory
// modules that its semester sums count from the elective offer.
func electiveStatus(markers []string) bool {
	for _, m := range markers {
		switch strings.ToLower(strings.TrimSpace(m)) {
		case "wp", "w", "wahlpflicht", "wp / prü", "wp prü":
			return true
		}
	}
	return false
}

// entwederBlocks numbers the branches of an „entweder … oder …" choice whose
// branches are blocks of their own: a heading carrying the branch's printed sum
// and the module rows under it. Such a heading ends a bare „oder" chain, so the
// choice is read from its opening „entweder" line instead — and only where the
// source proves where each branch ends: the branch's rows have to reach the sum
// its own heading prints.
func entwederBlocks(t pdfTable, header int, left, right float64, out map[int][2]int, labels map[int]string, group *int) map[int]bool {
	claimed := map[int]bool{}
	for ro := header + 1; ro < len(t.rows); ro++ {
		if !isEntwederRow(t.rows[ro]) {
			continue
		}
		marks, named := map[int][2]int{}, map[int]string{}
		g, branch, r, ok := *group+1, 0, ro+1, true
		for r < len(t.rows) {
			head, label := -1, ""
			if isSectionRow(t.rows[r]) {
				head, label, r = r, cellText(t.rows[r][0]), r+1
			}
			from := r
			for r < len(t.rows) && !isOderRow(t.rows[r]) && !alternativeBreak(t.rows[r]) {
				r++
			}
			if from == r || (head >= 0 && !blockReachesItsSum(t, head, from, r, left, right)) {
				ok = false
				break
			}
			for i := from; i < r; i++ {
				marks[i], named[i] = [2]int{g, branch}, label
			}
			if r < len(t.rows) && isOderRow(t.rows[r]) {
				branch, r = branch+1, r+1
				continue
			}
			break
		}
		if !ok || branch == 0 {
			continue
		}
		*group = g
		for k, v := range marks {
			out[k], labels[k] = v, named[k]
		}
		for i := ro; i < r; i++ {
			claimed[i] = true
		}
		ro = r - 1
	}
	return claimed
}

// blockReachesItsSum reports whether the semester cells of rows [from,to) come
// to what the block's heading prints in its total column.
func blockReachesItsSum(t pdfTable, head, from, to int, left, right float64) bool {
	printed, _, ok := parseCreditAmount(cellText(t.rows[head][len(t.rows[head])-1]))
	if !ok {
		return false
	}
	sum := 0.0
	for ri := from; ri < to; ri++ {
		for ci, b := range t.boxes[ri] {
			if b == nil || b.x0 < left-1 || b.x0 >= right-1 {
				continue
			}
			if v, _, ok := parseCreditAmount(cellText(t.rows[ri][ci])); ok {
				sum += v
			}
		}
	}
	return math.Abs(sum-printed) < 0.01
}
