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
	"slices"
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

var (
	// A phrase that rules out the modules after it, to the end of its statement:
	// „Keine erfolgreiche Teilnahme am zugehörigen Auslaufmodul 12212", „Kein Abschluss des
	// Moduls", „kein paralleler Besuch des Moduls", „Keine Doppelbelegung mit Modul", "No
	// successful participation in module", "No double occupancy with Module", "Students must
	// not have previously taken and passed Module". The noun has to end there, so „Keine
	// Teilnahmevoraussetzungen" rules out nothing.
	excludesAfter = regexp.MustCompile(`(?i)` +
		`\bkein(?:e|en|er|em)?\s+(?:(?:erfolgreich|vorherig|parallel|gleichzeitig|bisherig)\w*\s+(?:(?:oder|und|bzw\.?)\s+)?)*(?:teilnahme|belegung|abschluss|besuch|anrechnung)\b` +
		`|doppelbelegung` +
		`|\bno\s+(?:(?:succes+ful+|prior|previous|parallel|simultaneous)\s+(?:(?:or|and)\s+)?)*(?:participation|completion|completition|enrol+ment|registration|attendance)\b` +
		`|\bdouble\s+(?:occupancy|enrol+ment|registration|booking)\b` +
		`|\b(?:must|may|should|shall)\s+not\s+have\b` +
		`|\bnicht\s+für\s+studierende\b|\bnot\s+(?:open\s+)?(?:to|for)\s+students\b`)
	// A phrase that rules out every module of its sentence, the ones before it too: „Für
	// Studierende, die bereits am Modul 12110 Mediendesign erfolgreich teilgenommen haben, ist
	// eine Belegung nicht mehr möglich.", "Module 12276 Porous Materials was not successfully
	// completed."
	excludesSentence = regexp.MustCompile(`(?i)\bnicht\s+mehr\s+möglich\b|\bno\s+longer\s+(?:possible|permitted|allowed)\b` +
		`|\b(?:was|were)\s+not\s+(?:successfully\s+)?(?:completed|passed|taken)\b` +
		`|\b(?:wurde|wurden)\s+nicht\s+(?:erfolgreich\s+)?(?:abgeschlossen|bestanden|absolviert)\b`)
	// A statement that says what is needed rules nothing out: „Kein Abschluss erforderlich,
	// Teilnahme an 12345 genügt".
	statesANeed = regexp.MustCompile(`(?i)\b(?:erforderlich|notwendig|nötig|vorausgesetzt|verlangt|required|necessary|needed)\b`)
	// Where a statement ends within a line: a full stop, a semicolon, ! or ? before a space or
	// the end, or a clause that turns against it („…, aber Kenntnisse aus 12345").
	statementStop = regexp.MustCompile(`(?i)[.;!?](?:\s|$)|,?\s+(?:aber|jedoch|sondern|but|however)\b`)
	// A line that goes on with the statement before it: the module number of a list item
	// („• 11116 - Höhere Mathematik K"), „Modul 11282 …", a conjunction, or a lower-case word
	// that carries on the sentence ("in associated phase-out modules").
	listedModule   = regexp.MustCompile(`(?i)^(?:modul(?:e|s|en)?\b\s*)?\d{5}\b|^(?:und|oder|sowie|bzw|als auch|and|or|as well as)\b`)
	lineLead       = regexp.MustCompile(`^[\s•*_"'„“”‚‘’(\-–]*`)
	lowerCaseStart = regexp.MustCompile(`^\p{Ll}`)
)

