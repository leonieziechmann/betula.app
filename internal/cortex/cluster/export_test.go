package cluster

import (
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
)

// Crash stops p as a process that dies would: its lock file is closed without unlocking, and
// nothing is handed over.
func Crash(p *Peer) { p.crash() }

// SetBeforeApply has fn run before each Apply of p's follower.
func SetBeforeApply(p *Peer, fn func(store.JournalEntry)) { p.beforeApply.Store(&fn) }

// Failures is how many replication rounds of p have failed.
func Failures(p *Peer) int64 { return p.failures.Load() }

// SetStaleWait sets how long a candidate behind the recorded head waits, until the test ends.
func SetStaleWait(t interface{ Cleanup(func()) }, d time.Duration) {
	before := staleWait
	staleWait = d
	t.Cleanup(func() { staleWait = before })
}
