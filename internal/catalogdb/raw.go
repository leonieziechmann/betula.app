package catalogdb

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"time"
)

// Raw page sources. The archive accepts any source string so that a new scraper
// does not need a migration; these are the ones the build step knows.
const (
	SourceModuleCatalog = "module_catalog"
	SourceModulePage    = "module_page"
	SourceQISEvent      = "qis_event"
	SourceQISEventEntry = "qis_event_entry" // an event's entry in the QIS event search, keyed like its page
	SourceQISFUESList   = "qis_fues_list"
	SourceQISModuleList = "qis_module_list"
	SourceQISModulePage = "qis_module_page"
	SourceQISTree       = "qis_tree"
)

// ErrNotFound is returned when a requested row does not exist.
var ErrNotFound = errors.New("not found")

// RawPage is one archived response.
type RawPage struct {
	Source     string
	Key        string
	URL        string
	FetchedAt  time.Time
	ChangedAt  time.Time
	HTTPStatus int
	Hash       string
	Body       []byte
}

// PutPage stores the latest response for (source, key), replacing the previous
// one. changed_at only moves when the body differs from the archived body.
func (db *DB) PutPage(p RawPage) error {
	_, err := db.PutPageChanged(p)
	return err
}

// PutPageChanged is PutPage and reports whether the body differs from the archived
// one (true for a page that was not archived before).
func (db *DB) PutPageChanged(p RawPage) (bool, error) {
	changed, err := db.putPage(p)
	return changed, err
}

func (db *DB) putPage(p RawPage) (bool, error) {
	if p.Source == "" || p.Key == "" {
		return false, fmt.Errorf("raw page requires source and key")
	}
	if p.FetchedAt.IsZero() {
		p.FetchedAt = time.Now()
	}
	fetchedAt := p.FetchedAt.UTC().Format(time.RFC3339)

	var hash string
	var bodyGz []byte
	if len(p.Body) > 0 {
		sum := sha256.Sum256(p.Body)
		hash = hex.EncodeToString(sum[:])
		var buf bytes.Buffer
		zw, _ := gzip.NewWriterLevel(&buf, gzip.BestCompression)
		if _, err := zw.Write(p.Body); err != nil {
			return false, err
		}
		if err := zw.Close(); err != nil {
			return false, err
		}
		bodyGz = buf.Bytes()
	}

	var previous string
	err := db.sql.QueryRow("SELECT content_hash FROM raw_page WHERE source = ? AND key = ?", p.Source, p.Key).Scan(&previous)
	changed := err != nil || previous != hash

	_, err = db.sql.Exec(`
		INSERT INTO raw_page (source, key, source_url, fetched_at, changed_at, http_status, content_hash, body_gz)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(source, key) DO UPDATE SET
			source_url = excluded.source_url,
			fetched_at = excluded.fetched_at,
			changed_at = CASE WHEN raw_page.content_hash = excluded.content_hash
				THEN raw_page.changed_at ELSE excluded.changed_at END,
			http_status = excluded.http_status,
			content_hash = excluded.content_hash,
			body_gz = excluded.body_gz
	`, p.Source, p.Key, p.URL, fetchedAt, fetchedAt, p.HTTPStatus, hash, bodyGz)
	return changed, err
}

// GetPage returns the archived response for (source, key).
func (db *DB) GetPage(source, key string) (*RawPage, error) {
	row := db.sql.QueryRow(`
		SELECT source, key, source_url, fetched_at, changed_at, http_status, content_hash, body_gz
		FROM raw_page WHERE source = ? AND key = ?
	`, source, key)
	p, err := scanRawPage(row)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	return p, err
}

// PageFetchedAt returns when (source, key) was last fetched, or the zero time.
func (db *DB) PageFetchedAt(source, key string) (time.Time, error) {
	var fetchedAt string
	err := db.sql.QueryRow("SELECT fetched_at FROM raw_page WHERE source = ? AND key = ?", source, key).Scan(&fetchedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return time.Time{}, nil
	}
	if err != nil {
		return time.Time{}, err
	}
	return time.Parse(time.RFC3339, fetchedAt)
}

// EachPage calls fn for every archived page of a source, ordered by key.
func (db *DB) EachPage(source string, fn func(*RawPage) error) error {
	rows, err := db.sql.Query(`
		SELECT source, key, source_url, fetched_at, changed_at, http_status, content_hash, body_gz
		FROM raw_page WHERE source = ? ORDER BY key
	`, source)
	if err != nil {
		return err
	}
	defer rows.Close()

	for rows.Next() {
		p, err := scanRawPage(rows)
		if err != nil {
			return err
		}
		if err := fn(p); err != nil {
			return err
		}
	}
	return rows.Err()
}

