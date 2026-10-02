# Folia, restructured: a concept for the web tier's next architecture

> Draft for discussion, 2026-10-02. Nothing here is decided: §12 lists what the owner answers
> first, and several parts depend on those answers. Verified against `develop` at `b74d3fb`.
> Companion documents: `docs/frontend.md` (Folia as it is: architecture, rules R1–R23, checks),
> `docs/frontend-rewrite.md` and `docs/frontend-phase0.md` (the rewrite of 2026-09-19 and its
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

In one sentence: Folia becomes **a small public site** (a few server-rendered pages for search
engines, link previews and visitors without JavaScript) **plus a browser app** whose main thread
only draws, whose data and computations run in workers, whose frame is built once, and whose
systems are crates with their own API, state, texts and styles.

The proposal in five lines:

1. **Site and app** (§4): the server renders `/`, the catalog, the module pages and the legal
   pages; every other route is the app, in a document whose head the server still writes.
2. **One shell** (§5): header, navigation, sidebar, content, the panel beside it and the footer
   are mounted once; pages fill regions, and the shell shows each route's skeleton.
3. **Workers** (§6): a data worker holds the snapshot and runs every query, loader and
   computation; the semantic search keeps its worker; the service worker stays a cache.
4. **Crates** (§7): 34 crates in layers; features never use features; a test checks it.
5. **Steps** (§11): eight phases, each merged into `develop` with the site working; the second
   renderer goes early (phase 2), because that deletes the most.

## 1. Where Folia stands

| Crate | Rust lines | What it is |
|---|---|---|
| `folia-app` (`app/`) | 32,600 — pages 19,700 (the Stundenplan alone 9,800), texts 5,600, the rest 7,300 | every page and component, built twice: feature `ssr` for the server, `csr` for the browser |
| `folia-catalog` (`catalog/`) | 29,600 — 17,600 of code and 12,000 of tests; the timetable 12,700 | the data contract and its SQL, the page loaders, the URL scheme, the search, the Stundenplan's logic, the program map, the texts of the data, a Markdown reader |
| `folia-server` (`server/`) | 7,100 | axum: snapshot client, HTML cache, `/api/*`, link-preview cards, calendar feeds, the access gate |
| `folia-pack`, `folia-semantic`, `folia-client` | 2,300 · 2,000 · 300 | link codes; the semantic search; the browser's entry with the sql.js bridge |
| `app/assets` | `app.css` 3,270 lines, `enhance.js` 960, `boot.js` 274, `sw.js` 169 | one stylesheet, the behaviours written in JavaScript, start and offline |

What shapes the architecture today, and what it costs:

1. **One set of components, two renderers.** Every route is rendered by the server (complete HTML,
   also for visitors without JavaScript) and again by the browser app, which takes the page over.
   The price: 79 `#[cfg(feature = "csr")]` gates in `folia-app`; rules that exist only for this
   (R9 server HTML is user-independent, R15 `JsOnly`, R22 nothing thread-bound in a server
   render); a second form of every interactive control for the time without JavaScript (toggles
   as links to their next state, GET forms with hidden inputs, a `<select>` where the app has a
   picker); `enhance.js` smoothing the classic site; and `folia-app` compiled twice, once for each
   build (in the server's alone it is 35 of the 57 s of a fresh worktree, `docs/frontend.md`,
   „Build times").
2. **Pages are synchronous functions of their route.** SQLite answers synchronously on both sides,
   so a page calls one loader of `catalog::pages` and renders. In the browser all of it runs on the
   main thread: sql.js behind a JavaScript bridge (10–13 ms of the 48–54 ms of queries of the start
   page and the program overview), then building the page (25–75 ms), then layout — on a phone's
   CPU 260–850 ms per click before `pending.rs` existed. `pending.rs` and `skeleton.rs` (920 lines)
   now paint the click's new state first and do the work after it, but the work still blocks: a
   tap during the start page's loader (70 ms on a laptop, 280 on a phone) waits for it, and a
   loader cannot be split (`docs/frontend.md`, „A click answers first").
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
   in a Web Worker (`semantic/js`), with its index built on the main thread from rows of sql.js.

What works and is kept: the data contract (`v_*` views, R11); page loaders as plain functions over
a `Database`, most of whose results are serializable already (`CatalogData`, `ModuleData`,
`ProgramData` … derive `Serialize`, a remainder of the hydration plan); the URL as the state that
can be shared; visitor data in the browser only (R20); unknown stays unknown (R12); a click
answering in the next frame (R21); the start without a network; the two languages (R23); the checks.

## 2. Goals and non-goals

**Goals**

- **G1** Two kinds of routes: the **public site** (server-rendered, complete without JavaScript)
  and the **app** (JavaScript only).
- **G2** **One shell**: header, navigation, sidebar, content, the panel beside it and the footer are
  built once and filled by the pages.
- **G3** The **main thread draws** and takes input; queries and computations run in **workers**;
  data reaches the UI asynchronously, and every wait has a defined look.
- **G4** **Every system is a crate** with its own public API, state, texts, styles and tests; the
  dependencies between crates are rules, and a test checks them.
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
                                 │    /   /catalog   /catalog/module/<id>   /impressum   /datenschutz
                                 ├─ the app document for every other route: a head written per
                                 │    route (title, description, card), the shell's static frame
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

### 4.1 Which route is what (proposal)

| Route | Today | New | Why |
|---|---|---|---|
| `/` | server-rendered, the app takes over | **site** | the entrance for search engines and first visits |
| `/catalog?…` | server-rendered with the whole filter panel as links and forms | **site**: the paged list and the search; how many filters without JavaScript is question 3 | the way to every module for crawlers |
| `/catalog/module/<id>` | server-rendered | **site** | what people search for |
| `/impressum`, `/datenschutz` | server-rendered | **site** (static text) | § 5 DDG: reachable at all times; costs nothing |
| `/programs`, `/programs/<slug>/…` | server-rendered, indexed | **app** with a written head — or a site page, question 2 | today in the sitemap and in the index |
| `/bookmarks`, `/studyplan` | server-rendered explanation, `noindex` | **app** with a written head | the visitor's data lives in the browser |
| `/studyplan?share=<code>` | its own page: tags and card name the plan's modules | **app** with a written head that names them | a link preview runs no JavaScript |
| `/en/…` | every route | the same split | |
| cards, calendar feeds, `/api/*`, files | server | unchanged | |
| `/sitemap.xml`, `robots.txt` | every page of the site | the site's pages (and the programs, if question 2 keeps them) | |

### 4.2 How the server answers

1. **A site page:** complete HTML, cached per snapshot and build as today, rendered by
   `folia-site` from the same view crates the app uses for these routes (§7.5). The app takes the
   page over once it runs — a fresh render, not hydration, as today.
2. **The app document**, for every other route: the head written per route by plain code in the
   server (title, description, canonical address, `noindex`, the Open Graph tags with the card;
   for a shared Stundenplan the plan's modules), no component rendered; the body is the shell's
   static frame (rail, header, footer, an empty content area with the route's skeleton), the boot
   script, and a `<noscript>` that says the view needs JavaScript and links the site. It depends
   on the address and the snapshot only, so it is cached like a page; the cards it names are drawn
   as today.
3. Everything else as today: files, `/api/*`, cards, calendar feeds, the gate.

### 4.3 What goes away with it

- For the app's routes: their server rendering, their place in the HTML cache and the warm-up,
  `JsOnly`/R15, R9 and R22, and every second form of a control (the program overview's links and
  forms, the program page's lists for crawlers, the Merkliste's and the Stundenplan's
  explanations).
- `enhance.js` as the classic site's helper: what the site still needs stays small; the app's
  behaviours move into the crates that own them (§9).
- The checks of these routes without JavaScript (`bookmarks`, `programs`, `studyplan`, parts of
  `filters`, `module`, `search`, `top`); `crawl.mjs` walks only the site.

### 4.4 What gets worse, said plainly

- **Search engines** lose the program pages (the plan of every study direction with its
  `EducationalOccupationalProgram` data), unless they stay site pages (question 2). A crawler that
  runs JavaScript does not help: `robots.txt` keeps it out of `/api/`, on purpose, so the app never
  starts for it and it sees the app document's empty frame.
- **A first visit to an app route** — a shared program or Stundenplan link — shows the shell and
  the catalog's download (4.4 MB) before any content; today the server's page is there at once
  (question 5).
- Visitors without JavaScript get only the site.

## 5. The shell

### 5.1 Regions

```
┌──────┬─────────────────────────────────────────────────────────────────┐
│      │ header: where the visitor is, the search, the page's actions,   │
│      │         the status of the data                                  │
│ nav  ├─────────────┬────────────────────────────────────┬──────────────┤
│ rail │ sidebar     │ main: the route's content          │ aside: what  │
│      │ (filters,   │                                    │ was picked   │
│      │  sections,  │                                    │ (a module,   │
│      │  actions)   │                                    │  an area …)  │
│      ├─────────────┴────────────────────────────────────┴──────────────┤
│      │ footer: the ground (Impressum, Datenschutz, Datenstand, roots)  │
└──────┴─────────────────────────────────────────────────────────────────┘
 overlay layer: sheets, dialogs, notes, the search's suggestions
 phone: nav = the bottom bar · sidebar = a sheet (filters) or part of the page · aside = the page
```

### 5.2 How a page fills it

- The shell is mounted once, as the parent route of every page (`<ParentRoute>` with an
  `<Outlet/>` for `main`), and never rebuilt by a navigation: its elements, their scroll
  positions, the resize handles and the widths stay.
- A route declares its layout in the route table: which regions it uses, the skeleton of each,
  its area (the tab it belongs to) and where the header's search goes. The shell lays the regions
  out in the frame after the click (R21) and shows the skeleton wherever the page's content is not
  there yet. Today `pending.rs` and `skeleton.rs` do this by hand for every kind of page, and a new
  page means seven edits (§1); here it is one entry in the route table.
- A page puts content into a region with a component that stays part of the page's own tree and
  is mounted into the region's element (a portal):

  ```rust
  view! {
      <Region slot=Slot::Sidebar title=t.filters><Filters url/></Region>
      <Region slot=Slot::Aside open=picked><ModuleView id/></Region>
      <Await shape=Shape::List data=list let:page><List page/></Await>   // main
  }
  ```

  The region's content belongs to the page: it goes when the page goes, so nothing of a page is
  read after the page was disposed (R2, the crash of `docs/frontend-rewrite.md` §3A), while the
  region's element stays where it is.
- The site renders the same frame statically (§7.5): the first paint of a site page is the app's,
  and the takeover moves nothing, as today.
- *Considered:* a route that returns its regions as values (`PageRegions { sidebar, main, aside }`)
  for the shell to render. Easier to type, but the content would belong to the shell, which brings
  back the disposed-signal problem. The spike of phase 0 settles it.

### 5.3 What the shell owns

The navigation (rail and bottom bar with their counts, the tabs' memory: `tabs.rs`), the header
(title, the search field and where it searches, the status of the data — today a pill that
`boot.js` keeps alive with a `MutationObserver`), the resize handles and the remembered widths,
the phone's sheet and its gestures, the swipe along the bottom bar, Esc and „Zurück" (R10, R19),
„Nach oben", the skip link, the landmarks (`header`, `nav`, `main`, `aside`, `footer`, once
each), the theme and the language switch, an error boundary per region, the pending state, and
the birch (crown, wood, ground) as its background. Most of that is spread today
over `App`, `ui::Frame`, `tabs.rs`, `nav.rs`, `pending.rs`, `skeleton.rs` and `enhance.js`.

Two mechanisms become the regions' own behaviour instead of something each feature wires up:

- **The local views** (`local.rs`: a module shown beside a list, „Vollbild" in place, on a phone
  the module as the page, „Zurück" to where it was picked) are what the aside region does; the
  catalog, a program, the Merkliste and the Stundenplan only say where „Vollbild" leads. Today an
  area needs five parts for it (`docs/frontend.md`, „Local views").
- **Where the header's search goes** is part of the route's declaration (modules everywhere, the
  programs on their overview), not a `match` over the areas in the top bar.

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

- **SQLite in the worker, in two steps.** First sql.js moved into the worker as it is: no new
  toolchain, the whole catalog in the worker's memory instead of the page's. Then (spike in phase
  0): rusqlite on `sqlite-wasm-rs`, the same implementation of `Database` the server has, without
  a JavaScript bridge, and with the snapshot in the Origin Private File System (its `sahpool`
  file system, which only a dedicated worker can use — another reason for the worker), from which
  SQLite reads pages instead of holding the whole file in memory. To settle there: rusqlite reaches
  `wasm32-unknown-unknown` only in releases newer than the 0.32 pinned today; `sqlite-wasm-rs`
  compiles SQLite from C, so the C toolchain for `wasm32` that `docs/frontend.md` names as the
  obstacle has to exist in `build-client.sh` and in Nix; and several tabs on one file (a sync
  access handle locks its file).
- **The snapshot moves out of `boot.js` into the worker:** `/api/status`, the download (streamed,
  progress as events to the UI), the schema check (`user_version`), keeping it, opening it, the
  update in the background; one download for all tabs (Web Locks), the other tabs told
  (BroadcastChannel). Possible on top: switching to a new snapshot between two navigations, with a
  note, instead of at the next start (question 7).
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
  for in idle time. `docs/frontend.md` rejected that because a loader blocks the main thread (the
  start page's 70–280 ms); in the worker it blocks nothing.
- **The semantic search behind the data worker:** the data worker builds the index from
  `v_module_vector` and talks to the semantic worker over a `MessageChannel`, so „Ähnliche Module"
  is one request (the closest modules, then the filters), and nothing goes through the page.

### 6.3 The protocol

`folia-protocol` holds the messages: a request type per question with its answer type, serde, in a
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
composition   folia-app · folia-client             folia-worker           folia-site · folia-server
features      home · catalog · module · programs · bookmarks · planner · legal
widgets       folia-widgets
UI base       folia-shell · folia-stores · folia-data
              folia-design
data          folia-protocol                       folia-sqlite
domain        folia-pages (the loaders, what each page shows)
              folia-query · folia-timetable · folia-plans · folia-map · folia-semantic
              folia-routes
              folia-search
base          folia-calendar
              folia-model
              folia-locale · folia-text · folia-pack
beside        folia-cards (drawing, server) · folia-assets (build step)
```

Inside a layer, a crate may use the crates printed below it in that layer.

### 7.3 The map

**Base** — no I/O, compiles everywhere:

| Crate | From today | Holds |
|---|---|---|
| `folia-pack` | exists | link codes |
| `folia-locale` | `catalog::i18n` (`Locale`, `common`), the machinery of `app::i18n`, the number formats of `app::format` | languages and their address prefixes, dates, numbers, semester names, the pattern of a text group |
| `folia-text` | `catalog::text` | the module texts' Markdown reader |
| `folia-model` | `catalog::{db, rows, rows_detail, labels}`, the id checks of `catalog::url` | the contract: `Database`, `Value`, `DbError`, `SCHEMA_VERSION`, the rows, the codes with their labels, what an id is |
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
| `folia-map` | `catalog::graph` | the layout of the program map (the server lays it out once per snapshot) |
| `folia-pages` | the loaders and data types of `catalog::pages` | one loader per page and what each page shows (`CatalogData`, `ModuleData` …): the façade the worker and the site call |
| `folia-semantic` | exists | the semantic search |

**Data:**

| Crate | From today | Holds |
|---|---|---|
| `folia-sqlite` | `catalog::native` | rusqlite's `Database`: the server, the tests, later the worker |
| `folia-protocol` | new | the messages between the UI and the data worker (§6.3) |
| `folia-worker` | new; the snapshot part of `boot.js`, the answers kept in `client` | the data worker: snapshot, loaders, computations, the semantic worker's port |
| `folia-data` | `app::data`, the bridge of `client` | `DataClient`, its cache, a fake for tests |

**UI base and widgets:**

| Crate | From today | Holds | Kind |
|---|---|---|---|
| `folia-design` | the primitives of `ui.rs`, `icons.rs`, `combobox.rs`, the slider, the gesture of `swipe.rs`, the DOM half of `nav`, the tokens and the base of `app.css`, parts of `enhance.js` | tokens, base styles, components (button, switch, segmented row, chip, badge, tabs, sheet, picker, slider, skeleton, empty state, panel head, prose, icon and sprite), behaviours (resize handle, sheet, swipe, carousel) | iso |
| `folia-shell` | the chrome of `App`, `ui::{Frame, Plain, BackLink, ToTop}`, `tabs`, `local`, `pending`, `skeleton`, `ground`, `languages`, `launch`, the layout half of `nav`, parts of `enhance.js` | the shell of §5 | iso (its static frame) |
| `folia-stores` | the stores of `bookmarks`, `studyplan`, `myprogram`; the view settings in `localStorage` (`betula.finder`, `betula.plan.shape` …) | what the visitor keeps: one store type for all of them (load, check, save, follow the other tabs), undo and the question before throwing away | web |
| `folia-widgets` | the view of a module (`pages::module`), the row of a list, `week.rs`, the switches „Merken" and „Einplanen" | what several features show | iso |

**Features** — each its routes' pages, sidebars, texts and styles:

| Crate | From today | Kind |
|---|---|---|
| `folia-home` | `pages::home` | iso |
| `folia-catalog` | `pages::catalog` (the name is free once today's `folia-catalog` is split up in phase 1) | the list iso, the rest web |
| `folia-module` | `pages::module` | iso |
| `folia-programs` | `pages::{programs, program}` | web (iso if they stay site pages, question 2) |
| `folia-bookmarks` | `pages::bookmarks` | web |
| `folia-planner` | `pages::studyplan::*` (9,800 lines, the largest feature) | web |
| `folia-legal` | `pages::legal` | iso |

**Composition and server:**

| Crate | From today | Holds |
|---|---|---|
| `folia-app` | `App`, the routes | the route table and the services; thin |
| `folia-client` | exists | the UI bundle's entry |
| `folia-site` | new | the site's pages, composed of iso crates; its own short route list |
| `folia-cards` | `server::{cards, launch, birch, logo}` | the drawn pictures (resvg): cards, launch screens |
| `folia-assets` | `server/build` | one stylesheet from the crates' styles, the sprite, minifying (a build dependency of the server, as today) |
| `folia-server` | exists | HTTP, snapshot client, cache, `/api/*`, calendar feeds, gate |

That is 34 crates, and one of test support (§7.4). Fewer and larger is possible (question 10);
the layers and their rule matter more than the number.

**What the server must stop reaching into.** Today it uses page internals of the app:
`app::pages::catalog::PickerChoices` and `app::pages::programs::ProgramsReady` (made per
snapshot in `server/src/snapshot.rs`), `app::format` for the cards' texts, `app::ui::{Mark,
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

The tests move with their crates. What many of them share (`catalog/src/tests.rs`'s `open` and
`studyplan_db`, `area_fixtures`, the fixtures of `timetable::model::tests`) becomes a crate of
test support, a dev-dependency only; `every_query_runs_against_the_snapshot` stays the guard of
`folia-query`. Two statements run only in the tests today and get their caller or go:
`queries::module_vectors` is what the data worker builds the semantic index from (today `boot.js`
reads the vectors in JavaScript), `queries::search_suggestions` has none; `url::CALENDAR_PREFIX`
repeats `timetable::subscription::CALENDAR_PREFIX` and goes.

### 7.5 The site out of iso crates

- An iso crate builds without `web` as markup over data (what the site renders) and with `web`
  adds its behaviour. Today's `ssr`/`csr` split of one crate of 32,600 lines becomes a feature of
  a handful of small ones: `folia-design`, `folia-shell` (the static frame), `folia-widgets`, and
  the site's features (`home`, the catalog's list, `module`, `legal`).
- `folia-site` composes them: the shell's static frame with the page's sidebar and main, the same
  markup the app makes, so the takeover moves nothing.
- The site does not need the app's route table, its stores, `DataClient` or any feature beyond its
  own routes; the server no longer compiles the app.

### 7.6 Styles, texts and icons per crate

- **Styles:** every UI crate has its stylesheet, its classes under the crate's prefix (`ds-` the
  design system, `sh-` the shell, `cat-` the catalog …) and its cascade layer per layer of §7.2
  (`@layer tokens, base, design, shell, widgets, features`). `folia-assets` (today's
  `server/build`) collects them in the order of the layers into the one stylesheet a page loads,
  minified and kept as today. A test fails on a class outside its crate's prefix and on a
  custom property no token defines. `app.css`'s 3,270 lines go to their owners.
- **Texts:** a crate's texts are in its own module of text groups (R23 as it is); `folia-locale`
  has the pattern, and a crate that lacks a language's text does not compile, as today.
- **Icons:** a crate names the icons it uses; the sprite is made of all of them.

### 7.7 An example: the search as a closed system

`folia-search` holds the catalog's search and the pickers' match and nothing else:

- **API:** `fold` (the same folding as Radix's, checked against
  `internal/normalize/testdata/search.tsv`), the words of a query, the scored search
  (`Plan`, whose table the catalog's list and „weitere Treffer außerhalb deiner Filter" join, as
  today), the resolution of words no title knows (`resolve`, with its three statements), the
  ranking of the pickers (`fuzzy`).
- **Uses:** the `Database` seam of `folia-model`, nothing else: no labels, no texts, no page.
- **Used by:** `folia-query` (the filter as SQL, the matches outside the filters, „Ähnliche
  Module"), `folia-pages` (the resolution), `folia-plans` (the folding of names), the server
  (folded cache keys), and as pure functions the UI where a list is short and already there (the
  pickers' choices, the program overview's search).
- **Tests:** its own; today they are part of `catalog/src/tests.rs`.
- **Today:** spread over `search.rs`, `fuzzy.rs` and three functions of `queries.rs`, and part
  of the cycle above.

The semantic search (`folia-semantic`) stays a crate of its own: the two are different systems
and meet only in the catalog's loader („Ähnliche Module").

### 7.8 Compile times and size

On 2026-09-23 `folia-app` was split into crates by layer and page and measured
(`docs/frontend.md`, „Build times"; branch `claude/web-tier-compile-time-0a192a`): an edit in a
page got faster (8 instead of 10 s; 6.3 instead of 11 s for the browser app), an edit in `ui`
slower (12.5 instead of 9 s, every page compiles again), rustc's frontends took 49 instead of
23 s in all, and the bundle grew by 3.6 % gzipped. It was not taken. This concept differs in
what made that split expensive: the app compiles once (only the iso crates for the server as
well); the design system is a stable bottom layer once it is built, not the `ui.rs` every page
edits; and the bundle splits into two (UI, worker), each with less in it. Measured in phase 0 on
a skeleton of the layout before anything is moved; `erase_components` stays.

### 7.9 Where the crates live

```
folia/
  base/       pack  locale  text  model  calendar
  domain/     search  routes  query  timetable  plans  map  pages  semantic
  data/       sqlite  protocol  worker  data
  ui/         design  shell  stores  widgets
  features/   home  catalog  module  programs  bookmarks  planner  legal
  app/        app  client
  server/     site  cards  assets  server
```

Today the crates stand at the top of the repository beside Radix's Go code. The scripts,
`flake.nix` (its list of source directories), `deploy/ship.sh` (it reads `app/src/pages/legal.rs`)
and the build cache follow the move.

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
| `pages::catalog::phone_layout` | program, Stundenplan, Merkliste | `folia-shell` (which layout is on) |
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
| `semantic/js` | the worker and its face on the page | the worker stays; its face moves into the data worker |

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
| R17 every page is framed | becomes the shell's structure |
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

## 11. Steps

Every step is merged into `develop` as usual (CLAUDE.md), the site and the checks work after each
one, and canary gets it when the owner asks for a release. Sizes are relative.

| Phase | What | Size |
|---|---|---|
| 0 | **Spikes and answers.** (a) SQLite in a worker: sql.js moved vs rusqlite on `sqlite-wasm-rs` with the Origin Private File System; the catalog page's data including the messages, on a phone; memory; two tabs; the Nix build. (b) The shell with regions under Leptos 0.8: portals and disposal, transitions with the threshold, skeletons per route, the takeover without a flash. (c) Styles and texts per crate through `folia-assets`. (d) Compile times and bundle sizes of the crate layout on a skeleton. Plus the answers of §12. | M |
| 1 | **The domain out of `catalog`** into its crates (§7.3, §7.4): the three cycles broken, then moves; no behaviour changed, the tests go along. | M |
| 2 | **Site and app.** `folia-site` renders the public routes; the other routes get the app document (its frame is today's `App` with an empty page until the shell of phase 3 exists); their server rendering and their second forms go; sitemap, robots.txt, warm-up and the checks follow. | M |
| 3 | **Design system and shell.** `folia-design`, `folia-shell` with its regions; pages fill regions instead of building `ui::Frame`; the per-crate styles and texts. | L |
| 4 | **The asynchronous seam.** `DataClient` with its async API, first on the main thread's sql.js; the pages move to resources one by one; `pending.rs` shrinks to what the shell keeps. | L |
| 5 | **The data worker.** `folia-protocol`, `folia-worker`; `DataClient` switched to it; the snapshot out of `boot.js`; sql.js into the worker or replaced; the semantic search behind it. | L |
| 6 | **Features as crates;** `folia-app` becomes the composition root; the dependency test (R24) on. | M |
| 7 | **Rules and documents:** `docs/frontend.md` rewritten for the new structure, the checks consolidated, measured again (`snappy.mjs`, `folia assets`, `e2e/load`). | S |

Phases 3, 4 and 6 can go page by page together: a page moves into its crate, fills the shell's
regions and reads its data asynchronously in one round. Phase 2 comes early on purpose: it deletes
the most code, and everything after it needs to work in one renderer only.

The checks in `e2e/` are the safety net of every phase: they describe what the owner approved,
interaction by interaction, and a phase that cannot keep them green is cut smaller. Phase 2 removes
the checks of the app's routes without JavaScript; new ones come with the worker: a first visit on
an app route, a worker that dies and is started again, two tabs and one download, the start
offline with the snapshot kept by the worker.

What counts as done:

- `snappy.mjs` as today (the first frame within 80 ms, 200 ms with the CPU four times slower), and
  no main-thread task over 50 ms from a query or a computation during the checks.
- The UI bundle no larger than today's 1.43 MB in brotli; the worker's measured and stated.
- Memory of a phone's tab with the app stated, before and after the Origin Private File System.
- An edit in one feature crate rebuilds the browser app in less time than today's 11 s
  (`build-client.sh --dev`).
- The dependency test (R24) green; every crate with its own tests.

## 12. Questions for the owner

1. **Which routes without JavaScript?** „Die Startseite und den Modulkatalog + pages" read here as
   `/`, `/catalog` and the module pages `/catalog/module/<id>`. And the legal pages as site pages
   too (recommended: § 5 DDG, and they cost nothing)?
2. **The program pages:** may they leave the index (app only), or stay site pages as a third
   public route? Today the sitemap lists every current program with its plan, its areas and the
   plan of every further study direction, and a search for a program of the BTU can find them.
3. **The catalog without JavaScript:** every filter as today (links and forms), or the list, the
   search and the pages only? The second keeps far less of the filter panel in two forms.
4. **Link previews of the app's routes** (a program, the Merkliste, a shared Stundenplan): keep
   their titles and cards through the head the server writes (recommended), or drop them?
5. **A first visit to an app route** (a shared program or Stundenplan link): the shell and the
   catalog's download (4.4 MB, with its progress) until the content is there — acceptable?
6. **The frame:** „die Sidebar mit der Navigation" — is that the rail (the main navigation), the
   page's sidebar (filters, sections, actions), or should the two become one column? Is the footer
   the ground? Does the look stay as it is, and only the structure changes?
7. **A new snapshot during a visit:** switch to it between two navigations, with a note, or at the
   next start, as today?
8. **Leptos** stays (assumed here)?
9. **Pace:** the phases one after another on `develop` — with feature work paused meanwhile, or
   alongside it?
10. **Granularity:** 34 crates as in §7.3, or fewer and larger?
