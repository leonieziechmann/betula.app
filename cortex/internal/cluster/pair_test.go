//go:build unix && !aix && !solaris && !hurd

package cluster_test

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/cortex/internal/cluster"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

func TestAFollowerKeepsTheRowsOfItsLeader(t *testing.T) {
	host := newSite(t)
	host.set("/a", compressible(20000))
	host.set("/b", "the first content")
	host.set("/c", randomText(3000, 1))
	a, b, _ := pair(t, []func(*config){withHistory(time.Second)}, []func(*config){withHistory(time.Second)})
	if a.peer.Self().Epoch != 1 || b.leads() {
		t.Fatalf("a leads in epoch %d, b leads %v", a.peer.Self().Epoch, b.leads())
	}

	// Writes on the leader: fetch misses, a new version, files, deletions, retention (which
	// removes the first version of /b, the first of x, and y with its version and tombstone).
	for _, path := range []string{"/a", "/b", "/c"} {
		if resp, _ := a.fetch(t, host.url(path)); resp.StatusCode != http.StatusOK {
			t.Fatalf("fetch %s: %d", path, resp.StatusCode)
		}
	}
	host.set("/b", "the second content")
	if _, body := a.fetch(t, host.url("/b"), "mode", "refresh"); body != "the second content" {
		t.Fatalf("refresh: %q", body)
	}
	a.put(t, "docs/x.txt", "x, first")
	a.put(t, "docs/x.txt", "x, second")
	a.put(t, "docs/y.bin", randomText(5000, 2))
	if resp, _ := do(t, http.MethodDelete, a.url+"/v1/files/docs/y.bin", nil); resp.StatusCode != http.StatusNoContent {
		t.Fatalf("DELETE file: %d", resp.StatusCode)
	}
	if resp, _ := do(t, http.MethodDelete, a.url+"/v1/entries?url="+host.url("/c"), nil); resp.StatusCode != http.StatusNoContent {
		t.Fatalf("DELETE entry: %d", resp.StatusCode)
	}
	time.Sleep(1100 * time.Millisecond) // the superseded versions are older than the history (1 s)
	resp, body := do(t, http.MethodPost, a.url+"/v1/admin/prune", nil)
	var report struct {
		Prune struct {
			Versions     int64 `json:"versions"`
			FileVersions int64 `json:"file_versions"`
			Files        int64 `json:"files"`
		} `json:"prune"`
	}
	if err := json.Unmarshal([]byte(body), &report); err != nil || resp.StatusCode != http.StatusOK ||
		report.Prune.Versions != 1 || report.Prune.FileVersions != 3 || report.Prune.Files != 1 {
		t.Fatalf("prune: %d %s", resp.StatusCode, body)
	}

	caughtUp(t, a, b)
	sameRows(t, a, b, []string{"docs/x.txt", "docs/y.bin"}, true)
	allBlobsPresent(t, b)

	// The follower serves what it has itself.
	hits := host.hitsOf("/a")
	resp, body = b.fetch(t, host.url("/a"))
	if resp.StatusCode != http.StatusOK || body != compressible(20000) || resp.Header.Get("Cache-Status") != "Cortex; hit" ||
		resp.Header.Get("Cortex-Instance") != "b; role=follower" || host.hitsOf("/a") != hits {
		t.Errorf("a hit on the follower: %d %v", resp.StatusCode, resp.Header)
	}
	resp, body = do(t, http.MethodGet, b.url+"/v1/files/docs/x.txt", nil)
	if resp.StatusCode != http.StatusOK || body != "x, second" || resp.Header.Get("Cortex-Instance") != "b; role=follower" {
		t.Errorf("a file on the follower: %d %q %v", resp.StatusCode, body, resp.Header)
	}

	st := b.status(t)
	follower, _ := st["follower"].(map[string]any)
	leader, _ := st["leader"].(map[string]any)
	if st["role"] != "follower" || follower["state"] != "following" || follower["lag_entries"] != float64(0) ||
		follower["blobs_missing"] != float64(0) || follower["leader_url"] != a.url || leader["instance"] != "a" ||
		leader["epoch"] != float64(1) {
		t.Errorf("the follower's status: %v", st)
	}
	if st := a.status(t); st["role"] != "leader" {
		t.Errorf("the leader's status: %v", st)
	}
}

