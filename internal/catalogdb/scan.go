package catalogdb

import (
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/model"
)

// ScanPrograms returns the programs a study plan scan can work on, with their
// regulation documents. nameFilter and degreeFilter are case-insensitive substrings
// (matched here, not in SQL: SQLite only folds ASCII); programID selects one program.
func (db *DB) ScanPrograms(nameFilter, degreeFilter, programID string) ([]model.OfficialStudyProgram, error) {
	var programs []model.OfficialStudyProgram
	index := make(map[string]int)
	err := queryRows(db.sql, `
		SELECT id, name, stg_code, degree_raw, abschl_code, po_version, source_url, fetched_at
		FROM program ORDER BY name, degree_raw, po_version`, nil,
		func(scan func(...any) error) error {
			var p model.OfficialStudyProgram
			var fetchedAt string
			if err := scan(&p.ID, &p.ProgramName, &p.ProgramCode, &p.Degree, &p.DegreeCode, &p.POVersion, &p.QISURL, &fetchedAt); err != nil {
				return err
			}
			if programID != "" && p.ID != programID {
				return nil
			}
			if nameFilter != "" && !strings.Contains(strings.ToLower(p.ProgramName), strings.ToLower(nameFilter)) {
				return nil
			}
			if degreeFilter != "" && !strings.Contains(strings.ToLower(p.Degree), strings.ToLower(degreeFilter)) {
				return nil
			}
			p.ScrapedAt, _ = time.Parse(time.RFC3339, fetchedAt)
			index[p.ID] = len(programs)
			programs = append(programs, p)
			return nil
		})
	if err != nil {
		return nil, err
	}

	err = queryRows(db.sql, "SELECT program_id, title, doc_type, url FROM program_document ORDER BY program_id, ord", nil,
		func(scan func(...any) error) error {
			var programID string
			var d model.ProgramRegulationDocument
			if err := scan(&programID, &d.Title, &d.DocType, &d.URL); err != nil {
				return err
			}
			if i, ok := index[programID]; ok {
				programs[i].Documents = append(programs[i].Documents, d)
			}
			return nil
		})
	return programs, err
}

// ScanCatalog is the module catalog as the study plan validation needs it: identity
// (number, both titles) and the fields of the plausibility checks.
func (db *DB) ScanCatalog() ([]model.CurriculumCatalogModule, error) {
	var catalog []model.CurriculumCatalogModule
	err := queryRows(db.sql, `
		SELECT id, COALESCE(title_de, title, ''), COALESCE(title_en, ''), COALESCE(turnus_raw, ''),
		       COALESCE(duration_raw, ''), COALESCE(credits, 0)
		FROM module ORDER BY id`, nil,
		func(scan func(...any) error) error {
			var m model.CurriculumCatalogModule
			if err := scan(&m.ID, &m.TitleDE, &m.TitleEN, &m.Turnus, &m.Duration, &m.Credits); err != nil {
				return err
			}
			m.Code = m.ID
			catalog = append(catalog, m)
			return nil
		})
	return catalog, err
}

// ScanProgramClaims is what each program's own pages say belongs to it: the
// module pages that name the program and the modules under its QIS tree. What a
// study plan linked is left out, so the claim stays evidence of its own and a
// plan can never confirm itself.
func (db *DB) ScanProgramClaims() (map[string]map[string]bool, error) {
	claims := map[string]map[string]bool{}
	err := queryRows(db.sql, `
		SELECT program_id, module_id FROM program_module
		WHERE in_tree = 1 OR on_module_page = 1`, nil,
		func(scan func(...any) error) error {
			var program, module string
			if err := scan(&program, &module); err != nil {
				return err
			}
			if claims[program] == nil {
				claims[program] = map[string]bool{}
			}
			claims[program][module] = true
			return nil
		})
	return claims, err
}

// HasPlan reports whether a validated plan is stored for the program.
func (db *DB) HasPlan(programID string) (bool, error) {
	var exists bool
	err := db.sql.QueryRow("SELECT EXISTS(SELECT 1 FROM plan WHERE program_id = ?)", programID).Scan(&exists)
	return exists, err
}

// PlanFromModules converts the rows of a validated extraction, and the sums the
// regulation prints over them, into a plan.
func PlanFromModules(programID, sourceFile, layoutJSON string, modules []model.CurriculumModule, totals []model.CurriculumTotal) Plan {
	p := Plan{ProgramID: programID, SourceFile: sourceFile, LayoutJSON: layoutJSON}
	p.SourcePages, p.SourceLabel = planLocation(modules)
	for _, m := range modules {
		p.Entries = append(p.Entries, PlanEntry{
			ModuleID: m.ModuleID, ModuleCodeRaw: m.ModuleCode, ModuleName: m.ModuleName,
			Semester: m.RecommendedSemester, StartSemester: m.StartSemester, EndSemester: m.EndSemester,
			SemesterSpan: m.SemesterSpan, Credits: m.Credits, MinCredits: m.MinCredits, MaxCredits: m.MaxCredits,
			KindRaw: m.ModuleType, StudySection: m.StudySection, SubjectArea: m.SubjectArea,
			AreaRules: m.AreaRules, Specialization: m.Specialization, SourceEvidence: m.SourceEvidence,
			SourcePage: m.SourcePage,
		})
	}
	for _, t := range totals {
		p.Totals = append(p.Totals, PlanTotal{
			Label: t.Label, Scope: t.Scope, Specialization: t.Specialization,
			StartSemester: t.StartSemester, EndSemester: t.EndSemester,
			Credits: t.Credits, CreditsMax: t.CreditsMax, MinCredits: t.MinCredits, MaxCredits: t.MaxCredits,
			IsChoice: t.IsChoice, SourceEvidence: t.SourceEvidence, Entries: t.Entries,
		})
	}
	return p
}

// planLocation says where in the regulation the plan stands: the pages its rows
// were read from, written as a reader would („9", „9–11", „9, 13"), and the
// heading the plan stands under where the document prints one.
//
// A plan continued across a page break has rows on consecutive pages, which
// reads as a range; a document that prints a plan in two places keeps both
// pages listed rather than spanning the text in between.
func planLocation(modules []model.CurriculumModule) (pages, label string) {
	seen := map[int]bool{}
	var list []int
	for _, m := range modules {
		if m.SourcePage > 0 && !seen[m.SourcePage] {
			seen[m.SourcePage] = true
			list = append(list, m.SourcePage)
		}
		if label == "" {
			label = m.SourcePlanLabel
		}
	}
	sort.Ints(list)
	var parts []string
	for i := 0; i < len(list); {
		j := i
		for j+1 < len(list) && list[j+1] == list[j]+1 {
			j++
		}
		switch {
		case j == i:
			parts = append(parts, strconv.Itoa(list[i]))
		case j == i+1:
			parts = append(parts, strconv.Itoa(list[i])+", "+strconv.Itoa(list[j]))
		default:
			parts = append(parts, strconv.Itoa(list[i])+"\u2013"+strconv.Itoa(list[j]))
		}
		i = j + 1
	}
	return strings.Join(parts, ", "), label
}
