package upstream

import (
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"math"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/telemetry"
)

// fakeHost is an upstream server: handle answers, the rest records what arrived.
type fakeHost struct {
	srv    *httptest.Server
	handle http.HandlerFunc

	mu          sync.Mutex
	hits        int
	headers     []http.Header
	urls        []string
	inFlight    int
	maxInFlight int
	starts      []time.Time
	ends        []time.Time
}

func newFakeHost(t *testing.T, handle http.HandlerFunc) *fakeHost {
	t.Helper()
	f := &fakeHost{handle: handle}
	f.srv = httptest.NewServer(http.HandlerFunc(f.serve))
	t.Cleanup(f.srv.Close)
	return f
}

func (f *fakeHost) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	f.hits++
	f.headers = append(f.headers, r.Header.Clone())
	f.urls = append(f.urls, r.URL.String())
	f.inFlight++
	f.maxInFlight = max(f.maxInFlight, f.inFlight)
	f.starts = append(f.starts, time.Now())
	f.mu.Unlock()
	defer func() {
		f.mu.Lock()
		f.inFlight--
		f.ends = append(f.ends, time.Now())
		f.mu.Unlock()
	}()
	f.handle(w, r)
}

func (f *fakeHost) count() int {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.hits
}

func (f *fakeHost) running() int {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.inFlight
}

func (f *fakeHost) lastHeader() http.Header {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.headers[len(f.headers)-1]
}

// testPolicy allows every host, with a floor that does not slow the tests down; edit changes
// the default.
func testPolicy(edit func(*HostPolicy)) *Policy {
	p := &Policy{
		Allow:     []string{"*"},
		UserAgent: "Cortex-Test/1.0",
		Default: HostPolicy{Concurrency: 1, MaxAge: time.Hour, QueueWait: 5 * time.Second, BreakerFailures: 3,
			BreakerPause: time.Minute, Timeout: 5 * time.Second, MaxBody: 1 << 20},
		Hosts: map[string]HostPolicy{},
	}
	if edit != nil {
		edit(&p.Default)
	}
	return p
}

func noProxy(*http.Request) (*url.URL, error) { return nil, nil }

// newTestUpstream opens an Upstream that connects to the test servers on 127.0.0.1 directly.
func newTestUpstream(t *testing.T, opt Options) *Upstream {
	t.Helper()
	opt.AllowPrivate = true
	return openUpstream(t, opt)
}

func openUpstream(t *testing.T, opt Options) *Upstream {
	t.Helper()
	if opt.proxy == nil {
		opt.proxy = noProxy
	}
	u, err := New(opt)
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}
	t.Cleanup(u.Close)
	return u
}

// memSink keeps a body in memory, as the server's sink keeps it in the store.
type memSink struct {
	mu    sync.Mutex
	calls int
	body  []byte
	err   error // what reading the body ended with
}

func (s *memSink) sink(r io.Reader) (string, int64, error) {
	b, err := io.ReadAll(r)
	s.mu.Lock()
	s.calls++
	s.body, s.err = b, err
	s.mu.Unlock()
	if err != nil {
		return "", 0, err
	}
	return hashOf(b), int64(len(b)), nil
}

func (s *memSink) called() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.calls
}

func hashOf(b []byte) string {
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

// discardSink is a sink for tests that do not look at the body.
func discardSink(r io.Reader) (string, int64, error) {
	n, err := io.Copy(io.Discard, r)
	return "", n, err
}

// fakeClock is Options.Now for tests that move time on by hand.
type fakeClock struct {
	mu sync.Mutex
	t  time.Time
}

func newClock() *fakeClock { return &fakeClock{t: time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)} }

func (c *fakeClock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.t
}

func (c *fakeClock) Advance(d time.Duration) {
	c.mu.Lock()
	c.t = c.t.Add(d)
	c.mu.Unlock()
}

// logBuffer collects the JSON log of a test (captureLog).
type logBuffer struct {
	mu  sync.Mutex
	buf bytes.Buffer
}

func (l *logBuffer) Write(p []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.buf.Write(p)
}

// records returns the records of event, in order.
func (l *logBuffer) records(t *testing.T, event string) []map[string]any {
	t.Helper()
	l.mu.Lock()
	defer l.mu.Unlock()
	var out []map[string]any
	for _, line := range strings.Split(strings.TrimSpace(l.buf.String()), "\n") {
		if line == "" {
			continue
		}
		var rec map[string]any
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			t.Fatalf("log line %q: %v", line, err)
		}
		if rec["event"] == event {
			out = append(out, rec)
		}
	}
	return out
}

func captureLog(t *testing.T) *logBuffer {
	t.Helper()
	l := &logBuffer{}
	prev := slog.Default()
	slog.SetDefault(slog.New(slog.NewJSONHandler(l, &slog.HandlerOptions{Level: slog.LevelDebug})))
	t.Cleanup(func() { slog.SetDefault(prev) })
	return l
}

