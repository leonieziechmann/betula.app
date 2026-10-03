package service

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogbuild"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// What the service does, for GET /metrics (docs/radix/operations.md, "Metrics"). The requests
// to the university are counted by the crawl (radix_crawl_*).
var (
	cyclesTotal = metrics.Default.NewCounter("radix_cycles_total",
		"Finished cycles by result: ok, degraded (a crawl stage failed), failed (build, validation or export failed), interrupted.",
		"result")
	cycleRunning = metrics.Default.NewGauge("radix_cycle_running",
		"1 while a cycle runs.")
	cycleCrawls = metrics.Default.NewGauge("radix_cycle_crawls",
		"1 while the running cycle crawls; 0 for a rebuild from the archive.")
	lastCycleSeconds = metrics.Default.NewGauge("radix_last_cycle_duration_seconds",
		"How long the last cycle took.")
	lastCycleAt = metrics.Default.NewGauge("radix_last_cycle_timestamp_seconds",
		"When the last cycle finished (Unix time).")
	lastSuccessAt = metrics.Default.NewGauge("radix_last_success_timestamp_seconds",
		"When the last cycle that was ok or degraded finished (Unix time).")
	failedInARow = metrics.Default.NewGauge("radix_failed_cycles_in_a_row",
		"Failed cycles since the last successful one.")
	nextCycleAt = metrics.Default.NewGauge("radix_next_cycle_timestamp_seconds",
		"When the next cycle starts (Unix time).")
	stageRuns = metrics.Default.NewCounter("radix_stage_runs_total",
		"Stages of the cycles by outcome: ok, failed, skipped.",
		"stage", "outcome")
	stageSeconds = metrics.Default.NewGauge("radix_stage_duration_seconds",
		"How long the last run of a stage took.",
		"stage")
	buildsTotal = metrics.Default.NewCounter("radix_builds_total",
		"Builds of the catalog from the archive, by whether the published content changed.",
		"content")
	publishedTotal = metrics.Default.NewCounter("radix_snapshots_published_total",
		"Snapshots exported for the web server.")
	catalogItems = metrics.Default.NewGauge("radix_catalog_items",
		"What the last build holds: modules, programs, departments, areas, events, events_from_list (dates from the event search only).",
		"kind")
	prunedTotal = metrics.Default.NewCounter("radix_pruned_total",
		"Rows removed by retention: events past their last date, archived pages nothing leads to any more.",
		"what")
	moduleRowsChanged = metrics.Default.NewCounter("radix_module_rows_changed_total",
		"Rows of the QIS module table that read differently than before; their descriptions are fetched again.")
)

// active is the service whose state the scrape-time gauges report: the one serving HTTP.
var active atomic.Pointer[Service]

