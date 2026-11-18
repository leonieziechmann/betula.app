package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"path/filepath"
	"testing"
	"time"
)

var t0 = time.Date(2026, 11, 23, 8, 0, 0, 0, time.UTC)

func open(t *testing.T) *Store {
	t.Helper()
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = s.Close() })
	return s
}

func subscription(i int, lang string, topics ...string) Subscription {
	return Subscription{
		Endpoint: fmt.Sprintf("https://push.example/s/%d", i),
		P256dh:   []byte{4, byte(i)}, Auth: []byte{byte(i)}, Lang: lang, Topics: topics,
	}
}

func put(t *testing.T, s *Store, sub Subscription, at time.Time) {
	t.Helper()
	if _, err := s.Put(context.Background(), sub, at, 1000); err != nil {
		t.Fatal(err)
	}
}

func announce(t *testing.T, s *Store, topic string, at time.Time, life time.Duration) Announcement {
	t.Helper()
	a, made, err := s.Announce(context.Background(), Announcement{
		Topic: topic, SendAt: at, Until: at.Add(life), Urgency: "normal", Link: "/",
		Texts: map[string]Text{"de": {Title: "Titel", Body: "Text"}},
	}, at)
	if err != nil || !made {
		t.Fatalf("announce: %v, made %v", err, made)
	}
	return a
}

func nrs(targets []Target) []int64 {
	out := make([]int64, len(targets))
	for i, t := range targets {
		out[i] = t.Nr
	}
	return out
}

func TestSubscriptionsAreKeptReplacedAndCounted(t *testing.T) {
	ctx := context.Background()
	s := open(t)
	made, err := s.Put(ctx, subscription(1, "de", "exams", "semester"), t0, 2)
	if err != nil || !made || s.Subscriptions() != 1 {
		t.Fatalf("first: made %v, %v, %d", made, err, s.Subscriptions())
	}
	// Again: the same subscription with other keys, language and topics, and its number.
	changed := subscription(1, "en", "breaks")
	changed.P256dh, changed.Auth = []byte{4, 9}, []byte{9}
	if made, err := s.Put(ctx, changed, t0.Add(time.Hour), 2); err != nil || made || s.Subscriptions() != 1 {
		t.Fatalf("again: made %v, %v, %d", made, err, s.Subscriptions())
	}
	topics, lang, found, err := s.Topics(ctx, changed.Endpoint)
	if err != nil || !found || lang != "en" || len(topics) != 1 || topics[0] != "breaks" {
		t.Fatalf("topics %v %q %v %v", topics, lang, found, err)
	}
	a := announce(t, s, "breaks", t0.Add(2*time.Hour), time.Hour)
	batch, err := s.Batch(ctx, a, 10)
	if err != nil || len(batch) != 1 || batch[0].Nr != 1 || string(batch[0].P256dh) != "\x04\x09" || string(batch[0].Auth) != "\x09" || batch[0].Lang != "en" {
		t.Fatalf("batch %+v, %v", batch, err)
	}

	put(t, s, subscription(2, "de", "exams"), t0)
	if _, err := s.Put(ctx, subscription(3, "de", "exams"), t0, 2); !errors.Is(err, ErrFull) {
		t.Fatalf("a third of two: %v", err)
	}
	if made, err := s.Put(ctx, subscription(2, "de", "semester"), t0, 2); err != nil || made {
		t.Fatalf("an update while full: %v %v", made, err)
	}
	byTopic, byLang, err := s.Listeners(ctx)
	if err != nil || byTopic["breaks"] != 1 || byTopic["semester"] != 1 || byTopic["exams"] != 0 || byLang["de"] != 1 || byLang["en"] != 1 {
		t.Fatalf("listeners %v %v %v", byTopic, byLang, err)
	}
	if reach, err := s.Reach(ctx, "semester"); err != nil || reach["de"] != 1 || len(reach) != 1 {
		t.Fatalf("reach %v %v", reach, err)
	}

	if found, err := s.Remove(ctx, changed.Endpoint); err != nil || !found || s.Subscriptions() != 1 {
		t.Fatalf("remove: %v %v %d", found, err, s.Subscriptions())
	}
	if found, err := s.Remove(ctx, changed.Endpoint); err != nil || found {
		t.Fatalf("remove again: %v %v", found, err)
	}
	if _, _, found, _ := s.Topics(ctx, changed.Endpoint); found {
		t.Fatal("still there")
	}
	// A number is never given out again: the next one is 3, not 1.
	put(t, s, subscription(4, "de", "breaks"), t0)
	b := announce(t, s, "breaks", t0.Add(3*time.Hour), time.Hour)
	if batch, _ := s.Batch(ctx, b, 10); len(batch) != 1 || batch[0].Nr != 3 {
		t.Fatalf("batch %+v", batch)
	}
}

