# Brief: Web server & frontend rewrite on the schema v2 contract

> Hand-off brief, self-contained. Verified against the repository, the snapshot
> `snapshot/catalog-1d4691d0656871d6.db` and the frontend's own sql.js build on 2026-09-19.
> Companion documents: `docs/radix/schema-v2.md` (the data contract), `docs/radix/operations.md`
> (Radix), `docs/radix/data-sources.md` §7 (owner decisions about the data).

## 1. Context

The backend overhaul is finished on branch **`backend-data-overhaul`** (not merged into `master`).

```
b-tu.de, QIS ──▶ Radix (Go, `radix run`) ──▶ build ▶ validate ▶ export
                                                      │
                   GET /snapshot/catalog.db  (ETag = content hash, If-None-Match → 304, Range)
                                                      ▼
                                   web server (Rust, `server/`) ──/api/db──▶ browser (Leptos/WASM + sql.js)
```

- **Radix** listens on `127.0.0.1:8090` by default and serves:
  - `GET /snapshot/catalog.db` (503 + `Retry-After` before the first export)
  - `GET /snapshot/current.json` (`{"file","etag","bytes","exported_at"}`)
  - `GET /healthz` and `GET /status`

  **This HTTP endpoint is the only interface between Radix and the web server (Folia); they share no files.** See `docs/radix/operations.md` §1.
- **The snapshot** is a 36.8 MB SQLite file. It ships the read views, which are the **only read contract** (`docs/radix/schema-v2.md` §3). §4 of that document maps every query in today's `frontend/src/db.rs` to its replacement view.
- **The web tier has not been touched yet.** `server/` (axum + rusqlite) and `frontend/` (Leptos 0.7.8 CSR + sql.js) still read the **v1** file `btu_modules.db`, which no longer exists. Neither works against v2.

**Owner decisions that apply here**

- **The browser keeps downloading the SQLite file** and queries it locally with sql.js.
- **No backward compatibility.** URLs, slugs, storage keys and the component API may all change.
- **Keep the base structure; the owner likes it.** It is intuitive and gets you to your destination fast (§6):
  - left sidebar with grouped filters
  - main module table
  - module detail page
  - program page with tabs
  - top bar with search, share, and the „Gemerkt" / „Bestanden" views
- **UI language is German.** Module titles come in German or English, as the data has them.
- **Show unknown as unknown.** Stated information beats inferred; nothing is guessed (degree labels, module kind, semester, campus).
- **Deployment target** for Radix is a Linux service in Docker Swarm, with the image built with Nix, while staying Windows/CLI compatible. Confirm with the owner whether the web tier follows the same pattern (open question 7).
- **Logging:** rigorous structured logs with stable `event` names; ERROR means a human must act (`docs/radix/operations.md` §2). Apply the same rules to the web server.

## 2. Goal

1. **Web server:**
   - Becomes an HTTP client of Radix's snapshot endpoint.
   - Redistributes the snapshot to browsers as `/api/db` with the same ETag.
   - Renders its server-side fallback pages from the views.
2. **Frontend:** a rewrite that is coherent, correct, fast and stable, on the same page structure:
   - one design system instead of many styles
   - all data through the v2 views
   - no crashes, and every error visible to the user
3. **Checks** that keep it that way: data-layer tests against a real snapshot, a browser smoke walk, lints.

## 3. Verified problems in today's web tier

### A. Crashes (root cause found)

- In WASM a Rust panic cannot unwind: one panic freezes the whole app until reload, and there is no error screen.
- The program pages crashed when switching from one program to another: via the sidebar's „Studiengangsseite & Satzungen öffnen", via Back/Forward, or by any direct jump. A debug build reported:
  > At frontend\src\program_detail.rs:252:106, you tried to access a reactive value which was defined at frontend\src\app.rs:469:56, but it has already been disposed.

  **Mechanism:** pages are swapped by a `<For>` keyed on a tuple (`app.rs`), and they subscribe to the App-level `program_tab` signal. Navigating sets `program_tab` and the page id in the same step. The outgoing page's effects then run **once more after the page was disposed** and read page-owned values: first a `Signal` created by `.into()` inside the `<For>`, then the page-local `selected_area_filter` (`program_detail.rs:38`).
