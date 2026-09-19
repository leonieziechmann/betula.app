package catalogdb

import (
	"testing"
	"time"
)

func TestPruneEventsRemovesOldEventsAndRemembersThem(t *testing.T) {
	db := openTestDB(t)
	now := time.Date(2026, 9, 19, 12, 0, 0, 0, time.UTC)
	old := now.Add(-90 * 24 * time.Hour)

	for _, id := range []string{"ended-long-ago", "ended-last-week", "undated-unlinked", "undated-linked", "undated-fresh", "future-unlinked-stale"} {
		fetchedAt := old
		if id == "undated-fresh" {
			fetchedAt = now
		}
		if err := db.PutPage(RawPage{Source: SourceQISEvent, Key: id, URL: id, HTTPStatus: 200, Body: []byte(id), FetchedAt: fetchedAt}); err != nil {
			t.Fatal(err)
		}
	}
	// What the last build knew about these events.
	_, err := db.SQL().Exec(`
		INSERT INTO module (id, title, detail_status, offer_status, is_fues) VALUES ('11101', 'Lineare Algebra', 'ok', 'active', 0);
		INSERT INTO event (id, title, category, last_date, source_url, fetched_at) VALUES
			('ended-long-ago',   'a', 'teaching', '2026-07-21', 'u', '2026-06-21T12:00:00Z'),
			('ended-last-week',  'b', 'exam',     '2026-09-12', 'u', '2026-06-21T12:00:00Z'),
			('undated-unlinked', 'c', 'other',    NULL,         'u', '2026-06-21T12:00:00Z'),
			('undated-linked',   'd', 'other',    NULL,         'u', '2026-06-21T12:00:00Z'),
			('undated-fresh',    'e', 'other',    NULL,         'u', '2026-09-19T12:00:00Z'),
			('future-unlinked-stale', 'f', 'teaching', '2027-02-05', 'u', '2026-06-21T12:00:00Z');
		INSERT INTO module_event (module_id, event_id) VALUES ('11101', 'undated-linked'), ('11101', 'ended-last-week');`)
	if err != nil {
		t.Fatal(err)
	}

	removed, err := db.PruneEvents(now, 30*24*time.Hour)
	if err != nil || removed != 3 {
		t.Fatalf("PruneEvents = %d, %v; want 3", removed, err)
	}

	tombstones, err := db.EventTombstones()
	if err != nil || len(tombstones) != 3 || !tombstones["ended-long-ago"] || !tombstones["undated-unlinked"] || !tombstones["future-unlinked-stale"] {
		t.Fatalf("tombstones = %v (err %v)", tombstones, err)
	}
	for id, wantArchived := range map[string]bool{
		"ended-long-ago": false, "undated-unlinked": false, "future-unlinked-stale": false, // no module page links it and nothing refreshes it
		"ended-last-week": true, "undated-linked": true, "undated-fresh": true,
	} {
		_, err := db.GetPage(SourceQISEvent, id)
		if archived := err == nil; archived != wantArchived {
			t.Errorf("event %s archived = %v, want %v", id, archived, wantArchived)
		}
	}

	// Running it again removes nothing more.
	if removed, err := db.PruneEvents(now, 30*24*time.Hour); err != nil || removed != 0 {
		t.Fatalf("second PruneEvents = %d, %v", removed, err)
	}
}
