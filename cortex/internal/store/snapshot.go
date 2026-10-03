package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
)

// Snapshot writes a consistent copy of the index to w, an SQLite file, and returns the journal
// position it holds (OpenSnapshot, streamed and removed).
func (s *Store) Snapshot(w io.Writer) (seq, epoch int64, err error) {
	f, err := s.OpenSnapshot(context.Background())
	if err != nil {
		return 0, 0, err
	}
	defer f.Close()
	if _, err := io.Copy(w, f); err != nil {
		return 0, 0, fmt.Errorf("failed to send the snapshot: %w", err)
	}
	return f.Seq, f.Epoch, nil
}

// SnapshotFile is a copy of the index in DIR/tmp, open for reading from its start, with the
// journal position it holds. Close removes it.
type SnapshotFile struct {
	*os.File
	Seq, Epoch int64
	Size       int64
	release    func()
}

// Close closes and removes the copy and lets the next OpenSnapshot make one.
func (f *SnapshotFile) Close() error {
	err := f.File.Close()
	removeDB(f.Name())
	if f.release != nil {
		f.release()
		f.release = nil
	}
	return err
}

// OpenSnapshot makes a consistent copy of the index (VACUUM INTO a file in DIR/tmp) and
// returns it open, to be sent as it is: one copy on disk per snapshot, not a second one to
// learn the position before the first is sent (review 2: snapshot-unbounded-uncancelled).
// Writes go on meanwhile; the copy is the index as it was when the VACUUM began.
//
// One copy exists at a time: a second call waits until the first one's file is closed, or
// ctx ends. ctx also interrupts the VACUUM, for a follower that went away while it ran.
func (s *Store) OpenSnapshot(ctx context.Context) (*SnapshotFile, error) {
	select {
	case s.snapshotSlot <- struct{}{}:
	case <-ctx.Done():
		return nil, ctx.Err()
	}
	release := func() { <-s.snapshotSlot }
	f, err := s.makeSnapshot(ctx)
	if err != nil {
		release()
		return nil, err
	}
	f.release = release
	return f, nil
}

func (s *Store) makeSnapshot(ctx context.Context) (*SnapshotFile, error) {
	f, err := os.CreateTemp(s.tmpDir(), "snapshot-*.db")
	if err != nil {
		return nil, fmt.Errorf("failed to create a snapshot file: %w", err)
	}
	path := f.Name()
	_ = f.Close() // VACUUM INTO writes into an empty file
	done := false
	defer func() {
		if !done {
			removeDB(path)
		}
	}()
	if err := s.vacuumInto(ctx, path); err != nil {
		return nil, err
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	seq, epoch, err := snapshotPosition(path)
	if err != nil {
		return nil, err
	}
	in, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("failed to read the snapshot: %w", err)
	}
	info, err := in.Stat()
	if err != nil {
		_ = in.Close()
		return nil, fmt.Errorf("failed to read the snapshot: %w", err)
	}
	done = true
	return &SnapshotFile{File: in, Seq: seq, Epoch: epoch, Size: info.Size()}, nil
}

// vacuumInto copies the index into path. VACUUM INTO counts as a write, so it cannot run on a
// query_only reader; a connection of its own reads the index without taking the writer.
func (s *Store) vacuumInto(ctx context.Context, path string) error {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return err
	}
	db, err := sql.Open("sqlite", dsn(s.indexPath()))
	if err != nil {
		return fmt.Errorf("failed to open the index for a snapshot: %w", err)
	}
	defer db.Close()
	db.SetMaxOpenConns(1)
	if _, err := db.ExecContext(ctx, `VACUUM INTO ?`, path); err != nil {
		if ctx.Err() != nil {
			return fmt.Errorf("copying the index: %w", ctx.Err())
		}
		return fmt.Errorf("failed to copy the index: %w", err)
	}
	return nil
}

// snapshotPosition reads the newest journal entry of a copied index.
func snapshotPosition(path string) (seq, epoch int64, err error) {
	db, err := sql.Open("sqlite", path+"?_pragma=query_only(1)")
	if err != nil {
		return 0, 0, fmt.Errorf("failed to open the snapshot: %w", err)
	}
	defer db.Close()
	err = db.QueryRow(`SELECT seq, epoch FROM journal ORDER BY seq DESC LIMIT 1`).Scan(&seq, &epoch)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, 0, nil
	}
	if err != nil {
		return 0, 0, fmt.Errorf("failed to read the snapshot's position: %w", err)
	}
	return seq, epoch, nil
}

