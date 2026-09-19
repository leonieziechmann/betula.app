package catalogbuild

import (
	"fmt"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/normalize"
)

var (
	germanDate = regexp.MustCompile(`(\d{1,2})\.(\d{1,2})\.(\d{4})`)
	clockTime  = regexp.MustCompile(`^\d{1,2}:\d{2}`)
	firstFloat = regexp.MustCompile(`\d+(?:[.,]\d+)?`)
)

var weekdays = map[string]int{
	"mo": 1, "di": 2, "mi": 3, "do": 4, "fr": 5, "sa": 6, "so": 7,
	"mon": 1, "tue": 2, "wed": 3, "thu": 4, "fri": 5, "sat": 6, "sun": 7,
}

// writeEvents writes the archived QIS events and links them to the modules whose
// page lists them. Exams are a category of their own, so that consumers can keep
// them out of the recurring schedule and still show them.
func (b *builder) writeEvents() error {
	ids := make([]string, 0, len(b.src.events))
	for id := range b.src.events {
		ids = append(ids, id)
	}
	sort.Strings(ids)

	semesters := make(map[string]bool)
	for _, id := range ids {
		page := b.src.events[id]
		d := page.detail

		forms := normalize.TeachingForm(d.EventType)
		category := "other"
		switch low := strings.ToLower(d.EventType); {
		case strings.Contains(low, "prüfung") || strings.Contains(low, "klausur") || strings.Contains(low, "exam"):
			category, forms = "exam", nil
		case len(forms) > 0 && forms[0] != normalize.FormOther:
			category = "teaching"
		}

		semesterKey := normalize.SemesterKey(d.Semester)
		if semesterKey != "" && !semesters[semesterKey] {
			semesters[semesterKey] = true
			if err := b.insertSemester(semesterKey); err != nil {
				return err
			}
		}

		type dateRow struct {
			first, last string
		}
		dates := make([]dateRow, len(d.Schedules))
		var eventFirst, eventLast string
		for i, s := range d.Schedules {
			found := germanDate.FindAllStringSubmatch(s.Duration, -1)
			if len(found) > 0 {
				dates[i].first = isoDate(found[0])
				dates[i].last = isoDate(found[len(found)-1])
				if eventFirst == "" || dates[i].first < eventFirst {
					eventFirst = dates[i].first
				}
				if dates[i].last > eventLast {
					eventLast = dates[i].last
				}
			}
		}

		title := d.Title
		if title == "" {
			title = id
		}
		_, err := b.tx.Exec(`
			INSERT INTO event (id, number, title, type_raw, category, semester_key, sws, max_participants,
				first_date, last_date, source_url, fetched_at)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			id, null(d.EventNumber), title, null(d.EventType), category, null(semesterKey),
			null(parseFloat(d.SWS)), null(parseInt(d.MaxParticipants)),
			null(eventFirst), null(eventLast), page.url, page.fetchedAt.UTC().Format(time.RFC3339))
		if err != nil {
			return fmt.Errorf("event %s: %w", id, err)
		}
		b.report.Events++

		for _, form := range forms {
			if _, err := b.tx.Exec("INSERT OR IGNORE INTO event_form (event_id, form) VALUES (?, ?)", id, form); err != nil {
				return err
			}
		}
		ord := 0
		for _, p := range d.ResponsiblePersons {
			if strings.TrimSpace(p.Name) == "" {
				continue
			}
			ord++
			if _, err := b.tx.Exec("INSERT INTO event_person (event_id, ord, name, role) VALUES (?, ?, ?, ?)",
				id, ord, strings.TrimSpace(p.Name), null(p.Role)); err != nil {
				return err
			}
		}
		for i, s := range d.Schedules {
			_, err := b.tx.Exec(`
				INSERT INTO event_date (event_id, ord, group_name, weekday, start_time, end_time, rhythm, rhythm_raw,
					first_date, last_date, room, campus, instructor, comment, cancelled_dates)
				VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				id, i+1, null(s.GroupName), null(weekdays[strings.Trim(strings.ToLower(s.DayOfWeek), ". ")]),
				clock(s.StartTime), clock(s.EndTime), null(rhythm(s.Rhythm)), null(s.Rhythm),
				null(dates[i].first), null(dates[i].last), null(s.Room), null(normalize.Campus(s.Room)),
				null(s.Instructor), null(s.Comment), null(s.CancelledDates))
			if err != nil {
				return fmt.Errorf("event %s date %d: %w", id, i+1, err)
			}
		}
	}

	tombstones := make(map[string]bool)
	rows, err := b.tx.Query("SELECT event_id FROM event_tombstone")
	if err != nil {
		return err
	}
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			rows.Close()
			return err
		}
		tombstones[id] = true
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	// A module page decides which events belong to the module.
	moduleIDs := make([]string, 0, len(b.src.modulePages))
	for id := range b.src.modulePages {
		moduleIDs = append(moduleIDs, id)
	}
	sort.Strings(moduleIDs)
	for _, moduleID := range moduleIDs {
		for _, link := range b.src.modulePages[moduleID].detail.CurrentSemesterEvents {
			eventID := eventIDFromURL(link.URL)
			if eventID == "" {
				continue
			}
			if b.src.events[eventID] == nil {
				if !tombstones[eventID] { // removed on purpose by retention
					b.report.EventLinksNoArchive++
				}
				continue
			}
			if _, err := b.tx.Exec("INSERT OR IGNORE INTO module_event (module_id, event_id) VALUES (?, ?)", moduleID, eventID); err != nil {
				return err
			}
		}
	}
	return nil
}

