package upstream

import (
	"strconv"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/telemetry"
)

// What Cortex sends upstream, by host: a host the policy names (its exact name or "*.suffix"),
// or "other" for every host without an entry, so that the series stay few.
var (
	requestsTotal = telemetry.Registry.NewCounter("cortex_upstream_requests_total",
		"HTTP requests sent upstream, redirects included, by host (an entry of the policy, or \"other\") and answer (status code, or \"error\" without one).",
		"host", "code")
	requestSeconds = telemetry.Registry.NewHistogram("cortex_upstream_request_duration_seconds",
		"Time from sending a request upstream to the end of its body.",
		[]float64{0.1, 0.25, 0.5, 1, 2, 5, 10, 30, 60, 120, 300, 600}, "host")
	bytesTotal = telemetry.Registry.NewCounter("cortex_upstream_bytes_total",
		"Bytes of response bodies received from upstream, as content (a gzip body after decoding).", "host")
)

// seededCodes are the answers whose series exist from the start, at zero: increase() cannot see
// the first count of a series that appears in the middle of a time range (a first 503).
var seededCodes = []string{"200", "203", "204", "301", "302", "304", "404", "410", "429", "500", "502", "503", "504", "error"}

// The gauges of the hosts are read at scrape time from every Upstream that is open (one in the
// service; tests open more).
var live = struct {
	sync.Mutex
	set map[*Upstream]bool
}{set: make(map[*Upstream]bool)}

func init() {
	telemetry.Registry.NewGaugeFunc("cortex_host_queue",
		"Requests waiting for a slot of a host.",
		[]string{"host"}, func(emit func(float64, ...string)) {
			for label, g := range hostGauges() {
				emit(float64(g.queue), label)
			}
		})
	telemetry.Registry.NewGaugeFunc("cortex_host_in_flight",
		"Requests in flight to a host.",
		[]string{"host"}, func(emit func(float64, ...string)) {
			for label, g := range hostGauges() {
				emit(float64(g.inFlight), label)
			}
		})
	telemetry.Registry.NewGaugeFunc("cortex_host_paused",
		"Hosts paused by the breaker or by upstream's Retry-After: 1 or 0 for a host the policy names, the number of hosts for \"*.suffix\" and \"other\".",
		[]string{"host"}, func(emit func(float64, ...string)) {
			for label, g := range hostGauges() {
				emit(float64(g.paused), label)
			}
		})
}

type hostGauge struct{ queue, inFlight, paused int }

// hostGauges adds up the host states of every open Upstream by host label; every label of a
// policy is there, at zero when nothing happens.
func hostGauges() map[string]hostGauge {
	out := make(map[string]hostGauge)
	live.Lock()
	defer live.Unlock()
	for u := range live.set {
		p := u.Policy()
		for _, label := range p.labels() {
			out[label] = out[label]
		}
		now := u.now()
		u.mu.Lock()
		for host, s := range u.states {
			label := p.For(host).label()
			g := out[label]
			g.queue += s.queue.Len()
			g.inFlight += s.inFlight
			if now.Before(s.pausedUntil) {
				g.paused++
			}
			out[label] = g
		}
		u.mu.Unlock()
	}
	return out
}

// seed creates the series of every host label of p at zero.
func seed(p *Policy) {
	for _, label := range p.labels() {
		for _, code := range seededCodes {
			requestsTotal.Add(0, label, code)
		}
		bytesTotal.Add(0, label)
	}
}

// countRequest counts a request that was sent (or tried to be): code is the status, or
// "error" without one.
func countRequest(label string, status int, took time.Duration, bytes int64) {
	code := "error"
	if status != 0 {
		code = strconv.Itoa(status)
	}
	requestsTotal.Inc(label, code)
	requestSeconds.Observe(took.Seconds(), label)
	if bytes > 0 {
		bytesTotal.Add(float64(bytes), label)
	}
}
