package server

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/store"
)

// site plays the site's feed: it names what the test says.
type site struct {
	srv *httptest.Server

	mu     sync.Mutex
	body   string
	status int
	etag   string
	hits   int
}

func newSite(t *testing.T) *site {
	f := &site{status: http.StatusOK}
	f.srv = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		f.mu.Lock()
		defer f.mu.Unlock()
		f.hits++
		if f.etag != "" {
			w.Header().Set("ETag", f.etag)
			if r.Header.Get("If-None-Match") == f.etag {
				w.WriteHeader(http.StatusNotModified)
				return
			}
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(f.status)
		_, _ = w.Write([]byte(f.body))
	}))
	t.Cleanup(f.srv.Close)
	f.name()
	return f
}

// name sets what the feed names.
func (f *site) name(entries ...AnnounceRequest) {
	if entries == nil {
		entries = []AnnounceRequest{}
	}
	data, _ := json.Marshal(map[string]any{"announcements": entries})
	f.mu.Lock()
	f.body, f.status = string(data), http.StatusOK
	f.mu.Unlock()
}

// say sets the feed's answer as it is.
func (f *site) say(status int, body string) {
	f.mu.Lock()
	f.body, f.status = body, status
	f.mu.Unlock()
}

// berlin is where Betula's hours of the day are.
var berlin = func() *time.Location {
	loc, err := time.LoadLocation("Europe/Berlin")
	if err != nil {
		panic(err)
	}
	return loc
}()

// fed is a Xylem with a feed, which it reads when the test says so (the loop that reads it by
// itself never started: the feed is set after New).
func fed(t *testing.T, change ...func(*Options)) (*world, *site) {
	t.Helper()
	f := newSite(t)
	w := newWorld(t, append([]func(*Options){func(o *Options) { o.Location = berlin }}, change...)...)
	w.srv.feed = f.srv.URL + "/api/announcements"
	return w, f
}

// read reads the feed and lets the sender do what is due; it returns what the read did, as
// "what key" lines.
func (w *world) read() []string {
	w.t.Helper()
	report, err := w.srv.readFeed(w.t.Context())
	if err != nil {
		w.t.Fatalf("the feed: %v", err)
	}
	w.srv.work()
	var out []string
	for _, c := range report.Changes {
		out = append(out, c.What+" "+c.Key)
	}
	return out
}

func same(got []string, want ...string) bool {
	return strings.Join(got, "; ") == strings.Join(want, "; ")
}

// byKey reads the announcement with a key.
func (w *world) byKey(key string) store.Announcement {
	w.t.Helper()
	list, err := w.st.Announcements(w.t.Context(), 1000)
	if err != nil {
		w.t.Fatal(err)
	}
	for _, a := range list {
		if a.Key == key {
			return a
		}
	}
	w.t.Fatalf("no announcement %q", key)
	return store.Announcement{}
}

func notice(key, topic string, at *time.Time, title string) AnnounceRequest {
	return AnnounceRequest{Key: key, Topic: topic, At: at, Texts: textsOf("de", title, "", "en", title+" (en)", "")}
}

// The first read of a feed: what has a time waits for it; what is simply so, nobody knows since
// when, is old news and is never told.
func TestFirstReadOfTheFeedTellsNoOldNews(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "semester", "exams")
	exams := start.Add(72 * time.Hour)
	f.name(notice("dates-2027s", "semester", nil, "Die Termine fürs SoSe 2027 sind da"), notice("exams-2026w-1", "exams", &exams, "Die Prüfungsphase beginnt"))
	if got := w.read(); !same(got, "skipped dates-2027s", "made exams-2026w-1") {
		t.Fatalf("the first read: %v", got)
	}
	if a := w.byKey("dates-2027s"); a.State != store.Skipped || a.Source != store.ByFeed || a.Timed {
		t.Fatalf("the old news: %+v", a)
	}
	if a := w.byKey("exams-2026w-1"); a.State != store.Waiting || a.Source != store.ByFeed || !a.Timed || !a.SendAt.Equal(exams) {
		t.Fatalf("the one with a time: %+v", a)
	}
	// Read again and again, nothing changes; then its time comes.
	if got := w.read(); len(got) != 0 {
		t.Fatalf("the second read: %v", got)
	}
	w.clock.Add(73 * time.Hour)
	if got := w.read(); len(got) != 0 {
		t.Fatalf("a read after its time: %v", got)
	}
	got := b.messages()
	if len(got) != 1 || got[0].Message.Notification.Title != "Die Prüfungsphase beginnt" || got[0].Message.Notification.Tag != "exams-2026w-1" {
		t.Fatalf("told: %+v", got)
	}
	// It is told and no longer named: nothing to withdraw, and never told again.
	f.name(notice("dates-2027s", "semester", nil, "Die Termine fürs SoSe 2027 sind da"))
	if got := w.read(); len(got) != 0 || len(b.messages()) != 1 {
		t.Fatalf("after it was told: %v, %d messages", got, len(b.messages()))
	}
}

