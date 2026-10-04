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

	cortexclient "github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/service"
	"github.com/leonieziechmann/betula/radix/internal/version"
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
	dbPath := fs.String("db", envOr("RADIX_DB", defaultDBPath), "Database path (env RADIX_DB)")
	snapshotDir := fs.String("snapshot-dir", envOr("RADIX_SNAPSHOT_DIR", def.SnapshotDir), "Directory for exported snapshots (env RADIX_SNAPSHOT_DIR)")
	addr := fs.String("addr", envOr("RADIX_ADDR", "127.0.0.1:8090"), "Listen address for /snapshot, /healthz and /status (env RADIX_ADDR)")
	interval := fs.Duration("interval", envDuration("RADIX_INTERVAL", def.Interval), "Pause between cycles (env RADIX_INTERVAL)")
	offpeak := fs.String("offpeak", envOr("RADIX_OFFPEAK", "1-6"), "Local hours for bulk crawling, START-END, or 'any' (env RADIX_OFFPEAK)")
	moduleDelay := fs.Int("module-delay", envInt("RADIX_MODULE_DELAY_MS", int(def.Modules.Delay.Milliseconds())), "Pause between module page requests in ms (env RADIX_MODULE_DELAY_MS)")
	qisDelay := fs.Int("qis-delay", envInt("RADIX_QIS_DELAY_MS", int(def.Events.Delay.Milliseconds())), "Pause between QIS requests in ms (env RADIX_QIS_DELAY_MS)")
	listMaxAge := fs.Duration("list-max-age", envDuration("RADIX_LIST_MAX_AGE", def.Lists.MaxAge), "Refetch the module index (catalog list, FÜS list, QIS module table) after this long, off-peak (env RADIX_LIST_MAX_AGE)")
	moduleMaxAge := fs.Duration("module-max-age", envDuration("RADIX_MODULE_MAX_AGE", def.Modules.MaxAge), "Refetch a module page once per this long, each on a day of its own (env RADIX_MODULE_MAX_AGE)")
	qisModuleMaxAge := fs.Duration("qis-module-max-age", envDuration("RADIX_QIS_MODULE_MAX_AGE", def.QISModules.MaxAge), "Refetch a QIS module description nothing it depends on changed for once per this long, each on a day of its own (env RADIX_QIS_MODULE_MAX_AGE)")
	qisModuleUnsettledMaxAge := fs.Duration("qis-module-unsettled-max-age", envDuration("RADIX_QIS_MODULE_UNSETTLED_MAX_AGE", def.QISModules.UnsettledMaxAge), "Refetch the QIS description of a module offered this semester that names none of its events once per this long (env RADIX_QIS_MODULE_UNSETTLED_MAX_AGE)")
	eventListMaxAge := fs.Duration("event-list-max-age", envDuration("RADIX_EVENT_LIST_MAX_AGE", def.EventList.MaxAge), "Look every linked event up in the event search again after this long, off-peak (env RADIX_EVENT_LIST_MAX_AGE)")
	eventUnsettledMaxAge := fs.Duration("event-unsettled-max-age", envDuration("RADIX_EVENT_UNSETTLED_MAX_AGE", def.EventList.UnsettledMaxAge), "Look an event whose dates are not settled up again after this long, at any hour (env RADIX_EVENT_UNSETTLED_MAX_AGE)")
	eventPageMaxAge := fs.Duration("event-page-max-age", envDuration("RADIX_EVENT_PAGE_MAX_AGE", def.Events.ConfirmedMaxAge), "Refetch an event page the event search vouches for once per this long, each on a day of its own (env RADIX_EVENT_PAGE_MAX_AGE)")
	eventUnsettledPageMaxAge := fs.Duration("event-unsettled-page-max-age", envDuration("RADIX_EVENT_UNSETTLED_PAGE_MAX_AGE", def.Events.UnsettledMaxAge), "Refetch an event page the event search confirms while its dates are not settled once per this long (env RADIX_EVENT_UNSETTLED_PAGE_MAX_AGE)")
	eventMaxAge := fs.Duration("event-max-age", envDuration("RADIX_EVENT_MAX_AGE", def.Events.MaxAge), "Refetch an event page in doubt, one the event search does not confirm, once per this long (env RADIX_EVENT_MAX_AGE)")
	treeMaxAge := fs.Duration("tree-max-age", envDuration("RADIX_TREE_MAX_AGE", def.Tree.MaxAge), "Refetch a QIS tree page once per this long, each on a day of its own (env RADIX_TREE_MAX_AGE)")
	retention := fs.Duration("event-retention", envDuration("RADIX_EVENT_RETENTION", def.EventRetention), "Remove an event this long after its last date, 0 keeps all (env RADIX_EVENT_RETENTION)")
	archiveGrace := fs.Duration("archive-grace", envDuration("RADIX_ARCHIVE_GRACE", def.ArchiveGrace), "Remove archived pages nothing leads to any more this long after their fetch, 0 keeps them (env RADIX_ARCHIVE_GRACE)")
	staleAfter := fs.Duration("stale-after", envDuration("RADIX_STALE_AFTER", def.StaleAfter), "Report unhealthy without a successful cycle for this long (env RADIX_STALE_AFTER)")
	once := fs.Bool("once", false, "Run a single cycle and exit (exit code 1 if it failed)")
	cortex := addCortexFlags(fs)
	semanticOpts := addSemanticFlags(fs)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)

	if *logs.format == "" {
		*logs.format = "json" // a service logs for machines unless told otherwise
	}
	recorder, closeLog := logs.setup()
	defer closeLog()
	// Offline through Cortex the cycle reads Cortex's store and nothing else: no Gemini either.
	offline := cortex.offline()
	if offline {
		semanticOpts.summaryModel = nil
	}

	cfg := def
	cfg.SnapshotDir = *snapshotDir
	cfg.Interval = *interval
	cfg.Lists.MaxAge = *listMaxAge
	cfg.Modules.Delay = time.Duration(*moduleDelay) * time.Millisecond
	cfg.Modules.MaxAge = *moduleMaxAge
	cfg.QISModules.Delay = time.Duration(*qisDelay) * time.Millisecond
	cfg.QISModules.MaxAge = *qisModuleMaxAge
	cfg.QISModules.UnsettledMaxAge = *qisModuleUnsettledMaxAge
	cfg.EventList.Delay = 4 * time.Duration(*qisDelay) * time.Millisecond
	cfg.EventList.MaxAge = *eventListMaxAge
	cfg.EventList.UnsettledMaxAge = *eventUnsettledMaxAge
	cfg.Events.Delay = time.Duration(*qisDelay) * time.Millisecond
	cfg.Events.MaxAge = *eventMaxAge
	cfg.Events.ConfirmedMaxAge = *eventPageMaxAge
	cfg.Events.UnsettledMaxAge = *eventUnsettledPageMaxAge
	cfg.Events.EntryFresh = 2 * *eventListMaxAge
	cfg.Tree.Delay = 2 * time.Duration(*qisDelay) * time.Millisecond
	cfg.Tree.MaxAge = *treeMaxAge
	cfg.EventRetention = *retention
	cfg.ArchiveGrace = *archiveGrace
	cfg.StaleAfter = *staleAfter
	cfg.Endpoints.Client = cortex.client(cortexclient.ModeCache)
	declareFetchPath(cfg.Endpoints.Client != nil, offline)

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

	semantic, closeSemantic, err := semanticOpts.setup(ctx)
	if err != nil {
		slog.Error("cannot start the semantic search's encoder", "component", "cli", "event", "cli.failed", oplog.Err(err))
		os.Exit(2)
	}
	defer closeSemantic()
	cfg.Semantic = semantic

	db := openDB(*dbPath)
	defer db.Close()
	svc := service.New(db, cfg, recorder)
	if offline {
		declareBuildInfo("cortex-offline")
	} else {
		declareBuildInfo("run")
	}

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

