// Package service runs the scraper as a long-lived process: it keeps the raw
// archive fresh at a polite pace, rebuilds the catalog, and publishes a new
// snapshot when the data changed. The stages are also used one by one by the CLI.
package service

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/url"
	"sort"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
	"github.com/leonieziechmann/btu-scraper/internal/crawl"
	"github.com/leonieziechmann/btu-scraper/internal/oplog"
	"github.com/leonieziechmann/btu-scraper/internal/parser"
	"github.com/leonieziechmann/btu-scraper/internal/qistree"
)

// Pace is how fast and how much one stage may fetch.
type Pace struct {
	Workers int
	Delay   time.Duration
	MaxAge  time.Duration // pages archived more recently are not fetched again
	Limit   int           // at most this many pages are requested per run, oldest first; 0 = no limit
	Backoff time.Duration // first pause after a failed request (default 30 s)
}

// Endpoints are the pages the service reads. Tests point them at a local server.
type Endpoints struct {
	CatalogURL  string
	FUESURL     string
	ModuleURL   string // fmt template with the module ID
	EventURL    string // fmt template with the QIS event ID
	TreeRootURL string
}

// BTUEndpoints are the live BTU pages.
func BTUEndpoints() Endpoints {
	const qis = "https://www.b-tu.de/qisserver3/rds"
	return Endpoints{
		CatalogURL: "https://www.b-tu.de/modul",
		ModuleURL:  "https://www.b-tu.de/modul/%s",
		FUESURL:    qis + "?state=change&type=3&moduleParameter=pordpos&nextdir=change&next=TableSelectModul.vm&subdir=pord&P_start=0&P_anzahl=9999&missing=FUES",
		EventURL:   qis + "?state=verpublish&status=init&vmfile=no&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstaltung.veranstid=%s",
		TreeRootURL: qis + "?state=modulBeschrGast&moduleParameter=modDescr&next=tree.vm&nextdir=qispos/modulBeschr/gast&nodeID=auswahlBaum" +
			"&navigationPosition=modules%2CmodulBeschrGast&breadcrumb=modDescrViewOnly2&topitem=modules&subitem=modulBeschrGast&asi=",
	}
}

// CrawlLists archives the module catalog list and the FÜS list (one request each).
func CrawlLists(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	jobs := []crawl.Job{
		{Source: catalogdb.SourceModuleCatalog, Key: "list", URL: ep.CatalogURL},
		{Source: catalogdb.SourceQISFUESList, Key: "list", URL: ep.FUESURL},
	}
	return crawl.Run(ctx, db, jobs, crawl.Options{Workers: 1, Delay: pace.Delay, MaxAge: pace.MaxAge, Backoff: pace.Backoff})
}

// CrawlModules archives the module pages of every module the archived lists name.
func CrawlModules(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	ids, err := ModuleIDs(db)
	if err != nil {
		return crawl.Stats{}, err
	}
	jobs := make([]crawl.Job, 0, len(ids))
	for _, id := range ids {
		jobs = append(jobs, crawl.Job{Source: catalogdb.SourceModulePage, Key: id, URL: fmt.Sprintf(ep.ModuleURL, id)})
	}
	return runOldestFirst(ctx, db, jobs, pace)
}

// CrawlEvents archives the QIS pages of the events that module pages list for the
// current semester. Events removed by retention are not fetched again.
func CrawlEvents(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	ids, err := LinkedEventIDs(db)
	if err != nil {
		return crawl.Stats{}, err
	}
	tombstones, err := db.EventTombstones()
	if err != nil {
		return crawl.Stats{}, err
	}
	jobs := make([]crawl.Job, 0, len(ids))
	for _, id := range ids {
		if !tombstones[id] {
			jobs = append(jobs, crawl.Job{Source: catalogdb.SourceQISEvent, Key: id, URL: fmt.Sprintf(ep.EventURL, id)})
		}
	}
	return runOldestFirst(ctx, db, jobs, pace)
}

