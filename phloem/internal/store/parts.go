package store

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"strings"
	"time"
)

// Part is a part of a shared calendar as the store keeps it: sealed with a key Phloem never
// sees, read by anyone with its id, changed by its space alone.
type Part struct {
	Space []byte
	Rev   int64 // 1 when it was made, one more with every replacement
	Data  []byte
	At    time.Time // its last replacement
}

// PartState is what a watcher learns of a part: its version, or that it is gone. Rev 0 and not
// Gone: Phloem does not have it.
type PartState struct {
	Rev  int64
	Gone bool
}

// PutPart makes a part of a space (rev 1) or replaces it (one more), and touches the space. It
// returns the part's rev. ErrExists for an id that is another space's part, or a tombstone (a
// part never comes back, so that a link to it never shows another calendar); ErrTooMany when
// the part would be new and the space has maxParts; ErrNotFound for a space that is gone.
func (s *Store) PutPart(ctx context.Context, id, space, data []byte, now time.Time, maxParts int) (int64, error) {
	if data == nil {
		data = []byte{} // nil would be NULL
	}
	var rev int64
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		ms := now.UnixMilli()
		res, err := tx.ExecContext(ctx, "UPDATE spaces SET touched = MAX(touched, ?) WHERE id = ?", ms, space)
		if err != nil {
			return 0, 0, err
		}
		if n, _ := res.RowsAffected(); n == 0 {
			return 0, 0, ErrNotFound
		}
		var one int
		err = tx.QueryRowContext(ctx, "SELECT 1 FROM part_tombstones WHERE id = ?", id).Scan(&one)
		if err == nil {
			return 0, 0, ErrExists
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return 0, 0, err
		}
		var owner []byte
		var old int64
		err = tx.QueryRowContext(ctx, "SELECT space, rev, LENGTH(data) FROM parts WHERE id = ?", id).Scan(&owner, &rev, &old)
		switch {
		case err == nil:
			if !bytes.Equal(owner, space) {
				return 0, 0, ErrExists
			}
			rev++
			if _, err := tx.ExecContext(ctx, "UPDATE parts SET rev = ?, data = ?, at = ? WHERE id = ?", rev, data, ms, id); err != nil {
				return 0, 0, err
			}
			delta := int64(len(data)) - old
			if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes + ? WHERE id = ?", delta, space); err != nil {
				return 0, 0, err
			}
			return delta, 0, nil
		case !errors.Is(err, sql.ErrNoRows):
			return 0, 0, err
		}
		var n int
		if err := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM parts WHERE space = ?", space).Scan(&n); err != nil {
			return 0, 0, err
		}
		if n >= maxParts {
			return 0, 0, ErrTooMany
		}
		rev = 1
		if _, err := tx.ExecContext(ctx, "INSERT INTO parts (id, space, rev, data, at) VALUES (?, ?, 1, ?, ?)", id, space, data, ms); err != nil {
			return 0, 0, err
		}
		cost := int64(len(data)) + RowBytes
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes + ? WHERE id = ?", cost, space); err != nil {
			return 0, 0, err
		}
		return cost, 0, nil
	})
	if err != nil {
		return 0, err
	}
	return rev, nil
}

// DeletePart deletes a part of a space, which is gone from now on (its tombstone is of no
// space), and touches the space. false when the id is not a part of the space (nothing changes
// then).
func (s *Store) DeletePart(ctx context.Context, id, space []byte, now time.Time) (bool, error) {
	found := false
	err := s.write(ctx, func(tx *sql.Tx) (int64, int64, error) {
		var size int64
		err := tx.QueryRowContext(ctx, "SELECT LENGTH(data) FROM parts WHERE id = ? AND space = ?", id, space).Scan(&size)
		if errors.Is(err, sql.ErrNoRows) {
			return 0, 0, nil
		}
		if err != nil {
			return 0, 0, err
		}
		ms := now.UnixMilli()
		if _, err := tx.ExecContext(ctx, "DELETE FROM parts WHERE id = ?", id); err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "INSERT INTO part_tombstones (id, at) VALUES (?, ?)", id, ms); err != nil {
			return 0, 0, err
		}
		if _, err := tx.ExecContext(ctx, "UPDATE spaces SET bytes = bytes - ?, touched = MAX(touched, ?) WHERE id = ?", size+RowBytes, ms, space); err != nil {
			return 0, 0, err
		}
		found = true
		return -size, 0, nil // the row leaves the space and stays in the totals as the tombstone's
	})
	return found, err
}

