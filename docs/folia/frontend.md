# Folia, the web tier: architecture, rules, how to run it

> Betula has three parts named after the birch: **Cortex** (the bark: the cache between Betula and
> the internet, `docs/cortex/cortex.md`), **Radix** (the root: the Go collector, `docs/radix/operations.md`)
> and **Folia** (the leaves: this web tier, the Cargo workspace `folia/` with its crates in
> `folia/crates/<crate>`, each `folia-<crate>`, in the layers of `folia/layers.toml`: the domain —
> `folia-locale`, `folia-pack`, `folia-model`, `folia-calendar`, `folia-search`, `folia-routes`,
> `folia-query`, `folia-timetable`, `folia-plans`, `folia-semantic`, `folia-pages` — the data
> (`folia-data`), the UI — `folia-design`, `folia-stores`, `folia-shell`, `folia-widgets` and the
> features `folia-home`, `folia-catalog`, `folia-programs`, `folia-bookmarks`, `folia-planner` — and
> `folia-worker`, `folia-app`, `folia-client`, `folia-server` with the binary `folia`;
> docs/folia/folia-refactor.md §7 has the map).

> State: 2026-10-03. The site's pages are server-rendered and minimal; with JavaScript the
> browser app takes the page over and nothing is loaded again. Its questions go to the data
> worker, which keeps the catalog (sql.js, IndexedDB) beside the page's thread and shows a newer
> snapshot in place; the app starts without a network (a service worker keeps its shell).
> Decisions and their evidence: `docs/history/frontend-phase0.md`.

## 1. Overview

```
Radix ──HTTP──▶ Folia ──HTML (cached per snapshot)──▶ browser
 /snapshot/catalog.db        ──/api/db (brotli, ETag)─▶ browser: local SQLite (phase 2)
```

