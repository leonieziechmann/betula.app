// Package client talks to Cortex, the cache between Betula and the internet (docs/cortex/cortex.md).
// It fetches through GET /v1/fetch, stores and reads files, and turns an ordinary
// http.Client into one that asks Cortex instead of the host (Transport). It knows every
// instance of an installation and fails over between them: a request goes to the instance
// that answered last, and to the next one when that one cannot be reached or knows no leader.
// A request that changes something (PUT, DELETE, POST) goes to the next one only when the
// first cannot have carried it out; see ErrOutcomeUnknown.
//
// The package imports the standard library only, so that Radix can use it without anything
// of Cortex's server.
package client

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"sync/atomic"
	"syscall"
	"time"
)

// DefaultFailoverWait is how long a request goes round the instances while none of them
// answers: long enough for the follower to take over from a leader that went away.
const DefaultFailoverWait = 5 * time.Second

// failoverPause is the pause between two rounds over the instances.
const failoverPause = 250 * time.Millisecond

// maxErrorBody bounds what is read of an error answer.
const maxErrorBody = 64 << 10

// Cortex's error codes this package acts on (header Cortex-Error, docs/cortex/cortex.md).
const (
	codeNoLeader           = "no-leader"
	codeNotFound           = "not-found"
	codeNotLeader          = "not-leader"
	codePreconditionFailed = "precondition-failed"
)

// The errors an *Error unwraps to, by its code.
var (
	ErrNotFound           = errors.New("cortex: not found")
	ErrNotLeader          = errors.New("cortex: not the leader")
	ErrPreconditionFailed = errors.New("cortex: precondition failed")
)

// ErrOutcomeUnknown is in the error of a request that changes something (PUT, DELETE, POST)
// when it was sent and the connection broke before an answer came: the instance, or the
// leader it handed the request to, may have carried it out or not. Such a request is not
// sent again, neither to another instance nor by Go's transport, because a second answer
// would describe the state the first one left: a 412 for an If-Match that won, a 404 for a
// delete that was done, "not created" for a file that was. GetFile tells what is stored.
// (An error without it and without an answer, such as a refused connection, means that
// nothing was sent; a context that ends while the request is under way leaves the outcome
// open too.)
var ErrOutcomeUnknown = errors.New("cortex: no answer, the request may have been carried out")

// Error is an answer of Cortex that is not a success, for the calls that read it themselves
// (PutFile, GetFile, DeleteFile, Status, StepDown). errors.Is matches ErrNotFound,
// ErrNotLeader and ErrPreconditionFailed by its code.
type Error struct {
	StatusCode int
	Code       string // Cortex-Error, e.g. "not-found"; "" when the answer carried none
	Message    string
}

func (e *Error) Error() string {
	msg := fmt.Sprintf("cortex: %d", e.StatusCode)
	if e.Code != "" {
		msg += " " + e.Code
	}
	if e.Message != "" {
		msg += ": " + e.Message
	}
	return msg
}

// Unwrap returns the sentinel error of the code, or nil.
func (e *Error) Unwrap() error {
	switch e.Code {
	case codeNotFound:
		return ErrNotFound
	case codeNotLeader:
		return ErrNotLeader
	case codePreconditionFailed:
		return ErrPreconditionFailed
	}
	return nil
}

// ResponseError is the error of an answer that Cortex gave itself instead of the host's
// (one with Cortex-Error, such as 502 upstream-failed, or 404 not-found for a path it does
// not serve): an *Error with the status, the code and Cortex's message, after reading the
// answer's body and closing it. It is nil for an answer of the host, whose body is left as
// it is. Through Transport and Fetch such an answer is a response, not an error, so that a
// caller can tell it apart from the host's own status: a 404 with Cortex-Error says nothing
// about the page.
func ResponseError(resp *http.Response) error {
	if ErrorCode(resp) == "" {
		return nil
	}
	return errorFrom(resp)
}

// errorFrom reads an error answer into an *Error and closes its body.
func errorFrom(resp *http.Response) *Error {
	defer resp.Body.Close()
	e := &Error{StatusCode: resp.StatusCode, Code: resp.Header.Get("Cortex-Error")}
	data, _ := io.ReadAll(io.LimitReader(resp.Body, maxErrorBody))
	var body struct {
		Error   string `json:"error"`
		Message string `json:"message"`
	}
	if json.Unmarshal(data, &body) == nil && (body.Error != "" || body.Message != "") {
		if e.Code == "" {
			e.Code = body.Error
		}
		e.Message = body.Message
	} else {
		e.Message = strings.TrimSpace(string(data))
	}
	return e
}

