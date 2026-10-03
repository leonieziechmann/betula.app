// Package cluster is the place of a Cortex instance in its pair: which instance leads (fetches
// upstream and writes), and how the other follows. The HTTP server (package server) codes
// against Node only, so that how the leader is elected can change without the server: a
// single instance that always leads (NewSingle), or one of a pair that elects its leader
// through an Elector (New; Flock for two instances on one host) and keeps a copy of the
// leader's store while it follows. A Kubernetes Lease would be another Elector (see Elector).
package cluster

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

// Role is what an instance does: a follower serves what its copy has and forwards the rest to
// the leader, which fetches upstream and writes.
type Role int

// The roles.
const (
	Follower Role = iota
	Leader
)

// Info names an instance: its name, the URL the other instance reaches it at, and, for a
// leader, the epoch of its term and when the term began.
type Info struct {
	Instance, URL string
	Epoch         int64
	Since         time.Time
}

// FollowerState is how a follower keeps up with its leader (GET /status, the replication
// metrics). While leading, only BlobsMissing is set.
type FollowerState struct {
	// State is "following" while the lag is under a second (once behind by a second or more:
	// under half a second again) and the blobs it lacks have been counted; "catching_up"
	// otherwise while it reaches the leader; "snapshot" while it copies the leader's index;
	// "no_leader" when it knows or reaches none; "" on the leader.
	State      string  `json:"state"`
	LeaderURL  string  `json:"leader_url"`
	LagSeconds float64 `json:"lag_seconds"` // since the oldest entry of the leader's it lacks was written
	LagEntries int64   `json:"lag_entries"` // the leader's head, as its last answer named it, minus the follower's
	// BlobsMissing counts the blobs the index references that the instance does not have yet
	// and still expects from the other instance, e.g. after a snapshot. A follower with missing
	// blobs would answer for files it cannot read if it took over, so a hand-over waits for 0;
	// a leader promoted before its back-fill ended fetches them from the other instance and
	// reads them from there meanwhile.
	BlobsMissing int64 `json:"blobs_missing"`
}

// ErrNotLeader is returned by StepDown on a follower.
var ErrNotLeader = errors.New("not the leader")

// Node is the instance's place in the pair, as the HTTP server sees it.
type Node interface {
	Role() Role
	Self() Info                                           // this instance (URL = advertise URL; Epoch = current epoch while leading)
	Leader() (Info, bool)                                 // who leads now (Self while leading); false when unknown
	WaitLeader(ctx context.Context, d time.Duration) bool // blocks until THIS instance leads, d passes or ctx ends
	BeginWrite() (done func(), ok bool)                   // a write fence: ok only while leading; StepDown waits for every done
	StepDown() error                                      // ErrNotLeader on a follower; resign and stay out 15 s
	Follower() FollowerState                              // zero value while leading
	LeaderChanges() <-chan struct{}                       // closed-and-replaced (broadcast) whenever the role changes
}

// PeerNode is a Node that knows the other instance of its pair: a leader reads from there what
// its own copy lacks (a blob it has not back-filled yet), instead of failing the request
// (E2E-4). Peer is false when it has not seen the other instance.
type PeerNode interface {
	Node
	Peer() (Info, bool)
}

// errSingle is StepDown's answer on a single instance: there is no other instance to take
// over, and a single instance always leads.
var errSingle = errors.New("a single instance always leads: there is no other instance to hand over to")

// NewSingle returns the Node of an instance that runs alone (no --lock, development, tests): it
// leads at once and for ever. It begins one epoch in st at start (StartEpoch, with self.Epoch
// as the floor), so that its journal tells its term from the one before a restart.
//
// Log events: leader.acquired.
func NewSingle(st *store.Store, self Info) (Node, error) {
	now := time.Now()
	epoch, _, err := st.StartEpoch(self.Instance, self.URL, self.Epoch, now)
	if err != nil {
		return nil, fmt.Errorf("cluster: failed to begin an epoch: %w", err)
	}
	self.Epoch, self.Since = epoch, now.UTC()
	oplog.For("cluster").Info("leading", "event", "leader.acquired", "epoch", epoch, "instance", self.Instance,
		"url", self.URL, "elector", "single")
	return &single{self: self, changes: make(chan struct{})}, nil
}

// single is the Node of NewSingle. Its role never changes, so its LeaderChanges channel is
// never closed and StepDown has nothing to wait for.
type single struct {
	self    Info
	changes chan struct{}
}

func (n *single) Role() Role           { return Leader }
func (n *single) Self() Info           { return n.self }
func (n *single) Leader() (Info, bool) { return n.self, true }

func (n *single) WaitLeader(ctx context.Context, d time.Duration) bool { return true }

func (n *single) BeginWrite() (func(), bool) { return func() {}, true }

func (n *single) StepDown() error { return errSingle }

func (n *single) Follower() FollowerState { return FollowerState{} }

func (n *single) LeaderChanges() <-chan struct{} { return n.changes }
