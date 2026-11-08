package server

import (
	"context"
	"fmt"
	"net/http"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/coder/websocket"
)

func TestTwoDevicesPushAndHearEachOther(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)

	ca, w := e.connect(p, a, 0)
	if w.Slot != 1 || w.Head != 0 || fmt.Sprint(w.Devices) != "[1]" || w.Snapshot != nil || len(w.Entries) != 0 ||
		w.Now != e.clock.Now().UnixMilli() {
		t.Fatalf("the creator's welcome: %s", w.raw)
	}
	cb, w := e.connect(p, b, 0)
	if w.Slot != 2 || fmt.Sprint(w.Devices) != "[1 2]" {
		t.Fatalf("the second device's welcome: %s", w.raw)
	}
	if f := ca.expect("presence"); fmt.Sprint(f.Devices) != "[1 2]" {
		t.Fatalf("presence %s", f.raw)
	}

	ca.push("a-1", "sealed by a")
	if f := ca.expect("stored"); f.ID != "a-1" || f.Nr != 1 {
		t.Fatalf("stored %s", f.raw)
	}
	if f := cb.expect("entry"); f.Nr != 1 || f.Slot != 1 || decoded(t, f.Data) != "sealed by a" {
		t.Fatalf("entry %s", f.raw)
	}
	cb.push("b-1", "sealed by b")
	if f := cb.expect("stored"); f.ID != "b-1" || f.Nr != 2 {
		t.Fatalf("stored %s", f.raw)
	}
	if f := ca.expect("entry"); f.Nr != 2 || f.Slot != 2 || decoded(t, f.Data) != "sealed by b" {
		t.Fatalf("entry %s", f.raw)
	}
	ca.quiet(50 * time.Millisecond) // a hears nothing of its own push but stored
}

func TestThreeDevicesPresenceAndBroadcast(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b, c := newProfile(), randBytes(16), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	cb, _ := e.connect(p, b, 0)
	ca.expect("presence")
	cc, w := e.connect(p, c, 0)
	if w.Slot != 3 || fmt.Sprint(w.Devices) != "[1 2 3]" {
		t.Fatalf("welcome %s", w.raw)
	}
	for _, x := range []*client{ca, cb} {
		if f := x.expect("presence"); fmt.Sprint(f.Devices) != "[1 2 3]" {
			t.Fatalf("presence %s", f.raw)
		}
	}
	cc.push("c-1", "from c")
	cc.expect("stored")
	for _, x := range []*client{ca, cb} {
		if f := x.expect("entry"); f.Nr != 1 || f.Slot != 3 {
			t.Fatalf("entry %s", f.raw)
		}
	}
	_ = cc.conn.Close(websocket.StatusNormalClosure, "")
	for _, x := range []*client{ca, cb} {
		if f := x.expect("presence"); fmt.Sprint(f.Devices) != "[1 2]" {
			t.Fatalf("presence after c went: %s", f.raw)
		}
	}
}

func TestCatchUpAfterAReconnect(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	cb, _ := e.connect(p, b, 0)
	ca.expect("presence")
	ca.push("1", "one")
	ca.expect("stored")
	cb.expect("entry")
	_ = cb.conn.CloseNow()
	if f := ca.expect("presence"); fmt.Sprint(f.Devices) != "[1]" {
		t.Fatalf("presence %s", f.raw)
	}
	ca.push("2", "two")
	ca.expect("stored")
	ca.push("3", "three")
	ca.expect("stored")

	cb, w := e.connect(p, b, 1)
	if w.Slot != 2 || w.Head != 3 || w.Snapshot != nil || fmt.Sprint(w.nrs()) != "[2 3]" || decoded(t, w.Entries[1].Data) != "three" {
		t.Fatalf("catch-up welcome: %s", w.raw)
	}
	ca.expect("presence")
	// Live again from here.
	ca.push("4", "four")
	ca.expect("stored")
	if f := cb.expect("entry"); f.Nr != 4 {
		t.Fatalf("entry %s", f.raw)
	}
}

