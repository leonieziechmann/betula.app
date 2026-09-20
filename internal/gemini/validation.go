package gemini

import (
	"fmt"
	"math"
	"regexp"
	"sort"
	"strings"

	"github.com/leonieziechmann/betula/internal/model"
)

type ValidationIssue struct {
	Severity string `json:"severity"`
	Code     string `json:"code"`
	Module   string `json:"module,omitempty"`
	Message  string `json:"message"`
}

type SemesterCredits struct {
	Table       string  `json:"table"`
	Start       int     `json:"start_semester"`
	End         int     `json:"end_semester"`
	Min         float64 `json:"min_credits"`
	Max         float64 `json:"max_credits"`
	SourceTotal float64 `json:"source_total,omitempty"`
}

type ValidationReport struct {
	Valid     bool              `json:"valid"`
	StartTerm string            `json:"start_term"`
	Matched   int               `json:"matched_modules"`
	Issues    []ValidationIssue `json:"issues"`
	Credits   []SemesterCredits `json:"credits"`
}

// MatchCatalogModule requires a unique exact identity. In particular, a short
// title such as "Mathematik" must not match an arbitrary LIKE ... LIMIT 1 row.
// Where one title names several modules of the university, the program's own
// claim decides: a plan of Informatik that says "Programmierpraktikum" means
// the module Informatik itself lists, not its namesake in another faculty. It
// decides, it does not guess — without exactly one claimed candidate the row
// stays unlinked.
func MatchCatalogModule(m ExtractedModule, catalog model.CurriculumCatalog) *model.CurriculumCatalogModule {
	var candidates []int
	for i, c := range catalog.Modules {
		match := false
		if m.ModuleCode != "" {
			match = c.ID == m.ModuleCode || c.Code == m.ModuleCode
		} else {
			name := normalizedTitle(m.ModuleName)
			match = name != "" && (name == normalizedTitle(c.TitleDE) || name == normalizedTitle(c.TitleEN))
		}
		if match {
			candidates = append(candidates, i)
		}
	}
	if len(candidates) == 0 && m.ModuleCode == "" {
		return matchSimilarTitle(m.ModuleName, catalog)
	}
	if len(candidates) > 1 {
		candidates = claimed(candidates, catalog)
	}
	if len(candidates) != 1 {
		return nil
	}
	return &catalog.Modules[candidates[0]]
}

// LinkModule is the catalog module a plan row names, or "" where the catalog
// does not identify it beyond doubt. It is the whole rule: the scan writes the
// link it returns, and a later run over the stored rows must reach the same one.
func LinkModule(m ExtractedModule, catalog model.CurriculumCatalog) string {
	if c := MatchCatalogModule(m, catalog); c != nil && !IdentityConflict(m, *c) {
		return c.ID
	}
	return ""
}

// claimed keeps the candidates the program itself names.
func claimed(candidates []int, catalog model.CurriculumCatalog) []int {
	var own []int
	for _, i := range candidates {
		if catalog.Claims[catalog.Modules[i].ID] {
			own = append(own, i)
		}
	}
	return own
}

// compulsoryTotal marks a printed sum that counts the compulsory modules only,
// next to a separately printed elective budget.
var compulsoryTotal = regexp.MustCompile(`(?i)\(pflichtmodule\)`)

// totalRow recognizes whole-plan total rows. Hyphenation from wrapped labels
// ("Leistungs- punkte") is removed before matching. A wrong classification is
// caught by the reconciliation against the extracted cells, so the label list
// may be generous; partial subtotals such as "Summe Informatik-Vertiefung" are
// not on it.
var totalRow = totalLabelMatcher{regexp.MustCompile(`^(?:summe(?: der| aller| gesamt| über alle)?(?: studium| (?:master|bachelor)-?studium| gesamtstudium| gesamt| erreichte (?:lp|kp|leistungspunkte|kreditpunkte)| gutschrift(?: lp)?| aufwand(?: lp)?| anrechnung der lp des moduls / semester| leistungspunkte| kreditpunkte| lp| kp| ects| cp)?(?: pro semester| je semester)?|(?:lp|kp|cp|ects|leistungspunkte) gesamt(?: \d+)?|gesamt(?: lp| kp)?|insgesamt|total(?: credits)?|teilsummen? (?:pro|je) semester|gesamt-?summe(?: lp| kp)?|summe(?: lp| kp| ects| cp)? \(pflichtmodule\)|summe aufwand in der studienrichtung .+|arbeitsaufwand für die studienrichtung .+|summe nach arbeitsaufwand|(?:lp )?aufteilung nach studentischem arbeitsaufwand(?: \d\))?|[σ∑]? ?= ?\d+ ?(?:lp|kp|cp|ects))$`)}