var (
	// The conjunctions between two modules of a prerequisite text: an „oder" (group 1) or an
	// „und". „und/oder" ends in „oder".
	conjunction = regexp.MustCompile(`(?i)(\b(?:oder|or|beziehungsweise|alternativ|alternatively)\b|\bbzw\b\.?)` +
		`|\b(?:und|and|sowie|as\s+well\s+as|als\s+auch|plus|zusätzlich|additionally|außerdem|ferner|furthermore)\b|;`)
	// A statement after which one of the modules is enough: „eines der Module", „einem der
	// folgenden Module", „mindestens eines der Module", "one of the following modules", "at least
	// one of the modules", "one of the following courses of the modules".
	oneOf = regexp.MustCompile(`(?i)\b(?:mindestens\s+)?(?:eine[smr]?|eins)\s+der\s+(?:folgenden\s+|nachstehenden\s+|genannten\s+)?(?:module|modulen|lehrveranstaltungen)\b` +
		`|\b(?:at\s+least\s+)?one\s+of\s+(?:the\s+)?(?:following\s+)?(?:modules|courses)\b`)
	// What may stand between an „und" and the module it joins: „und Modul", „and module", „sowie
	// des Moduls".
	leadToModule = regexp.MustCompile(`(?i)^[\s•:–-]*(?:(?:modul(?:e|s|en)?|modules?|das|des|dem|the|of)\s*)*$`)
	// What stands within brackets, one level at a time, or within quotes, on one line.
	bracketed = regexp.MustCompile(`\([^()\n]*\)`)
	quoted    = regexp.MustCompile(`„[^“”"\n]*[“”"]|“[^”\n]*”|"[^"\n]*"`)
)

// PrerequisiteIDs returns the module IDs a prerequisite text asks for: ModuleIDs without
// those the text only names to rule them out („Keine erfolgreiche Teilnahme am zugehörigen
// Auslaufmodul 12212 - Digitaltechnik.", "No successful participation in module 14478",
// „Keine Doppelbelegung mit Modul 13966"). Only a phrase that states such an exclusion
// counts, never a bare „nicht": „wünschenswert, aber nicht zwingend erforderlich", „Ein
// Abschluss ist für die Anmeldung zu Modul 12345 nicht erforderlich", „(nicht mehr im Angebot
// ab SS 2011)" and "cannot be successfully completed without knowledge of" keep their modules.
// A module named once outside an exclusion stays, so a prerequisite too many is more likely
// than one missing (owner, 2026-10-16: „False positive bei den voraussetzungen ist aber besser
// als false negative").
func PrerequisiteIDs(text string) []string {
	var ids []string
	seen := make(map[string]bool)
	for _, t := range prerequisiteTokens(text) {
		if t.kept && !seen[t.id] {
			seen[t.id] = true
			ids = append(ids, t.id)
		}
	}
	return ids
}

// idToken is a module number of a prerequisite text, where it stands, and whether the text asks
// for it (kept) or only names it to rule it out.
type idToken struct {
	id       string
	from, to int
	kept     bool
}

// prerequisiteTokens are the module numbers of a prerequisite text in order, each with whether
// PrerequisiteIDs keeps it.
func prerequisiteTokens(text string) []idToken {
	type span struct{ from, to int }
	var excluded []span
	exclude := func(from, to int) {
		if !statesANeed.MatchString(text[from:to]) {
			excluded = append(excluded, span{from, to})
		}
	}
	for _, m := range excludesAfter.FindAllStringIndex(text, -1) {
		exclude(m[0], statementEnd(text, m[1]))
	}
	for _, m := range excludesSentence.FindAllStringIndex(text, -1) {
		exclude(sentenceStart(text, m[0]), statementEnd(text, m[1]))
	}

	var tokens []idToken
	for _, m := range moduleIDToken.FindAllStringIndex(text, -1) {
		kept := true
		for _, s := range excluded {
			if m[0] >= s.from && m[0] < s.to {
				kept = false
				break
			}
		}
		tokens = append(tokens, idToken{id: text[m[0]:m[1]], from: m[0], to: m[1], kept: kept})
	}
	return tokens
}

