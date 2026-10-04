//go:build !(unix && !aix && !solaris && !hurd)

package cluster

import (
	"context"
	"errors"
)

// errNoFlock is the answer of Flock's elector where the system has no flock.
var errNoFlock = errors.New("cluster: the flock elector needs a Unix system with flock(2); run without --lock")

// Flock returns an elector that cannot elect on this system: its Campaign fails. Cortex elects
// with flock on Unix systems only.
func Flock(lockPath string) Elector { return noFlock{} }

type noFlock struct{}

func (noFlock) String() string                         { return "flock" }
func (noFlock) Campaign(context.Context) (Term, error) { return nil, errNoFlock }
func (noFlock) Leader() (Info, bool)                   { return Info{}, false }
func (noFlock) Announce(Info) error                    { return errNoFlock }
func (noFlock) check() error                           { return errNoFlock }
