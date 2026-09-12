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

