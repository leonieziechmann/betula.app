package store

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"path/filepath"
	"testing"
	"time"
)

var ctx = context.Background()

var t0 = time.Date(2026, 11, 5, 12, 0, 0, 0, time.UTC)

func open(t *testing.T) *Store {
	t.Helper()
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = s.Close() })
	return s
}

func id(n byte, size int) []byte { return bytes.Repeat([]byte{n}, size) }

// space makes space n with token n and device n in slot 1.
func space(t *testing.T, s *Store, n byte) []byte {
	t.Helper()
	sp := id(n, 16)
	if err := s.CreateSpace(ctx, sp, TokenHash(id(n, 32)), id(n, 16), t0); err != nil {
		t.Fatal(err)
	}
	return sp
}

// checkBytes recomputes every space's bytes from its rows (a tombstone of a parcel is a row), and
// the totals from the spaces and the tombstones of parts: the cached numbers must be exact.
func checkBytes(t *testing.T, s *Store) {
	t.Helper()
	// The writer is one connection: this count comes before the rows below hold it.
	var buried int64
	if err := s.w.QueryRow("SELECT COUNT(*) FROM part_tombstones").Scan(&buried); err != nil {
		t.Fatal(err)
	}
	rows, err := s.w.Query(`SELECT id, bytes, tail_bytes,
		? + COALESCE((SELECT SUM(LENGTH(data) + LENGTH(CAST(pid AS BLOB)) + ?) FROM entries WHERE space = spaces.id), 0)
		  + COALESCE((SELECT LENGTH(data) + ? FROM snapshots WHERE space = spaces.id), 0)
		  + COALESCE((SELECT SUM(LENGTH(box) + ?) FROM pairs WHERE space = spaces.id), 0)
		  + COALESCE((SELECT SUM(LENGTH(data) + ?) FROM parts WHERE space = spaces.id), 0)
		  + (SELECT COUNT(*) * ? FROM pair_tombstones WHERE space = spaces.id),
		COALESCE((SELECT SUM(LENGTH(data)) FROM entries WHERE space = spaces.id), 0)
		FROM spaces`, RowBytes, RowBytes, RowBytes, RowBytes, RowBytes, RowBytes)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	var count int64
	total := buried * RowBytes
	for rows.Next() {
		var sp []byte
		var bytes, tail, want, wantTail int64
		if err := rows.Scan(&sp, &bytes, &tail, &want, &wantTail); err != nil {
			t.Fatal(err)
		}
		if bytes != want || tail != wantTail {
			t.Errorf("space %x: bytes %d tail %d, the rows hold %d and %d", sp[:2], bytes, tail, want, wantTail)
		}
		count++
		total += bytes
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	if n, b := s.Totals(); n != count || b != total {
		t.Errorf("totals %d spaces %d bytes, the table holds %d and %d", n, b, count, total)
	}
}

func TestOpenMigratesAndReopens(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	var v int
	if err := s.w.QueryRow("PRAGMA user_version").Scan(&v); err != nil || v != 2 {
		t.Fatalf("user_version %d, %v", v, err)
	}
	var mode string
	if err := s.w.QueryRow("PRAGMA journal_mode").Scan(&mode); err != nil || mode != "wal" {
		t.Fatalf("journal_mode %q, %v", mode, err)
	}
	if err := s.CreateSpace(ctx, id(1, 16), TokenHash(id(1, 32)), id(1, 16), t0); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Append(ctx, id(1, 16), 1, "a", []byte("data"), t0, 1<<20); err != nil {
		t.Fatal(err)
	}
	if err := s.Close(); err != nil {
		t.Fatal(err)
	}
	if err := s.Ping(ctx); !errors.Is(err, ErrClosed) {
		t.Errorf("Ping after Close: %v", err)
	}
	s, err = Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	if n, b := s.Totals(); n != 1 || b != 2*RowBytes+4+1 {
		t.Errorf("reopened totals %d %d", n, b)
	}
	if err := s.Ping(ctx); err != nil {
		t.Fatal(err)
	}
	checkBytes(t, s)
}

func TestOpenRefusesANewerIndex(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.w.Exec("PRAGMA user_version = 99"); err != nil {
		t.Fatal(err)
	}
	_ = s.Close()
	if _, err := Open(dir); err == nil {
		t.Fatal("an index of a newer binary opened")
	}
}

func TestCreateAuthenticateDelete(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	if err := s.CreateSpace(ctx, sp, TokenHash(id(9, 32)), id(9, 16), t0); !errors.Is(err, ErrExists) {
		t.Fatalf("a second space of the same id: %v", err)
	}
	got, err := s.Authenticate(ctx, sp, id(1, 32))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got.ID, sp) || got.Head != 0 || got.HasSnapshot || !got.Created.Equal(t0) || got.Bytes != RowBytes {
		t.Errorf("space %+v", got)
	}
	if _, err := s.Authenticate(ctx, sp, id(2, 32)); !errors.Is(err, ErrUnauthorized) {
		t.Errorf("a wrong token: %v", err)
	}
	if _, err := s.Authenticate(ctx, id(7, 16), id(7, 32)); !errors.Is(err, ErrUnauthorized) {
		t.Errorf("a missing space must look like a wrong token: %v", err)
	}
	if n, err := s.Devices(ctx); err != nil || n != 1 {
		t.Errorf("devices %d, %v", n, err)
	}
	if _, err := s.Append(ctx, sp, 1, "x", []byte("abc"), t0, 1<<20); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(5, 16), sp, []byte("box"), t0, t0.Add(time.Minute), 16); err != nil {
		t.Fatal(err)
	}
	if err := s.PutSnapshot(ctx, sp, 0, []byte("snap"), t0); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(6, 16), sp, []byte("box"), t0, t0.Add(time.Minute), 16); err != nil {
		t.Fatal(err)
	}
	if _, err := s.WithdrawPair(ctx, id(6, 16), sp, t0, 256); err != nil {
		t.Fatal(err)
	}
	checkBytes(t, s)
	found, parts, err := s.DeleteSpace(ctx, sp, t0)
	if err != nil || !found || len(parts) != 0 {
		t.Fatalf("delete: %v %v %v", found, parts, err)
	}
	if found, _, err := s.DeleteSpace(ctx, sp, t0); err != nil || found {
		t.Errorf("deleting twice: %v %v", found, err)
	}
	for _, table := range []string{"devices", "entries", "snapshots", "pairs", "pair_tombstones"} {
		var n int
		if err := s.w.QueryRow("SELECT COUNT(*) FROM " + table).Scan(&n); err != nil || n != 0 {
			t.Errorf("%s keeps %d rows of a deleted space (%v)", table, n, err)
		}
	}
	if n, b := s.Totals(); n != 0 || b != 0 {
		t.Errorf("totals after the delete: %d %d", n, b)
	}
}

