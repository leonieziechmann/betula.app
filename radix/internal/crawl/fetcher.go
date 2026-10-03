package crawl

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
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
		if page, err := f.db.GetPage(job.Source, job.Key); err == nil && f.opt.fresh(job.Key, page.FetchedAt) {
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
	if err := f.record(ctx, job, status, changed, true, err, log); err != nil {
		return nil, err
	}
	page, err := f.db.GetPage(job.Source, job.Key)
	if err != nil {
		return nil, err
	}
	return page.Body, nil
}

// Download fetches a page with the politeness, retries and logging of Get, but neither
// reads nor writes the archive: for an answer the caller takes apart and archives piece
// by piece (the QIS event search states a few hundred events in one page).
func (f *Fetcher) Download(ctx context.Context, job Job) ([]byte, error) {
	log := oplog.For("crawl").With("source", job.Source)
	status, body, err := fetchWithRetries(ctx, job, f.opt, log)
	defer sleep(ctx, jitter(f.opt.Delay))
	if err := f.record(ctx, job, status, false, false, err, log); err != nil {
		return nil, err
	}
	return body, nil
}

// record counts the outcome of a request and turns it into the error Get and Download
// return: ErrNotFound, ErrServerUnhealthy after too many failures in a row, or the failure.
// archived says whether the answer is a page of the archive (Get), whose outcome counts
// as a page; the pieces of a Download are counted by its caller (CountPage).
func (f *Fetcher) record(ctx context.Context, job Job, status int, changed, archived bool, err error, log *slog.Logger) error {
	switch {
	case err != nil && ctx.Err() != nil:
		return ctx.Err()
	case err != nil:
		CountPage(job.Source, "failed")
		f.stats.Failed++
		f.consecutive++
		log.Error("giving up on page", "event", "crawl.job_failed", "key", job.Key, "url", job.URL, "attempts", maxAttempts, oplog.Err(err))
		if f.consecutive >= maxConsecutiveFailures {
			log.Error("crawl aborted: the server keeps failing", "event", "crawl.aborted", "consecutive_failures", f.consecutive, oplog.Err(err))
			return fmt.Errorf("%w (last error: %v)", ErrServerUnhealthy, err)
		}
		return err
	case status == http.StatusNotFound:
		f.consecutive = 0
		f.stats.NotFound++
		if archived {
			CountPage(job.Source, "not_found")
		}
		log.Warn("page not found", "event", "crawl.not_found", "key", job.Key, "url", job.URL)
		return ErrNotFound
	}
	f.consecutive = 0
	f.stats.Fetched++
	if archived {
		CountPage(job.Source, changedOutcome(changed))
	}
	if changed {
		f.stats.Changed++
	}
	if done := f.stats.Fetched; done%progressEvery == 0 {
		log.Info("crawl progress", "event", "crawl.progress", "fetched", done, "changed", f.stats.Changed,
			"from_archive", f.stats.Skipped, "not_found", f.stats.NotFound, "failed", f.stats.Failed)
	}
	return nil
}
