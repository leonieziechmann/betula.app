# Frontend rewrite, phase 0: results and open questions

> Branch `frontend-rewrite` (from `backend-data-overhaul`), 2026-09-19.
> Brief: `docs/frontend-rewrite.md`. Everything below was measured on this machine against
> the snapshot `catalog-1d4691d0656871d6.db` (content digest `e20a744c…`).

## 1. What exists now

| Deliverable | Where | State |
|---|---|---|
| Branch; stopgap committed separately | `4b2f504` | done; the rewrite does not build on it |
| Shared data crate with the query test harness | `catalog/` | 8 tests green against the real snapshot; clippy clean; builds for `wasm32-unknown-unknown` |
| Leptos version spike | `spikes/leptos-dispose/`, `spikes/leptos-ssr/` (deleted once the results below stood here; in the history) | done, results in §2 |
| Browser smoke walk | `e2e/smoke-walk.js` (in-page), `e2e/run.mjs` (Playwright runner) | runs; verified against the SSR spike |

### The data crate (`catalog/`)

One crate for the web server **and** the browser app, so SQL, row structs, filter logic and
labels exist once (the old tier had slug logic twice and would have had every query twice):

- `db.rs`: the seam. `trait Database { fn query(name, sql, params) }`, typed row access by
  column name, `DbError` (`Unavailable`, `Sql`, `Decode`). Nothing is swallowed.
- `native.rs`: rusqlite implementation (feature `native`; never compiled to WASM).
  The browser gets its own implementation on top of sql.js in phase 2.
- `rows.rs`: structs mirroring the views. `Option` = the source does not say.
- `labels.rs`: every enum code with its German label. `Code<E>` carries a code this build
  has no label for instead of failing (a new kind of program or event must not break a page).
- `filter.rs`: `CatalogQuery`, the typed filter state, and its translation to SQL on the
  facet columns. `LIKE` only against `v_module_search`, with `%`/`_` escaped.
- `queries.rs`: one function per query; only `v_*` views.

The tests fail without a snapshot (`FOLIA_TEST_SNAPSHOT`, else `snapshot/current.json`):

- `catalog_filters_match_direct_sql`: 13 filter combinations, each compared with
  hand-written SQL on the views, each paged to the end (exact total, no module twice or missing).
- `snapshot_facts_of_the_brief`: the numbers of the brief §4. Exact numbers are asserted only
  for the pinned content digest, so a newer snapshot does not break the build; the relations
  (curriculum + FÜS list never overlap, counts equal `v_program`) always are.
- `every_enum_code_has_a_label`: reads the CHECK constraints from the snapshot's schema
  (31 enum constraints) and fails for a code without a label. Verified by mutation.
- `every_query_runs_against_the_snapshot`: fails for a `pub fn` in `queries.rs` the tests never ran.
- `unknown_stays_unknown`, `search_text_is_literal`, `lecturer_and_bookmark_filters`,
  `errors_are_reported_not_swallowed`.

Phase 0 covers the core queries (meta, semesters, departments, programs, program by slug,
program modules, catalog count/page, module, prerequisites, suggestions). The remaining ones
(schedule, exams, plan, areas, documents, versions, counterpart, lecturers, text items) are
added in phase 2; the coverage test forces each of them to run against real data.

