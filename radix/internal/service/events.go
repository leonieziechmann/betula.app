package service

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/http"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/crawl"
	"github.com/leonieziechmann/betula/radix/internal/model"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/parser"
)

// The dates of an event come from two readings of QIS: its entry in the event search,
// which states the dates of 250 events in one request, and its own page, which adds what
// the search leaves out, the remarks of the dates above all (docs/radix/data-sources.md §11).
// The search is asked every night about every linked event, and every two hours about the
// events whose dates are not settled; a page is fetched when the search has news for it,
// and otherwise once per its period.

// EventListPace is how often the event search is asked about the linked events.
type EventListPace struct {
	Pace                          // Delay between requests; MaxAge of an entry in the off-peak window; Limit of events per run
	UnsettledMaxAge time.Duration // an event with unsettled dates (parser.Unsettled) or one the search does not show is asked about again after this long, at any hour
}

// EventPagePace is how often the page of an event is fetched.
type EventPagePace struct {
	Pace                          // MaxAge: a page in doubt, one the event search does not confirm
	ConfirmedMaxAge time.Duration // a page the event search vouches for: it states the same dates, and they are settled; 0 vouches for nothing
	UnsettledMaxAge time.Duration // a page the event search confirms while its dates are not settled (parser.Unsettled); 0 leaves it in doubt
	EntryFresh      time.Duration // an entry speaks for its event only while it was read within this long
	DayLimit        int           // outside the off-peak window only pages the search has news for are fetched, at most this many per run
}

// eventListBatch is how many events one request of the event search asks for. 250 IDs
// make a URL of about 2 KB and an answer of about 1 MB; in September 2026 QIS answered a
// request for 300 in 2.2 s, no slower than a single event page.
const eventListBatch = 250

// eventState is what the archive holds of an event: its entry in the event search and its page.
type eventState struct {
	entry, page        *model.EventDetail // nil when not archived (or, for the entry, not shown by the search)
	entryAt, pageAt    time.Time          // fetched
	entryChangedAt     time.Time          // when the entry last read differently
	entryListed, paged bool               // archived with a body
	entryAsked         bool               // asked for, whether the search showed the event or not
}

func (st *eventState) readEntry(p *catalogdb.RawPage, listParser *parser.EventListParser) error {
	st.entryAsked, st.entryAt, st.entryChangedAt = true, p.FetchedAt, p.ChangedAt
	if p.HTTPStatus != http.StatusOK || len(p.Body) == 0 {
		return nil
	}
	d, err := listParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
	if err != nil {
		return fmt.Errorf("event entry %s: %w", p.Key, err)
	}
	st.entry, st.entryListed = d, true
	return nil
}

func (st *eventState) readPage(p *catalogdb.RawPage, pageParser *parser.EventParser) error {
	st.pageAt = p.FetchedAt
	if p.HTTPStatus != http.StatusOK || len(p.Body) == 0 {
		return nil
	}
	d, err := pageParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
	if err != nil {
		return fmt.Errorf("event page %s: %w", p.Key, err)
	}
	st.page, st.paged = d, true
	return nil
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
		return state(p.Key).readEntry(p, listParser)
	})
	if err != nil || !pages {
		return states, err
	}
	pageParser := parser.NewEventParser()
	err = db.EachPage(catalogdb.SourceQISEvent, func(p *catalogdb.RawPage) error {
		return state(p.Key).readPage(p, pageParser)
	})
	return states, err
}

