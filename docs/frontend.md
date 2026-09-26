# Folia, the web tier: architecture, rules, how to run it

> Betula has two parts named after the birch: **Radix** (the root: the Go collector, `docs/operations.md`)
> and **Folia** (the leaves: this web tier, the crates `folia-catalog`, `folia-app`, `folia-client`,
> `folia-pack` and `folia-server` with the binary `folia`).

> State: 2026-09-21. Every page is server-rendered and works without JavaScript; with
> JavaScript the browser app (WASM + local SQLite) takes the page over and nothing is loaded
> again, and the app starts without a network (a service worker keeps its shell, IndexedDB the
> catalog). Not yet: user data beyond the marked modules, context search.
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
| `pack/` | Values as codes that travel in a link (`pack::to_code`, `pack::from_code`): serde's data model as bits (fields by their place, numbers in as many bits as their size needs, `pack::set` and `pack::list` for ids), written in the 66 unreserved characters of an address (`A–Z a–z 0–9 - . _ ~`), the last two of them check the rest. No I/O, no dependency but serde; the format is frozen (`pack/src/lib.rs`). |
| `server/` | axum: snapshot client, HTML cache, the app's routes, `/api/db`, `/api/status`, `/healthz`, assets. |
| `e2e/` | `crawl.mjs` (the server-rendered site, no browser), `spa.mjs` (the browser app: takeover, no page loads, preview, filters, the virtual list, search), `filters.mjs`, `module.mjs`, `programs.mjs`, `bookmarks.mjs`, `phone.mjs` (the phone layout: the sheet, the pickers, the list), `home.mjs`, `snappy.mjs` (a click answering in the next frame, skeletons), `schema.mjs` (a local copy of the catalog of an older schema, with and without a network), `smoke-walk.js` + `run.mjs` (long program walk), `shot.mjs` (review screenshots). All use an installed Edge through `playwright-core`; without one, `node --import ./chromium.mjs <check>.mjs` runs a check in Playwright's Chromium or in the browser `SMOKE_BROWSER_PATH` names. |

### Routes (`catalog/src/url.rs`)

