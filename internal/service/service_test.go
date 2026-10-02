package service

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"maps"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
)

// fakeBTU serves a tiny catalog: one program with one PO and one area, two modules,
// one event. Tests change credits and break pages while the service runs.
type fakeBTU struct {
	srv       *httptest.Server
	mu        sync.Mutex
	credits   string
	qisEvents []string        // the events the QIS description of 11881 names
	removed   map[string]bool // events BTU removed: the search leaves them out, their page is the empty frame of QIS
	broken    map[string]bool // path prefix → answer 500
	hits      map[string]int
}

func newFakeBTU(t *testing.T) *fakeBTU {
	f := &fakeBTU{credits: "8", qisEvents: []string{"120999"}, removed: make(map[string]bool), broken: make(map[string]bool), hits: make(map[string]int)}
	f.srv = httptest.NewServer(http.HandlerFunc(f.serve))
	t.Cleanup(f.srv.Close)
	return f
}

func (f *fakeBTU) endpoints() Endpoints {
	return Endpoints{
		CatalogURL:    f.srv.URL + "/modul",
		FUESURL:       f.srv.URL + "/fues",
		ModuleURL:     f.srv.URL + "/modul/%s",
		QISModuleList: f.srv.URL + "/qis-table?P_start=%d&P_anzahl=%d",
		QISModuleURL:  f.srv.URL + "/qis-modul?nodeID=pordnr=%[1]s&objLanguage=%[2]s&pord.pordnr=%[1]s",
		EventURL:      f.srv.URL + "/event?veranstaltung.veranstid=%s",
		EventListURL:  f.srv.URL + "/search?veranstaltung.veranstid=%[1]s&P_anzahl=%[2]d",
		TreeRootURL:   f.srv.URL + "/tree?nodeID=auswahlBaum",
	}
}

const poNodeID = "auswahlBaum|studiengang:stg=079|abschluss:abschl=82|stgSpecials:vert=,schwp=,kzfa=H,pversion=2008"

