// Package upstream is how Cortex fetches from the internet: a GET for each request, under a host
// policy (which hosts may be fetched, and the floor each one gets) that holds for every client
// of Cortex together.
//
//   - A dialer that refuses loopback, private, link-local and other addresses that are not
//     globally reachable, whatever a name resolves to (unless AllowPrivate, for tests).
//   - Per host: at most `concurrency` requests in flight, a pause (±30 %) after each before its
//     slot is used again, a queue in order of arrival that turns a request away with
//     *BusyError after `queue_wait`.
//   - A breaker per host: `breaker_failures` failures in a row (network error, timeout, 5xx)
//     pause the host for `breaker_pause`; a 429 or 503 with Retry-After pauses it that long
//     (at most an hour). A paused host answers *PausedError at once.
//   - Conditional requests with the stored version's ETag and Last-Modified, to the URL asked
//     only (not to the targets of its redirects); its 304 is Result.NotModified.
//   - The body streams into Request.Sink (the server's store), at most `max_body` bytes, both
//     decoded and as they came over the wire (Cortex decodes gzip itself for that); a 2xx of a
//     host with `expect_type` must be of that media type. Through redirects, the answer at the
//     end must keep the rules of the host asked first as well as its own.
//   - Host names in ASCII only: an internationalised name must be given in punycode, so that
//     no Unicode spelling of a host escapes its entry in the policy.
//
// Coalescing identical requests is the server's business (per cache key); this package only
// guarantees the floor per host.
package upstream

import (
	"compress/gzip"
	"context"
	"errors"
	"fmt"
	"io"
	"mime"
	"net"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
)

// Errors of Fetch. The server maps them to its own: 403 host-not-allowed, 403
// address-not-allowed, 502 too-large, 502 wrong-type.
var (
	ErrHostNotAllowed    = errors.New("host not allowed")
	ErrAddressNotAllowed = errors.New("address not allowed")
	ErrTooLarge          = errors.New("body too large")
	ErrWrongType         = errors.New("wrong content type")
)

// StatusError is an answer upstream gave that Cortex does not store (anything but 200, 203,
// 204, 404, 410 and a 304 to a conditional request); its body was discarded.
type StatusError struct {
	Status int
	URL    string // the URL that answered
}

func (e *StatusError) Error() string {
	return fmt.Sprintf("upstream answered %d for %s", e.Status, e.URL)
}

const (
	maxRedirects     = 10
	dialTimeout      = 10 * time.Second
	tlsTimeout       = 10 * time.Second
	headerTimeout    = 60 * time.Second
	policyCheckEvery = 30 * time.Second
	// discardLimit is how much of an unwanted body is read so that the connection can be
	// used again; a longer one costs the connection instead.
	discardLimit = 64 << 10
)

// keptHeaders are the response headers stored with a version.
var keptHeaders = []string{"Content-Type", "Content-Language", "Content-Disposition", "Last-Modified", "ETag"}

// Options configure New. Policy and PolicyFile exclude each other; without either, the policy
// is DefaultPolicy.
type Options struct {
	Policy       *Policy
	PolicyFile   string // re-read when its modification time changes, checked every 30 s
	AllowPrivate bool   // no address checks, for tests and development
	Now          func() time.Time

	// For tests: the proxy of a request (default http.ProxyFromEnvironment) and how often
	// the policy file is checked.
	proxy      func(*http.Request) (*url.URL, error)
	checkEvery time.Duration
}

// Request is what Fetch fetches.
type Request struct {
	URL            string // absolute, http or https
	Accept         string // sent when not empty
	AcceptLanguage string // sent when not empty
	UserAgent      string // sent when not empty, else the policy's

	// The stored version's upstream ETag and Last-Modified, for a conditional request. They go
	// to URL only, never to the targets of its redirects: a host further down the chain would
	// compare them with a resource of its own, and its 304 would confirm a body it never sent.
	IfNoneMatch     string
	IfModifiedSince string

	// Sink receives the body of a storable answer, reads it to the end and returns its hash
	// and size (the server passes one that calls store.PutBlob). A body that is too large
	// fails its Read with ErrTooLarge before the end, so Sink never sees it complete.
	Sink func(io.Reader) (hash string, size int64, err error)
}

