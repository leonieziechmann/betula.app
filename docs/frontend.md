# The web tier: architecture, rules, how to run it

> State: phase 1 of `docs/frontend-rewrite.md` (2026-09-19). The site is fully server-rendered
> and works without JavaScript. The browser app (WASM + local SQLite) joins in phase 2.
> Decisions and their evidence: `docs/frontend-phase0.md`.

## 1. Overview

```
scraper ──HTTP──▶ server ──HTML (cached per snapshot)──▶ browser
 /snapshot/catalog.db        ──/api/db (gzip, ETag)───▶ browser: local SQLite (phase 2)
```

| Crate | Role |
|---|---|
| `catalog/` | The data contract in Rust: row structs, labels, `CatalogQuery` → SQL, every query, the page loaders (`pages.rs`) and the URL scheme (`url.rs`). No I/O; callers hand in a `Database`. Compiles natively and to WASM. |
| `app/` | The Leptos components. Feature `ssr` for the server, `hydrate` for the browser bundle. Pages get their data through `data::Source`. |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `smoke-walk.js` + `run.mjs` (the hydrated app, Playwright). |
| `frontend/` | The old CSR app. Not built; kept as reference until the plan grid and the fuzzy search are ported, then deleted. |

### Routes (`catalog/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `kind`, `lecturer`, `not-lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `prereqs`, `sort`, `desc`, `page` |
| `/catalog/module/<id>` | Module page |
| `/programs` | Program overview (current PO versions) |
| `/programs/<slug>/plan\|areas\|modules` | Program page and its tabs |

The catalog parameters are tolerant (repeated or comma-joined values, empty inputs of a plain
HTML form, nonsense ignored) and have one canonical spelling, which is also the cache key.

### Data flow

- **Pages are synchronous functions of their route parameters.** SQLite answers in 1–7 ms on
  both sides (rusqlite on the server, sql.js in the browser), so there are no async resources,
  no loading states between pages and nothing to serialize into the HTML. A page calls one
  loader of `catalog::pages` through `Source::run`; everything it shows comes from one snapshot.
- **The server renders and caches.** HTML depends only on URL + snapshot (rule R9), so the first
  request renders (5–100 ms) and later ones are a memory copy (2 ms), gzip included. A new
  snapshot starts a new generation. ETag per generation → `304` without rendering.
  `404`/`5xx` are `no-store`. Without a snapshot everything answers `503` + `Retry-After`.
