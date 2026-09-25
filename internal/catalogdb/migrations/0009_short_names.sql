-- Short names for the places where space is tight: a room as „ZHG HS.A“ instead of
-- „Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus“, a module as „AuP“ instead of
-- „Algorithmieren und Programmieren“. Both are derived by every build from what the
-- sources already say (owner, 2026-09-25: where an abbreviation does not fit how a
-- program uses it, the metadata is simply rebuilt).
--
-- event_date.room_short    „<building> <room>[<attachment>]“ (normalize.RoomShort); room keeps the full name
-- module_abbrev            one abbreviation per module, for pages without a program
-- program_module_abbrev    one per module of a program, unique within it (curriculum,
--                          electives and FÜS), for the Studienplan
--
-- Tables of their own rather than columns of program_module: program_module is the
-- materialization of v_program_module_src, which validate compares column for column,
-- and an abbreviation is computed in Go, not in SQL. Flags instead of text codes, so
-- that no new code needs a label in Folia (catalog::labels).
--
-- Existing data is not rewritten: until the next build, room_short is NULL and the new
-- tables are empty, and validate fails, so a migrated but unbuilt database is never
-- exported.

ALTER TABLE event_date ADD COLUMN room_short TEXT;

CREATE TABLE module_abbrev (
	module_id   TEXT    PRIMARY KEY REFERENCES module(id) ON DELETE CASCADE,
	abbrev      TEXT    NOT NULL,
	is_override INTEGER NOT NULL CHECK (is_override IN (0, 1))   -- a line of internal/abbrev/overrides.tsv
) WITHOUT ROWID;

CREATE TABLE program_module_abbrev (
	program_id  TEXT    NOT NULL,
	module_id   TEXT    NOT NULL,
	abbrev      TEXT    NOT NULL,
	is_override INTEGER NOT NULL CHECK (is_override IN (0, 1)),
	choice      INTEGER NOT NULL CHECK (choice >= 1),              -- 1: the module's first candidate; more: it fell back
	is_twin     INTEGER NOT NULL CHECK (is_twin IN (0, 1)),        -- -b, -c … after an identical title in the program
	PRIMARY KEY (program_id, module_id),
	FOREIGN KEY (program_id, module_id) REFERENCES program_module(program_id, module_id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE UNIQUE INDEX idx_program_module_abbrev_unique ON program_module_abbrev(program_id, abbrev COLLATE NOCASE);

-- The four views of 0003 with the new column appended last; Folia selects named columns.

DROP VIEW v_module;
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
	f.teaching_events, f.at_zentralcampus, f.at_sachsendorf, f.at_senftenberg,
	a.abbrev
FROM module m
LEFT JOIN department d ON d.id = m.department_id
LEFT JOIN module_facet f ON f.module_id = m.id
LEFT JOIN module_abbrev a ON a.module_id = m.id;

DROP VIEW v_program_module;
CREATE VIEW v_program_module AS
SELECT
	pm.program_id, pm.module_id, pm.relation, pm.kind, pm.kind_source, pm.kind_basis, pm.precedence,
	pm.area, pm.section, pm.in_tree, pm.on_module_page, pm.in_plan,
	m.title AS module_title, m.credits AS module_credits, m.offer_status, m.turnus_season,
	(SELECT MIN(e.semester) FROM plan_entry e
	 WHERE e.program_id = pm.program_id AND e.module_id = pm.module_id) AS plan_semester,
	pa.abbrev
FROM program_module pm
JOIN module m ON m.id = pm.module_id
LEFT JOIN program_module_abbrev pa ON pa.program_id = pm.program_id AND pa.module_id = pm.module_id;

DROP VIEW v_module_schedule;
CREATE VIEW v_module_schedule AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title, e.type_raw AS event_type,
	d.ord, d.group_name, d.weekday, d.start_time, d.end_time, d.rhythm, d.rhythm_raw,
	d.first_date, d.last_date, d.room, d.campus, d.instructor, d.comment, d.cancelled_dates,
	e.source_url, d.room_short
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id;

DROP VIEW v_module_exam;
CREATE VIEW v_module_exam AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title,
	d.ord, d.weekday, d.start_time, d.end_time, d.first_date, d.last_date, d.room, d.campus, d.comment,
	e.source_url, d.room_short
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category = 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id;