func TestVAPIDKeyIsMadeOnce(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	calls := 0
	make := func() (string, error) { calls++; return fmt.Sprintf("key-%d", calls), nil }
	first, err := s.VAPID(ctx, make)
	if err != nil || first != "key-1" {
		t.Fatal(first, err)
	}
	if again, err := s.VAPID(ctx, make); err != nil || again != "key-1" || calls != 1 {
		t.Fatalf("again %q, %v, %d calls", again, err, calls)
	}
	put(t, s, subscription(1, "de", "exams"), t0)
	if err := s.Close(); err != nil {
		t.Fatal(err)
	}
	if err := s.Ping(ctx); !errors.Is(err, ErrClosed) {
		t.Fatalf("ping after close: %v", err)
	}
	// Opened again: the key and the subscriptions are what they were.
	s, err = Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	if key, err := s.VAPID(ctx, make); err != nil || key != "key-1" || calls != 1 || s.Subscriptions() != 1 {
		t.Fatalf("reopened: %q, %v, %d calls, %d subscriptions", key, err, calls, s.Subscriptions())
	}
	failing := open(t)
	if _, err := failing.VAPID(ctx, func() (string, error) { return "", errors.New("no entropy") }); err == nil {
		t.Fatal("a key that could not be made was kept")
	}
	if key, _ := failing.VAPID(ctx, make); key != "key-2" {
		t.Fatalf("after the failure: %q", key)
	}
}

