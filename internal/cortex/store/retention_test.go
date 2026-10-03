package store

import (
	"encoding/json"
	"errors"
	"testing"
	"time"
)

func TestPruneAt180DayBoundaries(t *testing.T) {
	s := openTestStore(t)
	const history = 180 * 24 * time.Hour
	now := t0.Add(400 * 24 * time.Hour)
	cutoff := now.Add(-history)
	justBefore := cutoff.Add(-time.Microsecond)

	// An entry whose first version was superseded just before the cutoff, its second at it.
	const u = "https://www.b-tu.de/modul/11101"
	_, v1, _, _ := record(t, s, fetched(t, s, u, 200, "v1", t0))
	_, v2, _, _ := record(t, s, fetched(t, s, u, 200, "v2", justBefore))
	e1, v3, _, _ := record(t, s, fetched(t, s, u, 200, "v3", cutoff))
	// An entry whose only version is very old but current.
	e2, old, _, _ := record(t, s, fetched(t, s, "https://www.b-tu.de/modul/11102", 200, "old", t0))
	// An entry without a version, which no write leaves but retention clears up.
	rawExec(t, s, `INSERT INTO entry (key, url, host, source, created_at) VALUES ('GET http://example.com/', 'http://example.com/', 'example.com', 'unknown', ?)`,
		FormatTime(t0))

	// Files: deleted just before the cutoff; deleted at it; a version superseded just before it.
	putFile(t, s, "f1", "f1 v1", "text/plain", t0)
	if _, err := s.DeleteFile("f1", justBefore); err != nil {
		t.Fatal(err)
	}
	f2v1, _, _ := putFile(t, s, "f2", "f2 v1", "text/plain", t0)
	if _, err := s.DeleteFile("f2", cutoff); err != nil {
		t.Fatal(err)
	}
	putFile(t, s, "f3", "f3 v1", "text/plain", t0)
	f3v2, _, _ := putFile(t, s, "f3", "f3 v2", "text/plain", justBefore)
	rawExec(t, s, `INSERT INTO file (name, created_at) VALUES ('f4', ?)`, FormatTime(t0))

	before, _ := s.Position()
	stats, e, err := s.Prune(now, history)
	if err != nil {
		t.Fatalf("Prune failed: %v", err)
	}
	want := PruneStats{Cutoff: cutoff, Versions: 1, FileVersions: 3, Files: 2, Entries: 1}
	if stats != want {
		t.Fatalf("Prune = %+v, want %+v", stats, want)
	}
	var p prunePayload
	if e.Op != OpPrune || e.Seq != before+1 || json.Unmarshal(e.Payload, &p) != nil || p.Cutoff != FormatTime(cutoff) || !e.At.Equal(now) {
		t.Fatalf("journal entry %+v, want op prune with the cutoff", e)
	}

	_, versions, err := s.Versions(e1.Key)
	if err != nil || len(versions) != 2 || versions[0].ID != v3.ID || versions[1].ID != v2.ID {
		t.Fatalf("versions after Prune: %+v (err %v), want %d and %d (v1 %d removed)", versions, err, v3.ID, v2.ID, v1.ID)
	}
	if _, v, err := s.Lookup(e2.Key); err != nil || v.ID != old.ID {
		t.Fatalf("an old current version was removed: %+v (err %v)", v, err)
	}
	if _, _, err := s.Versions("GET http://example.com/"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("the entry without versions is kept: %v", err)
	}
	if _, err := s.FileVersions("f1"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("f1, deleted before the cutoff, is kept: %v", err)
	}
	if fvs, err := s.FileVersions("f2"); err != nil || len(fvs) != 2 || fvs[1].ID != f2v1.ID {
		t.Fatalf("f2, deleted at the cutoff: %+v (err %v), want the tombstone and its version", fvs, err)
	}
	if fvs, err := s.FileVersions("f3"); err != nil || len(fvs) != 1 || fvs[0].ID != f3v2.ID {
		t.Fatalf("f3: %+v (err %v), want only the current version", fvs, err)
	}
	if n := countRows(t, s, "file"); n != 2 {
		t.Fatalf("%d files, want f2 and f3", n)
	}

	// Nothing left to remove: no journal entry.
	stats, e, err = s.Prune(now, history)
	if err != nil || e.Seq != 0 || stats.Versions+stats.FileVersions+stats.Files+stats.Entries != 0 {
		t.Fatalf("second Prune = %+v, journal %+v (err %v), want nothing", stats, e, err)
	}
	if seq, _ := s.Position(); seq != before+1 {
		t.Fatalf("Position %d after an empty prune, want %d", seq, before+1)
	}
	if _, _, err := s.Prune(now, 0); err == nil {
		t.Fatalf("Prune with no history succeeded")
	}
}
