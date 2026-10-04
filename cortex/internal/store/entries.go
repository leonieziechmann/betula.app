package store

import (
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"
)

// Entry is one request identity: a canonical key (URL plus the Accept headers that are part of
// it) and what was last asked for it.
type Entry struct {
	ID             int64     `json:"id"`
	Key            string    `json:"key"`
	URL            string    `json:"url"`
	Host           string    `json:"host"`
	Source         string    `json:"source"`
	Accept         string    `json:"accept,omitempty"`
	AcceptLanguage string    `json:"accept_language,omitempty"`
	CreatedAt      time.Time `json:"created_at"`
	CurrentVersion int64     `json:"current_version,omitempty"` // 0 without a version
}

// Version is one answer of an entry that differed from the one before (in content or status).
type Version struct {
	ID           int64       `json:"id"`
	EntryID      int64       `json:"entry_id"`
	Status       int         `json:"status"`
	Hash         string      `json:"sha256"` // the blob, lower-case hex
	Size         int64       `json:"size"`
	Header       http.Header `json:"headers"`    // the kept response headers
	FinalURL     string      `json:"-"`          // the URL that answered, after redirects ("": not known)
	FetchedAt    time.Time   `json:"fetched_at"` // the first fetch that returned this content
	CheckedAt    time.Time   `json:"checked_at"` // the last fetch that returned it
	SupersededAt time.Time   `json:"superseded_at,omitzero"`
}

// EntryInfo is an entry with its current version, as ListEntries returns it.
type EntryInfo struct {
	Entry
	Current Version `json:"current"`
}

// Fetched is a completed fetch, for RecordFetch: the key and request as Canonical made them,
// the answer, and its body already stored as the blob Hash.
type Fetched struct {
	Key, URL, Host, Source, Accept, AcceptLanguage string
	Status                                         int
	Hash                                           string
	Size                                           int64
	Header                                         http.Header // the kept headers
	FinalURL                                       string      // the URL that answered, after redirects
	At                                             time.Time
}

// FinalURLHeader is the key under which a version's kept headers hold the URL that answered
// (Fetched.FinalURL), so that it travels with the row and the journal like the validators it
// belongs to. It is internal: Version.Header never has it, Version.FinalURL does.
const FinalURLHeader = "Cortex-Final-Url"

// EntryFilter narrows ListEntries. Empty fields match everything.
type EntryFilter struct {
	Host, Source string
	ChangedSince time.Time // the current version was first fetched at or after this time
}

// Listing limits: what a limit ≤ 0 means, and the most one call returns.
const (
	DefaultLimit = 100
	MaxLimit     = 1000
)

