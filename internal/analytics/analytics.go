package analytics

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	_ "modernc.org/sqlite"
)

// EventType defines valid anonymous telemetry events.
type EventType string

const (
	EventPageView        EventType = "view"
	EventModuleClick     EventType = "module_click"
	EventProgramSelected EventType = "program_select"
)

// Event represents an anonymous telemetry action.
// Notice: Strictly DSGVO / GDPR compliant. Contains NO IP address, NO user identifier, NO session cookie.
type Event struct {
	Type        EventType `json:"type"`
	TargetID    string    `json:"target_id,omitempty"`   // e.g. module ID or program ID
	TargetName  string    `json:"target_name,omitempty"` // e.g. program title
	Timestamp   time.Time `json:"timestamp"`
}

// ModuleStat represents aggregate statistics for a module.
type ModuleStat struct {
	ModuleID    string    `json:"module_id"`
	Title       string    `json:"title,omitempty"`
	ClickCount  int       `json:"click_count"`
	LastClicked time.Time `json:"last_clicked"`
}

// ProgramStat represents aggregate statistics for a study program.
type ProgramStat struct {
	ProgramID     string    `json:"program_id"`
	ProgramName   string    `json:"program_name"`
	SelectCount   int       `json:"select_count"`
	LastSelected  time.Time `json:"last_selected"`
}

// DailyViewStat represents aggregated views for a single day.
type DailyViewStat struct {
	Date  string `json:"date"`  // YYYY-MM-DD
	Count int    `json:"count"`
}

// Summary aggregates high-level analytics metrics.
type Summary struct {
	TotalPageViews     int           `json:"total_page_views"`
	TodayPageViews     int           `json:"today_page_views"`
	TotalModuleClicks  int           `json:"total_module_clicks"`
	TotalProgramSelect int           `json:"total_program_selections"`
	TopModules         []ModuleStat  `json:"top_modules"`
	TopPrograms        []ProgramStat `json:"top_programs"`
	DailyViews         []DailyViewStat `json:"daily_views"`
}

// Tracker manages the separate analytics SQLite database and handles non-blocking event collection.
type Tracker struct {
	db        *sql.DB
	eventChan chan Event
	flushChan chan chan struct{}
	doneChan  chan struct{}
	wg        sync.WaitGroup
	closed    bool
	mu        sync.Mutex
}

// NewTracker opens or initializes the analytics database at dbPath.
func NewTracker(dbPath string) (*Tracker, error) {
	if dbPath == "" {
		dbPath = "btu_analytics.db"
	}

	if dir := filepath.Dir(dbPath); dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create analytics db dir: %w", err)
		}
	}

	dsn := fmt.Sprintf("%s?_pragma=busy_timeout(5000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)", dbPath)
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open analytics db: %w", err)
	}

	t := &Tracker{
		db:        db,
		eventChan: make(chan Event, 2048), // generous non-blocking buffer
		flushChan: make(chan chan struct{}),
		doneChan:  make(chan struct{}),
	}

	if err := t.migrate(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("failed to migrate analytics db: %w", err)
	}

	// Start asynchronous background writer
	t.wg.Add(1)
	go t.worker()

	return t, nil
}

func (t *Tracker) migrate() error {
	schema := `
	-- Aggregated daily page views (No individual IP or timestamp history)
	CREATE TABLE IF NOT EXISTS daily_page_views (
		date TEXT PRIMARY KEY,
		count INTEGER NOT NULL DEFAULT 0
	);

	-- Aggregated hourly traffic for system load analysis (No personal identifiers)
	CREATE TABLE IF NOT EXISTS hourly_traffic (
		date_hour TEXT PRIMARY KEY,
		count INTEGER NOT NULL DEFAULT 0
	);

	-- Aggregated module click counters
	CREATE TABLE IF NOT EXISTS module_clicks (
		module_id TEXT PRIMARY KEY,
		count INTEGER NOT NULL DEFAULT 0,
		last_clicked DATETIME
	);
	CREATE INDEX IF NOT EXISTS idx_modclicks_count ON module_clicks(count DESC);

	-- Aggregated study program selection counters
	CREATE TABLE IF NOT EXISTS program_selections (
		program_id TEXT PRIMARY KEY,
		program_name TEXT NOT NULL,
		count INTEGER NOT NULL DEFAULT 0,
		last_selected DATETIME
	);
	CREATE INDEX IF NOT EXISTS idx_progselect_count ON program_selections(count DESC);
	`
	_, err := t.db.Exec(schema)
	return err
}

