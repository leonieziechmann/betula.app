package history

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// ErrBefore: nothing was recorded at or before the time asked for.
var ErrBefore = errors.New("history: nothing recorded at or before that time")

// StateAt returns the recorded state that was the published one at t.
func (h *History) StateAt(ctx context.Context, t time.Time) (*Snapshot, error) {
	s, err := scanSnapshot(h.db.QueryRowContext(ctx, "SELECT "+snapshotColumns+" WHERE s.valid_from <= ? ORDER BY s.seq DESC LIMIT 1",
		t.UTC().Format(timeFormat)))
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrBefore
	}
	return s, err
}

// At writes the catalog as it was published at t to out (which must not exist): a snapshot with
// the tables, indexes and views of that time and the rows valid then, which the Folia of that time
// reads as it read the original. The rows of every table are one pass over its copy (the rows
// whose span holds the state): seconds, however long the history. Numbers the build gave its
// rows (Record) are given anew; everything else is as published. Returns the state it wrote.
func (h *History) At(ctx context.Context, t time.Time, out string) (*Snapshot, error) {
	state, err := h.StateAt(ctx, t)
	if err != nil {
		return nil, err
	}
	if _, err := os.Stat(out); err == nil {
		return nil, fmt.Errorf("history: %s exists", out)
	}
	var body string
	if err := h.db.QueryRowContext(ctx, "SELECT ddl FROM history_schema s JOIN history_snapshot n ON n.schema_id = s.id WHERE n.seq = ?", state.Seq).Scan(&body); err != nil {
		return nil, err
	}
	var ddl []ddlRow
	if err := json.Unmarshal([]byte(body), &ddl); err != nil {
		return nil, fmt.Errorf("history: schema of state %d: %w", state.Seq, err)
	}
	known, err := loadTables(ctx, h.db)
	if err != nil {
		return nil, err
	}

	tmp := out + ".making"
	_ = os.Remove(tmp)
	if dir := filepath.Dir(out); dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, err
		}
	}
	db, err := sql.Open("sqlite", tmp)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1) // ATTACH and the pragmas are per connection
	fail := func(err error) (*Snapshot, error) {
		db.Close()
		os.Remove(tmp)
		return nil, err
	}
	for _, d := range ddl {
		if d.Type == "table" {
			if _, err := db.ExecContext(ctx, d.SQL); err != nil {
				return fail(fmt.Errorf("history: create %s: %w", d.Name, err))
			}
		}
	}
	if _, err := db.ExecContext(ctx, "ATTACH DATABASE ? AS h", h.path); err != nil {
		return fail(err)
	}
	for _, d := range ddl {
		if d.Type != "table" {
			continue
		}
		if err := ctx.Err(); err != nil {
			return fail(err)
		}
		k := known[d.Name]
		if k == nil {
			continue
		}
		rows, err := db.QueryContext(ctx, "SELECT name FROM pragma_table_info(?)", d.Name)
		if err != nil {
			return fail(err)
		}
		var cols []string
		for rows.Next() {
			var c string
			if err := rows.Scan(&c); err != nil {
				rows.Close()
				return fail(err)
			}
			if k.has(c) {
				cols = append(cols, c)
			}
		}
		rows.Close()
		if len(cols) == 0 {
			continue
		}
		list := quoteAll(cols)
		if _, err := db.ExecContext(ctx, "INSERT INTO main."+quote(d.Name)+" ("+list+") SELECT "+list+" FROM h."+quote(k.Store)+
			" WHERE _from <= ? AND (_to IS NULL OR _to > ?) ORDER BY rowid", state.Seq, state.Seq); err != nil {
			return fail(fmt.Errorf("history: rows of %s: %w", d.Name, err))
		}
	}
	if _, err := db.ExecContext(ctx, "DETACH DATABASE h"); err != nil {
		return fail(err)
	}
	// Indexes and triggers, then the views, in the order of the original; a view on a view made
	// later is made in a later round.
	var pending []ddlRow
	for _, d := range ddl {
		switch d.Type {
		case "index", "trigger":
			if _, err := db.ExecContext(ctx, d.SQL); err != nil {
				return fail(fmt.Errorf("history: create %s: %w", d.Name, err))
			}
		case "view":
			pending = append(pending, d)
		}
	}
	for len(pending) > 0 {
		var later []ddlRow
		var lastErr error
		for _, d := range pending {
			if _, err := db.ExecContext(ctx, d.SQL); err != nil {
				later, lastErr = append(later, d), err
			}
		}
		if len(later) == len(pending) {
			return fail(fmt.Errorf("history: create view %s: %w", later[0].Name, lastErr))
		}
		pending = later
	}
	for _, stmt := range []string{fmt.Sprintf("PRAGMA user_version = %d", state.SchemaVersion), "PRAGMA journal_mode = DELETE", "ANALYZE", "VACUUM"} {
		if _, err := db.ExecContext(ctx, stmt); err != nil {
			return fail(fmt.Errorf("history: %s: %w", stmt, err))
		}
	}
	if err := db.Close(); err != nil {
		os.Remove(tmp)
		return nil, err
	}
	return state, os.Rename(tmp, out)
}

