// Package crawl politely downloads source pages into the raw page archive.
package crawl

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"math/rand"
	"net/http"
	"sync"
	"time"

	cortexclient "github.com/leonieziechmann/betula/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

const (
	// DefaultUserAgent names the collector and how to reach its owner. b-tu.de/robots.txt
	// disallows /qisserver3/ for every robot, and the catalog is maintained there, so a
	// contact address lets the university's administrators write instead of only block.
	DefaultUserAgent = "Betula-Radix/1.0 (+https://betula.app; info@betula.app)"

	maxAttempts            = 3
	maxConsecutiveFailures = 10
	maxBodyBytes           = 16 << 20
	slowResponse           = 15 * time.Second
	progressEvery          = 250
)

// Job is one page to archive.
type Job struct {
	Source string
	Key    string
	URL    string
}

// Options controls pace and resume behaviour.
type Options struct {
	Workers   int           // parallel fetchers (default 1)
	Delay     time.Duration // pause per worker after each request, ±30 % jitter
	Backoff   time.Duration // first pause after a failed request, doubled per attempt (default 30 s)
	MaxAge    time.Duration // skip pages archived more recently than this; 0 fetches everything
	Spread    bool          // MaxAge is a period: every page is fetched once per period, at a time of its own (Due)
	UserAgent string
	Client    *http.Client // nil: a client of its own; Cortex's (cortex/client, the Go module cortex/) asks Cortex instead of the server
	Progress  func(done, total int, stats Stats)
}

// Stats counts the outcome per job.
type Stats struct {
	Fetched     int `json:"fetched"`                // 200, archived
	Changed     int `json:"changed"`                // of Fetched: the content differs from the archived page (or is new)
	NotFound    int `json:"not_found"`              // 404, archived without body
	Skipped     int `json:"skipped"`                // archived recently enough
	Failed      int `json:"failed"`                 // gave up after retries
	OfflineMiss int `json:"offline_miss,omitempty"` // offline through Cortex: not in its store, left as archived
}

// ErrServerUnhealthy aborts a crawl when the server keeps failing; hammering it
// further would be impolite and pointless.
var ErrServerUnhealthy = errors.New("too many consecutive failures, crawl aborted")

// ErrOfflineMiss is Cortex's answer, in mode offline (RADIX_CORTEX_MODE=offline), for a page it
// has not stored (504 offline-miss): the page is not to be had without asking the university,
// which offline nobody does. That is no failure, neither of the university nor of the crawl: the
// page is not asked for again in this crawl, does not count towards crawl.aborted and stays as
// archived; the next cycle asks again.
var ErrOfflineMiss = errors.New("not in Cortex's store, and offline nothing is fetched")

