package server

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/cluster"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

// journal reads GET /internal/v1/journal?query.
func journal(t *testing.T, c *testCortex, query string) (*http.Response, []store.JournalEntry, string) {
	t.Helper()
	resp, body := get(t, c.URL+"/internal/v1/journal?"+query)
	var entries []store.JournalEntry
	if resp.StatusCode == http.StatusOK {
		sc := bufio.NewScanner(strings.NewReader(body))
		sc.Buffer(nil, 16<<20)
		for sc.Scan() {
			var e store.JournalEntry
			if err := json.Unmarshal(sc.Bytes(), &e); err != nil {
				t.Fatalf("journal line %q: %v", sc.Text(), err)
			}
			entries = append(entries, e)
		}
	}
	return resp, entries, body
}

func TestTheJournalFeedsAFollowerThatEndsUpIdentical(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "a page"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))
	putFile(t, c, "notes.txt", "a file")

	resp, entries, body := journal(t, c, "after=0")
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Content-Type") != "application/x-ndjson" {
		t.Fatalf("journal: %d %s", resp.StatusCode, body)
	}
	seq, epoch, at := c.st.Head()
	if len(entries) != 3 || entries[0].Op != store.OpEpoch || entries[1].Op != store.OpVersion || entries[1].Blob != sha("a page") ||
		entries[2].Op != store.OpFileVersion || resp.Header.Get("Cortex-Head-Seq") != strconv.FormatInt(seq, 10) ||
		resp.Header.Get("Cortex-Head-At") != store.FormatTime(at) {
		t.Fatalf("entries %+v, head %v", entries, resp.Header)
	}

	// A follower that applies them (with the blobs) has the same index.
	follower, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer follower.Close()
	for _, e := range entries {
		if e.Blob != "" {
			resp, data := get(t, c.URL+"/v1/blobs/sha256:"+e.Blob, "Accept-Encoding", "gzip")
			if err := follower.ImportStored(e.Blob, strings.NewReader(data), resp.Header.Get("Content-Encoding") == "gzip"); err != nil {
				t.Fatal(err)
			}
		}
		if err := follower.Apply(e); err != nil {
			t.Fatalf("Apply %d: %v", e.Seq, err)
		}
	}
	if fseq, fepoch := follower.Position(); fseq != seq || fepoch != epoch {
		t.Errorf("follower at %d/%d, leader %d/%d", fseq, fepoch, seq, epoch)
	}
	if fv, err := follower.GetFile("notes.txt"); err != nil || fv.Hash != sha("a file") {
		t.Errorf("the follower's file: %+v %v", fv, err)
	}

	// From the middle, with the epoch of the entry it has.
	if resp, entries, _ := journal(t, c, "after=2&epoch=1"); resp.StatusCode != http.StatusOK || len(entries) != 1 || entries[0].Seq != 3 {
		t.Errorf("after=2: %d %+v", resp.StatusCode, entries)
	}
}

func TestTheJournalWaitsForTheNextEntryAndWakes(t *testing.T) {
	c := newTestCortex(t)
	seq, epoch := c.st.Position()

	start := time.Now()
	resp, entries, _ := journal(t, c, "after="+strconv.FormatInt(seq, 10)+"&wait=300ms")
	if resp.StatusCode != http.StatusOK || len(entries) != 0 || time.Since(start) < 250*time.Millisecond {
		t.Errorf("nothing new: %d %d entries after %s", resp.StatusCode, len(entries), time.Since(start))
	}

	go func() {
		time.Sleep(200 * time.Millisecond)
		do(t, http.MethodPut, c.URL+"/v1/files/woken", strings.NewReader("x"))
	}()
	start = time.Now()
	resp, entries, _ = journal(t, c, "after="+strconv.FormatInt(seq, 10)+"&epoch="+strconv.FormatInt(epoch, 10)+"&wait=20s")
	if took := time.Since(start); resp.StatusCode != http.StatusOK || len(entries) != 1 || entries[0].Seq != seq+1 || took > 5*time.Second {
		t.Errorf("woken: %d %+v after %s", resp.StatusCode, entries, took)
	}
}

