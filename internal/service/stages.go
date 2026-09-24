// Package service runs Radix as a long-lived process: it keeps the raw
// archive fresh at a polite pace, rebuilds the catalog, and publishes a new
// snapshot when the data changed. The stages are also used one by one by the CLI.
package service

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/crawl"
	"github.com/leonieziechmann/betula/internal/model"
	"github.com/leonieziechmann/betula/internal/normalize"
	"github.com/leonieziechmann/betula/internal/oplog"
	"github.com/leonieziechmann/betula/internal/parser"
	"github.com/leonieziechmann/betula/internal/qistree"
)

// Pace is how fast and how much one stage may fetch.
type Pace struct {
	Workers int
	Delay   time.Duration
	MaxAge  time.Duration // pages archived more recently are not fetched again
	Limit   int           // at most this many pages are requested per run, oldest first; 0 = no limit
	Backoff time.Duration // first pause after a failed request (default 30 s)
}

// EventListPace is how often the event search is asked about the linked events.
type EventListPace struct {
	Pace                            // Delay between requests; MaxAge of an entry in the off-peak window; Limit of events per run
	PlaceholderMaxAge time.Duration // an event that does not say yet when it takes place is asked about again after this long, at any hour
}

// EventPagePace is how often the page of an event is fetched again.
type EventPagePace struct {
	Pace                          // MaxAge: a page the event search does not confirm
	ConfirmedMaxAge time.Duration // a page whose dates the event search confirms; 0 confirms nothing
	EntryFresh      time.Duration // an entry confirms a page only while it was read within this long
}

// Endpoints are the pages the service reads. Tests point them at a local server.
type Endpoints struct {
	CatalogURL    string
	FUESURL       string
	ModuleURL     string // fmt template with the module ID
	QISModuleList string // fmt template with the first row and the number of rows
	QISModuleURL  string // fmt template: %[1]s the QIS pordnr, %[2]s the view language
	EventURL      string // fmt template with the QIS event ID
	EventListURL  string // fmt template: %[1]s comma-separated QIS event IDs, %[2]d their number; "" skips the event search
	TreeRootURL   string
}

