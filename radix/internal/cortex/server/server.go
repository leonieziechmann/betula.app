// Package server is Cortex's HTTP API (docs/cortex/cortex.md): fetching through the cache
// (GET /v1/fetch), what the store holds (/v1/entries), named files (/v1/files), blobs by hash
// (/v1/blobs), health and status, the administration of the pair, and the leader's side of
// the replication (/internal/v1/journal, /internal/v1/snapshot).
//
// The server codes against cluster.Node: while this instance leads, it fetches upstream and
// writes; while it follows, it serves what its copy has and forwards the rest to the leader.
// Every write to the index runs inside Node.BeginWrite, so that a step-down waits for it.
//
// Log events: http.listening, http.request, http.failed, upstream.fetched, upstream.failed
// (WARN; INFO when Cortex itself refused the request), blob.rejected (WARN), retention.failed
// (WARN).
package server

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
	"github.com/leonieziechmann/betula/radix/internal/cortex/upstream"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// The defaults of Options (spec §8).
const (
	DefaultHistory       = 4320 * time.Hour // 180 days
	DefaultJournalKeep   = 168 * time.Hour
	DefaultBlobGrace     = 168 * time.Hour
	DefaultPruneInterval = time.Hour
	DefaultLeaderWait    = 2 * time.Second
)

// Shutdown bounds: Serve gives the requests in flight ShutdownGrace to finish, then Close gives
// what runs detached (fetches, retention) closeGrace; together within the 25 s a container
// stop allows before the 30 s of swarm's stop_grace_period.
const (
	ShutdownGrace = 20 * time.Second
	closeGrace    = 5 * time.Second
)

// noLeaderAfter is how long no leader may be known before /healthz reports it.
const noLeaderAfter = 60 * time.Second

// maxLagSeconds is how far a follower may be behind before /healthz reports it (owner,
// 2026-10-02: the last five minutes may be lost).
const maxLagSeconds = 300

// Options configure a Server.
type Options struct {
	Store    *store.Store       // required
	Upstream *upstream.Upstream // required
	Node     cluster.Node       // required

	Version string // the release, for GET /status
	Build   string // the binary (version.Build), for GET /status

	History       time.Duration // a superseded version is kept this long; 0: DefaultHistory
	JournalKeep   time.Duration // journal entries are kept this long; 0: DefaultJournalKeep
	BlobGrace     time.Duration // an unreferenced blob is kept this long; 0: DefaultBlobGrace
	PruneInterval time.Duration // retention runs this often; 0: DefaultPruneInterval; < 0: never

	// Recorder is the log's recorder of recent problems, shown in GET /status; nil: none.
	Recorder *oplog.Recorder

	// LeaderWait is how long a follower waits to become the leader itself when it knows no
	// leader or cannot reach it; 0: DefaultLeaderWait.
	LeaderWait time.Duration

	// ForwardTransport carries the requests a follower forwards to the leader; nil: a
	// transport of the server's own (2 s to connect, no proxy, no compression of its own).
	ForwardTransport http.RoundTripper

	// Now is the clock of the fetch times and of the ages; nil: time.Now.
	Now func() time.Time
}

// Server is the HTTP API of one instance. It is safe for concurrent use.
type Server struct {
	st      *store.Store
	up      *upstream.Upstream
	node    cluster.Node
	opt     Options
	now     func() time.Time
	started time.Time

	handler  http.Handler
	transmit http.RoundTripper // to the leader

	// ctx bounds what runs detached from a request (fetches, retention): Close cancels it.
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup

	// draining ends when Serve begins to shut down: the journal's long polls answer.
	draining context.Context
	drain    context.CancelFunc

	flightMu sync.Mutex
	flights  map[string]*flight
	closed   bool // no new flights after Close

	files keyedMutex // serialises PUTs of one file name (If-Match)

	maintMu sync.Mutex // one retention run at a time

	mu            sync.Mutex
	lastRetention *retentionReport
	noLeaderSince time.Time
}

