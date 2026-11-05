package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// clientTimeout bounds `phloem healthcheck` and `phloem status`: the container's check gives
// 5 s.
const clientTimeout = 5 * time.Second

// get asks the local instance; it never goes through a proxy.
func get(ctx context.Context, url string) (int, []byte, error) {
	ctx, cancel := context.WithTimeout(ctx, clientTimeout)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return 0, nil, err
	}
	resp, err := (&http.Client{Transport: &http.Transport{Proxy: nil}}).Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	return resp.StatusCode, body, err
}

// runHealthcheck asks the local instance whether it is live (GET /healthz), so that the image
// needs no curl: exit 0 for a 200, else 1. As root (the image's health check) it drops to
// Phloem's user first.
func runHealthcheck(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("healthcheck", flag.ContinueOnError)
	target := fs.String("url", envOr("PHLOEM_HEALTH_URL", "http://127.0.0.1:8200/healthz"),
		"Liveness endpoint of the running instance (env PHLOEM_HEALTH_URL)")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	if err := dropIfRoot(); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	status, body, err := get(ctx, *target)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	fmt.Fprintln(stdout, strings.TrimSpace(string(body)))
	if status != http.StatusOK {
		return 1
	}
	return 0
}

// runStatus prints the status of the local instance (GET /status, which answers loopback only)
// as one JSON object.
func runStatus(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	target := fs.String("url", envOr("PHLOEM_STATUS_URL", "http://127.0.0.1:8200/status"),
		"Status endpoint of the running instance (env PHLOEM_STATUS_URL)")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	if err := dropIfRoot(); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	status, body, err := get(ctx, *target)
	if err != nil {
		if ctx.Err() != nil {
			fmt.Fprintln(stderr, "Interrupted.")
			return 130
		}
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	if status != http.StatusOK || !json.Valid(body) {
		fmt.Fprintf(stderr, "Error: %s answered %d: %s\n", *target, status, strings.TrimSpace(string(body)))
		return 1
	}
	fmt.Fprintln(stdout, strings.TrimSpace(string(body)))
	return 0
}