// RecordFetch records a fetch of f.Key. When the entry's current version has the same status
// and sha256, only its checked_at moves to f.At (journal op check); otherwise a new version
// becomes current (fetched_at = checked_at = f.At) and the old one is superseded at f.At (op
// version). The entry is created on its first fetch; its URL, host, accept headers and (when
// given) source are updated to f's. changed reports a new version.
//
// The key, URL, host and accept headers must be valid UTF-8 and the source empty or 1 to 64 of
// a-z, 0-9, '_', '.', '-' (else ErrInvalidInput). Of the kept headers, a value that is not UTF-8
// is made valid (strings.ToValidUTF8) and one longer than 8 KiB is left out.
//
// The blob must be stored (PutBlob) before, else ErrBlobMissing; its time is renewed.
func (s *Store) RecordFetch(f Fetched) (Entry, Version, bool, JournalEntry, error) {
	if f.Key == "" || f.URL == "" {
		return Entry{}, Version{}, false, JournalEntry{}, errors.New("record fetch: key and url are required")
	}
	if !ValidHash(f.Hash) {
		return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %s: blob %q: %w", f.URL, f.Hash, ErrInvalidHash)
	}
	if f.Status < 100 || f.Status > 999 || f.Size < 0 {
		return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %s: invalid status %d or size %d", f.URL, f.Status, f.Size)
	}
	for _, v := range []string{f.Key, f.URL, f.Host, f.Accept, f.AcceptLanguage} {
		if !utf8.ValidString(v) {
			return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %q: %w: %q is not UTF-8", f.URL, ErrInvalidInput, v)
		}
	}
	if f.Source != "" && !validSource(f.Source) {
		return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %s: %w: source %q", f.URL, ErrInvalidInput, f.Source)
	}
	kept := keptHeader(f.Header)
	kept.Del(FinalURLHeader) // only from f.FinalURL
	if f.FinalURL != "" {
		kept.Set(FinalURLHeader, f.FinalURL)
		kept = keptHeader(kept) // the same rules for it: UTF-8, at most 8 KiB, else left out
	}
	headers, err := json.Marshal(kept)
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %s: %w", f.URL, err)
	}
	if f.At.IsZero() {
		f.At = s.now()
	}
	at := stamp(f.At)
	atText := FormatTime(at)

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	defer func() { _ = tx.Rollback() }()

	e, found, err := entryRowByKey(tx, f.Key)
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	var current *versionRow
	if found && e.CurrentVersion != nil {
		v, err := versionRowByID(tx, *e.CurrentVersion)
		if err != nil {
			return Entry{}, Version{}, false, JournalEntry{}, err
		}
		current = &v
	}
	if !found {
		id, err := nextID(tx, "entry")
		if err != nil {
			return Entry{}, Version{}, false, JournalEntry{}, err
		}
		e = entryRow{ID: id, Key: f.Key, Source: f.Source, CreatedAt: atText}
	}
	e.URL, e.Host, e.Accept, e.AcceptLanguage = f.URL, f.Host, f.Accept, f.AcceptLanguage
	if f.Source != "" {
		e.Source = f.Source
	}

	p := versionPayload{Entry: e}
	op, blob := OpCheck, ""
	if current != nil && current.Status == f.Status && current.SHA256 == f.Hash {
		p.Version = *current
		if atText > p.Version.CheckedAt { // never backwards
			p.Version.CheckedAt = atText
		}
		// The validators are sent only to the URL they came from (Version.FinalURL): when the
		// same content now comes from another URL, or the version predates FinalURL, the
		// headers of this answer replace the old ones, so that validators and URL stay a pair.
		if f.FinalURL != "" && headerFinalURL(p.Version.Headers) != f.FinalURL {
			p.Version.Headers = string(headers)
		}
	} else {
		id, err := nextID(tx, "version")
		if err != nil {
			return Entry{}, Version{}, false, JournalEntry{}, err
		}
		p.Version = versionRow{ID: id, EntryID: e.ID, Status: f.Status, SHA256: f.Hash, Size: f.Size,
			Headers: string(headers), FetchedAt: atText, CheckedAt: atText}
		p.Entry.CurrentVersion = &id
		if current != nil {
			p.Superseded = &supersede{ID: current.ID, At: atText}
		}
		op, blob = OpVersion, f.Hash
	}
	if err := applyVersion(tx, op, p); err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	if err := s.renewBlob(f.Hash); err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, fmt.Errorf("record fetch of %s: %w", f.URL, err)
	}
	je, err := s.commit(tx, op, at, p, blob)
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	entry, err := p.Entry.public()
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	version, err := p.Version.public()
	if err != nil {
		return Entry{}, Version{}, false, JournalEntry{}, err
	}
	return entry, version, op == OpVersion, je, nil
}

// Imported is an answer that another program fetched, for RecordImport: the key and request as
// Canonical made them, the answer, its body already stored as the blob Hash, when that program
// got this content first (FetchedAt) and last (CheckedAt), and when it is given to Cortex (At,
// the time of the journal entry; zero: now).
type Imported struct {
	Key, URL, Host, Source, Accept, AcceptLanguage string
	Status                                         int
	Hash                                           string
	Size                                           int64
	Header                                         http.Header // the kept headers
	FetchedAt, CheckedAt                           time.Time
	At                                             time.Time
}

