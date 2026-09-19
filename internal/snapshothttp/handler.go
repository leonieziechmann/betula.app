// Package snapshothttp publishes exported catalog snapshots over HTTP. It is the
// only interface between the scraper and the web server: the web server polls
// with If-None-Match, downloads a snapshot when its ETag changed, and
// redistributes it to browsers.
package snapshothttp

import (
	"encoding/json"
	"errors"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/jakob/btu-scraper/internal/catalogdb"
)

const (
	// PointerPath returns the metadata of the current snapshot as JSON.
	PointerPath = "/snapshot/current.json"
	// DatabasePath returns the current snapshot, a SQLite file.
	DatabasePath = "/snapshot/catalog.db"
)

// Handler serves the current snapshot of dir.
//
//	GET /snapshot/current.json  {"file", "etag", "bytes", "exported_at"}
//	GET /snapshot/catalog.db    the SQLite file; ETag is a hash of its content,
//	                            If-None-Match yields 304, Range is supported
//
// Both answer 503 until the first export. Responses are never cached without
// revalidation, so a poll always sees the newest export.
func Handler(dir string) http.Handler {
	mux := http.NewServeMux()

	mux.HandleFunc("GET "+PointerPath, func(w http.ResponseWriter, r *http.Request) {
		snap, ok := current(w, dir)
		if !ok {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("ETag", snap.ETag)
		_ = json.NewEncoder(w).Encode(snap)
	})

	mux.HandleFunc("GET "+DatabasePath, func(w http.ResponseWriter, r *http.Request) {
		snap, ok := current(w, dir)
		if !ok {
			return
		}
		f, err := os.Open(filepath.Join(dir, filepath.Base(snap.File)))
		if err != nil {
			http.Error(w, "snapshot file is missing", http.StatusServiceUnavailable)
			return
		}
		defer f.Close()

		exportedAt, _ := time.Parse(time.RFC3339, snap.ExportedAt)
		w.Header().Set("Content-Type", "application/vnd.sqlite3")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("ETag", snap.ETag)
		// ServeContent answers If-None-Match with 304 from the ETag header set above.
		http.ServeContent(w, r, "", exportedAt, f)
	})

	return mux
}

func current(w http.ResponseWriter, dir string) (*catalogdb.Snapshot, bool) {
	snap, err := catalogdb.ReadSnapshotPointer(dir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			w.Header().Set("Retry-After", "60")
			http.Error(w, "no snapshot has been exported yet", http.StatusServiceUnavailable)
		} else {
			http.Error(w, "snapshot pointer is unreadable", http.StatusInternalServerError)
		}
		return nil, false
	}
	return snap, true
}
