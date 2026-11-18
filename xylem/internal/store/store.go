// Package store is Xylem's storage (docs/xylem/xylem.md, „Storage"): the browsers that asked
// for notifications with the topics each listens to, the announcements with how far each has
// come, and Xylem's VAPID key, in one SQLite file.
//
// One writer (a single connection, BEGIN IMMEDIATE, synchronous FULL) and a pool of readers, as
// Phloem's store. The number of subscriptions is cached and changes with every write that
// changes it, under the writer's lock.
package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"embed"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	_ "modernc.org/sqlite" // registers the driver "sqlite"
)

//go:embed migrations/*.sql
var migrationFS embed.FS

var (
	// ErrNotFound: no such announcement.
	ErrNotFound = errors.New("not found")
	// ErrFull: as many subscriptions as Xylem may hold.
	ErrFull = errors.New("full")
	// ErrFinished: an announcement that is done, cancelled, expired or skipped cannot be cancelled.
	ErrFinished = errors.New("finished")
	// ErrClosed is returned after Close.
	ErrClosed = errors.New("store closed")
)

// The states of an announcement.
const (
	Waiting   = "waiting"   // its time has not come
	Sending   = "sending"   // it walks the subscriptions
	Retrying  = "retrying"  // walked; some subscriptions wait for another attempt
	Done      = "done"      //
	Cancelled = "cancelled" // by a person: it stays so, whatever the feed names
	Expired   = "expired"   // Xylem did not run while it was due: nobody got it
	Withdrawn = "withdrawn" // it waited, and the feed no longer names it; named again, it waits again
	Skipped   = "skipped"   // it was there when Xylem first read the feed, with no time of its own: old news
)

// Who made an announcement.
const (
	ByCommand = "command" // a person, `xylem announce`
	ByFeed    = "feed"    // Betula itself: the feed of the site
)

// readers is the size of the pool of read connections.
const readers = 4

// Store is one data directory. It is safe for concurrent use.
type Store struct {
	dir string

	// mu guards the handles and the cached count; every write holds it from the transaction's
	// start to the update of the count.
	mu    sync.Mutex
	w     *sql.DB // the one writer
	r     *sql.DB // the readers (query_only)
	shut  bool
	count int64 // subscriptions
}

// Subscription is a browser's push subscription as it arrives: where its push service takes
// messages for it, the keys that seal them, its language and its topics.
type Subscription struct {
	Endpoint string
	P256dh   []byte
	Auth     []byte
	Lang     string
	Topics   []string
}

// Target is a subscription an announcement is written to.
type Target struct {
	Nr       int64
	Endpoint string
	P256dh   []byte
	Auth     []byte
	Lang     string
}

// Text is an announcement in one language. Link is the path its notification opens; empty: the
// announcement's.
type Text struct {
	Title string `json:"title"`
	Body  string `json:"body,omitempty"`
	Link  string `json:"link,omitempty"`
}

// Announcement is something to tell everybody who listens to Topic.
type Announcement struct {
	ID      int64
	Key     string // the sender's own name for it; "": none
	Topic   string
	SendAt  time.Time
	Until   time.Time // nobody gets it after
	Urgency string
	Link    string
	Texts   map[string]Text
	Created time.Time
	Source  string // ByCommand or ByFeed; "" when it is made: ByCommand
	// Timed: its maker named its time. False for one of the feed's that is so from now on, whose
	// time Xylem chose when it first read it.
	Timed bool

	State    string
	Cursor   int64
	Sent     int64
	Gone     int64
	Failed   int64
	Finished time.Time // zero while it is not
}

