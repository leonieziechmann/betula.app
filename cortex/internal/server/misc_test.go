package server

import (
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/metrics"
)

func TestBoundRangesDropsRangesThatGoBackOrAreTooMany(t *testing.T) {
	many := make([]string, maxRanges+1)
	for i := range many {
		many[i] = fmt.Sprintf("%d-%d", 2*i, 2*i)
	}
	for _, tc := range []struct {
		in   string
		keep bool
	}{
		{"", true},
		{"bytes=5-9", true},
		{"bytes=-1", true},
		{"bytes=100-", true},
		{"bytes=0-0, 10-19,20-", true},
		{"bytes=" + strings.Join(many[:maxRanges], ","), true},
		{"bytes=garbage,0-1", true}, // malformed: net/http answers 416
		{"items=0-1,0-1", true},     // not bytes: net/http ignores it
		{"bytes=" + strings.Join(many, ","), false},
		{"bytes=10-19,0-0", true}, // two in any order: one backward seek at most
		{"bytes=0-0,-1", true},
		{"bytes=0-0,5-5,-1", false},      // a suffix range among several
		{"bytes=10-19,20-29,0-0", false}, // backwards
		{"bytes=0-9,5-14,20-29", false},  // overlapping
		{"bytes=0-9,9-14,20-29", false},
		{"bytes=0-,10-19,20-29", false}, // after one that runs to the end
		{"bytes=0-0,4194303-4194303,0-0,4194303-4194303", false},
	} {
		h := http.Header{}
		if tc.in != "" {
			h.Set("Range", tc.in)
		}
		boundRanges(h)
		if got := h.Get("Range"); (got == tc.in) != tc.keep {
			t.Errorf("%.60q: Range %.60q afterwards, keep %v", tc.in, got, tc.keep)
		}
	}
}

// putGzipFile stores content (compressible, so stored gzip) as a named file.
func putGzipFile(t *testing.T, c *testCortex, name, content string) {
	t.Helper()
	resp, body := do(t, http.MethodPut, c.URL+"/v1/files/"+name, strings.NewReader(content))
	if resp.StatusCode != http.StatusCreated {
		t.Fatalf("PUT %s: %d %s", name, resp.StatusCode, body)
	}
	if b, err := c.st.StatBlob(sha(content)); err != nil || !b.Gzip {
		t.Fatalf("blob of %s: %+v %v, want stored gzip", name, b, err)
	}
}

// rangeGet asks for ranges of target without letting the transport ask for gzip.
func rangeGet(t *testing.T, target, ranges string) (*http.Response, string) {
	t.Helper()
	return get(t, target, "Range", ranges, "Accept-Encoding", "identity")
}

func TestAGzipFileAnswersAlternatingRangesWithTheWholeContent(t *testing.T) {
	c := newTestCortex(t)
	content := compressible(1 << 20)
	putGzipFile(t, c, "big.txt", content)
	last := len(content) - 1

	// 500 pairs of a byte at the start and one at the end: each pair used to decompress the
	// whole blob again.
	parts := make([]string, 0, 1000)
	for range 500 {
		parts = append(parts, "0-0", fmt.Sprintf("%d-%d", last, last))
	}
	start := time.Now()
	resp, body := rangeGet(t, c.URL+"/v1/files/big.txt", "bytes="+strings.Join(parts, ","))
	if resp.StatusCode != http.StatusOK || body != content {
		t.Errorf("alternating ranges: %d, %d bytes, want 200 and the whole %d", resp.StatusCode, len(body), len(content))
	}
	t.Logf("alternating ranges answered in %s", time.Since(start))

	// What a client asks for honestly still gets its parts.
	resp, body = rangeGet(t, c.URL+"/v1/files/big.txt", fmt.Sprintf("bytes=%d-%d", last-9, last))
	if resp.StatusCode != http.StatusPartialContent || body != content[last-9:] {
		t.Errorf("one range: %d %q", resp.StatusCode, body)
	}
	resp, body = rangeGet(t, c.URL+"/v1/files/big.txt", fmt.Sprintf("bytes=0-4,%d-%d", last-4, last))
	if resp.StatusCode != http.StatusPartialContent || !strings.HasPrefix(resp.Header.Get("Content-Type"), "multipart/byteranges") ||
		!strings.Contains(body, content[:5]) || !strings.Contains(body, content[last-4:]) {
		t.Errorf("ascending ranges: %d %s", resp.StatusCode, resp.Header.Get("Content-Type"))
	}
}

