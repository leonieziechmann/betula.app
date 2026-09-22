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
`foreign_keys = ON`.

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
| `module_prerequisite`, `module_successor` | module IDs named in the prerequisite texts / successor rows, only when the module exists | module page |
| `program` | one row per PO version. `id` = `<stg>-<abschl>-<pversion>` from the QIS node (`079-82-2008`), `slug` readable and unique (`bachelor-informatik-2008`), degree split into `degree_level`, `degree_type`, `study_variant` | QIS tree (the PO page's own breadcrumb, so the index pages above it are not needed) |
| `program_document`, `program_area` | statutes and amendments; the area tree below a PO with `section` and `stated_kind` | QIS tree |
| `module_program_ref` | every „Zuordnung zu Studiengängen" triple with `resolve_status` (`resolved`, `abroad`, `unresolved`) | module page |
| `program_module_assertion` | one row per statement "module M is in program P" per source, with `kind`, `kind_basis` (`stated`/`inferred`) and area | module page, QIS tree, validated plan |
| `plan`, `plan_entry`, `plan_scan_status` | validated study plans; written transactionally by `SavePlan`, never touched by the build; not foreign-keyed to derived tables, so a plan survives an incomplete crawl | statute PDFs |
| `plan_total`, `plan_total_entry` | the sums a regulation prints over the rows of its own plan, with the rows each counts. `scope` = `plan` (everything these semesters hold) or `section` (a named part); `is_choice` marks the sum that is the only statement of how much its rows count for. A sum is stored only where its rows reach it, so `credits` always lies between `min_credits` and `max_credits` | statute PDFs |
| `semester`, `event`, `event_form`, `event_person`, `event_date`, `module_event` | events keyed by semester (`2026S`, `2026W`), `category` (`teaching`, `exam`, `other`), `last_date` for the retention rule, campus per date | QIS event pages; the module page decides which events belong to a module |
| `program_module`, `module_facet` | materialized results of `v_program_module_src` and `v_module_facets_src` (section 3) | build |
| `meta` | `built_at`, `current_semester`, `radix_version` (the Radix that built it, `internal/version`), oldest/newest fetch and page count per source; `content_digest`, `data_changed_at` | build |

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
| `v_module` | module | `id, title, title_de, title_en, detail_status, page_lang, credits, language_raw, teaches_german, teaches_english, duration_raw, duration_semesters, turnus_raw, turnus_season, turnus_parity, offer_status, limitation_raw, is_limited, participant_limit, exam_form, exam_form_raw, exam_details, grading_raw, is_graded, is_fues, department_id, department, department_code, learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory, remarks, source_url, fetched_at, responsible, teaching_events, at_zentralcampus, at_sachsendorf, at_senftenberg` |
| `v_module_facets` | module | `module_id, credits, department_id, teaches_german, teaches_english, duration_semesters, offered_winter, offered_summer, turnus_season, turnus_parity, offer_status, is_limited, participant_limit, exam_form, exam_written, exam_oral, exam_paper, exam_presentation, exam_project, exam_practical, is_graded, is_fues, has_lecture, has_exercise, has_seminar, has_practical, has_project, has_excursion, teaching_events, at_zentralcampus, at_sachsendorf, at_senftenberg`. Campus flags are NULL (unknown) for a module without a room in the newest semester. |
| `v_module_search` | module × title variant | `module_id, term, kind` (`id`, `title_de`, `title_en`): the only place that needs `LIKE` |
| `v_module_lecturer` | module × person | `module_id, name, title, role` (`responsible`, `instructor`) |
| `v_module_teaching_form` | module × form | `module_id, ord, form, form_raw, workload_raw, sws, hours` |
| `v_module_text_item` | module × item | `module_id, kind` (`literature`, `course`)`, ord, text` |
| `v_module_prerequisite` | module × required module | `module_id, required_module_id, kind, required_title, required_offer_status` |
| `v_module_successor` | module × successor | `module_id, successor_id, successor_title` |
| `v_module_schedule` | module × event date, exams excluded | `module_id, semester_key, semester_label, event_id, event_number, event_title, event_type, ord, group_name, weekday, start_time, end_time, rhythm, rhythm_raw, first_date, last_date, room, campus, instructor, comment, cancelled_dates, source_url` |
| `v_module_exam` | module × exam date | `module_id, semester_key, semester_label, event_id, event_number, event_title, ord, weekday, start_time, end_time, first_date, last_date, room, campus, comment, source_url` |
| `v_module_program_link` | module × listed triple | `module_id, ord, degree_raw, program_raw, po_raw, resolve_status, program_id, program_slug, program_name, degree_display, po_version, is_latest_po, relation, kind, kind_source, area` |
| `v_program` | program | `id, slug, name, degree_level, degree_type, study_variant, degree_label, degree_raw, degree_display, po_version, po_year, po_amendment, family_key, name_key, is_latest_po, stg_code, abschl_code, source_url, fetched_at, has_plan, plan_validated_at, plan_status, curricular_modules, fues_modules, documents` |
| `v_program_module` | program × module | `program_id, module_id, relation, kind, kind_source, kind_basis, precedence, area, section, in_tree, on_module_page, in_plan, module_title, module_credits, offer_status, turnus_season, plan_semester` |
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
  service cycle) and remembered in `event_tombstone`, because module pages keep linking them.
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
chosen with and what they come to together. Of the 139 plans of 2026-09-22, 122 state what they
add up to.

A „Summe Aufwand" line is not one of them: it counts the work of a semester, not the credits
booked in it, and a plan may print both with different numbers (Elektrotechnik 2022 prints 27 and
24 for its second semester). The reader keeps the credit line.

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

Of the 182 program versions, 139 now have a validated plan. What the remaining 43 need is a
different kind of reading, not a fix: plans that mark a semester with „X" and print the credits in
a block column (Environmental and Resource Management), plans whose boxes carry no credits at all
(World Heritage Studies, Urban Design), documents that publish only an amendment, and programs
whose regulation has no plan table at all (Orientierungsstudium).

Open:

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