// Open opens (and creates) dir/xylem.db and applies pending migrations.
// Log events: db.migrated.
func Open(dir string) (*Store, error) {
	if err := os.MkdirAll(dir, 0o750); err != nil {
		return nil, fmt.Errorf("failed to create %s: %w", dir, err)
	}
	s := &Store{dir: dir}
	path := s.path()
	w, err := sql.Open("sqlite", dsn(path)+"&_txlock=immediate")
	if err != nil {
		return nil, fmt.Errorf("failed to open %s: %w", path, err)
	}
	w.SetMaxOpenConns(1)
	w.SetMaxIdleConns(1)
	w.SetConnMaxLifetime(0)
	w.SetConnMaxIdleTime(0)
	if err := migrate(w, path); err != nil {
		_ = w.Close()
		return nil, err
	}
	r, err := sql.Open("sqlite", dsn(path)+"&_pragma=query_only(1)")
	if err != nil {
		_ = w.Close()
		return nil, fmt.Errorf("failed to open %s: %w", path, err)
	}
	r.SetMaxOpenConns(readers)
	r.SetMaxIdleConns(readers)
	if err := r.Ping(); err != nil {
		_ = r.Close()
		_ = w.Close()
		return nil, fmt.Errorf("failed to open %s for reading: %w", path, err)
	}
	if err := w.QueryRow("SELECT COUNT(*) FROM subscriptions").Scan(&s.count); err != nil {
		_ = r.Close()
		_ = w.Close()
		return nil, fmt.Errorf("failed to count the subscriptions of %s: %w", path, err)
	}
	s.w, s.r = w, r
	return s, nil
}

func (s *Store) path() string { return filepath.Join(s.dir, "xylem.db") }

// dsn: busy_timeout, WAL, synchronous FULL and foreign keys on every connection.
func dsn(path string) string {
	return "file:" + path + "?_pragma=busy_timeout(30000)&_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=foreign_keys(ON)"
}

// Close checkpoints the WAL and closes both handles.
func (s *Store) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return nil
	}
	s.shut = true
	var errs []error
	if _, err := s.w.Exec("PRAGMA wal_checkpoint(TRUNCATE)"); err != nil {
		errs = append(errs, fmt.Errorf("failed to checkpoint: %w", err))
	}
	errs = append(errs, s.r.Close(), s.w.Close())
	return errors.Join(errs...)
}

// Ping runs a trivial query, for a liveness check.
func (s *Store) Ping(ctx context.Context) error {
	if s.closed() {
		return ErrClosed
	}
	var n int64
	return s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM meta").Scan(&n)
}

func (s *Store) closed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.shut
}

// Subscriptions is the number of subscriptions kept (cached, exact).
func (s *Store) Subscriptions() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.count
}

// write runs f in one write transaction under mu; f returns the change of the number of
// subscriptions, added to the count once the transaction committed.
func (s *Store) write(ctx context.Context, f func(tx *sql.Tx) (dSubs int64, err error)) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut {
		return ErrClosed
	}
	tx, err := s.w.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	dSubs, err := f(tx)
	if err != nil {
		_ = tx.Rollback()
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	s.count += dSubs
	return nil
}

// VAPID returns the key Xylem keeps, as text; a database that has none yet keeps what make
// returns, and has it from then on.
func (s *Store) VAPID(ctx context.Context, make func() (string, error)) (string, error) {
	var key string
	err := s.write(ctx, func(tx *sql.Tx) (int64, error) {
		err := tx.QueryRowContext(ctx, "SELECT value FROM meta WHERE key = 'vapid'").Scan(&key)
		if err == nil || !errors.Is(err, sql.ErrNoRows) {
			return 0, err
		}
		if key, err = make(); err != nil {
			return 0, err
		}
		_, err = tx.ExecContext(ctx, "INSERT INTO meta (key, value) VALUES ('vapid', ?)", key)
		return 0, err
	})
	return key, err
}

// SubscriptionID is what the store finds a subscription by: SHA-256 of its endpoint.
func SubscriptionID(endpoint string) []byte {
	sum := sha256.Sum256([]byte(endpoint))
	return sum[:]
}

