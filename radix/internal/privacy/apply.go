package privacy

import (
	"context"
	"database/sql"
	"fmt"
	"sort"
	"strings"
)

// Placeholder stands where a name was taken out of a text.
const Placeholder = "[entfernt]"

// DB is what Apply and Find need of a database: a *sql.DB or a *sql.Tx.
type DB interface {
	QueryContext(ctx context.Context, query string, args ...any) (*sql.Rows, error)
	ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error)
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
}

// Table is one table Apply and Find go through: a table of the catalog (radix.db or a snapshot)
// or the history's copy of one (history.db), named by the catalog table it holds.
type Table struct {
	Name   string   // the table in the database: "module_person", "h_module_person"
	Source string   // the catalog table it holds: "module_person"
	Key    []string // the columns an UPDATE or DELETE finds a row by ("rowid" for the history)
	Text   []string // the columns whose text is searched
	Module string   // the column that names the module a row belongs to ("module_id"; "id" in module), or ""
	Event  string   // the column that names the event a row belongs to ("event_id"; "id" in event), or ""
}

// The lists of names: a person's row in them is taken out as a whole.
var nameLists = map[string]bool{"module_person": true, "event_person": true}

// Result counts what Apply did.
type Result struct {
	People   int            // in the register
	Rows     int            // rows of module_person and event_person taken out
	Values   int            // text values a name was taken out of
	Modules  int            // modules a person of the register was responsible for, as the lists said
	Events   int            // events a person of the register held, as the lists said
	Changed  map[string]int // table → its rows Apply changed (not those it deleted)
	ByPerson map[int64]int  // register id → the rows and values of that person it took out
	keys     map[string][][]any
}

// ChangedKeys returns the keys of the rows of table that Apply changed (not those it deleted).
func (r Result) ChangedKeys(table string) [][]any { return r.keys[table] }

// Hit is a place where Find found a person.
type Hit struct {
	Person  int64  // register id (0 for a person that is not in the register)
	Table   string // as Table.Name
	Key     string // the row's key, "col=value, …"
	Column  string
	Snippet string // the text around the name
	Row     bool   // the row is the person's entry in a list of names (module_person, event_person)
}

type scanned struct {
	key    []any
	values []sql.NullString
	module string
	event  string
}

// scan reads the key, the text columns and the module and event columns of every row of t.
func scan(ctx context.Context, db DB, t Table) ([]scanned, error) {
	cols := append(append([]string{}, t.Key...), t.Text...)
	modAt, evAt := -1, -1
	if t.Module != "" {
		modAt = len(cols)
		cols = append(cols, t.Module)
	}
	if t.Event != "" {
		evAt = len(cols)
		cols = append(cols, t.Event)
	}
	quoted := make([]string, len(cols))
	for i, c := range cols {
		quoted[i] = quote(c)
	}
	rows, err := db.QueryContext(ctx, "SELECT "+strings.Join(quoted, ", ")+" FROM "+quote(t.Name))
	if err != nil {
		return nil, fmt.Errorf("privacy: read %s: %w", t.Name, err)
	}
	defer rows.Close()
	var out []scanned
	for rows.Next() {
		key := make([]any, len(t.Key))
		values := make([]sql.NullString, len(t.Text))
		var module, event sql.NullString
		ptrs := make([]any, 0, len(cols))
		for i := range key {
			ptrs = append(ptrs, &key[i])
		}
		for i := range values {
			ptrs = append(ptrs, &values[i])
		}
		if modAt >= 0 {
			ptrs = append(ptrs, &module)
		}
		if evAt >= 0 {
			ptrs = append(ptrs, &event)
		}
		if err := rows.Scan(ptrs...); err != nil {
			return nil, fmt.Errorf("privacy: read %s: %w", t.Name, err)
		}
		out = append(out, scanned{key: key, values: values, module: module.String, event: event.String})
	}
	return out, rows.Err()
}

func quote(name string) string {
	if name == "rowid" {
		return name
	}
	return `"` + strings.ReplaceAll(name, `"`, `""`) + `"`
}

func where(key []string) string {
	parts := make([]string, len(key))
	for i, k := range key {
		parts[i] = quote(k) + " = ?"
	}
	return strings.Join(parts, " AND ")
}

