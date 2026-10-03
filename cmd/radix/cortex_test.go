package main

import (
	"flag"
	"net/http"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/client/cortextest"
)

// fetchThrough sends one request with client and returns what the fake Cortex received.
func fetchThrough(t *testing.T, cortex *cortextest.Server, client *http.Client) cortextest.Fetch {
	t.Helper()
	before := len(cortex.Fetches())
	resp, err := client.Get("http://127.0.0.1:1/modul")
	if err != nil {
		t.Fatalf("Get: %v", err)
	}
	resp.Body.Close()
	fetches := cortex.Fetches()
	if len(fetches) != before+1 {
		t.Fatalf("Cortex got %d fetches, want %d", len(fetches), before+1)
	}
	return fetches[len(fetches)-1]
}

func TestCortexFlags(t *testing.T) {
	cortex := cortextest.NewServer()
	defer cortex.Close()
	parse := func(args ...string) cortexFlags {
		t.Helper()
		fs := flag.NewFlagSet("test", flag.ContinueOnError)
		f := addCortexFlags(fs)
		if err := fs.Parse(args); err != nil {
			t.Fatalf("Parse: %v", err)
		}
		return f
	}

	// Without RADIX_CORTEX_URL everything goes directly, as before.
	t.Setenv("RADIX_CORTEX_URL", "")
	if c := parse().client("cache"); c != nil {
		t.Errorf("client without --cortex = %+v, want nil", c)
	}
	if c := statutesClient(parse(), true); c.Transport != nil || c.Timeout != 60*time.Second {
		t.Errorf("statutes client without --cortex = %+v, want the direct one", c)
	}

	// From the environment, with the default max age.
	t.Setenv("RADIX_CORTEX_URL", cortex.URL)
	c := parse().client("cache")
	if c == nil || c.Timeout != cortexTimeout {
		t.Fatalf("client = %+v, want one through Cortex with a timeout of %v", c, cortexTimeout)
	}
	if f := fetchThrough(t, cortex, c); f.Mode != "cache" || f.MaxAge != "1h" || f.Stale != "never" || f.Source != "unknown" {
		t.Errorf("fetch %+v", f.Query)
	}

	// The flags win over the environment; a negative max age leaves it to Cortex.
	t.Setenv("RADIX_CORTEX_URL", "http://127.0.0.1:1")
	t.Setenv("RADIX_CORTEX_MAX_AGE", "2h")
	if f := fetchThrough(t, cortex, parse("--cortex", cortex.URL).client("cache")); f.MaxAge != "2h" {
		t.Errorf("max_age from the environment = %q", f.MaxAge)
	}
	if f := fetchThrough(t, cortex, parse("--cortex", cortex.URL, "--cortex-max-age", "-1s").client("cache")); f.Query.Has("max_age") {
		t.Errorf("a negative max age was sent: %+v", f.Query)
	}

	// download-statutes --force asks OPUS again.
	t.Setenv("RADIX_CORTEX_URL", cortex.URL)
	if f := fetchThrough(t, cortex, statutesClient(parse(), true)); f.Mode != "refresh" {
		t.Errorf("--force: mode = %q, want refresh", f.Mode)
	}
	if f := fetchThrough(t, cortex, statutesClient(parse(), false)); f.Mode != "cache" {
		t.Errorf("without --force: mode = %q, want cache", f.Mode)
	}
}
