package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/metrics"
	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/server"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/version"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// defaultAddr is where serve listens and the commands find it.
const defaultAddr = "127.0.0.1:8300"

// serveOptions are the flags of serve.
type serveOptions struct {
	addr, data, site, contact, lang, origins, hosts, feed, statsDir, stack *string
	statsInterval, feedInterval                                            *time.Duration
	max                                                                    *int64
	logs                                                                   *logFlags
}

func newServeFlags() (*flag.FlagSet, *serveOptions) {
	fs := flag.NewFlagSet("serve", flag.ContinueOnError)
	o := &serveOptions{
		addr: fs.String("addr", envOr("XYLEM_ADDR", defaultAddr),
			"Listen address of the HTTP API (env XYLEM_ADDR)"),
		data: fs.String("data", envOr("XYLEM_DATA", "xylem-data"),
			"Data directory: xylem.db, with the VAPID key in it (env XYLEM_DATA)"),
		site: fs.String("site", envOr("XYLEM_SITE", ""),
			"The site's origin, https://host: what a notification's link opens at (env XYLEM_SITE)"),
		contact: fs.String("contact", envOr("XYLEM_CONTACT", ""),
			"Where the operators of a push service reach Xylem's, a mailto: or https: address, sent to them with every message; empty: the site (env XYLEM_CONTACT)"),
		lang: fs.String("lang", envOr("XYLEM_LANG", server.DefaultLang),
			"The language every announcement is written in, read by a browser whose own it has no text in (env XYLEM_LANG)"),
		origins: fs.String("origins", envOr("XYLEM_ORIGINS", ""),
			"Browser origins answered for CORS, comma-separated (scheme://host[:port]); empty: the site's own, the origin of the request's host (env XYLEM_ORIGINS)"),
		hosts: fs.String("push-hosts", envOr("XYLEM_PUSH_HOSTS", ""),
			"Hosts taken as push services beside Google's, Mozilla's, Apple's and Microsoft's, comma-separated; .name: every host under it (env XYLEM_PUSH_HOSTS)"),
		feed: fs.String("feed", envOr("XYLEM_FEED", ""),
			"The address of the site's feed of announcements, read every --feed-interval: what Betula announces by itself; empty: only a person announces (env XYLEM_FEED)"),
		feedInterval: fs.Duration("feed-interval", envDuration("XYLEM_FEED_INTERVAL", server.DefaultLimits.FeedEvery),
			"How often the feed is read (env XYLEM_FEED_INTERVAL)"),
		max: fs.Int64("max-subscriptions", envInt64("XYLEM_MAX_SUBSCRIPTIONS", server.DefaultMaxSubscriptions),
			"How many subscriptions Xylem holds; at it no new one (env XYLEM_MAX_SUBSCRIPTIONS)"),
		statsDir: fs.String("stats-dir", envOr("XYLEM_STATS_DIR", ""),
			"Directory the stats file xylem.prom is written to; empty: none (env XYLEM_STATS_DIR)"),
		statsInterval: fs.Duration("stats-interval", envDuration("XYLEM_STATS_INTERVAL", server.DefaultStatsInterval),
			"How often the stats file is written (env XYLEM_STATS_INTERVAL)"),
		stack: fs.String("stack", envOr("XYLEM_STACK", ""),
			"Name of the instance: the label stack=… of the stats file (env XYLEM_STACK)"),
	}
	o.logs = addLogFlags(fs)
	return fs, o
}

// logFlags are the logging flags, with the defaults of XYLEM_LOG_FORMAT, XYLEM_LOG_LEVEL and
// XYLEM_LOG_FILE.
type logFlags struct {
	format, level, file *string
}

func addLogFlags(fs *flag.FlagSet) *logFlags {
	env := oplog.OptionsFromEnv()
	return &logFlags{
		format: fs.String("log-format", env.Format, "Log format: text or json; the default json (env XYLEM_LOG_FORMAT)"),
		level:  fs.String("log-level", env.Level, "Log level: debug, info, warn, error (env XYLEM_LOG_LEVEL)"),
		file:   fs.String("log-file", env.File, "Also append the log to this file (env XYLEM_LOG_FILE)"),
	}
}

// setup installs the logger, counting its problems in xylem_log_problems_total. The returned
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

// shutdownBudget is what the requests in flight get after SIGTERM; server.Close then lets the
// sender end the batch it is in (a request to a push service ends within 15 s), all within the
// 30 s a container stop allows.
const shutdownBudget = 5 * time.Second

