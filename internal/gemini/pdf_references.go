package gemini

import (
	"fmt"
	"math"
	"regexp"
	"strings"
)

type layoutPage struct {
	number   int
	geometry pdfPageGeometry
	tables   []pdfTable
}
type creditReference struct {
	amount                        float64
	code, title, evidence, target string
}

var annexHeading = regexp.MustCompile(`(?i)Anlage\s+(\d+)\s*:`)
var annexNumber = regexp.MustCompile(`(?i)Anlage\s+(\d+)`)
var creditReferencePattern = regexp.MustCompile(`(?i)\(?gemäß\s+Anlage\s+(\d+),?\s*Nr\.?\s*(\d+)\)?`)

func readCreditReferences(pages []layoutPage) map[string]creditReference {
	refs := map[string]creditReference{}
	ambiguous := map[string]bool{}
	uniform := map[string]float64{}
	mixed := map[string]bool{}
	for _, pg := range pages {
		for _, table := range pg.tables {
			b := tableBounds(table)
			heading := cleanPDFText(textInBox(pg.geometry.glyphs, &pdfBox{0, math.Max(0, b.y0-95), 2000, b.y0, false, false}))
			h := annexHeading.FindAllStringSubmatch(heading, -1)
			if len(h) == 0 {
				continue
			}
			annex := h[len(h)-1][1]
			if len(table.rows) < 2 {
				continue
			}
			header := table.rows[0]
			lp, title, code := -1, -1, -1
			for i, s := range header {
				v := strings.ToLower(cellText(s))
				if v == "lp" && lp < 0 {
					lp = i
				}
				if strings.Contains(v, "modul") && !strings.Contains(v, "nr") {
					title = i
				}
				if strings.Contains(v, "modul") && strings.Contains(v, "nr") {
					code = i
				}
			}
			if lp < 0 || title < 0 {
				continue
			}
			numbered := strings.Contains(strings.ToLower(cellText(header[0])), "lfd")
			for ri, row := range table.rows {
				if ri == 0 {
					continue
				}
				amount, hi, ok := parseCreditAmount(cellText(row[lp]))
				if !ok || amount != hi || amount <= 0 {
					continue
				}
				name := cellText(row[title])
				if name == "" {
					continue
				}
				r := creditReference{amount: amount, title: name, evidence: fmt.Sprintf("PDF page %d, Anlage %s, row %d: %s — %g LP", pg.number, annex, ri+1, name, amount)}
				if code >= 0 {
					r.code = cellText(row[code])
				}
				if m := annexNumber.FindStringSubmatch(name); m != nil {
					r.target = m[1]
				}
				if numbered {
					n := cellText(row[0])
					if semesterNumber.MatchString(n) {
						key := annex + ":" + strings.TrimSuffix(n, ".")
						if _, found := refs[key]; found {
							ambiguous[key] = true
						}
						refs[key] = r
					}
				}
				if r.code != "" && sourceModuleCode.MatchString(r.code+" x") {
					if uniform[annex] > 0 && uniform[annex] != amount {
						mixed[annex] = true
					}
					uniform[annex] = amount
				}
			}
		}
	}
	for key, r := range refs {
		if ambiguous[key] {
			delete(refs, key)
			continue
		}
		if r.target != "" && uniform[r.target] > 0 && !mixed[r.target] {
			unit := uniform[r.target]
			if math.Mod(r.amount, unit) == 0 {
				r.evidence += fmt.Sprintf("; all numbered modules in Anlage %s have %g LP", r.target, unit)
				r.amount = unit
				r.code = ""
				refs[key] = r
			}
		}
	}
	return refs
}
