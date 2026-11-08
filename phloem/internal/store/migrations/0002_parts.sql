-- Step 3 of the protocol (docs/phloem/protocol.md, since Phloem 0.2.0): the parts of shared
-- calendars, and what became of the parcels and parts that are no longer there. Times are
-- milliseconds since 1970.

-- A part of a shared calendar, sealed with a key of its own that Phloem never sees: anyone with
-- its id may read it, only its space may change it. rev is its version (1 when it was made, one
-- more with every replacement), at the time of its last one.
CREATE TABLE parts (
  id    BLOB PRIMARY KEY,
  space BLOB NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
  rev   INTEGER NOT NULL,
  data  BLOB NOT NULL,
  at    INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX parts_space ON parts (space);

-- The parts deleted, or whose space was, since at: such an id is never a part again, so that a
-- link to it, which others may hold, never shows another calendar. Of no space, since it
-- outlives its space; forgotten 395 days after.
CREATE TABLE part_tombstones (
  id BLOB PRIMARY KEY,
  at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX part_tombstones_at ON part_tombstones (at);

-- What became of a parcel that is no longer there, and when, for 30 days; deleted with its
-- space.
CREATE TABLE pair_tombstones (
  id    BLOB PRIMARY KEY,
  space BLOB NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK (state IN ('taken', 'expired', 'withdrawn')),
  at    INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX pair_tombstones_space ON pair_tombstones (space, at);
CREATE INDEX pair_tombstones_at ON pair_tombstones (at);