- **A stopgap is in the working tree, uncommitted** (`frontend/src/app.rs`, `frontend/src/query.rs`):
  - the tab became part of the `<For>` key and reaches the page as `Signal::stored`
  - the signal wrappers are created at App level

  On a debug build it passed 364 program visits with fast tab/chip/variant clicks, the sidebar path and Back/Forward, with zero errors. The rewrite must remove the pattern itself (§5 rules R1–R3). Commit the stopgap separately or drop it; don't build on it.

### B. Data access

- All SQL in `frontend/src/db.rs` targets v1 tables and parses free text with `LIKE` (turnus, language, exam form, campus …). v2 replaces this with enum/flag columns; see `docs/radix/schema-v2.md` §4.
- Every query error is swallowed (`unwrap_or_default()`), so a broken query looks like "no results".
- `LIMIT 300` combined with a total from another query produces wrong headers („285 von 4908").
- The department dropdown is always empty: its memo runs before the DB is loaded and never re-runs.
- The selected program lives in two places (`selected_program_*` signals and `filters.program_id`), synced by effects.
- Slug logic exists twice (`frontend/src/query.rs`, `folia/crates/server/src/slug.rs`). v2 has `v_program.slug`, so both can go.
- `frontend/static/sqlite_bridge.js`:
  - every query result travels as a JSON string
  - statements aren't freed when a query throws
  - `saveSnapshot` calls an undefined `reject`

### C. Inconsistent UI

- **Stylesheet** (`frontend/static/app.css`, 4,629 lines): 114 distinct hex colors but only 16 CSS variables, 56 font sizes, 30 box-shadows, 38 `!important`.
- **About 100 inline `style=` attributes** in the Rust code (37 in `program_detail.rs` alone).
- **The same element built several ways:** 4 accordion variants (the `Accordion` component itself is unused), 8+ badge/pill/chip families, emoji used as icons.
- **Server-rendered pages** (`folia/crates/server/src/html.rs`) use their own markup, and `/studiengaenge` has an inline `<style>`. The first paint therefore looks different from the app.

### D. Structure and performance

- **One `filters` signal holds everything:** data filters plus accordion state, view mode and sort. Opening an accordion re-runs the query and re-renders the catalog.
- **The whole catalog view is one closure:** every keystroke rebuilds both tables and re-evaluates prerequisites for every row.
- **Routing is hand-written:** `pushState` in 5 places, a `replaceState` effect, and a global `popstate` listener. Listeners are registered inside `Effect`s and `forget()`-ed.

### E. Server

- `/api/db` reads the v1 file from disk on every request, with ETag = file size.
- The SSR catalog renders all modules into one HTML page.

## 4. The data contract (what the UI builds on)

**Verified in the browser's SQLite.** Every public view works in the sql.js build the frontend ships (`frontend/static/sql-wasm.js`, SQLite **3.45.2**), loaded with the real snapshot. Measured in Node:

| Query | Time |
|---|---|
| filtered catalog page (100 rows) | 6.5 ms |
| exact count for the same filter | 1.1 ms |
| curricular modules of Informatik B.Sc. | 4.8 ms |
| module detail | 0.8 ms |
| `COUNT(*)` of the largest view (`v_module_program_link`, 30,191 rows) | 71 ms |

So SQL on the views is fast enough for live filtering. An in-memory index is not required.

**Rules for the frontend's data access**

- Read only `v_*` views (and `program_coverage`). Never base tables, never `v_*_src`.
- Filter and sort on view columns. No `LIKE` on free text except against `v_module_search`.
- Every list shows an exact total (a `COUNT(*)` with the same filter) and pages through the rest.
- If the contract lacks something, don't work around it in the frontend:
  - **Additive changes** (a new column or view) may be made in the backend: a new migration in `radix/internal/catalogdb/migrations/`, a test in `radix/internal/catalogbuild/build_test.go`, and `docs/radix/schema-v2.md` updated. `go test ./...` and `radix validate` must stay green.
  - **Changing the meaning** of an existing column needs the owner.

**Data facts the UI must handle** (from the current snapshot)

| Topic | Fact | UI consequence |
|---|---|---|
| Module kind | `v_program_module.kind`: compulsory 2,145 · elective 3,548 · thesis 159 · internship 21 · fues 3 · **NULL 4,712** curricular pairs | Show „Art nicht angegeben". Never default to Pflicht. Explain the source via `kind_source` / `kind_basis`. |
| FÜS | `relation`: curricular vs `fues` (17,829 pairs); each program has its own FÜS list; 288 modules have `is_fues` | With a program selected, show its FÜS list as a separate section. Informatik B.Sc. = 109 curricular + 116 FÜS. |
| Semester in a program | `plan_semester` known for 1,483 pairs, only from validated plans (140 programs have one) | Only show semesters that are stated. Programs without a plan get the explicit empty state (as today). |
| Degree | `degree_label` (B.Sc., M.A. …) only where the sources state it; `degree_display` falls back to „Bachelor"/„Master"; NULL for the 4 „Orientierungsstudium" programs | Use `degree_display`, else `degree_raw`. Never invent B.Sc. |
| Offer status | active 2,864 · not_offered 1,704 · phase_out 340 | Default filter is an open question (8). |
| Language | `teaches_german` / `teaches_english`: 3,928 German, 980 English | No hidden default. v1 hid all English modules. |
| Turnus | `turnus_season` winter 1,895 · summer 1,617 · both 604 · irregular 691 · NULL 2; `turnus_parity` even/odd for 99; filter via `offered_winter` / `offered_summer` | Use the facets. The quick filter „nächstes Semester" is computed from `v_semester`, not hard-coded („WiSe 26"). |
| Exam form | codes `map`, `prereq_map`, `mca`, `prereq_mca`, `other`, NULL; defined in `radix/internal/normalize/normalize.go`, allowed values in the CHECK of `migrations/0002_canonical.sql` | One label module in Rust, plus a test that reads the CHECK constraints from the snapshot and fails on any code without a German label. The same applies to every enum. |
| Grading | `is_graded`: 4,623 graded, 283 ungraded, 2 unknown | The filter works now (it returned 0 results in v1). |
| Schedules | Events keyed by semester (`v_semester`: `2026S` current, `2026W` with 1 event). Teaching events are pruned one month after their last date. Right now 526 modules have a recurring schedule and 634 have exam dates; exams are in `v_module_exam`, separate. | Label every schedule with its semester („SoSe 2026"). Show exams separately. Handle the gap before BTU publishes the next semester by falling back to turnus (owner decision Q5). |
| Campus | `at_*` flags are NULL (unknown) unless the module has a room in its newest semester; currently known for 234 modules | Tri-state: unknown must not look like „no". The campus filter should say it only covers modules with room data. |
| Search | `v_module_search` holds raw id / German title / English title (14,708 rows). SQLite `LIKE` folds ASCII only, so „übung" misses „Übung" (81 terms contain upper-case umlauts). | Either load the search terms once and match in Rust with Unicode folding (the existing `frontend/src/fuzzy/` engine can help), or add a folded column in the backend. Open question 6. |
| Freshness | `v_meta`: `data_changed_at`, `current_semester`, per-source newest/oldest fetch | Show „Datenstand" and the semester somewhere unobtrusive. |
| Program URLs | `v_program.slug` (e.g. `bachelor-informatik-2008`, `bachelor-soziale-arbeit-2020-84`); `v_program.id` is `<stg>-<abschl>-<pversion>` | Route by slug; no slug logic in the web tier. |

## 5. Target architecture (recommendation; deviate with a reason)

### Web server (`server/`)

- **Snapshot client:**
  - Poll `{FOLIA_SNAPSHOT_URL}` (e.g. `http://127.0.0.1:8090/snapshot/catalog.db`) with `If-None-Match`, at a configurable interval with backoff.
  - Download to a temp file, check it opens, then switch atomically. Keep the previous file while requests still use it.
  - Serve the last good snapshot when Radix is down; return 503 until the first snapshot exists.
- **`/api/db`:** serves the snapshot bytes with Radix's ETag, `304` on `If-None-Match`, and a sensible `Cache-Control`.
- **`/api/status`:** etag, bytes, `data_changed_at`.
- **`/healthz`:** fails when there is no snapshot, or it is older than a configured limit.
- **SSR fallback pages** are rendered from the same views with the same CSS classes as the app, so the first paint matches the hydrated look. How much SSR to keep is open question 1.
- **Structured logging** with stable event names (`tracing`, JSON in production). Configuration via flags and env. Same binary on Windows and Linux.

### Frontend (`frontend/`)

Leptos stays. Upgrading to the current Leptos release is allowed (no compatibility constraint); decide in a short spike at the start, and treat the disposed-signal behavior in §3A as one criterion.

```
frontend/src/
  app.rs        shell only: providers, router, layout, error boundary
  routes.rs     Route ⇄ URL; catalog filters as readable query parameters
  data/         bridge.rs (the only JS interop; Result<T, DbError>),
                rows.rs (structs mirroring the views), queries.rs (one fn per query, SQL only here)
  domain/       CatalogQuery (typed filter state), labels for every enum, prerequisite status
  state/        user data (bookmarks, completed; versioned localStorage keys), sidebar UI state
  ui/           design system: Button, Chip, Badge, Segmented, Accordion, Tabs, Card, DataTable,
                EmptyState, ErrorState, Skeleton, Icon (one inline-SVG set)
  pages/        catalog/, module.rs, program/ (plan, areas, modules), programs.rs, not_found.rs
  styles/       tokens.css, base.css, components.css (no inline styles)
```

**Rules**

- **R1. Navigation state reaches pages as plain values.** Only the router (or one closure at the top) reads the location. A page never subscribes to a signal that changes in the same step as the page is replaced.
- **R2. No page-owned value is read after unmount.** No `.into()` signal wrappers created inside a keyed or conditional block and handed to children. No `forget()`-ed listeners or timers that touch signals; use cleanup-aware listeners.
- **R3. No panics.** Deny `clippy::unwrap_used`, `expect_used` and `indexing_slicing` in the crate. A panic hook shows „Etwas ist schiefgelaufen – Seite neu laden" instead of a frozen page. DB errors render `ErrorState` with a retry.
- **R4. State is split.** The typed `CatalogQuery` is what the URL encodes and what triggers queries. Accordion open/closed and similar UI state never trigger a query.
- **R5. Keyed lists.** `<For key=id>` rows with per-row derived state, so toggling a bookmark updates one row. Page long lists („Weitere laden" or virtualization) with the exact total in the header.
- **R6. One source of truth per piece of state.** No effects that copy one signal into another.
- **R7. Design system.**
  - Tokens for color, spacing (4 px scale), about 6 font sizes, 3 radii and 2 shadows; light and dark.
  - Primitives cover everything. Zero inline styles, checked in CI.
  - One icon set instead of emoji. German UI strings in one place.
- **R8. Accessibility basics:**
  - everything works by keyboard, with visible focus
  - tabs and comboboxes have correct ARIA roles and states
  - sufficient contrast
  - usable on a phone (the sidebar becomes a drawer, as today)

**Delivery in the browser:**
- Keep caching the snapshot in IndexedDB keyed by ETag. On start, compare with `/api/status` and download only when it changed.
- Show download progress and a clear error with retry.
- Version the service worker's cache so new app builds replace old assets.

## 6. Pages and features to keep (same flow, rebuilt)

**Top bar**
- Search with keyboard-navigable suggestions (id, German and English title).
- Link to the program overview, share link, „Gemerkt" and „Bestanden" views with counts.

**Catalog** (`v_module_facets` ⋈ `v_module`)
- Sidebar groups:
  - **Studiengang:** program + PO (from `v_program_version`), Fachsemester (`plan_semester`), Modulart (`kind`).
  - **Dozierende:** include/exclude (`v_module_lecturer`), Fachgebiet (`v_department`).
  - **Turnus & Lehrformen:** turnus, teaching forms (`has_*`), duration (`duration_semesters`).
  - **Kriterien:** participant limit, FÜS, exam form, graded, „Voraussetzungen erfüllt" (`v_module_prerequisite` + completed set), hide phase-out/not-offered, ECTS range.
  - **Standort & Sprache.**
  - „Filter zurücksetzen" with the active-filter count.
- Table columns: status (bookmark, completed), ID, title (+ other language), ECTS, turnus, language, prerequisites, schedule; sortable.
- With a program selected: curricular and FÜS as separate sections.

**Module page** (`v_module` and satellites)
- Header badges, key facts, successor banner (`v_module_successor`).
- Prerequisite status with „jetzt abhaken".
- Texts, teaching forms, exam form and details, literature and course lists (`v_module_text_item`), remarks.
- Weekly calendar and event list per semester (`v_module_schedule`); exams separately (`v_module_exam`).
- Programs (`v_module_program_link`: relation, kind, area, PO).
- Link to the source page (`source_url`).

**Program page** (`v_program`)
- Header (degree, PO), Bachelor/Master counterpart (`v_program_counterpart`, highest `match_score`), PO switcher (`v_program_version`), documents (`v_program_document`), key numbers.
- Tabs:
  - **Regelstudienplan** (`v_program_plan` / `v_program_plan_entry`): the existing semester grid with variants, joint windows, workload and a link to the PDF page.
  - **Wahlpflicht & Bereiche:** the area tree from `v_program_module_area`.
  - **Alle Module:** curricular and FÜS.

**Program overview** (`program_coverage` + `v_program`)
- Today this exists only as a server page (`/studiengaenge`); bring it into the app.

**User data**
- Bookmarks and completed modules in `localStorage` under new, versioned keys. Module IDs are unchanged in v2.

## 7. Deliverables (phased; the app works after each phase)

0. **Setup.**
   - Branch from `backend-data-overhaul`.
   - Leptos version spike.
   - A native test harness: `cargo test` for `data/queries.rs` against a real snapshot via rusqlite as a dev-dependency (not compiled to WASM). It asserts the numbers in §4 and runs every query once.
   - A browser smoke walk (Playwright, or an in-page script driven by any runner). It visits every program with every tab, the area chips and the plan variants, navigates fast between programs, uses Back/Forward, and fails on any console error.
1. **Web server on v2:**
   - snapshot client, `/api/db`, `/api/status`, `/healthz`, logging
   - SSR fallback pages from the views
   - v1 code and `slug.rs` removed
2. **Frontend core:**
   - bridge, row structs, queries, labels
   - router, state, panic/error handling (R1–R6)
   - the existing pages ported just enough to work end to end against v2
3. **Design system and page rebuilds:**
   - tokens and primitives
   - then catalog, module page, program page, program overview
   - old CSS and components deleted as each page is replaced
4. **Hardening:**
   - PWA and caching, performance and accessibility pass
   - `docs/folia/frontend.md` (architecture, rules R1–R8, how to run against a local `radix serve-snapshot`)
   - build and container for the web tier (open question 7)
   - CI: `cargo test`, clippy, `trunk build --release`, the inline-style check, the smoke walk

## 8. Constraints

- **Don't change what Radix does.** Backend changes are limited to additive contract changes (§4), reported to the owner.
- **No crawling.** Develop against the existing snapshot or a local `radix serve-snapshot --addr 127.0.0.1:8090`.
- **Keep the tests green:** `go test ./...`, `cargo test` (workspace), clippy.
- **Work on Windows and Linux.** No secrets in code or config.
- **Tooling note for this machine:**
  - Edit source files with the editor tools, not Python heredocs; heredocs have corrupted backslashes here before.
  - Check reads for side effects before using them on stored data.

## 9. Open questions for the owner (bring evidence and a recommendation, don't decide alone)

1. **SSR depth:**
   - (a) full server-rendered pages for every route (no-JS users, search engines), or
   - (b) a minimal SSR shell plus module/program pages for sharing and SEO?
2. **Default catalog filters:** hide `not_offered` (1,704) and `phase_out` (340) modules by default?
3. **FÜS without a program selected:** show all 288 `is_fues` modules as their own section, or mix them into the list with a badge?
4. **URL scheme.** No compatibility needed. Today it is `/catalog`, `/catalog/module/<id>`, `/study-programm/<slug>/<tab>` (with a typo). For example: `/module/<id>`, `/studiengang/<slug>/plan|bereiche|module`, `/studiengaenge`.
5. **Leptos:** stay on 0.7.8 or upgrade (spike result)?
6. **Search folding:** fold in Rust after loading the terms, or add a folded column to `v_module_search` in the backend?
7. **Web tier deployment:** container built with Nix in the same Swarm stack as Radix (`docs/radix/operations.md` §3–4), reaching it at `http://radix:8090`?
8. **Schedule gap:** what to show between the pruning of a semester's teaching events and the next semester's publication. The decision so far is to fall back to turnus; confirm the wording.

## 10. Acceptance criteria

- **The browser gets its data only through the web server.** The app runs end to end against a snapshot fetched over HTTP from `radix serve-snapshot`, and serves it to the browser as `/api/db`; no shared files.
- **Only the contract is read.** `grep` finds no v1 table names and no base-table access in `server/` or `frontend/`; all SQL lives in `data/queries.rs` (frontend) and one module in the server.
- **Zero console errors in the smoke walk** on a debug build: all 182 programs × all tabs with fast navigation, Back/Forward, and the sidebar „open program" path.
- **Every list header shows the exact total.** Filter results match direct SQL on the views for a fixture set (e.g. `offered_winter = 1 AND has_exercise = 1` → 979 modules on the current snapshot; update the fixture when the snapshot changes).
- **Unknown values look unknown:** kind, degree label, semester, campus and grading are never guessed.
- **Every enum code has a German label,** enforced by the test in §4.
- **The design system is the only styling:** no inline `style=`, colors only from tokens, one icon set.
- **Fast enough:**
  - a filter change updates the table in under 100 ms on a desktop
  - a start from the IndexedDB cache shows the catalog in under 2 s
- **Documented:** `docs/folia/frontend.md` exists and `README.md` describes how to run the web tier.

## 11. Entry points

- **Contract:** `docs/radix/schema-v2.md` §3 (views), §4 (query map), §8 (events); `radix/internal/catalogdb/migrations/` (view SQL, CHECK constraints); `radix/internal/normalize/normalize.go` (enum meanings).
- **Service:** `docs/radix/operations.md` §1 (endpoints, flags), §2 (logging rules), §3–4 (secrets, Swarm, Nix).
- **Web server today:** `folia/crates/server/src/main.rs`, `routes.rs` (`/api/db`, pages), `db.rs` (v1 SQL), `html.rs` (SSR), `slug.rs`.
- **Frontend today:**
  - `frontend/src/app.rs` (shell, routing, `<For>` page swap)
  - `db.rs` (all SQL), `models.rs`, `query.rs` (URL codec, slugs)
  - `detail.rs`, `program_detail.rs`, `plan_view.rs`, `study_plan.rs`
  - `components/`, `fuzzy/`
  - `static/sqlite_bridge.js`, `static/sw.js`, `static/app.css`
- **Snapshot for development:** `snapshot/current.json` names the current file. Open it read-only for analysis; it is replaced, never modified.
