package server

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/coder/websocket"
)

// pollDoc is what a GET of a poll session answers.
type pollDoc struct {
	Frames []string `json:"frames"`
	Close  *struct {
		Code   int    `json:"code"`
		Reason string `json:"reason"`
	} `json:"close"`
}

// frameTexts are frames as a poll body carries them: each the JSON text of one frame.
func frameTexts(frames ...any) []string {
	out := []string{}
	for _, f := range frames {
		switch v := f.(type) {
		case string:
			out = append(out, v)
		default:
			data, _ := json.Marshal(v)
			out = append(out, string(data))
		}
	}
	return out
}

// openPoll opens a poll session with the frames; hdr are further headers (Origin, X-Real-Ip).
func (e *env) openPoll(hdr map[string]string, frames ...any) (int, string, map[string]any) {
	e.t.Helper()
	status, doc, _ := e.request(http.MethodPost, "/sync/v1/poll", hdr, map[string]any{"frames": frameTexts(frames...)})
	id, _ := doc["poll"].(string)
	return status, id, doc
}

// poller is a device on the socket over HTTPS: send POSTs frames, get asks once. Started, a
// goroutine asks again and again and hands the frames to the client's channel, as watch does a
// WebSocket's, so that next, expect, closed and quiet work as for a socket.
type poller struct {
	*client
	e   *env
	id  string
	hdr map[string]string
}

func (e *env) poller(id string, hdr map[string]string) *poller {
	h := map[string]string{"Betula-Poll": id}
	for k, v := range hdr {
		h[k] = v
	}
	return &poller{client: &client{t: e.t, frames: make(chan frame, 4096)}, e: e, id: id, hdr: h}
}

// poll opens a session for the device and starts asking.
func (e *env) poll(p profile, device []byte, since int64) *poller {
	e.t.Helper()
	status, id, doc := e.openPoll(nil, hello(p, device, since))
	if status != http.StatusCreated || len(id) != b64len(pollLen) {
		e.t.Fatalf("open: %d %v", status, doc)
	}
	pc := e.poller(id, nil)
	pc.start()
	return pc
}

// send POSTs frames into the session and returns the status.
func (pc *poller) send(frames ...any) int {
	pc.t.Helper()
	status, _, _ := pc.e.request(http.MethodPost, "/sync/v1/poll", pc.hdr, map[string]any{"frames": frameTexts(frames...)})
	return status
}

func (pc *poller) push(id, data string) {
	pc.t.Helper()
	if status := pc.send(map[string]string{"t": "push", "id": id, "data": b64s([]byte(data))}); status != http.StatusNoContent {
		pc.t.Fatalf("push: %d", status)
	}
}