// Put keeps a subscription, or replaces the keys, the language and the topics of the one with
// its endpoint; made says whether it is new. ErrFull when it is new and Xylem holds max already.
func (s *Store) Put(ctx context.Context, sub Subscription, now time.Time, max int64) (made bool, err error) {
	id := SubscriptionID(sub.Endpoint)
	err = s.write(ctx, func(tx *sql.Tx) (int64, error) {
		var nr int64
		err := tx.QueryRowContext(ctx, "SELECT nr FROM subscriptions WHERE id = ?", id).Scan(&nr)
		switch {
		case errors.Is(err, sql.ErrNoRows):
			if s.count >= max { // write holds mu
				return 0, ErrFull
			}
			res, err := tx.ExecContext(ctx, "INSERT INTO subscriptions (id, endpoint, p256dh, auth, lang, created) VALUES (?, ?, ?, ?, ?, ?)",
				id, sub.Endpoint, sub.P256dh, sub.Auth, sub.Lang, now.UnixMilli())
			if err != nil {
				return 0, err
			}
			if nr, err = res.LastInsertId(); err != nil {
				return 0, err
			}
			made = true
		case err != nil:
			return 0, err
		default:
			if _, err := tx.ExecContext(ctx, "UPDATE subscriptions SET p256dh = ?, auth = ?, lang = ? WHERE nr = ?", sub.P256dh, sub.Auth, sub.Lang, nr); err != nil {
				return 0, err
			}
			if _, err := tx.ExecContext(ctx, "DELETE FROM topics WHERE sub = ?", nr); err != nil {
				return 0, err
			}
		}
		for _, topic := range sub.Topics {
			if _, err := tx.ExecContext(ctx, "INSERT OR IGNORE INTO topics (topic, sub) VALUES (?, ?)", topic, nr); err != nil {
				return 0, err
			}
		}
		if made {
			return 1, nil
		}
		return 0, nil
	})
	return made, err
}

// Remove forgets the subscription with this endpoint; found says whether there was one.
func (s *Store) Remove(ctx context.Context, endpoint string) (found bool, err error) {
	err = s.write(ctx, func(tx *sql.Tx) (int64, error) {
		res, err := tx.ExecContext(ctx, "DELETE FROM subscriptions WHERE id = ?", SubscriptionID(endpoint))
		if err != nil {
			return 0, err
		}
		n, err := res.RowsAffected()
		found = n > 0
		return -n, err
	})
	return found, err
}

// Topics reads the topics of the subscription with this endpoint, sorted; found false: none.
func (s *Store) Topics(ctx context.Context, endpoint string) (topics []string, lang string, found bool, err error) {
	var nr int64
	err = s.r.QueryRowContext(ctx, "SELECT nr, lang FROM subscriptions WHERE id = ?", SubscriptionID(endpoint)).Scan(&nr, &lang)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, "", false, nil
	}
	if err != nil {
		return nil, "", false, err
	}
	rows, err := s.r.QueryContext(ctx, "SELECT topic FROM topics WHERE sub = ? ORDER BY topic", nr)
	if err != nil {
		return nil, "", false, err
	}
	defer rows.Close()
	for rows.Next() {
		var topic string
		if err := rows.Scan(&topic); err != nil {
			return nil, "", false, err
		}
		topics = append(topics, topic)
	}
	return topics, lang, true, rows.Err()
}

// Listeners counts the subscriptions by topic and by language.
func (s *Store) Listeners(ctx context.Context) (byTopic, byLang map[string]int64, err error) {
	count := func(query string) (map[string]int64, error) {
		rows, err := s.r.QueryContext(ctx, query)
		if err != nil {
			return nil, err
		}
		defer rows.Close()
		out := map[string]int64{}
		for rows.Next() {
			var name string
			var n int64
			if err := rows.Scan(&name, &n); err != nil {
				return nil, err
			}
			out[name] = n
		}
		return out, rows.Err()
	}
	if byTopic, err = count("SELECT topic, COUNT(*) FROM topics GROUP BY topic"); err != nil {
		return nil, nil, err
	}
	byLang, err = count("SELECT lang, COUNT(*) FROM subscriptions GROUP BY lang")
	return byTopic, byLang, err
}