func (b *builder) insertSemester(key string) error {
	year, err := strconv.Atoi(key[:len(key)-1])
	if err != nil {
		return nil
	}
	season, label := "summer", fmt.Sprintf("SoSe %d", year)
	starts, ends := fmt.Sprintf("%d-04-01", year), fmt.Sprintf("%d-09-30", year)
	if strings.HasSuffix(key, "W") {
		season, label = "winter", fmt.Sprintf("WiSe %d/%02d", year, (year+1)%100)
		starts, ends = fmt.Sprintf("%d-10-01", year), fmt.Sprintf("%d-03-31", year+1)
	}
	_, err = b.tx.Exec("INSERT INTO semester (key, season, year, label, starts_on, ends_on) VALUES (?, ?, ?, ?, ?, ?)",
		key, season, year, label, starts, ends)
	return err
}

// rhythm maps the QIS rhythm column. BTU alternates A and B weeks: „A/B" is every week.
func rhythm(raw string) string {
	switch low := strings.ToLower(strings.TrimSpace(raw)); {
	case low == "":
		return ""
	case low == "a/b" || strings.HasPrefix(low, "wöch") || strings.HasPrefix(low, "week"):
		return "weekly"
	case low == "a":
		return "week_a"
	case low == "b":
		return "week_b"
	case strings.HasPrefix(low, "einzel") || strings.HasPrefix(low, "single"):
		return "single"
	case strings.HasPrefix(low, "block"):
		return "block"
	}
	return "other"
}

func isoDate(m []string) string {
	day, _ := strconv.Atoi(m[1])
	month, _ := strconv.Atoi(m[2])
	return fmt.Sprintf("%s-%02d-%02d", m[3], month, day)
}

// clock keeps only a well-formed HH:MM.
func clock(raw string) any {
	m := clockTime.FindString(strings.TrimSpace(raw))
	if m == "" {
		return nil
	}
	if len(m) == 4 {
		m = "0" + m
	}
	return m
}

func parseFloat(raw string) float64 {
	v, _ := strconv.ParseFloat(strings.ReplaceAll(firstFloat.FindString(raw), ",", "."), 64)
	return v
}

func parseInt(raw string) int {
	v, _ := strconv.Atoi(regexp.MustCompile(`\d+`).FindString(raw))
	return v
}

func eventIDFromURL(raw string) string {
	u, err := url.Parse(raw)
	if err != nil {
		return ""
	}
	if v := u.Query().Get("veranstaltung.veranstid"); v != "" {
		return v
	}
	return u.Query().Get("veranstid")
}
