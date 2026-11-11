// Package store is Phloem's storage (docs/phloem/phloem.md, „Storage"): the spaces of the
// profiles that sync, their devices, the sealed entries after each space's snapshot, the
// snapshots, the parcels for devices to come and the parts of shared calendars, with what became
// of the parcels and parts that are no longer there (their tombstones), in one SQLite file.
// Phloem never opens what a space holds; the store keeps it as it came and counts its bytes for
// the budget.
//
// One writer (a single connection, BEGIN IMMEDIATE, synchronous FULL: what Phloem acknowledged
// as stored survives a power loss) and a pool of readers. Every write that changes the bytes of
// a space changes the cached total in the same step, under the writer's lock.
package store

import (
	"context"
	"crypto/sha256"
	"crypto/subtle"
	"database/sql"
	"embed"
	"encoding/hex"
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

	"github.com/leonieziechmann/betula/phloem/internal/oplog"
	"modernc.org/sqlite" // also registers the driver "sqlite"
)

//go:embed migrations/*.sql
var migrationFS embed.FS

var (
	// ErrNotFound: no such space, snapshot, parcel or part (an expired parcel counts as absent).
	ErrNotFound = errors.New("not found")
	// ErrExists: a space, a parcel or a part of this id is there already, or a tombstone of it.
	ErrExists = errors.New("exists")
	// ErrUnauthorized: the token is not the space's.
	ErrUnauthorized = errors.New("unauthorized")
	// ErrFull: every slot of the space is taken.
	ErrFull = errors.New("full")
	// ErrTooMany: the space has as many parcels waiting, or as many parts, as it may.
	ErrTooMany = errors.New("too many")
	// ErrGone: a part that was deleted, or whose space was.
	ErrGone = errors.New("gone")
	// ErrCompactFirst: the entries after the snapshot would exceed the hard limit; a snapshot
	// has to come first.
	ErrCompactFirst = errors.New("compact first")
	// ErrStale: a snapshot that covers less than the one kept.
	ErrStale = errors.New("stale snapshot")
	// ErrAhead: a snapshot that claims entries the space does not have.
	ErrAhead = errors.New("snapshot ahead of the entries")
	// ErrClosed is returned after Close.
	ErrClosed = errors.New("store closed")
)

// RowBytes is what a row costs the budget beside its data: an entry, a snapshot, a parcel or a
// part of no data still takes room, and so does a tombstone.
const RowBytes = 64

// The states of a parcel (GET /sync/v1/pairs): one that waits, and what became of one that no
// longer does, as its tombstone keeps it.
const (
	PairWaiting   = "waiting"
	PairTaken     = "taken"
	PairExpired   = "expired"
	PairWithdrawn = "withdrawn"
)

// spaceColumns are the columns scanSpace reads. The last says whether a snapshot exists:
// snap_upto is 0 both without one and with one as of entry 0 (the first device's).
const spaceColumns = "id, token_hash, created, touched, head, snap_upto, tail_bytes, bytes, EXISTS (SELECT 1 FROM snapshots WHERE space = spaces.id)"

// readers is the size of the pool of read connections.
const readers = 4

// Store is one data directory. It is safe for concurrent use.
type Store struct {
	dir string

	// mu guards the handles and the cached totals; every write holds it from the transaction's
	// start to the update of the totals.
	mu    sync.Mutex
	w     *sql.DB // the one writer
	r     *sql.DB // the readers (query_only)
	shut  bool
	total int64 // sum of spaces.bytes, and RowBytes for every part tombstone (it outlives its space)
	count int64 // number of spaces
	epoch []byte
}

// Space is one space as the store keeps it.
type Space struct {
	ID          []byte
	TokenHash   []byte
	Created     time.Time
	Touched     time.Time
	Head        int64 // the number of the newest entry; 0: none yet
	SnapUpto    int64 // the entry the snapshot is as of
	HasSnapshot bool
	TailBytes   int64 // the data of the entries after the snapshot
	Bytes       int64 // everything the space holds, for the budget
}

// Entry is one sealed batch.
type Entry struct {
	Nr   int64
	Slot int
	Data []byte
}

// Snapshot is a space's newest snapshot.
type Snapshot struct {
	Upto int64
	Data []byte
	At   time.Time
}

