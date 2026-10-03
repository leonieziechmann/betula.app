package service

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"runtime/debug"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogbuild"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/crawl"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/snapshothttp"
	"github.com/leonieziechmann/betula/radix/internal/version"
)

// Config is the behaviour of the long-running service.
type Config struct {
	Endpoints   Endpoints
	SnapshotDir string
	Interval    time.Duration // pause between cycles

	// Crawling only happens between these local hours, [start, end), except for the
	// event search's look at the events in doubt every two hours and the pages it has
	// news for. Equal values mean any time.
	OffPeakStart, OffPeakEnd int

	// The module index (the two lists and the QIS module table) is read every second night;
	// QIS module descriptions and the program tree change seldom and are read once a month,
	// the copy of the module pages on b-tu.de, a server of its own, weekly. A QIS module
	// description is read again sooner when something it depends on changed: its row in the
	// module table, the semester QIS calls current, or, weekly, while it names none of the
	// events of a semester the module is offered in. With Spread every page has a day of its
	// own in its period (crawl.Due), so that each night carries an even share of the month
	// or week. A Limit is per cycle; the off-peak window has about five.
	Lists, Modules, Tree Pace
	QISModules           ModulePace

	// EventList asks the QIS event search about the linked events, 250 to a request:
	// every event once per MaxAge in the off-peak window, and an event whose dates are not
	// settled every UnsettledMaxAge at any hour. Events fetches the page of an event when
	// the search has news for it, at any hour, and in the off-peak window also when its
	// day in its period has come.
	EventList EventListPace
	Events    EventPagePace

	EventRetention time.Duration        // keep an event this long after its last date; 0 keeps everything
	ArchiveGrace   time.Duration        // remove archived pages nothing leads to any more, this long after their fetch; 0 keeps them
	Baselines      []catalogdb.Baseline // count baselines for validate
	StaleAfter     time.Duration        // health: unhealthy without a successful cycle for this long

	// Semantic computes the vectors of Folia's semantic search (semantic.go); without an
	// encoder it does not run.
	Semantic Semantic
}

// DefaultConfig is a polite setup for the BTU servers, which asks QIS for what changes as
// often as it changes: the module index every second night; a QIS module description and a
// page of the program tree once a month, a description sooner when something it depends on
// changed; the dates of all events in about ten requests of the event search a night, and
// those not settled yet every two hours, 250 to a request; an event page when the search
// shows a change, weekly while the search confirms dates that are not settled yet, every
// three days while its dates are in doubt, and otherwise once a month. The copy of the
// module pages on b-tu.de is read weekly: another server, and a fast one. Every page of a
// weekly or monthly rhythm has a day of its own in it, so that the nights carry the same
// load instead of an archive read in a few nights coming due in a few nights again.
//
// The limits spread what comes at once, a new semester, over a few nights: 200 QIS module
// descriptions and 200 event pages a cycle, about five cycles a night. The dates of a new
// semester's events come from the event search meanwhile.
func DefaultConfig() Config {
	return Config{
		Endpoints:    BTUEndpoints(),
		SnapshotDir:  "snapshot",
		Interval:     30 * time.Minute,
		OffPeakStart: 1,
		OffPeakEnd:   6,
		// 40 hours: every second night, whatever time of the night the last reading was.
		Lists:   Pace{Delay: time.Second, MaxAge: 40 * time.Hour},
		Modules: Pace{Workers: 1, Delay: 500 * time.Millisecond, MaxAge: 7 * 24 * time.Hour, Spread: true, Limit: 400},
		QISModules: ModulePace{
			Pace:            Pace{Workers: 1, Delay: 500 * time.Millisecond, MaxAge: 30 * 24 * time.Hour, Spread: true, Limit: 200},
			UnsettledMaxAge: 7 * 24 * time.Hour,
		},
		EventList: EventListPace{Pace: Pace{Delay: 2 * time.Second, MaxAge: 12 * time.Hour}, UnsettledMaxAge: 2 * time.Hour},
		Events: EventPagePace{
			Pace:            Pace{Workers: 1, Delay: 500 * time.Millisecond, MaxAge: 3 * 24 * time.Hour, Spread: true, Limit: 200},
			ConfirmedMaxAge: 30 * 24 * time.Hour,
			UnsettledMaxAge: 7 * 24 * time.Hour,
			EntryFresh:      24 * time.Hour, // twice the list's MaxAge: a search that stopped working vouches for nothing after a day
			DayLimit:        50,
		},
		Tree:           Pace{Delay: time.Second, MaxAge: 30 * 24 * time.Hour, Spread: true, Limit: 300},
		EventRetention: 30 * 24 * time.Hour,
		ArchiveGrace:   7 * 24 * time.Hour,
		Baselines:      catalogdb.BTUBaselines,
		StaleAfter:     26 * time.Hour,
	}
}

