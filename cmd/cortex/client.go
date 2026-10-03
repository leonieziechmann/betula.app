package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"mime"
	"net/http"
	"os"
	"path"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/client"
)

// healthTimeout bounds `cortex healthcheck`: the container's check gives it 5 s.
const healthTimeout = 5 * time.Second

// runHealthcheck asks the local instance whether it is live (GET /livez), so that the image
// needs no curl: exit 0 for a 200, else 1.
func runHealthcheck(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("healthcheck", flag.ContinueOnError)
	target := fs.String("url", envOr("CORTEX_HEALTH_URL", "http://127.0.0.1:8100/livez"),
		"Liveness endpoint of the running instance (env CORTEX_HEALTH_URL)")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	ctx, cancel := context.WithTimeout(ctx, healthTimeout)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, *target, nil)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	resp, err := (&http.Client{Transport: &http.Transport{Proxy: nil}}).Do(req)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(resp.Body, 4096))
	fmt.Fprintln(stdout, strings.TrimSpace(string(body)))
	if resp.StatusCode != http.StatusOK {
		return 1
	}
	return 0
}

// clientFlags are the flags of the commands that talk to a running Cortex.
func clientFlags(name string) (*flag.FlagSet, *string) {
	fs := flag.NewFlagSet(name, flag.ContinueOnError)
	urls := fs.String("url", envOr("CORTEX_URL", "http://127.0.0.1:8100"),
		"Cortex to ask: its instances' URLs, comma-separated (env CORTEX_URL)")
	return fs, urls
}

// newClient returns the client of --url; ok false after an invalid one (exit 2).
func newClient(urls string) (*client.Client, bool) {
	c, err := client.New(urls, client.Options{})
	if err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return nil, false
	}
	return c, true
}

// failed reports err of a command: exit 130 when the command was interrupted, else 1.
func failed(ctx context.Context, err error) int {
	if ctx.Err() != nil {
		fmt.Fprintln(stderr, "Interrupted.")
		return 130
	}
	fmt.Fprintln(stderr, "Error:", err)
	return 1
}

// runStatus prints the status of the instance that answers (GET /status) as one JSON object:
// the deploy scripts read role, epoch, seq, follower.lag_seconds and follower.state with jq.
func runStatus(ctx context.Context, args []string) int {
	fs, urls := clientFlags("status")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	c, ok := newClient(*urls)
	if !ok {
		return 2
	}
	st, err := c.Status(ctx)
	if err != nil {
		return failed(ctx, err)
	}
	if !json.Valid(st.Raw) {
		return failed(ctx, errors.New("the status is not JSON"))
	}
	fmt.Fprintln(stdout, strings.TrimSpace(string(st.Raw)))
	return 0
}

// runStepDown asks the instance that answers to hand over (POST /v1/admin/step-down): exit 0
// when it stepped down, 1 when it does not lead or cannot hand over.
func runStepDown(ctx context.Context, args []string) int {
	fs, urls := clientFlags("step-down")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	c, ok := newClient(*urls)
	if !ok {
		return 2
	}
	if err := c.StepDown(ctx); err != nil {
		if errors.Is(err, client.ErrNotLeader) {
			fmt.Fprintln(stderr, "Error: this instance does not lead; nothing to hand over.")
			return 1
		}
		return failed(ctx, err)
	}
	fmt.Fprintln(stdout, "Stepped down: the other instance takes over.")
	return 0
}

// runPut stores a file: put NAME [FILE], the content of FILE, or of stdin without it. The type
// is --type, else the one of NAME's extension, else application/octet-stream. It prints the
// version that is current afterwards as JSON.
func runPut(ctx context.Context, args []string) int {
	fs, urls := clientFlags("put")
	contentType := fs.String("type", envOr("CORTEX_CONTENT_TYPE", ""),
		"Content type of the file; empty: from the name's extension (env CORTEX_CONTENT_TYPE)")
	if code, ok := parse(fs, args, 1, 2); !ok {
		return code
	}
	c, ok := newClient(*urls)
	if !ok {
		return 2
	}
	name := fs.Arg(0)
	var body io.Reader = stdin
	if file := fs.Arg(1); file != "" {
		f, err := os.Open(file)
		if err != nil {
			fmt.Fprintln(stderr, "Error:", err)
			return 1
		}
		defer f.Close()
		body = f // seekable: sent again on a fail-over
	}
	ct := *contentType
	if ct == "" {
		ct = mime.TypeByExtension(path.Ext(name))
	}
	info, created, err := c.PutFile(ctx, name, body, client.PutOptions{ContentType: ct})
	if err != nil {
		return failed(ctx, err)
	}
	out := struct {
		client.FileInfo
		Created bool `json:"created"`
	}{info, created}
	data, _ := json.Marshal(out)
	fmt.Fprintln(stdout, string(data))
	return 0
}

// runGet writes the current content of a file to stdout: get NAME. Exit 1 when there is none,
// or the content does not match its hash.
func runGet(ctx context.Context, args []string) int {
	fs, urls := clientFlags("get")
	if code, ok := parse(fs, args, 1, 1); !ok {
		return code
	}
	c, ok := newClient(*urls)
	if !ok {
		return 2
	}
	f, err := c.GetFile(ctx, fs.Arg(0), client.GetFileOptions{})
	if err != nil {
		return failed(ctx, err)
	}
	defer f.Body.Close()
	if _, err := io.Copy(stdout, f.Body); err != nil {
		return failed(ctx, err)
	}
	return 0
}
