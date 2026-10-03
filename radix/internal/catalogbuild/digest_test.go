package catalogbuild

import (
	"fmt"
	"strconv"
	"testing"
)

// The current semester moves on its own, without a single row of the catalog
// changing: BTU publishes the schedule of the next semester while the old one
// runs out. Only a changed digest is exported, so the digest has to see it.
func TestTheDigestSeesTheCurrentSemester(t *testing.T) {
	db, _ := buildFixture(t)
	// The fixture's semester is the calendar's (currentSemesterKey), so it moves to the
	// one after it: a fixed '2026W' was no move from 2026-10-01 on.
	var current string
	if err := db.SQL().QueryRow("SELECT value FROM meta WHERE key = 'current_semester'").Scan(&current); err != nil {
		t.Fatalf("current semester: %v", err)
	}

	digest := func() string {
		t.Helper()
		tx, err := db.SQL().Begin()
		if err != nil {
			t.Fatalf("begin: %v", err)
		}
		defer func() { _ = tx.Rollback() }()
		d, err := contentDigest(tx)
		if err != nil {
			t.Fatalf("contentDigest: %v", err)
		}
		return d
	}

	before := digest()
	if _, err := db.SQL().Exec("UPDATE meta SET value = ? WHERE key = 'current_semester'", nextSemester(t, current)); err != nil {
		t.Fatalf("update: %v", err)
	}
	if after := digest(); after == before {
		t.Error("the digest is the same after the semester moved; no snapshot would be published")
	}
}

// nextSemester is the semester after key: "2026S" → "2026W", "2026W" → "2027S".
func nextSemester(t *testing.T, key string) string {
	t.Helper()
	if len(key) == 5 {
		year, err := strconv.Atoi(key[:4])
		switch {
		case err != nil:
		case key[4] == 'S':
			return fmt.Sprintf("%dW", year)
		case key[4] == 'W':
			return fmt.Sprintf("%dS", year+1)
		}
	}
	t.Fatalf("no semester key: %q", key)
	return ""
}

// Short names are content: a new abbreviation or room short form must reach the browsers.
func TestTheDigestSeesShortNames(t *testing.T) {
	db, _ := buildFixture(t)
	digest := func() string {
		t.Helper()
		tx, err := db.SQL().Begin()
		if err != nil {
			t.Fatalf("begin: %v", err)
		}
		defer func() { _ = tx.Rollback() }()
		d, err := contentDigest(tx)
		if err != nil {
			t.Fatalf("contentDigest: %v", err)
		}
		return d
	}
	seen := map[string]string{"": digest()}
	for _, stmt := range []string{
		"UPDATE module_abbrev SET abbrev = 'LinA' WHERE module_id = '11101'",
		"UPDATE program_module_abbrev SET abbrev = 'FoDM' WHERE module_id = '11881' AND program_id = '079-82-2008'",
		"UPDATE event_date SET room_short = 'LG1A/0.23' WHERE event_id = '120286'",
		// The sums a plan prints are content as well: a rescan that changes only them must
		// publish a snapshot.
		`INSERT INTO plan_total (program_id, ord, label, scope, start_semester, end_semester, credits, credits_max,
			min_credits, max_credits, is_choice, entry_count) VALUES ('079-82-2008', 1, 'Summe', 'plan', 1, 1, 6, 6, 6, 6, 0, 1)`,
		"INSERT INTO plan_total_entry (program_id, total_ord, entry_ord) VALUES ('079-82-2008', 1, 1)",
	} {
		if _, err := db.SQL().Exec(stmt); err != nil {
			t.Fatalf("%s: %v", stmt, err)
		}
		d := digest()
		for prev, pd := range seen {
			if pd == d {
				t.Errorf("the digest after %q is the one after %q", stmt, prev)
			}
		}
		seen[stmt] = d
	}
}
