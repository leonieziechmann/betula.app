package cluster

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

// Timing of the follower.
const (
	minBackoff = 250 * time.Millisecond // after a failed round, doubled up to maxBackoff
	maxBackoff = 5 * time.Second
	// journalSlack is how much longer than the long poll's wait the journal's answer may take
	// to begin.
	journalSlack = 15 * time.Second
	// blobHeaderWait and snapshotHeaderWait bound the wait for the answer to begin; the leader
	// copies its whole index before it answers a snapshot.
	blobHeaderWait     = 30 * time.Second
	snapshotHeaderWait = 5 * time.Minute
	// idleWait ends a transfer that has not moved for so long.
	idleWait = time.Minute
	// backfillWorkers is how many blobs a back-fill fetches at a time: the leader serves
	// clients meanwhile, but a follower that lacks blobs is a hot spare that cannot take over
	// cleanly, so the window is kept short (2 workers fetched about 440 blobs/s on loopback). The blobs of the journal's pages, which the follower needs before it
	// can go on, are fetched by minBlobWorkers to maxBlobWorkers at a time: more while every
	// fetch succeeds and a page has more blobs than workers, fewer after a failure (E2E-3).
	backfillWorkers = 8
	minBlobWorkers  = 4
	maxBlobWorkers  = 16
	// applyChunk is how many entries one transaction applies (store.ApplyBatch): one commit
	// for many entries, and a follower that is stopped stops between two chunks (E2E-11).
	applyChunk = 250
	// failureLogEvery throttles replica.failed while the failures go on.
	failureLogEvery = time.Minute
	// followingBelow, resumeBelow and behindLogAfter: a follower is "following" while its lag
	// is under followingBelow; once it reached it, again only under resumeBelow (hysteresis),
	// and replica.caught_up is logged only after a lag of behindLogAfter (E2E-6, D3: the state
	// flipped and the line was logged on almost every page under a steady load).
	followingBelow = time.Second
	resumeBelow    = 500 * time.Millisecond
	behindLogAfter = time.Second
)

// errBlobAbsent is the answer of the instance asked for a blob that it does not have either.
var errBlobAbsent = errors.New("the leader does not have the blob")

// replica is one stretch of following: the state of the follower loop.
type replica struct {
	p     *Peer
	quiet bool // a new leader's catch-up: no replica.* log lines of its own

	// source is the instance the back-fill reads from: the announced leader, or for a leader
	// the other instance (asLeader: its requests say they come from a leader, so that a
	// follower that lacks the blob too does not hand them back to it).
	source   func() (Info, bool)
	asLeader bool

	leader   Info // the leader of the last round that reached it
	contact  bool // the last round reached it
	failures int
	logged   time.Time // of the last replica.failed

	// resnapshot is the pause before the next snapshot while snapshots do not help (the
	// index diverges again before any entry applies); zero after progress.
	resnapshot time.Duration

	workers atomic.Int32 // blob fetches at a time for the journal's pages

	backfillReq, countReq chan struct{}
	absentMu              sync.Mutex
	absent                map[string]bool // blobs the leader does not have either
}

func newReplica(p *Peer, quiet bool) *replica {
	r := &replica{p: p, quiet: quiet, source: p.announced, backfillReq: make(chan struct{}, 1), countReq: make(chan struct{}, 1),
		absent: make(map[string]bool)}
	r.workers.Store(minBlobWorkers)
	return r
}

// follow keeps the store a copy of the leader's until ctx ends: journal entries in order
// (each blob an entry names first), a snapshot when the journal cannot continue the local
// index, and the blobs the index references but the store lacks in the background. It follows
// at once; the count of the blobs the store lacks, a walk over the whole index, runs beside it
// (E2E-3), and the follower says "following" only once it is done.
func (p *Peer) follow(ctx context.Context) {
	p.mu.Lock()
	p.fs.State, p.fs.LeaderURL, p.fs.LagEntries = "no_leader", "", 0
	p.behindSince, p.inSync, p.counted = time.Now(), false, false
	p.countGen++
	p.mu.Unlock()

	r := newReplica(p, false)
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		r.counter(ctx)
	}()
	go func() {
		defer wg.Done()
		r.backfiller(ctx)
	}()
	defer wg.Wait()
	r.requestCount() // what an earlier run left missing, if anything
	r.requestBackfill()

	backoff := minBackoff
	for ctx.Err() == nil {
		leader, ok := p.announced()
		if !ok {
			r.noLeader()
			sleep(ctx, minBackoff)
			continue
		}
		_, err := r.stream(ctx, leader, false)
		if err == nil {
			backoff = minBackoff
			continue
		}
		if ctx.Err() != nil {
			return
		}
		r.failed(leader, err)
		sleep(ctx, backoff)
		backoff = min(2*backoff, maxBackoff)
	}
}

