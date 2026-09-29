# Schema v2: pipeline, tables and read views

> Deliverables 2–5 of `docs/backend-data-overhaul.md`. The rules behind the model are in
> `docs/data-sources.md`. State: 2026-09-19, first build from the re-crawled module pages.

## 1. Pipeline

```
crawl-qis-modules, crawl-modules, crawl-tree, crawl-events ──▶ raw_page ──▶ build ──▶ canonical tables ──▶ validate ──▶ export ──▶ serve-snapshot ──HTTP──▶ web server ──▶ browsers
      (network)                 (archive)   (no network, deterministic)       (gate)      snapshot/     ETag / 304
scan-curriculum ──▶ plan, plan_entry, plan_total  (validated PDF plans, a source of their own)
```

| Step | Command | Package | Notes |
|---|---|---|---|
| Archive pages | `radix crawl-qis-modules`, `crawl-modules`, `crawl-tree`, `crawl-events` | `internal/crawl`, `internal/catalogdb` (`raw.go`) | One row per page in `raw_page` (gzip body, `source_url`, `fetched_at`, `changed_at`, content hash). Polite: jitter, retries with growing pauses, abort after 10 consecutive failures, resume by `--max-age`. |
| Study plans | `radix download-statutes`, `radix scan-curriculum` | `internal/curriculumscan`, `internal/gemini`, `catalogdb/plans.go` | Validated plans are stored transactionally (`SavePlan`). The 140 plans of v1 were imported once; that importer is gone. |
| Build | `radix build` | `internal/catalogbuild`, `internal/normalize`, `internal/qistree` | Parses the archive and replaces all derived tables in **one transaction**; fails on any foreign-key violation. About 20 s for the whole catalog. A parser or normalization fix takes effect by building again, without the network. |
| Validate | `radix validate` | `catalogdb/validate.go` | Invariants (fail), source problems (warn), numbers (info), count baselines (fail below the minimum). Exit code 1 on failures. |
| Export | `radix export --out snapshot` | `catalogdb/export.go` | Refuses a database that fails validation. `VACUUM INTO` (a consistent copy that includes WAL frames), drops `raw_page`, rollback-journal mode, `ANALYZE`, `VACUUM`. Writes `snapshot/catalog-<hash>.db` and replaces `snapshot/current.json` atomically. Snapshots are never overwritten, because a reader may hold the previous one open. |
| Publish | `radix serve-snapshot --addr 127.0.0.1:8090` | `internal/snapshothttp` | `GET /snapshot/catalog.db` (ETag = content hash, `If-None-Match` → 304, Range) and `GET /snapshot/current.json`. **This HTTP endpoint is the only interface between Radix and Folia (the web server).** |

Database files: `radix.db` is the working database (archive + canonical, about 100 MB).
The snapshot is 34 MB (v1 shipped 50 MB). `btu_modules.db` (v1) is no longer written by
any v2 command.

Migrations live in `internal/catalogdb/migrations/NNNN_name.sql`, are applied in order inside
a transaction each, and are recorded in `PRAGMA user_version`. A gap, a duplicate number, a
failing statement or a database newer than the binary is an error. Every connection runs with
`foreign_keys = ON`. The snapshot keeps `user_version`, and Folia is built for the newest
migration: a new one raises `catalog::SCHEMA_VERSION` (a test of the `catalog` crate fails until
it does), browsers do not open a local copy of an older schema, and a Folia that serves an older
snapshot logs `snapshot.outdated` (docs/frontend.md, data flow).

## 2. Tables

Conventions: NULL means unknown (no `0`, `''`, `'-'`); every filterable attribute is an enum
(CHECK) or a 0/1 flag with the source text next to it as `*_raw`; everything except `raw_page`
and the `plan*` tables is derived and replaced by each build.

