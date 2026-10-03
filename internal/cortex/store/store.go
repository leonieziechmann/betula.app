// Package store is Cortex's storage: content-addressed blobs, the SQLite index of the fetched
// URLs and the named files with their history, and the journal a follower replays to keep an
// identical copy (docs/cortex.md §3 and §5).
//
// Layout of the data directory:
//
//	DIR/index.db                       the index (SQLite, WAL)
//	DIR/blobs/sha256/<hh>/<hex>        a blob as it was given (raw)
//	DIR/blobs/sha256/<hh>/<hex>.gz     a blob compressed with gzip
//	DIR/tmp/                           writes in progress; emptied at Open
//
// Every write to the index is one transaction that also appends one journal entry. The leader
// assigns the ids and the times; Apply writes the same rows on a follower, through the same
// code, so that the two indexes stay identical row for row.
package store

import (
	"context"
	"database/sql"
	"embed"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/internal/oplog"
	"modernc.org/sqlite" // also registers the driver "sqlite"
)

//go:embed migrations/*.sql
var migrationFS embed.FS

var (
	// ErrNotFound is returned for an entry, version, file or blob that does not exist (a
	// deleted file counts as absent).
	ErrNotFound = errors.New("not found")
	// ErrHashMismatch is returned when content does not have the sha256 it was expected to have.
	ErrHashMismatch = errors.New("hash mismatch")
	// ErrTooLarge is returned when content is longer than the limit it was given.
	ErrTooLarge = errors.New("too large")
	// ErrCompressed is returned by OpenRaw for a blob that is stored compressed.
	ErrCompressed = errors.New("blob is stored compressed")
	// ErrOutOfOrder is returned by Apply for an entry that does not follow the local journal.
	ErrOutOfOrder = errors.New("journal entry out of order")
	// ErrDiverged is returned by Apply for an entry that does not fit the local index (a row it
	// changes is missing, or one it adds exists): the follower's copy is not the leader's.
	ErrDiverged = errors.New("index diverged from the journal")
	// ErrInvalidHash is returned for a hash that is not 64 lower-case hex digits.
	ErrInvalidHash = errors.New("invalid sha256")
	// ErrInvalidCursor is returned for a listing cursor that no listing returned.
	ErrInvalidCursor = errors.New("invalid cursor")
	// ErrClosed is returned after Close, or after ReplaceIndex failed to open the new index.
	ErrClosed = errors.New("store closed")
	// ErrBlobMissing is returned by a write that would reference a blob that is not stored
	// (Apply, RecordFetch, PutFile); nothing is written. A follower fetches the blob (the
	// entry's Blob) from the leader and applies the same entry again.
	ErrBlobMissing = errors.New("blob missing")
	// ErrInvalidInput is returned, wrapped, for a write given text the index cannot keep as it
	// is: text that is not UTF-8 (encoding/json would carry it to a follower as U+FFFD), a
	// content type with control characters, a source outside [a-z0-9_.-]{1,64}, an invalid
	// file name.
	ErrInvalidInput = errors.New("invalid input")
)

// readers is the size of the pool of read connections.
const readers = 8

// Store is one data directory. It is safe for concurrent use.
type Store struct {
	dir string

	// mu guards the index handles: every operation on the index holds it for reading, and
	// ReplaceIndex and Close for writing, so no reader ever uses a closed handle.
	mu   sync.RWMutex
	w    *sql.DB // the one writer: one connection, BEGIN IMMEDIATE
	r    *sql.DB // the readers (query_only)
	shut bool    // Close was called

	// blobMu serialises placing a blob file with collecting one, so that a blob that a write
	// is about to reference is never removed (see GCBlobs).
	blobMu sync.Mutex

	// commitMu makes a commit and the move of the cached position one step: a writer holds it
	// from tx.Commit to advance, so the next writer's entry cannot become the head before this
	// one has, and CommittedHead, which takes it, sees the position the database has (E2E-2).
	commitMu sync.Mutex

	posMu   sync.Mutex
	pos     position
	changed chan struct{} // closed and replaced whenever the journal moves
	closed  bool          // Close was called (shut, for WaitAfter)

	stats statsCache

	// pendingMu guards pendingDirs, the blob directories that ImportStored renamed a file into
	// without syncing; syncPending syncs them before a commit can reference the blob. syncMu
	// keeps a commit from passing a sync that another goroutine has begun but not finished.
	pendingMu   sync.Mutex
	pendingDirs map[string]struct{}
	syncMu      sync.Mutex

	// snapshotSlot holds the one copy of the index that OpenSnapshot may have on disk at a time.
	snapshotSlot chan struct{}

	now func() time.Time // for the times a caller does not give (DeleteEntry)
}

