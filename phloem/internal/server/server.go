// Package server is Phloem's service (docs/phloem/protocol.md): it makes and deletes the spaces
// of the profiles that sync, keeps the parcels for devices to come, and hands the sealed entries
// of a space between its devices, live, over one WebSocket each. It opens nothing it keeps, and
// holds every limit of the contract itself, whatever a client does.
//
// Its numbers go to a file (the stats file, written every 15 s), not to an endpoint: a network
// the monitoring could ask Phloem over would also let Phloem reach the monitoring.
package server

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/phloem/internal/metrics"
	"github.com/leonieziechmann/betula/phloem/internal/oplog"
	"github.com/leonieziechmann/betula/phloem/internal/store"
)

// DefaultBudget is what all spaces together may hold before no new space or parcel is taken:
// 50 GB (owner, 2026-11-05: „<50gb für profile kein problem").
const DefaultBudget int64 = 50_000_000_000

// DefaultStatsInterval is how often the stats file is written.
const DefaultStatsInterval = 15 * time.Second

// Limits are Phloem's bounds. A zero field takes its value of DefaultLimits, which are the
// contract's; the tests make them small.
type Limits struct {
	MaxEntry    int   // the decoded data of a push
	MaxSnapshot int   // the decoded data of a snapshot
	MaxBox      int   // the decoded box of a parcel
	MaxPushID   int   // the characters of a push's id
	SoftTail    int64 // data of the entries after the snapshot above which the pusher hears compact
	HardTail    int64 // ... beyond which a push waits for a snapshot (compact_first)

	PushRate  float64 // pushes a second and space; a snapshot counts one for every 64 KiB begun
	PushBurst float64 // pushes at once

	MaxDevices int // slots of a space
	MaxSockets int // sockets of a space
	MaxConns   int // sockets of the instance
	MaxWaiting int // sockets of one address that have not said hello yet

	SpacesPerAddr int           // new spaces of one address in CreateWindow
	SpacesTotal   int           // new spaces in all in CreateWindow
	CreateWindow  time.Duration //
	TakesPerAddr  int           // parcels one address may ask for in an hour (found or not)

	PairTTL  time.Duration // how long a parcel waits
	MaxPairs int           // parcels a space may have waiting
	Expiry   time.Duration // a space not touched for this long is deleted

	HelloTimeout time.Duration // a socket's hello comes within it
	FrameTimeout time.Duration // a frame, once begun, arrives within it
	WriteTimeout time.Duration // a frame to a device is written within it
	PingInterval time.Duration // a ping to every device this often ...
	PingTimeout  time.Duration // ... and its pong within this, or the socket closes
	SweepEvery   time.Duration // expired spaces and parcels go this often

	BigFrames   int   // frames above 128 KiB in memory at a time, read or written
	QueueFrames int   // frames waiting for one socket; a socket further behind is closed
	QueueBytes  int64 // their bytes
}

// DefaultLimits are the contract's limits (docs/phloem/protocol.md) and Phloem's own beside them
// (docs/phloem/phloem.md, „Limits").
var DefaultLimits = Limits{
	MaxEntry:    64 << 10,
	MaxSnapshot: 6 << 20,
	MaxBox:      4 << 10,
	MaxPushID:   64,
	SoftTail:    128 << 10,
	HardTail:    256 << 10,

	PushRate:  50,
	PushBurst: 200,

	MaxDevices: 16,
	MaxSockets: 16,
	MaxConns:   4096,
	MaxWaiting: 32,

	SpacesPerAddr: 20,
	SpacesTotal:   500,
	CreateWindow:  time.Hour,
	TakesPerAddr:  120,

	PairTTL:  10 * time.Minute,
	MaxPairs: 16,
	Expiry:   395 * 24 * time.Hour,

	HelloTimeout: 10 * time.Second,
	FrameTimeout: time.Minute,
	WriteTimeout: 30 * time.Second,
	PingInterval: 30 * time.Second,
	PingTimeout:  15 * time.Second,
	SweepEvery:   time.Minute,

	BigFrames:   8,
	QueueFrames: 512,
	QueueBytes:  16 << 20,
}

