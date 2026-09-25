package catalogbuild

import (
	"testing"
)

// The current semester moves on its own, without a single row of the catalog
// changing: BTU publishes the schedule of the next semester while the old one
// runs out. Only a changed digest is exported, so the digest has to see it.
func TestTheDigestSeesTheCurrentSemester(t *testing.T) {
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

	before := digest()
	if _, err := db.SQL().Exec("UPDATE meta SET value = '2026W' WHERE key = 'current_semester'"); err != nil {
		t.Fatalf("update: %v", err)
	}
	if after := digest(); after == before {
		t.Error("the digest is the same after the semester moved; no snapshot would be published")
	}
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
		"UPDATE event_date SET room_short = 'LG1A 0.23' WHERE event_id = '120286'",
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