// buryParts makes every part of a space a tombstone as of now, before the space goes (its parts
// go with it), and returns their ids. The caller counts a row for each in the totals.
func buryParts(ctx context.Context, tx *sql.Tx, space []byte, now time.Time) ([][]byte, error) {
	rows, err := tx.QueryContext(ctx, "SELECT id FROM parts WHERE space = ?", space)
	if err != nil {
		return nil, err
	}
	var ids [][]byte
	for rows.Next() {
		var id []byte
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return nil, err
		}
		ids = append(ids, id)
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return nil, err
	}
	if len(ids) > 0 {
		if _, err := tx.ExecContext(ctx, "INSERT INTO part_tombstones (id, at) SELECT id, ? FROM parts WHERE space = ?", now.UnixMilli(), space); err != nil {
			return nil, err
		}
	}
	return ids, nil
}

// Part reads a part. ErrGone for one that was deleted, or whose space was; ErrNotFound for an id
// Phloem does not know.
func (s *Store) Part(ctx context.Context, id []byte) (Part, error) {
	if s.closed() {
		return Part{}, ErrClosed
	}
	var p Part
	var at int64
	err := s.r.QueryRowContext(ctx, "SELECT space, rev, data, at FROM parts WHERE id = ?", id).Scan(&p.Space, &p.Rev, &p.Data, &at)
	if errors.Is(err, sql.ErrNoRows) {
		return Part{}, s.partMissing(ctx, id)
	}
	if err != nil {
		return Part{}, err
	}
	p.At = time.UnixMilli(at)
	return p, nil
}

// PartOwner reads whose a part is: its space. ErrGone for one that was deleted, or whose space
// was; ErrNotFound for an id Phloem does not know.
func (s *Store) PartOwner(ctx context.Context, id []byte) ([]byte, error) {
	if s.closed() {
		return nil, ErrClosed
	}
	var space []byte
	err := s.r.QueryRowContext(ctx, "SELECT space FROM parts WHERE id = ?", id).Scan(&space)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, s.partMissing(ctx, id)
	}
	if err != nil {
		return nil, err
	}
	return space, nil
}

// partMissing is the error of an id that is no part: ErrGone when its tombstone says it was
// one, else ErrNotFound. A part deleted after the caller's read is found here, never missed:
// its tombstone comes in the same step.
func (s *Store) partMissing(ctx context.Context, id []byte) error {
	var one int
	err := s.r.QueryRowContext(ctx, "SELECT 1 FROM part_tombstones WHERE id = ?", id).Scan(&one)
	switch {
	case err == nil:
		return ErrGone
	case errors.Is(err, sql.ErrNoRows):
		return ErrNotFound
	}
	return err
}

// PartStates reads the state of the parts of ids, in their order, in one statement: as of one
// moment.
func (s *Store) PartStates(ctx context.Context, ids [][]byte) ([]PartState, error) {
	if s.closed() {
		return nil, ErrClosed
	}
	out := make([]PartState, len(ids))
	if len(ids) == 0 {
		return out, nil
	}
	where := map[string][]int{}
	args := make([]any, 0, 2*len(ids))
	for i, id := range ids {
		where[string(id)] = append(where[string(id)], i)
		args = append(args, id)
	}
	args = append(args, args...)
	in := strings.TrimSuffix(strings.Repeat("?, ", len(ids)), ", ")
	rows, err := s.r.QueryContext(ctx, "SELECT id, rev, 0 FROM parts WHERE id IN ("+in+") UNION ALL SELECT id, 0, 1 FROM part_tombstones WHERE id IN ("+in+")", args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	for rows.Next() {
		var id []byte
		var st PartState
		if err := rows.Scan(&id, &st.Rev, &st.Gone); err != nil {
			return nil, err
		}
		for _, i := range where[string(id)] {
			out[i] = st
		}
	}
	return out, rows.Err()
}

// Parts counts the parts of every space.
func (s *Store) Parts(ctx context.Context) (int64, error) {
	var n int64
	err := s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM parts").Scan(&n)
	return n, err
}