// Result is a storable answer: a body that went to the Sink, or a 304.
type Result struct {
	Status      int         // 200, 203, 204, 404, 410, or 304 with NotModified
	Header      http.Header // the kept headers (Content-Type, Content-Language, Content-Disposition, Last-Modified, ETag)
	Hash        string      // from the Sink; empty with NotModified
	Size        int64       // from the Sink
	NotModified bool        // URL itself answered 304 to the conditional request
	// FinalURL is the URL that answered, after redirects, as it was sent (net/url's spelling,
	// fragment removed); without a redirect, Request.URL in that form. Header's ETag and
	// Last-Modified are that URL's, so the server stores it with the version and passes them
	// as validators again only when it asks that same URL: they go to the first hop alone, and
	// a 304 there confirms the stored body only if that URL is where it came from.
	FinalURL string
}

// Upstream fetches under the host policy. It is safe for concurrent use.
type Upstream struct {
	policy       atomic.Pointer[Policy]
	allowPrivate bool
	now          func() time.Time
	proxyFor     func(*http.Request) (*url.URL, error)

	direct  *http.Client // the dialer checks every address
	proxied *http.Client // through the environment's proxy, the target checked beforehand
	dTrans  *http.Transport
	pTrans  *http.Transport

	mu     sync.Mutex
	states map[string]*hostState // by host name without port

	fileMu      sync.Mutex
	file        string
	fileMod     time.Time
	fileSize    int64
	fileMissing bool

	stop      chan struct{}
	done      chan struct{}
	closeOnce sync.Once
}

// New starts an Upstream; Close stops the goroutine that checks the policy file. A policy file
// that cannot be read or is invalid is an error here; later, a broken file keeps the last good
// policy.
//
// Log events: policy.loaded, policy.invalid (WARN), host.paused (WARN), host.resumed.
func New(opt Options) (*Upstream, error) {
	if opt.Policy != nil && opt.PolicyFile != "" {
		return nil, errors.New("upstream: Policy and PolicyFile exclude each other")
	}
	u := &Upstream{
		allowPrivate: opt.AllowPrivate,
		now:          opt.Now,
		proxyFor:     opt.proxy,
		states:       make(map[string]*hostState),
		file:         opt.PolicyFile,
		stop:         make(chan struct{}),
		done:         make(chan struct{}),
	}
	if u.now == nil {
		u.now = time.Now
	}
	if u.proxyFor == nil {
		u.proxyFor = http.ProxyFromEnvironment
	}

	var p *Policy
	switch {
	case opt.PolicyFile != "":
		fi, err := os.Stat(opt.PolicyFile)
		if err != nil {
			return nil, fmt.Errorf("upstream: policy: %w", err)
		}
		if p, err = LoadPolicy(opt.PolicyFile); err != nil {
			return nil, fmt.Errorf("upstream: %w", err)
		}
		u.fileMod, u.fileSize = fi.ModTime(), fi.Size()
	case opt.Policy != nil:
		var err error
		if p, err = opt.Policy.normalised(); err != nil {
			return nil, fmt.Errorf("upstream: %w", err)
		}
	default:
		p = DefaultPolicy()
	}

	dialer := &net.Dialer{Timeout: dialTimeout, KeepAlive: 30 * time.Second}
	direct := dialer.DialContext
	if !u.allowPrivate {
		direct = guardedDial(&net.Dialer{Timeout: dialTimeout, KeepAlive: 30 * time.Second, Control: refusePrivate})
	}
	u.dTrans = newTransport(direct, nil)
	u.pTrans = newTransport(dialer.DialContext, func(req *http.Request) (*url.URL, error) {
		proxy, err := u.proxyFor(req)
		if err == nil && proxy == nil {
			// Never a direct connection without the dialer's checks.
			err = errors.New("upstream: no proxy for " + req.URL.Host)
		}
		return proxy, err
	})
	u.direct = newClient(u.dTrans)
	u.proxied = newClient(u.pTrans)

	u.setPolicy(p)
	live.Lock()
	live.set[u] = true
	live.Unlock()

	every := opt.checkEvery
	if every <= 0 {
		every = policyCheckEvery
	}
	go u.watch(every)
	return u, nil
}