// Open opens (and creates) dir/phloem.db and applies pending migrations.
// Log events: db.migrated.
func Open(dir string) (*Store, error) {
	if err := os.MkdirAll(dir, 0o750); err != nil {
		return nil, fmt.Errorf("failed to create %s: %w", dir, err)
	}
	s := &Store{dir: dir}
	path := s.path()
	w, err := sql.Open("sqlite", dsn(path)+"&_txlock=immediate")
	if err != nil {
		return nil, fmt.Errorf("failed to open %s: %w", path, err)
	}
	w.SetMaxOpenConns(1)
	w.SetMaxIdleConns(1)
	w.SetConnMaxLifetime(0)
	w.SetConnMaxIdleTime(0)
	if err := migrate(w, path); err != nil {
		_ = w.Close()
		return nil, err
	}
	r, err := sql.Open("sqlite", dsn(path)+"&_pragma=query_only(1)")
	if err != nil {
		_ = w.Close()
		return nil, fmt.Errorf("failed to open %s: %w", path, err)
	}
	r.SetMaxOpenConns(readers)
	r.SetMaxIdleConns(readers)
	if err := r.Ping(); err != nil {
		_ = r.Close()
		_ = w.Close()
		return nil, fmt.Errorf("failed to open %s for reading: %w", path, err)
	}
	var tombstones int64
	if err := w.QueryRow("SELECT (SELECT COUNT(*) FROM spaces), (SELECT COALESCE(SUM(bytes), 0) FROM spaces), (SELECT COUNT(*) FROM part_tombstones)").Scan(&s.count, &s.total, &tombstones); err != nil {
		_ = r.Close()
		_ = w.Close()
		return nil, fmt.Errorf("failed to read the totals of %s: %w", path, err)
	}
	var epoch string
	err = w.QueryRow("SELECT value FROM meta WHERE key = 'epoch'").Scan(&epoch)
	if err == nil {
		s.epoch, err = hex.DecodeString(epoch)
	}
	if err != nil || len(s.epoch) != EpochLen {
		_ = r.Close()
		_ = w.Close()
		return nil, fmt.Errorf("failed to read the epoch of %s (%q): %v", path, epoch, err)
	}
	s.total += tombstones * RowBytes
	s.w, s.r = w, r
	return s, nil
}

func (s *Store) path() string { return filepath.Join(s.dir, "phloem.db") }

// EpochLen is the length of a database's epoch.
const EpochLen = 16

// Epoch is what this database is: 16 random bytes it got once (migration 0003). A device that
// knew another epoch learns that Phloem lost what it held (docs/phloem/protocol.md).
func (s *Store) Epoch() []byte { return s.epoch }

// dsn: busy_timeout, WAL, synchronous FULL and foreign keys on every connection.
func dsn(path string) string {
	return "file:" + path + "?_pragma=busy_timeout(30000)&_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=foreign_keys(ON)"
}

// Close checkpoints the WAL and closes both handles.
func (s *Store) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return nil
	}
	s.shut = true
	var errs []error
	if _, err := s.w.Exec("PRAGMA wal_checkpoint(TRUNCATE)"); err != nil {
		errs = append(errs, fmt.Errorf("failed to checkpoint: %w", err))
	}
	errs = append(errs, s.r.Close(), s.w.Close())
	return errors.Join(errs...)
}

// Ping runs a trivial query, for a liveness check.
func (s *Store) Ping(ctx context.Context) error {
	if s.closed() {
		return ErrClosed
	}
	var n int64
	return s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM meta").Scan(&n)
}

func (s *Store) closed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.shut
}

// Totals are the number of spaces and the bytes the budget counts: what the spaces hold, and a
// row for every part tombstone, which outlives its space (cached, exact).
func (s *Store) Totals() (spaces, bytes int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.count, s.total
}

// Devices counts the devices of every space.
func (s *Store) Devices(ctx context.Context) (int64, error) {
	var n int64
	err := s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM devices").Scan(&n)
	return n, err
}

// TokenHash is what the store keeps of a token.
func TokenHash(token []byte) []byte {
	sum := sha256.Sum256(token)
	return sum[:]
}

