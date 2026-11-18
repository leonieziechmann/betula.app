package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"time"
)

// FeedItem is an announcement as the feed names it now. Key is never empty.
type FeedItem struct {
	Announcement
	// Life is how long it is told: for one without a time of its own, Until follows the time
	// Xylem keeps for it.
	Life time.Duration
}

// What SyncFeed did with an item of the feed, or with an announcement the feed no longer names.
const (
	FeedMade      = "made"      // new: it waits for its time
	FeedRevised   = "revised"   // it waited, and the feed names it otherwise now
	FeedRevived   = "revived"   // it was withdrawn, and the feed names it again
	FeedWithdrawn = "withdrawn" // it waited, and the feed no longer names it
	FeedSkipped   = "skipped"   // the first read: it has no time of its own, so it is old news
	FeedCapped    = "capped"    // too many of the feed's within a day: not taken (now)
)

// FeedChange is one thing SyncFeed did.
type FeedChange struct {
	Key  string
	ID   int64 // 0: capped
	What string
	At   time.Time // when it is told (made, revised, revived)
}

// SyncFeed brings the announcements the feed made in line with what the feed names now, in one
// step:
//
//   - an item whose key Xylem does not know is kept and waits (FeedMade), unless perDay of the
//     feed's are told within a day of it already (FeedCapped: a feed that went wrong must not
//     reach every phone again and again);
//   - one that waits and is named otherwise now (its time, its texts) is changed (FeedRevised);
//     one without a time of its own keeps the time Xylem chose for it;
//   - one the feed withdrew and names again waits again (FeedRevived);
//   - what is being told, was told, or a person cancelled stays as it is, and so does an
//     announcement a person made under the same key;
//   - an announcement of the feed's that waits and is no longer named is withdrawn
//     (FeedWithdrawn), unless its key is in keep (named, but not readable this time).
//
// The first read of a feed (another one than the last, by its address) is the baseline: an item
// without a time of its own is something that is so already, nobody knows since when, and is
// kept as skipped instead of being told (FeedSkipped).
func (s *Store) SyncFeed(ctx context.Context, feed string, items []FeedItem, keep map[string]bool, now time.Time, perDay int) (changes []FeedChange, baseline bool, err error) {
	err = s.write(ctx, func(tx *sql.Tx) (int64, error) {
		changes, baseline = changes[:0], false
		var known string
		switch err := tx.QueryRowContext(ctx, "SELECT value FROM meta WHERE key = 'feed'").Scan(&known); {
		case errors.Is(err, sql.ErrNoRows):
			baseline = true
		case err != nil:
			return 0, err
		default:
			baseline = known != feed
		}
		named := map[string]bool{}
		for _, item := range items {
			named[item.Key] = true
			texts, err := json.Marshal(item.Texts)
			if err != nil {
				return 0, err
			}
			var (
				id                    int64
				state, source, was    string
				topic, urgency, link  string
				sendAt, until, timedI int64
			)
			err = tx.QueryRowContext(ctx, "SELECT id, state, source, topic, send_at, until, urgency, link, texts, timed FROM announcements WHERE key = ?", item.Key).
				Scan(&id, &state, &source, &topic, &sendAt, &until, &urgency, &link, &was, &timedI)
			timed := 0
			if item.Timed {
				timed = 1
			}
			switch {
			case errors.Is(err, sql.ErrNoRows):
				if baseline && !item.Timed {
					res, err := tx.ExecContext(ctx, `INSERT INTO announcements (key, topic, send_at, until, urgency, link, texts, created, source, timed, state, finished)
						VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'feed', 0, 'skipped', ?)`,
						item.Key, item.Topic, item.SendAt.UnixMilli(), item.Until.UnixMilli(), item.Urgency, item.Link, string(texts), now.UnixMilli(), now.UnixMilli())
					if err != nil {
						return 0, err
					}
					id, _ = res.LastInsertId()
					changes = append(changes, FeedChange{Key: item.Key, ID: id, What: FeedSkipped})
					continue
				}
				capped, err := feedCapped(ctx, tx, item.SendAt, perDay)
				if err != nil {
					return 0, err
				}
				if capped {
					changes = append(changes, FeedChange{Key: item.Key, What: FeedCapped, At: item.SendAt})
					continue
				}
				res, err := tx.ExecContext(ctx, `INSERT INTO announcements (key, topic, send_at, until, urgency, link, texts, created, source, timed)
					VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'feed', ?)`,
					item.Key, item.Topic, item.SendAt.UnixMilli(), item.Until.UnixMilli(), item.Urgency, item.Link, string(texts), now.UnixMilli(), timed)
				if err != nil {
					return 0, err
				}
				id, _ = res.LastInsertId()
				changes = append(changes, FeedChange{Key: item.Key, ID: id, What: FeedMade, At: item.SendAt})
			case err != nil:
				return 0, err
			case source != ByFeed:
				// A person's, under the same key: theirs.
			case state == Waiting:
				at, end := item.SendAt, item.Until
				if !item.Timed && timedI == 0 {
					// So from now on, and Xylem chose its time when it first read it: that
					// time stands, whenever it is read again.
					at = time.UnixMilli(sendAt)
					end = at.Add(item.Life)
				}
				if topic == item.Topic && sendAt == at.UnixMilli() && until == end.UnixMilli() && urgency == item.Urgency && link == item.Link && was == string(texts) && timedI == int64(timed) {
					continue
				}
				if _, err := tx.ExecContext(ctx, "UPDATE announcements SET topic = ?, send_at = ?, until = ?, urgency = ?, link = ?, texts = ?, timed = ? WHERE id = ?",
					item.Topic, at.UnixMilli(), end.UnixMilli(), item.Urgency, item.Link, string(texts), timed, id); err != nil {
					return 0, err
				}
				changes = append(changes, FeedChange{Key: item.Key, ID: id, What: FeedRevised, At: at})
			case state == Withdrawn:
				if _, err := tx.ExecContext(ctx, "UPDATE announcements SET state = 'waiting', finished = NULL, topic = ?, send_at = ?, until = ?, urgency = ?, link = ?, texts = ?, timed = ? WHERE id = ?",
					item.Topic, item.SendAt.UnixMilli(), item.Until.UnixMilli(), item.Urgency, item.Link, string(texts), timed, id); err != nil {
					return 0, err
				}
				changes = append(changes, FeedChange{Key: item.Key, ID: id, What: FeedRevived, At: item.SendAt})
			}
		}
		// What the feed made, what still waits, and what it no longer names.
		rows, err := tx.QueryContext(ctx, "SELECT id, key FROM announcements WHERE source = 'feed' AND state = 'waiting' AND key IS NOT NULL ORDER BY id")
		if err != nil {
			return 0, err
		}
		var gone []FeedChange
		for rows.Next() {
			var c FeedChange
			if err := rows.Scan(&c.ID, &c.Key); err != nil {
				_ = rows.Close()
				return 0, err
			}
			if !named[c.Key] && !keep[c.Key] {
				c.What = FeedWithdrawn
				gone = append(gone, c)
			}
		}
		if err := errors.Join(rows.Err(), rows.Close()); err != nil {
			return 0, err
		}
		for _, c := range gone {
			if _, err := tx.ExecContext(ctx, "UPDATE announcements SET state = 'withdrawn', finished = ? WHERE id = ?", now.UnixMilli(), c.ID); err != nil {
				return 0, err
			}
		}
		changes = append(changes, gone...)
		_, err = tx.ExecContext(ctx, "INSERT INTO meta (key, value) VALUES ('feed', ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value", feed)
		return 0, err
	})
	return changes, baseline, err
}

// feedCapped says whether perDay announcements of the feed's are told, or were, within a day
// of at.
func feedCapped(ctx context.Context, tx *sql.Tx, at time.Time, perDay int) (bool, error) {
	var n int
	day := (24 * time.Hour).Milliseconds()
	err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM announcements WHERE source = 'feed'
		AND state IN ('waiting', 'sending', 'retrying', 'done', 'expired') AND send_at > ? AND send_at < ?`,
		at.UnixMilli()-day, at.UnixMilli()+day).Scan(&n)
	return n >= perDay, err
}
