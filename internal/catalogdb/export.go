package catalogdb

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/oplog"
)

// snapshotDropTables exist for Radix only and are not shipped to readers: the archive, and
// the caches of the semantic search, whose summaries are Gemini's text, not the university's
// (readers get module_vector).
var snapshotDropTables = []string{"raw_page", "module_summary", "passage_embedding"}

const (
	// SnapshotPointer names the current snapshot of a snapshot directory.
	SnapshotPointer = "current.json"

	snapshotPrefix = "catalog-"
	snapshotsKept  = 2
)

// Snapshot is the content of current.json.
type Snapshot struct {
	File       string `json:"file"`  // file name inside the snapshot directory
	ETag       string `json:"etag"`  // strong validator: hash of the file content, quoted
	Bytes      int64  `json:"bytes"` //
	ExportedAt string `json:"exported_at"`
}

// Export writes a read-optimized, self-contained snapshot of the database for
// /api/db into dir and points current.json at it.
//
// The snapshot is a consistent copy (VACUUM INTO reads one transaction, so WAL
// frames the writer has not checkpointed are included), without the raw page
// archive, in rollback-journal mode (one file, safe to serve byte for byte), with
// planner statistics. Its name and ETag are a hash of its content.
//
// A reader may hold the previous snapshot open, which on Windows forbids replacing
// it. Snapshots are therefore never overwritten: a new file is written, the small
// pointer file is replaced atomically, and older snapshots are removed when nobody
// uses them any more.
//
// Log events: export.finished, export.failed (ERROR).
func (db *DB) Export(ctx context.Context, dir string) (*Snapshot, error) {
	start := time.Now()
	snap, err := db.export(ctx, dir)
	if err != nil {
		oplog.For("export").Error("export failed; the previous snapshot stays current", "event", "export.failed", "dir", dir, oplog.Err(err))
		return nil, err
	}
	oplog.For("export").Info("snapshot published", "event", "export.finished", "file", snap.File, "etag", snap.ETag,
		"bytes", snap.Bytes, "duration_ms", time.Since(start).Milliseconds())
	return snap, nil
}

func (db *DB) export(ctx context.Context, dir string) (*Snapshot, error) {
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, err
	}
	tmpPath := filepath.Join(dir, fmt.Sprintf("export-%d.tmp", time.Now().UnixNano()))
	defer os.Remove(tmpPath)

	if _, err := db.sql.ExecContext(ctx, "VACUUM INTO ?", tmpPath); err != nil {
		return nil, fmt.Errorf("failed to copy database: %w", err)
	}
	if err := trimSnapshot(ctx, tmpPath); err != nil {
		return nil, err
	}

	hash, size, err := contentHash(tmpPath)
	if err != nil {
		return nil, err
	}
	snap := &Snapshot{
		File:       snapshotPrefix + hash[:16] + ".db",
		ETag:       `"` + hash[:32] + `"`,
		Bytes:      size,
		ExportedAt: time.Now().UTC().Format(time.RFC3339),
	}

	target := filepath.Join(dir, snap.File)
	if _, err := os.Stat(target); err == nil {
		// Same content as an existing snapshot: keep the file readers may have open.
		_ = os.Remove(tmpPath)
	} else if err := os.Rename(tmpPath, target); err != nil {
		return nil, fmt.Errorf("failed to publish snapshot: %w", err)
	}

	pointer, _ := json.MarshalIndent(snap, "", "  ")
	pointerTmp := filepath.Join(dir, SnapshotPointer+".tmp")
	if err := os.WriteFile(pointerTmp, pointer, 0644); err != nil {
		return nil, err
	}
	if err := os.Rename(pointerTmp, filepath.Join(dir, SnapshotPointer)); err != nil {
		return nil, fmt.Errorf("failed to publish snapshot pointer: %w", err)
	}

	removeOldSnapshots(dir, snap.File)
	return snap, nil
}

// ReadSnapshotPointer returns the current snapshot of a snapshot directory.
func ReadSnapshotPointer(dir string) (*Snapshot, error) {
	data, err := os.ReadFile(filepath.Join(dir, SnapshotPointer))
	if err != nil {
		return nil, err
	}
	var snap Snapshot
	if err := json.Unmarshal(data, &snap); err != nil {
		return nil, fmt.Errorf("invalid %s: %w", SnapshotPointer, err)
	}
	return &snap, nil
}

// removeOldSnapshots keeps the newest snapshots. A file that is still open
// somewhere cannot be removed on Windows; it goes away after a later export.
func removeOldSnapshots(dir, current string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	type old struct {
		name    string
		modTime time.Time
	}
	var olds []old
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || name == current || !strings.HasPrefix(name, snapshotPrefix) || !strings.HasSuffix(name, ".db") {
			continue
		}
		if info, err := e.Info(); err == nil {
			olds = append(olds, old{name, info.ModTime()})
		}
	}
	sort.Slice(olds, func(i, j int) bool { return olds[i].modTime.After(olds[j].modTime) })
	for i, o := range olds {
		if i >= snapshotsKept-1 {
			_ = os.Remove(filepath.Join(dir, o.name))
		}
	}
}

func trimSnapshot(ctx context.Context, path string) error {
	snap, err := sql.Open("sqlite", path+"?_pragma=foreign_keys(ON)")
	if err != nil {
		return err
	}
	defer snap.Close()
	snap.SetMaxOpenConns(1) // the pragmas below are per connection

	for _, table := range snapshotDropTables {
		if _, err := snap.ExecContext(ctx, "DROP TABLE IF EXISTS "+table); err != nil {
			return fmt.Errorf("failed to drop %s: %w", table, err)
		}
	}

	var violations int
	if err := snap.QueryRowContext(ctx, "SELECT COUNT(*) FROM pragma_foreign_key_check").Scan(&violations); err != nil {
		return err
	}
	if violations > 0 {
		return fmt.Errorf("snapshot has %d foreign key violations; run validate", violations)
	}

	for _, stmt := range []string{"PRAGMA journal_mode = DELETE", "ANALYZE", "VACUUM"} {
		if _, err := snap.ExecContext(ctx, stmt); err != nil {
			return fmt.Errorf("%s failed: %w", stmt, err)
		}
	}
	return snap.Close()
}

func contentHash(path string) (string, int64, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", 0, err
	}
	defer f.Close()
	h := sha256.New()
	size, err := io.Copy(h, f)
	if err != nil {
		return "", 0, err
	}
	return hex.EncodeToString(h.Sum(nil)), size, nil
}
