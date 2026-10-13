package catalogbuild

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// Single dates that follow a rhythm are one series (normalize.Series): the rows stay as QIS lists
// them, and the series names the row of its first date in each of them.
func TestBuildReadsSingleDatesThatFollowARhythmAsASeries(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "series.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })
	put := func(id, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceQISEvent, Key: id, URL: id, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 10, 13, 1, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}
	room := "Lehrgebäude 1A - 0.22 - Zentralcampus"
	once := func(day, time, date string) fixtureSlot {
		return fixtureSlot{day: day, time: time, rhythm: "Einzel", dates: "am " + date, room: room}
	}
	// A seminar on the Tuesdays of October and November, listed date by date, one of them in
	// another room and one ending later; two Thursdays; a weekly row; and a row listed after the
	// series that QIS put first on the page of another event.
	moved := once("Di.", "13:45 bis 15:15", "10.11.2026")
	moved.room = "Hauptgebäude - HG 0.20 - Zentralcampus"
	put("160001", fixtureEventPage("Seminar Datenschutz", "Seminar", "WS 2026/27",
		once("Di.", "13:45 bis 15:15", "20.10.2026"),
		once("Di.", "13:45 bis 15:15", "13.10.2026"),
		once("Di.", "13:45 bis 15:15", "27.10.2026"),
		once("Di.", "13:45 bis 15:15", "03.11.2026"),
		moved,
		once("Di.", "13:45 bis 16:00", "17.11.2026"),
		once("Do.", "09:15 bis 10:45", "15.10.2026"),
		once("Do.", "09:15 bis 10:45", "22.10.2026"),
		fixtureSlot{day: "Mi.", time: "11:30 bis 13:00", rhythm: "A/B", dates: "07.10.2026 bis 27.01.2027", room: room},
	))
	// Every other Monday, the break taking one: a fortnightly series.
	put("160002", fixtureEventPage("Praktikum Werkstoffe", "Praktikum", "WS 2026/27",
		once("Mo.", "07:30 bis 11:00", "30.11.2026"),
		once("Mo.", "07:30 bis 11:00", "14.12.2026"),
		once("Mo.", "07:30 bis 11:00", "11.01.2027"),
		once("Mo.", "07:30 bis 11:00", "25.01.2027"),
	))
	// An exam's dates are sittings, however regular.
	put("160003", fixtureEventPage("Klausur Datenschutz", "Prüfung", "WS 2026/27",
		once("Fr.", "10:00 bis 12:00", "05.02.2027"),
		once("Fr.", "10:00 bis 12:00", "12.02.2027"),
		once("Fr.", "10:00 bis 12:00", "19.02.2027"),
	))

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	if report.EventSeries != 2 || report.EventSeriesDates != 10 {
		t.Errorf("series = %d with %d dates; want 2 with 10", report.EventSeries, report.EventSeriesDates)
	}
	want(t, db, "SELECT ord, rhythm, first_date, end_time, series, series_weeks FROM event_date WHERE event_id = '160001' ORDER BY ord",
		"1|single|2026-10-20|15:15|2|1",
		"2|single|2026-10-13|15:15|2|1",
		"3|single|2026-10-27|15:15|2|1",
		"4|single|2026-11-03|15:15|2|1",
		"5|single|2026-11-10|15:15|2|1",
		"6|single|2026-11-17|16:00|2|1",
		"7|single|2026-10-15|10:45|∅|∅",
		"8|single|2026-10-22|10:45|∅|∅",
		"9|weekly|2026-10-07|13:00|∅|∅")
	want(t, db, "SELECT DISTINCT series, series_weeks FROM event_date WHERE event_id = '160002'", "1|2")
	want(t, db, "SELECT COUNT(*) FROM event_date WHERE event_id = '160003' AND series IS NOT NULL", "0")
	// The readers' view carries both columns.
	want(t, db, "SELECT COUNT(*) FROM pragma_table_info('v_module_schedule') WHERE name IN ('series', 'series_weeks')", "2")

	checks, err := db.Validate(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, c := range checks {
		switch {
		case c.Status == catalogdb.StatusFail && c.Value > 0:
			t.Errorf("check %q fails on the built series: %+v", c.Name, c)
		case c.Name == "series of single dates" && c.Value != 2:
			t.Errorf("%q = %d, want 2", c.Name, c.Value)
		}
	}

	// A series that is not one: rows of another start, of a weekly rhythm, too few dates.
	for _, stmt := range []string{
		"UPDATE event_date SET start_time = '14:00' WHERE event_id = '160001' AND ord = 3",
		"UPDATE event_date SET series = NULL, series_weeks = NULL WHERE event_id = '160002' AND ord IN (3, 4)",
		"UPDATE event_date SET series = 9, series_weeks = 1 WHERE event_id = '160001' AND ord = 9",
	} {
		if _, err := db.SQL().Exec(stmt); err != nil {
			t.Fatal(err)
		}
	}
	checks, err = db.Validate(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	failed := map[string][]string{}
	for _, c := range checks {
		if c.Status == catalogdb.StatusFail && c.Value > 0 {
			failed[c.Name] = c.Samples
		}
	}
	for name, samples := range map[string][]string{
		"a series of single dates holds single dates of one slot": {"160001/3", "160001/9"},
		"a series of single dates has three dates or more":        {"160001/9", "160002/1"},
	} {
		if got, ok := failed[name]; !ok || len(got) != len(samples) || got[0] != samples[0] || got[1] != samples[1] {
			t.Errorf("check %q: samples %v, want %v (failed: %v)", name, got, samples, failed)
		}
	}
}
