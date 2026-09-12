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
	"unicode"

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
	succJSON, _ := json.Marshal(d.SuccessorModules)

	isPhaseOutInt := 0
	if d.IsPhaseOut {
		isPhaseOutInt = 1
	}
	isNotOfferedInt := 0
	if d.IsNotOffered {
		isNotOfferedInt = 1
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
			id, code, title_de, title_en, is_phase_out, is_not_offered, department,
			responsible_persons, successor_modules, language, duration, turnus, credits, credits_raw,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory,
			teaching_forms, literature, exam_type, exam_details, grading, limitation,
			study_programs, remarks, associated_courses, current_semester_events,
			cross_disciplinary, is_fues, raw_url, last_scraped_at, updated_at
		) VALUES (
			?, ?, ?, ?, ?, ?, ?,
			?, ?, ?, ?, ?, ?, ?,
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
			is_not_offered = excluded.is_not_offered,
			department = excluded.department,
			responsible_persons = excluded.responsible_persons,
			successor_modules = excluded.successor_modules,
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
		d.ID, d.Code, d.TitleDE, d.TitleEN, isPhaseOutInt, isNotOfferedInt, d.Department,
		string(respJSON), string(succJSON), d.Language, d.Duration, d.Turnus, d.Credits, d.CreditsRaw,
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
			id, code, title_de, title_en, is_phase_out, is_not_offered, department,
			responsible_persons, successor_modules, language, duration, turnus, credits, credits_raw,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory,
			teaching_forms, literature, exam_type, exam_details, grading, limitation,
			study_programs, remarks, associated_courses, current_semester_events,
			cross_disciplinary, is_fues, raw_url, last_scraped_at
		FROM modules
		WHERE id = ? OR code = ?
	`, id, id)

	var (
		d                                              model.ModuleDetail
		isPhaseOutInt, isNotOfferedInt                 int
		crossDiscInt, isFuesInt                        int
		respJSON, succJSON, tfJSON, litJSON, spJSON    sql.NullString
		coursesJSON, eventsJSON                        sql.NullString
		scrapedAtStr                                   sql.NullString
		titleEN, dept, lang, dur, turnus, credRaw      sql.NullString
		learnOut, contents, prereqRec, prereqMand      sql.NullString
		examType, examDet, grading, limit, remarks, url sql.NullString
	)

	err := row.Scan(
		&d.ID, &d.Code, &d.TitleDE, &titleEN, &isPhaseOutInt, &isNotOfferedInt, &dept,
		&respJSON, &succJSON, &lang, &dur, &turnus, &d.Credits, &credRaw,
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
	d.IsNotOffered = isNotOfferedInt == 1
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
	if succJSON.Valid && succJSON.String != "" {
		_ = json.Unmarshal([]byte(succJSON.String), &d.SuccessorModules)
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
			id, code, title_de, title_en, is_phase_out, is_not_offered, department,
			responsible_persons, successor_modules, language, duration, turnus, credits, credits_raw,
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
			isPhaseOutInt, isNotOfferedInt                 int
			crossDiscInt, isFuesInt                        int
			respJSON, succJSON, tfJSON, litJSON, spJSON    sql.NullString
			coursesJSON, eventsJSON                        sql.NullString
			scrapedAtStr                                   sql.NullString
			titleEN, dept, lang, dur, turnus, credRaw      sql.NullString
			learnOut, contents, prereqRec, prereqMand      sql.NullString
			examType, examDet, grading, limit, remarks, url sql.NullString
		)

		err := rows.Scan(
			&d.ID, &d.Code, &d.TitleDE, &titleEN, &isPhaseOutInt, &isNotOfferedInt, &dept,
			&respJSON, &succJSON, &lang, &dur, &turnus, &d.Credits, &credRaw,
			&learnOut, &contents, &prereqRec, &prereqMand,
			&tfJSON, &litJSON, &examType, &examDet, &grading, &limit,
			&spJSON, &remarks, &coursesJSON, &eventsJSON,
			&crossDiscInt, &isFuesInt, &url, &scrapedAtStr,
		)
		if err != nil {
			return nil, err
		}

		d.IsPhaseOut = isPhaseOutInt == 1
		d.IsNotOffered = isNotOfferedInt == 1
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
		if succJSON.Valid && succJSON.String != "" {
			_ = json.Unmarshal([]byte(succJSON.String), &d.SuccessorModules)
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
	if strings.EqualFold(normDegree, "abschluss im ausland") || strings.Contains(strings.ToLower(normDegree), "ausland") {
		return nil, ErrNotFound
	}

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

	// 2b. Match on program name and normalized short degree (e.g. Bachelor vs Bachelor (universitär))
	if normDegree != "" {
		shortDeg := FormatDegreeShort(normDegree)
		rows, err := s.db.Query(`
			SELECT id, program_name, program_code, degree, degree_code, po_version, qis_node_id, qis_url, documents, scraped_at
			FROM official_study_programs
			WHERE LOWER(program_name) = LOWER(?)
			ORDER BY po_version DESC
		`, normName)
		if err == nil {
			defer rows.Close()
			for rows.Next() {
				if p, err := scanOfficialProgram(rows); err == nil {
					if FormatDegreeShort(p.Degree) == shortDeg {
						return p, nil
					}
				}
			}
		}
	}

	// 3. Match on program name alone ONLY if degree was not specified
	if normDegree == "" {
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
	Query            string   // searches title_de, title_en, code, id, responsible_persons
	ProgramID        string   // matches module_study_programs.program_id
	ProgramName      string   // matches module_study_programs.program_name
	SemesterTurnus   string   // legacy / single turnus: "all", "next", "sose", "sose_even", "sose_odd", "wise", "wise_even", "wise_odd", "sporadic"
	SemesterTurnuses []string // multi-select turnuses: e.g. ["wise_odd", "sose_even", "sporadic"]
	MinCredits       float64
	MaxCredits       float64
	Language         string   // legacy / single language query
	Languages        []string // ["Deutsch", "English"]
	Campuses         []string // ["hauptcampus", "sachsendorf", "senftenberg"]
	CampusStrict     bool     // if true, all events must strictly be at the selected campus(es)
	Limitation       string   // "ja" (all), "nein" (unlimited only), "nur" (limited only)
	OnlyFUES         bool     // legacy flag: equivalent to FUESFilter = "nur"
	FUESFilter       string   // "inkl", "exkl", "nur"
	Instructors      []string // whitelist of professors/instructors (matches responsible_persons)
	ExcludePhaseOut  bool     // excludes modules where is_phase_out = 1
	NonAdjacentMajor string   // excludes modules where major is in study_programs
	Limit            int
	Offset           int
}

// ModuleCardItem represents a module card rendered in the web catalog.
type ModuleCardItem struct {
	ID                       string                    `json:"id"`
	Code                     string                    `json:"code"`
	TitleDE                  string                    `json:"title_de"`
	TitleEN                  string                    `json:"title_en"`
	Credits                  float64                   `json:"credits"`
	CreditsRaw               string                    `json:"credits_raw"`
	Language                 string                    `json:"language"`
	Department               string                    `json:"department"`
	Turnus                   string                    `json:"turnus"`
	Limitation               string                    `json:"limitation"`
	ExamType                 string                    `json:"exam_type"`
	IsPhaseOut               bool                      `json:"is_phase_out"`
	IsNotOffered             bool                      `json:"is_not_offered"`
	SuccessorModules         []string                  `json:"successor_modules,omitempty"`
	IsFUES                   bool                      `json:"is_fues"`
	PrerequisitesMandatory   string                    `json:"prerequisites_mandatory"`
	PrerequisitesRecommended string                    `json:"prerequisites_recommended"`
	MandatoryPrereqIDs       []string                  `json:"mandatory_prereq_ids"`
	RecommendedPrereqIDs     []string                  `json:"recommended_prereq_ids"`
	ResponsiblePersons       []model.ResponsiblePerson `json:"responsible_persons"`
	EventsCount              int                       `json:"events_count"`
	RawURL                   string                    `json:"raw_url"`
}

// StudyProgramOption represents a selectable study program in the UI.
type StudyProgramOption struct {
	ID          string   `json:"id"`
	RelatedIDs  []string `json:"related_ids,omitempty"`
	ProgramName string   `json:"program_name"`
	Degree      string   `json:"degree"`
	ShortTitle  string   `json:"short_title"` // e.g. "Informatik (Bachelor)"
	POVersion   string   `json:"po_version"`
	Count       int      `json:"count"` // number of linked modules
}

// isVariantDegree returns true if the degree specifies a track qualifier/modifier
// such as "- Doppelabschluss", "- erweiterte Fachsemester", "- Duales Studium", etc.
func isVariantDegree(degree string) bool {
	low := strings.ToLower(degree)
	return strings.Contains(low, " - ") ||
		strings.Contains(low, "doppelabschluss") ||
		strings.Contains(low, "erweiterte fachsemester") ||
		strings.Contains(low, "verringerte fachsemester") ||
		strings.Contains(low, "teilzeitstudium") ||
		strings.Contains(low, "praxisintegrierend") ||
		strings.Contains(low, "fernstudium")
}

var rePrereqID = regexp.MustCompile(`\b\d{5}\b`)

// ExtractPrereqIDs parses a prerequisite text and extracts all referenced 5-digit module IDs.
func ExtractPrereqIDs(text string) []string {
	trimmed := strings.TrimSpace(text)
	if trimmed == "" || strings.EqualFold(trimmed, "keine") || trimmed == "-" || strings.EqualFold(trimmed, "none") {
		return nil
	}
	// Filter out negative prerequisite lines ("Keine erfolgreiche Teilnahme an...")
	var cleanLines []string
	lines := strings.Split(text, "\n")
	inExclusion := false
	for _, l := range lines {
		low := strings.ToLower(l)
		if strings.Contains(low, "keine erfolgreiche teilnahme") || strings.Contains(low, "ausschluss") {
			inExclusion = true
			continue
		}
		if inExclusion {
			if strings.HasPrefix(strings.TrimSpace(l), "•") || strings.HasPrefix(strings.TrimSpace(l), "-") {
				continue
			}
			inExclusion = false
		}
		cleanLines = append(cleanLines, l)
	}

	searchContent := strings.Join(cleanLines, "\n")
	matches := rePrereqID.FindAllString(searchContent, -1)
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

var stopWordsMap = map[string]bool{
	"und": true, "oder": true, "der": true, "die": true, "das": true, "des": true, "dem": true, "den": true,
	"in": true, "im": true, "für": true, "von": true, "vom": true, "mit": true, "zu": true, "zur": true,
	"zum": true, "an": true, "am": true, "auf": true, "aus": true, "bei": true, "and": true, "or": true,
	"the": true, "of": true, "for": true, "with": true, "to": true, "at": true,
}

func getInitialsVariantsGo(text string) []string {
	text = strings.TrimSpace(text)
	if text == "" {
		return nil
	}
	var uppers []rune
	for _, r := range text {
		if unicode.IsUpper(r) {
			uppers = append(uppers, unicode.ToLower(r))
		}
	}

	fields := strings.FieldsFunc(text, func(r rune) bool {
		return unicode.IsSpace(r) || r == '-' || r == '_' || r == '/' || r == '.' || r == ',' || r == '(' || r == ')'
	})

	var allInits []rune
	var sigInits []rune
	for _, f := range fields {
		clean := strings.TrimFunc(f, func(r rune) bool {
			return !unicode.IsLetter(r) && !unicode.IsDigit(r)
		})
		if len(clean) > 0 {
			first := []rune(clean)[0]
			allInits = append(allInits, unicode.ToLower(first))
			if !stopWordsMap[strings.ToLower(clean)] {
				sigInits = append(sigInits, unicode.ToLower(first))
			}
		}
	}

	seen := make(map[string]bool)
	var variants []string
	add := func(s string) {
		if len(s) >= 2 && !seen[s] {
			seen[s] = true
			variants = append(variants, s)
		}
	}

	if len(uppers) >= 2 {
		add(string(uppers))
	}
	if len(sigInits) >= 2 {
		add(string(sigInits))
	}
	if len(allInits) >= 2 {
		add(string(allInits))
	}
	return variants
}

func matchInitialsGo(title string, query string) bool {
	q := strings.ToLower(strings.TrimSpace(query))
	if len(q) < 2 {
		return false
	}
	variants := getInitialsVariantsGo(title)
	for _, v := range variants {
		if v == q || strings.HasPrefix(v, q) {
			return true
		}
		qRunes := []rune(q)
		vRunes := []rune(v)
		qIdx := 0
		for i := 0; i < len(vRunes) && qIdx < len(qRunes); i++ {
			if vRunes[i] == qRunes[qIdx] {
				qIdx++
			}
		}
		if qIdx == len(qRunes) {
			return true
		}
	}
	return false
}

// FindModuleIDsByInitials finds all module IDs whose title initials match the query.
func (s *Storage) FindModuleIDsByInitials(query string) []string {
	q := strings.ToLower(strings.TrimSpace(query))
	if len(q) < 2 || len(q) > 8 {
		return nil
	}

	rows, err := s.db.Query("SELECT id, title_de, title_en FROM modules")
	if err != nil {
		return nil
	}
	defer rows.Close()

	var matchingIDs []string
	for rows.Next() {
		var id, titleDE string
		var titleEN sql.NullString
		if err := rows.Scan(&id, &titleDE, &titleEN); err != nil {
			continue
		}
		if matchInitialsGo(titleDE, q) || (titleEN.Valid && matchInitialsGo(titleEN.String, q)) {
			matchingIDs = append(matchingIDs, id)
		}
	}
	return matchingIDs
}

// buildFilterClauses constructs the JOIN and WHERE clauses along with query args for an AdvancedFilter.
func (s *Storage) buildFilterClauses(f AdvancedFilter) (joinSQL string, whereSQL string, args []interface{}) {
	var whereClauses []string
	var joins []string

	if f.ProgramID != "" || f.ProgramName != "" {
		joins = append(joins, "INNER JOIN module_study_programs msp ON msp.module_id = m.id")
		if f.ProgramID != "" {
			relatedIDs := s.GetRelatedProgramIDs(f.ProgramID)
			if len(relatedIDs) <= 1 {
				whereClauses = append(whereClauses, "msp.program_id = ? AND msp.degree != 'Abschluss im Ausland'")
				if len(relatedIDs) == 1 {
					args = append(args, relatedIDs[0])
				} else {
					args = append(args, f.ProgramID)
				}
			} else {
				placeholders := make([]string, len(relatedIDs))
				for i, id := range relatedIDs {
					placeholders[i] = "?"
					args = append(args, id)
				}
				whereClauses = append(whereClauses, fmt.Sprintf("msp.program_id IN (%s) AND msp.degree != 'Abschluss im Ausland'", strings.Join(placeholders, ",")))
			}
		} else {
			whereClauses = append(whereClauses, "(LOWER(msp.program_name) = LOWER(?) OR LOWER(msp.program_name) LIKE ?) AND msp.degree != 'Abschluss im Ausland'")
			args = append(args, f.ProgramName, "%"+strings.ToLower(f.ProgramName)+"%")
		}
	}

	if f.Query != "" {
		q := "%" + strings.ToLower(f.Query) + "%"
		subClauses := []string{
			"m.id LIKE ?",
			"m.code LIKE ?",
			"LOWER(m.title_de) LIKE ?",
			"LOWER(m.title_en) LIKE ?",
			"LOWER(m.department) LIKE ?",
			"LOWER(m.responsible_persons) LIKE ?",
		}
		args = append(args, q, q, q, q, q, q)

		initialsIDs := s.FindModuleIDsByInitials(f.Query)
		if len(initialsIDs) > 0 {
			placeholders := make([]string, len(initialsIDs))
			for i, id := range initialsIDs {
				placeholders[i] = "?"
				args = append(args, id)
			}
			subClauses = append(subClauses, fmt.Sprintf("m.id IN (%s)", strings.Join(placeholders, ",")))
		}

		whereClauses = append(whereClauses, "("+strings.Join(subClauses, " OR ")+")")
	}

	var turnusClauses []string
	turnusList := f.SemesterTurnuses
	if len(turnusList) == 0 && f.SemesterTurnus != "" && f.SemesterTurnus != "all" {
		turnusList = []string{f.SemesterTurnus}
	}

	for _, t := range turnusList {
		t = strings.ToLower(strings.TrimSpace(t))
		switch t {
		case "next":
			now := time.Now()
			month := now.Month()
			year := now.Year()
			// Switch in middle of semester: Months 1-6 -> SoSe, Months 7-12 -> WiSe
			if month >= 1 && month <= 6 {
				if year%2 == 0 {
					turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes sommersemester' OR LOWER(m.turnus) = 'every summer semester' OR LOWER(m.turnus) LIKE '%sommer%gerad%' OR LOWER(m.turnus) LIKE '%summer%even%')")
				} else {
					turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes sommersemester' OR LOWER(m.turnus) = 'every summer semester' OR LOWER(m.turnus) LIKE '%sommer%ungerad%' OR LOWER(m.turnus) LIKE '%summer%odd%')")
				}
			} else {
				if year%2 == 0 {
					turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes wintersemester' OR LOWER(m.turnus) = 'every winter semester' OR LOWER(m.turnus) LIKE '%winter%gerad%' OR LOWER(m.turnus) LIKE '%winter%even%')")
				} else {
					turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes wintersemester' OR LOWER(m.turnus) = 'every winter semester' OR LOWER(m.turnus) LIKE '%winter%ungerad%' OR LOWER(m.turnus) LIKE '%winter%odd%')")
				}
			}
		case "wise":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%winter%' OR LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%')")
		case "wise_even":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes wintersemester' OR LOWER(m.turnus) = 'every winter semester' OR LOWER(m.turnus) LIKE '%winter%gerad%' OR LOWER(m.turnus) LIKE '%winter%even%')")
		case "wise_odd":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes wintersemester' OR LOWER(m.turnus) = 'every winter semester' OR LOWER(m.turnus) LIKE '%winter%ungerad%' OR LOWER(m.turnus) LIKE '%winter%odd%')")
		case "sose":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%sommer%' OR LOWER(m.turnus) LIKE '%summer%' OR LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%')")
		case "sose_even":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes sommersemester' OR LOWER(m.turnus) = 'every summer semester' OR LOWER(m.turnus) LIKE '%sommer%gerad%' OR LOWER(m.turnus) LIKE '%summer%even%')")
		case "sose_odd":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%jedes semester%' OR LOWER(m.turnus) LIKE '%every semester%' OR LOWER(m.turnus) = 'jedes sommersemester' OR LOWER(m.turnus) = 'every summer semester' OR LOWER(m.turnus) LIKE '%sommer%ungerad%' OR LOWER(m.turnus) LIKE '%summer%odd%')")
		case "sporadic", "sporadisch":
			turnusClauses = append(turnusClauses, "(LOWER(m.turnus) LIKE '%sporadisch%' OR LOWER(m.turnus) LIKE '%unregelm%' OR LOWER(m.turnus) LIKE '%ankündigung%' OR LOWER(m.turnus) LIKE '%announcement%' OR LOWER(m.turnus) LIKE '%nach bedarf%' OR LOWER(m.turnus) LIKE '%irregular%' OR LOWER(m.turnus) LIKE '%on demand%')")
		}
	}

	if len(turnusClauses) > 0 {
		whereClauses = append(whereClauses, "("+strings.Join(turnusClauses, " OR ")+")")
	}

	switch strings.ToLower(strings.TrimSpace(f.Limitation)) {
	case "nein", "unlimited":
		whereClauses = append(whereClauses, "(m.limitation IS NULL OR m.limitation = '' OR LOWER(m.limitation) = 'keine')")
	case "nur", "limited":
		whereClauses = append(whereClauses, "(m.limitation IS NOT NULL AND m.limitation != '' AND LOWER(m.limitation) != 'keine')")
	}

	switch f.FUESFilter {
	case "nur":
		whereClauses = append(whereClauses, "(m.is_fues = 1 OR m.cross_disciplinary = 1)")
	case "exkl":
		whereClauses = append(whereClauses, "m.is_fues = 0 AND m.cross_disciplinary = 0")
	}

	if len(f.Instructors) > 0 {
		var instClauses []string
		for _, inst := range f.Instructors {
			inst = strings.TrimSpace(inst)
			if inst != "" {
				instClauses = append(instClauses, "LOWER(m.responsible_persons) LIKE ?")
				args = append(args, "%"+strings.ToLower(inst)+"%")
			}
		}
		if len(instClauses) > 0 {
			whereClauses = append(whereClauses, "("+strings.Join(instClauses, " OR ")+")")
		}
	}

	if f.ExcludePhaseOut {
		whereClauses = append(whereClauses, "m.is_phase_out = 0")
	}

	if f.NonAdjacentMajor != "" {
		whereClauses = append(whereClauses, "m.id NOT IN (SELECT module_id FROM module_study_programs WHERE LOWER(program_name) LIKE ?)")
		args = append(args, "%"+strings.ToLower(f.NonAdjacentMajor)+"%")
	}

	if len(f.Campuses) > 0 {
		var selectedConds []string
		for _, c := range f.Campuses {
			cond := campusRoomCondition(c)
			if cond != "" {
				selectedConds = append(selectedConds, cond)
			}
		}
		if len(selectedConds) > 0 {
			whereClauses = append(whereClauses, fmt.Sprintf("m.id IN (SELECT me.module_id FROM module_events me JOIN event_schedules es ON es.event_id = me.event_id WHERE %s)", strings.Join(selectedConds, " OR ")))
		}

		if f.CampusStrict {
			allKnown := []string{"hauptcampus", "sachsendorf", "senftenberg"}
			var unselectedConds []string
			for _, k := range allKnown {
				isSelected := false
				for _, c := range f.Campuses {
					cLow := strings.ToLower(strings.TrimSpace(c))
					if cLow == k || (k == "hauptcampus" && cLow == "zentralcampus") {
						isSelected = true
						break
					}
				}
				if !isSelected {
					unselectedConds = append(unselectedConds, campusRoomCondition(k))
				}
			}
			if len(unselectedConds) > 0 {
				whereClauses = append(whereClauses, fmt.Sprintf("m.id NOT IN (SELECT me.module_id FROM module_events me JOIN event_schedules es ON es.event_id = me.event_id WHERE %s)", strings.Join(unselectedConds, " OR ")))
			}
		}
	}

	if len(f.Languages) > 0 {
		hasDE := false
		hasEN := false
		for _, l := range f.Languages {
			low := strings.ToLower(strings.TrimSpace(l))
			if strings.Contains(low, "de") {
				hasDE = true
			}
			if strings.Contains(low, "en") {
				hasEN = true
			}
		}
		if hasDE && !hasEN {
			whereClauses = append(whereClauses, "LOWER(m.language) LIKE '%deutsch%'")
		} else if hasEN && !hasDE {
			whereClauses = append(whereClauses, "(LOWER(m.language) LIKE '%englisch%' OR LOWER(m.language) LIKE '%english%')")
		} else if hasDE && hasEN {
			whereClauses = append(whereClauses, "(LOWER(m.language) LIKE '%deutsch%' OR LOWER(m.language) LIKE '%english%' OR LOWER(m.language) LIKE '%englisch%')")
		}
	} else if f.Language != "" && !strings.EqualFold(f.Language, "all") {
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

	joinSQL = strings.Join(joins, " ")
	whereSQL = ""
	if len(whereClauses) > 0 {
		whereSQL = "WHERE " + strings.Join(whereClauses, " AND ")
	}
	return joinSQL, whereSQL, args
}

// SearchModuleIDs returns distinct module IDs matching the filter without pagination.
func (s *Storage) SearchModuleIDs(f AdvancedFilter) ([]string, error) {
	joinSQL, whereSQL, args := s.buildFilterClauses(f)
	query := fmt.Sprintf("SELECT DISTINCT m.id FROM modules m %s %s ORDER BY CAST(m.id AS INTEGER), m.id ASC", joinSQL, whereSQL)
	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err == nil {
			ids = append(ids, id)
		}
	}
	return ids, nil
}

// AutocompleteItem represents a lightweight module for live fuzzy autocomplete search.
type AutocompleteItem struct {
	ID      string  `json:"id"`
	TitleDE string  `json:"title_de"`
	TitleEN string  `json:"title_en,omitempty"`
	Credits float64 `json:"credits"`
	Turnus  string  `json:"turnus,omitempty"`
	IsFUES  bool    `json:"is_fues,omitempty"`
	IsPhase bool    `json:"is_phase,omitempty"`
}

// GetAllAutocompleteModules returns lightweight representations of all catalog modules for fuzzy search.
func (s *Storage) GetAllAutocompleteModules() ([]AutocompleteItem, error) {
	query := `SELECT id, title_de, title_en, credits, turnus, is_fues, cross_disciplinary, is_phase_out
	          FROM modules
	          ORDER BY CAST(id AS INTEGER), id ASC`
	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var items []AutocompleteItem
	for rows.Next() {
		var (
			it AutocompleteItem
			titleEN, turnus sql.NullString
			isFues, crossDisc, phaseOut int
		)
		if err := rows.Scan(&it.ID, &it.TitleDE, &titleEN, &it.Credits, &turnus, &isFues, &crossDisc, &phaseOut); err != nil {
			continue
		}
		if titleEN.Valid {
			it.TitleEN = titleEN.String
		}
		if turnus.Valid {
			it.Turnus = turnus.String
		}
		it.IsFUES = isFues == 1 || crossDisc == 1
		it.IsPhase = phaseOut == 1
		items = append(items, it)
	}
	return items, nil
}

// SearchModulesAdvanced executes an advanced query with smart filters and pagination.
func (s *Storage) SearchModulesAdvanced(f AdvancedFilter) ([]ModuleCardItem, int, error) {
	joinSQL, whereSQL, args := s.buildFilterClauses(f)

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
			m.id, m.code, m.title_de, m.title_en, m.is_phase_out, m.is_not_offered, m.department,
			m.responsible_persons, m.successor_modules, m.language, m.turnus, m.credits, m.credits_raw,
			m.prerequisites_recommended, m.prerequisites_mandatory, m.exam_type,
			m.is_fues, m.current_semester_events, m.raw_url, m.limitation
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
			isPhaseOutInt, isNotOfferedInt, isFuesInt             int
			titleEN, dept, lang, turnus, credRaw                  sql.NullString
			prereqRec, prereqMand, examType, urlStr               sql.NullString
			respJSON, succJSON, eventsJSON, limitationStr         sql.NullString
		)
		if err := rows.Scan(
			&item.ID, &item.Code, &item.TitleDE, &titleEN, &isPhaseOutInt, &isNotOfferedInt, &dept,
			&respJSON, &succJSON, &lang, &turnus, &item.Credits, &credRaw,
			&prereqRec, &prereqMand, &examType,
			&isFuesInt, &eventsJSON, &urlStr, &limitationStr,
		); err != nil {
			return nil, 0, err
		}

		item.TitleEN = titleEN.String
		item.IsPhaseOut = isPhaseOutInt == 1
		item.IsNotOffered = isNotOfferedInt == 1
		item.IsFUES = isFuesInt == 1
		item.Department = dept.String
		item.Language = lang.String
		item.Turnus = turnus.String
		item.Limitation = limitationStr.String
		item.CreditsRaw = credRaw.String
		item.PrerequisitesRecommended = prereqRec.String
		item.PrerequisitesMandatory = prereqMand.String
		item.ExamType = examType.String
		item.RawURL = urlStr.String

		if respJSON.Valid && respJSON.String != "" {
			_ = json.Unmarshal([]byte(respJSON.String), &item.ResponsiblePersons)
		}
		if succJSON.Valid && succJSON.String != "" {
			_ = json.Unmarshal([]byte(succJSON.String), &item.SuccessorModules)
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
			(SELECT COUNT(DISTINCT msp.module_id) FROM module_study_programs msp WHERE msp.program_id = p.id AND msp.degree != 'Abschluss im Ausland') as mod_count
		FROM official_study_programs p
		ORDER BY p.program_name ASC, p.degree ASC, p.po_version DESC
	`
	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var options []StudyProgramOption
	seenTitles := make(map[string]map[string]bool)

	for rows.Next() {
		var opt StudyProgramOption
		if err := rows.Scan(&opt.ID, &opt.ProgramName, &opt.Degree, &opt.POVersion, &opt.Count); err != nil {
			return nil, err
		}
		opt.ShortTitle = FormatProgramShort(opt.ProgramName, opt.Degree)
		if seenTitles[opt.ShortTitle] == nil {
			seenTitles[opt.ShortTitle] = make(map[string]bool)
		}
		if opt.POVersion != "" {
			seenTitles[opt.ShortTitle][opt.POVersion] = true
		}
		options = append(options, opt)
	}

	// Disambiguate if multiple distinct PO versions exist for the same ShortTitle
	for i := range options {
		if len(seenTitles[options[i].ShortTitle]) > 1 && options[i].POVersion != "" {
			options[i].ShortTitle = fmt.Sprintf("%s - PO %s", options[i].ShortTitle, options[i].POVersion)
		}
	}

	return options, rows.Err()
}

