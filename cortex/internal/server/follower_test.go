package server

import (
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/cluster"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

// newPair returns a leader (a single instance, "a") and a follower ("b") that knows it.
func newPair(t *testing.T) (leader, follower *testCortex, node *fakeNode) {
	t.Helper()
	leader = newTestCortex(t)
	node = newFollowerNode("b", &cluster.Info{Instance: "a", URL: leader.URL, Epoch: 1})
	follower = newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	return leader, follower, node
}

func TestAFollowerServesWhatItHasWithoutTheLeader(t *testing.T) {
	host := newFakeHost(t)
	leader, follower, _ := newPair(t)
	storeVersion(t, follower.st, host.url("/p"), http.StatusOK, "the follower's copy", t0)
	follower.clock.advance(10 * time.Minute)
	leader.Close() // nobody to ask

	for _, mode := range []string{"cache", "offline"} {
		resp, body := get(t, follower.fetchURL(host.url("/p"), "mode", mode))
		if resp.StatusCode != http.StatusOK || body != "the follower's copy" || resp.Header.Get("Cache-Status") != "Cortex; hit" ||
			resp.Header.Get("Cortex-Instance") != "b; role=follower" {
			t.Errorf("%s: %d %q %v", mode, resp.StatusCode, body, resp.Header)
		}
	}
	if resp, body := get(t, follower.fetchURL(host.url("/p"), "at", t0.Add(time.Minute).Format(time.RFC3339))); body != "the follower's copy" {
		t.Errorf("at: %d %q", resp.StatusCode, body)
	}
	if host.hitsOf("/p") != 0 {
		t.Error("the follower fetched upstream")
	}
}

func TestAFollowerForwardsMissesAndWritesToTheLeader(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "fetched by the leader"})
	leader, follower, _ := newPair(t)
	before := scrape(t, follower)[`cortex_requests_total{source="unknown",mode="cache",result="forwarded"}`]

	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "fetched by the leader" || resp.Header.Get("Cortex-Instance") != "a; role=leader" ||
		resp.Header.Get("Cache-Status") != "Cortex; fwd=miss; fwd-status=200; stored" {
		t.Errorf("forwarded miss: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	if got := scrape(t, follower)[`cortex_requests_total{source="unknown",mode="cache",result="forwarded"}`] - before; got != 1 {
		t.Errorf("forwarded counted %v times", got)
	}
	if versionsOf(t, leader, host.url("/p")) != 1 || versionsOf(t, follower, host.url("/p")) != 0 {
		t.Error("the leader did not store it, or the follower wrote")
	}
	// The follower's offline miss: the leader may have it.
	if resp, body := get(t, follower.fetchURL(host.url("/p"), "mode", "offline")); resp.StatusCode != http.StatusOK || body != "fetched by the leader" {
		t.Errorf("forwarded offline miss: %d %q", resp.StatusCode, body)
	}
	if host.hitsOf("/p") != 1 {
		t.Errorf("%d requests upstream, want 1", host.hitsOf("/p"))
	}

	// Writes and the reads of what only the leader has.
	if resp, info := putFile(t, follower, "from/b.txt", "written through b"); resp.StatusCode != http.StatusCreated || info.Name != "from/b.txt" {
		t.Errorf("forwarded PUT: %d", resp.StatusCode)
	}
	if fv, err := leader.st.GetFile("from/b.txt"); err != nil || fv.Hash != sha("written through b") {
		t.Errorf("the leader's file: %+v %v", fv, err)
	}
	if _, err := follower.st.GetFile("from/b.txt"); err == nil {
		t.Error("the follower wrote the file itself")
	}
	if resp, body := get(t, follower.URL+"/v1/files/from/b.txt"); resp.StatusCode != http.StatusOK || body != "written through b" {
		t.Errorf("forwarded GET of a file the follower lacks: %d %q", resp.StatusCode, body)
	}
	if resp, _ := do(t, http.MethodDelete, follower.URL+"/v1/files/from/b.txt", nil); resp.StatusCode != http.StatusNoContent {
		t.Errorf("forwarded DELETE: %d", resp.StatusCode)
	}
	if resp, _ := do(t, http.MethodPost, follower.URL+"/v1/admin/prune", nil); resp.StatusCode != http.StatusOK || resp.Header.Get("Cortex-Instance") != "a; role=leader" {
		t.Errorf("forwarded prune: %d", resp.StatusCode)
	}
	blob := compressible(5000)
	if resp, _ := do(t, http.MethodPut, follower.URL+"/v1/blobs/sha256:"+sha(blob), strings.NewReader(blob)); resp.StatusCode != http.StatusCreated || !leader.st.HasBlob(sha(blob)) {
		t.Errorf("forwarded blob PUT: %d", resp.StatusCode)
	}
	if resp, body := get(t, follower.URL+"/v1/blobs/sha256:"+sha(blob)); resp.StatusCode != http.StatusOK || body != blob {
		t.Errorf("forwarded blob GET: %d", resp.StatusCode)
	}

	// Step-down is the leader's own business: a follower refuses it.
	resp, body = do(t, http.MethodPost, follower.URL+"/v1/admin/step-down", nil)
	wantStatus(t, resp, body, http.StatusConflict, codeNotLeader)
}

