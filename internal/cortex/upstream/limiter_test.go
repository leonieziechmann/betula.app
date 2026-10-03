package upstream

import (
	"context"
	"errors"
	"fmt"
	"io"
	"math/rand/v2"
	"net/http"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// gate holds the requests of a test server until it is opened.
type gate struct {
	once sync.Once
	ch   chan struct{}
}

func newGate(t *testing.T) *gate {
	t.Helper()
	g := &gate{ch: make(chan struct{})}
	t.Cleanup(g.open) // before the server's Close, which waits for its handlers
	return g
}

func (g *gate) open() { g.once.Do(func() { close(g.ch) }) }

func (g *gate) wait(r *http.Request) {
	select {
	case <-g.ch:
	case <-r.Context().Done():
	}
}

func TestConcurrencyOneSerialisesRequests(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(10 * time.Millisecond)
		_, _ = io.WriteString(w, "ok")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	var wg sync.WaitGroup
	errs := make(chan error, 5)
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
			errs <- err
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Errorf("Fetch failed: %v", err)
		}
	}
	if f.hits != 5 || f.maxInFlight != 1 {
		t.Errorf("%d requests, at most %d at once; want 5, one at a time", f.hits, f.maxInFlight)
	}
}

func TestConcurrencyIsTheNumberOfSlots(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, r.URL.Query().Get("i"))
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.Concurrency = 3 })})

	var wg sync.WaitGroup
	for i := 0; i < 3; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/?i=first", Sink: discardSink})
		}()
	}
	waitFor(t, "three requests in flight", func() bool { return f.running() == 3 })
	for i := 0; i < 2; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/?i=later", Sink: discardSink})
		}()
	}
	waitFor(t, "two requests in the queue", func() bool { return stateOf(u, "127.0.0.1").Queue == 2 })
	if hs := stateOf(u, "127.0.0.1"); hs.InFlight != 3 || hs.Queue != 2 || f.running() != 3 {
		t.Errorf("got %+v, want 3 in flight and 2 waiting", hs)
	}
	g.open()
	wg.Wait()
	if f.hits != 5 || f.maxInFlight != 3 {
		t.Errorf("%d requests, at most %d at once; want 5, three at a time", f.hits, f.maxInFlight)
	}
}

func TestTheQueueKeepsTheOrderOfArrival(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, "ok")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	var wg sync.WaitGroup
	for i, name := range []string{"first", "a", "b", "c"} {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL + "/" + name, Sink: discardSink})
		}()
		// Each request joins only after the one before it did.
		if i == 0 {
			waitFor(t, "the first request in flight", func() bool { return f.running() == 1 })
		} else {
			waitFor(t, "a request in the queue", func() bool { return stateOf(u, "127.0.0.1").Queue == i })
		}
	}
	g.open()
	wg.Wait()
	if got := strings.Join(f.urls, " "); got != "/first /a /b /c" {
		t.Errorf("requests went in as %q, want in order of arrival", got)
	}
}

func TestPauseIsRespectedBeforeASlotIsUsedAgain(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "ok") })
	const pause = 80 * time.Millisecond
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.Pause = pause })})

	for i := 0; i < 3; i++ {
		if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); err != nil {
			t.Fatalf("Fetch failed: %v", err)
		}
	}
	f.mu.Lock()
	defer f.mu.Unlock()
	for i := 1; i < len(f.starts); i++ {
		// The slot is given back after the body, later than the handler ended: the gap is at
		// least the pause less its 30 % jitter.
		if gap := f.starts[i].Sub(f.ends[i-1]); gap < pause*7/10 {
			t.Errorf("request %d started %s after the one before ended, want at least %s", i, gap, pause*7/10)
		}
	}
}

func TestJitterIsWithinThirtyPercent(t *testing.T) {
	seenLow, seenHigh := false, false
	for i := 0; i < 1000; i++ {
		d := jitter(time.Second)
		if d < 700*time.Millisecond || d > 1300*time.Millisecond {
			t.Fatalf("jitter(1s) = %s, want 0.7s to 1.3s", d)
		}
		seenLow = seenLow || d < 900*time.Millisecond
		seenHigh = seenHigh || d > 1100*time.Millisecond
	}
	if !seenLow || !seenHigh {
		t.Error("jitter does not spread")
	}
	if jitter(0) != 0 {
		t.Error("jitter(0) is not 0")
	}
}

