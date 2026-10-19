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
	audiences, err := eventAudiences(q)
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
		if sameAudience(courses, audiences) {
			report.TracksOneAudience++
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
	return writeTrackPrograms(q, programs, keys, tracks, audiences, campuses, report)
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

// sameAudience says whether QIS lists the same programs at every course: then the place binds no
// program to a course, and the courses are no choice of campus but topics to choose from — the
// Research Module in Artificial Intelligence (14060) holds its seminars at the Zentralcampus and in
// Sachsendorf, every one of them for AI M.Sc. — or parts of the module that the SWS did not tell
// apart. A course whose events QIS lists nobody at leaves it open: the SWS decide alone.
func sameAudience(courses map[string][]string, audiences map[string]map[string]bool) bool {
	var first map[string]bool
	for _, campus := range sortedKeys(courses) {
		who := make(map[string]bool)
		for _, event := range courses[campus] {
			for key := range audiences[event] {
				who[key] = true
			}
		}
		if len(who) == 0 {
			return false
		}
		if first == nil {
			first = who
			continue
		}
		if len(who) != len(first) {
			return false
		}
		for key := range who {
			if !first[key] {
				return false
			}
		}
	}
	return true
}

// eventAudiences is whom QIS lists each event for (event_program): the families of the programs a
// row names, and a row that names none of the catalog's by its label („label:" and the label).
func eventAudiences(q querier) (map[string]map[string]bool, error) {
	rows, err := q.Query(`
		SELECT ep.event_id, IFNULL(p.family_key, 'label:' || ep.label)
		FROM event_program ep
		LEFT JOIN event_program_ref r ON r.event_id = ep.event_id AND r.ord = ep.ord
		LEFT JOIN program p ON p.id = r.program_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	audiences := make(map[string]map[string]bool)
	for rows.Next() {
		var event, who string
		if err := rows.Scan(&event, &who); err != nil {
			return nil, err
		}
		if audiences[event] == nil {
			audiences[event] = make(map[string]bool)
		}
		audiences[event][who] = true
	}
	return audiences, rows.Err()
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

// campusEvidence is where a program is taught, per campus: the SWS of the teaching events QIS lists
// its family at, and its compulsory modules held there.
type campusEvidence struct {
	sws     map[string]float64
	modules map[string]int
}

// main is the program's campus among `among` (every campus where nil): where QIS lists its family
// for strictly the most SWS; where QIS decides nothing there, where strictly the most of its
// compulsory modules are held; "" where neither decides.
func (e campusEvidence) main(among map[string][]string) string {
	if campus := mostSWS(e.sws, among); campus != "" {
		return campus
	}
	return mostOf(e.modules, among)
}

// writeProgramCampus says where each program is taught: where QIS lists the events of its family
// (every PO: the students of an older one are taught where those of the newest are), weighted by
// their SWS, an event without SWS as one; and as the owner proposed (2026-10-19: „den Standort der
// Studiengangs erkennen, anhand der Pflicht module im regelstudienplan"), per campus the compulsory
// modules of its validated plan held there — of its other statements where it has no plan with
// one —, each module in the newest semester in which it has dates with a room. A module with
// tracks in that semester counts nowhere: where it is taken is what is to be decided. QIS decides
// first (owner, the same day: „Wenn du bessere Kriterien findest um das einzustufen dann kannst du
// auch die Verwenden"): it states for whom an event is held, where the plan only says what is
// compulsory. Returns program → its evidence.
func writeProgramCampus(q querier, programs []*program, tracks map[moduleSemester]map[string][]string, report *Report) (map[string]campusEvidence, error) {
	listed, err := familyCampusSWS(q)
	if err != nil {
		return nil, err
	}

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

	result := make(map[string]campusEvidence)
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
		evidence := campusEvidence{sws: listed[p.familyKey], modules: counts}
		campuses := make(map[string]bool)
		for campus := range evidence.sws {
			campuses[campus] = true
		}
		for campus := range counts {
			campuses[campus] = true
		}
		if len(campuses) == 0 {
			continue
		}
		result[p.id] = evidence
		main := evidence.main(nil)
		switch {
		case main == "":
		case main == mostSWS(evidence.sws, nil):
			report.ProgramsWithCampus++
			report.ProgramsCampusByQIS++
		default:
			report.ProgramsWithCampus++
		}
		for _, campus := range sortedKeys(campuses) {
			if _, err := q.Exec("INSERT INTO program_campus (program_id, campus, modules, sws, is_main) VALUES (?, ?, ?, ?, ?)",
				p.id, campus, counts[campus], evidence.sws[campus], boolInt(campus == main)); err != nil {
				return nil, err
			}
		}
	}
	return result, nil
}

// familyCampusSWS is where QIS lists each family's teaching events: family → campus → SWS, each
// event once per family and only at the one campus of its dates with a room, an event without SWS
// as one.
func familyCampusSWS(q querier) (map[string]map[string]float64, error) {
	rows, err := q.Query(`
		SELECT DISTINCT e.id, IFNULL(e.sws, 0), p.family_key,
		       (SELECT COUNT(DISTINCT d.campus) FROM event_date d WHERE d.event_id = e.id AND d.campus IS NOT NULL),
		       (SELECT MIN(d.campus) FROM event_date d WHERE d.event_id = e.id AND d.campus IS NOT NULL)
		FROM event_program_ref r
		JOIN program p ON p.id = r.program_id
		JOIN event e ON e.id = r.event_id AND e.category = 'teaching'`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	listed := make(map[string]map[string]float64)
	for rows.Next() {
		var event, family string
		var sws float64
		var count int
		var campus sql.NullString
		if err := rows.Scan(&event, &sws, &family, &count, &campus); err != nil {
			return nil, err
		}
		if count != 1 {
			continue
		}
		if sws <= 0 {
			sws = 1
		}
		if listed[family] == nil {
			listed[family] = make(map[string]float64)
		}
		listed[family][campus.String] += sws
	}
	return listed, rows.Err()
}

// writeTrackPrograms says which course of a module with tracks each program takes: the courses
// whose events QIS lists the program on, a program of the same family (any of its POs: a page
// lists „Informatik Bachelor" without one); else the course at the campus where the program is
// taught (campusEvidence.main among the courses' campuses). Neither: no row, and a reader shows
// every course.
func writeTrackPrograms(q querier, programs []*program, keys []moduleSemester, tracks map[moduleSemester]map[string][]string, audiences map[string]map[string]bool, campuses map[string]campusEvidence, report *Report) error {
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
				for family := range audiences[event] {
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
			if campus := campuses[p.id].main(courses); campus != "" {
				if err := insert(key, campus, p.id, "campus"); err != nil {
					return err
				}
				report.TrackProgramsByCampus++
			}
		}
	}
	return nil
}

// mostSWS is the campus with strictly the most SWS, among those of `among` where it is given; ""
// for a tie or none.
func mostSWS(sws map[string]float64, among map[string][]string) string {
	best, most, tie := "", 0.0, false
	for _, campus := range sortedKeys(sws) {
		if among != nil {
			if _, ok := among[campus]; !ok {
				continue
			}
		}
		switch n := sws[campus]; {
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
