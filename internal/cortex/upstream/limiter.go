package upstream

import (
	"container/list"
	"context"
	"fmt"
	"math/rand/v2"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/oplog"
)

const (
	// maxRetryAfter caps a pause that upstream asks for with Retry-After.
	maxRetryAfter = time.Hour
	// forgetAfter is how long the state of a host that is idle and not paused is kept for its
	// failures in a row. A state without failures goes as soon as the host is idle (forgetLocked).
	forgetAfter = time.Hour
	// keepStates bounds the states kept although their host is idle (for its failures or its
	// pause): a client that names a new failing host in every request would grow the map, and
	// /status and every scrape with it, without end. Past it, a new host pushes an idle state
	// out (evictLocked).
	keepStates = 1000
)

// BusyError says that the host's slots stayed taken for the host's queue_wait: the request was
// not sent. The server answers 429 host-busy with Retry-After.
type BusyError struct {
	Host       string
	RetryAfter time.Duration // an estimate from the queue, at least 1 s
}

func (e *BusyError) Error() string {
	return fmt.Sprintf("host %s is busy: no free slot within the queue wait, retry after %s", e.Host, e.RetryAfter)
}

// PausedError says that the host is paused (by the breaker or by upstream's Retry-After): the
// request was not sent. The server answers 503 host-paused with Retry-After.
type PausedError struct {
	Host  string
	Until time.Time
}

func (e *PausedError) Error() string {
	return fmt.Sprintf("host %s is paused until %s", e.Host, e.Until.UTC().Format(time.RFC3339))
}

// HostState is what GET /status shows of a host.
type HostState struct {
	Host          string     `json:"host"`
	Queue         int        `json:"queue"`
	InFlight      int        `json:"in_flight"`
	PausedUntil   *time.Time `json:"paused_until"` // nil while not paused
	FailuresInRow int        `json:"failures_in_row"`
}

// hostState is the floor of one host (a host name without port), for every client together.
// Every field is guarded by Upstream.mu.
type hostState struct {
	limit       int           // the host's concurrency, from the policy of the last request
	inFlight    int           // requests that hold a slot
	cooling     int           // slots in their pause after a request
	queue       list.List     // *waiter, the oldest first
	failures    int           // failures in a row
	pausedUntil time.Time     // zero while not paused
	lastUsed    time.Time     // when a slot was last taken or given back
	took        time.Duration // moving average of a request's duration, for BusyError.RetryAfter
}

// waiter is a request in the queue of a host. done and err are guarded by Upstream.mu; ready
// is closed once done is set.
type waiter struct {
	ready chan struct{}
	done  bool  // granted a slot (err nil) or turned away (err set)
	err   error // *PausedError
}

func (s *hostState) free() int { return s.limit - s.inFlight - s.cooling }

func (s *hostState) idle() bool { return s.inFlight == 0 && s.cooling == 0 && s.queue.Len() == 0 }

// outcome is what a request says about the health of its host, for the breaker.
type outcome int

const (
	neutral outcome = iota // nothing reached the host, or the caller gave up
	success                // the host answered with anything but a 5xx
	failure                // network error, timeout, 5xx
)

// pauseEvent is a host.paused record, logged once the lock is released.
type pauseEvent struct {
	host     string
	until    time.Time
	reason   string // "breaker" or "retry-after"
	failures int
}

// stateLocked returns the state of host, creating it.
func (u *Upstream) stateLocked(host string, now time.Time) *hostState {
	s := u.states[host]
	if s == nil {
		if len(u.states) >= keepStates {
			u.evictLocked(now)
		}
		s = &hostState{limit: 1, lastUsed: now}
		u.states[host] = s
	}
	return s
}

// forgetLocked drops the state of host once it says nothing a new one would not: no request
// holds, waits for or pauses a slot, the host is not paused and has no failures in a row. So
// the map holds the hosts with work, a pause or failures, not every host ever asked.
func (u *Upstream) forgetLocked(host string, s *hostState) {
	if s.idle() && s.pausedUntil.IsZero() && s.failures == 0 && u.states[host] == s {
		delete(u.states, host)
	}
}

