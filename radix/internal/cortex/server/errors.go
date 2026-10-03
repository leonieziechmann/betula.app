package server

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"net/http"
	"strconv"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
	"github.com/leonieziechmann/betula/radix/internal/cortex/upstream"
)

// Cortex's error codes: the header Cortex-Error and the "error" of the JSON body.
const (
	codeBadRequest         = "bad-request"         // 400: a parameter is missing or invalid
	codeBadName            = "bad-name"            // 400: not a file name
	codeNotFound           = "not-found"           // 404
	codeMethodNotAllowed   = "method-not-allowed"  // 405
	codePreconditionFailed = "precondition-failed" // 412: If-Match
	codeHashMismatch       = "hash-mismatch"       // 422 (an upload) or 502 (a download)
	codeOfflineMiss        = "offline-miss"        // 504: offline and nothing stored
	codeUpstreamFailed     = "upstream-failed"     // 502
	codeWrongType          = "wrong-type"          // 502: not the host's expect_type
	codeTooLarge           = "too-large"           // 502: more than the host's max_body
	codeHostBusy           = "host-busy"           // 429 with Retry-After
	codeHostPaused         = "host-paused"         // 503 with Retry-After
	codeHostNotAllowed     = "host-not-allowed"    // 403
	codeAddressNotAllowed  = "address-not-allowed" // 403
	codeNotLeader          = "not-leader"          // 409: only the leader does this
	codeCannotStepDown     = "cannot-step-down"    // 409: the leader cannot hand over (a single instance)
	codeDiverged           = "diverged"            // 409: the follower's journal is not the leader's
	codeTrimmed            = "trimmed"             // 410: the journal no longer reaches back that far
	codeNoLeader           = "no-leader"           // 503 with Retry-After: 1
	codeInternal           = "internal"            // 500: Cortex failed (the store)
)

type errorBody struct {
	Error   string `json:"error"`
	Message string `json:"message"`
}

// writeError answers with one of Cortex's errors: the status, Cortex-Error and a JSON body
// {"error": code, "message": message}.
func writeError(w http.ResponseWriter, status int, code, message string) {
	h := w.Header()
	h.Set("Cortex-Error", code)
	h.Set("Content-Type", "application/json")
	h.Set("Cache-Control", "no-store")
	h.Del("Content-Length")
	h.Del("Content-Encoding")
	h.Del("ETag")
	h.Del("Last-Modified")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(errorBody{Error: code, Message: message})
}

// writeErrorRetry is writeError with Retry-After (whole seconds, at least 1).
func writeErrorRetry(w http.ResponseWriter, status int, code string, after time.Duration, message string) {
	w.Header().Set("Retry-After", strconv.FormatInt(retrySeconds(after), 10))
	writeError(w, status, code, message)
}

func retrySeconds(d time.Duration) int64 {
	return max(1, int64(math.Ceil(d.Seconds())))
}

// writeInternal answers 500 internal for a failure of Cortex itself.
func writeInternal(w http.ResponseWriter, what string, err error) {
	writeError(w, http.StatusInternalServerError, codeInternal, fmt.Sprintf("%s: %v", what, err))
}

// writeJSON answers with a JSON document.
func writeJSON(w http.ResponseWriter, status int, v any) {
	h := w.Header()
	h.Set("Content-Type", "application/json")
	h.Set("Cache-Control", "no-store")
	w.WriteHeader(status)
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	_ = enc.Encode(v)
}

// failure is how Cortex answers a fetch that failed upstream.
type failure struct {
	status     int
	code       string
	retryAfter time.Duration // 0: no Retry-After
}

// upstreamFailure maps an error of upstream.Fetch (or of the store as its sink) to Cortex's
// answer.
func upstreamFailure(err error, now time.Time) failure {
	var busy *upstream.BusyError
	var paused *upstream.PausedError
	switch {
	case errors.Is(err, upstream.ErrHostNotAllowed):
		return failure{http.StatusForbidden, codeHostNotAllowed, 0}
	case errors.Is(err, upstream.ErrAddressNotAllowed):
		return failure{http.StatusForbidden, codeAddressNotAllowed, 0}
	case errors.Is(err, upstream.ErrTooLarge), errors.Is(err, store.ErrTooLarge):
		return failure{http.StatusBadGateway, codeTooLarge, 0}
	case errors.Is(err, upstream.ErrWrongType):
		return failure{http.StatusBadGateway, codeWrongType, 0}
	case errors.Is(err, store.ErrHashMismatch):
		return failure{http.StatusBadGateway, codeHashMismatch, 0}
	case errors.As(err, &busy):
		return failure{http.StatusTooManyRequests, codeHostBusy, busy.RetryAfter}
	case errors.As(err, &paused):
		return failure{http.StatusServiceUnavailable, codeHostPaused, paused.Until.Sub(now)}
	}
	return failure{http.StatusBadGateway, codeUpstreamFailed, 0}
}

// refusal says whether a failure is Cortex's own refusal rather than a failure of the host:
// logged at INFO, since the host itself said nothing.
func (f failure) refusal() bool {
	switch f.code {
	case codeHostBusy, codeHostPaused, codeHostNotAllowed:
		return true
	}
	return false
}

func (f failure) write(w http.ResponseWriter, message string) {
	if f.code == codeHostBusy || f.code == codeHostPaused {
		writeErrorRetry(w, f.status, f.code, f.retryAfter, message)
		return
	}
	writeError(w, f.status, f.code, message)
}
