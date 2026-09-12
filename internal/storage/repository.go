package storage

import (
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/jakob/btu-scraper/internal/model"
)

var (
	ErrNotFound = errors.New("module not found")
)

// Filter holds criteria for querying modules.
type Filter struct {
	Query      string
	Department string
	Language   string
	MinCredits float64
	MaxCredits float64
	Limit      int
	Offset     int
}

// UpsertCatalog inserts or updates a list of discovered module summaries in a transaction.
func (s *Storage) UpsertCatalog(summaries []model.ModuleSummary) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	stmt, err := tx.Prepare(`
		INSERT INTO modules (id, code, title_de, raw_url, last_scraped_at, updated_at)
		VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
		ON CONFLICT(id) DO UPDATE SET
			code = excluded.code,
			title_de = CASE WHEN modules.title_de IS NULL OR modules.title_de = '' THEN excluded.title_de ELSE modules.title_de END,
			raw_url = excluded.raw_url,
			last_scraped_at = excluded.last_scraped_at,
			updated_at = CURRENT_TIMESTAMP
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, m := range summaries {
		scrapedAt := m.ScrapedAt.Format(time.RFC3339)
		if _, err := stmt.Exec(m.ID, m.Code, m.Title, m.URL, scrapedAt); err != nil {
			return fmt.Errorf("failed to upsert module %s: %w", m.ID, err)
		}
	}

	return tx.Commit()
}

// UpsertModuleDetail inserts or updates a complete module detail record.
func (s *Storage) UpsertModuleDetail(d *model.ModuleDetail) error {
	respJSON, _ := json.Marshal(d.ResponsiblePersons)
	tfJSON, _ := json.Marshal(d.TeachingForms)
	litJSON, _ := json.Marshal(d.Literature)
	spJSON, _ := json.Marshal(d.StudyPrograms)
	coursesJSON, _ := json.Marshal(d.AssociatedCourses)
	eventsJSON, _ := json.Marshal(d.CurrentSemesterEvents)

	isPhaseOutInt := 0
	if d.IsPhaseOut {
		isPhaseOutInt = 1
	}
	crossDiscInt := 0
	if d.CrossDisciplinary {
		crossDiscInt = 1
	}
	isFuesInt := 0
	if d.IsFUES || d.CrossDisciplinary {
		isFuesInt = 1
	}

	scrapedAt := d.LastScrapedAt.Format(time.RFC3339)

	query := `
		INSERT INTO modules (
			id, code, title_de, title_en, is_phase_out, department,
			responsible_persons, language, duration, turnus, credits, credits_raw,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory,
			teaching_forms, literature, exam_type, exam_details, grading, limitation,
			study_programs, remarks, associated_courses, current_semester_events,
			cross_disciplinary, is_fues, raw_url, last_scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?, ?,
			?, ?, ?, ?, ?, ?,
			?, ?, ?, ?,
			?, ?, ?, ?, ?, ?,
			?, ?, ?, ?,
			?, ?, ?, ?, CURRENT_TIMESTAMP
		)
		ON CONFLICT(id) DO UPDATE SET
			code = excluded.code,
			title_de = CASE WHEN excluded.title_de != '' THEN excluded.title_de ELSE modules.title_de END,
			title_en = CASE WHEN excluded.title_en != '' THEN excluded.title_en ELSE modules.title_en END,
			is_phase_out = excluded.is_phase_out,
			department = excluded.department,
			responsible_persons = excluded.responsible_persons,
			language = excluded.language,
			duration = excluded.duration,
			turnus = excluded.turnus,
			credits = excluded.credits,
			credits_raw = excluded.credits_raw,
			learning_outcomes = excluded.learning_outcomes,
			contents = excluded.contents,
			prerequisites_recommended = excluded.prerequisites_recommended,
			prerequisites_mandatory = excluded.prerequisites_mandatory,
			teaching_forms = excluded.teaching_forms,
			literature = excluded.literature,
			exam_type = excluded.exam_type,
			exam_details = excluded.exam_details,
			grading = excluded.grading,
			limitation = excluded.limitation,
			study_programs = excluded.study_programs,
			remarks = excluded.remarks,
			associated_courses = excluded.associated_courses,
			current_semester_events = excluded.current_semester_events,
			cross_disciplinary = CASE WHEN excluded.cross_disciplinary = 1 THEN 1 ELSE modules.cross_disciplinary END,
			is_fues = CASE WHEN excluded.is_fues = 1 THEN 1 ELSE modules.is_fues END,
			raw_url = excluded.raw_url,
			last_scraped_at = excluded.last_scraped_at,
			updated_at = CURRENT_TIMESTAMP
	`

	_, err := s.db.Exec(query,
		d.ID, d.Code, d.TitleDE, d.TitleEN, isPhaseOutInt, d.Department,
		string(respJSON), d.Language, d.Duration, d.Turnus, d.Credits, d.CreditsRaw,
		d.LearningOutcomes, d.Contents, d.PrerequisitesRecommended, d.PrerequisitesMandatory,
		string(tfJSON), string(litJSON), d.ExamType, d.ExamDetails, d.Grading, d.Limitation,
		string(spJSON), d.Remarks, string(coursesJSON), string(eventsJSON),
		crossDiscInt, isFuesInt, d.RawURL, scrapedAt,
	)
	return err
}

// GetModule fetches a single module detail by ID.
func (s *Storage) GetModule(id string) (*model.ModuleDetail, error) {
	row := s.db.QueryRow(`
		SELECT
			id, code, title_de, title_en, is_phase_out, department,
			responsible_persons, language, duration, turnus, credits, credits_raw,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory,
			teaching_forms, literature, exam_type, exam_details, grading, limitation,
			study_programs, remarks, associated_courses, current_semester_events,
			cross_disciplinary, is_fues, raw_url, last_scraped_at
		FROM modules
		WHERE id = ? OR code = ?
	`, id, id)

	var (
		d                                              model.ModuleDetail
		isPhaseOutInt, crossDiscInt, isFuesInt         int
		respJSON, tfJSON, litJSON, spJSON              sql.NullString
		coursesJSON, eventsJSON                        sql.NullString
		scrapedAtStr                                   sql.NullString
		titleEN, dept, lang, dur, turnus, credRaw      sql.NullString
		learnOut, contents, prereqRec, prereqMand      sql.NullString
		examType, examDet, grading, limit, remarks, url sql.NullString
	)

	err := row.Scan(
		&d.ID, &d.Code, &d.TitleDE, &titleEN, &isPhaseOutInt, &dept,
		&respJSON, &lang, &dur, &turnus, &d.Credits, &credRaw,
		&learnOut, &contents, &prereqRec, &prereqMand,
		&tfJSON, &litJSON, &examType, &examDet, &grading, &limit,
		&spJSON, &remarks, &coursesJSON, &eventsJSON,
		&crossDiscInt, &isFuesInt, &url, &scrapedAtStr,
	)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}

	d.IsPhaseOut = isPhaseOutInt == 1
	d.CrossDisciplinary = crossDiscInt == 1
	d.IsFUES = isFuesInt == 1
	d.TitleEN = titleEN.String
	d.Department = dept.String
	d.Language = lang.String
	d.Duration = dur.String
	d.Turnus = turnus.String
	d.CreditsRaw = credRaw.String
	d.LearningOutcomes = learnOut.String
	d.Contents = contents.String
	d.PrerequisitesRecommended = prereqRec.String
	d.PrerequisitesMandatory = prereqMand.String
	d.ExamType = examType.String
	d.ExamDetails = examDet.String
	d.Grading = grading.String
	d.Limitation = limit.String
	d.Remarks = remarks.String
	d.RawURL = url.String

	if respJSON.Valid && respJSON.String != "" {
		_ = json.Unmarshal([]byte(respJSON.String), &d.ResponsiblePersons)
	}
	if tfJSON.Valid && tfJSON.String != "" {
		_ = json.Unmarshal([]byte(tfJSON.String), &d.TeachingForms)
	}
	if litJSON.Valid && litJSON.String != "" {
		_ = json.Unmarshal([]byte(litJSON.String), &d.Literature)
	}
	if spJSON.Valid && spJSON.String != "" {
		_ = json.Unmarshal([]byte(spJSON.String), &d.StudyPrograms)
	}
	if coursesJSON.Valid && coursesJSON.String != "" {
		_ = json.Unmarshal([]byte(coursesJSON.String), &d.AssociatedCourses)
	}
	if eventsJSON.Valid && eventsJSON.String != "" {
		_ = json.Unmarshal([]byte(eventsJSON.String), &d.CurrentSemesterEvents)
	}
	if scrapedAtStr.Valid && scrapedAtStr.String != "" {
		if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
			d.LastScrapedAt = t
		}
	}

	return &d, nil
}

// ListModules queries module summaries with optional filtering.
func (s *Storage) ListModules(f Filter) ([]model.ModuleSummary, error) {
	var (
		whereClauses []string
		args         []interface{}
	)

	if f.Query != "" {
		whereClauses = append(whereClauses, "(id LIKE ? OR code LIKE ? OR title_de LIKE ? OR title_en LIKE ?)")
		pattern := "%" + f.Query + "%"
		args = append(args, pattern, pattern, pattern, pattern)
	}

	if f.Department != "" {
		whereClauses = append(whereClauses, "department LIKE ?")
		args = append(args, "%"+f.Department+"%")
	}

	if f.Language != "" {
		whereClauses = append(whereClauses, "language LIKE ?")
		args = append(args, "%"+f.Language+"%")
	}

	if f.MinCredits > 0 {
		whereClauses = append(whereClauses, "credits >= ?")
		args = append(args, f.MinCredits)
	}

	if f.MaxCredits > 0 {
		whereClauses = append(whereClauses, "credits <= ?")
		args = append(args, f.MaxCredits)
	}

	query := "SELECT id, code, title_de, raw_url, last_scraped_at FROM modules"
	if len(whereClauses) > 0 {
		query += " WHERE " + strings.Join(whereClauses, " AND ")
	}
	query += " ORDER BY CAST(code AS INTEGER), code ASC"

	if f.Limit > 0 {
		query += " LIMIT ?"
		args = append(args, f.Limit)
		if f.Offset > 0 {
			query += " OFFSET ?"
			args = append(args, f.Offset)
		}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.ModuleSummary
	for rows.Next() {
		var (
			m            model.ModuleSummary
			titleDe      sql.NullString
			rawURL       sql.NullString
			scrapedAtStr sql.NullString
		)
		if err := rows.Scan(&m.ID, &m.Code, &titleDe, &rawURL, &scrapedAtStr); err != nil {
			return nil, err
		}
		m.Title = titleDe.String
		m.URL = rawURL.String
		if scrapedAtStr.Valid && scrapedAtStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
				m.ScrapedAt = t
			}
		}
		results = append(results, m)
	}

	return results, rows.Err()
}

// Count returns the total number of modules in the database.
func (s *Storage) Count() (int, error) {
	var count int
	err := s.db.QueryRow("SELECT COUNT(*) FROM modules").Scan(&count)
	return count, err
}

// UpsertEvent inserts or updates an event, its schedules, and its module associations in a transaction.
func (s *Storage) UpsertEvent(e *model.EventDetail) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	respJSON, _ := json.Marshal(e.ResponsiblePersons)
	modsJSON, _ := json.Marshal(e.AssociatedModules)
	stgJSON, _ := json.Marshal(e.StudyPrograms)
	instJSON, _ := json.Marshal(e.Institutions)
	scrapedAt := e.LastScrapedAt.Format(time.RFC3339)

	query := `
		INSERT INTO events (
			id, event_number, title, event_type, semester, sws,
			expected_participants, max_participants, hyperlink, description,
			responsible_persons, associated_modules, study_programs, institutions,
			raw_url, last_scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?, ?,
			?, ?, ?, ?,
			?, ?, ?, ?,
			?, ?, CURRENT_TIMESTAMP
		)
		ON CONFLICT(id) DO UPDATE SET
			event_number = excluded.event_number,
			title = excluded.title,
			event_type = excluded.event_type,
			semester = excluded.semester,
			sws = excluded.sws,
			expected_participants = excluded.expected_participants,
			max_participants = excluded.max_participants,
			hyperlink = excluded.hyperlink,
			description = excluded.description,
			responsible_persons = excluded.responsible_persons,
			associated_modules = excluded.associated_modules,
			study_programs = excluded.study_programs,
			institutions = excluded.institutions,
			raw_url = excluded.raw_url,
			last_scraped_at = excluded.last_scraped_at,
			updated_at = CURRENT_TIMESTAMP
	`

	_, err = tx.Exec(query,
		e.ID, e.EventNumber, e.Title, e.EventType, e.Semester, e.SWS,
		e.ExpectedParticipants, e.MaxParticipants, e.Hyperlink, e.Description,
		string(respJSON), string(modsJSON), string(stgJSON), string(instJSON),
		e.RawURL, scrapedAt,
	)
	if err != nil {
		return fmt.Errorf("failed to upsert event %s: %w", e.ID, err)
	}

	// Replace existing schedules for this event
	if _, err := tx.Exec("DELETE FROM event_schedules WHERE event_id = ?", e.ID); err != nil {
		return fmt.Errorf("failed to clean up schedules for event %s: %w", e.ID, err)
	}

	if len(e.Schedules) > 0 {
		schedStmt, err := tx.Prepare(`
			INSERT INTO event_schedules (
				event_id, group_name, day_of_week, time_slot, start_time, end_time,
				rhythm, duration, room, room_url, instructor, instructor_url,
				comment, cancelled_dates
			) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		`)
		if err != nil {
			return err
		}
		defer schedStmt.Close()

		for _, sc := range e.Schedules {
			_, err := schedStmt.Exec(
				e.ID, sc.GroupName, sc.DayOfWeek, sc.TimeSlot, sc.StartTime, sc.EndTime,
				sc.Rhythm, sc.Duration, sc.Room, sc.RoomURL, sc.Instructor, sc.InstructorURL,
				sc.Comment, sc.CancelledDates,
			)
			if err != nil {
				return fmt.Errorf("failed to insert schedule for event %s: %w", e.ID, err)
			}
		}
	}

	// Link associated modules
	for _, modID := range e.AssociatedModules {
		_, _ = tx.Exec(`
			INSERT INTO module_events (module_id, event_id)
			VALUES (?, ?)
			ON CONFLICT(module_id, event_id) DO NOTHING
		`, modID, e.ID)
	}

	return tx.Commit()
}

// GetEvent retrieves a full event record including its schedules.
func (s *Storage) GetEvent(id string) (*model.EventDetail, error) {
	row := s.db.QueryRow(`
		SELECT
			id, event_number, title, event_type, semester, sws,
			expected_participants, max_participants, hyperlink, description,
			responsible_persons, associated_modules, study_programs, institutions,
			raw_url, last_scraped_at
		FROM events
		WHERE id = ? OR event_number = ?
	`, id, id)

	var (
		e                                        model.EventDetail
		respJSON, modsJSON, stgJSON, instJSON    sql.NullString
		sem, sws, expPart, maxPart, link, desc   sql.NullString
		url, scrapedAtStr, titleStr, typeStr, nr sql.NullString
	)

	err := row.Scan(
		&e.ID, &nr, &titleStr, &typeStr, &sem, &sws,
		&expPart, &maxPart, &link, &desc,
		&respJSON, &modsJSON, &stgJSON, &instJSON,
		&url, &scrapedAtStr,
	)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}

	e.EventNumber = nr.String
	e.Title = titleStr.String
	e.EventType = typeStr.String
	e.Semester = sem.String
	e.SWS = sws.String
	e.ExpectedParticipants = expPart.String
	e.MaxParticipants = maxPart.String
	e.Hyperlink = link.String
	e.Description = desc.String
	e.RawURL = url.String

	if respJSON.Valid && respJSON.String != "" {
		_ = json.Unmarshal([]byte(respJSON.String), &e.ResponsiblePersons)
	}
	if modsJSON.Valid && modsJSON.String != "" {
		_ = json.Unmarshal([]byte(modsJSON.String), &e.AssociatedModules)
	}
	if stgJSON.Valid && stgJSON.String != "" {
		_ = json.Unmarshal([]byte(stgJSON.String), &e.StudyPrograms)
	}
	if instJSON.Valid && instJSON.String != "" {
		_ = json.Unmarshal([]byte(instJSON.String), &e.Institutions)
	}
	if scrapedAtStr.Valid && scrapedAtStr.String != "" {
		if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
			e.LastScrapedAt = t
		}
	}

	schedules, err := s.getSchedulesForEvent(e.ID)
	if err == nil {
		e.Schedules = schedules
	}

	return &e, nil
}

// LinkModuleEvent explicitly links a module ID to an event ID.
func (s *Storage) LinkModuleEvent(moduleID, eventID string) error {
	_, err := s.db.Exec(`
		INSERT INTO module_events (module_id, event_id)
		VALUES (?, ?)
		ON CONFLICT(module_id, event_id) DO NOTHING
	`, moduleID, eventID)
	return err
}

// GetEventsForModule retrieves all events and schedules associated with a module ID.
func (s *Storage) GetEventsForModule(moduleID string) ([]model.EventDetail, error) {
	rows, err := s.db.Query(`
		SELECT DISTINCT
			e.id, e.event_number, e.title, e.event_type, e.semester, e.sws,
			e.expected_participants, e.max_participants, e.hyperlink, e.description,
			e.responsible_persons, e.associated_modules, e.study_programs, e.institutions,
			e.raw_url, e.last_scraped_at
		FROM events e
		INNER JOIN module_events me ON me.event_id = e.id
		WHERE me.module_id = ?
		ORDER BY e.event_type, e.title
	`, moduleID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var events []model.EventDetail
	for rows.Next() {
		var (
			e                                        model.EventDetail
			respJSON, modsJSON, stgJSON, instJSON    sql.NullString
			sem, sws, expPart, maxPart, link, desc   sql.NullString
			url, scrapedAtStr, titleStr, typeStr, nr sql.NullString
		)

		err := rows.Scan(
			&e.ID, &nr, &titleStr, &typeStr, &sem, &sws,
			&expPart, &maxPart, &link, &desc,
			&respJSON, &modsJSON, &stgJSON, &instJSON,
			&url, &scrapedAtStr,
		)
		if err != nil {
			return nil, err
		}

		e.EventNumber = nr.String
		e.Title = titleStr.String
		e.EventType = typeStr.String
		e.Semester = sem.String
		e.SWS = sws.String
		e.ExpectedParticipants = expPart.String
		e.MaxParticipants = maxPart.String
		e.Hyperlink = link.String
		e.Description = desc.String
		e.RawURL = url.String

		if respJSON.Valid && respJSON.String != "" {
			_ = json.Unmarshal([]byte(respJSON.String), &e.ResponsiblePersons)
		}
		if modsJSON.Valid && modsJSON.String != "" {
			_ = json.Unmarshal([]byte(modsJSON.String), &e.AssociatedModules)
		}
		if stgJSON.Valid && stgJSON.String != "" {
			_ = json.Unmarshal([]byte(stgJSON.String), &e.StudyPrograms)
		}
		if instJSON.Valid && instJSON.String != "" {
			_ = json.Unmarshal([]byte(instJSON.String), &e.Institutions)
		}
		if scrapedAtStr.Valid && scrapedAtStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
				e.LastScrapedAt = t
			}
		}

		schedules, err := s.getSchedulesForEvent(e.ID)
		if err == nil {
			e.Schedules = schedules
		}

		events = append(events, e)
	}

	return events, rows.Err()
}

func (s *Storage) getSchedulesForEvent(eventID string) ([]model.EventSchedule, error) {
	rows, err := s.db.Query(`
		SELECT
			group_name, day_of_week, time_slot, start_time, end_time,
			rhythm, duration, room, room_url, instructor, instructor_url,
			comment, cancelled_dates
		FROM event_schedules
		WHERE event_id = ?
		ORDER BY id ASC
	`, eventID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var schedules []model.EventSchedule
	for rows.Next() {
		var (
			sc                                                  model.EventSchedule
			grp, day, slot, start, end, rhythm, dur             sql.NullString
			room, roomURL, inst, instURL, comm, canc            sql.NullString
		)
		err := rows.Scan(
			&grp, &day, &slot, &start, &end,
			&rhythm, &dur, &room, &roomURL, &inst, &instURL,
			&comm, &canc,
		)
		if err != nil {
			return nil, err
		}

		sc.GroupName = grp.String
		sc.DayOfWeek = day.String
		sc.TimeSlot = slot.String
		sc.StartTime = start.String
		sc.EndTime = end.String
		sc.Rhythm = rhythm.String
		sc.Duration = dur.String
		sc.Room = room.String
		sc.RoomURL = roomURL.String
		sc.Instructor = inst.String
		sc.InstructorURL = instURL.String
		sc.Comment = comm.String
		sc.CancelledDates = canc.String

		schedules = append(schedules, sc)
	}

	return schedules, rows.Err()
}

// UpsertFUESList marks modules as FÜS in the database, inserting skeleton records if they do not yet exist.
func (s *Storage) UpsertFUESList(modules []model.FUESModule) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	stmt, err := tx.Prepare(`
		INSERT INTO modules (
			id, code, title_de, language, credits, credits_raw, limitation,
			cross_disciplinary, is_fues, raw_url, last_scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?, ?, ?,
			1, 1, ?, ?, CURRENT_TIMESTAMP
		)
		ON CONFLICT(id) DO UPDATE SET
			is_fues = 1,
			cross_disciplinary = 1,
			title_de = CASE WHEN modules.title_de IS NULL OR modules.title_de = '' THEN excluded.title_de ELSE modules.title_de END,
			language = CASE WHEN modules.language IS NULL OR modules.language = '' THEN excluded.language ELSE modules.language END,
			credits = CASE WHEN modules.credits IS NULL OR modules.credits = 0 THEN excluded.credits ELSE modules.credits END,
			credits_raw = CASE WHEN modules.credits_raw IS NULL OR modules.credits_raw = '' THEN excluded.credits_raw ELSE modules.credits_raw END,
			updated_at = CURRENT_TIMESTAMP
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, m := range modules {
		scrapedAt := m.ScrapedAt.Format(time.RFC3339)
		if _, err := stmt.Exec(
			m.ID, m.ID, m.Title, m.Language, m.Credits, m.CreditsRaw, m.Limitation,
			m.QISURL, scrapedAt,
		); err != nil {
			return fmt.Errorf("failed to upsert FUES module %s: %w", m.ID, err)
		}
	}

	return tx.Commit()
}

