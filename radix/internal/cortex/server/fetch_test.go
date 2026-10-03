package server

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"mime"
	"mime/multipart"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/upstream"
)

func TestFetchModesAgainstTheAgeOfWhatIsStored(t *testing.T) {
	type want struct {
		status      int
		code        string
		body        string
		cacheStatus string
		upstream    int // requests upstream for this request
	}
	served := func(body, cs string, n int) want { return want{http.StatusOK, "", body, cs, n} }
	for _, state := range []struct {
		name    string
		prime   bool
		advance time.Duration
		modes   map[string]want
	}{
		{"fresh", true, 10 * time.Minute, map[string]want{
			"offline": served("v1", "Cortex; hit", 0),
			"cache":   served("v1", "Cortex; hit", 0),
			"refresh": served("v2", "Cortex; fwd=request; fwd-status=200; stored", 1),
		}},
		{"older than max_age", true, 2 * time.Hour, map[string]want{
			"offline": served("v1", "Cortex; hit", 0),
			"cache":   served("v2", "Cortex; fwd=stale; fwd-status=200; stored", 1),
			"refresh": served("v2", "Cortex; fwd=request; fwd-status=200; stored", 1),
		}},
		{"not stored", false, 0, map[string]want{
			"offline": {http.StatusGatewayTimeout, codeOfflineMiss, "", "", 0},
			"cache":   served("v2", "Cortex; fwd=miss; fwd-status=200; stored", 1),
			"refresh": served("v2", "Cortex; fwd=request; fwd-status=200; stored", 1),
		}},
	} {
		t.Run(state.name, func(t *testing.T) {
			host := newFakeHost(t)
			c := newTestCortex(t)
			for mode := range state.modes {
				host.set("/"+mode, page{body: "v1"})
				if state.prime {
					if resp, body := get(t, c.fetchURL(host.url("/"+mode))); resp.StatusCode != http.StatusOK || body != "v1" {
						t.Fatalf("priming %s: %d %q", mode, resp.StatusCode, body)
					}
				}
				host.set("/"+mode, page{body: "v2"})
			}
			c.clock.advance(state.advance)
			for mode, w := range state.modes {
				before := host.hitsOf("/" + mode)
				resp, body := get(t, c.fetchURL(host.url("/"+mode), "mode", mode))
				wantStatus(t, resp, body, w.status, w.code)
				if w.code == "" && (body != w.body || resp.Header.Get("Cache-Status") != w.cacheStatus) {
					t.Errorf("%s: body %q, Cache-Status %q; want %q, %q", mode, body, resp.Header.Get("Cache-Status"), w.body, w.cacheStatus)
				}
				if got := host.hitsOf("/"+mode) - before; got != w.upstream {
					t.Errorf("%s: %d requests upstream, want %d", mode, got, w.upstream)
				}
			}
		})
	}
}

func TestMaxAgeOfTheRequestOverridesTheHosts(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "v1"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))
	c.clock.advance(2 * time.Hour) // older than the host's 1h

	if resp, body := get(t, c.fetchURL(host.url("/p"), "max_age", "3h")); body != "v1" || resp.Header.Get("Cache-Status") != "Cortex; hit" {
		t.Errorf("max_age=3h: %q %q, want the stored version", body, resp.Header.Get("Cache-Status"))
	}
	host.set("/p", page{body: "v2"})
	if _, body := get(t, c.fetchURL(host.url("/p"), "max_age", "0")); body != "v2" {
		t.Errorf("max_age=0: %q, want a fetch", body)
	}
	if host.hitsOf("/p") != 2 {
		t.Errorf("%d requests upstream, want 2", host.hitsOf("/p"))
	}
}

func TestConcurrentMissesCauseOneRequestUpstream(t *testing.T) {
	host := newFakeHost(t)
	gate := make(chan struct{})
	host.set("/p", page{body: "the page", gate: gate, started: make(chan struct{})})
	c := newTestCortex(t)
	before := scrape(t, c)["cortex_coalesced_total"]

	const clients = 10
	var wg sync.WaitGroup
	bodies := make([]string, clients)
	for i := range clients {
		wg.Add(1)
		go func() {
			defer wg.Done()
			resp, err := http.Get(c.fetchURL(host.url("/p")))
			if err != nil {
				t.Error(err)
				return
			}
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			bodies[i] = fmt.Sprintf("%d %s", resp.StatusCode, data)
		}()
	}
	waitFor(t, "every client to join the fetch", func() bool {
		return scrape(t, c)["cortex_coalesced_total"]-before == clients-1
	})
	close(gate)
	wg.Wait()
	for i, b := range bodies {
		if b != "200 the page" {
			t.Errorf("client %d got %q", i, b)
		}
	}
	if n := host.hitsOf("/p"); n != 1 {
		t.Errorf("%d requests upstream, want 1", n)
	}
}