// scrape reads telemetry.Registry into series -> value.
func scrape(t *testing.T) map[string]float64 {
	t.Helper()
	var buf bytes.Buffer
	if err := telemetry.Registry.WriteText(&buf); err != nil {
		t.Fatalf("WriteText failed: %v", err)
	}
	out := make(map[string]float64)
	for _, line := range strings.Split(buf.String(), "\n") {
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		i := strings.LastIndexByte(line, ' ')
		v, err := strconv.ParseFloat(line[i+1:], 64)
		if err != nil {
			t.Fatalf("metrics line %q: %v", line, err)
		}
		out[line[:i]] = v
	}
	return out
}

// waitFor polls cond until it holds, for at most 5 s.
func waitFor(t *testing.T, what string, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for !cond() {
		if time.Now().After(deadline) {
			t.Fatalf("timed out waiting for %s", what)
		}
		time.Sleep(time.Millisecond)
	}
}

func stateOf(u *Upstream, host string) HostState {
	for _, hs := range u.HostStates() {
		if hs.Host == host {
			return hs
		}
	}
	return HostState{Host: host}
}

func TestFetchStreamsAStorableAnswerIntoTheSinkWithTheKeptHeaders(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		h := w.Header()
		h.Set("Content-Type", "text/html; charset=utf-8")
		h.Set("Content-Language", "de")
		h.Set("Content-Disposition", `inline; filename="page.html"`)
		h.Set("Last-Modified", "Wed, 01 Oct 2026 10:00:00 GMT")
		h.Set("ETag", `"v1"`)
		h.Set("Set-Cookie", "session=1")
		h.Set("Cache-Control", "no-store")
		h.Set("Vary", "Accept")
		_, _ = io.WriteString(w, "<p>Modul</p>")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page?id=1&b=%2F#frag", Sink: sink.sink})
	if err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}
	if res.Status != 200 || res.NotModified || res.Size != 12 || res.Hash != hashOf([]byte("<p>Modul</p>")) {
		t.Errorf("got %+v, want a 200 of 12 bytes with its hash", res)
	}
	if res.FinalURL != f.srv.URL+"/page?id=1&b=%2F" {
		t.Errorf("FinalURL %q, want the URL without its fragment", res.FinalURL)
	}
	if got := f.urls[0]; got != "/page?id=1&b=%2F" {
		t.Errorf("upstream was asked for %q, want the path and query byte for byte", got)
	}
	if string(sink.body) != "<p>Modul</p>" || sink.calls != 1 {
		t.Errorf("sink got %q in %d calls", sink.body, sink.calls)
	}
	want := http.Header{
		"Content-Type":        {"text/html; charset=utf-8"},
		"Content-Language":    {"de"},
		"Content-Disposition": {`inline; filename="page.html"`},
		"Last-Modified":       {"Wed, 01 Oct 2026 10:00:00 GMT"},
		"Etag":                {`"v1"`},
	}
	if fmt.Sprint(res.Header) != fmt.Sprint(want) {
		t.Errorf("kept headers %v, want %v", res.Header, want)
	}
}

func TestStorableAndUnstorableStatuses(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		status, _ := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/"))
		w.Header().Set("Content-Type", "text/plain")
		w.WriteHeader(status)
		_, _ = io.WriteString(w, "body")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.BreakerFailures = 100 })})

	for _, tc := range []struct {
		status   int
		storable bool
	}{
		{200, true}, {203, true}, {204, true}, {404, true}, {410, true},
		{201, false}, {202, false}, {206, false}, {300, false}, {302, false}, {400, false},
		{401, false}, {403, false}, {429, false}, {451, false}, {500, false}, {502, false}, {503, false},
	} {
		var sink memSink
		res, err := u.Fetch(context.Background(), Request{URL: fmt.Sprintf("%s/%d", f.srv.URL, tc.status), Sink: sink.sink})
		if tc.storable {
			wantBody := "body"
			if tc.status == 204 {
				wantBody = ""
			}
			if err != nil || res.Status != tc.status || sink.calls != 1 || string(sink.body) != wantBody {
				t.Errorf("%d: got %+v (err %v), sink %d calls with %q; want it stored", tc.status, res, err, sink.calls, sink.body)
			}
			continue
		}
		var se *StatusError
		if !errors.As(err, &se) || se.Status != tc.status {
			t.Errorf("%d: got %+v (err %v), want *StatusError{%d}", tc.status, res, err, tc.status)
		}
		if sink.calls != 0 {
			t.Errorf("%d: the sink was called for an unstorable answer", tc.status)
		}
	}
	if hs := stateOf(u, "127.0.0.1"); hs.PausedUntil != nil {
		t.Errorf("host paused (%+v) by answers without Retry-After and below the breaker", hs)
	}
}

