//go:build unix && !aix && !solaris && !hurd

package cluster

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sync"
	"syscall"
	"time"
)

// flockPoll is how often a campaign tries the lock: a holder that dies is replaced within
// about this long.
const flockPoll = 50 * time.Millisecond

// flockWatch is how often the holder of the lock checks that the lock file is still the file
// it locked (see flockTerm.watch).
const flockWatch = 500 * time.Millisecond

// Flock returns the elector of instances on one host that share a directory: the one that
// holds an exclusive flock on lockPath leads. Campaign tries the lock every 50 ms, so when the
// holder dies (the kernel releases its lock with its file descriptor) another instance takes
// over within about 100 ms. The leader announces itself in leader.json beside the lock file
// (written to a temporary file, synced and renamed into place), which Leader reads.
//
// Every instance must reach the same file: the same host (one kernel) and the same directory,
// for example one Docker volume mounted into both containers. A network file system whose
// flock is not shared between clients would elect two leaders.
func Flock(lockPath string) Elector {
	dir := filepath.Dir(lockPath)
	return &flockElector{path: lockPath, announcePath: filepath.Join(dir, "leader.json"), headPath: filepath.Join(dir, "head.json")}
}

type flockElector struct {
	path, announcePath, headPath string

	mu   sync.Mutex
	term *flockTerm // the term this elector holds, nil when none
}

func (e *flockElector) String() string { return "flock" }

// check fails when the lock file cannot be opened (New refuses a configuration that cannot
// elect).
func (e *flockElector) check() error {
	if err := os.MkdirAll(filepath.Dir(e.path), 0o755); err != nil {
		return fmt.Errorf("cluster: lock directory: %w", err)
	}
	f, err := os.OpenFile(e.path, os.O_RDWR|os.O_CREATE, 0o644)
	if err != nil {
		return fmt.Errorf("cluster: lock file: %w", err)
	}
	return f.Close()
}

func (e *flockElector) Campaign(ctx context.Context) (Term, error) {
	if err := os.MkdirAll(filepath.Dir(e.path), 0o755); err != nil {
		return nil, fmt.Errorf("cluster: lock directory: %w", err)
	}
	tick := time.NewTicker(flockPoll)
	defer tick.Stop()
	for {
		term, err := e.try()
		if err != nil || term != nil {
			return term, err
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-tick.C:
		}
	}
}

// try takes the lock if nobody holds it: nil, nil when somebody does.
func (e *flockElector) try() (*flockTerm, error) {
	f, err := os.OpenFile(e.path, os.O_RDWR|os.O_CREATE, 0o644)
	if err != nil {
		return nil, fmt.Errorf("cluster: lock file: %w", err)
	}
	if err := flock(f, syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		_ = f.Close()
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EINTR) {
			return nil, nil
		}
		return nil, fmt.Errorf("cluster: lock %s: %w", e.path, err)
	}
	// The file may have been removed or replaced between the open and the lock: a lock on a
	// file that the others no longer open elects nobody.
	if !isFile(f, e.path) {
		_ = f.Close()
		return nil, nil
	}
	t := &flockTerm{e: e, f: f, lost: make(chan struct{}), done: make(chan struct{})}
	e.mu.Lock()
	e.term = t
	e.mu.Unlock()
	go t.watch()
	return t, nil
}

// flock applies how to f's lock, without Fd's switch of the file to blocking mode.
func flock(f *os.File, how int) error {
	raw, err := f.SyscallConn()
	if err != nil {
		return err
	}
	var ferr error
	if err := raw.Control(func(fd uintptr) { ferr = syscall.Flock(int(fd), how) }); err != nil {
		return err
	}
	return ferr
}

// isFile says whether path still names the file f has open.
func isFile(f *os.File, path string) bool {
	open, err := f.Stat()
	if err != nil {
		return false
	}
	named, err := os.Stat(path)
	return err == nil && os.SameFile(open, named)
}

// announcement is leader.json.
type announcement struct {
	Instance string `json:"instance"`
	URL      string `json:"url"`
	Epoch    int64  `json:"epoch"`
	Since    string `json:"since"` // RFC 3339
}

func (e *flockElector) Leader() (Info, bool) {
	data, err := os.ReadFile(e.announcePath)
	if err != nil {
		return Info{}, false
	}
	var a announcement
	if err := json.Unmarshal(data, &a); err != nil || a.Instance == "" {
		return Info{}, false
	}
	info := Info{Instance: a.Instance, URL: a.URL, Epoch: a.Epoch}
	if since, err := time.Parse(time.RFC3339Nano, a.Since); err == nil {
		info.Since = since
	}
	return info, true
}

// Announce writes leader.json, while this elector holds the lock: a leader that lost it must
// not overwrite its successor's announcement.
func (e *flockElector) Announce(info Info) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.term == nil || !e.term.holds() {
		return errors.New("cluster: only the holder of the lock announces itself")
	}
	a := announcement{Instance: info.Instance, URL: info.URL, Epoch: info.Epoch}
	if !info.Since.IsZero() {
		a.Since = info.Since.UTC().Format(time.RFC3339Nano)
	}
	data, err := json.Marshal(a)
	if err != nil {
		return err
	}
	return writeFileAtomic(e.announcePath, append(data, '\n'))
}

