// Package history keeps every state of the catalog that Radix published, in a database of its own
// (history.db): one copy of each table of the snapshot (h_<table>), in which a row is stored once
// for as long as it stays the same, with the snapshot it appeared in (_from) and the first one it
// was no longer in (_to). A new snapshot costs the rows that changed; a module whose text changed
// is a new version of its row, valid from that snapshot. The state at a time is the rows whose
// span holds it: one pass over each table, no chain of changes to replay (At). The privacy
// register reaches every version (Redact). See docs/radix/history.md.
package history

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/privacy"
	_ "modernc.org/sqlite"
)

// format is the layout of history.db that this package writes (history_meta format).
const format = "1"

// noise are columns that change without the content changing: when a page was last fetched. They
// make no new version (as they make no new snapshot: catalogbuild's content digest leaves them
// out), and a version keeps the value it was first recorded with.
var noise = map[string]bool{"fetched_at": true}

const baseSchema = `
CREATE TABLE IF NOT EXISTS history_meta (
	key   TEXT PRIMARY KEY,
	value TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS history_schema (
	id           INTEGER PRIMARY KEY,
	user_version INTEGER NOT NULL,
	digest       TEXT NOT NULL UNIQUE,
	ddl          TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_snapshot (
	seq            INTEGER PRIMARY KEY,
	valid_from     TEXT NOT NULL UNIQUE,
	recorded_at    TEXT NOT NULL,
	content_digest TEXT,
	radix_version  TEXT,
	schema_id      INTEGER NOT NULL REFERENCES history_schema(id),
	source         TEXT,
	rows_opened    INTEGER NOT NULL,
	rows_closed    INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS history_table (
	name    TEXT PRIMARY KEY,
	store   TEXT NOT NULL UNIQUE,
	columns TEXT NOT NULL
) WITHOUT ROWID;
`

// History is an open history.db.
type History struct {
	db   *sql.DB
	path string
}

// Open opens the history at path, and makes it when there is none.
func Open(path string) (*History, error) {
	if dir := filepath.Dir(path); dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, err
		}
	}
	db, err := sql.Open("sqlite", path+"?_pragma=busy_timeout(60000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)")
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	if _, err := db.Exec(baseSchema); err != nil {
		db.Close()
		return nil, fmt.Errorf("history: %s: %w", path, err)
	}
	var f string
	switch err := db.QueryRow("SELECT value FROM history_meta WHERE key = 'format'").Scan(&f); {
	case errors.Is(err, sql.ErrNoRows):
		if _, err := db.Exec("INSERT INTO history_meta (key, value) VALUES ('format', ?)", format); err != nil {
			db.Close()
			return nil, err
		}
	case err != nil:
		db.Close()
		return nil, err
	case f != format:
		db.Close()
		return nil, fmt.Errorf("history: %s has format %s, this Radix reads %s", path, f, format)
	}
	return &History{db: db, path: path}, nil
}

// Close closes the history.
func (h *History) Close() error { return h.db.Close() }

// Path is the file of the history.
func (h *History) Path() string { return h.path }

// Snapshot is one recorded state.
type Snapshot struct {
	Seq           int64
	ValidFrom     time.Time // since when it was the published state
	RecordedAt    time.Time
	ContentDigest string
	RadixVersion  string
	SchemaVersion int
	Source        string
	RowsOpened    int // row versions that begin with it
	RowsClosed    int // row versions that end with it
}

const timeFormat = time.RFC3339

func scanSnapshot(row interface{ Scan(...any) error }) (*Snapshot, error) {
	var s Snapshot
	var from, recorded string
	var digest, version, source sql.NullString
	if err := row.Scan(&s.Seq, &from, &recorded, &digest, &version, &s.SchemaVersion, &source, &s.RowsOpened, &s.RowsClosed); err != nil {
		return nil, err
	}
	s.ValidFrom, _ = time.Parse(timeFormat, from)
	s.RecordedAt, _ = time.Parse(timeFormat, recorded)
	s.ContentDigest, s.RadixVersion, s.Source = digest.String, version.String, source.String
	return &s, nil
}

const snapshotColumns = `s.seq, s.valid_from, s.recorded_at, s.content_digest, s.radix_version, c.user_version, s.source, s.rows_opened, s.rows_closed
	FROM history_snapshot s JOIN history_schema c ON c.id = s.schema_id`

// Last returns the newest recorded state, nil when nothing is recorded yet.
func (h *History) Last(ctx context.Context) (*Snapshot, error) {
	s, err := scanSnapshot(h.db.QueryRowContext(ctx, "SELECT "+snapshotColumns+" ORDER BY s.seq DESC LIMIT 1"))
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	return s, err
}

// Snapshots returns every recorded state, oldest first.
func (h *History) Snapshots(ctx context.Context) ([]Snapshot, error) {
	rows, err := h.db.QueryContext(ctx, "SELECT "+snapshotColumns+" ORDER BY s.seq")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Snapshot
	for rows.Next() {
		s, err := scanSnapshot(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, *s)
	}
	return out, rows.Err()
}

// meta reads a value of history_meta ("" when it is not set).
func readMeta(ctx context.Context, q privacy.DB, key string) string {
	var v string
	_ = q.QueryRowContext(ctx, "SELECT value FROM history_meta WHERE key = ?", key).Scan(&v)
	return v
}

func setMeta(ctx context.Context, q privacy.DB, key, value string) error {
	_, err := q.ExecContext(ctx, "INSERT INTO history_meta (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value", key, value)
	return err
}

