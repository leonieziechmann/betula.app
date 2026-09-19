# BTU Course & Module Scraper

A modular, extensible data collection application written in Go with minimal external dependencies. It collects course and module information from BTU Cottbus-Senftenberg (`b-tu.de/modul` and `b-tu.de/modul/<id>`), caches raw web data, and stores structured module records into SQLite.

## Architecture

The project is designed with an extensible **Provider Registry** architecture, making it simple to attach new university data sources (e.g. Prüfungspläne, QIS, Mensa, etc.) in the future:

```
btu-scraper/
├── cmd/
│   └── scraper/
│       └── main.go                 # CLI tool (commands: catalog, detail, all, list, show, providers)
├── internal/
│   ├── cache/
│   │   ├── cache.go                # Cache interface (Get, Set, Delete, Clear)
│   │   └── diskcache.go            # File-based disk cache with TTL & atomic writes
│   ├── model/
│   │   └── module.go               # ModuleSummary, ModuleDetail, StudyProgram, Event models
│   ├── parser/
│   │   ├── catalog.go              # Scrapes & extracts ~4,800 modules from b-tu.de/modul
│   │   ├── detail.go               # Bilingual parser for b-tu.de/modul/<id>
│   │   └── util.go                 # HTML traversal & node formatting helpers
│   ├── provider/
│   │   ├── provider.go             # Provider interface & Registry
│   │   ├── catalog_provider.go     # Scrapes & caches b-tu.de/modul
│   │   └── detail_provider.go      # Scrapes & caches b-tu.de/modul/<id>
│   └── storage/
│       ├── sqlite.go               # SQLite connection, WAL mode & migrations
│       └── repository.go           # Upsert and query repository
```

### Key Architectural Components

1. **Information Provider Registry (`internal/provider`)**:
   - `Provider` interface: represents an external source with a unique identifier and description.
   - `Registry`: thread-safe container to register, query, and list providers.
   - `BTUModuleCatalogProvider`: discovers module IDs, titles, and URLs from the catalog overview.
   - `BTUModuleDetailProvider`: extracts comprehensive course data for an individual module.

2. **Per-Provider Caching (`internal/cache`)**:
   - `DiskCache` stores responses on disk with configurable TTLs (e.g. 24h for the catalog page, 7 days for module details).
   - Atomic writes prevent partial cache files.
   - Supports `--refresh` flag to bypass the cache.

3. **Storage (`internal/storage`)**:
   - Built on `modernc.org/sqlite` (100% pure Go, no CGO or GCC required).
   - Stores structured course attributes: German/English titles, faculty/department, responsible staff, credits (ECTS), teaching forms & workload, prerequisites, study programs, exam details, and active semester links.
   - Indexed for fast full-text filtering by code, title, department, credits, and language.

## Quick Start

### Build

```bash
go build -o scraper.exe ./cmd/scraper
```

### Web Application (HTMX Smart Modulkatalog)

Start the interactive, zero-dependency web catalog with smart prerequisites checking and localStorage persistence:

```bash
# Start the web app on http://localhost:8080
./scraper.exe serve

# Start on a custom port or database path
./scraper.exe serve --port 3000 --db btu_modules.db
```

### CLI Commands

#### 1. Discover all modules from catalog
```bash
# Fetches b-tu.de/modul and saves all module summaries into SQLite (btu_modules.db)
go run ./cmd/scraper catalog
```

#### 2. Scrape details for a single module
```bash
# Scrapes module 11101 and displays a formatted summary card
go run ./cmd/scraper detail 11101

# Output as raw JSON
go run ./cmd/scraper detail 11101 --json
```

#### 3. Scrape all modules with concurrency & rate limiting
```bash
# Scrapes catalog then iterates over modules with 4 concurrent workers
go run ./cmd/scraper all --workers 4 --delay 100

# Test run with first 20 modules
go run ./cmd/scraper all --limit 20 --workers 3
```

#### 4. Search and filter stored modules
```bash
# Search by title or code
go run ./cmd/scraper list --search "Informatik" --limit 15

# Filter by department or credits
go run ./cmd/scraper list --dept "MINT" --min-credits 6
```

#### 5. Show module details from SQLite
```bash
# Formatted text card
go run ./cmd/scraper show 11101

# JSON output
go run ./cmd/scraper show 11101 --json
```

#### 6. Scrape and view course events / lectures (QIS)
```bash
# Scrape an individual event/lecture by ID or URL
go run ./cmd/scraper event 147828

# Scrape all events for a specific module (e.g. 11826 Informatik 1)
go run ./cmd/scraper module-events 11826

# Display timetables (type, day, time, room, instructor) for a module
go run ./cmd/scraper show-events 11826

# Output events as JSON
go run ./cmd/scraper show-events 11826 --json
```

#### 7. Fachübergreifendes Studium (FÜS) Scraper & Non-Adjacent Filter
```bash
# Scrape official approved FÜS modules (288 modules) from the BTU QIS portal
go run ./cmd/scraper fues

# List all study programs / majors discovered across module assignments
go run ./cmd/scraper fues-majors

# List all FÜS modules that are NOT adjacent to your major (e.g. Informatik)
# (Adjacent = module is assigned to that major in 'Zuordnung zu Studiengängen:')
go run ./cmd/scraper fues-eligible "Informatik"

# Filter by minimum credits and limit results
go run ./cmd/scraper fues-eligible "Informatik" --min-credits 6 --limit 10

# Output eligible modules as JSON
go run ./cmd/scraper fues-eligible "Informatik" --json
```

