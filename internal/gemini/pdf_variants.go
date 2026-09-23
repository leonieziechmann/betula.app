package gemini

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

var statusCell = regexp.MustCompile(`^(?:P|WP|W|Prü|SL|P / Prü|P Prü|WP Prü|WP / Prü|Pflicht|Wahlpflicht)$`)

// dualModeOf is the dual variant a plan name stands for. A name is read from
// its most precise part: „Dual · Dual ausbildungsintegrierend" is the
// ausbildungsintegrierend plan, and a caption that names both variants says
// only that the table belongs to the dual study.
func dualModeOf(name string) string {
	mode := ""
	for _, part := range strings.Split(name, "·") {
		if m := dualMode(part); m != "" {
			mode = m
		}
	}
	return mode
}

func selectProgramMode(l *PDFLayout, hint string) {
	hint = strings.ToLower(hint)
	selectAlternativeBranch(l, dualMode(hint))
	dual := strings.Contains(hint, "dual") || strings.Contains(hint, "praxisintegrier") || strings.Contains(hint, "ausbildungsintegrier")
	hintMode := dualMode(hint)
	extended := strings.Contains(hint, "erweitert") || strings.Contains(hint, "240")
	remove := map[string]bool{}
	hasDual, hasRegular := false, false
	for _, name := range l.PlanNames {
		n := strings.ToLower(name)
		hasDual = hasDual || strings.Contains(n, "dual")
		hasRegular = hasRegular || (!strings.Contains(n, "dual") && !strings.Contains(n, "240 lp") && !strings.Contains(n, "erweitert"))
	}
	lengthOnly := map[string]bool{}
	for id, name := range l.PlanNames {
		n := strings.ToLower(name)
		isDual := strings.Contains(n, "dual")
		isExtended := strings.Contains(n, "240 lp") || strings.Contains(n, "erweitert")
		planMode := dualModeOf(name)
		// Both name one of the two dual modes, and they disagree.
		wrongDualMode := hintMode != "" && planMode != "" && hintMode != "Dual" && planMode != "Dual" && hintMode != planMode
		if (hasDual && hasRegular && isDual != dual) || (strings.Contains(n, "grundlagenorientiert") && extended) || wrongDualMode {
			remove[id] = true
			continue
		}
		// The length of the studies only tells two plans apart where the
		// document still offers another one for this program. A dual plan of
		// 240 LP is not an "extended" variant of anything when it is the only
		// plan of its dual mode.
		if isExtended && !extended {
			lengthOnly[id] = true
		}
	}
	for id := range lengthOnly {
		remaining := 0
		for other := range l.PlanNames {
			if !remove[other] && !lengthOnly[other] {
				remaining++
			}
		}
		if remaining > 0 {
			remove[id] = true
		}
	}
	if len(remove) == 0 {
		return
	}
	cells := l.Cells[:0]
	for _, c := range l.Cells {
		if !remove[c.Table] {
			cells = append(cells, c)
		}
	}
	l.Cells = cells
	totals := l.Totals[:0]
	for _, c := range l.Totals {
		if !remove[c.Table] {
			totals = append(totals, c)
		}
	}
	l.Totals = totals
	for id := range remove {
		delete(l.PlanNames, id)
	}
	issues := l.Issues[:0]
	for _, issue := range l.Issues {
		drop := false
		for id := range remove {
			drop = drop || strings.HasPrefix(issue, id+"r") || strings.HasPrefix(issue, id+":")
		}
		if !drop {
			issues = append(issues, issue)
		}
	}
	l.Issues = issues
}

