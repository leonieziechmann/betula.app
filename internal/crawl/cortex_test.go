package crawl

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	cortexclient "github.com/leonieziechmann/betula/internal/cortex/client"
	"github.com/leonieziechmann/betula/internal/cortex/client/cortextest"
	"github.com/leonieziechmann/betula/internal/metrics"
)

// checkedAt is when the fake Cortex says it last fetched every answer, as for answers from
// its store: earlier than the requests.
var checkedAt = time.Date(2026, 10, 2, 8, 15, 0, 123456000, time.UTC)

// throughCortex starts a fake university behind a fake Cortex, and the client the crawl
// asks Cortex with, as Radix builds it.
func throughCortex(t *testing.T) (site *httptest.Server, cortex *cortextest.Server, client *http.Client) {
	t.Helper()
	site = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/modul/404" {
			http.NotFound(w, r)
			return
		}
		fmt.Fprintf(w, "<html>%s</html>", r.URL.Path)
	}))
	t.Cleanup(site.Close)
	cortex = cortextest.NewServer()
	t.Cleanup(cortex.Close)
	cortex.CheckedAt(checkedAt)
	c, err := cortexclient.New(cortex.URL, cortexclient.Options{})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	return site, cortex, c.HTTPClient(cortexclient.FetchOptions{Mode: cortexclient.ModeCache, MaxAge: time.Hour, Stale: cortexclient.StaleNever}, 3*time.Minute)
}

// checkFetches checks that every request reached Cortex as a fetch of the job's page in
// mode cache, with the job's source and the crawl's User-Agent.
func checkFetches(t *testing.T, cortex *cortextest.Server, jobs ...Job) {
	t.Helper()
	fetches := cortex.Fetches()
	if len(fetches) != len(jobs) {
		t.Fatalf("Cortex got %d fetches, want %d", len(fetches), len(jobs))
	}
	want := make(map[string]string)
	for _, j := range jobs {
		want[j.URL] = j.Source
	}
	for _, f := range fetches {
		if source, ok := want[f.URL]; !ok || f.Source != source || f.Mode != "cache" || f.MaxAge != "1h" || f.Stale != "never" ||
			f.Header.Get("User-Agent") != DefaultUserAgent {
			t.Errorf("Cortex got %+v (User-Agent %q), want url of a job, source %q, mode=cache max_age=1h stale=never",
				f.Query, f.Header.Get("User-Agent"), source)
		}
	}
}

// Through Cortex the pages are archived as fetched when Cortex fetched them, not when Radix
// asked: a page from Cortex's store is not taken for a fresh one.
func TestRunThroughCortexArchivesCortexCheckedAt(t *testing.T) {
	site, cortex, client := throughCortex(t)
	db := openTestDB(t)
	jobs := []Job{
		{Source: catalogdb.SourceModuleCatalog, Key: "list", URL: site.URL + "/modul"},
		{Source: catalogdb.SourceQISFUESList, Key: "list", URL: site.URL + "/fues?P_start=0&P_anzahl=9999"},
		{Source: catalogdb.SourceModulePage, Key: "404", URL: site.URL + "/modul/404"},
	}
	stats, err := Run(context.Background(), db, jobs, Options{Workers: 2, Client: client})
	if err != nil || stats.Fetched != 2 || stats.NotFound != 1 || stats.Failed != 0 {
		t.Fatalf("Run = %+v (err %v)", stats, err)
	}
	checkFetches(t, cortex, jobs...)
	for _, j := range jobs {
		p, err := db.GetPage(j.Source, j.Key)
		if err != nil {
			t.Fatalf("GetPage %s: %v", j.Key, err)
		}
		if !p.FetchedAt.Equal(checkedAt.Truncate(time.Second)) {
			t.Errorf("%s/%s fetched_at = %v, want Cortex-Checked-At %v", j.Source, j.Key, p.FetchedAt, checkedAt)
		}
	}
	if p, _ := db.GetPage(catalogdb.SourceQISFUESList, "list"); string(p.Body) != "<html>/fues</html>" {
		t.Errorf("archived body = %q", p.Body)
	}
}