- **Owner decision:** after the page is interactive all queries run in the browser on the
  downloaded snapshot (`/api/db`: 36.8 MB, 6.5 MB gzip, compressed once per snapshot; ETag =
  the scraper's content hash). Until the local database is ready the site behaves like a classic
  website: links are page loads, served from the cache. The server never answers data queries
  for the app, so its load is cached HTML plus one file.

## 2. Rules

R1–R8 from `docs/frontend-rewrite.md` §5 apply. In short: navigation state reaches pages as
plain values from the router; nothing page-owned is read after unmount; no panics (`unwrap`,
`expect`, indexing and `panic!` are denied by clippy in all three crates); filter state = the
URL, UI state never triggers a query; keyed lists; one source of truth; design tokens only, no
inline styles; keyboard and phone usable. Added in phase 0/1:

- **R9. Server HTML is user-independent.** Bookmarks, passed modules and the chosen major live
  in the browser and are applied after the app took over, never during the first render.
- **R10. Esc closes what feels like a popup** (suggestions, filter sheet, dialogs), nothing else.
  Going back is the browser's job: every view is a real history entry.
- **R11. All SQL lives in `catalog/src/queries.rs`,** reads only `v_*` views, and every `pub fn`
  there runs against a real snapshot in the tests (the build fails otherwise).
- **R12. Unknown stays unknown:** `Option` in the row structs, „nicht angegeben" on the page.
  A code without a label is shown as it is (`labels::Code`), and the label test flags it.

## 3. Running it

```bash
./scraper.exe serve-snapshot --addr 127.0.0.1:8090
```

```bash
cargo run -p btu-server
```

Open `http://127.0.0.1:8080`. The server fetches the snapshot over HTTP into `web-data/` and
keeps serving the last good one when the scraper is away, also after a restart.

| Flag | Environment | Default | |
|---|---|---|---|
| `--addr` | `BTU_WEB_ADDR` | `127.0.0.1:8080` | listen address |
| `--snapshot-url` | `BTU_SNAPSHOT_URL` | `http://127.0.0.1:8090/snapshot/catalog.db` | the scraper's endpoint (plain HTTP inside the deployment network) |
| `--data-dir` | `BTU_WEB_DATA_DIR` | `web-data` | downloaded snapshots |
| `--poll-seconds` | `BTU_SNAPSHOT_POLL` | `60` | check interval (conditional GET) |
| `--stale-after-seconds` | `BTU_SNAPSHOT_STALE_AFTER` | `21600` | `/healthz` fails when the scraper was silent this long (0: never) |
| `--html-cache-mb` | `BTU_HTML_CACHE_MB` | `128` | rendered pages kept in memory |
| `--site-root` | `BTU_SITE_ROOT` | `site` | browser bundle (`pkg/`), from phase 2 |
| `--log-format`, `--log-level` | `BTU_LOG_FORMAT`, `BTU_LOG_LEVEL` | `text`, `info` | `json` in production |

Endpoints besides the pages: `GET /api/db`, `GET /api/status`, `GET /healthz`, `/assets/app.css`,
`/assets/favicon.svg`, `/robots.txt`.

### Log events (same rules as `docs/operations.md` §2: ERROR = a human has to act)

| Level | `event` | Meaning |
|---|---|---|
| INFO | `server.listening`, `server.shutdown` | lifecycle |
| INFO | `snapshot.sync_started`, `snapshot.restored`, `snapshot.downloaded`, `snapshot.activated`, `snapshot.sync_recovered` | snapshot lifecycle (`etag`, `bytes`, `generation`) |
| DEBUG | `snapshot.unchanged` | the scraper answered 304 |
| INFO | `http.request` | access log: `method`, `path`, `status`, `ms`, `cache` (`hit`/`miss`/`-`) |
| WARN | `snapshot.fetch_failed` | scraper unreachable or not ready; retried with backoff; the last snapshot stays active |
| WARN | `snapshot.restore_failed`, `snapshot.compress_failed` | stored snapshot unusable / served uncompressed |
| ERROR | `snapshot.rejected` | a download is not a usable catalog; the previous snapshot stays active |
| ERROR | `snapshot.stale` | no answer from the scraper for longer than the limit |
| ERROR | `http.request` with `status >= 500`, `render.failed`, `snapshot.unreadable` | a request failed |
| ERROR | `server.start_failed`, `server.failed` | the server cannot run |

## 4. Checks

```bash
cargo test --workspace
```

needs a snapshot (`snapshot/current.json` or `BTU_TEST_SNAPSHOT`) and fails without one:

- `catalog`: every filter against direct SQL, exact totals and paging, the pinned numbers, enum
  labels from the CHECK constraints, every query and page loader against real data, the URL codec.
- `server`: a fake scraper over HTTP: not ready → 503; download, check, gzip, activate; 304 →
  no download; pages render, cache (`hit`/`miss`), revalidate; equal filters share a cache key;
  404 is never cached; `/api/db` with the scraper's ETag, gzip and 304; a broken export is
  rejected and the old snapshot stays; a new one invalidates pages; restart without the scraper.

```bash
cargo clippy --workspace --all-targets
```

```bash
cd e2e && node crawl.mjs
```

crawls the running site like a search engine: every program × tab, the whole catalog page by
page (the pages must add up to the header's total), module pages, the 404s. Last run: 817 pages,
0 failures, slowest page 113 ms on a debug build.

`e2e/run.mjs` (Playwright smoke walk with fast program-to-program navigation) is for the
hydrated app of phase 2; the pages already carry its `data-walk` attributes.

## 5. Not done yet

- Phase 2: browser bundle (`client/`), sql.js bridge as a `Source`, takeover once the local
  database is ready, user data (bookmarks, passed, major), service worker.
- Phase 3: design system, plan grid with variants, weekly calendar, filter bottom sheet, search
  with context ranking (own concept, see `docs/frontend-phase0.md`).
- Phase 4: Nix package and container, Swarm stack, CSP, `sitemap.xml`, CI.