func TestQueueWaitAnswersBusy(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, "ok")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.QueueWait = 50 * time.Millisecond })})

	done := make(chan error, 1)
	go func() {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/slow", Sink: discardSink})
		done <- err
	}()
	waitFor(t, "the first request in flight", func() bool { return f.running() == 1 })

	start := time.Now()
	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/next", Sink: discardSink})
	var be *BusyError
	if !errors.As(err, &be) || be.Host != "127.0.0.1" || be.RetryAfter < time.Second {
		t.Fatalf("err %v, want *BusyError with a Retry-After of at least 1 s", err)
	}
	if took := time.Since(start); took < 50*time.Millisecond {
		t.Errorf("turned away after %s, want after the queue wait", took)
	}
	if f.count() != 1 {
		t.Errorf("upstream got %d requests, want 1", f.count())
	}
	if hs := stateOf(u, "127.0.0.1"); hs.Queue != 0 || hs.FailuresInRow != 0 {
		t.Errorf("got %+v, want an empty queue and no failure", hs)
	}
	g.open()
	if err := <-done; err != nil {
		t.Errorf("the first request failed: %v", err)
	}
}

func TestAWaiterThatGivesUpLeavesTheQueue(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/slow" {
			g.wait(r)
		}
		_, _ = io.WriteString(w, "ok")
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(nil)})

	done := make(chan error, 1)
	go func() {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/slow", Sink: discardSink})
		done <- err
	}()
	waitFor(t, "the first request in flight", func() bool { return f.running() == 1 })

	ctx, cancel := context.WithCancel(context.Background())
	waiting := make(chan error, 1)
	go func() {
		_, err := u.Fetch(ctx, Request{URL: f.srv.URL + "/gives-up", Sink: discardSink})
		waiting <- err
	}()
	waitFor(t, "the second request in the queue", func() bool { return stateOf(u, "127.0.0.1").Queue == 1 })
	cancel()
	if err := <-waiting; !errors.Is(err, context.Canceled) {
		t.Errorf("err %v, want context.Canceled", err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.Queue != 0 || hs.InFlight != 1 {
		t.Errorf("got %+v, want the queue empty and one in flight", hs)
	}
	g.open()
	if err := <-done; err != nil {
		t.Fatalf("the first request failed: %v", err)
	}
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/after", Sink: discardSink}); err != nil {
		t.Errorf("the slot was lost: %v", err)
	}
	if f.count() != 2 {
		t.Errorf("upstream got %d requests, want 2", f.count())
	}
}

func TestBreakerOpensAfterFailuresInARowAndClosesAfterThePause(t *testing.T) {
	log := captureLog(t)
	var mu sync.Mutex
	failing := true
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()
		if failing {
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
		_, _ = io.WriteString(w, "ok")
	})
	clock := newClock()
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) {
		h.BreakerFailures = 3
		h.BreakerPause = 15 * time.Minute
	}), Now: clock.Now})

	for i := 0; i < 3; i++ {
		var se *StatusError
		if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); !errors.As(err, &se) || se.Status != 500 {
			t.Fatalf("request %d: err %v, want *StatusError{500}", i, err)
		}
	}
	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
	var pe *PausedError
	if !errors.As(err, &pe) || !pe.Until.Equal(clock.Now().Add(15*time.Minute)) {
		t.Fatalf("err %v, want *PausedError until 15 minutes from now", err)
	}
	if f.count() != 3 {
		t.Errorf("upstream got %d requests, want 3: a paused host is not asked", f.count())
	}
	hs := stateOf(u, "127.0.0.1")
	if hs.PausedUntil == nil || !hs.PausedUntil.Equal(pe.Until) || hs.FailuresInRow != 3 {
		t.Errorf("got %+v, want paused with 3 failures in a row", hs)
	}
	recs := log.records(t, "host.paused")
	if len(recs) != 1 || recs[0]["level"] != "WARN" || recs[0]["reason"] != "breaker" || recs[0]["host"] != "127.0.0.1" ||
		recs[0]["component"] != "upstream" {
		t.Errorf("host.paused records %v, want one WARN from the breaker", recs)
	}
	if got := scrape(t)[`cortex_host_paused{host="other"}`]; got != 1 {
		t.Errorf("cortex_host_paused %v, want 1", got)
	}

	clock.Advance(15*time.Minute + time.Second)
	mu.Lock()
	failing = false
	mu.Unlock()
	if res, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); err != nil || res.Status != 200 {
		t.Fatalf("after the pause: got %+v (err %v), want the host asked again", res, err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.PausedUntil != nil || hs.FailuresInRow != 0 {
		t.Errorf("got %+v, want not paused and the count reset by the success", hs)
	}
	if recs := log.records(t, "host.resumed"); len(recs) != 1 || recs[0]["level"] != "INFO" {
		t.Errorf("host.resumed records %v, want one INFO", recs)
	}
	if got := scrape(t)[`cortex_host_paused{host="other"}`]; got != 0 {
		t.Errorf("cortex_host_paused %v, want 0", got)
	}
}

func TestAfterAPauseOneMoreFailurePausesAgain(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { w.WriteHeader(http.StatusBadGateway) })
	clock := newClock()
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.BreakerFailures = 2 }), Now: clock.Now})

	for i := 0; i < 2; i++ {
		_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
	}
	clock.Advance(time.Hour)
	_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
	var pe *PausedError
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); !errors.As(err, &pe) {
		t.Errorf("err %v, want the host paused again after one failure", err)
	}
	if f.count() != 3 {
		t.Errorf("upstream got %d requests, want 3", f.count())
	}
}