func TestHeadersSentUpstreamAndNeverCookiesOrAuthorization(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/login" {
			http.SetCookie(w, &http.Cookie{Name: "session", Value: "secret", Path: "/"})
			w.Header().Set("WWW-Authenticate", `Basic realm="x"`)
			http.Redirect(w, r, "/page", http.StatusFound)
			return
		}
		_, _ = io.WriteString(w, "ok")
	})
	p := testPolicy(nil)
	u := newTestUpstream(t, Options{Policy: p})

	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", UserAgent: "Betula-Radix/1.0",
		Accept: "text/html", AcceptLanguage: "de-DE", Sink: discardSink}); err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}
	h := f.lastHeader()
	if h.Get("User-Agent") != "Betula-Radix/1.0" || h.Get("Accept") != "text/html" || h.Get("Accept-Language") != "de-DE" {
		t.Errorf("sent %v, want the client's User-Agent, Accept and Accept-Language", h)
	}
	if h.Get("Accept-Encoding") != "gzip" {
		t.Errorf("Accept-Encoding %q, want gzip asked for (Cortex decodes it itself)", h.Get("Accept-Encoding"))
	}

	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", Sink: discardSink}); err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}
	h = f.lastHeader()
	if h.Get("User-Agent") != "Cortex-Test/1.0" || h.Get("Accept") != "" || h.Get("Accept-Language") != "" {
		t.Errorf("sent %v, want the policy's User-Agent and no Accept or Accept-Language", h)
	}

	// A cookie set on the way is not sent on (no jar), and nothing sends Authorization.
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/login", Sink: discardSink}); err != nil {
		t.Fatalf("Fetch through the redirect failed: %v", err)
	}
	for i, h := range f.headers {
		if h.Get("Cookie") != "" || h.Get("Authorization") != "" {
			t.Errorf("request %d carried Cookie %q / Authorization %q", i, h.Get("Cookie"), h.Get("Authorization"))
		}
	}
	for i, h := range f.headers {
		for name := range h {
			switch name {
			case "User-Agent", "Accept", "Accept-Language", "Accept-Encoding":
			default:
				t.Errorf("request %d carried %s", i, name)
			}
		}
	}

	// User info would become an Authorization header: refused before anything is sent.
	hits := f.count()
	withUser := strings.Replace(f.srv.URL, "http://", "http://user:pass@", 1) + "/page"
	if _, err := u.Fetch(context.Background(), Request{URL: withUser, Sink: discardSink}); err == nil || strings.Contains(err.Error(), "pass") {
		t.Errorf("a URL with user info: err %v, want a refusal that does not show the password", err)
	}
	// A header value with a line break is the client's fault, not the host's.
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", UserAgent: "x\r\nCookie: a=b", Sink: discardSink}); err == nil {
		t.Error("a User-Agent with a line break was sent")
	}
	if f.count() != hits {
		t.Errorf("upstream was asked %d more times", f.count()-hits)
	}
}

func TestConditionalRequestYieldsNotModified(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("ETag", `"v1"`)
		if r.URL.Path == "/always-304" || r.Header.Get("If-None-Match") == `"v1"` {
			w.WriteHeader(http.StatusNotModified)
			return
		}
		_, _ = io.WriteString(w, "content")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", IfNoneMatch: `"v1"`,
		IfModifiedSince: "Wed, 01 Oct 2026 10:00:00 GMT", Sink: sink.sink})
	if err != nil || !res.NotModified || res.Status != 304 || res.Header.Get("ETag") != `"v1"` {
		t.Fatalf("got %+v (err %v), want NotModified with upstream's ETag", res, err)
	}
	if sink.calls != 0 {
		t.Errorf("the sink was called for a 304")
	}
	h := f.lastHeader()
	if h.Get("If-None-Match") != `"v1"` || h.Get("If-Modified-Since") != "Wed, 01 Oct 2026 10:00:00 GMT" {
		t.Errorf("sent %v, want both validators", h)
	}

	res, err = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", IfNoneMatch: `"v0"`, Sink: sink.sink})
	if err != nil || res.NotModified || res.Status != 200 || string(sink.body) != "content" {
		t.Errorf("a changed ETag: got %+v (err %v), want the new content", res, err)
	}

	// A 304 that was not asked for cannot be stored.
	_, err = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/always-304", Sink: sink.sink})
	var se *StatusError
	if !errors.As(err, &se) || se.Status != 304 {
		t.Errorf("an unasked 304: err %v, want *StatusError{304}", err)
	}
}