func (t *Tracker) worker() {
	defer t.wg.Done()

	// Batch events to minimize SQLite transactions
	batchTicker := time.NewTicker(2 * time.Second)
	defer batchTicker.Stop()

	var batch []Event

	for {
		select {
		case <-t.doneChan:
			// Drain remaining events before exiting
			for {
				select {
				case evt := <-t.eventChan:
					batch = append(batch, evt)
				default:
					t.flushBatch(batch)
					return
				}
			}
		case resp := <-t.flushChan:
			// Drain all pending in eventChan
			drain := true
			for drain {
				select {
				case evt := <-t.eventChan:
					batch = append(batch, evt)
				default:
					drain = false
				}
			}
			t.flushBatch(batch)
			batch = nil
			close(resp)
		case evt := <-t.eventChan:
			batch = append(batch, evt)
			if len(batch) >= 50 {
				t.flushBatch(batch)
				batch = nil
			}
		case <-batchTicker.C:
			if len(batch) > 0 {
				t.flushBatch(batch)
				batch = nil
			}
		}
	}
}

func (t *Tracker) flushBatch(batch []Event) {
	if len(batch) == 0 {
		return
	}

	tx, err := t.db.Begin()
	if err != nil {
		return
	}
	defer func() { _ = tx.Rollback() }()

	stmtPageView, err := tx.Prepare(`
		INSERT INTO daily_page_views (date, count) VALUES (?, 1)
		ON CONFLICT(date) DO UPDATE SET count = count + 1;
	`)
	if err != nil {
		return
	}
	defer stmtPageView.Close()

	stmtHourly, err := tx.Prepare(`
		INSERT INTO hourly_traffic (date_hour, count) VALUES (?, 1)
		ON CONFLICT(date_hour) DO UPDATE SET count = count + 1;
	`)
	if err != nil {
		return
	}
	defer stmtHourly.Close()

	stmtModule, err := tx.Prepare(`
		INSERT INTO module_clicks (module_id, count, last_clicked) VALUES (?, 1, ?)
		ON CONFLICT(module_id) DO UPDATE SET count = count + 1, last_clicked = excluded.last_clicked;
	`)
	if err != nil {
		return
	}
	defer stmtModule.Close()

	stmtProg, err := tx.Prepare(`
		INSERT INTO program_selections (program_id, program_name, count, last_selected) VALUES (?, ?, 1, ?)
		ON CONFLICT(program_id) DO UPDATE SET count = count + 1, program_name = excluded.program_name, last_selected = excluded.last_selected;
	`)
	if err != nil {
		return
	}
	defer stmtProg.Close()

	for _, evt := range batch {
		ts := evt.Timestamp
		if ts.IsZero() {
			ts = time.Now()
		}
		dateStr := ts.Format("2006-01-02")
		hourStr := ts.Format("2006-01-02 15")
		timeStr := ts.Format("2006-01-02 15:04:05")

		switch evt.Type {
		case EventPageView:
			_, _ = stmtPageView.Exec(dateStr)
			_, _ = stmtHourly.Exec(hourStr)
		case EventModuleClick:
			if evt.TargetID != "" {
				_, _ = stmtModule.Exec(evt.TargetID, timeStr)
			}
		case EventProgramSelected:
			if evt.TargetID != "" || evt.TargetName != "" {
				progID := evt.TargetID
				if progID == "" {
					progID = evt.TargetName
				}
				name := evt.TargetName
				if name == "" {
					name = progID
				}
				_, _ = stmtProg.Exec(progID, name, timeStr)
			}
		}
	}

	_ = tx.Commit()
}

// Track registers an anonymous event in non-blocking fashion.
// If the buffer is full under extreme spikes, drops the event rather than blocking callers.
func (t *Tracker) Track(evt Event) {
	t.mu.Lock()
	if t.closed {
		t.mu.Unlock()
		return
	}
	t.mu.Unlock()

	if evt.Timestamp.IsZero() {
		evt.Timestamp = time.Now()
	}

	select {
	case t.eventChan <- evt:
	default:
		// Queue full - gracefully drop to avoid degrading HTTP server responsiveness
	}
}

// TrackPageView tracks an anonymous page view.
func (t *Tracker) TrackPageView() {
	t.Track(Event{Type: EventPageView})
}

// TrackModuleClick tracks a user viewing a module card or opening the module modal.
func (t *Tracker) TrackModuleClick(moduleID string) {
	if moduleID != "" {
		t.Track(Event{
			Type:     EventModuleClick,
			TargetID: moduleID,
		})
	}
}

// TrackProgramSelect tracks a user selecting a study program filter.
func (t *Tracker) TrackProgramSelect(programID, programName string) {
	t.Track(Event{
		Type:       EventProgramSelected,
		TargetID:   programID,
		TargetName: programName,
	})
}

