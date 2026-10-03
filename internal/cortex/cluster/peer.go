package cluster

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
	"github.com/leonieziechmann/betula/internal/oplog"
)

// The defaults of Options.
const (
	DefaultStepDownPause = 15 * time.Second
	DefaultPollWait      = 25 * time.Second
)

// Bounds of a change of leader.
const (
	// fenceLog is how often a hand-over that waits for the writes in flight logs that it still
	// waits. It waits for them however long they run: a write admitted in this term that
	// committed after the next leader's entries were applied here would take the next
	// leader's epoch, and nothing would tell the two histories apart (review 2:
	// stray-write-takes-new-leaders-epoch). The store bounds every write (SQLite's busy
	// timeout), and a process that is killed meanwhile has crashed.
	fenceLog = 5 * time.Second
	// successorWait is how long a leader that ended its term on purpose keeps serving its
	// journal and blobs to its successor (its role stays Leader, the write fence closed): until
	// the successor announces itself, which it does once it has the whole journal (see
	// catchUp), or until nobody holds the lock for noSuccessorAfter (no successor is coming).
	// Within the 30 s a container stop allows, with the HTTP shutdown after it (E2E-1).
	successorWait    = 20 * time.Second
	noSuccessorAfter = time.Second
	// lockProbeEvery is how often a leader that waits for its successor asks the elector
	// whether somebody holds the lock (lockProbe).
	lockProbeEvery = 100 * time.Millisecond
	// catchUpMax bounds a new leader's catch-up from its predecessor, which normally ends
	// earlier: when it has the predecessor's head, or when the predecessor stops answering
	// (it refuses connections, or no longer leads: its successorWait passed).
	catchUpMax = 30 * time.Second
	// catchUpRetry is the pause after a catch-up round that failed while the predecessor still
	// answers (a blob that did not arrive, a 5xx).
	catchUpRetry = 250 * time.Millisecond
	// successorPoll is how often a leader that stepped down looks for its successor's
	// announcement.
	successorPoll = 20 * time.Millisecond
	// campaignRetry is the pause after a campaign that failed (a lock file that cannot be
	// opened), and after a term that could not or must not begin its epoch: the other
	// instance can win the lock meanwhile.
	campaignRetry = time.Second
	// headEvery is how often a leader records its head (HeadRecord) and checks that its
	// announcement is still there (review 2: leader-json-never-reannounced).
	headEvery = time.Second
	// acceptedLoss is how much of a crashed leader's last writes may be lost (owner,
	// 2026-10-02: the last minutes before a crash): a candidate behind a record that is not
	// final leads at once when its own newest entry is at most this much older than the
	// recorded one.
	acceptedLoss = 5 * time.Minute
)

// staleWait is how long a candidate that is behind the recorded head and cannot catch up from
// the instance that recorded it waits for that instance (resigning and campaigning again, so
// that it can win) before it leads anyway (review 2: stale-instance-wins-and-wipes-newer-data).
// A variable for the tests.
var staleWait = time.Minute

// Options configure a Peer. The zero value is the production setting.
type Options struct {
	// Elector elects the leader; nil: Flock(lockPath).
	Elector Elector
	// Transport carries the follower's requests to the leader (journal, snapshot, blobs); nil:
	// a transport of its own (2 s to connect, no proxy of the environment).
	Transport http.RoundTripper
	// StepDownPause is how long an instance that stepped down stays out of the election;
	// 0: DefaultStepDownPause (15 s), so that the other instance takes over.
	StepDownPause time.Duration
	// PollWait is how long a follower's request for the journal waits for the next entry at the
	// leader (the long poll); 0: DefaultPollWait (25 s).
	PollWait time.Duration
}

