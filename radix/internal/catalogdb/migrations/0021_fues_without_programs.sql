-- A FÜS module whose page admits no program is on the FÜS list of every program that has one
-- (owner, 2026-11-17, of „Einführung in die Amateurfunktechnik" (12045) in Informatik: „Das ist
-- nämlich eigentlich ok"). Its page says „Zuordnung zu Studiengängen: keine Zuordnung vorhanden"
-- beside „Das Modul ist für das Fachübergreifende Studium zugelassen.", and a FÜS module was on the
-- FÜS list of a program only where its page admits the program (decision Q1,
-- docs/radix/data-sources.md §7): 12045 was on none, and neither the catalog's FÜS list of
-- Informatik nor the planner's modules for its FÜS offered it. A page that names programs still
-- admits those; one that names none restricts no one: the Ordnung zum Fachübergreifenden Studium
-- lets a student choose freely from the FÜS catalog but for the Pflicht and Wahlpflicht modules
-- of the own program (§ 3), and those stay curricular, as for every FÜS module.
--
-- „Admits no program": no line of „Zuordnung zu Studiengängen" names a program of a degree, so
-- module_program_ref holds nothing for the module but „Abschluss im Ausland" (resolve_status
-- 'abroad'), or nothing; a module without a page has none either. „A program that has one": a FÜS
-- module's page admits the program and the module is not curricular there, as before. A program
-- without one gets none of them: Folia leads its FÜS rows to the BTU's whole list
-- (variants::fues_list_for), which holds them already.
--
-- On canary's snapshot of 2026-11-17: 25 of the 289 FÜS modules (19 „keine Zuordnung vorhanden",
-- 6 only „Abschluss im Ausland"), none of them curricular in a program with a FÜS list, so 3,725
-- pairs more (25 × 149 programs), and no pair before changes; Informatik B.Sc. 117 → 142 FÜS
-- modules. The pairs no source states are relation 'fues', kind 'fues', with no area, in_tree,
-- on_module_page and in_plan 0.
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
stated AS (
	SELECT program_id, module_id,
	       MAX(source = 'qis_tree')    AS in_tree,
	       MAX(source = 'module_page') AS on_module_page,
	       MAX(source = 'pdf_plan' AND IFNULL(kind_basis, 'stated') = 'stated') AS in_plan
	FROM statement
	GROUP BY program_id, module_id
),
-- the FÜS modules whose page admits no program
open_fues AS (
	SELECT m.id AS module_id
	FROM module m
	WHERE m.is_fues = 1
	  AND NOT EXISTS (SELECT 1 FROM module_program_ref r WHERE r.module_id = m.id AND r.resolve_status <> 'abroad')
),
-- the programs with a FÜS list of their own
fues_program AS (
	SELECT DISTINCT s.program_id
	FROM stated s
	JOIN module m ON m.id = s.module_id
	WHERE m.is_fues = 1 AND s.on_module_page = 1 AND s.in_tree = 0 AND s.in_plan = 0
),
pair AS (
	SELECT program_id, module_id, in_tree, on_module_page, in_plan FROM stated
	UNION ALL
	SELECT f.program_id, o.module_id, 0, 0, 0
	FROM fues_program f
	CROSS JOIN open_fues o
	WHERE NOT EXISTS (SELECT 1 FROM program_module_assertion a WHERE a.program_id = f.program_id AND a.module_id = o.module_id)
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