// write runs f in one write transaction under mu; f returns the change of the bytes the budget
// counts and of the number of spaces, which are added to the totals once the transaction
// committed.
func (s *Store) write(ctx context.Context, f func(tx *sql.Tx) (dBytes, dSpaces int64, err error)) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return ErrClosed
	}
	tx, err := s.w.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	dBytes, dSpaces, err := f(tx)
	if err != nil {
		_ = tx.Rollback()
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	s.total += dBytes
	s.count += dSpaces
	return nil
}

// CreateSpace makes a space with the hash of its token, and gives its first device slot `slot`:
// 1 for a new profile, the one it had for a space made anew after Phloem lost it.
func (s *Store) CreateSpace(ctx context.Context, id, tokenHash, device []byte, slot int, now time.Time) error {
	ms := now.UnixMilli()
	return s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		if _, err := tx.ExecContext(ctx, "INSERT INTO spaces (id, token_hash, created, touched, bytes) VALUES (?, ?, ?, ?, ?)", id, tokenHash, ms, ms, RowBytes); err != nil {
			if isConstraint(err) {
				return 0, 0, ErrExists
			}
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "INSERT INTO devices (space, device, slot, seen) VALUES (?, ?, ?, ?)", id, device, slot, ms); err != nil {
			return 0, 0, err
		}
		return RowBytes, 1, nil
	})
}

// Space reads a space.
func (s *Store) Space(ctx context.Context, id []byte) (Space, error) {
	if s.closed() {
		return Space{}, ErrClosed
	}
	return scanSpace(s.r.QueryRowContext(ctx, "SELECT "+spaceColumns+" FROM spaces WHERE id = ?", id))
}

func scanSpace(row *sql.Row) (Space, error) {
	var sp Space
	var created, touched int64
	err := row.Scan(&sp.ID, &sp.TokenHash, &created, &touched, &sp.Head, &sp.SnapUpto, &sp.TailBytes, &sp.Bytes, &sp.HasSnapshot)
	if errors.Is(err, sql.ErrNoRows) {
		return Space{}, ErrNotFound
	}
	if err != nil {
		return Space{}, err
	}
	sp.Created, sp.Touched = time.UnixMilli(created), time.UnixMilli(touched)
	return sp, nil
}

// Authenticate reads a space and checks its token in constant time: ErrUnauthorized for a token
// that is not its own, and for a space that does not exist (a caller learns nothing about which).
func (s *Store) Authenticate(ctx context.Context, id, token []byte) (Space, error) {
	sp, err := s.Space(ctx, id)
	if errors.Is(err, ErrNotFound) {
		subtle.ConstantTimeCompare(TokenHash(token), make([]byte, sha256.Size))
		return Space{}, ErrUnauthorized
	}
	if err != nil {
		return Space{}, err
	}
	if subtle.ConstantTimeCompare(TokenHash(token), sp.TokenHash) != 1 {
		return Space{}, ErrUnauthorized
	}
	return sp, nil
}

// DeleteSpace removes a space with everything it holds. Its parts are gone from now on: their
// tombstones outlive the space, and their ids come back for their watchers. false when there
// was no such space.
func (s *Store) DeleteSpace(ctx context.Context, id []byte, now time.Time) (bool, [][]byte, error) {
	found := false
	var parts [][]byte
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		var bytes int64
		err := tx.QueryRowContext(ctx, "SELECT bytes FROM spaces WHERE id = ?", id).Scan(&bytes)
		if errors.Is(err, sql.ErrNoRows) {
			return 0, 0, nil
		}
		if err != nil {
			return 0, 0, err
		}
		if parts, err = buryParts(ctx, tx, id, now); err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM spaces WHERE id = ?", id); err != nil {
			return 0, 0, err
		}
		found = true
		return -bytes + int64(len(parts))*RowBytes, -1, nil
	})
	if err != nil {
		return false, nil, err
	}
	return found, parts, nil
}

// Touch notes that a space is in use (hello, push, snapshot, parcel, part): it does not expire.
func (s *Store) Touch(ctx context.Context, id []byte, now time.Time) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		_, err := tx.ExecContext(ctx, "UPDATE spaces SET touched = MAX(touched, ?) WHERE id = ?", now.UnixMilli(), id)
		return 0, 0, err
	})
}

