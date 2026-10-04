package store

import (
	"errors"
	"net/http"
	"testing"
	"time"
)

// imported builds an import of rawURL that another program fetched first at first and last at
// last, with status and body (stored as a blob).
func imported(t *testing.T, s *Store, rawURL string, status int, body string, first, last time.Time) Imported {
	t.Helper()
	key, u, host, err := Canonical(rawURL, "", "")
	if err != nil {
		t.Fatalf("Canonical(%q) failed: %v", rawURL, err)
	}
	b := putBlob(t, s, body)
	return Imported{Key: key, URL: u, Host: host, Source: "module_page", Status: status, Hash: b.Hash, Size: b.Size,
		FetchedAt: first, CheckedAt: last}
}

func recordImport(t *testing.T, s *Store, f Imported) (Entry, Version, string) {
	t.Helper()
	e, v, result, err := s.RecordImport(f)
	if err != nil {
		t.Fatalf("RecordImport(%s) failed: %v", f.URL, err)
	}
	return e, v, result
}

// An answer of another program's archive is stored with that program's times, and the journal
// says when it was imported: a follower is not taken for days behind.
func TestImportKeepsTheTimesOfTheArchive(t *testing.T) {
	s := openTestStore(t)
	importedAt := t0.Add(1000 * time.Hour)
	s.now = func() time.Time { return importedAt }
	const u = "https://www.b-tu.de/modul/11101"
	first, last := t0, t0.Add(48*time.Hour)

	e, v, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "<html>Übung</html>", first, last))
	if result != ImportCreated || !v.FetchedAt.Equal(first) || !v.CheckedAt.Equal(last) || e.CurrentVersion != v.ID ||
		!e.CreatedAt.Equal(first) || e.Source != "module_page" || len(v.Header) != 0 || v.FinalURL != "" {
		t.Fatalf("import: %s, entry %+v, version %+v", result, e, v)
	}
	seq, _ := s.Position()
	j, err := s.JournalEntryAt(seq)
	if err != nil || j.Op != OpVersion || j.Blob != v.Hash || !j.At.Equal(importedAt) {
		t.Fatalf("journal %+v (err %v), want a version entry at the time of the import", j, err)
	}

	// The same once more: nothing to write.
	_, again, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "<html>Übung</html>", first, last))
	if result != ImportUnchanged || again.ID != v.ID {
		t.Fatalf("the same import again: %s, version %+v", result, again)
	}
	if now, _ := s.Position(); now != seq {
		t.Fatalf("the same import wrote journal entries: seq %d, was %d", now, seq)
	}

	// A later fetch of the same content only moves checked_at, as a fetch would.
	later := last.Add(24 * time.Hour)
	_, checked, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "<html>Übung</html>", first.Add(time.Hour), later))
	if result != ImportChecked || checked.ID != v.ID || !checked.FetchedAt.Equal(first) || !checked.CheckedAt.Equal(later) {
		t.Fatalf("a later import of the same content: %s, version %+v", result, checked)
	}
	if j, _ := s.JournalEntryAt(seq + 1); j.Op != OpCheck {
		t.Fatalf("journal entry %+v, want a check", j)
	}
	// An earlier one leaves it as it is.
	if _, _, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "<html>Übung</html>", first, last)); result != ImportUnchanged {
		t.Fatalf("an earlier import of the same content: %s, want %s", result, ImportUnchanged)
	}

	// Cortex's own fetch of the same content is a check of the imported version.
	_, fetchedV, changed, _ := record(t, s, fetched(t, s, u, http.StatusOK, "<html>Übung</html>", later.Add(time.Hour)))
	if changed || fetchedV.ID != v.ID || !fetchedV.CheckedAt.Equal(later.Add(time.Hour)) {
		t.Fatalf("a fetch after the import: changed %v, version %+v", changed, fetchedV)
	}
}

