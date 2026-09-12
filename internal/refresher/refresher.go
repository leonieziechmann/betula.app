package refresher

import (
	"context"
	"errors"
	"fmt"
	"math/rand"
	"sync"
	"sync/atomic"
	"time"

	"github.com/jakob/btu-scraper/internal/logger"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/provider"
	"github.com/jakob/btu-scraper/internal/storage"
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
		ModuleDetailDelay:     1500 * time.Millisecond,
		QISDelay:              4500 * time.Millisecond,
		CatalogInterval:       12 * time.Hour,
		OffPeakOnlyBackground: true,
	}
}

// Status represents the current state of the refresher service.
type Status struct {
	State              string    `json:"state"` // "idle", "off_peak_refresh", "catalog_scan", "backing_off"
	IsOffPeak          bool      `json:"is_off_peak"`
	PriorityQueueLen   int       `json:"priority_queue_len"`
	StandardQueueLen   int       `json:"standard_queue_len"`
	TotalDiscovered    uint64    `json:"total_discovered"`
	TotalRefreshed     uint64    `json:"total_refreshed"`
	TotalErrors        uint64    `json:"total_errors"`
	LastCatalogScan    time.Time `json:"last_catalog_scan"`
	LastRefresh        time.Time `json:"last_refresh"`
	BackoffUntil       time.Time `json:"backoff_until,omitempty"`
}

// Refresher coordinates hierarchical, polite, off-peak scheduled rescraping.
type Refresher struct {
	config       Config
	store        *storage.Storage
	catProvider  *provider.BTUModuleCatalogProvider
	detProvider  *provider.BTUModuleDetailProvider
	evtProvider  *provider.BTUEventProvider
	log          *logger.Logger
	priorityChan chan string // Module IDs with urgent / cache-miss priority
	wakeChan     chan struct{}

	mu               sync.RWMutex
	state            string
	backoffUntil     time.Time
	lastCatalogScan  time.Time
	lastRefresh      time.Time
	totalDiscovered  uint64
	totalRefreshed   uint64
	totalErrors      uint64
	queuedPriority   map[string]bool
	queuedPriorityMu sync.Mutex
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
		wakeChan:       make(chan struct{}, 1),
		state:          "idle",
		queuedPriority: make(map[string]bool),
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

	return r.store.GetModule(moduleID)
}

// Start runs the background scheduling loop until context is canceled.
func (r *Refresher) Start(ctx context.Context) {
	r.log.Info("REFRESHER", "Starting polite data refreshing service (Off-Peak: %02d:00-%02d:00, Module Delay: %v, QIS Delay: %v)",
		r.config.OffPeakStartHour, r.config.OffPeakEndHour, r.config.ModuleDetailDelay, r.config.QISDelay)

	catalogTicker := time.NewTicker(r.config.CatalogInterval)
	defer catalogTicker.Stop()

	// Initial catalog discovery check in background
	go func() {
		time.Sleep(3 * time.Second)
		r.runCatalogDiscovery(ctx)
	}()

	workerTicker := time.NewTicker(30 * time.Second)
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

	// If background refresh is off-peak only, verify current time
	if r.config.OffPeakOnlyBackground && !r.IsOffPeak() {
		r.setState("idle (daytime standby)")
		return
	}

	r.setState("off_peak_refresh")
	r.runSlowModuleBatch(ctx, 10)
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
	}

	// Polite jitter delay even on priority jobs
	r.sleepWithJitter(ctx, r.config.ModuleDetailDelay)
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

		err := r.detProvider.ScrapeModule(ctx, id, true)
		if err != nil {
			r.recordError("MODULE_DETAIL", id, err)
		} else {
			r.recordSuccess()
			atomic.AddUint64(&r.totalRefreshed, 1)
			r.mu.Lock()
			r.lastRefresh = time.Now()
			r.mu.Unlock()
		}

		// Strictly polite jittered sleep
		r.sleepWithJitter(ctx, r.config.ModuleDetailDelay)
	}
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
		r.triggerBackoff(5 * time.Minute)
	}
}

func (r *Refresher) recordSuccess() {
	r.log.RecordScrapeSuccess()
}

func (r *Refresher) triggerBackoff(dur time.Duration) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.backoffUntil = time.Now().Add(dur)
	r.state = fmt.Sprintf("backing_off (until %s)", r.backoffUntil.Format("15:04:05"))
	r.log.Warn("REFRESHER", "Triggered scraper backoff for %v due to university server response", dur)
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

// GetStatus returns the current live state of the refresher service.
func (r *Refresher) GetStatus() Status {
	r.mu.RLock()
	defer r.mu.RUnlock()

	return Status{
		State:            r.state,
		IsOffPeak:        r.IsOffPeak(),
		PriorityQueueLen: len(r.priorityChan),
		TotalDiscovered:  atomic.LoadUint64(&r.totalDiscovered),
		TotalRefreshed:   atomic.LoadUint64(&r.totalRefreshed),
		TotalErrors:      atomic.LoadUint64(&r.totalErrors),
		LastCatalogScan:  r.lastCatalogScan,
		LastRefresh:      r.lastRefresh,
		BackoffUntil:     r.backoffUntil,
	}
}

func isRateLimitOrServerDown(err string) bool {
	// Checks for 429 Too Many Requests, 503 Service Unavailable, 502, connection refused
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
