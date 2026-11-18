package server

import (
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/store"
)

// An announcement reaches the browsers that listen to its topic, each in its language, sealed
// for it, and nobody else.
func TestAnnouncementReachesItsTopicInEachLanguage(t *testing.T) {
	w := newWorld(t)
	german := w.subscribe("de", "exams")
	english := w.subscribe("en", "exams", "semester")
	other := w.subscribe("de", "semester")

	texts := textsOf("de", "Die Prüfungsphase beginnt", "Ab heute laufen die Prüfungen.", "en", "Exam period begins", "Exams start today.")
	en := texts["en"]
	en.Link = "/en/studyplan"
	texts["en"] = en
	out := w.announce(AnnounceRequest{Topic: "exams", Key: "exams-2026w", Link: "/studyplan", Texts: texts})
	if !out.Made || out.Reach["de"] != 1 || out.Reach["en"] != 1 {
		t.Fatalf("answer %+v", out)
	}
	id := out.Announcement.ID
	w.counts(id, store.Done, 2, 0, 0)

	if got := other.messages(); len(got) != 0 {
		t.Fatalf("a browser of another topic got %d messages", len(got))
	}
	de, eng := german.messages(), english.messages()
	if len(de) != 1 || len(eng) != 1 {
		t.Fatalf("messages: de %d, en %d", len(de), len(eng))
	}
	m := de[0].Message
	if m.WebPush != 8030 || m.Notification.Title != "Die Prüfungsphase beginnt" || m.Notification.Body != "Ab heute laufen die Prüfungen." ||
		m.Notification.Navigate != "https://betula.test/studyplan" || m.Notification.Lang != "de" || m.Notification.Tag != "exams-2026w" ||
		m.Notification.Timestamp != start.UnixMilli() || m.Notification.Data.Topic != "exams" {
		t.Fatalf("the German message: %s", de[0].Raw)
	}
	if n := eng[0].Message.Notification; n.Title != "Exam period begins" || n.Navigate != "https://betula.test/en/studyplan" || n.Lang != "en" {
		t.Fatalf("the English message: %s", eng[0].Raw)
	}
	// A declarative push message as Safari reads it: the two members it must have, as strings,
	// and nothing escaped that a parser of JSON would not take.
	if !strings.HasPrefix(de[0].Raw, `{"web_push":8030,"notification":{"title":"Die Prüfungsphase beginnt",`) || !strings.Contains(de[0].Raw, `"navigate":"https://`) {
		t.Fatalf("the message as sent: %s", de[0].Raw)
	}
	// The push service keeps it as long as it is told: a day.
	if de[0].TTL != 24*3600 || de[0].Urgency != "normal" {
		t.Fatalf("TTL %d, Urgency %q", de[0].TTL, de[0].Urgency)
	}
	// One token for the push service, signed once.
	if de[0].Token != eng[0].Token {
		t.Fatal("two requests to one push service came with two tokens")
	}

	// Told again under its key, it is one announcement and nobody hears it twice.
	again := w.announce(AnnounceRequest{Topic: "exams", Key: "exams-2026w", Texts: textsOf("de", "Noch einmal", "")})
	if again.Made || again.Announcement.ID != id || len(german.messages()) != 1 {
		t.Fatalf("the same key again: %+v, %d messages", again, len(german.messages()))
	}
}

func TestLanguageWithoutATextReadsTheNearest(t *testing.T) {
	w := newWorld(t)
	british := w.subscribe("en-GB", "breaks")
	french := w.subscribe("fr", "breaks")
	unnamed := w.subscribe("", "breaks")
	w.announce(AnnounceRequest{Topic: "breaks", Texts: textsOf("de", "Weihnachtspause", "", "en", "Christmas break", "")})
	for name, want := range map[*browser][2]string{british: {"Christmas break", "en"}, french: {"Weihnachtspause", "de"}, unnamed: {"Weihnachtspause", "de"}} {
		got := name.messages()
		if len(got) != 1 || got[0].Message.Notification.Title != want[0] || got[0].Message.Notification.Lang != want[1] {
			t.Errorf("want %q in %s, got %+v", want[0], want[1], got)
		}
	}
	// Without a key, the tag is the announcement's own.
	if tag := british.messages()[0].Message.Notification.Tag; tag != "xylem-1" {
		t.Errorf("tag %q", tag)
	}
	// A body of nothing is left out, not sent empty.
	if raw := british.messages()[0].Raw; strings.Contains(raw, `"body"`) {
		t.Errorf("an empty body was sent: %s", raw)
	}
}