func TestJoinSlots(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	slot := func(dev byte) int {
		t.Helper()
		n, err := s.Join(ctx, sp, id(dev, 16), t0, 3)
		if err != nil {
			t.Fatalf("join %d: %v", dev, err)
		}
		return n
	}
	if got := []int{slot(1), slot(2), slot(3), slot(2)}; fmt.Sprint(got) != "[1 2 3 2]" {
		t.Fatalf("slots %v: the creator has 1, each device keeps its own", got)
	}
	if _, err := s.Join(ctx, sp, id(4, 16), t0, 3); !errors.Is(err, ErrFull) {
		t.Fatalf("a fourth device of three slots: %v", err)
	}
	if err := s.Leave(ctx, sp, id(2, 16)); err != nil {
		t.Fatal(err)
	}
	if got := slot(4); got != 2 {
		t.Errorf("the freed slot goes to the next device: %d", got)
	}
	if _, err := s.Join(ctx, id(9, 16), id(1, 16), t0, 3); !errors.Is(err, ErrNotFound) {
		t.Errorf("join of a missing space: %v", err)
	}
	later := t0.Add(time.Hour)
	if _, err := s.Join(ctx, sp, id(1, 16), later, 3); err != nil {
		t.Fatal(err)
	}
	if got, _ := s.Space(ctx, sp); !got.Touched.Equal(later) {
		t.Errorf("a hello touches the space: %v", got.Touched)
	}
}