// Shared first semesters followed by separate regular/dual continuations.
// Project columns without moving coordinates; unselected branches cannot leak
// credits into the other plan.
func splitStudyVariants(t pdfTable) []pdfTable {
	for ri, row := range t.rows {
		if ri > 4 {
			break
		}
		var cols, nums []int
		for ci, s := range row {
			v := cellText(s)
			if semesterNumber.MatchString(v) {
				n, _ := strconv.Atoi(strings.TrimSuffix(v, "."))
				cols = append(cols, ci)
				nums = append(nums, n)
			}
		}
		if len(nums) < 4 || nums[0] != 1 {
			continue
		}
		split := -1
		for i := 1; i < len(nums); i++ {
			if nums[i] <= nums[i-1] {
				split = i
				break
			}
		}
		if split < 2 {
			continue
		}
		valid := true
		for i := 1; i < split; i++ {
			valid = valid && nums[i] == nums[i-1]+1
		}
		for i := split + 1; i < len(nums); i++ {
			valid = valid && nums[i] == nums[i-1]+1
		}
		if !valid {
			continue
		}
		common := nums[split] - 1
		if common < 1 || common >= split {
			continue
		}
		var out []pdfTable
		for v := 0; v < 2; v++ {
			keep := map[int]bool{}
			for i, c := range cols {
				keep[c] = (v == 0 && i < split) || (v == 1 && (i < common || i >= split))
			}
			copy := pdfTable{}
			for r := range t.rows {
				rr := append([]*string(nil), t.rows[r]...)
				bb := append([]*pdfBox(nil), t.boxes[r]...)
				for c, k := range keep {
					if !k {
						rr[c] = nil
						bb[c] = nil
					}
				}
				copy.rows = append(copy.rows, rr)
				copy.boxes = append(copy.boxes, bb)
				copy.origins = append(copy.origins, t.origin(r))
			}
			out = append(out, copy)
		}
		return out
	}
	return nil
}

var inlineLP = regexp.MustCompile(`(?i)\(?\s*(\d+(?:[.,]\d+)?)\s*(?:LP|ECTS|CP)\s*\)?`)

// Semester rows with named boxes and inline credits, e.g. BWL's example plan.
func appendRowStudyTable(l *PDFLayout, t pdfTable, page, number int) {
	if len(t.rows) < 3 {
		return
	}
	head := -1
	for i, row := range t.rows {
		if i > 3 {
			break
		}
		if len(row) >= 3 && semesterHeading.MatchString(strings.ToLower(cellText(row[0]))) && creditHeading.MatchString(strings.ToLower(cellText(row[len(row)-1]))) {
			head = i
			break
		}
	}
	if head < 0 {
		return
	}
	id := fmt.Sprintf("p%dt%drows", page, number)
	var cells, totals []SourceCell
	for ri := head + 1; ri < len(t.rows); ri++ {
		row := t.rows[ri]
		s := cellText(row[0])
		if !semesterNumber.MatchString(s) {
			continue
		}
		sem, _ := strconv.Atoi(strings.TrimSuffix(s, "."))
		lo, hi, ok := parseCreditAmount(cellText(row[len(row)-1]))
		if !ok || lo != hi || sem < 1 || sem > 12 {
			return
		}
		total := SourceCell{ID: fmt.Sprintf("%sr%dtotal", id, ri+1), Table: id, Page: page, RowIndex: ri + 1, Row: "Summe", Semesters: []int{sem}, Raw: cellText(row[len(row)-1]), Min: lo, Max: hi}
		totals = append(totals, total)
		for ci := 1; ci < len(row)-1; ci++ {
			b := t.boxes[ri][ci]
			name := cellText(row[ci])
			if b == nil || name == "" {
				continue
			}
			matches := inlineLP.FindAllStringSubmatch(name, -1)
			if len(matches) != 1 {
				return
			}
			amount, _, ok := parseCreditAmount(matches[0][1])
			if !ok {
				return
			}
			label := strings.TrimSpace(inlineLP.ReplaceAllString(name, ""))
			if label == "" {
				return
			}
			spans := []int{}
			for rj := head + 1; rj < len(t.rows); rj++ {
				rb := t.boxes[rj][0]
				if rb != nil && rb.y0 < b.y1-1 && rb.y1 > b.y0+1 {
					n, _ := strconv.Atoi(strings.TrimSuffix(cellText(t.rows[rj][0]), "."))
					if n > 0 {
						spans = append(spans, n)
					}
				}
			}
			sortInts(spans)
			if len(spans) == 0 {
				return
			}
			cells = append(cells, SourceCell{ID: fmt.Sprintf("%sr%dc%d", id, ri+1, ci+1), Table: id, Page: page, RowIndex: ri + 1, Row: label, Semesters: spans, Raw: name, Min: amount, Max: amount, BBox: []float64{b.x0, b.y0, b.x1, b.y1}})
		}
	}
	if len(totals) >= 2 && len(cells) > 0 {
		l.Cells = append(l.Cells, cells...)
		l.Totals = append(l.Totals, totals...)
	}
}
func sortInts(s []int) {
	for i := 1; i < len(s); i++ {
		for j := i; j > 0 && s[j] < s[j-1]; j-- {
			s[j], s[j-1] = s[j-1], s[j]
		}
	}
}

