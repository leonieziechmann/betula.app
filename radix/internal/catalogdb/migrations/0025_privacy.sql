-- The register of the people who asked that their personal data be taken out of Betula (DSGVO
-- Art. 17 and 21): module_person, event_person and the instructors of event_date name the
-- teachers of the university, and the module texts and the remarks of dates their e-mail
-- addresses. Every build takes the register's persons out of what it built, last, before the
-- content digest (internal/privacy, docs/radix/privacy.md); validate fails while any is left; the
-- history takes them out of every version it keeps.
--
-- privacy_person  one person: surname and given name (both, so that a namesake keeps their data),
--                 further forms of the name one a line (an e-mail address, „Prof. Mustermann"), a note
--                 where the request came from (a reference, never contact data), when it was added.
-- privacy_meta    build_digest, the register's digest the last build applied, and snapshots_digest,
--                 the one the export last removed the older snapshots for (a snapshot built before a
--                 person was added still names them, and is removed after the first export without them).
--
-- Neither is published: the export drops both (catalogdb.snapshotDropTables). Both travel with
-- radix.db wherever it is copied (a seed, the hand-over of the crawl, the canary's copy), so that
-- no instance builds the catalog without the register.

CREATE TABLE privacy_person (
	id       INTEGER PRIMARY KEY,
	surname  TEXT NOT NULL CHECK (TRIM(surname) <> ''),
	given    TEXT NOT NULL CHECK (TRIM(given) <> ''),
	forms    TEXT,
	note     TEXT,
	added_at TEXT NOT NULL
);

CREATE TABLE privacy_meta (
	key   TEXT PRIMARY KEY,
	value TEXT NOT NULL
) WITHOUT ROWID;
