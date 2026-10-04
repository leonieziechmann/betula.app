package catalogdb

import (
	"testing"
	"time"
)

// An answer fetched before the archived page, as Cortex gives one from its store, is
// older news: fetched_at never goes backwards, and neither body nor status are replaced.
func TestPutPageNeverMovesFetchedAtBackwards(t *testing.T) {
	db := openTestDB(t)
	day := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	put := func(at time.Time, status int, body string) bool {
		t.Helper()
		changed, err := db.PutPageChanged(RawPage{Source: SourceQISTree, Key: "root", URL: "u", FetchedAt: at, HTTPStatus: status, Body: []byte(body)})
		if err != nil {
			t.Fatalf("PutPageChanged failed: %v", err)
		}
		return changed
	}
	check := func(what string, fetched, changedAt time.Time, status int, body string) {
		t.Helper()
		p, err := db.GetPage(SourceQISTree, "root")
		if err != nil {
			t.Fatalf("GetPage failed: %v", err)
		}
		if !p.FetchedAt.Equal(fetched) || !p.ChangedAt.Equal(changedAt) || p.HTTPStatus != status || string(p.Body) != body {
			t.Errorf("%s: fetched_at=%v changed_at=%v status=%d body=%q, want %v %v %d %q", what,
				p.FetchedAt, p.ChangedAt, p.HTTPStatus, p.Body, fetched, changedAt, status, body)
		}
	}

	if !put(day, 200, "today") {
		t.Error("a new page is not reported as changed")
	}
	if put(day.Add(-time.Hour), 200, "an hour ago") {
		t.Error("an older answer is reported as changed")
	}
	check("older answer", day, day, 200, "today")
	if put(day.Add(-time.Hour), 404, "") {
		t.Error("an older 404 is reported as changed")
	}
	check("older 404", day, day, 200, "today")

	// The same second is not older: the archive keeps seconds, Cortex microseconds.
	if !put(day.Add(700*time.Millisecond), 200, "today, later") {
		t.Error("an answer of the same second is not stored")
	}
	check("same second", day, day, 200, "today, later")

	if put(day.Add(time.Hour), 200, "today, later") {
		t.Error("an unchanged body is reported as changed")
	}
	check("newer, unchanged", day.Add(time.Hour), day, 200, "today, later")
	if !put(day.Add(2*time.Hour), 404, "") {
		t.Error("a newer 404 is not reported as changed")
	}
	check("newer 404", day.Add(2*time.Hour), day.Add(2*time.Hour), 404, "")
}
