package store

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math/rand/v2"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"
)

// follow applies what the leader's journal has after the follower's position, as the follower
// loop does: in pages, each entry through JSON as the NDJSON stream carries it, and a blob the
// follower lacks fetched from the leader when Apply asks for it.
func follow(t *testing.T, leader, follower *Store) {
	t.Helper()
	if err := catchUp(leader, follower); err != nil {
		t.Fatal(err)
	}
}

// catchUp is follow for a goroutine that cannot fail a test itself.
func catchUp(leader, follower *Store) error {
	for {
		seq, _ := follower.Position()
		entries, err := leader.JournalAfter(seq, 7)
		if err != nil {
			return fmt.Errorf("JournalAfter(%d) failed: %w", seq, err)
		}
		if len(entries) == 0 {
			return nil
		}
		for _, e := range entries {
			line, err := json.Marshal(e)
			if err != nil {
				return err
			}
			var got JournalEntry
			if err := json.Unmarshal(line, &got); err != nil {
				return err
			}
			err = follower.Apply(got)
			if errors.Is(err, ErrBlobMissing) {
				if err := copyBlob(leader, follower, got.Blob); err != nil {
					return err
				}
				err = follower.Apply(got)
			}
			if err != nil {
				return fmt.Errorf("Apply(%d %s) failed: %w", e.Seq, e.Op, err)
			}
		}
	}
}

// copyBlob gives to the blob hash as from stores it, as the follower loop fetches it.
func copyBlob(from, to *Store, hash string) error {
	rc, gz, _, err := from.OpenStored(hash)
	if err != nil {
		return err
	}
	defer rc.Close()
	return to.ImportStored(hash, rc, gz)
}

// randomHistory runs n random writes on the leader: fetches of a few URLs with a few bodies and
// statuses, deletes, files, retention and new epochs, with the clock moving on.
func randomHistory(t *testing.T, s *Store, r *rand.Rand, n int, now *time.Time, check func(i int)) {
	t.Helper()
	urls := []string{
		"https://qis.b-tu.de/qisserver/rds?state=wtree&search=1",
		"https://qis.b-tu.de/qisserver/rds?search=1&state=wtree",
		"https://www.b-tu.de/modul/11101",
		"https://www.b-tu.de/modul/11102",
		"https://opus4.kobv.de/files/1.pdf",
		"http://example.com/a",
		"http://example.com/b",
	}
	accepts := []string{"", "", "application/json"}
	bodies := []string{"", "Montag", "Dienstag", "Mittwoch", "<html>Übung</html>"}
	statuses := []int{200, 200, 200, 404, 410}
	names := []string{"models/a", "models/b", "uploads/c.pdf", "x"}
	types := []string{"application/octet-stream", "application/pdf"}

	for i := 0; i < n; i++ {
		*now = now.Add(time.Duration(r.IntN(4*24*3600)) * time.Second).Add(time.Duration(r.IntN(1e6)) * time.Microsecond)
		switch op := r.IntN(100); {
		case op < 50:
			accept := accepts[r.IntN(len(accepts))]
			key, u, host, err := Canonical(urls[r.IntN(len(urls))], accept, "")
			if err != nil {
				t.Fatal(err)
			}
			b := putBlob(t, s, bodies[r.IntN(len(bodies))])
			f := Fetched{Key: key, URL: u, Host: host, Source: []string{"qis_tree", "module_page", ""}[r.IntN(3)],
				Accept: accept, Status: statuses[r.IntN(len(statuses))], Hash: b.Hash, Size: b.Size,
				Header: http.Header{"Content-Type": {"text/html"}, "Etag": {fmt.Sprintf(`"%d"`, r.IntN(3))}}, At: *now}
			if _, _, _, _, err := s.RecordFetch(f); err != nil {
				t.Fatalf("op %d: RecordFetch failed: %v", i, err)
			}
		case op < 57:
			key, _, _, _ := Canonical(urls[r.IntN(len(urls))], "", "")
			if _, err := s.DeleteEntry(key); err != nil && !errors.Is(err, ErrNotFound) {
				t.Fatalf("op %d: DeleteEntry failed: %v", i, err)
			}
		case op < 80:
			b := putBlob(t, s, bodies[r.IntN(len(bodies))])
			if _, _, _, err := s.PutFile(names[r.IntN(len(names))], b, types[r.IntN(len(types))], *now); err != nil {
				t.Fatalf("op %d: PutFile failed: %v", i, err)
			}
		case op < 90:
			if _, err := s.DeleteFile(names[r.IntN(len(names))], *now); err != nil && !errors.Is(err, ErrNotFound) {
				t.Fatalf("op %d: DeleteFile failed: %v", i, err)
			}
		case op < 98:
			if _, _, err := s.Prune(*now, time.Duration(10+r.IntN(30))*24*time.Hour); err != nil {
				t.Fatalf("op %d: Prune failed: %v", i, err)
			}
		default:
			if _, _, err := s.StartEpoch(fmt.Sprint("instance-", r.IntN(2)), "http://cortex_a:8100", int64(r.IntN(3)), *now); err != nil {
				t.Fatalf("op %d: StartEpoch failed: %v", i, err)
			}
		}
		if check != nil {
			check(i)
		}
	}
}