func TestSnapshotsCompactTheEntries(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.SoftTail, o.Limits.HardTail = 100, 200 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	forty := strings.Repeat("x", 40)
	for i := 1; i <= 5; i++ {
		ca.push(fmt.Sprint("p", i), forty)
		if f := ca.expect("stored"); f.Nr != int64(i) {
			t.Fatalf("stored %s", f.raw)
		}
		if i == 3 { // 120 bytes after the snapshot: above 100
			ca.expect("compact")
		}
	}
	ca.push("p6", forty) // 240 bytes would be above 200
	if f := ca.expect("error"); f.Code != "compact_first" || f.ID != "p6" {
		t.Fatalf("error %s", f.raw)
	}
	ca.snapshot(5, "every record as of 5")
	if f := ca.expect("snapshotted"); f.Upto != 5 {
		t.Fatalf("snapshotted %s", f.raw)
	}
	ca.push("p6", forty)
	if f := ca.expect("stored"); f.Nr != 6 {
		t.Fatalf("stored after the snapshot: %s", f.raw)
	}
	ca.quiet(30 * time.Millisecond) // 40 bytes: no compact

	cases := []struct {
		since    int64
		snapshot bool
		entries  string
	}{
		{0, true, "[6]"}, // a new device: the snapshot and what came after it
		{3, true, "[6]"}, // before the snapshot
		{5, false, "[6]"},
		{6, false, "[]"},
	}
	for _, c := range cases {
		cb, w := e.connect(p, randBytes(16), c.since)
		if (w.Snapshot != nil) != c.snapshot || fmt.Sprint(w.nrs()) != c.entries || w.Head != 6 {
			t.Errorf("since %d: %s", c.since, w.raw)
		}
		if w.Snapshot != nil && (w.Snapshot.Upto != 5 || decoded(t, w.Snapshot.Data) != "every record as of 5") {
			t.Errorf("since %d: snapshot %+v", c.since, w.Snapshot)
		}
		_ = cb.conn.CloseNow()
		ca.expect("presence")
		ca.expect("presence")
	}
	// An older snapshot is not kept; the answer names the kept one.
	ca.snapshot(2, "old")
	if f := ca.expect("snapshotted"); f.Upto != 5 {
		t.Fatalf("snapshotted %s", f.raw)
	}
}

// The first device uploads its snapshot as of entry 0 right after making the space; a device
// that comes with since 0 gets it (docs/phloem/protocol.md section 4).
func TestTheFirstSnapshotIsAsOfEntryZero(t *testing.T) {
	e := newEnv(t, nil)
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	ca.snapshot(0, "the profile as it was")
	if f := ca.expect("snapshotted"); f.Upto != 0 {
		t.Fatalf("snapshotted %s", f.raw)
	}
	cb, w := e.connect(p, randBytes(16), 0)
	if w.Snapshot == nil || w.Snapshot.Upto != 0 || decoded(t, w.Snapshot.Data) != "the profile as it was" || len(w.Entries) != 0 {
		t.Fatalf("welcome %s", w.raw)
	}
	ca.expect("presence")
	ca.push("1", "change")
	ca.expect("stored")
	cb.expect("entry")
	_, w = e.connect(p, randBytes(16), 0)
	if w.Snapshot == nil || w.Snapshot.Upto != 0 || fmt.Sprint(w.nrs()) != "[1]" {
		t.Fatalf("welcome of a third device: %s", w.raw)
	}
	// A device that holds entry 1 needs no snapshot.
	_, w = e.connect(p, randBytes(16), 1)
	if w.Snapshot != nil || len(w.Entries) != 0 {
		t.Fatalf("welcome since 1: %s", w.raw)
	}
}

// A since beyond the head (Phloem was restored from an older backup) is answered as since 0.
func TestSinceBeyondTheHeadGetsEverything(t *testing.T) {
	e := newEnv(t, nil)
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	for i := 1; i <= 3; i++ {
		ca.push(fmt.Sprint(i), "x")
		ca.expect("stored")
	}
	_, w := e.connect(p, randBytes(16), 99)
	if w.Snapshot != nil || fmt.Sprint(w.nrs()) != "[1 2 3]" || w.Head != 3 {
		t.Fatalf("since 99 without a snapshot: %s", w.raw)
	}
	ca.expect("presence")
	ca.snapshot(2, "as of 2")
	ca.expect("snapshotted")
	_, w = e.connect(p, randBytes(16), 99)
	if w.Snapshot == nil || w.Snapshot.Upto != 2 || fmt.Sprint(w.nrs()) != "[3]" {
		t.Fatalf("since 99 with a snapshot: %s", w.raw)
	}
}

