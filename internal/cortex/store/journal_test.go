package store

import (
	"encoding/json"
	"errors"
	"fmt"
	"math/rand/v2"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// E2E-2 / D1: commit made an entry visible (tx.Commit) before it moved the cached position, so
// JournalAfter returned entries beyond Head(). The leader answered with a head below the page
// it sent, and the follower that applied the page was refused as "ahead of the leader"
// (409 diverged) and copied the whole index, over and over under light concurrent writes.
func TestTheJournalNeverGoesPastTheHeadItIsReadWith(t *testing.T) {
	s := openTestStore(t)
	if _, _, err := s.StartEpoch("a", "http://a:8100", 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	var stop atomic.Bool
	var wg sync.WaitGroup
	for w := range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := 0; !stop.Load(); n++ {
				b, err := s.PutBlob(strings.NewReader(fmt.Sprintf("w%d-%d", w, n)), "", 0)
				if err == nil {
					_, _, _, err = s.PutFile(fmt.Sprintf("load/w%d/%08d", w, n), b, "application/octet-stream", time.Now())
				}
				if err != nil {
					t.Error(err)
					return
				}
			}
		}()
	}
	// The leader's side of a follower that is current: the page after its position, read up to
	// the head named with it; the follower then stands at the page's end.
	after, _ := s.Position()
	var reads, beyond, refused int
	for deadline := time.Now().Add(2 * time.Second); time.Now().Before(deadline); {
		head, _, _ := s.Head()
		if after > head {
			// What the handler does then: the database decides, and the head is the committed one.
			if c, why, err := s.CheckJournal(after, -1, ""); err != nil || c != Continues {
				refused++
				t.Logf("refused at %d: %s %v", after, why, err)
			}
			head, _, _ = s.CommittedHead()
			if after > head {
				refused++
			}
		}
		entries, err := s.JournalBetween(after, head, 1000)
		if err != nil {
			t.Fatal(err)
		}
		reads++
		if n := len(entries); n > 0 {
			if entries[n-1].Seq > head {
				beyond++
			}
			after = entries[n-1].Seq
		}
		// JournalAfter is the same, up to the head when it is called.
		page, err := s.JournalAfter(after-1, 1000)
		if err != nil {
			t.Fatal(err)
		}
		if h, _, _ := s.Head(); len(page) > 0 && page[len(page)-1].Seq > h {
			beyond++
		}
	}
	stop.Store(true)
	wg.Wait()
	if beyond > 0 || refused > 0 {
		t.Fatalf("in %d reads: %d pages went past the head, %d follower positions were refused", reads, beyond, refused)
	}
	if after < 50 {
		t.Fatalf("the writers wrote only %d entries", after)
	}
}

// Review 2 (epoch-reuse-hides-divergence, journal-boundary-skips-epoch-check,
// journal-oldest-minus-one-unchecked): the leader compared only the epoch of the follower's
// entry, and nothing at all at OldestSeq-1. Two leaders that started the same epoch number on
// the same seq (leader.json lost) wrote other entries the check could not tell apart, and a
// follower exactly at the trim boundary was served whatever its entry was.
func TestCheckJournalComparesTheEntryItselfAlsoAtTheTrimBoundary(t *testing.T) {
	a := openTestStore(t)
	b := openTestStore(t)
	if _, _, err := a.StartEpoch("a", "http://a:8100", 0, t0); err != nil { // 1, epoch 1
		t.Fatal(err)
	}
	record(t, a, fetched(t, a, "https://www.b-tu.de/modul/1", 200, "one", t0)) // 2
	follow(t, a, b)
	// Both start epoch 2 at seq 3, as two leaders without leader.json would, and write.
	if _, _, err := a.StartEpoch("a", "http://a:8100", 0, t0.Add(time.Minute)); err != nil {
		t.Fatal(err)
	}
	if _, _, err := b.StartEpoch("b", "http://b:8100", 0, t0.Add(time.Minute)); err != nil {
		t.Fatal(err)
	}
	record(t, a, fetched(t, a, "https://www.b-tu.de/modul/a", 200, "a's", t0.Add(2*time.Minute))) // 4
	record(t, b, fetched(t, b, "https://www.b-tu.de/modul/b", 200, "b's", t0.Add(2*time.Minute))) // 4
	record(t, a, fetched(t, a, "https://www.b-tu.de/modul/a2", 200, "more", t0.Add(3*time.Minute)))
	record(t, a, fetched(t, a, "https://www.b-tu.de/modul/a3", 200, "even more", t0.Add(4*time.Minute)))

	bseq, bepoch, bsum, err := b.LastSum()
	if err != nil || bseq != 4 || bepoch != 2 {
		t.Fatalf("b's LastSum = %d/%d (err %v), want 4/2", bseq, bepoch, err)
	}
	aseq, aepoch, asum, _ := func() (int64, int64, string, error) {
		e, err := a.JournalEntryAt(4)
		return e.Seq, e.Epoch, e.Sum(), err
	}()
	if aseq != 4 || aepoch != 2 || asum == bsum {
		t.Fatalf("a's entry 4: %d/%d sum %s, b's sum %s: the sums must differ", aseq, aepoch, asum, bsum)
	}
	check := func(what string, seq, epoch int64, sum string, want Continuation) {
		t.Helper()
		got, why, err := a.CheckJournal(seq, epoch, sum)
		if err != nil || got != want {
			t.Errorf("%s: CheckJournal(%d, %d, %.8s) = %d %q (err %v), want %d", what, seq, epoch, sum, got, why, err, want)
		}
	}
	check("the same seq and epoch, another entry", bseq, bepoch, bsum, Diverged)
	check("epoch only (an older follower)", bseq, bepoch, "", Continues) // what it cannot tell
	check("the leader's own entry", aseq, aepoch, asum, Continues)
	check("another epoch", aseq, 1, "", Diverged)
	check("ahead", 9, 2, "", Diverged)
	check("an empty follower", 0, -1, "", Continues)

	// Trimmed to [6]: 5 is gone from the journal, but its seq, epoch and sum are kept.
	five, err := a.JournalEntryAt(5)
	if err != nil {
		t.Fatal(err)
	}
	if n, err := a.TrimJournal(t0.Add(time.Hour)); err != nil || n != 5 || a.OldestSeq() != 6 {
		t.Fatalf("TrimJournal = %d (err %v), oldest %d", n, err, a.OldestSeq())
	}
	check("the trimmed boundary, the same entry", 5, five.Epoch, five.Sum(), Continues)
	check("the trimmed boundary, epoch only", 5, five.Epoch, "", Continues)
	check("the trimmed boundary, another epoch", 5, 7, "", Diverged)
	check("the trimmed boundary, another entry", 5, five.Epoch, bsum, Diverged)
	check("before the boundary", 4, 2, asum, Trimmed)
	check("an empty follower of a trimmed journal", 0, -1, "", Trimmed)
}

