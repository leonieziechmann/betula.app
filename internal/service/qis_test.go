package service

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
)

// fakeQIS serves the module table of QIS in the shape the crawler pages through,
// and one description per module.
type fakeQIS struct {
	srv     *httptest.Server
	modules int
	hits    map[string]int
	views   map[string]string // pordnr → the objLanguage it was asked for
}

func newFakeQIS(t *testing.T, modules int) *fakeQIS {
	f := &fakeQIS{modules: modules, hits: make(map[string]int), views: make(map[string]string)}
	f.srv = httptest.NewServer(http.HandlerFunc(f.serve))
	t.Cleanup(f.srv.Close)
	return f
}

func (f *fakeQIS) endpoints() Endpoints {
	return Endpoints{
		QISModuleList: f.srv.URL + "/table?P_start=%d&P_anzahl=%d",
		QISModuleURL:  f.srv.URL + "/module?nodeID=pordnr=%[1]s&objLanguage=%[2]s&pord.pordnr=%[1]s",
	}
}

func (f *fakeQIS) serve(w http.ResponseWriter, r *http.Request) {
	f.hits[r.URL.Path+"?"+r.URL.RawQuery]++
	switch r.URL.Path {
	case "/table":
		start, _ := strconv.Atoi(r.URL.Query().Get("P_start"))
		count, _ := strconv.Atoi(r.URL.Query().Get("P_anzahl"))
		var b strings.Builder
		b.WriteString(`<table summary="Suchergebnis"><tr><th>Nr.</th><th>Modultitel</th></tr>`)
		for i := start; i < start+count && i < f.modules; i++ {
			id := 20000 + i
			language := "Deutsch"
			if i%2 == 1 {
				language = "Englisch"
			}
			fmt.Fprintf(&b, `<tr><td>%d</td><td><a href="%s/module?nodeID=pordnr=%d&amp;pord.pordnr=%d">Modul %d</a></td>`+
				`<td>%s</td><td>6</td><td></td><td></td></tr>`, id, f.srv.URL, 900+i, 900+i, id, language)
		}
		b.WriteString(`</table>`)
		fmt.Fprint(w, b.String())
	case "/module":
		f.views[r.URL.Query().Get("pord.pordnr")] = r.URL.Query().Get("objLanguage")
		pordnr := r.URL.Query().Get("pord.pordnr")
		n, _ := strconv.Atoi(pordnr)
		id := 20000 + n - 900
		fmt.Fprintf(w, `<table>
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

	stats, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{})
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

	if _, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{}); err != nil {
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

	if _, err := CrawlQISModuleList(context.Background(), db, f.endpoints(), Pace{}); err != nil {
		t.Fatalf("CrawlQISModuleList: %v", err)
	}
	stats, err := CrawlQISModules(context.Background(), db, f.endpoints(), Pace{Workers: 2})
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
