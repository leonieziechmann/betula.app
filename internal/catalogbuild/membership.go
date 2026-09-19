package catalogbuild

import (
	"database/sql"
	"sort"
	"strings"

	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/normalize"
	"github.com/jakob/btu-scraper/internal/parser"
)

// writePageAssignments stores what each module page lists under „Zuordnung zu
// Studiengängen" and resolves every triple to a program. A triple is resolved
// only when program name, degree and PO version identify exactly one program;
// anything else is kept as 'unresolved' and reported, never guessed.
func (b *builder) writePageAssignments() error {
	// Exact index: name + level + type + variant. Loose index: without the type,
	// because QIS does not always state it in both languages („Master - Duales
	// Studium, praxisintegrierend" vs "Master (research-oriented) - Co-Op Programme …").
	index := make(map[string][]*program)
	loose := make(map[string][]*program)
	for _, p := range b.programs {
		key := foldName(p.name) + "|" + p.degree.Key()
		index[key] = append(index[key], p)
		looseKey := foldName(p.name) + "|" + p.degree.Level + "|" + p.degree.Variant
		loose[looseKey] = append(loose[looseKey], p)
	}
	candidatesFor := func(name string, degree normalize.DegreeInfo) []*program {
		if exact := index[foldName(name)+"|"+degree.Key()]; len(exact) > 0 {
			return exact
		}
		var result []*program
		for _, p := range loose[foldName(name)+"|"+degree.Level+"|"+degree.Variant] {
			if p.degree.Type == "" || degree.Type == "" {
				result = append(result, p)
			}
		}
		return result
	}

	ids := make([]string, 0, len(b.src.modulePages))
	for id := range b.src.modulePages {
		ids = append(ids, id)
	}
	sort.Strings(ids)

	for _, moduleID := range ids {
		d := b.src.modulePages[moduleID].detail
		statements := parseRemarkStatements(d.Remarks)
		asserted := make(map[string]bool)

		ord := 0
		for _, ref := range d.StudyPrograms {
			if ref.Program == "" || parser.IsNoAssignment(ref.Raw) {
				continue
			}
			ord++
			b.report.PageRefs++

			degree := normalize.Degree(ref.Degree)
			var resolved *program
			status := "unresolved"
			if degree.Level == normalize.LevelAbroad {
				status = "abroad"
				b.report.PageRefsAbroad++
			} else if resolved = resolveProgram(candidatesFor(ref.Program, degree), ref); resolved != nil {
				status = "resolved"
			} else {
				b.report.PageRefsUnresolved[ref.Raw]++
			}

			var programID any
			if resolved != nil {
				programID = resolved.id
			}
			if _, err := b.tx.Exec(`
				INSERT INTO module_program_ref (module_id, ord, degree_raw, program_raw, po_raw, program_id, resolve_status)
				VALUES (?, ?, ?, ?, ?, ?, ?)`,
				moduleID, ord, ref.Degree, ref.Program, ref.Regulation, programID, status); err != nil {
				return err
			}

			if resolved == nil || asserted[resolved.id] {
				continue
			}
			asserted[resolved.id] = true
			kind, area := statementFor(statements, resolved)
			if err := b.insertAssertion(resolved.id, moduleID, "module_page", 0, area, kind, "stated"); err != nil {
				return err
			}
		}
	}
	return nil
}

func foldName(s string) string {
	return strings.Join(strings.Fields(strings.ToLower(s)), " ")
}

// resolveProgram picks the PO version a module page means. Pages sometimes name
// only the base of a PO („PO 2024" for „2024 - NF 2026"); that is accepted when
// it fits exactly one PO version of the program.
func resolveProgram(candidates []*program, ref model.StudyProgram) *program {
	if len(candidates) == 0 {
		return nil
	}
	po := strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(ref.Regulation), "PO "))
	for _, p := range candidates {
		if p.poVersion == po {
			return p
		}
	}

	var match *program
	for _, p := range candidates {
		if po != "" && (strings.HasPrefix(p.poVersion, po) || strings.HasPrefix(po, p.poVersion)) {
			if match != nil {
				return nil
			}
			match = p
		}
	}
	if match != nil {
		return match
	}

	year, _ := normalize.POVersion(po)
	for _, p := range candidates {
		if year != 0 && p.poYear == year {
			if match != nil {
				return nil
			}
			match = p
		}
	}
	return match
}

// statementFor finds what the module's remarks say about a program. Remarks name a
// program by its name and short degree („Informatik B.Sc."), not by PO version.
func statementFor(statements []remarkStatement, p *program) (kind, area string) {
	name := foldName(strings.TrimSuffix(p.name, " - dual"))
	for _, st := range statements {
		if foldName(st.programName) != name || !normalize.LabelMatchesLevel(st.degreeLabel, p.degree.Level) {
			continue
		}
		return st.kind, st.area
	}
	return "", ""
}

// writePlanAssertions turns the validated plan entries into membership statements.
// The plan tables themselves are a source and are not modified by the build.
func (b *builder) writePlanAssertions() error {
	rows, err := b.tx.Query(`
		SELECT e.program_id, e.module_id, COALESCE(e.subject_area, ''), COALESCE(e.kind, '')
		FROM plan_entry e
		WHERE e.module_id IS NOT NULL
		ORDER BY e.program_id, e.ord`)
	if err != nil {
		return err
	}
	type entry struct{ programID, moduleID, area, kind string }
	var entries []entry
	for rows.Next() {
		var e entry
		if err := rows.Scan(&e.programID, &e.moduleID, &e.area, &e.kind); err != nil {
			rows.Close()
			return err
		}
		entries = append(entries, e)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	seen := make(map[entry]bool)
	for _, e := range entries {
		if b.programByID[e.programID] == nil {
			continue
		}
		if !b.moduleIDs[e.moduleID] {
			b.report.PlanEntriesUnknownModule++
			continue
		}
		if seen[e] {
			continue
		}
		seen[e] = true
		if err := b.insertAssertion(e.programID, e.moduleID, "pdf_plan", 0, e.area, e.kind, "stated"); err != nil {
			return err
		}
	}

	planRows, err := b.tx.Query("SELECT program_id FROM plan ORDER BY program_id")
	if err != nil {
		return err
	}
	defer planRows.Close()
	for planRows.Next() {
		var id sql.NullString
		if err := planRows.Scan(&id); err != nil {
			return err
		}
		if b.programByID[id.String] == nil {
			b.report.PlansWithoutProgram = append(b.report.PlansWithoutProgram, id.String)
		}
	}
	return planRows.Err()
}