// Peer is one instance of a pair whose leader an Elector elects (New): the Node the server
// codes against. It starts as a follower that keeps its store a copy of the leader's (see
// follow), campaigns all the while, and leads once it wins: it stops following, catches up
// with what its predecessor still serves, checks that it is not behind the head the last
// leader recorded, begins its epoch (store.StartEpoch), announces itself and opens the write
// fence. While it leads it records its head every second, announces itself again when the
// announcement went missing, and fetches the blobs its index lacks from the other instance. A
// step-down closes the fence, waits for every write in flight, records its final head,
// resigns, serves its journal until its successor has announced itself, follows, and stays
// out of the election for StepDownPause. A term that is lost (Term.Lost) ends at once.
//
// Log events: leader.acquired, leader.stepped_down, leader.lost (WARN), leader.campaign_failed
// (WARN), leader.deferred (WARN), leader.reannounced (WARN), leader.fence_wait (WARN),
// leader.record_failed (WARN), replica.stale_takeover (ERROR), replica.following,
// replica.caught_up, replica.diverged (WARN), replica.snapshot, replica.backfilled,
// replica.failed (WARN).
type Peer struct {
	st        *store.Store
	elector   Elector
	opt       Options
	name      string // the instance
	client    *http.Client
	transport *http.Transport // own transport, nil when Options.Transport is given

	ctx     context.Context
	cancel  context.CancelFunc
	stopped chan struct{} // closed when run has returned
	stepReq chan struct{} // StepDown asks run (capacity 1)

	failures atomic.Int64 // replication rounds that failed (tests)

	// beforeApply, when set (tests), runs before each Apply of the follower; beforeBackfill
	// before each back-fill pass.
	beforeApply    atomic.Pointer[func(store.JournalEntry)]
	beforeBackfill atomic.Pointer[func()]

	mu          sync.Mutex
	role        Role
	writable    bool          // the write fence is open: leading, not handing over
	writers     int           // writes in flight (BeginWrite without done)
	drained     chan struct{} // closed when the last write in flight ends after the fence closed
	self        Info          // the current term while leading
	term        Term          // the current term while leading
	changes     chan struct{} // closed and replaced when the role changes
	led         chan struct{} // closed while this instance leads with the fence open
	stepping    chan struct{} // closed when the hand-over StepDown asked for ends
	crashed     bool          // tests: stop without a hand-over
	fs          FollowerState
	behindSince time.Time // the follower is not known to be current since; zero when it is
	// inSync is the follower's "following" with hysteresis: lost when the lag reaches
	// followingBelow, back when it is under resumeBelow; counted says that BlobsMissing has
	// been counted since the follower started or took its last snapshot, and countGen tells a
	// count that began before a snapshot from one after it. behindLong says that the lag
	// reached behindLogAfter since the last replica.caught_up (E2E-6, D3).
	inSync, counted, behindLong bool
	countGen                    uint64
	peer                        Info      // the other instance as last seen: the leader followed, or the predecessor of this term
	staleSince                  time.Time // the first take-over deferred because this instance was behind the recorded head
	recorded                    HeadRecord
}

// New returns the Node of an instance that shares the leadership with another one: a follower
// at first, which leads once it wins the election of opts.Elector (default Flock(lockPath)).
// self names the instance: Instance unique in the pair, URL the base URL the other instance
// reaches it at (its advertise URL). It runs until ctx ends or Close; a leader then hands
// over (see Close).
func New(ctx context.Context, st *store.Store, self Info, lockPath string, opts Options) (*Peer, error) {
	if st == nil {
		return nil, errors.New("cluster: a store is required")
	}
	if self.Instance == "" {
		return nil, errors.New("cluster: the instance needs a name")
	}
	if u, err := url.Parse(self.URL); err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" {
		return nil, fmt.Errorf("cluster: advertise URL %q: want http://host:port", self.URL)
	}
	if opts.Elector == nil {
		if lockPath == "" {
			return nil, errors.New("cluster: a lock file or an elector is required")
		}
		opts.Elector = Flock(lockPath)
	}
	if c, ok := opts.Elector.(interface{ check() error }); ok {
		if err := c.check(); err != nil {
			return nil, err
		}
	}
	if opts.StepDownPause <= 0 {
		opts.StepDownPause = DefaultStepDownPause
	}
	if opts.PollWait <= 0 {
		opts.PollWait = DefaultPollWait
	}
	p := &Peer{
		st:      st,
		elector: opts.Elector,
		opt:     opts,
		name:    self.Instance,
		stopped: make(chan struct{}),
		stepReq: make(chan struct{}, 1),
		role:    Follower,
		self:    Info{Instance: self.Instance, URL: self.URL},
		changes: make(chan struct{}),
		led:     make(chan struct{}),
		fs:      FollowerState{State: "no_leader"},
	}
	rt := opts.Transport
	if rt == nil {
		p.transport = newTransport()
		rt = p.transport
	}
	p.client = &http.Client{Transport: rt, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	p.ctx, p.cancel = context.WithCancel(ctx)
	go p.run()
	return p, nil
}

