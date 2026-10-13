package normalize

import (
	"reflect"
	"testing"
)

// single is a single date of the group „[unbenannt]" at 13:45.
func single(ord int, day string) SeriesDate {
	return SeriesDate{Ord: ord, Group: "[unbenannt]", Start: "13:45", Day: day}
}

func TestSeriesOfSingleDates(t *testing.T) {
	// Tuesdays from 13.10.2026 to 26.01.2027, the two of the break (22.12., 29.12.) not listed:
	// one series, named by its first row.
	var tuesdays []SeriesDate
	for i, day := range []string{"2026-10-13", "2026-10-20", "2026-10-27", "2026-11-03", "2026-11-10", "2026-11-17",
		"2026-11-24", "2026-12-01", "2026-12-08", "2026-12-15", "2027-01-05", "2027-01-12", "2027-01-19", "2027-01-26"} {
		tuesdays = append(tuesdays, single(i+1, day))
	}
	got := Series(tuesdays)
	if len(got) != len(tuesdays) {
		t.Fatalf("weekly: %d of %d dates in a series: %v", len(got), len(tuesdays), got)
	}
	for ord, of := range got {
		if of != (SeriesOf{First: 1, Weeks: 1}) {
			t.Errorf("weekly: row %d is in %+v", ord, of)
		}
	}

	// Every other Thursday, the break taking one of them: a fortnightly series. QIS lists the
	// rows in any order; the series is named by the row of its first date.
	fortnightly := []SeriesDate{
		single(4, "2026-10-29"), single(3, "2026-10-15"), single(5, "2026-11-12"),
		single(6, "2026-11-26"), single(7, "2026-12-10"), single(8, "2027-01-07"), single(9, "2027-01-21"),
	}
	for ord, of := range Series(fortnightly) {
		if of != (SeriesOf{First: 3, Weeks: 2}) {
			t.Errorf("fortnightly: row %d is in %+v", ord, of)
		}
	}
	if n := len(Series(fortnightly)); n != len(fortnightly) {
		t.Errorf("fortnightly: %d of %d dates in a series", n, len(fortnightly))
	}

	cases := []struct {
		name  string
		dates []SeriesDate
		want  map[int]SeriesOf
	}{
		{"two dates are a pair, not a rhythm", []SeriesDate{single(1, "2026-10-13"), single(2, "2026-10-20")}, map[int]SeriesOf{}},
		{"three weeks apart is no rhythm Betula names", []SeriesDate{single(1, "2026-10-13"), single(2, "2026-11-03"), single(3, "2026-11-24")}, map[int]SeriesOf{}},
		{
			"a week missing here and there is still weekly",
			[]SeriesDate{single(1, "2026-10-13"), single(2, "2026-10-27"), single(3, "2026-11-03"), single(4, "2026-11-17")},
			map[int]SeriesOf{1: {1, 1}, 2: {1, 1}, 3: {1, 1}, 4: {1, 1}},
		},
		{
			"a gap of more than four weeks ends a series; the two dates after it are none",
			[]SeriesDate{single(1, "2026-10-13"), single(2, "2026-10-20"), single(3, "2026-10-27"), single(4, "2026-12-01"), single(5, "2026-12-08")},
			map[int]SeriesOf{1: {1, 1}, 2: {1, 1}, 3: {1, 1}},
		},
		{
			"four weeks apart are within one series",
			[]SeriesDate{single(1, "2026-10-13"), single(2, "2026-10-20"), single(3, "2026-11-17")},
			map[int]SeriesOf{1: {1, 1}, 2: {1, 1}, 3: {1, 1}},
		},
		{
			"two rows of one day (two rooms) count as one date",
			[]SeriesDate{single(1, "2026-10-13"), single(2, "2026-10-13"), single(3, "2026-10-20")},
			map[int]SeriesOf{},
		},
		{
			"two rows of one day in a series",
			[]SeriesDate{single(2, "2026-10-13"), single(1, "2026-10-13"), single(3, "2026-10-20"), single(4, "2026-10-27")},
			map[int]SeriesOf{1: {1, 1}, 2: {1, 1}, 3: {1, 1}, 4: {1, 1}},
		},
		{
			"another start, group or weekday is another slot",
			[]SeriesDate{
				single(1, "2026-10-13"), {Ord: 2, Group: "[unbenannt]", Start: "15:30", Day: "2026-10-20"},
				{Ord: 3, Group: "1-Gruppe", Start: "13:45", Day: "2026-10-27"}, single(4, "2026-10-28"),
			},
			map[int]SeriesOf{},
		},
		{
			"a date without a start or a day is in none",
			[]SeriesDate{single(1, "2026-10-13"), {Ord: 2, Group: "[unbenannt]", Day: "2026-10-20"}, {Ord: 3, Group: "[unbenannt]", Start: "13:45", Day: "20.10.2026"}, single(4, "2026-10-27")},
			map[int]SeriesOf{},
		},
	}
	for _, c := range cases {
		if got := Series(c.dates); !reflect.DeepEqual(got, c.want) {
			t.Errorf("%s: Series = %v, want %v", c.name, got, c.want)
		}
	}

	// Two slots of one event, each its own series.
	two := []SeriesDate{
		single(1, "2026-10-13"), single(2, "2026-10-20"), single(3, "2026-10-27"),
		{Ord: 4, Group: "[unbenannt]", Start: "09:15", Day: "2026-10-15"},
		{Ord: 5, Group: "[unbenannt]", Start: "09:15", Day: "2026-10-29"},
		{Ord: 6, Group: "[unbenannt]", Start: "09:15", Day: "2026-11-12"},
	}
	want := map[int]SeriesOf{1: {1, 1}, 2: {1, 1}, 3: {1, 1}, 4: {4, 2}, 5: {4, 2}, 6: {4, 2}}
	if got := Series(two); !reflect.DeepEqual(got, want) {
		t.Errorf("two slots: Series = %v, want %v", got, want)
	}
}