// page is one answer of the leader's journal.
type page struct {
	after   int64 // what it was asked for
	entries []store.JournalEntry
	head    int64 // Cortex-Head-Seq: the leader's head the page was read up to
	blobs   <-chan struct{}
	err     error
}

// stream follows leader from the store's position: one goroutine asks for the journal's pages
// one after the other and fetches each page's blobs as soon as it has the page, while this one
// applies the page before (store.ApplyBatch), so that the transfers and the commits overlap
// (E2E-3). It returns nil when the stream should start again from the store's position (after
// a snapshot, or the leader changed), the error of a round that failed, or, with catchUp, true
// once the store has the leader's head (a new leader's catch-up; nothing is waited for).
//
// Log events: replica.diverged (WARN), replica.caught_up.
func (r *replica) stream(ctx context.Context, leader Info, catchUp bool) (caughtUp bool, err error) {
	p := r.p
	seq, epoch, sum, err := p.st.LastSum()
	if err != nil {
		return false, err
	}
	if seq == 0 {
		return false, r.snapshot(ctx, leader, "the local index is empty")
	}
	sctx, stop := context.WithCancel(ctx)
	pages := make(chan *page, 1)                         // one page applies, one waits, one is asked for
	latest := &headSeen{applied: make(chan struct{}, 1)} // the newest head an answer named: pages in the pipe may be older
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		defer close(pages)
		r.produce(sctx, leader, seq, epoch, sum, catchUp, pages, latest)
	}()
	var haltOnce sync.Once
	halt := func() {
		haltOnce.Do(func() {
			stop()
			for range pages { // drained, so that the producer ends
			}
			wg.Wait()
		})
	}
	defer halt()
	resnapshot := func(why string) (bool, error) {
		halt()
		return false, r.snapshot(ctx, leader, why)
	}

	var head int64 = -1
	for pg := range pages {
		local, _ := p.st.Position()
		if pg.err != nil {
			var serr *statusError
			if !errors.As(pg.err, &serr) {
				return false, pg.err
			}
			if pg.after != local {
				return false, nil // asked ahead of the store: again from its position
			}
			switch {
			case serr.status == http.StatusGone || serr.code == "trimmed":
				return resnapshot("the leader's journal no longer reaches back to " + strconv.FormatInt(local, 10) + ": " + serr.message)
			case serr.code == "diverged":
				r.diverged(leader, local, serr.message)
				return resnapshot("diverged: " + serr.message)
			}
			return false, serr
		}
		r.contacted(leader)
		head = pg.head

		var todo []store.JournalEntry
		for _, e := range pg.entries {
			if e.Seq > local {
				todo = append(todo, e)
			}
		}
		if len(todo) > 0 {
			if todo[0].Seq != local+1 {
				r.diverged(leader, local, fmt.Sprintf("the leader's journal goes on with %d", todo[0].Seq))
				return resnapshot(fmt.Sprintf("the leader's journal has a gap after %d", local))
			}
			r.pending(todo[0].At, pg.head-local)
			if pg.blobs != nil {
				select {
				case <-pg.blobs:
				case <-ctx.Done():
					return false, ctx.Err()
				}
			}
			if err := r.apply(ctx, leader, todo); err != nil {
				switch {
				case errors.Is(err, store.ErrDiverged), errors.Is(err, store.ErrOutOfOrder):
					r.diverged(leader, local, err.Error())
					return resnapshot("diverged: " + err.Error())
				case errors.Is(err, errBlobAbsent):
					// The entry cannot apply without its blob; a snapshot takes the index as it is.
					r.warn(leader, "the leader lacks the blob of an entry: taking a snapshot", err)
					return resnapshot(err.Error())
				}
				return false, err
			}
			r.resnapshot = 0
		}
		r.applied(leader, latest, todo)
		latest.pending.Add(-1)
		select {
		case latest.applied <- struct{}{}:
		default:
		}
	}
	if err := ctx.Err(); err != nil {
		return false, err
	}
	local, _ := p.st.Position()
	return head >= 0 && local >= head, nil
}