func (f *fakeBTU) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	credits, qisEvents, removed := f.credits, f.qisEvents, maps.Clone(f.removed)
	f.hits[r.URL.Path]++
	for prefix := range f.broken {
		if strings.HasPrefix(r.URL.Path, prefix) {
			f.mu.Unlock()
			http.Error(w, "down", http.StatusInternalServerError)
			return
		}
	}
	f.mu.Unlock()

	treeLink := func(nodeID, text string) string {
		return fmt.Sprintf(`<li><a class="regular" href="%s/tree?nodeID=%s">%s</a></li>`, f.srv.URL, strings.ReplaceAll(nodeID, "|", "%7C"), text)
	}
	breadcrumb := `<div class="KruemelpfadEintrag"><a href="#">Studiengang: Informatik</a></div>
		<div class="KruemelpfadEintrag"><a href="#">Module für Abschluss: Bachelor (universitär)</a></div>
		<div class="KruemelpfadEintrag"> PO-Version: 2008 - 2. SÄ 2024 </div>`

	switch {
	case r.URL.Path == "/modul":
		fmt.Fprint(w, `<table><tbody class="list">
			<tr><td class="moduleNumber"><a href="/modul/11101">11101</a></td><td class="title">Lineare Algebra</td></tr>
			<tr><td class="moduleNumber"><a href="/modul/11881">11881</a></td><td class="title">Data Mining</td></tr></tbody></table>`)
	case r.URL.Path == "/fues":
		fmt.Fprint(w, `<table summary="Suchergebnis"><tr><th>Nr.</th><th>Titel</th></tr></table>`)
	case r.URL.Path == "/modul/11101" || r.URL.Path == "/modul/11881":
		id := strings.TrimPrefix(r.URL.Path, "/modul/")
		events := ""
		if id == "11881" {
			events = `<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul><li><a href="` + f.srv.URL + `/event?veranstaltung.veranstid=120285">120285 Vorlesung</a></li></ul></td></tr>`
		}
		fmt.Fprintf(w, `<div class="tx-btusysteme"><h1>%s - Modul %s</h1><table>
			<tr><td>Modulnummer:</td><td>%s</td></tr>
			<tr><td>Leistungspunkte:</td><td>%s</td></tr>
			<tr><td>Angebotsturnus:</td><td>jedes Wintersemester</td></tr>
			<tr><td>Inhalte:</td><td><ul><li>Grundlagen von Modul %s</li><li>Anwendungen</li></ul></td></tr>
			<tr><td>Zuordnung zu Studiengängen:</td><td><ul><li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>
			%s</table></div>`, id, id, id, credits, id, events)
	case r.URL.Path == "/qis-table":
		// The module table of QIS: the same two modules, with the internal number
		// that addresses their description.
		fmt.Fprintf(w, `<table summary="Suchergebnis"><tr><th>Nr.</th><th>Modultitel</th></tr>
			<tr><td>11101</td><td><a href="%s/qis-modul?pord.pordnr=6951">Lineare Algebra</a></td><td>Deutsch</td><td>%s</td><td></td><td></td></tr>
			<tr><td>11881</td><td><a href="%s/qis-modul?pord.pordnr=6952">Data Mining</a></td><td>Deutsch</td><td>%s</td><td></td><td></td></tr>
			</table>`, f.srv.URL, credits, f.srv.URL, credits)
	case r.URL.Path == "/qis-modul":
		// The description in QIS. For 11881 it names the event of the semester that
		// runs now, which the copy on b-tu.de does not know yet.
		id, events := "11101", ""
		if r.URL.Query().Get("pord.pordnr") == "6952" {
			id = "11881"
			var links strings.Builder
			for _, event := range qisEvents {
				fmt.Fprintf(&links, `<li><a href="%s/event?veranstaltung.veranstid=%s">%s Vorlesung</a></li>`, f.srv.URL, event, event)
			}
			events = `<tr><td class="tabelle1_alignleft">Veranstaltungen im aktuellen Semester:</td>` +
				`<td class="tabelle2inhalt"><ul>` + links.String() + `</ul></td></tr>`
		}
		fmt.Fprintf(w, `<table cellpadding="5">
			<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">%s</td></tr>
			<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">Modul %s</td></tr>
			<tr><td class="tabelle1_alignleft">Leistungspunkte:</td><td class="tabelle2inhalt">%s</td></tr>
			<tr><td class="tabelle1_alignleft">Angebotsturnus:</td><td class="tabelle2inhalt">jedes Wintersemester</td></tr>
			<tr><td class="tabelle1_alignleft">Inhalte:</td><td class="tabelle2inhalt"><ul><li>Grundlagen von Modul %s</li><li>Anwendungen</li></ul></td></tr>
			<tr><td class="tabelle1_alignleft">Zuordnung zu Studiengängen:</td><td class="tabelle2inhalt"><ul>
				<li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>
			%s</table>`, id, id, credits, id, events)
	case r.URL.Path == "/search":
		// The event search: the entry of every event asked for, stating what its page states.
		var entries strings.Builder
		var ids []string
		for _, id := range strings.Split(r.URL.Query().Get("veranstaltung.veranstid"), ",") {
			if !removed[id] {
				ids = append(ids, id)
			}
		}
		for _, id := range ids {
			fmt.Fprintf(&entries, `<div class="abstand_veranstaltung"></div>
				<div><h3><a href="%s/rds?state=verpublish&amp;publishid=%s&amp;publishSubDir=veranstaltung">Data Mining</a></h3></div>
				<div>WS 2026/27&nbsp;&nbsp;&nbsp;
				Vorlesung &nbsp;&nbsp;</div>
				<div><h3>Termin</h3></div>
				<table summary="Übersicht über alle Veranstaltungstermine"><tr><th>Tag</th><th>Zeit</th><th>Rhythmus</th><th>Dauer</th><th>fällt aus am</th><th>Lehrperson</th><th>Raum</th><th>Bemerkung</th></tr>
				<tr><td>Dienstag</td><td>09:15 bis<br>10:45</td><td>A/B 13.10.2026 bis 02.02.2027</td><td>13.10.2026 bis<br/>02.02.2027</td><td></td><td>&nbsp;</td><td>Lehrgebäude 1A - 0.22 - Zentralcampus</td><td></td></tr></table>`, f.srv.URL, id)
		}
		fmt.Fprintf(w, `<form><div class="InfoLeiste">%d Treffer</div>%s<div class="abstand_veranstaltung"></div></form>`, len(ids), entries.String())
	case r.URL.Path == "/event" && removed[r.URL.Query().Get("veranstaltung.veranstid")]:
		body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", emptiedEventPage))
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		_, _ = w.Write(body)
	case r.URL.Path == "/event":
		fmt.Fprint(w, `<h1>Data Mining - Einzelansicht</h1><table summary="Grunddaten zur Veranstaltung">
			<tr><th>Veranstaltungsart</th><td>Vorlesung</td><th>Semester</th><td>WS 2026/27</td></tr></table>
			<table summary="Übersicht über alle Veranstaltungstermine"><tr><th>Tag</th><th>Zeit</th><th>Rhythmus</th><th>Dauer</th><th>Raum</th></tr>
			<tr><td>Di.</td><td>09:15 bis 10:45</td><td>A/B</td><td>13.10.2026 bis 02.02.2027</td><td>Lehrgebäude 1A - 0.22 - Zentralcampus</td></tr></table>`)
	case r.URL.Path == "/tree":
		switch nodeID := r.URL.Query().Get("nodeID"); nodeID {
		case "auswahlBaum":
			fmt.Fprint(w, `<ul class="treelist">`+treeLink("auswahlBaum|studiengang:stg=079", "Studiengang: Informatik")+`</ul>`)
		case "auswahlBaum|studiengang:stg=079":
			fmt.Fprint(w, `<ul class="treelist">`+treeLink("auswahlBaum|studiengang:stg=079|abschluss:abschl=82", "Module für Abschluss: Bachelor (universitär)")+`</ul>`)
		case "auswahlBaum|studiengang:stg=079|abschluss:abschl=82":
			fmt.Fprint(w, `<ul class="treelist">`+treeLink(poNodeID, "PO-Version: 2008 - 2. SÄ 2024")+`</ul>`)
		case poNodeID:
			fmt.Fprint(w, breadcrumb+`<ul class="treelist">`+treeLink(poNodeID+"|konto:1", "Pflichtmodule")+`</ul>`)
		case poNodeID + "|konto:1":
			fmt.Fprint(w, breadcrumb+`<ul class="treelist">`+treeLink(poNodeID+"|konto:1|pruefung:1", "11101 Lineare Algebra")+`</ul>`)
		default:
			http.NotFound(w, r)
		}
	default:
		http.NotFound(w, r)
	}
}

