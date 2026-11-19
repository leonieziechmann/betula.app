// Package privacy takes the personal data of people who asked for it out of what Radix keeps and
// publishes (DSGVO Art. 17, 21): the register of those people (radix.db, privacy_person), the
// forms their names take, and the redaction every build applies before its content digest, that
// validate checks, and that the history applies to every version it keeps.
//
// A person stays in the register for as long as their data is to stay out: the crawl reads their
// name again from the university's pages every week, and the register is what keeps it out of
// the next build. The register is never published (the snapshot export drops it) and never
// written into the repository. See docs/radix/privacy.md.
package privacy

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"sort"
	"strings"
	"time"
)

// Person is one entry of the register.
type Person struct {
	ID      int64
	Surname string   // „Mustermann"
	Given   string   // „Max": the given name, needed so that a namesake keeps their data
	Forms   []string // further forms of the name (an e-mail address, „Prof. Mustermann", a maiden name)
	Note    string   // where the request came from: a reference, never contact data
	AddedAt string   // RFC 3339, UTC
}

// Validate refuses an entry that would take out more than one person's data or none.
func (p Person) Validate() error {
	if strings.TrimSpace(p.Surname) == "" || strings.TrimSpace(p.Given) == "" {
		return errors.New("privacy: a person needs a surname and a given name (a surname alone would take out every namesake)")
	}
	if len([]rune(strings.TrimSpace(folded(p.Surname)))) < 2 {
		return errors.New("privacy: the surname is too short to be told from the words around it")
	}
	return nil
}

// Name is the person as the lists write them: „Mustermann, Max".
func (p Person) Name() string { return p.Surname + ", " + p.Given }

// ErrNoRegister: the database has no register (a snapshot, or one older than schema 25).
var ErrNoRegister = errors.New("privacy: the database has no register (privacy_person)")

// Load reads the register, ordered by id. A database without the table (a snapshot) has none:
// the result is empty and the error ErrNoRegister.
func Load(ctx context.Context, db DB) ([]Person, error) {
	var n int
	if err := db.QueryRowContext(ctx, "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'privacy_person'").Scan(&n); err != nil {
		return nil, err
	}
	if n == 0 {
		return nil, ErrNoRegister
	}
	rows, err := db.QueryContext(ctx, "SELECT id, surname, given, COALESCE(forms, ''), COALESCE(note, ''), added_at FROM privacy_person ORDER BY id")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var people []Person
	for rows.Next() {
		var p Person
		var forms string
		if err := rows.Scan(&p.ID, &p.Surname, &p.Given, &forms, &p.Note, &p.AddedAt); err != nil {
			return nil, err
		}
		for _, f := range strings.Split(forms, "\n") {
			if f = strings.TrimSpace(f); f != "" {
				p.Forms = append(p.Forms, f)
			}
		}
		people = append(people, p)
	}
	return people, rows.Err()
}

// Add puts a person into the register and returns their id. The next build takes them out of the
// catalog, the next export of the snapshot, the history's next stage out of every version.
func Add(ctx context.Context, db DB, p Person, now time.Time) (int64, error) {
	if err := p.Validate(); err != nil {
		return 0, err
	}
	var forms []string
	for _, f := range p.Forms {
		if f = strings.TrimSpace(f); f != "" {
			forms = append(forms, f)
		}
	}
	res, err := db.ExecContext(ctx, "INSERT INTO privacy_person (surname, given, forms, note, added_at) VALUES (?, ?, ?, ?, ?)",
		strings.TrimSpace(p.Surname), strings.TrimSpace(p.Given), nullIfEmpty(strings.Join(forms, "\n")), nullIfEmpty(strings.TrimSpace(p.Note)),
		now.UTC().Format(time.RFC3339))
	if err != nil {
		return 0, fmt.Errorf("privacy: add: %w", err)
	}
	return res.LastInsertId()
}

// Remove takes a person out of the register: the next build has their data again, as the
// university publishes it. What the history lost to the register does not come back.
func Remove(ctx context.Context, db DB, id int64) (bool, error) {
	res, err := db.ExecContext(ctx, "DELETE FROM privacy_person WHERE id = ?", id)
	if err != nil {
		return false, err
	}
	n, err := res.RowsAffected()
	return n > 0, err
}

func nullIfEmpty(s string) any {
	if s == "" {
		return nil
	}
	return s
}

// Digest identifies what the register takes out: "" when it is empty. The history keeps the one it
// applied, and applies the register again to every version when it differs; the export removes
// older snapshots when the published one is the first of a new digest.
func Digest(people []Person) string {
	if len(people) == 0 {
		return ""
	}
	lines := make([]string, 0, len(people))
	for _, p := range people {
		forms := append([]string{}, p.Forms...)
		sort.Strings(forms)
		lines = append(lines, fmt.Sprintf("%s\x1f%s\x1f%s", folded(p.Surname), folded(p.Given), strings.Join(forms, "\x1e")))
	}
	sort.Strings(lines)
	h := sha256.Sum256([]byte(strings.Join(lines, "\n")))
	return hex.EncodeToString(h[:16])
}

// Meta reads a value of privacy_meta ("" when it is not set or there is no such table).
func Meta(ctx context.Context, db DB, key string) string {
	var v string
	if err := db.QueryRowContext(ctx, "SELECT value FROM privacy_meta WHERE key = ?", key).Scan(&v); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return ""
	}
	return v
}

// RegisterID tells this register from any other (privacy_meta register_id, made by migration 25):
// the history notes the one it follows and refuses another, so that a fresh or a wrong radix.db,
// whose register is empty, never lets back into it what this one keeps out. "" without one.
func RegisterID(ctx context.Context, db DB) string { return Meta(ctx, db, "register_id") }

// SetMeta writes a value of privacy_meta.
func SetMeta(ctx context.Context, db DB, key, value string) error {
	_, err := db.ExecContext(ctx, "INSERT INTO privacy_meta (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value", key, value)
	return err
}