func TestAppendNumbersTailsAndRepeats(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	for i, want := range []int64{1, 2, 3} {
		a, err := s.Append(ctx, sp, 1, fmt.Sprint("p", i), []byte("0123456789"), t0, 100)
		if err != nil {
			t.Fatal(err)
		}
		if a.Nr != want || a.TailBytes != 10*want || a.Again {
			t.Fatalf("append %d: %+v", i, a)
		}
	}
	// The same push again, after its answer was lost: its entry, nothing new.
	a, err := s.Append(ctx, sp, 1, "p1", []byte("0123456789"), t0, 100)
	if err != nil || a.Nr != 2 || !a.Again || a.TailBytes != 30 {
		t.Fatalf("a push sent again: %+v %v", a, err)
	}
	// Another slot may use the same id.
	if a, err := s.Append(ctx, sp, 2, "p1", []byte("x"), t0, 100); err != nil || a.Nr != 4 || a.Again {
		t.Fatalf("the same id from another slot: %+v %v", a, err)
	}
	if _, err := s.Append(ctx, sp, 1, "big", make([]byte, 70), t0, 100); !errors.Is(err, ErrCompactFirst) {
		t.Fatalf("beyond the hard tail: %v", err)
	}
	// A push sent again is answered even beyond the hard tail: it adds nothing.
	if a, err := s.Append(ctx, sp, 1, "p0", []byte("0123456789"), t0, 0); err != nil || a.Nr != 1 {
		t.Fatalf("a repeat beyond the tail: %+v %v", a, err)
	}
	entries, err := s.EntriesAfter(ctx, sp, 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 3 || entries[0].Nr != 2 || entries[2].Nr != 4 || entries[2].Slot != 2 || string(entries[2].Data) != "x" {
		t.Errorf("entries after 1: %+v", entries)
	}
	if _, err := s.Append(ctx, id(9, 16), 1, "a", nil, t0, 100); !errors.Is(err, ErrNotFound) {
		t.Errorf("append to a missing space: %v", err)
	}
	checkBytes(t, s)
}

func TestSnapshots(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	// The first device's snapshot, as of no entry at all.
	if err := s.PutSnapshot(ctx, sp, 0, []byte("first"), t0); err != nil {
		t.Fatal(err)
	}
	got, _ := s.Space(ctx, sp)
	if !got.HasSnapshot || got.SnapUpto != 0 {
		t.Fatalf("snapshot as of 0: %+v", got)
	}
	for i := range 4 {
		if _, err := s.Append(ctx, sp, 1, fmt.Sprint(i), []byte("entry"), t0, 1<<20); err != nil {
			t.Fatal(err)
		}
	}
	if err := s.PutSnapshot(ctx, sp, 5, []byte("x"), t0); !errors.Is(err, ErrAhead) {
		t.Fatalf("a snapshot beyond the head: %v", err)
	}
	if err := s.PutSnapshot(ctx, sp, 3, []byte("as of three"), t0.Add(time.Minute)); err != nil {
		t.Fatal(err)
	}
	got, _ = s.Space(ctx, sp)
	if got.SnapUpto != 3 || got.Head != 4 || got.TailBytes != 5 {
		t.Errorf("after the snapshot: %+v", got)
	}
	snap, err := s.Snapshot(ctx, sp)
	if err != nil || snap.Upto != 3 || string(snap.Data) != "as of three" || !snap.At.Equal(t0.Add(time.Minute)) {
		t.Errorf("snapshot %+v %v", snap, err)
	}
	if entries, _ := s.EntriesAfter(ctx, sp, 0); len(entries) != 1 || entries[0].Nr != 4 {
		t.Errorf("the entries up to the snapshot must go: %+v", entries)
	}
	if err := s.PutSnapshot(ctx, sp, 2, []byte("old"), t0); !errors.Is(err, ErrStale) {
		t.Errorf("an older snapshot: %v", err)
	}
	if err := s.PutSnapshot(ctx, sp, 3, []byte("again as of three"), t0); err != nil {
		t.Errorf("a snapshot as of the same entry replaces it: %v", err)
	}
	// An id compacted away is a new push when it comes again.
	if a, err := s.Append(ctx, sp, 1, "0", []byte("entry"), t0, 1<<20); err != nil || a.Again || a.Nr != 5 {
		t.Errorf("an id of an entry the snapshot took: %+v %v", a, err)
	}
	if _, err := s.Snapshot(ctx, id(9, 16)); !errors.Is(err, ErrNotFound) {
		t.Errorf("snapshot of a missing space: %v", err)
	}
	checkBytes(t, s)
}

func TestPairs(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	before, _ := s.Space(ctx, sp)
	later := t0.Add(time.Minute)
	if err := s.PutPair(ctx, id(5, 16), sp, []byte("sealed"), later, later.Add(10*time.Minute), 2); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(5, 16), sp, []byte("other"), later, later.Add(10*time.Minute), 2); !errors.Is(err, ErrExists) {
		t.Fatalf("a parcel id taken: %v", err)
	}
	if err := s.PutPair(ctx, id(6, 16), sp, []byte("two"), later, later.Add(10*time.Minute), 2); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(7, 16), sp, []byte("three"), later, later.Add(10*time.Minute), 2); !errors.Is(err, ErrTooMany) {
		t.Fatalf("a third parcel of two: %v", err)
	}
	if got, _ := s.Space(ctx, sp); !got.Touched.Equal(later) {
		t.Errorf("a parcel touches the space: %v", got.Touched)
	}
	checkBytes(t, s)
	took, err := s.TakePair(ctx, id(5, 16), later, 256)
	if err != nil || took.State != "" || string(took.Box) != "sealed" || !bytes.Equal(took.Space, sp) {
		t.Fatalf("take: %+v %v", took, err)
	}
	if took, err := s.TakePair(ctx, id(5, 16), later, 256); err != nil || took.State != PairTaken || took.Box != nil {
		t.Fatalf("a parcel is taken once, then says so: %+v %v", took, err)
	}
	// One that expired before the sweep saw it becomes its tombstone now, as of when it expired.
	if took, err := s.TakePair(ctx, id(6, 16), later.Add(10*time.Minute), 256); err != nil || took.State != PairExpired || !took.Expired || took.Box != nil {
		t.Fatalf("an expired parcel: %+v %v", took, err)
	}
	if took, err := s.TakePair(ctx, id(6, 16), later.Add(time.Hour), 256); err != nil || took.State != PairExpired || took.Expired {
		t.Fatalf("an expired parcel asked again: %+v %v", took, err)
	}
	if _, err := s.TakePair(ctx, id(9, 16), later, 256); !errors.Is(err, ErrNotFound) {
		t.Fatalf("an id that never was: %v", err)
	}
	// The boxes left; their rows stay as the tombstones'.
	if after, _ := s.Space(ctx, sp); after.Bytes != before.Bytes+2*RowBytes {
		t.Errorf("bytes %d after the parcels went, %d before", after.Bytes, before.Bytes)
	}
	// The id of a tombstone is not taken again: its parcel says what became of it.
	for _, pid := range [][]byte{id(5, 16), id(6, 16)} {
		if err := s.PutPair(ctx, pid, sp, []byte("again"), later, later.Add(time.Hour), 2); !errors.Is(err, ErrExists) {
			t.Errorf("a parcel of a tombstone's id: %v", err)
		}
	}
	if err := s.PutPair(ctx, id(8, 16), id(9, 16), []byte("x"), later, later, 2); !errors.Is(err, ErrNotFound) {
		t.Errorf("a parcel of a missing space: %v", err)
	}
	checkBytes(t, s)
}

