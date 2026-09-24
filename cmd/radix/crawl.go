package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogbuild"
	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/crawl"
	"github.com/leonieziechmann/betula/internal/oplog"
	"github.com/leonieziechmann/betula/internal/service"
)

// logFlags are shared by all schema v2 commands. Defaults come from RADIX_LOG_FORMAT,
// RADIX_LOG_LEVEL and RADIX_LOG_FILE, so a container can configure logging by environment.
type logFlags struct {
	format, level, file *string
}

func addLogFlags(fs *flag.FlagSet) *logFlags {
	env := oplog.OptionsFromEnv()
	return &logFlags{
		format: fs.String("log-format", env.Format, "Log format: text or json (env RADIX_LOG_FORMAT)"),
		level:  fs.String("log-level", env.Level, "Log level: debug, info, warn, error (env RADIX_LOG_LEVEL)"),
		file:   fs.String("log-file", env.File, "Also append the log to this file (env RADIX_LOG_FILE)"),
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
	dbPath := fs.String("db", defaultDBPath, "Database path")
	pace := addPaceFlags(fs, 4, 500, 24*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openDB(*dbPath)
	defer db.Close()

	if stats, err := service.CrawlLists(ctx, db, service.BTUEndpoints(), service.Pace{Delay: pace().Delay, MaxAge: time.Hour}); err != nil || stats.Failed > 0 {
		finishCrawl("crawl-modules", stats, err)
	}
	stats, err := service.CrawlModules(ctx, db, service.BTUEndpoints(), pace())
	finishCrawl("crawl-modules", stats, err)
}

// runCrawlQISModules archives the QIS module descriptions, the source the catalog
// is maintained in.
func runCrawlQISModules(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-qis-modules", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	pace := addPaceFlags(fs, 1, 500, 24*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openDB(*dbPath)
	defer db.Close()

	stats, changedRows, err := service.CrawlQISModuleList(ctx, db, service.BTUEndpoints(), service.Pace{Delay: pace().Delay, MaxAge: time.Hour})
	if err != nil || stats.Failed > 0 {
		finishCrawl("crawl-qis-modules", stats, err)
	}
	stats, err = service.CrawlQISModules(ctx, db, service.BTUEndpoints(), service.ModulePace{Pace: pace(), UnsettledMaxAge: pace().MaxAge}, changedRows)
	finishCrawl("crawl-qis-modules", stats, err)
}

// runCrawlEvents looks the events that module pages link up in the QIS event search and
// then archives the event pages the search has news for, or that are past their age.
func runCrawlEvents(ctx context.Context, args []string) {
	def := service.DefaultConfig()
	fs := flag.NewFlagSet("crawl-events", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	pace := addPaceFlags(fs, 1, 500, def.Events.MaxAge)
	listMaxAge := fs.Duration("list-max-age", def.EventList.MaxAge, "Look an event up in the event search again after this long")
	unsettledMaxAge := fs.Duration("unsettled-max-age", def.EventList.UnsettledMaxAge, "Look an event with unsettled dates up again after this long")
	pageMaxAge := fs.Duration("page-max-age", def.Events.ConfirmedMaxAge, "Fetch the page of an event the event search vouches for again after this long")
	unsettledPageMaxAge := fs.Duration("unsettled-page-max-age", def.Events.UnsettledMaxAge, "Fetch the page of an event the event search confirms while its dates are not settled again after this long")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openDB(*dbPath)
	defer db.Close()

	list := service.EventListPace{Pace: service.Pace{Delay: 4 * pace().Delay, MaxAge: *listMaxAge}, UnsettledMaxAge: *unsettledMaxAge}
	if stats, err := service.CrawlEventList(ctx, db, service.BTUEndpoints(), list, true); err != nil || stats.Failed > 0 {
		finishCrawl("crawl-events", stats, err)
	}
	pages := service.EventPagePace{Pace: pace(), ConfirmedMaxAge: *pageMaxAge, UnsettledMaxAge: *unsettledPageMaxAge, EntryFresh: 2 * *listMaxAge}
	stats, err := service.CrawlEvents(ctx, db, service.BTUEndpoints(), pages, true)
	finishCrawl("crawl-events", stats, err)
}

// runCrawlTree walks the QIS program tree and archives what is missing or stale.
func runCrawlTree(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-tree", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	pace := addPaceFlags(fs, 1, 1000, 7*24*time.Hour)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()

	db := openDB(*dbPath)
	defer db.Close()

	stats, err := service.CrawlTree(ctx, db, service.BTUEndpoints(), pace())
	finishCrawl("crawl-tree", stats, err)
}

// runPrune removes everything that is not part of the current dataset: events past
// their retention, and archived pages the current lists and the QIS root no longer
// lead to. Run `build` afterwards to drop them from the canonical tables as well.
func runPrune(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("prune", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	keep := fs.Duration("event-retention", 30*24*time.Hour, "Keep an event this long after its last date")
	grace := fs.Duration("archive-grace", 7*24*time.Hour, "Keep an unused archived page this long after it was fetched (0 removes all unused pages)")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()
	log := oplog.For("retention")

	db := openDB(*dbPath)
	defer db.Close()

	events, err := db.PruneEvents(time.Now(), *keep)
	if err != nil {
		log.Error("pruning events failed", "event", "retention.failed", oplog.Err(err))
		os.Exit(1)
	}
	log.Info("events pruned", "event", "retention.pruned", "removed", events, "keep", keep.String())

	unused, err := catalogbuild.Unused(ctx, db)
	if err != nil {
		log.Error("cannot determine the unused pages", "event", "retention.failed", oplog.Err(err))
		os.Exit(1)
	}
	var cutoff time.Time
	if *grace > 0 {
		cutoff = time.Now().Add(-*grace)
	}
	pages, err := db.PruneArchive(unused, cutoff)
	if err != nil {
		log.Error("pruning the archive failed", "event", "retention.failed", oplog.Err(err))
		os.Exit(1)
	}
	log.Info("archive pruned", "event", "retention.archive_pruned", "removed", pages,
		"unused_module_pages", len(unused[catalogdb.SourceModulePage]), "unused_tree_pages", len(unused[catalogdb.SourceQISTree]), "grace", grace.String())
}
