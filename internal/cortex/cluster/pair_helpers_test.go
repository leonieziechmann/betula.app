package cluster_test

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"math/rand"
	"net"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/internal/cortex/server"
	"github.com/leonieziechmann/betula/internal/cortex/store"
	"github.com/leonieziechmann/betula/internal/cortex/upstream"
)

// The tests of this file run two instances in the process, each with its own store in its
// own directory, the real server (server.New) on a real listener, and one lock file: the
// replication goes over HTTP as in production. Upstream is an httptest site.

const testPolicy = `{"default": {"max_age": "1h", "queue_wait": "10s", "breaker_failures": 100}}`

// site is a host Cortex fetches from.
type site struct {
	*httptest.Server
	mu    sync.Mutex
	pages map[string]string
	hits  map[string]int
}

func newSite(t *testing.T) *site {
	t.Helper()
	s := &site{pages: make(map[string]string), hits: make(map[string]int)}
	s.Server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		s.mu.Lock()
		s.hits[r.URL.Path]++
		body, ok := s.pages[r.URL.Path]
		s.mu.Unlock()
		if !ok {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		_, _ = io.WriteString(w, body)
	}))
	t.Cleanup(s.Close)
	return s
}

func (s *site) set(path, body string) {
	s.mu.Lock()
	s.pages[path] = body
	s.mu.Unlock()
}

func (s *site) hitsOf(path string) int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.hits[path]
}

func (s *site) url(path string) string { return s.URL + path }

// gate is a follower's transport to its leader that a test can hold: the long polls of the
// journal (not the catch-up of a new leader, which does not wait) and the blob downloads.
type gate struct {
	base      http.RoundTripper
	mu        sync.Mutex
	journal   chan struct{} // closed while open
	blobs     chan struct{}
	snapshots atomic.Int64
	afters    []string        // the after of every journal request, in order
	polled    map[string]bool // the after of every long poll that passed the open gate
	waiting   atomic.Int64    // journal requests waiting at the held gate now
}

// journalAfters returns the after of every journal request so far.
func (g *gate) journalAfters() []string {
	g.mu.Lock()
	defer g.mu.Unlock()
	return append([]string(nil), g.afters...)
}

func newGate() *gate {
	g := &gate{base: &http.Transport{Proxy: nil, DisableCompression: true}, journal: make(chan struct{}), blobs: make(chan struct{}),
		polled: make(map[string]bool)}
	close(g.journal)
	close(g.blobs)
	return g
}

func (g *gate) RoundTrip(req *http.Request) (*http.Response, error) {
	var wait chan struct{}
	var held bool // a journal request counted in waiting
	g.mu.Lock()
	if req.URL.Path == "/internal/v1/journal" {
		g.afters = append(g.afters, req.URL.Query().Get("after"))
	}
	switch {
	case req.URL.Path == "/internal/v1/journal" && req.URL.Query().Get("wait") != "0s":
		select {
		case <-g.journal:
			g.polled[req.URL.Query().Get("after")] = true
		default:
			wait = g.journal
			held = true
			g.waiting.Add(1)
		}
	case strings.HasPrefix(req.URL.Path, "/v1/blobs/"):
		wait = g.blobs
	case req.URL.Path == "/internal/v1/snapshot":
		g.snapshots.Add(1)
	}
	g.mu.Unlock()
	if wait != nil {
		select {
		case <-wait:
		case <-req.Context().Done():
		}
		if held {
			g.waiting.Add(-1)
		}
		if err := req.Context().Err(); err != nil {
			return nil, err
		}
	}
	return g.base.RoundTrip(req)
}

func hold(mu *sync.Mutex, ch *chan struct{}) {
	mu.Lock()
	defer mu.Unlock()
	select {
	case <-*ch:
		*ch = make(chan struct{})
	default:
	}
}

func release(mu *sync.Mutex, ch *chan struct{}) {
	mu.Lock()
	defer mu.Unlock()
	select {
	case <-*ch:
	default:
		close(*ch)
	}
}

func (g *gate) holdJournal()    { hold(&g.mu, &g.journal) }
func (g *gate) releaseJournal() { release(&g.mu, &g.journal) }
func (g *gate) holdBlobs()      { hold(&g.mu, &g.blobs) }
func (g *gate) releaseBlobs()   { release(&g.mu, &g.blobs) }

