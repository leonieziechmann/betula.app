package store

import (
	"database/sql"
	"encoding/base64"
	"errors"
	"fmt"
	"time"
)

// FileVersion is one version of a named file: content, or a tombstone left by a delete.
type FileVersion struct {
	ID           int64     `json:"version"`
	FileID       int64     `json:"-"`
	Name         string    `json:"name"`
	Deleted      bool      `json:"deleted,omitempty"`
	Hash         string    `json:"sha256"` // the blob, lower-case hex; "" for a tombstone
	Size         int64     `json:"size"`
	ContentType  string    `json:"content_type"`
	CreatedAt    time.Time `json:"created_at"`
	SupersededAt time.Time `json:"superseded_at,omitzero"`
}

// PutFile stores blob as the content of the file name. When the current version (not a
// tombstone) has the same sha256 and content type, nothing changes: it returns that version,
// created false and no journal entry (a zero JournalEntry). Otherwise a new version becomes
// current and the old one is superseded at at (op file_version).
//
// An invalid name, or a content type that is not UTF-8 or has control characters (a tab
// allowed), returns ErrInvalidInput. The blob must be stored (PutBlob) before, else
// ErrBlobMissing; its time is renewed.
func (s *Store) PutFile(name string, blob BlobInfo, contentType string, at time.Time) (FileVersion, bool, JournalEntry, error) {
	if !ValidFileName(name) {
		return FileVersion{}, false, JournalEntry{}, fmt.Errorf("invalid file name %q: %w", name, ErrInvalidInput)
	}
	if !ValidHash(blob.Hash) {
		return FileVersion{}, false, JournalEntry{}, fmt.Errorf("file %s: blob %q: %w", name, blob.Hash, ErrInvalidHash)
	}
	if blob.Size < 0 {
		return FileVersion{}, false, JournalEntry{}, fmt.Errorf("file %s: invalid size %d", name, blob.Size)
	}
	if !validHeaderText(contentType) {
		return FileVersion{}, false, JournalEntry{}, fmt.Errorf("file %s: %w: content type %q", name, ErrInvalidInput, contentType)
	}
	at = stamp(at)
	atText := FormatTime(at)

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	defer func() { _ = tx.Rollback() }()

	// Before the unchanged case too: a put of content that is not stored is refused even when
	// the current version names it.
	if err := s.renewBlob(blob.Hash); err != nil {
		return FileVersion{}, false, JournalEntry{}, fmt.Errorf("file %s: %w", name, err)
	}
	f, current, err := fileState(tx, name)
	if err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	if current != nil && current.Deleted == 0 && current.SHA256 == blob.Hash && current.ContentType == contentType {
		fv, err := current.public(name)
		return fv, false, JournalEntry{}, err
	}
	if f == nil {
		id, err := nextID(tx, "file")
		if err != nil {
			return FileVersion{}, false, JournalEntry{}, err
		}
		f = &fileRow{ID: id, Name: name, CreatedAt: atText}
	}
	id, err := nextID(tx, "file_version")
	if err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	p := fileVersionPayload{File: *f, Version: fileVersionRow{ID: id, FileID: f.ID, SHA256: blob.Hash, Size: blob.Size,
		ContentType: contentType, CreatedAt: atText}}
	p.File.CurrentVersion = &id
	if current != nil {
		p.Superseded = &supersede{ID: current.ID, At: atText}
	}
	if err := applyFileVersion(tx, p); err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	e, err := s.commit(tx, OpFileVersion, at, p, blob.Hash)
	if err != nil {
		return FileVersion{}, false, JournalEntry{}, err
	}
	fv, err := p.Version.public(name)
	return fv, true, e, err
}

// DeleteFile leaves a tombstone as the current version of name (op file_version). The earlier
// versions stay readable by time and id until retention removes them. ErrNotFound when there is
// no such file or it is deleted already.
func (s *Store) DeleteFile(name string, at time.Time) (JournalEntry, error) {
	at = stamp(at)
	atText := FormatTime(at)
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

	f, current, err := fileState(tx, name)
	if err != nil {
		return JournalEntry{}, err
	}
	if current == nil || current.Deleted != 0 {
		return JournalEntry{}, ErrNotFound
	}
	id, err := nextID(tx, "file_version")
	if err != nil {
		return JournalEntry{}, err
	}
	p := fileVersionPayload{File: *f, Version: fileVersionRow{ID: id, FileID: f.ID, Deleted: 1, CreatedAt: atText},
		Superseded: &supersede{ID: current.ID, At: atText}}
	p.File.CurrentVersion = &id
	if err := applyFileVersion(tx, p); err != nil {
		return JournalEntry{}, err
	}
	return s.commit(tx, OpFileVersion, at, p, "")
}