// ReplaceIndex replaces the index with a copy another instance made (Snapshot): it writes r to
// DIR/tmp, checks that it opens, is intact and has this binary's schema (or an older one, which
// is migrated), then closes the current index, renames the copy into place and opens it. It
// holds the index for writing only for the swap, so readers wait for it instead of seeing a
// closed handle. When the copy is refused the current index stays as it was. Log events:
// db.migrated (an older copy), index.replaced.
func (s *Store) ReplaceIndex(r io.Reader) error {
	t, err := s.createTemp("index-*.db")
	if err != nil {
		return err
	}
	defer func() {
		t.discard()
		if !t.placed {
			removeDB(t.f.Name())
		}
	}()
	if _, err := io.Copy(t, r); err != nil {
		return fmt.Errorf("failed to receive the index: %w", err)
	}
	if err := t.finish(); err != nil {
		return fmt.Errorf("failed to write the index: %w", err)
	}
	if err := verifyIndex(t.f.Name()); err != nil {
		return err
	}

	if err := s.swapIndex(t); err != nil {
		return err
	}
	s.stats.invalidate()
	seq, epoch := s.Position()
	oplog.For("store").Info("index replaced", "event", "index.replaced", "seq", seq, "epoch", epoch)
	return nil
}

// swapIndex closes the current index, renames t into its place and opens it. When the rename
// fails, the old index is opened again.
func (s *Store) swapIndex(t *tempFile) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return ErrClosed
	}
	if err := s.closeIndex(); err != nil {
		// The handles are closed all the same; what the old index had is replaced anyway.
		oplog.For("store").Warn("closing the old index failed", "event", "index.close_failed", oplog.Err(err))
	}
	path := s.indexPath()
	// A WAL left beside the database would be replayed into the new one: away with it before
	// the rename, not after.
	for _, side := range []string{path + "-wal", path + "-shm"} {
		if err := os.Remove(side); err != nil && !errors.Is(err, fs.ErrNotExist) {
			return errors.Join(fmt.Errorf("failed to remove %s: %w", side, err), s.openIndex())
		}
	}
	if err := os.Rename(t.f.Name(), path); err != nil {
		return errors.Join(fmt.Errorf("failed to replace the index: %w", err), s.openIndex())
	}
	t.placed = true
	if err := syncDir(s.dir); err != nil {
		// The new index is in place; after a crash the old one may be back, which is complete
		// (checkpointed above), only older.
		oplog.For("store").Warn("syncing the data directory failed", "event", "index.sync_failed", oplog.Err(err))
	}
	if err := s.openIndex(); err != nil {
		return fmt.Errorf("failed to open the new index: %w", err)
	}
	return nil
}

// verifyIndex checks a received index before it replaces the current one.
func verifyIndex(path string) error {
	db, err := sql.Open("sqlite", path+"?_pragma=query_only(1)")
	if err != nil {
		return fmt.Errorf("received index: %w", err)
	}
	defer db.Close()
	var check string
	if err := db.QueryRow(`PRAGMA quick_check`).Scan(&check); err != nil {
		return fmt.Errorf("received index does not open: %w", err)
	}
	if check != "ok" {
		return fmt.Errorf("received index is damaged: %s", check)
	}
	migrations, err := loadMigrations()
	if err != nil {
		return err
	}
	var version int
	if err := db.QueryRow(`PRAGMA user_version`).Scan(&version); err != nil {
		return fmt.Errorf("received index: %w", err)
	}
	if version < 1 || version > len(migrations) {
		return fmt.Errorf("received index has schema version %d, this binary knows 1 to %d", version, len(migrations))
	}
	var tables int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
		AND name IN ('meta', 'entry', 'version', 'file', 'file_version', 'journal')`).Scan(&tables); err != nil {
		return fmt.Errorf("received index: %w", err)
	}
	if tables != 6 {
		return fmt.Errorf("received index lacks tables of the schema (%d of 6)", tables)
	}
	return nil
}

// removeDB removes an SQLite file and the files SQLite keeps beside it.
func removeDB(path string) {
	for _, p := range []string{path, path + "-wal", path + "-shm", path + "-journal"} {
		_ = os.Remove(p)
	}
}