// holdJournalBehindLongPoll holds the journal's long polls once one asking for the entries
// after after has passed the gate: that one waits at the leader and answers with the next
// entry, the next one waits here. Holding at once could catch the follower between two long
// polls (under load it applies the last answer and asks again later), and then nothing would
// reach it.
func (g *gate) holdJournalBehindLongPoll(t *testing.T, after int64) {
	t.Helper()
	a := strconv.FormatInt(after, 10)
	waitFor(t, "a long poll after "+a+" to pass the gate", 20*time.Second, func() bool {
		g.mu.Lock()
		defer g.mu.Unlock()
		if !g.polled[a] {
			return false
		}
		select {
		case <-g.journal:
			g.journal = make(chan struct{})
		default:
		}
		return true
	})
}

// awaitHeldJournal waits until a long poll of the journal waits at the held gate: the one
// under way has ended, and nothing reaches the follower until the gate is released.
func (g *gate) awaitHeldJournal(t *testing.T) {
	t.Helper()
	waitFor(t, "a long poll to wait at the gate", 20*time.Second, func() bool { return g.waiting.Load() > 0 })
}

// instance is one Cortex of the pair.
type instance struct {
	name, dir string
	st        *store.Store
	up        *upstream.Upstream
	peer      *cluster.Peer
	srv       *server.Server
	ts        *httptest.Server
	url       string
	gate      *gate
	stopOnce  sync.Once
}

type config struct {
	pollWait  time.Duration
	history   time.Duration
	addr      string // listen here (a restart on the same address); "": any port
	holdBlobs bool
}

func withPollWait(d time.Duration) func(*config) { return func(c *config) { c.pollWait = d } }
func withHistory(d time.Duration) func(*config)  { return func(c *config) { c.history = d } }
func withAddr(addr string) func(*config)         { return func(c *config) { c.addr = addr } }
func withBlobsHeld() func(*config)               { return func(c *config) { c.holdBlobs = true } }

// startInstance runs an instance on dir (a restart when it held an instance before).
func startInstance(t *testing.T, name, dir, lock string, opts ...func(*config)) *instance {
	t.Helper()
	var cfg config
	for _, o := range opts {
		o(&cfg)
	}
	st, err := store.Open(dir)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	policy, err := upstream.ParsePolicy([]byte(testPolicy))
	if err != nil {
		t.Fatal(err)
	}
	up, err := upstream.New(upstream.Options{Policy: policy, AllowPrivate: true})
	if err != nil {
		t.Fatal(err)
	}
	in := &instance{name: name, dir: dir, st: st, up: up, gate: newGate()}
	if cfg.holdBlobs {
		in.gate.holdBlobs()
	}
	var handler atomic.Value // http.Handler, once the server exists
	in.ts = httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		handler.Load().(http.Handler).ServeHTTP(w, r)
	}))
	if cfg.addr != "" {
		_ = in.ts.Listener.Close()
		var ln net.Listener
		for deadline := time.Now().Add(5 * time.Second); ; {
			if ln, err = net.Listen("tcp", cfg.addr); err == nil || time.Now().After(deadline) {
				break
			}
			time.Sleep(20 * time.Millisecond)
		}
		if err != nil {
			t.Fatalf("listening on %s again: %v", cfg.addr, err)
		}
		in.ts.Listener = ln
	}
	in.url = "http://" + in.ts.Listener.Addr().String()
	in.peer, err = cluster.New(context.Background(), st, cluster.Info{Instance: name, URL: in.url}, lock,
		cluster.Options{Transport: in.gate, PollWait: cfg.pollWait})
	if err != nil {
		t.Fatalf("cluster.New: %v", err)
	}
	in.srv, err = server.New(server.Options{Store: st, Upstream: up, Node: in.peer, Version: "test", PruneInterval: -1, History: cfg.history})
	if err != nil {
		t.Fatalf("server.New: %v", err)
	}
	handler.Store(in.srv.Handler())
	in.ts.Start()
	t.Cleanup(in.stop)
	return in
}

