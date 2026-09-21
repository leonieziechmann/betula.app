# Folia, the web tier: architecture, rules, how to run it

> Betula has two parts named after the birch: **Radix** (the root: the Go collector, `docs/operations.md`)
> and **Folia** (the leaves: this web tier, the crates `folia-catalog`, `folia-app`, `folia-client`
> and `folia-server` with the binary `folia`).

> State: 2026-09-20. Every page is server-rendered and works without JavaScript; with
> JavaScript the browser app (WASM + local SQLite) takes the page over and nothing is loaded
> again. Not yet: the service worker of the PWA (the manifest and the icons exist), user data beyond the marked modules, context search.
> Decisions and their evidence: `docs/frontend-phase0.md`.

## 1. Overview

```
Radix ──HTTP──▶ Folia ──HTML (cached per snapshot)──▶ browser
 /snapshot/catalog.db        ──/api/db (gzip, ETag)───▶ browser: local SQLite (phase 2)
```

| Crate | Role |
|---|---|
| `catalog/` | The data contract in Rust: row structs, labels, `CatalogQuery` → SQL, every query, the page loaders (`pages.rs`) and the URL scheme (`url.rs`). No I/O; callers hand in a `Database`. Compiles natively and to WASM. |
| `app/` | The Leptos components. Feature `ssr` for the server, `csr` for the browser app. Pages get their data through `data::Source`. |
| `client/` | The browser app (WASM): `app` with feature `csr` on a `Source` backed by sql.js. Not a default workspace member (its `csr` would be unified with the server's `ssr`); built by `scripts/build-client.sh` into `site/pkg`. |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `spa.mjs` (the browser app: takeover, no page loads, preview, filters, search), `smoke-walk.js` + `run.mjs` (long program walk), `shot.mjs` (review screenshots). All use an installed Edge through `playwright-core`. |

### Routes (`catalog/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `kind`, `lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `marked`, `prereqs`, `sort`, `desc`, `page`. What can be wanted can also be excluded: `not-kind`, `not-lecturer`, `not-turnus`, `not-form`, `not-exam`, `not-campus`, `not-lang` (`exam=written&not-exam=presentation`: a written exam and no presentation) |
| `/catalog?…&open=<id>` | The same list with this module previewed next to it; the preview has a „Vollbild" link to the module's page. On a phone there is no preview: a tap on a row opens the module's page, and the app turns a shared `open` link into it |
| `/catalog/module/<id>` | The module's own page: a sidebar as wide as the filter panel (sections of the page, actions), the module on the rest of the screen |
| `/programs?q=…&level=…&form=…&plan=1` | Program overview (current PO versions) by faculty (`ProgramsUrl`): the search of the top bar, degree (`bachelor`, `master`, `teaching`, `doctoral`, `other`), form of study (`dual`, `double`, `flexible`), only with a validated study plan |
| `/programs/<slug>/plan\|areas\|modules[?variant=<n>][&area=<id>][&req=<n>][&open=<id>]` | Program page (`ProgramUrl`); its views are switched in the sidebar. Where a program has several study plans (one per study direction), `variant` says which one is shown; `area` is the area of „Wahlpflicht & Bereiche“ shown beside the page, `req` a row of the plan that names no module, `open` the module — all of them are content, so they stand in the address, work without JavaScript and are part of the server's cache key; the canonical address stays the plain one. A module opened out of an area keeps it, so closing the module returns to it. On a phone `open` leads to the module's own page, as in the catalog |
| `/programs/<slug>/plan\|areas\|modules` | Program page; its views are switched in the sidebar |
| `/bookmarks?turnus=…&sort=…&desc=1&open=<id>` | „Merkliste": the modules the visitor has marked (`BookmarksUrl`). The URL says how the list is shown (half of the year, order, the previewed module), never what is on it: the marks live in the browser. The server renders an explanation, the same for everybody, `noindex` |
| `/impressum`, `/datenschutz` | The legal pages (`app/src/pages/legal.rs`), linked from the start page's sidebar. **Placeholders since 2026-09-21**: while `legal::PLACEHOLDER` is true they say so, list what is still owed and are `noindex`, and `deploy/ship.sh` refuses to ship an instance open to everybody (`FOLIA_ACCESS_GATE` not `on`). Before going public: the real texts (owner's name, address, contact; the privacy notice naming the edge's access log, the gate's cookie, what stays in the browser, the lecturers' names), `PLACEHOLDER = false`, and links from every page, not only the start page (§ 5 DDG: reachable at all times) |

The catalog parameters are tolerant (repeated or comma-joined values, empty inputs of a plain
HTML form, nonsense ignored) and have one canonical spelling, which is also the cache key. A
value that is both wanted and excluded counts as wanted. An exclusion removes only what the data
states: a module whose campus or turnus is unknown stays in the list (R12). Lecturers: the
wanted ones are alternatives, the unwanted ones are all left out: (Meer or Köhler) and not
(Lambers or Hofstedt).

### Look and interaction (since 2026-09-19, owner-approved direction)

- **Name and logo** (owner decision 2026-09-20, domain `betula.app`): the product is **Betula**
  (the birch; B-T-U stands in the name). Running text says „Betula", only the wordmark stresses
  the three letters: BᴇTUʟᴀ in Inter 800 with E, L and A as small capitals and the E under the bar
  of the T (`ui::Wordmark`, `.wordmark`; the spacing is measured, not guessed). The mark is birch
  bark that also reads as the rows of a list (`ui::Mark`, 32 px grid; favicon on the same grid).
  The rail carries the mark, the top bar of the start page the wordmark with „Modulkatalog ·
  inoffiziell", so the two read as one logo; on a phone the start page carries both itself.
  „Inoffiziell" always stays with the name, and nothing borrows the university's colours or mark.
  Sizes, grids and numbers: `design/logo/logo.html`.
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
  - A chosen lecturer is a row: on the left the switch between + (wanted) and × (unwanted), the
    name with the academic title under it, on the right the button that takes the person out.
  - Entries of the program picker have one shape: name, short degree, year of the PO. The form of
    study is added only where two programs would otherwise read the same.
  - What the pickers offer is loaded once per visit (`pages::catalog_choices`), not with every
    list, which keeps a filter change to the queries of the list itself.
- **A module reads the same wherever it is opened:** preview and page are the same parts in the
  same order (times and key facts, then the description); a wide page puts the halves side by
  side, the description on the left. The page keeps the frame of the catalog, so nothing jumps
  when a preview becomes a page: its sidebar has the width and the handle of the filter panel.
  The sidebar jumps to the sections of the page (without history entries, so Esc still leaves the
  page) and holds the actions: „Merken", copy the link, the original at the BTU, and the place
  where the semester plan will live.
- **„Merken" (owner decision 2026-09-20: in the browser app only, and no data of a visitor on
  the server; `app/src/bookmarks.rs`, `app/src/pages/bookmarks.rs`).** A visitor marks modules to
  come back to. The marks live in this browser's `localStorage` (`betula.bookmarks.v1`, a line
  per module: id and time of marking, the newest first) and nowhere else: not in a URL, not in
  server HTML, in no request (R9, R13, R20). Another tab of the same browser follows through the
  `storage` event; another device has a list of its own.
  - **Where a module is marked:** at the end of its row in a list (a button *next to* the row's
    link, in a wrapper, because a button inside a link is neither; the row keeps 48 px free for
    it, quiet until the row is pointed at or the module is marked, always shown where nothing
    hovers); in the line of the module's badges (credits, turnus, language) at its right end
    (owner, 2026-09-20), in the preview and on its page, as a switch „Merken" / „Gemerkt" with
    its shortcut; and among the actions of the module page's sidebar, which stays in view while
    the page scrolls. All of them show one state. **`M`** marks what the visitor is
    at: the row the keyboard is on, else the module that is open. A marked module is neutral and
    strong (filled, inverted), like a chosen chip; the accent stays with primary actions.
  - **The rail's fourth item** („Merkliste", also in the phone's bottom bar) carries the number
    of marks. It is a tab like the others (R19).
  - **„Gemerkt" filters the catalog** as well („Eigenschaften": only the marked ones, a second
    click all but them, which is how one looks for what is still missing). The URL carries the
    switch (`marked=only` / `marked=none`), never the ids: the browser fills them into the query
    before it asks (`CatalogQuery::only_ids` / `without_ids`). A page that does not know the
    marks therefore matches nothing with `marked=only` instead of answering as if nothing were
    marked, and says so where the list would be empty — the app says that nothing of the
    visitor's fits the other filters, the server that it cannot know.
  - **The list of marked modules** is the catalog's list in the catalog's frame: sidebar, rows
    with the same columns, the preview of `open=<id>` floating at the right edge, on a phone the
    module's own page. The sidebar holds what belongs to the list as a whole: its numbers
    (modules, credits), the halves of the year as a row of links with their counts („Alle",
    „Winter", „Sommer": what the catalog's turnus filter would find among the marked), the order
    (of marking, the newest first; by title, credits, teaching events; the column headers sort as
    in the catalog), and the actions: copy the list as text, and empty it, which asks first and
    can be taken back („Rückgängig").
  - **A mark taken away on that page stays on the page,** dimmed, until the page is left: a slip
    is one click to undo, and the list does not jump under the pointer. Marking changes numbers,
    never the list: no query runs and the rows stay the same elements (each button reads the
    marks through a memo of its own, R5).
  - A marked module stays on the list when it is no longer offered, and one the snapshot does not
    know (taken out of the BTU's catalog) is named under „Nicht im Modulkatalog", not dropped
    (R12). What is stored is read like anything from outside: ids that cannot be ids are
    dropped, a module counts once, the list ends at 2,000.
  - A module opened from the marked modules (a tap on a phone, „Vollbild" of the preview) leads
    back to them with „Zurück" and Esc, not to the catalog's list (`Tabs::came_from`).
  - **To another device without a server in between:** „Auf anderes Gerät übertragen" copies a
    link to the list with the marked modules in its *fragment* (`/bookmarks#add=11101,12204`).
    A browser never sends the fragment of an address anywhere, neither with the request nor as a
    referrer, so the ids reach neither the server nor its logs. The page that is opened with
    such a link asks before it adds anything (a link must not fill somebody's list behind their
    back), says how many of the modules are new, and either answer takes the ids out of the
    address and out of the history entry. It also notices a fragment that arrives while the
    list is open (`hashchange`; the router only learns of fragments when a page is opened).
  - **Without the app** nothing of it shows: the buttons are not part of server-rendered rows
    (their room is, so nothing moves at the takeover), the switch beside the badges and the
    sidebar's action are rendered unpressed and kept invisible until the app runs
    (`visibility`, so their room is kept too), and without JavaScript all of it is gone (R15).
- **The areas are tabs (owner decision 2026-09-20, R19; `app/src/tabs.rs`):** the items of the
  rail (and of the phone's bottom bar) remember where their area was left. From another area a
  tab leads back to that place (the open program, the filtered list with its preview); on a page
  inside the area (a module, a program) the area's own tab leads up to the area's list as it was
  left; on the list it is the plain link. The same memory serves „Zurück" on a module's and a
  program's page (`ui::BackLink`; Esc does the same): it leads to the area's list, through the
  browser history if that list is where the visitor came from (`data-back="history"`, so the
  history does not grow), and as a plain link otherwise (after a change of tabs „Zurück" leads
  up, not back to the other area). And it tells a list which row to show again: the catalog
  scrolls to the module whose page was open just before, the program overview to the program.
  The memory is personal state: `sessionStorage`, never the URL, never server HTML; without the
  app every tab is the plain link to its area. **A module opened out of a program does not become
  what the catalog remembers** (owner, 2026-09-20): its tab keeps leading to the list as it was
  left, and that list does not reveal a module the visitor never picked there. A module opened
  beside a program and then taken to
  its own page („Vollbild") leads **back to that program**, not to the catalog: the step before
  answers, and after a reload the area's own memory does (it was left at a program page that
  names this module in `open`). Where „Zurück" leads is read again on every change of the
  address, so after closing the module beside a program Esc follows the link out of the program
  instead of walking the history back into the module it has just closed.
- **Every page has the same frame (owner decision 2026-09-20, R17):** a sidebar as wide as the
  catalog's filter panel, with the same handle and the same remembered width, and the page next
  to it (`ui::Frame`; the catalog builds it itself, its sidebar is the filter form). Going from
  one area to another, nothing jumps. The sidebar holds what belongs to the page as a whole:
  filters (catalog, program overview), the views of a program, the sections of a module, actions,
  and on the landing page the state of the data. On a phone a sidebar of filters is a sheet
  opened by the page's „Filter" button, views stay on top, everything else follows the page.
- **The program overview** is one section per faculty, and in it a matrix: a row per subject, a
  column per cycle of study (Bachelor, Master, the rest; Lehramt counts to its cycle). 148
  programs read as about 80 rows, 72 of 98 cells hold one program, and the columns are the same
  in every section, so the whole page stands on a few vertical lines. A program that is also
  offered in other forms of study carries them as segments of the same control
  („B.Sc. 2022 | dual, Ausbildung | dual, Praxis"); a segment's full name is its `aria-label`.
  A column nothing is left in after filtering is not drawn. Programs without a validated study
  plan are the quieter links. Tried before and dropped as clutter (owner, 2026-09-20): cards,
  and the rows set in several text columns; rows of unequal height with nothing to line up on.
  **The faculty is derived, not stated** (no source names a program's faculty):
  `catalog::pages::faculties` takes the department of the thesis module, else the department
  that offers at least half of the offered curriculum, else what the programs of the same
  subject agree on. The sidebar says so, and programs without a clear answer have a section of
  their own („unknown stays unknown", R12). The 2026-09-19 snapshot: 106 by thesis, 34 by
  majority, 4 by subject, 4 without.
- **The program page** (reworked 2026-09-20, second round; the first one was „unaufgeräumt"): the
  head is three lines that start on the same edge — where the visitor is („Zurück", the path),
  the name with the numbers of the program right of it on its baseline (Semester, LP, Module,
  FÜS-Module; length and size are what the validated plan says, so they are only there when there
  is one), and under it the line of the regulations (degree · PO · form of study · „aktuell" or the
  flag of an older PO). Everything that lists modules is **one table with the same columns**
  (Nr., Modul, Art, LP, Turnus, Semester): areas and semesters are groups of rows inside it,
  never boxes beside it, rows are one line high, and the views stand on the same vertical lines.
  **A program has one study plan per study direction** — they used to be drawn as one table, so
  Elektrotechnik B.Sc. listed modules twice and counted 60 LP in its first semester instead of 30
  (33 of the 140 programs with a plan have more than one). The chosen plan is `?variant=<n>`; the
  chips name it without the boilerplate of the PDF caption and carry the full caption as their
  title. The plan is drawn as its regulations print it — a row per module, a column per semester,
  the credits in the cell, modules over several semesters spanning their columns, the sums
  underneath — or as a list, semester after semester with its sum. Which of the two is personal:
  `localStorage`, the switch is in the sidebar and needs JavaScript (R9, R15); server HTML is
  always the matrix. Only what the plan puts into a single semester is added up in that
  semester's column; a footnote says so where a plan has modules over several semesters.
  **The page has a panel on the right** (`ui::Frame`'s `aside`, as wide as the catalog's preview,
  same handle, same remembered width): a module clicked in any of the three views opens in it
  (`?open=<id>`, the same panel as in the catalog, so a module reads the same wherever it is
  opened); an area clicked in „Wahlpflicht & Bereiche" — in the table or in the sidebar, which
  also brings it into view — shows what it holds (`?area=<id>`: its numbers, the areas under it,
  and its modules, each of them opening in the same panel and coming back to the area when it is
  closed); a row of the study plan that names no module of the catalog — most of them are
  requirements („Wahlpflichtmodule der Studienrichtung", „Wahlpflichtmodul aus der Informatik") —
  shows what the plan states about it and where the modules that can be chosen are listed
  (`?req=<n>`, the row's place in the chosen plan). **No source links a row to an area**
  (`area_rules` is prose about credits), so the name does the work and the panel says so: the
  labels of the areas are scored against the row's name (a distinctive word counts, „Wahlpflicht­
  modul" hardly), only the study directions the plan's caption names are kept („MIT und EET" →
  never the areas of PA or IoT), a single best fit is shown with its modules, and where two fit
  equally well both are named instead of one being picked (R12). A row the plan states as
  Pflicht, Abschlussarbeit or Praktikum means one module, not a choice: it gets no area at all,
  only the honest note that the catalog does not know it under this name and a search for it. And with nothing picked the panel holds the
  numbers of the view one is looking at — for the
  plan the chosen study direction with its semesters, credits and how much of it the catalog
  links, plus the credits per semester as bars; for the other views the areas and how the modules
  split by the kind the program states them as (every kind that occurs, „Art nicht angegeben"
  where no source says; nothing is counted into a kind it was not stated as). Unlike the catalog's preview it is a column of
  the layout, not a panel over the page: the tables keep the room that is left and give up the
  columns that carry least (Bereich, Turnus, Nr.) as it gets narrower. Tried before and dropped
  (owner, 2026-09-20): the matrix as a centred block in a wide empty panel — „liest sich zwar
  leichter, sieht trotzdem komisch aus"; the width wants content, not air.
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

### The landing page and the map of the programs (2026-09-20)

`/` answers three questions at a glance (`app/src/pages/home.rs`): what this is (headline, „inoffiziell"
in the first line), what is in it (four figures, and the map as the one picture of the page), where
to start (two buttons, entry links with their exact counts, the faculties). Below
that: what the app does, and questions and answers in plain text. That text is what the page is
found by; it only says what the app really does (the search covers titles and numbers, so it says
that).

**Second round (2026-09-21, owner: „unaufgeräumt und anstrengend für die Augen"; the sidebar and the
big map did not work; a fellow student missed colour; then: save height in the head, a carousel
that behaves like one for pictures, more questions; round 4: the figures bare beside the text,
„birch feeling", no example searches, snappy motion, autoplay with a pause button, the map dialog
for wide screens, the page still behind it).** The first panel is flat: text and the two buttons on
the left, the four figures on the right from a container width of 700 px on (the owner's notebook),
under the text below it. The figures (`dl.birch`) stand on birch bark (`--bark`, light in both
themes like the mark), one under the other with thin rules, and on each rule a stroke from
alternating edges, the strokes of the mark; each number is set so large that all of them are about
equally wide (`--em`, its width in units of its size, from `figure_em`; a single digit grows only
as large as three), quiet in weight and colour (owner: „kleiner und etwas dezenter"). Under it a
**carousel**: the map, then screenshots of the catalog, a study plan and a module page
(`app/assets/shots/*.webp`, light and dark, wide and phone, made by
`node e2e/showcase-shots.mjs <base-url>`, embedded in the binary, served under `/assets/shots/`; run
it again when those pages change). The current picture stands in front, half of each neighbour shows
behind it at its sides (owner: they overlap, as if behind it), it goes round (four slides on top of each other, placed by `--at`; before a move the one
that comes in is put on its side without animation, then everything moves one frame later, so no
picture crosses the frame; moves overshoot a little, `--spring`, and the caption of the picture
that comes forward slides in). It turns on by itself when the bar in the current tab is full (a CSS
animation; `animationend` moves it on), also under the pointer and where less motion is wanted (the
owner wants it on by default; the pictures then change without moving). The pause button beside
the tabs stops it; the browser remembers a stop (`localStorage` `betula.showcase` = `paused`,
nothing while it plays, R20); the open map stops it too. The mark of the current tab slides to it
(on a phone, where the names need their own widths, the tab itself is marked). The frame reaches
14 px above and 44 px below its place (padding taken back by negative margins, pointer events
passed through) so that shadows, the lift under the pointer and the overshoot are not cut. Arrows,
tabs, arrow keys, a swipe and a click on a neighbour turn it; a click on the current screenshot
opens its page, a click on the map opens it large in a `<dialog>` (the interactive map: hover,
pick, faculty outline, link; the first Escape puts a pick away, the next closes; the page behind
it does not scroll, `:root:has(.map-dialog[open])`). On a wide screen (≥ 1100 px and wider than
7:5) the dialog is the 4:3 map as high as the screen allows with a column on its left: the head,
the legend (with the line for shared modules) and what is shown — the program, its faculty, its
five closest relatives with the number of shared modules, the link. Owner: the carousel and the dialog only have
to work in the app; without JavaScript the map shows and leads to the program overview. Every
screenshot is lazy with `loading` written before its address (the app sets attributes in order; an
image with an address and no `loading` yet is fetched at once), so only what shows is fetched,
never the hidden theme or the phone's pictures. Colour: washes of the faculties' palette behind the
pictures, the ways into the catalog with a soft hue each (`--t-*`), the faculties in their map
colours, the abilities in the accent's tint. The questions are an accordion in two groups, „Über
Betula" and „Fürs Studium" (first semesters); the text stays in the page and in the FAQPage data.
The sidebar: the sections (the current one follows the scroll, `nav[data-spy]` in `enhance.js`),
the Datenstand and the versions as label | value rows — Folia's is `app::VERSION` („alpha-" + this
crate's version), Radix's is `meta.radix_version` of the snapshot (`internal/version`; older
snapshots say nothing, the page then says „nicht angegeben") — and at its foot Impressum and
Datenschutz.

The map (`catalog/src/graph.rs`): a dot per current program, a line where two curricula share
modules (Jaccard; modules of more than 40 programs are ignored), a force layout without
randomness. **The server lays it out once, when a snapshot is opened** (`Snapshot::open`,
event `snapshot.map_built`), for a 4:3 sheet (the carousel; 2:1 until 2026-09-21) and a tall one; nothing is laid out while a page
renders and nothing in the browser (owner decision). Server-rendered pages get it through context
(`data::ProgramMapHandle`), the browser app as `GET /api/map.json` (about 8 KB gzip; `boot.js`
fetches it next to the database and keeps a copy in IndexedDB; `window.betulaMap`). Without a map
the section is left out. The links are three `<path>` elements per sheet, the dots are SVG links
with `<title>` (so the map works without JavaScript and search engines follow the dots to the
programs). The app adds what a pointer over a dot shows (one signal, one overlay: its links, its
relatives, a line of text) and takes clicks itself. **A click picks a program, it does not open
it** (owner 2026-09-21: it happened by accident all the time): in the dialog the caption under the
map names it, its faculty and its closest relatives and links to the program (until the carousel a
card below the map listed all relatives as buttons; now they stand out on the map); a click beside
the dots, the ✕ or Escape put it away. Modifier clicks stay
plain links. A halo of 5 units around every dot takes the pointer too (R14); the halos lie under
all dots.

Faculties on the map: every program carries its derived faculty (`pages::faculties`, grouped by
number, 1 to 6; the snapshot lists 1 to 4 under their old and new names). In the layout the
faculty is only a faint hint (owner, twice: „wirklich nur ganz leicht"): all programs start mixed,
a very weak pull to the faculty's middle, links across faculties pull a little less; what
programs share decides. After the simulation every dot moves part of the way towards an even
spread per axis (`EVEN`), so the sheet is filled without dense clumps and empty stretches. A
faculty therefore lies in several places: its outline is one smooth shape per island (dots
closer than about the spacing of an even sheet; `ISLAND`), each a blurred convex hull drawn a
little generously, with the name at the largest island. The app shows only the outline of the
picked program's faculty, and the programs of the other faculties step back a little.
`node e2e/home.mjs` covers it.

### Search engines (`app/src/seo.rs`, 2026-09-20)

Aim: a search for a module or a program of the BTU finds the page here. What that rests on:

- **Every page states itself once** with `seo::Seo` (inside its frame): description, canonical
  address, Open Graph tags (picture: `app/assets/og.png`, made from `design/og/og.html`) and
  structured data. Nothing of it is set for the whole app. (Before, every page carried the app's
  default description, program pages a second one, and the module page lost its own.)
- **One address per page.** Filters, further pages and the preview of the catalog, and a filtered
  program overview are views: `noindex, follow`. `/programs/<slug>` names `/programs/<slug>/plan`
  as its address. Older examination regulations are `noindex`. Links that only lead to views
  (examples, entry links, toggles) carry `rel="nofollow"`.
- **Titles start with what people search for**: „<Modultitel> (<Nummer>) · Modul der BTU
  Cottbus-Senftenberg · Betula", „<Studiengang> (<Abschluss>): Regelstudienplan · BTU
  Cottbus-Senftenberg · Betula" (each view of a program has its own title).
- **Structured data states only what the page shows:** `WebSite` with its search and `FAQPage` on
  the landing page, `Course` (code, credits, language, provider, `sameAs` the BTU's page) and
  `BreadcrumbList` on a module, `BreadcrumbList` on a program.
- **`/sitemap.xml`** (made once per snapshot): the three entrances, every module that has a page,
  every current program with its views; `robots.txt` names it. Addresses are absolute and use
  `--public-url` (`SiteUrl` in the app; the browser app uses its own origin).
- The browser app removes the server's tags from the head when it takes over and writes its own,
  so the head describes the page that is shown.
- **Link previews** (messengers, Slack, Discord, X): the card is the page's own title and
  description with a picture (1200 × 630, absolute address, with type, size and alt text); X
  gets its `twitter:` twins, because only with them the large card shows everywhere. A preview
  never runs JavaScript, so all of it is in the server's HTML.
- **A module and a program have their own picture** (`/cards/module/<id>.png`,
  `/cards/program/<slug>.png`; `seo::module_card`, `seo::program_card`): the logo, the kind and
  number, the title (four sizes, at most four lines, then „…"), a line of facts and a quieter
  one (department; size of the curriculum). Every other page names the standard picture
  `og.png`. The server draws the cards itself (`server/src/cards.rs`): an SVG put together in
  Rust, set by resvg in static cuts of Inter (`server/assets/inter-*.ttf`, cut once from the
  app's variable font by `design/cards/make-fonts.py`, a development tool; nothing but the
  server's own process runs in production), measured with the same shaper before it is set,
  written as a palette PNG (20–30 kB). About 15 ms of drawing and 65 ms of packing in a dev
  build; the pixel crates are optimised in the dev profile for that.
  - **Kept:** a finished card stays in memory (`--card-cache-mb`, 64 MiB ≈ 2,500 cards) under
    the hash of what it says, so a new snapshot only redraws the cards whose text changed. The
    hash is also the ETag (`If-None-Match` → 304); `Cache-Control: public, max-age=86400`.
  - **Never in the way of the pages:** drawing runs on the blocking pool, at most half the
    processors at once (1–4). If every place is taken, or there is no snapshot, or drawing
    fails, the answer is `og.png` at once with `no-store`, so the next fetch gets the real card.
    An unknown module or program is a 404. Look at the design with
    `FOLIA_CARD_OUT=<dir> cargo test -p folia-server cards_for_review`.
- **Who the site is, outside a page** (static in the document's head, `app::shell`, so it
  survives the takeover): `/favicon.ico` (32 and 48 px) and the SVG icon, `/apple-touch-icon.png`
  (180 px, full bleed: iOS rounds it and uses it for the home screen and for previews in
  Messages), `/manifest.webmanifest` (name, colours, icons 192/512 and a maskable one) and
  `theme-color` (the page background; the head script and the theme switch turn it dark). The
  pictures are made from the mark's grids by `node design/logo/render-icons.mjs`. The manifest
  makes the site installable; it does not make it work offline (no service worker yet).

Not done: submitting the sitemap to the search consoles (needs the owner's accounts), a
`lastmod` per module (the snapshot has no date per module), English pages.

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
- **R19. The areas are tabs** (see „Look and interaction"): a new area gets an entry in
  `tabs::Area`, and a page inside an area uses `ui::BackLink`.
- **R18. The page follows a resize handle live, so pages have to be cheap to lay out.** The
  widths of filter panel and preview are custom properties on `<html>`, written at most once a
  frame while a handle is dragged; every write lays out the whole page (owner, 2026-09-20: the
  live feel is wanted; moving only the panel and catching up on release felt worse). What keeps
  that at 60 fps (`node e2e/resize-perf.mjs` measures frame times on every kind of page):
  - No layout whose cost explodes with its width: the program overview set in CSS text columns
    had to rebalance all columns with every pixel and stuttered; the matrix of grid rows does not.
  - Long lists do not lay out what is off screen: `content-visibility: auto` on the catalog's
    rows (with 350 rows loaded: 58 of 138 frames over 33 ms before, 1 after). The rows are direct
    children of the scrolling list, so this works per row; the last height is remembered.
  - A safety net for pages or machines that still cannot keep up: after three frames in a row
    over budget, the rest of that drag moves only the panel (inline width, above its neighbour)
    and the property is written when the handle is let go (`data-resize-mode`, and
    `data-resize-budget` to force it in `filters.mjs`).
- **R17. Every page is framed** by `ui::Frame` (see „Look and interaction"). A new page starts
  with the question what its sidebar holds, not whether it has one.
- **R16. In one reactive closure read the source, not a memo derived from it and the source.**
  reactive_graph 0.2.14 does not mark the observer that made a memo recompute as dirty. A closure
  that reads `state` (a memo derived from `query`) and then `query` therefore misses a change of
  `query` whenever `state` keeps its value: checking `state` recomputes `query`, `state` reports
  „unchanged", and `query` is clean by the time it is asked. It hits the first such closure of a
  page only (for the others the recomputation has already happened), which made the first toggle
  of the filter panel drop the rest of the filter. `e2e/filters.mjs` checks every link of the
  panel against the whole filter.
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
- **R20. What a visitor keeps stays with the visitor** (owner decision 2026-09-20: no data of a
  visitor on the server). Marked modules, and whatever follows them (passed modules, the own
  program), live in `localStorage` under a versioned key (`betula.<what>.v<n>`), behind one
  store per kind that is provided through context and is empty on the server
  (`bookmarks::Bookmarks`). They never become part of a URL (URLs are requested from the server
  and end up in its logs), of server HTML (R9) or of a request; a URL may carry how such data is
  shown, never the data. What is read from storage is checked like what comes from a URL.
  `e2e/bookmarks.mjs` watches every request of a session for marks.

## 3. Running it

```bash
./radix.exe serve-snapshot --addr 127.0.0.1:8090
```

```bash
bash scripts/build-client.sh
```

```bash
cargo run -p folia-server
```

The first command builds the browser app into `site/pkg` (needs the `wasm32-unknown-unknown`
target and `wasm-bindgen` 0.2.128, which Trunk keeps in its cache); without it the site simply
stays server-rendered.

Open `http://127.0.0.1:8080`. The server fetches the snapshot over HTTP into `web-data/` and
keeps serving the last good one when Radix is away, also after a restart.

| Flag | Environment | Default | |
|---|---|---|---|
| `--addr` | `FOLIA_ADDR` | `127.0.0.1:8080` | listen address |
| `--snapshot-url` | `FOLIA_SNAPSHOT_URL` | `http://127.0.0.1:8090/snapshot/catalog.db` | Radix's endpoint (plain HTTP inside the deployment network) |
| `--data-dir` | `FOLIA_DATA_DIR` | `web-data` | downloaded snapshots |
| `--poll-seconds` | `FOLIA_SNAPSHOT_POLL` | `60` | check interval (conditional GET) |
| `--stale-after-seconds` | `FOLIA_SNAPSHOT_STALE_AFTER` | `21600` | `/healthz` fails when Radix was silent this long (0: never) |
| `--html-cache-mb` | `FOLIA_HTML_CACHE_MB` | `128` | rendered pages kept in memory |
| `--site-root` | `FOLIA_SITE_ROOT` | `site` | browser bundle (`pkg/`), from phase 2 |
| `--card-cache-mb` | `FOLIA_CARD_CACHE_MB` | `64` | finished link-preview cards kept in memory |
| `--public-url` | `FOLIA_PUBLIC_URL` | `https://betula.app` | the site's address from outside: canonical links, link previews, sitemap |
| `--access-gate` | `FOLIA_ACCESS_GATE` | `off` | closed testing: the whole site asks for one shared password (see below) |
| `--log-format`, `--log-level` | `FOLIA_LOG_FORMAT`, `FOLIA_LOG_LEVEL` | `text`, `info` | `json` in production |

`folia healthcheck` is not the server but its probe (like `radix healthcheck`): it asks the server
that listens on `FOLIA_ADDR` for `/livez` and exits with 0 when it answers. The container image
uses it as `HEALTHCHECK`, because an image built with Nix has no curl or wget.

Endpoints besides the pages: `GET /api/db`, `GET /api/status`, `GET /api/map.json` (the map of the
programs, with the snapshot's ETag), `GET /healthz` (200 while a snapshot is served and Radix was
heard from; for an uptime monitor), `GET /livez` (200 while the process answers; for the container's
healthcheck, which must not restart a server that still serves its last snapshot),
`/assets/app.css`, `/assets/favicon.svg`,
`/assets/og.png`, `/favicon.ico`, `/apple-touch-icon.png`, `/assets/icon-192.png`,
`/assets/icon-512.png`, `/assets/icon-maskable-512.png`, `/manifest.webmanifest`,
`/cards/module/<id>.png`, `/cards/program/<slug>.png`, `/robots.txt`, `/sitemap.xml`, and
`GET`/`POST /access` (the login page of closed testing).

### Closed testing: the access gate (`server/src/access.rs`, 2026-09-21)

For the time in which the site is tested by invited people only (owner: the legal pages come
later, so nothing may be public yet). One switch, one secret:

```bash
FOLIA_ACCESS_GATE=on FOLIA_ACCESS_PASSWORD='…' cargo run -p folia-server
```

- **The switch** is `FOLIA_ACCESS_GATE` (`on`/`off`, also `true`/`false`, `1`/`0`; flag
  `--access-gate`). Off is the default and leaves no trace but `/access` leading on to the site.
  Taking the gate away later is this one value; the secret may stay where it is.
- **The password** is a secret and is found the way Radix finds its key (`docs/operations.md`
  §3), first hit wins: the file named by `FOLIA_ACCESS_PASSWORD_FILE`, a Docker secret
  `/run/secrets/folia-access-password` (or `folia_access_password`), a systemd credential
  `folia-access-password`, the variable `FOLIA_ACCESS_PASSWORD`. Never a flag. **A gate that is on
  and finds no password (or an unreadable file) keeps the server from starting**
  (`server.start_failed`); it never opens the site by accident. Use a long random password: it is
  the only thing between the site and the world.
- **What a visitor sees:** opening any page leads to `/access?next=<page>` (302), a login page in
  the look of the app that works without JavaScript; the right password leads back to the page
  (303, only ever to a path of this site). Everything that is not a page (`/api/db`, `/pkg/*`,
  cards, the sitemap …) answers `401` without the password.
- **The cookie** `betula_access` (`HttpOnly`, `SameSite=Lax`, `Secure` when the request came
  through the proxy as HTTPS, 90 days) holds the end of the visit, signed with a key made from
  the password (HMAC-SHA256). The server keeps no sessions and nothing about visitors: a
  restart or a new container keeps everybody in, **a new password ends every visit at once**.
- **Open without the password** is only what the login page, a home screen and a supervisor
  need: `/access`, the stylesheet, the font, the icons, `/manifest.webmanifest` (browsers fetch it
  without cookies), `/healthz` (uptime monitor), `/livez` (the container's healthcheck) and
  `/robots.txt`, which says `Disallow: /` while the gate is on. The login page is `noindex` and
  `no-store`.
- **Behind the gate** the site is what it was, the page cache included (the gate lies around it);
  only `Cache-Control: public` becomes `private`, so no cache between server and browser keeps a
  page for somebody else. Link previews of messengers show nothing while the gate is on: their
  fetchers have no password.
- **Guessing:** ten wrong passwords within a minute close the form for the rest of that minute
  (`429`, for the right password too; who is in stays in). The count is for the whole site, not
  per address, because the server keeps nothing about visitors; it bounds guessing to about
  14 000 tries a day, at the price that someone guessing can keep others from logging in.

In a container nothing else is needed. On the server the switch is `FOLIA_ACCESS_GATE` in the
instance's file (`deploy/stacks/canary.env`), and `deploy/stacks/betula.yml` has the rest:

```yaml
services:
  folia:
    environment:
      FOLIA_ACCESS_GATE: "on"          # "off" opens the site; the secret may stay
    secrets: [folia-access-password]   # found at /run/secrets/folia-access-password
secrets:
  folia-access-password:
    external: true                     # swarm: … | docker secret create folia-access-password -
    # file: ./access-password.txt      # compose without swarm: a git-ignored file instead
```

### The container (`flake.nix`, 2026-09-21)

```bash
nix build .#folia-image            # result → docker image tarball "betula-folia:latest"
docker load < result
docker run -d --name folia -p 8080:8080 -v folia-data:/data -e FOLIA_SNAPSHOT_URL=http://<radix>:8090/snapshot/catalog.db betula-folia:latest
```

`.#folia` is the server (`cargoLock`: no hash to keep up to date), `.#folia-client` the browser
app as `scripts/build-client.sh` builds it, with a wasm-bindgen CLI of exactly the version in
`Cargo.lock` (nixpkgs rarely has that one; after a change of the version the two hashes in
`flake.nix` have to be renewed, the comment there says how). The image holds `/bin/folia`, the
browser app under `/site` and nothing else; it runs as user 10001, to whom `/data` belongs (a
fresh named volume takes the owner over), with a read-only root file system if asked to. Defaults
inside: `FOLIA_ADDR=0.0.0.0:8080`, `FOLIA_DATA_DIR=/data`, `FOLIA_SITE_ROOT=/site`,
`FOLIA_SNAPSHOT_URL=http://radix:8090/snapshot/catalog.db`, `FOLIA_LOG_FORMAT=json`, and
`HEALTHCHECK folia healthcheck` (every 2 s while starting, then every 30 s). The tests do not run
in the Nix build (they need a snapshot). Shipping both images to the server and deploying an
instance is `deploy/ship.sh` (`deploy/README.md` §4).

### Log events (same rules as `docs/operations.md` §2: ERROR = a human has to act)

| Level | `event` | Meaning |
|---|---|---|
| INFO | `server.listening`, `server.shutdown` | lifecycle |
| INFO | `snapshot.sync_started`, `snapshot.restored`, `snapshot.downloaded`, `snapshot.activated`, `snapshot.sync_recovered` | snapshot lifecycle (`etag`, `bytes`, `generation`) |
| INFO | `snapshot.map_built` | the map of the programs was laid out for a snapshot (`programs`, `links`, `ms`) |
| WARN | `snapshot.map_failed` | it could not be; the landing page goes without the map |
| DEBUG | `snapshot.unchanged` | Radix answered 304 |
| INFO | `http.request` | access log: `method`, `path`, `status`, `ms`, `cache` (`hit`/`miss`/`-`) |
| DEBUG | `http.request` with `path=/livez` | the container's own probe, twice a minute |
| INFO | `access.gate_on` | closed testing is on (`source`: where the password was found, never the password) |
| INFO | `access.granted` | the access password was entered |
| WARN | `access.denied` | a wrong access password (`failures` in this minute, `closed` when the form closed; at most ten lines a minute) |
| DEBUG | `card.drawn` | a link-preview card was drawn (`key`, `bytes`, `ms`) |
| WARN | `card.busy` | every drawing place was taken, previews got the standard picture (`count`; at most one line a minute). Often: more places or a larger `--card-cache-mb` |
| WARN | `snapshot.fetch_failed` | Radix unreachable or not ready; retried with backoff; the last snapshot stays active |
| WARN | `snapshot.restore_failed`, `snapshot.compress_failed` | stored snapshot unusable / served uncompressed |
| ERROR | `snapshot.rejected` | a download is not a usable catalog; the previous snapshot stays active |
| ERROR | `snapshot.stale` | no answer from Radix for longer than the limit |
| ERROR | `http.request` with `status >= 500`, `render.failed`, `snapshot.unreadable` | a request failed |
| ERROR | `card.failed` | a card's text could not be read or the card could not be drawn; the preview got the standard picture |
| ERROR | `server.start_failed`, `server.failed` | the server cannot run |

## 4. Checks

```bash
cargo test
```

needs a snapshot (`snapshot/current.json` or `FOLIA_TEST_SNAPSHOT`) and fails without one:

- `catalog`: every filter against direct SQL (exclusions included), exact totals and paging, the
  pinned numbers, enum labels from the CHECK constraints, every query and page loader against
  real data, the URL codec, the ranking of the pickers (`fuzzy`).
- `server`: a fake Radix over HTTP: not ready → 503; download, check, gzip, activate; 304 →
  no download; pages render, cache (`hit`/`miss`), revalidate; equal filters share a cache key;
  404 is never cached; `/api/db` with Radix's ETag, gzip and 304; a broken export is
  rejected and the old snapshot stays; a new one invalidates pages; restart without Radix;
  one description and one absolute canonical address per page, `noindex` on a filtered list,
  the sitemap, the map of the programs as laid out with the snapshot. Closed testing (needs no
  snapshot): pages lead to the login page, everything else answers 401, what stays open, the
  way back as text and never to another host, wrong and right password, the cookie and its
  `Secure` behind the proxy, forged cookies, `private` instead of `public`, the closed form
  after ten wrong passwords, the signature and the end of a visit, where the password is found.

```bash
cargo clippy --all-targets
cargo clippy -p folia-client --target wasm32-unknown-unknown
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

```bash
cd e2e && node module.mjs
```

drives a module in its two sizes. Desktop: preview → page with the sidebar exactly where the
filter panel was, the same order of sections in both, jumps without history entries, one width
for sidebar and filter panel, Esc back to the list with the row in view. Phone: a tap opens the
page directly, the page starts with times and facts, back returns to the tapped row deep in the
endless list, a shared preview link becomes the page.

```bash
cd e2e && node bookmarks.mjs
```

drives „Merken". Desktop: the button at the end of a row (in its place, the column labels still
over their columns, a click neither navigates nor renders the list again), `M` on the row the
keyboard is on, in the preview and on the module's page, one state wherever a module shows, the
switch keeping its width, the number at the rail, a reload keeping the marks; the list of marked
modules (order of marking, numbers and credits, sorting by column and in the sidebar, the halves
of the year with their counts, the floating preview, a mark taken away staying on the page,
copying the list, the link for another device and what the other device does with it: asking
first, counting what is new, taking the ids out of the address without a history entry;
emptying with the question and „Rückgängig"); „Gemerkt" as a filter of the catalog (only the
marked ones and all but them, the rest of the filter kept, the tag above the list, a mark made
while it is on, the two empty states with and without the app); the rail's item as a tab;
„Zurück" and Esc from a module opened from the marked modules; a mark made in another tab
arriving; a module the snapshot does not know; garbage in the storage. Privacy: no request of the
whole session carries a mark, none leaves the site, and server HTML shows nothing marked. Phone:
a 44 px target, marking by touch, the bottom bar's count, a tap opening the module's page and
„Zurück" returning. Without the app: nothing of it shows without JavaScript, `/bookmarks`
explains itself and is `noindex`; with JavaScript but before the takeover the buttons are
invisible and their room is kept (the heading is exactly as tall as once the app runs).

```bash
cd e2e && node programs.mjs
```

drives the programs area: „Zurück" and Esc on a program's page leading back to the program in the
overview; the rail's items as tabs (the open program and the filtered catalog are still there
after a change of areas, „Zurück" after such a change leads up and not across); the sidebar in
the same place on catalog, overview, program page and landing page; the overview (faculties in order, every program exactly once, the counts of header,
sections and links agree, no two links of a subject read the same, nothing cut off in the
sidebar); filters as links that keep focus and sidebar, the search keeping the filters; a jump to
a faculty without a history entry; the views of a program in the sidebar; on a phone the filters
in a sheet; and the filter links without JavaScript. On a program's page: the numbers of the head
(and that none of them leaves its panel), one study plan per study direction (no module twice, the
first semester at 30 LP, semester columns of equal width), switching the direction through the
URL, a module opening beside the page (its row marked, the panel not lying over the table) and
closing again, „Vollbild" and back to the program without a new history entry, Esc leaving the
program instead of reopening the module, an area beside the page with its modules (and a module
picked out of it coming back to the area), a requirement of the plan with its numbers and its
ways on, the catalog's tab unchanged by a module seen in full screen out of a program, matrix and
list with the choice remembered in this browser only, the areas as groups of rows
with the sidebar leading to each of them without a history entry, all modules one line high with
their area, and on a phone the matrix scrolling inside its panel while the page does not.

```bash
cd e2e && GATE_PASSWORD=… node gate.mjs http://127.0.0.1:8086
```

drives closed testing against a folia started with `--access-gate` and the same password: without
it nothing answers but the login page and what it needs; a page leads to the login page; a wrong
password stays there, says so and keeps the way back; the right one leads to the wanted page with
the cookie as it should be, and behind the gate the browser app takes over without a single
refused request (the manifest included); the login page has one left edge, stands in the middle
and scrolls nowhere sideways; on a phone without JavaScript the same form logs in.

```bash
cd e2e && node home.mjs
```

drives the landing page: the map is part of the server's HTML (dots are links); the app draws it
from what `boot.js` handed over; a pointer over a dot shows its relatives; a click opens the
program without loading a page; the head has one description and one canonical address and both
follow a navigation; a phone gets the tall sheet and nothing scrolls sideways.

## 5. Not done yet

- PWA: manifest, service worker (offline start), update prompt. User data beyond „Merken": passed
  modules with the prerequisite check, „mein Studiengang". `wasm-opt` for the bundle.
- Phase 3: design system, weekly calendar, filter bottom sheet, search
  with context ranking (own concept, see `docs/frontend-phase0.md`).
- Phase 4: CSP, CI. (Done 2026-09-21: Nix package and container, the Swarm stack
  `deploy/stacks/betula.yml` with its instances, `deploy/ship.sh`.) `wasm-opt` is not part of
  the Nix build either.

### Ideas noted for later (owner: „schreib dir die mal auf", 2026-09-20)

- **Leafing through the filtered list from a module's page:** previous and next module with
  „12 von 35" in the sidebar, and keys for it, without going back to the list. Needs the list the
  visitor came from (its canonical query, kept outside the URL) and one query for the ids in order.
- **The neighbourhood of a module** in the sidebar: what it builds on and what builds on it
  (`v_module_prerequisite` in both directions), as a small map instead of two lists.
- **A stated faculty per program** instead of the derived one: the BTU's pages of the study
  programmes name it. That is a new source for Radix (an additive column, a crawl the
  owner has to approve), not a frontend change.

### „Merken": what is left (2026-09-20)

- **Marks in the tables of a program's page** (plan, areas, all modules): `bookmarks::MarkButton`
  with `MarkLook::Row` next to a row's link is made for it. Left out on purpose while the page
  itself is being worked on (owner, 2026-09-20); the panel beside the page already has the switch,
  because it is the catalog's module preview.
- **What follows the marks** (R20 applies): passed modules with the prerequisite check, the own
  program, the semester planner. A note per marked module would fit the same store.
