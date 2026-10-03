// Package normalize turns the free text of the BTU sources into the enum and
// flag columns of the canonical model. SQLite in the browser (sql.js) only folds
// ASCII, so every comparison a consumer needs has to be settled here, in Go.
//
// All functions are rule-based and total: an unknown value yields "unknown"
// (empty string / ok=false), never an error, and the raw text stays in the
// database. A value that appears tomorrow must not break the build.
package normalize

import (
	"regexp"
	"strconv"
	"strings"
)

var (
	spaces   = regexp.MustCompile(`\s+`)
	firstInt = regexp.MustCompile(`\d+`)
	number   = regexp.MustCompile(`\d+(?:[.,]\d+)?`)
)

// fold lowercases (Unicode-aware) and collapses whitespace.
func fold(s string) string {
	return spaces.ReplaceAllString(strings.ToLower(strings.TrimSpace(s)), " ")
}

func containsAny(s string, subs ...string) bool {
	for _, sub := range subs {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}

// IsNone reports whether a free-text field states "nothing" („keine", "None", „-" …).
func IsNone(raw string) bool {
	switch strings.TrimRight(fold(raw), ".") {
	case "", "-", "–", "keine", "keiner", "keines", "none", "nein", "no", "entfällt", "k.a", "n/a", "ohne":
		return true
	}
	return false
}

// Turnus seasons and parities.
const (
	SeasonWinter    = "winter"
	SeasonSummer    = "summer"
	SeasonBoth      = "both"
	SeasonIrregular = "irregular"

	ParityEven = "even"
	ParityOdd  = "odd"
)

// Turnus parses „jedes Wintersemester gerader Jahre" / "Each summer semester odd year".
// parity is only set together with a season. "ungerade" contains "gerade", so odd is
// tested first.
func Turnus(raw string) (season, parity string) {
	s := fold(raw)
	switch {
	case s == "":
		return "", ""
	case containsAny(s, "sporadisch", "ankündigung", "announcement", "unregelmäßig", "irregular", "bedarf"):
		return SeasonIrregular, ""
	case containsAny(s, "wintersemester", "winter semester", "wise"):
		season = SeasonWinter
	case containsAny(s, "sommersemester", "summer semester", "sose"):
		season = SeasonSummer
	case containsAny(s, "jedes semester", "every semester", "each semester"):
		return SeasonBoth, ""
	default:
		return "", ""
	}

	switch {
	case containsAny(s, "ungerade", "odd"):
		parity = ParityOdd
	case containsAny(s, "gerade", "even"):
		parity = ParityEven
	}
	return season, parity
}

// Languages reports the teaching languages named in „Deutsch", "English", „Deutsch/Englisch".
func Languages(raw string) (german, english bool) {
	s := fold(raw)
	return containsAny(s, "deutsch", "german"), containsAny(s, "englisch", "english")
}

// DurationSemesters parses „2 Semester" / "2 semesters". Durations in weeks
// („10 Wochen") are not semesters and yield ok=false.
func DurationSemesters(raw string) (semesters int, ok bool) {
	s := fold(raw)
	if !strings.Contains(s, "semester") {
		return 0, false
	}
	n, err := strconv.Atoi(firstInt.FindString(s))
	if err != nil || n <= 0 {
		return 0, false
	}
	return n, true
}

// Limitation parses „Teilnehmerbeschränkung". known=false means the page gave no
// value. limit is 0 when the module is limited but the text names no number.
func Limitation(raw string) (known, limited bool, limit int) {
	if strings.TrimSpace(raw) == "" {
		return false, false, 0
	}
	if IsNone(raw) {
		return true, false, 0
	}
	n, _ := strconv.Atoi(firstInt.FindString(raw))
	return true, true, n
}

// Graded parses „Prüfungsleistung - benotet" / "Study Performance – ungraded".
// "unbenotet"/"ungraded" contain the positive word, so they are tested first.
func Graded(raw string) (graded, ok bool) {
	s := fold(raw)
	switch {
	case containsAny(s, "unbenotet", "ungraded", "nicht benotet", "not graded"):
		return false, true
	case containsAny(s, "benotet", "graded"):
		return true, true
	}
	return false, false
}

// Exam forms („Modulprüfung").
const (
	ExamFormMAP       = "map"        // Modulabschlussprüfung
	ExamFormPrereqMAP = "prereq_map" // Voraussetzung + Modulabschlussprüfung
	ExamFormMCA       = "mca"        // Continuous Assessment
	ExamFormPrereqMCA = "prereq_mca"
	ExamFormOther     = "other" // stated, but none of the above (e.g. thesis, internship)
)

// ExamForm returns "" when the page gives no usable statement („Keine Angabe …").
func ExamForm(raw string) string {
	s := fold(raw)
	if s == "" || containsAny(s, "keine angabe", "unspecified") {
		return ""
	}
	prereq := containsAny(s, "voraussetzung", "prerequisite")
	switch {
	case containsAny(s, "(mca)", "continuous assessment"):
		if prereq {
			return ExamFormPrereqMCA
		}
		return ExamFormMCA
	case containsAny(s, "(map)", "modulabschlussprüfung", "final module examination"):
		if prereq {
			return ExamFormPrereqMAP
		}
		return ExamFormMAP
	}
	return ExamFormOther
}

// ExamKinds are the assessment types named in „Prüfungsleistung/en für Modulprüfung".
type ExamKinds struct {
	Written      bool // Klausur, written examination
	Oral         bool // mündliche Prüfung
	Paper        bool // Hausarbeit, Beleg, Bericht, Essay, Protokoll
	Presentation bool // Vortrag, Präsentation, Referat
	Project      bool // Projekt-, Entwurfsarbeit
	Practical    bool // Testat, Praktikum, Labor, praktische Prüfung
}

func ParseExamKinds(details string) ExamKinds {
	s := fold(details)
	return ExamKinds{
		Written:      containsAny(s, "klausur", "schriftliche prüfung", "schriftliche leistung", "written exam", "written test", "e-klausur", "e-prüfung"),
		Oral:         containsAny(s, "mündlich", "oral exam", "oral test", "kolloquium", "colloquium"),
		Paper:        containsAny(s, "hausarbeit", "beleg", "bericht", "essay", "protokoll", "ausarbeitung", "term paper", "report", "research paper", "written elaboration", "portfolio"),
		Presentation: containsAny(s, "vortrag", "präsentation", "referat", "presentation", "poster"),
		Project:      containsAny(s, "projekt", "entwurf", "project", "design"),
		Practical:    containsAny(s, "testat", "praktikum", "labor", "praktische prüfung", "practical", "laboratory", "übungsaufgaben", "hausaufgaben", "exercises", "homework"),
	}
}

// Teaching forms.
const (
	FormLecture      = "lecture"
	FormExercise     = "exercise"
	FormSeminar      = "seminar"
	FormPractical    = "practical" // Praktikum, Laborausbildung
	FormProject      = "project"   // Projekt, Entwurf, Stegreif
	FormTutorial     = "tutorial"
	FormConsultation = "consultation"
	FormExcursion    = "excursion"
	FormSelfStudy    = "self_study"
	FormPaper        = "paper" // Hausarbeit as a form of work
	FormOther        = "other"
)

// TeachingForm classifies „Vorlesung", "Exercise", „Laborausbildung" …
// Combined labels („Vorlesung/Übung", as QIS event types use them) yield several forms.
func TeachingForm(raw string) []string {
	s := fold(raw)
	if s == "" || s == "•" {
		return nil
	}
	var forms []string
	add := func(form string, subs ...string) {
		if containsAny(s, subs...) {
			forms = append(forms, form)
		}
	}
	add(FormLecture, "vorlesung", "lecture")
	add(FormExercise, "übung", "exercise")
	add(FormSeminar, "seminar")
	add(FormPractical, "praktikum", "praktische", "practical", "labor", "krankenbett", "schulpraktisch", "lernwerkst")
	add(FormProject, "projekt", "project", "entwurf", "stegreif")
	add(FormTutorial, "tutori")
	add(FormConsultation, "konsultation", "consultation")
	add(FormExcursion, "exkursion", "excursion")
	add(FormSelfStudy, "selbststudium", "self organised", "self-organised", "self study")
	add(FormPaper, "hausarbeit", "research paper")
	if len(forms) == 0 {
		return []string{FormOther}
	}
	return forms
}

// Workload parses „4 SWS", "2 Hours per Week per Semester", „150 Stunden", "120 Hours".
func Workload(raw string) (sws, hours float64) {
	s := fold(raw)
	m := number.FindString(s)
	if m == "" {
		return 0, 0
	}
	v, err := strconv.ParseFloat(strings.ReplaceAll(m, ",", "."), 64)
	if err != nil {
		return 0, 0
	}
	if containsAny(s, "sws", "per week") {
		return v, 0
	}
	if containsAny(s, "stunden", "hours", "std") {
		return 0, v
	}
	return 0, 0
}

// Campuses as named at the end of a QIS room label („… - Zentralcampus").
const (
	CampusZentral     = "zentralcampus"
	CampusSachsendorf = "sachsendorf"
	CampusSenftenberg = "senftenberg"
	CampusNord        = "nord"
)

// Campus reads the campus from the last " - " segment of a room label. Matching
// the whole label would find "lg" in „A**llg**emeine" and similar accidents.
func Campus(room string) string {
	i := strings.LastIndex(room, " - ")
	if i < 0 {
		return ""
	}
	s := fold(room[i+3:])
	switch {
	case strings.Contains(s, "zentralcampus"):
		return CampusZentral
	case strings.Contains(s, "sachsendorf"):
		return CampusSachsendorf
	case strings.Contains(s, "senftenberg"):
		return CampusSenftenberg
	case strings.Contains(s, "campus nord"):
		return CampusNord
	}
	return ""
}

var weekdays = map[string]int{
	"mo": 1, "di": 2, "mi": 3, "do": 4, "fr": 5, "sa": 6, "so": 7,
	"montag": 1, "dienstag": 2, "mittwoch": 3, "donnerstag": 4, "freitag": 5, "samstag": 6, "sonntag": 7,
	"mon": 1, "tue": 2, "wed": 3, "thu": 4, "fri": 5, "sat": 6, "sun": 7,
	"monday": 1, "tuesday": 2, "wednesday": 3, "thursday": 4, "friday": 5, "saturday": 6, "sunday": 7,
}

// Weekday reads the day of a date as QIS writes it: „Mi." on an event page, „Mittwoch" in
// the event search. 1 is Monday; 0 is unknown, as „keine Angabe" is.
func Weekday(raw string) int {
	return weekdays[strings.Trim(fold(raw), ". ")]
}

var clockTime = regexp.MustCompile(`^\d{1,2}:\d{2}`)

// Clock keeps a well-formed time of day as HH:MM, and "" for anything else.
func Clock(raw string) string {
	m := clockTime.FindString(strings.TrimSpace(raw))
	if len(m) == 4 {
		m = "0" + m
	}
	return m
}

var (
	semesterYear  = regexp.MustCompile(`(\d{4}|\d{2})(?:\s*/\s*\d{2,4})?`)
	poYear        = regexp.MustCompile(`\b(19|20)\d{2}\b`)
	moduleIDToken = regexp.MustCompile(`\b\d{5}\b`)
)

// SemesterKey turns „SS 2026", „WiSe 2026/27", "Winter 2026/27" into a sortable
// key: "2026S", "2026W". A winter semester is keyed by the year it starts in.
func SemesterKey(raw string) string {
	s := fold(raw)
	var season string
	switch {
	case containsAny(s, "ws", "wise", "winter"):
		season = "W"
	case containsAny(s, "ss", "sose", "sommer", "summer"):
		season = "S"
	default:
		return ""
	}
	m := semesterYear.FindStringSubmatch(s)
	if m == nil {
		return ""
	}
	year := m[1]
	if len(year) == 2 {
		year = "20" + year
	}
	return year + season
}

// POVersion splits „2008 - 2. SÄ 2024" into the base year and the amendment text.
func POVersion(raw string) (year int, amendment string) {
	s := strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(raw), "PO "))
	base, rest, _ := strings.Cut(s, " - ")
	year, _ = strconv.Atoi(poYear.FindString(base))
	return year, strings.TrimSpace(rest)
}