func (l Limits) withDefaults() Limits {
	d := DefaultLimits
	pick := func(v *int, def int) {
		if *v <= 0 {
			*v = def
		}
	}
	pick64 := func(v *int64, def int64) {
		if *v <= 0 {
			*v = def
		}
	}
	pickF := func(v *float64, def float64) {
		if *v <= 0 {
			*v = def
		}
	}
	pickD := func(v *time.Duration, def time.Duration) {
		if *v <= 0 {
			*v = def
		}
	}
	pick(&l.MaxEntry, d.MaxEntry)
	pick(&l.MaxSnapshot, d.MaxSnapshot)
	pick(&l.MaxBox, d.MaxBox)
	pick(&l.MaxPushID, d.MaxPushID)
	pick64(&l.SoftTail, d.SoftTail)
	pick64(&l.HardTail, d.HardTail)
	pickF(&l.PushRate, d.PushRate)
	pickF(&l.PushBurst, d.PushBurst)
	pick(&l.MaxDevices, d.MaxDevices)
	pick(&l.MaxSockets, d.MaxSockets)
	pick(&l.MaxConns, d.MaxConns)
	pick(&l.MaxWaiting, d.MaxWaiting)
	pick(&l.SpacesPerAddr, d.SpacesPerAddr)
	pick(&l.SpacesTotal, d.SpacesTotal)
	pickD(&l.CreateWindow, d.CreateWindow)
	pick(&l.TakesPerAddr, d.TakesPerAddr)
	pickD(&l.PairTTL, d.PairTTL)
	pick(&l.MaxPairs, d.MaxPairs)
	pickD(&l.Expiry, d.Expiry)
	pickD(&l.HelloTimeout, d.HelloTimeout)
	pickD(&l.FrameTimeout, d.FrameTimeout)
	pickD(&l.WriteTimeout, d.WriteTimeout)
	pickD(&l.PingInterval, d.PingInterval)
	pickD(&l.PingTimeout, d.PingTimeout)
	pickD(&l.SweepEvery, d.SweepEvery)
	pick(&l.BigFrames, d.BigFrames)
	pick(&l.QueueFrames, d.QueueFrames)
	pick64(&l.QueueBytes, d.QueueBytes)
	return l
}

// readLimit is the largest frame a device may send after its hello: a snapshot of MaxSnapshot
// bytes in base64url with its JSON around it fits (about 8.5 MiB for 6 MiB); a larger frame
// closes the socket (1009).
func (l Limits) readLimit() int64 {
	return int64(b64len(max(l.MaxSnapshot, l.MaxEntry))) + 512<<10
}

// Options configure a Server.
type Options struct {
	Store *store.Store

	// Origins are the browser origins taken, as scheme://host[:port], exactly. Empty: the
	// origin whose host is the request's own (the site's own origin behind Traefik).
	Origins []string

	// Budget is what all spaces together may hold; at it, no new space or parcel (507). 0:
	// DefaultBudget.
	Budget int64

	// StatsDir is where phloem.prom is written; "": nowhere.
	StatsDir string
	// StatsInterval: 0 is DefaultStatsInterval.
	StatsInterval time.Duration
	// Stack, when set, is the label stack="…" of every series of the stats file, so that two
	// instances read by one Alloy stay apart.
	Stack string

	Version, Build string

	Limits Limits

	// Registry holds the metrics; nil: a registry of the server's own (the tests).
	Registry *metrics.Registry

	// Now is the clock; nil: time.Now.
	Now func() time.Time
}

// LogProblems counts the log's WARN and ERROR records by level and event; cmd/phloem hands it
// to oplog. It lives in metrics.Default, which serve writes the stats file from.
var LogProblems = metrics.Default.NewCounter("phloem_log_problems_total",
	"Log records at WARN or ERROR, by level and event.", "level", "event")