func init() {
	// The series that are there from the start, at zero: increase() cannot see the first count
	// of a series that appears in the middle of a time range (a first failed cycle).
	for _, result := range []string{"ok", "degraded", "failed", "interrupted"} {
		cyclesTotal.Add(0, result)
	}
	for _, stage := range []string{"lists", "modules", "qis-modules", "tree", "event-list", "events",
		"retention", "build", "archive", "validate", "export", "semantic"} {
		for _, outcome := range []string{"ok", "failed", "skipped"} {
			stageRuns.Add(0, stage, outcome)
		}
	}
	buildsTotal.Add(0, "changed")
	buildsTotal.Add(0, "unchanged")
	publishedTotal.Add(0)
	moduleRowsChanged.Add(0)
	prunedTotal.Add(0, "events")
	prunedTotal.Add(0, "archive_pages")

	metrics.Default.NewGaugeFunc("radix_healthy",
		"1 while GET /healthz answers 200.", nil,
		func(emit func(float64, ...string)) {
			if s := active.Load(); s != nil {
				emit(metrics.Bool(s.Status().Healthy))
			}
		})
	metrics.Default.NewGaugeFunc("radix_offpeak",
		"1 inside the off-peak window, when the bulk stages crawl.", nil,
		func(emit func(float64, ...string)) {
			if s := active.Load(); s != nil {
				emit(metrics.Bool(s.inOffPeak(s.now())))
			}
		})

	// The archive says what was fetched and what changed across restarts, which the
	// counters forget. Read at most once a minute: a scrape comes every 30 s.
	archive := func(read func(st catalogdb.ArchiveStat, emit func(float64, ...string))) func(func(float64, ...string)) {
		return func(emit func(float64, ...string)) {
			s := active.Load()
			if s == nil {
				return
			}
			for _, st := range s.archiveStats() {
				read(st, emit)
			}
		}
	}
	metrics.Default.NewGaugeFunc("radix_archive_pages",
		"Archived pages by source and what the server answered (ok or not_found).", []string{"source", "status"},
		archive(func(st catalogdb.ArchiveStat, emit func(float64, ...string)) {
			emit(float64(st.Pages-st.NotFound), st.Source, "ok")
			emit(float64(st.NotFound), st.Source, "not_found")
		}))
	metrics.Default.NewGaugeFunc("radix_archive_fetched_24h",
		"Archived pages fetched in the last 24 hours, by source.", []string{"source"},
		archive(func(st catalogdb.ArchiveStat, emit func(float64, ...string)) {
			emit(float64(st.FetchedSince), st.Source)
		}))
	metrics.Default.NewGaugeFunc("radix_archive_changed_24h",
		"Archived pages whose content changed (or that are new) in the last 24 hours, by source.", []string{"source"},
		archive(func(st catalogdb.ArchiveStat, emit func(float64, ...string)) {
			emit(float64(st.ChangedSince), st.Source)
		}))
	metrics.Default.NewGaugeFunc("radix_archive_oldest_fetch_timestamp_seconds",
		"The oldest fetch of an archived page, by source (Unix time).", []string{"source"},
		archive(func(st catalogdb.ArchiveStat, emit func(float64, ...string)) {
			emit(float64(st.Oldest.Unix()), st.Source)
		}))
	metrics.Default.NewGaugeFunc("radix_archive_newest_fetch_timestamp_seconds",
		"The newest fetch of an archived page, by source (Unix time).", []string{"source"},
		archive(func(st catalogdb.ArchiveStat, emit func(float64, ...string)) {
			emit(float64(st.Newest.Unix()), st.Source)
		}))
}

// archiveCache keeps the last ArchiveStats for a minute.
type archiveCache struct {
	mu    sync.Mutex
	at    time.Time
	stats []catalogdb.ArchiveStat
}

func (c *archiveCache) invalidate() {
	c.mu.Lock()
	c.at = time.Time{}
	c.mu.Unlock()
}

func (s *Service) archiveStats() []catalogdb.ArchiveStat {
	c := &s.archive
	c.mu.Lock()
	defer c.mu.Unlock()
	now := s.now()
	if !c.at.IsZero() && now.Sub(c.at) < time.Minute {
		return c.stats
	}
	stats, err := s.db.ArchiveStats(now.Add(-24 * time.Hour))
	if err != nil {
		oplog.For("metrics").Warn("cannot sum up the archive", "event", "metrics.archive_failed", oplog.Err(err))
		return c.stats // the last good numbers; a gap would read as an empty archive
	}
	c.at, c.stats = now, stats
	return stats
}

// countCycle records a finished cycle in the metrics.
func countCycle(r CycleResult, lastSuccess time.Time, failed int) {
	cyclesTotal.Inc(r.Result)
	cycleRunning.Set(0)
	cycleCrawls.Set(0)
	lastCycleSeconds.Set(r.FinishedAt.Sub(r.StartedAt).Seconds())
	lastCycleAt.Set(float64(r.FinishedAt.Unix()))
	if !lastSuccess.IsZero() {
		lastSuccessAt.Set(float64(lastSuccess.Unix()))
	}
	failedInARow.Set(float64(failed))
	for _, st := range r.Stages {
		outcome := "ok"
		switch {
		case st.Skipped != "":
			outcome = "skipped"
		case st.Error != "":
			outcome = "failed"
		}
		stageRuns.Inc(st.Name, outcome)
		if st.Skipped == "" {
			stageSeconds.Set(float64(st.DurationMS)/1000, st.Name)
		}
	}
	if r.Published {
		publishedTotal.Inc()
	}
}

// countBuild records what a build found.
func countBuild(report *catalogbuild.Report) {
	content := "unchanged"
	if report.ContentChanged {
		content = "changed"
	}
	buildsTotal.Inc(content)
	for kind, n := range map[string]int{
		"modules": report.Modules, "programs": report.Programs, "departments": report.Departments,
		"areas": report.Areas, "events": report.Events, "events_from_list": report.EventsFromList,
	} {
		catalogItems.Set(float64(n), kind)
	}
}