func TestASuccessResetsTheFailuresInARow(t *testing.T) {
	var mu sync.Mutex
	statuses := []int{500, 500, 200, 500, 500, 404, 503, 503}
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		status := statuses[0]
		statuses = statuses[1:]
		mu.Unlock()
		w.WriteHeader(status)
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.BreakerFailures = 3 })})

	for i := 0; i < 8; i++ {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		var pe *PausedError
		if errors.As(err, &pe) {
			t.Fatalf("request %d: paused, though no three failures came in a row", i)
		}
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 2 || hs.PausedUntil != nil {
		t.Errorf("got %+v, want 2 failures in a row and no pause", hs)
	}
}

func TestNetworkErrorsAndTimeoutsAreFailuresAndAClientGivingUpIsNot(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, "late")
	})
	closed := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {})
	closed.srv.Close()
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) {
		h.Timeout = 50 * time.Millisecond
		h.BreakerFailures = 100
	})})

	if _, err := u.Fetch(context.Background(), Request{URL: closed.srv.URL, Sink: discardSink}); err == nil {
		t.Fatal("a closed port answered")
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 1 {
		t.Errorf("connection refused: got %+v, want 1 failure in a row", hs)
	}

	_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Errorf("err %v, want the timeout", err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 2 {
		t.Errorf("timeout: got %+v, want 2 failures in a row", hs)
	}

	ctx, cancel := context.WithCancel(context.Background())
	time.AfterFunc(10*time.Millisecond, cancel)
	if _, err := u.Fetch(ctx, Request{URL: f.srv.URL, Sink: discardSink}); !errors.Is(err, context.Canceled) {
		t.Errorf("err %v, want context.Canceled", err)
	}
	if hs := stateOf(u, "127.0.0.1"); hs.FailuresInRow != 2 {
		t.Errorf("a client that gave up: got %+v, want still 2 failures in a row", hs)
	}
}

func TestAPausedHostTurnsAwayTheRequestsWaitingForIt(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		w.WriteHeader(http.StatusInternalServerError)
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.BreakerFailures = 1 })})

	first := make(chan error, 1)
	go func() {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		first <- err
	}()
	waitFor(t, "the first request in flight", func() bool { return f.running() == 1 })
	second := make(chan error, 1)
	go func() {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		second <- err
	}()
	waitFor(t, "the second request in the queue", func() bool { return stateOf(u, "127.0.0.1").Queue == 1 })
	g.open()

	var se *StatusError
	if err := <-first; !errors.As(err, &se) {
		t.Errorf("the first: err %v, want *StatusError", err)
	}
	var pe *PausedError
	if err := <-second; !errors.As(err, &pe) {
		t.Errorf("the second: err %v, want *PausedError", err)
	}
	if f.count() != 1 {
		t.Errorf("upstream got %d requests, want 1", f.count())
	}
}

