package gemini

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

// A Lesefassung may print the same Regelstudienplan several times, once per
// study branch, and replace a whole block of semester columns with a single
// merged cell that points at another Anlage: „1. bis 5. Fachsemester analog zu
// Anlage 2.1", „8. Fachsemester analog zu 6. Fachsemester gemäß Anlage 2.1".
//
// Following such a cell invents nothing. The referenced Anlage prints those
// semesters in full, in the same document, and the referencing plan's own
// „Σ = 180 LP" line verifies the copy semester by semester — a copy whose
// semesters do not reach what this plan prints over them is refused.
var annexPlanReference = regexp.MustCompile(`(?i)^(\d{1,2})\.(?:\s*bis\s*(\d{1,2})\.)?\s*Fachsemester\s+analog\s+zu\s+(?:(\d{1,2})\.\s*Fachsemester\s+(?:gemäß|laut|nach)\s+)?Anlage\s+(\d+(?:\.\d+)*)\s*$`)

// annexReference is one such merged cell: the semesters of this plan it stands
// for, the semester of the referenced Anlage they start at, and where it was
// printed.
type annexReference struct {
	table, cell, annex string
	page, rowIndex     int
	bbox               []float64
	from, to           int // semesters of this plan
	sourceFrom         int // the first semester in the referenced Anlage
	raw                string
}

// parseAnnexReference reads the cell's text. semesters is the column span the
// cell physically covers; text and geometry have to agree, otherwise the cell
// is not understood and stays an issue.
func parseAnnexReference(value string, semesters []int) (annexReference, bool) {
	m := annexPlanReference.FindStringSubmatch(strings.TrimSpace(value))
	if m == nil {
		return annexReference{}, false
	}
	from, _ := strconv.Atoi(m[1])
	to := from
	if m[2] != "" {
		to, _ = strconv.Atoi(m[2])
	}
	sourceFrom := from
	if m[3] != "" {
		sourceFrom, _ = strconv.Atoi(m[3])
	}
	if to < from || len(semesters) != to-from+1 || semesters[0] != from || semesters[len(semesters)-1] != to {
		return annexReference{}, false
	}
	return annexReference{annex: m[4], from: from, to: to, sourceFrom: sourceFrom, raw: strings.TrimSpace(value)}, true
}

// annexTables indexes the study tables of a document by the Anlage number
// printed above them („2.1" -> „p9t1").
var annexLabel = regexp.MustCompile(`(?i)Anlage\s+(\d+(?:\.\d+)*)`)

func annexTables(pages []layoutPage) map[string]string {
	out, ambiguous := map[string]string{}, map[string]bool{}
	for _, pg := range pages {
		for ti, t := range pg.tables {
			b := tableBounds(t)
			// The band is defined by the table, not by the page origin: clamping
			// it to zero inverts the box where a table sits at a negative
			// coordinate, and an inverted box silently holds nothing.
			heading := cleanPDFText(textInBox(pg.geometry.glyphs, &pdfBox{0, b.y0 - 95, 2000, b.y0, false, false}))
			all := annexLabel.FindAllStringSubmatch(heading, -1)
			if len(all) == 0 {
				continue
			}
			number := all[len(all)-1][1]
			id := fmt.Sprintf("p%dt%d", pg.number, ti+1)
			if _, seen := out[number]; seen {
				ambiguous[number] = true
			}
			out[number] = id
		}
	}
	for number := range ambiguous {
		delete(out, number)
	}
	return out
}

// resolveAnnexReferences copies the referenced semesters into the referencing
// plan. Each copied cell keeps the row label and credits the referenced Anlage
// printed, and says where they were read.
func resolveAnnexReferences(layout *PDFLayout, pages []layoutPage) {
	if len(layout.annexRefs) == 0 {
		return
	}
	index := annexTables(pages)
	for _, ref := range layout.annexRefs {
		source, ok := index[ref.annex]
		if !ok || source == ref.table {
			layout.Issues = append(layout.Issues, fmt.Sprintf("%s: %q names an Anlage this document does not print as a plan", ref.cell, ref.raw))
			continue
		}
		offset := ref.from - ref.sourceFrom
		sourceTo := ref.sourceFrom + (ref.to - ref.from)
		var copied []SourceCell
		for _, c := range layout.Cells {
			if c.Table != source || len(c.Semesters) == 0 {
				continue
			}
			if c.Semesters[0] < ref.sourceFrom || c.Semesters[len(c.Semesters)-1] > sourceTo {
				continue
			}
			n := c
			n.ID = fmt.Sprintf("%s<%s", c.ID, ref.cell)
			n.Table, n.Page, n.RowIndex = ref.table, ref.page, ref.rowIndex
			n.BBox = append([]float64(nil), ref.bbox...)
			n.Semesters = nil
			for _, s := range c.Semesters {
				n.Semesters = append(n.Semesters, s+offset)
			}
			n.CreditEvidence = fmt.Sprintf("%s (PDF page %d, cell %s: %s — %s LP)", ref.raw, c.Page, c.ID, c.Row, c.Raw)
			copied = append(copied, n)
		}
		if len(copied) == 0 {
			layout.Issues = append(layout.Issues, fmt.Sprintf("%s: %q — Anlage %s prints no requirement in semester %d", ref.cell, ref.raw, ref.annex, ref.sourceFrom))
			continue
		}
		layout.Cells = append(layout.Cells, copied...)
	}
	layout.annexRefs = nil
}