// Join gives a device its slot in a space: the one it has, else `want` where that is one of
// 1..maxSlots and free (the slot it had before Phloem lost the space: its writes stay numbered
// in it), else the smallest free one (ErrFull when none is). The space is touched.
func (s *Store) Join(ctx context.Context, id, device []byte, want int, now time.Time, maxSlots int) (int, error) {
	slot := 0
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		ms := now.UnixMilli()
		res, err := tx.ExecContext(ctx, "UPDATE spaces SET touched = MAX(touched, ?) WHERE id = ?", ms, id)
		if err != nil {
			return 0, 0, err
		}
		if n, _ := res.RowsAffected(); n == 0 {
			return 0, 0, ErrNotFound
		}
		err = tx.QueryRowContext(ctx, "SELECT slot FROM devices WHERE space = ? AND device = ?", id, device).Scan(&slot)
		if err == nil {
			_, err = tx.ExecContext(ctx, "UPDATE devices SET seen = ? WHERE space = ? AND device = ?", ms, id, device)
			return 0, 0, err
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return 0, 0, err
		}
		rows, err := tx.QueryContext(ctx, "SELECT slot FROM devices WHERE space = ? ORDER BY slot", id)
		if err != nil {
			return 0, 0, err
		}
		taken := map[int]bool{}
		for rows.Next() {
			var n int
			if err := rows.Scan(&n); err != nil {
				_ = rows.Close()
				return 0, 0, err
			}
			taken[n] = true
		}
		if err := errors.Join(rows.Err(), rows.Close()); err != nil {
			return 0, 0, err
		}
		if want >= 1 && want <= maxSlots && !taken[want] {
			slot = want
		}
		for n := 1; n <= maxSlots && slot == 0; n++ {
			if !taken[n] {
				slot = n
			}
		}
		if slot == 0 {
			return 0, 0, ErrFull
		}
		_, err = tx.ExecContext(ctx, "INSERT INTO devices (space, device, slot, seen) VALUES (?, ?, ?, ?)", id, device, slot, ms)
		return 0, 0, err
	})
	return slot, err
}

// Leave frees a device's slot.
func (s *Store) Leave(ctx context.Context, id, device []byte) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		_, err := tx.ExecContext(ctx, "DELETE FROM devices WHERE space = ? AND device = ?", id, device)
		return 0, 0, err
	})
}

// Appended is where a push went.
type Appended struct {
	Nr        int64
	TailBytes int64 // the data of the entries after the snapshot, with this one
	Again     bool  // the slot had pushed this id already: Nr is that entry, nothing was added
}

// Append keeps a sealed batch as the space's next entry, pid the device's own id of the push. A
// pid the slot pushed already among the entries after the snapshot is the same push sent again
// (its answer was lost): that entry is the answer, and nothing is added. ErrCompactFirst when
// the entries after the snapshot would hold more than hardTail bytes of data.
func (s *Store) Append(ctx context.Context, id []byte, slot int, pid string, data []byte, now time.Time, hardTail int64) (Appended, error) {
	var out Appended
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		var head, tail int64
		err := tx.QueryRowContext(ctx, "SELECT head, tail_bytes FROM spaces WHERE id = ?", id).Scan(&head, &tail)
		if errors.Is(err, sql.ErrNoRows) {
			return 0, 0, ErrNotFound
		}
		if err != nil {
			return 0, 0, err
		}
		var nr int64
		err = tx.QueryRowContext(ctx, "SELECT nr FROM entries WHERE space = ? AND slot = ? AND pid = ?", id, slot, pid).Scan(&nr)
		if err == nil {
			out = Appended{Nr: nr, TailBytes: tail, Again: true}
			return 0, 0, nil
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return 0, 0, err
		}
		size := int64(len(data))
		if tail+size > hardTail {
			return 0, 0, ErrCompactFirst
		}
		nr = head + 1
		if _, err := tx.ExecContext(ctx, "INSERT INTO entries (space, nr, slot, pid, data) VALUES (?, ?, ?, ?, ?)", id, nr, slot, pid, data); err != nil {
			return 0, 0, err
		}
		cost := size + int64(len(pid)) + RowBytes
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET head = ?, tail_bytes = tail_bytes + ?, bytes = bytes + ?, touched = MAX(touched, ?) WHERE id = ?", nr, size, cost, now.UnixMilli(), id); err != nil {
			return 0, 0, err
		}
		out = Appended{Nr: nr, TailBytes: tail + size}
		return cost, 0, nil
	})
	return out, err
}

