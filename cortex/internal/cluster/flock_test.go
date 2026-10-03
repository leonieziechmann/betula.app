//go:build unix && !aix && !solaris && !hurd

package cluster

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// won is a campaign that ended with the lock.
type won struct {
	name string
	term Term
}

// campaign runs e's campaign in the background; a won term arrives on wins.
func campaign(ctx context.Context, name string, e Elector, wins chan<- won) {
	go func() {
		if term, err := e.Campaign(ctx); err == nil {
			wins <- won{name, term}
		}
	}()
}

// takeOverWithin bounds a take-over by a campaign that polls the lock every flockPoll: a few
// polls, and room for a scheduler on a loaded machine (the bound was 200 ms, 4 polls).
const takeOverWithin = 20 * flockPoll

func waitWin(t *testing.T, wins <-chan won, within time.Duration) won {
	t.Helper()
	select {
	case w := <-wins:
		return w
	case <-time.After(within):
		t.Fatalf("nobody won within %s", within)
		return won{}
	}
}

func noWin(t *testing.T, wins <-chan won, during time.Duration) {
	t.Helper()
	select {
	case w := <-wins:
		t.Fatalf("%s won although the lock is held", w.name)
	case <-time.After(during):
	}
}

func TestFlockElectsOneLeaderAndTheOtherTakesOverWhenItDies(t *testing.T) {
	lock := filepath.Join(t.TempDir(), "leader.lock")
	electors := map[string]Elector{"a": Flock(lock), "b": Flock(lock)}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	wins := make(chan won, 2)
	for name, e := range electors {
		campaign(ctx, name, e, wins)
	}
	first := waitWin(t, wins, 10*time.Second)
	noWin(t, wins, 300*time.Millisecond) // exactly one

	// The leader dies: the kernel closes its file, and with it the lock.
	start := time.Now()
	first.term.(*flockTerm).abandon()
	// The other campaign polls the lock every flockPoll (50 ms): it takes over within a few
	// polls (takeOverWithin leaves room for a loaded machine).
	second := waitWin(t, wins, 10*time.Second)
	if took := time.Since(start); took >= takeOverWithin {
		t.Errorf("the take-over took %s, want less than %s", took, takeOverWithin)
	}
	if second.name == first.name {
		t.Fatalf("%s won twice", first.name)
	}

	// Resign hands over: the first one campaigns again and wins once the second resigns.
	campaign(ctx, first.name, electors[first.name], wins)
	noWin(t, wins, 200*time.Millisecond)
	if err := second.term.Resign(); err != nil {
		t.Fatalf("Resign: %v", err)
	}
	if err := second.term.Resign(); err != nil {
		t.Errorf("a second Resign: %v", err)
	}
	start = time.Now()
	third := waitWin(t, wins, 10*time.Second)
	if third.name != first.name || time.Since(start) >= takeOverWithin {
		t.Errorf("after the resignation %s won after %s", third.name, time.Since(start))
	}
	select {
	case <-second.term.Lost():
		t.Error("a resigned term counts as lost")
	default:
	}
	_ = third.term.Resign()
}

