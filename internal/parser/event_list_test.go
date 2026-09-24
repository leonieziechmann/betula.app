package parser

import (
	"bytes"
	"os"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/internal/model"
)

// testdata/qis_event_list.html is an answer of the QIS event search to twelve event IDs
// (2026-09-24), with leading whitespace removed; testdata/qis_event_<id>.html are the pages
// of nine of them, cut to the content. They were chosen for what they show: groups,
// single dates, a series without an end, a cancelled date with a note, remarks the list
// does not show, an exam of the summer semester, the placeholder QIS enters for an exam
// without a date, and an event without any date.
func readEventList(t *testing.T) *EventList {
	t.Helper()
	body, err := os.ReadFile("testdata/qis_event_list.html")
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	list, err := SplitEventList(bytes.NewReader(body))
	if err != nil {
		t.Fatalf("SplitEventList: %v", err)
	}
	return list
}

func listEntry(t *testing.T, list *EventList, id string) *model.EventDetail {
	t.Helper()
	for _, e := range list.Entries {
		if e.ID == id {
			d, err := NewEventListParser().Parse(bytes.NewReader(e.HTML), e.ID, "https://example/event/"+id)
			if err != nil {
				t.Fatalf("Parse %s: %v", id, err)
			}
			return d
		}
	}
	t.Fatalf("no entry for %s", id)
	return nil
}

func eventPage(t *testing.T, id string) *model.EventDetail {
	t.Helper()
	body, err := os.ReadFile("testdata/qis_event_" + id + ".html")
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	d, err := NewEventParser().Parse(bytes.NewReader(body), id, "https://example/event/"+id)
	if err != nil {
		t.Fatalf("Parse %s: %v", id, err)
	}
	return d
}

func TestSplitEventList(t *testing.T) {
	list := readEventList(t)
	if list.Hits != 12 {
		t.Errorf("Hits = %d, want 12", list.Hits)
	}
	want := []string{"148307", "145503", "147828", "149030", "151278", "150708", "148362", "147988", "151102", "151296", "152864", "149089"}
	if len(list.Entries) != len(want) {
		t.Fatalf("%d entries, want %d", len(list.Entries), len(want))
	}
	for i, e := range list.Entries {
		if e.ID != want[i] {
			t.Errorf("entry %d is %s, want %s", i, e.ID, want[i])
		}
		// An entry is archived on its own: it must not carry its neighbour or the page footer.
		if n := strings.Count(string(e.HTML), "publishSubDir=veranstaltung"); n != 1 {
			t.Errorf("entry %s links %d events, want 1", e.ID, n)
		}
		if strings.Contains(string(e.HTML), "divfoot") {
			t.Errorf("entry %s carries the page footer", e.ID)
		}
	}
}

// A page that is not a result of the search is an error, never an empty result: an empty
// result would mark every event asked for as gone.
func TestSplitEventListRejectsOtherPages(t *testing.T) {
	body, err := os.ReadFile("testdata/qis_event_151296.html")
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	if _, err := SplitEventList(bytes.NewReader(body)); err == nil {
		t.Errorf("an event page was taken for a result of the search")
	}
	if list, err := SplitEventList(strings.NewReader(`<div class="InfoLeiste">0 Treffer</div>`)); err != nil || len(list.Entries) != 0 || list.Hits != 0 {
		t.Errorf("an empty result = %+v, %v", list, err)
	}
}