// New returns the server of an instance and starts its background work: retention every
// PruneInterval, and watching the node's role. Close stops it.
func New(o Options) (*Server, error) {
	if o.Store == nil || o.Upstream == nil || o.Node == nil {
		return nil, errors.New("server: Store, Upstream and Node are required")
	}
	if o.History == 0 {
		o.History = DefaultHistory
	}
	if o.History < 0 {
		return nil, fmt.Errorf("server: history %s: must be positive", o.History)
	}
	if o.JournalKeep <= 0 {
		o.JournalKeep = DefaultJournalKeep
	}
	if o.BlobGrace <= 0 {
		o.BlobGrace = DefaultBlobGrace
	}
	if o.PruneInterval == 0 {
		o.PruneInterval = DefaultPruneInterval
	}
	if o.LeaderWait <= 0 {
		o.LeaderWait = DefaultLeaderWait
	}
	s := &Server{
		st:       o.Store,
		up:       o.Upstream,
		node:     o.Node,
		opt:      o,
		now:      o.Now,
		flights:  make(map[string]*flight),
		transmit: o.ForwardTransport,
	}
	if s.now == nil {
		s.now = time.Now
	}
	if s.transmit == nil {
		s.transmit = newForwardTransport()
	}
	s.started = s.now()
	s.ctx, s.cancel = context.WithCancel(context.Background())
	s.draining, s.drain = context.WithCancel(s.ctx)
	s.handler = s.logRequests(s.routes())
	active.Store(s)

	s.wg.Add(1)
	go s.watchRole()
	if o.PruneInterval > 0 {
		s.wg.Add(1)
		go s.maintenance(o.PruneInterval)
	}
	return s, nil
}

// Handler is the HTTP API, with the request log.
func (s *Server) Handler() http.Handler { return s.handler }

// Close stops the background work and cancels the fetches that run detached from their
// requests; it waits for them at most a few seconds. It does not close the store or the
// upstream, which belong to the caller.
func (s *Server) Close() error {
	s.drain()
	s.flightMu.Lock()
	s.closed = true
	s.flightMu.Unlock()
	s.cancel()

	done := make(chan struct{})
	go func() {
		s.wg.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-time.After(closeGrace):
		return errors.New("server: background work still running after the close grace")
	}
}

// ListenAndServe listens on addr and serves until ctx ends (Serve).
func (s *Server) ListenAndServe(ctx context.Context, addr string) error {
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		oplog.For("http").Error("cannot listen", "event", "http.failed", "addr", addr, oplog.Err(err))
		return err
	}
	return s.Serve(ctx, ln)
}

// Serve answers on ln until ctx ends, then shuts down: the journal's long polls answer at once,
// the requests in flight get ShutdownGrace to finish, and whatever is left is cut off. It
// returns nil after a shutdown and the error of a server that failed.
//
// Log events: http.listening, http.failed (ERROR).
func (s *Server) Serve(ctx context.Context, ln net.Listener) error {
	hs := s.httpServer()
	errc := make(chan error, 1)
	go func() { errc <- hs.Serve(ln) }()
	oplog.For("http").Info("listening", "event", "http.listening", "addr", ln.Addr().String())

	select {
	case err := <-errc:
		oplog.For("http").Error("HTTP server stopped", "event", "http.failed", "addr", ln.Addr().String(), oplog.Err(err))
		return err
	case <-ctx.Done():
	}
	s.drain()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), ShutdownGrace)
	defer cancel()
	if err := hs.Shutdown(shutdownCtx); err != nil {
		// Fetches that a request still waits for end now; their requests with them.
		s.cancel()
		_ = hs.Close()
	}
	if err := <-errc; err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}

// idleTimeout closes a keep-alive connection that has sent no request for this long, so that a
// client that vanished without a FIN does not hold a goroutine and a file descriptor until TCP
// keep-alive notices (soak: idle connections only ever grew).
const idleTimeout = 2 * time.Minute

// httpServer is the http.Server of Serve. ReadHeaderTimeout bounds a slow client's headers.
// There is no ReadTimeout, which would cut off a large PUT of a file or blob, and no
// WriteTimeout, which would cut off the journal's long polls and long downloads.
func (s *Server) httpServer() *http.Server {
	return &http.Server{Handler: s.handler, ReadHeaderTimeout: 10 * time.Second, IdleTimeout: idleTimeout}
}

