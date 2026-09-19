package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
	_ "time/tzdata" // off-peak hours are local time; do not depend on the host having zoneinfo

	"github.com/leonieziechmann/btu-scraper/internal/oplog"
	"github.com/leonieziechmann/btu-scraper/internal/service"
)

// envOr returns the environment variable name, or fallback when it is unset.
// Every flag of `run` has one, so a container is configured by environment alone.
func envOr(name, fallback string) string {
	if v := os.Getenv(name); v != "" {
		return v
	}
	return fallback
}

func envInt(name string, fallback int) int {
	if v, err := strconv.Atoi(os.Getenv(name)); err == nil {
		return v
	}
	return fallback
}

func envDuration(name string, fallback time.Duration) time.Duration {
	if v, err := time.ParseDuration(os.Getenv(name)); err == nil {
		return v
	}
	return fallback
}

// runService is the long-running mode: keep the archive fresh, rebuild, publish
// snapshots over HTTP. It stops cleanly on SIGINT / SIGTERM.
func runService(ctx context.Context, args []string) {
	def := service.DefaultConfig()

	fs := flag.NewFlagSet("run", flag.ExitOnError)
	dbPath := fs.String("db", envOr("BTU_DB", defaultDBPath), "Database path (env BTU_DB)")
	snapshotDir := fs.String("snapshot-dir", envOr("BTU_SNAPSHOT_DIR", def.SnapshotDir), "Directory for exported snapshots (env BTU_SNAPSHOT_DIR)")
	addr := fs.String("addr", envOr("BTU_ADDR", "127.0.0.1:8090"), "Listen address for /snapshot, /healthz and /status (env BTU_ADDR)")
	interval := fs.Duration("interval", envDuration("BTU_INTERVAL", def.Interval), "Pause between cycles (env BTU_INTERVAL)")
	offpeak := fs.String("offpeak", envOr("BTU_OFFPEAK", "1-6"), "Local hours for bulk crawling, START-END, or 'any' (env BTU_OFFPEAK)")
	moduleDelay := fs.Int("module-delay", envInt("BTU_MODULE_DELAY_MS", int(def.Modules.Delay.Milliseconds())), "Pause between module page requests in ms (env BTU_MODULE_DELAY_MS)")
	qisDelay := fs.Int("qis-delay", envInt("BTU_QIS_DELAY_MS", int(def.Events.Delay.Milliseconds())), "Pause between QIS requests in ms (env BTU_QIS_DELAY_MS)")
	moduleMaxAge := fs.Duration("module-max-age", envDuration("BTU_MODULE_MAX_AGE", def.Modules.MaxAge), "Refetch a module page after this long (env BTU_MODULE_MAX_AGE)")
	eventMaxAge := fs.Duration("event-max-age", envDuration("BTU_EVENT_MAX_AGE", def.Events.MaxAge), "Refetch an event page after this long (env BTU_EVENT_MAX_AGE)")
	treeMaxAge := fs.Duration("tree-max-age", envDuration("BTU_TREE_MAX_AGE", def.Tree.MaxAge), "Refetch a QIS tree page after this long (env BTU_TREE_MAX_AGE)")
	retention := fs.Duration("event-retention", envDuration("BTU_EVENT_RETENTION", def.EventRetention), "Remove an event this long after its last date, 0 keeps all (env BTU_EVENT_RETENTION)")
	archiveGrace := fs.Duration("archive-grace", envDuration("BTU_ARCHIVE_GRACE", def.ArchiveGrace), "Remove archived pages nothing leads to any more this long after their fetch, 0 keeps them (env BTU_ARCHIVE_GRACE)")
	staleAfter := fs.Duration("stale-after", envDuration("BTU_STALE_AFTER", def.StaleAfter), "Report unhealthy without a successful cycle for this long (env BTU_STALE_AFTER)")
	once := fs.Bool("once", false, "Run a single cycle and exit (exit code 1 if it failed)")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)

	if *logs.format == "" {
		*logs.format = "json" // a service logs for machines unless told otherwise
	}
	recorder, closeLog := logs.setup()
	defer closeLog()

	cfg := def
	cfg.SnapshotDir = *snapshotDir
	cfg.Interval = *interval
	cfg.Modules.Delay = time.Duration(*moduleDelay) * time.Millisecond
	cfg.Modules.MaxAge = *moduleMaxAge
	cfg.Events.Delay = time.Duration(*qisDelay) * time.Millisecond
	cfg.Events.MaxAge = *eventMaxAge
	cfg.Tree.Delay = 2 * time.Duration(*qisDelay) * time.Millisecond
	cfg.Tree.MaxAge = *treeMaxAge
	cfg.EventRetention = *retention
	cfg.ArchiveGrace = *archiveGrace
	cfg.StaleAfter = *staleAfter

	if strings.EqualFold(*offpeak, "any") {
		cfg.OffPeakStart, cfg.OffPeakEnd = 0, 0
	} else {
		start, end, ok := strings.Cut(*offpeak, "-")
		s, errS := strconv.Atoi(start)
		e, errE := strconv.Atoi(end)
		if !ok || errS != nil || errE != nil || s < 0 || s > 23 || e < 0 || e > 23 {
			slog.Error("invalid --offpeak, expected START-END with hours 0-23, or 'any'", "component", "cli", "event", "cli.failed", "value", *offpeak)
			os.Exit(2)
		}
		cfg.OffPeakStart, cfg.OffPeakEnd = s, e
	}

	db := openDB(*dbPath)
	defer db.Close()
	svc := service.New(db, cfg, recorder)

	if *once {
		if result := svc.RunCycle(ctx); result.Result == "failed" {
			os.Exit(1)
		}
		return
	}

	httpErr := make(chan error, 1)
	go func() { httpErr <- svc.ListenAndServe(ctx, *addr) }()
	go func() {
		// Without its HTTP endpoint the service is useless to the web server: stop, so
		// that the supervisor (systemd, Docker) restarts it and the failure is visible.
		if err := <-httpErr; err != nil {
			slog.Error("stopping: the HTTP endpoint failed", "component", "service", "event", "service.fatal", oplog.Err(err))
			os.Exit(1)
		}
	}()

	_ = svc.Run(ctx)
}

// runHealthcheck asks a running service for its health. It exists so that a container
// image needs no curl: HEALTHCHECK CMD ["/bin/scraper", "healthcheck"].
func runHealthcheck(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("healthcheck", flag.ExitOnError)
	target := fs.String("url", envOr("BTU_HEALTH_URL", "http://127.0.0.1:8090/healthz"), "Health endpoint of the running service (env BTU_HEALTH_URL)")
	_ = fs.Parse(args)

	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, *target, nil)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(resp.Body, 4096))
	fmt.Println(strings.TrimSpace(string(body)))
	if resp.StatusCode != http.StatusOK {
		os.Exit(1)
	}
}
