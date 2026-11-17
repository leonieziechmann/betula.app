-- The lines of a FÜS catalog's negative list that hold for one choice have a table of their own:
-- schema 23 kept them in fues_negative beside those that hold for every student of the program,
-- and gave those the scope '' — an empty string in a text column of the snapshot, which validate
-- refuses (unknown is NULL; „no empty-string or '-' placeholders in text columns"). So the first
-- build that read a catalog failed its validation and exported nothing: on canary, from its first
-- cycle with Radix 0.14.0 on 2026-11-17, every cycle kept the snapshot the deploy had built before
-- any catalog was archived.
--
-- fues_negative         a line of a negative list that holds for every student of the program (a
--                       part for its degree or profile applied)
-- fues_negative_choice  a line that holds for one choice only: the choice („Anwendungsfach Physik")
--
-- The FÜS lists and their notes are what they were meant to be with schema 23. Existing data is not
-- rewritten: the next build fills both tables from the archived catalogs.

DROP VIEW v_program_fues_src;
DROP TABLE fues_negative;

CREATE TABLE fues_negative (
	semester   TEXT NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	program_id TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id  TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	PRIMARY KEY (semester, program_id, module_id)
) WITHOUT ROWID;

CREATE TABLE fues_negative_choice (
	semester   TEXT NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	program_id TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id  TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	choice     TEXT NOT NULL CHECK (TRIM(choice) NOT IN ('', '-')),
	PRIMARY KEY (semester, program_id, module_id, choice)
) WITHOUT ROWID;

-- As 0023's: every module of the newest catalog of each season for every program with a FÜS list,
-- but its own; admitted 1 unless its negative list excludes the module for every student of it,
-- excluded_for the choices it excludes it for. CROSS JOIN keeps the programs the outer loop.
CREATE VIEW v_program_fues_src AS
SELECT fp.program_id, c.module_id, s.season, s.semester,
       CASE WHEN EXISTS (SELECT 1 FROM fues_negative n
                         WHERE n.semester = s.semester AND n.program_id = fp.program_id
                           AND n.module_id = c.module_id)
            THEN 0 ELSE 1 END AS admitted,
       (SELECT group_concat(x.choice, '; ') FROM (
           SELECT n.choice FROM fues_negative_choice n
           WHERE n.semester = s.semester AND n.program_id = fp.program_id
             AND n.module_id = c.module_id
           ORDER BY n.choice) x) AS excluded_for
FROM v_fues_program fp
CROSS JOIN v_fues_season s
JOIN fues_catalog c ON c.semester = s.semester
WHERE NOT EXISTS (SELECT 1 FROM program_module_assertion o
                  WHERE o.program_id = fp.program_id AND o.module_id = c.module_id
                    AND (o.source = 'qis_tree' OR (o.source = 'pdf_plan' AND IFNULL(o.kind_basis, 'stated') = 'stated')));