// newTransport is the follower's transport to its leader: on the same host or network, so a
// connection that takes longer than 2 s is not coming; never through a proxy of the
// environment; the stored gzip of a blob is asked for explicitly (no transparent gzip).
func newTransport() *http.Transport {
	dialer := &net.Dialer{Timeout: 2 * time.Second, KeepAlive: 30 * time.Second}
	return &http.Transport{
		Proxy:               nil,
		DialContext:         dialer.DialContext,
		MaxIdleConns:        8,
		MaxIdleConnsPerHost: 4,
		IdleConnTimeout:     90 * time.Second,
		DisableCompression:  true,
	}
}

// Close stops the instance and waits for it: a leader hands over first (the write fence
// closes, the writes in flight end, it resigns and serves its journal until its successor has
// announced itself: at most successorWait after the writes ended, about a second when no
// successor is campaigning), so that the other instance takes over with everything
// acknowledged. Call it before the HTTP server stops listening, and before the
// store closes: the follower writes to it until Close returns.
func (p *Peer) Close() error {
	p.cancel()
	<-p.stopped
	if p.transport != nil {
		p.transport.CloseIdleConnections()
	}
	return nil
}

// crash stops the instance as a process that dies would: the lock file is closed without
// resigning, nothing is handed over (tests).
func (p *Peer) crash() {
	p.mu.Lock()
	p.crashed = true
	term := p.term
	p.mu.Unlock()
	if t, ok := term.(interface{ abandon() }); ok {
		t.abandon()
	}
	p.cancel()
	<-p.stopped
	if p.transport != nil {
		p.transport.CloseIdleConnections()
	}
}

// run is the life of the instance: follow until elected, lead until the term ends, again.
func (p *Peer) run() {
	defer close(p.stopped)
	var hold time.Time // stay out of the election until then
	for {
		term := p.followUntilElected(hold)
		if term == nil {
			return
		}
		if !p.takeOver(term) {
			hold = time.Now().Add(campaignRetry)
			continue
		}
		hold = p.lead(term)
		if p.ctx.Err() != nil {
			return
		}
	}
}

// followUntilElected follows the leader while it campaigns (after hold), until it wins: the
// follower stops before it returns the term. nil when the instance stops.
//
// Log events: leader.campaign_failed (WARN).
func (p *Peer) followUntilElected(hold time.Time) Term {
	fctx, stop := context.WithCancel(p.ctx)
	done := make(chan struct{})
	go func() {
		defer close(done)
		p.follow(fctx)
	}()
	defer func() {
		stop()
		<-done
	}()

	if d := time.Until(hold); d > 0 {
		t := time.NewTimer(d)
		select {
		case <-t.C:
		case <-p.ctx.Done():
			t.Stop()
			return nil
		}
	}
	var failures int
	var logged time.Time
	for {
		term, err := p.elector.Campaign(p.ctx)
		if err == nil {
			if p.ctx.Err() != nil {
				_ = term.Resign()
				return nil
			}
			return term
		}
		if p.ctx.Err() != nil {
			return nil
		}
		failures++
		if failures == 1 || time.Since(logged) >= time.Minute {
			logged = time.Now()
			oplog.For("cluster").Warn("cannot campaign for the leadership", "event", "leader.campaign_failed",
				"elector", electorName(p.elector), "failures", failures, oplog.Err(err))
		}
		if !sleep(p.ctx, campaignRetry) {
			return nil
		}
	}
}

