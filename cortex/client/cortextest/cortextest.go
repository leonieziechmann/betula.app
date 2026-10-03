// Package cortextest is a fake Cortex for the tests of its clients, as httptest is a fake
// HTTP server. Its GET /v1/fetch asks the URL it names directly, with the request headers
// Cortex passes on, and answers with what the host said and the headers Cortex adds
// (docs/cortex/cortex.md). It stores nothing: every fetch goes to the host, an offline one misses,
// and a test can make every fetch answer with one of Cortex's errors instead.
package cortextest

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"sync"
	"time"
)

// TimeFormat is how Cortex writes its times: RFC 3339 in UTC, to the microsecond.
const TimeFormat = "2006-01-02T15:04:05.000000Z07:00"

// Fetch is one request to GET /v1/fetch as the server received it.
type Fetch struct {
	URL, Mode, MaxAge, Stale, Expect, At, Source string
	Query                                        url.Values  // every parameter
	Header                                       http.Header // the request headers
}

// Server is a running fake Cortex. Close it when done.
type Server struct {
	*httptest.Server

	mu         sync.Mutex
	fetches    []Fetch
	checkedAt  time.Time
	failStatus int
	failCode   string
}

// NewServer starts a fake Cortex.
func NewServer() *Server {
	s := &Server{}
	s.Server = httptest.NewServer(http.HandlerFunc(s.serve))
	return s
}

// Fetches returns the requests to GET /v1/fetch so far, in order.
func (s *Server) Fetches() []Fetch {
	s.mu.Lock()
	defer s.mu.Unlock()
	return append([]Fetch(nil), s.fetches...)
}

// CheckedAt makes every answer say that Cortex last fetched it at t (Cortex-Checked-At), as
// for an answer from its store; zero: the time of the request, as for a fresh fetch.
func (s *Server) CheckedAt(t time.Time) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.checkedAt = t
}

// Fail makes every fetch answer with Cortex's error code and status instead of asking the
// host; a status of 0 asks the host again.
func (s *Server) Fail(status int, code string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.failStatus, s.failCode = status, code
}

func (s *Server) serve(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cortex-Instance", "fake; role=leader")
	if r.URL.Path != "/v1/fetch" || (r.Method != http.MethodGet && r.Method != http.MethodHead) {
		writeError(w, http.StatusNotFound, "not-found", "the fake serves GET /v1/fetch only")
		return
	}
	q := r.URL.Query()
	f := Fetch{URL: q.Get("url"), Mode: q.Get("mode"), MaxAge: q.Get("max_age"), Stale: q.Get("stale"), Expect: q.Get("expect"),
		At: q.Get("at"), Source: q.Get("source"), Query: q, Header: r.Header.Clone()}
	s.mu.Lock()
	s.fetches = append(s.fetches, f)
	checkedAt, failStatus, failCode := s.checkedAt, s.failStatus, s.failCode
	s.mu.Unlock()

	switch {
	case failStatus != 0:
		writeError(w, failStatus, failCode, "the test makes every fetch fail")
		return
	case f.Mode == "offline" || f.At != "":
		writeError(w, http.StatusGatewayTimeout, "offline-miss", "the fake stores nothing")
		return
	}

	req, err := http.NewRequestWithContext(r.Context(), r.Method, f.URL, nil)
	if err != nil {
		writeError(w, http.StatusBadRequest, "bad-request", err.Error())
		return
	}
	for _, name := range []string{"Accept", "Accept-Language", "User-Agent"} {
		if v := r.Header.Get(name); v != "" {
			req.Header.Set(name, v)
		}
	}
	resp, err := http.DefaultTransport.RoundTrip(req)
	if err != nil {
		writeError(w, http.StatusBadGateway, "upstream-failed", err.Error())
		return
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		writeError(w, http.StatusBadGateway, "upstream-failed", err.Error())
		return
	}

	now := time.Now()
	if checkedAt.IsZero() {
		checkedAt = now
	}
	sum := sha256.Sum256(body)
	h := w.Header()
	if ct := resp.Header.Get("Content-Type"); ct != "" {
		h.Set("Content-Type", ct)
	}
	h.Set("ETag", `"sha256:`+hex.EncodeToString(sum[:])+`"`)
	h.Set("Cache-Status", "Cortex; fwd=miss; fwd-status="+strconv.Itoa(resp.StatusCode)+"; stored")
	h.Set("Cortex-Fetched-At", checkedAt.UTC().Format(TimeFormat))
	h.Set("Cortex-Checked-At", checkedAt.UTC().Format(TimeFormat))
	h.Set("Cortex-Status", strconv.Itoa(resp.StatusCode))
	w.WriteHeader(resp.StatusCode)
	if r.Method != http.MethodHead {
		_, _ = w.Write(body)
	}
}

func writeError(w http.ResponseWriter, status int, code, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Cortex-Error", code)
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"error": code, "message": message})
}