// EntriesAfter reads the entries of a space with numbers above after, in order.
func (s *Store) EntriesAfter(ctx context.Context, id []byte, after int64) ([]Entry, error) {
	if s.closed() {
		return nil, ErrClosed
	}
	rows, err := s.r.QueryContext(ctx, "SELECT nr, slot, data FROM entries WHERE space = ? AND nr > ? ORDER BY nr", id, after)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Entry
	for rows.Next() {
		var e Entry
		if err := rows.Scan(&e.Nr, &e.Slot, &e.Data); err != nil {
			return nil, err
		}
		out = append(out, e)
	}
	return out, rows.Err()
}

// PutSnapshot keeps a space's snapshot as of entry upto and forgets the entries up to it. A
// snapshot that covers less than the kept one is ErrStale, one beyond the head ErrAhead.
func (s *Store) PutSnapshot(ctx context.Context, id []byte, upto int64, data []byte, now time.Time) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		var head, snapUpto int64
		err := tx.QueryRowContext(ctx, "SELECT head, snap_upto FROM spaces WHERE id = ?", id).Scan(&head, &snapUpto)
		if errors.Is(err, sql.ErrNoRows) {
			return 0, 0, ErrNotFound
		}
		if err != nil {
			return 0, 0, err
		}
		if upto < snapUpto {
			return 0, 0, ErrStale
		}
		if upto > head {
			return 0, 0, ErrAhead
		}
		var oldSnap int64
		err = tx.QueryRowContext(ctx, "SELECT LENGTH(data) FROM snapshots WHERE space = ?", id).Scan(&oldSnap)
		hadSnap := err == nil
		if err != nil && !errors.Is(err, sql.ErrNoRows) {
			return 0, 0, err
		}
		// goneData leaves the tail; goneBytes (the data with the push ids) and the rows leave
		// the budget.
		var goneData, goneBytes, goneRows int64
		if err := tx.QueryRowContext(ctx, "SELECT COALESCE(SUM(LENGTH(data)), 0), COALESCE(SUM(LENGTH(data) + LENGTH(CAST(pid AS BLOB))), 0), COUNT(*) FROM entries WHERE space = ? AND nr <= ?", id, upto).Scan(&goneData, &goneBytes, &goneRows); err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM entries WHERE space = ? AND nr <= ?", id, upto); err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "INSERT INTO snapshots (space, upto, data, at) VALUES (?, ?, ?, ?) ON CONFLICT (space) DO UPDATE SET upto = excluded.upto, data = excluded.data, at = excluded.at", id, upto, data, now.UnixMilli()); err != nil {
			return 0, 0, err
		}
		delta := int64(len(data)) - goneBytes - goneRows*RowBytes
		if hadSnap {
			delta -= oldSnap
		} else {
			delta += RowBytes
		}
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET snap_upto = ?, tail_bytes = tail_bytes - ?, bytes = bytes + ?, touched = MAX(touched, ?) WHERE id = ?", upto, goneData, delta, now.UnixMilli(), id); err != nil {
			return 0, 0, err
		}
		return delta, 0, nil
	})
}

// Snapshot reads a space's snapshot; ErrNotFound when it has none.
func (s *Store) Snapshot(ctx context.Context, id []byte) (Snapshot, error) {
	if s.closed() {
		return Snapshot{}, ErrClosed
	}
	var snap Snapshot
	var at int64
	err := s.r.QueryRowContext(ctx, "SELECT upto, data, at FROM snapshots WHERE space = ?", id).Scan(&snap.Upto, &snap.Data, &at)
	if errors.Is(err, sql.ErrNoRows) {
		return Snapshot{}, ErrNotFound
	}
	if err != nil {
		return Snapshot{}, err
	}
	snap.At = time.UnixMilli(at)
	return snap, nil
}

