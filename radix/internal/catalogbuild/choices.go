package catalogbuild

import (
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

// The events of a module of which a student attends some, not all (folia/design/wahlangebote/
// konzept.html; owner, 2026-10-19: „ich will nicht, dass wir sonderfälle per hand eintragen,
// sondern eine lösung haben, die in die zukunft funktioniert … Bitte bilde alles außer daten passen
// nicht zusammen in der DB und dem UI ab“, then „Passt so, nimm deine Vorschläge und bau es“):
// extras beside the compulsory part, and choices among events — one content more than once
// (for other programs, or in groups) or different contents to choose from. Read from the SWS of
// the module and of its events, their forms, their titles and whom QIS lists them for; no module
// is named anywhere. Whatever the rules do not decide stays compulsory, as before: a choice read
// wrongly hides a date a student needs, one missed only shows every date. The rules were tried on
// canary's snapshot of 2026-10-19 against a review by hand (research/wahlangebote): of the
// choice catalogues 68 of 76, of the same event more than once 18 of 19, of the extras 3 of 4,
// and none of the 16 modules whose data do not fit.

// choiceShare is how much of what a choice covers one option must hold to be one; a choice of
// one form takes options down to half of it (English for Academic Purposes: courses of 4 and 2
// SWS for 6).
const choiceShare = 0.75

// contactForms are the forms of a module's description that are time in a room.
var contactForms = map[string]bool{"lecture": true, "exercise": true, "practical": true, "seminar": true,
	"project": true, "tutorial": true, "consultation": true, "excursion": true, "other": true}

var (
	// groupMark is a group a title names: „(Gruppe 2)“, „2. Gruppe“, „(A-Kurs)“.
	groupMark = regexp.MustCompile(`(?i)\s*[\(\[]?\s*(?:\bGruppe\s*\d+\b|\b\d+\.\s*Gruppe\b|\b[A-D]-Kurs\b)\s*[\)\]]?`)
	groupNum  = regexp.MustCompile(`(?i)\bGruppe\s*(\d+)\b|\b(\d+)\.\s*Gruppe\b|\b([A-D])-Kurs\b`)
	// extraWord is a title that names a support format.
	extraWord = regexp.MustCompile(`(?i)Tutori|Aufbaukurs|Sprechstunde|Fragestunde|Repetitori|Vorkurs|Brückenkurs`)
	// kindWord is a form a title begins with: „Seminar Town and House“.
	kindWord   = regexp.MustCompile(`(?i)^\s*(?:Vorlesung|Übung|Seminar|Praktikum|Projekt|Tutorium|Laborpraktikum|Proseminar|Oberseminar)\s*[:\-–]?\s*`)
	choiceWord = regexp.MustCompile(`(?i)[a-zäöüß0-9]+`)
	spaces     = regexp.MustCompile(`[\s\p{Zs}]+`)
	// partWord is a part's number: titles that differ only by one are parts of the module.
	partWord = regexp.MustCompile(`^(?:t\d+|teil|part|i|ii|iii|iv|v)$`)
)

// markerWords say for whom or in which variant an event is held, not what it is.
var markerWords = map[string]bool{"für": true, "for": true, "und": true, "and": true, "the": true, "der": true,
	"die": true, "das": true, "den": true, "online": true, "präsenz": true, "gruppe": true, "group": true,
	"kurs": true, "a": true, "b": true, "c": true, "d": true}

// formSet is the forms of an event (event_form), or the union of a unit's.
type formSet map[string]bool

func (f formSet) key() string {
	return strings.Join(sortedKeys(f), ",")
}

func (f formSet) meets(g formSet) bool {
	for form := range f {
		if g[form] {
			return true
		}
	}
	return false
}

// choiceEvent is a teaching event of a module in one semester as the choices are read from it.
type choiceEvent struct {
	id, title string
	sws       float64 // what its page states; 0 when it states nothing
	forms     formSet
	course    string // the campus of the module's course it is in; "" for none
}

// choiceUnit is what a choice offers: an event, or the events of one title in different forms.
type choiceUnit struct {
	events []*choiceEvent
	title  string
	sws    float64
	forms  formSet
	label  string // „Gruppe 2“
	order  int
}

// moduleChoice is one choice: of its options a student takes pick.
type moduleChoice struct {
	kind    string  // program, group, topic
	basis   string  // named, module, form
	sws     float64 // what the options cover; 0 for named groups
	pick    int
	options []*choiceUnit
	course  string
}

// moduleExtra is an event beside the compulsory part.
type moduleExtra struct {
	event *choiceEvent
	basis string // tutorial, title
}

// writeChoices finds the choices and extras of every module in every semester, in each course
// at a campus of its own (module_track) apart, and writes them (module_choice,
// module_choice_option, module_extra). It needs the tracks and whom QIS lists the events for.
func (b *builder) writeChoices() error {
	return writeChoices(b.tx, b.report)
}

func writeChoices(q querier, report *Report) error {
	titles, err := moduleTitles(q)
	if err != nil {
		return err
	}
	stated, err := moduleFormSWS(q)
	if err != nil {
		return err
	}
	audiences, err := eventAudiences(q)
	if err != nil {
		return err
	}
	events, keys, err := choiceEvents(q)
	if err != nil {
		return err
	}
	for _, key := range keys {
		evs := events[key]
		courses := make(map[string]bool)
		for _, e := range evs {
			if e.course != "" {
				courses[e.course] = true
			}
		}
		scopes := [][]*choiceEvent{evs}
		if len(courses) > 0 {
			scopes = nil
			for _, course := range sortedKeys(courses) {
				var scope []*choiceEvent
				for _, e := range evs {
					if e.course == course || e.course == "" {
						scope = append(scope, e)
					}
				}
				scopes = append(scopes, scope)
			}
		}
		// The first course claims what its courses share: an event is in one choice, or one extra.
		taken := make(map[string]bool)
		number := 0
		for _, scope := range scopes {
			extras, choices := choicesOf(titles[key.module], stated[key.module], scope, audiences)
			for _, x := range extras {
				if taken[x.event.id] {
					continue
				}
				taken[x.event.id] = true
				if _, err := q.Exec("INSERT INTO module_extra (module_id, event_id, semester_key, basis) VALUES (?, ?, ?, ?)",
					key.module, x.event.id, key.semester, x.basis); err != nil {
					return err
				}
				report.ChoiceExtras++
			}
		next:
			for _, c := range choices {
				for _, o := range c.options {
					for _, e := range o.events {
						if taken[e.id] {
							continue next
						}
					}
				}
				number++
				var sws any
				if c.sws > 0 {
					sws = c.sws
				}
				var course any
				if c.course != "" {
					course = c.course
				}
				if _, err := q.Exec("INSERT INTO module_choice (module_id, semester_key, choice, course, kind, basis, sws, pick) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
					key.module, key.semester, number, course, c.kind, c.basis, sws, c.pick); err != nil {
					return err
				}
				for i, o := range c.options {
					var label any
					if o.label != "" {
						label = o.label
					}
					for _, e := range o.events {
						taken[e.id] = true
						if _, err := q.Exec("INSERT INTO module_choice_option (module_id, semester_key, choice, option, event_id, label) VALUES (?, ?, ?, ?, ?, ?)",
							key.module, key.semester, number, i+1, e.id, label); err != nil {
							return err
						}
					}
				}
				switch c.kind {
				case "program":
					report.ChoicesByProgram++
				case "group":
					report.ChoicesInGroups++
				default:
					report.ChoicesOfTopics++
				}
			}
		}
		if number > 0 {
			report.ModulesWithChoices++
		}
	}
	return nil
}

// choicesOf reads one scope of a module's teaching in a semester: its extras, then its choices —
// groups a title names, a catalogue of whole units, choices within one form — in that order; what
// a step places, the next does not look at. title is the module's, sws what its description states
// per form of contact.
func choicesOf(title string, sws map[string]float64, events []*choiceEvent, audiences map[string]map[string]bool) ([]moduleExtra, []*moduleChoice) {
	stated := make(formSet)
	var total float64
	for form, v := range sws {
		if v > 0 {
			stated[form] = true
			total += v
		}
	}
	if total <= 0 {
		return nil, nil
	}

	// 1. Extras: a tutorial the module states no SWS for, or a title that names a support format —
	// where the rest still covers the module's SWS of that form: what is needed to reach them is
	// never an extra.
	var extras []moduleExtra
	var rest []*choiceEvent
	for _, e := range events {
		named := extraWord.MatchString(e.title)
		tutorial := e.forms["tutorial"] && sws["tutorial"] <= 0
		if named || tutorial {
			others := make(formSet)
			for form := range e.forms {
				if form != "tutorial" {
					others[form] = true
				}
			}
			var need, held float64
			for form := range others {
				need += sws[form]
			}
			for _, o := range events {
				if o != e && o.forms.meets(others) && !extraWord.MatchString(o.title) {
					held += o.sws
				}
			}
			if need <= 0 || held >= choiceShare*need {
				basis := "title"
				if tutorial {
					basis = "tutorial"
				}
				extras = append(extras, moduleExtra{event: e, basis: basis})
				continue
			}
		}
		rest = append(rest, e)
	}

	// 2. Groups a title names: titles alike but for „Gruppe 2“ or „A-Kurs“, one of them. Groups of
	// the same numbers in one module belong together (Kontrapunkt/Tonsatz I and Gehörbildung I, the
	// one right after the other in each group).
	var choices []*moduleChoice
	used := make(map[string]bool)
	byStem := make(map[string][]*choiceEvent)
	var stems []string
	for _, e := range rest {
		if !groupMark.MatchString(e.title) {
			continue
		}
		s := choiceStem(e.title)
		if _, ok := byStem[s]; !ok {
			stems = append(stems, s)
		}
		byStem[s] = append(byStem[s], e)
	}
	sort.Strings(stems)
	var named []*moduleChoice
	for _, s := range stems {
		es := byStem[s]
		if len(es) < 2 {
			continue
		}
		c := &moduleChoice{kind: "group", basis: "named", pick: 1}
		for _, e := range es {
			label, order := groupLabel(e.title)
			c.options = append(c.options, &choiceUnit{events: []*choiceEvent{e}, title: e.title, sws: e.sws, forms: e.forms, label: label, order: order})
			used[e.id] = true
		}
		sort.SliceStable(c.options, func(i, j int) bool { return c.options[i].order < c.options[j].order })
		named = append(named, c)
	}
	choices = append(choices, linkGroups(named)...)

	units := choiceUnits(rest, used)

	// 3. A catalogue of whole units: each holds about the module's SWS (Berufspraktische
	// Profilierung: 14 projects of 4 SWS for 4; Basis-Deutsch: courses of 4 SWS for 8, two of them).
	var sized []*choiceUnit
	for _, u := range units {
		if u.sws > 0 {
			sized = append(sized, u)
		}
	}
	if len(sized) >= 2 {
		whole := true
		for _, u := range sized {
			if u.sws < choiceShare*total {
				whole = false
			}
		}
		sizes := unitSizes(sized)
		if whole && similarSizes(sizes) && surplusOf(sizes, total) && formsOverlap(sized, stated) && !partsOfModule(title, sized) {
			pick := pickOf(total, sizes)
			kinds := make(map[string]bool)
			for _, u := range sized {
				kinds[u.forms.key()] = true
			}
			options := append([]*choiceUnit(nil), sized...)
			for _, u := range units {
				if u.sws <= 0 && kinds[u.forms.key()] {
					options = append(options, u)
				}
			}
			sortUnits(options)
			if len(options) > pick {
				choices = append(choices, &moduleChoice{kind: classifyChoice(options, audiences), basis: "module", sws: total, pick: pick, options: options})
				return extras, withCourse(choices)
			}
		}
	}

	// 4. A choice within one form: its units hold more than the module states for it, by at least
	// one unit, each at least half its share (Höhere Mathematik T3: three exercises of 2 SWS for 2).
	// Where no unit is of a form the module states (QIS's „Projekt“, the module's „Seminar“), the
	// module's SWS stand in for the form's.
	mismatch := true
	for _, u := range units {
		if u.forms.meets(stated) {
			mismatch = false
		}
	}
	byKind := make(map[string][]*choiceUnit)
	var kinds []string
	for _, u := range units {
		k := u.forms.key()
		if _, ok := byKind[k]; !ok {
			kinds = append(kinds, k)
		}
		byKind[k] = append(byKind[k], u)
	}
	for _, k := range kinds {
		us := byKind[k]
		var need float64
		for form := range us[0].forms {
			need += sws[form]
		}
		if need <= 0 && mismatch {
			need = total
		}
		var sz []*choiceUnit
		for _, u := range us {
			if u.sws > 0 {
				sz = append(sz, u)
			}
		}
		if need <= 0 || len(sz) < 2 {
			continue
		}
		sizes := unitSizes(sz)
		if !surplusOf(sizes, need) || !similarSizes(sizes) {
			continue
		}
		pick := pickOf(need, sizes)
		if len(us) <= pick {
			continue
		}
		fits := true
		for _, x := range sizes {
			if x < 0.5*need/float64(pick) || x > 1.25*need {
				fits = false
			}
		}
		if !fits || partsOfModule(title, sz) {
			continue
		}
		choices = append(choices, &moduleChoice{kind: classifyChoice(us, audiences), basis: "form", sws: need, pick: pick, options: us})
	}
	return extras, withCourse(choices)
}

// withCourse names the course a choice is in: the one of its events in one.
func withCourse(choices []*moduleChoice) []*moduleChoice {
	for _, c := range choices {
		for _, o := range c.options {
			for _, e := range o.events {
				if e.course != "" {
					c.course = e.course
				}
			}
		}
	}
	return choices
}

// linkGroups makes one choice of the named groups that share their numbers: an option per group,
// with the events of every title in it.
func linkGroups(named []*moduleChoice) []*moduleChoice {
	by := make(map[string][]*moduleChoice)
	var order []string
	for _, c := range named {
		labels := make([]string, 0, len(c.options))
		seen := make(map[string]bool)
		distinct := true
		for _, o := range c.options {
			if seen[o.label] {
				distinct = false
			}
			seen[o.label] = true
			labels = append(labels, o.label)
		}
		key := strings.Join(labels, "|")
		if !distinct {
			key = "\x00" + strconv.Itoa(len(order))
		}
		if _, ok := by[key]; !ok {
			order = append(order, key)
		}
		by[key] = append(by[key], c)
	}
	var out []*moduleChoice
	for _, key := range order {
		cs := by[key]
		if len(cs) == 1 {
			out = append(out, cs[0])
			continue
		}
		linked := &moduleChoice{kind: "group", basis: "named", pick: 1}
		for i, o := range cs[0].options {
			unit := &choiceUnit{title: o.title, label: o.label, order: o.order, forms: make(formSet)}
			for _, c := range cs {
				other := c.options[i]
				unit.events = append(unit.events, other.events...)
				unit.sws += other.sws
				for form := range other.forms {
					unit.forms[form] = true
				}
			}
			linked.options = append(linked.options, unit)
		}
		out = append(out, linked)
	}
	return out
}

// choiceUnits are the events not yet placed, each a unit of its own but where one title is held
// in different forms (a lecture and its seminar, „Schauplätze …“): those are one. A title held
// more than once in one form is the module's own course, each event apart (Höhere Mathematik T3:
// the lecture and three exercises of that title).
func choiceUnits(events []*choiceEvent, used map[string]bool) []*choiceUnit {
	by := make(map[string][]*choiceEvent)
	var stems []string
	for _, e := range events {
		if used[e.id] {
			continue
		}
		s := choiceStem(e.title)
		if _, ok := by[s]; !ok {
			stems = append(stems, s)
		}
		by[s] = append(by[s], e)
	}
	var units []*choiceUnit
	for _, s := range stems {
		es := by[s]
		kinds := make(map[string]bool)
		for _, e := range es {
			kinds[e.forms.key()] = true
		}
		if len(es) > 1 && len(kinds) == len(es) {
			u := &choiceUnit{events: es, title: es[0].title, forms: make(formSet)}
			for _, e := range es {
				u.sws += e.sws
				for form := range e.forms {
					u.forms[form] = true
				}
			}
			units = append(units, u)
			continue
		}
		for _, e := range es {
			units = append(units, &choiceUnit{events: []*choiceEvent{e}, title: e.title, sws: e.sws, forms: e.forms})
		}
	}
	return units
}

// classifyChoice says what a choice offers: one content more than once (titles alike but for a
// group, a variant, an audience), for other programs where QIS names others at its options
// ('program') or in groups ('group'); else different contents ('topic').
func classifyChoice(options []*choiceUnit, audiences map[string]map[string]bool) string {
	if !alikeTitles(options) {
		return "topic"
	}
	seen := make(map[string]bool)
	for _, o := range options {
		who := make(map[string]bool)
		for _, e := range o.events {
			for key := range audiences[e.id] {
				who[key] = true
			}
		}
		if len(who) == 0 {
			return "group"
		}
		seen[strings.Join(sortedKeys(who), "\x00")] = true
	}
	if len(seen) > 1 {
		return "program"
	}
	return "group"
}

// alikeTitles says whether every two options' titles share at least 60 % of their words, leaving
// out what says for whom or in which variant an event is held.
func alikeTitles(options []*choiceUnit) bool {
	ws := make([]map[string]bool, len(options))
	for i, o := range options {
		ws[i] = make(map[string]bool)
		for w := range titleWords(o.title) {
			if !markerWords[w] {
				ws[i][w] = true
			}
		}
	}
	for i := range ws {
		for j := i + 1; j < len(ws); j++ {
			a, b := ws[i], ws[j]
			if len(a) == 0 || len(b) == 0 {
				return false
			}
			both, either := 0, len(b)
			for w := range a {
				if b[w] {
					both++
				} else {
					either++
				}
			}
			if float64(both)/float64(either) < 0.6 {
				return false
			}
		}
	}
	return true
}

// partsOfModule says whether options are parts of the module rather than a choice: their titles
// differ only by a part's number (T1, Teil 2, II), or the module's title names each apart
// („Werkstoff- und Physiklabor“ over „… Werkstofflabor“ and „… Physiklabor“).
func partsOfModule(title string, options []*choiceUnit) bool {
	words := make([]map[string]bool, len(options))
	for i, o := range options {
		words[i] = titleWords(o.title)
	}
	for i := range words {
		for j := i + 1; j < len(words); j++ {
			diff := 0
			parts := true
			for w := range words[i] {
				if !words[j][w] {
					diff++
					parts = parts && partWord.MatchString(w)
				}
			}
			for w := range words[j] {
				if !words[i][w] {
					diff++
					parts = parts && partWord.MatchString(w)
				}
			}
			if diff > 0 && parts {
				return true
			}
		}
	}
	var module []string
	for _, w := range choiceWord.FindAllString(title, -1) {
		module = append(module, strings.ToLower(w))
	}
	all := make([]map[string]bool, len(options))
	for i, o := range options {
		all[i] = make(map[string]bool)
		for _, w := range choiceWord.FindAllString(o.title, -1) {
			all[i][strings.ToLower(w)] = true
		}
	}
	named := func(w string) bool {
		if utf8.RuneCountInString(w) < 4 {
			return false
		}
		for _, m := range module {
			if utf8.RuneCountInString(m) >= 4 && (strings.HasPrefix(w, m) || strings.HasPrefix(m, w)) {
				return true
			}
		}
		return false
	}
	for i := range all {
		found := false
		own := 0
		for w := range all[i] {
			common := true
			for j := range all {
				if !all[j][w] {
					common = false
					break
				}
			}
			if common {
				continue
			}
			own++
			if named(w) {
				found = true
			}
		}
		if own == 0 || !found {
			return false
		}
	}
	return true
}

// formsOverlap says whether every two options share a form the module states — or are all of one
// form it does not state (QIS's „Projekt“ where the module says „Seminar“).
func formsOverlap(options []*choiceUnit, stated formSet) bool {
	keys := make(map[string]bool)
	none := true
	for _, o := range options {
		keys[o.forms.key()] = true
		if o.forms.meets(stated) {
			none = false
		}
	}
	if none && len(keys) == 1 {
		return true
	}
	for i := range options {
		for j := i + 1; j < len(options); j++ {
			shared := false
			for form := range options[i].forms {
				if options[j].forms[form] && stated[form] {
					shared = true
				}
			}
			if !shared {
				return false
			}
		}
	}
	return true
}

// surplusOf says whether options hold more than need: at least a quarter more, and at least one
// option more than need.
func surplusOf(sizes []float64, need float64) bool {
	var sum float64
	for _, x := range sizes {
		sum += x
	}
	return sum >= 1.25*need && sum >= need+choiceShare*median(sizes)
}

// similarSizes says whether the options are about one size: the largest at most twice the least.
func similarSizes(sizes []float64) bool {
	if len(sizes) == 0 {
		return false
	}
	least, most := sizes[0], sizes[0]
	for _, x := range sizes {
		least, most = math.Min(least, x), math.Max(most, x)
	}
	return most <= 2*least
}

// pickOf is how many options of the sizes cover need.
func pickOf(need float64, sizes []float64) int {
	return max(1, int(math.RoundToEven(need/median(sizes))))
}

func unitSizes(units []*choiceUnit) []float64 {
	sizes := make([]float64, len(units))
	for i, u := range units {
		sizes[i] = u.sws
	}
	return sizes
}

func median(xs []float64) float64 {
	s := append([]float64(nil), xs...)
	sort.Float64s(s)
	n := len(s)
	if n == 0 {
		return 0
	}
	if n%2 == 1 {
		return s[n/2]
	}
	return (s[n/2-1] + s[n/2]) / 2
}

// sortUnits puts the options in the order of their first event.
func sortUnits(units []*choiceUnit) {
	sort.SliceStable(units, func(i, j int) bool { return units[i].events[0].id < units[j].events[0].id })
}

// choiceStem is a title without its group, the form it begins with and the case: what events of
// one content share.
func choiceStem(title string) string {
	t := groupMark.ReplaceAllString(title, " ")
	t = kindWord.ReplaceAllString(t, "")
	t = spaces.ReplaceAllString(t, " ")
	return strings.ToLower(strings.Trim(t, " -–:/"))
}

// titleWords are a title's words, lower case, without the form it begins with.
func titleWords(title string) map[string]bool {
	words := make(map[string]bool)
	for _, w := range choiceWord.FindAllString(kindWord.ReplaceAllString(title, ""), -1) {
		words[strings.ToLower(w)] = true
	}
	return words
}

// groupLabel is the group a title names, „Gruppe 2“ or „A-Kurs“, and its place among the groups.
func groupLabel(title string) (string, int) {
	m := groupNum.FindStringSubmatch(title)
	switch {
	case m == nil:
		return "", 0
	case m[1] != "" || m[2] != "":
		n := m[1] + m[2]
		order, _ := strconv.Atoi(n)
		return "Gruppe " + n, order
	default:
		letter := strings.ToUpper(m[3])
		return letter + "-Kurs", int(letter[0]-'A') + 1
	}
}

// moduleTitles is every module's title.
func moduleTitles(q querier) (map[string]string, error) {
	rows, err := q.Query("SELECT id, IFNULL(title, '') FROM module")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	titles := make(map[string]string)
	for rows.Next() {
		var id, title string
		if err := rows.Scan(&id, &title); err != nil {
			return nil, err
		}
		titles[id] = title
	}
	return titles, rows.Err()
}

// moduleFormSWS is what every module's description states per form of contact.
func moduleFormSWS(q querier) (map[string]map[string]float64, error) {
	rows, err := q.Query("SELECT module_id, form, SUM(sws) FROM module_teaching_form WHERE sws > 0 GROUP BY module_id, form")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := make(map[string]map[string]float64)
	for rows.Next() {
		var id, form string
		var sws float64
		if err := rows.Scan(&id, &form, &sws); err != nil {
			return nil, err
		}
		if !contactForms[form] {
			continue
		}
		if out[id] == nil {
			out[id] = make(map[string]float64)
		}
		out[id][form] += sws
	}
	return out, rows.Err()
}

// choiceEvents are the teaching events of every module per semester, in the order of their ids,
// with their forms and the course of the module they are in.
func choiceEvents(q querier) (map[moduleSemester][]*choiceEvent, []moduleSemester, error) {
	forms := make(map[string]formSet)
	rows, err := q.Query("SELECT event_id, form FROM event_form")
	if err != nil {
		return nil, nil, err
	}
	for rows.Next() {
		var id, form string
		if err := rows.Scan(&id, &form); err != nil {
			rows.Close()
			return nil, nil, err
		}
		if forms[id] == nil {
			forms[id] = make(formSet)
		}
		forms[id][form] = true
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, nil, err
	}
	rows, err = q.Query(`
		SELECT me.module_id, e.semester_key, e.id, IFNULL(e.title, ''), IFNULL(e.sws, 0), IFNULL(mt.campus, '')
		FROM module_event me
		JOIN event e ON e.id = me.event_id AND e.category = 'teaching'
		LEFT JOIN module_track mt ON mt.module_id = me.module_id AND mt.event_id = e.id
		WHERE e.semester_key IS NOT NULL
		ORDER BY me.module_id, e.semester_key, e.id`)
	if err != nil {
		return nil, nil, err
	}
	defer rows.Close()
	events := make(map[moduleSemester][]*choiceEvent)
	var keys []moduleSemester
	for rows.Next() {
		var key moduleSemester
		e := &choiceEvent{}
		if err := rows.Scan(&key.module, &key.semester, &e.id, &e.title, &e.sws, &e.course); err != nil {
			return nil, nil, err
		}
		e.forms = forms[e.id]
		if len(e.forms) == 0 {
			e.forms = formSet{"other": true}
		}
		if _, ok := events[key]; !ok {
			keys = append(keys, key)
		}
		events[key] = append(events[key], e)
	}
	return events, keys, rows.Err()
}