// What becomes so later is told when Xylem first reads it, in the hours of the day.
func TestNewsWithoutATimeIsToldByDay(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "semester")
	if got := w.read(); len(got) != 0 { // the first read, of a feed that names nothing
		t.Fatal(got)
	}
	// 03:10 in Berlin: the catalog was built at night.
	w.clock.Add(time.Date(2026, 11, 24, 2, 10, 0, 0, time.UTC).Sub(start))
	f.name(notice("dates-2027s", "semester", nil, "Die Termine fürs SoSe 2027 sind da"))
	if got := w.read(); !same(got, "made dates-2027s") {
		t.Fatal(got)
	}
	eight := time.Date(2026, 11, 24, 8, 0, 0, 0, berlin)
	if a := w.byKey("dates-2027s"); a.State != store.Waiting || !a.SendAt.Equal(eight) || !a.Until.Equal(eight.Add(24*time.Hour)) {
		t.Fatalf("made at night: %+v", a)
	}
	if len(b.messages()) != 0 {
		t.Fatal("told at night")
	}
	// Read again before eight: the time Xylem chose stands.
	w.clock.Add(3 * time.Hour)
	if got := w.read(); len(got) != 0 {
		t.Fatalf("read again: %v", got)
	}
	w.clock.Add(2 * time.Hour) // 08:10
	w.read()
	if got := b.messages(); len(got) != 1 || got[0].Message.Notification.Timestamp != eight.UnixMilli() {
		t.Fatalf("at eight: %+v", got)
	}

	// By day it is told at once; in the evening the next morning.
	w.clock.Add(6 * time.Hour) // 14:10
	f.name(notice("dates-2027s", "semester", nil, "x"), notice("by-day", "semester", nil, "Am Tag"))
	w.read()
	if got := b.messages(); len(got) != 2 || got[1].Message.Notification.Title != "Am Tag" {
		t.Fatalf("by day: %d messages", len(got))
	}
	w.clock.Add(7 * time.Hour) // 21:10
	f.name(notice("dates-2027s", "semester", nil, "x"), notice("by-day", "semester", nil, "Am Tag"), notice("at-night", "semester", nil, "Am Abend"))
	w.read()
	if a := w.byKey("at-night"); !a.SendAt.Equal(time.Date(2026, 11, 25, 8, 0, 0, 0, berlin)) || len(b.messages()) != 2 {
		t.Fatalf("in the evening: %+v, %d messages", a, len(b.messages()))
	}
}

func TestHoursOfTheDay(t *testing.T) {
	w, _ := fed(t)
	at := func(text string) time.Time {
		v, err := time.ParseInLocation("2006-01-02 15:04", text, berlin)
		if err != nil {
			t.Fatal(err)
		}
		return v
	}
	for now, want := range map[string]string{
		"2026-11-24 07:59": "2026-11-24 08:00",
		"2026-11-24 08:00": "2026-11-24 08:00",
		"2026-11-24 19:59": "2026-11-24 19:59",
		"2026-11-24 20:00": "2026-11-25 08:00",
		"2026-12-31 23:30": "2027-01-01 08:00",
		"2027-03-28 01:30": "2027-03-28 08:00", // the night the clocks go forward
		"2026-10-25 02:30": "2026-10-25 08:00", // ... and back
	} {
		if got := w.srv.nextDaytime(at(now)); !got.Equal(at(want)) {
			t.Errorf("%s: %s, want %s", now, got.In(berlin).Format("2006-01-02 15:04"), want)
		}
	}
}

