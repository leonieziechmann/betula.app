-- Kinds from the plan's slots (docs/radix/schema-v2.md, „Kinds from the plan's slots“): a row of a
-- validated plan that names no module but a Wahlpflicht slot („Anwendungsfach“) is filled from
-- areas of the program's tree, and a module there that no source gives a kind is Wahlpflicht,
-- inferred (catalogbuild.writeSlotKinds; owner, 2026-10-18, Systemtheorie I in Informatik
-- B.Sc.). Such a statement is source 'pdf_plan', kind_basis 'inferred', area_label the row's
-- name, no area_id: the plan names the slot, not the module.
--
-- v_program_module_src.in_plan   the plan names the module itself: an inferred statement of the
--                                plan does not count (before, every pdf_plan statement did; all
--                                of them were stated)
-- v_module_program_link          kind_basis appended last, so that the module page can say that
--                                a kind is inferred; Folia selects named columns
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

DROP VIEW v_module_program_link;
CREATE VIEW v_module_program_link AS
SELECT
	r.module_id, r.ord, r.degree_raw, r.program_raw, r.po_raw, r.resolve_status,
	p.id AS program_id, p.slug AS program_slug, p.name AS program_name,
	COALESCE(p.degree_label, CASE p.degree_level
		WHEN 'bachelor' THEN 'Bachelor' WHEN 'master' THEN 'Master'
		WHEN 'teaching_bachelor' THEN 'Lehramt Bachelor' WHEN 'teaching_master' THEN 'Lehramt Master'
		WHEN 'doctoral' THEN 'Promotion' END) AS degree_display,
	p.po_version, p.is_latest_po,
	pm.relation, pm.kind, pm.kind_source, pm.area, pm.kind_basis
FROM module_program_ref r
LEFT JOIN program p ON p.id = r.program_id
LEFT JOIN program_module pm ON pm.program_id = r.program_id AND pm.module_id = r.module_id;
