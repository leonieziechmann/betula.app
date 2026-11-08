package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"net/netip"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/phloem/internal/metrics"
	"github.com/leonieziechmann/betula/phloem/internal/oplog"
	"github.com/leonieziechmann/betula/phloem/internal/server"
	"github.com/leonieziechmann/betula/phloem/internal/store"
	"github.com/leonieziechmann/betula/phloem/internal/version"
)

// serveOptions are the flags of serve.
type serveOptions struct {
	addr, data, statsDir, origins, stack *string
	statsInterval                        *time.Duration
	budget                               *int64
	logs                                 *logFlags

	turnHost, turnIP, turnRelays, turnTLSHost *string
	turnPort                                  *int
}

func newServeFlags() (*flag.FlagSet, *serveOptions) {
	fs := flag.NewFlagSet("serve", flag.ContinueOnError)
	o := &serveOptions{
		addr: fs.String("addr", envOr("PHLOEM_ADDR", "127.0.0.1:8200"),
			"Listen address of the HTTP API and the WebSockets (env PHLOEM_ADDR)"),
		data: fs.String("data", envOr("PHLOEM_DATA", "phloem-data"),
			"Data directory: phloem.db (env PHLOEM_DATA)"),
		statsDir: fs.String("stats-dir", envOr("PHLOEM_STATS_DIR", ""),
			"Directory the stats file phloem.prom is written to; empty: none (env PHLOEM_STATS_DIR)"),
		statsInterval: fs.Duration("stats-interval", envDuration("PHLOEM_STATS_INTERVAL", server.DefaultStatsInterval),
			"How often the stats file is written (env PHLOEM_STATS_INTERVAL)"),
		origins: fs.String("origins", envOr("PHLOEM_ORIGINS", ""),
			"Browser origins taken for the WebSocket and answered for CORS, comma-separated (scheme://host[:port]); empty: the site's own, the origin of the request's host (env PHLOEM_ORIGINS)"),
		budget: fs.Int64("budget", envInt64("PHLOEM_BUDGET_BYTES", server.DefaultBudget),
			"Bytes all spaces together may hold; at it no new spaces or parcels (env PHLOEM_BUDGET_BYTES)"),
		stack: fs.String("stack", envOr("PHLOEM_STACK", ""),
			"Name of the instance: the label stack=… of the stats file (env PHLOEM_STACK)"),
		turnHost: fs.String("turn-host", envOr("PHLOEM_TURN_HOST", ""),
			"The site's host, named by the URLs of STUN and TURN; empty: no STUN and TURN (env PHLOEM_TURN_HOST)"),
		turnPort: fs.Int("turn-port", int(envInt64("PHLOEM_TURN_PORT", 3478)),
			"Where STUN and TURN listen, over UDP and TCP, on every address (env PHLOEM_TURN_PORT)"),
		turnIP: fs.String("turn-ip", envOr("PHLOEM_TURN_IP", ""),
			"The host's public IPv4 address: the relays' address, and never a peer (env PHLOEM_TURN_IP)"),
		turnRelays: fs.String("turn-relays", envOr("PHLOEM_TURN_RELAY_PORTS", ""),
			"The UDP ports of the relays, as first-last (env PHLOEM_TURN_RELAY_PORTS)"),
		turnTLSHost: fs.String("turn-tls-host", envOr("PHLOEM_TURN_TLS_HOST", ""),
			"The name of turns:<name>:443, where Traefik ends TLS and hands TCP to the TURN port; empty: none (env PHLOEM_TURN_TLS_HOST)"),
	}
	o.logs = addLogFlags(fs)
	return fs, o
}

// logFlags are the logging flags, with the defaults of PHLOEM_LOG_FORMAT, PHLOEM_LOG_LEVEL and
// PHLOEM_LOG_FILE.
type logFlags struct {
	format, level, file *string
}