func TestExpectTypeMismatchNeverCallsTheSink(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/challenge":
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			_, _ = io.WriteString(w, "<html>not a bot?</html>")
		case "/untyped":
			w.Header()["Content-Type"] = nil // no sniffing either
			_, _ = io.WriteString(w, "%PDF-1.7")
		case "/missing":
			w.Header().Set("Content-Type", "text/html")
			w.WriteHeader(http.StatusNotFound)
			_, _ = io.WriteString(w, "<html>gone</html>")
		default:
			w.Header().Set("Content-Type", "Application/PDF; qs=0.9")
			_, _ = io.WriteString(w, "%PDF-1.7")
		}
	})
	p := testPolicy(nil)
	pdf := p.Default
	pdf.ExpectType = "application/pdf"
	p.Hosts["127.0.0.1"] = pdf
	u := newTestUpstream(t, Options{Policy: p})

	for _, path := range []string{"/challenge", "/untyped"} {
		var sink memSink
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + path, Sink: sink.sink})
		if !errors.Is(err, ErrWrongType) {
			t.Errorf("%s: err %v, want ErrWrongType", path, err)
		}
		if sink.calls != 0 {
			t.Errorf("%s: the sink was called", path)
		}
	}
	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/challenge", Sink: discardSink})
	if err == nil || !strings.Contains(err.Error(), "text/html") {
		t.Errorf("err %v, want it to name the type upstream sent", err)
	}

	var sink memSink
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/statute.pdf", Sink: sink.sink}); err != nil || res.Status != 200 || string(sink.body) != "%PDF-1.7" {
		t.Errorf("a PDF: got %+v (err %v), want it stored", res, err)
	}
	// Only a 2xx must be of the type: a 404 is the statement that there is nothing.
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/missing", Sink: sink.sink}); err != nil || res.Status != 404 {
		t.Errorf("a 404: got %+v (err %v), want it stored", res, err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 0 {
		t.Errorf("a wrong type counted as a failure of the host: %+v", hs)
	}
}

func TestMaxBodyIsEnforcedAndTheSinkNeverSeesACompleteBody(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		n, _ := strconv.Atoi(r.URL.Query().Get("n"))
		if r.URL.Path == "/announced" {
			w.Header().Set("Content-Length", strconv.Itoa(n))
		}
		for i := 0; i < n; i++ {
			_, _ = w.Write([]byte{'x'})
			if r.URL.Path == "/chunked" {
				w.(http.Flusher).Flush()
			}
		}
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.MaxBody = 10 })})

	var sink memSink
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/chunked?n=100", Sink: sink.sink}); !errors.Is(err, ErrTooLarge) {
		t.Errorf("100 bytes, chunked: err %v, want ErrTooLarge", err)
	}
	if sink.calls != 1 || len(sink.body) > 10 || !errors.Is(sink.err, ErrTooLarge) {
		t.Errorf("sink read %d bytes ending in %v, want at most 10 and ErrTooLarge instead of the end", len(sink.body), sink.err)
	}

	sink = memSink{}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/chunked?n=11", Sink: sink.sink}); !errors.Is(err, ErrTooLarge) {
		t.Errorf("11 bytes: err %v, want ErrTooLarge", err)
	}
	if len(sink.body) != 10 || sink.err == nil {
		t.Errorf("sink read %d bytes ending in %v, want 10 and an error", len(sink.body), sink.err)
	}

	sink = memSink{}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/announced?n=100", Sink: sink.sink}); !errors.Is(err, ErrTooLarge) {
		t.Errorf("100 bytes announced: err %v, want ErrTooLarge", err)
	}
	if sink.calls != 0 {
		t.Errorf("the sink was called for a body announced too large")
	}

	sink = memSink{}
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/chunked?n=10", Sink: sink.sink}); err != nil || res.Size != 10 {
		t.Errorf("exactly max_body: got %+v (err %v), want it stored", res, err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 0 {
		t.Errorf("a body too large counted as a failure of the host: %+v", hs)
	}
}

func TestSinkErrorsArePassedThrough(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "content") })
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	errDiskFull := errors.New("disk full")
	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: func(r io.Reader) (string, int64, error) {
		_, _ = io.Copy(io.Discard, r)
		return "", 0, errDiskFull
	}})
	if !errors.Is(err, errDiskFull) {
		t.Errorf("err %v, want the sink's error", err)
	}

	// A sink that stops before the end would store a part as the whole.
	_, err = u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: func(r io.Reader) (string, int64, error) {
		n, err := io.CopyN(io.Discard, r, 3)
		return "", n, err
	}})
	if err == nil {
		t.Error("a sink that read 3 of 7 bytes passed")
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 0 {
		t.Errorf("a failing sink counted as a failure of the host: %+v", hs)
	}
}