func TestFollowerBuiltFromTheJournalIsIdenticalToTheLeader(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	r := rand.New(rand.NewPCG(42, 7))
	now := t0
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, now); err != nil {
		t.Fatal(err)
	}
	pruned := 0
	randomHistory(t, leader, r, 1000, &now, func(i int) {
		if i%111 == 0 {
			follow(t, leader, follower)
			sameDump(t, fmt.Sprintf("after op %d", i), dump(t, follower), dump(t, leader))
		}
	})
	follow(t, leader, follower)
	want := dump(t, leader)
	sameDump(t, "at the end", dump(t, follower), want)

	for _, row := range want["journal"] {
		if bytes.Contains([]byte(row), []byte(`op="prune"`)) {
			pruned++
		}
	}
	if pruned < 5 || len(want["version"]) < 5 || len(want["file_version"]) < 3 {
		t.Fatalf("the history is too tame: %d prunes, %d versions, %d file versions", pruned, len(want["version"]), len(want["file_version"]))
	}
	ls, le := leader.Position()
	fs, fe := follower.Position()
	if ls != fs || le != fe || leader.OldestSeq() != follower.OldestSeq() {
		t.Fatalf("follower at %d/%d (oldest %d), leader at %d/%d (oldest %d)", fs, fe, follower.OldestSeq(), ls, le, leader.OldestSeq())
	}
	_, _, lat := leader.Head()
	_, _, fat := follower.Head()
	if !lat.Equal(fat) {
		t.Fatalf("Head at %v on the follower, %v on the leader", fat, lat)
	}
}

func TestApplyRefusesEntriesOutOfOrder(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	var entries []JournalEntry
	for i := 0; i < 3; i++ {
		_, _, _, e := record(t, leader, fetched(t, leader, fmt.Sprint("https://www.b-tu.de/modul/", i), 200, fmt.Sprint(i), t0))
		entries = append(entries, e)
	}
	for _, e := range entries {
		if err := copyBlob(leader, follower, e.Blob); err != nil {
			t.Fatal(err)
		}
	}
	if err := follower.Apply(entries[1]); !errors.Is(err, ErrOutOfOrder) {
		t.Fatalf("Apply(2) on an empty journal: err %v, want ErrOutOfOrder", err)
	}
	if err := follower.Apply(entries[0]); err != nil {
		t.Fatalf("Apply(1) failed: %v", err)
	}
	if err := follower.Apply(entries[0]); !errors.Is(err, ErrOutOfOrder) {
		t.Fatalf("Apply(1) twice: err %v, want ErrOutOfOrder", err)
	}
	if err := follower.Apply(entries[2]); !errors.Is(err, ErrOutOfOrder) {
		t.Fatalf("Apply(3) after 1: err %v, want ErrOutOfOrder", err)
	}
	if seq, _ := follower.Position(); seq != 1 || countRows(t, follower, "entry") != 1 {
		t.Fatalf("refused entries changed the follower: seq %d", seq)
	}
	// An entry with a field this binary does not know is refused, not half applied.
	odd := entries[1]
	odd.Payload = json.RawMessage(bytes.Replace(odd.Payload, []byte(`{"entry":`), []byte(`{"new_field":1,"entry":`), 1))
	if err := follower.Apply(odd); err == nil {
		t.Fatalf("Apply with an unknown field succeeded")
	}
	odd = entries[1]
	odd.Op = "rename"
	if err := follower.Apply(odd); err == nil {
		t.Fatalf("Apply of an unknown op succeeded")
	}
	if err := follower.Apply(entries[1]); err != nil {
		t.Fatalf("Apply(2) failed: %v", err)
	}
}

