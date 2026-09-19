package refresher

import (
	"context"
	"errors"
	"fmt"
	"math/rand"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/logger"
	"github.com/leonieziechmann/btu-scraper/internal/model"
	"github.com/leonieziechmann/btu-scraper/internal/provider"
	"github.com/leonieziechmann/btu-scraper/internal/storage"
)

// Config holds configuration for the polite background refreshing service.
type Config struct {
	// Off-peak hours in 24h format (e.g. 1 and 6 means 01:00 to 06:00 local time)
	OffPeakStartHour int
	OffPeakEndHour   int

	// Polite delays
	ModuleDetailDelay time.Duration // e.g. 1500ms
	QISDelay          time.Duration // e.g. 4500ms
	CatalogInterval   time.Duration // e.g. 12 * time.Hour

	// Day-time behavior: if true, low-priority background sweeps only run during off-peak hours
	OffPeakOnlyBackground bool
}

// DefaultConfig provides conservative, university-friendly defaults.
func DefaultConfig() Config {
	return Config{
		OffPeakStartHour:      1, // 01:00
		OffPeakEndHour:        6, // 06:00
		ModuleDetailDelay:     500 * time.Millisecond,
		QISDelay:              500 * time.Millisecond,
		CatalogInterval:       12 * time.Hour,
		OffPeakOnlyBackground: true,
	}
}

// EventScrapeJob represents an item in the QIS event scraping queue.
type EventScrapeJob struct {
	EventID  string `json:"event_id"`
	PageURL  string `json:"page_url"`
	ModuleID string `json:"module_id"`
	Title    string `json:"title"`
}

// Status represents the current state of the refresher service.
type Status struct {
	State              string                 `json:"state"` // "idle", "off_peak_refresh", "qis_event_scrape", "catalog_scan", "backing_off"
	ActiveJob          string                 `json:"active_job"`
	IsOffPeak          bool                   `json:"is_off_peak"`
	PriorityQueueLen   int                    `json:"priority_queue_len"`
	EventQueueLen      int                    `json:"event_queue_len"`
	StandardQueueLen   int                    `json:"standard_queue_len"`
	TotalDiscovered    uint64                 `json:"total_discovered"`
	TotalRefreshed     uint64                 `json:"total_refreshed"`
	TotalEventsScraped uint64                 `json:"total_events_scraped"`
	TotalErrors        uint64                 `json:"total_errors"`
	LastCatalogScan    time.Time              `json:"last_catalog_scan"`
	LastRefresh        time.Time              `json:"last_refresh"`
	BackoffUntil       time.Time              `json:"backoff_until,omitempty"`
	Freshness          storage.FreshnessStats `json:"freshness"`
}

// Refresher coordinates hierarchical, polite, off-peak scheduled rescraping.
type Refresher struct {
	config       Config
	store        *storage.Storage
	catProvider  *provider.BTUModuleCatalogProvider
	detProvider  *provider.BTUModuleDetailProvider
	evtProvider  *provider.BTUEventProvider
	log          *logger.Logger
	priorityChan chan string         // Module IDs with urgent / cache-miss priority
	eventQueue   chan EventScrapeJob // QIS events queue for sequential gentle crawl
	wakeChan     chan struct{}

	mu                 sync.RWMutex
	state              string
	activeJob          string
	backoffUntil       time.Time
	lastCatalogScan    time.Time
	lastRefresh        time.Time
	totalDiscovered    uint64
	totalRefreshed     uint64
	totalEventsScraped uint64
	totalErrors        uint64
	consecutiveErrors  int
	queuedPriority     map[string]bool
	queuedPriorityMu   sync.Mutex
	queuedEvents       map[string]bool
	queuedEventsMu     sync.Mutex
}

// NewRefresher creates a new refresher service.
func NewRefresher(
	cfg Config,
	store *storage.Storage,
	catProvider *provider.BTUModuleCatalogProvider,
	detProvider *provider.BTUModuleDetailProvider,
	evtProvider *provider.BTUEventProvider,
	log *logger.Logger,
) *Refresher {
	if log == nil {
		log = logger.Default()
	}

	return &Refresher{
		config:         cfg,
		store:          store,
		catProvider:    catProvider,
		detProvider:    detProvider,
		evtProvider:    evtProvider,
		log:            log,
		priorityChan:   make(chan string, 1000),
		eventQueue:     make(chan EventScrapeJob, 10000),
		wakeChan:       make(chan struct{}, 1),
		state:          "idle",
		queuedPriority: make(map[string]bool),
		queuedEvents:   make(map[string]bool),
	}
}