func newTransport(dial func(ctx context.Context, network, addr string) (net.Conn, error), proxy func(*http.Request) (*url.URL, error)) *http.Transport {
	return &http.Transport{
		Proxy:             proxy,
		DialContext:       dial,
		ForceAttemptHTTP2: true,
		// Cortex asks for gzip and decodes it itself (exchange.answer): Go's transparent gzip
		// would hide the bytes on the wire, which max_body bounds too.
		DisableCompression:     true,
		TLSHandshakeTimeout:    tlsTimeout,
		ResponseHeaderTimeout:  headerTimeout,
		ExpectContinueTimeout:  time.Second,
		IdleConnTimeout:        90 * time.Second,
		MaxIdleConns:           64,
		MaxIdleConnsPerHost:    4,
		MaxResponseHeaderBytes: 1 << 20,
	}
}

// newClient has no cookie jar and follows no redirect: Fetch follows them itself, through the
// allow list and the floor of each host.
func newClient(t *http.Transport) *http.Client {
	return &http.Client{
		Transport:     t,
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}

// Close stops checking the policy file and drops the idle connections. Requests in flight run
// to their end.
func (u *Upstream) Close() {
	u.closeOnce.Do(func() {
		close(u.stop)
		<-u.done
		live.Lock()
		delete(live.set, u)
		live.Unlock()
		u.dTrans.CloseIdleConnections()
		u.pTrans.CloseIdleConnections()
	})
}

// Policy is the policy in force (read-only).
func (u *Upstream) Policy() *Policy { return u.policy.Load() }

func (u *Upstream) setPolicy(p *Policy) {
	u.policy.Store(p)
	seed(p)
	u.applyLimits(p)
	source := u.file
	if source == "" {
		source = "built in"
	}
	oplog.For("upstream").Info("host policy loaded", "event", "policy.loaded", "file", source,
		"hosts", len(p.Hosts), "allow", strings.Join(p.Allow, ","))
}

// watch checks the policy file and sweeps the host states until Close.
func (u *Upstream) watch(every time.Duration) {
	defer close(u.done)
	t := time.NewTicker(every)
	defer t.Stop()
	for {
		select {
		case <-u.stop:
			return
		case <-t.C:
			u.checkPolicy()
			u.sweep()
		}
	}
}

// checkPolicy reloads the policy file when its modification time (or size) changed. A file
// that is gone, unreadable or invalid keeps the policy in force and is logged once.
func (u *Upstream) checkPolicy() {
	if u.file == "" {
		return
	}
	u.fileMu.Lock()
	defer u.fileMu.Unlock()
	fi, err := os.Stat(u.file)
	if err != nil {
		if !u.fileMissing {
			u.fileMissing = true
			oplog.For("upstream").Warn("host policy unreadable, keeping the last good one", "event", "policy.invalid",
				"file", u.file, oplog.Err(err))
		}
		return
	}
	u.fileMissing = false
	if fi.ModTime().Equal(u.fileMod) && fi.Size() == u.fileSize {
		return
	}
	u.fileMod, u.fileSize = fi.ModTime(), fi.Size()
	p, err := LoadPolicy(u.file)
	if err != nil {
		oplog.For("upstream").Warn("host policy invalid, keeping the last good one", "event", "policy.invalid",
			"file", u.file, oplog.Err(err))
		return
	}
	u.setPolicy(p)
}

// Fetch sends a GET for r.URL, following up to 10 redirects (http and https, allowed hosts
// only; each one a request of its own host, under its floor). A storable answer (200, 203,
// 204, 404, 410) streams into r.Sink. The validators go with the first request only, and only
// its 304 is Result.NotModified: a 304 further down a redirect chain is *StatusError, so that
// a stored body is never confirmed by a host that did not send it. The answer at the end of a
// chain must have the expect_type of the host asked first as well as its own, and is held to
// the smaller max_body of the two.
//
// Errors: ErrHostNotAllowed (also for a host that is not ASCII), ErrAddressNotAllowed,
// *BusyError, *PausedError, ErrTooLarge, ErrWrongType, *StatusError, the Sink's error
// (wrapped), ctx's error, or a network error.
func (u *Upstream) Fetch(ctx context.Context, r Request) (Result, error) {
	if r.Sink == nil {
		return Result{}, errors.New("upstream: Request.Sink is nil")
	}
	target, err := url.Parse(r.URL)
	if err != nil {
		return Result{}, fmt.Errorf("upstream: %w", err)
	}
	if err := checkURL(target); err != nil {
		return Result{}, err
	}
	for _, h := range []struct{ name, value string }{
		{"Accept", r.Accept}, {"Accept-Language", r.AcceptLanguage}, {"User-Agent", r.UserAgent},
		{"If-None-Match", r.IfNoneMatch}, {"If-Modified-Since", r.IfModifiedSince},
	} {
		// Refused here, not by the transport: a refusal there would count against the host.
		if strings.ContainsFunc(h.value, isControlButTab) {
			return Result{}, fmt.Errorf("upstream: %s %q: control characters", h.name, h.value)
		}
	}
	target.Fragment, target.RawFragment = "", ""
	c := &chain{r: r}
	for redirects := 0; ; redirects++ {
		res, next, err := u.hop(ctx, target, c)
		if err != nil || next == nil {
			return res, err
		}
		if redirects == maxRedirects {
			return Result{}, fmt.Errorf("upstream: %s: more than %d redirects", r.URL, maxRedirects)
		}
		target = next
	}
}

// chain is what the requests of one Fetch share, from the first to the end of its redirects.
type chain struct {
	r         Request
	first     *HostPolicy // the policy of the host asked first, from its hop on
	firstHost string
}

// checkURL accepts an absolute http or https URL without user info (which would become an
// Authorization header) whose host is ASCII.
func checkURL(target *url.URL) error {
	switch {
	case target.Scheme != "http" && target.Scheme != "https":
		return fmt.Errorf("upstream: %s: only http and https", target.Redacted())
	case target.Opaque != "" || target.Hostname() == "":
		return fmt.Errorf("upstream: %s: not an absolute URL", target.Redacted())
	case target.User != nil:
		return fmt.Errorf("upstream: %s: user info in the URL", target.Redacted())
	case !isASCII(target.Hostname()):
		// Go's transport maps such a name to ASCII (IDNA) before it dials, so fullwidth letters
		// or a soft hyphen would reach a configured host under a name its entry, its floor and
		// its breaker do not know. The policy's names are ASCII; so must the URL's be.
		return fmt.Errorf("%w: host %q is not ASCII: give an internationalised host name in punycode (xn--…)",
			ErrHostNotAllowed, target.Hostname())
	}
	return nil
}

func isASCII(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= 0x80 {
			return false
		}
	}
	return true
}

