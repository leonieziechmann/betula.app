package catalogdb

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"github.com/leonieziechmann/betula/internal/abbrev"
	"github.com/leonieziechmann/betula/internal/oplog"
)

// Check severities.
const (
	StatusOK   = "ok"
	StatusInfo = "info" // a number worth knowing, never a failure
	StatusWarn = "warn" // source data that needs a look; does not fail the run
	StatusFail = "fail" // broken invariant or regression
)

// Check is one validation result.
type Check struct {
	Name    string
	Status  string
	Value   int64
	Detail  string
	Samples []string
}

// Baseline is a lower bound for a count. It turns a number that was once verified
// against the live site into a regression test: a parser that silently stops
// recognising a label drops below it.
type Baseline struct {
	Name  string
	Query string
	Min   int64
}

// BTUBaselines were verified on 2026-09-19 (docs/data-sources.md). Minimums sit a
// little below the measured values, so that ordinary catalog changes do not trip them.
var BTUBaselines = []Baseline{
	{"modules", "SELECT COUNT(*) FROM module", 4800},
	{"modules with a parsed page", "SELECT COUNT(*) FROM module WHERE detail_status = 'ok'", 4800},
	// The fields come from QIS since 2026-09-20 (docs/data-sources.md). The three
	// checks below replace the one on English module pages on b-tu.de, which is no
	// longer the source: that a description is read at all, that its study programs
	// are read, and that it names the events of the semester that runs now.
	{"modules with a QIS description", "SELECT COUNT(*) FROM module WHERE description_source = 'qis'", 3000},
	{"QIS modules with program assignments (P2)", "SELECT COUNT(DISTINCT r.module_id) FROM module_program_ref r JOIN module m ON m.id = r.module_id WHERE m.description_source = 'qis'", 2400},
	{"modules with an English title", "SELECT COUNT(*) FROM module WHERE title_en IS NOT NULL AND title_en <> ''", 3000},
	{"modules with known grading (P1)", "SELECT COUNT(*) FROM module WHERE is_graded IS NOT NULL", 4800},
	{"modules with exam details (P1)", "SELECT COUNT(*) FROM module WHERE exam_details IS NOT NULL", 4000},
	{"facet: exercise (v1 found 0 of 1,535)", "SELECT COUNT(*) FROM v_module_facets WHERE has_exercise = 1", 1535},
	{"facet: plain winter semester", "SELECT COUNT(*) FROM v_module_facets WHERE turnus_season = 'winter' AND turnus_parity IS NULL", 1800},
	{"facet: even-year modules are not odd-year modules", "SELECT COUNT(*) FROM v_module_facets WHERE turnus_parity = 'even'", 40},
	{"facet: English-taught", "SELECT COUNT(*) FROM v_module_facets WHERE teaches_english = 1", 900},
	{"facet: limited participation with a number", "SELECT COUNT(*) FROM v_module_facets WHERE participant_limit IS NOT NULL", 200},
	{"FÜS modules", "SELECT COUNT(*) FROM module WHERE is_fues = 1", 250},
	{"modules with a known campus, if any event is archived", "SELECT CASE WHEN (SELECT COUNT(*) FROM event_date WHERE campus IS NOT NULL) = 0 THEN 1000000 ELSE (SELECT COUNT(*) FROM v_module_facets WHERE at_zentralcampus IS NOT NULL) END", 100},
	{"programs", "SELECT COUNT(*) FROM program", 170},
	{"programs with tree modules", "SELECT COUNT(*) FROM program_coverage WHERE tree_modules > 0", 170},
	{"validated plans", "SELECT COUNT(*) FROM v_program_plan", 140},
	{"Lehramt programs reached by module pages (P3)", "SELECT COUNT(DISTINCT program_id) FROM program_module pm JOIN program p ON p.id = pm.program_id WHERE p.degree_level = 'teaching_bachelor' AND pm.on_module_page = 1", 20},
	// Measured 2026-09-25: 68 % of the 4,936 modules have a program-free abbreviation of three
	// characters. A derivation that silently degrades (a broken splitter, lost overrides) drops
	// below. A share of the modules, not of the program pairs: a FÜS module counts once, not once
	// for each of the ~60 programs that offer it, so new language courses cannot trip it. (That
	// every module has an abbreviation is a check above, not a baseline.)
	{"module abbreviations of exactly three characters, % of the modules",
		"SELECT COUNT(*) * 100 / MAX(1, (SELECT COUNT(*) FROM module_abbrev)) FROM module_abbrev WHERE LENGTH(abbrev) = 3", 55},
}