// StageResult is the outcome of one stage of a cycle.
type StageResult struct {
	Name       string       `json:"name"`
	DurationMS int64        `json:"duration_ms"`
	Crawl      *crawl.Stats `json:"crawl,omitempty"`
	Skipped    string       `json:"skipped,omitempty"` // why the stage did not run
	Error      string       `json:"error,omitempty"`
}

// CycleResult is the outcome of one cycle. Result is "ok" (everything ran),
// "degraded" (a crawl stage failed; the published data is intact but may age) or
// "failed" (build, validation or export failed; the previous snapshot stays current).
type CycleResult struct {
	StartedAt  time.Time     `json:"started_at"`
	FinishedAt time.Time     `json:"finished_at"`
	Result     string        `json:"result"`
	Published  bool          `json:"published"` // a new snapshot was exported
	Stages     []StageResult `json:"stages"`
}

// Service keeps the catalog up to date and serves its snapshots.
type Service struct {
	db       *catalogdb.DB
	cfg      Config
	recorder *oplog.Recorder
	now      func() time.Time

	mu            sync.Mutex
	startedAt     time.Time
	cycles        int
	last          *CycleResult
	lastSuccessAt time.Time
	failedInARow  int
	nextCycleAt   time.Time

	summaryFailed map[string]time.Time // text hash → when Gemini last failed to summarise it (semantic.go)
	semanticLast  semanticStats        // what the semantic stage did last (RunOffline)
	archive       archiveCache         // for GET /metrics
}

// New creates a service. recorder may be nil.
func New(db *catalogdb.DB, cfg Config, recorder *oplog.Recorder) *Service {
	return &Service{db: db, cfg: cfg, recorder: recorder, now: time.Now, startedAt: time.Now()}
}

// Run executes cycles until ctx is cancelled. A failing cycle never ends the
// service: it is logged, shows in /status and /healthz, and the next cycle tries again.
func (s *Service) Run(ctx context.Context) error {
	log := oplog.For("service")
	log.Info("service started", "event", "service.started", "interval", s.cfg.Interval.String(),
		"offpeak_start", s.cfg.OffPeakStart, "offpeak_end", s.cfg.OffPeakEnd, "snapshot_dir", s.cfg.SnapshotDir)

	// A new release may read the archived pages differently (a parser, a rule). Its first
	// cycle would apply that only after crawling, hours at night; offline never. So it
	// builds from the archive first and publishes what the new rules make of it.
	if s.BuiltByOtherRelease() {
		s.Rebuild(ctx)
	}

	for {
		s.RunCycle(ctx)
		if ctx.Err() != nil {
			log.Info("service stopped", "event", "service.stopped")
			return nil
		}

		s.mu.Lock()
		s.nextCycleAt = s.now().Add(s.cfg.Interval)
		nextCycleAt.Set(float64(s.nextCycleAt.Unix()))
		s.mu.Unlock()

		timer := time.NewTimer(s.cfg.Interval)
		select {
		case <-ctx.Done():
			timer.Stop()
			log.Info("service stopped", "event", "service.stopped")
			return nil
		case <-timer.C:
		}
	}
}

// RunCycle runs one cycle: crawl what is due, apply retention, build, and if the content changed
// validate and export; then, unless the cycle failed, compute the semantic search's missing
// vectors (semantic.go), which the next build publishes.
//
// Log events: cycle.started, stage.finished / stage.failed (ERROR), cycle.finished
// (ERROR when the result is "failed", WARN when "degraded"), cycle.panic (ERROR).
func (s *Service) RunCycle(ctx context.Context) CycleResult {
	return s.cycle(ctx, true)
}

// Rebuild is a cycle without the crawl: it derives the catalog from the archive as it is
// and publishes it if the content changed. It sends nothing to the university.
//
// Log events: service.rebuild, then those of RunCycle.
func (s *Service) Rebuild(ctx context.Context) CycleResult {
	oplog.For("service").Info("the catalog was built by another release; building it again from the archive",
		"event", "service.rebuild", "built_by", s.builtBy(), "this_build", version.Build())
	return s.cycle(ctx, false)
}