// A push sent again after a reconnect (its stored was lost) gets its old number, and nobody
// hears it twice.
func TestAPushSentAgainKeepsItsNumber(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	cb, _ := e.connect(p, b, 0)
	ca.expect("presence")

	ca.push("x", "first")
	ca.expect("stored")
	cb.expect("entry")
	ca.push("x", "first")
	if f := ca.expect("stored"); f.Nr != 1 || f.ID != "x" {
		t.Fatalf("stored again: %s", f.raw)
	}
	ca.push("y", "second")
	ca.expect("stored")
	if f := cb.expect("entry"); f.Nr != 2 {
		t.Fatalf("b heard the repeat: %s", f.raw)
	}
	// Another device may use the same id.
	cb.push("x", "b's own")
	if f := cb.expect("stored"); f.Nr != 3 {
		t.Fatalf("b's x: %s", f.raw)
	}
	ca.expect("entry")
	// After a reconnect, the outbox goes again.
	_ = ca.conn.CloseNow()
	cb.expect("presence")
	ca, _ = e.connect(p, a, 1)
	cb.expect("presence")
	ca.push("y", "second")
	if f := ca.expect("stored"); f.Nr != 2 {
		t.Fatalf("y after the reconnect: %s", f.raw)
	}
	cb.quiet(30 * time.Millisecond)
	// Ids count among the entries after the snapshot only.
	ca.snapshot(3, "all")
	ca.expect("snapshotted")
	ca.push("x", "new")
	if f := ca.expect("stored"); f.Nr != 4 {
		t.Fatalf("x after the snapshot: %s", f.raw)
	}
	cb.expect("entry")
}

func TestSlotsAreKeptAndAFullSpaceSaysSo(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxDevices = 2 })
	p, a, b, c := newProfile(), randBytes(16), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	cb, w := e.connect(p, b, 0)
	if w.Slot != 2 {
		t.Fatalf("welcome %s", w.raw)
	}
	ca.expect("presence")
	cc, f := e.open(p, c, 0)
	if f.T != "error" || f.Code != "full" {
		t.Fatalf("a third device of two: %s", f.raw)
	}
	if code, _ := cc.closed(); code != websocket.StatusPolicyViolation {
		t.Errorf("closed with %d", code)
	}
	// „Nur dieses Gerät trennen": b's slot is free.
	cb.send(`{"t":"leave"}`)
	if code, reason := cb.closed(); code != websocket.StatusNormalClosure || reason != "left" {
		t.Errorf("leave closed with %d %q", code, reason)
	}
	if f := ca.expect("presence"); fmt.Sprint(f.Devices) != "[1]" {
		t.Fatalf("presence %s", f.raw)
	}
	_, w = e.connect(p, c, 0)
	if w.Slot != 2 {
		t.Fatalf("c gets the freed slot: %s", w.raw)
	}
	ca.expect("presence")
	// a keeps its slot on every hello.
	_, w = e.connect(p, a, 0)
	if w.Slot != 1 {
		t.Fatalf("a's slot: %s", w.raw)
	}
}

func TestASecondHelloClosesTheOlderSocket(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	old, _ := e.connect(p, a, 0)
	cb, _ := e.connect(p, b, 0)
	old.expect("presence")
	fresh, w := e.connect(p, a, 0)
	if w.Slot != 1 || fmt.Sprint(w.Devices) != "[1 2]" {
		t.Fatalf("welcome %s", w.raw)
	}
	if code, reason := old.closed(); code != 4000 || reason != "replaced" {
		t.Errorf("the older socket closed with %d %q", code, reason)
	}
	cb.quiet(50 * time.Millisecond) // a stayed online: no presence
	cb.push("b", "data")
	cb.expect("stored")
	if f := fresh.expect("entry"); f.Nr != 1 {
		t.Fatalf("entry %s", f.raw)
	}
}

