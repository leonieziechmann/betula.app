package main

import (
	"context"
	"crypto/tls"
	"io"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"
)

// Result is one HTTP request as the load tool saw it.
type Result struct {
	Step   int
	Label  string
	Status int
	Err    string
	Dur    time.Duration
	TTFB   time.Duration
	Bytes  int64
	Cache  string // Folia's x-cache: hit, miss or empty
}

type clientOptions struct {
	connectTo string // dial this address instead of the host's (DNS blocked, or a container address)
	h1        bool
	newConn   bool
	timeout   time.Duration
	insecure  bool
}

func newClient(o clientOptions) *http.Client {
	dialer := &net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second}
	tr := &http.Transport{
		DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
			if o.connectTo != "" {
				_, port, _ := net.SplitHostPort(addr)
				if strings.Contains(o.connectTo, ":") {
					addr = o.connectTo
				} else {
					addr = net.JoinHostPort(o.connectTo, port)
				}
			}
			// IPv4 only: over IPv6 every visitor of the site shares one rate-limit bucket.
			return dialer.DialContext(ctx, "tcp4", addr)
		},
		MaxIdleConns:        20000,
		MaxIdleConnsPerHost: 20000,
		IdleConnTimeout:     90 * time.Second,
		TLSHandshakeTimeout: 15 * time.Second,
		DisableCompression:  true, // the scenarios send Accept-Encoding themselves and count wire bytes
		DisableKeepAlives:   o.newConn,
		ForceAttemptHTTP2:   !o.h1,
		TLSClientConfig:     &tls.Config{InsecureSkipVerify: o.insecure},
	}
	if o.h1 {
		tr.TLSNextProto = map[string]func(string, *tls.Conn) http.RoundTripper{}
	}
	return &http.Client{
		Transport: tr,
		Timeout:   o.timeout,
		CheckRedirect: func(*http.Request, []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
}

// etags remembers what the server answered per path, for the conditional requests of returning
// visitors.
var etags sync.Map

type header = map[string]string

func fetch(ctx context.Context, c *http.Client, base, path, label string, h header) Result {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, base+path, nil)
	if err != nil {
		return Result{Label: label, Err: "request: " + err.Error()}
	}
	for k, v := range h {
		if v == "@etag" {
			tag, ok := etags.Load(path)
			if !ok {
				continue
			}
			v = tag.(string)
		}
		req.Header.Set(k, v)
	}
	start := time.Now()
	var ttfb time.Duration
	resp, err := c.Do(req)
	if err != nil {
		return Result{Label: label, Err: shortErr(err), Dur: time.Since(start)}
	}
	ttfb = time.Since(start)
	n, err := io.Copy(io.Discard, resp.Body)
	resp.Body.Close()
	r := Result{Label: label, Status: resp.StatusCode, Dur: time.Since(start), TTFB: ttfb, Bytes: n, Cache: resp.Header.Get("x-cache")}
	if err != nil {
		r.Err = "body: " + shortErr(err)
	}
	if tag := resp.Header.Get("ETag"); tag != "" && resp.StatusCode == 200 {
		etags.Store(path, tag)
	}
	return r
}

func shortErr(err error) string {
	s := err.Error()
	switch {
	case strings.Contains(s, "Client.Timeout") || strings.Contains(s, "deadline exceeded"):
		return "timeout"
	case strings.Contains(s, "connection refused") || strings.Contains(s, "actively refused"):
		return "refused"
	case strings.Contains(s, "connection reset") || strings.Contains(s, "forcibly closed"):
		return "reset"
	case strings.Contains(s, "EOF"):
		return "eof"
	case strings.Contains(s, "too many open files") || strings.Contains(s, "buffer space"):
		return "client-sockets"
	}
	if len(s) > 60 {
		s = s[len(s)-60:]
	}
	return s
}