// Reach counts the subscriptions that listen to topic, by language: who an announcement of it
// would be written to now.
func (s *Store) Reach(ctx context.Context, topic string) (map[string]int64, error) {
	rows, err := s.r.QueryContext(ctx, "SELECT s.lang, COUNT(*) FROM topics t JOIN subscriptions s ON s.nr = t.sub WHERE t.topic = ? GROUP BY s.lang", topic)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[string]int64{}
	for rows.Next() {
		var lang string
		var n int64
		if err := rows.Scan(&lang, &n); err != nil {
			return nil, err
		}
		out[lang] = n
	}
	return out, rows.Err()
}

const announcementColumns = "id, COALESCE(key, ''), topic, send_at, until, urgency, link, texts, created, state, cursor, sent, gone, failed, COALESCE(finished, 0), source, timed"

type scanner interface{ Scan(dest ...any) error }

func scanAnnouncement(row scanner) (Announcement, error) {
	var a Announcement
	var sendAt, until, created, finished int64
	var texts string
	if err := row.Scan(&a.ID, &a.Key, &a.Topic, &sendAt, &until, &a.Urgency, &a.Link, &texts, &created, &a.State, &a.Cursor, &a.Sent, &a.Gone, &a.Failed, &finished, &a.Source, &a.Timed); err != nil {
		return a, err
	}
	a.SendAt, a.Until, a.Created = time.UnixMilli(sendAt), time.UnixMilli(until), time.UnixMilli(created)
	if finished > 0 {
		a.Finished = time.UnixMilli(finished)
	}
	if err := json.Unmarshal([]byte(texts), &a.Texts); err != nil {
		return a, fmt.Errorf("announcement %d: its texts do not read: %w", a.ID, err)
	}
	return a, nil
}

// Announce keeps an announcement a person made, waiting for its time. One with a key that is
// taken already is not kept again: the one that has it comes back, made false.
func (s *Store) Announce(ctx context.Context, a Announcement, now time.Time) (kept Announcement, made bool, err error) {
	texts, err := json.Marshal(a.Texts)
	if err != nil {
		return a, false, err
	}
	err = s.write(ctx, func(tx *sql.Tx) (int64, error) {
		var key any
		if a.Key != "" {
			key = a.Key
			kept, err = scanAnnouncement(tx.QueryRowContext(ctx, "SELECT "+announcementColumns+" FROM announcements WHERE key = ?", a.Key))
			if err == nil || !errors.Is(err, sql.ErrNoRows) {
				return 0, err
			}
		}
		res, err := tx.ExecContext(ctx, "INSERT INTO announcements (key, topic, send_at, until, urgency, link, texts, created) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
			key, a.Topic, a.SendAt.UnixMilli(), a.Until.UnixMilli(), a.Urgency, a.Link, string(texts), now.UnixMilli())
		if err != nil {
			return 0, err
		}
		id, err := res.LastInsertId()
		if err != nil {
			return 0, err
		}
		made = true
		kept, err = scanAnnouncement(tx.QueryRowContext(ctx, "SELECT "+announcementColumns+" FROM announcements WHERE id = ?", id))
		return 0, err
	})
	return kept, made, err
}

// Announcement reads one. ErrNotFound.
func (s *Store) Announcement(ctx context.Context, id int64) (Announcement, error) {
	a, err := scanAnnouncement(s.r.QueryRowContext(ctx, "SELECT "+announcementColumns+" FROM announcements WHERE id = ?", id))
	if errors.Is(err, sql.ErrNoRows) {
		return a, ErrNotFound
	}
	return a, err
}

// Announcements reads the newest limit announcements, the newest first.
func (s *Store) Announcements(ctx context.Context, limit int) ([]Announcement, error) {
	return s.announcements(ctx, "SELECT "+announcementColumns+" FROM announcements ORDER BY id DESC LIMIT ?", limit)
}

