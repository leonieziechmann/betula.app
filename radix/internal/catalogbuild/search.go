package catalogbuild

import (
	"database/sql"
	"fmt"
	"slices"
	"sort"
	"strings"

	"github.com/leonieziechmann/betula/radix/internal/abbrev"
	"github.com/leonieziechmann/betula/radix/internal/normalize"
)

// writeSearch folds what Folia's search compares a query with (docs/radix/schema-v2.md, „Search“):
// the titles of every module, their initials, and its abbreviations — its own, every other a
// program gives it, and the known short forms of words of its titles („bwl“ for a title with
// Betriebswirtschaftslehre). Only the names of a module, not the texts of its description (owner,
// 2026-09-30). It reads module and the abbreviations, so it runs after them.
func (b *builder) writeSearch() error {
	abbrevs := map[string][]string{}
	rows, err := b.tx.Query(`SELECT module_id, abbrev FROM module_abbrev
		UNION SELECT module_id, abbrev FROM program_module_abbrev ORDER BY 1, 2`)
	if err != nil {
		return fmt.Errorf("abbreviations: %w", err)
	}
	for rows.Next() {
		var id, a string
		if err := rows.Scan(&id, &a); err != nil {
			rows.Close()
			return err
		}
		abbrevs[id] = append(abbrevs[id], a)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	type module struct {
		id     string
		de, en sql.NullString
	}
	var modules []module
	rows, err = b.tx.Query("SELECT id, title_de, title_en FROM module ORDER BY id")
	if err != nil {
		return fmt.Errorf("modules: %w", err)
	}
	for rows.Next() {
		var m module
		if err := rows.Scan(&m.id, &m.de, &m.en); err != nil {
			rows.Close()
			return err
		}
		modules = append(modules, m)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	for _, m := range modules {
		var initials []string
		forms := map[string]bool{}
		for _, title := range []sql.NullString{m.de, m.en} {
			if !title.Valid {
				continue
			}
			if i := normalize.SearchInitials(title.String); i != "" && !slices.Contains(initials, i) {
				initials = append(initials, i)
			}
			for _, form := range abbrev.KnownFormsIn(title.String) {
				forms[normalize.SearchAbbrev(form)] = true
			}
		}
		for _, a := range abbrevs[m.id] {
			if folded := normalize.SearchAbbrev(a); folded != "" {
				forms[folded] = true
			}
		}
		folded := make([]string, 0, len(forms))
		for form := range forms {
			folded = append(folded, form)
		}
		sort.Strings(folded)

		if _, err := b.tx.Exec("INSERT INTO module_folded (module_id, title_de, title_en, initials, abbrevs) VALUES (?, ?, ?, ?, ?)",
			m.id, searchText(m.de), searchText(m.en), nullable(strings.Join(initials, " ")), nullable(strings.Join(folded, " "))); err != nil {
			return fmt.Errorf("module %s: %w", m.id, err)
		}
	}
	return nil
}

// searchText is normalize.SearchText of a title, NULL without one (or without a letter in it).
func searchText(title sql.NullString) any {
	if !title.Valid {
		return nil
	}
	return nullable(normalize.SearchText(title.String))
}

// nullable is NULL for "": unknown is NULL (validate: no empty strings in text columns).
func nullable(s string) any {
	if s == "" {
		return nil
	}
	return s
}