func TestEventListParser(t *testing.T) {
	list := readEventList(t)

	d := listEntry(t, list, "151296")
	if d.Title != "Lightweight Design and Construction" || d.EventNumber != "350523" || d.EventType != "Übung/Praktikum" ||
		d.Semester != "WS 2026/27" || d.SWS != "2" || d.ExpectedParticipants != "45" || d.RawURL != "https://example/event/151296" {
		t.Errorf("basic data = %+v", d)
	}
	if len(d.ResponsiblePersons) != 2 || d.ResponsiblePersons[1].Name != "Coppola Rupp" {
		t.Errorf("persons = %+v", d.ResponsiblePersons)
	}
	if len(d.Schedules) != 6 {
		t.Fatalf("%d dates, want 6: %+v", len(d.Schedules), d.Schedules)
	}
	first, single, group3 := d.Schedules[0], d.Schedules[1], d.Schedules[2]
	if first.GroupName != UnnamedGroup || first.DayOfWeek != "Mittwoch" || first.StartTime != "07:30" || first.EndTime != "09:00" ||
		first.Rhythm != "A/B" || first.Duration != "07.10.2026 bis 27.01.2027" {
		t.Errorf("first date = %+v", first)
	}
	// The list names the room by building and number; the page adds the campus.
	if first.Room != "Forschungszentrum 3H - 1.05" || RoomID(first.RoomURL) != "3460" {
		t.Errorf("room = %q (%s)", first.Room, first.RoomURL)
	}
	if single.Rhythm != "Einzel" || single.Duration != "20.11.2026 bis 20.11.2026" {
		t.Errorf("single date = %+v", single)
	}
	if group3.GroupName != "3-Gruppe" {
		t.Errorf("group = %q, want 3-Gruppe", group3.GroupName)
	}
	// The „Bemerkung" column of the list is the maximum of participants of the date; it is
	// not a remark.
	for _, s := range d.Schedules {
		if s.Comment != "" {
			t.Errorf("comment = %q", s.Comment)
		}
	}

	// A cancelled date keeps its note, written as the page writes it.
	if got := listEntry(t, list, "152864").Schedules[1].CancelledDates; got != "14.10.2026: findet ersatzweise im HS 11.301 statt." {
		t.Errorf("cancelled = %q", got)
	}
	// A series without an end, and the first of several instructors, as the page parser reads them.
	phase := listEntry(t, list, "151278").Schedules
	if phase[0].Rhythm != "A" || phase[0].Duration != "von 05.10.2026" || phase[1].Instructor != "Hempel" || phase[1].Room != "Mehrzweckgebäude - 222" {
		t.Errorf("dates = %+v", phase)
	}
	// „keine Angabe" is no day; the date still counts for its time.
	zww := listEntry(t, list, "148307").Schedules
	if len(zww) != 1 || zww[0].DayOfWeek != "" || zww[0].StartTime != "09:00" {
		t.Errorf("dates = %+v", zww)
	}
	if got := listEntry(t, list, "149089").MaxParticipants; got != "100" {
		t.Errorf("maximum = %q, want 100", got)
	}
	if got := listEntry(t, list, "149030").Schedules; len(got) != 0 {
		t.Errorf("an event without dates has %+v", got)
	}
}

// QIS prints an empty link where an event has no institution. It follows the type, which
// must not be taken for the label of an institution.
func TestEventListParserEmptyInstitution(t *testing.T) {
	entry := `<div><h3><a href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;publishid=148210&amp;publishSubDir=veranstaltung">Doktoranden-Seminar</a></h3></div>
<div>
    WS 2026/27&nbsp;&nbsp;&nbsp;
 440003 &nbsp;&nbsp;&nbsp;
Seminar &nbsp;&nbsp;
<a class="regular" href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;publishSubDir=einrichtung&amp;einrichtung.eid="></a>
Zugeordnete Lehrperson
:&nbsp;&nbsp;
<a class="regular" href="https://www.b-tu.de/qisserver3/rds?state=verpublish&amp;publishSubDir=personal&amp;personal.pid=750">Riebel</a>
</div>`
	d, err := NewEventListParser().Parse(strings.NewReader(entry), "", "")
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if d.ID != "148210" || d.EventType != "Seminar" || d.EventNumber != "440003" || len(d.Institutions) != 0 || len(d.ResponsiblePersons) != 1 {
		t.Errorf("entry = %+v", d)
	}
}

