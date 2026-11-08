package server

import (
	"fmt"
	"net/http"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/coder/websocket"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

func TestPartsArePutServedAndDeleted(t *testing.T) {
	e := newEnv(t, nil)
	p, q := newProfile(), newProfile()
	e.mustCreate(p, randBytes(16))
	e.mustCreate(q, randBytes(16))
	id := randBytes(16)
	e.mustPutPart(p, id, "sealed calendar", 1)
	// Anyone with the id reads it, without the headers of a space.
	status, doc, hdr := e.getPart(id, "", "")
	if status != http.StatusOK || decoded(t, doc["data"].(string)) != "sealed calendar" || doc["rev"] != float64(1) || hdr.Get("ETag") != `"1"` {
		t.Fatalf("get: %d %v %v", status, doc, hdr)
	}
	// The version named: 304, its ETag, no body.
	status, doc, hdr = e.getPart(id, `"1"`, "")
	if status != http.StatusNotModified || hdr.Get("ETag") != `"1"` || doc != nil {
		t.Fatalf("the version named: %d %v %v", status, doc, hdr)
	}
	e.mustPutPart(p, id, "changed", 2)
	for inm, want := range map[string]int{`"1"`: 200, `"2"`: 304, `W/"2"`: 304, `"1", "2"`: 304, "*": 304, `"3"`: 200, `2`: 200} {
		if status, _, hdr := e.getPart(id, inm, ""); status != want || hdr.Get("ETag") != `"2"` {
			t.Errorf("If-None-Match %s: %d %v, want %d", inm, status, hdr.Get("ETag"), want)
		}
	}
	// Another space deletes nothing of it.
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/part", partHeaders(q, id), nil); status != http.StatusNoContent {
		t.Fatalf("another space's delete: %d", status)
	}
	if status, doc, _ := e.getPart(id, "", ""); status != http.StatusOK || doc["rev"] != float64(2) {
		t.Fatalf("after another space's delete: %d %v", status, doc)
	}
	// „Nicht mehr teilen": gone from now on, for every space.
	for range 2 {
		if status, _, _ := e.request(http.MethodDelete, "/sync/v1/part", partHeaders(p, id), nil); status != http.StatusNoContent {
			t.Fatalf("delete: %d", status)
		}
	}
	for _, inm := range []string{"", `"2"`} {
		if status, doc, _ := e.getPart(id, inm, ""); status != http.StatusGone || doc["error"] != "gone" {
			t.Errorf("a deleted part (%s): %d %v", inm, status, doc)
		}
	}
	for _, who := range []profile{p, q} {
		if status, doc := e.putPart(who, id, "back"); status != http.StatusConflict || doc["error"] != "exists" {
			t.Errorf("a part of a deleted one's id: %d %v", status, doc)
		}
	}
	if status, doc, _ := e.getPart(randBytes(16), "", ""); status != http.StatusNotFound || doc["error"] != "not_found" {
		t.Errorf("an id that never was: %d %v", status, doc)
	}
	// What is not what the contract says.
	for name, c := range map[string]struct {
		method string
		hdr    map[string]string
		body   any
		want   int
	}{
		"get, short id":       {http.MethodGet, map[string]string{"Betula-Part": "AAAA"}, nil, 400},
		"get, no id":          {http.MethodGet, nil, nil, 400},
		"put, padded id":      {http.MethodPut, partHeaders(p, nil), map[string]string{"data": "eA"}, 400},
		"put, no data":        {http.MethodPut, partHeaders(p, randBytes(16)), map[string]string{}, 400},
		"put, data not b64":   {http.MethodPut, partHeaders(p, randBytes(16)), map[string]string{"data": "!!"}, 400},
		"put, not json":       {http.MethodPut, partHeaders(p, randBytes(16)), "data", 400},
		"put, a wrong token":  {http.MethodPut, partHeaders(profile{p.space, randBytes(32)}, randBytes(16)), map[string]string{"data": "eA"}, 401},
		"put, no space":       {http.MethodPut, map[string]string{"Betula-Part": b64s(randBytes(16))}, map[string]string{"data": "eA"}, 401},
		"delete, short id":    {http.MethodDelete, partHeaders(p, randBytes(15)), nil, 400},
		"delete, no token":    {http.MethodDelete, map[string]string{"Betula-Space": b64s(p.space), "Betula-Part": b64s(randBytes(16))}, nil, 401},
		"delete, wrong token": {http.MethodDelete, partHeaders(profile{p.space, randBytes(32)}, randBytes(16)), nil, 401},
	} {
		if name == "put, padded id" {
			c.hdr["Betula-Part"] = b64s(randBytes(16)) + "=="
		}
		if status, doc, _ := e.request(c.method, "/sync/v1/part", c.hdr, c.body); status != c.want {
			t.Errorf("%s: %d %v, want %d", name, status, doc, c.want)
		}
	}
	var buf strings.Builder
	_ = e.srv.reg.WriteText(&buf)
	for _, want := range []string{
		`phloem_parts_total{event="put"} 2`,
		`phloem_parts_total{event="deleted"} 1`,
		`phloem_parts_total{event="gone"} 2`,
		`phloem_parts_total{event="missed"} 1`,
		`phloem_parts_total{event="not_modified"} 5`,
		`phloem_parts_total{event="served"} 5`,
		`phloem_rejections_total{reason="exists"} 2`,
	} {
		if !strings.Contains(buf.String(), want) {
			t.Errorf("metrics lack %s:\n%s", want, buf.String())
		}
	}
}

