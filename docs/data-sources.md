# Data sources: where every fact comes from

> **Names.** Written before the project had a name. Since 2026-09-20 the product is Betula, "the scraper" is
> **Radix** (`cmd/radix`, `radix.db`, `RADIX_*`) and the web tier is **Folia**; the text below keeps the old names.
>
> Deliverable 1 of `docs/backend-data-overhaul.md`.
> **Historical audit.** The code paths and line numbers below refer to the v1 scraper as of commit
> `6393aca`. That code was removed after schema v2 replaced it; the defects P1–P6 are fixed.
> Verified on 2026-09-19 against `btu_modules.db` (opened read-only), the scraper code,
> 45 module pages + the FÜS list + the catalog list from `.cache`, and 2 live requests (§6).
>
> **Status of decisions.** The owner decided all open questions on 2026-09-19. Rules marked
> **DECIDED (Qn)** follow those decisions; §7 records each decision next to its evidence.
> Everything else follows directly from the data.

## 1. Summary

1. **The module page (S2) is the authority for every fact about a module.** It is the
   freshest source (all 4,906 catalog modules were re-fetched on 2026-09-19) and the
   other QIS-derived sources agree with it wherever they overlap: FÜS list credits agree
   in 288 of 288 modules, the FÜS flag agrees on 45 of 45 cached pages.
2. **Most of today's "source conflicts" are parser defects, not disagreeing sources.**
   Five defects (§3) explain the missing grading, 979 modules without any program
   assignment, 1,565 wrong Lehramt links, and most module-type conflicts.
3. **The module page and the QIS tree make the same membership statement** for regular
   modules (90–94 % agreement for German non-FÜS modules, §5.3). The 10,000+ "extra"
   module-page pairs are FÜS modules that list every program admitted to take them
   (96 % of the extra pairs). That is a different relation, not a conflict.
4. **The QIS tree never says "Pflicht".** The scraper defaults to `Pflicht` whenever a
   path has no "wahl" in it; 7,533 of 9,220 `Pflicht` rows are this default. The validated
   PDF plan and the module page's remarks are the only real sources for the module kind.
5. **Raw pages are not retained** (45 of 4,908 module pages are in `.cache`). Fixing the
   parser defects needs one polite re-crawl of the module pages (about 4,900 requests,
   roughly 41 minutes at the configured 500 ms). That needs the owner's go-ahead.
   Schema v2 should store the raw label/value rows so a parser fix never needs the
   network again.

## 2. Source inventory

| # | Source | URL | Provider / parser | Cache key, TTL | Last fetched (DB) | Refresh today |
|---|---|---|---|---|---|---|
| S1 | Module catalog list | `b-tu.de/modul` | `provider/catalog_provider.go`, `parser/catalog.go` | `catalog:list`, 24 h | cache valid | Refresher, every 12 h (`catalog_interval_hours`) |
| S2 | Module page | `b-tu.de/modul/<id>` | `provider/detail_provider.go`, `parser/detail.go` | `module:<id>`, 7 d | 4,906 on 2026-09-19, 2 on 2026-09-11 | Refresher `runSlowModuleBatch`: oldest first, off-peak 01–06 h, 500 ms ±30 %, always `forceRefresh` |
| S3 | QIS event page | `qisserver3/rds?state=verpublish…veranstid=<id>` | `provider/event_provider.go`, `parser/event.go` | `event:<id>`, 3 d | 2026-09-11 … 09-14 | Discovered only through S2's "Veranstaltungen im aktuellen Semester" links; 800 ms delay |
| S4 | QIS FÜS list | `qisserver3/rds?…TableSelectModul.vm…missing=FUES` | `provider/fues_provider.go`, `parser/fues.go` | `fues:catalog`, 7 d | cache valid until 2026-09-24 | Manual (`scraper fues`) |
| S5 | QIS program tree (`modulBeschrGast`) | `qisserver3/rds?state=modulBeschrGast…` | `provider/program_tree_provider.go`, `parser/program_tree.go` | `qis:tree:<sha1(url)>`, 7 d | programs 09-11 … 09-17, tree rows 2026-09-17 | Manual (`programs`, `qis-tree`); 1 worker, 300–1000 ms |
| S6 | OPUS statute PDFs + Gemini enrichment, validated by PDF geometry | `opus4.kobv.de/opus4-btu/files/…` | `internal/gemini/*`, `internal/curriculumscan/*` | files in `statutes/` | plans validated 09-18 … 09-19 | Manual (`scan-curriculum`) |
| S7 | Legacy unverified AI statute scan | – | `MatchAndLinkCurriculumModules` | – | – | Dead path, see Q3 |

S1–S5 are all views of the same HIS/QIS database. S6 is the legal text. This matters for
conflict rules: S1–S5 disagreeing with each other points to a scraper bug; S2/S5
disagreeing with S6 points to a regulation that is older or newer than the live catalog.

What each source **states** versus what the scraper **infers**:

| Source | States | Inferred by the scraper (not a source statement) |
|---|---|---|
| S2 | All module fields; degree / program / PO triples; FÜS sentence; events of the current semester; free-text remarks that often name the kind and area per program | Phase-out and "not offered" (substring search); successor IDs (regex on remarks); resolution of a triple to a program ID |
| S4 | The module is approved for FÜS; language, credits, limitation | – |
| S5 | Program → degree → PO version → documents; PO → area nodes → module leaves | `module_type` (defaults to `Pflicht`), `study_section`, `subject_area`, `specialization` (position in the path) |
| S6 | Semester column, credits or credit range, table structure (deterministic, from PDF geometry) | Module number, kind and area (Gemini enrichment, checked by the validator) |

## 3. Defects found while verifying

These change the conflict numbers in the brief, so they come before the per-fact tables.