// takeOver begins the term this instance won: it applies what its predecessor still serves,
// makes sure it is not behind the head the last leader recorded, begins its epoch above every
// epoch it knows of, announces itself and opens the write fence. false when it must not lead
// yet (behind the recorded head; see staleWait) or the epoch could not begin or not be
// announced: it has resigned again.
//
// Log events: leader.acquired, leader.deferred (WARN), replica.stale_takeover (ERROR).
func (p *Peer) takeOver(term Term) bool {
	log := oplog.For("cluster")
	prev, known := p.elector.Leader()
	rec, haveRec := p.lastHead()
	var caught catchUpReport
	if known && prev.Instance != p.name && prev.URL != "" {
		caught = p.catchUp(prev)
	}
	// The recorded head names the last leader too; it differs from the announcement only when
	// leader.json was lost or rewritten, and then it is the better source.
	if haveRec && rec.Instance != p.name && rec.URL != "" && (!known || rec.Instance != prev.Instance || rec.URL != prev.URL) &&
		p.behindRecord(rec).behind {
		more := p.catchUp(Info{Instance: rec.Instance, URL: rec.URL, Epoch: rec.Epoch})
		caught = catchUpReport{tried: true, ok: more.ok, applied: caught.applied + more.applied, err: more.err}
	}
	if p.ctx.Err() != nil { // stopped meanwhile: no epoch of its own
		p.endTerm(term)
		return false
	}

	var stale staleness
	if haveRec {
		stale = p.behindRecord(rec)
	}
	if stale.behind && stale.mustWait(rec) {
		p.mu.Lock()
		first := p.staleSince.IsZero()
		if first {
			p.staleSince = time.Now()
		}
		waited := time.Since(p.staleSince)
		p.mu.Unlock()
		attrs := stale.attrs(rec)
		if waited < staleWait {
			if first {
				log.Warn("behind the head the last leader recorded: waiting for it to come back", append([]any{"event", "leader.deferred",
					"instance", p.name, "wait_s", staleWait.Seconds()}, attrs...)...)
			}
			p.endTerm(term)
			return false
		}
		log.Error("leading although behind the head the last leader recorded: what it had and this instance lacks is lost",
			append([]any{"event", "replica.stale_takeover", "instance", p.name, "waited_ms", waited.Milliseconds()}, attrs...)...)
	}
	p.mu.Lock()
	p.staleSince = time.Time{}
	p.mu.Unlock()

	// The epoch goes above every one known: the index's own (StartEpoch), the announced one
	// and the recorded one, which outlives a leader.json that went missing (review 2:
	// epoch-reuse-hides-divergence).
	now := time.Now()
	epoch, _, err := p.st.StartEpoch(p.name, p.self.URL, max(prev.Epoch, rec.Epoch), now)
	if err != nil {
		log.Error("cannot begin an epoch: resigning", "event", "leader.campaign_failed", "elector", electorName(p.elector), oplog.Err(err))
		_ = term.Resign()
		return false
	}
	info := Info{Instance: p.name, URL: p.self.URL, Epoch: epoch, Since: now.UTC()}
	if err := p.elector.Announce(info); err != nil {
		log.Error("cannot announce the leadership: resigning", "event", "leader.campaign_failed", "elector", electorName(p.elector),
			"epoch", epoch, oplog.Err(err))
		_ = term.Resign()
		return false
	}
	p.mu.Lock()
	select {
	case <-p.stepReq: // asked of an earlier term
	default:
	}
	p.role, p.writable, p.self, p.term = Leader, true, info, term
	p.fs, p.behindSince = FollowerState{BlobsMissing: p.fs.BlobsMissing}, time.Time{}
	switch {
	case known && prev.Instance != p.name && prev.URL != "":
		p.peer = prev
	case haveRec && rec.Instance != p.name && rec.URL != "":
		p.peer = Info{Instance: rec.Instance, URL: rec.URL, Epoch: rec.Epoch}
	}
	close(p.led)
	p.broadcastLocked()
	p.mu.Unlock()
	p.recordHead(false)

	attrs := []any{"event", "leader.acquired", "epoch", epoch, "instance", p.name, "url", info.URL, "elector", electorName(p.elector)}
	if known {
		attrs = append(attrs, "previous", prev.Instance, "previous_epoch", prev.Epoch)
	}
	if caught.tried {
		attrs = append(attrs, "caught_up", caught.ok, "caught_up_entries", caught.applied)
		if caught.err != nil {
			attrs = append(attrs, "catch_up_error", caught.err.Error())
		}
	}
	if stale.behind {
		attrs = append(attrs, stale.attrs(rec)...)
	}
	log.Info("leading", attrs...)
	return true
}

// lastHead is the elector's HeadRecord, if it keeps one.
func (p *Peer) lastHead() (HeadRecord, bool) {
	if b, ok := p.elector.(headBook); ok {
		return b.LastHead()
	}
	return HeadRecord{}, false
}

// staleness is how a store compares with a HeadRecord.
type staleness struct {
	behind   bool      // the store lacks the recorded entry
	diverged bool      // it has another entry at the recorded seq
	entries  int64     // how many entries it lacks (diverged: unknown, 0)
	seq      int64     // its newest entry
	at       time.Time // when that was written
}

