package crawl

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"

	"github.com/jakob/btu-scraper/internal/catalogdb"
)

func openTestDB(t *testing.T) *catalogdb.DB {
	t.Helper()
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })
	return db
}

func TestRunArchivesPagesAndNotFound(t *testing.T) {
	var userAgent atomic.Value
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		userAgent.Store(r.Header.Get("User-Agent"))
		if r.URL.Path == "/modul/404" {
			http.NotFound(w, r)
			return
		}
		fmt.Fprintf(w, "<html>%s</html>", r.URL.Path)
	}))
	defer srv.Close()

	db := openTestDB(t)
	jobs := []Job{
		{Source: catalogdb.SourceModulePage, Key: "11101", URL: srv.URL + "/modul/11101"},
		{Source: catalogdb.SourceModulePage, Key: "404", URL: srv.URL + "/modul/404"},
	}
	stats, err := Run(context.Background(), db, jobs, Options{Workers: 2})
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if stats.Fetched != 1 || stats.NotFound != 1 || stats.Failed != 0 {
		t.Fatalf("stats = %+v", stats)
	}
	if got := userAgent.Load(); got != DefaultUserAgent {
		t.Fatalf("User-Agent = %v", got)
	}

	page, err := db.GetPage(catalogdb.SourceModulePage, "11101")
	if err != nil || string(page.Body) != "<html>/modul/11101</html>" {
		t.Fatalf("archived page = %+v (err %v)", page, err)
	}
	missing, err := db.GetPage(catalogdb.SourceModulePage, "404")
	if err != nil || missing.HTTPStatus != 404 || missing.Body != nil {
		t.Fatalf("archived 404 = %+v (err %v)", missing, err)
	}
}

func TestRunSkipsFreshPages(t *testing.T) {
	var hits int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&hits, 1)
		fmt.Fprint(w, "new")
	}))
	defer srv.Close()

	db := openTestDB(t)
	_ = db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: "fresh", URL: "u", HTTPStatus: 200, Body: []byte("old"), FetchedAt: time.Now()})
	_ = db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: "stale", URL: "u", HTTPStatus: 200, Body: []byte("old"), FetchedAt: time.Now().Add(-48 * time.Hour)})

	jobs := []Job{
		{Source: catalogdb.SourceModulePage, Key: "fresh", URL: srv.URL + "/fresh"},
		{Source: catalogdb.SourceModulePage, Key: "stale", URL: srv.URL + "/stale"},
	}
	stats, err := Run(context.Background(), db, jobs, Options{MaxAge: 24 * time.Hour})
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if stats.Skipped != 1 || stats.Fetched != 1 || atomic.LoadInt32(&hits) != 1 {
		t.Fatalf("stats = %+v, hits = %d", stats, hits)
	}
	fresh, _ := db.GetPage(catalogdb.SourceModulePage, "fresh")
	if string(fresh.Body) != "old" {
		t.Fatalf("fresh page was refetched: %q", fresh.Body)
	}
}

func TestRunRetriesThenAbortsWhenServerKeepsFailing(t *testing.T) {
	var hits int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&hits, 1)
		http.Error(w, "busy", http.StatusServiceUnavailable)
	}))
	defer srv.Close()

	db := openTestDB(t)
	var jobs []Job
	for i := 0; i < 50; i++ {
		jobs = append(jobs, Job{Source: catalogdb.SourceModulePage, Key: fmt.Sprint(i), URL: srv.URL})
	}
	stats, err := Run(context.Background(), db, jobs, Options{Backoff: time.Millisecond})
	if !errors.Is(err, ErrServerUnhealthy) {
		t.Fatalf("Run error = %v, want ErrServerUnhealthy", err)
	}
	if stats.Failed != maxConsecutiveFailures {
		t.Fatalf("failed jobs = %d, want %d", stats.Failed, maxConsecutiveFailures)
	}
	if got := atomic.LoadInt32(&hits); got != maxConsecutiveFailures*maxAttempts {
		t.Fatalf("requests = %d, want %d", got, maxConsecutiveFailures*maxAttempts)
	}
	if _, err := db.GetPage(catalogdb.SourceModulePage, "0"); err != catalogdb.ErrNotFound {
		t.Fatalf("failed responses must not be archived, got err %v", err)
	}
}

func TestRunRecoversAfterTransientFailure(t *testing.T) {
	var hits int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if atomic.AddInt32(&hits, 1) == 1 {
			http.Error(w, "busy", http.StatusTooManyRequests)
			return
		}
		fmt.Fprint(w, "ok")
	}))
	defer srv.Close()

	db := openTestDB(t)
	stats, err := Run(context.Background(), db, []Job{{Source: catalogdb.SourceModulePage, Key: "1", URL: srv.URL}}, Options{Backoff: time.Millisecond})
	if err != nil || stats.Fetched != 1 || stats.Failed != 0 {
		t.Fatalf("stats = %+v, err = %v", stats, err)
	}
}
