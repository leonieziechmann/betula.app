package gemini

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
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
	// AltOne marks a group that is one requirement however its options are
	// spread over the semester columns: the „oder" is printed at the end of a
	// module name, so each alternative is exactly one row. An „oder" printed on
	// a line of its own separates blocks that may each hold a requirement per
	// semester, and those are counted per column instead.
	AltOne bool
	// AltLabel is the heading of the branch a cell belongs to („Praxisinte-
	// grierende Studienphase"), where the choice is printed as named blocks.
	AltLabel string `json:"alt_label,omitempty"`
	// Track is the heading the plan prints over this alternative
	// („Schwerpunkt Philosophie, Ethik und Kulturwissenschaften"). It tells two
	// alternatives apart where their rows are worded identically.
	Track string `json:"track,omitempty"`
	// Additional marks a budget printed as "+6" on top of a total line that
	// counts the compulsory modules only; it is part of the plan but never of
	// that printed sum.
	Additional bool `json:"additional,omitempty"`
	// Section is the caption the plan prints over this cell („Specialization
	// Phase"); it names a part of the studies, not a requirement.
	Section string `json:"section,omitempty"`
	// RowIndex is the cell's row in its physical table, counted from the top of
	// the table. The printed sums are bound to the rows above them by it.
	RowIndex int `json:"row_index,omitempty"`
	// Grand marks the plan's own grand total: the cell of the per-row total
	// column ("Summe LP") in the total row. It covers every semester at once and
	// counts every row, including one the per-semester line leaves out, so it is
	// never one of the disjoint semester totals.
	Grand bool `json:"grand,omitempty"`
}

type PDFLayout struct {
	Cells     []SourceCell      `json:"cells"`
	Totals    []SourceCell      `json:"totals"`
	Tables    []json.RawMessage `json:"tables"`
	Issues    []string          `json:"issues"`
	StartTerm string            `json:"start_term"`
	PlanNames map[string]string `json:"plan_names,omitempty"`
	// PlanOptions names the Studienoption („Anlage 3: … Fernstudienprogramm")
	// a plan table was printed under, for documents that print the plans of
	// several options of the same program.
	PlanOptions map[string]string `json:"plan_options,omitempty"`
	// Notes are remarks about what the reader ignored; they are reported as info.
	Notes      []string          `json:"notes,omitempty"`
	Amendments []AmendmentReview `json:"amendments,omitempty"`
	// annexRefs are the merged cells that name another Anlage instead of
	// printing credits; they are resolved once every page has been read.
	annexRefs []annexReference
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
	selectStudyOption(l, hint)
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
	totals := DerivePlanTotals(layout)
	res.Totals = totals
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
		if m.StudySection == "" {
			m.StudySection = c.Section
		}
		// Where in the regulation this row stands: the page a reader turns to,
		// and the heading of the plan it belongs to.
		m.SourcePage, m.SourceTable = c.Page, c.Table
		m.SourcePlanLabel = layout.PlanNames[c.Table]
		if len(layout.PlanNames) > 1 {
			m.Specialization = layout.PlanNames[c.Table]
		}
		if c.Track != "" {
			m.Specialization = c.Track
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
		// A row that names a credit range, or spreads over several semesters,
		// does not say on its own what it finally counts for. What the plan
		// prints over it does, and the sum over the fewest rows says the most:
		// „Summe Komplexe des Fachstudiums 44" over three rows of „10-24".
		if m.SemesterSpan != "" || m.MaxCredits > 0 {
			if narrow := narrowestTotals(totals, c.ID); len(narrow) > 0 {
				m.AreaRules = strings.TrimSpace(m.AreaRules + " " + narrow[0].Describe())
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
	// The standard period is what the plan's own semester columns show. A model
	// guess must never turn a six-semester plan into out-of-bounds semesters.
	highest := 0
	for _, c := range layout.Cells {
		for _, sem := range c.Semesters {
			if sem > highest {
				highest = sem
			}
		}
	}
	if highest > 0 {
		res.StandardPeriodSemesters = highest
	}
	res.StartTerm = layout.StartTerm
	res.Layout = layout
	return nil
}

// narrowestTotals are the sums a plan row belongs to, from the one over the
// fewest rows to the one over the whole plan. A row belongs to several: its
// section's sum, the sum of the part of the studies, and the plan's own total.
func narrowestTotals(totals []PlanTotal, cell string) []PlanTotal {
	var out []PlanTotal
	for _, t := range totals {
		for _, member := range t.Members {
			if member == cell {
				out = append(out, t)
				break
			}
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return len(out[i].Members) < len(out[j].Members) })
	return out
}