func newTestService(t *testing.T, site *fakeBTU) (*Service, Config) {
	t.Helper()
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })

	fast := Pace{Workers: 2, Backoff: time.Millisecond} // MaxAge 0: refetch everything in every cycle
	cfg := Config{
		Endpoints:   site.endpoints(),
		SnapshotDir: filepath.Join(t.TempDir(), "snapshot"),
		Lists:       fast, Modules: fast, QISModules: ModulePace{Pace: fast}, Tree: fast,
		EventList: EventListPace{Pace: fast}, Events: EventPagePace{Pace: fast},
		StaleAfter: time.Hour,
	}
	return New(db, cfg, nil), cfg
}

func stage(r CycleResult, name string) StageResult {
	for _, s := range r.Stages {
		if s.Name == name {
			return s
		}
	}
	return StageResult{Name: "missing stage " + name}
}

func TestCyclePublishesOnlyWhenContentChanges(t *testing.T) {
	site := newFakeBTU(t)
	svc, cfg := newTestService(t, site)
	ctx := context.Background()

	first := svc.RunCycle(ctx)
	if first.Result != "ok" || !first.Published {
		t.Fatalf("first cycle = %+v", first)
	}
	if s := stage(first, "modules"); s.Crawl == nil || s.Crawl.Fetched != 2 || s.Crawl.Failed != 0 {
		t.Errorf("modules stage = %+v", s)
	}
	if s := stage(first, "tree"); s.Crawl == nil || s.Crawl.Fetched != 5 {
		t.Errorf("tree stage = %+v", s)
	}
	if s := stage(first, "qis-modules"); s.Crawl == nil || s.Crawl.Fetched != 2 || s.Crawl.Failed != 0 {
		t.Errorf("qis-modules stage = %+v", s)
	}
	// One event from the copy on b-tu.de, one only the QIS description names: both are
	// looked up in the event search, in one request, and then their pages are fetched.
	if s := stage(first, "event-list"); s.Crawl == nil || s.Crawl.Fetched != 2 || s.Crawl.NotFound != 0 {
		t.Errorf("event-list stage = %+v", s)
	}
	if site.hits["/search"] != 1 {
		t.Errorf("%d requests of the event search, want 1", site.hits["/search"])
	}
	if s := stage(first, "events"); s.Crawl == nil || s.Crawl.Fetched != 2 {
		t.Errorf("events stage = %+v", s)
	}
	snapshot1, err := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir)
	if err != nil {
		t.Fatalf("no snapshot after the first cycle: %v", err)
	}

	// Everything is fetched again, nothing changed: no new snapshot for the browsers.
	second := svc.RunCycle(ctx)
	if second.Result != "ok" || second.Published || stage(second, "export").Skipped != "content unchanged" {
		t.Fatalf("second cycle = %+v", second)
	}
	if again, _ := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir); again == nil || again.ETag != snapshot1.ETag {
		t.Errorf("snapshot changed without new content: %+v → %+v", snapshot1, again)
	}

	site.mu.Lock()
	site.credits = "9"
	site.mu.Unlock()
	third := svc.RunCycle(ctx)
	snapshot3, _ := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir)
	if third.Result != "ok" || !third.Published || snapshot3 == nil || snapshot3.ETag == snapshot1.ETag {
		t.Fatalf("third cycle = %+v, snapshot %+v", third, snapshot3)
	}
}

