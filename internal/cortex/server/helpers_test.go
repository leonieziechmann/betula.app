package server

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"math/rand"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/internal/cortex/store"
	"github.com/leonieziechmann/betula/internal/cortex/upstream"
)

// t0 is when every test's clock starts.
var t0 = time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)

// fakeClock is the server's clock in the tests: it moves only when told.
type fakeClock struct {
	mu  sync.Mutex
	now time.Time
}

func (c *fakeClock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.now
}

func (c *fakeClock) advance(d time.Duration) {
	c.mu.Lock()
	c.now = c.now.Add(d)
	c.mu.Unlock()
}

// page is what the fake host answers for a path.
type page struct {
	status  int
	body    string
	header  map[string]string
	etag    string        // sent as ETag; If-None-Match with it → 304
	gate    chan struct{} // when set, the answer waits until it is closed
	started chan struct{} // when set, closed (once) when the request arrives
}

// fakeHost is a site Cortex fetches from: pages by path, the requests it got.
type fakeHost struct {
	*httptest.Server
	mu       sync.Mutex
	pages    map[string]*page
	hits     map[string]int
	headers  map[string]http.Header // of the last request per path
	finished map[string]int         // answers written to the end
	once     map[*page]*sync.Once
}

func newFakeHost(t *testing.T) *fakeHost {
	t.Helper()
	h := &fakeHost{pages: make(map[string]*page), hits: make(map[string]int), headers: make(map[string]http.Header),
		finished: make(map[string]int), once: make(map[*page]*sync.Once)}
	h.Server = httptest.NewServer(http.HandlerFunc(h.serve))
	t.Cleanup(h.Close)
	return h
}

func (h *fakeHost) set(path string, p page) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if p.status == 0 {
		p.status = http.StatusOK
	}
	h.pages[path] = &p
	h.once[&p] = new(sync.Once)
}

func (h *fakeHost) url(path string) string { return h.URL + path }

func (h *fakeHost) hitsOf(path string) int {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.hits[path]
}

func (h *fakeHost) finishedOf(path string) int {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.finished[path]
}

func (h *fakeHost) lastHeader(path string) http.Header {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.headers[path]
}

func (h *fakeHost) serve(w http.ResponseWriter, r *http.Request) {
	h.mu.Lock()
	h.hits[r.URL.Path]++
	h.headers[r.URL.Path] = r.Header.Clone()
	p := h.pages[r.URL.Path]
	var once *sync.Once
	if p != nil {
		once = h.once[p]
	}
	h.mu.Unlock()
	if p == nil {
		http.NotFound(w, r)
		return
	}
	if p.started != nil {
		once.Do(func() { close(p.started) })
	}
	if p.gate != nil {
		select {
		case <-p.gate:
		case <-r.Context().Done():
			return
		}
	}
	for k, v := range p.header {
		if v == "" {
			w.Header()[k] = nil // not sent, not sniffed
			continue
		}
		w.Header().Set(k, v)
	}
	if p.etag != "" {
		w.Header().Set("ETag", p.etag)
		if r.Header.Get("If-None-Match") == p.etag {
			w.WriteHeader(http.StatusNotModified)
			return
		}
	}
	w.WriteHeader(p.status)
	if _, err := io.WriteString(w, p.body); err == nil {
		h.mu.Lock()
		h.finished[r.URL.Path]++
		h.mu.Unlock()
	}
}

// testCortex is a Cortex instance on an httptest server.
type testCortex struct {
	*httptest.Server
	srv   *Server
	st    *store.Store
	up    *upstream.Upstream
	node  cluster.Node
	clock *fakeClock
}

type testConfig struct {
	policy       string // JSON (upstream.ParsePolicy); "" a default with max_age 1h
	allowPrivate bool
	node         func(st *store.Store) cluster.Node
	leaderWait   time.Duration
}

func withPolicy(policy string) func(*testConfig) {
	return func(c *testConfig) { c.policy = policy }
}

func withNode(node func(st *store.Store) cluster.Node) func(*testConfig) {
	return func(c *testConfig) { c.node = node }
}

// withAddressChecks lets the dialer refuse private addresses, as in production.
func withAddressChecks() func(*testConfig) {
	return func(c *testConfig) { c.allowPrivate = false }
}