func keyString(cols []string, key []any) string {
	parts := make([]string, len(cols))
	for i, c := range cols {
		v := key[i]
		if b, ok := v.([]byte); ok {
			v = string(b)
		}
		parts[i] = fmt.Sprintf("%s=%v", c, v)
	}
	return strings.Join(parts, ", ")
}

// pass is one run of Apply or Find over a set of tables.
type pass struct {
	people  []Person
	persons []*person
	modules []map[string]bool // per person: the modules whose lists of names hold them
	events  []map[string]bool
}

func newPass(people []Person) *pass {
	p := &pass{people: people}
	for _, person := range people {
		p.persons = append(p.persons, compile(person))
		p.modules = append(p.modules, map[string]bool{})
		p.events = append(p.events, map[string]bool{})
	}
	return p
}

// listed reports which persons a row of a list of names is (the whole row is theirs).
func (p *pass) listed(t Table, r scanned) []int {
	if !nameLists[t.Source] {
		return nil
	}
	at := -1
	for i, c := range t.Text {
		if c == "name" {
			at = i
		}
	}
	if at < 0 || !r.values[at].Valid {
		return nil
	}
	f := folded(r.values[at].String)
	var who []int
	for i, person := range p.persons {
		if person.strict.MatchString(f) {
			who = append(who, i)
		}
	}
	return who
}

// list finds the rows of a list of names that are persons of the register, before anything of
// the table is searched, and notes their modules and events: the surname alone then means them
// in every row of those, whatever the order of the rows.
func (p *pass) list(t Table, rows []scanned) [][]int {
	out := make([][]int, len(rows))
	for n, r := range rows {
		out[n] = p.listed(t, r)
		for _, i := range out[n] {
			if r.module != "" {
				p.modules[i][r.module] = true
			}
			if r.event != "" {
				p.events[i][r.event] = true
			}
		}
	}
	return out
}

// bare reports the persons whose surname alone means them in this row: it belongs to a module or
// an event whose lists of names hold them.
func (p *pass) bare(r scanned) []bool {
	out := make([]bool, len(p.persons))
	for i := range p.persons {
		out[i] = (r.module != "" && p.modules[i][r.module]) || (r.event != "" && p.events[i][r.event])
	}
	return out
}

// redact takes every mention of the register's persons out of s; ok is false when nothing was
// found. by counts the mentions per person.
func (p *pass) redact(s string, bare []bool, by []int) (string, bool) {
	changed := false
	for round := 0; round < 4; round++ { // a name the replacement made adjacent to another is found in the next round
		f, at := fold(s)
		var spans []span
		for i, person := range p.persons {
			found := person.mentions(f, bare[i])
			by[i] += len(found)
			spans = append(spans, found...)
		}
		if len(spans) == 0 {
			break
		}
		spans = merge(spans)
		var b strings.Builder
		last := 0
		for _, sp := range spans {
			b.WriteString(s[last:at[sp.start]])
			b.WriteString(Placeholder)
			last = at[sp.end]
		}
		b.WriteString(s[last:])
		if b.String() == s {
			break
		}
		s, changed = b.String(), true
	}
	return s, changed
}

// onlyPlaceholders: nothing is left of a list of names but what was taken out (event_date's
// instructor „Mustermann" or „Mustermann, Musterfrau" with both in the register).
func onlyPlaceholders(s string) bool {
	rest := strings.ReplaceAll(s, Placeholder, "")
	return strings.Trim(rest, " ,;/&+-–\t\n") == "" || strings.EqualFold(strings.TrimSpace(strings.Trim(rest, " ,;/&+-–")), "und")
}

// order puts the lists of names first: the modules and events they tie a person to decide where
// the surname alone means them.
func order(tables []Table) []Table {
	out := append([]Table{}, tables...)
	sort.SliceStable(out, func(i, j int) bool { return nameLists[out[i].Source] && !nameLists[out[j].Source] })
	return out
}

