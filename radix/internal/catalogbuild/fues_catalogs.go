package catalogbuild

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/fuescatalog"
)

// fuesCatalog is a FÜS catalog as archived and read.
type fuesCatalog struct {
	url       string
	fetchedAt time.Time
	catalog   *fuescatalog.Catalog
}

// readFUESCatalog reads a catalog's PDF; the tests read rows instead.
var readFUESCatalog = func(body []byte) (*fuescatalog.Catalog, error) {
	rows, err := fuescatalog.ReadRows(body)
	if err != nil {
		return nil, err
	}
	return fuescatalog.Parse(rows)
}

// loadFUESCatalogs reads the archived FÜS catalogs (docs/radix/data-sources.md §19). A catalog the
// server answered with anything but its content (a 404: not published yet) is none; one that
// cannot be read is left out and reported (build.fues_catalogs_unreadable), and the build goes on:
// the FÜS lists then follow the catalogs it could read, or the module pages without any. Two
// copies of one semester's catalog: the one fetched last.
func loadFUESCatalogs(db *catalogdb.DB, src *sources, report *Report) error {
	bySemester := map[string]fuesCatalog{}
	err := db.EachPage(catalogdb.SourceBTUFUESCatalog, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		catalog, err := readFUESCatalog(p.Body)
		if err != nil {
			report.FUESCatalogsUnreadable = append(report.FUESCatalogsUnreadable, fmt.Sprintf("%s (%s): %v", p.Key, p.URL, err))
			return nil
		}
		if have, ok := bySemester[catalog.Semester]; ok && have.fetchedAt.After(p.FetchedAt) {
			return nil
		}
		bySemester[catalog.Semester] = fuesCatalog{url: p.URL, fetchedAt: p.FetchedAt, catalog: catalog}
		return nil
	})
	if err != nil {
		return err
	}
	semesters := make([]string, 0, len(bySemester))
	for s := range bySemester {
		semesters = append(semesters, s)
	}
	sort.Strings(semesters)
	for _, s := range semesters {
		src.fuesCatalogs = append(src.fuesCatalogs, bySemester[s])
	}
	return nil
}

// writeFUESCatalogs writes what the catalogs say (docs/radix/schema-v2.md, „FÜS catalogs"): their
// modules; the programs each heading names; the lines of the negative lists, a part for a degree
// or a profile applied to the programs it holds for, a part for what a student chooses kept as
// its scope. A module the module lists do not hold is left out and reported
// (build.fues_unknown_modules); so is a heading that names no program (fues_catalog_heading,
// which validate warns of).
func (b *builder) writeFUESCatalogs() error {
	if len(b.src.fuesCatalogs) == 0 {
		return nil
	}
	known := map[string]bool{}
	rows, err := b.tx.Query("SELECT id FROM module")
	if err != nil {
		return err
	}
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			rows.Close()
			return err
		}
		known[id] = true
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}
	match := newProgramMatcher(b.programs)

	for _, c := range b.src.fuesCatalogs {
		cat := c.catalog
		var modules []fuescatalog.Module
		for _, m := range cat.Modules {
			if known[m.ID] {
				modules = append(modules, m)
			} else {
				b.report.FUESModulesUnknown[m.ID]++
			}
		}
		if _, err := b.tx.Exec(`INSERT INTO fues_catalog_document (semester, issued, modules, url, fetched_at) VALUES (?, ?, ?, ?, ?)`,
			cat.Semester, null(cat.Issued), len(modules), c.url, c.fetchedAt.UTC().Format(time.RFC3339)); err != nil {
			return fmt.Errorf("FÜS catalog %s: %w", cat.Semester, err)
		}
		b.report.FUESCatalogs++
		for _, m := range modules {
			if _, err := b.tx.Exec(`INSERT INTO fues_catalog (semester, module_id, campus) VALUES (?, ?, ?)`, cat.Semester, m.ID, null(m.Campus)); err != nil {
				return fmt.Errorf("FÜS catalog %s, module %s: %w", cat.Semester, m.ID, err)
			}
		}

		// The headings: those of its list of programs, and any a negative list uses beside them.
		headings := append([]string(nil), cat.Programs...)
		seen := map[string]bool{}
		for _, h := range headings {
			seen[h] = true
		}
		for _, e := range cat.Exclusions {
			if !seen[e.Program] {
				seen[e.Program] = true
				headings = append(headings, e.Program)
			}
		}
		notes := map[string]string{}
		for _, n := range cat.Notes {
			if n.Scope == "" {
				notes[n.Program] = n.Text
			}
		}
		named := map[string]bool{}
		for _, h := range headings {
			programs := match.programs(h)
			if _, err := b.tx.Exec(`INSERT INTO fues_catalog_heading (semester, heading, programs) VALUES (?, ?, ?)`, cat.Semester, h, len(programs)); err != nil {
				return fmt.Errorf("FÜS catalog %s, heading %q: %w", cat.Semester, h, err)
			}
			for _, p := range programs {
				if named[p.id] {
					continue
				}
				named[p.id] = true
				if _, err := b.tx.Exec(`INSERT INTO fues_catalog_program (semester, program_id, heading, note) VALUES (?, ?, ?, ?)`,
					cat.Semester, p.id, h, null(notes[h])); err != nil {
					return fmt.Errorf("FÜS catalog %s, program %s: %w", cat.Semester, p.id, err)
				}
			}
		}

		for _, e := range cat.Exclusions {
			if !known[e.ModuleID] {
				b.report.FUESModulesUnknown[e.ModuleID]++
				continue
			}
			kind := fuescatalog.ScopeKind(e.Scope)
			for _, p := range match.programs(e.Program) {
				scope := ""
				switch kind {
				case fuescatalog.ScopeChoice:
					scope = fuescatalog.ChoiceLabel(e.Scope)
				case fuescatalog.ScopeBachelor:
					if p.degree.Level != "bachelor" {
						continue
					}
				case fuescatalog.ScopeMaster:
					if p.degree.Level != "master" {
						continue
					}
				case fuescatalog.ScopeProfile:
					if !fuescatalog.ProfileHolds(e.Scope, p.degree.Type, strings.HasPrefix(p.degree.Variant, "dual")) {
						continue
					}
				}
				res, err := b.tx.Exec(`INSERT OR IGNORE INTO fues_negative (semester, program_id, module_id, scope) VALUES (?, ?, ?, ?)`,
					cat.Semester, p.id, e.ModuleID, scope)
				if err != nil {
					return fmt.Errorf("FÜS catalog %s, negative list of %s: %w", cat.Semester, p.id, err)
				}
				if n, _ := res.RowsAffected(); n > 0 {
					b.report.FUESNegatives++
				}
			}
		}
	}
	return nil
}