// selectAlternativeBranch resolves an „entweder … oder …" choice whose branches
// name the dual modes („Praxisintegrierende Studienphase" /
// „Ausbildungsintegrierende duale Studienphase"): this program version is one of
// them, so the branch it names is its compulsory block and the other is not its
// plan at all. Where the hint or the headings do not name a mode the choice
// stays open and both branches remain alternatives, as the source prints them.
func selectAlternativeBranch(l *PDFLayout, hintMode string) {
	if hintMode == "" || hintMode == "Dual" {
		return
	}
	// A group is only resolved where every branch of it names a mode of its own
	// and exactly one of them is this program's.
	modes := map[int]map[int]string{}
	for _, c := range l.Cells {
		if c.AltGroup == 0 || c.AltLabel == "" {
			continue
		}
		if modes[c.AltGroup] == nil {
			modes[c.AltGroup] = map[int]string{}
		}
		modes[c.AltGroup][c.AltIndex] = dualMode(c.AltLabel)
	}
	resolved := map[int]int{}
	for group, branches := range modes {
		keep, named := -1, 0
		for idx, mode := range branches {
			if mode == "" {
				named = -1
				break
			}
			named++
			if mode == hintMode {
				keep = idx
			}
		}
		if named == len(branches) && named > 1 && keep >= 0 {
			resolved[group] = keep
		}
	}
	if len(resolved) == 0 {
		return
	}
	cells := l.Cells[:0]
	for _, c := range l.Cells {
		keep, ok := resolved[c.AltGroup]
		if !ok {
			cells = append(cells, c)
			continue
		}
		if c.AltIndex != keep {
			continue
		}
		// The program version has made the choice, so the block is compulsory.
		c.AltGroup, c.AltIndex = 0, 0
		cells = append(cells, c)
	}
	l.Cells = cells
}

// A regulation may print the plans of several Studienoptionen of one program in
// one document, each in its own Anlage („Anlage 1 … Präsenzstudienprogramm",
// „Anlage 3 … Fernstudienprogramm"). The tables look alike, so only the Anlage
// they stand under says which program version they belong to.
var studyOptions = []struct {
	option   string
	document *regexp.Regexp
	program  *regexp.Regexp
}{
	{"fernstudium", regexp.MustCompile(`(?i)fernstudien|fernstudium`), regexp.MustCompile(`(?i)fernstudi`)},
	{"doppelabschluss", regexp.MustCompile(`(?i)doppelabschluss|double degree`), regexp.MustCompile(`(?i)doppelabschluss|double degree`)},
	{"präsenz", regexp.MustCompile(`(?i)präsenzstudien|präsenzstudium`), nil},
}

// studyOptionOf names the Studienoption of the last „Anlage N:" heading on a
// page, or "" where the page opens no Anlage that names one.
func studyOptionOf(pageText string) string {
	text := cleanPDFText(pageText)
	i := lastAnlageTitle(text)
	if i < 0 {
		return ""
	}
	title := text[i:]
	if len(title) > 200 {
		title = title[:200]
	}
	for _, o := range studyOptions {
		if o.document.MatchString(title) {
			return o.option
		}
	}
	return ""
}

// selectStudyOption keeps the plans of the Studienoption this program version
// is. A degree that names none of the options is the base option the others are
// derived from („Präsenz"), which is how the university names these versions:
// the other two carry their option in the degree itself.
func selectStudyOption(l *PDFLayout, hint string) {
	present := map[string]bool{}
	for _, o := range l.PlanOptions {
		present[o] = true
	}
	if len(present) < 2 {
		return
	}
	want := "präsenz"
	for _, o := range studyOptions {
		if o.program != nil && o.program.MatchString(hint) {
			want = o.option
			break
		}
	}
	// The document prints the plans of several options and none of them is this
	// program's. Keeping another option's plan would store a curriculum this
	// version does not have, so the document is left without one.
	keep := func(cells []SourceCell) []SourceCell {
		out := cells[:0]
		for _, c := range cells {
			if o, known := l.PlanOptions[c.Table]; !known || (present[want] && o == want) {
				out = append(out, c)
			}
		}
		return out
	}
	l.Cells, l.Totals = keep(l.Cells), keep(l.Totals)
	for table, o := range l.PlanOptions {
		if o != want {
			delete(l.PlanNames, table)
		}
	}
	issues := l.Issues[:0]
	for _, issue := range l.Issues {
		drop := false
		for table, o := range l.PlanOptions {
			drop = drop || (o != want && (strings.HasPrefix(issue, table+"r") || strings.HasPrefix(issue, table+":")))
		}
		if !drop {
			issues = append(issues, issue)
		}
	}
	l.Issues = issues
}