func addLogFlags(fs *flag.FlagSet) *logFlags {
	env := oplog.OptionsFromEnv()
	return &logFlags{
		format: fs.String("log-format", env.Format, "Log format: text or json; the default json (env PHLOEM_LOG_FORMAT)"),
		level:  fs.String("log-level", env.Level, "Log level: debug, info, warn, error (env PHLOEM_LOG_LEVEL)"),
		file:   fs.String("log-file", env.File, "Also append the log to this file (env PHLOEM_LOG_FILE)"),
	}
}

// setup installs the logger, counting its problems in phloem_log_problems_total. The returned
// function flushes the log file; ok false after an invalid option (exit 2).
func (l *logFlags) setup() (func(), bool) {
	if *l.format == "" {
		*l.format = "json" // a service logs for machines unless told otherwise
	}
	_, closeLog, err := oplog.Setup(oplog.Options{Format: *l.format, Level: *l.level, File: *l.file, Problems: server.LogProblems})
	if err != nil {
		fmt.Fprintf(stderr, "Error: %v\n", err)
		return nil, false
	}
	return func() { _ = closeLog() }, true
}

// shutdownBudget is what the requests in flight get after SIGTERM, within the 30 s a container
// stop allows; server.Close then gives the sockets up to 10 s.
const shutdownBudget = 10 * time.Second

// runServe is the long-running mode: the HTTP API and the WebSockets until SIGINT or SIGTERM,
// then a graceful stop (exit 0): no new connections, the requests in flight end, every device
// hears 1001 and comes back to the next instance.
//
// Log events: service.started, service.stopped, service.fatal (ERROR), service.root (WARN),
// http.listening, http.failed (ERROR).
func runServe(ctx context.Context, args []string) int {
	fs, o := newServeFlags()
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	closeLog, ok := o.logs.setup()
	if !ok {
		return 2
	}
	defer closeLog()
	log := oplog.For("phloem")
	fatal := func(code int, msg string, err error) int {
		log.Error(msg, "event", "service.fatal", oplog.Err(err))
		return code
	}
	if os.Geteuid() == 0 {
		log.Warn("serve runs as root: the image runs start, which drops every privilege first", "event", "service.root")
	}
	var origins []string
	for _, o := range strings.Split(*o.origins, ",") {
		if o = strings.TrimSpace(o); o != "" {
			origins = append(origins, o)
		}
	}
	if *o.statsInterval <= 0 || *o.budget <= 0 {
		return fatal(2, "invalid configuration", errors.New("stats-interval and budget must be positive"))
	}

	st, err := store.Open(*o.data)
	if err != nil {
		return fatal(1, "cannot open the data directory", err)
	}
	defer st.Close()
	turnOpt, err := o.turn()
	if err != nil {
		return fatal(2, "invalid configuration", err)
	}
	ln, err := net.Listen("tcp", *o.addr)
	if err != nil {
		turnOpt.close()
		oplog.For("http").Error("cannot listen", "event", "http.failed", "addr", *o.addr, oplog.Err(err))
		return fatal(1, "stopping: the HTTP endpoint failed", err)
	}
	// The server declares its metrics in metrics.Default, the registry of the stats file: once
	// in a process.
	srv, err := server.New(server.Options{
		Store: st, Origins: origins, Budget: *o.budget, StatsDir: *o.statsDir, StatsInterval: *o.statsInterval,
		Stack: *o.stack, Version: version.Version, Build: version.Build(), Registry: metrics.Default,
		TURN: turnOpt.TURN,
	})
	if err != nil {
		_ = ln.Close()
		turnOpt.close()
		return fatal(2, "invalid configuration", err)
	}
	hs := httpServer(srv)
	served := make(chan error, 1)
	go func() { served <- hs.Serve(ln) }()
	oplog.For("http").Info("listening", "event", "http.listening", "addr", ln.Addr().String())
	log.Info("started", "event", "service.started", "addr", *o.addr, "data", *o.data, "stats_dir", *o.statsDir,
		"origins", strings.Join(origins, ","), "budget", *o.budget, "stack", *o.stack,
		"turn", turnOpt.describe(), "version", version.Version, "build", version.Build(), "uid", os.Geteuid())

	var serveErr error
	select {
	case serveErr = <-served:
		oplog.For("http").Error("HTTP server stopped", "event", "http.failed", "addr", ln.Addr().String(), oplog.Err(serveErr))
	case <-ctx.Done():
	}
	stopping := time.Now()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownBudget)
	defer cancel()
	if err := hs.Shutdown(shutdownCtx); err != nil {
		_ = hs.Close()
	}
	_ = srv.Close() // the sockets hear 1001; the stats file says phloem_up 0
	if serveErr != nil && !errors.Is(serveErr, http.ErrServerClosed) {
		return fatal(1, "stopping: the HTTP endpoint failed", serveErr)
	}
	log.Info("stopped", "event", "service.stopped", "duration_ms", time.Since(stopping).Milliseconds())
	return 0
}

