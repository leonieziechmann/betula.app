package crawl

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

func TestDueRules(t *testing.T) {
	period := 30 * 24 * time.Hour
	now := time.Date(2026, 10, 15, 2, 0, 0, 0, time.UTC)
	for i := 0; i < 100; i++ {
		key := fmt.Sprint(10000 + i)
		if Due(key, now.Add(-period/2+time.Hour), now, period) {
			t.Errorf("%s is due within half a period of its last read", key)
		}
		if !Due(key, now.Add(-period*3/2), now, period) {
			t.Errorf("%s is not due one and a half periods after its last read", key)
		}
	}
	if !Due("x", time.Time{}, now, period) || !Due("x", now, now, 0) {
		t.Errorf("a page never read, or without a period, must be due")
	}
}

// An archive read in the same three nights, as the whole archive was before the monthly
// rhythm, must not come due in the same three nights a month later: every page has a
// night of its own, and the nights fill evenly.
func TestDueSpreadsAPeriodEvenly(t *testing.T) {
	const pages = 3000
	period := 30 * 24 * time.Hour
	start := time.Date(2026, 9, 24, 2, 0, 0, 0, time.UTC)
	fetched := make([]time.Time, pages)
	for i := range fetched {
		fetched[i] = start.Add(-time.Duration(i%3) * 24 * time.Hour)
	}

	reads := make([]int, pages)
	busiest := 0
	for day := 1; day <= 120; day++ {
		now := start.Add(time.Duration(day) * 24 * time.Hour)
		n := 0
		for i := range fetched {
			if Due(fmt.Sprint(20000+i), fetched[i], now, period) {
				fetched[i] = now
				reads[i]++
				n++
			}
		}
		if n > busiest {
			busiest = n
		}
	}
	// 3,000 pages a month are 100 a night; a night of its own for every page keeps the
	// busiest night close to that, from the first month on.
	if busiest > 160 {
		t.Errorf("the busiest night reads %d pages, want about 100", busiest)
	}
	for i, r := range reads {
		if r < 3 || r > 5 {
			t.Fatalf("page %d was read %d times in 120 days, want about four", i, r)
		}
	}
}

// With Spread, pages read in one night come due on days of their own: a period later,
// those whose time came in the second half of the period, about half of them (of these
// 60 keys, every half period holds the times of 24 to 36). Run and the Fetcher, which the
// tree walk uses, both honour it; without it, every page a period old is due.
func TestSpreadInRunAndFetcher(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, "new") }))
	defer srv.Close()
	ctx := context.Background()
	period := 30 * 24 * time.Hour
	archive := func() (*catalogdb.DB, []Job) {
		db := openTestDB(t)
		var jobs []Job
		for i := 0; i < 60; i++ {
			key := fmt.Sprint(20000 + i)
			if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceQISTree, Key: key, URL: "u", HTTPStatus: 200, Body: []byte("old"), FetchedAt: time.Now().Add(-period)}); err != nil {
				t.Fatalf("PutPage: %v", err)
			}
			jobs = append(jobs, Job{Source: catalogdb.SourceQISTree, Key: key, URL: srv.URL + "/" + key})
		}
		return db, jobs
	}

	db, jobs := archive()
	stats, err := Run(ctx, db, jobs, Options{MaxAge: period, Spread: true})
	if err != nil || stats.Fetched < 20 || stats.Fetched > 40 || stats.Fetched+stats.Skipped != len(jobs) {
		t.Errorf("Run with Spread: %+v (%v), want about half fetched", stats, err)
	}

	db, jobs = archive()
	fetcher := NewFetcher(db, Options{MaxAge: period, Spread: true})
	for _, job := range jobs {
		if _, err := fetcher.Get(ctx, job); err != nil {
			t.Fatalf("Get: %v", err)
		}
	}
	if s := fetcher.Stats(); s.Fetched < 20 || s.Fetched > 40 || s.Fetched+s.Skipped != len(jobs) {
		t.Errorf("Fetcher with Spread: %+v, want about half fetched", s)
	}

	db, jobs = archive()
	if stats, err := Run(ctx, db, jobs, Options{MaxAge: period}); err != nil || stats.Fetched != len(jobs) {
		t.Errorf("Run without Spread: %+v (%v), want every page", stats, err)
	}
}