// RunOffline is what an offline Radix does besides serving its snapshot (serve-snapshot --db),
// and it sends nothing out: neither to the university nor to Gemini. A catalog another release
// built is built again from the archive (Rebuild). With an embedding model, the semantic stage
// then computes the vectors of the passages that have none (the modules' texts, with the
// summaries the database brought from the instance that crawls), a budget at a time, and a
// cycle without the crawl publishes each part, until a stage finds nothing left to compute.
//
// Log events: those of Rebuild and RunCycle, semantic.finished.
func (s *Service) RunOffline(ctx context.Context) {
	s.cfg.Semantic.Summarizer = nil
	if s.BuiltByOtherRelease() {
		s.Rebuild(ctx) // which ends with the semantic stage, as every cycle does
	} else if s.cfg.Semantic.Encoder != nil {
		s.semanticStage(ctx, &CycleResult{Result: "ok"})
	}
	// What a stage computed, the next build publishes; that cycle's stage computes more.
	for s.cfg.Semantic.Encoder != nil && ctx.Err() == nil && s.semanticLast.VectorsWritten > 0 {
		s.semanticLast = semanticStats{}
		if s.cycle(ctx, false).Result == "failed" {
			return
		}
	}
}

// BuiltByOtherRelease says whether the canonical tables were derived by another binary than
// this one (meta radix_build), or never. The build writes it, so a Rebuild ends it.
func (s *Service) BuiltByOtherRelease() bool {
	current := version.Build()
	return current == "" || s.builtBy() != current
}

func (s *Service) builtBy() string {
	var built string
	_ = s.db.SQL().QueryRow("SELECT value FROM meta WHERE key = 'radix_build'").Scan(&built)
	return built
}

