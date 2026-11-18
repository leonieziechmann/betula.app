// Package server is Xylem's service (docs/xylem/protocol.md): it keeps the push subscriptions
// of the browsers that asked for notifications, with the topics each listens to, and writes an
// announcement to every subscription of its topic when its time has come, sealed for each
// browser, through the browser's own push service. It knows no profile and no plan: what it
// says, it says to everybody who listens.
//
// Its numbers go to a file (the stats file, written every 15 s), not to an endpoint, as
// Phloem's do.
package server

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/metrics"
	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// DefaultMaxSubscriptions is how many subscriptions Xylem holds before it takes no new one: far
// above what a university's students come to, far below what would fill a small disk (a
// subscription is some 500 bytes).
const DefaultMaxSubscriptions int64 = 200_000

// DefaultStatsInterval is how often the stats file is written.
const DefaultStatsInterval = 15 * time.Second

// DefaultLang is the language an announcement must be written in, and the one a browser of a
// language without a text of its own reads.
const DefaultLang = "de"

// Limits are Xylem's bounds. A zero field takes its value of DefaultLimits, which are the
// contract's; the tests make them small.
type Limits struct {
	MaxEndpoint int // the bytes of a subscription's endpoint
	MaxTopics   int // topics of one subscription

	NewPerAddr    int           // new subscriptions of one address in NewWindow
	NewTotal      int           // new subscriptions in all in NewWindow
	NewWindow     time.Duration //
	WritesPerAddr int           // subscriptions one address may put or delete in an hour

	MaxTitle     int           // the characters of an announcement's title
	MaxBody      int           // ... of its body
	MaxLink      int           // the bytes of a link
	MaxLanguages int           // the languages of one announcement
	MinLife      time.Duration // the shortest an announcement may be told for
	MaxLife      time.Duration // the longest: no push service keeps a message longer
	DefaultLife  time.Duration // unless it says otherwise

	Senders     int             // requests to push services at a time
	BatchSize   int             // subscriptions an announcement is written to in one step
	SendTimeout time.Duration   // a request to a push service ends within it
	Backoff     []time.Duration // the waits before another attempt, one after the other; then it is given up
	MaxWait     time.Duration   // the longest a push service's Retry-After is followed
	TokenLife   time.Duration   // how long a VAPID token is signed for
	TickEvery   time.Duration   // how often what is due is looked for

	FeedEvery      time.Duration // how often the feed is read
	FeedTimeout    time.Duration // a read of the feed ends within it
	MaxFeedBytes   int           // the bytes of the feed
	MaxFeedEntries int           // the announcements the feed may name
	FeedPerDay     int           // announcements of the feed's told within a day of each other
	DayFrom        int           // the hours of the day, in the machine's time zone: an announcement
	DayUntil       int           // of the feed's without a time of its own is told within them
	SweepEvery     time.Duration // finished announcements go this often ...
	KeepFinished   time.Duration // ... once they are finished for this long
}

// DefaultLimits are the contract's limits (docs/xylem/protocol.md) and Xylem's own beside them
// (docs/xylem/xylem.md, „Limits").
var DefaultLimits = Limits{
	MaxEndpoint: 2048,
	MaxTopics:   16,

	NewPerAddr:    120, // a lecture hall behind one address subscribes within an hour
	NewTotal:      5000,
	NewWindow:     time.Hour,
	WritesPerAddr: 1200,

	MaxTitle:     120,
	MaxBody:      600,
	MaxLink:      512,
	MaxLanguages: 8,
	MinLife:      time.Minute,
	MaxLife:      28 * 24 * time.Hour,
	DefaultLife:  24 * time.Hour,

	Senders:     8,
	BatchSize:   100,
	SendTimeout: 15 * time.Second,
	Backoff:     []time.Duration{time.Minute, 5 * time.Minute, 15 * time.Minute, time.Hour, 3 * time.Hour, 6 * time.Hour, 12 * time.Hour},
	MaxWait:     time.Hour,
	TokenLife:   12 * time.Hour,
	TickEvery:   5 * time.Second,

	FeedEvery:      10 * time.Minute,
	FeedTimeout:    20 * time.Second,
	MaxFeedBytes:   256 << 10,
	MaxFeedEntries: 64,
	FeedPerDay:     4,
	DayFrom:        8,
	DayUntil:       20,
	SweepEvery:     time.Hour,
	KeepFinished:   395 * 24 * time.Hour,
}

