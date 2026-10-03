package server

import (
	"net/http"
	"sync"
	"sync/atomic"

	"github.com/leonieziechmann/betula/radix/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/radix/internal/cortex/telemetry"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
)

// The server's metrics, in telemetry.Registry (GET /metrics). The store and upstream declare
// their own there; cmd/cortex declares cortex_build_info and cortex_start_time_seconds. No
// label is named instance: Prometheus's scrape job sets that one.
var (
	// active is the server the gauges read (a process has one; tests make more, the newest wins).
	active atomic.Pointer[Server]

	requestsTotal = newSourceCounter(telemetry.Registry.NewCounter("cortex_requests_total",
		"Requests to GET /v1/fetch, by source, mode and result: hit, miss, stale, refresh (fetched upstream for these three), stale_if_error, offline_miss, error, forwarded (to the leader), not_modified (If-None-Match). At most 64 sources are named; the ones after are counted as source \"other\".",
		"source", "mode", "result"), maxSources)
	coalescedTotal = telemetry.Registry.NewCounter("cortex_coalesced_total",
		"Fetches that joined an upstream request already running for the same key instead of sending their own.")
	leaderChangesTotal = telemetry.Registry.NewCounter("cortex_leader_changes_total",
		"Changes of this instance's role (it became the leader, or stopped leading).")
	httpRequestsTotal = telemetry.Registry.NewCounter("cortex_http_requests_total",
		"HTTP requests to this instance, by route and status code.", "route", "code")

	// LogProblems counts the log records at WARN and ERROR by level and event; cmd/cortex
	// passes it to oplog.Setup (Options.Problems).
	LogProblems = telemetry.Registry.NewCounter("cortex_log_problems_total",
		"Log records at WARN and ERROR, by level and event.", "level", "event")
)

var (
	fetchModes   = []string{"offline", "cache", "refresh"}
	fetchResults = []string{"hit", "miss", "stale", "refresh", "stale_if_error", "offline_miss", "error", "forwarded", "not_modified"}
	routeNames   = []string{"fetch", "entries", "files", "blobs", "livez", "healthz", "status", "metrics", "step_down", "prune", "journal", "snapshot", "other"}
)

func init() {
	// increase() cannot see the first count of a series that appears in the middle of a time range.
	for _, mode := range fetchModes {
		for _, result := range fetchResults {
			requestsTotal.Add(0, defaultSource, mode, result)
			requestsTotal.Add(0, otherSource, mode, result)
		}
	}
	coalescedTotal.Add(0)
	leaderChangesTotal.Add(0)
	for _, route := range routeNames {
		for _, code := range []string{"200", "404", "500", "502", "503"} {
			httpRequestsTotal.Add(0, route, code)
		}
	}
	for _, p := range []struct{ level, event string }{
		{"ERROR", "service.fatal"}, {"ERROR", "http.request"}, {"ERROR", "http.failed"},
		{"WARN", "http.request"}, {"WARN", "upstream.failed"}, {"WARN", "host.paused"}, {"WARN", "policy.invalid"},
		{"WARN", "blob.rejected"}, {"WARN", "replica.diverged"}, {"WARN", "replica.failed"}, {"WARN", "retention.failed"},
		{"WARN", "leader.lost"}, {"WARN", "leader.campaign_failed"},
	} {
		LogProblems.Add(0, p.level, p.event)
	}

	gauge := func(name, help string, read func(*Server) float64) {
		telemetry.Registry.NewGaugeFunc(name, help, nil, func(emit func(float64, ...string)) {
			if s := active.Load(); s != nil {
				emit(read(s))
			}
		})
	}
	gauge("cortex_role", "1 while this instance leads, 0 while it follows.", func(s *Server) float64 {
		if s.node.Role() == cluster.Leader {
			return 1
		}
		return 0
	})
	gauge("cortex_epoch", "The epoch of the newest entry of the journal: the leader's term.", func(s *Server) float64 {
		_, epoch := s.st.Position()
		return float64(epoch)
	})
	gauge("cortex_journal_seq", "The sequence number of the newest entry of the journal.", func(s *Server) float64 {
		seq, _ := s.st.Position()
		return float64(seq)
	})
	gauge("cortex_replication_lag_seconds", "How far the follower is behind its leader: the age of the oldest change it has not applied (0 on the leader and when caught up).", func(s *Server) float64 {
		return s.node.Follower().LagSeconds
	})
	gauge("cortex_replication_lag_entries", "Journal entries of the leader the follower has not applied (0 on the leader).", func(s *Server) float64 {
		return float64(s.node.Follower().LagEntries)
	})
	gauge("cortex_blobs_missing", "Blobs the index references that this instance does not have yet (a follower after a snapshot, or a leader promoted while it fetched them); 48-cortex.sh hands over only at 0.", func(s *Server) float64 {
		return float64(s.node.Follower().BlobsMissing)
	})
}

// maxSources is how many values the source label of cortex_requests_total takes at most, beside
// otherSource.
const maxSources = 64

// otherSource is the source label of the requests whose source came after the first maxSources.
const otherSource = "other"

// sourceCounter is a counter whose first label is a client's ?source=, which anyone may choose:
// a client that puts a job id or a timestamp there would add series without end (memory, the
// size of every scrape, Prometheus), as the host label would without its bound. The first
// maxSources values seen keep their name; the rest are counted as otherSource. The request log
// and the entries keep the full source.
type sourceCounter struct {
	c   *metrics.Counter
	max int

	mu   sync.Mutex
	seen map[string]bool
}

func newSourceCounter(c *metrics.Counter, max int) *sourceCounter {
	return &sourceCounter{c: c, max: max, seen: make(map[string]bool)}
}

// label is the label value of source.
func (s *sourceCounter) label(source string) string {
	if source == otherSource {
		return source
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.seen[source] {
		return source
	}
	if len(s.seen) >= s.max {
		return otherSource
	}
	s.seen[source] = true
	return source
}

// Add adds v to the series of source, mode and result.
func (s *sourceCounter) Add(v float64, source, mode, result string) {
	s.c.Add(v, s.label(source), mode, result)
}

// Inc adds one.
func (s *sourceCounter) Inc(source, mode, result string) { s.Add(1, source, mode, result) }

// handleMetrics serves telemetry.Registry: Cortex's metrics only, never Radix's.
func (s *Server) handleMetrics(w http.ResponseWriter, r *http.Request) {
	telemetry.Registry.Handler().ServeHTTP(w, r)
}
