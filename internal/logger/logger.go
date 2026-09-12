package logger

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"time"
)

// Level represents log severity.
type Level int

const (
	LevelDebug Level = iota
	LevelInfo
	LevelWarn
	LevelError
)

func (l Level) String() string {
	switch l {
	case LevelDebug:
		return "DEBUG"
	case LevelInfo:
		return "INFO"
	case LevelWarn:
		return "WARN"
	case LevelError:
		return "ERROR"
	default:
		return "UNKNOWN"
	}
}

// LogEntry represents an individual structured log message.
type LogEntry struct {
	Timestamp time.Time `json:"timestamp"`
	Level     string    `json:"level"`
	Component string    `json:"component"`
	Message   string    `json:"message"`
}

// RingBuffer stores a fixed number of recent log entries in memory.
type RingBuffer struct {
	mu      sync.RWMutex
	entries []LogEntry
	maxSize int
	head    int
	isFull  bool
}

// NewRingBuffer creates a new circular buffer with the specified capacity.
func NewRingBuffer(maxSize int) *RingBuffer {
	if maxSize <= 0 {
		maxSize = 300
	}
	return &RingBuffer{
		entries: make([]LogEntry, maxSize),
		maxSize: maxSize,
	}
}

// Add appends a new entry to the ring buffer.
func (rb *RingBuffer) Add(entry LogEntry) {
	rb.mu.Lock()
	defer rb.mu.Unlock()

	rb.entries[rb.head] = entry
	rb.head = (rb.head + 1) % rb.maxSize
	if rb.head == 0 {
		rb.isFull = true
	}
}

// GetAll returns the buffered entries in chronological order.
func (rb *RingBuffer) GetAll(limit int, minLevel Level) []LogEntry {
	rb.mu.RLock()
	defer rb.mu.RUnlock()

	var total int
	if rb.isFull {
		total = rb.maxSize
	} else {
		total = rb.head
	}

	var result []LogEntry
	start := 0
	if rb.isFull {
		start = rb.head
	}

	for i := 0; i < total; i++ {
		idx := (start + i) % rb.maxSize
		e := rb.entries[idx]
		var entryLevel Level
		switch e.Level {
		case "DEBUG":
			entryLevel = LevelDebug
		case "INFO":
			entryLevel = LevelInfo
		case "WARN":
			entryLevel = LevelWarn
		case "ERROR":
			entryLevel = LevelError
		}

		if entryLevel >= minLevel {
			result = append(result, e)
		}
	}

	if limit > 0 && len(result) > limit {
		result = result[len(result)-limit:]
	}

	return result
}

// Logger provides leveled, multi-sink logging with health monitoring.
type Logger struct {
	mu              sync.Mutex
	minLevel        Level
	fileWriter      io.WriteCloser
	ringBuffer      *RingBuffer
	consecutiveErrs uint64
	totalScrapeErrs uint64
	totalReqs       uint64
	totalReqErrors  uint64
	alertCallback   func(alert string)
}

// Options configure the logger.
type Options struct {
	MinLevel   Level
	FilePath   string
	BufferSize int
}

var defaultLogger *Logger
var defaultOnce sync.Once

// Default returns a shared process-wide logger.
func Default() *Logger {
	defaultOnce.Do(func() {
		defaultLogger, _ = NewLogger(Options{
			MinLevel:   LevelInfo,
			FilePath:   "btu_scraper.log",
			BufferSize: 300,
		})
	})
	return defaultLogger
}

// NewLogger creates a new configured logger.
func NewLogger(opts Options) (*Logger, error) {
	var fw io.WriteCloser
	if opts.FilePath != "" {
		if dir := filepath.Dir(opts.FilePath); dir != "" && dir != "." {
			_ = os.MkdirAll(dir, 0755)
		}
		f, err := os.OpenFile(opts.FilePath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
		if err != nil {
			return nil, fmt.Errorf("failed to open log file %s: %w", opts.FilePath, err)
		}
		fw = f
	}

	bufSize := opts.BufferSize
	if bufSize <= 0 {
		bufSize = 300
	}

	return &Logger{
		minLevel:   opts.MinLevel,
		fileWriter: fw,
		ringBuffer: NewRingBuffer(bufSize),
	}, nil
}

// SetAlertCallback configures a function to call when an alert threshold is hit.
func (l *Logger) SetAlertCallback(cb func(alert string)) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.alertCallback = cb
}

