package oplog

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/internal/metrics"
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

func TestOptionsFromEnvPrefixReadsTheNamedVariables(t *testing.T) {
	t.Setenv("RADIX_LOG_FORMAT", "json")
	t.Setenv("RADIX_LOG_LEVEL", "debug")
	t.Setenv("RADIX_LOG_FILE", "radix.log")
	t.Setenv("CORTEX_LOG_FORMAT", "text")
	t.Setenv("CORTEX_LOG_LEVEL", "warn")
	t.Setenv("CORTEX_LOG_FILE", "")

	if got, want := OptionsFromEnv(), (Options{Format: "json", Level: "debug", File: "radix.log"}); got != want {
		t.Errorf("OptionsFromEnv() = %+v, want %+v", got, want)
	}
	if got, want := OptionsFromEnvPrefix("CORTEX"), (Options{Format: "text", Level: "warn"}); got != want {
		t.Errorf("OptionsFromEnvPrefix(CORTEX) = %+v, want %+v", got, want)
	}
}

func TestSetupCountsProblemsInTheCounterItIsGiven(t *testing.T) {
	previous := slog.Default()
	t.Cleanup(func() { slog.SetDefault(previous) })

	reg := metrics.NewRegistry()
	own := reg.NewCounter("cortex_log_problems_total", "Log records at WARN and ERROR.", "level", "event")
	var radixBefore bytes.Buffer
	if err := metrics.Default.WriteText(&radixBefore); err != nil {
		t.Fatal(err)
	}

	recorder, closeLog, err := Setup(Options{Level: "error", Problems: own})
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
			t.Errorf("own counter lacks %s:\n%s", want, text.String())
		}
	}
	var radixAfter bytes.Buffer
	if err := metrics.Default.WriteText(&radixAfter); err != nil {
		t.Fatal(err)
	}
	if radixAfter.String() != radixBefore.String() {
		t.Errorf("radix_log_problems_total moved:\nbefore %s\nafter %s", radixBefore.String(), radixAfter.String())
	}
	if sum := recorder.Summary(5); sum.Warnings != 1 || sum.Errors != 1 {
		t.Errorf("summary = %+v, want one warning and one error", sum)
	}
}

func TestNewRecorderStillCountsInRadixsCounter(t *testing.T) {
	const series = `radix_log_problems_total{level="WARN",event="oplog.test_radix_counter"} `
	count := func() string {
		var text bytes.Buffer
		if err := metrics.Default.WriteText(&text); err != nil {
			t.Fatal(err)
		}
		for _, line := range strings.Split(text.String(), "\n") {
			if v, ok := strings.CutPrefix(line, series); ok {
				return v
			}
		}
		return "0"
	}
	before := count()
	recorder := NewRecorder(slog.NewTextHandler(io.Discard, nil), 5)
	slog.New(recorder).Warn("request failed, retrying", "event", "oplog.test_radix_counter")
	if after := count(); after == before {
		t.Errorf("radix_log_problems_total did not count the warning (%s before and after)", before)
	}
}