| URL | Page |
|---|---|
| `/` | Landing page: every function with a link |
| `/catalog?…` | Module catalog. The query string is the whole filter state (`CatalogUrl`): `q`, `program`, `list=fues`, `semester`, `area`, `kind`, `lecturer`, `department`, `turnus`, `years`, `form`, `duration`, `limited`, `fues`, `exam`, `graded`, `events`, `status`, `ects_min`, `ects_max`, `campus`, `lang`, `marked`, `prereqs`, `sort`, `desc`, `page`. What can be wanted can also be excluded: `not-kind`, `not-lecturer`, `not-turnus`, `not-form`, `not-exam`, `not-campus`, `not-lang` (`exam=written&not-exam=presentation`: a written exam and no presentation). `area=<id>[,<id>…]` are areas of the selected program's module tree („Wahlpflichtmodule Praktische Informatik"): the modules the tree places in any of them or below one (several come from a row of the plan that means several areas, opened from the program's page; the picker then says „5 Bereiche", one tag per area above the list) |
| `/catalog?…&open=<id>` | In the app: the same list with this module previewed next to it; the preview has a „Vollbild" link to the module's page. On a phone there is no preview: a tap on a row opens the module's page, and the app turns a shared `open` link into it. The server's page (crawlers, no JavaScript) ignores `open`: it renders the plain list, every row leading to the module's page (owner decision 2026-09-21: the server's HTML is for crawlers, the app for people, and no query parameter changes the server's layout) |
| `/catalog/module/<id>` | The module's own page: a sidebar as wide as the filter panel (sections of the page, actions), the module on the rest of the screen |
| `/programs?q=…&level=…&form=…&plan=1` | Program overview (current PO versions) by faculty (`ProgramsUrl`): the search of the top bar, degree (`bachelor`, `master`, `teaching`, `doctoral`, `other`), form of study (`dual`, `double`, `flexible`), only with a validated study plan |
| `/programs/<slug>/plan\|areas\|modules[?variant=<n>][&area=<id>][&req=<n>][&open=<id>][&full=1]` | Program page (`ProgramUrl`); its views are switched in the sidebar. Where a program has several study plans (one per study direction), `variant` says which one is shown; `area` is the area of „Wahlpflicht & Bereiche“ shown beside the page, `req` a row of the plan that names no module, `open` the module — they stand in the address (a shared link, the history) and the app renders them; the server's page ignores all of them (it lays nothing beside itself: its module links lead to the module's page, its area links to the catalog narrowed down to the area, a row without a module is text), so they are no part of its cache key; the canonical address stays the plain one. A module opened out of an area keeps it, so closing the module returns to it. `full=1` shows the module of `open` in full: the module's own page, in place, so that „Vollbild" stays in the programs area (its tab, its history, its „Zurück"); the canonical address of that view is the module's page. On a phone whatever is picked — the module, the area, the row of the plan — is the page (`open` alone shows the module in full there) |
| `/programs/<slug>/plan\|areas\|modules` | Program page; its views are switched in the sidebar |
| `/bookmarks?turnus=…&sort=…&desc=1&open=<id>[&full=1]` | „Merkliste": the modules the visitor has marked (`BookmarksUrl`). The URL says how the list is shown (half of the year, order, the previewed module), never what is on it: the marks live in the browser. `full=1` shows the module of `open` in full, in the list's place, as `full=1` does on a program's page (a local view, `app/src/local.rs`): „Vollbild" stays among the marked modules (their tab, their history, their „Zurück"); on a phone `open` alone does. The server renders an explanation, the same for everybody, `noindex` |
| `/impressum`, `/datenschutz` | The legal pages (`app/src/pages/legal.rs`): the Impressum and the Datenschutzerklärung, final since 2026-09-25 (placeholders from 2026-09-21). Linked from the ground at the end of every page („The birch"; § 5 DDG: reachable at all times). The privacy notice says what the software does — the edge's access log and its retention, Folia's log, what stays in the browser, the calendar feed, the gate's cookie, the lecturers' names (Art. 14 DSGVO) — and `legal.rs` names the source of each part: a change there is a change of the text. `legal::PLACEHOLDER` stays the switch `deploy/ship.sh` reads: true again, the pages are `noindex` and no instance open to everybody (`FOLIA_ACCESS_GATE` not `on`) ships |

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
  - **Pickers** (`app/src/combobox.rs`: program, area, lecturers, department) have a search that
    forgives typos and knows initials and abbreviations (`catalog::fuzzy`: „infomatik bsc"), arrow
    keys, Enter, Esc. Their popup is fixed to the window, so no panel clips it; on a phone it
    opens in place — and there nothing that moves the window closes it: the on-screen keyboard
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
    module's whole page in the list's place. The sidebar holds what belongs to the list as a
    whole: its numbers (modules, credits), the halves of the year as a row of links with their
    counts („Alle", „Winter", „Sommer": what the catalog's turnus filter would find among the
    marked), the order (of marking, the newest first; by title, credits, teaching events; the
    column headers sort as in the catalog), and the actions: copy the list as text, and empty it,
    which asks first and can be taken back („Rückgängig").
  - **A mark taken away on that page stays on the page,** dimmed, until the page is left: a slip
    is one click to undo, and the list does not jump under the pointer. Marking changes numbers,
    never the list: no query runs and the rows stay the same elements (each button reads the
    marks through a memo of its own, R5).
  - A marked module stays on the list when it is no longer offered, and one the snapshot does not
    know (taken out of the BTU's catalog) is named under „Nicht im Modulkatalog", not dropped
    (R12). What is stored is read like anything from outside: ids that cannot be ids are
    dropped, a module counts once, the list ends at 2,000.
  - **A module opened from the marked modules stays among them** (owner, 2026-09-24: „Vollbild"
    used to switch to the catalog's address, so the catalog's tab kept the module open and its
    „Zurück" led back to the marked modules): „Vollbild" of the preview shows the module's whole
    page in the list's place (`&full=1`), and on a phone a tap on a row does (`open` alone), as on
    a program's page (a local view, `app/src/local.rs`). The tab „Merkliste" stays the current one
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
  app every tab is the plain link to its area. **A module opened out of an area that shows its
  modules in place does not become what the catalog remembers** (a program, the marked modules:
  `Area::shows_in_place`; owner, 2026-09-20 and 2026-09-24): its tab keeps leading to the list
  as it was left, and that list does not reveal a module the visitor never picked there. Such a
  module stays in its area anyway (`app/src/local.rs`); a module's own page reached from there (a
  successor named on the module's page) leads **back into that area**, not to the catalog: the
  step before answers, and after a reload the programs' own memory does (it was left at a
  program page that names this module in `open`). Not where the page is what the catalog was
  left at: the visitor came back to it (the catalog's tab, Back), and „Zurück" leads up to the
  catalog's list, not across to the area they were in between. Where „Zurück" leads is read
  again on every change of the address, so after closing the module beside a program Esc follows
  the link out of the program instead of walking the history back into the module it has just
  closed.
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
  the page (`Filling` in `app/src/pages/program.rs`), and „Zurück" leads to what it was picked
  from (a module picked out of an area back to the area). No preview that then has to be opened
  in full, no panel that unfolds under the page. Without the app the same HTML (the panel beside
  the page) is shown as the page by the stylesheet (`.aside-picked`).
- **Local views (owner, 2026-09-24: „so, dass man das in jedem Tab ganz einfach implementieren
  kann als lokale Ansicht"; `app/src/local.rs`):** showing a module in place is one mechanism,
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
  (`plan_total`, `docs/schema-v2.md` §6). The head's „LP", the sum under the matrix and the sum
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
  (`catalog::plan::areas_for_row`, rewritten 2026-09-21 after the owner found „Komplex Praktische
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
- **The search in the top bar belongs to the page:** modules everywhere, programs on `/programs`.
  In the browser app it filters while typing (history entry replaced, not added).
- **Tokens:** `app/assets/app.css` starts with the token block (colors, radii, shadows); everything
  below uses tokens only. One look, light and dark: dark follows the system, the switch in the rail
  overrides it (`data-theme` on `<html>`, remembered in `localStorage`). Accent color only for
  primary actions and the marker of the open row; selected chips are neutral (inverted). The
  accent is a muted birch-leaf green, `oklch(.53 .07 149)` in both themes (owner, 2026-09-22,
  replacing the blue: "wir nennen das ding betula"): the hue of the owner's favourite tone
  `oklch(.6867 .0996 149)`, deep enough for white labels. Labels on the accent are always white —
  dark text on the green was "grauenhaft" — and the owner prefers toned-down colours to saturated
  ones.
  Font: Inter (variable, latin subset, OFL), self-hosted. Icons: Lucide (ISC), inlined through
  `app/src/icons.rs`. The only `style` attributes carry data as custom properties: the week grid
  and the credit slider (`--from`, `--to`, `--at`), the place of a picker's popup, `ui::Hit`.
- **`assets/enhance.js`** (progressive enhancement until the browser app takes over): the plain
  fields of the filter form apply on change, panels keep their scroll position across page loads.
  In both modes: the shortcuts, the theme switch, the filter sheet, and the two resize handles. Page changes use
  cross-document view transitions where the browser supports them. Their opt-in
  (`@view-transition`) is written inline into every head the server writes
  (`app::VIEW_TRANSITION_STYLE`), not into app.css: Chromium decides when it first shows the new
  page, from the style sheets applied by then, and the stylesheet (revalidated on every load) often
  arrives after the parser has reached `<body>`. The page then came without the fade and with
  "ViewTransition opt-in disabled" in the console (`e2e/gate.mjs` checks it with a slow stylesheet).
- `design/prototype.html` is the clickable design prototype the direction was agreed on;
  `node e2e/shot.mjs <url> <out.png> [w] [h] [--dark]` takes review screenshots.

### Data flow

- **Pages are synchronous functions of their route parameters.** SQLite answers in 1–7 ms on
  both sides (rusqlite on the server, sql.js in the browser), so there are no async resources,
  no loading states between pages and nothing to serialize into the HTML. A page calls one
  loader of `catalog::pages` through `Source::run`; everything it shows comes from one snapshot.
  What the browser app shows between a click and the page is the page's skeleton, one frame
  before the page is built („A click answers first" below), not a state the page waits in.
- **The server renders and caches.** HTML depends only on URL + snapshot (rule R9), so the first
  request renders (5–100 ms) and later ones are a memory copy (2 ms), gzip included; the cache keeps
  the compressed page only, and the pages of the sitemap are rendered into it after every new
  snapshot while the server is idle („Load“ in §3). A new snapshot starts a new generation. ETag
  per generation and build → `304` without rendering.
  Pages are `public, no-cache`: the browser asks every time and mostly hears `304` (until
  2026-09-21 they were `max-age=300, stale-while-revalidate=86400`, so after a deploy a browser
  showed the old build's page with the new build's stylesheet for up to five minutes, and once
  more after that; the server has only the files of its own build, whatever `?v=` asks for).
  `404`/`5xx` are `no-store`. Without a snapshot everything answers `503` + `Retry-After`.
- **The browser app (owner decision: all queries run in the browser).** `assets/boot.js` opens
  the local copy of the snapshot (`/api/db`: 36.8 MB, 6.5 MB gzip; kept in IndexedDB with its ETag;
  sql.js) and loads the WASM bundle (535 KB gzip) in parallel; then `client::start()` replaces the
  server-rendered body by the app. Not hydration: the local copy may be older than the server's
  page, so the app renders fresh with the same components. From then on links, filters and the
  search are client-side navigation on the local database (measured: takeover 1.2 s on a first
  visit, preview 130 ms, filter 150 ms including the test driver). Until the takeover, and if
  anything fails, the site stays a classic website served from the HTML cache. A newer snapshot is
  downloaded in the background and used from the next start, unless the copy is of an older
  schema than the build reads (2026-09-23): such a copy is never opened. `boot.js` reads a copy's
  schema from its SQLite header (`user_version`) and compares it with the build's
  (`catalog::SCHEMA_VERSION`, Radix's newest migration, written in by the server); with the
  network an older copy is replaced first, as on a first visit; offline the app does not start, and
  the status says so. Before, a returning visitor worked on the old copy until the download behind
  it had finished, and after 0008 the plan page failed with „no such column: source_pages". A
  server whose own snapshot is older (Radix has not exported the new schema yet) says so in
  `/api/status` and logs `snapshot.outdated`; nothing is downloaded from it, and the site stays a
  classic website until Radix has. The server never answers data queries for the app: its load is
  cached HTML, static files and one database file.
- **Fine-grained updates:** the catalog page splits its URL into the filter (what the list is),
  `page` (where the visitor is in it) and `open` (the preview). Opening a preview or scrolling
  re-renders neither list nor filters, and a filter change leaves the preview alone.
- **The list is virtual** (`VirtualRows` in `app/src/pages/catalog.rs`, 2026-09-21; before,
  chunks of 50 were appended and prepended, and a long scroll grew slow): an element as tall as
  the whole list holds only the rows that are on screen and a few around them, each at its
  offset, so the scrollbar has the length of the list from the start. Rows are measured once
  rendered and estimated (at the average of the measured ones) until then; a row above what is
  visible that turns out taller or shorter than estimated moves everything below it, so the list
  scrolls by the difference and nothing jumps under the visitor's eyes. Pages of 50 are loaded
  when their rows come near (one query, a few milliseconds) and dropped again when far. `page` in
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
- **A semester lists what can be chosen for it, too** (`catalog::plan`, 2026-09-21): a plan
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

- **Paint first, then work** (`app/src/pending.rs`). A navigation reaches the router one frame
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
- **What still has to be computed stands there as a skeleton** (`app/src/skeleton.rs`): the frame
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
- **The answers of the local catalog are kept for the visit** (`client/src/lib.rs`, `Answers`):
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
  only to warm the kept answers would hold a second copy of the catalog (37 MB) on a phone.
- **Warming the answers ahead, in idle time.** A page's loader cannot be split (the start page's
  is 70 ms of a laptop, 280 ms of a phone), and a tap that comes during it waits for it.

`node e2e/snappy.mjs` checks all of it (§4).

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
is the program's (no part of the cache key changes). `node e2e/programs.mjs` walks it.

### Exam dates the BTU cannot mean (`catalog/src/exam_reading.rs`, 2026-09-21)

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
140 with a validated plan) with `catalog/examples/area_survey.rs`, which opens a snapshot and calls
the crate's own functions — exactly what the app does — and prints every picker and every row:

    cargo run -p folia-catalog --features native --example area_survey -- <catalog-*.db> [slug…]

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
fixtures (`catalog/src/area_fixtures.rs`: Informatik B.Sc. and M.Sc., Elektrotechnik B.Sc. 2022
with its study directions, Elektrotechnik M.Sc. 2018, Architektur, Wirtschaftsingenieurwesen
dual), and the fixture generator (`internal/catalogbuild/folia_fixture_test.go`) builds its trees
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
under the text below it. The figures (`dl.birch`) stand on the panel in the colours of the theme
(owner, 2026-09-22: „sollten sich mit dem Thema anpassen", only the colours; until then they stood
on `--bark`, light in both themes like the mark, a bright block on a dark page), one under the
other with thin rules, and on each rule a stroke from alternating edges, the strokes of the mark;
each number is set so large that all of them are about equally wide (`--em`, its width in units of
its size, from `figure_em`; a single digit grows only as large as three), quiet in weight and
colour (owner: „kleiner und etwas dezenter"). Under it a
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
The sidebar: the sections (the current one follows the scroll, `nav[data-spy]` in `enhance.js`)
and the Datenstand as label | value rows. The versions and Impressum and Datenschutz moved into the
ground at the end of every page on 2026-09-25 („The birch" below).

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

### The birch: crown and ground (2026-09-25)

The owner's idea: leaves along the header, as if the view began where a birch's crown begins, and
at the end of every page the ground with the roots. Picked on a design canvas of concepts
(`design/birch/`), in the owner's words where they decided:

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
  abgeschnitten verkleinert" and does not scroll): on a wide screen the page scrolls inside the view
  as always, and its scrollbar is the only one — the window's is never shown. The ground waits below
  the window's edge, fixed to the window. While the page is not at its end the window has no room
  to scroll (`data-ground="mid"` on `<html>`), so the wheel over the header or the rail changes
  nothing. At its end (`end`) the window gets room for exactly the ground and the gap above it;
  the next turn of the wheel goes on from the page to the window, and as far as the window
  scrolls, the ground comes up and the view gets shorter (`in`). `enhance.js` writes how far
  straight onto the boxes that move, once per frame (the view's height, the ground's shift, the
  bodies of the panels beside the page); the first build set a custom property on `<html>`, and
  the browser worked out the style of the whole page again in every frame — „mega laggy": a glide
  into the ground and back on the start page cost 391 ms of style, now 48 (plain scrolling there:
  22), with no long frame left. The rail and the view stick to the window's top, so nothing else
  moves. The page stays at its end, so its end goes up with the ground (a short list, which has
  no end to reach, simply gets a shorter panel); the panels beside it keep the height of their
  content and are cut off by their shorter box (`.sidebar > .body`, `.filters > form`,
  `.work > .detail > .scroll` get as much as the ground shows as a negative margin), and while the ground
  shows the wheel over them goes to the window, so they do not scroll. Upwards the ground leaves
  first (the wheel upwards belongs to the window while it shows), then the page scrolls. A page
  that leaves its end under the ground all the same — its scrollbar, a question opened, another
  page — sends the ground back down; Tab into the ground brings it up. Every panel ends 8 px above
  the ground, as above the window's edge (a framed page keeps 1 px under its last panel, not 24).
  Without `enhance.js` the ground lies after the view in the body's second row and the window
  scrolls to it; on a phone the ground follows the page, full width, the bottom bar floating over
  its lower part, and the crown carries the frosted background of the bar at the top.

The pieces: `app/src/ground.rs` (`Crown`, `Ground`; the ground's data is `pages::ground`, the meta
and the current semester), „the birch" in `app/assets/app.css`, the ground's behaviour in
`app/assets/enhance.js`. The crown runs along the whole top on every screen (owner, the same
evening: „durchgehend und auf allen Geräten"; the first build hung only where nothing stood — at
the mark, at the end of the title column, right of the search — and fell apart into clumps with a
thin edge between them and a gap on wide screens). Two masks per season (`app/assets/birch/`): a
tile of 1200 px that repeats to the right edge, dense at the top and hanging deeper and shallower in
long waves (never below 46 px, so nothing hangs out under the search), and at the left end a head of
420 px: the mark, and over the title a clearing where only the crown's edge hangs in, the twigs at
its sides leaning away („ein natürlicher Platz für die Schrift", as the name had it before). The
head ends in the tile's own end, and the tile is cut to its box from 420 px on (`mask-clip:
content-box`), so the seam does not show and nothing is drawn twice. A phone has no title in its
bar and shows the tile alone. The mark and the search stand in front of the crown and cover it.
The masks are coloured by tokens (`--crown`, `--crown-ck` for spring's catkins, per season and
theme), so the same files serve light and dark. `node design/birch/birch.mjs` draws all
masks, the roots and the leaf litter again (deterministic, seeded); the server serves them under
`/assets/birch/` (`api::birch`), and a server test fetches every mask the stylesheet names.
`node e2e/ground.mjs` drives it with a real wheel.

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
  makes the site installable; the service worker makes it start without a network.
- **Offline (`app/assets/sw.js`, served as `/sw.js`, registered by `boot.js`; 2026-09-21):** the
  worker keeps the shell of the app — a page of the site (the browser app renders whatever the
  address names from the local catalog), the scripts, the styles, the bundle, the font, the icons,
  the manifest — and nothing of the data: the catalog is in IndexedDB, where `boot.js` keeps it,
  and `/api/*` is never intercepted. Pages come from the network first and are kept for the way
  back (sixty of them); offline, the kept page, else the shell. Assets come from the cache first.
  The server writes its build into the worker, so a new build installs a new worker, which caches
  the new shell and drops the old one; the worker's own file is revalidated on every use like the
  other assets. **A page and its files always come from one build** (2026-09-21): the document
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
  once more, the bundle included. Assets stay `no-cache` with the build as ETag: the server
  answers every `?v=` with the file it has, so such an address must not be cached as immutable.
  `e2e/deploy.mjs` plays a deploy with two builds whose stylesheets differ. `boot.js` finds
  `/api/status` unreachable offline and simply opens the copy it has, unless that copy is of an
  older schema than the build reads: then the app does not start, the page stays the one the
  worker kept, and the status says „Offline – die Daten werden neu geladen, sobald du online
  bist" (the only case in which a failed start says anything). Once the app runs it says
  nothing: the „Offline bereit" notice is gone (owner, 2026-09-21: „wenn es einfach
  funktioniert, dann passt das"); only the loading of the data on a first visit is announced. `e2e/pwa.mjs` cuts the network and loads pages afresh; `e2e/schema.mjs` plants a copy of an older schema and starts with and without a network.

Not done: submitting the sitemap to the search consoles (needs the owner's accounts), a
`lastmod` per module (the snapshot has no date per module), English pages.

## 2. Rules

R1–R8 from `docs/frontend-rewrite.md` §5 apply. In short: navigation state reaches pages as
plain values from the router; nothing page-owned is read after unmount; no panics (`unwrap`,
`expect`, indexing and `panic!` are denied by clippy in all three crates); filter state = the
URL, UI state never triggers a query; keyed lists; one source of truth; design tokens only, no
inline styles; keyboard and phone usable. Added in phase 0/1:

- **R9. Server HTML is user-independent.** Merkliste, Studienplan, Mein Studiengang and the
  finder switch („Passt in meinen Stundenplan") follow the Merkliste: they live in the browser, the
  server renders them empty, and they are applied after the takeover, never during the first
  render (passed modules will do the same).
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
  `app/src/pages/legal.rs`): a new store, or a new way for stored data into an address, is a
  change of that text too. `e2e/bookmarks.mjs` watches every request of a session for marks.
  Exceptions, decided by the owner (2026-09-23/24/25): the address of a calendar subscription
  (`/calendar/<code>.ics`, `catalog::timetable::subscription`) carries the semester, the planned
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
  the server drops it) — never a list of what is planned.
- **R21. A click answers in the next frame** (2026-09-23, „A click answers first"). What the
  visitor starts goes through `Pending` — links do by themselves; a handler that navigates calls
  `Pending::go`, not the router's `navigate` (that is for what the app does on its own). A
  control that shows where the visitor is shows where the app is going as well (`Pending::to`,
  `search_on`, `path`), and what takes a new page or list to compute has its skeleton
  (`Pending::waits`, `skeleton`). A new page gets a `pending::Shape`, or one that looks like it.

## 3. Running it

```bash
./radix.exe serve-snapshot --addr 127.0.0.1:8090
```

```bash
bash scripts/build-client.sh --dev
```

```bash
cargo run -p folia-server
```

The first command builds the browser app into `site/pkg` (needs the `wasm32-unknown-unknown`
target and `wasm-bindgen` 0.2.128, which Trunk keeps in its cache); without it the site simply
stays server-rendered. `--dev` builds it with the `wasm-dev` profile and the flags of
`.cargo/config.toml` (`--cfg erase_components`, see „Build times"); leave it off to build the
bundle that ships, as Nix builds it. Which one you want is a question of minutes:

| | profile | edit a page, build again | `site/pkg` | gzipped |
|---|---|---|---|---|
| `--dev` | `wasm-dev`, `erase_components` | 11 s | 7.4 MB | 1.4 MB |
| (none) | `wasm-release` | 2 min 18 s | 19 MB | 1.4 MB |

`wasm-release` owes those two minutes to fat LTO, `opt-level = "z"` and its single codegen unit,
and all of it buys the megabyte that people who load the site once do not have to fetch. On
localhost the difference arrives over the loopback; never deploy a bundle built with `--dev`.

### Build times

The workspace is around 20 000 lines of Rust on 339 crates, and most of `folia-app` is Leptos
views (384 `view!` macros). What that costs on a laptop with six cores (Ryzen 5 PRO 4650U, twelve
threads, 16 GB), measured 2026-09-23:

| | without `erase_components` | with it (since 2026-09-23) |
|---|---|---|
| cold build, empty cache | 5 min 21 s | 3 min 24 s |
| new worktree: fork the main checkout's cache, then build | 19 s + 2 min 46 s | 19 s + 57 s |
| edit a page in `app/` (the catalog's filter panel), `cargo build` | 19 s | 10 s |
| edit `app/src/ui.rs`, `cargo build` | 14 s | 9 s |
| edit `server/`, `cargo build` | 19 s | 6 s |
| edit a page, `scripts/build-client.sh --dev` | 15 s | 11 s |
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

It is a flag for rustc and not a feature, so it is one of the flags `scripts/build-cache.sh`
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
git config core.hooksPath scripts/hooks   # once per clone: git does not carry it along
bash scripts/build-cache.sh setup         # in the main checkout, and after `rustup update`
bash scripts/build-cache.sh prime         # in the main checkout, after a merge into master
bash scripts/build-cache.sh gc            # drop the caches of worktrees that are gone
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
`scripts/hooks/post-checkout` with the null commit as the previous HEAD, and the hook runs
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

With the cache outside the checkout, `scripts/build-client.sh` asks `cargo metadata` where the
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
`/assets/app.css`, `/assets/icons.svg` (the sprite every icon points at), `/assets/favicon.svg`, `/sw.js` (the service worker with the build written in),
`/assets/og.png`, `/favicon.ico`, `/apple-touch-icon.png`, `/assets/icon-192.png`,
`/assets/icon-512.png`, `/assets/icon-maskable-512.png`, `/manifest.webmanifest`,
`/cards/module/<id>.png`, `/cards/program/<slug>.png`, `/robots.txt`, `/sitemap.xml`, and
`GET`/`POST /access` (the login page of closed testing). The stylesheet and the scripts answer
under any `?v=<build>` as well (the page links them so, see Offline). Every answer carries the
header `x-build` with the build of the process (version and start time, as in `/api/status`).

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
  may fetch. Tests in `server/src/tests.rs`:
  `closed_testing_asks_for_the_password_before_anything_else` (what passes and what not),
  `a_studyplan_is_a_calendar_feed` (the gated feed with a snapshot), `broken_calendar_codes_are_404`,
  `the_log_keeps_no_calendar_code`, `calendar_services_may_fetch_feeds`; the shapes in
  `catalog/src/timetable/subscription.rs`.
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

### Load (2026-09-26)

Owner: „wie viel Traffic meine Seite aushält … von Crawlern, richtigen Benutzern, Benutzern ohne
JS und Abfragen von Google Calendar“, then „optimiere das Setup auf Performance … du kannst alle
Ressourcen des VPS verwenden“ (live and canary, each Folia and Radix, and the management around
them). Measured with `e2e/load` (below) in a lab on the workstation — Folia's release build as Nix
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

- **Places** (`server/src/busy.rs`): renders and calendar feeds run one per processor; a request
  that finds every place taken waits (at most `FOLIA_RENDER_WAIT_MS`, and never behind more than 64
  waiting per place) and is then answered **503 with `Retry-After`** — crawlers and calendar
  services come back later. Cached pages, files and `/livez` need no place.
- **One more worker thread than processors**: with one thread for everything, renders starved the
  accept loop and connections were refused before any request could be told 503.
- **The cache keeps pages compressed only**, drops views before the pages of the sitemap, and renders
  a page once however many ask for it at the same time (`server/src/cache.rs`). 128 MiB used to hold
  1,200 pages — not even the modules; the sitemap's 5,235 pages take 29 MiB now.
- **Warm-up** (`server/src/warm.rs`): after every new snapshot and after a start, every page of the
  sitemap is rendered into the cache while the server is idle — 22 s on one processor here, about a
  minute on the server — so crawlers walking the sitemap and visitors after a deploy meet no render.
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

**Running it** — `e2e/load` is a Go program without dependencies (`go build -o betula-load.exe .`
in that directory); `e2e/load/lab.ps1` starts the lab on Windows:

```bash
betula-load discover -base http://127.0.0.1:18080 -out pages.tsv -max 40000     # what a crawler finds
cargo run --release -p folia-catalog --features native --example loadtest_feeds -- <catalog-*.db> 2026W --heavy 20 > feeds.tsv
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

### Log events (same rules as `docs/operations.md` §2: ERROR = a human has to act)

| Level | `event` | Meaning |
|---|---|---|
| INFO | `server.listening`, `server.shutdown` | lifecycle |
| INFO | `snapshot.sync_started`, `snapshot.restored`, `snapshot.downloaded`, `snapshot.activated`, `snapshot.sync_recovered` | snapshot lifecycle (`etag`, `bytes`, `generation`; `schema_version` when activated) |
| INFO | `snapshot.map_built` | the map of the programs was laid out for a snapshot (`programs`, `links`, `ms`) |
| WARN | `snapshot.map_failed` | it could not be; the landing page goes without the map |
| DEBUG | `snapshot.unchanged` | Radix answered 304 |
| INFO | `http.request` | access log: `method`, `path`, `status`, `ms`, `cache` (`hit`/`miss`/`-`); every path under `/calendar/` is written `/calendar/….ics` (a code names somebody's plan) |
| WARN | `http.request` with `cache=busy` | a request turned away with 503 because every place was taken ("Load"): no error of the server |
| WARN | `server.busy` | the same, at most once a minute: `what` (`render`, `calendar`), `places`, `wait_ms`, `turned_away` since the start. Often: more processors, or a crawler to slow down |
| INFO | `cache.warmed` | the pages of the sitemap are in the cache (`pages`, `rendered`, `kept`, `ms`, `generation`) |
| WARN | `cache.warm_failed`, `snapshot.choices_failed` | the sitemap could not be listed for the warm-up / the pickers of the catalog are loaded per page again |
| DEBUG | `http.request` with `path=/livez` | the container's own probe, twice a minute |
| DEBUG | `calendar.served` | a calendar feed was made (`bytes`, `ms`; never the code or the modules) |
| INFO | `access.gate_on` | closed testing is on (`source`: where the password was found, never the password) |
| INFO | `access.granted` | the access password was entered |
| WARN | `access.denied` | a wrong access password (`failures` in this minute, `closed` when the form closed; at most ten lines a minute) |
| DEBUG | `card.drawn` | a link-preview card was drawn (`key`, `bytes`, `ms`) |
| WARN | `card.busy` | every drawing place was taken, previews got the standard picture (`count`; at most one line a minute). Often: more places or a larger `--card-cache-mb` |
| WARN | `snapshot.fetch_failed` | Radix unreachable or not ready; retried with backoff; the last snapshot stays active |
| WARN | `snapshot.restore_failed`, `snapshot.compress_failed` | stored snapshot unusable / served uncompressed |
| ERROR | `snapshot.rejected` | a download is not a usable catalog; the previous snapshot stays active |
| ERROR | `snapshot.outdated` | the active snapshot is of an older schema than this build reads (`schema_version`, `needs`): pages that need the newer columns fail, browsers do not start the app on it. Served all the same; Radix has to export a new one (with `RADIX_CRAWL=off` it never does by itself) |
| ERROR | `snapshot.stale` | no answer from Radix for longer than the limit |
| ERROR | `http.request` with `status >= 500`, `render.failed`, `snapshot.unreadable` | a request failed |
| ERROR | `card.failed` | a card's text could not be read or the card could not be drawn; the preview got the standard picture |
| ERROR | `calendar.failed` | a calendar feed could not be read from the snapshot (or its task failed); the calendar service got a 500 and asks again later |
| ERROR | `server.start_failed`, `server.failed` | the server cannot run |

## 4. Checks

```bash
cargo test
```

needs a snapshot (`snapshot/current.json` or `FOLIA_TEST_SNAPSHOT`) and fails without one.
Without a crawl, a synthetic one serves for development and for the browser checks below
(`internal/catalogbuild/folia_fixture_test.go`: 1,200 modules with varied facets, 115 programs
with trees, areas and degree labels, Informatik B.Sc. and Elektrotechnik B.Sc. with validated
plans, a few events — numbers made up, nothing of it says anything about the BTU; the checks
that compare pinned or real-data numbers fail on it, everything else runs):

```bash
BETULA_FIXTURE_DIR=$PWD/snapshot go test ./internal/catalogbuild -run TestWriteFoliaFixture -count=1
./radix serve-snapshot --dir snapshot          # then cargo run -p folia-server as usual
```

What `cargo test` checks:

- `pack` (needs no snapshot): every shape of serde's data model there and back, the codes of
  fixed values (the format is frozen), a field added at the end read from older codes, versioned
  codes (the layout in four bits, another layout named, never read as a plain code), what the
  format refuses; every character typed wrong, every swap of neighbours and of characters one
  apart is caught in codes of several lengths and kinds; codes that check out but hold garbage
  are refused without a panic, and without more work than their length allows.
- `catalog`: every filter against direct SQL (exclusions included), exact totals and paging, the
  pinned numbers, enum labels from the CHECK constraints, every query and page loader against
  real data, the URL codec, the ranking of the pickers (`fuzzy`); `SCHEMA_VERSION` is the number
  of Radix's newest migration, and the snapshot of the tests is not older.
- `server`: a fake Radix over HTTP: not ready → 503; download, check, gzip, activate; 304 →
  no download; pages render, cache (`hit`/`miss`), revalidate; equal filters share a cache key;
  404 is never cached; `/api/db` with Radix's ETag, gzip and 304; `/api/status` with the
  snapshot's schema, `boot.js` with the build's; a broken export is rejected and the old snapshot
  stays; one of an older schema is served; a new one invalidates pages; restart without Radix;
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
its scroll position), filters, closes with Esc, opens the full page, goes back, scrolls the
virtual list (as long as the whole list from the start, a slice rendered, the last rows there at
its end, `page` following, the length unchanged), searches programs, and fails on any page load
after the takeover or any console error.

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
cd e2e && node module.mjs
```

drives a module in its two sizes. Desktop: preview → page with the sidebar exactly where the
filter panel was, the same order of sections in both, jumps without history entries, one width
for sidebar and filter panel, Esc back to the list with the row in view. Phone: a tap opens the
page directly, the page starts with times and facts, back returns to the tapped row deep in the
endless list, a shared preview link becomes the page.

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
cd e2e && node ground.mjs
```

drives the ground on a wide screen with a real wheel: the window without a scrollbar and without
room while the page is not at its end (the wheel over the header or the rail moves nothing), the
ground coming up at the end with the rail and the header standing, the page's end and the panel
beside it 8 px above it, the panel cut off and not scrolled by the wheel, upwards the ground
leaving first, a short list bringing it at once without its rows moving, the end of the whole
virtual list going up with it, a question opened and a page opened from the ground sending it
away, Tab bringing it up; on a phone the ground at the end of the page.

```bash
cd e2e && node pwa.mjs
```

installs the app with the network (the worker has the shell, the bundle included; no status
pill once the app runs), then cuts the network and loads pages afresh: the catalog with a
filter, a module page never seen before, the program page seen before, and a step inside the
app out of the local catalog.

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
  started with). User data beyond „Merken": passed modules with the prerequisite check, „mein
  Studiengang". `wasm-opt` for the bundle.
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
- **The timetable as a calendar subscription** is built (2026-09-24): `/calendar/<code>.ics`,
  one semester of the Studienplan per code (semester, planned modules, hidden kinds, events and
  Termine, chosen Termine, the Standort; `catalog::timetable::subscription`), made anew from the
  active snapshot on every fetch. R20 has the owner's decision, §3 the gate and the log, „Der
  Studienplan" in §1 the rest; the privacy notice's part is „Kalender-Abo"
  (`app/src/pages/legal.rs`), which lists what a code carries. The note of 2026-09-23 (a code of
  event ids) is superseded: a code of modules and hide rules also brings the exams QIS publishes
  later. Since 2026-09-25 an entry is as short as a slot of the week, for the phone (owner: „so
  kompakt wie möglich … so wie die Infos bei der Ansicht auf der Seite"): „VL EvS" in „ZHG/HS.C",
  „Ü AuP · 1 von 3", „Prüfung EvS · 2. Termin"; its description says it all in full (what QIS calls
  the event, the modules with number and title, the rooms as QIS names them, and what the rows say;
  `catalog::timetable::export`). A program's abbreviations differ from a module's own in about 7 %
  of its compulsory modules, and the download must be the feed, so the code carries the program;
  and it names the layout of its fields in four bits (`subscription::VERSION`), so a later layout
  is read beside it. Codes of the time before are not read (canary only). Still open: the edge logs
  the address like every address, 7 days in Loki and 7 days in the host's journal
  (`deploy/README.md` §9); a Traefik router for `/calendar/` with
  `observability.accessLogs=false` would leave it out, but needs the blue-green priority label in
  `deploy/vps/50-app.sh`, `55-switch.sh`, `lib-stacks.sh` and `91-verify-stacks.sh`. And a
  subscription over several semesters: today the next semester needs a new address.