// GetFile returns the current version of name. ErrNotFound when there is none or it is a
// tombstone.
func (s *Store) GetFile(name string) (FileVersion, error) {
	return s.fileVersion(`SELECT `+fileVersionColumns("v")+`, f.name FROM file f JOIN file_version v ON v.id = f.current_version
		WHERE f.name = ?`, name)
}

// GetFileAt returns the version of name that was current at a time. ErrNotFound when there was
// none or it was a tombstone.
func (s *Store) GetFileAt(name string, at time.Time) (FileVersion, error) {
	return s.fileVersion(`SELECT `+fileVersionColumns("v")+`, f.name FROM file f JOIN file_version v ON v.file_id = f.id
		WHERE f.name = ?1 AND v.created_at <= ?2 AND (v.superseded_at IS NULL OR v.superseded_at > ?2)
		ORDER BY v.created_at DESC, v.id DESC LIMIT 1`, name, FormatTime(stamp(at)))
}

// GetFileVersion returns the version id of name. ErrNotFound when name has no such version or
// it is a tombstone.
func (s *Store) GetFileVersion(name string, id int64) (FileVersion, error) {
	return s.fileVersion(`SELECT `+fileVersionColumns("v")+`, f.name FROM file f JOIN file_version v ON v.file_id = f.id
		WHERE f.name = ? AND v.id = ?`, name, id)
}

func (s *Store) fileVersion(query string, args ...any) (FileVersion, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return FileVersion{}, err
	}
	fv, err := scanFileVersion(s.r.QueryRow(query, args...))
	if err != nil {
		return FileVersion{}, err
	}
	if fv.Deleted {
		return FileVersion{}, ErrNotFound
	}
	return fv, nil
}

// FileVersions returns every version of name that retention has kept, tombstones included, the
// newest first. ErrNotFound when there is no such file.
func (s *Store) FileVersions(name string) ([]FileVersion, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return nil, err
	}
	rows, err := s.r.Query(`SELECT `+fileVersionColumns("v")+`, f.name FROM file f JOIN file_version v ON v.file_id = f.id
		WHERE f.name = ? ORDER BY v.id DESC`, name)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	versions := []FileVersion{}
	for rows.Next() {
		fv, err := scanFileVersion(rows)
		if err != nil {
			return nil, err
		}
		versions = append(versions, fv)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	if len(versions) == 0 {
		return nil, ErrNotFound
	}
	return versions, nil
}

// ListFiles returns the current versions of the files (not deleted) whose name starts with
// prefix, ordered by name, at most limit (≤ 0: DefaultLimit, at most MaxLimit). next is the
// cursor for the following page, "" after the last.
func (s *Store) ListFiles(prefix, cursor string, limit int) ([]FileVersion, string, error) {
	after := ""
	if cursor != "" {
		b, err := base64.RawURLEncoding.DecodeString(cursor)
		if err != nil || len(b) == 0 {
			return nil, "", fmt.Errorf("%w: %q", ErrInvalidCursor, cursor)
		}
		after = string(b)
	}
	limit = clampLimit(limit)
	upper := prefixEnd(prefix)

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return nil, "", err
	}
	query := `SELECT ` + fileVersionColumns("v") + `, f.name FROM file f JOIN file_version v ON v.id = f.current_version
		WHERE v.deleted = 0 AND f.name >= ? AND f.name > ?`
	args := []any{prefix, after}
	if upper != "" {
		query += ` AND f.name < ?`
		args = append(args, upper)
	}
	rows, err := s.r.Query(query+` ORDER BY f.name LIMIT ?`, append(args, limit+1)...)
	if err != nil {
		return nil, "", err
	}
	defer rows.Close()
	files := []FileVersion{}
	for rows.Next() {
		fv, err := scanFileVersion(rows)
		if err != nil {
			return nil, "", err
		}
		files = append(files, fv)
	}
	if err := rows.Err(); err != nil {
		return nil, "", err
	}
	next := ""
	if len(files) > limit {
		files = files[:limit]
		next = base64.RawURLEncoding.EncodeToString([]byte(files[limit-1].Name))
	}
	return files, next, nil
}

