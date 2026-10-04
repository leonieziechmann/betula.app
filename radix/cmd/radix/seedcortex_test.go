package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"path/filepath"
	"sort"
	"sync"
	"testing"
	"time"

	cortexclient "github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// seeded is one import a fake Cortex received.
type seeded struct {
	query url.Values
	body  string
}

func TestSeedCortexGivesTheWholeAnswersWithTheirTimes(t *testing.T) {
	path := filepath.Join(t.TempDir(), "radix.db")
	db, err := catalogdb.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	first := time.Date(2026, 9, 27, 23, 21, 45, 0, time.UTC)
	last := time.Date(2026, 10, 4, 3, 41, 1, 0, time.UTC)
	for _, p := range []catalogdb.RawPage{
		{Source: catalogdb.SourceModulePage, Key: "11101", URL: "https://www.b-tu.de/modul/11101", HTTPStatus: 200, Body: []byte("<html>Übung</html>"), FetchedAt: first},
		// The same content again: changed_at stays at the first fetch.
		{Source: catalogdb.SourceModulePage, Key: "11101", URL: "https://www.b-tu.de/modul/11101", HTTPStatus: 200, Body: []byte("<html>Übung</html>"), FetchedAt: last},
		{Source: catalogdb.SourceModulePage, Key: "404", URL: "https://www.b-tu.de/modul/404", HTTPStatus: 404, FetchedAt: last},
		{Source: catalogdb.SourceQISTree, Key: "root", URL: "https://qis.b-tu.de/qisserver/rds?state=wtree&search=1&trex=step&root120262=1|2#top", HTTPStatus: 200, Body: []byte("tree"), FetchedAt: last},
		// A piece of the event search, under the URL of its event's page: never given.
		{Source: catalogdb.SourceQISEventEntry, Key: "4711", URL: "https://qis.b-tu.de/qisserver/rds?state=verpublish&veranstaltung.veranstid=4711", HTTPStatus: 200, Body: []byte("entry"), FetchedAt: last},
		{Source: catalogdb.SourceQISEvent, Key: "4711", URL: "https://qis.b-tu.de/qisserver/rds?state=verpublish&veranstaltung.veranstid=4711", HTTPStatus: 200, Body: []byte("page"), FetchedAt: last},
	} {
		if err := db.PutPage(p); err != nil {
			t.Fatal(err)
		}
	}
	db.Close()

	var mu sync.Mutex
	var got []seeded
	cortex := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		if r.Method != http.MethodPut || r.URL.Path != "/v1/entries" {
			http.Error(w, "unexpected", http.StatusTeapot)
			return
		}
		mu.Lock()
		got = append(got, seeded{query: r.URL.Query(), body: string(body)})
		mu.Unlock()
		w.WriteHeader(http.StatusCreated)
		fmt.Fprint(w, `{"result":"created","version":{"id":1,"sha256":"sha256:x"}}`)
	}))
	defer cortex.Close()
	client, err := cortexclient.New(cortex.URL, cortexclient.Options{})
	if err != nil {
		t.Fatal(err)
	}

	archive, err := catalogdb.OpenReadOnly(path)
	if err != nil {
		t.Fatal(err)
	}
	defer archive.Close()
	stats, err := seedCortex(context.Background(), archive, client, catalogdb.WholeAnswerSources, 3)
	if err != nil || stats.Pages != 4 || stats.Results[cortexclient.ImportCreated] != 4 || stats.Failed != 0 {
		t.Fatalf("seedCortex = %+v, %v; want 4 pages created", stats, err)
	}

	sort.Slice(got, func(a, b int) bool { return got[a].query.Get("url") < got[b].query.Get("url") })
	sum := func(s string) string {
		h := sha256.Sum256([]byte(s))
		return "sha256:" + hex.EncodeToString(h[:])
	}
	want := []struct{ url, status, fetched, checked, source, expect, body string }{
		{"https://qis.b-tu.de/qisserver/rds?state=verpublish&veranstaltung.veranstid=4711", "200", "2026-10-04T03:41:01Z", "2026-10-04T03:41:01Z", "qis_event", sum("page"), "page"},
		{"https://qis.b-tu.de/qisserver/rds?state=wtree&search=1&trex=step&root120262=1|2", "200", "2026-10-04T03:41:01Z", "2026-10-04T03:41:01Z", "qis_tree", sum("tree"), "tree"},
		{"https://www.b-tu.de/modul/11101", "200", "2026-09-27T23:21:45Z", "2026-10-04T03:41:01Z", "module_page", sum("<html>Übung</html>"), "<html>Übung</html>"},
		{"https://www.b-tu.de/modul/404", "404", "2026-10-04T03:41:01Z", "2026-10-04T03:41:01Z", "module_page", "", ""},
	}
	if len(got) != len(want) {
		t.Fatalf("Cortex got %d imports, want %d: %+v", len(got), len(want), got)
	}
	for i, w := range want {
		q := got[i].query
		if q.Get("url") != w.url || q.Get("status") != w.status || q.Get("fetched_at") != w.fetched || q.Get("checked_at") != w.checked ||
			q.Get("source") != w.source || q.Get("expect") != w.expect || got[i].body != w.body {
			t.Errorf("import %d: %v body %q, want %+v", i, q, got[i].body, w)
		}
	}
}

// A seed that Cortex refuses stops after a few pages instead of trying every page.
func TestSeedCortexStopsWhenCortexRefusesEverything(t *testing.T) {
	path := filepath.Join(t.TempDir(), "radix.db")
	db, err := catalogdb.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 3*seedMaxFailuresInRow; i++ {
		if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceModulePage, Key: fmt.Sprint(i), URL: fmt.Sprintf("https://www.b-tu.de/modul/%d", i),
			HTTPStatus: 200, Body: []byte("x"), FetchedAt: time.Now()}); err != nil {
			t.Fatal(err)
		}
	}
	db.Close()
	cortex := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cortex-Error", "internal")
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, `{"error":"internal","message":"the disk is full"}`)
	}))
	defer cortex.Close()
	client, _ := cortexclient.New(cortex.URL, cortexclient.Options{})
	archive, err := catalogdb.OpenReadOnly(path)
	if err != nil {
		t.Fatal(err)
	}
	defer archive.Close()
	stats, err := seedCortex(context.Background(), archive, client, catalogdb.WholeAnswerSources, 1)
	if err == nil || !stats.Abandoned || stats.Failed != seedMaxFailuresInRow {
		t.Fatalf("seedCortex = %+v, %v; want it given up after %d failures", stats, err, seedMaxFailuresInRow)
	}
}