// behindRecord compares the store with rec. A store that cannot be read counts as not behind:
// the record protects data, it must not keep the pair from electing.
func (p *Peer) behindRecord(rec HeadRecord) staleness {
	seq, _, at := p.st.Head()
	s := staleness{seq: seq, at: at}
	switch {
	case seq < rec.Seq:
		s.behind, s.entries = true, rec.Seq-seq
	case rec.Sum != "":
		if e, err := p.st.JournalEntryAt(rec.Seq); err == nil && e.Sum() != rec.Sum {
			s.behind, s.diverged = true, true
		}
	}
	return s
}

// mustWait says whether a candidate this far behind rec has to wait for rec's writer: always
// after a hand-over or a shutdown (the writer is intact and comes back), and after a crash when
// it would lose more than the last acceptedLoss.
func (s staleness) mustWait(rec HeadRecord) bool {
	return rec.Final || s.at.IsZero() || rec.At.Sub(s.at) > acceptedLoss
}

func (s staleness) attrs(rec HeadRecord) []any {
	attrs := []any{"behind_entries", s.entries, "seq", s.seq, "recorded_by", rec.Instance, "recorded_seq", rec.Seq,
		"recorded_epoch", rec.Epoch, "recorded_final", rec.Final}
	if !rec.At.IsZero() && !s.at.IsZero() {
		attrs = append(attrs, "behind_seconds", max(0, rec.At.Sub(s.at).Seconds()))
	}
	if s.diverged {
		attrs = append(attrs, "diverged", true)
	}
	return attrs
}

// catchUpReport is what a new leader's catch-up did.
type catchUpReport struct {
	tried, ok bool
	applied   int64
	err       error
}

// catchUp applies what the previous leader has and this instance lacks, before this one begins
// its own epoch. A previous leader that handed over serves its journal and blobs until it sees
// its successor's announcement (successorWait), so the catch-up goes on until it has the
// predecessor's head, however long a large blob takes, or until the predecessor stops
// answering: a predecessor that is gone refuses at once, and what it had not replicated is
// lost (owner, 2026-10-02: the last minutes before a crash may be). catchUpMax bounds it.
func (p *Peer) catchUp(prev Info) catchUpReport {
	ctx, cancel := context.WithTimeout(p.ctx, catchUpMax)
	defer cancel()
	r := newReplica(p, true)
	before, _ := p.st.Position()
	rep := catchUpReport{tried: true}
	for ctx.Err() == nil {
		caughtUp, err := r.stream(ctx, prev, true)
		if err == nil && caughtUp {
			rep.ok, rep.err = true, nil
			break
		}
		if err == nil {
			continue // a snapshot, or a page answered ahead of the store: again from the store's position
		}
		rep.err = err
		if ctx.Err() != nil || stopsAnswering(err) {
			break
		}
		if !sleep(ctx, catchUpRetry) {
			break
		}
	}
	if rep.err == nil && !rep.ok {
		rep.err = ctx.Err()
	}
	after, _ := p.st.Position()
	rep.applied = max(0, after-before)
	return rep
}

// stopsAnswering says whether err means that the instance asked is no longer there to ask:
// its port refuses connections (or the dial fails otherwise), or it no longer leads.
func stopsAnswering(err error) bool {
	var op *net.OpError
	if errors.As(err, &op) && op.Op == "dial" {
		return true
	}
	if errors.Is(err, syscall.ECONNREFUSED) {
		return true
	}
	var serr *statusError
	return errors.As(err, &serr) && serr.code == "not-leader"
}

// lead runs the term: it records the head and checks the announcement every headEvery, and
// fetches the blobs the index lacks from the other instance, until a step-down, the term lost,
// or the instance stopping. It returns until when the instance stays out of the next election.
func (p *Peer) lead(term Term) time.Time {
	bctx, stopBackfill := context.WithCancel(p.ctx)
	backfilled := make(chan struct{})
	go func() {
		defer close(backfilled)
		p.backfillAsLeader(bctx)
	}()
	stop := func() {
		stopBackfill()
		<-backfilled
	}
	tick := time.NewTicker(headEvery)
	defer tick.Stop()
	for {
		select {
		case <-term.Lost():
			stop()
			p.lose(term)
			return time.Time{}
		case <-p.stepReq:
			stop()
			p.handOver(term, "step-down")
			return time.Now().Add(p.opt.StepDownPause)
		case <-p.ctx.Done():
			stop()
			p.mu.Lock()
			crashed := p.crashed
			p.mu.Unlock()
			if crashed {
				p.endTerm(term)
			} else {
				p.handOver(term, "shutdown")
			}
			return time.Time{}
		case <-tick.C:
			p.recordHead(false)
			p.reannounce()
		}
	}
}