func TestAClientThatGivesUpDoesNotAbortTheDownloadOthersWaitFor(t *testing.T) {
	host := newFakeHost(t)
	gate, started := make(chan struct{}), make(chan struct{})
	host.set("/big", page{body: compressible(100_000), gate: gate, started: started})
	c := newTestCortex(t)
	before := scrape(t, c)["cortex_coalesced_total"]

	ctx, cancel := context.WithCancel(context.Background())
	quitter := make(chan error, 1)
	go func() {
		req, _ := http.NewRequestWithContext(ctx, http.MethodGet, c.fetchURL(host.url("/big")), nil)
		resp, err := http.DefaultClient.Do(req)
		if err == nil {
			resp.Body.Close()
		}
		quitter <- err
	}()
	<-started
	waiter := make(chan string, 1)
	go func() {
		resp, err := http.Get(c.fetchURL(host.url("/big")))
		if err != nil {
			waiter <- err.Error()
			return
		}
		defer resp.Body.Close()
		data, _ := io.ReadAll(resp.Body)
		waiter <- fmt.Sprintf("%d %d", resp.StatusCode, len(data))
	}()
	waitFor(t, "the second client to join", func() bool { return scrape(t, c)["cortex_coalesced_total"]-before == 1 })

	cancel()
	if err := <-quitter; err == nil {
		t.Fatal("the cancelled client got an answer")
	}
	time.Sleep(50 * time.Millisecond) // the server notices the client is gone
	close(gate)
	if got := <-waiter; got != "200 100000" {
		t.Errorf("the client that waited got %q", got)
	}
	if host.finishedOf("/big") != 1 || host.hitsOf("/big") != 1 {
		t.Errorf("upstream: %d requests, %d finished; want one download to its end", host.hitsOf("/big"), host.finishedOf("/big"))
	}
	if resp, _ := get(t, c.fetchURL(host.url("/big"), "mode", "offline")); resp.StatusCode != http.StatusOK {
		t.Errorf("offline after the download: %d, want it stored", resp.StatusCode)
	}
}

// waitFor polls cond for up to 5 s.
func waitFor(t *testing.T, what string, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for !cond() {
		if time.Now().After(deadline) {
			t.Fatalf("timed out waiting for %s", what)
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func TestAFailingHostGetsTheStoredVersionUnlessStaleIsNever(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "stored"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))
	c.clock.advance(2 * time.Hour)
	host.set("/p", page{status: http.StatusInternalServerError, body: "down"})

	resp, body := get(t, c.fetchURL(host.url("/p")))
	wantStatus(t, resp, body, http.StatusOK, "")
	if body != "stored" || resp.Header.Get("Cache-Status") != "Cortex; hit; detail=stale-if-error" ||
		resp.Header.Get("Cortex-Upstream-Error") != codeUpstreamFailed {
		t.Errorf("stale-if-error: %q, Cache-Status %q, Cortex-Upstream-Error %q", body, resp.Header.Get("Cache-Status"), resp.Header.Get("Cortex-Upstream-Error"))
	}

	resp, body = get(t, c.fetchURL(host.url("/p"), "stale", "never"))
	wantStatus(t, resp, body, http.StatusBadGateway, codeUpstreamFailed)
	if !strings.Contains(body, "500") {
		t.Errorf("the message does not name upstream's status: %s", body)
	}

	// Nothing of the failures was stored.
	if n := versionsOf(t, c, host.url("/p")); n != 1 {
		t.Errorf("%d versions, want 1", n)
	}
}

// versionsOf returns how many versions GET /v1/entries lists for target (0 for a 404).
func versionsOf(t *testing.T, c *testCortex, target string, params ...string) int {
	t.Helper()
	q := url.Values{"url": {target}}
	for i := 0; i+1 < len(params); i += 2 {
		q.Set(params[i], params[i+1])
	}
	resp, body := get(t, c.URL+"/v1/entries?"+q.Encode())
	if resp.StatusCode == http.StatusNotFound {
		return 0
	}
	var out struct {
		Versions []json.RawMessage `json:"versions"`
	}
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &out) != nil {
		t.Fatalf("entries of %s: %d %s", target, resp.StatusCode, body)
	}
	return len(out.Versions)
}

func TestExpectServesAMatchingVersionAtAnyAgeAndRefusesAnotherDownload(t *testing.T) {
	host := newFakeHost(t)
	const model = "weights of a model"
	host.set("/model", page{body: model})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/model")))
	c.clock.advance(48 * time.Hour)

	resp, body := get(t, c.fetchURL(host.url("/model"), "expect", "sha256:"+sha(model)))
	if resp.StatusCode != http.StatusOK || body != model || resp.Header.Get("Cache-Status") != "Cortex; hit" {
		t.Errorf("expect with the stored hash: %d %q %q, want a hit", resp.StatusCode, body, resp.Header.Get("Cache-Status"))
	}
	if host.hitsOf("/model") != 1 {
		t.Errorf("a matching version was fetched again")
	}

	resp, body = get(t, c.fetchURL(host.url("/model"), "expect", "sha256:"+sha("other weights")))
	wantStatus(t, resp, body, http.StatusBadGateway, codeHashMismatch)
	if host.hitsOf("/model") != 2 {
		t.Errorf("a version of another hash was not fetched")
	}
	if n := versionsOf(t, c, host.url("/model")); n != 1 {
		t.Errorf("%d versions after a mismatch, want 1 (nothing stored)", n)
	}

	// refresh with the right hash checks it upstream.
	resp, body = get(t, c.fetchURL(host.url("/model"), "mode", "refresh", "expect", "sha256:"+sha(model)))
	if resp.StatusCode != http.StatusOK || body != model || !strings.HasPrefix(resp.Header.Get("Cache-Status"), "Cortex; fwd=request") {
		t.Errorf("refresh with expect: %d %q %q", resp.StatusCode, body, resp.Header.Get("Cache-Status"))
	}
}