// What RecordImport did with an answer.
const (
	ImportCreated   = "created"   // a new version, current now (journal op version)
	ImportChecked   = "checked"   // the current version has this content; its checked_at moved (op check)
	ImportUnchanged = "unchanged" // the current version has this content and knew it at least as late: nothing written
	ImportOlder     = "older"     // the current version has other content, checked at or after CheckedAt: nothing written
)

// importSkew is how far CheckedAt may lie ahead of this clock: the program that fetched may run
// on another host.
const importSkew = time.Minute

// RecordImport records an answer that another program fetched (an archive of its own, as Radix's
// raw pages), as if Cortex had fetched it at the times that program gives, so that a client that
// asks offline gets what that program had:
//   - without a current version the answer becomes one, first fetched at FetchedAt and last
//     checked at CheckedAt (op version; a new entry is created at FetchedAt);
//   - a current version of the same status and sha256 has its checked_at moved to CheckedAt when
//     that is later (op check), as a fetch would; its fetched_at stays. Nothing is written when
//     it does not move and the entry is as the import names it (ImportUnchanged);
//   - a current version of other content last checked before CheckedAt is superseded by the
//     answer (op version), at FetchedAt, or at its own checked_at when that is later: the import
//     then counts as first fetched at that time too, so that the versions follow each other;
//   - a current version of other content checked at or after CheckedAt is newer than the answer:
//     nothing is written (ImportOlder). Cortex keeps no history before its current version.
//
// The journal entry carries the time of the import (At), not the times of the answer: the
// follower's lag and the trimming of the journal are about when the index changed. FetchedAt
// after CheckedAt, or CheckedAt more than a minute after At, is ErrInvalidInput; the rest is
// checked as by RecordFetch. The blob must be stored (PutBlob) before, else
// ErrBlobMissing; its time is renewed.
func (s *Store) RecordImport(f Imported) (Entry, Version, string, error) {
	if f.Key == "" || f.URL == "" {
		return Entry{}, Version{}, "", errors.New("record import: key and url are required")
	}
	if !ValidHash(f.Hash) {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: blob %q: %w", f.URL, f.Hash, ErrInvalidHash)
	}
	if f.Status < 100 || f.Status > 999 || f.Size < 0 {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: invalid status %d or size %d", f.URL, f.Status, f.Size)
	}
	for _, v := range []string{f.Key, f.URL, f.Host, f.Accept, f.AcceptLanguage} {
		if !utf8.ValidString(v) {
			return Entry{}, Version{}, "", fmt.Errorf("record import of %q: %w: %q is not UTF-8", f.URL, ErrInvalidInput, v)
		}
	}
	if f.Source != "" && !validSource(f.Source) {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: %w: source %q", f.URL, ErrInvalidInput, f.Source)
	}
	if f.FetchedAt.IsZero() || f.CheckedAt.IsZero() || f.FetchedAt.After(f.CheckedAt) {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: %w: fetched_at %s and checked_at %s (want both, the first not after the second)",
			f.URL, ErrInvalidInput, FormatTime(f.FetchedAt), FormatTime(f.CheckedAt))
	}
	now := f.At
	if now.IsZero() {
		now = s.now()
	}
	if f.CheckedAt.After(now.Add(importSkew)) {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: %w: checked_at %s lies in the future", f.URL, ErrInvalidInput, FormatTime(f.CheckedAt))
	}
	kept := keptHeader(f.Header)
	kept.Del(FinalURLHeader) // an import knows no URL that answered: validators are not sent for it
	headers, err := json.Marshal(kept)
	if err != nil {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: %w", f.URL, err)
	}
	first, last := FormatTime(stamp(f.FetchedAt)), FormatTime(stamp(f.CheckedAt))

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return Entry{}, Version{}, "", err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return Entry{}, Version{}, "", err
	}
	defer func() { _ = tx.Rollback() }()

	e, found, err := entryRowByKey(tx, f.Key)
	if err != nil {
		return Entry{}, Version{}, "", err
	}
	var current *versionRow
	if found && e.CurrentVersion != nil {
		v, err := versionRowByID(tx, *e.CurrentVersion)
		if err != nil {
			return Entry{}, Version{}, "", err
		}
		current = &v
	}
	before := e
	if !found {
		id, err := nextID(tx, "entry")
		if err != nil {
			return Entry{}, Version{}, "", err
		}
		e = entryRow{ID: id, Key: f.Key, Source: f.Source, CreatedAt: first}
	}
	e.URL, e.Host, e.Accept, e.AcceptLanguage = f.URL, f.Host, f.Accept, f.AcceptLanguage
	if f.Source != "" {
		e.Source = f.Source
	}

	p := versionPayload{Entry: e}
	result, op, blob := ImportCreated, OpVersion, f.Hash
	switch {
	case current != nil && current.Status == f.Status && current.SHA256 == f.Hash:
		p.Version = *current
		if last <= current.CheckedAt && sameNames(e, before) {
			entry, version, err := publicPair(e, *current)
			return entry, version, ImportUnchanged, err
		}
		p.Version.CheckedAt = max(current.CheckedAt, last) // never backwards
		result, op, blob = ImportChecked, OpCheck, ""
	case current != nil && current.CheckedAt >= last:
		entry, version, err := publicPair(before, *current)
		return entry, version, ImportOlder, err
	default:
		id, err := nextID(tx, "version")
		if err != nil {
			return Entry{}, Version{}, "", err
		}
		from := first
		if current != nil {
			from = max(first, current.CheckedAt)
			p.Superseded = &supersede{ID: current.ID, At: from}
		}
		p.Version = versionRow{ID: id, EntryID: e.ID, Status: f.Status, SHA256: f.Hash, Size: f.Size,
			Headers: string(headers), FetchedAt: from, CheckedAt: last}
		p.Entry.CurrentVersion = &id
	}
	if err := applyVersion(tx, op, p); err != nil {
		return Entry{}, Version{}, "", err
	}
	if err := s.renewBlob(f.Hash); err != nil {
		return Entry{}, Version{}, "", fmt.Errorf("record import of %s: %w", f.URL, err)
	}
	if _, err := s.commit(tx, op, stamp(now), p, blob); err != nil {
		return Entry{}, Version{}, "", err
	}
	entry, version, err := publicPair(p.Entry, p.Version)
	return entry, version, result, err
}