// Apply takes the register's persons out of the tables: their rows of module_person and
// event_person, and every mention of them in a text (given name or initial with the surname, a
// title or salutation with the surname, their e-mail addresses, the register's further forms,
// and the surname alone in the rows of their own modules and events), which reads Placeholder
// instead; an instructor of event_date left with nothing but that is NULL. It is deterministic
// and idempotent: the same register on the same data takes out the same, and nothing more the
// second time.
func Apply(ctx context.Context, db DB, tables []Table, people []Person) (Result, error) {
	res := Result{People: len(people), Changed: map[string]int{}, ByPerson: map[int64]int{}, keys: map[string][][]any{}}
	if len(people) == 0 {
		return res, nil
	}
	p := newPass(people)
	counts := make([]int, len(people))
	for _, t := range order(tables) {
		if err := ctx.Err(); err != nil {
			return res, err
		}
		rows, err := scan(ctx, db, t)
		if err != nil {
			return res, err
		}
		var drop, update []scanned
		listed := p.list(t, rows)
		for n, r := range rows {
			if who := listed[n]; len(who) > 0 {
				for _, i := range who {
					counts[i]++
				}
				drop = append(drop, r)
				continue
			}
			bare := p.bare(r)
			changed := false
			for i, v := range r.values {
				if !v.Valid {
					continue
				}
				out, ok := p.redact(v.String, bare, counts)
				if !ok {
					continue
				}
				changed = true
				res.Values++
				if t.Source == "event_date" && t.Text[i] == "instructor" && onlyPlaceholders(out) {
					r.values[i] = sql.NullString{}
				} else {
					r.values[i] = sql.NullString{String: out, Valid: true}
				}
			}
			if changed {
				update = append(update, r)
			}
		}
		if len(drop) > 0 {
			del := "DELETE FROM " + quote(t.Name) + " WHERE " + where(t.Key)
			for _, r := range drop {
				if _, err := db.ExecContext(ctx, del, r.key...); err != nil {
					return res, fmt.Errorf("privacy: delete from %s: %w", t.Name, err)
				}
			}
			res.Rows += len(drop)
		}
		if len(update) > 0 {
			sets := make([]string, len(t.Text))
			for i, c := range t.Text {
				sets[i] = quote(c) + " = ?"
			}
			stmt := "UPDATE " + quote(t.Name) + " SET " + strings.Join(sets, ", ") + " WHERE " + where(t.Key)
			for _, r := range update {
				args := make([]any, 0, len(r.values)+len(r.key))
				for _, v := range r.values {
					if v.Valid {
						args = append(args, v.String)
					} else {
						args = append(args, nil)
					}
				}
				args = append(args, r.key...)
				if _, err := db.ExecContext(ctx, stmt, args...); err != nil {
					return res, fmt.Errorf("privacy: update %s: %w", t.Name, err)
				}
				res.keys[t.Name] = append(res.keys[t.Name], r.key)
			}
			res.Changed[t.Name] += len(update)
		}
	}
	for i := range people {
		res.Modules += len(p.modules[i])
		res.Events += len(p.events[i])
		res.ByPerson[people[i].ID] += counts[i]
	}
	return res, nil
}

// Find reports where the persons are mentioned in the tables, by the rules of Apply, and changes
// nothing: for validate, which fails on any hit after the build took them out, and for a
// request for information (Art. 15 DSGVO), which persons not in the register can make. The
// surname alone counts only in the rows of modules and events whose lists of names still hold
// the person: once Apply took those rows out, the surname alone is not looked for.
func Find(ctx context.Context, db DB, tables []Table, people []Person, limit int) ([]Hit, int, error) {
	if len(people) == 0 {
		return nil, 0, nil
	}
	p := newPass(people)
	var hits []Hit
	total := 0
	add := func(h Hit) {
		total++
		if limit <= 0 || len(hits) < limit {
			hits = append(hits, h)
		}
	}
	for _, t := range order(tables) {
		if err := ctx.Err(); err != nil {
			return hits, total, err
		}
		rows, err := scan(ctx, db, t)
		if err != nil {
			return hits, total, err
		}
		listed := p.list(t, rows)
		for n, r := range rows {
			if who := listed[n]; len(who) > 0 {
				for _, i := range who {
					add(Hit{Person: people[i].ID, Table: t.Name, Key: keyString(t.Key, r.key), Column: "name", Snippet: valueOf(t, r, "name"), Row: true})
				}
				continue
			}
			bare := p.bare(r)
			for c, v := range r.values {
				if !v.Valid {
					continue
				}
				f, at := fold(v.String)
				for i, person := range p.persons {
					for _, sp := range merge(person.mentions(f, bare[i])) {
						add(Hit{Person: people[i].ID, Table: t.Name, Key: keyString(t.Key, r.key), Column: t.Text[c], Snippet: snippet(v.String, at[sp.start], at[sp.end])})
					}
				}
			}
		}
	}
	return hits, total, nil
}

