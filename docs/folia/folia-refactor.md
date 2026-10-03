# Folia, restructured: a concept for the web tier's next architecture

> Draft, 2026-10-02; the owner's answers of the same day are in §0.1 and worked in. What is
> still open is in §12; a minimal version (§11, phase 0) tests the plan before it is final.
> Verified against `develop` at `b74d3fb`.
> Companion documents: `docs/folia/frontend.md` (Folia as it is: architecture, rules R1–R23, checks),
> `docs/history/frontend-rewrite.md` and `docs/history/frontend-phase0.md` (the rewrite of 2026-09-19 and its
> decisions).

## 0. The brief

Owner, 2026-10-02:

- „ich habe mittlerweile die anforderung abgelegt, dass ohne js und mit js jeweils funktionieren
  muss. Ich würde nur zwei ruten ohne js anbieten halt die startseite und den modulkatalog + pages."
- „Aber ich möchte die interaktive app architektonisch komplett umgestalten. Zum beispiel hat ja
  jede seite den header, die sidebar mit der navigation und einen footer."
- „dann soll auch viel mehr asynchron laufen vielleicht sogar mit ein bis zwei service workern,
  so dass berechnungen nicht blockieren"
- „Es muss viel mehr mit eigenen crates gearbeitet werden. Zum beispiel könnte die suche eine
  eigene crate sein … crates für das design … Alles so dass jedes system möglichst abgeschlossen
  ist."

### 0.1 The owner's answers (2026-10-02)