func TestDeletingTheSpaceTellsEveryDevice(t *testing.T) {
	for _, via := range []string{"http", "socket"} {
		t.Run(via, func(t *testing.T) {
			e := newEnv(t, nil)
			p, a, b := newProfile(), randBytes(16), randBytes(16)
			e.mustCreate(p, a)
			ca, _ := e.connect(p, a, 0)
			cb, _ := e.connect(p, b, 0)
			ca.expect("presence")
			if via == "http" {
				if status, doc, _ := e.request(http.MethodDelete, "/sync/v1/space", p.headers(), nil); status != http.StatusNoContent {
					t.Fatalf("delete: %d %v", status, doc)
				}
			} else {
				ca.send(`{"t":"delete"}`)
			}
			for _, c := range []*client{ca, cb} {
				c.expect("gone")
				if code, reason := c.closed(); code != websocket.StatusNormalClosure || reason != "gone" {
					t.Errorf("closed with %d %q", code, reason)
				}
			}
			c, f := e.open(p, a, 0)
			if f.T != "error" || f.Code != "unauthorized" {
				t.Fatalf("hello after the delete: %s", f.raw)
			}
			c.closed()
			if status, _, _ := e.request(http.MethodDelete, "/sync/v1/space", p.headers(), nil); status != http.StatusUnauthorized {
				t.Errorf("a second delete: %d", status)
			}
			if n, _ := e.st.Totals(); n != 0 {
				t.Errorf("%d spaces left", n)
			}
			// The same space can be made again, and gets a room of its own.
			e.mustCreate(p, a)
			_, w := e.connect(p, a, 0)
			if w.Head != 0 || w.Slot != 1 {
				t.Fatalf("welcome of the new space: %s", w.raw)
			}
		})
	}
}

// The devices of a space online hear when one of its parcels is taken; nobody else does, and
// nothing else of a parcel is told.
func TestTheDevicesOnlineHearTheirParcelTaken(t *testing.T) {
	e := newEnv(t, nil)
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	cb, _ := e.connect(p, b, 0)
	ca.expect("presence")
	other, d := newProfile(), randBytes(16)
	e.mustCreate(other, d)
	co, _ := e.connect(other, d, 0)
	id := randBytes(16)
	if status, doc := e.leavePair(p, id, "the keys, sealed", 0); status != http.StatusCreated {
		t.Fatalf("leave: %d %v", status, doc)
	}
	if status, _ := e.takePair(id); status != http.StatusOK {
		t.Fatalf("take: %d", status)
	}
	for _, c := range []*client{ca, cb} {
		if f := c.expect("pair"); f.ID != b64s(id) || f.State != "taken" {
			t.Fatalf("pair %s", f.raw)
		}
	}
	co.quiet(30 * time.Millisecond)
	// Withdrawn, expired, or asked for again: nothing to hear.
	gone := randBytes(16)
	e.leavePair(p, gone, "x", 0)
	hdr := p.headers()
	hdr["Betula-Pair"] = b64s(gone)
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/pair", hdr, nil); status != http.StatusNoContent {
		t.Fatal(status)
	}
	late := randBytes(16)
	e.leavePair(p, late, "x", 60)
	e.clock.Advance(2 * time.Minute)
	for _, x := range [][]byte{id, gone, late} {
		if status, _ := e.takePair(x); status != http.StatusNotFound {
			t.Fatalf("take: %d", status)
		}
	}
	ca.quiet(30 * time.Millisecond)
	// A space without sockets: nobody to tell, and the parcel is taken all the same.
	_ = ca.conn.CloseNow()
	_ = cb.conn.CloseNow()
	eventually(t, "the space has no sockets", func() bool {
		e.srv.mu.Lock()
		defer e.srv.mu.Unlock()
		return e.srv.rooms[string(p.space)] == nil
	})
	last := randBytes(16)
	e.leavePair(p, last, "x", 0)
	if status, doc := e.takePair(last); status != http.StatusOK || decoded(t, doc["box"].(string)) != "x" {
		t.Fatalf("take without sockets: %d %v", status, doc)
	}
}

