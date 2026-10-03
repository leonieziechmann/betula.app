# Brief: Backend & database overhaul (data sources, schema, read views)

> **Names.** Written before the project had a name. Since 2026-09-20 the product is Betula, "the scraper" is
> **Radix** (`radix/cmd/radix`, `radix.db`, `RADIX_*`) and the web tier is **Folia**; the text below keeps the old names.

> Hand-off brief. It is self-contained; everything below was verified against the
> repository and `btu_modules.db` on 2026-09-19 unless marked *(verify)*.

## 1. Context

The project collects course data about BTU Cottbus-Senftenberg and shows it in a web app.

| Part | Where | Role |
|---|---|---|
| Scraper / writer | `radix/cmd/scraper/main.go`, `radix/internal/provider/*`, `radix/internal/parser/*`, `radix/internal/gemini/*`, `radix/internal/storage/*` (Go, `modernc.org/sqlite`, no CGO) | Scrapes sources, writes `btu_modules.db` (WAL mode) |
| Browser app | `frontend/` (Rust/Leptos → WASM) | Downloads the **whole DB file** via `GET /api/db` and queries it in the browser with sql.js (`frontend/static/sqlite_bridge.js`, all SQL in `frontend/src/db.rs`) |
| Web server | `server/` (Rust/axum, rusqlite, read-only) | Serves `/api/db`, the PWA, and server-rendered fallback pages (SQL in `folia/crates/server/src/db.rs`) |
| Legacy | `radix/internal/web/*` (Go templates) | Old UI, to be retired; not a consumer to design for |

**Decisions already made by the owner**

- Shipping the SQLite file to the browser stays. The problem is *what is in the DB and how it is queried*, not the transport.
- Nothing is released; no backward compatibility is needed (schema, IDs, slugs may change).
- Backend and database get fixed **first**. A frontend rewrite follows and will be built on the read contract this work defines. Do **not** rewrite the frontend here.

## 2. Goal

1. **Know where every fact comes from.** For each fact the app shows, list its sources, decide which one is authoritative, and define the fallback and conflict rules.
2. **Store it cleanly.** Redesign the schema into:
   - raw per-source data with provenance
   - a canonical, normalized, read-optimized model (NULL = unknown, enums instead of free text, enforced foreign keys)