func TestApplyDetectsADivergedIndex(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	const u = "https://www.b-tu.de/modul/11101"
	record(t, leader, fetched(t, leader, u, 200, "a", t0))
	follow(t, leader, follower)

	// The follower lost the version the next entry supersedes.
	rawExec(t, follower, `DELETE FROM version`)
	record(t, leader, fetched(t, leader, u, 200, "b", t0.Add(time.Hour)))
	entries, err := leader.JournalAfter(1, 0)
	if err != nil || len(entries) != 1 {
		t.Fatalf("JournalAfter = %d entries (err %v)", len(entries), err)
	}
	if err := follower.Apply(entries[0]); !errors.Is(err, ErrDiverged) {
		t.Fatalf("Apply on a diverged index: err %v, want ErrDiverged", err)
	}
	if seq, _ := follower.Position(); seq != 1 {
		t.Fatalf("a refused entry moved the follower to %d", seq)
	}

	// An entry the follower has a different row for (another key under the same id).
	other := openTestStore(t)
	record(t, other, fetched(t, other, "https://www.b-tu.de/modul/2", 200, "x", t0))
	rawExec(t, other, `DELETE FROM journal`)
	other.posMu.Lock()
	other.pos = position{}
	other.posMu.Unlock()
	first, err := leader.JournalEntryAt(1)
	if err != nil {
		t.Fatal(err)
	}
	if err := other.Apply(first); !errors.Is(err, ErrDiverged) {
		t.Fatalf("Apply of a version over another row: err %v, want ErrDiverged", err)
	}
	if _, err := leader.JournalEntryAt(99); !errors.Is(err, ErrNotFound) {
		t.Fatalf("JournalEntryAt(99): err %v, want ErrNotFound", err)
	}
}

func TestSnapshotReplacesAnIndexWhileReadersRun(t *testing.T) {
	leader := openTestStore(t)
	r := rand.New(rand.NewPCG(3, 4))
	now := t0
	randomHistory(t, leader, r, 300, &now, nil)

	// The other instance has an index of its own, diverged, and readers on it.
	other := openTestStore(t)
	record(t, other, fetched(t, other, "https://www.b-tu.de/modul/1", 200, "other", t0))
	putFile(t, other, "models/a", "other", "text/plain", t0)

	stop := make(chan struct{})
	var wg sync.WaitGroup
	var mu sync.Mutex
	var problems []error
	report := func(err error) {
		if err != nil && !errors.Is(err, ErrNotFound) {
			mu.Lock()
			problems = append(problems, err)
			mu.Unlock()
		}
	}
	key, _, _, _ := Canonical("https://www.b-tu.de/modul/11101", "", "")
	for i := 0; i < 6; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				select {
				case <-stop:
					return
				default:
				}
				_, _, err := other.Lookup(key)
				report(err)
				_, _, err = other.ListEntries(EntryFilter{}, "", 10)
				report(err)
				_, err = other.GetFile("models/a")
				report(err)
				seq, _ := other.Position()
				_, err = other.JournalAfter(seq-1, 5)
				report(err)
				if _, err = other.RecentStats(); errors.Is(err, ErrStatsPending) {
					err = nil // counted in the background; not an error of the swap
				}
				report(err)
			}
		}()
	}

	pr, pw := io.Pipe()
	var seq, epoch int64
	var snapErr error
	sent := make(chan struct{})
	go func() {
		defer close(sent)
		seq, epoch, snapErr = leader.Snapshot(pw)
		pw.CloseWithError(snapErr)
	}()
	err := other.ReplaceIndex(pr)
	pr.CloseWithError(errors.New("receiver done")) // a sender still writing gives up
	<-sent
	if err != nil {
		t.Fatalf("ReplaceIndex failed: %v", err)
	}
	if snapErr != nil {
		t.Fatalf("Snapshot failed: %v", snapErr)
	}
	time.Sleep(20 * time.Millisecond) // the readers keep reading the new index
	close(stop)
	wg.Wait()
	if len(problems) > 0 {
		t.Fatalf("readers saw %d errors, the first: %v", len(problems), problems[0])
	}

	ls, le := leader.Position()
	rs, re := other.Position()
	if seq != ls || epoch != le || rs != ls || re != le || other.OldestSeq() != leader.OldestSeq() {
		t.Fatalf("snapshot at %d/%d, replaced index at %d/%d, leader at %d/%d", seq, epoch, rs, re, ls, le)
	}
	sameDump(t, "after ReplaceIndex", dump(t, other), dump(t, leader))
	if names := tmpFiles(t, other); len(names) != 0 {
		t.Fatalf("tmp keeps %v", names)
	}
	if names := tmpFiles(t, leader); len(names) != 0 {
		t.Fatalf("the leader's tmp keeps %v", names)
	}

	// The replaced index follows on from the snapshot's position.
	randomHistory(t, leader, r, 100, &now, nil)
	follow(t, leader, other)
	sameDump(t, "following after the snapshot", dump(t, other), dump(t, leader))

	// It survives a restart.
	dir := other.Dir()
	if err := other.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(dir)
	if err != nil {
		t.Fatalf("reopen after ReplaceIndex failed: %v", err)
	}
	defer reopened.Close()
	sameDump(t, "after a restart", dump(t, reopened), dump(t, leader))
}

