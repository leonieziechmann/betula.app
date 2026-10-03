package server

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
	"github.com/leonieziechmann/betula/internal/oplog"
)

// Limits of the journal's long poll.
const (
	journalBatch   = 1000
	journalMaxWait = 30 * time.Second
)

// handleJournal is GET /internal/v1/journal?after=<seq>&epoch=<e>&sum=<hex>&wait=<dur>, the
// leader's side of the replication: up to 1000 journal entries after after, as NDJSON. When
// there is none yet it waits up to wait (at most 30 s) for the first.
//
// The follower names its newest entry: after (its seq), epoch and sum (store.JournalEntry.Sum,
// from store.LastSum); epoch and sum are optional, and each one given must match. The leader
// decides against its database (store.CheckJournal): 409 diverged when its entry at after is
// another one or it has none there (also at OldestSeq-1, whose trimmed entry it still knows),
// 410 trimmed when its journal no longer reaches back to after. Both take a snapshot. A
// follower answers 409 not-leader.
//
// The answer names the head the page was read up to: Cortex-Head-Seq, Cortex-Head-Epoch and
// Cortex-Head-At are the leader's newest entry when the page was read, and the page holds every
// entry from after+1 to that head unless the page limits (1000 entries, about 8 MiB) cut it
// short. So a follower that has applied the page and stands at Cortex-Head-Seq was current
// when the answer was made (the Date header); it is behind by Cortex-Head-Seq minus its seq
// otherwise (E2E-6). The page never goes past the head it names (E2E-2).
func (s *Server) handleJournal(w http.ResponseWriter, r *http.Request) {
	if !s.leading() {
		writeError(w, http.StatusConflict, codeNotLeader, "this instance follows; ask the leader")
		return
	}
	q := r.URL.Query()
	after, err := intParam(q.Get("after"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, "after: "+err.Error())
		return
	}
	epoch, err := intParam(q.Get("epoch"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, "epoch: "+err.Error())
		return
	}
	if !q.Has("epoch") {
		epoch = -1 // not given: not compared
	}
	sum := q.Get("sum")
	if sum != "" && !store.ValidHash(sum) {
		writeError(w, http.StatusBadRequest, codeBadRequest, "sum "+strconv.Quote(sum)+": 64 lower-case hex digits")
		return
	}
	var wait time.Duration
	if v := q.Get("wait"); v != "" {
		if wait, err = time.ParseDuration(v); err != nil || wait < 0 {
			writeError(w, http.StatusBadRequest, codeBadRequest, "wait "+strconv.Quote(v)+": a Go duration of 0 or more")
			return
		}
		wait = min(wait, journalMaxWait)
	}

	switch verdict, why, err := s.st.CheckJournal(after, epoch, sum); {
	case err != nil:
		writeInternal(w, "journal", err)
		return
	case verdict == store.Diverged:
		writeError(w, http.StatusConflict, codeDiverged, why)
		return
	case verdict == store.Trimmed:
		writeError(w, http.StatusGone, codeTrimmed, why)
		return
	}

	// The follower's entry is in the database; the cached head includes it once its writer
	// has moved it, which CommittedHead waits for.
	head, headEpoch, headAt := s.st.Head()
	if after > head {
		head, headEpoch, headAt = s.st.CommittedHead()
	}
	entries, err := s.st.JournalBetween(after, head, journalBatch)
	if err == nil && len(entries) == 0 && wait > 0 {
		ctx, cancel := context.WithTimeout(r.Context(), wait)
		stop := context.AfterFunc(s.draining, cancel)
		werr := s.st.WaitAfter(ctx, after)
		stop()
		cancel()
		if werr == nil {
			head, headEpoch, headAt = s.st.Head()
			entries, err = s.st.JournalBetween(after, head, journalBatch)
		}
	}
	if err != nil {
		writeInternal(w, "journal", err)
		return
	}

	h := w.Header()
	h.Set("Content-Type", "application/x-ndjson")
	h.Set("Cache-Control", "no-store")
	h.Set("Cortex-Head-Seq", strconv.FormatInt(head, 10))
	h.Set("Cortex-Head-Epoch", strconv.FormatInt(headEpoch, 10))
	if !headAt.IsZero() {
		h.Set("Cortex-Head-At", store.FormatTime(headAt))
	}
	w.WriteHeader(http.StatusOK)
	enc := json.NewEncoder(w) // one entry per line
	for _, e := range entries {
		if err := enc.Encode(e); err != nil {
			return
		}
	}
	_ = http.NewResponseController(w).Flush()
}

// intParam reads a whole number of 0 or more; "" is 0.
func intParam(v string) (int64, error) {
	if v == "" {
		return 0, nil
	}
	n, err := strconv.ParseInt(v, 10, 64)
	if err != nil || n < 0 {
		return 0, fmt.Errorf("%q: a whole number of 0 or more", v)
	}
	return n, nil
}

// handleSnapshot is GET /internal/v1/snapshot: the leader's index as an SQLite file, with the
// journal position it holds in Cortex-Seq and Cortex-Epoch. The copy is made in DIR/tmp first
// (the position is known only when it is done) and sent from there: one copy on disk, and one
// snapshot at a time (store.OpenSnapshot); a second request waits for the first. A client that
// goes away ends its copy, also while it waits or the VACUUM runs. HEAD answers the current
// position without a copy. A follower answers 409 not-leader.
//
// Log events: snapshot.sent, snapshot.failed (WARN).
func (s *Server) handleSnapshot(w http.ResponseWriter, r *http.Request) {
	if !s.leading() {
		writeError(w, http.StatusConflict, codeNotLeader, "this instance follows; ask the leader")
		return
	}
	h := w.Header()
	if r.Method == http.MethodHead {
		seq, epoch := s.st.Position()
		h.Set("Content-Type", "application/vnd.sqlite3")
		h.Set("Cache-Control", "no-store")
		h.Set("Cortex-Seq", strconv.FormatInt(seq, 10))
		h.Set("Cortex-Epoch", strconv.FormatInt(epoch, 10))
		w.WriteHeader(http.StatusOK)
		return
	}
	log := oplog.For("cortex")
	start := time.Now()
	f, err := s.st.OpenSnapshot(r.Context())
	if err != nil {
		if r.Context().Err() != nil {
			log.Info("snapshot abandoned", "event", "snapshot.failed", "remote", r.RemoteAddr, oplog.Err(err))
			return // nobody to answer
		}
		log.Warn("snapshot failed", "event", "snapshot.failed", "remote", r.RemoteAddr, oplog.Err(err))
		writeInternal(w, "snapshot", err)
		return
	}
	defer f.Close()
	h.Set("Content-Type", "application/vnd.sqlite3")
	h.Set("Cache-Control", "no-store")
	h.Set("Content-Length", strconv.FormatInt(f.Size, 10))
	h.Set("Cortex-Seq", strconv.FormatInt(f.Seq, 10))
	h.Set("Cortex-Epoch", strconv.FormatInt(f.Epoch, 10))
	w.WriteHeader(http.StatusOK)
	n, err := io.Copy(w, f.File)
	if err != nil {
		log.Warn("snapshot not sent", "event", "snapshot.failed", "remote", r.RemoteAddr, "seq", f.Seq, "bytes", n, oplog.Err(err))
		return
	}
	log.Info("snapshot sent", "event", "snapshot.sent", "remote", r.RemoteAddr, "seq", f.Seq, "epoch", f.Epoch,
		"bytes", n, "duration_ms", time.Since(start).Milliseconds())
}