func TestTheJournalTellsADivergedOrTrimmedFollowerToTakeASnapshot(t *testing.T) {
	c := newTestCortex(t)
	for _, name := range []string{"a", "b", "c"} {
		putFile(t, c, name, name)
	}
	seq, _ := c.st.Position() // 4: the epoch and three files

	for _, tc := range []struct {
		query  string
		status int
		code   string
	}{
		{"after=9&epoch=1", http.StatusConflict, codeDiverged}, // the follower is ahead
		{"after=2&epoch=7", http.StatusConflict, codeDiverged}, // another epoch at 2
		{"after=x", http.StatusBadRequest, codeBadRequest},     // not a number
		{"after=1&wait=soon", http.StatusBadRequest, codeBadRequest},
	} {
		resp, _, body := journal(t, c, tc.query)
		wantStatus(t, resp, body, tc.status, tc.code)
	}

	if _, err := c.st.TrimJournal(time.Now().Add(time.Hour)); err != nil { // keeps only the newest
		t.Fatal(err)
	}
	resp, _, body := journal(t, c, "after=0")
	wantStatus(t, resp, body, http.StatusGone, codeTrimmed)
	resp, _, body = journal(t, c, "after=2&epoch=1")
	wantStatus(t, resp, body, http.StatusGone, codeTrimmed)
	// The entry just before the oldest kept one cannot be checked, but nothing is missing after it.
	if resp, entries, _ := journal(t, c, "after="+strconv.FormatInt(seq-1, 10)+"&epoch=1"); resp.StatusCode != http.StatusOK || len(entries) != 1 {
		t.Errorf("after the oldest-1: %d %+v", resp.StatusCode, entries)
	}
}

func TestTheSnapshotIsTheLeadersIndex(t *testing.T) {
	c := newTestCortex(t)
	putFile(t, c, "kept.txt", "kept")
	resp, body := get(t, c.URL+"/internal/v1/snapshot")
	seq, epoch := c.st.Position()
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Cortex-Seq") != strconv.FormatInt(seq, 10) ||
		resp.Header.Get("Cortex-Epoch") != strconv.FormatInt(epoch, 10) || !strings.HasPrefix(body, "SQLite format 3\x00") ||
		resp.Header.Get("Content-Length") != strconv.Itoa(len(body)) {
		t.Fatalf("snapshot: %d %v", resp.StatusCode, resp.Header)
	}

	follower, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer follower.Close()
	if err := follower.ReplaceIndex(bytes.NewReader([]byte(body))); err != nil {
		t.Fatal(err)
	}
	if fseq, fepoch := follower.Position(); fseq != seq || fepoch != epoch {
		t.Errorf("follower at %d/%d, want %d/%d", fseq, fepoch, seq, epoch)
	}
	if fv, err := follower.GetFile("kept.txt"); err != nil || fv.Hash != sha("kept") {
		t.Errorf("the file in the snapshot: %+v %v", fv, err)
	}
}

func TestOnlyTheLeaderServesTheReplication(t *testing.T) {
	c := newTestCortex(t, withNode(func(*store.Store) cluster.Node {
		return newFollowerNode("b", &cluster.Info{Instance: "a", URL: "http://127.0.0.1:1"})
	}))
	for _, path := range []string{"/internal/v1/journal?after=0", "/internal/v1/snapshot"} {
		resp, body := get(t, c.URL+path)
		wantStatus(t, resp, body, http.StatusConflict, codeNotLeader)
	}
}

// E2E-2 / D1: the leader read its entries from the database and its head from the cached
// position, which a commit moves only after the entry is visible. Under concurrent writes a
// current follower got a page past Cortex-Head-Seq, asked again from its end and was refused
// as diverged ("the follower is at N, the leader's journal ends at N-1"), then copied the whole
// index, again and again. A follower that follows the journal is never refused now, and no
// page goes past the head it names (E2E-6: the head it names is the one the page was read up
// to, so a follower at Cortex-Head-Seq is current).
func TestACurrentFollowerIsNeverRefusedWhileTheLeaderWrites(t *testing.T) {
	c := newTestCortex(t)
	var stop atomic.Bool
	var wg sync.WaitGroup
	for w := range 3 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := 0; !stop.Load(); n++ {
				b, err := c.st.PutBlob(strings.NewReader(fmt.Sprintf("w%d-%d", w, n)), "", 0)
				if err == nil {
					_, _, _, err = c.st.PutFile(fmt.Sprintf("load/w%d/%08d", w, n), b, "text/plain", time.Now())
				}
				if err != nil {
					t.Error(err)
					return
				}
			}
		}()
	}
	defer func() {
		stop.Store(true)
		wg.Wait()
	}()

	var after, epoch int64
	var sum string
	answers := 0
	for deadline := time.Now().Add(2 * time.Second); time.Now().Before(deadline); answers++ {
		q := url.Values{"after": {strconv.FormatInt(after, 10)}, "wait": {"1s"}}
		if after > 0 {
			q.Set("epoch", strconv.FormatInt(epoch, 10))
			q.Set("sum", sum)
		}
		resp, entries, body := journal(t, c, q.Encode())
		if resp.StatusCode != http.StatusOK {
			t.Fatalf("after %d answers, the follower at %d was refused: %d %s", answers, after, resp.StatusCode, body)
		}
		head, err := strconv.ParseInt(resp.Header.Get("Cortex-Head-Seq"), 10, 64)
		if err != nil || resp.Header.Get("Cortex-Head-Epoch") == "" {
			t.Fatalf("head %v", resp.Header)
		}
		for _, e := range entries {
			if e.Seq != after+1 {
				t.Fatalf("entry %d after %d", e.Seq, after)
			}
			after, epoch, sum = e.Seq, e.Epoch, e.Sum()
		}
		if after > head {
			t.Fatalf("the page ends at %d, past the head %d it names", after, head)
		}
		if len(entries) < 1000 && after != head {
			t.Fatalf("a page that was not cut short ends at %d, before the head %d it names", after, head)
		}
	}
	if after < 50 {
		t.Fatalf("the writers wrote only %d entries in %d answers", after, answers)
	}
}

