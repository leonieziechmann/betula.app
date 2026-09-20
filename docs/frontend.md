# The web tier: architecture, rules, how to run it

> State: 2026-09-20. Every page is server-rendered and works without JavaScript; with
> JavaScript the browser app (WASM + local SQLite) takes the page over and nothing is loaded
> again. Not yet: PWA (service worker, manifest), bookmarks and other user data, context search.
> Decisions and their evidence: `docs/frontend-phase0.md`.

## 1. Overview

```
scraper ──HTTP──▶ server ──HTML (cached per snapshot)──▶ browser
 /snapshot/catalog.db        ──/api/db (gzip, ETag)───▶ browser: local SQLite (phase 2)
```

| Crate | Role |
|---|---|
| `catalog/` | The data contract in Rust: row structs, labels, `CatalogQuery` → SQL, every query, the page loaders (`pages.rs`) and the URL scheme (`url.rs`). No I/O; callers hand in a `Database`. Compiles natively and to WASM. |
| `app/` | The Leptos components. Feature `ssr` for the server, `csr` for the browser app. Pages get their data through `data::Source`. |
| `client/` | The browser app (WASM): `app` with feature `csr` on a `Source` backed by sql.js. Not a default workspace member (its `csr` would be unified with the server's `ssr`); built by `scripts/build-client.sh` into `site/pkg`. |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `spa.mjs` (the browser app: takeover, no page loads, preview, filters, search), `smoke-walk.js` + `run.mjs` (long program walk), `shot.mjs` (review screenshots). All use an installed Edge through `playwright-core`. |
| `frontend/` | The old CSR app. Not built; kept as reference until the plan grid and the fuzzy search are ported, then deleted. |

### Routes (`catalog/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `kind`, `lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `prereqs`, `sort`, `desc`, `page`. What can be wanted can also be excluded: `not-kind`, `not-lecturer`, `not-turnus`, `not-form`, `not-exam`, `not-campus`, `not-lang` (`exam=written&not-exam=presentation`: a written exam and no presentation) |
| `/catalog?…&open=<id>` | The same list with this module previewed next to it (full screen on a phone); the preview has a „Vollbild" link to the module's page |
| `/catalog/module/<id>` | The module's own page, two columns on the whole screen |
| `/programs?q=…` | The program overview filtered by the search in the top bar |
| `/programs` | Program overview (current PO versions) |
| `/programs/<slug>/plan\|areas\|modules` | Program page and its tabs |

The catalog parameters are tolerant (repeated or comma-joined values, empty inputs of a plain
HTML form, nonsense ignored) and have one canonical spelling, which is also the cache key. A
value that is both wanted and excluded counts as wanted. An exclusion removes only what the data
states: a module whose campus or turnus is unknown stays in the list (R12).

### Look and interaction (since 2026-09-19, owner-approved direction)

- **Layout:** a thin icon rail (52 px), a top bar with the search, and the whole remaining screen
  for content, with 8 px gaps and 9 px corners. The catalog is three panels side by side: filters,
  list, and the preview of the selected module (`open=<id>`). The preview floats above the list,
  docked to the right edge, so the table is never resized; its width is changed by dragging its
  left edge (arrow keys work too, a double click resets it) and remembered in `localStorage`.
  Selecting a module keeps the list, the filters and the scroll position; on a phone the preview fills the screen,
  the filters become a bottom sheet and the rail a bottom bar. The list uses container queries:
  the narrower it gets, the fewer columns it shows.
- **Targets:** whole rows are links (54 px, 72 px on a phone); filter toggles are 32 px on the
  desktop to keep the panel short, 44 px on a phone. Small controls have **virtual oversizing**
  (R14): they take the pointer in an invisible area around them.
- **The filter panel** (`app/src/pages/catalog.rs`, JavaScript first, owner decision 2026-09-20):
  - It is rendered once and then follows the URL (`Filters` takes memos, not values), so a change
    keeps the focus, the scroll position and what is folded open. Its width is dragged at the
    handle in the gap between the two boxes, inside 232–440 px, and remembered in `localStorage`.
  - **Toggles are links** to the list with the next state of their value: off → with → without →
    off („keine Vorträge" is the second click). The small box on their left shows the state, so
    they read as switches; the toggles of a row share its whole width. As links they need no
    handler (the router turns the click into a navigation), work without JavaScript, carry
    `rel="nofollow"` and `data-noscroll`, and the space bar flips them like a checkbox.
    „One of a few" (list, plan semester, duration, years) is a segmented row of the same links.
  - **Pickers** (`app/src/combobox.rs`: program, lecturers, department) have a search that
    forgives typos and knows initials and abbreviations (`catalog::fuzzy`: „infomatik bsc"), arrow
    keys, Enter, Esc. Their popup is fixed to the window, so no panel clips it; on a phone it
    opens in place. The module comment of the component lists what keeps it predictable (it is a
    rewrite: the picker of the old frontend lost its mark to the mouse, closed the preview with
    Esc and knew its selection by label). Without the app the same places hold a plain `select`
    or text field inside a GET form, and hidden inputs carry what the links have set.
  - **Credits:** a slider with two knobs (0–30, the right end means „no upper limit") and the two
    exact numbers under it. The knobs cannot pass each other; the filter follows when a knob is
    let go.
  - A chosen lecturer is a toggle of its own: with („bei"), without („nicht bei"), gone.
- **The search in the top bar belongs to the page:** modules everywhere, programs on `/programs`.
  In the browser app it filters while typing (history entry replaced, not added).
- **Tokens:** `app/assets/app.css` starts with the token block (colors, radii, shadows); everything
  below uses tokens only. One look, light and dark: dark follows the system, the switch in the rail
  overrides it (`data-theme` on `<html>`, remembered in `localStorage`). Accent color only for
  primary actions and the marker of the open row; selected chips are neutral (inverted).
  Font: Inter (variable, latin subset, OFL), self-hosted. Icons: Lucide (ISC), inlined through
  `app/src/icons.rs`. The only `style` attributes carry data as custom properties: the week grid
  and the credit slider (`--from`, `--to`, `--at`), the place of a picker's popup, `ui::Hit`.
- **`assets/enhance.js`** (progressive enhancement until the browser app takes over): the plain
  fields of the filter form apply on change, panels keep their scroll position across page loads.
  In both modes: the shortcuts, the theme switch, the filter sheet, and the two resize handles. Page changes use
  cross-document view transitions where the browser supports them.
- `design/prototype.html` is the clickable design prototype the direction was agreed on;
  `node e2e/shot.mjs <url> <out.png> [w] [h] [--dark]` takes review screenshots.

### Data flow

- **Pages are synchronous functions of their route parameters.** SQLite answers in 1–7 ms on
  both sides (rusqlite on the server, sql.js in the browser), so there are no async resources,
  no loading states between pages and nothing to serialize into the HTML. A page calls one
  loader of `catalog::pages` through `Source::run`; everything it shows comes from one snapshot.
- **The server renders and caches.** HTML depends only on URL + snapshot (rule R9), so the first
  request renders (5–100 ms) and later ones are a memory copy (2 ms), gzip included. A new
  snapshot starts a new generation. ETag per generation → `304` without rendering.
  `404`/`5xx` are `no-store`. Without a snapshot everything answers `503` + `Retry-After`.
- **The browser app (owner decision: all queries run in the browser).** `assets/boot.js` opens
  the local copy of the snapshot (`/api/db`: 36.8 MB, 6.5 MB gzip; kept in IndexedDB with its ETag;
  sql.js) and loads the WASM bundle (535 KB gzip) in parallel; then `client::start()` replaces the
  server-rendered body by the app. Not hydration: the local copy may be older than the server's
  page, so the app renders fresh with the same components. From then on links, filters and the
  search are client-side navigation on the local database (measured: takeover 1.2 s on a first
  visit, preview 130 ms, filter 150 ms including the test driver). Until the takeover, and if
  anything fails, the site stays a classic website served from the HTML cache. A newer snapshot is
  downloaded in the background and used from the next start. The server never answers data
  queries for the app: its load is cached HTML, static files and one database file.
- **Fine-grained updates:** the catalog page splits its URL into the filter (what the list is),
  `page` (where the visitor is in it) and `open` (the preview). Opening a preview or scrolling
  re-renders neither list nor filters, and a filter change leaves the preview alone.
- **Endless list:** the list is a sequence of chunks, one per page of 50. The server renders the
  page the URL names, with pager links (no JavaScript, search engines). In the browser app the
  next chunk is appended when the visitor gets near the end, earlier ones are prepended on request
  („Vorherige Module laden", scroll position kept), and `page` in the URL follows the chunk at the
  top of the screen by replacing the history entry. A shared link with `page=7` starts there.

## 2. Rules

R1–R8 from `docs/frontend-rewrite.md` §5 apply. In short: navigation state reaches pages as
plain values from the router; nothing page-owned is read after unmount; no panics (`unwrap`,
`expect`, indexing and `panic!` are denied by clippy in all three crates); filter state = the
URL, UI state never triggers a query; keyed lists; one source of truth; design tokens only, no
inline styles; keyboard and phone usable. Added in phase 0/1:

- **R9. Server HTML is user-independent.** Bookmarks, passed modules and the chosen major live
  in the browser and are applied after the app took over, never during the first render.
- **R10. Shortcuts are written next to their button** (`kbd`): Esc closes the filter sheet or the
  module preview and, on a module's own page, goes back to where the visitor came from; `F` opens
  the previewed module full screen; ↑/↓ move through the list (rows are links, so this is just
  focus) and Enter opens the selected row; Ctrl+K or `/` jumps to the search. Every view is a real
  history entry, so the browser's back always works too.
- **R13. Personal view settings never go into the URL**: theme and the widths of the filter panel
  and the preview live in `localStorage` and are applied before the first paint by the script in
  `<head>`.
- **R14. Virtual oversizing.** A small control takes the pointer in an area larger than it shows:
  an invisible layer that belongs to the control itself (a wrapper element would receive the click
  instead of the control). Set per side with `--hit`, `--hit-x`/`--hit-y`, `--hit-t/-r/-b/-l`;
  the stylesheet sets whole families (toggles, segments, tag and sort links, rail), `ui::Hit`
  writes the properties for a single element. **Towards a neighbouring control at most half the
  gap to it**, so two areas meet in the middle and never overlap; more only into space that holds
  no control. Resize handles are zones of 16–20 px around a 4 px grip; the slider's knob is drawn
  inside a larger thumb.
- **R15. What needs JavaScript is not shown without it:** shortcut hints, resize handles, the
  slider, the theme switch. Wrap it in `ui::JsOnly` (or give a single element the class
  `js-only`; `ui::Shortcut` does it for `kbd`). The parts stay in the HTML, because the server
  sends the same cached page to everybody (R9); the stylesheet hides them until the script in
  `<head>` has marked the document, which is before the first paint. What only the browser app
  can do (endless list, pickers) is rendered by the app alone or shown under `html.app`.
- **R11. All SQL lives in `catalog/src/queries.rs`,** reads only `v_*` views, and every `pub fn`
  there runs against a real snapshot in the tests (the build fails otherwise).
- **R12. Unknown stays unknown:** `Option` in the row structs, „nicht angegeben" on the page.
  A code without a label is shown as it is (`labels::Code`), and the label test flags it.

## 3. Running it

```bash
./scraper.exe serve-snapshot --addr 127.0.0.1:8090
```

```bash
bash scripts/build-client.sh
```

```bash
cargo run -p btu-server
```

The first command builds the browser app into `site/pkg` (needs the `wasm32-unknown-unknown`
target and `wasm-bindgen` 0.2.128, which Trunk keeps in its cache); without it the site simply
stays server-rendered.

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
cargo test
```

needs a snapshot (`snapshot/current.json` or `BTU_TEST_SNAPSHOT`) and fails without one:

- `catalog`: every filter against direct SQL (exclusions included), exact totals and paging, the
  pinned numbers, enum labels from the CHECK constraints, every query and page loader against
  real data, the URL codec, the ranking of the pickers (`fuzzy`).
- `server`: a fake scraper over HTTP: not ready → 503; download, check, gzip, activate; 304 →
  no download; pages render, cache (`hit`/`miss`), revalidate; equal filters share a cache key;
  404 is never cached; `/api/db` with the scraper's ETag, gzip and 304; a broken export is
  rejected and the old snapshot stays; a new one invalidates pages; restart without the scraper.

```bash
cargo clippy --all-targets
cargo clippy -p btu-client --target wasm32-unknown-unknown
```

```bash
cd e2e && node crawl.mjs
```

crawls the running site like a search engine: every program × tab, the whole catalog page by
page (the pages must add up to the header's total), module pages, the 404s. Last run: 817 pages,
0 failures, slowest page 113 ms on a debug build.

```bash
cd e2e && node spa.mjs
```

drives the browser app in Edge: waits for the takeover, then opens a preview (the list must keep
its scroll position), filters, closes with Esc, opens the full page, goes back, searches programs,
and fails on any page load after the takeover or any console error.

```bash
cd e2e && node filters.mjs
```

drives the filter panel: a toggle through its three states (the panel must stay the same element
and keep the focus), rows of toggles filling the width, oversized hit areas, the search field
aligned with the list, the program picker (typo, arrow keys against a resting mouse pointer, wrap
around, Enter, focus back on the button, Esc closing only the picker, click outside, clear, no two
entries alike), the lecturer picker, the slider (drag, keyboard, knobs not crossing, typed
numbers), the panel's width (limits, `localStorage`, reset), the group header across the border
of two pages, and the same panel without JavaScript (links keep the rest of the filter, the form
keeps what the links set, nothing that needs JavaScript is visible).

## 5. Not done yet

- PWA: manifest, service worker (offline start), update prompt. User data: bookmarks, passed
  modules with the prerequisite check, „mein Studiengang". `wasm-opt` for the bundle.
- Phase 3: design system, plan grid with variants, weekly calendar, filter bottom sheet, search
  with context ranking (own concept, see `docs/frontend-phase0.md`).
- Phase 4: Nix package and container, Swarm stack, CSP, `sitemap.xml`, CI.