func (l *Logger) log(level Level, component, format string, args ...interface{}) {
	if level < l.minLevel {
		return
	}

	msg := fmt.Sprintf(format, args...)
	now := time.Now()
	entry := LogEntry{
		Timestamp: now,
		Level:     level.String(),
		Component: component,
		Message:   msg,
	}

	if l.ringBuffer != nil {
		l.ringBuffer.Add(entry)
	}

	line := fmt.Sprintf("[%s] [%-5s] [%s] %s\n",
		now.Format("2006-01-02 15:04:05"),
		level.String(),
		component,
		msg,
	)

	l.mu.Lock()
	// Console output
	if level >= LevelWarn {
		_, _ = os.Stderr.WriteString(line)
	} else {
		_, _ = os.Stdout.WriteString(line)
	}

	// File output
	if l.fileWriter != nil {
		_, _ = l.fileWriter.Write([]byte(line))
	}
	l.mu.Unlock()
}

// Debug logs a debug message.
func (l *Logger) Debug(component, format string, args ...interface{}) {
	l.log(LevelDebug, component, format, args...)
}

// Info logs an informational message.
func (l *Logger) Info(component, format string, args ...interface{}) {
	l.log(LevelInfo, component, format, args...)
}

// Warn logs a warning message.
func (l *Logger) Warn(component, format string, args ...interface{}) {
	l.log(LevelWarn, component, format, args...)
}

// Error logs an error message.
func (l *Logger) Error(component, format string, args ...interface{}) {
	l.log(LevelError, component, format, args...)
}

// RecordScrapeSuccess resets the consecutive error counter.
func (l *Logger) RecordScrapeSuccess() {
	atomic.StoreUint64(&l.consecutiveErrs, 0)
}

// RecordScrapeFailure logs and increments consecutive scraper failure count.
func (l *Logger) RecordScrapeFailure(component, target string, err error) {
	fails := atomic.AddUint64(&l.consecutiveErrs, 1)
	atomic.AddUint64(&l.totalScrapeErrs, 1)

	l.Error(component, "Scrape failure on %s (consecutive: %d): %v", target, fails, err)

	// Trigger alert if error threshold exceeded
	if fails >= 5 {
		alertMsg := fmt.Sprintf("CRITICAL: Scraper component '%s' has failed %d times consecutively! Last error on %s: %v", component, fails, target, err)
		l.Warn("MONITOR", "%s", alertMsg)
		l.mu.Lock()
		cb := l.alertCallback
		l.mu.Unlock()
		if cb != nil {
			cb(alertMsg)
		}
	}
}

// RecordHTTPRequest records an HTTP request metrics for load monitoring.
func (l *Logger) RecordHTTPRequest(duration time.Duration, isError bool) {
	atomic.AddUint64(&l.totalReqs, 1)
	if isError {
		atomic.AddUint64(&l.totalReqErrors, 1)
	}

	// Detect slow requests / potential site overload
	if duration > 2*time.Second {
		l.Warn("WEB_LOAD", "High latency detected: request took %v", duration.Round(time.Millisecond))
	}
}

// GetHealthStats returns a snapshot of monitoring metrics.
func (l *Logger) GetHealthStats() map[string]interface{} {
	return map[string]interface{}{
		"consecutive_scrape_errors": atomic.LoadUint64(&l.consecutiveErrs),
		"total_scrape_errors":       atomic.LoadUint64(&l.totalScrapeErrs),
		"total_http_requests":       atomic.LoadUint64(&l.totalReqs),
		"total_http_errors":         atomic.LoadUint64(&l.totalReqErrors),
	}
}

// GetRecentLogs retrieves recent logs from the in-memory ring buffer.
func (l *Logger) GetRecentLogs(limit int, minLevel Level) []LogEntry {
	if l.ringBuffer == nil {
		return nil
	}
	return l.ringBuffer.GetAll(limit, minLevel)
}

// Close closes the underlying log file.
func (l *Logger) Close() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.fileWriter != nil {
		return l.fileWriter.Close()
	}
	return nil
}
