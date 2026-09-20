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

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/oplog"
)

const (
	DefaultUserAgent = "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)"

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
	UserAgent string
	Client    *http.Client
	Progress  func(done, total int, stats Stats)
}

// Stats counts the outcome per job.
type Stats struct {
	Fetched  int `json:"fetched"`   // 200, archived
	Changed  int `json:"changed"`   // of Fetched: the content differs from the archived page (or is new)
	NotFound int `json:"not_found"` // 404, archived without body
	Skipped  int `json:"skipped"`   // archived recently enough
	Failed   int `json:"failed"`    // gave up after retries
}

// ErrServerUnhealthy aborts a crawl when the server keeps failing; hammering it
// further would be impolite and pointless.
var ErrServerUnhealthy = errors.New("too many consecutive failures, crawl aborted")

// Run archives all jobs. It returns ErrServerUnhealthy after
// maxConsecutiveFailures failed jobs in a row.
//
// Log events: crawl.started, crawl.progress, crawl.retry (WARN), crawl.slow (WARN),
// crawl.not_found (WARN), crawl.job_failed (ERROR), crawl.aborted (ERROR), crawl.finished.
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
				"not_found", stats.NotFound, "failed", stats.Failed, "elapsed_s", int(time.Since(start).Seconds()))
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
					} else if !fetchedAt.IsZero() && time.Since(fetchedAt) < opt.MaxAge {
						record(func(s *Stats) { s.Skipped++ }, false, nil)
						continue
					}
				}

				status, changed, err := fetchAndArchive(ctx, db, job, opt, log)
				switch {
				case err != nil && ctx.Err() != nil && !errors.Is(err, ErrServerUnhealthy):
					return // interrupted; not a failure of the page
				case err != nil:
					log.Error("giving up on page", "event", "crawl.job_failed", "key", job.Key, "url", job.URL,
						"attempts", maxAttempts, oplog.Err(err))
					record(func(s *Stats) { s.Failed++ }, true, err)
				case status == http.StatusNotFound:
					log.Warn("page not found", "event", "crawl.not_found", "key", job.Key, "url", job.URL)
					record(func(s *Stats) { s.NotFound++ }, false, nil)
				default:
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
		"not_found", stats.NotFound, "failed", stats.Failed, "duration_s", int(time.Since(start).Seconds()),
		"aborted", runErr != nil)

	if runErr != nil {
		return stats, runErr
	}
	return stats, ctx.Err()
}

// fetchAndArchive retries transient failures with a growing pause. 200 and 404
// are final answers and are archived; everything else is an error.
func fetchAndArchive(ctx context.Context, db *catalogdb.DB, job Job, opt Options, log *slog.Logger) (int, bool, error) {
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		begin := time.Now()
		status, body, err := fetch(ctx, job.URL, opt)
		if took := time.Since(begin); took > slowResponse {
			log.Warn("slow response", "event", "crawl.slow", "key", job.Key, "duration_ms", took.Milliseconds(), "status", status)
		}

		if err == nil && (status == http.StatusOK || status == http.StatusNotFound) {
			if status == http.StatusNotFound {
				body = nil
			}
			changed, err := db.PutPageChanged(catalogdb.RawPage{
				Source:     job.Source,
				Key:        job.Key,
				URL:        job.URL,
				FetchedAt:  time.Now(),
				HTTPStatus: status,
				Body:       body,
			})
			if err != nil {
				return status, false, fmt.Errorf("failed to archive: %w", err)
			}
			return status, changed, nil
		}
		if err == nil {
			err = fmt.Errorf("unexpected status %d", status)
		}
		lastErr = err
		if ctx.Err() != nil {
			return 0, false, lastErr
		}
		if attempt < maxAttempts {
			pause := opt.Backoff * time.Duration(1<<(attempt-1))
			log.Warn("request failed, retrying", "event", "crawl.retry", "key", job.Key, "attempt", attempt,
				"status", status, "pause_s", int(pause.Seconds()), oplog.Err(err))
			sleep(ctx, pause)
		}
	}
	return 0, false, lastErr
}

func fetch(ctx context.Context, pageURL string, opt Options) (int, []byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, pageURL, nil)
	if err != nil {
		return 0, nil, err
	}
	req.Header.Set("User-Agent", opt.UserAgent)

	resp, err := opt.Client.Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxBodyBytes))
	if err != nil {
		return resp.StatusCode, nil, err
	}
	return resp.StatusCode, body, nil
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
