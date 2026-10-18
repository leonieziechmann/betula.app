package catalogbuild

import (
	"database/sql"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/leonieziechmann/betula/radix/internal/normalize"
)

// Kinds from the plan's slots (docs/radix/schema-v2.md, „Kinds from the plan's slots"): a row of a
// validated plan that names no module but a Wahlpflicht slot („Anwendungsfach", „Wahl eines
// Moduls aus Anlage 2c") is filled from areas of the program's tree. A module there that no
// source gives a kind is one the student may choose for that slot: Wahlpflicht, inferred from
// the plan (source pdf_plan, basis inferred, precedence 4), so every stated kind still wins.
// Systemtheorie I in Informatik B.Sc. is the case (owner, 2026-10-18): its tree area
// „Komplex Nebenfach / Maschinenbau / Elektrotechnik" names no kind, its page states none for
// Informatik, and the plan fills it as „Anwendungsfach", Wahlpflicht.
//
// Which areas a row means is Folia's rule (folia plans::plan::areas_for_row), ported line by
// line, so that the modules Folia offers for a slot are the ones that are called Wahlpflicht:
// the words a name is known by (distinctive), how well two names fit (fit, akin), study
// directions, rows that list areas, and a name above the leaves („Anwendungsfach" for the
// areas below „Komplex Nebenfach"). testdata/slot_words.tsv holds both to the same words and
// scores. Only a row the plan marks Wahlpflicht counts: a Pflicht row names a slot as well
// (Physics M.Sc.'s „Minor Subject"), but says nothing about the modules it is filled from.

// slotArea is an area of a program's tree with modules, as Folia's CatalogArea has it.
type slotArea struct {
	id        int64
	label     string
	path      string
	ancestors []string // the labels above it, from the top down
	modules   int      // placed directly in it
	choice    bool     // a module in it is not known to be compulsory, the thesis or the internship
}

// slotRow is a row of a validated plan.
type slotRow struct {
	moduleID       string // "" for a row that names no module
	name           string
	kind           string
	specialization string
}

// slotStatement is a module a Wahlpflicht slot of its program's plan is filled from.
type slotStatement struct {
	program, module string
	row             string // the slot's name in the plan
}

type queryer interface {
	Query(query string, args ...any) (*sql.Rows, error)
}

// writeSlotKinds adds the inferred statements; it runs after every stated one is written.
func (b *builder) writeSlotKinds() error {
	statements, err := slotKinds(b.tx, nil)
	if err != nil {
		return err
	}
	for _, s := range statements {
		if err := b.insertAssertion(s.program, s.module, "pdf_plan", 0, s.row, kindElective, "inferred"); err != nil {
			return err
		}
	}
	return nil
}