func TestReplaceIndexRefusesWhatIsNotAnIndex(t *testing.T) {
	s := openTestStore(t)
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "kept", t0))
	want := dump(t, s)

	if err := s.ReplaceIndex(bytes.NewReader([]byte("not a database"))); err == nil {
		t.Fatalf("ReplaceIndex of garbage succeeded")
	}
	if err := s.ReplaceIndex(io.MultiReader(bytes.NewReader([]byte("SQLite format 3\x00")), failingReader{})); err == nil {
		t.Fatalf("ReplaceIndex of a broken stream succeeded")
	}
	if err := s.ReplaceIndex(bytes.NewReader(nil)); err == nil {
		t.Fatalf("ReplaceIndex of an empty stream succeeded")
	}
	// Databases of another schema: a table missing, a newer version.
	for _, change := range []string{`DROP TABLE journal`, `PRAGMA user_version = 99`} {
		other := openTestStore(t)
		rawExec(t, other, change)
		path := filepath.Join(t.TempDir(), "copy.db")
		rawExec(t, other, `VACUUM INTO ?`, path)
		copied, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		if err := s.ReplaceIndex(bytes.NewReader(copied)); err == nil {
			t.Fatalf("ReplaceIndex of an index after %q succeeded", change)
		}
	}

	sameDump(t, "after refused replacements", dump(t, s), want)
	if names := tmpFiles(t, s); len(names) != 0 {
		t.Fatalf("tmp keeps %v", names)
	}
	if _, _, err := s.Lookup("GET https://www.b-tu.de/modul/1"); err != nil {
		t.Fatalf("the index does not answer after refused replacements: %v", err)
	}
}

func TestStartEpochCountsUpFromTheHighestItKnows(t *testing.T) {
	s := openTestStore(t)
	epoch, e, err := s.StartEpoch("a", "http://cortex_a:8100", 0, t0)
	if err != nil || epoch != 1 || e.Epoch != 1 || e.Op != OpEpoch || e.Seq != 1 {
		t.Fatalf("first StartEpoch = %d, %+v (err %v)", epoch, e, err)
	}
	_, _, _, je := record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "x", t0))
	if je.Epoch != 1 {
		t.Fatalf("a write in epoch 1 carries epoch %d", je.Epoch)
	}
	if epoch, _, _ = s.StartEpoch("b", "http://cortex_b:8100", 5, t0.Add(time.Hour)); epoch != 6 {
		t.Fatalf("StartEpoch with floor 5 = %d, want 6", epoch)
	}
	if epoch, _, _ = s.StartEpoch("a", "http://cortex_a:8100", 2, t0.Add(2*time.Hour)); epoch != 7 {
		t.Fatalf("StartEpoch with a lower floor = %d, want 7", epoch)
	}
	if seq, ep := s.Position(); seq != 4 || ep != 7 {
		t.Fatalf("Position = %d/%d, want 4/7", seq, ep)
	}
	var instance string
	if err := s.r.QueryRow(`SELECT value FROM meta WHERE key = 'leader_instance'`).Scan(&instance); err != nil || instance != "a" {
		t.Fatalf("leader_instance = %q (err %v)", instance, err)
	}
}

func TestTrimJournalKeepsTheNewestEntry(t *testing.T) {
	s := openTestStore(t)
	for i := 0; i < 5; i++ {
		record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, fmt.Sprint(i), t0.Add(time.Duration(i)*24*time.Hour)))
	}
	n, err := s.TrimJournal(t0.Add(2 * 24 * time.Hour))
	if err != nil || n != 2 || s.OldestSeq() != 3 {
		t.Fatalf("TrimJournal = %d (err %v), oldest %d, want 2 removed, oldest 3", n, err, s.OldestSeq())
	}
	if n, err = s.TrimJournal(t0.Add(100 * 24 * time.Hour)); err != nil || n != 2 {
		t.Fatalf("TrimJournal of everything = %d (err %v), want 2: the newest stays", n, err)
	}
	if seq, _ := s.Position(); seq != 5 || s.OldestSeq() != 5 {
		t.Fatalf("Position %d, oldest %d, want 5, 5", seq, s.OldestSeq())
	}
	// The next entry still follows on.
	_, _, _, e := record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "next", t0.Add(200*24*time.Hour)))
	if e.Seq != 6 {
		t.Fatalf("next entry %d, want 6", e.Seq)
	}
	entries, err := s.JournalAfter(0, 0)
	if err != nil || len(entries) != 2 || entries[0].Seq != 5 {
		t.Fatalf("JournalAfter(0) = %+v (err %v), want 5 and 6", entries, err)
	}
}