// httpServer is serve's HTTP server: ReadHeaderTimeout for a slow client's headers, IdleTimeout
// for keep-alive connections of clients that vanished; no ReadTimeout or WriteTimeout, which
// would end the WebSockets (the handlers bound the bodies themselves). Its own errors (a TLS
// probe, a reset) go to the log at debug.
func httpServer(srv *server.Server) *http.Server {
	return &http.Server{
		Handler:           srv.Handler(),
		ReadHeaderTimeout: 10 * time.Second,
		IdleTimeout:       2 * time.Minute,
		MaxHeaderBytes:    16 << 10,
		ErrorLog:          slog.NewLogLogger(slog.Default().Handler(), slog.LevelDebug),
	}
}

// turnSetup is serve's TURN: the options with their listeners, nil without TURN.
type turnSetup struct{ *server.TURN }

// turn reads the TURN flags and opens STUN's and TURN's listeners on every address.
func (o *serveOptions) turn() (turnSetup, error) {
	host := strings.TrimSpace(*o.turnHost)
	if host == "" {
		return turnSetup{}, nil
	}
	ip, err := netip.ParseAddr(strings.TrimSpace(*o.turnIP))
	if err != nil || !ip.Is4() {
		return turnSetup{}, fmt.Errorf("turn-ip %q is no IPv4 address", *o.turnIP)
	}
	first, last, ok := strings.Cut(strings.TrimSpace(*o.turnRelays), "-")
	lo, err1 := strconv.Atoi(first)
	hi, err2 := strconv.Atoi(last)
	if !ok || err1 != nil || err2 != nil {
		return turnSetup{}, fmt.Errorf("turn-relays %q is not first-last", *o.turnRelays)
	}
	if *o.turnPort < 1 || *o.turnPort > 65535 {
		return turnSetup{}, fmt.Errorf("invalid turn-port %d", *o.turnPort)
	}
	addr := net.JoinHostPort("0.0.0.0", strconv.Itoa(*o.turnPort))
	pc, err := net.ListenPacket("udp4", addr)
	if err != nil {
		return turnSetup{}, fmt.Errorf("STUN and TURN over UDP: %w", err)
	}
	ln, err := net.Listen("tcp4", addr)
	if err != nil {
		_ = pc.Close()
		return turnSetup{}, fmt.Errorf("TURN over TCP: %w", err)
	}
	return turnSetup{&server.TURN{
		Host: host, Port: *o.turnPort, IP: ip, RelayMin: lo, RelayMax: hi,
		TLSHost: strings.TrimSpace(*o.turnTLSHost), PacketConn: pc, Listener: ln,
	}}, nil
}

// close closes the listeners of a TURN that never started.
func (t turnSetup) close() {
	if t.TURN != nil {
		_ = t.PacketConn.Close()
		_ = t.Listener.Close()
	}
}

// describe says in a word what TURN serves, for the log.
func (t turnSetup) describe() string {
	if t.TURN == nil {
		return "off"
	}
	return fmt.Sprintf("%s:%d relays %d-%d at %s tls %q", t.Host, t.Port, t.RelayMin, t.RelayMax, t.IP, t.TLSHost)
}
