package main

import (
	"bytes"
	"context"
	"encoding/json"
	"flag"
	"io"
	"log/slog"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/radix/internal/cortex/server"
	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
	"github.com/leonieziechmann/betula/radix/internal/cortex/upstream"
)

// capture replaces stdin, stdout and stderr for one command.
func capture(t *testing.T, in string) (out, errOut *bytes.Buffer) {
	t.Helper()
	out, errOut = new(bytes.Buffer), new(bytes.Buffer)
	oldIn, oldOut, oldErr := stdin, stdout, stderr
	stdin, stdout, stderr = strings.NewReader(in), out, errOut
	t.Cleanup(func() { stdin, stdout, stderr = oldIn, oldOut, oldErr })
	return out, errOut
}

// newInstance runs a Cortex in the process, with node (nil: a single instance).
func newInstance(t *testing.T, node func(*store.Store) cluster.Node) (*httptest.Server, *store.Store) {
	t.Helper()
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = st.Close() })
	up, err := upstream.New(upstream.Options{AllowPrivate: true})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(up.Close)
	var n cluster.Node
	if node != nil {
		n = node(st)
	} else if n, err = cluster.NewSingle(st, cluster.Info{Instance: "a", URL: "http://cortex_a:8100"}); err != nil {
		t.Fatal(err)
	}
	srv, err := server.New(server.Options{Store: st, Upstream: up, Node: n, Version: cortexVersion, PruneInterval: -1})
	if err != nil {
		t.Fatal(err)
	}
	ts := httptest.NewServer(srv.Handler())
	t.Cleanup(func() {
		ts.Close()
		_ = srv.Close()
	})
	return ts, st
}

func TestStatusPrintsOneJSONObject(t *testing.T) {
	ts, _ := newInstance(t, nil)
	out, errOut := capture(t, "")
	if code := runStatus(context.Background(), []string{"--url", ts.URL}); code != 0 {
		t.Fatalf("exit %d: %s", code, errOut)
	}
	dec := json.NewDecoder(bytes.NewReader(out.Bytes()))
	dec.UseNumber()
	var doc struct {
		Role     string      `json:"role"`
		Epoch    json.Number `json:"epoch"`
		Seq      json.Number `json:"seq"`
		Follower struct {
			LagSeconds json.Number `json:"lag_seconds"`
			State      *string     `json:"state"`
		} `json:"follower"`
	}
	if err := dec.Decode(&doc); err != nil {
		t.Fatalf("not JSON: %v\n%s", err, out)
	}
	if dec.More() {
		t.Errorf("more than one JSON document:\n%s", out)
	}
	if _, err := doc.Epoch.Int64(); err != nil || doc.Role != "leader" || doc.Follower.LagSeconds == "" || doc.Follower.State == nil {
		t.Errorf("status %+v", doc)
	}
	if _, err := doc.Seq.Int64(); err != nil {
		t.Errorf("seq %q is not an integer", doc.Seq)
	}
}

// steppingNode is a leader whose StepDown succeeds.
type steppingNode struct {
	mu   sync.Mutex
	lead bool
}

func (n *steppingNode) Role() cluster.Role {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.lead {
		return cluster.Leader
	}
	return cluster.Follower
}
func (n *steppingNode) Self() cluster.Info                                   { return cluster.Info{Instance: "a"} }
func (n *steppingNode) Leader() (cluster.Info, bool)                         { return cluster.Info{}, false }
func (n *steppingNode) WaitLeader(ctx context.Context, d time.Duration) bool { return false }
func (n *steppingNode) BeginWrite() (func(), bool)                           { return func() {}, n.Role() == cluster.Leader }
func (n *steppingNode) Follower() cluster.FollowerState                      { return cluster.FollowerState{} }
func (n *steppingNode) LeaderChanges() <-chan struct{}                       { return nil }
func (n *steppingNode) StepDown() error {
	n.mu.Lock()
	defer n.mu.Unlock()
	if !n.lead {
		return cluster.ErrNotLeader
	}
	n.lead = false
	return nil
}