// campusRoomCondition returns a SQL expression matching rooms belonging to a campus.
func campusRoomCondition(campus string) string {
	switch strings.ToLower(strings.TrimSpace(campus)) {
	case "hauptcampus", "zentralcampus":
		return "(LOWER(es.room) LIKE '%zentralcampus%' OR LOWER(es.room) LIKE '%hauptcampus%' OR LOWER(es.room) LIKE '%hauptgebäude%' OR LOWER(es.room) LIKE '%lehrgebäude%' OR LOWER(es.room) LIKE '%audimax%' OR LOWER(es.room) LIKE '%großer hörsaal%')"
	case "sachsendorf":
		return "(LOWER(es.room) LIKE '%sachsendorf%')"
	case "senftenberg":
		return "(LOWER(es.room) LIKE '%senftenberg%' OR LOWER(es.room) LIKE '%sfb%')"
	default:
		return fmt.Sprintf("(LOWER(es.room) LIKE '%%%s%%')", strings.ToLower(campus))
	}
}

// StudyProgramGroup groups a study program with all its PO versions.
type StudyProgramGroup struct {
	Key         string               `json:"key"`          // e.g. "Informatik (Bachelor)"
	ProgramName string               `json:"program_name"` // e.g. "Informatik"
	Degree      string               `json:"degree"`       // e.g. "Bachelor (universitär)"
	ShortTitle  string               `json:"short_title"`  // e.g. "Informatik (Bachelor)"
	TotalCount  int                  `json:"total_count"`  // total module count in latest/primary PO
	POs         []StudyProgramOption `json:"pos"`
}

