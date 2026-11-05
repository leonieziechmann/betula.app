// A bare Go reverse proxy in front of the SSE server, standing in for Traefik (also Go, net/http):
// what an idle streamed connection costs a Go proxy.
package main

import (
	"fmt"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
	"strings"
	"sync/atomic"
	"time"
)

func main() {
	target, _ := url.Parse("http://127.0.0.1:18777")
	rp := httputil.NewSingleHostReverseProxy(target)
	rp.FlushInterval = -1
	rp.Transport = &http.Transport{MaxIdleConnsPerHost: 20000, MaxConnsPerHost: 0}
	var open int64
	go func() {
		for {
			time.Sleep(2 * time.Second)
			b, _ := os.ReadFile("/proc/self/status")
			for _, l := range strings.Split(string(b), "\n") {
				if strings.HasPrefix(l, "VmRSS") {
					fmt.Fprintf(os.Stderr, "open=%d %s\n", atomic.LoadInt64(&open), l)
				}
			}
		}
	}()
	http.ListenAndServe("127.0.0.1:18778", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt64(&open, 1)
		defer atomic.AddInt64(&open, -1)
		rp.ServeHTTP(w, r)
	}))
}
