package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/crawl"
	"github.com/leonieziechmann/btu-scraper/internal/oplog"
	"github.com/leonieziechmann/btu-scraper/internal/service"
)

// logFlags are shared by all schema v2 commands. Defaults come from BTU_LOG_FORMAT,
// BTU_LOG_LEVEL and BTU_LOG_FILE, so a container can configure logging by environment.
type logFlags struct {
	format, level, file *string
}

func addLogFlags(fs *flag.FlagSet) *logFlags {
	env := oplog.OptionsFromEnv()
	return &logFlags{
		format: fs.String("log-format", env.Format, "Log format: text or json (env BTU_LOG_FORMAT)"),
		level:  fs.String("log-level", env.Level, "Log level: debug, info, warn, error (env BTU_LOG_LEVEL)"),
		file:   fs.String("log-file", env.File, "Also append the log to this file (env BTU_LOG_FILE)"),
	}
}

// setup installs the logger. The returned function flushes the log file.
func (l *logFlags) setup() (*oplog.Recorder, func()) {
	recorder, closeLog, err := oplog.Setup(oplog.Options{Format: *l.format, Level: *l.level, File: *l.file})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(2)
	}
	return recorder, func() { _ = closeLog() }
}

func addPaceFlags(fs *flag.FlagSet, workers, delayMs int, maxAge time.Duration) func() service.Pace {
	w := fs.Int("workers", workers, "Parallel fetchers")
	d := fs.Int("delay", delayMs, "Pause per worker after each request in milliseconds (±30% jitter)")
	a := fs.Duration("max-age", maxAge, "Do not fetch pages archived more recently than this (0 refetches everything)")
	l := fs.Int("limit", 0, "Request at most this many pages, oldest first (0 for all)")
	return func() service.Pace {
		return service.Pace{Workers: *w, Delay: time.Duration(*d) * time.Millisecond, MaxAge: *a, Limit: *l}
	}
}

// finishCrawl turns the outcome of a crawl command into an exit code:
// 0 everything archived, 1 pages failed or the crawl was aborted, 130 interrupted.
func finishCrawl(name string, stats crawl.Stats, err error) {
	switch {
	case errors.Is(err, context.Canceled):
		slog.Warn("interrupted; run the command again to resume, archived pages are skipped", "component", "cli", "event", "cli.interrupted", "command", name)
		os.Exit(130)
	case err != nil:
		slog.Error("command failed", "component", "cli", "event", "cli.failed", "command", name, oplog.Err(err))
		os.Exit(1)
	case stats.Failed > 0:
		slog.Error("some pages could not be fetched", "component", "cli", "event", "cli.failed", "command", name, "failed", stats.Failed)
		os.Exit(1)
	}
}

// runCrawlModules archives the module catalog list, the FÜS list and every module page.
func runCrawlModules(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-modules", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	pace := addPaceFlags(fs, 4, 500, 24*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openV2(*dbPath)
	defer db.Close()

	if stats, err := service.CrawlLists(ctx, db, service.BTUEndpoints(), service.Pace{Delay: pace().Delay, MaxAge: time.Hour}); err != nil || stats.Failed > 0 {
		finishCrawl("crawl-modules", stats, err)
	}
	stats, err := service.CrawlModules(ctx, db, service.BTUEndpoints(), pace())
	finishCrawl("crawl-modules", stats, err)
}

// runCrawlEvents archives the QIS event pages that module pages link.
func runCrawlEvents(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-events", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	pace := addPaceFlags(fs, 1, 500, 72*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openV2(*dbPath)
	defer db.Close()

	stats, err := service.CrawlEvents(ctx, db, service.BTUEndpoints(), pace())
	finishCrawl("crawl-events", stats, err)
}

// runCrawlTree walks the QIS program tree and archives what is missing or stale.
func runCrawlTree(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-tree", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	pace := addPaceFlags(fs, 1, 1000, 7*24*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openV2(*dbPath)
	defer db.Close()

	stats, err := service.CrawlTree(ctx, db, service.BTUEndpoints(), pace())
	finishCrawl("crawl-tree", stats, err)
}

// runPruneEvents applies the retention rule to archived events.
func runPruneEvents(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("prune-events", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	keep := fs.Duration("keep", 30*24*time.Hour, "Keep an event this long after its last date")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openV2(*dbPath)
	defer db.Close()

	removed, err := db.PruneEvents(time.Now(), *keep)
	if err != nil {
		slog.Error("pruning events failed", "component", "retention", "event", "retention.failed", oplog.Err(err))
		os.Exit(1)
	}
	slog.Info("events pruned", "component", "retention", "event", "retention.pruned", "removed", removed, "keep", keep.String())
}
