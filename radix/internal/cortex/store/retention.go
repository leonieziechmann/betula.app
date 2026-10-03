package store

import (
	"database/sql"
	"fmt"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// PruneStats is what one run of retention removed.
type PruneStats struct {
	Cutoff       time.Time `json:"cutoff"`
	Versions     int64     `json:"versions"`
	FileVersions int64     `json:"file_versions"`
	Files        int64     `json:"files"`
	Entries      int64     `json:"entries"`
}

func (p PruneStats) empty() bool {
	return p.Versions == 0 && p.FileVersions == 0 && p.Files == 0 && p.Entries == 0
}

func (p PruneStats) count() {
	prunedTotal.Add(float64(p.Versions), "versions")
	prunedTotal.Add(float64(p.FileVersions), "file_versions")
	prunedTotal.Add(float64(p.Files), "files")
	prunedTotal.Add(float64(p.Entries), "entries")
}

// Prune applies the retention (owner, 2026-10-02): current versions stay for ever, a superseded
// one for history after it was superseded. With cutoff = now − history it removes the versions
// and file versions superseded before cutoff, the files whose current version is a tombstone
// created before cutoff (with all their versions), and the entries and files left without a
// version (op prune, payload {cutoff}). A follower runs the same statements with the same
// cutoff on the same rows, so the result is the same. A run that removes nothing writes
// nothing and returns a zero JournalEntry. Log events: prune.finished.
func (s *Store) Prune(now time.Time, history time.Duration) (PruneStats, JournalEntry, error) {
	if history <= 0 {
		return PruneStats{}, JournalEntry{}, fmt.Errorf("prune: history must be positive, not %s", history)
	}
	start := time.Now()
	now = stamp(now)
	cutoff := FormatTime(now.Add(-history))

	stats, e, err := s.prune(now, cutoff)
	if err != nil {
		return PruneStats{}, JournalEntry{}, err
	}
	stats.count()
	oplog.For("store").Info("retention finished", "event", "prune.finished", "cutoff", cutoff,
		"versions", stats.Versions, "file_versions", stats.FileVersions, "files", stats.Files, "entries", stats.Entries,
		"seq", e.Seq, "duration_ms", time.Since(start).Milliseconds())
	return stats, e, nil
}

func (s *Store) prune(now time.Time, cutoff string) (PruneStats, JournalEntry, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return PruneStats{}, JournalEntry{}, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return PruneStats{}, JournalEntry{}, err
	}
	defer func() { _ = tx.Rollback() }()
	stats, err := applyPrune(tx, cutoff)
	if err != nil {
		return PruneStats{}, JournalEntry{}, err
	}
	if stats.empty() {
		return stats, JournalEntry{}, nil // the rollback leaves the index as it was
	}
	e, err := s.commit(tx, OpPrune, now, prunePayload{Cutoff: cutoff}, "")
	if err != nil {
		return PruneStats{}, JournalEntry{}, err
	}
	return stats, e, nil
}

// applyPrune removes what retention removes at cutoff (TimeFormat) and counts it.
func applyPrune(tx *sql.Tx, cutoff string) (PruneStats, error) {
	var stats PruneStats
	var err error
	if stats.Cutoff, err = parseTime(cutoff); err != nil {
		return PruneStats{}, fmt.Errorf("prune: %w", err)
	}
	if stats.Versions, err = execCount(tx, `DELETE FROM version WHERE superseded_at < ?`, cutoff); err != nil {
		return PruneStats{}, fmt.Errorf("failed to remove superseded versions: %w", err)
	}
	if stats.FileVersions, err = execCount(tx, `DELETE FROM file_version WHERE superseded_at < ?`, cutoff); err != nil {
		return PruneStats{}, fmt.Errorf("failed to remove superseded file versions: %w", err)
	}

	// A file deleted before cutoff goes with its tombstone (and the versions that are left).
	const deletedFiles = `SELECT f.id FROM file f JOIN file_version v ON v.id = f.current_version
		WHERE v.deleted = 1 AND v.created_at < ?`
	var cascaded int64
	if err := tx.QueryRow(`SELECT COUNT(*) FROM file_version WHERE file_id IN (`+deletedFiles+`)`, cutoff).Scan(&cascaded); err != nil {
		return PruneStats{}, fmt.Errorf("failed to count the versions of deleted files: %w", err)
	}
	if stats.Files, err = execCount(tx, `DELETE FROM file WHERE id IN (`+deletedFiles+`)`, cutoff); err != nil {
		return PruneStats{}, fmt.Errorf("failed to remove deleted files: %w", err)
	}
	stats.FileVersions += cascaded

	if stats.Entries, err = execCount(tx, `DELETE FROM entry WHERE NOT EXISTS (SELECT 1 FROM version v WHERE v.entry_id = entry.id)`); err != nil {
		return PruneStats{}, fmt.Errorf("failed to remove entries without a version: %w", err)
	}
	empty, err := execCount(tx, `DELETE FROM file WHERE NOT EXISTS (SELECT 1 FROM file_version v WHERE v.file_id = file.id)`)
	if err != nil {
		return PruneStats{}, fmt.Errorf("failed to remove files without a version: %w", err)
	}
	stats.Files += empty
	return stats, nil
}

func execCount(tx *sql.Tx, query string, args ...any) (int64, error) {
	res, err := tx.Exec(query, args...)
	if err != nil {
		return 0, err
	}
	return res.RowsAffected()
}
