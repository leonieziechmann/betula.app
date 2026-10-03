package server

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
	"github.com/leonieziechmann/betula/radix/internal/cortex/upstream"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// The parameters of GET /v1/fetch.
const (
	modeOffline   = "offline"
	modeCache     = "cache"
	modeRefresh   = "refresh"
	staleIfError  = "if-error"
	staleNever    = "never"
	defaultSource = "unknown"
)

var sourcePattern = regexp.MustCompile(`^[a-z0-9_.-]{1,64}$`)

// fetchParams is a request to GET /v1/fetch, checked.
type fetchParams struct {
	key, url, host         string // from store.Canonical
	accept, acceptLanguage string // part of the key
	userAgent              string // passed upstream, not part of the key
	mode, stale, source    string
	maxAge                 time.Duration
	maxAgeSet              bool
	expect                 string    // sha256 hex, or ""
	at                     time.Time // zero: the current version
}

// parseFetch reads the parameters of r. The source and the mode are filled in as far as they
// are valid even when an error is returned, for the metrics.
func parseFetch(r *http.Request) (fetchParams, error) {
	q := r.URL.Query()
	p := fetchParams{mode: modeCache, stale: staleIfError, source: defaultSource}
	if v := q.Get("source"); v != "" {
		if !sourcePattern.MatchString(v) {
			return p, fmt.Errorf("source %q: 1 to 64 of a-z, 0-9, '_', '.', '-'", v)
		}
		p.source = v
	}
	switch v := q.Get("mode"); v {
	case "":
	case modeOffline, modeCache, modeRefresh:
		p.mode = v
	default:
		return p, fmt.Errorf("mode %q: offline, cache or refresh", v)
	}
	if v := q.Get("at"); v != "" {
		t, err := time.Parse(time.RFC3339, v)
		if err != nil {
			return p, fmt.Errorf("at %q: not an RFC 3339 time", v)
		}
		p.at, p.mode = t, modeOffline // a version of the past is never fetched
	}

	rawURL := q.Get("url")
	if rawURL == "" {
		return p, errors.New("url is required")
	}
	p.accept = strings.Join(r.Header.Values("Accept"), ", ")
	p.acceptLanguage = strings.Join(r.Header.Values("Accept-Language"), ", ")
	p.userAgent = r.Header.Get("User-Agent")
	var err error
	if p.key, p.url, p.host, err = store.Canonical(rawURL, p.accept, p.acceptLanguage); err != nil {
		return p, err
	}
	if v := q.Get("max_age"); v != "" {
		d, err := time.ParseDuration(v)
		if err != nil || d < 0 {
			return p, fmt.Errorf("max_age %q: a Go duration of 0 or more", v)
		}
		p.maxAge, p.maxAgeSet = d, true
	}
	switch v := q.Get("stale"); v {
	case "":
	case staleIfError, staleNever:
		p.stale = v
	default:
		return p, fmt.Errorf("stale %q: if-error or never", v)
	}
	if v := q.Get("expect"); v != "" {
		hex, ok := strings.CutPrefix(v, "sha256:")
		if !ok || !store.ValidHash(hex) {
			return p, fmt.Errorf("expect %q: sha256:<64 lower-case hex digits>", v)
		}
		p.expect = hex
	}
	return p, nil
}