const testPolicy = `{"default": {"max_age": "1h", "queue_wait": "10s", "breaker_failures": 100}}`

func newTestCortex(t *testing.T, opts ...func(*testConfig)) *testCortex {
	t.Helper()
	cfg := testConfig{policy: testPolicy, allowPrivate: true}
	for _, o := range opts {
		o(&cfg)
	}
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { _ = st.Close() })
	policy, err := upstream.ParsePolicy([]byte(cfg.policy))
	if err != nil {
		t.Fatalf("policy: %v", err)
	}
	up, err := upstream.New(upstream.Options{Policy: policy, AllowPrivate: cfg.allowPrivate})
	if err != nil {
		t.Fatalf("upstream.New: %v", err)
	}
	t.Cleanup(up.Close)
	var node cluster.Node
	if cfg.node != nil {
		node = cfg.node(st)
	} else if node, err = cluster.NewSingle(st, cluster.Info{Instance: "a", URL: "http://cortex-a.invalid"}); err != nil {
		t.Fatalf("NewSingle: %v", err)
	}
	clock := &fakeClock{now: t0}
	srv, err := New(Options{Store: st, Upstream: up, Node: node, Version: "test", Build: "0123456789ab",
		PruneInterval: -1, LeaderWait: cfg.leaderWait, Now: clock.Now})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	c := &testCortex{srv: srv, st: st, up: up, node: node, clock: clock}
	c.Server = httptest.NewServer(srv.Handler())
	t.Cleanup(func() {
		c.Server.Close()
		_ = srv.Close()
	})
	return c
}

// fetchURL is the /v1/fetch URL of target with the parameters given as name, value pairs.
func (c *testCortex) fetchURL(target string, params ...string) string {
	q := url.Values{"url": {target}}
	for i := 0; i+1 < len(params); i += 2 {
		q.Set(params[i], params[i+1])
	}
	return c.URL + "/v1/fetch?" + q.Encode()
}

// do sends a request and returns the answer with its body read.
func do(t *testing.T, method, target string, body io.Reader, header ...string) (*http.Response, string) {
	t.Helper()
	req, err := http.NewRequest(method, target, body)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i+1 < len(header); i += 2 {
		req.Header.Set(header[i], header[i+1])
	}
	if req.Header.Get("Accept-Encoding") == "" {
		req.Header.Set("Accept-Encoding", "identity") // no transparent gzip of the transport's own
	}
	resp, err := http.DefaultTransport.RoundTrip(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, target, err)
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("%s %s: reading the body: %v", method, target, err)
	}
	return resp, string(data)
}

func get(t *testing.T, target string, header ...string) (*http.Response, string) {
	t.Helper()
	return do(t, http.MethodGet, target, nil, header...)
}

// wantStatus fails unless resp has status (and, for a Cortex error, code).
func wantStatus(t *testing.T, resp *http.Response, body string, status int, code string) {
	t.Helper()
	if resp.StatusCode != status || resp.Header.Get("Cortex-Error") != code {
		t.Fatalf("answer %d %q, want %d %q; body %s", resp.StatusCode, resp.Header.Get("Cortex-Error"), status, code, body)
	}
	if code != "" {
		var e errorBody
		if err := json.Unmarshal([]byte(body), &e); err != nil || e.Error != code || e.Message == "" {
			t.Fatalf("error body %q, want {\"error\":%q,\"message\":…}", body, code)
		}
	}
}

func sha(s string) string {
	sum := sha256.Sum256([]byte(s))
	return hex.EncodeToString(sum[:])
}

// randomText is incompressible content (stored raw).
func randomText(n int, seed int64) string {
	r := rand.New(rand.NewSource(seed))
	b := make([]byte, n)
	for i := range b {
		b[i] = byte(r.Intn(256))
	}
	return string(b)
}

// compressible is content gzip shrinks (stored compressed).
func compressible(n int) string {
	return strings.Repeat("Modul 11101: Mathematik für Ingenieure. ", n/40+1)[:n]
}

