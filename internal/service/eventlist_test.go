package service

import (
	"bytes"
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/parser"
)

// fakeEventSearch answers the event search of QIS with the real entries of
// internal/parser/testdata (the events asked for that it knows, and only those), and
// serves the real event pages of the same directory.
type fakeEventSearch struct {
	srv *httptest.Server

	mu       sync.Mutex
	entries  map[string]string // event ID → the markup of its entry
	searches [][]string        // the IDs each search asked for
	pages    []string          // the event pages fetched
	ignoreID bool              // answer every search with every event, as if QIS dropped the filter
}

func newFakeEventSearch(t *testing.T) *fakeEventSearch {
	t.Helper()
	body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", "qis_event_list.html"))
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	list, err := parser.SplitEventList(bytes.NewReader(body))
	if err != nil {
		t.Fatalf("SplitEventList: %v", err)
	}
	f := &fakeEventSearch{entries: make(map[string]string)}
	for _, e := range list.Entries {
		f.entries[e.ID] = string(e.HTML)
	}
	f.srv = httptest.NewServer(http.HandlerFunc(f.serve))
	t.Cleanup(f.srv.Close)
	return f
}

func (f *fakeEventSearch) endpoints() Endpoints {
	return Endpoints{
		EventURL:     f.srv.URL + "/event?veranstaltung.veranstid=%s",
		EventListURL: f.srv.URL + "/search?veranstaltung.veranstid=%[1]s&P_start=0&P_anzahl=%[2]d&P.vx=lang",
	}
}

func (f *fakeEventSearch) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	defer f.mu.Unlock()
	switch r.URL.Path {
	case "/search":
		asked := strings.Split(r.URL.Query().Get("veranstaltung.veranstid"), ",")
		f.searches = append(f.searches, asked)
		ids := asked
		if f.ignoreID {
			ids = nil
			for id := range f.entries {
				ids = append(ids, id)
			}
			sort.Strings(ids)
		}
		var shown []string
		for _, id := range ids {
			if entry, ok := f.entries[id]; ok {
				shown = append(shown, `<div class="abstand_veranstaltung"></div>`+entry)
			}
		}
		fmt.Fprintf(w, `<html><body><form><div class="InfoLeiste">%d Treffer</div>%s<div class="abstand_veranstaltung"></div></form></body></html>`,
			len(shown), strings.Join(shown, "\n"))
	case "/event":
		id := r.URL.Query().Get("veranstaltung.veranstid")
		f.pages = append(f.pages, id)
		body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", "qis_event_"+id+".html"))
		if err != nil {
			http.NotFound(w, r)
			return
		}
		_, _ = w.Write(body)
	default:
		http.NotFound(w, r)
	}
}

func (f *fakeEventSearch) takeSearches() [][]string {
	f.mu.Lock()
	defer f.mu.Unlock()
	s := f.searches
	f.searches = nil
	return s
}

func (f *fakeEventSearch) takePages() []string {
	f.mu.Lock()
	defer f.mu.Unlock()
	p := f.pages
	f.pages = nil
	sort.Strings(p)
	return p
}

// linkEvents archives a QIS module description that links the events, which is how the
// crawl learns which events to look up.
func linkEvents(t *testing.T, db *catalogdb.DB, ids ...string) {
	t.Helper()
	var links strings.Builder
	for _, id := range ids {
		fmt.Fprintf(&links, `<li><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;veranstaltung.veranstid=%s">%s Vorlesung</a></li>`, id, id)
	}
	body := `<table><tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">13921</td></tr>
		<tr><td class="tabelle1_alignleft">Veranstaltungen im aktuellen Semester:</td><td class="tabelle2inhalt"><ul>` + links.String() + `</ul></td></tr></table>`
	if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceQISModulePage, Key: "13921", URL: "13921", HTTPStatus: 200, Body: []byte(body), FetchedAt: time.Now()}); err != nil {
		t.Fatalf("PutPage: %v", err)
	}
}

