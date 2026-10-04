package main

import (
	"bufio"
	"context"
	"fmt"
	"math/rand"
	"net/http"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

// Page is an address of a server-rendered page and what kind of page it is.
type Page struct {
	Path string
	Kind string
}

func kindOf(path string) string {
	p, query, _ := strings.Cut(path, "?")
	switch {
	case p == "/" || p == "":
		return "home"
	case strings.HasPrefix(p, "/catalog/module/"):
		return "module"
	case p == "/catalog" && query == "":
		return "catalog"
	case p == "/catalog":
		return "catalog-filter"
	case p == "/programs" && query == "":
		return "programs"
	case p == "/programs":
		return "programs-filter"
	case strings.HasPrefix(p, "/programs/"):
		parts := strings.Split(strings.Trim(p, "/"), "/")
		tab := "plan"
		if len(parts) >= 3 {
			tab = parts[2]
		}
		if query != "" {
			return "program-" + tab + "-q"
		}
		return "program-" + tab
	default:
		return "other"
	}
}

// Env is what every job of a run shares.
type Env struct {
	c        *http.Client
	base     string
	pages    []Page
	hot      int // the warm scenarios draw from the first `hot` pages (popular first)
	feeds    []string
	coldNext atomic.Int64
	coldWrap atomic.Bool
	seed     atomic.Int64
	rngPool  sync.Pool
}

func (e *Env) rng() *rand.Rand {
	if r, ok := e.rngPool.Get().(*rand.Rand); ok {
		return r
	}
	return rand.New(rand.NewSource(time.Now().UnixNano() + e.seed.Add(7919)))
}

func (e *Env) put(r *rand.Rand) { e.rngPool.Put(r) }

// zipfPage draws a page the way visitors do: a few pages very often, most pages rarely.
func (e *Env) zipfPage(r *rand.Rand) Page {
	n := e.hot
	if n <= 0 || n > len(e.pages) {
		n = len(e.pages)
	}
	z := rand.NewZipf(r, 1.1, 8, uint64(n-1))
	return e.pages[z.Uint64()]
}

func (e *Env) coldPage() Page {
	i := e.coldNext.Add(1) - 1
	if i >= int64(len(e.pages)) {
		e.coldWrap.Store(true)
	}
	return e.pages[i%int64(len(e.pages))]
}

const (
	uaCrawler  = "Mozilla/5.0 (compatible; betula-load/1.0; +load test of the owner)"
	uaBrowser  = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 betula-load/1.0"
	uaCalendar = "Google-Calendar-Importer betula-load/1.0"
	acceptHTML = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
)

func crawlerHeaders() header {
	return header{"User-Agent": uaCrawler, "Accept": acceptHTML, "Accept-Encoding": "gzip, deflate, br"}
}

func browserHeaders(accept string) header {
	return header{"User-Agent": uaBrowser, "Accept": accept, "Accept-Encoding": "gzip, deflate, br, zstd"}
}

func conditional(h header) header {
	h["If-None-Match"] = "@etag"
	return h
}

// A job is what one arrival does: one request, or a visitor's burst of them.
type Job func(ctx context.Context, e *Env) []Result

var jobs = map[string]Job{
	// A crawler on its first pass: every address once, so every page is rendered.
	"crawl-cold": func(ctx context.Context, e *Env) []Result {
		p := e.coldPage()
		return []Result{fetch(ctx, e.c, e.base, p.Path, "page:"+p.Kind, crawlerHeaders())}
	},
	// A crawler on pages that are cached already (popular ones more often).
	"crawl-warm": func(ctx context.Context, e *Env) []Result {
		r := e.rng()
		p := e.zipfPage(r)
		e.put(r)
		return []Result{fetch(ctx, e.c, e.base, p.Path, "page:"+p.Kind, crawlerHeaders())}
	},
	// A page view without JavaScript: the page, the stylesheet and the font (revalidated: they
	// are no-cache; a third of the views are a visitor's first one and fetch them in full).
	"nojs": func(ctx context.Context, e *Env) []Result {
		r := e.rng()
		p := e.zipfPage(r)
		first := r.Float64() < 0.3
		e.put(r)
		out := []Result{fetch(ctx, e.c, e.base, p.Path, "page:"+p.Kind, browserHeaders(acceptHTML))}
		assets := []struct{ path, label, accept string }{
			{"/assets/app.css", "asset:css", "text/css,*/*;q=0.1"},
			{"/assets/inter-latin.woff2", "asset:font", "*/*"},
		}
		for _, res := range parallel(ctx, len(assets), func(i int) Result {
			h := browserHeaders(assets[i].accept)
			if !first {
				h = conditional(h)
			}
			return fetch(ctx, e.c, e.base, assets[i].path, assets[i].label, h)
		}) {
			out = append(out, res)
		}
		return out
	},
	// A visitor's first visit with JavaScript: the page, its files, then boot.js's downloads
	// (the whole catalog as /api/db, the app's WASM, sql.js), then the service worker's install,
	// which revalidates what it keeps.
	"js-first": func(ctx context.Context, e *Env) []Result {
		start := time.Now()
		r := e.rng()
		p := Page{"/", "home"}
		if r.Float64() < 0.5 {
			p = e.zipfPage(r)
		}
		e.put(r)
		out := []Result{fetch(ctx, e.c, e.base, p.Path, "page:"+p.Kind, browserHeaders(acceptHTML))}
		wave := func(paths []string, cond bool) {
			out = append(out, parallel(ctx, len(paths), func(i int) Result {
				h := browserHeaders("*/*")
				if cond {
					h = conditional(h)
				}
				return fetch(ctx, e.c, e.base, paths[i], labelOf(paths[i]), h)
			})...)
		}
		wave([]string{"/assets/app.css", "/assets/inter-latin.woff2", "/assets/enhance.js", "/assets/boot.js", "/assets/favicon.svg"}, false)
		wave([]string{"/api/status", "/assets/sql-wasm.js", "/assets/sql-wasm.wasm", "/api/db", "/api/map.json", "/pkg/folia_client.js", "/pkg/folia_client_bg.wasm", "/sw.js"}, false)
		// The worker's install asks for its shell again with cache: "no-cache".
		wave([]string{"/", "/assets/app.css", "/assets/enhance.js", "/assets/boot.js", "/assets/sql-wasm.js", "/assets/sql-wasm.wasm",
			"/pkg/folia_client.js", "/pkg/folia_client_bg.wasm", "/assets/inter-latin.woff2", "/assets/favicon.svg", "/favicon.ico",
			"/apple-touch-icon.png", "/assets/icon-192.png", "/assets/icon-512.png", "/assets/icon-maskable-512.png", "/manifest.webmanifest"}, true)
		return append(out, jobResult("job:js-first", start, out))
	},
	// A returning visitor with the app installed: the page is revalidated through the service
	// worker, boot.js asks /api/status, the map is revalidated; the catalog is already there.
	"js-return": func(ctx context.Context, e *Env) []Result {
		start := time.Now()
		r := e.rng()
		p := e.zipfPage(r)
		e.put(r)
		out := []Result{fetch(ctx, e.c, e.base, p.Path, "page:"+p.Kind, conditional(browserHeaders(acceptHTML)))}
		out = append(out, parallel(ctx, 2, func(i int) Result {
			path := []string{"/api/status", "/api/map.json"}[i]
			h := browserHeaders("*/*")
			if i == 1 {
				h = conditional(h)
			}
			return fetch(ctx, e.c, e.base, path, labelOf(path), h)
		})...)
		return append(out, jobResult("job:js-return", start, out))
	},
	// A returning visitor after Radix exported new data: the same, plus the new catalog.
	"js-update": func(ctx context.Context, e *Env) []Result {
		start := time.Now()
		out := parallel(ctx, 2, func(i int) Result {
			path := []string{"/api/status", "/api/db"}[i]
			return fetch(ctx, e.c, e.base, path, labelOf(path), browserHeaders("*/*"))
		})
		return append(out, jobResult("job:js-update", start, out))
	},
	// A calendar service fetching a subscribed Studienplan.
	"ics": func(ctx context.Context, e *Env) []Result {
		r := e.rng()
		path := e.feeds[r.Intn(len(e.feeds))]
		e.put(r)
		return []Result{fetch(ctx, e.c, e.base, path, "ics", header{"User-Agent": uaCalendar, "Accept": "*/*", "Accept-Encoding": "gzip"})}
	},
	"livez": func(ctx context.Context, e *Env) []Result {
		return []Result{fetch(ctx, e.c, e.base, "/livez", "livez", header{"User-Agent": uaCrawler})}
	},
}

func labelOf(path string) string {
	switch {
	case path == "/api/db":
		return "api:db"
	case strings.HasPrefix(path, "/api/"):
		return "api:" + strings.TrimSuffix(strings.TrimPrefix(path, "/api/"), ".json")
	case strings.HasSuffix(path, ".wasm"):
		return "asset:wasm"
	case strings.HasSuffix(path, ".js"):
		return "asset:js"
	case strings.HasSuffix(path, ".css"):
		return "asset:css"
	case strings.HasSuffix(path, ".woff2"):
		return "asset:font"
	case path == "/":
		return "page:home"
	default:
		return "asset:icon"
	}
}

// jobResult sums a visitor's requests up as one line: how long the whole visit took on the wire.
func jobResult(label string, start time.Time, parts []Result) Result {
	res := Result{Label: label, Status: 200, Dur: time.Since(start)}
	for _, p := range parts {
		res.Bytes += p.Bytes
		if p.Err != "" && res.Err == "" {
			res.Err = p.Err
		}
		if p.Status >= 400 && res.Status < p.Status {
			res.Status = p.Status
		}
	}
	return res
}

func parallel(ctx context.Context, n int, f func(i int) Result) []Result {
	out := make([]Result, n)
	var wg sync.WaitGroup
	for i := 0; i < n; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			out[i] = f(i)
		}(i)
	}
	wg.Wait()
	return out
}