func TestPartsAreLimited(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxParts, o.Limits.MaxPart, o.Limits.PartsPerDay = 2, 100, 3 })
	p, q := newProfile(), newProfile()
	e.mustCreate(p, randBytes(16))
	e.mustCreate(q, randBytes(16))
	one, two, three := randBytes(16), randBytes(16), randBytes(16)
	e.mustPutPart(p, one, "x", 1)
	if status, doc := e.putPart(q, one, "mine now"); status != http.StatusConflict || doc["error"] != "exists" {
		t.Fatalf("another space's part: %d %v", status, doc)
	}
	e.mustPutPart(p, two, "x", 1)
	if status, doc := e.putPart(p, three, "x"); status != http.StatusTooManyRequests || doc["error"] != "too_many" {
		t.Fatalf("a third part of two: %d %v", status, doc)
	}
	// A replacement is no new part; the size is of the decoded bytes.
	e.mustPutPart(p, two, strings.Repeat("y", 100), 2)
	if status, doc := e.putPart(p, two, strings.Repeat("y", 101)); status != http.StatusRequestEntityTooLarge || doc["error"] != "too_big" {
		t.Fatalf("a part too big: %d %v", status, doc)
	}
	if status, doc, _ := e.request(http.MethodPut, "/sync/v1/part", partHeaders(p, two), `{"data":"`+strings.Repeat("A", 2000)+`"}`); status != http.StatusRequestEntityTooLarge {
		t.Fatalf("a body too big: %d %v", status, doc)
	}
	// Three new parts a day, however many are deleted: the one refused for too_many did not
	// count.
	del := func(id []byte) {
		t.Helper()
		if status, _, _ := e.request(http.MethodDelete, "/sync/v1/part", partHeaders(p, id), nil); status != http.StatusNoContent {
			t.Fatal(status)
		}
	}
	del(two)
	e.mustPutPart(p, three, "x", 1)
	del(three)
	four := randBytes(16)
	if status, doc := e.putPart(p, four, "x"); status != http.StatusTooManyRequests || doc["error"] != "slow_down" {
		t.Fatalf("a fourth new part of the day: %d %v", status, doc)
	}
	e.mustPutPart(p, one, "a replacement goes on", 2)
	e.clock.Advance(8 * time.Hour)
	e.mustPutPart(p, four, "x", 1)
	// Another space has its own day.
	e.mustPutPart(q, randBytes(16), "x", 1)
}

func TestTheBudgetStopsNewPartsNotTheirReplacements(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Budget = 300 })
	p := newProfile()
	e.mustCreate(p, randBytes(16))
	one := randBytes(16)
	e.mustPutPart(p, one, strings.Repeat("x", 200), 1) // 64 + 200 + 64: the budget is reached
	if status, doc := e.putPart(p, randBytes(16), "y"); status != http.StatusInsufficientStorage || doc["error"] != "budget" {
		t.Fatalf("a new part beyond the budget: %d %v", status, doc)
	}
	e.mustPutPart(p, one, "smaller", 2)
	e.mustPutPart(p, one, strings.Repeat("z", 250), 3)
}

