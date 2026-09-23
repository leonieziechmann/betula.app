package gemini

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
)

// chooseOneFootnote is a footnote that says exactly one of the blocks it marks
// is taken: „* siehe § 6 (3) – Ein Schwerpunkt ist zu belegen." The captured
// noun is the word the sub-headings of those blocks begin with.
var chooseOneFootnote = regexp.MustCompile(`(?i)(?:^|\s)[*⁎]\s*[^*]{0,80}?\bein(?:er)?\s+(?:der\s+(?:beiden\s+)?)?([A-Za-zÄÖÜäöüß]{4,})\s+ist\s+zu\s+(?:belegen|wählen|absolvieren)`)

// footnoteMark ends a label that refers to a footnote („Schwerpunktbereich*").
var footnoteMark = regexp.MustCompile(`[*⁎]\s*$`)

type tableRows struct {
	ID   string      `json:"id"`
	Page int         `json:"page"`
	Rows [][]*string `json:"rows"`
}

// headingLabel is the text of a heading row inside a plan table: exactly one
// non-empty cell, and not a number. Which column it sits in is left open --
// a merged heading cell does not always begin in the label column.
func headingLabel(row []*string) string {
	label := ""
	for _, s := range row {
		text := cellText(s)
		if text == "" {
			continue
		}
		if label != "" {
			return ""
		}
		label = text
	}
	if _, _, ok := parseCreditAmount(label); ok {
		return ""
	}
	return label
}

// applyChoiceFootnotes marks the alternative blocks of a plan that prints every
// specialization one under the other and says, in a footnote to the section
// they sit in, that one of them is taken. The blocks are only accepted as
// alternatives where they are interchangeable — same credits in the same
// semesters — so which one a student picks cannot change what the plan's own
// semester sums have to add up to. Where they differ the plan does not say how
// the choice moves its credits and nothing is marked.
func applyChoiceFootnotes(layout *PDFLayout, pageText map[int]string) {
	group := 0
	for _, raw := range layout.Tables {
		var t tableRows
		if json.Unmarshal(raw, &t) != nil {
			continue
		}
		m := chooseOneFootnote.FindStringSubmatch(pageText[t.Page])
		if m == nil {
			continue
		}
		noun := strings.ToLower(m[1])
		for ri, row := range t.Rows {
			if !footnoteMark.MatchString(headingLabel(row)) {
				continue
			}
			// The marked section row is followed by the blocks themselves, each
			// opened by a heading that begins with the footnote's noun.
			var starts []int
			for rj := ri + 1; rj < len(t.Rows); rj++ {
				label := headingLabel(t.Rows[rj])
				if label == "" {
					continue
				}
				if strings.HasPrefix(strings.ToLower(label), noun+" ") {
					starts = append(starts, rj)
					continue
				}
				if len(starts) > 0 {
					break // the section is over: a heading that names something else
				}
			}
			if len(starts) < 2 {
				continue
			}
			// The tracks are printed as parallel blocks, so the gap between the
			// first two headings is how many rows a block has. That also says
			// where the last one ends, which nothing below it would.
			size := starts[1] - starts[0] - 1
			blocks := make([][]int, len(starts))
			ok := size > 0
			for i, s := range starts {
				if s+size >= len(t.Rows) || (i > 0 && s != starts[i-1]+size+1) {
					ok = false
					break
				}
				for r := s + 1; r <= s+size; r++ {
					// A block holds module rows only: a heading or a printed sum
					// inside it means the blocks are not parallel after all.
					if headingLabel(t.Rows[r]) != "" || subtotalLabel.MatchString(rowText(t.Rows[r])) {
						ok = false
						break
					}
					blocks[i] = append(blocks[i], r+1) // cells count rows from 1
				}
			}
			if !ok {
				continue
			}
			// A refusal must leave the document exactly as it reads today: an
			// Issue would reject the whole PDF, so this is only a note.
			if !interchangeable(layout, t.ID, blocks) {
				layout.Notes = append(layout.Notes, fmt.Sprintf("%sr%d: the %q blocks differ in credits; the plan does not say how the choice is placed, so every block is still counted", t.ID, ri+1, noun))
				continue
			}
			group++
			for i, rows := range blocks {
				want := map[int]bool{}
				for _, r := range rows {
					want[r] = true
				}
				track := headingLabel(t.Rows[starts[i]])
				for ci := range layout.Cells {
					if c := &layout.Cells[ci]; c.Table == t.ID && want[c.RowIndex] {
						c.AltGroup, c.AltIndex, c.Track = group, i, track
					}
				}
			}
		}
	}
}

// interchangeable reports whether every block carries the same credits in the
// same semesters, so that counting any one of them gives the same plan.
func interchangeable(layout *PDFLayout, table string, blocks [][]int) bool {
	var first []string
	for i, rows := range blocks {
		want := map[int]bool{}
		for _, r := range rows {
			want[r] = true
		}
		var shape []string
		for _, c := range layout.Cells {
			if c.Table == table && want[c.RowIndex] {
				shape = append(shape, fmt.Sprintf("%v|%g|%g", c.Semesters, c.Min, c.Max))
			}
		}
		if len(shape) == 0 {
			return false
		}
		sortStrings(shape)
		if i == 0 {
			first = shape
			continue
		}
		if strings.Join(shape, ";") != strings.Join(first, ";") {
			return false
		}
	}
	return true
}

// rowText joins a row for label matching.
func rowText(row []*string) string {
	var parts []string
	for _, s := range row {
		if text := cellText(s); text != "" {
			parts = append(parts, text)
		}
	}
	return strings.Join(parts, " ")
}

func sortStrings(s []string) {
	for i := 1; i < len(s); i++ {
		for j := i; j > 0 && s[j] < s[j-1]; j-- {
			s[j], s[j-1] = s[j-1], s[j]
		}
	}
}
