-- Events of which a student attends some, not all (folia/design/wahlangebote/konzept.html; owner,
-- 2026-10-19: „ich will nicht, dass wir sonderfälle per hand eintragen, sondern eine lösung haben,
-- die in die zukunft funktioniert … Bitte bilde alles außer daten passen nicht zusammen in der DB
-- und dem UI ab“, then „Passt so, nimm deine Vorschläge und bau es“). Every build reads them from
-- the SWS of a module and of its events, their forms, their titles and whom QIS lists them for
-- (catalogbuild/choices.go, docs/radix/schema-v2.md, „Choices“); no module is named anywhere.
--
-- module_choice          a choice among a module's events in a semester, within one of its
--                        courses at a campus where it has them: how many options one takes and
--                        how many SWS they cover; 'program' the same event more than once with
--                        QIS naming other programs at each option, 'group' the same event in
--                        groups, 'topic' different contents to choose from
-- module_choice_option   the options of a choice, each one event or more (a lecture and the
--                        seminar of its title; „Gruppe 2“ of two events that share the numbers)
-- module_extra           events beside the compulsory part, attended by whoever wants to: a
--                        tutorial the module does not state, a title naming a support format
--
-- v_module_schedule.choice, .option, .extra   the choice and option a date's event is one of, and
--                                             whether it is an extra; NULL, NULL, 0 for most
-- v_module_choice           the choices with their options' events and the SWS QIS gives them
-- v_module_choice_family    the families of the programs QIS lists at an option's events
--
-- Existing data is not rewritten: the tables stay empty until the next build.

CREATE TABLE module_choice (
	module_id    TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	semester_key TEXT    NOT NULL REFERENCES semester(key),
	choice       INTEGER NOT NULL CHECK (choice >= 1),
	course       TEXT             CHECK (course IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	kind         TEXT    NOT NULL CHECK (kind IN ('program', 'group', 'topic')),
	basis        TEXT    NOT NULL CHECK (basis IN ('named', 'module', 'form')),
	sws          REAL             CHECK (sws > 0),
	pick         INTEGER NOT NULL CHECK (pick >= 1),
	PRIMARY KEY (module_id, semester_key, choice)
) WITHOUT ROWID;

CREATE TABLE module_choice_option (
	module_id    TEXT    NOT NULL,
	semester_key TEXT    NOT NULL,
	choice       INTEGER NOT NULL,
	option       INTEGER NOT NULL CHECK (option >= 1),
	event_id     TEXT    NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	label        TEXT,
	PRIMARY KEY (module_id, semester_key, choice, option, event_id),
	FOREIGN KEY (module_id, semester_key, choice) REFERENCES module_choice(module_id, semester_key, choice) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE UNIQUE INDEX idx_module_choice_option_event ON module_choice_option(module_id, event_id);

CREATE TABLE module_extra (
	module_id    TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	event_id     TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
	semester_key TEXT NOT NULL REFERENCES semester(key),
	basis        TEXT NOT NULL CHECK (basis IN ('tutorial', 'title')),
	PRIMARY KEY (module_id, event_id)
) WITHOUT ROWID;

CREATE VIEW v_module_choice AS
SELECT c.module_id, c.semester_key, c.choice, c.course, c.kind, c.basis, c.sws, c.pick,
       o.option, o.event_id, o.label, e.sws AS event_sws
FROM module_choice c
JOIN module_choice_option o ON o.module_id = c.module_id AND o.semester_key = c.semester_key AND o.choice = c.choice
LEFT JOIN event e ON e.id = o.event_id;

CREATE VIEW v_module_choice_family AS
SELECT DISTINCT o.module_id, o.semester_key, o.choice, o.option, p.family_key
FROM module_choice_option o
JOIN event_program_ref r ON r.event_id = o.event_id
JOIN program p ON p.id = r.program_id;

-- v_module_schedule of 0015 with the choice, the option and the extra appended last; Folia selects
-- named columns.

DROP VIEW v_module_schedule;
CREATE VIEW v_module_schedule AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title, e.type_raw AS event_type,
	d.ord, d.group_name, d.weekday, d.start_time, d.end_time, d.rhythm, d.rhythm_raw,
	d.first_date, d.last_date, d.room, d.campus, d.instructor, d.comment, d.cancelled_dates,
	e.source_url, d.room_short, d.series, d.series_weeks, mt.campus AS track,
	mco.choice, mco.option, CASE WHEN mx.event_id IS NULL THEN 0 ELSE 1 END AS extra
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id
LEFT JOIN module_track mt ON mt.module_id = me.module_id AND mt.event_id = e.id
LEFT JOIN module_choice_option mco ON mco.module_id = me.module_id AND mco.event_id = e.id
LEFT JOIN module_extra mx ON mx.module_id = me.module_id AND mx.event_id = e.id;
