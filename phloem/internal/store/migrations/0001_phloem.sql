-- Phloem's index (docs/phloem/phloem.md, „Storage"). Everything a space holds is sealed by the
-- devices; Phloem keeps it as it came and never opens it. Times are milliseconds since 1970.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

-- A profile that syncs. token_hash is SHA-256 of its token. head is the number of its newest
-- entry (0: none yet), snap_upto what its snapshot covers (0 also when it has none: the table
-- snapshots says whether it has one), tail_bytes the data of its entries after the snapshot,
-- bytes everything it holds (entries, snapshot, parcels, with a little for every row), for the
-- budget.
CREATE TABLE spaces (
  id         BLOB PRIMARY KEY,
  token_hash BLOB NOT NULL,
  created    INTEGER NOT NULL,
  touched    INTEGER NOT NULL,
  head       INTEGER NOT NULL DEFAULT 0,
  snap_upto  INTEGER NOT NULL DEFAULT 0,
  tail_bytes INTEGER NOT NULL DEFAULT 0,
  bytes      INTEGER NOT NULL DEFAULT 0
) STRICT, WITHOUT ROWID;

CREATE INDEX spaces_touched ON spaces (touched);

-- The devices of a space, by their own id, each with its slot (1–16) for as long as it stays.
CREATE TABLE devices (
  space  BLOB NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
  device BLOB NOT NULL,
  slot   INTEGER NOT NULL,
  seen   INTEGER NOT NULL,
  PRIMARY KEY (space, device),
  UNIQUE (space, slot)
) STRICT, WITHOUT ROWID;

-- The sealed batches of a space after its snapshot, numbered in the order they came. pid is the
-- device's own id of the push: a push sent again (its answer was lost) finds its entry by it.
CREATE TABLE entries (
  space BLOB NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
  nr    INTEGER NOT NULL,
  slot  INTEGER NOT NULL,
  pid   TEXT NOT NULL,
  data  BLOB NOT NULL,
  PRIMARY KEY (space, nr),
  UNIQUE (space, slot, pid)
) STRICT, WITHOUT ROWID;

-- The newest snapshot of a space: every record as of entry upto.
CREATE TABLE snapshots (
  space BLOB PRIMARY KEY REFERENCES spaces (id) ON DELETE CASCADE,
  upto  INTEGER NOT NULL,
  data  BLOB NOT NULL,
  at    INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Parcels for a device to come: fetched once, or gone when they expire.
CREATE TABLE pairs (
  id      BLOB PRIMARY KEY,
  space   BLOB NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
  box     BLOB NOT NULL,
  expires INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX pairs_expires ON pairs (expires);
CREATE INDEX pairs_space ON pairs (space);