| # | Question | Answer |
|---|---|---|
| 1 | Routes without JavaScript | the start page, the catalog with the module pages, the legal pages („Impressum und Datenschutz muss auch so gehen") |
| 2 | Program pages | they stay for Google („die Studiengangsseiten sollen weiter für google existieren"): the program overview and every program's page are site pages too |
| 3 | How much the site can do | as little as possible, the design consistent with the app („mach die komplette ssr seite minimal in ihrer Funktionalität nur das Design soll konsistent sein") |
| 4 | A first visit to an app route | loading is fine, but the first paint as fast as possible and the rest of the app loaded after it |
| 5 | What „die Sidebar mit der Navigation" meant | the icon rail, the header, the footer and the background: what every page has; the content, sidebars included, changes from page to page |
| 6 | Pace | first a minimal version to find the problems while development goes on; once the plan is final, Folia's development pauses until the restructuring is done |
| — | The repository | „komplett unordentlich": it is ordered and structured as part of this (§7.9) |
| — | Names | every crate of Folia is `folia-<crate>` |
| 7 | A new snapshot during a visit | at once („sofort rein mit den neuen daten", §6.2) |
| 8 | Framework | Leptos stays |
| 9 | Granularity | not more than 34 crates, rather fewer where they merge sensibly: 26 (§7.3) |
| 10 | The repository's new order | as proposed (§7.9) |

In one sentence: Folia becomes **a small public site** (a few server-rendered pages for search
engines, link previews and visitors without JavaScript) **plus a browser app** whose main thread
only draws, whose data and computations run in workers, whose frame is built once, and whose
systems are crates with their own API, state, texts and styles.

The proposal in five lines:

1. **Site and app** (§4): the server renders the start page, the catalog, the module pages, the
   programs and the legal pages, with as little function as possible; the Merkliste and the
   Stundenplan are the app alone, in a document whose head the server still writes.
2. **One shell** (§5): the icon rail, the header, the footer and the background are mounted once;
   a page brings its content, sidebars included, built from the design system's frame.
3. **Workers** (§6): a data worker holds the snapshot and runs every query, loader and
   computation; the semantic search keeps its worker; the service worker stays a cache.
4. **Crates** (§7): 26 crates in layers; features never use features; a test checks it.
5. **Steps** (§11): a minimal version first, beside the running development; then, with Folia's
   development paused, the repository put in order and the phases, each merged into `develop`
   with the site working.
6. **First paint first** (§6.7): the site's HTML or the app document paints at once, the app's
   code and the catalog follow in stages.

## 1. Where Folia stands

| Crate | Rust lines | What it is |
|---|---|---|
| `folia-app` (`app/`) | 32,600 — pages 19,700 (the Stundenplan alone 9,800), texts 5,600, the rest 7,300 | every page and component, built twice: feature `ssr` for the server, `csr` for the browser |
| `folia-catalog` (`catalog/`) | 29,600 — 17,600 of code and 12,000 of tests; the timetable 12,700 | the data contract and its SQL, the page loaders, the URL scheme, the search, the Stundenplan's logic, the program map, the texts of the data, a Markdown reader |
| `folia-server` (`server/`) | 7,100 | axum: snapshot client, HTML cache, `/api/*`, link-preview cards, calendar feeds, the access gate |
| `folia-pack`, `folia-semantic`, `folia-client` | 2,300 · 2,000 · 300 | link codes; the semantic search; the browser's entry with the sql.js bridge |
| `folia/assets` | `app.css` 3,270 lines, `enhance.js` 960, `boot.js` 274, `sw.js` 169 | one stylesheet, the behaviours written in JavaScript, start and offline |

What shapes the architecture today, and what it costs:

1. **One set of components, two renderers.** Every route is rendered by the server (complete HTML,
   also for visitors without JavaScript) and again by the browser app, which takes the page over.
   The price: 79 `#[cfg(feature = "csr")]` gates in `folia-app`; rules that exist only for this
   (R9 server HTML is user-independent, R15 `JsOnly`, R22 nothing thread-bound in a server
   render); a second form of every interactive control for the time without JavaScript (toggles
   as links to their next state, GET forms with hidden inputs, a `<select>` where the app has a
   picker); `enhance.js` smoothing the classic site; and `folia-app` compiled twice, once for each
   build (in the server's alone it is 35 of the 57 s of a fresh worktree, `docs/folia/frontend.md`,
   „Build times").
2. **Pages are synchronous functions of their route.** SQLite answers synchronously on both sides,
   so a page calls one loader of `catalog::pages` and renders. In the browser all of it runs on the
   main thread: sql.js behind a JavaScript bridge (10–13 ms of the 48–54 ms of queries of the start
   page and the program overview), then building the page (25–75 ms), then layout — on a phone's
   CPU 260–850 ms per click before `pending.rs` existed. `pending.rs` and `skeleton.rs` (920 lines)
   now paint the click's new state first and do the work after it, but the work still blocks: a
   tap during the start page's loader (70 ms on a laptop, 280 on a phone) waits for it, and a
   loader cannot be split (`docs/folia/frontend.md`, „A click answers first").
3. **Every page builds its frame.** The chrome lives in `App` (rail, top bar, bottom bar, crown,
   wood), but the sidebar, its handle, the page, the panel beside it and the ground at its end are
   `ui::Frame`, rendered by each page — or written out by hand: the catalog and the Merkliste carry
   copies of its markup, the start page one of `ui::Plain`, `skeleton.rs` replicas of all of them;
   the resize handle's markup stands six times, and the scroll area has three ids. A change of route
   remounts the whole page with its sidebar (also `/programs/<slug>` → `/programs/<slug>/areas`,
   which are two routes), the module page builds a new frame for every module, and the app loses
   the sidebar's scroll position on the way. „Nothing jumps" is a rule (R17), kept by matching
   classes and by widths kept on `<html>`, not by structure. And a new page means seven edits: the
   routes in `lib.rs`, `pending::Shape`, `pending::change`, `skeleton::frame`, `tabs::Area` with its
   memory, the navigation's items and the top bar's `match`.
4. **Two large crates hold everything.** `folia-app` has all pages, components and texts in one
   compile unit; `folia-catalog` mixes the data contract with the search, the Stundenplan's whole
   timetable logic, the URL scheme, the program map's layout and i18n.
5. **Shared files instead of owned ones, and patterns written many times.** One stylesheet, one
   folder of texts for all pages (a new group is three edits in `i18n/mod.rs`), one sprite, one
   `enhance.js` with a contract of `data-action` attributes and ids every page has to keep: a change
   of one feature edits files every feature shares. Meanwhile what should exist once exists several
   times: four stores in `localStorage` with the same mechanics, five closures for „Rückgängig",
   segmented controls and switches written by hand in nine places, 28 empty states written inline
   beside `ui::EmptyState`, the preview of a module loaded three times and drawn twice alike,
   `Ground` rendered (and its query asked) twice on every page.
6. **Memory and start.** The catalog (44 MB, 4.4 MB in brotli) is kept in IndexedDB and, once
   opened, held whole in sql.js's memory on the main thread. Only the semantic search already runs
   in a Web Worker (`folia/crates/semantic/js`), with its index built on the main thread from rows of sql.js.

What works and is kept: the data contract (`v_*` views, R11); page loaders as plain functions over
a `Database`, most of whose results are serializable already (`CatalogData`, `ModuleData`,
`ProgramData` … derive `Serialize`, a remainder of the hydration plan); the URL as the state that
can be shared; visitor data in the browser only (R20); unknown stays unknown (R12); a click
answering in the next frame (R21); the start without a network; the two languages (R23); the checks.

## 2. Goals and non-goals

**Goals**

- **G1** Two kinds of routes: the **public site** (server-rendered, complete without JavaScript)
  and the **app** (JavaScript only).
- **G2** **One shell**: the icon rail, the header, the footer and the background are built once;
  the pages change inside it.
- **G3** The **main thread draws** and takes input; queries and computations run in **workers**;
  data reaches the UI asynchronously, and every wait has a defined look.
- **G4** **Every system is a crate** with its own public API, state, texts, styles and tests; the
  dependencies between crates are rules, and a test checks them.
- **G6** **The first paint comes first**; the app's functions load after it.
- **G7** **An ordered repository**: Radix and Folia apart, every crate `folia-<crate>`.
- **G5** Nothing the owner approved gets lost on the way: the look and the interactions, the
  addresses, R12, R20, R21, the start without a network, both languages; the checks move along.

**Non-goals**

- No change in Radix or in the data contract (an additive view or column at most, as today).
- No query API on the server: queries keep running in the browser (owner decision 9 of
  2026-09-19). The server keeps serving cached HTML, files and one database file.
- No other framework: Leptos 0.8 stays (for the app CSR only, for the site SSR).
- No visual redesign as part of this. The shell makes one easier later.

## 3. The target picture

```
Radix ──/snapshot/catalog.db──▶ folia-server
                                 ├─ site pages, rendered and cached per snapshot (folia-site):
                                 │    / /catalog /catalog/module/<id> /programs… /impressum …
                                 ├─ the app document for the Merkliste and the Stundenplan: a head
                                 │    written per route (title, description, card), the shell
                                 └─ /api/db  /api/status  cards  calendar feeds  files  sitemap
                                                  │
┌─ a browser tab ─────────────────────────────────▼────────────────────────────────────────────────┐
│  main thread                       data worker (new)                  semantic worker (exists)   │
│  UI bundle: Leptos, CSR    ◀─────▶ worker bundle, no Leptos   ◀─────▶ the query model,           │
│  shell · features · design  typed  SQLite on the snapshot       port   the modules' vectors      │
│  DataClient (async)         messages page loaders, search,                                       │
│                                    timetable, plans, kept answers                                │
│                                    the snapshot: download, keep, update ◀── /api/db              │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│  service worker (one for the site, as today): the shell of a build, the model, the start offline │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## 4. Routes: the public site and the app

### 4.1 Which route is what

| Route | Today | New |
|---|---|---|
| `/` | server-rendered, the app takes over | **site** |
| `/catalog?…` | server-rendered with the whole filter panel as links and forms | **site**, minimal: the list paged through (`?page=<n>`) and each row a link to its module; no filter panel (owner, question 3) |
| `/catalog/module/<id>` | server-rendered | **site** |
| `/programs`, `/programs/<slug>/plan\|areas`, `?variant=<n>` | server-rendered, indexed | **site**, minimal: the overview as a list by faculty, a program's plan as a list (as phones and crawlers get it today), its areas with their modules as links; no filters, no switches |
| `/programs/<slug>/my-plan` | server-rendered, `noindex` | **app** (the visitor's) |
| `/impressum`, `/datenschutz` | server-rendered | **site** (static text; owner, question 1) |
| `/bookmarks`, `/studyplan` | server-rendered explanation, `noindex` | **app** with a written head |
| `/studyplan?share=<code>` | its own page: tags and card name the plan's modules | **app** with a written head that names them (a link preview runs no JavaScript) |
| `/en/…` | every route | the same split |
| cards, calendar feeds, `/api/*`, files | server | unchanged |
| `/sitemap.xml`, `robots.txt` | the site's pages | the same, without the views of the lists |

**The site is minimal on purpose.** It is for search engines, link previews and a first paint;
everything a person does there — filtering, the preview beside the list, marking, planning — is
the app's, once it has taken over. Its pages use the app's markup and stylesheet for what they show,
so the design is the same and the takeover moves nothing.

**What Google sees is the site.** Googlebot runs JavaScript, but the app does not start for it:
`robots.txt` keeps crawlers out of `/api/` on purpose, and the app needs the catalog (4.4 MB) before
it can show anything. So everything that should be found stays a site page — which, with the
programs, it does.

### 4.2 How the server answers

1. **A site page:** complete HTML, cached per snapshot and build as today, rendered by
   `folia-site` from the same view crates the app uses for these routes (§7.5).
2. **The app document**, for the Merkliste, the Stundenplan and „Mein Plan": the head written per
   route by plain code in the server (title, description, canonical address, `noindex`, the Open
   Graph tags with the card; for a shared Stundenplan the plan's modules); the body is the shell
   (rail, header, footer, background) with the route's skeleton, the boot script, and a
   `<noscript>` that says the view needs JavaScript. It depends on the address and the snapshot
   only, so it is cached like a page.
3. Everything else as today: files, `/api/*`, cards, calendar feeds, the gate.

### 4.3 What goes away with it

- Every second form of a control: the filter panel as links and GET forms, the pickers' `<select>`,
  the program overview's filter links, the program page's switches; `JsOnly`/R15 in the app's code.
- The server rendering of the Merkliste, the Stundenplan and „Mein Plan" (and their place in the
  cache and the warm-up); R9 and R22 outside the site's crates.
- `enhance.js` as the classic site's helper: the site needs almost nothing; the app's behaviours
  move into the crates that own them (§9).
- The checks that drive the site's filters without JavaScript (`filters`, parts of `bookmarks`,
  `programs`, `studyplan`, `module`, `search`, `top`); `crawl.mjs` walks the site as before.

### 4.4 What gets worse, said plainly

- Visitors without JavaScript can read everything, but no longer filter the catalog or the
  programs.
- A first visit to an app route (a shared Stundenplan) shows the shell and the catalog's download
  (4.4 MB, with its progress) before its content; accepted by the owner (question 4), with the
  first paint kept fast (§6.7).

## 5. The shell

### 5.1 What it is

The shell is what every page has (owner, question 5): **the icon rail** (the bottom bar on a
phone), **the header**, **the footer** (the ground: Impressum, Datenschutz, Datenstand, the roots)
and **the background** (the birch: crown, wood). Everything between them is the page's: its
sidebar, its content, the panel beside it.

```
┌──────┬─────────────────────────────────────────────────────────────────┐
│      │ header: where the visitor is, the search, the data's status     │
│ icon ├─────────────────────────────────────────────────────────────────┤
│ rail │                                                                 │
│      │ the page: whatever the route shows — for most pages the         │
│      │ design system's frame (sidebar · content · panel beside it)     │
│      │                                                                 │
│      ├─────────────────────────────────────────────────────────────────┤
│      │ footer: the ground                                              │
└──────┴─────────────────────────────────────────────────────────────────┘
 background: the birch (crown, wood) · overlays: sheets, dialogs, notes, the search's suggestions
 phone: the icon rail is the bottom bar
```

### 5.2 How it works

- **Mounted once**, as the parent route of every page (`<ParentRoute>` with an `<Outlet/>` for the
  page). A navigation changes the page and nothing of the shell: the rail, the header, the footer
  and the background stay the same elements. Today the footer is rendered by every page again
  (twice per page, with its query), and the header's whole content by every change of area.
- **The route table says what the shell needs to know about a page:** its area (the rail's tab it
  belongs to, with the tabs' memory of `tabs.rs`), where the header's search goes, the page's title
  and its skeleton. One entry per route, instead of the seven edits a new page needs today (§1).
- **The skeleton is the shell's:** in the frame after a click (R21) the shell puts the route's
  skeleton where the page will be, if the page's data takes longer than the threshold (§6.4).
  Today `pending.rs` and `skeleton.rs` do this by hand for every kind of page.
- **The page's frame is a component, not a rule.** Sidebar, content and the panel beside it are
  `folia-design`'s `Frame` (with the resize handles, the remembered widths, the sheet on a phone,
  the panel that is the page on a phone): written once, used by every page that wants it. The
  hand copies in the catalog, the Merkliste and the start page, the six copies of the resize
  handle and the three ids of the scroll area go (§1). „Nothing jumps" between pages comes from
  using the same component.
- **The local views** (`local.rs`: a module beside a list, „Vollbild" in place, on a phone the
  module as the page, „Zurück" to where it was picked) are what the frame's panel does; a page
  only says where „Vollbild" leads.
- **The site renders the shell statically** (§7.5): the first paint of a site page is the app's.

### 5.3 What the shell owns

The rail and the bottom bar with their counts, the tabs' memory, the header (title, the search
field, the status of the data — today a pill that `boot.js` keeps alive with a
`MutationObserver`), the footer, the background, Esc and „Zurück" (R10, R19), „Nach oben", the
skip link, the landmarks (`header`, `nav`, `main`, `footer`), the theme and the language switch, an
error boundary around the page, and the pending state. Spread today over `App`, `ui::Frame`,
`tabs.rs`, `nav.rs`, `pending.rs`, `skeleton.rs`, `ground.rs` and `enhance.js`.

## 6. Asynchrony: threads and workers

### 6.1 Which thread does what

| Thread | Does | Today |
|---|---|---|
| main | rendering, input, animation | everything |
| **data worker** (new; dedicated, one per tab) | the snapshot (download, keep, open, update); every query and page loader; the search (resolution, typos, the pickers' fuzzy match); the Stundenplan's computations (clashes, „Passt in meinen Stundenplan", exams, the .ics file); the kept answers | the main thread (sql.js and Rust) |
| semantic worker (exists) | the query model and the modules' vectors | exists; its index built on the main thread |
| service worker (exists) | the shell of a build and the model in Cache Storage, the start without a network | exists |

**One service worker, two Web Workers.** A service worker is the browser's proxy between a page
and the network: one per scope (the site has one), started for an event and stopped when idle
for a while, shared by all tabs. It is the right place for caching and the start offline, and the
wrong one for a database held in memory or for long computations. Threads for computing are
Web Workers; a dedicated worker lives as long as its tab. So the brief's „ein bis zwei service
worker" becomes: the service worker as today, plus a data worker, plus the semantic search's worker
that exists.

### 6.2 The data worker

- **SQLite in the worker, in two steps** (both tried in the minimal version, §11.1). First sql.js
  moved into the worker as it is: no new toolchain, the whole catalog in the worker's memory
  instead of the page's. Then rusqlite on `sqlite-wasm-rs`, the same implementation of `Database`
  the server has, without a JavaScript bridge, **in SQLite's memory file system**: on the real
  catalog 2 to 4 times faster than sql.js. Not on OPFS (the `sahpool` file system): a page not yet
  in SQLite's cache is read through a sync access handle, so a query that scans the module texts
  took 1.4 s instead of 19 ms, and a second tab cannot open the pool at all. The file itself is kept
  in Cache Storage, which every tab's worker reads. What step 2 needs: rusqlite 0.40 (the server's
  0.32 moves with it — one SQLite per workspace), and clang for `wasm32` in `build-client.sh` and
  in Nix (`sqlite-wasm-rs` compiles SQLite from C; the container's clang 18 did); the worker's
  bundle grows by SQLite, some 230 kB in brotli more than sql.js's.
- **The snapshot moves out of `boot.js` into the worker:** `/api/status`, the download (streamed,
  progress as events to the UI), the schema check (`user_version`), keeping it, opening it, the
  update in the background; one download for all tabs (Web Locks), the other tabs told
  (BroadcastChannel).
- **New data at once** (owner, 2026-10-02: „sofort rein mit den neuen daten"; today a new snapshot
  is only used from the next start). The worker checks `/api/status` when it starts, when the tab
  comes back into view and every few minutes while it is open; a new snapshot is downloaded in the
  background, checked, and opened beside the old one. Then the worker switches: requests from then
  on are answered from the new one, the kept answers and `DataClient`'s cache are dropped, and the
  page on screen asks for its data again and shows it in place — no skeleton, no lost scroll
  position, the visitor's marks and plan untouched (they live in the browser and name modules by
  id; a module the new catalog no longer has is shown as such, R12). Every tab switches, told by the
  one that downloaded. Not at once: a snapshot of a newer schema than the build reads — it needs the
  new build, so it waits for the next page load, as today; and a copy of an older schema is never
  opened.
- **What it takes off the main thread** — the costly places, as the code has them:
  - the finder („Passt in meinen Stundenplan"): `fit::candidates` builds a whole timetable for
    every module with dates in the semester (some 1,200 in a winter semester, from about a
    megabyte of rows; three for a module with tracks in two towns), then `fit::fits` runs on every
    change of the plan;
  - the Stundenplan's clashes: `clash::weigh` (every pair of placed rows, and a search over open
    choices of up to 10,000 steps a group) is computed anew by `clashes`, `hard_rows`,
    `hard_pairs` and `overlaps` — four to five times for one render of the Stundenplan, two of
    them from its pages' own code (`head.rs`, `aside.rs`). In the worker the plan's timetable is
    computed once per change of the plan, and the page receives its views finished;
  - the exam warnings (every pair of planned modules × every pair of their Termine), the .ics file
    (`export::calendar_of`), the expansion of rows into their dates (`occur`, up to 400 a row);
  - the search's resolution where the words as typed find nothing (every title into a vocabulary,
    then edit distances), and the pickers' ranking with every key (`fuzzy::rank`).

  The program map's layout stays where it is: on the server, once per snapshot (`graph`: 400
  rounds of a force layout, for two sheets).
- **One message per page.** The UI asks for a page's data (`CatalogPage(url)`), the worker runs the
  whole loader (today's `catalog::pages`) on one snapshot and answers with what the page shows. No
  chain of queries across the threads. What the app keeps today only to save computing moves along
  with the computing: the answers kept for the visit (`client`'s `Answers`) and the finder's
  candidates (`CandidateSet`, which the catalog page holds between two runs of
  `pages::fit`).
- **Prefetching becomes cheap.** A link under the pointer or the next page of a list can be asked
  for in idle time. `docs/folia/frontend.md` rejected that because a loader blocks the main thread (the
  start page's 70–280 ms); in the worker it blocks nothing.
- **The semantic search behind the data worker:** the data worker builds the index from
  `v_module_vector` and talks to the semantic worker over a `MessageChannel`, so „Ähnliche Module"
  is one request (the closest modules, then the filters), and nothing goes through the page.

### 6.3 The protocol

`folia-pages` holds the messages (its module `ask`): a request type per question with its answer type, serde, in a
binary format (postcard; already in `Cargo.lock`) in transferable buffers.

```rust
pub trait Ask: Serialize + DeserializeOwned {
    type Answer: Serialize + DeserializeOwned;
    /// Requests of one lane replace each other while they wait: typing, a slider.
    const LANE: Lane;
}
pub struct CatalogPage { pub url: CatalogUrl, pub locale: Locale }
impl Ask for CatalogPage { type Answer = Result<CatalogData, DataError>; const LANE: Lane = Lane::Page; }
```

- A request id per message; per lane, a waiting request is replaced by a newer one („newest
  wins", as `semantic.js` does it today); priorities: what was clicked before prefetching before
  background work.
- The handshake compares the build: UI and worker are always of one build, as the page and its
  files are today (`?v=<build>`).
- A worker that dies (a panic, R3) is started again by the UI, which retries once and otherwise
  shows the region's error state.

### 6.4 The UI side

```
click ─▶ shell: the route's regions, what was clicked in its new state ─────────▶ next frame
      └▶ DataClient: CatalogPage(url) ── message ──▶ data worker: the loader (SQLite, search …)
                                       ◀── answer ── the page's data
         the region renders it; its skeleton only if the answer took longer than the threshold
```

- `DataClient` (`folia-data`): an async method per request, a cache keyed by request and snapshot,
  requests in flight shared.
- Pages read their data through resources under a transition: what is shown stays until the new
  data is there; after the threshold (50 ms, `pending::SLOW_MS` today) the region shows its
  skeleton. R21 stays — what was clicked shows its new state in the next frame — and comes from the
  shell and the controls reading the target route, without `pending.rs` replaying the router's
  events.
- The takeover of a site page waits until the worker has answered for that page, so the app's
  first render has its data at once and shows no skeleton over a finished page.
- **What a worker cannot take off the main thread:** building and laying out the page (25–75 ms
  on a laptop). Kept small by the virtual lists (exist), keyed rows, rendering what is visible
  first and the rest in idle slices, and a smaller DOM. The worker takes the queries and the
  computations away and lets the page paint while they run.

### 6.5 The service worker

Stays as it is: the shell per build, the model's own cache, pages kept for the way back. For the
app's routes it answers with the app document of its build, offline included. It never touches the
catalog. (Background Sync and Periodic Background Sync would let it fetch a new snapshot without
an open tab; Chromium only, so not part of the plan.)

### 6.6 What it costs

- Every page is asynchronous: every region needs its skeleton (the shell has them per route).
- Page data crosses the thread boundary: a few kilobytes for 50 rows of the catalog, to be
  measured; a page model of the Stundenplan more.
- Two WASM bundles share code (row types, labels, addresses); the UI bundle loses the SQL and the
  timetable logic, the worker's has no Leptos. Sizes measured in phase 0.
- Debugging across threads.
- Until the snapshot lives in the Origin Private File System, every tab holds a copy in memory, as
  today.

### 6.7 First paint first, the rest in stages

Owner (question 4): loading is fine, but the first paint as fast as possible, and the rest of the
app loaded after it.

| Stage | What | Comes from |
|---|---|---|
| 1. first paint | the page as HTML: a site page complete, an app route the shell with its skeleton; the stylesheet, the font; the head scripts set theme, widths, language | the server's HTML cache (or the service worker offline); nothing waits for a script |
| 2. the shell runs | a small UI bundle: the shell, the router, the design system, `DataClient`; the rail and the header work, a site page stays as it is | `/pkg/…`, kept `immutable` per build |
| 3. data | the data worker starts in parallel with stage 2, opens the kept snapshot or downloads it (progress in the header) | the worker's own bundle, `/api/db` |
| 4. the page | the feature of the current route, loaded when it is first needed; the app takes the page over once its data is there | a bundle per feature (lazy routes), to be proved in the spike |
| 5. later | the other features in idle time; the semantic search's model last, as today | idle time, low priority |

- **Code splitting works without `cargo-leptos`** (minimal version, §11.1): a `#[lazy]` function per
  route, the UI bundle built as a binary with LTO, relocations and its symbols, split by Leptos's
  own splitter (`wasm_split_cli_support`, what `cargo-leptos` runs), then `wasm-bindgen
  --keep-lld-exports` on the main part. The split's loader imports the main module by its plain
  name, so the build goes into the file names (`/pkg/<build>/…`) instead of `?v=<build>`: two
  addresses of one module would be two instances of the bundle. Dioxus's `wasm-split-cli` does not
  read Leptos's names.
- Stage 1 never waits for WASM: today's rule (until the takeover the site is a plain website)
  stays, and holds for the app document too.
- Measured in the minimal version: time to the first paint, to a working rail, to the page with its
  data, for a first visit and a returning one, on a phone's CPU.

## 7. Crates

### 7.1 Principles

- **One system, one crate.** A crate has a small public API (what its `lib.rs` exports), owns its
  state, its texts, its styles and its icons, and has tests of its own.
- **Layers**, from the bottom: base → domain → data → UI base → widgets → features →
  composition. A crate uses only layers below its own. **Features never use features.**
- **Where it runs:** *iso* crates render on the server too (the site) and use no browser API —
  their behaviour sits behind a feature `web`; *web* crates are the browser app's; *native* crates
  the server's; *worker* crates the data worker's. Only the iso crates are built twice — today it
  is all of `folia-app`.
- **The UI has no database.** No `Database` is implemented in the UI bundle, so no query can run
  on the main thread, not even by mistake: data comes through `DataClient` (R25). The UI may use
  the domain crates' types and their cheap pure functions.
- **Checked, not hoped for:** the table of layers is a file in the repository, and a workspace test
  compares `cargo metadata` with it (R24).

### 7.2 Layers

```
composition   folia-app (the UI bundle)            folia-worker           folia-site · folia-server
features      home · catalog · programs · bookmarks · planner
widgets       folia-widgets
UI base       folia-shell · folia-stores · folia-data
              folia-design
domain        folia-pages (the loaders, what each page shows, the messages to the worker)
              folia-query · folia-timetable · folia-plans · folia-semantic
              folia-routes
              folia-search
base          folia-calendar
              folia-model (with the Markdown reader; rusqlite behind its feature `sqlite`)
              folia-locale · folia-pack
beside        folia-cards (drawing, server)
```

Inside a layer, a crate may use the crates printed below it in that layer.

### 7.3 The map

**Base** — no I/O, compiles everywhere:

| Crate | From today | Holds |
|---|---|---|
| `folia-pack` | exists | link codes |
| `folia-locale` | `catalog::i18n` (`Locale`, `common`), the machinery of `app::i18n`, the number formats of `app::format` | languages and their address prefixes, dates, numbers, semester names, the pattern of a text group |
| `folia-model` | `catalog::{db, rows, rows_detail, labels, text}`, the id checks of `catalog::url`; behind the feature `sqlite` `catalog::native` | the contract: `Database`, `Value`, `DbError`, `SCHEMA_VERSION`, the rows, the codes with their labels, what an id is, the module texts' Markdown reader; with `sqlite` rusqlite's `Database` (the server, the tests, later the worker) |
| `folia-calendar` | `catalog::timetable::{day, semester, kind, rowkey, select, cancel, share, subscription}` | days, holidays, semesters, the kinds of events, row keys, a plan's selection, the codes of a shared plan and of a calendar subscription |

**Domain** — runs where the data is (the server, the data worker); only `folia-search` and
`folia-query` contain SQL:

| Crate | From today | Holds |
|---|---|---|
| `folia-search` | `catalog::{search, fuzzy}`, the three search statements of `catalog::queries` | folding as Radix folds, the words of a query, the scored search, typos and their resolution, the pickers' match |
| `folia-routes` | `catalog::url`, the filter state of `catalog::filter` | every address: the types, reading and writing them, `listed`, the local views; the catalog's filter as the address writes it |
| `folia-query` | `catalog::queries`, the SQL half of `catalog::filter` | the contract's statements (R11) and the filter turned into SQL, with what the loader adds to it (the resolution of the search, the derived areas, the visitor's ids) |
| `folia-timetable` | the rest of `catalog::timetable`, `catalog::exam_reading` | a semester's Termine and their dates, clashes, exams, the views of a week, what fits, the .ics file — pure, no database |
| `folia-plans` | `catalog::{studyplan, plan, variants}`, the area logic of `catalog::pages` | the rows of a study plan and their areas, the study directions, the Stundenplan's stored documents and the import of a Regelstudienplan |
| `folia-pages` | the loaders and data types of `catalog::pages`, `catalog::graph` | one loader per page and what each page shows (`CatalogData`, `ModuleData` …): the façade the worker and the site call; the messages between the UI and the worker (§6.3: a request per loader, its answer the loader's data); the layout of the program map, which the server makes once per snapshot |
| `folia-semantic` | exists | the semantic search |

**Data:**

| Crate | From today | Holds |
|---|---|---|
| `folia-worker` | new; the snapshot part of `boot.js`, the answers kept in `client` | the data worker: snapshot, loaders, computations, the semantic worker's port |
| `folia-data` | `app::data`, the bridge of `client` | `DataClient`, its cache, a fake for tests |

**UI base and widgets:**

| Crate | From today | Holds | Kind |
|---|---|---|---|
| `folia-design` | the primitives of `ui.rs` with `ui::{Frame, Plain}`, `icons.rs`, `combobox.rs`, the slider, the gesture of `swipe.rs`, the DOM half of `nav`, the tokens and the base of `app.css`, parts of `enhance.js` | tokens, base styles, the page's frame (sidebar, content, the panel beside it, their handles), components (button, switch, segmented row, chip, badge, tabs, sheet, picker, slider, skeleton, empty state, panel head, prose, icon and sprite), behaviours (resize handle, sheet, swipe, carousel) | iso |
| `folia-shell` | the chrome of `App` (rail, header, bottom bar, crown, wood), `ui::{BackLink, ToTop}`, `tabs`, `pending`, `skeleton`, `ground`, `languages`, `launch`, the layout half of `nav`, parts of `enhance.js` | the shell of §5: rail, header, footer, background | iso (rendered statically by the site) |
| `folia-stores` | the stores of `bookmarks`, `studyplan`, `myprogram`; the view settings in `localStorage` (`betula.finder`, `betula.plan.shape` …) | what the visitor keeps: one store type for all of them (load, check, save, follow the other tabs), undo and the question before throwing away | web |
| `folia-widgets` | the view of a module (`pages::module`), the row of a list, `week.rs`, the switches „Merken" and „Einplanen" | what several features show | iso |

**Features** — each its routes' pages, sidebars, texts and styles:

| Crate | From today | Kind |
|---|---|---|
| `folia-home` | `pages::{home, legal}` | iso: the start page, Impressum and Datenschutz |
| `folia-catalog` | `pages::{catalog, module}` (the name is free once today's `folia-catalog` is split up) | the list and the module page iso, the rest web |
| `folia-programs` | `pages::{programs, program}` | the overview and the plan iso (site pages), „Mein Plan" and the rest web |
| `folia-bookmarks` | `pages::bookmarks` | web |
| `folia-planner` | `pages::studyplan::*` (9,800 lines, the largest feature) | web |

**Composition and server:**

| Crate | From today | Holds |
|---|---|---|
| `folia-app` | `App`, the routes, `client` | the route table and the services, and the UI bundle's entry (a `cdylib`); thin |
| `folia-site` | new | the site's pages, composed of iso crates; its own short route list |
| `folia-cards` | `server::{cards, launch, birch, logo}` | the drawn pictures (resvg): cards, launch screens |
| `folia-server` | exists | HTTP, snapshot client, cache, `/api/*`, calendar feeds, gate; its build script collects the crates' styles and icons into one stylesheet and one sprite and minifies them (today's `folia/crates/server/build`) |

That is 26 crates, and one of test support (§7.4). The owner asked for not more than the 34 of the
first draft, rather fewer (2026-10-02): merged where two had no reason to stand apart — the Markdown
reader and rusqlite's `Database` into the contract (`folia-model`), the messages and the map's
layout into `folia-pages`, the module page into the catalog (it is the catalog's route), the legal
pages into the start page's crate, the UI bundle's entry into `folia-app`, the stylesheet's build
into the server's build script. What stays apart does so for a reason: a different place it runs
(server, worker, UI), a different layer, or heavy dependencies (`folia-cards` and its resvg).

**What the server must stop reaching into.** Today it uses page internals of the app:
`app::pages::catalog::PickerChoices` and `app::pages::programs::ProgramsReady` (made per
snapshot in `folia/crates/server/src/snapshot.rs`), `app::format` for the cards' texts, `app::ui::{Mark,
Wordmark}` for the gate's login page, and the whole `App` to discover its routes
(`generate_route_list(app::App)`). In the new map these are `folia-site`'s own data, `folia-locale`,
`folia-design` and `folia-site`'s route list.

### 7.4 Cutting `catalog` apart

Of its 29,600 lines, 17,600 are code and 12,000 tests. Only `queries` and `search` write SQL
(`filter` builds fragments of it, `pages` calls `queries`); everything else — all of the
timetable, the study plans, the program map, the addresses, the Markdown reader — is computation
over rows. Three cycles between its modules stand in the way of crates, and each is broken by
moving a few items:

| Cycle | What breaks it |
|---|---|
| `filter` → `search` → `queries` → `filter`; `fuzzy` ⇄ `search`; `queries` → `url` → `filter`; `url` ⇄ `timetable::share` | the three statements of the search (`search_count`, `search_words_found`, `search_titles`) into `folia-search`, the folding beside `fuzzy`; the id checks (`is_module_id`, `is_program_id`) into `folia-model`; the path of a shared plan into `folia-routes`, so that its code needs no address |
| `pages` ⇄ `plan`, `pages` ⇄ `variants` | the areas of a program (`CatalogArea`, `catalog_areas`, `is_structural` …, `pages.rs` 158–481) into `folia-plans` |
| `timetable::{occur, facts, clash, model}` | nothing: the four stay together in `folia-timetable` (moving `clash::Weeks` into `facts` would only tidy it) |

Two cuts more, which the layers ask for:

- **The catalog's filter in two:** what the address says (`CatalogQuery` without
  `text_resolution`, in `folia-routes`; the resolution is never part of an address already) and
  what the loader runs (`folia-query`: the filter as SQL, with the resolution of the search, the
  areas derived for a semester and the visitor's ids).
- **The timetable in two:** its core (`day`, `semester`, `kind`, `rowkey`, `select`, `cancel` and
  the codes of a shared plan and of a subscription), which the addresses and the filter need,
  becomes `folia-calendar`; its engine (`occur`, `facts`, `model`, `clash`, `exams`, `views`,
  `fit`, `ics`, `export`) `folia-timetable`.

The tests move with their crates. What many of them share (`folia/crates/catalog/src/tests.rs`'s `open` and
`studyplan_db`, `area_fixtures`, the fixtures of `timetable::model::tests`) becomes a crate of
test support, a dev-dependency only; `every_query_runs_against_the_snapshot` stays the guard of
`folia-query`. Two statements run only in the tests today and get their caller or go:
`queries::module_vectors` is what the data worker builds the semantic index from (today `boot.js`
reads the vectors in JavaScript), `queries::search_suggestions` has none; `url::CALENDAR_PREFIX`
repeats `timetable::subscription::CALENDAR_PREFIX` and goes.

### 7.5 The site out of iso crates

- An iso crate builds without `web` as markup over data (what the site renders) and with `web`
  adds its behaviour. Today's `ssr`/`csr` split of one crate of 32,600 lines becomes a feature of
  a handful of small ones: `folia-design`, `folia-shell`, `folia-widgets`, and the site's parts of
  the features (`home`, the catalog's list, `module`, the programs' overview and plan, `legal`).
  Those parts show and link; they have no behaviour of their own, since the site is minimal.
- `folia-site` composes them: the shell with the page in it, the same markup the app makes, so the
  design is the same and the takeover moves nothing.
- The site does not need the app's route table, its stores, `DataClient` or any feature beyond its
  own routes; the server no longer compiles the app.

### 7.6 Styles, texts and icons per crate

- **Styles:** every UI crate has its stylesheet, its classes under the crate's prefix (`ds-` the
  design system, `sh-` the shell, `cat-` the catalog …) and its cascade layer per layer of §7.2
  (`@layer tokens, base, design, shell, widgets, features`). the server's build script (today's
  `folia/crates/server/build`) collects them in the order of the layers into the one stylesheet a page loads,
  minified and kept as today. A test fails on a class outside its crate's prefix and on a
  custom property no token defines. `app.css`'s 3,270 lines go to their owners.
- **Texts:** a crate's texts are in its own module of text groups (R23 as it is); `folia-locale`
  has the pattern, and a crate that lacks a language's text does not compile, as today.
- **Icons:** a crate names the icons it uses; the sprite is made of all of them.

### 7.7 An example: the search as a closed system

`folia-search` holds the catalog's search and the pickers' match and nothing else:

- **API:** `fold` (the same folding as Radix's, checked against
  `radix/internal/normalize/testdata/search.tsv`), the words of a query, the scored search
  (`Plan`, whose table the catalog's list and „weitere Treffer außerhalb deiner Filter" join, as
  today), the resolution of words no title knows (`resolve`, with its three statements), the
  ranking of the pickers (`fuzzy`).
- **Uses:** the `Database` seam of `folia-model`, nothing else: no labels, no texts, no page.
- **Used by:** `folia-query` (the filter as SQL, the matches outside the filters, „Ähnliche
  Module"), `folia-pages` (the resolution), `folia-plans` (the folding of names), the server
  (folded cache keys), and as pure functions the UI where a list is short and already there (the
  pickers' choices, the program overview's search).
- **Tests:** its own; today they are part of `folia/crates/catalog/src/tests.rs`.
- **Today:** spread over `search.rs`, `fuzzy.rs` and three functions of `queries.rs`, and part
  of the cycle above.

The semantic search (`folia-semantic`) stays a crate of its own: the two are different systems
and meet only in the catalog's loader („Ähnliche Module").

### 7.8 Compile times and size

On 2026-09-23 `folia-app` was split into crates by layer and page and measured
(`docs/folia/frontend.md`, „Build times"; branch `claude/web-tier-compile-time-0a192a`): an edit in a
page got faster (8 instead of 10 s; 6.3 instead of 11 s for the browser app), an edit in `ui`
slower (12.5 instead of 9 s, every page compiles again), rustc's frontends took 49 instead of
23 s in all, and the bundle grew by 3.6 % gzipped. It was not taken. This concept differs in
what made that split expensive: the app compiles once (only the iso crates for the server as
well); the design system is a stable bottom layer once it is built, not the `ui.rs` every page
edits; and the bundle splits into two (UI, worker), each with less in it. Measured in phase 0 on
a skeleton of the layout before anything is moved; `erase_components` stays.

### 7.9 The repository, in order

Owner: „Aktuell ist das repo komplett unordentlich das muss unbedingt geordnet und strukturiert
werden." Today Radix's Go (`cmd/`, `internal/`, `go.mod`) and Folia's Rust (`app/`, `catalog/`,
`client/`, `pack/`, `semantic/`, `server/`, `Cargo.toml`) stand side by side at the top, with
Folia's assets in `folia/assets`, its scripts in `scripts/`, its checks in `e2e/` (and a Go load test
inside them), its design sources in `design/`, research in `research/`, and nine documents for both in
`docs/`. Proposed:

```
README.md  CLAUDE.md  flake.nix  flake.lock  .github/  .env.example
radix/                  the collector (Go): go.mod, radix/cmd/radix, internal/…
folia/                  the web tier (Rust)
  Cargo.toml  Cargo.lock    the workspace
  crates/<crate>/           every crate, flat: folia/crates/routes is the crate folia-routes
  layers.toml               the layers of §7.2, which the dependency test reads (R24)
  e2e/                      the browser checks; folia/e2e/load, the load test
  design/                   design sources (birch, forest, logo, cards, og, prototypes)
  scripts/                  build-client.sh, dev.sh, build-cache.sh, hooks
deploy/                 unchanged: stacks, VPS scripts, models.lock (both parts)
docs/
  radix/                operations.md, schema-v2.md, data-sources.md, backend-data-overhaul.md
  folia/                frontend.md (rewritten in the last phase), i18n.md, this concept
  history/              frontend-rewrite.md, frontend-phase0.md: decided and done
research/               today's research/ (the semantic search's measurements and scripts)
```

**Names** (owner): every crate is `folia-<crate>` — `folia-routes`, `folia-search`,
`folia-shell` … — and so is its library (`folia_routes`, not `routes` or `catalog` as the
`[lib] name` of today's crates sets it). Its directory is `folia/crates/<crate>`. The binary stays
`folia`.

**What moves with it:** the Go module path (`github.com/leonieziechmann/betula` →
`…/betula/radix`, every import of `internal/…`), `flake.nix` (its list of source directories, the
`cargoLock`, the Go build), `.github/workflows/images.yml` (`BUILD_PATHS`), `deploy/ship.sh` (it
reads `folia/crates/app/src/pages/legal.rs`), `folia/scripts/build-cache.sh` and the hooks (target directories),
`folia/scripts/dev.sh`, the include paths of the server (`../app/assets`), the semantic search's test
that reads `radix/internal/normalize/testdata/search.tsv`, and every path in the documents. A move of
everything at once collides with every branch in flight, which is why it happens when Folia's
development pauses (§11), as the first step, in commits of moves alone (`git mv`, so the history
follows).

## 8. State, and the ways between systems

- **The address** is what can be shared and bookmarked: filters, the view, the module beside a
  list — as today, each feature with its address type in `folia-routes`.
- **Stores** (`folia-stores`) are what the visitor keeps (R20): the Merkliste, the Stundenplan,
  „Mein Studiengang", the view settings. Reactive (Leptos's `reactive_stores`, already in
  `Cargo.lock`), kept under versioned keys, the same in every tab (the `storage` event, as the
  Merkliste does today).
- **Services through context**, provided by the composition root: `DataClient`, the stores, the
  shell's navigation, the settings. A feature takes what it needs; a test provides fakes.
- **Features never call each other.** They meet in addresses (a link to another feature's route,
  built with its type in `folia-routes`) and in stores: „Einplanen" in the catalog's row calls the
  Stundenplan's store, not the Stundenplan's pages. What several features show (a module's view, a
  row of a list of modules, the week grid, the switches „Merken" and „Einplanen") is one layer
  further down, in `folia-widgets`.
- **The shell knows the features only through the route table** of the composition root.

Today's pages reach into each other in a handful of places; each has a home one layer down:

| Today | Used by | Goes to |
|---|---|---|
| `pages::module::{ModulePanel, ModuleFull}`, `semesters_of` | catalog, Merkliste, program, `local.rs`, `swipe.rs` | `folia-widgets` (the view of a module) |
| `pages::catalog::{Row, ListKeys}` | Merkliste | `folia-widgets` (a list of modules and its keys) |
| `pages::catalog::{Choice, Toggle, Tri}`, `duration_choices`, `years_choices` | start page | `folia-design` (the switches), `folia-routes` (what a filter can say) |
| `pages::catalog::phone_layout` | program, Stundenplan, Merkliste | `folia-design` (the frame knows which layout is on) |
| `pages::catalog::finder_on` (the finder's last choice, from `localStorage`) | the Stundenplan's head and module list | `folia-stores` (a view setting) |

And the shared modules lean on pages and on the chrome: `pending` → `local` → `pages::module`,
`swipe` → `pages::module`, `ui` → `tabs` and `ground`. In the layers they cannot: the design system
knows no shell, the shell no feature.

## 9. JavaScript

| File | Today | New |
|---|---|---|
| the head's two inline scripts | the language, the theme, the widths, the iOS launch screens, before the first paint | stay: they have to run before the first paint |
| `boot.js` | the snapshot (IndexedDB), sql.js, the bundle, the program map, the semantic search | starts the data worker and the UI bundle; the rest moves into the worker |
| `enhance.js` | 960 lines: the classic site's helpers and the behaviours both modes share | the site keeps what it needs; the behaviours move to their crates (the design system: resize, sheet; the shell: the bottom bar's swipe, shortcuts, theme, „Nach oben") |
| `sql-wasm.js`, `sql-wasm.wasm` | sql.js on the main thread | in the worker (step 1), gone with rusqlite (step 2) |
| `sw.js` | the shell and the model | stays; answers the app's routes with the app document |
| `folia/crates/semantic/js` | the worker and its face on the page | the worker stays; its face moves into the data worker |

Where a behaviour is pure motion (the springs of the bottom bar, Web Animations computed from
them), it may stay a small script owned by its crate; what touches the app's state is Rust.

## 10. The rules

| Rule | New |
|---|---|
| R1–R8 | stay. R3: a panic in the data worker ends only the worker, the UI starts it again |
| R9 server HTML is user-independent | the site only |
| R10 shortcuts next to their button, R12, R13, R14, R18, R20, R23 | stay |
| R11 all SQL in `queries.rs` | stays: the SQL is in `folia-query`, and the search's own statements in `folia-search` (§7.4) |
| R15 what needs JavaScript is hidden without it | the site only |
| R16 read the source, not a memo of it | stays while the framework behaves so |
| R17 every page is framed | becomes structure: the shell around every page, the design system's frame in it |
| R19 the areas are tabs | stays, in the shell |
| R21 a click answers in the next frame | stays, through the shell and transitions |
| R22 nothing thread-bound in a server render | the site's crates only |

New:

- **R24 Layers.** A crate depends only on the layers below it; features never on features. A
  workspace test reads `cargo metadata` and fails on a dependency the table of §7 does not allow.
- **R25 No data on the main thread.** The UI reads data only through `DataClient`, and its bundle
  has no `Database`; queries and computations run in the worker. Building the page stays on the
  main thread (§6.4).
- **R26 Every asynchronous region has its skeleton,** shown only after the threshold.
- **R27 A crate owns its styles, texts and icons** (§7.6).
- **R28 UI, worker and service worker of one build** only; the protocol checks it at the handshake.
- **R29 What crosses a boundary comes back as it went.** Nothing that goes to or from the worker
  skips a field (`skip_serializing_if`); a test sends every answer through the format. And a future
  writes into a page only through `use_ask`, which writes with `try_`: the page may be gone.

## 11. Steps

The owner's order (question 6): first a **minimal version** that finds the problems, while the
development of Folia goes on; then the plan is made final, Folia's development pauses, and the
restructuring is done to the end.

### 11.1 The minimal version (phase 0)

A thin slice through every new part, on a branch of its own beside `develop` (it is merged
nowhere; what it teaches goes into this document). It holds just enough to meet each risk once:

| Part | What it does | The question it answers |
|---|---|---|
| crates | `folia-model`, `folia-routes`, `folia-query` and `folia-pages` (a few loaders, their messages), `folia-worker`, `folia-data`, `folia-design` (the frame, a few components), `folia-shell`, one feature, `folia-site`, under `folia/crates/` with their `folia-<crate>` names | do the layers hold, what does an edit cost to build, how large are the two bundles |
| data worker | opens the snapshot and answers the catalog's list and a module's page; sql.js in the worker first, then rusqlite on `sqlite-wasm-rs` with the Origin Private File System | the time of a request with its messages, on a phone; memory; two tabs; the Nix build |
| shell | rail, header, footer and background mounted once; the catalog's list and the module page inside it, through the design system's frame | no remount on navigation, disposal of what a page made, the skeleton after the threshold |
| loading in stages | the app document and a site page painting before any WASM, the shell's bundle, the worker, the feature's bundle loaded on demand | does code splitting work without `cargo-leptos`; the times of §6.7 |
| site | the catalog's list and a module page rendered by `folia-site` from the same view crates | is the design the app's to the pixel; does the takeover move nothing |
| styles | the frame's and one feature's styles collected by the server's build script | the per-crate stylesheet and its test |

Done when each question has a measured answer in §6–§7, and the plan is changed where an answer
says so.

### 11.1.1 What it found (2026-10-02)

Branch `spike/folia-next`, `folia/` (how to run it: `folia/README.md`). Measured in headless
Chromium on the container's four cores, against the catalog of betula.app (44 MB), the server and
the browser on one machine (the download is the loopback's, not a network's). Release bundles
unless a line says otherwise.

| Question | Answer |
|---|---|
| Does the shell stay? | yes: rail, header, footer and background are the same elements after navigating to a module, back, and to another page of the list (the check marks them and looks again) |
| Does the takeover move anything? | no: the site's module page and the app's render of it are the same pixels; the app takes over only once the worker has answered for the page |
| First paint | 100–270 ms on every route, before any WASM ran; on an app route (`/bookmarks`) the shell paints from the app document at 70–100 ms |
| Ready (the app has taken over) | first visit 840–950 ms (the catalog downloaded and opened in the worker meanwhile), returning 360–450 ms (the snapshot from Cache Storage) |
| A request across the threads | the catalog's list: 18–35 ms in the worker, 19–36 ms in all; a module: 11–13 ms, 16–17 ms in all; postcard: 12.9 kB for 50 rows, 5.1 kB for a module |
| A click to the page | module 76–89 ms, the next page of the list 77 ms (debug UI bundle); with the CPU four times slower 207–212 ms. No skeleton came up in any of it: every answer came within the 50 ms threshold |
| New data at once | the page on screen shows the new snapshot in place 1.4 s after it was announced (download and opening of 44 MB included), no skeleton, the list stays |
| Sizes (brotli) | UI bundle 157 kB (Leptos, router, shell, two views; split: 140 kB + 23 kB for the module page), worker 84 kB + sql.js 291 kB; with rusqlite instead of sql.js the worker's SQLite is 504 kB, unoptimised for size |
| Builds | first build of the UI bundle 84 s, of the worker 32 s (`wasm-dev`), both in release 107 s; an edit in the catalog's crate or in the shell: 1.2 s to a new UI bundle (a small app — indicative, not today's 11 s compared) |
| Layers | the test reads `cargo metadata --no-deps` against `folia/layers.toml` and fails on a feature using a feature (tried) |
| SQLite in Rust | builds and runs; in memory 2–4× faster than sql.js; on OPFS slow and one tab only (§6.2) |
| Splitting | works without `cargo-leptos`; file names must carry the build (§6.7) |

What broke on the way, and what the plan takes from it:

- **A format that does not describe itself breaks on `skip_serializing_if`.** `CatalogQuery`
  skipped `text_resolution` when empty, so postcard could not read back a `CatalogData`. The field
  is now always written (the minimal version's one change to `catalog/`), and `folia-pages` tests
  that every answer survives the trip. Rule R29.
- **A future outlives its page.** A request's answer arrived after the page that asked had gone
  (another route) and wrote into its disposed signals: the crash of `docs/history/frontend-rewrite.md` §3A,
  now through `async`. `use_ask` writes only with `try_`; nothing else in a page writes from a
  future. Rule R29.
- **What the worker says before the app listens is lost.** The worker starts with the page (stage
  3) and announces its snapshot before the UI bundle is there; `boot.js` keeps those messages for
  the app.
- **Leptos's executor starts with the mount**: what runs before it (asking for the first page's
  data) uses the browser's.
- **The footer is part of each page's scroll area today** (`app.css`, „one scroll area"); as the
  shell's, the scroll area is the shell's too. The minimal version lets `main` scroll; the real
  layout is a task of phase 4.
- Not tried: the semantic search behind the worker, the service worker, two tabs sharing one
  download, a phone (only its CPU, throttled), Nix.

### 11.2 The restructuring (Folia's development paused)

Every step is merged into `develop` as usual (CLAUDE.md), the site and the checks work after each,
and canary gets it when the owner asks for a release. Sizes are relative.

| Phase | What | Size |
|---|---|---|
| 1 | **The repository in order** (§7.9): `radix/`, `folia/`, `docs/`, `research/`; moves alone, then the paths that follow them; the crates take their `folia-<crate>` names. | M |
| 2 | **The domain out of `catalog`** into its crates (§7.3, §7.4): the three cycles broken, then moves; no behaviour changed, the tests go along. | M |
| 3 | **Site and app.** `folia-site` renders the site (§4.1), minimal; the Merkliste and the Stundenplan get the app document; the second forms of every control go; sitemap, robots.txt, warm-up and the checks follow. | M |
| 4 | **Design system and shell.** `folia-design` with the frame, `folia-shell` with rail, header, footer and background mounted once; the per-crate styles and texts. | L |
| 5 | **The asynchronous seam.** `DataClient` with its async API, first on the main thread's sql.js; the pages move to resources one by one; `pending.rs` shrinks to what the shell keeps. | L |
| 6 | **The data worker and the stages of loading.** The messages in `folia-pages`, `folia-worker`; `DataClient` switched to it; the snapshot out of `boot.js`; the semantic search behind it; the bundles split as the minimal version found. | L |
| 7 | **Features as crates;** `folia-app` becomes the composition root; the dependency test (R24) on. | M |
| 8 | **Rules and documents:** `docs/folia/frontend.md` rewritten for the new structure, the checks consolidated, measured again (`snappy.mjs`, `folia assets`, `folia/e2e/load`). | S |

Phases 4, 5 and 7 can go page by page together: a page moves into its crate, uses the frame and
reads its data asynchronously in one round. Phase 3 comes early: it deletes the most code, and
everything after it needs to work in one renderer only.

The checks in `e2e/` are the safety net of every phase: they describe what the owner approved,
interaction by interaction, and a phase that cannot keep them green is cut smaller. Phase 3 removes
the checks of the site's filters without JavaScript; new ones come with the worker: a first visit on
an app route, a worker that dies and is started again, two tabs and one download, the start offline
with the snapshot kept by the worker, the stages of §6.7.

What counts as done:

- `snappy.mjs` as today (the first frame within 80 ms, 200 ms with the CPU four times slower), and
  no main-thread task over 50 ms from a query or a computation during the checks.
- The first paint of every route before any WASM has run; the times of §6.7 stated.
- The UI bundle no larger than today's 1.43 MB in brotli; the worker's measured and stated.
- Memory of a phone's tab with the app stated, before and after the Origin Private File System.
- An edit in one feature crate rebuilds the browser app in less time than today's 11 s
  (`build-client.sh --dev`).
- The dependency test (R24) green; every crate with its own tests.

### 11.3 How the phases went

**Phase 1 (2026-10-03), the repository in order.** As §7.9 drew it. The flake builds Radix from
`radix/` and the workspace from `folia/` (only `Cargo.*`, `crates/` and `assets/` are its source:
a change to the checks or the scripts builds no new image); `deploy/ship.sh`'s `BUILD_PATHS` and the
images workflow name the same directories. The Go module's vendor hash did not change. The libraries
are `folia_<crate>` (`folia_app`, `folia_pack`, `folia_semantic`), so the semantic search's module
leaves cargo as `folia_semantic.wasm` (`build-semantic.sh` and the flake copy it to the names the
browser loads, as before).

**Phase 2 (2026-10-03), the domain out of `catalog`.** Nine crates and the test support, as §7.3 and
§7.4 have them; the 293 tests of `catalog` run in their crates. Where the cut went other than planned:

- `folia-model` also holds `meta` (the search reads the snapshot's digest, and the test support the
  pinned one) and `ids` with `MAX_PLANNED` (a plan's cap, which `folia-plans` checks and the
  statements apply); `folia-query` re-exports both.
- The words of the contract are split by crate: `Locale::texts` has the common ones, and
  `folia_calendar::i18n` (the holidays), `folia_plans::i18n` and `folia_timetable::i18n` each have a
  `texts(locale)`. The app's `Texts` names the three (`data`, `plans_data`, `timetable_data`).
- The filter's SQL is the trait `folia_query::sql::CatalogSql` (`to_sql`, `order_by`,
  `order_terms`) on `folia_routes::CatalogQuery`, which keeps `text_resolution`.
- `Subscription::of`, the timetable's half of a calendar subscription, is
  `folia_timetable::export::subscription_of`; the shared plan's address is
  `folia_routes::url::share_path`.
- Fixtures other crates' tests use sit behind a feature `fixtures` (`folia_timetable::model::tests`,
  `folia_plans::area_fixtures`); no crate depends on one above it, not even in its tests (the two
  tests that loaded a page moved into `folia-pages`).
- `queries::search_suggestions` (no caller) and its row went, as did `url::CALENDAR_PREFIX`.

**Phase 3 (2026-10-03), site and app.** The server renders the site's routes as before, minimal:
the catalog without its filter panel (its place holds the panel's bars until the app takes over,
`skeleton::FiltersStandin`, so the list stands where the app's will; without JavaScript the list has
the room), the program overview without its filters (`FilterGroupsStandin`; the faculty jumps
stay), every row a link to the module's page as before. The Merkliste and the Stundenplan get the
app document: their tags (a shared plan's naming its modules) and the page's skeleton
(`skeleton::AppStandin`), with what the view needs in a `<noscript>`. What existed only for a page
without JavaScript went: the pickers' `<select>`s, the typed person, the filter form's hidden fields
and its submission (`enhance.js`'s auto-submit and slider), the server's `PickerChoices`. Where the
app's own control is a link (a program's views, the tags above the list, the pages of the list), the
site keeps it: it is the same markup, not a second form. „Mein Plan" stays as it is: its page is
static text in both. Still in `folia-app`, rendered by the server: `folia-site` comes with the iso
crates it is composed of (phases 4 and 7).

**Phases 5 and 6 (2026-10-03), the asynchronous seam and the data worker.** Every question a page
asks is a type of `folia_pages::ask` (24 of them: one per loader, and the few statements a page
asked directly), with its answer and the loader it runs; `answer_json` answers one by its name in
JSON, what the worker does with every message. `folia_app::data::DataClient` is where a page asks:
`get` (reactive: the answer kept, else the question sent and `None`, and the scope runs again when
its own answer is there), `ask` (a future, for handlers), `now` (`get`, with
`DataError::pending` while the answer is on its way), `use_ask` and `DataError::or_before` (what is
shown stays until the new answer is there). The server and the tests answer on their thread
(`DataClient::new`), the browser through the data worker (`DataClient::remote`). Where it went other
than §6:

- **The data worker runs the app's bundle with sql.js** (step one of §6.2), as the search worker did,
  which it replaces: `folia/crates/client/js/data-worker.js`. The page's thread has no catalog any
  more (no sql.js, no `betulaDb`). The worker's own bundle without Leptos and rusqlite in it are
  step two, still open.
- **The snapshot is the worker's**: it finds the copy kept in IndexedDB (not Cache Storage: the same
  store `boot.js` used, which a worker reads as well, so nothing is downloaded again after the
  update), checks the schema, downloads (one tab at a time, a Web Lock; progress to the header's
  pill), keeps and opens it. It looks for a newer snapshot when the tab comes back into view and
  every five minutes; a newer one is opened beside the one in use and answers from then on, the page
  forgets its answers and asks again (`snapshot_changed`), and the other tabs open the copy the
  first one kept (a BroadcastChannel).
- **No skeleton for the moment an answer takes, without a route table:** the takeover keeps the
  server's page in front of the app as a picture (`boot.js`, `takePicture`) until the app's first
  answers are in (`betulaAnswered`, no question on its way in two looks a frame apart); a step of
  the app keeps a picture of the page before in front of the new one the same way (`pending::hold`,
  at most 400 ms). The checks and `enhance.js` take the end of the picture as the takeover
  (`__betulaApp`).
- **The messages are JSON**, not postcard (R29 holds all the same: nothing that crosses skips a
  field). The heavy computations of the Stundenplan (`clash::weigh` and the rest) still run on the
  page's thread over the rows the worker sends; the finder (`FitAsk`) runs in the worker.

## 12. Still open

Nothing the owner has to decide before the minimal version: every question of the first draft is
answered (§0.1). What the minimal version measures (§11.1) may raise new ones; they come here.
