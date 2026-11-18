package server

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/store"
)

// The feed: what Betula announces by itself (docs/xylem/protocol.md section 7). The site knows
// the catalog, and so when a semester's lectures begin and pause, when its exams are, and when
// the next semester's dates are there; it names these as announcements at one address, and
// Xylem reads that address every few minutes and keeps what it names as its own (store.SyncFeed):
// new ones wait for their time, ones that changed are changed while they wait, ones that are no
// longer named are withdrawn.
//
// Nothing the feed says is told at once without Xylem's own bounds: a link is a path of the
// site, a text has its length, at most Limits.FeedPerDay of the feed's are told within a day,
// and what has no time of its own is told in the hours of the day.

// feedDoc is the feed: the announcements the site names now, each as `xylem announce` takes one,
// with a key.
type feedDoc struct {
	Announcements []AnnounceRequest `json:"announcements"`
}

// FeedReport is what one read of the feed came to (`xylem feed`, POST /feed).
type FeedReport struct {
	// Unchanged: the feed is what it was at the last read (304); nothing was looked at.
	Unchanged bool `json:"unchanged,omitempty"`
	// Baseline: the first read of this feed, where what has no time of its own is skipped.
	Baseline bool `json:"baseline,omitempty"`
	// Entries is how many announcements the feed names.
	Entries int          `json:"entries"`
	Changes []FeedChange `json:"changes"`
}

// FeedChange is what a read did with one announcement of the feed.
type FeedChange struct {
	Key string `json:"key"`
	ID  int64  `json:"id,omitempty"`
	// What: made, revised, revived, withdrawn, skipped (see store.SyncFeed); late (its time is
	// over: not taken), invalid (not readable: Detail says why), capped.
	What   string     `json:"what"`
	At     *time.Time `json:"at,omitempty"`
	Detail string     `json:"detail,omitempty"`
}

// FeedStatus is the feed in `xylem status`.
type FeedStatus struct {
	URL     string     `json:"url"`
	Read    *time.Time `json:"read,omitempty"` // the last read that came through
	Entries int        `json:"entries"`
	Failing string     `json:"failing,omitempty"` // why the last read did not
}

// feedState is what the reads of the feed left, guarded by Server.mu.
type feedState struct {
	etag     string
	read     time.Time
	entries  int
	failing  string
	warned   map[string]bool // the entries already called invalid, by key and reason
	flooding time.Time       // when the cap was last logged
}

// checkFeed checks Options.Feed: an http or https address.
func checkFeed(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", nil
	}
	u, err := url.Parse(raw)
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" || u.User != nil || u.Fragment != "" {
		return "", fmt.Errorf("invalid feed %q: an http or https address", raw)
	}
	return u.String(), nil
}

