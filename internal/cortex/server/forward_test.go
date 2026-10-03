package server

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/internal/cortex/store"
)

// withLeaderWait sets how long a follower waits for a leader.
func withLeaderWait(d time.Duration) func(*testConfig) {
	return func(c *testConfig) { c.leaderWait = d }
}

// setLeader points the fake node at another leader, as a new announcement would.
func (n *fakeNode) setLeader(leader *cluster.Info) {
	n.mu.Lock()
	n.leader = leader
	n.mu.Unlock()
}

func TestAFollowerThatCannotReachTheLeaderServesItsStaleCopy(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "fresh upstream"})
	leader := newTestCortex(t)
	follower := newTestCortex(t, withLeaderWait(500*time.Millisecond), withNode(func(*store.Store) cluster.Node {
		return newFollowerNode("b", &cluster.Info{Instance: "a", URL: leader.URL, Epoch: 1})
	}))
	storeVersion(t, follower.st, host.url("/p"), http.StatusOK, "the follower's copy", t0)
	follower.clock.advance(2 * time.Hour) // older than the host's max_age: a fetch is due
	leader.Close()
	before := scrape(t, follower)[`cortex_requests_total{source="unknown",mode="cache",result="stale_if_error"}`]

	start := time.Now()
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "the follower's copy" ||
		resp.Header.Get("Cache-Status") != "Cortex; hit; detail=stale-if-error" || resp.Header.Get("Cortex-Upstream-Error") != codeNoLeader ||
		resp.Header.Get("Cortex-Instance") != "b; role=follower" || resp.Header.Get("Cortex-Error") != "" {
		t.Errorf("stale on the follower: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	if took := time.Since(start); took < 450*time.Millisecond {
		t.Errorf("answered after %s, before the wait for a leader (500 ms)", took)
	}
	if got := scrape(t, follower)[`cortex_requests_total{source="unknown",mode="cache",result="stale_if_error"}`] - before; got != 1 {
		t.Errorf("stale_if_error counted %v times", got)
	}
	// HEAD and refresh too; never with stale=never, for a version of the past, or another hash.
	if resp, _ := do(t, http.MethodHead, follower.fetchURL(host.url("/p")), nil); resp.StatusCode != http.StatusOK {
		t.Errorf("HEAD: %d", resp.StatusCode)
	}
	if _, body := get(t, follower.fetchURL(host.url("/p"), "mode", "refresh")); body != "the follower's copy" {
		t.Errorf("refresh: %q", body)
	}
	for _, params := range [][]string{
		{"stale", "never"},
		{"at", t0.Add(-time.Hour).Format(time.RFC3339)},
		{"expect", "sha256:" + sha("another content")},
	} {
		resp, body := get(t, follower.fetchURL(host.url("/p"), params...))
		wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
	}
	if host.hitsOf("/p") != 0 {
		t.Error("the follower fetched upstream")
	}

	// Nor without any leader known; nothing stored is no-leader.
	node := newFollowerNode("c", nil)
	alone := newTestCortex(t, withLeaderWait(500*time.Millisecond), withNode(func(*store.Store) cluster.Node { return node }))
	storeVersion(t, alone.st, host.url("/p"), http.StatusOK, "c's copy", t0)
	alone.clock.advance(2 * time.Hour)
	if resp, body := get(t, alone.fetchURL(host.url("/p"))); resp.StatusCode != http.StatusOK || body != "c's copy" {
		t.Errorf("no leader known: %d %q", resp.StatusCode, body)
	}
	resp, body = get(t, alone.fetchURL(host.url("/other")))
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
}

func TestAFollowerForwardsToALeaderThatAnnouncesItselfWhileItWaits(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "fetched by the new leader"})
	old, follower, node := newPair(t)
	old.Close()
	successor := newTestCortex(t) // "a" again, as a single instance
	go func() {
		time.Sleep(300 * time.Millisecond)
		node.setLeader(&cluster.Info{Instance: "c", URL: successor.URL, Epoch: 2})
	}()
	start := time.Now()
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "fetched by the new leader" || time.Since(start) > 1500*time.Millisecond {
		t.Errorf("after the new announcement: %d %q after %s", resp.StatusCode, body, time.Since(start))
	}
	if versionsOf(t, successor, host.url("/p")) != 1 {
		t.Error("the new leader did not store it")
	}
}