// Every entry of the fixture agrees with the page of its event: the list and the page state
// the same dates, although the page adds remarks („MCA-Teilleistung", „nur online"),
// full names, roles and the campus of every room.
func TestSameScheduleOnRealPages(t *testing.T) {
	list := readEventList(t)
	for _, id := range []string{"145503", "147828", "147988", "149030", "150708", "151102", "151278", "151296", "152864"} {
		entry, page := listEntry(t, list, id), eventPage(t, id)
		if !SameSchedule(entry, page) {
			t.Errorf("%s: the list and the page disagree\nlist: %s\npage: %s", id, scheduleKey(entry), scheduleKey(page))
		}
	}
	if page := eventPage(t, "151102"); !strings.Contains(page.Schedules[0].Comment, "MCA-Teilleistung") {
		t.Errorf("the page lost its remark: %+v", page.Schedules)
	}
}

// What the list catches is what makes the page worth fetching again.
func TestSameScheduleSeesChanges(t *testing.T) {
	list := readEventList(t)
	page := eventPage(t, "151278")
	changes := map[string]func(d *model.EventDetail){
		"room": func(d *model.EventDetail) {
			d.Schedules[1].RoomURL = strings.Replace(d.Schedules[1].RoomURL, "rgid=3392", "rgid=3393", 1)
		},
		"time":      func(d *model.EventDetail) { d.Schedules[0].StartTime = "07:45" },
		"day":       func(d *model.EventDetail) { d.Schedules[0].DayOfWeek = "Donnerstag" },
		"rhythm":    func(d *model.EventDetail) { d.Schedules[0].Rhythm = "B" },
		"end":       func(d *model.EventDetail) { d.Schedules[0].Duration = "05.10.2026 bis 25.01.2027" },
		"cancelled": func(d *model.EventDetail) { d.Schedules[0].CancelledDates = "12.10.2026" },
		"new date": func(d *model.EventDetail) {
			d.Schedules = append(d.Schedules, model.EventSchedule{GroupName: UnnamedGroup, DayOfWeek: "Freitag", StartTime: "10:00", EndTime: "12:00"})
		},
		"no dates": func(d *model.EventDetail) { d.Schedules = nil },
		"title":    func(d *model.EventDetail) { d.Title = "Betriebliche Phase 2" },
	}
	for name, change := range changes {
		entry := listEntry(t, list, "151278")
		change(entry)
		if SameSchedule(entry, page) {
			t.Errorf("%s: a change of the list went unnoticed", name)
		}
	}
	// The order of the dates is the list's own and says nothing.
	entry := listEntry(t, list, "151278")
	entry.Schedules[0], entry.Schedules[1] = entry.Schedules[1], entry.Schedules[0]
	if !SameSchedule(entry, page) {
		t.Errorf("the order of the dates made a difference")
	}
}

func TestAwaitsDates(t *testing.T) {
	list := readEventList(t)
	for id, want := range map[string]bool{
		"149030": true,  // no date at all
		"150708": true,  // the placeholder of an exam: Sunday 01:00–02:30 on 27.12.2015
		"151296": false, // a schedule
		"145503": false, // an exam on 16.09.2026
		"148307": true,  // a time, but neither a day of the week nor a day
	} {
		if got := AwaitsDates(listEntry(t, list, id)); got != want {
			t.Errorf("AwaitsDates(%s) = %v, want %v", id, got, want)
		}
	}
	day := func(s model.EventSchedule) *model.EventDetail {
		return &model.EventDetail{Schedules: []model.EventSchedule{s}}
	}
	if !AwaitsDates(day(model.EventSchedule{DayOfWeek: "Mittwoch", Rhythm: "A/B"})) {
		t.Errorf("a day without a time is no date yet")
	}
	if AwaitsDates(day(model.EventSchedule{StartTime: "23:45", EndTime: "24:00", Duration: "am 29.01.2027"})) {
		t.Errorf("a deadline is a date")
	}
}
