-- Read contract. Consumers (browser app, SSR server, CLI) read these views and nothing else.
--
-- Two views hold logic that is expensive to evaluate per query in the browser
-- (v_program_module_src, v_module_facets_src). The build materializes them into
-- program_module and module_facet; the public views read those tables. The logic
-- still lives in exactly one place: the *_src view.

-- ---------------------------------------------------------------------------
-- Program ↔ module: membership, relation and kind by precedence
-- ---------------------------------------------------------------------------

CREATE TABLE program_module (
	program_id     TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id      TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	relation       TEXT    NOT NULL CHECK (relation IN ('curricular', 'fues')),
	kind           TEXT CHECK (kind IN ('compulsory', 'elective', 'thesis', 'internship', 'fues')),
	kind_source    TEXT CHECK (kind_source IN ('pdf_plan', 'module_page', 'qis_tree')),
	kind_basis     TEXT CHECK (kind_basis IN ('stated', 'inferred')),
	precedence     INTEGER,            -- rank of the statement that decided kind: 1 = strongest; NULL without kind
	area           TEXT,
	section        TEXT,
	in_tree        INTEGER NOT NULL CHECK (in_tree IN (0, 1)),
	on_module_page INTEGER NOT NULL CHECK (on_module_page IN (0, 1)),
	in_plan        INTEGER NOT NULL CHECK (in_plan IN (0, 1)),
	PRIMARY KEY (program_id, module_id)
) WITHOUT ROWID;

CREATE INDEX idx_program_module_module ON program_module(module_id);
CREATE INDEX idx_program_module_relation ON program_module(program_id, relation, kind);

-- Precedence of a statement about the kind of a module in a program:
--   a stated value beats an inferred one; among equals: pdf_plan > module_page > qis_tree.
-- relation:
--   'fues'       the module is on the FÜS list, its page admits the program, and neither the
--                program's tree nor its validated plan contains it
--   'curricular' everything else a source places in the program
CREATE VIEW v_program_module_src AS
WITH statement AS (
	SELECT a.id, a.program_id, a.module_id, a.source, a.kind, a.kind_basis, a.area_label,
	       pa.path AS area_path, pa.section, pa.ord AS area_ord,
	       (CASE a.kind_basis WHEN 'stated' THEN 0 ELSE 3 END)
	         + (CASE a.source WHEN 'pdf_plan' THEN 1 WHEN 'module_page' THEN 2 ELSE 3 END) AS precedence
	FROM program_module_assertion a
	LEFT JOIN program_area pa ON pa.id = a.area_id
),
pair AS (
	SELECT program_id, module_id,
	       MAX(source = 'qis_tree')    AS in_tree,
	       MAX(source = 'module_page') AS on_module_page,
	       MAX(source = 'pdf_plan')    AS in_plan
	FROM statement
	GROUP BY program_id, module_id
),
best AS (
	-- the deciding kind statement per pair; compulsory wins a tie inside one source
	SELECT s.program_id, s.module_id, s.kind, s.source, s.kind_basis, s.precedence
	FROM statement s
	WHERE s.kind IS NOT NULL
	  AND s.id = (
		SELECT s2.id FROM statement s2
		WHERE s2.program_id = s.program_id AND s2.module_id = s.module_id AND s2.kind IS NOT NULL
		ORDER BY s2.precedence,
		         CASE s2.kind WHEN 'compulsory' THEN 1 WHEN 'thesis' THEN 2 WHEN 'internship' THEN 3 WHEN 'elective' THEN 4 ELSE 5 END,
		         s2.id
		LIMIT 1)
)
SELECT
	p.program_id,
	p.module_id,
	CASE WHEN m.is_fues = 1 AND p.in_tree = 0 AND p.in_plan = 0 THEN 'fues' ELSE 'curricular' END AS relation,
	CASE WHEN m.is_fues = 1 AND p.in_tree = 0 AND p.in_plan = 0 THEN 'fues' ELSE b.kind END       AS kind,
	CASE WHEN m.is_fues = 1 AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.source END       AS kind_source,
	CASE WHEN m.is_fues = 1 AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.kind_basis END   AS kind_basis,
	CASE WHEN m.is_fues = 1 AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.precedence END   AS precedence,
	-- area: the tree is the authority for structure; plan heading and page remark are fallbacks
	COALESCE(
		(SELECT s.area_path FROM statement s
		 WHERE s.program_id = p.program_id AND s.module_id = p.module_id AND s.area_path IS NOT NULL
		 ORDER BY s.area_ord LIMIT 1),
		(SELECT s.area_label FROM statement s
		 WHERE s.program_id = p.program_id AND s.module_id = p.module_id AND s.area_label IS NOT NULL
		 ORDER BY s.precedence LIMIT 1)) AS area,
	(SELECT s.section FROM statement s
	 WHERE s.program_id = p.program_id AND s.module_id = p.module_id AND s.section IS NOT NULL
	 ORDER BY s.area_ord LIMIT 1) AS section,
	p.in_tree, p.on_module_page, p.in_plan