// PrerequisiteChoices returns the modules of PrerequisiteIDs as choices: of the modules of one
// choice one is enough, and every choice is needed. „Erfolgreiche Modulprüfung von: Modul 12102
// Programmierpraktikum ODER Modul 11900 Programmierpraktikum (IMT) UND Modul 12104 Entwicklung
// von Softwaresystemen" (12202) is [[12102 11900] [12104]]; most texts name no alternatives,
// and each of their modules is a choice of its own.
//
// Two modules are alternatives where the text says so (joint has the details):
//   - an „oder", "or", „bzw." or „alternativ" is the last conjunction between them and joins the
//     two modules themselves („12102 Programmierpraktikum ODER" above „• Modul 11900", „25102 …
//     ODER 25201"); a conjunction within brackets or quotes is part of a title („Gebäudekunde und
//     Entwerfen (GK1-B)", „(Regional or International Scale)") and counts for nothing;
//   - such a word on a line of its own or at the start of one („ODER" between two paragraphs,
//     „bzw. des Moduls:"), when it stands between two single modules. Between two lists it
//     opposes the lists, not their nearest modules („11103 Analysis I … 11217
//     Wahrscheinlichkeitstheorie or very good knowledge of the content of the modules 11113 …",
//     12826), and so it does between pairs („12894 … and 12895 … or 11494 … and 11747", 13297):
//     no choice of single modules says that;
//   - the statement says that one of them is enough: „eines der Module", „einem der folgenden
//     Module", "at least one of the following modules".
//
// A module that would be an alternative in one place and needed on its own or among other
// alternatives in another makes every module of those choices one of its own: a prerequisite too
// many is better than one missing (owner, 2026-10-16), and a text that says the same twice, in
// German and in English, says it once.
func PrerequisiteChoices(text string) [][]string {
	tokens := prerequisiteTokens(text)
	n := len(tokens)
	if n == 0 {
		return nil
	}
	masked := maskTitles(text)

	joints := make([]joint, n-1)
	for i := 0; i+1 < n; i++ {
		joints[i] = conjunctionBetween(masked, tokens[i].to, tokens[i+1].from)
	}
	// Blocks: the runs of modules that a conjunction on a line of its own does not divide.
	block := make([]int, n)
	size := map[int]int{0: 1}
	for i := 1; i < n; i++ {
		block[i] = block[i-1]
		if joints[i-1].block {
			block[i]++
		}
		size[block[i]]++
	}

	parent := make([]int, n)
	for i := range parent {
		parent[i] = i
	}
	var find func(int) int
	find = func(i int) int {
		for parent[i] != i {
			parent[i] = parent[parent[i]]
			i = parent[i]
		}
		return i
	}
	link := func(a, b int) {
		if tokens[a].kept && tokens[b].kept {
			parent[find(b)] = find(a)
		}
	}
	tightAnd := func(i int) bool { return i >= 0 && i < len(joints) && joints[i].and }
	for i, j := range joints {
		switch {
		case !j.or:
		case j.block:
			if size[block[i]] == 1 && size[block[i+1]] == 1 {
				link(i, i+1)
			}
		// „12894 Regelungstechnik 1 and 12895 Regelungstechnik 2 or 11494 Control Engineering 1
		// and 11747 Control Engineering 2" (13297) opposes the pairs; which binds closer, a text
		// does not say.
		case !tightAnd(i-1) && !tightAnd(i+1):
			link(i, i+1)
		}
	}
	for _, m := range oneOf.FindAllStringIndex(text, -1) {
		end := statementEnd(text, m[1])
		first := -1
		for i, t := range tokens {
			if !t.kept || t.from < m[1] || t.from >= end {
				continue
			}
			if first < 0 {
				first = i
			} else {
				link(first, i)
			}
		}
	}

	// The choices, each once, in the order the text names them.
	var choices [][]string
	index := make(map[int]int)
	for i, t := range tokens {
		if !t.kept {
			continue
		}
		root := find(i)
		c, ok := index[root]
		if !ok {
			c = len(choices)
			index[root] = c
			choices = append(choices, nil)
		}
		if !slices.Contains(choices[c], t.id) {
			choices[c] = append(choices[c], t.id)
		}
	}
	key := func(choice []string) string {
		return strings.Join(slices.Sorted(slices.Values(choice)), " ")
	}
	var distinct [][]string
	seen := make(map[string]bool)
	for _, c := range choices {
		if k := key(c); !seen[k] {
			seen[k] = true
			distinct = append(distinct, c)
		}
	}
	in := make(map[string]int)
	for _, c := range distinct {
		for _, id := range c {
			in[id]++
		}
	}
	var out [][]string
	alone := make(map[string]bool)
	for _, c := range distinct {
		split := len(c) == 1
		for _, id := range c {
			split = split || in[id] > 1
		}
		if !split {
			out = append(out, c)
			continue
		}
		for _, id := range c {
			if !alone[id] {
				alone[id] = true
				out = append(out, []string{id})
			}
		}
	}
	return out
}