// stop stops the instance as SIGTERM does: the node hands over while the server answers.
func (in *instance) stop() {
	in.stopOnce.Do(func() {
		_ = in.peer.Close()
		_ = in.srv.Close()
		in.ts.CloseClientConnections()
		in.ts.Close()
		in.up.Close()
		_ = in.st.Close()
	})
}

// crash stops the instance as a process that dies: the listener goes away first, then the
// lock file is closed without resigning; nothing is handed over.
func (in *instance) crash() {
	in.stopOnce.Do(func() {
		_ = in.srv.Close() // the long polls answer; nothing runs detached any more
		in.ts.CloseClientConnections()
		in.ts.Close()
		cluster.Crash(in.peer)
		in.up.Close()
		_ = in.st.Close()
	})
}

func (in *instance) leads() bool { return in.peer.Role() == cluster.Leader }

func (in *instance) addr() string { return strings.TrimPrefix(in.url, "http://") }

func waitFor(t *testing.T, what string, within time.Duration, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(within)
	for !cond() {
		if time.Now().After(deadline) {
			t.Fatalf("waited %s for %s", within, what)
		}
		time.Sleep(5 * time.Millisecond)
	}
}

// caughtUp waits until follower has leader's position and says so.
func caughtUp(t *testing.T, leader, follower *instance) {
	t.Helper()
	waitFor(t, follower.name+" to catch up with "+leader.name, 20*time.Second, func() bool {
		ls, le := leader.st.Position()
		fs, fe := follower.st.Position()
		f := follower.peer.Follower()
		return ls == fs && le == fe && f.State == "following" && f.LagEntries == 0
	})
}

// pair starts a leader and then a follower that has caught up with it.
func pair(t *testing.T, aOpts, bOpts []func(*config)) (a, b *instance, lock string) {
	t.Helper()
	lock = filepath.Join(t.TempDir(), "lock", "leader.lock")
	a = startInstance(t, "a", t.TempDir(), lock, aOpts...)
	waitFor(t, "a to lead", 10*time.Second, a.leads)
	b = startInstance(t, "b", t.TempDir(), lock, bOpts...)
	caughtUp(t, a, b)
	return a, b, lock
}

// do sends a request to an instance and returns the answer with its body read.
func do(t *testing.T, method, target string, body io.Reader, header ...string) (*http.Response, string) {
	t.Helper()
	req, err := http.NewRequest(method, target, body)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i+1 < len(header); i += 2 {
		req.Header.Set(header[i], header[i+1])
	}
	req.Header.Set("Accept-Encoding", "identity")
	resp, err := (&http.Transport{Proxy: nil}).RoundTrip(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, target, err)
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("%s %s: %v", method, target, err)
	}
	return resp, string(data)
}

func (in *instance) fetchURL(target string, params ...string) string {
	q := url.Values{"url": {target}}
	for i := 0; i+1 < len(params); i += 2 {
		q.Set(params[i], params[i+1])
	}
	return in.url + "/v1/fetch?" + q.Encode()
}

func (in *instance) fetch(t *testing.T, target string, params ...string) (*http.Response, string) {
	t.Helper()
	return do(t, http.MethodGet, in.fetchURL(target, params...), nil)
}

func (in *instance) put(t *testing.T, name, content string) *http.Response {
	t.Helper()
	resp, body := do(t, http.MethodPut, in.url+"/v1/files/"+name, strings.NewReader(content), "Content-Type", "text/plain")
	if resp.StatusCode != http.StatusCreated && resp.StatusCode != http.StatusOK {
		t.Fatalf("PUT %s on %s: %d %s", name, in.name, resp.StatusCode, body)
	}
	return resp
}

// status reads GET /status.
func (in *instance) status(t *testing.T) map[string]any {
	t.Helper()
	resp, body := do(t, http.MethodGet, in.url+"/status", nil)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("/status of %s: %d", in.name, resp.StatusCode)
	}
	var doc map[string]any
	if err := json.Unmarshal([]byte(body), &doc); err != nil {
		t.Fatal(err)
	}
	return doc
}

func sha(s string) string {
	sum := sha256.Sum256([]byte(s))
	return hex.EncodeToString(sum[:])
}

func randomText(n int, seed int64) string {
	r := rand.New(rand.NewSource(seed))
	b := make([]byte, n)
	for i := range b {
		b[i] = byte(r.Intn(256))
	}
	return string(b)
}