// Version is one version of a row: the values it had from From until To (zero: it still has them).
type Version struct {
	From, To time.Time
	Key      string         // the row's primary key, "col=value, …"
	Values   map[string]any // its columns, NULL left out
	Changed  []string       // the columns that differ from the version of the same key before it; nil for the first
	Gone     bool           // no version of the same key follows: the row left the catalog at To
}

// Log returns the versions of the rows of a catalog table that match filter (column → value, compared
// as text), oldest first: „module id=11881" is that module's row, „module_person module_id=11881"
// its responsible persons, each version with the columns it changed.
func (h *History) Log(ctx context.Context, tableName string, filter map[string]string) ([]Version, error) {
	known, err := loadTables(ctx, h.db)
	if err != nil {
		return nil, err
	}
	t := known[tableName]
	if t == nil {
		names := make([]string, 0, len(known))
		for n := range known {
			names = append(names, n)
		}
		sort.Strings(names)
		return nil, fmt.Errorf("history: no table %q; the history has %s", tableName, strings.Join(names, ", "))
	}
	cols := make([]string, len(t.Columns))
	var key []string
	for i, c := range t.Columns {
		cols[i] = c.Name
		if c.PK > 0 {
			key = append(key, c.Name)
		}
	}
	var conds []string
	var args []any
	fcols := make([]string, 0, len(filter))
	for c := range filter {
		fcols = append(fcols, c)
	}
	sort.Strings(fcols)
	for _, c := range fcols {
		if !t.has(c) {
			return nil, fmt.Errorf("history: %s has no column %q", tableName, c)
		}
		conds = append(conds, "CAST("+quote(c)+" AS TEXT) = ?")
		args = append(args, filter[c])
	}
	q := "SELECT _from, _to, " + quoteAll(cols) + " FROM " + quote(t.Store)
	if len(conds) > 0 {
		q += " WHERE " + strings.Join(conds, " AND ")
	}
	q += " ORDER BY _from, rowid"
	times, err := h.times(ctx)
	if err != nil {
		return nil, err
	}
	rows, err := h.db.QueryContext(ctx, q, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Version
	previous := map[string]*Version{}
	for rows.Next() {
		var from int64
		var to sql.NullInt64
		values := make([]any, len(cols))
		ptrs := []any{&from, &to}
		for i := range values {
			ptrs = append(ptrs, &values[i])
		}
		if err := rows.Scan(ptrs...); err != nil {
			return nil, err
		}
		v := Version{From: times[from], Values: map[string]any{}}
		if to.Valid {
			v.To = times[to.Int64]
		}
		var keyParts []string
		for i, c := range cols {
			if b, ok := values[i].([]byte); ok && isText(b) {
				values[i] = string(b)
			}
			if values[i] != nil {
				v.Values[c] = values[i]
			}
		}
		for _, k := range key {
			keyParts = append(keyParts, fmt.Sprintf("%s=%v", k, v.Values[k]))
		}
		v.Key = strings.Join(keyParts, ", ")
		if len(key) == 0 {
			out = append(out, v) // no key to follow a row by (its number was the build's, Record): versions without their changes
			continue
		}
		if p := previous[v.Key]; p != nil {
			v.Changed = []string{}
			for _, c := range cols {
				if !equal(p.Values[c], v.Values[c]) && !noise[c] {
					v.Changed = append(v.Changed, c)
				}
			}
		}
		out = append(out, v)
		previous[v.Key] = &out[len(out)-1]
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	// A version is the last of its key when none of the same key begins where it ends.
	begins := map[string]bool{}
	for _, v := range out {
		begins[v.Key+"\x00"+v.From.Format(timeFormat)] = true
	}
	for i := range out {
		if len(key) > 0 && !out[i].To.IsZero() && !begins[out[i].Key+"\x00"+out[i].To.Format(timeFormat)] {
			out[i].Gone = true
		}
	}
	return out, nil
}

func isText(b []byte) bool {
	for _, c := range b {
		if c == 0 {
			return false
		}
	}
	return true
}
