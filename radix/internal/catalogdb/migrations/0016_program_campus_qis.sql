-- Where a program is taught, by QIS first (docs/radix/schema-v2.md, „Tracks“; owner, 2026-10-19:
-- „Wenn du bessere Kriterien findest um das einzustufen dann kannst du auch die Verwenden.“).
-- The event pages say for whom each event is held, and where its students are follows from
-- them more surely than from the campuses of the compulsory modules of the Regelstudienplan: on
-- canary's snapshot of 2026-10-19 both agree for 76 families, QIS places 26 more that the plans do
-- not, and where they differ QIS is plain (Wirtschaftsingenieurwesen B.Sc., FH profile: 30 of the
-- 33 SWS QIS lists it at in Senftenberg, three compulsory modules of four at the Zentralcampus).
--
-- program_campus.sws      the SWS of the teaching events at the campus that QIS lists a program of
--                         the program's family at (an event without SWS counts one); modules may
--                         now be 0, at a campus that only QIS names
-- program_campus.is_main  where QIS lists the family for strictly the most SWS; where it decides
--                         nothing, where strictly the most compulsory modules are held, as before
--
-- The table is derived and every build writes it anew; its rows are carried over with sws 0 until
-- then.

CREATE TABLE program_campus_new (
	program_id TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	campus     TEXT    NOT NULL CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg', 'nord')),
	modules    INTEGER NOT NULL CHECK (modules >= 0),
	sws        REAL    NOT NULL CHECK (sws >= 0),
	is_main    INTEGER NOT NULL CHECK (is_main IN (0, 1)),
	PRIMARY KEY (program_id, campus),
	CHECK (modules > 0 OR sws > 0)
) WITHOUT ROWID;

INSERT INTO program_campus_new (program_id, campus, modules, sws, is_main)
SELECT program_id, campus, modules, 0, is_main FROM program_campus;

DROP VIEW v_program_campus;
DROP TABLE program_campus;
ALTER TABLE program_campus_new RENAME TO program_campus;

CREATE VIEW v_program_campus AS
SELECT program_id, campus, modules, sws, is_main FROM program_campus;
