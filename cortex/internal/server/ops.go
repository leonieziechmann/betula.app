package server

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/cluster"
	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/store"
	"github.com/leonieziechmann/betula/cortex/internal/upstream"
)

// livezTimeout bounds the trivial query of GET /livez.
const livezTimeout = 2 * time.Second

// ping runs the store's trivial query, bounded by ctx even when the index is locked for a swap.
func (s *Server) ping(ctx context.Context) error {
	errc := make(chan error, 1)
	go func() { errc <- s.st.Ping(ctx) }()
	select {
	case err := <-errc:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

// handleLivez is GET /livez, for the container's health check: 200 while the process serves
// and the index answers a trivial query within 2 s, else 503.
func (s *Server) handleLivez(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), livezTimeout)
	defer cancel()
	if err := s.ping(ctx); err != nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]any{"status": "unhealthy", "problems": []string{"the index does not answer: " + err.Error()}})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"status": "ok"})
}

// problems are what makes the instance unhealthy: an index that does not answer, no leader
// known for a minute, a follower more than 5 minutes behind, a follower that lacks blobs its
// index references (it fetches them from the leader, e.g. after a snapshot; until then it
// would answer for files it cannot read if it took over).
func (s *Server) problems(ctx context.Context) []string {
	problems := []string{}
	pctx, cancel := context.WithTimeout(ctx, livezTimeout)
	defer cancel()
	if err := s.ping(pctx); err != nil {
		problems = append(problems, "the index does not answer: "+err.Error())
	}
	if since := s.checkLeader(); !since.IsZero() && s.now().Sub(since) >= noLeaderAfter {
		problems = append(problems, fmt.Sprintf("no leader known since %s", store.FormatTime(since)))
	}
	if f := s.node.Follower(); s.node.Role() == cluster.Follower {
		if f.LagSeconds > maxLagSeconds {
			problems = append(problems, fmt.Sprintf("the follower is %.0f s behind its leader (%d entries)", f.LagSeconds, f.LagEntries))
		}
		if f.BlobsMissing > 0 {
			problems = append(problems, fmt.Sprintf("the follower lacks %d blobs its index references (fetching them from the leader)", f.BlobsMissing))
		}
	} else if f.BlobsMissing > 0 { // promoted while it was still back-filling
		problems = append(problems, fmt.Sprintf("the leader lacks %d blobs its index references (fetching them from the other instance)", f.BlobsMissing))
	}
	return problems
}

// handleHealthz is GET /healthz: 200 {"status":"ok","role":…} or 503
// {"status":"unhealthy","role":…,"problems":[…]}.
func (s *Server) handleHealthz(w http.ResponseWriter, r *http.Request) {
	role := roleName(s.node.Role())
	if problems := s.problems(r.Context()); len(problems) > 0 {
		writeJSON(w, http.StatusServiceUnavailable, map[string]any{"status": "unhealthy", "role": role, "problems": problems})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"status": "ok", "role": role})
}

// Status is what GET /status says about the instance. `cortex status` prints it; the deploy
// scripts read role, epoch, seq and follower (state, lag_seconds, lag_entries, blobs_missing:
// a hand-over goes only to a follower that is "following" with 0 and 0).
type Status struct {
	Instance   string                `json:"instance"`
	Role       string                `json:"role"`  // leader or follower
	Epoch      int64                 `json:"epoch"` // of the newest journal entry
	Seq        int64                 `json:"seq"`   // the newest journal entry
	OldestSeq  int64                 `json:"oldest_seq"`
	HeadAt     string                `json:"head_at,omitempty"`
	Leader     *LeaderStatus         `json:"leader"` // null when no leader is known
	Follower   cluster.FollowerState `json:"follower"`
	Healthy    bool                  `json:"healthy"`
	Problems   []string              `json:"problems"`
	Hosts      []upstream.HostState  `json:"hosts"`
	Store      *store.Stats          `json:"store"`
	StoreError string                `json:"store_error,omitempty"`
	LastPrune  *retentionReport      `json:"last_prune"`
	Version    string                `json:"version"`
	Build      string                `json:"build,omitempty"`
	StartedAt  string                `json:"started_at"`
	Log        *oplog.Summary        `json:"log,omitempty"`
}

// LeaderStatus names the leader.
type LeaderStatus struct {
	Instance string `json:"instance"`
	URL      string `json:"url"`
	Epoch    int64  `json:"epoch,omitempty"`
	Since    string `json:"since,omitempty"`
}

// Status describes the instance now.
func (s *Server) Status(ctx context.Context) Status {
	seq, epoch, at := s.st.Head()
	st := Status{
		Instance:  s.node.Self().Instance,
		Role:      roleName(s.node.Role()),
		Epoch:     epoch,
		Seq:       seq,
		OldestSeq: s.st.OldestSeq(),
		HeadAt:    formatOptional(at),
		Follower:  s.node.Follower(),
		Problems:  s.problems(ctx),
		Hosts:     s.up.HostStates(),
		Version:   s.opt.Version,
		Build:     s.opt.Build,
		StartedAt: store.FormatTime(s.started),
	}
	st.Healthy = len(st.Problems) == 0
	if leader, ok := s.node.Leader(); ok {
		st.Leader = &LeaderStatus{Instance: leader.Instance, URL: leader.URL, Epoch: leader.Epoch, Since: formatOptional(leader.Since)}
	}
	// RecentStats never waits for the walk over the blobs, which takes seconds in a large store
	// (E2E-5): the last numbers, or store_error until the first count is done.
	if stats, err := s.st.RecentStats(); err != nil {
		st.StoreError = err.Error()
	} else {
		st.Store = &stats
	}
	s.mu.Lock()
	if s.lastRetention != nil {
		report := *s.lastRetention
		st.LastPrune = &report
	}
	s.mu.Unlock()
	if s.opt.Recorder != nil {
		sum := s.opt.Recorder.Summary(20)
		st.Log = &sum
	}
	return st
}