// sameNames says whether two rows of one entry name the request and its source alike.
func sameNames(a, b entryRow) bool {
	return a.URL == b.URL && a.Host == b.Host && a.Source == b.Source && a.Accept == b.Accept && a.AcceptLanguage == b.AcceptLanguage
}

// publicPair is an entry row and a version row as the API shows them.
func publicPair(e entryRow, v versionRow) (Entry, Version, error) {
	entry, err := e.public()
	if err != nil {
		return Entry{}, Version{}, err
	}
	version, err := v.public()
	if err != nil {
		return Entry{}, Version{}, err
	}
	return entry, version, nil
}

// maxHeaderValue is the longest kept response header value the index keeps. Every check entry
// of the journal carries the whole version row again, headers included, and a host may send a
// megabyte of ETag (review 1).
const maxHeaderValue = 8 << 10

// keptHeader is h as the index keeps it: every value valid UTF-8 and at most maxHeaderValue
// bytes, a longer one left out. encoding/json would turn bytes that are not UTF-8 into U+FFFD
// as well, but only in the text it writes; here the row and the journal get the same.
func keptHeader(h http.Header) http.Header {
	kept := http.Header{}
	for name, values := range h {
		for _, v := range values {
			if v = strings.ToValidUTF8(v, "\uFFFD"); len(v) <= maxHeaderValue {
				kept[name] = append(kept[name], v)
			}
		}
	}
	return kept
}

// headerFinalURL is the FinalURLHeader of a version row's headers ("" when it has none).
func headerFinalURL(headers string) string {
	var h http.Header
	if json.Unmarshal([]byte(headers), &h) != nil {
		return ""
	}
	return h.Get(FinalURLHeader)
}