// A new release may read the archive differently. Radix builds again at start when another
// binary built the catalog, and publishes what the new rules make of it without asking BTU.
func TestANewReleaseRebuildsFromTheArchiveWithoutCrawling(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	ctx := context.Background()

	if !svc.BuiltByOtherRelease() {
		t.Fatal("a database never built does not count as built by another release")
	}
	if first := svc.RunCycle(ctx); first.Result != "ok" || !first.Published {
		t.Fatalf("first cycle = %+v", first)
	}
	if svc.BuiltByOtherRelease() {
		t.Fatal("the catalog this binary built counts as built by another release")
	}

	// The release before: another binary, whose rules made other content of the same pages.
	if _, err := svc.db.SQL().Exec(`UPDATE meta SET value = 'other' WHERE key IN ('radix_build', 'content_digest')`); err != nil {
		t.Fatal(err)
	}
	if !svc.BuiltByOtherRelease() {
		t.Fatal("a catalog another binary built is not noticed")
	}
	site.mu.Lock()
	hits := maps.Clone(site.hits)
	site.mu.Unlock()

	rebuilt := svc.Rebuild(ctx)
	if rebuilt.Result != "ok" || !rebuilt.Published {
		t.Fatalf("rebuild = %+v", rebuilt)
	}
	for _, s := range rebuilt.Stages {
		if s.Crawl != nil || s.Skipped == "outside the off-peak window" {
			t.Errorf("the rebuild ran crawl stage %+v", s)
		}
	}
	site.mu.Lock()
	defer site.mu.Unlock()
	if !maps.Equal(hits, site.hits) {
		t.Errorf("the rebuild asked BTU: %v → %v", hits, site.hits)
	}
	if s := stage(rebuilt, "export"); s.Skipped != "" || s.Error != "" {
		t.Errorf("export stage = %+v", s)
	}
	if svc.BuiltByOtherRelease() {
		t.Error("after the rebuild the catalog still counts as built by another release")
	}
}

// column runs a query and joins the first column of its rows with commas.
func column(t *testing.T, db *catalogdb.DB, query string) string {
	t.Helper()
	rows, err := db.SQL().Query(query)
	if err != nil {
		t.Fatalf("%s: %v", query, err)
	}
	defer rows.Close()
	var values []string
	for rows.Next() {
		var v string
		if err := rows.Scan(&v); err != nil {
			t.Fatalf("%s: %v", query, err)
		}
		values = append(values, v)
	}
	if err := rows.Err(); err != nil {
		t.Fatalf("%s: %v", query, err)
	}
	return strings.Join(values, ",")
}