func TestAFollowerWaitsForTheLeaderWhenTheAnnouncedOneDoesNotLead(t *testing.T) {
	// The instance the announcement names stepped down a moment ago: it answers no-leader.
	var asked atomic.Int64
	stepping := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		asked.Add(1)
		writeErrorRetry(w, http.StatusServiceUnavailable, codeNoLeader, time.Second, "this instance hands over")
	}))
	t.Cleanup(stepping.Close)
	node := newFollowerNode("b", &cluster.Info{Instance: "a", URL: stepping.URL, Epoch: 1})
	follower := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	host := newFakeHost(t)
	host.set("/p", page{body: "fetched by the new leader"})
	go func() {
		time.Sleep(300 * time.Millisecond)
		node.promote() // the follower wins the election
	}()
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "fetched by the new leader" || asked.Load() == 0 {
		t.Errorf("a fetch during a hand-over: %d %q (asked the old leader %d times)", resp.StatusCode, body, asked.Load())
	}
	if versionsOf(t, follower, host.url("/p")) != 1 {
		t.Error("the new leader did not store it")
	}

	// A write whose body went to the instance that refused it cannot be handled here (the body
	// is gone): the client gets the no-leader at once, on which it sends the write again.
	other := newTestCortex(t, withNode(func(*store.Store) cluster.Node {
		return newFollowerNode("c", &cluster.Info{Instance: "a", URL: stepping.URL, Epoch: 1})
	}))
	start := time.Now()
	resp, _ = putFile(t, other, "during-the-hand-over.txt", "sent to the old leader")
	if resp.StatusCode != http.StatusServiceUnavailable || resp.Header.Get("Cortex-Error") != codeNoLeader || time.Since(start) > time.Second {
		t.Errorf("a PUT refused by the old leader: %d %q after %s", resp.StatusCode, resp.Header.Get("Cortex-Error"), time.Since(start))
	}
}

func TestAForwardedWriteTheLeaderMayHaveAppliedIsNotAnsweredNoLeader(t *testing.T) {
	// A leader that reads the request and dies before it answers.
	var asked atomic.Int64
	dying := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		asked.Add(1)
		buf := make([]byte, 1<<16)
		for {
			if _, err := r.Body.Read(buf); err != nil {
				break
			}
		}
		conn, _, err := http.NewResponseController(w).Hijack()
		if err == nil {
			_ = conn.Close()
		}
	}))
	t.Cleanup(dying.Close)
	node := newFollowerNode("b", &cluster.Info{Instance: "a", URL: dying.URL, Epoch: 1})
	follower := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))

	for _, tc := range []struct {
		method, path, body string
	}{
		{http.MethodPut, "/v1/files/x.txt", "the content"},
		{http.MethodDelete, "/v1/files/x.txt", ""},
		{http.MethodDelete, "/v1/entries?url=" + "http%3A%2F%2Fexample.org%2F", ""},
	} {
		before := asked.Load()
		resp, body := do(t, tc.method, follower.URL+tc.path, strings.NewReader(tc.body))
		wantStatus(t, resp, body, http.StatusBadGateway, codeForwardFailed)
		if asked.Load()-before != 1 {
			t.Errorf("%s %s: the leader was asked %d times, want once", tc.method, tc.path, asked.Load()-before)
		}
	}
	// A read can be asked again: after the wait, no-leader.
	resp, body := get(t, follower.URL+"/v1/files/x.txt")
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeNoLeader)
}

func TestTheFollowerReportsTheBlobsItLacks(t *testing.T) {
	node := newFollowerNode("b", nil)
	node.setFollower(cluster.FollowerState{State: "following", LeaderURL: "http://a.invalid", BlobsMissing: 3})
	c := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))

	resp, body := get(t, c.URL+"/status")
	var st struct {
		Healthy  bool     `json:"healthy"`
		Problems []string `json:"problems"`
		Follower struct {
			BlobsMissing *int64 `json:"blobs_missing"`
		} `json:"follower"`
	}
	if err := json.Unmarshal([]byte(body), &st); err != nil || resp.StatusCode != http.StatusOK {
		t.Fatalf("/status: %d %v", resp.StatusCode, err)
	}
	if st.Follower.BlobsMissing == nil || *st.Follower.BlobsMissing != 3 || st.Healthy {
		t.Errorf("/status: %s", body)
	}
	resp, body = get(t, c.URL+"/healthz")
	if resp.StatusCode != http.StatusServiceUnavailable || !strings.Contains(body, "lacks 3 blobs") {
		t.Errorf("/healthz: %d %s", resp.StatusCode, body)
	}

	node.setFollower(cluster.FollowerState{State: "following", LeaderURL: "http://a.invalid"})
	node.setLeader(&cluster.Info{Instance: "a", URL: "http://a.invalid"})
	if resp, body := get(t, c.URL+"/healthz"); resp.StatusCode != http.StatusOK {
		t.Errorf("/healthz without missing blobs: %d %s", resp.StatusCode, body)
	}
	if resp, body := get(t, c.URL+"/status"); !strings.Contains(body, `"blobs_missing": 0`) {
		t.Errorf("/status shows no blobs_missing: %d %s", resp.StatusCode, body)
	}
}