// leading says whether this instance leads now.
func (s *Server) leading() bool { return s.node.Role() == cluster.Leader }

// roleName is the role as the API names it.
func roleName(r cluster.Role) string {
	if r == cluster.Leader {
		return "leader"
	}
	return "follower"
}

// instanceHeader is the value of Cortex-Instance: "<instance>; role=<leader|follower>".
func (s *Server) instanceHeader() string {
	return s.node.Self().Instance + "; role=" + roleName(s.node.Role())
}

// routes is the API. Every path answers 405 with Allow for a method it does not take, and
// every other path 404 not-found, both as Cortex errors.
func (s *Server) routes() http.Handler {
	mux := http.NewServeMux()
	type methods map[string]http.HandlerFunc
	route := func(path, name string, m methods) {
		allow := make([]string, 0, len(m)+1)
		for method, h := range m {
			mux.Handle(method+" "+path, s.named(name, h))
			allow = append(allow, method)
			if method == http.MethodGet {
				allow = append(allow, http.MethodHead)
			}
		}
		sort.Strings(allow)
		mux.Handle(path, s.named(name, methodNotAllowed(allow)))
	}
	route("/v1/fetch", "fetch", methods{"GET": s.handleFetch})
	route("/v1/entries", "entries", methods{"GET": s.handleEntries, "DELETE": s.handleDeleteEntry})
	route("/v1/files", "files", methods{"GET": s.handleListFiles})
	route("/v1/blobs/{hash}", "blobs", methods{"GET": s.handleGetBlob, "PUT": s.handlePutBlob})
	route("/livez", "livez", methods{"GET": s.handleLivez})
	route("/healthz", "healthz", methods{"GET": s.handleHealthz})
	route("/status", "status", methods{"GET": s.handleStatus})
	route("/metrics", "metrics", methods{"GET": s.handleMetrics})
	route("/v1/admin/step-down", "step_down", methods{"POST": s.handleStepDown})
	route("/v1/admin/prune", "prune", methods{"POST": s.handlePrune})
	route("/internal/v1/journal", "journal", methods{"GET": s.handleJournal})
	route("/internal/v1/snapshot", "snapshot", methods{"GET": s.handleSnapshot})
	mux.Handle("/", s.named("other", func(w http.ResponseWriter, r *http.Request) {
		writeError(w, http.StatusNotFound, codeNotFound, "no such endpoint: "+r.URL.Path)
	}))

	// File names are taken from the escaped path by hand, before the mux, which would clean
	// "a/../b" and redirect, where Cortex answers bad-name.
	files := s.named("files", s.handleFile)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		boundRanges(r.Header)
		if strings.HasPrefix(r.URL.Path, filesPrefix) || strings.HasPrefix(r.URL.EscapedPath(), filesPrefix) {
			files.ServeHTTP(w, r)
			return
		}
		mux.ServeHTTP(w, r)
	})
}

// named labels a request with its route, for the request log and cortex_http_requests_total.
func (s *Server) named(route string, h http.HandlerFunc) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		info(r).route = route
		h(w, r)
	})
}

func methodNotAllowed(allow []string) http.HandlerFunc {
	value := strings.Join(allow, ", ")
	return func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Allow", value)
		writeError(w, http.StatusMethodNotAllowed, codeMethodNotAllowed, r.Method+" is not allowed here; allowed: "+value)
	}
}

// keyedMutex is a mutex per key, for the PUTs of one file name.
type keyedMutex struct {
	mu    sync.Mutex
	locks map[string]*keyedLock
}

type keyedLock struct {
	mu   sync.Mutex
	refs int
}

// lock locks key and returns its unlock.
func (k *keyedMutex) lock(key string) func() {
	k.mu.Lock()
	if k.locks == nil {
		k.locks = make(map[string]*keyedLock)
	}
	l := k.locks[key]
	if l == nil {
		l = &keyedLock{}
		k.locks[key] = l
	}
	l.refs++
	k.mu.Unlock()

	l.mu.Lock()
	return func() {
		l.mu.Unlock()
		k.mu.Lock()
		if l.refs--; l.refs == 0 {
			delete(k.locks, key)
		}
		k.mu.Unlock()
	}
}
