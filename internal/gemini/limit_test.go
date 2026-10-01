package gemini

import (
	"context"
	"errors"
	"net/http"
	"sync"
	"testing"
	"time"
	_ "time/tzdata" // the quota day's zone, also on hosts without zoneinfo
)

// fakeClock is a clock whose sleeps pass at once and move its time on.
type fakeClock struct {
	mu    sync.Mutex
	t     time.Time
	slept []time.Duration
}

func (f *fakeClock) now() time.Time {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.t
}

func (f *fakeClock) sleep(ctx context.Context, d time.Duration) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	f.mu.Lock()
	defer f.mu.Unlock()
	f.t = f.t.Add(d)
	f.slept = append(f.slept, d)
	return nil
}

func (f *fakeClock) advance(d time.Duration) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.t = f.t.Add(d)
}

func (f *fakeClock) total() time.Duration {
	f.mu.Lock()
	defer f.mu.Unlock()
	var sum time.Duration
	for _, d := range f.slept {
		sum += d
	}
	return sum
}

// testLimiter returns a limiter on a fake clock that starts at 10:00 Pacific time.
func testLimiter(t *testing.T, perMinute, perDay int) (*Limiter, *fakeClock) {
	t.Helper()
	l := NewLimiter(perMinute, perDay)
	clock := &fakeClock{t: time.Date(2026, 10, 1, 10, 0, 0, 0, l.zone)}
	l.now, l.sleep = clock.now, clock.sleep
	return l, clock
}

func TestLimiterDefaults(t *testing.T) {
	l := NewLimiter(0, -1)
	if l.perMinute != DefaultRequestsPerMinute || l.perDay != DefaultRequestsPerDay {
		t.Fatalf("defaults: %d a minute, %d a day", l.perMinute, l.perDay)
	}
	if l.zone.String() != "America/Los_Angeles" {
		t.Errorf("the quota day begins in %s, want America/Los_Angeles", l.zone)
	}
}

func TestLimiterPacesRequestsPerMinute(t *testing.T) {
	l, clock := testLimiter(t, 3, 100)
	start := clock.now()
	var sent []time.Duration
	for range 10 {
		if err := l.Wait(context.Background()); err != nil {
			t.Fatal(err)
		}
		sent = append(sent, clock.now().Sub(start))
		clock.advance(time.Second) // the request itself
	}
	// Three at once, then each slot frees a minute after the request that held it.
	want := []time.Duration{0, 1, 2, 60, 61, 62, 120, 121, 122, 180}
	for i := range want {
		if sent[i] != want[i]*time.Second {
			t.Fatalf("request %d went out after %s, want %ds (all: %v)", i+1, sent[i], want[i], sent)
		}
	}
	for i := range sent {
		in := 0
		for j := i; j < len(sent) && sent[j]-sent[i] < time.Minute; j++ {
			in++
		}
		if in > 3 {
			t.Fatalf("%d requests within a minute from %s", in, sent[i])
		}
	}
}

func TestLimiterStopsAtTheDailyCapUntilMidnightPacific(t *testing.T) {
	l, clock := testLimiter(t, 100, 5)
	for i := range 5 {
		if err := l.Wait(context.Background()); err != nil {
			t.Fatalf("request %d: %v", i+1, err)
		}
	}
	for range 2 {
		if err := l.Wait(context.Background()); !errors.Is(err, ErrDailyLimit) {
			t.Fatalf("sixth request: %v, want ErrDailyLimit", err)
		}
	}
	if len(clock.slept) != 0 {
		t.Fatalf("waited %v for the next day instead of returning", clock.slept)
	}

	// 23:59 Pacific is still the same day, even though it is the next day in UTC.
	clock.advance(13*time.Hour + 59*time.Minute)
	if err := l.Wait(context.Background()); !errors.Is(err, ErrDailyLimit) {
		t.Fatalf("at 23:59 Pacific: %v, want ErrDailyLimit", err)
	}
	clock.advance(time.Minute)
	if err := l.Wait(context.Background()); err != nil {
		t.Fatalf("after midnight Pacific: %v", err)
	}
}