func TestStepDownExitsNonZeroUnlessItHandsOver(t *testing.T) {
	node := &steppingNode{lead: true}
	ts, _ := newInstance(t, func(*store.Store) cluster.Node { return node })
	capture(t, "")
	if code := runStepDown(context.Background(), []string{"--url", ts.URL}); code != 0 {
		t.Errorf("the leader: exit %d, want 0", code)
	}
	_, errOut := capture(t, "")
	if code := runStepDown(context.Background(), []string{"--url", ts.URL}); code != 1 || !strings.Contains(errOut.String(), "does not lead") {
		t.Errorf("a follower (409 not-leader): exit %d, %s", code, errOut)
	}

	single, _ := newInstance(t, nil)
	_, errOut = capture(t, "")
	if code := runStepDown(context.Background(), []string{"--url", single.URL}); code != 1 || !strings.Contains(errOut.String(), "cannot-step-down") {
		t.Errorf("a single instance: exit %d, %s", code, errOut)
	}
}

func TestHealthcheckExitCodes(t *testing.T) {
	ts, st := newInstance(t, nil)
	out, _ := capture(t, "")
	if code := runHealthcheck(context.Background(), []string{"--url", ts.URL + "/livez"}); code != 0 || !strings.Contains(out.String(), `"ok"`) {
		t.Errorf("live: exit %d, %s", code, out)
	}
	t.Setenv("CORTEX_HEALTH_URL", ts.URL+"/livez")
	capture(t, "")
	if code := runHealthcheck(context.Background(), nil); code != 0 {
		t.Errorf("from CORTEX_HEALTH_URL: exit %d", code)
	}
	_ = st.Close()
	capture(t, "")
	if code := runHealthcheck(context.Background(), nil); code != 1 {
		t.Errorf("an index that does not answer: exit %d, want 1", code)
	}
	ts.Close()
	capture(t, "")
	if code := runHealthcheck(context.Background(), nil); code != 1 {
		t.Errorf("nobody listening: exit %d, want 1", code)
	}
}