// get asks once; it may run beside the test's goroutine.
func (pc *poller) get() (int, pollDoc, error) {
	req, err := http.NewRequest(http.MethodGet, pc.e.ts.URL+"/sync/v1/poll", nil)
	if err != nil {
		return 0, pollDoc{}, err
	}
	for k, v := range pc.hdr {
		req.Header.Set(k, v)
	}
	resp, err := pc.e.ts.Client().Do(req)
	if err != nil {
		return 0, pollDoc{}, err
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	var doc pollDoc
	if err == nil && resp.StatusCode == http.StatusOK {
		err = json.Unmarshal(data, &doc)
	}
	return resp.StatusCode, doc, err
}

func (pc *poller) mustGet() pollDoc {
	pc.t.Helper()
	status, doc, err := pc.get()
	if status != http.StatusOK || err != nil {
		pc.t.Fatalf("get: %d %v", status, err)
	}
	return doc
}

// start asks in a goroutine until the session ends; its close is the client's status.
func (pc *poller) start() {
	go func() {
		defer close(pc.frames)
		for {
			status, doc, err := pc.get()
			if status != http.StatusOK || err != nil {
				pc.status = -1
				return
			}
			for _, text := range doc.Frames {
				var f frame
				if err := json.Unmarshal([]byte(text), &f); err != nil {
					f.T = "unreadable"
				}
				f.raw = text
				pc.frames <- f
			}
			if doc.Close != nil {
				pc.status, pc.reason = websocket.StatusCode(doc.Close.Code), doc.Close.Reason
				return
			}
		}
	}()
}

// texts are the types of frames, for a message.
func types(doc pollDoc) string {
	var out []string
	for _, text := range doc.Frames {
		var f frame
		_ = json.Unmarshal([]byte(text), &f)
		out = append(out, f.T)
	}
	return strings.Join(out, " ")
}

func (e *env) metric(line string) bool {
	var buf strings.Builder
	_ = e.srv.reg.WriteText(&buf)
	return strings.Contains(buf.String(), line)
}

func TestTheSocketOverHTTPS(t *testing.T) {
	e := newEnv(t, nil)
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	b := e.poll(p, db, 0)
	if w := b.expect("welcome"); w.Slot != 2 || fmt.Sprint(w.Devices) != "[1 2]" || !w.Live {
		t.Fatalf("welcome %s", w.raw)
	}
	if f := a.expect("presence"); fmt.Sprint(f.Devices) != "[1 2]" {
		t.Fatalf("presence %s", f.raw)
	}
	if !e.metric("phloem_poll_sessions 1") || !e.metric("phloem_poll_sessions_opened_total 1") || !e.metric("phloem_sockets 2") {
		t.Error("the numbers do not count the session")
	}

	a.push("a-1", "sealed by a")
	a.expect("stored")
	if f := b.expect("entry"); f.Nr != 1 || f.Slot != 1 || decoded(t, f.Data) != "sealed by a" {
		t.Fatalf("entry %s", f.raw)
	}
	b.push("b-1", "sealed by b")
	if f := b.expect("stored"); f.ID != "b-1" || f.Nr != 2 {
		t.Fatalf("stored %s", f.raw)
	}
	if f := a.expect("entry"); f.Nr != 2 || f.Slot != 2 || decoded(t, f.Data) != "sealed by b" {
		t.Fatalf("entry %s", f.raw)
	}
	// Several frames in one POST go in their order.
	if status := b.send(map[string]string{"t": "push", "id": "b-2", "data": b64s([]byte("2"))},
		map[string]string{"t": "push", "id": "b-3", "data": b64s([]byte("3"))}); status != http.StatusNoContent {
		t.Fatalf("send: %d", status)
	}
	if f, g := b.expect("stored"), b.expect("stored"); f.ID != "b-2" || g.ID != "b-3" || g.Nr != f.Nr+1 {
		t.Fatalf("stored %s %s", f.raw, g.raw)
	}
	a.expect("entry")
	a.expect("entry")

	a.send(map[string]any{"t": "signal", "to": 2, "data": b64s([]byte("offer"))})
	if f := b.expect("signal"); !strings.Contains(f.raw, `"from":1`) || decoded(t, f.Data) != "offer" {
		t.Fatalf("signal %s", f.raw)
	}
	b.send(map[string]any{"t": "signal", "to": 1, "data": b64s([]byte("answer"))})
	if f := a.expect("signal"); !strings.Contains(f.raw, `"from":2`) || decoded(t, f.Data) != "answer" {
		t.Fatalf("signal %s", f.raw)
	}

	// „Nur dieses Gerät trennen" over HTTPS: the close comes through the GET, then the session
	// is gone.
	b.send(`{"t":"leave"}`)
	if code, reason := b.closed(); code != websocket.StatusNormalClosure || reason != "left" {
		t.Fatalf("closed with %d %q", code, reason)
	}
	if f := a.expect("presence"); fmt.Sprint(f.Devices) != "[1]" {
		t.Fatalf("presence %s", f.raw)
	}
	if status, _, _ := b.get(); status != http.StatusNotFound {
		t.Errorf("a GET after the close: %d", status)
	}
	if status, doc, _ := e.request(http.MethodPost, "/sync/v1/poll", b.hdr, map[string]any{"frames": []string{}}); status != http.StatusNotFound || doc["error"] != "gone" {
		t.Errorf("a POST after the close: %d %v", status, doc)
	}
	if !e.metric("phloem_poll_sessions 0") || !e.metric(`phloem_poll_sessions_ended_total{reason="closed"} 1`) {
		t.Error("the numbers do not count the end")
	}
}

func TestAPollWelcomeHoldsTheSnapshot(t *testing.T) {
	e := newEnv(t, nil)
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	a.push("1", "one")
	a.expect("stored")
	a.snapshot(1, "every record as of 1")
	a.expect("snapshotted")
	a.push("2", "two")
	a.expect("stored")
	b := e.poll(p, randBytes(deviceLen), 0)
	w := b.expect("welcome")
	if w.Snapshot == nil || w.Snapshot.Upto != 1 || decoded(t, w.Snapshot.Data) != "every record as of 1" || fmt.Sprint(w.nrs()) != "[2]" {
		t.Fatalf("welcome %s", w.raw)
	}
	// With since, what came after it alone.
	c := e.poll(p, randBytes(deviceLen), 2)
	if w := c.expect("welcome"); w.Snapshot != nil || len(w.Entries) != 0 || w.Head != 2 {
		t.Fatalf("welcome since 2: %s", w.raw)
	}
}

// A device that sends its hello again (its GET's answer was lost) replaces its older session as
// it would its older socket.
func TestANewSessionReplacesTheOlder(t *testing.T) {
	e := newEnv(t, nil)
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	old := e.poll(p, db, 0)
	old.expect("welcome")
	a.expect("presence")
	fresh := e.poll(p, db, 0)
	fresh.expect("welcome")
	if code, reason := old.closed(); code != 4000 || reason != "replaced" {
		t.Fatalf("the older session closed with %d %q", code, reason)
	}
	a.quiet(50 * time.Millisecond) // b stayed online
	a.push("1", "x")
	a.expect("stored")
	fresh.expect("entry")
}

func TestANewerGetEndsTheOlder(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PollWait = 5 * time.Second })
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	status, id, _ := e.openPoll(nil, hello(p, db, 0))
	if status != http.StatusCreated {
		t.Fatal(status)
	}
	b := e.poller(id, nil)
	if doc := b.mustGet(); types(doc) != "welcome" {
		t.Fatalf("got %v", doc.Frames)
	}
	a.expect("presence")
	waiting := func() bool {
		e.srv.pollMu.Lock()
		s := e.srv.polls[string(mustDecode(t, id))]
		e.srv.pollMu.Unlock()
		s.mu.Lock()
		defer s.mu.Unlock()
		return s.getter != nil
	}
	type answer struct {
		doc   pollDoc
		after time.Duration
	}
	get := func() chan answer {
		ch := make(chan answer, 1)
		start := time.Now()
		go func() {
			_, doc, _ := b.get()
			ch <- answer{doc, time.Since(start)}
		}()
		return ch
	}
	older := get()
	eventually(t, "the first GET waits", waiting)
	newer := get()
	select {
	case got := <-older:
		if len(got.doc.Frames) != 0 || got.doc.Close != nil || got.after > 3*time.Second {
			t.Fatalf("the older GET: %+v after %s", got.doc, got.after)
		}
	case <-time.After(4 * time.Second):
		t.Fatal("the older GET still waits")
	}
	eventually(t, "the newer GET waits", waiting)
	a.push("1", "x")
	a.expect("stored")
	if got := <-newer; types(got.doc) != "entry" {
		t.Fatalf("the newer GET: %v", got.doc.Frames)
	}
}