// Server is one Phloem. Close ends it.
type Server struct {
	st      *store.Store
	opt     Options
	lim     Limits
	now     func() time.Time
	budget  int64
	origins map[string]bool
	reg     *metrics.Registry
	m       meters
	started time.Time

	ctx    context.Context // ends with Close: every socket and store call ends
	cancel context.CancelFunc
	stop   chan struct{} // ends the loops
	up     atomic.Bool
	big    chan struct{} // the frames above bigFrame in memory

	creates   *limiter // new spaces by address
	createAll *limiter // new spaces in all (one key)
	takes     *limiter // parcels asked for by address
	pushes    *limiter // pushes and snapshots by space

	devices atomic.Int64 // devices of every space, as of the last tick

	mu          sync.Mutex
	rooms       map[string]*room // the spaces with sockets, by space id
	conns       int              // open sockets
	waiting     map[string]int   // sockets before their hello, by address
	shut        bool
	budgetLevel int // 0 below half the budget, 1 above, 2 at it: logged when it changes
	statsFailed bool

	wg sync.WaitGroup // the loops, the sockets and their writers
}

var stackName = regexp.MustCompile(`^[A-Za-z0-9_.-]*$`)

// New starts a server on opt.Store: its loops run until Close.
func New(opt Options) (*Server, error) {
	if opt.Store == nil {
		return nil, errors.New("no store")
	}
	if !stackName.MatchString(opt.Stack) {
		return nil, fmt.Errorf("invalid stack name %q", opt.Stack)
	}
	origins := map[string]bool{}
	for _, raw := range opt.Origins {
		o, err := normalizeOrigin(raw)
		if err != nil {
			return nil, err
		}
		origins[o] = true
	}
	if opt.Budget <= 0 {
		opt.Budget = DefaultBudget
	}
	if opt.StatsInterval <= 0 {
		opt.StatsInterval = DefaultStatsInterval
	}
	if opt.Now == nil {
		opt.Now = time.Now
	}
	reg := opt.Registry
	if reg == nil {
		reg = metrics.NewRegistry()
	}
	lim := opt.Limits.withDefaults()
	ctx, cancel := context.WithCancel(context.Background())
	s := &Server{
		st:        opt.Store,
		opt:       opt,
		lim:       lim,
		now:       opt.Now,
		budget:    opt.Budget,
		origins:   origins,
		reg:       reg,
		started:   time.Now(),
		ctx:       ctx,
		cancel:    cancel,
		stop:      make(chan struct{}),
		big:       make(chan struct{}, lim.BigFrames),
		creates:   perWindow(lim.SpacesPerAddr, lim.CreateWindow),
		createAll: perWindow(lim.SpacesTotal, lim.CreateWindow),
		takes:     perWindow(lim.TakesPerAddr, time.Hour),
		pushes:    newLimiter(lim.PushRate, lim.PushBurst),
		rooms:     map[string]*room{},
		waiting:   map[string]int{},
	}
	s.up.Store(true)
	s.declare()
	s.wg.Add(2)
	go s.sweepLoop()
	go s.statsLoop()
	return s, nil
}

// normalizeOrigin checks an origin of Options.Origins: scheme://host[:port], lower case.
func normalizeOrigin(raw string) (string, error) {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" ||
		(u.Path != "" && u.Path != "/") || u.RawQuery != "" || u.Fragment != "" || u.User != nil {
		return "", fmt.Errorf("invalid origin %q: expected scheme://host[:port]", raw)
	}
	return strings.ToLower(u.Scheme + "://" + u.Host), nil
}

// closeGrace is how long Close waits for the sockets to close by themselves (they hear 1001
// first) before it cuts them.
const closeGrace = 10 * time.Second

// Close tells every device to come back later (1001), ends the loops, waits for the sockets
// and writes the stats file a last time, with phloem_up 0. The store stays open.
func (s *Server) Close() error {
	s.mu.Lock()
	if s.shut {
		s.mu.Unlock()
		return nil
	}
	s.shut = true
	rooms := make([]*room, 0, len(s.rooms))
	for _, r := range s.rooms {
		rooms = append(rooms, r)
	}
	s.mu.Unlock()
	for _, r := range rooms {
		r.mu.Lock()
		for _, p := range r.peers {
			p.send(outFrame{last: true, code: statusGoingAway, reason: "restart"})
		}
		r.mu.Unlock()
	}
	close(s.stop)
	done := make(chan struct{})
	go func() {
		s.wg.Wait()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(closeGrace):
		s.cancel()
		<-done
	}
	s.cancel()
	s.up.Store(false)
	if s.opt.StatsDir != "" {
		_ = s.writeStats()
	}
	return nil
}