func TestWithdrawingAndListingParcels(t *testing.T) {
	s := open(t)
	sp, other := space(t, s, 1), space(t, s, 2)
	put := func(n byte, owner []byte, expires time.Time) {
		t.Helper()
		if err := s.PutPair(ctx, id(n, 16), owner, []byte("box"), t0, expires, 32); err != nil {
			t.Fatal(err)
		}
	}
	put(5, sp, t0.Add(time.Hour))
	put(6, sp, t0.Add(2*time.Hour))
	put(7, sp, t0.Add(time.Minute))
	put(8, other, t0.Add(time.Hour))
	later := t0.Add(10 * time.Minute)
	withdraw := func(n byte, owner []byte) bool {
		t.Helper()
		found, err := s.WithdrawPair(ctx, id(n, 16), owner, later, 256)
		if err != nil {
			t.Fatal(err)
		}
		return found
	}
	if withdraw(8, sp) || withdraw(9, sp) || withdraw(7, sp) {
		t.Fatal("withdrawn: another space's parcel, an id that never was, or one that waits no longer")
	}
	if !withdraw(5, sp) {
		t.Fatal("the space's parcel that waits was not withdrawn")
	}
	if withdraw(5, sp) {
		t.Error("withdrawn twice")
	}
	if took, err := s.TakePair(ctx, id(5, 16), later, 256); err != nil || took.State != PairWithdrawn {
		t.Errorf("a withdrawn parcel: %+v %v", took, err)
	}
	if got, _ := s.Space(ctx, sp); !got.Touched.Equal(later) {
		t.Errorf("withdrawing touches the space: %v", got.Touched)
	}
	if took, err := s.TakePair(ctx, id(8, 16), later, 256); err != nil || string(took.Box) != "box" {
		t.Errorf("the other space's parcel: %+v %v", took, err)
	}
	list, err := s.Pairs(ctx, sp, later)
	if err != nil {
		t.Fatal(err)
	}
	var got []string
	for _, p := range list {
		got = append(got, fmt.Sprintf("%d:%s@%s", p.ID[0], p.State, p.At.Sub(t0)))
	}
	// Newest first: a waiting one at its expiry, the rest when it happened; the one expired that
	// the sweep has not seen yet is expired.
	if want := "[6:waiting@2h0m0s 5:withdrawn@10m0s 7:expired@1m0s]"; fmt.Sprint(got) != want {
		t.Errorf("listing %v, want %s", got, want)
	}
	if list, err := s.Pairs(ctx, id(9, 16), later); err != nil || len(list) != 0 {
		t.Errorf("listing of a space without parcels: %v %v", list, err)
	}
	checkBytes(t, s)
}