func TestAnnouncementWalksItsTopicInBatches(t *testing.T) {
	ctx := context.Background()
	s := open(t)
	for i := 1; i <= 5; i++ {
		put(t, s, subscription(i, "de", "exams"), t0)
	}
	put(t, s, subscription(6, "de", "semester"), t0)
	put(t, s, subscription(7, "de", "exams"), t0.Add(2*time.Hour)) // after the announcement's time
	a := announce(t, s, "exams", t0.Add(time.Hour), 24*time.Hour)
	if a.ID != 1 || a.State != Waiting || a.Cursor != 0 || !a.Finished.IsZero() || a.Texts["de"].Title != "Titel" {
		t.Fatalf("made %+v", a)
	}
	if due, _ := s.Due(ctx, t0); len(due) != 0 {
		t.Fatal("due before its time")
	}
	if at, ok, _ := s.Next(ctx); !ok || !at.Equal(a.SendAt) {
		t.Fatalf("next %v %v", at, ok)
	}
	now := t0.Add(time.Hour)
	due, err := s.Due(ctx, now)
	if err != nil || len(due) != 1 {
		t.Fatal(due, err)
	}

	batch, _ := s.Batch(ctx, a, 2)
	if got := nrs(batch); len(got) != 2 || got[0] != 1 || got[1] != 2 {
		t.Fatalf("first batch %v", got)
	}
	ok, err := s.Record(ctx, a.ID, 2, []Outcome{{Sub: 1, Result: Sent}, {Sub: 2, Result: Gone}})
	if err != nil || !ok {
		t.Fatal(ok, err)
	}
	if s.Subscriptions() != 6 {
		t.Fatalf("%d subscriptions after one was gone", s.Subscriptions())
	}
	a, _ = s.Announcement(ctx, a.ID)
	if a.State != Sending || a.Cursor != 2 || a.Sent != 1 || a.Gone != 1 {
		t.Fatalf("after the first batch %+v", a)
	}
	batch, _ = s.Batch(ctx, a, 2)
	if got := nrs(batch); len(got) != 2 || got[0] != 3 || got[1] != 4 {
		t.Fatalf("second batch %v", got)
	}
	retryAt := now.Add(time.Minute)
	if ok, err := s.Record(ctx, a.ID, 4, []Outcome{{Sub: 3, Result: Again, Due: retryAt}, {Sub: 4, Result: Failed}}); err != nil || !ok {
		t.Fatal(ok, err)
	}
	a, _ = s.Announcement(ctx, a.ID)
	batch, _ = s.Batch(ctx, a, 2)
	if got := nrs(batch); len(got) != 1 || got[0] != 5 {
		t.Fatalf("third batch %v: the one of another topic or the late one is in it", got)
	}
	if ok, err := s.Record(ctx, a.ID, 5, []Outcome{{Sub: 5, Result: Sent}}); err != nil || !ok {
		t.Fatal(ok, err)
	}
	a, _ = s.Announcement(ctx, a.ID)
	if batch, _ := s.Batch(ctx, a, 2); len(batch) != 0 {
		t.Fatalf("a fourth batch %v", nrs(batch))
	}
	if err := s.Walked(ctx, a.ID, now); err != nil {
		t.Fatal(err)
	}
	a, _ = s.Announcement(ctx, a.ID)
	if a.State != Retrying || a.Sent != 2 || a.Gone != 1 || a.Failed != 1 || !a.Finished.IsZero() {
		t.Fatalf("walked %+v", a)
	}
	if byState, retries, err := s.Pending(ctx); err != nil || byState[Retrying] != 1 || retries != 1 {
		t.Fatalf("pending %v %d %v", byState, retries, err)
	}

	// The attempt that waits: not due yet, then due; tried again and failing, it counts two.
	if list, _ := s.Retries(ctx, now, 10); len(list) != 0 {
		t.Fatal("due before its wait was over")
	}
	list, err := s.Retries(ctx, retryAt, 10)
	if err != nil || len(list) != 1 || list[0].Target.Nr != 3 || list[0].Attempts != 1 || list[0].Announcement.ID != a.ID {
		t.Fatalf("retries %+v %v", list, err)
	}
	if err := s.Resolve(ctx, a.ID, []Outcome{{Sub: 3, Result: Again, Due: retryAt.Add(5 * time.Minute)}}); err != nil {
		t.Fatal(err)
	}
	if settled, _ := s.Settle(ctx, retryAt); len(settled) != 0 {
		t.Fatalf("settled while an attempt waits: %+v", settled)
	}
	list, _ = s.Retries(ctx, retryAt.Add(5*time.Minute), 10)
	if len(list) != 1 || list[0].Attempts != 2 {
		t.Fatalf("retries %+v", list)
	}
	// What is resolved for a subscription that no longer waits changes nothing.
	if err := s.Resolve(ctx, a.ID, []Outcome{{Sub: 3, Result: Sent}, {Sub: 5, Result: Sent}, {Sub: 99, Result: Gone}}); err != nil {
		t.Fatal(err)
	}
	settled, err := s.Settle(ctx, retryAt.Add(6*time.Minute))
	if err != nil || len(settled) != 1 || settled[0].State != Done || settled[0].Dropped != 0 {
		t.Fatalf("settled %+v %v", settled, err)
	}
	a, _ = s.Announcement(ctx, a.ID)
	if a.State != Done || a.Sent != 3 || a.Gone != 1 || a.Failed != 1 || a.Finished.IsZero() {
		t.Fatalf("done %+v", a)
	}
	if again, _ := s.Settle(ctx, retryAt.Add(7*time.Minute)); len(again) != 0 {
		t.Fatalf("settled twice: %+v", again)
	}
}