// Options configure a Client.
type Options struct {
	// HTTPClient sends the requests to the instances. Nil: a client of the package's own,
	// which gives up connecting to an instance after 2 s, follows no redirect, and has no
	// overall timeout: the context of a call bounds it.
	HTTPClient *http.Client
	// FailoverWait is how long a request goes round the instances while none answers;
	// 0 means DefaultFailoverWait, a negative value a single round.
	FailoverWait time.Duration
}

// Client sends requests to a Cortex installation. It is safe for concurrent use.
type Client struct {
	endpoints []string
	http      *http.Client
	wait      time.Duration
	current   atomic.Int32 // the index of the instance that answered last
}

// New creates a Client for the instances in urls, a comma-separated list of base URLs
// (http://cortex_a:8100,http://cortex_b:8100). The first one is asked first. A base URL is
// a scheme and a host, with "/" or nothing after it: Cortex serves its API at the root, and
// below a path every request would reach its answer for an unknown path (404 not-found)
// instead, which a careless caller could take for the host's 404.
func New(urls string, opts Options) (*Client, error) {
	var endpoints []string
	for _, raw := range strings.Split(urls, ",") {
		raw = strings.TrimSpace(raw)
		if raw == "" {
			continue
		}
		u, err := url.Parse(raw)
		if err != nil {
			return nil, fmt.Errorf("cortex URL %q: %w", raw, err)
		}
		// "?" and "#" are looked for in raw: an empty query or fragment leaves no trace in u.
		if (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" || u.User != nil || (u.Path != "" && u.Path != "/") ||
			strings.ContainsAny(raw, "?#") {
			return nil, fmt.Errorf("cortex URL %q: want http:// or https:// and a host, with no path, credentials, query or fragment", raw)
		}
		endpoints = append(endpoints, u.Scheme+"://"+u.Host)
	}
	if len(endpoints) == 0 {
		return nil, errors.New("no cortex URL given")
	}
	c := &Client{endpoints: endpoints, http: opts.HTTPClient, wait: opts.FailoverWait}
	if c.http == nil {
		c.http = defaultHTTPClient()
	}
	if c.wait == 0 {
		c.wait = DefaultFailoverWait
	}
	return c, nil
}

// Endpoints returns the base URLs of the instances, in the order of New.
func (c *Client) Endpoints() []string {
	return append([]string(nil), c.endpoints...)
}

func defaultHTTPClient() *http.Client {
	// Cortex is on the same host or the same overlay network: a connection that takes
	// longer than 2 s is not coming, and the next instance is worth asking instead.
	dialer := &net.Dialer{Timeout: 2 * time.Second, KeepAlive: 30 * time.Second}
	return &http.Client{
		Transport: &http.Transport{
			Proxy:                 http.ProxyFromEnvironment,
			DialContext:           dialer.DialContext,
			MaxIdleConns:          64,
			MaxIdleConnsPerHost:   16,
			IdleConnTimeout:       90 * time.Second,
			TLSHandshakeTimeout:   10 * time.Second,
			ExpectContinueTimeout: time.Second,
		},
		// Cortex follows the redirects of a host itself and never answers with one; a
		// redirect is handed to the caller as it is.
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}

// request is what one call sends, to whichever instance takes it.
type request struct {
	method string
	target string      // path and query, /v1/fetch?url=…
	header http.Header // nil: none
	// body returns the body for one attempt and its length (-1: unknown); nil: no body.
	body func() (io.ReadCloser, int64, error)
	// once says that body can be read a single time: the request goes to one instance at most.
	once bool
}

// do sends r to the instance that answered last, and fails over to the next one when an
// instance cannot be connected to, closes or resets the connection before it answers, or
// answers 503 with Cortex-Error no-leader: round after round, with a pause of 250 ms between
// rounds, until one answers or FailoverWait has passed. What goes to the next instance:
//   - a request no instance could be connected to (a dial error): none has seen it, whatever
//     its method, as long as its body can be read again;
//   - after a 503 no-leader: GET, HEAD, PUT and DELETE (with a body that can be read again).
//     The instance did not carry the request out, nor did a leader it handed it to: Cortex
//     answers a write that may have reached the leader with 502 forward-failed instead;
//   - after a connection that broke once the request was sent: GET and HEAD only, which ask
//     and change nothing. A PUT or DELETE may have been carried out, and its answer from
//     another instance would describe the state it left (412 for an If-Match that won, 404
//     for a delete that was done): the error, with ErrOutcomeUnknown, goes to the caller.
//
// When every instance keeps answering no-leader, the last of these answers is returned.
func (c *Client) do(ctx context.Context, r request) (*http.Response, error) {
	asks := r.method == http.MethodGet || r.method == http.MethodHead
	idempotent := asks || r.method == http.MethodPut || r.method == http.MethodDelete
	start := time.Now()
	first := int(c.current.Load())
	var (
		lastResp *http.Response // an answer no-leader, its body read
		lastErr  error
	)
	for {
		for i := range c.endpoints {
			idx := (first + i) % len(c.endpoints)
			resp, sent, err := c.attempt(ctx, idx, r)
			if err == nil && !noLeader(resp) {
				c.current.Store(int32(idx))
				return resp, nil
			}
			if err == nil {
				lastResp = buffered(resp)
				if !idempotent || r.once {
					return lastResp, nil
				}
				continue
			}
			if ctx.Err() != nil {
				return nil, err
			}
			lastErr = err
			switch {
			case unreachable(err) && !r.once:
			case !sent || unreachable(err):
				return nil, err
			case dropped(err) && asks:
			case !asks:
				// Sent, perhaps read and carried out: whatever broke, only the caller can
				// find out what happened.
				return nil, fmt.Errorf("%w: %w", ErrOutcomeUnknown, err)
			default:
				return nil, err
			}
		}
		if c.wait < 0 || time.Since(start)+failoverPause > c.wait {
			break
		}
		if err := pause(ctx, failoverPause); err != nil {
			return nil, err
		}
	}
	if lastResp != nil {
		return lastResp, nil
	}
	return nil, lastErr
}

// attempt sends r to one instance. sent says whether the request went to the transport: an
// error before that (the body or the URL) leaves nothing an instance could have seen.
func (c *Client) attempt(ctx context.Context, idx int, r request) (resp *http.Response, sent bool, err error) {
	base := c.endpoints[idx]
	var body io.ReadCloser
	length := int64(0)
	if r.body != nil {
		if body, length, err = r.body(); err != nil {
			return nil, false, err
		}
	}
	req, err := http.NewRequestWithContext(ctx, r.method, base+r.target, body)
	if err != nil {
		if body != nil {
			_ = body.Close()
		}
		return nil, false, err
	}
	if body != nil {
		if length == 0 {
			_ = body.Close()
			req.Body = http.NoBody
		}
		req.ContentLength = length
		if !r.once {
			// The transport sends a request again by itself when it wrote none of it to a
			// connection the instance had closed while it was idle; for that it needs the
			// body anew.
			req.GetBody = func() (io.ReadCloser, error) {
				b, _, err := r.body()
				return b, err
			}
		}
	}
	for name, values := range r.header {
		req.Header[name] = values
	}
	// A PUT or DELETE carries no Idempotency-Key, not even an empty one (which is not sent):
	// with one, the transport would take it for a request it may send again after the
	// instance read it, on a connection that broke before the answer (see ErrOutcomeUnknown).
	resp, err = c.http.Do(req)
	if err != nil {
		// http.Client names the URL; this names the instance.
		var ue *url.Error
		if errors.As(err, &ue) {
			err = ue.Err
		}
		return nil, true, fmt.Errorf("cortex %s: %w", base, err)
	}
	return resp, true, nil
}

// noLeader says whether resp is the answer of an instance that knows no leader to take
// the request.
func noLeader(resp *http.Response) bool {
	return resp.StatusCode == http.StatusServiceUnavailable && resp.Header.Get("Cortex-Error") == codeNoLeader
}

// buffered reads the body of a small answer into memory and closes the connection's, so
// that the answer can be kept while other instances are asked.
func buffered(resp *http.Response) *http.Response {
	data, _ := io.ReadAll(io.LimitReader(resp.Body, maxErrorBody))
	_ = resp.Body.Close()
	resp.Body = io.NopCloser(bytes.NewReader(data))
	resp.ContentLength = int64(len(data))
	return resp
}

// unreachable says whether err means the instance could not be connected to: nothing was sent.
func unreachable(err error) bool {
	var op *net.OpError
	return errors.As(err, &op) && op.Op == "dial"
}

// dropped says whether the instance closed or reset the connection before its answer began.
func dropped(err error) bool {
	return errors.Is(err, io.EOF) || errors.Is(err, io.ErrUnexpectedEOF) ||
		errors.Is(err, syscall.ECONNRESET) || errors.Is(err, syscall.ECONNABORTED) || errors.Is(err, syscall.EPIPE)
}

func pause(ctx context.Context, d time.Duration) error {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.C:
		return nil
	}
}