func TestAtServesTheVersionCurrentThen(t *testing.T) {
	host := newFakeHost(t)
	host.set("/doc", page{body: "first"})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/doc")))
	c.clock.advance(time.Hour)
	host.set("/doc", page{body: "second"})
	get(t, c.fetchURL(host.url("/doc"), "mode", "refresh"))

	for _, tc := range []struct {
		at   time.Time
		want string
	}{
		{t0.Add(30 * time.Minute), "first"},
		{t0.Add(2 * time.Hour), "second"},
		{t0.Add(-time.Hour), ""},
	} {
		// mode refresh is ignored: at is always offline.
		resp, body := get(t, c.fetchURL(host.url("/doc"), "at", tc.at.Format(time.RFC3339), "mode", "refresh"))
		if tc.want == "" {
			wantStatus(t, resp, body, http.StatusGatewayTimeout, codeOfflineMiss)
			continue
		}
		if resp.StatusCode != http.StatusOK || body != tc.want {
			t.Errorf("at %s: %d %q, want %q", tc.at, resp.StatusCode, body, tc.want)
		}
	}
	if host.hitsOf("/doc") != 2 {
		t.Errorf("at sent requests upstream: %d", host.hitsOf("/doc"))
	}
}

func TestNotFoundAndGoneAreStoredAndServedWithTheirStatus(t *testing.T) {
	host := newFakeHost(t)
	host.set("/gone", page{status: http.StatusGone, body: "this page is gone"})
	c := newTestCortex(t)

	for _, tc := range []struct {
		path   string
		status int
		body   string
	}{
		{"/missing", http.StatusNotFound, "404 page not found\n"},
		{"/gone", http.StatusGone, "this page is gone"},
	} {
		for i, cs := range []string{fmt.Sprintf("Cortex; fwd=miss; fwd-status=%d; stored", tc.status), "Cortex; hit"} {
			resp, body := get(t, c.fetchURL(host.url(tc.path)))
			if resp.StatusCode != tc.status || body != tc.body || resp.Header.Get("Cortex-Error") != "" ||
				resp.Header.Get("Cortex-Status") != strconv.Itoa(tc.status) || resp.Header.Get("Cache-Status") != cs {
				t.Errorf("%s, request %d: %d %q error %q Cortex-Status %q Cache-Status %q", tc.path, i+1, resp.StatusCode, body,
					resp.Header.Get("Cortex-Error"), resp.Header.Get("Cortex-Status"), resp.Header.Get("Cache-Status"))
			}
		}
		if host.hitsOf(tc.path) != 1 {
			t.Errorf("%s: %d requests upstream, want 1", tc.path, host.hitsOf(tc.path))
		}
	}
}

func TestServerErrorsOfTheHostAreNotStored(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{status: http.StatusServiceUnavailable, body: "maintenance"})
	c := newTestCortex(t)
	for range 2 {
		resp, body := get(t, c.fetchURL(host.url("/p")))
		wantStatus(t, resp, body, http.StatusBadGateway, codeUpstreamFailed)
		if !strings.Contains(body, "503") {
			t.Errorf("message does not name 503: %s", body)
		}
	}
	if host.hitsOf("/p") != 2 {
		t.Errorf("%d requests upstream, want 2 (nothing stored)", host.hitsOf("/p"))
	}
	if n := versionsOf(t, c, host.url("/p")); n != 0 {
		t.Errorf("%d versions stored, want none", n)
	}
}

func TestAcceptHeadersArePartOfTheKey(t *testing.T) {
	host := newFakeHost(t)
	host.set("/neg", page{body: "negotiated"})
	c := newTestCortex(t)

	get(t, c.fetchURL(host.url("/neg")), "Accept", "application/pdf")
	if got := host.lastHeader("/neg").Get("Accept"); got != "application/pdf" {
		t.Errorf("Accept upstream = %q", got)
	}
	get(t, c.fetchURL(host.url("/neg")), "Accept", "text/html", "Accept-Language", "de")
	if got := host.lastHeader("/neg").Get("Accept-Language"); got != "de" {
		t.Errorf("Accept-Language upstream = %q", got)
	}
	get(t, c.fetchURL(host.url("/neg")), "Accept", "application/pdf") // a hit
	if host.hitsOf("/neg") != 2 {
		t.Errorf("%d requests upstream, want 2 (one per Accept)", host.hitsOf("/neg"))
	}
	if versionsOf(t, c, host.url("/neg"), "accept", "application/pdf") != 1 ||
		versionsOf(t, c, host.url("/neg"), "accept", "text/html", "accept_language", "de") != 1 ||
		versionsOf(t, c, host.url("/neg")) != 0 {
		t.Error("the entries are not keyed by the Accept headers")
	}
}