3. **Localize data-access logic in SQL views.** Every question the apps ask („modules of program X", „is module Y offered in winter", „semester of module Y in program X") is answered by one view. Frontend, SSR server and CLI read **only** these views, so query logic stops being re-implemented (differently) in three places.
4. **Make correctness checkable.** Add a validation command and regression tests with concrete expected numbers.

## 3. Data sources today

| # | Source | Code | Writes | Notes |
|---|---|---|---|---|
| S1 | b-tu.de/modul (module list) | `provider/catalog_provider.go`, `parser/catalog.go` | `modules` (id, title) | ~4,900 modules |
| S2 | b-tu.de/modul/&lt;id&gt; (module description page) | `provider/detail_provider.go`, `parser/detail.go` | `modules` (all detail columns), `modules.study_programs` (JSON), `module_study_programs` rows via `resolveAndLinkStudyPrograms` | „Zuordnung zu Studiengängen" is matched to an official program by `FindOfficialProgram` (name + degree + PO, with fallbacks). 76 of 19,897 assignments are unresolved and dropped silently; link errors are ignored (`_ =`). |
| S3 | QIS course pages (`veranstaltung`) | `provider/event_provider.go`, `parser/event.go` | `events`, `event_schedules`, `module_events` | All 2,278 events are `SS 2026`. The UI calls them „aktuelles Semester", but WiSe 26/27 is about to start. |
| S4 | QIS FÜS list | `provider/fues_provider.go`, `parser/fues.go` | `modules.is_fues`, `modules.cross_disciplinary` | Upserts are sticky (`CASE WHEN excluded.is_fues = 1 THEN 1 ELSE modules.is_fues END`), so a module is never un-flagged. |
| S5 | QIS module-description tree per program (`modulBeschrGast` tree) | `provider/program_tree_provider.go`, `parser/program_tree.go`, CLI `qis-tree` | `official_study_programs`, `program_curriculum_modules` with `source_file='qis_tree'` | 179 of 182 programs. Contains membership, Pflicht/Wahlpflicht and area structure (`subject_area` e.g. „Komplex Nebenfach", „Praktische Informatik"; `study_section` Grundstudium/Fachstudium). No semesters, no credits. |
| S6 | OPUS statutes (PDF) + Gemini enrichment, validated by PDF geometry | `radix/internal/gemini/*`, `radix/internal/curriculumscan/*`, CLI `scan-curriculum` | `program_curriculum_modules` (PDF rows), `validated_curriculum_plans` (`layout_json` + per-row `source_evidence`), `program_scan_status` | 140 programs have a validated plan. Described in `README.md`. This pipeline's guarantees must be kept. |
| S7 | Older unverified AI statute scans | CLI path via `MatchAndLinkCurriculumModules` | `module_study_programs` with `source='ai_statute_scan'` | Legacy; see problem A2. |

Row counts: `modules` 4,908 · `events` 2,278 · `event_schedules` 3,105 · `module_events` 3,103 · `official_study_programs` 182 · `module_study_programs` 21,177 · `program_curriculum_modules` 17,446 (12,571 QIS tree, 4,875 PDF of which 2,849 linked to a module) · `validated_curriculum_plans` 140.

## 4. Verified problems

### A. Provenance: one table mixes sources that overwrite each other

- **A1. Two different membership answers.** The catalog's program filter reads `module_study_programs`. The program page reads `program_curriculum_modules`. For 136 of 182 programs they disagree; e.g. Informatik B.Sc. (PO 2008 – 2. SÄ 2024) has 207 modules vs 108.
- **A2. `module_study_programs` has four origins:**
  - module page (`source IS NULL`): 10,534 rows
  - `ai_statute_scan`: 7,247 rows
  - `curriculum_unassigned`: 1,783 rows
  - `verified_pdf_cells`: 1,613 rows

  All are upserted on the same `(module_id, program_id)` key, so the last writer wins.
- **A3. Provenance is mislabeled.** CLI `qis-tree` (`radix/cmd/scraper/main.go:1398`) saves QIS rows and then calls `MatchAndLinkCurriculumModules`. That runs over **all** program rows, QIS tree included, and tags every match `ai_statute_scan`. Result: only 147 of the 7,247 „AI" rows are *not* QIS-tree memberships. The remaining module-page rows are exactly the 10,534 pairs the QIS tree does *not* contain, so module pages and the QIS tree are two genuinely different membership statements. Which one is „true" is a product question (see §7).
- **A4. The same fact from two sources disagrees.** Where a module appears in both the QIS tree and the PDF plan of the same program, `module_type` differs in 824 of 3,194 pairs (e.g. Pflicht vs Wahlpflicht vs „Modul").
- **A5. Denormalized copies drift.** `module_study_programs.program_name`/`degree` differ from `official_study_programs` in 218 rows. `modules.study_programs` (5.4 MB JSON) duplicates the link table.

### B. Sentinels and integrity

- **B1. `0` and `''` mean „unknown".** Columns added via `ALTER … DEFAULT 0`:
  - `module_study_programs.credits = 0` in 17,015 rows, although the module has credits. `COALESCE(msp.credits, m.credits)` therefore yields 0.
  - `recommended_semester = 0` in 17,364 of 21,177 rows. Only 2,030 rows carry a real semester.
  - Unmatched PDF rows store `module_id = ''` instead of NULL.
- **B2. Foreign keys are never enforced.** The Go DSN sets WAL/synchronous but not `foreign_keys`. `PRAGMA foreign_key_check` reports 2,063 violations: 2,053 `program_curriculum_modules → modules` (the `''` IDs plus 27 dangling IDs) and 10 `module_events → modules`.
- **B3. Ad-hoc migrations.** `radix/internal/storage/sqlite.go` runs `ALTER TABLE` statements and ignores their errors. There is no schema version and no way to know which shape a DB file has.

### C. Free text that every consumer re-parses (and gets wrong)

sql.js has no ICU: `LOWER()`/`LIKE` only fold ASCII. So normalization must happen in Go, at write/export time. Current raw values and the bugs they cause in the frontend's SQL:

| Field | Raw values (examples) | Bug today |
|---|---|---|
| `turnus` | „jedes Wintersemester" 1,531 · „Every winter semester" 362 · „jedes Semester" 479 · „Every semester" 124 · „sporadisch nach Ankündigung" 497 · „jedes Wintersemester gerader Jahre" 27 · „jedes Sommersemester ungerader Jahre" 21 · „Each winter semester even year" 7 … | Substring `'%gerad%'` also matches „ungerade". `'%jede%'` pulls 1,373 summer modules into a „WiSe" filter. |
| `language` | „Deutsch" 3,928 · „English" 979 · „Englisch" 1 | ad-hoc `LIKE` |
| `duration` | „1 Semester" 3,475 · „1 semester" 930 · „2 Semester" 444 · „2 semesters" 30 · „6 semesters" 13 · „3 semesters" 5 · „10 Wochen" 3 · „8 Wochen" 1 · NULL 5 | `LIKE '%1%'` matches „10 Wochen" |
| `teaching_forms` (JSON) / `events.event_type` | „Übung", „Vorlesung", „Praktikum" … | `LOWER('Übung')` stays „Übung", so the Übung filter matches 0 of 1,535 modules |
| `exam_type` | free text | Beleg/Hausarbeit/Projekt guessed by substring |
| `grading` | empty for **all** 4,908 modules | „benotet" filter returns 0. Either scrape it or drop the concept. |
| `limitation` | „keine" 3,835 · '' 982 · numbers („20", „25" …) | ad-hoc `NOT IN ('keine','nein','ohne','k.a.')` |
| `event_schedules.room` | „… - Zentralcampus", „… - Campus Sachsendorf", „… - Campus Senftenberg" | Campus guessed via `'%lg%'`, which matches „Al**lg**emeine Elektrotechnik – Campus Senftenberg". Only 1,264 modules have any schedule. |
| `official_study_programs.degree` | „Bachelor (universitär)" 39 · „Master (universitär)" 46 · „LA Bachelor Grundstufe/Primarstufe" 24 · „… - Duales Studium, praxisintegrierend" … | The frontend invents „B.Sc." for every „Bachelor (universitär)", incl. Soziale Arbeit. The source has no B.Sc./B.A. information. |
| `prerequisites_*` | free text | module IDs extracted by scanning for 5-digit numbers |
| `successor_modules` | JSON array or comma list | parsed two ways |

### D. Freshness and export

- **D1. `/api/db` serves the raw DB file.** `folia/crates/server/src/routes.rs` reads it with `tokio::fs::read` while the writer uses WAL. Frames not yet checkpointed are missing, so browser clients can see older data than the SSR pages (which read through SQLite).
- **D2. The ETag is only the file size** (`"btu-db-<size>"`), so an update of the same size is never re-downloaded.
- **D3. Only one semester of events.** Events carry a semester string, but there is no notion of which semester is „current" or which comes next.
- **D4. The snapshot carries data no consumer needs.** `study_programs` JSON (5.4 MB), `current_semester_events` JSON (0.9 MB), `room_url`/`instructor_url`, etc. Not a priority, but the export is the natural place to trim.

## 5. What consumers read today (the future views must cover this)

From `frontend/src/db.rs` (mirrored partly in `folia/crates/server/src/db.rs` and `radix/internal/storage/repository.go`):

- **Catalog list** with filters: search text; program; semester in program; module kind (Pflicht/Wahlpflicht/FÜS); lecturer include/exclude (from `responsible_persons` and schedule instructors); department; turnus season/parity; teaching forms; duration; exam form; graded; limitation; FÜS; phase-out/not-offered; credits range; campus; language. Sort by id/title/credits/event count. Needs exact total counts (today a `LIMIT 300` makes the header wrong).
- **Autocomplete** by title/id/code.
- **Module detail:** all detail fields; events with schedules; linked programs; the module's validated plan entries per program (semester span, credits window, `source_evidence`).
- **Program:** detail and documents; other POs of the same program; Bachelor/Master counterpart (currently name heuristics); module membership with type/area/specialization; validated plan (`validated_curriculum_plans.layout_json`).
- **Lists:** programs; departments (currently the last path segment of `department`); coverage overview (`program_coverage` view already exists).

## 6. Deliverables

1. **`docs/radix/data-sources.md`**: a table per fact (module core data, program list, membership, module type, area/section, semester, credits, schedule, FÜS flag, prerequisites, successor, grading, campus). For each fact: sources, scraper code path, freshness, observed conflicts with numbers, chosen authority and fallback, and how provenance is stored. Where sources conflict, spot-check a sample against the live site through the existing cache/politeness settings, and record the result.
2. **Schema v2** with versioned migrations (`PRAGMA user_version` or a migrations table, errors not ignored) and `foreign_keys=ON`.
   - Raw/staging tables per source, each with `source`, `source_url`, `fetched_at`.
   - Canonical tables with NULL for unknown and normalized enum/flag columns (turnus season + parity, language set, duration in semesters, teaching-form and exam-form flags, campus set, limitation as number/NULL, degree level + variant, semester of each event) plus the raw text kept for display.
   - No denormalized copies.
   - The PDF/validated-plan pipeline keeps its transactional guarantees (see `README.md` and the `radix/internal/gemini` tests).
3. **Read views**, the only contract for all consumers, e.g.:
   - `v_module` (display-ready card)
   - `v_module_facets` (all filterable normalized columns)
   - `v_program` (with degree label/level and PO family)
   - `v_program_module` (membership with `kind`, `area`, `section`, `source`, `precedence`)
   - `v_program_plan_entry` (validated semester data only)
   - `v_module_schedule` (per semester, with campus)
   - `v_module_program_link` (for the module page)
   - `v_program_counterpart`
   - `v_department`
   - `program_coverage` (keep)

   Each view is documented with its columns, and every view is exercised by tests. Names are suggestions.
4. **Export step** (new CLI command) that writes a read-optimized snapshot for `/api/db`:
   - checkpoint or `VACUUM INTO`
   - views and indexes included
   - raw-only data dropped
   - content-hash ETag recorded
5. **`validate` command + tests.** It reports invariants and fails on regressions: FK check, no `''` IDs, coverage per program, conflicts between sources, facet counts. Use the numbers in §4 as fixtures (e.g. the „Übung" teaching-form facet must find ~1,535 modules; „winter" must include the 1,893 plain winter-semester modules).
6. **Server alignment:** `folia/crates/server/src/db.rs` (and Go where still used) query the views instead of their own SQL. `frontend/src/db.rs` is **not** rewritten here. Do list, per current frontend query, which view replaces it.

## 7. Open questions for the owner (bring evidence, don't decide alone)

1. **What does „module belongs to program X" mean?** The QIS tree (exam-regulation structure) vs the module page's „Zuordnung zu Studiengängen" (10,534 extra pairs, e.g. Informatik B.Sc. lists ERP, Bodenschutzrecht, Logistik). Candidate: the QIS tree is membership; module-page pairs are a separate, labeled „also assigned" relation.
2. **Conflicting module type** (QIS vs PDF, 824 pairs): which wins, per program?
3. **Legacy `ai_statute_scan` data:** delete, quarantine, or keep behind a flag?
4. **Degree labels:** keep the source's terms („Bachelor/Master" + variant), or maintain a curated mapping to B.Sc./B.A./…? This affects slugs.
5. **Events:** which semester(s) to scrape and show, and how „current/next semester" is defined.
6. **Grading:** scrape it from a reliable source, or remove the filter?

## 8. Constraints

- Go only for the scraper (pure-Go SQLite, no CGO). Rust server stays read-only (`query_only`).
- **Scrape politely.** Reuse `radix/internal/cache` and the refresher's off-peak/delay settings; no bulk re-crawls without the owner's go-ahead.
- Keep `go test ./...` green and network-free. Keep the regression corpus for the PDF parser.
- Never commit API keys (`GEMINI_API_KEY`, `config.yaml`).
- Work on a branch; don't touch `frontend/` beyond reading it.

## 9. Acceptance criteria

- One view answers „modules of program X"; SSR server and CLI use it and agree on the counts.
- `PRAGMA foreign_key_check` is empty on an exported snapshot, and no `''`/`0` sentinels remain for unknown values.
- Every filterable attribute is a normalized column exposed in a view. No consumer needs `LIKE` on free text except for full-text search.
- `docs/radix/data-sources.md` states the authority for every fact, and the `validate` report shows no unexplained conflicts.
- The export produces a checkpointed snapshot with a content-hash ETag, served by `/api/db`.

## 10. Useful entry points

- Schema/migrations: `radix/internal/storage/sqlite.go`, `radix/internal/storage/program_coverage.go`
- Writers: `radix/internal/storage/repository.go`
  - `UpsertModuleDetail` :66
  - `UpsertFUESList` :638
  - `FindOfficialProgram` :1131
  - `LinkModuleToStudyProgram` :1033
  - `saveCurriculumModules` :2347
  - `MatchAndLinkCurriculumModules` :2489
- Module-page link resolution: `radix/internal/provider/detail_provider.go:155`
- CLI commands: `radix/cmd/scraper/main.go`
  - `qis-tree` flow at :1398
  - `scan-curriculum` flow
  - `serve`
- Consumers: `frontend/src/db.rs`, `folia/crates/server/src/db.rs`, `folia/crates/server/src/routes.rs` (`/api/db`, `/api/status`)
- Live DB: `btu_modules.db` in the repo root (open read-only for analysis)