// Review 2 (journal-boundary-skips-epoch-check, journal-oldest-minus-one-unchecked,
// epoch-reuse-hides-divergence): a follower at exactly OldestSeq-1 was served without any
// check, and elsewhere only the epoch of its entry was compared. The leader keeps the trimmed
// entry's (seq, epoch, sum) and compares the follower's sum when it sends one.
func TestTheJournalChecksTheFollowersEntryAlsoAtTheTrimBoundary(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "a page"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))   // 2
	putFile(t, c, "notes.txt", "a file") // 3
	two, err := c.st.JournalEntryAt(2)
	if err != nil {
		t.Fatal(err)
	}
	other := two
	other.Payload = []byte(`{"another":"entry"}`)

	for _, tc := range []struct {
		query  string
		status int
		code   string
	}{
		{"after=2&epoch=1&sum=" + two.Sum(), http.StatusOK, ""},
		{"after=2&epoch=1&sum=" + other.Sum(), http.StatusConflict, codeDiverged}, // the same seq and epoch, another entry
		{"after=2&sum=" + other.Sum(), http.StatusConflict, codeDiverged},
		{"after=2&epoch=1&sum=nothex", http.StatusBadRequest, codeBadRequest},
	} {
		resp, _, body := journal(t, c, tc.query)
		wantStatus(t, resp, body, tc.status, tc.code)
	}

	if _, err := c.st.TrimJournal(time.Now().Add(1000 * time.Hour)); err != nil { // keeps only 3
		t.Fatal(err)
	}
	for _, tc := range []struct {
		query  string
		status int
		code   string
	}{
		{"after=2&epoch=999", http.StatusConflict, codeDiverged}, // the reviewers' reproducer: served before
		{"after=2&epoch=1&sum=" + other.Sum(), http.StatusConflict, codeDiverged},
		{"after=2&epoch=1&sum=" + two.Sum(), http.StatusOK, ""},
		{"after=2&epoch=1", http.StatusOK, ""},
		{"after=1&epoch=1", http.StatusGone, codeTrimmed},
	} {
		resp, entries, body := journal(t, c, tc.query)
		wantStatus(t, resp, body, tc.status, tc.code)
		if tc.status == http.StatusOK && (len(entries) != 1 || entries[0].Seq != 3) {
			t.Errorf("%s: %+v", tc.query, entries)
		}
	}
}

// Review 2 (snapshot-unbounded-uncancelled): HEAD made a whole copy to send no body, and every
// GET two. HEAD answers the position now; a GET sends its one copy and leaves nothing behind.
func TestTheSnapshotLeavesNoCopyBehind(t *testing.T) {
	c := newTestCortex(t)
	putFile(t, c, "kept.txt", "kept")
	seq, epoch := c.st.Position()
	resp, body := do(t, http.MethodHead, c.URL+"/internal/v1/snapshot", nil)
	if resp.StatusCode != http.StatusOK || body != "" || resp.Header.Get("Cortex-Seq") != strconv.FormatInt(seq, 10) ||
		resp.Header.Get("Cortex-Epoch") != strconv.FormatInt(epoch, 10) {
		t.Fatalf("HEAD: %d %v", resp.StatusCode, resp.Header)
	}
	var wg sync.WaitGroup
	for range 3 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			resp, err := http.Get(c.URL + "/internal/v1/snapshot")
			if err != nil {
				t.Error(err)
				return
			}
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			if resp.StatusCode != http.StatusOK || !strings.HasPrefix(string(data), "SQLite format 3\x00") {
				t.Errorf("GET: %d, %d bytes", resp.StatusCode, len(data))
			}
		}()
	}
	wg.Wait()
	waitFor(t, "an empty DIR/tmp", func() bool {
		files, _ := os.ReadDir(filepath.Join(c.st.Dir(), "tmp"))
		return len(files) == 0
	})
}
