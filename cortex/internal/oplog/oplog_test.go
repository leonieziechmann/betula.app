package oplog

import (
	"bytes"
	"encoding/json"
	"errors"
	"log/slog"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/cortex/internal/metrics"
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

func TestOptionsFromEnvReadsCortexsVariables(t *testing.T) {
	t.Setenv("CORTEX_LOG_FORMAT", "json")
	t.Setenv("CORTEX_LOG_LEVEL", "debug")
	t.Setenv("CORTEX_LOG_FILE", "cortex.log")
	if got, want := OptionsFromEnv(), (Options{Format: "json", Level: "debug", File: "cortex.log"}); got != want {
		t.Errorf("OptionsFromEnv() = %+v, want %+v", got, want)
	}
}

func TestSetupCountsProblemsInTheCounterItIsGiven(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })

	reg := metrics.NewRegistry()
	own := reg.NewCounter("cortex_log_problems_total", "Log records at WARN and ERROR.", "level", "event")
	_, closeLog, err := Setup(Options{Level: "error", Problems: own})
	if err != nil {
		t.Fatal(err)
	}
	defer closeLog()
	For("upstream").Warn("host paused", "event", "host.paused")
	For("cortex").Error("stopping", "event", "service.fatal")

	var text bytes.Buffer
	if err := reg.WriteText(&text); err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		`cortex_log_problems_total{level="WARN",event="host.paused"} 1`,
		`cortex_log_problems_total{level="ERROR",event="service.fatal"} 1`,
	} {
		if !strings.Contains(text.String(), want) {
			t.Errorf("missing %s in\n%s", want, text.String())
		}
	}
}