func TestRetryAfterPausesTheHost(t *testing.T) {
	log := captureLog(t)
	clock := newClock()
	var mu sync.Mutex
	answer := func(w http.ResponseWriter) { _, _ = io.WriteString(w, "ok") }
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		a := answer
		mu.Unlock()
		a(w)
	})
	set := func(status int, retryAfter string) {
		mu.Lock()
		answer = func(w http.ResponseWriter) {
			if retryAfter != "" {
				w.Header().Set("Retry-After", retryAfter)
			}
			w.WriteHeader(status)
		}
		mu.Unlock()
	}
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.BreakerFailures = 100 }), Now: clock.Now})
	fetch := func() error {
		_, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		return err
	}

	for _, tc := range []struct {
		name       string
		status     int
		retryAfter string // "date" is the clock's now plus pause, as an HTTP date
		pause      time.Duration
	}{
		{"503 with seconds", 503, "120", 120 * time.Second},
		{"429 with a date", 429, "date", 5 * time.Minute},
		{"429 for longer than an hour", 429, "99999", time.Hour},
		{"503 without Retry-After", 503, "", 0},
		{"429 with a date in the past", 429, "Wed, 01 Oct 2025 10:00:00 GMT", 0},
		{"500 with seconds", 500, "120", 0},
	} {
		if tc.retryAfter == "date" {
			tc.retryAfter = clock.Now().Add(tc.pause).Format(http.TimeFormat)
		}
		set(tc.status, tc.retryAfter)
		var se *StatusError
		if err := fetch(); !errors.As(err, &se) || se.Status != tc.status {
			t.Fatalf("%s: err %v, want *StatusError{%d}", tc.name, err, tc.status)
		}
		set(200, "")
		err := fetch()
		var pe *PausedError
		if tc.pause == 0 {
			if err != nil {
				t.Errorf("%s: err %v, want no pause", tc.name, err)
			}
			continue
		}
		if !errors.As(err, &pe) || !pe.Until.Equal(clock.Now().Add(tc.pause)) {
			t.Errorf("%s: err %v, want paused until %s from now", tc.name, err, tc.pause)
		}
		clock.Advance(tc.pause - time.Second)
		if err := fetch(); !errors.As(err, &pe) {
			t.Errorf("%s: err %v a second before the end, want still paused", tc.name, err)
		}
		clock.Advance(time.Second)
		if err := fetch(); err != nil {
			t.Errorf("%s: err %v after the pause, want the host asked", tc.name, err)
		}
	}
	for _, rec := range log.records(t, "host.paused") {
		if rec["reason"] != "retry-after" {
			t.Errorf("host.paused %v, want reason retry-after", rec)
		}
	}
	if n := len(log.records(t, "host.paused")); n != 3 {
		t.Errorf("%d host.paused records, want 3", n)
	}
}

func TestParseRetryAfter(t *testing.T) {
	now := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	for _, tc := range []struct {
		in   string
		want time.Duration
	}{
		{"", 0},
		{"0", 0},
		{"-5", 0},
		{" 30 ", 30 * time.Second},
		{"3600", time.Hour},
		{"86400", time.Hour},
		{"Fri, 02 Oct 2026 10:01:00 GMT", time.Minute},
		{"Fri, 02 Oct 2026 09:59:00 GMT", 0},
		{"soon", 0},
	} {
		if got := parseRetryAfter(tc.in, now); got != tc.want {
			t.Errorf("parseRetryAfter(%q) = %s, want %s", tc.in, got, tc.want)
		}
	}
}

func TestHostGaugesShowTheQueueAndTheRequestsInFlight(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, "ok")
	})
	p := testPolicy(nil)
	p.Hosts["127.0.0.1"] = p.Default
	u := newTestUpstream(t, Options{Policy: p})

	var wg sync.WaitGroup
	for i := 0; i < 2; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		}()
	}
	waitFor(t, "one in flight and one waiting", func() bool {
		hs := stateOf(u, "127.0.0.1")
		return hs.InFlight == 1 && hs.Queue == 1
	})
	m := scrape(t)
	if m[`cortex_host_in_flight{host="127.0.0.1"}`] != 1 || m[`cortex_host_queue{host="127.0.0.1"}`] != 1 ||
		m[`cortex_host_paused{host="127.0.0.1"}`] != 0 || m[`cortex_host_in_flight{host="other"}`] != 0 {
		t.Errorf("gauges %v, want one in flight and one waiting for 127.0.0.1", m)
	}
	g.open()
	wg.Wait()
	m = scrape(t)
	if m[`cortex_host_in_flight{host="127.0.0.1"}`] != 0 || m[`cortex_host_queue{host="127.0.0.1"}`] != 0 {
		t.Errorf("gauges %v after the requests, want 0", m)
	}
}