// evictLocked drops the idle state least worth keeping to make room (keepStates): a host
// without an entry of the policy goes before one with an entry, a paused host goes last, and
// among equals the one used longest ago goes first. A state with work is never dropped (its
// requests hold it); if every state has work, the map grows with the requests under way only.
func (u *Upstream) evictLocked(now time.Time) {
	p := u.Policy()
	worth := func(host string, s *hostState) int {
		w := 0
		if now.Before(s.pausedUntil) {
			w += 2
		}
		if p.For(host).Name != "" {
			w++
		}
		return w
	}
	victim, vw := "", 0
	var vs *hostState
	for host, s := range u.states {
		if !s.idle() {
			continue
		}
		if w := worth(host, s); vs == nil || w < vw || w == vw && s.lastUsed.Before(vs.lastUsed) {
			victim, vw, vs = host, w, s
		}
	}
	if vs != nil {
		delete(u.states, victim)
	}
}

// acquire takes a slot of host: at most hp.Concurrency requests are in flight, and a slot is
// used again only after its pause. Requests wait in order of arrival. It fails with
// *PausedError while the host is paused (a request waiting when the pause begins too), with
// *BusyError when no slot came free within hp.QueueWait, or with ctx's error.
func (u *Upstream) acquire(ctx context.Context, host string, hp HostPolicy) error {
	now := u.now()
	u.mu.Lock()
	s := u.stateLocked(host, now)
	s.limit = hp.Concurrency
	resumed := u.resumeLocked(s, now)
	if now.Before(s.pausedUntil) {
		until := s.pausedUntil
		u.mu.Unlock()
		logResumed(host, resumed)
		return &PausedError{Host: host, Until: until}
	}
	if s.queue.Len() == 0 && s.free() > 0 {
		s.inFlight++
		s.lastUsed = now
		u.mu.Unlock()
		logResumed(host, resumed)
		return nil
	}
	w := &waiter{ready: make(chan struct{})}
	e := s.queue.PushBack(w)
	u.mu.Unlock()
	logResumed(host, resumed)

	timer := time.NewTimer(hp.QueueWait)
	defer timer.Stop()
	var cause error
	select {
	case <-w.ready:
		return w.err
	case <-timer.C:
	case <-ctx.Done():
		cause = ctx.Err()
	}

	u.mu.Lock()
	defer u.mu.Unlock()
	if w.done {
		if w.err != nil || cause == nil {
			return w.err
		}
		// Granted just as the caller gave up: the slot goes to the next in line.
		s.inFlight--
		u.dispatchLocked(s)
		u.forgetLocked(host, s)
		return cause
	}
	s.queue.Remove(e)
	u.forgetLocked(host, s)
	if cause != nil {
		return cause
	}
	return &BusyError{Host: host, RetryAfter: s.retryAfter(hp)}
}

// release gives back the slot of a request. The breaker learns the outcome, a 429 or 503
// pauses the host for retryAfter (at most 1 h), and with sent the slot first pauses for the
// host's pause (±30 %).
func (u *Upstream) release(host string, hp HostPolicy, sent bool, o outcome, retryAfter, took time.Duration) {
	now := u.now()
	u.mu.Lock()
	s := u.states[host] // held slots keep a state from being forgotten
	s.inFlight--
	s.lastUsed = now
	if took > 0 {
		if s.took == 0 {
			s.took = took
		} else {
			s.took = (3*s.took + took) / 4
		}
	}

	var paused *pauseEvent
	switch o {
	case success:
		s.failures = 0
	case failure:
		s.failures++
		if s.failures >= hp.BreakerFailures {
			paused = u.pauseLocked(host, s, now.Add(hp.BreakerPause), "breaker")
		}
	}
	if retryAfter > 0 {
		if p := u.pauseLocked(host, s, now.Add(min(retryAfter, maxRetryAfter)), "retry-after"); p != nil {
			paused = p
		}
	}

	if pause := jitter(hp.Pause); sent && pause > 0 {
		s.cooling++ // keeps the state until the pause is over
		time.AfterFunc(pause, func() {
			u.mu.Lock()
			s.cooling--
			u.dispatchLocked(s)
			u.forgetLocked(host, s)
			u.mu.Unlock()
		})
	} else {
		u.dispatchLocked(s)
		u.forgetLocked(host, s)
	}
	u.mu.Unlock()

	if paused != nil {
		oplog.For("upstream").Warn("host paused", "event", "host.paused", "host", paused.host,
			"until", paused.until.UTC().Format(time.RFC3339), "reason", paused.reason,
			"failures_in_row", paused.failures, "pause_s", int(paused.until.Sub(now).Round(time.Second).Seconds()))
	}
}