// Run archives all jobs. It returns ErrServerUnhealthy after
// maxConsecutiveFailures failed jobs in a row.
//
// Log events: crawl.started, crawl.progress, crawl.retry (WARN), crawl.slow (WARN),
// crawl.not_found (WARN), crawl.offline_miss (DEBUG), crawl.job_failed (ERROR), crawl.aborted
// (ERROR), crawl.finished.
func Run(ctx context.Context, db *catalogdb.DB, jobs []Job, opt Options) (Stats, error) {
	if opt.Workers <= 0 {
		opt.Workers = 1
	}
	if opt.Backoff <= 0 {
		opt.Backoff = 30 * time.Second
	}
	if opt.UserAgent == "" {
		opt.UserAgent = DefaultUserAgent
	}
	if opt.Client == nil {
		opt.Client = &http.Client{Timeout: 30 * time.Second}
	}

	log := oplog.For("crawl")
	if len(jobs) > 0 {
		log = log.With("source", jobs[0].Source)
	}
	start := time.Now()
	log.Info("crawl started", "event", "crawl.started", "jobs", len(jobs), "workers", opt.Workers,
		"delay_ms", opt.Delay.Milliseconds(), "max_age", opt.MaxAge.String())

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	queue := make(chan Job, len(jobs))
	for _, j := range jobs {
		queue <- j
	}
	close(queue)

	var (
		mu          sync.Mutex
		stats       Stats
		done        int
		consecutive int
		runErr      error
		wg          sync.WaitGroup
	)

	record := func(update func(*Stats), failed bool, err error) {
		mu.Lock()
		defer mu.Unlock()
		update(&stats)
		done++
		if failed {
			consecutive++
			if consecutive >= maxConsecutiveFailures && runErr == nil {
				runErr = fmt.Errorf("%w (last error: %v)", ErrServerUnhealthy, err)
				log.Error("crawl aborted: the server keeps failing", "event", "crawl.aborted",
					"consecutive_failures", consecutive, "done", done, "jobs", len(jobs), oplog.Err(err))
				cancel()
			}
		} else {
			consecutive = 0
		}
		if done%progressEvery == 0 && done < len(jobs) {
			log.Info("crawl progress", "event", "crawl.progress", "done", done, "jobs", len(jobs),
				"fetched", stats.Fetched, "changed", stats.Changed, "skipped", stats.Skipped,
				"not_found", stats.NotFound, "failed", stats.Failed, "offline_miss", stats.OfflineMiss,
				"elapsed_s", int(time.Since(start).Seconds()))
		}
		if opt.Progress != nil {
			opt.Progress(done, len(jobs), stats)
		}
	}

	for w := 0; w < opt.Workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for job := range queue {
				if ctx.Err() != nil {
					return
				}

				if opt.MaxAge > 0 {
					fetchedAt, err := db.PageFetchedAt(job.Source, job.Key)
					if err != nil {
						log.Error("cannot read the archive", "event", "crawl.archive_error", "key", job.Key, oplog.Err(err))
					} else if opt.fresh(job.Key, fetchedAt) {
						record(func(s *Stats) { s.Skipped++ }, false, nil)
						continue
					}
				}

				status, changed, err := fetchAndArchive(ctx, db, job, opt, log)
				switch {
				case err != nil && ctx.Err() != nil && !errors.Is(err, ErrServerUnhealthy):
					return // interrupted; not a failure of the page
				case errors.Is(err, ErrOfflineMiss):
					log.Debug("not in Cortex's store", "event", "crawl.offline_miss", "key", job.Key, "url", job.URL)
					CountPage(job.Source, "offline_miss")
					record(func(s *Stats) { s.OfflineMiss++ }, false, nil)
				case err != nil:
					log.Error("giving up on page", "event", "crawl.job_failed", "key", job.Key, "url", job.URL,
						"attempts", maxAttempts, oplog.Err(err))
					CountPage(job.Source, "failed")
					record(func(s *Stats) { s.Failed++ }, true, err)
				case status == http.StatusNotFound:
					log.Warn("page not found", "event", "crawl.not_found", "key", job.Key, "url", job.URL)
					CountPage(job.Source, "not_found")
					record(func(s *Stats) { s.NotFound++ }, false, nil)
				default:
					CountPage(job.Source, changedOutcome(changed))
					record(func(s *Stats) {
						s.Fetched++
						if changed {
							s.Changed++
						}
					}, false, nil)
				}

				sleep(ctx, jitter(opt.Delay))
			}
		}()
	}
	wg.Wait()

	level := slog.LevelInfo
	if stats.Failed > 0 {
		level = slog.LevelWarn
	}
	log.Log(context.Background(), level, "crawl finished", "event", "crawl.finished", "jobs", len(jobs),
		"fetched", stats.Fetched, "changed", stats.Changed, "skipped", stats.Skipped,
		"not_found", stats.NotFound, "failed", stats.Failed, "offline_miss", stats.OfflineMiss,
		"duration_s", int(time.Since(start).Seconds()), "aborted", runErr != nil)

	if runErr != nil {
		return stats, runErr
	}
	return stats, ctx.Err()
}

// fetchAndArchive retries transient failures with a growing pause. 200 and 404
// are final answers and are archived, as fetched when the server gave them (through
// Cortex, which may answer from its store, Cortex-Checked-At); everything else is an error,
// and so is every answer Cortex gave itself (fetch).
func fetchAndArchive(ctx context.Context, db *catalogdb.DB, job Job, opt Options, log *slog.Logger) (int, bool, error) {
	status, body, fetchedAt, err := fetchWithRetries(ctx, job, opt, log)
	if err != nil {
		return 0, false, err
	}
	changed, err := db.PutPageChanged(catalogdb.RawPage{
		Source:     job.Source,
		Key:        job.Key,
		URL:        job.URL,
		FetchedAt:  fetchedAt,
		HTTPStatus: status,
		Body:       body,
	})
	if err != nil {
		return status, false, fmt.Errorf("failed to archive: %w", err)
	}
	return status, changed, nil
}

