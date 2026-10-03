package server

import (
	"bytes"
	"context"
	"encoding/json"
	"log/slog"
	"net"
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/internal/cortex/store"
	"github.com/leonieziechmann/betula/internal/cortex/upstream"
)

// statusOf reads GET /status with json.Number, so that integers can be told from fractions.
func statusOf(t *testing.T, c *testCortex) map[string]any {
	t.Helper()
	resp, body := get(t, c.URL+"/status")
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Content-Type") != "application/json" {
		t.Fatalf("/status: %d %s", resp.StatusCode, body)
	}
	dec := json.NewDecoder(strings.NewReader(body))
	dec.UseNumber()
	var doc map[string]any
	if err := dec.Decode(&doc); err != nil {
		t.Fatalf("/status: %v\n%s", err, body)
	}
	if dec.More() {
		t.Fatalf("/status holds more than one JSON document")
	}
	return doc
}

func isInteger(v any) bool {
	n, ok := v.(json.Number)
	if !ok {
		return false
	}
	_, err := n.Int64()
	return err == nil
}

func TestStatusDescribesTheLeader(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	// The status lists every host the policy names, idle or not (and others only while they
	// have work, a pause or failures).
	c := newTestCortex(t, withPolicy(`{"default": {"max_age": "1h", "queue_wait": "10s", "breaker_failures": 100},
		"hosts": {"127.0.0.1": {"pause": "0s"}}}`))
	get(t, c.fetchURL(host.url("/p")))
	// /status serves the numbers last counted and never waits for a count (E2E-5): count now.
	if _, err := c.st.RefreshStats(); err != nil {
		t.Fatal(err)
	}

	doc := statusOf(t, c)
	follower, _ := doc["follower"].(map[string]any)
	leader, _ := doc["leader"].(map[string]any)
	st, _ := doc["store"].(map[string]any)
	if doc["instance"] != "a" || doc["role"] != "leader" || doc["epoch"] != json.Number("1") || doc["seq"] != json.Number("2") ||
		!isInteger(doc["epoch"]) || !isInteger(doc["seq"]) || doc["version"] != "test" || doc["started_at"] != "2026-10-02T10:00:00.000000Z" {
		t.Errorf("status: %v", doc)
	}
	if follower == nil || follower["lag_seconds"] != json.Number("0") || follower["state"] != "" {
		t.Errorf("follower on the leader: %v", doc["follower"])
	}
	if leader == nil || leader["instance"] != "a" || st == nil || st["entries"] != json.Number("1") || doc["healthy"] != true {
		t.Errorf("leader %v, store %v, healthy %v", leader, st, doc["healthy"])
	}
	hosts, _ := doc["hosts"].([]any)
	if len(hosts) != 1 || hosts[0].(map[string]any)["host"] != "127.0.0.1" {
		t.Errorf("hosts: %v", doc["hosts"])
	}
	if _, ok := doc["last_prune"]; !ok {
		t.Error("last_prune missing")
	}
}

func TestStatusDescribesAFollower(t *testing.T) {
	node := newFollowerNode("b", &cluster.Info{Instance: "a", URL: "http://cortex_a:8100", Epoch: 3, Since: t0})
	node.setFollower(cluster.FollowerState{State: "catching_up", LeaderURL: "http://cortex_a:8100", LagSeconds: 12.5, LagEntries: 40})
	c := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	doc := statusOf(t, c)
	follower, _ := doc["follower"].(map[string]any)
	leader, _ := doc["leader"].(map[string]any)
	if doc["role"] != "follower" || follower["lag_seconds"] != json.Number("12.5") || follower["lag_entries"] != json.Number("40") ||
		follower["state"] != "catching_up" || leader["url"] != "http://cortex_a:8100" || leader["since"] != "2026-10-02T10:00:00.000000Z" {
		t.Errorf("status: %v", doc)
	}
	if !isInteger(doc["epoch"]) || !isInteger(doc["seq"]) {
		t.Errorf("epoch %v, seq %v: want integers", doc["epoch"], doc["seq"])
	}
}

