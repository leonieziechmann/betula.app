package client

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// instance is a fake Cortex instance that counts its requests and answers with answer.
type instance struct {
	srv    *httptest.Server
	hits   atomic.Int32
	answer func(w http.ResponseWriter, r *http.Request)
	mu     sync.Mutex
}

func newInstance(t *testing.T, answer func(w http.ResponseWriter, r *http.Request)) *instance {
	t.Helper()
	in := &instance{answer: answer}
	in.srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		in.hits.Add(1)
		in.mu.Lock()
		answer := in.answer
		in.mu.Unlock()
		answer(w, r)
	}))
	t.Cleanup(in.srv.Close)
	return in
}

func (in *instance) set(answer func(w http.ResponseWriter, r *http.Request)) {
	in.mu.Lock()
	defer in.mu.Unlock()
	in.answer = answer
}

// answering answers every request with 200 and its name.
func answering(name string) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		_, _ = io.Copy(io.Discard, r.Body)
		fmt.Fprint(w, name)
	}
}

// noLeaderAnswer is the answer of an instance that knows no leader.
func noLeaderAnswer(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cortex-Error", "no-leader")
	w.Header().Set("Retry-After", "1")
	w.WriteHeader(http.StatusServiceUnavailable)
	fmt.Fprint(w, `{"error":"no-leader","message":"no leader known"}`)
}

// dropping reads the request and closes the connection without an answer, as an instance
// that crashes does; with reset, it resets the connection instead.
func dropping(t *testing.T, reset bool) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		_, _ = io.Copy(io.Discard, r.Body)
		conn, _, err := w.(http.Hijacker).Hijack()
		if err != nil {
			t.Errorf("Hijack: %v", err)
			return
		}
		if tcp, ok := conn.(*net.TCPConn); ok && reset {
			_ = tcp.SetLinger(0)
		}
		_ = conn.Close()
	}
}

// refusing is the URL of a port nothing listens on.
func refusing(t *testing.T) string {
	t.Helper()
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("Listen: %v", err)
	}
	addr := l.Addr().String()
	_ = l.Close()
	return "http://" + addr
}

func newTestClient(t *testing.T, wait time.Duration, urls ...string) *Client {
	t.Helper()
	c, err := New(strings.Join(urls, ","), Options{FailoverWait: wait})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	return c
}