func TestPartsAndWatchesCountAsPushes(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PushRate, o.Limits.PushBurst = 1, 3 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	id := randBytes(16)
	e.mustPutPart(p, id, "1", 1)
	e.mustPutPart(p, id, "2", 2)
	c.push("x", "data")
	c.expect("stored")
	if status, doc := e.putPart(p, id, "3"); status != http.StatusTooManyRequests || doc["error"] != "slow_down" {
		t.Fatalf("a fourth push: %d %v", status, doc)
	}
	c.watch(id)
	if f := c.expect("error"); f.Code != "slow_down" || f.ID != "" {
		t.Fatalf("a watch beyond the pushes: %s", f.raw)
	}
	e.clock.Advance(time.Second)
	c.watch(id)
	if f := c.expect("watching"); f.watched() != "2" {
		t.Fatalf("watching %s", f.raw)
	}
	e.clock.Advance(time.Second)
	e.mustPutPart(p, id, "3", 3)
	if f := c.expect("part"); f.Rev != 3 {
		t.Fatalf("part %s", f.raw)
	}
}

func TestPartReadsAreLimitedByAddress(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PartGetsPerAddr = 2 })
	p := newProfile()
	e.mustCreate(p, randBytes(16))
	id := randBytes(16)
	e.mustPutPart(p, id, "x", 1)
	var got []int
	for _, part := range [][]byte{id, randBytes(16), id} {
		status, _, _ := e.getPart(part, "", "198.51.100.7")
		got = append(got, status)
	}
	if fmt.Sprint(got) != "[200 404 429]" {
		t.Fatalf("reads of one address, found or not: %v", got)
	}
	if status, _, _ := e.getPart(id, "", "198.51.100.8"); status != http.StatusOK {
		t.Fatalf("another address: %d", status)
	}
}

func TestPartsCountTowardTheBudget(t *testing.T) {
	e := newEnv(t, nil)
	p := newProfile()
	e.mustCreate(p, randBytes(16))
	bytes := func() int64 {
		_, b := e.st.Totals()
		return b
	}
	base := bytes()
	id := randBytes(16)
	e.mustPutPart(p, id, strings.Repeat("x", 100), 1)
	if got := bytes() - base; got != 100+store.RowBytes {
		t.Errorf("a part costs %d", got)
	}
	e.mustPutPart(p, id, strings.Repeat("x", 30), 2)
	if got := bytes() - base; got != 30+store.RowBytes {
		t.Errorf("a smaller replacement: %d", got)
	}
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/part", partHeaders(p, id), nil); status != http.StatusNoContent {
		t.Fatal(status)
	}
	if got := bytes() - base; got != store.RowBytes {
		t.Errorf("a deleted part's tombstone: %d", got)
	}
	e.mustPutPart(p, randBytes(16), "y", 1)
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/space", p.headers(), nil); status != http.StatusNoContent {
		t.Fatal(status)
	}
	// The tombstones outlive the space, and stay in the budget.
	if n, b := e.st.Totals(); n != 0 || b != 2*store.RowBytes {
		t.Errorf("after the space went: %d spaces, %d bytes", n, b)
	}
	if st := e.srv.Status(); st.Bytes != 2*store.RowBytes {
		t.Errorf("status %+v", st)
	}
}

