package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"net/url"
	"os"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/cluster"
	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/server"
	"github.com/leonieziechmann/betula/cortex/internal/store"
	"github.com/leonieziechmann/betula/cortex/internal/telemetry"
	"github.com/leonieziechmann/betula/cortex/internal/upstream"
	"github.com/leonieziechmann/betula/cortex/internal/version"
)

// serveOptions are the flags of serve.
type serveOptions struct {
	addr, data, instance, advertiseURL, lock, hosts *string
	history, journalKeep, blobGrace, pruneInterval  *time.Duration
	allowPrivate                                    *bool
	logs                                            *logFlags
}

func newServeFlags() (*flag.FlagSet, *serveOptions) {
	fs := flag.NewFlagSet("serve", flag.ContinueOnError)
	o := &serveOptions{
		addr: fs.String("addr", envOr("CORTEX_ADDR", "127.0.0.1:8100"),
			"Listen address of the HTTP API (env CORTEX_ADDR)"),
		data: fs.String("data", envOr("CORTEX_DATA", "cortex-data"),
			"Data directory: index.db, blobs/, tmp/ (env CORTEX_DATA)"),
		instance: fs.String("instance", envOr("CORTEX_INSTANCE", hostname()),
			"Name of this instance, unique in the pair; default the host name (env CORTEX_INSTANCE)"),
		advertiseURL: fs.String("advertise-url", envOr("CORTEX_ADVERTISE_URL", ""),
			"URL the other instance reaches this one at; empty: http://<addr> (env CORTEX_ADVERTISE_URL)"),
		lock: fs.String("lock", envOr("CORTEX_LOCK", ""),
			"Lock file the two instances of a pair elect their leader with (flock); empty: one instance, always the leader (env CORTEX_LOCK)"),
		hosts: fs.String("hosts", envOr("CORTEX_HOSTS_FILE", ""),
			"Host policy (JSON: allowed hosts, the floor per host), re-read when it changes; empty: every public host with the defaults (env CORTEX_HOSTS_FILE)"),
		history: fs.Duration("history", envDuration("CORTEX_HISTORY", server.DefaultHistory),
			"Keep a superseded version this long after it was superseded (env CORTEX_HISTORY)"),
		journalKeep: fs.Duration("journal-keep", envDuration("CORTEX_JOURNAL_KEEP", server.DefaultJournalKeep),
			"Keep journal entries this long; a follower away longer starts from a snapshot (env CORTEX_JOURNAL_KEEP)"),
		blobGrace: fs.Duration("blob-grace", envDuration("CORTEX_BLOB_GRACE", server.DefaultBlobGrace),
			"Remove a blob nothing references only this long after it was last written (env CORTEX_BLOB_GRACE)"),
		pruneInterval: fs.Duration("prune-interval", envDuration("CORTEX_PRUNE_INTERVAL", server.DefaultPruneInterval),
			"Run the retention this often (env CORTEX_PRUNE_INTERVAL)"),
		allowPrivate: fs.Bool("allow-private", envBool("CORTEX_ALLOW_PRIVATE", false),
			"Allow fetching from loopback and private addresses, for development and tests (env CORTEX_ALLOW_PRIVATE)"),
	}
	o.logs = addLogFlags(fs)
	return fs, o
}

func hostname() string {
	name, err := os.Hostname()
	if err != nil || name == "" {
		return "cortex"
	}
	return name
}

// logFlags are the logging flags of serve, with the defaults of CORTEX_LOG_FORMAT,
// CORTEX_LOG_LEVEL and CORTEX_LOG_FILE.
type logFlags struct {
	format, level, file *string
}

func addLogFlags(fs *flag.FlagSet) *logFlags {
	env := oplog.OptionsFromEnv()
	return &logFlags{
		format: fs.String("log-format", env.Format, "Log format: text or json; serve's default json (env CORTEX_LOG_FORMAT)"),
		level:  fs.String("log-level", env.Level, "Log level: debug, info, warn, error (env CORTEX_LOG_LEVEL)"),
		file:   fs.String("log-file", env.File, "Also append the log to this file (env CORTEX_LOG_FILE)"),
	}
}

// setup installs the logger, counting its problems in cortex_log_problems_total. The returned
// function flushes the log file; ok false after an invalid option (exit 2).
func (l *logFlags) setup() (*oplog.Recorder, func(), bool) {
	recorder, closeLog, err := oplog.Setup(oplog.Options{Format: *l.format, Level: *l.level, File: *l.file, Problems: server.LogProblems})
	if err != nil {
		fmt.Fprintf(stderr, "Error: %v\n", err)
		return nil, nil, false
	}
	return recorder, func() { _ = closeLog() }, true
}