// A subscription its push service calls gone is forgotten; the others are told.
func TestGoneSubscriptionIsForgotten(t *testing.T) {
	w := newWorld(t)
	stays := w.subscribe("de", "exams")
	gone := w.subscribe("de", "exams")
	missing := w.subscribe("de", "exams")
	gone.answer(http.StatusGone)
	missing.answer(http.StatusNotFound)
	out := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")})
	w.counts(out.Announcement.ID, store.Done, 1, 2, 0)
	if n := w.st.Subscriptions(); n != 1 {
		t.Fatalf("%d subscriptions left, want 1", n)
	}
	if len(stays.messages()) != 1 {
		t.Fatal("the one that stayed was not told")
	}
	// The next announcement does not ask for them again.
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Noch eine", "")})
	if gone.requests() != 1 || missing.requests() != 1 || len(stays.messages()) != 2 {
		t.Fatalf("requests: gone %d, missing %d; stayed got %d", gone.requests(), missing.requests(), len(stays.messages()))
	}
}

// A push service that is busy is asked again later, as often as the waits allow and while the
// announcement is told.
func TestRetryUntilItIsTaken(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	fine := w.subscribe("de", "exams")
	b.answer(http.StatusServiceUnavailable, http.StatusInternalServerError)
	id := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")}).Announcement.ID
	w.counts(id, store.Retrying, 1, 0, 0)
	if len(fine.messages()) != 1 || b.requests() != 1 {
		t.Fatalf("after the first round: fine %d, busy asked %d times", len(fine.messages()), b.requests())
	}

	w.clock.Add(30 * time.Second) // not due yet
	w.srv.work()
	if b.requests() != 1 {
		t.Fatalf("asked again after 30 s (%d requests)", b.requests())
	}
	w.clock.Add(31 * time.Second) // the first wait, a minute
	w.srv.work()
	w.counts(id, store.Retrying, 1, 0, 0)
	if b.requests() != 2 {
		t.Fatalf("%d requests after the first wait", b.requests())
	}
	w.clock.Add(4 * time.Minute) // the second wait is five minutes
	w.srv.work()
	if b.requests() != 2 {
		t.Fatalf("asked again before the second wait was over (%d requests)", b.requests())
	}
	w.clock.Add(61 * time.Second)
	w.srv.work()
	w.counts(id, store.Done, 2, 0, 0)
	got := b.messages()
	if len(got) != 1 || got[0].Message.Notification.Title != "Prüfungen" {
		t.Fatalf("the busy one got %+v", got)
	}
	// What is left of the day it is told for: the push service keeps it no longer.
	if want := 24*3600 - 6*60 - 2; got[0].TTL != want {
		t.Fatalf("TTL %d, want %d", got[0].TTL, want)
	}
	if len(fine.messages()) != 1 {
		t.Fatalf("the other one got it %d times", len(fine.messages()))
	}
}

func TestRetryAfterIsFollowed(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	b.after = 600
	b.answer(http.StatusTooManyRequests)
	id := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")}).Announcement.ID
	w.clock.Add(9 * time.Minute)
	w.srv.work()
	if b.requests() != 1 {
		t.Fatalf("asked again within the ten minutes the push service named (%d requests)", b.requests())
	}
	w.clock.Add(61 * time.Second)
	w.srv.work()
	w.counts(id, store.Done, 1, 0, 0)
}