// Lena's Betula follows a part of Max's profile: it hears every new version and its end, live.
func TestWatchersHearThePartsOfOtherSpaces(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	md, ld := randBytes(16), randBytes(16)
	e.mustCreate(max, md)
	e.mustCreate(lena, ld)
	part, later := randBytes(16), randBytes(16)
	e.mustPutPart(max, part, "v1", 1)
	cl, _ := e.connect(lena, ld, 0)
	cl.watch(part, later, part)
	f := cl.expect("watching")
	if f.watched() != "1 0 1" || f.Parts[0].ID != b64s(part) || f.Parts[1].ID != b64s(later) || f.Parts[2].ID != b64s(part) {
		t.Fatalf("watching %s", f.raw)
	}
	e.mustPutPart(max, part, "v2", 2)
	if f := cl.expect("part"); f.ID != b64s(part) || f.Rev != 2 || f.Gone {
		t.Fatalf("part %s", f.raw)
	}
	cl.quiet(50 * time.Millisecond) // once, though named twice
	// Max's own devices hear nothing of a part they do not watch.
	cm, _ := e.connect(max, md, 0)
	e.mustPutPart(max, part, "v3", 3)
	if f := cl.expect("part"); f.Rev != 3 {
		t.Fatalf("part %s", f.raw)
	}
	cm.quiet(30 * time.Millisecond)
	// A part made after the watch began: rev 0 then, and its versions from now on.
	e.mustPutPart(max, later, "new", 1)
	if f := cl.expect("part"); f.ID != b64s(later) || f.Rev != 1 {
		t.Fatalf("part %s", f.raw)
	}
	// „Nicht mehr teilen".
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/part", partHeaders(max, part), nil); status != http.StatusNoContent {
		t.Fatal(status)
	}
	if f := cl.expect("part"); f.ID != b64s(part) || !f.Gone || f.Rev != 0 || !strings.Contains(f.raw, `"gone":true`) {
		t.Fatalf("part %s", f.raw)
	}
	cl.watch(part, later)
	if f := cl.expect("watching"); f.watched() != "gone 1" {
		t.Fatalf("watching %s", f.raw)
	}
	// A device may watch its own space's part as well.
	cm.watch(later)
	if f := cm.expect("watching"); f.watched() != "1" {
		t.Fatalf("watching %s", f.raw)
	}
	e.mustPutPart(max, later, "newer", 2)
	for _, c := range []*client{cl, cm} {
		if f := c.expect("part"); f.ID != b64s(later) || f.Rev != 2 {
			t.Fatalf("part %s", f.raw)
		}
	}
}

func TestAWatchReplacesTheOneBefore(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	ld := randBytes(16)
	e.mustCreate(max, randBytes(16))
	e.mustCreate(lena, ld)
	a, b := randBytes(16), randBytes(16)
	e.mustPutPart(max, a, "a", 1)
	e.mustPutPart(max, b, "b", 1)
	cl, _ := e.connect(lena, ld, 0)
	cl.watch(a)
	cl.expect("watching")
	cl.watch(b)
	if f := cl.expect("watching"); f.watched() != "1" || f.Parts[0].ID != b64s(b) {
		t.Fatalf("watching %s", f.raw)
	}
	e.mustPutPart(max, a, "a2", 2)
	cl.quiet(50 * time.Millisecond)
	e.mustPutPart(max, b, "b2", 2)
	if f := cl.expect("part"); f.ID != b64s(b) || f.Rev != 2 {
		t.Fatalf("part %s", f.raw)
	}
	if n := e.srv.watchers.size(); n != 1 {
		t.Errorf("%d parts watched", n)
	}
	cl.watch()
	if f := cl.expect("watching"); len(f.Parts) != 0 || !strings.Contains(f.raw, `"parts":[]`) {
		t.Fatalf("watching nothing: %s", f.raw)
	}
	e.mustPutPart(max, b, "b3", 3)
	cl.quiet(50 * time.Millisecond)
	if n := e.srv.watchers.size(); n != 0 {
		t.Errorf("%d parts watched after a watch of none", n)
	}
}