func TestUserAgentGoesUpstreamButIsNotPartOfTheKey(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	c := newTestCortex(t)

	get(t, c.fetchURL(host.url("/p")), "User-Agent", "Radix/0.6", "Cookie", "session=1", "Authorization", "Bearer secret")
	up := host.lastHeader("/p")
	if up.Get("User-Agent") != "Radix/0.6" || up.Get("Cookie") != "" || up.Get("Authorization") != "" {
		t.Errorf("upstream headers %v: want the User-Agent, no Cookie, no Authorization", up)
	}
	if resp, _ := get(t, c.fetchURL(host.url("/p")), "User-Agent", "Other/1"); resp.Header.Get("Cache-Status") != "Cortex; hit" {
		t.Errorf("another User-Agent is not a hit: %q", resp.Header.Get("Cache-Status"))
	}

	// Without one, the policy's.
	req, _ := http.NewRequest(http.MethodGet, c.fetchURL(host.url("/p"), "mode", "refresh"), nil)
	req.Header.Set("User-Agent", "") // Go sends none
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if got := host.lastHeader("/p").Get("User-Agent"); got != upstream.DefaultUserAgent {
		t.Errorf("User-Agent without one = %q, want the policy's %q", got, upstream.DefaultUserAgent)
	}
}

func TestAServedVersionCarriesCortexsHeaders(t *testing.T) {
	host := newFakeHost(t)
	const lastModified = "Thu, 01 Oct 2026 08:00:00 GMT"
	host.set("/page", page{body: "<p>Modul</p>", etag: `"up-1"`, header: map[string]string{
		"Content-Type": "text/html; charset=utf-8", "Content-Language": "de", "Content-Disposition": "inline",
		"Last-Modified": lastModified, "Cache-Control": "no-store", "Vary": "Cookie", "X-Other": "dropped",
	}})
	c := newTestCortex(t)

	resp, body := get(t, c.fetchURL(host.url("/page")))
	h := resp.Header
	stamp := "2026-10-02T10:00:00.000000Z"
	for name, want := range map[string]string{
		"Content-Type": "text/html; charset=utf-8", "Content-Language": "de", "Content-Disposition": "inline",
		"Content-Length": "12", "ETag": `"sha256:` + sha("<p>Modul</p>") + `"`, "Last-Modified": lastModified, "Age": "0",
		"Cache-Status": "Cortex; fwd=miss; fwd-status=200; stored", "Cortex-Fetched-At": stamp, "Cortex-Checked-At": stamp,
		"Cortex-Status": "200", "Cortex-Upstream-ETag": `"up-1"`, "Cortex-Instance": "a; role=leader",
		"Cache-Control": "", "Vary": "", "X-Other": "",
	} {
		if got := h.Get(name); got != want {
			t.Errorf("%s = %q, want %q", name, got, want)
		}
	}
	if body != "<p>Modul</p>" || h.Get("Cortex-Version") == "" {
		t.Errorf("body %q, Cortex-Version %q", body, h.Get("Cortex-Version"))
	}

	c.clock.advance(10 * time.Minute)
	resp, _ = get(t, c.fetchURL(host.url("/page")))
	if resp.Header.Get("Age") != "600" || resp.Header.Get("Cache-Status") != "Cortex; hit" {
		t.Errorf("hit after 10 min: Age %q, Cache-Status %q", resp.Header.Get("Age"), resp.Header.Get("Cache-Status"))
	}

	// Older than max_age: a conditional request with upstream's validators, answered 304: a check.
	c.clock.advance(2 * time.Hour)
	resp, body = get(t, c.fetchURL(host.url("/page")))
	up := host.lastHeader("/page")
	if up.Get("If-None-Match") != `"up-1"` || up.Get("If-Modified-Since") != lastModified {
		t.Errorf("conditional request: If-None-Match %q, If-Modified-Since %q", up.Get("If-None-Match"), up.Get("If-Modified-Since"))
	}
	if resp.StatusCode != http.StatusOK || body != "<p>Modul</p>" ||
		resp.Header.Get("Cache-Status") != "Cortex; fwd=stale; fwd-status=304; stored" ||
		resp.Header.Get("Cortex-Fetched-At") != stamp || resp.Header.Get("Cortex-Checked-At") != "2026-10-02T12:10:00.000000Z" ||
		resp.Header.Get("Age") != "0" {
		t.Errorf("after a 304: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	if n := versionsOf(t, c, host.url("/page")); n != 1 {
		t.Errorf("%d versions after a 304, want 1", n)
	}
}

func TestLastModifiedIsTheFetchTimeWhenUpstreamSendsNone(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x", header: map[string]string{"Content-Type": ""}})
	c := newTestCortex(t)
	resp, _ := get(t, c.fetchURL(host.url("/p")))
	if got := resp.Header.Get("Last-Modified"); got != t0.Format(http.TimeFormat) {
		t.Errorf("Last-Modified = %q, want the fetch time", got)
	}
	if got := resp.Header.Get("Content-Type"); got != "" {
		t.Errorf("Content-Type = %q although upstream named none", got)
	}
}

func TestIfNoneMatchWithCortexsETagIsNotModified(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "content"})
	c := newTestCortex(t)
	resp, _ := get(t, c.fetchURL(host.url("/p")))
	etag := resp.Header.Get("ETag")
	before := scrape(t, c)[`cortex_requests_total{source="unknown",mode="cache",result="not_modified"}`]

	resp, body := get(t, c.fetchURL(host.url("/p")), "If-None-Match", `"other", `+etag)
	if resp.StatusCode != http.StatusNotModified || body != "" || resp.Header.Get("ETag") != etag {
		t.Errorf("If-None-Match: %d %q ETag %q", resp.StatusCode, body, resp.Header.Get("ETag"))
	}
	if got := scrape(t, c)[`cortex_requests_total{source="unknown",mode="cache",result="not_modified"}`] - before; got != 1 {
		t.Errorf("not_modified counted %v times", got)
	}
}