// DeleteEntry forgets a key: the entry and all its versions (op entry_delete). ErrNotFound when
// there is no such entry.
func (s *Store) DeleteEntry(key string) (JournalEntry, error) {
	at := stamp(s.now())
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return JournalEntry{}, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return JournalEntry{}, err
	}
	defer func() { _ = tx.Rollback() }()
	e, found, err := entryRowByKey(tx, key)
	if err != nil {
		return JournalEntry{}, err
	}
	if !found {
		return JournalEntry{}, ErrNotFound
	}
	p := entryDeletePayload{ID: e.ID, Key: e.Key}
	if err := applyEntryDelete(tx, p); err != nil {
		return JournalEntry{}, err
	}
	return s.commit(tx, OpEntryDelete, at, p, "")
}

// Lookup returns an entry and its current version. ErrNotFound when the key has no entry or no
// current version.
func (s *Store) Lookup(key string) (Entry, Version, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return Entry{}, Version{}, err
	}
	row := s.r.QueryRow(`SELECT `+entryColumns("e")+`, `+versionColumns("v")+`
		FROM entry e JOIN version v ON v.id = e.current_version WHERE e.key = ?`, key)
	return scanEntryVersion(row)
}

// LookupAt returns an entry and the version that was current at a time: the newest version
// first fetched at or before at that was not superseded at or before at. ErrNotFound when
// there is none.
func (s *Store) LookupAt(key string, at time.Time) (Entry, Version, error) {
	t := FormatTime(stamp(at))
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return Entry{}, Version{}, err
	}
	row := s.r.QueryRow(`SELECT `+entryColumns("e")+`, `+versionColumns("v")+`
		FROM entry e JOIN version v ON v.entry_id = e.id
		WHERE e.key = ?1 AND v.fetched_at <= ?2 AND (v.superseded_at IS NULL OR v.superseded_at > ?2)
		ORDER BY v.fetched_at DESC, v.id DESC LIMIT 1`, key, t)
	return scanEntryVersion(row)
}

// Versions returns an entry and all its versions that retention has kept, the newest first.
// ErrNotFound when the key has no entry.
func (s *Store) Versions(key string) (Entry, []Version, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return Entry{}, nil, err
	}
	tx, err := s.r.Begin() // one snapshot for the entry and its versions
	if err != nil {
		return Entry{}, nil, err
	}
	defer func() { _ = tx.Rollback() }()
	er, found, err := entryRowByKey(tx, key)
	if err != nil {
		return Entry{}, nil, err
	}
	if !found {
		return Entry{}, nil, ErrNotFound
	}
	entry, err := er.public()
	if err != nil {
		return Entry{}, nil, err
	}
	rows, err := tx.Query(`SELECT `+versionColumns("v")+` FROM version v WHERE v.entry_id = ? ORDER BY v.id DESC`, er.ID)
	if err != nil {
		return Entry{}, nil, err
	}
	defer rows.Close()
	versions := []Version{}
	for rows.Next() {
		var vr versionRow
		if err := vr.scan(rows); err != nil {
			return Entry{}, nil, err
		}
		v, err := vr.public()
		if err != nil {
			return Entry{}, nil, err
		}
		versions = append(versions, v)
	}
	return entry, versions, rows.Err()
}