// produce asks leader for the journal's pages from after on (epoch and sum naming the entry
// there) and hands them to out, each with its blobs being fetched, until ctx ends, a request
// fails (the failed page is the last), the announced leader changes, or, with catchUp, the
// pages reach the leader's head. A follower that has the head asks again with the long poll.
func (r *replica) produce(ctx context.Context, leader Info, after, epoch int64, sum string, catchUp bool, out chan<- *page, latest *headSeen) {
	var fetches sync.WaitGroup
	defer fetches.Wait()
	wait := time.Duration(0)
	for ctx.Err() == nil {
		pg := r.page(ctx, leader, after, epoch, sum, wait)
		if pg.err == nil && len(pg.entries) == 0 && pg.head > after {
			pg.err = fmt.Errorf("the leader's journal answered no entry after %d although its head is %d", after, pg.head)
		}
		if pg.err == nil && len(pg.entries) > 0 {
			pg.blobs = r.prefetch(ctx, leader, pg.entries, &fetches)
		}
		if pg.err == nil {
			latest.saw(pg.head)
		}
		latest.pending.Add(1)
		select {
		case out <- pg:
		case <-ctx.Done():
			return
		}
		if pg.err != nil {
			return
		}
		if n := len(pg.entries); n > 0 {
			last := pg.entries[n-1]
			after, epoch, sum = last.Seq, last.Epoch, last.Sum()
		}
		current := after >= pg.head
		if catchUp && current {
			return
		}
		// A follower that has the head waits for the next entry (the long poll), once the
		// pages handed on are applied; nothing is to be fetched ahead meanwhile. When they took
		// so long that the answer is no longer fresh, it asks once without waiting: whether it
		// is current now, it learns at once, not at the end of a long poll of an idle leader.
		wait = 0
		if current {
			for latest.pending.Load() > 0 {
				select {
				case <-latest.applied:
				case <-ctx.Done():
					return
				}
			}
			if _, at := latest.get(); time.Since(at) < resumeBelow {
				wait = r.p.opt.PollWait
			}
		}
		if l, ok := r.p.announced(); !catchUp && (!ok || l.Instance != leader.Instance || l.URL != leader.URL || l.Epoch != leader.Epoch) {
			return
		}
	}
}

// page asks leader once for the journal after after (waiting up to wait for the first entry).
// The request names the entry there by its epoch and checksum, so that the leader can tell it
// from another entry with the same seq and epoch (review 2: epoch-reuse-hides-divergence).
func (r *replica) page(ctx context.Context, leader Info, after, epoch int64, sum string, wait time.Duration) *page {
	pg := &page{after: after}
	q := url.Values{"after": {strconv.FormatInt(after, 10)}, "epoch": {strconv.FormatInt(epoch, 10)}, "wait": {wait.String()}}
	if sum != "" {
		q.Set("sum", sum)
	}
	resp, err := r.p.get(ctx, base(leader)+"/internal/v1/journal?"+q.Encode(), nil, wait+journalSlack)
	if err != nil {
		pg.err = err
		return pg
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		pg.err = readStatusError(resp)
		return pg
	}
	pg.head, err = strconv.ParseInt(resp.Header.Get("Cortex-Head-Seq"), 10, 64)
	if err != nil {
		pg.err = fmt.Errorf("the journal's Cortex-Head-Seq %q: %w", resp.Header.Get("Cortex-Head-Seq"), err)
		return pg
	}
	dec := json.NewDecoder(resp.Body)
	for {
		var e store.JournalEntry
		if err := dec.Decode(&e); errors.Is(err, io.EOF) {
			break
		} else if err != nil {
			pg.err = fmt.Errorf("reading the journal: %w", err)
			return pg
		}
		pg.entries = append(pg.entries, e)
	}
	return pg
}