// handleFetch is GET and HEAD /v1/fetch: an answer of a host, from the store or fetched
// (spec §4).
//
//	mode     stored, age ≤ max_age   stored, older   not stored
//	offline  serve                   serve           504 offline-miss
//	cache    serve                   fetch           fetch
//	refresh  fetch                   fetch           fetch
//
// The age is the time since the current version was last checked upstream. A version with the
// expected hash (expect) is fresh at any age; with expect, a version of another hash counts as
// not stored. at serves the version current at that time (offline). A follower serves what its
// copy has and forwards the rest to the leader, an offline miss included (the leader may have
// it).
func (s *Server) handleFetch(w http.ResponseWriter, r *http.Request) {
	ri := info(r)
	ri.result, ri.mode, ri.source, ri.url = "error", modeCache, defaultSource, r.URL.Query().Get("url")
	defer func() { requestsTotal.Inc(ri.source, ri.mode, ri.result) }()

	p, err := parseFetch(r)
	ri.mode, ri.source = p.mode, p.source
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	ri.url = p.url
	if !p.maxAgeSet {
		p.maxAge = s.up.Policy().For(p.host).MaxAge
	}

	if !p.at.IsZero() {
		_, v, err := s.st.LookupAt(p.key, p.at)
		switch {
		case err == nil && s.usable(v, p.expect):
			s.serve(w, r, v, "Cortex; hit", nil, "hit")
		case err == nil || errors.Is(err, store.ErrNotFound):
			s.offlineMiss(w, r, err == nil && s.lacksBlob(v, p.expect), fmt.Sprintf("no version of %s was current at %s", p.url, p.at.UTC().Format(time.RFC3339)))
		default:
			writeInternal(w, "lookup", err)
		}
		return
	}

	_, v, err := s.st.Lookup(p.key)
	if err != nil && !errors.Is(err, store.ErrNotFound) {
		writeInternal(w, "lookup", err)
		return
	}
	usable := err == nil && s.usable(v, p.expect)
	fresh := usable && (p.expect != "" || s.age(v) <= p.maxAge)

	switch {
	case p.mode == modeOffline && usable, p.mode == modeCache && fresh:
		s.serve(w, r, v, "Cortex; hit", nil, "hit")
		return
	case p.mode == modeOffline:
		s.offlineMiss(w, r, err == nil && s.lacksBlob(v, p.expect), "nothing stored for "+p.url)
		return
	}

	// Fetch: only the leader does.
	if !s.leading() && s.forward(w, r) {
		return
	}
	fwd, result := "miss", "miss"
	switch {
	case p.mode == modeRefresh:
		fwd, result = "request", "refresh"
	case usable:
		fwd, result = "stale", "stale"
	}

	res, ok := s.fetchCoalesced(r.Context(), p)
	switch {
	case !ok:
		return // the client went away; the fetch goes on for the others
	case res.notLeader:
		if s.forward(w, r) {
			return
		}
		// This instance leads again: the client asks again (no-leader is a fail-over).
		ri.result = "error"
		writeErrorRetry(w, http.StatusServiceUnavailable, codeNoLeader, time.Second, "the role of this instance changed during the fetch")
		return
	case res.err != nil:
		f := upstreamFailure(res.err, time.Now()) // upstream's pauses are on the wall clock
		if res.internal {
			f = failure{http.StatusInternalServerError, codeInternal, 0}
		}
		if p.stale == staleIfError && usable {
			s.serve(w, r, v, "Cortex; hit; detail=stale-if-error", http.Header{"Cortex-Upstream-Error": {f.code}}, "stale_if_error")
			return
		}
		f.write(w, res.err.Error())
		return
	}
	cacheStatus := fmt.Sprintf("Cortex; fwd=%s; fwd-status=%d; stored", fwd, res.status)
	s.serve(w, r, res.version, cacheStatus, nil, result)
}

// usable says whether a version can be served here: its blob is stored, and it has the
// expected hash when one is given.
func (s *Server) usable(v store.Version, expect string) bool {
	return (expect == "" || v.Hash == expect) && s.st.HasBlob(v.Hash)
}

// lacksBlob says whether a version would be usable but for its blob, which this instance does
// not have (yet). A leader fetches such a version upstream again, as one not stored, unless it
// is offline; then it reads it from the other instance.
func (s *Server) lacksBlob(v store.Version, expect string) bool {
	return (expect == "" || v.Hash == expect) && !s.st.HasBlob(v.Hash)
}

// age is how long ago a version was last checked upstream.
func (s *Server) age(v store.Version) time.Duration {
	return max(0, s.now().Sub(v.CheckedAt))
}

// offlineMiss answers a request nothing stored can answer: a follower asks the leader, which
// may have fetched it a moment ago; the leader answers 504 offline-miss. lacksBlob says that a
// version is stored but its blob is not: a leader promoted before its back-fill ended asks the
// other instance first (E2E-4).
func (s *Server) offlineMiss(w http.ResponseWriter, r *http.Request, lacksBlob bool, message string) {
	if !s.leading() && s.forward(w, r) {
		return
	}
	if lacksBlob && s.leading() && s.fromPeer(w, r) {
		return
	}
	info(r).result = "offline_miss"
	writeError(w, http.StatusGatewayTimeout, codeOfflineMiss, message)
}