// While it waits, an announcement follows what the feed says of it; once told, it is told.
func TestWaitingAnnouncementFollowsTheFeed(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("en", "exams")
	monday := start.Add(7 * 24 * time.Hour)
	f.name(notice("exams-2026w-1", "exams", &monday, "Die Prüfungsphase beginnt"))
	w.read()

	// The catalog learned more: the exams begin a week later, and the text is another.
	later := monday.Add(7 * 24 * time.Hour)
	changed := notice("exams-2026w-1", "exams", &later, "Die Prüfungsphase beginnt am 8. Februar")
	changed.For, changed.Link = "36h", "/studyplan"
	f.name(changed)
	if got := w.read(); !same(got, "revised exams-2026w-1") {
		t.Fatal(got)
	}
	a := w.byKey("exams-2026w-1")
	if !a.SendAt.Equal(later) || !a.Until.Equal(later.Add(36*time.Hour)) || a.Link != "/studyplan" || a.ID != 1 {
		t.Fatalf("revised: %+v", a)
	}
	w.clock.Add(8 * 24 * time.Hour) // past the first time, before the new one
	w.read()
	if len(b.messages()) != 0 {
		t.Fatal("told at the time it had before")
	}
	w.clock.Add(7 * 24 * time.Hour)
	w.read()
	got := b.messages()
	if len(got) != 1 || got[0].Message.Notification.Title != "Die Prüfungsphase beginnt am 8. Februar (en)" || got[0].Message.Notification.Navigate != "https://betula.test/studyplan" {
		t.Fatalf("told: %+v", got)
	}
	// Told: what the feed says of it now changes nothing.
	again := later.Add(30 * 24 * time.Hour)
	f.name(notice("exams-2026w-1", "exams", &again, "Noch einmal"))
	if got := w.read(); len(got) != 0 {
		t.Fatalf("after it was told: %v", got)
	}
	w.clock.Add(31 * 24 * time.Hour)
	w.read()
	if len(b.messages()) != 1 {
		t.Fatal("told twice")
	}
}

// What the feed no longer names is not told; named again, it is.
func TestWithdrawnAndNamedAgain(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "breaks")
	friday := start.Add(4 * 24 * time.Hour)
	f.name(notice("breaks-2026w-1", "breaks", &friday, "Weihnachtspause"))
	w.read()
	f.name()
	if got := w.read(); !same(got, "withdrawn breaks-2026w-1") {
		t.Fatal(got)
	}
	if a := w.byKey("breaks-2026w-1"); a.State != store.Withdrawn || a.Finished.IsZero() {
		t.Fatalf("withdrawn: %+v", a)
	}
	w.clock.Add(5 * 24 * time.Hour)
	w.read()
	if len(b.messages()) != 0 {
		t.Fatal("a withdrawn announcement was told")
	}
	// Named again, at a new time.
	next := w.clock.Now().Add(24 * time.Hour)
	f.name(notice("breaks-2026w-1", "breaks", &next, "Weihnachtspause"))
	if got := w.read(); !same(got, "revived breaks-2026w-1") {
		t.Fatal(got)
	}
	if a := w.byKey("breaks-2026w-1"); a.State != store.Waiting || !a.Finished.IsZero() || !a.SendAt.Equal(next) || a.ID != 1 {
		t.Fatalf("revived: %+v", a)
	}
	w.clock.Add(25 * time.Hour)
	w.read()
	if len(b.messages()) != 1 {
		t.Fatalf("%d messages after it was named again", len(b.messages()))
	}

	// One whose time the feed moves into the past is withdrawn, not told late for a time that
	// was never right.
	soon := w.clock.Now().Add(48 * time.Hour)
	f.name(notice("exams-2026w-1", "breaks", &soon, "Prüfungen"))
	w.read()
	past := w.clock.Now().Add(-72 * time.Hour)
	f.name(notice("exams-2026w-1", "breaks", &past, "Prüfungen"))
	if got := w.read(); !same(got, "late exams-2026w-1", "withdrawn exams-2026w-1") {
		t.Fatal(got)
	}
}