func TestWaitAfterWakesOnTheNextEntry(t *testing.T) {
	s := openTestStore(t)
	done := make(chan error, 1)
	go func() { done <- s.WaitAfter(t.Context(), 0) }()
	select {
	case err := <-done:
		t.Fatalf("WaitAfter returned before a write: %v", err)
	case <-time.After(20 * time.Millisecond):
	}
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "x", t0))
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("WaitAfter = %v", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatalf("WaitAfter did not wake")
	}
	if err := s.WaitAfter(t.Context(), 0); err != nil {
		t.Fatalf("WaitAfter behind the head = %v", err)
	}
	ctx, cancel := context.WithTimeout(t.Context(), 10*time.Millisecond)
	defer cancel()
	if err := s.WaitAfter(ctx, 1); err == nil {
		t.Fatalf("WaitAfter at the head returned without an entry")
	}
}

func TestSnapshotIsConsistentWhileTheLeaderWrites(t *testing.T) {
	leader := openTestStore(t)
	r := rand.New(rand.NewPCG(5, 6))
	now := t0
	randomHistory(t, leader, r, 200, &now, nil)

	// Writes go on while the snapshot is taken; the copy must hold exactly the rows of the
	// journal position it reports, so that following on from there gives the leader's index.
	stop := make(chan struct{})
	written := make(chan int)
	go func() {
		n := 0
		defer func() { written <- n }()
		for i := 0; ; i++ {
			select {
			case <-stop:
				return
			default:
			}
			b, err := leader.PutBlob(bytes.NewReader([]byte(fmt.Sprint("busy ", i))), "", 0)
			if err != nil {
				return
			}
			key, u, host, _ := Canonical(fmt.Sprint("https://www.b-tu.de/busy/", i%5), "", "")
			if _, _, _, _, err := leader.RecordFetch(Fetched{Key: key, URL: u, Host: host, Source: "busy", Status: 200,
				Hash: b.Hash, Size: b.Size, At: t0.Add(time.Duration(i) * time.Second)}); err != nil {
				return
			}
			n++
		}
	}()
	time.Sleep(5 * time.Millisecond)
	var snap bytes.Buffer
	seq, epoch, err := leader.Snapshot(&snap)
	time.Sleep(5 * time.Millisecond)
	close(stop)
	n := <-written
	if err != nil {
		t.Fatalf("Snapshot failed: %v", err)
	}
	if head, _ := leader.Position(); head <= seq {
		t.Logf("no write came after the snapshot (%d writes, head %d, snapshot at %d)", n, head, seq)
	}

	other := openTestStore(t)
	if err := other.ReplaceIndex(&snap); err != nil {
		t.Fatalf("ReplaceIndex failed: %v", err)
	}
	if got, gotEpoch := other.Position(); got != seq || gotEpoch != epoch {
		t.Fatalf("replaced index at %d/%d, snapshot said %d/%d", got, gotEpoch, seq, epoch)
	}
	follow(t, leader, other)
	sameDump(t, "following on from a snapshot taken during writes", dump(t, other), dump(t, leader))
}