func TestGivingUp(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	b.answer(500, 500, 500, 500, 500)
	id := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")}).Announcement.ID
	for range 4 {
		w.clock.Add(10 * time.Minute)
		w.srv.work()
	}
	// The first attempt and one after each of the two waits.
	w.counts(id, store.Done, 0, 0, 1)
	if b.requests() != 3 {
		t.Fatalf("%d requests, want 3", b.requests())
	}
	if n := w.st.Subscriptions(); n != 1 {
		t.Fatal("a subscription whose push service was broken was forgotten")
	}

	// And where the time runs out before the attempts do: told for a minute, the first wait
	// alone ends with it.
	b.answer(503)
	short := w.announce(AnnounceRequest{Topic: "exams", For: "1m", Texts: textsOf("de", "Kurz", "")}).Announcement.ID
	w.counts(short, store.Done, 0, 0, 1)

	// And one that waits for an attempt when its time ends: the attempt is not made.
	b.answer(503)
	waiting := w.announce(AnnounceRequest{Topic: "exams", For: "3m", Texts: textsOf("de", "Drei Minuten", "")}).Announcement.ID
	w.counts(waiting, store.Retrying, 0, 0, 0)
	asked := b.requests()
	w.clock.Add(time.Minute + time.Second)
	b.answer(503) // the second wait, five minutes, would end after its time: given up at once
	w.srv.work()
	w.counts(waiting, store.Done, 0, 0, 1)
	if b.requests() != asked+1 {
		t.Fatalf("%d requests more, want 1", b.requests()-asked)
	}
}

// A push service that refuses Xylem's token: the subscription stays (the fault is Xylem's), and
// the announcement counts it as failed.
func TestRefusedTokenKeepsTheSubscription(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	b.answer(http.StatusForbidden, http.StatusUnauthorized, http.StatusBadRequest, http.StatusRequestEntityTooLarge, http.StatusTeapot)
	for i := range 5 {
		id := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")}).Announcement.ID
		w.counts(id, store.Done, 0, 0, 1)
		if n := w.st.Subscriptions(); n != 1 {
			t.Fatalf("answer %d: the subscription was forgotten", i)
		}
	}
	if b.requests() != 5 {
		t.Fatalf("%d requests: one of them was tried again", b.requests())
	}
}

func TestAnnouncementWaitsForItsTime(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "breaks")
	at := start.Add(48 * time.Hour)
	out := w.announce(AnnounceRequest{Topic: "breaks", At: &at, For: "12h", Texts: textsOf("de", "Weihnachtspause", "Bis zum 4. Januar.")})
	id := out.Announcement.ID
	w.counts(id, store.Waiting, 0, 0, 0)
	if !out.Announcement.At.Equal(at) || !out.Announcement.Until.Equal(at.Add(12*time.Hour)) {
		t.Fatalf("at %s until %s", out.Announcement.At, out.Announcement.Until)
	}
	w.clock.Add(47 * time.Hour)
	w.srv.work()
	if len(b.messages()) != 0 {
		t.Fatal("told before its time")
	}

	// Who subscribes after its time does not hear what was said before; who was there does.
	w.clock.Add(time.Hour + time.Minute)
	late := w.subscribe("de", "breaks")
	w.srv.work()
	w.counts(id, store.Done, 1, 0, 0)
	got := b.messages()
	if len(got) != 1 || got[0].TTL != 12*3600-60 || got[0].Message.Notification.Timestamp != at.UnixMilli() {
		t.Fatalf("got %+v", got)
	}
	if len(late.messages()) != 0 {
		t.Fatal("a browser that subscribed afterwards was told")
	}
}

// Xylem did not run while an announcement was due: nobody hears stale news.
func TestExpiredAnnouncementIsNeverTold(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	at := start.Add(time.Hour)
	rec := w.do(http.MethodPost, "/announcements", "", AnnounceRequest{Topic: "exams", At: &at, For: "2h", Texts: textsOf("de", "Heute", "")})
	if rec.Code != http.StatusCreated {
		t.Fatal(rec.Code, rec.Body)
	}
	w.clock.Add(4 * time.Hour)
	w.srv.work()
	w.counts(1, store.Expired, 0, 0, 0)
	if len(b.messages()) != 0 {
		t.Fatal("told after its time")
	}
	// An announcement whose time is over when it is made is not taken at all.
	past := w.clock.Now().Add(-3 * time.Hour)
	rec = w.do(http.MethodPost, "/announcements", "", AnnounceRequest{Topic: "exams", At: &past, For: "2h", Texts: textsOf("de", "Gestern", "")})
	if rec.Code != http.StatusBadRequest || !strings.Contains(errorOf(rec), "its time is over") {
		t.Fatalf("%d %s", rec.Code, rec.Body)
	}
	// One whose time began before and is not over is told now.
	began := w.clock.Now().Add(-time.Hour)
	id := w.announce(AnnounceRequest{Topic: "exams", At: &began, For: "2h", Texts: textsOf("de", "Seit einer Stunde", "")}).Announcement.ID
	w.counts(id, store.Done, 1, 0, 0)
	if got := b.messages(); len(got) != 1 || got[0].TTL != 3600 {
		t.Fatalf("got %+v", got)
	}
}