Two numbers in the brief's table are off and corrected in the tests: `turnus_season` is
winter 1,950 / summer 1,661 (the brief's 1,895 / 1,617 leave out the 99 modules with a year parity).

### The smoke walk (`e2e/`)

`smoke-walk.js` runs inside the page and needs no tooling; `run.mjs` drives it with Playwright
(`playwright-core`, 1 package). It can use an installed Edge/Chrome (`SMOKE_BROWSER_CHANNEL=msedge`),
so nothing has to be downloaded on Windows. Pages take part through `data-walk` attributes, so the
walk is independent of design and URL scheme. It fails on console errors, page errors, failed
requests, 5xx responses, hydration warnings, a program page that does not render, and on any full
page load during client-side navigation.

Against the SSR spike: 182 programs, 546 tab visits, 1,387 chip clicks, 182 fast program-to-program
jumps with Back/Forward, 24 s. It reported one real defect (no favicon → 404) and exited 1.

## 2. Spike results

### 2.1 The disposed-signal crash (§3A of the brief)

`spikes/leptos-dispose`: 70 lines that copy the pattern (pages swapped by a keyed `<For>`, a
`Signal` made with `.into()` inside it, attribute closures of the page reading it, page id and
tab changing in one step). Same source, two versions:

| Leptos | Result |
|---|---|
| 0.7.8 | Panics after 6 page changes with the exact message of the brief (`…you tried to access a reactive value … already been disposed`); the app is frozen afterwards. Only when the page id is set *before* the signal the page subscribes to. |
| 0.8.20 | 1,100 page changes in three cadences, 0 errors. |

So 0.8 no longer runs the outgoing page's render effects after disposal. The rules R1–R3 stay
(a panic in WASM is still fatal), but the framework stops punishing this mistake.

### 2.2 Server rendering + hydration

`spikes/leptos-ssr`: Leptos 0.8.20, `leptos_axum`, axum 0.8, `leptos_router`; the same
components render on the server and hydrate in the browser; data through `catalog` (rusqlite on
the server, server functions after hydration).

- **Without JavaScript** every route is complete HTML with the right numbers (`/katalog?winter=1&uebung=1`
  → 979 modules; Informatik B.Sc. → 109 + 116; `<title>` per page; 404 for unknown routes).
  The catalog filter is a `<Form method="GET">`: a plain form without JS, client-side navigation with it.
  The URL is the only filter state.
- **Render time** 14–28 ms per page, uncached, including opening SQLite per request.
  Nothing in the HTML depends on the user (bookmarks etc. live in the browser), so every page
  can be cached by URL and dropped when the snapshot's ETag changes, exactly as the owner proposed.
- **Hydrated**: 182 programs through client-side navigation, 407 chip clicks, then 182 direct
  program-to-program jumps in 5.5 s with Back/Forward: 0 errors, 0 hydration warnings, no full page load.
  Route parameters come from the router, so a page never owns navigation state (rule R1 by construction).
- **Size**: hydration bundle 477 KB gzip without `wasm-opt` (today's CSR app: 411 KB gzip with
  `wasm-opt`, plus sql.js 0.65 MB).
- **Tooling finding**: `cargo-leptos` does not build on this machine (`openssl-sys` on the
  `x86_64-pc-windows-gnu` toolchain). Not needed: `cargo build --target wasm32-unknown-unknown`
  + `wasm-bindgen` (the binary Trunk already caches, version pinned to the crate) + `cargo build`
  for the server is three commands, identical on Windows, Linux and in Nix. Leptos 0.8 expects
  `pkg/<name>.js` and `pkg/<name>_bg.wasm`, the names `wasm-bindgen` writes.

### 2.3 What this means for the architecture

The brief assumed a CSR app plus separately written server pages ("first paint looks different
from the app" is its own problem C). With the owner's goal of full SSR the better shape is:

```
catalog/   shared: rows, labels, CatalogQuery, queries            (exists)
app/       Leptos components, isomorphic (features ssr / hydrate) = the brief's frontend/src layout
server/    axum: snapshot client, HTML cache, leptos routes, /api/db, /api/status, /healthz
client/    the WASM entry point (hydrate)
```

One set of components and one CSS, so SSR and app cannot drift apart. Two rules join R1–R8:

- **R9. Server HTML is user-independent.** Bookmarks, passed modules, the chosen major are applied
  after hydration (in effects), never during the first render. This keeps pages cacheable and
  prevents hydration mismatches.
- **R10. Every route has a logical parent** (the UI graph), so Esc and the back affordance are defined:
  Esc closes the topmost overlay (suggestions, filter sheet, dialog), else goes up:
  `Modul → the list or program it was opened from, else Katalog`; `Studiengang → Studiengänge`;
  `Gemerkt/Bestanden → Katalog`. Going up uses `history.back()` when the previous entry is that
  parent (scroll position and filters come back), else navigates to it.

## 3. Open questions (brief §9) with evidence and recommendation

1. **SSR depth.** Answered by the owner on 2026-09-19: full server rendering for every route,
   search engines and no-JS users included; rendered HTML cached on read, invalidated when the
   snapshot changes. Spike 2.2 shows it works. *To confirm:* without JS you can read and filter
   everything; „Gemerkt", „Bestanden", „mein Studiengang", fuzzy suggestions and the prerequisite
   check need JS because they live in the browser.
2. **Default catalog filters.** Of the 1,704 `not_offered` modules 1,649 are in no curriculum at
   all and only 43 in a curriculum of a current PO; 553 have a successor; 2 have events this
   semester. Of the 340 `phase_out` modules 239 are still in current curricula.
   *Recommendation:* hide `not_offered` by default, keep `phase_out` visible with a badge; one
   visible switch „auch nicht mehr angebotene Module (1.704)"; inside a selected program show
   everything the curriculum lists.
3. **FÜS without a program.** 262 of the 288 FÜS-list modules are also in some curriculum, so a
   separate section would duplicate them. *Recommendation:* one list, a FÜS badge, and the FÜS
   filter (only / none). The separate FÜS section appears once a program is selected, because
   then it is that program's own list.
4. **URL scheme.** *Recommendation:* German, flat, readable:
   `/` catalog (filters as query parameters, e.g. `?q=algebra&turnus=winter&lehrform=uebung`),
   `/modul/<id>`, `/studiengaenge`, `/studiengang/<slug>` with `/plan`, `/bereiche`, `/module`,
   `/gemerkt`, `/bestanden`. Later: `/planer`.
5. **Leptos.** *Recommendation:* upgrade to 0.8.20 (§2.1, §2.2), SSR + hydration, no islands
   (the app is one interactive surface), no `cargo-leptos`.
6. **Search folding.** „ökologie" finds 8 modules with SQLite `LIKE` and 31 when folded;
   „ubung" 0 vs 4. Search is part of `CatalogQuery`, and totals and paging happen in SQL on
   both sides (server and browser). *Recommendation:* an additive backend change: a
   `term_folded` column in `v_module_search` (lower case, ß→ss, diacritics removed; the same
   function in Go at build time and in Rust on the query text). Fuzzy suggestions in the top
   bar can still load the 14,708 terms once (149 KB gzip) and use the existing `fuzzy/` engine.
7. **Web tier deployment.** *Recommendation:* yes, same pattern: `flake.nix` gets `web` and
   `web-container`; one Swarm stack; Radix is reachable only inside the stack network as
   `http://radix:8090`, only the web tier publishes a port; same logging rules.
8. **Schedule gap.** Today 525 modules have a schedule, all in SoSe 2026, which ends on 30 Sep;
   1,552 active winter modules have nothing for WiSe 2026/27 yet. *Recommendation:* show the
   newest semester's schedule with its label. When it is not the coming one, add:
   „Termine aus dem SoSe 2026. Für das WiSe 2026/27 hat die BTU noch keine Termine veröffentlicht."
   When there is none: „Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul
   im Wintersemester angeboten."

New questions that came out of phase 0:

9. **Where do queries run after hydration?** The earlier decision was: the browser downloads the
   SQLite file (36.8 MB, 6.6 MB gzip, about 40 MB of memory) and queries locally. With full SSR
   the first paint no longer needs it. Options: (a) local as before, server functions only until
   the download finished; (b) server only: simplest, always fresh, but every filter change is a
   round trip and there is no offline mode; (c) server first, local database as an upgrade:
   automatically on desktop and unmetered connections, on mobile after „Offline verfügbar machen".
   The shared crate makes (a) and (c) cheap: the same query functions run on both sides.
   *Recommendation:* (c), built as (b) first (MVP) with the seam in place.
10. **One Leptos app instead of CSR + server templates** (§2.3): crates `catalog`, `app`,
    `server`, `client`; `frontend/` and the old `server/` are replaced, not ported.
    *Recommendation:* yes.
11. **Mobile layout.** *Proposal:* bottom navigation (Katalog, Studiengänge, Gemerkt; Planer later)
    instead of the sidebar; filters in a bottom sheet with a live „123 Module anzeigen" button,
    swipe down to close; module rows become cards; program tabs swipe horizontally (CSS scroll
    snap); the browser's own edge-swipe back keeps working because every view is a real history
    entry; page transitions with the View Transitions API where available. Built CSS-first
    (`dialog`, `popover`, scroll snap), so most of it works before or without WASM.

## 4. Owner decisions (2026-09-19)

| # | Decision |
|---|---|
| 1 | Full SSR, mainly for search engines; a shared link must open without JavaScript. Less comfort without JS is accepted. |
| 2 | Default catalog: hide `not_offered`, keep `phase_out` with a badge, one switch to show everything; inside a program the whole curriculum. |
| 3 | FÜS: with a program selected, its FÜS section lists only what that program accepts as FÜS. To see all FÜS modules, select „Alle Studiengänge". |
| 4 | URLs: `/` is a landing page that lists every function with a link; `/catalog`, `/catalog/module/<id>`, `/programs`, `/programs/<slug>/…`. No `/gemerkt`, `/bestanden` for now. |
| 5 | Leptos 0.8.20, SSR + hydration, no cargo-leptos. |
| 6 | Search is planned separately (§5): it has to react to the selected context, not just fold umlauts. |
| 7 | Web container built with Nix, same Swarm stack, Radix reachable only inside the stack. |
| 8 | Schedule gap: the short note under the schedule, as proposed. |
| 9 | **Queries run in the browser** on the downloaded snapshot: lowest latency on click, lowest server load. Stale data for a while is fine. No query API on the server. |
| 10 | One Leptos app: crates `catalog`, `app`, `server`, (`client`). |
| 11 | Esc only for what feels like a popup. |
| 12 | Mobile concept: left to the implementation. |

Consequence of 9, found in phase 1: because SQLite answers synchronously on both sides, pages
are synchronous functions of their route parameters. Nothing is serialized into the HTML for
hydration (the catalog page went from 314 KB to 74 KB, 13 KB gzipped), and until the local
database is ready the site simply behaves like a classic website served from the HTML cache.

## 5. Search: concept for discussion (not decided)

What the owner asked for: the search reacts to the selected data. With a program selected its
modules are boosted, but modules outside the program can still be found, at least in the
suggestions.

- **One box, typed results** (it should feel like a command palette): modules, programs,
  lecturers. Enter on a module opens it; Enter on plain text filters the catalog.
- **Ranking = text match × context.** Text: exact id > id prefix > title starts with > word
  starts with > substring > fuzzy (typos), on folded text (case, ß→ss, diacritics; „okologie"
  finds „Ökologie": 31 modules instead of 8). Context boosts, strongest first: module of the
  selected (or „my") program's curriculum › of its FÜS list › offered in the coming semester ›
  currently offered › same department as the module being viewed.
- **Suggestions never filter by context, they only order by it**, in two groups: „In deinem
  Studiengang" first, „Weitere Module" below. The catalog table keeps its filters, and says so
  when the text also matches outside them: „12 weitere Treffer außerhalb von Informatik B.Sc.".
- **Where it runs:** in the browser, in Rust, on an index built once from `v_module_search`
  (14,708 terms, 149 KB gzip, already part of the snapshot) plus the facets needed for the
  boosts. The existing `frontend/src/fuzzy/` engine is the starting point. The folding function
  lives in `catalog`, so the server uses the same one for the no-JS fallback (`/catalog?q=…`),
  which stays a plain list without the grouping.
- **Backend:** nothing required. Optional later: a `term_folded` column so the no-JS `LIKE`
  search folds too.
- Open: should „my program" (remembered major) boost everywhere, or only while it is selected
  as a filter? Should lecturers and programs appear in the same suggestion list from the start?
