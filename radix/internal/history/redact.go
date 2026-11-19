package history

import (
	"context"
	"database/sql"
	"fmt"
	"sort"

	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// Redact applies the privacy register to every version the history keeps, as the build applies
// it to the catalog (privacy.Apply): a person's rows of the lists of names go from every state,
// and their names from every text. Versions that the redaction made equal to the one before or
// after them become one. It notes the register's digest (history_meta privacy_digest): Record applies it
// again when the register changed since.
func (h *History) Redact(ctx context.Context, people []privacy.Person) (privacy.Result, error) {
	tx, err := h.db.BeginTx(ctx, nil)
	if err != nil {
		return privacy.Result{}, err
	}
	defer func() { _ = tx.Rollback() }()
	res, err := redactTx(ctx, tx, people)
	if err != nil {
		return res, err
	}
	return res, tx.Commit()
}

// Sync applies the register when it changed since it was last applied, and reports whether it did.
func (h *History) Sync(ctx context.Context, people []privacy.Person) (bool, privacy.Result, error) {
	if readMeta(ctx, h.db, "privacy_digest") == privacy.Digest(people) {
		return false, privacy.Result{}, nil
	}
	res, err := h.Redact(ctx, people)
	return err == nil, res, err
}

// Find reports where the persons are named in any version the history keeps, by the rules of the
// register (privacy.Find), and changes nothing: a request for information (Art. 15 DSGVO).
func (h *History) Find(ctx context.Context, people []privacy.Person, limit int) ([]privacy.Hit, int, error) {
	known, err := loadTables(ctx, h.db)
	if err != nil {
		return nil, 0, err
	}
	return privacy.Find(ctx, h.db, historyTables(known), people, limit)
}

// historyTables describes the copies of the tables for privacy.Apply and privacy.Find: each by the
// catalog table it holds, its rows found by their rowid.
func historyTables(known map[string]*table) []privacy.Table {
	var tables []privacy.Table
	for _, t := range known {
		text := privacy.TextColumns(t.Columns)
		if len(text) == 0 {
			continue
		}
		module, event := privacy.Belongs(t.Name, t.Columns)
		tables = append(tables, privacy.Table{Name: t.Store, Source: t.Name, Key: []string{"rowid"}, Text: text, Module: module, Event: event})
	}
	sort.Slice(tables, func(i, j int) bool { return tables[i].Name < tables[j].Name })
	return tables
}

func redactTx(ctx context.Context, tx *sql.Tx, people []privacy.Person) (privacy.Result, error) {
	known, err := loadTables(ctx, tx)
	if err != nil {
		return privacy.Result{}, err
	}
	tables := historyTables(known)
	res, err := privacy.Apply(ctx, tx, tables, people)
	if err != nil {
		return res, err
	}
	for _, t := range tables {
		keys := res.ChangedKeys(t.Name)
		if len(keys) == 0 {
			continue
		}
		if err := rehash(ctx, tx, known[t.Source], keys); err != nil {
			return res, fmt.Errorf("history: %s: %w", t.Name, err)
		}
		if err := compact(ctx, tx, known[t.Source]); err != nil {
			return res, fmt.Errorf("history: %s: %w", t.Name, err)
		}
	}
	return res, setMeta(ctx, tx, "privacy_digest", privacy.Digest(people))
}

// rehash computes the hash of the rows the redaction changed, as Record computes it.
func rehash(ctx context.Context, tx *sql.Tx, t *table, keys [][]any) error {
	names := make([]string, len(t.Columns))
	for i, c := range t.Columns {
		names[i] = c.Name
	}
	hs := newHasher(names)
	read, err := tx.PrepareContext(ctx, "SELECT "+quoteAll(names)+" FROM "+quote(t.Store)+" WHERE rowid = ?")
	if err != nil {
		return err
	}
	defer read.Close()
	write, err := tx.PrepareContext(ctx, "UPDATE "+quote(t.Store)+" SET _hash = ? WHERE rowid = ?")
	if err != nil {
		return err
	}
	defer write.Close()
	for _, key := range keys {
		values := make([]any, len(names))
		ptrs := make([]any, len(names))
		for i := range values {
			ptrs[i] = &values[i]
		}
		if err := read.QueryRowContext(ctx, key[0]).Scan(ptrs...); err != nil {
			return err
		}
		if _, err := write.ExecContext(ctx, hs.sum(values), key[0]); err != nil {
			return err
		}
	}
	return nil
}

// compact joins versions of a row that are equal and follow each other: [a, b) and [b, c) of the
// same hash are [a, c). The state at every time stays what it was.
func compact(ctx context.Context, tx *sql.Tx, t *table) error {
	type version struct {
		id       int64
		from     int64
		to       sql.NullInt64
		hash     string
		consumed bool
	}
	rows, err := tx.QueryContext(ctx, "SELECT rowid, _from, _to, _hash FROM "+quote(t.Store))
	if err != nil {
		return err
	}
	byHash := map[string][]*version{}
	for rows.Next() {
		var v version
		var h []byte
		if err := rows.Scan(&v.id, &v.from, &v.to, &h); err != nil {
			rows.Close()
			return err
		}
		v.hash = string(h)
		byHash[v.hash] = append(byHash[v.hash], &v)
	}
	rows.Close()
	extend, err := tx.PrepareContext(ctx, "UPDATE "+quote(t.Store)+" SET _to = ? WHERE rowid = ?")
	if err != nil {
		return err
	}
	defer extend.Close()
	drop, err := tx.PrepareContext(ctx, "DELETE FROM "+quote(t.Store)+" WHERE rowid = ?")
	if err != nil {
		return err
	}
	defer drop.Close()
	for _, vs := range byHash {
		if len(vs) < 2 {
			continue
		}
		sort.Slice(vs, func(i, j int) bool { return vs[i].from < vs[j].from })
		for i, a := range vs {
			if a.consumed {
				continue
			}
			changed := false
			for a.to.Valid {
				var next *version
				for _, b := range vs[i+1:] {
					if !b.consumed && b.from == a.to.Int64 {
						next = b
						break
					}
				}
				if next == nil {
					break
				}
				next.consumed = true
				a.to = next.to
				changed = true
				if _, err := drop.ExecContext(ctx, next.id); err != nil {
					return err
				}
			}
			if changed {
				var to any
				if a.to.Valid {
					to = a.to.Int64
				}
				if _, err := extend.ExecContext(ctx, to, a.id); err != nil {
					return err
				}
			}
		}
	}
	return nil
}