// When BTU removes an event, the event search stops showing it and its page turns into the
// empty frame of QIS, while the module description, read again only within a month, still
// names it. The next cycle builds the catalog without the event and without the link, and
// publishes it. The archive keeps what it has of the event while a description names it, so
// that the crawler keeps asking; once none does, the archive stage removes it after its
// grace period.
func TestAnEventBTURemovesLeavesTheCatalogAndTheArchive(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	svc.cfg.ArchiveGrace = 7 * 24 * time.Hour
	ctx := context.Background()

	if r := svc.RunCycle(ctx); r.Result != "ok" || !r.Published {
		t.Fatalf("first cycle = %+v", r)
	}
	if got := column(t, svc.db, "SELECT event_id FROM module_event WHERE module_id = '11881' ORDER BY 1"); got != "120285,120999" {
		t.Fatalf("events of 11881 = %s", got)
	}

	site.mu.Lock()
	site.removed["120999"] = true
	site.mu.Unlock()
	r := svc.RunCycle(ctx)
	if r.Result != "ok" || !r.Published {
		t.Fatalf("cycle after the removal = %+v", r)
	}
	if s := stage(r, "event-list"); s.Crawl == nil || s.Crawl.NotFound != 1 {
		t.Errorf("event-list stage = %+v", s)
	}
	if got := column(t, svc.db, "SELECT id FROM event ORDER BY 1"); got != "120285" {
		t.Errorf("events = %s, want 120999 left out", got)
	}
	if got := column(t, svc.db, "SELECT DISTINCT event_id FROM v_module_schedule WHERE module_id = '11881'"); got != "120285" {
		t.Errorf("schedule of 11881 = %s", got)
	}
	if got := column(t, svc.db, "SELECT source FROM raw_page WHERE key = '120999' ORDER BY 1"); got != "qis_event,qis_event_entry" {
		t.Errorf("archived of 120999 while a description names it: %s", got)
	}

	// The description is read again and no longer names the event. A week later nothing
	// has fetched its rows since, and the archive stage removes them.
	site.mu.Lock()
	site.qisEvents = nil
	site.mu.Unlock()
	age(t, svc.db, catalogdb.SourceQISEvent, "120999", 8*24*time.Hour)
	age(t, svc.db, catalogdb.SourceQISEventEntry, "120999", 8*24*time.Hour)
	if r := svc.RunCycle(ctx); r.Result != "ok" || stage(r, "archive").Error != "" {
		t.Fatalf("cycle after the description = %+v", r)
	}
	if got := column(t, svc.db, "SELECT source FROM raw_page WHERE key = '120999'"); got != "" {
		t.Errorf("still archived of 120999: %s", got)
	}
	if got := column(t, svc.db, "SELECT id FROM event ORDER BY 1"); got != "120285" {
		t.Errorf("events = %s", got)
	}
}

func TestBrokenSourceDegradesButKeepsServing(t *testing.T) {
	site := newFakeBTU(t)
	svc, cfg := newTestService(t, site)
	ctx := context.Background()
	if r := svc.RunCycle(ctx); !r.Published {
		t.Fatalf("setup cycle = %+v", r)
	}
	before, _ := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir)

	site.mu.Lock()
	site.broken["/modul/"] = true
	site.mu.Unlock()
	r := svc.RunCycle(ctx)
	if r.Result != "degraded" || stage(r, "modules").Crawl.Failed != 2 {
		t.Fatalf("cycle with broken module pages = %+v", r)
	}
	after, _ := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir)
	if after == nil || after.ETag != before.ETag {
		t.Errorf("snapshot must stay: %+v → %+v", before, after)
	}
	if st := svc.Status(); !st.Healthy || st.LastSuccessAt == nil {
		t.Errorf("a degraded cycle still serves good data: %+v", st)
	}
}