func TestRangeOnRawAndCompressedBlobs(t *testing.T) {
	host := newFakeHost(t)
	c := newTestCortex(t)
	for _, tc := range []struct {
		name, body string
		gzip       bool
	}{
		{"raw", randomText(4000, 1), false},
		{"gzip", compressible(20_000), true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			host.set("/"+tc.name, page{body: tc.body})
			resp, body := get(t, c.fetchURL(host.url("/"+tc.name)), "Range", "bytes=100-199")
			if resp.StatusCode != http.StatusPartialContent || body != tc.body[100:200] ||
				resp.Header.Get("Content-Range") != fmt.Sprintf("bytes 100-199/%d", len(tc.body)) {
				t.Errorf("range on the fetch: %d %q", resp.StatusCode, resp.Header.Get("Content-Range"))
			}
			if _, gz, _, err := c.st.OpenStored(sha(tc.body)); err != nil || gz != tc.gzip {
				t.Fatalf("stored gzip=%v (%v), want %v", gz, err, tc.gzip)
			}
			// From the store; backwards too (a second part before the first).
			resp, body = get(t, c.fetchURL(host.url("/"+tc.name)), "Range", "bytes=3000-3009,10-19")
			if resp.StatusCode != http.StatusPartialContent {
				t.Fatalf("multi-range: %d", resp.StatusCode)
			}
			parts := readParts(t, resp, body)
			if len(parts) != 2 || parts[0] != tc.body[3000:3010] || parts[1] != tc.body[10:20] {
				t.Errorf("multi-range parts %q", parts)
			}
			resp, body = get(t, c.fetchURL(host.url("/"+tc.name)), "Range", fmt.Sprintf("bytes=%d-", len(tc.body)-5))
			if resp.StatusCode != http.StatusPartialContent || body != tc.body[len(tc.body)-5:] {
				t.Errorf("suffix range: %d", resp.StatusCode)
			}
		})
	}
}

// readParts reads a multipart/byteranges answer.
func readParts(t *testing.T, resp *http.Response, body string) []string {
	t.Helper()
	_, params, err := mime.ParseMediaType(resp.Header.Get("Content-Type"))
	if err != nil {
		t.Fatalf("Content-Type %q: %v", resp.Header.Get("Content-Type"), err)
	}
	mr := multipart.NewReader(strings.NewReader(body), params["boundary"])
	var parts []string
	for {
		p, err := mr.NextPart()
		if err == io.EOF {
			return parts
		}
		if err != nil {
			t.Fatal(err)
		}
		data, _ := io.ReadAll(p)
		parts = append(parts, string(data))
	}
}