// apply applies entries in chunks of applyChunk, each in one transaction, and stops between
// two chunks when ctx ends. A chunk whose blobs the store lacks (blob GC removed one after the
// prefetch, or the prefetch failed) gets them fetched and is applied again.
func (r *replica) apply(ctx context.Context, leader Info, entries []store.JournalEntry) error {
	p := r.p
	for len(entries) > 0 {
		if err := ctx.Err(); err != nil {
			return err
		}
		chunk := entries[:min(applyChunk, len(entries))]
		entries = entries[len(chunk):]
		if hook := p.beforeApply.Load(); hook != nil {
			for _, e := range chunk {
				if err := ctx.Err(); err != nil {
					return err
				}
				(*hook)(e)
			}
		}
		err := p.st.ApplyBatch(chunk)
		var missing *store.BlobMissingError
		for tries := 0; errors.As(err, &missing) && tries < 3; tries++ {
			if ferr := r.fetchAll(ctx, leader, missing.Hashes); ferr != nil {
				return ferr
			}
			err = p.st.ApplyBatch(chunk)
		}
		if err != nil {
			return err
		}
	}
	return nil
}

// prefetch starts fetching the blobs that entries name and the store lacks; the channel is
// closed when every fetch has ended. Failures are left to apply, which fetches what is still
// missing and reports why not.
func (r *replica) prefetch(ctx context.Context, leader Info, entries []store.JournalEntry, wg *sync.WaitGroup) <-chan struct{} {
	done := make(chan struct{})
	seen := make(map[string]bool)
	var hashes []string
	for _, e := range entries {
		if e.Blob != "" && !seen[e.Blob] && !r.p.st.HasBlob(e.Blob) {
			seen[e.Blob] = true
			hashes = append(hashes, e.Blob)
		}
	}
	if len(hashes) == 0 {
		close(done)
		return done
	}
	wg.Add(1)
	go func() {
		defer wg.Done()
		defer close(done)
		_ = r.fetchAll(ctx, leader, hashes)
	}()
	return done
}

// fetchAll imports hashes from leader, as many at a time as the follower's adaptive
// concurrency says, and adapts it: one more while every fetch succeeds and there were more
// blobs than workers, half after a failure. It returns the first failure.
func (r *replica) fetchAll(ctx context.Context, leader Info, hashes []string) error {
	workers := int(r.workers.Load())
	ch := make(chan string)
	var mu sync.Mutex
	var first error
	var wg sync.WaitGroup
	for range min(workers, len(hashes)) {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for hash := range ch {
				if err := r.fetchBlob(ctx, leader, hash); err != nil {
					mu.Lock()
					if first == nil {
						first = err
					}
					mu.Unlock()
				}
			}
		}()
	}
	for _, hash := range hashes {
		select {
		case ch <- hash:
		case <-ctx.Done():
		}
	}
	close(ch)
	wg.Wait()
	switch {
	case first != nil && ctx.Err() == nil && !errors.Is(first, errBlobAbsent):
		r.workers.Store(int32(max(minBlobWorkers, workers/2)))
	case first == nil && len(hashes) > workers:
		r.workers.Store(int32(min(maxBlobWorkers, workers+1)))
	}
	if first == nil {
		first = ctx.Err()
	}
	return first
}

// fetchBlob imports one blob from the leader in the form the leader stores it.
func (r *replica) fetchBlob(ctx context.Context, leader Info, hash string) error {
	var header http.Header
	if r.asLeader {
		// A follower that lacks the blob would hand the request to its leader: this one.
		header = http.Header{"Accept-Encoding": {"gzip"}, "Cortex-Forwarded": {r.p.name}}
	} else {
		header = http.Header{"Accept-Encoding": {"gzip"}}
	}
	resp, err := r.p.get(ctx, base(leader)+"/v1/blobs/sha256:"+hash, header, blobHeaderWait)
	if err != nil {
		return fmt.Errorf("blob sha256:%s: %w", hash, err)
	}
	defer resp.Body.Close()
	switch {
	case resp.StatusCode == http.StatusOK:
	case resp.StatusCode == http.StatusNotFound:
		return fmt.Errorf("%w: sha256:%s", errBlobAbsent, hash)
	case r.asLeader && resp.StatusCode == http.StatusServiceUnavailable && resp.Header.Get("Cortex-Error") == "no-leader":
		return fmt.Errorf("%w: sha256:%s (%s)", errBlobAbsent, hash, leader.Instance)
	default:
		return fmt.Errorf("blob sha256:%s: %w", hash, readStatusError(resp))
	}
	var gz bool
	switch enc := strings.ToLower(strings.TrimSpace(resp.Header.Get("Content-Encoding"))); enc {
	case "", "identity":
	case "gzip":
		gz = true
	default:
		return fmt.Errorf("blob sha256:%s: unknown Content-Encoding %q", hash, enc)
	}
	if err := r.p.st.ImportStored(hash, resp.Body, gz); err != nil {
		return fmt.Errorf("blob sha256:%s: %w", hash, err)
	}
	return nil
}