// ListEntries returns the entries that have a current version and match filter, in the order
// they were created, at most limit (≤ 0: DefaultLimit, at most MaxLimit). next is the cursor
// for the following page, "" after the last.
func (s *Store) ListEntries(filter EntryFilter, cursor string, limit int) ([]EntryInfo, string, error) {
	var after int64
	if cursor != "" {
		n, err := strconv.ParseInt(cursor, 10, 64)
		if err != nil || n < 0 {
			return nil, "", fmt.Errorf("%w: %q", ErrInvalidCursor, cursor)
		}
		after = n
	}
	limit = clampLimit(limit)
	since := ""
	if !filter.ChangedSince.IsZero() {
		since = FormatTime(stamp(filter.ChangedSince))
	}

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return nil, "", err
	}
	// Only the conditions that filter, so that SQLite can use the index on (host, source),
	// whose rows are in id order within one host and source.
	query := `SELECT ` + entryColumns("e") + `, ` + versionColumns("v") + `
		FROM entry e JOIN version v ON v.id = e.current_version WHERE e.id > ?`
	args := []any{after}
	if filter.Host != "" {
		query += ` AND e.host = ?`
		args = append(args, filter.Host)
	}
	if filter.Source != "" {
		query += ` AND e.source = ?`
		args = append(args, filter.Source)
	}
	if since != "" {
		query += ` AND v.fetched_at >= ?`
		args = append(args, since)
	}
	rows, err := s.r.Query(query+` ORDER BY e.id LIMIT ?`, append(args, limit+1)...)
	if err != nil {
		return nil, "", err
	}
	defer rows.Close()
	result := []EntryInfo{}
	for rows.Next() {
		e, v, err := scanEntryVersion(rows)
		if err != nil {
			return nil, "", err
		}
		result = append(result, EntryInfo{Entry: e, Current: v})
	}
	if err := rows.Err(); err != nil {
		return nil, "", err
	}
	next := ""
	if len(result) > limit {
		result = result[:limit]
		next = strconv.FormatInt(result[limit-1].ID, 10)
	}
	return result, next, nil
}

func clampLimit(limit int) int {
	if limit <= 0 {
		return DefaultLimit
	}
	return min(limit, MaxLimit)
}

// entryRow and versionRow are the rows as the index holds them; journal payloads carry them so
// that a follower writes exactly what the leader wrote.
type entryRow struct {
	ID             int64  `json:"id"`
	Key            string `json:"key"`
	URL            string `json:"url"`
	Host           string `json:"host"`
	Source         string `json:"source"`
	Accept         string `json:"accept"`
	AcceptLanguage string `json:"accept_language"`
	CreatedAt      string `json:"created_at"`
	CurrentVersion *int64 `json:"current_version"`
}

type versionRow struct {
	ID           int64   `json:"id"`
	EntryID      int64   `json:"entry_id"`
	Status       int     `json:"status"`
	SHA256       string  `json:"sha256"`
	Size         int64   `json:"size"`
	Headers      string  `json:"headers"`
	FetchedAt    string  `json:"fetched_at"`
	CheckedAt    string  `json:"checked_at"`
	SupersededAt *string `json:"superseded_at"`
}

// supersede is the version a write replaced and when.
type supersede struct {
	ID int64  `json:"id"`
	At string `json:"at"`
}

type scanner interface{ Scan(dest ...any) error }

// querier is a transaction or a handle.
type querier interface {
	QueryRow(query string, args ...any) *sql.Row
}

func entryColumns(a string) string {
	return a + ".id, " + a + ".key, " + a + ".url, " + a + ".host, " + a + ".source, " + a + ".accept, " +
		a + ".accept_language, " + a + ".created_at, " + a + ".current_version"
}

func versionColumns(a string) string {
	return a + ".id, " + a + ".entry_id, " + a + ".status, " + a + ".sha256, " + a + ".size, " + a + ".headers, " +
		a + ".fetched_at, " + a + ".checked_at, " + a + ".superseded_at"
}

func (e *entryRow) dest() []any {
	return []any{&e.ID, &e.Key, &e.URL, &e.Host, &e.Source, &e.Accept, &e.AcceptLanguage, &e.CreatedAt, &e.CurrentVersion}
}

func (v *versionRow) dest() []any {
	return []any{&v.ID, &v.EntryID, &v.Status, &v.SHA256, &v.Size, &v.Headers, &v.FetchedAt, &v.CheckedAt, &v.SupersededAt}
}

func (v *versionRow) scan(sc scanner) error {
	return sc.Scan(v.dest()...)
}

