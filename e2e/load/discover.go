package main

import (
	"bufio"
	"compress/gzip"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"regexp"
	"strings"
	"sync"
	"time"
)

var (
	hrefRe = regexp.MustCompile(`href="([^"]+)"`)
	locRe  = regexp.MustCompile(`<loc>([^<]+)</loc>`)
)

// notPages are addresses a crawler meets in the HTML that are no server-rendered pages.
var notPages = []string{"/api/", "/assets/", "/pkg/", "/cards/", "/calendar/", "/sw.js", "/favicon", "/manifest", "/apple-touch", "/access", "/robots.txt", "/sitemap.xml"}

// localPath is the path of a link that stays on the site (`host` as in the sitemap's addresses).
func localPath(href, host string) (string, bool) {
	href = strings.NewReplacer("&amp;", "&", "&quot;", `"`, "&#x27;", "'", "&#39;", "'").Replace(href)
	if i := strings.IndexByte(href, '#'); i >= 0 {
		href = href[:i]
	}
	if strings.HasPrefix(href, "http://") || strings.HasPrefix(href, "https://") {
		rest := href[strings.Index(href, "//")+2:]
		slash := strings.IndexByte(rest, '/')
		if slash < 0 {
			rest += "/"
			slash = len(rest) - 1
		}
		// Links to QIS, b-tu.de or OPUS are not the site's.
		if host == "" || rest[:slash] != host {
			return "", false
		}
		href = rest[slash:]
	}
	if !strings.HasPrefix(href, "/") || strings.HasPrefix(href, "//") {
		return "", false
	}
	for _, p := range notPages {
		if strings.HasPrefix(href, p) {
			return "", false
		}
	}
	return href, true
}

// discover crawls the server-rendered site like a search engine: the sitemap, then every link
// breadth first. It writes one line per page: path, kind, status, bytes on the wire, ms, x-cache.
func discover(c *http.Client, base string, max, conc int, outPath string, nosite bool) error {
	out, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer out.Close()
	w := bufio.NewWriter(out)
	defer w.Flush()

	seen := map[string]bool{}
	var queue []string
	var mu sync.Mutex
	push := func(p string) {
		if len(seen) >= max || seen[p] {
			return
		}
		seen[p] = true
		queue = append(queue, p)
	}
	for _, p := range []string{"/", "/catalog", "/programs"} {
		push(p)
	}
	// The site's own host: the base's, or the public address its sitemap is written with.
	host := strings.TrimPrefix(strings.TrimPrefix(base, "https://"), "http://")
	if !nosite {
		if body, err := get(c, base+"/sitemap.xml"); err == nil {
			if m := locRe.FindStringSubmatch(body); m != nil {
				rest := m[1][strings.Index(m[1], "//")+2:]
				if slash := strings.IndexByte(rest, '/'); slash > 0 {
					host = rest[:slash]
				}
			}
			for _, m := range locRe.FindAllStringSubmatch(body, -1) {
				if p, ok := localPath(m[1], host); ok {
					push(p)
				}
			}
		} else {
			fmt.Fprintln(os.Stderr, "sitemap:", err)
		}
	}
	fmt.Fprintf(os.Stderr, "discover: %d addresses from the start pages and the sitemap\n", len(queue))

	next := 0
	var wg sync.WaitGroup
	sem := make(chan struct{}, conc)
	started := time.Now()
	done := 0
	for {
		mu.Lock()
		if next >= len(queue) {
			mu.Unlock()
			// Wait for the pages in flight: they may add more.
			wg.Wait()
			mu.Lock()
			if next >= len(queue) {
				mu.Unlock()
				break
			}
		}
		p := queue[next]
		next++
		mu.Unlock()

		sem <- struct{}{}
		wg.Add(1)
		go func(p string) {
			defer wg.Done()
			defer func() { <-sem }()
			req, _ := http.NewRequestWithContext(context.Background(), http.MethodGet, base+p, nil)
			for k, v := range crawlerHeaders() {
				req.Header.Set(k, v)
			}
			req.Header.Set("Accept-Encoding", "gzip")
			t := time.Now()
			resp, err := c.Do(req)
			if err != nil {
				mu.Lock()
				fmt.Fprintf(w, "%s\t%s\t0\t0\t%.1f\t%s\n", p, kindOf(p), float64(time.Since(t).Microseconds())/1000, shortErr(err))
				mu.Unlock()
				return
			}
			raw, _ := io.ReadAll(resp.Body)
			resp.Body.Close()
			ms := float64(time.Since(t).Microseconds()) / 1000
			body := raw
			if resp.Header.Get("Content-Encoding") == "gzip" {
				if zr, err := gzip.NewReader(strings.NewReader(string(raw))); err == nil {
					body, _ = io.ReadAll(zr)
				}
			}
			mu.Lock()
			defer mu.Unlock()
			fmt.Fprintf(w, "%s\t%s\t%d\t%d\t%.1f\t%s\n", p, kindOf(p), resp.StatusCode, len(raw), ms, resp.Header.Get("x-cache"))
			done++
			if done%500 == 0 {
				fmt.Fprintf(os.Stderr, "  %d pages, %d known, %.0f pages/s\n", done, len(seen), float64(done)/time.Since(started).Seconds())
			}
			if resp.StatusCode != 200 || !strings.HasPrefix(resp.Header.Get("Content-Type"), "text/html") {
				return
			}
			for _, m := range hrefRe.FindAllStringSubmatch(string(body), -1) {
				if q, ok := localPath(m[1], host); ok {
					push(q)
				}
			}
		}(p)
	}
	fmt.Fprintf(os.Stderr, "discover: %d pages in %s\n", done, time.Since(started).Round(time.Second))
	return nil
}

func get(c *http.Client, url string) (string, error) {
	req, _ := http.NewRequest(http.MethodGet, url, nil)
	req.Header.Set("User-Agent", uaCrawler)
	resp, err := c.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	b, err := io.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		return "", fmt.Errorf("%s: HTTP %d", url, resp.StatusCode)
	}
	return string(b), err
}