// Validate checks the invariants of a built database. It is read-only.
func (db *DB) Validate(ctx context.Context, baselines []Baseline) ([]Check, error) {
	v := &validator{ctx: ctx, db: db.sql}

	v.count("foreign keys hold", StatusFail, "SELECT COUNT(*) FROM pragma_foreign_key_check", "")
	v.emptyStrings()
	v.count("no 0 placeholders in nullable numbers", StatusFail, `
		SELECT (SELECT COUNT(*) FROM module WHERE credits = 0 OR duration_semesters = 0 OR participant_limit = 0)
		     + (SELECT COUNT(*) FROM plan_entry WHERE semester = 0 OR start_semester = 0 OR end_semester = 0
		                                           OR credits = 0 OR min_credits = 0 OR max_credits = 0)`, "")
	v.count("program_module matches its source view", StatusFail, `
		SELECT (SELECT COUNT(*) FROM (SELECT * FROM v_program_module_src EXCEPT SELECT * FROM program_module))
		     + (SELECT COUNT(*) FROM (SELECT * FROM program_module EXCEPT SELECT * FROM v_program_module_src))`, "")
	v.count("module_facet matches its source view", StatusFail, `
		SELECT (SELECT COUNT(*) FROM (SELECT * FROM v_module_facets_src EXCEPT SELECT * FROM module_facet))
		     + (SELECT COUNT(*) FROM (SELECT * FROM module_facet EXCEPT SELECT * FROM v_module_facets_src))`, "")
	v.count("a FÜS relation never overlaps the curriculum", StatusFail,
		"SELECT COUNT(*) FROM program_module WHERE relation = 'fues' AND (in_tree = 1 OR in_plan = 1)", "")

	// Short names (docs/schema-v2.md, „Short names“). A migrated database that was not built
	// again has none, and fails here: it must not be exported.
	v.count("every event date with a room has a short form", StatusFail,
		"SELECT COUNT(*) FROM event_date WHERE room IS NOT NULL AND room_short IS NULL", "")
	v.count("a short room name names one room", StatusFail, `
		SELECT COUNT(*) FROM (SELECT room_short FROM event_date WHERE room_short IS NOT NULL
		                      GROUP BY room_short HAVING COUNT(DISTINCT room) > 1)`,
		`SELECT room_short || ': ' || GROUP_CONCAT(DISTINCT room) FROM event_date WHERE room_short IS NOT NULL
		 GROUP BY room_short HAVING COUNT(DISTINCT room) > 1`)
	v.count("every module has an abbreviation", StatusFail,
		"SELECT COUNT(*) FROM module m WHERE NOT EXISTS (SELECT 1 FROM module_abbrev a WHERE a.module_id = m.id)", "")
	v.count("every module of a program has an abbreviation", StatusFail, `
		SELECT COUNT(*) FROM program_module pm WHERE NOT EXISTS (SELECT 1 FROM program_module_abbrev a
		 WHERE a.program_id = pm.program_id AND a.module_id = pm.module_id)`, "")
	// unique without case and without the & and - a reader passes over (B&B is BB)
	const abbrevKey = "REPLACE(REPLACE(abbrev, '&', ''), '-', '') COLLATE NOCASE"
	v.count("abbreviations are unique within a program", StatusFail, `
		SELECT COUNT(*) FROM (SELECT 1 FROM program_module_abbrev GROUP BY program_id, `+abbrevKey+` HAVING COUNT(*) > 1)`,
		`SELECT program_id || ' ' || GROUP_CONCAT(abbrev) || ': ' || GROUP_CONCAT(module_id) FROM program_module_abbrev
		 GROUP BY program_id, `+abbrevKey+` HAVING COUNT(*) > 1`)
	v.blockedAbbreviations()
	v.count("abbreviations are 2 to 10 characters without spaces", StatusFail, `
		SELECT (SELECT COUNT(*) FROM module_abbrev WHERE LENGTH(abbrev) NOT BETWEEN 2 AND 10 OR abbrev LIKE '% %')
		     + (SELECT COUNT(*) FROM program_module_abbrev WHERE LENGTH(abbrev) NOT BETWEEN 2 AND 10 OR abbrev LIKE '% %')`, "")
	v.count("(program, module) pairs whose abbreviation is not their first choice", StatusInfo,
		"SELECT COUNT(*) FROM program_module_abbrev WHERE choice > 1 OR is_twin = 1", "")
	v.count("room short forms longer than 12 characters", StatusInfo,
		"SELECT COUNT(DISTINCT room) FROM event_date WHERE LENGTH(room_short) > 12", "")

	v.count("modules without a module page", StatusWarn, "SELECT COUNT(*) FROM module WHERE detail_status = 'missing'",
		"SELECT id || ' ' || title FROM module WHERE detail_status = 'missing' ORDER BY id")
	// Folia shows a module's dates by semester and leaves out one without. An event QIS has
	// removed is not built at all (catalogbuild.loadEvents), so one here is a page whose
	// semester the parser did not read.
	v.count("events a module links that have no semester", StatusWarn, `
		SELECT COUNT(*) FROM event e WHERE e.semester_key IS NULL AND EXISTS (SELECT 1 FROM module_event me WHERE me.event_id = e.id)`,
		`SELECT e.id || ' ' || e.title || ' → ' || GROUP_CONCAT(me.module_id, ', ') FROM event e JOIN module_event me ON me.event_id = e.id
		 WHERE e.semester_key IS NULL GROUP BY e.id ORDER BY e.id`)
	v.count("module page assignments that resolve to no program", StatusWarn, "SELECT COUNT(*) FROM module_program_ref WHERE resolve_status = 'unresolved'",
		"SELECT degree_raw || ' / ' || program_raw || ' / ' || po_raw || '  ×' || COUNT(*) FROM module_program_ref WHERE resolve_status = 'unresolved' GROUP BY 1 ORDER BY COUNT(*) DESC")
	v.count("FÜS list and module page sentence disagree", StatusWarn, "SELECT COUNT(*) FROM module WHERE page_states_fues IS NOT NULL AND page_states_fues <> is_fues",
		"SELECT id || ' list=' || is_fues || ' page=' || page_states_fues FROM module WHERE page_states_fues IS NOT NULL AND page_states_fues <> is_fues ORDER BY id")
	v.count("programs without any tree module", StatusWarn, "SELECT COUNT(*) FROM program_coverage WHERE tree_modules = 0",
		"SELECT program_id || ' ' || program_name || ' (' || degree || ', PO ' || po_version || ')' FROM program_coverage WHERE tree_modules = 0 ORDER BY 1")
	v.count("validated plans without a program", StatusWarn, "SELECT COUNT(*) FROM plan WHERE program_id NOT IN (SELECT id FROM program)",
		"SELECT program_id FROM plan WHERE program_id NOT IN (SELECT id FROM program) ORDER BY 1")
	v.count("plan entries matched to a module that is not in the catalog", StatusWarn,
		"SELECT COUNT(*) FROM plan_entry WHERE module_id IS NOT NULL AND module_id NOT IN (SELECT id FROM module)",
		"SELECT program_id || ': ' || module_id || ' ' || module_name FROM plan_entry WHERE module_id IS NOT NULL AND module_id NOT IN (SELECT id FROM module) ORDER BY 1")
	// A printed sum is only worth storing where the rows it names reach it: the
	// whole point of plan_total is that a plan with elective budgets can be added
	// up. A sum outside the interval of its own rows would be a misread cell.
	v.count("plan totals their own rows cannot reach", StatusFail, `
		SELECT COUNT(*) FROM plan_total WHERE credits_max < min_credits - 0.01 OR credits > max_credits + 0.01`,
		`SELECT program_id || ' ' || label || ' (' || start_semester || '-' || end_semester || '): ' || credits
		 || '-' || credits_max || ' LP, rows give ' || min_credits || '-' || max_credits FROM plan_total
		 WHERE credits_max < min_credits - 0.01 OR credits > max_credits + 0.01 ORDER BY 1`)
	v.count("plans whose regulation prints a span instead of a sum", StatusInfo,
		"SELECT COUNT(DISTINCT program_id) FROM plan_total WHERE credits_max - credits > 0.01", "")
	v.count("plan totals whose row count does not match their rows", StatusFail, `
		SELECT COUNT(*) FROM plan_total t
		WHERE t.entry_count <> (SELECT COUNT(*) FROM plan_total_entry te WHERE te.program_id = t.program_id AND te.total_ord = t.ord)`, "")
	v.count("plans that state what they add up to", StatusInfo,
		"SELECT COUNT(DISTINCT program_id) FROM plan_total WHERE scope = 'plan'", "")
	v.count("groups of rows a plan ties to one sum", StatusInfo,
		"SELECT COUNT(*) FROM plan_total WHERE is_choice = 1", "")

	// Conflicts between sources. Two *stated* kinds for one pair are a real disagreement;
	// the precedence rule decides, and this list is what a human should look at.
	const kindConflict = `
		FROM program_module_assertion a
		JOIN program_module_assertion b ON b.program_id = a.program_id AND b.module_id = a.module_id
		WHERE a.kind_basis = 'stated' AND b.kind_basis = 'stated' AND a.source < b.source AND a.kind <> b.kind`
	v.count("pairs whose sources state different kinds", StatusWarn,
		"SELECT COUNT(*) FROM (SELECT DISTINCT a.program_id, a.module_id"+kindConflict+")",
		"SELECT a.source || '=' || a.kind || ' vs ' || b.source || '=' || b.kind || '  ×' || COUNT(DISTINCT a.program_id || a.module_id)"+kindConflict+" GROUP BY a.source, a.kind, b.source, b.kind ORDER BY COUNT(DISTINCT a.program_id || a.module_id) DESC")
	v.count("plan credits differ from catalog credits", StatusInfo, "SELECT COUNT(*) FROM v_program_plan_entry WHERE credits_differ_from_catalog = 1", "")

	v.count("curricular pairs only the tree states", StatusInfo, "SELECT COUNT(*) FROM program_module WHERE relation = 'curricular' AND in_tree = 1 AND on_module_page = 0", "")
	v.count("curricular pairs only the module page states", StatusInfo, "SELECT COUNT(*) FROM program_module WHERE relation = 'curricular' AND in_tree = 0 AND in_plan = 0", "")
	v.count("curricular pairs without any stated kind", StatusInfo, "SELECT COUNT(*) FROM program_module WHERE relation = 'curricular' AND kind IS NULL", "")
	v.count("FÜS modules whose page admits no program", StatusInfo,
		"SELECT COUNT(*) FROM module m WHERE m.is_fues = 1 AND NOT EXISTS (SELECT 1 FROM program_module pm WHERE pm.module_id = m.id)", "")
	v.count("programs without a short degree label", StatusInfo, "SELECT COUNT(*) FROM program WHERE degree_label IS NULL AND degree_level IN ('bachelor', 'master')", "")
	v.count("events", StatusInfo, "SELECT COUNT(*) FROM event", "")

	for _, b := range baselines {
		v.baseline(b)
	}
	return v.checks, v.err
}