func fetchBody(t *testing.T, c *Client) (int, string, error) {
	t.Helper()
	resp, err := c.Fetch(context.Background(), "https://www.b-tu.de/modul", FetchOptions{Mode: ModeCache, MaxAge: -1})
	if err != nil {
		return 0, "", err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	return resp.StatusCode, string(body), err
}

func TestNewTakesACommaSeparatedListOfInstances(t *testing.T) {
	c, err := New(" http://cortex_a:8100/ ,http://cortex_b:8100,", Options{})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	if got := strings.Join(c.Endpoints(), " "); got != "http://cortex_a:8100 http://cortex_b:8100" {
		t.Errorf("Endpoints = %s", got)
	}
	if c.wait != DefaultFailoverWait {
		t.Errorf("FailoverWait = %v, want the default", c.wait)
	}
	for _, urls := range []string{"", " , ", "cortex_a:8100", "ftp://cortex_a", "http://", "http://user:secret@cortex_a:8100",
		"http://cortex_a:8100/?x=1", "http://cortex_a:8100/#top", "http://cortex_a:8100,::"} {
		if _, err := New(urls, Options{}); err == nil {
			t.Errorf("New(%q) accepted", urls)
		}
	}
}

// Cortex serves its API at the root. Below a path, every request would reach Cortex's answer
// for a path it does not serve, a 404 not-found that a careless caller takes for the host's
// 404 (a crawl archived it in place of the page), so a base URL with a path is refused; and
// with a "?" or "#" left over, the paths of the API would end up in the query or fragment.
func TestNewRefusesABaseURLWithAPath(t *testing.T) {
	for _, urls := range []string{"http://cortex_a:8100/v1", "http://cortex_a:8100/cortex/", "http://cortex_a:8100//",
		"http://cortex_a:8100/%2F", "http://cortex_a:8100/v1,http://cortex_b:8100", "http://cortex_a:8100,http://cortex_b:8100/v1",
		"http://cortex_a:8100?", "http://cortex_a:8100/?", "http://cortex_a:8100#", "http://cortex_a:8100/#"} {
		if c, err := New(urls, Options{}); err == nil {
			t.Errorf("New(%q) accepted: %v", urls, c.Endpoints())
		} else if !strings.Contains(err.Error(), "no path") {
			t.Errorf("New(%q): %v, want the reason", urls, err)
		}
	}
	c, err := New("HTTP://cortex_a:8100/,https://[fd00::b]:8100", Options{})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	if got := strings.Join(c.Endpoints(), " "); got != "http://cortex_a:8100 https://[fd00::b]:8100" {
		t.Errorf("Endpoints = %s", got)
	}
}

// An instance nothing listens on is passed over, and the one that answered is asked first
// from then on.
func TestFailoverPassesOverAnInstanceThatRefusesConnections(t *testing.T) {
	b := newInstance(t, answering("b"))
	var tried []string
	var mu sync.Mutex
	counting := &http.Client{Transport: roundTripFunc(func(r *http.Request) (*http.Response, error) {
		mu.Lock()
		tried = append(tried, r.URL.Host)
		mu.Unlock()
		return http.DefaultTransport.RoundTrip(r)
	})}
	down := refusing(t)
	c, err := New(down+","+b.srv.URL, Options{HTTPClient: counting})
	if err != nil {
		t.Fatalf("New: %v", err)
	}

	for i := 0; i < 3; i++ {
		if status, body, err := fetchBody(t, c); err != nil || status != 200 || body != "b" {
			t.Fatalf("fetch %d = %d %q (err %v), want b's answer", i, status, body, err)
		}
	}
	downHost := strings.TrimPrefix(down, "http://")
	bHost := strings.TrimPrefix(b.srv.URL, "http://")
	if got := strings.Join(tried, " "); got != strings.Join([]string{downHost, bHost, bHost, bHost}, " ") {
		t.Errorf("instances tried = %s, want the refusing one once, then b", got)
	}
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (f roundTripFunc) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }

// An instance that closes or resets the connection before it answers is passed over too.
func TestFailoverPassesOverAnInstanceThatDropsTheConnection(t *testing.T) {
	for _, tc := range []struct {
		name  string
		reset bool
	}{{"closed", false}, {"reset", true}} {
		t.Run(tc.name, func(t *testing.T) {
			a := newInstance(t, dropping(t, tc.reset))
			b := newInstance(t, answering("b"))
			c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)
			if status, body, err := fetchBody(t, c); err != nil || status != 200 || body != "b" {
				t.Fatalf("fetch = %d %q (err %v), want b's answer", status, body, err)
			}
			if _, _, err := fetchBody(t, c); err != nil {
				t.Fatalf("second fetch: %v", err)
			}
			if a.hits.Load() != 1 || b.hits.Load() != 2 {
				t.Errorf("hits a=%d b=%d, want 1 and 2 (b answered, so b is asked first)", a.hits.Load(), b.hits.Load())
			}
		})
	}
}

// While no instance knows a leader, the client goes round them, a pause between rounds,
// until one takes the request; then it stays with that one, and when that one fails, it
// recovers with the other.
func TestFailoverWaitsForALeaderAndRecovers(t *testing.T) {
	a := newInstance(t, noLeaderAnswer)
	b := newInstance(t, noLeaderAnswer)
	c := newTestClient(t, 5*time.Second, a.srv.URL, b.srv.URL)

	elected := make(chan struct{})
	go func() {
		// The follower takes over after a few rounds.
		for b.hits.Load() < 3 {
			time.Sleep(5 * time.Millisecond)
		}
		b.set(answering("b"))
		close(elected)
	}()
	start := time.Now()
	status, body, err := fetchBody(t, c)
	<-elected
	if err != nil || status != 200 || body != "b" {
		t.Fatalf("fetch = %d %q (err %v), want b's answer once it leads", status, body, err)
	}
	if took := time.Since(start); took < 2*failoverPause {
		t.Errorf("took %v: the rounds were not paused", took)
	}

	// b answers now, and is asked first.
	aHits := a.hits.Load()
	if _, body, err := fetchBody(t, c); err != nil || body != "b" || a.hits.Load() != aHits {
		t.Errorf("after the election: %q (err %v), a asked %d more times", body, err, a.hits.Load()-aHits)
	}

	// b goes away; a leads now.
	b.srv.CloseClientConnections()
	b.set(dropping(t, false))
	a.set(answering("a"))
	if _, body, err := fetchBody(t, c); err != nil || body != "a" {
		t.Fatalf("after b went away: %q (err %v), want a", body, err)
	}
}