func TestAForwardedRequestIsNeverForwardedAgain(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	_, follower, _ := newPair(t)
	resp, body := get(t, follower.fetchURL(host.url("/p")), "Cortex-Forwarded", "a")
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
	if resp.Header.Get("Retry-After") != "1" || host.hitsOf("/p") != 0 {
		t.Errorf("Retry-After %q, %d requests upstream", resp.Header.Get("Retry-After"), host.hitsOf("/p"))
	}
}

func TestAFollowerWhoseLeaderIsDownAnswersNoLeaderAfterTwoSeconds(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	leader, follower, _ := newPair(t)
	leader.Close()

	start := time.Now()
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	took := time.Since(start)
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
	if resp.Header.Get("Retry-After") != "1" || took < 1900*time.Millisecond || took > 5*time.Second {
		t.Errorf("Retry-After %q after %s, want 1 after about 2 s", resp.Header.Get("Retry-After"), took)
	}
	if resp.Header.Get("Cortex-Instance") != "b; role=follower" {
		t.Errorf("Cortex-Instance %q", resp.Header.Get("Cortex-Instance"))
	}
}

func TestAFollowerThatTakesOverWhileWaitingHandlesTheRequestItself(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "fetched by b"})
	leader, follower, node := newPair(t)
	leader.Close()
	go func() {
		time.Sleep(300 * time.Millisecond)
		node.promote()
	}()
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "fetched by b" {
		t.Errorf("after the take-over: %d %q", resp.StatusCode, body)
	}
	if versionsOf(t, follower, host.url("/p")) != 1 {
		t.Error("the new leader did not store it")
	}
}

func TestAnUploadTheLeaderNeverReadIsStoredByTheInstanceThatTookOver(t *testing.T) {
	leader, follower, node := newPair(t)
	leader.Close()
	go func() {
		time.Sleep(300 * time.Millisecond)
		node.promote()
	}()
	resp, info := putFile(t, follower, "upload.bin", "the whole body")
	if resp.StatusCode != http.StatusCreated || info.SHA256 != "sha256:"+sha("the whole body") || info.Size != 14 {
		t.Errorf("PUT after the take-over: %d %+v", resp.StatusCode, info)
	}
}

func TestAFollowerWithoutALeaderWaitsThenAnswersNoLeader(t *testing.T) {
	node := newFollowerNode("b", nil)
	c := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	start := time.Now()
	resp, body := do(t, http.MethodDelete, c.URL+"/v1/files/x", nil)
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
	if took := time.Since(start); took < 1900*time.Millisecond {
		t.Errorf("answered after %s, want after the 2 s wait", took)
	}
}

func TestTheLeaderStepsDownOnRequest(t *testing.T) {
	node := newFollowerNode("a", nil)
	node.promote()
	c := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	before := scrape(t, c)["cortex_leader_changes_total"]

	resp, body := do(t, http.MethodPost, c.URL+"/v1/admin/step-down", nil)
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Cortex-Instance") != "a; role=follower" || node.Role() != cluster.Follower {
		t.Errorf("step-down: %d %s %v", resp.StatusCode, body, resp.Header)
	}
	resp, body = do(t, http.MethodPost, c.URL+"/v1/admin/step-down", nil)
	wantStatus(t, resp, body, http.StatusConflict, codeNotLeader)
	waitFor(t, "the role change to be counted", func() bool { return scrape(t, c)["cortex_leader_changes_total"]-before == 1 })

	// A single instance has nobody to hand over to.
	single := newTestCortex(t)
	resp, body = do(t, http.MethodPost, single.URL+"/v1/admin/step-down", nil)
	wantStatus(t, resp, body, http.StatusConflict, codeCannotStepDown)
}

func TestAFollowerForwardsAnEntryWhoseBlobItLacks(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "the leader's copy"})
	leader, follower, _ := newPair(t)
	get(t, leader.fetchURL(host.url("/p")))

	// The follower has the row (replicated) but not yet the blob (fetched in the background).
	v := storeVersion(t, follower.st, host.url("/p"), http.StatusOK, "the leader's copy", t0)
	for _, suffix := range []string{"", ".gz"} {
		_ = os.Remove(filepath.Join(follower.st.Dir(), "blobs", "sha256", v.Hash[:2], v.Hash+suffix))
	}
	if follower.st.HasBlob(v.Hash) {
		t.Fatal("the blob is still there")
	}
	for _, mode := range []string{"cache", "offline"} {
		resp, body := get(t, follower.fetchURL(host.url("/p"), "mode", mode))
		if resp.StatusCode != http.StatusOK || body != "the leader's copy" || resp.Header.Get("Cortex-Instance") != "a; role=leader" {
			t.Errorf("%s: %d %q %v", mode, resp.StatusCode, body, resp.Header)
		}
	}
	if host.hitsOf("/p") != 1 {
		t.Errorf("%d requests upstream, want the leader's one", host.hitsOf("/p"))
	}
}