// LogChecks writes the outcome of Validate to the operational log: one ERROR per
// failed check (validate.check_failed), one WARN per warning (validate.check_warned)
// and a summary (validate.finished). It returns the number of failed checks.
func LogChecks(checks []Check) int {
	log := oplog.For("validate")
	failed, warned := 0, 0
	for _, c := range checks {
		switch c.Status {
		case StatusFail:
			failed++
			log.Error("check failed", "event", "validate.check_failed", "check", c.Name, "value", c.Value, "detail", c.Detail, "samples", c.Samples)
		case StatusWarn:
			warned++
			log.Warn("check warns", "event", "validate.check_warned", "check", c.Name, "value", c.Value, "samples", c.Samples)
		}
	}
	if failed > 0 {
		log.Error("validation failed; no snapshot will be published from this data", "event", "validate.finished", "checks", len(checks), "failed", failed, "warned", warned)
	} else {
		log.Info("validation passed", "event", "validate.finished", "checks", len(checks), "failed", 0, "warned", warned)
	}
	return failed
}

// HasFailures reports whether any check failed.
func HasFailures(checks []Check) bool {
	for _, c := range checks {
		if c.Status == StatusFail {
			return true
		}
	}
	return false
}

type validator struct {
	ctx    context.Context
	db     *sql.DB
	checks []Check
	err    error
}