// serve answers with a stored version and counts the request as result (not_modified when
// If-None-Match matches). A version whose blob went missing in between is forwarded by a
// follower, and read from the other instance by a leader (blobMissing).
func (s *Server) serve(w http.ResponseWriter, r *http.Request, v store.Version, cacheStatus string, extra http.Header, result string) {
	ri := info(r)
	body, err := s.openBlob(v.Hash, v.Size)
	if err != nil {
		if errors.Is(err, store.ErrNotFound) { // went missing in between, or not back-filled yet (E2E-4)
			s.blobMissing(w, r, v.Hash)
			return
		}
		ri.result = "error"
		writeInternal(w, "blob "+v.Hash, err)
		return
	}
	defer body.Close()

	h := w.Header()
	for _, name := range []string{"Content-Type", "Content-Language", "Content-Disposition"} {
		if value := v.Header.Get(name); value != "" {
			h.Set(name, value)
		}
	}
	etag := `"sha256:` + v.Hash + `"`
	h.Set("ETag", etag)
	lastModified := v.Header.Get("Last-Modified")
	if lastModified == "" {
		lastModified = v.FetchedAt.UTC().Format(http.TimeFormat)
	}
	h.Set("Last-Modified", lastModified)
	h.Set("Age", strconv.FormatInt(int64(s.age(v)/time.Second), 10))
	h.Set("Cache-Status", cacheStatus)
	h.Set("Cortex-Fetched-At", store.FormatTime(v.FetchedAt))
	h.Set("Cortex-Checked-At", store.FormatTime(v.CheckedAt))
	h.Set("Cortex-Version", strconv.FormatInt(v.ID, 10))
	h.Set("Cortex-Status", strconv.Itoa(v.Status))
	if tag := v.Header.Get("ETag"); tag != "" {
		h.Set("Cortex-Upstream-ETag", tag)
	}
	for name, values := range extra {
		h[name] = values
	}

	if v.Status >= 200 && v.Status < 300 && etagMatches(r.Header.Get("If-None-Match"), etag) {
		ri.result = "not_modified"
		h.Del("Content-Type")
		w.WriteHeader(http.StatusNotModified)
		return
	}
	ri.result = result
	if v.Status == http.StatusOK {
		// Range, If-Range, If-Modified-Since and HEAD.
		modified, _ := http.ParseTime(lastModified)
		if _, ok := h["Content-Type"]; !ok {
			h["Content-Type"] = nil // no sniffing: upstream named no type
		}
		http.ServeContent(w, r, "", modified, body)
		return
	}
	if v.Status != http.StatusNoContent {
		h.Set("Content-Length", strconv.FormatInt(v.Size, 10))
	}
	w.WriteHeader(v.Status)
	if r.Method != http.MethodHead && v.Status != http.StatusNoContent {
		_, _ = io.Copy(w, body)
	}
}

// etagMatches is the weak comparison of If-None-Match against etag.
func etagMatches(header, etag string) bool {
	if header == "" {
		return false
	}
	for _, part := range strings.Split(header, ",") {
		part = strings.TrimSpace(part)
		if part == "*" || strings.TrimPrefix(part, "W/") == etag {
			return true
		}
	}
	return false
}

// flight is one upstream request that every request for the same key and expected hash waits
// for.
type flight struct {
	done chan struct{}
	res  fetchResult
}

// fetchResult is how a flight ended.
type fetchResult struct {
	version   store.Version // the version recorded (new, or the current one checked)
	status    int           // upstream's answer: its status, or 304
	err       error         // upstream failed (or the store, with internal)
	internal  bool          // err is Cortex's own failure
	notLeader bool          // the instance stopped leading: the request goes to the leader
}

// fetchCoalesced fetches p upstream, or joins the fetch of the same key and expected hash that
// is running. The fetch runs detached from ctx: a client that gives up (ok false) does not end
// a download that others wait for.
func (s *Server) fetchCoalesced(ctx context.Context, p fetchParams) (fetchResult, bool) {
	k := p.key + "\x00" + p.expect
	s.flightMu.Lock()
	if s.closed {
		s.flightMu.Unlock()
		return fetchResult{notLeader: true}, true
	}
	f, joined := s.flights[k]
	if !joined {
		f = &flight{done: make(chan struct{})}
		s.flights[k] = f
		s.wg.Add(1)
		go func() {
			defer s.wg.Done()
			f.res = s.fetchUpstream(p)
			s.flightMu.Lock()
			delete(s.flights, k)
			s.flightMu.Unlock()
			close(f.done)
		}()
	}
	s.flightMu.Unlock()
	if joined {
		coalescedTotal.Inc()
	}
	select {
	case <-f.done:
		return f.res, true
	case <-ctx.Done():
		return fetchResult{}, false
	}
}