func TestWatchesEndWithTheirSocket(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	ld := randBytes(16)
	e.mustCreate(max, randBytes(16))
	e.mustCreate(lena, ld)
	a, b := randBytes(16), randBytes(16)
	e.mustPutPart(max, a, "a", 1)
	c1, _ := e.connect(lena, ld, 0)
	c1.watch(a, b)
	c1.expect("watching")
	d2 := randBytes(16)
	c2, _ := e.connect(lena, d2, 0)
	c1.expect("presence")
	c2.watch(a)
	c2.expect("watching")
	if n := e.srv.watchers.size(); n != 2 {
		t.Fatalf("%d parts watched", n)
	}
	_ = c1.conn.CloseNow()
	c2.expect("presence")
	eventually(t, "the closed socket watches nothing", func() bool { return e.srv.watchers.size() == 1 })
	e.mustPutPart(max, a, "a2", 2)
	if f := c2.expect("part"); f.Rev != 2 {
		t.Fatalf("part %s", f.raw)
	}
	// A newer socket of the device takes over (4000): the older one's watches end with it.
	c3, _ := e.connect(lena, d2, 0)
	if code, _ := c2.closed(); code != 4000 {
		t.Fatalf("the older socket closed with %d", code)
	}
	eventually(t, "the replaced socket watches nothing", func() bool { return e.srv.watchers.size() == 0 })
	c3.watch(a)
	c3.expect("watching")
	e.mustPutPart(max, a, "a3", 3)
	if f := c3.expect("part"); f.Rev != 3 {
		t.Fatalf("part %s", f.raw)
	}
}

func TestWatchesAreChecked(t *testing.T) {
	e := newEnv(t, nil)
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	ids := func(n int) []string {
		out := []string{}
		for range n {
			out = append(out, b64s(randBytes(16)))
		}
		return out
	}
	c, _ := e.connect(p, a, 0)
	c.send(map[string]any{"t": "watch", "parts": ids(64)})
	if f := c.expect("watching"); len(f.Parts) != 64 || f.watched() != strings.TrimSpace(strings.Repeat("0 ", 64)) {
		t.Fatalf("a watch of 64: %s", f.raw)
	}
	for name, bad := range map[string]any{
		"65 parts":     map[string]any{"t": "watch", "parts": ids(65)},
		"a short id":   map[string]any{"t": "watch", "parts": []string{b64s(randBytes(15))}},
		"a long id":    map[string]any{"t": "watch", "parts": []string{b64s(randBytes(17))}},
		"padded":       map[string]any{"t": "watch", "parts": []string{b64s(randBytes(16)) + "=="}},
		"not base64":   `{"t":"watch","parts":["!!!!!!!!!!!!!!!!!!!!!!"]}`,
		"no list":      `{"t":"watch","parts":"x"}`,
		"no parts":     `{"t":"watch"}`,
		"null":         `{"t":"watch","parts":null}`,
		"a number":     `{"t":"watch","parts":[1]}`,
		"one bad of 3": map[string]any{"t": "watch", "parts": append(ids(2), b64s(randBytes(15)))},
	} {
		c, _ := e.connect(p, a, 0)
		c.send(bad)
		if code, _ := c.closed(); code != websocket.StatusPolicyViolation {
			t.Errorf("%s: closed with %d", name, code)
		}
	}
	eventually(t, "the sockets that broke the protocol watch nothing", func() bool { return e.srv.watchers.size() == 0 })
}

// A space deleted leaves its parts gone: their watchers hear it, readers get 410, and nobody
// can make them again, the space made anew included.
func TestADeletedSpaceLeavesItsPartsGone(t *testing.T) {
	for _, via := range []string{"http", "socket"} {
		t.Run(via, func(t *testing.T) {
			e := newEnv(t, nil)
			max, lena := newProfile(), newProfile()
			md, ld := randBytes(16), randBytes(16)
			e.mustCreate(max, md)
			e.mustCreate(lena, ld)
			part, other := randBytes(16), randBytes(16)
			e.mustPutPart(max, part, "v1", 1)
			e.mustPutPart(lena, other, "lena's", 1)
			cl, _ := e.connect(lena, ld, 0)
			cl.watch(part, other)
			cl.expect("watching")
			cm, _ := e.connect(max, md, 0)
			cm.watch(part) // its own, from the socket that deletes the space
			cm.expect("watching")
			if via == "http" {
				if status, _, _ := e.request(http.MethodDelete, "/sync/v1/space", max.headers(), nil); status != http.StatusNoContent {
					t.Fatal(status)
				}
			} else {
				cm.send(`{"t":"delete"}`)
			}
			cm.expect("gone")
			if f := cl.expect("part"); f.ID != b64s(part) || !f.Gone {
				t.Fatalf("part %s", f.raw)
			}
			cl.quiet(30 * time.Millisecond) // lena's own part stays
			if status, doc, _ := e.getPart(part, "", ""); status != http.StatusGone || doc["error"] != "gone" {
				t.Errorf("a part of a deleted space: %d %v", status, doc)
			}
			if status, _, _ := e.getPart(other, "", ""); status != http.StatusOK {
				t.Errorf("another space's part: %d", status)
			}
			e.mustCreate(max, md)
			for _, who := range []profile{max, lena} {
				if status, doc := e.putPart(who, part, "back"); status != http.StatusConflict || doc["error"] != "exists" {
					t.Errorf("a part of the deleted space's id: %d %v", status, doc)
				}
			}
			var buf strings.Builder
			_ = e.srv.reg.WriteText(&buf)
			if !strings.Contains(buf.String(), `phloem_parts_total{event="deleted"} 1`) {
				t.Errorf("metrics:\n%s", buf.String())
			}
		})
	}
}

