-- Canonical model. Conventions:
--   * NULL means unknown. There are no 0 / '' / '-' placeholders.
--   * Every filterable attribute is a normalized column (enum via CHECK, or 0/1 flag);
--     the source text is kept next to it as *_raw for display.
--   * Everything in this file except the plan_* tables is derived from raw_page by the
--     build step and is replaced as a whole on each build.
--   * Facts that several sources state (program membership, module kind) are stored as
--     one assertion row per source. No source overwrites another.

CREATE TABLE meta (
	key   TEXT PRIMARY KEY,
	value TEXT NOT NULL
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Modules (source: module page; FÜS flag: QIS FÜS list)
-- ---------------------------------------------------------------------------

CREATE TABLE department (
	id      INTEGER PRIMARY KEY,
	code    TEXT,              -- '1' … '6', 'GW', 'ZES' …; NULL when the source names no unit code
	name_de TEXT,
	name_en TEXT,
	label   TEXT NOT NULL      -- display name: German if known, else English
);

CREATE TABLE module (
	id                        TEXT PRIMARY KEY,
	title                     TEXT NOT NULL,      -- display title: title_de, else title_en
	title_de                  TEXT,
	title_en                  TEXT,
	detail_status             TEXT NOT NULL CHECK (detail_status IN ('ok', 'missing')),
	page_lang                 TEXT CHECK (page_lang IN ('de', 'en')),
	department_id             INTEGER REFERENCES department(id),
	department_raw            TEXT,
	credits                   REAL,
	language_raw              TEXT,
	teaches_german            INTEGER CHECK (teaches_german IN (0, 1)),
	teaches_english           INTEGER CHECK (teaches_english IN (0, 1)),
	duration_raw              TEXT,
	duration_semesters        INTEGER,
	turnus_raw                TEXT,
	turnus_season             TEXT CHECK (turnus_season IN ('winter', 'summer', 'both', 'irregular')),
	turnus_parity             TEXT CHECK (turnus_parity IN ('even', 'odd')),
	offer_status              TEXT NOT NULL CHECK (offer_status IN ('active', 'phase_out', 'not_offered')),
	limitation_raw            TEXT,
	is_limited                INTEGER CHECK (is_limited IN (0, 1)),
	participant_limit         INTEGER,            -- NULL: unlimited, unknown, or limited without a number
	exam_form_raw             TEXT,
	exam_form                 TEXT CHECK (exam_form IN ('map', 'prereq_map', 'mca', 'prereq_mca', 'other')),
	exam_details              TEXT,
	exam_written              INTEGER CHECK (exam_written IN (0, 1)),       -- exam_* flags are NULL without exam_details
	exam_oral                 INTEGER CHECK (exam_oral IN (0, 1)),
	exam_paper                INTEGER CHECK (exam_paper IN (0, 1)),
	exam_presentation         INTEGER CHECK (exam_presentation IN (0, 1)),
	exam_project              INTEGER CHECK (exam_project IN (0, 1)),
	exam_practical            INTEGER CHECK (exam_practical IN (0, 1)),
	grading_raw               TEXT,
	is_graded                 INTEGER CHECK (is_graded IN (0, 1)),
	is_fues                   INTEGER NOT NULL CHECK (is_fues IN (0, 1)),   -- on the latest QIS FÜS list
	page_states_fues          INTEGER CHECK (page_states_fues IN (0, 1)),   -- cross-check only
	learning_outcomes         TEXT,
	contents                  TEXT,
	prerequisites_recommended TEXT,
	prerequisites_mandatory   TEXT,
	remarks                   TEXT,
	source_url                TEXT,
	fetched_at                TEXT
);

CREATE INDEX idx_module_department ON module(department_id);
CREATE INDEX idx_module_credits ON module(credits);

CREATE TABLE module_person (
	module_id TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	ord       INTEGER NOT NULL,
	name      TEXT    NOT NULL,   -- „Köhler, Ekkehard"
	title     TEXT,               -- „Prof. Dr. rer. nat. habil."
	PRIMARY KEY (module_id, ord)
) WITHOUT ROWID;

CREATE INDEX idx_module_person_name ON module_person(name);

CREATE TABLE module_teaching_form (
	module_id    TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	ord          INTEGER NOT NULL,
	form         TEXT    NOT NULL CHECK (form IN ('lecture', 'exercise', 'seminar', 'practical', 'project',
	                                              'tutorial', 'consultation', 'excursion', 'self_study', 'paper', 'other')),
	form_raw     TEXT    NOT NULL,
	workload_raw TEXT,
	sws          REAL,
	hours        REAL,
	PRIMARY KEY (module_id, ord)
) WITHOUT ROWID;

CREATE INDEX idx_module_teaching_form_form ON module_teaching_form(form, module_id);

-- Display-only lists of a module page.
CREATE TABLE module_text_item (
	module_id TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	kind      TEXT    NOT NULL CHECK (kind IN ('literature', 'course')),
	ord       INTEGER NOT NULL,
	text      TEXT    NOT NULL,
	PRIMARY KEY (module_id, kind, ord)
) WITHOUT ROWID;

-- Module IDs named in the prerequisite texts, kept only when the module exists.
CREATE TABLE module_prerequisite (
	module_id          TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	required_module_id TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	kind               TEXT NOT NULL CHECK (kind IN ('mandatory', 'recommended')),
	PRIMARY KEY (module_id, required_module_id, kind)
) WITHOUT ROWID;

CREATE TABLE module_successor (
	module_id    TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	successor_id TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	PRIMARY KEY (module_id, successor_id)
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Programs (source: QIS tree)
-- ---------------------------------------------------------------------------

CREATE TABLE program (
	id                 TEXT PRIMARY KEY,          -- <stg>-<abschl>-<pversion>, e.g. 079-82-2008
	slug               TEXT NOT NULL UNIQUE,      -- readable URL key, e.g. bsc-informatik-2008
	name               TEXT NOT NULL,
	stg_code           TEXT NOT NULL,
	abschl_code        TEXT NOT NULL,
	degree_raw         TEXT NOT NULL,             -- „Bachelor (universitär) - Duales Studium, praxisintegrierend"
	degree_level       TEXT NOT NULL CHECK (degree_level IN ('bachelor', 'master', 'teaching_bachelor', 'teaching_master',
	                                                         'doctoral', 'none', 'other')),
	degree_type        TEXT CHECK (degree_type IN ('university', 'applied')),
	study_variant      TEXT CHECK (study_variant IN ('dual_practice', 'dual_training', 'double_degree', 'extended',
	                                                 'reduced', 'distance', 'part_time', 'other')),   -- NULL: regular
	degree_label       TEXT,                      -- „B.Sc." when a source states it, else NULL
	degree_label_basis TEXT CHECK (degree_label_basis IN ('stated', 'curated')),
	po_version         TEXT NOT NULL,             -- „2008 - 2. SÄ 2024"
	po_year            INTEGER,
	po_amendment       TEXT,
	family_key         TEXT NOT NULL,             -- same program and degree across PO versions
	name_key           TEXT NOT NULL,             -- same subject across degree levels (Bachelor/Master counterpart)
	is_latest_po       INTEGER NOT NULL CHECK (is_latest_po IN (0, 1)),
	source_url         TEXT NOT NULL,
	fetched_at         TEXT NOT NULL
);

CREATE INDEX idx_program_family ON program(family_key);
CREATE INDEX idx_program_name_key ON program(name_key);

CREATE TABLE program_document (
	program_id TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	ord        INTEGER NOT NULL,
	title      TEXT    NOT NULL,
	doc_type   TEXT    NOT NULL CHECK (doc_type IN ('statute', 'amendment', 'other')),
	url        TEXT    NOT NULL,
	PRIMARY KEY (program_id, ord)
) WITHOUT ROWID;

-- The area nodes below a PO, as QIS books them („Fachstudium" → „Praktische Informatik").
CREATE TABLE program_area (
	id          INTEGER PRIMARY KEY,
	program_id  TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	parent_id   INTEGER REFERENCES program_area(id) ON DELETE CASCADE,
	ord         INTEGER NOT NULL,
	depth       INTEGER NOT NULL,                 -- 1 = directly below the PO
	label       TEXT    NOT NULL,
	path        TEXT    NOT NULL,                 -- labels from the PO down, joined by ' / ', bookkeeping nodes removed
	section     TEXT CHECK (section IN ('basic', 'main', 'specialization', 'core')),  -- Grund-/Fach-/Vertiefungs-/Kernstudium
	stated_kind TEXT CHECK (stated_kind IN ('compulsory', 'elective', 'thesis', 'internship', 'fues')),  -- only if a label on the path says so
	source_url  TEXT    NOT NULL
);

CREATE INDEX idx_program_area_program ON program_area(program_id);

-- ---------------------------------------------------------------------------
-- Validated study plans (source: statute PDFs; written by scan-curriculum, never by the build)
--
-- A plan is expensive to produce and is not derived from raw_page. It therefore must
-- survive a build in which its program or modules are missing (e.g. after an incomplete
-- tree crawl): program_id and module_id are deliberately not foreign keys into the derived
-- tables. The views join them, and validate reports plans without a program.
-- ---------------------------------------------------------------------------

CREATE TABLE plan (
	program_id   TEXT PRIMARY KEY,
	source_file  TEXT NOT NULL,
	layout_json  TEXT NOT NULL,
	validated_at TEXT NOT NULL
);

CREATE TABLE plan_entry (
	id              INTEGER PRIMARY KEY,
	program_id      TEXT    NOT NULL REFERENCES plan(program_id) ON DELETE CASCADE,
	ord             INTEGER NOT NULL,
	module_id       TEXT,                         -- catalog module the row was matched to
	module_code_raw TEXT,
	module_name     TEXT    NOT NULL,
	semester        INTEGER,                      -- exact semester; NULL when the PDF only gives a span or nothing
	start_semester  INTEGER,
	end_semester    INTEGER,
	semester_span   TEXT,                         -- „5-6" as printed
	credits         REAL,
	min_credits     REAL,
	max_credits     REAL,
	kind            TEXT CHECK (kind IN ('compulsory', 'elective', 'thesis', 'internship', 'fues')),
	kind_raw        TEXT,
	study_section   TEXT,
	subject_area    TEXT,
	area_rules      TEXT,
	specialization  TEXT,
	source_evidence TEXT,
	UNIQUE (program_id, ord)
);

CREATE INDEX idx_plan_entry_module ON plan_entry(module_id);

CREATE TABLE plan_scan_status (
	program_id TEXT PRIMARY KEY,
	status     TEXT NOT NULL CHECK (status IN ('saved', 'saved_with_warnings', 'needs_review', 'no_plan', 'missing_source')),
	message    TEXT,
	source     TEXT,
	updated_at TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- Program ↔ module (sources: module page, QIS tree, validated plan)
-- ---------------------------------------------------------------------------

-- The triples a module page lists under „Zuordnung zu Studiengängen", resolved or not.
CREATE TABLE module_program_ref (
	module_id      TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	ord            INTEGER NOT NULL,
	degree_raw     TEXT    NOT NULL,
	program_raw    TEXT    NOT NULL,
	po_raw         TEXT    NOT NULL,
	program_id     TEXT REFERENCES program(id) ON DELETE SET NULL,
	resolve_status TEXT    NOT NULL CHECK (resolve_status IN ('resolved', 'abroad', 'unresolved')),
	PRIMARY KEY (module_id, ord)
) WITHOUT ROWID;

CREATE INDEX idx_module_program_ref_program ON module_program_ref(program_id);

-- One row per statement "module M is part of program P" per source (and per area, because
-- the tree may book a module under several areas of one program).
CREATE TABLE program_module_assertion (
	id         INTEGER PRIMARY KEY,
	program_id TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id  TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	source     TEXT NOT NULL CHECK (source IN ('module_page', 'qis_tree', 'pdf_plan')),
	area_id    INTEGER REFERENCES program_area(id) ON DELETE CASCADE,   -- qis_tree only
	area_label TEXT,                                                    -- module_page remarks / pdf_plan heading
	kind       TEXT CHECK (kind IN ('compulsory', 'elective', 'thesis', 'internship', 'fues')),
	kind_basis TEXT CHECK (kind_basis IN ('stated', 'inferred')),       -- NULL exactly when kind is NULL
	CHECK ((kind IS NULL) = (kind_basis IS NULL))
);

CREATE INDEX idx_pma_program ON program_module_assertion(program_id, module_id);
CREATE INDEX idx_pma_module ON program_module_assertion(module_id);

-- ---------------------------------------------------------------------------
-- Events (source: QIS event pages)
-- ---------------------------------------------------------------------------

CREATE TABLE semester (
	key       TEXT PRIMARY KEY,                   -- '2026S', '2026W' (winter keyed by its starting year); sorts chronologically
	season    TEXT    NOT NULL CHECK (season IN ('summer', 'winter')),
	year      INTEGER NOT NULL,
	label     TEXT    NOT NULL,                   -- „SoSe 2026", „WiSe 2026/27"
	starts_on TEXT    NOT NULL,                   -- 04-01 / 10-01
	ends_on   TEXT    NOT NULL                    -- 09-30 / 03-31
) WITHOUT ROWID;

CREATE TABLE event (
	id               TEXT PRIMARY KEY,            -- QIS veranstid
	number           TEXT,
	title            TEXT NOT NULL,
	type_raw         TEXT,
	category         TEXT NOT NULL CHECK (category IN ('teaching', 'exam', 'other')),
	semester_key     TEXT REFERENCES semester(key),
	sws              REAL,
	max_participants INTEGER,
	first_date       TEXT,                        -- ISO dates over all event_date rows
	last_date        TEXT,                        -- retention: the event is deleted one month after this
	source_url       TEXT NOT NULL,
	fetched_at       TEXT NOT NULL
);

CREATE INDEX idx_event_semester ON event(semester_key, category);

CREATE TABLE event_form (
	event_id TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	form     TEXT NOT NULL CHECK (form IN ('lecture', 'exercise', 'seminar', 'practical', 'project',
	                                       'tutorial', 'consultation', 'excursion', 'self_study', 'paper', 'other')),
	PRIMARY KEY (event_id, form)
) WITHOUT ROWID;

CREATE TABLE event_person (
	event_id TEXT    NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	ord      INTEGER NOT NULL,
	name     TEXT    NOT NULL,
	role     TEXT,
	PRIMARY KEY (event_id, ord)
) WITHOUT ROWID;

CREATE TABLE event_date (
	id              INTEGER PRIMARY KEY,
	event_id        TEXT    NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	ord             INTEGER NOT NULL,
	group_name      TEXT,
	weekday         INTEGER CHECK (weekday BETWEEN 1 AND 7),   -- 1 = Monday
	start_time      TEXT,                                      -- 'HH:MM'
	end_time        TEXT,
	rhythm          TEXT CHECK (rhythm IN ('weekly', 'week_a', 'week_b', 'single', 'block', 'other')),
	rhythm_raw      TEXT,
	first_date      TEXT,
	last_date       TEXT,
	room            TEXT,
	campus          TEXT CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	instructor      TEXT,
	comment         TEXT,
	cancelled_dates TEXT
);

CREATE INDEX idx_event_date_event ON event_date(event_id);

CREATE TABLE module_event (
	module_id TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	event_id  TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	PRIMARY KEY (module_id, event_id)
) WITHOUT ROWID;

CREATE INDEX idx_module_event_event ON module_event(event_id);
