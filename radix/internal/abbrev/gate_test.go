package abbrev

import (
	"bufio"
	"database/sql"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

// TestGate derives every abbreviation of a real catalog and compares it with the forms the
// rules gave on 2026-09-25 (testdata/gate): Informatik B.Sc., Maschinenbau B.Sc. 2021,
// Betriebswirtschaftslehre B.A. 2024, Soziale Arbeit B.A. 2020 and the default of every
// module. It runs only with a snapshot, so that every rule change shows what it moves:
//
//	RADIX_ABBREV_GATE=/path/to/catalog-abca4baa1d8f8d8e.db go test ./internal/abbrev -run TestGate -v
//
// It also fails when a derived form is blocked or two heads of a program share a stem, and
// with RADIX_ABBREV_GATE_STRICT=1 on any difference. The files hold the forms of the snapshot
// catalog-abca4baa1d8f8d8e.db (schema 8; a newer catalog moves some of them by itself). A rule
// change that moves forms on purpose writes them again, keeping each file's comment lines
// (the rules they record: add the new one by hand):
//
//	RADIX_ABBREV_GATE=/path/to/catalog-abca4baa1d8f8d8e.db RADIX_ABBREV_GATE_WRITE=1 go test ./internal/abbrev -run TestGate
func TestGate(t *testing.T) {
	path := os.Getenv("RADIX_ABBREV_GATE")
	if path == "" {
		t.Skip("RADIX_ABBREV_GATE not set")
	}
	db, err := sql.Open("sqlite", "file:"+filepath.ToSlash(path)+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	in, err := ReadCatalog(db)
	if err != nil {
		t.Fatal(err)
	}
	overrides, err := Overrides()
	if err != nil {
		t.Fatal(err)
	}
	res := Derive(in.Modules, in.Members, in.Titles, overrides)

	// No derived form of the real catalog is blocked, and no stem is shared by two heads of a
	// program: the rules hold on real titles, not only on the examples above.
	title := map[string]string{}
	for _, m := range in.Modules {
		title[m.ID] = m.Title
	}
	blockedIn := func(scope string, choices map[string]Choice) {
		for id, c := range choices {
			if why, b := Blocked(c.Abbrev, title[id]); b && (!c.Override || c.Twin) {
				t.Errorf("%s %s %q → %s, blocked (%s)", scope, id, title[id], c.Abbrev, why)
			}
		}
	}
	blockedIn("default", res.Defaults)
	d := &deriver{split: newSplitter(Vocabulary(in.Titles)), titles: title, parsed: map[string]*parsed{}}
	for pid, choices := range res.Programs {
		blockedIn(pid, choices)
		heads := map[string]string{}
		for id, c := range choices {
			s, head := stemKey(c.Abbrev), d.parse(id).seriesKey
			if other, ok := heads[s]; ok && other != head && !c.Twin {
				t.Errorf("%s: %s (%s) shares its stem with %q", pid, c.Abbrev, title[id], other)
			}
			heads[s] = head
		}
	}

	files, _ := filepath.Glob("testdata/gate/*.tsv")
	sort.Strings(files)
	total := 0
	for _, file := range files {
		name := strings.TrimSuffix(filepath.Base(file), ".tsv")
		got := res.Defaults
		if name != "defaults" {
			got = res.Programs[name]
		}
		if os.Getenv("RADIX_ABBREV_GATE_WRITE") != "" {
			writeGate(t, file, got, title)
			continue
		}
		want := readGate(t, file)
		var plus, minus []string
		for id, w := range want {
			if g, ok := got[id]; !ok || g.Abbrev != w[0] {
				minus = append(minus, id+" "+w[0]+" ("+w[1]+")")
				if ok {
					plus = append(plus, id+" "+g.Abbrev+" ("+w[1]+")")
				}
			}
		}
		for id, g := range got {
			if _, ok := want[id]; !ok {
				plus = append(plus, id+" "+g.Abbrev+" (new)")
			}
		}
		sort.Strings(plus)
		sort.Strings(minus)
		t.Logf("%s: +%d/−%d of %d", name, len(plus), len(minus), len(want))
		for _, s := range minus {
			t.Logf("  − %s", s)
		}
		for _, s := range plus {
			t.Logf("  + %s", s)
		}
		total += len(plus) + len(minus)
	}
	if total > 0 && os.Getenv("RADIX_ABBREV_GATE_STRICT") != "" {
		t.Errorf("%d differences from the reference", total)
	}
}

// writeGate writes a gate file again: its comment lines as they are, then module<TAB>abbrev<TAB>title
// for every module of got, by module number.
func writeGate(t *testing.T, file string, got map[string]Choice, title map[string]string) {
	t.Helper()
	old, err := os.ReadFile(file)
	if err != nil {
		t.Fatal(err)
	}
	var b strings.Builder
	for _, line := range strings.Split(string(old), "\n") {
		if !strings.HasPrefix(line, "#") {
			break
		}
		b.WriteString(strings.TrimRight(line, "\r") + "\n")
	}
	ids := make([]string, 0, len(got))
	for id := range got {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		b.WriteString(id + "\t" + got[id].Abbrev + "\t" + title[id] + "\n")
	}
	if err := os.WriteFile(file, []byte(b.String()), 0o644); err != nil {
		t.Fatal(err)
	}
	t.Logf("%s: written, %d modules", file, len(ids))
}

// readGate reads module<TAB>abbrev<TAB>title lines.
func readGate(t *testing.T, file string) map[string][2]string {
	t.Helper()
	f, err := os.Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	out := map[string][2]string{}
	s := bufio.NewScanner(f)
	for s.Scan() {
		line := strings.TrimRight(s.Text(), "\r")
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		cols := strings.SplitN(line, "\t", 3)
		if len(cols) < 3 {
			t.Fatalf("%s: bad line %q", file, line)
		}
		out[cols[0]] = [2]string{cols[1], cols[2]}
	}
	if err := s.Err(); err != nil {
		t.Fatal(err)
	}
	return out
}
