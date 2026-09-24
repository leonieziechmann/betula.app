package service

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
)

// fakeQIS serves the module table of QIS in the shape the crawler pages through,
// and one description per module, each under the head that names the semester QIS
// calls current.
type fakeQIS struct {
	srv     *httptest.Server
	modules int

	mu       sync.Mutex
	semester string         // the head of every page, „WiSe 2026/27"
	credits  map[int]string // module number → the credits its row states, "6" by default
	hits     map[string]int
	views    map[string]string // pordnr → the objLanguage it was asked for
	served   []string          // module numbers whose description was served, in order
}

// takeServed returns the module numbers whose description was served since the last call.
func (f *fakeQIS) takeServed() []string {
	f.mu.Lock()
	defer f.mu.Unlock()
	served := f.served
	f.served = nil
	sort.Strings(served)
	return served
}

func newFakeQIS(t *testing.T, modules int) *fakeQIS {
	f := &fakeQIS{modules: modules, semester: "WiSe 2026/27", credits: make(map[int]string), hits: make(map[string]int), views: make(map[string]string)}
	f.srv = httptest.NewServer(http.HandlerFunc(f.serve))
	t.Cleanup(f.srv.Close)
	return f
}

func (f *fakeQIS) head() string {
	return `<div class="services"><ol><li><a href="/rds?state=change&amp;getglobal=semester" id="choosesemester" title="Semester wählen ...">` + f.semester + `</a></li></ol></div>`
}

func (f *fakeQIS) endpoints() Endpoints {
	return Endpoints{
		QISModuleList: f.srv.URL + "/table?P_start=%d&P_anzahl=%d",
		QISModuleURL:  f.srv.URL + "/module?nodeID=pordnr=%[1]s&objLanguage=%[2]s&pord.pordnr=%[1]s",
	}
}

func (f *fakeQIS) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.hits[r.URL.Path+"?"+r.URL.RawQuery]++
	switch r.URL.Path {
	case "/table":
		start, _ := strconv.Atoi(r.URL.Query().Get("P_start"))
		count, _ := strconv.Atoi(r.URL.Query().Get("P_anzahl"))
		var b strings.Builder
		b.WriteString(f.head() + `<table summary="Suchergebnis"><tr><th>Nr.</th><th>Modultitel</th></tr>`)
		for i := start; i < start+count && i < f.modules; i++ {
			id := 20000 + i
			language := "Deutsch"
			if i%2 == 1 {
				language = "Englisch"
			}
			credits := f.credits[id]
			if credits == "" {
				credits = "6"
			}
			fmt.Fprintf(&b, `<tr><td>%d</td><td><a href="%s/module?nodeID=pordnr=%d&amp;pord.pordnr=%d">Modul %d</a></td>`+
				`<td>%s</td><td>%s</td><td></td><td></td></tr>`, id, f.srv.URL, 900+i, 900+i, id, language, credits)
		}
		b.WriteString(`</table>`)
		fmt.Fprint(w, b.String())
	case "/module":
		f.views[r.URL.Query().Get("pord.pordnr")] = r.URL.Query().Get("objLanguage")
		pordnr := r.URL.Query().Get("pord.pordnr")
		n, _ := strconv.Atoi(pordnr)
		id := 20000 + n - 900
		f.served = append(f.served, strconv.Itoa(id))
		fmt.Fprintf(w, f.head()+`<table>
			<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">%d</td></tr>
			<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">Modul %d</td></tr>
			<tr><td class="tabelle1_alignleft">Veranstaltungen im aktuellen Semester:</td>
			    <td class="tabelle2inhalt"><ul><li><a href="/event?veranstaltung.veranstid=%d">Vorlesung</a></li></ul></td></tr>
			</table>`, id, id, 500000+n)
	default:
		http.NotFound(w, r)
	}
}

func openTestDB(t *testing.T) *catalogdb.DB {
	t.Helper()
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

// The table is longer than one response may be, so it is read in chunks until one
// of them is short. Nothing may be lost at a chunk border.
func TestCrawlQISModuleListPagesThroughTheTable(t *testing.T) {
	const modules = qisModuleListChunk + 7
	f := newFakeQIS(t, modules)
	db := openTestDB(t)

	stats, _, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{})
	if err != nil {
		t.Fatalf("CrawlQISModuleList: %v", err)
	}
	if stats.Fetched != 2 {
		t.Errorf("fetched %d chunks, want 2", stats.Fetched)
	}

	refs, err := QISModuleRefs(db)
	if err != nil {
		t.Fatalf("QISModuleRefs: %v", err)
	}
	if len(refs) != modules {
		t.Fatalf("got %d modules, want %d", len(refs), modules)
	}
	if refs[0].ModuleID != "20000" || refs[0].Pordnr != "900" {
		t.Errorf("first ref = %+v", refs[0])
	}
	last := refs[len(refs)-1]
	if last.ModuleID != strconv.Itoa(20000+modules-1) {
		t.Errorf("last ref = %+v", last)
	}
}