// readEventState reads the entry and the page of one event.
func readEventState(db *catalogdb.DB, id string) (*eventState, error) {
	st := &eventState{}
	if p, err := db.GetPage(catalogdb.SourceQISEventEntry, id); err == nil {
		if err := st.readEntry(p, parser.NewEventListParser()); err != nil {
			return nil, err
		}
	} else if err != catalogdb.ErrNotFound {
		return nil, err
	}
	if p, err := db.GetPage(catalogdb.SourceQISEvent, id); err == nil {
		if err := st.readPage(p, parser.NewEventParser()); err != nil {
			return nil, err
		}
	} else if err != catalogdb.ErrNotFound {
		return nil, err
	}
	return st, nil
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
// archived yet) every linked event is due whose entry is older than pace.MaxAge. At any
// hour an event is due whose entry is older than pace.UnsettledMaxAge while its dates are
// not settled (parser.Unsettled: none yet, a placeholder, or dates that look wrong) or the
// search did not show it: asking again costs a share of one request, and in doubt the
// answer is the same. An event the search does not show is archived as 404; its page stays
// its only source.
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
	unsettledDue := func(st *eventState) bool {
		doubtful := (st.entryListed && parser.Unsettled(st.entry)) || (st.entryAsked && !st.entryListed)
		return doubtful && now.Sub(st.entryAt) >= pace.UnsettledMaxAge
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
			if st == nil || !st.entryAsked || pace.MaxAge <= 0 || now.Sub(st.entryAt) >= pace.MaxAge || unsettledDue(st) {
				due = append(due, id)
			} else {
				stats.Skipped++
			}
		}
	} else {
		var candidates []string
		for id, st := range states {
			if !tombstones[id] && unsettledDue(st) {
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
		var ta, tb time.Time
		if st := states[due[a]]; st != nil {
			ta = st.entryAt
		}
		if st := states[due[b]]; st != nil {
			tb = st.entryAt
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
	fetcher := crawl.NewFetcher(db, crawl.Options{Delay: pace.Delay, Backoff: pace.Backoff, Client: ep.Client})
	var notListed []string
	for from := 0; from < len(due); from += eventListBatch {
		batch := due[from:min(from+eventListBatch, len(due))]
		job := crawl.Job{
			Source: catalogdb.SourceQISEventEntry,
			Key:    fmt.Sprintf("search:%s..%s", batch[0], batch[len(batch)-1]),
			URL:    fmt.Sprintf(ep.EventListURL, strings.Join(batch, ","), len(batch)),
		}
		// fetchedAt is when QIS gave the answer: through Cortex, possibly before this request.
		body, fetchedAt, err := fetcher.Download(ctx, job)
		if errors.Is(err, crawl.ErrOfflineMiss) {
			// Offline through Cortex, which has not stored this search: its events wait for the
			// next cycle, their entries stay as archived.
			stats.OfflineMiss++
			stats.Skipped += len(batch)
			continue
		}
		if err != nil {
			stats.Failed = fetcher.Stats().Failed
			return stats, err
		}
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
			outcome := "unchanged"
			if changed {
				stats.Changed++
				outcome = "changed"
			}
			crawl.CountPage(catalogdb.SourceQISEventEntry, outcome)
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
			crawl.CountPage(catalogdb.SourceQISEventEntry, "not_found")
			notListed = append(notListed, id)
		}
	}
	if len(notListed) > 0 {
		log.Warn("the event search does not show events that module pages link; their pages stay their only source",
			"event", "crawl.not_listed", "count", len(notListed), "examples", firstIDs(notListed, 5))
	}
	log.Info("event list finished", "event", "crawl.finished", "events", len(due), "requests", requests,
		"listed", stats.Fetched, "changed", stats.Changed, "not_listed", stats.NotFound, "skipped", stats.Skipped,
		"offline_miss", stats.OfflineMiss, "duration_s", int(time.Since(start).Seconds()))
	return stats, nil
}

func firstIDs(ids []string, n int) []string {
	if len(ids) > n {
		return ids[:n]
	}
	return ids
}

// Why the page of an event is due; pageNotDue when it is not.
const (
	pageNotDue     = -1
	pageNew        = 0 // never fetched
	pageListNews   = 1 // the search states other dates, and changed after the page was fetched
	pagePastItsAge = 2
)

// pageRank says whether the page of event id is due, and whether the search vouches for it.
// The search has news for a page when its entry changed after the page was fetched and now
// states other dates, or no longer shows the event: a change, not a difference in reading,
// so that no page is fetched again and again for the same entry. An event BTU removes drops
// out of the search, and its page, the empty frame of QIS then, takes it out of the catalog
// in the same cycle (docs/radix/data-sources.md §11). The search vouches for a page that states
// the same dates when they are settled: the page is read once per pace.ConfirmedMaxAge. A
// page that states the same dates while they are not settled is read once per
// pace.UnsettledMaxAge: the search, asked about those dates every two hours, shows when they
// come, and the page is read for what it states alone, a remark such as „Termin nach
// Vereinbarung". Any other page is in doubt and is read once per pace.MaxAge, as every page
// was before the search was asked (a search that stopped working vouches for nothing). With
// pace.Spread each page has a day of its own in its period, so that pages read in the same
// night do not come due together again.
func pageRank(id string, st *eventState, pace EventPagePace, now time.Time) (rank int, vouched bool) {
	asked := st.entryAsked && pace.EntryFresh > 0 && now.Sub(st.entryAt) < pace.EntryFresh
	listed := asked && st.entryListed
	agrees := listed && st.paged && parser.SameSchedule(st.entry, st.page)
	due := func(period time.Duration) int {
		if pace.due(id, st.pageAt, now, period) {
			return pagePastItsAge
		}
		return pageNotDue
	}
	switch {
	case st.pageAt.IsZero():
		return pageNew, false
	case asked && !agrees && st.entryChangedAt.After(st.pageAt):
		return pageListNews, false
	case agrees && pace.ConfirmedMaxAge > 0 && !parser.Unsettled(st.entry):
		return due(pace.ConfirmedMaxAge), true
	case agrees && pace.UnsettledMaxAge > 0 && parser.Unsettled(st.entry):
		return due(pace.UnsettledMaxAge), false
	}
	return due(pace.MaxAge), false
}

// CrawlEvents archives the QIS pages of the events that module pages link. With all (the
// off-peak window, or nothing archived yet), every page that is due, in the order of
// pageRank: pages never fetched, pages the search has news for, pages whose day in their
// period has come, oldest first. At other hours only the first two, at most pace.DayLimit,
// so that a change the search shows by day reaches the catalog with its remarks within the
// cycle. Events removed by retention are not fetched again.
//
// Log events: crawl.pages_due, crawl.up_to_date, and those of crawl.Run.
func CrawlEvents(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace EventPagePace, all bool) (crawl.Stats, error) {
	log := oplog.For("crawl").With("source", catalogdb.SourceQISEvent)
	tombstones, err := db.EventTombstones()
	if err != nil {
		return crawl.Stats{}, err
	}
	now := time.Now()

	var ids []string
	states := make(map[string]*eventState)
	if all {
		if ids, err = LinkedEventIDs(db); err != nil {
			return crawl.Stats{}, err
		}
		if states, err = readEventStates(db, true); err != nil {
			return crawl.Stats{}, err
		}
	} else {
		// By day only an entry that changed since its page was fetched, or one without a
		// page, can make a page due; the archive says which without reading a body. That
		// the search no longer shows an event is such a change.
		entries, err := db.PageStates(catalogdb.SourceQISEventEntry)
		if err != nil {
			return crawl.Stats{}, err
		}
		pages, err := db.PageStates(catalogdb.SourceQISEvent)
		if err != nil {
			return crawl.Stats{}, err
		}
		for id, e := range entries {
			if pace.EntryFresh <= 0 || now.Sub(e.FetchedAt) >= pace.EntryFresh {
				continue
			}
			if p, ok := pages[id]; !ok || e.ChangedAt.After(p.FetchedAt) {
				ids = append(ids, id)
			}
		}
		sort.Strings(ids)
		for _, id := range ids {
			st, err := readEventState(db, id)
			if err != nil {
				return crawl.Stats{}, err
			}
			states[id] = st
		}
	}

	type dueJob struct {
		job  crawl.Job
		rank int
		at   time.Time
	}
	var due []dueJob
	var counts [3]int
	notDue, vouched := 0, 0
	for _, id := range ids {
		if tombstones[id] {
			continue
		}
		st := states[id]
		if st == nil {
			st = &eventState{}
		}
		rank, v := pageRank(id, st, pace, now)
		if v {
			vouched++
		}
		if rank == pageNotDue || (!all && rank == pagePastItsAge) {
			notDue++
			continue
		}
		counts[rank]++
		due = append(due, dueJob{job: crawl.Job{Source: catalogdb.SourceQISEvent, Key: id, URL: fmt.Sprintf(ep.EventURL, id)}, rank: rank, at: st.pageAt})
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
	limit := pace.Limit
	if !all {
		limit = pace.DayLimit
	}
	if limit > 0 && len(due) > limit {
		notDue += len(due) - limit
		due = due[:limit]
	}
	if len(due) == 0 {
		log.Info("nothing to fetch", "event", "crawl.up_to_date", "pages", notDue, "all", all)
		return crawl.Stats{Skipped: notDue}, nil
	}
	log.Info("event pages due", "event", "crawl.pages_due", "all", all, "never_fetched", counts[pageNew], "changed_in_list", counts[pageListNews],
		"past_age", counts[pagePastItsAge], "vouched_by_list", vouched, "fetching", len(due))
	jobs := make([]crawl.Job, len(due))
	for i, d := range due {
		jobs[i] = d.job
	}
	stats, err := crawl.Run(ctx, db, jobs, crawl.Options{Workers: pace.Workers, Delay: pace.Delay, Backoff: pace.Backoff, Client: ep.Client})
	stats.Skipped += notDue
	return stats, err
}
