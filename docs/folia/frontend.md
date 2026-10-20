# Folia, the web tier: architecture, rules, how to run it

> Betula has three parts named after the birch: **Cortex** (the bark: the cache between Betula and
> the internet, `docs/cortex/cortex.md`), **Radix** (the root: the Go collector, `docs/radix/operations.md`)
> and **Folia** (the leaves: this web tier, the Cargo workspace `folia/` with its crates in
> `folia/crates/<crate>`, each `folia-<crate>`, in the layers of `folia/layers.toml`: the domain —
> `folia-locale`, `folia-pack`, `folia-model`, `folia-calendar`, `folia-search`, `folia-routes`,
> `folia-query`, `folia-timetable`, `folia-plans`, `folia-semantic`, `folia-pages` — the data
> (`folia-data`), the UI — `folia-design`, `folia-stores`, `folia-shell`, `folia-widgets` and the
> features `folia-home`, `folia-catalog`, `folia-programs`, `folia-bookmarks`, `folia-planner`,
> `folia-profile` — and
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
| `home/`, `catalog/`, `programs/`, `bookmarks/`, `planner/`, `profile/` | The features: each its pages and their texts. None uses another. |
| `app/` | The composition: the document and `App` with its routes. |
| `client/` | The browser app (WASM): `app` with feature `csr`, asking the data worker. Not a default workspace member (its `csr` would be unified with the server's `ssr`); built by `folia/scripts/build-client.sh` into `site/pkg`. |
| `worker/` | The data worker (WASM, no Leptos) and its script: the catalog in sql.js, every question of the app's pages answered beside the page's thread; built with the client. |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets, the drawn cards. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `spa.mjs` (the browser app: takeover, no page loads, preview, filters, the virtual list, search), `search.mjs` (the search of the catalog: typos, relevance, what is meant first and marked, a lecturer written another way, what the filters leave out, „Ähnliche Module“), `typing.mjs` (typing in the search: its questions in the data worker, how long the keys wait), `filters.mjs`, `module.mjs`, `courses.mjs` (a module held at two campuses: every course in its colour, Mein Studiengang's alone with „Auch zeigen", another program's highlighted, the Stundenplan's line naming the course), `choices.mjs` (a module's choices and extras: the exercises of HM T3 dashed and Mein Studiengang's taken with „Auch zeigen", the Aufbaukurs dotted, the groups of Musiktheorie, the 14 projects of Berufspraktische Profilierung in the list alone, the catalog's row; the Stundenplan with HM T3: the choice open, made with „✓" and opened again, „Zusatz", Bauingenieurwesen's exercise preselected and another taken in the panel), `programs.mjs`, `bookmarks.mjs`, `selection.mjs` (several modules at once on the Merkliste: Ctrl, Shift, drawn with the mouse, the keys, the bar's „Sammlung", „Einplanen", „Teilen" and „Entfernen" with „Rückgängig", a row's menu, a finger held on a phone), `share.mjs` („Weitergeben": a collection's „Teilen" with its head, „Link kopieren", „Notizen mitnehmen", the text with each module's link; its link on another device kept as a collection, again with the notes under „Anderer Name …", which refuses a name there is; the whole list for a third device with every collection and note; the selection's link of modules alone; nothing of it in a request), `everywhere.mjs` (the Merkliste from elsewhere: the tab's menu of its views, the catalog's „Gemerkt" per collection and „nicht in …", „Mein Studium"'s Wahlpflicht with the marked module first, „Module hinzufügen" with „Merkliste" by collection and search, the Gesamtplan's cell, the three tabs on a phone, the Stundenplan's „Aus der Merkliste" with „passt", the folded summer's, a collection taken over and „Rückgängig", nothing of it in a request), `phone.mjs` (the phone layout: the sheet, the pickers, the screen's keyboard, the list, a module's sheet from below, a tab held), `back.mjs` (Back on a phone as Android and an iPhone have it and on a wide screen: what lies over a page, the semester of „Mein Studium", the filters' draft, a module closed, a stack for each tab, the profile as a page of a while, a link from outside), `stages.mjs` (the tabs that stay built: a tab seen before in the next frame as it was left, scrolled as far, the title and the tags of the page in front), `achse.mjs` (a change of tab on a phone, the pages on an axis: a tap, Back to Start, taps on the way, a finger along the bar, the ground and the dots with their page, less motion, a wide screen), `swipe.mjs` (a row of the catalog and of the Merkliste swiped on a phone: „Merken", „Einplanen"), `plannote.mjs` („Einplanen" as concept E has it: the dialog for a study not set up, the note with ‹ ›, ← →, „Weitere Module auch nach …" and „Rückgängig", the arrow's menu — passed and back, moved, ⇧P and its keys, taken out —, a row swiped on a phone), `studyplan-phone.mjs` (the Stundenplan's week on a phone), `study.mjs` („Mein Studium": the first visit, the areas, the semester at the top of its box and the list of the semesters at its name, the rows as the catalog's with their status, the rows' menus, selecting several without boxes, adding, moving and taking out, the Gesamtplan, the views — the Regelstudienplan with „Einplanen" and „Rückgängig", „Wählen" into the Wahlpflicht and a module of a list planned —, the phone — its row of pages, the way back with the first move off the overview, the tab and the way back while the row flings, a module's sheet over a semester, the views as pages of their own, „Studiengang wechseln" with the screen's keyboard —, the Stundenplan's import), `home.mjs`, `snappy.mjs` (a click answering in the next frame, skeletons), `top.mjs` („Nach oben"), `languages.mjs` (the app in English, `docs/folia/i18n.md`), `profile.mjs` (the profiles and their page: a new profile's own keys, a switch here and in another tab, renaming, emptying and deleting with „Rückgängig“, the menu of the profiles at the rail's way and the phone's box with „Neues Profil“, the profile page on top of the list it was opened from and its arrow back to it, light, dark or the system's with the browser's chrome and the manifest, a profile as a file and back, the phone), `schema.mjs` (a local copy of the catalog of an older schema, with and without a network), `worker.mjs` (the data worker: no catalog on the page's thread, no blank moment at the takeover, one download for two tabs, a newer snapshot shown in place), `takeover.mjs` (nothing moves at the takeover: the server's page, the picture in front of the app and the app's page box for box, on a phone and a wide screen, at the top and scrolled down), `smoke-walk.js` + `run.mjs` (long program walk), `shot.mjs` (review screenshots). All use an installed Edge through `playwright-core`; without one, `node --import ./chromium.mjs <check>.mjs` runs a check in Playwright's Chromium or in the browser `SMOKE_BROWSER_PATH` names. |

### Routes (`folia/crates/routes/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `area`, `kind`, `lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `events`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `marked`, `prereqs`, `sort`, `desc`, `page`. What can be wanted can also be excluded: `not-kind`, `not-lecturer`, `not-turnus`, `not-form`, `not-exam`, `not-campus`, `not-lang` (`exam=written&not-exam=presentation`: a written exam and no presentation). `area=<id>[,<id>…]` are areas of the selected program's module tree („Wahlpflichtmodule Praktische Informatik"): the modules the tree places in any of them or below one (several come from a row of the plan that means several areas, opened from the program's page; the picker then says „5 Bereiche", one tag per area above the list) |
| `/catalog?…&open=<id>` | In the app: the same list with this module previewed next to it; the preview has a „Vollbild" link to the module's page. On a phone the preview is a sheet from below over the list, without „Vollbild" (2026-10-06, 2026-10-16, „A module on a phone" below; until then a tap on a row opened the module's page, and the app turned a shared `open` link into it). The server's page (crawlers, no JavaScript) ignores `open`: it renders the plain list, every row leading to the module's page (owner decision 2026-09-21: the server's HTML is for crawlers, the app for people, and no query parameter changes the server's layout) |
| `/catalog/module/<id>` | The module's own page: a sidebar as wide as the filter panel (sections of the page, actions), the module on the rest of the screen |
| `/programs?q=…&level=…&form=…&plan=1` | Program overview (current PO versions) by faculty (`ProgramsUrl`): the search of the top bar, degree (`bachelor`, `master`, `teaching`, `doctoral`, `other`), form of study (`dual`, `double`, `flexible`), only with a validated study plan |
| `/study[?view=plan\|areas][&open=<id>][&full=1]` | „Mein Studium“ (`StudyUrl`, 2026-10-04): the visitor's study semester by semester, the first page of the Studium tab in the app (see „Mein Studium“ below). What is passed and planned lives in the browser; the address says only how it is shown: which view (`view`, 2026-10-14: `plan` the Regelstudienplan, `areas` the Wahlpflicht, none the semesters; on a phone each a page of its own that Back leaves), which module stands beside the page, and whether it fills it. The semester a phone's row shows rather than the overview is no part of the address since 2026-10-18 but the state of its entry of the history, which a reload, Back and Forward bring along and which writes no step („Back" below); from 2026-10-05 it was `plan=1`, a step after the overview, which is still read (and links the box „Studium planen" without JavaScript) and gives way to `/study` once the row is back at the overview; a desktop shows both at once. The server renders one stand-in for everybody, `noindex`, not in the sitemap |
| `/programs/<slug>/plan\|areas\|my-plan[?variant=<n>][&area=<id>][&req=<n>][&open=<id>][&full=1]` | Program page (`ProgramUrl`); its views are switched in the sidebar: the Regelstudienplan (`plan`) and „Wahlpflicht & Bereiche“ (`areas`). „Mein Plan“ (`my-plan`, the visitor's, `noindex` and not in the sitemap, `ProgramTab::indexed`) took the place of „Alle Module“ on 2026-09-25 (the program's modules are its catalog, `/catalog?program=<slug>`, and `…/modules` is a 404) and became „Mein Studium“ on 2026-10-04: the views no longer list it, the app goes on from its address to `/study` in the same history entry, and the server's page there says so. Where a program has several study plans (one per study direction), `variant` says which one is shown; `area` is the area of „Wahlpflicht & Bereiche“ shown beside the page, `req` a row of the plan that names no module, `open` the module — they stand in the address (a shared link, the history) and the app renders them; the server's page ignores all but `variant` (it lays nothing beside itself: its module links lead to the module's page, its area links to the catalog narrowed down to the area, a row without a module is text), so they are no part of its cache key and its canonical address is the plain one. The plan of each further study direction is a page of its own (2026-09-26): `?variant=<n>` is its canonical address, listed in the sitemap, with the direction in its title; the first is the plain address, and a number past the last plan names the last. A module opened out of an area keeps it, so closing the module returns to it. `full=1` shows the module of `open` in full: the module's own page, in place, so that „Vollbild" stays in the programs area (its tab, its history, its „Zurück"); the canonical address of that view is the module's page. On a phone an area or a row of the plan picked is the page, and a module comes up as a sheet over the page or over what was picked (2026-10-06; until then `open` alone showed it in full there) |
| `/bookmarks?collection=…&turnus=…&stand=…&counts=1&lang=…&ects_min=…&campus=…&graded=yes&fits=…&sort=…&desc=1&open=<id>[&full=1]` | „Merkliste": the modules the visitor has marked (`BookmarksUrl`). The URL says how the list is shown (the view — a collection by its number, `none` for „Ohne Sammlung", never its name —, half of the year, „Stand", „Zählt in meinem Studium", the few of the catalog's filters it has under the catalog's names, the order — `added`, `study`, `offer`, `title`, `ects`, `events`, each turned round with `desc=1` —, the previewed module), never what is on it: the marks live in the browser, and so does what is typed into the search at the head on this page (`folia_shell::search::LocalSearch`, R20). `full=1` shows the module of `open` in full, in the list's place, as `full=1` does on a program's page (a local view, `folia/crates/widgets/src/local.rs`): „Vollbild" stays among the marked modules (their tab, their history, their „Zurück"); on a phone `open` is a sheet over the list (2026-10-06; until then it did what `full=1` does). The server renders an explanation, the same for everybody, `noindex` |
| `/studyplan?sem=…&view=…&open=<id>&row=<key>&import=…&variant=<n>[&share=<code>]` | The Stundenplan (`StudyplanUrl`): how the plan is shown, never what is in it (R20), with one exception: `share`, a semester of a plan handed on by a link (`folia_calendar::share`, owner 2026-09-26), which the page offers to take over. The server renders an explanation, `noindex`, the same for everybody; for a `share` code a page of its own, whose tags and picture name the plan's modules (a link preview runs no JavaScript) |
| `/profile` | „Profil" (owner, 2026-10-14): the profiles of this browser and the active one's page — its name (renamed, deleted), „Mein Studiengang", what it keeps with the way to each and „Profil leeren", the settings every profile shares (light, dark or the system's; the language); in the sidebar the profiles (a click switches), „Neues Profil", and a profile kept as a file or made of one. What it shows lives in the browser; the address is the page and says nothing more. A page of a while: on a phone its arrow leads back to where the visitor was (2026-10-19). The server renders one stand-in for everybody, `noindex`, not in the sitemap („Profiles" below) |
| `/impressum`, `/datenschutz` | The legal pages (`folia/crates/home/src/legal.rs`): the Impressum and the Datenschutzerklärung, final since 2026-09-25 (placeholders from 2026-09-21). Linked from the ground at the end of every page („The birch"; § 5 DDG: reachable at all times), on a phone all but the row of pages of „Mein Studium", which has no ground (owner, 2026-10-19): there they are a tab further, at the end of any other page. The privacy notice says what the software does — the edge's access log and its retention, Folia's log, what stays in the browser, the calendar feed, the gate's cookie, the lecturers' names (Art. 14 DSGVO) — and `legal.rs` names the source of each part: a change there is a change of the text. `legal::PLACEHOLDER` stays the switch `deploy/ship.sh` reads: true again, the pages are `noindex` and no instance open to everybody (`FOLIA_ACCESS_GATE` not `on`) ships |

The catalog parameters are tolerant (repeated or comma-joined values, empty inputs of a plain
HTML form, nonsense ignored) and have one canonical spelling, which is also the cache key. A
value that is both wanted and excluded counts as wanted. An exclusion removes only what the data
states: a module whose campus or turnus is unknown stays in the list (R12). Lecturers: the
wanted ones are alternatives, the unwanted ones are all left out: (Meer or Köhler) and not
(Lambers or Hofstedt).

**Every route in every language** (2026-09-27, `docs/folia/i18n.md`): the addresses above are the
German pages; the same page in English is the same address under `/en` (`/en/catalog?…`, the start
page `/en`), and so are the cards, the manifests and the calendar feed (`/en/cards/…`,
`/en/manifest.webmanifest`, `/en/manifest-dark.webmanifest`, `/en/calendar/<code>.ics`). `/de/…` leads to the plain address. The
paths inside the app never carry the prefix (`folia_locale::Locale::path`/`split`, R23).

### Look and interaction (since 2026-09-19, owner-approved direction)

- **Betula does not say no** (owner, 2026-10-18; the whole principle is in `CLAUDE.md`, „Design"):
  what a student asks for and is easy to do is allowed, with a note or a warning where it is
  unusual, rather than a disabled control. A limit needs a reason that holds (what cannot be),
  not the usual case; defaults follow the usual case. The first to follow it: planning a module
  into a semester behind the student, with „liegt zurück" or a warning
  (`folia/design/einplanen/oberflaeche.html`).
- **Name and logo** (owner decision 2026-09-20, domain `betula.app`): the product is **Betula**
  (the birch; B-T-U stands in the name). Running text says „Betula", only the wordmark stresses
  the three letters: BᴇTUʟᴀ in Inter 800 with E, L and A as small capitals and the E under the bar
  of the T (`ui::Wordmark`, `.wordmark`; the spacing is measured, not guessed). The rail carries
  the icon, the top bar of the start page the wordmark with „Modulkatalog · inoffiziell", so the
  two read as one logo; on a phone the start page carries both itself. „Inoffiziell" always
  stays with the name, and nothing borrows the university's colours or mark. Sizes, grids and
  numbers: `folia/design/logo/logo.html`.
- **The icon is a birch leaf in paper on a square of ink** (owner, 2026-10-15: „Tinte", „mit
  Abstand das beste"), with four bars of birch bark cut out of it that also read as the rows of a
  list, entering from its edges left and right in turn; in the dark it turns over, an ink leaf on
  a light square (owner: „im dark modus muss das invertiert werden"). It is the same everywhere
  („das neue icon komplett verwendet"): the rail, the start page and the login page
  (`ui::Mark`, in `--logo-tile` and `--logo-leaf`), the tab, the link previews, the home screen,
  the splash screens and the start of the installed app, and the site's results in Google Search.
  The leaf stands whole in any shape a launcher cuts and in one colour for Android's themed icons
  (owner, 2026-09-26, of a mark whose bars ran out to the edges of its square: „sieht bei einer
  Kugel mit Material You wirklich nach nichts aus"). Before, the site had a mark of its own (a
  white square with four ink bars) and the app's icon was this leaf, white with a stem on a green
  gradient (owner, 2026-10-15: the colours clashed, the gradient looked retro).
  `folia/design/logo/app-icon.mjs`, described in `logo.html`.
- **Layout:** a thin icon rail (52 px), a top bar with the search, and the whole remaining screen
  for content, with 8 px gaps and 9 px corners. The catalog is three panels side by side: filters,
  list, and the preview of the selected module (`open=<id>`). The preview floats above the list,
  docked to the right edge, so the table is never resized; its width is changed by dragging its
  left edge (arrow keys work too, a double click resets it) and remembered in `localStorage`.
  Selecting a module keeps the list, the filters and the scroll position; on a phone the preview is a sheet from below,
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
    opens in place, under its button and across what the picker stands in, a dialog, a sheet or
    a panel (owner, 2026-09-29: „Komboboxen auf dem Handy sollen immer die volle Breite einnehmen
    … von ganz links bis nach ganz rechts. Kein Überstand"; 2026-10-17: „Eingabeboxen/comboboxen
    immer von ganz links nach ganz rechts mit 8pt margin zum Rand in der Mobile Version"): 8 px in
    from either edge, however far in its button stands (the way in's, beside the marks; a card's
    row in a dialog), and never narrower than the button; no name it lists widens it (a long one
    pushed the popup and the button out of the panel). A button that spans its line (nothing
    beside it, its picker across what it stands in but for 40 px at either edge, `nav::popup_place`)
    spans the list's width with it while open, its content where it was, so that the two are one
    box (a card's row's comes out over the card's edges so); one among the words of a line (a step
    of the way in, a bar's) keeps its size. Until 1.12.0 the popup stood 16 px in from a panel's
    edges and in a dialog no wider than the button, under a button narrower than it. A sheet's
    fields stand 8 px off its edges too (the catalog's filters, the Stundenplan's). There nothing
    that moves the window closes it: the on-screen keyboard that opens for the search field
    shrinks the window and scrolls the field into view, which used to close the popup the moment
    it opened (and what the keyboard does to a dialog, a sheet and the picker: „The screen's
    keyboard"). The module comment of the component lists what
    keeps it predictable (it is a rewrite: the picker of the old frontend lost its mark to the
    mouse, closed the preview with Esc and knew its selection by label). Without the app the same
    places hold a plain `select` or text field inside a GET form, and hidden inputs carry what
    the links have set.
    **A choice of a few entries** (a Studienbeginn, a study direction, the plan's semester in
    „Module hinzufügen", the Stundenplan's plan, Fachsemester and placeholder) is `combobox::Select`
    (owner, 2026-10-16: „Dropdown menus allgemein sind im browser styling passe das mal an, so dass
    das mehr mit der combobox matcht"): the combobox's button and popup without its search field,
    in place of the browser's `<select>`, whose list Windows drew as a blue bar on grey and a phone
    as a wheel. The button keeps the focus while the popup is open (the select-only combobox of the
    ARIA patterns): ↑ ↓, Pos1, Ende, Bild ↑ ↓ mark an entry, Enter or the space bar takes it, Esc
    closes the popup and nothing else (in a dialog the dialog stays open), letters jump to the entry
    they begin; an entry's detail stands beside it („WiSe 2025/26 · jetzt 3. FS" is „WiSe 2025/26"
    and „jetzt 3. FS"). On a phone it opens in place, as the combobox does. `small` is the button
    of a bar of tools (34 px, 46 on a phone); the button and its entries carry their value in
    `data-value` (the checks read it, as they read a `select`'s value).
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
    (`enhance.js`; a page's dialog that a phone shows as a sheet from below, `dialog[data-sheet]`,
    is dragged down the same way and closes as its × does, owner, 2026-10-05). A touch decides with its first move: downwards where nothing inside the sheet
    is scrolled down (the head, the row of buttons, the top of its list) it drags the sheet,
    anything else inside scrolls the sheet's list. **The page behind an open sheet stands still**
    (owner: scrolling it along felt „richtig clunky"): `html.sheet-open` takes the page's
    scrolling away, the sheet's list keeps its scroll to itself (`overscroll-behavior`), and a
    touch on the dimmed page moves nothing.
  - **What is picked in the sheet is a draft** (owner, 2026-09-22: every tap rebuilt the list
    behind the sheet, 250–540 ms of a phone's time with the CPU throttled 4×; now the tap costs
    the panel and one count). The panel shows the draft, its button counts the modules it holds
    (`pages::catalog_summary`, which says what `pages::catalog` says about the same filter, the
    semester of a plan included), and the list and the address follow once, when „Module
    anzeigen" applies it, as one history entry, not one per tap. Any other way of closing the
    sheet — its ×, a swipe, a tap beside it, Esc, Back — throws the draft away and leaves the list
    as it was (owner, 2026-10-18: „Die Filter sollen bei der Mobile Version verworfen werden, wenn
    man das Fenster schließt ist zurück macht"; until then each way applied it). The links of the
    panel stay links (they work without JavaScript and on the desktop, where every change still
    applies at once): on a phone their address becomes the draft. This is the one place where a
    filter lives outside the URL, for as long as the sheet is open.
  - **Back closes the sheet**, as in an app, and the sheet has no entry in the history: on Android
    Back is a close request to its `CloseWatcher` (`enhance.js`); on an iPhone, which has none,
    Back leaves the list and the sheet goes with it (§ „Back" below). Until 2026-10-18 the open
    sheet was a step of its own in the history. The programs overview's sheet applies every tap
    at once.
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
- **A module spends the height of a phone on what is read** (owner, 2026-10-17, of the module's
  sheet on a phone, „bestimmt ist das teils auch auf die pc Version übertragbar"; the same parts
  everywhere, `folia/crates/widgets/src/module.rs`, app.css, enhance.js „a module's Termine"):
  - **The Termine under the week are folded** („die Termine können ruhig ausklappbar sein, so dass
    nicht so viel vertical space verschwendet wird"): a quiet line „Termine als Liste (5) +" as the
    Stundenplan's, which unfolds and folds the list as a thing does, its height from nothing to its
    own and back (`details.evfold`, a `<details>` that opens without JavaScript too). A slot of the
    week is a link to its lines (`GridSlot::lines`, `termin-<n>`): tapped, it unfolds the list, is
    marked as the list's line is (the accent), and its lines light up a moment („dass wenn man auf
    einen Termin im Kalender klickt, dass sich die Termine automatisch ausklappen und dann der
    angeklickte Termin highlighted wird"). The week stays where it is („Idealer Weise ohne den view
    zu verschieben aber wenn sich sein muss, dann mach eine animation rein"): the list unfolds under
    it, and only where a line is out of sight then does what scrolls glide, as little as brings it
    in (a third of a second and a little more the further), while the list unfolds; a sheet half up
    goes all up for it on its spring. Folded again, the marks go. Without a week (Termine without a
    time) the list is all there is and stays open; the exam dates stay open („Prüfungen einfach
    ausgeklappt lassen").
  - **The facts stand two side by side** where their values fit („Da könnten einige auch
    nebeneinander passen … so dass möglichst effizient vertical space frei gegeben wird"; a phone
    had them one under the other): each half the line, its value broken between words where it has
    to („Zentralcampus" over „Cottbus") and no narrower than its longest word, so one whose word does
    not fit beside another takes a line of its own, and so does the one left alone before it; a
    value longer than 24 letters (`FACT_BESIDE`), the persons and the teaching forms always do. On a
    phone the boxes are a little tighter. The eight facts of a module took eight rows of a phone of
    390 px and take five (Prüfung and Benotung, Dauer and Plätze, Sprache and Standort, then the
    persons and the teaching forms): 313 px instead of 506.
  - **„Studiengänge" is the last section** („die Studiengänge können ganz an den Schluss das
    interessiert einen wahrscheinlich eh nicht"), after the texts, on a wide page at the end of the
    description's column, and shows five curricula at most („zeige da maximal 5 mit
    Auslauffunktion"): the fifth runs out under „Alle 12 anzeigen", which unfolds the rest and goes,
    the focus on the sixth; without JavaScript all of them. On a phone a program's kind stands under
    its line, where the name keeps its width and a long kind is whole („Wahlpflich…" was cut off).
- **A module held at two campuses shows the course of the program** (owner, 2026-10-19: „das Modul
  Prgrammierpraktikum [ist] teil von mehreren Studiengängen, wobei ein Teil der Studiengänge in
  Cottbus und ein andere teil in Senftenberg ist. Deswegen gibt es zwei Vorlesungen … Aber es wird
  nur erwartet, dass die Studies eine der beiden vls/praktika besucht"; schema 15). Radix reads the
  courses and which one each program takes (docs/radix/schema-v2.md, „Tracks": whom QIS lists an
  event for, else the campus the program is taught at — where QIS lists its other events, else its
  compulsory modules —, with the SWS as the check of plausibility and no courses where QIS lists
  the same programs at each); `v_module_schedule.track` carries a date's course, `v_module_track_program` the
  programs' (`ModuleData::tracks`). Seven modules of 2026W, the Programmierpraktikum (12102) at the
  Zentralcampus for Informatik, Mathematik and five more and in Sachsendorf for Medizininformatik
  among them; Betriebssysteme und Rechnernetze (11760), whose two campuses hold its two required
  parts, has none, nor has the Research Module in Artificial Intelligence (14060), whose seminars
  at both campuses QIS lists for AI M.Sc. alone (Radix 0.9.1).
  - **The module's Termine** (`folia_widgets::courses`, `module.rs` `course_view`, `shown_slots`,
    `list_groups`, `course_note`) for whom the page stands: with „Mein Studiengang" (and the
    catalog filtered by it, or by another PO of it) only the courses it takes, in the week and the
    list, and a line over the week, „Nur die Termine für Medizininformatik · B.Sc.
    (Cottbus-Sachsendorf). Auch angeboten: Zentralcampus Cottbus." with „Auch zeigen", which brings
    the others back faint (Betula does not say no, „Design" in `CLAUDE.md`). Where the page stands
    for another program — the catalog filtered by it (`CatalogQuery::program`), its program page
    and a module in full there (`ModulePanel`/`ModuleFull`/`ModuleInPlace` `program`) — its courses
    as they are and the others faint („die anderen etwas leichter anzeigen"): „Hervorgehoben: die
    Termine für Informatik · B.Sc. (Zentralcampus Cottbus). Blass: Cottbus-Sachsendorf." Without
    either, on the server always (R9), and for a program Radix decides nothing for (it says so),
    every course in its colour (`course-1` … `course-4`, app.css: a stripe on its slots, a dot before
    its campus in the line over the week and before its heading) and the list under one heading per
    course, „Cottbus-Sachsendorf · für Medizininformatik · B.Sc." (three programs, then „und 4
    weitere"), the Termine in no course last („Für alle Angebote"); the line over the week says „An
    zwei Standorten angeboten: ● Zentralcampus Cottbus und ● Cottbus-Sachsendorf. Du besuchst eines
    der Angebote.", the key to the stripes while the list is folded away. Its tooltip says whether
    QIS names the program at the Termine or its campus decided. Exams are no course's
    and stay as they are.
  - **The Stundenplan** (owner: „hier würde ich erstmal nur die termine anzeigen, die den
    eingerichteten Studiengang entsprechend"; `folia_timetable::model`, `Shown`): of a planned
    module with courses it shows those the plan's program takes (`StudyplanData::taken`, the
    program its abbreviations are of: the plan's, else „Mein Studiengang"; a subscription carries
    it, so the feed is the same). A town chosen in the sidebar („Standort") decides first, the
    program then among the courses in it — 12102's two are both in Cottbus —, else the town
    derived from the plan's other modules; „Beide" shows every course. A course hidden is named by
    its campus („Standort Cottbus-Sachsendorf", `HiddenBy::Course`) and counts for no chip, its
    exams by their town as before; the line „Abgeleitet: …" names what the program decided
    („Programmierpraktikum in Cottbus-Sachsendorf laut Studiengang"), and „Standort offen" asks only
    about a module whose courses are in both towns and that nothing decides. Rows that carry no
    course (a snapshot before schema 15) keep the reading of two towns' courses before it: a
    lecture and a non-lecture event in each town; on canary's data of 2026-10-19 those five
    modules are among Radix's eight. „Passt in meinen Stundenplan" judges a candidate's course by
    the plan's town, as before (not done: by the program).
- **A module's choices and extras** (owner, 2026-10-19: „ich will nicht, dass wir sonderfälle per
  hand eintragen, sondern eine lösung haben, die in die zukunft funktioniert … Bitte bilde alles
  außer daten passen nicht zusammen in der DB und dem UI ab", then of the concept
  folia/design/wahlangebote/konzept.html: „Passt so, nimm deine Vorschläge und bau es"; schema 17).
  Radix reads at every build which events of a module one attends some of, not all
  (docs/radix/schema-v2.md, „Choices"): one event more than once, QIS naming other programs at each
  (`program`: the three exercises of Höhere Mathematik T3, 11206), in groups (`group`:
  Musiktheorie, 13151, „Gruppe 1" to „3" of two events each) or different contents (`topic`: one of
  the 14 projects of Berufspraktische Profilierung, 12672; English for Academic Purposes, 14011, for
  6 SWS of courses of 4 and 2), and the extras beside the compulsory part (a tutorial the module
  states no SWS for, a title naming a support format: the Aufbaukurs of 11206).
  `v_module_schedule.choice`, `option` and `extra` carry a date's, `v_module_choice` the choices with
  their options' events, types and SWS, `v_module_choice_family` whom QIS lists the options for
  (`ModuleData::choices`, `choice_families`; the model `folia_timetable::choices`). On canary's data
  of 2026-10-19: 94 modules with a choice (8 by program, 14 in groups, 72 of topics) and 22 extras in
  21 modules; where the data do not add up, nothing.
  - **The module's Termine** (`folia_widgets::options`; `module.rs` `option_views`, `shown_slots`,
    `role_groups`, `choice_note`): a line over the week per choice — „Die Übung gibt es dreimal, je
    für andere Studiengänge. Du besuchst eine.", „Kontrapunkt/Tonsatz I und Gehörbildung I gibt es
    in 3 Gruppen. Du besuchst eine, beide in derselben Gruppe.", „Wahlangebot: Du belegst eines der
    14 Projekte (je 4 SWS)." or „… Übungen für 6 SWS." — and one for the extras („Zusätzlich,
    freiwillig: Aufbaukurs Höhere Mathematik - T3."). In the week an option to choose is dashed
    (`.slot.alt`), its group or whom QIS lists it for in its third line („für Bauingenieurwesen"),
    its choice in its tooltip; an extra dotted and lighter, „+" before its kind; the options of a
    choice of more than four stand in the list alone (`WEEK_OPTIONS`), and a week left without a
    slot leaves the list unfolded. The list falls into parts with headings: „Pflicht", per choice
    „Eine der 3 Übungen" (its options by their time, a gap between options of several lines, a
    long choice's after its first five behind „9 weitere anzeigen"), „Für andere Studiengänge",
    „Zusätzlich, freiwillig"; a line of an option names whom QIS lists it for where the options
    are for other programs. For whom, as with the courses: with Mein Studiengang the exercise QIS
    lists it at stands as one attended, under „Pflicht" („Die Übung für Bauingenieurwesen · B.Sc.:
    Di 07:30. Auch angeboten, für andere Studiengänge: Mi 13:45 und Do 11:30.") and „Auch zeigen"
    brings the others back faint; where QIS lists it at two of three, it chooses between those;
    another program the page stands for has its options highlighted and the others faint; a
    program QIS lists at none or at each chooses freely, and the line says so. In groups and among
    topics nothing is chosen for anyone (Betula does not say no, „Design" in `CLAUDE.md`): the
    options QIS lists the program at come first, the others fainter and last („QIS nennt Soziale
    Arbeit · Bachelor bei 7 davon, sie stehen zuerst."). On the server (R9) and without a program
    every option is open.
  - **The catalog's row** (`options::row_offer`): where „2 Termine" stands on a phone, in the line
    under the title, „14 Projekte zur Wahl", „Übung je Studiengang", „3 Gruppen zur Wahl" (owner,
    2026-10-19: „finde eine bessere formulierung für den Katalog und die Merkliste, so dass betulas
    Gruppen mechanic nicht mit der von der uni verwechselt wird"; the Merkliste's own are
    „Sammlungen", `folia/design/merken/oberflaeche.html`), and „2 Standorte" for a module held at two
    campuses, of the module's newest semester with teaching. One query reads them for the rows of a
    page (`queries::catalog_offers`, 0.2 ms for 50 rows on canary's data), so the list's own query
    stays as it was.
  - **The Stundenplan** (step 2 of the concept; `folia_timetable::model` `Across`, `take`,
    `clash`, `fit`; the planner's `week.rs`, `aside.rs`, `head.rs`, `side.rs`): a choice among a
    module's events (`Timetable::choices`, from `StudyplanData::choices`, `modules_choices`) is open
    until it is made. In the week its options are dashed with „1 von 3" in their tooltip (a choice
    of two „2 von 4") and „✓ Diese nehmen" / „× Nicht diese"; a choice of more than four options
    stands only in the panel beside the plan (`WEEK_OPTIONS`), whose options say „passt" or
    „überschneidet sich" (`clash::option_meetings`). Over the week „1 Wahl offen", in its list „1
    von 3 wählen: Übung · Höhere Mathematik - T3", or „0 von 3 Terminen frei: …" when no option is
    free („Zu wenige Termine frei für 2 von 5: …"; `Timetable::blocked_choices`). A tap takes an
    option (a Termin of it in `Selection::chosen_rows`, which a subscription carries and a share
    link does not); once as many are taken as the choice asks for, the others leave the week
    (`HiddenBy::Option`) and step back in the panel, where a tap on one takes it instead („Ein Tipp
    auf einen anderen Termin wählt um"); „Wahl aufheben" opens the choice again (of a choice of two
    the one option). For the plan's program (`StudyplanData::listed`, `modules_choice_listed`, by
    its family) Betula preselects what QIS lists it at: as many options as the choice asks for, or
    fewer, are taken (`TakenBy::Program`, the radio filled, „Laut Studiengang; die anderen sind für
    andere Studiengänge."; in „Abgeleitet: …" „Höhere Mathematik - T3: Übung Di 07:30 laut
    Studiengang"), more narrow the choice to them; the others are „für andere Studiengänge"
    (`HiddenBy::Program`), a tap away. An open choice weighs as alternatives (an option meeting
    the plan is no overlap while another is free), and the calendar (.ics, subscription) holds what
    is attended and what is chosen, not the options of an open choice. Extras (`Event::extra`) are
    lighter in the week, „zusätzlich, freiwillig" in their tooltip, in no overlap, and the chip
    „Zusatz" under „Zeigen" hides them (their events in `hidden_events`, so the calendar leaves them
    out too). „Passt in meinen Stundenplan" judges a candidate's choice by its best options: it
    fits when as many of them are free as it asks for (`fit::Candidates::choices`), and the plan
    beside a module's week (the overlay) takes the module's choices as well.
- **A module's number and its abbreviation** (owner, 2026-10-19: „auf der Modul page neben die
  Modul Nummer auch die Abkürzung anzeigen? Bei der Liste muss das nicht sein. Ich glaube das wäre
  tmi"; Folia 1.33.1, `folia_widgets::module::ModuleIds`). At the head of a module's page, of its
  preview and of its panel beside the Stundenplan the number stands in a chip, and beside it the
  abbreviation in a chip of the same look („12104" „EvS"), named „Kürzel" in its tooltip and for
  screen readers. It is the form the Stundenplan names the module by: that of the program the page
  stands for (the catalog filtered by it, its program page), else of „Mein Studiengang" (the
  page's program is the visitor's own where it is of that family, as for the courses), else the
  module's own, which is the server's (R9). A program calls a module otherwise in 961 of 28,573
  pairs, 330 modules on the data of 2026-10-19 (the „Praktikum" 11150 is „Pra" by itself and
  „Prak-b" in Informatik M.Sc., beside another „Praktikum"; `ModuleData::program_abbrevs`,
  `abbrev_in`). The panel beside the Stundenplan takes the plan's form (`StudyplanData::abbrevs`),
  also for a module planned in another semester than the one shown, whose data it asks with the
  page's program since 1.33.2 (before, without one, it named such a module by its own form). A
  list's rows keep the number alone, and the sidebar of a module's page its number.
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
    its shortcut, beside „Einplanen" — the two stand the first way that fits (owner, 2026-10-20:
    „wenn die nicht mehr in eine reihe passen mit den metadate, dass die ersmal nebeneinander in
    ihrer eigenen reihe sind (jeweils 50% platz). Und wenn das auch nicht mehr passt, dann
    untereinander mit voller breite"): side by side at the right end of the badges' line, each as
    wide as what it says; on a line of their own under the badges, half each; one over the other,
    the whole line each (app.css `.hero-line`; before, from 2026-09-26, they stood one over the
    other at the line's end). Which way follows from the line's width alone, never from a click
    (the same day: „das darf auch auf keinen fall springen, wenn man einen knopf anklickt, weil dann
    ja mehr angezeigt wird. Der Einzige punkt, wann es springen darf, ist wenn man das window
    rezised"): each switch is as wide as `--pair` (18rem, on a phone 10rem) whatever it shows, the
    room of its arrow kept while it has none, its label at its start and its key at its end, and
    what it says small takes what is between them and gives way. On a phone the two always have a
    line of their own, half each (owner, 2026-09-27), one over the other where a half is narrower
    than that; among the actions of the module
    page's sidebar, which stays in view while the page scrolls; and on a phone by swiping the
    module's row in the catalog or in the Merkliste to the left (below). All of them show one
    state.
    **`M`** marks what the visitor is at: the row the keyboard is on, else the module that is
    open. A marked module is neutral and strong (filled, inverted), like a chosen chip; the
    accent stays with primary actions.
  - **The rail's fourth item** („Merkliste", also in the phone's bottom bar) carries the number
    of marks. It is a tab like the others (R19).
  - **„Gemerkt" filters the catalog** as well („Eigenschaften": only the marked ones, a second
    click all but them, which is how one looks for what is still missing). Once there is a
    collection, the chip opens under itself which of the marked modules it is about (Folia 1.37.0;
    oberflaeche.html, „Katalog · Filter „Gemerkt"": „wie „Passt in meinen Stundenplan“ seine
    Lehrformen"): „Alle", „Ohne Sammlung" and each collection, with how many each holds
    (`catalog::MarkedViews`), so „nur gemerkte" becomes „in ‚Für Lena‘" and „ohne gemerkte" „nicht
    in ‚Wahlpflicht‘"; the tag above the list names the collection. The URL carries the switch
    (`marked=only` / `marked=none`) and a collection by its number alone (`collection=3`,
    `collection=none`; only with `marked`), never the ids or a name: the browser fills the ids
    into the query before it asks (`CatalogQuery::only_ids` / `without_ids`, `marked_in`), and a
    collection this browser does not have holds nothing. A page that does not know the
    marks therefore matches nothing with `marked=only` instead of answering as if nothing were
    marked, and says so where the list would be empty — the app says that nothing of the
    visitor's fits the other filters, the server that it cannot know.
  - **The list of marked modules** is the catalog's list in the catalog's frame: sidebar, rows
    with the same columns, the preview of `open=<id>` floating at the right edge, on a phone a
    sheet over the list (folia/design/merken/oberflaeche.html, „Suchen, ordnen, filtern"; Folia
    1.34.0). The head counts the marked modules and their credits.
    - **The search at the head searches the list** while the Merkliste is up, as it searches the
      programs on their overview: the catalog's search (`pages::bookmarks`, titles in both
      languages, numbers, abbreviations, lecturers, its typos corrected, the rows marked where it
      found them), among the marked modules, the head saying „2 von 9 gemerkten", and in what
      the visitor keeps of them: the text of their notes and the names of their collections
      (konzept.html: „„Lena“ findet, wobei „Lena fragen“ notiert ist"; the page finds those
      itself and hands them to the data worker beside the text, `BookmarksAsk::also`). What is
      typed never goes into an address or a request (R20): it lives in the page
      (`folia_shell::search::LocalSearch`), and the data worker searches in the browser. Under what it found, or alone where it found nothing, a line
      leads into the whole catalog with the same text and says how many modules it finds there
      (konzept.html: „Der Weg in den ganzen Katalog bleibt eine Zeile").
    - **The order is at the heads of the columns,** as in the catalog (owner, 2026-10-19: „die
      Sortierung bei der PC version etwas komisch. Das könnte noch eleganter umgesetzt werden"):
      „Gemerkt" over the bookmarks orders by the time of marking (the newest first, then the
      oldest), „Modul" by the study with Mein Studiengang and by the title without one (as the
      catalog's „Modul" is the plan's order inside a program), „LP", „Turnus" by the offer,
      „Termine"; a second click turns an order round. „Zuletzt gemerkt" falls into sections by
      the time of marking (Heute, Gestern, Diese Woche, Letzte Woche, then the months, a month of
      another year with its year; „Früher" for a mark without a time), „Nach Studium" by where a
      module counts in Mein Studiengang (Pflicht, Wahlpflicht by area in the order of the module
      tree, Abschlussarbeit, Praktikum, FÜS — the program's own list and the BTU's general one —,
      „Gehört nicht zu deinem Studiengang"; `pages::bookmark_places`), „Nach Angebot" by the half
      of the year (Winter, Sommer, Jedes Semester, Unregelmäßig, Ohne Angabe), each heading with
      its number and staying under the heads of the columns while its rows scroll. A search
      keeps the order without the sections. On a phone the sheet of the filters lists the orders.
    - **What „Mein Studium" says stands at the row** (owner, 2026-10-19: „Ja mein stadium"): its
      mark (planned, under way, passed, not passed, as the semesters of „Mein Studium" draw it)
      with the semester, after what the catalog says of the module (`list::RowStatus`).
    - **The filters are the few that separate marked modules** (owner, 2026-10-19: „hier reichen
      ein paar, die wirklich relevant sind"), each with its numbers under the others and there
      once it separates something (konzept.html, „Filter, die nur da sind, wenn sie etwas
      trennen"): „Angeboten im" while the marked modules are not all offered alike, „Stand"
      (open, planned, passed; a module planned in a semester that is over and not passed is open
      again) once „Mein Studium" says something of one, „Zählt in meinem Studium" with Mein
      Studiengang, „Passt in meinen Stundenplan" once the Stundenplan holds something (the
      catalog's finder against the current semester, comparing what it compared last), and,
      folded, four of the catalog's: language, credits, campus, graded. The tags above the list
      name what filters it, each taking its filter away. On a phone the sidebar is a sheet behind
      „Filter" (its badge counts the filters and an order other than the first), and what is
      picked there is a draft, as in the catalog's sheet: „N Module anzeigen" applies it, any
      other way it goes throws it away.
    - **Collections** („Sammlungen", Folia 1.35.0; folia/design/merken/konzept.html, „Sammlungen
      und Notizen", oberflaeche.html, „Die Seite"; owner, 2026-10-19: „Sammlungen“, „zwei Gruppen
      gleichen Namens soll nicht erlaubt sein … Leere Gruppen sind aber gut"): what only the
      visitor knows of the modules, ordered by them. A module is in any number of collections, or
      in none; a collection may be empty, for good; every name is there once (case and the spaces
      at its ends do not count: „für lena" is „Für Lena"). What is in a collection is marked:
      putting a module into one marks it, taking it out leaves it on the Merkliste, taking its
      mark away takes it out of all of them (with „Rückgängig"). Each collection is a view of the
      list (`collection=<n>`, „Ohne Sammlung" `collection=none`), in which the search, the order
      and the filters hold, with numbers of its own („3 in Für Lena · 22 LP"). On a wide screen
      the sidebar begins with them — „Alle", „Ohne Sammlung" once there is a collection, each
      collection with its number, the one shown dark, „+ Neue Sammlung", a field where Enter makes
      one and opens it, and a name there says „Gibt es schon · Enter öffnet sie" —; on a phone they
      are chips over the list, which scroll sideways, „+" makes one, and the sheet behind „Filter"
      has the filters alone. A collection's menu (its ⋯ while pointed at or focused, a right
      click; on a phone the chip held, or ⋯ in the head) renames it (F2), shares it („Teilen",
      since 1.39.0: its link with its name, below), moves it up or down and deletes it (Entf):
      „Löschen" chosen asks nothing, the modules stay marked, and the note at the bottom says
      „‚Für Lena‘ gelöscht · 3 bleiben gemerkt" with „Rückgängig", and „Auch 2 entfernen" where
      modules were in it alone. Its key asks first (since 1.39.2; owner, 2026-10-20: „einen
      Delete-Shortcut …, wo man dann aber nochmal gefragt wird"): Entf (on a Mac ⌫ as well) at the
      collection in the sidebar, or in its open menu, puts the question where the menu stands —
      „‚Für Lena‘ löschen?", „Die 3 Module darin bleiben gemerkt.", „Löschen" with the focus,
      „Abbrechen"; F2 in the open menu renames as at the collection. The menu (and its question)
      gives the focus back to what opened it as it closes, or, the collection deleted, to the view
      the sidebar shows. No menu stands under the rail: the menus fixed to the window (the
      Merkliste's, „Sammlung und Notiz", those of „Mein Studium") stay right of it on a wide
      screen (`folia_design::nav::room`; owner, 2026-10-20: a collection's ⋯ in a narrow sidebar
      had put its menu behind the rail). A row names a module's collections outlined after what the
      catalog and „Mein Studium" say of it (two by name, then „+1"; the one the list shows not),
      and the first line of its note under it, as wide as the row, its Markdown drawn and cut with
      „…" (`list::RowMine`). An empty collection, „Ohne Sammlung" with everything in collections
      and a collection that is not there (deleted, or a link from another browser) each say so.
    - **The whole list has a menu as a collection does** (oberflaeche.html: „„Alle“ hat ein Menü
      wie eine Sammlung"), in place of the sidebar's former „Aktionen": „Link für ein anderes
      Gerät", „Als Text kopieren" (the note at the bottom says „Link kopiert", „Liste kopiert"),
      „Merkliste leeren", which asks first, the answer with the focus, and is taken back with
      „Rückgängig" in the note. What the list is kept in is said in its empty state; the whole of
      it in the privacy notice and on the profile page.
    - **Several at once** (Folia 1.36.0; konzept.html, „Mehrere auf einmal", oberflaeche.html,
      „Mehrere auswählen"; `folia/crates/bookmarks/src/pick.rs`): rows are selected as in „Mein
      Studium" since 1.31.0 — no boxes, the row tinted in the accent and hatched, the bar in the
      place of the heads of the columns, as tall (on a phone over the tab bar). On a wide screen Ctrl
      or ⌘ and a click selects a row or lets it go, Shift and a click takes in the rows from the last
      one clicked, a mouse pressed on a row and drawn onto others selects them (begun on a selected
      row it lets them go; a row's link is not dragged away), and a row's right click has a menu of
      its own: „Sammlung und Notiz …" (S), „Auswählen" or „Abwählen", „Einplanen" where its switch
      would plan it (or „Aus dem Plan nehmen"), „Merken" or „Von der Merkliste entfernen" (M); while
      it is open, S and M are those entries for its row (since 1.39.2). A key pressed in any menu,
      dialog or question is its own: F, M, P, S and N never reach the module open behind it (before
      1.39.2 M marked that module from a menu, `enhance.js`, `OVER`). On a
      phone a finger held still on a row for 450 ms selects it (`folia_widgets::swipe`, `on_hold`:
      held is no swipe, and lifting it no tap), and drawn on, the rows under it; a swipe stays a
      swipe. While rows are selected a click or a tap selects or lets go. Keys, new for both lists
      (oberflaeche.html: „Neu, für beide Listen"; „Mein Studium" has them too): Shift and ↑ ↓ take
      in the next row, or let go of the last coming back towards where the range began; Ctrl or ⌘
      and A every row the list shows (not in a field); Esc lets go; the space bar at a row selects
      it; S with rows selected is the bar's „Sammlung". A range, a drawn selection and „Alle
      auswählen" take in what is marked, a row dimmed on the page only when it is chosen itself;
      what leaves the list (a search, a filter, another view) is let go of. The bar: „3 ausgewählt ·
      14 LP", „Alle auswählen" (a wide screen's), „Sammlung" — the menu of their collections, a
      check where all of them are in one, a dash where some are, a tap on the dash takes in the
      rest, no note and no default (`CollectAsk::more`) —, „Einplanen" — „Jedes in sein Semester"
      (each where its own switch aims, `studyplan::aim_now` with the semester of its newest events,
      `NewestSemestersAsk`, asked once for the list) or under „Alle in" the semesters from the
      current one as far as the plan reaches and at least four, with the Fachsemester, „jetzt" and
      whether they are offered there, and folded „Frühere Semester" back to the first Fachsemester;
      what is planned ahead moves along, what is there already stays and is counted
      (`studyplan::plan_picks`) —, „Teilen" (the link of these modules, their notes along if
      switched so, or them as text; since 1.39.0), „Entfernen", and ×. What it did the note at the bottom says — „3 eingeplant · 2 in
      WiSe 26/27, 1 in SoSe 27", „3 Module entfernt" — with „Rückgängig" for all of them.
    - **From elsewhere** (Folia 1.37.0; konzept.html, „Von überall"): wherever Betula asks for a
      module, the marked ones are at hand with the same collections — in „Mein Studium" a third
      source of „Module hinzufügen", first in a cell of the Gesamtplan and in the Wahlpflicht's
      lists, with how many count in a choice („Mein Studium" below); the catalog's „Gemerkt" per
      collection; the Merkliste's tab held or right clicked, a menu of its views. The Stundenplan's
      „Aus der Merkliste" (Folia 1.38.0, `studyplan::head::FromBookmarks`), closed until asked,
      lists every marked module the semester does not hold yet: its collections as chips with how
      many each holds of these (those that hold none left out), a search of titles, numbers,
      notes and collections, what the semester offers each with how it sits in the plan — „passt",
      „Passt mit Übung Do 13:45", or „Überschneidet sich mit: Analysis I (Di 09:15)" in the
      warning's colour, as a module's week says it beside the plan (`pages::fit_lines`, the
      overlay's line for many at once, asked in the browser once the list is open) — and
      „Einplanen", „3 gemerkte Module übernehmen" for all of them shown (of a collection too), and
      folded under them „Im WiSe 2026/27 nicht angeboten · 2", each with why („nur SoSe", „nicht
      mehr angeboten", the turnus) and „Einplanen" all the same.
    - **Near the limit** (256 modules, owner, 2026-10-19: „Also 256 Module sind auch genug. Dann
      gibt es einfach ein Limit, und man bekommt ne Meldung") the head of the whole list says
      „248 / 256" from 240 marks on, from 250 in the warning's colour; nowhere else is a limit
      seen. A link that brings more than fits offers what fits.
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
    page in the list's place (`&full=1`), as on a program's page (a local view,
    `folia/crates/widgets/src/local.rs`). The tab „Merkliste" stays the current one and remembers the module,
    „Zurück" and Esc lead to the list with the module beside it again (on a phone its sheet),
    through the history, and show the row it was opened from; the
    catalog's tab never hears of it. What is listed stays meanwhile: a mark taken away on the
    module's page leaves the module on the list, dimmed. A module's own page reached from there (a
    successor named on the page) belongs to the marked modules as well: its „Zurück" leads back
    (`Tabs::came_from`), and the catalog's tab still leads to its list.
  - **The menu of a module's collections and its note** (`folia_widgets::collect`;
    oberflaeche.html, „Ein Menü für Sammlung und Notiz"; owner, 2026-10-19: „mach einfach gleich
    den Fokus in das Suchfeld", „Mach mal notizen mit md support, du brauchst kein visuellen
    editor"): one menu wherever it opens — the field of the note after marking (below), the arrow
    of a module's switch, **S** at a row or at the open module, **N** with the focus in the note
    (`enhance.js` `betula:collect`). At its top a field with the focus (on a wide screen) that
    filters the collections and makes one („‚mathe‘ anlegen"; a name there is that collection);
    ↑ ↓ and Enter go through the rows, no digits. The collections with a check and how many
    marked modules each holds, in their own order. „Standard für neue Merker" once the module is in
    a collection: what is marked from then on goes into the same collections until it is
    switched off, over a restart too (oberflaeche.html, question B); switched on in the menu it
    follows what is chosen there, and a module in none says which default there is, with
    „Aufheben". The note in Markdown under it, a field that grows: **bold**, *italic*, `code`,
    lists and links; drawn safely (`folia_model::text::note_blocks`: links only to `http`,
    `https` and `mailto`, opening in a new tab, a picture only a link and never loaded, HTML as
    text). On a wide screen the menu stands at what opened it and each choice holds at once; Esc
    or a click beside it closes it, the focus goes back. On a phone it is a sheet from below and a
    draft (owner, 2026-10-19: „Wenn man das fenster so zu macht, dann werden die changes nicht
    gespeichert"): „Fertig" at its foot, as wide as the sheet, keeps what was chosen and written
    and ends the note it came from; swiped down, tapped beside or Back lets all of it go, a
    collection made in it too; the field gets the focus only when tapped, and with the keyboard up
    the sheet takes what the keyboard leaves. What gets a collection or a note there is marked,
    without the default.
  - **At the module** its switch says more (oberflaeche.html, „Am Modul"): marked, „Gemerkt"
    carries the module's collections small, as „Einplanen" carries its semester, and an arrow of
    its own opens the menu (42 px on a phone); not marked, where a default is set, „Merken · in
    Für Lena" (a row swiped to mark says the same). The note stands under the switches, whole,
    its Markdown drawn; the pencil, or a click beside a link, edits it.
  - **The note after marking** (`folia_widgets::marknote`; oberflaeche.html, „Merken und der
    Hinweis danach"; owner, 2026-10-19: „die option mit dem Feld auch mit abstand am besten") is a
    note that waits for interaction in the frame of the note of „Einplanen", without its stepper:
    a check, the module's name on a wide screen, the field that says where the module went —
    „Ohne Sammlung", „Für Lena", „Wahlpflicht, Für Lena +1", a pin where the default put it
    there, „Sammlung" with a plus while there is none — and „Rückgängig", which takes the mark
    away with what the default gave it. A tap on the field, or S while it stands, opens the menu;
    the note waits behind it (`WaitNote`'s `held`), and on a phone „Fertig" ends it (`away`).
    „Entfernt" with „Rückgängig" brings a mark back with its collections and its note; a mark
    taken away and given again in the same tab comes back as it was, at its place (a dimmed row
    on the Merkliste „holt es zurück"). „Merkliste voll · 256 Module" with the way to it marks
    nothing. One note stands at a time: it takes the place of the note of „Einplanen", and that
    one takes its place (`Planning::hush`).
  - **Kept beside the marks, in this browser alone:** the collections, the modules in them, the
    notes, the default and the collections used last are a text of their own under
    `betula.collections.v1` in each profile (`folia_stores::collections`), so that an older build
    reads the marks as before and nothing moves (konzept.html: „Ein zweiter Schlüssel je
    Profil … Kein MAJOR"). It is read like anything from outside: a name twice, a collection
    that is not there, an id that is none, a number twice are dropped line by line, names and
    notes are cut at their caps (60, 4,000 characters), 64 collections at most. What belongs to
    a module that is no longer marked (an older tab unmarked it) goes with the next change. A
    profile's file, „Profil leeren" and the profile page take it along.
  - **To another device without a server in between:** „Link für ein anderes Gerät" copies a
    link to the list with the marked modules in its *fragment*, as a code (`/bookmarks#m=…`,
    `pack/`): the ids as a set of numbers, in ascending order, each as its distance from the one
    before, in characters an address carries as they are, and two check characters at the end.
    The order of marking does not travel (owner, 2026-09-23: it does not matter); the other device
    marks them all at once. The code begins with the number of its layout, in four bits
    (`pack::to_versioned_code`; owner, 2026-09-25), so a later layout can be read beside it. 20
    marked modules take 35 to 40 characters (as ids one by one, `11101,12204,…`, 124). A character
    typed wrong or two swapped are always noticed, a link cut short almost always; the page then
    says the link is broken, and offers nothing of it. Since Folia 1.39.0 (konzept.html,
    „Weitergeben"; oberflaeche.html, „Weitergeben und Ankommen") a code also carries collections
    — each its name and its modules by their places in the list — and notes, as fields at the end
    of the layout, empty and so absent in a code of modules alone: a code of before reads as it
    did, and one of modules alone is the same code as before (`bookmarks::share_fragment`,
    `Brought`; `folia_pack`'s rule for lasting codes). The link for another device takes
    everything along (konzept.html: „Das eigene Gerät bekommt alles"): every collection with its
    marked modules and every note. „Teilen" in a collection's menu and in the bar of the rows
    selected is a menu of three lines: „Link kopieren" (a collection's link carries its name),
    „Notizen mitnehmen" (off until switched on, for as long as the page is open: „Notizen bleiben
    zu Hause, außer du nimmst sie mit"), „Als Text kopieren" (each module a line of a message,
    its number, title, LP and the link to its page). What a link brings is checked like anything
    from outside (`bookmarks::brought_from_fragment`): names clean and each once, modules of the
    list, notes clean and one each. It arrives as it was sent: modules alone „Hinzufügen"; a
    collection „Für Lena · 3 Module aus einem Link." with „Als Sammlung merken", „Nur merken" and
    „Verwerfen", and where a collection of its name is there „Die Sammlung gibt es schon." with
    „Zu ‚Für Lena‘ hinzufügen" and „Anderer Name …", whose field refuses a name there is („Gibt es
    schon"); the link for another device „46 Module in 4 Sammlungen aus einem Link. 12 davon neu."
    with „Hinzufügen". A note comes where a module has none, never over one kept here
    (`Shelf::adopt`); more than there is room for keeps the first that fit („Die ersten 8 merken"). A list too long for a code (thousands of ids
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
  browser history if that list is where the visitor came from (as the browser says it where it
  can, else as the memory guesses it, `data-back="history"`; so the history does not grow), and
  as a plain link otherwise (after a change of tabs „Zurück" leads
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
  What moves is the bar's row of tabs (owner: „eine Animation der Selection um den Prozess vom
  Swipen visuell zu unterstützen"; the page went along from Folia 1.28.0, „The pages on an axis"
  below, before that „keine Seitenanimationen"): a finger that moves
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
- **„Einplanen" plans at once, and a note says where** (owner, 2026-10-18, concept E of
  `folia/design/einplanen/oberflaeche.html` after the behaviour of `verhalten.html`: „Also ich liebe
  Option E, das ist mit Abstand die eleganteste"; `folia/crates/stores/src/studyplan.rs` and
  `studyplan/planning.rs`, `folia/crates/widgets/src/plannote.rs`). The menu of the semesters that
  „Einplanen" had held besides its click (1.18.0–1.22.0: a mouse resting on it, P held or Shift+P, a
  finger held while its border filled; owner: „irgendwie ist das kleine gute Option") is gone with
  the building block it was (`holdmenu`); so is „Anderes Semester" of the sidebar, gone since
  1.19.0.
  - **Where a click plans** (`target_semester`, `Planning::base`): a page that means a semester
    (the finder's, a placeholder's) that semester; else the semester the session plans for, where
    the module is offered then (its turnus, or its events are there); else the next one that
    offers it; a module without a season (irregular, unknown) where its events are, when they are
    later. The semester of the session (P1) is, the first time a session needs it, the running one
    until 15 November or 15 May and the next one after that (who plans in the middle of a semester
    plans the next); it stays until the tab closes (owner: „Wichtig ist aber, dass sich das nicht
    verändert in der aktuellen Session, das währe sehr disorientierend"), in the tab's
    `sessionStorage` (`betula.plan.semester`), never before the snapshot's current semester.
  - **The switch** says where in its small line („Einplanen · WiSe 26/27"); planned from the current
    semester on (the first of those) it says „Eingeplant · SoSe 27 · 4. FS" and a click takes it out
    there, with what was passed of it there. A module planned only in semesters behind the student is
    planned anew (a retake), the small line names them; where it is passed it says so („· bestanden",
    „bestanden: SoSe 26"). The sidebar of a module's page says the same under its action. The
    browser app's alone (R9): where a session plans depends on the day.
  - **Its arrow** (`PlanButton`, `folia/crates/widgets/src/planmenu.rs`; owner, 2026-10-20: „bei
    merken haben wir ja nachdem es selected ist so einen more context pfeil ran gemacht, damit man
    das nicht löschen und dann wieder merken muss, um was daran zu ändern. Mach das mal bei planen
    auch so. Dass man da das semester ändern kann aber zum beispiel auch das modul als bestanden
    markieren kann"): planned, the switch at a module has an arrow of its own, as „Gemerkt" has, and
    ⇧P opens it. Its menu is worded as the rows' menus of „Mein Studium" and looks as they do
    (`.st-pop`, a sheet from below on a phone): „Als bestanden markieren" (or „Bestanden
    zurücknehmen") in the semester it is planned in; „Verschieben nach ›", every semester from the
    first Fachsemester (six back without a Studienbeginn) to three after the later of its own and
    the current one, or the end of the study, each with its Fachsemester, „jetzt" and whether it
    offers the module — one behind the student as well, for who passed it then; „Im Stundenplan
    öffnen" with the module beside the week; „Entfernen". Each holds at once and the note says so
    with „Rückgängig", a move as after a click, with ‹ ›; what was passed goes along with a move.
  - **The note** (`PlanNote`, app.css „The note of „Einplanen"", `.plan-toast`) after a click, P or
    a row swiped, a note that waits for interaction (below; owner, 2026-10-18: „Bau diese
    notification mal bitte als wait for interaction notification"): it stands until the student does
    something else; a newer one takes its place, and another profile takes it away. On a wide screen
    one bar across the content, one line high (the same day: „bei der pc Version kann die
    notification ruhig über die ganze Breite rechts von der sidebar gehen und dann vllt etwas
    weniger hoch sein"), in order whatever the room (the same day: „Auf der pc Version bei sehr
    kleinen Bildschirmen, gibt es das problem, dass die notification bar so alles übereinander
    zeichnet. Bau die mal so, dass das alles responsive je nach verfügbaren platz ist"; „Du musst
    halt noch mal gucken wie du die Elemente ein bisschen besser anordnest und das ist halt nicht so
    durcheinander aussieht"; app.css `.planning-note`, `@container wait-note`): the check and the
    module's name at the left, the semester on a quiet ground of its own, „Rückgängig" and × at the
    right, each part a column of its own and nothing over anything else; the name has the room that
    is left and is gone where the note is narrower than 560 px. The switch comes as a row over all
    of it, its icon over the check, its words where the name begins, its switch over the ×, and the
    note grows upwards (the same day, asked whether one line without the module's name would be
    better: „Nee, zwei zeilig passt. Lass das so"), so the semester stays where it is. On a phone
    its switch is a row over the semester, so that what is under the finger stays where it was. In
    it a check, the semester between ‹ and › with a line under it — its Fachsemester and the warning
    („kein Angebot" where the turnus leaves it out, „liegt zurück" in a semester behind the student,
    in the warning's colour), and what happened („eingeplant", „verschoben") only where nothing else
    is said (the same day: „Wichtig finde ich, dass das eingeplante Semester (also fs. 12) und die
    Warnung da steht. Der Rest kann optional ausgeblendet werden") — and „Rückgängig". The semester
    and its arrows are one control, as wide whatever it says, so the arrows never move (the same
    day: „müssen die Links und rechts Buttons virtuell über den komplette text da gehen … Dazu muss
    auch swiping unterstützt werden"): a tap on its left half is ‹, on its right half ›, and a
    finger swiped sideways over it moves the module a semester the way a page turns (to the left the
    next one; the note's `Sideways`), the words following the finger and coming in from the side the
    semester came from. It moves the module at once, a semester at a time, into the past as well
    (the same day: „Betula sollte bei sowas nicht nein sagen", CLAUDE.md „Design"), to the first
    Fachsemester (six back where the Studienbeginn is not known); ← and → do the same while the note
    stands, its own keys, where the keys mean nothing else (a field, a picker, a menu, a slider, a
    panel's handle, a dialog). Once it was moved, a second row „Weitere Module auch nach SoSe 27"
    with a switch makes that semester the session's for the next modules (P3, `betula.plan.chosen`),
    a past one too; while it is on it stands in every note, to be switched off where it is seen. A
    module taken out says „Datenbanken entfernt", with „Rückgängig"; where the note is narrow the
    module's name gives way, „entfernt" never. „Rückgängig" puts back the plan as it was before the
    click while the plan is still what the note made of it, else undoes what the note did alone.
  - **Where no study is set up** (no Studienbeginn stored, and the session has not answered with
    „Ohne Studium einplanen"), „Einplanen" — its switch, P, a row swiped — opens one dialog instead
    (owner, 2026-10-18: „so dass man im best case nur auf ok drücken muss, dann das Studium
    eingerichtet ist und dass Modul im richtigen Semester geplant ist"; `widgets/src/plansetup.rs`,
    `studyplan/fit.rs`): „Dein Studium" as the card of „Mein Studium" has it (a sentence, the
    program, the Fachsemester with − and +) and under it the semester the module goes into, with ‹
    ›. „Einrichten und einplanen" stores the program and the Studienbeginn in „Mein Studiengang" and
    plans the module, „Ohne Studium einplanen" plans it alone and the session asks no more
    (`betula.plan.asked`), „Abbrechen", × and Esc leave everything as it was. The program comes from
    the catalog's filter, else the Stundenplan's of the session, else „Mein Studiengang"; without
    any, none is assumed (owner: „Weil der assumed Studiengang wahrscheinlich falsch ist"): the
    module's programs stand open — three at most, each program once by its newest regulation,
    compulsory before elective, those whose plan places the module first — with the search for any
    other right under them, the app's picker as it is, as wide and as tall as the programs over it
    and so when it opens, its list under it as wide (the same day: „Erstmal ist es zu weit links,
    dann wird es breiter wenn man rein klickt und die combox ist nicht so styled, wie alle anderen
    in der App"); „Einrichten und einplanen" waits for one. A program known is a row as in „Mein
    Studium", its picker the way to another, the module's programs first in it („Studiengänge mit
    diesem Modul"; the same day: „was ist das für ein clusterfuck, wenn man schon ein Studiengangs
    ausgewählt hat und den doch nochmal ändern möchte"). Once a program is known, its
    Regelstudienplan fills in what the student has not set (owner: „Aber nur, wenn der User noch
    nichts an den werten geändert hat"): without a Fachsemester, the one that puts the module into
    its semester of the plan; with one, the semester that does. A Bachelor's odd Fachsemester are
    winters unless its plan says otherwise (`intake_season`), a Master's are not assumed (owner:
    „Master hingegen haben diese "Einschränkung" nicht"). The filter's semester is the semester
    being looked at, read by the time and that season (owner: „wenn man semester 3 auswählt im
    filter ist man probably erst im zweiten"). A place of the plan behind the student puts the
    module where „Einplanen" aimed and says why; a semester behind the student is allowed, with a
    warning. Where „Weitere Module auch nach …" aimed the click, the module's row says so („für
    weitere Module gewählt") with „Aufheben", which clears the choice for the session and aims as
    without it (a dialog stood at „SoSe 2034 · 16. FS" without a word why).
  - **A row swiped to the right** on a phone (`swipe.rs`) does what the switch does, the note or
    the dialog after it; its ground says „Einplanen" with the semester, and keeps saying it where
    the dialog comes next.
- **A note that waits for interaction** (`folia_design::waitnote::WaitNote`, app.css „A note that
  waits for interaction", `.wait-note`; owner, 2026-10-18: „Bau diese notification mal bitte als
  wait for interaction notification. Vielleicht brauchen wir das später nochmal"): at the bottom of
  the window, over the page, what was just done and what follows from it, and × at the end of its
  last row. What it says can ask how wide the note is (`@container wait-note`) and lay itself out
  for the room it has. It has no timer: it stands until the student does something, and what is done
  with it keeps it; anything else is moving on, and it goes (the same day: „sobald erkannt wurde,
  dass damit nicht interagiert wurde und der Nutzer weiter macht soll es ausgeblendet werden. Aber
  wir müssen aufpassen, dass versehentliche Interaktionen, wie ein klein wenig scrollen kein Problem
  sind") — a click or a tap elsewhere; a key that is not its own (`on_key`), not a modifier, Tab, a
  function key or a key with a modifier; another page or tab, another module opened or its panel or
  sheet closed, other filters or another search (the address but its `page`, which the catalog's
  list writes as it scrolls); a scroll of anything further than a fifth of the window (140 to 260
  px), not a little one. Nothing within 0.4 s of its coming or of a change of it by the student's
  hand (`changed`) counts, which is the page answering what was done. It goes down out of sight,
  from where it stands, and then the host lets go of it (the same day: „wenn man da die Modul
  Ansicht schließt sollte die notification auch mit eher Animation nach unten raus geschlossen
  werden. Aktuell ist das ziemlich unangenehm anzusehen"): its place is kept from its coming, so
  what changes around it — a module's sheet that closes on a phone, a page without a sidebar — never
  moves it on its way out, and a module's sheet sliding away takes it along at once (enhance.js
  `sheet-away` on the document). × and Esc put it away, and a finger swiping it down (the same day:
  „Auf dem Handy soll es die Geste geben die notification nach unten weg zu wischen"); a part of it
  may take a finger swiped sideways (`Sideways`), and a swipe is never a tap as well. On a wide
  screen one bar across the content: right of the sidebar where the page has one, the catalog's
  filters or the sidebar of the frame, as wide as its handle makes it (the same day: „Bei der pc
  Version soll die notification nicht über die sidebar (z.b. filter) gehen, sondern nur über den
  content"), else right of the rail; on a phone a card as wide as the bottom bar and over it (over a
  module's sheet at the bottom of the screen), under the bar in the order of what lies on what and
  flat, so it casts nothing on the bar and comes up from behind it (the same day: „keinen schatten
  auf die navbar werfen"), without ×. Dark in both themes (the same day: „dark modus muss natürlich
  auch dafür funktionieren"): inverted on the light page, a raised dark surface with a fine edge on
  the dark one (`--toast`, `--on-toast`). The host shows it while it has something to say, gives it
  what it says as one element, and lets go of it on `on_gone` — of this note alone, a newer one that
  took its place staying (a key that makes the next note). The note of „Einplanen" is the first that
  uses it.

- **A tab held gives what it holds besides its page** (owner, 2026-10-06: „wenn man die buttons in
  der nav bar lange gedrückt hält, dass dann eine special aktion kommt. Bei mein Studium währe das
  dann eine auswahl von Regelstudienplan, Wahlpflicht, Alle Studiengänge. Das sollte natürlich auch
  mit [Rechts]klick funktionieren. Bei dem katalog könnte sich dann gleich der katalog mit offenem
  Filter öffnen (das wäre aber mobile only weil auf dem pc ist der ja eh immer offen)"; `enhance.js`
  „a tab held", `chrome::TabMenu`, app.css `.tab-menu`): a finger that stays on a tab of the bar for
  420 ms without moving (a little before the browser's own long press), or a right click (the
  menu key) on a tab of the bar or the rail. „Studium" opens a menu at its tab, over the page
  dimmed a little, above the bar on a phone and beside the rail on a wide screen: the name of
  „Mein Studiengang", the views „Regelstudienplan" and „Wahlpflicht" of „Mein Studium" where one
  is chosen (since 2026-10-14: until then the program's pages), and „Alle Studiengänge". The
  Merkliste's, once there is a collection (Folia 1.37.0; oberflaeche.html, „Der Tab, lange
  gedrückt"), its views: „Alle gemerkten", „Ohne Sammlung" and each collection, each with how many
  marked modules it holds (`data-hold="menu"` on the tab only then; the browser app's alone, R9).
  A tap beside it, Esc, a way taken or another page closes it. The catalog's tab held on a phone
  opens the catalog's list as it was left (else the catalog its tab starts at) with its filter
  sheet open; a right click on it on a wide screen, where the filters are always there, keeps the
  browser's menu, as do the other tabs. The touch that held is no tap and no swipe (its click, which
  would land on the sheet's dimmed page and close it again, goes before anything hears it), and
  the bar's tabs never get the browser's own menu or preview of a link (`-webkit-touch-callout`).
  The app marks the tabs that hold something (`data-hold="menu"`, `data-hold="filters"` with the
  list's address in `data-list`); before the app runs none does.
- **The tab of the area one is in, tapped again, takes its page back to where it starts**
  (`tabs::TabAgain`, counted by `NavItems`, also where the tab leads to the address the app is at
  and the router has nothing to do; on a phone the history goes back to the tab's first entry,
  `nav::tab`): „Mein Studium" goes back to its overview at once, wherever the
  row of its pages is or goes, and at the overview up to its top (owner, 2026-10-06: „wenn man in
  dem studium tab swiped, funktioniert der button in der nav bar auch erst, wenn die seite sich
  nicht mehr bewegt. Das muss auch unbedingt gelöst werden. Wenn man den anklickt muss es sofort
  wieder an die standard position gehen"; „Mein Studium" below).
- **The program an area has chosen is that area's, and only for the session** (owner, 2026-10-15:
  „dass sich das nur pro Tab (i.e. Katalog/Studiengang/Timetable) und Session gespeichert werden
  soll. Ansonsten wird der Mein Studiengang genommen und wenn da keiner hinterlegt ist, dann halt
  keiner"). The catalog's is its filter (`program=`), in its address, which the tabs' memory keeps
  for the session; its first entry of a session is the catalog of „Mein Studiengang". „Studium"
  opens on „Mein Studium", the study of „Mein Studiengang", and returns to the program's page it
  was left at. The Stundenplan's („Studiengang" in its sidebar, `PlanDoc::program`) is kept in the
  tab's `sessionStorage` (`betula.studyplan.program`, each profile its own;
  `folia_stores::studyplan`), never in the plan's text, where a build before wrote it as `g` (read
  past since): a pick or a Fachsemester taken over there is the Stundenplan's alone, sets no „Mein
  Studiengang" and reaches no other area; a saved plan and a shared one still carry their program,
  and loading or taking one over makes it the session's. Where an area has chosen none in the
  session its program is „Mein Studiengang", and without one there is none. „Mein Studiengang" is
  set where it is meant: in „Mein Studium" (its first visit, „Studiengang wechseln") and on the
  start page. **Every program picker lists „Mein Studiengang" first**
  („immer der erste Eintrag beim Dropdown"): under its own heading, above „Alle Studiengänge" where
  the picker has it, and in its place among the others as well (`combobox::with_first`; the
  catalog, the Stundenplan, „Studiengang wechseln").
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
  the finger starts. The mark's side has its icon where the row's bookmark stands, and the row's
  bookmark goes with the card as one (it trailed the finger by the 120 ms its button's transform
  eases in, owner, 2026-10-17: „nicht fest attached, sondern floated so mit"); its colours still
  ease in, its place does not (the same day: „Die transition soll weiter bleiben, aber nur für
  farbe und nicht position"). The ground starts in
  the look of what the module is, and once the action is armed, a third of the card or 120 px away
  (or half as far and flicked, as the sheet measures a flick), it takes the look of what the module
  will be and the icon springs (owner, 2026-10-17: „erst dunkel ist und dann hell wird, wenn es dann
  gemerkt ist. Und dann hell ist, wenn es schon gemerkt ist und dann dunkel wird, wenn es wieder
  entmerkt wird", in the dark theme, and „das gleiche mit grün für einplanen und rot für
  entfernen"): „Merken" from the quiet ground to the inverted look of a marked module (its switch
  on the module's page), „Entfernen · von der Merkliste" from that look back to the quiet ground;
  „Einplanen" from the quiet ground to the accent, „Entfernen · aus …" from the accent to a muted
  red at the accent's lightness (`--remove`, white labels as on the accent), striped at 45° a shade
  darker (owner, 2026-10-17: „schwer zu erkennen … für rot grün blinde, ob wir jetzt planen oder
  entplanen … 45 Grad Striche im Hintergrund"), so that it is told from the accent by more than its
  hue. Let go armed, the ground says what was
  done („Gemerkt", „Eingeplant", „Entfernt"), the card goes aside as far as that takes, holds a
  moment and glides back, and the action follows after the next frame (R21); let go before, the
  card glides back and nothing happens. Nothing of it waits for its animation (owner, 2026-10-17:
  „Ich will, dass man sofort korrigieren kann und nicht erst warten muss, bis die Animation fertig
  ist"): a card that holds what was done or glides back is the next finger's at once, from where it
  stands (read from the page, as far as its glide has come), its hold and its rest are left undone,
  and what its ground says is read anew, so a swipe right after one undoes it. The row stays where it is (R5: marking and planning change
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
- **The program overview** is one accordion per faculty (owner, 2026-10-14: „Ich mag die
  akkordion elemente von den Fakultäten, aber wie die studiengänge dann darin gelistet werden ist
  eine katastrophe"; the three ways of listing them that followed are in
  `folia/design/studium/konzepte.html`, and the owner took the first, „Variante 1 mit
  ausklappen"). A faculty's head says its number, its name, what it has by degree („12 Bachelor ·
  11 Master · 1 weitere") and how many; the faculties wait to be opened, and where a filter or the
  search narrows the list every faculty with something left is open. In a faculty, where it is
  wide enough (a container query on `.faccs`, 760 px), a matrix: a row per subject, a column per
  cycle of study (Bachelor, Master, the rest only where a faculty has some; Lehramt counts to its
  cycle), the columns the same in every faculty, so the page stands on a few vertical lines. Every
  program is a line of text, not a box: its degree (with its PO where a cell has two of one
  degree), the forms of study it is also offered in indented under it („dual, Ausbildung"), and
  at its end how long and how many credits its validated plan says („6 Sem. · 180 LP",
  `folia_pages::PlanFacts`, what the head of its page says, worked out once per snapshot from the
  plans of every program at once, `queries::latest_plan_entries`/`latest_plan_totals`; „kein
  geprüfter Plan" where it has none, the quieter line). Where the faculty is narrower (a phone) a
  row per subject: one program is a link with its degree and facts under the name; several fold
  open (a `<details>` of its own) and list them by degree. The fourteen combinations of the
  Lehramt Primarstufe are one subject, each named by its combination („Deutsch – Englisch"). Both
  drawings are in the server's HTML, the stylesheet shows the one the width asks for: the
  accordions are `<details>`, which open without JavaScript, and a jump of the sidebar to a faculty
  opens it (enhance.js). Coming back from a program, its faculty (on a phone its subject) is open
  and the program in view. In the app the head names the visitor's own program with the way to
  „Mein Studium" and the Master after it (or the Bachelor before it, `v_program_counterpart`,
  `CounterpartAsk`), and the list marks it; the server writes an empty box in its place where the
  browser keeps a program (`html.mine`), so nothing moves at the takeover. Tried before and
  dropped as clutter (owner, 2026-09-20): cards, and the rows set in several text columns; rows of
  unequal height with nothing to line up on; then (2026-10-14) programs as segmented boxes in the
  cells, which on a phone stacked into a wall of boxes.
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
  phone a module comes up from below as a sheet over the page („A module on a phone" below), the
  same panel with the same address, one tap and one history entry; on a program's page an area
  or a row of the plan picked is the page there (`Filling` in `folia/crates/programs/src/program.rs`), „Zurück"
  leading to the program, and a module picked out of it comes up over it. Without the app the same
  HTML (the panel beside the page) is shown as the page by the stylesheet (`.aside-picked`).
- **A module on a phone** (owner, 2026-10-06: „Aktuell wird ja wenn man auf ein modul klickt gleich
  die volle seite aufgemacht. Aber ich finde es währe viel besser, wenn man wie bei der Übersicht
  nach bereichen so ein menu bekommt, dass sich dann von unten öffnet. Hierbei finde ich, dass es
  sinn ergeben würde, wenn es sich erstmal bis zur hälfte oder einemdrittel öffnet und dann kann
  man nach unten swipen oder es nach oben um es zu schließen oder den vollen bereich zu verwenden.
  Achte dabei darauf, dass man auch noch scrollen können muss in dem fenster. Vieleicht gibt es da
  schon UX know how wie man das richtig umsetzt"; app.css `.detail.is-module`, `enhance.js` „a
  module's sheet"): a module picked in the catalog's list, a program's page, the marked modules,
  „Mein Studium" and the Stundenplan comes up from below as a sheet over the page, to half the
  screen, with a grabber at its top. It follows what a phone's own sheets do (the „medium" and
  „large" detents of iOS, the half-expanded state of Material's bottom sheet): a finger takes it
  all up (`is-full`), the whole screen but a strip at its top, or down and away, which closes it.
  Half up, a finger moves the sheet and the content does not scroll yet; all up the content
  scrolls inside the sheet and keeps its scroll to itself, and a finger down where the content is
  at its top takes the sheet down. Pulled up past all up, the rest of the finger's way scrolls the
  content, and let go there the content goes on as the finger went, slower and slower. A tap on
  the grabber takes it all up and back. Its × stays at its top right while the content scrolls.
  It has no „Vollbild" (owner, 2026-10-16: „Vollbild als knopf ergibt keinen sinn, der muss
  raus"): all up it is the whole screen already; beside a wide page the module keeps it, and the
  Stundenplan's sheet keeps „Modul ansehen", which leads to what its panel does not show. Closed
  by a finger, its × or a tap beside it, it slides down and its close link is followed — back
  through the history where the page without the module is the step before (as the browser says
  it, else `data-back="history"`; § „Back"), so that Back does not bring it again; Back and Esc
  close it at once.
  The page behind it stays where it is, dimmed and still, the tab bar under it; a touch beside the
  sheet moves nothing. The pages under it keep what was theirs when a module was the page and is
  no more: the Stundenplan no longer scrolls back to where it was left when the sheet goes, since
  it never left it.
- **The module's sheet moves as a thing does** (owner, 2026-10-16: „Wenn man etwas anklickt, dann
  sollte es eine animation geben, wie es sich öffnet, so dass es nicht einfach erscheint … wenn
  man mit genug speed oder swipe weite das zu macht, dann soll von ganz oben nach ganz zu gehen.
  Ja und dann muss die animation zu der mitte auch physics based sein. Aktuell snapped das richtig
  komisch von voll zu mitte, selbst wenn man ganz langsam swiped. Das muss sich einfach natürlich
  anfühlen"; enhance.js „a module's sheet"). Each of its moves is a damped spring, played as Web
  Animations, which the compositor runs while the page's thread builds the module; the stylesheet
  only holds it where it rests. It comes up from below in one move, from its skeleton on to the
  module: before, the skeleton's entrance ended where the module came, which stood there at once
  for a frame and then came up from below again; on a phone, whose module takes longer than the
  step's own wait, the skeleton went, the list behind jumped to its top, and the module came up a
  second time. Let go, it goes on from where it is and as fast as the finger went, to where it
  would come to rest thrown at that speed (slowing down by 0.0025 px per ms²): all up in the upper
  half of the way to half up, half up down to a third of the way from there to the bottom, below
  that away; a flick (0.45 px per ms) goes at least to the next stop its way. So a slow pull comes
  to rest at the stop it is nearest as slowly as it was let go (a fixed curve of a third of a
  second set off at more than 2 px per ms whatever the finger's speed), a gentle flick down from
  all up stops half up, and a quick one (from about 1.4 px per ms where a finger lets go 100 to
  200 px down) or a long one takes it away — a flick down from all up always stopped half up
  before. Going away its spring is softer and without a swing, so that a thrown sheet keeps going
  rather than shooting off; to a stop the swing is too small to see. The page behind is less
  dimmed the further the sheet is below half up. A finger catches it on its way. A sheet that
  comes in the place of another (the module in its skeleton's, another module's through a link in
  it) goes on where that one was — on its way, at rest, or under the finger — and a skeleton whose
  page shows nothing beside itself while the module's answer is on its way stays on the screen
  until the module comes. The picture of the page held in front of the next one while its answers
  come (`pending::hold`) shows the page as far down as the window has it on a phone, where the
  window is what scrolls (it showed the page from its top). With reduced motion every move is
  short and without a swing, as the bottom bar's.
- **Local views (owner, 2026-09-24: „so, dass man das in jedem Tab ganz einfach implementieren
  kann als lokale Ansicht"; `folia/crates/widgets/src/local.rs`):** showing a module in place is one mechanism,
  not a feature of a page. The program page and the marked modules use it, and so will the
  semester plan. An area that lists modules gets it with five parts: its address implements
  `url::LocalView` (`open`, `full`, read and written by `url::local_from_pairs` and
  `url::local_pairs`); its page asks `local::filling` whether the module fills it (after
  „Vollbild") and then shows `local::ModuleInPlace` with `local::back_href` as „Zurück" (the page
  with the module beside it again), else its own content with `ModulePanel` beside it (on a phone
  a sheet over it) and `local::full_href` as „Vollbild"; its rows lead to the module beside the
  page (`Row`'s `preview`); `pending::change` names its steps with `local_change` (the module
  coming to fill the page is the module's page, going back is the page's column); and its
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
  the same numbers as `.matrix` in app.css). A column that gives way („Art"; in the list also
  „Bereich", where the page is at most 860 px wide) gets 0 px wide and keeps its cells, so that the
  credits stay under „LP" — **and its cells keep their text on one line** (`white-space: nowrap`,
  2026-10-14): in 0 px, `overflow-wrap: break-word` put every letter on a line of its own, hidden
  but just as tall, and on a phone every row of Informatik B.Sc. was 252 px high instead of 37
  („Grundstudium") and its head 121 px instead of 25 („Bereich") — the head of each of the 102
  programs whose plan names areas, the rows of 99 of them, on a phone and in a window of 1200 px,
  at worst a row of 1,484 px (Digitale Gesellschaft, „Orientierung in den Schwerpunkten -
  Gesellschaft, Recht und Ethik im digitalen Wandel"). A row is as tall as its name;
  `e2e/programs.mjs` checks it on a phone and at every width of the window.
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
  **On a phone the page starts with the program** (owner, 2026-10-15: „wenn man sich die seite von
  einem studiengang angucken möchte, dann ist das layout komplett unpraktisch"; the box above the
  page „ist ja mal komplett unnötig"): „Zurück" is the box at the head left of the search, and the
  way to the profile page gives way to it (`chrome::TopBack`, below); the views
  (Regelstudienplan, Wahlpflicht & Bereiche) end the head, under the line of the regulations
  (`program::ProgramViews`, in the HTML twice, the stylesheet showing the layout's); the sidebar
  ends the page (`Frame` without `sidebar_first`), with the way into the catalog and what is
  related, after the regulations. What the sidebar offered to do with a program went everywhere:
  „Als meinen Studiengang setzen" (the program is chosen in „Mein Studium" and on the start page;
  `MineButton` is gone), „In den Stundenplan", which made no sense there, and „Studium planen"
  with them. „Ordnungen & Dokumente" and „Im Verzeichnis der BTU" were a jump to the end of the
  page and a link: the end of the page is „Ordnungen & Dokumente" with the regulations and, last,
  the program's module tree in the BTU's course catalogue (QIS, `program.source_url`; „Modulbaum
  des Studiengangs"), on every program's page, also where no regulation is known.
  **Where the page is narrow** (its container at most 720 px; owner, 2026-10-15, of Informatik on
  a phone: „wie goofy der header bei Informatik aussieht"): the numbers of the head are one row,
  each its figure over its word, the room left shared among them — they used to wrap, „117
  FÜS-Module" alone on a second line behind its rule — and two rows of two on the narrowest phones
  (at most 330 px, where „116–126 LP" left no room for four). The line of the regulations says
  „PO 2008 - 2. SÄ 2024" (a screen reader still reads „Prüfungsordnung"), so „aktuell" stays on
  it; and wherever a part of that line wraps, its line starts without the dot (the dot stands in
  the room before each part, and the line is cut where that room begins). A name with a word
  longer than the line breaks inside it.
- **The way back at the head of a phone's page** (`chrome::TopBack`; owner, 2026-10-05 for a
  semester of „Mein Studium", 2026-10-15 for a program's page: „lass mal bitte noch den zurück
  button nach oben links neben die search bar packen. Und wenn der angezeigt wird, dann soll die
  profilauswahl ausgeblendet werden. Mach das mal als wiederverwendbares system", and the same day
  for every page: „mach den zurückbutton mal überall nach oben neben die suche, wo aktuell noch so
  einer gelisted ist"): a square box left of the search, as high as it; while it is there the way
  to the profile page right of the search gives way to it, inert, and the search keeps its width.
  The two change places as one row that glides (owner, 2026-10-17: „Aktuell ist die Animation noch
  etwas akward … Mach das mal smooth"): the box, the search and the way to the profile page move by
  a box and the gap, what comes fading in and growing a little, what goes fading out, in 280 ms on
  the curve of the glide over the keyboard. Nothing of the bar's layout changes (the box stands
  over the row's start, out of it; the row's end is clipped), only `transform` and `opacity`, which
  the browser runs on a thread of their own: the glide goes on while the page a tap opened is
  built. Before, width and margins moved in 180 ms, and a phone (the CPU four times slower) showed
  one or two of their frames on the way to „Alle Studiengänge" and none on the way back, the page's
  first 100–400 ms of work in their way.
  **Every „Zurück" of a page is that box on a phone**: `frame::BackLink` (a
  module's own page, a module in full in a program, the Merkliste, the Stundenplan or „Mein
  Studium", a program's page, an area or a row of the plan as a page of its own) puts it there,
  leading where the link leads and through the history where that is the step before
  (`TopBack::follow`; on a phone the box is Back, § „Back": what lies over the page closes first,
  and inside a tab's stack it goes one step back), and steps aside while the head has it
  (`.topbar.backed ~ .content .at-head`);
  a page's step that has no such link (a semester of „Mein Studium", its views on a phone) puts it
  there the same way while the step shows. The program overview has it too, up to „Mein Studium"
  as it was left (owner, 2026-10-17: „wenn man auf alle Studiengänge geht, dann fehlt da ein
  zurück button, um zur mein Studium ansicht zurück zu kommen"; `chrome::Up::Study`), through the
  history where that is the step before. Each page that put it there takes it away with it, and
  only its own (a number per `follow`): the page the app goes to may say where before the one it
  leaves is gone. On the way to another page the bar follows the click, the box there or gone at
  once. **The server writes the bar before the page** and knows nothing of its links, so the
  address says which pages have the box from their first paint (`chrome::back_at_head`: a
  program's page whatever fills it, and a module's own page, leading to the list of its area as
  it was left until the page says where — on the server's page its list, where the page's link
  leads too —, and the program overview, leading to „Mein Studium"): the server's page has the
  box and no way to the profile page where the app has them, and nothing moves at the takeover
  (`e2e/takeover.mjs`: the overview, a program's and a module's page on a phone); without
  JavaScript the box is the plain link, and the overview has none, „Mein Studium" being the app's
  alone (`.topbar.app-back`, app.css: the way to the profile page stays). What only the app
  shows (a module in full, an area as a page) gets the box when it comes. A wide screen keeps „Zurück (Esc)" in the
  page. Not ways back and so not moved: the × of a module's sheet, the pager's „Zurück", a menu's
  way back to its first level.
- **Dialogs: one frame in three sizes** (owner, 2026-10-16: „Die Modalen [Dialoge] sind nicht
  einheitlich und total unordentlich … Neben Close steht kein esc … Die Buttons für bestätigen und
  abbruch sind links … Modale menus springen zu viel in der größe. Erstelle dir mal 3 verschiedene
  größen"; app.css „The dialogs", `planner/src/study/dialog.rs`, `ui::DialogClose`). A kind of
  dialog picks its size by what it holds and keeps it while it is open, whatever its content does:
  **s**, a question or a note, 440 px wide and as tall as what it says; **m**, a form or an
  overview, 600 × 600 px („Studiengang wechseln", the areas, an area); **l**, a list to choose
  from, 840 × 720 px („Module hinzufügen", a choice's modules); each at most the window less 24 px
  all round, its body scrolling inside. Until then the width was the kind's and the height the
  content's: „Module hinzufügen" shrank from 720 to 310 px when its tab changed, „Studiengang
  wechseln" grew and shrank with the program picked. The head says what the dialog is and has its
  way out, „× Schließen Esc" as a module beside a list has it (on a phone the × alone; the map of
  the start page, a picture as large as the window allows and no dialog of these sizes, has the
  same way out); the foot has the buttons on the right, the primary last, and what it says about
  them on their left. On a phone a dialog is a sheet as wide as the screen — s as tall as its
  content, m three quarters of the screen, l all but its top — whose buttons share its width, the
  cancel too, at least 24 px over the screen's foot (`max(24px, 14px + safe-area)`; owner: „Beim
  handy müssen die buttons etwas weiter vom boden weg sein. Und die volle breite (zusammen)
  einnehmen"); a search field in it spans it 8 px off its edges, as a picker does.
- **The screen's keyboard** (`folia/crates/design/src/keyboard.rs`; owner, 2026-10-17: „dann muss das
  Fenster auch hoch gehen, wenn das virtual keyboard rein kommt, so dass man noch sehen kann was
  man macht. Also das modal kann ruhig full screen werden, wenn man das Keyboard öffnet. (Aber nur
  bei onscreen keyboard)"). A browser lays the keyboard over the page and tells it nothing: it
  shrinks only the part of the page it shows (`visualViewport`; Chrome on Android, Safari) or the
  page with it (Firefox). So it counts as up while a text field has the focus and the part shown is
  more than 120 px shorter than the page was at that width (the tallest seen, where a finger
  points; on a computer the window as it is): never with a keyboard of wire or radio, which covers
  nothing, nor on a computer whose window is made lower. The root then carries `data-keyboard` and
  where that part is (`--vv-top`, `--vv-height`). On a phone a dialog is then that part, from edge
  to edge, and so is the catalog's filters or another sheet; the dialog's foot (the buttons), the
  sheet's („1.200 Module anzeigen") and the tab bar wait for the keyboard to go. What is typed in
  goes to the top of what scrolls it, its name over it (a dialog's body, a sheet's, the page under
  its bar of the search), what scrolls it having the room to while the keyboard is up, so that what
  follows it shows: a picker, whose list ends above the keyboard (`--kb-room`), and any other text
  field, the buttons beside a profile's or a plan's name, the hits under „Module hinzufügen"'s
  search (owner, the same day, of the dialog of „Mein Studiengang": „Aber das kann ruhig bei jeder
  combobox/Textfeld passieren"); not one in what stays where it is (the bar of the search, a
  dialog's bar). Once the keyboard has gone, what scrolled is back where it was. Before, the sheet
  stayed at the screen's foot behind the keyboard, and the picker's list with it. **Nothing of it
  jumps** (owner, the same day, after a try on her phone: „Animation bitte. Aktuell springt das
  noch komisch"): the dialog or the sheet glides into that part and back by its top and height
  (`Element.animate` from where it stood to its new place, 280 ms, a glide under way going on from
  where it is), what is typed in scrolls up and back smoothly, and what scrolled keeps its room
  below (an out-of-flow mark, `--kb-pad`; padding pushed the dialog's foot out) a moment after the
  keyboard has gone (`data-keyboard-leaving`), so that it scrolls back rather than jumping to its
  new end. An open picker's button draws out to its list's width and back, the list unrolls from
  it (clipped, so that what it covers does not show through it). With less motion asked for, all
  of it is there at once. `e2e/study.mjs` takes the sheet's top and its body's scroll frame by
  frame.
- **„Mein Studium": the Studium tab plans the study** (owner, 2026-10-04; `folia/crates/planner/src/study`,
  `folia/crates/plans/src/study.rs`). Owner: „Primär soll man da sein Studium planen können. Also das
  aktuelle und zukünftige Semester … der Regelstudienplan ist ja eine Illusion". After a first
  version that laid the plan over the semesters by a rule, the owner's mockup round of the same day
  set what it is now: „Betula blockiert nicht. Die Nutzer sind erwachsene Menschen … Erlaube ihnen
  alles zu tun, was sie möchten", and Betula has what is relevant at hand, shown or one click away;
  the program is set once („so eine Sache, die man für sich halt so einmal macht"); a progress bar
  of what is passed, planned and not planned yet, split by area, with what goes beyond an area as
  overflow; the semesters begin empty and are filled from the Regelstudienplan's Fachsemester and
  the Wiederholer; on a phone one semester as a card turned by ‹ › and a swipe, at the end a page
  that adds a semester (first the overview above it and nothing between, „Danach sollten direkt die
  Semester kommen"; after a try on her phone the overview a page of its own, the semesters behind
  it, see „On a phone"); on a desktop the mockup's concept A by default („am sichersten … am besten
  zwischen Handy und PC") and concept C, calmed, as the second view. Her look at the page that day („Die
  drei Punkte sind in der Allgemeinheit verstanden als Kontextmenü", „Das Abhaken von dem Modul ist
  viel zu einfach … Änder das mal zu einer Multiselection für Bearbeitung", „Die klickflächen sind zu
  klein", the cards of the areas in one row) made the rows what they are now. So:
  - **Where.** In the app the Studium tab leads to `/study` (see „The areas are tabs"); the
    program's Regelstudienplan and its Wahlpflicht are views of the study, with the student's
    stand in them (owner, 2026-10-14, concept A of `folia/design/studium/konzepte.html`: „Konzept
    a ist schon mit Abstand das Beste"; see „The views" below), and the overview of all programs is
    one link away. The sidebar, top to bottom: the program's card (three lines without labels:
    „Informatik B.Sc.", „PO 2008" with the study direction, „3. Fachsemester · seit WiSe 25/26";
    the card itself is the button to „Studiengang wechseln"); „Ansichten": „Semester" (jetzt 3.
    FS), „Gesamtplan", „Regelstudienplan" (Soll · Ist) and „Wahlpflicht" (what is left to choose),
    the first two one address and a setting of the browser's (R13), the others addresses of their
    own (`view`), and the one switch between them (since 2026-10-19 no switch over the semesters
    says it again); „Studiengang": its regulations and documents on its page, and „Alle
    Studiengänge"; then the legend, the parts of the bar („Fortschrittsbalken") apart from the marks
    of a module („Zeichen an Modulen"), a word each, and the storage hint. Nothing in it says again what the page or the tab
    bar says (owner, 2026-10-04: „Mein Studium Sidebar ist noch sehr redundant und nicht platz
    effizient"): no label over the card, no way to the Stundenplan, whose tab is beside it. A
    phone has no sidebar: all of it is on its overview (below). A program's views are the
    plan and the areas, as the program's page for everybody (and for crawlers) has them; its „Mein
    Plan" is this page. The visitor's own program carries „Studium
    planen", the start page's step „Studiengang wählen" leads here once a program is kept, and so
    does the Stundenplan's „Studium planen →".
  - **The study as a card** (`card.rs`; owner, 2026-10-16: „Der Studiengang modale dialog ist
    designtechnisch echt schwach … Ich stelle mir das eher so als card vor, in der man dann
    editiert. Also so ein ding von ich bin in informatik 9. semester", and of the concepts in
    `folia/design/studiengang/konzepte.html` the same day: „option a mit karte finde ich auch am
    besten"). „Studiengang wechseln" and the first visit are one card: over its rows the sentence
    they make („Du studierst Informatik B.Sc. im 9. Fachsemester.", „Du wechselst zu Informatik
    M.Sc., ins 1. Fachsemester."), then a row each for the program (its picker reads as the value,
    no box until pointed at; under it the plan's length, „Regelstudienplan: 6 Semester, 180 LP"),
    the study direction (owner: „es gibt noch studiengänge, wie etechnik, wo man noch ausrichtungen
    hat"; „MIT und EET", „eine von 2"; only where the program has study directions, see below) and
    the Fachsemester with − and + (under it „seit WiSe 2022/23", the semesters of leave among
    them). The Fachsemester is asked for, not the Studienbeginn: the card keeps the semester
    that makes the current one that Fachsemester (`study::start_of_fs`, the semesters of leave not
    counted); below the first it is a study that begins in one of the next two semesters. A row
    that differs from what is kept is tinted and says in its line what it was, with „Zurück" („vorher
    Informatik B.Sc. · PO 2008"); what was before shows only once something changed. **Nothing
    moves while the card is used** (owner: „aktuell passiert es noch, dass das layout jumped, wenn
    man zum beispiel das semester ändert. Das darf auf keinen fall passieren. Wenn man eine aktion
    macht, wie semester +/- darf nicht unterbrochen werden durch layout shifts"): the sentence is one
    line (two on a phone), every row as tall whatever it says, its line one line cut with „…", a
    picker's list lies over what follows, the box under the card has one height whatever it says,
    the foot's „Übernehmen" says the same whatever changes and what it counts („2 Änderungen")
    stands on its left (on a phone over the buttons). `e2e/study.mjs` measures the card's parts
    before and after + and − (the first visit, a phone's sheet). The one move is the study
    direction's row (owner, the same day: „wenn ein studiengang keine ausrichtung hat, dann sollte
    das nicht eingeblendet sein, ansonsten ist das verwirrend. Mach mal ne nette animation, wenn
    sich das ausklappt"): it is there only where the program has study directions, unfolds as such a
    program is picked (its height from nothing to its own, what it says sliding down into its place
    a moment after) and folds away, quicker, for one without; while a program's plans are on their
    way it stays as it is, and folded it is out of reach of Tab (`inert`). With less motion asked
    for it is there or gone at once.
  - **The first visit** (`setup.rs`): while no program or no Studienbeginn is kept, „Richte dein
    Studium einmal ein": the card, empty („Wähle deinen Studiengang und dein Fachsemester."; the
    program kept elsewhere offered, the Fachsemester assumed by `studyplan::intake_start` until it
    is counted), and a box that fills the semesters before the current one from the plan, „1. und
    2. FS nach Regelstudienplan füllen" (`study::fill_past`, the Stundenplan's import per
    Fachsemester; then only marking what was passed is left), in the first Fachsemester there and
    saying there are none. „Studium anlegen" stores it in one go (`MyProgram::set_study`).
  - **„Studiengang wechseln"** (`side.rs`, a dialog of size m): the card with what is kept, and
    under it, once the program or its direction changed, what of the plan counts there before
    anything changes („Bestanden 36 von 46 LP kommen dort vor", the first modules that count nowhere
    there, which stay as „ohne Bereich"); where only the Fachsemester changed, that the plan stays as
    it is. Before any change and where only the Fachsemester changed, that is a line at the foot of
    the body over the dialog's foot, without a box (owner, 2026-10-17: „die änderungsbox, wo die
    Änderungen drin stehen sieht akward aus. Mach die mal weg und den Text nach unten bündig"; it
    was a dashed box under the card, on a phone cut off by the foot); with what counts there,
    „Nichts wird gelöscht …" stands there. Nothing of the plan is touched by a change, and changing
    back counts all as before.
    Another program starts in its first Fachsemester now, with its first plan; the program kept,
    picked again, brings back its Studienbeginn and its direction (owner, 2026-10-16: „wenn man den
    studiengang wechselt, dann ist es wahrscheinlich, dass das fachsemester wieder bei 1 ist. Aber
    wenn man zurück den aktuellen studiengang auswählt. Dann sollten die werte wieder zurück"), and
    so does a changed program's „Zurück".
  - **The overview** (`overview.rs`): „46 von 180 LP bestanden", the credits passed, planned
    („davon 6 über Bedarf") and not planned yet, and a bar of the areas, each as wide as it asks
    (and as what goes beyond it): passed in the area's colour, planned in its tint, open grey,
    beyond need striped with „+6" over it. On a desktop a card per area under it, all of them in one
    row and each in one line („Die Cards dürfen nur eine Zeile haben. Aber idealer weise auch ohne
    Scrolling"): „● Informatik 44/66", what is taken of what it asks, green with ✓ where it is all
    planned, in the area's colour beyond need, a long name giving way first; the rest („18 LP
    offen", „ganz eingeplant", „+6 LP über Bedarf") where the pointer rests. Nothing else: what was
    not passed and a semester heavier than the plan are said where they are, in the semesters and
    in „Passt in dieses Semester" (owner, 2026-10-04: „Da soll einfach nur zu sehen sein, wie steht
    es bei mir um meine LP"; until then hints under the cards, with „Einplanen" and „Markieren"). An area's
    card opens its dialog: what it asks and holds, the student's modules there, the plan's rows still
    open, its modules in the catalog (the tree's areas, the FÜS list) and the plan's page.
  - **The views** (owner, 2026-10-14: concept A, „Regelstudienplan und Wahlpflicht werden
    Ansichten von „Mein Studium" – mit deinem Stand darin"; `regular.rs`, `choose.rs`, the data in
    `folia/crates/plans/src/choices.rs`). Until then both were the program's pages, another world
    with another title and another sidebar, where nothing of the student's showed and nothing
    could be planned. Now, on a desktop under the overview, on a phone a page of its own behind it
    (the way back at the head, through the history where the overview is the step before):
    - **„Regelstudienplan"** (`view=plan`): the plan as its regulation prints it, a row a row of it,
      grouped by the areas of the progress in their colours (each with what is taken of what it
      asks), a column each Fachsemester with the semester it is for the student (the current one
      shaded). Every column's head has the same lines, so that the numbers stand in one row (owner,
      2026-10-16: „die semester so durcheinander angezeigt"; „WiSe 26/27" broke into two lines and
      „SoSe 27" did not, and the heads, aligned at their foot, put each number at another height):
      „jetzt" over the current one (its place kept in the others, none where no column is the
      current one), the Fachsemester, the half of the year, the year. The credits stand in the cell of the row's Fachsemester in the look of where the row
      stands — passed in the area's colour, planned in its tint, not passed and taken again in the
      warning's, open as a ring — and where the student has it in another semester a dashed cell
      there too; „Bei dir" says the semester, or offers „Einplanen" (into the semester it fits
      next, `study::open_lines`, „Rückgängig" as everywhere) and, for a row without a module,
      „Wählen ›" into the Wahlpflicht with its choice picked. The foot: what the plan puts into
      each Fachsemester, and what the student has there. „Mein Stand" switches the stand off: the
      plan as printed. On a phone the Fachsemester are sections with a strip to jump to (sticky),
      a line of what is passed of the ones before and a retake, a card a row.
    - **„Wahlpflicht"** (`view=areas`): what the plan leaves to choose (`choices::Choice`): its rows
      without a module, those of one name in one area together („Anwendungsfach" in the 3. and the
      4. FS, Maschinenbau's „Wahlpflichtmodul 1"–„4"), by the areas of the progress, those with
      something open first; each with what the plan says of it (credits and Fachsemester, one of
      several lists, a sum it shares with others: „Zusammen mit Praktische Informatik und
      Angewandte und Technische Informatik 44 LP"), its lists (the tree's areas its name points at,
      `plan::areas_for_row`, as tabs where there are several: „Mathematik", „Physik" …; else the
      areas of its progress area where a student chooses; the FÜS the catalog's FÜS list), and the
      list's modules with their turnus. The semester to plan into is the one the plan puts the
      choice into (not before the current one) or the one after it: what it does not offer waits
      under „N weitere, nicht im SoSe angeboten · alle zeigen", and „Alle" plans each into the next
      semester that offers it. „+ In SoSe 27" plans the module (`study::add`). On a desktop the
      choices on the left and the one picked on the right; on a phone the areas as chips and their
      choices one under the other, six modules each before „alle zeigen". What is on the Merkliste
      (Folia 1.37.0; konzept.html, „Von überall") comes first after what the student has, offered
      in the semester or not (it was kept for a reason), with the bookmark; a choice says how many
      marked modules its lists hold („1 gemerktes zählt hier"), on a phone its area's chip too.
    - **What is the student's.** Which module fills which row is never guessed (as everywhere,
      `studyplan::Planned::fills`): a row without a module stands as the student put it — its
      placeholders, the modules that fill them, its mark as done. What the student planned or
      passed of a choice's lists is said of the choice as a whole („8 von 12 LP gewählt", on the
      row of the plan too), as the progress counts a module towards an area by where the tree
      places it; a module counts towards one choice at most, the first in the order of the plan
      that lists it and has room. „Noch 64 LP zu wählen" is what the choices still ask for, a sum
      several share counted once.
  - **On a phone** (`phone.rs`, owner, 2026-10-05, after a try on her phone: „die landing Page vom
    Studium relativ komisch und unübersichtlich. Mach das mal so, dass man oben die progressbar hat
    und darunter gleich die legende dazu. Danach kommt der Studiengang und Dann kommen die links zu
    regelstudienplan und Fachbereichen. Danach kommt dann eien box studium planen, mit einer preview
    vom Studium … wenn man da rauf klickt oder nach links swiped bekommt man den plan vom semester …
    Unten sind dann die rest links aus anpassen"): an overview, and the semesters on either side of
    it. The overview: the headline with „Bereiche ›" and the bar, under it what the bar's parts are
    with their credits („46 bestanden", „50 geplant", „84 offen", „über Bedarf" where some is), a
    tap anywhere on the box opening the areas as a sheet (owner, 2026-10-06: „oben die progress bar
    komplett zu einer clickfläche werden, um das menu für den Progress anzuzeigen, aktuell klickt
    sich das auf dem handy komisch", and the same day: „nicht nur der ganze balken, sondern die
    ganze progress box muss klickbar sein"; until then „Bereiche ›" and the bar were two buttons and
    the rest of the box nothing, `.st-progress-open` now reaches over all of it, to its edges, and
    the box darkens a little under the finger); the program's card and the ways to the views, each
    with a line of where it stands („Regelstudienplan — Soll und Ist · 1. und 2. FS: 52 von 60 LP
    bestanden", „Wahlpflicht — Noch 64 LP zu wählen · Nebenfach, Fachstudium"); the box „Studium
    planen" with „Jetzt WiSe 2026/27 · 8 von 30 LP" and a column per semester (what is passed and
    planned there stacked, the plan's credits of its Fachsemester as the track behind, its
    Fachsemester under it, the current one set off); at the end „Alle Studiengänge" („Nach
    Fakultät, deiner obenauf") and the storage hint. The sheet „Anpassen" is gone. The overview
    is a page of the semesters' row (owner, 2026-10-06: „als erstes muss die landing page für mein
    Studium mit auf den pager, so dass man gleich durch swipen kann. Und dann sollten alle
    vergangenen Semester auf die linke seite also so dass man nach rechts swipen muss, um die zu
    sehen"; on 2026-10-05 the overview and the semesters had been two pages side by side, the
    semesters a row of their own inside the second, and before that only the box „Studium planen"
    was drawn to the left): the semesters that are over, the overview, the current semester and the
    ones to come, the page that adds one, in one row the browser scrolls by itself (`pager.rs`). It
    opens at the overview; a finger anywhere on it draws the whole page to the left, to the current
    semester, or to the right, to the last that is over (a study begun this semester has nothing
    there, and the overview comes first). The box still leads to the current semester by a tap, a
    column to its semester, and the row glides there. What a tap sends the row to (the box, a
    column, the way back, ‹ ›, Back) it glides to in a quarter of a second, quick at first and
    slowing into place, the app's own frames, so that the tap shows where the page lies and the
    finger learns the way (owner, the same day: „die Animationen müssen more snappy sein. Aktuell
    sind die sehr langsam und träge. Die sind primär dafür da, dass man versteht wie der
    positionelle Zusammenhang ist, so dass die gesten intuitiv werden"): 240 ms for a page, 40 ms
    more for each further one, at most 360 ms, the first move in the frame after the tap (the
    browser's smooth scroll took 300 ms for a page and 430 ms for two, and was slow to begin). A
    finger's fling and the snap after it are the browser's; a tap while they still move the row
    takes it from the browser at once (owner, 2026-10-06: „wenn man in dem studium tab swiped,
    funktioniert der button in der nav bar auch erst, wenn die seite sich nicht mehr bewegt. Das
    muss auch unbedingt gelöst werden. Wenn man den anklickt muss es sofort wieder an die standard
    position gehen"; until then such a glide waited until the row rested): while it glides the row
    is no scroller (`is-gliding`, `overflow: clip`), which alone ends what the browser moves of it —
    under `overflow: hidden` the browser's snap went on in time and came back the moment the row
    was a scroller again, and no scroll of the page's stopped it, nor did a scroller gone for one
    frame — and its pages are drawn where it glides (`dom::shift_row`, a `translate` each); at the
    end it is a scroller again, scrolled to where it is drawn, in the same frame. Measured in Chrome
    with CDP touches: the tab of „Studium" or the way back tapped while the row still flings to the
    next page, the row turns in the next frame or the one after (22–30 ms after the click) and is at
    the overview a quarter of a second later, without a move of the browser's after it. A finger
    on the row makes a glide wait until it lets go. The way back slides in and out in 180 ms, the sheets come up in 200–220 ms, the dots change
    in 150 ms. Where the row comes to rest its entry of the history keeps the semester, no step and
    no address of its own (2026-10-18, § „Back"; until then `/study?plan=1`, a step after the
    overview), and Back, the links and the way back move the row in turn. At its ends the row keeps the finger's move to
    itself, and the window, while it is there, has no horizontal overscroll of its own, which a
    browser could take for a swipe back through the history (`overscroll-behavior-x`). The page that
    comes in shows from its top, wherever the window was scrolled to in the one before, as a page of
    an app does: while the row moves the pages it does not rest at are drawn as far down as the
    window is past its top (`dom::lift`, as a finger comes down on it), and as soon as the row moves
    the window scrolls the page it heads for, from its top to its end, and nothing of the page it
    leaves (`dom::turn`): the window goes up as far as the pages coming in were drawn down and they
    come back to their places, the page that goes is drawn as far up, where it was, and counts for
    nothing of the row's height, which is the height of the page it heads for (`is-turning`,
    `data-to`), all in one frame (owner, 2026-10-19: „wenn man bei einem langen semester ganz nach
    unten scrolled, dann zur seite swiped und dann nach oben scrolled, wird der vertikale space von
    der gerade verschwinden seite noch als scrollbar angesehen, bis sie verschwindet und dann springt
    es wieder zurück … Ich glaube der beste fix ist da einfach den overflow für die page auszustellen,
    die weg geswiped wird"). The row reaches up under the bar at the top, so that the page that goes
    is cut off at the window's edge as before, not under the bar. At rest the window stays where it
    is (`dom::land`): what is scrolled while the row moves stays, up or down, and the page that was
    left starts at its top again, off the screen (the same day: „wenn man vertikal am scrollen ist
    während man swiped. Dann springt das am [Ende vom] swipen wieder zum Anfang. Mach das mal lieber
    so, dass sich das scrollen off Screen resetted wenn man weg geswiped hat"). Back at the page it
    left (a short pull), the row is as before at once (`dom::unturn`), not only once it rests. The
    window keeps no scroll anchor meanwhile; a finger that comes down while a page coming in shows
    leaves it where it is drawn. The page the row rests at alone can
    be used; the others are let go of at rest, not under the finger (a whole page made inert cost
    the frame it happened in on a thread four times slower). A semester's way back, on either side
    of the overview, is a square box left of the search at the head of the screen, as the search is
    (owner, 2026-10-05: „wenn man in der semester ansicht ist, soll es oben links neben der search
    bar im gleichen style eine quadratische box sein mit einem zurück pfeil"; 2026-10-06: „Mach bei
    beiden Richtungen weiterhin den pfeil oben hin um zurück zu kommen"; `chrome::TopBack`, which a
    page with a step below its first one fills while that step shows; until then „‹ Übersicht" over
    the cards): it brings the overview in place, as Back does (the semester is a state of the
    entry, which Back undoes first; § „Back"); Esc, a close request in Chrome, closes the
    semesters' menus and dialogs first. It slides in from the left as soon as the row leaves
    the overview, under the finger, in a fling or a glide (owner, 2026-10-06: „Der zurück wird
    aktuell erst eingeblendet, wenn man dann aufhört zu swipen. Aber der sollte eigentlich sofort
    eingeblendet werden, wenn man weg swiped, von der main page"; measured: with the first move of
    the finger, 2–5 px from the overview), the search making room, and out again as soon, with the
    row's first move back into the overview from the page next to it (a few px, a finger's tremble
    aside), under the finger, in a fling or the snap, and with the first frame of a glide of the
    app's to it (owner, 2026-10-18: „ich finde das einblenden und ausblenden sollte gleich schnell
    gehen … verschwindet aber bei der home page, erst wenn ich aufgehört habe zu swipen und der
    screen still steht"; before, it went once the row rested at the overview); a row that turns out
    again before it is there brings it back (`pager::way_back_shows`). Pressed while the address
    still says the overview (the row on its way from it), it brings the row back at once, as the
    tab of „Studium" tapped again does from anywhere
    (`tabs::TabAgain`; at the overview the window goes up to its top). The dots stand over the tab
    bar, a dot for each page of the row, the overview's a little square between the semesters that
    are over and the current one, there on the overview too; they stay while the page scrolls
    (owner, 2026-10-05: „Die Legende und die swiping dots sollten fest über der nav bar sein"), in
    no box and with no legend of the rows' marks (the same day, after a look: „Mach mal die Legende
    weg und die Punkte nicht in eine box, sondern einfach nur den grauen Hintergrund mit blur zum
    content"): on the page's grey, frosted over what passes under it as the bar at the top is, from
    the screen's foot and edges (the tab bar floating over it) to a little above the dots, where it
    fades into the page. No ground comes over them: the row has none under it, on none of its
    pages (owner, 2026-10-19: „wenn man nach unten scrolled zum footer und dann weiter swiped dann
    gibt es ganz komisches behavior. Ich würde den footer bei den semester ansichten einfach raus
    nehmen. Der hat da eh keinen Mehrwert"), and its end stands above the dots. The dialogs („Bereiche",
    „Studiengang wechseln", „Module hinzufügen") are sheets from below that a finger drags down to
    close them, as the catalog's filters (owner, the same day: „Das muss auch unbedingt dazu, dass
    man die einfach wieder runter sliden kann"; `dialog[data-sheet]`, `enhance.js`): the sheet
    follows the finger and the dim behind it fades, let go far enough or flicked it goes on down and
    closes, short of it it slides back. „Studiengang wechseln" says no more in its head than its
    title (owner, the same day, of its lead „Das machst du selten …": „Mach den weg").
  - **The semesters** (`focus.rs`, the view „Semester"; in the catalog's look since 2026-10-19, Folia
    1.31.0): on a desktop the semester in focus at the top of its box (owner, 2026-10-19: „Die
    aktuelle Auswahl mit (z.B. WiSe 2022/23 1. FS) kann dann nach ganz oben in die box wandern. Die
    Option mit links und rechts für vorgeriges/nächstes semester soll bleiben"; until then a strip
    of every semester stood over it — what each held of what the plan puts in, a load bar, „jetzt",
    „+" — and over that a line with „Semester" and the switch „Semester | Gesamtplan", which went:
    „Es verbraucht zu [viel] Platz und bietet wenig Mehrwert", „um mehr platz zu schaffen"; the
    sidebar's „Ansichten" switch the views): its head
    with ‹ ›, its name and its „⋯" (`menu.rs`: „Im Stundenplan öffnen", „Alle auswählen",
    „Urlaubssemester", and „Semester entfernen" for an empty one added beyond the plan), its rows
    under the heads of their columns, and „Module hinzufügen" as the last row; an empty one is a
    mark, „Noch keine Module" and „Module hinzufügen" right under it (owner, 2026-10-05: „keine Module
    sollte mit icon und gleich modul hinzufügen sein"), no sentence on where modules come from. Its
    name, on a phone as well, is a button with a chevron that opens the list of every semester
    (`MenuFor::Pick`; owner, the same day: „Dazu soll man aber auch auf das semester selbst klicken
    können, um eine selection (maybe combobox) zu bekommen, um direkt das aktuelle semester auswählen
    zu können"): each semester with its Fachsemester and, at its end, what the strip said of it („24
    von 32 bestanden", „32 von 30 LP" with a warning where it is more than the plan puts in, „noch
    leer · Plan 30"), „jetzt" at the current one, a check at the one shown, which has the focus, and
    „Semester anfügen" after the last; a menu at the name on a desktop (its left edge at the name's),
    a sheet from below on a phone, where the row glides to the semester picked. Beside it „Passt in
    dieses Semester" (`study::fits`): the plan's open rows of its Fachsemester, the Wiederholer of the
    semesters before it, what is still open of the Fachsemester before it (the earliest first, four
    of them), each one „+" from being planned here; never a row of a Fachsemester after it, and only
    what the semester offers (owner, 2026-10-04: „Es darf nur vorgeschlagen werden, was in dem
    Semester oder zuvor angeboten wurde … im SoSe soll kein WiSe modul angezeigt werden"). Where the
    open rows of its Fachsemester are all of the other half of the year it says so („Was noch offen
    ist, wird im SoSe nicht angeboten."); a semester of leave has those of the Fachsemester reached.
    *A row* is a module as the catalog's list has it (owner, 2026-10-19, on a phone: „das aussehen,
    von den Modulen im semesterplan zu weit von den modulen im Modulkatalog abweicht … Gestalte das
    so, das der aufbau komplett gleich ist. Also padding, margin, Titel, LP, und format von den
    Metadaten", and on a desktop: „Auch hier soll 1 zu 1 der Style vom Modulkatalog übernommen
    werden, so dass man einfach versteht, dass es sich um ein Modul handelt"): the catalog's `.row`,
    its title over the line of its number, its area where the catalog puts a module's kind (a dot in
    the area's colour and its name) and its marks as the catalog's flags, its credits with „LP", and
    on a desktop the catalog's columns as well (`folia_widgets::list::Facts`: Verantwortlich,
    Prüfung, Turnus, Sprache, Termine; the table is the container `list`, so that they go one by one
    as it gets narrower, as the list's do), every other row shaded, under the heads of the catalog's
    columns. At its end, where the catalog has the bookmark and as large, its status (the same day:
    „Die merken flag soll getauscht werden mit einem Status symbol (In progress/bestanden/nicht
    bestanden)"; `focus::Status`): a ring for planned (a semester to come), a ring half filled in the
    accent of „jetzt" for under way (the current one), a check on green for passed, a cross in red
    for not passed (in a semester that is over); the sidebar's legend names them under „Zeichen an
    Modulen". The whole box is the module's button (the same day: „Der Modul Soll auch nicht mehr
    anklickbar sein und auch nicht unterstrichen werden wenn man darüber hovert/anklickt. Es soll die
    gesamte modulbox als button funktionieren"): its link (`.st-open`) reaches over all of it, so a
    click anywhere on it opens the module beside the page, the name underlines no more and is no
    target of its own; a row of the plan that names no module leads so to its modules in the catalog
    („Modul wählen", `variants::row_query`). Until then the name was a link of its own that
    underlined, a box to tick stood before the row and a „⋯" after it. The status, or a right click
    on the row, opens its menu at the status or the pointer (`menu.rs`): „Modul wählen" for a
    placeholder, „Als bestanden markieren" (or „Bestanden zurücknehmen"), „Verschieben nach ›" (a
    submenu of every semester it can go to — one that is over only for what was done or not passed
    there —, each with its Fachsemester and whether it offers the module), „Auswählen", „Entfernen";
    not the module, which a click on the row opens (owner, 2026-10-05: „modul ansehen aus dem kontext
    menu nehmen, weil man ja per klick auf das feld das auf macht"), only in the Gesamtplan, whose
    click opens the menu. The arrows go through a menu, → opens the submenu and ← closes it, Escape
    or a click beside it closes it and gives the focus back. *Selecting* (`Selection`) rows for what
    is done to several needs no box to tick (owner, 2026-10-19: „Die selection soll nicht mehr per
    checkbox umgesetzt werden, sondern per hervorheben/vlt texture funktionieren"): a row selected is
    tinted in the accent, lined with it and hatched. Ctrl or ⌘ and a click selects a row or lets it
    go, Shift and a click the rows up to it, a mouse pressed on a row and drawn onto others every row
    it passes (or, begun on a selected row, lets go of them; `focus::Draw`, which the list hears, as a
    mouse's moves go to the row under it), „Auswählen" in a row's menu the row and the semester's
    „Alle auswählen" all of them; while rows are selected a click on a row selects it or lets it go,
    and the space bar on a row does the same. Shift and ↑ ↓ at a row take in the next one, or let go
    of the last coming back towards where the range began (`focus::step`), and Ctrl or ⌘ and A selects
    every row of the semester in view (Folia 1.36.0, as the Merkliste has them: oberflaeche.html of
    the Merkliste, „Neu, für beide Listen"). A bar takes the place of the columns' heads, as tall as
    they are, so that no row moves: „3 ausgewählt · 18 LP", „Alle auswählen" while some are not,
    „Als bestanden markieren" („Bestanden zurücknehmen" where all are), „Verschieben nach" (the
    semesters as a menu), „Entfernen", „×", its words short where the table is narrow (`@container
    list`); Escape lets go too, and so does turning to another semester. A row is no longer dragged
    onto another semester: the strip it was dropped on is gone, and „Verschieben nach" moves it (the
    Gesamtplan's entries are still dragged between its columns). On a phone the semester is a page of the row the overview is a page of (`pager.rs`;
    owner, 2026-10-05: „die swipe bewegungen nach links und rechts brauchen Animationen, damit man
    versteht was passiert …", and after a try on her phone the same day: „das swiping ist richtig
    komisch das stockt immer wieder … dass das sich wie eine native app anfühlt"): every semester a
    page, side by side in a row the browser scrolls by itself and stops at a page, one a swipe
    (`scroll-snap`), in the look of the catalog's list (the same day, with a picture of the
    catalog on her phone: „mach mal das Design der Semester pages angelehnt an das design, dass
    schon im Modulkatalog etabliert wurde. Also die rows pro modul sollen ähnlich aussehen. Es soll
    keine box geben, und der text für welches Semester gerade ist oben unter die suchbar auch ohne
    box"; until then a card as wide as the page): no box around the semester, its head right under
    the search as the catalog's count is (the semester, „jetzt", under it its Fachsemester and
    credits; at its end ‹ › and its „⋯" in a box as the catalog's „Filter"), and a row as the
    catalog's card on a phone (its padding, its title, its line of number, area and marks, „DE · 2
    Termine" under them, its credits in LP, its status where the card has the bookmark, 44 px as
    that is); „Module hinzufügen" under them a card to
    be filled, with a dashed edge. What is on two pages lies as far apart as from the screen's edge,
    12 px (owner, the same day: „dass die Abstände zwischen den boxen genau der Abstand zum rand
    ist, so dass sich das nicht doppelt, wenn man swiped"; until then 24 px): a page's place has
    half of it on either side and the row half of it at its ends, a page rests in the middle, and a
    step is a page and its space; what lies at a page's edge draws its ring inside it and its
    shadow ends at the page's sides, so that the pages beside the one in view, which end at the
    screen's edges, draw no line and no shade there (`--shadow-lift`, the panel's shadow without its
    ring). A finger moves the row as it moves any list, on the browser's own thread, with its fling
    and its bounce at the ends, and nothing of the app runs while it moves; ‹ › glide it on to the
    semester before or after in a quarter of a second, past the overview where it lies between
    them. (The cards the app moved under the finger itself, two rounds
    before, stalled: on a thread four times slower, as a phone's, frames of 100–117 ms and up to
    340 ms of the app's work a swipe, the neighbours drawn as the finger came and the cards drawn
    anew as it went; the row the browser scrolls has none.) The app hears only where the row is:
    the dots follow the page in the middle, and the page the row comes to rest at alone can be used
    (the others are inert) and is the semester in focus. The row is as tall as that page (the
    others count for nothing) and at least as tall as the screen under the bar, so that a page
    coming in shows as far as the screen does; it changes at rest, never under a finger, and a page
    shows from its top as it comes in (`dom::lift`). It is the end of the page, with no ground after
    it (2026-10-19): one came in from down in the ground only in the strip above it, cut off, the
    ground standing still under it, and at rest the window jumped up. The view is as tall as the
    screen as it is (`dvh`), not as at its largest (`lvh`, which put the ground just past the
    window's edge): a page shorter than the screen does not scroll by the height of the browser's
    bars into nothing (owner, the same day: „irgendwie kann man noch scrollen, obwohl da kein
    content ist"). The row comes with the page it opens at and its
    neighbours drawn, the rest once it is there, two a step, and the neighbours of a page it comes to
    rest at at once, never while a finger is on the row or it moves (with the CPU four times slower,
    no long task while it moves: what a page's coming to rest asks, 50–100 ms there, comes after,
    and a glide's frames take 30–50 ms there).
    While rows
    are selected a finger selects, and the row does not move sideways. Under its rows what the plan
    still has open for its Fachsemester and the semester offers, under a heading as the catalog's
    semesters have one („Laut Regelstudienplan · 3. FS" and a line), each the catalog's card as the
    semester's own rows (owner, 2026-10-19: „Es gibt keinen grund, dass der hintergrund anders sein
    sollte"; until then open cards, a ring and no ground) with its number, area and credits and a
    „+" where the bookmark is that plans it here (owner, 2026-10-05: „die module, die noch
    vorgesehen sind, sollten auch als rows organisiert sein und nicht als fließtext"; a desktop has
    them beside it, „Passt in dieses Semester"); a long press selects a row, and the finger held on
    and drawn up or down selects every row it passes (the page does not scroll meanwhile), a tap on
    a row selects it or lets it go; the bar takes the tab bar's place over it (owner,
    2026-10-05: „Mach das mal so, dass die die navbar überdeckt"), and a menu is a sheet from below
    whose submenu takes its place (with „‹" back). After the last semester a page adds one („WiSe
    2028/29 anfügen · 7. Fachsemester", as a semester of leave if ticked). A row's marks:
    „Wiederholung · aus 1. FS", „im SoSe nicht angeboten · nur WiSe", what it asks for and is not in
    place („setzt voraus … – noch nicht eingeplant", `picker::needs_text`, only what is asked for,
    not what is recommended), „über
    Bedarf · FÜS hat schon 6 LP", „Platzhalter" (a row of the plan without a module, with „Modul
    wählen": its modules in the catalog, `variants::row_query`), „gefüllt mit …", and in a semester
    that is over „nicht bestanden · wieder im …" or „… noch nicht wieder eingeplant · Einplanen".
  - **The Gesamtplan** (`grid.rs`, a desktop's second view, switched to in the sidebar and kept in
    `localStorage` `betula.study.view`): the areas as rows (with their figures and a thin bar), the semesters as
    columns (the ones before the current one folded into „Bisher", „Bisher aufklappen"), each cell
    the area's modules in that semester as plain lines; in grey where it fits next what the plan
    still has open (`study::open_lines`: a row at its first Fachsemester, not before the current
    semester, in the next one that offers it; a Wiederholer in the next semester that offers it), a
    click adds it; „Modul hinzufügen" on a cell's hover offers that area's modules for that semester
    (`picker::CellDialog`: the tree's areas below it as filters, those offered then first, the rest
    folded; in each the area's marked modules first, marked — asked for of their own, so that no
    limit of the list leaves one out); a click on a module (or a right click) opens its menu, the semesters' row menu; a
    module dragged into another column moves there.
  - **Adding, moving, taking out** (owner: adding and removing were „eine absolute Krise"):
    „Module hinzufügen" (`picker.rs`) for a semester lists the rows of a Fachsemester of the plan
    (its own; „Plan-Semester" picks another), each with where it stands (passed, planned elsewhere
    „– würde hierher verschoben", planned here), its turnus, what it asks for of other modules
    (`study::needs`, from the catalog's prerequisites: in place it reads „empfiehlt … ✓", where
    something is missing only that, in the warning colour where it is asked for), then the
    Wiederholer, and „Im Katalog suchen" (the catalog's search, the program's modules or all).
    Once something is marked, the Merkliste is a third source between the two (Folia 1.37.0;
    folia/design/merken/konzept.html, „Von überall"), and the three tabs have short names, so that
    they fit a phone side by side: „Regelstudienplan", „Merkliste", „Katalog". „Merkliste" lists
    the marked modules in the order of marking, by collection (chips with their numbers, where
    there is one) and by a search of their titles, numbers, notes and collections, each with what
    it means for the semester as the catalog's search says it (offered or „im SoSe nicht
    angeboten", planned elsewhere „– würde hierher verschoben", passed, the area it counts in,
    what it asks for); what the semester offers first, then what it does not, what is done last.
    Its modules are asked for in the browser once the tab is opened (R9). What is ticked stays
    ticked across the tabs. Wherever the dialog lists a marked module, it says so with its
    collections („gemerkt: Wahlpflicht"). The plan's open modules that the semester offers come ticked for its own Fachsemester;
    „Einfügen (n)" says what the semester comes to then against the plan. Marking as passed,
    moving and taking out are a row's menu and the bar of the rows selected (`menu::Act`, the same
    for one and for several); marking is no box to tick by the way any more (owner: „Das ist eine
    Aktion, die man idealer weise nur einmal macht"). Every change answers in the next frame (R21);
    each of them says so at the bottom with „Rückgängig". Nothing is refused: a module that the
    semester does not offer, a semester heavier than the plan, credits beyond an area are said,
    never blocked.
  - **The rule** (`folia_plans::study`, pure, tested on every plan of the snapshot). *The areas*
    (`areas`): a row of the plan belongs to the thesis, to the FÜS, or to a group by its
    `subject_area` (Informatik B.Sc.: „Komplex Informatik" …), else the top of the program's module
    tree where its module or its choice lies, else its `study_section`; rows the regulation lets
    the student choose between (a sum over rows of ranges, `plan::choice_of`) are one group, which
    joins the group of their section; at most nine areas, the smallest together as „Weitere". An
    area asks for its rows' credits, a choice by the sum the regulation prints over it; it holds
    the tree's areas named like it and those its choices lie in (with the areas below them) and the
    areas its modules are placed in. Informatik B.Sc. comes out as the mockup had it: Informatik 66,
    Mathematik 24, Nebenfach 18, FÜS 6, Fachstudium 54, Bachelor-Arbeit 12. *Counting*: what is
    passed first, then what is planned, semester by semester; an item counts towards its plan row's
    area, else the first area of the tree's placing with room left, else the FÜS for a module of
    the FÜS list or the catalog's FÜS; what goes beyond an area's need is „über Bedarf" and counts
    nowhere else; a module of no area counts as „ohne Bereich". A row of the plan counts until
    modules fill it. *The semesters*: from the Studienbeginn (or the first semester anything is in)
    to the current one, the end of the Regelstudienzeit, the last that holds anything and the last
    the student added; each with its Fachsemester, none for a semester of leave, which moves the
    later ones on. What was put into a semester that is over and not marked as passed there is not passed
    („nicht bestanden"); until it is planned again or passed later it is a Wiederholer, and taken
    again it says so and stands first.
  - **What is stored** is only what the student says, in the Stundenplan's store (R20): the `m` and
    `p` lines are the semesters' modules and placeholders — the Stundenplan's lines: a semester's
    timetable is its part of the study, one plan for both pages —, `d <semester> <module>` a module
    passed, `q <semester> <program> <ord> <caption> <name>` a row of the plan done. „Mein
    Studiengang" keeps the program, the plan of the study direction, the Studienbeginn, the
    semesters of leave (`leave`) and the last semester added (`until`). The privacy notice lists
    them („Speicher im Browser").
  - **And the Stundenplan.** A semester planned here is the Stundenplan's semester and the other
    way round; its tab counts the current semester's modules. Its „Importieren" has „Mein Studium"
    as its second source: the Wiederholer into the semester shown (`study::retake_import`). „Plan
    leeren" empties the semester shown and leaves the others and what was passed.
  - `node folia/e2e/study.mjs` checks the first visit, the overview (the credits, nothing else), the
    sidebar (the program's card and its dialog, no way to the Stundenplan, the legend in two
    parts, the statuses in the second), the semester at the top of its box with no strip and no
    switch over it, the list of the semesters at its name (what each holds, „jetzt", the one shown
    with the focus, Escape), the rows as the catalog's (its row and columns, the status, no box to
    tick, no underline under the pointer), what fits a winter and a summer,
    a row's menu at its status (without the module; marking as passed, „Verschieben nach ›",
    „Auswählen", removing, „Rückgängig", a right click, the keys: Enter, the arrows into the submenu
    and back, Escape and the focus back),
    selecting (the semester's „Alle auswählen", the rows hatched, a press drawn over three, the
    bar, Ctrl and a click, two rows moved by the bar, Escape), „Module hinzufügen", an area, a
    module beside the page by its name and by a click at its credits, the Gesamtplan (each cell as
    tall as what it lists, its menu, its grey lines), the phone
    (the overview in its order and
    without a sidebar, one row of the semesters that are over, the overview and the rest, the
    overview the page in view and no way back at the head, the row's dots on it; the box by a tap
    and a column; the way back a square box left of the search, as the search is; the
    semesters without a box, their head under the search, their rows the catalog's cards (the
    catalog's padding and title, number, area and LP, the status where the bookmark is, 44 px); no
    ground under the row and its end above the dots, the pages 12 px from the screen's edges
    and from each other, the dots fixed over the tab bar on the frosted grey, without a box or a
    legend, an empty semester and the plan's rows with their „+", ‹ › gliding and past the overview,
    the glides of a tap quick (the box, ‹ past the overview, the way back: at most 280–360 ms from
    the first move to rest), a real finger moving the row and a short pull going back, the last
    semester that is over drawn to the overview, a semester scrolled while the row moves staying as
    scrolled at rest, up from the end of one into the next with nothing of the one that goes above it, the window scrolled into a semester and the next
    shown from its top, a long press (the row hatched, no box) and the bar over the tab bar, a tap
    selecting one more, the menu at a status as a sheet without the module, a semester added, the way back and Back without a step more, the credits opening the
    areas from their headline, their bar, their legend and next to each edge of their box, the
    areas' sheet dragged a little and back and dragged down to close, the program's card without a
    lead, the whole overview drawn by a finger a little and back, to the left to the current
    semester and to the right to the one before with the way back at the head, the first semester
    holding a finger to the right, the way back there while a finger still holds the overview a
    little drawn and gone once it is back, the tab of „Studium" and the way back tapped while the
    row flings to the next page bringing the overview at most 450 ms after the click and nothing
    moving after, a module of a semester as a sheet over the row, half the screen high, drawn down
    and away with the row still at its semester) and the Stundenplan's import; a finger is the
    browser's own touch (CDP), so that the browser scrolls. The glides are measured where the row's
    first page is drawn, since a glide draws the pages of a row that does not scroll meanwhile.
  - `node folia/e2e/phone.mjs` checks a module of the catalog as a sheet: coming up from below in
    one move to half the screen, without „Vollbild", all up by a finger, its content scrolling there
    and back, down a third to half, down and away (the list where it was, the history as before the
    module), a flick up, let go slowly on its way down from all up gliding to half up no faster than
    1.6 px per ms, a gentle flick down from all up to half up and a quick one away, a tap beside it,
    its ×, Back; and a tab held: „Studium"'s three ways (the finger's release going nowhere, a tap
    beside closing them, a way taken), a short tap as ever, the catalog's list with its filters.
- **The search in the top bar belongs to the page:** modules everywhere, programs on `/programs`.
  In the browser app it filters while typing (history entry replaced, not added). How it finds and
  orders modules: „The search of the catalog“ below.
- **„Nach oben" (owner, 2026-09-26: „wenn man im Modulkatalog ne weile gescrolled hat, … richtig
  schwierig wieder nach oben zu kommen"; `ui::ToTop`, `enhance.js`):** one round button for every
  page, in the corner of what scrolls. On a wide screen that is the page — what the ground takes
  for the page: the catalog's list, the marked modules, a framed page — and the button stands
  16 px inside its panel, left of a module or an area that floats beside it (`--preview`, now
  declared on the view so that the button can use it), and goes up with the view when the ground
  comes; the wheel over it turns the page under it, a mouse's notch gliding as the browser's own
  wheel does, written frame by frame like the way up (below) and by each frame's share of the way,
  so that what the list makes up in between stays made up and the notch goes its whole way (the
  browser's smooth scrolling ended at half of it, where the list made up for rows measured anew).
  On a phone it is the window, and the button floats 12 px above the bottom bar at its right edge
  (44 px), under the bars, the sheets and the skeleton of a page. It shows once the page is more
  than a screen down — the page is asked where it is when it scrolls and when another page stands
  there, never for a skeleton, so the frame after a click (R21) lays out nothing ahead of time for
  it — and takes it to its top: at once to a screen above the top, then gliding the rest in a
  third of a second, a frame at a time. Not the browser's own smooth scrolling: over the virtual
  list it would build every row it passes and end where the list makes up for a row taller than
  estimated (a script's scroll ends a smooth one); written frame by frame, the glide arrives at
  the top whatever the list did in between. A wheel, a touch, a click or a key stops it; where
  less motion is wanted it is up at once. The list follows as it follows any scroll (`page` leaves
  the address), a module beside it stays, and on a wide screen the ground goes back down (the page
  has left its end). Pressed with the keyboard, the focus goes to the start of the page
  (`#content`, where „Zum Inhalt springen" leads), not to where the button was; Tab from the end
  of a page meets it before the ground. It needs JavaScript (R15); the classic site before the
  takeover has it as well.
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
  eingeklappt"); the line counts the week shown, and the list stays open under a module opened
  from it, which comes up as a sheet over the plan („A module on a phone"). With nothing planned a phone shows no empty week, only „Noch keine
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
  older than the server's page, so the app renders fresh with the same components. Nothing moves
  at the takeover (owner, 2026-10-15: „Das Layout darf sich auf gar keinen Fall verschieben nach
  dem Laden"): the picture is a copy of the body itself, a second `<body>` after it that the rules
  of `body` lay out as they laid out the page (copied into a `<div>` with four of the body's
  properties, its children stood in a row on a phone, the ground a narrow strip at the right), where
  the body lies, with its scroll positions, its running animations and the sizes of what is drawn
  only near the screen (`content-visibility: auto`); the window's scroll is not anchored while it
  stands, and the app's page takes the scroll of the server's parts and those sizes before the
  picture goes (`e2e/takeover.mjs`). From then on
  links, filters and the search are client-side navigation; a step keeps what is in front, the
  page before or the skeleton where one stands, in front of the new one until its answers are
  there (`pending::hold`, at most 400 ms). Until the takeover,
  and if anything fails, the site stays a website served from the HTML cache.
  A copy of an older schema than the build reads is never opened (`user_version` in the SQLite
  header against `folia_model::SCHEMA_VERSION`): with the network it is replaced first, offline the
  app does not start (and nothing says so). A server whose own snapshot is older says so in
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
  loaded before it), as does a shared link with `open`, loaded directly. Behind the takeover's
  picture of the server's first page the list renders every row from the first one, so that the
  rows on the screen stand where that page had them, but only while the top of the screen is on
  that page: a link to a module further down is at its row before the picture goes, and from the
  first row that was every row down to it (2026-10-17, `/catalog?open=11787`: 2,986 rows, every
  page of them asked for and all of them built and measured again with each answer; a wide
  screen froze for minutes, a phone for seconds; `e2e/module.mjs`). A filter change starts at the
  top: the list is rendered anew, but its panel is the same element as before and keeps the
  scroll position, which the new list would otherwise take for its own and write into the URL as
  its page (the first list of a visit is left as it is: a page the browser restored). The server renders the page the URL names, with pager links (no JavaScript,
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
  list following the scroll position). What the
  step changes decides what waits for it (`pending::Change`): another page, another column of
  the same page (a view of a program, the program overview or the marked modules filtered or
  ordered otherwise), the catalog's list, the module beside a list, what stands beside a
  program's page.
- **What was clicked is in its new state in the next frame**, because those parts read where
  the app is going (`Pending::to`) and not only where the router is: the rail and the bottom bar,
  the title and the search of the top bar, the catalog's filter panel and the tags above its list
  (so the head of the list has its height before the rows come), the row whose module opens, the
  views of a program, the toggles of the program overview, the sidebar of the marked modules.
  The mark of a tab changes without a fade (only a hover fades in) and without the browser's tap
  highlight on the bottom bar: a fade waits for the thread, which builds the page then, and the
  mark seemed to stay on the tab before (owner, 2026-10-18; Folia 1.24.6).
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
  The skeleton of a page or a column stays until the new page has its answers: `pending::hold`
  puts a copy of it in front of the page the router builds, its sweeps at the times of the
  skeleton's (`getAnimations`), until no answer is on its way (at most 400 ms). Until Folia
  1.24.6 that picture was one of the page before, which came back between the skeleton and the
  new page on every tab of a phone (owner, 2026-10-18: „dann flickert der bildshirm, weil erst
  Skeltons geladen werden, bevor der content kommt"; filmed in
  `folia/design/tabwechsel/konzept.html`).
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

### Back (`folia/crates/shell/src/nav.rs`, 2026-10-18)

Owner, 2026-10-18: „wir müssen unbedingt an der browser history/backgraphen überarbeiten": Back
opened dialogs again, the selection bar of „Mein Studium" stayed after Back, and after a few
semesters turned one Back did not lead to the overview as the arrow at the head did; „am handy ist
das ja eher wie eine App … der back graph beim handy ist halt eher so dass man zurück zum main
screen kommt und nicht die Seiten rückwärts abzuspielen". The concept with its decisions is
`folia/design/zurueck/konzept.html`; what it came to:

- **A phone and a desktop each have their own Back**, and the layout shown decides which (owner,
  the same day: „wenn die mobile version gezeigt wird, sollte auch das mobile verhalten verwendet
  werden"; `nav::phone`, the 900 px of `folia_design::nav::is_phone`). On a desktop Back is the
  browser's history: a page, a module beside a list, a filter are steps of it. On a phone Back and
  the arrow at the head of a page are one key (owner: „wenn oben links ein back pfeil ist das immer
  gleich sein, wie die handy back option"; `nav::arrow`): it closes first what lies over the page,
  then leads one level up, through the tab to Start, and from there out of Betula.
- **Android first** (owner: „auf gar keinen Fall darf Android darunter leiden. Dann geht es halt
  nur auf Android."): what Safari cannot do, an iPhone does without, and nothing on Android is made
  worse for it. Safari has no `CloseWatcher`: on an iPhone the swipe from the edge is a step of the
  history, and what lies over a page goes with the page it leaves; its ×, a tap beside it and the
  arrow close it. Whether that is enough there is open (owner: „Iphone konnte ich noch nicht testen
  aber wenn es nicht passt kümmert ich mich später darum").
- **What lies over a page has no entry in the history** (`nav::layer_while`): the menus (a tab
  held, the profiles, a row's), the selection and the dialogs of „Mein Studium", the filters'
  sheet, and on a phone the semester its row shows. Forward never opens one again, and a dialog
  Back closed does not come back. In the phone's layout each one open has a `CloseWatcher` (Chrome
  on Android from 126), and Back closes the newest; a `<dialog>` shown modally has the browser's
  own. A watcher made without a fresh tap joins the group of the one before, and one Back would
  close both: the selection, which a long press opens, gets its watcher with the finger's release.
  Chrome takes Esc for the same close request; where a watcher has it, the page's own Esc stands
  aside (`nav::watched`). A new address closes what lies over the old one (`nav::Follow`), the
  semester aside.
- **The semester of „Mein Studium"'s row is the entry's state, not its address** (`nav::keep`,
  `history.replaceState` under `betula.semester`): turning the row (a finger, ‹ ›, „Studium
  planen", a column) writes no step, a reload, Back and Forward bring the semester with the entry,
  and Back from a semester brings the overview at once, however many were turned, as the arrow does.
  `?plan=1`, which said it until then, is still read (without JavaScript the box „Studium planen"
  links it) and gives way to `/study` in the same entry once the row is back at the overview.
- **The filters' sheet of a phone throws its draft away** whichever way it closes but its button
  (owner: „Die Filter sollen bei der Mobile Version verworfen werden, wenn man das Fenster schließt
  ist zurück macht"): its ×, a swipe down, a tap beside it, Esc and Back leave the list as it was;
  „Module anzeigen" applies the draft as one step (`enhance.js`, `sheet-apply`). A step beside a
  page — another filter, order or search of a list, another view of a page — takes the place of the
  current entry on a phone, so that Back leads up rather than through every filter; on a desktop it
  is a step of its own after a pause of a second, closer to the one before it takes its place
  (`Pending::in_place`, `PAUSE_MS`; owner: „Auf dem pc schritt pro Pause", „die 1s am rechner
  passt"). The search's first key on a page is a step of its own and the keys after it take its
  place; on a phone the first one too.
- **Closing what stands beside a page goes back** where the page without it is the step before (a
  module's × and Esc beside a list, its sheet on a phone, „Entfernen" of a planned module in the
  Stundenplan: `Pending::back_to`, `nav::close_to`, `cameFrom` in `enhance.js`), so that Back does
  not bring it again and Forward does; what it changes shows in the next frame, as with a link.
  Elsewhere the page without it takes the place of the current entry. A tab whose page as it was
  left is the step before goes back there too (on a wide screen, and on a phone where the tab's
  stack cannot be written in place; `Pending::back_if_before`). The step before is what the
  browser says it is (the Navigation API: Chrome, Edge, Safari from 26.2, Firefox from 147;
  `nav::came_from`), else what the memory of the tabs guesses (`data-back="history"`).
- **A stack for each tab on a phone** (`nav::tab`, `nav::steer`): the history is Start and, right
  above it, the stack of the tab the visitor is in, so that Back leads up through the tab to Start
  and then out (owner: „Erst zur Startseite dann raus"). A change of tabs — the bar, a swipe along
  it, or a link to a page of another tab, such as a module's program (owner: „Tab Wechsel, das wird
  sonst giga irritierend") — writes the history in place: what is missing first, while the tap is
  fresh (Chrome skips an entry written without one when Back is pressed), then the entries in place
  (`history.go`, `replaceState`; `nav::plan`). What the tap changes shows in the next frame
  (`Pending::hold`); the history is written after that frame, as a link's step is (the tap stays
  fresh for seconds), and the router takes the top at the end (`Pending::release`). The tab left
  keeps its stack (in `sessionStorage`, `betula.nav.stacks`) and comes back as it was left; what is
  left over ahead is no way Forward (`nav::on_popstate` steps back out of it). The tab tapped again
  leads to its first page. A module's own page opened out of a tab that shows its modules in place stays in that tab
  (`nav::tab_of_page`), and so does the profile page, a page of a while (`nav::passing`; „Profiles“
  below): a step on top of the tab, which its tab tapped leaves for the page under it, and which a
  tab left from it does not keep in its stack.
- **A link from outside** (a messenger) stays as it came while it is only read, so that Back leads
  back out (owner: „wenn es keine Interaktion mit dem back graphen gab dann in den massanger, aber
  wenn die app verwendet wurde dann über start menu"); its first use (a link, a tab, the arrow)
  lays under it what lies above it, up to Start (`nav::seed`, `nav::chain_of`): under a module's
  page the catalog and Start, under a program „Alle Studiengänge", „Mein Studium" and Start.
- **The router's own list of steps** knows nothing of a step back by several at once or of a change
  of tabs, and leptos_router 0.8 leaves out a push to the address that list ends at: the address
  and the history then stayed behind where the page went. So `Pending` writes each new step itself
  and lets the router take its place (`Pending::commit`).
- **Where it stops**: without the Navigation API, or where an entry below is of another load of a
  page (the site's pages before the app took over, another language; going there loads that page
  anew), a change of tabs cannot write the history in place and each link is a step of its own, as
  before; the app's own ways back go there only as a link, never through the history. A reload is
  no such load: the entries a page wrote stay its own (they share its document, as the HTML
  standard has it; measured in Chromium).

`node folia/e2e/back.mjs` checks it (§4).

### Tabs that stay built (`folia/crates/shell/src/stage.rs`, 2026-10-18)

Owner, 2026-10-18, after a phone's tab change built the whole page anew each time: „Am anfang
darf die App ruhig etwas laden/kurz Skeltons zeigen. Aber wenn die App dann einmal läuft, dann muss
es sich instant und einfach nur gut anfühlen"; and on the prototypes: „Es ist nice zu sehen, dass
die Seiten gleich da sind. Das muss auf jeden Fall rein". The concept with its steps and what was
measured is `folia/design/tabwechsel/konzept.html`; step 0 (the skeleton held instead of the page
before, the tab's mark at once) is Folia 1.24.6, this is step 1 (Folia 1.26.0):

- **A stage for each tab** (`stage::Stages`, in `main`): Start, Module, Studium, Merkliste,
  Stundenplan, and one for the pages of no tab (the profile, the legal pages, a page that does not
  exist; the one visited last stays). Each holds the page of its place and keeps it built while
  Betula is open. A change of tab swaps which stage is in front, in the frame of the tap, without a
  skeleton and without a picture held in front of it, and the page comes back as it was left: the
  same elements, a module beside the list, a filter, scrolled as far. A module's own page opened from
  a tab that shows its modules in place stays on that tab's stage (`Key::of`, as `nav::tab_of_page`).
- **Where a page is** (`Place`): the router shows one place, the stage it is on follows it, and a
  stage behind stays where it was left (the top of its tab's stack, `nav`). The pages read their
  stage's place, `folia_design::i18n::use_location` and `use_params_map` (never leptos_router's
  hooks): the router's while the router shows the stage. A stage builds its page anew when another
  page comes to it, not when the query changes, as the router's routes did. The router keeps one
  route that draws nothing (`/*any`), so that it still ends each step as before (its scroll, its
  entry in the history).
- **A step to where a stage stands already** (`Pending::stands`): the stage comes to the front at
  once (`Pending::to`) and the router follows in the same task (Back and Forward right after their
  listener: replayed from inside it, the listener would be called again while it runs); nothing is
  held in front of it, the window is not taken to the top, and its time is no measure for a
  skeleton. A step anywhere else goes as before: a skeleton where such a step was slow, then the
  picture of the stage in front held over the new page until its answers are in.
- **Behind** is built and not drawn: `content-visibility: hidden` (app.css; focus, find in page and
  the accessibility tree skip it) and cut to nothing (`clip-path`, so that its box, which covers the
  content area on a wide screen, takes no click), no room on a phone, under the one in front in the
  content area on a wide screen. Not `inert` (until Folia 1.28.0), nor `pointer-events` or
  `visibility`: what a stage's elements inherit has the browser work out their style anew as it
  comes or goes, hundreds of them, and a change of tab waited for it. The window's scroll on a phone
  is kept for each stage as it scrolls and given back as a stage comes to the front where it was
  left; one at a new place is where the router puts it, at the top if it never scrolled. On a wide
  screen each page scrolls in its own areas, which keep their place.
- **What a page does to the whole document it does in front alone** (`folia_design::stage::front`):
  its tags for search engines (`seo::Seo`, one description and one canonical address in the
  document), the listeners of the window (Esc of „Mein Studium", the catalog's list following the
  window, the fragment of the Merkliste), the layers of `nav`, the semester the row of „Mein
  Studium" keeps in the entry the browser is at. The catalog's list writes its page into the
  address only while the router is at it and no step is on its way: Back from a list still settling
  at a shared module's row wrote the list's page into Start's entry and came back to the list. The
  page in front alone names the scroll timeline the wood rises on (`--page`, app.css): a name two
  pages give is no timeline at all, and the wood stood at the foot of the window. Its title is kept
  on its stage (`stage::Title`, a stack for each stage as `leptos_meta` keeps one), and the stages
  give the document the title of the stage in front. A page that goes behind is left as a new
  address leaves it: what lies over it closes (`nav::layer_while`; a `<dialog>` open on it, such as
  the map of the start page, as the stages swap), the filters' sheet throws its draft away
  (`enhance.js`, `betula:stage`), the semester of „Mein Studium" stays, without its watcher.
- **Parts of the same name**: several stages have a `#filters`, a `#preview`, a `#page-scroll`.
  The browser app looks for a page's parts on the stage of the page that asks
  (`folia_design::stage::by_id`, `select`, `select_all`: an effect has its page's owner), else on the
  stage in front (a listener has no owner, and only the stage in front takes input), and finds what
  is around the pages outside every stage. `enhance.js` looks on the stage in front (`find`,
  `findAll`, `byId`) and hears `betula:stage` when another one comes to the front; app.css's rules
  that ask for a page (`:has(…)`) ask for the stage in front, `#content > .page-stage.front …`; the
  checks do the same.
- **The server** knows nothing of the stages: it renders the page of its address with its routes, on
  a stage in front (`<div class="page-stage front">`), so that the app's page stands where the
  server's stood at the takeover.
- **Leptos 0.8 on the way**: a component has no reactive owner of its own, so the six stages give
  their context through `Provider` (side by side every page got the last one); a `leptos_meta`
  title or tag cannot be taken away and put back by a closure (it holds no place in the page, so
  `tachys` has nowhere to put it back), hence the stack of titles and the empty view after the tags
  of `Seo`; and the class `stage` was the landing page's showcase's already.

Measured with the dev bundle on the laptop of §3 (Chromium, the real catalog): a tab seen before
is in front in the first frame after the click, 13–16 ms after it on a wide screen and 20–40 ms after
the tap in a phone's window (412 × 915); before, the same change built its page again behind a
skeleton for 100 to 600 ms.

**Every tab built at the start** (step 2, Folia 1.27.2, `stage::prebuild`; owner, 2026-10-18: „Am
anfang darf die App ruhig etwas laden/kurz Skeltons zeigen"): once the app has taken the page over
and its first page's answers are in, Betula builds the other tabs' pages behind it, one after the
other, the cheapest first (Merkliste, Stundenplan, Start, Module, Studium), each in a task of its
own and only while nothing happens: no finger on the screen, nothing for 600 ms (no key, wheel or
scroll), no step on its way, no question of a page on its way; in an idle moment where the browser
tells one (`requestIdleCallback`; Safari does not, then a moment later). A stage is built at the
place its tab leads to (`chrome::tab_target`, on a phone the top of the tab's stack, `nav::tab_top`)
and as if it was left there at its top: its first visit only swaps, as every later one does, and
opens at its top. It counts as standing once its answers are in (`ready`; the stage is `aria-busy`
until then): a tab tapped before that comes as before, with a skeleton, and its page is the one being
built. What a page behind would do over time waits for the front: the row of „Mein Studium" draws
the pages further out once its stage comes to the front (a step of them on a stage behind was what a
tap then waited for). Measured with the dev bundle in a phone's window (412 × 915, a study set up):
every tab built 1.05 s after the takeover, its longest task 56 ms (1.8 s and 270 ms with the CPU four
times slower); each first visit in the first frame after the tap, 22–35 ms after it, at its top. On
canary's build of 1.26.0 with the CPU four times slower a tab's first build is at most a task of
0.19 s („Mein Studium" with a study; the concept measured 1.8 s on 1.22.0), so „Studium" is built as
it is.

**The pages on an axis on a phone** (step 3, Folia 1.28.0, as it is since 1.29.2; `enhance.js` „the
Achse"; owner, 2026-10-18, of the prototype's ways: „Ich muss sagen ich finde achse fühlt sich am
besten an. Quasi so, als währe es nicht da aber man versteht trotzdem, wie das funktioniert",
„Swipen soll explizit nur über die navbar funktionieren", and of the page following the finger: „das
verhalten im Prototypen passt für mich"; the next day, of 1.28.0 on a phone: „Es wäre wirklich
perfekt, wenn es sich so anfühlt als würde es sofort los gehen", of a quick swipe „sieht die
Animation echt ganz schön laggy aus", of one at its usual speed „dann sehe ich die Animation gar
nicht. Da sollte es vielleicht ein max speed geben. Aber trotzdem so, dass man dann noch weiter
machen kann also nichts blockiert", of quick taps „dann flackert die seite … Es muss ein Konzept für
schnelle aufeinander folgende aktionen geben"; folia/design/tabwechsel/konzept.html, „Schnell
hintereinander"): where a phone's view is shown, a change of tab between two pages that stand moves
them on an axis.

- **A tap, Back, a link to another tab**: the page that comes slides in 36 px from the side of its
  tab and gets clear over the page that goes, from the first frame: a spring of 0.06 s that starts
  at its full pace, a fifth of the page in the first frame, half after 45 ms; its way on a spring of
  0.3 s. The page that goes slides 36 px away to the side of its own tab under it (a damped spring
  of 0.13 s, nothing past its place). At rest after about half a second. A far jump is the same
  (Start to Stundenplan comes from the right as Start to Module does), and Back from a tab's root to
  Start comes as a tap on „Start". Where a page stands and how clear it is are springs worked out
  ahead, every 1/120 s, and played as Web Animations, which the compositor runs while the app takes
  its step; each starts a frame in, since the frame before showed where it starts.
- **Never empty**: the pages on their way are opaque (the window's ground, `--bg`) and lie one over
  the other, the page they go to on top (their `z-index`, from enhance.js): what lies under a page
  shows only as far as that page is not clear yet. 1.28.0 faded the page that goes out before the
  one that comes faded in, as Material has it; half there 250 ms after a tap, the screen mostly
  empty for 170 ms between, which read as loading, and quick taps made it flicker.
- **On the way**: a tap, Back or a finger takes the pages where they stand and as fast as they go,
  and the page it goes to goes on top, every page still seen as much as it was (`onTop`: on top it
  is as clear as it was seen, each page under it as clear as what is left over it lets it be seen
  as much again), so nothing jumps however many come. A page under it that was still coming gets
  clear under it, and a page all covered goes behind at once (one layer less to draw). A finger on
  the page, a wheel or a key puts the pages at rest at once.
- **Before the change**: a finger on a tab makes the page in front and the pages it may bring layers
  while it is down (`axisPrepare`; on another tab that tab's page, on the current one the two beside
  it, where a swipe starts), undone 0.7 s after if nothing came of it. A tap on a tab whose page
  stands starts the pages itself (a click listener before the app's), marks its tab at once
  (`aria-current`, as the start does before the app: the mark answers in the frame the pages start
  in) and gives the app the click once the pages' animations have started (`later`: the `ready` of
  the first, 150 ms at most), so that the app's work (the stages swapped, the title, the scroll)
  does not hold up their first frames; a finger, a key or another click before that has the app
  take it at once, in the order they came. The swipe's click comes the same way.
- **A finger along the bar** says where the pages should be (`axisGrab`, `axisDrag`, `axisRelease` in
  the swipe): as far along the way to the tab beside as the bar's lens is. They follow it frame by
  frame on a damped spring of 0.12 s, never faster than four ways a second (`axisFollow`), so a slow
  swipe goes with the finger and a quick one is seen as a motion: the page of the tab beside comes on
  top, clearer as the way goes on, the page in front slides away under it. Let go, they go on from
  where they are and as fast as they went, not faster (the page on top gets clear on a spring of
  0.09 s from its own pace), to the tab the bar goes to (a third of the way, or flicked), and its link
  is clicked behind them. A finger that comes again on their way takes them where they are, and
  until it has turned them what was on its way goes on. Only a page that stands comes along
  (`chrome.rs`, `window.__betulaStands(area)`); where no tab lies that way the page gives a little,
  as the bar does.
- **Layers**: the window scrolls one page only, so for as long as they move the pages are layers of
  their own (`.page-stage.moving`, app.css): the stage fixed and as large as the screen, which is
  what moves (so that what of a page stands fixed, the dots of „Mein Studium" or a sheet put away,
  stands where it stood), its page scrolling inside it from under the bar at the top as far as it
  was scrolled, its own ground at its end where the window's lies at rest. The window keeps room
  for the farthest any page was scrolled, the page in front follows its scroll, and at rest that
  page is in the window again in the same frame. The stages tell enhance.js before they swap
  (`betula:stage-going`: the page that goes becomes a layer while it is still drawn, where nothing
  made it one before) and after (`betula:stage`, with `from`, `to` and `step`, whether both pages
  stand); a stage that goes behind keeps how far it was scrolled and how high it was
  (`data-scroll`, `data-height`) for a finger that brings it back.
- **The cost**: what a layer sets is never inherited and never reaches into the page beyond the
  parts it names (no `pointer-events`, no custom property on the stage, no rule for all of its
  children): any of it had the browser work out the style of the stage's every element, about a
  thousand. Measured with the bundle that ships in a phone's window, a finger's touches sent as a
  phone sends them: the page that comes half there 50 to 80 ms after a tap (76 ms with the CPU four
  times slower; 1.28.0: 250 and 440 ms), no frame with the window showing between the pages, the
  app's work (about 50 ms slowed down four times) after the pages started.
- **Not**: on a wide screen (the page is there in the next frame), where a sheet or a dialog lies
  over one of the pages, after the browser animated a step already (Safari's swipe back from the
  edge, `hasUAVisualTransition`), on a tab whose page does not stand yet (it comes as before); with
  less motion a short dissolve, nothing slides.

`node folia/e2e/stages.mjs` and `node folia/e2e/achse.mjs` check it (§4).

### The search of the catalog (`folia/crates/search`, 2026-09-30; ranked anew 2026-10-19, Folia 1.30.0)

Owner: the search should improve, first with the names of the modules („erstmal eine neue Zeile in
der db wo keine Sonderzeichen etc drin sind“), typos forgiven; a semantic search („Ähnliche
Module“) is worked on apart from this, so only the names are searched, not the texts of a
description. The list stays the list it was („ich mag das UI da und würde das ungerne tauschen“).

Owner, 2026-10-19, on „TI“, which listed 49 modules with „TIN* - trans*, inter*, nicht-binär“ first
and „Zeitdiskrete Systeme und Regelungen“ and „Ingenieurholzbau“ among them: „gemeint damit war
Theoretische Informatik. Und die suchergebnisse sind ja mal so unfassbar bad … Erstell mal ein
konzept, wie wir das hier richtig filtern und sortieren können, so dass die relevantesten
ergebnisse oben sind.“ The concept is `folia/design/suche/konzept.html`, with the owner's answers
(„Ähnliche module sollen unbedingt bleiben“, „Die zufälligen kürzel sind ok“, a boost for one's
program, the lecturers searched with rules of their own, „die gefundenen Module bei allen gleich
… Aber die reihenfolge darf sich zwischen nutzern ändern“, the other language's title weighted
less, capitals as typed more). Its prototype (`engine.mjs`) is the rule book: `folia_search::rank`
follows it rule for rule, and the two order 165 queries the same on the snapshot of 2026-10-19,
every module of it (`ranking.rs`' `dump_for_the_prototype`).

- **What is searched** (`rank::Index`, read once per snapshot from `v_module`, `v_program_module`
  and `v_module_lecturer`, by its `content_digest`): the module number, the abbreviations — the
  module's own and those programs give it („AuP“) —, the German and the English title, and the
  names of the lecturers, those responsible for the module and those who teach its courses. Not
  the descriptions: that is what „Ähnliche Module“ are for. Case, umlauts, ß and accents do not
  decide whether a word is found („okologie“ finds „Ökologie“); every word has to be found, in any
  order („lernen maschinelles“); fillers („für“, „und“, „der“, „Prof.“, „Dr.“ …) do not count; `%`
  or `_` are no patterns but marks between words. A number of one or two digits and a Roman
  numeral up to ten are whole words, the one the same as the other („Analysis 1“ finds „Analysis
  I“).
- **Points** (`rank::points`, the weights in one place; the concept's „Punkte“). Every word of the
  query gets the points of its best find:
  - *the whole module named*: its number 125, an abbreviation 125 (that of a part without its
    number, „BS“ for „BS1“, 120), every word of a title but fillers and numerals 120 („mathe 1“,
    „theo inf“, „TI“);
  - *a word in a title*: a whole word 80, the short form of words that follow each other 80 —
    their initials or starts, a filler's letter taken or left out („TI“, „theoinf“, „AuD“, „LinA“;
    a piece no longer than the one before, a numeral never one, and it ends a group of words) —,
    the start of a word or of a module number (from three digits) 60, a short form that leaves out
    a filler 55, from four letters on inside a word 35;
  - *a lecturer's name*, with rules of its own (owner: „etwas restriktiver, aber typos oder
    ähnliche schreibweisen bitte doch zulassen. Zum beispeil Petra Hofstedt kann ja auf viele arten
    und weisen geschrieben werden“): the surname whole, from three letters, 75; its start from four
    letters 55; from four letters a surname written alike 50 — both made alike (`rank::alike`:
    „ae“ and „ä“ as „a“, „dt“ and „th“ as „t“, „ph“ as „f“, „ck“ as „k“, „tz“ as „z“, „ie“ and
    „y“ as „i“, „ei“ as „ai“, an „h“ after a vowel left out, a doubled letter as one), then one typo
    apart (two from eight letters), the first letter right: „Hoffstedt“, „Hofstätt“ and „Hofstet“
    find Hofstedt's modules, „Doering“ Döring's. A first name or an initial counts only with the surname („Petra Hofstedt“, „P.
    Hofstedt“; „Petra“ alone is no name);
  - *factors*: a find in the title of the other language ×0.75, and there a word of one or two
    letters finds no starts of words (owner: „dass der nicht primary language match beim titel
    etwas schlechter gewertet wird“: „ti“ no longer finds „Time-discrete Systems“), capitals as
    typed ×1.1 where the query has capitals (owner: „wenn man halt case matching beim wortanfang
    hat, dann soll das auch besser geweichtet sein“: for „TI“ „TIN*“ before „Tissue Engineering“,
    for „Ti“ the other way round), every word before the find in its title −3 % (up to five), the
    words of the query side by side in their order ×1.05, a lecturer who teaches the module and is
    not responsible for it ×0.9.

  A module has the points of its weakest word, or those of naming it whole where they are more.
  What the query finds depends on the query alone: there is no limit of points („die gefundenen
  Module bei allen gleich“), and the random short forms stay (owner: „Das ist ein tradeoff, mit
  dem ich leben kann“: „TI“ also finds „Biodiversity of Terrestrial Invertebrates“).
- **The order:** while there is a search, the list is ordered by relevance — the most words found,
  the most points, then offered before phase-out, the fewer words left over in the title, a series
  together and in its order („Elektrotechnik 1“, „2“), the module more programs have (a number's
  start: the numbers in their order), A–Z — also inside a program. The search hands the catalog's
  SQL its hits in that order (`search::Plan::table`: `json_each(?)`, joined as `sr`, ordered by
  `sr.rank`); filters, the count, the pages and what lies outside the filters are SQL as before. A
  column orders the matches instead, „Modul“ by title (not by the study plan while searching);
  typing in the top bar goes back to relevance (`app::TopBar`), and without a search the list has
  its order as before.
- **Mein Studiengang** (owner: „dass die module aus deinem Studiengang ganz generell einen kleinen
  boost sogar bekommen. Vlt score einfach x 1.2 oder so (Da muss man nochmal gucken, welche werte
  da gut sind)“): the points of the modules of the program this browser keeps are raised ×1.35.
  Measured, ×1.2 was too little: for „Masterarbeit“ a student of Artificial Intelligence M.Sc. got
  their own „Master Thesis“ at place 33, behind the „Master-Arbeit“ of 32 other programs. With
  ×1.35 one's own module named in the other language comes before another's named in the language
  shown, and not before one named by its own abbreviation as typed: with Informatik M.Sc. „SE“ is
  „Sicherheitsengineering“ (its abbreviation), then „Softwaretechnik“. The program goes with the
  query as `CatalogQuery::rank_program`, filled by the catalog like the marks (`with_mine`), never
  in an address and never on the server (R9): it changes the order only, so a link to a search
  lists the same modules for everyone (owner: „Die idee ist bloß, dass man seinen freunden ne
  suche teilen kann“). The owner confirmed ×1.35 the same day: „Ja klar 1.35 ist auch voll OK. Die
  1.2 waren nur ein geschätzter Startwert.“
- **The row says why** (`widgets::list::Row`, `CatalogRow::marks`, filled by `catalog_page` from
  `search::marks`): the letters found are marked (`<mark>`, `--hit`) in the title, in the number
  and in a line under the row that says what else the module was found by — the title in the
  other language („engl. Time-discrete Systems and Regulators“) or a lecturer („verantwortlich
  Hofstedt, Petra“, „lehrt …“) — and an abbreviation stands as a tag („Kürzel ThI“). Since Folia
  1.32.1 (owner, 2026-10-19):
  - the line is a row of the row's grid of its own, as wide as the row, the columns staying beside
    the title and the bookmark beside them („Mach mal die line mit der Übersetzung über die ganze
    Länge (oder zumindest 75%) von der Zeile“); on a phone it is as wide as the title, the credits
    centred beside both; in 1.30.0 it wrapped inside the title's column;
  - the title one sees is marked where it has a word of the query even when something else found
    the module, an abbreviation, the number, a lecturer, the other title („selbst wenn das kürzel
    gematcht wird, dann markier mal trotzdem … Also hier könnte ruhig [An]alysis stehen“:
    `rank::Index::shown_finds`; the order does not depend on it);
  - in the abbreviation's tag the letters the query names are marked as in the title („Kürzel
    **An**2“; „ein einheitliches zeichen für die suche“, „vlt mach das mal nur so in den badges,
    dass da der Matching text makiert wird“: `rank::named_parts`), the tag itself gray as the
    others.
- **What the words as typed do not find** (`search::resolve`, run by `pages::catalog_scope`, so
  that the page and the filter panel agree): where no module has all of them, the words no module
  has are corrected to the word of a title they are within one typo of (two from eight letters on,
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
- **Speed** (2026-10-19): the search ranks in Rust, not in SQL. Reading the index costs about
  100 ms once per snapshot (natively, in a release build); then a text new to it takes 3–5 ms to
  score every module (`rank::evaluate`, 9 ms for a long word, 20 ms where a typo is corrected), and
  the order for the program a quarter of a millisecond (`rank::order`). The scores of the last
  text, its order and its resolution are kept (`search::evaluated`, `ranked`, `resolve`): a page
  resolves its text with the list and again with the counts of its filters, and orders it once.
  In the data worker on canary the index took a quarter of a second of a laptop, which the first
  search of a visit waited for (457 ms against 200–230 for the next ones; a phone's second or so).
  So it is made before (Folia 1.32.1, `search::prepare`, `worker_prepare_search`): `boot.js` tells
  the worker a search is coming as a search field takes the focus, and with a mouse once the
  browser is idle, as it loads the semantic search; the worker makes it once per snapshot, when
  no question has come for 300 ms, so the questions of a page being built go first.
  Before, the search was SQL over `v_module_folded` that scored only the modules whose names
  contained a form of every word, 2 to 8 ms in the sql.js of the app on a laptop. It all runs in
  the data worker while typing („Typing“ below).
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
  - **The vocabulary of the titles is made once per snapshot** (since 2026-10-19 with the
    search's index, `rank::index`, by its `content_digest`), not for every word no title has.

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
    (`html.js:not(.mine)`: „Studiengang wählen", with the picker's chevron and its room), so they
    stay when the app takes over. The
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
  tag (`dl.tree-figures`). Flat, as the icon is: the first trunk was shaded at its edges and the
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
  the view, in the body's second row; none under the row of pages of „Mein Studium", 2026-10-19), full width, the bottom bar floating over its lower part, and
  the crown carries the frosted background of the bar at the top. A page shorter than the window
  does not leave the ground floating halfway up the screen (owner, 2026-09-26: „mindestens unten
  bündig"), and the ground does not show before it is scrolled to (owner, 2026-10-05: „der footer
  ist gerade häufig zu sehen mach das so, dass der mindestens genau außerhalb vom bild ist, so dass
  man rein scrollen muss"): the view is at least as high as the window at its largest (`100lvh`, the
  browser's bars hidden), so the ground lies just past the window's lower edge, the end of the page
  that is scrolled to.
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
  - With the ground up and the area standing still (`data-rest`, `enhance.js`), the panels it cut
    off end where their content ends, with round corners as the end of the list has (owner,
    2026-10-04: „die Ecken abrunden, wenn der Footer oben ist"): the filter panel or the sidebar,
    a pinned short list, the module beside the page. The ground draws the corners (`.ground-cap`,
    `folia_shell::ground`), so they stand on its edge in every frame; the first scroll of the area
    takes them away, and while it moves the panels are cut off straight as before. A panel whose
    own box ended above the ground would show a gap there in the frame before it grew back, as the
    natively scrolled ground goes down. The module's corners come once it has slid in; a panel
    dragged by itself alone (`data-resize-mode="panel"`) has none until it is let go.
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
the logo, at the end of the title column, right of the search — and fell apart into clumps with a
thin edge between them and a gap on wide screens). Two masks per season (`folia/assets/birch/`): a
tile of 1200 px that repeats to the right edge, dense at the top and hanging deeper and shallower in
long waves (never below 46 px, so nothing hangs out under the search), and at the left end a head of
420 px: the logo, and over the title a clearing where only the crown's edge hangs in, the twigs at
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
  the Stundenplan in the navigation, the language's way on a view, a program's other examination
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
    `Europe/Berlin`; a series of single dates one in its rhythm, `P1W` to `P4W`, with the days it
    leaves out as `exceptDate`) and the exam dates as `EducationEvent`s (`subEvent`) with the day's offset
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
  weit und links etwas daneben" was the site's head there), and the icon stands in front of the
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
  survives the takeover), all of it the icon: `/favicon.ico` (32 and 48 px) and the SVG icon (turned
  over where the browser is dark); `/apple-touch-icon.png` (180 px, full bleed: iOS rounds it and
  uses it for the home screen and for previews in Messages) and `/manifest.webmanifest` (the name
  „Betula · Modulkatalog", „Betula · Module catalogue" in English, which Android writes under the
  icon of its splash screen (owner, 2026-10-15: „Das sieht classy aus"); colours; icons 192/512,
  a maskable one of 334 px for the launcher, a round one of 512 px for the splash screen, and a
  monochrome one); `/manifest-dark.webmanifest`, the same app in the dark page's colours and with the splash
  screen's icon turned over; and `theme-color` (the page background). The head script and the
  theme switch turn both dark: the page names the dark manifest instead of the light one and the
  dark `theme-color`, and so does a page open when the system turns dark, unless a theme of
  Betula's own was chosen. The pictures are made from `folia/design/logo/app-icon.mjs` by
  `node folia/design/logo/render-icons.mjs`. The manifest makes the site installable; the service
  worker makes it start without a network.
  - **Google Search** (2026-10-01) shows one picture per host beside the results, taken from the
    links of the start page (`icon`, `apple-touch-icon`). It reads no SVG (BMP, GIF, ICO, PNG,
    JPEG, PPM, TIFF; square, larger than 48 px recommended) and does not document how it chooses
    among several; as far as can be seen, the largest. So the start page links the icon of the
    app once more, as an `icon` of 192 px (`app::ICON_192`, square and a multiple of 48): larger
    than the ICO (48 px at most) and than `apple-touch-icon.png` (180), it is the largest
    of either kind of link (`folia/crates/server/src/tests.rs` checks that). The tab takes the SVG: Chromium
    does not even fetch the PNG (tried at 1×, 2× and 3×), and Firefox takes the SVG whatever else
    is linked (`selectIcons` in its `FaviconLoader.sys.mjs`).
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
    the icon of an installed app did not take part. The splash screen of the installed app
    depends on what installed it (screen recordings of both on a Pixel, 2026-10-18):
    - **Chrome** installs a WebAPK, and from Android 12 on Android draws its splash screen: the
      launcher's icon (ink and paper, whatever the theme) cut round, 240 dp, in the middle of the
      whole screen, on the manifest's background, and no name.
    - **Brave** (and what else installs a shortcut) draws Chromium's (`SplashLayout`,
      `WebappSplashController`) after its own with Brave's lion: the manifest's background, the
      picture of the splash screen as it is (a shortcut forgets that it is maskable) at 128 dp,
      and the manifest's name under it, 24 sp, its bottom 48 dp above the bottom; the icon's middle
      16 dp above the middle (24 on a tablet). Up to Chromium 154 that is the whole screen's;
      from 155 on (`setupSplashInsets`) the screen's less the bars, the gesture area at the bottom
      counted (32 dp with gestures, where the bar is 24). Brave fetches the picture once, when the
      app is added to the home screen.

    The picture of the splash screen is the maskable icon cut round (`ICON_SPLASH`,
    `icon-splash-512.png`; owner, 2026-10-18: „wenn das Icon schon am Anfang rund ist … im
    Zweifelsfall mach halt das Icon Bild rund"): named maskable, so that Chromium takes it before
    the plain ones, and drawn as it is by a shortcut, round from the start; a mask of Android's
    shows only its middle, which is the full-bleed icon's. Chrome takes the icon of the size it
    asks for or the next larger one (`ManifestIconSelector`): 83 dp for the launcher, which 334 px
    serve up to a density of 4.02, and 128 dp for the splash screen, which asks for more than 334 px
    from a density of 2.61 on (most phones of today) and takes the 512. So the dark manifest names
    a turned-over 512 (`ICON_SPLASH_DARK`) and the same launcher's icon: only the splash screen
    turns over in the dark, and the home screen's icon never changes with the theme (Chrome asks
    the visitor once before it takes a new icon or name: the change to „Tinte" and the new name
    are asked about once).
  - **Light and dark on Android** (2026-10-15; owner: „wenn Betula als pwa installiert startet, dann
    wird bei dem loading screen nicht der Darkmodus berücksichtigt, weshalb dann die gesture bar
    unten die falsche farbe hat"). Chrome (158 as of 2026-10; `WebappIntentDataProvider`,
    `WebappActivity`) reads one `theme_color` and one `background_color` from a manifest and no
    dark ones: `color_scheme_dark` of the draft is not parsed, and its own experiment's dark fields
    are filled by nothing. The installed app is drawn in them as long as it is open, whatever the
    page says: the splash screen in the background, and the strip under the gesture bar in the
    theme colour (`WebAppNavigationBarThemeColor`; without a theme colour in the window's
    background, the same colour again). A standalone app does not draw edge to edge (only the
    media viewer of a Custom Tab does), so `viewport-fit=cover` and the page's `theme-color` do not
    reach that strip; the page's `theme-color` colours the status bar. With one manifest the strip
    of a phone in the dark was the light page. So there are two, the same app (`id`, the start, the
    icons) in the colours of either theme, and a page names the one of its theme (`MANIFEST_DARK`):
    a phone in the dark installs the app dark, splash screen and strip. Chrome looks at the
    manifest a page of the installed app names once a day at most (`WebappDataStorage`, a month
    when the WebAPK server asks for fewer updates) and has the app minted again when its colours
    differ (`BACKGROUND_COLOR_DIFFERS`, `THEME_COLOR_DIFFERS`; the same `id`, so the address of the
    manifest may change): a phone that turned dark, or a theme chosen in Betula, reaches the
    installed app on a later start. Installing the app again makes it at once.
  - **iOS** draws no splash screen for a web app: it shows the picture the page names for exactly
    its screen, orientation and colour scheme (`apple-touch-startup-image`), and without one a
    blank screen until the page is drawn. The head script names the pictures of the screen it runs
    on (only where `navigator.standalone` exists, that is on iOS: two on a phone, upright, light
    and dark; four on a tablet, turned too), and iOS takes them when the app is added to the home
    screen. Sixty `<link>`s for all screens in every page would have cost every visitor almost a
    kilobyte for what only a home screen of iOS reads. The server draws them
    (`/assets/launch/<width>x<height>[-dark].png`, `folia/crates/server/src/launch.rs`, resvg like the cards)
    for the screens of `app::launch` (12 iPhones, 9 iPads): the icon in the middle of the page's
    background, 128 px as on Android's splash screen, on a soft shadow in the light and turned over
    in the dark, and the wordmark at the bottom as the start page sets it (`.wordmark.small`,
    22 px; owner, 2026-10-18: „unten gerne den BeTUla Schriftzug mit dem Styling wie im Start
    Screen"); on its first
    request, one at a time, and keeps them (20–40 kB each, 2 MB for all). The gate lets them
    through like the icons. A screen that is not listed gets no picture and starts blank as before:
    a new iPhone is one line in `folia/crates/shell/src/launch.rs`. Look at all of them with
    `FOLIA_LAUNCH_OUT=<dir> cargo test -p folia-server launch_screens_for_review`.
  - **The start of the installed app** (2026-10-15; owner: „ich möchte einen viel cooleren
    ladebildschirm haben", then concept A of `folia/design/launch/konzepte.html`, with „Tinte" and
    the new name from `icon.html`). Where the installed app starts on a phone or a tablet
    (`display-mode: standalone` or `navigator.standalone`, and a coarse pointer), the head script
    lays the picture of the system's splash screen over the page before its first paint (`.launch`,
    beside the body, where the app's takeover does not reach it), with the wordmark at the bottom.
    The icon stands where the system left its own, so that nothing jumps (owner, 2026-10-18:
    „springt der loading screen immer noch"; until then it stood 16 dp above the page's middle,
    where Chromium had it before it drew edge to edge: 14 dp under Brave's and smaller and lighter
    than Chrome's): in Chrome Android's, 240 dp in the middle of the screen with the launcher's
    colours (`.sys`); in Brave Chromium's, 128 dp with the leaf of the round picture (.873),
    placed by the Chromium the user agent names; on iOS the launch screen as the server draws it.
    The screen's geometry places it in the page (`--mid`, `--size`, `--leaf`, `--leaf-at`, `--name`),
    less the status bar above the page: on Android what enhance.js measured at the first touch of
    an earlier start (a finger's place on the screen less its place in the page,
    `betula.launch.top`, kept only where the bars around the page leave room for it), else the bars
    less a gesture bar of 24 dp; on iOS the whole difference. The first picture of the page is the
    splash screen again. enhance.js holds it while the system fades its own splash from above it
    (350 ms; 150 on iOS, whose switch from the launch screen is at once), then the icon flies to the
    page's own, the first `.logo` on the screen, in 640 ms along the way of concept A, turns from
    its circle into its square and its leaf into the logo's size, and takes the logo's colours on
    the last stretch, where it is small; the wordmark flies the same way to the page's own, the
    first on the screen (on a phone the start page's, beside the logo), while the background clears.
    The page's own are not there while they fly (owner, 2026-10-18: „das Icon soll erst existieren
    wenn es hin geflogen ist"; `html.launch-logo`, `html.launch-mark`): they come in the frame the
    picture goes, under where the icon, which took the logo's ring and shadow on the last stretch,
    and the wordmark landed. Without a logo on the screen (a phone's
    page other than the start page) or with reduced motion it only fades, and without a wordmark on
    the screen the launch's fades as the icon sets off; a tap takes it away; once the icon flies,
    taps reach the page. Once a session (`betula.launched` in sessionStorage): a link or a reload
    in the app is no start. Not on the login page of closed testing (it names no manifest and runs
    no enhance.js), and without enhance.js the picture fades by itself after 4 s (`launch-gone`,
    which reduced motion leaves alone). Brave's name under the icon fades with its splash into the
    wordmark. `folia/e2e/launch.mjs` checks where it stands for Chrome, Brave before and from
    Chromium 155 and iOS, that it lands on the logo, when it is not there, and the measured status
    bar.
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
  older schema than the build reads: then the app does not start and the page stays the one the
  worker kept. Nothing of the data is said in the top bar any more: not „Offline bereit" once the
  app runs (owner, 2026-09-21: „wenn es einfach funktioniert, dann passt das"), not the loading of
  the data on a first visit, and not why the app does not start offline (owner, 2026-10-15: „Ach
  mach das einfach weg. Das Laden geht schnell genug, so dass man es nicht sehen muss", „Ja, den
  Offline-Hinweis auch weg"; on a phone the words took the search's room beside them, „D…", and
  gave it back at the takeover). `folia/e2e/pwa.mjs` cuts the network and loads pages afresh; `folia/e2e/schema.mjs` plants a copy of an older schema and starts with and without a network, and one that cannot be read.

Not done: submitting the sitemap to the search consoles (needs the owner's accounts), English
pages.

### Profiles (`folia/crates/stores/src/profiles.rs`, `folia/crates/profile`; 2026-10-14)

Owner, 2026-10-14: „eine Profile funktion, so dass man mehrere Profile in Betula anlegen kann und
dann eine seite hat, wo man seine ganzen informationen/einstellungen verwalten kann". A profile is
a set of what a visitor keeps, side by side with others in one browser: a second program beside
the first, a change of program tried out apart from the real plan, a browser two people share.
There is no account (R20): the profiles live in the browser like the Merkliste.

- **What a profile holds** (`profiles::KEYS`): the Merkliste, the Stundenplan with „Mein Studium“
  (one text, `PlanDoc`), the saved plans and „Mein Studiengang“. **What every profile shares** is
  the browser's (R13): the language, light or dark, the widths of the panels, what the finder
  compares, the tabs' memory of the session. The Stundenplan's program of the session is a
  profile's own (`profiles::SESSION_KEYS`, in `sessionStorage` under the profile's key), no part
  of its file, and goes when the profile is emptied or deleted.
- **Keys.** The first profile keeps the plain keys (`betula.bookmarks.v1` …), the ones a browser
  had before there were profiles: nothing moves when profiles come, and a build without them (a
  rollback, an old tab of the installed app) shows the first profile as it is. Another profile
  keeps its number after `betula.` (`betula.p2.bookmarks.v1`, `profiles::key_of`). The list is
  `betula.profiles.v1`: `active<TAB>2`, then `profile<TAB><n><TAB><made, s since 1970><TAB><name>`
  per profile; the first alone, unnamed, writes none. It is read like anything from outside
  (numbers 1–999, a profile once, at most 8, a name one line of at most 40 characters, the active
  one among them), never as no profile, and what stands under the plain keys always has its
  profile (the first deleted and written to by a build without profiles since: it is listed
  again).
- **Switching** is the browser's, not a tab's: every tab follows, as marks do. The stores follow
  the active profile through `profiles::Kept`: they read the new profile's texts when it changes
  (here, in another tab, or when the profile page wrote into them: emptied, put back) and write
  only under the key of the profile whose text they hold, so a switch never writes one profile's
  text under another's. What a page keeps for „Rückgängig“ (the Stundenplan's `PlanCtx::undo`, the
  Merkliste's emptied list) is dropped with another profile, and the Merkliste lists the new
  profile's marks as if it had been opened with them. The script in `<head>` reads „Mein
  Studiengang“ of the active profile for `html.mine`.
- **The way to it** (`chrome::ProfileButton`): the profile's badge (its initial in one of the
  eight soft hues `--t-*`; a person while there is one profile without a name) at the end of the
  rail above the switch between light and dark, and on a phone, which has no rail, a box at the
  right of the search at the head of every page (owner, 2026-10-14: „für die mobile Version noch
  ein Konzept wie man an sein Profil rankommt“), as high and as wide as the search is high, in
  its style — as the way back of a page is a box at the search's left (`TopBack`), to which it
  gives way while that is there (owner, 2026-10-15). Beside
  it the search keeps its words on a phone 390 px wide (a little less room around its icon and
  words there); the start page's brand line holds the brand alone. The page is no area's: no tab
  is current on it, the badge is, and the top bar says „Profil“. The server writes the person, the
  same for everybody (R9), and without JavaScript the way leads to the page's stand-in.
- **A page of a while** (owner, 2026-10-19: „dass die Profile page, als temporäre seite getreated
  werden soll wo man dann oben den zurück pfeil hat und genau wieder da landet, wo man davor war";
  `nav::passing`, `chrome::Up::Before`): on a phone the page has the arrow at the head (its address
  puts it there, `back_at_head`, so the server writes it as well), and it leads back to the page
  the visitor came from as it was left — the same entry of the history, its stage still built,
  scrolled as far; Back does the same. It is no tab's: a step on top of the tab it was opened from,
  not a change of tabs, so that the tab in the bar leads back to the page under it rather than to
  its first page, and a tab left from the profile keeps its stack without it. Come to without a page
  before it (a reload, a link from outside), the arrow leads to Start.
- **The menu of the profiles** (`chrome::ProfileMenu`, `ProfileSwitcher`; owner, 2026-10-14:
  „wenn man beim pc über das profil in der nav bar links hovert, kann man die profile direkt
  auswählen. Beim Handy long hold.“): the profiles with their „Mein Studiengang“
  (`Profiles::program_lines`, as on the page), the active one checked; a click makes one the
  active profile after the next frame (R21), and the menu goes at once. Below them „Neues Profil“
  (owner, 2026-10-19: „ein kleines Menü, wo man die profile als option hat, neues Profil anlegen und
  dann halt profil Einstellungen, und vlt noch change lang"): tapped, a field for the name takes its
  place (none: „Profil 3“), and „Anlegen“ or Enter makes the new profile, empty, the active one as
  the page's „Neues Profil“ does, the menu gone at once; a menu a mouse opened stays while the name
  is typed, and Esc leaves the field before it closes the menu. With eight profiles it says that
  there can be no other, as the page does. Then „Profil & Einstellungen“ (the page) and the other
  language (`languages::LanguageWays`: this page in it, a page load). A mouse resting on the rail's
  way for 160 ms opens it beside the way, growing up from its foot, without moving the focus; it
  goes 280 ms after the mouse has left the way and the menu (the gap between them is crossed in that
  time), and a mouse passing over the way opens nothing. A right click or the menu key on the way opens it as a tab's menu is opened
  (`TabMenu`): over a dimmed page, the focus on the active profile, until a choice, a click beside
  it or Esc (the focus back on the way). On a phone a tap on the box opens it below the box, over a
  dimmed page, and closes it again (owner, 2026-10-19: „die häufigste Aktion wird sein, dass man
  das nur das Profil wechseln will"; `aria-haspopup`, `aria-expanded`): the page is one of its ways,
  and nothing changes in the history until a way is taken. A finger held on the box opens it as
  well (enhance.js, `data-hold="profiles"` → `betula:profile-menu`, as a tab is held), and the
  finger's release goes nowhere. Without JavaScript the box is the link to the page's stand-in.
- **The page** (R17): the sidebar holds the profiles (a click marks one and switches after the
  next frame, R21; under each its „Mein Studiengang“), „Neues Profil“ (a name, none: „Profil 3“;
  the new profile is empty and active), and „Sichern & übertragen“. The page is the active
  profile: its head (badge, name, when it was made; „Umbenennen“, „Löschen“ — the only profile is
  emptied, not deleted), „Mein Studiengang“ (program with the plan of its Studienrichtung,
  Studienbeginn and Fachsemester, Urlaubssemester, Standort; changed in „Mein Studium“), what it
  keeps (Merkliste, Stundenplan, „Mein Studium“, saved plans, each row the way there; „Profil
  leeren“), and the settings (light, dark or the system's, set by `enhance.js` as the rail's
  switch is, `data-action="theme-set"`, which says so with `betula:theme`; the language, a page
  load). The language lives in the profile since 2026-10-14 (owner: „allgemein kann man die
  Spracheinstellungen komplett in das Profil moven“): here and in the menu of the profiles; the
  rail and the start page's brand line have no switch any more, and without JavaScript the page's
  stand-in offers the other language (docs/folia/i18n.md). A profile deleted or emptied comes back
  with „Rückgängig“ at the head until the page is left. On a phone the sidebar comes first
  (`sidebar_first`), and this is where a phone chooses light or dark at all.
- **A profile as a file** („Als Datei sichern“, `profiles::Backup`): JSON,
  `{"betula": "profile", "version": 1, "name": …, "stores": {"betula.bookmarks.v1": …}}`, the
  stores named by the first profile's keys, made in the browser (a Blob) and named
  `betula-<name>-<day>.json`. „Aus Datei laden“ reads it in the browser (at most 4 MB), checks
  every store as the store reads its text, says what it holds and asks; the profile it makes is a
  new one, its name told apart from a profile of the same name („Zweitstudium (2)“). A file of a
  later layout is refused, not half read. Neither reaches the server; the privacy notice says so
  („Speicher im Browser“).

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
  focus) and Enter opens the selected row; Ctrl+K or `/` jumps to the search; in the app `M` marks
  and `P` plans the module one is at, ← and → move it while the note of what it planned stands, and
  Esc puts the note away („„Einplanen" plans at once, and a note says where"). Every view is a real
  history entry, so the browser's back always works too.
  No action is a key's alone (owner, 2026-10-20: „ein No-Go, dass Aktionen nur über Shortcuts
  erreichbar sind … das, was Betula selbst macht, soll als Komponente da sein, welche auch per
  Button bedient werden kann"): a key is a quicker way to a button or an entry of a menu that is
  there. What every program does (Ctrl+C and Ctrl+V, Esc, Enter, the arrows, F2, Entf) needs no
  hint; Betula's own keys are written next to their button or entry. The head of a list names
  ↑ ↓, Enter and M; S and N its tooltip alone (owner, the same day: „Sammlung sieht hier zu viel
  aus").
  The tabs of the navigation are a digit away (owner, 2026-10-16: „wenn man ctrl/alt drückt sollen
  dann die shortcuts bei den nav items angezeigt werden. Wichtig ist, dass … so übliche shortcuts,
  wie ctrl+c/s/p usw. weiter funktionieren"): 1 the start page, 2 the modules, 3 „Studium",
  4 the Merkliste, 5 the Stundenplan (`Area::key`), each a click on its tab — a second time a tab
  leads its area back to where it starts, as a click does. The keys are the ones a page has to
  itself on each system: **Alt** and the digit on Windows, **Ctrl and Alt** and the digit elsewhere
  (⌃⌥ on a Mac, the keys of a page's own access keys there). Ctrl and a digit chooses the browser's
  tab everywhere, Alt and a digit Firefox's on Linux, ⌥ and a digit types a character on a Mac; the
  digits of the row above the letters only, so Alt and the number pad still type a character by
  its code on Windows; Ctrl, Alt or ⌘ with any other key (Ctrl+C, S, P and the rest) stays the
  browser's and the system's, and so does AltGr (on Windows Ctrl and Alt, which is why Alt is
  alone there). Pressed, the keys show at once what leads to each tab, in full (owner, the same
  day: „zeig ruhig die shortcuts gleich an, dann findet vlt jemand die auch. und zeig mal die volle
  kombo. Also "alt + 1" oder so"): „Alt + 1" on Windows, „Strg + Alt + 1" („Ctrl + Alt + 1" in
  English) on Linux, „⌃ + ⌥ + 1" on a Mac — the tab's digit (`NavKey`) after the words of the
  system and the language, which enhance.js gives `--nav-keys`; inverted, as a tooltip, beside the
  rail's tab and above a phone's bar (a tablet with a keyboard), there while `:root[data-nav-keys]`.
  Whoever presses Alt for anything else sees them as well, and so finds them. Another key or the
  mouse with them, and they go until the keys are let go. An Alt held for a quarter of a second or
  longer was held for them: let go, it opens no menu of the browser; tapped, it is the browser's
  as ever. Nothing while a modal dialog is open (`enhance.js`, „the navigation's keys").
- **R13. Personal view settings never go into the URL**: theme and the widths of the filter panel
  and the preview live in `localStorage` and are applied before the first paint by the script in
  `<head>`. So does the language (`betula.language`, `docs/folia/i18n.md`): the first script of the head
  opens the page in it, which is a page load, because a language is an address. They are the
  browser's, the same in every profile (the profile page sets light, dark or the system's and the
  language, the menu of the profiles the language; the rail's switch light or dark).
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
  One exception: a kind no source states is left out, not „Art nicht angegeben" — students
  trying Betula asked what that meant, while a module without a kind reads as not assigned
  without being told (owner, 2026-10-18); `KindBadge` draws nothing, the program's tables leave
  the cell empty as they do for LP and Turnus. Never a default kind either. A module in the
  program's minor („Komplex Nebenfach", „Nebenfach", „Künstlerisches Nebenfach", „Minor
  Subject" in its area's path; `folia_design::ui::in_minor`) that no source makes Pflicht reads
  „Nebenfach" instead of „Wahlpflicht", in the program's list, on the module page and in the
  table of the areas (owner, 2026-10-18: the Anwendungsfach of Informatik „als Anwendung oder
  minor"); its kind stays elective for the filters and the planner, and the plan's table keeps
  the slot's own name („Anwendungsfach").
  A code without a label is shown as it is (`labels::Code`), and the label test flags it. What
  Radix infers is said where it is shown: a kind whose `kind_basis` is `inferred` reads
  „Wahlpflicht · abgeleitet", its title saying from what (`folia_design::ui::KindBadge`, in the
  catalog's list of a program and on the module page; schema 14, docs/radix/schema-v2.md, „Kinds
  from the plan's slots").
- **R20. What a visitor keeps stays with the visitor** (owner decision 2026-09-20: no data of a
  visitor on the server). Marked modules, and whatever follows them (passed modules, the own
  program), live in `localStorage` under a versioned key (`betula.<what>.v<n>`), behind one
  store per kind that is provided through context and is empty on the server
  (`bookmarks::Bookmarks`), each profile its own (2026-10-14, „Profiles"): the first under the plain
  key, another under its number (`betula.p2.bookmarks.v1`), the profiles and the active one under
  `betula.profiles.v1`. They never become part of a URL (URLs are requested from the server
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

(Without a crawl of your own: the catalog of canary, from `https://canary.betula.app/api/db`, §4.)

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
target and `wasm-bindgen` 0.2.128, which Trunk keeps in its cache; in the cloud the prebuild
installs both, „Build times"); without it the site simply stays server-rendered. `--dev` builds it
with the `wasm-dev` profile and the flags of `folia/.cargo/config.toml` (`--cfg
erase_components`, see „Build times"), and keeps the names of its
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

Owner, 2026-10-20: „ich arbeite jetzt fast ausschließlich in der cloud, wo sich keine
inkrementellen compile files geteilt werden", and: „wie man die entwicklungs compile zeit für
cloud runner reduzieren kann". Measured that day in a cloud session (Anthropic's VM: four vCPUs,
16 GB, Ubuntu 24.04, rustc 1.97, which links with `rust-lld`), `cargo build` in `folia/`, every
edit a real one (a string grows by a character):

| | without `erase_components` | with it |
|---|---|---|
| cold build, empty `target/` | 5 min 8 s | 3 min 8 s |
| edit a page of the catalog (`folia-catalog`) | 18 s | 8 s |
| edit a page of the Stundenplan (`folia-planner`) | 21 s | 12 s |
| edit `folia/crates/design/src/ui.rs` | 29 s | 18 s |
| edit `folia-server` | 26 s | 5 s |
| the version bump that ends a branch (the 13 crates at the app's version) | 3 min 51 s | 1 min 18 s |
| nothing to do | 0.6 s | 0.6 s |
| `target/` after the build | 7.0 GB | 3.4 GB |
| the debug server | 1.4 GB | 226 MB |

With the flag, the browser app's first dev build (`build-client.sh --dev`: its dependencies for
`wasm32-unknown-unknown`) takes 2 min 55 s more, the first `cargo test --no-run` 1 min 34 s, and
the three leave 12 GB in `target/`. Most of a cold build is the workspace's own crates, the
Leptos views and the server that renders them: by `cargo build --timings` the longest path of a
cold build ends in `folia-widgets`, `folia-planner` and `folia-server`, 22, 39 and 14 s with the
flag, 69, 107 and 76 s without it.

The version bump costs what it costs because a package's version is part of the identity of its
units (`-C metadata`, the names of its artefacts and of its incremental session): a crate whose
version changes is compiled from nothing, not incrementally, and the 13 crates at the app's
version change together. (Measured as well: the server's version alone, and the shell's
`VERSION` with it, 13 s. Which crates carry the version is the owner's rule, CLAUDE.md.)

**`--cfg erase_components`.** Leptos gives every view a type of its own: an element is a type with
its attributes and children, a component's `impl IntoView` the whole tree below it. rustc infers
these types in one frontend thread per crate, and instantiates the code that renders them in
`folia-server` and again in `folia-client`. With the flag, Leptos erases the type of every
component and every list of children (`AnyView`; the documentation is at `src/lib.rs:264` of
leptos 0.8.20, and leptos asks its macros for the same through
`[target.'cfg(erase_components)'.dependencies]`), and little is left to infer or to instantiate.

It is a flag for rustc and not a feature, so it stands in `folia/.cargo/config.toml`, which every
cargo command in `folia/` reads, for the host and for `wasm32-unknown-unknown` alike. Its table is
`[target.'cfg(all())']` and not `build.rustflags`: cargo joins the flags of every target table
that matches, so a machine's own (a linker) are added to it, where `build.rustflags` would give
way to them. It is for working on the code: the bundle that ships (`build-client.sh` without
`--dev`) is built without the flags of that file, and Nix never sees it (`flake.nix` builds from
`Cargo.toml`, `Cargo.lock`, `crates/` and `assets/`). What the two builds do differently, checked
on 6 500 pages (every address of the sitemap and 1 100 lists and filters found on them):

- The server's HTML carries more of Leptos's markers (`<!>`, `<!--<() />-->`): 2–5 % of a page,
  about 1 % gzipped. Without its comments every page is the same, byte for byte.
- Every reactive closure costs an `Arc<Mutex<_>>` and a call through a vtable. A page not yet
  in the cache renders in 8.8 ms instead of 8.0 ms (median of 250, debug builds).
- What compiles with the flag need not compile without it: the types it erases are deeper there,
  and so closer to `recursion_limit`. Nix builds without it, and `deploy/ship.sh` builds with Nix
  before it ships anything, so such a failure stops a release instead of slipping into one. To
  see it earlier: `CARGO_ENCODED_RUSTFLAGS= cargo build` (every dependency once more, in the
  same `target/`).

A change of the flags is a change of every unit's fingerprint: after one, everything is built
again.

**In the cloud.** Every session is a VM of its own, made from the cloud environment's snapshot:
the repository as it was cloned when the snapshot was taken, brought to the session's branch by
`git fetch` and `git checkout` (which touch only the files that differ, so cargo takes what is
unchanged for built), and whatever the environment's setup script wrote. Nothing one session
builds reaches another. Two things start the builds early:

- **`.claude/hooks/prebuild.sh`**, the SessionStart hook of `.claude/settings.json`. In a cloud
  session (`CLAUDE_CODE_REMOTE=true`; anywhere else it does nothing) it returns at once and
  leaves the prebuild running in the background, at the lowest priority: the
  `wasm32-unknown-unknown` target and `wasm-bindgen` at the version of `folia/Cargo.lock` (the
  release's binary, checked against the sha256 in the script: a second instead of two minutes of
  `cargo install`), Go's toolchain and modules, then in `folia/` `cargo build` and the browser
  app's dev build, then Radix and Cortex with their tests compiled. From nothing that is about six
  minutes, Go aside, while the session reads; its log is `/tmp/prebuild.log`. A cargo command of
  the session that builds into the directory of a running step (the same profile and target)
  waits for its lock („Blocking waiting for file lock on build directory") and then finds that
  work done; the browser app's build holds up no build of the server. `cargo test --no-run` is
  not one of the steps: it builds into `target/debug` as `cargo build` does, and a `cargo build`
  of the session waited 84 s for it with nothing left to do, in the very minutes in which the
  first edits are built. The first `cargo test` of a session takes its 1 min 34 s instead.
- **The setup script of the cloud environment** (claude.ai/code, the environment's settings,
  „Setup script"). It runs once, and what it leaves is kept in the environment's snapshot for
  about seven days, if it finishes within about five minutes:

  ```bash
  #!/bin/bash
  cd /home/user/betula.app 2>/dev/null && bash .claude/hooks/prebuild.sh setup
  exit 0
  ```

  `setup` is the same prebuild in the foreground, stopped after 240 s (`PREBUILD_BUDGET`) so that
  the snapshot is made: the tools, Go's toolchain and modules, and the cold `cargo build`, which
  fits; what the budget leaves (the rest of the browser app, Go's builds) the hook builds in the
  session. A session from such a snapshot should then find every dependency built, and compile
  the workspace's crates its branch changed, incrementally, except after a version bump: the
  snapshot keeps `folia/target` beside the clone, and the checkout leaves the files that did not
  change with the times they had (this session's repository was cloned at 07:34 and brought to its
  branch at 10:13; what did not change still says 07:34). The first session after the setup script
  is in place confirms it: `ls folia/target` before anything is built.

Go's modules and its toolchain come from `storage.googleapis.com`, where `proxy.golang.org` sends
them. The agent proxy of a session puts a network secret of the environment for `*.googleapis.com`
on those requests too, and while that secret fails, every one of them fails with it (`502`,
„injection failed", 2026-10-20), Google's fonts in the drawings of `folia/design` as well: give
such a secret the one host it is for (Gemini: `generativelanguage.googleapis.com`). The setup
script runs before the agent proxy and is not concerned.

**What it replaces.** Until 2026-10-20 `folia/scripts/build-cache.sh` gave every git worktree of
a workstation a build cache of its own, forked from the main checkout's with every dependency
built, and a hook of git (`folia/scripts/hooks/post-checkout`) forked it for every new worktree:
on the laptop of 2026-09-23 a new worktree's first build took 19 s + 57 s instead of 3 min 24 s.
In the cloud none of it applies — a fresh clone has no main checkout to fork from, and git does not
carry `core.hooksPath` along — and the one thing of it that pays everywhere, the flag, is in
`folia/.cargo/config.toml` now. The `.cargo/config.toml` the script wrote at the top of a checkout
can go; it is git-ignored, so a machine's own settings may stand there, such as the linker of
Windows: the toolchain ships `rust-lld`, but `gcc`, the linker driver of `x86_64-pc-windows-gnu`,
finds it only when pointed at the toolchain's `gcc-ld` shims (19 s instead of 24 s per rebuild on
that laptop; `<sysroot>` is what `rustc --print sysroot` says):

```toml
[target.x86_64-pc-windows-gnu]
rustflags = ["-Clink-arg=-fuse-ld=lld", "-Clink-arg=-B<sysroot>/lib/rustlib/x86_64-pc-windows-gnu/bin/gcc-ld"]
```

Measured with the worktree caches and not taken, so it is not tried again: one target directory
for several checkouts (they overwrite each other's incremental sessions: 200 s per switch instead
of 14 s), and sources backdated so that copied artefacts pass for current (the first edit then
takes the three minutes, and a file that differs but looks older leaves its old code in the
binary). `CARGO_INCREMENTAL=0` is part of every unit's fingerprint: measuring with it builds
everything again.

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
`/assets/icon-512.png`, `/assets/icon-maskable-334.png`, `/assets/icon-splash-512.png`,
`/assets/icon-splash-dark-512.png`, `/assets/icon-monochrome-512.png`, `/manifest.webmanifest`,
`/manifest-dark.webmanifest` (the same in the dark theme's colours), `/assets/launch/<w>x<h>[-dark].png`
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
`server/` is built again in 5 s in the cloud (`cargo build`, „Build times"). The page cache is not
warmed (`FOLIA_WARM_CACHE=off`): the warm-up
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
  listed screens only), the manifests (`/manifest.webmanifest`, `/manifest-dark.webmanifest`, each
  language's: browsers fetch them without cookies), `/healthz`
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
**The real catalog** is the one Betula serves (owner, 2026-10-01): `/api/db` is the snapshot
Radix exported, byte for byte (44 MB; `--compressed` takes it in brotli or gzip). Take canary's:
it is built from `master`, so its snapshot has the schema the queries of `develop` are written for.
betula.app still runs Folia 1.0.3 on a snapshot of schema 9 (2026-10-15), and with it
`the_queries_are_written_for_the_newest_schema` and every test of real data fails. Canary is
behind its gate: the owner gives the password, which never goes into the repository; the command
asks for it.

```bash
read -rsp "canary password: " pw; echo; curl -sS -o /dev/null -c snapshot/canary.cookies \
  --data-urlencode "password=$pw" --data-urlencode next=/ https://canary.betula.app/access; unset pw
curl --compressed -b snapshot/canary.cookies -o snapshot/catalog.db https://canary.betula.app/api/db
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
  the order by relevance, typos, the most words, what it finds outside the filters; and in
  `folia-search`'s `tests/ranking.rs` where what is meant lands, capitals as typed and the other
  title, lecturers written alike, the same modules for everyone and a program's first);
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
corrected typo above the list and the matches outside the filters under it, and lists Theoretische
Informatik first for „TI“, its T and I marked; in the browser app, a
search inside Informatik B.Sc. says what it finds outside the program and its link leads there; with
a stand-in for the semantic search (`window.betulaSemantic`; the repository has no model) „Ähnliche
Module“ stand under the results without the result and the module no longer offered, one of them
opens beside the list, and the arrow keys go on from the list's last row into them; a typo is
corrected and said (and the semantic search is asked for the corrected word), the best match comes
first, „Modul“ orders the matches by title, and typing orders them by relevance again; „TI“ lists
Theoretische Informatik first with its T and I marked, and „Hoffstedt“ finds Petra Hofstedt's
modules, the row saying „Hofstedt, Petra“ under the title. Last, a search the address carries: its
list asks before `boot.js` offers the semantic search and still gets its „Ähnliche Module“.

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
their own under the badges, filling it half each. On both, a shared preview link of a module far
down the unfiltered list (90 % of the way, from the server's pages), loaded directly: the page
answers within 3 s all the while, the list never holds more than 120 rows, the module stands
beside the list (a sheet on a phone) with its row marked and in view (on a phone it begins above
the sheet). Before the fix of 2026-10-17 a wide screen stopped answering and a phone's list held
2,945 rows.

```bash
cd e2e && node phone.mjs
```

drives the catalog on a phone, with real touches (`Input.dispatchTouchEvent`, so the browser
scrolls as it would under a finger): the page behind the open sheet standing still (swipes on
the dimmed page, the sheet's list scrolled past its end), the sheet swiped down from its list,
its head and its buttons (following the finger, snapping back after a short slow pull), a mouse
drag at its head and a tap beside it closing it, the draft (taps change the sheet's count but
neither the list nor the address; „Module anzeigen" applies it with one history entry, a swipe,
the close button and Esc throw it away, „Zurücksetzen" empties it; Back walks the lists applied,
an iPhone's within the page), the area picker staying open while the window
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
and one to the right one tab to the left, from wherever on the bar it starts, as a tap does (the
history written in place, Start the entry under every tab: § „Back") and without a page load; while the finger is down the row of tabs follows it and the mark
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
cd e2e && node back.mjs
```

drives Back as § „Back" has it. A phone as Android has it (Back a close request to the newest
`CloseWatcher` where something lies over the page, Escape in Chromium, else a step back): a dialog
of „Mein Studium" closes with Back and writes no entry, and Forward opens nothing; the box
„Studium planen" turns the row to the current semester without a step of the history and the
entry keeps it across a reload, the arrow at the head and Back bring the overview, three turns by
‹ › take one Back; the selection first, then the semester, by Back and by the arrow; the catalog's filters: Back throws the
sheet's draft away and writes nothing, „Module anzeigen" is one step, Back leads to the list
before; a module's sheet closed by its × goes back a step. A stack for each tab: Start under every
tab, a change of tabs writing the history in place, Forward not leading into what it left over,
the tab left coming back as it was, Back up through the tab to Start, the tab tapped again leading
to its first page, a module's program changing the tab with „Mein Studium" and „Alle Studiengänge"
under it, Start's tab leading to Start after the profile lay above it. A link from outside: Back leaves Betula while it is only read, the arrow lays the list
and Start under it. An iPhone (no `CloseWatcher`): Back is the history, and a dialog goes with the
page it leaves. A wide screen: filters closer than a second one step, after a pause another; the
module beside the list closed by its × going back where the list was the step before, and the
list's tab as well; after two steps back at once a module opened is a step of its own with its
address; after a reload the × still goes back, without loading the page anew.

```bash
cd e2e && node stages.mjs
```

drives the tabs that stay built as § „Tabs that stay built" has them, on a phone (the bottom bar,
taps) and on a wide screen (the rail, clicks): a tab seen before is seen within 80 ms of the
finger's or the button's release (in front, or on a phone the page on top as the pages move: the app
swaps the stages behind them once they move), without a skeleton and without a picture held in front
of it, with the same page
(the very elements), scrolled as far (the window on a phone, the list's area on a wide screen) and
with the module beside the list still there; one stage in front once the pages have come to rest,
the others not drawn, a link in them taking no focus, and on a phone without room; the title, one canonical address and one description of the page in front;
a filter and a row of a page that came back; the filters' sheet left open going as its tab changes,
nothing dimmed; Back on a phone up to Start, which comes as it was left. All of it once the start is
over; the start itself: every tab built behind the first page (a study set up, so that „Studium" is
„Mein Studium"), each first visit in the first frame after the tap without a skeleton and at its
top, the longest task meanwhile (with the CPU four times slower as well), the row of „Mein Studium"
at its overview, and a tab tapped while the others are built coming as before, its page not built
anew.

```bash
cd e2e && node achse.mjs
```

films the pages on an axis as § „The pages on an axis on a phone" has them, every frame, on a phone
with a study set up and every tab built, and what the eye sees of each (how clear it is, of what the
pages over it leave): a tap from Start scrolled down to Module, Module coming from the right on top
and half seen within 120 ms of the change's first frame, the tab's mark on „Module" in that frame,
Start going to the left under it; no frame jumping (12 px and a third of what is seen of a page at
most) and none with the window showing between the pages; at rest within 0.8 s with nothing of the
layers left and the window where the page was left; Back from Module to Start coming from the left,
scrolled as left; a tap back on the way; three taps in a row, and four quick ones back and forth; a
finger along the bar from the Merkliste to the Stundenplan (at a third of the way the Merkliste going
and the Stundenplan on top, partly seen, at the end the Stundenplan most of the way there) and let go,
a quick swipe (a finger's moves every 8 ms, sent as a phone sends them) taking 0.15 s at least from
a tenth of the page seen to nine tenths, and one let go short (nothing changes); in the first frame
of a change the ground of a page scrolled to its end and the dots of „Mein Studium" where they stood;
less motion a fade; a tab tapped before its page stands not moving; a wide screen not moving.

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
cd e2e && node plannote.mjs
```

drives „Einplanen" as concept E has it, in the synthetic catalog (§4): Algorithmen und
Datenstrukturen (11112, a winter module of the 1st semester of Informatik B.Sc.'s plan, compulsory
in three programs) and Analysis I (11103, a summer one). On a wide screen, without a study: the
session's semester kept in the tab, the switch saying where it plans; a click opens the dialog, „Was
studierst du?", three compulsory programs open with Informatik first („Pflicht · 1. Sem.") and the
search under them, as wide and as tall as they are and so when it opens, Esc in it leaving the
dialog open; „Einrichten und einplanen" waiting and nothing planned; Informatik picked, „im 1.
Fachsemester" and the module „wie im Regelstudienplan (1. Semester)", its row's picker with
„Studiengänge mit diesem Modul" first; the 3rd Fachsemester set, the warning of the plan's place
behind; ‹ into the past, the warning of the past; „Einrichten und einplanen" stores the program and
a Studienbeginn and plans the module, and the note says the semester and „eingeplant · 1. FS", no
switch yet, × at its end. A click on the semester's right half moves it at once („verschoben · 2.
FS") and brings „Weitere Module auch nach …"; the note is one bar across the content right of the
filters, one line high with the module named — the check and the name at the left, the semester,
„Rückgängig" and × at the right — and the switch comes as a row over all of it, lined up with them
(its icon over the check, its words where the name begins, its switch over the ×), while the
semester stays where it is; dark with light text in the dark theme and the light one; the left half
moves it back without the arrows moving, ← and → move it back and forth; the switch on keeps the
semester for the session; „Rückgängig" takes the module out. Analysis I then plans into the semester
chosen (P), its note shows the switch on, which turns off there; ‹ stops at the first Fachsemester,
and so does the left half; P on the module planned takes it out („Analysis I entfernt") and
„Rückgängig" puts it back. The note waits: it stands 6 s without a thing done, and a click elsewhere
puts it away, as do ×, Esc, a key not its own (↓), another module opened and a long scroll; a little
scroll does not. With SoSe 2034 chosen for the next modules, the dialog says „für weitere Module
gewählt" and „Aufheben" aims as without it. On a phone a row swiped to the right opens the dialog as
a sheet from below, nothing planned, the search not growing as it opens; „Ohne Studium einplanen"
plans and puts the question aside for the session; the note stands over the bottom bar without the
module's name or ×, as wide as the bar, under it and without a shadow; a tap on the semester's right
half moves the module, brings the switch as a row over the semester and leaves the semester under
the finger, between the arrows the warning alone („kein Angebot"); a finger swiped over it to the
left moves it on, to the right back, a short one not at all; swiped down the note goes and the plan
stays. The next row swiped plans at once; a little scroll leaves its note, a long one puts it away,
and so do a tap elsewhere and a module opened; planned from a module's sheet, the note goes down
with the sheet as it closes, never up first. On a small wide screen (1024 px with the sidebar at its
widest, 1100 px, 1366 px) nothing of the note lies over anything else or out of it, the semester
stays where it is as the switch comes a row over it, the module is named where there is room, and
„entfernt" stands whole.

The other checks that plan (`module.mjs`, `swipe.mjs`, `snappy.mjs`) put the question aside for the
session first (`betula.plan.asked`), as a student who answered „Ohne Studium einplanen".

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
than the window (the empty Stundenplan) just past the window's lower edge.

```bash
cd e2e && node top.mjs
```

drives „Nach oben": on a wide screen the catalog's whole list (out of sight at its top and less
than a screen down, in the corner of the list far down; the wheel over it turning the list its
whole way, measured by a row on the screen, since the list's making up for rows measured anew
moves the scroll position and nothing that is seen; a click bringing the list to its top with its
first row, `page` gone from the address, the button gone and no page loaded; a wheel stopping the
way up; with a module beside the list the button left of it and the module staying; Enter on it,
and Tab going on from the start of the page; the module's page opened from far down starting
without it, and Back to the list far down bringing it again), a long page with the ground in (the
button above it, the ground going back down), less motion (up at once); on a phone the window (the
button 12 px above the bottom bar, a tap), and a page far down before the app takes over (the
app's own button there after it); the classic site with the app kept away; without JavaScript no
button.

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
browser none. Every picture named is served as a PNG exactly as large as the screen. And the
manifest a page names: the dark one in a dark system or with „Dunkel" chosen in Betula, the light
one in a light system or with „Hell" chosen, in English `/en/…`, each served in its colours.
And the start of the installed app: where its icon and wordmark stand for Chrome (Android's
splash), Brave (Chromium's, before and from 155) and iOS, under a measured status bar, that they
fly to the logo once a session, fade without one or with reduced motion, and go at a tap; the
status bar measured at a touch, and kept only where it fits. Needs no snapshot.

```bash
cd e2e && node bookmarks.mjs
```

drives „Merken". Desktop: the button at the end of a row (in its place, the column labels still
over their columns, a click neither navigates nor renders the list again), `M` on the row the
keyboard is on, in the preview and on the module's page, one state wherever a module shows, the
switch keeping its width, the number at the rail, a reload keeping the marks; the list of marked
modules (order of marking in its sections, numbers and credits, the order at the heads of the
columns with „Gemerkt" turned round and „Turnus" in its sections, the search at the head finding
among the marked modules without touching the address or any request, with its line into the
whole catalog and its empty state, the halves of the year with their counts where the marks are
not all offered alike, the floating preview, a mark taken away staying on the page,
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
a 44 px target, marking by touch, the bottom bar's count, the sheet behind „Filter" (closed until
opened, an order picked there a draft until „2 Module anzeigen", the badge counting it, a draft
thrown away with the ×), a tap opening the module in the list's place (the address still the
list's) and „Zurück" returning. Without the app: nothing of it
shows without JavaScript, `/bookmarks` explains itself and is `noindex`; with JavaScript but
before the takeover the buttons are invisible and their room is kept (the heading is exactly as
tall as once the app runs).

```bash
cd e2e && node profile.mjs
```

drives the profiles and their page: the rail's way to the page (current there, no tab), no switch
of the languages in the rail, the first profile unnamed with the browser's marks; „Neues Profil“
(the name asked for, the profile empty and active, the rail's badge, the list as kept, its marks
under `betula.p2.…` alone); a switch in the sidebar bringing a profile's marks back; renaming;
another tab whose Merkliste and rail follow a switch; „Profil leeren“ and „Rückgängig“; the menu of
the profiles (a mouse resting on the rail's way: the profiles, the page, the other language, a
profile chosen; a right click: the focus on the active profile, one chosen with the keyboard, Esc);
light, dark and the system's with the rail's switch and the browser's chrome; a profile as a file
(its name, its JSON) and a profile made of it, a file that is none; deleting the active profile and
„Rückgängig“; the script in `<head>` reading the active profile's program. Privacy: no request
carries a profile's name or a mark, none leaves the site. English; on a phone the box at the head
(as high as the search, at its right, nothing on the brand line), a finger held on it (the menu, a
release that goes nowhere, a profile tapped there), a tap leading to the page, the profiles before
the page, no sideways scrolling; without JavaScript the page explains itself, is `noindex`, offers
the other language, and the rail's way to it shows.

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
the choice remembered in this browser only, the plan at every width of the window (no cell out of
its column, no row taller than its name, the list where the matrix has no room), the areas as
groups of rows with the sidebar leading to each of them without a history entry, „Mein Plan"
leading to all of the program's modules in the catalog (as many as the head counts), and on a
phone the plan as a list without a switch, its rows as tall as their names (Informatik B.Sc.,
„Art" and „Bereich" given way), and a page that does not scroll sideways, a module coming up as a
sheet over the page with one history entry (over „Mein Studium", „Alle Studiengänge" and Start,
which the first use of a page come from outside lays under it) and its × going back a step, an
area becoming the page and a module picked out of it leading back to the area.

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
then on a phone, once the start is over and every tab built, the bottom bar and a module. It
clicks what is seen (`>> visible=true`: the stages behind have parts of the same name), and an
interaction nothing comes of ends after 15 s. The first frame after each has to come within 80 ms
and show what was clicked in its new state (and a skeleton, where the page or the list is being
built for the first time), a later one the result without a skeleton, and no skeleton stays.
All of it once more with the CPU slowed down four times (200 ms for the first frame), where the
skeletons are what bridges the wait. The numbers it prints are the time to the first frame and
to the result.

```bash
cd e2e && node navkeys.mjs
```

drives the navigation's keys (R10) on three systems made up through `navigator.platform`: on
Linux Ctrl and Alt and a digit go to the tab, 1 to 5 one after another while the keys stay held,
and from the search with a text typed, without a page load; pressed, they show at once
„Strg + Alt + 1" to „… 5" at the rail's tabs and nothing in the hidden bar, and let go, nothing;
Ctrl alone, Alt alone, Ctrl+Alt+X and a click while they are held show nothing; Alt and a digit,
Ctrl and a digit, Ctrl, Alt and Shift and a digit, a digit no tab has, Ctrl+C, S and P do nothing
and keep their default, and so do the keys while a modal dialog is open. On Windows Alt alone,
„Alt + 1" to „… 5" while it is held, its keyup taken after it was held, and a tapped Alt shows them
and keeps its own; Ctrl and Alt (AltGr) do nothing. On a Mac ⌃⌥, „⌃ + ⌥ + 1", while ⌥ or ⌘ with a
digit do nothing; in English „Ctrl + Alt + 1". Where the app does not start, Ctrl+Alt+3 loads the
page of „Studium".

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
  is read beside it. Codes of the time before are not read (canary only). Since 2026-10-13 a
  Termin is one entry, a series (owner: an entry per date was „ziemlich nervig" to deal with): its
  rule (RRULE: every week or every two to four weeks, every day of a block or its working days)
  runs from its first date to its last, UNTIL the end of that day in UTC; the dates it makes
  and the Termin does not hold (the break, a holiday, a date QIS cancels) are left out one by one
  (EXDATE, in the series' time zone and start), and a date that says more or other than the series
  (a room note, another end, another room) follows it in full under its UID (RECURRENCE-ID), so a
  date is exactly what it was. The series says what most of its dates say. Its UID is its row
  key's (`148701-a2633@betula.app`); a single date keeps the UID of its key and its date. Single
  dates that Radix reads as one series (QIS lists a seminar's Tuesdays one by one;
  `event_date.series`, docs/radix/schema-v2.md, „Series of single dates“, schema 13) are one series
  too, under the key of the row of its first date; its description says „in QIS als
  Einzeltermine", and a week it has no date in is named („kein Termin in QIS"). Exams stay an entry
  per date. The cap of a calendar counts VEVENTs: „nur die ersten 5000 Einträge". The pinned checks
  read their snapshot of schema 9 as Radix's later migrations leave it
  (`folia_test_support::migrated`), without series, and count the 249 dates of the pinned code in
  its series (`Entry::dates`: what a calendar shows). Still open: the edge logs
  the address like every address, 7 days in Loki and 7 days in the host's journal
  (`deploy/README.md` §9); a Traefik router for `/calendar/` with
  `observability.accessLogs=false` would leave it out, but needs the blue-green priority label in
  `deploy/vps/50-app.sh`, `55-switch.sh`, `lib-stacks.sh` and `91-verify-stacks.sh`. And a
  subscription over several semesters: today the next semester needs a new address.
- **A series of single dates is one Termin wherever Folia shows Termine** (owner, 2026-10-13:
  „Kannst du die reoccuring termine in folia bitte auch gebündelt zeigen. Weil das ist gerade
  sehr unübersichtlich, wenn da einzeltermine sind"): the dates Radix reads as one series
  (`EventDate::series`, schema 13; `module_schedule` reads the columns as the Studienplan's
  queries do) stand as the Termin they are, in the series' rhythm, instead of a line and a slot
  per date. Its dates stay single dates where dates count: the agenda lists each, a cancelled one
  with its reason, and clashes, choices and SWS go by them (`Every::of`, QIS's rhythm). The shown
  rhythm is `Row::every` (`Every::shown`): every week, every other week in the A or B weeks its
  dates fall in (`SemesterFacts::ab_week`; without A/B weeks a fortnightly series stays single
  dates there), every three weeks (`Every::ThreeWeeks`, which QIS itself never states) or four.
  A date of a series at another time than its others stands apart as the single date it is.
  - **The module page** (`folia/crates/widgets/src/module.rs`, `termine`, `termin_text`): one line
    in the list, „Seminar · wöchentlich · 6 Termine · 13.10.2026 – 17.11.2026 · HG 0.20 · …", its
    dates in the line's tooltip („in QIS als Einzeltermine: 13.10.2026, 20.10.2026, …"); one slot of
    the week like a weekly one, its rhythm and days in the tooltip, where its dates were one
    `.once` slot that counted them („6 Termine"); one schema.org `Schedule` in its rhythm (`P1W` to
    `P4W`) with the days it leaves out (`exceptDate`). Since 2026-10-14 (owner: „mach auch die
    zwei anderen Bündelungen") the list has one line per Termin in two more ways: the single dates
    of one slot that are no series, one line that counts and names them („Seminar · 1-Gruppe · 6
    Einzeltermine · 06.10., 20.10., …"; the week drew them as one slot already), the same date in
    several rooms among them; and the recurring rows of one slot (event, group, weekday, start,
    end, rhythm), a Termin in several rooms, and (owner: „bündel auch die geteilten Zeiträume") one
    QIS splits into several ranges, which the line names one by one („wöchentlich · 09.10.–23.10.,
    06.11.–27.11., 11.12.–18.12., 08.01.–15.01."; 84 of the 1,825 recurring slots of 2026W; three
    of them have ranges of another end, and rows QIS gives no range, which stay lines of their
    own). Single dates beside a slot's weekly rows stay apart, as in the week. The 1,503 module
    pages of 2026W show 4,181 lines (all rendered from canary's snapshot, none with an empty
    „()" or a „[unbenannt]"), 4,936 with the series alone bundled (Management, 12950: 60 to 6;
    14675: 21 to 5). A room, person or
    comment not every date or range of a line has names them, or where most have it those it
    leaves out (`said_on`: „ZB/VI.03 (16.10., 27.11.) · 'Feedback Session' findet online statt
    (06.11., 18.12.)", „SFB/1.308 (außer 12.01.) / SFB/1.309 (12.01.)", „SFB/1.211
    (09.10.–23.10.)"), so that the line says of no date what QIS says of another. QIS's „[unbenannt]", its group without a name,
    is no group the line names (owner: it read as a person QIS does not know); the Studienplan and
    the calendar left it out already.
  - **The Stundenplan and the Studienplan's week** (`Timetable::regular_week`, `studyplan/week.rs`):
    one recurring slot, „ab"/„bis" where it covers a part of the lecture period, A or B, „3-wöch.",
    „4-wöch.", its dates in the tooltip; a date QIS cancels stays in its range (`Row::slot_day`),
    and a series QIS cancels whole has no slot, as a recurring row has none.
  - **The panel beside the plan** (`studyplan/aside.rs`, `by_termin`): one Termin, „Di 13:45–15:15
    · HG 0.20 · A/B" over „5 Termine · Zentralcampus · 13.10.–17.11."; its eye hides and shows the
    keys of all its dates together (a plan hides rows by their keys, one per date), and the list of
    what is hidden („2 ausgeblendet") names the hidden series once, „Einblenden" showing them all
    (`Unhide::Rows`).