func TestHostStatesListTheConfiguredHostsAndForgetIdleOnes(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/fail" {
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
		_, _ = io.WriteString(w, "ok")
	})
	clock := newClock()
	p := testPolicy(nil)
	p.Hosts["qis.b-tu.de"] = p.Default
	p.Hosts["*.kobv.de"] = p.Default
	u := newTestUpstream(t, Options{Policy: p, Now: clock.Now})
	hosts := func() string {
		var out []string
		for _, hs := range u.HostStates() {
			out = append(out, hs.Host)
		}
		return strings.Join(out, " ")
	}
	fetch := func(path string) {
		t.Helper()
		_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL + path, Sink: discardSink})
	}

	// A host that answered and is idle says nothing a new state would not: gone at once.
	fetch("/")
	if got := hosts(); got != "qis.b-tu.de" {
		t.Errorf("hosts %q after a success, want only the configured one", got)
	}
	if n := statesHeld(u); n != 0 {
		t.Errorf("%d states held, want none", n)
	}

	// A failure is kept, for an hour after the host was last asked.
	fetch("/fail")
	if got := hosts(); got != "127.0.0.1 qis.b-tu.de" {
		t.Errorf("hosts %q after a failure, want the failing host and the configured one, sorted", got)
	}
	clock.Advance(30 * time.Minute)
	u.sweep()
	if got := hosts(); got != "127.0.0.1 qis.b-tu.de" {
		t.Errorf("hosts %q: a failure half an hour ago was forgotten", got)
	}
	clock.Advance(31 * time.Minute)
	u.sweep()
	if got := hosts(); got != "qis.b-tu.de" {
		t.Errorf("hosts %q, want only the configured host after an idle hour", got)
	}

	// A success after a failure resets the count, and the state goes with it.
	fetch("/fail")
	fetch("/")
	if got := hosts(); got != "qis.b-tu.de" || statesHeld(u) != 0 {
		t.Errorf("hosts %q (%d states held) after a success, want only the configured one", got, statesHeld(u))
	}
}

func statesHeld(u *Upstream) int {
	u.mu.Lock()
	defer u.mu.Unlock()
	return len(u.states)
}

func TestAHostInItsPauseIsKeptUntilThePauseIsOver(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "ok") })
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) { h.Pause = 50 * time.Millisecond })})

	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink}); err != nil {
		t.Fatalf("Fetch failed: %v", err)
	}
	if hs := u.HostStates(); len(hs) != 1 || hs[0].Host != "127.0.0.1" {
		t.Errorf("got %+v right after the request, want the host listed while its slot pauses", hs)
	}
	waitFor(t, "the state forgotten after the pause", func() bool { return statesHeld(u) == 0 })
	if hs := u.HostStates(); len(hs) != 0 {
		t.Errorf("got %+v, want no host after the pause", hs)
	}
}

func TestARefusedAddressLeavesNoHostState(t *testing.T) {
	u := openUpstream(t, Options{Policy: testPolicy(nil)}) // AllowPrivate false, as in production
	targets := []string{"http://[fd00::1]/", "http://[::ffff:10.0.0.1]:8080/", "http://[fe80::1%25eth0]/", "https://127.0.0.1/"}
	for i := 0; i < 300; i++ {
		targets = append(targets, fmt.Sprintf("http://10.%d.%d.1/", i/250, i%250))
	}
	for _, target := range targets {
		if _, err := u.Fetch(context.Background(), Request{URL: target, Sink: discardSink}); !errors.Is(err, ErrAddressNotAllowed) {
			t.Fatalf("%s: err %v, want ErrAddressNotAllowed", target, err)
		}
	}
	if n := statesHeld(u); n != 0 {
		t.Errorf("%d host states after %d refused addresses, want none", n, len(targets))
	}
	if hs := u.HostStates(); len(hs) != 0 {
		t.Errorf("HostStates lists %d hosts (the first %+v), want none", len(hs), hs[0])
	}
}