func TestHeadAnswersWithTheHeadersOnly(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "twelve bytes", header: map[string]string{"Content-Type": "text/plain"}})
	host.set("/gone", page{status: http.StatusGone, body: "gone"})
	c := newTestCortex(t)
	resp, body := do(t, http.MethodHead, c.fetchURL(host.url("/p")), nil)
	if resp.StatusCode != http.StatusOK || body != "" || resp.Header.Get("Content-Length") != "12" || resp.Header.Get("ETag") == "" {
		t.Errorf("HEAD: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	resp, body = do(t, http.MethodHead, c.fetchURL(host.url("/gone")), nil)
	if resp.StatusCode != http.StatusGone || body != "" || resp.Header.Get("Content-Length") != "4" {
		t.Errorf("HEAD of a 410: %d %q %v", resp.StatusCode, body, resp.Header)
	}
}

func TestUpstreamFailuresMapToCortexsErrors(t *testing.T) {
	closed := newFakeHost(t)
	closedURL := closed.url("/p")
	closed.Close()

	for _, tc := range []struct {
		name   string
		policy string
		checks bool // the dialer's address checks
		body   string
		header map[string]string
		status int
		code   string
	}{
		{"too large", `{"default": {"max_body": 10}}`, false, "more than ten bytes", nil, http.StatusBadGateway, codeTooLarge},
		{"wrong type", `{"hosts": {"127.0.0.1": {"expect_type": "application/pdf"}}}`, false, "<html>a challenge</html>",
			map[string]string{"Content-Type": "text/html"}, http.StatusBadGateway, codeWrongType},
		{"host not allowed", `{"allow": ["example.org"]}`, false, "x", nil, http.StatusForbidden, codeHostNotAllowed},
		{"address not allowed", testPolicy, true, "x", nil, http.StatusForbidden, codeAddressNotAllowed},
	} {
		t.Run(tc.name, func(t *testing.T) {
			host := newFakeHost(t)
			host.set("/p", page{body: tc.body, header: tc.header})
			opts := []func(*testConfig){withPolicy(tc.policy)}
			if tc.checks {
				opts = append(opts, withAddressChecks())
			}
			c := newTestCortex(t, opts...)
			resp, body := get(t, c.fetchURL(host.url("/p")))
			wantStatus(t, resp, body, tc.status, tc.code)
			if versionsOf(t, c, host.url("/p")) != 0 {
				t.Error("a refused answer was stored")
			}
		})
	}

	t.Run("network error", func(t *testing.T) {
		c := newTestCortex(t)
		resp, body := get(t, c.fetchURL(closedURL))
		wantStatus(t, resp, body, http.StatusBadGateway, codeUpstreamFailed)
	})

	t.Run("host paused", func(t *testing.T) {
		host := newFakeHost(t)
		host.set("/fail", page{status: http.StatusInternalServerError})
		c := newTestCortex(t, withPolicy(`{"default": {"breaker_failures": 1, "breaker_pause": "1h"}}`))
		resp, body := get(t, c.fetchURL(host.url("/fail")))
		wantStatus(t, resp, body, http.StatusBadGateway, codeUpstreamFailed)
		resp, body = get(t, c.fetchURL(host.url("/other")))
		wantStatus(t, resp, body, http.StatusServiceUnavailable, codeHostPaused)
		if after, _ := strconv.Atoi(resp.Header.Get("Retry-After")); after < 3500 || after > 3600 {
			t.Errorf("Retry-After = %q, want about an hour", resp.Header.Get("Retry-After"))
		}
		if host.hitsOf("/other") != 0 {
			t.Error("a paused host was asked")
		}
	})

	t.Run("host busy", func(t *testing.T) {
		host := newFakeHost(t)
		gate, started := make(chan struct{}), make(chan struct{})
		host.set("/slow", page{body: "slow", gate: gate, started: started})
		host.set("/next", page{body: "next"})
		c := newTestCortex(t, withPolicy(`{"default": {"concurrency": 1, "queue_wait": "200ms"}}`))
		slow := make(chan int, 1)
		go func() {
			resp, _ := get(t, c.fetchURL(host.url("/slow")))
			slow <- resp.StatusCode
		}()
		<-started
		resp, body := get(t, c.fetchURL(host.url("/next")))
		close(gate)
		wantStatus(t, resp, body, http.StatusTooManyRequests, codeHostBusy)
		if after, err := strconv.Atoi(resp.Header.Get("Retry-After")); err != nil || after < 1 {
			t.Errorf("Retry-After = %q, want at least 1", resp.Header.Get("Retry-After"))
		}
		if status := <-slow; status != http.StatusOK {
			t.Errorf("the slow request: %d", status)
		}
	})
}

func TestBadFetchParametersAreBadRequests(t *testing.T) {
	c := newTestCortex(t)
	for _, q := range []string{
		"",
		"url=ftp%3A%2F%2Fexample.org%2F",
		"url=https%3A%2F%2Fuser%3Apw%40example.org%2F",
		"url=%2Frelative",
		"url=https%3A%2F%2Fexample.org%2F&mode=online",
		"url=https%3A%2F%2Fexample.org%2F&max_age=soon",
		"url=https%3A%2F%2Fexample.org%2F&max_age=-1h",
		"url=https%3A%2F%2Fexample.org%2F&stale=always",
		"url=https%3A%2F%2Fexample.org%2F&expect=md5%3Aabc",
		"url=https%3A%2F%2Fexample.org%2F&expect=sha256%3AABC",
		"url=https%3A%2F%2Fexample.org%2F&at=yesterday",
		"url=https%3A%2F%2Fexample.org%2F&source=Module+Page",
	} {
		resp, body := get(t, c.URL+"/v1/fetch?"+q)
		if resp.StatusCode != http.StatusBadRequest || resp.Header.Get("Cortex-Error") != codeBadRequest {
			t.Errorf("?%s: %d %q %s", q, resp.StatusCode, resp.Header.Get("Cortex-Error"), body)
		}
	}
}

func TestTheRequestCountsBySourceModeAndResult(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	c := newTestCortex(t)
	series := func(mode, result string) string {
		return `cortex_requests_total{source="module_page",mode="` + mode + `",result="` + result + `"}`
	}
	before := scrape(t, c)
	get(t, c.fetchURL(host.url("/p"), "source", "module_page"))
	get(t, c.fetchURL(host.url("/p"), "source", "module_page"))
	get(t, c.fetchURL(host.url("/q"), "source", "module_page", "mode", "offline"))
	get(t, c.fetchURL(host.url("/p"), "source", "module_page", "mode", "refresh"))
	after := scrape(t, c)
	for s, want := range map[string]float64{
		series("cache", "miss"): 1, series("cache", "hit"): 1, series("offline", "offline_miss"): 1, series("refresh", "refresh"): 1,
	} {
		if got := after[s] - before[s]; got != want {
			t.Errorf("%s grew by %v, want %v", s, got, want)
		}
	}
}

func TestStoredEntriesAreListedPagedAndDeleted(t *testing.T) {
	host := newFakeHost(t)
	c := newTestCortex(t)
	for i, source := range []string{"lists", "lists", "module_page"} {
		path := fmt.Sprintf("/p%d", i)
		host.set(path, page{body: path})
		get(t, c.fetchURL(host.url(path), "source", source))
	}

	type listing struct {
		Entries []struct {
			URL     string `json:"url"`
			Source  string `json:"source"`
			Current struct {
				SHA256    string `json:"sha256"`
				FetchedAt string `json:"fetched_at"`
			} `json:"current"`
		} `json:"entries"`
		NextCursor string `json:"next_cursor"`
	}
	list := func(q string) listing {
		t.Helper()
		resp, body := get(t, c.URL+"/v1/entries?"+q)
		var l listing
		if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &l) != nil {
			t.Fatalf("?%s: %d %s", q, resp.StatusCode, body)
		}
		return l
	}
	if l := list("source=lists"); len(l.Entries) != 2 || l.NextCursor != "" || l.Entries[0].Current.SHA256 != "sha256:"+sha("/p0") ||
		l.Entries[0].Current.FetchedAt != "2026-10-02T10:00:00.000000Z" {
		t.Errorf("source=lists: %+v", l)
	}
	first := list("limit=2")
	second := list("limit=2&cursor=" + url.QueryEscape(first.NextCursor))
	if len(first.Entries) != 2 || first.NextCursor == "" || len(second.Entries) != 1 || second.NextCursor != "" {
		t.Errorf("paging: %+v then %+v", first, second)
	}
	if l := list("host=127.0.0.1&changed_since=" + url.QueryEscape(t0.Add(time.Minute).Format(time.RFC3339))); len(l.Entries) != 0 {
		t.Errorf("changed_since after every fetch: %+v", l)
	}
	for _, q := range []string{"cursor=x", "limit=-1", "changed_since=today"} {
		resp, body := get(t, c.URL+"/v1/entries?"+q)
		wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)
	}

	del := c.URL + "/v1/entries?url=" + url.QueryEscape(host.url("/p0"))
	if resp, body := do(t, http.MethodDelete, del, nil); resp.StatusCode != http.StatusNoContent {
		t.Errorf("DELETE: %d %s", resp.StatusCode, body)
	}
	resp, body := do(t, http.MethodDelete, del, nil)
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	resp, body = get(t, del)
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	if l := list(""); len(l.Entries) != 2 {
		t.Errorf("after the delete: %d entries", len(l.Entries))
	}
}