// fetchWithRetries retries transient failures with a growing pause. 200 and 404 are
// final answers (a 404 without its body), returned with the time of the answer (fetch);
// everything else is an error. ErrOfflineMiss is final too: asked again, Cortex's store has the
// page no more than a moment ago.
func fetchWithRetries(ctx context.Context, job Job, opt Options, log *slog.Logger) (int, []byte, time.Time, error) {
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		begin := time.Now()
		status, body, at, err := fetch(ctx, job, opt)
		took := time.Since(begin)
		if ctx.Err() == nil {
			countRequest(job.Source, status, err, took, len(body))
		}
		if took > slowResponse {
			log.Warn("slow response", "event", "crawl.slow", "key", job.Key, "duration_ms", took.Milliseconds(), "status", status)
		}
		if errors.Is(err, ErrOfflineMiss) {
			return 0, nil, time.Time{}, err
		}

		if err == nil && (status == http.StatusOK || status == http.StatusNotFound) {
			if status == http.StatusNotFound {
				body = nil
			}
			return status, body, at, nil
		}
		if err == nil {
			err = fmt.Errorf("unexpected status %d", status)
		}
		lastErr = err
		if ctx.Err() != nil {
			return 0, nil, time.Time{}, lastErr
		}
		if attempt < maxAttempts {
			pause := opt.Backoff * time.Duration(1<<(attempt-1))
			log.Warn("request failed, retrying", "event", "crawl.retry", "key", job.Key, "attempt", attempt,
				"status", status, "pause_s", int(pause.Seconds()), oplog.Err(err))
			sleep(ctx, pause)
		}
	}
	return 0, nil, time.Time{}, lastErr
}

// fetch sends one request for the page of job. Its source goes along in the context, for
// Cortex's client to name it to Cortex. The time is when the server gave the answer: the
// time Cortex-Checked-At states for an answer through Cortex, which may come from its
// store, and otherwise now.
//
// An answer Cortex gave itself instead of the server's (Cortex-Error: a 502 upstream-failed,
// a 503 host-paused, or a 404 not-found for a path it does not serve, as below a wrong
// RADIX_CORTEX_URL) is a failed attempt, never the page: its status says nothing about the
// page, and a 404 taken from it would replace the archived page with an empty one. The
// error names Cortex's status and code; the request counts as one without an answer of the
// server (code "error"), and the attempt is retried, given up and counted towards the abort
// like any other failure. Cortex's 504 offline-miss is ErrOfflineMiss instead (code
// "offline_miss"): in mode offline the page is not there to be had.
func fetch(ctx context.Context, job Job, opt Options) (int, []byte, time.Time, error) {
	req, err := http.NewRequestWithContext(cortexclient.WithSource(ctx, job.Source), http.MethodGet, job.URL, nil)
	if err != nil {
		return 0, nil, time.Time{}, err
	}
	req.Header.Set("User-Agent", opt.UserAgent)

	resp, err := opt.Client.Do(req)
	if err != nil {
		return 0, nil, time.Time{}, err
	}
	defer resp.Body.Close()
	if err := cortexclient.ResponseError(resp); err != nil {
		if cortexclient.ErrorCode(resp) == cortexclient.CodeOfflineMiss {
			return 0, nil, time.Time{}, fmt.Errorf("%w: %v", ErrOfflineMiss, err)
		}
		return 0, nil, time.Time{}, err
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxBodyBytes))
	if err != nil {
		return resp.StatusCode, nil, time.Time{}, err
	}
	at := cortexclient.CheckedAt(resp.Header)
	if at.IsZero() {
		at = time.Now()
	}
	return resp.StatusCode, body, at, nil
}

func jitter(d time.Duration) time.Duration {
	if d <= 0 {
		return 0
	}
	return time.Duration(float64(d) * (0.7 + 0.6*rand.Float64()))
}

func sleep(ctx context.Context, d time.Duration) {
	if d <= 0 {
		return
	}
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
	case <-t.C:
	}
}