func TestAnExpiredSpaceLeavesItsPartsGone(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	ld := randBytes(16)
	e.mustCreate(max, randBytes(16))
	e.mustCreate(lena, ld)
	part := randBytes(16)
	e.mustPutPart(max, part, "v1", 1) // touched now, then never again
	day := 24 * time.Hour
	e.clock.Advance(300 * day)
	cl, _ := e.connect(lena, ld, 0) // a hello touches
	cl.watch(part)
	if f := cl.expect("watching"); f.watched() != "1" {
		t.Fatalf("watching %s", f.raw)
	}
	e.clock.Advance(96 * day)
	e.srv.sweep()
	if f := cl.expect("part"); f.ID != b64s(part) || !f.Gone {
		t.Fatalf("part %s", f.raw)
	}
	if status, _, _ := e.getPart(part, "", ""); status != http.StatusGone {
		t.Errorf("a part of an expired space: %d", status)
	}
	// 395 days later its id is forgotten.
	e.clock.Advance(395*day + time.Minute)
	e.srv.sweep()
	if status, _, _ := e.getPart(part, "", ""); status != http.StatusNotFound {
		t.Errorf("a part gone 395 days ago: %d", status)
	}
}

// Many writers of a part, a watcher that keeps watching anew, pushes in both spaces, and at last
// the part's space deleted, all at once: the watcher hears the versions in order, never a
// watching older than a part frame before it, and nothing after gone; nothing deadlocks.
func TestWatchersHearPartsInOrderUnderLoad(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PushBurst = 10000 })
	max, lena := newProfile(), newProfile()
	md, ld := randBytes(16), randBytes(16)
	e.mustCreate(max, md)
	e.mustCreate(lena, ld)
	part := randBytes(16)
	e.mustPutPart(max, part, "v1", 1)
	cl, _ := e.connect(lena, ld, 0)
	cm, _ := e.connect(max, md, 0)
	cl.watch(part)
	cl.expect("watching")
	cm.watch(part)
	cm.expect("watching")

	const writers, each, watches = 4, 15, 20
	var wg sync.WaitGroup
	for w := range writers {
		wg.Go(func() {
			for i := range each {
				status, doc, _ := e.request(http.MethodPut, "/sync/v1/part", partHeaders(max, part), map[string]string{"data": b64s([]byte(fmt.Sprint(w, i)))})
				if status != http.StatusOK {
					t.Errorf("put: %d %v", status, doc)
					return
				}
			}
		})
	}
	wg.Go(func() {
		for range watches {
			cl.watch(part)
		}
	})
	wg.Go(func() {
		for i := range 20 {
			cl.push(fmt.Sprint("l", i), "x")
			cm.push(fmt.Sprint("m", i), "y")
		}
	})
	wg.Wait()
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/space", max.headers(), nil); status != http.StatusNoContent {
		t.Fatal(status)
	}

	last, parts, watching := int64(1), 0, 0
	for gone := false; !gone; {
		f := cl.next()
		switch f.T {
		case "part":
			if f.Gone {
				gone = true
				break
			}
			if f.Rev <= last {
				t.Fatalf("part rev %d after %d", f.Rev, last)
			}
			last = f.Rev
			parts++
		case "watching":
			if rev := f.Parts[0].Rev; rev < last {
				t.Fatalf("watching rev %d after part rev %d", rev, last)
			} else {
				last = rev
			}
			watching++
		case "stored":
		default:
			t.Fatalf("got %s", f.raw)
		}
	}
	// A version whose frame crossed a newer one's is not told; the newest always is.
	if last != 1+writers*each || parts == 0 || parts > writers*each || watching != watches {
		t.Errorf("last rev %d, %d part frames, %d watching", last, parts, watching)
	}
	for f := range drain(cl, 50*time.Millisecond) {
		if f.T == "part" {
			t.Errorf("after gone: %s", f.raw)
		}
	}
	// Max's socket hears the versions of its own part, its pushes stored, and gone at last.
	last = 1
	for f := cm.next(); f.T != "gone"; f = cm.next() {
		switch {
		case f.T == "part" && f.Rev > last:
			last = f.Rev
		case f.T != "stored":
			t.Fatalf("max got %s after rev %d", f.raw, last)
		}
	}
	if last != 1+writers*each {
		t.Errorf("max heard up to rev %d", last)
	}
}