func TestTheVersionsOfAURL(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "one", header: map[string]string{"Content-Type": "text/plain"}})
	c := newTestCortex(t)
	get(t, c.fetchURL(host.url("/p")))
	c.clock.advance(time.Hour)
	host.set("/p", page{body: "two"})
	get(t, c.fetchURL(host.url("/p"), "mode", "refresh"))

	resp, body := get(t, c.URL+"/v1/entries?url="+url.QueryEscape(host.url("/p")))
	var out struct {
		Entry struct {
			URL            string `json:"url"`
			Host           string `json:"host"`
			CurrentVersion int64  `json:"current_version"`
		} `json:"entry"`
		Versions []struct {
			ID           int64               `json:"id"`
			Status       int                 `json:"status"`
			SHA256       string              `json:"sha256"`
			Size         int64               `json:"size"`
			Headers      map[string][]string `json:"headers"`
			FetchedAt    string              `json:"fetched_at"`
			CheckedAt    string              `json:"checked_at"`
			SupersededAt string              `json:"superseded_at"`
		} `json:"versions"`
	}
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &out) != nil {
		t.Fatalf("%d %s", resp.StatusCode, body)
	}
	v := out.Versions
	if len(v) != 2 || v[0].SHA256 != "sha256:"+sha("two") || v[0].SupersededAt != "" || out.Entry.CurrentVersion != v[0].ID ||
		v[1].SHA256 != "sha256:"+sha("one") || v[1].SupersededAt != "2026-10-02T11:00:00.000000Z" ||
		v[1].Headers["Content-Type"][0] != "text/plain" || out.Entry.Host != "127.0.0.1" {
		t.Errorf("versions: %s", body)
	}
}