func TestTheFollowerTakesOverFromACrashedLeaderThatRejoinsAfterwards(t *testing.T) {
	host := newSite(t)
	host.set("/p", "a page")
	host.set("/q", "a page b fetched while leading")
	a, b, lock := pair(t, nil, nil)
	a.put(t, "before.txt", "replicated")
	a.fetch(t, host.url("/p"))
	caughtUp(t, a, b)

	// Entries b never gets: its next long poll waits at its gate (once the one asking after
	// what b has is under way, so that it answers with the next entry).
	seqB, _ := b.st.Position()
	b.gate.holdJournalBehindLongPoll(t, seqB)
	a.put(t, "reaches-b.txt", "the long poll already waiting answers with it")
	caughtUp(t, a, b)
	a.put(t, "lost.txt", "only a has it")
	a.put(t, "lost-too.bin", randomText(4000, 3))
	epochA := a.peer.Self().Epoch
	dirA, addrA := a.dir, a.addr()

	start := time.Now()
	a.crash()
	waitFor(t, "b to take over", 10*time.Second, b.leads)
	t.Logf("b took over after %s", time.Since(start))
	if e := b.peer.Self().Epoch; e <= epochA {
		t.Errorf("b leads in epoch %d, a led in %d", e, epochA)
	}
	b.put(t, "after.txt", "written while a was down")
	if resp, body := b.fetch(t, host.url("/q")); resp.StatusCode != http.StatusOK || body != "a page b fetched while leading" {
		t.Errorf("b's first fetch: %d %q", resp.StatusCode, body)
	}
	if resp, _ := do(t, http.MethodGet, b.url+"/v1/files/lost.txt", nil); resp.StatusCode != http.StatusNotFound {
		t.Errorf("b has a's last write: %d", resp.StatusCode)
	}

	// a comes back on its data: its last entries are not b's, so it starts from b's snapshot,
	// and fetches the blobs that b wrote meanwhile.
	a2 := startInstance(t, "a", dirA, lock, withAddr(addrA), withBlobsHeld())
	waitFor(t, "a to count the blobs it lacks", 20*time.Second, func() bool {
		f := a2.peer.Follower()
		return f.BlobsMissing == 2 && f.State == "following"
	})
	if st := a2.status(t); st["follower"].(map[string]any)["blobs_missing"] != float64(2) || st["healthy"] != false {
		t.Errorf("a's status while it lacks blobs: %v", st)
	}
	if resp, body := do(t, http.MethodGet, a2.url+"/healthz", nil); resp.StatusCode != http.StatusServiceUnavailable || !strings.Contains(body, "lacks 2 blobs") {
		t.Errorf("a's /healthz while it lacks blobs: %d %s", resp.StatusCode, body)
	}
	a2.gate.releaseBlobs()
	waitFor(t, "a to fetch the blobs", 20*time.Second, func() bool { return a2.peer.Follower().BlobsMissing == 0 })
	if a2.leads() || !b.leads() {
		t.Fatal("the instance that came back took over")
	}
	if n := a2.gate.snapshots.Load(); n != 1 {
		t.Errorf("a took %d snapshots, want 1", n)
	}
	caughtUp(t, b, a2)
	sameRows(t, b, a2, []string{"before.txt", "reaches-b.txt", "lost.txt", "lost-too.bin", "after.txt"}, false)
	allBlobsPresent(t, a2)
	if _, err := a2.st.GetFile("lost.txt"); !errors.Is(err, store.ErrNotFound) {
		t.Errorf("a kept the write b never got: %v", err)
	}
	if resp, body := do(t, http.MethodGet, a2.url+"/v1/files/after.txt", nil); resp.StatusCode != http.StatusOK ||
		body != "written while a was down" || resp.Header.Get("Cortex-Instance") != "a; role=follower" {
		t.Errorf("a serving b's file: %d %q %v", resp.StatusCode, body, resp.Header)
	}
}

