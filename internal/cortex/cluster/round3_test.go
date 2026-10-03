//go:build unix && !aix && !solaris && !hurd

package cluster_test

import (
	"bytes"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/internal/cortex/store"
)

// The regression tests of the third review round: real-load verification (E2E-*) and review
// 2's cluster findings. Each fails on the code before the round.

// lockedBuffer is a log that tests read while instances write it.
type lockedBuffer struct {
	mu  sync.Mutex
	buf bytes.Buffer
}

func (b *lockedBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.Write(p)
}

// count is how many lines of the log have "event":event.
func (b *lockedBuffer) count(event string) int {
	b.mu.Lock()
	defer b.mu.Unlock()
	return strings.Count(b.buf.String(), `"event":"`+event+`"`)
}

// lines returns the lines of the log with "event":event.
func (b *lockedBuffer) lines(event string) string {
	b.mu.Lock()
	defer b.mu.Unlock()
	var out []string
	for _, line := range strings.Split(b.buf.String(), "\n") {
		if strings.Contains(line, `"event":"`+event+`"`) {
			out = append(out, line)
		}
	}
	return strings.Join(out, "\n")
}

// captureLog sends the log to a buffer until the test ends.
func captureLog(t *testing.T) *lockedBuffer {
	t.Helper()
	out := &lockedBuffer{}
	previous := slog.Default()
	slog.SetDefault(slog.New(slog.NewJSONHandler(out, &slog.HandlerOptions{Level: slog.LevelDebug})))
	t.Cleanup(func() { slog.SetDefault(previous) })
	return out
}