func TestHealthReportsTheProblemsOfThePair(t *testing.T) {
	c := newTestCortex(t)
	resp, body := get(t, c.URL+"/healthz")
	if resp.StatusCode != http.StatusOK || strings.TrimSpace(body) != "{\n  \"role\": \"leader\",\n  \"status\": \"ok\"\n}" {
		t.Errorf("healthz: %d %s", resp.StatusCode, body)
	}
	if resp, body := get(t, c.URL+"/livez"); resp.StatusCode != http.StatusOK || !strings.Contains(body, `"ok"`) {
		t.Errorf("livez: %d %s", resp.StatusCode, body)
	}

	node := newFollowerNode("b", &cluster.Info{Instance: "a", URL: "http://127.0.0.1:1"})
	f := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return node }))
	if resp, _ := get(t, f.URL+"/healthz"); resp.StatusCode != http.StatusOK {
		t.Errorf("a follower in step: %d", resp.StatusCode)
	}
	node.setFollower(cluster.FollowerState{State: "following", LagSeconds: 400, LagEntries: 9})
	resp, body = get(t, f.URL+"/healthz")
	if resp.StatusCode != http.StatusServiceUnavailable || !strings.Contains(body, "400 s behind") || !strings.Contains(body, `"unhealthy"`) {
		t.Errorf("a follower 400 s behind: %d %s", resp.StatusCode, body)
	}

	lost := newFollowerNode("b", nil)
	l := newTestCortex(t, withNode(func(*store.Store) cluster.Node { return lost }))
	if resp, _ := get(t, l.URL+"/healthz"); resp.StatusCode != http.StatusOK {
		t.Errorf("no leader for a moment: %d", resp.StatusCode)
	}
	l.clock.advance(61 * time.Second)
	resp, body = get(t, l.URL+"/healthz")
	if resp.StatusCode != http.StatusServiceUnavailable || !strings.Contains(body, "no leader known since") {
		t.Errorf("no leader for a minute: %d %s", resp.StatusCode, body)
	}

	_ = c.st.Close()
	for _, path := range []string{"/livez", "/healthz"} {
		if resp, body := get(t, c.URL+path); resp.StatusCode != http.StatusServiceUnavailable || !strings.Contains(body, "the index does not answer") {
			t.Errorf("%s with a closed index: %d %s", path, resp.StatusCode, body)
		}
	}
}

func TestMetricsAreCortexsOwn(t *testing.T) {
	c := newTestCortex(t)
	// The duration histogram has no series before the first request upstream: one fetch, so the
	// test does not depend on the tests that ran before it.
	h := newFakeHost(t)
	h.set("/page", page{status: http.StatusOK, body: "content"})
	if resp, body := get(t, c.fetchURL(h.url("/page"))); resp.StatusCode != http.StatusOK {
		t.Fatalf("fetch: %d %s", resp.StatusCode, body)
	}
	// The store's gauges have no sample before the first count, which a scrape never waits for.
	if _, err := c.st.RefreshStats(); err != nil {
		t.Fatal(err)
	}
	resp, body := get(t, c.URL+"/metrics")
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("/metrics: %d", resp.StatusCode)
	}
	for _, family := range []string{
		"cortex_requests_total", "cortex_coalesced_total", "cortex_role", "cortex_epoch", "cortex_journal_seq",
		"cortex_replication_lag_seconds", "cortex_replication_lag_entries", "cortex_blobs_missing", "cortex_leader_changes_total",
		"cortex_http_requests_total", "cortex_log_problems_total", "cortex_upstream_requests_total",
		"cortex_upstream_request_duration_seconds", "cortex_upstream_bytes_total", "cortex_host_queue", "cortex_host_in_flight",
		"cortex_host_paused", "cortex_entries", "cortex_versions", "cortex_files", "cortex_blobs", "cortex_blob_bytes",
		"cortex_blob_original_bytes", "cortex_pruned_total",
	} {
		if !strings.Contains(body, "\n# TYPE "+family+" ") {
			t.Errorf("no family %s", family)
		}
	}
	if strings.Contains(body, "radix_") || strings.Contains(body, `instance="`) {
		t.Errorf("/metrics shows Radix's metrics or an instance label:\n%s", body)
	}
	m := scrape(t, c)
	if m["cortex_role"] != 1 || m["cortex_epoch"] != 1 || m["cortex_journal_seq"] != 2 { // the epoch, then the fetch
		t.Errorf("role %v, epoch %v, seq %v", m["cortex_role"], m["cortex_epoch"], m["cortex_journal_seq"])
	}
	if m[`cortex_http_requests_total{route="metrics",code="200"}`] < 1 {
		t.Error("the scrape itself was not counted")
	}
}

func TestUnknownPathsAndMethodsAreCortexErrors(t *testing.T) {
	c := newTestCortex(t)
	resp, body := get(t, c.URL+"/v2/nothing")
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	resp, body = do(t, http.MethodPost, c.URL+"/v1/fetch?url=https%3A%2F%2Fexample.org%2F", nil)
	wantStatus(t, resp, body, http.StatusMethodNotAllowed, codeMethodNotAllowed)
	if resp.Header.Get("Allow") != "GET, HEAD" || resp.Header.Get("Cortex-Instance") != "a; role=leader" {
		t.Errorf("Allow %q, Cortex-Instance %q", resp.Header.Get("Allow"), resp.Header.Get("Cortex-Instance"))
	}
	resp, body = get(t, c.URL+"/v1/admin/prune")
	wantStatus(t, resp, body, http.StatusMethodNotAllowed, codeMethodNotAllowed)
}