// hop sends one request: the answer, or the next URL of a redirect.
func (u *Upstream) hop(ctx context.Context, target *url.URL, c *chain) (Result, *url.URL, error) {
	if err := ctx.Err(); err != nil {
		return Result{}, nil, err
	}
	host := normalHost(target.Hostname())
	p := u.Policy()
	if !p.Allows(host) {
		return Result{}, nil, fmt.Errorf("%w: %s", ErrHostNotAllowed, host)
	}
	// An address that can never be fetched is refused before it takes a slot: it would leave a
	// host state behind for every address a client makes up.
	if err := u.refuseLiteral(host); err != nil {
		return Result{}, nil, fmt.Errorf("upstream: %s: %w", target.Redacted(), err)
	}
	hp := p.For(host)
	x := exchange{u: u, ctx: ctx, target: target, r: c.r, host: host, hp: hp, userAgent: p.UserAgent}
	if c.first == nil {
		first := hp
		c.first, c.firstHost = &first, host
		x.conditional = c.r.IfNoneMatch != "" || c.r.IfModifiedSince != ""
	}
	x.first, x.firstHost = *c.first, c.firstHost
	if err := u.acquire(ctx, host, hp); err != nil {
		return Result{}, nil, err
	}

	start := time.Now()
	res, next, err := x.run()
	took := time.Since(start)
	if x.sent {
		countRequest(hp.label(), x.status, took, x.bytes)
	}
	u.release(host, hp, x.sent, x.outcome, x.retryAfter, took)
	return res, next, err
}

