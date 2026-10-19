package model

import "time"

// EventResponsiblePerson represents an instructor or organizer assigned to an event.
type EventResponsiblePerson struct {
	Name string `json:"name"`
	Role string `json:"role,omitempty"` // e.g. "verantwort", "begleitend"
	URL  string `json:"url,omitempty"`
}

// EventSchedule represents an individual scheduled meeting/slot for an event.
type EventSchedule struct {
	GroupName      string `json:"group_name,omitempty"` // e.g. "[unbenannt]", "Gruppe 1"
	DayOfWeek      string `json:"day_of_week"`          // e.g. "Di.", "Mo."
	TimeSlot       string `json:"time_slot"`            // e.g. "10:00 bis 11:30"
	StartTime      string `json:"start_time,omitempty"` // e.g. "10:00"
	EndTime        string `json:"end_time,omitempty"`   // e.g. "11:30"
	Rhythm         string `json:"rhythm,omitempty"`     // e.g. "wöchentlich", "A/B", "Einzeltermin"
	Duration       string `json:"duration,omitempty"`   // e.g. "14.04.2026 bis 21.07.2026"
	Room           string `json:"room,omitempty"`       // e.g. "Gebäude 6 - SFB - 6.210 Labor für Medientechnik"
	RoomURL        string `json:"room_url,omitempty"`   // link to room in QIS
	Instructor     string `json:"instructor,omitempty"` // instructor name in schedule row
	InstructorURL  string `json:"instructor_url,omitempty"`
	Comment        string `json:"comment,omitempty"`
	CancelledDates string `json:"cancelled_dates,omitempty"` // fällt aus am
}

// EventProgram is a row of an event page's „Studiengänge": a program the event is meant for, as
// QIS names it (owner, 2026-10-19: the Programmierpraktikum in Sachsendorf is Medizininformatik's,
// the one at the Zentralcampus Informatik's and Mathematik's). The page lists it in its own words:
// „Informatik Bachelor (universitäres Profil)", „Elektrotechnik - MIT - Bachelor",
// „Medizintechnik - dual: EM - Bachelor - A2"; the build resolves it (catalogbuild.writeEventPrograms).
type EventProgram struct {
	Label     string `json:"label"`               // „Künstliche Intelligenz Bachelor"
	Semesters string `json:"semesters,omitempty"` // „1 - 1": the Fachsemester it is meant for
	PO        string `json:"po,omitempty"`        // „2022"; often empty
	Remark    string `json:"remark,omitempty"`    // „Wahlpflicht"
	QISID     string `json:"qis_id,omitempty"`    // k_abstgv.abstgvnr of the link to the program's page in QIS
}

// EventDetail represents a full lecture/exercise/exam event from the BTU QIS system.
type EventDetail struct {
	ID                   string                   `json:"id"`                 // veranstid
	EventNumber          string                   `json:"event_number"`       // e.g. "140037"
	Title                string                   `json:"title"`              // e.g. "Einführung in die Programmierung - C++ (SFB)"
	EventType            string                   `json:"event_type"`         // e.g. "Vorlesung", "Übung", "Prüfung"
	Semester             string                   `json:"semester,omitempty"` // e.g. "SS 2026"
	SWS                  string                   `json:"sws,omitempty"`      // e.g. "2"
	ExpectedParticipants string                   `json:"expected_participants,omitempty"`
	MaxParticipants      string                   `json:"max_participants,omitempty"`
	Hyperlink            string                   `json:"hyperlink,omitempty"`
	Description          string                   `json:"description,omitempty"`
	ResponsiblePersons   []EventResponsiblePerson `json:"responsible_persons,omitempty"`
	AssociatedModules    []string                 `json:"associated_modules,omitempty"` // list of module IDs (e.g. ["11826", "12105"])
	Programs             []EventProgram           `json:"programs,omitempty"`
	Institutions         []string                 `json:"institutions,omitempty"`
	Schedules            []EventSchedule          `json:"schedules,omitempty"`
	RawURL               string                   `json:"raw_url"`
	LastScrapedAt        time.Time                `json:"last_scraped_at"`
}