func TestStoreFailuresOfAFetchAreInternalErrors(t *testing.T) {
	host := newFakeHost(t)
	host.set("/p", page{body: "x"})
	c := newTestCortex(t)
	_ = c.st.Close()
	resp, body := get(t, c.fetchURL(host.url("/p")))
	wantStatus(t, resp, body, http.StatusInternalServerError, codeInternal)
}

// Review 2 (validators-final-hop-sent-to-first-hop): the version of /latest, fetched through a
// redirect to /v3, kept /v3's validators, and the next refresh sent them to /latest: a 304 from
// /latest then confirmed /v3's body as /latest's. They go only to the URL they came from now.
func TestValidatorsGoOnlyToTheURLTheyCameFrom(t *testing.T) {
	host := newFakeHost(t)
	host.set("/latest", page{status: http.StatusFound, header: map[string]string{"Location": "/v3"}})
	host.set("/v3", page{body: "the body of v3", etag: `"x"`})
	c := newTestCortex(t)
	if resp, body := get(t, c.fetchURL(host.url("/latest"))); resp.StatusCode != http.StatusOK || body != "the body of v3" {
		t.Fatalf("first fetch: %d %q", resp.StatusCode, body)
	}

	// /latest answers itself now, with an ETag that happens to be the same.
	host.set("/latest", page{body: "the body of latest", etag: `"x"`})
	resp, body := get(t, c.fetchURL(host.url("/latest"), "mode", "refresh"))
	if resp.StatusCode != http.StatusOK || body != "the body of latest" {
		t.Fatalf("refresh: %d %q (%s): /v3's body confirmed as /latest's", resp.StatusCode, body, resp.Header.Get("Cache-Status"))
	}
	if v := host.lastHeader("/latest").Get("If-None-Match"); v != "" {
		t.Fatalf("/v3's validator went to /latest: If-None-Match %q", v)
	}
	// Its own validators do go to it, and its 304 is taken.
	resp, body = get(t, c.fetchURL(host.url("/latest"), "mode", "refresh"))
	if resp.StatusCode != http.StatusOK || body != "the body of latest" || host.lastHeader("/latest").Get("If-None-Match") != `"x"` {
		t.Fatalf("second refresh: %d %q, If-None-Match %q", resp.StatusCode, body, host.lastHeader("/latest").Get("If-None-Match"))
	}
	// The internal header is never shown.
	if resp, body := get(t, c.URL+"/v1/entries"); resp.StatusCode != http.StatusOK || strings.Contains(body, "Cortex-Final-Url") {
		t.Fatalf("entries: %d %s", resp.StatusCode, body)
	}
}

// Review 2 (sink-store-failure-mapped-upstream): a store that could not keep a downloaded body
// (a full disk) was answered 502 upstream-failed and logged at WARN, blaming the host. It is
// Cortex's own failure: 500 internal, logged at ERROR.
func TestAStoreFailureWhileDownloadingIsCortexsOwn(t *testing.T) {
	var out bytes.Buffer
	previous := slog.Default()
	slog.SetDefault(slog.New(slog.NewJSONHandler(&out, &slog.HandlerOptions{Level: slog.LevelDebug})))
	t.Cleanup(func() { slog.SetDefault(previous) })

	host := newFakeHost(t)
	host.set("/p", page{body: "a page"})
	c := newTestCortex(t)
	tmp := filepath.Join(c.st.Dir(), "tmp")
	if err := os.RemoveAll(tmp); err != nil { // where PutBlob writes: it fails as on a full disk
		t.Fatal(err)
	}
	resp, body := get(t, c.fetchURL(host.url("/p")))
	wantStatus(t, resp, body, http.StatusInternalServerError, codeInternal)
	var logged map[string]any
	for _, line := range strings.Split(strings.TrimSpace(out.String()), "\n") {
		var rec map[string]any
		if json.Unmarshal([]byte(line), &rec) == nil && rec["event"] == "upstream.failed" {
			logged = rec
		}
	}
	if logged == nil || logged["level"] != "ERROR" || logged["code"] != codeInternal {
		t.Fatalf("upstream.failed: %v", logged)
	}

	// A body that is not what was expected is still the host's.
	if err := os.Mkdir(tmp, 0o755); err != nil {
		t.Fatal(err)
	}
	resp, body = get(t, c.fetchURL(host.url("/p"), "expect", "sha256:"+sha("another body")))
	wantStatus(t, resp, body, http.StatusBadGateway, codeHashMismatch)
}
