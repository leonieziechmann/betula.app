package gemini

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"unicode"
)

type SourceCell struct {
	ID             string    `json:"id"`
	Table          string    `json:"table"`
	Page           int       `json:"page"`
	Row            string    `json:"row"`
	OriginalRow    string    `json:"original_row,omitempty"`
	CreditEvidence string    `json:"credit_evidence,omitempty"`
	Semesters      []int     `json:"semesters"`
	Raw            string    `json:"raw"`
	Min            float64   `json:"min"`
	Max            float64   `json:"max"`
	BBox           []float64 `json:"bbox"`
	SharedRows     bool      `json:"shared_rows"`
	Workload       []float64 `json:"workload,omitempty"`
	CreditSemester int       `json:"credit_semester,omitempty"`
	// Optional marks a bracketed "(6)" cell: the module sits in one of the
	// listed semesters, which the plan does not narrow down.
	Optional bool `json:"optional,omitempty"`
	// Marker is a legend mark printed behind the credit value; "+" means the
	// module is taken together with an integration module.
	Marker string `json:"marker,omitempty"`
	// Bold and Shaded describe the cell's typography and grey fill. Where the
	// regulation's legend gives them a meaning, Elective (not printed bold) and
	// InPlan (grey: part of the possible study plan) are set; PlanSemester is the
	// semester of the grey cell of an "optional in several semesters" module.
	Bold, Shaded, Elective, InPlan bool
	PlanSemester                   int
	// AltGroup/AltIndex: rows separated by an "oder" line are alternatives; the
	// first alternative (index 0) is the one the semester totals count.
	AltGroup, AltIndex int
}

type PDFLayout struct {
	Cells     []SourceCell      `json:"cells"`
	Totals    []SourceCell      `json:"totals"`
	Tables    []json.RawMessage `json:"tables"`
	Issues    []string          `json:"issues"`
	StartTerm string            `json:"start_term"`
	PlanNames map[string]string `json:"plan_names,omitempty"`
	// Notes are remarks about what the reader ignored; they are reported as info.
	Notes      []string          `json:"notes,omitempty"`
	Amendments []AmendmentReview `json:"amendments,omitempty"`
}

// ReadPDFLayout preserves the physical columns, including empty and merged cells.
func ReadPDFLayout(ctx context.Context, path string) (*PDFLayout, error) {
	return ReadPDFLayoutPages(ctx, path, nil)
}

// ReadPDFLayoutPages selects complete plan variants by physical PDF page number.
// An empty selection inspects the entire document.
func ReadPDFLayoutPages(ctx context.Context, path string, pages []int) (*PDFLayout, error) {
	layout, err := extractPDFLayout(ctx, path, pages...)
	if err != nil {
		return nil, err
	}
	return validatePDFLayout(layout)
}

func ReadPDFLayoutForProgram(ctx context.Context, path string, pages []int, hint string) (*PDFLayout, error) {
	l, err := extractPDFLayout(ctx, path, pages...)
	if err != nil {
		return nil, err
	}
	selectProgramMode(l, hint)
	return validatePDFLayout(l)
}

func validatePDFLayout(layout *PDFLayout) (*PDFLayout, error) {
	if len(layout.Cells) == 0 {
		return nil, fmt.Errorf("no supported ruled semester/ECTS table found (no study plan in this document, e.g. a discontinued program or an amendment); manual review required")
	}
	if len(layout.Issues) > 0 {
		return nil, fmt.Errorf("ambiguous PDF layout; manual review required: %s", strings.Join(layout.Issues, "; "))
	}
	// Detect truncated tables before making a paid model request. A found header
	// alone does not prove that the geometry reader found the remaining table rows.
	covered := make(map[string]map[int]bool)
	for _, c := range layout.Totals {
		if !totalRow.MatchString(strings.TrimSpace(c.Row)) {
			continue
		}
		if covered[c.Table] == nil {
			covered[c.Table] = make(map[int]bool)
		}
		for _, sem := range c.Semesters {
			covered[c.Table][sem] = true
		}
	}
	incomplete := map[string]string{}
	complete := map[string]bool{}
	var order []string
	for _, c := range layout.Cells {
		if _, seen := complete[c.Table]; !seen {
			complete[c.Table] = true
			order = append(order, c.Table)
		}
		for _, sem := range c.Semesters {
			if !covered[c.Table][sem] && incomplete[c.Table] == "" {
				incomplete[c.Table] = fmt.Sprintf("incomplete table %s: no whole-plan total for semester %d; manual review required", c.Table, sem)
				complete[c.Table] = false
			}
		}
	}
	if len(incomplete) > 0 {
		anyComplete := false
		for _, ok := range complete {
			anyComplete = anyComplete || ok
		}
		if !anyComplete {
			return nil, fmt.Errorf("%s", incomplete[order[0]])
		}
		// Another table of the same document is a verified plan. A table without
		// any semester totals next to it is a supplement (typically the list of
		// elective modules of a specialization), not a plan of its own; say so
		// instead of failing the whole document.
		cells := layout.Cells[:0]
		for _, c := range layout.Cells {
			if incomplete[c.Table] == "" {
				cells = append(cells, c)
			}
		}
		layout.Cells = cells
		totals := layout.Totals[:0]
		for _, c := range layout.Totals {
			if incomplete[c.Table] == "" {
				totals = append(totals, c)
			}
		}
		layout.Totals = totals
		for _, table := range order {
			if incomplete[table] != "" {
				delete(layout.PlanNames, table)
				layout.Notes = append(layout.Notes, fmt.Sprintf("Table %s has no semester totals and was ignored as a supplement to the verified plan(s)", table))
			}
		}
	}
	return layout, nil
}