func scanRawPage(scanner interface{ Scan(...interface{}) error }) (*RawPage, error) {
	var (
		p                    RawPage
		fetchedAt, changedAt string
		bodyGz               []byte
	)
	if err := scanner.Scan(&p.Source, &p.Key, &p.URL, &fetchedAt, &changedAt, &p.HTTPStatus, &p.Hash, &bodyGz); err != nil {
		return nil, err
	}
	var err error
	if p.FetchedAt, err = time.Parse(time.RFC3339, fetchedAt); err != nil {
		return nil, fmt.Errorf("raw page %s/%s: invalid fetched_at: %w", p.Source, p.Key, err)
	}
	if p.ChangedAt, err = time.Parse(time.RFC3339, changedAt); err != nil {
		return nil, fmt.Errorf("raw page %s/%s: invalid changed_at: %w", p.Source, p.Key, err)
	}
	if len(bodyGz) > 0 {
		zr, err := gzip.NewReader(bytes.NewReader(bodyGz))
		if err != nil {
			return nil, fmt.Errorf("raw page %s/%s: %w", p.Source, p.Key, err)
		}
		defer zr.Close()
		if p.Body, err = io.ReadAll(zr); err != nil {
			return nil, fmt.Errorf("raw page %s/%s: %w", p.Source, p.Key, err)
		}
	}
	return &p, nil
}

// PageState is when a page was fetched and when its content last changed, and how the
// server answered.
type PageState struct {
	FetchedAt, ChangedAt time.Time
	HTTPStatus           int
}

// PageStates returns the state of every archived page of a source, without reading a body.
func (db *DB) PageStates(source string) (map[string]PageState, error) {
	rows, err := db.sql.Query("SELECT key, fetched_at, changed_at, http_status FROM raw_page WHERE source = ?", source)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	result := make(map[string]PageState)
	for rows.Next() {
		var key, fetchedAt, changedAt string
		var st PageState
		if err := rows.Scan(&key, &fetchedAt, &changedAt, &st.HTTPStatus); err != nil {
			return nil, err
		}
		if st.FetchedAt, err = time.Parse(time.RFC3339, fetchedAt); err != nil {
			return nil, fmt.Errorf("raw page %s/%s: invalid fetched_at: %w", source, key, err)
		}
		if st.ChangedAt, err = time.Parse(time.RFC3339, changedAt); err != nil {
			return nil, fmt.Errorf("raw page %s/%s: invalid changed_at: %w", source, key, err)
		}
		result[key] = st
	}
	return result, rows.Err()
}

// FetchTimes returns when each page of a source was last fetched.
func (db *DB) FetchTimes(source string) (map[string]time.Time, error) {
	rows, err := db.sql.Query("SELECT key, fetched_at FROM raw_page WHERE source = ?", source)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	result := make(map[string]time.Time)
	for rows.Next() {
		var key, fetchedAt string
		if err := rows.Scan(&key, &fetchedAt); err != nil {
			return nil, err
		}
		t, err := time.Parse(time.RFC3339, fetchedAt)
		if err != nil {
			return nil, fmt.Errorf("raw page %s/%s: invalid fetched_at: %w", source, key, err)
		}
		result[key] = t
	}
	return result, rows.Err()
}

// ArchiveStat sums up the archived pages of one source.
type ArchiveStat struct {
	Source          string
	Pages, NotFound int       // all pages; of them the ones the server answered with 404
	FetchedSince    int       // fetched at or after the time ArchiveStats was given
	ChangedSince    int       // whose body changed at or after it (or that are new since)
	Oldest, Newest  time.Time // the oldest and the newest fetch
}

// ArchiveStats sums up the archive per source, without reading a body: for monitoring.
func (db *DB) ArchiveStats(since time.Time) ([]ArchiveStat, error) {
	at := since.UTC().Format(time.RFC3339)
	rows, err := db.sql.Query(`
		SELECT source, COUNT(*), SUM(http_status = 404), SUM(fetched_at >= ?), SUM(changed_at >= ?),
			MIN(fetched_at), MAX(fetched_at)
		FROM raw_page GROUP BY source ORDER BY source
	`, at, at)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []ArchiveStat
	for rows.Next() {
		var st ArchiveStat
		var oldest, newest string
		if err := rows.Scan(&st.Source, &st.Pages, &st.NotFound, &st.FetchedSince, &st.ChangedSince, &oldest, &newest); err != nil {
			return nil, err
		}
		if st.Oldest, err = time.Parse(time.RFC3339, oldest); err != nil {
			return nil, fmt.Errorf("raw pages of %s: invalid fetched_at: %w", st.Source, err)
		}
		if st.Newest, err = time.Parse(time.RFC3339, newest); err != nil {
			return nil, fmt.Errorf("raw pages of %s: invalid fetched_at: %w", st.Source, err)
		}
		result = append(result, st)
	}
	return result, rows.Err()
}