func TestPutAndGetAFile(t *testing.T) {
	ts, _ := newInstance(t, nil)
	file := filepath.Join(t.TempDir(), "plan.json")
	if err := os.WriteFile(file, []byte(`{"modules":[]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	out, errOut := capture(t, "")
	if code := runPut(context.Background(), []string{"--url", ts.URL, "plans/b-sc.json", file}); code != 0 {
		t.Fatalf("put: exit %d, %s", code, errOut)
	}
	var info struct {
		Name        string `json:"name"`
		SHA256      string `json:"sha256"`
		ContentType string `json:"content_type"`
		Created     bool   `json:"created"`
	}
	if err := json.Unmarshal(out.Bytes(), &info); err != nil || info.Name != "plans/b-sc.json" || !info.Created ||
		info.ContentType != "application/json" || !strings.HasPrefix(info.SHA256, "sha256:") {
		t.Errorf("put printed %s (%v)", out, err)
	}

	out, errOut = capture(t, "")
	if code := runGet(context.Background(), []string{"--url", ts.URL, "plans/b-sc.json"}); code != 0 || out.String() != `{"modules":[]}` {
		t.Errorf("get: exit %d, %q %s", code, out, errOut)
	}

	// From stdin, with a type of its own.
	capture(t, "from a pipe")
	if code := runPut(context.Background(), []string{"--url", ts.URL, "--type", "text/plain", "notes"}); code != 0 {
		t.Errorf("put from stdin: exit %d", code)
	}
	out, _ = capture(t, "")
	if code := runGet(context.Background(), []string{"--url", ts.URL, "notes"}); code != 0 || out.String() != "from a pipe" {
		t.Errorf("get after a put from stdin: exit %d, %q", code, out)
	}

	_, errOut = capture(t, "")
	if code := runGet(context.Background(), []string{"--url", ts.URL, "missing"}); code != 1 || !strings.Contains(errOut.String(), "not-found") {
		t.Errorf("get of a missing file: exit %d, %s", code, errOut)
	}
	capture(t, "")
	if code := runGet(context.Background(), []string{"--url", ts.URL}); code != 2 {
		t.Errorf("get without a name: exit %d, want 2", code)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	capture(t, "")
	if code := runGet(ctx, []string{"--url", ts.URL, "notes"}); code != 130 {
		t.Errorf("an interrupted get: exit %d, want 130", code)
	}
}

func TestEveryFlagNamesItsEnvironmentVariable(t *testing.T) {
	env := regexp.MustCompile(`\(env (CORTEX_[A-Z_]+)\)$`)
	sets := []*flag.FlagSet{}
	fs, _ := newServeFlags()
	sets = append(sets, fs)
	for _, name := range []string{"status", "step-down", "put", "get"} {
		cfs, _ := clientFlags(name)
		sets = append(sets, cfs)
	}
	for _, fs := range sets {
		fs.VisitAll(func(f *flag.Flag) {
			if !env.MatchString(f.Usage) {
				t.Errorf("%s --%s: help %q does not end in (env CORTEX_…)", fs.Name(), f.Name, f.Usage)
			}
		})
	}

	t.Setenv("CORTEX_ADDR", "0.0.0.0:9100")
	t.Setenv("CORTEX_HISTORY", "720h")
	t.Setenv("CORTEX_ALLOW_PRIVATE", "true")
	t.Setenv("CORTEX_LOG_LEVEL", "debug")
	_, o := newServeFlags()
	if *o.addr != "0.0.0.0:9100" || *o.history != 720*time.Hour || !*o.allowPrivate || *o.logs.level != "debug" {
		t.Errorf("defaults from the environment: addr %s, history %s, allow-private %v, log level %s", *o.addr, *o.history, *o.allowPrivate, *o.logs.level)
	}
	fs, o = newServeFlags()
	if err := fs.Parse([]string{"--addr", "127.0.0.1:1"}); err != nil || *o.addr != "127.0.0.1:1" {
		t.Errorf("a flag wins over the environment: %s", *o.addr)
	}
}

func TestCommandsAndExitCodes(t *testing.T) {
	t.Setenv("CORTEX_ENV_FILE", filepath.Join(t.TempDir(), "absent.env"))
	out, _ := capture(t, "")
	if code := run([]string{"help"}); code != 0 || !strings.Contains(out.String(), "step-down") {
		t.Errorf("help: exit %d\n%s", code, out)
	}
	out, _ = capture(t, "")
	if code := run([]string{"version"}); code != 0 || !strings.HasPrefix(out.String(), "Cortex "+cortexVersion) {
		t.Errorf("version: exit %d, %q", code, out)
	}
	_, errOut := capture(t, "")
	if code := run([]string{"frobnicate"}); code != 2 || !strings.Contains(errOut.String(), "Unknown command") {
		t.Errorf("an unknown command: exit %d", code)
	}
	capture(t, "")
	if code := run([]string{"status", "--no-such-flag"}); code != 2 {
		t.Errorf("an unknown flag: exit %d, want 2", code)
	}
	capture(t, "")
	if code := run([]string{"status", "--url", "ftp://x"}); code != 2 {
		t.Errorf("an invalid --url: exit %d, want 2", code)
	}
}

// freeAddr returns a local address nothing listens on.
func freeAddr(t *testing.T) string {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	addr := ln.Addr().String()
	ln.Close()
	return addr
}

func TestServeRunsUntilItIsStopped(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })
	addr := freeAddr(t)
	data := t.TempDir()
	capture(t, "")

	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan int, 1)
	go func() {
		done <- runServe(ctx, []string{"--addr", addr, "--data", data, "--instance", "a", "--log-level", "error",
			"--log-file", filepath.Join(data, "cortex.log")})
	}()
	deadline := time.Now().Add(10 * time.Second)
	for {
		resp, err := http.Get("http://" + addr + "/livez")
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				break
			}
		}
		if time.Now().After(deadline) {
			t.Fatalf("serve did not come up: %v", err)
		}
		time.Sleep(20 * time.Millisecond)
	}
	resp, err := http.Get("http://" + addr + "/metrics")
	if err != nil {
		t.Fatal(err)
	}
	metrics, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	for _, want := range []string{"cortex_build_info{build=", "cortex_start_time_seconds ", "cortex_role 1"} {
		if !strings.Contains(string(metrics), want) {
			t.Errorf("/metrics lacks %q", want)
		}
	}
	if strings.Contains(string(metrics), "radix_") {
		t.Error("/metrics shows radix_ metrics")
	}
	resp, err = http.Get("http://" + addr + "/status")
	if err != nil {
		t.Fatal(err)
	}
	var st struct {
		Instance, Role, Version string
	}
	_ = json.NewDecoder(resp.Body).Decode(&st)
	resp.Body.Close()
	if st.Instance != "a" || st.Role != "leader" || st.Version != cortexVersion {
		t.Errorf("status %+v", st)
	}

	start := time.Now()
	cancel()
	select {
	case code := <-done:
		if code != 0 {
			t.Errorf("serve exited %d after the stop, want 0", code)
		}
	case <-time.After(25 * time.Second):
		t.Fatal("serve did not stop within 25 s")
	}
	// A connection a client dialled but never used counts as busy for 5 s (net/http).
	if took := time.Since(start); took > 10*time.Second {
		t.Errorf("stopping took %s", took)
	}
	if _, err := os.Stat(filepath.Join(data, "index.db")); err != nil {
		t.Errorf("no index in the data directory: %v", err)
	}
}

func TestServeRefusesWhatItCannotRun(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })
	capture(t, "")
	for _, tc := range []struct {
		name string
		args []string
	}{
		{"a broken host policy", []string{"--hosts", filepath.Join(t.TempDir(), "missing.json")}},
		{"a history of zero", []string{"--history", "0s"}},
		{"an unknown log level", []string{"--log-level", "loud"}},
		{"an argument", []string{"extra"}},
		{"a pair whose URL the other cannot reach", []string{"--lock", filepath.Join(t.TempDir(), "leader.lock"), "--addr", "0.0.0.0:0"}},
		{"a pair with an advertise URL without a host", []string{"--lock", filepath.Join(t.TempDir(), "leader.lock"), "--advertise-url", "http://:8100"}},
	} {
		args := append([]string{"--addr", freeAddr(t), "--data", t.TempDir(), "--log-level", "error"}, tc.args...)
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second) // a serve that runs exits 0
		if code := runServe(ctx, args); code != 2 {
			t.Errorf("%s: exit %d, want 2", tc.name, code)
		}
		cancel()
	}
	// A lock file that cannot be opened (a directory) cannot elect anybody.
	args := []string{"--addr", freeAddr(t), "--data", t.TempDir(), "--log-level", "error", "--lock", t.TempDir()}
	if code := runServe(context.Background(), args); code != 1 {
		t.Errorf("a lock file that cannot be opened: exit %d, want 1", code)
	}
}

// statusOf runs `cortex status` against addr and decodes what it prints; nil when it fails.
func statusOf(out *bytes.Buffer, addr string) map[string]any {
	out.Reset()
	if code := runStatus(context.Background(), []string{"--url", "http://" + addr}); code != 0 {
		return nil
	}
	var doc map[string]any
	if json.Unmarshal(out.Bytes(), &doc) != nil {
		return nil
	}
	return doc
}

func TestServeWithALockElectsOneLeaderThatHandsOver(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })
	out, _ := capture(t, "") // once: the instances read the streams while they run
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	addrs := map[string]string{"a": freeAddr(t), "b": freeAddr(t)}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan int, 2)
	for name, addr := range addrs {
		data := t.TempDir()
		go func() {
			done <- runServe(ctx, []string{"--addr", addr, "--data", data, "--instance", name, "--lock", lock, "--log-level", "error"})
		}()
	}

	// roles waits until one instance leads and the other follows it, caught up.
	roles := func(what string) (leader, follower string) {
		t.Helper()
		deadline := time.Now().Add(15 * time.Second)
		for {
			st := map[string]map[string]any{}
			for name, addr := range addrs {
				st[name] = statusOf(out, addr)
			}
			for _, pair := range [][2]string{{"a", "b"}, {"b", "a"}} {
				l, f := st[pair[0]], st[pair[1]]
				if l == nil || f == nil || l["role"] != "leader" || f["role"] != "follower" {
					continue
				}
				fs, _ := f["follower"].(map[string]any)
				if fs["state"] == "following" && fs["lag_entries"] == float64(0) && fs["blobs_missing"] == float64(0) &&
					fs["leader_url"] == "http://"+addrs[pair[0]] && f["epoch"] == l["epoch"] {
					return pair[0], pair[1]
				}
			}
			if time.Now().After(deadline) {
				t.Fatalf("%s: no leader with a follower that caught up: %v", what, st)
			}
			time.Sleep(50 * time.Millisecond)
		}
	}
	leader, follower := roles("at start")
	before := statusOf(out, addrs[leader])["epoch"].(float64)

	if code := runStepDown(context.Background(), []string{"--url", "http://" + addrs[leader]}); code != 0 {
		t.Fatalf("step-down of the leader: exit %d", code)
	}
	newLeader, newFollower := roles("after the step-down")
	if newLeader != follower || newFollower != leader {
		t.Errorf("after the step-down %s leads, want %s", newLeader, follower)
	}
	if after := statusOf(out, addrs[newLeader])["epoch"].(float64); after <= before {
		t.Errorf("the epoch went from %v to %v", before, after)
	}
	if code := runStepDown(context.Background(), []string{"--url", "http://" + addrs[newFollower]}); code != 1 {
		t.Errorf("step-down of the follower: exit %d, want 1", code)
	}

	cancel()
	for range addrs {
		select {
		case code := <-done:
			if code != 0 {
				t.Errorf("serve exited %d after the stop, want 0", code)
			}
		case <-time.After(30 * time.Second):
			t.Fatal("serve did not stop within 30 s")
		}
	}
}

// D5: an instance of a pair that is stopped hands over, then answers about a second more
// without keep-alive (Connection: close) before its port closes, so that a client moves to the
// other instance on a fresh connection instead of losing a request on a reused one. The whole
// stop stays within the 30 s a container stop allows.
func TestAStoppingInstanceOfAPairAnswersWithoutKeepAliveBeforeItCloses(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })
	capture(t, "")
	addr := freeAddr(t)
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan int, 1)
	go func() {
		done <- runServe(ctx, []string{"--addr", addr, "--data", t.TempDir(), "--instance", "a", "--lock", lock, "--log-level", "error"})
	}()
	client := &http.Client{Transport: &http.Transport{Proxy: nil, DisableKeepAlives: true}}
	deadline := time.Now().Add(10 * time.Second)
	for {
		resp, err := client.Get("http://" + addr + "/status")
		if err == nil {
			var st struct{ Role string }
			_ = json.NewDecoder(resp.Body).Decode(&st)
			resp.Body.Close()
			if st.Role == "leader" {
				break
			}
		}
		if time.Now().After(deadline) {
			t.Fatalf("serve did not lead: %v", err)
		}
		time.Sleep(20 * time.Millisecond)
	}

	start := time.Now()
	cancel()
	var closing int // answers with Connection: close after the hand-over
	for {
		// A fresh connection each time, from a client that would keep it alive.
		tr := &http.Transport{Proxy: nil}
		resp, err := (&http.Client{Transport: tr}).Get("http://" + addr + "/livez")
		if err != nil {
			break // the port is closed
		}
		_, _ = io.Copy(io.Discard, resp.Body)
		resp.Body.Close()
		tr.CloseIdleConnections()
		if resp.Close && resp.StatusCode == http.StatusOK {
			closing++
		}
		if time.Since(start) > 30*time.Second {
			t.Fatal("still answering 30 s after the stop")
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Logf("%d answers with Connection: close; the port closed %s after the stop", closing, time.Since(start).Round(time.Millisecond))
	if closing < 5 {
		t.Errorf("%d answers with Connection: close before the port closed, want those of about a second", closing)
	}
	select {
	case code := <-done:
		if code != 0 {
			t.Errorf("serve exited %d", code)
		}
	case <-time.After(30 * time.Second):
		t.Fatal("serve did not stop within 30 s")
	}
	if took := time.Since(start); took > 10*time.Second {
		t.Errorf("stopping took %s with nobody to hand over to", took)
	}
}