// handleStatus is GET /status.
func (s *Server) handleStatus(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, s.Status(r.Context()))
}

// handleStepDown is POST /v1/admin/step-down: the leader resigns and stays a follower for
// 15 s, so that the other instance takes over (a rolling update). 409 not-leader on a
// follower; 409 cannot-step-down when there is nobody to hand over to (a single instance).
func (s *Server) handleStepDown(w http.ResponseWriter, r *http.Request) {
	if !s.leading() {
		writeError(w, http.StatusConflict, codeNotLeader, "this instance follows")
		return
	}
	err := s.node.StepDown()
	w.Header().Set("Cortex-Instance", s.instanceHeader())
	switch {
	case errors.Is(err, cluster.ErrNotLeader):
		writeError(w, http.StatusConflict, codeNotLeader, "this instance follows")
	case err != nil:
		writeError(w, http.StatusConflict, codeCannotStepDown, err.Error())
	default:
		writeJSON(w, http.StatusOK, map[string]any{"status": "stepped down", "instance": s.node.Self().Instance, "role": roleName(s.node.Role())})
	}
}

// retentionReport is what one run of retention did (GET /status last_prune, POST
// /v1/admin/prune).
type retentionReport struct {
	At               string            `json:"at"`
	Prune            *store.PruneStats `json:"prune"` // null when this instance did not lead (a follower gets prune through the journal)
	PruneSeq         int64             `json:"prune_seq,omitempty"`
	JournalTrimmed   int               `json:"journal_trimmed"`
	BlobsRemoved     int               `json:"blobs_removed"`
	BlobBytesRemoved int64             `json:"blob_bytes_removed"`
	DurationMS       int64             `json:"duration_ms"`
	Errors           []string          `json:"errors,omitempty"`
}

// retention runs the retention once: Prune (in the write fence, while leading), TrimJournal
// and GCBlobs (on both roles). The store logs prune.finished, journal.trimmed and gc.finished.
// pruned says whether Prune ran.
//
// Log events: retention.failed (WARN).
func (s *Server) retention() (report retentionReport, pruned bool) {
	s.maintMu.Lock()
	defer s.maintMu.Unlock()
	start := time.Now()
	now := s.now()
	report.At = store.FormatTime(now)
	log := oplog.For("cortex")
	failed := func(step string, err error) {
		report.Errors = append(report.Errors, step+": "+err.Error())
		log.Warn("retention failed", "event", "retention.failed", "step", step, oplog.Err(err))
	}

	if s.leading() {
		if done, ok := s.node.BeginWrite(); ok {
			stats, e, err := s.st.Prune(now, s.opt.History)
			done()
			if err != nil {
				failed("prune", err)
			} else {
				report.Prune, report.PruneSeq, pruned = &stats, e.Seq, true
			}
		}
	}
	if n, err := s.st.TrimJournal(now.Add(-s.opt.JournalKeep)); err != nil {
		failed("trim", err)
	} else {
		report.JournalTrimmed = n
	}
	if n, bytes, err := s.st.GCBlobs(s.opt.BlobGrace, now); err != nil {
		failed("gc", err)
	} else {
		report.BlobsRemoved, report.BlobBytesRemoved = n, bytes
	}
	report.DurationMS = time.Since(start).Milliseconds()
	// Retention changed the counts; /status and /metrics keep serving the old ones until the
	// count that this starts in the background is done (at start: the first count).
	s.st.InvalidateStats()
	_, _ = s.st.RecentStats()
	s.mu.Lock()
	s.lastRetention = &report
	s.mu.Unlock()
	return report, pruned
}

// handlePrune is POST /v1/admin/prune: the retention now, on the leader (a follower forwards
// it); its report as JSON.
func (s *Server) handlePrune(w http.ResponseWriter, r *http.Request) {
	if !s.leading() && s.forward(w, r) {
		return
	}
	report, pruned := s.retention()
	if !pruned && len(report.Errors) == 0 {
		s.noLeader(w, r, "this instance stopped leading before the prune")
		return
	}
	if len(report.Errors) > 0 {
		writeError(w, http.StatusInternalServerError, codeInternal, strings.Join(report.Errors, "; "))
		return
	}
	writeJSON(w, http.StatusOK, report)
}

// maintenance runs the retention at start and every interval until Close.
func (s *Server) maintenance(every time.Duration) {
	defer s.wg.Done()
	t := time.NewTicker(every)
	defer t.Stop()
	for {
		if s.ctx.Err() != nil {
			return
		}
		s.retention()
		select {
		case <-s.ctx.Done():
			return
		case <-t.C:
		}
	}
}

// watchRole counts the role changes of the node (cortex_leader_changes_total) and keeps track
// of how long no leader has been known, until Close.
func (s *Server) watchRole() {
	defer s.wg.Done()
	t := time.NewTicker(time.Second)
	defer t.Stop()
	for {
		changes := s.node.LeaderChanges()
		s.checkLeader()
		select {
		case <-s.ctx.Done():
			return
		case <-changes:
			leaderChangesTotal.Inc()
		case <-t.C:
		}
	}
}

// checkLeader notes whether a leader is known and returns since when none is (zero while one
// is).
func (s *Server) checkLeader() time.Time {
	_, ok := s.node.Leader()
	s.mu.Lock()
	defer s.mu.Unlock()
	switch {
	case ok:
		s.noLeaderSince = time.Time{}
	case s.noLeaderSince.IsZero():
		s.noLeaderSince = s.now()
	}
	return s.noLeaderSince
}