// Review 1 (non-utf8-journal-divergence): the leader wrote strings that are not UTF-8 into its
// rows as they were, while encoding/json carried U+FFFD for them in the journal, so the
// follower held other keys, collided on the second such URL (ErrDiverged), and diverged again
// after every snapshot. Such text is refused now, header values are made valid before they are
// written, and the follower stays identical through all of it.
func TestTextThatIsNotUTF8NeverDividesLeaderAndFollower(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, t0); err != nil {
		t.Fatal(err)
	}
	// The review's URLs: a raw Latin-1 byte in the query (a client's %FC, decoded by the server).
	for _, raw := range []string{"https://qis.b-tu.de/x?name=M\xfcller", "https://qis.b-tu.de/x?name=M\xe4ller"} {
		if key, _, _, err := Canonical(raw, "", ""); err == nil {
			t.Errorf("Canonical(%q) = %q, want an error", raw, key)
		}
		// A caller that builds the key itself does not get past RecordFetch either.
		b := putBlob(t, leader, raw)
		_, _, _, _, err := leader.RecordFetch(Fetched{Key: "GET " + raw, URL: raw, Host: "qis.b-tu.de", Source: "qis_tree",
			Status: 200, Hash: b.Hash, Size: b.Size, At: t0})
		if !errors.Is(err, ErrInvalidInput) {
			t.Errorf("RecordFetch of %q: err %v, want ErrInvalidInput", raw, err)
		}
	}
	valid := fetched(t, leader, "https://qis.b-tu.de/x?name=M%FCller", 200, "mueller", t0)
	for _, change := range []func(*Fetched){
		func(f *Fetched) { f.Accept = "text/html\xff" },
		func(f *Fetched) { f.AcceptLanguage = "de-\xe4" },
		func(f *Fetched) { f.Source = "Qis Tree" },
		func(f *Fetched) { f.Source = "qis\xfc" },
		func(f *Fetched) { f.Host = "qis.b-tu.de\xff" },
	} {
		f := valid
		change(&f)
		if _, _, _, _, err := leader.RecordFetch(f); !errors.Is(err, ErrInvalidInput) {
			t.Errorf("RecordFetch of %+v: err %v, want ErrInvalidInput", f, err)
		}
	}
	if _, _, _, err := leader.PutFile("f", putBlob(t, leader, "body"), "text/plain; name=\xfc", t0); !errors.Is(err, ErrInvalidInput) {
		t.Errorf("PutFile with a content type that is not UTF-8: err %v, want ErrInvalidInput", err)
	}
	if seq, _ := leader.Position(); seq != 1 {
		t.Fatalf("refused writes moved the journal to %d", seq)
	}

	// Header values that are not UTF-8 are made valid before the row is written.
	valid.Header = http.Header{"Content-Disposition": {"attachment; filename=\"M\xfcller.pdf\""}, "Etag": {"\"\xe4\xf6\""}}
	_, v, _, _ := record(t, leader, valid)
	if got := v.Header.Get("Content-Disposition"); got != "attachment; filename=\"M\uFFFDller.pdf\"" {
		t.Errorf("kept Content-Disposition %q", got)
	}
	valid.At = t0.Add(time.Hour)
	record(t, leader, valid) // a check: the row again
	putFile(t, leader, "f", "body", "text/plain; name=\"Müller\"", t0.Add(2*time.Hour))
	follow(t, leader, follower)
	sameDump(t, "after text that is not UTF-8", dump(t, follower), dump(t, leader))
	if _, fv, err := follower.Lookup(valid.Key); err != nil || fv.Header.Get("Etag") != "\"\uFFFD\"" {
		t.Fatalf("follower Lookup = %+v (err %v)", fv, err)
	}

	// The review's endless loop: a snapshot, then checks of the same rows; still identical.
	other := openTestStore(t)
	var snap bytes.Buffer
	if _, _, err := leader.Snapshot(&snap); err != nil {
		t.Fatal(err)
	}
	if err := other.ReplaceIndex(&snap); err != nil {
		t.Fatal(err)
	}
	for i := range 2 {
		valid.At = t0.Add(time.Duration(3+i) * time.Hour)
		record(t, leader, valid)
	}
	follow(t, leader, other)
	follow(t, leader, follower)
	sameDump(t, "following on from a snapshot", dump(t, other), dump(t, leader))
	sameDump(t, "following on", dump(t, follower), dump(t, leader))
}

// commit is the last line of defence: a payload that encoding/json would change is refused, so
// that no row is ever written that the journal does not carry exactly.
func TestCommitRefusesAPayloadThatJSONWouldChange(t *testing.T) {
	s := openTestStore(t)
	if _, _, err := s.StartEpoch("cortex-\xff", "http://cortex_a:8100", 0, t0); err == nil {
		t.Fatalf("StartEpoch with an instance name that is not UTF-8 succeeded")
	}
	if seq, epoch := s.Position(); seq != 0 || epoch != 0 || countRows(t, s, "meta") != 0 || countRows(t, s, "journal") != 0 {
		t.Fatalf("a refused payload was written: position %d/%d", seq, epoch)
	}
	// Text JSON only escapes (<, >, &, U+2028) comes back as it was and is written.
	follower := openTestStore(t)
	if _, _, err := s.StartEpoch("a<&> ", "http://cortex_a:8100/?a=1&b=<2>", 0, t0); err != nil {
		t.Fatalf("StartEpoch with escaped text failed: %v", err)
	}
	follow(t, s, follower)
	sameDump(t, "after escaped text", dump(t, follower), dump(t, s))
}