// prefixEnd is the smallest string greater than every string that starts with prefix (in byte
// order, which is SQLite's for TEXT), or "" when there is none.
func prefixEnd(prefix string) string {
	b := []byte(prefix)
	for i := len(b) - 1; i >= 0; i-- {
		if b[i] < 0xff {
			b[i]++
			return string(b[:i+1])
		}
	}
	return ""
}

type fileRow struct {
	ID             int64  `json:"id"`
	Name           string `json:"name"`
	CreatedAt      string `json:"created_at"`
	CurrentVersion *int64 `json:"current_version"`
}

type fileVersionRow struct {
	ID           int64   `json:"id"`
	FileID       int64   `json:"file_id"`
	Deleted      int     `json:"deleted"`
	SHA256       string  `json:"sha256"`
	Size         int64   `json:"size"`
	ContentType  string  `json:"content_type"`
	CreatedAt    string  `json:"created_at"`
	SupersededAt *string `json:"superseded_at"`
}

func fileVersionColumns(a string) string {
	return a + ".id, " + a + ".file_id, " + a + ".deleted, " + a + ".sha256, " + a + ".size, " + a + ".content_type, " +
		a + ".created_at, " + a + ".superseded_at"
}

func (v *fileVersionRow) dest() []any {
	return []any{&v.ID, &v.FileID, &v.Deleted, &v.SHA256, &v.Size, &v.ContentType, &v.CreatedAt, &v.SupersededAt}
}

func scanFileVersion(sc scanner) (FileVersion, error) {
	var v fileVersionRow
	var name string
	err := sc.Scan(append(v.dest(), &name)...)
	if errors.Is(err, sql.ErrNoRows) {
		return FileVersion{}, ErrNotFound
	}
	if err != nil {
		return FileVersion{}, err
	}
	return v.public(name)
}

func (v fileVersionRow) public(name string) (FileVersion, error) {
	fv := FileVersion{ID: v.ID, FileID: v.FileID, Name: name, Deleted: v.Deleted != 0, Hash: v.SHA256, Size: v.Size,
		ContentType: v.ContentType}
	var err error
	if fv.CreatedAt, err = parseTime(v.CreatedAt); err != nil {
		return FileVersion{}, fmt.Errorf("file version %d: %w", v.ID, err)
	}
	if fv.SupersededAt, err = parseNullTime(v.SupersededAt); err != nil {
		return FileVersion{}, fmt.Errorf("file version %d: %w", v.ID, err)
	}
	return fv, nil
}

// fileState returns the file row of name and its current version (nil when absent).
func fileState(tx *sql.Tx, name string) (*fileRow, *fileVersionRow, error) {
	var f fileRow
	err := tx.QueryRow(`SELECT id, name, created_at, current_version FROM file WHERE name = ?`, name).
		Scan(&f.ID, &f.Name, &f.CreatedAt, &f.CurrentVersion)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil, nil
	}
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read file %q: %w", name, err)
	}
	if f.CurrentVersion == nil {
		return &f, nil, nil
	}
	var v fileVersionRow
	err = tx.QueryRow(`SELECT `+fileVersionColumns("v")+` FROM file_version v WHERE v.id = ?`, *f.CurrentVersion).Scan(v.dest()...)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read version %d of file %q: %w", *f.CurrentVersion, name, err)
	}
	return &f, &v, nil
}

// applyFileVersion writes the rows of a file_version entry: the file row, the superseded
// version, and the new version.
func applyFileVersion(tx *sql.Tx, p fileVersionPayload) error {
	f := p.File
	_, err := tx.Exec(`INSERT INTO file (id, name, created_at, current_version) VALUES (?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET name = excluded.name, created_at = excluded.created_at,
			current_version = excluded.current_version`,
		f.ID, f.Name, f.CreatedAt, f.CurrentVersion)
	if err != nil {
		return fmt.Errorf("failed to write file %d: %w", f.ID, err)
	}
	if p.Superseded != nil {
		if err := execOne(tx, `UPDATE file_version SET superseded_at = ? WHERE id = ? AND file_id = ? AND superseded_at IS NULL`,
			p.Superseded.At, p.Superseded.ID, f.ID); err != nil {
			return fmt.Errorf("failed to supersede file version %d: %w", p.Superseded.ID, err)
		}
	}
	v := p.Version
	_, err = tx.Exec(`INSERT INTO file_version (id, file_id, deleted, sha256, size, content_type, created_at, superseded_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		v.ID, v.FileID, v.Deleted, v.SHA256, v.Size, v.ContentType, v.CreatedAt, v.SupersededAt)
	if err != nil {
		return fmt.Errorf("failed to write file version %d: %w", v.ID, err)
	}
	return nil
}
