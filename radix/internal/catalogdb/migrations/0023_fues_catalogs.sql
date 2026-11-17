-- The FÜS lists from the BTU's FÜS catalogs (owner, 2026-11-17: „Anzeigen wollen wir aber nur die,
-- die nach Ordnung ok sin." and „mach beides, mit Hinweis statt ausblenden"; docs/radix/
-- data-sources.md §19, docs/radix/schema-v2.md „FÜS catalogs"). The BTU publishes a catalog for
-- every semester, a PDF its page „Aktuelle Infos" links (service.CrawlFUESCatalogs): the modules
-- admitted to the Fachübergreifendes Studium in that semester, and the programs' negative lists
-- (Ordnung zum Fachübergreifenden Studium, § 3 Abs. 2), some by degree or profile, some by what a
-- student chooses (an Anwendungsfach, a Schwerpunkt, a Studienrichtung).
--
-- A program's FÜS list is then, for each season, the newest catalog of that season less the
-- program's own modules (the tree places them or the plan states them, § 3 Abs. 1) less what its
-- negative list excludes for every student of it; program_module holds the modules any season
-- admits (relation 'fues'). What its negative list excludes for one choice only stays on the list
-- with a note, and so does a module that one season's catalog holds but excludes while the other
-- admits it (program_fues_note). Without any catalog the FÜS lists are what the FÜS modules' pages
-- admit, as before: those are the current semester's catalog less the negative lists, an
-- exclusion for one choice taken for all.
--
-- fues_catalog_document  a catalog Radix read: its semester ('2026W', '2026S'), the date it
--                        states, how many modules it lists, where it is
-- fues_catalog           its modules, by the campus it groups them under
-- fues_catalog_heading   its headings of programs, and how many programs of the catalog each names
--                        (0: none, which validate warns of)
-- fues_catalog_program   the programs it names, by its headings („Informatik" is every PO of the
--                        B.Sc. and the M.Sc.), with what it says instead of modules („Bitte bei der
--                        Studiengangleitung erfragen!")
-- fues_negative          a line of a negative list: scope '' for every student of the program (a
--                        part for its degree or profile applied), else the choice it holds for
--                        („Anwendungsfach Physik")
-- program_fues_note      a module of a program's FÜS list where a season says more: admitted 0,
--                        that season's catalog holds the module and excludes it for every student;
--                        excluded_for, the choices it excludes it for, '; '-joined
--
-- Existing data is not rewritten: the tables stay empty until a build finds a catalog archived.

CREATE TABLE fues_catalog_document (
	semester   TEXT PRIMARY KEY CHECK (length(semester) = 5 AND substr(semester, 5, 1) IN ('S', 'W')),
	issued     TEXT,
	modules    INTEGER NOT NULL,
	url        TEXT    NOT NULL,
	fetched_at TEXT    NOT NULL
);

CREATE TABLE fues_catalog (
	semester  TEXT NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	module_id TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	campus    TEXT CHECK (campus IN ('zentralcampus', 'sachsendorf', 'senftenberg')),
	PRIMARY KEY (semester, module_id)
) WITHOUT ROWID;

CREATE TABLE fues_catalog_heading (
	semester TEXT    NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	heading  TEXT    NOT NULL,
	programs INTEGER NOT NULL,
	PRIMARY KEY (semester, heading)
) WITHOUT ROWID;

CREATE TABLE fues_catalog_program (
	semester   TEXT NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	program_id TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	heading    TEXT NOT NULL,
	note       TEXT,
	PRIMARY KEY (semester, program_id)
) WITHOUT ROWID;

CREATE TABLE fues_negative (
	semester   TEXT NOT NULL REFERENCES fues_catalog_document(semester) ON DELETE CASCADE,
	program_id TEXT NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id  TEXT NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	scope      TEXT NOT NULL DEFAULT '',
	PRIMARY KEY (semester, program_id, module_id, scope)
) WITHOUT ROWID;

CREATE TABLE program_fues_note (
	program_id   TEXT    NOT NULL REFERENCES program(id) ON DELETE CASCADE,
	module_id    TEXT    NOT NULL REFERENCES module(id) ON DELETE CASCADE,
	season       TEXT    NOT NULL CHECK (season IN ('winter', 'summer')),
	semester     TEXT    NOT NULL,
	admitted     INTEGER NOT NULL CHECK (admitted IN (0, 1)),
	excluded_for TEXT,
	PRIMARY KEY (program_id, module_id, season)
) WITHOUT ROWID;

-- The newest catalog of each season.
CREATE VIEW v_fues_season AS
SELECT MAX(semester) AS semester,
       CASE substr(MAX(semester), 5, 1) WHEN 'W' THEN 'winter' ELSE 'summer' END AS season
FROM fues_catalog_document
GROUP BY substr(semester, 5, 1);

-- The programs with a FÜS list: a FÜS module's page admits the program where it is not its own,
-- or the newest catalog of a season names it.
CREATE VIEW v_fues_program AS
SELECT a.program_id
FROM program_module_assertion a
JOIN module m ON m.id = a.module_id
WHERE a.source = 'module_page' AND m.is_fues = 1
  AND NOT EXISTS (SELECT 1 FROM program_module_assertion o
                  WHERE o.program_id = a.program_id AND o.module_id = a.module_id
                    AND (o.source = 'qis_tree' OR (o.source = 'pdf_plan' AND IFNULL(o.kind_basis, 'stated') = 'stated')))
UNION
SELECT cp.program_id
FROM fues_catalog_program cp
JOIN v_fues_season s ON s.semester = cp.semester;

-- Every module of the newest catalog of each season for every program with a FÜS list, but its
-- own: admitted 1 unless its negative list excludes the module for every student of it. CROSS JOIN
-- keeps the programs the outer loop: SQLite would read v_fues_program anew for every module.
CREATE VIEW v_program_fues_src AS
SELECT fp.program_id, c.module_id, s.season, s.semester,
       CASE WHEN EXISTS (SELECT 1 FROM fues_negative n
                         WHERE n.semester = s.semester AND n.program_id = fp.program_id
                           AND n.module_id = c.module_id AND n.scope = '')
            THEN 0 ELSE 1 END AS admitted,
       (SELECT group_concat(x.scope, '; ') FROM (
           SELECT n.scope FROM fues_negative n
           WHERE n.semester = s.semester AND n.program_id = fp.program_id
             AND n.module_id = c.module_id AND n.scope <> ''
           ORDER BY n.scope) x) AS excluded_for
FROM v_fues_program fp
CROSS JOIN v_fues_season s
JOIN fues_catalog c ON c.semester = s.semester
WHERE NOT EXISTS (SELECT 1 FROM program_module_assertion o
                  WHERE o.program_id = fp.program_id AND o.module_id = c.module_id
                    AND (o.source = 'qis_tree' OR (o.source = 'pdf_plan' AND IFNULL(o.kind_basis, 'stated') = 'stated')));

-- What program_fues_note holds: the seasons of a module of a program's FÜS list that say more.
CREATE VIEW v_program_fues_note_src AS
WITH f AS (SELECT * FROM v_program_fues_src),
admitted AS (SELECT program_id, module_id FROM f GROUP BY program_id, module_id HAVING MAX(admitted) = 1)
SELECT f.program_id, f.module_id, f.season, f.semester, f.admitted, f.excluded_for
FROM f
JOIN admitted a ON a.program_id = f.program_id AND a.module_id = f.module_id
WHERE f.admitted = 0 OR f.excluded_for IS NOT NULL;

CREATE VIEW v_program_fues_note AS
SELECT program_id, module_id, season, semester, admitted, excluded_for FROM program_fues_note;

-- The catalogs Radix read; is_newest for the newest of its season, the one the FÜS lists follow.
CREATE VIEW v_fues_catalog AS
SELECT d.semester,
       CASE substr(d.semester, 5, 1) WHEN 'W' THEN 'winter' ELSE 'summer' END AS season,
       d.issued, d.modules, d.url,
       (d.semester IN (SELECT semester FROM v_fues_season)) AS is_newest
FROM fues_catalog_document d;

-- program_module: as 0022's, but where there are catalogs a FÜS module that neither the tree nor
-- the plan places in a program is on its FÜS list where a season's newest catalog admits it, and
-- nowhere else; its page's naming the program decides nothing then.
DROP VIEW v_program_module_src;
CREATE VIEW v_program_module_src AS
WITH statement AS (
	SELECT a.id, a.program_id, a.module_id, a.source, a.kind, a.kind_basis, a.area_label,
	       pa.path AS area_path, pa.section, pa.ord AS area_ord,
	       (CASE a.kind_basis WHEN 'stated' THEN 0 ELSE 3 END)
	         + (CASE a.source WHEN 'pdf_plan' THEN 1 WHEN 'module_page' THEN 2 ELSE 3 END) AS precedence
	FROM program_module_assertion a
	LEFT JOIN program_area pa ON pa.id = a.area_id
),
stated AS (
	SELECT program_id, module_id,
	       MAX(source = 'qis_tree')    AS in_tree,
	       MAX(source = 'module_page') AS on_module_page,
	       MAX(source = 'pdf_plan' AND IFNULL(kind_basis, 'stated') = 'stated') AS in_plan
	FROM statement
	GROUP BY program_id, module_id
),
catalog_pair AS (
	SELECT DISTINCT program_id, module_id FROM v_program_fues_src WHERE admitted = 1
),
pair AS (
	SELECT s.program_id, s.module_id, s.in_tree, s.on_module_page, s.in_plan, 0 AS in_catalog
	FROM stated s
	JOIN module m ON m.id = s.module_id
	WHERE s.in_tree = 1 OR s.in_plan = 1 OR m.is_fues = 0
	   OR NOT EXISTS (SELECT 1 FROM fues_catalog_document)
	   OR EXISTS (SELECT 1 FROM catalog_pair c WHERE c.program_id = s.program_id AND c.module_id = s.module_id)
	UNION ALL
	-- what only a catalog places in the program is its FÜS, whether QIS flags the module or not
	SELECT c.program_id, c.module_id, 0, 0, 0, 1
	FROM catalog_pair c
	WHERE NOT EXISTS (SELECT 1 FROM program_module_assertion a WHERE a.program_id = c.program_id AND a.module_id = c.module_id)
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
	CASE WHEN (m.is_fues = 1 OR p.in_catalog = 1) AND p.in_tree = 0 AND p.in_plan = 0 THEN 'fues' ELSE 'curricular' END AS relation,
	CASE WHEN (m.is_fues = 1 OR p.in_catalog = 1) AND p.in_tree = 0 AND p.in_plan = 0 THEN 'fues' ELSE b.kind END       AS kind,
	CASE WHEN (m.is_fues = 1 OR p.in_catalog = 1) AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.source END       AS kind_source,
	CASE WHEN (m.is_fues = 1 OR p.in_catalog = 1) AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.kind_basis END   AS kind_basis,
	CASE WHEN (m.is_fues = 1 OR p.in_catalog = 1) AND p.in_tree = 0 AND p.in_plan = 0 THEN NULL ELSE b.precedence END   AS precedence,
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