FROM pair p
JOIN module m ON m.id = p.module_id
LEFT JOIN best b ON b.program_id = p.program_id AND b.module_id = p.module_id;

-- "Modules of program X", "programs of module Y". One row per pair.
CREATE VIEW v_program_module AS
SELECT
	pm.program_id, pm.module_id, pm.relation, pm.kind, pm.kind_source, pm.kind_basis, pm.precedence,
	pm.area, pm.section, pm.in_tree, pm.on_module_page, pm.in_plan,
	m.title AS module_title, m.credits AS module_credits, m.offer_status, m.turnus_season,
	(SELECT MIN(e.semester) FROM plan_entry e
	 WHERE e.program_id = pm.program_id AND e.module_id = pm.module_id) AS plan_semester
FROM program_module pm
JOIN module m ON m.id = pm.module_id;

-- Every place the QIS tree books a module in a program (a module can sit in several areas).
CREATE VIEW v_program_module_area AS
SELECT a.program_id, a.module_id, pa.id AS area_id, pa.path AS area, pa.label AS area_label,
       pa.depth, pa.ord AS area_ord, pa.section, a.kind, a.kind_basis
FROM program_module_assertion a
JOIN program_area pa ON pa.id = a.area_id
WHERE a.source = 'qis_tree';

-- The module page's view on its programs, including what could not be resolved.
CREATE VIEW v_module_program_link AS
SELECT
	r.module_id, r.ord, r.degree_raw, r.program_raw, r.po_raw, r.resolve_status,
	p.id AS program_id, p.slug AS program_slug, p.name AS program_name,
	COALESCE(p.degree_label, CASE p.degree_level
		WHEN 'bachelor' THEN 'Bachelor' WHEN 'master' THEN 'Master'
		WHEN 'teaching_bachelor' THEN 'Lehramt Bachelor' WHEN 'teaching_master' THEN 'Lehramt Master'
		WHEN 'doctoral' THEN 'Promotion' END) AS degree_display,
	p.po_version, p.is_latest_po,
	pm.relation, pm.kind, pm.kind_source, pm.area
FROM module_program_ref r
LEFT JOIN program p ON p.id = r.program_id
LEFT JOIN program_module pm ON pm.program_id = r.program_id AND pm.module_id = r.module_id;

-- ---------------------------------------------------------------------------
-- Programs
-- ---------------------------------------------------------------------------

CREATE VIEW v_program AS
SELECT
	p.id, p.slug, p.name,
	p.degree_level, p.degree_type, p.study_variant, p.degree_label, p.degree_raw,
	COALESCE(p.degree_label, CASE p.degree_level
		WHEN 'bachelor' THEN 'Bachelor' WHEN 'master' THEN 'Master'
		WHEN 'teaching_bachelor' THEN 'Lehramt Bachelor' WHEN 'teaching_master' THEN 'Lehramt Master'
		WHEN 'doctoral' THEN 'Promotion' END) AS degree_display,
	p.po_version, p.po_year, p.po_amendment, p.family_key, p.name_key, p.is_latest_po,
	p.stg_code, p.abschl_code, p.source_url, p.fetched_at,
	(pl.program_id IS NOT NULL) AS has_plan,
	pl.validated_at             AS plan_validated_at,
	st.status                   AS plan_status,
	(SELECT COUNT(*) FROM program_module pm WHERE pm.program_id = p.id AND pm.relation = 'curricular') AS curricular_modules,
	(SELECT COUNT(*) FROM program_module pm WHERE pm.program_id = p.id AND pm.relation = 'fues')       AS fues_modules,
	(SELECT COUNT(*) FROM program_document d WHERE d.program_id = p.id)                                AS documents