func valueOf(t Table, r scanned, col string) string {
	for i, c := range t.Text {
		if c == col {
			return r.values[i].String
		}
	}
	return ""
}

// snippet is the text around s[start:end], at most 40 characters on either side.
func snippet(s string, start, end int) string {
	from, to := start, end
	for n := 0; from > 0 && n < 40; n++ {
		from--
		for from > 0 && s[from]&0xC0 == 0x80 {
			from--
		}
	}
	for n := 0; to < len(s) && n < 40; n++ {
		to++
		for to < len(s) && s[to]&0xC0 == 0x80 {
			to++
		}
	}
	out := strings.Join(strings.Fields(s[from:to]), " ")
	if from > 0 {
		out = "…" + out
	}
	if to < len(s) {
		out += "…"
	}
	return out
}

// CatalogTables describes the tables of a catalog database (radix.db or a snapshot), read from
// its schema: every table but the raw archive and the register itself. Searched are the columns
// declared TEXT, but for the primary key and for addresses (url, *_url) and hashes (*_hash).
func CatalogTables(ctx context.Context, db DB) ([]Table, error) {
	names, err := tableNames(ctx, db)
	if err != nil {
		return nil, err
	}
	var tables []Table
	for _, name := range names {
		if name == "raw_page" || strings.HasPrefix(name, "privacy_") {
			continue
		}
		cols, err := Columns(ctx, db, name)
		if err != nil {
			return nil, err
		}
		t := Table{Name: name, Source: name}
		var pk []string
		for _, c := range cols {
			if c.PK > 0 {
				pk = append(pk, c.Name)
			}
		}
		t.Key = pk
		if len(pk) == 0 {
			t.Key = []string{"rowid"}
		}
		t.Text = TextColumns(cols)
		t.Module, t.Event = Belongs(name, cols)
		if len(t.Text) > 0 {
			tables = append(tables, t)
		}
	}
	return tables, nil
}

// Column is a column of a table, as PRAGMA table_info reads it.
type Column struct {
	Name string `json:"name"`
	Type string `json:"type"`
	PK   int    `json:"pk,omitempty"`
}

// Columns reads the columns of a table.
func Columns(ctx context.Context, db DB, table string) ([]Column, error) {
	rows, err := db.QueryContext(ctx, "SELECT name, type, pk FROM pragma_table_info(?) ORDER BY cid", table)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var cols []Column
	for rows.Next() {
		var c Column
		if err := rows.Scan(&c.Name, &c.Type, &c.PK); err != nil {
			return nil, err
		}
		cols = append(cols, c)
	}
	return cols, rows.Err()
}

// TextColumns are the columns of a table that Apply and Find search: those declared TEXT, but
// for the primary key, addresses and hashes.
func TextColumns(cols []Column) []string {
	var text []string
	for _, c := range cols {
		lower := strings.ToLower(c.Name)
		if c.PK > 0 || !strings.Contains(strings.ToUpper(c.Type), "TEXT") ||
			lower == "url" || strings.HasSuffix(lower, "_url") || strings.HasSuffix(lower, "_hash") {
			continue
		}
		text = append(text, c.Name)
	}
	return text
}

// Belongs names the columns that tie a row of the catalog table to a module and to an event.
func Belongs(table string, cols []Column) (module, event string) {
	has := map[string]bool{}
	for _, c := range cols {
		has[c.Name] = true
	}
	switch {
	case table == "module" && has["id"]:
		module = "id"
	case has["module_id"]:
		module = "module_id"
	}
	switch {
	case table == "event" && has["id"]:
		event = "id"
	case has["event_id"]:
		event = "event_id"
	}
	return module, event
}

func tableNames(ctx context.Context, db DB) ([]string, error) {
	rows, err := db.QueryContext(ctx, "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var names []string
	for rows.Next() {
		var n string
		if err := rows.Scan(&n); err != nil {
			return nil, err
		}
		names = append(names, n)
	}
	return names, rows.Err()
}