// A table that shrank must not keep naming modules through a chunk that is now
// behind its end.
func TestCrawlQISModuleListDropsChunksBehindTheEnd(t *testing.T) {
	f := newFakeQIS(t, 3)
	db := openTestDB(t)

	stale := catalogdb.RawPage{
		Source: catalogdb.SourceQISModuleList, Key: qisModuleListKey(qisModuleListChunk),
		URL: "https://example/old", FetchedAt: time.Now(), HTTPStatus: 200,
		Body: []byte(`<table summary="Suchergebnis"><tr><td>19999</td><td><a href="/m?pord.pordnr=1">Altes Modul</a></td></tr></table>`),
	}
	if err := db.PutPage(stale); err != nil {
		t.Fatalf("PutPage: %v", err)
	}

	if _, _, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{}); err != nil {
		t.Fatalf("CrawlQISModuleList: %v", err)
	}

	refs, err := QISModuleRefs(db)
	if err != nil {
		t.Fatalf("QISModuleRefs: %v", err)
	}
	if len(refs) != 3 {
		t.Fatalf("got %d modules, want 3: %+v", len(refs), refs)
	}
	for _, r := range refs {
		if r.ModuleID == "19999" {
			t.Errorf("the module of the dropped chunk is still named: %+v", refs)
		}
	}
}

// Every module of the table gets its description archived under its module number.
func TestCrawlQISModules(t *testing.T) {
	f := newFakeQIS(t, 3)
	db := openTestDB(t)

	if _, _, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{}); err != nil {
		t.Fatalf("CrawlQISModuleList: %v", err)
	}
	stats, err := CrawlQISModules(context.Background(), db, f.endpoints(), ModulePace{Pace: Pace{Workers: 2}}, nil)
	if err != nil {
		t.Fatalf("CrawlQISModules: %v", err)
	}
	if stats.Fetched != 3 || stats.Failed != 0 {
		t.Fatalf("stats = %+v", stats)
	}

	// A description is written in the language the module is taught in; the other
	// view of it leaves the learning outcomes and the contents empty.
	if f.views["900"] != "de" || f.views["901"] != "en" {
		t.Errorf("views asked for = %v, want the German module in de and the English one in en", f.views)
	}

	page, err := db.GetPage(catalogdb.SourceQISModulePage, "20001")
	if err != nil {
		t.Fatalf("GetPage: %v", err)
	}
	if !strings.Contains(string(page.Body), "veranstid=500901") {
		t.Errorf("the archived description does not name the event: %s", page.Body)
	}

	// The events of the current semester are what this source is for: the event
	// crawl has to see them, or it would keep fetching last semester's.
	ids, err := LinkedEventIDs(db)
	if err != nil {
		t.Fatalf("LinkedEventIDs: %v", err)
	}
	want := map[string]bool{"500900": true, "500901": true, "500902": true}
	if len(ids) != len(want) {
		t.Fatalf("LinkedEventIDs = %v, want %v", ids, want)
	}
	for _, id := range ids {
		if !want[id] {
			t.Errorf("LinkedEventIDs = %v, unexpected %s", ids, id)
		}
	}
}

// A row of the module table that changed is reported, so that the description of its
// module, which states the same facts, is read again.
func TestCrawlQISModuleListReportsChangedRows(t *testing.T) {
	f := newFakeQIS(t, 3)
	db := openTestDB(t)
	ctx := context.Background()

	if _, changed, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{}); err != nil || len(changed) != 0 {
		t.Fatalf("first reading: changed %v, %v", changed, err)
	}
	f.mu.Lock()
	f.credits[20001] = "8"
	f.mu.Unlock()
	if _, changed, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{}); err != nil || len(changed) != 1 || !changed["20001"] {
		t.Errorf("after the credits of 20001 changed: changed %v, %v", changed, err)
	}
	if _, changed, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{}); err != nil || len(changed) != 0 {
		t.Errorf("read again unchanged: changed %v, %v", changed, err)
	}
	// Within its age the table is not read at all.
	stats, changed, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{MaxAge: time.Hour})
	if err != nil || stats.Fetched != 0 || len(changed) != 0 {
		t.Errorf("within its age: %+v, changed %v, %v", stats, changed, err)
	}
}