func (s *Service) cycle(ctx context.Context, crawlFirst bool) (result CycleResult) {
	log := oplog.For("service")
	result = CycleResult{StartedAt: s.now(), Result: "ok"}
	cycleRunning.Set(1)
	cycleCrawls.Set(metrics.Bool(crawlFirst))
	log.Info("cycle started", "event", "cycle.started", "offpeak", s.inOffPeak(result.StartedAt), "crawl", crawlFirst)

	defer func() {
		if r := recover(); r != nil {
			result.Result = "failed"
			log.Error("cycle panicked", "event", "cycle.panic", "panic", fmt.Sprint(r), "stack", string(debug.Stack()))
		}
		result.FinishedAt = s.now()
		s.record(result)

		attrs := []any{"event", "cycle.finished", "result", result.Result, "published", result.Published,
			"duration_s", int(result.FinishedAt.Sub(result.StartedAt).Seconds())}
		switch result.Result {
		case "failed":
			log.Error("cycle failed; the previous snapshot stays current", attrs...)
		case "degraded":
			log.Warn("cycle finished with crawl problems", attrs...)
		default:
			log.Info("cycle finished", attrs...)
		}
	}()

	// 1. Crawl. A failing source degrades the cycle but does not stop the others.
	crawlStage := func(name string, bulk bool, source string, run func() (crawl.Stats, error)) {
		if ctx.Err() != nil || !crawlFirst {
			return
		}
		if bulk && !s.inOffPeak(s.now()) && s.archived(source) {
			result.Stages = append(result.Stages, StageResult{Name: name, Skipped: "outside the off-peak window"})
			return
		}
		start := s.now()
		stats, err := run()
		stage := StageResult{Name: name, DurationMS: s.now().Sub(start).Milliseconds(), Crawl: &stats}
		if err != nil && ctx.Err() == nil {
			stage.Error = err.Error()
			result.Result = "degraded"
			log.Error("crawl stage failed", "event", "stage.failed", "stage", name, oplog.Err(err))
		} else if stats.Failed > 0 {
			result.Result = "degraded"
		}
		result.Stages = append(result.Stages, stage)
	}
	crawlStage("lists", true, catalogdb.SourceModuleCatalog, func() (crawl.Stats, error) { return CrawlLists(ctx, s.db, s.cfg.Endpoints, s.cfg.Lists) })
	crawlStage("modules", true, catalogdb.SourceModulePage, func() (crawl.Stats, error) { return CrawlModules(ctx, s.db, s.cfg.Endpoints, s.cfg.Modules) })
	// The QIS descriptions carry the events of the semester that runs now, so they
	// are read before the events they name. The table they list comes first.
	crawlStage("qis-modules", true, catalogdb.SourceQISModulePage, func() (crawl.Stats, error) {
		stats, changedRows, err := CrawlQISModuleList(ctx, s.db, s.cfg.Endpoints, s.cfg.Lists)
		if err != nil || stats.Failed > 0 {
			return stats, err
		}
		return CrawlQISModules(ctx, s.db, s.cfg.Endpoints, s.cfg.QISModules, changedRows)
	})
	crawlStage("tree", true, catalogdb.SourceQISTree, func() (crawl.Stats, error) { return CrawlTree(ctx, s.db, s.cfg.Endpoints, s.cfg.Tree) })
	// The event search runs in every cycle: at night it reads the dates of every linked
	// event, by day only those of events whose dates are not settled, every two hours. The
	// event pages come after it, because what they need depends on what it found: by day
	// only the pages it has news for.
	crawlStage("event-list", false, catalogdb.SourceQISEventEntry, func() (crawl.Stats, error) {
		all := s.inOffPeak(s.now()) || !s.archived(catalogdb.SourceQISEventEntry)
		return CrawlEventList(ctx, s.db, s.cfg.Endpoints, s.cfg.EventList, all)
	})
	crawlStage("events", false, catalogdb.SourceQISEvent, func() (crawl.Stats, error) {
		all := s.inOffPeak(s.now()) || !s.archived(catalogdb.SourceQISEvent)
		return CrawlEvents(ctx, s.db, s.cfg.Endpoints, s.cfg.Events, all)
	})
	if ctx.Err() != nil {
		result.Result = "interrupted"
		return result
	}

	// 2. Retention, 3. build, 4. validate and export. Any failure here keeps the old snapshot.
	step := func(name string, run func() error) bool {
		start := s.now()
		err := run()
		stage := StageResult{Name: name, DurationMS: s.now().Sub(start).Milliseconds()}
		if err != nil {
			stage.Error = err.Error()
			result.Result = "failed"
			log.Error("stage failed", "event", "stage.failed", "stage", name, oplog.Err(err))
		}
		result.Stages = append(result.Stages, stage)
		return err == nil
	}

	if s.cfg.EventRetention > 0 {
		step("retention", func() error {
			removed, err := s.db.PruneEvents(s.now(), s.cfg.EventRetention)
			if err == nil && removed > 0 {
				oplog.For("retention").Info("events pruned", "event", "retention.pruned", "removed", removed)
				prunedTotal.Add(float64(removed), "events")
			}
			return err
		})
	}

	var report *catalogbuild.Report
	if !step("build", func() (err error) { report, err = catalogbuild.Build(ctx, s.db); return err }) {
		return result
	}
	countBuild(report)

	if s.cfg.ArchiveGrace > 0 {
		step("archive", func() error {
			removed, err := s.db.PruneArchive(report.Unused, s.now().Add(-s.cfg.ArchiveGrace))
			if err == nil && removed > 0 {
				oplog.For("retention").Info("archive pruned", "event", "retention.archive_pruned", "removed", removed)
				prunedTotal.Add(float64(removed), "archive_pages")
			}
			return err
		})
	}

	// Last, after the snapshot of this build is out: the vectors of the semantic search for the
	// module texts this build brought, which the next build publishes. A defer, so it also runs
	// when there is nothing to export; before the deferred logging of the cycle above.
	defer func() {
		if ctx.Err() == nil && result.Result != "failed" {
			s.semanticStage(ctx, &result)
		}
	}()

	_, pointerErr := catalogdb.ReadSnapshotPointer(s.cfg.SnapshotDir)
	if !report.ContentChanged && pointerErr == nil {
		result.Stages = append(result.Stages, StageResult{Name: "export", Skipped: "content unchanged"})
		return result
	}

	ok := step("validate", func() error {
		checks, err := s.db.Validate(ctx, s.cfg.Baselines)
		if err != nil {
			return err
		}
		if failed := catalogdb.LogChecks(checks); failed > 0 {
			return fmt.Errorf("%d checks failed", failed)
		}
		return nil
	})
	if !ok {
		return result
	}
	result.Published = step("export", func() error { _, err := s.db.Export(ctx, s.cfg.SnapshotDir); return err })
	return result
}

func (s *Service) record(r CycleResult) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.cycles++
	s.last = &r
	switch r.Result {
	case "ok", "degraded":
		s.lastSuccessAt = r.FinishedAt
		s.failedInARow = 0
	case "failed":
		s.failedInARow++
	}
	countCycle(r, s.lastSuccessAt, s.failedInARow)
	s.archive.invalidate() // the cycle changed the archive
}

