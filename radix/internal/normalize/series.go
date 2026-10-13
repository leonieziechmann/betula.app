package normalize

import (
	"sort"
	"time"
)

// Series of single dates. QIS lists some Termine date by date, each row „Einzel am 13.10.2026"
// with its weekday and times, although the dates follow a rhythm all the same: a seminar on six
// Tuesdays, a lab every other Thursday. A calendar showed them as six entries of their own, and
// the week of a module as „6 Termine". Series reads such rows as what they are:
//
//   - single dates of one event in one group, on one weekday, at one start („Di 13:45"), whatever
//     their end, room, teacher or comment: a date in another room is still a date of the series;
//   - cut where two dates are more than SeriesMaxGapDays apart;
//   - SeriesMinDates distinct dates or more, on a grid of one to four weeks: the gaps between them
//     have a greatest common divisor of 7, 14, 21 or 28 days;
//   - and the rhythm shows: at least half of the gaps are one step of it. Dates two, three and four
//     weeks apart in turn have a divisor of a week, but meet no week in a row (canary's data of
//     2026-10-13: nineteen such runs, „Einführung in wissenschaftliches Arbeiten" on 15.10., 12.11.,
//     26.11., 17.12. and 14.01.); they stay single dates.
//
// A series is named by the ord of the row of its first date. Nothing is merged or invented: the
// rows stay as QIS lists them, a reader that knows no series reads them as before, and a reader
// that knows them still finds each date, its room and its cancellation in its own row.

// SeriesMinDates is the fewest distinct dates of a series: two dates a week apart are a pair,
// three make a rhythm.
const SeriesMinDates = 3

// SeriesMaxGapDays is the longest gap within a series: the two weeks of the Christmas break and
// one week more. A weekly series may miss three dates in a row, a fortnightly one one.
const SeriesMaxGapDays = 28

// SeriesDate is a single date as Series reads it: its row's ord, the group it belongs to, its
// start as Clock writes it („13:45") and its day (ISO, „2026-10-13"). A date without a start
// or a day is in no series.
type SeriesDate struct {
	Ord   int
	Group string
	Start string
	Day   string
}

// SeriesOf is the series a single date belongs to: the ord of the row of the series' first date
// (the lowest ord on that day), and the weeks from one date of the series to the next, 1 to 4.
type SeriesOf struct {
	First int
	Weeks int
}

// Series returns the series of every date that belongs to one, by ord. dates are the single
// dates of one event; Series does not check that they are single.
func Series(dates []SeriesDate) map[int]SeriesOf {
	type slot struct {
		group, start string
		weekday      time.Weekday
	}
	slots := make(map[slot][]seriesDay)
	var order []slot
	for _, d := range dates {
		if Clock(d.Start) != d.Start || d.Start == "" {
			continue
		}
		day, err := time.Parse("2006-01-02", d.Day)
		if err != nil {
			continue
		}
		key := slot{d.Group, d.Start, day.Weekday()}
		if _, seen := slots[key]; !seen {
			order = append(order, key)
		}
		slots[key] = append(slots[key], seriesDay{d.Ord, day})
	}

	series := make(map[int]SeriesOf)
	for _, key := range order {
		days := slots[key]
		sort.Slice(days, func(i, j int) bool {
			if !days[i].day.Equal(days[j].day) {
				return days[i].day.Before(days[j].day)
			}
			return days[i].ord < days[j].ord
		})
		run := 0
		for i := 1; i <= len(days); i++ {
			if i < len(days) && daysBetween(days[i-1].day, days[i].day) <= SeriesMaxGapDays {
				continue
			}
			if weeks, ok := rhythmOf(days[run:i]); ok {
				for _, d := range days[run:i] {
					series[d.ord] = SeriesOf{First: days[run].ord, Weeks: weeks}
				}
			}
			run = i
		}
	}
	return series
}

// seriesDay is one row of a slot: its ord and its day.
type seriesDay struct {
	ord int
	day time.Time
}

// rhythmOf says whether the days of a run, in date order, are a series, and how many weeks lie
// between two of its dates: SeriesMinDates distinct days whose gaps divide by one to four weeks and
// by nothing longer, at least half of them one step.
func rhythmOf(days []seriesDay) (weeks int, ok bool) {
	var gaps []int
	step := 0
	for i := 1; i < len(days); i++ {
		gap := daysBetween(days[i-1].day, days[i].day)
		if gap == 0 {
			continue
		}
		gaps = append(gaps, gap)
		step = gcd(step, gap)
	}
	if len(gaps)+1 < SeriesMinDates || step%7 != 0 || step < 7 || step > 28 {
		return 0, false
	}
	steps := 0
	for _, gap := range gaps {
		if gap == step {
			steps++
		}
	}
	if 2*steps < len(gaps) {
		return 0, false
	}
	return step / 7, true
}

// daysBetween counts the days from a to b, two midnights in UTC.
func daysBetween(a, b time.Time) int {
	return int(b.Sub(a).Hours() / 24)
}

func gcd(a, b int) int {
	for b != 0 {
		a, b = b, a%b
	}
	return a
}