FROM program p
LEFT JOIN plan pl ON pl.program_id = p.id
LEFT JOIN plan_scan_status st ON st.program_id = p.id;

-- Other PO versions of the same program and degree.
CREATE VIEW v_program_version AS
SELECT p.id AS program_id, o.id AS other_id, o.slug AS other_slug, o.po_version, o.po_year, o.is_latest_po
FROM program p
JOIN program o ON o.family_key = p.family_key AND o.id <> p.id;

-- Bachelor ↔ Master of the same subject. Best match first: same degree type and study
-- variant, latest PO. Consumers take the first row per (program_id, counterpart_level).
CREATE VIEW v_program_counterpart AS
SELECT
	p.id AS program_id, o.id AS counterpart_id, o.slug AS counterpart_slug, o.name AS counterpart_name,
	o.degree_level AS counterpart_level, o.po_version AS counterpart_po_version,
	(COALESCE(o.degree_type, '') = COALESCE(p.degree_type, ''))
	  + (COALESCE(o.study_variant, '') = COALESCE(p.study_variant, ''))
	  + o.is_latest_po AS match_score
FROM program p
JOIN program o ON o.name_key = p.name_key AND o.degree_level <> p.degree_level
WHERE (p.degree_level = 'bachelor' AND o.degree_level = 'master')
   OR (p.degree_level = 'master' AND o.degree_level = 'bachelor')
   OR (p.degree_level = 'teaching_bachelor' AND o.degree_level = 'teaching_master')
   OR (p.degree_level = 'teaching_master' AND o.degree_level = 'teaching_bachelor');

CREATE VIEW v_program_document AS
SELECT program_id, ord, title, doc_type, url FROM program_document;

-- Validated semester data only. A program without a validated plan has no rows here.
CREATE VIEW v_program_plan_entry AS
SELECT
	e.program_id, e.ord, e.module_id, e.module_code_raw, e.module_name,
	e.semester, e.start_semester, e.end_semester, e.semester_span,
	e.credits, e.min_credits, e.max_credits, e.kind, e.kind_raw,
	e.study_section, e.subject_area, e.area_rules, e.specialization, e.source_evidence,
	m.title AS catalog_title, m.credits AS catalog_credits,
	(e.credits IS NOT NULL AND m.credits IS NOT NULL AND e.credits <> m.credits) AS credits_differ_from_catalog
FROM plan_entry e
LEFT JOIN module m ON m.id = e.module_id;

CREATE VIEW v_program_plan AS
SELECT pl.program_id, pl.source_file, pl.layout_json, pl.validated_at
FROM plan pl
JOIN program p ON p.id = pl.program_id;

-- Coverage per program: how much of each source exists. Replaces the v1 program_coverage view.
CREATE VIEW program_coverage AS
SELECT
	p.id AS program_id, p.name AS program_name, p.degree_raw AS degree, p.po_version,
	(SELECT COUNT(*) FROM program_module pm WHERE pm.program_id = p.id AND pm.in_tree = 1)        AS tree_modules,
	(SELECT COUNT(*) FROM program_module pm WHERE pm.program_id = p.id AND pm.on_module_page = 1
	                                          AND pm.relation = 'curricular')                     AS page_modules,
	(SELECT COUNT(*) FROM plan_entry e WHERE e.program_id = p.id)                                 AS plan_entries,
	(SELECT COUNT(*) FROM plan_entry e WHERE e.program_id = p.id AND e.module_id IS NOT NULL)     AS plan_entries_linked,
	(SELECT COUNT(*) FROM program_module pm WHERE pm.program_id = p.id AND pm.relation = 'curricular'
	                                          AND pm.kind IS NULL)                                AS modules_without_kind,
	COALESCE(st.status, 'not_scanned') AS plan_status
FROM program p
LEFT JOIN plan_scan_status st ON st.program_id = p.id;

-- ---------------------------------------------------------------------------
-- Modules
-- ---------------------------------------------------------------------------