func TestAStepDownHandsOverEveryAcknowledgedWrite(t *testing.T) {
	a, b, _ := pair(t, nil, []func(*config){withPollWait(100 * time.Millisecond)})

	// b's next long poll waits at its gate; the one under way ends within its 100 ms.
	b.gate.holdJournal()
	b.gate.awaitHeldJournal(t)
	a.put(t, "acknowledged.txt", "b has not seen this one when a steps down")
	if seqA, _ := a.st.Position(); func() int64 { s, _ := b.st.Position(); return s }() == seqA {
		t.Fatal("b got the write before the step-down: the test proves nothing")
	}

	start := time.Now()
	resp, body := do(t, http.MethodPost, a.url+"/v1/admin/step-down", nil)
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Cortex-Instance") != "a; role=follower" {
		t.Fatalf("step-down: %d %s %v", resp.StatusCode, body, resp.Header)
	}
	t.Logf("the step-down took %s", time.Since(start))
	// The step-down ends once b announced itself; b takes the role just after it announced,
	// which a loaded machine may run a little later.
	waitFor(t, "b to lead after the step-down", 10*time.Second, b.leads)
	if !b.leads() || a.leads() {
		t.Fatalf("after the step-down a leads %v, b leads %v", a.leads(), b.leads())
	}
	fv, err := b.st.GetFile("acknowledged.txt")
	if err != nil || fv.Hash != sha("b has not seen this one when a steps down") {
		t.Fatalf("the new leader lacks the acknowledged write: %+v %v", fv, err)
	}
	if resp, _ := do(t, http.MethodPost, a.url+"/v1/admin/step-down", nil); resp.StatusCode != http.StatusConflict {
		t.Errorf("a second step-down on the follower: %d", resp.StatusCode)
	}
	b.gate.releaseJournal()
	caughtUp(t, b, a)
	sameRows(t, b, a, []string{"acknowledged.txt"}, true)
	if a.peer.Follower().LeaderURL != b.url {
		t.Errorf("a follows %q, want %q", a.peer.Follower().LeaderURL, b.url)
	}
}

func TestAStepDownUnderLoadLosesNoAcknowledgedWrite(t *testing.T) {
	a, b, _ := pair(t, nil, nil)
	c, err := client.New(a.url+","+b.url, client.Options{FailoverWait: 10 * time.Second})
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	acked := map[string]string{} // name → sha256
	stop := make(chan struct{})
	var wg sync.WaitGroup
	for w := range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := 0; ; n++ {
				select {
				case <-stop:
					return
				default:
				}
				name := "load/" + string(rune('a'+w)) + "/" + time.Now().Format("150405.000000000")
				content := name + " " + randomText(64, int64(n))
				info, _, err := c.PutFile(context.Background(), name, strings.NewReader(content), client.PutOptions{ContentType: "text/plain"})
				if err != nil {
					continue // not acknowledged
				}
				if info.SHA256 != "sha256:"+sha(content) {
					t.Errorf("%s acknowledged with %s", name, info.SHA256)
				}
				mu.Lock()
				acked[name] = sha(content)
				mu.Unlock()
			}
		}()
	}
	// Writes before the step-down and after it, however slowly the machine runs them.
	ackedSoFar := func() int {
		mu.Lock()
		defer mu.Unlock()
		return len(acked)
	}
	time.Sleep(500 * time.Millisecond)
	waitFor(t, "writes before the step-down", 20*time.Second, func() bool { return ackedSoFar() >= 5 })
	if resp, body := do(t, http.MethodPost, a.url+"/v1/admin/step-down", nil); resp.StatusCode != http.StatusOK {
		t.Fatalf("step-down: %d %s", resp.StatusCode, body)
	}
	before := ackedSoFar()
	time.Sleep(500 * time.Millisecond)
	waitFor(t, "writes after the step-down", 20*time.Second, func() bool { return ackedSoFar() >= before+5 })
	close(stop)
	wg.Wait()

	if !b.leads() {
		t.Fatal("b does not lead after the step-down")
	}
	caughtUp(t, b, a)
	if len(acked) < 10 {
		t.Fatalf("only %d writes acknowledged", len(acked))
	}
	for name, hash := range acked {
		for _, in := range []*instance{b, a} {
			if fv, err := in.st.GetFile(name); err != nil || fv.Hash != hash {
				t.Errorf("%s lacks the acknowledged %s: %+v %v", in.name, name, fv, err)
			}
		}
	}
	t.Logf("%d writes acknowledged around the step-down", len(acked))
}

