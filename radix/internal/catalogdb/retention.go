package catalogdb

import (
	"fmt"
	"time"
)

// PruneEvents removes archived events that are no longer current, their page and their
// entry in the event search, and leaves a tombstone, so that they are not fetched again
// while a module page still links them:
//
//   - an event whose last date is more than keep ago;
//   - an event that no module page links any more and that has not been refreshed
//     within keep (the crawler only refreshes linked events), whatever its dates.
//
// Nothing of a semester that has not ended is removed, whatever its dates say. QIS
// dates an oral examination "by arrangement" with a placeholder: 186 examinations of
// WiSe 2026/27 carried 27.12.2015 and were deleted by the rule above on 2026-09-21,
// although they are the examinations of the semester that is about to start.
//
// It works on the result of the last build (event.last_date, module_event); the next
// build drops the events from the canonical tables.
func (db *DB) PruneEvents(now time.Time, keep time.Duration) (int, error) {
	cutoffDate := now.Add(-keep).UTC().Format("2006-01-02")
	cutoffTime := now.Add(-keep).UTC().Format(time.RFC3339)
	prunedAt := now.UTC().Format(time.RFC3339)

	tx, err := db.sql.Begin()
	if err != nil {
		return 0, err
	}
	defer func() { _ = tx.Rollback() }()

	today := now.UTC().Format("2006-01-02")
	_, err = tx.Exec(`
		INSERT OR REPLACE INTO event_tombstone (event_id, last_date, pruned_at)
		SELECT e.id, e.last_date, ? FROM event e
		WHERE NOT EXISTS (SELECT 1 FROM semester s WHERE s.key = e.semester_key AND s.ends_on >= ?)
		  AND ((e.last_date IS NOT NULL AND e.last_date < ?)
		    OR (e.fetched_at < ?
		        AND NOT EXISTS (SELECT 1 FROM module_event me WHERE me.event_id = e.id)))`,
		prunedAt, today, cutoffDate, cutoffTime)
	if err != nil {
		return 0, fmt.Errorf("failed to mark events: %w", err)
	}
	var removed int
	err = tx.QueryRow(`SELECT COUNT(DISTINCT key) FROM raw_page WHERE source IN (?, ?) AND key IN (SELECT event_id FROM event_tombstone WHERE pruned_at = ?)`,
		SourceQISEvent, SourceQISEventEntry, prunedAt).Scan(&removed)
	if err != nil {
		return 0, fmt.Errorf("failed to count the events to remove: %w", err)
	}
	if _, err := tx.Exec(`DELETE FROM raw_page WHERE source IN (?, ?) AND key IN (SELECT event_id FROM event_tombstone WHERE pruned_at = ?)`,
		SourceQISEvent, SourceQISEventEntry, prunedAt); err != nil {
		return 0, fmt.Errorf("failed to remove event pages: %w", err)
	}
	return removed, tx.Commit()
}

// EventTombstones returns the IDs of events that were removed by retention.
func (db *DB) EventTombstones() (map[string]bool, error) {
	rows, err := db.sql.Query("SELECT event_id FROM event_tombstone")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make(map[string]bool)
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		result[id] = true
	}
	return result, rows.Err()
}

// PruneArchive removes archived pages that the last build did not use (Report.Unused:
// module pages of modules that left the lists, tree pages the root no longer reaches)
// and that were fetched before cutoff. The grace period keeps a page through a
// short-lived glitch of a list; a zero cutoff removes everything unused.
func (db *DB) PruneArchive(unused map[string][]string, cutoff time.Time) (int, error) {
	tx, err := db.sql.Begin()
	if err != nil {
		return 0, err
	}
	defer func() { _ = tx.Rollback() }()

	limit := "9999-12-31T23:59:59Z"
	if !cutoff.IsZero() {
		limit = cutoff.UTC().Format(time.RFC3339)
	}
	removed := 0
	for source, keys := range unused {
		for _, key := range keys {
			res, err := tx.Exec("DELETE FROM raw_page WHERE source = ? AND key = ? AND fetched_at < ?", source, key, limit)
			if err != nil {
				return 0, fmt.Errorf("failed to remove %s %s: %w", source, key, err)
			}
			n, _ := res.RowsAffected()
			removed += int(n)
		}
	}
	return removed, tx.Commit()
}