CREATE TABLE module_facet (
	module_id          TEXT PRIMARY KEY REFERENCES module(id) ON DELETE CASCADE,
	credits            REAL,
	department_id      INTEGER,
	teaches_german     INTEGER,
	teaches_english    INTEGER,
	duration_semesters INTEGER,
	offered_winter     INTEGER,        -- turnus says winter or every semester; NULL when irregular/unknown
	offered_summer     INTEGER,
	turnus_season      TEXT,
	turnus_parity      TEXT,
	offer_status       TEXT NOT NULL,
	is_limited         INTEGER,
	participant_limit  INTEGER,
	exam_form          TEXT,
	exam_written       INTEGER,
	exam_oral          INTEGER,
	exam_paper         INTEGER,
	exam_presentation  INTEGER,
	exam_project       INTEGER,
	exam_practical     INTEGER,
	is_graded          INTEGER,
	is_fues            INTEGER NOT NULL,
	has_lecture        INTEGER NOT NULL,
	has_exercise       INTEGER NOT NULL,
	has_seminar        INTEGER NOT NULL,
	has_practical      INTEGER NOT NULL,
	has_project        INTEGER NOT NULL,
	has_excursion      INTEGER NOT NULL,
	teaching_events    INTEGER NOT NULL,   -- non-exam events in the newest semester that has events
	at_zentralcampus   INTEGER,            -- campus flags are NULL without any room in that semester
	at_sachsendorf     INTEGER,
	at_senftenberg     INTEGER
) WITHOUT ROWID;

CREATE VIEW v_module_facets_src AS
WITH latest AS (
	SELECT MAX(semester_key) AS semester_key FROM event WHERE category <> 'exam'
),
room AS (
	SELECT me.module_id,
	       COUNT(DISTINCT e.id)                 AS teaching_events,
	       COUNT(d.campus)                      AS rooms_with_campus,
	       MAX(d.campus = 'zentralcampus')      AS at_zentralcampus,
	       MAX(d.campus = 'sachsendorf')        AS at_sachsendorf,
	       MAX(d.campus = 'senftenberg')        AS at_senftenberg
	FROM module_event me
	JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
	JOIN latest l ON l.semester_key = e.semester_key
	LEFT JOIN event_date d ON d.event_id = e.id
	GROUP BY me.module_id
),
form AS (
	SELECT module_id,
	       MAX(form = 'lecture')   AS has_lecture,
	       MAX(form = 'exercise')  AS has_exercise,
	       MAX(form = 'seminar')   AS has_seminar,
	       MAX(form = 'practical') AS has_practical,
	       MAX(form = 'project')   AS has_project,
	       MAX(form = 'excursion') AS has_excursion
	FROM module_teaching_form
	GROUP BY module_id
)
SELECT
	m.id AS module_id, m.credits, m.department_id, m.teaches_german, m.teaches_english, m.duration_semesters,
	CASE WHEN m.turnus_season IN ('winter', 'both') THEN 1 WHEN m.turnus_season = 'summer' THEN 0 END AS offered_winter,
	CASE WHEN m.turnus_season IN ('summer', 'both') THEN 1 WHEN m.turnus_season = 'winter' THEN 0 END AS offered_summer,
	m.turnus_season, m.turnus_parity, m.offer_status, m.is_limited, m.participant_limit,
	m.exam_form, m.exam_written, m.exam_oral, m.exam_paper, m.exam_presentation, m.exam_project, m.exam_practical,
	m.is_graded, m.is_fues,
	COALESCE(f.has_lecture, 0), COALESCE(f.has_exercise, 0), COALESCE(f.has_seminar, 0),
	COALESCE(f.has_practical, 0), COALESCE(f.has_project, 0), COALESCE(f.has_excursion, 0),
	COALESCE(r.teaching_events, 0),
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_zentralcampus, 0) END,
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_sachsendorf, 0) END,
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_senftenberg, 0) END
FROM module m
LEFT JOIN form f ON f.module_id = m.id
LEFT JOIN room r ON r.module_id = m.id;

-- All filterable attributes of a module, one row per module. No LIKE needed.
CREATE VIEW v_module_facets AS
SELECT * FROM module_facet;

