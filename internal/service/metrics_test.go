package service

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
)

// scrape reads GET /metrics into series → value. The counters are shared by every test
// of the package, so tests compare differences or what is read at scrape time.
func scrape(t *testing.T, srv *httptest.Server) map[string]float64 {
	t.Helper()
	resp, err := http.Get(srv.URL + "/metrics")
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("/metrics = %d %s", resp.StatusCode, body)
	}
	values := make(map[string]float64)
	for _, line := range strings.Split(string(body), "\n") {
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		i := strings.LastIndexByte(line, ' ')
		v, err := strconv.ParseFloat(line[i+1:], 64)
		if err != nil {
			t.Fatalf("unreadable line %q: %v", line, err)
		}
		values[line[:i]] = v
	}
	return values
}

func TestMetricsCountWhatACycleDid(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	srv := httptest.NewServer(svc.Handler())
	t.Cleanup(srv.Close)

	before := scrape(t, srv)
	if r := svc.RunCycle(context.Background()); r.Result != "ok" || !r.Published {
		t.Fatalf("cycle = %+v", r)
	}
	resp, err := http.Get(srv.URL + "/snapshot/catalog.db")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	after := scrape(t, srv)

	grew := func(series string, want float64) {
		t.Helper()
		if got := after[series] - before[series]; got != want {
			t.Errorf("%s grew by %v, want %v", series, got, want)
		}
	}
	// What the fake BTU was asked: two module pages, one search for two events, their pages.
	grew(`radix_crawl_requests_total{source="module_page",code="200"}`, 2)
	grew(`radix_crawl_requests_total{source="qis_event_entry",code="200"}`, 1)
	grew(`radix_crawl_requests_total{source="qis_event",code="200"}`, 2)
	grew(`radix_crawl_pages_total{source="module_page",outcome="changed"}`, 2)
	grew(`radix_crawl_pages_total{source="qis_event_entry",outcome="changed"}`, 2)
	grew(`radix_crawl_pages_total{source="qis_tree",outcome="changed"}`, 5)
	grew(`radix_crawl_request_duration_seconds_count{source="qis_event"}`, 2)
	grew(`radix_cycles_total{result="ok"}`, 1)
	grew(`radix_stage_runs_total{stage="export",outcome="ok"}`, 1)
	grew(`radix_builds_total{content="changed"}`, 1)
	grew(`radix_snapshots_published_total`, 1)
	grew(`radix_snapshot_requests_total{file="catalog.db",code="200"}`, 1)

	for series, want := range map[string]float64{
		`radix_healthy`:                                         1,
		`radix_cycle_running`:                                   0,
		`radix_failed_cycles_in_a_row`:                          0,
		`radix_catalog_items{kind="modules"}`:                   2,
		`radix_archive_pages{source="module_page",status="ok"}`: 2,
		`radix_archive_pages{source="qis_tree",status="ok"}`:    5,
		`radix_archive_fetched_24h{source="module_page"}`:       2,
		`radix_archive_changed_24h{source="qis_event_entry"}`:   2,
	} {
		if got, ok := after[series]; !ok || got != want {
			t.Errorf("%s = %v (present %v), want %v", series, got, ok, want)
		}
	}
	if after[`radix_snapshot_exported_timestamp_seconds`] == 0 || after[`radix_last_success_timestamp_seconds`] == 0 {
		t.Errorf("no time of the snapshot or of the last success: %v", after)
	}
}