// snapshot replaces the local index with the leader's and has the blobs it lacks counted and
// back-filled. While snapshots do not help (the index diverges again before any entry
// applies), each one waits longer than the one before, from 1 s up to a minute.
//
// Log events: replica.snapshot.
func (r *replica) snapshot(ctx context.Context, leader Info, reason string) error {
	p := r.p
	if r.resnapshot > 0 && !sleep(ctx, r.resnapshot) {
		return ctx.Err()
	}
	r.resnapshot = min(max(2*r.resnapshot, time.Second), time.Minute)
	p.mu.Lock()
	p.fs.State = "snapshot"
	p.mu.Unlock()

	start := time.Now()
	resp, err := p.get(ctx, base(leader)+"/internal/v1/snapshot", nil, snapshotHeaderWait)
	if err != nil {
		return fmt.Errorf("snapshot: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("snapshot: %w", readStatusError(resp))
	}
	body := &countingReader{r: resp.Body}
	if err := p.st.ReplaceIndex(body); err != nil {
		return fmt.Errorf("snapshot: %w", err)
	}
	seq, epoch := p.st.Position()
	p.mu.Lock()
	p.fs.State = "catching_up"
	p.counted = false // the new index references other blobs: "following" waits for their count
	p.countGen++
	p.mu.Unlock()
	r.requestCount()
	oplog.For("cluster").Info("replaced the index with the leader's", "event", "replica.snapshot", "leader", leader.Instance,
		"leader_url", leader.URL, "reason", reason, "seq", seq, "epoch", epoch, "bytes", body.n,
		"duration_ms", time.Since(start).Milliseconds())
	r.requestBackfill()
	return nil
}

// contacted notes a round that reached the leader.
//
// Log events: replica.following.
func (r *replica) contacted(leader Info) {
	p := r.p
	p.mu.Lock()
	p.fs.LeaderURL = leader.URL
	if p.fs.State == "no_leader" || p.fs.State == "snapshot" {
		p.fs.State = "catching_up"
	}
	if leader.Instance != p.name {
		p.peer = leader
	}
	if !r.quiet {
		p.staleSince = time.Time{} // another instance leads: a later take-over waits anew if it must
	}
	p.mu.Unlock()
	if r.contact && r.leader.Instance == leader.Instance && r.leader.URL == leader.URL && r.leader.Epoch == leader.Epoch {
		return
	}
	failures := r.failures
	r.contact, r.leader, r.failures = true, leader, 0
	if !r.quiet {
		oplog.For("cluster").Info("following", "event", "replica.following", "leader", leader.Instance, "leader_url", leader.URL,
			"leader_epoch", leader.Epoch, "failures_before", failures)
	}
}

// pending notes that the follower is about to apply entries of the leader's, the oldest
// written at at, with entries still to apply up to the leader's head. The lag counts from the
// oldest entry the follower lacks, so it does not move while that is still missing.
func (r *replica) pending(at time.Time, entries int64) {
	p := r.p
	p.mu.Lock()
	if p.behindSince.IsZero() || at.Before(p.behindSince) { // a time noted before the leader answered is a guess
		p.behindSince = at
	}
	p.fs.LagEntries = max(0, entries)
	p.mu.Unlock()
}

// headSeen is the newest head of the leader's that an answer of a stream named, and when it
// came; pending counts the pages handed to the applying side and not applied yet.
type headSeen struct {
	mu      sync.Mutex
	seq     int64
	at      time.Time
	pending atomic.Int64
	applied chan struct{} // a page was applied (capacity 1)
}

func (h *headSeen) saw(seq int64) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if seq >= h.seq {
		h.seq, h.at = seq, time.Now()
	}
}

func (h *headSeen) get() (int64, time.Time) {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.seq, h.at
}