// However many parcels a space leaves, it keeps the tombstones of keep of them: the oldest go
// first.
func TestASpaceKeepsTheTombstonesOfItsNewestParcels(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	for n := byte(1); n <= 6; n++ {
		at := t0.Add(time.Duration(n) * time.Minute)
		if err := s.PutPair(ctx, id(n, 16), sp, []byte("box"), at, at.Add(time.Hour), 32); err != nil {
			t.Fatal(err)
		}
		if found, err := s.WithdrawPair(ctx, id(n, 16), sp, at, 3); err != nil || !found {
			t.Fatalf("withdraw %d: %v %v", n, found, err)
		}
	}
	for n := byte(1); n <= 6; n++ {
		took, err := s.TakePair(ctx, id(n, 16), t0.Add(time.Hour), 3)
		switch {
		case n <= 3 && !errors.Is(err, ErrNotFound):
			t.Errorf("parcel %d is forgotten: %+v %v", n, took, err)
		case n > 3 && (err != nil || took.State != PairWithdrawn):
			t.Errorf("parcel %d is kept: %+v %v", n, took, err)
		}
	}
	if got, _ := s.Space(ctx, sp); got.Bytes != RowBytes+3*RowBytes {
		t.Errorf("bytes %d: the space and three tombstones", got.Bytes)
	}
	checkBytes(t, s)
}

