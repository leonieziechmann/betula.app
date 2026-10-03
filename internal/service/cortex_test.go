package service

import (
	"context"
	"sort"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	cortexclient "github.com/leonieziechmann/betula/internal/cortex/client"
	"github.com/leonieziechmann/betula/internal/cortex/client/cortextest"
)

// With Endpoints.Client every stage asks Cortex, nothing goes to the university directly,
// each request names the source of its pages, and every page is archived as fetched when
// Cortex fetched it: the pages of the crawl stages and the entries of the event search.
func TestEveryStageAsksCortexWithItsSource(t *testing.T) {
	site := newFakeBTU(t)
	cortex := cortextest.NewServer()
	t.Cleanup(cortex.Close)
	checkedAt := time.Date(2026, 10, 2, 8, 15, 0, 123456000, time.UTC)
	cortex.CheckedAt(checkedAt)
	c, err := cortexclient.New(cortex.URL, cortexclient.Options{})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	svc, _ := newTestService(t, site)
	svc.cfg.Endpoints.Client = c.HTTPClient(cortexclient.FetchOptions{Mode: cortexclient.ModeCache, MaxAge: time.Hour, Stale: cortexclient.StaleNever}, 3*time.Minute)

	result := svc.RunCycle(context.Background())
	if result.Result != "ok" || !result.Published {
		t.Fatalf("cycle through Cortex = %+v", result)
	}

	fetches := cortex.Fetches()
	site.mu.Lock()
	hits := 0
	for _, n := range site.hits {
		hits += n
	}
	site.mu.Unlock()
	if hits == 0 || len(fetches) != hits {
		t.Errorf("Cortex got %d fetches, the university %d requests; want each request through Cortex", len(fetches), hits)
	}
	sources := make(map[string]bool)
	for _, f := range fetches {
		sources[f.Source] = true
		if f.Mode != "cache" || f.MaxAge != "1h" || f.Stale != "never" {
			t.Errorf("fetch %+v", f.Query)
		}
	}
	var got []string
	for s := range sources {
		got = append(got, s)
	}
	sort.Strings(got)
	for _, want := range []string{catalogdb.SourceModuleCatalog, catalogdb.SourceQISFUESList, catalogdb.SourceModulePage,
		catalogdb.SourceQISModuleList, catalogdb.SourceQISModulePage, catalogdb.SourceQISTree, catalogdb.SourceQISEventEntry,
		catalogdb.SourceQISEvent} {
		if !sources[want] {
			t.Errorf("no fetch named the source %s; sources %v", want, got)
		}
	}

	rows, err := svc.db.SQL().Query("SELECT DISTINCT source, fetched_at FROM raw_page ORDER BY source")
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	archived := 0
	for rows.Next() {
		var source, fetchedAt string
		if err := rows.Scan(&source, &fetchedAt); err != nil {
			t.Fatal(err)
		}
		archived++
		if fetchedAt != checkedAt.Format(time.RFC3339) {
			t.Errorf("%s archived as fetched at %s, want Cortex-Checked-At %s", source, fetchedAt, checkedAt.Format(time.RFC3339))
		}
	}
	if err := rows.Err(); err != nil || archived < 8 {
		t.Errorf("%d sources archived (err %v)", archived, err)
	}
}
