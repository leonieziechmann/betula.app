package main

import (
	"flag"
	"log/slog"
	"net/http"
	"os"
	"strings"
	"time"

	cortexclient "github.com/leonieziechmann/betula/internal/cortex/client"
	"github.com/leonieziechmann/betula/internal/oplog"
)

// cortexTimeout bounds one request through Cortex: Cortex keeps a request waiting in the
// queue of its host for up to a minute and then takes as long as the host does, and a
// request Radix gives up on is asked again in vain.
const cortexTimeout = 3 * time.Minute

// cortexFlags send the requests of the crawl and of the statute download through Cortex
// (docs/cortex.md) instead of to the university. Gemini is asked directly either way.
type cortexFlags struct {
	urls   *string
	maxAge *time.Duration
}

func addCortexFlags(fs *flag.FlagSet) cortexFlags {
	return cortexFlags{
		urls: fs.String("cortex", envOr("RADIX_CORTEX_URL", ""),
			"Fetch through Cortex, its instances' URLs comma-separated; empty: directly (env RADIX_CORTEX_URL)"),
		maxAge: fs.Duration("cortex-max-age", envDuration("RADIX_CORTEX_MAX_AGE", time.Hour),
			"Take a page Cortex fetched within this long from Cortex without asking the host again; negative: Cortex's per-host max_age (env RADIX_CORTEX_MAX_AGE)"),
	}
}

// client returns the HTTP client for the requests to the university: one that asks Cortex
// in mode (cortexclient.ModeCache, or ModeRefresh to fetch every page anew), stale=never
// so that a failure stays a failure for the crawl's retries, or nil, directly, without
// --cortex. An invalid --cortex exits with code 2.
//
// Log events: crawl.cortex.
func (f cortexFlags) client(mode string) *http.Client {
	if strings.TrimSpace(*f.urls) == "" {
		return nil
	}
	c, err := cortexclient.New(*f.urls, cortexclient.Options{})
	if err != nil {
		slog.Error("invalid --cortex", "component", "cli", "event", "cli.failed", "value", *f.urls, oplog.Err(err))
		os.Exit(2)
	}
	oplog.For("crawl").Info("requests go through Cortex", "event", "crawl.cortex", "cortex", strings.Join(c.Endpoints(), ","),
		"mode", mode, "max_age", f.maxAge.String())
	return c.HTTPClient(cortexclient.FetchOptions{Mode: mode, MaxAge: *f.maxAge, Stale: cortexclient.StaleNever}, cortexTimeout)
}