// recordHead records the newest journal entry with the elector (HeadRecord), when it changed
// since the last record; final after the last write of the term. It returns the record.
//
// Log events: leader.record_failed (WARN).
func (p *Peer) recordHead(final bool) HeadRecord {
	b, ok := p.elector.(headBook)
	if !ok {
		return HeadRecord{}
	}
	seq, _, _, err := p.st.LastSum()
	var e store.JournalEntry
	if err == nil {
		e, err = p.st.JournalEntryAt(seq)
	}
	if err != nil {
		oplog.For("cluster").Warn("cannot read the head to record", "event", "leader.record_failed", oplog.Err(err))
		return HeadRecord{}
	}
	p.mu.Lock()
	rec := HeadRecord{Instance: p.name, URL: p.self.URL, Epoch: e.Epoch, Seq: e.Seq, Sum: e.Sum(), At: e.At.UTC(), Final: final}
	same := rec == p.recorded
	p.mu.Unlock()
	if same {
		return rec
	}
	if err := b.RecordHead(rec); err != nil {
		oplog.For("cluster").Warn("cannot record the head", "event", "leader.record_failed", "seq", rec.Seq, "final", final, oplog.Err(err))
		return rec
	}
	p.mu.Lock()
	p.recorded = rec
	p.mu.Unlock()
	return rec
}

// reannounce announces this instance again when the announcement is missing or names somebody
// else while it leads: without it the follower would wait for a leader for ever.
//
// Log events: leader.reannounced (WARN).
func (p *Peer) reannounce() {
	p.mu.Lock()
	self, leading := p.self, p.role == Leader && p.writable
	p.mu.Unlock()
	if !leading {
		return
	}
	l, ok := p.elector.Leader()
	if ok && l.Instance == self.Instance && l.URL == self.URL && l.Epoch == self.Epoch {
		return
	}
	found := "nothing"
	if ok {
		found = fmt.Sprintf("%s (%s) epoch %d", l.Instance, l.URL, l.Epoch)
	}
	err := p.elector.Announce(self)
	attrs := []any{"event", "leader.reannounced", "instance", self.Instance, "epoch", self.Epoch, "found", found}
	if err != nil {
		attrs = append(attrs, oplog.Err(err))
	}
	oplog.For("cluster").Warn("the announcement of the leadership was missing or named another: announced again", attrs...)
}

// endTerm ends a term without a hand-over: resigned, or after a crash (tests) its lock file
// closed as a dying process's would be.
func (p *Peer) endTerm(term Term) {
	p.mu.Lock()
	crashed := p.crashed
	p.mu.Unlock()
	if t, ok := term.(interface{ abandon() }); ok && crashed {
		t.abandon()
		return
	}
	_ = term.Resign()
}

// closeFence closes the write fence: BeginWrite refuses from now on, and WaitLeader waits. The
// channel is closed when the writes in flight have ended.
func (p *Peer) closeFence() <-chan struct{} {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.writable = false
	select {
	case <-p.led:
		p.led = make(chan struct{})
	default:
	}
	ch := make(chan struct{})
	if p.writers == 0 {
		close(ch)
	} else {
		p.drained = ch
	}
	return ch
}

// waitWrites waits for every write in flight, however long it runs (see fenceLog), and logs
// every fenceLog that it still waits.
//
// Log events: leader.fence_wait (WARN).
func (p *Peer) waitWrites(drained <-chan struct{}, reason string) {
	start := time.Now()
	t := time.NewTicker(fenceLog)
	defer t.Stop()
	for {
		select {
		case <-drained:
			return
		case <-t.C:
			p.mu.Lock()
			n := p.writers
			p.mu.Unlock()
			oplog.For("cluster").Warn("waiting for the writes in flight before the term ends", "event", "leader.fence_wait",
				"reason", reason, "writes", n, "waited_ms", time.Since(start).Milliseconds())
		}
	}
}

