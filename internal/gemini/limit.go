package gemini

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"
)

// The free tier of the Gemini API allows a project a number of requests per minute
// and per day for each model. Google does not publish the figures; AI Studio shows
// a project its own, and they change. The defaults are a guess at what the free
// tier grants gemini-3.5-flash-lite, not a figure checked against it: a project
// whose day allows fewer requests runs into the API's 429 for the daily quota,
// which ends the day's requests just as the limiter's own count does (see
// generate). Pass the project's figures to NewLimiter where they are known.
const (
	DefaultRequestsPerMinute = 10
	DefaultRequestsPerDay    = 900
)

// maxRetryDelay is the longest delay of a 429 that a request waits out. The free
// tier asks for less than a minute when the per-minute quota is used; a longer
// delay ends the request, and Wait refuses every request until it is over, so
// that the caller is not held up for hours.
const maxRetryDelay = 2 * time.Minute

// ErrDailyLimit says that the day's requests are used up, by the limiter's own
// count or by the API's word. Nothing gets through before the quota resets at
// midnight Pacific time, so the caller stops for this cycle instead of retrying.
var ErrDailyLimit = errors.New("gemini: daily request limit reached")

// Limiter paces the requests to the Gemini API: at most perMinute in any minute
// and perDay in a day, the day beginning at midnight Pacific time as Google's
// does. It is safe for concurrent use and may be shared by the clients of one
// API key. Its count lives in memory, so a restarted process starts the day
// afresh; the API's own 429 for the daily quota then stops it (see generate).
type Limiter struct {
	perMinute int
	perDay    int
	zone      *time.Location // where the quota's day begins

	// The clock, replaced in tests so that they never sleep.
	now   func() time.Time
	sleep func(context.Context, time.Duration) error

	mu        sync.Mutex
	recent    []time.Time // when the requests of the last minute went out, oldest first
	day       time.Time   // midnight of the day that today counts
	today     int         // requests of that day
	used      bool        // the API said that day's quota is used
	notBefore time.Time   // a 429 asked to wait until then
}

// NewLimiter returns a limiter for perMinute requests a minute and perDay a day.
// A value of zero or less takes the default.
func NewLimiter(perMinute, perDay int) *Limiter {
	if perMinute <= 0 {
		perMinute = DefaultRequestsPerMinute
	}
	if perDay <= 0 {
		perDay = DefaultRequestsPerDay
	}
	return &Limiter{perMinute: perMinute, perDay: perDay, zone: pacificTime(), now: time.Now, sleep: sleepContext}
}

// Wait blocks until a request may go out and counts it as sent. It returns
// ErrDailyLimit at once when the day's requests are used up rather than waiting
// for the next day, an error at once while a 429 asked to wait longer than
// maxRetryDelay, and the context's error when it ends first.
func (l *Limiter) Wait(ctx context.Context) error {
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		wait, err := l.reserve()
		if err != nil || wait <= 0 {
			return err
		}
		// Another caller may take the slot meanwhile; then this one waits again.
		if err := l.sleep(ctx, wait); err != nil {
			return err
		}
	}
}

// reserve takes a slot if one is free now, or says how long until one may be.
func (l *Limiter) reserve() (time.Duration, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	now := l.now()
	l.turnDay(now)
	if l.used || l.today >= l.perDay {
		return 0, ErrDailyLimit
	}
	if l.notBefore.Sub(now) > maxRetryDelay {
		return 0, fmt.Errorf("gemini: the api asked to wait until %s", l.notBefore.Format(time.RFC3339))
	}
	gone := 0
	for gone < len(l.recent) && now.Sub(l.recent[gone]) >= time.Minute {
		gone++
	}
	l.recent = l.recent[gone:]

	next := l.notBefore
	if len(l.recent) >= l.perMinute {
		if free := l.recent[len(l.recent)-l.perMinute].Add(time.Minute); free.After(next) {
			next = free
		}
	}
	if next.After(now) {
		return next.Sub(now), nil
	}
	l.recent = append(l.recent, now)
	l.today++
	return 0, nil
}