func TestPruneRemovesWhatWasSupersededLongAgo(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "old"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))
	c.clock.advance(time.Hour)
	host.set("/p", page{body: "new"})
	get(t, c.fetchURL(host.url("/p"), "mode", "refresh"))

	c.clock.advance(179 * 24 * time.Hour)
	resp, body := do(t, http.MethodPost, c.URL+"/v1/admin/prune", nil)
	var report retentionReport
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &report) != nil || report.Prune == nil || report.Prune.Versions != 0 {
		t.Fatalf("prune within 180 days: %d %s", resp.StatusCode, body)
	}
	c.clock.advance(2 * 24 * time.Hour)
	resp, body = do(t, http.MethodPost, c.URL+"/v1/admin/prune", nil)
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &report) != nil || report.Prune == nil ||
		report.Prune.Versions != 1 || report.PruneSeq == 0 {
		t.Fatalf("prune after 180 days: %d %s", resp.StatusCode, body)
	}
	if n := versionsOf(t, c, host.url("/p")); n != 1 {
		t.Errorf("%d versions after the prune, want the current one", n)
	}
	if doc := statusOf(t, c); doc["last_prune"] == nil {
		t.Error("/status has no last_prune")
	}
}

func TestRetentionRunsByItself(t *testing.T) {
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	up, err := upstream.New(upstream.Options{AllowPrivate: true})
	if err != nil {
		t.Fatal(err)
	}
	defer up.Close()
	node, err := cluster.NewSingle(st, cluster.Info{Instance: "a"})
	if err != nil {
		t.Fatal(err)
	}
	srv, err := New(Options{Store: st, Upstream: up, Node: node, PruneInterval: 20 * time.Millisecond})
	if err != nil {
		t.Fatal(err)
	}
	waitFor(t, "a retention run", func() bool { return srv.Status(context.Background()).LastPrune != nil })
	if err := srv.Close(); err != nil {
		t.Fatal(err)
	}
	if report := srv.Status(context.Background()).LastPrune; report.Prune == nil || len(report.Errors) != 0 {
		t.Errorf("report %+v", report)
	}
}

func TestServeShutsDownWithoutWaitingForLongPolls(t *testing.T) {
	c := newTestCortex(t)
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	served := make(chan error, 1)
	go func() { served <- c.srv.Serve(ctx, ln) }()

	seq, _ := c.st.Position()
	polled := make(chan int, 1)
	go func() {
		resp, err := http.Get("http://" + ln.Addr().String() + "/internal/v1/journal?wait=30s&after=" + itoa(seq))
		if err != nil {
			polled <- 0
			return
		}
		resp.Body.Close()
		polled <- resp.StatusCode
	}()
	time.Sleep(200 * time.Millisecond)
	start := time.Now()
	cancel()
	if err := <-served; err != nil {
		t.Errorf("Serve: %v", err)
	}
	if took := time.Since(start); took > 3*time.Second {
		t.Errorf("shutdown took %s", took)
	}
	if status := <-polled; status != http.StatusOK {
		t.Errorf("the long poll got %d, want an empty 200", status)
	}
}

func TestTheRequestLogAndTheFetchLog(t *testing.T) {
	var out bytes.Buffer
	previous := slog.Default()
	slog.SetDefault(slog.New(slog.NewJSONHandler(&out, &slog.HandlerOptions{Level: slog.LevelDebug})))
	t.Cleanup(func() { slog.SetDefault(previous) })

	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	host.set("/down", page{status: http.StatusBadGateway})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p"), "source", "module_page"))
	get(t, c.fetchURL(host.url("/down")))
	get(t, c.URL+"/healthz")

	var fetched, failed, request, failedRequest, health map[string]any
	for _, line := range strings.Split(strings.TrimSpace(out.String()), "\n") {
		var rec map[string]any
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			t.Fatalf("log line %q: %v", line, err)
		}
		switch {
		case rec["event"] == "upstream.fetched":
			fetched = rec
		case rec["event"] == "upstream.failed":
			failed = rec
		case rec["event"] == "http.request" && rec["route"] == "fetch" && rec["status"] == 200.0:
			request = rec
		case rec["event"] == "http.request" && rec["route"] == "fetch":
			failedRequest = rec
		case rec["event"] == "http.request" && rec["route"] == "healthz":
			health = rec
		}
	}
	if fetched == nil || fetched["component"] != "upstream" || fetched["url"] != host.url("/p") || fetched["status"] != 200.0 ||
		fetched["changed"] != true || fetched["bytes"] != 1.0 || fetched["source"] != "module_page" {
		t.Errorf("upstream.fetched: %v", fetched)
	}
	if failed == nil || failed["level"] != "WARN" || failed["code"] != codeUpstreamFailed {
		t.Errorf("upstream.failed: %v", failed)
	}
	if request == nil || request["level"] != "INFO" || request["result"] != "miss" || request["component"] != "http" {
		t.Errorf("http.request of a fetch: %v", request)
	}
	if failedRequest == nil || failedRequest["level"] != "WARN" || failedRequest["error"] != codeUpstreamFailed {
		t.Errorf("http.request of a failed fetch: %v", failedRequest)
	}
	if health == nil || health["level"] != "DEBUG" {
		t.Errorf("http.request of /healthz: %v", health)
	}
}