// However the frames of writers cross, a socket hears the versions of a part in order: an older
// one is not told, nothing after gone, and a watch anew never answers older than what it was
// told.
func TestASocketHearsTheVersionsOfAPartInOrder(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	ld := randBytes(16)
	e.mustCreate(max, randBytes(16))
	e.mustCreate(lena, ld)
	part, other := randBytes(16), randBytes(16)
	e.mustPutPart(max, part, "v1", 1)
	cl, _ := e.connect(lena, ld, 0)
	cl.watch(part)
	cl.expect("watching")
	e.srv.tellPart(part, 1) // what the watch said already
	e.srv.tellPart(part, 3)
	e.srv.tellPart(part, 2)  // crossed by 3
	e.srv.tellPart(other, 5) // not watched
	if f := cl.expect("part"); f.Rev != 3 {
		t.Fatalf("part %s", f.raw)
	}
	cl.quiet(30 * time.Millisecond)
	// The store still says 1: the watch anew answers what the socket was told.
	cl.watch(part, other)
	if f := cl.expect("watching"); f.watched() != "3 0" {
		t.Fatalf("watching %s", f.raw)
	}
	e.srv.tellPart(part, revGone)
	e.srv.tellPart(part, 4)
	if f := cl.expect("part"); !f.Gone {
		t.Fatalf("part %s", f.raw)
	}
	cl.quiet(30 * time.Millisecond)
	// A part no longer watched is no longer told, and forgotten: watched again, it is new.
	cl.watch(other)
	cl.expect("watching")
	e.srv.tellPart(part, 6)
	cl.quiet(30 * time.Millisecond)
	cl.watch(part)
	if f := cl.expect("watching"); f.watched() != "1" {
		t.Fatalf("watching %s", f.raw)
	}
}

// drain yields the frames c gets until it is quiet for d.
func drain(c *client, d time.Duration) func(yield func(frame) bool) {
	return func(yield func(frame) bool) {
		for {
			select {
			case f, ok := <-c.frames:
				if !ok || !yield(f) {
					return
				}
			case <-time.After(d):
				return
			}
		}
	}
}

func TestTheStatsFileCountsParts(t *testing.T) {
	e := newEnv(t, nil)
	max, lena := newProfile(), newProfile()
	ld := randBytes(16)
	e.mustCreate(max, randBytes(16))
	e.mustCreate(lena, ld)
	part := randBytes(16)
	e.mustPutPart(max, part, "x", 1)
	e.mustPutPart(max, randBytes(16), "y", 1)
	cl, _ := e.connect(lena, ld, 0)
	cl.watch(part, randBytes(16))
	cl.expect("watching")
	e.srv.tick()
	var buf strings.Builder
	_ = e.srv.reg.WriteText(&buf)
	for _, want := range []string{"phloem_parts 2", "phloem_parts_watched 2", "# TYPE phloem_parts_total counter"} {
		if !strings.Contains(buf.String(), want) {
			t.Errorf("metrics lack %q:\n%s", want, buf.String())
		}
	}
}