// ListMajors returns a sorted list of unique study programs / majors discovered across all modules.
func (s *Storage) ListMajors() ([]string, error) {
	rows, err := s.db.Query(`SELECT study_programs FROM modules WHERE study_programs IS NOT NULL AND study_programs != '' AND study_programs != '[]'`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	seen := make(map[string]bool)
	for rows.Next() {
		var raw sql.NullString
		if err := rows.Scan(&raw); err != nil {
			continue
		}
		if !raw.Valid || raw.String == "" {
			continue
		}
		var programs []model.StudyProgram
		if err := json.Unmarshal([]byte(raw.String), &programs); err == nil {
			for _, p := range programs {
				name := strings.TrimSpace(p.Program)
				if name != "" {
					seen[name] = true
				}
			}
		}
	}

	majors := make([]string, 0, len(seen))
	for m := range seen {
		majors = append(majors, m)
	}
	sort.Strings(majors)
	return majors, nil
}

// GetFUESForMajor returns all FÜS modules that are NOT adjacent to the given major.
func (s *Storage) GetFUESForMajor(major string, minCredits float64) ([]model.ModuleDetail, error) {
	query := `
		SELECT
			id, code, title_de, title_en, is_phase_out, department,
			responsible_persons, language, duration, turnus, credits, credits_raw,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory,
			teaching_forms, literature, exam_type, exam_details, grading, limitation,
			study_programs, remarks, associated_courses, current_semester_events,
			cross_disciplinary, is_fues, raw_url, last_scraped_at
		FROM modules
		WHERE (is_fues = 1 OR cross_disciplinary = 1)
	`
	var args []interface{}
	if minCredits > 0 {
		query += " AND credits >= ?"
		args = append(args, minCredits)
	}
	query += " ORDER BY CAST(id AS INTEGER), id ASC"

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	normMajor := strings.ToLower(strings.TrimSpace(major))
	var eligible []model.ModuleDetail

	for rows.Next() {
		var (
			d                                              model.ModuleDetail
			isPhaseOutInt, crossDiscInt, isFuesInt         int
			respJSON, tfJSON, litJSON, spJSON              sql.NullString
			coursesJSON, eventsJSON                        sql.NullString
			scrapedAtStr                                   sql.NullString
			titleEN, dept, lang, dur, turnus, credRaw      sql.NullString
			learnOut, contents, prereqRec, prereqMand      sql.NullString
			examType, examDet, grading, limit, remarks, url sql.NullString
		)

		err := rows.Scan(
			&d.ID, &d.Code, &d.TitleDE, &titleEN, &isPhaseOutInt, &dept,
			&respJSON, &lang, &dur, &turnus, &d.Credits, &credRaw,
			&learnOut, &contents, &prereqRec, &prereqMand,
			&tfJSON, &litJSON, &examType, &examDet, &grading, &limit,
			&spJSON, &remarks, &coursesJSON, &eventsJSON,
			&crossDiscInt, &isFuesInt, &url, &scrapedAtStr,
		)
		if err != nil {
			return nil, err
		}

		d.IsPhaseOut = isPhaseOutInt == 1
		d.CrossDisciplinary = crossDiscInt == 1
		d.IsFUES = isFuesInt == 1
		d.TitleEN = titleEN.String
		d.Department = dept.String
		d.Language = lang.String
		d.Duration = dur.String
		d.Turnus = turnus.String
		d.CreditsRaw = credRaw.String
		d.LearningOutcomes = learnOut.String
		d.Contents = contents.String
		d.PrerequisitesRecommended = prereqRec.String
		d.PrerequisitesMandatory = prereqMand.String
		d.ExamType = examType.String
		d.ExamDetails = examDet.String
		d.Grading = grading.String
		d.Limitation = limit.String
		d.Remarks = remarks.String
		d.RawURL = url.String

		if respJSON.Valid && respJSON.String != "" {
			_ = json.Unmarshal([]byte(respJSON.String), &d.ResponsiblePersons)
		}
		if tfJSON.Valid && tfJSON.String != "" {
			_ = json.Unmarshal([]byte(tfJSON.String), &d.TeachingForms)
		}
		if litJSON.Valid && litJSON.String != "" {
			_ = json.Unmarshal([]byte(litJSON.String), &d.Literature)
		}
		if spJSON.Valid && spJSON.String != "" {
			_ = json.Unmarshal([]byte(spJSON.String), &d.StudyPrograms)
		}
		if coursesJSON.Valid && coursesJSON.String != "" {
			_ = json.Unmarshal([]byte(coursesJSON.String), &d.AssociatedCourses)
		}
		if eventsJSON.Valid && eventsJSON.String != "" {
			_ = json.Unmarshal([]byte(eventsJSON.String), &d.CurrentSemesterEvents)
		}
		if scrapedAtStr.Valid && scrapedAtStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
				d.LastScrapedAt = t
			}
		}

		// Check adjacency against the requested major
		adjacent := false
		if normMajor != "" {
			for _, sp := range d.StudyPrograms {
				prog := strings.ToLower(sp.Program)
				raw := strings.ToLower(sp.Raw)
				if strings.Contains(prog, normMajor) || strings.Contains(raw, normMajor) {
					adjacent = true
					break
				}
			}
		}

		if !adjacent {
			eligible = append(eligible, d)
		}
	}

	return eligible, rows.Err()
}