// ModuleIDs returns the distinct 5-digit tokens of a text, in order of appearance.
// The caller keeps only those that are existing module IDs.
func ModuleIDs(text string) []string {
	var ids []string
	seen := make(map[string]bool)
	for _, id := range moduleIDToken.FindAllString(text, -1) {
		if !seen[id] {
			seen[id] = true
			ids = append(ids, id)
		}
	}
	return ids
}

// Module kinds in a program, shared by all sources.
const (
	KindCompulsory = "compulsory"
	KindElective   = "elective"
	KindThesis     = "thesis"
	KindInternship = "internship"
	KindFUES       = "fues"
)

// PlanKind maps the kind column of a study plan („Pflicht", „Wahlpflicht",
// „Abschlussarbeit", „Praktikum"). „Modul" and anything unknown state no kind.
func PlanKind(raw string) string {
	s := fold(raw)
	switch {
	case containsAny(s, "wahl", "elective", "optional"):
		return KindElective
	case containsAny(s, "pflicht", "compulsory", "mandatory"):
		return KindCompulsory
	case containsAny(s, "abschlussarbeit", "thesis"):
		return KindThesis
	case containsAny(s, "praktikum", "internship"):
		return KindInternship
	case containsAny(s, "füs", "fachübergreifend"):
		return KindFUES
	}
	return ""
}