// position is where the journal stands: its newest and its oldest entry.
type position struct {
	seq, epoch int64
	at         time.Time
	oldest     int64
}

// Open opens the data directory dir, creating the layout and the index when they do not
// exist, removes what an earlier process left in DIR/tmp and applies pending migrations.
// Log events: db.migrated.
func Open(dir string) (*Store, error) {
	s := &Store{dir: dir, changed: make(chan struct{}), now: time.Now, snapshotSlot: make(chan struct{}, 1)}
	for _, d := range []string{dir, s.blobRoot(), s.tmpDir()} {
		if err := os.MkdirAll(d, 0755); err != nil {
			return nil, fmt.Errorf("failed to create %s: %w", d, err)
		}
	}
	if err := s.emptyTmp(); err != nil {
		return nil, err
	}
	if err := s.openIndex(); err != nil {
		return nil, err
	}
	active.Store(s)
	return s, nil
}

// Dir returns the data directory.
func (s *Store) Dir() string {
	return s.dir
}

// Close closes the index. Blob operations keep working on the files; everything that needs the
// index returns ErrClosed.
func (s *Store) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return nil
	}
	s.shut = true
	active.CompareAndSwap(s, nil)
	// Every commit synced the directories of the blobs it references; what is left are blobs
	// no row names yet, synced here so that a clean stop keeps them all the same.
	err := errors.Join(s.syncPending(), s.closeIndex())

	s.posMu.Lock()
	s.closed = true
	close(s.changed) // wakes WaitAfter, which then sees closed
	s.changed = make(chan struct{})
	s.posMu.Unlock()
	return err
}

// Ping runs a trivial query against the index, for a liveness check.
func (s *Store) Ping(ctx context.Context) error {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return err
	}
	var n int64
	return s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM meta").Scan(&n)
}

func (s *Store) indexPath() string { return filepath.Join(s.dir, "index.db") }
func (s *Store) blobRoot() string  { return filepath.Join(s.dir, "blobs", "sha256") }
func (s *Store) tmpDir() string    { return filepath.Join(s.dir, "tmp") }

// ready says whether the index handles can be used; the caller holds mu.
func (s *Store) ready() error {
	if s.shut || s.w == nil {
		return ErrClosed
	}
	return nil
}

// emptyTmp removes what a write in progress left behind when its process ended.
func (s *Store) emptyTmp() error {
	entries, err := os.ReadDir(s.tmpDir())
	if err != nil {
		return fmt.Errorf("failed to read %s: %w", s.tmpDir(), err)
	}
	for _, e := range entries {
		if err := os.RemoveAll(filepath.Join(s.tmpDir(), e.Name())); err != nil {
			return fmt.Errorf("failed to empty %s: %w", s.tmpDir(), err)
		}
	}
	return nil
}

// dsn is catalogdb's: busy_timeout, WAL, synchronous NORMAL and foreign keys on every
// connection (the _pragma values are applied per connection).
func dsn(path string) string {
	return path + "?_pragma=busy_timeout(60000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)&_pragma=foreign_keys(ON)"
}

// openIndex opens the writer and the readers on DIR/index.db, migrates it and loads the
// journal position. The caller holds mu for writing (or has not published s yet).
func (s *Store) openIndex() error {
	path := s.indexPath()
	// One writer: a single connection whose transactions take the write lock at BEGIN, so
	// that two writes never start on the same snapshot and fail on upgrading their lock.
	w, err := sql.Open("sqlite", dsn(path)+"&_txlock=immediate")
	if err != nil {
		return fmt.Errorf("failed to open %s: %w", path, err)
	}
	w.SetMaxOpenConns(1)
	w.SetMaxIdleConns(1)
	w.SetConnMaxLifetime(0)
	w.SetConnMaxIdleTime(0)
	if err := migrate(w, path); err != nil {
		_ = w.Close()
		return err
	}

	r, err := sql.Open("sqlite", dsn(path)+"&_pragma=query_only(1)")
	if err != nil {
		_ = w.Close()
		return fmt.Errorf("failed to open %s: %w", path, err)
	}
	r.SetMaxOpenConns(readers)
	r.SetMaxIdleConns(readers)
	if err := r.Ping(); err != nil {
		_ = r.Close()
		_ = w.Close()
		return fmt.Errorf("failed to open %s for reading: %w", path, err)
	}

	pos, err := loadPosition(w)
	if err != nil {
		_ = r.Close()
		_ = w.Close()
		return err
	}
	s.w, s.r = w, r
	s.posMu.Lock()
	s.pos = pos
	close(s.changed)
	s.changed = make(chan struct{})
	s.posMu.Unlock()
	return nil
}

