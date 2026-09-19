// Package crawl politely downloads source pages into the raw page archive.
package crawl

import (
	"context"
	"errors"
	"fmt"
	"io"
	"math/rand"
	"net/http"
	"sync"
	"time"

	"github.com/jakob/btu-scraper/internal/catalogdb"
)

const (
	DefaultUserAgent = "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)"

	maxAttempts            = 3
	maxConsecutiveFailures = 10
	maxBodyBytes           = 16 << 20
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
	Fetched  int // 200, archived
	NotFound int // 404, archived without body
	Skipped  int // archived recently enough
	Failed   int // gave up after retries
}

// ErrServerUnhealthy aborts a crawl when the server keeps failing; hammering it
// further would be impolite and pointless.
var ErrServerUnhealthy = errors.New("too many consecutive failures, crawl aborted")

// Run archives all jobs. It returns ErrServerUnhealthy after
// maxConsecutiveFailures failed jobs in a row.
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
				cancel()
			}
		} else {
			consecutive = 0
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
					if err == nil && !fetchedAt.IsZero() && time.Since(fetchedAt) < opt.MaxAge {
						record(func(s *Stats) { s.Skipped++ }, false, nil)
						continue
					}
				}

				status, err := fetchAndArchive(ctx, db, job, opt)
				switch {
				case err != nil:
					record(func(s *Stats) { s.Failed++ }, true, err)
				case status == http.StatusNotFound:
					record(func(s *Stats) { s.NotFound++ }, false, nil)
				default:
					record(func(s *Stats) { s.Fetched++ }, false, nil)
				}

				sleep(ctx, jitter(opt.Delay))
			}
		}()
	}
	wg.Wait()

	if runErr != nil {
		return stats, runErr
	}
	return stats, ctx.Err()
}

// fetchAndArchive retries transient failures with a growing pause. 200 and 404
// are final answers and are archived; everything else is an error.
func fetchAndArchive(ctx context.Context, db *catalogdb.DB, job Job, opt Options) (int, error) {
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		status, body, err := fetch(ctx, job.URL, opt)
		if err == nil && (status == http.StatusOK || status == http.StatusNotFound) {
			if status == http.StatusNotFound {
				body = nil
			}
			return status, db.PutPage(catalogdb.RawPage{
				Source:     job.Source,
				Key:        job.Key,
				URL:        job.URL,
				FetchedAt:  time.Now(),
				HTTPStatus: status,
				Body:       body,
			})
		}
		if err == nil {
			err = fmt.Errorf("unexpected status %d", status)
		}
		lastErr = fmt.Errorf("%s %s: %w", job.Source, job.Key, err)
		if ctx.Err() != nil {
			return 0, lastErr
		}
		if attempt < maxAttempts {
			sleep(ctx, opt.Backoff*time.Duration(1<<(attempt-1)))
		}
	}
	return 0, lastErr
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
		return 0, nil, err
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
