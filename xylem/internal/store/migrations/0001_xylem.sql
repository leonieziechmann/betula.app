-- Xylem's database (docs/xylem/xylem.md, „Storage"). Times are milliseconds since 1970.

-- 'vapid': Xylem's VAPID key, made once (its private scalar, base64url). Every subscription was
-- made with its public half, so it is as old as the first of them.
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

-- A browser that asked for notifications: where its push service takes messages for it
-- (endpoint), the keys only this browser can open a message with (p256dh, auth), the language
-- its texts are in, and since when. Nothing says whose browser it is: no profile, no address,
-- no account. nr numbers them in the order they came and is never given out again; an
-- announcement walks them by it. id is SHA-256 of the endpoint.
CREATE TABLE subscriptions (
  nr       INTEGER PRIMARY KEY AUTOINCREMENT,
  id       BLOB NOT NULL UNIQUE,
  endpoint TEXT NOT NULL,
  p256dh   BLOB NOT NULL,
  auth     BLOB NOT NULL,
  lang     TEXT NOT NULL,
  created  INTEGER NOT NULL
) STRICT;

-- What a subscription wants to hear of.
CREATE TABLE topics (
  topic TEXT NOT NULL,
  sub   INTEGER NOT NULL REFERENCES subscriptions (nr) ON DELETE CASCADE,
  PRIMARY KEY (topic, sub)
) STRICT, WITHOUT ROWID;

CREATE INDEX topics_sub ON topics (sub);

-- Something to tell everybody who listens to a topic: from send_at, and nobody after until. key
-- is the sender's own name for it, taken once (an announcement made twice is one). texts is JSON,
-- by language: {"de": {"title": …, "body": …, "link": …}}; link the path its notification opens
-- where a language names none. state: waiting (its time has not come), sending (it walks the
-- subscriptions, cursor the nr of the last one handled), retrying (walked; some wait for another
-- attempt), done, cancelled, expired (Xylem did not run while it was due). sent, gone and failed
-- count the subscriptions: written to, found gone at their push service, given up on.
CREATE TABLE announcements (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  key      TEXT UNIQUE,
  topic    TEXT NOT NULL,
  send_at  INTEGER NOT NULL,
  until    INTEGER NOT NULL,
  urgency  TEXT NOT NULL,
  link     TEXT NOT NULL,
  texts    TEXT NOT NULL,
  created  INTEGER NOT NULL,
  state    TEXT NOT NULL DEFAULT 'waiting',
  cursor   INTEGER NOT NULL DEFAULT 0,
  sent     INTEGER NOT NULL DEFAULT 0,
  gone     INTEGER NOT NULL DEFAULT 0,
  failed   INTEGER NOT NULL DEFAULT 0,
  finished INTEGER
) STRICT;

CREATE INDEX announcements_state ON announcements (state, send_at);

-- The subscriptions an announcement could not be written to yet (the push service was busy or
-- did not answer): tried again when due, attempts times so far.
CREATE TABLE retries (
  announcement INTEGER NOT NULL REFERENCES announcements (id) ON DELETE CASCADE,
  sub          INTEGER NOT NULL REFERENCES subscriptions (nr) ON DELETE CASCADE,
  attempts     INTEGER NOT NULL,
  due          INTEGER NOT NULL,
  PRIMARY KEY (announcement, sub)
) STRICT, WITHOUT ROWID;

CREATE INDEX retries_due ON retries (due);
CREATE INDEX retries_sub ON retries (sub);
