package snapshothttp

import (
	"net/http"
	"path"
	"strconv"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
)

// How the web server takes the snapshots, and which one it is offered. In both modes,
// "run" and "serve-snapshot".
var requestsTotal = metrics.Default.NewCounter("radix_snapshot_requests_total",
	"Requests for the snapshot by file (catalog.db, current.json) and status: 200 a download, 304 the web server has it already.",
	"file", "code")

// served is the snapshot directory the scrape-time gauges read: the one Handler serves.
var served atomic.Pointer[string]

func init() {
	for _, file := range []string{"catalog.db", "current.json"} {
		for _, code := range []string{"200", "304", "503"} {
			requestsTotal.Add(0, file, code) // there from the start: increase() misses a series' first count
		}
	}
	pointer := func(read func(snap *catalogdb.Snapshot, emit func(float64, ...string))) func(func(float64, ...string)) {
		return func(emit func(float64, ...string)) {
			dir := served.Load()
			if dir == nil {
				return
			}
			if snap, err := catalogdb.ReadSnapshotPointer(*dir); err == nil {
				read(snap, emit)
			}
		}
	}
	metrics.Default.NewGaugeFunc("radix_snapshot_exported_timestamp_seconds",
		"When the snapshot the web server is offered was exported (Unix time). Absent while there is none.", nil,
		pointer(func(snap *catalogdb.Snapshot, emit func(float64, ...string)) {
			if at, err := time.Parse(time.RFC3339, snap.ExportedAt); err == nil {
				emit(float64(at.Unix()))
			}
		}))
	metrics.Default.NewGaugeFunc("radix_snapshot_bytes",
		"Size of the snapshot the web server is offered.", nil,
		pointer(func(snap *catalogdb.Snapshot, emit func(float64, ...string)) { emit(float64(snap.Bytes)) }))
}

func counted(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		sw := &statusWriter{ResponseWriter: w, status: http.StatusOK}
		next(sw, r)
		requestsTotal.Inc(path.Base(r.URL.Path), strconv.Itoa(sw.status))
	}
}
