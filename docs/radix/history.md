# The history of the published catalog

Radix keeps every state of the catalog it published, in a database of its own beside `radix.db`:
`history.db` (owner, 2026-11-19: „am liebsten ja wirklich alles. Aber bitte so umgesetzt, dass sich
nur ‚Änderungen' gespeichert werden … Zum Beispiel Moduldaten gültig ab. Was ich möchte ist, dass es
rechnerisch schnell geht, den Stand an einem bestimmten Zeitpunkt zu rekonstruieren … So dass wir das
kontinuierlich mitschreiben"). The snapshot browsers get stays as it was: the history is never in it
(„Die ausgelieferte DB soll aber erstmal so bleiben"). Package `radix/internal/history`; the
privacy register reaches it (`docs/radix/privacy.md`).

## 1. What it keeps

Everything a snapshot holds: every table, every row, every column, the schema. Before it, the
catalog forgot: a build holds only the current dataset, an event goes 30 days after its last date
(`--event-retention`), the export keeps two snapshots, and Cortex keeps the raw pages that changed
for 180 days. The history keeps the end of every semester as it was published (an event leaves the
catalog only once its semester is over, so its last version is its final one), the Termine that
changed during it, and every module, program and plan as it stood on any day since the history began.

## 2. How it is stored

One copy of each table of the snapshot, `h_<table>`, with three columns before the table's own:

| Column | What it holds |
|---|---|
| `_from` | the state (`history_snapshot.seq`) the row appeared in |
| `_to` | the first state it was no longer in; NULL while it is in the newest |
| `_hash` | a hash of the row's values, by which the next snapshot finds it again |

A row is stored once for as long as it stays the same: a module whose text changed is a new row,
valid from the state that brought it, and its old row ends there. A new snapshot costs the rows that
changed, nothing else. No diffs, no chain to replay: the state at a time is the rows whose span holds
it, `_from <= s AND (_to IS NULL OR _to > s)`, one pass over each table.

- **The hash** is over every column but `fetched_at` (the content digest of catalogbuild leaves it out
  for the same reason: a page fetched again without a change is no change), by column name, NULL left
  out: a column a migration adds, empty in a row, changes the row's hash nowhere. A version keeps the
  `fetched_at` it was first recorded with.
- **The build's row numbers are not kept.** An INTEGER PRIMARY KEY that no foreign key refers to
  (`event_date.id`, `plan_entry.id`, `program_module_assertion.id`) is the order the build wrote the
  rows in: one event written earlier renumbers every date after it. No reader reads them (no read
  view has them); `At` numbers the rows anew. `program_area.id` and `department.id`, which foreign
  keys name, are kept.
- **Rows are a multiset**, matched by their hash: two equal rows are two rows.
- **The schema grows with the snapshots.** A column a migration adds is added to the copy (rows before
  it have NULL there), a table a migration adds gets a copy, a table that went keeps its copy with
  every row ended. Each state names its schema (`history_schema`: the snapshot's `sqlite_master` and
  `user_version`, stored once per schema), so a state is rebuilt with the tables, indexes and views
  it was published with.

The history's own tables:

| Table | One row per |
|---|---|
| `history_snapshot` | recorded state: `seq`, `valid_from` (since when it was the published one: when it was exported, `current.json` `exported_at`), `recorded_at`, `content_digest`, `radix_version`, `schema_id`, `source` (the file), `rows_opened`, `rows_closed` |
| `history_schema` | schema: `user_version`, `ddl` (JSON of `sqlite_master`), its digest |
| `history_table` | table of the snapshots: its copy (`store`) and its columns with type and key |
| `history_meta` | `format` (1), `privacy_digest` (the register last applied, `docs/radix/privacy.md`), `register_id` (the register it follows; another is refused) |

A snapshot whose content digest is the newest state's records nothing. A state is valid from when it
was exported (not from `data_changed_at`, the build that first made the content: a cycle whose
validation or export failed publishes it later); a file recorded without its `current.json` from its
`data_changed_at`. One published again after a newer one (a rollback to an older release) is valid
from the time it is recorded, as is one published within the second of the one before.

## 3. Numbers (2026-11-19)

Measured on the two snapshots of that morning, betula.app's (Radix 0.10.0, schema 17) and canary's
(Radix 0.14.1, schema 24), recorded in that order, then each state rebuilt and compared with the
original, table by table (`TestRealSnapshots`):

| | |
|---|---|
| first state | 252,934 rows of 39 tables in 3.7 s; history 57.2 MB (the snapshot: 46.1 MB) |
| second state, a release later | 44,993 rows opened, 3,334 ended, in 1.9 s; history 62.7 MB. The FÜS catalogs and the documents the release brought, the FÜS lists they changed (program_module +15,563), the prerequisites with their new column; modules and events: none, the same crawl |
| `At` of either state | 2.0 s and 2.5 s; every table equal to the original |

What a day of the semester adds is what changed: some 30 events a night with news (`docs/radix/data-sources.md` §12),
a few module descriptions, at about 1.3 KB an event and 3 KB a module. A semester of changed Termine is
some 10–25 MB, every table of the catalog a few dozen MB a year.

## 4. Commands

```bash
radix history status                                   # states, rows, size, the register applied
radix history record --snapshot snapshot/              # add the current snapshot (a file works too)
radix history at --time 2026-11-19 --out then.db       # the catalog as published at the end of that day
radix history log module id=11881                      # the versions of a module's row, what changed
radix history log event_date event_id=151296           # the dates of an event, every version
radix history export --out betula-history.db           # the full export: a copy of the whole history
radix history redact                                   # apply the privacy register of --db to every version
```

Each takes `--history` (env `RADIX_HISTORY_DB`, default `history.db`) and `--db`, the database whose
privacy register applies; a `--db` that does not exist is refused, never made (a fresh one has an
empty register), and so is another radix.db than the one whose register the history follows.
`record` refuses a time before the newest state (`--valid-from`, for snapshots kept elsewhere and
recorded later, oldest first), and without a register once one was applied (`--no-register` says
there is no radix.db at hand). `at` writes a snapshot of that schema
that the Folia of that time reads as it read the original.

The full export is SQLite: `history_snapshot` says when each state was published, and the rows of a
table at state *s* are `SELECT * FROM h_module WHERE _from <= s AND (_to IS NULL OR _to > s)`; joined
with `history_snapshot` on `_from` and `_to`, the span of a row is two times.

## 5. In the service

`radix run --history <path>` (env `RADIX_HISTORY_DB`; stage `history`, every cycle that did not fail,
after the export or in its place, before the semantic stage): it applies the privacy register to every
version when the register changed since it was last applied (`history.redacted`), and records the
published snapshot when the newest state is not it (`history.recorded`: `state`, `valid_from`,
`rows_opened`, `rows_closed`, `schema`). A snapshot published while the stage failed is recorded by the
next cycle, before the one that replaced it, while its file is kept (the export keeps the current one
and the one before), valid from when its file was written; one published before the history was
enabled is its first state. Its failure degrades the cycle; what is published is intact. Only `run` records: `serve-snapshot` (a colour that does not crawl) leaves the history alone,
and since at most one colour of a site crawls, one Radix writes it.

The stack sets `RADIX_HISTORY_DB=/data/history.db` for every instance (`deploy/stacks/betula.yml`):
beside `radix.db` in the instance's volume, so that it goes with the crawl when a colour hands it over
(`deploy/README.md` section 4: `ship.sh --seed` takes a `history.db` beside `SEED_DB` along, and
`45-seed.sh` puts it into the volume). A canary records what it publishes into its own volume, which the
next build of master replaces: its history begins again with each build. Releases before Radix 0.15.0
know no history; the public site records once it runs one.

The history is the one thing of Radix that no crawl brings back: lost, it is gone. It needs a backup
of its own (`deploy/README.md` section 8: none exists yet); `radix history export` makes a consistent
copy while the service writes.

## 6. Checks

`radix/internal/history`: what a record stores and ends (a fetch without a change, the build's
numbers moving, a column added empty: no new version), every state rebuilt equal to its snapshot, the
log of a row and of one that went, a schema that grew, the register applied to every version and
versions it made equal joined, a snapshot of a Radix without the register recorded without the person,
a record without the register refused once one was applied, and one with another register, a state
valid from its export. `TestRealSnapshots` does the same on real snapshots
(`HISTORY_SNAPSHOTS=a.db,b.db`, oldest first). `radix/internal/service`: the stage over five cycles, one
of them failing, whose snapshot the next records.