// slotKinds finds the modules of the Wahlpflicht slots that no statement gives a kind, from
// the statements, the trees and the plans q holds (a build's transaction, or a snapshot).
// onRow, if not nil, sees each Wahlpflicht slot and the areas it is filled from.
func slotKinds(q queryer, onRow func(program string, row slotRow, areas []slotArea)) ([]slotStatement, error) {
	scan := func(query string, row func(*sql.Rows) error) error {
		rows, err := q.Query(query)
		if err != nil {
			return err
		}
		defer rows.Close()
		for rows.Next() {
			if err := row(rows); err != nil {
				return err
			}
		}
		return rows.Err()
	}

	kinds := make(map[[2]string]string) // (program, module) → the kind the statements decide; "" for none
	if err := scan("SELECT program_id, module_id, IFNULL(kind, '') FROM v_program_module_src", func(rows *sql.Rows) error {
		var program, module, kind string
		if err := rows.Scan(&program, &module, &kind); err != nil {
			return err
		}
		kinds[[2]string{program, module}] = kind
		return nil
	}); err != nil {
		return nil, err
	}

	type node struct {
		parent      sql.NullInt64
		label, path string
	}
	nodes := make(map[int64]node)
	if err := scan("SELECT id, parent_id, label, path FROM program_area", func(rows *sql.Rows) error {
		var id int64
		var n node
		if err := rows.Scan(&id, &n.parent, &n.label, &n.path); err != nil {
			return err
		}
		nodes[id] = n
		return nil
	}); err != nil {
		return nil, err
	}

	// The tree's placements in tree order, as Folia reads them (queries::program_areas).
	type placement struct {
		module string
		area   int64
	}
	placements := make(map[string][]placement)
	if err := scan(`
		SELECT a.program_id, a.module_id, a.area_id FROM program_module_assertion a
		JOIN program_area pa ON pa.id = a.area_id
		WHERE a.source = 'qis_tree'
		ORDER BY a.program_id, pa.ord, pa.id, a.module_id`, func(rows *sql.Rows) error {
		var program string
		var p placement
		if err := rows.Scan(&program, &p.module, &p.area); err != nil {
			return err
		}
		placements[program] = append(placements[program], p)
		return nil
	}); err != nil {
		return nil, err
	}

	entries := make(map[string][]slotRow)
	var programs []string
	if err := scan(`
		SELECT e.program_id, IFNULL(e.module_id, ''), e.module_name, IFNULL(e.kind, ''), IFNULL(e.specialization, '')
		FROM plan_entry e JOIN program p ON p.id = e.program_id
		ORDER BY e.program_id, e.ord`, func(rows *sql.Rows) error {
		var program string
		var r slotRow
		if err := rows.Scan(&program, &r.moduleID, &r.name, &r.kind, &r.specialization); err != nil {
			return err
		}
		if len(entries[program]) == 0 {
			programs = append(programs, program)
		}
		entries[program] = append(entries[program], r)
		return nil
	}); err != nil {
		return nil, err
	}

	var statements []slotStatement
	for _, program := range programs {
		if len(placements[program]) == 0 {
			continue
		}
		var areas []slotArea
		index := make(map[int64]int)
		for _, p := range placements[program] {
			kind := kinds[[2]string{program, p.module}]
			choice := kind != kindCompulsory && kind != kindThesis && kind != kindInternship
			if i, ok := index[p.area]; ok {
				areas[i].modules++
				areas[i].choice = areas[i].choice || choice
				continue
			}
			var chain []string
			seen := map[int64]bool{p.area: true}
			for next := nodes[p.area].parent; next.Valid && len(chain) <= 32; {
				parent, ok := nodes[next.Int64]
				if !ok || seen[next.Int64] {
					break
				}
				seen[next.Int64] = true
				chain = append(chain, parent.label)
				next = parent.parent
			}
			for i, j := 0, len(chain)-1; i < j; i, j = i+1, j-1 {
				chain[i], chain[j] = chain[j], chain[i]
			}
			index[p.area] = len(areas)
			areas = append(areas, slotArea{id: p.area, label: nodes[p.area].label, path: nodes[p.area].path, ancestors: chain, modules: 1, choice: choice})
		}

		inferred := make(map[string]bool)
		for _, row := range entries[program] {
			if row.moduleID != "" || row.kind != kindElective || strings.TrimSpace(row.name) == "" || isFUESRow(row) {
				continue
			}
			var variant []slotRow
			for _, other := range entries[program] {
				if other.specialization == row.specialization {
					variant = append(variant, other)
				}
			}
			found := areasForRow(row, row.specialization, areas, variant)
			if onRow != nil {
				onRow(program, row, found)
			}
			for _, area := range found {
				for _, p := range placements[program] {
					if p.area != area.id || inferred[p.module] || kinds[[2]string{program, p.module}] != "" {
						continue
					}
					inferred[p.module] = true
					statements = append(statements, slotStatement{program: program, module: p.module, row: row.name})
				}
			}
		}
	}
	return statements, nil
}

// isFUESRow is Folia's plan::is_fues: a row that asks for a module of the FÜS, by its kind or
// only by its name. Its modules are the program's FÜS list, not its tree.
func isFUESRow(row slotRow) bool {
	if row.kind == kindFUES {
		return true
	}
	for _, word := range strings.FieldsFunc(normalize.SearchFold(row.name), notAlphanumeric) {
		if word == "fus" || strings.HasPrefix(word, "fusmodul") || strings.HasPrefix(word, "fachubergreifend") || strings.HasPrefix(word, "facherubergreifend") {
			return true
		}
	}
	return false
}