// count adds a check that is fine at 0. severity says what a non-zero count means.
func (v *validator) count(name, severity, query, sampleQuery string) {
	if v.err != nil {
		return
	}
	var n int64
	if err := v.db.QueryRowContext(v.ctx, query).Scan(&n); err != nil {
		v.err = fmt.Errorf("check %q: %w", name, err)
		return
	}
	c := Check{Name: name, Value: n, Status: StatusOK}
	if severity == StatusInfo {
		c.Status = StatusInfo
	} else if n > 0 {
		c.Status = severity
		if sampleQuery != "" {
			c.Samples = v.samples(sampleQuery)
		}
	}
	v.checks = append(v.checks, c)
}

// blockedAbbreviations fails on a derived abbreviation that abbrev.Blocked refuses (KKK, SS,
// a building of short room names …): the same list the derivation reads. An override line may
// name a blocked form on purpose, so is_override = 1 rows are left out, but not its twins: their
// letter suffix (NP-d, read as NPD) is derived.
func (v *validator) blockedAbbreviations() {
	const name = "no derived abbreviation is on the blocked list"
	if v.err != nil {
		return
	}
	rows, err := v.db.QueryContext(v.ctx, `
		SELECT '', a.module_id, a.abbrev, m.title FROM module_abbrev a JOIN module m ON m.id = a.module_id WHERE a.is_override = 0
		UNION ALL
		SELECT a.program_id, a.module_id, a.abbrev, m.title FROM program_module_abbrev a JOIN module m ON m.id = a.module_id
		WHERE a.is_override = 0 OR a.is_twin = 1
		ORDER BY 1, 2`)
	if err != nil {
		v.err = fmt.Errorf("check %q: %w", name, err)
		return
	}
	defer rows.Close()
	c := Check{Name: name, Status: StatusOK}
	for rows.Next() {
		var program, module, form, title string
		if err := rows.Scan(&program, &module, &form, &title); err != nil {
			v.err = fmt.Errorf("check %q: %w", name, err)
			return
		}
		if why, blocked := abbrev.Blocked(form, title); blocked {
			c.Value++
			if len(c.Samples) < 8 {
				c.Samples = append(c.Samples, strings.TrimSpace(program+" "+module+" "+form+" ("+why+")"))
			}
		}
	}
	if err := rows.Err(); err != nil {
		v.err = fmt.Errorf("check %q: %w", name, err)
		return
	}
	if c.Value > 0 {
		c.Status = StatusFail
	}
	v.checks = append(v.checks, c)
}

