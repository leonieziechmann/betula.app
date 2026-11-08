package store

import (
	"bytes"
	"errors"
	"fmt"
	"testing"
	"time"
)

func TestPartsAreMadeReplacedAndDeleted(t *testing.T) {
	s := open(t)
	sp, other := space(t, s, 1), space(t, s, 2)
	put := func(n byte, owner []byte, data string) (int64, error) {
		t.Helper()
		return s.PutPart(ctx, id(n, 16), owner, []byte(data), t0, 3)
	}
	for i, want := range []int64{1, 2, 3} {
		rev, err := put(5, sp, fmt.Sprint("version ", i))
		if err != nil || rev != want {
			t.Fatalf("put %d: rev %d %v", i, rev, err)
		}
	}
	checkBytes(t, s)
	p, err := s.Part(ctx, id(5, 16))
	if err != nil || p.Rev != 3 || string(p.Data) != "version 2" || !bytes.Equal(p.Space, sp) || !p.At.Equal(t0) {
		t.Fatalf("part %+v %v", p, err)
	}
	if owner, err := s.PartOwner(ctx, id(5, 16)); err != nil || !bytes.Equal(owner, sp) {
		t.Errorf("owner %x %v", owner, err)
	}
	// Another space cannot take it, nor delete it.
	if _, err := put(5, other, "mine now"); !errors.Is(err, ErrExists) {
		t.Errorf("another space's put: %v", err)
	}
	if found, err := s.DeletePart(ctx, id(5, 16), other, t0); err != nil || found {
		t.Errorf("another space's delete: %v %v", found, err)
	}
	if _, err := put(6, id(9, 16), "x"); !errors.Is(err, ErrNotFound) {
		t.Errorf("a part of a missing space: %v", err)
	}
	// Three parts a space; a replacement is no new part.
	for _, n := range []byte{6, 7} {
		if _, err := put(n, sp, "x"); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := put(8, sp, "x"); !errors.Is(err, ErrTooMany) {
		t.Errorf("a fourth part of three: %v", err)
	}
	if rev, err := put(7, sp, "longer than before"); err != nil || rev != 2 {
		t.Errorf("a replacement at the limit: %d %v", rev, err)
	}
	checkBytes(t, s)

	later := t0.Add(time.Hour)
	if found, err := s.DeletePart(ctx, id(5, 16), sp, later); err != nil || !found {
		t.Fatalf("delete: %v %v", found, err)
	}
	if got, _ := s.Space(ctx, sp); !got.Touched.Equal(later) {
		t.Errorf("a delete touches the space: %v", got.Touched)
	}
	if found, err := s.DeletePart(ctx, id(5, 16), sp, later); err != nil || found {
		t.Errorf("deleted twice: %v %v", found, err)
	}
	if _, err := s.Part(ctx, id(5, 16)); !errors.Is(err, ErrGone) {
		t.Errorf("a deleted part: %v", err)
	}
	if _, err := s.PartOwner(ctx, id(5, 16)); !errors.Is(err, ErrGone) {
		t.Errorf("the owner of a deleted part: %v", err)
	}
	// A part never comes back, for no space.
	for _, owner := range [][]byte{sp, other} {
		if _, err := put(5, owner, "again"); !errors.Is(err, ErrExists) {
			t.Errorf("a part of a deleted one's id: %v", err)
		}
	}
	if _, err := s.Part(ctx, id(9, 16)); !errors.Is(err, ErrNotFound) {
		t.Errorf("an id that never was: %v", err)
	}
	// The space has room for a new part again.
	if rev, err := put(8, sp, "x"); err != nil || rev != 1 {
		t.Errorf("a part after a delete: %d %v", rev, err)
	}
	if n, err := s.Parts(ctx); err != nil || n != 3 {
		t.Errorf("parts %d %v", n, err)
	}
	checkBytes(t, s)
}

func TestPartsCountTowardTheBytes(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	total := func() int64 {
		_, b := s.Totals()
		return b
	}
	start := total()
	if _, err := s.PutPart(ctx, id(5, 16), sp, make([]byte, 100), t0, 32); err != nil {
		t.Fatal(err)
	}
	if got := total() - start; got != 100+RowBytes {
		t.Errorf("a new part costs %d", got)
	}
	if _, err := s.PutPart(ctx, id(5, 16), sp, make([]byte, 40), t0, 32); err != nil {
		t.Fatal(err)
	}
	if got := total() - start; got != 40+RowBytes {
		t.Errorf("a smaller replacement: %d", got)
	}
	if _, err := s.PutPart(ctx, id(6, 16), sp, nil, t0, 32); err != nil {
		t.Fatalf("a part of no data: %v", err)
	}
	if p, err := s.Part(ctx, id(6, 16)); err != nil || len(p.Data) != 0 {
		t.Errorf("a part of no data: %+v %v", p, err)
	}
	checkBytes(t, s)
	if _, err := s.DeletePart(ctx, id(5, 16), sp, t0); err != nil {
		t.Fatal(err)
	}
	// The data left; the row stays, as the tombstone's, of no space.
	if got, _ := s.Space(ctx, sp); got.Bytes != RowBytes+RowBytes {
		t.Errorf("the space holds %d", got.Bytes)
	}
	if got := total() - start; got != RowBytes+RowBytes {
		t.Errorf("after the delete: %d", got)
	}
	checkBytes(t, s)
}

// A space deleted takes its parts with it, and leaves their ids gone: they never come back.
func TestDeletingASpaceLeavesItsPartsGone(t *testing.T) {
	s := open(t)
	sp, other := space(t, s, 1), space(t, s, 2)
	for _, n := range []byte{5, 6} {
		if _, err := s.PutPart(ctx, id(n, 16), sp, []byte("part"), t0, 32); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := s.PutPart(ctx, id(7, 16), other, []byte("kept"), t0, 32); err != nil {
		t.Fatal(err)
	}
	found, parts, err := s.DeleteSpace(ctx, sp, t0)
	if err != nil || !found || len(parts) != 2 {
		t.Fatalf("delete: %v %d %v", found, len(parts), err)
	}
	got := map[byte]bool{}
	for _, p := range parts {
		got[p[0]] = true
	}
	if !got[5] || !got[6] {
		t.Errorf("the parts gone: %v", got)
	}
	states, err := s.PartStates(ctx, [][]byte{id(5, 16), id(7, 16), id(9, 16), id(6, 16), id(7, 16)})
	if err != nil {
		t.Fatal(err)
	}
	if want := "[{0 true} {1 false} {0 false} {0 true} {1 false}]"; fmt.Sprint(states) != want {
		t.Errorf("states %v, want %v", states, want)
	}
	// The space made again with the same id cannot take them back.
	space(t, s, 1)
	if _, err := s.PutPart(ctx, id(5, 16), sp, []byte("again"), t0, 32); !errors.Is(err, ErrExists) {
		t.Errorf("a part of the space made again: %v", err)
	}
	if n, b := s.Totals(); n != 2 || b != 2*RowBytes+(4+RowBytes)+2*RowBytes {
		t.Errorf("totals %d %d: two spaces, a part, two tombstones", n, b)
	}
	checkBytes(t, s)
	if states, err := s.PartStates(ctx, nil); err != nil || len(states) != 0 {
		t.Errorf("no states: %v %v", states, err)
	}
}

// Many writers of parts at once: the totals stay exact.
func TestConcurrentPartsKeepTheTotalsExact(t *testing.T) {
	s := open(t)
	sp := space(t, s, 1)
	done := make(chan error, 8)
	for w := range 8 {
		go func() {
			for i := range 20 {
				pid := id(byte(w+1), 16)
				if _, err := s.PutPart(ctx, pid, sp, make([]byte, w*7+i), t0, 32); err != nil {
					done <- err
					return
				}
			}
			if w%2 == 0 {
				if _, err := s.DeletePart(ctx, id(byte(w+1), 16), sp, t0); err != nil {
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
	if p, err := s.Part(ctx, id(2, 16)); err != nil || p.Rev != 20 {
		t.Errorf("part 2: %+v %v", p, err)
	}
	checkBytes(t, s)
}
