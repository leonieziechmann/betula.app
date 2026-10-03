package store

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/telemetry"
)

// Stats is the size of the store.
type Stats struct {
	Entries           int64 `json:"entries"`
	Versions          int64 `json:"versions"`
	Files             int64 `json:"files"` // files with content (not deleted)
	FileVersions      int64 `json:"file_versions"`
	Blobs             int64 `json:"blobs"`               // blob files on disk
	BlobBytes         int64 `json:"blob_bytes"`          // as stored
	BlobOriginalBytes int64 `json:"blob_original_bytes"` // before compression (see countBlobs)
}

// Stats counts the rows of the index and walks the blob files. It reads every blob directory;
// RecentStats is the cheap way to ask often, and the one that never waits for the walk.
func (s *Store) Stats() (Stats, error) {
	var st Stats
	if err := s.countRows(&st); err != nil {
		return Stats{}, err
	}
	beforeBlobWalk()
	dirs, err := os.ReadDir(s.blobRoot())
	if err != nil {
		return Stats{}, fmt.Errorf("failed to read %s: %w", s.blobRoot(), err)
	}
	for _, d := range dirs {
		if d.IsDir() && isHexPair(d.Name()) {
			if err := s.countBlobs(d.Name(), &st); err != nil {
				return Stats{}, err
			}
		}
	}
	return st, nil
}

// beforeBlobWalk is called by Stats before it walks the blobs, a variable so that a test can
// make the walk as slow as that of a large store.
var beforeBlobWalk = func() {}

func (s *Store) countRows(st *Stats) error {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return err
	}
	err := s.r.QueryRow(`SELECT (SELECT COUNT(*) FROM entry), (SELECT COUNT(*) FROM version),
		(SELECT COUNT(*) FROM file f JOIN file_version v ON v.id = f.current_version WHERE v.deleted = 0),
		(SELECT COUNT(*) FROM file_version)`).Scan(&st.Entries, &st.Versions, &st.Files, &st.FileVersions)
	if err != nil {
		return fmt.Errorf("failed to count the index: %w", err)
	}
	return nil
}

// countBlobs adds the blob files of blobs/sha256/<hh>. The original size of a compressed blob
// comes from the index, or from gzip's trailer when no row references it: modulo 4 GiB, so a
// lower bound for such a blob of 4 GiB or more (good enough for a gauge; StatBlob, which a
// Content-Length comes from, does not guess).
func (s *Store) countBlobs(hh string, st *Stats) error {
	s.mu.RLock()
	if err := s.ready(); err != nil {
		s.mu.RUnlock()
		return err
	}
	sizes, err := s.referencedWithPrefix(hh)
	s.mu.RUnlock()
	if err != nil {
		return err
	}
	dir := filepath.Join(s.blobRoot(), hh)
	files, err := os.ReadDir(dir)
	if err != nil {
		return fmt.Errorf("failed to read %s: %w", dir, err)
	}
	for _, f := range files {
		hash, gz, ok := parseBlobName(f.Name())
		if !ok {
			continue
		}
		info, err := f.Info()
		if errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err != nil {
			return fmt.Errorf("failed to stat %s: %w", f.Name(), err)
		}
		st.Blobs++
		st.BlobBytes += info.Size()
		switch size, known := sizes[hash]; {
		case !gz:
			st.BlobOriginalBytes += info.Size()
		case known:
			st.BlobOriginalBytes += size
		default:
			if size, err := trailerSize(filepath.Join(dir, f.Name()), info.Size()); err == nil {
				st.BlobOriginalBytes += size
			}
		}
	}
	return nil
}

func trailerSize(path string, size int64) (int64, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer f.Close()
	return gzipTrailerSize(f, size)
}

// statsCache keeps the last Stats: the metrics read it at every scrape and /status at every
// call, and counting walks every blob directory (seconds for 100,000 blobs, E2E-5).
type statsCache struct {
	mu         sync.Mutex
	at         time.Time // when the numbers were counted (or the count failed); zero: stale
	stats      Stats
	valid      bool
	refreshing bool   // a count runs in the background
	gen        uint64 // moved by invalidate: a count that began before is stale when it ends
}

func (c *statsCache) invalidate() {
	c.mu.Lock()
	c.at = time.Time{}
	c.gen++
	c.mu.Unlock()
}

// statsMaxAge is how old the numbers RecentStats returns may become before it has them
// counted again.
const statsMaxAge = time.Minute

// ErrStatsPending is returned by RecentStats before the store has been counted once.
var ErrStatsPending = errors.New("the store is being counted")