#### 8. Official Study Programs, Regulations & Statutes (QIS & OPUS 4)
```bash
# Scrape statutes and amendments for a specific study program
go run ./cmd/scraper programs --name "Informatik"

# Filter by degree (Bachelor, Master)
go run ./cmd/scraper programs --name "Informatik" --degree "Master"

# Attempt PDF downloads (safely flags bot protection without mitigating)
go run ./cmd/scraper programs --name "Informatik" --download

# Query already stored study programs from SQLite (0ms response time)
go run ./cmd/scraper programs --from-db --name "Informatik"

# Output as JSON
go run ./cmd/scraper programs --from-db --name "Informatik" --json

# Polite crawl for all 92 programs (use 1-2 workers and a polite delay for sensitive servers)
go run ./cmd/scraper programs --workers 1 --delay 1000
```

#### 9. Relational Study Program Module Query
```bash
# Query all modules associated with a study program (by name or official ID)
go run ./cmd/scraper program-modules "Informatik"

# Query by exact official study program ID
go run ./cmd/scraper program-modules "stg_079_abschl_82_po_2008_-_2._SÄ_2024"

# Output as JSON
go run ./cmd/scraper program-modules --json "Informatik"
```

#### 10. List registered providers
```bash
go run ./cmd/scraper providers
```

## Running Tests

```bash
go test -v ./...
```

## Reliable semester extraction from regulation PDFs

`scan-curriculum` reads semester columns from PDF cell coordinates entirely in
**Go**, then uses Gemini to enrich those cells with module numbers and
categories. The model's semester and credit predictions are replaced by the
source cell values. Empty columns, horizontal spans, vertically merged elective
slots, and subtotal rows retain their meaning. There is no credit-balancing
algorithm that moves modules between semesters.

The scraper and PDF parser run in the Go binary; the frontend uses Rust/WASM.
No Python installation, pip packages, CGO, or external PDF commands are required.
The PDF reader is pinned in `go.mod`. Configure the existing Gemini API access:

```powershell
$env:GEMINI_API_KEY = "your-key"
```

The existing `gemini` configuration and `--model` option remain supported.
The application loads `config.yaml` after environment defaults, so an API key
already configured there takes precedence. Do not commit credentials.

Validate without changing curriculum records:

```powershell
go run ./cmd/scraper scan-curriculum --name Informatik --degree Bachelor --force --dry-run
```

`--name` is a substring filter; use `--program-id` for a single exact program.
To store validated results, omit `--dry-run`. Previous PDF plan rows for the program
are replaced and catalog links are updated in one transaction; QIS tree rows remain.
Failed validation preserves the existing records.
Only programs with a snapshot in `validated_curriculum_plans` are skipped.
QIS membership and older AI rows do not count as a validated semester plan.
Use `--force` to explicitly reprocess an already validated plan.

Useful options:

| Option | Meaning |
| --- | --- |
| `--start-term auto` | Use an explicit intake statement from the PDF; otherwise report unknown |
| `--start-term winter` / `summer` | Check a specific intake: summer offerings are even semesters only for winter intake |
| `--credit-tolerance 6` | Warn when source semester totals differ from the expected average by more than 6 LP |
| `--report-dir PATH` | Create a timestamped run directory containing evidence, structured logs and a review report; default `.cache/curriculum` |
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

The Rust study-plan view uses `validated_curriculum_plans` and the per-row
`source_evidence` saved atomically with a successful extraction. It shows semesters,
multi-semester teaching, joint credit windows, and a selector for separate variants.
QIS catalog membership and legacy AI rows are not presented as verified semester
requirements. Programs without a verified snapshot show an explicit empty state;
their module catalogs and original documents remain available.

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
go run ./cmd/scraper scan-curriculum --report-dir .cache/curriculum/remaining
```

The Rust frontend uses these shareable routes (including direct reloads):

- `/catalogue`: catalog with default filters.
- `/catalogue?duration=2&grading=benotet`: readable filter parameters. Only settings
  that differ from defaults are included. Further parameters include `search`,
  `program`, `semester`, `min-credits` and `max-credits`. Multiple professor selections
  repeat `prof-includes` or `prof-excludes`; accordion state stays out of the URL.
- `/course/<id>`: module details.
- `/study-programm/bsc-informatik-2008/plan`: verified semester plan.
- `/study-programm/bsc-informatik-2008/electives`: elective catalogs.
- `/study-programm/bsc-informatik-2008/modules`: all linked catalog modules, even
  when the semester plan still needs review; each module appears once.

Program slugs use degree, name and base PO year; distinct study modes and colliding
versions receive a disambiguating suffix. Umlauts are transliterated for slugs.
Internal program IDs remain unchanged. Old ID paths, `/?module=...`, `/?program=...`
and `/catalouge?data=...` links still resolve and become canonical readable URLs.
The Go server serves the SPA entry point for these paths and retains 404 responses
for missing assets and unknown API endpoints. Static dependencies use absolute paths.

Optional integration tests run the actual PDF extractor against downloaded PDFs:

```powershell
$env:BTU_PDF_TEST_DIR = (Resolve-Path statutes).Path
go test ./internal/gemini -run 'Test.*PDFIntegration|TestPDFLayoutIntegration|Golden' -v
```

The default `go test ./...` suite requires no API key or network calls. It generates
PDF fixtures in Go to test actual text/line decoding, empty and merged cells,
graphics transforms, and rejection of incomplete layouts. It also checks
source binding, season parity, credit validation, response truncation, ambiguous
catalog matches, and transactional replacement/rollback.