// UpsertOfficialProgram inserts or updates an official study program record in SQLite.
func (s *Storage) UpsertOfficialProgram(p *model.OfficialStudyProgram) error {
	docsJSON, err := json.Marshal(p.Documents)
	if err != nil {
		return fmt.Errorf("failed to marshal documents: %w", err)
	}

	scrapedAt := p.ScrapedAt.Format(time.RFC3339)
	if p.ScrapedAt.IsZero() {
		scrapedAt = time.Now().UTC().Format(time.RFC3339)
	}

	query := `
		INSERT INTO official_study_programs (
			id, program_name, program_code, degree, degree_code,
			po_version, qis_node_id, qis_url, documents, scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?,
			?, ?, ?, ?, ?, CURRENT_TIMESTAMP
		)
		ON CONFLICT(id) DO UPDATE SET
			program_name = excluded.program_name,
			program_code = excluded.program_code,
			degree = excluded.degree,
			degree_code = excluded.degree_code,
			po_version = excluded.po_version,
			qis_node_id = excluded.qis_node_id,
			qis_url = excluded.qis_url,
			documents = excluded.documents,
			scraped_at = excluded.scraped_at,
			updated_at = CURRENT_TIMESTAMP
	`

	_, err = s.db.Exec(query,
		p.ID, p.ProgramName, p.ProgramCode, p.Degree, p.DegreeCode,
		p.POVersion, p.QISNodeID, p.QISURL, string(docsJSON), scrapedAt,
	)
	return err
}