// RecentStats returns the last Stats counted, at most about a minute old, without waiting for
// a count: when the numbers are older (or InvalidateStats was called) it starts one in the
// background and returns the old numbers meanwhile. Before the first count has finished it
// returns ErrStatsPending. A failed count keeps the last good numbers (a gap would read as an
// empty store) and is tried again a minute later.
//
// Log events: metrics.stats_failed (WARN).
func (s *Store) RecentStats() (Stats, error) {
	c := &s.stats
	c.mu.Lock()
	defer c.mu.Unlock()
	if (c.at.IsZero() || time.Since(c.at) >= statsMaxAge) && !c.refreshing {
		c.refreshing = true
		go s.countStats(c.gen)
	}
	if !c.valid {
		return Stats{}, ErrStatsPending
	}
	return c.stats, nil
}

// RefreshStats counts the store now, waiting for it, and keeps the numbers for RecentStats.
func (s *Store) RefreshStats() (Stats, error) {
	c := &s.stats
	c.mu.Lock()
	gen := c.gen
	c.mu.Unlock()
	return s.storeStats(gen)
}

// InvalidateStats has the next RecentStats count again (after retention or a new index).
func (s *Store) InvalidateStats() {
	s.stats.invalidate()
}

// countStats is RecentStats' count in the background.
func (s *Store) countStats(gen uint64) {
	_, _ = s.storeStats(gen)
	c := &s.stats
	c.mu.Lock()
	c.refreshing = false
	c.mu.Unlock()
}

// storeStats counts and keeps the numbers, unless invalidate was called after gen was read:
// they may then be of the index before, and the next RecentStats counts again.
func (s *Store) storeStats(gen uint64) (Stats, error) {
	st, err := s.Stats()
	c := &s.stats
	c.mu.Lock()
	defer c.mu.Unlock()
	if err != nil {
		if !errors.Is(err, ErrClosed) {
			oplog.For("store").Warn("cannot count the store", "event", "metrics.stats_failed", oplog.Err(err))
		}
		if c.gen == gen {
			c.at = time.Now() // not again at every call
		}
		return Stats{}, err
	}
	c.stats, c.valid = st, true
	if c.gen == gen {
		c.at = time.Now()
	}
	return st, nil
}

// ReferencedBlobs calls fn with every blob hash a version or a file version references, once
// each, in hash order. It reads the index a page at a time and calls fn between the pages, so a
// slow fn (a follower fetching what it lacks) holds nothing.
func (s *Store) ReferencedBlobs(fn func(hash string) error) error {
	after := ""
	for {
		page, err := s.referencedPage(after, referencedPage)
		if err != nil {
			return err
		}
		for _, hash := range page {
			if err := fn(hash); err != nil {
				return err
			}
		}
		if len(page) < referencedPage {
			return nil
		}
		after = page[len(page)-1]
	}
}

// referencedPage is how many hashes ReferencedBlobs reads at a time.
var referencedPage = 1000

func (s *Store) referencedPage(after string, limit int) ([]string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return nil, err
	}
	rows, err := s.r.Query(`SELECT sha256 FROM version WHERE sha256 > ?1
		UNION SELECT sha256 FROM file_version WHERE deleted = 0 AND sha256 > ?1
		ORDER BY 1 LIMIT ?2`, after, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to read the blob references: %w", err)
	}
	defer rows.Close()
	var page []string
	for rows.Next() {
		var hash string
		if err := rows.Scan(&hash); err != nil {
			return nil, err
		}
		page = append(page, hash)
	}
	return page, rows.Err()
}

// The metrics of the store, in telemetry.Registry. The gauges read the store that was opened
// last (a process has one) through RecentStats, so a scrape never waits for a count; before the
// first count they have no sample.
var (
	active atomic.Pointer[Store]

	prunedTotal = telemetry.Registry.NewCounter("cortex_pruned_total",
		"What retention removed: versions, file_versions, files, entries (Prune and Apply), blobs (GCBlobs), journal_entries (TrimJournal).",
		"what")
)

func init() {
	// increase() cannot see the first count of a series that appears in the middle of a time range.
	for _, what := range []string{"versions", "file_versions", "files", "entries", "blobs", "journal_entries"} {
		prunedTotal.Add(0, what)
	}
	gauge := func(name, help string, read func(Stats) int64) {
		telemetry.Registry.NewGaugeFunc(name, help, nil, func(emit func(float64, ...string)) {
			s := active.Load()
			if s == nil {
				return
			}
			if st, err := s.RecentStats(); err == nil {
				emit(float64(read(st)))
			}
		})
	}
	gauge("cortex_entries", "URLs in the index (counted at most a minute ago).", func(st Stats) int64 { return st.Entries })
	gauge("cortex_versions", "Versions of URLs in the index, current and superseded.", func(st Stats) int64 { return st.Versions })
	gauge("cortex_files", "Named files with content (not deleted).", func(st Stats) int64 { return st.Files })
	gauge("cortex_blobs", "Blob files on disk.", func(st Stats) int64 { return st.Blobs })
	gauge("cortex_blob_bytes", "Bytes of the blob files on disk, as stored (compressed or raw).", func(st Stats) int64 { return st.BlobBytes })
	gauge("cortex_blob_original_bytes", "Bytes of the blobs before compression.", func(st Stats) int64 { return st.BlobOriginalBytes })
}
