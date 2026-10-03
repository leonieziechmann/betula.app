-- Where in the regulation a study plan stands.
--
-- A plan is read out of one document, but a regulation is dozens of pages of
-- legal text with the Regelstudienplan somewhere in an appendix — and a
-- Lesefassung may print four of them, one per study branch. „source_file" alone
-- does not let a reader check what Betula shows: they have to leaf through the
-- PDF to find the table the figures came from.
--
-- plan.source_pages names the pages the plan stands on, as a reader would write
-- them („9" or „9–11"), and plan.source_label the heading it stands under
-- („Anlage 2.1 Regelstudienplan Bachelor of Science – grundlagenorientiert").
-- plan_entry.source_page says it per row, because a plan continued across a page
-- break has rows on both.
--
-- All three come from the cell each row was read from, never from a model: the
-- page is where the PDF drew the box, and the label is the heading the reader
-- already binds that table to.

ALTER TABLE plan ADD COLUMN source_pages TEXT;
ALTER TABLE plan ADD COLUMN source_label TEXT;
ALTER TABLE plan_entry ADD COLUMN source_page INTEGER;

DROP VIEW IF EXISTS v_program_plan_entry;
CREATE VIEW v_program_plan_entry AS
SELECT
	e.program_id, e.ord, e.module_id, e.module_code_raw, e.module_name,
	e.semester, e.start_semester, e.end_semester, e.semester_span,
	e.credits, e.min_credits, e.max_credits, e.kind, e.kind_raw,
	e.study_section, e.subject_area, e.area_rules, e.specialization, e.source_evidence,
	e.source_page,
	m.title AS catalog_title, m.credits AS catalog_credits,
	(e.credits IS NOT NULL AND m.credits IS NOT NULL AND e.credits <> m.credits) AS credits_differ_from_catalog
FROM plan_entry e
LEFT JOIN module m ON m.id = e.module_id;

DROP VIEW IF EXISTS v_program_plan;
CREATE VIEW v_program_plan AS
SELECT pl.program_id, pl.source_file, pl.source_pages, pl.source_label,
       pl.layout_json, pl.validated_at
FROM plan pl
JOIN program p ON p.id = pl.program_id;