// A person's word stands above the feed's: what a person cancelled stays cancelled, and what a
// person announced under a key is theirs.
func TestAPersonsWordStands(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "exams")
	monday := start.Add(48 * time.Hour)
	f.name(notice("exams-2026w-1", "exams", &monday, "Die Prüfungsphase beginnt"))
	w.read()
	if rec := w.do(http.MethodDelete, "/announcements/1", "", nil); rec.Code != http.StatusOK {
		t.Fatal(rec.Code, rec.Body)
	}
	other := monday.Add(time.Hour)
	f.name(notice("exams-2026w-1", "exams", &other, "Die Prüfungsphase beginnt doch"))
	if got := w.read(); len(got) != 0 {
		t.Fatalf("after a person cancelled it: %v", got)
	}
	w.clock.Add(72 * time.Hour)
	w.read()
	if a := w.byKey("exams-2026w-1"); a.State != store.Cancelled || len(b.messages()) != 0 {
		t.Fatalf("%s, %d messages", a.State, len(b.messages()))
	}
	// One the feed withdrew can be cancelled as well, so that it stays away.
	soon := w.clock.Now().Add(24 * time.Hour)
	f.name(notice("exams-2026w-2", "exams", &soon, "Zweite Prüfungsphase"))
	w.read()
	f.name()
	w.read()
	if rec := w.do(http.MethodDelete, "/announcements/2", "", nil); rec.Code != http.StatusOK {
		t.Fatal(rec.Code, rec.Body)
	}
	f.name(notice("exams-2026w-2", "exams", &soon, "Zweite Prüfungsphase"))
	if got := w.read(); len(got) != 0 {
		t.Fatalf("named again after a person cancelled it: %v", got)
	}

	// A person's announcement under a key the feed names later: theirs, as they wrote it.
	at := w.clock.Now().Add(2 * time.Hour)
	w.announce(AnnounceRequest{Key: "breaks-2026w-1", Topic: "exams", At: &at, Texts: textsOf("de", "Von Hand", "")})
	f.name(notice("breaks-2026w-1", "exams", &at, "Vom Feed"))
	if got := w.read(); len(got) != 0 {
		t.Fatalf("a key a person took: %v", got)
	}
	f.name()
	if got := w.read(); len(got) != 0 {
		t.Fatalf("a person's announcement was withdrawn: %v", got)
	}
	w.clock.Add(3 * time.Hour)
	w.read()
	if got := b.messages(); len(got) != 1 || got[0].Message.Notification.Title != "Von Hand" {
		t.Fatalf("told: %+v", got)
	}
	if a := w.byKey("breaks-2026w-1"); a.Source != store.ByCommand {
		t.Fatalf("made by %q", a.Source)
	}
}

// What the feed names wrongly is left out, and what it named rightly before stays.
func TestUnreadableEntriesOfTheFeed(t *testing.T) {
	w, f := fed(t)
	monday := start.Add(48 * time.Hour)
	good := notice("lectures-2026w-begin", "semester", &monday, "Die Vorlesungszeit beginnt")
	f.name(good)
	w.read()

	elsewhere := notice("exams-2026w-1", "exams", &monday, "Prüfungen")
	elsewhere.Link = "https://evil.example/"
	long := notice("lectures-2026w-begin", "semester", &monday, strings.Repeat("ä", 121))
	nameless := notice("", "semester", &monday, "Ohne Namen")
	twice := notice("breaks-2026w-1", "breaks", &monday, "Zweimal")
	english := AnnounceRequest{Key: "dates-2027s", Topic: "semester", Texts: textsOf("en", "Only English", "")}
	f.name(elsewhere, long, nameless, twice, twice, english)
	report, err := w.srv.readFeed(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	what := map[string]string{}
	for _, c := range report.Changes {
		what[c.Key+"/"+c.What] = c.Detail
	}
	for key, want := range map[string]string{
		"exams-2026w-1/invalid":        "a path of the site",
		"lectures-2026w-begin/invalid": "characters, 120 at most",
		"/invalid":                     "no key",
		"breaks-2026w-1/invalid":       "named twice",
		"dates-2027s/invalid":          `none in "de"`,
		"breaks-2026w-1/made":          "",
	} {
		if detail, ok := what[key]; !ok || !strings.Contains(detail, want) {
			t.Errorf("%s: %q (there: %v), want %q", key, detail, ok, want)
		}
	}
	if len(report.Changes) != 6 {
		t.Errorf("changes: %+v", report.Changes)
	}
	// The one that waited keeps its text and its time, and is not withdrawn.
	if a := w.byKey("lectures-2026w-begin"); a.State != store.Waiting || a.Texts["de"].Title != "Die Vorlesungszeit beginnt" {
		t.Fatalf("the one that waited: %+v", a)
	}
}

// A feed that went wrong must not reach every phone again and again: at most four of its
// announcements within a day.
func TestFeedCannotFlood(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "semester")
	if got := w.read(); len(got) != 0 {
		t.Fatal(got)
	}
	// By day, eight things are suddenly so.
	w.clock.Add(6 * time.Hour)
	var names []AnnounceRequest
	for i := range 8 {
		names = append(names, notice("oops-"+string(rune('a'+i)), "semester", nil, "Neu"))
	}
	f.name(names...)
	made, capped := 0, 0
	for _, c := range w.read() {
		switch strings.Fields(c)[0] {
		case "made":
			made++
		case "capped":
			capped++
		}
	}
	if made != 4 || capped != 4 || len(b.messages()) != 4 {
		t.Fatalf("made %d, not taken %d, told %d; want 4, 4, 4", made, capped, len(b.messages()))
	}
	// Ten minutes later, and the next: still four.
	for range 3 {
		w.clock.Add(10 * time.Minute)
		w.read()
	}
	if len(b.messages()) != 4 {
		t.Fatalf("%d messages after the next reads", len(b.messages()))
	}
	// And a day on, the rest: what is still so is still news.
	w.clock.Add(24 * time.Hour)
	w.read()
	if len(b.messages()) != 8 {
		t.Fatalf("%d messages a day later, want 8", len(b.messages()))
	}
	// What a person announces is no part of the count.
	out := w.announce(AnnounceRequest{Topic: "semester", Texts: textsOf("de", "Von Hand", "")})
	if !out.Made || len(b.messages()) != 9 {
		t.Fatalf("a person's announcement beside them: %+v, %d messages", out, len(b.messages()))
	}
}

