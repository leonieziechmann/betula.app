package main

import (
	"bytes"
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net/url"
	"os"
	"slices"
	"strings"
	"sync"
	"time"

	cortexclient "github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// seedMaxFailuresInRow stops a seed that keeps failing: Cortex is down or refuses everything.
const seedMaxFailuresInRow = 10

// seedStats counts what Cortex made of the pages a seed gave it.
type seedStats struct {
	Pages     int
	Bytes     int64
	Results   map[string]int // by cortexclient.ImportCreated, …; "counted" in a dry run
	Failed    int
	BySource  map[string]int
	Abandoned bool // stopped after seedMaxFailuresInRow failures in a row
}

// runSeedCortex gives Cortex the archive's answers (docs/cortex/cortex.md §4.3): every archived
// page of the sources whose page is the whole answer of its URL, with the archive's times, so
// that Cortex serves each one as if it had fetched it itself, offline too. Run once when Cortex
// starts out next to an archive that was crawled without it; again whenever that is worth it:
// an answer Cortex has already, or newer, changes nothing. The database is only read, as it
// is (catalogdb.OpenReadOnly): take a copy of one a Radix writes to.
//
// Log events: seed.started, seed.progress, seed.failed (WARN), seed.finished, cli.failed (ERROR).
func runSeedCortex(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("seed-cortex", flag.ExitOnError)
	dbPath := fs.String("db", envOr("RADIX_DB", defaultDBPath), "Database path, read as it is: a copy of a database a Radix writes to (env RADIX_DB)")
	urls := fs.String("cortex", envOr("RADIX_CORTEX_URL", ""), "Cortex's instances, comma-separated (env RADIX_CORTEX_URL)")
	sources := fs.String("sources", strings.Join(catalogdb.WholeAnswerSources, ","), "The archive's sources to give, comma-separated; only those whose page is the whole answer of its URL")
	workers := fs.Int("workers", 2, "Pages given to Cortex at a time")
	dryRun := fs.Bool("dry-run", false, "Count what would be given, and send nothing")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()
	fail := func(msg string, attrs ...any) {
		slog.Error(msg, append([]any{"component", "cli", "event", "cli.failed", "command", "seed-cortex"}, attrs...)...)
		closeLog()
		os.Exit(2)
	}

	var picked []string
	for _, s := range strings.Split(*sources, ",") {
		if s = strings.TrimSpace(s); s == "" {
			continue
		}
		if !slices.Contains(catalogdb.WholeAnswerSources, s) {
			fail("not a source whose page is the whole answer of its URL", "source", s, "sources", strings.Join(catalogdb.WholeAnswerSources, ","))
		}
		picked = append(picked, s)
	}
	if *workers < 1 || *workers > 16 {
		fail("--workers must be 1 to 16", "value", *workers)
	}
	db, err := catalogdb.OpenReadOnly(*dbPath)
	if err != nil {
		fail("cannot read the database", "db", *dbPath, oplog.Err(err))
	}
	defer db.Close()

	var client *cortexclient.Client
	if !*dryRun {
		if strings.TrimSpace(*urls) == "" {
			fail("--cortex is required (or --dry-run)")
		}
		if client, err = cortexclient.New(*urls, cortexclient.Options{}); err != nil {
			fail("invalid --cortex", "value", *urls, oplog.Err(err))
		}
	}
	stats, err := seedCortex(ctx, db, client, picked, *workers)
	switch {
	case errors.Is(err, context.Canceled):
		slog.Warn("interrupted; run the command again: what Cortex has already changes nothing", "component", "cli", "event", "cli.interrupted", "command", "seed-cortex")
		closeLog()
		os.Exit(130)
	case err != nil:
		slog.Error("seed failed", "component", "cli", "event", "cli.failed", "command", "seed-cortex", oplog.Err(err))
		closeLog()
		os.Exit(1)
	case stats.Failed > 0:
		slog.Error("Cortex did not take every page", "component", "cli", "event", "cli.failed", "command", "seed-cortex", "failed", stats.Failed)
		closeLog()
		os.Exit(1)
	}
}

// seedCortex gives client the archived pages of sources (nil: counts them only), workers at a
// time.
func seedCortex(ctx context.Context, db *catalogdb.DB, client *cortexclient.Client, sources []string, workers int) (seedStats, error) {
	log := oplog.For("seed")
	stats := seedStats{Results: map[string]int{}, BySource: map[string]int{}}
	log.Info("giving Cortex the archive's answers", "event", "seed.started", "db", db.Path(), "sources", strings.Join(sources, ","),
		"workers", workers, "dry_run", client == nil)
	start := time.Now()

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	pages := make(chan *catalogdb.RawPage)
	var (
		mu        sync.Mutex
		inRow     int
		abandoned bool
		wg        sync.WaitGroup
	)
	done := func(p *catalogdb.RawPage, result string, err error) {
		mu.Lock()
		defer mu.Unlock()
		stats.Pages++
		stats.Bytes += int64(len(p.Body))
		stats.BySource[p.Source]++
		if err != nil {
			stats.Failed++
			inRow++
			log.Warn("Cortex did not take a page", "event", "seed.failed", "source", p.Source, "key", p.Key, "url", p.URL, oplog.Err(err))
			if inRow >= seedMaxFailuresInRow && !abandoned {
				abandoned = true
				cancel()
			}
		} else {
			inRow = 0
			stats.Results[result]++
		}
		if stats.Pages%1000 == 0 {
			log.Info("seed progress", "event", "seed.progress", "pages", stats.Pages, "created", stats.Results[cortexclient.ImportCreated],
				"failed", stats.Failed, "elapsed_s", int(time.Since(start).Seconds()))
		}
	}
	for range workers {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for p := range pages {
				if client == nil {
					done(p, "counted", nil)
					continue
				}
				result, err := seedPage(ctx, client, p)
				if ctx.Err() != nil {
					return // interrupted, or given up: no failure of the page
				}
				done(p, result, err)
			}
		}()
	}

	var readErr error
	for _, source := range sources {
		readErr = db.EachPage(source, func(p *catalogdb.RawPage) error {
			select {
			case pages <- p:
				return nil
			case <-ctx.Done():
				return ctx.Err()
			}
		})
		if readErr != nil {
			break
		}
	}
	close(pages)
	wg.Wait()

	stats.Abandoned = abandoned
	level := slog.LevelInfo
	if stats.Failed > 0 {
		level = slog.LevelWarn
	}
	log.Log(context.Background(), level, "seed finished", "event", "seed.finished", "pages", stats.Pages, "bytes", stats.Bytes,
		"created", stats.Results[cortexclient.ImportCreated], "checked", stats.Results[cortexclient.ImportChecked],
		"unchanged", stats.Results[cortexclient.ImportUnchanged], "older", stats.Results[cortexclient.ImportOlder],
		"failed", stats.Failed, "by_source", stats.BySource, "abandoned", abandoned, "duration_s", int(time.Since(start).Seconds()))
	switch {
	case abandoned:
		return stats, fmt.Errorf("stopped after %d pages in a row that Cortex did not take", seedMaxFailuresInRow)
	case readErr != nil && !errors.Is(readErr, context.Canceled):
		return stats, fmt.Errorf("reading the archive: %w", readErr)
	case ctx.Err() != nil:
		return stats, ctx.Err()
	}
	return stats, nil
}