// dispatchLocked hands free slots to the oldest waiters.
func (u *Upstream) dispatchLocked(s *hostState) {
	for s.queue.Len() > 0 && s.free() > 0 {
		w := s.queue.Remove(s.queue.Front()).(*waiter)
		s.inFlight++
		w.done = true
		close(w.ready)
	}
}

// pauseLocked pauses host until until (unless it already is, as long) and turns away every
// request waiting for it.
func (u *Upstream) pauseLocked(host string, s *hostState, until time.Time, reason string) *pauseEvent {
	if !until.After(s.pausedUntil) {
		return nil
	}
	s.pausedUntil = until
	for e := s.queue.Front(); e != nil; e = e.Next() {
		w := e.Value.(*waiter)
		w.done = true
		w.err = &PausedError{Host: host, Until: until}
		close(w.ready)
	}
	s.queue.Init()
	return &pauseEvent{host: host, until: until, reason: reason, failures: s.failures}
}

// resumeLocked ends a pause that is over and says whether it did. The count of failures stays:
// after a pause one more failure pauses the host again, a success resets it.
func (u *Upstream) resumeLocked(s *hostState, now time.Time) bool {
	if s.pausedUntil.IsZero() || now.Before(s.pausedUntil) {
		return false
	}
	s.pausedUntil = time.Time{}
	return true
}

func logResumed(host string, resumed bool) {
	if resumed {
		oplog.For("upstream").Info("host resumed", "event", "host.resumed", "host", host)
	}
}

// retryAfter estimates when a slot is free for a request that gives up now: the requests still
// waiting and one more, shared by the slots, each the host's usual duration plus its pause.
func (s *hostState) retryAfter(hp HostPolicy) time.Duration {
	per := s.took
	if per <= 0 {
		per = time.Second
	}
	per += hp.Pause
	rounds := (s.queue.Len() + 1 + s.limit - 1) / max(s.limit, 1)
	d := time.Duration(rounds) * per
	d = (d + time.Second - 1).Truncate(time.Second)
	return min(max(d, time.Second), maxRetryAfter)
}

// sweep logs the end of pauses that are over and forgets idle hosts whose pause is over and
// that have no failures in a row, or whose failures are older than forgetAfter.
func (u *Upstream) sweep() {
	now := u.now()
	var resumed []string
	u.mu.Lock()
	for host, s := range u.states {
		if u.resumeLocked(s, now) {
			resumed = append(resumed, host)
		}
		if s.idle() && s.pausedUntil.IsZero() && (s.failures == 0 || now.Sub(s.lastUsed) > forgetAfter) {
			delete(u.states, host)
		}
	}
	u.mu.Unlock()
	sort.Strings(resumed)
	for _, host := range resumed {
		logResumed(host, true)
	}
}

// applyLimits gives every host the concurrency of policy p, so that a raised limit lets
// waiting requests start at once.
func (u *Upstream) applyLimits(p *Policy) {
	u.mu.Lock()
	defer u.mu.Unlock()
	for host, s := range u.states {
		s.limit = p.For(host).Concurrency
		u.dispatchLocked(s)
	}
}

// HostStates is the state of every host the policy names (by its exact name) and of every
// other host with work (requests in flight, waiting or in their pause), a pause or failures in
// a row, sorted by host. Hosts asked before and idle since are not listed: the list is bounded
// by the policy, the requests under way and keepStates.
func (u *Upstream) HostStates() []HostState {
	p := u.Policy()
	now := u.now()
	byHost := make(map[string]HostState)
	u.mu.Lock()
	for host, s := range u.states {
		paused := now.Before(s.pausedUntil)
		if s.idle() && !paused && s.failures == 0 {
			continue // a pause just over, which the next request or sweep forgets
		}
		hs := HostState{Host: host, Queue: s.queue.Len(), InFlight: s.inFlight, FailuresInRow: s.failures}
		if paused {
			until := s.pausedUntil
			hs.PausedUntil = &until
		}
		byHost[host] = hs
	}
	u.mu.Unlock()
	for key := range p.Hosts {
		if _, ok := byHost[key]; !ok && !strings.HasPrefix(key, "*.") {
			byHost[key] = HostState{Host: key}
		}
	}
	out := make([]HostState, 0, len(byHost))
	for _, hs := range byHost {
		out = append(out, hs)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Host < out[j].Host })
	return out
}

// jitter spreads a pause by ±30 %, as Radix's crawl does.
func jitter(d time.Duration) time.Duration {
	if d <= 0 {
		return 0
	}
	return time.Duration(float64(d) * (0.7 + 0.6*rand.Float64()))
}
