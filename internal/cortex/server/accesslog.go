package server

import (
	"context"
	"io"
	"log/slog"
	"net/http"
	"strconv"
	"time"

	"github.com/leonieziechmann/betula/internal/oplog"
)

// reqInfo is what the handlers tell the request log about a request.
type reqInfo struct {
	route string // fetch, entries, files, blobs, …, other

	// GET /v1/fetch
	url, source, mode string
	result            string // the result label of cortex_requests_total

	forwardedTo string // the leader the request went to
}

type reqInfoKey struct{}

// info returns the reqInfo of r (a throwaway one outside the request log).
func info(r *http.Request) *reqInfo {
	if ri, ok := r.Context().Value(reqInfoKey{}).(*reqInfo); ok {
		return ri
	}
	return &reqInfo{}
}

// statusWriter remembers the status and counts the bytes of an answer. It passes Flush on
// (the journal's NDJSON, the reverse proxy) and unwraps for http.ResponseController.
//
// It also brings Cortex-Instance up to date when the headers go out: the role can change while
// a request runs (a follower that becomes the leader during WaitLeader and then fetches
// upstream itself), and the header must name the role that answered, not the one the request
// found.
type statusWriter struct {
	http.ResponseWriter
	status      int
	bytes       int64
	wroteHeader bool

	instance func() string // the current value of Cortex-Instance
	early    string        // the value set when the request came in
}

// refreshInstance sets Cortex-Instance to the role of this moment, unless a handler put
// another value there (a forwarded answer names the leader, which answered it).
func (w *statusWriter) refreshInstance() {
	if w.wroteHeader || w.instance == nil {
		return
	}
	h := w.ResponseWriter.Header()
	if v := h.Values("Cortex-Instance"); len(v) == 1 && v[0] == w.early {
		h.Set("Cortex-Instance", w.instance())
	}
}

func (w *statusWriter) WriteHeader(status int) {
	w.refreshInstance()
	if !w.wroteHeader && status >= 200 { // 1xx answers are informational, the final one follows
		w.status, w.wroteHeader = status, true
	}
	w.ResponseWriter.WriteHeader(status)
}

func (w *statusWriter) Write(b []byte) (int, error) {
	w.refreshInstance()
	w.wroteHeader = true
	n, err := w.ResponseWriter.Write(b)
	w.bytes += int64(n)
	return n, err
}

// ReadFrom keeps the connection's own ReadFrom (sendfile for a blob file) reachable.
func (w *statusWriter) ReadFrom(r io.Reader) (int64, error) {
	w.refreshInstance()
	w.wroteHeader = true
	var n int64
	var err error
	if rf, ok := w.ResponseWriter.(io.ReaderFrom); ok {
		n, err = rf.ReadFrom(r)
	} else {
		n, err = io.Copy(writerOnly{w.ResponseWriter}, r)
	}
	w.bytes += n
	return n, err
}

func (w *statusWriter) Flush() {
	w.refreshInstance()
	w.wroteHeader = true
	_ = http.NewResponseController(w.ResponseWriter).Flush()
}

func (w *statusWriter) Unwrap() http.ResponseWriter { return w.ResponseWriter }

// writerOnly hides every method but Write, so that io.Copy does not call ReadFrom again.
type writerOnly struct{ io.Writer }

// quietRoutes are asked every few seconds and say nothing new: DEBUG.
var quietRoutes = map[string]bool{"livez": true, "healthz": true, "metrics": true, "status": true, "journal": true}

// logRequests sets Cortex-Instance on every answer (refreshed when the headers are written), counts the answer in
// cortex_http_requests_total and logs it (event http.request).
//
// Levels: a failure of Cortex itself (500 internal, or a 5xx without a code) is an ERROR; an
// error answer of Cortex is a WARN, except the ones that are answers rather than trouble
// (not-found, offline-miss, precondition-failed: INFO); a stored answer of a host, whatever its
// status (a 404 page is content), is INFO; 304s and the health, status, metrics and journal
// requests that arrive every few seconds are DEBUG.
func (s *Server) logRequests(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		ri := &reqInfo{route: "other"}
		r = r.WithContext(context.WithValue(r.Context(), reqInfoKey{}, ri))
		// Set now as well, for an answer the handler leaves empty: net/http writes its headers
		// after the handler returns, past the statusWriter.
		early := s.instanceHeader()
		sw := &statusWriter{ResponseWriter: w, status: http.StatusOK, instance: s.instanceHeader, early: early}
		sw.Header().Set("Cortex-Instance", early)
		defer func() {
			httpRequestsTotal.Inc(ri.route, strconv.Itoa(sw.status))
			code := sw.Header().Get("Cortex-Error")
			level := slog.LevelInfo
			switch {
			case code == codeInternal || (code == "" && sw.status >= 500):
				level = slog.LevelError
			case code == codeNotFound || code == codeOfflineMiss || code == codePreconditionFailed:
				level = slog.LevelInfo
			case code != "":
				level = slog.LevelWarn
			case sw.status == http.StatusNotModified || quietRoutes[ri.route]:
				level = slog.LevelDebug
			}
			attrs := []any{"event", "http.request", "method", r.Method, "path", r.URL.Path, "route", ri.route,
				"status", sw.status, "bytes", sw.bytes, "duration_ms", time.Since(start).Milliseconds(),
				"remote", r.RemoteAddr, "user_agent", r.UserAgent()}
			if ri.url != "" {
				attrs = append(attrs, "url", ri.url, "source", ri.source, "mode", ri.mode, "result", ri.result)
			}
			if cs := sw.Header().Get("Cache-Status"); cs != "" {
				attrs = append(attrs, "cache", cs)
			}
			if code != "" {
				attrs = append(attrs, "error", code)
			}
			if ri.forwardedTo != "" {
				attrs = append(attrs, "forwarded_to", ri.forwardedTo)
			}
			oplog.For("http").Log(r.Context(), level, "request", attrs...)
		}()
		next.ServeHTTP(sw, r)
	})
}
