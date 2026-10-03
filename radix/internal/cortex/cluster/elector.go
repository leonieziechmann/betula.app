package cluster

import (
	"context"
	"fmt"
	"time"
)

// Elector decides which instance of a pair leads. New codes against it, so that the way the
// leader is elected can change without the rest: Flock elects among instances on one host
// through a lock file both can open.
//
// A Kubernetes elector would implement the same interface with a coordination.k8s.io Lease:
// Campaign creates or takes over the Lease once it has expired, with holderIdentity set to the
// instance's advertise URL, and then renews it in a loop; Leader reads holderIdentity (and the
// epoch from an annotation that Announce writes); Lost fires when a renewal fails before the
// Lease expires, since another instance may hold it from then on. Nothing else in Cortex would
// change. It is not built: Cortex runs as a pair on one host (owner, 2026-10-02).
type Elector interface {
	// Campaign blocks until this instance leads, and returns its term; or until ctx ends (its
	// error). Other errors mean that the election cannot be held (a lock file that cannot be
	// opened); the caller tries again later.
	Campaign(ctx context.Context) (Term, error)
	// Leader returns the leader as it last announced itself; false when none has. The leader
	// may have gone away since: a follower finds that out when it cannot reach it.
	Leader() (Info, bool)
	// Announce publishes the leader after it has begun its epoch (store.StartEpoch). Only the
	// holder of a term can announce.
	Announce(Info) error
}

// Term is one instance's time as the leader.
type Term interface {
	// Resign ends the term at once: another instance can win the next Campaign.
	Resign() error
	// Lost is closed when the term ended without Resign: another instance may lead from then
	// on, so the holder must stop writing at once.
	Lost() <-chan struct{}
}

// electorName names an elector in the log (leader.acquired elector=…).
func electorName(e Elector) string {
	if s, ok := e.(fmt.Stringer); ok {
		return s.String()
	}
	return fmt.Sprintf("%T", e)
}

// HeadRecord is the leader's head as it records it beside the election (head.json for Flock):
// the newest journal entry it has (Seq, Epoch, Sum: store.JournalEntry.Sum; At: when it was
// written) and who wrote it. A candidate that wins the election compares its own journal with
// it before it leads, so that an instance that is behind does not lead over the data of the
// one that is ahead (review 2: stale-instance-wins-and-wipes-newer-data), and takes its epoch
// above Epoch, also when leader.json is gone (review 2: epoch-reuse).
type HeadRecord struct {
	Instance string    `json:"instance"`
	URL      string    `json:"url"`
	Epoch    int64     `json:"epoch"`
	Seq      int64     `json:"seq"`
	Sum      string    `json:"sum"`
	At       time.Time `json:"at"`
	// Final says that the leader wrote it when it ended its term on purpose (a step-down, a
	// shutdown), after its last write: its data is intact and it comes back, so nothing it
	// acknowledged may be given up for a quick take-over. A record that is not final may be
	// that of a leader that crashed, whose last minutes may be lost (owner, 2026-10-02).
	Final bool `json:"final"`
}

// headBook is an Elector that keeps the leader's HeadRecord where every candidate reads it.
// Optional: without it, a candidate knows only the announced leader (Leader).
type headBook interface {
	// RecordHead replaces the record, while this elector holds a term (as Announce).
	RecordHead(HeadRecord) error
	// LastHead returns the last record; false when there is none.
	LastHead() (HeadRecord, bool)
}

// lockProbe is an Elector that can tell whether some instance holds the leadership now. A
// leader that handed over asks it to tell a successor that is catching up (it holds the
// lock) from none at all (nobody does), so that it does not wait for a successor that is not
// coming. Optional: without it, the leader waits the whole successor time.
type lockProbe interface {
	Held() (bool, error)
}