// exchange is one request and its answer, inside a slot of the host; besides the result, it
// tells the limiter and the metrics what happened.
type exchange struct {
	u         *Upstream
	ctx       context.Context
	target    *url.URL
	r         Request
	host      string
	hp        HostPolicy // of host
	userAgent string     // the policy's

	// The policy of the host the chain began with (hp itself on the first hop): its
	// expect_type and max_body hold for the answer at the end too, so that a redirect to
	// another host (a login or a challenge page) is not judged by that host's rules alone.
	first     HostPolicy
	firstHost string
	// conditional says that this request carries the validators: only the first one does.
	conditional bool

	sent       bool          // a request went out (or tried to): it counts and the slot pauses
	status     int           // 0 without an answer
	bytes      int64         // of the body
	outcome    outcome       // for the breaker
	retryAfter time.Duration // a 429 or 503 asked for this pause
}

func (x *exchange) run() (Result, *url.URL, error) {
	ctx, cancel := context.WithTimeout(x.ctx, x.hp.Timeout)
	defer cancel()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, x.target.String(), nil)
	if err != nil {
		return Result{}, nil, fmt.Errorf("upstream: %w", err)
	}
	ua := x.r.UserAgent
	if ua == "" {
		ua = x.userAgent
	}
	req.Header.Set("User-Agent", ua)
	// Asked for here and decoded in answer, not by Go's transport, which would hide the bytes
	// on the wire from max_body.
	req.Header.Set("Accept-Encoding", "gzip")
	headers := []struct{ name, value string }{
		{"Accept", x.r.Accept},
		{"Accept-Language", x.r.AcceptLanguage},
	}
	if x.conditional {
		headers = append(headers, []struct{ name, value string }{
			{"If-None-Match", x.r.IfNoneMatch},
			{"If-Modified-Since", x.r.IfModifiedSince},
		}...)
	}
	for _, h := range headers {
		if h.value != "" {
			req.Header.Set(h.name, h.value)
		}
	}

	client := x.u.direct
	proxy, err := x.u.proxyFor(req)
	if err != nil {
		return Result{}, nil, fmt.Errorf("upstream: proxy: %w", err)
	}
	if proxy != nil {
		if !x.u.allowPrivate {
			if err := checkTarget(ctx, normalHost(x.target.Hostname())); err != nil {
				if !errors.Is(err, ErrAddressNotAllowed) {
					x.sent, x.outcome = true, x.failed(err)
				}
				return Result{}, nil, err
			}
		}
		client = x.u.proxied
	}

	resp, err := client.Do(req)
	if err != nil {
		if errors.Is(err, ErrAddressNotAllowed) {
			return Result{}, nil, err
		}
		x.sent, x.outcome = true, x.failed(err)
		return Result{}, nil, err
	}
	defer resp.Body.Close()
	x.sent, x.status = true, resp.StatusCode
	return x.answer(resp)
}