// GetGroupedStudyPrograms returns study programs grouped by base title with deduplicated PO versions.
func (s *Storage) GetGroupedStudyPrograms() ([]StudyProgramGroup, error) {
	query := `
		SELECT 
			p.id, p.program_name, p.degree, p.po_version
		FROM official_study_programs p
		ORDER BY p.program_name ASC, p.degree ASC, p.po_version DESC
	`
	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	groupMap := make(map[string]*StudyProgramGroup)
	var groupOrder []string
	poIndexMap := make(map[string]map[string]int)

	for rows.Next() {
		var opt StudyProgramOption
		if err := rows.Scan(&opt.ID, &opt.ProgramName, &opt.Degree, &opt.POVersion); err != nil {
			return nil, err
		}
		opt.ShortTitle = FormatProgramShort(opt.ProgramName, opt.Degree)
		key := opt.ShortTitle

		grp, exists := groupMap[key]
		if !exists {
			grp = &StudyProgramGroup{
				Key:         key,
				ProgramName: opt.ProgramName,
				Degree:      opt.Degree,
				ShortTitle:  opt.ShortTitle,
				POs:         []StudyProgramOption{},
			}
			groupMap[key] = grp
			groupOrder = append(groupOrder, key)
			poIndexMap[key] = make(map[string]int)
		}

		if idx, poExists := poIndexMap[key][opt.POVersion]; poExists {
			// Merge duplicate PO entry
			existing := &grp.POs[idx]
			existing.RelatedIDs = append(existing.RelatedIDs, opt.ID)
			// Prefer non-variant degree as canonical ID and degree
			if isVariantDegree(existing.Degree) && !isVariantDegree(opt.Degree) {
				existing.ID = opt.ID
				existing.Degree = opt.Degree
			}
		} else {
			opt.RelatedIDs = []string{opt.ID}
			poIndexMap[key][opt.POVersion] = len(grp.POs)
			grp.POs = append(grp.POs, opt)
		}
	}

	// Calculate distinct module counts for each deduplicated PO
	for _, grp := range groupMap {
		for i := range grp.POs {
			po := &grp.POs[i]
			placeholders := make([]string, len(po.RelatedIDs))
			args := make([]interface{}, len(po.RelatedIDs))
			for j, id := range po.RelatedIDs {
				placeholders[j] = "?"
				args[j] = id
			}
			q := fmt.Sprintf(`
				SELECT COUNT(DISTINCT module_id) 
				FROM module_study_programs 
				WHERE program_id IN (%s) AND degree != 'Abschluss im Ausland'
			`, strings.Join(placeholders, ","))
			var count int
			_ = s.db.QueryRow(q, args...).Scan(&count)
			po.Count = count
			if count > grp.TotalCount {
				grp.TotalCount = count
			}
		}
	}

	result := make([]StudyProgramGroup, 0, len(groupOrder))
	for _, k := range groupOrder {
		result = append(result, *groupMap[k])
	}
	return result, rows.Err()
}