// notAlphanumeric parts a text where Rust's char::is_alphanumeric does not hold.
func notAlphanumeric(r rune) bool {
	return !(unicode.IsLetter(r) || unicode.IsNumber(r) ||
		unicode.In(r, unicode.Other_Alphabetic, unicode.Other_Lowercase, unicode.Other_Uppercase))
}

func runes(s string) int { return utf8.RuneCountInString(s) }

func allASCIIDigits(s string) bool {
	for _, r := range s {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

// directionOf is the study direction a label ends in („Informatik (MIT)" → „MIT").
func directionOf(label string) string {
	at := strings.LastIndex(label, "(")
	if at < 0 {
		return ""
	}
	rest := label[at+1:]
	end := strings.Index(rest, ")")
	if end < 0 {
		return ""
	}
	inside := strings.TrimSpace(rest[:end])
	if inside == "" || len(inside) > 12 || strings.IndexFunc(inside, unicode.IsSpace) >= 0 {
		return ""
	}
	return inside
}

// sameDirection: a plan printed for direction („PA", as a caption spells it) is about the
// areas of areaDirection („PAu", as the tree spells it).
func sameDirection(direction, areaDirection string) bool {
	return len(direction) >= 2 && (strings.HasPrefix(areaDirection, direction) || strings.HasPrefix(direction, areaDirection))
}

// withoutDirection is a label without the study direction it ends in.
func withoutDirection(label string) string {
	if directionOf(label) == "" {
		return label
	}
	return strings.TrimRightFunc(label[:strings.LastIndex(label, "(")], unicode.IsSpace)
}

var (
	slotNoise = wordSet("komplex wahlpflicht wahlpflichtmodul wahlpflichtmodule wahlpflichtbereich wahlpflichtfach wahlpflichtkatalog wahlpflichtangebot " +
		"wahl wahlbereich modul module moduls modulen modules pflicht pflichtmodul pflichtmodule wp wpf pf " +
		"bereich bereiche bereichs studium katalog lp kp ects aus dem der des die das den im in und oder " +
		"von vom zum zur mit ein eine einer eines einem einen fur gewahlten gewahlte wahlbar wahlen gemass gem anlage anlagen " +
		"siehe pru sl bis je nach sowie zu compulsory elective optional mandatory of the and or from")
	slotRoman      = map[string]string{"i": "1", "ii": "2", "iii": "3", "iv": "4", "vi": "6"}
	slotNebenfach  = wordSet("nebenfach nebenfacher anwendungsfach anwendungsfacher anwendungen anwendungsbereich anwendungsbereiche")
	slotReferences = wordSet("anlage anlagen tab tabelle seite")
	slotPhases     = wordSet("grundstudium fachstudium hauptstudium basisstudium kernstudium vertiefungsstudium grundlagenstudium bachelorstudium masterstudium")
	slotAccounts   = []string{"gesamtkonto", "total account", "module an der ", "module an den ", "modules at ", "studierende der ", "students of "}
)

func wordSet(words string) map[string]bool {
	set := make(map[string]bool)
	for _, w := range strings.Fields(words) {
		set[w] = true
	}
	return set
}

// distinctive is Folia's plan::distinctive: the words a name says which area it is by —
// folded, without the words of what kind of thing it is („Komplex", „Wahlpflichtmodule",
// „Modul aus dem"), articles and the plan's references („gem. Anlage a5"), a footnote number
// in front or glued to the end, Roman numbers as digits, a plural as its singular, and
// „Anwendungsfach", „Anwendungen" … as „nebenfach". A name of nothing but numbers says nothing.
func distinctive(text string) []string {
	var words []string
	afterReference := false
	for _, word := range strings.FieldsFunc(normalize.SearchFold(text), notAlphanumeric) {
		if digit, ok := slotRoman[word]; ok {
			word = digit
		} else {
			letters := 0
			for _, r := range word {
				if unicode.IsLetter(r) || unicode.In(r, unicode.Other_Alphabetic) {
					letters++
				}
			}
			if letters >= 4 {
				word = strings.TrimRight(word, "0123456789")
			}
		}
		number := allASCIIDigits(word)
		reference := !number && strings.ContainsAny(word, "0123456789") && runes(word) <= 3
		if (len(word) < 2 && !number) || reference || slotNoise[word] {
			afterReference = slotReferences[word]
			continue
		}
		if number && (len(words) == 0 || afterReference) {
			afterReference = false
			continue
		}
		afterReference = slotReferences[word]
		if slotNebenfach[word] {
			word = "nebenfach"
		} else {
			word = singular(word)
		}
		words = append(words, word)
	}
	for _, w := range words {
		if !allASCIIDigits(w) {
			return words
		}
	}
	return nil
}

// singular gives singular and plural one form („grundlagen", „grundlage" → „grundlag").
func singular(word string) string {
	if runes(word) > 6 {
		for _, ending := range []string{"en", "e", "n", "s"} {
			if stem, ok := strings.CutSuffix(word, ending); ok {
				return stem
			}
		}
	}
	return word
}

func contains(words []string, word string) bool {
	for _, w := range words {
		if w == word {
			return true
		}
	}
	return false
}

// fit is how well the words of a row fit the words of a name: the same words (100), the one
// within the other (60, 5 more for each word they share), some words in common (10 each, where
// they are at least half of the row's words other than numbers), or nothing (0).
func fit(row, name []string) int {
	if len(row) == 0 || len(name) == 0 {
		return 0
	}
	within := func(inner, outer []string) bool {
		for _, w := range inner {
			if !contains(outer, w) {
				return false
			}
		}
		return true
	}
	if len(row) == len(name) && within(row, name) && within(name, row) {
		return 100
	}
	shared := 0
	for _, w := range row {
		if contains(name, w) {
			shared++
		}
	}
	if within(row, name) || within(name, row) {
		return 60 + 5*shared
	}
	words, matched := 0, 0
	for _, w := range row {
		if allASCIIDigits(w) {
			continue
		}
		words++
		for _, other := range name {
			if !allASCIIDigits(other) && akin(w, other) {
				matched++
				break
			}
		}
	}
	if matched*2 < words {
		return 0
	}
	return 10 * matched
}

// akin: two words that read as one — one the start of the other (an inflection, or a long
// word), never a short word at the start of a compound, or with a long beginning in common.
func akin(a, b string) bool {
	ar, br := []rune(a), []rune(b)
	common := 0
	for common < len(ar) && common < len(br) && ar[common] == br[common] {
		common++
	}
	shorter, longer := min(len(ar), len(br)), max(len(ar), len(br))
	prefix := common == shorter && (longer-shorter <= 2 || shorter >= 8)
	return shorter >= 5 && (prefix || (common >= 8 && common*4 >= shorter*3))
}

// isStructural is Folia's areas::is_structural: a phase, an account, or a label that says
// nothing but a kind — never the name of a field.
func isStructural(label string) bool {
	folded := normalize.SearchFold(strings.TrimSpace(label))
	if slotPhases[folded] {
		return true
	}
	for _, account := range slotAccounts {
		if strings.HasPrefix(folded, account) {
			return true
		}
	}
	return len(distinctive(withoutDirection(label))) == 0
}

// areaWords are the words an area is known by: its label's, or for a label that says nothing
// but a kind, those of the nearest node above that names a field (its index in ancestors).
func areaWords(area slotArea) ([]string, int) {
	if own := distinctive(withoutDirection(area.label)); len(own) > 0 {
		return own, -1
	}
	for i := len(area.ancestors) - 1; i >= 0; i-- {
		if !isStructural(area.ancestors[i]) {
			return distinctive(withoutDirection(area.ancestors[i])), i
		}
	}
	return nil, -1
}

// alternatives are the parts of a row that names several areas at once.
func alternatives(name string) []string {
	var quoted []string
	parts := splitAny(name, "„“\"")
	for i := 1; i < len(parts); i += 2 {
		if part := strings.TrimSpace(parts[i]); runes(part) >= 3 {
			quoted = append(quoted, part)
		}
	}
	if len(quoted) >= 2 {
		return quoted
	}
	if _, rest, ok := strings.Cut(name, ":"); ok {
		name = rest
	}
	var result []string
	for _, part := range strings.Split(name, " / ") {
		for _, alt := range strings.Split(part, " oder ") {
			if alt = strings.TrimSpace(alt); alt != "" {
				result = append(result, alt)
			}
		}
	}
	return result
}

// splitAny splits s at every rune of seps, keeping empty parts.
func splitAny(s, seps string) []string {
	var parts []string
	start := 0
	for i, r := range s {
		if strings.ContainsRune(seps, r) {
			parts = append(parts, s[start:i])
			start = i + utf8.RuneLen(r)
		}
	}
	return append(parts, s[start:])
}

// areasForRow is Folia's plan::areas_for_row (its `areas`, not its `others`): the areas a
// student chooses from that a row of the plan is about. plan is the caption of the plan
// (the row's specialization), rows the rows of the same plan.
func areasForRow(entry slotRow, plan string, areas []slotArea, rows []slotRow) []slotArea {
	rowWords := distinctive(entry.name)
	if len(rowWords) == 0 {
		return nil
	}
	var planParts []string
	for _, part := range strings.FieldsFunc(plan, notAlphanumeric) {
		if len(part) >= 2 {
			planParts = append(planParts, part)
		}
	}
	caption := normalize.SearchFold(plan)
	var directions []string
	for _, area := range areas {
		direction := directionOf(area.label)
		if direction == "" {
			continue
		}
		name := normalize.SearchFold(withoutDirection(area.label))
		named := false
		for _, part := range planParts {
			if sameDirection(part, direction) {
				named = true
				break
			}
		}
		if named || (runes(name) >= 8 && strings.Contains(caption, name)) {
			directions = append(directions, direction)
		}
	}
	type candidate struct {
		area    slotArea
		words   []string
		namedBy int
	}
	var candidates []candidate
	for _, area := range areas {
		if !area.choice {
			continue
		}
		if direction := directionOf(area.label); len(directions) > 0 && direction != "" && !contains(directions, direction) {
			continue
		}
		words, namedBy := areaWords(area)
		candidates = append(candidates, candidate{area, words, namedBy})
	}

	// A row that lists areas by their names: each of them (unless the whole row is one name).
	if parts := alternatives(entry.name); len(parts) >= 2 {
		whole := false
		for _, c := range candidates {
			if fit(rowWords, c.words) == 100 {
				whole = true
				break
			}
		}
		if !whole {
			var named [][]slotArea
			for _, part := range parts {
				words := distinctive(part)
				var found []slotArea
				for _, c := range candidates {
					if fit(words, c.words) == 100 {
						found = append(found, c.area)
					}
				}
				if len(found) > 0 {
					named = append(named, found)
				}
			}
			if len(named) >= 2 {
				var listed []slotArea
				seen := make(map[int64]bool)
				for _, found := range named {
					for _, area := range found {
						if !seen[area.id] {
							seen[area.id] = true
							listed = append(listed, area)
						}
					}
				}
				return listed
			}
		}
	}

	// What the other rows of the plan name on their own is not what this row means by a name
	// above it.
	claimed := func(words []string) bool {
		for _, other := range rows {
			if other.moduleID == "" && other.name != entry.name && fit(distinctive(other.name), words) == 100 {
				return true
			}
		}
		return false
	}
	type scoredArea struct {
		score int
		area  slotArea
	}
	var scored []scoredArea
	for _, c := range candidates {
		byName := fit(rowWords, c.words)
		if byName != 100 && c.namedBy >= 0 {
			byName /= 2
		}
		byAncestor := 0
		for i, ancestor := range c.area.ancestors {
			if i == c.namedBy {
				continue
			}
			if score := fit(rowWords, distinctive(withoutDirection(ancestor))); score >= 60 && score > byAncestor {
				byAncestor = score
			}
		}
		if byName == 0 && byAncestor > 0 && claimed(c.words) {
			continue
		}
		score := byName + byAncestor/2
		if direction := directionOf(c.area.label); score > 0 && direction != "" && contains(directions, direction) {
			score += 5
		}
		if score > 0 {
			scored = append(scored, scoredArea{score, c.area})
		}
	}
	sort.SliceStable(scored, func(i, j int) bool {
		if scored[i].score != scored[j].score {
			return scored[i].score > scored[j].score
		}
		return scored[i].area.modules > scored[j].area.modules
	})
	var result []slotArea
	for _, s := range scored {
		if s.score != scored[0].score || len(result) == 8 {
			break
		}
		result = append(result, s.area)
	}
	return result
}