-- Display-ready module: card and detail page.
CREATE VIEW v_module AS
SELECT
	m.id, m.title, m.title_de, m.title_en, m.detail_status, m.page_lang,
	m.credits, m.language_raw, m.teaches_german, m.teaches_english,
	m.duration_raw, m.duration_semesters, m.turnus_raw, m.turnus_season, m.turnus_parity, m.offer_status,
	m.limitation_raw, m.is_limited, m.participant_limit,
	m.exam_form, m.exam_form_raw, m.exam_details, m.grading_raw, m.is_graded, m.is_fues,
	m.department_id, d.label AS department, d.code AS department_code,
	m.learning_outcomes, m.contents, m.prerequisites_recommended, m.prerequisites_mandatory, m.remarks,
	m.source_url, m.fetched_at,
	(SELECT GROUP_CONCAT(CASE WHEN mp.title IS NULL THEN mp.name ELSE mp.title || ' ' || mp.name END, '; ')
	 FROM (SELECT * FROM module_person WHERE module_id = m.id ORDER BY ord) mp) AS responsible,
	f.teaching_events, f.at_zentralcampus, f.at_sachsendorf, f.at_senftenberg
FROM module m
LEFT JOIN department d ON d.id = m.department_id
LEFT JOIN module_facet f ON f.module_id = m.id;

-- Autocomplete and full-text candidates: id plus every title variant.
CREATE VIEW v_module_search AS
SELECT id AS module_id, id AS term, 'id' AS kind FROM module
UNION ALL SELECT id, title_de, 'title_de' FROM module WHERE title_de IS NOT NULL
UNION ALL SELECT id, title_en, 'title_en' FROM module WHERE title_en IS NOT NULL;

-- Everyone who teaches a module: responsible persons of the page and instructors of its events.
CREATE VIEW v_module_lecturer AS
SELECT module_id, name, title, 'responsible' AS role FROM module_person
UNION
SELECT me.module_id, d.instructor, NULL, 'instructor'
FROM module_event me JOIN event_date d ON d.event_id = me.event_id
WHERE d.instructor IS NOT NULL;

CREATE VIEW v_module_teaching_form AS
SELECT module_id, ord, form, form_raw, workload_raw, sws, hours FROM module_teaching_form;

CREATE VIEW v_module_text_item AS
SELECT module_id, kind, ord, text FROM module_text_item;

CREATE VIEW v_module_prerequisite AS
SELECT mp.module_id, mp.required_module_id, mp.kind, m.title AS required_title, m.offer_status AS required_offer_status
FROM module_prerequisite mp JOIN module m ON m.id = mp.required_module_id;

CREATE VIEW v_module_successor AS
SELECT ms.module_id, ms.successor_id, m.title AS successor_title
FROM module_successor ms JOIN module m ON m.id = ms.successor_id;

-- Recurring schedule of a module per semester. Exams are in v_module_exam.
CREATE VIEW v_module_schedule AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title, e.type_raw AS event_type,
	d.ord, d.group_name, d.weekday, d.start_time, d.end_time, d.rhythm, d.rhythm_raw,
	d.first_date, d.last_date, d.room, d.campus, d.instructor, d.comment, d.cancelled_dates,
	e.source_url
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id;

CREATE VIEW v_module_exam AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title,
	d.ord, d.weekday, d.start_time, d.end_time, d.first_date, d.last_date, d.room, d.campus, d.comment,
	e.source_url
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category = 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id;

CREATE VIEW v_semester AS
SELECT s.key, s.season, s.year, s.label, s.starts_on, s.ends_on,
       (s.key = (SELECT value FROM meta WHERE key = 'current_semester')) AS is_current,
       (SELECT COUNT(*) FROM event e WHERE e.semester_key = s.key AND e.category <> 'exam') AS teaching_events,
       (SELECT COUNT(*) FROM event e WHERE e.semester_key = s.key AND e.category = 'exam')  AS exam_events
FROM semester s;

CREATE VIEW v_department AS
SELECT d.id, d.code, d.label, d.name_de, d.name_en,
       (SELECT COUNT(*) FROM module m WHERE m.department_id = d.id) AS modules
FROM department d;

CREATE VIEW v_meta AS
SELECT key, value FROM meta;
