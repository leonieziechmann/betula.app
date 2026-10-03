//go:build unix && !aix && !solaris && !hurd

package cluster_test

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/client"
)

// TestSoakLeadersComeAndGo kills the leader 20 times under the load of clients that store
// files and fetch pages through both instances, and restarts it on its data each time. Every
// write acknowledged more than 5 s before the next kill must survive (owner, 2026-10-02: the
// last minutes before a crash may be lost; this pair loses far less). About 2.5 minutes.
func TestSoakLeadersComeAndGo(t *testing.T) {
	if os.Getenv("CORTEX_SOAK") != "1" {
		t.Skip("the soak test runs with CORTEX_SOAK=1 (about 2.5 minutes)")
	}
	const (
		kills  = 20
		cycle  = 6500 * time.Millisecond // longer than the 5 s within which a write may be lost
		lossOK = 5 * time.Second
	)
	host := newSite(t)
	for i := range 20 {
		host.set(fmt.Sprintf("/page/%d", i), compressible(2000+i))
	}
	lock := filepath.Join(t.TempDir(), "leader.lock")
	dirs := map[string]string{"a": t.TempDir(), "b": t.TempDir()}
	insts := map[string]*instance{"a": startInstance(t, "a", dirs["a"], lock)}
	waitFor(t, "a to lead", 5*time.Second, insts["a"].leads)
	insts["b"] = startInstance(t, "b", dirs["b"], lock)
	addrs := map[string]string{"a": insts["a"].addr(), "b": insts["b"].addr()}
	c, err := client.New("http://"+addrs["a"]+",http://"+addrs["b"], client.Options{FailoverWait: 15 * time.Second})
	if err != nil {
		t.Fatal(err)
	}

	type write struct {
		name, hash string
		acked      time.Time
	}
	var mu sync.Mutex
	var writes []write
	stop := make(chan struct{})
	var wg sync.WaitGroup
	for w := range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := 0; ; n++ {
				select {
				case <-stop:
					return
				default:
				}
				name := fmt.Sprintf("soak/%d/%06d", w, n)
				content := name + ": " + randomText(200+n%3000, int64(w*1000000+n))
				if _, _, err := c.PutFile(context.Background(), name, strings.NewReader(content), client.PutOptions{}); err != nil {
					continue
				}
				mu.Lock()
				writes = append(writes, write{name, sha(content), time.Now()})
				mu.Unlock()
			}
		}()
	}
	wg.Add(1)
	go func() { // and some reading
		defer wg.Done()
		for n := 0; ; n++ {
			select {
			case <-stop:
				return
			default:
			}
			resp, err := c.Fetch(context.Background(), host.url(fmt.Sprintf("/page/%d", n%20)), client.FetchOptions{MaxAge: -1})
			if err == nil {
				_, _ = io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
			}
			time.Sleep(5 * time.Millisecond)
		}
	}()

	var killed []time.Time
	for i := range kills {
		time.Sleep(cycle)
		var leader *instance
		waitFor(t, "a leader", 10*time.Second, func() bool {
			for _, in := range insts {
				if in.leads() {
					leader = in
					return true
				}
			}
			return false
		})
		other := insts[map[string]string{"a": "b", "b": "a"}[leader.name]]
		ls, _ := leader.st.Position()
		fs, _ := other.st.Position()
		lag := other.peer.Follower()
		leader.crash()
		killed = append(killed, time.Now()) // it answered requests in flight until here
		time.Sleep(300 * time.Millisecond)
		insts[leader.name] = startInstance(t, leader.name, dirs[leader.name], lock, withAddr(addrs[leader.name]))
		t.Logf("kill %d: %s at %d; the follower at %d (%.2f s behind, state %s, %d blobs missing)", i+1, leader.name, ls, fs,
			lag.LagSeconds, lag.State, lag.BlobsMissing)
	}
	time.Sleep(cycle)
	close(stop)
	wg.Wait()

	var leader, follower *instance
	waitFor(t, "a leader", 10*time.Second, func() bool {
		for name, in := range insts {
			if in.leads() {
				leader, follower = in, insts[map[string]string{"a": "b", "b": "a"}[name]]
				return true
			}
		}
		return false
	})
	caughtUp(t, leader, follower)
	waitFor(t, "the follower's blobs", 30*time.Second, func() bool { return follower.peer.Follower().BlobsMissing == 0 })

	var checked, atRisk, lost int
	for _, w := range writes {
		risky := false
		for _, k := range killed {
			if k.After(w.acked) {
				risky = k.Sub(w.acked) <= lossOK
				break
			}
		}
		present := true
		for _, in := range []*instance{leader, follower} {
			if fv, err := in.st.GetFile(w.name); err != nil || fv.Hash != w.hash || !in.st.HasBlob(fv.Hash) {
				present = false
			}
		}
		switch {
		case risky:
			atRisk++
			if !present {
				lost++
			}
		case !present:
			t.Errorf("%s, acknowledged at %s, is lost", w.name, w.acked.Format("15:04:05.000"))
		default:
			checked++
		}
	}
	t.Logf("%d writes acknowledged: %d checked and kept, %d within 5 s of a kill (%d of them lost)", len(writes), checked, atRisk, lost)
	if checked < 100 {
		t.Errorf("only %d writes checked", checked)
	}
}