// newNode is the instance's place in the pair: alone and always the leader without a lock
// file, else one of two that elect their leader with flock on it. ctx ends with the process,
// and a leader then begins its hand-over at once. stop waits for the node to stop (a leader's
// hand-over takes until its successor announced itself: at most about 20 s); it must run while
// the HTTP server still listens, so that the successor can fetch the last entries and blobs,
// and before the store closes.
func newNode(ctx context.Context, st *store.Store, self cluster.Info, lock string) (node cluster.Node, stop func() error, err error) {
	if lock == "" {
		node, err := cluster.NewSingle(st, self)
		return node, func() error { return nil }, err
	}
	peer, err := cluster.New(ctx, st, self, lock, cluster.Options{})
	if err != nil {
		return nil, nil, err
	}
	return peer, sync.OnceValue(peer.Close), nil
}

// The bounds of a stop (SIGTERM), from the signal on, within the 30 s of swarm's
// stop_grace_period: the hand-over of a leader of a pair may take handOverBudget (its successor
// catches up meanwhile, E2E-1); a follower then answers without keep-alive for keepAliveDrain
// (D5); the requests in flight get until shutdownBudget, at least minShutdown; and
// server.Close gives what runs detached at most 5 s more. A hand-over that does not end
// within its budget (a write that does not end) is given up: the process ends as one that
// crashed would.
const (
	handOverBudget = 22 * time.Second
	keepAliveDrain = time.Second
	shutdownBudget = 24 * time.Second
	singleShutdown = server.ShutdownGrace
	minShutdown    = time.Second
)

// runServe is the long-running mode: the HTTP API until SIGINT or SIGTERM, then a graceful
// shutdown within the 30 s a container stop allows (exit 0). A leader of a pair hands over
// first, while the HTTP server still answers, so that its successor fetches every entry and
// blob it lacks from here; then the instance, a follower now, stops keep-alives and listens
// about a second more, so that clients go to the other instance on a fresh connection, which
// they may send a write again on, rather than lose one on a connection reused as it closes.
//
// Log events: service.started, service.stopped, service.fatal (ERROR), http.listening,
// http.failed (ERROR).
func runServe(ctx context.Context, args []string) int {
	fs, o := newServeFlags()
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	if *o.logs.format == "" {
		*o.logs.format = "json" // a service logs for machines unless told otherwise
	}
	recorder, closeLog, ok := o.logs.setup()
	if !ok {
		return 2
	}
	defer closeLog()
	for _, d := range []struct {
		name  string
		value time.Duration
	}{{"history", *o.history}, {"journal-keep", *o.journalKeep}, {"blob-grace", *o.blobGrace}, {"prune-interval", *o.pruneInterval}} {
		if d.value <= 0 {
			slog.Error("invalid flag: must be positive", "component", "cli", "event", "cli.failed", "flag", d.name, "value", d.value.String())
			return 2
		}
	}
	advertise := *o.advertiseURL
	if advertise == "" {
		advertise = "http://" + *o.addr
	}
	if *o.lock != "" && !reachable(advertise) {
		slog.Error("invalid flag: the other instance of the pair cannot reach this URL; set --advertise-url", "component", "cli",
			"event", "cli.failed", "flag", "advertise-url", "value", advertise)
		return 2
	}
	log := oplog.For("cortex")
	fatal := func(code int, msg string, err error) int {
		log.Error(msg, "event", "service.fatal", oplog.Err(err))
		return code
	}

	declareBuildInfo()
	st, err := store.Open(*o.data)
	if err != nil {
		return fatal(1, "cannot open the data directory", err)
	}
	// The store closes last, after the node, which writes to it until it stops; not when the
	// node could not be stopped (a write that does not end), whose close would wait for it.
	storeInUse := false
	defer func() {
		if !storeInUse {
			_ = st.Close()
		}
	}()
	up, err := upstream.New(upstream.Options{PolicyFile: *o.hosts, AllowPrivate: *o.allowPrivate})
	if err != nil {
		return fatal(2, "invalid host policy", err)
	}
	defer up.Close()
	self := cluster.Info{Instance: *o.instance, URL: advertise}
	node, stopNode, err := newNode(ctx, st, self, *o.lock)
	if err != nil {
		return fatal(1, "cannot take a place in the pair", err)
	}

	srv, err := server.New(server.Options{Store: st, Upstream: up, Node: node, Version: cortexVersion, Build: version.Build(),
		History: *o.history, JournalKeep: *o.journalKeep, BlobGrace: *o.blobGrace, PruneInterval: *o.pruneInterval,
		Recorder: recorder})
	if err != nil {
		_ = stopNode()
		return fatal(2, "invalid configuration", err)
	}
	ln, err := net.Listen("tcp", *o.addr)
	if err != nil {
		oplog.For("http").Error("cannot listen", "event", "http.failed", "addr", *o.addr, oplog.Err(err))
		_ = stopNode()
		_ = srv.Close()
		return fatal(1, "stopping: the HTTP endpoint failed", err)
	}
	draining, drain := context.WithCancel(context.Background())
	defer drain()
	hs := httpServer(srv, draining)
	served := make(chan error, 1)
	go func() { served <- hs.Serve(ln) }()
	oplog.For("http").Info("listening", "event", "http.listening", "addr", ln.Addr().String())
	log.Info("started", "event", "service.started", "instance", self.Instance, "addr", *o.addr, "advertise_url", advertise,
		"data", *o.data, "role", roleOf(node), "version", cortexVersion, "build", version.Build(), "hosts", *o.hosts,
		"lock", *o.lock)

	var serveErr error
	select {
	case serveErr = <-served:
		oplog.For("http").Error("HTTP server stopped", "event", "http.failed", "addr", ln.Addr().String(), oplog.Err(serveErr))
	case <-ctx.Done():
	}
	stopping := time.Now()
	pair := *o.lock != ""

	// The hand-over, while the HTTP server answers (the node began it when ctx ended).
	stopped := make(chan error, 1)
	go func() { stopped <- stopNode() }()
	select {
	case <-stopped:
	case <-time.After(handOverBudget):
		storeInUse = true
		log.Error("the hand-over did not end in time: stopping without it", "event", "service.fatal",
			"budget_s", handOverBudget.Seconds())
	}
	budget := singleShutdown
	if pair {
		budget = shutdownBudget
		if serveErr == nil {
			// A follower now: answers say Connection: close, and idle connections close, while
			// the port still takes new ones (D5).
			hs.SetKeepAlivesEnabled(false)
			time.Sleep(keepAliveDrain)
		}
	}
	drain() // the journal's long polls answer at once
	shutdownCtx, cancel := context.WithDeadline(context.Background(), maxTime(stopping.Add(budget), time.Now().Add(minShutdown)))
	defer cancel()
	if err := hs.Shutdown(shutdownCtx); err != nil {
		_ = srv.Close() // fetches that a request still waits for end now; their requests with them
		_ = hs.Close()
	}
	closeErr := srv.Close() // what still runs detached gets a few seconds
	if serveErr != nil && !errors.Is(serveErr, http.ErrServerClosed) {
		return fatal(1, "stopping: the HTTP endpoint failed", serveErr)
	}
	attrs := []any{"event", "service.stopped", "instance", self.Instance, "duration_ms", time.Since(stopping).Milliseconds()}
	if closeErr != nil {
		attrs = append(attrs, oplog.Err(closeErr))
	}
	log.Info("stopped", attrs...)
	if storeInUse {
		return 1
	}
	return 0
}

