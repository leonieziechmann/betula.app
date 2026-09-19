// Package catalogbuild derives the canonical tables of the schema v2 database
// from its raw page archive. The build never touches the network and is
// deterministic: the same archive always yields the same canonical data, and a
// parser or normalization fix takes effect by simply building again.
package catalogbuild

import (
	"context"
	"database/sql"
	"fmt"
	"sort"
	"time"

	"github.com/jakob/btu-scraper/internal/catalogdb"
)

// derivedTables are replaced as a whole by every build, children first.
// plan, plan_entry and plan_scan_status are a source of their own and stay.
var derivedTables = []string{
	"module_facet", "program_module",
	"module_event", "event_date", "event_person", "event_form", "event", "semester",
	"program_module_assertion", "module_program_ref",
	"program_area", "program_document", "program",
	"module_successor", "module_prerequisite", "module_text_item",
	"module_teaching_form", "module_person", "module", "department",
	"meta",
}

// Report says what the build found. Everything that was dropped or left
// unresolved is counted here instead of disappearing silently.
type Report struct {
	BuiltAt time.Time

	Modules            int
	ModulesWithoutPage int // known from a list, but no module page in the archive
	Departments        int
	UnpairedEnglishDep []string // English department names without a German counterpart

	Programs           int
	ProgramsWithLabel  int // short degree label (B.Sc. …) stated by a source
	Areas              int
	TreeLeaves         int
	TreeLeavesNoModule map[string]int // module IDs in the tree that are not in the catalog → leaf count
	MissingTreePages   int            // area pages missing from the archive

	PageRefs           int
	PageRefsAbroad     int
	PageRefsUnresolved map[string]int // "degree / program / PO" → count

	Assertions map[string]int // source → rows

	PlanEntriesUnknownModule int
	PlansWithoutProgram      []string

	Events              int
	EventLinksNoArchive int // events a module page links that are not archived yet
}

// Build replaces all derived tables from the raw archive, in one transaction.
func Build(ctx context.Context, db *catalogdb.DB) (*Report, error) {
	report := &Report{
		BuiltAt:            time.Now().UTC().Truncate(time.Second),
		TreeLeavesNoModule: make(map[string]int),
		PageRefsUnresolved: make(map[string]int),
		Assertions:         make(map[string]int),
	}

	// Parse everything before the transaction starts: parsing is the slow part
	// and must not hold the write lock.
	src, err := loadSources(ctx, db, report)
	if err != nil {
		return nil, err
	}

	tx, err := db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()

	for _, table := range derivedTables {
		if _, err := tx.Exec("DELETE FROM " + table); err != nil {
			return nil, fmt.Errorf("failed to clear %s: %w", table, err)
		}
	}

	b := &builder{tx: tx, src: src, report: report}
	steps := []struct {
		name string
		run  func() error
	}{
		{"departments", b.writeDepartments},
		{"modules", b.writeModules},
		{"module links", b.writeModuleLinks},
		{"programs", b.writePrograms},
		{"module page assignments", b.writePageAssignments},
		{"plan assertions", b.writePlanAssertions},
		{"events", b.writeEvents},
		{"materialized views", b.materialize},
		{"meta", b.writeMeta},
	}
	for _, step := range steps {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		if err := step.run(); err != nil {
			return nil, fmt.Errorf("build step %q failed: %w", step.name, err)
		}
	}

	if err := foreignKeyCheck(tx); err != nil {
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return report, nil
}

type builder struct {
	tx     *sql.Tx
	src    *sources
	report *Report

	departmentIDs map[string]int64 // department_raw → department.id
	moduleIDs     map[string]bool  // modules written
	programs      []*program       // programs written
	programByID   map[string]*program
}

// materialize evaluates the two expensive *_src views once per build. The public
// views read the resulting tables, so the browser never has to run that logic.
func (b *builder) materialize() error {
	for table, view := range map[string]string{"program_module": "v_program_module_src", "module_facet": "v_module_facets_src"} {
		if _, err := b.tx.Exec("INSERT INTO " + table + " SELECT * FROM " + view); err != nil {
			return fmt.Errorf("%s: %w", table, err)
		}
	}
	return nil
}

// foreignKeyCheck fails the build instead of committing dangling references.
func foreignKeyCheck(tx *sql.Tx) error {
	rows, err := tx.Query("PRAGMA foreign_key_check")
	if err != nil {
		return err
	}
	defer rows.Close()

	var violations []string
	for rows.Next() {
		var table, parent string
		var rowid sql.NullInt64
		var fkid int
		if err := rows.Scan(&table, &rowid, &parent, &fkid); err != nil {
			return err
		}
		if len(violations) < 5 {
			violations = append(violations, fmt.Sprintf("%s → %s", table, parent))
		}
	}
	if err := rows.Err(); err != nil {
		return err
	}
	if len(violations) > 0 {
		return fmt.Errorf("foreign key violations after build, e.g. %v", violations)
	}
	return nil
}

func (b *builder) writeMeta() error {
	meta := map[string]string{
		"built_at":         b.report.BuiltAt.Format(time.RFC3339),
		"current_semester": currentSemesterKey(b.report.BuiltAt),
	}
	rows, err := b.tx.Query("SELECT source, MIN(fetched_at), MAX(fetched_at), COUNT(*) FROM raw_page WHERE http_status = 200 GROUP BY source")
	if err != nil {
		return err
	}
	for rows.Next() {
		var source, oldest, newest string
		var n int
		if err := rows.Scan(&source, &oldest, &newest, &n); err != nil {
			rows.Close()
			return err
		}
		meta["source."+source+".oldest_fetch"] = oldest
		meta["source."+source+".newest_fetch"] = newest
		meta["source."+source+".pages"] = fmt.Sprint(n)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	keys := make([]string, 0, len(meta))
	for k := range meta {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		if _, err := b.tx.Exec("INSERT INTO meta (key, value) VALUES (?, ?)", k, meta[k]); err != nil {
			return err
		}
	}
	return nil
}

// currentSemesterKey: summer semester runs April to September, winter October to March.
func currentSemesterKey(now time.Time) string {
	now = now.In(time.FixedZone("CET", 3600))
	switch m := now.Month(); {
	case m >= time.April && m <= time.September:
		return fmt.Sprintf("%dS", now.Year())
	case m >= time.October:
		return fmt.Sprintf("%dW", now.Year())
	default:
		return fmt.Sprintf("%dW", now.Year()-1)
	}
}

// null maps the zero value to SQL NULL: unknown is never stored as an empty string or 0.
func null[T comparable](v T) any {
	var zero T
	if v == zero {
		return nil
	}
	return v
}

func boolInt(v bool) int {
	if v {
		return 1
	}
	return 0
}