func TestHostStatesStayBoundedWhenEveryHostFails(t *testing.T) {
	clock := newClock()
	p := testPolicy(nil)
	p.Hosts["configured.example"] = p.Default
	u := newTestUpstream(t, Options{Policy: p, Now: clock.Now})
	hp := u.Policy().Default
	fail := func(host string, retryAfter time.Duration) {
		t.Helper()
		if err := u.acquire(context.Background(), host, u.Policy().For(host)); err != nil {
			t.Fatalf("acquire %s: %v", host, err)
		}
		clock.Advance(time.Millisecond)
		u.release(host, hp, false, failure, retryAfter, 0)
	}

	// Worth keeping: a host paused by its Retry-After, and a host the policy names.
	fail("paused.example", time.Hour)
	fail("configured.example", 0)
	// A client naming a new failing host in every request.
	for i := 0; i < 2*keepStates; i++ {
		fail(fmt.Sprintf("h%d.example", i), 0)
	}

	u.mu.Lock()
	n := len(u.states)
	_, paused := u.states["paused.example"]
	_, configured := u.states["configured.example"]
	_, newest := u.states[fmt.Sprintf("h%d.example", 2*keepStates-1)]
	_, oldest := u.states["h0.example"]
	u.mu.Unlock()
	if n > keepStates {
		t.Errorf("%d host states, want at most %d", n, keepStates)
	}
	if !paused || !configured {
		t.Errorf("paused kept %v, configured kept %v: want both kept over hosts that only failed", paused, configured)
	}
	if !newest || oldest {
		t.Errorf("newest kept %v, oldest kept %v: want the ones used longest ago to go", newest, oldest)
	}
	if hs := u.HostStates(); len(hs) > keepStates {
		t.Errorf("HostStates lists %d hosts, want at most %d", len(hs), keepStates)
	}
}

func TestForgettingIdleHostsKeepsTheFloorUnderLoad(t *testing.T) {
	var cur, peak atomic.Int64
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		n := cur.Add(1)
		defer cur.Add(-1)
		for p := peak.Load(); n > p && !peak.CompareAndSwap(p, n); p = peak.Load() {
		}
		switch r.URL.Query().Get("k") {
		case "0":
			_, _ = io.WriteString(w, "ok")
		case "1":
			w.WriteHeader(http.StatusInternalServerError)
		case "2":
			time.Sleep(time.Duration(rand.IntN(3)) * time.Millisecond)
			_, _ = io.WriteString(w, "slow")
		case "3":
			if conn, _, err := http.NewResponseController(w).Hijack(); err == nil {
				_, _ = conn.Write([]byte("HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nabc"))
				_ = conn.Close()
			}
		case "4":
			http.Redirect(w, r, "/?k=0", http.StatusFound)
		case "5":
			w.Header().Set("Retry-After", "0")
			w.WriteHeader(http.StatusTooManyRequests)
		}
	})
	u := newTestUpstream(t, Options{Policy: testPolicy(func(h *HostPolicy) {
		h.Concurrency = 2
		h.Pause = time.Millisecond
		h.QueueWait = 20 * time.Millisecond
		h.BreakerFailures = 1 << 30
		h.Timeout = time.Second
	})})

	stop := make(chan struct{})
	watched := make(chan struct{})
	go func() {
		defer close(watched)
		for {
			select {
			case <-stop:
				return
			default:
				_ = u.HostStates()
				_ = hostGauges()
				u.sweep()
				time.Sleep(time.Millisecond)
			}
		}
	}()
	var wg sync.WaitGroup
	for g := 0; g < 32; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < 30; i++ {
				ctx, cancel := context.WithCancel(context.Background())
				if rand.IntN(4) == 0 {
					time.AfterFunc(time.Duration(rand.IntN(2000))*time.Microsecond, cancel)
				}
				_, _ = u.Fetch(ctx, Request{URL: fmt.Sprintf("%s/?k=%d", f.srv.URL, rand.IntN(6)), Sink: discardSink})
				cancel()
			}
		}()
	}
	wg.Wait()
	close(stop)
	<-watched

	waitFor(t, "the host idle", func() bool {
		u.mu.Lock()
		defer u.mu.Unlock()
		s := u.states["127.0.0.1"]
		return s == nil || s.idle()
	})
	if p := peak.Load(); p > 2 {
		t.Errorf("%d requests at once at the host, want at most its concurrency 2", p)
	}
	// Nothing leaked: the next request gets a slot at once.
	if _, err := u.Fetch(context.Background(), Request{URL: f.srv.URL + "/?k=0", Sink: discardSink}); err != nil {
		t.Errorf("a request after the load: %v", err)
	}
}