func TestSettleEndsWhatIsOver(t *testing.T) {
	ctx := context.Background()
	s := open(t)
	for i := 1; i <= 3; i++ {
		put(t, s, subscription(i, "de", "exams"), t0)
	}
	never := announce(t, s, "exams", t0, time.Hour)                // never begun
	begun := announce(t, s, "exams", t0, time.Hour)                // begun, not through
	retrying := announce(t, s, "exams", t0, time.Hour)             // through, two attempts wait
	later := announce(t, s, "exams", t0.Add(time.Hour), time.Hour) // not due yet
	if ok, err := s.Record(ctx, begun.ID, 1, []Outcome{{Sub: 1, Result: Sent}}); err != nil || !ok {
		t.Fatal(ok, err)
	}
	if ok, err := s.Record(ctx, retrying.ID, 3, []Outcome{{Sub: 1, Result: Sent}, {Sub: 2, Result: Again, Due: t0.Add(time.Minute)}, {Sub: 3, Result: Again, Due: t0.Add(time.Minute)}}); err != nil || !ok {
		t.Fatal(ok, err)
	}
	if err := s.Walked(ctx, retrying.ID, t0); err != nil {
		t.Fatal(err)
	}
	settled, err := s.Settle(ctx, t0.Add(time.Hour))
	if err != nil || len(settled) != 3 {
		t.Fatalf("settled %+v %v", settled, err)
	}
	for id, want := range map[int64]struct {
		state                string
		sent, failed, cursor int64
	}{never.ID: {Expired, 0, 0, 0}, begun.ID: {Done, 1, 0, 1}, retrying.ID: {Done, 1, 2, 3}} {
		a, _ := s.Announcement(ctx, id)
		if a.State != want.state || a.Sent != want.sent || a.Failed != want.failed || a.Cursor != want.cursor || a.Finished.IsZero() {
			t.Errorf("announcement %d: %+v, want %+v", id, a, want)
		}
	}
	if a, _ := s.Announcement(ctx, later.ID); a.State != Waiting {
		t.Fatalf("the one that is not due: %s", a.State)
	}
	if _, retries, _ := s.Pending(ctx); retries != 0 {
		t.Fatalf("%d attempts still wait", retries)
	}
	if list, _ := s.Retries(ctx, t0.Add(2*time.Hour), 10); len(list) != 0 {
		t.Fatal("an attempt of a finished announcement is due")
	}
	// Recording for one that is finished changes nothing and says so.
	if ok, err := s.Record(ctx, begun.ID, 3, []Outcome{{Sub: 2, Result: Sent}}); err != nil || ok {
		t.Fatalf("record after the end: %v %v", ok, err)
	}
	if a, _ := s.Announcement(ctx, begun.ID); a.Sent != 1 || a.Cursor != 1 {
		t.Fatalf("changed after its end: %+v", a)
	}
}

