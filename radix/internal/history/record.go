package history

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// RecordOptions say how a snapshot is recorded.
type RecordOptions struct {
	// ValidFrom is when the snapshot became the published state (a backfill of kept snapshots);
	// it must be after the newest recorded state. Zero takes the snapshot's meta data_changed_at
	// (built_at where it has none), and where that is not after the newest state, the time of
	// recording: a rollback to an older release publishes its older state from now on.
	ValidFrom time.Time
	// People is the privacy register, as radix.db has it: they are taken out of the snapshot before
	// it is recorded (a snapshot of a Radix without the register, or one built before a person was
	// added), and out of every version of the history first when the register changed since it
	// was last applied (Redact).
	People []privacy.Person
	// NoRegister: the caller has no register (no radix.db at hand). A history that a register was
	// applied to refuses to record then, since what the register keeps out would come back.
	NoRegister bool
	// Now is the clock; nil is time.Now.
	Now func() time.Time
}

// ddlRow is one entry of the snapshot's sqlite_master.
type ddlRow struct {
	Type  string `json:"type"`
	Name  string `json:"name"`
	Table string `json:"tbl_name"`
	SQL   string `json:"sql"`
}

// Record adds the snapshot at path as the newest state: every row of every table that was not in
// the newest state is stored, valid from it, and every row of the newest state that the snapshot
// no longer has ends with it. The schema of the snapshot is kept beside it (At builds its tables
// from it). One transaction: an interrupted Record records nothing. The result is nil when the
// snapshot holds what the newest state holds (its content digest), which records nothing.
//
// A table's INTEGER PRIMARY KEY that no foreign key refers to (event_date.id, plan_entry.id,
// program_module_assertion.id) is a number the build gives each row in the order it writes them:
// it is not stored, so that a row written earlier does not renumber, and so change, every row
// after it. At numbers them anew.
func (h *History) Record(ctx context.Context, path string, opt RecordOptions) (*Snapshot, error) {
	now := time.Now
	if opt.Now != nil {
		now = opt.Now
	}
	// The register first: what it keeps out goes from every version before anything is compared.
	applied := readMeta(ctx, h.db, "privacy_digest")
	switch {
	case opt.NoRegister && applied != "":
		return nil, fmt.Errorf("history: a privacy register was applied to %s; record with the register (radix.db), or what it keeps out comes back", h.path)
	case !opt.NoRegister && applied != privacy.Digest(opt.People):
		if _, err := h.Redact(ctx, opt.People); err != nil {
			return nil, err
		}
	}
	snap, err := openSnapshot(path)
	if err != nil {
		return nil, err
	}
	defer func() { snap.Close() }()

	metaValues := map[string]string{}
	rows, err := snap.QueryContext(ctx, "SELECT key, value FROM meta WHERE key IN ('content_digest', 'data_changed_at', 'built_at', 'radix_version')")
	if err != nil {
		return nil, fmt.Errorf("history: %s is no snapshot of Radix: %w", path, err)
	}
	for rows.Next() {
		var k, v string
		if err := rows.Scan(&k, &v); err != nil {
			rows.Close()
			return nil, err
		}
		metaValues[k] = v
	}
	rows.Close()
	var userVersion int
	if err := snap.QueryRowContext(ctx, "PRAGMA user_version").Scan(&userVersion); err != nil {
		return nil, err
	}
	ddl, err := readDDL(ctx, snap)
	if err != nil {
		return nil, err
	}

	last, err := h.Last(ctx)
	if err != nil {
		return nil, err
	}
	if last != nil && metaValues["content_digest"] != "" && last.ContentDigest == metaValues["content_digest"] {
		return nil, nil
	}
	if len(opt.People) > 0 {
		// What is recorded is a copy without the register's persons; what was compared is the
		// snapshot as published, which a Radix with the register built without them already.
		tmp, err := redactedCopy(ctx, path, filepath.Dir(h.path), opt.People)
		if err != nil {
			return nil, err
		}
		defer os.Remove(tmp)
		snap.Close()
		if snap, err = openSnapshot(tmp); err != nil {
			return nil, err
		}
	}

	validFrom := opt.ValidFrom
	if validFrom.IsZero() {
		for _, key := range []string{"data_changed_at", "built_at"} {
			if t, err := time.Parse(time.RFC3339, metaValues[key]); err == nil {
				validFrom = t
				break
			}
		}
	}
	recordedAt := now().UTC().Truncate(time.Second)
	if validFrom.IsZero() {
		validFrom = recordedAt
	}
	validFrom = validFrom.UTC().Truncate(time.Second)
	if last != nil && !validFrom.After(last.ValidFrom) {
		if !opt.ValidFrom.IsZero() {
			return nil, fmt.Errorf("history: the newest state is valid from %s, not before %s; record the snapshots in their order",
				last.ValidFrom.Format(timeFormat), validFrom.Format(timeFormat))
		}
		// Published again after a newer state (a rollback), or within the second of the one
		// before: valid from now, and at least a second after it.
		validFrom = recordedAt
		if !validFrom.After(last.ValidFrom) {
			validFrom = last.ValidFrom.Add(time.Second)
		}
	}

	tx, err := h.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()

	schemaID, err := schemaOf(ctx, tx, userVersion, ddl)
	if err != nil {
		return nil, err
	}
	seq := int64(1)
	if last != nil {
		seq = last.Seq + 1
	}
	known, err := loadTables(ctx, tx)
	if err != nil {
		return nil, err
	}
	referenced, err := referencedColumns(ctx, snap, ddl)
	if err != nil {
		return nil, err
	}

	result := &Snapshot{Seq: seq, ValidFrom: validFrom, RecordedAt: recordedAt, ContentDigest: metaValues["content_digest"],
		RadixVersion: metaValues["radix_version"], SchemaVersion: userVersion, Source: filepath.Base(path)}
	present := map[string]bool{}
	for _, d := range ddl {
		if d.Type != "table" {
			continue
		}
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		present[d.Name] = true
		opened, closed, err := mirror(ctx, tx, snap, d.Name, known[d.Name], referenced, seq)
		if err != nil {
			return nil, fmt.Errorf("history: table %s: %w", d.Name, err)
		}
		result.RowsOpened += opened
		result.RowsClosed += closed
	}
	for name, t := range known {
		if present[name] {
			continue
		}
		res, err := tx.ExecContext(ctx, "UPDATE "+quote(t.Store)+" SET _to = ? WHERE _to IS NULL", seq)
		if err != nil {
			return nil, err
		}
		n, _ := res.RowsAffected()
		result.RowsClosed += int(n)
	}

	if _, err := tx.ExecContext(ctx, `INSERT INTO history_snapshot (seq, valid_from, recorded_at, content_digest, radix_version, schema_id, source, rows_opened, rows_closed)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`, seq, validFrom.Format(timeFormat), recordedAt.Format(timeFormat),
		nullable(result.ContentDigest), nullable(result.RadixVersion), schemaID, result.Source, result.RowsOpened, result.RowsClosed); err != nil {
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return result, nil
}

func openSnapshot(path string) (*sql.DB, error) {
	if _, err := os.Stat(path); err != nil {
		return nil, err
	}
	db, err := sql.Open("sqlite", "file:"+filepath.ToSlash(path)+"?mode=ro&immutable=1&_pragma=query_only(1)")
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	return db, nil
}

func nullable(s string) any {
	if s == "" {
		return nil
	}
	return s
}

// redactedCopy copies the snapshot beside the history and takes the register's persons out of it.
func redactedCopy(ctx context.Context, path, dir string, people []privacy.Person) (string, error) {
	in, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer in.Close()
	out, err := os.CreateTemp(dir, "record-*.db")
	if err != nil {
		return "", err
	}
	tmp := out.Name()
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		os.Remove(tmp)
		return "", err
	}
	if err := out.Close(); err != nil {
		os.Remove(tmp)
		return "", err
	}
	db, err := sql.Open("sqlite", tmp)
	if err != nil {
		os.Remove(tmp)
		return "", err
	}
	defer db.Close()
	db.SetMaxOpenConns(1)
	tables, err := privacy.CatalogTables(ctx, db)
	if err == nil {
		_, err = privacy.Apply(ctx, db, tables, people)
	}
	if err != nil {
		os.Remove(tmp)
		return "", fmt.Errorf("history: privacy register on %s: %w", path, err)
	}
	return tmp, nil
}

