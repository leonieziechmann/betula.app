package snapshothttp

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

func get(t *testing.T, url, ifNoneMatch string) (*http.Response, []byte) {
	t.Helper()
	req, _ := http.NewRequest(http.MethodGet, url, nil)
	if ifNoneMatch != "" {
		req.Header.Set("If-None-Match", ifNoneMatch)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("GET %s failed: %v", url, err)
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	return resp, body
}

func TestPollingClientOnlyDownloadsChangedSnapshots(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	dir := filepath.Join(t.TempDir(), "snapshot")
	srv := httptest.NewServer(Handler(dir))
	defer srv.Close()

	// Before the first export the web server has to wait.
	if resp, _ := get(t, srv.URL+DatabasePath, ""); resp.StatusCode != http.StatusServiceUnavailable || resp.Header.Get("Retry-After") == "" {
		t.Fatalf("without snapshot: status %d", resp.StatusCode)
	}

	first, err := db.Export(context.Background(), dir)
	if err != nil {
		t.Fatalf("Export failed: %v", err)
	}
	resp, body := get(t, srv.URL+DatabasePath, "")
	if resp.StatusCode != http.StatusOK || resp.Header.Get("ETag") != first.ETag || int64(len(body)) != first.Bytes {
		t.Fatalf("first download: status %d, etag %s, %d bytes; want %+v", resp.StatusCode, resp.Header.Get("ETag"), len(body), first)
	}
	if string(body[:15]) != "SQLite format 3" {
		t.Fatalf("body is not a SQLite file: %q", body[:15])
	}

	// Next poll: nothing changed, nothing is transferred.
	if resp, body := get(t, srv.URL+DatabasePath, first.ETag); resp.StatusCode != http.StatusNotModified || len(body) != 0 {
		t.Fatalf("unchanged poll: status %d, %d bytes", resp.StatusCode, len(body))
	}

	resp, body = get(t, srv.URL+PointerPath, "")
	var pointer catalogdb.Snapshot
	if err := json.Unmarshal(body, &pointer); err != nil || pointer != *first || resp.Header.Get("ETag") != first.ETag {
		t.Fatalf("pointer = %+v (err %v)", pointer, err)
	}

	// A new export is picked up by the same conditional request.
	if _, err := db.SQL().Exec("INSERT INTO meta (key, value) VALUES ('built_at', '2026-09-20T00:00:00Z')"); err != nil {
		t.Fatal(err)
	}
	second, err := db.Export(context.Background(), dir)
	if err != nil {
		t.Fatalf("second Export failed: %v", err)
	}
	if resp, body := get(t, srv.URL+DatabasePath, first.ETag); resp.StatusCode != http.StatusOK || resp.Header.Get("ETag") != second.ETag || int64(len(body)) != second.Bytes {
		t.Fatalf("changed poll: status %d, etag %s; want %s", resp.StatusCode, resp.Header.Get("ETag"), second.ETag)
	}

	if resp, _ := get(t, srv.URL+"/snapshot/../v2.db", ""); resp.StatusCode == http.StatusOK {
		t.Fatal("handler serves files outside the snapshot contract")
	}
}
