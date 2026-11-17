package service

import (
	"context"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// TestCrawlFUESCatalogs: the page „Aktuelle Infos" links the winter's catalog; the summer's, which
// it no longer links, comes by the name of its file while the archive has no copy. A second run
// within MaxAge asks nothing; a page that links none still finds the current one by its name.
func TestCrawlFUESCatalogs(t *testing.T) {
	var mu sync.Mutex
	var asked []string
	links := true
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		asked = append(asked, r.URL.Path)
		withLink := links
		mu.Unlock()
		switch r.URL.Path {
		case "/aktuelle-infos":
			if withLink {
				_, _ = w.Write([]byte(`<h3>FÜS-Modulkatalog zum Wintersemester 2026/2027</h3><a href="/files/F%c3%9cS_Katalog_WiSe_2026_27.pdf">FÜS-Modulkatalog zum Wintersemester 2026/2027</a>`))
			} else {
				_, _ = w.Write([]byte(`<p>Nichts Neues.</p>`))
			}
		case "/files/FÜS_Katalog_WiSe_2026_27.pdf":
			_, _ = w.Write([]byte("%PDF winter"))
		case "/files/FÜS_Katalog_SoSe_2026.pdf":
			_, _ = w.Write([]byte("%PDF summer"))
		default:
			http.NotFound(w, r)
		}
	}))
	defer srv.Close()

	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "radix.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	ep := Endpoints{FUESCatalogPage: srv.URL + "/aktuelle-infos", FUESCatalogFiles: srv.URL + "/files/"}
	now := time.Date(2026, 11, 17, 12, 0, 0, 0, time.UTC)
	ctx := context.Background()

	stats, err := CrawlFUESCatalogs(ctx, db, ep, Pace{MaxAge: time.Hour}, now)
	if err != nil || stats.Fetched != 3 || stats.Failed != 0 {
		t.Fatalf("first run: %+v, %v (asked %v)", stats, err, asked)
	}
	for key, body := range map[string]string{"2026W": "%PDF winter", "2026S": "%PDF summer"} {
		p, err := db.GetPage(catalogdb.SourceBTUFUESCatalog, key)
		if err != nil || p.HTTPStatus != 200 || string(p.Body) != body {
			t.Errorf("%s: %+v, %v", key, p, err)
		}
	}

	asked = nil
	if stats, err := CrawlFUESCatalogs(ctx, db, ep, Pace{MaxAge: time.Hour}, now); err != nil || stats.Fetched != 0 || len(asked) != 0 {
		t.Errorf("second run within MaxAge: %+v, %v, asked %v", stats, err, asked)
	}

	// A page that links none, a day later, in a fresh archive: both by their names.
	db2, err := catalogdb.Open(filepath.Join(t.TempDir(), "radix2.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db2.Close()
	mu.Lock()
	links, asked = false, nil
	mu.Unlock()
	stats, err = CrawlFUESCatalogs(ctx, db2, ep, Pace{MaxAge: time.Hour}, now.Add(24*time.Hour))
	if err != nil || stats.Fetched != 3 {
		t.Errorf("without a link: %+v, %v, asked %v", stats, err, asked)
	}
	if p, err := db2.GetPage(catalogdb.SourceBTUFUESCatalog, "2026W"); err != nil || string(p.Body) != "%PDF winter" {
		t.Errorf("the winter's by its name: %+v, %v", p, err)
	}
}