// age moves the fetch of an archived page into the past.
func age(t *testing.T, db *catalogdb.DB, source, key string, by time.Duration) {
	t.Helper()
	at := time.Now().Add(-by).UTC().Format(time.RFC3339)
	if _, err := db.SQL().Exec("UPDATE raw_page SET fetched_at = ? WHERE source = ? AND key = ?", at, source, key); err != nil {
		t.Fatalf("age %s %s: %v", source, key, err)
	}
}

// Every linked event is looked up at night, 250 to a request, and each entry is archived
// on its own under the address of the event's page. An event the search does not show is
// remembered as such.
func TestEventListLooksUpTheLinkedEvents(t *testing.T) {
	f := newFakeEventSearch(t)
	db := openTestDB(t)
	ids := []string{"151296", "152864", "150708", "149030"}
	for i := 0; i < eventListBatch; i++ {
		ids = append(ids, fmt.Sprint(900000+i)) // linked, but not in the search
	}
	linkEvents(t, db, ids...)

	stats, err := CrawlEventList(context.Background(), db, f.endpoints(), EventListPace{Pace: Pace{MaxAge: 12 * time.Hour}, UnsettledMaxAge: time.Hour}, true)
	if err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	if stats.Fetched != 4 || stats.NotFound != eventListBatch || stats.Failed != 0 {
		t.Errorf("stats = %+v", stats)
	}
	searches := f.takeSearches()
	if len(searches) != 2 || len(searches[0]) != eventListBatch || len(searches[1]) != 4 {
		t.Fatalf("%d searches of %v IDs, want 2 of 250 and 4", len(searches), lengths(searches))
	}

	page, err := db.GetPage(catalogdb.SourceQISEventEntry, "151296")
	if err != nil {
		t.Fatalf("GetPage: %v", err)
	}
	if page.URL != f.srv.URL+"/event?veranstaltung.veranstid=151296" || !strings.Contains(string(page.Body), "Lightweight Design and Construction") {
		t.Errorf("entry = %s %s", page.URL, page.Body[:80])
	}
	if strings.Contains(string(page.Body), "Einführung in die Erziehungswissenschaft") {
		t.Errorf("the entry of 151296 carries its neighbour")
	}
	if missing, err := db.GetPage(catalogdb.SourceQISEventEntry, "900000"); err != nil || missing.HTTPStatus != http.StatusNotFound {
		t.Errorf("an event the search does not show = %+v, %v", missing, err)
	}

	// Looked up a moment ago: nothing is due, by night or by day.
	for _, all := range []bool{true, false} {
		if _, err := CrawlEventList(context.Background(), db, f.endpoints(), EventListPace{Pace: Pace{MaxAge: 12 * time.Hour}, UnsettledMaxAge: time.Hour}, all); err != nil {
			t.Fatalf("CrawlEventList: %v", err)
		}
	}
	if searches := f.takeSearches(); len(searches) != 0 {
		t.Errorf("searched again for %v", lengths(searches))
	}
}

// By day the events whose dates are not settled are looked up again every hour: those
// without dates, those with a placeholder, and those the search did not show. Only while a
// module page links them.
func TestEventListChecksUnsettledEventsByDay(t *testing.T) {
	f := newFakeEventSearch(t)
	db := openTestDB(t)
	ids := []string{"151296", "152864", "150708", "149030", "900000"}
	linkEvents(t, db, ids...)
	pace := EventListPace{Pace: Pace{MaxAge: 12 * time.Hour}, UnsettledMaxAge: time.Hour}
	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), pace, true); err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	f.takeSearches()

	// Within the hour nothing is asked again.
	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), pace, false); err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	if searches := f.takeSearches(); len(searches) != 0 {
		t.Errorf("searched again within the hour: %v", searches)
	}

	for _, id := range ids {
		age(t, db, catalogdb.SourceQISEventEntry, id, 2*time.Hour)
	}
	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), pace, false); err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	// 149030 has no date, 150708 only the placeholder of an exam, and the search did not
	// show 900000; the dates of 151296 and 152864 are settled.
	if searches := f.takeSearches(); len(searches) != 1 || strings.Join(searches[0], ",") != "149030,150708,900000" {
		t.Errorf("searched for %v, want the three events in doubt", searches)
	}

	// An event no module page links any more is left to age.
	linkEvents(t, db, "151296", "152864", "150708")
	for _, id := range ids {
		age(t, db, catalogdb.SourceQISEventEntry, id, 2*time.Hour)
	}
	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), pace, false); err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	if searches := f.takeSearches(); len(searches) != 1 || strings.Join(searches[0], ",") != "150708" {
		t.Errorf("searched for %v, want 150708 alone", searches)
	}
}

