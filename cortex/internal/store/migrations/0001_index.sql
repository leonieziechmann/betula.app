-- Cortex's index: the fetched URLs (entry) and every answer that differed from the one before
-- (version), the files stored by name (file, file_version), and the journal a follower replays
-- to keep an identical copy. The bodies are blobs outside the database, named by the sha256 of
-- their original bytes.
--
-- Times are UTC text in the fixed-width form 2006-01-02T15:04:05.000000Z, so that string order
-- is time order. Ids are assigned by the leader and replicated as they are; AUTOINCREMENT keeps
-- an id from being used twice after retention removed the newest rows (a client may keep a
-- version id it was given).
CREATE TABLE meta (
	key   TEXT PRIMARY KEY,   -- epoch, leader_instance, leader_url, leader_since
	value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE entry (
	id              INTEGER PRIMARY KEY AUTOINCREMENT,
	key             TEXT    NOT NULL UNIQUE,      -- GET <url>[\naccept: …][\naccept-language: …]
	url             TEXT    NOT NULL,             -- the normalized URL
	host            TEXT    NOT NULL,
	source          TEXT    NOT NULL,             -- the client's name for it, as last asked
	accept          TEXT    NOT NULL DEFAULT '',
	accept_language TEXT    NOT NULL DEFAULT '',
	created_at      TEXT    NOT NULL,
	current_version INTEGER                       -- version.id; NULL without a version
);

CREATE INDEX idx_entry_host_source ON entry(host, source);

CREATE TABLE version (
	id            INTEGER PRIMARY KEY AUTOINCREMENT,
	entry_id      INTEGER NOT NULL REFERENCES entry(id) ON DELETE CASCADE,
	status        INTEGER NOT NULL,             -- upstream's status: 200, 404, …
	sha256        TEXT    NOT NULL,             -- of the original body: the blob
	size          INTEGER NOT NULL,             -- original bytes
	headers       TEXT    NOT NULL,             -- JSON object of the kept response headers
	fetched_at    TEXT    NOT NULL,             -- the first fetch that returned this content
	checked_at    TEXT    NOT NULL,             -- the last fetch that returned it
	superseded_at TEXT                          -- when the next version replaced it; NULL while current
);

CREATE INDEX idx_version_entry ON version(entry_id);
CREATE INDEX idx_version_sha256 ON version(sha256);
CREATE INDEX idx_version_superseded ON version(superseded_at);

CREATE TABLE file (
	id              INTEGER PRIMARY KEY AUTOINCREMENT,
	name            TEXT    NOT NULL UNIQUE,      -- /-separated, as PUT /v1/files/{name} gave it
	created_at      TEXT    NOT NULL,
	current_version INTEGER                       -- file_version.id, a tombstone after a DELETE
);

CREATE TABLE file_version (
	id            INTEGER PRIMARY KEY AUTOINCREMENT,
	file_id       INTEGER NOT NULL REFERENCES file(id) ON DELETE CASCADE,
	deleted       INTEGER NOT NULL DEFAULT 0,   -- 1: a tombstone (DELETE); sha256 '' and size 0
	sha256        TEXT    NOT NULL,
	size          INTEGER NOT NULL,
	content_type  TEXT    NOT NULL,
	created_at    TEXT    NOT NULL,
	superseded_at TEXT                          -- NULL while current
);

CREATE INDEX idx_file_version_file ON file_version(file_id);
CREATE INDEX idx_file_version_sha256 ON file_version(sha256);
CREATE INDEX idx_file_version_superseded ON file_version(superseded_at);

-- One row per change of the index, in the order the leader made them. payload holds the
-- complete rows the change wrote, so that a follower writes exactly the same; blob names the
-- blob the change needs (the follower fetches it first).
CREATE TABLE journal (
	seq     INTEGER PRIMARY KEY,                -- 1, 2, …; the newest row is never trimmed
	epoch   INTEGER NOT NULL,                   -- the leader's term
	at      TEXT    NOT NULL,
	op      TEXT    NOT NULL,                   -- epoch | version | check | entry_delete | file_version | prune
	payload TEXT    NOT NULL,                   -- JSON
	blob    TEXT    NOT NULL DEFAULT ''         -- sha256 hex, or ''
);

CREATE INDEX idx_journal_at ON journal(at);