// CrawlTree walks the QIS program tree from its root. Pages that are fresh in the
// archive are read from there, so a run only requests what is missing or stale,
// and it discovers programs and PO versions that did not exist before.
func CrawlTree(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	log := oplog.For("crawl").With("source", catalogdb.SourceQISTree)
	start := time.Now()
	log.Info("tree crawl started", "event", "crawl.started", "delay_ms", pace.Delay.Milliseconds(), "max_age", pace.MaxAge.String(), "limit", pace.Limit)

	fetcher := crawl.NewFetcher(db, crawl.Options{Delay: pace.Delay, MaxAge: pace.MaxAge, Backoff: pace.Backoff})
	errLimit := errors.New("page limit reached")

	result, err := qistree.Walk(ctx, ep.TreeRootURL,
		func(ctx context.Context, pageURL string) ([]byte, error) {
			if s := fetcher.Stats(); pace.Limit > 0 && s.Fetched+s.NotFound+s.Failed >= pace.Limit {
				return nil, errLimit
			}
			body, err := fetcher.Get(ctx, crawl.Job{Source: catalogdb.SourceQISTree, Key: pageURL, URL: pageURL})
			switch {
			case err == nil:
				return body, nil
			case errors.Is(err, crawl.ErrServerUnhealthy), ctx.Err() != nil:
				return nil, err
			default:
				return nil, qistree.ErrPageMissing // skip this subtree, go on with the rest
			}
		},
		func(qistree.Page) error { return nil })

	stats := fetcher.Stats()
	if errors.Is(err, errLimit) {
		err = nil
	}
	log.Info("tree crawl finished", "event", "crawl.finished", "pages_walked", result.Pages, "fetched", stats.Fetched,
		"changed", stats.Changed, "from_archive", stats.Skipped, "not_found", stats.NotFound, "failed", stats.Failed,
		"unreachable", len(result.Missing), "duration_s", int(time.Since(start).Seconds()), "aborted", err != nil)
	return stats, err
}

// runOldestFirst orders jobs by the age of their archived page (never fetched
// first), drops the fresh ones, applies the limit, and crawls the rest.
func runOldestFirst(ctx context.Context, db *catalogdb.DB, jobs []crawl.Job, pace Pace) (crawl.Stats, error) {
	if len(jobs) == 0 {
		return crawl.Stats{}, nil
	}
	fetched, err := db.FetchTimes(jobs[0].Source)
	if err != nil {
		return crawl.Stats{}, err
	}

	var due []crawl.Job
	fresh := 0
	now := time.Now()
	for _, j := range jobs {
		if at, ok := fetched[j.Key]; ok && pace.MaxAge > 0 && now.Sub(at) < pace.MaxAge {
			fresh++
			continue
		}
		due = append(due, j)
	}
	sort.SliceStable(due, func(a, b int) bool { return fetched[due[a].Key].Before(fetched[due[b].Key]) })
	if pace.Limit > 0 && len(due) > pace.Limit {
		due = due[:pace.Limit]
	}

	if len(due) == 0 {
		oplog.For("crawl").Info("nothing to fetch", "event", "crawl.up_to_date", "source", jobs[0].Source, "pages", len(jobs))
		return crawl.Stats{Skipped: fresh}, nil
	}
	stats, err := crawl.Run(ctx, db, due, crawl.Options{Workers: pace.Workers, Delay: pace.Delay, Backoff: pace.Backoff})
	stats.Skipped += fresh
	return stats, err
}

// ModuleIDs is the union of the module IDs on the archived catalog list and FÜS list.
func ModuleIDs(db *catalogdb.DB) ([]string, error) {
	seen := make(map[string]bool)

	list, err := db.GetPage(catalogdb.SourceModuleCatalog, "list")
	if err != nil {
		return nil, fmt.Errorf("the module catalog list is not archived: %w", err)
	}
	summaries, err := parser.NewCatalogParser("").Parse(bytes.NewReader(list.Body))
	if err != nil {
		return nil, err
	}
	for _, s := range summaries {
		seen[s.ID] = true
	}
	if len(seen) == 0 {
		return nil, fmt.Errorf("the module catalog list names no modules; the page layout may have changed")
	}

	if fues, err := db.GetPage(catalogdb.SourceQISFUESList, "list"); err == nil {
		modules, err := parser.NewFUESParser().Parse(bytes.NewReader(fues.Body))
		if err != nil {
			return nil, err
		}
		for _, m := range modules {
			seen[m.ID] = true
		}
	} else if !errors.Is(err, catalogdb.ErrNotFound) {
		return nil, err
	}

	ids := make([]string, 0, len(seen))
	for id := range seen {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	return ids, nil
}

// LinkedEventIDs are the QIS event IDs that archived module pages link.
func LinkedEventIDs(db *catalogdb.DB) ([]string, error) {
	seen := make(map[string]bool)
	detailParser := parser.NewDetailParser()
	err := db.EachPage(catalogdb.SourceModulePage, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		d, err := detailParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("module page %s: %w", p.Key, err)
		}
		for _, link := range d.CurrentSemesterEvents {
			if id := eventIDFromURL(link.URL); id != "" {
				seen[id] = true
			}
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	ids := make([]string, 0, len(seen))
	for id := range seen {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	return ids, nil
}

func eventIDFromURL(raw string) string {
	u, err := url.Parse(raw)
	if err != nil {
		return ""
	}
	if v := u.Query().Get("veranstaltung.veranstid"); v != "" {
		return v
	}
	return u.Query().Get("veranstid")
}