func TestHelloIsCheckedFirst(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.HelloTimeout = 200 * time.Millisecond })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	wrong := profile{p.space, randBytes(32)}
	for name, first := range map[string]any{
		"wrong token":   hello(wrong, a, 0),
		"missing space": hello(newProfile(), a, 0),
	} {
		c := watch(t, e.mustDial(nil))
		c.send(first)
		if f := c.expect("error"); f.Code != "unauthorized" {
			t.Errorf("%s: %s", name, f.raw)
		}
		if code, _ := c.closed(); code != websocket.StatusPolicyViolation {
			t.Errorf("%s: closed with %d", name, code)
		}
	}
	for name, first := range map[string]any{
		"push first":    `{"t":"push","id":"x","data":""}`,
		"not json":      `hello`,
		"short space":   `{"t":"hello","space":"AAAA","token":"` + b64s(p.token) + `","device":"` + b64s(a) + `"}`,
		"padded base64": `{"t":"hello","space":"` + b64s(p.space) + `==","token":"` + b64s(p.token) + `","device":"` + b64s(a) + `"}`,
		"negative":      hello(p, a, -1),
		"too long":      `{"t":"hello","pad":"` + strings.Repeat("x", 8<<10) + `"}`,
	} {
		c := watch(t, e.mustDial(nil))
		c.send(first)
		if code, _ := c.closed(); code != websocket.StatusPolicyViolation && code != websocket.StatusMessageTooBig {
			t.Errorf("%s: closed with %d", name, code)
		}
	}
	c := watch(t, e.mustDial(nil))
	if code, _ := c.closed(); code != -1 && code != websocket.StatusPolicyViolation {
		t.Errorf("no hello: closed with %d", code)
	}
	// After the hello, a frame against the protocol ends the socket too.
	for name, bad := range map[string]string{
		"second hello": `{"t":"hello"}`,
		"unknown":      `{"t":"dance"}`,
		"no id":        `{"t":"push","data":""}`,
		"long id":      `{"t":"push","id":"` + strings.Repeat("i", 65) + `","data":""}`,
		"bad data":     `{"t":"push","id":"x","data":"!!"}`,
		"no upto":      `{"t":"snapshot","data":""}`,
		"ahead":        `{"t":"snapshot","upto":7,"data":""}`,
	} {
		c, _ := e.connect(p, a, 0)
		c.send(bad)
		if code, _ := c.closed(); code != websocket.StatusPolicyViolation {
			t.Errorf("%s: closed with %d", name, code)
		}
	}
	c, _ = e.connect(p, a, 0)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	_ = c.conn.Write(ctx, websocket.MessageBinary, []byte{1, 2, 3})
	if code, _ := c.closed(); code != websocket.StatusPolicyViolation && code != -1 {
		t.Errorf("a binary frame: closed with %d", code)
	}
}

func TestSizesArePhloemsOwn(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxEntry, o.Limits.MaxSnapshot = 1000, 5000 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	c.push("big", strings.Repeat("x", 1001))
	if f := c.expect("error"); f.Code != "too_big" || f.ID != "big" {
		t.Fatalf("error %s", f.raw)
	}
	c.push("fits", strings.Repeat("x", 1000)) // the limit is on the decoded bytes
	c.expect("stored")
	c.snapshot(1, strings.Repeat("s", 5001))
	if f := c.expect("error"); f.Code != "too_big" || f.ID != "" {
		t.Fatalf("error %s", f.raw)
	}
	c.snapshot(1, strings.Repeat("s", 5000))
	c.expect("snapshotted")
	// A frame beyond the read limit (the largest snapshot in base64url, and room for JSON)
	// closes the socket.
	limit := e.srv.lim.readLimit()
	c.send(`{"t":"push","id":"huge","data":"` + strings.Repeat("A", int(limit)) + `"}`)
	if code, _ := c.closed(); code != websocket.StatusMessageTooBig {
		t.Errorf("a frame beyond the limit: closed with %d", code)
	}
	if limit != int64(b64len(5000))+512<<10 {
		t.Errorf("read limit %d", limit)
	}
	if d := DefaultLimits.readLimit(); d < 8<<20+1<<10 || d > 9<<20 {
		t.Errorf("the default read limit %d does not fit a 6 MiB snapshot in base64url", d)
	}
}