func TestAGetWaitsForAFrameAtMostPollWait(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PollWait = 300 * time.Millisecond })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	_, id, _ := e.openPoll(nil, hello(p, da, 0))
	b := e.poller(id, nil)
	b.mustGet() // the welcome
	start := time.Now()
	doc := b.mustGet()
	if took := time.Since(start); len(doc.Frames) != 0 || doc.Close != nil || took < 250*time.Millisecond || took > 3*time.Second {
		t.Fatalf("got %+v after %s", doc, took)
	}
}

// A session nobody asks anything for ends like a socket that stopped answering; one whose
// device asks stays, whatever Phloem's pings do.
func TestAnIdleSessionEndsAndAnAskingOneStays(t *testing.T) {
	e := newEnv(t, func(o *Options) {
		o.Limits.PollIdle, o.Limits.PollWait = 400*time.Millisecond, 100*time.Millisecond
		o.Limits.PingInterval, o.Limits.PingTimeout = 50*time.Millisecond, 100*time.Millisecond
	})
	p, da, db, dc := newProfile(), randBytes(deviceLen), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	asking := e.poll(p, db, 0) // asks again and again
	asking.expect("welcome")
	a.expect("presence")
	_, id, _ := e.openPoll(nil, hello(p, dc, 0))
	silent := e.poller(id, nil)
	if doc := silent.mustGet(); types(doc) != "welcome" {
		t.Fatalf("got %v", doc.Frames)
	}
	for _, c := range []*client{a, asking.client} {
		if f := c.expect("presence"); fmt.Sprint(f.Devices) != "[1 2 3]" {
			t.Fatalf("presence when the silent one came: %s", f.raw)
		}
	}
	for _, c := range []*client{a, asking.client} {
		if f := c.expect("presence"); fmt.Sprint(f.Devices) != "[1 2]" {
			t.Fatalf("presence after the silent one went: %s", f.raw)
		}
	}
	if status, _, _ := silent.get(); status != http.StatusNotFound {
		t.Errorf("a GET of the idle session: %d", status)
	}
	if !e.metric(`phloem_poll_sessions_ended_total{reason="idle"} 1`) {
		t.Error("the idle end is not counted")
	}
	a.quiet(time.Second) // 20 pings later, the asking one is there
	a.push("1", "x")
	a.expect("stored")
	asking.expect("entry")
}