// PutPair keeps a parcel until expires. ErrExists for an id that is taken, or whose tombstone
// says what became of an earlier parcel of it; ErrNotFound for a space that is gone; ErrTooMany
// when the space has maxPairs parcels waiting that have not expired.
func (s *Store) PutPair(ctx context.Context, pairID, space, box []byte, now, expires time.Time, maxPairs int) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		res, err := tx.ExecContext(ctx, "UPDATE spaces SET touched = MAX(touched, ?) WHERE id = ?", now.UnixMilli(), space)
		if err != nil {
			return 0, 0, err
		}
		if n, _ := res.RowsAffected(); n == 0 {
			return 0, 0, ErrNotFound
		}
		if _, err := pairState(ctx, tx, pairID); err == nil {
			return 0, 0, ErrExists
		} else if !errors.Is(err, ErrNotFound) {
			return 0, 0, err
		}
		var waiting int
		if err := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM pairs WHERE space = ? AND expires > ?", space, now.UnixMilli()).Scan(&waiting); err != nil {
			return 0, 0, err
		}
		if waiting >= maxPairs {
			return 0, 0, ErrTooMany
		}
		if _, err := tx.ExecContext(ctx, "INSERT INTO pairs (id, space, box, expires) VALUES (?, ?, ?, ?)", pairID, space, box, expires.UnixMilli()); err != nil {
			if isConstraint(err) {
				return 0, 0, ErrExists
			}
			return 0, 0, err
		}
		cost := int64(len(box)) + RowBytes
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes + ? WHERE id = ?", cost, space); err != nil {
			return 0, 0, err
		}
		return cost, 0, nil
	})
}

// Took is what TakePair found: the parcel with its space when it waited (State ""), else what
// became of it.
type Took struct {
	Box     []byte
	Space   []byte // the parcel's space, whose devices hear that it was taken
	State   string // "": Box is the parcel, taken now; else PairTaken, PairExpired or PairWithdrawn
	Expired bool   // it expired before the sweep saw it, and became its tombstone by this call
}

// TakePair hands a parcel out once: in the same step it becomes a tombstone (taken). One that
// expired before the sweep saw it becomes its tombstone (expired) instead. ErrNotFound when
// neither a parcel nor a tombstone has the id. An id that finds no parcel costs a read, not the
// writer: guessing ids does not hold up the pushes. A space keeps the tombstones of keep
// parcels at most.
func (s *Store) TakePair(ctx context.Context, pairID []byte, now time.Time, keep int) (Took, error) {
	if s.closed() {
		return Took{}, ErrClosed
	}
	var one int
	err := s.r.QueryRowContext(ctx, "SELECT 1 FROM pairs WHERE id = ?", pairID).Scan(&one)
	if errors.Is(err, sql.ErrNoRows) {
		state, err := pairState(ctx, s.r, pairID)
		if err != nil {
			return Took{}, err
		}
		return Took{State: state}, nil
	}
	if err != nil {
		return Took{}, err
	}
	var out Took
	err = s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		var space, box []byte
		var expires int64
		err := tx.QueryRowContext(ctx, "SELECT space, box, expires FROM pairs WHERE id = ?", pairID).Scan(&space, &box, &expires)
		if errors.Is(err, sql.ErrNoRows) { // taken or withdrawn since the read
			state, err := pairState(ctx, tx, pairID)
			out = Took{State: state}
			return 0, 0, err
		}
		if err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM pairs WHERE id = ?", pairID); err != nil {
			return 0, 0, err
		}
		out = Took{Box: box, Space: space}
		state, at := PairTaken, now.UnixMilli()
		if expires <= now.UnixMilli() {
			out = Took{State: PairExpired, Expired: true}
			state, at = PairExpired, expires
		}
		delta, err := pairGone(ctx, tx, pairID, space, int64(len(box)), state, at, keep)
		return delta, 0, err
	})
	if err != nil {
		return Took{}, err
	}
	return out, nil
}

// WithdrawPair withdraws a parcel of a space that waits: a tombstone (withdrawn) from now on,
// and the space is touched. false when the id is not a parcel of the space that waits (nothing
// changes then; an id that finds none costs a read). A space keeps the tombstones of keep
// parcels at most.
func (s *Store) WithdrawPair(ctx context.Context, pairID, space []byte, now time.Time, keep int) (bool, error) {
	if s.closed() {
		return false, ErrClosed
	}
	const waits = "SELECT LENGTH(box) FROM pairs WHERE id = ? AND space = ? AND expires > ?"
	ms := now.UnixMilli()
	var size int64
	err := s.r.QueryRowContext(ctx, waits, pairID, space, ms).Scan(&size)
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	if err != nil {
		return false, err
	}
	found := false
	err = s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		err := tx.QueryRowContext(ctx, waits, pairID, space, ms).Scan(&size)
		if errors.Is(err, sql.ErrNoRows) { // taken since the read
			return 0, 0, nil
		}
		if err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM pairs WHERE id = ?", pairID); err != nil {
			return 0, 0, err
		}
		delta, err := pairGone(ctx, tx, pairID, space, size, PairWithdrawn, ms, keep)
		if err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET touched = MAX(touched, ?) WHERE id = ?", ms, space); err != nil {
			return 0, 0, err
		}
		found = true
		return delta, 0, nil
	})
	if err != nil {
		return false, err
	}
	return found, nil
}