// A description is read once a month, unless something it depends on changed: its row in
// the module table, or the semester QIS calls current, which decides the events it names.
func TestQISModulesFollowWhatTheyDependOn(t *testing.T) {
	f := newFakeQIS(t, 3)
	db := openTestDB(t)
	ctx := context.Background()
	pace := ModulePace{Pace: Pace{MaxAge: 30 * 24 * time.Hour}, UnsettledMaxAge: 7 * 24 * time.Hour}
	crawlModules := func() []string {
		t.Helper()
		_, changed, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{})
		if err != nil {
			t.Fatalf("CrawlQISModuleList: %v", err)
		}
		if _, err := CrawlQISModules(ctx, db, f.endpoints(), pace, changed); err != nil {
			t.Fatalf("CrawlQISModules: %v", err)
		}
		return f.takeServed()
	}

	if got := crawlModules(); strings.Join(got, ",") != "20000,20001,20002" {
		t.Fatalf("first run read %v", got)
	}
	if got := crawlModules(); len(got) != 0 {
		t.Errorf("read %v although nothing changed", got)
	}

	f.mu.Lock()
	f.credits[20002] = "9"
	f.mu.Unlock()
	if got := crawlModules(); strings.Join(got, ",") != "20002" {
		t.Errorf("read %v, want the module whose row changed", got)
	}

	// QIS moves on to the summer semester: every description named the winter's events.
	f.mu.Lock()
	f.semester = "SoSe 2027"
	f.mu.Unlock()
	if got := crawlModules(); strings.Join(got, ",") != "20000,20001,20002" {
		t.Errorf("read %v, want every description after the semester moved on", got)
	}
	if got := crawlModules(); len(got) != 0 {
		t.Errorf("read %v again", got)
	}

	// A month on, every description is read again.
	for _, id := range []string{"20000", "20001", "20002"} {
		age(t, db, catalogdb.SourceQISModulePage, id, 31*24*time.Hour)
	}
	if got := crawlModules(); len(got) != 3 {
		t.Errorf("read %v after a month, want all three", got)
	}
}

// BTU publishes the events of a semester module by module. A module offered in the
// semester the catalog presents whose description names none of its events is read
// weekly until it does.
func TestQISModulesOfferedWithoutEventsAreReadWeekly(t *testing.T) {
	f := newFakeQIS(t, 3)
	db := openTestDB(t)
	ctx := context.Background()
	pace := ModulePace{Pace: Pace{MaxAge: 30 * 24 * time.Hour}, UnsettledMaxAge: 7 * 24 * time.Hour}
	if _, _, err := CrawlQISModuleList(ctx, db, f.endpoints(), Pace{}); err != nil {
		t.Fatalf("CrawlQISModuleList: %v", err)
	}
	if _, err := CrawlQISModules(ctx, db, f.endpoints(), pace, nil); err != nil {
		t.Fatalf("CrawlQISModules: %v", err)
	}
	f.takeServed()

	// What the last build found: 20000 is offered in winter and names no winter event,
	// 20001 is offered every semester and names one, 20002 is offered in summer only.
	_, err := db.SQL().Exec(`
		INSERT INTO meta (key, value) VALUES ('current_semester', '2026W');
		INSERT INTO semester (key, season, year, label, starts_on, ends_on) VALUES ('2026W', 'winter', 2026, 'WiSe 2026/27', '2026-10-01', '2027-03-31');
		INSERT INTO module (id, title, detail_status, offer_status, is_fues, turnus_season) VALUES
			('20000', 'Modul 20000', 'ok', 'active', 0, 'winter'),
			('20001', 'Modul 20001', 'ok', 'active', 0, 'both'),
			('20002', 'Modul 20002', 'ok', 'active', 0, 'summer');
		INSERT INTO event (id, title, category, semester_key, source_url, fetched_at) VALUES ('500901', 'Vorlesung', 'teaching', '2026W', 'u', '2026-09-24T01:00:00Z');
		INSERT INTO module_event (module_id, event_id) VALUES ('20001', '500901');`)
	if err != nil {
		t.Fatal(err)
	}
	for _, id := range []string{"20000", "20001", "20002"} {
		age(t, db, catalogdb.SourceQISModulePage, id, 8*24*time.Hour)
	}
	if _, err := CrawlQISModules(ctx, db, f.endpoints(), pace, nil); err != nil {
		t.Fatalf("CrawlQISModules: %v", err)
	}
	if got := f.takeServed(); strings.Join(got, ",") != "20000" {
		t.Errorf("read %v, want the winter module without winter events", got)
	}
}