// mix parses "crawl-cold=40,nojs=10" into a weighted chooser of jobs.
func mix(spec string) (func(r *rand.Rand) string, error) {
	type w struct {
		name string
		cum  float64
	}
	var ws []w
	total := 0.0
	for _, part := range strings.Split(spec, ",") {
		name, weight, ok := strings.Cut(strings.TrimSpace(part), "=")
		if !ok {
			weight = "1"
		}
		if _, known := jobs[name]; !known {
			return nil, fmt.Errorf("unknown scenario %q", name)
		}
		v, err := strconv.ParseFloat(weight, 64)
		if err != nil || v <= 0 {
			return nil, fmt.Errorf("bad weight in %q", part)
		}
		total += v
		ws = append(ws, w{name, total})
	}
	return func(r *rand.Rand) string {
		x := r.Float64() * total
		i := sort.Search(len(ws), func(i int) bool { return ws[i].cum > x })
		if i >= len(ws) {
			i = len(ws) - 1
		}
		return ws[i].name
	}, nil
}

func readLines(path string, col int) ([]string, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var out []string
	s := bufio.NewScanner(f)
	s.Buffer(make([]byte, 1<<20), 1<<20)
	for s.Scan() {
		line := strings.TrimSpace(s.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		fields := strings.Split(line, "\t")
		if col < len(fields) {
			out = append(out, fields[col])
		}
	}
	return out, s.Err()
}
