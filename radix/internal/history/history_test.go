package history

import (
	"context"
	"database/sql"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// The schema of a miniature snapshot: a table keyed by its natural key, one WITHOUT ROWID, one whose
// INTEGER PRIMARY KEY is the build's row number (event_date), one whose number a foreign key
// refers to (program_area), and meta.
const miniSchema = `
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT) WITHOUT ROWID;
CREATE TABLE module (id TEXT PRIMARY KEY, title TEXT NOT NULL, credits INTEGER, remarks TEXT, fetched_at TEXT NOT NULL);
CREATE TABLE module_person (module_id TEXT NOT NULL, ord INTEGER NOT NULL, name TEXT NOT NULL, PRIMARY KEY (module_id, ord)) WITHOUT ROWID;
CREATE TABLE event (id TEXT PRIMARY KEY, title TEXT NOT NULL);
CREATE TABLE event_person (event_id TEXT NOT NULL, ord INTEGER NOT NULL, name TEXT NOT NULL, PRIMARY KEY (event_id, ord)) WITHOUT ROWID;
CREATE TABLE event_date (id INTEGER PRIMARY KEY, event_id TEXT NOT NULL REFERENCES event(id), ord INTEGER NOT NULL, room TEXT, instructor TEXT);
CREATE TABLE program_area (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES program_area(id), label TEXT NOT NULL);
CREATE INDEX idx_event_date_event ON event_date(event_id);
CREATE VIEW v_module AS SELECT id, title, credits FROM module;
`

type snap struct {
	digest, changedAt string
	rows              string // INSERT statements
	extra             string // statements after the schema (a migration)
}

func writeSnapshot(t *testing.T, dir string, n int, s snap) string {
	t.Helper()
	path := filepath.Join(dir, fmt.Sprintf("catalog-%d.db", n))
	db, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	db.SetMaxOpenConns(1)
	stmts := miniSchema + s.extra + s.rows + fmt.Sprintf(
		"\nINSERT INTO meta VALUES ('content_digest', '%s'), ('data_changed_at', '%s'), ('radix_version', '0.15.0');\nPRAGMA user_version = %d;",
		s.digest, s.changedAt, 24+n)
	if _, err := db.Exec(stmts); err != nil {
		t.Fatalf("snapshot %d: %v", n, err)
	}
	return path
}

const rows1 = `
INSERT INTO module VALUES ('1', 'Analysis', 6, 'Fragen an Max Mustermann', '2026-11-01T00:00:00Z'), ('2', 'Algebra', 8, NULL, '2026-11-01T00:00:00Z');
INSERT INTO module_person VALUES ('1', 1, 'Mustermann, Max'), ('2', 1, 'Musterfrau, Erika');
INSERT INTO event VALUES ('100', 'Analysis'), ('200', 'Algebra');
INSERT INTO event_person VALUES ('100', 1, 'Mustermann, Max , Prof.'), ('200', 1, 'Musterfrau, Erika');
INSERT INTO event_date VALUES (1, '100', 1, 'HS 1', 'Mustermann'), (2, '200', 1, 'HS 2', 'Musterfrau');
INSERT INTO program_area VALUES (1, NULL, 'Gesamtkonto'), (2, 1, 'Pflicht');
`

// The second state: Analysis has 8 credits, Algebra was only fetched again, a new event is written
// before the others so the build's numbers of event_date move, an area is gone.
const rows2 = `
INSERT INTO module VALUES ('1', 'Analysis', 8, 'Fragen an Max Mustermann', '2026-11-01T00:00:00Z'), ('2', 'Algebra', 8, NULL, '2026-11-10T00:00:00Z');
INSERT INTO module_person VALUES ('1', 1, 'Mustermann, Max'), ('2', 1, 'Musterfrau, Erika');
INSERT INTO event VALUES ('050', 'Vorkurs'), ('100', 'Analysis'), ('200', 'Algebra');
INSERT INTO event_person VALUES ('100', 1, 'Mustermann, Max , Prof.'), ('200', 1, 'Musterfrau, Erika');
INSERT INTO event_date VALUES (1, '050', 1, 'HS 3', NULL), (2, '100', 1, 'HS 1', 'Mustermann'), (3, '200', 1, 'HS 2', 'Musterfrau');
INSERT INTO program_area VALUES (1, NULL, 'Gesamtkonto');
`

// dump renders every table of a database but meta, its rows sorted, the build's row numbers of
// event_date left out: what a reader of the snapshot sees.
func dump(t *testing.T, path string) string {
	t.Helper()
	db, err := sql.Open("sqlite", "file:"+path+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	var out []string
	for _, q := range []string{
		"SELECT id, title, credits, remarks FROM module",
		"SELECT module_id, ord, name FROM module_person",
		"SELECT id, title FROM event",
		"SELECT event_id, ord, name FROM event_person",
		"SELECT event_id, ord, room, instructor FROM event_date",
		"SELECT id, parent_id, label FROM program_area",
		"SELECT id, title, credits FROM v_module",
	} {
		rows, err := db.Query(q)
		if err != nil {
			out = append(out, q+": "+err.Error())
			continue
		}
		cols, _ := rows.Columns()
		var lines []string
		for rows.Next() {
			vals := make([]sql.NullString, len(cols))
			ptrs := make([]any, len(cols))
			for i := range vals {
				ptrs[i] = &vals[i]
			}
			if err := rows.Scan(ptrs...); err != nil {
				t.Fatal(err)
			}
			parts := make([]string, len(cols))
			for i, v := range vals {
				parts[i] = "∅"
				if v.Valid {
					parts[i] = v.String
				}
			}
			lines = append(lines, strings.Join(parts, "|"))
		}
		rows.Close()
		sort.Strings(lines)
		out = append(out, q+"\n  "+strings.Join(lines, "\n  "))
	}
	return strings.Join(out, "\n")
}

func TestRecordStoresOnlyWhatChangedAndAtRebuildsEveryState(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	h, err := Open(filepath.Join(dir, "history.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	s1 := writeSnapshot(t, dir, 1, snap{digest: "d1", changedAt: "2026-11-19T04:00:00Z", rows: rows1})
	s2 := writeSnapshot(t, dir, 2, snap{digest: "d2", changedAt: "2026-11-20T04:00:00Z", rows: rows2})

	first, err := h.Record(ctx, s1, RecordOptions{})
	if err != nil || first == nil {
		t.Fatalf("first record: %v %v", first, err)
	}
	if first.RowsOpened != 15 || first.RowsClosed != 0 || first.Seq != 1 { // 3 of meta, 2 of each other table
		t.Errorf("first: %+v", first)
	}
	again, err := h.Record(ctx, s1, RecordOptions{})
	if err != nil || again != nil {
		t.Fatalf("the same content was recorded again: %+v %v", again, err)
	}
	second, err := h.Record(ctx, s2, RecordOptions{})
	if err != nil || second == nil {
		t.Fatalf("second record: %v %v", second, err)
	}
	// Opened: Analysis with 8 credits, the event 050 and its date, the meta digest and its time.
	// Closed: Analysis with 6, area 2, the meta digest and its time. Algebra's fetch and the new
	// numbers of event_date are no change.
	if second.RowsOpened != 5 || second.RowsClosed != 4 {
		t.Errorf("second: opened %d, closed %d", second.RowsOpened, second.RowsClosed)
	}

	for i, c := range []struct {
		at   string
		want string
	}{{"2026-11-19T12:00:00Z", s1}, {"2026-11-20T04:00:00Z", s2}, {"2027-01-01T00:00:00Z", s2}} {
		out := filepath.Join(dir, fmt.Sprintf("at-%d.db", i))
		at, _ := time.Parse(time.RFC3339, c.at)
		state, err := h.At(ctx, at, out)
		if err != nil {
			t.Fatalf("At(%s): %v", c.at, err)
		}
		if got, want := dump(t, out), dump(t, c.want); got != want {
			t.Errorf("At(%s) = state %d:\n%s\nwant:\n%s", c.at, state.Seq, got, want)
		}
	}
	if _, err := h.At(ctx, time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC), filepath.Join(dir, "none.db")); err != ErrBefore {
		t.Errorf("before the first state: %v", err)
	}

	versions, err := h.Log(ctx, "module", map[string]string{"id": "1"})
	if err != nil {
		t.Fatal(err)
	}
	if len(versions) != 2 || versions[0].To != versions[1].From || strings.Join(versions[1].Changed, ",") != "credits" || versions[1].Values["credits"] != int64(8) || !versions[1].To.IsZero() {
		t.Errorf("log of module 1: %+v", versions)
	}
	areas, err := h.Log(ctx, "program_area", map[string]string{"id": "2"})
	if err != nil || len(areas) != 1 || !areas[0].Gone {
		t.Errorf("log of the area that went: %+v %v", areas, err)
	}

	st, err := h.Status(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if st.Snapshots != 2 || st.RowVersions != 20 || st.CurrentRows != 16 {
		t.Errorf("status: %+v", st)
	}
	if err := h.Export(ctx, filepath.Join(dir, "export.db")); err != nil {
		t.Fatal(err)
	}
}

func TestASchemaThatGrowsKeepsTheOldStates(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	h, err := Open(filepath.Join(dir, "history.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	s1 := writeSnapshot(t, dir, 1, snap{digest: "d1", changedAt: "2026-11-19T04:00:00Z", rows: rows1})
	// A migration adds a column; it is empty in every row but one.
	rows := strings.Replace(rows1, "INSERT INTO event VALUES ('100', 'Analysis'), ('200', 'Algebra');",
		"INSERT INTO event VALUES ('100', 'Analysis', 'HS1'), ('200', 'Algebra', NULL);", 1)
	s2 := writeSnapshot(t, dir, 2, snap{digest: "d2", changedAt: "2026-11-21T04:00:00Z", extra: "ALTER TABLE event ADD COLUMN room_short TEXT;", rows: rows})
	if _, err := h.Record(ctx, s1, RecordOptions{}); err != nil {
		t.Fatal(err)
	}
	second, err := h.Record(ctx, s2, RecordOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if second.RowsOpened != 3 || second.RowsClosed != 3 { // event 100 with its short form, the meta digest and its time
		t.Errorf("a column empty in every other row changed them: %+v", second)
	}
	old := filepath.Join(dir, "old.db")
	if _, err := h.At(ctx, time.Date(2026, 11, 20, 0, 0, 0, 0, time.UTC), old); err != nil {
		t.Fatal(err)
	}
	db, _ := sql.Open("sqlite", old)
	defer db.Close()
	var n int
	if err := db.QueryRow("SELECT COUNT(*) FROM pragma_table_info('event') WHERE name = 'room_short'").Scan(&n); err != nil || n != 0 {
		t.Errorf("the state before the migration has its schema: room_short %d %v", n, err)
	}
	var v int
	if err := db.QueryRow("PRAGMA user_version").Scan(&v); err != nil || v != 25 {
		t.Errorf("user_version of the old state: %d %v", v, err)
	}
}

func TestTheRegisterReachesEveryVersion(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	h, err := Open(filepath.Join(dir, "history.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	s1 := writeSnapshot(t, dir, 1, snap{digest: "d1", changedAt: "2026-11-19T04:00:00Z", rows: rows1})
	if _, err := h.Record(ctx, s1, RecordOptions{}); err != nil {
		t.Fatal(err)
	}
	// Max Mustermann asks to be taken out. The Radix that builds the catalog has the register from
	// now on: its next snapshot no longer names him.
	people := []privacy.Person{{ID: 1, Surname: "Mustermann", Given: "Max"}}
	rows2 := strings.NewReplacer("'Fragen an Max Mustermann'", "'Fragen an [entfernt]'", "('1', 1, 'Mustermann, Max'), ", "", "('100', 1, 'Mustermann, Max , Prof.'), ", "", "'HS 1', 'Mustermann'", "'HS 1', NULL").Replace(rows1)
	s2 := writeSnapshot(t, dir, 2, snap{digest: "d2", changedAt: "2026-11-20T04:00:00Z", rows: rows2})

	if _, err := h.Record(ctx, s2, RecordOptions{NoRegister: true}); err != nil {
		t.Fatal(err) // no register was ever applied: recording without one is allowed
	}
	if _, err := h.Redact(ctx, people); err != nil {
		t.Fatal(err)
	}
	if _, err := h.Record(ctx, s2, RecordOptions{NoRegister: true}); err == nil {
		t.Fatal("after the register was applied, a record without it was taken")
	}

	for _, at := range []string{"2026-11-19T12:00:00Z", "2026-11-20T12:00:00Z"} {
		out := filepath.Join(dir, "at-"+at[:10]+".db")
		tm, _ := time.Parse(time.RFC3339, at)
		if _, err := h.At(ctx, tm, out); err != nil {
			t.Fatal(err)
		}
		if got, want := dump(t, out), dump(t, s2); got != want {
			t.Errorf("At(%s) still differs from the redacted state:\n%s\nwant:\n%s", at, got, want)
		}
	}
	// The versions of module 1 and of Analysis's date differed only by him; they are one now.
	for table, filter := range map[string]map[string]string{"module": {"id": "1"}, "event_date": {"event_id": "100"}} {
		versions, err := h.Log(ctx, table, filter)
		if err != nil {
			t.Fatal(err)
		}
		if len(versions) != 1 || !versions[0].To.IsZero() {
			t.Errorf("%s %v: %d versions after the redaction: %+v", table, filter, len(versions), versions)
		}
	}
	persons, err := h.Log(ctx, "module_person", map[string]string{"module_id": "1"})
	if err != nil || len(persons) != 0 {
		t.Errorf("his row of module_person is in no version: %+v %v", persons, err)
	}
	if done, _, err := h.Sync(ctx, people); err != nil || done {
		t.Errorf("the same register applied again: %v %v", done, err)
	}

	// A snapshot of a Radix without the register names him again: Record takes him out first.
	s3 := writeSnapshot(t, dir, 3, snap{digest: "d3", changedAt: "2026-11-21T04:00:00Z", rows: strings.Replace(rows1, "'Algebra', 8", "'Algebra', 9", 1)})
	third, err := h.Record(ctx, s3, RecordOptions{People: people})
	if err != nil {
		t.Fatal(err)
	}
	if third.RowsOpened != 3 { // Algebra with 9 credits, the meta digest and time; nothing of him
		t.Errorf("third: %+v", third)
	}
	out := filepath.Join(dir, "at-3.db")
	if _, err := h.At(ctx, time.Date(2026, 11, 22, 0, 0, 0, 0, time.UTC), out); err != nil {
		t.Fatal(err)
	}
	if d := dump(t, out); strings.Contains(d, "Mustermann, Max") || strings.Contains(d, "Max Mustermann") || strings.Contains(d, "|Mustermann") {
		t.Errorf("the state of a snapshot without the register names him:\n%s", d)
	}
}
