package history

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// TestRealSnapshots records real snapshots in their order and checks that At gives back each of
// them: every table, every row, as published (but the build's row numbers). Skipped unless
// HISTORY_SNAPSHOTS names them, comma-separated, oldest first: canary's and betula.app's
// /api/db (docs/folia/frontend.md says how to fetch them).
func TestRealSnapshots(t *testing.T) {
	list := os.Getenv("HISTORY_SNAPSHOTS")
	if list == "" {
		t.Skip("HISTORY_SNAPSHOTS names no snapshots")
	}
	ctx := context.Background()
	dir := t.TempDir()
	h, err := Open(filepath.Join(dir, "history.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	paths := strings.Split(list, ",")
	var states []*Snapshot
	for i, p := range paths {
		start := time.Now()
		// Each a day after the one before, whatever their meta says: two instances' snapshots of the same day.
		s, err := h.Record(ctx, p, RecordOptions{ValidFrom: time.Date(2026, 11, 19+i, 6, 0, 0, 0, time.UTC), People: peopleFromEnv()})
		if err != nil {
			t.Fatalf("record %s: %v", p, err)
		}
		st, _ := h.Status(ctx)
		info, _ := os.Stat(p)
		t.Logf("recorded %s (%.1f MB, schema %d, Radix %s) in %s: %d rows opened, %d closed; history %.1f MB, %d row versions",
			filepath.Base(p), float64(info.Size())/1e6, s.SchemaVersion, s.RadixVersion, time.Since(start).Round(time.Millisecond),
			s.RowsOpened, s.RowsClosed, float64(st.Bytes)/1e6, st.RowVersions)
		states = append(states, s)
	}
	for i, p := range paths {
		if len(peopleFromEnv()) > 0 {
			break // the states differ from the originals by what the register took out
		}
		out := filepath.Join(dir, fmt.Sprintf("at-%d.db", i))
		start := time.Now()
		if _, err := h.At(ctx, states[i].ValidFrom.Add(time.Hour), out); err != nil {
			t.Fatal(err)
		}
		t.Logf("state %d rebuilt in %s", states[i].Seq, time.Since(start).Round(time.Millisecond))
		compareSnapshots(t, p, out)
	}
}

func peopleFromEnv() []privacy.Person {
	var people []privacy.Person
	for i, entry := range strings.Split(os.Getenv("HISTORY_PEOPLE"), ";") {
		if surname, given, ok := strings.Cut(entry, ","); ok {
			people = append(people, privacy.Person{ID: int64(i + 1), Surname: strings.TrimSpace(surname), Given: strings.TrimSpace(given)})
		}
	}
	return people
}

// compareSnapshots compares every table of two snapshots as multisets of rows, leaving out the
// columns Record does not keep (the build's row numbers).
func compareSnapshots(t *testing.T, want, got string) {
	t.Helper()
	open := func(p string) *sql.DB {
		db, err := sql.Open("sqlite", "file:"+p+"?mode=ro&immutable=1")
		if err != nil {
			t.Fatal(err)
		}
		return db
	}
	a, b := open(want), open(got)
	defer a.Close()
	defer b.Close()
	ctx := context.Background()
	ddl, err := readDDL(ctx, a)
	if err != nil {
		t.Fatal(err)
	}
	referenced, err := referencedColumns(ctx, a, ddl)
	if err != nil {
		t.Fatal(err)
	}
	var va, vb int
	a.QueryRow("PRAGMA user_version").Scan(&va)
	b.QueryRow("PRAGMA user_version").Scan(&vb)
	if va != vb {
		t.Errorf("user_version %d, rebuilt %d", va, vb)
	}
	tables := 0
	for _, d := range ddl {
		if d.Type != "table" {
			continue
		}
		cols, err := privacy.Columns(ctx, a, d.Name)
		if err != nil {
			t.Fatal(err)
		}
		var names []string
		for _, c := range cols {
			if !volatile(d.Name, c, cols, referenced) {
				names = append(names, c.Name)
			}
		}
		ra, rb := rowsOf(t, a, d.Name, names), rowsOf(t, b, d.Name, names)
		if strings.Join(ra, "\n") != strings.Join(rb, "\n") {
			t.Errorf("table %s: %d rows, rebuilt %d, or they differ", d.Name, len(ra), len(rb))
			continue
		}
		tables++
	}
	t.Logf("%d tables of %s are equal", tables, filepath.Base(want))
}

func rowsOf(t *testing.T, db *sql.DB, table string, cols []string) []string {
	t.Helper()
	rows, err := db.Query("SELECT " + quoteAll(cols) + " FROM " + quote(table))
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		vals := make([]any, len(cols))
		ptrs := make([]any, len(cols))
		for i := range vals {
			ptrs[i] = &vals[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			t.Fatal(err)
		}
		out = append(out, fmt.Sprintf("%#v", vals))
	}
	sort.Strings(out)
	return out
}
