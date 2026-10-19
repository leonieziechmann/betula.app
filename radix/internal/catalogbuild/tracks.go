package catalogbuild

import (
	"database/sql"
	"sort"
)

// trackShare is how much of a module's SWS the events at one campus must hold to be a course of
// their own: the Programmierpraktikum's 3 SWS (1 lecture, 2 lab) are held whole at the Zentralcampus
// and whole in Sachsendorf, while Betriebssysteme und Rechnernetze (11760) holds 2 of its 4 SWS in
// Sachsendorf and the other 2 at the Zentralcampus, both required.
const trackShare = 0.75

// trackEvent is a teaching event of a module in one semester, as the tracks are read from it.
type trackEvent struct {
	id      string
	campus  string  // the one campus of its dates with a room; "" when they are at several, or at none
	sws     float64 // what its page states; 0 when it states nothing
	lecture bool
}

// moduleSemester is a module's teaching in one semester.
type moduleSemester struct {
	module, semester string
}

// querier is what the steps of the tracks read and write through: the build's transaction, or
// a snapshot in a test (TestTracksOnSnapshot).
type querier interface {
	Query(query string, args ...any) (*sql.Rows, error)
	Exec(query string, args ...any) (sql.Result, error)
}

// writeTracks finds the modules taught as two whole courses at two campuses, of which a student
// attends one (docs/radix/schema-v2.md, „Tracks"): the courses (module_track), where each
// program is taught (program_campus), and which course each program takes (module_track_program).
func (b *builder) writeTracks() error {
	return writeTracks(b.tx, b.programs, b.report)
}

func writeTracks(q querier, programs []*program, report *Report) error {
	events, err := teachingEvents(q)
	if err != nil {
		return err
	}
	sws, lecture, err := moduleSWS(q)
	if err != nil {
		return err
	}

	keys := make([]moduleSemester, 0, len(events))
	for key := range events {
		keys = append(keys, key)
	}
	sort.Slice(keys, func(i, j int) bool {
		if keys[i].module != keys[j].module {
			return keys[i].module < keys[j].module
		}
		return keys[i].semester < keys[j].semester
	})

	tracks := make(map[moduleSemester]map[string][]string)
	for _, key := range keys {
		courses := tracksOf(events[key], sws[key.module], lecture[key.module])
		if courses == nil {
			continue
		}
		tracks[key] = courses
		report.ModulesWithTracks++
		for _, campus := range sortedKeys(courses) {
			for _, event := range courses[campus] {
				if _, err := q.Exec("INSERT INTO module_track (module_id, event_id, semester_key, campus) VALUES (?, ?, ?, ?)",
					key.module, event, key.semester, campus); err != nil {
					return err
				}
			}
		}
	}

	campuses, err := writeProgramCampus(q, programs, tracks, report)
	if err != nil {
		return err
	}
	return writeTrackPrograms(q, programs, keys, tracks, campuses, report)
}

// tracksOf reads the courses of a module's teaching in one semester: the events at each campus,
// where at least two campuses each hold a course of their own — at least trackShare of the
// module's SWS, and a lecture where the module has lectures. nil when there are no two: a module
// whose parts are held at two campuses, all of them required, has none, and neither has one
// whose SWS are unknown (QIS's own numbers decide, not a guess). An event at several campuses or
// at none is in no course; it stays everybody's.
func tracksOf(events []trackEvent, sws, lectureSWS float64) map[string][]string {
	if sws <= 0 {
		return nil
	}
	byCampus := make(map[string][]trackEvent)
	for _, e := range events {
		if e.campus != "" {
			byCampus[e.campus] = append(byCampus[e.campus], e)
		}
	}
	courses := make(map[string][]string)
	for campus, held := range byCampus {
		var sum float64
		var lecture bool
		for _, e := range held {
			sum += e.sws
			lecture = lecture || e.lecture
		}
		if sum < trackShare*sws || (lectureSWS > 0 && !lecture) {
			continue
		}
		for _, e := range held {
			courses[campus] = append(courses[campus], e.id)
		}
	}
	if len(courses) < 2 {
		return nil
	}
	return courses
}