type totalLabelMatcher struct{ re *regexp.Regexp }

// footnoteLead is a footnote index printed in front of a total's label,
// as in "1 Summe LP".
var footnoteLead = regexp.MustCompile(`^[0-9⁰¹²³⁴⁵⁶⁷⁸⁹]\)?\s+`)

func (m totalLabelMatcher) MatchString(s string) bool {
	s = strings.ToLower(strings.Join(strings.Fields(s), " "))
	s = strings.ReplaceAll(s, "- ", "")
	s = strings.TrimRight(s, " :*")
	s = footnoteLead.ReplaceAllString(s, "")
	return m.re.MatchString(s)
}

var singleSemesterDuration = regexp.MustCompile(`(?i)^1\s*semester`)

// ValidateCurriculum reports contradictions without changing any extracted data.
// startTerm may override only the intake assumption, never the source semester.
func ValidateCurriculum(res *CurriculumExtractionResult, catalog model.CurriculumCatalog, startTerm string, tolerance float64) ValidationReport {
	if startTerm == "" || startTerm == "auto" {
		startTerm = res.StartTerm
	}
	report := ValidationReport{Valid: true, StartTerm: startTerm, Issues: []ValidationIssue{}, Credits: []SemesterCredits{}}
	add := func(severity, code, module, message string) {
		report.Issues = append(report.Issues, ValidationIssue{severity, code, module, message})
		if severity == "error" {
			report.Valid = false
		}
	}
	if len(res.Modules) == 0 {
		add("error", "empty_plan", "", "No curriculum modules extracted")
	}
	if startTerm != "winter" && startTerm != "summer" {
		// Informational only: without a printed intake the season check is
		// simply skipped; the intake can be added later where a module needs it.
		add("info", "unknown_intake", "", "Intake is unknown; season parity was not checked. Set --start-term winter or summer for a specific intake.")
	}
	if res.Layout != nil {
		for _, note := range res.Layout.Notes {
			add("info", "ignored_table", "", note)
		}
	}
	creditsByModule := make(map[string]float64)
	known := make(map[string]*model.CurriculumCatalogModule)
	// A PDF may print several plan variants (regular, dual, ...). Credits of the
	// same module in different physical tables are alternatives, never additive.
	tableOf := map[string]string{}
	if res.Layout != nil {
		for _, c := range res.Layout.Cells {
			tableOf[c.ID] = c.Table
		}
	}
	groupTable := map[string]string{}
	ranged := make(map[string]bool)
	concrete, linkedConcrete := 0, 0
	for _, m := range res.Modules {
		kind := strings.ToLower(m.ModuleType + " " + m.ModuleName)
		isConcrete := !strings.Contains(kind, "wahl") && !strings.Contains(kind, "wpf") && !strings.Contains(kind, "füs") && !strings.Contains(kind, "fachübergreifend")
		if isConcrete {
			concrete++
		}
		if m.StartSemester < 1 || m.EndSemester < m.StartSemester || (res.StandardPeriodSemesters > 0 && m.EndSemester > res.StandardPeriodSemesters) {
			add("error", "semester_bounds", m.ModuleName, "Semester is outside the study period")
		}
		if m.Credits < 0 || m.MinCredits < 0 || m.MaxCredits < m.MinCredits {
			add("error", "credit_bounds", m.ModuleName, "Invalid credit range")
		}
		c := MatchCatalogModule(m, catalog)
		if c == nil {
			if m.ModuleCode != "" {
				add("warning", "unmatched_code", m.ModuleName, "Module code is absent or ambiguous in the catalog: "+m.ModuleCode)
			} else if isConcrete {
				add("warning", "unmatched_title", m.ModuleName, "No unique, sufficiently close catalog title; no module link assigned")
			}
			continue
		}
		if IdentityConflict(m, *c) {
			// The printed code now names a different module (codes are reused or
			// renamed over the years). The plan itself is fine; only the link is
			// withheld, and the catalog data of the wrong module is not compared.
			add("warning", "catalog_identity_conflict", m.ModuleName, fmt.Sprintf("Code %s belongs to %q in the catalog; no module link assigned", m.ModuleCode, c.TitleDE))
			continue
		}
		report.Matched++
		if m.ModuleCode == "" && normalizedTitle(m.ModuleName) != normalizedTitle(c.TitleDE) && normalizedTitle(m.ModuleName) != normalizedTitle(c.TitleEN) {
			add("warning", "catalog_title_match", m.ModuleName, fmt.Sprintf("Unique text match: %s — %s; source title preserved", c.ID, c.TitleDE))
		}
		if isConcrete {
			linkedConcrete++
		}

		group := c.ID + "|" + m.Specialization + "|" + tableOf[m.SourceCell]
		groupTable[group] = tableOf[m.SourceCell]
		known[group] = c
		creditsByModule[group] += m.Credits
		ranged[group] = ranged[group] || m.MaxCredits > 0
		turnus := strings.ToLower(c.Turnus)
		winter := strings.Contains(turnus, "winter")
		summer := strings.Contains(turnus, "sommer") || strings.Contains(turnus, "summer")
		// Multi-semester cells do not specify an individual placement or start date.
		// Modules lasting several semesters may be credited at the end of teaching.
		multi := c.Duration != "" && !singleSemesterDuration.MatchString(c.Duration)
		if m.StartSemester == m.EndSemester && !multi && winter != summer && (startTerm == "winter" || startTerm == "summer") {
			expectedWinter := (m.StartSemester%2 == 1) == (startTerm == "winter")
			if expectedWinter != winter {
				add("warning", "season_conflict", m.ModuleName, fmt.Sprintf("Semester %d with %s intake conflicts with catalog offering %q (module %s); review source/version", m.StartSemester, startTerm, c.Turnus, c.ID))
			}
		}
		if strings.Contains(turnus, "jahre") || strings.Contains(turnus, "year") {
			add("warning", "biennial_offering", m.ModuleName, "Offering also depends on the calendar year; intake year is not known")
		}
	}
	if concrete > 0 && linkedConcrete*2 < concrete {
		add("warning", "catalog_coverage", "", fmt.Sprintf("Only %d of %d non-elective requirements are linked to catalog modules; check missing historical modules or title differences", linkedConcrete, concrete))
	}
	// The regulation is authoritative for its own plan. If the printed semester
	// totals of the module's table add up to the extracted credits, a differing
	// catalog value only reflects another regulation version: report it for
	// information. Otherwise the layout itself is suspect and stays a warning.
	conflictTables := map[string]bool{}
	checkedTables := make(map[string]bool)
	emitCreditConflicts := func() {
		keys := make([]string, 0, len(known))
		for key := range known {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			c := known[key]
			if ranged[key] || c.Credits <= 0 || math.Abs(creditsByModule[key]-c.Credits) <= 0.01 {
				continue
			}
			table := groupTable[key]
			msg := fmt.Sprintf("Source has %.1f LP, current catalog has %.1f LP; check regulation version", creditsByModule[key], c.Credits)
			if table != "" && checkedTables[table] && !conflictTables[table] {
				add("info", "catalog_credit_conflict", c.TitleDE, msg+" (semester totals of the source plan add up; regulation value kept)")
			} else {
				add("warning", "catalog_credit_conflict", c.TitleDE, msg)
			}
		}
	}
	if res.Layout == nil {
		emitCreditConflicts()
		add("error", "missing_layout", "", "No verifiable PDF cell evidence")
		return report
	}
	planCells := effectiveCells(res.Layout)
	// Group by physical table and semester span, never distribute ranges equally.
	groups := make(map[string]*SemesterCredits)
	var order []string
	for _, c := range planCells {
		start, end := c.Semesters[0], c.Semesters[len(c.Semesters)-1]
		key := fmt.Sprintf("%s:%d:%d", c.Table, start, end)
		if groups[key] == nil {
			groups[key] = &SemesterCredits{Table: c.Table, Start: start, End: end}
			order = append(order, key)
		}
		groups[key].Min += c.Min
		groups[key].Max += c.Max
	}
	checked := 0
	// A table that prints both a credit line and a workload line: when the
	// workload line adds up with the extracted cells, the reading is right and a
	// credit line that does not follow the crediting rule is only a warning.
	aufwandBad, aufwandMatched := map[string]bool{}, map[string]bool{}
	creditMismatch, aufwandMismatch := map[string][]string{}, map[string][]string{}
	compulsoryTables := map[string]bool{}
	printedLoByTable, printedHiByTable := map[string]float64{}, map[string]float64{}
	printedSemesters := map[string]map[int]bool{}
	overlappingTotals := map[string]bool{}
	coveredSemesters := make(map[string]map[int]bool)
	for _, t := range dominantTotals(res.Layout.Totals) {
		// A printed total may itself be a range when the plan contains elective
		// budgets ("28 - 32"); it is then checked as an interval.
		if !totalRow.MatchString(strings.TrimSpace(t.Row)) {
			continue
		}
		checked++
		if printedSemesters[t.Table] == nil {
			printedSemesters[t.Table] = map[int]bool{}
		}
		for _, sem := range t.Semesters {
			if printedSemesters[t.Table][sem] {
				overlappingTotals[t.Table] = true
			}
			printedSemesters[t.Table][sem] = true
		}
		printedLoByTable[t.Table] += t.Min
		printedHiByTable[t.Table] += t.Max
		checkedTables[t.Table] = true
		if coveredSemesters[t.Table] == nil {
			coveredSemesters[t.Table] = make(map[int]bool)
		}
		for _, sem := range t.Semesters {
			coveredSemesters[t.Table][sem] = true
		}
		start, end := t.Semesters[0], t.Semesters[len(t.Semesters)-1]
		low, high := 0.0, 0.0
		aufwand := strings.Contains(strings.ToLower(t.Row), "aufwand")
		compulsory := compulsoryTotal.MatchString(t.Row)
		if compulsory {
			compulsoryTables[t.Table] = true
		}
		for _, c := range planCells {
			if c.Table != t.Table {
				continue
			}
			if compulsory && c.Elective {
				continue // the elective budget is printed as its own line
			}
			a, b := c.Semesters[0], c.Semesters[len(c.Semesters)-1]
			if aufwand && len(c.Workload) == len(c.Semesters) && len(c.Workload) > 0 {
				// A workload total counts the effort per semester, not the credit
				// that is booked in the last semester of the module.
				for i, sem := range c.Semesters {
					if sem >= start && sem <= end {
						low += c.Workload[i]
						high += c.Workload[i]
					}
				}
				continue
			}
			if c.CreditSemester > 0 {
				a, b = c.CreditSemester, c.CreditSemester
			}
			if a >= start && b <= end {
				low += c.Min
				high += c.Max
			} else if a <= end && b >= start {
				high += c.Max
			}
		}
		if t.Max < low-0.01 || t.Min > high+0.01 {
			msg := fmt.Sprintf("%s semesters %d-%d: source total %s LP outside extracted %.1f–%.1f LP", t.Table, start, end, amountRange(t.Min, t.Max), low, high)
			if aufwand {
				aufwandBad[t.Table] = true
				aufwandMismatch[t.Table] = append(aufwandMismatch[t.Table], msg)
			} else {
				creditMismatch[t.Table] = append(creditMismatch[t.Table], msg)
			}
		} else if aufwand {
			aufwandMatched[t.Table] = true
		}
		catalogSum, covered, required := catalogTotalForSpan(res, catalog, t)
		if covered > 0 {
			code, severity := "catalog_total_check", "info"
			if covered == required && math.Abs(catalogSum-t.Min) > .01 {
				code, severity = "catalog_total_conflict", "warning"
			}
			if covered == required && math.Abs(catalogSum-t.Min) <= .01 && (t.Min < low-.01 || t.Min > high+.01) {
				code, severity = "catalog_suggests_layout_error", "error"
			}
			add(severity, code, "", fmt.Sprintf("%s semesters %d-%d: printed %.1f LP; extracted %.1f–%.1f LP; catalog %.1f LP (%d/%d requirements with unambiguous credit allocation)", t.Table, start, end, t.Min, low, high, catalogSum, covered, required))
		}
		expected := 30.0
		if res.TotalCredits > 0 && res.StandardPeriodSemesters > 0 {
			expected = res.TotalCredits / float64(res.StandardPeriodSemesters)
		}
		if math.Abs(t.Min/float64(end-start+1)-expected) > tolerance {
			add("warning", "semester_load", "", fmt.Sprintf("%s semesters %d-%d: %.1f LP per semester, expected about %.1f (tolerance %.1f)", t.Table, start, end, t.Min/float64(end-start+1), expected, tolerance))
		}
		key := fmt.Sprintf("%s:%d:%d", t.Table, start, end)
		if groups[key] != nil {
			groups[key].SourceTotal = t.Min
		}
	}
	// A plan whose printed totals add up to the extracted requirements as a whole,
	// but not semester by semester, is internally inconsistent in the regulation
	// itself: the modules, their credits and the plan total are still verified.
	planReconciles := map[string]bool{}
	for table, totalLo := range printedLoByTable {
		if overlappingTotals[table] {
			continue
		}
		lo, hi := 0.0, 0.0
		for _, c := range planCells {
			if c.Table == table && !(compulsoryTables[table] && c.Elective) {
				lo += c.Min
				hi += c.Max
			}
		}
		// Only an exact match excuses the split. With elective ranges on either
		// side the totals could agree by coincidence and hide a misread cell.
		exact := math.Abs(printedHiByTable[table]-totalLo) < .01 && math.Abs(hi-lo) < .01
		planReconciles[table] = exact && math.Abs(totalLo-lo) < .01
	}
	for _, table := range sortedKeys(aufwandMismatch) {
		for _, msg := range aufwandMismatch[table] {
			if planReconciles[table] {
				add("warning", "source_semester_split_unexplained", "", msg+" (the plan total matches the extracted requirements; the regulation's own per-semester split does not)")
				continue
			}
			conflictTables[table] = true
			add("error", "source_total_conflict", "", msg)
		}
	}
	for _, table := range sortedKeys(creditMismatch) {
		for _, msg := range creditMismatch[table] {
			if aufwandMatched[table] && !aufwandBad[table] {
				add("warning", "source_credit_total_unexplained", "", msg+" (the printed workload totals add up; check how the regulation credits multi-semester modules)")
			} else if planReconciles[table] {
				add("warning", "source_semester_split_unexplained", "", msg+" (the plan total matches the extracted requirements; the regulation's own per-semester split does not)")
			} else {
				conflictTables[table] = true
				add("error", "source_total_conflict", "", msg)
			}
		}
	}
	if checked == 0 {
		add("error", "missing_source_totals", "", "No unambiguous whole-plan semester totals found; this may be a truncated table or an amendment without the complete plan")
		for _, key := range order {
			g := groups[key]
			if g.Start == g.End && (g.Min > 30+tolerance || g.Max < 30-tolerance) {
				add("warning", "semester_load", "", fmt.Sprintf("%s semester %d: %.1f–%.1f LP; inspect elective groups and missing totals", g.Table, g.Start, g.Min, g.Max))
			}
		}
	}
	for _, table := range sortedFloatKeys(printedLoByTable) {
		if overlappingTotals[table] {
			continue
		}
		totalLo, totalHi := printedLoByTable[table], printedHiByTable[table]
		lo, hi := 0.0, 0.0
		for _, c := range planCells {
			if c.Table == table && !(compulsoryTables[table] && c.Elective) {
				lo += c.Min
				hi += c.Max
			}
		}
		if totalHi < lo-.01 || totalLo > hi+.01 {
			conflictTables[table] = true
			add("error", "source_plan_total_conflict", "", fmt.Sprintf("%s: entire plan has %.1f–%.1f LP, disjoint printed semester totals sum to %s LP; inspect alternative tracks or duplicated requirements", table, lo, hi, amountRange(totalLo, totalHi)))
		}
	}
	for _, key := range order {
		if !checkedTables[groups[key].Table] {
			conflictTables[groups[key].Table] = true
			add("error", "incomplete_table", "", groups[key].Table+": no whole-plan totals; verify continuation pages or plan variants")
			checkedTables[groups[key].Table] = true
		}
		g := groups[key]
		for sem := g.Start; sem <= g.End; sem++ {
			if !coveredSemesters[g.Table][sem] {
				conflictTables[g.Table] = true
				add("error", "missing_semester_total", "", fmt.Sprintf("%s semester %d has no whole-plan source total", g.Table, sem))
			}
		}
	}
	for _, key := range order {
		report.Credits = append(report.Credits, *groups[key])
	}
	emitCreditConflicts()
	return report
}

