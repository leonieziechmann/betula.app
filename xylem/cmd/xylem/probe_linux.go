//go:build linux

package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"net"
	"net/url"
	"strings"
	"sync"
	"syscall"
	"time"
)

// probeTarget is one thing tried from inside Xylem's container: what the firewall forbids
// (open false), or the way out Xylem cannot do without (open true).
type probeTarget struct {
	network, addr, what string
	open                bool
}

// probeTargets are tried by every probe. Forbidden: a resolver other than Docker's, a port other
// than 443, the metadata service on any port, a private address, anything over IPv6.
var probeTargets = []probeTarget{
	{network: "udp4", addr: "1.1.1.1:53", what: "a public resolver, asked directly"},
	{network: "tcp4", addr: "1.1.1.1:80", what: "a public web server without TLS"},
	{network: "tcp4", addr: "169.254.169.254:80", what: "the metadata service"},
	{network: "tcp4", addr: "169.254.169.254:443", what: "the metadata service on 443"},
	{network: "tcp4", addr: "10.0.0.1:443", what: "a private address on 443"},
	{network: "udp4", addr: "10.0.0.1:53", what: "a private resolver"},
	{network: "udp6", addr: "[2606:4700:4700::1111]:53", what: "a public resolver over IPv6"},
	{network: "tcp6", addr: "[2606:4700:4700::1111]:443", what: "a public web server over IPv6"},
}

// probeResult is one line of the probe's output.
type probeResult struct {
	Check  string `json:"check"`
	Target string `json:"target,omitempty"`
	What   string `json:"what,omitempty"`
	Result string `json:"result"` // blocked, open; ok, failed for the privileges
	Want   string `json:"want"`
	Detail string `json:"detail,omitempty"`
}

// runProbe tries, from inside Xylem's container, what its firewall forbids and the way out it
// leaves, and checks the privileges of the service's process: one JSON object per line, each with
// what was found (result) and what should be (want); exit 1 where the two differ. The
// verification after a deploy runs it with docker exec (deploy/vps/49-xylem.sh); extra arguments
// are more TCP targets that must be closed (ip:port), such as the addresses of Traefik and of the
// host.
func runProbe(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("probe", flag.ContinueOnError)
	pid := fs.Int("pid", 1, "The process of the service whose privileges are checked")
	timeout := fs.Duration("timeout", 2*time.Second, "How long a connection may take before it counts as dropped")
	web := fs.String("web", "1.1.1.1:443", "A public address on port 443 that must answer: the way to the push services (ip:port)")
	name := fs.String("name", siteHost(), "A name Docker's resolver must know: the site's (env XYLEM_SITE)")
	if code, ok := parse(fs, args, 0, -1); !ok {
		return code
	}
	targets := append([]probeTarget(nil), probeTargets...)
	for _, a := range fs.Args() {
		if !isAddrPort(a) {
			fmt.Fprintf(stderr, "Error: %q is not ip:port\n", a)
			return 2
		}
		targets = append(targets, probeTarget{network: "tcp", addr: a, what: "given"})
	}
	if !isAddrPort(*web) {
		fmt.Fprintf(stderr, "Error: --web %q is not ip:port\n", *web)
		return 2
	}
	targets = append(targets, probeTarget{network: "tcp4", addr: *web, what: "a public web server on 443: the way to the push services", open: true})

	results := make([]probeResult, len(targets)+2)
	var wg sync.WaitGroup
	for i, t := range targets {
		wg.Add(1)
		go func() {
			defer wg.Done()
			results[i] = probeOne(ctx, t, *timeout)
		}()
	}
	wg.Add(1)
	go func() {
		defer wg.Done()
		lctx, cancel := context.WithTimeout(ctx, 3**timeout)
		defer cancel()
		r := probeResult{Check: "dns", Target: *name, What: "a name asked of Docker's resolver", Result: "blocked", Want: "open"}
		if addrs, err := net.DefaultResolver.LookupHost(lctx, *name); err == nil {
			r.Result, r.Detail = "open", strings.Join(addrs, " ")
		} else {
			r.Detail = err.Error()
		}
		results[len(targets)] = r
	}()
	wg.Wait()
	priv := probeResult{Check: "privileges", Target: fmt.Sprintf("/proc/%d/status", *pid), Result: "ok", Want: "ok"}
	if err := checkUnprivileged(priv.Target, xylemID, xylemID); err != nil {
		priv.Result, priv.Detail = "failed", err.Error()
	}
	results[len(targets)+1] = priv

	code := 0
	enc := json.NewEncoder(stdout)
	for _, r := range results {
		if r.Result != r.Want {
			code = 1
		}
		_ = enc.Encode(r)
	}
	return code
}

// siteHost is the host of XYLEM_SITE, the name the probe asks Docker's resolver for; betula.app
// where the site is not set.
func siteHost() string {
	if u, err := url.Parse(envOr("XYLEM_SITE", "")); err == nil && u.Hostname() != "" {
		return u.Hostname()
	}
	return "betula.app"
}

func isAddrPort(a string) bool {
	host, port, err := net.SplitHostPort(a)
	return err == nil && net.ParseIP(host) != nil && port != ""
}

// probeOne tries one target. Dropped here (EPERM), no route, or no answer at all: blocked. An
// answer of any kind, a refusal too, means the packet got out: open.
func probeOne(ctx context.Context, t probeTarget, timeout time.Duration) probeResult {
	r := probeResult{Check: strings.TrimRight(t.network, "46"), Target: t.addr, What: t.what, Result: "blocked", Want: "blocked"}
	if t.open {
		r.Want = "open"
	}
	if strings.HasPrefix(t.network, "udp") {
		if err := sendDropped(t.network, t.addr); err != nil {
			if errors.Is(err, syscall.ENETUNREACH) || errors.Is(err, syscall.EHOSTUNREACH) || errors.Is(err, syscall.EADDRNOTAVAIL) || errors.Is(err, syscall.EAFNOSUPPORT) {
				r.Detail = err.Error()
			} else {
				r.Result, r.Detail = "open", err.Error()
			}
		} else {
			r.Detail = "dropped here (EPERM)"
		}
		return r
	}
	dctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	c, err := (&net.Dialer{}).DialContext(dctx, t.network, t.addr)
	switch {
	case err == nil:
		_ = c.Close()
		r.Result, r.Detail = "open", "connected"
	case errors.Is(err, syscall.ECONNREFUSED):
		r.Result, r.Detail = "open", "refused: the packet got out"
	case errors.Is(err, context.DeadlineExceeded) || errors.Is(err, syscall.ETIMEDOUT):
		r.Detail = "no answer within " + timeout.String()
	default:
		r.Detail = err.Error()
	}
	return r
}