func TestPushesSlowDown(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PushRate, o.Limits.PushBurst = 1, 3 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	for i := 1; i <= 3; i++ {
		c.push(fmt.Sprint(i), "x")
		c.expect("stored")
	}
	c.push("4", "x")
	if f := c.expect("error"); f.Code != "slow_down" || f.ID != "4" {
		t.Fatalf("error %s", f.raw)
	}
	e.clock.Advance(2 * time.Second)
	c.push("4", "x")
	if f := c.expect("stored"); f.Nr != 4 {
		t.Fatalf("stored %s", f.raw)
	}
	// The space's limit holds for every socket of it, and a reconnect does not refill it.
	_ = c.conn.CloseNow()
	c, _ = e.connect(p, a, 4)
	c.push("5", "x")
	c.expect("stored")
	c.push("6", "x")
	if f := c.expect("error"); f.Code != "slow_down" {
		t.Fatalf("error %s", f.raw)
	}
}

func TestExpiryDeletesUntouchedSpaces(t *testing.T) {
	e := newEnv(t, nil)
	old, kept := newProfile(), newProfile()
	a := randBytes(16)
	e.mustCreate(old, a)
	e.mustCreate(kept, a)
	c, _ := e.connect(old, a, 0) // touched now, then never again
	day := 24 * time.Hour
	e.clock.Advance(300 * day)
	k, _ := e.connect(kept, a, 0) // a hello touches
	_ = k.conn.CloseNow()
	e.clock.Advance(96 * day)
	e.srv.sweep()
	c.expect("gone")
	c.closed()
	if _, f := e.open(old, a, 0); f.Code != "unauthorized" {
		t.Errorf("hello to the expired space: %s", f.raw)
	}
	if _, w := e.connect(kept, a, 0); w.Slot != 1 {
		t.Errorf("the kept space: %s", w.raw)
	}
	var buf strings.Builder
	_ = e.srv.reg.WriteText(&buf)
	if !strings.Contains(buf.String(), `phloem_spaces_deleted_total{reason="expired"} 1`) {
		t.Errorf("metrics:\n%s", buf.String())
	}
}

func TestPingsFindDevicesThatWent(t *testing.T) {
	e := newEnv(t, func(o *Options) {
		o.Limits.PingInterval, o.Limits.PingTimeout = 50*time.Millisecond, 100*time.Millisecond
	})
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	// b says hello and reads its welcome, then never reads again: it answers no ping.
	conn := e.mustDial(nil)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	data := []byte(fmt.Sprintf(`{"t":"hello","space":"%s","token":"%s","device":"%s","since":0}`, b64s(p.space), b64s(p.token), b64s(b)))
	if err := conn.Write(ctx, websocket.MessageText, data); err != nil {
		t.Fatal(err)
	}
	if _, _, err := conn.Read(ctx); err != nil {
		t.Fatal(err)
	}
	if f := ca.expect("presence"); fmt.Sprint(f.Devices) != "[1 2]" {
		t.Fatalf("presence %s", f.raw)
	}
	if f := ca.expect("presence"); fmt.Sprint(f.Devices) != "[1]" {
		t.Fatalf("presence after b went silent: %s", f.raw)
	}
	ca.quiet(300 * time.Millisecond) // a answers its pings and stays
}

func TestCloseTellsTheDevicesToComeBack(t *testing.T) {
	e := newEnv(t, nil)
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	done := make(chan struct{})
	go func() {
		_ = e.srv.Close()
		close(done)
	}()
	if code, reason := c.closed(); code != websocket.StatusGoingAway || reason != "restart" {
		t.Errorf("closed with %d %q", code, reason)
	}
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("Close did not return")
	}
	if _, resp, err := e.dial(nil); err == nil || resp == nil || resp.StatusCode != http.StatusServiceUnavailable {
		t.Errorf("a socket after Close: %v", err)
	}
}