func (l Limits) withDefaults() Limits {
	d := DefaultLimits
	pick := func(v *int, def int) {
		if *v <= 0 {
			*v = def
		}
	}
	pickD := func(v *time.Duration, def time.Duration) {
		if *v <= 0 {
			*v = def
		}
	}
	pick(&l.MaxEndpoint, d.MaxEndpoint)
	pick(&l.MaxTopics, d.MaxTopics)
	pick(&l.NewPerAddr, d.NewPerAddr)
	pick(&l.NewTotal, d.NewTotal)
	pickD(&l.NewWindow, d.NewWindow)
	pick(&l.WritesPerAddr, d.WritesPerAddr)
	pick(&l.MaxTitle, d.MaxTitle)
	pick(&l.MaxBody, d.MaxBody)
	pick(&l.MaxLink, d.MaxLink)
	pick(&l.MaxLanguages, d.MaxLanguages)
	pickD(&l.MinLife, d.MinLife)
	pickD(&l.MaxLife, d.MaxLife)
	pickD(&l.DefaultLife, d.DefaultLife)
	pick(&l.Senders, d.Senders)
	pick(&l.BatchSize, d.BatchSize)
	pickD(&l.SendTimeout, d.SendTimeout)
	if l.Backoff == nil {
		l.Backoff = d.Backoff
	}
	pickD(&l.MaxWait, d.MaxWait)
	pickD(&l.TokenLife, d.TokenLife)
	pickD(&l.TickEvery, d.TickEvery)
	pickD(&l.FeedEvery, d.FeedEvery)
	pickD(&l.FeedTimeout, d.FeedTimeout)
	pick(&l.MaxFeedBytes, d.MaxFeedBytes)
	pick(&l.MaxFeedEntries, d.MaxFeedEntries)
	pick(&l.FeedPerDay, d.FeedPerDay)
	if l.DayFrom <= 0 && l.DayUntil <= 0 {
		l.DayFrom, l.DayUntil = d.DayFrom, d.DayUntil
	}
	pickD(&l.SweepEvery, d.SweepEvery)
	pickD(&l.KeepFinished, d.KeepFinished)
	return l
}

// Options configure a Server.
type Options struct {
	Store *store.Store
	// Key is Xylem's VAPID key (the store keeps it).
	Key *webpush.Key

	// Site is the site's origin (https://betula.app): what a notification's link opens at.
	Site string
	// Contact is where a push service's operators reach Xylem's: a mailto: or https: address,
	// the subject of every VAPID token. "": Site.
	Contact string
	// Lang is the default language; "": DefaultLang.
	Lang string

	// Origins are the browser origins taken, as scheme://host[:port], exactly. Empty: the
	// origin whose host is the request's own (the site's own origin behind Traefik).
	Origins []string

	// Feed is the address of the site's feed of announcements (docs/xylem/protocol.md section
	// 7): what Betula announces by itself. "": none, and only a person announces.
	Feed string
	// FeedClient reads the feed; nil: Xylem's own (no proxy, no redirect followed).
	FeedClient *http.Client
	// Location is the time zone of the hours of the day (Limits.DayFrom); nil: the machine's.
	Location *time.Location

	// Hosts are hosts taken as push services beside the ones Xylem knows (Services): a name
	// exactly, or with a leading dot every host under it.
	Hosts []string

	// MaxSubscriptions is how many subscriptions Xylem holds; at it, no new one (507). 0:
	// DefaultMaxSubscriptions.
	MaxSubscriptions int64

	// StatsDir is where xylem.prom is written; "": nowhere.
	StatsDir string
	// StatsInterval: 0 is DefaultStatsInterval.
	StatsInterval time.Duration
	// Stack, when set, is the label stack="…" of every series of the stats file.
	Stack string

	Version, Build string

	Limits Limits

	// Registry holds the metrics; nil: a registry of the server's own (the tests).
	Registry *metrics.Registry

	// Client sends the requests to the push services; nil: Xylem's own, which reaches public
	// addresses on port 443 alone, through no proxy, and follows no redirect.
	Client *http.Client
	// Insecure takes endpoints over plain HTTP and on any port as well: for the checks, which
	// play the push service on a loopback address. Never set by serve.
	Insecure bool

	// Now is the clock; nil: time.Now.
	Now func() time.Time
}