// Due reads the announcements whose time has come and that have not walked the subscriptions
// yet, the earliest first.
func (s *Store) Due(ctx context.Context, now time.Time) ([]Announcement, error) {
	return s.announcements(ctx, "SELECT "+announcementColumns+" FROM announcements WHERE state IN ('waiting', 'sending') AND send_at <= ? ORDER BY send_at, id", now.UnixMilli())
}

func (s *Store) announcements(ctx context.Context, query string, args ...any) ([]Announcement, error) {
	rows, err := s.r.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Announcement
	for rows.Next() {
		a, err := scanAnnouncement(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, a)
	}
	return out, rows.Err()
}

// Cancel ends an announcement that is not finished: nobody else gets it. One the feed withdrew
// can be cancelled too, so that it stays away if the feed names it again. ErrNotFound,
// ErrFinished.
func (s *Store) Cancel(ctx context.Context, id int64, now time.Time) (Announcement, error) {
	var a Announcement
	err := s.write(ctx, func(tx *sql.Tx) (int64, error) {
		var state string
		err := tx.QueryRowContext(ctx, "SELECT state FROM announcements WHERE id = ?", id).Scan(&state)
		if errors.Is(err, sql.ErrNoRows) {
			return 0, ErrNotFound
		}
		if err != nil {
			return 0, err
		}
		if state != Waiting && state != Sending && state != Retrying && state != Withdrawn {
			return 0, ErrFinished
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM retries WHERE announcement = ?", id); err != nil {
			return 0, err
		}
		if _, err := tx.ExecContext(ctx, "UPDATE announcements SET state = 'cancelled', finished = ? WHERE id = ?", now.UnixMilli(), id); err != nil {
			return 0, err
		}
		a, err = scanAnnouncement(tx.QueryRowContext(ctx, "SELECT "+announcementColumns+" FROM announcements WHERE id = ?", id))
		return 0, err
	})
	return a, err
}