// applied notes a page applied (done: its entries the store did not have) against the newest
// head an answer named (latest), which is newer than the page's when the pages after it are
// already on their way. A store that has that head was current when the leader answered: no
// lag, if the answer is fresh; if it waited in the pipe, the lag is at most its age, and the
// answer to the request under way tells. Else the store lags from the next entry it lacks,
// written at the earliest when the last one it applied was, and LagEntries is the leader's
// head minus the store's position, exact as of that answer.
//
// Log events: replica.caught_up.
func (r *replica) applied(leader Info, latest *headSeen, done []store.JournalEntry) {
	p := r.p
	local, _ := p.st.Position()
	head, answered := latest.get()
	now := time.Now()
	p.mu.Lock()
	p.inSyncLocked(now, false) // notes a lag that reached behindLogAfter before it is reset
	current := local >= head
	logIt := false
	switch {
	case current && now.Sub(answered) < resumeBelow:
		p.behindSince, p.fs.LagEntries = time.Time{}, 0
		logIt, p.behindLong = p.behindLong, false
	case current:
		p.fs.LagEntries = 0
		if p.behindSince.IsZero() || answered.After(p.behindSince) {
			p.behindSince = answered
		}
	default:
		p.fs.LagEntries = head - local
		if n := len(done); n > 0 {
			p.behindSince = done[n-1].At
		} else if p.behindSince.IsZero() {
			p.behindSince = now
		}
	}
	p.inSyncLocked(now, true)
	p.mu.Unlock()
	if logIt && !r.quiet {
		oplog.For("cluster").Info("caught up with the leader", "event", "replica.caught_up", "leader", leader.Instance,
			"leader_url", leader.URL, "seq", local)
	}
}

// noLeader notes that no leader is known.
func (r *replica) noLeader() {
	p := r.p
	p.mu.Lock()
	p.fs.State, p.fs.LeaderURL = "no_leader", ""
	if p.behindSince.IsZero() {
		p.behindSince = time.Now()
	}
	p.mu.Unlock()
	r.contact = false
}

// failed notes a round that failed: the follower is not known to be current from now on (its
// lag grows), and the failure is logged, the first one of a run and then once a minute.
//
// Log events: replica.failed (WARN).
func (r *replica) failed(leader Info, err error) {
	p := r.p
	p.failures.Add(1)
	unreachable := true
	var serr *statusError
	if errors.As(err, &serr) && serr.code != "not-leader" && serr.code != "no-leader" {
		unreachable = false
	}
	p.mu.Lock()
	if unreachable {
		p.fs.State = "no_leader"
	}
	if p.behindSince.IsZero() {
		p.behindSince = time.Now()
	}
	p.mu.Unlock()
	r.contact = false
	r.failures++
	if r.failures == 1 || time.Since(r.logged) >= failureLogEvery {
		r.logged = time.Now()
		oplog.For("cluster").Warn("replication failed", "event", "replica.failed", "leader", leader.Instance,
			"leader_url", leader.URL, "failures", r.failures, oplog.Err(err))
	}
}

// warn logs a replication problem that the follower works around.
func (r *replica) warn(leader Info, msg string, err error) {
	oplog.For("cluster").Warn(msg, "event", "replica.failed", "leader", leader.Instance, "leader_url", leader.URL, oplog.Err(err))
}

// diverged logs that the local index is not a copy of the leader's from seq on: what this
// instance wrote that the leader does not have is dropped by the snapshot that follows.
//
// Log events: replica.diverged (WARN).
func (r *replica) diverged(leader Info, seq int64, why string) {
	_, epoch := r.p.st.Position()
	oplog.For("cluster").Warn("the index diverged from the leader's: taking a snapshot", "event", "replica.diverged",
		"leader", leader.Instance, "leader_url", leader.URL, "seq", seq, "epoch", epoch, "detail", why)
}

// requestCount asks the counter for a count of the blobs the store lacks.
func (r *replica) requestCount() {
	select {
	case r.countReq <- struct{}{}:
	default:
	}
}