// scrape reads GET /metrics into series → value.
func scrape(t *testing.T, c *testCortex) map[string]float64 {
	t.Helper()
	resp, body := get(t, c.URL+"/metrics")
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("/metrics: %d", resp.StatusCode)
	}
	out := make(map[string]float64)
	sc := bufio.NewScanner(strings.NewReader(body))
	for sc.Scan() {
		line := sc.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		i := strings.LastIndexByte(line, ' ')
		v, err := strconv.ParseFloat(line[i+1:], 64)
		if err != nil {
			t.Fatalf("metrics line %q: %v", line, err)
		}
		out[line[:i]] = v
	}
	return out
}

// storeVersion puts content into st directly as the answer to target fetched at at, as the
// leader's write would (for a follower's copy, or an old version).
func storeVersion(t *testing.T, st *store.Store, target string, status int, content string, at time.Time) store.Version {
	t.Helper()
	key, norm, host, err := store.Canonical(target, "", "")
	if err != nil {
		t.Fatal(err)
	}
	b, err := st.PutBlob(strings.NewReader(content), "", 0)
	if err != nil {
		t.Fatal(err)
	}
	_, v, _, _, err := st.RecordFetch(store.Fetched{Key: key, URL: norm, Host: host, Source: "test", Status: status,
		Hash: b.Hash, Size: b.Size, Header: http.Header{"Content-Type": {"text/plain"}}, At: at})
	if err != nil {
		t.Fatal(err)
	}
	return v
}

// fakeNode is a cluster.Node the test drives: a follower of a leader elsewhere, or a leader.
type fakeNode struct {
	mu       sync.Mutex
	role     cluster.Role
	self     cluster.Info
	leader   *cluster.Info
	follower cluster.FollowerState
	led      chan struct{} // closed when it becomes the leader
	changes  chan struct{}
	stepDown error
}

func newFollowerNode(self string, leader *cluster.Info) *fakeNode {
	return &fakeNode{role: cluster.Follower, self: cluster.Info{Instance: self, URL: "http://" + self + ".invalid"}, leader: leader,
		led: make(chan struct{}), changes: make(chan struct{}), follower: cluster.FollowerState{State: "following"}}
}

func (n *fakeNode) Role() cluster.Role {
	n.mu.Lock()
	defer n.mu.Unlock()
	return n.role
}

func (n *fakeNode) Self() cluster.Info {
	n.mu.Lock()
	defer n.mu.Unlock()
	return n.self
}

func (n *fakeNode) Leader() (cluster.Info, bool) {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.role == cluster.Leader {
		return n.self, true
	}
	if n.leader == nil {
		return cluster.Info{}, false
	}
	return *n.leader, true
}

func (n *fakeNode) WaitLeader(ctx context.Context, d time.Duration) bool {
	t := time.NewTimer(d)
	defer t.Stop()
	n.mu.Lock()
	led := n.led
	n.mu.Unlock()
	select {
	case <-led:
		return true
	case <-t.C:
	case <-ctx.Done():
	}
	return n.Role() == cluster.Leader
}

func (n *fakeNode) BeginWrite() (func(), bool) {
	return func() {}, n.Role() == cluster.Leader
}

func (n *fakeNode) StepDown() error {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.role != cluster.Leader {
		return cluster.ErrNotLeader
	}
	if n.stepDown != nil {
		return n.stepDown
	}
	n.role = cluster.Follower
	n.led = make(chan struct{})
	close(n.changes)
	n.changes = make(chan struct{})
	return nil
}

func (n *fakeNode) Follower() cluster.FollowerState {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.role == cluster.Leader {
		return cluster.FollowerState{}
	}
	return n.follower
}

func (n *fakeNode) LeaderChanges() <-chan struct{} {
	n.mu.Lock()
	defer n.mu.Unlock()
	return n.changes
}

// promote makes the node the leader.
func (n *fakeNode) promote() {
	n.mu.Lock()
	defer n.mu.Unlock()
	n.role = cluster.Leader
	n.self.Epoch++
	close(n.led)
	close(n.changes)
	n.changes = make(chan struct{})
}

func (n *fakeNode) setFollower(f cluster.FollowerState) {
	n.mu.Lock()
	n.follower = f
	n.mu.Unlock()
}
