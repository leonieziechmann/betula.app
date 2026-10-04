package client

import (
	"context"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// The modes of a fetch (docs/cortex/cortex.md): offline serves only what Cortex has, cache serves
// what is younger than max_age and fetches the rest, refresh always fetches.
const (
	ModeOffline = "offline"
	ModeCache   = "cache"
	ModeRefresh = "refresh"
)

// What Cortex does when the host fails and it has a version stored: serve that one, marked
// stale (StaleIfError), or answer with the failure (StaleNever).
const (
	StaleIfError = "if-error"
	StaleNever   = "never"
)

// DefaultSource is the source of a request that names none.
const DefaultSource = "unknown"

// CodeOfflineMiss is Cortex's error code (ErrorCode) when a fetch in mode offline, or of the
// version current at a time, finds nothing stored: 504 offline-miss.
const CodeOfflineMiss = "offline-miss"

// FetchOptions are the parameters of GET /v1/fetch. An empty field is left out, so that
// Cortex's default applies, except MaxAge, which is left out only when negative.
type FetchOptions struct {
	Mode   string        // ModeOffline, ModeCache (Cortex's default) or ModeRefresh
	MaxAge time.Duration // cache: a stored answer younger than this is served; 0 asks the host every time; negative: the host's max_age
	Stale  string        // StaleIfError (Cortex's default) or StaleNever
	Expect string        // sha256:<hex>: the content must have this hash
	Source string        // what the request is for (metrics, logs); see WithSource
	At     time.Time     // the version that was current at that time (implies offline); zero: the current one

	// The request headers of the fetch. Accept and Accept-Language are part of what Cortex
	// stores an answer under; User-Agent is passed on to the host, which otherwise gets
	// Cortex's own.
	Accept, AcceptLanguage, UserAgent string
}

type sourceKey struct{}

// WithSource returns a context that names what its requests are for (qis_tree, statute,
// …): the Transport sends it as the source of each fetch. Without a Cortex client in the
// way it changes nothing.
func WithSource(ctx context.Context, source string) context.Context {
	return context.WithValue(ctx, sourceKey{}, source)
}

func sourceOf(ctx context.Context) string {
	s, _ := ctx.Value(sourceKey{}).(string)
	return s
}

// CheckedAt is when Cortex last fetched the content of an answer from its host (header
// Cortex-Checked-At): for an answer from its store, earlier than the request. Zero when
// the header is missing or invalid, as in an answer that did not come through Cortex.
func CheckedAt(h http.Header) time.Time {
	t, err := time.Parse(time.RFC3339Nano, h.Get("Cortex-Checked-At"))
	if err != nil {
		return time.Time{}
	}
	return t
}

// ErrorCode is Cortex's code of an error answer (header Cortex-Error), such as
// "upstream-failed", "wrong-type" or "host-busy"; "" for an answer of the host itself.
func ErrorCode(resp *http.Response) string {
	if resp == nil {
		return ""
	}
	return resp.Header.Get("Cortex-Error")
}

// Fetch asks Cortex for rawURL. The answer is the host's (status, body and the headers
// Cortex keeps) or Cortex's own error (ErrorCode, ResponseError); the error is for a
// request no instance answered. The caller closes the body. The source is o.Source, else
// the one of ctx (WithSource), else "unknown".
func (c *Client) Fetch(ctx context.Context, rawURL string, o FetchOptions) (*http.Response, error) {
	if err := checkFetchURL(rawURL); err != nil {
		return nil, err
	}
	source := o.Source
	if source == "" {
		source = sourceOf(ctx)
	}
	header := make(http.Header)
	setFetchHeaders(header, o.Accept, o.AcceptLanguage, o.UserAgent)
	return c.do(ctx, request{method: http.MethodGet, target: fetchTarget(rawURL, o, source), header: header})
}

// Transport returns a RoundTripper that sends GET and HEAD requests to Cortex instead of to
// their host: GET https://host/path becomes GET {instance}/v1/fetch?url=…&mode=…&max_age=…
// &stale=…&source=… with o's parameters. The request's User-Agent, Accept and
// Accept-Language go along (o's when the request has none); nothing else of it does. The
// source is the one of the request's context (WithSource), else o.Source, else "unknown".
// The answer's Request is the original request, which is not changed. Other methods are
// refused. An answer of Cortex's own (Cortex-Error, see ResponseError) comes back as a
// response, with Cortex's status: a caller that takes a 404 for the host's has to look.
func (c *Client) Transport(o FetchOptions) http.RoundTripper {
	return &transport{c: c, o: o}
}

// HTTPClient returns an http.Client that sends its requests through Transport(o), each
// bounded by timeout (0: none).
func (c *Client) HTTPClient(o FetchOptions, timeout time.Duration) *http.Client {
	return &http.Client{Transport: c.Transport(o), Timeout: timeout}
}

type transport struct {
	c *Client
	o FetchOptions
}

func (t *transport) RoundTrip(req *http.Request) (*http.Response, error) {
	if req.Body != nil {
		// A RoundTripper closes the body; a fetch sends none.
		_ = req.Body.Close()
	}
	method := req.Method
	if method == "" {
		method = http.MethodGet
	}
	if req.URL == nil {
		return nil, fmt.Errorf("cortex: request without URL")
	}
	target := *req.URL
	target.Fragment, target.RawFragment = "", ""
	rawURL := target.String()
	if method != http.MethodGet && method != http.MethodHead {
		return nil, fmt.Errorf("cortex: %s %s: only GET and HEAD go through Cortex", method, target.Redacted())
	}
	if err := checkFetchURL(rawURL); err != nil {
		return nil, err
	}

	source := sourceOf(req.Context())
	if source == "" {
		source = t.o.Source
	}
	header := make(http.Header)
	userAgent := t.o.UserAgent
	if _, ok := req.Header["User-Agent"]; ok {
		userAgent = req.Header.Get("User-Agent")
	}
	setFetchHeaders(header, joined(req.Header, "Accept", t.o.Accept), joined(req.Header, "Accept-Language", t.o.AcceptLanguage), userAgent)

	resp, err := t.c.do(req.Context(), request{method: method, target: fetchTarget(rawURL, t.o, source), header: header})
	if err != nil {
		return nil, err
	}
	resp.Request = req
	return resp, nil
}

// joined is the request header name with all its values, else fallback.
func joined(h http.Header, name, fallback string) string {
	if values := h.Values(name); len(values) > 0 {
		return strings.Join(values, ", ")
	}
	return fallback
}

// setFetchHeaders sets the headers of a fetch. Without a User-Agent none is sent, rather
// than Go's: Cortex then sends its own to the host.
func setFetchHeaders(h http.Header, accept, acceptLanguage, userAgent string) {
	if accept != "" {
		h.Set("Accept", accept)
	}
	if acceptLanguage != "" {
		h.Set("Accept-Language", acceptLanguage)
	}
	h["User-Agent"] = []string{userAgent}
}

func checkFetchURL(rawURL string) error {
	u, err := url.Parse(rawURL)
	if err != nil {
		return fmt.Errorf("cortex: %w", err)
	}
	if (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" {
		return fmt.Errorf("cortex: %s: only absolute http and https URLs can be fetched", u.Redacted())
	}
	return nil
}

// fetchTarget is the path and query of GET /v1/fetch, its parameters in the order of the
// documentation.
func fetchTarget(rawURL string, o FetchOptions, source string) string {
	var b strings.Builder
	b.WriteString("/v1/fetch?url=")
	b.WriteString(url.QueryEscape(rawURL))
	add := func(name, value string) {
		if value != "" {
			b.WriteString("&" + name + "=" + url.QueryEscape(value))
		}
	}
	add("mode", o.Mode)
	if o.MaxAge >= 0 {
		add("max_age", formatDuration(o.MaxAge))
	}
	add("stale", o.Stale)
	add("expect", o.Expect)
	if !o.At.IsZero() {
		add("at", o.At.UTC().Format(time.RFC3339Nano))
	}
	if source == "" {
		source = DefaultSource
	}
	add("source", source)
	return b.String()
}

// formatDuration writes a Go duration without its zero minutes and seconds: 1h, not 1h0m0s.
func formatDuration(d time.Duration) string {
	if d == 0 {
		return "0"
	}
	s := d.String()
	if strings.HasSuffix(s, "m0s") {
		s = s[:len(s)-2]
	}
	if strings.HasSuffix(s, "h0m") {
		s = s[:len(s)-2]
	}
	return s
}