// overBudget says whether all spaces together hold the budget.
func (s *Server) overBudget() bool {
	_, used := s.st.Totals()
	return used >= s.budget
}

// checkBudget logs when the bytes held cross half the budget or the budget.
// Log events: budget.half (WARN), budget.reached (ERROR), budget.recovered.
func (s *Server) checkBudget() {
	_, used := s.st.Totals()
	level := 0
	switch {
	case used >= s.budget:
		level = 2
	case used*2 >= s.budget:
		level = 1
	}
	s.mu.Lock()
	prev := s.budgetLevel
	s.budgetLevel = level
	s.mu.Unlock()
	if level == prev {
		return
	}
	log := oplog.For("budget")
	attrs := []any{"bytes", used, "budget", s.budget}
	switch {
	case level == 2:
		log.Error("the budget is reached: no new spaces or parcels", append(attrs, "event", "budget.reached")...)
	case level == 1 && prev == 0:
		log.Warn("half of the budget is used", append(attrs, "event", "budget.half")...)
	default:
		log.Info("below the budget again", append(attrs, "event", "budget.recovered", "level", level)...)
	}
}

func (s *Server) sweepLoop() {
	defer s.wg.Done()
	t := time.NewTicker(s.lim.SweepEvery)
	defer t.Stop()
	for {
		select {
		case <-s.stop:
			return
		case <-t.C:
			s.sweep()
		}
	}
}

// sweep deletes the spaces not touched for Limits.Expiry (their devices online hear gone) and
// the parcels that expired, and forgets the limiters' full buckets.
// Log events: sweep.done, store.failed (ERROR).
func (s *Server) sweep() {
	now := s.now()
	swept, err := s.st.Sweep(s.ctx, now, now.Add(-s.lim.Expiry))
	if err != nil {
		if s.ctx.Err() == nil {
			oplog.For("store").Error("the sweep failed", "event", "store.failed", "op", "sweep", oplog.Err(err))
		}
		return
	}
	for _, id := range swept.Spaces {
		s.m.deleted.Inc("expired")
		s.notifyGone(id)
	}
	if swept.Pairs > 0 {
		s.m.parcels.Add(float64(swept.Pairs), "expired")
	}
	if len(swept.Spaces) > 0 || swept.Pairs > 0 {
		oplog.For("store").Info("swept", "event", "sweep.done", "spaces", len(swept.Spaces), "parcels", swept.Pairs)
	}
	for _, l := range []*limiter{s.creates, s.createAll, s.takes, s.pushes} {
		l.prune(now)
	}
}

func (s *Server) statsLoop() {
	defer s.wg.Done()
	s.tick()
	t := time.NewTicker(s.opt.StatsInterval)
	defer t.Stop()
	for {
		select {
		case <-s.stop:
			return
		case <-t.C:
			s.tick()
		}
	}
}

// tick counts the devices, looks at the budget and writes the stats file.
// Log events: stats.failed (WARN, once until it works again), stats.recovered.
func (s *Server) tick() {
	if n, err := s.st.Devices(s.ctx); err == nil {
		s.devices.Store(n)
	}
	s.checkBudget()
	if s.opt.StatsDir == "" {
		return
	}
	err := s.writeStats()
	s.mu.Lock()
	was := s.statsFailed
	s.statsFailed = err != nil
	s.mu.Unlock()
	switch {
	case err != nil && !was:
		oplog.For("stats").Warn("cannot write the stats file", "event", "stats.failed", "dir", s.opt.StatsDir, oplog.Err(err))
	case err == nil && was:
		oplog.For("stats").Info("the stats file is written again", "event", "stats.recovered", "dir", s.opt.StatsDir)
	}
}

// storeFailed logs a store call that failed (not one that ended with the server).
func (s *Server) storeFailed(op string, err error) {
	if s.ctx.Err() != nil {
		return
	}
	oplog.For("store").Error("a store call failed", "event", "store.failed", "op", op, oplog.Err(err))
}