// BTUEndpoints are the live BTU pages.
func BTUEndpoints() Endpoints {
	const qis = "https://www.b-tu.de/qisserver3/rds"
	const moduleTable = qis + "?state=change&type=3&moduleParameter=pordpos&nextdir=change&next=TableSelectModul.vm&subdir=pord"
	return Endpoints{
		CatalogURL:    "https://www.b-tu.de/modul",
		ModuleURL:     "https://www.b-tu.de/modul/%s",
		FUESURL:       moduleTable + "&P_start=0&P_anzahl=9999&missing=FUES",
		QISModuleList: moduleTable + "&P_start=%d&P_anzahl=%d",
		// The module description in QIS. nodeID is not decoration: without it the answer
		// leaves out the events of the current semester. objLanguage picks the language
		// the description is written in, not a translation: the German view of a module
		// taught in English states „keine" for its learning outcomes and contents.
		QISModuleURL: qis + "?state=modulBeschrDetailInfo&moduleParameter=modDescr&struct=auswahlBaum" +
			"&nextdir=qispos/modulBeschr/bearbeiter&next=redTree.vm&createInfoTree=Y&create=blobs&expand=1" +
			"&nodeID=auswahlBaum%%7Cmodul:pordnr=%[1]s&rest=A&objLanguage=%[2]s&pord.pordnr=%[1]s",
		EventURL: qis + "?state=verpublish&status=init&vmfile=no&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstaltung.veranstid=%s",
		// The event search in its long view, which prints the dates of every event. It
		// takes a list of event IDs across semesters and answers with those events only.
		EventListURL: qis + "?state=wsearchv&search=1&veranstaltung.veranstid=%[1]s&P_start=0&P_anzahl=%[2]d&P.sort=veranstaltung.veranstnr&P.vx=lang",
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

// qisModuleListChunk is how many rows one request of the QIS module table asks for.
// The whole table in one response is 27 MB, more than the crawler keeps of a single
// page; 1,000 rows are about 8 MB.
const qisModuleListChunk = 1000

// maxQISModuleListChunks bounds the loop if the table never returns a short chunk.
const maxQISModuleListChunks = 50

// CrawlQISModuleList archives the QIS module table in chunks, until a chunk is
// shorter than a full one. Chunks behind the end of a table that shrank are
// removed, so that they cannot keep naming modules the table no longer has.
func CrawlQISModuleList(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	fetcher := crawl.NewFetcher(db, crawl.Options{Delay: pace.Delay, MaxAge: pace.MaxAge, Backoff: pace.Backoff})
	rowParser := parser.NewFUESParser()

	for i := 0; i < maxQISModuleListChunks; i++ {
		start := i * qisModuleListChunk
		job := crawl.Job{
			Source: catalogdb.SourceQISModuleList,
			Key:    qisModuleListKey(start),
			URL:    fmt.Sprintf(ep.QISModuleList, start, qisModuleListChunk),
		}
		body, err := fetcher.Get(ctx, job)
		if err != nil {
			return fetcher.Stats(), err
		}
		rows, err := rowParser.Parse(bytes.NewReader(body))
		if err != nil {
			return fetcher.Stats(), fmt.Errorf("QIS module table at %d: %w", start, err)
		}
		if len(rows) < qisModuleListChunk {
			return fetcher.Stats(), dropQISListChunksAfter(db, start)
		}
	}
	return fetcher.Stats(), fmt.Errorf("the QIS module table has more than %d rows; no chunk ended it", maxQISModuleListChunks*qisModuleListChunk)
}

func qisModuleListKey(start int) string { return fmt.Sprintf("rows-%06d", start) }

// dropQISListChunksAfter removes archived chunks that start after the last one read.
func dropQISListChunksAfter(db *catalogdb.DB, lastStart int) error {
	fetched, err := db.FetchTimes(catalogdb.SourceQISModuleList)
	if err != nil {
		return err
	}
	var stale []string
	for key := range fetched {
		var start int
		if _, err := fmt.Sscanf(key, "rows-%d", &start); err != nil || start <= lastStart {
			// A key of another shape is not ours to judge; "list" is the key of the
			// single-page attempt this chunking replaced.
			if key == "list" {
				stale = append(stale, key)
			}
			continue
		}
		stale = append(stale, key)
	}
	if len(stale) == 0 {
		return nil
	}
	n, err := db.PruneArchive(map[string][]string{catalogdb.SourceQISModuleList: stale}, time.Time{})
	if err != nil {
		return err
	}
	oplog.For("crawl").Info("removed module table chunks behind the end", "event", "crawl.list_chunks_dropped",
		"source", catalogdb.SourceQISModuleList, "removed", n)
	return nil
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

// CrawlQISModules archives the QIS module description of every module the archived
// QIS module table names. QIS is where the catalog is maintained, so its page carries
// the events of the current semester weeks before the CMS copy on b-tu.de does.
func CrawlQISModules(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	refs, err := QISModuleRefs(db)
	if err != nil {
		return crawl.Stats{}, err
	}
	jobs := make([]crawl.Job, 0, len(refs))
	for _, r := range refs {
		jobs = append(jobs, crawl.Job{
			Source: catalogdb.SourceQISModulePage,
			Key:    r.ModuleID,
			URL:    fmt.Sprintf(ep.QISModuleURL, r.Pordnr, r.Language),
		})
	}
	return runOldestFirst(ctx, db, jobs, pace)
}

// eventListBatch is how many events one request of the event search asks for. 250 IDs
// make a URL of about 2 KB and an answer of about 1 MB; in September 2026 QIS answered a
// request for 300 in 2.2 s, no slower than a single event page.
const eventListBatch = 250

// eventState is what the archive holds of an event: its entry in the event search and its page.
type eventState struct {
	entry, page        *model.EventDetail // nil when not archived (or, for the entry, not shown by the search)
	entryAt, pageAt    time.Time
	entryListed, paged bool // archived with a body
	entryAsked         bool // asked for, whether the search showed the event or not
}

// readEventStates parses the archived entries and, with pages, the archived event pages.
func readEventStates(db *catalogdb.DB, pages bool) (map[string]*eventState, error) {
	states := make(map[string]*eventState)
	state := func(id string) *eventState {
		if states[id] == nil {
			states[id] = &eventState{}
		}
		return states[id]
	}
	listParser := parser.NewEventListParser()
	err := db.EachPage(catalogdb.SourceQISEventEntry, func(p *catalogdb.RawPage) error {
		st := state(p.Key)
		st.entryAsked, st.entryAt = true, p.FetchedAt
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		d, err := listParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("event entry %s: %w", p.Key, err)
		}
		st.entry, st.entryListed = d, true
		return nil
	})
	if err != nil || !pages {
		return states, err
	}
	pageParser := parser.NewEventParser()
	err = db.EachPage(catalogdb.SourceQISEvent, func(p *catalogdb.RawPage) error {
		st := state(p.Key)
		st.pageAt = p.FetchedAt
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		d, err := pageParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("event page %s: %w", p.Key, err)
		}
		st.page, st.paged = d, true
		return nil
	})
	return states, err
}

func isEventID(id string) bool {
	if id == "" {
		return false
	}
	for _, r := range id {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

// CrawlEventList looks the events the module pages link up in the QIS event search, 250
// to a request, and archives the entry of each event on its own (source qis_event_entry,
// under the address of the event's page). With all (the off-peak window, or nothing
// archived yet) every linked event is due whose entry is older than pace.MaxAge; at other
// hours only the events that do not say yet when they take place (parser.AwaitsDates),
// once their entry is older than pace.PlaceholderMaxAge. An event the search does not
// show is archived as 404; its page stays its only source.
//
// The answer must be the events asked for and nothing else: a page that is not a result
// of the search, or that shows other or more events, stops the stage before anything is
// archived from it.
//
// Log events: crawl.started, crawl.finished, crawl.up_to_date, crawl.not_listed (WARN).
func CrawlEventList(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace EventListPace, all bool) (crawl.Stats, error) {
	log := oplog.For("crawl").With("source", catalogdb.SourceQISEventEntry)
	if ep.EventListURL == "" {
		return crawl.Stats{}, nil
	}
	states, err := readEventStates(db, false)
	if err != nil {
		return crawl.Stats{}, err
	}
	tombstones, err := db.EventTombstones()
	if err != nil {
		return crawl.Stats{}, err
	}
	now := time.Now()
	placeholderDue := func(st *eventState) bool {
		return st.entryListed && parser.AwaitsDates(st.entry) && now.Sub(st.entryAt) >= pace.PlaceholderMaxAge
	}

	var due []string
	var stats crawl.Stats
	if all {
		ids, err := LinkedEventIDs(db)
		if err != nil {
			return crawl.Stats{}, err
		}
		for _, id := range ids {
			if tombstones[id] || !isEventID(id) {
				continue
			}
			st := states[id]
			if st == nil || !st.entryAsked || pace.MaxAge <= 0 || now.Sub(st.entryAt) >= pace.MaxAge || placeholderDue(st) {
				due = append(due, id)
			} else {
				stats.Skipped++
			}
		}
	} else {
		var candidates []string
		for id, st := range states {
			if !tombstones[id] && placeholderDue(st) {
				candidates = append(candidates, id)
			}
		}
		if len(candidates) > 0 {
			// Only what a module page still links: an event nobody links has to age
			// until retention removes it.
			ids, err := LinkedEventIDs(db)
			if err != nil {
				return crawl.Stats{}, err
			}
			linked := make(map[string]bool, len(ids))
			for _, id := range ids {
				linked[id] = true
			}
			for _, id := range candidates {
				if linked[id] {
					due = append(due, id)
				}
			}
		}
	}
	if len(due) == 0 {
		log.Info("nothing to look up", "event", "crawl.up_to_date", "events", stats.Skipped, "all", all)
		return stats, nil
	}
	sort.Slice(due, func(a, b int) bool {
		sa, sb := states[due[a]], states[due[b]]
		var ta, tb time.Time
		if sa != nil {
			ta = sa.entryAt
		}
		if sb != nil {
			tb = sb.entryAt
		}
		if !ta.Equal(tb) {
			return ta.Before(tb)
		}
		return due[a] < due[b]
	})
	if pace.Limit > 0 && len(due) > pace.Limit {
		stats.Skipped += len(due) - pace.Limit
		due = due[:pace.Limit]
	}

	start := time.Now()
	requests := (len(due) + eventListBatch - 1) / eventListBatch
	log.Info("event list started", "event", "crawl.started", "events", len(due), "requests", requests, "all", all, "delay_ms", pace.Delay.Milliseconds())
	fetcher := crawl.NewFetcher(db, crawl.Options{Delay: pace.Delay, Backoff: pace.Backoff})
	var notListed []string
	for from := 0; from < len(due); from += eventListBatch {
		batch := due[from:min(from+eventListBatch, len(due))]
		job := crawl.Job{
			Source: catalogdb.SourceQISEventEntry,
			Key:    fmt.Sprintf("search:%s..%s", batch[0], batch[len(batch)-1]),
			URL:    fmt.Sprintf(ep.EventListURL, strings.Join(batch, ","), len(batch)),
		}
		body, err := fetcher.Download(ctx, job)
		if err != nil {
			stats.Failed = fetcher.Stats().Failed
			return stats, err
		}
		fetchedAt := time.Now()
		list, err := parser.SplitEventList(bytes.NewReader(body))
		if err != nil {
			return stats, fmt.Errorf("event search for %d events from %s: %w", len(batch), batch[0], err)
		}
		asked := make(map[string]bool, len(batch))
		for _, id := range batch {
			asked[id] = true
		}
		if list.Hits != len(list.Entries) {
			return stats, fmt.Errorf("event search for %d events from %s: the page states %d hits but shows %d; it did not answer the list of IDs", len(batch), batch[0], list.Hits, len(list.Entries))
		}
		shown := make(map[string]bool, len(list.Entries))
		for _, e := range list.Entries {
			if !asked[e.ID] || shown[e.ID] {
				return stats, fmt.Errorf("event search for %d events from %s: it shows event %s, which was not asked for or is shown twice; it did not answer the list of IDs", len(batch), batch[0], e.ID)
			}
			shown[e.ID] = true
		}

		for _, e := range list.Entries {
			changed, err := db.PutPageChanged(catalogdb.RawPage{Source: catalogdb.SourceQISEventEntry, Key: e.ID, URL: fmt.Sprintf(ep.EventURL, e.ID),
				FetchedAt: fetchedAt, HTTPStatus: http.StatusOK, Body: e.HTML})
			if err != nil {
				return stats, fmt.Errorf("failed to archive the entry of event %s: %w", e.ID, err)
			}
			stats.Fetched++
			if changed {
				stats.Changed++
			}
		}
		for _, id := range batch {
			if shown[id] {
				continue
			}
			if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceQISEventEntry, Key: id, URL: fmt.Sprintf(ep.EventURL, id),
				FetchedAt: fetchedAt, HTTPStatus: http.StatusNotFound}); err != nil {
				return stats, fmt.Errorf("failed to archive the entry of event %s: %w", id, err)
			}
			stats.NotFound++
			notListed = append(notListed, id)
		}
	}
	if len(notListed) > 0 {
		log.Warn("the event search does not show events that module pages link; their pages stay their only source",
			"event", "crawl.not_listed", "count", len(notListed), "examples", firstIDs(notListed, 5))
	}
	log.Info("event list finished", "event", "crawl.finished", "events", len(due), "requests", requests,
		"listed", stats.Fetched, "changed", stats.Changed, "not_listed", stats.NotFound, "skipped", stats.Skipped,
		"duration_s", int(time.Since(start).Seconds()))
	return stats, nil
}

func firstIDs(ids []string, n int) []string {
	if len(ids) > n {
		return ids[:n]
	}
	return ids
}

// CrawlEvents archives the QIS pages of the events that module pages link. The event
// search states their dates every night, so a page is fetched when the list has news for
// it: pages never fetched first, then pages whose dates the list states otherwise, then
// pages past their age, oldest first. A page the list confirms (parser.SameSchedule)
// ages by pace.ConfirmedMaxAge, for what only the page states, the remarks of its dates;
// a page it does not, because the search does not show the event or its entry was not
// read within pace.EntryFresh, ages by pace.MaxAge, as every page did before the list.
// Events removed by retention are not fetched again.
func CrawlEvents(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace EventPagePace) (crawl.Stats, error) {
	log := oplog.For("crawl").With("source", catalogdb.SourceQISEvent)
	ids, err := LinkedEventIDs(db)
	if err != nil {
		return crawl.Stats{}, err
	}
	tombstones, err := db.EventTombstones()
	if err != nil {
		return crawl.Stats{}, err
	}
	states, err := readEventStates(db, true)
	if err != nil {
		return crawl.Stats{}, err
	}

	type dueJob struct {
		job  crawl.Job
		rank int // 0 never fetched, 1 the list states other dates, 2 past its age
		at   time.Time
	}
	var due []dueJob
	fresh, confirmed, ranks := 0, 0, [3]int{}
	now := time.Now()
	for _, id := range ids {
		if tombstones[id] {
			continue
		}
		st := states[id]
		if st == nil {
			st = &eventState{}
		}
		job := crawl.Job{Source: catalogdb.SourceQISEvent, Key: id, URL: fmt.Sprintf(ep.EventURL, id)}
		listed := st.entryListed && pace.EntryFresh > 0 && now.Sub(st.entryAt) < pace.EntryFresh
		rank := -1
		switch {
		case st.pageAt.IsZero():
			rank = 0
		case listed && st.paged && parser.SameSchedule(st.entry, st.page):
			confirmed++
			if pace.ConfirmedMaxAge <= 0 || now.Sub(st.pageAt) >= pace.ConfirmedMaxAge {
				rank = 2
			}
		case listed && st.entryAt.After(st.pageAt):
			rank = 1
		case pace.MaxAge <= 0 || now.Sub(st.pageAt) >= pace.MaxAge:
			rank = 2
		}
		if rank < 0 {
			fresh++
			continue
		}
		ranks[rank]++
		due = append(due, dueJob{job: job, rank: rank, at: st.pageAt})
	}
	sort.SliceStable(due, func(a, b int) bool {
		if due[a].rank != due[b].rank {
			return due[a].rank < due[b].rank
		}
		if !due[a].at.Equal(due[b].at) {
			return due[a].at.Before(due[b].at)
		}
		return due[a].job.Key < due[b].job.Key
	})
	if pace.Limit > 0 && len(due) > pace.Limit {
		fresh += len(due) - pace.Limit
		due = due[:pace.Limit]
	}
	log.Info("event pages due", "event", "crawl.pages_due", "never_fetched", ranks[0], "changed_in_list", ranks[1],
		"past_age", ranks[2], "confirmed_by_list", confirmed, "fetching", len(due))
	if len(due) == 0 {
		log.Info("nothing to fetch", "event", "crawl.up_to_date", "pages", fresh)
		return crawl.Stats{Skipped: fresh}, nil
	}
	jobs := make([]crawl.Job, len(due))
	for i, d := range due {
		jobs[i] = d.job
	}
	stats, err := crawl.Run(ctx, db, jobs, crawl.Options{Workers: pace.Workers, Delay: pace.Delay, Backoff: pace.Backoff})
	stats.Skipped += fresh
	return stats, err
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

// QISModuleRef is one row of the QIS module table: the module number students see,
// the internal number of its description, which addresses the page, and the view to
// ask for — a description is written in the language the module is taught in, and
// the other view leaves its texts empty.
type QISModuleRef struct {
	ModuleID string
	Pordnr   string
	Language string // objLanguage: "de" or "en"
}

// qisViewLanguage picks the view from the teaching language of the module. German
// is the fallback: it is what an unknown or a bilingual module is described in.
func qisViewLanguage(raw string) string {
	german, english := normalize.Languages(raw)
	if english && !german {
		return "en"
	}
	return "de"
}

var rePordnr = regexp.MustCompile(`pord\.pordnr=(\d+)`)

// QISModuleRefs reads the archived chunks of the QIS module table. Rows without a
// link to a description are skipped: without its pordnr the page cannot be addressed.
func QISModuleRefs(db *catalogdb.DB) ([]QISModuleRef, error) {
	rowParser := parser.NewFUESParser()
	seen := make(map[string]bool)
	var refs []QISModuleRef

	err := db.EachPage(catalogdb.SourceQISModuleList, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		rows, err := rowParser.Parse(bytes.NewReader(p.Body))
		if err != nil {
			return fmt.Errorf("QIS module table %s: %w", p.Key, err)
		}
		for _, r := range rows {
			m := rePordnr.FindStringSubmatch(r.QISURL)
			if m == nil || seen[r.ID] {
				continue
			}
			seen[r.ID] = true
			refs = append(refs, QISModuleRef{ModuleID: r.ID, Pordnr: m[1], Language: qisViewLanguage(r.Language)})
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	if len(refs) == 0 {
		return nil, fmt.Errorf("the archived QIS module table names no module descriptions; it may not be archived yet, or the page layout changed")
	}
	sort.Slice(refs, func(a, b int) bool { return refs[a].ModuleID < refs[b].ModuleID })
	return refs, nil
}

// LinkedEventIDs are the QIS event IDs that archived module descriptions link:
// both the QIS page, which names the events of the semester that runs now, and the
// copy on b-tu.de, which can still name the exams of the one that is ending.
func LinkedEventIDs(db *catalogdb.DB) ([]string, error) {
	seen := make(map[string]bool)
	detailParser := parser.NewDetailParser()
	qisParser := parser.NewQISModuleParser()

	read := func(source string, parse func([]byte, string, string) ([]model.ModuleEvent, error)) error {
		return db.EachPage(source, func(p *catalogdb.RawPage) error {
			if p.HTTPStatus != 200 || len(p.Body) == 0 {
				return nil
			}
			links, err := parse(p.Body, p.Key, p.URL)
			if err != nil {
				return fmt.Errorf("%s %s: %w", source, p.Key, err)
			}
			for _, link := range links {
				if id := eventIDFromURL(link.URL); id != "" {
					seen[id] = true
				}
			}
			return nil
		})
	}

	err := read(catalogdb.SourceQISModulePage, func(body []byte, key, pageURL string) ([]model.ModuleEvent, error) {
		d, err := qisParser.Parse(bytes.NewReader(body), key, pageURL)
		if err != nil {
			return nil, err
		}
		return d.CurrentSemesterEvents, nil
	})
	if err != nil {
		return nil, err
	}
	err = read(catalogdb.SourceModulePage, func(body []byte, key, pageURL string) ([]model.ModuleEvent, error) {
		d, err := detailParser.Parse(bytes.NewReader(body), key, pageURL)
		if err != nil {
			return nil, err
		}
		return d.CurrentSemesterEvents, nil
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
