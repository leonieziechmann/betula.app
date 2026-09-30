package catalogbuild

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
)

// digestTables are what readers of a snapshot see. meta is left out, and so is
// every fetched_at column: a page that was fetched again without any change must
// not make a new snapshot, or every browser would download 30 MB for nothing.
var digestTables = []string{
	"department", "module", "module_person", "module_teaching_form", "module_text_item",
	"module_prerequisite", "module_successor",
	"program", "program_document", "program_area",
	"module_program_ref", "program_module_assertion", "program_module", "module_facet",
	"plan", "plan_entry", "plan_scan_status", "plan_total", "plan_total_entry",
	"semester", "event", "event_form", "event_person", "event_date", "module_event",
	"module_abbrev", "program_module_abbrev", "module_folded",
}

// contentDigest hashes the published content. Builds are deterministic, so equal
// content yields an equal digest.
func contentDigest(tx *sql.Tx) (string, error) {
	h := sha256.New()
	for _, table := range digestTables {
		rows, err := tx.Query("SELECT * FROM " + table + " ORDER BY 1, 2")
		if err != nil {
			return "", fmt.Errorf("digest of %s: %w", table, err)
		}
		cols, err := rows.Columns()
		if err != nil {
			rows.Close()
			return "", err
		}
		fmt.Fprintf(h, "\x1etable %s\n", table)

		values := make([]sql.RawBytes, len(cols))
		ptrs := make([]any, len(cols))
		for i := range values {
			ptrs[i] = &values[i]
		}
		for rows.Next() {
			if err := rows.Scan(ptrs...); err != nil {
				rows.Close()
				return "", err
			}
			for i, v := range values {
				if cols[i] == "fetched_at" {
					continue
				}
				if v == nil {
					h.Write([]byte{0})
				} else {
					h.Write([]byte{1})
					h.Write(v)
				}
				h.Write([]byte{0x1f})
			}
			h.Write([]byte{'\n'})
		}
		err = rows.Err()
		rows.Close()
		if err != nil {
			return "", err
		}
	}

	// The current semester lives in meta, which is left out above, but it is content:
	// pages name it, and it moves on its own when the schedule of the next semester is
	// published while the old one runs out. Without it here, that switch would never
	// reach a browser, because only a changed digest is exported.
	var current sql.NullString
	if err := tx.QueryRow("SELECT value FROM meta WHERE key = 'current_semester'").Scan(&current); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return "", fmt.Errorf("digest of the current semester: %w", err)
	}
	fmt.Fprintf(h, "\x1ecurrent_semester %s\n", current.String)

	return hex.EncodeToString(h.Sum(nil)), nil
}
