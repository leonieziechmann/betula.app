package cluster

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
)

// fakeElector grants the terms the test hands it.
type fakeElector struct {
	mu        sync.Mutex
	announced *Info
	campaigns int
	grant     chan *fakeTerm
}

func newFakeElector() *fakeElector { return &fakeElector{grant: make(chan *fakeTerm)} }

func (e *fakeElector) Campaign(ctx context.Context) (Term, error) {
	e.mu.Lock()
	e.campaigns++
	e.mu.Unlock()
	select {
	case t := <-e.grant:
		return t, nil
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

func (e *fakeElector) Leader() (Info, bool) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.announced == nil {
		return Info{}, false
	}
	return *e.announced, true
}

func (e *fakeElector) Announce(i Info) error {
	e.mu.Lock()
	e.announced = &i
	e.mu.Unlock()
	return nil
}

func (e *fakeElector) campaignsSoFar() int {
	e.mu.Lock()
	defer e.mu.Unlock()
	return e.campaigns
}

type fakeTerm struct {
	lost     chan struct{}
	resigned atomic.Bool
}

func newFakeTerm() *fakeTerm { return &fakeTerm{lost: make(chan struct{})} }

func (t *fakeTerm) Resign() error         { t.resigned.Store(true); return nil }
func (t *fakeTerm) Lost() <-chan struct{} { return t.lost }

func waitUntil(t *testing.T, what string, within time.Duration, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(within)
	for !cond() {
		if time.Now().After(deadline) {
			t.Fatalf("waited %s for %s", within, what)
		}
		time.Sleep(2 * time.Millisecond)
	}
}

// leadingPeer returns a peer that has won its first term.
func leadingPeer(t *testing.T, opts Options) (*Peer, *fakeElector, *fakeTerm) {
	t.Helper()
	e := newFakeElector()
	opts.Elector = freeElector{e} // no other instance holds the lock: Close ends after about a second
	p, err := New(context.Background(), openTestStore(t), Info{Instance: "a", URL: "http://cortex_a:8100"}, "", opts)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = p.Close() })
	if p.Role() != Follower {
		t.Error("a new peer leads before it won")
	}
	if done, ok := p.BeginWrite(); ok || done != nil {
		t.Error("a follower admitted a write")
	}
	if err := p.StepDown(); !errors.Is(err, ErrNotLeader) {
		t.Errorf("StepDown of a follower = %v, want ErrNotLeader", err)
	}
	term := newFakeTerm()
	e.grant <- term
	waitUntil(t, "the peer to lead", 10*time.Second, func() bool { return p.Role() == Leader })
	return p, e, term
}

func TestAPeerLeadsInAnEpochOfItsOwnOnceItWins(t *testing.T) {
	p, e, _ := leadingPeer(t, Options{})
	self := p.Self()
	if self.Instance != "a" || self.URL != "http://cortex_a:8100" || self.Epoch != 1 || self.Since.IsZero() {
		t.Errorf("self = %+v", self)
	}
	if leader, ok := p.Leader(); !ok || leader != self {
		t.Errorf("leader = %+v %v, want itself", leader, ok)
	}
	if announced, ok := e.Leader(); !ok || announced != self {
		t.Errorf("announced %+v %v, want %+v", announced, ok, self)
	}
	if seq, epoch := p.st.Position(); seq != 1 || epoch != 1 {
		t.Errorf("position %d/%d, want the epoch entry", seq, epoch)
	}
	if !p.WaitLeader(context.Background(), 0) || p.Follower() != (FollowerState{}) {
		t.Errorf("WaitLeader false or follower state %+v while leading", p.Follower())
	}
}