// Batch reads the next limit subscriptions an announcement is written to: those that listen to
// its topic, after its cursor, and that were there when its time came (who subscribes later
// does not get what was said before).
func (s *Store) Batch(ctx context.Context, a Announcement, limit int) ([]Target, error) {
	rows, err := s.r.QueryContext(ctx, `SELECT s.nr, s.endpoint, s.p256dh, s.auth, s.lang FROM topics t JOIN subscriptions s ON s.nr = t.sub
		WHERE t.topic = ? AND t.sub > ? AND s.created <= ? ORDER BY t.sub LIMIT ?`, a.Topic, a.Cursor, a.SendAt.UnixMilli(), limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Target
	for rows.Next() {
		var t Target
		if err := rows.Scan(&t.Nr, &t.Endpoint, &t.P256dh, &t.Auth, &t.Lang); err != nil {
			return nil, err
		}
		out = append(out, t)
	}
	return out, rows.Err()
}

// What became of one attempt to write an announcement to a subscription.
const (
	Sent   = iota // the push service took it
	Gone          // the push service says the subscription is no more: it is forgotten
	Again         // tried again at Outcome.Due
	Failed        // given up on, for this announcement
)

// Outcome is what became of an attempt.
type Outcome struct {
	Sub    int64
	Result int
	Due    time.Time // Again
}

// Record keeps what became of a batch and moves the announcement's cursor behind it, in one
// step: after a crash, a batch is written again whole or not at all. An announcement cancelled
// meanwhile stays cancelled; ok false says so.
func (s *Store) Record(ctx context.Context, id, cursor int64, outcomes []Outcome) (ok bool, err error) {
	err = s.write(ctx, func(tx *sql.Tx) (int64, error) {
		var state string
		if err := tx.QueryRowContext(ctx, "SELECT state FROM announcements WHERE id = ?", id).Scan(&state); err != nil {
			return 0, err
		}
		if state != Waiting && state != Sending {
			return 0, nil
		}
		ok = true
		var sent, gone, failed, removed int64
		for _, o := range outcomes {
			switch o.Result {
			case Sent:
				sent++
			case Gone:
				n, err := forget(ctx, tx, o.Sub, id)
				if err != nil {
					return 0, err
				}
				removed += n
				gone++
			case Again:
				if _, err := tx.ExecContext(ctx, "INSERT OR REPLACE INTO retries (announcement, sub, attempts, due) VALUES (?, ?, 1, ?)", id, o.Sub, o.Due.UnixMilli()); err != nil {
					return 0, err
				}
			default:
				failed++
			}
		}
		_, err := tx.ExecContext(ctx, "UPDATE announcements SET state = 'sending', cursor = MAX(cursor, ?), sent = sent + ?, gone = gone + ?, failed = failed + ? WHERE id = ?",
			cursor, sent, gone, failed, id)
		return -removed, err
	})
	return ok, err
}

// forget deletes a subscription its push service says is gone, and counts it as gone for every
// other announcement that still waited to be written to it. It returns how many subscriptions it
// deleted (0 when it was gone already).
func forget(ctx context.Context, tx *sql.Tx, sub, except int64) (int64, error) {
	if _, err := tx.ExecContext(ctx, "UPDATE announcements SET gone = gone + 1 WHERE id != ? AND id IN (SELECT announcement FROM retries WHERE sub = ?)", except, sub); err != nil {
		return 0, err
	}
	res, err := tx.ExecContext(ctx, "DELETE FROM subscriptions WHERE nr = ?", sub)
	if err != nil {
		return 0, err
	}
	return res.RowsAffected()
}

// Walked says that an announcement has been through every subscription: it is done, or
// retrying while some wait for another attempt.
func (s *Store) Walked(ctx context.Context, id int64, now time.Time) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, error) {
		_, err := tx.ExecContext(ctx, `UPDATE announcements SET
			state = CASE WHEN EXISTS (SELECT 1 FROM retries WHERE announcement = announcements.id) THEN 'retrying' ELSE 'done' END,
			finished = CASE WHEN EXISTS (SELECT 1 FROM retries WHERE announcement = announcements.id) THEN NULL ELSE ? END
			WHERE id = ? AND state IN ('waiting', 'sending')`, now.UnixMilli(), id)
		return 0, err
	})
}

// Retry is a subscription that waits for another attempt of an announcement.
type Retry struct {
	Announcement Announcement
	Target       Target
	Attempts     int
}

// Retries reads the attempts that are due, the earliest first.
func (s *Store) Retries(ctx context.Context, now time.Time, limit int) ([]Retry, error) {
	rows, err := s.r.QueryContext(ctx, `SELECT r.announcement, r.attempts, s.nr, s.endpoint, s.p256dh, s.auth, s.lang
		FROM retries r JOIN subscriptions s ON s.nr = r.sub WHERE r.due <= ? ORDER BY r.due, r.announcement, r.sub LIMIT ?`, now.UnixMilli(), limit)
	if err != nil {
		return nil, err
	}
	var out []Retry
	var ids []int64
	for rows.Next() {
		var r Retry
		var id int64
		if err := rows.Scan(&id, &r.Attempts, &r.Target.Nr, &r.Target.Endpoint, &r.Target.P256dh, &r.Target.Auth, &r.Target.Lang); err != nil {
			_ = rows.Close()
			return nil, err
		}
		out = append(out, r)
		ids = append(ids, id)
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return nil, err
	}
	known := map[int64]Announcement{}
	for i, id := range ids {
		a, ok := known[id]
		if !ok {
			if a, err = s.Announcement(ctx, id); err != nil {
				return nil, err
			}
			known[id] = a
		}
		out[i].Announcement = a
	}
	return out, nil
}

