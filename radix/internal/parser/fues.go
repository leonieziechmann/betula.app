package parser

import (
	"fmt"
	"io"
	"regexp"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// FUESParser parses the QIS Fachübergreifendes Studium module list.
type FUESParser struct{}

// NewFUESParser creates a new FUESParser.
func NewFUESParser() *FUESParser {
	return &FUESParser{}
}

// Parse extracts all FÜS modules from HTML.
func (p *FUESParser) Parse(r io.Reader) ([]model.FUESModule, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse FÜS HTML: %w", err)
	}

	var results []model.FUESModule
	now := time.Now().UTC()

	tables := FindAllByTag(doc, atom.Table)
	var targetTable *html.Node
	for _, tbl := range tables {
		summary := strings.ToLower(GetAttr(tbl, "summary"))
		if strings.Contains(summary, "suchergebnis") {
			targetTable = tbl
			break
		}
	}
	if targetTable == nil && len(tables) > 0 {
		targetTable = tables[0]
	}
	if targetTable == nil {
		return nil, nil
	}

	rows := FindAllByTag(targetTable, atom.Tr)
	if len(rows) < 2 {
		return nil, nil
	}

	// First row may be headers
	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) < 2 {
			continue
		}

		modID := CleanSingleLine(NodeText(tds[0]))
		if modID == "" || strings.EqualFold(modID, "nr.") || strings.EqualFold(modID, "nr") {
			continue
		}

		fuesMod := model.FUESModule{
			ID:        modID,
			IsFUES:    true,
			ScrapedAt: now,
		}

		if len(tds) > 1 {
			fuesMod.Title = CleanSingleLine(NodeText(tds[1]))
			a := FindFirstByTag(tds[1], atom.A)
			if a != nil {
				fuesMod.QISURL = GetAttr(a, "href")
				aText := CleanSingleLine(NodeText(a))
				if aText != "" {
					fuesMod.Title = aText
				}
			}
		}

		if len(tds) > 2 {
			fuesMod.Language = CleanSingleLine(NodeText(tds[2]))
		}

		if len(tds) > 3 {
			credStr := CleanSingleLine(NodeText(tds[3]))
			fuesMod.CreditsRaw = credStr
			fuesMod.Credits = parseCredits(credStr)
		}

		if len(tds) > 4 {
			isFuesText := strings.ToLower(CleanSingleLine(NodeText(tds[4])))
			if isFuesText == "ja" || isFuesText == "yes" {
				fuesMod.IsFUES = true
			}
		}

		if len(tds) > 5 {
			fuesMod.Limitation = CleanSingleLine(NodeText(tds[5]))
		}

		results = append(results, fuesMod)
	}

	return results, nil
}

// TableRowStatements reads what each row of the QIS module table states (the FÜS list has
// the same columns): one string per module number, of the text of every cell and the
// pordnr of the description the row links. Two readings of the table give a module a
// different string exactly when one of its facts changed: title, language, credits, FÜS
// approval or limitation.
func TableRowStatements(r io.Reader) (map[string]string, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse the module table: %w", err)
	}
	var table *html.Node
	for _, tbl := range FindAllByTag(doc, atom.Table) {
		if strings.Contains(strings.ToLower(GetAttr(tbl, "summary")), "suchergebnis") {
			table = tbl
			break
		}
	}
	statements := make(map[string]string)
	if table == nil {
		return statements, nil
	}
	for _, tr := range FindAllByTag(table, atom.Tr) {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) < 2 {
			continue
		}
		cells := make([]string, 0, len(tds)+1)
		for _, td := range tds {
			cells = append(cells, CleanSingleLine(NodeText(td)))
		}
		if a := FindFirstByTag(tds[1], atom.A); a != nil {
			if m := rePordnrLink.FindStringSubmatch(GetAttr(a, "href")); m != nil {
				cells = append(cells, m[1])
			}
		}
		if cells[0] != "" {
			statements[cells[0]] = strings.Join(cells, "\x1f")
		}
	}
	return statements, nil
}

var rePordnrLink = regexp.MustCompile(`pord\.pordnr=(\d+)`)