// Review 1 (gc-vs-apply-dangling-blob): GCBlobs read the references, the follower saw the blob
// present and applied an entry naming it, and GC removed it afterwards. Apply now renews the
// blob under blobMu before it commits (GC looks at the time again under blobMu), or returns
// ErrBlobMissing when GC was first; either way the index never names a blob that is gone.
//
// staleBlobAboutToBeNamed sets the scene: the follower has the blob of url, old and no longer
// referenced (the entry was purged), and the leader's next journal entry names it again (the
// URL was fetched again with the same content, which renewed the blob on the leader only).
func staleBlobAboutToBeNamed(t *testing.T) (leader, follower *Store, key string) {
	t.Helper()
	leader = openTestStore(t)
	follower = openTestStore(t)
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, t0); err != nil {
		t.Fatal(err)
	}
	const url, body = "https://www.b-tu.de/modul/11101", "<html>module 11101</html>"
	f := fetched(t, leader, url, 200, body, t0)
	record(t, leader, f)
	if err := copyBlob(leader, follower, f.Hash); err != nil {
		t.Fatal(err)
	}
	follow(t, leader, follower)
	ageBlob(t, follower, f.Hash, 30*24*time.Hour) // imported a month ago
	if _, err := leader.DeleteEntry(f.Key); err != nil {
		t.Fatal(err)
	}
	follow(t, leader, follower)
	record(t, leader, fetched(t, leader, url, 200, body, t0.Add(time.Minute)))
	return leader, follower, f.Key
}

// noDanglingBlob fails unless the follower has the blob its entry for key names and is the
// leader's copy.
func noDanglingBlob(t *testing.T, leader, follower *Store, key string) {
	t.Helper()
	_, v, err := follower.Lookup(key)
	if err != nil {
		t.Fatal(err)
	}
	if !follower.HasBlob(v.Hash) {
		t.Fatalf("the follower's index names blob %s, which GC removed", v.Hash)
	}
	sameDump(t, "after GC and Apply", dump(t, follower), dump(t, leader))
}

// GC first: it holds the references and waits for blobMu to remove the blob (the test holds
// blobMu); the follower applies meanwhile, and its Apply waits for blobMu too.
func TestGCThatGoesFirstMakesAFollowerFetchTheBlobAgain(t *testing.T) {
	leader, follower, key := staleBlobAboutToBeNamed(t)
	follower.blobMu.Lock()
	gcDone := make(chan error, 1)
	var removed int
	go func() {
		var err error
		removed, _, err = follower.GCBlobs(7*24*time.Hour, time.Now())
		gcDone <- err
	}()
	waitForStack(t, "removeIfStale")
	applied := make(chan error, 1)
	go func() { applied <- catchUp(leader, follower) }()
	time.Sleep(10 * time.Millisecond) // Apply gets as far as it can
	follower.blobMu.Unlock()
	if err := <-gcDone; err != nil {
		t.Fatalf("GCBlobs failed: %v", err)
	}
	if err := <-applied; err != nil {
		t.Fatalf("the follower loop failed: %v", err)
	}
	if removed != 1 {
		t.Logf("Apply was first after all (GC removed %d)", removed)
	}
	noDanglingBlob(t, leader, follower, key)
}

// Apply first: GC has read the references and pauses before its first removal (in the sync of
// the index); the follower applies the entry then, which renews the blob, and GC leaves it.
func TestGCLeavesABlobAFollowerReferencedWhileItLooked(t *testing.T) {
	leader, follower, key := staleBlobAboutToBeNamed(t)
	paused, resume := make(chan struct{}), make(chan struct{})
	saved := syncFile
	t.Cleanup(func() { syncFile = saved })
	var once sync.Once
	syncFile = func(f *os.File) error {
		if filepath.Base(f.Name()) == "index.db-wal" {
			once.Do(func() {
				close(paused)
				<-resume
			})
		}
		return saved(f)
	}
	gcDone := make(chan error, 1)
	var removed int
	go func() {
		var err error
		removed, _, err = follower.GCBlobs(7*24*time.Hour, time.Now())
		gcDone <- err
	}()
	select {
	case <-paused:
	case err := <-gcDone:
		t.Fatalf("GCBlobs (err %v) removed %d blobs without syncing the index first", err, removed)
	}
	err := catchUp(leader, follower)
	close(resume)
	if err != nil {
		t.Fatalf("the follower loop failed: %v", err)
	}
	if err := <-gcDone; err != nil {
		t.Fatalf("GCBlobs failed: %v", err)
	}
	if removed != 0 {
		t.Errorf("GCBlobs removed %d blobs, want none: the follower referenced the blob again", removed)
	}
	noDanglingBlob(t, leader, follower, key)
}

// waitForStack waits until some goroutine is in the function name.
func waitForStack(t *testing.T, name string) {
	t.Helper()
	buf := make([]byte, 1<<20)
	for deadline := time.Now().Add(10 * time.Second); ; {
		if n := runtime.Stack(buf, true); strings.Contains(string(buf[:n]), name) {
			return
		}
		if time.Now().After(deadline) {
			t.Fatalf("no goroutine reached %s", name)
		}
		time.Sleep(time.Millisecond)
	}
}

