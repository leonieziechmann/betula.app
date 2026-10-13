-- Series of single dates (docs/radix/schema-v2.md, „Series of single dates“): QIS lists some
-- Termine date by date, „Einzel am 13.10.2026“, „Einzel am 20.10.2026“ …, although the dates
-- follow a rhythm. A calendar showed each date as an entry of its own (owner, 2026-10-13: „an
-- sich auch periodisch … vielleicht schon in radix, weil das ja an sich datennormalisierung
-- ist“). Every build reads such rows as one series (normalize.Series): single dates of one event
-- in one group, on one weekday and at one start, on a grid of one to four weeks with at least half
-- of the gaps one step, three dates or more, no two following dates more than four weeks apart.
-- The rows stay as QIS lists them; the two columns only say which of them belong together.
--
-- event_date.series         the ord of the row of the series' first date, the same in every
--                           row of the series; NULL for a row in no series
-- event_date.series_weeks   the weeks from one date of the series to the next: 1 to 4
--
-- Existing data is not rewritten: until the next build no row is in a series, which a reader
-- reads as before, date by date.

ALTER TABLE event_date ADD COLUMN series INTEGER;
ALTER TABLE event_date ADD COLUMN series_weeks INTEGER CHECK (series_weeks BETWEEN 1 AND 4);

-- v_module_schedule of 0009 with the two columns appended last; Folia selects named columns.

DROP VIEW v_module_schedule;
CREATE VIEW v_module_schedule AS
SELECT
	me.module_id, e.semester_key, s.label AS semester_label,
	e.id AS event_id, e.number AS event_number, e.title AS event_title, e.type_raw AS event_type,
	d.ord, d.group_name, d.weekday, d.start_time, d.end_time, d.rhythm, d.rhythm_raw,
	d.first_date, d.last_date, d.room, d.campus, d.instructor, d.comment, d.cancelled_dates,
	e.source_url, d.room_short, d.series, d.series_weeks
FROM module_event me
JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
LEFT JOIN semester s ON s.key = e.semester_key
LEFT JOIN event_date d ON d.event_id = e.id;
