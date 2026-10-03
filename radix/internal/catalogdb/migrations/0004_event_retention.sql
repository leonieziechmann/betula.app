-- Retention: an event is removed one month after its last date. A module page keeps
-- linking a past event until QIS switches to the next semester, so the crawler has to
-- remember which events it removed on purpose; otherwise it would fetch them again.
CREATE TABLE event_tombstone (
	event_id  TEXT PRIMARY KEY,
	last_date TEXT,              -- last date of the event when it was removed, if it had one
	pruned_at TEXT NOT NULL
) WITHOUT ROWID;