func scanEntryVersion(sc scanner) (Entry, Version, error) {
	var er entryRow
	var vr versionRow
	err := sc.Scan(append(er.dest(), vr.dest()...)...)
	if errors.Is(err, sql.ErrNoRows) {
		return Entry{}, Version{}, ErrNotFound
	}
	if err != nil {
		return Entry{}, Version{}, err
	}
	e, err := er.public()
	if err != nil {
		return Entry{}, Version{}, err
	}
	v, err := vr.public()
	if err != nil {
		return Entry{}, Version{}, err
	}
	return e, v, nil
}

func entryRowByKey(q querier, key string) (entryRow, bool, error) {
	var e entryRow
	err := q.QueryRow(`SELECT `+entryColumns("e")+` FROM entry e WHERE e.key = ?`, key).Scan(e.dest()...)
	if errors.Is(err, sql.ErrNoRows) {
		return entryRow{}, false, nil
	}
	if err != nil {
		return entryRow{}, false, fmt.Errorf("failed to read entry %q: %w", key, err)
	}
	return e, true, nil
}

func versionRowByID(q querier, id int64) (versionRow, error) {
	var v versionRow
	err := q.QueryRow(`SELECT `+versionColumns("v")+` FROM version v WHERE v.id = ?`, id).Scan(v.dest()...)
	if err != nil {
		return versionRow{}, fmt.Errorf("failed to read version %d: %w", id, err)
	}
	return v, nil
}

func (e entryRow) public() (Entry, error) {
	created, err := parseTime(e.CreatedAt)
	if err != nil {
		return Entry{}, fmt.Errorf("entry %d: %w", e.ID, err)
	}
	entry := Entry{ID: e.ID, Key: e.Key, URL: e.URL, Host: e.Host, Source: e.Source, Accept: e.Accept,
		AcceptLanguage: e.AcceptLanguage, CreatedAt: created}
	if e.CurrentVersion != nil {
		entry.CurrentVersion = *e.CurrentVersion
	}
	return entry, nil
}

func (v versionRow) public() (Version, error) {
	version := Version{ID: v.ID, EntryID: v.EntryID, Status: v.Status, Hash: v.SHA256, Size: v.Size}
	if err := json.Unmarshal([]byte(v.Headers), &version.Header); err != nil {
		return Version{}, fmt.Errorf("version %d: invalid headers: %w", v.ID, err)
	}
	if version.Header != nil {
		version.FinalURL = version.Header.Get(FinalURLHeader)
		version.Header.Del(FinalURLHeader)
	}
	var err error
	if version.FetchedAt, err = parseTime(v.FetchedAt); err != nil {
		return Version{}, fmt.Errorf("version %d: %w", v.ID, err)
	}
	if version.CheckedAt, err = parseTime(v.CheckedAt); err != nil {
		return Version{}, fmt.Errorf("version %d: %w", v.ID, err)
	}
	if version.SupersededAt, err = parseNullTime(v.SupersededAt); err != nil {
		return Version{}, fmt.Errorf("version %d: %w", v.ID, err)
	}
	return version, nil
}

// nextID is the id the next row of table gets: past every id it ever had (AUTOINCREMENT keeps
// the highest in sqlite_sequence), so that an id is never used twice.
func nextID(tx *sql.Tx, table string) (int64, error) {
	var query string
	switch table {
	case "entry":
		query = `SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'entry'), 0), COALESCE((SELECT MAX(id) FROM entry), 0)) + 1`
	case "version":
		query = `SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'version'), 0), COALESCE((SELECT MAX(id) FROM version), 0)) + 1`
	case "file":
		query = `SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'file'), 0), COALESCE((SELECT MAX(id) FROM file), 0)) + 1`
	case "file_version":
		query = `SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'file_version'), 0), COALESCE((SELECT MAX(id) FROM file_version), 0)) + 1`
	default:
		return 0, fmt.Errorf("no id sequence for table %q", table)
	}
	var id int64
	if err := tx.QueryRow(query).Scan(&id); err != nil {
		return 0, fmt.Errorf("failed to assign an id in %s: %w", table, err)
	}
	return id, nil
}
