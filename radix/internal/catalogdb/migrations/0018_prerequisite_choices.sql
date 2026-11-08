-- Which modules a prerequisite text names as alternatives (docs/radix/data-sources.md §15; owner,
-- 2026-11-08, of 12202 Softwarepraktikum: „Und das ist so ein Quatsch in der Modulbeschreibung.
-- Meinst du das kann man auch parsen?"). „Erfolgreiche Modulprüfung von: Modul 12102
-- Programmierpraktikum ODER Modul 11900 Programmierpraktikum (IMT) UND Modul 12104" asks for one of
-- the two Programmierpraktika and for 12104; until now its three modules read as three that are
-- all needed, 11900 among them, which is no longer offered.
--
-- module_prerequisite.choice    the choice a required module is one of: of the rows of one module
--                               and kind with the same choice one is enough, and every choice is
--                               needed; 1, 2, … in the order the text names them. Most choices
--                               hold one module (normalize.PrerequisiteChoices)
-- v_module_prerequisite.choice  the same
--
-- The table is derived and every build writes it anew; its rows are carried over, each a choice of
-- its own, as each module was needed until now.

CREATE TABLE module_prerequisite_new (
	module_id          TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	required_module_id TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	kind               TEXT    NOT NULL CHECK (kind IN ('mandatory', 'recommended')),
	choice             INTEGER NOT NULL CHECK (choice >= 1),
	PRIMARY KEY (module_id, required_module_id, kind)
) WITHOUT ROWID;

INSERT INTO module_prerequisite_new (module_id, required_module_id, kind, choice)
SELECT module_id, required_module_id, kind,
       ROW_NUMBER() OVER (PARTITION BY module_id, kind ORDER BY required_module_id)
FROM module_prerequisite;

DROP VIEW v_module_prerequisite;
DROP TABLE module_prerequisite;
ALTER TABLE module_prerequisite_new RENAME TO module_prerequisite;

CREATE VIEW v_module_prerequisite AS
SELECT mp.module_id, mp.required_module_id, mp.kind, mp.choice, m.title AS required_title, m.offer_status AS required_offer_status
FROM module_prerequisite mp JOIN module m ON m.id = mp.required_module_id;
