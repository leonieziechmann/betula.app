package store

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

// Review 2 (snapshot-unbounded-uncancelled): every snapshot request made two full copies of the
// index (VACUUM INTO one file, copied into another), any number of them ran at once, and none
// stopped when the follower went away. OpenSnapshot makes one copy, one at a time, and gives
// up with its context.
func TestOpenSnapshotMakesOneCopyAtATime(t *testing.T) {
	s := openTestStore(t)
	putFile(t, s, "kept.txt", "kept", "text/plain", t0)
	first, err := s.OpenSnapshot(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	seq, epoch := s.Position()
	if first.Seq != seq || first.Epoch != epoch || first.Size <= 0 {
		t.Fatalf("snapshot at %d/%d (%d bytes), want %d/%d", first.Seq, first.Epoch, first.Size, seq, epoch)
	}
	if files := tmpFiles(t, s); len(files) != 1 {
		t.Fatalf("a snapshot keeps %v in DIR/tmp, want one copy", files)
	}

	// A second one waits for the first; a caller that gives up meanwhile gets its context's error.
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	if f, err := s.OpenSnapshot(ctx); !errors.Is(err, context.DeadlineExceeded) {
		if f != nil {
			f.Close()
		}
		t.Fatalf("a second snapshot while the first is open: %v, want it to wait", err)
	}
	got := make(chan error, 1)
	go func() {
		f, err := s.OpenSnapshot(context.Background())
		if err == nil {
			err = f.Close()
		}
		got <- err
	}()
	select {
	case err := <-got:
		t.Fatalf("the second snapshot did not wait for the first: %v", err)
	case <-time.After(100 * time.Millisecond):
	}
	var head [16]byte
	if _, err := first.Read(head[:]); err != nil || !strings.HasPrefix(string(head[:]), "SQLite format 3") {
		t.Fatalf("the copy reads %q (err %v)", head, err)
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	if err := <-got; err != nil {
		t.Fatalf("the second snapshot after the first: %v", err)
	}
	if files := tmpFiles(t, s); len(files) != 0 {
		t.Fatalf("DIR/tmp keeps %v after the snapshots were closed", files)
	}

	// A caller that is gone before the copy is made gets no copy, and none is left behind.
	gone, cancel := context.WithCancel(context.Background())
	cancel()
	if f, err := s.OpenSnapshot(gone); err == nil {
		f.Close()
		t.Fatal("a snapshot for a caller that went away was made")
	}
	if files := tmpFiles(t, s); len(files) != 0 {
		t.Fatalf("DIR/tmp keeps %v after a cancelled snapshot", files)
	}
	// The slot is free again.
	f, err := s.OpenSnapshot(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	f.Close()
}
