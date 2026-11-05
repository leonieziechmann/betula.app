package server

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/coder/websocket"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

// clock is the server's clock in a test: it moves only when told.
type clock struct {
	mu sync.Mutex
	t  time.Time
}

func (c *clock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.t
}

func (c *clock) Advance(d time.Duration) {
	c.mu.Lock()
	c.t = c.t.Add(d)
	c.mu.Unlock()
}

// env is one Phloem over HTTP, on a store of its own.
type env struct {
	t     *testing.T
	st    *store.Store
	srv   *Server
	ts    *httptest.Server
	clock *clock
}

func newEnv(t *testing.T, mod func(*Options)) *env {
	t.Helper()
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &clock{t: time.Date(2026, 11, 5, 12, 0, 0, 0, time.UTC)}
	opt := Options{
		Store: st, Version: "test", Now: c.Now,
		// Pings only where a test wants them.
		Limits: Limits{PingInterval: time.Hour, HelloTimeout: 2 * time.Second, WriteTimeout: 5 * time.Second},
	}
	if mod != nil {
		mod(&opt)
	}
	srv, err := New(opt)
	if err != nil {
		t.Fatal(err)
	}
	ts := httptest.NewServer(srv.Handler())
	t.Cleanup(func() {
		_ = srv.Close()
		ts.Close()
		_ = st.Close()
	})
	return &env{t: t, st: st, srv: srv, ts: ts, clock: c}
}

func randBytes(n int) []byte {
	b := make([]byte, n)
	_, _ = rand.Read(b)
	return b
}

func b64s(b []byte) string { return enc.EncodeToString(b) }

// profile is what a profile's key gives a device: its space and its token.
type profile struct{ space, token []byte }

func newProfile() profile { return profile{randBytes(spaceLen), randBytes(tokenLen)} }

func (p profile) headers() map[string]string {
	return map[string]string{"Betula-Space": b64s(p.space), "Authorization": "Bearer " + b64s(p.token)}
}

// request sends one HTTP request; body, when not nil, is sent as JSON (a string as it is).
func (e *env) request(method, path string, hdr map[string]string, body any) (int, map[string]any, http.Header) {
	e.t.Helper()
	var r io.Reader
	switch b := body.(type) {
	case nil:
	case string:
		r = strings.NewReader(b)
	default:
		data, _ := json.Marshal(b)
		r = bytes.NewReader(data)
	}
	req, err := http.NewRequest(method, e.ts.URL+path, r)
	if err != nil {
		e.t.Fatal(err)
	}
	for k, v := range hdr {
		req.Header.Set(k, v)
	}
	resp, err := e.ts.Client().Do(req)
	if err != nil {
		e.t.Fatal(err)
	}
	defer resp.Body.Close()
	data, _ := io.ReadAll(resp.Body)
	var doc map[string]any
	_ = json.Unmarshal(data, &doc)
	return resp.StatusCode, doc, resp.Header
}

// create makes p's space with device in slot 1, from address addr ("": the test's own).
func (e *env) create(p profile, device []byte, addr string) (int, map[string]any) {
	e.t.Helper()
	hdr := map[string]string{}
	if addr != "" {
		hdr["X-Real-Ip"] = addr
	}
	status, doc, _ := e.request(http.MethodPost, "/sync/v1/space", hdr,
		map[string]string{"space": b64s(p.space), "token": b64s(p.token), "device": b64s(device)})
	return status, doc
}

func (e *env) mustCreate(p profile, device []byte) {
	e.t.Helper()
	if status, doc := e.create(p, device, ""); status != http.StatusCreated || doc["slot"] != float64(1) {
		e.t.Fatalf("create: %d %v", status, doc)
	}
}

// frame is every frame Phloem sends.
type frame struct {
	T        string `json:"t"`
	Slot     int    `json:"slot"`
	Head     int64  `json:"head"`
	Now      int64  `json:"now"`
	Devices  []int  `json:"devices"`
	Snapshot *struct {
		Upto int64  `json:"upto"`
		Data string `json:"data"`
	} `json:"snapshot"`
	Entries []struct {
		Nr   int64  `json:"nr"`
		Slot int    `json:"slot"`
		Data string `json:"data"`
	} `json:"entries"`
	ID   string `json:"id"`
	Nr   int64  `json:"nr"`
	Data string `json:"data"`
	Upto int64  `json:"upto"`
	Code string `json:"code"`
	raw  string
}

// nrs are the numbers of a welcome's entries.
func (f frame) nrs() []int64 {
	out := []int64{}
	for _, e := range f.Entries {
		out = append(out, e.Nr)
	}
	return out
}

// client is one device's socket; a goroutine reads it (and so answers pings) into frames.
type client struct {
	t      *testing.T
	conn   *websocket.Conn
	frames chan frame
	status websocket.StatusCode // set before frames closes
	reason string
}