// fetchUpstream sends one request upstream, conditional when the current version has upstream
// validators, streams a storable answer into the store and records it (in BeginWrite).
//
// Log events: upstream.fetched, upstream.failed (WARN; INFO for Cortex's own refusals: the
// host is busy, paused or not allowed).
func (s *Server) fetchUpstream(p fetchParams) fetchResult {
	if !s.leading() {
		return fetchResult{notLeader: true}
	}
	hp := s.up.Policy().For(p.host)
	// Detached from every client, with room for the host's queue and its own timeout: a deadline
	// of Cortex's own would count against the host's breaker.
	ctx, cancel := context.WithTimeout(s.ctx, hp.QueueWait+2*hp.Timeout)
	defer cancel()

	var cur *store.Version
	if _, v, err := s.st.Lookup(p.key); err == nil && s.usable(v, p.expect) {
		cur = &v
	}
	// sinkErr is the store's own failure to keep the body (a full disk, an I/O error): Cortex's
	// fault, not the host's, so it is answered 500 internal and logged at ERROR (review 2:
	// sink-store-failure-mapped-upstream). A content check (hash, size) stays upstream's.
	var sinkErr error
	req := upstream.Request{URL: p.url, Accept: p.accept, AcceptLanguage: p.acceptLanguage, UserAgent: p.userAgent,
		Sink: func(r io.Reader) (string, int64, error) {
			b, err := s.st.PutBlob(r, p.expect, 0) // upstream enforces max_body
			if err != nil && !errors.Is(err, store.ErrHashMismatch) && !errors.Is(err, store.ErrTooLarge) {
				sinkErr = err
			}
			return b.Hash, b.Size, err
		}}
	// The validators are those of the URL that answered the stored version. upstream sends them
	// with the first request only, to p.url, so they go only when that is the URL they came
	// from: another URL's Last-Modified could get a 304 that confirms a body p.url never sent
	// (review 2: validators-final-hop-sent-to-first-hop).
	if cur != nil && sameURL(cur.FinalURL, p.url) {
		req.IfNoneMatch, req.IfModifiedSince = cur.Header.Get("ETag"), cur.Header.Get("Last-Modified")
	}

	start := time.Now()
	res, err := s.up.Fetch(ctx, req)
	took := time.Since(start)
	log := oplog.For("upstream")
	if err != nil && sinkErr != nil && errors.Is(err, sinkErr) {
		log.Error("cannot store a download", "event", "upstream.failed", "url", p.url, "host", p.host, "source", p.source,
			"code", codeInternal, "duration_ms", took.Milliseconds(), oplog.Err(err))
		return fetchResult{err: fmt.Errorf("storing the body of %s: %w", p.url, sinkErr), internal: true}
	}
	if err != nil {
		f := upstreamFailure(err, time.Now())
		level := log.Warn
		if f.refusal() || errors.Is(err, context.Canceled) {
			level = log.Info
		}
		level("fetch failed", "event", "upstream.failed", "url", p.url, "host", p.host, "source", p.source,
			"code", f.code, "duration_ms", took.Milliseconds(), oplog.Err(err))
		return fetchResult{err: err}
	}

	fetched := store.Fetched{Key: p.key, URL: p.url, Host: p.host, Source: p.source, Accept: p.accept,
		AcceptLanguage: p.acceptLanguage, Status: res.Status, Hash: res.Hash, Size: res.Size, Header: res.Header,
		FinalURL: res.FinalURL, At: s.now()}
	status := res.Status
	if res.NotModified {
		if cur == nil { // upstream answers NotModified only to a conditional request
			return fetchResult{err: errors.New("upstream answered 304 to an unconditional request"), internal: true}
		}
		fetched.Status, fetched.Hash, fetched.Size, fetched.Header = cur.Status, cur.Hash, cur.Size, cur.Header
		fetched.FinalURL = cur.FinalURL
		status = http.StatusNotModified
	}
	done, ok := s.node.BeginWrite()
	if !ok {
		return fetchResult{notLeader: true}
	}
	_, v, changed, _, err := s.st.RecordFetch(fetched)
	done()
	if err != nil {
		log.Error("cannot record a fetch", "event", "upstream.failed", "url", p.url, "host", p.host, "source", p.source,
			"code", codeInternal, oplog.Err(err))
		return fetchResult{err: fmt.Errorf("recording the fetch of %s: %w", p.url, err), internal: true}
	}
	log.Info("fetched", "event", "upstream.fetched", "url", p.url, "host", p.host, "source", p.source,
		"status", status, "bytes", res.Size, "changed", changed, "version", v.ID, "duration_ms", took.Milliseconds())
	return fetchResult{version: v, status: status}
}

// sameURL says whether a and b are the same URL (as url.URL.String writes them): the final
// URL upstream reports is the parsed form of the URL it asked.
func sameURL(a, b string) bool {
	if a == "" || b == "" {
		return false
	}
	if a == b {
		return true
	}
	ua, err := url.Parse(a)
	if err != nil {
		return false
	}
	ub, err := url.Parse(b)
	return err == nil && ua.String() == ub.String()
}