func TestCancel(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	at := start.Add(time.Hour)
	id := w.announce(AnnounceRequest{Topic: "exams", At: &at, Texts: textsOf("de", "Später", "")}).Announcement.ID
	if rec := w.do(http.MethodDelete, "/announcements/1", "", nil); rec.Code != http.StatusOK {
		t.Fatal(rec.Code, rec.Body)
	}
	w.clock.Add(2 * time.Hour)
	w.srv.work()
	w.counts(id, store.Cancelled, 0, 0, 0)
	if len(b.messages()) != 0 {
		t.Fatal("a cancelled announcement was told")
	}
	if rec := w.do(http.MethodDelete, "/announcements/1", "", nil); rec.Code != http.StatusConflict {
		t.Fatalf("cancelled twice: %d", rec.Code)
	}
	for _, path := range []string{"/announcements/99", "/announcements/x", "/announcements/0"} {
		if rec := w.do(http.MethodDelete, path, "", nil); rec.Code != http.StatusNotFound {
			t.Errorf("%s: %d", path, rec.Code)
		}
	}

	// One that waits for another attempt: cancelled, the attempt is not made.
	b.answer(503)
	retrying := w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Jetzt", "")}).Announcement.ID
	w.counts(retrying, store.Retrying, 0, 0, 0)
	if rec := w.do(http.MethodDelete, "/announcements/2", "", nil); rec.Code != http.StatusOK {
		t.Fatal(rec.Code, rec.Body)
	}
	w.clock.Add(time.Hour)
	w.srv.work()
	if b.requests() != 1 {
		t.Fatalf("%d requests: the attempt of a cancelled announcement was made", b.requests())
	}
}

// Many subscriptions, in batches: each hears it once.
func TestEveryBatchIsWrittenOnce(t *testing.T) {
	w := newWorld(t, func(o *Options) { o.Limits.BatchSize, o.Limits.Senders = 3, 2 })
	var all []*browser
	for i := range 10 {
		lang := "de"
		if i%3 == 0 {
			lang = "en"
		}
		all = append(all, w.subscribe(lang, "semester"))
	}
	w.subscribe("de", "exams")
	id := w.announce(AnnounceRequest{Topic: "semester", Texts: textsOf("de", "Die Termine sind da", "", "en", "The dates are in", "")}).Announcement.ID
	w.counts(id, store.Done, 10, 0, 0)
	for i, b := range all {
		if got := b.messages(); len(got) != 1 {
			t.Errorf("browser %d got %d messages", i, len(got))
		}
	}
}

// A token is signed for twelve hours and replaced before it ends.
func TestTokenIsRenewed(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams")
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Eins", "")})
	w.clock.Add(9 * time.Hour)
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Zwei", "")})
	w.clock.Add(2 * time.Hour) // eleven hours: within the last sixth
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Drei", "")})
	got := b.messages()
	if len(got) != 3 || got[0].Token != got[1].Token || got[1].Token == got[2].Token {
		t.Fatalf("tokens: same %v, renewed %v", got[0].Token == got[1].Token, got[1].Token != got[2].Token)
	}
}

// What was told is kept for thirteen months and then deleted.
func TestOldAnnouncementsGo(t *testing.T) {
	w := newWorld(t)
	w.subscribe("de", "exams")
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Alt", "")})
	w.clock.Add(394 * 24 * time.Hour)
	if n, err := w.st.Sweep(t.Context(), w.clock.Now().Add(-w.srv.lim.KeepFinished)); err != nil || n != 0 {
		t.Fatalf("after 394 days: %d, %v", n, err)
	}
	w.clock.Add(2 * 24 * time.Hour)
	if n, err := w.st.Sweep(t.Context(), w.clock.Now().Add(-w.srv.lim.KeepFinished)); err != nil || n != 1 {
		t.Fatalf("after 396 days: %d, %v", n, err)
	}
}