// handOver ends a term on purpose: the fence closes, every write in flight ends, the final head
// is recorded, the lock is released, and the instance serves its journal and blobs as a leader
// that writes nothing until its successor has announced itself, which it does once its
// catch-up has the whole journal (successorWait), so that the successor gets every write this
// instance acknowledged. Then it follows.
//
// Log events: leader.stepped_down.
func (p *Peer) handOver(term Term, reason string) {
	log := oplog.For("cluster")
	start := time.Now()
	p.waitWrites(p.closeFence(), reason)
	p.mu.Lock()
	epoch := p.self.Epoch
	p.mu.Unlock()
	head := p.recordHead(true)
	resignErr := term.Resign()
	successor, outcome := p.awaitSuccessor(epoch, successorWait)

	p.mu.Lock()
	p.role, p.term = Follower, nil
	p.fs, p.behindSince = FollowerState{State: "no_leader", BlobsMissing: p.fs.BlobsMissing}, time.Now()
	p.broadcastLocked()
	if p.stepping != nil {
		close(p.stepping)
		p.stepping = nil
	}
	p.mu.Unlock()

	attrs := []any{"event", "leader.stepped_down", "reason", reason, "epoch", epoch, "instance", p.name, "head_seq", head.Seq,
		"successor_wait", outcome, "duration_ms", time.Since(start).Milliseconds()}
	if outcome == "announced" {
		attrs = append(attrs, "successor", successor.Instance, "successor_epoch", successor.Epoch)
	} else {
		attrs = append(attrs, "successor", "")
	}
	if resignErr != nil {
		attrs = append(attrs, oplog.Err(resignErr))
	}
	log.Info("stepped down", attrs...)
}

// awaitSuccessor waits up to d for the announcement of a leader of a later epoch than epoch:
// outcome "announced"; "none" when the elector says that nobody has held the lock for
// noSuccessorAfter (no candidate is catching up from here); "timeout" else.
func (p *Peer) awaitSuccessor(epoch int64, d time.Duration) (Info, string) {
	deadline := time.Now().Add(d)
	probe, _ := p.elector.(lockProbe)
	var free, probed time.Time // nobody held the lock since; the last probe
	for {
		if l, ok := p.elector.Leader(); ok && l.Epoch > epoch && l.Instance != p.name {
			return l, "announced"
		}
		now := time.Now()
		if now.After(deadline) {
			return Info{}, "timeout"
		}
		if probe != nil && now.Sub(probed) >= lockProbeEvery {
			probed = now
			switch held, err := probe.Held(); {
			case err != nil || held:
				free = time.Time{}
			case free.IsZero():
				free = now
			case now.Sub(free) >= noSuccessorAfter:
				return Info{}, "none"
			}
		}
		time.Sleep(successorPoll)
	}
}

// lose ends a term that the elector says is lost: another instance may lead already, so the
// role changes at once. Following waits for every write in flight (lead returns only then): a
// write of this term that committed after the next leader's epoch entry was applied here
// would be stamped with that epoch and look like the next leader's own (review 2:
// stray-write-takes-new-leaders-epoch). Committed before, it keeps this term's epoch, and the
// next leader's journal check finds the divergence.
//
// Log events: leader.lost (WARN).
func (p *Peer) lose(term Term) {
	drained := p.closeFence()
	p.mu.Lock()
	epoch := p.self.Epoch
	p.role, p.term = Follower, nil
	p.fs, p.behindSince = FollowerState{State: "no_leader", BlobsMissing: p.fs.BlobsMissing}, time.Now()
	p.broadcastLocked()
	if p.stepping != nil {
		close(p.stepping)
		p.stepping = nil
	}
	p.mu.Unlock()
	oplog.For("cluster").Warn("lost the leadership", "event", "leader.lost", "epoch", epoch, "instance", p.name,
		"elector", electorName(p.elector))
	p.waitWrites(drained, "lost")
	_ = term.Resign()
}

// broadcastLocked tells LeaderChanges' readers that the role changed. The caller holds mu.
func (p *Peer) broadcastLocked() {
	close(p.changes)
	p.changes = make(chan struct{})
}

// Role is Leader from the moment the epoch began until the successor announced itself after a
// step-down (while handing over, BeginWrite refuses), else Follower.
func (p *Peer) Role() Role {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.role
}

// Self is this instance; Epoch and Since are those of its term while it leads.
func (p *Peer) Self() Info {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.role == Leader {
		return p.self
	}
	return Info{Instance: p.name, URL: p.self.URL}
}

// Leader is this instance while it leads, else the leader the elector announced, unless that is
// this instance (an announcement of its own earlier term, or its own while it hands over).
func (p *Peer) Leader() (Info, bool) {
	p.mu.Lock()
	if p.role == Leader && p.writable {
		self := p.self
		p.mu.Unlock()
		return self, true
	}
	p.mu.Unlock()
	return p.announced()
}