// UpsertOfficialPrograms batch upserts multiple official study programs in a transaction.
func (s *Storage) UpsertOfficialPrograms(programs []model.OfficialStudyProgram) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	stmt, err := tx.Prepare(`
		INSERT INTO official_study_programs (
			id, program_name, program_code, degree, degree_code,
			po_version, qis_node_id, qis_url, documents, scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?,
			?, ?, ?, ?, ?, CURRENT_TIMESTAMP
		)
		ON CONFLICT(id) DO UPDATE SET
			program_name = excluded.program_name,
			program_code = excluded.program_code,
			degree = excluded.degree,
			degree_code = excluded.degree_code,
			po_version = excluded.po_version,
			qis_node_id = excluded.qis_node_id,
			qis_url = excluded.qis_url,
			documents = excluded.documents,
			scraped_at = excluded.scraped_at,
			updated_at = CURRENT_TIMESTAMP
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, p := range programs {
		docsJSON, _ := json.Marshal(p.Documents)
		scrapedAt := p.ScrapedAt.Format(time.RFC3339)
		if p.ScrapedAt.IsZero() {
			scrapedAt = time.Now().UTC().Format(time.RFC3339)
		}

		if _, err := stmt.Exec(
			p.ID, p.ProgramName, p.ProgramCode, p.Degree, p.DegreeCode,
			p.POVersion, p.QISNodeID, p.QISURL, string(docsJSON), scrapedAt,
		); err != nil {
			return fmt.Errorf("failed to upsert study program %s: %w", p.ID, err)
		}
	}

	return tx.Commit()
}

// ListOfficialPrograms returns stored official study programs, optionally filtered by program name or degree.
func (s *Storage) ListOfficialPrograms(programFilter, degreeFilter string) ([]model.OfficialStudyProgram, error) {
	query := `
		SELECT
			id, program_name, program_code, degree, degree_code,
			po_version, qis_node_id, qis_url, documents, scraped_at
		FROM official_study_programs
		WHERE 1=1
	`
	var args []interface{}
	if programFilter != "" {
		query += " AND program_name LIKE ?"
		args = append(args, "%"+programFilter+"%")
	}
	if degreeFilter != "" {
		query += " AND degree LIKE ?"
		args = append(args, "%"+degreeFilter+"%")
	}
	query += " ORDER BY program_name ASC, degree ASC, po_version DESC"

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.OfficialStudyProgram
	for rows.Next() {
		var (
			p                                                  model.OfficialStudyProgram
			pCode, dCode, nodeID, qisURL, docsJSON, scrapedStr sql.NullString
		)

		if err := rows.Scan(
			&p.ID, &p.ProgramName, &pCode, &p.Degree, &dCode,
			&p.POVersion, &nodeID, &qisURL, &docsJSON, &scrapedStr,
		); err != nil {
			return nil, err
		}

		p.ProgramCode = pCode.String
		p.DegreeCode = dCode.String
		p.QISNodeID = nodeID.String
		p.QISURL = qisURL.String

		if docsJSON.Valid && docsJSON.String != "" {
			_ = json.Unmarshal([]byte(docsJSON.String), &p.Documents)
		}
		if scrapedStr.Valid && scrapedStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedStr.String); err == nil {
				p.ScrapedAt = t
			}
		}

		results = append(results, p)
	}

	return results, rows.Err()
}

// GetOfficialProgram retrieves a single official study program by ID.
func (s *Storage) GetOfficialProgram(id string) (*model.OfficialStudyProgram, error) {
	row := s.db.QueryRow(`
		SELECT
			id, program_name, program_code, degree, degree_code,
			po_version, qis_node_id, qis_url, documents, scraped_at
		FROM official_study_programs
		WHERE id = ?
	`, id)

	var (
		p                                                  model.OfficialStudyProgram
		pCode, dCode, nodeID, qisURL, docsJSON, scrapedStr sql.NullString
	)

	if err := row.Scan(
		&p.ID, &p.ProgramName, &pCode, &p.Degree, &dCode,
		&p.POVersion, &nodeID, &qisURL, &docsJSON, &scrapedStr,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}

	p.ProgramCode = pCode.String
	p.DegreeCode = dCode.String
	p.QISNodeID = nodeID.String
	p.QISURL = qisURL.String

	if docsJSON.Valid && docsJSON.String != "" {
		_ = json.Unmarshal([]byte(docsJSON.String), &p.Documents)
	}
	if scrapedStr.Valid && scrapedStr.String != "" {
		if t, err := time.Parse(time.RFC3339, scrapedStr.String); err == nil {
			p.ScrapedAt = t
		}
	}

	return &p, nil
}

// LinkModuleToStudyProgram associates a module ID with an official study program ID.
func (s *Storage) LinkModuleToStudyProgram(moduleID, programID, progName, degree, regulation string) error {
	_, err := s.db.Exec(`
		INSERT INTO module_study_programs (module_id, program_id, program_name, degree, regulation)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(module_id, program_id) DO UPDATE SET
			program_name = excluded.program_name,
			degree = excluded.degree,
			regulation = excluded.regulation
	`, moduleID, programID, progName, degree, regulation)
	return err
}

// GetLinkedStudyProgramsForModule retrieves all official study programs linked to a module.
func (s *Storage) GetLinkedStudyProgramsForModule(moduleID string) ([]model.OfficialStudyProgram, error) {
	rows, err := s.db.Query(`
		SELECT
			op.id, op.program_name, op.program_code, op.degree, op.degree_code,
			op.po_version, op.qis_node_id, op.qis_url, op.documents, op.scraped_at
		FROM official_study_programs op
		INNER JOIN module_study_programs msp ON msp.program_id = op.id
		WHERE msp.module_id = ?
		ORDER BY op.program_name, op.degree, op.po_version DESC
	`, moduleID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.OfficialStudyProgram
	for rows.Next() {
		var (
			p                                                  model.OfficialStudyProgram
			pCode, dCode, nodeID, qisURL, docsJSON, scrapedStr sql.NullString
		)
		if err := rows.Scan(
			&p.ID, &p.ProgramName, &pCode, &p.Degree, &dCode,
			&p.POVersion, &nodeID, &qisURL, &docsJSON, &scrapedStr,
		); err != nil {
			return nil, err
		}
		p.ProgramCode = pCode.String
		p.DegreeCode = dCode.String
		p.QISNodeID = nodeID.String
		p.QISURL = qisURL.String
		if docsJSON.Valid && docsJSON.String != "" {
			_ = json.Unmarshal([]byte(docsJSON.String), &p.Documents)
		}
		if scrapedStr.Valid && scrapedStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedStr.String); err == nil {
				p.ScrapedAt = t
			}
		}
		results = append(results, p)
	}
	return results, rows.Err()
}

// GetModulesForStudyProgram returns all modules linked to a given official study program.
func (s *Storage) GetModulesForStudyProgram(programIDOrName string) ([]model.ModuleSummary, error) {
	query := `
		SELECT DISTINCT
			m.id, m.code, m.title_de, m.raw_url, m.last_scraped_at
		FROM modules m
		INNER JOIN module_study_programs msp ON msp.module_id = m.id
		WHERE msp.program_id = ? OR LOWER(msp.program_name) = LOWER(?) OR LOWER(msp.program_name) LIKE ?
		ORDER BY CAST(m.id AS INTEGER), m.id ASC
	`
	rows, err := s.db.Query(query, programIDOrName, programIDOrName, "%"+strings.ToLower(programIDOrName)+"%")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.ModuleSummary
	for rows.Next() {
		var (
			m            model.ModuleSummary
			code, title  sql.NullString
			urlStr       sql.NullString
			scrapedAtStr sql.NullString
		)
		if err := rows.Scan(&m.ID, &code, &title, &urlStr, &scrapedAtStr); err != nil {
			return nil, err
		}
		m.Code = code.String
		m.Title = title.String
		m.URL = urlStr.String
		if scrapedAtStr.Valid && scrapedAtStr.String != "" {
			if t, err := time.Parse(time.RFC3339, scrapedAtStr.String); err == nil {
				m.ScrapedAt = t
			}
		}
		results = append(results, m)
	}
	return results, rows.Err()
}

// FindOfficialProgram searches for the best matching official study program in the database.
func (s *Storage) FindOfficialProgram(name, degree, regulation string) (*model.OfficialStudyProgram, error) {
	normName := strings.TrimSpace(name)
	if normName == "" {
		return nil, ErrNotFound
	}

	normDegree := strings.TrimSpace(degree)
	cleanReg := strings.TrimPrefix(strings.TrimSpace(regulation), "PO ")

	// 1. Exact match on program name and degree and regulation
	if normDegree != "" && cleanReg != "" {
		row := s.db.QueryRow(`
			SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at
			FROM official_study_programs
			WHERE LOWER(program_name) = LOWER(?) AND LOWER(degree) = LOWER(?) AND (po_version LIKE ? OR ? LIKE '%' || po_version || '%')
			LIMIT 1
		`, normName, normDegree, "%"+cleanReg+"%", cleanReg)
		if p, err := scanOfficialProgram(row); err == nil {
			return p, nil
		}
	}

	// 2. Exact match on program name and degree
	if normDegree != "" {
		row := s.db.QueryRow(`
			SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at
			FROM official_study_programs
			WHERE LOWER(program_name) = LOWER(?) AND LOWER(degree) = LOWER(?)
			ORDER BY po_version DESC LIMIT 1
		`, normName, normDegree)
		if p, err := scanOfficialProgram(row); err == nil {
			return p, nil
		}
	}

	// 3. Match on program name alone
	row := s.db.QueryRow(`
		SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at
		FROM official_study_programs
		WHERE LOWER(program_name) = LOWER(?)
		ORDER BY po_version DESC LIMIT 1
	`, normName)
	if p, err := scanOfficialProgram(row); err == nil {
		return p, nil
	}

	// 4. Case-insensitive substring match
	row = s.db.QueryRow(`
		SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at
		FROM official_study_programs
		WHERE LOWER(program_name) LIKE ?
		ORDER BY po_version DESC LIMIT 1
	`, "%"+strings.ToLower(normName)+"%")
	if p, err := scanOfficialProgram(row); err == nil {
		return p, nil
	}

	return nil, ErrNotFound
}

func scanOfficialProgram(scanner interface{ Scan(...interface{}) error }) (*model.OfficialStudyProgram, error) {
	var (
		p                                                  model.OfficialStudyProgram
		pCode, dCode, nodeID, qisURL, docsJSON, scrapedStr sql.NullString
	)

	if err := scanner.Scan(
		&p.ID, &p.ProgramName, &pCode, &p.Degree, &dCode,
		&p.POVersion, &nodeID, &qisURL, &docsJSON, &scrapedStr,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}

	p.ProgramCode = pCode.String
	p.DegreeCode = dCode.String
	p.QISNodeID = nodeID.String
	p.QISURL = qisURL.String

	if docsJSON.Valid && docsJSON.String != "" {
		_ = json.Unmarshal([]byte(docsJSON.String), &p.Documents)
	}
	if scrapedStr.Valid && scrapedStr.String != "" {
		if t, err := time.Parse(time.RFC3339, scrapedStr.String); err == nil {
			p.ScrapedAt = t
		}
	}

	return &p, nil
}

// AdvancedFilter specifies criteria for web search and smart filtering.
type AdvancedFilter struct {
	Query            string  // searches title_de, title_en, code, id, responsible_persons
	ProgramID        string  // matches module_study_programs.program_id
	ProgramName      string  // matches module_study_programs.program_name
	SemesterTurnus   string  // "wise", "sose", "all"
	MinCredits       float64
	MaxCredits       float64
	Language         string  // "Deutsch", "Englisch", "all"
	OnlyFUES         bool
	ExcludePhaseOut  bool    // excludes modules where is_phase_out = 1
	NonAdjacentMajor string  // excludes modules where major is in study_programs
	Limit            int
	Offset           int
}

// ModuleCardItem represents a module card rendered in the web catalog.
type ModuleCardItem struct {
	ID                       string   `json:"id"`
	Code                     string   `json:"code"`
	TitleDE                  string   `json:"title_de"`
	TitleEN                  string   `json:"title_en"`
	Credits                  float64  `json:"credits"`
	CreditsRaw               string   `json:"credits_raw"`
	Language                 string   `json:"language"`
	Department               string   `json:"department"`
	Turnus                   string   `json:"turnus"`
	ExamType                 string   `json:"exam_type"`
	IsPhaseOut               bool     `json:"is_phase_out"`
	IsFUES                   bool     `json:"is_fues"`
	PrerequisitesMandatory   string   `json:"prerequisites_mandatory"`
	PrerequisitesRecommended string   `json:"prerequisites_recommended"`
	MandatoryPrereqIDs       []string `json:"mandatory_prereq_ids"`
	RecommendedPrereqIDs     []string `json:"recommended_prereq_ids"`
	ResponsiblePersons       []string `json:"responsible_persons"`
	EventsCount              int      `json:"events_count"`
	RawURL                   string   `json:"raw_url"`
}

// StudyProgramOption represents a selectable study program in the UI.
type StudyProgramOption struct {
	ID          string `json:"id"`
	ProgramName string `json:"program_name"`
	Degree      string `json:"degree"`
	ShortTitle  string `json:"short_title"` // e.g. "Informatik (Bachelor)"
	POVersion   string `json:"po_version"`
	Count       int    `json:"count"` // number of linked modules
}

var rePrereqID = regexp.MustCompile(`\b\d{5}\b`)

// ExtractPrereqIDs parses a prerequisite text and extracts all referenced 5-digit module IDs.
func ExtractPrereqIDs(text string) []string {
	if text == "" || strings.EqualFold(strings.TrimSpace(text), "keine") {
		return nil
	}
	matches := rePrereqID.FindAllString(text, -1)
	if len(matches) == 0 {
		return nil
	}
	seen := make(map[string]bool)
	var unique []string
	for _, m := range matches {
		if !seen[m] {
			seen[m] = true
			unique = append(unique, m)
		}
	}
	return unique
}

// SearchModulesAdvanced executes an advanced query with smart filters and pagination.
func (s *Storage) SearchModulesAdvanced(f AdvancedFilter) ([]ModuleCardItem, int, error) {
	var whereClauses []string
	var args []interface{}
	var joins []string

	if f.ProgramID != "" || f.ProgramName != "" {
		joins = append(joins, "INNER JOIN module_study_programs msp ON msp.module_id = m.id")
		if f.ProgramID != "" {
			whereClauses = append(whereClauses, "msp.program_id = ?")
			args = append(args, f.ProgramID)
		} else {
			whereClauses = append(whereClauses, "(LOWER(msp.program_name) = LOWER(?) OR LOWER(msp.program_name) LIKE ?)")
			args = append(args, f.ProgramName, "%"+strings.ToLower(f.ProgramName)+"%")
		}
	}

	if f.Query != "" {
		q := "%" + strings.ToLower(f.Query) + "%"
		whereClauses = append(whereClauses, "(m.id LIKE ? OR m.code LIKE ? OR LOWER(m.title_de) LIKE ? OR LOWER(m.title_en) LIKE ? OR LOWER(m.department) LIKE ? OR LOWER(m.responsible_persons) LIKE ?)")
		args = append(args, q, q, q, q, q, q)
	}

	if f.SemesterTurnus == "wise" {
		whereClauses = append(whereClauses, "(LOWER(m.turnus) LIKE '%winter%' OR LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%')")
	} else if f.SemesterTurnus == "sose" {
		whereClauses = append(whereClauses, "(LOWER(m.turnus) LIKE '%sommer%' OR LOWER(m.turnus) LIKE '%summer%' OR LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%')")
	}

	if f.OnlyFUES {
		whereClauses = append(whereClauses, "m.is_fues = 1")
	}

	if f.ExcludePhaseOut {
		whereClauses = append(whereClauses, "m.is_phase_out = 0")
	}

	if f.NonAdjacentMajor != "" {
		whereClauses = append(whereClauses, "m.id NOT IN (SELECT module_id FROM module_study_programs WHERE LOWER(program_name) LIKE ?)")
		args = append(args, "%"+strings.ToLower(f.NonAdjacentMajor)+"%")
	}

	if f.Language != "" && !strings.EqualFold(f.Language, "all") {
		whereClauses = append(whereClauses, "LOWER(m.language) LIKE ?")
		args = append(args, "%"+strings.ToLower(f.Language)+"%")
	}

	if f.MinCredits > 0 {
		whereClauses = append(whereClauses, "m.credits >= ?")
		args = append(args, f.MinCredits)
	}
	if f.MaxCredits > 0 {
		whereClauses = append(whereClauses, "m.credits <= ?")
		args = append(args, f.MaxCredits)
	}

	joinSQL := strings.Join(joins, " ")
	whereSQL := ""
	if len(whereClauses) > 0 {
		whereSQL = "WHERE " + strings.Join(whereClauses, " AND ")
	}

	// 1. Get total count
	countQuery := fmt.Sprintf("SELECT COUNT(DISTINCT m.id) FROM modules m %s %s", joinSQL, whereSQL)
	var total int
	if err := s.db.QueryRow(countQuery, args...).Scan(&total); err != nil {
		return nil, 0, err
	}

	// 2. Query items
	limit := f.Limit
	if limit <= 0 {
		limit = 50
	}
	if limit > 200 {
		limit = 200
	}
	offset := f.Offset
	if offset < 0 {
		offset = 0
	}

	selectQuery := fmt.Sprintf(`
		SELECT DISTINCT
			m.id, m.code, m.title_de, m.title_en, m.is_phase_out, m.department,
			m.responsible_persons, m.language, m.turnus, m.credits, m.credits_raw,
			m.prerequisites_recommended, m.prerequisites_mandatory, m.exam_type,
			m.is_fues, m.current_semester_events, m.raw_url
		FROM modules m
		%s
		%s
		ORDER BY CAST(m.id AS INTEGER), m.id ASC
		LIMIT ? OFFSET ?
	`, joinSQL, whereSQL)

	queryArgs := append(args, limit, offset)
	rows, err := s.db.Query(selectQuery, queryArgs...)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()

	var items []ModuleCardItem
	for rows.Next() {
		var (
			item                                                  ModuleCardItem
			isPhaseOutInt, isFuesInt                              int
			titleEN, dept, lang, turnus, credRaw                  sql.NullString
			prereqRec, prereqMand, examType, urlStr               sql.NullString
			respJSON, eventsJSON                                  sql.NullString
		)
		if err := rows.Scan(
			&item.ID, &item.Code, &item.TitleDE, &titleEN, &isPhaseOutInt, &dept,
			&respJSON, &lang, &turnus, &item.Credits, &credRaw,
			&prereqRec, &prereqMand, &examType,
			&isFuesInt, &eventsJSON, &urlStr,
		); err != nil {
			return nil, 0, err
		}

		item.TitleEN = titleEN.String
		item.IsPhaseOut = isPhaseOutInt == 1
		item.IsFUES = isFuesInt == 1
		item.Department = dept.String
		item.Language = lang.String
		item.Turnus = turnus.String
		item.CreditsRaw = credRaw.String
		item.PrerequisitesRecommended = prereqRec.String
		item.PrerequisitesMandatory = prereqMand.String
		item.ExamType = examType.String
		item.RawURL = urlStr.String

		if respJSON.Valid && respJSON.String != "" {
			_ = json.Unmarshal([]byte(respJSON.String), &item.ResponsiblePersons)
		}
		if eventsJSON.Valid && eventsJSON.String != "" {
			var events []model.ModuleEvent
			if err := json.Unmarshal([]byte(eventsJSON.String), &events); err == nil {
				item.EventsCount = len(events)
			}
		}

		item.MandatoryPrereqIDs = ExtractPrereqIDs(item.PrerequisitesMandatory)
		item.RecommendedPrereqIDs = ExtractPrereqIDs(item.PrerequisitesRecommended)

		items = append(items, item)
	}

	return items, total, rows.Err()
}

// FormatDegreeShort simplifies official degree descriptions (e.g. "Bachelor (universitär)" -> "Bachelor").
func FormatDegreeShort(degree string) string {
	deg := strings.TrimSpace(degree)
	low := strings.ToLower(deg)
	switch {
	case strings.Contains(low, "bachelor"):
		if strings.Contains(low, "dual") {
			return "Bachelor (dual)"
		}
		return "Bachelor"
	case strings.Contains(low, "master"):
		if strings.Contains(low, "dual") {
			return "Master (dual)"
		}
		return "Master"
	case strings.Contains(low, "diplom"):
		return "Diplom"
	case strings.Contains(low, "orientierung"):
		return "Orientierungsstudium"
	case strings.Contains(low, "abschluss im ausland"):
		return "Auslandsabschluss"
	default:
		deg = strings.ReplaceAll(deg, "(universitär)", "")
		deg = strings.ReplaceAll(deg, "(fachhochschulisch)", "")
		deg = strings.TrimSpace(deg)
		if deg == "" {
			return "Sonstiges"
		}
		return deg
	}
}

// FormatProgramShort formats a study program as a short title, e.g. "Informatik (Bachelor)".
func FormatProgramShort(name, degree string) string {
	cleanName := strings.TrimSpace(name)
	shortDeg := FormatDegreeShort(degree)
	if shortDeg != "" && !strings.EqualFold(cleanName, shortDeg) {
		return fmt.Sprintf("%s (%s)", cleanName, shortDeg)
	}
	return cleanName
}

// GetAllStudyPrograms returns official study programs with their linked module count.
func (s *Storage) GetAllStudyPrograms() ([]StudyProgramOption, error) {
	query := `
		SELECT 
			p.id, p.program_name, p.degree, p.po_version,
			(SELECT COUNT(DISTINCT msp.module_id) FROM module_study_programs msp WHERE msp.program_id = p.id) as mod_count
		FROM official_study_programs p
		ORDER BY p.program_name ASC, p.degree ASC, p.po_version DESC
	`
	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var options []StudyProgramOption
	seenTitles := make(map[string]int)

	for rows.Next() {
		var opt StudyProgramOption
		if err := rows.Scan(&opt.ID, &opt.ProgramName, &opt.Degree, &opt.POVersion, &opt.Count); err != nil {
			return nil, err
		}
		opt.ShortTitle = FormatProgramShort(opt.ProgramName, opt.Degree)
		seenTitles[opt.ShortTitle]++
		options = append(options, opt)
	}

	// Disambiguate if multiple PO versions exist for the same ShortTitle
	for i := range options {
		if seenTitles[options[i].ShortTitle] > 1 && options[i].POVersion != "" {
			options[i].ShortTitle = fmt.Sprintf("%s - PO %s", options[i].ShortTitle, options[i].POVersion)
		}
	}

	return options, rows.Err()
}

// GetProgramTotalModules returns the total count of modules assigned to a program, or total in catalog if programID is empty.
func (s *Storage) GetProgramTotalModules(programID string) (int, error) {
	if programID == "" {
		var total int
		err := s.db.QueryRow("SELECT COUNT(*) FROM modules").Scan(&total)
		return total, err
	}
	var total int
	err := s.db.QueryRow("SELECT COUNT(DISTINCT module_id) FROM module_study_programs WHERE program_id = ?", programID).Scan(&total)
	return total, err
}

