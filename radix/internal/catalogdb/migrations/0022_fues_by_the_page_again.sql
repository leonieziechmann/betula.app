-- A program's FÜS list is again what the FÜS modules' pages admit (migration 0014's view): migration
-- 0021 put every FÜS module whose page admits no program on the FÜS list of every program that has
-- one, and that shows what the regulations do not allow (owner, 2026-11-17: „Man kann bei der füs
-- liste tatsächlich alle anmelden auch die die man nicht belegen darf. Anzeigen wollen wir aber nur
-- die, die nach Ordnung ok sin.").
--
-- What a FÜS module's „Zuordnung zu Studiengängen" is: the BTU's FÜS catalog of the current semester
-- (the PDF „FÜS-Modulkatalog", 152 modules for the winter 2026/27 of 2026-10-14) less the programs'
-- „Negativlisten" in it (Ordnung zum Fachübergreifenden Studium, § 3). Every module of that catalog
-- names programs (120 in the median, 15 at least); of the 137 FÜS modules not in it, 111 name none
-- but programs they are curricular in, 26 are on one to ten FÜS lists. Informatik B.Sc.: the
-- catalog allows 117 modules for every Anwendungsfach, the 117 of its FÜS list. „Einführung in die
-- Amateurfunktechnik" (12045) is taught in the summer: it is in the catalog of the summer 2026, not
-- in this winter's, and so names no program now. Of the 25 modules 0021 added, none is in the
-- winter's catalog, 15 are in the summer's, whose negative lists exclude 286 of the pairs, and 10 in
-- neither.
--
-- The FÜS lists by the catalogs of both seasons and their negative lists follow (docs/radix/
-- data-sources.md §7, Q1).
--
-- Existing data is not rewritten: program_module keeps what the last build wrote until the next.

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
pair AS (
	SELECT program_id, module_id,
	       MAX(source = 'qis_tree')    AS in_tree,
	       MAX(source = 'module_page') AS on_module_page,
	       MAX(source = 'pdf_plan' AND IFNULL(kind_basis, 'stated') = 'stated') AS in_plan
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
