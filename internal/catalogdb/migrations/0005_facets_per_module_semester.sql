-- The schedule facets (teaching_events, at_*) looked at the newest semester that has any
-- event. BTU publishes the next semester event by event over weeks: the first published
-- winter event made every module that was still on the summer semester lose its campus.
-- Each module now uses the newest semester in which it has teaching events itself.
DROP VIEW v_module_facets_src;

CREATE VIEW v_module_facets_src AS
WITH latest AS (
	SELECT me.module_id, MAX(e.semester_key) AS semester_key
	FROM module_event me
	JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
	GROUP BY me.module_id
),
room AS (
	SELECT me.module_id,
	       COUNT(DISTINCT e.id)                 AS teaching_events,
	       COUNT(d.campus)                      AS rooms_with_campus,
	       MAX(d.campus = 'zentralcampus')      AS at_zentralcampus,
	       MAX(d.campus = 'sachsendorf')        AS at_sachsendorf,
	       MAX(d.campus = 'senftenberg')        AS at_senftenberg
	FROM module_event me
	JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
	JOIN latest l ON l.module_id = me.module_id AND l.semester_key IS e.semester_key
	LEFT JOIN event_date d ON d.event_id = e.id
	GROUP BY me.module_id
),
form AS (
	SELECT module_id,
	       MAX(form = 'lecture')   AS has_lecture,
	       MAX(form = 'exercise')  AS has_exercise,
	       MAX(form = 'seminar')   AS has_seminar,
	       MAX(form = 'practical') AS has_practical,
	       MAX(form = 'project')   AS has_project,
	       MAX(form = 'excursion') AS has_excursion
	FROM module_teaching_form
	GROUP BY module_id
)
SELECT
	m.id AS module_id, m.credits, m.department_id, m.teaches_german, m.teaches_english, m.duration_semesters,
	CASE WHEN m.turnus_season IN ('winter', 'both') THEN 1 WHEN m.turnus_season = 'summer' THEN 0 END AS offered_winter,
	CASE WHEN m.turnus_season IN ('summer', 'both') THEN 1 WHEN m.turnus_season = 'winter' THEN 0 END AS offered_summer,
	m.turnus_season, m.turnus_parity, m.offer_status, m.is_limited, m.participant_limit,
	m.exam_form, m.exam_written, m.exam_oral, m.exam_paper, m.exam_presentation, m.exam_project, m.exam_practical,
	m.is_graded, m.is_fues,
	COALESCE(f.has_lecture, 0)   AS has_lecture,   COALESCE(f.has_exercise, 0) AS has_exercise,
	COALESCE(f.has_seminar, 0)   AS has_seminar,   COALESCE(f.has_practical, 0) AS has_practical,
	COALESCE(f.has_project, 0)   AS has_project,   COALESCE(f.has_excursion, 0) AS has_excursion,
	COALESCE(r.teaching_events, 0) AS teaching_events,
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_zentralcampus, 0) END AS at_zentralcampus,
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_sachsendorf, 0) END   AS at_sachsendorf,
	CASE WHEN r.rooms_with_campus > 0 THEN COALESCE(r.at_senftenberg, 0) END   AS at_senftenberg
FROM module m
LEFT JOIN form f ON f.module_id = m.id
LEFT JOIN room r ON r.module_id = m.id;