// putDirect stores a file in st without HTTP: quicker, for tests that need many entries.
func putDirect(t *testing.T, st *store.Store, name, content string) {
	t.Helper()
	b, err := st.PutBlob(strings.NewReader(content), "", 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, _, err := st.PutFile(name, b, "text/plain", time.Now()); err != nil {
		t.Fatal(err)
	}
}

func hasFileWith(st *store.Store, name, content string) bool {
	fv, err := st.GetFile(name)
	return err == nil && fv.Hash == sha(content) && st.HasBlob(fv.Hash)
}

// E2E-1, review 2 handover-tail-lost-on-slow-blob: a hand-over, by step-down and by shutdown
// (SIGTERM), waits until the successor has copied the last write, also when its blob takes
// longer than the old bounds (2 s catch-up, 1 s at shutdown).
func TestAHandOverWaitsUntilTheSuccessorHasTheLastWrite(t *testing.T) {
	for _, how := range []string{"step-down", "shutdown"} {
		t.Run(how, func(t *testing.T) {
			a, b, _ := pair(t, nil, nil)
			b.gate.holdBlobs() // the last write's blob takes 2.5 s to copy (a large file)
			content := randomText(4096, 7)
			a.put(t, "big/model.bin", content)
			go func() {
				time.Sleep(2500 * time.Millisecond)
				b.gate.releaseBlobs()
			}()
			switch how {
			case "step-down":
				if err := a.peer.StepDown(); err != nil {
					t.Fatal(err)
				}
			case "shutdown":
				go a.stop()
			}
			waitFor(t, "b to lead", 25*time.Second, b.leads)
			if !hasFileWith(b.st, "big/model.bin", content) {
				t.Errorf("the new leader lacks the write acknowledged before the %s", how)
			}
		})
	}
}

// Review 2 stray-write-takes-new-leaders-epoch: a term ends only when every write admitted in
// it has ended, however long it runs (the old bound at shutdown was 1 s), so the write is in
// the journal the successor copies.
func TestATermEndsOnlyAfterTheWritesItAdmitted(t *testing.T) {
	a, b, _ := pair(t, nil, nil)
	done, ok := a.peer.BeginWrite() // a write stuck behind SQLite's writer, say
	if !ok {
		t.Fatal("a refused a write")
	}
	stopped := make(chan struct{})
	go func() {
		a.stop()
		close(stopped)
	}()
	time.Sleep(1500 * time.Millisecond)
	if b.leads() {
		t.Fatal("b leads while a write admitted by a is still in flight")
	}
	putDirect(t, a.st, "late.txt", "committed 1.5 s into the shutdown")
	done()
	waitFor(t, "b to lead", 25*time.Second, b.leads)
	if !hasFileWith(b.st, "late.txt", "committed 1.5 s into the shutdown") {
		t.Error("b lacks the write a acknowledged during its shutdown")
	}
	<-stopped
}

// The same when the term is lost: the instance may follow the next leader only after its own
// writes ended, else a late one is stamped with the next leader's epoch and looks like the
// next leader's own entry.
func TestALostTermFollowsOnlyAfterTheWritesItAdmitted(t *testing.T) {
	a, b, lock := pair(t, nil, nil)
	epoch := a.peer.Self().Epoch
	done, ok := a.peer.BeginWrite()
	if !ok {
		t.Fatal("a refused a write")
	}
	if err := os.Remove(lock); err != nil { // the term is lost: b locks a new lock file
		t.Fatal(err)
	}
	waitFor(t, "b to lead", 20*time.Second, b.leads)
	time.Sleep(1500 * time.Millisecond)
	blob, err := a.st.PutBlob(strings.NewReader("stray"), "", 0)
	if err != nil {
		t.Fatal(err)
	}
	_, _, e, err := a.st.PutFile("stray.txt", blob, "text/plain", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	done()
	if e.Epoch != epoch {
		t.Errorf("the late write of a's lost term is stamped with epoch %d, want a's %d (b leads in %d)", e.Epoch, epoch, b.peer.Self().Epoch)
	}
	b.put(t, "after.txt", "b's")
	waitFor(t, "a to follow b without the stray write", 15*time.Second, func() bool {
		return a.peer.Follower().State == "following" && hasFileWith(a.st, "after.txt", "b's")
	})
	if _, err := a.st.GetFile("stray.txt"); err == nil {
		t.Error("a keeps the stray write the leader never had")
	}
}

// Review 2 stale-instance-wins-and-wipes-newer-data: an instance that is behind the head the
// last leader recorded at its shutdown does not lead over it while that leader is away; when
// the leader comes back, nothing it acknowledged is lost.
func TestAnInstanceBehindTheRecordedHeadWaitsForItsWriter(t *testing.T) {
	adir, bdir := t.TempDir(), t.TempDir()
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	a := startInstance(t, "a", adir, lock)
	waitFor(t, "a to lead", 10*time.Second, a.leads)
	aaddr := a.addr()
	b := startInstance(t, "b", bdir, lock)
	caughtUp(t, a, b)
	b.stop() // the follower goes away
	for i := range 20 {
		a.put(t, fmt.Sprintf("newer/%d", i), "acknowledged while b was away")
	}
	a.stop() // graceful, nobody to hand over to: its record is final

	b2 := startInstance(t, "b", bdir, lock) // b comes back first
	time.Sleep(2500 * time.Millisecond)
	if b2.leads() {
		t.Fatal("b leads although it lacks 20 entries that a acknowledged before it stopped")
	}
	a2 := startInstance(t, "a", adir, lock, withAddr(aaddr))
	var leader, follower *instance
	waitFor(t, "a leader with a follower that caught up", 20*time.Second, func() bool {
		for _, pr := range [][2]*instance{{a2, b2}, {b2, a2}} {
			if pr[0].leads() && !pr[1].leads() && pr[1].peer.Follower().State == "following" {
				ls, _ := pr[0].st.Position()
				fs, _ := pr[1].st.Position()
				if ls == fs {
					leader, follower = pr[0], pr[1]
					return true
				}
			}
		}
		return false
	})
	for i := range 20 {
		for _, in := range []*instance{leader, follower} {
			if !hasFileWith(in.st, fmt.Sprintf("newer/%d", i), "acknowledged while b was away") {
				t.Errorf("%s lacks newer/%d", in.name, i)
			}
		}
	}
}

// When the writer of the recorded head does not come back within staleWait, the instance leads
// anyway, and says so at ERROR with how far behind it is.
func TestAnInstanceBehindTheRecordedHeadLeadsAfterWaiting(t *testing.T) {
	cluster.SetStaleWait(t, 2*time.Second)
	logs := captureLog(t)
	adir, bdir := t.TempDir(), t.TempDir()
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	a := startInstance(t, "a", adir, lock)
	waitFor(t, "a to lead", 10*time.Second, a.leads)
	b := startInstance(t, "b", bdir, lock)
	caughtUp(t, a, b)
	b.stop()
	a.put(t, "only-on-a.txt", "a's")
	a.stop()

	start := time.Now()
	b2 := startInstance(t, "b", bdir, lock)
	waitFor(t, "b to lead after waiting", 15*time.Second, b2.leads)
	if took := time.Since(start); took < 1900*time.Millisecond {
		t.Errorf("b led after %s, want after the 2 s it waits for a", took)
	}
	if n := logs.count("replica.stale_takeover"); n != 1 {
		t.Errorf("%d replica.stale_takeover lines, want 1", n)
	}
	if n := logs.count("leader.deferred"); n != 1 {
		t.Errorf("%d leader.deferred lines, want 1 (at the start of the wait)", n)
	}
}

// Review 2 leader-json-never-reannounced: a leader announces itself again when leader.json
// goes missing, and the follower follows again.
func TestALeaderAnnouncesItselfAgainWhenTheAnnouncementGoesMissing(t *testing.T) {
	a, b, lock := pair(t, nil, nil)
	if err := os.Remove(filepath.Join(filepath.Dir(lock), "leader.json")); err != nil {
		t.Fatal(err)
	}
	waitFor(t, "leader.json again", 15*time.Second, func() bool {
		_, err := os.Stat(filepath.Join(filepath.Dir(lock), "leader.json"))
		return err == nil
	})
	a.put(t, "after.txt", "x")
	caughtUp(t, a, b)
	if resp, body := do(t, http.MethodPut, b.url+"/v1/files/via-b.txt", strings.NewReader("y")); resp.StatusCode != http.StatusCreated {
		t.Errorf("PUT via the follower: %d %s", resp.StatusCode, body)
	}
}

// E2E-3: the follower asks for the next page of the journal while the blobs of the page before
// are still on their way, instead of one page at a time.
func TestTheFollowerAsksForTheNextPageWhileTheBlobsOfOneArrive(t *testing.T) {
	a, b, lock := pair(t, nil, nil)
	b.stop()
	for i := range 1100 { // more than a page (1000 entries), one blob each
		putDirect(t, a.st, fmt.Sprintf("page/%d", i), fmt.Sprintf("content %d", i))
	}
	b2 := startInstance(t, "b", b.dir, lock, withBlobsHeld())
	waitFor(t, "two journal requests while the first page's blobs are held", 20*time.Second, func() bool {
		seen := map[string]bool{}
		for _, after := range b2.gate.journalAfters() {
			seen[after] = true
		}
		return len(seen) >= 2
	})
	b2.gate.releaseBlobs()
	caughtUp(t, a, b2)
}

// E2E-3: a follower that restarts behind a leader that writes about 200 entries a second
// catches up while the load goes on.
func TestARestartedFollowerCatchesUpUnderLoad(t *testing.T) {
	a, b, lock := pair(t, nil, nil)
	var written atomic.Int64
	stop := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		tick := time.NewTicker(5 * time.Millisecond) // about 200 a second
		defer tick.Stop()
		for n := 0; ; n++ {
			select {
			case <-stop:
				return
			case <-tick.C:
			}
			putDirect(t, a.st, fmt.Sprintf("load/%06d", n), randomText(512+n%2048, int64(n)))
			written.Add(1)
		}
	}()
	defer func() {
		close(stop)
		wg.Wait()
	}()
	time.Sleep(time.Second)
	b.stop()
	time.Sleep(5 * time.Second) // about 1000 entries and blobs behind
	ls, _ := a.st.Position()
	b2 := startInstance(t, "b", b.dir, lock)
	start := time.Now()
	fs, _ := b2.st.Position()
	waitFor(t, "b to follow under the load", 60*time.Second, func() bool {
		f := b2.peer.Follower()
		return f.State == "following" && f.LagSeconds < 1
	})
	took := time.Since(start)
	t.Logf("restarted %d entries behind; following after %s; the leader wrote %d entries in all (%.0f/s)",
		ls-fs, took.Round(time.Millisecond), written.Load(), float64(written.Load())/time.Since(start.Add(-6*time.Second)).Seconds())
}

// E2E-6, D3: under a steady load the follower says "following" all along, and logs
// replica.caught_up only after it was behind by a second or more, once.
func TestAFollowerUnderASteadyLoadSaysFollowing(t *testing.T) {
	logs := captureLog(t)
	a, b, _ := pair(t, nil, nil)
	stop := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		for n := 0; ; n++ {
			select {
			case <-stop:
				return
			case <-time.After(10 * time.Millisecond):
			}
			putDirect(t, a.st, fmt.Sprintf("steady/%d", n), "steady "+strconv.Itoa(n)) // a blob each
		}
	}()
	var samples, following int
	for deadline := time.Now().Add(3 * time.Second); time.Now().Before(deadline); time.Sleep(5 * time.Millisecond) {
		samples++
		if f := b.peer.Follower(); f.State == "following" {
			following++
		}
	}
	if following < samples*98/100 {
		t.Errorf("following in %d of %d samples under a steady load", following, samples)
	}
	if n := logs.count("replica.caught_up"); n != 0 {
		t.Errorf("%d replica.caught_up lines although the follower was never a second behind", n)
	}

	b.gate.holdBlobs() // now it falls behind: the next entries wait for their blobs
	time.Sleep(1500 * time.Millisecond)
	// Once its lag reaches a second (on a loaded machine the writer may write the first entry
	// b waits for late), b says it is catching up.
	waitFor(t, "b to be a second behind", 20*time.Second, func() bool { return b.peer.Follower().LagSeconds >= 1 })
	if f := b.peer.Follower(); f.State != "catching_up" || f.LagSeconds < 1 || f.LagEntries < 1 {
		t.Errorf("held a second behind under the load: %+v", f)
	}
	b.gate.releaseBlobs()
	waitFor(t, "b to follow again", 20*time.Second, func() bool { return b.peer.Follower().State == "following" })
	close(stop)
	wg.Wait()
	caughtUp(t, a, b)
	if n := logs.count("replica.caught_up"); n != 1 {
		t.Errorf("%d replica.caught_up lines after one stretch behind, want 1:\n%s", n, logs.lines("replica.caught_up"))
	}
}