func TestPollsAndOrigins(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Origins = []string{"https://betula.app"} })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	if status, _, doc := e.openPoll(map[string]string{"Origin": "https://evil.example"}, hello(p, da, 0)); status != http.StatusForbidden || doc["error"] != "origin" {
		t.Fatalf("another origin: %d %v", status, doc)
	}
	status, _, hdr := e.request(http.MethodOptions, "/sync/v1/poll", map[string]string{
		"Origin": "https://betula.app", "Access-Control-Request-Method": "POST", "Access-Control-Request-Headers": "betula-poll, content-type"}, nil)
	if status != http.StatusNoContent || !strings.Contains(hdr.Get("Access-Control-Allow-Headers"), "Betula-Poll") ||
		!strings.Contains(hdr.Get("Access-Control-Allow-Methods"), "POST") {
		t.Fatalf("preflight: %d %v", status, hdr)
	}
	status, id, _ := e.openPoll(map[string]string{"Origin": "https://betula.app"}, hello(p, da, 0))
	if status != http.StatusCreated {
		t.Fatalf("the site's origin: %d", status)
	}
	if status, doc, _ := e.request(http.MethodGet, "/sync/v1/poll", map[string]string{"Origin": "https://evil.example", "Betula-Poll": id}, nil); status != http.StatusForbidden || doc["error"] != "origin" {
		t.Fatalf("a GET of another origin: %d %v", status, doc)
	}
	b := e.poller(id, map[string]string{"Origin": "https://betula.app"})
	status, doc, _ := b.get()
	if status != http.StatusOK || types(doc) != "welcome" {
		t.Fatalf("a GET of the site's origin: %d %v", status, doc)
	}
	if status, _, hdr := e.request(http.MethodPost, "/sync/v1/poll", b.hdr, map[string]any{"frames": []string{}}); status != http.StatusNoContent || hdr.Get("Access-Control-Allow-Origin") != "https://betula.app" {
		t.Fatalf("a POST of the site's origin: %d %v", status, hdr)
	}
}