func (s *Service) inOffPeak(t time.Time) bool {
	start, end := s.cfg.OffPeakStart, s.cfg.OffPeakEnd
	if start == end {
		return true
	}
	h := t.Hour()
	if start < end {
		return h >= start && h < end
	}
	return h >= start || h < end
}

// archived reports whether a source has any page yet. A source without pages is
// crawled right away: a fresh installation should not stay empty until the night.
func (s *Service) archived(source string) bool {
	times, err := s.db.FetchTimes(source)
	return err == nil && len(times) > 0
}

// Status is the content of GET /status.
type Status struct {
	Healthy       bool                `json:"healthy"`
	Problems      []string            `json:"problems"` // why the service is not healthy
	StartedAt     time.Time           `json:"started_at"`
	Cycles        int                 `json:"cycles"`
	LastCycle     *CycleResult        `json:"last_cycle"`
	LastSuccessAt *time.Time          `json:"last_success_at"`
	FailedInARow  int                 `json:"failed_cycles_in_a_row"`
	NextCycleAt   *time.Time          `json:"next_cycle_at"`
	Snapshot      *catalogdb.Snapshot `json:"snapshot"`
	Log           *oplog.Summary      `json:"log,omitempty"`
}

// Status evaluates the health of the service. It is unhealthy when there is no
// snapshot to serve, when the last cycles failed, or when no cycle has succeeded
// for StaleAfter.
func (s *Service) Status() Status {
	s.mu.Lock()
	st := Status{StartedAt: s.startedAt, Cycles: s.cycles, LastCycle: s.last, FailedInARow: s.failedInARow, Problems: []string{}}
	if !s.lastSuccessAt.IsZero() {
		t := s.lastSuccessAt
		st.LastSuccessAt = &t
	}
	if !s.nextCycleAt.IsZero() {
		t := s.nextCycleAt
		st.NextCycleAt = &t
	}
	s.mu.Unlock()

	snap, err := catalogdb.ReadSnapshotPointer(s.cfg.SnapshotDir)
	if err != nil {
		st.Problems = append(st.Problems, "no snapshot has been published yet")
	} else {
		st.Snapshot = snap
	}
	if st.FailedInARow >= 2 {
		st.Problems = append(st.Problems, fmt.Sprintf("the last %d cycles failed", st.FailedInARow))
	}
	reference := st.StartedAt
	if st.LastSuccessAt != nil {
		reference = *st.LastSuccessAt
	}
	if s.cfg.StaleAfter > 0 && s.now().Sub(reference) > s.cfg.StaleAfter {
		st.Problems = append(st.Problems, fmt.Sprintf("no successful cycle since %s", reference.UTC().Format(time.RFC3339)))
	}
	st.Healthy = len(st.Problems) == 0

	if s.recorder != nil {
		sum := s.recorder.Summary(20)
		st.Log = &sum
	}
	return st
}

// Handler serves the snapshot endpoints plus:
//
//	GET /metrics  the counters and gauges in the Prometheus text format (docs/radix/operations.md)
//	GET /healthz  200 {"status":"ok"} or 503 {"status":"unhealthy","problems":[…]}; for
//	              container health checks and uptime monitors
//	GET /status   the full Status as JSON, including the most recent warnings and errors
func (s *Service) Handler() http.Handler {
	active.Store(s)
	mux := http.NewServeMux()
	mux.Handle("/snapshot/", snapshothttp.Handler(s.cfg.SnapshotDir))
	mux.Handle("GET /metrics", metrics.Default.Handler())

	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		st := s.Status()
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Cache-Control", "no-store")
		body := map[string]any{"status": "ok"}
		if !st.Healthy {
			w.WriteHeader(http.StatusServiceUnavailable)
			body = map[string]any{"status": "unhealthy", "problems": st.Problems}
		}
		_ = json.NewEncoder(w).Encode(body)
	})

	mux.HandleFunc("GET /status", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Cache-Control", "no-store")
		enc := json.NewEncoder(w)
		enc.SetIndent("", "  ")
		_ = enc.Encode(s.Status())
	})

	return snapshothttp.AccessLog(mux)
}

// ListenAndServe serves Handler until ctx is cancelled.
func (s *Service) ListenAndServe(ctx context.Context, addr string) error {
	server := &http.Server{Addr: addr, Handler: s.Handler(), ReadHeaderTimeout: 10 * time.Second}
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()
	oplog.For("http").Info("listening", "event", "http.listening", "addr", addr)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		oplog.For("http").Error("HTTP server stopped", "event", "http.failed", "addr", addr, oplog.Err(err))
		return err
	}
	return nil
}