// seedPage gives Cortex one archived page: the URL as the crawl asks for it (through Go's
// request, as cortexclient.Transport sends it), its status, and its times: the content was
// first fetched when it last changed (changed_at), last fetched at fetched_at. Sent again once
// when the answer was lost on the way: an import changes nothing the second time.
func seedPage(ctx context.Context, client *cortexclient.Client, p *catalogdb.RawPage) (string, error) {
	u, err := url.Parse(p.URL)
	if err != nil {
		return "", fmt.Errorf("the archived URL: %w", err)
	}
	u.Fragment, u.RawFragment = "", ""
	first := p.ChangedAt
	if first.IsZero() || first.After(p.FetchedAt) {
		first = p.FetchedAt
	}
	o := cortexclient.ImportOptions{Status: p.HTTPStatus, FetchedAt: first, CheckedAt: p.FetchedAt, Source: p.Source}
	if p.Hash != "" {
		o.Expect = "sha256:" + p.Hash
	}
	var res cortexclient.ImportResult
	for attempt := 0; attempt < 2; attempt++ {
		res, err = client.Import(ctx, u.String(), bytes.NewReader(p.Body), o)
		if !errors.Is(err, cortexclient.ErrOutcomeUnknown) {
			break
		}
	}
	return res.Result, err
}