// declareBuildInfo names the running binary and its mode in GET /metrics: run, cortex-offline
// (run with --cortex-mode offline) or serve-snapshot.
func declareBuildInfo(mode string) {
	build := version.Build()
	if len(build) > 12 {
		build = build[:12]
	}
	started := float64(time.Now().Unix())
	metrics.Default.NewGaugeFunc("radix_build_info",
		"Always 1: the binary that runs (the start of its hash, as meta radix_build) and its mode: run, cortex-offline (run, every page from Cortex's store alone) or serve-snapshot.",
		[]string{"build", "mode"}, func(emit func(float64, ...string)) { emit(1, build, mode) })
	metrics.Default.NewGaugeFunc("radix_start_time_seconds",
		"When the process started (Unix time).", nil, func(emit func(float64, ...string)) { emit(started) })
}

// declareFetchPath says in GET /metrics whether the crawl goes through Cortex: the crawl's
// radix_crawl_* metrics then count requests to Cortex, most of them answered from its store,
// and what reached the university is Cortex's cortex_upstream_requests_total. Offline, none of
// them did.
func declareFetchPath(viaCortex, offline bool) {
	v, o := metrics.Bool(viaCortex), metrics.Bool(offline)
	metrics.Default.NewGaugeFunc("radix_crawl_via_cortex",
		"1 when the crawl and the statute download go through Cortex (RADIX_CORTEX_URL), 0 when Radix asks the university itself.",
		nil, func(emit func(float64, ...string)) { emit(v) })
	metrics.Default.NewGaugeFunc("radix_crawl_cortex_offline",
		"1 when every request goes to Cortex in mode offline (RADIX_CORTEX_MODE=offline): answered from its store alone, nothing reaches the university.",
		nil, func(emit func(float64, ...string)) { emit(o) })
}

// runHealthcheck asks a running service for its health. It exists so that a container
// image needs no curl: HEALTHCHECK CMD ["/bin/radix", "healthcheck"].
func runHealthcheck(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("healthcheck", flag.ExitOnError)
	target := fs.String("url", envOr("RADIX_HEALTH_URL", "http://127.0.0.1:8090/healthz"), "Health endpoint of the running service (env RADIX_HEALTH_URL)")
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