// counter counts the blobs the index references and the store lacks when asked (at the start
// of following and after a snapshot), beside the following itself, and sets BlobsMissing. A
// count that began before a snapshot does not count for the index the snapshot brought.
func (r *replica) counter(ctx context.Context) {
	p := r.p
	for {
		select {
		case <-ctx.Done():
			return
		case <-r.countReq:
		}
		p.mu.Lock()
		gen := p.countGen
		p.mu.Unlock()
		n, err := r.countMissing(ctx)
		if err != nil {
			if ctx.Err() == nil {
				oplog.For("cluster").Warn("cannot count the blobs the index lacks", "event", "replica.failed", oplog.Err(err))
				r.requestCount()
				sleep(ctx, time.Second)
			}
			continue
		}
		p.mu.Lock()
		if p.countGen == gen {
			p.fs.BlobsMissing, p.counted = n, true
		}
		p.mu.Unlock()
	}
}

// requestBackfill asks the back-filler for a pass.
func (r *replica) requestBackfill() {
	select {
	case r.backfillReq <- struct{}{}:
	default:
	}
}

// backfiller runs the back-fill passes that are asked for, until ctx ends; a pass that leaves
// blobs the leader has is tried again after a pause (1 s, doubled up to a minute).
func (r *replica) backfiller(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case <-r.backfillReq:
		}
		pause := time.Second
		for !r.backfill(ctx) {
			if !sleep(ctx, pause) {
				return
			}
			pause = min(2*pause, time.Minute)
		}
	}
}

// backfill fetches the blobs the index references and the store lacks from the leader (for a
// leader: from the other instance), a few at a time, while the journal goes on; BlobsMissing
// counts them down. It returns true when
// none is left that the leader has (one the leader lacks too is logged, not counted).
//
// Log events: replica.backfilled, replica.failed (WARN).
func (r *replica) backfill(ctx context.Context) bool {
	p := r.p
	if hook := p.beforeBackfill.Load(); hook != nil {
		(*hook)()
	}
	missing, err := r.countMissing(ctx)
	if err != nil {
		if ctx.Err() == nil {
			oplog.For("cluster").Warn("cannot count the blobs the index lacks", "event", "replica.failed", oplog.Err(err))
		}
		return false
	}
	p.setBlobsMissing(missing)
	if missing == 0 {
		return true
	}
	leader, ok := r.source()
	if !ok {
		return false
	}

	start := time.Now()
	var fetched, absent, failed atomic.Int64
	var errMu sync.Mutex
	var firstErr error
	hashes := make(chan string)
	var wg sync.WaitGroup
	for range backfillWorkers {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for hash := range hashes {
				err := r.fetchBlob(ctx, leader, hash)
				switch {
				case err == nil:
					fetched.Add(1)
					p.addBlobsMissing(-1)
				case errors.Is(err, errBlobAbsent):
					absent.Add(1)
					r.markAbsent(hash)
					p.addBlobsMissing(-1)
				default:
					failed.Add(1)
					errMu.Lock()
					if firstErr == nil {
						firstErr = err
					}
					errMu.Unlock()
				}
			}
		}()
	}
	walkErr := p.st.ReferencedBlobs(func(hash string) error {
		if p.st.HasBlob(hash) || r.isAbsent(hash) {
			return nil
		}
		select {
		case hashes <- hash:
			return nil
		case <-ctx.Done():
			return ctx.Err()
		}
	})
	close(hashes)
	wg.Wait()

	remaining, cerr := r.countMissing(ctx)
	if cerr == nil {
		p.setBlobsMissing(remaining)
	}
	log := oplog.For("cluster")
	attrs := []any{"event", "replica.backfilled", "leader", leader.Instance, "fetched", fetched.Load(), "failed", failed.Load(),
		"absent_on_leader", absent.Load(), "remaining", remaining, "duration_ms", time.Since(start).Milliseconds()}
	if firstErr != nil {
		attrs = append(attrs, oplog.Err(firstErr))
	}
	if ctx.Err() == nil {
		log.Info("fetched the blobs the index lacked", attrs...)
	}
	if n := absent.Load(); n > 0 {
		log.Warn("the instance asked lacks blobs the index references", "event", "replica.failed", "leader", leader.Instance, "blobs", n)
	}
	return walkErr == nil && cerr == nil && remaining == 0
}