func TestSocketsAreCounted(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxConns, o.Limits.MaxWaiting = 3, 1 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	addr := func(ip string) http.Header { return http.Header{"X-Real-Ip": {ip}} }
	waiting := e.mustDial(addr("203.0.113.1")) // no hello yet
	if _, resp, err := e.dial(addr("203.0.113.1")); err == nil || resp.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("a second socket of an address without hello: %v", err)
	}
	other := e.mustDial(addr("203.0.113.2"))
	c := watch(t, other)
	c.send(hello(p, a, 0))
	c.expect("welcome")
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := waiting.Write(ctx, websocket.MessageText, []byte(`{"t":"hello"}`)); err != nil {
		t.Fatal(err)
	}
	if code, _ := watch(t, waiting).closed(); code != websocket.StatusPolicyViolation {
		t.Fatalf("an invalid hello: closed with %d", code)
	}
	eventually(t, "the socket without a valid hello is gone", func() bool {
		e.srv.mu.Lock()
		defer e.srv.mu.Unlock()
		return e.srv.conns == 1 && len(e.srv.waiting) == 0
	})
	e.mustDial(addr("203.0.113.1"))
	e.mustDial(addr("203.0.113.3"))
	if _, resp, err := e.dial(addr("203.0.113.4")); err == nil || resp.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("a fourth socket of three: %v", err)
	}
}

// Many devices push at once: every device learns every number once, in order, without a gap.
func TestConcurrentPushesStayInOrder(t *testing.T) {
	e := newEnv(t, nil)
	p := newProfile()
	devices := [][]byte{randBytes(16), randBytes(16), randBytes(16)}
	e.mustCreate(p, devices[0])
	clients := make([]*client, len(devices))
	for i, d := range devices {
		clients[i], _ = e.connect(p, d, 0)
		for j := range i { // the earlier ones hear the newcomer
			clients[j].expect("presence")
		}
	}
	const each = 40
	var wg sync.WaitGroup
	for i, c := range clients {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := range each {
				data := fmt.Sprintf(`{"t":"push","id":"%d-%d","data":"%s"}`, i, n, b64s([]byte("x")))
				ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
				_ = c.conn.Write(ctx, websocket.MessageText, []byte(data))
				cancel()
			}
		}()
	}
	wg.Wait()
	total := each * len(clients)
	for i, c := range clients {
		var all, heard []int64
		for len(all) < total {
			f := c.next()
			switch f.T {
			case "stored":
				all = append(all, f.Nr)
			case "entry":
				all = append(all, f.Nr)
				heard = append(heard, f.Nr)
			default:
				t.Fatalf("device %d: %s", i, f.raw)
			}
		}
		if !slices.IsSorted(heard) {
			t.Errorf("device %d heard the entries out of order", i)
		}
		slices.Sort(all)
		for n, nr := range all {
			if nr != int64(n+1) {
				t.Fatalf("device %d: number %d at %d", i, nr, n)
			}
		}
	}
}

// A device that does not read its socket is let go once its queue is full; it catches up when
// it comes back.
func TestASlowDeviceIsLetGo(t *testing.T) {
	e := newEnv(t, func(o *Options) {
		o.Limits.QueueFrames, o.Limits.QueueBytes = 4, 1<<20
		o.Limits.SoftTail, o.Limits.HardTail, o.Limits.PushBurst = 1<<30, 1<<30, 10000
		o.Limits.WriteTimeout = 300 * time.Millisecond
	})
	p, a, b := newProfile(), randBytes(16), randBytes(16)
	e.mustCreate(p, a)
	ca, _ := e.connect(p, a, 0)
	slow := e.mustDial(nil)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	data := []byte(fmt.Sprintf(`{"t":"hello","space":"%s","token":"%s","device":"%s","since":0}`, b64s(p.space), b64s(p.token), b64s(b)))
	if err := slow.Write(ctx, websocket.MessageText, data); err != nil {
		t.Fatal(err)
	}
	if _, _, err := slow.Read(ctx); err != nil {
		t.Fatal(err)
	}
	ca.expect("presence")
	big := strings.Repeat("z", 60<<10)
	for n := range 500 {
		ca.push(fmt.Sprint(n), big)
		for {
			f := ca.next()
			if f.T == "stored" {
				break
			}
			if f.T == "presence" && fmt.Sprint(f.Devices) == "[1]" {
				// Let go; when it comes back it catches up from what it holds.
				_, w := e.connect(p, b, 0)
				if w.Head < int64(n) || len(w.Entries) < n {
					t.Fatalf("the slow device's welcome: head %d, %d entries", w.Head, len(w.Entries))
				}
				return
			}
			t.Fatalf("got %s", f.raw)
		}
	}
	t.Fatal("the slow device was never let go")
}