// IsOffPeak returns true if the current local time is within off-peak hours.
func (r *Refresher) IsOffPeak() bool {
	now := time.Now()
	hour := now.Hour()
	if r.config.OffPeakStartHour <= r.config.OffPeakEndHour {
		return hour >= r.config.OffPeakStartHour && hour < r.config.OffPeakEndHour
	}
	// Handles overnight windows (e.g. 22:00 to 05:00)
	return hour >= r.config.OffPeakStartHour || hour < r.config.OffPeakEndHour
}

// EnqueueCacheMiss adds a module ID to the high-priority queue because of a cache miss or direct user request.
func (r *Refresher) EnqueueCacheMiss(moduleID string) {
	if moduleID == "" {
		return
	}
	r.queuedPriorityMu.Lock()
	if r.queuedPriority[moduleID] {
		r.queuedPriorityMu.Unlock()
		return
	}
	r.queuedPriority[moduleID] = true
	r.queuedPriorityMu.Unlock()

	select {
	case r.priorityChan <- moduleID:
		r.log.Debug("REFRESHER", "Enqueued cache-miss priority module %s", moduleID)
		r.wake()
	default:
		r.log.Warn("REFRESHER", "Priority queue full, dropped module %s", moduleID)
	}
}

// EnqueueEvent adds a QIS event to the event scraping queue.
func (r *Refresher) EnqueueEvent(job EventScrapeJob) {
	if job.EventID == "" && job.PageURL == "" {
		return
	}
	key := job.EventID
	if key == "" {
		key = job.PageURL
	}

	r.queuedEventsMu.Lock()
	if r.queuedEvents[key] {
		r.queuedEventsMu.Unlock()
		return
	}
	r.queuedEvents[key] = true
	r.queuedEventsMu.Unlock()

	select {
	case r.eventQueue <- job:
		r.log.Debug("REFRESHER", "Enqueued QIS event scrape job: %s (%s)", job.EventID, job.Title)
		r.wake()
	default:
		r.log.Warn("REFRESHER", "Event queue full, dropped event %s", job.EventID)
	}
}

// SyncDiscoveredEvents scans all modules in SQLite for discovered QIS events and queues those not yet scraped.
func (r *Refresher) SyncDiscoveredEvents(ctx context.Context) {
	if r.store == nil {
		return
	}

	discovered, err := r.store.GetDiscoveredEventRefs()
	if err != nil {
		r.log.Warn("REFRESHER", "Failed to retrieve discovered events from storage: %v", err)
		return
	}

	enqueued := 0
	for _, d := range discovered {
		if !d.IsScraped {
			r.EnqueueEvent(EventScrapeJob{
				EventID:  d.EventID,
				PageURL:  d.URL,
				ModuleID: d.ModuleID,
				Title:    d.Title,
			})
			enqueued++
		}
	}

	r.log.Info("REFRESHER", "Discovered %d total events referenced across modules (%d unscraped queued for gentle QIS crawl)",
		len(discovered), enqueued)
}

func (r *Refresher) wake() {
	select {
	case r.wakeChan <- struct{}{}:
	default:
	}
}

// FetchModuleNow performs an on-demand synchronous scrape for a module if not in cache or missing details.
func (r *Refresher) FetchModuleNow(ctx context.Context, moduleID string) (*model.ModuleDetail, error) {
	if r.detProvider == nil {
		return nil, errors.New("detail provider not configured")
	}

	r.log.Info("REFRESHER", "On-demand cache miss fetch triggered for module %s", moduleID)
	err := r.detProvider.ScrapeModule(ctx, moduleID, true)
	if err != nil {
		r.recordError("MODULE_ONDEMAND", moduleID, err)
		return nil, err
	}

	r.recordSuccess()
	atomic.AddUint64(&r.totalRefreshed, 1)

	detail, err := r.store.GetModule(moduleID)
	if err == nil && detail != nil {
		// Automatically enqueue any newly discovered events
		for _, me := range detail.CurrentSemesterEvents {
			eid := provider.ExtractIDFromURL(me.URL)
			r.EnqueueEvent(EventScrapeJob{
				EventID:  eid,
				PageURL:  me.URL,
				ModuleID: moduleID,
				Title:    me.Title,
			})
		}
	}

	return detail, err
}