func TestFlockCampaignEndsWithItsContext(t *testing.T) {
	lock := filepath.Join(t.TempDir(), "leader.lock")
	holder, err := Flock(lock).Campaign(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer holder.Resign()
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	start := time.Now()
	if _, err := Flock(lock).Campaign(ctx); !errors.Is(err, context.DeadlineExceeded) {
		t.Errorf("Campaign = %v, want the context's error", err)
	}
	if took := time.Since(start); took > time.Second {
		t.Errorf("Campaign returned %s after its context ended", took)
	}
}

func TestFlockAnnouncesTheLeaderInLeaderJSON(t *testing.T) {
	dir := t.TempDir()
	lock := filepath.Join(dir, "leader.lock")
	a, b := Flock(lock), Flock(lock)
	if _, ok := b.Leader(); ok {
		t.Error("a leader is known before anybody announced one")
	}
	since := time.Date(2026, 10, 2, 12, 0, 0, 123456000, time.UTC)
	if err := a.Announce(Info{Instance: "a", URL: "http://cortex_a:8100", Epoch: 1, Since: since}); err == nil {
		t.Error("an instance that does not hold the lock announced itself")
	}
	term, err := a.Campaign(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if err := a.Announce(Info{Instance: "a", URL: "http://cortex_a:8100", Epoch: 3, Since: since}); err != nil {
		t.Fatalf("Announce: %v", err)
	}
	got, ok := b.Leader()
	if !ok || got.Instance != "a" || got.URL != "http://cortex_a:8100" || got.Epoch != 3 || !got.Since.Equal(since) {
		t.Errorf("the other instance reads %+v, %v", got, ok)
	}

	data, err := os.ReadFile(filepath.Join(dir, "leader.json"))
	if err != nil {
		t.Fatal(err)
	}
	var doc map[string]any
	if err := json.Unmarshal(data, &doc); err != nil {
		t.Fatalf("leader.json is not JSON: %v\n%s", err, data)
	}
	want := map[string]any{"instance": "a", "url": "http://cortex_a:8100", "epoch": float64(3), "since": "2026-10-02T12:00:00.123456Z"}
	for k, v := range want {
		if doc[k] != v {
			t.Errorf("leader.json %s = %v, want %v (%s)", k, doc[k], v, data)
		}
	}
	if len(doc) != len(want) {
		t.Errorf("leader.json has more than %v: %s", want, data)
	}
	if info, err := os.Stat(filepath.Join(dir, "leader.json")); err != nil || info.Mode().Perm() != 0o644 {
		t.Errorf("leader.json mode %v (%v), want 0644", info.Mode(), err)
	}
	entries, _ := os.ReadDir(dir)
	for _, e := range entries {
		if e.Name() != "leader.lock" && e.Name() != "leader.json" {
			t.Errorf("%s left in the lock directory", e.Name())
		}
	}

	// A leader that resigned no longer overwrites its successor's announcement.
	if err := term.Resign(); err != nil {
		t.Fatal(err)
	}
	bt, err := b.Campaign(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer bt.Resign()
	if err := b.Announce(Info{Instance: "b", URL: "http://cortex_b:8100", Epoch: 4}); err != nil {
		t.Fatal(err)
	}
	if err := a.Announce(Info{Instance: "a", URL: "http://cortex_a:8100", Epoch: 3}); err == nil {
		t.Error("the resigned leader announced itself")
	}
	if got, ok := a.Leader(); !ok || got.Instance != "b" || got.Epoch != 4 || !got.Since.IsZero() {
		t.Errorf("after the hand-over the old leader reads %+v, %v", got, ok)
	}
}

func TestFlockTermIsLostWhenTheLockFileIsReplaced(t *testing.T) {
	lock := filepath.Join(t.TempDir(), "leader.lock")
	a := Flock(lock)
	term, err := a.Campaign(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer term.Resign()
	// Somebody removes the lock file: the next instance locks a new file and leads too, so the
	// holder must learn that its term is over.
	if err := os.Remove(lock); err != nil {
		t.Fatal(err)
	}
	other, err := Flock(lock).Campaign(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer other.Resign()
	select {
	case <-term.Lost():
	case <-time.After(20 * flockWatch):
		t.Fatal("the term was not lost after its lock file was replaced")
	}
	if err := a.Announce(Info{Instance: "a", URL: "http://a"}); err == nil {
		t.Error("a lost term announced itself")
	}
	select {
	case <-other.Lost():
		t.Error("the new holder's term was lost too")
	default:
	}
}

func TestFlockRefusesALockFileItCannotOpen(t *testing.T) {
	dir := t.TempDir() // a directory is no lock file
	e := Flock(dir)
	if _, err := e.Campaign(context.Background()); err == nil {
		t.Error("Campaign on a directory succeeded")
	}
	if err := e.(*flockElector).check(); err == nil {
		t.Error("check of a directory succeeded")
	}
	if _, err := New(context.Background(), openTestStore(t), Info{Instance: "a", URL: "http://a:8100"}, dir, Options{}); err == nil {
		t.Error("New with a lock file that cannot be opened succeeded")
	}
	// A missing directory is created.
	lock := filepath.Join(t.TempDir(), "lock", "leader.lock")
	term, err := Flock(lock).Campaign(context.Background())
	if err != nil {
		t.Fatalf("Campaign in a missing directory: %v", err)
	}
	_ = term.Resign()
}
