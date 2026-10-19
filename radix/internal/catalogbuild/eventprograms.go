package catalogbuild

import (
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/leonieziechmann/betula/radix/internal/model"
	"github.com/leonieziechmann/betula/radix/internal/normalize"
)

// writeEventPrograms stores whom each event is for, as its page lists it under „Studiengänge"
// (model.EventProgram), and resolves every row to the programs of the catalog it names
// (docs/radix/schema-v2.md, „Tracks"). A row that names no program of the catalog is kept as
// 'unresolved' and reported, never guessed.
func (b *builder) writeEventPrograms() error {
	names := newProgramNames(b.programs)
	ids := make([]string, 0, len(b.src.events))
	for id := range b.src.events {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		if err := insertEventPrograms(b.tx, names, id, b.src.events[id].detail.Programs, b.report); err != nil {
			return err
		}
	}
	return nil
}

// insertEventPrograms writes the programs one event page lists, resolved.
func insertEventPrograms(q querier, names *programNames, id string, programs []model.EventProgram, report *Report) error {
	for i, row := range programs {
		from, to := semesterRange(row.Semesters)
		resolved := names.resolve(row)
		status := "unresolved"
		if len(resolved) > 0 {
			status = "resolved"
			report.EventProgramsResolved++
		} else {
			report.EventProgramsUnresolved[row.Label]++
		}
		if _, err := q.Exec(`
			INSERT INTO event_program (event_id, ord, label, semester_min, semester_max, po_raw, remark, qis_id, resolve_status)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			id, i+1, row.Label, null(from), null(to), null(row.PO), null(row.Remark), null(row.QISID), status); err != nil {
			return err
		}
		for _, p := range resolved {
			if _, err := q.Exec("INSERT INTO event_program_ref (event_id, ord, program_id) VALUES (?, ?, ?)", id, i+1, p.id); err != nil {
				return err
			}
		}
	}
	return nil
}

var semesterSpan = regexp.MustCompile(`^\s*(\d{1,2})\s*(?:-\s*(\d{1,2}))?\s*$`)

// semesterRange reads „1 - 3" (and „5"); 0 for what it does not state.
func semesterRange(raw string) (from, to int) {
	m := semesterSpan.FindStringSubmatch(raw)
	if m == nil {
		return 0, 0
	}
	from, _ = strconv.Atoi(m[1])
	to = from
	if m[2] != "" {
		to, _ = strconv.Atoi(m[2])
	}
	if to < from {
		return 0, 0
	}
	return from, to
}

// programNames finds the programs an event page names. A page writes the program's name and
// then, in its own words, the degree, the profile, the form of study and a study direction:
// „Informatik Bachelor (universitäres Profil)", „Elektrotechnik - MIT - Bachelor",
// „Medizintechnik - dual: EM - Bachelor - A2", „Bauingenieurwesen Bachelor erweiterte
// Fachsemester", „Energietechnik und Energiewirtschaft EET". The name decides, the longest
// program name the label begins with; what the label states of the degree narrows it down;
// what it does not state leaves every program of that name. A study direction is no program of
// its own in the catalog and names its program.
type programNames struct {
	byName map[string][]*program // folded name without „- dual" → programs
	names  []string              // the keys of byName, longest first
}

func newProgramNames(programs []*program) *programNames {
	n := &programNames{byName: make(map[string][]*program)}
	for _, p := range programs {
		name := baseName(p.name)
		if name == "" {
			continue
		}
		if n.byName[name] == nil {
			n.names = append(n.names, name)
		}
		n.byName[name] = append(n.byName[name], p)
	}
	sort.Slice(n.names, func(i, j int) bool {
		if len(n.names[i]) != len(n.names[j]) {
			return len(n.names[i]) > len(n.names[j])
		}
		return n.names[i] < n.names[j]
	})
	return n
}

// baseName is a program's name as an event page begins it: folded, and without the „- dual"
// the catalog's name of a dual program carries (the page says „dual" after the name, or as
// „Medizintechnik - dual: EM").
func baseName(name string) string {
	folded := foldName(name)
	folded = strings.TrimSuffix(folded, " - dual")
	return strings.TrimSpace(folded)
}

var (
	labelWord = regexp.MustCompile(`[\p{L}\p{N}]+`)
	poOfLabel = regexp.MustCompile(`\b(?:19|20)\d{2}\b`)
)

// degreeBetween is a degree a label writes between a name's parts: „Lehramt Primarstufe Bachelor -
// Deutsch-Englisch" for the program „Lehramt Primarstufe Deutsch-Englisch".
var degreeBetween = regexp.MustCompile(` (?:bachelor|master)(?: -)? `)

// resolve gives the programs a row names, nil for none.
func (n *programNames) resolve(row model.EventProgram) []*program {
	label := foldName(row.Label)
	name, rest := n.name(label)
	if name == "" {
		name, _ = n.name(degreeBetween.ReplaceAllString(label, " "))
		rest = label
	}
	if name == "" {
		return nil
	}
	words := make(map[string]bool)
	for _, w := range labelWord.FindAllString(rest, -1) {
		words[w] = true
	}

	var levels []string
	switch {
	case words["bachelor"]:
		levels = []string{normalize.LevelBachelor, normalize.LevelTeachingBachelor}
	case words["master"]:
		levels = []string{normalize.LevelMaster, normalize.LevelTeachingMaster}
	case words["promotion"] || words["doktorand"] || words["doktoranden"]:
		levels = []string{normalize.LevelDoctoral}
	}
	var kind string
	switch {
	case strings.Contains(rest, "universitär"):
		kind = normalize.TypeUniversity
	case strings.Contains(rest, "fachhochschul") || strings.Contains(rest, "anwendungs"):
		kind = normalize.TypeApplied
	}
	variants := map[string]bool{"": true}
	switch {
	case words["dual"] || words["duales"]:
		variants = map[string]bool{normalize.VariantDualPractice: true, normalize.VariantDualTraining: true}
		switch {
		case strings.Contains(rest, "ausbildungsintegrierend") || words["a2"]:
			variants = map[string]bool{normalize.VariantDualTraining: true}
		case strings.Contains(rest, "praxisintegrierend") || words["p2"]:
			variants = map[string]bool{normalize.VariantDualPractice: true}
		}
	case strings.Contains(rest, "erweitert"):
		variants = map[string]bool{normalize.VariantExtended: true}
	case strings.Contains(rest, "verringert"):
		variants = map[string]bool{normalize.VariantReduced: true}
	case strings.Contains(rest, "teilzeit"):
		variants = map[string]bool{normalize.VariantPartTime: true}
	case strings.Contains(rest, "fernstudium"):
		variants = map[string]bool{normalize.VariantDistance: true}
	case strings.Contains(rest, "doppelabschluss"):
		variants = map[string]bool{normalize.VariantDoubleDegree: true}
	}

	var found, other []*program
	for _, p := range n.byName[name] {
		if len(levels) > 0 && !contains(levels, p.degree.Level) {
			continue
		}
		if kind != "" && p.degree.Type != "" && p.degree.Type != kind {
			continue
		}
		if !variants[p.degree.Variant] {
			other = append(other, p)
			continue
		}
		found = append(found, p)
	}
	// A label that names no form of study names the program where the catalog has it in another
	// form only (Hebammenwissenschaft is a dual program, and QIS also lists it without „dual").
	if len(found) == 0 && len(variants) == 1 && variants[""] {
		found = other
	}
	// A PO the row names narrows them down to its programs, where the catalog has them; one it
	// does not have names them all („2025" for a program whose regulation of 2025 is not out yet).
	if year, _ := strconv.Atoi(poOfLabel.FindString(row.PO)); year != 0 {
		var of []*program
		for _, p := range found {
			if p.poYear == year {
				of = append(of, p)
			}
		}
		if len(of) > 0 {
			found = of
		}
	}
	return found
}

// name is the longest program name the label begins with, and what follows it; "" for none.
func (n *programNames) name(label string) (name, rest string) {
	for _, candidate := range n.names {
		if rest, ok := strings.CutPrefix(label, candidate); ok && (rest == "" || !isWordRune(rest)) {
			return candidate, rest
		}
	}
	return "", ""
}

// isWordRune: the label goes on in the middle of a word („Informatik" is not the start of
// „Informatikdidaktik").
func isWordRune(rest string) bool {
	for _, r := range rest {
		return labelWord.MatchString(string(r))
	}
	return false
}