// Start runs the background scheduling loop until context is canceled.
func (r *Refresher) Start(ctx context.Context) {
	r.log.Info("REFRESHER", "Starting polite data refreshing service (Off-Peak: %02d:00-%02d:00, Module Delay: %v, QIS Delay: %v)",
		r.config.OffPeakStartHour, r.config.OffPeakEndHour, r.config.ModuleDetailDelay, r.config.QISDelay)

	catalogTicker := time.NewTicker(r.config.CatalogInterval)
	defer catalogTicker.Stop()

	// 1. Initial discovery pass in background
	go func() {
		time.Sleep(2 * time.Second)
		r.SyncDiscoveredEvents(ctx)
		time.Sleep(3 * time.Second)
		r.runCatalogDiscovery(ctx)
	}()

	workerTicker := time.NewTicker(15 * time.Second)
	defer workerTicker.Stop()

	for {
		select {
		case <-ctx.Done():
			r.setState("stopped")
			r.log.Info("REFRESHER", "Refresher service gracefully stopped")
			return

		case modID := <-r.priorityChan:
			r.queuedPriorityMu.Lock()
			delete(r.queuedPriority, modID)
			r.queuedPriorityMu.Unlock()

			r.processPriorityModule(ctx, modID)

		case evtJob := <-r.eventQueue:
			r.queuedEventsMu.Lock()
			k := evtJob.EventID
			if k == "" {
				k = evtJob.PageURL
			}
			delete(r.queuedEvents, k)
			r.queuedEventsMu.Unlock()

			r.processEventJob(ctx, evtJob)

		case <-catalogTicker.C:
			r.runCatalogDiscovery(ctx)

		case <-workerTicker.C:
			r.checkAndRunScheduled(ctx)

		case <-r.wakeChan:
			// Woken up by new queue item
		}
	}
}

func (r *Refresher) checkAndRunScheduled(ctx context.Context) {
	if r.isBackingOff() {
		return
	}

	// Priority queue or event queue takes precedence
	if len(r.priorityChan) > 0 || len(r.eventQueue) > 0 {
		return
	}

	// If background module refresh is off-peak only, verify current time
	if r.config.OffPeakOnlyBackground && !r.IsOffPeak() {
		r.setState("idle (daytime standby)")
		return
	}

	r.setState("off_peak_refresh")
	r.runSlowModuleBatch(ctx, 5)
}

// runCatalogDiscovery scans b-tu.de/modul to discover newly added modules and enqueues them.
func (r *Refresher) runCatalogDiscovery(ctx context.Context) {
	if r.catProvider == nil || r.isBackingOff() {
		return
	}

	r.setState("catalog_scan")
	r.log.Info("REFRESHER", "Scanning module catalog (capable server) to discover new or updated modules...")

	// 1. Get existing module IDs from local DB
	existingSummaries, _ := r.store.ListModules(storage.Filter{Limit: 10000})
	knownIDs := make(map[string]bool)
	for _, s := range existingSummaries {
		knownIDs[s.ID] = true
	}

	// 2. Discover modules from b-tu.de/modul
	count, err := r.catProvider.ScrapeCatalog(ctx, true)
	if err != nil {
		r.recordError("CATALOG_DISCOVERY", "b-tu.de/modul", err)
		return
	}
	r.recordSuccess()

	r.mu.Lock()
	r.lastCatalogScan = time.Now()
	r.mu.Unlock()

	// 3. Compare with newly saved modules and automatically enqueue any brand-new modules with high priority
	updatedSummaries, _ := r.store.ListModules(storage.Filter{Limit: 10000})
	newlyFound := 0
	for _, s := range updatedSummaries {
		if !knownIDs[s.ID] {
			newlyFound++
			r.EnqueueCacheMiss(s.ID)
		}
	}

	atomic.AddUint64(&r.totalDiscovered, uint64(count))
	r.log.Info("REFRESHER", "Catalog scan complete. %d total modules in catalog (%d newly discovered and enqueued).", count, newlyFound)
	r.setState("idle")
}