func TestATruncatedBodyIsAFailureOfTheHost(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Length", "100")
		_, _ = io.WriteString(w, "only ten b")
		w.(http.Flusher).Flush()
		conn, _, err := http.NewResponseController(w).Hijack()
		if err == nil {
			_ = conn.Close()
		}
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	var sink memSink
	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: sink.sink})
	if err == nil || errors.Is(err, ErrTooLarge) {
		t.Fatalf("err %v, want a read error", err)
	}
	if !errors.Is(sink.err, io.ErrUnexpectedEOF) {
		t.Errorf("the sink's read ended in %v, want io.ErrUnexpectedEOF", sink.err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 1 {
		t.Errorf("got %+v, want one failure in a row", hs)
	}
}

func TestRedirectsAreFollowedWithinTheAllowList(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/a":
			http.Redirect(w, r, "/b", http.StatusFound)
		case r.URL.Path == "/b":
			w.Header().Set("Location", "c?x=1")
			w.WriteHeader(http.StatusMovedPermanently)
		case r.URL.Path == "/c":
			_, _ = io.WriteString(w, "end")
		case r.URL.Path == "/away":
			http.Redirect(w, r, "http://elsewhere.example/x", http.StatusTemporaryRedirect)
		case r.URL.Path == "/ftp":
			http.Redirect(w, r, "ftp://files.example/x", http.StatusFound)
		case strings.HasPrefix(r.URL.Path, "/loop/"):
			n, _ := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/loop/"))
			if n == 0 {
				_, _ = io.WriteString(w, "landed")
				return
			}
			http.Redirect(w, r, fmt.Sprintf("/loop/%d", n-1), http.StatusSeeOther)
		}
	})
	p := testPolicy(nil)
	p.Allow = []string{"127.0.0.1"}
	u := newTestUpstream(t, Options{Policy: p})

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/a", Sink: sink.sink})
	if err != nil || res.FinalURL != f.srv.URL+"/c?x=1" || string(sink.body) != "end" {
		t.Errorf("got %+v (err %v), want the end of the chain", res, err)
	}

	hits := f.count()
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/away", Sink: discardSink}); !errors.Is(err, ErrHostNotAllowed) {
		t.Errorf("a redirect to a host not on the allow list: err %v, want ErrHostNotAllowed", err)
	}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/ftp", Sink: discardSink}); err == nil {
		t.Error("a redirect to ftp was followed")
	}
	if f.count() != hits+2 {
		t.Errorf("upstream got %d requests, want 2", f.count()-hits)
	}

	hits = f.count()
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/loop/10", Sink: discardSink}); err != nil || res.Status != 200 {
		t.Errorf("10 redirects: got %+v (err %v), want them followed", res, err)
	}
	if f.count() != hits+11 {
		t.Errorf("10 redirects took %d requests, want 11", f.count()-hits)
	}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/loop/11", Sink: discardSink}); err == nil || !strings.Contains(err.Error(), "redirects") {
		t.Errorf("11 redirects: err %v, want a refusal", err)
	}
	if _, err := u.Fetch(context.Background(), Request{URL: "http://elsewhere.example/", Sink: discardSink}); !errors.Is(err, ErrHostNotAllowed) {
		t.Errorf("a host not on the allow list: err %v, want ErrHostNotAllowed", err)
	}
}

func TestHostLabelIsTheEntryOrOther(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "12345") })
	before := scrape(t)

	unconfigured := newTestUpstream(t, Options{Policy: testPolicy(nil)})
	if _, err := unconfigured.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}
	p := testPolicy(nil)
	p.Hosts["127.0.0.1"] = p.Default
	configured := newTestUpstream(t, Options{Policy: p})
	if _, err := configured.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}

	after := scrape(t)
	for _, tc := range []struct {
		series string
		grew   float64
	}{
		{`cortex_upstream_requests_total{host="other",code="200"}`, 1},
		{`cortex_upstream_requests_total{host="127.0.0.1",code="200"}`, 1},
		{`cortex_upstream_bytes_total{host="other"}`, 5},
		{`cortex_upstream_bytes_total{host="127.0.0.1"}`, 5},
		{`cortex_upstream_request_duration_seconds_count{host="other"}`, 1},
	} {
		if got := after[tc.series] - before[tc.series]; got != tc.grew {
			t.Errorf("%s grew by %v, want %v", tc.series, got, tc.grew)
		}
	}
	for _, series := range []string{
		`cortex_upstream_requests_total{host="other",code="503"}`,
		`cortex_upstream_requests_total{host="127.0.0.1",code="error"}`,
		`cortex_host_queue{host="other"}`,
		`cortex_host_in_flight{host="127.0.0.1"}`,
		`cortex_host_paused{host="127.0.0.1"}`,
	} {
		if _, ok := after[series]; !ok {
			t.Errorf("%s is missing, want it there from the start", series)
		}
	}
	// The gauges show the labels of the open Upstreams' policies, nothing else.
	for series := range after {
		if strings.HasPrefix(series, "cortex_host_") && !strings.Contains(series, `host="other"`) && !strings.Contains(series, `host="127.0.0.1"`) {
			t.Errorf("unexpected host label in %s", series)
		}
	}
}

