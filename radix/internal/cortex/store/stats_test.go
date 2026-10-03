package store

import (
	"bytes"
	"errors"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/telemetry"
)

func TestStatsCountRowsAndBlobs(t *testing.T) {
	s := openTestStore(t)
	text := strings.Repeat("Vorlesungsverzeichnis ", 1000)
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, text, t0))
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "short", t0.Add(time.Hour)))
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/2", 200, "short", t0))
	putFile(t, s, "a", "file a", "text/plain", t0)
	putFile(t, s, "b", "file b", "text/plain", t0)
	if _, err := s.DeleteFile("b", t0.Add(time.Hour)); err != nil {
		t.Fatal(err)
	}
	orphan := putBlob(t, s, strings.Repeat("orphan ", 500)) // compressed, no row: gzip's trailer

	st, err := s.Stats()
	if err != nil {
		t.Fatalf("Stats failed: %v", err)
	}
	textBlob, _ := s.StatBlob(sha([]byte(text)))
	wantStored := textBlob.Stored + int64(len("short")+len("file a")+len("file b")) + orphan.Stored
	wantOriginal := int64(len(text)+len("short")+len("file a")+len("file b")) + orphan.Size
	want := Stats{Entries: 2, Versions: 3, Files: 1, FileVersions: 3, Blobs: 5, BlobBytes: wantStored, BlobOriginalBytes: wantOriginal}
	if st != want {
		t.Fatalf("Stats = %+v, want %+v", st, want)
	}

	// RecentStats and the gauges read the same, counted once a minute (RefreshStats: now).
	if _, err := s.RefreshStats(); err != nil {
		t.Fatal(err)
	}
	recent, err := s.RecentStats()
	if err != nil || recent != want {
		t.Fatalf("RecentStats = %+v (err %v)", recent, err)
	}
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/3", 200, "new", t0))
	if recent, _ = s.RecentStats(); recent.Entries != 2 {
		t.Fatalf("RecentStats counted again within a minute: %+v", recent)
	}
	var out bytes.Buffer
	if err := telemetry.Registry.WriteText(&out); err != nil {
		t.Fatal(err)
	}
	for _, line := range []string{"cortex_entries 2\n", "cortex_versions 3\n", "cortex_files 1\n", "cortex_blobs 5\n",
		`cortex_pruned_total{what="versions"}`, `cortex_pruned_total{what="blobs"}`} {
		if !strings.Contains(out.String(), line) {
			t.Errorf("/metrics lacks %q", line)
		}
	}
}

func TestReferencedBlobsNamesEveryReferenceOnce(t *testing.T) {
	s := openTestStore(t)
	defer func(n int) { referencedPage = n }(referencedPage)
	referencedPage = 7
	var want []string
	for i := 0; i < 52; i++ { // several pages, the last one short
		b := putBlob(t, s, strings.Repeat("x", i))
		want = append(want, b.Hash)
		f := fetched(t, s, "https://www.b-tu.de/modul/"+strings.Repeat("1", 1+i%9), 200, strings.Repeat("x", i), t0.Add(time.Duration(i)*time.Second))
		record(t, s, f)
	}
	// A file with the same content as a version, and a tombstone (no blob).
	putFile(t, s, "same", "xx", "text/plain", t0)
	putFile(t, s, "gone", "x", "text/plain", t0)
	if _, err := s.DeleteFile("gone", t0.Add(time.Hour)); err != nil {
		t.Fatal(err)
	}
	sort.Strings(want)

	var got []string
	if err := s.ReferencedBlobs(func(hash string) error {
		got = append(got, hash)
		return nil
	}); err != nil {
		t.Fatalf("ReferencedBlobs failed: %v", err)
	}
	if len(got) != len(want) {
		t.Fatalf("ReferencedBlobs gave %d hashes, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("hash %d = %s, want %s", i, got[i], want[i])
		}
	}
	stop := errors.New("enough")
	n := 0
	if err := s.ReferencedBlobs(func(string) error { n++; return stop }); !errors.Is(err, stop) || n != 1 {
		t.Fatalf("ReferencedBlobs did not stop at fn's error: %v after %d", err, n)
	}
}

// E2E-5 / D2: RecentStats counted inside the call once its minute was over, holding the cache's
// lock through the walk over every blob: every /status and /metrics waited seconds for it in a
// large store. It returns the last numbers now and counts in the background.
func TestRecentStatsNeverWaitsForTheBlobWalk(t *testing.T) {
	s := openTestStore(t)
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/1", 200, "one", t0))
	if _, err := s.RefreshStats(); err != nil {
		t.Fatal(err)
	}
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/2", 200, "two", t0))

	release := make(chan struct{})
	walking := make(chan struct{}, 1)
	saved := beforeBlobWalk
	t.Cleanup(func() { beforeBlobWalk = saved })
	beforeBlobWalk = func() {
		select {
		case walking <- struct{}{}:
		default:
		}
		<-release
	}
	s.InvalidateStats()
	for i := 0; i < 3; i++ { // the first starts the count, the others find it running
		done := make(chan Stats, 1)
		go func() {
			st, _ := s.RecentStats()
			done <- st
		}()
		select {
		case st := <-done:
			if st.Entries != 1 {
				t.Fatalf("RecentStats while counting = %+v, want the last numbers (1 entry)", st)
			}
		case <-time.After(5 * time.Second):
			close(release)
			t.Fatal("RecentStats waited for the blob walk")
		}
	}
	select {
	case <-walking:
	case <-time.After(5 * time.Second):
		t.Fatal("RecentStats did not start a count")
	}
	close(release)
	deadline := time.Now().Add(5 * time.Second)
	for {
		if st, _ := s.RecentStats(); st.Entries == 2 {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("the background count never arrived")
		}
		time.Sleep(10 * time.Millisecond)
	}

	// A fresh store has no numbers until its first count is done: ErrStatsPending, at once.
	fresh := openTestStore(t)
	beforeBlobWalk = func() { time.Sleep(200 * time.Millisecond) }
	start := time.Now()
	if _, err := fresh.RecentStats(); !errors.Is(err, ErrStatsPending) || time.Since(start) > 100*time.Millisecond {
		t.Fatalf("RecentStats of a fresh store: %v after %s, want ErrStatsPending at once", err, time.Since(start))
	}
	for deadline := time.Now().Add(5 * time.Second); ; time.Sleep(10 * time.Millisecond) { // the count ends before the hook goes
		if _, err := fresh.RecentStats(); err == nil {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("the first count of a fresh store never arrived")
		}
	}
}