// table is what history_table says of a copied table.
type table struct {
	Name    string
	Store   string
	Columns []privacy.Column
}

func loadTables(ctx context.Context, q privacy.DB) (map[string]*table, error) {
	rows, err := q.QueryContext(ctx, "SELECT name, store, columns FROM history_table")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[string]*table{}
	for rows.Next() {
		var t table
		var cols string
		if err := rows.Scan(&t.Name, &t.Store, &cols); err != nil {
			return nil, err
		}
		if err := json.Unmarshal([]byte(cols), &t.Columns); err != nil {
			return nil, fmt.Errorf("history: columns of %s: %w", t.Name, err)
		}
		out[t.Name] = &t
	}
	return out, rows.Err()
}

func (t *table) has(col string) bool {
	for _, c := range t.Columns {
		if c.Name == col {
			return true
		}
	}
	return false
}

func quote(name string) string { return `"` + strings.ReplaceAll(name, `"`, `""`) + `"` }

func quoteAll(names []string) string {
	q := make([]string, len(names))
	for i, n := range names {
		q[i] = quote(n)
	}
	return strings.Join(q, ", ")
}

// hasher hashes the rows of one table: every column but the noise, by name, NULL left out (a
// column added since, empty in every row, changes no hash).
type hasher struct {
	order []int // indices of the columns that count, by name
	names []string
}

func newHasher(cols []string) *hasher {
	h := &hasher{names: cols}
	for i, c := range cols {
		if !noise[c] {
			h.order = append(h.order, i)
		}
	}
	sort.Slice(h.order, func(a, b int) bool { return cols[h.order[a]] < cols[h.order[b]] })
	return h
}

func (h *hasher) sum(values []any) []byte {
	d := sha256.New()
	buf := make([]byte, 0, 64)
	for _, i := range h.order {
		v := values[i]
		if v == nil {
			continue
		}
		buf = append(buf[:0], h.names[i]...)
		buf = append(buf, 0)
		switch x := v.(type) {
		case int64:
			buf = strconv.AppendInt(append(buf, 'i'), x, 10)
		case float64:
			buf = strconv.AppendFloat(append(buf, 'r'), x, 'g', -1, 64)
		case string:
			buf = append(append(buf, 't'), x...)
		case []byte:
			buf = append(append(buf, 'b'), x...)
		case bool:
			b := int64(0)
			if x {
				b = 1
			}
			buf = strconv.AppendInt(append(buf, 'i'), b, 10)
		case time.Time:
			buf = append(append(buf, 't'), x.UTC().Format(time.RFC3339Nano)...)
		default:
			buf = append(append(buf, '?'), fmt.Sprint(x)...)
		}
		buf = append(buf, 0x1f)
		d.Write(buf)
	}
	return d.Sum(nil)[:16]
}

// Status counts what the history holds.
type Status struct {
	Snapshots     int
	First, Last   *Snapshot
	Tables        int
	RowVersions   int64 // rows stored, all versions
	CurrentRows   int64 // rows of the newest state
	Bytes         int64 // size of the file
	PrivacyDigest string
}

// Status reads what the history holds.
func (h *History) Status(ctx context.Context) (Status, error) {
	var st Status
	snaps, err := h.Snapshots(ctx)
	if err != nil {
		return st, err
	}
	st.Snapshots = len(snaps)
	if len(snaps) > 0 {
		st.First, st.Last = &snaps[0], &snaps[len(snaps)-1]
	}
	tables, err := loadTables(ctx, h.db)
	if err != nil {
		return st, err
	}
	st.Tables = len(tables)
	for _, t := range tables {
		var all, open int64
		if err := h.db.QueryRowContext(ctx, "SELECT COUNT(*), COUNT(*) FILTER (WHERE _to IS NULL) FROM "+quote(t.Store)).Scan(&all, &open); err != nil {
			return st, err
		}
		st.RowVersions += all
		st.CurrentRows += open
	}
	if info, err := os.Stat(h.path); err == nil {
		st.Bytes = info.Size()
	}
	st.PrivacyDigest = readMeta(ctx, h.db, "privacy_digest")
	return st, nil
}

// Export writes a consistent copy of the whole history to out, which must not exist: the full
// export, readable with any SQLite (docs/radix/history.md says how to read it).
func (h *History) Export(ctx context.Context, out string) error {
	if _, err := os.Stat(out); err == nil {
		return fmt.Errorf("history: %s exists", out)
	}
	_, err := h.db.ExecContext(ctx, "VACUUM INTO ?", out)
	return err
}

// timeOf is the valid_from of a sequence number, for Log.
func (h *History) times(ctx context.Context) (map[int64]time.Time, error) {
	rows, err := h.db.QueryContext(ctx, "SELECT seq, valid_from FROM history_snapshot")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[int64]time.Time{}
	for rows.Next() {
		var seq int64
		var from string
		if err := rows.Scan(&seq, &from); err != nil {
			return nil, err
		}
		out[seq], _ = time.Parse(timeFormat, from)
	}
	return out, rows.Err()
}

// equal compares two values as SQLite would store them.
func equal(a, b any) bool {
	switch x := a.(type) {
	case []byte:
		y, ok := b.([]byte)
		return ok && string(x) == string(y)
	case float64:
		y, ok := b.(float64)
		return ok && (x == y || (math.IsNaN(x) && math.IsNaN(y)))
	}
	return a == b
}
