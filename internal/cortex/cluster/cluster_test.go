package cluster

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
)

func openTestStore(t *testing.T) *store.Store {
	t.Helper()
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = st.Close() })
	return st
}

func TestASingleInstanceLeadsAtOnceInAnEpochOfItsOwn(t *testing.T) {
	st := openTestStore(t)
	node, err := NewSingle(st, Info{Instance: "a", URL: "http://127.0.0.1:8100"})
	if err != nil {
		t.Fatal(err)
	}
	if node.Role() != Leader {
		t.Errorf("role = %v, want Leader", node.Role())
	}
	self := node.Self()
	if self.Instance != "a" || self.URL != "http://127.0.0.1:8100" || self.Epoch != 1 || self.Since.IsZero() {
		t.Errorf("self = %+v, want instance a, its URL, epoch 1 and a start", self)
	}
	if leader, ok := node.Leader(); !ok || leader != self {
		t.Errorf("leader = %+v, %v; want itself", leader, ok)
	}
	// The epoch is in the journal: the first entry of the term.
	if seq, epoch := st.Position(); seq != 1 || epoch != 1 {
		t.Errorf("position = %d, %d; want the epoch entry 1 in epoch 1", seq, epoch)
	}
	if !node.WaitLeader(context.Background(), time.Second) {
		t.Error("WaitLeader = false on the leader")
	}
	if f := node.Follower(); f != (FollowerState{}) {
		t.Errorf("follower state = %+v, want the zero value while leading", f)
	}

	done, ok := node.BeginWrite()
	if !ok {
		t.Fatal("BeginWrite refused on the leader")
	}
	done()

	if err := node.StepDown(); err == nil || errors.Is(err, ErrNotLeader) {
		t.Errorf("StepDown = %v, want a refusal that is not ErrNotLeader", err)
	}
	if node.Role() != Leader {
		t.Error("a refused step-down changed the role")
	}
	select {
	case <-node.LeaderChanges():
		t.Error("LeaderChanges fired although the role never changes")
	default:
	}
}

func TestASingleInstanceCountsItsEpochUpAfterARestart(t *testing.T) {
	dir := t.TempDir()
	for want := int64(1); want <= 3; want++ {
		st, err := store.Open(dir)
		if err != nil {
			t.Fatal(err)
		}
		node, err := NewSingle(st, Info{Instance: "a", URL: "http://a"})
		if err != nil {
			t.Fatal(err)
		}
		if got := node.Self().Epoch; got != want {
			t.Errorf("start %d: epoch %d", want, got)
		}
		_ = st.Close()
	}

	// The announced epoch is a floor.
	st := openTestStore(t)
	node, err := NewSingle(st, Info{Instance: "b", URL: "http://b", Epoch: 7})
	if err != nil {
		t.Fatal(err)
	}
	if got := node.Self().Epoch; got != 8 {
		t.Errorf("epoch with floor 7 = %d, want 8", got)
	}
}

func TestNewSingleFailsOnAClosedStore(t *testing.T) {
	st := openTestStore(t)
	_ = st.Close()
	if _, err := NewSingle(st, Info{Instance: "a"}); err == nil {
		t.Error("NewSingle on a closed store succeeded")
	}
}
