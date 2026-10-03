# Betula

An unofficial catalog of the modules, study programs and study plans of BTU Cottbus-Senftenberg
(https://betula.app). Betula is the birch; its two parts are named after the tree:

- **Radix** (the root, Go): collects the course data (modules, study programs, study plans,
  events), keeps it up to date as a long-running service, and publishes it as SQLite snapshots
  over HTTP.
- **Folia** (the leaves, Rust): the web server and the browser app. It fetches the snapshots and
  redistributes them to browsers, which query the database locally through documented read views.

```
QIS (module descriptions, tree, events), b-tu.de/modul ──crawl──▶ raw page archive ──build──▶ canonical tables + views ──validate──▶ export ──HTTP──▶ Folia ──▶ browsers
statute PDFs (OPUS) ──scan-curriculum (PDF geometry + optional Gemini enrichment)──▶ validated study plans
```

| | |
|---|---|
| [docs/radix/operations.md](docs/radix/operations.md) | running it as a service, configuration, secrets, log events, notifications, container / Docker Swarm / systemd |
| [docs/radix/schema-v2.md](docs/radix/schema-v2.md) | pipeline, tables, the read views (the contract for consumers), what is still open |
| [docs/radix/data-sources.md](docs/radix/data-sources.md) | where every fact comes from, which source wins, and the evidence |
| [docs/radix/backend-data-overhaul.md](docs/radix/backend-data-overhaul.md) | the brief this design follows |
| [docs/folia/frontend.md](docs/folia/frontend.md) | Folia: the web server and the browser app, flags, endpoints, checks |
| [docs/folia/folia-refactor.md](docs/folia/folia-refactor.md) | Folia's restructuring: site and app, shell, workers, crates |

## Repository

| Directory | What it holds |
|---|---|
| `radix/` | Radix, one Go module (`github.com/leonieziechmann/betula/radix`): `cmd/radix`, `internal/` |
| `folia/` | Folia, one Cargo workspace: `crates/<crate>` (package `folia-<crate>`), `assets/` (stylesheet, scripts, icons the server embeds), `e2e/` (browser checks), `design/` (sources of drawings and icons), `scripts/` (builds, the dev server) |
| `docs/` | `radix/`, `folia/`, and `history/` for the briefs and plans that are done |
| `deploy/` | the stacks, the server set-up, `ship.sh` |
| `research/` | experiments that are not part of the product (the semantic search's training and demos) |
| `flake.nix` | the two binaries and their container images |

## Quick start

```bash
cd radix
go build -o radix ./cmd/radix      # pure Go, no CGO; Windows, Linux, macOS
radix run                            # service: crawl politely, build, validate, export, serve
radix run --once                     # a single cycle
radix help                           # all commands
```

`radix run` serves `GET /snapshot/catalog.db` (ETag, `If-None-Match` → 304), `/healthz` and
`/status` on `127.0.0.1:8090`. The stages also run one by one: `crawl-qis-modules`, `crawl-modules`,
`crawl-tree`, `crawl-events`, `prune`, `build`, `validate`, `export`, `serve-snapshot`.

The module fields come from the module description in QIS, not from `b-tu.de/modul`: that page
is a copy BTU's CMS renders, and it lags — in September 2026 it still named the events of the
summer while QIS already had the winter schedule (`docs/radix/data-sources.md` §10).

```bash
nix build .#radix                    # static binary; the tests run inside the build
nix build .#radix-image                  # container image with a health check (docs/radix/operations.md)
(cd radix && go test ./...)         # network-free, no API key needed
```

### Layout

| Package | Role |
|---|---|
| `radix/cmd/radix` | command line of Radix |
| `radix/internal/service` | the service loop, its stages, `/healthz` and `/status` |
| `radix/internal/crawl`, `radix/internal/qistree` | polite archiving; QIS program tree walker |
| `radix/internal/catalogdb` | database: migrations, raw archive, plans, validate, export, retention |
| `radix/internal/catalogbuild`, `radix/internal/normalize`, `radix/internal/parser` | raw pages → canonical tables; rule-based normalization (room short forms included); HTML parsers |
| `radix/internal/abbrev` | module abbreviations (AuP, EEG), unique within a program, derived by every build; the curated `overrides.tsv` and the `blocked.tsv` of forms never derived |
| `radix/internal/gemini`, `radix/internal/curriculumscan`, `radix/internal/statutes`, `radix/internal/planaudit` | study plans from regulation PDFs, with audit trail |
| `radix/internal/secrets` | credentials from Docker/systemd secrets, environment or the OS credential store |
| `radix/internal/oplog`, `radix/internal/snapshothttp` | structured operational log; snapshot HTTP endpoints |
| `folia/crates/{catalog,app,client,pack,semantic,server}` | Folia, the web tier in Rust (`docs/folia/frontend.md`): the data contract with every SQL query, the Leptos app, the codes that carry a value in a link (a compact bit format for serde in base 66, with two check characters), and the web server that fetches snapshots over HTTP, renders and caches the pages and serves `/api/db`. |
| `folia/e2e/` | crawl of the server-rendered site; Playwright smoke walk for the browser app |

### Web tier

```bash
radix/radix serve-snapshot --addr 127.0.0.1:8090
```

```bash
git config core.hooksPath folia/scripts/hooks   # once per clone: new worktrees set themselves up
bash folia/scripts/build-cache.sh setup         # once in the main checkout: its build cache and flags
bash folia/scripts/dev.sh --watch               # the browser app and the server, built again on every change
```

Then open http://127.0.0.1:8080. `folia/scripts/dev.sh` builds the browser app for localhost when it is
stale (`folia/scripts/build-client.sh --dev`, seconds instead of minutes) and runs the server with
`folia/assets` live: an edit of the stylesheet, a script or an SVG is there with the next reload,
without a build; `--watch` builds again and restarts when Rust code changes, and
`bash folia/scripts/dev.sh sizes` lists what ships. What ships is minified when the server is built
(`folia/crates/server/build/main.rs`). Folia talks to Radix only through the snapshot
endpoint. Flags, endpoints, log events and checks: `docs/folia/frontend.md`. `cargo test --workspace` (in
`folia/`) needs an exported snapshot (`radix export`), or the one betula.app serves: `curl --compressed -o
snapshot/catalog.db https://betula.app/api/db`, then `FOLIA_TEST_SNAPSHOT=snapshot/catalog.db`
(`docs/folia/frontend.md` §4 also names it for `serve-snapshot`). `folia/scripts/build-client.sh` without `--dev` builds the
bundle that ships.
A new worktree forks the main checkout's build cache as `git worktree add` creates it, so only
the workspace's own crates compile; what that costs and how the caches are kept and dropped:
`docs/folia/frontend.md` §3.

### Credentials

The Gemini API key is never read from a flag or a configuration file:

```bash
cp .env.example .env                   # development: put GEMINI_API_KEY there; the file is git-ignored
radix secret set gemini-api-key      # or: Windows Credential Manager, macOS Keychain, Secret Service
radix secret status                  # where the key is found; never prints it
```

A service gets it as a Docker secret (`/run/secrets/gemini-api-key`), a systemd credential,
`GEMINI_API_KEY_FILE`, or `GEMINI_API_KEY`. Without a key, `scan-curriculum` still works with
the deterministic PDF reader alone.

## Reliable semester extraction from regulation PDFs

`scan-curriculum` reads semester columns from PDF cell coordinates entirely in
**Go**, then uses Gemini to enrich those cells with module numbers and
categories. The model's semester and credit predictions are replaced by the
source cell values. Empty columns, horizontal spans, vertically merged elective
slots, and subtotal rows retain their meaning. There is no credit-balancing
algorithm that moves modules between semesters.

The PDF parser runs in the Go binary: no Python, CGO or external PDF commands. The PDF reader is
pinned in `radix/go.mod`. The key is resolved by `radix/internal/secrets` (see above); `--model` or `GEMINI_MODEL` selects the model.

**What a plan adds up to is read as well.** A row may print a range („Komplex Grundlagen der
Informatik, 10-24 LP"), and three such rows are anything between 30 and 72 LP: the rows alone do
not say what a degree costs. The regulation does, in the lines it prints over its own rows, and
`plan_total` keeps each of them with the rows it counts — the line over those three rows, and the
line over the whole table. A line is kept only where its rows reach the printed value, so a sum is
evidence and not an assumption (`radix/internal/gemini/plan_totals.go`, `docs/radix/schema-v2.md` §6).

**A plan is read from its own regulation.** An issue of the Amtliches Mitteilungsblatt may print
the Bachelor's and the Master's Prüfungsordnung of one subject one after the other, each with its
own plan. A program reads only the pages its own regulation stands on, as the issue's table of
contents names them; a document whose regulations cannot be told apart, or that has none for the
program's degree, goes to review (`radix/internal/gemini/pdf_regulations.go`).

A plan row is linked to a catalog module only where the catalog identifies it beyond doubt: by the
printed module number, or by a title that names exactly one module. Titles repeat across the
university — 68 modules are called „Bachelor-Arbeit", and both „Grundlagen der Elektrotechnik" are
current, one read by Maschinenbau and one by Elektrotechnik — so where a title names several, the
modules the program itself claims (its module pages, its QIS tree) decide. Claiming none or
several leaves the row unlinked, as does a row that names no module at all („Anwendungsfach",
„Komplex Praktische Informatik"). `relink-plans` applies this matching to the stored plans without
reading a PDF again (`docs/radix/operations.md`, „One-off commands").

Validate without changing curriculum records:

```powershell
go run ./cmd/radix scan-curriculum --name Informatik --degree Bachelor --force --dry-run
```

`--name` is a substring filter; use `--program-id` (e.g. `079-82-2008`) for a single program.
The PDFs are expected in `statutes/` (`radix download-statutes`).
To store validated results, omit `--dry-run`. The previous plan of the program is replaced in
one transaction (`catalogdb.SavePlan`); the next `build` derives the membership statements.
Failed validation preserves the existing records.
Only programs with a stored validated plan are skipped.
Use `--force` to explicitly reprocess an already validated plan.

Useful options:

| Option | Meaning |
| --- | --- |
| `--start-term auto` | Use an explicit intake statement from the PDF; otherwise report unknown |
| `--start-term winter` / `summer` | Check a specific intake: summer offerings are even semesters only for winter intake |
| `--credit-tolerance 6` | Warn when source semester totals differ from the expected average by more than 6 LP |
| `--report-dir PATH` | Create a timestamped run directory containing evidence, structured logs and a review report; default `logs/curriculum` |
| `--json` | JSON Lines on stdout; diagnostics go to stderr |
| `--pdf PATH --program-id ID` | Select the applicable complete study plan when multiple statutes/amendments exist |
| `--plan-pages 7,9` | With `--pdf`, select complete plan variants on these physical PDF pages; unrelated/dual-study appendices are not mixed in |

Checks include unique catalog identity, winter/summer offerings, credit differences
from the current catalog, semester bounds, missing/duplicate cells, and the PDF's
printed semester totals. Source sums take priority over an assumed 30 LP.
Current catalog credits can differ from historical regulations, so those differences
are warnings. Season conflicts require review. Multi-semester teaching and biennial
offerings are treated separately; unknown intake never implies winter intake.

An exact semester of `0` means **the PDF does not specify an individual semester**.
For example, a merged 5–6 cell retains `start_semester=5`, `end_semester=6`, and
`semester_span="5-6"`. A 10–24 LP range retains `min_credits=10`, `max_credits=24`,
and `credits=0`; no mean or arbitrary allocation is invented. The validation report
also retains joint totals, such as 60 LP across semesters 5–6. Semester totals
must not add up every alternative in a Wahlpflicht catalog.

**Supported layouts:** ruled tables with a clear consecutive semester header and
numeric LP values or LP ranges, plus semester panels with separate module/LP columns
and printed totals (Medizininformatik). A notation such as `(3+3) 6` is supported
when the document explicitly explains workload versus credited LP: the module spans
two semesters, workload is 3+3, and 6 LP are credited on completion. Separate complete
tables remain separate study variants. Scanned/image-only documents, split table fragments,
other unresolved formulas, Form XObjects, unsupported font
encodings/text orientations, and alternatives nested within one table may
require a dedicated layout parser. These cases are rejected for review, not silently
saved through an unverified AI fallback. A successful plausibility check is not a
general accuracy guarantee for every regulation. The current regression corpus
contains Informatik B.Sc., Wirtschaftsinformatik B.Sc., Medizininformatik B.Sc.
(2024) and the two six-semester Elektrotechnik B.Sc. variants (2022), including
merged cells, LP ranges, workload, semester panels, repeated amendment copies,
unnamed subtotals, and deliberately wrong/incomplete AI enrichment.

Each scan writes three audit files in its `run-<timestamp>` directory:

- `events.jsonl`: durable chronological events, including program ID/name, source,
  module, severity, issue code, recommended next step and evidence path. Events are
  flushed individually, including failures and skipped programs.
- `summary.json`: final status, outcome counts, issue counts and per-program results.
- `review.md`: human-readable list of warnings/errors and concrete follow-up work.

Programs without PDFs and ambiguous source versions are reported explicitly.
Existing validated snapshots are preserved. Successfully saved plans with warnings
are counted separately from clean results. A run with unresolved programs exits
nonzero, even if some other programs were imported successfully. API credentials
are redacted from extraction errors and audit messages.
Low catalog coverage is also a warning: if fewer than half of the concrete,
non-elective requirements can be linked, historical module records or title
differences need review. A semester plan can be geometrically valid while these
links are still missing. Wahlpflicht/FÜS budgets do not require one concrete link.

To process all remaining programs without replacing validated plans:

```powershell
go run ./cmd/radix scan-curriculum --report-dir logs/curriculum/remaining
```

Optional integration tests run the actual PDF extractor against downloaded PDFs:

```powershell
$env:RADIX_PDF_TEST_DIR = (Resolve-Path statutes).Path
go test ./internal/gemini -run 'Test.*PDFIntegration|TestPDFLayoutIntegration|Golden' -v
```

The default `go test ./...` suite requires no API key or network calls. It generates
PDF fixtures in Go to test actual text/line decoding, empty and merged cells,
graphics transforms, and rejection of incomplete layouts. It also checks
source binding, season parity, credit validation, response truncation, ambiguous
catalog matches, and transactional replacement/rollback.