// E2E-11: a follower that is applying entries when it wins the election stops between two
// entries, and its catch-up from a predecessor that refuses connections ends at once.
func TestAFollowerThatWinsStopsApplyingAtOnce(t *testing.T) {
	a, b, _ := pair(t, nil, nil)
	cluster.SetBeforeApply(b.peer, func(store.JournalEntry) { time.Sleep(20 * time.Millisecond) }) // a slow disk
	for i := range 300 {
		putDirect(t, a.st, fmt.Sprintf("slow/%d", i), strconv.Itoa(i))
	}
	time.Sleep(300 * time.Millisecond) // b is applying them: 6 s at this pace
	start := time.Now()
	a.crash()
	waitFor(t, "b to lead", 20*time.Second, b.leads)
	if took := time.Since(start); took > 1500*time.Millisecond {
		t.Errorf("b led %s after the crash, want within about a second", took)
	}
}

// E2E-4: a follower promoted while it still lacks blobs serves them from the other instance
// (503 blob-missing, never 500, while that is away), says how many it lacks, and fetches them
// from the other instance once it is back.
func TestALeaderPromotedWithMissingBlobsFetchesThemFromTheOtherInstance(t *testing.T) {
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	adir := t.TempDir()
	a := startInstance(t, "a", adir, lock)
	waitFor(t, "a to lead", 10*time.Second, a.leads)
	aaddr := a.addr()
	for i := range 3 {
		a.put(t, fmt.Sprintf("plans/%d.json", i), compressible(5000+i))
	}
	b := startInstance(t, "b", t.TempDir(), lock, withBlobsHeld()) // an empty follower: a snapshot, no blobs yet
	waitFor(t, "b to follow without the blobs", 20*time.Second, func() bool {
		f := b.peer.Follower()
		return f.State == "following" && f.BlobsMissing == 3
	})
	a.crash()
	waitFor(t, "b to lead", 20*time.Second, b.leads)
	if n := b.peer.Follower().BlobsMissing; n != 3 {
		t.Errorf("the leader says it lacks %d blobs, want 3", n)
	}
	if resp, body := do(t, http.MethodGet, b.url+"/v1/files/plans/0.json", nil); resp.StatusCode != http.StatusServiceUnavailable ||
		resp.Header.Get("Cortex-Error") != "blob-missing" {
		t.Errorf("a file whose blob nobody can serve: %d %s", resp.StatusCode, body)
	}

	a2 := startInstance(t, "a", adir, lock, withAddr(aaddr)) // back, as a follower
	caughtUp(t, b, a2)
	// Before b has fetched it, the other instance serves it (b's own fetches are still held).
	if resp, body := do(t, http.MethodGet, b.url+"/v1/files/plans/1.json", nil); resp.StatusCode != http.StatusOK || body != compressible(5001) {
		t.Errorf("a file whose blob only the other instance has: %d %q", resp.StatusCode, body)
	}
	b.gate.releaseBlobs()
	waitFor(t, "the leader to fetch its blobs", 30*time.Second, func() bool { return b.peer.Follower().BlobsMissing == 0 })
	allBlobsPresent(t, b)
}