// When no instance knows a leader within FailoverWait, the last answer is handed on: a 503
// the caller can read and retry.
func TestFailoverGivesUpWithTheLastAnswer(t *testing.T) {
	a := newInstance(t, noLeaderAnswer)
	b := newInstance(t, noLeaderAnswer)
	c := newTestClient(t, 600*time.Millisecond, a.srv.URL, b.srv.URL)

	start := time.Now()
	resp, err := c.Fetch(context.Background(), "https://www.b-tu.de/modul", FetchOptions{MaxAge: -1})
	if err != nil {
		t.Fatalf("Fetch: %v", err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	took := time.Since(start)
	if resp.StatusCode != http.StatusServiceUnavailable || ErrorCode(resp) != "no-leader" || !strings.Contains(string(body), "no leader known") {
		t.Errorf("answer = %d %q %q, want the 503 no-leader", resp.StatusCode, ErrorCode(resp), body)
	}
	if took < 500*time.Millisecond || took > 3*time.Second {
		t.Errorf("gave up after %v, want about the FailoverWait of 600 ms", took)
	}
	if rounds := a.hits.Load(); rounds < 2 || b.hits.Load() != rounds {
		t.Errorf("hits a=%d b=%d, want the same number of rounds, at least 2", rounds, b.hits.Load())
	}

	// A single round with a negative FailoverWait.
	single := newTestClient(t, -1, a.srv.URL, b.srv.URL)
	aHits := a.hits.Load()
	if status, _, err := fetchBody(t, single); err != nil || status != http.StatusServiceUnavailable || a.hits.Load() != aHits+1 {
		t.Errorf("single round: %d (err %v), a asked %d times", status, err, a.hits.Load()-aHits)
	}

	// Every instance unreachable: the connection error, after the same wait.
	down := newTestClient(t, 300*time.Millisecond, refusing(t), refusing(t))
	start = time.Now()
	if _, _, err := fetchBody(t, down); err == nil || !strings.Contains(err.Error(), "connection refused") {
		t.Errorf("all down: err = %v, want connection refused", err)
	}
	if took := time.Since(start); took < 250*time.Millisecond {
		t.Errorf("all down: gave up after %v, want about 300 ms", took)
	}
}

// A 503 of another kind, a host that is paused, is Cortex's answer for the caller, not a
// reason to ask the other instance.
func TestOtherErrorsAreAnswers(t *testing.T) {
	a := newInstance(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cortex-Error", "host-paused")
		w.WriteHeader(http.StatusServiceUnavailable)
	})
	b := newInstance(t, answering("b"))
	c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)
	if status, _, err := fetchBody(t, c); err != nil || status != http.StatusServiceUnavailable || b.hits.Load() != 0 {
		t.Errorf("fetch = %d (err %v), b asked %d times; want a's 503", status, err, b.hits.Load())
	}
}

// A POST may have been carried out by an instance that dropped the connection: it is not
// sent again. One that could not be connected to never saw it.
func TestOnlyIdempotentRequestsAreSentAgain(t *testing.T) {
	a := newInstance(t, dropping(t, false))
	b := newInstance(t, answering("b"))
	c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)
	if err := c.StepDown(context.Background()); err == nil || b.hits.Load() != 0 {
		t.Errorf("StepDown through a dropping instance: err = %v, b asked %d times; want the error, b not asked", err, b.hits.Load())
	}

	c = newTestClient(t, time.Second, refusing(t), b.srv.URL)
	if err := c.StepDown(context.Background()); err != nil || b.hits.Load() != 1 {
		t.Errorf("StepDown past an unreachable instance: err = %v, b asked %d times", err, b.hits.Load())
	}

	// No leader: a POST is not sent round either.
	n := newInstance(t, noLeaderAnswer)
	c = newTestClient(t, time.Second, n.srv.URL, b.srv.URL)
	if err := c.StepDown(context.Background()); err == nil || b.hits.Load() != 1 {
		t.Errorf("StepDown at an instance without leader: err = %v, b asked %d times", err, b.hits.Load())
	}
}

// The caller's context ends the waiting.
func TestFailoverEndsWithTheContext(t *testing.T) {
	a := newInstance(t, noLeaderAnswer)
	c := newTestClient(t, 10*time.Second, a.srv.URL)
	ctx, cancel := context.WithTimeout(context.Background(), 400*time.Millisecond)
	defer cancel()
	start := time.Now()
	_, err := c.Fetch(ctx, "https://www.b-tu.de/modul", FetchOptions{})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Errorf("err = %v, want the context's deadline", err)
	}
	if took := time.Since(start); took > 2*time.Second {
		t.Errorf("returned after %v", took)
	}
}

// Many goroutines share one client while it fails over.
func TestFailoverIsSafeForConcurrentUse(t *testing.T) {
	a := newInstance(t, dropping(t, false))
	b := newInstance(t, answering("b"))
	c := newTestClient(t, 2*time.Second, a.srv.URL, b.srv.URL)
	var wg sync.WaitGroup
	errs := make(chan error, 16)
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, body, err := fetchBody(t, c); err != nil || body != "b" {
				errs <- fmt.Errorf("%q, %v", body, err)
			}
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Error(err)
	}
}