// answer handles the response of the request.
func (x *exchange) answer(resp *http.Response) (Result, *url.URL, error) {
	status := resp.StatusCode
	from := x.target.String()
	x.outcome = success
	if status >= 500 {
		x.outcome = failure
	}

	switch {
	case isRedirect(status) && resp.Header.Get("Location") != "":
		x.discard(resp.Body)
		next, err := redirectTarget(x.target, resp.Header.Get("Location"))
		return Result{}, next, err

	case status == http.StatusNotModified && x.conditional:
		x.discard(resp.Body)
		return Result{Status: status, Header: kept(resp.Header), NotModified: true, FinalURL: from}, nil, nil

	case !Storable(status):
		// A 304 lands here unless this request carried the validators: further down a redirect
		// chain it would say only that a resource of another URL did not change.
		if status == http.StatusTooManyRequests || status == http.StatusServiceUnavailable {
			x.retryAfter = parseRetryAfter(resp.Header.Get("Retry-After"), x.u.now())
		}
		x.discard(resp.Body)
		return Result{}, nil, &StatusError{Status: status, URL: from}
	}

	var gzipped bool
	switch ce := strings.TrimSpace(strings.Join(resp.Header.Values("Content-Encoding"), ", ")); {
	case ce == "" || strings.EqualFold(ce, "identity"):
	case strings.EqualFold(ce, "gzip") || strings.EqualFold(ce, "x-gzip"):
		gzipped = true
	default:
		// Cortex asks for gzip only; any other coding would be stored as if it were the content.
		return Result{}, nil, fmt.Errorf("upstream: %s answered with Content-Encoding %q", from, ce)
	}
	if status/100 == 2 && status != http.StatusNoContent {
		if err := x.checkType(resp.Header.Get("Content-Type"), from); err != nil {
			return Result{}, nil, err
		}
	}
	maxBody := min(x.hp.MaxBody, x.first.MaxBody)
	if resp.ContentLength > maxBody {
		return Result{}, nil, fmt.Errorf("%w: %s announces %d bytes, more than %d", ErrTooLarge, from, resp.ContentLength, maxBody)
	}

	b := &body{r: resp.Body, max: maxBody}
	if gzipped {
		// max_body bounds the bytes on the wire too: gzip can make any number of them decode
		// to nothing (empty members), which the decoded count alone would never stop.
		b.wire = &wire{r: resp.Body, max: maxBody}
		b.r = &gunzip{r: b.wire}
	}
	hash, size, err := x.r.Sink(b)
	if err == nil && !b.eof && b.err == nil && !b.tooLarge {
		// The Sink stopped at the last byte without reading the end: make sure it is the end.
		var one [1]byte
		if n, rerr := b.Read(one[:]); n > 0 || rerr != io.EOF {
			err = errors.New("the sink stopped before the end of the body")
		}
	}
	x.bytes = b.n
	switch {
	case b.wire != nil && b.wire.tooLarge:
		return Result{}, nil, fmt.Errorf("%w: %s sent more than %d bytes of gzip", ErrTooLarge, from, maxBody)
	case b.tooLarge:
		return Result{}, nil, fmt.Errorf("%w: %s sent more than %d bytes", ErrTooLarge, from, maxBody)
	case b.err != nil:
		x.outcome = x.failed(b.err)
		return Result{}, nil, fmt.Errorf("upstream: reading the body of %s: %w", from, b.err)
	case err != nil:
		return Result{}, nil, fmt.Errorf("upstream: storing the body of %s: %w", from, err)
	case size != b.n:
		return Result{}, nil, fmt.Errorf("upstream: the sink stored %d bytes of the %d of %s", size, b.n, from)
	}
	return Result{Status: status, Header: kept(resp.Header), Hash: hash, Size: size, FinalURL: from}, nil, nil
}

// checkType holds an answer to the expect_type of its host and to that of the host the chain
// began with.
func (x *exchange) checkType(contentType, from string) error {
	got := mediaType(contentType)
	for _, rule := range []struct{ want, host string }{{x.hp.ExpectType, x.host}, {x.first.ExpectType, x.firstHost}} {
		if rule.want == "" || got == rule.want {
			continue
		}
		answered := got
		if answered == "" {
			answered = "no Content-Type"
		}
		if rule.host != x.host {
			return fmt.Errorf("%w: %s answered %s, not %s, which %s (where the redirects began) requires",
				ErrWrongType, from, answered, rule.want, rule.host)
		}
		return fmt.Errorf("%w: %s answered %s, not %s", ErrWrongType, from, answered, rule.want)
	}
	return nil
}

// failed judges an error for the breaker: the caller giving up says nothing about the host,
// a network error or a timeout does.
func (x *exchange) failed(err error) outcome {
	if errors.Is(x.ctx.Err(), context.Canceled) || errors.Is(err, ErrAddressNotAllowed) {
		return neutral
	}
	return failure
}

// discard reads a little of an unwanted body, so that the connection can be used again.
func (x *exchange) discard(r io.Reader) {
	n, _ := io.CopyN(io.Discard, r, discardLimit)
	x.bytes += n
}

// body hands the response body to the Sink, max bytes at most: past them it fails with
// ErrTooLarge, so the Sink never sees the end of a body that is too large. A gzip body is
// decoded on the way, and wire holds its bytes as they came to max as well.
type body struct {
	r        io.Reader // the content: the response body, or gunzip over wire
	wire     *wire     // a gzip body as it came; nil for one without a coding
	n, max   int64
	tooLarge bool
	eof      bool
	err      error // reading the response failed (network, timeout, broken gzip)
}

