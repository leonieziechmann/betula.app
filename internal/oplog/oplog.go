// Package oplog is the operational log of Radix, the collector service, and of Cortex, the
// cache between Betula and the internet.
//
// Every record is one line with a level, a component and a stable event name:
//
//	{"time":"…","level":"ERROR","msg":"giving up on page","component":"crawl",
//	 "event":"crawl.job_failed","source":"module_page","key":"11101","error":"…"}
//
// Alerting keys on level=ERROR (something needs a human) or on an event name.
// WARN is for things that are wrong in the source data or that recovered by
// themselves; INFO tells the story of a run. The JSON format is meant for
// journald, Docker and log shippers, the text format for a terminal.
package oplog

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/internal/metrics"
)

// problemsTotal counts what the Recorder sees, for GET /metrics: one series per level and
// event name, which are a fixed vocabulary (docs/operations.md, "Logging").
var problemsTotal = metrics.Default.NewCounter("radix_log_problems_total",
	"Log records at WARN and ERROR, by level and event.", "level", "event")

// Options configure the process-wide logger.
type Options struct {
	Format string // "text" (default) or "json"
	Level  string // "debug", "info" (default), "warn", "error"
	File   string // optional: also append to this file

	// Problems counts the WARN and ERROR records by level and event name (two labels, in
	// that order). Nil: radix_log_problems_total in metrics.Default, Radix's. Cortex passes
	// its own cortex_log_problems_total, declared in the registry its GET /metrics serves.
	Problems *metrics.Counter
}

// OptionsFromEnv reads RADIX_LOG_FORMAT, RADIX_LOG_LEVEL and RADIX_LOG_FILE, so that a
// container or a systemd unit can configure logging without changing the command line.
func OptionsFromEnv() Options {
	return OptionsFromEnvPrefix("RADIX")
}

// OptionsFromEnvPrefix reads <prefix>_LOG_FORMAT, <prefix>_LOG_LEVEL and <prefix>_LOG_FILE:
// "RADIX" for Radix (OptionsFromEnv), "CORTEX" for Cortex.
func OptionsFromEnvPrefix(prefix string) Options {
	return Options{
		Format: os.Getenv(prefix + "_LOG_FORMAT"),
		Level:  os.Getenv(prefix + "_LOG_LEVEL"),
		File:   os.Getenv(prefix + "_LOG_FILE"),
	}
}

// Setup installs the process-wide slog logger and returns the recorder of recent
// problems plus a function that flushes and closes the log file.
func Setup(opt Options) (*Recorder, func() error, error) {
	var level slog.Level
	switch strings.ToLower(strings.TrimSpace(opt.Level)) {
	case "", "info":
		level = slog.LevelInfo
	case "debug":
		level = slog.LevelDebug
	case "warn", "warning":
		level = slog.LevelWarn
	case "error":
		level = slog.LevelError
	default:
		return nil, nil, fmt.Errorf("unknown log level %q (debug, info, warn, error)", opt.Level)
	}

	var out io.Writer = os.Stderr
	closeFn := func() error { return nil }
	if opt.File != "" {
		if dir := filepath.Dir(opt.File); dir != "" && dir != "." {
			if err := os.MkdirAll(dir, 0755); err != nil {
				return nil, nil, fmt.Errorf("failed to create log directory: %w", err)
			}
		}
		f, err := os.OpenFile(opt.File, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to open log file: %w", err)
		}
		out = io.MultiWriter(os.Stderr, f)
		closeFn = f.Close
	}

	handlerOpts := &slog.HandlerOptions{Level: level}
	var handler slog.Handler
	switch strings.ToLower(strings.TrimSpace(opt.Format)) {
	case "", "text":
		handler = slog.NewTextHandler(out, handlerOpts)
	case "json":
		handler = slog.NewJSONHandler(out, handlerOpts)
	default:
		return nil, nil, fmt.Errorf("unknown log format %q (text, json)", opt.Format)
	}

	recorder := NewRecorder(handler, 100)
	if opt.Problems != nil {
		recorder.state.counter = opt.Problems
	}
	slog.SetDefault(slog.New(recorder))
	return recorder, closeFn, nil
}