// announced is the leader the elector announced, other than this instance.
func (p *Peer) announced() (Info, bool) {
	l, ok := p.elector.Leader()
	if !ok || l.Instance == p.name || l.URL == "" {
		return Info{}, false
	}
	return l, true
}

// WaitLeader waits until this instance leads with its write fence open, d passes or ctx ends.
func (p *Peer) WaitLeader(ctx context.Context, d time.Duration) bool {
	p.mu.Lock()
	led := p.led
	p.mu.Unlock()
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-led:
		return true
	case <-t.C:
	case <-ctx.Done():
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.role == Leader && p.writable
}

// BeginWrite admits a write while the fence is open; the write calls done when it has
// committed. A step-down waits for every done before it resigns.
func (p *Peer) BeginWrite() (func(), bool) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.role != Leader || !p.writable {
		return nil, false
	}
	p.writers++
	var once sync.Once
	return func() {
		once.Do(func() {
			p.mu.Lock()
			defer p.mu.Unlock()
			p.writers--
			if p.writers == 0 && p.drained != nil {
				close(p.drained)
				p.drained = nil
			}
		})
	}, true
}

// StepDown hands the leadership over (see handOver) and returns when this instance follows;
// it stays out of the election for StepDownPause. ErrNotLeader on a follower.
func (p *Peer) StepDown() error {
	p.mu.Lock()
	if p.role != Leader {
		p.mu.Unlock()
		return ErrNotLeader
	}
	if p.stepping == nil {
		p.stepping = make(chan struct{})
		select {
		case p.stepReq <- struct{}{}:
		default:
		}
	}
	ch := p.stepping
	p.mu.Unlock()
	select {
	case <-ch:
	case <-p.stopped:
	}
	return nil
}

// Follower is how this instance keeps up with its leader. While it leads, only BlobsMissing:
// the blobs its index references that it lacks (a follower promoted before its back-fill
// ended) and fetches from the other instance (E2E-4).
//
// State is "following" while the lag is under a second, with hysteresis: a follower that fell
// behind by a second or more is "catching_up" until it is under half a second again; and only
// once it has counted the blobs it lacks (a hand-over waits for 0). LagEntries is how many
// entries the leader's head, as its last answer named it, is ahead of the store (E2E-6).
func (p *Peer) Follower() FollowerState {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.role == Leader {
		return FollowerState{BlobsMissing: p.fs.BlobsMissing}
	}
	fs := p.fs
	fs.LagSeconds = p.lagLocked(time.Now()).Seconds()
	if fs.State == "catching_up" && p.inSyncLocked(time.Now(), false) && p.counted {
		fs.State = "following"
	}
	return fs
}

// lagLocked is how far the follower is behind: since when it has not had the leader's oldest
// entry it lacks. The caller holds mu.
func (p *Peer) lagLocked(now time.Time) time.Duration {
	if p.behindSince.IsZero() {
		return 0
	}
	return max(0, now.Sub(p.behindSince))
}

// inSyncLocked updates and returns the hysteresis of "following". Only an answer of the leader
// (answered) can make it true again: before the follower has heard from the leader, its lag
// is a guess. The caller holds mu.
func (p *Peer) inSyncLocked(now time.Time, answered bool) bool {
	lag := p.lagLocked(now)
	if lag >= behindLogAfter {
		p.behindLong = true
	}
	switch {
	case p.inSync && lag >= followingBelow:
		p.inSync = false
	case !p.inSync && answered && lag < resumeBelow:
		p.inSync = true
	}
	return p.inSync
}

// Peer is the other instance of the pair as this one last saw it: the leader it followed, or
// the predecessor of its term. A leader reads the blobs it lacks from there. false when it has
// not seen one.
func (p *Peer) Peer() (Info, bool) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.peer.URL == "" || p.peer.Instance == p.name {
		return Info{}, false
	}
	return p.peer, true
}

// LeaderChanges is closed (and replaced) when the role of this instance changes.
func (p *Peer) LeaderChanges() <-chan struct{} {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.changes
}

// sleep waits d or until ctx ends: false when it ended.
func sleep(ctx context.Context, d time.Duration) bool {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-t.C:
		return true
	case <-ctx.Done():
		return false
	}
}