// turnDay starts a new count when now belongs to a later day than the count.
func (l *Limiter) turnDay(now time.Time) {
	t := now.In(l.zone)
	midnight := time.Date(t.Year(), t.Month(), t.Day(), 0, 0, 0, 0, l.zone)
	if !midnight.Equal(l.day) {
		l.day, l.today, l.used = midnight, 0, false
	}
}

// holdOff keeps every request back for d from now, as a 429 asks.
func (l *Limiter) holdOff(d time.Duration) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if until := l.now().Add(d); until.After(l.notBefore) {
		l.notBefore = until
	}
}

// dailyQuotaUsed records that the API refused a request for the daily quota:
// until the day ends, Wait returns ErrDailyLimit whatever its own count says.
func (l *Limiter) dailyQuotaUsed() {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.turnDay(l.now())
	l.used = true
}

// pacificTime is where Google's quota day begins. It is loaded on first use, not
// when the package is initialised, because the zone may come from time/tzdata,
// which a binary like Radix links in for hosts without zoneinfo. Without either,
// Pacific Standard Time is an hour off in summer, and the API's 429 covers that.
var pacificTime = sync.OnceValue(func() *time.Location {
	if loc, err := time.LoadLocation("America/Los_Angeles"); err == nil {
		return loc
	}
	return time.FixedZone("PST", -8*60*60)
})

// sleepContext waits d, or less when the context ends first.
func sleepContext(ctx context.Context, d time.Duration) error {
	timer := time.NewTimer(d)
	defer timer.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

// quotaError is what a 429 says about the quota it ran into.
type quotaError struct {
	message    string        // error.message of the answer
	retryDelay time.Duration // how long to wait, zero when the answer does not say
	daily      bool          // a per-day quota is used up
}

// readQuotaError reads a 429 of the Gemini API. The body is a google.rpc.Status:
// its details carry a google.rpc.RetryInfo with the delay to wait and a
// google.rpc.QuotaFailure naming the quotas used up, such as
// "GenerateRequestsPerDayPerProjectPerModel-FreeTier". A Retry-After header,
// in seconds or as a date, stands in for a missing RetryInfo.
func readQuotaError(h http.Header, body []byte, now time.Time) quotaError {
	var status struct {
		Error struct {
			Message string `json:"message"`
			Details []struct {
				Type       string `json:"@type"`
				RetryDelay string `json:"retryDelay"`
				Violations []struct {
					QuotaID     string `json:"quotaId"`
					Description string `json:"description"`
				} `json:"violations"`
			} `json:"details"`
		} `json:"error"`
	}
	var q quotaError
	if json.Unmarshal(body, &status) == nil {
		q.message = status.Error.Message
		for _, d := range status.Error.Details {
			switch {
			case strings.HasSuffix(d.Type, "google.rpc.RetryInfo"):
				// A protobuf Duration in JSON: seconds with an "s", such as "33s" or "1.5s".
				if delay, err := time.ParseDuration(d.RetryDelay); err == nil && delay > 0 {
					q.retryDelay = delay
				}
			case strings.HasSuffix(d.Type, "google.rpc.QuotaFailure"):
				for _, v := range d.Violations {
					if perDay(v.QuotaID) || perDay(v.Description) {
						q.daily = true
					}
				}
			}
		}
	}
	if q.message == "" {
		q.message = strings.TrimSpace(string(body))
	}
	if q.retryDelay == 0 {
		q.retryDelay = retryAfter(h.Get("Retry-After"), now)
	}
	return q
}

// perDay tells a quota of a day ("…PerDay…", "requests per day") from one of a
// minute.
func perDay(s string) bool {
	s = strings.ToLower(s)
	return strings.Contains(s, "perday") || strings.Contains(s, "per day")
}

// retryAfter reads a Retry-After header: a number of seconds or an HTTP date.
func retryAfter(v string, now time.Time) time.Duration {
	v = strings.TrimSpace(v)
	if v == "" {
		return 0
	}
	if s, err := strconv.Atoi(v); err == nil {
		if s > 0 {
			return time.Duration(s) * time.Second
		}
		return 0
	}
	if t, err := http.ParseTime(v); err == nil && t.After(now) {
		return t.Sub(now)
	}
	return 0
}