| # | Defect | Where | Effect (measured) |
|---|---|---|---|
| P1 | **Exam rows collide.** `normalizeKey` tests `modulprüfung` / `moduleexam` before `prüfungsleistung` and `bewertung`. The labels „Prüfungsleistung/en für Modulprüfung" and „Bewertung der Modulprüfung" (EN: "Assessment Mode for Module Examination", "Evaluation of Module Examination") all contain that substring, so all three rows land in `ExamType` and the last one wins. | `parser/detail.go:432-437` | `exam_type` holds the **grading** value for 4,903 modules („Prüfungsleistung - benotet" 3,675 · "Performance Verification – graded" 945 · „Studienleistung - unbenotet" 249 · "Study Performance – ungraded" 34). The real exam form (e.g. „Voraussetzung + Modulabschlussprüfung (MAP)") and the exam details (Klausur 90 min …) are lost. `exam_details` and `grading` are empty for all 4,908 modules. |
| P2 | **English module pages use labels the parser does not know:** "Part of the Study Programme", "Limited Number of Participants", "Module Components", "Components to be offered in the Current Semester", "This module has been approved for the general studies". Degrees are also English there ("Bachelor (research-oriented)", "… - Co-Op Programme with Practical Placement"). | `parser/detail.go:438-447`, `:101`, `:137` | All **979** English-language modules have 0 program assignments, an empty limitation, 0 associated courses and 0 current-semester event links (only 31 of them have any event). 1,954 of the 1,967 QIS-tree memberships whose module page "lists no program" are these modules. |
| P3 | **Lehramt degrees contain a slash.** „LA Bachelor Grundstufe/Primarstufe / Lehramt Primarstufe Deutsch-Englisch / PO 2025" is split on `/` into degree „LA Bachelor Grundstufe", program „Primarstufe", regulation „Lehramt Primarstufe Deutsch-Englisch". `FindOfficialProgram` then falls back to `LIKE '%primarstufe%' ORDER BY po_version DESC LIMIT 1`. | `parser/detail.go:465`, `storage/repository.go:1203` | 1,715 entries take this path. **1,565 are linked to the wrong program** (all collapse onto „Lehramt Primarstufe Mathematik-Sport, PO 2026"); 150 are right by coincidence. 64 more stay unresolved. |
| P4 | **QIS tree kind is a default.** `AnalyzeQISPath` starts with `moduleType = "Pflicht"` and only changes it when a path segment contains „wahl", „abschlussarbeit", „füs" … English trees ("Compulsory Elective and Optional Modules") never match. | `parser/program_tree.go:428` | 7,533 of 9,220 `Pflicht` rows have no „pflicht"/"compulsory" label anywhere in the stored path. All 1,981 rows of English-labelled trees are `Pflicht`. In Informatik B.Sc. the elective complexes „Praktische Informatik" (10), „Angewandte und Technische Informatik" (9), „Grundlagen der Informatik" (7) are all stored as `Pflicht`. |
| P5 | **Stale IDs in the JSON copy.** „Abschluss im Ausland" links were deleted from `module_study_programs` by a migration, but `modules.study_programs` still carries `official_program_id` for 1,077 such entries. Link rows are never deleted when a page stops listing a program. | `storage/sqlite.go:243`, `LinkModuleToStudyProgram` | **339 of the 10,534 module-page link rows are no longer stated by the module's page** (stale). Both problems go away with the JSON column removed and a replace-per-module write. |
| P6 | **Raw input is not retained.** The refresher always fetches with `forceRefresh`, and only 45 of 4,908 module pages (896 of 2,278 event pages) are in `.cache`. | `refresher/refresher.go:499` | P1–P3 cannot be fixed by re-parsing; a re-crawl is needed. |

Consequences for schema v2 (deliverable 2): keep the raw label → value rows of every module page
(plus page language and a content hash) in the staging table, match labels by an exact
DE/EN label map instead of substrings, split the assignment triple on `" / "` from the
right (PO last, program second to last), and map English degree strings to the German ones.

## 4. How provenance is stored (common scheme)

Referenced by every fact below as "scheme §4".

- **Raw / staging, one table per source** (`raw_module_page`, `raw_module_catalog`,
  `raw_event_page`, `raw_fues_list`, `raw_qis_tree_node`, `raw_pdf_plan_row`). Every row
  has `source`, `source_url`, `fetched_at`, `content_hash`. A scrape **replaces** the rows of
  the page it fetched (no sticky upserts), so a removed statement disappears.
- **Single-source facts** (everything that only S2 states) live as columns in the canonical
  table. Their provenance is the staging row of the same key (`module_id` →
  `raw_module_page`), so no per-column source is needed.
- **Multi-source facts** (membership, kind, area, credits per program, FÜS flag) are stored
  as one *assertion row per source* with a `source` enum. A view applies the precedence
  rule and exposes `source` and `precedence`, as the brief asks for `v_program_module`.
  No source ever overwrites another source's row.
- **`source` enum:** `module_page`, `module_catalog`, `qis_event`, `qis_fues_list`,
  `qis_tree`, `pdf_plan`, `curated` (hand-maintained mappings, e.g. degree labels),
  `legacy_ai` (only if Q3 says "quarantine").
- **Unknown is NULL.** No `0`, `''`, `'-'` or `'Modul'` placeholders.

## 5. Facts

Every table has the same rows: sources, code path, freshness, observed conflicts,
authority and fallback, provenance.

### 5.1 Module core data

Title DE/EN, department, responsible persons, language, duration, turnus, credits (see 5.7),
learning outcomes, contents, teaching forms, literature, limitation, remarks, phase-out.

| | |
|---|---|
| Sources | S2 (all fields). S1 (id, title only). S4 (title, language, credits, limitation for the 288 FÜS modules). |
| Code path | `detail_provider.go:ScrapeModule` → `parser/detail.go:Parse` → `repository.go:66 UpsertModuleDetail`. S1: `UpsertModuleSummaries`. S4: `repository.go:638 UpsertFUESList` (fills only empty columns). |
| Freshness | S2: 4,906 modules fetched 2026-09-19; rolling off-peak refresh, oldest first. |
| Conflicts | **S4 vs S2:** credits 0 of 288 differ; titles 0 differ; language differs only in spelling („Englisch" vs "English", 82); limitation has 18 real differences, 17 of them English modules where S2's value was lost to P2 and S4 has the number (e.g. 12642: 25). **Module set:** 4,906 in S1; 2 more (12690, 14037) exist only as S4 skeleton rows without any detail. **Gaps from P2:** 979 modules without limitation. 5 modules have no department/turnus. **Department** is free text in two languages and two faculty structures (e.g. „Fakultät 1 - MINT - …" 535, "Faculty 1 - Mathematics, …" 250, the older „Fakultät 1 - Mathematik, Naturwissenschaften und Informatik" 188). Turnus, language, duration value lists: see brief §4 C. |
| Authority | **S2.** Fallback S4 for limitation/language/credits only while S2 has no value (it then agrees anyway). S1 is a discovery list: it decides which IDs exist and gets a title only until S2 was fetched. A module that is only in S4 and returns 404 on S2 is kept with `detail_status = 'missing'`, not as a half-filled module. |
| Provenance | Scheme §4, single-source. Normalized columns (turnus season/parity, language set, duration in semesters, limitation as integer/NULL, faculty number) are derived in Go at write time and the raw text is kept for display. |

### 5.2 Program list

Program, degree, PO version, regulation documents.

| | |
|---|---|
| Sources | S5 only (root → „Studiengang:" → „Module für Abschluss:" → „PO-Version:" → document links). |
| Code path | `program_tree_provider.go:ScrapeProgramTree` / `processStudyProgram` → `UpsertOfficialPrograms`. ID = `stg_<code>_abschl_<code>_po_<version>`. |
| Freshness | 2026-09-11 … 09-17. Manual. 7 d cache. |
| Conflicts | None (single source). 182 programs; 148 program/degree families: 115 with one PO version, 32 with two, 1 with three. 3 programs have no documents and no tree (Hebammenwissenschaft, Pflegewissenschaft, Physiotherapie, all PO 2026). 21 distinct degree strings; 8 rows are not degrees („Strukturiertes Promotionsstudium" 4, „keine Abschlussprüfung möglich" 4). Module pages reference POs the list does not have as an exact string in 304 entries (page „PO 2024", list „2024 - NF 2026"); the substring match is unique in all 304 cases. |
| Authority | **S5.** Degree level (bachelor/master/lehramt/doctoral/none) and variant (dual praxisintegrierend, dual ausbildungsintegrierend, Doppelabschluss, erweiterte/verringerte Fachsemester, Fernstudium, Teilzeit) are parsed from the degree string. The short label (B.Sc., B.A. …) is derived automatically, **DECIDED (Q4)**, see §7. PO family = same `program_code` + degree level. |
| Provenance | `raw_qis_tree_node` rows of the program/degree/PO levels; documents as child rows instead of a JSON column. |

### 5.3 Membership: "module M belongs to program P"

| | |
|---|---|
| Sources | **S2** „Zuordnung zu Studiengängen" (degree / program / PO triples). **S5** module leaves under a PO. **S6** plan rows that were matched to a module. S7 adds nothing: only 147 of its 7,247 rows are not QIS-tree memberships (brief A3). |
| Code path | S2: `detail_provider.go:155 resolveAndLinkStudyPrograms` → `repository.go:1131 FindOfficialProgram` → `:1033 LinkModuleToStudyProgram` (errors ignored). S5: `TraverseQISCurriculum` → `SaveCurriculumModules(…, "qis_tree")`. S6: `SaveValidatedCurriculumModules`. |
| Freshness | S2 2026-09-19 · S5 2026-09-17 · S6 09-18 … 09-19. |
| Conflicts | Definitions: *page pair* = distinct (module, resolved program) from the page, without „Abschluss im Ausland" (1,089 entries) and „keine Zuordnung vorhanden" (1,476 modules). *Tree pair* = distinct (module, program) among QIS-tree leaves. **Page pairs 17,243 · tree pairs 10,140 · both 6,985 · page-only 10,258 · tree-only 3,155.** |
| | **Page-only (10,258):** 10,153 are in programs that have a tree. **9,752 of those (96 %) belong to FÜS-flagged modules.** A FÜS module lists a median of 74 programs, a regular module a median of 2. Informatik B.Sc.: 95 page-only modules, 94 of them FÜS (ERP, Einführung in die Logistik, Medienanalyse …). This confirms the owner's description: FÜS modules list every program that may attend. 401 page-only pairs come from regular modules. |
| | **Tree-only (3,155):** 2,604 belong to regular modules. 1,954 of those are English modules whose page list was dropped by P2. The live check (§6) shows the page does list the program. Only 650 remain for German regular modules. |
| | **Agreement for German regular modules:** 5,869 of 6,519 tree pairs are on the page (90 %); 5,869 of 6,270 page pairs are in the tree (94 %). Elective memberships are listed on the page too: 1,352 of 1,352 tree pairs whose path is labelled as elective, and 70 of 82 pairs the PDF plan calls Wahlpflicht. So the page is **not** limited to programs where the module is mandatory. |
| | **„keine Zuordnung vorhanden"** (1,476 modules) is consistent: only 11 of them appear in any tree. |
| | **Resolution quality of page triples** (19,897 real entries): 16,725 exact · 304 unique substring match on the PO · 1,715 Lehramt entries through P3 (1,565 wrong) · 76 unresolved and dropped silently. |
| | **S6:** 2,343 distinct linked pairs; 2,174 are in the tree; 143 are in neither tree nor page (older regulations naming modules the live catalog no longer assigns). 2,026 PDF rows have `module_id = ''` and 15 distinct IDs do not exist in `modules`. |
| Authority | **DECIDED (Q1).** Two relations instead of one: **(a) curricular membership** = every module a student can take towards the degree: the pair is in the QIS tree, or on the page of a module that is not FÜS-flagged, or in the validated plan. **(b) the program's FÜS list** = pair is on the page of a FÜS-flagged module and the module is *not* curricular in that program (FÜS is meant to widen the horizon, so a program's own modules never count as its FÜS). Every program therefore has its own FÜS list. Within (a) the sources are a union with provenance, not a precedence: page and tree state the same thing, and after fixing P2/P3 the single-source remainder (about 650 + 401 pairs) becomes a `validate` report line instead of a silent difference. S6-only pairs are kept with `source = pdf_plan` and shown in the plan, but do not count as live-catalog membership. |
| Provenance | Scheme §4, multi-source: `program_module_assertion(program_id, module_id, source, relation, raw_ref, fetched_at)`; `v_program_module` exposes one row per pair with `relation`, the list of sources and `precedence`. Unresolved page triples are kept in staging with `resolve_status` instead of being dropped. |

### 5.4 Module kind (Pflicht / Wahlpflicht / Abschlussarbeit / Praktikum / FÜS)

| | |
|---|---|
| Sources | **S6** (kind per plan row, validated). **S2 remarks**, free text such as „Studiengang Mathematik B.Sc.: Pflichtmodul im Komplex ‚Grundlagen'" / "Compulsory elective module in complex …": 712 modules carry such statements. This is the owner's "program information written in different fields". Not parsed today. **S5** only when a node label says so explicitly. |
| Code path | S6: `internal/gemini/*` → `saveCurriculumModules`. S5: `parser/program_tree.go:395 AnalyzeQISPath`. |
| Freshness | As 5.3. |
| Conflicts | Tree vs plan, same module and program: **824 of 3,194 row pairs** (brief A4) = **438 of 2,174 distinct pairs.** Breakdown of the 438: tree `Pflicht` → plan `Wahlpflicht` 235 · → `Modul`+`Pflicht` 74 · → `Abschlussarbeit` 41 · → `Praktikum` 18; tree `Abschlussarbeit` → plan `Pflicht` 29; others 41. The largest group is P4: the conflicting `Pflicht` rows sit in „Total Account / Compulsory Elective and Optional Modules", „Individuelle Spezialisierung", „Vertiefungsmodule", „Schwerpunkt (…)". Live check (§6): tree says `Pflicht` for 11861 and 11881 in Informatik B.Sc.; the live module page says "Compulsory elective module in complex …" for both. Plan values: `Pflicht` 1,933 · `Wahlpflicht` 317 · `Modul` 103 (placeholder for "unknown") · `Abschlussarbeit` 55 · `Praktikum` 23. |
| Authority | **DECIDED (Q2).** General rule: **a stated value beats an inferred value.** Among stated values: `pdf_plan` > `module_page` remarks > `qis_tree` node label. Inferred values (e.g. "thesis" from a module title) rank below every stated value and are marked `basis = 'inferred'`. The tree's `Pflicht` default is dropped entirely: no label, no value. `Modul` becomes NULL. |
| Provenance | Scheme §4, multi-source: `kind` is a column of the assertion row, so each source keeps its own value and the view picks by precedence. `validate` lists pairs where two explicit statements disagree. |

### 5.5 Area, section, specialization

| | |
|---|---|
| Sources | **S5** path position („Grundstudium"/„Fachstudium", „Komplex Nebenfach", „Praktische Informatik"). **S6** table headings of the plan. **S2 remarks** („… im Komplex ‚Grundlagen'"). |
| Code path | `AnalyzeQISPath` (section by keyword, area = first remaining segment, specialization = last). S6 via Gemini enrichment. |
| Freshness | As 5.3. |
| Conflicts | Not comparable as strings today (tree labels vs PDF headings vs German/English). English trees produce areas such as „Total Account / Modules at the Brandenburg University of Technology …", which is bookkeeping, not an area. The full path is not stored, only three derived strings. |
| Authority | **S5 for structure** (it is complete for 179 programs and is how the exam office books modules); S6 headings are kept with the plan rows for the plan view only. Store the **full node path** in staging and derive section/area/specialization in one place. |
| Provenance | `raw_qis_tree_node` (node id, parent id, label, url); canonical `program_area` rows reference the node. |

### 5.6 Recommended semester

| | |
|---|---|
| Sources | **S6 only.** No other source has semesters. |
| Code path | `internal/gemini/pdf_*` (geometry) → `validation.go` → `SaveValidatedCurriculumModules` → `validated_curriculum_plans.layout_json` + rows with `source_evidence`. |
| Freshness | 140 validated plans, 2026-09-18 … 09-19. Scan status: saved 19 · saved with warnings 111 · needs review 28 · no plan 21 · missing source 3. |
| Conflicts | None between sources. Sentinels: of 4,875 plan rows, 4,147 have an exact semester, 508 only a span (`recommended_semester = 0`, `start_semester > 0`), 220 none. In `module_study_programs`, `recommended_semester = 0` in 17,364 of 21,177 rows (brief B1) because the column is copied into a table where most rows have no plan. |
| Authority | **S6, validated plans only.** Exact semester NULL when the PDF gives a span; span as `start_semester`/`end_semester`. No copy into the membership table. |
| Provenance | Existing `source_evidence` per row and `layout_json` per plan are kept as they are, including the transactional replace. |

### 5.7 Credits

| | |
|---|---|
| Sources | **S2** „Leistungspunkte" (current catalog value). **S4** (same value). **S6** credits per plan row, credit ranges for elective slots. |
| Code path | `parseCredits` in `parser/detail.go:455`; S6 as 5.6. |
| Freshness | As above. |
| Conflicts | S4 vs S2: 0 of 288. **S6 vs S2: 30 of 2,341** linked pairs differ, e.g. 11826 Informatik 1: catalog 5, Umweltingenieurwesen PO 2021 plan 6; 12238: catalog 6, Heritage Studies PO 2017 plan 3. These are regulations older than the catalog, which the README already treats as a warning. Sentinel: `module_study_programs.credits = 0` in 17,015 rows (brief B1). No module has `credits` 0 or NULL. |
| Authority | **Module credits: S2.** **Credits of a plan entry: S6**, shown only in the plan view, with a flag when it differs from the catalog. No per-program credits column on the membership relation; `COALESCE(msp.credits, m.credits)` disappears. |
| Provenance | Single-source column on the module; plan credits stay on the plan row. |

### 5.8 Events and schedule

| | |
|---|---|
| Sources | **S3**, reached only through S2's list of current-semester events. S3 also names its modules („gehört zu Modul") and programs. |
| Code path | `event_provider.go:ScrapeEventsForModule` → `parser/event.go` → `UpsertEvent`, `LinkModuleEvent` (errors ignored). |
| Freshness | 2,278 events, all `SS 2026`, fetched 2026-09-11 … 09-14. S2 was re-fetched on 09-19 and still links SS 2026 events (11101 lists a repeat exam). 3 d cache. |
| Conflicts | **Coverage:** 1,418 modules list events; 0 of the 979 English modules do (P2), so roughly a fifth of the catalog can never get a schedule. 1,264 modules have at least one schedule row. 840 of the 2,278 events are of type „Prüfung" and carry no teaching schedule. 10 `module_events` rows point to 4 module IDs that do not exist (12363, 12365, 12369, 12374). Event type values are combined strings („Vorlesung/Übung", „Seminar/Praktikum"). |
| Authority | **S3** for everything about an event; **S2** decides which events belong to a module; S3's own module list is a cross-check that `validate` reports. **DECIDED (Q5):** every event is keyed by a normalized semester (`2026S`, `2026W`); events are deleted one month after their last date; exam events („Prüfung") are a separate category that is not mixed into the recurring schedule but stays queryable. |
| Provenance | `raw_event_page`; events keep `semester`; module ↔ event links are replaced per module on each S2 fetch. |

### 5.9 Campus

| | |
|---|---|
| Sources | **S3** room text, suffix after the last " - ". |
| Code path | `parser/event.go:parseTermine` (room stored verbatim). Campus is guessed in the frontend with `LIKE '%lg%'`. |
| Freshness | As 5.8. |
| Conflicts | None between sources. Of 3,105 schedule rows: Zentralcampus 1,079 · Campus Senftenberg 934 · Campus Sachsendorf 309 · „Campus Nord" 2 · no room 781. The suffix is unambiguous in every non-empty row. |
| Authority | **S3**, parsed in Go into an enum (`zentralcampus`, `sachsendorf`, `senftenberg`, `nord`), NULL without a room. A module's campus set is the union over its events **of one semester**. It is unknown for modules without schedule rows, not "none". |
| Provenance | Column on the schedule row; raw room text kept. |

### 5.10 FÜS flag

| | |
|---|---|
| Sources | **S4** (list of approved modules). **S2** sentence „Das Modul ist für das Fachübergreifende Studium zugelassen." / "This module has been approved for the general studies." |
| Code path | `UpsertFUESList` sets `is_fues = 1` and `cross_disciplinary = 1`. `UpsertModuleDetail` uses `CASE WHEN excluded.is_fues = 1 THEN 1 ELSE modules.is_fues END`. Both columns always carry the same value. The English sentence is not recognized (P2). |
| Freshness | S4 cache valid until 2026-09-24. |
| Conflicts | S4 288 modules = DB 288. On all 45 cached module pages the sentence agrees with list membership (45 of 45). The flag is sticky: a module removed from the list stays flagged forever (brief S4). |
| Authority | **S4** (it is the complete, official list and costs one request). S2's sentence is the cross-check; a disagreement is a `validate` finding. The flag is **replaced** on every S4 fetch: set for the listed IDs, cleared for all others. One column instead of two. |
| Provenance | `raw_fues_list` rows with `fetched_at`; `v_module.is_fues` = exists in the latest list. |

### 5.11 Prerequisites

| | |
|---|---|
| Sources | **S2** „Empfohlene Voraussetzungen" and „Zwingende Voraussetzungen", free text. |
| Code path | `parser/detail.go:207-211`, `normalizePrereqText` turns „keine", „none", „entfällt" … into `'-'`. Module IDs are extracted later, by consumers, by scanning for 5-digit numbers. |
| Freshness | As S2. |
| Conflicts | Single source. 644 modules have mandatory text (485 with a 5-digit number), 2,527 have recommended text (1,199 with a number). `'-'` is a sentinel. |
| Authority | **S2.** Text kept for display, NULL instead of `'-'`. Referenced modules are extracted **once in Go** into `module_prerequisite(module_id, required_module_id, kind)`, only for numbers that are existing module IDs; the rest stays text. |
| Provenance | Single-source; the link rows are derived and rebuilt on each fetch of the module. |

### 5.12 Successor modules, phase-out, "not offered"

| | |
|---|---|
| Sources | **S2**: a „Nachfolgemodul" row where present, otherwise the regex `Nachfolge…(\d{5})` on the remarks; phase-out from „Auslaufmodul"/"phase-out" and "no longer offered" phrases in heading, number, turnus and remarks. |
| Code path | `parser/detail.go:254-289`, `containsNotOffered`. |
| Freshness | As S2. |
| Conflicts | Single source. 23 modules have a successor; 1 successor ID does not exist in `modules`. All 4,868 non-NULL values are JSON arrays; the comma-list form named in the brief no longer occurs in the DB, only in consumer code. Phase-out 76 modules, 31 of them also "not offered". |
| Authority | **S2.** Successors as rows with a foreign key (the dangling one is reported, not stored). `offer_status` enum (`active`, `phase_out`, `not_offered`) instead of two flags that are always set together in one direction. |
| Provenance | Single-source, derived rows. |

### 5.13 Grading and exam form

| | |
|---|---|
| Sources | **S2** has three separate rows: „Modulprüfung" (form: MAP, MCA, …), „Prüfungsleistung/en für Modulprüfung" (details: Klausur 90 min, Hausarbeit …), „Bewertung der Modulprüfung" (graded or not). S6 has a `graded` column from Gemini enrichment, unvalidated. |
| Code path | `parser/detail.go:235-242`, broken by P1. |
| Freshness | As S2. |
| Conflicts | **The brief's "grading is empty for all modules" is P1, not a missing source.** The grading value exists for 4,903 modules, stored in the wrong column: graded 4,620, ungraded 283, none 5. Both live pages in §6 show all three rows. Lost today: exam form and exam details for every module. |
| Authority | **S2** for all three. Normalized: `is_graded` (boolean/NULL), `exam_form` enum from „Modulprüfung" (MAP, MCA, prerequisite + MAP …), exam-type flags (Klausur, mündlich, Hausarbeit, Beleg, Projekt, Präsentation) parsed in Go from the details text. Whether the filter stays is **Q6**; the data question behind it is answered. |
| Provenance | Single-source. |

### 5.14 Short names (2026-09-25)

| | |
|---|---|
| Sources | **None states them.** Rooms: S3's room text, and for the building tokens the legend of BTU's campus plan of the Zentralcampus (`20211119_Campusplan_Zentralcampus_Legende.pdf` on `www-docs.b-tu.de`, linked from https://www.b-tu.de/campusplan/zentralcampus-cottbus), which prints HG, LG 1A, VG 1C, LB 4B, FZ 3E, IKMZ, MZG …; QIS itself prints a few (`HG 0.16`, `ZB VI.01`, `LH 3D`). Modules: a search of every source on 2026-09-25 found no abbreviation field — none in the QIS module description or table, none on the module pages, none in the event numbers or titles. 23 modules use an acronym on their own page or in their events (IR, PuI 1, ERP, CCS, GIS …); 16 of them are what the rules derive anyway. |
| Code path | `normalize.RoomShort` → `event_date.room_short`; `internal/abbrev` → `module_abbrev`, `program_module_abbrev`. Both in `build`, from the canonical tables, without a request. |
| Rejected | **Plan position codes** (BP23, OM3, E3-B) and „Kurzbezeichnung" codes (D1.1, KA 3.1) that Fakultät 4 and 6 print: they name a slot of one program's plan, not a module, depend on the program and the PO, and the event titles write them inconsistently. They could become a `plan_code` of their own later. **Program area abbreviations as reserved words:** only 11 of 28,424 pairs coincide with an area abbreviation of their program. |
| Authority | **Derived** (`docs/schema-v2.md`, „Short names"). Where the initials of all words of a title make exactly three characters, function words small, they are the first derived candidate (the owner, 2026-09-25, §11: EvS, AuP, GdW). A curated file, `internal/abbrev/overrides.tsv`, gives the owner's examples, a few common forms and three that a module's own page uses (IR, PuI, OOP) as a first candidate that beats every derived one; between two lines in a program the resolution decides. `internal/abbrev/blocked.tsv` lists forms that are never derived (SS, KKK, PO …). A room building that the table lacks keeps its QIS name, and the build warns. |
| Provenance | Rebuilt by every build; never stored by a consumer. `program_module_abbrev.is_override`, `choice` and `is_twin` say how a form came about. |

## 6. Spot-check log

Only two live requests were made: 2026-09-19 16:43 CEST, 2 s apart, with the scraper's
User-Agent. Both responses were written to `.cache` under the scraper's key (`module:<id>`,
7 d TTL), so the scraper will not request them again this week. Everything else was checked
against the DB and already cached pages. No bulk crawl.

| Check | Stored in DB | Live / cached source | Result |
|---|---|---|---|
| 11861 Operating Systems II, membership in Informatik B.Sc. (tree yes, page "no") | 0 program assignments | Live page lists 6 entries, including "Bachelor (research-oriented) / Informatik / PO 2008 - 2. SÄ 2024" | Sources agree. Parser defect P2. |
| 11861 and 11881, kind in Informatik B.Sc. | tree: `Pflicht` | Live remarks: "Compulsory elective module in complex ‚Angewandte und technische Informatik'" / "… ‚Grundlagen der Informatik'" | Tree value is the P4 default. The module page states the kind. |
| 11881, limitation | `''` | Live: "Limited Number of Participants: 80" | P2. |
| 11861, 11881, 11101, 11377: grading | `grading = ''`, `exam_type = 'Prüfungsleistung - benotet'` | Pages have separate rows for exam form, exam details and grading | P1. |
| 11861, 11881: events | 0 event links | Live: "Components to be offered in the Current Semester: 121033 Examination …" | P2. |
| 11101 (cached): remarks | not parsed | „Studiengang Mathematik B.Sc.: Pflichtmodul im Komplex ‚Grundlagen'" … | Remarks carry kind and area per program, and the B.Sc. label. |
| FÜS sentence vs S4 list, 45 cached pages | – | 45 of 45 agree | No conflict. |
| S4 credits vs S2, 288 modules | – | 0 differ | No conflict. |

## 7. Owner decisions (2026-09-19), with the evidence they rest on

**Q1. What does "module belongs to program X" mean?**
Evidence: 5.3. The page and the tree agree for regular modules (90–94 %); the extra page pairs
are FÜS modules listing the programs admitted to them (9,752 of 10,153). The page also lists
elective memberships, not only mandatory ones.
Decision: two relations. **Curricular** = all modules a student can attend for the degree, i.e.
everything QIS lists for the program (tree ∪ module page of non-FÜS modules ∪ validated plan).
**FÜS of a program** = FÜS-flagged modules whose page admits the program, minus the program's
curricular modules. Each program has its own allowed FÜS list.

**Q2. Conflicting module kind (tree vs plan).**
Evidence: 5.4. Of 438 conflicting pairs, the large groups come from the tree's `Pflicht`
default (P4); the live check sided with the plan and the module page.
Decision: the PDF plan has precedence, because the tree's `Pflicht` is a default. General rule
for all facts: **specified information wins over inferred information.**

**Q3. Legacy `ai_statute_scan` data.**
Evidence: brief A3: 7,100 of 7,247 rows are QIS-tree memberships with the wrong label; the
rest (147) and the 1,783 `curriculum_unassigned` rows have no verifiable origin. Their semester
and credits are the `0` sentinels.
Decision: remove everything that is legacy. The dataset must contain no trial-and-error
artifacts. Consequence: schema v2 is built as a fresh database from the sources; only the
validated PDF plans (S6) are carried over from the old file.

**Q4. Degree labels.**
Evidence: 5.2. S5 has no B.Sc./B.A. information, so "B.Sc." for Soziale Arbeit is invented.
The label does exist in the sources, but only as free text: module remarks mention B.Sc. 554
times, M.Sc. 740, B.A. 13, B.Eng. 9, M.A. 5, and the statute titles name the degree.
Decision: make labels as readable as possible and convert **before** the value enters the
database, but fault tolerance comes first, and a newly added program must work without a
scraper update. Consequence: no hand-maintained list is required. Level, type and variant are
parsed from the degree string by rules with a generic fallback (`other`, raw text kept). The
short label (B.Sc. …) is derived from the sources (module remarks „Studiengang X B.Sc.",
statute titles) by majority, and is NULL when nothing states it; the display label then falls
back to „Bachelor" / „Master". An optional `curated` override table may correct single
programs but is never needed for a new one.

**Q5. Events: which semesters, and what is "current"?**
Evidence: 5.8. One semester only (SS 2026); S2 exposes only what QIS calls the current
semester; the winter semester starts 2026-10-01.
Decision: BTU publishes the next semester late, while students already want to plan it. Events
are keyed by semester and deleted automatically one month after their last date. Exam events
are treated differently: not shown among the recurring events, but still accessible.
Consequence to keep in mind: between the purge of a semester's teaching events and the
publication of the next semester there is a gap with no schedule data; the turnus facet
(winter/summer, parity) is what the planner can rely on in that gap. The retention period is a
config value.

**Q6. Grading: scrape or drop?**
Evidence: 5.13. The source has it for 4,903 of 4,908 modules; it was lost to P1.
Decision: keep the filter. Fix P1.

**Q7. One polite re-crawl of the module pages.**
P1–P3 cannot be repaired from stored data (P6). About 4,900 requests at 500 ms ±30 % in the
off-peak window, the refresher's normal pace. Best done once, *after* the v2 staging table
exists, so the raw rows are kept and no later parser fix needs the network.
Decision: approved for `b-tu.de/modul/<id>`, at about four times the refresher's pace
(≈ 8 requests/s overall). Not approved here: bulk re-crawls of QIS event or tree pages.

## 8. Reproducing the numbers

All numbers come from read-only queries against `btu_modules.db`
(`sqlite3.connect("file:btu_modules.db?mode=ro", uri=True)`), for example:

```sql
-- P1: grading stored in exam_type
SELECT exam_type, COUNT(*) FROM modules GROUP BY 1 ORDER BY 2 DESC;

-- P2: English modules without assignments / limitation
SELECT language, json_array_length(study_programs) = 0 AS no_programs,
       COALESCE(limitation, '') = '' AS no_limitation, COUNT(*)
FROM modules GROUP BY 1, 2, 3;

-- P4: Pflicht rows without an explicit label in the stored path
SELECT module_type,
       LOWER(COALESCE(subject_area,'') || ' ' || COALESCE(specialization,'') || ' ' ||
             COALESCE(study_section,'')) LIKE '%pflicht%' AS explicit_label,
       COUNT(*)
FROM program_curriculum_modules WHERE source_file = 'qis_tree' GROUP BY 1, 2;

-- 5.4: kind conflicts, row pairs (824 of 3,194)
SELECT COUNT(*), SUM(q.module_type <> p.module_type)
FROM program_curriculum_modules q
JOIN program_curriculum_modules p
  ON p.program_id = q.program_id AND p.module_id = q.module_id
WHERE q.source_file = 'qis_tree' AND COALESCE(p.source_file,'') <> 'qis_tree'
  AND q.module_id <> '';
```

The page-vs-tree pair sets in 5.3 are built from `modules.study_programs`
(`official_program_id`, excluding „Abschluss im Ausland" and „keine Zuordnung vorhanden")
and from `program_curriculum_modules WHERE source_file = 'qis_tree' AND module_id <> ''`.
These set comparisons become `validate` checks in deliverable 5, with the numbers above as
fixtures. Expect them to move after the re-crawl: tree-only should fall from 3,155 to about
1,200, and the 1,565 wrong Lehramt links should disappear.

## 9. Additional owner requirement

**Q8.** The database must ship with views so that the frontend can use simple queries. This is
deliverable 3 of the brief: the views are part of the exported snapshot and are the only read
contract.

## 10. The module description moved to QIS (2026-09-20)

**What was wrong.** `b-tu.de/modul/<id>` is not a source: it is a copy that BTU's CMS renders
from QIS, and the copy lags. On 2026-09-20, with QIS already on WiSe 2026/27, every one of the
4,908 module pages was re-fetched and **not one had changed**: they still named the events of
SoSe 2026. Module 11289 „Softwaretechnik" linked `veranstid=145503` (Prüfung SS 2026) while QIS
listed `veranstid=153213` for the same Veranstaltungsnummer 120632, in WS 2026/27. Analysis I
(11103) showed one repeat exam on the copy and its whole winter schedule in QIS — lecture
130930, exercise 130931, exam 130932, with rooms and times (Mo 09:15, HG 3.45, 05.10.2026 …
25.01.2027). Since the event crawl only ever sees what a module description links, the winter
semester could not enter the catalog at all through the copy.

**The source now.** The description in QIS, `state=modulBeschrDetailInfo&pord.pordnr=<n>`
(`nodeID=auswahlBaum|modul:pordnr=<n>` is required — without it the answer leaves out the
events). It carries every field the copy has, in the same labels and list shapes, plus the
German **and** the English title in one page. Its rows are read by `parser.QISModuleParser`
through the same `applyRow` as the copy, so both end up in one `model.ModuleDetail`.

The module numbers and their `pordnr` come from the QIS module table
(`TableSelectModul.vm`, source `qis_module_list`). The whole table is 27 MB in one response —
more than the crawler keeps of a page (16 MB) — so it is read in chunks of 1,000 rows
(`rows-000000`, `rows-001000`, …) until a chunk is short; chunks behind the end are removed.

| | b-tu.de list | QIS table | both |
|---|---|---|---|
| Modules | 4,908 | 3,237 | 3,209 |
| Only there | 1,699 (1,698 `not_offered`, 1 `phase_out`) | 28 | |
| Active modules missing | 0 | 0 | |

**Precedence.** QIS wins for every module it has a description for; `module.description_source`
states `qis` or `btu_cms` per module, and `source_url` is the page the fields were read from —
the QIS address for a QIS-sourced module, not the nicer `b-tu.de/modul/<id>`. The 1,698 modules
QIS no longer lists keep their copy, so old regulations and study plans do not lose their modules.

**Events are the exception: both descriptions count.** QIS names only the semester that runs
now, while the copy still names the exams of the one that is ending — on 2026-09-20 the repeat
exams of SoSe 2026, some of them days away. An event states its own semester, so both sets of
links are kept (`mergeEventLinks`); retention removes an event 30 days after its last date as
before.

**One view per module, in its own language.** A QIS description is written in the language the
module is taught in; the other view is not a translation. The German view of 11191 „EMC in
Electrical Power Installations" states `Lernziele: keine` and `Inhalte: keine` and puts the
English title first, while the English view carries the text. The teaching language is a column
of the module table, so `QISModuleRefs` asks for `objLanguage=en` for the 691 English-taught
modules and `de` for the rest — one request per module, and `isEnglishModulePage` marks the
English ones exactly as it did for the copy, which the build uses to sort the two titles.

**What QIS says and the copy does not.** A phase-out has a row of its own in QIS
(„Auslaufmodul: Nachfolgemodul seit: 20.01.2023" with a link to the successor); 11162
„Wirtschaftsprüfung" was `active` with no remark in the catalog built from the copy. The
successor is read from the link text, never from its address: a QIS link carries the internal
`pordnr` (16532), which is five digits too and would name a module that does not exist.

**The catalog follows the published semester, not the calendar.** With the winter schedule in
(1,496 modules, 2,004 teaching events on 2026-09-21) the summer semester still had ten days to
run, but students plan with the semester they can attend. `meta.current_semester` therefore takes
the later of the calendar semester and the newest one whose schedule is published, where
published means at least 100 modules have a dated teaching event in it. The threshold is what
keeps a single early event from moving the whole catalog, as one Polish course nearly did on
2026-09-19. The value is part of the content digest, or a semester that moves without any other
change would never be exported to a browser.

## 11. Owner decisions (2026-09-25): short names

The owner asked for short names of rooms and modules where space is tight (the Studienplan's week
grid, agenda, notes and legend; the module overlay; perhaps the calendar export and the catalog).
Evidence and rules: §5.14 and `docs/schema-v2.md`, „Short names".

- **Rooms** are „<Gebäude>/<Raum>[<Attachment>]": Zentrales Hörsaalgebäude is ZHG, a Hörsaal HS
  („ZHG HS.3"), Lehrgebäude LG, Verfügungsgebäude VG, Mehrzweckgebäude MZG, and the Großer Hörsaal
  GHS, „damit es nicht mit HG verwechselt wird". A room is its floor and number: „VG1C 0.07",
  without a space inside the building token.
- **A slash between building and room** (on the review page, 2026-09-25): „ZHG/HS.C" instead of
  „ZHG HS.C", „weil sich das viel besser liest". So ZHG/HS.C, ZHG/SEM.4, ZHG/AM.1, LG1A/HS.2,
  VG1C/0.03, HG/0.16, LG3A/352, SFB/1.308, SD/9.117, Mensa/0.33.1; a hall that is a building of its
  own stays alone (GHS). No form has a second slash: two rooms QIS writes as a pair or a range are
  joined by „+" (FZ3E/2.26+27, ZB2CD/229+230, LG10/211a+b), and the Lehrgebäude 4/1, 4/3, 4/4 of
  Campus Nord are LG4-1, LG4-3, LG4-4. A room with words keeps its spaces (ZB2CD/AT Oestreich M).
- **Modules** get abbreviations that are actually used — Algorithmieren und Programmieren is
  AuP, Elektrische und Elektronische Grundlagen der Informatik EEG. Three letters are the sweet
  spot; a form that occurs twice among the modules a program lets its students select is nobody's,
  and both fall back. They are computed by Radix, so that they are in the database.
- **Three initials of the whole title win** (on the review page, 2026-09-25, on Entwicklung von
  Softwaresystemen, derived as ESS): „Ja ist bestimmt Entwicklung von Softwaresystemen. Das wird eher
  EvS genannt, ich denke mal, wenn die Buchstaben beim Anagramm passen, dann nimmt man die i. d. R."
  Where the initials of all words of the title make exactly three characters — content words as
  capitals, function words (und, von, der, für, in, mit, zu, an, auf, aus, bei, über …; of, for, the,
  to, in, on) as their lowercase letter — that form is the first choice, ahead of compound parts and
  every other derivation: EvS, AuP, GdW (Grundlagen der Werkstoffe), EuH (Ethik und Handeln), KuL
  (Kommunikation und Lernstrategien). A hyphen part counts as a word (Bau- und Stadtbaugeschichte 1
  → BuS1, Kinder- und Jugendhilfe → KuJ), „&" as und, English „and" as & (M5); a series number is
  appended. Overrides, the blocked and reserved forms and the uniqueness within a program still come
  first. On the data of 2026-09-23 it moved 372 of 4,936 defaults and 2,680 of 28,424 pairs; forms a
  reader may miss: Grundlagen der Elektrotechnik GdE (was GET), Signal- und Systemtheorie SuS (SST),
  Kinder- und Jugendhilfe KuJ (KJH). An override line can bring any of them back.
- **They are metadata, not facts.** „Und wenn das mal nicht passt mit dem, wie es im Studiengang
  verändert wird. Egal, dann machen wir die meta eben neu": every build derives them again, and
  a consumer never stores one.

Open, with the default the build uses until the owner decides:

| | Question | Default |
|---|---|---|
| R1 | Audimax as `ZHG/AM.1`, `ZHG/Audimax 1` or `ZHG/AX1`? | `ZHG/AM.1` |
| R2 | Three-digit numbers as printed (`LG3A/324`) or as floor.number (`LG3A/3.24`)? | as printed |
| R3 | Senftenberg and Sachsendorf as `SFB/1.308` / `SD/7.116`? | yes |
| R4 | The annexes of LG 2C and 2D folded into `LG2C` / `LG2D`? | folded |
| R5 | Ateliers keep „AT" (`ZB2CD/AT Oestreich M`)? | keep |
| R6 | Invented tokens PRH, SH1 and bare outdoor places (`Fakultätsgarten`)? | as listed |
| R7 | Senftenberg rooms lose their description (`SFB/1.210`, not the Skills Lab's name)? | drop it; the full name stays in the tooltip |
| R8 | Sports hall fields as `SFB/9.151 F2`? | `F2` |
| M1 | Two-word titles with three characters (`ThI`, `EAl`, `NMa`) or two initials (`TI`, `EA`, `NM`)? A word's second letter between two capitals reads as a function word, as the u of AuP does (`EfA` next to `SfA` „Statistik für Anwender“), so the rules widen the last word instead (review, 2026-09-25) | three, without a function-looking letter; `TI` and the like can be overrides |
| M2 | The displayed title (English for English-taught modules: `ERTS`) or always the German one? | the displayed title |
| M3 | Identical titles in one program: `HäG` / `HäG-b`? | `-b`, `-c` |
| M4 | A contested form goes to neither module (`GMa` / `GMi`) or first come, first served? | neither |
| M5 | English „and" as `&` (`A&M`)? | `&` |
| M6 | Language courses as `DaF-B1.1`? | yes |
| M7 | Which other forms are well known (TI, SE …)? | only those in `overrides.tsv` |
| M8 | The program-free form on catalog cards and on a module page without a program? | yes |
| M9 | Plan position codes (BP23, OM3) as a `plan_code` of their own? | not now |
| M10 | The blocked forms (`internal/abbrev/blocked.tsv`): SA is blocked for the Nazi SA, but it is the common form of „Studienarbeit“ (5 modules, now `Stu`), and SS of „Steuerungssysteme“ (now `Ste`). An override line may bring a blocked form back. | blocked; no override |
| M11 | Room kinds as module forms: Sem („Seminar“) next to `ZHG/SEM.4` (owner 2026-09-25: a Seminarraum is „SEM“), AT (Analogtechnik, Architekturtheorie …) next to the ateliers `ZB2CD/AT Oestreich M`? They always follow a building token, so only the buildings (and HS, the building HS3) are blocked. | allowed |