func (b *body) Read(p []byte) (int, error) {
	switch {
	case b.tooLarge:
		return 0, ErrTooLarge
	case b.err != nil:
		return 0, b.err
	case b.eof:
		return 0, io.EOF
	}
	n, err := b.r.Read(clip(p, b.max-b.n))
	if int64(n) > b.max-b.n {
		n = int(b.max - b.n)
		b.n = b.max
		b.tooLarge = true
		return n, ErrTooLarge
	}
	b.n += int64(n)
	switch {
	case b.wire != nil && b.wire.tooLarge:
		b.tooLarge = true
		return n, ErrTooLarge
	case err == io.EOF:
		b.eof = true
	case err != nil:
		b.err = err
	}
	return n, err
}

// wire counts the bytes of a gzip body as they come over the network, max at most: past them
// it fails with ErrTooLarge, whatever they decode to.
type wire struct {
	r        io.Reader
	n, max   int64
	tooLarge bool
}

func (w *wire) Read(p []byte) (int, error) {
	if w.tooLarge {
		return 0, ErrTooLarge
	}
	n, err := w.r.Read(clip(p, w.max-w.n))
	if int64(n) > w.max-w.n {
		n = int(w.max - w.n)
		w.n = w.max
		w.tooLarge = true
		return n, ErrTooLarge
	}
	w.n += int64(n)
	return n, err
}

// clip shortens p to room bytes and one more, the one that shows a body goes past its limit.
// It compares before it adds, so that a limit near the largest int64 cannot overflow (the
// policy keeps max_body far below it as well).
func clip(p []byte, room int64) []byte {
	if int64(len(p)) > room {
		return p[:room+1]
	}
	return p
}

// gunzip decodes a gzip body from its first Read on, as Go's transport does, so that nothing is
// read before the Sink asks; an empty body is an empty answer, as there too.
type gunzip struct {
	r   io.Reader
	zr  *gzip.Reader
	err error
}

func (g *gunzip) Read(p []byte) (int, error) {
	if g.err != nil {
		return 0, g.err
	}
	if g.zr == nil {
		zr, err := gzip.NewReader(g.r)
		if err != nil {
			g.err = err // io.EOF for an empty body
			return 0, err
		}
		g.zr = zr
	}
	return g.zr.Read(p)
}

func isControlButTab(r rune) bool { return r != '\t' && isControl(r) }

func isRedirect(status int) bool {
	switch status {
	case http.StatusMovedPermanently, http.StatusFound, http.StatusSeeOther, http.StatusTemporaryRedirect, http.StatusPermanentRedirect:
		return true
	}
	return false
}

// Storable are the answers Cortex keeps: content, and the statement that there is none.
func Storable(status int) bool {
	switch status {
	case http.StatusOK, http.StatusNonAuthoritativeInfo, http.StatusNoContent, http.StatusNotFound, http.StatusGone:
		return true
	}
	return false
}

// redirectTarget resolves a Location against the URL that sent it.
func redirectTarget(from *url.URL, location string) (*url.URL, error) {
	next, err := from.Parse(location)
	if err != nil {
		return nil, fmt.Errorf("upstream: %s redirects to %q: %w", from, location, err)
	}
	if err := checkURL(next); err != nil {
		return nil, fmt.Errorf("upstream: %s redirects: %w", from, err)
	}
	next.Fragment, next.RawFragment = "", ""
	return next, nil
}

func kept(h http.Header) http.Header {
	out := make(http.Header)
	for _, name := range keptHeaders {
		if v := h.Get(name); v != "" {
			out.Set(name, v)
		}
	}
	return out
}

// mediaType is the media type of a Content-Type, in lower case and without parameters.
func mediaType(contentType string) string {
	mt, _, err := mime.ParseMediaType(contentType)
	if err != nil && mt == "" {
		mt, _, _ = strings.Cut(contentType, ";")
		mt = strings.ToLower(strings.TrimSpace(mt))
	}
	return mt
}

// parseRetryAfter reads Retry-After as seconds or as an HTTP date; 0 when absent, invalid or
// past.
func parseRetryAfter(v string, now time.Time) time.Duration {
	v = strings.TrimSpace(v)
	if v == "" {
		return 0
	}
	if secs, err := strconv.ParseInt(v, 10, 64); err == nil {
		if secs <= 0 {
			return 0
		}
		return time.Duration(min(secs, int64(maxRetryAfter/time.Second))) * time.Second
	}
	if t, err := http.ParseTime(v); err == nil && t.After(now) {
		return t.Sub(now)
	}
	return 0
}