// programMatcher finds the programs a catalog's heading names: a program's name as it stands, or
// without „ - dual" (the heading names both variants); else each part of a heading that names
// several („Städtebau und Stadtplanung/ Stadtplanung und Städtebau; Stadt- und Regionalplanung");
// else the programs whose name starts with it („Lehramt Primarstufe" for each of its subjects);
// else the one name that differs from it by a slip of two letters at most („Hybrid Electric
// Propulsation Technology", „… Revitalisation").
type programMatcher struct {
	byName map[string][]*program // folded name, and folded name without „ - dual"
	names  []string
}

func newProgramMatcher(programs []*program) *programMatcher {
	m := &programMatcher{byName: map[string][]*program{}}
	add := func(name string, p *program) {
		if _, ok := m.byName[name]; !ok {
			m.names = append(m.names, name)
		}
		m.byName[name] = append(m.byName[name], p)
	}
	for _, p := range programs {
		name := foldName(p.name)
		add(name, p)
		if base := strings.TrimSuffix(name, " - dual"); base != name {
			add(base, p)
		}
	}
	sort.Strings(m.names)
	return m
}

func (m *programMatcher) programs(heading string) []*program {
	h := foldName(heading)
	if ps := m.byName[h]; len(ps) > 0 {
		return unique(ps)
	}
	var found []*program
	parts := strings.FieldsFunc(h, func(r rune) bool { return r == '/' || r == ';' })
	if len(parts) > 1 {
		for _, part := range parts {
			found = append(found, m.exactOrPrefix(strings.TrimSpace(part))...)
		}
		if len(found) > 0 {
			return unique(found)
		}
	}
	if found = m.exactOrPrefix(h); len(found) > 0 {
		return unique(found)
	}
	if len([]rune(h)) >= 12 {
		best, bestName, tie := 3, "", false
		for _, name := range m.names {
			d := levenshtein(h, name)
			switch {
			case d < best:
				best, bestName, tie = d, name, false
			case d == best:
				tie = true
			}
		}
		if bestName != "" && !tie {
			return unique(m.byName[bestName])
		}
	}
	return nil
}

func (m *programMatcher) exactOrPrefix(name string) []*program {
	if ps := m.byName[name]; len(ps) > 0 {
		return ps
	}
	if len([]rune(name)) < 8 {
		return nil
	}
	var found []*program
	for _, n := range m.names {
		if strings.HasPrefix(n, name+" ") {
			found = append(found, m.byName[n]...)
		}
	}
	return found
}

func unique(ps []*program) []*program {
	seen := map[string]bool{}
	var out []*program
	for _, p := range ps {
		if !seen[p.id] {
			seen[p.id] = true
			out = append(out, p)
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].id < out[j].id })
	return out
}

func levenshtein(a, b string) int {
	ra, rb := []rune(a), []rune(b)
	prev := make([]int, len(rb)+1)
	cur := make([]int, len(rb)+1)
	for j := range prev {
		prev[j] = j
	}
	for i := 1; i <= len(ra); i++ {
		cur[0] = i
		for j := 1; j <= len(rb); j++ {
			cost := 1
			if ra[i-1] == rb[j-1] {
				cost = 0
			}
			cur[j] = min(prev[j]+1, cur[j-1]+1, prev[j-1]+cost)
		}
		prev, cur = cur, prev
	}
	return prev[len(rb)]
}