// A subscription that is gone takes its waiting attempts along, and each announcement that
// waited for it counts it.
func TestGoneSubscriptionLeavesEveryAnnouncement(t *testing.T) {
	ctx := context.Background()
	s := open(t)
	put(t, s, subscription(1, "de", "exams"), t0)
	a := announce(t, s, "exams", t0, time.Hour)
	b := announce(t, s, "exams", t0, time.Hour)
	for _, id := range []int64{a.ID, b.ID} {
		if _, err := s.Record(ctx, id, 1, []Outcome{{Sub: 1, Result: Again, Due: t0.Add(time.Minute)}}); err != nil {
			t.Fatal(err)
		}
		if err := s.Walked(ctx, id, t0); err != nil {
			t.Fatal(err)
		}
	}
	if err := s.Resolve(ctx, a.ID, []Outcome{{Sub: 1, Result: Gone}}); err != nil {
		t.Fatal(err)
	}
	if s.Subscriptions() != 0 {
		t.Fatal("still there")
	}
	if _, retries, _ := s.Pending(ctx); retries != 0 {
		t.Fatalf("%d attempts wait for a subscription that is gone", retries)
	}
	if _, err := s.Settle(ctx, t0.Add(2*time.Minute)); err != nil {
		t.Fatal(err)
	}
	for _, id := range []int64{a.ID, b.ID} {
		if got, _ := s.Announcement(ctx, id); got.State != Done || got.Gone != 1 || got.Sent != 0 || got.Failed != 0 {
			t.Errorf("announcement %d: %+v", id, got)
		}
	}
	// And one the browser itself took back while an attempt waited: the attempt goes with it.
	put(t, s, subscription(2, "de", "exams"), t0)
	c := announce(t, s, "exams", t0, time.Hour)
	if _, err := s.Record(ctx, c.ID, 2, []Outcome{{Sub: 2, Result: Again, Due: t0.Add(time.Minute)}}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Remove(ctx, subscription(2, "", "").Endpoint); err != nil {
		t.Fatal(err)
	}
	if list, _ := s.Retries(ctx, t0.Add(time.Hour), 10); len(list) != 0 {
		t.Fatalf("an attempt for a subscription that left: %+v", list)
	}
}

func TestKeysCancelAndSweep(t *testing.T) {
	ctx := context.Background()
	s := open(t)
	put(t, s, subscription(1, "de", "exams"), t0)
	first, made, err := s.Announce(ctx, Announcement{Key: "exams-2026w", Topic: "exams", SendAt: t0, Until: t0.Add(time.Hour), Urgency: "high", Link: "/x", Texts: map[string]Text{"de": {Title: "Eins"}}}, t0)
	if err != nil || !made || first.Key != "exams-2026w" || first.Urgency != "high" || first.Link != "/x" {
		t.Fatalf("%+v %v %v", first, made, err)
	}
	same, made, err := s.Announce(ctx, Announcement{Key: "exams-2026w", Topic: "breaks", SendAt: t0, Until: t0.Add(time.Hour), Urgency: "low", Link: "/", Texts: map[string]Text{"de": {Title: "Zwei"}}}, t0)
	if err != nil || made || same.ID != first.ID || same.Topic != "exams" || same.Texts["de"].Title != "Eins" {
		t.Fatalf("the key again: %+v %v %v", same, made, err)
	}
	// Without a key, each is its own.
	a, b := announce(t, s, "exams", t0, time.Hour), announce(t, s, "exams", t0, time.Hour)
	if a.ID == b.ID || a.Key != "" {
		t.Fatalf("%+v %+v", a, b)
	}
	if list, err := s.Announcements(ctx, 2); err != nil || len(list) != 2 || list[0].ID != b.ID || list[1].ID != a.ID {
		t.Fatalf("list %+v %v", list, err)
	}

	if _, err := s.Record(ctx, a.ID, 1, []Outcome{{Sub: 1, Result: Again, Due: t0.Add(time.Minute)}}); err != nil {
		t.Fatal(err)
	}
	cancelled, err := s.Cancel(ctx, a.ID, t0.Add(time.Second))
	if err != nil || cancelled.State != Cancelled || cancelled.Finished.IsZero() {
		t.Fatalf("%+v %v", cancelled, err)
	}
	if _, retries, _ := s.Pending(ctx); retries != 0 {
		t.Fatal("the attempt of a cancelled announcement waits")
	}
	if _, err := s.Cancel(ctx, a.ID, t0); !errors.Is(err, ErrFinished) {
		t.Fatalf("cancelled twice: %v", err)
	}
	if _, err := s.Cancel(ctx, 99, t0); !errors.Is(err, ErrNotFound) {
		t.Fatalf("cancel of none: %v", err)
	}
	if _, err := s.Announcement(ctx, 99); !errors.Is(err, ErrNotFound) {
		t.Fatalf("read of none: %v", err)
	}
	if due, _ := s.Due(ctx, t0.Add(time.Minute)); len(due) != 2 {
		t.Fatalf("%d due: the cancelled one is among them, or one is missing", len(due))
	}

	// Finished long ago, it goes; what is not finished stays however old.
	if n, err := s.Sweep(ctx, t0.Add(time.Second)); err != nil || n != 0 {
		t.Fatal(n, err)
	}
	if n, err := s.Sweep(ctx, t0.Add(time.Hour)); err != nil || n != 1 {
		t.Fatal(n, err)
	}
	if list, _ := s.Announcements(ctx, 10); len(list) != 2 {
		t.Fatalf("%d left, want the two that are not finished", len(list))
	}
}

// A database of Xylem 0.1.0, which knew no feed: what it holds was made by a person, at a time
// they named.
func TestDatabaseOfTheFirstXylemIsMigrated(t *testing.T) {
	dir := t.TempDir()
	first, err := migrationFS.ReadFile("migrations/0001_xylem.sql")
	if err != nil {
		t.Fatal(err)
	}
	db, err := sql.Open("sqlite", "file:"+filepath.Join(dir, "xylem.db"))
	if err != nil {
		t.Fatal(err)
	}
	for _, statement := range []string{
		string(first),
		"PRAGMA user_version = 1",
		`INSERT INTO announcements (key, topic, send_at, until, urgency, link, texts, created) VALUES ('exams-2026w', 'exams', 1800000000000, 1800086400000, 'normal', '/', '{"de":{"title":"Prüfungen"}}', 1790000000000)`,
		`INSERT INTO announcements (topic, send_at, until, urgency, link, texts, created, state, sent, finished) VALUES ('breaks', 1790000000000, 1790086400000, 'normal', '/', '{"de":{"title":"Pause"}}', 1790000000000, 'done', 7, 1790000001000)`,
	} {
		if _, err := db.Exec(statement); err != nil {
			t.Fatal(err)
		}
	}
	_ = db.Close()
	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	list, err := s.Announcements(context.Background(), 10)
	if err != nil || len(list) != 2 {
		t.Fatal(list, err)
	}
	for _, a := range list {
		if a.Source != ByCommand || !a.Timed {
			t.Errorf("announcement %d: made by %q, timed %v", a.ID, a.Source, a.Timed)
		}
	}
	if list[1].Key != "exams-2026w" || list[1].State != Waiting || list[0].State != Done || list[0].Sent != 7 {
		t.Fatalf("after the migration: %+v", list)
	}
	// And the feed leaves a person's announcement alone, whatever it names under its key.
	item := FeedItem{Announcement: Announcement{Key: "exams-2026w", Topic: "exams", SendAt: time.UnixMilli(1800000000000), Until: time.UnixMilli(1800000360000),
		Urgency: "normal", Link: "/", Texts: map[string]Text{"de": {Title: "Vom Feed"}}, Timed: true}, Life: time.Hour}
	changes, baseline, err := s.SyncFeed(context.Background(), "https://betula.app/api/announcements", []FeedItem{item}, nil, time.UnixMilli(1790000002000), 4)
	if err != nil || !baseline || len(changes) != 0 {
		t.Fatalf("the feed over a person's key: %+v, baseline %v, %v", changes, baseline, err)
	}
	if a, _ := s.Announcement(context.Background(), 1); a.Texts["de"].Title != "Prüfungen" || a.Source != ByCommand {
		t.Fatalf("changed by the feed: %+v", a)
	}
}

func TestNewerDatabaseIsRefused(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	_ = s.Close()
	db, err := sql.Open("sqlite", "file:"+filepath.Join(dir, "xylem.db"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec("PRAGMA user_version = 99"); err != nil {
		t.Fatal(err)
	}
	_ = db.Close()
	if _, err := Open(dir); err == nil {
		t.Fatal("a database of a newer Xylem was opened")
	}
}