func list(text string) []string {
	var out []string
	for _, item := range strings.Split(text, ",") {
		if item = strings.TrimSpace(item); item != "" {
			out = append(out, item)
		}
	}
	return out
}

// runServe is the long-running mode: the HTTP API and the sender until SIGINT or SIGTERM, then
// a graceful stop (exit 0): no new connections, the requests in flight end, the sender ends the
// batch it is in.
//
// Log events: service.started, service.stopped, service.fatal (ERROR), service.root (WARN),
// service.key (the first start of a database: its VAPID key was made), http.listening,
// http.failed (ERROR).
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
	log := oplog.For("xylem")
	fatal := func(code int, msg string, err error) int {
		log.Error(msg, "event", "service.fatal", oplog.Err(err))
		return code
	}
	if os.Geteuid() == 0 {
		log.Warn("serve runs as root: Xylem needs no privilege", "event", "service.root")
	}
	if *o.statsInterval <= 0 || *o.max <= 0 {
		return fatal(2, "invalid configuration", errors.New("stats-interval and max-subscriptions must be positive"))
	}
	if *o.feedInterval < time.Minute {
		return fatal(2, "invalid configuration", errors.New("feed-interval: a minute at least"))
	}
	if strings.TrimSpace(*o.site) == "" {
		return fatal(2, "invalid configuration", errors.New("--site (XYLEM_SITE) is not set: the origin a notification's link opens at, https://host"))
	}

	st, err := store.Open(*o.data)
	if err != nil {
		return fatal(1, "cannot open the data directory", err)
	}
	defer st.Close()
	made := false
	text, err := st.VAPID(ctx, func() (string, error) {
		key, err := webpush.NewKey()
		if err != nil {
			return "", err
		}
		made = true
		return key.String(), nil
	})
	if err != nil {
		return fatal(1, "cannot read the VAPID key", err)
	}
	key, err := webpush.ParseKey(text)
	if err != nil {
		return fatal(1, "the VAPID key of the database does not read", err)
	}
	if made {
		log.Info("a VAPID key was made: this database's first start", "event", "service.key", "public", key.Public())
	}
	ln, err := net.Listen("tcp", *o.addr)
	if err != nil {
		oplog.For("http").Error("cannot listen", "event", "http.failed", "addr", *o.addr, oplog.Err(err))
		return fatal(1, "stopping: the HTTP endpoint failed", err)
	}
	// The server declares its metrics in metrics.Default, the registry of the stats file: once
	// in a process.
	srv, err := server.New(server.Options{
		Store: st, Key: key, Site: *o.site, Contact: *o.contact, Lang: *o.lang, Origins: list(*o.origins), Hosts: list(*o.hosts),
		MaxSubscriptions: *o.max, StatsDir: *o.statsDir, StatsInterval: *o.statsInterval, Stack: *o.stack,
		Version: version.Version, Build: version.Build(), Registry: metrics.Default,
		Feed: *o.feed, Limits: server.Limits{FeedEvery: *o.feedInterval},
	})
	if err != nil {
		_ = ln.Close()
		return fatal(2, "invalid configuration", err)
	}
	hs := &http.Server{
		Handler:           srv.Handler(),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       40 * time.Second,
		WriteTimeout:      40 * time.Second,
		IdleTimeout:       2 * time.Minute,
		MaxHeaderBytes:    16 << 10,
		ErrorLog:          slog.NewLogLogger(slog.Default().Handler(), slog.LevelDebug),
	}
	served := make(chan error, 1)
	go func() { served <- hs.Serve(ln) }()
	oplog.For("http").Info("listening", "event", "http.listening", "addr", ln.Addr().String())
	log.Info("started", "event", "service.started", "addr", *o.addr, "data", *o.data, "site", *o.site, "lang", *o.lang,
		"feed", *o.feed, "stats_dir", *o.statsDir, "origins", *o.origins, "push_hosts", *o.hosts, "max_subscriptions", *o.max, "stack", *o.stack,
		"subscriptions", st.Subscriptions(), "version", version.Version, "build", version.Build(), "uid", os.Geteuid())

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
	_ = srv.Close() // the sender ends its batch; the stats file says xylem_up 0
	if serveErr != nil && !errors.Is(serveErr, http.ErrServerClosed) {
		return fatal(1, "stopping: the HTTP endpoint failed", serveErr)
	}
	log.Info("stopped", "event", "service.stopped", "duration_ms", time.Since(stopping).Milliseconds())
	return 0
}