// If QIS stopped filtering by ID, the answer would name events nobody asked for and leave
// out the ones asked for. Such an answer is refused before anything is archived from it.
func TestEventListRefusesAnAnswerToAnotherQuestion(t *testing.T) {
	f := newFakeEventSearch(t)
	f.ignoreID = true
	db := openTestDB(t)
	linkEvents(t, db, "151296", "900000")

	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), EventListPace{Pace: Pace{MaxAge: time.Hour}}, true); err == nil {
		t.Fatalf("an answer with events that were not asked for was accepted")
	}
	times, err := db.FetchTimes(catalogdb.SourceQISEventEntry)
	if err != nil || len(times) != 0 {
		t.Errorf("archived %v from a refused answer (%v)", times, err)
	}
}

var (
	testEventList  = EventListPace{Pace: Pace{MaxAge: 12 * time.Hour}, UnsettledMaxAge: time.Hour}
	testEventPages = EventPagePace{Pace: Pace{MaxAge: 72 * time.Hour}, ConfirmedMaxAge: 30 * 24 * time.Hour, EntryFresh: 24 * time.Hour, DayLimit: 2}
)

// crawlEvents runs both event stages, as a cycle does, and returns the pages fetched.
func crawlEvents(t *testing.T, f *fakeEventSearch, db *catalogdb.DB, all bool) []string {
	t.Helper()
	if _, err := CrawlEventList(context.Background(), db, f.endpoints(), testEventList, all); err != nil {
		t.Fatalf("CrawlEventList: %v", err)
	}
	if _, err := CrawlEvents(context.Background(), db, f.endpoints(), testEventPages, all); err != nil {
		t.Fatalf("CrawlEvents: %v", err)
	}
	return f.takePages()
}

// The page of an event is fetched when it is missing, when the search has news for it, or
// when it is past its age; a page the search vouches for ages by a month instead of three
// days, and a search that stopped working vouches for nothing.
func TestEventPagesFollowTheList(t *testing.T) {
	f := newFakeEventSearch(t)
	db := openTestDB(t)
	ids := []string{"151296", "152864", "147988"}
	linkEvents(t, db, ids...)

	if got := crawlEvents(t, f, db, true); strings.Join(got, ",") != "147988,151296,152864" {
		t.Fatalf("first run fetched %v, want every page", got)
	}
	if got := crawlEvents(t, f, db, true); len(got) != 0 {
		t.Errorf("fetched %v although the search vouches for every page", got)
	}

	// Past the three days that were the rule before the search: still vouched for.
	for _, id := range ids {
		age(t, db, catalogdb.SourceQISEvent, id, 4*24*time.Hour)
	}
	if got := crawlEvents(t, f, db, true); len(got) != 0 {
		t.Errorf("fetched %v, pages the search vouches for wait a month", got)
	}

	// The search moves a date of 152864: its page is fetched, the others are not.
	f.mu.Lock()
	f.entries["152864"] = strings.Replace(f.entries["152864"], "16:30", "17:00", 1)
	f.mu.Unlock()
	age(t, db, catalogdb.SourceQISEventEntry, "152864", 13*time.Hour)
	if got := crawlEvents(t, f, db, true); strings.Join(got, ",") != "152864" {
		t.Errorf("fetched %v, want the page of the event the search changed", got)
	}
	// The page still says 16:30. Reading the same entry again is no news: the page is
	// not fetched again and again for a difference the next fetch would not settle.
	age(t, db, catalogdb.SourceQISEventEntry, "152864", 13*time.Hour)
	if got := crawlEvents(t, f, db, true); len(got) != 0 {
		t.Errorf("fetched %v again for an entry that did not change", got)
	}

	// A month on, a vouched page is fetched again for its remarks.
	age(t, db, catalogdb.SourceQISEvent, "151296", 31*24*time.Hour)
	if got := crawlEvents(t, f, db, true); strings.Join(got, ",") != "151296" {
		t.Errorf("fetched %v, want the page past a month", got)
	}

	// Without a working search, the pages fall back to their three days.
	for _, id := range ids {
		age(t, db, catalogdb.SourceQISEventEntry, id, 2*24*time.Hour)
		age(t, db, catalogdb.SourceQISEvent, id, 4*24*time.Hour)
	}
	if _, err := CrawlEvents(context.Background(), db, f.endpoints(), testEventPages, true); err != nil {
		t.Fatalf("CrawlEvents: %v", err)
	}
	if got := f.takePages(); strings.Join(got, ",") != "147988,151296,152864" {
		t.Errorf("fetched %v, want every page once the search is two days old", got)
	}
}