func readDDL(ctx context.Context, db *sql.DB) ([]ddlRow, error) {
	rows, err := db.QueryContext(ctx, "SELECT type, name, tbl_name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL ORDER BY rowid")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []ddlRow
	for rows.Next() {
		var d ddlRow
		if err := rows.Scan(&d.Type, &d.Name, &d.Table, &d.SQL); err != nil {
			return nil, err
		}
		out = append(out, d)
	}
	return out, rows.Err()
}

// schemaOf stores the schema once and returns its id.
func schemaOf(ctx context.Context, tx *sql.Tx, userVersion int, ddl []ddlRow) (int64, error) {
	body, err := json.Marshal(ddl)
	if err != nil {
		return 0, err
	}
	sum := sha256.Sum256(append([]byte(fmt.Sprintf("%d\n", userVersion)), body...))
	digest := hex.EncodeToString(sum[:16])
	var id int64
	err = tx.QueryRowContext(ctx, "SELECT id FROM history_schema WHERE digest = ?", digest).Scan(&id)
	if err == nil {
		return id, nil
	}
	if err != sql.ErrNoRows {
		return 0, err
	}
	res, err := tx.ExecContext(ctx, "INSERT INTO history_schema (user_version, digest, ddl) VALUES (?, ?, ?)", userVersion, digest, string(body))
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

// referencedColumns: the columns some foreign key of the snapshot points to, "table.column".
func referencedColumns(ctx context.Context, db *sql.DB, ddl []ddlRow) (map[string]bool, error) {
	out := map[string]bool{}
	for _, d := range ddl {
		if d.Type != "table" {
			continue
		}
		rows, err := db.QueryContext(ctx, `SELECT "table", "to" FROM pragma_foreign_key_list(?)`, d.Name)
		if err != nil {
			return nil, err
		}
		for rows.Next() {
			var parent string
			var to sql.NullString
			if err := rows.Scan(&parent, &to); err != nil {
				rows.Close()
				return nil, err
			}
			if to.Valid && to.String != "" {
				out[parent+"."+to.String] = true
			} else {
				out[parent+".*"] = true // the parent's primary key
			}
		}
		rows.Close()
	}
	return out, nil
}

// volatile reports whether a column of the snapshot is the row number the build gives: the
// table's only primary key column, an INTEGER (so the rowid), and nothing refers to it.
func volatile(name string, c privacy.Column, cols []privacy.Column, referenced map[string]bool) bool {
	if c.PK == 0 || !strings.EqualFold(strings.TrimSpace(c.Type), "INTEGER") {
		return false
	}
	pks := 0
	for _, o := range cols {
		if o.PK > 0 {
			pks++
		}
	}
	return pks == 1 && !referenced[name+"."+c.Name] && !referenced[name+".*"]
}

// mirror records one table of the snapshot as of seq.
func mirror(ctx context.Context, tx *sql.Tx, snap *sql.DB, name string, known *table, referenced map[string]bool, seq int64) (opened, closed int, err error) {
	cols, err := privacy.Columns(ctx, snap, name)
	if err != nil {
		return 0, 0, err
	}
	var stored []privacy.Column
	for _, c := range cols {
		if !volatile(name, c, cols, referenced) {
			stored = append(stored, c)
		}
	}
	t, err := ensureTable(ctx, tx, name, known, stored)
	if err != nil {
		return 0, 0, err
	}

	open := map[string][]int64{}
	rows, err := tx.QueryContext(ctx, "SELECT rowid, _hash FROM "+quote(t.Store)+" WHERE _to IS NULL")
	if err != nil {
		return 0, 0, err
	}
	for rows.Next() {
		var id int64
		var h []byte
		if err := rows.Scan(&id, &h); err != nil {
			rows.Close()
			return 0, 0, err
		}
		open[string(h)] = append(open[string(h)], id)
	}
	rows.Close()

	names := make([]string, len(stored))
	for i, c := range stored {
		names[i] = c.Name
	}
	hs := newHasher(names)
	insert, err := tx.PrepareContext(ctx, "INSERT INTO "+quote(t.Store)+" (_from, _hash, "+quoteAll(names)+") VALUES (?, ?"+strings.Repeat(", ?", len(names))+")")
	if err != nil {
		return 0, 0, err
	}
	defer insert.Close()

	src, err := snap.QueryContext(ctx, "SELECT "+quoteAll(names)+" FROM "+quote(name))
	if err != nil {
		return 0, 0, err
	}
	type row struct {
		hash   []byte
		values []any
	}
	var fresh []row
	for src.Next() {
		values := make([]any, len(names))
		ptrs := make([]any, len(names))
		for i := range values {
			ptrs[i] = &values[i]
		}
		if err := src.Scan(ptrs...); err != nil {
			src.Close()
			return 0, 0, err
		}
		h := hs.sum(values)
		if ids := open[string(h)]; len(ids) > 0 {
			open[string(h)] = ids[1:] // the same row as in the newest state: nothing to store
			continue
		}
		fresh = append(fresh, row{h, values})
	}
	err = src.Err()
	src.Close()
	if err != nil {
		return 0, 0, err
	}
	for _, r := range fresh {
		if _, err := insert.ExecContext(ctx, append([]any{seq, r.hash}, r.values...)...); err != nil {
			return 0, 0, err
		}
	}
	end, err := tx.PrepareContext(ctx, "UPDATE "+quote(t.Store)+" SET _to = ? WHERE rowid = ?")
	if err != nil {
		return 0, 0, err
	}
	defer end.Close()
	for _, ids := range open {
		for _, id := range ids {
			if _, err := end.ExecContext(ctx, seq, id); err != nil {
				return 0, 0, err
			}
			closed++
		}
	}
	return len(fresh), closed, nil
}

// ensureTable makes the copy of a table, or adds the columns it lacks.
func ensureTable(ctx context.Context, tx *sql.Tx, name string, known *table, stored []privacy.Column) (*table, error) {
	if known == nil {
		t := &table{Name: name, Store: "h_" + name, Columns: stored}
		defs := []string{"_from INTEGER NOT NULL", "_to INTEGER", "_hash BLOB NOT NULL"}
		for _, c := range stored {
			defs = append(defs, quote(c.Name)) // no type: every value stays as the snapshot had it
		}
		stmts := []string{
			"CREATE TABLE " + quote(t.Store) + " (" + strings.Join(defs, ", ") + ")",
			"CREATE INDEX " + quote(t.Store+"__open") + " ON " + quote(t.Store) + " (_hash) WHERE _to IS NULL",
		}
		var key []string
		for _, c := range stored {
			if c.PK > 0 {
				key = append(key, c.Name)
			}
		}
		if len(key) > 0 {
			stmts = append(stmts, "CREATE INDEX "+quote(t.Store+"__key")+" ON "+quote(t.Store)+" ("+quoteAll(key)+", _from)")
		}
		for _, s := range stmts {
			if _, err := tx.ExecContext(ctx, s); err != nil {
				return nil, err
			}
		}
		return t, saveTable(ctx, tx, t)
	}
	changed := false
	for _, c := range stored {
		found := false
		for i, k := range known.Columns {
			if k.Name == c.Name {
				found = true
				if k.Type != c.Type || k.PK != c.PK {
					known.Columns[i].Type, known.Columns[i].PK = c.Type, c.PK
					changed = true
				}
			}
		}
		if !found {
			if _, err := tx.ExecContext(ctx, "ALTER TABLE "+quote(known.Store)+" ADD COLUMN "+quote(c.Name)); err != nil {
				return nil, err
			}
			known.Columns = append(known.Columns, c)
			changed = true
		}
	}
	if changed {
		return known, saveTable(ctx, tx, known)
	}
	return known, nil
}

func saveTable(ctx context.Context, tx *sql.Tx, t *table) error {
	cols, err := json.Marshal(t.Columns)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, "INSERT INTO history_table (name, store, columns) VALUES (?, ?, ?) ON CONFLICT (name) DO UPDATE SET columns = excluded.columns",
		t.Name, t.Store, string(cols))
	return err
}