func TestPollRequestsAreChecked(t *testing.T) {
	e := newEnv(t, nil)
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	for name, body := range map[string]any{
		"no frames":     map[string]any{"frames": []string{}},
		"no list":       map[string]any{},
		"not json":      "frames",
		"push first":    map[string]any{"frames": frameTexts(map[string]string{"t": "push", "id": "x", "data": ""})},
		"a short hello": map[string]any{"frames": frameTexts(`{"t":"hello","space":"AAAA"}`)},
		"not a string":  map[string]any{"frames": []any{hello(p, da, 0)}},
	} {
		if status, doc, _ := e.request(http.MethodPost, "/sync/v1/poll", nil, body); status != http.StatusBadRequest || doc["error"] != "invalid" {
			t.Errorf("%s: %d %v", name, status, doc)
		}
	}
	for _, method := range []string{http.MethodGet, http.MethodPost} {
		if status, doc, _ := e.request(method, "/sync/v1/poll", map[string]string{"Betula-Poll": "x"}, map[string]any{"frames": []string{}}); status != http.StatusBadRequest || doc["error"] != "invalid" {
			t.Errorf("%s with a short id: %d %v", method, status, doc)
		}
		if status, doc, _ := e.request(method, "/sync/v1/poll", map[string]string{"Betula-Poll": b64s(randBytes(pollLen))}, map[string]any{"frames": []string{}}); status != http.StatusNotFound || doc["error"] != "gone" {
			t.Errorf("%s of an unknown session: %d %v", method, status, doc)
		}
	}
	if status, _, hdr := e.request(http.MethodPut, "/sync/v1/poll", nil, nil); status != http.StatusMethodNotAllowed || hdr.Get("Allow") != "GET, POST, OPTIONS" {
		t.Errorf("PUT: %d %v", status, hdr)
	}
	// A hello the socket refuses is answered through the GET, as on a socket.
	wrong := profile{p.space, randBytes(tokenLen)}
	status, id, _ := e.openPoll(nil, hello(wrong, da, 0))
	if status != http.StatusCreated {
		t.Fatalf("a wrong token: %d", status)
	}
	b := e.poller(id, nil)
	b.start()
	if f := b.expect("error"); f.Code != "unauthorized" {
		t.Fatalf("a wrong token: %s", f.raw)
	}
	if code, _ := b.closed(); code != websocket.StatusPolicyViolation {
		t.Fatalf("a wrong token: closed with %d", code)
	}
}

// A frame too big closes the socket behind the session with 1009 as it would any socket; a body
// beyond what a POST carries is refused and does the same.
func TestPollFramesTooBig(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxEntry, o.Limits.MaxSnapshot = 1000, 1000 })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	limit := int(e.srv.lim.readLimit())

	_, id, _ := e.openPoll(nil, hello(p, da, 0))
	b := e.poller(id, nil)
	b.mustGet()
	frame := `{"t":"push","id":"huge","data":"` + strings.Repeat("A", limit) + `"}`
	if status := b.send(frame); status != http.StatusNoContent {
		t.Fatalf("a frame beyond the socket's limit: %d", status)
	}
	if doc := b.mustGet(); doc.Close == nil || doc.Close.Code != int(websocket.StatusMessageTooBig) {
		t.Fatalf("got %+v", doc)
	}

	_, id, _ = e.openPoll(nil, hello(p, da, 0))
	b = e.poller(id, nil)
	b.mustGet()
	body := `{"frames":["` + strings.Repeat("A", limit+pollSlack) + `"]}`
	if status, doc, _ := e.request(http.MethodPost, "/sync/v1/poll", b.hdr, body); status != http.StatusRequestEntityTooLarge || doc["error"] != "too_big" {
		t.Fatalf("a body beyond the limit: %d %v", status, doc)
	}
	if doc := b.mustGet(); doc.Close == nil || doc.Close.Code != int(websocket.StatusMessageTooBig) {
		t.Fatalf("got %+v", doc)
	}
	if status, _, _ := b.get(); status != http.StatusNotFound {
		t.Fatalf("after the close: %d", status)
	}
}

func TestPollsAreAdmittedLikeSockets(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxConns = 2 })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	e.connect(p, da, 0)
	b := e.poll(p, randBytes(deviceLen), 0)
	b.expect("welcome")
	if status, _, doc := e.openPoll(nil, hello(p, randBytes(deviceLen), 0)); status != http.StatusServiceUnavailable || doc["error"] != "busy" {
		t.Fatalf("a third socket of two: %d %v", status, doc)
	}
	if _, resp, err := e.dial(nil); err == nil || resp.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("a WebSocket beside: %v", err)
	}
	if !e.metric(`phloem_rejections_total{reason="busy"} 2`) {
		t.Error("busy is not counted once each")
	}
}