func TestSweep(t *testing.T) {
	s := open(t)
	old := space(t, s, 1)
	kept := space(t, s, 2)
	if _, err := s.Append(ctx, old, 1, "a", []byte("data"), t0, 1<<20); err != nil {
		t.Fatal(err)
	}
	for _, n := range []byte{3, 4} {
		if _, err := s.PutPart(ctx, id(n, 16), old, []byte("part"), t0, 32); err != nil {
			t.Fatal(err)
		}
	}
	day := 24 * time.Hour
	if err := s.Touch(ctx, kept, t0.Add(300*day)); err != nil {
		t.Fatal(err)
	}
	// A part of the kept space deleted long ago, one lately; parcels that expire, wait, and
	// one taken long ago.
	for _, n := range []byte{7, 8} {
		if _, err := s.PutPart(ctx, id(n, 16), kept, []byte("part"), t0, 32); err != nil {
			t.Fatal(err)
		}
	}
	if found, err := s.DeletePart(ctx, id(7, 16), kept, t0); err != nil || !found {
		t.Fatal(found, err)
	}
	if found, err := s.DeletePart(ctx, id(8, 16), kept, t0.Add(200*day)); err != nil || !found {
		t.Fatal(found, err)
	}
	if err := s.PutPair(ctx, id(5, 16), kept, []byte("box"), t0.Add(300*day), t0.Add(390*day), 16); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(6, 16), kept, []byte("box"), t0.Add(300*day), t0.Add(500*day), 16); err != nil {
		t.Fatal(err)
	}
	if err := s.PutPair(ctx, id(9, 16), kept, []byte("box"), t0.Add(300*day), t0.Add(500*day), 16); err != nil {
		t.Fatal(err)
	}
	if took, err := s.TakePair(ctx, id(9, 16), t0.Add(300*day), 256); err != nil || took.State != "" {
		t.Fatal(took, err)
	}
	checkBytes(t, s)
	now := t0.Add(396 * day)
	horizons := Horizons{Idle: now.Add(-395 * day), Pairs: now.Add(-30 * day), Parts: now.Add(-395 * day)}
	swept, err := s.Sweep(ctx, now, horizons, 256)
	if err != nil {
		t.Fatal(err)
	}
	if len(swept.Spaces) != 1 || !bytes.Equal(swept.Spaces[0], old) || swept.Pairs != 1 || len(swept.Parts) != 2 {
		t.Fatalf("swept %+v", swept)
	}
	if _, err := s.Space(ctx, old); !errors.Is(err, ErrNotFound) {
		t.Errorf("the untouched space: %v", err)
	}
	if _, err := s.Space(ctx, kept); err != nil {
		t.Errorf("the touched space: %v", err)
	}
	// The parts of the expired space are gone from now on; the part deleted 396 days ago is
	// forgotten, the one of 196 days ago not.
	for n, want := range map[byte]error{3: ErrGone, 4: ErrGone, 7: ErrNotFound, 8: ErrGone} {
		if _, err := s.Part(ctx, id(n, 16)); !errors.Is(err, want) {
			t.Errorf("part %d: %v, want %v", n, err, want)
		}
	}
	// The parcel that expired is its tombstone, as of when it expired; the one taken 96 days ago
	// is forgotten.
	list, err := s.Pairs(ctx, kept, now)
	if err != nil {
		t.Fatal(err)
	}
	if len(list) != 2 || list[0].ID[0] != 6 || list[0].State != PairWaiting || list[1].ID[0] != 5 || list[1].State != PairExpired ||
		!list[1].At.Equal(t0.Add(390*day)) {
		t.Errorf("parcels after the sweep: %+v", list)
	}
	checkBytes(t, s)
	if swept, err := s.Sweep(ctx, now, horizons, 256); err != nil || len(swept.Spaces) != 0 || swept.Pairs != 0 || len(swept.Parts) != 0 {
		t.Errorf("a second sweep: %+v %v", swept, err)
	}
	checkBytes(t, s)
}