// Resolve keeps what became of the attempts of Retries: an attempt that is to be made again
// counts one more, the others no longer wait.
func (s *Store) Resolve(ctx context.Context, announcement int64, outcomes []Outcome) error {
	return s.write(ctx, func(tx *sql.Tx) (int64, error) {
		var sent, gone, failed, removed int64
		for _, o := range outcomes {
			// Only what still waits: a subscription that left meanwhile, or an announcement
			// cancelled, took its row along.
			var attempts int
			err := tx.QueryRowContext(ctx, "SELECT attempts FROM retries WHERE announcement = ? AND sub = ?", announcement, o.Sub).Scan(&attempts)
			if errors.Is(err, sql.ErrNoRows) {
				continue
			}
			if err != nil {
				return 0, err
			}
			switch o.Result {
			case Again:
				if _, err := tx.ExecContext(ctx, "UPDATE retries SET attempts = attempts + 1, due = ? WHERE announcement = ? AND sub = ?", o.Due.UnixMilli(), announcement, o.Sub); err != nil {
					return 0, err
				}
				continue
			case Gone:
				n, err := forget(ctx, tx, o.Sub, announcement)
				if err != nil {
					return 0, err
				}
				removed += n
				gone++
			case Sent:
				sent++
			default:
				failed++
			}
			if _, err := tx.ExecContext(ctx, "DELETE FROM retries WHERE announcement = ? AND sub = ?", announcement, o.Sub); err != nil {
				return 0, err
			}
		}
		_, err := tx.ExecContext(ctx, "UPDATE announcements SET sent = sent + ?, gone = gone + ?, failed = failed + ? WHERE id = ?", sent, gone, failed, announcement)
		return -removed, err
	})
}

// Settled is an announcement Settle finished.
type Settled struct {
	ID      int64
	State   string
	Dropped int64 // the attempts that still waited when its time ran out
}

// Settle finishes what has nothing left to do: an announcement whose time ran out before it was
// through (expired when nobody was written to yet, else done, and what still waited for another
// attempt counts as failed), and one that only waited for attempts none of which is left.
func (s *Store) Settle(ctx context.Context, now time.Time) ([]Settled, error) {
	var out []Settled
	err := s.write(ctx, func(tx *sql.Tx) (int64, error) {
		out = out[:0]
		rows, err := tx.QueryContext(ctx, `SELECT id, state, until <= ?, cursor, (SELECT COUNT(*) FROM retries WHERE announcement = announcements.id)
			FROM announcements WHERE state IN ('waiting', 'sending', 'retrying') AND (until <= ? OR state = 'retrying') ORDER BY id`, now.UnixMilli(), now.UnixMilli())
		if err != nil {
			return 0, err
		}
		type row struct {
			id, cursor, waiting int64
			state               string
			over                bool
		}
		var all []row
		for rows.Next() {
			var r row
			if err := rows.Scan(&r.id, &r.state, &r.over, &r.cursor, &r.waiting); err != nil {
				_ = rows.Close()
				return 0, err
			}
			all = append(all, r)
		}
		if err := errors.Join(rows.Err(), rows.Close()); err != nil {
			return 0, err
		}
		for _, r := range all {
			state := Done
			switch {
			case r.over && r.state == Waiting && r.cursor == 0:
				state = Expired
			case !r.over && r.waiting > 0:
				continue // retrying, and attempts are left
			}
			var dropped int64
			if r.over {
				dropped = r.waiting
				if _, err := tx.ExecContext(ctx, "DELETE FROM retries WHERE announcement = ?", r.id); err != nil {
					return 0, err
				}
			}
			if _, err := tx.ExecContext(ctx, "UPDATE announcements SET state = ?, failed = failed + ?, finished = ? WHERE id = ?", state, dropped, now.UnixMilli(), r.id); err != nil {
				return 0, err
			}
			out = append(out, Settled{ID: r.id, State: state, Dropped: dropped})
		}
		return 0, nil
	})
	return out, err
}