| Table | Content | Source |
|---|---|---|
| `raw_page` | latest body of every fetched page | all (not in the snapshot) |
| `module` | one row per module, normalized and raw columns; `description_source` says which page the fields come from | the QIS module description where QIS has one, else the copy on `b-tu.de/modul` (`docs/data-sources.md` §10); FÜS list for `is_fues` and as fallback for a module without a page (`detail_status = 'missing'`) |
| `department` | organisational units; German and English names of one unit are paired by unit code and shared responsible persons | module page |
| `module_person`, `module_teaching_form`, `module_text_item` | responsible persons, teaching forms with SWS/hours, literature and course lists | module page |
| `module_prerequisite`, `module_successor` | module IDs named in the prerequisite texts / the rows that state a replacement, on either of its two modules (`docs/data-sources.md` §14), only when the module exists | module page |
| `program` | one row per PO version. `id` = `<stg>-<abschl>-<pversion>` from the QIS node (`079-82-2008`), `slug` readable and unique (`bachelor-informatik-2008`), degree split into `degree_level`, `degree_type`, `study_variant` | QIS tree (the PO page's own breadcrumb, so the index pages above it are not needed) |
| `program_document`, `program_area` | statutes and amendments; the area tree below a PO with `section` and `stated_kind` | QIS tree |
| `module_program_ref` | every „Zuordnung zu Studiengängen" triple with `resolve_status` (`resolved`, `abroad`, `unresolved`) | module page |
| `program_module_assertion` | one row per statement "module M is in program P" per source, with `kind`, `kind_basis` (`stated`/`inferred`) and area | module page, QIS tree, validated plan |
| `plan`, `plan_entry`, `plan_scan_status` | validated study plans; written transactionally by `SavePlan`, never touched by the build; not foreign-keyed to derived tables, so a plan survives an incomplete crawl | statute PDFs |
| `plan_total`, `plan_total_entry` | the sums a regulation prints over the rows of its own plan, with the rows each counts. `scope` = `plan` (everything these semesters hold) or `section` (a named part); `is_choice` marks the sum that is the only statement of how much its rows count for. A sum is stored only where its rows reach it, so `credits` always lies between `min_credits` and `max_credits` | statute PDFs |
| `semester`, `event`, `event_form`, `event_person`, `event_date`, `module_event` | events keyed by semester (`2026S`, `2026W`), `category` (`teaching`, `exam`, `other`), `last_date` for the retention rule, campus per date; `event_date.room_short` is the room's short form („ZHG/HS.A“), `room` keeps the full name | the QIS event page where the QIS event search confirms it, else the newer of page and search entry (`docs/data-sources.md` §11); a reading that states nothing of the event is none, and an event without a reading, one BTU removed, is not built; the module page decides which events belong to a module; `room_short` by the build (section „Short names“) |
| `module_abbrev`, `program_module_abbrev` | the abbreviation of every module („AuP“), and of every module of every program, unique within the program; `is_override` (a line of the curated file), `choice` (1 = the first candidate; more = it fell back), `is_twin` (`-b`, `-c` after an identical title) | build, from the titles (section „Short names“) |
| `program_module`, `module_facet` | materialized results of `v_program_module_src` and `v_module_facets_src` (section 3) | build |
| `meta` | `built_at`, `current_semester`, `radix_version` (the Radix that built it, `internal/version`), `radix_build` (a hash of the binary that built it: a new release builds again at start), oldest/newest fetch and page count per source; `content_digest`, `data_changed_at` | build |

### Degree labels and new programs (decision Q4)

Nothing about a program is looked up in a hand-written list. Level, type and variant are parsed
from the German or English degree string by rules; an unknown string becomes `other` and the
raw text is kept. The short label (`B.Sc.`, `M.A.` …) is taken from what module pages say
(„Studiengang Informatik B.Sc.: …") when at least two mentions agree to 70 % and the label fits
the level; otherwise it is NULL and `degree_display` falls back to „Bachelor" / „Master". For
names offered both as a university and as an applied program, `*.Eng.` mentions only count for
the applied one. Today 47 of 149 bachelor and master programs have a stated label; Soziale
Arbeit, which v1 labelled „B.Sc.", has none.

### Membership and kind (decisions Q1, Q2)

- `relation = 'fues'`: the module is on the FÜS list, its page admits the program, and neither
  the program's tree nor its validated plan contains it. Everything else is `curricular`.
- `kind` is decided by the strongest statement: a stated value beats an inferred one, then
  `pdf_plan` > `module_page` (remarks) > `qis_tree` (a label on the area path). `precedence` is
  1–3 for stated, 4–6 for inferred statements. The tree's v1 default „Pflicht" no longer exists:
  without a label the tree states no kind.

## 3. Read views (the contract)

Consumers read only these. `v_*_src` views and base tables are implementation.

| View | One row per | Columns |
|---|---|---|
| `v_module` | module | `id, title, title_de, title_en, detail_status, page_lang, credits, language_raw, teaches_german, teaches_english, duration_raw, duration_semesters, turnus_raw, turnus_season, turnus_parity, offer_status, limitation_raw, is_limited, participant_limit, exam_form, exam_form_raw, exam_details, grading_raw, is_graded, is_fues, department_id, department, department_code, learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory, remarks, source_url, fetched_at, responsible, teaching_events, at_zentralcampus, at_sachsendorf, at_senftenberg, abbrev` (the module's abbreviation without a program) |
| `v_module_facets` | module | `module_id, credits, department_id, teaches_german, teaches_english, duration_semesters, offered_winter, offered_summer, turnus_season, turnus_parity, offer_status, is_limited, participant_limit, exam_form, exam_written, exam_oral, exam_paper, exam_presentation, exam_project, exam_practical, is_graded, is_fues, has_lecture, has_exercise, has_seminar, has_practical, has_project, has_excursion, teaching_events, at_zentralcampus, at_sachsendorf, at_senftenberg`. Campus flags are NULL (unknown) for a module without a room in the newest semester. |
| `v_module_search` | module × title variant | `module_id, term, kind` (`id`, `title_de`, `title_en`): the only place that needs `LIKE` |
| `v_module_lecturer` | module × person | `module_id, name, title, role` (`responsible`, `instructor`) |
| `v_module_teaching_form` | module × form | `module_id, ord, form, form_raw, workload_raw, sws, hours` |
| `v_module_text_item` | module × item | `module_id, kind` (`literature`, `course`)`, ord, text` |
| `v_module_prerequisite` | module × required module | `module_id, required_module_id, kind, required_title, required_offer_status` |
| `v_module_successor` | module × successor | `module_id, successor_id, successor_title` |
| `v_module_schedule` | module × event date, exams excluded | `module_id, semester_key, semester_label, event_id, event_number, event_title, event_type, ord, group_name, weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, room, campus, instructor, comment, cancelled_dates, source_url, room_short` |
| `v_module_exam` | module × exam date | `module_id, semester_key, semester_label, event_id, event_number, event_title, ord, weekday, start_time, end_time, first_date, last_date, room, campus, comment, source_url, room_short` |
| `v_module_program_link` | module × listed triple | `module_id, ord, degree_raw, program_raw, po_raw, resolve_status, program_id, program_slug, program_name, degree_display, po_version, is_latest_po, relation, kind, kind_source, area` |
| `v_program` | program | `id, slug, name, degree_level, degree_type, study_variant, degree_label, degree_raw, degree_display, po_version, po_year, po_amendment, family_key, name_key, is_latest_po, stg_code, abschl_code, source_url, fetched_at, has_plan, plan_validated_at, plan_status, curricular_modules, fues_modules, documents` |
| `v_program_module` | program × module | `program_id, module_id, relation, kind, kind_source, kind_basis, precedence, area, section, in_tree, on_module_page, in_plan, module_title, module_credits, offer_status, turnus_season, plan_semester, abbrev` (unique within the program) |
| `v_program_module_area` | tree placement | `program_id, module_id, area_id, area, area_label, depth, area_ord, section, kind, kind_basis` |
| `v_program_plan`, `v_program_plan_entry` | validated plan / plan row | layout JSON; `program_id, ord, module_id, module_code_raw, module_name, semester, start_semester, end_semester, semester_span, credits, min_credits, max_credits, kind, kind_raw, study_section, subject_area, area_rules, specialization, source_evidence, catalog_title, catalog_credits, credits_differ_from_catalog` |
| `v_program_plan_total`, `v_program_plan_total_entry` | a printed sum of a plan / the rows it counts | `program_id, ord, label, scope, specialization, start_semester, end_semester, credits, min_credits, max_credits, is_choice, entry_count, source_evidence` — `program_id, total_ord, entry_ord` |
| `v_program_version` | program × other PO | `program_id, other_id, other_slug, po_version, po_year, is_latest_po` |
| `v_program_counterpart` | program × Bachelor/Master counterpart | `program_id, counterpart_id, counterpart_slug, counterpart_name, counterpart_level, counterpart_po_version, match_score` (take the highest) |
| `v_program_document` | program × document | `program_id, ord, title, doc_type, url` |
| `v_department` | department | `id, code, label, name_de, name_en, modules` |
| `v_semester` | semester | `key, season, year, label, starts_on, ends_on, is_current, teaching_events, exam_events`. `is_current` follows `meta.current_semester`: the calendar decides (April–September summer, October–March winter), but a semester whose schedule is already published wins over it — a semester counts as published once 100 modules have a dated teaching event in it, so that the few events BTU releases early cannot move the catalog. |
| `program_coverage` | program | `program_id, program_name, degree, po_version, tree_modules, page_modules, plan_entries, plan_entries_linked, modules_without_kind, plan_status` |
| `v_meta` | key | `key, value` |

`semester_key` and `semester_label` of `v_module_schedule` and `v_module_exam` are NULL for an event
whose semester Radix cannot read. `validate` warns when a module links one, and Folia leaves its
dates off the module page (2026-09-27: two events QIS had emptied were built so, and Folia could not
read the pages of their modules; such an event is no longer built, `docs/data-sources.md` §11).

Every view is exercised by `internal/catalogbuild/build_test.go` against a miniature catalog
with the markup of the live pages. Measured on the real data, each of the queries below takes
about 1 ms in SQLite.

## 4. Which view replaces which frontend query

`frontend/src/db.rs` is not changed by this work. This is the map for the rewrite.

| Today (`frontend/src/db.rs`) | Replacement |
|---|---|
| `get_total_count` | `SELECT COUNT(*) FROM v_module_facets WHERE …` with the same filter as the list, without `LIMIT`, so the header is exact |
| `get_all_study_programs` | `v_program` (`slug`, `name`, `degree_display`, `po_version`, `is_latest_po`) |
| `query_filtered_modules` | `v_module_facets f JOIN v_module m` for cards. Program filter: `JOIN v_program_module pm ON pm.module_id = f.module_id AND pm.program_id = ? AND pm.relation = 'curricular'` (or `IN ('curricular','fues')`). Semester in program: `pm.plan_semester`. Kind: `pm.kind`. Turnus: `offered_winter`, `offered_summer`, `turnus_parity`. Teaching forms: `has_*`. Duration: `duration_semesters`. Exam: `exam_form`, `exam_*`. Graded: `is_graded`. Limitation: `is_limited`, `participant_limit`. Campus: `at_*`. Language: `teaches_*`. Department: `department_id`. Lecturer include/exclude: `EXISTS (SELECT 1 FROM v_module_lecturer …)`. Text search: `v_module_search`. |
| `extract_module_ids`, `evaluate_prerequisites` | `v_module_prerequisite` (IDs are extracted once, at build time) |
| `get_module_detail` | `v_module` + `v_module_teaching_form`, `v_module_text_item`, `v_module_successor`, `v_module_lecturer` |
| `get_module_events` | `v_module_schedule` (recurring) and `v_module_exam` (exams, shown separately) |
| `get_linked_programs` | `v_module_program_link` |
| `get_curriculum_entries` | `v_program_plan_entry WHERE module_id = ?` |
| `get_autocomplete_suggestions` | `v_module_search` |
| `get_study_program_detail` | `v_program` + `v_program_document` |
| `get_study_program_all_regulations` | `v_program_version` |
| `get_study_program_counterpart` (name heuristics) | `v_program_counterpart ORDER BY match_score DESC LIMIT 1` |
| `get_study_program_curriculum_modules` | `v_program_module` (+ `v_program_module_area` for grouping by area) |
| `get_verified_study_plan` | `v_program_plan`, `v_program_plan_entry` |
| `get_all_departments` | `v_department` |

## 5. Numbers after the first v2 build (2026-09-19)

The module re-crawl fetched 4,904 pages in 16 m 38 s (4 workers, 500 ms ±30 %), 2 were fresh in
the cache, 2 answered 404 (12690, 14037: on the FÜS list, no page), 0 failed.

| | v1 | v2 |
|---|---|---|
| Modules with program assignments from English pages | 0 of 979 | 670 |
| Module page assignments | 19,897, 76 dropped, 1,565 Lehramt links wrong | 30,191, 0 unresolved, 1,926 „Abschluss im Ausland" kept as such |
| Grading known | 0 | 4,906 (4,623 graded, 283 ungraded) |
| Exam form / exam details | lost | 4,345 with a form, 4,905 with details |
| „Übung" facet | 0 of 1,535 | 1,898 (German and English) |
| Plain winter semester | substring guess | 1,895 |
| Pairs whose sources disagree on the kind | 438 | 43, all listed by `validate` |
| Informatik B.Sc. (PO 2008) | 207 vs 108 modules, depending on the page | 109 curricular + 116 FÜS, one answer |
| Foreign-key violations | 2,063 | 0 (build and export refuse to write any) |
| Departments | free text in two languages | 14 units, every English name paired |

`validate` currently warns about: 2 modules without a page, 3 programs without any tree module
(Hebammenwissenschaft, Pflegewissenschaft, Physiotherapie, all PO 2026), 43 kind conflicts. It
reports as information: 4,712 curricular pairs without any stated kind, 415 pairs only the
module page states, 109 only the tree states, 25 FÜS modules whose page admits no program.

## 6. Done since the first build, and what is still open

Done (see `docs/operations.md`):

- **Service mode.** `radix run` replaces the v1 refresher: rolling polite crawl of lists, module
  pages, QIS tree and events, retention, build, validate, export, and the HTTP endpoints
  (`/snapshot/*`, `/healthz`, `/status`) in one process.
- **Snapshots only change with the content.** The build computes a digest over everything a reader
  sees (without `meta` and the `fetched_at` columns). An unchanged digest means no export, so a
  refetched but unchanged page does not make every browser download the database again.
  `meta.data_changed_at` says when the content last changed.
- **Retention.** Events are removed one month after their last date (`prune`, and in every
  service cycle) and remembered in `event_tombstone`, because module pages keep linking them. An
  event BTU removed is not built at all; its page and search entry are unused archive pages, which
  the `archive` stage removes a week after nothing fetches them any more (2026-09-27).
- **QIS tree complete.** `crawl-tree` fetched the 152 missing index pages; a walk from the root now
  reaches all 2,652 pages, so new programs and PO versions are discovered.
- **Events archived** by `crawl-events` (section 8).
- **Structured logging** with stable events in every stage.
- **Nix**: `flake.nix` builds the static binary and a container image.

Also done: **the v1 code is gone** (`internal/storage`, `web`, `refresher`, `provider`, `cache`,
`analytics`, `config`, the old logger and all v1 commands). `scan-curriculum` and
`download-statutes` work on this database (`catalogdb.ScanPrograms`, `ScanCatalog`, `SavePlan`);
the Gemini path was run against the API on 2026-09-19 (Informatik B.Sc., dry run: 23 entries, 9
linked, identical to the stored plan). The API key moved from `config.yaml` into the secret
sources of `docs/operations.md` §3.

### What a plan adds up to (2026-09-22)

A plan row may print a range instead of a number: Informatik 2008 asks for „Komplex Grundlagen
der Informatik, 10-24 LP" three times. Adding the lower bounds made that degree 166 LP instead of
180, and its last two semesters — printed as one merged column — had no sum at all. The
regulation does state the answer, in the lines it prints over its own rows: „Summe Komplexe des
Fachstudiums 44" over exactly those three rows, and „Summe Studium 32 28 30 30 60" over the whole
table. Those lines were read and used to validate the extraction, and then thrown away.

`plan_total` keeps them (`internal/gemini/plan_totals.go`). A plan is printed like an account
sheet, so a line sums the rows between it and the line before it, and where that does not add up,
the line before it as well — „Summe Grundstudium" stands over three „Summe Komplex …" lines. The
binding is kept only where the rows reach the printed value: exactly, or within the range their
own budgets leave open. A sum nothing explains is dropped rather than guessed at, which `validate`
checks again over the stored rows.

A module over several semesters belongs to none of them alone, so it only raises the upper bound
of a semester it reaches into — the same arithmetic the validation of the printed totals uses. A
semester whose modules all reach into it that way names no row (`entry_count` 0); its printed sum
is kept all the same, because it is the only thing the plan says about that semester.

What this gives a reader: the credits of a program are the plan's own sums (180, not 166), a
semester printed as a merged column has its figure, and a row with a range names the rows it is
chosen with and what they come to together.

A „Summe Aufwand" line is not one of them: it counts the work of a semester, not the credits
booked in it, and a plan may print both with different numbers (Elektrotechnik 2022 prints 27 and
24 for its second semester). The reader keeps the credit line.

Three ways of printing a sum took their own reading:

- **A sum over the compulsory modules with a budget under it.** Städtebau und Stadtplanung 2019
  prints „Summe LP (Pflichtmodule) 27 33 33 27 24 24" and, beneath it, „+ Wahlpflichtmodule (12 LP
  müssen insg. belegt werden) … +6 +6". The elective rows above are the choices that budget stands
  for, not requirements of their own, and the „+" is the plan's own arithmetic: the fifth semester
  holds 24 + 6 = 30 LP. Adding every row instead made that degree 228 LP; it is 180.
- **A plan of panels.** Medizininformatik 2016 prints one miniature table per semester with its
  own „LP 28" beside it (`pdf_panels.go`). There is no row order to walk, so each sum is bound to
  the requirements of its own panel directly.
- **A sum that is itself a span.** Angewandte Mathematik 2019 prints „28 - 32" where other plans
  print a number, because several of its rows are budgets. Both sides of the check are then
  intervals and have to meet rather than to be equal; `credits`/`credits_max` keep what was
  printed. That degree states 116–126 LP, which is what the page now says — adding the rows' lower
  bounds said 110.

### Plans the reader passed over (2026-09-22)

Two documents held a study plan the reader would not take:

- **Elektrotechnik dual (ausbildungsintegrierend), PO 2022.** Its Anlage b.2 heads one table with
  „im dualen praxisintegrierenden **und** im dualen ausbildungsintegrierenden Studium" and tells
  the two apart inside the table. The heading was read as the first of the two modes it names, so
  both tracks were labelled „praxisintegrierend" and the program's own plan was dropped as the
  wrong variant. A title naming both now names the dual study as such, and a track that names its
  own mode keeps that name (`dualMode`, `selectProgramMode`).
- **Physics M.Sc., PO 2021.** Its plan is one box per semester column, with „Specialization
  Phase" and „Research Phase" printed over the columns and a line labelled „Leistungspunkte"
  instead of „Summe". Both were read as modules that had lost their credits, and the document was
  sent to review. A row of boxes that names no credits anywhere is now a caption — kept as the
  `study_section` of the modules under it — and the credit line is read as the sum it is. A box
  that points at its credits („Entwurfsprojekt 1 (Gemäß Anlage 1, Nr. 1)") stays a requirement
  even where the appendix was not read, so nothing disappears into a caption.

Of the 182 program versions, 139 then had a validated plan.

### Where a plan stands in its regulation (2026-09-23)

A regulation is dozens of pages of legal text with the Regelstudienplan somewhere in an appendix,
and a Lesefassung may print four of them, one per study branch. `plan.source_file` named the
document but not the place, so nobody could check what Betula shows without leafing through the
PDF.

`plan.source_pages` now names the pages the plan stands on, written as a reader would („9",
„9–11", „9, 13"), `plan.source_label` the heading it stands under, and `plan_entry.source_page`
says it per row, because a plan continued across a page break has rows on both. All three come
from the cell each row was read from — the page is where the PDF drew the box and the label is the
heading the reader already binds the table to — so a plan whose pages were not recorded says
nothing rather than guessing.

The program page prints it after the provenance line: „… geprüft am 23.09.2026. Dort auf Seite 11,
unter „Dual ausbildungsintegrierend · Regelstudienplan …"." A heading that only reads
„Regelstudienplan" is left out, because the page is already under that word.

### Nine more shapes a plan is printed in (2026-09-23)

Each of the 29 documents still without a plan was opened and read. All 29 hold one, and 25 state it
without anything having to be guessed — what was missing was a reading, not the source. Nine of
those readings are now in, taking the corpus from 139 to 161 of 182. Each is refused where the
document does not verify it:

- **A semester column headed by where the semester is spent** („1 ECN", „2 UNIZG", „3 BTU",
  „4 Thesis"). The number decides the column as a bare number does, and the label is kept.
  Read only where a table has no plain semester header at all, and never where the label counts
  something else — „1. Studienjahr" spans two semesters, and reading it as the first would put
  every module in the wrong semester *and still add up*, the one error `ValidateCurriculum` cannot
  catch (`parseSemesterHeaderSite`, with its own negative test).
- **A plan that points at another Anlage** for a block of its semesters („1. bis 5. Fachsemester
  analog zu Anlage 2.1"). The referenced Anlage prints those semesters in full in the same
  document; text and cell geometry must name exactly the same span, and the referencing plan's own
  „Σ = 180 LP" line verifies the copy semester by semester (`pdf_annex_refs.go`).
- **A choice printed as two named blocks** („entweder" · block with its own sum · „oder" · block).
  A bare „oder" chain cannot read this because the branch headings end it. Read from the opening
  „entweder" line instead, and only where each branch's rows reach the sum its own heading prints
  (`entwederBlocks`, `blockReachesItsSum`).
- **A grey „möglicher Studienplan" that leaves a row unpainted.** Grey chooses between the
  placements a row offers; a row it painted nothing in offered no choice. Such a row is added back
  only where the printed totals ask for it: the grey reading must contradict a total, adding
  exactly the unpainted rows must reach every contradicted total exactly, and no total the grey
  reading already explains may move (`unpaintedRowsThePrintedSumsAskFor`).
- **Several study options in one document**, each under its own Anlage („Präsenzstudienprogramm",
  „Fernstudienprogramm", „Doppelabschluss"). The tables look alike, so only the heading above them
  says which program version a plan belongs to (`studyOptionOf`, `selectStudyOption`).
- **A module box naming several modules**, each pointing at the appendix row its credits stand in,
  with „oder" between two making them alternatives; and a box that explains the plan rather than
  requiring anything of it (the footnotes under a total line) is no longer read as a requirement.
- **A footnote that makes blocks alternatives** („Ein Schwerpunkt ist zu belegen").
- **A semester column captioned on a second ruled header row** — a run of bare numbers with the
  word they count under each. The pairing must be complete, so it is the document's and not the
  reader's.
- **A box plan closing its columns with bare amounts** („30 LP  30 LP  30 LP  30 LP"): the
  semester sum, but only where every column holds exactly one box and each holds nothing but a
  credit value.
- **A module box pointing at a whole appendix** („WP-Modul (gemäß Anlage 6)") rather than one of
  its rows: worth what every module listed there is worth, and only where they all carry the same
  value. A box drawn as a shaded paragraph is joined back from the one cell per printed line the
  ruling cut it into.
- **The one figure a plan prints that its semesters leave out.** A doctoral thesis spanning every
  semester belongs to none of them, so „Summe 8 6 8 6 2 30" leaves it out while the per-row total
  column says 180. That column is read only where the table proves it twice: each row's value in
  it equals the sum of that row's semester cells, and the rows add up to what the total row prints
  there (`appendGrandTotal`).
- **An „oder" at the end of a module name** („31205 Strömungslehre oder" over „43205 Technische
  Hydromechanik"). It does not mean what an „oder" on a line of its own means: that one separates
  blocks which may each hold a requirement per semester, so the choice is made per column, while
  this one makes each alternative exactly one row, so the choice is decided once for the group
  however its rows are spread (`inlineOderRows`, `AltOne`).

Three readings were built and then **not kept**, because a change that unlocks nothing is not
worth its risk: a rule for the inset semester headers of Stadt- und Regionalplanung 2016 (it
altered no plan anywhere in the corpus, and that document's real obstacle is „6(1+2)", six credits
split across two semesters); a loosening that would let a single column count as a semester header,
the shape most likely to produce a false plan; and a second reading of the per-row total column,
whose one document `appendGrandTotal` already explains.

One reading was kept only after the corpus caught a fault in it. Joining a shaded box back
together walked the grid by column and took its width from the first row, but a table's rows need
not be equally long: it read past the end of a shorter row and panicked, taking two working plans
down with it. The gate showed +2/−2 where the feature alone was +2; without the full-corpus pass
that would have shipped.

What the remaining 21 need is still a different kind of reading: plans that mark a semester with
„X" and print the credits in a block column (Environmental and Resource Management), a transposed
matrix whose credits stand in an annex, a plan of module boxes with no label column at all,
documents that publish only an amendment, and the four documents whose plan cannot be verified
from the document alone — which, by R12, is a reason not to store it, not a reason to guess.

### What the model does not decide (2026-09-23)

The corpus gate reads offline, so the full Gemini scan of all 182 program versions was set beside an
offline scan of the same database: 161 plans each, with the same totals, pages, credits and links.
Two differences came from the model, and `BindSourceCells` now takes both from the document:

- **The order of the rows.** The binding kept the order of the model's answer and appended the
  cells it had left out. Where the model left out the rows the study directions share, they
  („Höhere Mathematik T1", first semester) stood after „Wahlpflicht-Modul 4"; where it answered the
  second table first, `plan.source_label` came from that one (211-88-2021: „Regelstudienplan"
  instead of „Studienplan · Seite 12"). The rows now stand in the order of the layout's cells, the
  order the offline reader gives them (211-82-2021, 211-88-2021, 216-82-2022, G19-P2-2022,
  879-88-2018).
- **The variant a row belongs to.** The model's `specialization` survived wherever the document
  prints one plan and no track, and Folia splits a plan into variants by it: Wirtschaftsmathematik
  (276-82-2023, D02-P2-2023) grew a variant „Komplex Vertiefung" of two rows. It now comes only
  from the headings of several plans or the track printed over an alternative.

Bound again with the change, all 164 stored model answers give the offline order, variants and
heading (seven did not before), so the seven programs read with `--offline` as a stopgap can be read
with the model again.

A thesis and the FÜS take their kind from their name. The thesis
matters beyond its row: a program's faculty is the department of its thesis module
(`catalog::pages::faculties`), and the model called 105 of 199 Bachelor and Master theses „Pflicht"
— Elektrotechnik B.Sc. 2022 and its dual variant lost their thesis and moved from MINT to Fakultät 3
in the program overview. `ClassifyRequirement` now wins for these two kinds, and both of its rules
name the thing instead of mentioning it: „PhD Thesis Writing Skills" and „Status Seminar ERM:
Progress Reports PhD Thesis" are courses, „Fachübergreifende Projektarbeit" is a module of its own.
Bound again with that, every program has its faculty of 2026-09-21 back, and G02-P2-2022 joins its
two Elektrotechnik siblings in MINT. `validate` counts 104 pairs whose sources state different kinds
(99 with the model's kinds on all 161 plans, 157 offline before the narrower rules and 152 after):
the thesis now agrees with the QIS tree in 6 more pairs and disagrees in 12 more with the module
pages and the parts of the tree that call it „compulsory".

An internship takes its kind from its name as well, now that its rule names one. The old rule took
every name with a word ending in „praktikum", and 162 of those cells are no internship: 119 Lehramt
modules with a school practicum in them („Fachdidaktik Mathematik (beinhaltet fachdidaktisches
Tagespraktikum, fTP)"), 40 lab courses („Programmierpraktikum", „Laborpraktikum der
Elektrotechnik", „Werkstofftechnik 2 mit Praktikum", „Physikalisches Praktikum I", „Praktikum
Maschinelles Lernen"), two choices („Proseminar oder Praktikum") and „Projektpraktikum
Medizininformatik", a name BTU also gives to labs. An internship is named by where it is served or
by what it is in the degree: Berufs-, Berufsfeld-, Betriebs-, Industrie(fach)-, Ingenieur-, Pflicht-
and Bachelor-Praktikum, „Außeruniversitäres Praktikum", Praxisphase, Praxismodul, internship and a
bare „Praktikum". The regulations decide the doubtful names: an Integrationspraktikum is 800 hours
in an administration, a company or an institution, a Forschungspraktikum 18 weeks at a research
institution („ein Pflichtpraktikum"), a Wirtschaftspraktikum, „Praktikum Maschinenbau" and
„Praktikum Wirtschaftsingenieurwesen" are served in a company, and the dual „Praxis Musikschule
(Praktikum Dual)" at the partner music school. „Praktikum Maschinenbau" is matched by the program it
names, because „Praktikum Medientechnik" is a lab.

On the 161 saved plans, 64 cells become internships that the model called „Pflicht" (52), gave no
kind (11) or „Wahlpflicht" (1: Elektrotechnik M.Sc. 2026's „Praktikum / Praxisphase", the slot for a
Forschungs- or Industriefachpraktikum). `validate` still counts 104 pairs: Soziale Arbeit dual's
„Praxismodul 1–6" now agree with the QIS tree, and six internships disagree with the module pages or
tree nodes that call them „compulsory". The old rule would have given 127. Offline, where the rule
decides alone, 160 cells are no longer internships and the count falls from 152 to 128. No program
changes faculty on either path. The model and the offline reader now differ on 5 of 138 internship
cells instead of 227 of 297; „Betriebliche Phase 1" and „Schulpraktische Studien / Praktisches
Studiensemester" do not say what they are in their name and stay the model's.

### One issue, two regulations (2026-09-23)

Materialchemie B.Sc. and M.Sc. 2018 had the same stored plan. Both regulations stand in one issue of
the Amtliches Mitteilungsblatt (17/2018: the Bachelor's on pages 2–5, the Master's on pages 6–10),
and the reader took the tables of the whole document, so the Bachelor had a Master-Arbeit and the
Master the Bachelor's first two semesters. Eight documents of the corpus print regulations of two
degrees, and three more plans were mixed the same way: Künstliche Intelligenz B.Sc. 2022 carried the
Master's plan as a second variant, Künstliche Intelligenz Technologie B.Sc. 2022 had it among its own
rows (35 of them, 300 LP, both tables being headed „Regelstudienplan"), and Bauingenieurwesen M.Sc.
2014 carried nothing but the three plans of the Bachelor.

A program now reads only the pages of its own regulation (`pdf_regulations.go`). The cover of every
issue lists each regulation with the page it begins on, and that list decides. The running footers
name the regulation too, but 07/2014 prints „Master-Studiengang" under two pages of the Bachelor's
plans, which would have handed Bauingenieurwesen M.Sc. one of them. The contents count only where the
pages agree — each regulation begins on a page that prints the number the contents give it — and a
document whose regulations cannot be told apart, or that has none for the program's degree, goes to
review. Reading the pages rather than sorting the tables afterwards also keeps the regulations'
appendices apart: both have an „Anlage 1", and a box pointing at „Gemäß Anlage 1, Nr. 1" could have
found the other one's row.

The Bachelor's plan was also read only in part. Its Anlage 3 prints „a) Grundstudium" under „1.
Semester" and „2. Semester" and continues with „b) Fachstudium" under „3." to „6. Semester", and a
box plan was read only where its columns began at the first semester. A box table may now begin
later where it is the rest of the plan read just before it — that plan ends at the semester before
its first column — and its own „Summe LP" line verifies it like any other; one that begins after a
gap is refused. Materialchemie B.Sc. now has 24 rows over six semesters and 180 LP, the modules of
its Anlage 1.

An offline scan of all 182 program versions before and after the change differs in exactly these
five plans. Materialchemie B.Sc. goes from 20 rows to 24 (the ten of the Master out, the fourteen
of its Fachstudium in), Materialchemie M.Sc. from 20 to 10, Künstliche Intelligenz B.Sc. from 32 to
21 and Künstliche Intelligenz Technologie B.Sc. from 35 to 23; each is now one plan, of 180 LP for a
Bachelor and 120 for the Master. Bauingenieurwesen M.Sc. 2014 has none: its own plan is a box plan
pointing into its Anlage 1 that the reader cannot read yet, so 160 plans are valid instead of 161,
and the refusal names the pages it read. The model is given the same cells as before for 159 of the
164 stored model answers; the answers for the four others bind to their new layouts in the offline
order and variants. No program moves to another faculty: each keeps a thesis of one department.

A rescan does not remove the Bachelor's plans stored for Bauingenieurwesen M.Sc. 2014, because a
plan that does not validate never replaces a stored one; until its own plan can be read, they have
to be deleted by hand.

### Short names (2026-09-25)

The Studienplan's week grid, its agenda lines, notes and legend have room for ten characters, not
for „Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus“ or „Elektrische und elektronische
Grundlagen der Informatik“. Schema 9 (`0009_short_names.sql`) adds a short form of both, derived by
every build; the full name stays where it was, for the tooltip and the detail view. No source states
either of them (`docs/data-sources.md` §5.14), and the owner accepted that they follow the catalog:
where a form does not fit how a program uses it, the metadata is simply built again („dann machen wir
die meta eben neu“). **Folia never stores one**: it keeps module numbers and rooms and reads the short
form from the current snapshot.

**Rooms** (`normalize.RoomShort`, `event_date.room_short`). The form is `<building>/<room>[<attachment>]`:
`ZHG/HS.A`, `VG1C/0.07`, `LG3A/324`, `LG10/211a+b`, `SFB/14C.103`, `SD/7.116`, `Mensa/0.33.1`, `GHS`
(the owner, 2026-09-25: „ZHG/HS.C“ rather than „ZHG HS.C“, „weil sich das viel besser liest“). The
slash is the only one in a form: two rooms QIS writes as a pair or a range are joined by `+`
(2.26/2.27 → `FZ3E/2.26+27`, 229/230 → `ZB2CD/229+230`, 211a/b → `LG10/211a+b`), the Lehrgebäude
4/1, 4/3 and 4/4 of Campus Nord are `LG4-1`, `LG4-3`, `LG4-4`, and a building name the table lacks
gets a hyphen for its slash; a room with words keeps its spaces (`ZB2CD/AT Oestreich M`). The building
token comes from a table of 38 QIS building names, taken from the legend of BTU's campus plan (October
2021) and the owner's decisions (ZHG, GHS „damit es nicht mit HG verwechselt wird“, VG1C). On the
campuses Senftenberg and Sachsendorf the room number already starts with its building, so the token
is the campus code. The rules, in order: 13 curated places (the outdoor places, the eAssessment room);
a name without a building part keeps its text (with `SFB/`, `SD/` in front on those campuses); SFB/SD keep
the number and drop the description („Feld 2“ of the sports hall stays as ` F2`); a hall that is a
building of its own is the token alone (GHS, HS3, LH3D, SH1: no slash); `Hörsaal X`, `Seminarraum N` and
`Audimax N` become `HS.X`, `SEM.N`, `AM.N`; ateliers keep „AT“ and their name; otherwise the printed
number and what tells rooms with one number apart. An unknown building keeps its name as QIS spells
it — no acronym is invented — and the build logs `build.rooms_unknown_building`. Two rooms that would
share a form both keep their long form (`build.room_short_collisions`). On the data of 2026-09-23
all 232 rooms events use get a form of their own, 223 of them with 12 characters or fewer; the median
is 9 characters, against 52 for the full string. `internal/normalize/testdata/rooms.tsv` holds all
505 rooms of QIS's room list and the three more that events name, with their forms.

**Modules** (package `internal/abbrev`, `module_abbrev`, `program_module_abbrev`). Every title gets
a ranked list of candidates with a cost: word initials, the initials of compound parts
(Betriebs|systeme → BS; the splitter learns its words from the catalog's own titles, and before a
head that ends many compounds it also takes a part it does not know: Deponie|technik → DT,
Mehrgrößen|regelung → MR), the parts of a word written in parts (CampusTV → CTV), a known form (BWL,
SW in SWP), function letters (Algorithmieren und Programmieren → AuP), a generic opening or a
trailing phrase left out (Elektrische und elektronische Grundlagen der Informatik → EEG), the first
letters of one word, an acronym the title states for itself („(GIS)“), subtitle forms and longer
forms. Three characters are the sweet spot; a series number is appended (BS1, MIT1, DaF-B1.1). A
lowercase letter between two capitals reads as a function word, as the u of AuP does, so a word's
second letter in that place costs 0.5 more: Numerische Mathematik is NMa, not NuM, and Effiziente
Algorithmen EAl, not EfA next to SfA „Statistik für Anwender“. Three characters stay, not two
initials (M1 in `docs/data-sources.md` §13, the owner on 2026-09-25: „CFi sagt sich viel besser als
CF“): a form students can say beats a shorter one they cannot (§5.14 there).

**Three initials of the whole title come first** (the owner, 2026-09-25: „wenn die Buchstaben beim
Anagramm passen, dann nimmt man die i. d. R.“). Where the initials of all words of the head make
exactly three characters — a content word as its capital, a function word as the lowercase letter it
leaves (und u, von v, der/die/das/des d, für f, in/im i, mit m, zu/zur/zum z, an/am/auf/aus a, of o,
the t …; English „and“ as &, M5, and „&“ in a German title as u) — that form is the first choice,
ahead of compound parts and every other derived form: Entwicklung von Softwaresystemen is EvS, not
ESS from Software|systeme; Grundlagen der Werkstoffe GdW, Ethik und Handeln EuH, Kommunikation und
Lernstrategien KuL, Mathematics of Engineering I MoE1. A lowercase letter only ever stands for a
function word between two capitals, so the head has three words and the first and last are content
words (three content words give their plain initials). A hyphen part is a word of its own, but a
head „X- und Y“ is written as its terms (below); the series number is appended as everywhere; a head
with an acronym or a slash group is left to the other forms, and so is a sibling (its subtitle tells
it apart). In the module's list the form stands at the top, one hundredth ahead of
the cheapest other candidate, and its matching score is a class of its own (below). An override line
and an acronym the title states for itself („(GIS)“) still come before it, and the blocked forms, the
reserved forms and the uniqueness within a program hold: two titles of one program with one such form
(Grundlagen der Werkstoffe and Grundlagen der Wirtschaftsinformatik, both GdW) contest it like any
other form.

**„X- und Y“ by its terms** (the owner, 2026-09-25: „Wenn man Wörter mit einem Bindestrich
verbindet, dann sollte das Füllwort (und) wegfallen und da eher die kanonischen Begriffe verwendet
werden. Also z. B. SST.“). In a head of three words „X- und Y“ (also oder, &) X- is a compound cut
short (the tokenizer marks a word that ends in a hyphen) that shares its tail with Y: Signal- und
Systemtheorie is Signaltheorie und Systemtheorie. The und drops, and X, the part of Y before its tail
and the tail give a capital each: SST, Kinder- und Jugendhilfe KJH, Staats- und Verwaltungsrecht SVR,
Arzt- und Medizinrecht AMR, Kolben- und Strömungsmaschinen KSM, Arbeits- und
Beschäftigungssoziologie ABS, Bau- und Stadtbaugeschichte 1 BSG1. The tail is Y's last compound part
as the splitter finds it (…theorie, …hilfe, …recht, …maschinen, …soziologie, …geschichte), and what
stands before it is one term (Stadt|bau|geschichte: Stadtbau, S). The form takes the place and the
class of the three initials (`hyphenTerms` in `internal/abbrev/candidates.go`). Where the splitter
cannot take Y apart, its tail is unknown and the head keeps the three initials (Medien- und
Kultursemiotik MuK: „Semiotik“ is no word of another title; an override line can give it MKS). After a
word cut short no form inserts a function letter: when SST is taken, Signal- und Systemtheorie falls
back to SSy, not SuS.

Never derived: a form on `internal/abbrev/blocked.tsv` (SS, SA, NS, KZ, KKK, NPD, AfD, MfS, THC,
NSA, IBM …: a public timetable must not show them next to a lecture; PO, WS, SWS and LP, which a
study plan shows itself; CO, since Controlling II as Co2 read as the gas) and the capitals of the
buildings of short room names (ZHG, HG, HS, LG, VG, ZB, SFB, SD …: a week grid shows a module and
its room side by side), also with a series number or language level (SS1, SS-A1), and compared as
uniqueness compares forms, so S&A is SA and a suffix NP-d is NPD; a title that has the form as a
word of its own may use it. The review of 2026-09-25 added WS, SWS, LP, MfS and CO and the
comparison without & and -: 20 defaults and 422 pairs moved (Wirtschaftssoziologie WS → Wir,
Controlling II Co2 → Con2, Modellieren und FE-Simulieren MFS → MFES, Sustainability and
Digitalisation S&D → SDi). A curated file, `internal/abbrev/overrides.tsv`, gives the owner's two
examples and a few well-known forms (BA, MA, DB, ABWL n, BS n, OOP, and three a module's own page
uses) as a first candidate. It beats every derived candidate, whatever the tier, and the form of an
owner or common line is reserved in the whole catalog: no module of another head derives AuP or MA
(Datenbanken I may have DB1, Medienanalyse may not have MA). Where two lines' forms meet in a
program, tier and priority decide; siblings with one line are told apart by their subtitle
(Allgemeine Betriebswirtschaftslehre III: Investition … / Beschaffung … → ABWL3I / ABWL3B). The file
is read strictly: five columns at most, a program id of the form `079-82-2008`, no pattern twice.

Within a program, over all modules its students may select (curriculum, electives and FÜS), every
abbreviation is unique, compared without case and without the & and - a reader passes over (B&B
and BB are one form). So is its stem, the form without its series number or language level: ST for
Steuerungstechnik next to ST1 and ST2 for Systemtheorie I and II reads as one series, so two
different heads never share a stem, while a series (Systemtheorie I, II) keeps its own.

**A contested form goes to the better match** (the owner, 2026-09-25: „Wenn das Kürzel schon
existiert, dann darf das Modul das Kürzel behalten, das den höheren Matching-Score hat — muss
kaskadieren, achte aber drauf, dass es nach 3 Mal garantiert terminiert.“). Every candidate has a
matching score, one scale for all modules (`internal/abbrev/assign.go`; `docs/data-sources.md` §13):
10,000 for the form of an override line, 9,000 for an acronym the title states for itself, 8,000 for
the initials of all words (EvS, or the terms of „X- und Y“), and 5,000 minus its cost for every
other form (1 to 7,999: the word, compound and function-letter forms near 5,000, subtitle and longer
forms lower, first letters lowest). A module's list is ordered by it. In a program's contest a module
of that program's curriculum (compulsory, thesis, internship, elective) scores 5,000 more for every
form, a FÜS module nothing (the owner, 2026-09-25: „Alle Module, die in einem Curriculum existieren
und nicht ausschließlich FÜS sind, sollten da auch nochmal einen ordentlichen Boost bekommen.“). The
bonus is the width of the derived band: a curriculum module's derived form of ordinary cost (below
1,000) outranks everything a FÜS module derives, its initials and a stated acronym included; only a
FÜS module's override line can still beat it. It goes by the program, not the module, because 170 of
the 171 modules any program offers as FÜS are in another program's curriculum. Within one module every
form gets it, so no list changes its order. In a program:

1. **Claim.** Every module claims its best candidate. Where two claims conflict — one form for two
   titles, or one stem for two heads — the higher score (with the curriculum's bonus) keeps it; a
   tie goes to the module first in priority order (compulsory modules, the thesis and internships,
   then other curricular modules, then FÜS; within a tier the plan semester, then the module number).
2. **Cascade, at most three rounds.** Every module claims the first candidate of its list it can win
   — one nobody holds, or one whose holders all score lower for theirs (a tie again to priority) —
   if that comes before the form it holds: a module without a form, and one whose better form has
   come free again because the module that took it was displaced in turn. It takes it, and a holder
   it beats is displaced and claims again in the next round.
3. **Rest.** After the third round, one at a time and best first, a module takes a candidate before
   the one it holds (any, without a form) that conflicts with nothing held; nobody is displaced any
   more. A module whose list is used up gets its first form with a letter (`-b` … `-z`, `-bb` …) that
   no form or stem of the program has and that reads as no blocked form and no form reserved for
   another head (NP-d would be NPD).

It terminates: the claim and the three rounds are four passes; the rest displaces nobody, so each of
its steps gives a module a form or moves a holder up its list, which ends; and there are far more
letter suffixes than holders and blocked forms. Ties go by priority in every round, not to whoever
holds the form, so a displacement is never undone by a tie (the loop of a naive cascade: A takes X
from B, B takes it back), and going back to a form that came free cannot loop either. What the
round limit can leave is a module that would still beat the holder of a better form: the rest takes
only free forms. Before the review of 2026-09-25 a module never went back, so a form it had lost
could come free and stay unused (M loses AB to H, N of M's series takes the stem AB from H with AB2,
and M ended with ZZ; now M gets AB). On the data of 2026-09-23 this moved no pair. Within the
curriculum the better match wins whatever the kind: next to the compulsory „Einführung in die
Logistik“ (EiL, a derived form) an elective „Elektronik im Labor“ (EiL, its initials) keeps EiL; were
„Elektronik im Labor“ one of the program's FÜS offers, the bonus would give EiL to the compulsory
module (4,950 + 5,000 against 8,000). With equal scores priority decides:
Grundzüge der Makro- and Mikroökonomik are both GdM; the first in priority order keeps it, the other
takes GMÖ. This replaced the rule that a contested form goes to neither (GMa / GMi, M4); whether a
tie should still go to neither is open (M12 in `docs/data-sources.md` §13).

Siblings — one head, different subtitles — are told apart by the subtitle (Dynamik der
Kraftfahrzeuge - Längs-/Querdynamik → DKL / DKQ). Identical titles share the form in the contest and
then get `-b`, `-c` (two „Häusliche Gewalt“ of Soziale Arbeit: HäG, HäG-b); the plain form goes to
the better claim, which is the first in priority order unless an override line gives one of them
another list. `module_abbrev` holds the form without a program for every module: its best candidate.
Defaults are not contested: a module without a program sits next to no other, a catalog-wide contest
over 4,936 modules would take forms from modules that are never read side by side (68 modules share
the title Bachelorarbeit), and a program's form is its module's default in 96.6 % of the pairs
anyway.

The result depends only on the catalog, never on the order it is read in, but not only on the
program: a new module can move the forms of its own program, and a new title anywhere can move forms
in every program, because the compound splitter and its mined heads learn from all titles (before
the open heads, one added title „Wechselstrom und Gleichstrom“ turned Wechselstromtechnik from Wec
into WT in 12 programs). The owner accepted that; `build.finished` counts the pairs whose form moved
since the build before as `abbrev_changed`.

On the data of 2026-09-23: 4,936 modules and 28,424 (program, module) pairs in 182 programs, no
duplicate, no blocked form, no stem two heads share; 71.8 % of the pairs have exactly three
characters and 94.7 % at most four (68.2 % of the modules' defaults have three); 97.7 % got their
first choice (mean 3.7 fallbacks per program); 94.6 % of the modules have the same form in every
program they are in, and 96.6 % of the pairs the module's default. The three initials of the whole
title moved 372 defaults and 2,680 pairs (2,619 to that form, 61 as a knock-on); the scored
assignment then moved 567 pairs and no default; the review's fixes 20 defaults and 422 pairs (280
newly blocked, 133 Industrial Heating Systems and their Defossilization IHSTD → IHS, whose „their“
is an article now, 9 S&D); the owner's calls of the same day 86 defaults and 1,389 pairs: the terms
of „X- und Y“ 1,334 (1,317 to the terms, 17 in contests), the curriculum's bonus 73 and no default
(34 curricular pairs that had lost a form to a FÜS module keep it; fallbacks of curricular pairs
445 → 410, of FÜS pairs 234 → 268). 18 programs are settled by the claim, 142 need one cascade
round, 22 two, none three; one list runs out („Methods“ in 013-D8-2022, Met-b). AuP and EEG hold in
all 110 of their program pairs; in Umweltwissenschaften Bachelor 2025 (G29-82-2025) the internship
„Außeruniversitäres Praktikum“ no longer takes AuP. The snapshot grows by 1.7 MB.
`internal/abbrev/testdata/gate` holds the forms of four programs and every default on
`catalog-abca4baa1d8f8d8e.db` (schema 8, data of 2026-09-23). With `RADIX_ABBREV_GATE=<that
snapshot> go test ./internal/abbrev -run TestGate -v` the test prints what a rule change moves, and
fails on a blocked form or a stem two heads of a program share; `RADIX_ABBREV_GATE_STRICT=1` also
fails on any difference, and `RADIX_ABBREV_GATE_WRITE=1` writes the files again (their comment lines
stay: add the new rule to them by hand).

`validate` fails when an event date with a room has no short form, a short form names two rooms, a
module or a program's module has no abbreviation, a program has one twice (without case, & and -), a
derived one is blocked (`abbrev.Blocked`, the list the derivation reads), or one is not 2 to 10
characters without spaces (an override line may name a blocked form, but the letter suffix of its
twin is checked); a baseline, at least 55 % of the modules with a default of exactly three
characters (68 % now), catches a derivation that silently degrades. It counts modules, not program
pairs: a FÜS module is in about 60 programs, so a few new language courses would move a share of
pairs by themselves. A database migrated to schema 9 but not built again fails the first of these,
so it is never exported: after the release, an instance that does not crawl (`RADIX_CRAWL=off`)
needs `radix build`, then `validate` and `export` (`docs/operations.md`; `deploy/vps/50-app.sh`
builds a seeded volume before its first export). The build warns with `build.rooms_unknown_building`
(an event room names a building the table lacks), `build.room_short_collisions` (two rooms would
share a form and keep their long form) and `build.abbrev_overrides_unused` (a line of the override
file applies to no module: a module number the catalog lacks, a program without that module, a
pattern that matches nothing an earlier line does not take); each wants a line in the table or the
file. `build.finished` counts `abbrev_fell_back`, `abbrev_twins` and `abbrev_changed`.

Open:

- **Schema 9 in Folia.** `catalog::SCHEMA_VERSION` (`catalog/src/db.rs`) is still 8: this lane changes
  no Folia code, so `catalog::tests::the_queries_are_written_for_the_newest_schema` fails while
  migration 0009 is in the tree. The merge must carry the Folia side with it: raise the constant to 9
  (browsers then refuse an older snapshot, so every instance needs `radix build`, then `export`,
  before the web build that reads 9 goes live), and pin `STUDYPLAN_DIGEST` (`catalog/src/tests.rs`,
  `server/src/tests.rs`) to a schema-9 export in `snapshot/`: the digest covers `module_abbrev` and
  `program_module_abbrev`, so no schema-9 snapshot matches the pinned 4b65e821… and the pinned checks
  are skipped until then.
- **Web server (Rust) and frontend.** Both still read the v1 layout and do not work against a
  snapshot. The server becomes an HTTP client of the service: poll `/snapshot/catalog.db` with
  `If-None-Match`, keep the file, serve it as `/api/db` with the same ETag, and answer SSR pages
  from the views. The frontend rewrite follows the view map in section 4.
- **Plan matching.** Only about 44 % of the plan entries are linked to a catalog module (title
  matching). v2 has clean German and English titles for every module, which should lift this.
- **v1 data** left the repository on 2026-09-19 (databases, disk cache, logs, config): it is in
  `betula-radix-backup-2026-09-19` next to the repository, together with a copy of the working
  database from before the first prune. No code reads it; the importer for the v1 plans was removed.
- **`data-sources.md`** describes the v1 code paths it audited; those files no longer exist.

## 7. Incident note

The one-time v1 cache import (`import-cache`, since removed) read the v1 disk cache through `DiskCache.Get`, which deletes an entry when it is
expired. The 896 cached QIS event pages (3-day TTL, fetched 09-11 … 09-14) were already expired
and were removed by that read. Nothing Radix could still use was lost, but they would have
been useful as offline test data for events.

## 8. Events after the first QIS crawl (2026-09-19)

`crawl-tree` (152 pages fetched, 2,500 read from the archive) and `crawl-events` (2,812 pages,
one request at a time, 500 ms ±30 %, 30 minutes) finished with 0 retries, 0 not-found and 0
failed pages.

| | |
|---|---|
| Events | 2,812: 1,776 teaching, 1,009 exams, 27 other |
| Semesters | SoSe 2026: 1,802 teaching + 1,009 exam events. WiSe 2026/27: 1 event so far |
| Modules with a recurring schedule / with exam dates | 1,325 / 1,078 |
| English-taught modules with events | 341 (v1: 31) |
| Modules with a known campus | 1,005 (Zentralcampus 619, Sachsendorf 181, Senftenberg 217) |
| Instructors / responsible persons | 187 / 521 distinct names |

Two things the real data showed:

- **One early winter event broke the schedule facets.** They looked at "the newest semester that
  has events", which became WiSe 2026/27 as soon as its first event was published, and 1,324
  modules lost their campus. Each module now uses the newest semester in which it has teaching
  events itself (migration 0005, regression test, and a `validate` baseline).
- **Retention will remove most of these events at once.** 1,663 of the 2,812 events ended more
  than a month ago (the summer lecture period ended in July) and 770 have no date at all. With
  `--event-retention 720h` the first service cycle removes the 1,663. That is the decided rule;
  until BTU publishes the winter semester the schedule views will mostly hold exams. The live
  The prune ran on 2026-09-19: 1,149 events remain (554 teaching, 577 exams, 18 other).