func TestABodyLimitNearTheLargestInt64CannotOverflow(t *testing.T) {
	// The policy keeps max_body at 1 PiB at most; the readers must not overflow even past that.
	for _, max := range []int64{math.MaxInt64, math.MaxInt64 - 1, maxBodyLimit} {
		got, err := io.ReadAll(&body{r: strings.NewReader("hello"), max: max})
		if err != nil || string(got) != "hello" {
			t.Errorf("body with max %d: got %q (err %v), want hello", max, got, err)
		}
		got, err = io.ReadAll(&wire{r: strings.NewReader("hello"), max: max})
		if err != nil || string(got) != "hello" {
			t.Errorf("wire with max %d: got %q (err %v), want hello", max, got, err)
		}
	}
	// Three bytes of room left below the largest int64: the fourth is too many.
	b := &body{r: strings.NewReader("hello"), max: math.MaxInt64, n: math.MaxInt64 - 3}
	if got, err := io.ReadAll(b); !errors.Is(err, ErrTooLarge) || string(got) != "hel" {
		t.Errorf("body at the edge: got %q (err %v), want hel and ErrTooLarge", got, err)
	}
	w := &wire{r: strings.NewReader("hello"), max: math.MaxInt64, n: math.MaxInt64 - 3}
	if got, err := io.ReadAll(w); !errors.Is(err, ErrTooLarge) || string(got) != "hel" {
		t.Errorf("wire at the edge: got %q (err %v), want hel and ErrTooLarge", got, err)
	}

	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "hello") })
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.MaxBody = maxBodyLimit })})
	var sink memSink
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: sink.sink}); err != nil || res.Size != 5 {
		t.Errorf("max_body 1 PiB: got %+v (err %v), want the 5 bytes", res, err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.InFlight != 0 {
		t.Errorf("got %+v, want the slot given back", hs)
	}
}

func TestValidatorsGoOnlyToTheURLAsked(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		conditional := r.Header.Get("If-None-Match") != "" || r.Header.Get("If-Modified-Since") != ""
		switch r.URL.Path {
		case "/latest":
			// It led to /v3 when the stored version was fetched; the validators are /v3's.
			http.Redirect(w, r, "/v1", http.StatusFound)
		case "/v1":
			// As Apache or Go's ServeContent: 304 when the file is not newer than the date asked.
			if conditional {
				w.WriteHeader(http.StatusNotModified)
				return
			}
			_, _ = io.WriteString(w, "version one")
		case "/broken":
			http.Redirect(w, r, "/always-304", http.StatusFound)
		case "/always-304":
			w.WriteHeader(http.StatusNotModified)
		}
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})
	validators := func(sink *memSink, path string) Request {
		return Request{URL: f.srv.URL + path, IfNoneMatch: `"v3"`, IfModifiedSince: "Thu, 01 Oct 2026 10:00:00 GMT", Sink: sink.sink}
	}

	var sink memSink
	res, err := u.Fetch(context.Background(), validators(&sink, "/latest"))
	if err != nil || res.NotModified || res.Status != 200 || string(sink.body) != "version one" || res.FinalURL != f.srv.URL+"/v1" {
		t.Fatalf("got %+v %q (err %v), want /v1's content: its 304 would confirm /v3's body", res, sink.body, err)
	}
	f.mu.Lock()
	first, second := f.headers[0], f.headers[1]
	f.mu.Unlock()
	if first.Get("If-None-Match") != `"v3"` || first.Get("If-Modified-Since") == "" {
		t.Errorf("the URL asked got %v, want both validators", first)
	}
	if second.Get("If-None-Match") != "" || second.Get("If-Modified-Since") != "" {
		t.Errorf("the redirect's target got %v, want no validators", second)
	}

	// A 304 the target sends all the same confirms nothing: an unstorable answer.
	sink = memSink{}
	_, err = u.Fetch(context.Background(), validators(&sink, "/broken"))
	var se *StatusError
	if !errors.As(err, &se) || se.Status != 304 || sink.called() != 0 {
		t.Errorf("a 304 at the end of a redirect: err %v, sink %d calls; want *StatusError{304}", err, sink.called())
	}

	// The URL asked itself answers 304: the stored version is confirmed.
	res, err = u.Fetch(context.Background(), validators(&sink, "/v1"))
	if err != nil || !res.NotModified {
		t.Errorf("a 304 of the URL asked: got %+v (err %v), want NotModified", res, err)
	}
}

// Without stored validators nothing conditional goes out, not even an empty header, and a 304
// of the URL asked is then an answer nobody asked for: *StatusError, never NotModified, which
// would have the server confirm a version it may not have (the server sends no validators when
// the stored version came from another URL). Only the validator given goes out.
func TestNoValidatorsMeansNoConditionalRequest(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/always-304" {
			w.Header().Set("ETag", `"v1"`)
			w.WriteHeader(http.StatusNotModified)
			return
		}
		_, _ = io.WriteString(w, "content")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})
	sent := func(h http.Header, name string) bool { _, ok := h[name]; return ok }

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page#part", Sink: sink.sink})
	if err != nil || res.NotModified || res.Status != 200 || string(sink.body) != "content" {
		t.Fatalf("got %+v (err %v), want the content", res, err)
	}
	if res.FinalURL != f.srv.URL+"/page" {
		t.Errorf("FinalURL %q, want the URL asked without its fragment", res.FinalURL)
	}
	if h := f.lastHeader(); sent(h, "If-None-Match") || sent(h, "If-Modified-Since") {
		t.Errorf("sent %v without validators, want no If-None-Match or If-Modified-Since", h)
	}

	sink = memSink{}
	_, err = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/always-304", Sink: sink.sink})
	var se *StatusError
	if !errors.As(err, &se) || se.Status != 304 || sink.called() != 0 {
		t.Errorf("a 304 on the first hop without validators: err %v, sink %d calls; want *StatusError{304}",
			err, sink.called())
	}

	_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page",
		IfModifiedSince: "Thu, 01 Oct 2026 10:00:00 GMT", Sink: sink.sink})
	if h := f.lastHeader(); sent(h, "If-None-Match") || h.Get("If-Modified-Since") != "Thu, 01 Oct 2026 10:00:00 GMT" {
		t.Errorf("sent %v with If-Modified-Since only, want just that validator", h)
	}
}

