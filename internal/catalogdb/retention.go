package catalogdb

import (
	"fmt"
	"time"
)

// PruneEvents removes archived event pages whose event ended more than keep ago and
// leaves a tombstone, so that they are not fetched again. An event without any date
// is removed once no module page links it any more and it has not been refreshed
// within keep. It works on the result of the last build (event.last_date); the next
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

	_, err = tx.Exec(`
		INSERT OR REPLACE INTO event_tombstone (event_id, last_date, pruned_at)
		SELECT e.id, e.last_date, ? FROM event e
		WHERE (e.last_date IS NOT NULL AND e.last_date < ?)
		   OR (e.last_date IS NULL AND e.fetched_at < ?
		       AND NOT EXISTS (SELECT 1 FROM module_event me WHERE me.event_id = e.id))`,
		prunedAt, cutoffDate, cutoffTime)
	if err != nil {
		return 0, fmt.Errorf("failed to mark events: %w", err)
	}
	res, err := tx.Exec(`DELETE FROM raw_page WHERE source = ? AND key IN (SELECT event_id FROM event_tombstone WHERE pruned_at = ?)`,
		SourceQISEvent, prunedAt)
	if err != nil {
		return 0, fmt.Errorf("failed to remove event pages: %w", err)
	}
	removed, _ := res.RowsAffected()
	return int(removed), tx.Commit()
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