// Review 1 (trimjournal-hole): journal.at is the caller's time and need not grow with seq, so
// trimming by time left [2 4], a follower at 1 was served 2 and 4, and Apply(4) failed out of
// order instead of the 410 that leads to a snapshot. Only a prefix is trimmed now: up to the
// newest entry older than the cutoff (review 2 changed the end of the prefix, see
// TestTrimJournalIsNotStoppedByAnEntryFromTheFuture), so the young entry 2 before the old 3
// goes with it, never 3 alone.
func TestTrimJournalTrimsOnlyAPrefix(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, t0); err != nil { // seq 1
		t.Fatal(err)
	}
	// A fetch that began later and finished first, then the one that began earlier.
	record(t, leader, fetched(t, leader, "https://www.b-tu.de/a", 200, "a", t0.Add(20*time.Minute))) // seq 2
	record(t, leader, fetched(t, leader, "https://www.b-tu.de/b", 200, "b", t0.Add(10*time.Minute))) // seq 3
	record(t, leader, fetched(t, leader, "https://www.b-tu.de/c", 200, "c", t0.Add(30*time.Minute))) // seq 4
	record(t, leader, fetched(t, leader, "https://www.b-tu.de/d", 200, "d", t0.Add(40*time.Minute))) // seq 5
	follow(t, leader, follower)
	n, err := leader.TrimJournal(t0.Add(15 * time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	all, err := leader.JournalAfter(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	var seqs []int64
	for _, e := range all {
		seqs = append(seqs, e.Seq)
	}
	if n != 3 || fmt.Sprint(seqs) != "[4 5]" || leader.OldestSeq() != 4 {
		t.Fatalf("TrimJournal removed %d, the journal is %v (oldest %d), want 3 removed and [4 5]", n, seqs, leader.OldestSeq())
	}
	// The follower trims its own journal, of the same entries, the same way.
	if _, err := follower.TrimJournal(t0.Add(15 * time.Minute)); err != nil {
		t.Fatal(err)
	}
	sameDump(t, "after the trim", dump(t, follower), dump(t, leader))
}

// Review 1 (journalafter-unbounded-bytes): a page of the journal was up to 1000 entries of any
// size, held in memory at once. It stops at about 8 MiB of payload now.
func TestJournalAfterStopsAtAboutEightMiB(t *testing.T) {
	leader := openTestStore(t)
	// Five kept headers of 8 KiB each: JSON writes '<' as six bytes, the payload as seven, so
	// one entry is 280 KiB and forty are 11 MiB.
	big := strings.Repeat("<", 8<<10)
	header := http.Header{"Content-Type": {big}, "Content-Language": {big}, "Content-Disposition": {big}, "Etag": {big}, "Last-Modified": {big}}
	const n = 40
	for i := range n {
		f := fetched(t, leader, fmt.Sprint("https://www.b-tu.de/modul/", i), 200, fmt.Sprint(i), t0)
		f.Header = header
		record(t, leader, f)
	}
	entries, err := leader.JournalAfter(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	total := 0
	for _, e := range entries {
		total += len(e.Payload)
	}
	if len(entries) < 20 || len(entries) >= n || total > 8<<20 {
		t.Fatalf("JournalAfter returned %d entries of %d bytes, want fewer than %d and at most 8 MiB", len(entries), total, n)
	}
	if len(entries[0].Payload) < 250<<10 {
		t.Fatalf("an entry is only %d bytes; the test does not test the limit", len(entries[0].Payload))
	}
	follower := openTestStore(t)
	follow(t, leader, follower)
	sameDump(t, "after paging through big entries", dump(t, follower), dump(t, leader))
}

func TestJournalAfterReturnsAnEntryLargerThanThePageAlone(t *testing.T) {
	saved := journalPageBytes
	journalPageBytes = 1
	t.Cleanup(func() { journalPageBytes = saved })
	s := openTestStore(t)
	for i := range 3 {
		record(t, s, fetched(t, s, fmt.Sprint("https://www.b-tu.de/modul/", i), 200, fmt.Sprint(i), t0))
	}
	for after := int64(0); after < 3; after++ {
		entries, err := s.JournalAfter(after, 0)
		if err != nil || len(entries) != 1 || entries[0].Seq != after+1 {
			t.Fatalf("JournalAfter(%d) = %d entries (err %v), want only %d", after, len(entries), err, after+1)
		}
	}
}