// LogProblems counts the log's WARN and ERROR records by level and event; cmd/xylem hands it to
// oplog. It lives in metrics.Default, which serve writes the stats file from.
var LogProblems = metrics.Default.NewCounter("xylem_log_problems_total",
	"Log records at WARN or ERROR, by level and event.", "level", "event")

// Server is one Xylem. Close ends it.
type Server struct {
	st       *store.Store
	key      *webpush.Key
	opt      Options
	lim      Limits
	now      func() time.Time
	site     string
	contact  string
	lang     string
	max      int64
	origins  map[string]bool
	services []Service
	client   *http.Client
	feed     string // the feed's address; "": none
	feedHTTP *http.Client
	loc      *time.Location
	reg      *metrics.Registry
	m        meters
	started  time.Time

	ctx    context.Context // ends with Close, after the loops: the sweep's store calls
	cancel context.CancelFunc
	stop   chan struct{} // ends the loops
	wake   chan struct{} // an announcement was made: look for what is due now
	up     atomic.Bool
	closed sync.Once

	writes *limiter // subscriptions put or deleted, by address
	news   *limiter // new subscriptions, by address
	newAll *limiter // new subscriptions in all (one key)

	workMu sync.Mutex // one round of the sender at a time
	feedMu sync.Mutex // one read of the feed at a time
	feedSt feedState  // guarded by mu

	tokenMu sync.Mutex
	tokens  map[string]token // VAPID tokens by the origin of the push service

	mu          sync.Mutex
	unknown     map[string]int64     // hosts of endpoints refused as no push service's, with how often
	complained  map[string]time.Time // when a push service's refusal of Xylem's token was last logged
	full        bool                 // logged that no new subscription is taken
	statsFailed bool

	wg sync.WaitGroup // the loops
}

var stackName = regexp.MustCompile(`^[A-Za-z0-9_.-]*$`)

// New starts a server on opt.Store: its loops run until Close.
func New(opt Options) (*Server, error) {
	if opt.Store == nil {
		return nil, errors.New("no store")
	}
	if opt.Key == nil {
		return nil, errors.New("no VAPID key")
	}
	if !stackName.MatchString(opt.Stack) {
		return nil, fmt.Errorf("invalid stack name %q", opt.Stack)
	}
	site, err := normalizeOrigin(opt.Site)
	if err != nil {
		return nil, fmt.Errorf("site: %w", err)
	}
	if !opt.Insecure && !strings.HasPrefix(site, "https://") {
		return nil, fmt.Errorf("site %q: a notification opens an https address", opt.Site)
	}
	contact := strings.TrimSpace(opt.Contact)
	if contact == "" {
		contact = site
	}
	if u, err := url.Parse(contact); err != nil || (u.Scheme != "mailto" && u.Scheme != "https") || strings.ContainsAny(contact, " \t\r\n") {
		return nil, fmt.Errorf("contact %q: a mailto: or https: address", opt.Contact)
	}
	origins := map[string]bool{}
	for _, raw := range opt.Origins {
		o, err := normalizeOrigin(raw)
		if err != nil {
			return nil, err
		}
		origins[o] = true
	}
	services, err := withHosts(Services, opt.Hosts)
	if err != nil {
		return nil, err
	}
	feed, err := checkFeed(opt.Feed)
	if err != nil {
		return nil, err
	}
	if opt.Lang == "" {
		opt.Lang = DefaultLang
	}
	if !langName.MatchString(opt.Lang) {
		return nil, fmt.Errorf("invalid language %q", opt.Lang)
	}
	if opt.MaxSubscriptions <= 0 {
		opt.MaxSubscriptions = DefaultMaxSubscriptions
	}
	if opt.StatsInterval <= 0 {
		opt.StatsInterval = DefaultStatsInterval
	}
	if opt.Now == nil {
		opt.Now = time.Now
	}
	reg := opt.Registry
	if reg == nil {
		reg = metrics.NewRegistry()
	}
	lim := opt.Limits.withDefaults()
	if lim.DayFrom < 0 || lim.DayFrom >= lim.DayUntil || lim.DayUntil > 24 {
		return nil, fmt.Errorf("the hours of the day %d to %d are none", lim.DayFrom, lim.DayUntil)
	}
	client := opt.Client
	if client == nil {
		client = pushClient(lim.SendTimeout, lim.Senders)
	}
	feedHTTP := opt.FeedClient
	if feedHTTP == nil {
		feedHTTP = feedClient(lim.FeedTimeout)
	}
	loc := opt.Location
	if loc == nil {
		loc = time.Local
	}
	ctx, cancel := context.WithCancel(context.Background())
	s := &Server{
		st:         opt.Store,
		key:        opt.Key,
		opt:        opt,
		lim:        lim,
		now:        opt.Now,
		site:       site,
		contact:    contact,
		lang:       opt.Lang,
		max:        opt.MaxSubscriptions,
		origins:    origins,
		services:   services,
		client:     client,
		feed:       feed,
		feedHTTP:   feedHTTP,
		loc:        loc,
		feedSt:     feedState{warned: map[string]bool{}},
		reg:        reg,
		started:    time.Now(),
		ctx:        ctx,
		cancel:     cancel,
		stop:       make(chan struct{}),
		wake:       make(chan struct{}, 1),
		writes:     perWindow(lim.WritesPerAddr, time.Hour),
		news:       perWindow(lim.NewPerAddr, lim.NewWindow),
		newAll:     perWindow(lim.NewTotal, lim.NewWindow),
		tokens:     map[string]token{},
		unknown:    map[string]int64{},
		complained: map[string]time.Time{},
	}
	s.up.Store(true)
	s.declare()
	s.wg.Add(3)
	if s.feed != "" {
		s.wg.Add(1)
		go s.feedLoop()
	}
	go s.sendLoop()
	go s.sweepLoop()
	go s.statsLoop()
	return s, nil
}