func TestLimiterHoldsBackForA429(t *testing.T) {
	l, clock := testLimiter(t, 100, 100)
	l.holdOff(37 * time.Second)
	l.holdOff(5 * time.Second) // a shorter delay does not shorten the longer one
	if err := l.Wait(context.Background()); err != nil {
		t.Fatal(err)
	}
	if got := clock.total(); got != 37*time.Second {
		t.Fatalf("waited %s, want 37s", got)
	}
}

func TestLimiterTakesTheAPIsWordForTheDailyQuota(t *testing.T) {
	l, clock := testLimiter(t, 100, 100)
	l.dailyQuotaUsed()
	if err := l.Wait(context.Background()); !errors.Is(err, ErrDailyLimit) {
		t.Fatalf("after the API's daily 429: %v, want ErrDailyLimit", err)
	}
	clock.advance(14 * time.Hour)
	if err := l.Wait(context.Background()); err != nil {
		t.Fatalf("the next day: %v", err)
	}
}

func TestLimiterIsSafeForConcurrentUse(t *testing.T) {
	l, _ := testLimiter(t, 1000, 40)
	var wg sync.WaitGroup
	var mu sync.Mutex
	sent, refused := 0, 0
	for range 100 {
		wg.Go(func() {
			err := l.Wait(context.Background())
			mu.Lock()
			defer mu.Unlock()
			switch {
			case err == nil:
				sent++
			case errors.Is(err, ErrDailyLimit):
				refused++
			default:
				t.Error(err)
			}
		})
	}
	wg.Wait()
	if sent != 40 || refused != 60 {
		t.Fatalf("%d sent and %d refused, want 40 and 60", sent, refused)
	}
}

func TestLimiterWaitEndsWithTheContext(t *testing.T) {
	l, _ := testLimiter(t, 1, 100)
	if err := l.Wait(context.Background()); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := l.Wait(ctx); !errors.Is(err, context.Canceled) {
		t.Fatalf("got %v, want context.Canceled", err)
	}
}

func TestReadQuotaError(t *testing.T) {
	now := time.Date(2026, 10, 1, 12, 0, 0, 0, time.UTC)
	for _, tc := range []struct {
		name   string
		header http.Header
		body   string
		delay  time.Duration
		daily  bool
	}{
		{
			name: "per-minute quota with RetryInfo",
			body: `{"error":{"code":429,"message":"You exceeded your current quota.","status":"RESOURCE_EXHAUSTED","details":[
				{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaMetric":"generativelanguage.googleapis.com/generate_content_free_tier_requests","quotaId":"GenerateRequestsPerMinutePerProjectPerModel-FreeTier","quotaValue":"10"}]},
				{"@type":"type.googleapis.com/google.rpc.Help","links":[{"description":"Learn more","url":"https://ai.google.dev/gemini-api/docs/rate-limits"}]},
				{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"33.5s"}]}}`,
			delay: 33500 * time.Millisecond,
		},
		{
			name: "per-day quota",
			body: `{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[
				{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier"}]},
				{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"4s"}]}}`,
			delay: 4 * time.Second,
			daily: true,
		},
		{
			name:  "per-day quota by its description",
			body:  `{"error":{"code":429,"details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"description":"Quota exceeded for requests per day."}]}]}}`,
			daily: true,
		},
		{
			name:   "Retry-After in seconds",
			header: http.Header{"Retry-After": {"20"}},
			body:   `{"error":{"code":429}}`,
			delay:  20 * time.Second,
		},
		{
			name:   "Retry-After as a date",
			header: http.Header{"Retry-After": {now.Add(90 * time.Second).Format(http.TimeFormat)}},
			body:   `not json`,
			delay:  90 * time.Second,
		},
		{
			name:   "RetryInfo before Retry-After",
			header: http.Header{"Retry-After": {"20"}},
			body:   `{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"7s"}]}}`,
			delay:  7 * time.Second,
		},
		{
			name: "nothing said",
			body: `{"error":{"code":429,"message":"Resource exhausted"}}`,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			h := tc.header
			if h == nil {
				h = http.Header{}
			}
			q := readQuotaError(h, []byte(tc.body), now)
			if q.retryDelay != tc.delay || q.daily != tc.daily {
				t.Fatalf("got delay %s and daily %v, want %s and %v", q.retryDelay, q.daily, tc.delay, tc.daily)
			}
		})
	}
}