func TestHostsMustBeASCII(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/to-alias" {
			http.Redirect(w, r, "http://ｌｏｃａｌｈｏｓｔ:"+r.URL.Query().Get("port")+"/x", http.StatusFound)
			return
		}
		w.Header().Set("Content-Type", "text/html")
		_, _ = io.WriteString(w, "<html>bot protection</html>")
	})
	port := f.srv.URL[strings.LastIndexByte(f.srv.URL, ':')+1:]
	p := testPolicy(nil)
	pdf := p.Default
	pdf.ExpectType, pdf.Pause = "application/pdf", 2*time.Second
	p.Hosts["localhost"] = pdf
	u := newTestUpstream(t, Options{Policy: p})

	// Go's transport maps each of these to "localhost" before it dials; the policy would not.
	for _, alias := range []string{
		"http://ｌｏｃａｌｈｏｓｔ:" + port + "/x",         // fullwidth letters
		"http://loca\u00adlhost:" + port + "/x",   // a soft hyphen
		"http://LOCALHO\u017fT:" + port + "/x",    // a long s
		"http://%EF%BD%8Cocalhost:" + port + "/x", // a fullwidth l, percent-encoded
		"http://ｌｏｃａｌｈｏｓｔ.:" + port + "/x?a=b",    // and a trailing dot
	} {
		var sink memSink
		_, err := u.Fetch(context.Background(), Request{URL: alias, Sink: sink.sink})
		if !errors.Is(err, ErrHostNotAllowed) || !strings.Contains(err.Error(), "punycode") {
			t.Errorf("%s: err %v, want ErrHostNotAllowed asking for punycode", alias, err)
		}
		if sink.called() != 0 {
			t.Errorf("%s: the sink was called", alias)
		}
	}
	if f.count() != 0 {
		t.Errorf("upstream got %d requests, want none", f.count())
	}

	// A redirect is held to it too.
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/to-alias?port=" + port, Sink: discardSink}); !errors.Is(err, ErrHostNotAllowed) {
		t.Errorf("a redirect to a Unicode host: err %v, want ErrHostNotAllowed", err)
	}
	if f.count() != 1 {
		t.Errorf("upstream got %d requests, want only the redirect", f.count())
	}
	for _, hs := range u.HostStates() {
		if hs.Host != "localhost" && hs.Host != "127.0.0.1" {
			t.Errorf("a host state for %q", hs.Host)
		}
	}
	// The ASCII name keeps its rules.
	if _, err := u.Fetch(context.Background(), Request{URL: "http://localhost:" + port + "/x", Sink: discardSink}); !errors.Is(err, ErrWrongType) {
		t.Errorf("localhost: err %v, want ErrWrongType from its entry", err)
	}
}

func TestTheRulesOfTheFirstHostHoldAtTheEndOfARedirect(t *testing.T) {
	var port string
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		to127 := func(path string) { http.Redirect(w, r, "http://127.0.0.1:"+port+path, http.StatusFound) }
		switch r.URL.Path {
		case "/statute.pdf":
			to127("/challenge")
		case "/big.pdf":
			to127("/big")
		case "/doc.pdf":
			to127("/doc")
		case "/to-pdf-host":
			http.Redirect(w, r, "http://localhost:"+port+"/challenge", http.StatusFound)
		case "/challenge":
			w.Header().Set("Content-Type", "text/html")
			_, _ = io.WriteString(w, "<html>are you a bot?</html>")
		case "/big":
			w.Header().Set("Content-Type", "application/pdf")
			for i := 0; i < 100; i++ {
				_, _ = w.Write([]byte{'x'})
				w.(http.Flusher).Flush()
			}
		case "/doc":
			w.Header().Set("Content-Type", "application/pdf")
			_, _ = io.WriteString(w, "%PDF-1.7")
		}
	})
	port = f.srv.URL[strings.LastIndexByte(f.srv.URL, ':')+1:]
	p := testPolicy(nil)
	pdf := p.Default
	pdf.ExpectType, pdf.MaxBody = "application/pdf", 50
	p.Hosts["localhost"] = pdf // 127.0.0.1 has the default: any type, 1 MiB
	u := newTestUpstream(t, Options{Policy: p})
	pdfURL := func(path string) string { return "http://localhost:" + port + path }

	var sink memSink
	_, err := u.Fetch(context.Background(), Request{URL: pdfURL("/statute.pdf"), Sink: sink.sink})
	if !errors.Is(err, ErrWrongType) || !strings.Contains(err.Error(), "localhost") {
		t.Errorf("a PDF host redirecting to a challenge on another host: err %v, want ErrWrongType naming localhost", err)
	}
	if sink.called() != 0 {
		t.Errorf("the challenge page went to the sink")
	}
	if _, err := u.Fetch(context.Background(), Request{URL: pdfURL("/big.pdf"), Sink: discardSink}); !errors.Is(err, ErrTooLarge) {
		t.Errorf("100 bytes behind a host with max_body 50: err %v, want ErrTooLarge", err)
	}
	sink = memSink{}
	if res, err := u.Fetch(context.Background(), Request{URL: pdfURL("/doc.pdf"), Sink: sink.sink}); err != nil || string(sink.body) != "%PDF-1.7" {
		t.Errorf("a PDF behind a redirect: got %+v (err %v), want it stored", res, err)
	}
	// The host at the end keeps its own rules, as before.
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/to-pdf-host", Sink: discardSink}); !errors.Is(err, ErrWrongType) {
		t.Errorf("a redirect to the PDF host: err %v, want ErrWrongType", err)
	}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/challenge", Sink: discardSink}); err != nil {
		t.Errorf("a host without rules: err %v", err)
	}
}