// A device that does not ask for its frames is let go once its queue is full, as a slow
// socket; it comes back with since.
func TestAPollSessionFarBehindIsLetGo(t *testing.T) {
	e := newEnv(t, func(o *Options) {
		o.Limits.QueueFrames, o.Limits.QueueBytes = 4, 1<<20
		o.Limits.SoftTail, o.Limits.HardTail, o.Limits.PushBurst = 1<<30, 1<<30, 10000
	})
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	_, id, _ := e.openPoll(nil, hello(p, db, 0))
	b := e.poller(id, nil)
	b.mustGet()
	a.expect("presence")
	pushed := 0
	for gone := false; !gone; {
		if pushed++; pushed > 20 {
			t.Fatal("the session was never let go")
		}
		a.push(fmt.Sprint(pushed), "x")
		for {
			f := a.next()
			if f.T == "stored" {
				break
			}
			if f.T != "presence" || fmt.Sprint(f.Devices) != "[1]" {
				t.Fatalf("got %s", f.raw)
			}
			gone = true
		}
		if !gone {
			gone = e.metric(`phloem_poll_sessions_ended_total{reason="slow"} 1`)
			if gone {
				a.expect("presence")
			}
		}
	}
	if pushed != 5 { // 4 wait, the fifth is too many
		t.Errorf("let go after %d frames", pushed)
	}
	if doc := b.mustGet(); len(doc.Frames) != 0 || doc.Close == nil || doc.Close.Code != 0 || doc.Close.Reason != "" {
		t.Fatalf("got %+v", doc)
	}
	c := e.poll(p, db, 0)
	if w := c.expect("welcome"); w.Head != int64(pushed) || len(w.Entries) != pushed {
		t.Fatalf("welcome %s", w.raw)
	}
}

func TestCloseTellsPollingDevicesToComeBack(t *testing.T) {
	e := newEnv(t, nil)
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	b := e.poll(p, da, 0)
	b.expect("welcome")
	time.Sleep(50 * time.Millisecond) // its GET waits
	done := make(chan struct{})
	go func() {
		_ = e.srv.Close()
		close(done)
	}()
	if code, reason := b.closed(); code != websocket.StatusGoingAway || reason != "restart" {
		t.Errorf("closed with %d %q", code, reason)
	}
	<-done
	if status, _, doc := e.openPoll(nil, hello(p, da, 0)); status != http.StatusServiceUnavailable || doc["error"] != "busy" {
		t.Errorf("a session after Close: %d %v", status, doc)
	}
}

// Drain, as serve calls it when its HTTP server begins to shut down, ends the GETs that wait
// with the close at once.
func TestDrainEndsTheWaitingGets(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.PollWait = 20 * time.Second })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	_, id, _ := e.openPoll(nil, hello(p, da, 0))
	b := e.poller(id, nil)
	b.mustGet()
	start := time.Now()
	var wg sync.WaitGroup
	wg.Add(1)
	var doc pollDoc
	go func() {
		defer wg.Done()
		_, doc, _ = b.get()
	}()
	time.Sleep(50 * time.Millisecond)
	e.srv.Drain()
	wg.Wait()
	if doc.Close == nil || doc.Close.Code != int(websocket.StatusGoingAway) || time.Since(start) > 5*time.Second {
		t.Fatalf("got %+v after %s", doc, time.Since(start))
	}
}

func TestWriteQuotedIsJSON(t *testing.T) {
	for _, text := range []string{`{"t":"x","id":"a\"b\\c"}`, "plain", "tab\tand\x01", `ü and ✓`, ""} {
		var buf bytes.Buffer
		w := bufio.NewWriter(&buf)
		writeQuoted(w, []byte(text))
		_ = w.Flush()
		var got string
		if err := json.Unmarshal(buf.Bytes(), &got); err != nil || got != text {
			t.Errorf("%q: %s (%v)", text, buf.String(), err)
		}
	}
}

func mustDecode(t *testing.T, s string) []byte {
	t.Helper()
	b, err := enc.DecodeString(s)
	if err != nil {
		t.Fatal(err)
	}
	return b
}