// closeIndex checkpoints the WAL into the database file and closes both handles. The caller
// holds mu for writing.
func (s *Store) closeIndex() error {
	if s.w == nil {
		return nil
	}
	var errs []error
	if _, err := s.w.Exec("PRAGMA wal_checkpoint(TRUNCATE)"); err != nil {
		errs = append(errs, fmt.Errorf("failed to checkpoint the index: %w", err))
	}
	if err := s.r.Close(); err != nil {
		errs = append(errs, err)
	}
	if err := s.w.Close(); err != nil {
		errs = append(errs, err)
	}
	s.w, s.r = nil, nil
	return errors.Join(errs...)
}

type migration struct {
	version int
	name    string
	sql     string
}

// loadMigrations reads migrations/NNNN_name.sql in version order. A gap or a duplicate number
// is an error: the version is the only record of the index's shape.
func loadMigrations() ([]migration, error) {
	entries, err := fs.ReadDir(migrationFS, "migrations")
	if err != nil {
		return nil, err
	}
	var result []migration
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || !strings.HasSuffix(name, ".sql") {
			continue
		}
		prefix, _, ok := strings.Cut(name, "_")
		if !ok {
			return nil, fmt.Errorf("migration %q: expected NNNN_name.sql", name)
		}
		version, err := strconv.Atoi(prefix)
		if err != nil || version <= 0 {
			return nil, fmt.Errorf("migration %q: invalid version prefix", name)
		}
		body, err := migrationFS.ReadFile("migrations/" + name)
		if err != nil {
			return nil, err
		}
		result = append(result, migration{version: version, name: name, sql: string(body)})
	}
	sort.Slice(result, func(i, j int) bool { return result[i].version < result[j].version })
	for i, m := range result {
		if m.version != i+1 {
			return nil, fmt.Errorf("migration %q: expected version %d", m.name, i+1)
		}
	}
	return result, nil
}

// migrate applies the pending migrations, each in its own transaction, and refuses an index
// that a newer binary has migrated further. Log events: db.migrated.
func migrate(db *sql.DB, path string) error {
	migrations, err := loadMigrations()
	if err != nil {
		return fmt.Errorf("failed to load migrations: %w", err)
	}
	var current int
	if err := db.QueryRow("PRAGMA user_version").Scan(&current); err != nil {
		return fmt.Errorf("failed to read the schema version of %s: %w", path, err)
	}
	if current > len(migrations) {
		return fmt.Errorf("index %s has schema version %d, this binary only knows %d", path, current, len(migrations))
	}
	for _, m := range migrations[current:] {
		tx, err := db.Begin()
		if err != nil {
			return err
		}
		if _, err := tx.Exec(m.sql); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s failed: %w", m.name, err)
		}
		// user_version lives in the database header and is part of the transaction.
		if _, err := tx.Exec(fmt.Sprintf("PRAGMA user_version = %d", m.version)); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s: failed to set version: %w", m.name, err)
		}
		if err := tx.Commit(); err != nil {
			return fmt.Errorf("migration %s: commit failed: %w", m.name, err)
		}
		oplog.For("store").Info("migration applied", "event", "db.migrated", "migration", m.name, "schema_version", m.version, "db", path)
	}
	return nil
}

// isConstraint says whether err is SQLite's constraint violation (a duplicate id or key, a
// missing parent row).
func isConstraint(err error) bool {
	var se *sqlite.Error
	return errors.As(err, &se) && se.Code()&0xff == 19 // SQLITE_CONSTRAINT
}

// syncDir makes a rename or a removal in dir durable.
func syncDir(dir string) error {
	return syncPath(dir)
}

// syncPath syncs the file or directory at path.
func syncPath(path string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	err = syncFile(f)
	if cerr := f.Close(); err == nil {
		err = cerr
	}
	return err
}

// syncFile is every fsync of the store, a variable so that a test can see the order of them.
var syncFile = func(f *os.File) error { return f.Sync() }

// syncIndex makes every committed write to the index durable. With WAL and synchronous=NORMAL
// a commit is written to the WAL but synced only at the next checkpoint, so that a power loss
// may undo it; syncing the WAL is what synchronous=FULL would have done at the commit. SQLite
// syncs the directory itself when it creates the WAL. The caller holds mu for reading, so
// that ReplaceIndex does not swap the files meanwhile.
func (s *Store) syncIndex() error {
	err := syncPath(s.indexPath() + "-wal")
	if errors.Is(err, fs.ErrNotExist) {
		return nil // no WAL, so no commit that only the WAL holds
	}
	if err != nil {
		return fmt.Errorf("failed to sync the index: %w", err)
	}
	return nil
}
