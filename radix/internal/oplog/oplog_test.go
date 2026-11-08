package oplog

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"strings"
	"testing"
)

func TestRecorderKeepsProblemsAndPassesEverythingOn(t *testing.T) {
	var out bytes.Buffer
	recorder := NewRecorder(slog.NewJSONHandler(&out, &slog.HandlerOptions{Level: slog.LevelInfo}), 2)
	log := slog.New(recorder).With("component", "crawl")

	log.Debug("not shown")
	log.Info("crawl started", "event", "crawl.started")
	log.Warn("request failed, retrying", "event", "crawl.retry", "key", "11101")
	log.Error("giving up on page", "event", "crawl.job_failed", "key", "11101", Err(errors.New("unexpected status 503")))
	log.Error("crawl aborted", "event", "crawl.aborted")

	lines := strings.Split(strings.TrimSpace(out.String()), "\n")
	if len(lines) != 4 {
		t.Fatalf("handler got %d lines, want 4 (debug is below the level):\n%s", len(lines), out.String())
	}
	var failed map[string]any
	if err := json.Unmarshal([]byte(lines[2]), &failed); err != nil {
		t.Fatal(err)
	}
	if failed["level"] != "ERROR" || failed["component"] != "crawl" || failed["event"] != "crawl.job_failed" || failed["error"] != "unexpected status 503" {
		t.Errorf("error line = %v", failed)
	}

	sum := recorder.Summary(10)
	if sum.Errors != 2 || sum.Warnings != 1 || sum.LastError.IsZero() {
		t.Errorf("counters = %+v", sum)
	}
	// Only the newest `max` problems are kept, newest first, with their attributes.
	if len(sum.Recent) != 2 || sum.Recent[0].Attrs["event"] != "crawl.aborted" || sum.Recent[1].Attrs["key"] != "11101" || sum.Recent[1].Attrs["component"] != "crawl" {
		t.Errorf("recent = %+v", sum.Recent)
	}
}

func TestSetupRejectsUnknownOptions(t *testing.T) {
	if _, _, err := Setup(Options{Level: "loud"}); err == nil {
		t.Error("unknown level accepted")
	}
	if _, _, err := Setup(Options{Format: "xml"}); err == nil {
		t.Error("unknown format accepted")
	}
}

// A deploy stops the process with SIGTERM, which cancels the service's context: what that cut
// short needs nobody. A "context canceled" from elsewhere, and a real error while stopping, do.
func TestInterruptedIsOnlyTheCancellationOfTheContext(t *testing.T) {
	stopped, stop := context.WithCancel(context.Background())
	stop()
	expired, expire := context.WithTimeout(context.Background(), 0)
	defer expire()
	<-expired.Done()

	for _, c := range []struct {
		name string
		ctx  context.Context
		err  error
		want bool
	}{
		{"the build sees the stop", stopped, context.Canceled, true},
		{"wrapped on the way out", stopped, fmt.Errorf("build step %q failed: %w", "modules", context.Canceled), true},
		{"a real error while stopping", stopped, errors.New("disk I/O error"), false},
		{"nothing went wrong", stopped, nil, false},
		{"cancelled elsewhere, the process runs on", context.Background(), context.Canceled, false},
		{"a deadline, not a stop", expired, context.DeadlineExceeded, false},
	} {
		if got := Interrupted(c.ctx, c.err); got != c.want {
			t.Errorf("%s: Interrupted = %v, want %v", c.name, got, c.want)
		}
	}
}