// The search cannot vouch for what it does not state. The page of an event whose dates are
// not settled, and of one the search does not show, ages by three days: a remark such as
// „Termin nach Vereinbarung" is on the page only.
func TestEventPagesInDoubtAgeByDays(t *testing.T) {
	f := newFakeEventSearch(t)
	db := openTestDB(t)
	ids := []string{"151296", "150708", "149030"}
	linkEvents(t, db, append(ids, "151278")...)
	f.mu.Lock()
	delete(f.entries, "151278") // a page, but no entry in the search
	f.mu.Unlock()

	if got := crawlEvents(t, f, db, true); strings.Join(got, ",") != "149030,150708,151278,151296" {
		t.Fatalf("first run fetched %v", got)
	}
	for _, id := range append(ids, "151278") {
		age(t, db, catalogdb.SourceQISEvent, id, 4*24*time.Hour)
	}
	if got := crawlEvents(t, f, db, true); strings.Join(got, ",") != "149030,150708,151278" {
		t.Errorf("fetched %v, want the pages in doubt and not the vouched one", got)
	}
}

// By day a page is fetched only when the search has news for it, so that a change reaches
// the catalog with its remarks within the cycle; pages past their age wait for the night.
func TestEventPagesByDay(t *testing.T) {
	f := newFakeEventSearch(t)
	db := openTestDB(t)
	linkEvents(t, db, "151296", "152864", "150708", "149030")
	if got := crawlEvents(t, f, db, true); len(got) != 4 {
		t.Fatalf("first run fetched %v", got)
	}

	// Pages past their age, and unsettled entries read again without a change: nothing by day.
	for _, id := range []string{"151296", "152864", "150708", "149030"} {
		age(t, db, catalogdb.SourceQISEvent, id, 40*24*time.Hour)
		age(t, db, catalogdb.SourceQISEventEntry, id, 2*time.Hour)
	}
	if got := crawlEvents(t, f, db, false); len(got) != 0 {
		t.Errorf("fetched %v by day, want nothing", got)
	}
	if got := crawlEvents(t, f, db, true); len(got) != 4 {
		t.Errorf("fetched %v at night, want the four pages past their age", got)
	}

	// The exam of 150708 gets its date, and the event without dates gets dates: the
	// hourly look at unsettled events sees it, and the pages follow at once.
	f.mu.Lock()
	f.entries["150708"] = strings.Replace(f.entries["150708"], "01:00", "10:00", 1)
	f.entries["149030"] = strings.ReplaceAll(f.entries["150708"], "150708", "149030") // dates that appeared
	f.mu.Unlock()
	for _, id := range []string{"150708", "149030"} {
		age(t, db, catalogdb.SourceQISEvent, id, time.Hour) // the night was an hour ago
		age(t, db, catalogdb.SourceQISEventEntry, id, 2*time.Hour)
	}
	if got := crawlEvents(t, f, db, false); strings.Join(got, ",") != "149030,150708" {
		t.Errorf("fetched %v by day, want the two pages the search has news for", got)
	}
}

func lengths(searches [][]string) []int {
	n := make([]int, len(searches))
	for i, s := range searches {
		n[i] = len(s)
	}
	return n
}