// PairInfo is a parcel of a space as GET /sync/v1/pairs lists it: one that waits (At: when it
// expires), or what became of one (At: when it happened).
type PairInfo struct {
	ID    []byte
	State string
	At    time.Time
}

// Pairs lists the parcels of a space, the newest At first. One that expired before the sweep
// saw it is listed as expired.
func (s *Store) Pairs(ctx context.Context, space []byte, now time.Time) ([]PairInfo, error) {
	if s.closed() {
		return nil, ErrClosed
	}
	rows, err := s.r.QueryContext(ctx, `SELECT id, CASE WHEN expires > ? THEN ? ELSE ? END, expires FROM pairs WHERE space = ?
		UNION ALL SELECT id, state, at FROM pair_tombstones WHERE space = ?
		ORDER BY 3 DESC, 1`, now.UnixMilli(), PairWaiting, PairExpired, space, space)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []PairInfo
	for rows.Next() {
		var p PairInfo
		var at int64
		if err := rows.Scan(&p.ID, &p.State, &at); err != nil {
			return nil, err
		}
		p.At = time.UnixMilli(at)
		out = append(out, p)
	}
	return out, rows.Err()
}

// queryer is a reader of one row: the readers' pool, or a transaction.
type queryer interface {
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
}

// pairState reads what became of a parcel; ErrNotFound when no tombstone says.
func pairState(ctx context.Context, q queryer, pairID []byte) (string, error) {
	var state string
	err := q.QueryRowContext(ctx, "SELECT state FROM pair_tombstones WHERE id = ?", pairID).Scan(&state)
	if errors.Is(err, sql.ErrNoRows) {
		return "", ErrNotFound
	}
	return state, err
}

// pairGone makes a parcel that left (its box of size bytes, its row deleted) a tombstone of
// state as of at, and forgets the space's oldest tombstones beyond keep, so that what a space
// keeps of its parcels stays bounded however many it leaves. It returns the change of the
// space's bytes: the box leaves, its row stays as the tombstone's.
func pairGone(ctx context.Context, tx *sql.Tx, pairID, space []byte, size int64, state string, at int64, keep int) (int64, error) {
	if _, err := tx.ExecContext(ctx, "INSERT INTO pair_tombstones (id, space, state, at) VALUES (?, ?, ?, ?)", pairID, space, state, at); err != nil {
		return 0, err
	}
	res, err := tx.ExecContext(ctx, `DELETE FROM pair_tombstones WHERE id IN
		(SELECT id FROM pair_tombstones WHERE space = ? ORDER BY at DESC, id DESC LIMIT -1 OFFSET ?)`, space, keep)
	if err != nil {
		return 0, err
	}
	forgotten, err := res.RowsAffected()
	if err != nil {
		return 0, err
	}
	delta := -size - forgotten*RowBytes
	if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes + ? WHERE id = ?", delta, space); err != nil {
		return 0, err
	}
	return delta, nil
}

// Horizons are what a sweep forgets: the spaces not touched since Idle, the tombstones of
// parcels of before Pairs and those of parts of before Parts.
type Horizons struct {
	Idle, Pairs, Parts time.Time
}

// Swept is what a sweep removed.
type Swept struct {
	Spaces [][]byte // the spaces not touched since the horizon
	Parts  [][]byte // their parts, gone from now on
	Pairs  int      // the parcels that expired
}