// effectiveCells are the cells whose credits the printed semester totals add
// up. Where the regulation prints a "possible study plan" as grey cells, only
// those count; the other cells are alternatives, and a module that may be taken
// in several semesters counts in the semester of its grey cell.
func effectiveCells(l *PDFLayout) []SourceCell {
	tableHasPlan := map[string]bool{}
	for _, c := range l.Cells {
		if c.InPlan {
			tableHasPlan[c.Table] = true
		}
	}
	// Within one "oder" group the plan still expects one requirement per semester
	// column: "A oder B" in the first semester and "C oder D" in the second are
	// two obligations, not one.
	type altKey struct {
		table string
		group int
		span  string
	}
	first := map[altKey]int{}
	key := func(c SourceCell) altKey {
		return altKey{c.Table, c.AltGroup, fmt.Sprint(c.Semesters)}
	}
	for _, c := range l.Cells {
		if c.AltGroup == 0 {
			continue
		}
		if idx, seen := first[key(c)]; !seen || c.AltIndex < idx {
			first[key(c)] = c.AltIndex
		}
	}
	counted := map[altKey]bool{}
	var out []SourceCell
	for _, c := range l.Cells {
		if c.AltGroup > 0 {
			k := key(c)
			if c.AltIndex != first[k] || counted[k] {
				continue // another alternative for this semester already counts
			}
			counted[k] = true
		}
		if c.Additional {
			continue // a budget the plan prints on top of its own semester sums
		}
		if tableHasPlan[c.Table] {
			if !c.InPlan {
				continue
			}
			if c.PlanSemester > 0 {
				c.Semesters = []int{c.PlanSemester}
			}
		}
		out = append(out, c)
	}
	return out
}

