package crawl

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/oplog"
)

// ErrNotFound is returned by Fetcher.Get for a page the server answers with 404.
var ErrNotFound = errors.New("page not found")

// Fetcher gets pages one at a time, for crawls that discover their URLs while
// walking (the QIS tree). It answers from the archive when the archived page is
// fresh enough and otherwise fetches with the same politeness, retries and
// logging as Run. It is not safe for concurrent use: such crawls are sequential.
type Fetcher struct {
	db          *catalogdb.DB
	opt         Options
	stats       Stats
	consecutive int
}

// NewFetcher creates a Fetcher. opt.Workers is ignored.
func NewFetcher(db *catalogdb.DB, opt Options) *Fetcher {
	if opt.Backoff <= 0 {
		opt.Backoff = 30 * time.Second
	}
	if opt.UserAgent == "" {
		opt.UserAgent = DefaultUserAgent
	}
	if opt.Client == nil {
		opt.Client = &http.Client{Timeout: 45 * time.Second}
	}
	return &Fetcher{db: db, opt: opt}
}

// Stats returns the outcome so far.
func (f *Fetcher) Stats() Stats {
	return f.stats
}

// Get returns the body of a page. Errors: ErrNotFound, ErrServerUnhealthy (stop the
// crawl), or the error of a page that was given up after retries.
func (f *Fetcher) Get(ctx context.Context, job Job) ([]byte, error) {
	log := oplog.For("crawl").With("source", job.Source)

	if f.opt.MaxAge > 0 {
		if page, err := f.db.GetPage(job.Source, job.Key); err == nil && time.Since(page.FetchedAt) < f.opt.MaxAge {
			f.stats.Skipped++
			if page.HTTPStatus != http.StatusOK || len(page.Body) == 0 {
				return nil, ErrNotFound
			}
			return page.Body, nil
		} else if err != nil && !errors.Is(err, catalogdb.ErrNotFound) {
			log.Error("cannot read the archive", "event", "crawl.archive_error", "key", job.Key, oplog.Err(err))
		}
	}

	status, changed, err := fetchAndArchive(ctx, f.db, job, f.opt, log)
	defer sleep(ctx, jitter(f.opt.Delay))

	switch {
	case err != nil && ctx.Err() != nil:
		return nil, ctx.Err()
	case err != nil:
		f.stats.Failed++
		f.consecutive++
		log.Error("giving up on page", "event", "crawl.job_failed", "key", job.Key, "url", job.URL, "attempts", maxAttempts, oplog.Err(err))
		if f.consecutive >= maxConsecutiveFailures {
			log.Error("crawl aborted: the server keeps failing", "event", "crawl.aborted", "consecutive_failures", f.consecutive, oplog.Err(err))
			return nil, fmt.Errorf("%w (last error: %v)", ErrServerUnhealthy, err)
		}
		return nil, err
	case status == http.StatusNotFound:
		f.consecutive = 0
		f.stats.NotFound++
		log.Warn("page not found", "event", "crawl.not_found", "key", job.Key, "url", job.URL)
		return nil, ErrNotFound
	}

	f.consecutive = 0
	f.stats.Fetched++
	if changed {
		f.stats.Changed++
	}
	if done := f.stats.Fetched; done%progressEvery == 0 {
		log.Info("crawl progress", "event", "crawl.progress", "fetched", done, "changed", f.stats.Changed,
			"from_archive", f.stats.Skipped, "not_found", f.stats.NotFound, "failed", f.stats.Failed)
	}
	page, err := f.db.GetPage(job.Source, job.Key)
	if err != nil {
		return nil, err
	}
	return page.Body, nil
}