// A feed that cannot be read changes nothing: what waits is told, nothing is withdrawn.
func TestFeedThatCannotBeRead(t *testing.T) {
	w, f := fed(t)
	b := w.subscribe("de", "exams")
	soon := start.Add(time.Hour)
	f.name(notice("exams-2026w-1", "exams", &soon, "Die Prüfungsphase beginnt"))
	w.read()

	many := make([]AnnounceRequest, 65)
	for i := range many {
		many[i] = notice("n-"+strings.Repeat("x", i+1), "exams", &soon, "x")
	}
	tooMany, _ := json.Marshal(map[string]any{"announcements": many})
	for name, answer := range map[string]struct {
		status int
		body   string
	}{
		"the site is down":       {http.StatusBadGateway, "bad gateway"},
		"no snapshot yet":        {http.StatusServiceUnavailable, "no snapshot yet"},
		"closed testing":         {http.StatusUnauthorized, "Betula is in closed testing"},
		"a page":                 {http.StatusOK, "<!doctype html><title>Betula</title>"},
		"JSON that says nothing": {http.StatusOK, `{"error":"x"}`},
		"null":                   {http.StatusOK, `{"announcements":null}`},
		"too large":              {http.StatusOK, `{"announcements":[],"x":"` + strings.Repeat("a", 300<<10) + `"}`},
		"too many":               {http.StatusOK, string(tooMany)},
		"a redirect":             {http.StatusFound, ""},
	} {
		f.say(answer.status, answer.body)
		if _, err := w.srv.readFeed(t.Context()); err == nil {
			t.Errorf("%s: the read came through", name)
		}
		if a := w.byKey("exams-2026w-1"); a.State != store.Waiting {
			t.Fatalf("%s: what waited is %s", name, a.State)
		}
	}
	var st Status
	_ = json.Unmarshal(w.do(http.MethodGet, "/status", "", nil).Body.Bytes(), &st)
	if st.Feed == nil || st.Feed.Failing == "" || st.Feed.Read == nil || st.Feed.Entries != 1 || !strings.HasSuffix(st.Feed.URL, "/api/announcements") {
		t.Fatalf("status of a failing feed: %+v", st.Feed)
	}
	w.clock.Add(2 * time.Hour)
	w.srv.work()
	if len(b.messages()) != 1 {
		t.Fatal("what waited was not told while the feed was down")
	}
	f.name()
	if got := w.read(); len(got) != 0 {
		t.Fatalf("the feed is back: %v", got)
	}
	var back Status
	_ = json.Unmarshal(w.do(http.MethodGet, "/status", "", nil).Body.Bytes(), &back)
	if back.Feed == nil || back.Feed.Failing != "" || back.Feed.Entries != 0 {
		t.Fatalf("status after it came back: %+v", back.Feed)
	}
}