func (r *Refresher) processPriorityModule(ctx context.Context, moduleID string) {
	if r.detProvider == nil {
		return
	}

	r.setActiveJob(fmt.Sprintf("Modul %s (High-Prio)", moduleID))
	r.setState("priority_refresh")
	r.log.Info("REFRESHER", "Processing priority module scrape: %s", moduleID)

	err := r.detProvider.ScrapeModule(ctx, moduleID, true)
	if err != nil {
		r.recordError("MODULE_DETAIL", moduleID, err)
	} else {
		r.recordSuccess()
		atomic.AddUint64(&r.totalRefreshed, 1)
		r.mu.Lock()
		r.lastRefresh = time.Now()
		r.mu.Unlock()

		// Automatically discover and enqueue any linked QIS semester events
		if detail, err := r.store.GetModule(moduleID); err == nil && detail != nil {
			for _, me := range detail.CurrentSemesterEvents {
				eid := provider.ExtractIDFromURL(me.URL)
				r.EnqueueEvent(EventScrapeJob{
					EventID:  eid,
					PageURL:  me.URL,
					ModuleID: moduleID,
					Title:    me.Title,
				})
			}
		}
	}

	// Polite jitter delay even on priority jobs
	r.sleepWithJitter(ctx, r.config.ModuleDetailDelay)
	r.setActiveJob("")
	r.setState("idle")
}

func (r *Refresher) processEventJob(ctx context.Context, job EventScrapeJob) {
	if r.evtProvider == nil || r.isBackingOff() {
		return
	}

	desc := job.Title
	if desc == "" {
		desc = job.EventID
	}
	r.setActiveJob(fmt.Sprintf("QIS Event %s: %s", job.EventID, desc))
	r.setState("qis_event_scrape")
	r.log.Info("REFRESHER", "Gently scraping QIS event %s (%s)...", job.EventID, desc)

	eventDetail, err := r.evtProvider.ScrapeEvent(ctx, job.EventID, job.PageURL, false)
	if err != nil {
		r.recordError("QIS_EVENT", job.EventID, err)
	} else {
		r.recordSuccess()
		atomic.AddUint64(&r.totalEventsScraped, 1)
		if job.ModuleID != "" && eventDetail != nil {
			_ = r.store.LinkModuleEvent(job.ModuleID, eventDetail.ID)
		}
		r.mu.Lock()
		r.lastRefresh = time.Now()
		r.mu.Unlock()
	}

	// Strictly gentle delay for fragile QIS portal
	r.sleepWithJitter(ctx, r.config.QISDelay)
	r.setActiveJob("")
	r.setState("idle")
}

func (r *Refresher) runSlowModuleBatch(ctx context.Context, batchSize int) {
	if r.detProvider == nil {
		return
	}

	// Query modules with oldest last_scraped_at
	rows, err := r.store.DB().QueryContext(ctx, `
		SELECT id FROM modules 
		ORDER BY last_scraped_at ASC NULLS FIRST, id ASC 
		LIMIT ?
	`, batchSize)
	if err != nil {
		r.log.Error("REFRESHER", "Failed to query stale modules: %v", err)
		return
	}
	defer rows.Close()

	var moduleIDs []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err == nil {
			moduleIDs = append(moduleIDs, id)
		}
	}

	if len(moduleIDs) == 0 {
		return
	}

	r.log.Info("REFRESHER", "Rescraping batch of %d modules gently (polite delay: %v)...", len(moduleIDs), r.config.ModuleDetailDelay)

	for _, id := range moduleIDs {
		select {
		case <-ctx.Done():
			return
		default:
		}

		if r.isBackingOff() {
			return
		}

		// Hierarchy check: If any urgent priority item is waiting, pause batch and yield
		select {
		case prioID := <-r.priorityChan:
			r.processPriorityModule(ctx, prioID)
		default:
		}

		r.setActiveJob(fmt.Sprintf("Modul %s (Slow Refresh)", id))
		err := r.detProvider.ScrapeModule(ctx, id, true)
		if err != nil {
			r.recordError("MODULE_DETAIL", id, err)
		} else {
			r.recordSuccess()
			atomic.AddUint64(&r.totalRefreshed, 1)
			r.mu.Lock()
			r.lastRefresh = time.Now()
			r.mu.Unlock()

			// Enqueue any discovered events on this module
			if detail, err := r.store.GetModule(id); err == nil && detail != nil {
				for _, me := range detail.CurrentSemesterEvents {
					eid := provider.ExtractIDFromURL(me.URL)
					r.EnqueueEvent(EventScrapeJob{
						EventID:  eid,
						PageURL:  me.URL,
						ModuleID: id,
						Title:    me.Title,
					})
				}
			}
		}

		// Strictly polite jittered sleep
		r.sleepWithJitter(ctx, r.config.ModuleDetailDelay)
	}

	r.setActiveJob("")
}