func normalizedTitle(s string) string {
	return strings.Map(func(r rune) rune {
		if unicode.IsLetter(r) || unicode.IsDigit(r) {
			return unicode.ToLower(r)
		}
		return -1
	}, s)
}

var sourceModuleCode = regexp.MustCompile(`^([0-9]{5,6})\s+(.+)$`)

// BindSourceCells is deliberately deterministic: the model labels cells, but never
// decides semester columns or numeric credit values. Spans have no exact semester.
func BindSourceCells(res *CurriculumExtractionResult, layout *PDFLayout) error {
	if len(res.Modules) == 0 {
		return fmt.Errorf("model did not identify a matching study plan")
	}
	// Every source cell already has an authoritative title, semester and LP.
	// Missing AI enrichment must not erase that requirement from the plan.
	for _, c := range layout.Cells {
		found := false
		for _, m := range res.Modules {
			if m.SourceCell == c.ID {
				found = true
			}
		}
		if !found {
			typ := "Modul"
			if c.SharedRows || strings.Contains(strings.ToLower(c.Row), "wahl") || strings.Contains(strings.ToLower(c.Row), "wpf") {
				typ = "Wahlpflicht"
			}
			res.Modules = append(res.Modules, ExtractedModule{SourceCell: c.ID, ModuleName: c.Row, ModuleType: typ})
		}
	}
	cells := make(map[string]SourceCell)
	for _, c := range layout.Cells {
		cells[c.ID] = c
	}
	used := make(map[string]bool)
	for i := range res.Modules {
		m := &res.Modules[i]
		c, ok := cells[m.SourceCell]
		if !ok {
			return fmt.Errorf("%s: missing or invalid source cell %q", m.ModuleName, m.SourceCell)
		}
		if used[c.ID] {
			return fmt.Errorf("source cell %s was counted twice", c.ID)
		}
		used[c.ID] = true
		sourceName := c.Row
		// A model-proposed code is not evidence. Only printed identifiers or a
		// unique catalog title match may establish module identity.
		m.ModuleCode = ""
		if parts := sourceModuleCode.FindStringSubmatch(c.Row); parts != nil {
			m.ModuleCode = parts[1]
			sourceName = parts[2]
		}
		// Source text is authoritative, including when the enrichment model
		// shortens a title. It cannot rename a requirement or choose one option.
		m.ModuleName = sourceName
		if len(layout.PlanNames) > 1 {
			m.Specialization = layout.PlanNames[c.Table]
		}
		if strings.Contains(strings.ToLower(sourceName), " oder ") || strings.Contains(strings.ToLower(sourceName), "wpf") || c.SharedRows {
			m.ModuleCode = ""
			m.ModuleType = "Wahlpflicht"
		}
		m.ModuleName = sourceName
		if strings.TrimSpace(m.ModuleName) == "" || len(c.Semesters) == 0 {
			return fmt.Errorf("%s: empty module/semester evidence", c.ID)
		}
		m.StartSemester, m.EndSemester = c.Semesters[0], c.Semesters[len(c.Semesters)-1]
		m.RecommendedSemester = 0
		m.SemesterSpan = ""
		m.RecommendedSemesterRaw = fmt.Sprint(m.StartSemester)
		if m.StartSemester == m.EndSemester {
			m.RecommendedSemester = m.StartSemester
		} else {
			m.SemesterSpan = fmt.Sprintf("%d-%d", m.StartSemester, m.EndSemester)
			m.RecommendedSemesterRaw = m.SemesterSpan
			// The grey cell of the printed sample plan; other semesters of the
			// span remain possible.
			if c.PlanSemester > 0 {
				m.RecommendedSemester = c.PlanSemester
			}
		}
		if c.Elective || c.AltGroup > 0 {
			m.ModuleType = "Wahlpflicht"
		}
		if c.AltGroup > 0 {
			m.Remarks = strings.TrimSpace(m.Remarks + " Alternative (\"oder\") innerhalb dieser Gruppe.")
		}
		if c.Marker == "+" {
			m.Remarks = strings.TrimSpace(m.Remarks + " Zusammen mit einem Integrationsmodul zu belegen (+).")
		}
		m.Credits, m.MinCredits, m.MaxCredits = 0, 0, 0
		if c.Min == c.Max {
			m.Credits = c.Min
		} else {
			m.MinCredits, m.MaxCredits = c.Min, c.Max
		}
		if m.SemesterSpan != "" || m.MaxCredits > 0 {
			for _, total := range layout.Totals {
				if total.Table == c.Table && totalRow.MatchString(strings.TrimSpace(total.Row)) && total.Min == total.Max && len(total.Semesters) > 0 && total.Semesters[0] <= m.StartSemester && total.Semesters[len(total.Semesters)-1] >= m.EndSemester {
					m.AreaRules = strings.TrimSpace(m.AreaRules + fmt.Sprintf(" Source constraint: all requirements together total %.1f LP in semesters %d-%d.", total.Min, total.Semesters[0], total.Semesters[len(total.Semesters)-1]))
				}
			}
		}
		if c.SharedRows {
			m.ModuleCode = ""
			m.ModuleType = "Wahlpflicht"
			m.ModuleName = "Wahlpflicht: " + c.Row
		}
		m.Remarks = strings.TrimSpace(m.Remarks + fmt.Sprintf(" [PDF page %d, cell %s; %s LP]", c.Page, c.ID, c.Raw))
	}
	for _, c := range layout.Cells {
		if !used[c.ID] {
			return fmt.Errorf("source cell %s (%s) was omitted; incomplete extraction", c.ID, c.Row)
		}
	}
	res.StartTerm = layout.StartTerm
	res.Layout = layout
	return nil
}