func (v *validator) baseline(b Baseline) {
	if v.err != nil {
		return
	}
	var n int64
	if err := v.db.QueryRowContext(v.ctx, b.Query).Scan(&n); err != nil {
		v.err = fmt.Errorf("baseline %q: %w", b.Name, err)
		return
	}
	c := Check{Name: "baseline: " + b.Name, Value: n, Status: StatusOK, Detail: fmt.Sprintf("minimum %d", b.Min)}
	if n < b.Min {
		c.Status = StatusFail
	}
	v.checks = append(v.checks, c)
}

func (v *validator) samples(query string) []string {
	rows, err := v.db.QueryContext(v.ctx, query+" LIMIT 8")
	if err != nil {
		return []string{"(sample query failed: " + err.Error() + ")"}
	}
	defer rows.Close()
	var result []string
	for rows.Next() {
		var s sql.NullString
		if rows.Scan(&s) == nil {
			result = append(result, s.String)
		}
	}
	return result
}

// emptyStrings checks every TEXT column of the canonical tables: unknown is NULL.
func (v *validator) emptyStrings() {
	if v.err != nil {
		return
	}
	rows, err := v.db.QueryContext(v.ctx, `
		SELECT m.name, p.name FROM sqlite_master m JOIN pragma_table_info(m.name) p
		WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%' AND m.name <> 'raw_page' AND UPPER(p.type) = 'TEXT'
		ORDER BY 1, 2`)
	if err != nil {
		v.err = err
		return
	}
	var columns [][2]string
	for rows.Next() {
		var table, column string
		if err := rows.Scan(&table, &column); err != nil {
			rows.Close()
			v.err = err
			return
		}
		columns = append(columns, [2]string{table, column})
	}
	rows.Close()

	c := Check{Name: "no empty-string or '-' placeholders in text columns", Status: StatusOK}
	for _, tc := range columns {
		var n int64
		q := fmt.Sprintf(`SELECT COUNT(*) FROM "%s" WHERE TRIM("%s") IN ('', '-')`, tc[0], tc[1])
		if err := v.db.QueryRowContext(v.ctx, q).Scan(&n); err != nil {
			v.err = err
			return
		}
		if n > 0 {
			c.Value += n
			c.Status = StatusFail
			c.Samples = append(c.Samples, fmt.Sprintf("%s.%s ×%d", tc[0], tc[1], n))
		}
	}
	if len(c.Samples) > 8 {
		c.Samples = append(c.Samples[:8], fmt.Sprintf("… and %d more columns", len(c.Samples)-8))
	}
	c.Detail = strings.TrimSpace(fmt.Sprintf("%d text columns checked", len(columns)))
	v.checks = append(v.checks, c)
}