| Crate | Role |
|---|---|
| domain | The data contract and what is computed from it, without I/O (callers hand in a `Database`), native and WASM: `model` (rows, labels, `Database`), `calendar`, `search`, `routes` (every address, `url.rs`), `query` (the SQL, R11), `timetable`, `plans`, `pages` (one loader per page, and the questions of the app's pages, `ask.rs`). |
| `pack/` | Values as codes that travel in a link (`pack::to_code`, `pack::from_code`): serde's data model as bits (fields by their place, numbers in as many bits as their size needs, `pack::set` and `pack::list` for ids), written in the 66 unreserved characters of an address (`A–Z a–z 0–9 - . _ ~`), the last two of them check the rest. No I/O, no dependency but serde; the format is frozen (`folia/crates/pack/src/lib.rs`). |
| `data/` | `DataClient`: where every page asks its questions. On the server it answers them on its thread from the active snapshot; in the browser it sends them to the data worker. |
| `design/`, `stores/`, `shell/`, `widgets/` | The UI below the pages (Leptos, feature `ssr` for the server, `csr` for the browser): building blocks and the languages, what a visitor keeps, the shell (chrome, frame, a step between pages, the ground), what several pages show. |
| `home/`, `catalog/`, `programs/`, `bookmarks/`, `planner/` | The features: each its pages and their texts. None uses another. |
| `app/` | The composition: the document and `App` with its routes. |
| `client/` | The browser app (WASM): `app` with feature `csr`, asking the data worker. Not a default workspace member (its `csr` would be unified with the server's `ssr`); built by `folia/scripts/build-client.sh` into `site/pkg`. |
| `worker/` | The data worker (WASM, no Leptos) and its script: the catalog in sql.js, every question of the app's pages answered beside the page's thread; built with the client. |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets, the drawn cards. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `spa.mjs` (the browser app: takeover, no page loads, preview, filters, the virtual list, search), `search.mjs` (the search of the catalog: typos, relevance, what the filters leave out, „Ähnliche Module“), `typing.mjs` (typing in the search: its questions in the data worker, how long the keys wait), `filters.mjs`, `module.mjs`, `programs.mjs`, `bookmarks.mjs`, `phone.mjs` (the phone layout: the sheet, the pickers, the list), `swipe.mjs` (a row of the catalog and of the Merkliste swiped on a phone: „Merken", „Einplanen"), `studyplan-phone.mjs` (the Stundenplan's week on a phone), `study.mjs` („Mein Studium": the Studium tab, the rule, ticking off and moving, the timetable, the phone), `home.mjs`, `snappy.mjs` (a click answering in the next frame, skeletons), `top.mjs` („Nach oben"), `languages.mjs` (the app in English, `docs/folia/i18n.md`), `schema.mjs` (a local copy of the catalog of an older schema, with and without a network), `worker.mjs` (the data worker: no catalog on the page's thread, no blank moment at the takeover, one download for two tabs, a newer snapshot shown in place), `smoke-walk.js` + `run.mjs` (long program walk), `shot.mjs` (review screenshots). All use an installed Edge through `playwright-core`; without one, `node --import ./chromium.mjs <check>.mjs` runs a check in Playwright's Chromium or in the browser `SMOKE_BROWSER_PATH` names. |

### Routes (`folia/crates/routes/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `area`, `kind`, `lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `events`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `marked`, `prereqs`, `sort`, `desc`, `page`. What can be wanted can also be excluded: `not-kind`, `not-lecturer`, `not-turnus`, `not-form`, `not-exam`, `not-campus`, `not-lang` (`exam=written&not-exam=presentation`: a written exam and no presentation). `area=<id>[,<id>…]` are areas of the selected program's module tree („Wahlpflichtmodule Praktische Informatik"): the modules the tree places in any of them or below one (several come from a row of the plan that means several areas, opened from the program's page; the picker then says „5 Bereiche", one tag per area above the list) |
| `/catalog?…&open=<id>` | In the app: the same list with this module previewed next to it; the preview has a „Vollbild" link to the module's page. On a phone there is no preview: a tap on a row opens the module's page, and the app turns a shared `open` link into it. The server's page (crawlers, no JavaScript) ignores `open`: it renders the plain list, every row leading to the module's page (owner decision 2026-09-21: the server's HTML is for crawlers, the app for people, and no query parameter changes the server's layout) |
| `/catalog/module/<id>` | The module's own page: a sidebar as wide as the filter panel (sections of the page, actions), the module on the rest of the screen |
| `/programs?q=…&level=…&form=…&plan=1` | Program overview (current PO versions) by faculty (`ProgramsUrl`): the search of the top bar, degree (`bachelor`, `master`, `teaching`, `doctoral`, `other`), form of study (`dual`, `double`, `flexible`), only with a validated study plan |
| `/study[?open=<id>][&full=1]` | „Mein Studium“ (`StudyUrl`, 2026-10-04): the visitor's study semester by semester, the first page of the Studium tab in the app (see „Mein Studium“ below). What is passed and planned lives in the browser; the address says only which module stands beside the page, and whether it fills it. The server renders one stand-in for everybody, `noindex`, not in the sitemap |
| `/programs/<slug>/plan\|areas\|my-plan[?variant=<n>][&area=<id>][&req=<n>][&open=<id>][&full=1]` | Program page (`ProgramUrl`); its views are switched in the sidebar: the Regelstudienplan (`plan`) and „Wahlpflicht & Bereiche“ (`areas`). „Mein Plan“ (`my-plan`, the visitor's, `noindex` and not in the sitemap, `ProgramTab::indexed`) took the place of „Alle Module“ on 2026-09-25 (the program's modules are its catalog, `/catalog?program=<slug>`, and `…/modules` is a 404) and became „Mein Studium“ on 2026-10-04: the views no longer list it, the app goes on from its address to `/study` in the same history entry, and the server's page there says so. Where a program has several study plans (one per study direction), `variant` says which one is shown; `area` is the area of „Wahlpflicht & Bereiche“ shown beside the page, `req` a row of the plan that names no module, `open` the module — they stand in the address (a shared link, the history) and the app renders them; the server's page ignores all but `variant` (it lays nothing beside itself: its module links lead to the module's page, its area links to the catalog narrowed down to the area, a row without a module is text), so they are no part of its cache key and its canonical address is the plain one. The plan of each further study direction is a page of its own (2026-09-26): `?variant=<n>` is its canonical address, listed in the sitemap, with the direction in its title; the first is the plain address, and a number past the last plan names the last. A module opened out of an area keeps it, so closing the module returns to it. `full=1` shows the module of `open` in full: the module's own page, in place, so that „Vollbild" stays in the programs area (its tab, its history, its „Zurück"); the canonical address of that view is the module's page. On a phone whatever is picked — the module, the area, the row of the plan — is the page (`open` alone shows the module in full there) |
| `/bookmarks?turnus=…&sort=…&desc=1&open=<id>[&full=1]` | „Merkliste": the modules the visitor has marked (`BookmarksUrl`). The URL says how the list is shown (half of the year, order, the previewed module), never what is on it: the marks live in the browser. `full=1` shows the module of `open` in full, in the list's place, as `full=1` does on a program's page (a local view, `folia/crates/widgets/src/local.rs`): „Vollbild" stays among the marked modules (their tab, their history, their „Zurück"); on a phone `open` alone does. The server renders an explanation, the same for everybody, `noindex` |
| `/studyplan?sem=…&view=…&open=<id>&row=<key>&import=…&variant=<n>[&share=<code>]` | The Stundenplan (`StudyplanUrl`): how the plan is shown, never what is in it (R20), with one exception: `share`, a semester of a plan handed on by a link (`folia_calendar::share`, owner 2026-09-26), which the page offers to take over. The server renders an explanation, `noindex`, the same for everybody; for a `share` code a page of its own, whose tags and picture name the plan's modules (a link preview runs no JavaScript) |
| `/impressum`, `/datenschutz` | The legal pages (`folia/crates/home/src/legal.rs`): the Impressum and the Datenschutzerklärung, final since 2026-09-25 (placeholders from 2026-09-21). Linked from the ground at the end of every page („The birch"; § 5 DDG: reachable at all times). The privacy notice says what the software does — the edge's access log and its retention, Folia's log, what stays in the browser, the calendar feed, the gate's cookie, the lecturers' names (Art. 14 DSGVO) — and `legal.rs` names the source of each part: a change there is a change of the text. `legal::PLACEHOLDER` stays the switch `deploy/ship.sh` reads: true again, the pages are `noindex` and no instance open to everybody (`FOLIA_ACCESS_GATE` not `on`) ships |

The catalog parameters are tolerant (repeated or comma-joined values, empty inputs of a plain
HTML form, nonsense ignored) and have one canonical spelling, which is also the cache key. A
value that is both wanted and excluded counts as wanted. An exclusion removes only what the data
states: a module whose campus or turnus is unknown stays in the list (R12). Lecturers: the
wanted ones are alternatives, the unwanted ones are all left out: (Meer or Köhler) and not
(Lambers or Hofstedt).

**Every route in every language** (2026-09-27, `docs/folia/i18n.md`): the addresses above are the
German pages; the same page in English is the same address under `/en` (`/en/catalog?…`, the start
page `/en`), and so are the cards, the manifest and the calendar feed (`/en/cards/…`,
`/en/manifest.webmanifest`, `/en/calendar/<code>.ics`). `/de/…` leads to the plain address. The
paths inside the app never carry the prefix (`folia_locale::Locale::path`/`split`, R23).

### Look and interaction (since 2026-09-19, owner-approved direction)

- **Name and logo** (owner decision 2026-09-20, domain `betula.app`): the product is **Betula**
  (the birch; B-T-U stands in the name). Running text says „Betula", only the wordmark stresses
  the three letters: BᴇTUʟᴀ in Inter 800 with E, L and A as small capitals and the E under the bar
  of the T (`ui::Wordmark`, `.wordmark`; the spacing is measured, not guessed). The mark is birch
  bark that also reads as the rows of a list (`ui::Mark`, 32 px grid; favicon on the same grid).
  The rail carries the mark, the top bar of the start page the wordmark with „Modulkatalog ·
  inoffiziell", so the two read as one logo; on a phone the start page carries both itself.
  „Inoffiziell" always stays with the name, and nothing borrows the university's colours or mark.
  Sizes, grids and numbers: `folia/design/logo/logo.html`.
- **The icon of the installed app is a birch leaf** (owner, 2026-09-26): the mark cut to a circle
  „sieht bei einer Kugel mit Material You wirklich nach nichts aus" (its bars run out to the edges
  of its square, and a launcher's circle leaves a white disc with stubs). The leaf is white on the
  green of the birch leaf and carries the mark's four bars as the marks of bark, entering from its
  edges as they enter the square: close enough to the mark that it is recognised, and whole in any
  shape a launcher cuts and in one colour for Android's themed icons. It is the icon of the home
  screen (the manifest's icons, `apple-touch-icon.png`, the splash screens) and, since 2026-10-01,
  of the site's results in Google Search (owner: the app icon there); the site keeps the mark in
  the tab, the rail and the link previews („ich mag das aktuelle Icon eigentlich sehr").
  `folia/design/logo/app-icon.mjs`, described in `logo.html`.
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
- **The filter panel** (`folia/crates/catalog/src/catalog.rs`, JavaScript first, owner decision 2026-09-20):
  - It is rendered once and then follows the URL (`Filters` takes memos, not values), so a change
    keeps the focus, the scroll position and what is folded open. Its width is dragged at the
    handle in the gap between the two boxes, inside 232–440 px, and remembered in `localStorage`.
  - **Toggles are links** to the list with the next state of their value: off → with → without →
    off („keine Vorträge" is the second click). The small box on their left shows the state, so
    they read as switches; the toggles of a row share its whole width. As links they need no
    handler (the router turns the click into a navigation), work without JavaScript, carry
    `rel="nofollow"` and `data-noscroll`, and the space bar flips them like a checkbox.
    „One of a few" (list, plan semester, duration, years) is a segmented row of the same links.
  - **„Passt in meinen Stundenplan"** (the finder; until 2026-09-26 „Passt in meinen Plan", which
    read as the Regelstudienplan, and so the line under a module's week: „Passt in deinen
    Stundenplan (WiSe 2026/27)") is a toggle with more under it: switched on, the classes it
    compares show below it (Vorlesungen, Übungen, Prüfungen, auch ohne Termine), and a chevron at
    its end says so, like the head of an accordion: pointing right while it is off, down while
    they show (`aria-expanded`). It names no semester (owner, 2026-09-26: „nur mit Passt in
    meinen Stundenplan ohne das semester"): which one it checks, its tag above the list says once
    it is on. Where the panel is too narrow for the label on one line (at its narrowest), the label
    takes a second one instead of being cut off. Switched on again it compares what it compared the
    last time (owner, 2026-09-26: „nicht immer resettet"), from the Stundenplan's „+ Modul" and
    „Modul finden" as well: the browser keeps the choice as the address writes it
    (`localStorage` `betula.finder`, e.g. `fits-skip=exam&fits-undated=1`; nothing while it
    compares everything), a view setting like the width of the panel (R13), read back like an
    address (`pages::catalog::finder_on`).
  - **Pickers** (`folia/crates/design/src/combobox.rs`: program, area, lecturers, department) have a search that
    forgives typos and knows initials and abbreviations (`folia_search::fuzzy`: „infomatik bsc"), arrow
    keys, Enter, Esc. Their popup is fixed to the window, so no panel clips it; on a phone it
    opens in place, under its button and across the panel the picker stands in (owner,
    2026-09-29: „Komboboxen auf dem Handy sollen immer die volle Breite einnehmen … von ganz
    links bis nach ganz rechts. Kein Überstand"): 16 px in from either edge of the panel, however
    far in its button stands (the way in's, beside the marks), and never narrower than the
    button; no name it lists widens it (a long one pushed the popup and the button out of the
    panel). There nothing that moves the window closes it: the on-screen keyboard
    that opens for the search field shrinks the window and scrolls the field into view, which
    used to close the popup the moment it opened. The module comment of the component lists what
    keeps it predictable (it is a rewrite: the picker of the old frontend lost its mark to the
    mouse, closed the preview with Esc and knew its selection by label). Without the app the same
    places hold a plain `select` or text field inside a GET form, and hidden inputs carry what
    the links have set.
  - **The areas of a program** („Bereich", with a program selected, curriculum only): of the
    areas of its module tree (`pages::CatalogArea`, from `v_program_module_area`) only those a
    student chooses from (owner, 2026-09-21: „eigentlich will man auch nur nach den Wahlpflicht­
    modulen filtern, weil die anderen ja eh fix sind" — and not every node of the tree, no
    „Grundstudium", „Fachstudium", „Komplex Informatik"). An area is fixed when every module the
    tree places directly in it is known to be compulsory, the thesis or the internship, by what
    the program's sources settle on for the module (`v_program_module.kind`: the plan, the module
    page, the tree's own label); a module nobody says anything about counts as a choice (R12: not
    known to be fixed is not fixed). Each entry is the area's name and how many modules it holds,
    nothing else (the path of the tree beside the name had pushed the names into
    „Wahlpflichtmod…"), the name without a leading „Wahlpflichtmodule" (`CatalogArea::name`: every
    area offered is one to choose from), in **few, stable sections** (owner, 2026-09-21: never the
    same heading twice, no node that only structures the tree as a heading; the plain `select`
    uses `optgroup`). The rule, taken from the 179 real trees (`pages::catalog_areas`, numbers in
    „The area picker and the plan's rows on the real data" below): the heading of an area is the
    **highest node above it that names a field** — not a phase („Grundstudium", „Fachstudium",
    „Hauptstudium"), not an account („Gesamtkonto …", „Total Account - …", „Module an der …",
    „Modules at …") and not a label that says nothing but a kind („Pflichtmodule",
    „Wahlpflichtmodule (KT)", „Compulsory Elective and Optional Modules"; `pages::is_structural`),
    shown without a leading „Komplex" („Komplex Nebenfach" → „Nebenfach"; the trees say
    „Nebenfach", „Anwendungen" or „Anwendungsbereiche", never „Anwendungsfach" — that word is the
    plan's). One level of headings, however deep an area lies; a section of more than 12 areas
    whose fields below hold them splits into those fields (Wirtschaftsingenieurwesen dual:
    „Ingenieurwissenschaftlicher Schwerpunkt" → Produktionstechnik, Umwelttechnik …). A heading over
    a single area is none, headings that read the same are one section, and the areas without a
    heading — the program's own — come first, then the sections in the order of the tree. An area
    whose label is only a kind takes the name of its field (Architektur: „Entwerfen", not five
    times „Wahlpflichtmodule"), unless other areas stand under that field, then it keeps its label
    there (Elektrotechnik M.Sc.: „Studienrichtung Kommunikationstechnik (KT)": „Wahlpflichtmodule
    (KT)", „Zweite Fremdsprache"). Two areas that would read the same where they stand are told
    apart by a node above them („Mathematik", „Mathematik (Anwendungen)"). For Informatik B.Sc.
    that reads: Proseminar oder Praktikum, Grundlagen der Informatik, Praktische Informatik,
    Angewandte und Technische Informatik, Seminar oder Praktikum aus der Informatik — then
    „Nebenfach": Praktische Mathematik, Mathematik, Physik, Maschinenbau / Elektrotechnik,
    Wirtschaftswissenschaften, Bauingenieurwesen. **Praktische Mathematik stands in the
    Nebenfach** although the owner counted it among the own electives: the tree places it in the
    Komplex Nebenfach, and nothing in the data tells it from the subjects beside it but its
    label's „Wahlpflichtmodule", which Wirtschaftsmathematik's alternatives carry too (open
    question for the owner, see below). The nodes above an area come from the tree itself
    (`queries::program_area_tree`, every node of `program_area` with its `parent_id`), never from
    splitting the path: a label may read „Maschinenbau / Elektrotechnik". The picker keeps each
    section together while one types, too (`combobox::grouped`): the section of the best match
    comes first, its heading with it; the areas without a heading are one section there as well,
    set off by a line where they follow another. The full label and the path
    still find an area when typed. An area filters to the modules the tree places in it or in
    an area below it; that is how the elective modules of a program are listed, whatever the plan
    says about their semester.
  - **On a phone the panel is a sheet** from below, opened by the list's „Filter" button, and
    closed the way a sheet is expected to close: swiped down (owner, 2026-09-22: „braucht
    unbedingt eine Geste"; it follows the finger and is let go when pulled far or flicked, else
    it slides back), with a tap on the page behind it, with its buttons, or with Esc
    (`enhance.js`). A touch decides with its first move: downwards where nothing inside the sheet
    is scrolled down (the head, the row of buttons, the top of its list) it drags the sheet,
    anything else inside scrolls the sheet's list. **The page behind an open sheet stands still**
    (owner: scrolling it along felt „richtig clunky"): `html.sheet-open` takes the page's
    scrolling away, the sheet's list keeps its scroll to itself (`overscroll-behavior`), and a
    touch on the dimmed page moves nothing.
  - **What is picked in the sheet is a draft** (owner, 2026-09-22: every tap rebuilt the list
    behind the sheet, 250–540 ms of a phone's time with the CPU throttled 4×; now the tap costs
    the panel and one count). The panel shows the draft, its button counts the modules it holds
    (`pages::catalog_summary`, which says what `pages::catalog` says about the same filter, the
    semester of a plan included), and the list and the address follow once, when the sheet
    closes, whichever way it closes. So a sheet adds one history entry, not one per tap. The
    links of the panel stay links (they work without JavaScript and on the desktop, where every
    change still applies at once): on a phone their address becomes the draft. This is the one
    place where a filter lives outside the URL, for as long as the sheet is open.
  - **Back closes the sheet**, as in an app: the open sheet of the browser app is a step of its
    own in the history (`data-draft` on the panel; `enhance.js` pushes a same-address entry when
    the sheet opens and takes it back before the list follows, so the list's own entry takes its
    place). The programs overview's sheet applies every tap at once and has no such step.
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
- **A module's texts are set as text, not as lines** (owner, 2026-10-01: „support für Blocksatz",
  „Paragraphen sollen als solche erkennbar sein. Listen sollen erkannt werden und dem entsprechend
  formatiert werden"). Learning outcomes, contents, assessment, remarks and the prerequisites in
  the page's words are Markdown since schema 10 (docs/radix/schema-v2.md §3, „Module texts"), read by
  `folia_model::text` and set by `ui::Prose`: paragraphs apart by a gap, lists with their markers in
  the margin (a list labelled „(1)", „a)", „IV." with its labels there), strong and emphasized
  words, the line breaks the text keeps. The text is justified (Blocksatz) and hyphenated by the
  rules of its own language — the `lang` of the module's page (`v_module.page_lang`), so a German
  text on the English page breaks as German — never into syllables of fewer than three letters;
  a column too narrow for it is set ragged (Blocksatz, under „Look and interaction"). Nothing but
  text reaches the page: `folia_model::text` reads the CommonMark Radix writes — paragraphs, lists,
  strong and emphasized text, line breaks, escapes — by CommonMark's rules and nothing else, so a
  „#", a „<b>" or a „[link](…)" a text holds anyway stands as it is. The reader is the catalog's
  own: pulldown-cmark made the browser's app 60 KB larger (brotli), the reader and the views
  10 KB. The page's description and its structured data take the text as one line
  (`text::plain`, the items of a list apart by „·").
- **„Merken" (owner decision 2026-09-20: in the browser app only, and no data of a visitor on
  the server; `folia/crates/stores/src/bookmarks.rs`, `folia/crates/bookmarks/src/bookmarks.rs`).** A visitor marks modules to
  come back to. The marks live in this browser's `localStorage` (`betula.bookmarks.v1`, a line
  per module: id and time of marking, the newest first) and nowhere else: not in a URL, not in
  server HTML, in no request (R9, R13, R20). Another tab of the same browser follows through the
  `storage` event; another device has a list of its own.
  - **Where a module is marked:** at the end of its row in a list (a button *next to* the row's
    link, in a wrapper, because a button inside a link is neither; the row keeps 48 px free for
    it, quiet until the row is pointed at or the module is marked, always shown where nothing
    hovers); in the line of the module's badges (credits, turnus, language) at its right end
    (owner, 2026-09-20), in the preview and on its page, as a switch „Merken" / „Gemerkt" with
    its shortcut, beside „Einplanen" — where the line has no room for the two side by side, they
    stand one over the other at its end and the badges wrap in the rest of it (owner,
    2026-09-26; on a phone the pair has a line of its own under the badges and fills it, half
    each, owner 2026-09-27); among the actions of the module
    page's sidebar, which stays in view while the page scrolls; and on a phone by swiping the
    module's row in the catalog or in the Merkliste to the left (below). All of them show one
    state.
    **`M`** marks what the visitor is at: the row the keyboard is on, else the module that is
    open. A marked module is neutral and strong (filled, inverted), like a chosen chip; the
    accent stays with primary actions.
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
    module's whole page in the list's place. The sidebar holds what belongs to the list as a
    whole: its numbers (modules, credits), the halves of the year as a row of links with their
    counts („Alle", „Winter", „Sommer": what the catalog's turnus filter would find among the
    marked), the order (of marking, the newest first; by title, credits, teaching events; the
    column headers sort as in the catalog), and the actions: copy the list as text, and empty it,
    which asks first and can be taken back („Rückgängig").
  - **A mark taken away on that page stays on the page,** dimmed, until the page is left: a slip
    is one click to undo, and the list does not jump under the pointer. Marking changes numbers,
    never the list: no query runs and the rows stay the same elements (each button reads the
    marks through a memo of its own, R5). So does a row swiped to the left on a phone („Entfernen
    · von der Merkliste", below): it stays, dimmed, and swiped to the left once more it is marked
    again; while it is swiped its card is whole, or the ground would show through it.
  - A marked module stays on the list when it is no longer offered, and one the snapshot does not
    know (taken out of the BTU's catalog) is named under „Nicht im Modulkatalog", not dropped
    (R12). What is stored is read like anything from outside: ids that cannot be ids are
    dropped, a module counts once, the list ends at 2,000.
  - **A module opened from the marked modules stays among them** (owner, 2026-09-24: „Vollbild"
    used to switch to the catalog's address, so the catalog's tab kept the module open and its
    „Zurück" led back to the marked modules): „Vollbild" of the preview shows the module's whole
    page in the list's place (`&full=1`), and on a phone a tap on a row does (`open` alone), as on
    a program's page (a local view, `folia/crates/widgets/src/local.rs`). The tab „Merkliste" stays the current one
    and remembers the module, „Zurück" and Esc lead to the list — with the module beside it again,
    through the history, on a phone without it — and show the row it was opened from; the
    catalog's tab never hears of it. What is listed stays meanwhile: a mark taken away on the
    module's page leaves the module on the list, dimmed. A module's own page reached from there (a
    successor named on the page) belongs to the marked modules as well: its „Zurück" leads back
    (`Tabs::came_from`), and the catalog's tab still leads to its list.
  - **To another device without a server in between:** „Auf anderes Gerät übertragen" copies a
    link to the list with the marked modules in its *fragment*, as a code (`/bookmarks#m=…`,
    `pack/`): the ids as a set of numbers, in ascending order, each as its distance from the one
    before, in characters an address carries as they are, and two check characters at the end.
    The order of marking does not travel (owner, 2026-09-23: it does not matter); the other device
    marks them all at once. The code begins with the number of its layout, in four bits
    (`pack::to_versioned_code`; owner, 2026-09-25), so a later layout can be read beside it. 20
    marked modules take 35 to 40 characters (as ids one by one, `11101,12204,…`, 124). A character
    typed wrong or two swapped are always noticed, a link cut short almost always; the page then
    says the link is broken, and offers nothing of it. A list too long for a code (thousands of ids
    that are no module numbers, which the catalog does not have) gets no link. Links of the time
    before (ids one by one, codes without a layout) are not read: there was only canary then, and
    the owner wants no code for old links while none is needed (2026-09-25).
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
- **The areas are tabs (owner decision 2026-09-20, R19; `folia/crates/shell/src/tabs.rs`):** the items of the
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
  app every tab is the plain link to its area. **A module opened out of an area that shows its
  modules in place does not become what the catalog remembers** (a program, the marked modules:
  `Area::shows_in_place`; owner, 2026-09-20 and 2026-09-24): its tab keeps leading to the list
  as it was left, and that list does not reveal a module the visitor never picked there. Such a
  module stays in its area anyway (`folia/crates/widgets/src/local.rs`); a module's own page reached from there (a
  successor named on the module's page) leads **back into that area**, not to the catalog: the
  step before answers, and after a reload the programs' own memory does (it was left at a
  program page that names this module in `open`). Not where the page is what the catalog was
  left at: the visitor came back to it (the catalog's tab, Back), and „Zurück" leads up to the
  catalog's list, not across to the area they were in between. Where „Zurück" leads is read
  again on every change of the address, so after closing the module beside a program Esc follows
  the link out of the program instead of walking the history back into the module it has just
  closed. **„Studium" is the visitor's study first** (owner, 2026-10-04): the list of its area is
  „Mein Studium" (`/study`), where the tab leads on the first entry of a session and, inside the
  area (the overview, a program), up to; the program overview is one link away in its sidebar.
  „Zurück" on a program's page leads to the list it came from, „Mein Studium" or the overview as it
  was left, and to the overview where nothing is remembered (`Area::back_root`, which the server's
  page links). Without the app the tab is the overview: there is nothing to plan without it.
- **A swipe along the phone's bottom bar goes to the tab beside the current one** (owner,
  2026-09-30, and the other way round the next day, the first direction being „invertiert zu dem,
  was man intuitiv erwartet": „die ganze Leiste zu bewegen und den selector stehen zu lassen und
  erst wenn man los lässt geht das dann wieder zur original Location zurück"; of the prototype's
  ways „nur tabs + ein Element", `folia/design/tabbar/swipe.html`; `enhance.js`, app.css): to the left
  one tab to the right, to the right one to the left, from wherever on the bar the finger starts.
  What moves is the bar's row of tabs, not the page (owner: „keine Seitenanimationen", but „eine
  Animation der Selection um den Prozess vom Swipen visuell zu unterstützen"): a finger that moves
  sideways rather than up or down takes the row with it inside the bar, which stays, clips the row
  and fades it out at its two ends, while the mark of the current tab stays where it is; so a
  finger to the left brings the tab on the right under the mark, with the finger as far as that
  tab and then held back (a swipe is one tab, never two), and held back from the start where no
  tab lies that way. The mark that stays is the bar's lens (`.bottomnav-lens`, made by enhance.js on
  the bar's first swipe), laid over the current tab's mark from the first move: the mark's colour
  with a copy of the row inside that moves as the row does, light, so what is under the lens is
  light and the rest dark, cut at its edge, a count with its icon; each name turns dark as the lens
  comes close. Let go a third of the way there or further, or flicked (as the sheet measures a
  flick, by the events' own times), and the tab is clicked: the page follows as it follows a tap
  (R21, one step of the history; before the app takes over the browser loads it); otherwise
  nothing happens. Either way the row springs back to its place and the lens to the tab that is
  current, on one damped spring with the small swing of the other marks that slide: the row from
  where it is and as fast as it went (a flick carries it on a few px first), the lens from a
  standstill (a glide on one fixed curve, which turned the row round at full speed, felt „ein wenig
  klunky"). The glide is Web Animations of keyframes computed from the spring, which the
  compositor runs while the tab's page is built, and a finger that catches it takes the row and
  the lens where they are, so two quick swipes go two tabs (before the app, while the next page
  loads, the bar takes taps only). The tab's own mark
  takes over once its tab is the current one, in one frame and without its fade. Up and down the
  bar scrolls the page as before (`touch-action: pan-y`); the moves of a swipe are the bar's alone
  (a quick one left to Chromium ended in a fling of nothing, and the next tap anywhere, up to a
  second later, only stopped that fling). A mouse (a narrow window) drags the row the same way, and
  what it lets go of is no click; a tap while the row glides taps the tab under the finger.
- **A row of the catalog and of the Merkliste is swiped to mark and to plan its module** (owner,
  2026-09-30: „Nach links wischen merken nach rechts wischen planen. Mach das so, dass dann darunter
  freigelegt wird was die Aktion macht (also Icon und Text)", and the same day for the Merkliste:
  „Mach das auch in der Merkliste"; `folia/crates/widgets/src/swipe.rs`, app.css): on a phone, in the browser app,
  to the left „Merken", to the right „Einplanen" — the two switches of the module's
  page, pressed from the list without opening the module, with the same effect (`MarkButton`,
  `studyplan::press`). The card follows the finger and uncovers what lies under it at the side it
  leaves: the action's icon and word, and a line of what it is done to — the semester „Einplanen"
  plans into, aimed as the switch aims (from the module's own Termine, as on its page, and the
  finder's semester and placeholder), „von der Merkliste". What the ground says is what the swipe
  does, not what the module is: on a marked module the left side says „Entfernen · von der
  Merkliste", on a planned one the right side „Entfernen · aus WiSe 2026/27"; it is read once, when
  the finger starts. The mark's side has its icon where the row's bookmark stands. It is quiet
  until the action is armed, a third of the card or 120 px away (or half as far and flicked, as the
  sheet measures a flick): then it takes the colour of its side — the inverted look of a marked
  module, the accent for the plan — and the icon springs. Let go armed, the ground says what was
  done („Gemerkt", „Eingeplant", „Entfernt"), the card goes aside as far as that takes, holds a
  moment and glides back, and the action follows after the next frame (R21); let go before, the
  card glides back and nothing happens. The row stays where it is (R5: marking and planning change
  what the module's buttons say, not the list — unless the list is filtered by them, „Gemerkt",
  „Passt in meinen Stundenplan"; on the Merkliste a module swiped off it stays, dimmed, as with
  its button). The card moves inside its own place: while it is swiped the row
  clips it and draws the ring and shadow the card has at rest, so nothing changes as the swipe
  begins and the card never reaches past the page's edge (it pushed the page 188 px wider than the
  window). The finger is read as along the bottom bar: the first move past 10 px decides (a row is
  something to tap, a tap may wobble), sideways it is the row's, up or down the page scrolls
  (`touch-action: pan-y`); the moves of a swipe are the row's alone (its own `touchmove` cancels
  them: the app is built without Leptos's delegation of events, so the listener is the row's and
  may), and neither a swipe nor its end is a tap or a step of the history. A mouse (a narrow
  window) drags the card the same way; a wide screen has the bookmark at the end of the row and the
  preview beside the list, and no swipe. The rows of both lists are the same `Row` (`swipe`); the
  Merkliste's modules the catalog does not know („Nicht im Modulkatalog") are no `Row` and keep
  their bookmark alone.
- **Every page has the same frame (owner decision 2026-09-20, R17):** a sidebar as wide as the
  catalog's filter panel, with the same handle and the same remembered width, and the page next
  to it (`ui::Frame`; the catalog builds it itself, its sidebar is the filter form). Going from
  one area to another, nothing jumps. The sidebar holds what belongs to the page as a whole:
  filters (catalog, program overview), the views of a program, the sections of a module, actions.
  On a phone a sidebar of filters is a sheet opened by the page's „Filter" button, views stay on
  top, everything else follows the page. **The landing page is the one exception (owner,
  2026-09-28):** its sidebar (the sections, the Datenstand) made it confusing and odd to look at,
  so it has no frame; its panels stand in one column in the middle (see „The landing page").
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
  `folia_pages::faculties` takes the department of the thesis module, else the department
  that offers at least half of the offered curriculum, else what the programs of the same
  subject agree on. The sidebar says so, and programs without a clear answer have a section of
  their own („unknown stays unknown", R12). The 2026-09-19 snapshot: 106 by thesis, 34 by
  majority, 4 by subject, 4 without.
- **The server's pages lay nothing beside themselves** (owner decision 2026-09-21: the server's
  HTML is for crawlers, the browser app for people, and „die GET-Parameter sollen bei SSR nichts
  fürs Seitenlayout machen"): `open`, `full`, `area` and `req` are the app's. The server renders
  the catalog and a program's page as if they were not in the address (`CatalogPage` and
  `ProgramPage` drop them where `APP` is false, `cache_key` too), and links pages of their own
  where the app would open something beside the page: a row of the catalog's list and a module
  of a program lead to `/catalog/module/<id>`, an area of a program to the catalog narrowed down
  to it (`?program=<slug>&area=<id>`), a row of the plan that names no module is text. The app
  keeps the preview, the panel beside the program and „Vollbild" in place, and turns a shared
  address with these parameters into what it names.
- **A module is shown where it was opened** (2026-09-21): on the desktop beside the list or the
  page (`open=<id>`), and in full where „Vollbild" is asked for — the catalog's preview leads to
  the module's own page (its area), the program's panel and the preview beside the marked
  modules to `…&full=1` (the same page, rendered by `ModuleFull` in place, so the tab, the history
  and „Zurück" stay what they were; before, „Vollbild" out of a program and out of the marked
  modules switched to the catalog's address, and the back graph and the tabs had to guess). On a
  phone nothing stands beside a page: what is tapped is the page, with one tap and one history
  entry — a row of the catalog leads to the module's page, a row of the marked modules to the
  module in the list's place; on a program's page a module, an area or a row of the plan becomes
  the page (`Filling` in `folia/crates/programs/src/program.rs`), and „Zurück" leads to what it was picked
  from (a module picked out of an area back to the area). No preview that then has to be opened
  in full, no panel that unfolds under the page. Without the app the same HTML (the panel beside
  the page) is shown as the page by the stylesheet (`.aside-picked`).
- **Local views (owner, 2026-09-24: „so, dass man das in jedem Tab ganz einfach implementieren
  kann als lokale Ansicht"; `folia/crates/widgets/src/local.rs`):** showing a module in place is one mechanism,
  not a feature of a page. The program page and the marked modules use it, and so will the
  semester plan. An area that lists modules gets it with five parts: its address implements
  `url::LocalView` (`open`, `full`, read and written by `url::local_from_pairs` and
  `url::local_pairs`); its page asks `local::filling` whether the module fills it (after
  „Vollbild", and on a phone whatever is opened) and then shows `local::ModuleInPlace` with
  `local::back_href` as „Zurück", else its own content with `ModulePanel` beside it and
  `local::full_href` as „Vollbild"; its rows lead to the module beside the page on a phone as
  well (`Row` with `in_place`); `pending::change` names its steps with `local_change` (the
  module coming to fill the page is the module's page, going back is the page's column); and its
  `tabs::Area` says `shows_in_place`.
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
  `localStorage`, the switch is in the sidebar and needs JavaScript (R9, R15), and the matrix is
  the app's default on a large screen. **The server's HTML and the phone always draw the list,
  and offer nothing else** (owner, 2026-09-21: a phone has no room for the matrix, and a list is
  what a search engine or any other reader of the HTML reads best); on a phone the switch is not
  there. **The matrix never runs into itself and never scrolls sideways** (owner, 2026-09-21: at
  1200 px „Art" lay over „Modul"): the names are at least 200 px wide, „Art" gives way before
  them, and where the page is narrower than the names and the semesters the app draws the list —
  the switch then shows the list, the matrix greyed out with a line saying why, and the choice
  stays: with room (a wider window, a narrower panel) the matrix comes back (`matrix_min_width`,
  the same numbers as `.matrix` in app.css).
  **What a plan adds up to is the plan's own arithmetic, not ours** (owner, 2026-09-22: Informatik
  B.Sc. said 166 LP and showed nothing for its last two semesters). A row may print a range —
  „Komplex Grundlagen der Informatik, 10–24 LP" — and three of them are anything between 30 and
  72 LP, so adding the rows up gives a lower bound, not the degree. The regulation prints the
  answer in the lines over its own rows, and Radix keeps them with the rows each counts
  (`plan_total`, `docs/radix/schema-v2.md` §6). The head's „LP", the sum under the matrix and the sum
  of every semester group of the list are those printed lines where
  a plan has them; the semesters a regulation sums together („5.–6.") stand as one figure across
  them, which is why those semesters used to be empty. Without such lines nothing changes: only
  what the plan puts into a single semester is added up in that semester's column, and a footnote
  says so where a plan has modules over several semesters.
  A row that prints a range says in its panel which rows it is chosen with and what they come to
  together („zusammen 44 LP … einzeln 30 bis 72 LP"), each of them a link to its own row: that is
  the only statement the sources make about how such a budget is split. Where a regulation prints
  a span for a whole semester instead of a number, because several of its rows are budgets, the
  program's LP are that span („116–126 LP", Angewandte Mathematik M.Sc.) — a number the sources
  do not state is not put in its place.
  **The page has a panel on the right** (`ui::Frame`'s `aside`, as wide as the catalog's preview,
  same handle, same remembered width): a module clicked in any of the three views opens in it
  (`?open=<id>`, the same panel as in the catalog, so a module reads the same wherever it is
  opened); an area clicked in „Wahlpflicht & Bereiche" — in the table or in the sidebar, which
  also brings it into view — shows what it holds (`?area=<id>`: its numbers, the areas under it,
  and its modules, each of them opening in the same panel and coming back to the area when it is
  closed); a row of the study plan that names no module of the catalog — most of them are
  requirements („Wahlpflichtmodule der Studienrichtung", „Wahlpflichtmodul aus der Informatik") —
  shows what the plan states about it and where the modules that can be chosen are listed
  (`?req=<n>`, the row's place in the chosen plan). All of this is the app's: the server's page
  links the module's page and the catalog narrowed down to the area instead, and shows the row as
  text (see „The server's pages lay nothing beside themselves"). **No source links a row to an area**
  (`area_rules` is prose about credits), so the name does the work and the panel says so
  (`folia_plans::plan::areas_for_row`, rewritten 2026-09-21 after the owner found „Komplex Praktische
  Informatik" pointing at four areas, and checked the same day against all 1 059 rows of the real
  plans that name no module): only areas a student chooses from come into question (a
  requirement row never means the Pflichtmodule); the words that say what kind of thing a name is
  („Komplex", „Wahlpflichtmodule", „WP"/„WPF", „Modul aus dem", „Compulsory Elective Modules",
  articles) and what the plans print around a name (numbering „5 …", footnotes glued to a word
  „Wirtschaftswissenschaften2", references „gem. Anlage a5", „Prü/SL") are dropped on both sides;
  a plural reads as its singular, a roman number as its digit, and „Anwendungsfach",
  „Anwendungen", „Anwendungsbereiche" as „Nebenfach". Then the same words as an area are that
  area and no other („Komplex Praktische Informatik" → „Praktische Informatik"); a row that lists
  areas by name means each of them („Wahlpflicht: Komplex Grundlagen der Informatik / Komplex
  Praktische Informatik / Komplex Angewandte und Technische Informatik", „Schwerpunkt A oder
  Schwerpunkt B", „„A“, „B“ oder „C“"); a name within an area's name or the other way round fits
  next best and all that fit equally well are meant („Wahlpflichtmodul aus der Informatik" →
  every „… Informatik" area to choose from); words in common count least, only where they are at
  least half of the row's words and never by numbers (so a module „Physics of Modern Devices" is
  not the area „Technology and Devices", „Schwerpunkt 1" not „Konstruktiver Ingenieurbau - 1"),
  and are only named as also possible when they come near the best. A name that is a node above
  the leaves means the leaves below it — except those another row of the same plan names on
  their own („Anwendungsfach" → Mathematik, Physik, Maschinenbau / Elektrotechnik,
  Wirtschaftswissenschaften, Bauingenieurwesen; Praktische Mathematik has its row „Modul aus dem
  Bereich Praktische Mathematik"). An area whose label is only a kind is known by its field
  („Wahlpflichtmodul (MIT)" is the list of „Informatik (MIT)"). Only the study directions the
  plan's caption names are kept, by their short names or a shorter spelling of them („PA und
  IoT" → PAu, IoT) or by the name the tree puts in front of one („Studienrichtung
  „Umwelttechnik“" → „Umwelttechnik (UMT)"). A single best fit is shown with its modules, and
  where several fit equally well all are named instead of one being picked (R12). A row the plan
  states as Pflicht, Abschlussarbeit or Praktikum means one module, not a choice: it gets no area
  at all, only the honest note that the catalog does not know it under this name and a search for
  it; a row that names the FÜS by kind **or by name** („Fachübergreifendes Studium", „Modul aus
  dem FÜS-Katalog der BTU" — 112 of them are stated Wahlpflicht) leads to the FÜS list
  (`plan::is_fues`). The panel of an area is the same kind of panel (owner, 2026-09-23): the way
  into the catalog narrowed down to the area first, then the areas under it and its modules;
  opened out of a row of the plan the row stays in the address (`ProgramUrl::with_area_keeping_req`),
  so the panel says how one got there — „Anwendungsfach / Mathematik", each step a link back —
  and closing it returns to the row. **With nothing picked nothing stands beside the page**
  (owner, 2026-09-23: on a 13-inch screen a third column left the plan too little room): the
  panel floats over the page like the catalog's preview, docked to the right edge, and is there
  only while a module, an area or a row is picked. What it held with nothing picked went: the
  numbers of the head repeated, and the bars „LP je Semester" repeated the sum row of the plan
  (moved to the sidebar first, then dropped by the owner the same day: redundant, and they pushed
  what the sidebar is for out of view). Until then the panel was a column of the layout, and the
  tables gave up the columns that carry least (Bereich, Turnus, Nr.) as it got narrower. Tried before and dropped
  (owner, 2026-09-20): the matrix as a centred block in a wide empty panel — „liest sich zwar
  leichter, sieht trotzdem komisch aus"; the width wants content, not air.
- **„Mein Studium": the Studium tab plans the study** (owner, 2026-10-04; `folia/crates/planner/src/study`,
  `folia/crates/plans/src/study.rs`). Owner: „für den Workflow passt es viel besser, wenn da primär
  die Studium planen seite drin ist … Man landet ja aktuell immer auf der regelstudienplan seite.
  Aber die Informationen ist eigentlich gar nicht so primär relevant … Primär soll man da sein
  Studium planen können. Also das aktuelle und zukünftige Semester. Dabei ist wichtig dass der
  regelstudienplan ja eine Illusion ist, deswegen muss man den plan so machen können dass die
  fälligen module pro Semester und halt von den vorherigen, die nicht abgeschlossen wurden als
  erstes im plan ist". So:
  - **Where.** In the app the Studium tab leads to `/study` (see „The areas are tabs"); the overview
    and the programs' pages are what the study is looked up in: „Regelstudienplan", „Wahlpflicht &
    Bereiche" and „Alle Studiengänge ansehen" stand in its sidebar. A program's views are the plan
    and the areas; its „Mein Plan" is this page now. The visitor's own program carries „Studium
    planen" among its actions (beside „Mein Studiengang", the app's alone), the start page's step
    „Studiengang wählen" leads here once a program is kept (it led to the Regelstudienplan), and so
    does the Stundenplan's „Studium planen →". The top bar says „Mein Studium" and searches modules.
  - **What it shows.** Without a program what the page is for and the picker of all programs (a
    pick is „Mein Studiengang"). With one: the head (the program, „3. Fachsemester · WiSe 2026/27",
    the credits passed of what the plan comes to as a bar, „Regelstudienzeit bis …" and
    „voraussichtlich fertig: …"); „Bisher", the semesters before the current one, folded and open
    while something there is not ticked off („Hake ab, was du bestanden hast", „Alles bestanden"
    per semester); and a panel per semester from the current one on, to the end of the
    Regelstudienzeit and on to the last one that holds anything („nach der Regelstudienzeit"). A
    semester names its Fachsemester and „jetzt", and what it comes to beside what the plan puts into
    it (in the warning colour where it is more; „≥" where a row states a range or stands in
    several semesters). A row: the box that ticks it off („bestanden"; a row of the plan without a
    module is „erledigt"), its name (a module opens beside the page, a local view; a row of the plan
    has „Module finden", the catalog of what it means, `variants::row_query`), its marks
    („nachholen · aus 1. FS", „verschoben · laut Plan 4. FS", „vorgezogen …", „nur im WiSe" where the
    turnus moved it, „eigenes Modul", „im Stundenplan"), its credits, and ↑ ↓.
  - **The rule** (`folia_plans::study::study`, pure and run on every plan of the snapshot in its
    tests), from the Regelstudienplan of the kept study direction, the Studienbeginn, the catalog's
    turnus and what the student says. A module passed stands in the semester it was passed in and
    nowhere after. One the student placed into a semester from the current one on stands there.
    Any other stands in the first semester from its Fachsemester on, and not before the current
    one, that offers it. So what is not passed when its semester ends is due again, first, in the
    next semester that offers it, and the plan moves on by itself as the semesters pass; a
    Studienbeginn in the other half of the year moves a module of one season by the turnus as well.
    In a semester from the current one on what is left over comes first (by the semester it was
    due in), then the plan's rows in the plan's order, then the student's own modules; a row ticked
    off keeps its place, so nothing jumps under the pointer, and what the tick takes out of a later
    semester goes there. A row of the plan without a module has no turnus: it stands in each
    semester of its span that is not past, after the last one the student placed it in, and in the
    current one once its span is over. A module planned besides the plan (from „Einplanen", a choice
    for a row) stands where it is planned; one not passed is not planned again by itself (another
    elective may take its place), the row it counted for is. What the semesters before the current
    one held stays in them, open or ticked off. Unknown turnus restricts nothing (R12); one of even
    or odd years counts by the year a semester begins in.
  - **What is stored** is only what the student says, in the Stundenplan's store (R20): `d
    <semester> <module>`, a module passed, and `q <semester> <program> <ord> <caption> <name>`, a
    row of the plan done (found again by its name where a later snapshot numbers the rows anew),
    and for where a module or a row is placed the `m` and `p` lines the Stundenplan keeps anyway
    (`PlanDoc::place`, `place_row`): a module placed into the current semester is in its
    timetable, and one the Stundenplan plans is placed. ↑ ↓ place into the semester before or after
    that offers the module (not before the current one, not past the 30th Fachsemester), ↺ takes the
    placing back. A build before these lines keeps them as lines of a tag it does not know. The
    Studienbeginn is „Mein Studiengang"'s (`start`): until one is stored the page assumes the start
    of a study now in its first semester (`studyplan::intake_start`) and asks „Stimmt das?"; the
    study direction is the plan kept with „Mein Studiengang" (`ProgramPlans`), chosen in the sidebar
    as on the program's page. The privacy notice lists it („Speicher im Browser").
  - **And the Stundenplan.** The current semester goes into its timetable with one click („In den
    Stundenplan (n)": its open modules, those of the plan marked as taken from it, and its rows as
    placeholders, which the finder fills as before; „Rückgängig"). The Stundenplan's „Importieren"
    has „Mein Studium" as its second source (it was „Mein Plan · bald"): the semester shown as the
    study has it, what is left over first. Its tab counts the current semester's modules, not what
    „Mein Studium" places into later ones, and so does the start page's step; „Plan leeren" empties
    the semester shown and leaves the others and what was passed.
  - `node folia/e2e/study.mjs` checks the page, the tab, the rule on Informatik B.Sc. in its third
    semester, the timetable and the phone.
- **The search in the top bar belongs to the page:** modules everywhere, programs on `/programs`.
  In the browser app it filters while typing (history entry replaced, not added). How it finds and
  orders modules: „The search of the catalog“ below.
- **„Nach oben" (owner, 2026-09-26: „wenn man im Modulkatalog ne weile gescrolled hat, … richtig
  schwierig wieder nach oben zu kommen"; `ui::ToTop`, `enhance.js`):** one round button for every
  page, in the corner of what scrolls. On a wide screen that is the page — what the ground takes
  for the page: the catalog's list, the marked modules, a framed page — and the button stands
  16 px inside its panel, left of a module or an area that floats beside it (`--preview`, now
  declared on the view so that the button can use it), and goes up with the view when the ground
  comes; the wheel over it turns the page under it. On a phone it is the window, and the button
  floats 12 px above the bottom bar at its right edge (44 px), under the bars, the sheets and the
  skeleton of a page. It shows once the page is more than a screen down — the page is asked where
  it is when it scrolls and when another page stands there, never for a skeleton, so the frame
  after a click (R21) lays out nothing ahead of time for it — and takes it to its top: at once to
  a screen above the top, then gliding the rest in a third of a second, a frame at a time. Not the
  browser's own smooth scrolling: over the virtual list it would build every row it passes and end
  where the list makes up for a row taller than estimated (a script's scroll ends a smooth one);
  written frame by frame, the glide arrives at the top whatever the list did in between. A wheel,
  a touch, a click or a key stops it; where less motion is wanted it is up at once. The list
  follows as it follows any scroll (`page` leaves the address), a module beside it stays, and on a
  wide screen the ground goes back down (the page has left its end). Pressed with the keyboard,
  the focus goes to the start of the page (`#content`, where „Zum Inhalt springen" leads), not to
  where the button was; Tab from the end of a page meets it before the ground. It needs
  JavaScript (R15); the classic site before the takeover has it as well.
- **The Stundenplan on a phone** (owner, 2026-09-27: „Es gibt keine Wochenansicht beim Kalender
  auf dem Smartphone"; `folia/crates/planner/src/studyplan/week.rs` `WeekCarousel`, `mod.rs`): „Woche" is
  the week grid of a wide screen too, fitted to the height the screen leaves under the bar of the
  search and over the bottom bar, and it alone takes the whole width of the screen („den ganzen
  horizontalen Platz"; it leaves the page's margin and the panel's, 12 + 14 px, and the rest of
  the page keeps them). Its slots say what and where they are held („zeige auch die location
  an"), their time only where a slot is wide enough (a tablet's); the red mark of a clash stands
  in a slot's bottom corner, where it covers no name. A slot is a link to its module; the „✓" and
  „×" are the list's. Where the plan has Termine of A or B weeks only, the grid is a carousel of
  „A-Woche", „B-Woche" and „A/B", as the pictures of the start page are one, with the tabs under
  it (they take the place of the head's switch): a finger carries the weeks sideways and lets the
  next one in, any other move scrolls the page, and the three stand in a row without going round.
  Under the grid the list of the week's days (each Termin a row as tall as a finger, with its
  buttons) is closed until its line „Termine als Liste (8) +" opens it („standardmäßig
  eingeklappt"); the line counts the week shown, and the list stays open while a module opened
  from it is the page. With nothing planned a phone shows no empty week, only „Noch keine
  Termine" and the ways to modules, as before: the page stays shorter than the screen.
  „Kalender" (the .ics file and the subscription) stands under the Termine, in every view: in the
  sheet „Anpassen", among what is shown, nobody looked for it. A wider screen keeps it in the
  sidebar.
- **Tokens:** `folia/assets/app.css` starts with the token block (colors, radii, shadows); everything
  below uses tokens only. One look, light and dark: dark follows the system, the switch in the rail
  overrides it (`data-theme` on `<html>`, remembered in `localStorage`). Accent color only for
  primary actions and the marker of the open row; selected chips are neutral (inverted). The
  accent is a muted birch-leaf green, `oklch(.53 .07 149)` in both themes (owner, 2026-09-22,
  replacing the blue: "wir nennen das ding betula"): the hue of the owner's favourite tone
  `oklch(.6867 .0996 149)`, deep enough for white labels. Labels on the accent are always white —
  dark text on the green was "grauenhaft" — and the owner prefers toned-down colours to saturated
  ones.
  Font: Inter (variable, latin subset, OFL), self-hosted. Icons: Lucide (ISC), inlined through
  `folia/crates/design/src/icons.rs`. The only `style` attributes carry data as custom properties: the week grid
  and the credit slider (`--from`, `--to`, `--at`), the place of a picker's popup, `ui::Hit`.
- **Blocksatz** (owner, 2026-10-01: „support für Blocksatz", „Ja mach mal Blocksatz überall ab wo es
  sinnvoll ist/gut aussieht"): running text that is read through — a module's texts, the parts of
  the Impressum and the Datenschutz, the answers to the questions on the start page — is justified
  and hyphenated by the rules of its language (`<html lang>`; a module's text carries its own),
  never into syllables of fewer than three letters or words of fewer than six. What is read at a
  glance stays ragged: headings, leads, hints, notes, labels, and the cells of a grid — the
  abilities on the start page were tried and their lists of long nouns, some 45 characters a
  line, stood apart by wide gaps. A column narrower than 23 em of its text (some 42 characters a
  line: a phone narrower than 390 px; at 40 the gaps showed) is set ragged, the items of a list
  below 32 em; each text asks its own container (`prose`, `part`, `faq` in `app.css`).
- **`assets/enhance.js`** (progressive enhancement until the browser app takes over): the plain
  fields of the filter form apply on change, panels keep their scroll position across page loads.
  In both modes: the shortcuts, the theme switch, the filter sheet, the swipe along the phone's
  bottom bar, and the two resize handles. Page changes use
  cross-document view transitions where the browser supports them. Their opt-in
  (`@view-transition`) is written inline into every head the server writes
  (`app::VIEW_TRANSITION_STYLE`), not into app.css: Chromium decides when it first shows the new
  page, from the style sheets applied by then, and the stylesheet (revalidated on every load) often
  arrives after the parser has reached `<body>`. The page then came without the fade and with
  "ViewTransition opt-in disabled" in the console (`folia/e2e/gate.mjs` checks it with a slow stylesheet).
- `folia/design/prototype.html` is the clickable design prototype the direction was agreed on, and
  `folia/design/tabbar/swipe.html` the one of the swipe along the bottom bar;
  `node folia/e2e/shot.mjs <url> <out.png> [w] [h] [--dark]` takes review screenshots.

### Data flow

- **Pages ask, through one seam** (2026-10-03, docs/folia/folia-refactor.md §6.4). Every question a
  page has is a type of `folia_pages::ask` (24 of them, one per loader, with its answer), asked
  through `folia_data::DataClient`: `get` in a reactive scope (the answer kept, or the question on
  its way and the scope run again when it is there), `ask` in a handler, `use_ask` for a memo that
  keeps the answer before while the new one comes. On the server the client answers on its thread
  from the active snapshot (rusqlite), so a page renders complete in one pass; in the browser the
  data worker answers. Everything a page shows comes from one snapshot.
- **The server renders and caches.** HTML depends only on URL + snapshot (rule R9), so the first
  request renders (5–100 ms) and later ones are a memory copy (2 ms), brotli included; the cache
  keeps the compressed page only, and the pages of the sitemap are rendered into it after every new
  snapshot while the server is idle („Load“ in §3). A new snapshot starts a new generation. ETag
  per generation and build → `304` without rendering.
  Pages are `public, no-cache`: the browser asks every time and mostly hears `304` (until
  2026-09-21 they were `max-age=300, stale-while-revalidate=86400`, so after a deploy a browser
  showed the old build's page with the new build's stylesheet for up to five minutes, and once
  more after that; the server has only the files of its own build, whatever `?v=` asks for). The
  page is what names the build of its stylesheet, scripts and bundle (`?v=<build>`), which the
  browser keeps without asking (`immutable`, „Caching and compression" in §3): the page's `304`
  is how a new build reaches it.
  `404`/`5xx` are `no-store`. Without a snapshot everything answers `503` + `Retry-After`.
- **The browser app (owner decision: all queries run in the browser, 2026-10-03 in its data
  worker).** `assets/boot.js` starts the data worker (`folia/crates/worker/js/data-worker.js`, with
  its own bundle `folia-worker`) and loads the app's bundle in parallel. The worker finds the copy
  of the snapshot it kept in IndexedDB (by its ETag, as a Blob), asks the server what it has
  (`/api/status`), downloads a new one (`/api/db`: 44 MB, 4.4 MB in brotli; one tab at a time, a
  Web Lock) and opens it in sql.js; the page's thread holds no catalog. Then `client::start()`
  replaces the server-rendered body by the app, the server's page staying in front of it as a
  picture until the app's first answers are in (`betulaAnswered`). Not hydration: the copy may be
  older than the server's page, so the app renders fresh with the same components. From then on
  links, filters and the search are client-side navigation; a step keeps the page before in front
  of the new one until its answers are there (`pending::hold`, at most 400 ms). Until the takeover,
  and if anything fails, the site stays a website served from the HTML cache.
  A copy of an older schema than the build reads is never opened (`user_version` in the SQLite
  header against `folia_model::SCHEMA_VERSION`): with the network it is replaced first, offline the
  app does not start, and the status says so. A server whose own snapshot is older says so in
  `/api/status` (`snapshot.outdated`), and nothing is downloaded from it. A newer snapshot is
  shown at once (owner, 2026-10-02): the worker looks when the tab comes back into view and every
  five minutes, opens a new one beside the one in use and answers from it; the page shows the
  answers it had until each new one is there (`DataClient::forget`), so it stays where it is, and
  the other tabs open the copy the first one kept (a BroadcastChannel). The server never answers
  data queries for the app: its load is cached HTML, static files and one database file.
- **Fine-grained updates:** the catalog page splits its URL into the filter (what the list is),
  `page` (where the visitor is in it) and `open` (the preview). Opening a preview or scrolling
  re-renders neither list nor filters, and a filter change leaves the preview alone.
- **The list is virtual** (`VirtualRows` in `folia/crates/catalog/src/catalog.rs`, 2026-09-21; before,
  chunks of 50 were appended and prepended, and a long scroll grew slow): an element as tall as
  the whole list holds only the rows that are on screen and a few around them, each at its
  offset, so the scrollbar has the length of the list from the start. Rows are measured once
  rendered and estimated (at the average of the measured ones) until then; a row above what is
  visible that turns out taller or shorter than estimated moves everything below it, so the list
  scrolls by the difference and nothing jumps under the visitor's eyes. Pages of 50 are loaded
  when their rows come near (one query, a few milliseconds) and dropped again when far. Above and
  below the rendered rows stand skeleton rows (`.vfill`; a row whose page is not loaded yet is one
  too, `.vfill-row`): a fast scroll that runs ahead of the list in the compositor's frames shows
  rows being filled in, not an empty panel. A fill is one row of the list's columns, as tall as
  the fill, each column painting its bar again every row (`skeleton::fill`, eight elements however
  far it reaches), and on a phone the 24 cards next to the rows come first, an element each with
  their bars in their background (`skeleton::cards`); built as rows of sixteen elements they made
  half the elements of the page („A phone that froze" below). `page` in
  the URL follows the row at the top of the screen (history entry replaced); a shared link with
  `page=7` starts there, and coming back from a module the list centres on its row
  (`queries::catalog_position`: the row's place in the ordered list, one window query, no pages
  loaded before it). A filter change starts at the top: the list is rendered anew, but its panel
  is the same element as before and keeps the scroll position, which the new list would otherwise
  take for its own and write into the URL as its page (the first list of a visit is left as it
  is: a page the browser restored). The server renders the page the URL names, with pager links (no JavaScript,
  search engines). Inside a program the list is in plan order with the plan's semester at every
  row; the headings between the semesters are gone (the semester filter is for that) — so the
  elective modules, which the plan places in no semester, are simply the rows after the last
  semester, and the area picker lists them by area.
- **A semester lists what can be chosen for it, too** (`folia_plans::plan`, 2026-09-21): a plan
  places the compulsory modules in semesters and asks for the rest with rows that name no module
  („Wahlpflichtmodule der Informatik, 12 LP"), so „3. Semester" used to show two modules where
  five are to be taken. Now `pages::catalog` reads the plan's requirement rows of the semester,
  derives the areas their names point at (the same derivation the program page uses beside a row
  of the plan: what the name and an area's label have in common, within the plan's study
  direction) and fills them into the query: the list holds the modules the plan places in the
  semester and, unplaced, the modules of those areas — or, where a row points at no area, every
  elective the plan places nowhere. A note above the list says what the plan asks for, which
  area each row was taken to mean (as links to the list narrowed down to it), and that this is
  derived from names, never stated (R12). Rows stated as Pflicht, Abschlussarbeit or Praktikum
  without a module are one module the catalog does not know under that name; FÜS rows point to
  the program's FÜS list. The URL still says only the semester; the derived areas are part of the
  query the page ran (`CatalogData::effective`), which the endless list loads further pages with.
  **The note is a line a row and scrolls with the list** (owner, 2026-09-23: fixed above the
  rows it left room for two of them on a laptop, and „viel Redundanz"): „≥ 6 LP Anwendungsfach:
  „Mathematik“, … oder „Physik“" — how much (a choice at least that much), what the plan calls
  the row and the areas; a name that only repeats its areas gives way to „aus dem Bereich" /
  „aus den Bereichen" (`SemesterRequirement::named_by_areas`: „Modul aus dem Bereich Praktische
  Mathematik", „Wahlpflicht: Komplex A / Komplex B"), and the areas the name fits less well are
  no longer named. The note stands in the rows' scroll area above the heads of the columns, which
  stick to its top (`.rows > .cols`, as tall as `--cols`, so that the skeleton of a filter on its
  way sticks below them); the virtual list takes the visible part to begin below the heads
  (`nav::list_viewport`) and scrolls to a row from where its content stands in the panel, not
  from what is visible now (`nav::scroll_list_to`), since the note scrolls away on the way.
- **Termine bestätigt** (`events=yes|no`, owner 2026-09-23: hide the modules that probably do not
  take place): a toggle right below the semesters of the plan (and below the program picker
  without one) that keeps only the modules with published teaching events — exactly the rows
  whose „Termine" say something other than „noch keine" (`v_module_facets.teaching_events`, the
  events of the module's newest semester that has any; 1,776 of 4,936 modules on 2026-09-23,
  1,496 of them in the WiSe 2026/27). Crossed out it keeps only those with none yet.

### A click answers first (2026-09-23)

Owner: „die js web app fühlt sich irgendwie ziemlich langsam an", switching pages and changing
filters should not feel slow, the results should come as soon as they do, and where there is a
wait, skeletons should bridge it. What was wrong, measured on the laptop of §3 with the dev
bundle (Event Timing: from the click to the next frame): **a click showed nothing until the page
was done.** The router takes a link in the click's own task and runs the queries and the new page
in its microtasks, before the browser may paint. A filter toggle took 56 ms to its next frame, a
module's page 80 ms, the program overview 88–120 ms, the start page 136 ms; with the CPU slowed
down four times (a phone) 260–280, 340, 380–850 and 590–775 ms. Where that time goes (V8 profile
of the same clicks): the queries 14 ms of a toggle and 50–72 ms of the start page and the
overview, building the page 25–75 ms, style and layout the rest.

- **Paint first, then work** (`folia/crates/shell/src/pending.rs`). A navigation reaches the router one frame
  later; in that frame what can be shown at once is shown. `Pending` takes over, before the
  router's own listeners: clicks on the links the router would take (the same checks, and not
  what another handler has claimed with `preventDefault`), Back and Forward (the browser's
  `popstate`, handed to the router again a frame later; the replay is marked, `enhance.js`
  ignores it), and what the app starts itself through `Pending::go` (the search of the top bar,
  the pickers and the credit slider, the draft of the phone's filter sheet; the search goes
  quietly, `go_quietly`: what is listed stays until the next result, since a skeleton with every
  letter would flicker). A step that changes nothing the visitor sees (the fragment, the list's
  `page`) goes to the router as before, and so does everything the app navigates on its own (the
  list following the scroll position, a phone turning `open` into the module's page). What the
  step changes decides what waits for it (`pending::Change`): another page, another column of
  the same page (a view of a program, the program overview or the marked modules filtered or
  ordered otherwise), the catalog's list, the module beside a list, what stands beside a
  program's page.
- **What was clicked is in its new state in the next frame**, because those parts read where
  the app is going (`Pending::to`) and not only where the router is: the rail and the bottom bar,
  the title and the search of the top bar, the catalog's filter panel and the tags above its list
  (so the head of the list has its height before the rows come), the row whose module opens, the
  views of a program, the toggles of the program overview, the sidebar of the marked modules.
  Two clicks before the first has reached the page add up: the second link already leads from
  where the first goes (the toggles' addresses follow the panel).
- **What still has to be computed stands there as a skeleton** (`folia/crates/shell/src/skeleton.rs`): the frame
  of the page that comes, built from the layout classes of the real one (`.work`, `.framed`,
  `.panel`, `.sidebar`, `.page`, `.row`, `.module-grid`), so every panel stands where the page will
  put its own, in both layouts and both themes, and grey bars where the text will be; the
  catalog's list keeps its frame and shows skeleton rows over the old ones; the module beside a
  list or a program has a panel of its own. A band of light sweeps over it once the wait is
  longer than .35 s, moved by the compositor, so it keeps moving while the page is being built.
  Everything that is not layout has a class of its own (`sk-…`): nothing that looks for the parts
  of a page (the count, a chip, a module's page), the checks included, finds a skeleton instead.
  `main` says `aria-busy` meanwhile. A module that replaces its skeleton does not slide in
  once more (`data-settling` for that frame), nor does a skeleton where a module stood already.
- **A skeleton only where the wait is seen:** a change of the same kind that took at least 50 ms
  the last time (`SLOW_MS`, smoothed), and always the first time. A result that comes quicker
  would come about when the skeleton does, and the skeleton would only flash; then the click
  shows its new state in the next frame and the result in the one after. On a laptop the
  preview and most lists come that quickly; on a phone nearly everything waits long enough for
  a skeleton.
- **The answers of the local catalog are kept for the visit** (`folia/crates/client/src/lib.rs`, `Answers`):
  by statement and parameters, up to about 24 MB (the oldest go first). The copy `boot.js` opened
  is never written to and stays the same until the next start, and no query reads the clock, so
  an answer holds for the whole visit. Coming back to a page, Back, and a filter taken back ask
  sql.js nothing: the start page's queries alone take 70 ms of a laptop.

After (same machine, bundle and measurement): from the click to the next frame 24–32 ms, the
frame itself after 5–10 ms (the new state, perhaps a skeleton); with the CPU four times slower
16–104 ms instead of 250–850. The result comes a frame later than before (8–18 ms, the skeleton
a few of them): a toggle 36 ms (33–36 before), the preview 24 ms (16), a module's page 68 ms
(58); coming back it comes sooner than before: the start page after about 50 ms instead of 100,
the program overview after 27–32 instead of 65–73. The bundle that ships (`wasm-release`) does
the same on this machine: 24–40 ms to the next frame, the frame itself after 6–9 ms.

Asked by the owner and not done, measured:

- **SQLite in the Rust bundle** (rusqlite on `sqlite-wasm-rs`) instead of sql.js behind a
  JavaScript bridge. The catalog is in memory either way (sql.js keeps all 37 MB in its WASM
  memory); what the bridge costs is handing each answer from sql.js to JavaScript and on to Rust:
  10–13 ms of the 48–54 ms of queries of the start page and the program overview (SQLite itself
  38–41 ms), 7 of 14 ms of a toggle. It would need a C toolchain for `wasm32` in
  `build-client.sh` and in Nix, and `boot.js` to hand over the bytes instead. Kept answers take
  away all of it on a second visit.
- **The queries in a Web Worker**, so that the main thread can paint while they run. The larger
  part of a click is building the page (25–75 ms), which stays on the main thread; and every page
  would have to load asynchronously, with loading states, against „Data flow" above. A worker
  only to warm the kept answers would hold a second copy of the catalog (37 MB) on a phone. Done
  for the search of the top bar since (2026-10-02, „The search of the catalog", „Typing"): a
  search comes with every pause of the typing, and its queries were what made it lag; a click
  still runs its page's queries itself.
- **Warming the answers ahead, in idle time.** A page's loader cannot be split (the start page's
  is 70 ms of a laptop, 280 ms of a phone), and a tap that comes during it waits for it.

`node folia/e2e/snappy.mjs` checks all of it (§4).

### A phone that froze (2026-10-02)

Owner, on an Android phone with the app installed: „Wenn man das auf dem handy verwendet friert
das häufig ein und reagiert nicht mehr", shortly after opening it and when switching tabs; „das
sind bestimmt nur symptome von einem größeren problem". It was the main thread. A phone has one
for everything (the touches, the page, sql.js, the app), and the start and every tab switch held
it for seconds: whatever was tapped meanwhile waited, and a tab tapped while the app was starting
loaded the next page and began the start anew. Measured in Chromium with the CPU slowed down four
times, a phone's window (412 × 915), the real catalog (45 MB), a warm start (the catalog in
IndexedDB, the bundle in the cache), the build before and after side by side: the long tasks (over
50 ms) from the navigation until 8 s after the app runs, and those of a tab switch until 1.2 s
after its page is there.

- **`enhance.js` measured and restyled every page on a phone, for what only a wide screen has.**
  The cover over the ground, the heads of the list's columns, the scrollbar's gap and the wood that
  rises at the end of the scroll area (`--bar`, `--cover`, `--list-head-h`, `.short`) were worked
  out with every change of the page, so as each page was built, a layout in each of its frames, and
  written even where nothing had changed: a custom property written on an element restyles all of
  it. „Nach oben" asked where the page was as each page was built (at the start of the app the
  whole page was laid out twice for it). Now that part is off on a phone (a window that becomes
  narrow gives back what it was given), writes only what changed, and the button looks once the
  page scrolls or has loaded. About a quarter of a tab switch.
- **The catalog was kept in IndexedDB as bytes** (an ArrayBuffer of 45 MB under `current`): every
  start copied it into the page in one piece, half a second of the main thread, and an update
  twice more. It is kept as a Blob under `catalog` now: IndexedDB keeps it as a file of its own and
  hands it back unread, and `arrayBuffer()` reads it away from the page while sql.js loads. A copy
  under the old name is taken over once and deleted, so that a build of before (a release rolled
  back) finds none and fetches the catalog anew rather than failing on a Blob; old → new → old →
  new each started. A copy the browser cannot read any more (the file of its Blob gone; Chromium
  keeps a large value as a file of its own either way) is replaced as an older one is: before, the
  app never started again (`NotFoundError`, at every start).
- **The semantic search was loaded at every start** once the browser was idle: its vectors read
  out of the catalog and handed to its worker, and its model's 15 MB, on a phone at a moment nobody
  could see coming, for those who never search too. It is loaded when it is first wanted now
  („Ähnliche Module" below).
- **The start page built what nobody sees:** 2,200 elements of the large map in its closed
  `<dialog>`, built now as it first opens (the server's page has it as before); and the start page
  and the program overview are 17 and 10 screens of a phone, whose panels are drawn only near the
  screen now (`content-visibility: auto`, with the height they were drawn with; a jump to a section
  draws them on its way, `.jumping`, or the glide ended beside it). A phone's closed sheet of
  filters, a third of the catalog's page, is not drawn until it opens (`content-visibility:
  hidden`, until it has slid down again).
- **A link followed while the app was starting loaded the next page**, the classic website's way:
  a blank screen, and the start from the beginning, the catalog read anew. `boot.js` says that the
  app is on its way (`__betulaStarting`), and `enhance.js` waits for it meanwhile: a tab is current
  at once, and the app goes where the last link leads as soon as it runs (`betulaStarted`); where
  it does not start (an error, or nothing for 6 s), the page loads as before.

After: a warm start keeps the main thread busy for 1.8–2.3 s instead of 4.6–5.6 s, and the app
runs after 2.2–2.5 s instead of 3.7–3.9 s (three starts each). Six tab switches (catalog, programs,
start page, twice) take 3.8–4.3 s instead of 7.7–8.4 s, the start page 0.6–0.7 s instead of
1.6–1.9 s. A tab tapped as soon as the page shows: its page after 4.1–5.0 s instead of 6.2–7.7 s,
and the page is never loaded again; before, it was in 9 of 14 runs, and the start began anew.

Not done, measured:

- **The takeover** is the longest task left, 1.0–1.3 s of a phone at every start (1.2–1.8 s before):
  `client::start()` builds the whole page anew in place of the server's. Hydration would take the
  server's page over as it is; it needs the local copy to be the server's snapshot, which it need
  not be („Data flow" above), so a start on another copy would still build anew.
- **Areas that stay built:** a tab left could be kept, hidden and not drawn, instead of being built
  again when the visitor comes back. The router and the pages assume one page at a time (its ids,
  its effects, what it reads of the address).
- **The start page's numbers** could come computed with the snapshot, from Radix:
  `queries::program_department_counts` alone takes 250 ms of a phone, at the first visit of the
  page in a start (its answers are kept, „A click answers first" above).
- **sql.js copies the catalog once more**, into its own memory (`new SQL.Database`, 300 ms of a
  phone).
- **A panel the browser skips paints only its own background:** a fling down the start page that
  comes to a panel before it is drawn shows it empty for a moment. The catalog's lists paint a
  skeleton there (`.vfill`, the plain lists' `.row-wrap`, „Termine"'s weeks); the panels of the
  start page and the program overview have none yet.

Then the skeletons a fast scroll shows (owner: „Mach das mal auf develop", the same day). They
had come with 24 skeleton rows above and 24 below the rendered rows of the catalog, sixteen
elements each and built anew with every list: some 770 of the page's 1,800 elements, styled, laid
out and painted at every filter, the fill above even at the top of the list, where it has no
height. Measured on the real catalog with the CPU slowed down four times, twelve filter toggles
each, the three builds side by side: 1.31 s a toggle with them, 1.13 s with the fills as they are
now (one row of the columns, and on a phone 24 cards of one element; 1,070 elements), 1.11 s
without any (style 265, 253 and 224 ms a toggle; paint 274, 207 and 228 ms). They look as
before; the bars of the fills are square, as they were already past the 24 rows. `snappy.mjs`
misses its 200 ms for the first frame now and then on the machine of these measurements with any
of the three, without fills too.

`node folia/e2e/tabbar.mjs` checks the start (§4).

### The search of the catalog (`folia/crates/search/src/lib.rs`, 2026-09-30)

Owner: the search should improve, first with the names of the modules („erstmal eine neue Zeile in
der db wo keine Sonderzeichen etc drin sind“), typos forgiven; a semantic search („Ähnliche
Module“) is worked on apart from this, so only the names are searched, not the texts of a
description. The list stays the list it was („ich mag das UI da und würde das ungerne tauschen“).

- **What is searched:** the words of the query in the names Radix folds (`v_module_folded`,
  docs/radix/schema-v2.md „Search“): the module number, the German and the English title, the initials of
  a title and the abbreviations — the module's own, those programs give it („AuP“), and the known
  short forms of words of the titles („BWL“). Case, umlauts, ß and accents do not matter („okologie“
  finds „Ökologie“: 21 offered modules, where `LIKE` found none); every word has to be found, in any
  order („lernen maschinelles“); fillers („für“, „und“, „der“ …) do not count; `%` or `_` are no
  patterns but marks between words. Not the names of the lecturers (the filter „Lehrende“ is there
  for them), not the descriptions.
- **How a word is found, best first** (the module's score is the sum over the words): as the module
  number, as an abbreviation, as a whole word of a title (its first word a little more), as the start
  of a module number („118“ finds the numbers that start with it, not 21185), as the start of a word,
  as the initials of words that follow each other („ti“, Theoretische Informatik; „ki“ finds
  „Künstliche Intelligenz“), and from four letters on inside a word („netz“ in „Stromnetze“, while
  „ki“ no longer finds „Schlüsselqualifikationen“). A number of one or two digits and a Roman numeral
  up to ten are whole words, the one the same as the other („Analysis 1“ finds „Analysis I“; 411 of
  the offered titles number their parts with Roman numerals).
- **The order:** while there is a search, the list is ordered by relevance — the most words found,
  then the score, then the title — also inside a program. A column orders the matches instead,
  „Modul“ by title (not by the study plan while searching); typing in the top bar goes back to
  relevance (`app::TopBar`), and without a search the list has its order as before.
- **What the words as typed do not find** (`search::resolve`, run by `pages::catalog_scope`, so
  that the page and the filter panel agree): where no module has all of them, the words no title
  knows are corrected to the word of a title they are within one typo of (two from eight letters on,
  the first letter right, the rules of the pickers' `fuzzy::typos_at_start`; of several, the one
  most modules have): „algoritmen“ is „Algorithmen“, „wirtschaftsinfromatik“
  „Wirtschaftsinformatik“. The head of the list says so: „Keine Treffer für „algoritmen“. Ergebnisse
  für „Algorithmen“:“. Where that finds nothing either, a query of two words or more lists the
  modules with the most of them, and says „Kein Modul enthält alle Wörter. Hier sind die mit den
  meisten davon:“. Whatever the filters: the whole catalog decides whether the words find something.
  The resolution is part of the effective query (`CatalogQuery::text_resolution`), never of a URL.
- **Outside the filters** (owner: the filters apply, with a note): under the rows, what the search
  finds that the other filters leave out, „27 weitere Treffer außerhalb deiner Filter, 3 davon
  nicht mehr angeboten · anzeigen“; the link is the catalog with the search alone (and the modules
  no longer offered, where some are among them). Without filters it is only those no longer offered.
  With JavaScript and without it alike (`pages::CatalogData::elsewhere`).
- **Speed:** the search is SQL on the snapshot like every filter (`search::Plan::table`, joined as
  `sr`). Only modules whose names contain a form of every word are scored at all: in the sql.js of
  the app on a laptop a query takes 2 to 8 ms (without that filter 11 to 72 ms, which a phone would
  have felt while typing).
- **„Ähnliche Module“** (2026-10-01; owner: „Unterteilung in Ergebnisse und Ähnliche Module“, the
  semantic search from develop, folia/crates/semantic/README.md): under the rows, after the note on the filters,
  the modules whose descriptions mean what the search says, as rows of the list under the heading
  „Ähnliche Module“ — marking, the preview, swiping and the arrow keys work as on the list's rows
  (they are `a.row`s in `.rows`, after the list). With every search of three letters or digits and
  more (`pages::searches_similar`); the filters apply as to the results; the results themselves are
  left out; at most 10, the closest first (`pages::similar`, `queries::similar_rows`). The semantic
  search is asked for the text as the list searched it, a typo corrected (`pages::similar_text`),
  and hands over the 500 modules closest to it, a tenth of the catalog, of which the filters keep
  their share: inside a program its modules among them, rather than any module of the program. Only
  the browser app has the semantic search, and only once its model is loaded (`data::Semantic`,
  `window.betulaSemantic`; none without a model on the server, without vectors in the snapshot, with
  data saving): until then, and on the server's page, nothing stands there, and the rows come when
  it answers (`SimilarModules`, a `LocalResource`), under the results, so nothing above them moves.
  It is loaded when it is first wanted (2026-10-02, before at every start): as a search field takes
  the focus, in the browser's idle time, or when a search asks it; with a mouse once the browser is
  idle („A phone that froze" above).
  **What the search finds is not among them, not even under another number** (2026-10-02; owner:
  „Was in der direkten Suche gefunden wird, soll nicht mehr bei den ähnlichen Modulen gezeigt
  werden"): a module that bears the title of one of the list's rows is left out too, and a title
  stands there once, the closest of its modules (`queries::similar_rows`, `pages::similar`). 210
  titles of the offered modules are borne by several numbers (a module per program, an old and a
  new number; on 2026-10-02) with the same text and so the same vector: a search for one of them by
  its number or by an abbreviation of its own („14851", „AGAB") found that one, and the closest of
  all „Ähnliche Module" was the same module under its other number. By id the results were never
  among them (`similar_rows` leaves out what the search finds), while typing neither: the rows of a
  list and its „Ähnliche Module" are of the same text.
- **Typing** (2026-10-02; owner: „Wenn man tippt, dann lagt das ziemlich. Die Suche muss auf jeden
  Fall asynchron, vielleicht sogar mit Service Worker gebaut werden"). Measured as `folia/e2e/typing.mjs`
  (§4) measures, on the snapshot of 2026-10-02, a key every 200 ms, the CPU slowed down four times
  (a phone): every pause of the typing ran the search's queries on the page's thread (its text
  resolved, the vocabulary of every title made again for a word no title has, the list, its count,
  what it finds outside the filters, and the 500 hits of „Ähnliche Module") and built the list
  anew — 600–870 ms in one task, keys waiting up to two thirds of a second, 6–8 s of long tasks
  while typing one word (without the slowdown 13–15 long tasks a word, up to 180 ms, keys waiting
  up to 125 ms). Now:
  - **The queries of what is typed run in a Web Worker** (`data::Worker`, `folia/crates/client/src/worker.rs`,
    `folia/crates/worker/js/data-worker.js`): a copy of the catalog the page opened, which `boot.js` hands over
    (the Blob it keeps, which the worker reads, once the app runs and the browser is idle),
    and the app's own bundle, whose `worker_catalog` and `worker_similar` run `pages::catalog` and
    `pages::similar` there, the same code on the same data as the page would. The search of the top
    bar goes quietly as before (`Pending::go_quietly`, 140 ms after the last key), but the step now
    waits until the worker has worked out the list of where it goes (`Pending::prepare_with`, which
    `CatalogPage` gives the list of an address: its query with the marks and what fits the plan
    filled in), and the page then takes that list and asks its own copy nothing
    (`CatalogPage::list`). A key typed meanwhile drops the step (`Pending::typed`): the page is not
    built for a text that is gone already. „Ähnliche Module" are worked out there too, and come
    once the visitor stops typing (`Pending::typing`, 400 ms after the last key), since built while
    the next keys come they would hold them up. Until the worker answers (a second or so after the
    app starts), where it failed, on a device that says it has less than 2 GB, and on the server's
    page, everything runs as before on the page's own copy. **Not the service worker:** a browser
    stops an idle one after some seconds (Chrome: 30), and each start would open the 44 MB again;
    it keeps the worker's script with the shell (`sw.js`). The worker is a second copy of the
    catalog in memory (44 MB) and of the bundle's code.
  - **Building the list costs less**, what is left on the page's thread: the first frame renders
    the rows a screen holds (`VirtualRows`, the rows around them a frame later), the skeleton rows
    around the rendered ones are only there for a list longer than it renders at once, the scroll position of a list that replaces another is reset
    before it is built (setting it after laid the new list out in that task, and the frame laid it
    out again), and the list's height and the rows' offsets are custom properties no element
    inherits (`@property --list-h`, `--top`; measuring the rows changed them, and every row was
    styled again).
  - **The vocabulary of the titles is made once per snapshot** (`search::vocabulary`, by its
    `content_digest`), not for every word no title has.

  After (same snapshot, keys, slowdown): 140–270 ms at most in one task, keys waiting mostly less
  than 160 ms, 2–4 s of long tasks while typing a word; without the slowdown at most five long
  tasks a word, none over 100 ms, no key waiting more than 60 ms. The rest is the browser laying
  out and painting the new rows, and the page's own reactions to them (`enhance.js`).

### From the program's page into the catalog (2026-09-21)

Owner: the program's page and the catalog should work together, the way into the catalog first
in the sidebar, with the right areas chosen. The first entry of the program's sidebar is
„Im Modulkatalog" (`program::catalog_for`), and it takes along what is picked on the page: an
area beside the page → the catalog narrowed down to that area; a row of the plan → the areas the
row means, the same derivation as the row's panel (`plan::areas_for_row`; „Anwendungsfach" →
`area=348,350,351,352,349`, the five Nebenfächer), the FÜS list for a FÜS row, a search for the
name of a single module, the program's electives where no area fits; nothing picked → the
program. The second line of the link says what it lists („Praktische Informatik", „5 Bereiche",
„109 Module"); the row's panel opens with the same way in, as its first button, and lists what
can be chosen under it (owner, 2026-09-23: it stated credits, kind and semester twice, as badges
and as facts, and put the catalog last). The catalog's area filter takes several
areas for that (`ProgramScope::areas`, any of them). The server's page picks nothing, so its link
is the program's (no part of the cache key changes). `node folia/e2e/programs.mjs` walks it.

### Exam dates the BTU cannot mean (`folia/crates/timetable/src/exam_reading.rs`, 2026-09-21)

Owner: Analysis I (11103) listed two exams „So 01:00–02:30, 27.12.2015" under WiSe 2026/27; the
page should correct that by itself and say that the data is odd. The raw QIS page says exactly
that (no time-zone bug of ours), but it is no typo: of 1,078 exam dates with a time in the
snapshot, 289 lie outside 06:00–22:00, and all follow one of two patterns. **262 × 01:00–02:30**
on a Sunday or without weekday is how QIS enters an exam without a fixed date („mündliche
Prüfung, Termin nach Vereinbarung", „IKMZ e-Klausur"); 207 of them carry a date nine to eleven
years before their semester (205 × 27.12.2015), 54 none. **27 × 23:45–24:00** is the day a term
paper or take-home exam is due. The first idea, shifting by twelve hours, would have turned all
289 into times nobody set. Owner decisions (2026-09-21, all four as recommended):

- The placeholder shows no weekday and no time: „Zeit offen", or „Termin offen" when its date is
  dropped as well — which it is where it lies outside the semester (±1 semester, by month: repeat
  exams reach up to 170 days past the end, none lies before the start). A date is never invented.
- A deadline reads „So bis 24:00", unmarked (it is a reading, not a doubt); the original is the
  row's tooltip.
- Anything else outside 06:00–22:00, an end before the start, or a date outside the semester keeps
  the source's value and is only marked (none in the snapshot of 2026-09-21).
- A marked row carries a line with the info icon: what QIS says where the row shows something
  else („In QIS: So 01:00–02:30 · 27.12.2015"), else what is odd. One note under the list explains
  the marks. Radix and the snapshot keep the entry as read (provenance names what was read); the
  reading is data (`ExamReading { stated, shown, reasons }`), not a string, so a later output
  (structured data, a calendar) can use the same one. The unmerged `seo-maxing` branch has its own
  `when::is_placeholder` (Sunday only) and deadline test (end ≥ 24:00); on a merge one of the two
  should call the other.

### The area picker and the plan's rows on the real data (2026-09-21)

The area picker and `areas_for_row` had been built against the synthetic snapshot only. Checked
against the real one (export of 2026-09-20 23:21, 182 programs, 179 of them with a module tree,
140 with a validated plan) with `folia/crates/pages/examples/area_survey.rs`, which opens a snapshot and calls
the crate's own functions — exactly what the app does — and prints every picker and every row:

    cargo run -p folia-pages --example area_survey -- <catalog-*.db> [slug…]

**Before** (heading = the node directly above, runs of equal headings, commit 42f004a): 10
headings that came twice in one picker, 194 headings over a single area, 66 headings that only
structure the tree („Grundstudium", „Gesamtkonto …", „Wahlpflichtmodule"). **After** (the rule in
„The areas of a program" above): 0, 0 and 0. 111 pickers have no heading at all, 42 one, 25 two
to seven, one (Umweltwissenschaften dual, six study directions twice) 13. Two pickers still show
two entries that read the same where they stand, both from the source: Maschinenbau dual 2018 has
two sibling nodes „Wahlpflichtmodule (STA)", Soziale Arbeit 2020 (double degree) two accounts that
differ only three levels up.

**Fixed or a choice** (`CatalogArea::choice`): of 1 614 areas with modules, 551 are fixed and not
offered, 1 063 are offered: 597 hold a module known to be elective (or FÜS), 395 are offered only
because no source gives any of their modules a kind — 16 programs have no kind in their whole tree
(Architektur M.Sc., the Orientierungsstudium, Wirtschaftsingenieurwesen M.Sc. 2025 …) — and 71 hold
nothing but fixed modules and one or more without a kind (Lehramt „Unterrichtsfach 1: Deutsch" 3
Pflicht + 1 unknown, Elektrotechnik 2018 „Ingenieurtechnische Module" 16 + 5). No area with a known
elective module is hidden. `stated_kind` along the path is already part of the module's kind (the
tree's label, `v_program_module_area.kind`), and the module pages' remarks are one of the sources
of `v_program_module.kind` (872 modules); nothing more is there to use without new sources. The
rule stays as the owner set it (R12: not known to be fixed is not fixed); whether an area of
mostly Pflicht with one unknown module should count as fixed is the owner's call (open).

**Rows of the plans** that name no module (1 059, counted once per plan caption): 348 are stated
Pflicht/Abschlussarbeit/Praktikum (one module), 152 name the FÜS (40 by their kind, 112 more by
their name only). Of the other 559, **175 now point at one area, 56 at several** (all of them
meant: a row listing areas, or two study directions of one plan) and 328 at none. Before: 166, 41
and 464 (the FÜS rows among them). Of the 328 without an area most name a single module that is not
linked („GT1-B 25102 Bau- und Stadtbaugeschichte 1", „Heritage Studies oder Heritage Studies
(Online)") or say nothing but „Wahlpflichtmodul 3" — both rightly get none, the panel then points
to every elective of the program. Checked by hand, the areas the rows now point at are right with
few exceptions: „Wahlbereich Volkwirtschaftliche Grundlagen" (a typo in the plan) finds every
„… Grundlagen" area of Wirtschaftsinformatik instead of „Volkswirtschaftliche Grundlagen", and
double-degree plans whose areas lie in accounts are not found. The real labels are the tests'
fixtures (`folia/crates/plans/src/area_fixtures.rs`: Informatik B.Sc. and M.Sc., Elektrotechnik B.Sc. 2022
with its study directions, Elektrotechnik M.Sc. 2018, Architektur, Wirtschaftsingenieurwesen
dual), and the fixture generator (`radix/internal/catalogbuild/folia_fixture_test.go`) builds its trees
and the plan of Informatik in the same shapes, so the browser checks see what the real data has.

Owner decisions and open questions (2026-09-21):
- Decided by the owner: no fixed area in the picker; never a heading twice; no structural node as
  a heading; for Informatik two sections, the own electives and the Anwendungs-/Nebenfach; a
  data-driven rule, no special case for one program.
- Taken as the default here, open to change: the heading is shown without „Komplex"; the areas
  without a heading come first, whatever the tree order; 12 areas as the length at which a section
  splits into its fields.
- **Open:** Praktische Mathematik — the owner put it among the own electives, the tree puts it in
  the Komplex Nebenfach, and the plan asks for it in a row of its own. The picker follows the tree;
  the row „Anwendungsfach" leaves it out because of its own row. Moving it would need a rule the
  data does not carry, or a curated exception.
- **Open:** areas of Pflicht modules with a single module of unknown kind (71) — offered now.

### The landing page and the map of the programs (2026-09-20)

`/` answers three questions at a glance (`folia/crates/home/src/home/mod.rs`): what this is (headline, „inoffiziell"
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
under the text below it. The figures (`dl.birch`) stand on the panel in the colours of the theme
(owner, 2026-09-22: „sollten sich mit dem Thema anpassen", only the colours; until then they stood
on `--bark`, light in both themes like the mark, a bright block on a dark page), one under the
other with thin rules, and on each rule a stroke from alternating edges, the strokes of the mark;
each number is set so large that all of them are about equally wide (`--em`, its width in units of
its size, from `figure_em`; a single digit grows only as large as three), quiet in weight and
colour (owner: „kleiner und etwas dezenter"). Under it a
**carousel**: the map, then screenshots of the catalog, a study plan and a module page
(`folia/assets/shots/*.webp`, light and dark, wide and phone, made by
`node folia/e2e/showcase-shots.mjs <base-url>`, embedded in the binary, served under `/assets/shots/`; run
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
it does not scroll, `:root:has(.map-dialog[open])`; the app builds it as the dialog first opens,
2026-10-02: some 2,200 elements nobody sees until then). On a wide screen (≥ 1100 px and wider than
7:5) the dialog is the 4:3 map as high as the screen allows with a column on its left: the head,
the legend (with the line for shared modules) and what is shown — the program, its faculty, its
five closest relatives with the number of shared modules, the link. Owner: the carousel and the dialog only have
to work in the app; without JavaScript the map shows and leads to the program overview. Every
screenshot is lazy with `loading` written before its address (the app sets attributes in order; an
image with an address and no `loading` yet is fetched at once), so only what shows is fetched,
never the hidden theme or the phone's pictures. Colour: washes of the faculties' palette behind the
pictures, the ways into the catalog with a soft hue each (`--t-*`), the faculties in their map
colours, the abilities in the accent's tint. The questions are an accordion in three groups, „Über
Betula", „Betula nutzen" and „Fürs Studium" (first semesters); the text stays in the page and in the
FAQPage data. Until 2026-09-28 a sidebar held the sections (the current one following the
scroll, `nav[data-spy]` in `enhance.js`) and the Datenstand as label | value rows (see below). The versions and Impressum and Datenschutz
moved into the ground at the end of every page on 2026-09-25 („The birch" below).

**What the page says of Betula itself (2026-09-28).** Owner: what is known of a tool is mostly what
its own site says, and asked to compare Betula with other tools, search engines and the assistants
built on them marked it down unfairly — no account was read as missing functions, and neither the
pages that work without JavaScript nor the study plans read from the regulations were noticed; so
the page has to sell the functions and the architecture, with a text after the questions „wo darüber
alles gesprochen wird", the flow through the app and every filter. What changed:
- **Nine abilities** instead of six (`abilities`, three rows of three from a container width of
  1180 px; with two a row the odd last one takes the whole row, so no cell stays empty): the plans
  read from the regulations, the Stundenplan, no account („Kein Konto, und nichts fehlt") and
  „Schnell, offline, auch ohne JavaScript" are abilities of their own. Two old texts said more than
  the app does and were corrected on the way: a module's page names what the module requires, not
  what it is required for, and the versions stand in the ground, not in the sidebar.
- **The questions** got a group „Betula nutzen" (finding a module, the Stundenplan, marking,
  several devices, without JavaScript, offline and as an app, English); „Kostet Betula etwas?" and
  „Warum gibt es kein Konto? Fehlt dadurch etwas?" are two questions now, and the second one
  answers it: the account is left out, not the functions.
- **„Betula im Detail"** after the questions (`#im-detail`;
  `pages/home/detail.rs`, its words in `i18n/home_detail.rs`). Its first version, eight chapters of
  text, was „so langweilig und einfach nur eine wall of text" (owner, the same day): it had to be
  interesting to look at for everybody without breaking Betula's look. Now a panel names the
  chapters (links in their tints), and each chapter is a panel of its own — its name in its tint
  over a headline, a line under it, four points — with a picture made of the app's own parts on a
  wash of the tint, laid out by its own width (container `picture`): the four ways from a question
  to the one module page; the filter panel as a board of its twelve groups in the panel's order,
  look and words (`catalog::Texts`, so a renamed filter is renamed there too), an example chosen
  (winter, English, no written exam: `detail::example`) with how many modules the catalog has for
  it — on a phone the groups of a first look and the others after „Alle Filter zeigen" (a checkbox,
  so with and without JavaScript). Its chips were links into the catalog, and a click on a filter
  left the start page (owner, 2026-09-29: „mach das so, dass die filter tatsächlich funktionieren
  und man dann unten sieht, wie viele module das selected hat und sich die angucken kann"). Now, in
  the browser app, every chip that filters by itself and the rows of the duration and the years
  switch the board's own selection in place, with the catalog panel's toggles
  (`pages::catalog::Toggle`: off, with, without); the number under the board follows
  (`queries::catalog_count` on the local copy), stays at the bottom of the view while the board is
  in it, and its button opens the catalog with that selection. What needs more than a click (a
  program, the Merkliste, the Stundenplan, a name, the slider) stays a picture; without the app all
  of it is, and no chip is a link. Then a regulation's PDF read into the plan's matrix, with how
  many of the current programs have a checked plan (`Overview::plans`); a week in the Stundenplan's
  own grid (`WeekGrid`) with a clash and the ways into a calendar; where a
  visitor's things live, and four figures; the site with and without JavaScript; the birch from the
  ground (Radix, its sources and how often it reads them) up the trunk (the Datenstand) into the
  season's crown (Folia); the catalog and a module on a phone (`--device`, its frame), lazy and in
  the theme shown. On a wide page the picture stands beside the words, every other one on the left,
  the board across the width under them. Four chapters end in a way on (the catalog, the programs,
  the Stundenplan, the privacy notice).
- **The structured data** names Betula as a `WebApplication`: its abilities as `featureList`, free
  (`offers` at 0 €, `isAccessibleForFree`), `browserRequirements` saying that it runs without
  JavaScript and offline with it.
- What a text quotes of the page — a button, a view, a filter, a way in, the values of a filter — is
  checked against the page's own words in every language (`i18n/home.rs` and `i18n/home_detail.rs`,
  their tests).

The map (`folia/crates/pages/src/graph.rs`): a dot per current program, a line where two curricula share
modules (Jaccard; modules of more than 40 programs are ignored), a force layout without
randomness. **The server lays it out once, when a snapshot is opened** (`Snapshot::open`,
event `snapshot.map_built`), for a 4:3 sheet (the carousel; 2:1 until 2026-09-21) and a tall one; nothing is laid out while a page
renders and nothing in the browser (owner decision). Server-rendered pages get it through context
(`data::ProgramMapHandle`), the browser app as `GET /api/map.json` (50 KB in brotli; `boot.js`
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
`node folia/e2e/home.mjs` covers it.

**No sidebar, and a way in for a first visit (2026-09-28).** Owner: the sidebar was „sehr
verwirrend" and made the page look odd; a new visitor has to find their way, and the onboarding
matters most — „organisch ohne Popup, einfach intuitiv". So:
- **No frame.** The start page is the one page without `ui::Frame` (R17): `div.page.home-page`
  is its page, in a scroll area of its own (`#page-scroll`, which „Nach oben" and the jumps
  follow; the ground is its end, „One scroll area"), and its
  panels stand in one column in the middle, at most 1600 px wide. Of the sidebar's parts the
  jumps to the sections are the foot of the way in now; the Datenstand was the ground's already
  (date, semester, source, at the end of every page), and the answer „Wie aktuell ist der
  Katalog?" says so instead of pointing at the sidebar.
- **As wide as still reads well** (owner, 2026-09-29: at 1280 px the column was „etwas zu
  restriktiv … wenn man das auf großen Bildschirmen aufmacht, dann hat man links und rechts
  wirklich noch zu viel Platz"; at 1600: „jetzt fühlt es sich sehr viel gewollter an"). Every text
  of the page keeps its own measure (the lead, the heads, the steps, the abilities, the answers);
  the one without, the points of a chapter of „Betula im Detail", is as wide as the chapter's
  lead at 1600 px, which is why the column stops there. A window of 1920 px still shows a strip
  of the wood on either side (130 px), one of 2560 px shows 450 px. The carousel's current
  picture is 52 % of the column, up to 832 px, so its neighbours still reach the panel's edges.
- **The way in** (`#loslegen`, `pages/home/start.rs`), right under the first panel: „So legst du
  los", three steps in the order a semester is planned — „Studiengang wählen", „Module finden und
  merken", „Stundenplan bauen" —, each a mark, its name, one line, one button and where the
  navigation keeps it, joined to the others through the marks by a line (side by side from a
  container width of 900 px, one under the other below it). „Jederzeit unter" and the rail's and
  the bottom bar's own icon and name (`t.app`), the Merkliste and the Stundenplan only with
  JavaScript, as the navigation has them: the navigation is learnt on the way, without a tour.
  The head's „Direkt suchen" (with its keys) goes into the search at the top for whoever looks for
  one module (`data-action="search"` in `enhance.js`; without JavaScript it opens the catalog).
  Its foot: „Erst einmal verstehen, was Betula ist?" and the jumps to „Was Betula kann", the
  questions and „Betula im Detail" (`data-action="jump"`, no history entry).
- **One click a step** (owner, the same day, on the first version, whose steps led to the pages
  where they are done: „Das sind hier 300 Schritte, um ans Ziel zu kommen. Ich will dass man bei
  der Einführung immer nur so ein Click pro Step braucht", and whoever knows the site goes
  straight on). Each step's one button does it or leads to where it is done:
  - „Studiengang wählen" (`start::ProgramPick`, the first panel's second button as well) opens a
    picker of all programs in place (the catalog's `Combobox`: type a few letters, Enter), and a
    pick is „Mein Studiengang" at once; the page stays and the way in goes on. Once a program is
    kept, and without JavaScript, the button is „Alle Studiengänge" (the program overview). The
    server's link carries both words, and the stylesheet shows the ones the app will
    (`html.js:not(.mine)`: „Studiengang wählen"), so they stay when the app takes over. The
    page's scroller takes an open picker away (`ClosePopups`, as the catalog's filter panel).
    Where its button has no room for its words (a phone of 320 px, in English; the way in's at
    360 px) they take a second line, as those of the other buttons do, instead of pushing the
    button out of the panel.
  - „Module deines Studiengangs": the catalog of that program (`MineResolved::catalog_href`),
    „Zum Katalog" without one.
  - „Fachsemester übernehmen": the Stundenplan with the program's Regelstudienplan ready to take
    over (`/studyplan?import=mine`), while nothing is planned; else „Zum Stundenplan".
- **It follows what this browser has done** (the app's alone, R9; the server's page is the one of
  a first visit): the program set as „Mein Studiengang" (while it is in the snapshot), a module on
  the Merkliste, a module in the Stundenplan each make their step done — a tick in the mark, the
  line to the next step in the accent, and in place of the navigation's items what was done: the
  program's name (leading to it, with its stored Studienrichtung, `program_href`), „5 Module
  gemerkt" (the Merkliste), „8 Module eingeplant". The first step not done is the next one
  (`aria-current="step"`): its mark and its button wear the accent. Only words and colours change,
  never a height, so nothing moves when the app takes over.
- **The figures on a birch** (owner, the same day: the figures „waren voll cool"; then, on a first
  version with a small birch of its own beside the text — crown and ground — that doubled the
  page's crown: „rechts am Rand so ein dickerer Birkenstamm und dann nach links die Stats"). A
  trunk of bark runs down the first panel's right edge, its whole height (`.hero-trunk`, 46 px;
  the marks of the bark are a mask in the stylesheet, as wide as the trunk and repeated down it),
  and the figures (modules, programs, faculties, the Termine of the current semester) hang to its
  left as tags on branches of two lengths, each thick where it leaves the trunk and thin at its
  tag (`dl.tree-figures`). Flat, as the mark is: the first trunk was shaded at its edges and the
  tags had shadows, which looked round („warum ist das Ding jetzt plötzlich 3D?"); the trunk is one
  tone now and the tags are outlined. Only from a container width of 700 px on: on a phone the
  trunk down the edge looked wrong and the figures took the height the way in needs, so there
  the first panel is its text alone (the figures stay in the HTML). The branch at the panel's side
  grows out of the trunk. It replaced the stack of bare numbers (`dl.birch`).
- **The wood** (owner, 2026-09-29, in place of the branches that grew out of eight panels, which
  did not fill the room beside a wide page well: „so ein Scherenschnitt im Hintergrund, so dass man
  quasi einen Birkenwald im Hintergrund sieht. Die Seitenleiste soll dabei frei bleiben"; of five
  directions in `folia/design/forest` — `node folia/design/forest/build.mjs` writes the page that compares them
  — „Option E, organisch und ohne Kronen"). Behind every page (first the start page; owner, the same day: „bring den Wald auf alle Seiten")
  stands a wood of birches cut out of
  the background's grey, a few steps darker (a few steps lighter on dark, where nothing is darker
  than the background): two layers, thin trees further back and fainter (`--wood-back`), the wood in
  front (`--wood-front`), with barks cut into the trunks, hanging twigs, the leaves of the season
  (none in winter), grass, a mushroom and a bird. It is `div.wood` (`ground::Wood`, beside the crown in the
  app's shell), fixed, beneath everything (the crown and the ground included), from the rail to the
  window's right edge and bottom; the rail stays free. Beside the start page's column it fills the
  room of a wide window; on every page it shows wherever the panels leave a gap. Its masks (`<season>-wood-back.svg`, `-front.svg`, 3400 × 1600, whole pixels, their
  path data relative and as short as it gets, `folia/design/forest/path.mjs`; beside each a brotli copy
  at quality 11, `.svg.br`, which `api::birch` sends to a browser that takes `br`: 57–82 KB a
  season, only fetched where they show) are drawn by `folia/design/forest/forest.mjs` from
  `folia/design/forest/wood.mjs`, the generator the comparison uses as well: the column's 1600 px in their
  middle (1280 until 2026-09-29; redrawn for the wider column from the same seeds, the trees beside
  it only moved out with its edges), 900 px of wood on either side, the trees walking outwards from
  the column's edges, so the ones next to it are the same on every screen and a wider one only
  adds trees further out. Under
  the column is wood as well, with its ground (owner, 2026-09-29: „in der Mitte muss auch Content
  sein, damit da keine Lücken sind"): the panels hide it, but where they leave a gap — between two
  of them, or under a column shorter than the one beside it — the wood goes on.
  The stylesheet lays the masks' middle under the column's and their bottom on the window's; when
  the ground comes up at the end of the page, the wood goes up with its edge (`enhance.js`, the same
  inset as the ground's), so the birches stand on it (owner, 2026-09-29: „dass der Footer die untere
  Kante anhebt, wenn er kommt"). A window taller than 1600 px sees the trunks fade out at their top:
  a gradient drawn into the masks, not one the stylesheet lays over them (R18). From the desktop's layout on
  (901 px); none on a phone, where the page scrolls with the window. The grey is mixed in OKLab: in OKLCH
  a grey's hue is `none`, and Chromium drew the mix reddish.
- **The first panel's trunk at night** (owner, the same day): in the dark theme the trunk and the
  branches of its figures are the moon's grey with dark marks (`--hero-bark`, `--hero-bark-ink`,
  `--hero-limb`); white bark glared there.
- New class names were checked against the stylesheet, the scripts and every page: `.steps` and
  `.step` are the chips of „Betula im Detail" (the way to a module, „Klausur"), so the way in is
  `.start-path`, `.start-steps`, `.start-step`, `.start-mark`, `.start-btn` and so on.

`node folia/e2e/home.mjs` checks the page without a sidebar, the birch of the figures and the wood behind it
(at 1920 px the column 1600 px wide in the middle, the masks drawn and sized for it), the
three steps and their buttons, a pick in the picker (the first step done, the others leading into
the program, the first panel's button „Alle Studiengänge", no page load), „Direkt suchen", a jump
of the foot, and the steps done for a browser with a program and a marked module; the server test
that there is no sidebar, that the first step is the next one, the two links of „Studiengang
wählen", the trunk and the wood (and that no branch is left).

### The birch: crown and ground (2026-09-25)

The owner's idea: leaves along the header, as if the view began where a birch's crown begins, and
at the end of every page the ground with the roots. Picked on a design canvas of concepts
(`folia/design/birch/`), in the owner's words where they decided:

- **The crown** is the edge of a crown as a cut-out silhouette in two tones („Scherenschnitt"; the
  leaves behind at half strength), at colour level 4 of 5, the leaves drawn realistically. It
  **follows the seasons** by the date: March to May yellow-green with yellow catkins, June to
  August green, September to November gold with falling leaves and seeds, December to February bare
  twigs with catkins. The script in `<head>` names the season (`data-season` on `<html>`), so the
  server's HTML stays the same for everybody (R9); without it the crown is summer's.
- **The ground** is the footer after all of a page, across everything right of the rail (the rail
  stays whole), with the roots as calm abstract lines, quieter than the leaves. Its colour: light,
  Lausitz sand with a little depth towards the bottom; dark, a cool depth from the tone of the
  panels to almost black („Tiefe") — the brown soil of the first draft was „ein absolutes No-Go"
  against the cool dark UI. It holds the wordmark with „Modulkatalog · inoffiziell", „Betula ist ein
  inoffizielles Projekt und gehört nicht zur BTU.", Impressum and Datenschutz, the versions of Folia
  and Radix (the roots: Radix brings the data; a snapshot without Radix's version leaves it out),
  the date of the data, the semester and the source. In autumn fallen leaves lie on its edge.
- **How it comes** („Kopfzeile bleibt, Tafeln werden kürzer", and after the first build: „der
  Content darf nur EINE Scrollbar haben", the rail must not move, the panel on the left is „nur
  abgeschnitten verkleinert"): on a wide screen every page is one scroll area with the ground as
  its end (below). Until 2026-09-29 the page scrolled inside the view and the window scrolled on
  for the ground once the page was at its end (`data-ground` on `<html>`, the view laid out for
  the ground once the window stood still, the page gliding to its end above it again); two steps
  that the owner found „unfassbar janky", so they are gone. Every panel ends 8 px above the ground,
  as above the window's edge. On a phone the ground follows the page (the app's own ground after
  the view, in the body's second row), full width, the bottom bar floating over its lower part, and
  the crown carries the frosted background of the bar at the top. A page shorter than the window
  does not leave the ground floating halfway up the screen (owner, 2026-09-26: „mindestens unten
  bündig"): the body is a column at least as high as the window (`100dvh`, the window with the
  browser's bars as they are), the view takes the room that is left, and the ground ends at the
  window's lower edge.
- **One scroll area** (owner, 2026-09-29: the two steps — first the page, then the window for the
  ground — felt „unfassbar janky"; decided on a prototype, variant b, „vom Aussehen alles so wie
  heute, bloß dass es sich besser anfühlt"). Every page flows (`.work.flowing`: the catalog, the
  Merkliste, `ui::Frame`, `ui::Plain` for a page without a frame, the start page): everything right of the rail
  under the top bar is one scroll area with one scrollbar at the window's right edge, and the page
  and the ground after it scroll in it natively, as one — the ground is simply the end of the page
  (`Ground` at the end of the page's `.work`; the app's own ground after the view is the phone's).
  It is plain CSS, so it works without `enhance.js` as well. Not the window: the content would run under the top bar, and the crown
  and the wood would have to be painted again there; the area ends under the top bar as the page
  did, and the wheel over the top bar and the rail does nothing, as before.
  - What stands beside the page is pinned: the filter panel, a frame's sidebar and the module
    beside the list or the page (a frame's `aside`) stand
    still, as high as the area shows, their place running through the ground's row, so the ground
    slides over their lower end instead of pushing them up. From 8 px above the ground down, all
    across, the background is the wood's front colour (owner, 2026-09-30, instead of the panels'
    ring and round corners drawn along with the ground, which looked off): a band behind the
    ground (`.work.flowing::after`) cuts the panels off straight and shows in the ground's round
    corners, and under the wood, as it goes up, its front colour goes on to the window's edge (a
    shadow of `.wood`). Once the area stands still (150 ms), the pinned panels' content ends above
    the ground (`--cover`, `enhance.js`), one layout nobody sees since the ground covers exactly
    what goes, and „Nach oben" stands above it; as the ground goes back down their content follows
    it in the same frame (owner, the same day: once the ground was gone the sidebar still waited
    for the scroll to end). Shrinking them with the scroll in every frame would lag behind the
    natively scrolled ground.
  - The scrollbar stands in the 8 px right of the page (its width is `--bar`, `enhance.js`), not
    beside them (owner, 2026-09-30: the gap to the scrollbar was too wide).
  - The list keeps its box: its head (the number, the filters in force) stays at the top, the
    heads of its columns under it (`--list-head-h`, measured by `enhance.js` with its fraction),
    the head's ring in the area's 1 px above the panels, where theirs are (owner, 2026-09-30: a
    pixel lower, rows showed through above it), the note of a
    semester between them scrolls away as before. Where the area cuts the list off at the bottom a
    sticky edge (`.list-cap`) draws the ring and the round corners; at the real end of the list it
    stands where the list ends and draws what the list draws there. The round corners of the head
    let the page behind show through a mask on the area, which stands still while its content
    scrolls. The virtual list follows the area (`nav::list_viewport`, `nav::watch_scroll`).
    A list short enough to stand whole above the ground (a search with a few hits) is pinned like
    the filter panel (`.short`, `enhance.js`): its rows stay where they are when the ground comes,
    as before, and the ground slides over its empty end.
  - A page of panels (a frame's page, the start page) flows in its column and is cut straight at
    the top, as before; its end comes 8 px above the ground. The Studienplan's fold measures
    itself by what the area shows (`100vh - 64px`), not by the page, which is as tall as its
    content. Tab from a page's last link goes into the ground, which the browser scrolls into
    view, and then to „Nach oben".
  - The wood goes up with the ground in the same frame: a scroll-driven animation on the area's
    timeline (`--page`, the last 208 px of the scroll); where a browser has none, `enhance.js`
    moves it. Less motion keeps it (owner, 2026-09-30: it stayed down): it moves only as the
    visitor scrolls.

The pieces: `folia/crates/shell/src/ground.rs` (`Crown`, `Ground`; the ground's data is `pages::ground`, the meta
and the current semester), „the birch" in `folia/assets/app.css`, the ground's behaviour in
`folia/assets/enhance.js`. The crown runs along the whole top on every screen (owner, the same
evening: „durchgehend und auf allen Geräten"; the first build hung only where nothing stood — at
the mark, at the end of the title column, right of the search — and fell apart into clumps with a
thin edge between them and a gap on wide screens). Two masks per season (`folia/assets/birch/`): a
tile of 1200 px that repeats to the right edge, dense at the top and hanging deeper and shallower in
long waves (never below 46 px, so nothing hangs out under the search), and at the left end a head of
420 px: the mark, and over the title a clearing where only the crown's edge hangs in, the twigs at
its sides leaning away („ein natürlicher Platz für die Schrift", as the name had it before). The
head ends in the tile's own end, and the tile is cut to its box from 420 px on (`mask-clip:
content-box`), so the seam does not show and nothing is drawn twice. A phone has no title in its
bar and shows the tile alone. The mark and the search stand in front of the crown and cover it.
The masks are coloured by tokens (`--crown`, `--crown-ck` for spring's catkins, per season and
theme), so the same files serve light and dark. `node folia/design/birch/birch.mjs` draws all
masks, the roots and the leaf litter again (deterministic, seeded); the server serves them under
`/assets/birch/` (`api::birch`), and a server test fetches every mask the stylesheet names.
`node folia/e2e/ground.mjs` drives it with a real wheel. The link-preview cards hang the same crown from
their top (see „Search engines"), with heads of their own that `birch.mjs` draws beside the site's
(`<season>-card-head.svg` for the cards the server draws, not served; `<season>-og-head.svg` for the
standard picture): the same twigs and seeds, only the clearing where the card's wordmark stands.

### Search engines (`folia/crates/shell/src/seo.rs`, 2026-09-20)

Aim: a search for a module or a program of the BTU finds the page here. What that rests on:

- **Every page states itself once** with `seo::Seo` (inside its frame): description, canonical
  address, Open Graph tags (picture: `folia/assets/og-<season>.png`, made from `folia/design/og/og.html`) and
  structured data. Nothing of it is set for the whole app. (Before, every page carried the app's
  default description, program pages a second one, and the module page lost its own.)
- **One address per page.** Filters and the preview of the catalog, and a filtered program
  overview are views: `noindex, follow`. `/programs/<slug>` names `/programs/<slug>/plan` as its
  address. Older examination regulations are `noindex`, and so is a program's „Mein Plan“
  (`…/my-plan`, the visitor's: `ProgramTab::indexed`).
- **Every module and every plan is reached through pages that are listed** (2026-09-26): the pages
  of the unfiltered catalog (`/catalog?page=<n>`, with „Seite n" in the title) have addresses of
  their own and are indexed — they are the way to a module that no program's page links, and a
  search engine stops following the links of a page it is told not to list; a page past the last
  stays `noindex`. The plan of every study direction has its own address (`?variant=<n>`, see
  „Routes"). The cache counts both as pages, not as views (`cache::listed`).
- **A crawler is led to pages, never to views** (2026-09-30: Googlebot had fetched 250,000
  addresses, walking the filters — every filtered list links more filters, orders and pages).
  `folia_routes::url::listed` says what a page is, by its address: one without a query, a further page
  of the unfiltered catalog (`/catalog?page=<n>`; `page` comes after every filter, so
  `/catalog?turnus=winter&page=2` is a view) and the plan of a further study direction
  (`…/plan?variant=<n>`); not the Merkliste, the Stundenplan or „Mein Plan“, which are the
  visitor's. Every link of the server's HTML to anything else carries `rel="nofollow"`: the toggles
  and the orders of the lists, the tags that take a filter away, the pager of a filtered list, the
  catalog narrowed down to a program or an area, the examples on the start page, the Merkliste and
  the Stundenplan in the navigation, the language switch on a view, a program's other examination
  regulations from its „Mein Plan“ (they keep the view, and lead to theirs; from the plan they are
  followed) (`seo::nofollow` where the target decides). The pager of the unfiltered catalog and the
  plans of the study directions are followed. `nofollow` is a hint, and a crawler keeps asking for
  the addresses it already knows, so **robots.txt** closes the views of the lists as well, as
  Google advises for filters: `Disallow: /catalog?`, `/programs?` and `/bookmarks?` in every
  language, with `Allow: /catalog?page=` (the longer rule wins) and `Disallow: /catalog?page=*&`
  again for a page with a filter or a preview behind it. The Stundenplan's query stays open (a
  shared plan, `?share=`, is a page for link previews), and so do calendar feeds, cards and the
  sitemap. The link previews of X, LinkedIn and Facebook read robots.txt too and fetch only what is
  shared, so their group disallows `/api/` alone: a filtered list shared there keeps its card.
  `crawlers_are_led_to_pages_and_kept_out_of_views` (server) reads robots.txt as Google does and
  fails on any link of the site's pages that a crawler may follow to a view.
- **Titles start with what people search for**: „<Modultitel> (<Nummer>) · Modul der BTU
  Cottbus-Senftenberg · Betula", „<Studiengang> (<Abschluss>): Regelstudienplan · BTU
  Cottbus-Senftenberg · Betula" (each view of a program has its own title).
- **Structured data states only what the page shows:** `WebSite` with its search, `WebApplication`
  with the abilities the page lists (2026-09-28, „The landing page" above) and `FAQPage` on the
  landing page, `Course` and `BreadcrumbList` on a module, `EducationalOccupationalProgram`
  and `BreadcrumbList` on a program. Since 2026-09-26 it carries what search engines answer
  questions with (owner: Google's AI answers should know when a module's Termine and exams are and
  in which semester a plan places it — as much as sensible, without bloating the page):
  - the `Course` of a module: code, credits (`QuantitativeValue`), language, provider, `sameAs`
    the BTU's page, the modules it requires (`coursePrerequisites`), the semester each validated
    plan places it in (`educationalAlignment`: the plan is the framework, „1. Semester" the
    level), and its Termine as `hasCourseInstance`: a `CourseInstance` per semester the page
    shows, with a `Schedule` per slot of the week (days, times, first and last date, `P1W`/`P2W`,
    `Europe/Berlin`) and the exam dates as `EducationEvent`s (`subEvent`) with the day's offset
    (`+01:00`/`+02:00`, `folia_calendar::day::berlin_offset`). A Termin without a time of the
    week has no slot; an exam date the page marks (QIS's placeholder, a doubtful time) is not
    stated. Exams of another semester than the teaching are an instance of their own.
  - the program: degree, the semesters and credits its validated plans agree on
    (`timeToComplete`, `numberOfCredits`; left out where the plans differ) and, on the plan's
    view, the modules of the plan shown (`hasCourse`, each by the address of its page). One `@id`
    for both views: the plan's address.
  - No rich result comes of any of it in German any more (Course info ended in June 2025, the
    course list is English only, event results take events the public can book): it is for the
    index and the answers built on it. validator.schema.org: no errors, no warnings (fixture of
    2026-09-26). What it costs: a module with two lectures, two groups and two exams +2.4 kB,
    +0.5 kB compressed; one with thirteen Termine +4.7 kB, +0.5 kB compressed; a plan +1 kB.
- **The facts are text first.** What the answers of a search engine quote is the page's text, so
  it says them plainly: a module's sections are headings (`h3.label` under the title's `h2`,
  styled as the labels were), a day in a Termin's line is `<time datetime="2027-02-15">`, and
  „Studiengänge" names the semester each validated plan places the module in („PO 2008 ·
  1. Semester · …"; „5.–6. Semester" for a span, „4. oder 5. Semester" where the study directions
  differ; `v_program_plan_entry` by module, `queries::module_plan_places`; `plan_semester` of
  `v_program_module` is the smallest exact semester and has none for a span). That line wraps
  in the room the name leaves (`contain: inline-size`), so the kind at the right end stays whole.
- **The server's page is what crawlers index**, also those that run JavaScript: `robots.txt`
  disallows `/api/`, so a crawler's renderer gets no `/api/status`, `boot.js` does not start the
  browser app, and the page stays as the server wrote it. Without that line Googlebot could
  download the catalog and let the app replace the page (`start()` empties the body and the
  head's tags).
- **`/sitemap.xml`**: the three entrances, every module that has a page, every current program
  with its plan (and the plan of each further study direction) and its areas; `robots.txt` names it. Addresses
  are absolute and use `--public-url` (`SiteUrl` in the app; the browser app uses its own origin).
  **`lastmod`** (2026-09-26, `folia/crates/server/src/lastmod.rs`): when the page last said something new, so
  that a search engine fetches again the pages whose Termine or exam dates changed and not the
  thousands that did not. The warm-up renders every page once per snapshot and notes a fingerprint
  of its `<main>` — without the build in its addresses and without the line of when the source was
  fetched, which change although the page says nothing new; where it changed, the page's date is
  the snapshot's `data_changed_at` (the same for every instance, so two colours agree). The record
  is `lastmod.json` in the data directory: a restart or a deploy keeps it. The sitemap is made anew
  after each round of the warm-up (its ETag is its content's); without the warm-up it names no
  dates. Measured on a fixture: a newer snapshot in which one exam moved gave that module's page
  and the landing page („Zuletzt geändert") a new date, the 73 other pages kept theirs.
- The browser app removes the server's tags from the head when it takes over and writes its own,
  so the head describes the page that is shown. What is the same on every page (the stylesheet,
  the preloaded font, the icons) is part of the document (`app::shell`) and never written by the
  app: it mounts fresh instead of hydrating, so a `leptos_meta` tag in `App` lands in the head a
  second time. Two copies of `app.css`, one from the service worker and one from the network,
  once mixed an old sheet into a new one (2026-09-21); `home.mjs` and `pwa.mjs` count them.
- **Link previews** (messengers, Slack, Discord, X): the card is the page's own title and
  description with a picture (1200 × 630, absolute address, with type, size and alt text); X
  gets its `twitter:` twins, because only with them the large card shows everywhere. A preview
  never runs JavaScript, so all of it is in the server's HTML.
- **A module and a program have their own picture** (`/cards/module/<id>.png`,
  `/cards/program/<slug>.png`; `seo::module_card`, `seo::program_card`): the logo, the kind and
  number, the title (four sizes, at most four lines, then „…"), a line of facts and a quieter
  one (department; size of the curriculum). So do the Merkliste and the Stundenplan
  (`/cards/bookmarks.png`, `/cards/studyplan.png`; owner, 2026-09-26): what the page is, the same
  for everybody, since what a visitor keeps lives in the browser. A Stundenplan handed on by a link
  (R20) has its own (`/cards/studyplan/<code>.png`): its modules as tags in the plan's tones, by
  the names its week grid gives them (`StudyplanData::slot_names`: „MIT-1", „AuP"; three sizes,
  then „+3"), how many, their credits and the program, and their titles as the quiet line, cut
  between two titles. Every other page names the standard picture `og.png`.
- **The birch's crown hangs along the top of every card** (owner, 2026-09-26), in the season the
  card is drawn in (`birch::Season`: the months the site's script goes by, on the server's clock),
  half as large again as along the site's top, since a preview is seen small. The card has a head
  of its own (`<season>-card-head.svg`): its clearing is just as wide as the wordmark („viel zu
  weit und links etwas daneben" was the site's head there), and the mark stands in front of the
  crown before it, as the rail's does. The season is part of what a card's hash is made of, so a
  card is drawn anew, under a new ETag, when the season turns. The grey, the face, the crown and
  the face's hairline are drawn once per season and every card onto a copy: the crown's 1,500
  leaves took two thirds of a card's time. The standard picture is `folia/design/og/og.html` in the four
  seasons (`folia/assets/og-<season>.png`, with its own head, `<season>-og-head.svg`, fitted to its
  larger wordmark); `/assets/og.png` answers with the season's, the season in its ETag.
- **The server draws the cards itself** (`folia/crates/server/src/cards.rs`): an SVG put together in Rust, set
  by resvg in static cuts of Inter (`folia/crates/server/assets/inter-*.ttf`, cut once from the app's variable
  font by `folia/design/cards/make-fonts.py`, a development tool; nothing but the server's own process
  runs in production), measured with the same shaper before it is set, written as a palette PNG
  (40–50 kB with the crown; a card of tags, whose eight tones with their grounds a palette does
  not hold, in true colour, about 80 kB). About 70–120 ms a card in a dev build, the first of a
  season some 230 ms (its ground); the pixel crates are optimised in the dev profile for that.
  - **Kept:** a finished card stays in memory (`--card-cache-mb`, 64 MiB ≈ 2,500 cards) under
    the hash of what it says, so a new snapshot only redraws the cards whose text changed. The
    hash is also the ETag (`If-None-Match` → 304); `Cache-Control: public, max-age=86400`.
  - **Never in the way of the pages:** drawing runs on the blocking pool, at most half the
    processors at once (1–4). If every place is taken, or there is no snapshot, or drawing
    fails, the answer is the season's `og.png` at once with `no-store`, so the next fetch gets the
    real card. An unknown module or program is a 404, and so is a shared plan's code that does not
    decode or names no module the catalog knows. Look at the design with
    `FOLIA_CARD_OUT=<dir> cargo test -p folia-server cards_for_review` (`FOLIA_CARD_SEASON=spring`
    for another season than today's).
- **Who the site is, outside a page** (static in the document's head, `app::shell`, so it
  survives the takeover): `/favicon.ico` (32 and 48 px) and the SVG icon, both the mark;
  `/apple-touch-icon.png` (180 px, full bleed: iOS rounds it and uses it for the home screen and
  for previews in Messages) and `/manifest.webmanifest` (name, colours, icons 192/512, maskable
  ones of 512 and 1024 px and a monochrome one), all the icon of the installed app; and
  `theme-color` (the page background; the head script and the theme switch turn it dark). The
  pictures are made from the mark's grids and from `folia/design/logo/app-icon.mjs` by
  `node folia/design/logo/render-icons.mjs`. The manifest makes the site installable; the service
  worker makes it start without a network.
  - **Google Search** (2026-10-01) shows one picture per host beside the results, taken from the
    links of the start page (`icon`, `apple-touch-icon`). It reads no SVG (BMP, GIF, ICO, PNG,
    JPEG, PPM, TIFF; square, larger than 48 px recommended) and does not document how it chooses
    among several; as far as can be seen, the largest. So the start page links the icon of the
    app once more, as an `icon` of 192 px (`app::ICON_192`, square and a multiple of 48): larger
    than the mark's ICO (48 px at most) and than `apple-touch-icon.png` (180), it is the largest
    of either kind of link (`folia/crates/server/src/tests.rs` checks that). The tab keeps the mark: Chromium
    takes the SVG and does not even fetch the PNG (tried at 1×, 2× and 3×), and Firefox takes the
    SVG whatever else is linked (`selectIcons` in its `FaviconLoader.sys.mjs`).
  - **Why Google showed none** (2026-10-01): until 2026-09-27 `betula.app` was the placeholder
    (`deploy/stacks/placeholder.yml`): `noindex, nofollow`, no icon linked, `/favicon.ico` a 204
    without content. Google takes the icon anew only when it processes the start page again,
    which takes days to weeks; four days after the opening its favicon service had the leaf for
    `/en` and `/en/catalog` (first seen after the opening) and nothing for `/`. After a deploy that
    changes the icon: Search Console, URL Inspection of `https://betula.app/`, „Request indexing".
    What Google's favicon service has for an address:
    `https://t3.gstatic.com/faviconV2?client=SOCIAL&type=FAVICON&fallback_opts=TYPE,SIZE,URL&url=https://betula.app/&size=64`
    (a 404 means none).
  - **Android** (2026-09-26) cuts the maskable icon into the launcher's shape and shows its middle
    87 % (Chromium pads the web's safe circle of 80 % onto Android's, 66 of 108 dp; the mask shows
    72): the leaf is drawn smaller by that much and sits in the middle, so a circle, a squircle
    or a teardrop shows the whole of it. With themed icons (Material You) the launcher tints the
    monochrome icon, the leaf with its marks cut out, in the colours of the wallpaper; without it
    the icon of an installed app did not take part. The splash screen of the installed app is
    Android's own: the manifest's background (the light page, `#f1f2f4`) with the maskable icon in
    its middle, drawn at about 220 dp, which the 1024 px icon keeps sharp.
  - **iOS** draws no splash screen for a web app: it shows the picture the page names for exactly
    its screen, orientation and colour scheme (`apple-touch-startup-image`), and without one a
    blank screen until the page is drawn. The head script names the pictures of the screen it runs
    on (only where `navigator.standalone` exists, that is on iOS: two on a phone, upright, light
    and dark; four on a tablet, turned too), and iOS takes them when the app is added to the home
    screen. Sixty `<link>`s for all screens in every page would have cost every visitor almost a
    kilobyte for what only a home screen of iOS reads. The server draws them
    (`/assets/launch/<width>x<height>[-dark].png`, `folia/crates/server/src/launch.rs`, resvg like the cards)
    for the screens of `app::launch` (12 iPhones, 9 iPads): the icon of the installed app in the
    middle of the page's background, as large as twice an icon of the home screen, on a soft
    shadow in the light, the wordmark and „Modulkatalog · inoffiziell" at the bottom; on its first
    request, one at a time, and keeps them (20–40 kB each, 2 MB for all). The gate lets them
    through like the icons. A screen that is not listed gets no picture and starts blank as before:
    a new iPhone is one line in `folia/crates/shell/src/launch.rs`. Look at all of them with
    `FOLIA_LAUNCH_OUT=<dir> cargo test -p folia-server launch_screens_for_review`.
- **Offline (`folia/assets/sw.js`, served as `/sw.js`, registered by `boot.js`; 2026-09-21):** the
  worker keeps the shell of the app — a page of the site (the browser app renders whatever the
  address names from the local catalog), the scripts, the styles, the bundle, the font, the icons,
  the manifest — and nothing of the data: the catalog is in IndexedDB, where `boot.js` keeps it,
  and `/api/*` is never intercepted. Pages come from the network first and are kept for the way
  back (sixty of them); offline, the kept page, else the shell. Assets come from the cache first.
  The semantic search's model (`/models/e5-de-en-<hash>.bin`, 15 MB; `folia/crates/semantic/README.md`, „In
  the app") is kept apart, in `betula-models`, which no build drops: its address names its
  content, so a deploy does not download it again, and a new model replaces the old one.
  The server writes its build into the worker, so a new build installs a new worker, which caches
  the new shell and drops the old one; the worker's own file is revalidated on every use, whatever
  its address. **A page and its files always come from one build** (2026-09-21): the document
  links the stylesheet and the scripts with the build of the server that wrote it
  (`/assets/app.css?v=<build>`, `app::BuildId`, the login page of closed testing too), `boot.js`
  asks for the bundle and sql.js with the same `?v=`, and the worker keeps these files under
  exactly these addresses. Before, the first load after a deploy was answered by the old worker:
  the page from the network (new markup), the stylesheet, the scripts and the bundle from its
  cache (old); only the next load was consistent. Now the new page asks for `?v=<new build>`,
  which the old worker does not have, so the network answers. And a worker keeps only what its
  own build answered: every answer of the server names its build in the header `x-build`, so the
  old worker does not keep a page of the new build (offline it would show the new markup with
  its old files: `deploy.mjs` caught exactly that), and kept pages are per build. Pages and the
  files of an install are asked of the server (`cache: "no-cache"`), never taken from the
  browser's HTTP cache, which may hold a page of an older build. The install skips only a file
  the server does not have (a bundle not built yet, 404); no network, an error or an answer of
  another build (the server moved on) fails it, and the worker in charge keeps its whole shell
  until the next load tries again. The font, the icons and the manifest
  keep plain addresses: they do not change with a build. The build is the version plus the
  start time of the process, so a restart is a new build and a returning visitor loads the shell
  once more, the bundle included; to the nanosecond (2026-09-30), since two colours of the site
  that start in the same second when the host boots must not name their different files alike.
  **Under the address of its own build a file is `immutable`** (2026-09-30, `api::Keep`): kept a
  year and never asked for again, since nothing the process serves there changes while it runs
  and a new build is a new address, which only the page names (and the page is asked for every
  time). The server answers every `?v=` with the file it has, so under any other build's address
  (a page of the old build after a deploy, the other colour during a switch) the file is
  `no-cache`, and so are the addresses without a build (the font, the icons, the manifest, the
  masks of the birch) and the worker, whatever it is asked with. Until 2026-09-30 every file was
  `no-cache`, and every page load asked for each again. After building the app again
  (`build-client.sh`), start the server again as well: the bundle is kept under the address of
  its build, in the worker as in the browser's cache. Not so under `folia/scripts/dev.sh`
  (`--live-assets`): there nothing is immutable and the worker keeps nothing („Working on the
  site").
  `folia/e2e/deploy.mjs` plays a deploy with two builds whose stylesheets differ. `boot.js` finds
  `/api/status` unreachable offline and simply opens the copy it has, unless that copy is of an
  older schema than the build reads: then the app does not start, the page stays the one the
  worker kept, and the status says „Offline – die Daten werden neu geladen, sobald du online
  bist" (the only case in which a failed start says anything). Once the app runs it says
  nothing: the „Offline bereit" notice is gone (owner, 2026-09-21: „wenn es einfach
  funktioniert, dann passt das"); only the loading of the data on a first visit is announced. `folia/e2e/pwa.mjs` cuts the network and loads pages afresh; `folia/e2e/schema.mjs` plants a copy of an older schema and starts with and without a network, and one that cannot be read.

Not done: submitting the sitemap to the search consoles (needs the owner's accounts), English
pages.

## 2. Rules

R1–R8 from `docs/history/frontend-rewrite.md` §5 apply. In short: navigation state reaches pages as
plain values from the router; nothing page-owned is read after unmount; no panics (`unwrap`,
`expect`, indexing and `panic!` are denied by clippy in all three crates); filter state = the
URL, UI state never triggers a query; keyed lists; one source of truth; design tokens only, no
inline styles; keyboard and phone usable. Added in phase 0/1:

- **R9. Server HTML is user-independent.** Merkliste, Studienplan, Mein Studiengang and the
  finder switch („Passt in meinen Stundenplan") follow the Merkliste: they live in the browser, the
  server renders them empty, and they are applied after the takeover, never during the first
  render; so do the passed modules of „Mein Studium".
- **R10. Shortcuts are written next to their button** (`kbd`): Esc closes the filter sheet or the
  module preview and, on a module's own page, goes back to where the visitor came from; `F` opens
  the previewed module full screen; ↑/↓ move through the list (rows are links, so this is just
  focus) and Enter opens the selected row; Ctrl+K or `/` jumps to the search. Every view is a real
  history entry, so the browser's back always works too.
- **R13. Personal view settings never go into the URL**: theme and the widths of the filter panel
  and the preview live in `localStorage` and are applied before the first paint by the script in
  `<head>`. So does the language (`betula.language`, `docs/folia/i18n.md`): the first script of the head
  opens the page in it, which is a page load, because a language is an address.
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
  that at 60 fps (`node folia/e2e/resize-perf.mjs` measures frame times on every kind of page):
  - No layout whose cost explodes with its width: the program overview set in CSS text columns
    had to rebalance all columns with every pixel and stuttered; the matrix of grid rows does not.
  - Long lists do not lay out what is off screen: `content-visibility: auto` on the catalog's
    rows (with 350 rows loaded: 58 of 138 frames over 33 ms before, 1 after). The rows are direct
    children of the scrolling list, so this works per row; the last height is remembered.
  - Nothing is drawn again that the width does not change. Every write restyles every element,
    and Chromium makes a gradient in a background or a mask, and an image taken out of a custom
    property, anew with every restyle, and then draws them again: the wood behind the page (its
    fade was a `linear-gradient` over its masks) and the crown (its masks came in `--crown-head`,
    `--crown-mask`) were drawn again in every frame of a drag, most of its raster work. Their masks
    now stand in the rules as they are and the wood's fade is drawn into its masks
    (`folia/design/forest/forest.mjs`): the catalog's drag, headless without a GPU, rasterises 0.3 s
    instead of 4.2 s. The ground's gradient is left, small and below the window's edge.
  - A safety net for pages or machines that still cannot keep up: after three frames in a row
    over budget, the rest of that drag moves only the panel (inline width, above its neighbour)
    and the property is written when the handle is let go (`data-resize-mode`, and
    `data-resize-budget`, with which `filters.mjs` forces either mode: whether a page keeps up
    depends on the machine, what the two modes do does not; headless on four cores without a GPU,
    the catalog falls back after a few frames).
- **R17. Every page is framed** by `ui::Frame` (see „Look and interaction"). A new page starts
  with the question what its sidebar holds, not whether it has one. The one exception is the
  landing page (owner, 2026-09-28: its sidebar confused a first visit; „The landing page").
- **R16. In one reactive closure read the source, not a memo derived from it and the source.**
  reactive_graph 0.2.14 does not mark the observer that made a memo recompute as dirty. A closure
  that reads `state` (a memo derived from `query`) and then `query` therefore misses a change of
  `query` whenever `state` keeps its value: checking `state` recomputes `query`, `state` reports
  „unchanged", and `query` is clean by the time it is asked. It hits the first such closure of a
  page only (for the others the recomputation has already happened), which made the first toggle
  of the filter panel drop the rest of the filter. `folia/e2e/filters.mjs` checks every link of the
  panel against the whole filter.
- **R15. What needs JavaScript is not shown without it:** shortcut hints, resize handles, the
  slider, the theme switch. Wrap it in `ui::JsOnly` (or give a single element the class
  `js-only`; `ui::Shortcut` does it for `kbd`). The parts stay in the HTML, because the server
  sends the same cached page to everybody (R9); the stylesheet hides them until the script in
  `<head>` has marked the document, which is before the first paint. What only the browser app
  can do (endless list, pickers) is rendered by the app alone or shown under `html.app`.
- **R11. All SQL lives in `folia/crates/query/src/lib.rs`,** reads only `v_*` views, and every `pub fn`
  there runs against a real snapshot in the tests (the build fails otherwise). One exception, on
  purpose: the nodes of the module tree (`queries::program_area_tree`) come from the table
  `program_area`, because no view has the nodes that hold no module („Komplex Nebenfach") with
  their `parent_id`; a view `v_program_area` needs a Radix migration and a new export (owed).
- **R12. Unknown stays unknown:** `Option` in the row structs, „nicht angegeben" on the page.
  A code without a label is shown as it is (`labels::Code`), and the label test flags it.
- **R20. What a visitor keeps stays with the visitor** (owner decision 2026-09-20: no data of a
  visitor on the server). Marked modules, and whatever follows them (passed modules, the own
  program), live in `localStorage` under a versioned key (`betula.<what>.v<n>`), behind one
  store per kind that is provided through context and is empty on the server
  (`bookmarks::Bookmarks`). They never become part of a URL (URLs are requested from the server
  and end up in its logs), of server HTML (R9) or of a request; a URL may carry how such data is
  shown, never the data. What is read from storage is checked like what comes from a URL.
  The privacy notice lists what a browser keeps („Speicher im Browser" in
  `folia/crates/home/src/legal.rs`): a new store, or a new way for stored data into an address, is a
  change of that text too. `folia/e2e/bookmarks.mjs` watches every request of a session for marks.
  Exceptions, decided by the owner (2026-09-23/24/25): the address of a calendar subscription
  (`/calendar/<code>.ics`, `folia_calendar::subscription`) carries the semester, the planned
  modules, what is hidden or chosen (kinds, events, Termine, the Standort) and the program whose
  abbreviations name the modules in its entries (the plan's, else „Mein Studiengang"; 2026-09-25)
  as a `pack` code of kind `calendar`; the server resolves it anew on every fetch and keeps
  nothing; Folia's own log writes `/calendar/….ics`; the edge's access log keeps the address like
  every address, 7 days in the monitoring and 7 days in the host's journal. And „Mein
  Studiengang" (kept in the browser as `program.id`, in no other address) may stand in a catalog
  address as a chosen program does (`/catalog?program=<slug>`), only where the catalog is filtered
  by it — the catalog tab's first entry of a session; the app never carries it along into other
  addresses and never re-adds it once removed. Like the Merkliste's `open`, the Studienplan's
  address names the one module and Termin shown beside it (`open`, `row`), and the catalog's
  app-only `fill=p<n>` names a placeholder by its local number (it says nothing about the visitor;
  the server drops it) — never a list of what is planned. And a Stundenplan handed on by a link
  (owner, 2026-09-26: the link preview of a shared Stundenplan shows its modules, „MIT-1, AuP,
  EEG"; a preview runs no JavaScript and has no storage, so only the address can carry them):
  `/studyplan?share=<code>` carries the semester, the planned modules in their order and the
  program whose abbreviations name them, as a `pack` code of kind `studyplan`
  (`folia_calendar::share`), nothing hidden or chosen. The visitor makes it („Link zum Teilen
  kopieren" in the sidebar's group „Plan"), whoever opens it is offered the modules to take over
  (`folia/crates/planner/src/studyplan/share.rs`), and the server's page names them in its tags and its
  picture (`/cards/studyplan/<code>.png`), resolving the code anew on every request and keeping
  nothing; Folia's own log writes the page's path without its query and the picture's as
  `/cards/studyplan/….png`; the edge's access log keeps the address like every address. The
  privacy notice's part is „Stundenplan teilen".
- **R21. A click answers in the next frame** (2026-09-23, „A click answers first"). What the
  visitor starts goes through `Pending` — links do by themselves; a handler that navigates calls
  `Pending::go`, not the router's `navigate` (that is for what the app does on its own). A
  control that shows where the visitor is shows where the app is going as well (`Pending::to`,
  `search_on`, `path`), and what takes a new page or list to compute has its skeleton
  (`Pending::waits`, `skeleton`). A new page gets a `pending::Shape`, or one that looks like it.
- **R23. What the app writes is in the page's language** (2026-09-27, `docs/folia/i18n.md`). A text of
  the app lives in a group of texts (`folia/crates/<crate>/src/i18n/<group>.rs`, a field and one
  `const` per language, in the crate of its part), never as a literal in a view; a component takes them once (`let t = i18n::t();`).
  Every address of the app written into a page goes through `t.path(…)`, and what reads an
  address uses `i18n::use_location` (the path without the language's prefix). The data is not
  translated: titles, descriptions and names are shown as the BTU writes them. A server test
  renders every kind of page in English and lists every link out of English
  (`a_page_in_english_stays_in_english`).
- **R22. A render on the server makes nothing that is bound to its thread** (2026-09-26). A
  page renders in a task that can go on on another worker thread: `leptos_meta` waits a tick
  for its tags (a little task of its own that wakes the render again), and after three such
  hand-overs in a row tokio puts the woken task where an idle worker can take it over
  (`MAX_LIFO_POLLS_PER_TICK`). The page's reactive owner is cleaned up at the end of the render,
  on whatever thread that is, and a value made with `StoredValue::new_local` or
  `RwSignal::new_local` (anything kept in a `SendWrapper`) panics when it is dropped on another
  thread than it was made on. `Pending` kept the router's navigation (an `Rc`) in such a value.
  The warm-up, which renders page after page in one task, died on it after 307 of 5,235 pages in
  one run and 515 in the next: a race, within a few hundred pages under load, while on an idle
  workstation a run could get through. A visitor's render is handed over once and stayed on its
  thread (none of 5,235 failed under the same load), but nothing promises that. What needs such
  a value is the browser's: make it only with the feature `csr` and leave it `None` on the
  server, as `Pending`'s own part and `nav::watch_size` do; what the server renders stays the
  same.

## 3. Running it

```bash
./radix.exe serve-snapshot --addr 127.0.0.1:8090
```

(Without a crawl of your own: the catalog of betula.app, from `https://betula.app/api/db`, §4.)

```bash
bash folia/scripts/dev.sh --watch
```

The second command is the two below, and builds them again as the code changes („Working on the
site"):

```bash
bash folia/scripts/build-client.sh --dev
```

```bash
cargo run -p folia-server
```

The first of them builds the browser app into `site/pkg` (needs the `wasm32-unknown-unknown`
target and `wasm-bindgen` 0.2.128, which Trunk keeps in its cache); without it the site simply
stays server-rendered. `--dev` builds it with the `wasm-dev` profile and the flags of
`.cargo/config.toml` (`--cfg erase_components`, see „Build times"), and keeps the names of its
functions for the debugger; leave it off to build the bundle that ships, as Nix builds it,
without them. Which one you want is a question of minutes:

| | profile | edit a page, build again | `site/pkg` | gzipped |
|---|---|---|---|---|
| `--dev` | `wasm-dev`, `erase_components` | 11 s | 7.4 MB | 1.4 MB |
| (none) | `wasm-release` | 2 min 18 s | 19 MB | 1.4 MB |

`wasm-release` owes those two minutes to fat LTO, `opt-level = "z"` and its single codegen unit,
and all of it buys the megabyte that people who load the site once do not have to fetch. On
localhost the difference arrives over the loopback; never deploy a bundle built with `--dev`.
(Since 2026-10-01 the bundle that ships leaves the names of its functions out: as Nix built it
from develop that day, 33.8 MB with them, 29.0 MB of them names, and 4.7 MB without; 1.74 and
1.43 MB in brotli, „What ships".)

### Build times

The workspace is around 20 000 lines of Rust on 339 crates, and most of `folia-app` is Leptos
views (384 `view!` macros). What that costs on a laptop with six cores (Ryzen 5 PRO 4650U, twelve
threads, 16 GB), measured 2026-09-23:

| | without `erase_components` | with it (since 2026-09-23) |
|---|---|---|
| cold build, empty cache | 5 min 21 s | 3 min 24 s |
| new worktree: fork the main checkout's cache, then build | 19 s + 2 min 46 s | 19 s + 57 s |
| edit a page in `app/` (the catalog's filter panel), `cargo build` | 19 s | 10 s |
| edit `folia/crates/design/src/ui.rs`, `cargo build` | 14 s | 9 s |
| edit `server/`, `cargo build` | 19 s | 6 s |
| edit a page, `folia/scripts/build-client.sh --dev` | 15 s | 11 s |
| `cargo build` with nothing to do | 0.9 s | 0.9 s |

The edits are real ones: a string grows by a character. An edit that keeps every length the same
(a digit for a digit) costs the server 8 s instead of 19 s without the flag, and the rest a
second less, so measuring with such an edit flatters.

**`--cfg erase_components`.** Leptos gives every view a type of its own: an element is a type with
its attributes and children, a component's `impl IntoView` the whole tree below it. rustc infers
these types in one frontend thread for all of `folia-app`, and instantiates the code that renders
them in `folia-server` and again in `folia-client`. With the flag, Leptos erases the type of every
component and every list of children (`AnyView`; the documentation is at `src/lib.rs:264` of
leptos 0.8.20), and little is left to infer or to instantiate. From the fork's first build:
`folia-app` 96 s → 35 s (its frontend 67 s → 23 s), `folia-server` 61 s → 13 s; the debug server
binary is 137 MB instead of 1.2 GB, the browser app of `--dev` 7.4 MB instead of 29 MB.

It is a flag for rustc and not a feature, so it is one of the flags `folia/scripts/build-cache.sh`
writes into `.cargo/config.toml`, for the host and for `wasm32-unknown-unknown`, and base and
forks are built with it alike. It is for working on the code: the bundle that ships
(`build-client.sh` without `--dev`) is built without the flags of that file, and Nix never sees
it. What the two builds do differently, checked on 6 500 pages (every address of the sitemap and
1 100 lists and filters found on them):

- The server's HTML carries more of Leptos's markers (`<!>`, `<!--<() />-->`): 2–5 % of a page,
  about 1 % gzipped. Without its comments every page is the same, byte for byte.
- Every reactive closure costs an `Arc<Mutex<_>>` and a call through a vtable. A page not yet
  in the cache renders in 8.8 ms instead of 8.0 ms (median of 250, debug builds).
- What compiles with the flag need not compile without it: the types it erases are deeper there,
  and so closer to `recursion_limit`. Nix builds without it, and `deploy/ship.sh` builds with Nix
  before it ships anything, so such a failure stops a release instead of slipping into one. To
  see it earlier: `CARGO_ENCODED_RUSTFLAGS= cargo build` (every dependency once more, in the
  same cache).

A change of the flags is a change of every unit's fingerprint: after one, `setup` and `prime` in
the main checkout, or every fork builds all 415 units again.

Every checkout gets its own build cache, and a new worktree starts from a copy of the main
checkout's:

```bash
git config core.hooksPath folia/scripts/hooks   # once per clone: git does not carry it along
bash folia/scripts/build-cache.sh setup         # in the main checkout, and after `rustup update`
bash folia/scripts/build-cache.sh prime         # in the main checkout, after a merge into develop
bash folia/scripts/build-cache.sh gc            # drop the caches of worktrees that are gone
```

The main checkout builds into `target/base`; `setup` in a worktree copies it to
`target/wt-<worktree>` and points the worktree there. The copy carries the 339 dependencies, so
only `folia-catalog`, `folia-app` and `folia-server` compile — once, after which every edit is
incremental. The fork is 1.9 GB and takes 19 s (base holds the tests' and the browser app's
dependencies too); after its first build a worktree's cache is about 4.7 GB, which is why `gc`
exists. Before it did, `target/` had grown to 140 GB across fifteen caches of branches long
merged. Each cache records which worktree it belongs to, and `gc` drops it once that directory
is gone, so the caches follow the worktrees and not `git worktree list` (which still lists a
worktree under its old path after the repository has moved, until `git worktree repair`).

A worktree inside the main checkout (`.claude/worktrees/<name>`) takes its flags from the main
checkout's `.cargo/config.toml` and writes only its target directory into its own. Cargo reads
the config files of every directory above the one it runs in and joins their lists; written into
both, the flags stood there twice, no unit of the fork matched, and the first build of such a
worktree took 5 min 20 s instead of 2 min 46 s (until 2026-09-23).

A new worktree needs none of these commands. `git worktree add` runs
`folia/scripts/hooks/post-checkout` with the null commit as the previous HEAD, and the hook runs
`setup` before `git worktree add` returns (16 s longer) and `gc` in the background after it —
deleting a gone worktree's cache takes Windows half a minute per 2 GB, and nothing needs to wait
for that. Branch switches and file checkouts leave the hook at its first line, and it never makes
the checkout fail: a worktree without its cache is slower, not broken. Because `gc` may run while
another worktree is being forked, `setup` records a cache's owner before it copies anything, or
`gc` would take the half-copied cache for an orphan.

What does not work, measured, so it is not tried again:

- **One cache for all worktrees.** rustc keeps one incremental session per crate, and worktrees
  building into the same target directory overwrite each other's. Alternating edits between
  two of them cost 202 s, 196 s, 202 s instead of 14 s each.
- **Reusing the workspace crates on a fork.** The incremental session records absolute paths and
  is worthless at any other path, so it is left out of the copy together with the executables.
  Cargo's `trim-paths`, which would change that, is nightly-only.
- **Backdating the sources of a new worktree** so cargo takes the copied artefacts for its own.
  The first build then takes 2 s, but the first edit 198 s, because there is no incremental
  session to build on: the three minutes move, they do not go away. And cargo compares
  timestamps, never contents, so a file that differs from what the cache was built from but
  looks older would silently leave its old code in the binary. `setup` does the opposite: it
  touches every workspace source after the fork, so nothing in the copy can pass for current.
- **Measuring with `CARGO_INCREMENTAL=0`.** The variable is part of every unit's fingerprint:
  all 356 units of the cache are built again (5 min), and the cache holds them twice.

What is left of the 57 s: `folia-catalog` 10 s, `folia-app` 35 s (23 s of it one frontend
thread), `folia-server` 13 s. **`folia-app` in crates** (one per layer and per page, so that the
pages compile side by side) was measured on top of the flag and not taken for now: the first
build 57 s → 51 s, an edit in a page 10 s → 8 s, the same edit in the browser app 11 s → 6.3 s,
but an edit in `ui` 9 s → 12.5 s (every page and the shell compile again), rustc's frontends
23 s → 49 s in all, and the bundle that ships 3.6 % larger gzipped. Without the flag it is worth
more (the first build 166 s → 128 s). The split is the branch `claude/web-tier-compile-time-0a192a`.

`setup` also writes the linker into `.cargo/config.toml`: the toolchain ships `rust-lld`, but
`gcc` — the linker driver on `x86_64-pc-windows-gnu` — only finds it when pointed at the
toolchain's `gcc-ld` shims. That is 24 s against 19 s on every rebuild, and a minute and a quarter
on a cold one. The linker is a path on this machine, so the file is generated and not in git; and
one script writes the flags of base and forks because they must be built with the same ones, or
the copy is of no use.

With the cache outside the checkout, `folia/scripts/build-client.sh` asks `cargo metadata` where the
bundle landed instead of assuming `target/`.

Open `http://127.0.0.1:8080`. The server fetches the snapshot over HTTP into `web-data/` and
keeps serving the last good one when Radix is away, also after a restart.

| Flag | Environment | Default | |
|---|---|---|---|
| `--addr` | `FOLIA_ADDR` | `127.0.0.1:8080` | listen address |
| `--snapshot-url` | `FOLIA_SNAPSHOT_URL` | `http://127.0.0.1:8090/snapshot/catalog.db` | Radix's endpoint (plain HTTP inside the deployment network) |
| `--data-dir` | `FOLIA_DATA_DIR` | `web-data` | downloaded snapshots |
| `--poll-seconds` | `FOLIA_SNAPSHOT_POLL` | `60` | check interval (conditional GET) |
| `--stale-after-seconds` | `FOLIA_SNAPSHOT_STALE_AFTER` | `21600` | `/healthz` fails when Radix was silent this long (0: never) |
| `--html-cache-mb` | `FOLIA_HTML_CACHE_MB` | `128` | rendered pages kept in memory, compressed (256 in production) |
| `--warm-cache` | `FOLIA_WARM_CACHE` | `on` | render every page of the sitemap into the cache after a new snapshot, while idle ("Load") |
| `--workers` | `FOLIA_WORKERS` | `0` | worker threads; 0: one more than the processors the container may use |
| `--render-places` | `FOLIA_RENDER_PLACES` | `0` | pages rendered at once; 0: one per processor |
| `--render-wait-ms` | `FOLIA_RENDER_WAIT_MS` | `3000` | how long a page waits for a place before it is answered 503 with `Retry-After` |
| `--feed-places` | `FOLIA_FEED_PLACES` | `0` | calendar feeds made at once (0: one per processor); a feed waits at most 10 s |
| `--site-root` | `FOLIA_SITE_ROOT` | `site` | browser bundle (`pkg/`), from phase 2 |
| `--semantic-model` | `FOLIA_SEMANTIC_MODEL` | (none) | the browser's model of the semantic search (`e5-de-en.bin`, 15 MB, `folia/crates/semantic/README.md`), served as `/models/e5-de-en-<hash>.bin` and named in `/api/status` (`semantic_model`) and `boot.js`; without it, or when it cannot be read (`semantic.model_unreadable`, ERROR), the app has no semantic search and everything else runs. A file named by a sha256 (the model store, `deploy/models.lock`) has to have that content |
| `--semantic-passage-model` | `FOLIA_SEMANTIC_PASSAGE_MODEL` | (none) | Radix's id of the passage model the query model was made for (16 hex digits); the browser offers the semantic search only on a snapshot whose `meta.semantic_model` is that one. Without it, on any. `vps/50-app.sh` sets both from `deploy/models.lock` |
| `--live-assets` | `FOLIA_LIVE_ASSETS` | (none) | while working on the site: the stylesheet, the scripts and the SVGs from this directory (`folia/assets`) as they are, not minified; nothing kept as immutable, and a service worker that keeps nothing („Working on the site"); never in production |
| `--card-cache-mb` | `FOLIA_CARD_CACHE_MB` | `64` | finished link-preview cards kept in memory |
| `--public-url` | `FOLIA_PUBLIC_URL` | `https://betula.app` | the site's address from outside: canonical links, link previews, sitemap |
| `--access-gate` | `FOLIA_ACCESS_GATE` | `off` | closed testing: the whole site asks for one shared password (see below) |
| `--log-format`, `--log-level` | `FOLIA_LOG_FORMAT`, `FOLIA_LOG_LEVEL` | `text`, `info` | `json` in production |

`folia healthcheck` is not the server but its probe (like `radix healthcheck`): it asks the server
that listens on `FOLIA_ADDR` for `/livez` and exits with 0 when it answers. The container image
uses it as `HEALTHCHECK`, because an image built with Nix has no curl or wget. `folia assets`
lists what ships („What ships"); like every command it takes the server's flags before it
(`folia --site-root site assets`).

Endpoints besides the pages: `GET /api/db`, `GET /api/status`, `GET /api/map.json` (the map of the
programs, with the snapshot's ETag), `GET /healthz` (200 while a snapshot is served and Radix was
heard from; for an uptime monitor), `GET /livez` (200 while the process answers; for the container's
healthcheck, which must not restart a server that still serves its last snapshot),
`/assets/app.css`, `/assets/icons.svg` (the sprite every icon points at), `/assets/favicon.svg`, `/sw.js` (the service worker with the build written in),
`/assets/og.png`, `/favicon.ico`, `/apple-touch-icon.png`, `/assets/icon-192.png`,
`/assets/icon-512.png`, `/assets/icon-maskable-512.png`, `/assets/icon-maskable-1024.png`,
`/assets/icon-monochrome-512.png`, `/manifest.webmanifest`, `/assets/launch/<w>x<h>[-dark].png`
(the launch screens of iOS), `/cards/module/<id>.png`, `/cards/program/<slug>.png`, `/robots.txt`,
`/sitemap.xml`, and
`GET`/`POST /access` (the login page of closed testing). The stylesheet and the scripts answer
under any `?v=<build>` as well (the page links them so, see Offline). Every answer carries the
header `x-build` with the build of the process (version and start time, as in `/api/status`).

**Caching and compression** (2026-09-30; `folia/crates/server/src/encoding.rs`, `api::Keep`). Everything goes
out in brotli to a client that takes it (every browser does over HTTPS), in gzip to one that takes
only gzip, plain to the rest; what is made once is compressed once, at brotli's best and off the
threads that answer requests. Measured against the gzip -6 of before: the stylesheet 63 → 51 kB,
the bundle 2.8 → 1.7 MB, the snapshot 7.6 → 4.4 MB, the start page 118 → 95 kB.

| Answer | `Cache-Control` | Coding |
|---|---|---|
| pages | `public, no-cache`, ETag of snapshot and build → `304` | brotli 5 as rendered, kept so in the cache; gzip made anew for a client without brotli |
| a file under `?v=<build of this process>`: stylesheet, scripts, sprite, sql.js, bundle | `public, max-age=31536000, immutable` | brotli 11, made once per process (`encoding::Kept`); the bundle at 9 (2 s instead of 31 s) |
| every other file: without a build or of another one, the font, the icons, the manifest, the birch, `/sw.js` | `public, no-cache`, the build as ETag → `304` | brotli 11 where it is not compressed already (woff2, PNG, WebP); the wood's masks as drawn (`.svg.br`) |
| `/api/db` | `public, no-cache`, Radix's ETag → `304` | brotli 11 (4 MiB window), made in the background after a new snapshot (a minute and a half of one processor, about 120 MB of memory for the while) and kept beside it; gzip until then |
| `/api/map.json`, `/sitemap.xml` | `public, max-age=300, stale-while-revalidate=86400` | brotli 11, the sitemap at 9, made on first request |
| calendar feeds | `private, max-age=900`, ETag of the content | brotli 5 as made |

A `304` says how to keep what it confirms (`Cache-Control`, `Vary`), as its `200` did. After a
start the files every page asks for are compressed in the background (`warm::files`, 8 s of one
processor), so the first visitor does not wait for them. Traefik's `compress` passes all of it through and compresses
what comes plain (a 404 page, the login page) in brotli too
(`deploy/config/traefik/dynamic/middlewares.yml`: `br` before `zstd`, since it takes the first of
the codings a browser weighs the same).

### What ships (2026-10-01)

Owner, 2026-09-28: „Aktuell wird da echt noch viel unnötiger code geshiped." Until then the server
embedded `folia/assets` as it is written — its comments, its indentation — and the browser app kept
the names of its functions. Now whatever a browser gets as text is minified when the server is
built (`folia/crates/server/build/main.rs`, which cargo runs before it compiles the server;
`folia/crates/server/src/assets.rs` lists what the server embeds of it). The minified bytes go the way every
file goes (brotli 11, made once per process, `encoding::Kept`): no compression in the build.

- **The stylesheet** with lightningcss, written shorter and nothing else: no `targets`, so nothing
  is lowered or prefixed. It writes `rgba(…)` as `#rrggbbaa` and so rounds an alpha to 1/255.
  One thing it gets wrong is put right after it (`TimelineApart` in `folia/crates/server/build/main.rs`): it
  folds `animation: wood-rise linear both; animation-timeline: --page` into
  `animation:linear both wood-rise --page`, since its data has Chrome read a timeline in the
  shorthand from version 115. Chrome 141 does not and drops the whole declaration, and the wood of a
  page stopped moving with its scroll (`folia/e2e/ground.mjs` found it). So the timeline goes back into
  an `animation-timeline` of its own after the shorthand, and `assets::tests` keeps it there.
- **The scripts** with oxc: comments and whitespace out, names shortened, constants folded, and
  in no newer syntax than the scripts are written in (ES2020: `?.` and `??` in `enhance.js`; left
  to itself oxc wrote `a ||= b`, ES2021, into a script every browser loads). A classic script's
  top-level names are globals that other scripts call (sql.js's `initSqlJs`, which `boot.js`
  calls), so a script is read as one and they stay; only what a module alone can be
  (`import.meta`, a top-level `await`: `boot.js`) is read as a module. What the server writes in
  when it serves them (`__BUILD__`, `__SCHEMA__`) must still be there: the build fails when the
  minifier folded one away. `Number("__SCHEMA__")` was folded into `NaN` before the server could
  write the number in, which is why `boot.js` writes `const SCHEMA = __SCHEMA__;`.
- **The SVGs** by `folia/crates/server/build/svg.rs`: the same numbers, written shorter — no leading zero, a
  segment of a path relative to where it starts when that is shorter, `h`/`v` for a line along an
  axis, a repeated command without its letter, between numbers only the separators the grammar
  needs. Quotes, ids and colours stay as they are: the cards find them as text (`cards::mask`).
  resvg draws every file the same to the pixel (`assets::tests`). Chromium adds relative
  coordinates in single precision, so an edge pixel may come out a level or a few of 255 lighter
  or darker. **The wood** behind the start page is not minified: `folia/design/forest/forest.mjs` draws
  it as short as it gets and compresses it itself (`.svg.br`), and what `svg.rs` made of it was a
  few per cent shorter but compressed 1 to 2 % larger (autumn's back 38,911 → 39,638 bytes).
- **The manifest** is written without indentation.
- **The browser app** without the names of its functions (`--remove-name-section`, and the
  `producers` section, in `folia/scripts/build-client.sh` and `flake.nix`; `--dev` keeps them for the
  debugger). A panic's stack trace then names functions by number. `wasm-opt -Oz` on top was
  measured on 2026-09-28 and left out (−0.4 % compressed, not worth another tool in every build).

What a first visit downloads besides the page itself, as the server sends it to a browser
(brotli, 2026-10-01, both builds by Nix, `#folia` and `#folia-client`; the birch of autumn):

| | develop | now | brotli develop | brotli now |
|---|---|---|---|---|
| `app.css` | 267,814 | 172,771 | 52,281 | 28,600 |
| `enhance.js` | 46,474 | 19,446 | 12,883 | 6,105 |
| `boot.js` | 8,812 | 3,739 | 2,912 | 1,542 |
| `sw.js` | 6,643 | 2,369 | 2,253 | 992 |
| `sql-wasm.js` (sql.js, minified already) | 48,863 | 43,518 | 15,038 | 13,156 |
| `manifest.webmanifest` | 1,143 | 869 | 360 | 335 |
| the birch of a page (crown, its head, roots, litter) | 254,098 | 209,615 | 47,557 | 31,389 |
| the wood of the start page (as drawn, unchanged) | 365,102 | 365,102 | 72,802 | 72,802 |
| `folia_client_bg.wasm` | 33,757,019 | 4,732,748 | 1,740,869 | 1,431,872 |
| all of it, with what stayed as it was | 35,536,473 | 6,330,672 | 2,283,390 | 1,923,222 |

What stayed as it was: `folia_client.js` (wasm-bindgen's 64 kB, 9 kB in brotli:
`build-client.sh` writes it, not the server's build), the sprite (`app::icons` writes it), the
pictures, the font and sql.js's WASM (compressed already), and the HTML of the pages (Leptos
writes it without indentation). A first visit thus fetches 16 % less, the stylesheet and the
scripts about half.

`folia assets` (or `bash folia/scripts/dev.sh sizes`) lists every file as written and as served, each
also in brotli, and the browser app in `<site-root>/pkg`; a bundle that still carries the names of
its functions (a `--dev` build) is named as one not to ship.

What it costs: the minifiers are build dependencies of the server — built once per build cache,
none of them in the server — and they run again only when a file of `folia/assets` changed. oxc
needs rustc 1.96 or newer (nixpkgs has 1.98).

### Working on the site: `folia/scripts/dev.sh` (2026-10-01)

```bash
bash folia/scripts/dev.sh                 # build what is stale, serve on http://127.0.0.1:8080
bash folia/scripts/dev.sh --watch         # and build again and restart when Rust code changes
bash folia/scripts/dev.sh -- --addr …     # the server's own flags after `--`
bash folia/scripts/dev.sh sizes           # what ships (folia assets)
```

It builds the browser app when anything of its code is newer than the bundle (`build-client.sh
--dev`), builds the server, and runs it with `--live-assets folia/assets`: the stylesheet, the
scripts and the SVGs come from disk on every request, as they are written — no build and no
restart for them, an edit is there with the next reload. Each answer is tagged by what the file
holds, so an unchanged file is still a 304. Nothing is kept as immutable there, not even under
`?v=<build>` (`api::Keep::of`): an edited file, or a bundle built again, is a new file under the
same address. The service worker of such a server keeps nothing and listens to no request (and
drops what the worker of an earlier run kept), so a reload never gets an old file from it. Only
the files the server serves anyway are read (a path is looked up in `assets::MINIFIED` or
`birch::file`, never joined as it comes), and none is compressed: they go to this machine.

`--watch` looks at the Rust code once a second (`find -newer`, which Git Bash has as well): a
change in `client/` builds the browser app, one in `server/` the server, one in `app/`, `catalog/`
or `pack/` both, and the server is restarted. The browser app is built while the old server still
answers; the server is stopped before it is built, because Windows does not let a build overwrite
a running `folia.exe`. What does not build is said, and the next change is waited for. An edit of
`server/` answers with the new build in 12 s here (four cores, without the flags of
`folia/scripts/build-cache.sh`). The page cache is not warmed (`FOLIA_WARM_CACHE=off`): the warm-up
renders all 5,000 pages after every start, and here every restart is one.

What is tested while working on it is not what ships: the stylesheet and the scripts minified,
offline with the real service worker. For that, `cargo run -p folia-server` as before, and the
bundle without `--dev`.

### Closed testing: the access gate (`folia/crates/server/src/access.rs`, 2026-09-21)

For the time in which the site is tested by invited people only (owner: the legal pages come
later, so nothing may be public yet). One switch, one secret:

```bash
FOLIA_ACCESS_GATE=on FOLIA_ACCESS_PASSWORD='…' cargo run -p folia-server
```

- **The switch** is `FOLIA_ACCESS_GATE` (`on`/`off`, also `true`/`false`, `1`/`0`; flag
  `--access-gate`). Off is the default and leaves no trace but `/access` leading on to the site.
  Taking the gate away later is this one value; the secret may stay where it is.
- **The password** is a secret and is found the way Radix finds its key (`docs/radix/operations.md`
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
  need: `/access`, the stylesheet, the font, the icons, the launch screens of iOS (those of the
  listed screens only), `/manifest.webmanifest` (browsers fetch it without cookies), `/healthz`
  (uptime monitor), `/livez` (the container's healthcheck) and
  `/robots.txt`, which says `Disallow: /` while the gate is on, except `Allow: /calendar/`. The
  login page is `noindex` and `no-store`.
- **And a Studienplan's calendar subscription** (owner decision 2026-09-24): `/calendar/<code>.ics`
  answers without the password when its code decodes (`subscription::is_feed_path`: the `pack`
  alphabet, 1 to 1,024 characters, `.ics` and nothing else, `%HH` escapes of alphabet characters
  read as those characters; then the check characters, the kind `calendar`, the layout
  `subscription::VERSION` and the caps), because
  calendar services fetch it from their own servers and send no cookie. What that opens is the
  QIS schedule of the modules a code names, which every module page shows behind the gate, and
  nothing about a visitor; the check characters turn guesses away before any handler runs. Every
  other path under `/calendar/` (`/calendar/abc`, `/calendar/Ab.ics.ics`, a Merkliste code with
  `.ics`) stays behind the gate. The feed answers `private` already, so the gate leaves its
  `Cache-Control` alone. No robots.txt disallows a feed, gate on or off: Google Calendar reads
  robots.txt before it fetches a subscription and gives up on a disallowed address; the feed's
  `X-Robots-Tag: noindex` keeps it out of indexes, and a crawler only sees that header when it
  may fetch. Tests in `folia/crates/server/src/tests.rs`:
  `closed_testing_asks_for_the_password_before_anything_else` (what passes and what not),
  `a_studyplan_is_a_calendar_feed` (the gated feed with a snapshot), `broken_calendar_codes_are_404`,
  `the_log_keeps_no_calendar_code`, `calendar_services_may_fetch_feeds`; the shapes in
  `folia/crates/calendar/src/subscription.rs`.
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
app as `folia/scripts/build-client.sh` builds it, with a wasm-bindgen CLI of exactly the version in
`Cargo.lock` (nixpkgs rarely has that one; after a change of the version the two hashes in
`flake.nix` have to be renewed, the comment there says how). The image holds `/bin/folia`, the
browser app under `/site` and nothing else; it runs as user 10001, to whom `/data` belongs (a
fresh named volume takes the owner over), with a read-only root file system if asked to. Defaults
inside: `FOLIA_ADDR=0.0.0.0:8080`, `FOLIA_DATA_DIR=/data`, `FOLIA_SITE_ROOT=/site`,
`FOLIA_SNAPSHOT_URL=http://radix:8090/snapshot/catalog.db`, `FOLIA_LOG_FORMAT=json`, and
`HEALTHCHECK folia healthcheck` (every 2 s while starting, then every 30 s). The tests do not run
in the Nix build (they need a snapshot). Shipping both images to the server and deploying an
instance is `deploy/ship.sh` (`deploy/README.md` §4).

### Load (2026-09-26)

Owner: „wie viel Traffic meine Seite aushält … von Crawlern, richtigen Benutzern, Benutzern ohne
JS und Abfragen von Google Calendar“, then „optimiere das Setup auf Performance … du kannst alle
Ressourcen des VPS verwenden“ (live and canary, each Folia and Radix, and the management around
them). Measured with `folia/e2e/load` (below) in a lab on the workstation — Folia's release build as Nix
builds it, the production settings, pinned to as many processors as its container gets — and
against https://canary.betula.app for what is open there (calendar feeds, the rate limit).

**What a request costs.** One processor of the workstation; the server's is about 2.5 times
slower (the same calendar feeds took 10 ms here and about 28 ms there, the heaviest 47 and 114
ms). Before → after this round:

| | before | after |
|---|---|---|
| a filtered list of the catalog, not cached | 31 ms (32/s per processor) | 11.5 ms (85/s) |
| the program overview with a filter / without | 31 ms / 31 ms | 3.3 ms / 5.5 ms |
| a module / a program's page, not cached | 4.3 ms / 10 ms | the same |
| any cached page | 0.2–0.3 ms (3,000/s) | the same |
| a calendar feed | 12 ms (85/s) | the same, one per processor at a time |
| a first visit with JavaScript | 57 ms, 10.8 MB (7.6 MB of them `/api/db`) | the same |
| a page view without JavaScript (cached page, stylesheet and font revalidated) | 1 ms | the same |

The filtered list spent 12 of its 31 ms loading what its pickers offer — every program, department
and person (the persons alone 8.5 ms) — on every render, though that changes only with the snapshot;
the program overview likewise its programs. Both are made once per snapshot now and handed to the
renders (`PickerChoices`, `ProgramsReady`, like the map of the programs). The persons' `<datalist>`
(705 names, 28 kB of every page of the catalog) is gone: the server's catalog is there to lead search
engines to the modules, and the persons stand on the module's page (owner: „das soll nur auf die
Modulseite“). The icons point into one sprite (`app::icons`) instead of carrying their paths
(owner: „das kann ja auch alles statisch geserved und nur verlinkt werden“): a page of the catalog
went from 129 to 91 kB (18.5 to 11.3 kB compressed), a module's from 20 to 18 kB.

**How the server behaves under load** (the part that mattered most). One processor and one worker
thread, as in production until now: a crawler asking for filtered lists faster than they were made
(45 a second, below the 50 Traefik lets one address ask) built a queue that only grew — after half
a minute every answer took 6 to 13 seconds, `/livez` included, and the container's healthcheck
(3 failed probes of 3 s) would have restarted it with an empty cache. On the server that was 13
lists a second; canary really did tip over at 20 calendar feeds a second (test from outside, the
queue grew to 15 s). Now:

- **Places** (`folia/crates/server/src/busy.rs`): renders and calendar feeds run one per processor; a request
  that finds every place taken waits (at most `FOLIA_RENDER_WAIT_MS`, and never behind more than 64
  waiting per place) and is then answered **503 with `Retry-After`** — crawlers and calendar
  services come back later. Cached pages, files and `/livez` need no place.
- **One more worker thread than processors**: with one thread for everything, renders starved the
  accept loop and connections were refused before any request could be told 503.
- **The cache keeps pages compressed only** (in brotli since 2026-09-30), drops views before the pages search engines list (the sitemap's, and the further pages of the catalog), and renders
  a page once however many ask for it at the same time (`folia/crates/server/src/cache.rs`). 128 MiB used to hold
  1,200 pages — not even the modules; the sitemap's 5,235 pages take 29 MiB now.
- **Warm-up** (`folia/crates/server/src/warm.rs`): after every new snapshot and after a start, every page of the
  sitemap is rendered into the cache while the server is idle — 22 s on one processor here, about a
  minute on the server — so crawlers walking the sitemap and visitors after a deploy meet no render.
  Each page renders in a task of its own: a render that panics costs that page, logged as
  `cache.warm_page_failed`, and the warm-up goes on (before, one ended it until the next restart,
  R22).
- `/api/db` hands every browser the same bytes from memory.

Measured on three processors (the new CPU limit, `deploy/stacks/betula.yml`): 197 filtered lists a
second not cached; at 1.5 and 3 times that the surplus got 503 within about a second, and `/livez`
answered in 17–39 ms (p99). A mix of crawlers (cold and cached pages), visitors without JavaScript,
first and returning visitors with JavaScript and calendar services at 200 visits a second (590
requests, 109 MB a second: among them 60 lists rendered, 10 first visits and 40 feeds a second)
used 1.6 of the 3 processors, every answer within 110 ms (p99).

**What that means on the server** (4 shared vCPUs, factor 2.5; Folia may use 3): about 80
filtered lists or 270 modules a second not cached, over 3,000 cached pages, about 100 calendar
feeds (Google fetches a subscription every few hours: 7,000 subscriptions are 1 a second). What
runs out first with real visitors is the **bandwidth**: a first visit with JavaScript downloads about
10 MB (9 MB with the release bundle), so 200 Mbit/s carry 2.5 first visits a second — 9,000 an
hour — and 1 Gbit/s five times that (see the Contabo plan for the port). A new snapshot makes every
returning visitor download `/api/db` again (7.6 MB). From the workstation (100–125 Mbit/s) the
server's own port could not be measured.

**Traefik's rate limit** (50 a second per address, bursts of 100, `config/traefik/dynamic/
middlewares.yml`) works as configured: of 800 requests in 10 s from one address 596 passed (= 100 +
50 × 10). A first visit with JavaScript is about 30 requests, so one address carries about 3 first
visits at once and 1.7 a second after that — worth raising if a campus network puts many students
behind one address (a lecture hall opening the app together).

**Not changed, noted:** a catalog page still renders 155 icons and a list of 183 programs into a
`<select>` for visitors without JavaScript; a filtered list's remaining 11.5 ms are 7.7 ms of queries
(the totals of the program's two lists, the page, the program's areas) and the render. The home page
is 410 kB (106 kB compressed), rendered once per snapshot.

**Running it** — `folia/e2e/load` is a Go program without dependencies (`go build -o betula-load.exe .`
in that directory); `folia/e2e/load/lab.ps1` starts the lab on Windows:

```bash
betula-load discover -base http://127.0.0.1:18080 -out pages.tsv -max 40000     # what a crawler finds
cargo run --release -p folia-pages --example loadtest_feeds -- <catalog-*.db> 2026W --heavy 20 > feeds.tsv
betula-load run -base http://127.0.0.1:18080 -scenario crawl-cold -pages pages.tsv -rates 50,100,200 -step 30s -probe -pid <folia>
betula-load run -base http://127.0.0.1:18080 -scenario "crawl-cold=30,crawl-warm=20,nojs=10,js-first=5,js-return=15,ics=20" -pages pages.tsv -hot 3000 -feeds feeds.tsv -rates 25,50,100,200
betula-load run -base https://canary.betula.app -scenario ics -feeds feeds.tsv -rates 5,10,20 -abort-p99 5s   # the server, what is open there
betula-load logstats -log folia.log                                                # what each kind of page cost the server
```

Jobs: `crawl-cold` (every address once: renders), `crawl-warm` (popular pages more often),
`nojs`, `js-first`, `js-return`, `js-update` (a returning visitor after a new snapshot), `ics`,
`livez`; `-rates` starts them on a Poisson schedule whatever the server does (an overloaded server
shows as latency and errors, not as a politely lower rate), `-concs` runs that many back to back.
`-probe` asks `/livez` every second next to the load. `cargo run … --example loadtest_profile --
<catalog-*.db> pages.tsv` times every query a page of the catalog runs. Against the server stay
below the rate limit and watch Grafana: the whole site is one small VPS.

### Log events (same rules as `docs/radix/operations.md` §2: ERROR = a human has to act)

| Level | `event` | Meaning |
|---|---|---|
| INFO | `server.listening`, `server.shutdown` | lifecycle |
| INFO | `snapshot.sync_started`, `snapshot.restored`, `snapshot.downloaded`, `snapshot.activated`, `snapshot.sync_recovered` | snapshot lifecycle (`etag`, `bytes`, `generation`; `schema_version` when activated) |
| INFO | `snapshot.map_built` | the map of the programs was laid out for a snapshot (`programs`, `links`, `ms`) |
| INFO | `snapshot.compressed` | the brotli copy of the active snapshot was made; `/api/db` sends it from now on (`etag`, `bytes`, `ms`) |
| INFO | `files.compressed` | after a start, the files every page asks for are in brotli (`files`, `ms`) |
| WARN | `snapshot.map_failed` | it could not be; the landing page goes without the map |
| DEBUG | `snapshot.unchanged` | Radix answered 304 |
| INFO | `http.request` | access log: `method`, `path`, `status`, `ms`, `cache` (`hit`/`miss`/`-`); every path under `/calendar/` is written `/calendar/….ics` and every path under `/cards/studyplan/` `/cards/studyplan/….png` (a code names somebody's plan; a shared Stundenplan's page, `?share=`, is logged without its query like every page) |
| WARN | `http.request` with `cache=busy` | a request turned away with 503 because every place was taken ("Load"): no error of the server |
| WARN | `server.busy` | the same, at most once a minute: `what` (`render`, `calendar`), `places`, `wait_ms`, `turned_away` since the start. Often: more processors, or a crawler to slow down |
| INFO | `cache.warmed` | the pages of the sitemap are in the cache (`pages`, `rendered`, `kept`, `other`, `changed`: pages that say something new, their `lastmod`; `failed`: pages whose render panicked; `ms`, `generation`) |
| WARN | `lastmod.write_failed` | the dates of the sitemap (`lastmod.json`) could not be written to the data directory; they are kept in memory until the next start |
| WARN | `cache.warm_failed`, `snapshot.choices_failed` | the sitemap could not be listed for the warm-up / the pickers of the catalog are loaded per page again |
| DEBUG | `http.request` with `path=/livez` | the container's own probe, twice a minute |
| DEBUG | `calendar.served` | a calendar feed was made (`bytes`, `ms`; never the code or the modules) |
| INFO | `access.gate_on` | closed testing is on (`source`: where the password was found, never the password) |
| INFO | `access.granted` | the access password was entered |
| WARN | `access.denied` | a wrong access password (`failures` in this minute, `closed` when the form closed; at most ten lines a minute) |
| DEBUG | `card.drawn` | a link-preview card was drawn (`key`, `bytes`, `ms`) |
| WARN | `server.live_assets` | the server reads the stylesheet, the scripts and the SVGs from disk (`--live-assets`, `dir`): for working on the site, never in production |
| WARN | `assets.live_failed` | under `--live-assets`, a file the server serves is not on disk (`path`, `error`): answered 404 |
| WARN | `card.busy` | every drawing place was taken, previews got the standard picture (`count`; at most one line a minute). Often: more places or a larger `--card-cache-mb` |
| WARN | `snapshot.fetch_failed` | Radix unreachable or not ready; retried with backoff; the last snapshot stays active |
| WARN | `snapshot.restore_failed`, `snapshot.compress_failed` | stored snapshot unusable / its gzip or brotli copy could not be made: served in gzip or uncompressed (brotli is not tried again for that snapshot) |
| ERROR | `snapshot.rejected` | a download is not a usable catalog; the previous snapshot stays active |
| ERROR | `snapshot.outdated` | the active snapshot is of an older schema than this build reads (`schema_version`, `needs`): pages that need the newer columns fail, browsers do not start the app on it. Served all the same; Radix has to export a new one (with `RADIX_CRAWL=off` it never does by itself) |
| ERROR | `snapshot.stale` | no answer from Radix for longer than the limit |
| ERROR | `http.request` with `status >= 500`, `render.failed`, `snapshot.unreadable` | a request failed |
| ERROR | `cache.warm_page_failed` | a page of the sitemap panicked while it was rendered for the warm-up (`path`, `generation`, `error` with the panic's message); it is left out and the warm-up goes on (R22) |
| ERROR | `card.failed` | a card's text could not be read or the card could not be drawn; the preview got the standard picture |
| ERROR | `calendar.failed` | a calendar feed could not be read from the snapshot (or its task failed); the calendar service got a 500 and asks again later |
| ERROR | `server.start_failed`, `server.failed` | the server cannot run |

## 4. Checks

```bash
cargo test
```

needs a snapshot (`snapshot/current.json` or `FOLIA_TEST_SNAPSHOT`) and fails without one.
**The real catalog** is the one betula.app serves (owner, 2026-10-01): `/api/db` is the snapshot
Radix exported, byte for byte (44 MB; `--compressed` takes it in brotli or gzip).

```bash
curl --compressed -o snapshot/catalog.db https://betula.app/api/db
FOLIA_TEST_SNAPSHOT=snapshot/catalog.db cargo test -p folia-server
```

To serve it to a local Folia as well (§3), give it the name and the pointer `radix export` writes
(`snapshot/` is git-ignored), then `radix serve-snapshot` publishes it:

```bash
cd snapshot && hash=$(sha256sum catalog.db | cut -c1-32) && mv catalog.db "catalog-${hash:0:16}.db" &&
  printf '{"file":"catalog-%s.db","etag":"\\"%s\\"","bytes":%s,"exported_at":"%s"}\n' "${hash:0:16}" "$hash" \
    "$(stat -c%s "catalog-${hash:0:16}.db")" "$(date -u +%FT%TZ)" > current.json
```

Then `cargo test` finds it without `FOLIA_TEST_SNAPSHOT`.
Without a crawl, a synthetic one serves for development and for the browser checks below
(`radix/internal/catalogbuild/folia_fixture_test.go`: 1,200 modules with varied facets, 115 programs
with trees, areas and degree labels, Informatik B.Sc. and Elektrotechnik B.Sc. with validated
plans, a lecture for every ninth module — dated in the semester the build date makes the current
one, spread over its week in each campus's grid of times, a few in the A or the B weeks only, so
the finder and the Stundenplan compare real slots whenever it is made; and the modules the
browser checks look for, as the real catalog has them: Datenbanken (12330) with its four badges
and the only title with „Datenbank", Analysis I (11103) with an exam as QIS enters one without a
date, 12000 with a deadline at 23:45–24:00; numbers made up, nothing of it says anything about
the BTU; the checks that compare pinned or real-data numbers fail on it, everything else runs):

```bash
BETULA_FIXTURE_DIR=$PWD/snapshot go test ./internal/catalogbuild -run TestWriteFoliaFixture -count=1
./radix serve-snapshot --dir snapshot          # then cargo run -p folia-server as usual
```

What `cargo test` checks:

- `layers` (`folia-test-support`, needs no snapshot): every crate uses only what its layer may
  (`folia/layers.toml`): the layers below its own and those before it in its own, features no
  feature, the UI no `folia-query`.

- `pack` (needs no snapshot): every shape of serde's data model there and back, the codes of
  fixed values (the format is frozen), a field added at the end read from older codes, versioned
  codes (the layout in four bits, another layout named, never read as a plain code), what the
  format refuses; every character typed wrong, every swap of neighbours and of characters one
  apart is caught in codes of several lengths and kinds; codes that check out but hold garbage
  are refused without a panic, and without more work than their length allows.
- `catalog`: every filter against direct SQL (exclusions included), exact totals and paging, the
  pinned numbers, enum labels from the CHECK constraints, every query and page loader against
  real data, the URL codec, the ranking of the pickers (`fuzzy`), the search (folded as Radix
  folds, from `radix/internal/normalize/testdata/search.tsv`; words in any order, numbers and numerals,
  the order by relevance, typos, the most words, what it finds outside the filters);
  `SCHEMA_VERSION` is the number of Radix's newest migration, and the snapshot of the tests is not
  older.
- `server`: a fake Radix over HTTP: not ready → 503; download, check, gzip, activate, brotli; 304 →
  no download; pages render, cache (`hit`/`miss`), revalidate; equal filters share a cache key;
  404 is never cached; pages and files in brotli, gzip or plain as the client takes them, files
  `immutable` under the build's own address and `no-cache` under any other, a `304` that says how
  to keep; `/api/db` with Radix's ETag, brotli once made, gzip and 304; `/api/status` with the
  snapshot's schema, `boot.js` with the build's; a broken export is rejected and the old snapshot
  stays; one of an older schema is served; a new one invalidates pages; restart without Radix;
  one description and one absolute canonical address per page, `noindex` on a filtered list,
  the sitemap, robots.txt as Google reads it (every page of the sitemap and of the catalog open,
  the views of the lists closed) and no link a crawler may follow to a view, the map of the
  programs as laid out with the snapshot. Closed testing (needs no
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

crawls the running site like a search engine: every program in each view its sidebar links (the
plan and the areas for search engines, „Mein Plan“ `noindex`), the whole catalog page by page (the
pages must add up to the header's total), module pages, the 404s (the former `…/modules` among
them). Last run: 821 pages, 0 failures, slowest page 291 ms (the first, `/`) on a debug build.

```bash
cd e2e && node spa.mjs
```

drives the browser app in Edge: waits for the takeover, then opens a preview (the list must keep
its scroll position), filters, closes with Esc, opens the full page, goes back, scrolls the
virtual list (as long as the whole list from the start, a slice rendered, the last rows there at
its end, `page` following, the length unchanged), searches programs, and fails on any page load
after the takeover or any console error.

```bash
cd e2e && node search.mjs
```

searches modules („The search of the catalog“): without JavaScript, the server's page says a
corrected typo above the list and the matches outside the filters under it; in the browser app, a
search inside Informatik B.Sc. says what it finds outside the program and its link leads there; with
a stand-in for the semantic search (`window.betulaSemantic`; the repository has no model) „Ähnliche
Module“ stand under the results without the result and the module no longer offered, one of them
opens beside the list, and the arrow keys go on from the list's last row into them; a typo is
corrected and said (and the semantic search is asked for the corrected word), the best match comes
first, „Modul“ orders the matches by title, and typing orders them by relevance again. Last, a
search the address carries: its list asks before `boot.js` offers the semantic search and still gets
its „Ähnliche Module“.

```bash
cd e2e && WORKER_SNAPSHOT_DIR=../../snapshot node worker.mjs
```

checks the data worker: the page's thread holds no catalog (no sql.js, no `betulaDb`), the list
is on screen in every frame up to the takeover, a filter and a module beside the list are
answered by the worker, a second tab downloads nothing. With `WORKER_SNAPSHOT_DIR` (the
directory `radix serve-snapshot` serves, Folia polling it often: `FOLIA_SNAPSHOT_POLL=2`) it
writes a newer snapshot, waits until the server has it (in a debug build after the brotli of the
one before, minutes) and checks that the open page shows it in place, staying where it was, and
that the other tab gets it too; then it puts the pointer back.

```bash
cd e2e && node typing.mjs
```

types in the search of the top bar once the data worker answers („The search of the
catalog“, „Typing“): four words, a key every 200 ms, two of them with the CPU slowed down four
times. The page's thread runs none of the search's queries meanwhile, the list
follows what was typed, and „Ähnliche Module“ (a stand-in for the semantic search that names the
list's own rows first) hold none of the list's rows; a module searched by its number (14851) does
not find its namesake (14508, „Anti-Gewalt-Arbeit“) among them. It prints how long the keys waited
for the page's thread (Event Timing) and its long tasks, and fails where a key waited longer than
100 ms without the slowdown. Last run (2026-10-02, a cloud container of four cores, the dev
bundle): without the slowdown keys waited up to 56 ms, the longest task 72 ms; slowed down four
times up to 163 ms, the longest task 190 ms.

```bash
cd e2e && node filters.mjs
```

drives the filter panel: a toggle through its three states (the panel must stay the same element
and keep the focus), rows of toggles filling the width, oversized hit areas, the search field
aligned with the list, the program picker (typo, arrow keys against a resting mouse pointer, wrap
around, Enter, focus back on the button, Esc closing only the picker, click outside, clear, no two
entries alike), the lecturer picker, the slider (drag, keyboard, knobs not crossing, typed
numbers), the panel's width (limits, `localStorage`, reset), the list of a program (plan order,
the semester at every row, no headings, the second page reached by scrolling; a semester listing
more than the plan places in it, with the note saying what the plan asks for and that it is
derived) with the area picker (an elective area filters the list, the tag above it, the panel not
rebuilt; only the areas to choose from, under the heading of the area above them, named without
„Wahlpflichtmodule"), and the same panel without JavaScript (links keep the rest of the filter, the form keeps
what the links set, nothing that needs JavaScript is visible, a shared address with `open` shows
the plain list, a row leads to the module's page).

```bash
cd e2e && node languages.mjs
```

drives the browser app in English (`docs/folia/i18n.md`): it takes over on `/en/catalog`, and a filter,
a preview, „Vollbild", Esc, the programmes, a programme and its areas, the Merkliste, the
Stundenplan, the start page and the search of the top bar all stay under `/en` without a page load,
with no link of the page leading into German but the switch; the switch in the rail then loads the
same address in German.

```bash
cd e2e && node module.mjs
```

drives a module in its two sizes. Desktop: preview → page with the sidebar exactly where the
filter panel was, the same order of sections in both, jumps without history entries, one width
for sidebar and filter panel, Esc back to the list with the row in view. Phone: a tap opens the
page directly, the page starts with times and facts, back returns to the tapped row deep in the
endless list, a shared preview link becomes the page; „Einplanen" and „Merken" on a line of
their own under the badges, filling it half each.

```bash
cd e2e && node phone.mjs
```

drives the catalog on a phone, with real touches (`Input.dispatchTouchEvent`, so the browser
scrolls as it would under a finger): the page behind the open sheet standing still (swipes on
the dimmed page, the sheet's list scrolled past its end), the sheet swiped down from its list,
its head and its buttons (following the finger, snapping back after a short slow pull), a mouse
drag at its head and a tap beside it closing it, the draft (taps change the sheet's count but
neither the list nor the address; its button, a swipe and the close button apply it with one
history entry; „Zurücksetzen" empties it), the area picker staying open while the window
shrinks (the on-screen keyboard) and counting its modules, and the virtual list with the window
scrolling (the last rows at its end, the page keeping its height, no two rows overlapping,
`page` following).

```bash
cd e2e && node studyplan-phone.mjs
```

drives the Stundenplan's week on a phone, with real touches as `phone.mjs` does, on any snapshot:
it plans modules of the current semester held in A weeks, in B weeks and every week, which it
finds in the snapshot the server serves (`/api/db`, read with the app's own sql.js). The week is
a grid as wide as the window, three of them (A/B shown, the others out of the pointer's and the
focus's reach) with their tabs under them and no switch in the head; each slot tall enough says
where it is held; a swipe to the right brings the week before, carried by the finger while it
moves, and opens nothing; two more end at the A-Woche; a short move changes nothing, a move up
scrolls the page; the list of days is closed under a line that counts the week shown, opens with
it and follows the tabs; a slot opens its module, and back the week is in view as it was;
„Kalender" stands under the Termine in the views „Woche", „Termine" and „Prüfungen" and not in
the sheet; a wide screen keeps it in the sidebar and the switch in the head; a plan without A or
B weeks has the grid alone.

```bash
cd e2e && node tabbar.mjs
```

drives the phone's bottom bar with real touches as `phone.mjs` does, each carrying its time so
that a flick is a flick however slow the protocol: a swipe to the left goes one tab to the right
and one to the right one tab to the left, from wherever on the bar it starts, as one step of the
history and without a page load; while the finger is down the row of tabs follows it and the mark
stays (the bar's lens, over the tab's and in its colour, with a copy of the row inside that lies
over the row) and the page stands still; let go, the row and the lens glide as Web Animations,
and once the tab is current its own mark is back and nothing of the swipe is left on the bar; a
long pull goes one tab and no further, a short slow one glides back, a short flick goes on, at
either end the tab stays; a finger catches the glide with the lens where it is and goes on from
there, two swipes in a row go two tabs, a tap right after a quick swipe is a tap, and a finger up
the bar scrolls the page; while the app is starting (its bundle held back for 3 s) a tab tapped
and a swipe are current at once, the page is not loaded again, and the app shows the last one's
page once it runs; where the app does not start (its bundle fails) a swipe loads the tab's page.

```bash
cd e2e && node swipe.mjs
```

swipes a row of the catalog on a phone with real touches as `tabbar.mjs` does: to the left the
card follows the finger (less the slop) inside its place, the page neither moving nor growing
wider, and uncovers „Merken", quiet until the action is armed and then in the inverted colour;
let go, the ground says „Gemerkt", the module is marked (its bookmark, the Merkliste's count, the
store) and the row comes to rest with nothing of the swipe left, no step of the history and no
page load; to the right „Einplanen" with the semester, armed in the accent, „Eingeplant", the
Stundenplan counting the module; again each way „Entfernen · von der Merkliste" and „Entfernen ·
aus …" take them out; a short slow pull does nothing, a short flick marks, and the bookmark tapped
right after it is a tap; a finger up a row scrolls the page and takes no row; a tap on another row
right after a swipe opens its module; on the Merkliste a module swiped off it stays, dimmed, its
card whole while it is swiped again and marked again by that swipe, and one swiped to the right is
planned; a mouse in a narrow window drags the card and marks, its drag no click; a wide screen's
row takes no drag.

```bash
cd e2e && node ground.mjs
```

drives the ground on a wide screen with a real wheel: the window without a scrollbar and without
room while the page is not at its end (the wheel over the header or the rail moves nothing), the
ground coming up at the end with the rail and the header standing, the page's end and the panel
beside it 8 px above it, the panel cut off and not scrolled by the wheel, upwards the ground
leaving first, a short list bringing it at once without its rows moving, the end of the whole
virtual list going up with it, the page leaving its end by its scrollbar and a page opened from
the ground sending it away, Tab bringing it up; on a phone the ground at the end of the page, and under a page shorter
than the window (the empty Stundenplan) at the window's lower edge.

```bash
cd e2e && node top.mjs
```

drives „Nach oben": on a wide screen the catalog's whole list (out of sight at its top and less
than a screen down, in the corner of the list far down; the wheel over it turning the list; a click
bringing the list to its top with its first row, `page` gone from the address, the button gone
and no page loaded; a wheel stopping the way up; with a module beside the list the button left of
it and the module staying; Enter on it, and Tab going on from the start of the page; the module's
page opened from far down starting without it, and Back to the list far down bringing it again), a
long page with the ground in (the button above it, the ground going back down), less motion (up at
once); on a phone the window (the button 12 px above the bottom bar, a tap), and a page far down
before the app takes over (the app's own button there after it); the classic site with the app
kept away; without JavaScript no button.

```bash
cd e2e && node pwa.mjs
```

installs the app with the network (the worker has the shell, the bundle included; no status
pill once the app runs), then cuts the network and loads pages afresh: the catalog with a
filter, a module page never seen before, the program page seen before, and a step inside the
app out of the local catalog.

```bash
cd e2e && node launch.mjs
```

checks the launch screens of iOS: an iPhone (with `navigator.standalone`, as iOS has it) names
the two pictures of its screen, upright, light and dark; an iPad also the turned ones; a desktop
browser none. Every picture named is served as a PNG exactly as large as the screen. Needs no
snapshot.

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
„Vollbild" of a module opened from the marked modules in place (`full=1`, the tab „Merkliste"
still the current one, the module's sidebar, the catalog's tab untouched), „Zurück" and Esc back
to the list with the module beside it and without a new history entry; a mark made in another
tab arriving; a module the snapshot does not know; garbage in the storage. Privacy: no request of the
whole session carries a mark, none leaves the site, and server HTML shows nothing marked. Phone:
a 44 px target, marking by touch, the bottom bar's count, a tap opening the module in the list's
place (the address still the list's) and „Zurück" returning. Without the app: nothing of it
shows without JavaScript, `/bookmarks` explains itself and is `noindex`; with JavaScript but
before the takeover the buttons are invisible and their room is kept (the heading is exactly as
tall as once the app runs).

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
closing again, „Vollbild" in place (`full=1`, the programs tab still current, the module's
sidebar) and back to the program without a new history entry, Esc leaving the program instead of
reopening the module, an area beside the page with its modules (and a module picked out of it
coming back to the area), a requirement of the plan with its numbers and its ways on, the
catalog's tab unchanged by a module seen in full screen out of a program, matrix and list with
the choice remembered in this browser only, the areas as groups of rows with the sidebar leading
to each of them without a history entry, „Mein Plan" leading to all of the program's modules in the
catalog (as many as the head counts), and on a phone the plan as a list without a switch and a
page that does not scroll sideways, a module becoming the page with one tap and one history entry
and „Zurück" leading back, an area becoming the page and a module picked out of it leading back to
the area.

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

```bash
cd e2e && node snappy.mjs
```

drives a click answering first („A click answers first"), frame by frame after each interaction:
a toggle, a row, Esc, the rail into the program overview, a program, one of its views, Back, the
start page and the catalog as it was left, two toggles before the first has reached the list;
then on a phone the bottom bar and a module. The first frame after each has to come within 80 ms
and show what was clicked in its new state (and a skeleton, where the page or the list is being
built for the first time), a later one the result without a skeleton, and no skeleton stays.
All of it once more with the CPU slowed down four times (200 ms for the first frame), where the
skeletons are what bridges the wait. The numbers it prints are the time to the first frame and
to the result.

## 5. Not done yet

- PWA: an update prompt (the worker installs a new build silently; a page keeps the bundle it
  started with). User data beyond „Merken": the prerequisite check of what is passed („Mein
  Studium" keeps it, 2026-10-04). `wasm-opt` for the bundle.
- Phase 3: design system, weekly calendar, filter bottom sheet, search
  with context ranking (own concept, see `docs/history/frontend-phase0.md`).
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

- **Marks in the tables of a program's page** (plan, areas): `bookmarks::MarkButton`
  with `MarkLook::Row` next to a row's link is made for it. Left out on purpose while the page
  itself is being worked on (owner, 2026-09-20); the panel beside the page already has the switch,
  because it is the catalog's module preview.
- **What follows the marks** (R20 applies): the own program and the semester planner are there
  („Mein Studiengang", „Mein Studium" with what is passed, 2026-10-04); the prerequisite check is
  left. A note per marked module would fit the same store.
- **The timetable as a calendar subscription** is built (2026-09-24): `/calendar/<code>.ics`,
  one semester of the Studienplan per code (semester, planned modules, hidden kinds, events and
  Termine, chosen Termine, the Standort; `folia_calendar::subscription`), made anew from the
  active snapshot on every fetch. R20 has the owner's decision, §3 the gate and the log, „Der
  Studienplan" in §1 the rest; the privacy notice's part is „Kalender-Abo"
  (`folia/crates/home/src/legal.rs`), which lists what a code carries. The note of 2026-09-23 (a code of
  event ids) is superseded: a code of modules and hide rules also brings the exams QIS publishes
  later. Since 2026-09-25 an entry is as short as a slot of the week, for the phone (owner: „so
  kompakt wie möglich … so wie die Infos bei der Ansicht auf der Seite"): „VL EvS" in „ZHG/HS.C",
  „Ü AuP · 1 von 3", „Prüfung EvS · 2. Termin"; its description says it all in full (what QIS calls
  the event, the modules with number and title, the rooms as QIS names them, and what the rows say;
  `folia_timetable::export`). A program's abbreviations differ from a module's own in about 7 %
  of its compulsory modules, and the download must be the feed, so the code carries the program;
  and it names the layout of its fields in four bits (`subscription::VERSION`), so a later layout
  is read beside it. Codes of the time before are not read (canary only). Still open: the edge logs
  the address like every address, 7 days in Loki and 7 days in the host's journal
  (`deploy/README.md` §9); a Traefik router for `/calendar/` with
  `observability.accessLogs=false` would leave it out, but needs the blue-green priority label in
  `deploy/vps/50-app.sh`, `55-switch.sh`, `lib-stacks.sh` and `91-verify-stacks.sh`. And a
  subscription over several semesters: today the next semester needs a new address.