// normalizeOrigin checks an origin: scheme://host[:port], lower case.
func normalizeOrigin(raw string) (string, error) {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" ||
		(u.Path != "" && u.Path != "/") || u.RawQuery != "" || u.Fragment != "" || u.User != nil {
		return "", fmt.Errorf("invalid origin %q: expected scheme://host[:port]", raw)
	}
	return strings.ToLower(u.Scheme + "://" + u.Host), nil
}

// Close ends the server: the loops stop, the sender once the batch it is in is written and
// recorded (a request to a push service ends within Limits.SendTimeout), and the stats file
// says xylem_up 0.
func (s *Server) Close() error {
	s.closed.Do(func() {
		s.up.Store(false)
		close(s.stop)
		s.wg.Wait()
		s.cancel()
		if s.opt.StatsDir != "" {
			_ = s.writeStats()
		}
	})
	return nil
}

// sweepLoop forgets the announcements finished long ago, and the limiters' idle keys.
// Log events: sweep.done, store.failed (ERROR).
func (s *Server) sweepLoop() {
	defer s.wg.Done()
	t := time.NewTicker(s.lim.SweepEvery)
	defer t.Stop()
	for {
		select {
		case <-s.stop:
			return
		case <-t.C:
		}
		now := s.now()
		for _, l := range []*limiter{s.writes, s.news, s.newAll} {
			l.prune(now)
		}
		n, err := s.st.Sweep(s.ctx, now.Add(-s.lim.KeepFinished))
		if err != nil {
			s.storeFailed("sweep", err)
			continue
		}
		if n > 0 {
			oplog.For("store").Info("old announcements deleted", "event", "sweep.done", "announcements", n)
		}
	}
}

// statsLoop writes the stats file every interval.
// Log events: stats.failed (WARN), stats.recovered.
func (s *Server) statsLoop() {
	defer s.wg.Done()
	if s.opt.StatsDir == "" {
		return
	}
	write := func() {
		err := s.writeStats()
		s.mu.Lock()
		failed := s.statsFailed
		s.statsFailed = err != nil
		s.mu.Unlock()
		switch {
		case err != nil && !failed:
			oplog.For("stats").Warn("cannot write the stats file", "event", "stats.failed", "dir", s.opt.StatsDir, oplog.Err(err))
		case err == nil && failed:
			oplog.For("stats").Info("the stats file is written again", "event", "stats.recovered", "dir", s.opt.StatsDir)
		}
	}
	write()
	t := time.NewTicker(s.opt.StatsInterval)
	defer t.Stop()
	for {
		select {
		case <-s.stop:
			return
		case <-t.C:
			write()
		}
	}
}

// storeFailed logs a store call that failed, unless the server is closing (its context ended).
func (s *Server) storeFailed(op string, err error) {
	if s.ctx.Err() != nil || errors.Is(err, store.ErrClosed) {
		return
	}
	oplog.For("store").Error("a store call failed", "event", "store.failed", "op", op, oplog.Err(err))
}