func sortedKeys(m map[string][]string) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

// amountRange prints a credit value or, for elective budgets, its range.
func amountRange(lo, hi float64) string {
	if math.Abs(hi-lo) < 0.01 {
		return fmt.Sprintf("%.1f", lo)
	}
	return fmt.Sprintf("%.1f–%.1f", lo, hi)
}

func sortedFloatKeys(m map[string]float64) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

// isAufwandRow marks a total that counts student workload per semester instead
// of the credits booked in that semester.
func isAufwandRow(c SourceCell) bool {
	return strings.Contains(strings.ToLower(c.Row), "aufwand")
}

// dominantTotals drops section subtotals that carry the same generic label as
// the whole-plan total, as in "Summe" under a group of modules and "Summe
// Studium" at the bottom of the same table. For one table, one semester span
// and one kind of total, the whole plan is never smaller than one of its
// sections, so the largest printed value is the plan total. Workload rows are
// kept next to credit rows because they measure different things.
func dominantTotals(totals []SourceCell) []SourceCell {
	key := func(c SourceCell) string {
		return fmt.Sprintf("%s|%v|%t", c.Table, c.Semesters, isAufwandRow(c))
	}
	best := map[string]float64{}
	for _, c := range totals {
		if !totalRow.MatchString(strings.TrimSpace(c.Row)) {
			continue
		}
		if v, seen := best[key(c)]; !seen || c.Min > v {
			best[key(c)] = c.Min
		}
	}
	used := map[string]bool{}
	out := make([]SourceCell, 0, len(totals))
	for _, c := range totals {
		if !totalRow.MatchString(strings.TrimSpace(c.Row)) {
			out = append(out, c)
			continue
		}
		k := key(c)
		if used[k] || c.Min != best[k] {
			continue
		}
		used[k] = true
		out = append(out, c)
	}
	return out
}