// httpServer is the HTTP server of serve: server.Server's handler with server.Serve's bounds
// (ReadHeaderTimeout for a slow client's headers, IdleTimeout for keep-alive connections of
// clients that vanished; no ReadTimeout or WriteTimeout, which would cut off large uploads,
// downloads and the journal's long polls). serve runs it itself, so that it can stop
// keep-alives before it stops listening. A long poll of the journal ends when draining does.
func httpServer(srv *server.Server, draining context.Context) *http.Server {
	h := srv.Handler()
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/internal/v1/journal" {
			ctx, cancel := context.WithCancel(r.Context())
			defer cancel()
			stop := context.AfterFunc(draining, cancel)
			defer stop()
			r = r.WithContext(ctx)
		}
		h.ServeHTTP(w, r)
	})
	return &http.Server{Handler: handler, ReadHeaderTimeout: 10 * time.Second, IdleTimeout: 2 * time.Minute}
}

func maxTime(a, b time.Time) time.Time {
	if a.After(b) {
		return a
	}
	return b
}

// reachable says whether another instance can reach a URL: http or https with a host that is
// not a wildcard address (0.0.0.0, ::, or none, as an advertise URL derived from
// --addr 0.0.0.0:8100 would have).
func reachable(raw string) bool {
	u, err := url.Parse(raw)
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Hostname() == "" {
		return false
	}
	ip := net.ParseIP(u.Hostname())
	return ip == nil || !ip.IsUnspecified()
}

func roleOf(node cluster.Node) string {
	if node.Role() == cluster.Leader {
		return "leader"
	}
	return "follower"
}

var buildInfoOnce sync.Once

// declareBuildInfo names the running binary in GET /metrics: cortex_build_info{build} and
// cortex_start_time_seconds.
func declareBuildInfo() {
	buildInfoOnce.Do(func() {
		build := version.Build()
		if len(build) > 12 {
			build = build[:12]
		}
		started := float64(time.Now().Unix())
		telemetry.Registry.NewGaugeFunc("cortex_build_info",
			"Always 1: the binary that runs (the start of the sha256 of its executable).",
			[]string{"build"}, func(emit func(float64, ...string)) { emit(1, build) })
		telemetry.Registry.NewGaugeFunc("cortex_start_time_seconds",
			"When the process started (Unix time).", nil, func(emit func(float64, ...string)) { emit(started) })
	})
}