// The sum is the same on the leader, after the NDJSON stream, and on the follower that stored it.
func TestTheSumOfAnEntryIsTheSameOnBothSides(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	if _, _, err := leader.StartEpoch("a", "http://a:8100", 0, t0); err != nil {
		t.Fatal(err)
	}
	f := fetched(t, leader, "https://www.b-tu.de/modul/<&>", 200, "Übung <b>", t0)
	f.Header.Set("Content-Disposition", `attachment; filename="a&b<c>.pdf"`)
	record(t, leader, f)
	follow(t, leader, follower)
	for seq := int64(1); seq <= 2; seq++ {
		le, err := leader.JournalEntryAt(seq)
		if err != nil {
			t.Fatal(err)
		}
		line, _ := json.Marshal(le)
		var wire JournalEntry
		if err := json.Unmarshal(line, &wire); err != nil {
			t.Fatal(err)
		}
		fe, err := follower.JournalEntryAt(seq)
		if err != nil {
			t.Fatal(err)
		}
		if le.Sum() != wire.Sum() || le.Sum() != fe.Sum() || !ValidHash(le.Sum()) {
			t.Fatalf("entry %d: sum %s on the leader, %s on the wire, %s on the follower", seq, le.Sum(), wire.Sum(), fe.Sum())
		}
	}
}

// Review 2 (trimjournal-stalls-behind-future-at): the prefix ended before the first entry at or
// after the cutoff, so one entry stamped while the clock was a year ahead stopped all trimming.
func TestTrimJournalIsNotStoppedByAnEntryFromTheFuture(t *testing.T) {
	s := openTestStore(t)
	record(t, s, fetched(t, s, "https://www.b-tu.de/x", 200, "skew", t0.Add(365*24*time.Hour))) // the clock was a year ahead
	for i := 0; i < 50; i++ {
		record(t, s, fetched(t, s, fmt.Sprintf("https://www.b-tu.de/m/%d", i), 200, fmt.Sprint(i), t0.Add(time.Duration(i)*time.Hour)))
	}
	n, err := s.TrimJournal(t0.Add(30 * 24 * time.Hour)) // everything but the skewed entry is older
	if err != nil || n != 50 || s.OldestSeq() != 51 {
		t.Fatalf("TrimJournal = %d (err %v), oldest %d; want 50 removed, the newest kept", n, err, s.OldestSeq())
	}
}