func (e *env) wsURL() string { return "ws" + strings.TrimPrefix(e.ts.URL, "http") + "/sync/v1/ws" }

// dial opens a socket without saying hello.
func (e *env) dial(hdr http.Header) (*websocket.Conn, *http.Response, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	conn, resp, err := websocket.Dial(ctx, e.wsURL(), &websocket.DialOptions{HTTPHeader: hdr})
	if err == nil {
		conn.SetReadLimit(-1)
		e.t.Cleanup(func() { _ = conn.CloseNow() })
	}
	return conn, resp, err
}

func (e *env) mustDial(hdr http.Header) *websocket.Conn {
	e.t.Helper()
	conn, resp, err := e.dial(hdr)
	if err != nil {
		status := 0
		if resp != nil {
			status = resp.StatusCode
		}
		e.t.Fatalf("dial: %v (%d)", err, status)
	}
	return conn
}

func watch(t *testing.T, conn *websocket.Conn) *client {
	c := &client{t: t, conn: conn, frames: make(chan frame, 4096)}
	go func() {
		defer close(c.frames)
		for {
			_, data, err := conn.Read(context.Background())
			if err != nil {
				c.status = websocket.CloseStatus(err)
				var ce websocket.CloseError
				if errors.As(err, &ce) {
					c.reason = ce.Reason
				}
				return
			}
			var f frame
			if err := json.Unmarshal(data, &f); err != nil {
				f.T = "unreadable"
			}
			f.raw = string(data)
			c.frames <- f
		}
	}()
	return c
}

func hello(p profile, device []byte, since int64) map[string]any {
	return map[string]any{"t": "hello", "space": b64s(p.space), "token": b64s(p.token), "device": b64s(device), "since": since}
}

// open dials and says hello, and returns the socket with its first frame.
func (e *env) open(p profile, device []byte, since int64) (*client, frame) {
	e.t.Helper()
	c := watch(e.t, e.mustDial(nil))
	c.send(hello(p, device, since))
	return c, c.next()
}

// connect is open that wants a welcome.
func (e *env) connect(p profile, device []byte, since int64) (*client, frame) {
	e.t.Helper()
	c, f := e.open(p, device, since)
	if f.T != "welcome" {
		e.t.Fatalf("hello answered %s", f.raw)
	}
	return c, f
}

func (c *client) send(v any) {
	c.t.Helper()
	var data []byte
	switch b := v.(type) {
	case string:
		data = []byte(b)
	case []byte:
		data = b
	default:
		data, _ = json.Marshal(v)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := c.conn.Write(ctx, websocket.MessageText, data); err != nil {
		c.t.Fatalf("send: %v", err)
	}
}

func (c *client) push(id, data string) {
	c.t.Helper()
	c.send(map[string]string{"t": "push", "id": id, "data": b64s([]byte(data))})
}

func (c *client) snapshot(upto int64, data string) {
	c.t.Helper()
	c.send(map[string]any{"t": "snapshot", "upto": upto, "data": b64s([]byte(data))})
}

func (c *client) next() frame {
	c.t.Helper()
	select {
	case f, ok := <-c.frames:
		if !ok {
			c.t.Fatalf("the socket closed (%d %q) instead of a frame", c.status, c.reason)
		}
		return f
	case <-time.After(5 * time.Second):
		c.t.Fatal("no frame within 5 s")
	}
	return frame{}
}

// expect wants the next frame to be of type t.
func (c *client) expect(t string) frame {
	c.t.Helper()
	f := c.next()
	if f.T != t {
		c.t.Fatalf("got %s, want a %s", f.raw, t)
	}
	return f
}

// closed wants the socket to close next, and returns its code and reason.
func (c *client) closed() (websocket.StatusCode, string) {
	c.t.Helper()
	for {
		select {
		case f, ok := <-c.frames:
			if !ok {
				return c.status, c.reason
			}
			c.t.Fatalf("got %s, want the close", f.raw)
		case <-time.After(10 * time.Second):
			c.t.Fatal("not closed within 10 s")
		}
	}
}

// quiet wants no frame for d.
func (c *client) quiet(d time.Duration) {
	c.t.Helper()
	select {
	case f, ok := <-c.frames:
		if ok {
			c.t.Fatalf("got %s, want nothing", f.raw)
		}
		c.t.Fatalf("closed (%d %q), want nothing", c.status, c.reason)
	case <-time.After(d):
	}
}

// eventually waits for cond, for up to 5 s.
func eventually(t *testing.T, what string, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for !cond() {
		if time.Now().After(deadline) {
			t.Fatalf("not within 5 s: %s", what)
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func decoded(t *testing.T, s string) string {
	t.Helper()
	b, err := enc.DecodeString(s)
	if err != nil {
		t.Fatalf("not base64url: %q", s)
	}
	return string(b)
}