// RecordHead writes head.json beside leader.json, while this elector holds the lock, for the
// same reason as Announce.
func (e *flockElector) RecordHead(h HeadRecord) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.term == nil || !e.term.holds() {
		return errors.New("cluster: only the holder of the lock records the head")
	}
	data, err := json.Marshal(h)
	if err != nil {
		return err
	}
	return writeFileAtomic(e.headPath, append(data, '\n'))
}

// LastHead reads head.json; false when it is missing or unreadable (a record that cannot be
// read protects nothing, and must not keep the pair from electing a leader).
func (e *flockElector) LastHead() (HeadRecord, bool) {
	data, err := os.ReadFile(e.headPath)
	if err != nil {
		return HeadRecord{}, false
	}
	var h HeadRecord
	if err := json.Unmarshal(data, &h); err != nil || h.Instance == "" {
		return HeadRecord{}, false
	}
	return h, true
}

// Held says whether some instance holds the lock now: it tries the lock itself without
// waiting and lets go at once when it got it. A candidate's campaign that tries in that
// instant tries again 50 ms later.
func (e *flockElector) Held() (bool, error) {
	f, err := os.OpenFile(e.path, os.O_RDWR|os.O_CREATE, 0o644)
	if err != nil {
		return false, fmt.Errorf("cluster: lock file: %w", err)
	}
	defer f.Close()
	if err := flock(f, syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EINTR) {
			return true, nil
		}
		return false, fmt.Errorf("cluster: lock %s: %w", e.path, err)
	}
	return false, flock(f, syscall.LOCK_UN)
}

// writeFileAtomic replaces path with data: a temporary file beside it, synced, renamed into
// place, and the directory synced, so that a reader sees the old file or the new one, never a
// part, also after a crash.
func writeFileAtomic(path string, data []byte) error {
	dir := filepath.Dir(path)
	f, err := os.CreateTemp(dir, "."+filepath.Base(path)+".*")
	if err != nil {
		return fmt.Errorf("cluster: %s: %w", filepath.Base(path), err)
	}
	placed := false
	defer func() {
		if !placed {
			_ = os.Remove(f.Name())
		}
	}()
	_, err = f.Write(data)
	if err == nil {
		err = f.Chmod(0o644)
	}
	if err == nil {
		err = f.Sync()
	}
	if cerr := f.Close(); err == nil {
		err = cerr
	}
	if err == nil {
		err = os.Rename(f.Name(), path)
	}
	if err != nil {
		return fmt.Errorf("cluster: %s: %w", filepath.Base(path), err)
	}
	placed = true
	d, err := os.Open(dir)
	if err != nil {
		return fmt.Errorf("cluster: %s: %w", filepath.Base(path), err)
	}
	defer d.Close()
	if err := d.Sync(); err != nil {
		return fmt.Errorf("cluster: %s: %w", filepath.Base(path), err)
	}
	return nil
}

// flockTerm is the time an elector holds the lock: its file stays open until Resign.
type flockTerm struct {
	e *flockElector

	mu       sync.Mutex
	f        *os.File // nil once ended
	lost     chan struct{}
	lostOnce sync.Once
	done     chan struct{} // closed when the term ends: the watcher stops
}

func (t *flockTerm) Lost() <-chan struct{} { return t.lost }

// holds says whether the term still holds the lock.
func (t *flockTerm) holds() bool {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.f == nil {
		return false
	}
	select {
	case <-t.lost:
		return false
	default:
		return true
	}
}

// Resign unlocks and closes the lock file.
func (t *flockTerm) Resign() error {
	f := t.end()
	if f == nil {
		return nil
	}
	return errors.Join(flock(f, syscall.LOCK_UN), f.Close())
}

// end ends the term and returns its file (nil when it had ended already).
func (t *flockTerm) end() *os.File {
	t.mu.Lock()
	f := t.f
	t.f = nil
	t.mu.Unlock()
	if f == nil {
		return nil
	}
	close(t.done)
	t.e.mu.Lock()
	if t.e.term == t {
		t.e.term = nil
	}
	t.e.mu.Unlock()
	return f
}

// abandon closes the lock file without unlocking it first, as a process that dies does (tests).
func (t *flockTerm) abandon() {
	if f := t.end(); f != nil {
		_ = f.Close()
	}
}

// watch checks that the lock file is still the file this term locked. Somebody who removes or
// replaces it lets another instance lock a new file while this one still believes it leads:
// the term is lost then.
func (t *flockTerm) watch() {
	tick := time.NewTicker(flockWatch)
	defer tick.Stop()
	for {
		select {
		case <-t.done:
			return
		case <-tick.C:
		}
		t.mu.Lock()
		f := t.f
		t.mu.Unlock()
		if f == nil {
			return
		}
		named, err := os.Stat(t.e.path)
		if err != nil && !errors.Is(err, fs.ErrNotExist) {
			continue // cannot tell; the lock is not lost for that
		}
		if open, ferr := f.Stat(); err != nil || ferr != nil || !os.SameFile(open, named) {
			t.lostOnce.Do(func() { close(t.lost) })
			return
		}
	}
}