// GetSummary compiles high-level aggregated metrics.
func (t *Tracker) GetSummary() (Summary, error) {
	var s Summary

	// 1. Total page views
	_ = t.db.QueryRow(`SELECT COALESCE(SUM(count), 0) FROM daily_page_views`).Scan(&s.TotalPageViews)

	// 2. Today's page views
	today := time.Now().Format("2006-01-02")
	_ = t.db.QueryRow(`SELECT COALESCE(count, 0) FROM daily_page_views WHERE date = ?`, today).Scan(&s.TodayPageViews)

	// 3. Total module clicks
	_ = t.db.QueryRow(`SELECT COALESCE(SUM(count), 0) FROM module_clicks`).Scan(&s.TotalModuleClicks)

	// 4. Total program selections
	_ = t.db.QueryRow(`SELECT COALESCE(SUM(count), 0) FROM program_selections`).Scan(&s.TotalProgramSelect)

	// 5. Top 10 modules
	topMods, _ := t.GetTopModules(10)
	s.TopModules = topMods

	// 6. Top 10 programs
	topProgs, _ := t.GetTopPrograms(10)
	s.TopPrograms = topProgs

	// 7. Last 14 days of views
	daily, _ := t.GetDailyViews(14)
	s.DailyViews = daily

	return s, nil
}

// GetTopModules returns the most clicked modules.
func (t *Tracker) GetTopModules(limit int) ([]ModuleStat, error) {
	if limit <= 0 {
		limit = 10
	}
	rows, err := t.db.Query(`
		SELECT module_id, count, COALESCE(last_clicked, CURRENT_TIMESTAMP)
		FROM module_clicks
		ORDER BY count DESC
		LIMIT ?
	`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []ModuleStat
	for rows.Next() {
		var m ModuleStat
		var lastClickedStr sql.NullString
		if err := rows.Scan(&m.ModuleID, &m.ClickCount, &lastClickedStr); err == nil {
			if lastClickedStr.Valid && lastClickedStr.String != "" {
				if parsed, err := time.Parse("2006-01-02 15:04:05", lastClickedStr.String); err == nil {
					m.LastClicked = parsed
				} else if parsed, err := time.Parse(time.RFC3339, lastClickedStr.String); err == nil {
					m.LastClicked = parsed
				}
			}
			result = append(result, m)
		}
	}
	return result, nil
}

// GetTopPrograms returns the most selected study programs.
func (t *Tracker) GetTopPrograms(limit int) ([]ProgramStat, error) {
	if limit <= 0 {
		limit = 10
	}
	rows, err := t.db.Query(`
		SELECT program_id, program_name, count, COALESCE(last_selected, CURRENT_TIMESTAMP)
		FROM program_selections
		ORDER BY count DESC
		LIMIT ?
	`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []ProgramStat
	for rows.Next() {
		var p ProgramStat
		var lastSelectedStr sql.NullString
		if err := rows.Scan(&p.ProgramID, &p.ProgramName, &p.SelectCount, &lastSelectedStr); err == nil {
			if lastSelectedStr.Valid && lastSelectedStr.String != "" {
				if parsed, err := time.Parse("2006-01-02 15:04:05", lastSelectedStr.String); err == nil {
					p.LastSelected = parsed
				} else if parsed, err := time.Parse(time.RFC3339, lastSelectedStr.String); err == nil {
					p.LastSelected = parsed
				}
			}
			result = append(result, p)
		}
	}
	return result, nil
}

// GetDailyViews returns views grouped by day for the last N days.
func (t *Tracker) GetDailyViews(days int) ([]DailyViewStat, error) {
	if days <= 0 {
		days = 14
	}
	rows, err := t.db.Query(`
		SELECT date, count
		FROM daily_page_views
		ORDER BY date DESC
		LIMIT ?
	`, days)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []DailyViewStat
	for rows.Next() {
		var d DailyViewStat
		if err := rows.Scan(&d.Date, &d.Count); err == nil {
			result = append(result, d)
		}
	}
	return result, nil
}

// Flush forces pending events in memory to write immediately to disk (used for testing or graceful shutdown).
// Flush forces pending events in memory to write immediately to disk (used for testing or graceful shutdown).
func (t *Tracker) Flush() {
	t.mu.Lock()
	if t.closed {
		t.mu.Unlock()
		return
	}
	t.mu.Unlock()

	resp := make(chan struct{})
	select {
	case t.flushChan <- resp:
		<-resp
	case <-time.After(3 * time.Second):
		// Timeout safety
	}
}

// Close gracefully stops the worker and closes the analytics database.
func (t *Tracker) Close() error {
	t.mu.Lock()
	if t.closed {
		t.mu.Unlock()
		return nil
	}
	t.closed = true
	t.mu.Unlock()

	close(t.doneChan)
	t.wg.Wait()
	return t.db.Close()
}