// Pending counts what is not finished: the announcements by state (waiting, sending, retrying)
// and the attempts that wait.
func (s *Store) Pending(ctx context.Context) (byState map[string]int64, retries int64, err error) {
	rows, err := s.r.QueryContext(ctx, "SELECT state, COUNT(*) FROM announcements WHERE state IN ('waiting', 'sending', 'retrying') GROUP BY state")
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()
	byState = map[string]int64{}
	for rows.Next() {
		var state string
		var n int64
		if err := rows.Scan(&state, &n); err != nil {
			return nil, 0, err
		}
		byState[state] = n
	}
	if err := rows.Err(); err != nil {
		return nil, 0, err
	}
	err = s.r.QueryRowContext(ctx, "SELECT COUNT(*) FROM retries").Scan(&retries)
	return byState, retries, err
}

// Next is when the next announcement that waits is due; ok false: none waits.
func (s *Store) Next(ctx context.Context) (at time.Time, ok bool, err error) {
	var ms sql.NullInt64
	err = s.r.QueryRowContext(ctx, "SELECT MIN(send_at) FROM announcements WHERE state = 'waiting'").Scan(&ms)
	if err != nil || !ms.Valid {
		return time.Time{}, false, err
	}
	return time.UnixMilli(ms.Int64), true, nil
}

// Sweep deletes the announcements finished before a time, and returns how many.
func (s *Store) Sweep(ctx context.Context, before time.Time) (int64, error) {
	var n int64
	err := s.write(ctx, func(tx *sql.Tx) (int64, error) {
		res, err := tx.ExecContext(ctx, "DELETE FROM announcements WHERE finished IS NOT NULL AND finished < ?", before.UnixMilli())
		if err != nil {
			return 0, err
		}
		n, err = res.RowsAffected()
		return 0, err
	})
	return n, err
}

type migration struct {
	version int
	name    string
	sql     string
}

// loadMigrations reads migrations/NNNN_name.sql in version order; a gap or a duplicate number is
// an error, since the version is the only record of the database's shape.
func loadMigrations() ([]migration, error) {
	entries, err := fs.ReadDir(migrationFS, "migrations")
	if err != nil {
		return nil, err
	}
	var result []migration
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || !strings.HasSuffix(name, ".sql") {
			continue
		}
		prefix, _, ok := strings.Cut(name, "_")
		if !ok {
			return nil, fmt.Errorf("migration %q: expected NNNN_name.sql", name)
		}
		version, err := strconv.Atoi(prefix)
		if err != nil || version <= 0 {
			return nil, fmt.Errorf("migration %q: invalid version prefix", name)
		}
		body, err := migrationFS.ReadFile("migrations/" + name)
		if err != nil {
			return nil, err
		}
		result = append(result, migration{version: version, name: name, sql: string(body)})
	}
	sort.Slice(result, func(i, j int) bool { return result[i].version < result[j].version })
	for i, m := range result {
		if m.version != i+1 {
			return nil, fmt.Errorf("migration %q: expected version %d", m.name, i+1)
		}
	}
	return result, nil
}

// migrate applies the pending migrations, each in its own transaction, and refuses a database
// that a newer binary migrated further. Log events: db.migrated.
func migrate(db *sql.DB, path string) error {
	migrations, err := loadMigrations()
	if err != nil {
		return fmt.Errorf("failed to load migrations: %w", err)
	}
	var current int
	if err := db.QueryRow("PRAGMA user_version").Scan(&current); err != nil {
		return fmt.Errorf("failed to read the schema version of %s: %w", path, err)
	}
	if current > len(migrations) {
		return fmt.Errorf("database %s has schema version %d, this binary only knows %d", path, current, len(migrations))
	}
	for _, m := range migrations[current:] {
		tx, err := db.Begin()
		if err != nil {
			return err
		}
		if _, err := tx.Exec(m.sql); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s failed: %w", m.name, err)
		}
		if _, err := tx.Exec(fmt.Sprintf("PRAGMA user_version = %d", m.version)); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s: failed to set version: %w", m.name, err)
		}
		if err := tx.Commit(); err != nil {
			return fmt.Errorf("migration %s: commit failed: %w", m.name, err)
		}
		oplog.For("store").Info("migration applied", "event", "db.migrated", "migration", m.name, "schema_version", m.version, "db", path)
	}
	return nil
}