// joint is how a prerequisite text joins two modules it names one after the other, by the last
// conjunction between them.
type joint struct {
	// An „oder" that joins the two modules themselves: it ends the line of the first, with the
	// second on the next line („12102 Programmierpraktikum ODER" above „• Modul 11900"), or the
	// second follows it on its line with no comma or full stop between („25102 … ODER 25201").
	// Not one that goes on with something else („Regelungstechnik 1 (Modul 12894) oder
	// Grundlagen der Regelungstechnik oder vergleichbare Module", 12895; „sollten vorher oder
	// zeitlich parallel belegt werden. Ein Abschluss ist aber für die Anmeldung zu Modul 23506
	// nicht erforderlich").
	or bool
	// An „und" between two modules of one line, right before the second („12894 Regelungstechnik
	// 1 and 12895").
	and bool
	// The conjunction stands on a line of its own or at the start of one, with the second module
	// on a later line („ODER" between two paragraphs, „bzw. des Moduls:" above „• 12156"). One
	// that the second module follows on its line goes on with the line before („11217:
	// Wahrscheinlichkeitstheorie" above „or 11917: Mathematik W-3 (Statistik)").
	block bool
}

// conjunctionBetween reads the last conjunction of masked[from:to], the text between two module
// numbers with the titles masked.
func conjunctionBetween(masked string, from, to int) (j joint) {
	all := conjunction.FindAllStringSubmatchIndex(masked[from:to], -1)
	if len(all) == 0 {
		return j
	}
	last := all[len(all)-1]
	at, end := from+last[0], from+last[1]
	rest := masked[end:to]
	line, after, broken := strings.Cut(rest, "\n")
	lineStart := strings.LastIndexByte(masked[:at], '\n') + 1
	j.block = broken && lineStart > from && lineLead.FindString(masked[lineStart:at]) == masked[lineStart:at]
	switch {
	case last[2] < 0:
		j.and = !strings.Contains(masked[from:to], "\n") && leadToModule.MatchString(rest)
	case j.block:
		j.or = true
	case broken:
		j.or = strings.Trim(line, " \t.,:;!?") == "" && !strings.Contains(after, "\n")
	default:
		j.or = !strings.ContainsAny(rest, ",;") && !statementStop.MatchString(rest)
	}
	return j
}

// maskTitles blanks what stands within brackets or quotes, the titles of modules and their
// short names, keeping every position and line break: „Bau- und Stadtbaugeschichte 1 (GT1-B)".
func maskTitles(text string) string {
	b := []byte(text)
	blank := func(loc []int) {
		for k := loc[0]; k < loc[1]; k++ {
			if b[k] != '\n' {
				b[k] = ' '
			}
		}
	}
	for {
		locs := bracketed.FindAllIndex(b, -1)
		if len(locs) == 0 {
			break
		}
		for _, loc := range locs {
			blank(loc)
		}
	}
	for _, loc := range quoted.FindAllIndex(b, -1) {
		blank(loc)
	}
	return string(b)
}

// statementEnd is where the statement that runs through i ends: at a statementStop, or at the
// end of a line unless the next one goes on with it, as the items of a list it opens do
// („Keine erfolgreiche Teilnahme an den Modulen:" and „• 11107 …", „• 11116 …").
func statementEnd(text string, i int) int {
	for {
		lineEnd := len(text)
		if n := strings.IndexByte(text[i:], '\n'); n >= 0 {
			lineEnd = i + n
		}
		if loc := statementStop.FindStringIndex(text[i:lineEnd]); loc != nil {
			return i + loc[0]
		}
		if lineEnd == len(text) || !continuesStatement(text[lineEnd+1:]) {
			return lineEnd
		}
		i = lineEnd + 1
	}
}

// continuesStatement reports whether the line at the start of rest goes on with the statement
// of the line before it. A list item does when it names a module first; any other line also
// when it starts in lower case.
func continuesStatement(rest string) bool {
	line, _, _ := strings.Cut(rest, "\n")
	item := strings.HasPrefix(strings.TrimSpace(line), "•")
	line = lineLead.ReplaceAllString(line, "")
	return listedModule.MatchString(line) || (!item && lowerCaseStart.MatchString(line))
}

// sentenceStart is where the sentence that holds i begins: after the last statementStop
// before it on its line, or at the line's start.
func sentenceStart(text string, i int) int {
	lineStart := strings.LastIndexByte(text[:i], '\n') + 1
	start := lineStart
	for _, loc := range statementStop.FindAllStringIndex(text[lineStart:i], -1) {
		start = lineStart + loc[1]
	}
	return start
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
