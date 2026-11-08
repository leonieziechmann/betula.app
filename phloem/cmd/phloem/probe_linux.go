//go:build linux

package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"net"
	"strings"
	"sync"
	"syscall"
	"time"
)

// probeTarget is one thing the firewall forbids. from, when set, is the local UDP port the
// datagram leaves from (a relay's, with TURN).
type probeTarget struct {
	network, addr, what string
	from                int
}

// probeTargets are tried by every probe: Docker's resolver, a public resolver, a public web
// server, the cloud's metadata address, and the same over IPv6.
var probeTargets = []probeTarget{
	{network: "udp4", addr: "127.0.0.11:53", what: "Docker's resolver"},
	{network: "tcp4", addr: "127.0.0.11:53", what: "Docker's resolver"},
	{network: "udp4", addr: "1.1.1.1:53", what: "a public resolver"},
	{network: "tcp4", addr: "1.1.1.1:443", what: "a public web server"},
	{network: "tcp4", addr: "169.254.169.254:80", what: "the metadata service"},
	{network: "udp6", addr: "[2606:4700:4700::1111]:53", what: "a public resolver over IPv6"},
	{network: "tcp6", addr: "[2606:4700:4700::1111]:443", what: "a public web server over IPv6"},
}

// probeResult is one line of the probe's output.
type probeResult struct {
	Check  string `json:"check"`
	Target string `json:"target,omitempty"`
	What   string `json:"what,omitempty"`
	Result string `json:"result"` // blocked, open; ok, failed for the privileges
	Detail string `json:"detail,omitempty"`
}

// runProbe tries, from inside Phloem's container, what its firewall forbids, and checks the
// privileges of the service's process: one JSON object per line, exit 1 if anything got out
// or the service has a privilege. The verification after a deploy runs it with docker exec
// (deploy/vps/49-phloem.sh); extra arguments are more TCP targets (ip:port), such as the
// addresses of Traefik and the monitoring.
func runProbe(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("probe", flag.ContinueOnError)
	pid := fs.Int("pid", 1, "The process of the service whose privileges are checked")
	timeout := fs.Duration("timeout", 2*time.Second, "How long a connection may take before it counts as dropped")
	if code, ok := parse(fs, args, 0, -1); !ok {
		return code
	}
	if err := dropIfRoot(); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	targets := append([]probeTarget(nil), probeTargets...)
	for _, a := range fs.Args() {
		host, port, err := net.SplitHostPort(a)
		if err != nil || net.ParseIP(host) == nil || port == "" {
			fmt.Fprintf(stderr, "Error: %q is not ip:port\n", a)
			return 2
		}
		targets = append(targets, probeTarget{network: "tcp", addr: a, what: "given"})
	}
	// With TURN, a relay's port lets UDP out to public addresses alone: from one of them, the
	// addresses that are none, and Phloem's own, must stay closed.
	if turn, err := turnFirewallFromEnv(); err == nil && turn.on() {
		for _, t := range []probeTarget{
			{network: "udp4", addr: "10.0.0.1:9", what: "a private address, from a relay's port"},
			{network: "udp4", addr: "169.254.169.254:80", what: "the metadata service, from a relay's port"},
			{network: "udp4", addr: "127.0.0.11:53", what: "Docker's resolver, from a relay's port"},
			{network: "udp4", addr: net.JoinHostPort(turn.ip.String(), "22"), what: "Phloem's own address, from a relay's port"},
		} {
			t.from = turn.relayMax
			targets = append(targets, t)
		}
	}

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
		lctx, cancel := context.WithTimeout(ctx, *timeout)
		defer cancel()
		r := probeResult{Check: "dns", Target: "betula.app", What: "a name lookup", Result: "blocked"}
		if addrs, err := net.DefaultResolver.LookupHost(lctx, "betula.app"); err == nil {
			r.Result, r.Detail = "open", strings.Join(addrs, " ")
		} else {
			r.Detail = err.Error()
		}
		results[len(targets)] = r
	}()
	wg.Wait()
	priv := probeResult{Check: "privileges", Target: fmt.Sprintf("/proc/%d/status", *pid), Result: "ok"}
	if err := checkUnprivileged(priv.Target, phloemID, phloemID); err != nil {
		priv.Result, priv.Detail = "failed", err.Error()
	}
	results[len(targets)+1] = priv

	code := 0
	enc := json.NewEncoder(stdout)
	for _, r := range results {
		if r.Result == "open" || r.Result == "failed" {
			code = 1
		}
		_ = enc.Encode(r)
	}
	return code
}

// probeOne tries one target. Dropped here (EPERM), no route, or no answer at all: blocked. An
// answer of any kind, a refusal too, means the packet got out: open.
func probeOne(ctx context.Context, t probeTarget, timeout time.Duration) probeResult {
	r := probeResult{Check: strings.TrimRight(t.network, "46"), Target: t.addr, What: t.what, Result: "blocked"}
	if strings.HasPrefix(t.network, "udp") {
		send := func() error { return sendDropped(t.network, t.addr) }
		if t.from != 0 {
			send = func() error { return sendDroppedFrom(t.from, t.addr) }
		}
		if err := send(); err != nil {
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

// sendDroppedFrom is sendDropped from a UDP port of its own: the highest of the relays' that is
// free, going down (a relay may hold the one asked for).
func sendDroppedFrom(port int, addr string) error {
	to, err := net.ResolveUDPAddr("udp4", addr)
	if err != nil {
		return err
	}
	var c *net.UDPConn
	for p := port; p > port-16 && p >= 1024; p-- {
		if c, err = net.ListenUDP("udp4", &net.UDPAddr{Port: p}); err == nil {
			break
		}
	}
	if err != nil {
		return err
	}
	defer c.Close()
	_, err = c.WriteToUDP(dnsQuery(), to)
	switch {
	case err == nil:
		return errors.New("the datagram was sent")
	case errors.Is(err, syscall.EPERM):
		return nil
	default:
		return err
	}
}
