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
	SourceQISFUESList   = "qis_fues_list"
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
	if p.Source == "" || p.Key == "" {
		return fmt.Errorf("raw page requires source and key")
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
			return err
		}
		if err := zw.Close(); err != nil {
			return err
		}
		bodyGz = buf.Bytes()
	}

	_, err := db.sql.Exec(`
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
	return err
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