// countMissing counts the blobs the index references that the store lacks and the leader may
// have.
func (r *replica) countMissing(ctx context.Context) (int64, error) {
	var n int64
	err := r.p.st.ReferencedBlobs(func(hash string) error {
		if !r.p.st.HasBlob(hash) && !r.isAbsent(hash) {
			n++
		}
		return ctx.Err()
	})
	return n, err
}

func (r *replica) markAbsent(hash string) {
	r.absentMu.Lock()
	r.absent[hash] = true
	r.absentMu.Unlock()
}

func (r *replica) isAbsent(hash string) bool {
	r.absentMu.Lock()
	defer r.absentMu.Unlock()
	return r.absent[hash]
}

// backfillAsLeader fetches the blobs a leader's index references and its store lacks from the
// other instance, until none is left that the other has or ctx ends: a follower promoted
// before its back-fill ended would otherwise fail every read of those blobs for as long as it
// leads (E2E-4). The other instance serves them also as a follower; until it is back, the
// passes are tried again after a pause (1 s, doubled up to a minute).
func (p *Peer) backfillAsLeader(ctx context.Context) {
	r := newReplica(p, false)
	r.source, r.asLeader = p.Peer, true
	pause := time.Second
	for !r.backfill(ctx) {
		if !sleep(ctx, pause) {
			return
		}
		pause = min(2*pause, time.Minute)
	}
}

func (p *Peer) setBlobsMissing(n int64) {
	p.mu.Lock()
	p.fs.BlobsMissing = n
	p.mu.Unlock()
}

func (p *Peer) addBlobsMissing(d int64) {
	p.mu.Lock()
	p.fs.BlobsMissing = max(0, p.fs.BlobsMissing+d)
	p.mu.Unlock()
}

// base is the leader's base URL without a trailing slash.
func base(leader Info) string { return strings.TrimRight(leader.URL, "/") }

// get sends GET target and returns the answer once it begins within headerWait. Its body fails
// when no byte arrives for idleWait; closing it releases the request.
func (p *Peer) get(ctx context.Context, target string, header http.Header, headerWait time.Duration) (*http.Response, error) {
	ctx, cancel := context.WithCancel(ctx)
	timer := time.AfterFunc(headerWait, cancel)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, target, nil)
	if err != nil {
		timer.Stop()
		cancel()
		return nil, err
	}
	for k, v := range header {
		req.Header[k] = v
	}
	resp, err := p.client.Do(req)
	timer.Stop()
	if err != nil {
		cancel()
		return nil, err
	}
	resp.Body = &watchedBody{rc: resp.Body, cancel: cancel, timer: time.AfterFunc(idleWait, cancel)}
	return resp, nil
}

// watchedBody cancels its request when no byte arrives for idleWait.
type watchedBody struct {
	rc     io.ReadCloser
	cancel context.CancelFunc
	timer  *time.Timer
}

func (b *watchedBody) Read(buf []byte) (int, error) {
	n, err := b.rc.Read(buf)
	if n > 0 {
		b.timer.Reset(idleWait)
	}
	return n, err
}

func (b *watchedBody) Close() error {
	b.timer.Stop()
	err := b.rc.Close()
	b.cancel()
	return err
}

// countingReader counts what passes through it.
type countingReader struct {
	r io.Reader
	n int64
}

func (c *countingReader) Read(buf []byte) (int, error) {
	n, err := c.r.Read(buf)
	c.n += int64(n)
	return n, err
}

// statusError is an answer of the leader other than the one asked for.
type statusError struct {
	status        int
	code, message string
}

func (e *statusError) Error() string {
	if e.code == "" {
		return fmt.Sprintf("the leader answered %d: %s", e.status, e.message)
	}
	return fmt.Sprintf("the leader answered %d %s: %s", e.status, e.code, e.message)
}

// readStatusError reads a Cortex error answer ({"error":…,"message":…}, Cortex-Error).
func readStatusError(resp *http.Response) *statusError {
	e := &statusError{status: resp.StatusCode, code: resp.Header.Get("Cortex-Error")}
	data, _ := io.ReadAll(io.LimitReader(resp.Body, 4<<10))
	var body struct {
		Error   string `json:"error"`
		Message string `json:"message"`
	}
	if json.Unmarshal(data, &body) == nil {
		if e.code == "" {
			e.code = body.Error
		}
		e.message = body.Message
	}
	if e.message == "" {
		e.message = strings.TrimSpace(string(data))
	}
	return e
}