// Sweep removes the spaces not touched since h.Idle (their parts are gone from now on), makes
// the parcels expired at now tombstones (expired, as of when they expired; a space keeps those
// of keep parcels at most), and forgets the tombstones of before h.Pairs and h.Parts.
func (s *Store) Sweep(ctx context.Context, now time.Time, h Horizons, keep int) (Swept, error) {
	var out Swept
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		out = Swept{}
		var dBytes, dSpaces int64
		rows, err := tx.QueryContext(ctx, "SELECT id, bytes FROM spaces WHERE touched < ?", h.Idle.UnixMilli())
		if err != nil {
			return 0, 0, err
		}
		for rows.Next() {
			var id []byte
			var bytes int64
			if err := rows.Scan(&id, &bytes); err != nil {
				_ = rows.Close()
				return 0, 0, err
			}
			out.Spaces = append(out.Spaces, id)
			dBytes -= bytes
			dSpaces--
		}
		if err := errors.Join(rows.Err(), rows.Close()); err != nil {
			return 0, 0, err
		}
		for _, id := range out.Spaces {
			parts, err := buryParts(ctx, tx, id, now)
			if err != nil {
				return 0, 0, err
			}
			out.Parts = append(out.Parts, parts...)
			dBytes += int64(len(parts)) * RowBytes
			if _, err := tx.ExecContext(ctx, "DELETE FROM spaces WHERE id = ?", id); err != nil {
				return 0, 0, err
			}
		}

		prow, err := tx.QueryContext(ctx, "SELECT id, space, LENGTH(box), expires FROM pairs WHERE expires <= ?", now.UnixMilli())
		if err != nil {
			return 0, 0, err
		}
		type expiredPair struct {
			id, space     []byte
			size, expires int64
		}
		var expired []expiredPair
		for prow.Next() {
			var p expiredPair
			if err := prow.Scan(&p.id, &p.space, &p.size, &p.expires); err != nil {
				_ = prow.Close()
				return 0, 0, err
			}
			expired = append(expired, p)
		}
		if err := errors.Join(prow.Err(), prow.Close()); err != nil {
			return 0, 0, err
		}
		for _, p := range expired {
			if _, err := tx.ExecContext(ctx, "DELETE FROM pairs WHERE id = ?", p.id); err != nil {
				return 0, 0, err
			}
			delta, err := pairGone(ctx, tx, p.id, p.space, p.size, PairExpired, p.expires, keep)
			if err != nil {
				return 0, 0, err
			}
			dBytes += delta
		}
		out.Pairs = len(expired)

		// The tombstones of parcels cost their spaces a row each; those of parts cost the
		// totals alone, since they outlive their spaces.
		trows, err := tx.QueryContext(ctx, "SELECT space, COUNT(*) FROM pair_tombstones WHERE at < ? GROUP BY space", h.Pairs.UnixMilli())
		if err != nil {
			return 0, 0, err
		}
		type forgotten struct {
			space []byte
			n     int64
		}
		var old []forgotten
		for trows.Next() {
			var f forgotten
			if err := trows.Scan(&f.space, &f.n); err != nil {
				_ = trows.Close()
				return 0, 0, err
			}
			old = append(old, f)
		}
		if err := errors.Join(trows.Err(), trows.Close()); err != nil {
			return 0, 0, err
		}
		for _, f := range old {
			if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes - ? WHERE id = ?", f.n*RowBytes, f.space); err != nil {
				return 0, 0, err
			}
			dBytes -= f.n * RowBytes
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM pair_tombstones WHERE at < ?", h.Pairs.UnixMilli()); err != nil {
			return 0, 0, err
		}
		res, err := tx.ExecContext(ctx, "DELETE FROM part_tombstones WHERE at < ?", h.Parts.UnixMilli())
		if err != nil {
			return 0, 0, err
		}
		n, err := res.RowsAffected()
		if err != nil {
			return 0, 0, err
		}
		dBytes -= n * RowBytes
		return dBytes, dSpaces, nil
	})
	return out, err
}

type migration struct {
	version int
	name    string
	sql     string
}

// loadMigrations reads migrations/NNNN_name.sql in version order; a gap or a duplicate number is
// an error, since the version is the only record of the index's shape.
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

// migrate applies the pending migrations, each in its own transaction, and refuses an index that
// a newer binary migrated further. Log events: db.migrated.
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

// isConstraint says whether err is SQLite's constraint violation (a duplicate id or key).
func isConstraint(err error) bool {
	var se *sqlite.Error
	return errors.As(err, &se) && se.Code()&0xff == 19 // SQLITE_CONSTRAINT
}
