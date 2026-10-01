// Package service runs Radix as a long-lived process: it keeps the raw
// archive fresh at a polite pace, rebuilds the catalog, and publishes a new
// snapshot when the data changed. The stages are also used one by one by the CLI.
package service

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
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
	Spread  bool          // MaxAge is a period instead: every page is fetched once per period, at a time of its own in it (crawl.Due); the stages of single pages honour it, lists are read in one piece
	Limit   int           // at most this many pages are requested per run, oldest first; 0 = no limit
	Backoff time.Duration // first pause after a failed request (default 30 s)
}

// due says whether a page fetched at fetched (zero: never) is due at now when it is to be
// fetched once per period: with Spread at its own time in the period, otherwise as soon
// as it is older.
func (p Pace) due(key string, fetched, now time.Time, period time.Duration) bool {
	if p.Spread {
		return crawl.Due(key, fetched, now, period)
	}
	return period <= 0 || fetched.IsZero() || now.Sub(fetched) >= period
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
//
// It returns the modules whose row changed in this run (title, language, credits, FÜS
// approval, limitation): their description states the same facts, so CrawlQISModules
// fetches it again instead of waiting for its age.
//
// Log events: crawl.module_rows_changed, and those of the fetches.
func CrawlQISModuleList(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, map[string]bool, error) {
	// The rows as they were, read only when a chunk is due: the table is 27 MB.
	var before map[string]string
	fetched, err := db.FetchTimes(catalogdb.SourceQISModuleList)
	if err != nil {
		return crawl.Stats{}, nil, err
	}
	for _, at := range fetched {
		if pace.MaxAge <= 0 || time.Since(at) >= pace.MaxAge {
			if before, err = moduleTableRows(db); err != nil {
				return crawl.Stats{}, nil, err
			}
			break
		}
	}

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
			return fetcher.Stats(), nil, err
		}
		rows, err := rowParser.Parse(bytes.NewReader(body))
		if err != nil {
			return fetcher.Stats(), nil, fmt.Errorf("QIS module table at %d: %w", start, err)
		}
		if len(rows) < qisModuleListChunk {
			if err := dropQISListChunksAfter(db, start); err != nil {
				return fetcher.Stats(), nil, err
			}
			break
		}
		if i == maxQISModuleListChunks-1 {
			return fetcher.Stats(), nil, fmt.Errorf("the QIS module table has more than %d rows; no chunk ended it", maxQISModuleListChunks*qisModuleListChunk)
		}
	}

	changed := make(map[string]bool)
	if before == nil || fetcher.Stats().Fetched == 0 {
		return fetcher.Stats(), changed, nil
	}
	after, err := moduleTableRows(db)
	if err != nil {
		return fetcher.Stats(), nil, err
	}
	var ids []string
	for id, row := range after {
		if old, ok := before[id]; ok && old != row {
			changed[id] = true
			ids = append(ids, id)
		}
	}
	if len(ids) > 0 {
		sort.Strings(ids)
		moduleRowsChanged.Add(float64(len(ids)))
		oplog.For("crawl").Info("module rows changed; their descriptions are fetched again", "event", "crawl.module_rows_changed",
			"source", catalogdb.SourceQISModuleList, "count", len(ids), "examples", firstIDs(ids, 5))
	}
	return fetcher.Stats(), changed, nil
}

// moduleTableRows reads what every row of the archived QIS module table states.
func moduleTableRows(db *catalogdb.DB) (map[string]string, error) {
	rows := make(map[string]string)
	err := db.EachPage(catalogdb.SourceQISModuleList, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		statements, err := parser.TableRowStatements(bytes.NewReader(p.Body))
		if err != nil {
			return fmt.Errorf("QIS module table %s: %w", p.Key, err)
		}
		for id, row := range statements {
			rows[id] = row
		}
		return nil
	})
	return rows, err
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

// ModulePace is how often the QIS description of a module is fetched.
type ModulePace struct {
	Pace                          // MaxAge: a description nothing it depends on has changed for
	UnsettledMaxAge time.Duration // a module offered in the semester the catalog presents whose description names none of its events
}

// Why the description of a module is due.
const (
	moduleNew       = iota // never fetched
	moduleChanged          // its row in the module table changed, or QIS moved on to another semester
	moduleUnsettled        // offered in the semester the catalog presents, and none of its events named yet
	modulePastAge
)