func TestFailedValidationBlocksPublishingAndTurnsUnhealthy(t *testing.T) {
	site := newFakeBTU(t)
	svc, cfg := newTestService(t, site)
	svc.cfg.Baselines = []catalogdb.Baseline{{Name: "modules", Query: "SELECT COUNT(*) FROM module", Min: 4800}}
	ctx := context.Background()

	srv := httptest.NewServer(svc.Handler())
	defer srv.Close()
	get := func(path string) (int, []byte) {
		resp, err := http.Get(srv.URL + path)
		if err != nil {
			t.Fatal(err)
		}
		defer resp.Body.Close()
		body, _ := io.ReadAll(resp.Body)
		return resp.StatusCode, body
	}

	for i := 0; i < 2; i++ {
		if r := svc.RunCycle(ctx); r.Result != "failed" || r.Published || stage(r, "validate").Error == "" {
			t.Fatalf("cycle %d = %+v", i+1, r)
		}
	}
	if _, err := catalogdb.ReadSnapshotPointer(cfg.SnapshotDir); err == nil {
		t.Fatal("a snapshot was published although validation failed")
	}

	code, body := get("/healthz")
	var health struct {
		Status   string   `json:"status"`
		Problems []string `json:"problems"`
	}
	_ = json.Unmarshal(body, &health)
	if code != http.StatusServiceUnavailable || health.Status != "unhealthy" || len(health.Problems) != 2 {
		t.Errorf("/healthz = %d %s", code, body)
	}

	code, body = get("/status")
	var st Status
	if err := json.Unmarshal(body, &st); err != nil || code != 200 || st.Healthy || st.FailedInARow != 2 || st.Cycles != 2 || st.LastCycle == nil {
		t.Errorf("/status = %d %s (err %v)", code, body, err)
	}
	if code, _ := get("/snapshot/catalog.db"); code != http.StatusServiceUnavailable {
		t.Errorf("/snapshot/catalog.db without snapshot = %d", code)
	}

	// Recovery: with sane baselines the next cycle publishes and health returns.
	svc.cfg.Baselines = nil
	if r := svc.RunCycle(ctx); r.Result != "ok" || !r.Published {
		t.Fatalf("recovery cycle = %+v", r)
	}
	if code, _ := get("/healthz"); code != http.StatusOK {
		t.Errorf("/healthz after recovery = %d", code)
	}
	if code, body := get("/snapshot/catalog.db"); code != http.StatusOK || !strings.HasPrefix(string(body), "SQLite format 3") {
		t.Errorf("/snapshot/catalog.db after recovery = %d", code)
	}
}

func TestBulkCrawlingWaitsForOffPeakExceptOnFirstStart(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	svc.cfg.OffPeakStart, svc.cfg.OffPeakEnd = 1, 6
	svc.now = func() time.Time { return time.Date(2026, 9, 19, 14, 0, 0, 0, time.Local) }
	ctx := context.Background()

	// Empty archive: bootstrap right away, even at 14:00.
	if r := svc.RunCycle(ctx); stage(r, "modules").Crawl == nil || stage(r, "modules").Crawl.Fetched != 2 {
		t.Fatalf("bootstrap cycle = %+v", r)
	}
	// Afterwards bulk stages wait for the night, the module index as well. The event
	// stages run, but by day they only ask about what is not settled and fetch the pages
	// the search has news for: here nothing.
	r := svc.RunCycle(ctx)
	for _, name := range []string{"lists", "modules", "qis-modules", "tree"} {
		if stage(r, name).Skipped == "" {
			t.Errorf("stage %s ran outside the off-peak window: %+v", name, stage(r, name))
		}
	}
	for _, name := range []string{"event-list", "events"} {
		if s := stage(r, name); s.Skipped != "" || s.Crawl == nil || s.Crawl.Fetched != 0 || s.Crawl.Failed != 0 {
			t.Errorf("stage %s by day = %+v", name, s)
		}
	}

	svc.now = func() time.Time { return time.Date(2026, 9, 20, 2, 0, 0, 0, time.Local) }
	if r := svc.RunCycle(ctx); stage(r, "modules").Crawl == nil || stage(r, "modules").Crawl.Fetched != 2 {
		t.Errorf("night cycle = %+v", r)
	}
}
