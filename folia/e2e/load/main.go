// betula-load: load tests for Folia, the web server of Betula — crawlers, visitors with and
// without JavaScript, calendar services fetching subscribed Studienpläne. What it measured and
// how the lab is set up: docs/folia/frontend.md, "Load"; lab.ps1 starts the lab on Windows.
//
//	betula-load discover -base http://127.0.0.1:18080 -out pages.tsv [-max 40000] [-c 4]
//	betula-load run -base URL -scenario crawl-cold -pages pages.tsv -rates 5,10,20 -step 30s [-pid N]
//	betula-load run -base URL -scenario "crawl-cold=30,nojs=20,js-first=1,ics=20" -rates 10,20
//	betula-load run -base URL -scenario ics -feeds feeds.tsv -concs 1,4,16
//	betula-load probe -base URL -paths /livez,/robots.txt -n 20 [-newconn]
//
// Open model (-rates): jobs start on a Poisson schedule whatever the server does, so an
// overloaded server shows up as latency and errors instead of a politely lower rate. Closed model
// (-concs): that many workers loop back to back, the way a crawler with N connections does.
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"
	"time"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: betula-load discover|run|probe [flags]")
		os.Exit(2)
	}
	cmd := os.Args[1]
	fs := flag.NewFlagSet(cmd, flag.ExitOnError)
	base := fs.String("base", "http://127.0.0.1:18080", "the site")
	connectTo := fs.String("connect-to", "", "dial this address instead of the host's (IP or IP:port)")
	h1 := fs.Bool("h1", false, "HTTP/1.1 only (no HTTP/2 over TLS)")
	newConn := fs.Bool("newconn", false, "a new connection (and TLS handshake) for every request")
	timeout := fs.Duration("timeout", 60*time.Second, "per request")
	insecure := fs.Bool("insecure", false, "accept any certificate")

	// discover
	out := fs.String("out", "", "discover: pages file to write; run: JSON lines to append")
	max := fs.Int("max", 40000, "discover: most pages")
	conc := fs.Int("c", 4, "discover: parallel requests")
	nosite := fs.Bool("no-sitemap", false, "discover: start from / only")

	// run
	scenario := fs.String("scenario", "crawl-cold", "job or weighted mix: crawl-cold, crawl-warm, nojs, js-first, js-return, js-update, ics, livez")
	pagesPath := fs.String("pages", "", "pages file from discover")
	kinds := fs.String("kinds", "", "only pages of these kinds (comma separated)")
	hot := fs.Int("hot", 0, "warm scenarios draw from the first N pages only (0 = all)")
	shuffle := fs.Bool("shuffle", true, "crawl-cold: pages in random order")
	feedsPath := fs.String("feeds", "", "calendar feed paths (first column)")
	rates := fs.String("rates", "", "open model: jobs per second per step, e.g. 5,10,20")
	concs := fs.String("concs", "", "closed model: workers per step, e.g. 1,4,16")
	step := fs.Duration("step", 30*time.Second, "length of a step")
	drain := fs.Duration("drain", 90*time.Second, "how long a step waits for its jobs to finish")
	maxInflight := fs.Int("max-inflight", 4000, "jobs in flight beyond which arrivals are dropped")
	pid := fs.Int("pid", 0, "server process to sample CPU and memory of (local runs)")
	probe := fs.Bool("probe", false, "ask /livez once a second next to the load")
	abortP99 := fs.Duration("abort-p99", 0, "stop after a step whose p99 exceeds this")
	abortErr := fs.Float64("abort-errors", 0, "stop after a step with more than this share of failed or refused requests")
	name := fs.String("name", "", "name of the run in the JSON lines")
	uniform := fs.Bool("uniform", false, "evenly spaced arrivals instead of Poisson")

	// probe
	paths := fs.String("paths", "/livez", "probe: comma separated paths")
	n := fs.Int("n", 20, "probe: requests per path, one after another")

	// logstats
	logPath := fs.String("log", "", "logstats: Folia's JSON log")
	since := fs.String("since", "", "logstats: only lines with a timestamp at or after this (RFC 3339 prefix)")

	fs.Parse(os.Args[2:])
	base2 := strings.TrimRight(*base, "/")
	c := newClient(clientOptions{connectTo: *connectTo, h1: *h1, newConn: *newConn, timeout: *timeout, insecure: *insecure})

	switch cmd {
	case "discover":
		if *out == "" {
			fatal("discover needs -out")
		}
		if err := discover(c, base2, *max, *conc, *out, *nosite); err != nil {
			fatal(err.Error())
		}
	case "run":
		e := &Env{c: c, base: base2, hot: *hot}
		if *pagesPath != "" {
			lines, err := readLines(*pagesPath, 0)
			if err != nil {
				fatal(err.Error())
			}
			want := map[string]bool{}
			for _, k := range strings.Split(*kinds, ",") {
				if k != "" {
					want[k] = true
				}
			}
			for _, p := range lines {
				k := kindOf(p)
				if len(want) == 0 || want[k] {
					e.pages = append(e.pages, Page{p, k})
				}
			}
			if *shuffle && strings.Contains(*scenario, "crawl-cold") {
				r := time.Now().UnixNano()
				for i := len(e.pages) - 1; i > 0; i-- {
					r = r*6364136223846793005 + 1442695040888963407
					j := int(uint64(r)>>33) % (i + 1)
					e.pages[i], e.pages[j] = e.pages[j], e.pages[i]
				}
			}
			fmt.Fprintf(os.Stderr, "%d pages\n", len(e.pages))
		}
		if *feedsPath != "" {
			f, err := readLines(*feedsPath, 0)
			if err != nil {
				fatal(err.Error())
			}
			e.feeds = f
			fmt.Fprintf(os.Stderr, "%d feeds\n", len(e.feeds))
		}
		needsPages := strings.Contains(*scenario, "crawl") || strings.Contains(*scenario, "nojs") || strings.Contains(*scenario, "js-first") || strings.Contains(*scenario, "js-return")
		if needsPages && len(e.pages) == 0 {
			e.pages = []Page{{"/", "home"}}
		}
		if strings.Contains(*scenario, "ics") && len(e.feeds) == 0 {
			fatal("the ics scenario needs -feeds")
		}
		o := runOptions{scenario: *scenario, step: *step, drain: *drain, maxInflight: *maxInflight, pid: *pid, probe: *probe,
			abortP99: *abortP99, abortErr: *abortErr, out: *out, name: *name, uniform: *uniform}
		for _, s := range split(*rates) {
			v, err := strconv.ParseFloat(s, 64)
			if err != nil || v <= 0 {
				fatal("bad rate " + s)
			}
			o.rates = append(o.rates, v)
		}
		for _, s := range split(*concs) {
			v, err := strconv.Atoi(s)
			if err != nil || v <= 0 {
				fatal("bad concurrency " + s)
			}
			o.concs = append(o.concs, v)
		}
		if len(o.rates) == 0 && len(o.concs) == 0 {
			fatal("run needs -rates or -concs")
		}
		if err := run(e, o); err != nil {
			fatal(err.Error())
		}
	case "probe":
		e := &Env{c: c, base: base2}
		for _, p := range split(*paths) {
			var rs []Result
			for i := 0; i < *n; i++ {
				rs = append(rs, fetch(context.Background(), e.c, e.base, p, p, header{"User-Agent": uaCrawler, "Accept-Encoding": "gzip"}))
			}
			s := summarize(rs)
			codes := []string{}
			for c, k := range s.Status {
				codes = append(codes, fmt.Sprintf("%s:%d", c, k))
			}
			sort.Strings(codes)
			fmt.Printf("%-40s n=%-4d p50 %7.1f  p90 %7.1f  max %7.1f ms  ttfb50 %6.1f  %s %v\n", trunc(p, 40), s.N, s.P50, s.P90, s.Max, s.TTFB50, strings.Join(codes, " "), s.Errors)
		}
	case "logstats":
		if err := logstats(*logPath, *since); err != nil {
			fatal(err.Error())
		}
	default:
		fatal("unknown command " + cmd)
	}
}

func split(s string) []string {
	var out []string
	for _, p := range strings.Split(s, ",") {
		if p = strings.TrimSpace(p); p != "" {
			out = append(out, p)
		}
	}
	return out
}

func trunc(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n-1] + "…"
}

func fatal(msg string) {
	fmt.Fprintln(os.Stderr, "betula-load:", msg)
	os.Exit(1)
}
