-- Courses of a module taught at two places (docs/radix/schema-v2.md, „Tracks“; owner, 2026-10-19:
-- „das Modul Prgrammierpraktikum [ist] teil von mehreren Studiengängen, wobei ein Teil der
-- Studiengänge in Cottbus und ein andere teil in Senftenberg ist. Deswegen gibt es zwei
-- Vorlesungen … Aber es wird nur erwartet, dass die Studies eine der beiden vls/praktika
-- besucht.“). Programmierpraktikum (12102) is held at the Zentralcampus for Informatik, Mathematik
-- and five more, and in Sachsendorf for Medizininformatik; a student attends one of the two.
--
-- event_program          what an event page lists under „Studiengänge“: the program as QIS names
--                        it, the Fachsemester, the PO, a remark, the id of QIS's link
-- event_program_ref      the programs of the catalog a row of event_program names (every PO of the
--                        program where the row names none; none where the name resolves to nothing)
-- program_campus         where a program is taught: per campus, how many compulsory modules of its
--                        Regelstudienplan are held there (catalogbuild.writeProgramCampus)
-- module_track           the teaching events of a module in a semester that form a course of their
--                        own at one campus, when at least two campuses each hold one
-- module_track_program   which of those courses a program takes: the one whose events QIS lists
--                        the program on ('qis'), else the one at the campus where the program has
--                        more of its compulsory modules ('campus'); none where neither decides
--
-- v_module_schedule.track    the campus of the course the event belongs to; NULL for an event that
--                            is in no course of its own (every event of most modules)
--
-- Existing data is not rewritten: the tables stay empty until the next build, and a reader sees no
-- tracks until then.

CREATE TABLE event_program (
	event_id       TEXT    NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	ord            INTEGER NOT NULL,
	label          TEXT    NOT NULL,                 -- „Informatik Bachelor (universitäres Profil)“
	semester_min   INTEGER,                          -- „1 - 3“: 1
	semester_max   INTEGER,                          -- „1 - 3“: 3
	po_raw         TEXT,
	remark         TEXT,
	qis_id         TEXT,                             -- k_abstgv.abstgvnr of the link to the program in QIS
	resolve_status TEXT    NOT NULL CHECK (resolve_status IN ('resolved', 'unresolved')),
	PRIMARY KEY (event_id, ord)
) WITHOUT ROWID;

CREATE TABLE event_program_ref (
	event_id   TEXT    NOT NULL,
	ord        INTEGER NOT NULL,
	program_id TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	PRIMARY KEY (event_id, ord, program_id),
	FOREIGN KEY (event_id, ord) REFERENCES event_program(event_id, ord) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX idx_event_program_ref_program ON event_program_ref(program_id);

CREATE TABLE program_campus (
	program_id TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	campus     TEXT    NOT NULL CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	modules    INTEGER NOT NULL CHECK (modules > 0),
	is_main    INTEGER NOT NULL CHECK (is_main IN (0, 1)),
	PRIMARY KEY (program_id, campus)
) WITHOUT ROWID;

CREATE TABLE module_track (
	module_id    TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	event_id     TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	semester_key TEXT NOT NULL REFERENCES semester(key),
	campus       TEXT NOT NULL CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	PRIMARY KEY (module_id, event_id)
) WITHOUT ROWID;

CREATE TABLE module_track_program (
	module_id    TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	semester_key TEXT NOT NULL REFERENCES semester(key),
	campus       TEXT NOT NULL CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	program_id   TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	basis        TEXT NOT NULL CHECK (basis IN ('qis', 'campus')),
	PRIMARY KEY (module_id, semester_key, program_id, campus)
) WITHOUT ROWID;

CREATE VIEW v_event_program AS
SELECT ep.event_id, ep.ord, ep.label, ep.semester_min, ep.semester_max, ep.po_raw, ep.remark,
       ep.resolve_status, r.program_id, p.family_key
FROM event_program ep
LEFT JOIN event_program_ref r ON r.event_id = ep.event_id AND r.ord = ep.ord
LEFT JOIN program p ON p.id = r.program_id;

CREATE VIEW v_program_campus AS
SELECT program_id, campus, modules, is_main FROM program_campus;

CREATE VIEW v_module_track_program AS
SELECT t.module_id, t.semester_key, t.campus, t.program_id, t.basis, p.family_key
FROM module_track_program t
JOIN program p ON p.id = t.program_id;

-- v_module_schedule of 0013 with the track appended last; Folia selects named columns.

DROP VIEW v_module_schedule;
CREATE VIEW v_module_schedule AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title, e.type_raw AS event_type,
	d.ord, d.group_name, d.weekday, d.start_time, d.end_time, d.rhythm, d.rhythm_raw,
	d.first_date, d.last_date, d.room, d.campus, d.instructor, d.comment, d.cancelled_dates,
	e.source_url, d.room_short, d.series, d.series_weeks, mt.campus AS track
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id
LEFT JOIN module_track mt ON mt.module_id = me.module_id AND mt.event_id = e.id;