func TestAFollowerOnAnEmptyDataDirectoryCopiesTheLeader(t *testing.T) {
	host := newSite(t)
	host.set("/big", compressible(200000))
	host.set("/raw", randomText(10000, 4))
	lock := t.TempDir() + "/leader.lock"
	a := startInstance(t, "a", t.TempDir(), lock)
	waitFor(t, "a to lead", 10*time.Second, a.leads)
	a.fetch(t, host.url("/big"))
	a.fetch(t, host.url("/raw"))
	a.put(t, "plans/b-sc.json", compressible(9000))
	a.put(t, "plans/m-sc.json", `{"modules":[]}`)

	b := startInstance(t, "b", t.TempDir(), lock, withBlobsHeld())
	waitFor(t, "b to count the blobs it lacks", 20*time.Second, func() bool {
		f := b.peer.Follower()
		return f.BlobsMissing == 4 && f.State == "following"
	})
	if n := b.gate.snapshots.Load(); n != 1 {
		t.Errorf("%d snapshots, want 1", n)
	}
	b.gate.releaseBlobs()
	waitFor(t, "b to fetch the blobs", 20*time.Second, func() bool { return b.peer.Follower().BlobsMissing == 0 })
	caughtUp(t, a, b)
	sameRows(t, a, b, []string{"plans/b-sc.json", "plans/m-sc.json"}, true)
	allBlobsPresent(t, b)

	// What b has, it serves itself, also without the leader.
	_ = a.srv.Close()
	a.ts.CloseClientConnections()
	a.ts.Close()
	if resp, body := do(t, http.MethodGet, b.url+"/v1/files/plans/b-sc.json", nil); resp.StatusCode != http.StatusOK || body != compressible(9000) {
		t.Errorf("a file from b: %d", resp.StatusCode)
	}
	if resp, body := b.fetch(t, host.url("/raw"), "mode", "offline"); resp.StatusCode != http.StatusOK || body != randomText(10000, 4) {
		t.Errorf("a page from b: %d", resp.StatusCode)
	}
}

func TestAFollowerImportsABlobThatVanishedBeforeItsEntryApplied(t *testing.T) {
	a, b, _ := pair(t, nil, nil)
	var once sync.Once
	vanished := make(chan string, 1)
	cluster.SetBeforeApply(b.peer, func(e store.JournalEntry) {
		if e.Blob == "" {
			return
		}
		once.Do(func() {
			// The blob was there when the follower looked; blob GC removes it before Apply.
			removeBlob(t, b.st, e.Blob)
			vanished <- e.Blob
		})
	})
	a.put(t, "x.txt", "content whose blob vanishes on the follower")
	caughtUp(t, a, b)
	select {
	case hash := <-vanished:
		if !b.st.HasBlob(hash) {
			t.Errorf("the follower applied the entry without its blob")
		}
	default:
		t.Fatal("the hook never ran")
	}
	if n := cluster.Failures(b.peer); n != 0 {
		t.Errorf("%d failed rounds: the follower should import the blob and apply the entry again at once", n)
	}
	sameRows(t, a, b, []string{"x.txt"}, true)
}

func TestAFollowerThatCannotReachItsLeaderServesWhatItHas(t *testing.T) {
	host := newSite(t)
	host.set("/p", "the follower's copy")
	host.set("/new", "nobody has fetched this")
	a, b, _ := pair(t, nil, nil)
	a.fetch(t, host.url("/p"))
	caughtUp(t, a, b)

	// a still holds the lock but cannot be reached.
	_ = a.srv.Close()
	a.ts.CloseClientConnections()
	a.ts.Close()

	start := time.Now()
	resp, body := b.fetch(t, host.url("/p"), "max_age", "0s")
	if resp.StatusCode != http.StatusOK || body != "the follower's copy" ||
		resp.Header.Get("Cache-Status") != "Cortex; hit; detail=stale-if-error" || resp.Header.Get("Cortex-Upstream-Error") != "no-leader" {
		t.Errorf("stale on the follower: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	if took := time.Since(start); took < 1900*time.Millisecond || took > 15*time.Second {
		t.Errorf("answered after %s, want after the 2 s the follower waits for a leader", took)
	}
	for _, tc := range []struct{ name, url, stale string }{
		{"stale=never", host.url("/p"), "never"},
		{"nothing stored", host.url("/new"), ""},
	} {
		params := []string{"max_age", "0s"}
		if tc.stale != "" {
			params = append(params, "stale", tc.stale)
		}
		resp, body := b.fetch(t, tc.url, params...)
		if resp.StatusCode != http.StatusServiceUnavailable || resp.Header.Get("Cortex-Error") != "no-leader" || resp.Header.Get("Retry-After") != "1" {
			t.Errorf("%s: %d %s", tc.name, resp.StatusCode, body)
		}
	}
	if b.leads() {
		t.Error("b took over although a holds the lock")
	}
	a.peer.Close()
}