func compressible(n int) string {
	return strings.Repeat("Modul 11101: Mathematik für Ingenieure. ", n/40+1)[:n]
}

// rows is everything an index holds that a follower must copy, to compare two instances.
type rows struct {
	Seq, Epoch int64
	Stats      store.Stats
	Journal    []store.JournalEntry
	Entries    map[string][]store.Version
	EntryRows  map[string]store.Entry
	Files      map[string][]store.FileVersion
}

// rowsOf reads st, with the versions of the files named (deleted ones included).
func rowsOf(t *testing.T, st *store.Store, files []string) rows {
	t.Helper()
	var r rows
	r.Seq, r.Epoch = st.Position()
	var err error
	if r.Stats, err = st.Stats(); err != nil {
		t.Fatal(err)
	}
	if r.Journal, err = st.JournalAfter(0, store.MaxLimit); err != nil {
		t.Fatal(err)
	}
	infos, _, err := st.ListEntries(store.EntryFilter{}, "", store.MaxLimit)
	if err != nil {
		t.Fatal(err)
	}
	r.Entries, r.EntryRows = make(map[string][]store.Version), make(map[string]store.Entry)
	for _, info := range infos {
		e, versions, err := st.Versions(info.Key)
		if err != nil {
			t.Fatal(err)
		}
		r.Entries[info.Key], r.EntryRows[info.Key] = versions, e
	}
	r.Files = make(map[string][]store.FileVersion)
	for _, name := range files {
		versions, err := st.FileVersions(name)
		if err != nil && !errors.Is(err, store.ErrNotFound) {
			t.Fatal(err)
		}
		r.Files[name] = versions
	}
	return r
}

// sameRows fails unless the two stores hold the same rows (and, with blobs, the same blobs).
func sameRows(t *testing.T, a, b *instance, files []string, blobs bool) {
	t.Helper()
	ra, rb := rowsOf(t, a.st, files), rowsOf(t, b.st, files)
	if !blobs {
		ra.Stats.Blobs, ra.Stats.BlobBytes, ra.Stats.BlobOriginalBytes = 0, 0, 0
		rb.Stats.Blobs, rb.Stats.BlobBytes, rb.Stats.BlobOriginalBytes = 0, 0, 0
	}
	if ra.Seq != rb.Seq || ra.Epoch != rb.Epoch {
		t.Errorf("position: %s %d/%d, %s %d/%d", a.name, ra.Seq, ra.Epoch, b.name, rb.Seq, rb.Epoch)
	}
	if ra.Stats != rb.Stats {
		t.Errorf("stats: %s %+v, %s %+v", a.name, ra.Stats, b.name, rb.Stats)
	}
	if !reflect.DeepEqual(ra.Journal, rb.Journal) {
		t.Errorf("the journals differ: %s has %d entries, %s %d", a.name, len(ra.Journal), b.name, len(rb.Journal))
	}
	if !reflect.DeepEqual(ra.EntryRows, rb.EntryRows) || !reflect.DeepEqual(ra.Entries, rb.Entries) {
		t.Errorf("the entries differ:\n%s %+v\n%s %+v", a.name, ra.Entries, b.name, rb.Entries)
	}
	if !reflect.DeepEqual(ra.Files, rb.Files) {
		t.Errorf("the files differ:\n%s %+v\n%s %+v", a.name, ra.Files, b.name, rb.Files)
	}
}

// allBlobsPresent fails unless st has every blob its index references.
func allBlobsPresent(t *testing.T, in *instance) {
	t.Helper()
	var missing []string
	if err := in.st.ReferencedBlobs(func(hash string) error {
		if !in.st.HasBlob(hash) {
			missing = append(missing, hash)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if len(missing) > 0 {
		t.Errorf("%s lacks %d referenced blobs: %v", in.name, len(missing), missing)
	}
}

// removeBlob deletes a blob file from st behind the store's back.
func removeBlob(t *testing.T, st *store.Store, hash string) {
	t.Helper()
	for _, suffix := range []string{"", ".gz"} {
		if err := os.Remove(filepath.Join(st.Dir(), "blobs", "sha256", hash[:2], hash+suffix)); err != nil && !errors.Is(err, os.ErrNotExist) {
			t.Fatal(err)
		}
	}
}