// For returns the logger of a component. Call it when logging, not at package
// initialisation, so that it picks up the logger installed by Setup.
func For(component string) *slog.Logger {
	return slog.Default().With("component", component)
}

// Err is the attribute every error is logged under.
func Err(err error) slog.Attr {
	if err == nil {
		return slog.String("error", "")
	}
	return slog.String("error", err.Error())
}

// Problem is a recorded WARN or ERROR record.
type Problem struct {
	Time    time.Time         `json:"time"`
	Level   string            `json:"level"`
	Message string            `json:"message"`
	Attrs   map[string]string `json:"attrs,omitempty"`
}

// Recorder is a slog.Handler that passes every record on and remembers the most
// recent warnings and errors, so that a status endpoint can show what went wrong
// without access to the log stream.
type Recorder struct {
	next  slog.Handler
	attrs []slog.Attr
	state *recorderState
}

type recorderState struct {
	mu       sync.Mutex
	problems []Problem
	max      int
	errors   int64
	warnings int64
	lastErr  time.Time
	counter  *metrics.Counter // {level, event}
}

// NewRecorder wraps next and keeps the last max problems. It counts them in
// radix_log_problems_total (Setup with Options.Problems counts them elsewhere).
func NewRecorder(next slog.Handler, max int) *Recorder {
	return &Recorder{next: next, state: &recorderState{max: max, counter: problemsTotal}}
}

func (r *Recorder) Enabled(ctx context.Context, level slog.Level) bool {
	return level >= slog.LevelWarn || r.next.Enabled(ctx, level)
}

func (r *Recorder) Handle(ctx context.Context, rec slog.Record) error {
	if rec.Level >= slog.LevelWarn {
		p := Problem{Time: rec.Time, Level: rec.Level.String(), Message: rec.Message, Attrs: make(map[string]string)}
		for _, a := range r.attrs {
			p.Attrs[a.Key] = a.Value.String()
		}
		rec.Attrs(func(a slog.Attr) bool {
			p.Attrs[a.Key] = a.Value.String()
			return true
		})

		r.state.counter.Inc(p.Level, p.Attrs["event"])

		s := r.state
		s.mu.Lock()
		if rec.Level >= slog.LevelError {
			s.errors++
			s.lastErr = rec.Time
		} else {
			s.warnings++
		}
		s.problems = append(s.problems, p)
		if len(s.problems) > s.max {
			s.problems = s.problems[len(s.problems)-s.max:]
		}
		s.mu.Unlock()
	}
	if !r.next.Enabled(ctx, rec.Level) {
		return nil
	}
	return r.next.Handle(ctx, rec)
}

func (r *Recorder) WithAttrs(attrs []slog.Attr) slog.Handler {
	return &Recorder{next: r.next.WithAttrs(attrs), attrs: append(append([]slog.Attr{}, r.attrs...), attrs...), state: r.state}
}

func (r *Recorder) WithGroup(name string) slog.Handler {
	return &Recorder{next: r.next.WithGroup(name), attrs: r.attrs, state: r.state}
}

// Summary is what the recorder knows since the process started.
type Summary struct {
	Errors    int64     `json:"errors"`
	Warnings  int64     `json:"warnings"`
	LastError time.Time `json:"last_error,omitempty"`
	Recent    []Problem `json:"recent"`
}

// Summary returns the counters and the most recent problems, newest first.
func (r *Recorder) Summary(limit int) Summary {
	s := r.state
	s.mu.Lock()
	defer s.mu.Unlock()

	sum := Summary{Errors: s.errors, Warnings: s.warnings, LastError: s.lastErr, Recent: []Problem{}}
	for i := len(s.problems) - 1; i >= 0 && len(sum.Recent) < limit; i-- {
		sum.Recent = append(sum.Recent, s.problems[i])
	}
	return sum
}