// teachingEvents are the teaching events of every module per semester, with the campus of their
// dates.
func teachingEvents(q querier) (map[moduleSemester][]trackEvent, error) {
	rows, err := q.Query(`
		SELECT me.module_id, e.semester_key, e.id, IFNULL(e.sws, 0),
		       EXISTS (SELECT 1 FROM event_form f WHERE f.event_id = e.id AND f.form = 'lecture'),
		       (SELECT COUNT(DISTINCT d.campus) FROM event_date d WHERE d.event_id = e.id AND d.campus IS NOT NULL),
		       (SELECT MIN(d.campus) FROM event_date d WHERE d.event_id = e.id AND d.campus IS NOT NULL)
		FROM module_event me
		JOIN event e ON e.id = me.event_id AND e.category = 'teaching'
		WHERE e.semester_key IS NOT NULL
		ORDER BY me.module_id, e.semester_key, e.id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	events := make(map[moduleSemester][]trackEvent)
	for rows.Next() {
		var key moduleSemester
		var e trackEvent
		var count int
		var campus sql.NullString
		if err := rows.Scan(&key.module, &key.semester, &e.id, &e.sws, &e.lecture, &count, &campus); err != nil {
			return nil, err
		}
		if count == 1 {
			e.campus = campus.String
		}
		events[key] = append(events[key], e)
	}
	return events, rows.Err()
}

// moduleSWS is every module's SWS, and those of its lectures, as its description states them.
func moduleSWS(q querier) (sws, lecture map[string]float64, err error) {
	rows, err := q.Query(`
		SELECT module_id, SUM(sws), SUM(CASE WHEN form = 'lecture' THEN sws ELSE 0 END)
		FROM module_teaching_form WHERE sws > 0 GROUP BY module_id`)
	if err != nil {
		return nil, nil, err
	}
	defer rows.Close()
	sws, lecture = make(map[string]float64), make(map[string]float64)
	for rows.Next() {
		var id string
		var all, lectures float64
		if err := rows.Scan(&id, &all, &lectures); err != nil {
			return nil, nil, err
		}
		sws[id], lecture[id] = all, lectures
	}
	return sws, lecture, rows.Err()
}

// writeProgramCampus says where each program is taught (owner, 2026-10-19: „den Standort der
// Studiengangs erkennen, anhand der Pflicht module im regelstudienplan"): per campus, the
// compulsory modules of its validated plan held there — of its other statements where it has no
// plan with one —, each module in the newest semester in which it has dates with a room. A module
// with tracks in that semester counts nowhere: where it is taken is what is to be decided. The
// main campus holds more of them than any other. Returns program → campus → modules.
func writeProgramCampus(q querier, programs []*program, tracks map[moduleSemester]map[string][]string, report *Report) (map[string]map[string]int, error) {
	compulsory := make(map[string][]string) // program → modules
	planned := make(map[string]bool)
	rows, err := q.Query(`
		SELECT DISTINCT pe.program_id, pe.module_id FROM plan_entry pe
		JOIN program p ON p.id = pe.program_id
		JOIN module m ON m.id = pe.module_id
		WHERE pe.kind = 'compulsory'
		ORDER BY 1, 2`)
	if err != nil {
		return nil, err
	}
	for rows.Next() {
		var program, module string
		if err := rows.Scan(&program, &module); err != nil {
			rows.Close()
			return nil, err
		}
		compulsory[program] = append(compulsory[program], module)
		planned[program] = true
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, err
	}
	rows, err = q.Query(`
		SELECT program_id, module_id FROM program_module
		WHERE kind = 'compulsory' AND relation = 'curricular'
		ORDER BY 1, 2`)
	if err != nil {
		return nil, err
	}
	for rows.Next() {
		var program, module string
		if err := rows.Scan(&program, &module); err != nil {
			rows.Close()
			return nil, err
		}
		if !planned[program] {
			compulsory[program] = append(compulsory[program], module)
		}
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, err
	}

	// The campuses of every module in the newest semester in which it has dates with a room.
	type held struct {
		semester string
		campuses map[string]bool
	}
	at := make(map[string]*held)
	rows, err = q.Query(`
		SELECT me.module_id, e.semester_key, d.campus
		FROM module_event me
		JOIN event e ON e.id = me.event_id AND e.category <> 'exam'
		JOIN event_date d ON d.event_id = e.id
		WHERE d.campus IS NOT NULL AND e.semester_key IS NOT NULL
		GROUP BY me.module_id, e.semester_key, d.campus`)
	if err != nil {
		return nil, err
	}
	for rows.Next() {
		var module, semester, campus string
		if err := rows.Scan(&module, &semester, &campus); err != nil {
			rows.Close()
			return nil, err
		}
		switch h := at[module]; {
		case h == nil || semester > h.semester:
			at[module] = &held{semester: semester, campuses: map[string]bool{campus: true}}
		case semester == h.semester:
			h.campuses[campus] = true
		}
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, err
	}

	result := make(map[string]map[string]int)
	for _, p := range programs {
		counts := make(map[string]int)
		for _, module := range compulsory[p.id] {
			h := at[module]
			if h == nil || tracks[moduleSemester{module, h.semester}] != nil {
				continue
			}
			for campus := range h.campuses {
				counts[campus]++
			}
		}
		if len(counts) == 0 {
			continue
		}
		result[p.id] = counts
		main := mostOf(counts, nil)
		if main != "" {
			report.ProgramsWithCampus++
		}
		for _, campus := range sortedKeys(counts) {
			if _, err := q.Exec("INSERT INTO program_campus (program_id, campus, modules, is_main) VALUES (?, ?, ?, ?)",
				p.id, campus, counts[campus], boolInt(campus == main)); err != nil {
				return nil, err
			}
		}
	}
	return result, nil
}

// writeTrackPrograms says which course of a module with tracks each program takes: the courses
// whose events QIS lists the program on, a program of the same family (any of its POs: a page
// lists „Informatik Bachelor" without one); else the course at the campus where the program has
// strictly more of its compulsory modules than at the other courses' campuses. Neither: no row,
// and a reader shows every course.
func writeTrackPrograms(q querier, programs []*program, keys []moduleSemester, tracks map[moduleSemester]map[string][]string, campuses map[string]map[string]int, report *Report) error {
	families := make(map[string][]string) // event → families QIS lists it for
	rows, err := q.Query(`
		SELECT DISTINCT r.event_id, p.family_key FROM event_program_ref r
		JOIN program p ON p.id = r.program_id
		ORDER BY 1, 2`)
	if err != nil {
		return err
	}
	for rows.Next() {
		var event, family string
		if err := rows.Scan(&event, &family); err != nil {
			rows.Close()
			return err
		}
		families[event] = append(families[event], family)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	insert := func(key moduleSemester, campus, program, basis string) error {
		_, err := q.Exec("INSERT INTO module_track_program (module_id, semester_key, campus, program_id, basis) VALUES (?, ?, ?, ?, ?)",
			key.module, key.semester, campus, program, basis)
		return err
	}
	for _, key := range keys {
		courses := tracks[key]
		if courses == nil {
			continue
		}
		listed := make(map[string]map[string]bool) // family → the courses QIS lists it on
		for campus, events := range courses {
			for _, event := range events {
				for _, family := range families[event] {
					if listed[family] == nil {
						listed[family] = make(map[string]bool)
					}
					listed[family][campus] = true
				}
			}
		}
		for _, p := range programs {
			if on := listed[p.familyKey]; len(on) > 0 {
				for _, campus := range sortedKeys(on) {
					if err := insert(key, campus, p.id, "qis"); err != nil {
						return err
					}
				}
				report.TrackProgramsByQIS++
				continue
			}
			if campus := mostOf(campuses[p.id], courses); campus != "" {
				if err := insert(key, campus, p.id, "campus"); err != nil {
					return err
				}
				report.TrackProgramsByCampus++
			}
		}
	}
	return nil
}

// mostOf is the campus with strictly the most modules, among those of `among` where it is given;
// "" for a tie or none.
func mostOf(counts map[string]int, among map[string][]string) string {
	best, most, tie := "", 0, false
	for _, campus := range sortedKeys(counts) {
		if among != nil {
			if _, ok := among[campus]; !ok {
				continue
			}
		}
		switch n := counts[campus]; {
		case n > most:
			best, most, tie = campus, n, false
		case n == most && n > 0:
			tie = true
		}
	}
	if tie {
		return ""
	}
	return best
}

func sortedKeys[T any](m map[string]T) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