func (r *Refresher) sleepWithJitter(ctx context.Context, baseDelay time.Duration) {
	if baseDelay <= 0 {
		return
	}
	// Add ±30% random jitter to avoid repetitive bot fingerprints
	jitterRange := int64(baseDelay / 3)
	var jitter int64
	if jitterRange > 0 {
		jitter = rand.Int63n(jitterRange*2) - jitterRange
	}
	sleepDur := time.Duration(int64(baseDelay) + jitter)
	if sleepDur < 200*time.Millisecond {
		sleepDur = 200 * time.Millisecond
	}

	select {
	case <-ctx.Done():
	case <-time.After(sleepDur):
	}
}

func (r *Refresher) recordError(component, target string, err error) {
	atomic.AddUint64(&r.totalErrors, 1)
	r.log.RecordScrapeFailure(component, target, err)

	// Trigger exponential backoff on connection or rate-limiting errors
	errStr := err.Error()
	if isRateLimitOrServerDown(errStr) {
		r.mu.Lock()
		r.consecutiveErrors++
		errCount := r.consecutiveErrors
		r.mu.Unlock()

		backoffDur := computeExponentialBackoff(errCount)
		r.triggerBackoff(backoffDur)
	}
}

func (r *Refresher) recordSuccess() {
	r.mu.Lock()
	r.consecutiveErrors = 0
	r.mu.Unlock()
	r.log.RecordScrapeSuccess()
}

func computeExponentialBackoff(errCount int) time.Duration {
	switch {
	case errCount <= 1:
		return 5 * time.Second
	case errCount == 2:
		return 15 * time.Second
	case errCount == 3:
		return 30 * time.Second
	case errCount == 4:
		return 1 * time.Minute
	case errCount == 5:
		return 2 * time.Minute
	case errCount == 6:
		return 5 * time.Minute
	case errCount == 7:
		return 15 * time.Minute
	case errCount == 8:
		return 30 * time.Minute
	default:
		return 60 * time.Minute // Max 1 hour exponential backoff
	}
}

func (r *Refresher) triggerBackoff(dur time.Duration) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.backoffUntil = time.Now().Add(dur)
	r.state = fmt.Sprintf("backing_off (until %s)", r.backoffUntil.Format("15:04:05"))
	r.log.Warn("REFRESHER", "Triggered scraper exponential backoff for %v (error streak: %d) due to university server response", dur, r.consecutiveErrors)
}

func (r *Refresher) isBackingOff() bool {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return time.Now().Before(r.backoffUntil)
}

func (r *Refresher) setState(s string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.state = s
}

func (r *Refresher) setActiveJob(job string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.activeJob = job
}

// GetStatus returns the current live state of the refresher service including data freshness stats.
func (r *Refresher) GetStatus() Status {
	r.mu.RLock()
	state := r.state
	active := r.activeJob
	lastCat := r.lastCatalogScan
	lastRef := r.lastRefresh
	backoff := r.backoffUntil
	r.mu.RUnlock()

	var fresh storage.FreshnessStats
	if r.store != nil {
		fresh, _ = r.store.GetFreshnessStats()
	}

	return Status{
		State:              state,
		ActiveJob:          active,
		IsOffPeak:          r.IsOffPeak(),
		PriorityQueueLen:   len(r.priorityChan),
		EventQueueLen:      len(r.eventQueue),
		TotalDiscovered:    atomic.LoadUint64(&r.totalDiscovered),
		TotalRefreshed:     atomic.LoadUint64(&r.totalRefreshed),
		TotalEventsScraped: atomic.LoadUint64(&r.totalEventsScraped),
		TotalErrors:        atomic.LoadUint64(&r.totalErrors),
		LastCatalogScan:    lastCat,
		LastRefresh:        lastRef,
		BackoffUntil:       backoff,
		Freshness:          fresh,
	}
}

func isRateLimitOrServerDown(err string) bool {
	return len(err) > 0 && (contains(err, "429") || contains(err, "503") || contains(err, "502") || contains(err, "connection refused") || contains(err, "timeout"))
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) > 0 && searchSubstring(s, substr))
}

func searchSubstring(s, substr string) bool {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