// A data directory of Phloem 0.1.0, with the first migration only, opens: the second adds the
// tables of step 3, and what the first held stays as it was.
func TestOpenMigratesTheFirstSchema(t *testing.T) {
	dir := t.TempDir()
	db, err := sql.Open("sqlite", dsn(filepath.Join(dir, "phloem.db")))
	if err != nil {
		t.Fatal(err)
	}
	first, err := migrationFS.ReadFile("migrations/0001_phloem.sql")
	if err != nil {
		t.Fatal(err)
	}
	sp, pid, ms := id(1, 16), id(5, 16), t0.UnixMilli()
	for _, q := range []struct {
		sql  string
		args []any
	}{
		{string(first), nil},
		{"PRAGMA user_version = 1", nil},
		{"INSERT INTO spaces (id, token_hash, created, touched, bytes) VALUES (?, ?, ?, ?, ?)", []any{sp, TokenHash(id(1, 32)), ms, ms, 2*RowBytes + 3}},
		{"INSERT INTO devices (space, device, slot, seen) VALUES (?, ?, 1, ?)", []any{sp, id(1, 16), ms}},
		{"INSERT INTO pairs (id, space, box, expires) VALUES (?, ?, ?, ?)", []any{pid, sp, []byte("box"), ms + 60_000}},
	} {
		if _, err := db.Exec(q.sql, q.args...); err != nil {
			t.Fatalf("%.40s: %v", q.sql, err)
		}
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	var v int
	if err := s.w.QueryRow("PRAGMA user_version").Scan(&v); err != nil || v != 2 {
		t.Fatalf("user_version %d, %v", v, err)
	}
	if n, b := s.Totals(); n != 1 || b != 2*RowBytes+3 {
		t.Errorf("totals %d %d", n, b)
	}
	if _, err := s.Authenticate(ctx, sp, id(1, 32)); err != nil {
		t.Fatalf("the space of 0.1.0: %v", err)
	}
	checkBytes(t, s)
	if took, err := s.TakePair(ctx, pid, t0, 256); err != nil || string(took.Box) != "box" {
		t.Fatalf("the parcel of 0.1.0: %+v %v", took, err)
	}
	if took, err := s.TakePair(ctx, pid, t0, 256); err != nil || took.State != PairTaken {
		t.Fatalf("the parcel of 0.1.0 taken: %+v %v", took, err)
	}
	if rev, err := s.PutPart(ctx, id(7, 16), sp, []byte("part"), t0, 32); err != nil || rev != 1 {
		t.Fatalf("a part of the space of 0.1.0: %d %v", rev, err)
	}
	checkBytes(t, s)
}

func TestConcurrentWritersKeepTheTotalsExact(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	done := make(chan error, 8)
	for w := range 8 {
		go func() {
			for i := range 25 {
				if _, err := s.Append(ctx, sp, w+1, fmt.Sprint(i), make([]byte, w*10+i), t0, 1<<30); err != nil {
					done <- err
					return
				}
			}
			done <- nil
		}()
	}
	for range 8 {
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
	got, _ := s.Space(ctx, sp)
	if got.Head != 200 {
		t.Errorf("head %d after 200 pushes", got.Head)
	}
	entries, _ := s.EntriesAfter(ctx, sp, 0)
	for i, e := range entries {
		if e.Nr != int64(i+1) {
			t.Fatalf("entry %d has number %d: no gap, no double", i, e.Nr)
		}
	}
	checkBytes(t, s)
}
