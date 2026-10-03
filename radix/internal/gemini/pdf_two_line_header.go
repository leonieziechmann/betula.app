package gemini

import "strings"

// semesterCaption is the word a column caption uses for the thing its numbers
// count. Narrower than creditSubHeader on purpose: "LP" or "SWS" under a run of
// numbers says what the cells hold, not what the numbers are.
var semesterCaption = map[string]bool{"sem": true, "sem.": true, "semester": true, "fs": true, "fachsemester": true}

// captionedBelow reports a column caption printed on two ruled header rows: a
// run of bare numbers, and under each of them the word they count ("1. 2. 3."
// over "Sem Sem Sem"). Every number must be captioned and nothing else may
// stand in the caption row between them, so the pairing is the document's, not
// the reader's.
func captionedBelow(t pdfTable, ri int, boxes []*pdfBox) bool {
	if len(boxes) < 2 || ri+1 >= len(t.rows) {
		return false
	}
	left, right := boxes[0].x0, boxes[len(boxes)-1].x1
	captioned := 0
	for cj, b := range t.boxes[ri+1] {
		if b == nil || b.x1 <= left+1 || b.x0 >= right-1 {
			continue
		}
		if !semesterCaption[strings.ToLower(cellText(t.rows[ri+1][cj]))] {
			return false
		}
		captioned++
	}
	return captioned == len(boxes)
}

// headerBlock is the x-range the semester columns occupy. A caption cell may be
// narrower than the column under it, the header row being subdivided by hairline
// spacers; the block then reaches over those. Only a spacer is absorbed — an
// empty cell narrower than half the narrowest caption — so an unlabelled column
// beside the block never becomes part of it.
func headerBlock(t pdfTable, hi int, header []*pdfBox) (left, right float64) {
	left, right = header[0].x0, header[len(header)-1].x1
	narrowest := header[0].x1 - header[0].x0
	for _, h := range header {
		if w := h.x1 - h.x0; w < narrowest {
			narrowest = w
		}
	}
	for ci, b := range t.boxes[hi] {
		if b == nil || cellText(t.rows[hi][ci]) != "" || b.x1-b.x0 >= narrowest/2 {
			continue
		}
		if b.x1 > left-1 && b.x1 < left+1 {
			left = b.x0
		}
		if b.x0 > right-1 && b.x0 < right+1 {
			right = b.x1
		}
	}
	return left, right
}