// GetRelatedProgramIDs returns all official study program IDs that share the same program name,
// simplified degree (e.g. Master), and PO version.
func (s *Storage) GetRelatedProgramIDs(programID string) []string {
	if programID == "" {
		return nil
	}
	var progName, degree, poVersion string
	err := s.db.QueryRow("SELECT program_name, degree, po_version FROM official_study_programs WHERE id = ?", programID).Scan(&progName, &degree, &poVersion)
	if err != nil {
		return []string{programID}
	}
	shortDeg := FormatDegreeShort(degree)
	rows, err := s.db.Query(`
		SELECT id, degree FROM official_study_programs 
		WHERE program_name = ? AND po_version = ?
	`, progName, poVersion)
	if err != nil {
		return []string{programID}
	}
	defer rows.Close()

	var ids []string
	for rows.Next() {
		var id, d string
		if err := rows.Scan(&id, &d); err == nil {
			if FormatDegreeShort(d) == shortDeg {
				ids = append(ids, id)
			}
		}
	}
	if len(ids) == 0 {
		return []string{programID}
	}
	return ids
}

// GetProgramTotalModules returns the total count of modules assigned to a program, or total in catalog if programID is empty.
func (s *Storage) GetProgramTotalModules(programID string) (int, error) {
	if programID == "" {
		var total int
		err := s.db.QueryRow("SELECT COUNT(*) FROM modules").Scan(&total)
		return total, err
	}
	relatedIDs := s.GetRelatedProgramIDs(programID)
	placeholders := make([]string, len(relatedIDs))
	args := make([]interface{}, len(relatedIDs))
	for i, id := range relatedIDs {
		placeholders[i] = "?"
		args[i] = id
	}
	q := fmt.Sprintf(`
		SELECT COUNT(DISTINCT module_id) 
		FROM module_study_programs 
		WHERE program_id IN (%s) AND degree != 'Abschluss im Ausland'
	`, strings.Join(placeholders, ","))
	var total int
	err := s.db.QueryRow(q, args...).Scan(&total)
	return total, err
}