func TestFeedUnchangedAndAnotherFeed(t *testing.T) {
	w, f := fed(t)
	soon := start.Add(time.Hour)
	f.etag = `"one"`
	f.name(notice("exams-2026w-1", "exams", &soon, "Prüfungen"), notice("dates-2027s", "semester", nil, "Termine"))
	if report, err := w.srv.readFeed(t.Context()); err != nil || !report.Baseline || report.Unchanged || report.Entries != 2 {
		t.Fatalf("the first read: %+v, %v", report, err)
	}
	if report, err := w.srv.readFeed(t.Context()); err != nil || !report.Unchanged || len(report.Changes) != 0 {
		t.Fatalf("the same again: %+v, %v", report, err)
	}
	f.mu.Lock()
	f.etag = `"two"`
	f.mu.Unlock()
	if report, err := w.srv.readFeed(t.Context()); err != nil || report.Unchanged || report.Baseline || len(report.Changes) != 0 {
		t.Fatalf("another version, the same content: %+v, %v", report, err)
	}

	// Another feed (canary's Xylem pointed at the site's): its first read is a first read.
	other := newSite(t)
	other.name(notice("dates-2027w", "semester", nil, "Termine fürs Wintersemester"), notice("exams-2026w-1", "exams", &soon, "Prüfungen"))
	w.srv.feed = other.srv.URL + "/api/announcements"
	w.srv.feedSt.etag = ""
	if got := w.read(); !same(got, "skipped dates-2027w") {
		t.Fatalf("another feed's first read: %v", got)
	}
}

// Without a feed nothing is read, and `xylem feed` says so; with one, only this machine may ask.
func TestFeedIsReadOnAsking(t *testing.T) {
	plain := newWorld(t)
	if rec := plain.do(http.MethodPost, "/feed", "", `{}`); rec.Code != http.StatusConflict || errorOf(rec) != "no_feed: no feed is set (--feed, XYLEM_FEED)" {
		t.Fatalf("without a feed: %d %s", rec.Code, rec.Body)
	}
	var st Status
	_ = json.Unmarshal(plain.do(http.MethodGet, "/status", "", nil).Body.Bytes(), &st)
	if st.Feed != nil {
		t.Fatalf("a feed in the status of a Xylem without one: %+v", st.Feed)
	}

	w, f := fed(t)
	soon := start.Add(time.Hour)
	f.name(notice("exams-2026w-1", "exams", &soon, "Prüfungen"))
	if rec := w.do(http.MethodPost, "/feed", "203.0.113.9", `{}`); rec.Code != http.StatusNotFound {
		t.Fatalf("from outside: %d", rec.Code)
	}
	rec := w.do(http.MethodPost, "/feed", "", `{}`)
	var report FeedReport
	if err := json.Unmarshal(rec.Body.Bytes(), &report); err != nil || rec.Code != http.StatusOK || !report.Baseline || len(report.Changes) != 1 ||
		report.Changes[0].What != "made" || report.Changes[0].ID != 1 || report.Changes[0].At == nil || !report.Changes[0].At.Equal(soon) {
		t.Fatalf("POST /feed: %d %s", rec.Code, rec.Body)
	}
	f.say(http.StatusInternalServerError, "")
	if rec := w.do(http.MethodPost, "/feed", "", `{}`); rec.Code != http.StatusBadGateway || !strings.Contains(errorOf(rec), "answered 500") {
		t.Fatalf("a failing feed: %d %s", rec.Code, rec.Body)
	}
	// The list says who made what.
	var list struct{ Announcements []AnnouncementView }
	_ = json.Unmarshal(w.do(http.MethodGet, "/announcements", "", nil).Body.Bytes(), &list)
	if len(list.Announcements) != 1 || list.Announcements[0].Source != "feed" {
		t.Fatalf("list: %+v", list.Announcements)
	}

	for _, feed := range []string{"betula.app/api/announcements", "ftp://betula.app/x", "https://user@betula.app/x", "https:///x"} {
		st, _ := store.Open(t.TempDir())
		if srv, err := New(Options{Store: st, Key: w.srv.key, Site: "https://betula.app", Feed: feed}); err == nil {
			_ = srv.Close()
			t.Errorf("the feed %q was taken", feed)
		}
		_ = st.Close()
	}
}

// A Xylem that is given a feed reads it by itself, at its start.
func TestFeedIsReadByItself(t *testing.T) {
	f := newSite(t)
	soon := start.Add(time.Hour)
	f.name(notice("exams-2026w-1", "exams", &soon, "Prüfungen"))
	w := newWorld(t, func(o *Options) { o.Feed = f.srv.URL + "/api/announcements" })
	for deadline := time.Now().Add(5 * time.Second); ; time.Sleep(10 * time.Millisecond) {
		if list, _ := w.st.Announcements(t.Context(), 10); len(list) == 1 && list[0].Key == "exams-2026w-1" {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("the feed was not read")
		}
	}
}