// feedClient reads the feed: the address is the operator's own word (the site, by whatever way
// leads to it: its public address, or one inside the house), so no rule about addresses; no
// proxy, no redirect followed.
func feedClient(timeout time.Duration) *http.Client {
	return &http.Client{
		Timeout:       timeout,
		Transport:     &http.Transport{Proxy: nil, ResponseHeaderTimeout: timeout, IdleConnTimeout: time.Minute},
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}

// feedLoop reads the feed every Limits.FeedEvery, the first time at once.
func (s *Server) feedLoop() {
	defer s.wg.Done()
	t := time.NewTicker(s.lim.FeedEvery)
	defer t.Stop()
	for {
		_, _ = s.readFeed(context.Background())
		select {
		case <-s.stop:
			return
		case <-t.C:
		}
	}
}

// errNoFeed: `xylem feed` on a Xylem without one.
var errNoFeed = errors.New("no feed is set (--feed, XYLEM_FEED)")

// readFeed reads the feed once and brings Xylem's announcements in line with it.
// Log events: feed.read (something changed), feed.failed (WARN, until it reads again),
// feed.recovered, feed.invalid (WARN, once per entry and reason), feed.flood (ERROR, once an
// hour).
func (s *Server) readFeed(ctx context.Context) (FeedReport, error) {
	if s.feed == "" {
		return FeedReport{}, errNoFeed
	}
	s.feedMu.Lock()
	defer s.feedMu.Unlock()
	report, err := s.syncFeed(ctx)
	now := s.now()
	s.mu.Lock()
	was := s.feedSt.failing
	if err != nil {
		s.feedSt.failing = err.Error()
	} else {
		s.feedSt.failing, s.feedSt.read = "", now
		if !report.Unchanged {
			s.feedSt.entries = report.Entries
		}
	}
	s.mu.Unlock()
	switch {
	case err != nil:
		s.m.feedReads.Inc("failed")
		if was == "" {
			oplog.For("feed").Warn("the feed cannot be read: what waits is told all the same, nothing new is learned", "event", "feed.failed", "feed", s.feed, oplog.Err(err))
		}
		return report, err
	case was != "":
		oplog.For("feed").Info("the feed reads again", "event", "feed.recovered", "feed", s.feed)
	}
	if report.Unchanged {
		s.m.feedReads.Inc("unchanged")
		return report, nil
	}
	s.m.feedReads.Inc("read")
	counts := map[string]int{}
	for _, c := range report.Changes {
		counts[c.What]++
		s.m.feedChanges.Inc(c.What)
	}
	if n := counts[store.FeedCapped]; n > 0 {
		s.mu.Lock()
		log := time.Since(s.feedSt.flooding) >= time.Hour
		if log {
			s.feedSt.flooding = time.Now()
		}
		s.mu.Unlock()
		if log {
			oplog.For("feed").Error("the feed names more announcements within a day than Xylem tells: the rest is not taken", "event", "feed.flood",
				"feed", s.feed, "not_taken", n, "per_day", s.lim.FeedPerDay)
		}
	}
	if counts[store.FeedMade]+counts[store.FeedRevised]+counts[store.FeedRevived]+counts[store.FeedWithdrawn]+counts[store.FeedSkipped] > 0 {
		oplog.For("feed").Info("the feed was read", "event", "feed.read", "entries", report.Entries, "baseline", report.Baseline,
			"made", counts[store.FeedMade], "revised", counts[store.FeedRevised], "revived", counts[store.FeedRevived],
			"withdrawn", counts[store.FeedWithdrawn], "skipped", counts[store.FeedSkipped], "late", counts["late"], "invalid", counts["invalid"])
		select {
		case s.wake <- struct{}{}: // one of them may be due now
		default:
		}
	}
	return report, nil
}

// syncFeed fetches the feed and hands what it names to the store.
func (s *Server) syncFeed(ctx context.Context) (FeedReport, error) {
	ctx, cancel := context.WithTimeout(ctx, s.lim.FeedTimeout)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, s.feed, nil)
	if err != nil {
		return FeedReport{}, err
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", "Xylem/"+s.opt.Version)
	s.mu.Lock()
	etag := s.feedSt.etag
	s.mu.Unlock()
	if etag != "" {
		req.Header.Set("If-None-Match", etag)
	}
	resp, err := s.feedHTTP.Do(req)
	if err != nil {
		return FeedReport{}, err
	}
	defer resp.Body.Close()
	if resp.StatusCode == http.StatusNotModified && etag != "" {
		return FeedReport{Unchanged: true}, nil
	}
	if resp.StatusCode != http.StatusOK {
		return FeedReport{}, fmt.Errorf("the feed answered %d", resp.StatusCode)
	}
	data, err := io.ReadAll(io.LimitReader(resp.Body, int64(s.lim.MaxFeedBytes)+1))
	if err != nil {
		return FeedReport{}, err
	}
	if len(data) > s.lim.MaxFeedBytes {
		return FeedReport{}, fmt.Errorf("the feed is larger than %d bytes", s.lim.MaxFeedBytes)
	}
	var doc feedDoc
	if err := json.Unmarshal(data, &doc); err != nil {
		return FeedReport{}, fmt.Errorf("the feed is no JSON: %w", err)
	}
	if doc.Announcements == nil {
		// {"announcements": []} says that there is nothing to announce; anything else that
		// happens to be JSON says nothing, and must not withdraw what waits.
		return FeedReport{}, errors.New("the feed has no member \"announcements\"")
	}
	if len(doc.Announcements) > s.lim.MaxFeedEntries {
		return FeedReport{}, fmt.Errorf("the feed names %d announcements, %d at most", len(doc.Announcements), s.lim.MaxFeedEntries)
	}

	now := s.now()
	report := FeedReport{Entries: len(doc.Announcements), Changes: []FeedChange{}}
	var items []store.FeedItem
	keep := map[string]bool{}
	seen := map[string]bool{}
	for i, entry := range doc.Announcements {
		entry.Dry = false
		if !keyName.MatchString(entry.Key) || seen[entry.Key] {
			why := "no key: an announcement of the feed is known by its key"
			if seen[entry.Key] {
				why = "its key is named twice"
			}
			report.Changes = append(report.Changes, s.invalid(fmt.Sprintf("entry %d", i+1), entry.Key, why))
			continue
		}
		seen[entry.Key] = true
		timed := entry.At != nil
		if !timed {
			// So from now on: told in the hours of the day, not at the hour the catalog happened
			// to be built.
			at := s.nextDaytime(now)
			entry.At = &at
		}
		a, problem, over := s.check(entry, now)
		switch {
		case over:
			report.Changes = append(report.Changes, FeedChange{Key: entry.Key, What: "late", At: entry.At})
			continue
		case problem != "":
			keep[entry.Key] = true // what waited under this key stays as it was
			report.Changes = append(report.Changes, s.invalid(entry.Key, entry.Key, problem))
			continue
		}
		a.Timed = timed
		items = append(items, store.FeedItem{Announcement: a, Life: a.Until.Sub(a.SendAt)})
	}
	changes, baseline, err := s.st.SyncFeed(ctx, s.feed, items, keep, now, s.lim.FeedPerDay)
	if err != nil {
		s.storeFailed("feed", err)
		return FeedReport{}, err
	}
	report.Baseline = baseline
	for _, c := range changes {
		change := FeedChange{Key: c.Key, ID: c.ID, What: c.What}
		if !c.At.IsZero() {
			at := c.At
			change.At = &at
		}
		report.Changes = append(report.Changes, change)
	}
	// Only now: a read that failed half way is read again whole.
	s.mu.Lock()
	s.feedSt.etag = resp.Header.Get("ETag")
	s.mu.Unlock()
	return report, nil
}

// invalid is the change of an entry that is not readable, logged once per entry and reason.
func (s *Server) invalid(name, key, why string) FeedChange {
	s.mu.Lock()
	first := !s.feedSt.warned[name+"\x00"+why] && len(s.feedSt.warned) < 256
	if first {
		s.feedSt.warned[name+"\x00"+why] = true
	}
	s.mu.Unlock()
	if first {
		oplog.For("feed").Warn("an announcement of the feed is not taken", "event", "feed.invalid", "entry", name, "why", why)
	}
	return FeedChange{Key: key, What: "invalid", Detail: why}
}

// nextDaytime is now within the hours of the day (Limits.DayFrom to DayUntil, in the time zone
// of the machine), else the next time they begin.
func (s *Server) nextDaytime(now time.Time) time.Time {
	local := now.In(s.loc)
	begins := time.Date(local.Year(), local.Month(), local.Day(), s.lim.DayFrom, 0, 0, 0, s.loc)
	switch hour := local.Hour(); {
	case hour < s.lim.DayFrom:
		return begins
	case hour >= s.lim.DayUntil:
		return time.Date(local.Year(), local.Month(), local.Day()+1, s.lim.DayFrom, 0, 0, 0, s.loc)
	}
	return now
}

// feedStatus is the feed for `xylem status`; nil without one.
func (s *Server) feedStatus() *FeedStatus {
	if s.feed == "" {
		return nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	st := &FeedStatus{URL: s.feed, Entries: s.feedSt.entries, Failing: s.feedSt.failing}
	if !s.feedSt.read.IsZero() {
		read := s.feedSt.read
		st.Read = &read
	}
	return st
}

// readFeedNow is POST /feed: one read now, and what it came to.
func (s *Server) readFeedNow(w http.ResponseWriter, r *http.Request) {
	report, err := s.readFeed(r.Context())
	switch {
	case errors.Is(err, errNoFeed):
		writeJSON(w, http.StatusConflict, map[string]string{"error": "no_feed", "detail": err.Error()})
	case err != nil:
		writeJSON(w, http.StatusBadGateway, map[string]string{"error": "feed", "detail": err.Error()})
	default:
		writeJSON(w, http.StatusOK, report)
	}
}