func TestFetcherThroughCortex(t *testing.T) {
	site, cortex, client := throughCortex(t)
	db := openTestDB(t)
	fetcher := NewFetcher(db, Options{Client: client})
	ctx := context.Background()

	tree := Job{Source: catalogdb.SourceQISTree, Key: "root", URL: site.URL + "/tree?nodeID=auswahlBaum%7Cstudiengang"}
	body, err := fetcher.Get(ctx, tree)
	if err != nil || string(body) != "<html>/tree</html>" {
		t.Fatalf("Get = %q, %v", body, err)
	}
	if p, _ := db.GetPage(tree.Source, tree.Key); !p.FetchedAt.Equal(checkedAt.Truncate(time.Second)) {
		t.Errorf("fetched_at = %v, want Cortex-Checked-At %v", p.FetchedAt, checkedAt)
	}

	search := Job{Source: catalogdb.SourceQISEventEntry, Key: "search:1..2", URL: site.URL + "/search?veranstaltung.veranstid=1,2"}
	body, at, err := fetcher.Download(ctx, search)
	if err != nil || string(body) != "<html>/search</html>" || !at.Equal(checkedAt) {
		t.Fatalf("Download = %q, %v, %v; want the body at Cortex-Checked-At %v", body, at, err, checkedAt)
	}
	if _, err := db.GetPage(search.Source, search.Key); !errors.Is(err, catalogdb.ErrNotFound) {
		t.Errorf("Download archived its answer: %v", err)
	}
	checkFetches(t, cortex, tree, search)
	if s := fetcher.Stats(); s.Fetched != 2 || s.Failed != 0 {
		t.Errorf("stats = %+v", s)
	}
}

// Without Cortex, an answer is archived as fetched now.
func TestDownloadDirectlyIsFetchedNow(t *testing.T) {
	site := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, "list") }))
	defer site.Close()
	before := time.Now()
	_, at, err := NewFetcher(openTestDB(t), Options{}).Download(context.Background(), Job{Source: catalogdb.SourceQISEventEntry, Key: "k", URL: site.URL})
	if err != nil || at.Before(before) || at.After(time.Now()) {
		t.Errorf("Download at %v (err %v), want between %v and now", at, err, before)
	}
}

// Cortex may answer from its store with a page older than the one archived, fetched
// directly before Radix used Cortex: the archive keeps the newer one.
func TestThroughCortexFetchedAtNeverGoesBackwards(t *testing.T) {
	site, _, client := throughCortex(t)
	db := openTestDB(t)
	newer := checkedAt.Add(2 * time.Hour).Truncate(time.Second)
	for _, key := range []string{"11101", "11102"} {
		if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: key, URL: "u", HTTPStatus: 200, Body: []byte("newer"), FetchedAt: newer}); err != nil {
			t.Fatalf("PutPage: %v", err)
		}
	}

	job := Job{Source: catalogdb.SourceModulePage, Key: "11101", URL: site.URL + "/modul/11101"}
	stats, err := Run(context.Background(), db, []Job{job}, Options{Client: client})
	if err != nil || stats.Fetched != 1 || stats.Changed != 0 {
		t.Fatalf("Run = %+v (err %v), want one unchanged page", stats, err)
	}
	if p, _ := db.GetPage(job.Source, job.Key); string(p.Body) != "newer" || !p.FetchedAt.Equal(newer) {
		t.Errorf("archive = %q at %v, want the newer page at %v", p.Body, p.FetchedAt, newer)
	}

	// Get answers with what the archive holds.
	body, err := NewFetcher(db, Options{Client: client}).Get(context.Background(),
		Job{Source: catalogdb.SourceModulePage, Key: "11102", URL: site.URL + "/modul/11102"})
	if err != nil || string(body) != "newer" {
		t.Errorf("Get = %q, %v, want the newer archived page", body, err)
	}
}

// stale=never: when the host fails, Cortex says so, and the crawl retries and gives up as
// it does without Cortex.
func TestThroughCortexAFailureStaysAFailure(t *testing.T) {
	site, cortex, client := throughCortex(t)
	cortex.Fail(http.StatusBadGateway, "upstream-failed")
	db := openTestDB(t)
	stats, err := Run(context.Background(), db, []Job{{Source: catalogdb.SourceModulePage, Key: "1", URL: site.URL + "/modul/1"}},
		Options{Client: client, Backoff: time.Millisecond})
	if err != nil || stats.Failed != 1 || len(cortex.Fetches()) != maxAttempts {
		t.Errorf("Run = %+v (err %v), %d fetches; want the page failed after %d attempts", stats, err, len(cortex.Fetches()), maxAttempts)
	}
	if _, err := db.GetPage(catalogdb.SourceModulePage, "1"); !errors.Is(err, catalogdb.ErrNotFound) {
		t.Errorf("Cortex's error was archived: %v", err)
	}
}

// requests is the count of radix_crawl_requests_total for source and code so far.
func requests(t *testing.T, source, code string) float64 {
	t.Helper()
	var text bytes.Buffer
	if err := metrics.Default.WriteText(&text); err != nil {
		t.Fatalf("WriteText: %v", err)
	}
	series := fmt.Sprintf("radix_crawl_requests_total{source=%q,code=%q} ", source, code)
	for _, line := range strings.Split(text.String(), "\n") {
		if v, ok := strings.CutPrefix(line, series); ok {
			n, err := strconv.ParseFloat(v, 64)
			if err != nil {
				t.Fatalf("%s: %v", line, err)
			}
			return n
		}
	}
	return 0
}