// FreshnessStats aggregates data freshness and queue discovery information.
type FreshnessStats struct {
	ModulesTotal          int `json:"modules_total"`
	ModulesFresh          int `json:"modules_fresh"`          // Scraped within last 7 days
	ModulesStale          int `json:"modules_stale"`          // Scraped > 7 days ago
	ModulesUnscraped      int `json:"modules_unscraped"`      // Missing detailed contents
	ModulesFreshPct       int `json:"modules_fresh_pct"`
	EventsTotalDiscovered int `json:"events_total_discovered"` // Unique events referenced by modules
	EventsScraped         int `json:"events_scraped"`         // Stored in events table
	EventsFresh           int `json:"events_fresh"`           // Scraped within last 7 days
	EventsStale           int `json:"events_stale"`           // Scraped > 7 days ago
	EventsPending         int `json:"events_pending"`         // Discovered but not yet scraped into events table
	EventsFreshPct        int `json:"events_fresh_pct"`
	SchedulesTotal        int `json:"schedules_total"`
	ProgramsTotal         int `json:"programs_total"`
}

// DiscoveredEventRef represents an event reference discovered from a module's current semester events.
type DiscoveredEventRef struct {
	EventID   string `json:"event_id"`
	Title     string `json:"title"`
	URL       string `json:"url"`
	ModuleID  string `json:"module_id"`
	IsScraped bool   `json:"is_scraped"`
}