// E2E-3: a follower applied one entry per transaction, barely faster than the leader wrote.
// ApplyBatch applies a page in one transaction, with Apply's errors, and writes nothing on one.
func TestApplyBatchIsApplyForAWholePage(t *testing.T) {
	leader := openTestStore(t)
	r := rand.New(rand.NewPCG(5, 6))
	now := t0
	if _, _, err := leader.StartEpoch("a", "http://cortex_a:8100", 0, now); err != nil {
		t.Fatal(err)
	}
	randomHistory(t, leader, r, 400, &now, nil)
	all, err := leader.JournalAfter(0, 0)
	if err != nil {
		t.Fatal(err)
	}

	// Without the blobs: refused, nothing written, the first missing blob named.
	follower := openTestStore(t)
	err = follower.ApplyBatch(all)
	var missing *BlobMissingError
	if !errors.As(err, &missing) || !errors.Is(err, ErrBlobMissing) || len(missing.Hashes) < 2 || missing.Hash != missing.Hashes[0] {
		t.Fatalf("ApplyBatch without the blobs: %v", err)
	}
	for _, e := range all {
		if e.Blob != "" {
			if e.Blob != missing.Hash || e.Seq != missing.Seq {
				t.Fatalf("the first missing blob is entry %d's %s, the error names %d's %s", e.Seq, e.Blob, missing.Seq, missing.Hash)
			}
			break
		}
	}
	if seq, _ := follower.Position(); seq != 0 || countRows(t, follower, "journal") != 0 || countRows(t, follower, "entry") != 0 {
		t.Fatalf("a refused batch wrote: seq %d", seq)
	}
	for _, hash := range missing.Hashes {
		if err := copyBlob(leader, follower, hash); err != nil {
			t.Fatal(err)
		}
	}

	// Out of order within the batch, and diverged in its middle: nothing written either.
	swapped := append([]JournalEntry{}, all[:5]...)
	swapped[3], swapped[4] = swapped[4], swapped[3]
	if err := follower.ApplyBatch(swapped); !errors.Is(err, ErrOutOfOrder) {
		t.Fatalf("ApplyBatch out of order: %v", err)
	}
	broken := append([]JournalEntry{}, all[:20]...)
	broken[12].Op, broken[12].Blob = OpEntryDelete, "" // an entry the follower has no row for
	broken[12].Payload = json.RawMessage(`{"id":999999,"key":"GET https://nowhere.invalid/"}`)
	if err := follower.ApplyBatch(broken); !errors.Is(err, ErrDiverged) {
		t.Fatalf("ApplyBatch of an entry that does not fit: %v, want ErrDiverged", err)
	}
	if seq, _ := follower.Position(); seq != 0 || countRows(t, follower, "journal") != 0 {
		t.Fatalf("a refused batch wrote: seq %d", seq)
	}

	// In pages, the follower ends up identical, as with Apply.
	for i := 0; i < len(all); i += 64 {
		if err := follower.ApplyBatch(all[i:min(i+64, len(all))]); err != nil {
			t.Fatalf("ApplyBatch(%d…) failed: %v", all[i].Seq, err)
		}
	}
	sameDump(t, "after the batches", dump(t, follower), dump(t, leader))
	if fs, _ := follower.Position(); fs != all[len(all)-1].Seq {
		t.Fatalf("follower at %d, want %d", fs, all[len(all)-1].Seq)
	}
}

// E2E-3: ImportStored synced the blob directory for every blob. It leaves that to the next
// commit now, which syncs the directory before any row that names the blob can be durable.
func TestImportedBlobsAreDurableBeforeTheRowsThatNameThem(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	var entries []JournalEntry
	for i := 0; i < 20; i++ {
		_, _, _, e := record(t, leader, fetched(t, leader, fmt.Sprint("https://www.b-tu.de/modul/", i), 200, fmt.Sprint("body ", i), t0))
		entries = append(entries, e)
	}
	var mu sync.Mutex
	var dirSyncs []string
	var committed bool // whether the follower had the batch when a directory was synced
	saved := syncFile
	t.Cleanup(func() { syncFile = saved })
	syncFile = func(f *os.File) error {
		if info, err := f.Stat(); err == nil && info.IsDir() {
			seq, _ := follower.Position()
			mu.Lock()
			dirSyncs = append(dirSyncs, filepath.Base(f.Name()))
			committed = committed || seq > 0
			mu.Unlock()
		}
		return saved(f)
	}
	dirs := map[string]bool{}
	for _, e := range entries {
		if err := copyBlob(leader, follower, e.Blob); err != nil {
			t.Fatal(err)
		}
		dirs[e.Blob[:2]] = true
	}
	mu.Lock()
	imported := len(dirSyncs)
	mu.Unlock()
	// Only a new <hh> directory is synced into blobs/sha256 at once (the root, "sha256").
	for _, d := range dirSyncs {
		if d != "sha256" {
			t.Fatalf("ImportStored synced the blob directory %s; that is the commit's", d)
		}
	}
	if err := follower.ApplyBatch(entries); err != nil {
		t.Fatal(err)
	}
	mu.Lock()
	defer mu.Unlock()
	synced := map[string]bool{}
	for _, d := range dirSyncs[imported:] {
		synced[d] = true
	}
	for d := range dirs {
		if !synced[d] {
			t.Fatalf("the commit did not sync blob directory %s (synced %v)", d, dirSyncs[imported:])
		}
	}
	if committed || len(dirSyncs[imported:]) != len(dirs) {
		t.Fatalf("directories synced after the commit (%v), or not once each: %v", committed, dirSyncs[imported:])
	}
}