// gzipped compresses b as one gzip member.
func gzipped(t *testing.T, b []byte) []byte {
	t.Helper()
	var buf bytes.Buffer
	zw := gzip.NewWriter(&buf)
	if _, err := zw.Write(b); err != nil {
		t.Fatal(err)
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

func TestAGzipBodyIsDecodedAndHeldToMaxBodyOnTheWireToo(t *testing.T) {
	page := gzipped(t, []byte("<p>Modul</p>"))
	bomb := gzipped(t, make([]byte, 1<<20)) // a kilobyte that decodes to a mebibyte
	empty := gzipped(t, nil)                // a member that decodes to nothing
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		w.Header().Set("Content-Encoding", "gzip")
		switch r.URL.Path {
		case "/page":
			_, _ = w.Write(page)
		case "/bomb":
			_, _ = w.Write(bomb)
		case "/empty-members":
			// Megabytes on the wire that decode to five bytes: only a limit on the wire stops them.
			chunk := bytes.Repeat(empty, 1<<12)
			for i := 0; i < 64; i++ {
				if _, err := w.Write(chunk); err != nil {
					return
				}
			}
			_, _ = w.Write(gzipped(t, []byte("hello")))
		case "/no-body":
		case "/broken":
			_, _ = io.WriteString(w, "this is not gzip")
		case "/brotli":
			w.Header().Set("Content-Encoding", "br")
			_, _ = io.WriteString(w, "\x0b\x02\x80hello\x03")
		}
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) {
		h.MaxBody = 4096
		h.BreakerFailures = 100
	})})

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/page", Sink: sink.sink})
	if err != nil || string(sink.body) != "<p>Modul</p>" || res.Size != 12 || res.Hash != hashOf([]byte("<p>Modul</p>")) {
		t.Errorf("a gzip page: got %+v %q (err %v), want it decoded", res, sink.body, err)
	}
	if res.Header.Get("Content-Encoding") != "" || f.lastHeader().Get("Accept-Encoding") != "gzip" {
		t.Errorf("kept headers %v, sent %v: want gzip asked for and no coding kept", res.Header, f.lastHeader())
	}

	sink = memSink{}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/bomb", Sink: sink.sink}); !errors.Is(err, ErrTooLarge) {
		t.Errorf("a mebibyte decoded from a kilobyte: err %v, want ErrTooLarge", err)
	}
	if len(sink.body) > 4096 || !errors.Is(sink.err, ErrTooLarge) {
		t.Errorf("the sink read %d bytes ending in %v, want at most 4096 and ErrTooLarge", len(sink.body), sink.err)
	}

	sink = memSink{}
	_, err = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/empty-members", Sink: sink.sink})
	if !errors.Is(err, ErrTooLarge) || !strings.Contains(err.Error(), "gzip") {
		t.Errorf("megabytes of empty gzip members: err %v, want ErrTooLarge for the bytes on the wire", err)
	}
	if !errors.Is(sink.err, ErrTooLarge) {
		t.Errorf("the sink's read ended in %v, want ErrTooLarge", sink.err)
	}

	sink = memSink{}
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/no-body", Sink: sink.sink}); err != nil || res.Size != 0 || sink.called() != 1 {
		t.Errorf("an empty gzip body: got %+v (err %v), want an empty answer, as Go's transport gives", res, err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 0 {
		t.Errorf("got %+v: a body too large counted as a failure of the host", hs)
	}

	sink = memSink{}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/broken", Sink: sink.sink}); err == nil || errors.Is(err, ErrTooLarge) {
		t.Errorf("a body that is not gzip: err %v, want a read error", err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 1 {
		t.Errorf("got %+v, want broken gzip to count as a failure of the host", hs)
	}
	sink = memSink{}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/brotli", Sink: sink.sink}); err == nil || sink.called() != 0 {
		t.Errorf("a coding Cortex did not ask for: err %v, sink %d calls; want a refusal before the sink", err, sink.called())
	}
}