func TestImportOfOtherContentSupersedesOnlyWhatIsOlder(t *testing.T) {
	s := openTestStore(t)
	s.now = func() time.Time { return t0.Add(1000 * time.Hour) }
	const u = "https://qis.b-tu.de/qisserver/rds?state=verpublish&veranstaltung.veranstid=1"
	_, v1, _ := recordImport(t, s, imported(t, s, u, http.StatusOK, "Montag 9 Uhr", t0, t0.Add(time.Hour)))

	// Other content, fetched after the current version was last checked: it takes over then.
	e, v2, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "Dienstag 11 Uhr", t0.Add(48*time.Hour), t0.Add(72*time.Hour)))
	if result != ImportCreated || v2.ID == v1.ID || e.CurrentVersion != v2.ID || !v2.FetchedAt.Equal(t0.Add(48*time.Hour)) {
		t.Fatalf("newer content: %s, entry %+v, version %+v", result, e, v2)
	}
	_, versions, err := s.Versions(e.Key)
	if err != nil || len(versions) != 2 || !versions[1].SupersededAt.Equal(t0.Add(48*time.Hour)) {
		t.Fatalf("Versions = %+v (err %v), want the first superseded when the second was first fetched", versions, err)
	}

	// Other content the program last saw before Cortex's current version was checked: older.
	seq, _ := s.Position()
	if _, cur, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "Mittwoch 13 Uhr", t0, t0.Add(72*time.Hour))); result != ImportOlder || cur.ID != v2.ID {
		t.Fatalf("older content: %s, version %+v, want %s and the current version", result, cur, ImportOlder)
	}
	if now, _ := s.Position(); now != seq {
		t.Fatalf("an older import wrote journal entries: seq %d, was %d", now, seq)
	}

	// Other content first seen before the current version's last check, last seen after it:
	// Cortex knows it from that check on, so the versions follow each other.
	_, v3, result := recordImport(t, s, imported(t, s, u, http.StatusNotFound, "", t0.Add(60*time.Hour), t0.Add(96*time.Hour)))
	if result != ImportCreated || !v3.FetchedAt.Equal(t0.Add(72*time.Hour)) || v3.Status != http.StatusNotFound {
		t.Fatalf("overlapping content: %s, version %+v, want first fetched at the last check of the one before", result, v3)
	}
	for _, tc := range []struct {
		at   time.Time
		want int64
	}{
		{t0, v1.ID},
		{t0.Add(48 * time.Hour), v2.ID},
		{t0.Add(72*time.Hour - time.Microsecond), v2.ID},
		{t0.Add(72 * time.Hour), v3.ID},
	} {
		if _, v, err := s.LookupAt(e.Key, tc.at); err != nil || v.ID != tc.want {
			t.Errorf("LookupAt(%v) = version %d (err %v), want %d", tc.at, v.ID, err, tc.want)
		}
	}
}

func TestImportRefusesTimesThatDoNotFit(t *testing.T) {
	s := openTestStore(t)
	now := t0.Add(1000 * time.Hour)
	s.now = func() time.Time { return now }
	const u = "https://www.b-tu.de/modul/11101"
	for name, times := range map[string][2]time.Time{
		"first after last":   {t0.Add(time.Hour), t0},
		"no first":           {{}, t0},
		"no last":            {t0, {}},
		"last in the future": {t0, now.Add(2 * time.Minute)},
	} {
		f := imported(t, s, u, http.StatusOK, "x", times[0], times[1])
		if _, _, _, err := s.RecordImport(f); !errors.Is(err, ErrInvalidInput) {
			t.Errorf("%s: err %v, want ErrInvalidInput", name, err)
		}
	}
	// A clock of another host a little ahead is no reason to refuse.
	if _, _, result := recordImport(t, s, imported(t, s, u, http.StatusOK, "x", t0, now.Add(30*time.Second))); result != ImportCreated {
		t.Fatalf("checked_at 30 s ahead: %s", result)
	}
	if seq, _ := s.Position(); seq != 1 {
		t.Fatalf("the refused imports wrote journal entries: seq %d, want 1", seq)
	}
	f := imported(t, s, "https://www.b-tu.de/modul/2", http.StatusOK, "y", t0, t0)
	f.Hash = "0000000000000000000000000000000000000000000000000000000000000000"
	if _, _, _, err := s.RecordImport(f); !errors.Is(err, ErrBlobMissing) {
		t.Fatalf("an import of a blob that is not stored: err %v, want ErrBlobMissing", err)
	}
}

// A follower that replays the journal of imports has the same index as the leader.
func TestFollowerOfImportsIsIdenticalToTheLeader(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	leader.now = func() time.Time { return t0.Add(1000 * time.Hour) }
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, t0); err != nil {
		t.Fatal(err)
	}
	for i, body := range []string{"a", "b", "a", "c"} {
		u := "https://www.b-tu.de/modul/" + string(rune('1'+i%2))
		first := t0.Add(time.Duration(i) * 24 * time.Hour)
		recordImport(t, leader, imported(t, leader, u, http.StatusOK, body, first, first.Add(time.Hour)))
	}
	follow(t, leader, follower)
	sameDump(t, "after the imports", dump(t, follower), dump(t, leader))
}
