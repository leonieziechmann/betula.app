package service

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
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
	srv     *httptest.Server
	mu      sync.Mutex
	credits string
	broken  map[string]bool // path prefix → answer 500
	hits    map[string]int
}

func newFakeBTU(t *testing.T) *fakeBTU {
	f := &fakeBTU{credits: "8", broken: make(map[string]bool), hits: make(map[string]int)}
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
		TreeRootURL:   f.srv.URL + "/tree?nodeID=auswahlBaum",
	}
}

const poNodeID = "auswahlBaum|studiengang:stg=079|abschluss:abschl=82|stgSpecials:vert=,schwp=,kzfa=H,pversion=2008"

func (f *fakeBTU) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	credits := f.credits
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
			<tr><td>Zuordnung zu Studiengängen:</td><td><ul><li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>
			%s</table></div>`, id, id, id, credits, events)
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
			events = fmt.Sprintf(`<tr><td class="tabelle1_alignleft">Veranstaltungen im aktuellen Semester:</td>`+
				`<td class="tabelle2inhalt"><ul><li><a href="%s/event?veranstaltung.veranstid=120999">120999 Vorlesung</a></li></ul></td></tr>`, f.srv.URL)
		}
		fmt.Fprintf(w, `<table cellpadding="5">
			<tr><td class="tabelle1_alignleft">Modulnummer:</td><td class="tabelle2inhalt">%s</td></tr>
			<tr><td class="tabelle1_alignleft">Modultitel:</td><td class="tabelle2inhalt">Modul %s</td></tr>
			<tr><td class="tabelle1_alignleft">Leistungspunkte:</td><td class="tabelle2inhalt">%s</td></tr>
			<tr><td class="tabelle1_alignleft">Angebotsturnus:</td><td class="tabelle2inhalt">jedes Wintersemester</td></tr>
			<tr><td class="tabelle1_alignleft">Zuordnung zu Studiengängen:</td><td class="tabelle2inhalt"><ul>
				<li>Bachelor (universitär) / Informatik / PO 2008 - 2. SÄ 2024</li></ul></td></tr>
			%s</table>`, id, id, credits, events)
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
		Lists:       fast, Modules: fast, QISModules: fast, Events: fast, Tree: fast,
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
	// One event from the copy on b-tu.de, one only the QIS description names.
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
	// Afterwards bulk stages wait for the night; the lists are still checked.
	r := svc.RunCycle(ctx)
	for _, name := range []string{"modules", "tree", "events"} {
		if stage(r, name).Skipped == "" {
			t.Errorf("stage %s ran outside the off-peak window: %+v", name, stage(r, name))
		}
	}
	if stage(r, "lists").Crawl == nil || stage(r, "lists").Crawl.Fetched != 2 {
		t.Errorf("lists stage = %+v", stage(r, "lists"))
	}

	svc.now = func() time.Time { return time.Date(2026, 9, 20, 2, 0, 0, 0, time.Local) }
	if r := svc.RunCycle(ctx); stage(r, "modules").Crawl == nil || stage(r, "modules").Crawl.Fetched != 2 {
		t.Errorf("night cycle = %+v", r)
	}
}