// CrawlQISModules archives the QIS module description of every module the archived
// QIS module table names. QIS is where the catalog is maintained, so its page carries
// the events of the current semester weeks before the CMS copy on b-tu.de does.
//
// A description changes seldom but for the events it names, so it is fetched when
// something it depends on changed after it was read, and otherwise rarely. In this order:
// descriptions never fetched; those whose row in the module table changed (changedRows,
// from CrawlQISModuleList); those read while QIS called another semester current than it
// does now, which name the events of the old one; then the modules offered in the
// semester the catalog presents whose description names none of its events, once per
// pace.UnsettledMaxAge; all others once per pace.MaxAge. Oldest first within each. With
// pace.Spread every description has a day of its own in these periods, so that
// descriptions read in the same night do not come due in the same night again.
//
// Log events: crawl.modules_due, crawl.up_to_date, and those of crawl.Run.
func CrawlQISModules(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace ModulePace, changedRows map[string]bool) (crawl.Stats, error) {
	refs, err := QISModuleRefs(db)
	if err != nil {
		return crawl.Stats{}, err
	}

	// When each description was read, and the semester QIS called current then. The
	// semester QIS calls current now is the one the most recently fetched QIS page names;
	// the module table, read every second night, wins a tie.
	type reading struct {
		at       time.Time
		semester string
	}
	readings := make(map[string]reading)
	var nowAt time.Time
	var nowSemester string
	note := func(at time.Time, semester string) {
		if semester != "" && (nowSemester == "" || at.After(nowAt)) {
			nowAt, nowSemester = at, semester
		}
	}
	err = db.EachPage(catalogdb.SourceQISModuleList, func(p *catalogdb.RawPage) error {
		note(p.FetchedAt, normalize.SemesterKey(parser.QISSemester(p.Body)))
		return nil
	})
	if err != nil {
		return crawl.Stats{}, err
	}
	err = db.EachPage(catalogdb.SourceQISModulePage, func(p *catalogdb.RawPage) error {
		semester := normalize.SemesterKey(parser.QISSemester(p.Body))
		readings[p.Key] = reading{at: p.FetchedAt, semester: semester}
		note(p.FetchedAt, semester)
		return nil
	})
	if err != nil {
		return crawl.Stats{}, err
	}
	unsettled, err := unsettledModules(db)
	if err != nil {
		return crawl.Stats{}, err
	}

	type dueJob struct {
		job  crawl.Job
		rank int
		at   time.Time
	}
	var due []dueJob
	var counts [4]int
	notDue := 0
	now := time.Now()
	for _, r := range refs {
		rd, fetched := readings[r.ModuleID]
		rank := -1
		switch {
		case !fetched:
			rank = moduleNew
		case changedRows[r.ModuleID], rd.semester != "" && nowSemester != "" && rd.semester != nowSemester:
			rank = moduleChanged
		case unsettled[r.ModuleID] && pace.due(r.ModuleID, rd.at, now, pace.UnsettledMaxAge):
			rank = moduleUnsettled
		case pace.due(r.ModuleID, rd.at, now, pace.MaxAge):
			rank = modulePastAge
		}
		if rank < 0 {
			notDue++
			continue
		}
		counts[rank]++
		due = append(due, dueJob{rank: rank, at: rd.at, job: crawl.Job{
			Source: catalogdb.SourceQISModulePage,
			Key:    r.ModuleID,
			URL:    fmt.Sprintf(ep.QISModuleURL, r.Pordnr, r.Language),
		}})
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
		notDue += len(due) - pace.Limit
		due = due[:pace.Limit]
	}
	log := oplog.For("crawl").With("source", catalogdb.SourceQISModulePage)
	if len(due) == 0 {
		log.Info("nothing to fetch", "event", "crawl.up_to_date", "pages", notDue)
		return crawl.Stats{Skipped: notDue}, nil
	}
	log.Info("module descriptions due", "event", "crawl.modules_due", "never_fetched", counts[moduleNew], "changed", counts[moduleChanged],
		"unsettled", counts[moduleUnsettled], "past_age", counts[modulePastAge], "qis_semester", nowSemester, "fetching", len(due))
	jobs := make([]crawl.Job, len(due))
	for i, d := range due {
		jobs[i] = d.job
	}
	stats, err := crawl.Run(ctx, db, jobs, crawl.Options{Workers: pace.Workers, Delay: pace.Delay, Backoff: pace.Backoff})
	stats.Skipped += notDue
	return stats, err
}

// unsettledModules are the modules offered in the semester the catalog presents, by their
// turnus, whose descriptions name none of its events, as the last build found them. None
// before the first build.
func unsettledModules(db *catalogdb.DB) (map[string]bool, error) {
	var current string
	err := db.SQL().QueryRow("SELECT value FROM meta WHERE key = 'current_semester'").Scan(&current)
	if errors.Is(err, sql.ErrNoRows) || current == "" {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	season := "summer"
	if strings.HasSuffix(current, "W") {
		season = "winter"
	}
	rows, err := db.SQL().Query(`
		SELECT m.id FROM module m
		WHERE m.offer_status = 'active' AND m.turnus_season IN (?, 'both')
		  AND NOT EXISTS (SELECT 1 FROM module_event me JOIN event e ON e.id = me.event_id
		                  WHERE me.module_id = m.id AND e.semester_key = ?)`, season, current)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	unsettled := make(map[string]bool)
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		unsettled[id] = true
	}
	return unsettled, rows.Err()
}

// CrawlTree walks the QIS program tree from its root. Pages that are fresh in the
// archive are read from there, so a run only requests what is missing or stale,
// and it discovers programs and PO versions that did not exist before.
func CrawlTree(ctx context.Context, db *catalogdb.DB, ep Endpoints, pace Pace) (crawl.Stats, error) {
	log := oplog.For("crawl").With("source", catalogdb.SourceQISTree)
	start := time.Now()
	log.Info("tree crawl started", "event", "crawl.started", "delay_ms", pace.Delay.Milliseconds(), "max_age", pace.MaxAge.String(), "limit", pace.Limit)

	// With Spread every page once per MaxAge, each at a time of its own, so that the tree
	// does not come due in one piece.
	fetcher := crawl.NewFetcher(db, crawl.Options{Delay: pace.Delay, MaxAge: pace.MaxAge, Spread: pace.Spread, Backoff: pace.Backoff})
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

// runOldestFirst fetches every page once per pace.MaxAge (with pace.Spread each on a day
// of its own in that period, so that pages read together do not come due together again).
// It orders the due jobs by the age of their archived page (never fetched first), applies
// the limit, and crawls them.
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
		if at, ok := fetched[j.Key]; ok && !pace.due(j.Key, at, now, pace.MaxAge) {
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
