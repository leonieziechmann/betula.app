-- Announcements Betula makes by itself (docs/xylem/xylem.md, „The feed"): Xylem reads them from
-- a feed of the site and keeps them as its own.
--
-- source: who made it, 'command' (a person, `xylem announce`) or 'feed'. timed: whether its
-- maker named its time (1), or Xylem chose it when it first read it (0: something that is so
-- from now on, like a semester's dates being there, told at the next hour of the day).
--
-- Two more states, both the feed's: 'withdrawn' (it waited, and the feed no longer names it:
-- nobody is told; named again, it waits again) and 'skipped' (it was there when Xylem first read
-- the feed, with no time of its own: old news, never told).
ALTER TABLE announcements ADD COLUMN source TEXT NOT NULL DEFAULT 'command';
ALTER TABLE announcements ADD COLUMN timed INTEGER NOT NULL DEFAULT 1;
