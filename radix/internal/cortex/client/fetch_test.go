package client

import (
	"context"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync"
	"testing"
	"time"
)

// recorder is a fake Cortex that keeps the requests it received and answers each with 200.
type recorder struct {
	srv  *httptest.Server
	mu   sync.Mutex
	reqs []*http.Request
}

func newRecorder(t *testing.T) *recorder {
	t.Helper()
	rec := &recorder{}
	rec.srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		rec.mu.Lock()
		rec.reqs = append(rec.reqs, r.Clone(context.Background()))
		rec.mu.Unlock()
		w.Header().Set("Cortex-Checked-At", "2026-10-02T08:15:00.123456Z")
		w.Header().Set("Content-Type", "text/html")
		_, _ = io.WriteString(w, "<html>page</html>")
	}))
	t.Cleanup(rec.srv.Close)
	return rec
}

func (rec *recorder) last(t *testing.T) *http.Request {
	t.Helper()
	rec.mu.Lock()
	defer rec.mu.Unlock()
	if len(rec.reqs) == 0 {
		t.Fatal("Cortex received no request")
	}
	return rec.reqs[len(rec.reqs)-1]
}

// The transport sends a request for a page to Cortex instead, with the page's URL byte for
// byte and the job's source, passes on the headers the host gets, and hands the answer back
// as the answer to the original request, which stays as it was.
func TestTransportRewritesARequestIntoAFetch(t *testing.T) {
	rec := newRecorder(t)
	c := newTestClient(t, time.Second, rec.srv.URL)
	hc := c.HTTPClient(FetchOptions{Mode: ModeCache, MaxAge: time.Hour, Stale: StaleNever}, time.Minute)

	page := "https://www.b-tu.de/qisserver3/rds?state=wtree&nodeID=auswahlBaum%7Cstudiengang%3Astg%3D079&P_anzahl=10"
	req, err := http.NewRequestWithContext(WithSource(context.Background(), "qis_tree"), http.MethodGet, page+"#top", nil)
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("User-Agent", "Betula-Radix/1.0 (+https://betula.app; info@betula.app)")
	req.Header.Add("Accept", "text/html")
	req.Header.Add("Accept", "*/*;q=0.1")
	req.Header.Set("Accept-Language", "de")
	req.Header.Set("Cookie", "session=secret")
	req.Header.Set("Authorization", "Bearer secret")
	before := req.URL.String()

	resp, err := hc.Do(req)
	if err != nil {
		t.Fatalf("Do: %v", err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	// (http.Client hands its RoundTripper a copy of a request it puts a deadline on.)
	if resp.StatusCode != 200 || string(body) != "<html>page</html>" || resp.Request.URL.String() != page+"#top" {
		t.Errorf("answer = %d %q to %s", resp.StatusCode, body, resp.Request.URL)
	}
	direct, err := c.Transport(FetchOptions{}).RoundTrip(req)
	if err != nil {
		t.Fatalf("RoundTrip: %v", err)
	}
	direct.Body.Close()
	if direct.Request != req {
		t.Error("the answer's Request is not the original request")
	}
	rec.mu.Lock()
	rec.reqs = rec.reqs[:1]
	rec.mu.Unlock()
	if want := time.Date(2026, 10, 2, 8, 15, 0, 123456000, time.UTC); !CheckedAt(resp.Header).Equal(want) {
		t.Errorf("CheckedAt = %v, want %v", CheckedAt(resp.Header), want)
	}
	if req.URL.String() != before || req.URL.Host != "www.b-tu.de" || len(req.Header) != 5 {
		t.Errorf("the original request changed: %s, %v", req.URL, req.Header)
	}

	got := rec.last(t)
	wantQuery := "url=" + url.QueryEscape(page) + "&mode=cache&max_age=1h&stale=never&source=qis_tree"
	if got.Method != http.MethodGet || got.URL.Path != "/v1/fetch" || got.URL.RawQuery != wantQuery {
		t.Errorf("Cortex got %s %s?%s\nwant /v1/fetch?%s", got.Method, got.URL.Path, got.URL.RawQuery, wantQuery)
	}
	if got.URL.Query().Get("url") != page {
		t.Errorf("url = %q, want %q byte for byte", got.URL.Query().Get("url"), page)
	}
	for name, want := range map[string]string{
		"User-Agent":      "Betula-Radix/1.0 (+https://betula.app; info@betula.app)",
		"Accept":          "text/html, */*;q=0.1",
		"Accept-Language": "de",
		"Cookie":          "",
		"Authorization":   "",
	} {
		if v := got.Header.Get(name); v != want {
			t.Errorf("Cortex got %s: %q, want %q", name, v, want)
		}
	}
}

// Without a User-Agent of the request none goes to Cortex, rather than Go's: Cortex then
// sends its own to the host. The transport's options fill in what the request leaves out.
func TestTransportDefaults(t *testing.T) {
	rec := newRecorder(t)
	c := newTestClient(t, time.Second, rec.srv.URL)
	ctx := context.Background()
	send := func(o FetchOptions, ctx context.Context, method string) *http.Request {
		t.Helper()
		req, _ := http.NewRequestWithContext(ctx, method, "http://example.org/a b?x=1", nil)
		resp, err := c.HTTPClient(o, 0).Do(req)
		if err != nil {
			t.Fatalf("Do: %v", err)
		}
		resp.Body.Close()
		return rec.last(t)
	}

	got := send(FetchOptions{MaxAge: -1}, ctx, http.MethodGet)
	if _, ok := got.Header["User-Agent"]; ok {
		t.Errorf("User-Agent = %q, want none", got.Header.Get("User-Agent"))
	}
	if q := got.URL.RawQuery; q != "url=http%3A%2F%2Fexample.org%2Fa%2520b%3Fx%3D1&source=unknown" {
		t.Errorf("query = %s", q)
	}

	at := time.Date(2026, 9, 1, 12, 0, 0, 0, time.FixedZone("CEST", 2*3600))
	got = send(FetchOptions{Mode: ModeOffline, MaxAge: 0, Stale: StaleIfError, Expect: "sha256:ab", At: at, Source: "model",
		Accept: "application/pdf", AcceptLanguage: "en", UserAgent: "Folia"}, ctx, http.MethodHead)
	q := got.URL.Query()
	if got.Method != http.MethodHead || q.Get("mode") != "offline" || q.Get("max_age") != "0" || q.Get("stale") != "if-error" ||
		q.Get("expect") != "sha256:ab" || q.Get("at") != "2026-09-01T10:00:00Z" || q.Get("source") != "model" {
		t.Errorf("Cortex got %s ?%s", got.Method, got.URL.RawQuery)
	}
	if got.Header.Get("Accept") != "application/pdf" || got.Header.Get("Accept-Language") != "en" || got.Header.Get("User-Agent") != "Folia" {
		t.Errorf("headers = %v", got.Header)
	}

	// The source of the context wins over the transport's.
	got = send(FetchOptions{Source: "model"}, WithSource(ctx, "statute"), http.MethodGet)
	if s := got.URL.Query().Get("source"); s != "statute" {
		t.Errorf("source = %q, want the context's", s)
	}
}

func TestTransportRefusesWhatCortexCannotFetch(t *testing.T) {
	rec := newRecorder(t)
	c := newTestClient(t, time.Second, rec.srv.URL)
	hc := c.HTTPClient(FetchOptions{}, 0)

	body := &closeTracker{Reader: strings.NewReader("data")}
	req, _ := http.NewRequest(http.MethodPost, "https://generativelanguage.googleapis.com/v1/models", body)
	if _, err := hc.Do(req); err == nil || !strings.Contains(err.Error(), "only GET and HEAD") {
		t.Errorf("POST: err = %v", err)
	}
	if !body.closed {
		t.Error("the body of a refused request was not closed")
	}
	if _, err := c.Transport(FetchOptions{}).RoundTrip(&http.Request{Method: http.MethodGet, URL: &url.URL{Path: "/relative"}, Header: http.Header{}}); err == nil {
		t.Error("a relative URL was fetched")
	}
	if _, err := c.Fetch(context.Background(), "file:///etc/passwd", FetchOptions{}); err == nil {
		t.Error("a file URL was fetched")
	}
	rec.mu.Lock()
	defer rec.mu.Unlock()
	if len(rec.reqs) != 0 {
		t.Errorf("Cortex got %d requests", len(rec.reqs))
	}
}

type closeTracker struct {
	io.Reader
	closed bool
}

func (c *closeTracker) Close() error {
	c.closed = true
	return nil
}

func TestFetchSendsItsOptions(t *testing.T) {
	rec := newRecorder(t)
	c := newTestClient(t, time.Second, rec.srv.URL)
	resp, err := c.Fetch(WithSource(context.Background(), "ignored"), "https://huggingface.co/model.bin",
		FetchOptions{Mode: ModeRefresh, MaxAge: 90 * time.Minute, Expect: "sha256:00ff", Source: "model", UserAgent: "Radix"})
	if err != nil {
		t.Fatalf("Fetch: %v", err)
	}
	resp.Body.Close()
	got := rec.last(t)
	want := "url=https%3A%2F%2Fhuggingface.co%2Fmodel.bin&mode=refresh&max_age=1h30m&expect=sha256%3A00ff&source=model"
	if got.URL.RawQuery != want || got.Header.Get("User-Agent") != "Radix" {
		t.Errorf("Cortex got ?%s (User-Agent %q)\nwant ?%s", got.URL.RawQuery, got.Header.Get("User-Agent"), want)
	}

	resp, err = c.Fetch(WithSource(context.Background(), "statute"), "https://opus4.kobv.de/x.pdf", FetchOptions{MaxAge: -1})
	if err != nil {
		t.Fatalf("Fetch: %v", err)
	}
	resp.Body.Close()
	if s := rec.last(t).URL.Query().Get("source"); s != "statute" {
		t.Errorf("source = %q, want the context's when the options name none", s)
	}
}

func TestCheckedAtAndErrorCode(t *testing.T) {
	for _, tc := range []struct {
		value string
		want  time.Time
	}{
		{"2026-10-02T08:15:00.123456Z", time.Date(2026, 10, 2, 8, 15, 0, 123456000, time.UTC)},
		{"2026-10-02T10:15:00+02:00", time.Date(2026, 10, 2, 8, 15, 0, 0, time.UTC)},
		{"", time.Time{}},
		{"yesterday", time.Time{}},
	} {
		h := http.Header{}
		if tc.value != "" {
			h.Set("Cortex-Checked-At", tc.value)
		}
		if got := CheckedAt(h); !got.Equal(tc.want) {
			t.Errorf("CheckedAt(%q) = %v, want %v", tc.value, got, tc.want)
		}
	}

	if ErrorCode(nil) != "" || ErrorCode(&http.Response{Header: http.Header{}}) != "" {
		t.Error("ErrorCode of an answer without Cortex-Error is not empty")
	}
	if got := ErrorCode(&http.Response{Header: http.Header{"Cortex-Error": {"wrong-type"}}}); got != "wrong-type" {
		t.Errorf("ErrorCode = %q", got)
	}
}

// ResponseError tells an answer Cortex gave itself from the host's, whose body it leaves
// alone.
func TestResponseError(t *testing.T) {
	if ResponseError(nil) != nil {
		t.Error("ResponseError(nil) is not nil")
	}
	hostBody := &closeTracker{Reader: strings.NewReader("<html>gone</html>")}
	host := &http.Response{StatusCode: http.StatusNotFound, Header: http.Header{"Cortex-Status": {"404"}}, Body: hostBody}
	if err := ResponseError(host); err != nil {
		t.Errorf("ResponseError of the host's 404 = %v, want nil", err)
	}
	if rest, _ := io.ReadAll(host.Body); string(rest) != "<html>gone</html>" || hostBody.closed {
		t.Errorf("the host's body was touched: %q left, closed %v", rest, hostBody.closed)
	}

	own := &closeTracker{Reader: strings.NewReader(`{"error":"not-found","message":"no such endpoint: /v1/v1/fetch"}`)}
	err := ResponseError(&http.Response{StatusCode: http.StatusNotFound, Header: http.Header{"Cortex-Error": {"not-found"}}, Body: own})
	var cerr *Error
	if !errors.As(err, &cerr) || cerr.StatusCode != 404 || cerr.Code != "not-found" || cerr.Message != "no such endpoint: /v1/v1/fetch" ||
		err.Error() != "cortex: 404 not-found: no such endpoint: /v1/v1/fetch" || !errors.Is(err, ErrNotFound) {
		t.Errorf("ResponseError of Cortex's own 404 = %#v", err)
	}
	if !own.closed {
		t.Error("the body of Cortex's answer was not closed")
	}
}

func TestFormatDuration(t *testing.T) {
	for d, want := range map[time.Duration]string{
		0:                       "0",
		time.Hour:               "1h",
		720 * time.Hour:         "720h",
		90 * time.Minute:        "1h30m",
		70 * time.Minute:        "1h10m",
		10 * time.Minute:        "10m",
		90 * time.Second:        "1m30s",
		1500 * time.Millisecond: "1.5s",
		500 * time.Millisecond:  "500ms",
	} {
		if got := formatDuration(d); got != want {
			t.Errorf("formatDuration(%v) = %q, want %q", d, got, want)
		} else if back, err := time.ParseDuration(got); err != nil || back != d {
			t.Errorf("ParseDuration(%q) = %v, %v", got, back, err)
		}
	}
}
