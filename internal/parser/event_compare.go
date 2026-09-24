package parser

import (
	"fmt"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode"

	"github.com/leonieziechmann/betula/internal/model"
	"github.com/leonieziechmann/betula/internal/normalize"
)

var reDay = regexp.MustCompile(`(\d{1,2})\.(\d{1,2})\.(\d{4})`)

// SameSchedule reports whether two readings of an event state the same: its entry in the
// event search and its own page. It compares what both of them state and the catalog
// shows (title, number, type, semester, hours per week, the maximum of participants, and
// every date with its group, day, times, rhythm, days, room, instructor and cancellations)
// and leaves out what only the page states: the remark of a date, the first names and
// roles of the persons, the campus of a room. A room is compared by its QIS ID.
func SameSchedule(a, b *model.EventDetail) bool {
	return scheduleKey(a) == scheduleKey(b)
}

func scheduleKey(d *model.EventDetail) string {
	var b strings.Builder
	b.WriteString(strings.Join([]string{
		CleanSingleLine(d.Title), CleanSingleLine(d.EventNumber), CleanSingleLine(d.EventType),
		normalize.SemesterKey(d.Semester), CleanSingleLine(d.SWS), CleanSingleLine(d.MaxParticipants),
	}, "\x1f"))

	rows := make([]string, 0, len(d.Schedules))
	for _, s := range d.Schedules {
		first, last := DayRange(s.Duration)
		room := RoomID(s.RoomURL)
		if room == "" {
			room = CleanSingleLine(s.Room)
		}
		group := CleanSingleLine(s.GroupName)
		if group == UnnamedGroup {
			group = ""
		}
		rows = append(rows, strings.Join([]string{
			group,
			strconv.Itoa(normalize.Weekday(s.DayOfWeek)),
			normalize.Clock(s.StartTime), normalize.Clock(s.EndTime),
			strings.ToLower(CleanSingleLine(reRhythmTail.ReplaceAllString(reRhythmDays.ReplaceAllString(s.Rhythm, ""), ""))),
			first, last, room,
			strings.ToLower(CleanSingleLine(s.Instructor)),
			words(s.CancelledDates),
		}, "\x1f"))
	}
	sort.Strings(rows)
	for _, row := range rows {
		b.WriteString("\n")
		b.WriteString(row)
	}
	return b.String()
}

// DayRange returns the first and the last day a date names, as YYYY-MM-DD: „14.10.2026
// bis 27.01.2027", „am 04.12.2026", „von 05.10.2026".
func DayRange(duration string) (first, last string) {
	found := reDay.FindAllStringSubmatch(duration, -1)
	if len(found) == 0 {
		return "", ""
	}
	iso := func(m []string) string {
		day, _ := strconv.Atoi(m[1])
		month, _ := strconv.Atoi(m[2])
		return fmt.Sprintf("%s-%02d-%02d", m[3], month, day)
	}
	return iso(found[0]), iso(found[len(found)-1])
}

// RoomID returns the QIS ID of the room a date links (raum.rgid), or "".
func RoomID(roomURL string) string {
	if roomURL == "" {
		return ""
	}
	u, err := url.Parse(roomURL)
	if err != nil {
		return ""
	}
	return u.Query().Get("raum.rgid")
}

// words reduces a text to its lowercased words, so that „14.10.2026: findet … statt."
// and „14.10.2026 findet … statt." are the same statement.
func words(s string) string {
	return strings.Join(strings.FieldsFunc(strings.ToLower(s), func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r) && r != '.'
	}), " ")
}

// AwaitsDates reports whether an event does not say yet when it takes place: it has no
// date, or none with a time and a day. The placeholder QIS enters for an exam whose date
// is not fixed, 01:00 to 02:30 on a Sunday or without a weekday, is no date either
// (Folia reads it the same way, catalog/src/exam_reading.rs).
func AwaitsDates(d *model.EventDetail) bool {
	for _, s := range d.Schedules {
		first, _ := DayRange(s.Duration)
		if normalize.Clock(s.StartTime) == "" || (normalize.Weekday(s.DayOfWeek) == 0 && first == "") || isPlaceholder(s) {
			continue
		}
		return false
	}
	return true
}

func isPlaceholder(s model.EventSchedule) bool {
	weekday := normalize.Weekday(s.DayOfWeek)
	return normalize.Clock(s.StartTime) == "01:00" && normalize.Clock(s.EndTime) == "02:30" && (weekday == 7 || weekday == 0)
}

// Unsettled reports whether the dates of an event are not settled, so that they are worth
// asking about more often: the event awaits its dates, or one of its dates is the
// placeholder of an exam or looks wrong the way Folia marks it (catalog/src/exam_reading.rs):
// a time before 06:00 or after 22:00 that is not a deadline („bis 24:00"), an end before the
// start, or a day more than six months away from the event's own semester.
func Unsettled(d *model.EventDetail) bool {
	if AwaitsDates(d) {
		return true
	}
	from, to, known := semesterMonths(d.Semester)
	for _, s := range d.Schedules {
		if isPlaceholder(s) {
			return true
		}
		start, end := minutes(s.StartTime), minutes(s.EndTime)
		deadline := end == 24*60 && (start < 0 || start >= 22*60) // the day something is due
		if !deadline && (outsideDay(start) || outsideDay(end) || (start >= 0 && end >= 0 && end < start)) {
			return true
		}
		if !known {
			continue
		}
		first, last := DayRange(s.Duration)
		for _, day := range []string{first, last} {
			if m, ok := month(day); ok && (m < from || m > to) {
				return true
			}
		}
	}
	return false
}

// minutes returns a time of day in minutes, -1 when there is none.
func minutes(raw string) int {
	clock := normalize.Clock(raw)
	if clock == "" {
		return -1
	}
	h, _ := strconv.Atoi(clock[:2])
	m, _ := strconv.Atoi(clock[3:])
	return h*60 + m
}

func outsideDay(t int) bool {
	return t >= 0 && (t < 6*60 || t > 22*60)
}

// semesterMonths is the span of months a date of the semester can lie in: the semester and
// six months on either side, as months counted from year 0.
func semesterMonths(semester string) (from, to int, ok bool) {
	key := normalize.SemesterKey(semester)
	if key == "" {
		return 0, 0, false
	}
	year, err := strconv.Atoi(key[:len(key)-1])
	if err != nil {
		return 0, 0, false
	}
	starts := year*12 + 3 // April
	if strings.HasSuffix(key, "W") {
		starts = year*12 + 9 // October
	}
	return starts - 6, starts + 5 + 6, true
}

func month(isoDay string) (int, bool) {
	if len(isoDay) < 7 {
		return 0, false
	}
	year, errY := strconv.Atoi(isoDay[:4])
	m, errM := strconv.Atoi(isoDay[5:7])
	if errY != nil || errM != nil {
		return 0, false
	}
	return year*12 + m - 1, true
}
