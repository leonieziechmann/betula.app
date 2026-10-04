package crawl

import (
	"context"
	"errors"
	"fmt"
	"testing"
	"time"

	cortexclient "github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// Offline through Cortex (RADIX_CORTEX_MODE=offline), a page Cortex has not stored is skipped:
// asked once, not retried, no failure and no abort however many there are, and the archive
// keeps what it has. The fake Cortex stores nothing, so every page misses.
func TestOfflineThroughCortexSkipsWhatCortexLacks(t *testing.T) {
	site, cortex, _ := throughCortex(t)
	c, err := cortexclient.New(cortex.URL, cortexclient.Options{})
	if err != nil {
		t.Fatal(err)
	}
	client := c.HTTPClient(cortexclient.FetchOptions{Mode: cortexclient.ModeOffline, Stale: cortexclient.StaleNever}, time.Minute)
	db := openTestDB(t)
	archivedAt := time.Date(2026, 10, 1, 3, 0, 0, 0, time.UTC)
	if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: "1", URL: site.URL + "/modul/1", HTTPStatus: 200,
		Body: []byte("archived"), FetchedAt: archivedAt}); err != nil {
		t.Fatal(err)
	}

	var jobs []Job
	for i := 1; i <= maxConsecutiveFailures+2; i++ {
		jobs = append(jobs, Job{Source: catalogdb.SourceModulePage, Key: fmt.Sprint(i), URL: fmt.Sprintf("%s/modul/%d", site.URL, i)})
	}
	before := requests(t, catalogdb.SourceModulePage, "offline_miss")
	stats, err := Run(context.Background(), db, jobs, Options{Workers: 2, Client: client, Backoff: time.Hour})
	if err != nil || stats.OfflineMiss != len(jobs) || stats.Failed != 0 || stats.Fetched != 0 {
		t.Fatalf("Run = %+v (err %v), want every page an offline miss and nothing failed", stats, err)
	}
	if n := len(cortex.Fetches()); n != len(jobs) {
		t.Errorf("Cortex was asked %d times, want once per page", n)
	}
	for _, f := range cortex.Fetches() {
		if f.Mode != "offline" {
			t.Errorf("fetch in mode %q, want offline", f.Mode)
		}
	}
	if got := requests(t, catalogdb.SourceModulePage, "offline_miss") - before; got != float64(len(jobs)) {
		t.Errorf("radix_crawl_requests_total{code=offline_miss} rose by %v, want %d", got, len(jobs))
	}
	if p, err := db.GetPage(catalogdb.SourceModulePage, "1"); err != nil || string(p.Body) != "archived" || !p.FetchedAt.Equal(archivedAt) {
		t.Errorf("the archived page changed: %+v (err %v)", p, err)
	}

	// Get answers with the archive; without an archived page it says so.
	fetcher := NewFetcher(db, Options{Client: client})
	if body, err := fetcher.Get(context.Background(), jobs[0]); err != nil || string(body) != "archived" {
		t.Errorf("Get of an archived page = %q, %v; want the archived page", body, err)
	}
	if _, err := fetcher.Get(context.Background(), jobs[1]); !errors.Is(err, ErrOfflineMiss) {
		t.Errorf("Get of a page neither Cortex nor the archive has: %v, want ErrOfflineMiss", err)
	}
	if _, _, err := fetcher.Download(context.Background(), jobs[2]); !errors.Is(err, ErrOfflineMiss) {
		t.Errorf("Download: %v, want ErrOfflineMiss", err)
	}
	if s := fetcher.Stats(); s.OfflineMiss != 3 || s.Failed != 0 {
		t.Errorf("fetcher stats = %+v, want 3 offline misses and no failure", s)
	}
}