// Cortex's own error is a failed attempt, never the page. Its 404 not-found (its answer for a
// path it does not serve, as below a RADIX_CORTEX_URL with a path) says nothing about the
// page: it neither replaces the archived page with an empty 404 nor counts as a page that is
// gone. The crawl retries it, gives the page up, counts the requests as without an answer,
// and aborts when page after page fails so, as when the university keeps failing; Get and
// Download fail alike, rather than with ErrNotFound.
func TestThroughCortexItsOwnErrorIsNeverAPage(t *testing.T) {
	site, cortex, client := throughCortex(t)
	cortex.Fail(http.StatusNotFound, "not-found")
	db := openTestDB(t)
	archived := catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: "11101", URL: site.URL + "/modul/11101", HTTPStatus: 200,
		Body: []byte("<html>module 11101</html>"), FetchedAt: time.Now().Add(-48 * time.Hour).Truncate(time.Second)}
	if err := db.PutPage(archived); err != nil {
		t.Fatalf("PutPage: %v", err)
	}
	unchanged := func(when string) {
		t.Helper()
		if p, err := db.GetPage(archived.Source, archived.Key); err != nil || p.HTTPStatus != 200 || string(p.Body) != string(archived.Body) ||
			!p.FetchedAt.Equal(archived.FetchedAt) {
			t.Errorf("%s: archive = %d %q at %v (err %v), want the archived page unchanged", when, p.HTTPStatus, p.Body, p.FetchedAt, err)
		}
	}
	job := Job{Source: archived.Source, Key: archived.Key, URL: archived.URL}
	failedBefore, notFoundBefore := requests(t, job.Source, "error"), requests(t, job.Source, "404")

	stats, err := Run(context.Background(), db, []Job{job}, Options{Client: client, Backoff: time.Millisecond})
	if err != nil || stats.Failed != 1 || stats.NotFound != 0 || len(cortex.Fetches()) != maxAttempts {
		t.Errorf("Run = %+v (err %v), %d fetches; want the page failed after %d attempts", stats, err, len(cortex.Fetches()), maxAttempts)
	}
	unchanged("Run")
	if failed, notFound := requests(t, job.Source, "error")-failedBefore, requests(t, job.Source, "404")-notFoundBefore; failed != maxAttempts || notFound != 0 {
		t.Errorf("requests counted: %v error, %v 404; want %d error, no 404", failed, notFound, maxAttempts)
	}

	fetcher := NewFetcher(db, Options{Client: client, Backoff: time.Millisecond})
	if body, err := fetcher.Get(context.Background(), job); err == nil || errors.Is(err, ErrNotFound) || !strings.Contains(err.Error(), "404 not-found") {
		t.Errorf("Get = %q, %v; want Cortex's error, not ErrNotFound", body, err)
	}
	unchanged("Get")
	search := Job{Source: catalogdb.SourceQISEventEntry, Key: "search:1..2", URL: site.URL + "/search?veranstaltung.veranstid=1,2"}
	if body, _, err := fetcher.Download(context.Background(), search); err == nil || errors.Is(err, ErrNotFound) ||
		!strings.Contains(err.Error(), "404 not-found") {
		t.Errorf("Download = %q, %v; want Cortex's error, not ErrNotFound", body, err)
	}
	if s := fetcher.Stats(); s.Failed != 2 || s.NotFound != 0 || s.Fetched != 0 {
		t.Errorf("Fetcher stats = %+v, want 2 failed", s)
	}
	for i := 2; ; i++ {
		_, err := fetcher.Get(context.Background(), job)
		if errors.Is(err, ErrServerUnhealthy) {
			if i != maxConsecutiveFailures-1 {
				t.Errorf("the Fetcher gave up after %d failures in a row, want %d", i+1, maxConsecutiveFailures)
			}
			break
		}
		if i >= maxConsecutiveFailures {
			t.Fatalf("the Fetcher did not give up after %d failures in a row: %v", i+1, err)
		}
	}

	jobs := make([]Job, maxConsecutiveFailures)
	for i := range jobs {
		jobs[i] = Job{Source: catalogdb.SourceModulePage, Key: strconv.Itoa(20000 + i), URL: fmt.Sprintf("%s/modul/%d", site.URL, 20000+i)}
	}
	stats, err = Run(context.Background(), db, jobs, Options{Client: client, Backoff: time.Millisecond})
	if !errors.Is(err, ErrServerUnhealthy) || stats.NotFound != 0 || stats.Failed != maxConsecutiveFailures {
		t.Errorf("Run of %d pages = %+v (err %v), want them failed and the crawl aborted", len(jobs), stats, err)
	}
	unchanged("the aborted crawl")
}