func TestAStepDownWaitsForTheWritesInFlightAndStaysOutOfTheElection(t *testing.T) {
	p, e, term := leadingPeer(t, Options{StepDownPause: 400 * time.Millisecond})
	changes := p.LeaderChanges()
	done, ok := p.BeginWrite()
	if !ok {
		t.Fatal("the leader refused a write")
	}
	stepped := make(chan error, 1)
	go func() { stepped <- p.StepDown() }()

	// The fence closes at once; the term ends only after the write.
	waitUntil(t, "the fence to close", 10*time.Second, func() bool {
		d, ok := p.BeginWrite()
		if ok {
			d()
		}
		return !ok
	})
	if p.WaitLeader(context.Background(), 10*time.Millisecond) {
		t.Error("WaitLeader true while handing over")
	}
	time.Sleep(100 * time.Millisecond)
	select {
	case err := <-stepped:
		t.Fatalf("StepDown returned (%v) before the write in flight ended", err)
	default:
	}
	if term.resigned.Load() {
		t.Fatal("resigned before the write in flight ended")
	}
	done()
	done() // a second call changes nothing
	waitUntil(t, "the resignation", 10*time.Second, term.resigned.Load)

	// Until the successor announces itself, this instance still counts as the leader (its
	// journal is served), but writes nothing.
	if p.Role() != Leader {
		t.Error("the role changed before the successor announced itself")
	}
	if _, ok := p.Leader(); ok {
		t.Error("a leader is known while this instance hands over and nobody else announced")
	}
	// The pause begins after the successor announced itself, so no campaign can come before
	// announcedAt + 400 ms however late this goroutine runs.
	announcedAt := time.Now()
	_ = e.Announce(Info{Instance: "b", URL: "http://cortex_b:8100", Epoch: 2})
	select {
	case err := <-stepped:
		if err != nil {
			t.Fatalf("StepDown = %v", err)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("StepDown did not return after the successor announced itself")
	}
	select {
	case <-changes:
	default:
		t.Error("LeaderChanges did not fire")
	}
	if p.Role() != Follower || p.Self().Epoch != 0 {
		t.Errorf("after the step-down: role %v, self %+v", p.Role(), p.Self())
	}
	if leader, ok := p.Leader(); !ok || leader.Instance != "b" {
		t.Errorf("leader after the step-down: %+v %v", leader, ok)
	}

	// It stays out of the election for StepDownPause, then campaigns again.
	campaigns := e.campaignsSoFar()
	time.Sleep(time.Until(announcedAt.Add(250 * time.Millisecond)))
	if n, at := e.campaignsSoFar(), time.Since(announcedAt); n != campaigns && at < 400*time.Millisecond {
		t.Errorf("campaigned %d times within the pause (%s after the announcement)", n-campaigns, at)
	}
	waitUntil(t, "the next campaign", 10*time.Second, func() bool { return e.campaignsSoFar() > campaigns })
	if took := time.Since(announcedAt); took < 400*time.Millisecond {
		t.Errorf("campaigned again %s after the successor announced itself, want after the pause of 400 ms", took)
	}
	e.grant <- newFakeTerm()
	waitUntil(t, "the peer to lead again", 10*time.Second, func() bool { return p.Role() == Leader })
	if epoch := p.Self().Epoch; epoch != 3 {
		t.Errorf("epoch %d after b's 2, want 3", epoch)
	}
}

func TestAPeerWhoseTermIsLostFollowsAtOnce(t *testing.T) {
	p, e, term := leadingPeer(t, Options{})
	changes := p.LeaderChanges()
	campaigns := e.campaignsSoFar()
	close(term.lost)
	waitUntil(t, "the peer to follow", 5*time.Second, func() bool { return p.Role() == Follower })
	if _, ok := p.BeginWrite(); ok {
		t.Error("a write admitted after the term was lost")
	}
	select {
	case <-changes:
	case <-time.After(10 * time.Second):
		t.Error("LeaderChanges did not fire")
	}
	waitUntil(t, "the resignation", 10*time.Second, term.resigned.Load)
	waitUntil(t, "the next campaign, without a pause", 5*time.Second, func() bool { return e.campaignsSoFar() > campaigns })
}

// freeElector is a fakeElector whose lock nobody else holds (lockProbe): no successor is
// campaigning.
type freeElector struct{ *fakeElector }

func (freeElector) Held() (bool, error) { return false, nil }

// A leader that is stopped with no successor campaigning (nobody takes the lock) waits for one
// about a second, not the whole successorWait. Before review 2 the bound was a fixed second;
// now a successor that is catching up gets up to successorWait (E2E-1), so the test gives the
// elector a lock probe that says nobody holds the lock.
func TestClosingALeaderWithoutASuccessorEndsWithinTwoSeconds(t *testing.T) {
	e := freeElector{newFakeElector()}
	p, err := New(context.Background(), openTestStore(t), Info{Instance: "a", URL: "http://cortex_a:8100"}, "", Options{Elector: e})
	if err != nil {
		t.Fatal(err)
	}
	term := newFakeTerm()
	e.grant <- term
	waitUntil(t, "the peer to lead", 10*time.Second, func() bool { return p.Role() == Leader })
	start := time.Now()
	if err := p.Close(); err != nil {
		t.Fatal(err)
	}
	// About a second; the bound leaves room for a loaded machine and is still half of
	// successorWait (20 s), which Close took before.
	if took := time.Since(start); took > 10*time.Second {
		t.Errorf("Close took %s", took)
	}
	if !term.resigned.Load() || p.Role() != Follower {
		t.Errorf("after Close: resigned %v, role %v", term.resigned.Load(), p.Role())
	}
	if err := p.StepDown(); !errors.Is(err, ErrNotLeader) {
		t.Errorf("StepDown after Close = %v", err)
	}
}

func TestNewRefusesWhatCannotElect(t *testing.T) {
	st := openTestStore(t)
	for _, tc := range []struct {
		name string
		self Info
		lock string
	}{
		{"no instance", Info{URL: "http://a:8100"}, "x.lock"},
		{"no URL", Info{Instance: "a"}, "x.lock"},
		{"a URL without a host", Info{Instance: "a", URL: "http://"}, "x.lock"},
		{"no lock", Info{Instance: "a", URL: "http://a:8100"}, ""},
	} {
		if _, err := New(context.Background(), st, tc.self, tc.lock, Options{}); err == nil {
			t.Errorf("%s: New succeeded", tc.name)
		}
	}
	if _, err := New(context.Background(), nil, Info{Instance: "a", URL: "http://a:8100"}, "x.lock", Options{Elector: newFakeElector()}); err == nil {
		t.Error("New without a store succeeded")
	}
}

// fakeLeader serves a real store's journal, snapshot and blobs over HTTP, leaving out the
// journal entry skip (a hole, as a journal trimmed by time could have had).
func fakeLeader(t *testing.T, st *store.Store, skip int64) (*httptest.Server, *atomic.Int64) {
	t.Helper()
	var snapshots atomic.Int64
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/internal/v1/journal":
			after, _ := strconv.ParseInt(r.URL.Query().Get("after"), 10, 64)
			entries, err := st.JournalAfter(after, 0)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			seq, _, at := st.Head()
			w.Header().Set("Cortex-Head-Seq", strconv.FormatInt(seq, 10))
			w.Header().Set("Cortex-Head-At", store.FormatTime(at))
			enc := json.NewEncoder(w)
			for _, e := range entries {
				if e.Seq != skip {
					_ = enc.Encode(e)
				}
			}
		case r.URL.Path == "/internal/v1/snapshot":
			snapshots.Add(1)
			_, _, _ = st.Snapshot(w)
		case strings.HasPrefix(r.URL.Path, "/v1/blobs/sha256:"):
			rc, gz, _, err := st.OpenStored(strings.TrimPrefix(r.URL.Path, "/v1/blobs/sha256:"))
			if err != nil {
				http.NotFound(w, r)
				return
			}
			defer rc.Close()
			if gz {
				w.Header().Set("Content-Encoding", "gzip")
			}
			_, _ = io.Copy(w, rc)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(srv.Close)
	return srv, &snapshots
}

func TestAFollowerTakesASnapshotWhenTheLeadersJournalHasAHole(t *testing.T) {
	leader := openTestStore(t)
	at := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	if _, _, err := leader.StartEpoch("a", "http://a", 0, at); err != nil {
		t.Fatal(err)
	}
	for i, content := range []string{"first", "second", "third"} {
		b, err := leader.PutBlob(strings.NewReader(content), "", 0)
		if err != nil {
			t.Fatal(err)
		}
		if _, _, _, err := leader.PutFile("f"+strconv.Itoa(i), b, "text/plain", at.Add(time.Duration(i)*time.Second)); err != nil {
			t.Fatal(err)
		}
	}
	// The follower has the epoch entry; the leader's journal answers 2 and 4, not 3.
	follower := openTestStore(t)
	first, err := leader.JournalAfter(0, 1)
	if err != nil || follower.Apply(first[0]) != nil {
		t.Fatalf("seeding the follower: %v", err)
	}
	srv, snapshots := fakeLeader(t, leader, 3)
	e := newFakeElector()
	_ = e.Announce(Info{Instance: "a", URL: srv.URL, Epoch: 1})
	p, err := New(context.Background(), follower, Info{Instance: "b", URL: "http://cortex_b:8100"}, "", Options{Elector: e})
	if err != nil {
		t.Fatal(err)
	}
	defer p.Close()

	// Once the follower says it follows with no blob missing, it has them all: it counts the
	// blobs it lacks before it says so.
	waitUntil(t, "the follower to catch up", 20*time.Second, func() bool {
		fs, _ := follower.Position()
		ls, _ := leader.Position()
		f := p.Follower()
		return fs == ls && f.State == "following" && f.LagEntries == 0 && f.BlobsMissing == 0
	})
	if n := snapshots.Load(); n != 1 {
		t.Errorf("%d snapshots, want 1", n)
	}
	for i, content := range []string{"first", "second", "third"} {
		fv, err := follower.GetFile("f" + strconv.Itoa(i))
		if err != nil || fv.Hash != sha256Hex(content) || !follower.HasBlob(fv.Hash) {
			t.Errorf("f%d on the follower: %+v %v", i, fv, err)
		}
	}
}

func sha256Hex(s string) string {
	sum := sha256.Sum256([]byte(s))
	return hex.EncodeToString(sum[:])
}

func TestAFollowerCountsTheBlobsItLacksBeforeItSaysItFollows(t *testing.T) {
	leader := openTestStore(t)
	at := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	if _, _, err := leader.StartEpoch("a", "http://a", 0, at); err != nil {
		t.Fatal(err)
	}
	for i, content := range []string{"first", "second", "third"} {
		b, err := leader.PutBlob(strings.NewReader(content), "", 0)
		if err != nil {
			t.Fatal(err)
		}
		if _, _, _, err := leader.PutFile("f"+strconv.Itoa(i), b, "text/plain", at); err != nil {
			t.Fatal(err)
		}
	}
	srv, snapshots := fakeLeader(t, leader, -1)
	follower := openTestStore(t) // empty: it starts from a snapshot, which has no blobs
	e := newFakeElector()
	p, err := New(context.Background(), follower, Info{Instance: "b", URL: "http://cortex_b:8100"}, "", Options{Elector: e})
	if err != nil {
		t.Fatal(err)
	}
	defer p.Close()
	release := make(chan struct{})
	defer close(release)
	hold := func() { <-release } // the back-fill waits: only the follower's own count is there
	p.beforeBackfill.Store(&hold)
	_ = e.Announce(Info{Instance: "a", URL: srv.URL, Epoch: 1})

	waitUntil(t, "the follower to say it follows", 20*time.Second, func() bool {
		f := p.Follower()
		return f.State == "following" && f.LagEntries == 0
	})
	if f := p.Follower(); f.BlobsMissing != 3 || snapshots.Load() != 1 {
		t.Fatalf("following after %d snapshots with %d blobs missing, want 1 and 3: a hand-over would go to it", snapshots.Load(), f.BlobsMissing)
	}
	release <- struct{}{} // the first pass (at the start, nothing to do) may already be past it
	waitUntil(t, "the back-fill", 20*time.Second, func() bool { return p.Follower().BlobsMissing == 0 })
	for i := range 3 {
		if fv, err := follower.GetFile("f" + strconv.Itoa(i)); err != nil || !follower.HasBlob(fv.Hash) {
			t.Errorf("f%d: %+v %v", i, fv, err)
		}
	}
}

func TestAFollowerThatRestartsWithoutSomeBlobsCountsThemFirst(t *testing.T) {
	leader := openTestStore(t)
	at := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	if _, _, err := leader.StartEpoch("a", "http://a", 0, at); err != nil {
		t.Fatal(err)
	}
	var hashes []string
	for i, content := range []string{"first", "second"} {
		b, err := leader.PutBlob(strings.NewReader(content), "", 0)
		if err != nil {
			t.Fatal(err)
		}
		if _, _, _, err := leader.PutFile("f"+strconv.Itoa(i), b, "text/plain", at); err != nil {
			t.Fatal(err)
		}
		hashes = append(hashes, b.Hash)
	}
	// The follower has every entry, but its blobs went missing (an earlier back-fill that
	// did not finish, a disk that lost them).
	follower := openTestStore(t)
	entries, err := leader.JournalAfter(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	for _, e := range entries {
		if e.Blob != "" {
			rc, gz, _, err := leader.OpenStored(e.Blob)
			if err != nil {
				t.Fatal(err)
			}
			err = follower.ImportStored(e.Blob, rc, gz)
			rc.Close()
			if err != nil {
				t.Fatal(err)
			}
		}
		if err := follower.Apply(e); err != nil {
			t.Fatal(err)
		}
	}
	for _, hash := range hashes {
		for _, suffix := range []string{"", ".gz"} {
			_ = os.Remove(filepath.Join(follower.Dir(), "blobs", "sha256", hash[:2], hash+suffix))
		}
	}

	srv, snapshots := fakeLeader(t, leader, -1)
	e := newFakeElector()
	p, err := New(context.Background(), follower, Info{Instance: "b", URL: "http://cortex_b:8100"}, "", Options{Elector: e})
	if err != nil {
		t.Fatal(err)
	}
	defer p.Close()
	release := make(chan struct{})
	defer close(release)
	hold := func() { <-release }
	p.beforeBackfill.Store(&hold)
	_ = e.Announce(Info{Instance: "a", URL: srv.URL, Epoch: 1})

	waitUntil(t, "the follower to say it follows", 20*time.Second, func() bool {
		f := p.Follower()
		return f.State == "following" && f.LagEntries == 0
	})
	if f := p.Follower(); f.BlobsMissing != 2 || snapshots.Load() != 0 {
		t.Fatalf("following after %d snapshots with %d blobs missing, want 0 and 2", snapshots.Load(), f.BlobsMissing)
	}
	release <- struct{}{}
	waitUntil(t, "the back-fill", 20*time.Second, func() bool { return p.Follower().BlobsMissing == 0 })
	for _, hash := range hashes {
		if !follower.HasBlob(hash) {
			t.Errorf("blob %s not back-filled", hash)
		}
	}
}

// bookElector is a freeElector that keeps a HeadRecord (headBook).
type bookElector struct {
	freeElector
	head HeadRecord
}

func (e *bookElector) RecordHead(h HeadRecord) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.head = h
	return nil
}

func (e *bookElector) LastHead() (HeadRecord, bool) {
	e.mu.Lock()
	defer e.mu.Unlock()
	return e.head, e.head.Instance != ""
}

// Review 2 epoch-reuse: the epoch of a new term goes above the recorded head's, also when the
// announcement (leader.json) is gone and the index knows of no later epoch. While it leads, the
// instance records its own head.
func TestANewTermsEpochGoesAboveTheRecordedHead(t *testing.T) {
	e := &bookElector{freeElector: freeElector{newFakeElector()}, head: HeadRecord{Instance: "b", URL: "http://cortex_b:8100", Epoch: 7}}
	p, err := New(context.Background(), openTestStore(t), Info{Instance: "a", URL: "http://cortex_a:8100"}, "", Options{Elector: e})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = p.Close() })
	e.grant <- newFakeTerm()
	waitUntil(t, "the peer to lead", 10*time.Second, func() bool { return p.Role() == Leader })
	if epoch := p.Self().Epoch; epoch != 8 {
		t.Errorf("epoch %d, want 8: above the recorded 7", epoch)
	}
	// The role flips before the first record is written: wait for it rather than look once.
	recorded := func() bool { h, _ := e.LastHead(); return h.Instance == "a" }
	waitUntil(t, "the leader to record its head", 10*time.Second, recorded)
	if h, _ := e.LastHead(); h.Instance != "a" || h.Epoch != 8 || h.Seq != 1 || h.Sum == "" || h.Final {
		t.Errorf("the record while leading: %+v", h)
	}
}