func TestInflatingStopsDecompressingAgainAndAgain(t *testing.T) {
	c := newTestCortex(t)
	content := compressible(256 << 10)
	b, err := c.st.PutBlob(strings.NewReader(content), "", 0)
	if err != nil || !b.Gzip {
		t.Fatalf("PutBlob: %+v %v", b, err)
	}
	r, err := c.srv.openBlob(b.Hash, b.Size)
	if err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	one := make([]byte, 1)
	var readErr error
	pairs := 0
	for ; pairs < 100 && readErr == nil; pairs++ {
		for _, off := range []int64{b.Size - 1, 0} {
			if _, err := r.Seek(off, io.SeekStart); err != nil {
				t.Fatal(err)
			}
			if _, readErr = io.ReadFull(r, one); readErr != nil {
				break
			}
			if one[0] != content[off] {
				t.Fatalf("byte %d: %q, want %q", off, one[0], content[off])
			}
		}
	}
	// Each pair decompresses the whole blob once more.
	if most := int(inflateBudget(b.Size)/b.Size) + 1; !errors.Is(readErr, errInflateBudget) || pairs > most {
		t.Errorf("after %d pairs of seeks: %v, want %v within %d", pairs, readErr, errInflateBudget, most)
	}

	// Reading forward, even after a peek at the start (http.ServeContent's sniffing), is not cut.
	r2, err := c.srv.openBlob(b.Hash, b.Size)
	if err != nil {
		t.Fatal(err)
	}
	defer r2.Close()
	if _, err := io.ReadFull(r2, make([]byte, 512)); err != nil {
		t.Fatal(err)
	}
	if _, err := r2.Seek(0, io.SeekStart); err != nil {
		t.Fatal(err)
	}
	if got, err := io.ReadAll(r2); err != nil || string(got) != content {
		t.Errorf("read after a peek: %d bytes, %v", len(got), err)
	}
}

func TestSourceCounterNamesAtMostItsLimit(t *testing.T) {
	c := newSourceCounter(metrics.NewRegistry().NewCounter("t_total", "test", "source", "mode", "result"), 3)
	got := []string{}
	for _, s := range []string{"a", "b", "a", "c", "d", "other", "b", "e"} {
		got = append(got, c.label(s))
	}
	if want := "a b a c other other b other"; strings.Join(got, " ") != want {
		t.Errorf("labels %q, want %q", strings.Join(got, " "), want)
	}
}

func TestTheSourceLabelOfFetchesIsBounded(t *testing.T) {
	// The set of sources is the process's; give it back so that later tests see their own.
	requestsTotal.mu.Lock()
	saved := make(map[string]bool, len(requestsTotal.seen))
	for k := range requestsTotal.seen {
		saved[k] = true
	}
	requestsTotal.mu.Unlock()
	t.Cleanup(func() {
		requestsTotal.mu.Lock()
		requestsTotal.seen = saved
		requestsTotal.mu.Unlock()
	})

	c := newTestCortex(t)
	for i := range 2 * maxSources {
		// No url: a 400, counted all the same.
		resp, _ := get(t, c.URL+fmt.Sprintf("/v1/fetch?source=job-%d", i))
		if resp.StatusCode != http.StatusBadRequest {
			t.Fatalf("status %d", resp.StatusCode)
		}
	}
	sources := map[string]bool{}
	for series := range scrape(t, c) {
		if rest, ok := strings.CutPrefix(series, `cortex_requests_total{source="`); ok {
			sources[rest[:strings.IndexByte(rest, '"')]] = true
		}
	}
	if len(sources) > maxSources+1 || !sources[otherSource] || !sources["job-0"] || sources[fmt.Sprintf("job-%d", 2*maxSources-1)] {
		t.Errorf("%d source labels (other: %v), want at most %d and other", len(sources), sources[otherSource], maxSources+1)
	}
	if got := scrape(t, c)[`cortex_requests_total{source="other",mode="cache",result="error"}`]; got < float64(maxSources) {
		t.Errorf("%v requests counted as other, want at least %d", got, maxSources)
	}
}

func TestTheHTTPServerClosesIdleConnectionsButNotLongAnswers(t *testing.T) {
	c := newTestCortex(t)
	hs := c.srv.httpServer()
	if hs.IdleTimeout != 2*time.Minute || hs.ReadHeaderTimeout <= 0 || hs.ReadTimeout != 0 || hs.WriteTimeout != 0 {
		t.Errorf("idle %s, read header %s, read %s, write %s; want 2m, set, none, none",
			hs.IdleTimeout, hs.ReadHeaderTimeout, hs.ReadTimeout, hs.WriteTimeout)
	}
}

func TestCortexInstanceNamesTheRoleThatAnswered(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "fetched by b"})
	leader, follower, node := newPair(t)
	leader.Close()
	go func() {
		time.Sleep(300 * time.Millisecond)
		node.promote()
	}()
	// The request finds b a follower; b takes over while it waits and fetches as the leader.
	resp, body := get(t, follower.fetchURL(host.url("/p")))
	if resp.StatusCode != http.StatusOK || body != "fetched by b" || resp.Header.Get("Cortex-Instance") != "b; role=leader" {
		t.Errorf("after the take-over: %d %q, Cortex-Instance %q", resp.StatusCode, body, resp.Header.Get("Cortex-Instance"))
	}
}

func TestStatusWriterLeavesAnInstanceAHandlerSet(t *testing.T) {
	role := "follower"
	rec := httptest.NewRecorder()
	early := "b; role=follower"
	w := &statusWriter{ResponseWriter: rec, status: http.StatusOK, instance: func() string { return "b; role=" + role }, early: early}
	w.Header().Set("Cortex-Instance", early)
	w.Header().Set("Cortex-Instance", "a; role=leader") // a forwarded answer: the leader's
	role = "leader"
	w.WriteHeader(http.StatusOK)
	if got := rec.Header().Get("Cortex-Instance"); got != "a; role=leader" {
		t.Errorf("Cortex-Instance %q, want the leader's", got)
	}
}