// GetDiscoveredEventRefs scans all module current_semester_events and returns unique discovered events.
func (s *Storage) GetDiscoveredEventRefs() ([]DiscoveredEventRef, error) {
	rows, err := s.db.Query(`
		SELECT id, current_semester_events
		FROM modules
		WHERE current_semester_events IS NOT NULL
		  AND current_semester_events != ''
		  AND current_semester_events != '[]'
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	// Get set of already scraped event IDs
	scrapedRows, err := s.db.Query("SELECT id FROM events")
	scrapedMap := make(map[string]bool)
	if err == nil {
		defer scrapedRows.Close()
		for scrapedRows.Next() {
			var eid string
			if err := scrapedRows.Scan(&eid); err == nil {
				scrapedMap[eid] = true
			}
		}
	}

	seenEvent := make(map[string]bool)
	var result []DiscoveredEventRef

	for rows.Next() {
		var modID string
		var eventsJSON string
		if err := rows.Scan(&modID, &eventsJSON); err != nil {
			continue
		}

		var events []model.ModuleEvent
		if err := json.Unmarshal([]byte(eventsJSON), &events); err != nil {
			continue
		}

		for _, evt := range events {
			evtID := parseEventIDFromURL(evt.URL)
			key := evtID
			if key == "" {
				key = evt.URL
			}
			if key == "" || seenEvent[key] {
				continue
			}
			seenEvent[key] = true

			result = append(result, DiscoveredEventRef{
				EventID:   evtID,
				Title:     evt.Title,
				URL:       evt.URL,
				ModuleID:  modID,
				IsScraped: scrapedMap[evtID],
			})
		}
	}

	return result, nil
}

// GetFreshnessStats returns aggregated metrics on data freshness and queue discovery status.
func (s *Storage) GetFreshnessStats() (FreshnessStats, error) {
	var st FreshnessStats

	// 1. Module counts
	_ = s.db.QueryRow("SELECT COUNT(*) FROM modules").Scan(&st.ModulesTotal)
	_ = s.db.QueryRow(`
		SELECT COUNT(*) FROM modules 
		WHERE last_scraped_at >= datetime('now', '-7 days')
		  AND contents IS NOT NULL AND contents != ''
	`).Scan(&st.ModulesFresh)
	_ = s.db.QueryRow(`
		SELECT COUNT(*) FROM modules 
		WHERE last_scraped_at < datetime('now', '-7 days')
		  AND contents IS NOT NULL AND contents != ''
	`).Scan(&st.ModulesStale)
	_ = s.db.QueryRow(`
		SELECT COUNT(*) FROM modules 
		WHERE contents IS NULL OR contents = ''
	`).Scan(&st.ModulesUnscraped)

	if st.ModulesTotal > 0 {
		st.ModulesFreshPct = int(float64(st.ModulesFresh) / float64(st.ModulesTotal) * 100)
	}

	// 2. Events & Schedules counts
	_ = s.db.QueryRow("SELECT COUNT(*) FROM events").Scan(&st.EventsScraped)
	_ = s.db.QueryRow(`
		SELECT COUNT(*) FROM events 
		WHERE last_scraped_at >= datetime('now', '-7 days')
	`).Scan(&st.EventsFresh)
	_ = s.db.QueryRow(`
		SELECT COUNT(*) FROM events 
		WHERE last_scraped_at < datetime('now', '-7 days')
	`).Scan(&st.EventsStale)
	_ = s.db.QueryRow("SELECT COUNT(*) FROM event_schedules").Scan(&st.SchedulesTotal)

	// 3. Discovered events from modules
	discovered, _ := s.GetDiscoveredEventRefs()
	st.EventsTotalDiscovered = len(discovered)
	pending := 0
	for _, d := range discovered {
		if !d.IsScraped {
			pending++
		}
	}
	st.EventsPending = pending

	if st.EventsTotalDiscovered > 0 {
		st.EventsFreshPct = int(float64(st.EventsScraped) / float64(st.EventsTotalDiscovered) * 100)
	}

	// 4. Programs count
	_ = s.db.QueryRow("SELECT COUNT(*) FROM official_study_programs").Scan(&st.ProgramsTotal)

	return st, nil
}

func parseEventIDFromURL(rawURL string) string {
	if rawURL == "" {
		return ""
	}
	if idx := strings.Index(rawURL, "veranstid="); idx != -1 {
		sub := rawURL[idx+len("veranstid="):]
		if amp := strings.Index(sub, "&"); amp != -1 {
			return sub[:amp]
		}
		return sub
	}
	return ""
}

// InstructorItem represents a distinct course instructor or coordinator.
type InstructorItem struct {
	Name     string `json:"name"`     // e.g. "Köhler, Ekkehard"
	Title    string `json:"title"`    // e.g. "Prof. Dr. rer. nat. habil."
	FullName string `json:"fullname"` // e.g. "Prof. Dr. rer. nat. habil. Köhler, Ekkehard"
}

// GetAllInstructors returns all unique instructors across all modules, sorted alphabetically by name.
func (s *Storage) GetAllInstructors() ([]InstructorItem, error) {
	rows, err := s.db.Query(`SELECT DISTINCT responsible_persons FROM modules WHERE responsible_persons IS NOT NULL AND responsible_persons != '' AND responsible_persons != '[]'`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	seen := make(map[string]InstructorItem)
	for rows.Next() {
		var rawJSON string
		if err := rows.Scan(&rawJSON); err == nil {
			var persons []model.ResponsiblePerson
			if err := json.Unmarshal([]byte(rawJSON), &persons); err == nil {
				for _, p := range persons {
					name := strings.TrimSpace(p.Name)
					if name == "" {
						name = strings.TrimSpace(p.Raw)
					}
					if name != "" {
						if existing, ok := seen[name]; !ok || (p.Title != "" && existing.Title == "") {
							seen[name] = InstructorItem{
								Name:     name,
								Title:    strings.TrimSpace(p.Title),
								FullName: strings.TrimSpace(p.FullName()),
							}
						}
					}
				}
			}
		}
	}

	result := make([]InstructorItem, 0, len(seen))
	for _, item := range seen {
		result = append(result, item)
	}
	sort.Slice(result, func(i, j int) bool {
		return strings.ToLower(result[i].Name) < strings.ToLower(result[j].Name)
	})
	return result, nil
}

