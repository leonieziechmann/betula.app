// Package abbrev derives short names for modules: „AuP“ for „Algorithmieren und
// Programmieren“, „EEG“ for „Elektrische und elektronische Grundlagen der Informatik“.
//
// No source states them (docs/radix/data-sources.md §5.14), so they are derived, and derived
// again by every build: an abbreviation is metadata that follows the catalog, never a
// fact to keep (owner, 2026-09-25: „dann machen wir die meta eben neu“).
//
// Every title gets a ranked list of candidates with a cost: word initials, compound
// initials (Betriebs|systeme → BS), function letters (AuP), dropped openings and tails
// (EEG), first letters, subtitle forms. Three characters are the sweet spot, and where the
// initials of all words make exactly three, function words small, they come first (the
// owner's rule: „Entwicklung von Softwaresystemen“ → EvS, not ESS); a title „X- und Y“ gives its
// terms without the und instead (Signal- und Systemtheorie → SST). A form on the
// blocked list (blocked.tsv, the building tokens of short room names) or one the override file reserves for another
// title is never derived. Within a program, over all of its selectable modules (curriculum,
// electives and FÜS), every abbreviation is unique, and so is its stem (ST and ST1 read as one
// series): where two modules want one form, the one whose title it fits better (its matching
// score, with a bonus for a module of the program's curriculum over a FÜS module) keeps it, and
// the other moves on, displacing a weaker holder in turn, for at most three rounds (assign.go). The rules and their numbers: docs/radix/schema-v2.md, „Short names“.
package abbrev

import (
	"sort"
	"strconv"
	"strings"
)

// Module is a module of the catalog with the title Folia shows (module.title).
type Module struct{ ID, Title string }

// Member is a module a program lets its students select. Tier orders the contest for a
// candidate: 0 compulsory, thesis and internship, 1 other curricular, 2 FÜS; tiers 0 and 1, the
// program's curriculum, also score scoreCurriculum more than a FÜS module.
// PlanSemester is the semester of the program's plan, 99 for none.
type Member struct {
	ProgramID, ModuleID string
	Tier, PlanSemester  int
}

// Tier maps a relation and kind of v_program_module to Member.Tier.
func Tier(relation, kind string) int {
	switch {
	case relation == "fues":
		return 2
	case kind == "compulsory" || kind == "thesis" || kind == "internship":
		return 0
	}
	return 1
}

// Choice is the abbreviation a module gets.
type Choice struct {
	Abbrev   string
	Override bool // a line of overrides.tsv made it
	Choice   int  // 1: the module's first candidate; more: it fell back
	Twin     bool // -b, -c … after an identical title in the program
}

// Result is what Derive found.
type Result struct {
	Defaults  map[string]Choice            // module → its abbreviation without a program
	Programs  map[string]map[string]Choice // program → module → its abbreviation there
	FellBack  int                          // pairs that did not get their first choice (twins included)
	Twins     int                          // pairs that got -b, -c …
	Overrides int                          // pairs an override line made
	// UnusedOverrides are lines of the override file that apply to no module: a module number
	// the catalog lacks, a (program, module) pair that does not exist, a pattern that matches
	// no title an earlier line does not take.
	UnusedOverrides []string
}

// Derive computes the abbreviation of every module (Defaults) and of every module of every
// program (Programs), unique within the program. titles are all title variants of the
// catalog (title, title_de, title_en): they are the vocabulary the compound splitter knows.
// The result depends only on its input, never on its order.
func Derive(modules []Module, members []Member, titles []string, overrides []Override) *Result {
	d := &deriver{
		split:        newSplitter(Vocabulary(titles)),
		titles:       map[string]string{},
		parsed:       map[string]*parsed{},
		base:         map[string][]candidate{},
		sib:          map[string][]candidate{},
		prefix:       map[string][]candidate{},
		used:         map[*Override]bool{},
		reserved:     map[string]map[string]bool{},
		reservedStem: map[string]map[string]bool{},
	}
	for _, m := range modules {
		d.titles[m.ID] = m.Title
	}
	d.byModule = map[string][]*Override{}
	for i := range overrides {
		o := &overrides[i]
		if o.ModuleID != "" {
			d.byModule[o.ModuleID] = append(d.byModule[o.ModuleID], o)
		} else {
			d.patterns = append(d.patterns, o)
		}
	}

	res := &Result{Defaults: map[string]Choice{}, Programs: map[string]map[string]Choice{}}
	ids := make([]string, 0, len(d.titles))
	for id := range d.titles {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	// The forms of the owner's and the common lines mean one thing in the whole catalog: no
	// other title derives them (AuP is Algorithmieren und Programmieren, MA a master's thesis).
	for _, id := range ids {
		p := d.parse(id)
		for _, a := range d.lines(id, p, "") {
			if a.o.Source == "page" {
				continue
			}
			reserve(d.reserved, key(a.text), p)
			if Stem(a.text) == a.text {
				reserve(d.reservedStem, stemKey(a.text), p)
			}
		}
	}
	for _, id := range ids {
		c := d.list(id, false, "")[0]
		res.Defaults[id] = Choice{Abbrev: c.text, Override: c.override, Choice: 1}
	}

	byProgram := map[string][]entry{}
	for _, m := range members {
		byProgram[m.ProgramID] = append(byProgram[m.ProgramID], entry{m.ModuleID, m.Tier, m.PlanSemester})
	}
	for pid, entries := range byProgram {
		sort.Slice(entries, func(i, j int) bool {
			a, b := entries[i], entries[j]
			if a.tier != b.tier {
				return a.tier < b.tier
			}
			if a.sem != b.sem {
				return a.sem < b.sem
			}
			return a.module < b.module
		})
		choices := d.resolve(pid, entries)
		for _, c := range choices {
			if c.Choice > 1 || c.Twin {
				res.FellBack++
			}
			if c.Twin {
				res.Twins++
			}
			if c.Override {
				res.Overrides++
			}
		}
		res.Programs[pid] = choices
	}
	res.UnusedOverrides = d.unused(overrides, members)
	return res
}

func reserve(m map[string]map[string]bool, k string, p *parsed) {
	if m[k] == nil {
		m[k] = map[string]bool{}
	}
	m[k][p.seriesKey] = true
}

// titleKey tells titles apart: two modules with one titleKey are twins.
func titleKey(title string) string { return strings.ToLower(title) }

// unused lists the lines of the override file that applied to no module.
func (d *deriver) unused(overrides []Override, members []Member) []string {
	pairs := map[string]bool{}
	for _, m := range members {
		pairs[m.ProgramID+"\t"+m.ModuleID] = true
	}
	var out []string
	for i := range overrides {
		o := &overrides[i]
		if d.used[o] {
			continue
		}
		what, why := o.ModuleID, "names no module"
		if o.Pattern != nil {
			what, why = "/"+o.Pattern.String()+"/", "matches no module an earlier line does not take"
		}
		if o.Program != "" {
			what += " in " + o.Program
			if _, ok := d.titles[o.ModuleID]; o.ModuleID != "" && ok && !pairs[o.Program+"\t"+o.ModuleID] {
				why = "the program has no such module"
			}
		}
		out = append(out, "line "+strconv.Itoa(o.Line)+": "+what+" → "+o.Abbrev+" ("+why+")")
	}
	return out
}

type entry struct {
	module    string
	tier, sem int
}

type deriver struct {
	split    *splitter
	titles   map[string]string
	parsed   map[string]*parsed
	base     map[string][]candidate // module → program-free candidates
	sib      map[string][]candidate // module → candidates as a sibling
	prefix   map[string][]candidate // module + shared opening → candidates of the rest
	byModule map[string][]*Override
	patterns []*Override
	used     map[*Override]bool // lines that applied to a module
	// reserved: key of an owner's or common line's form → the heads (seriesKey) of the titles
	// it belongs to, so that Datenbanken I may have DB1; reservedStem the same for the stems of
	// the forms without a designator (MA blocks MA2 of another head).
	reserved, reservedStem map[string]map[string]bool
}

func (d *deriver) parse(id string) *parsed {
	if p, ok := d.parsed[id]; ok {
		return p
	}
	title, ok := d.titles[id]
	if !ok {
		title = id
	}
	p := parseTitle(title, d.split)
	d.parsed[id] = p
	return p
}

// list returns the ranked candidates of a module, with the override lines of every
// program and of the given one on top.
func (d *deriver) list(id string, sibling bool, program string) []candidate {
	cache := d.base
	if sibling {
		cache = d.sib
	}
	p := d.parse(id)
	l, ok := cache[id]
	if !ok {
		l, _ = d.override(d.candidates(p, sibling), id, p, "", sibling)
		l = d.usable(l, p)
		cache[id] = l
	}
	if program != "" {
		if lp, applied := d.override(l, id, p, program, sibling); applied {
			l = d.usable(lp, p)
		}
	}
	return l
}

type appliedLine struct {
	o    *Override
	text string
}

// lines are the override lines for a module, in one program or in every program, in the
// order they are put on top: the first matching title pattern, then the module's number.
func (d *deriver) lines(id string, p *parsed, program string) []appliedLine {
	var out []appliedLine
	for _, o := range d.patterns {
		if o.Program != program {
			continue
		}
		if text, ok := o.apply(p); ok {
			out = append(out, appliedLine{o, text})
			break
		}
	}
	for _, o := range d.byModule[id] {
		if o.Program == program {
			text, _ := o.apply(p)
			out = append(out, appliedLine{o, text})
		}
	}
	return out
}

// override puts the override lines for a module on top of its candidates. A sibling (another
// module of the program has its head and designator) puts the override's form with its
// subtitle first: ABWL3I and ABWL3B for the two „Allgemeine Betriebswirtschaftslehre III“,
// not ABWL3 for one and derived initials for the other.
func (d *deriver) override(l []candidate, id string, p *parsed, program string, sibling bool) ([]candidate, bool) {
	as := d.lines(id, p, program)
	for _, a := range as {
		d.used[a.o] = true
		l = withOverride(l, a.text)
	}
	if len(as) > 0 && sibling {
		var subs []candidate
		for _, sp := range subtitleForms(p) {
			subs = append(subs, candidate{text: l[0].text + sp.s, cost: -100, how: "override+sub", override: true})
		}
		l = withFirst(subs, l)
	}
	return l, len(as) > 0
}

func withOverride(l []candidate, text string) []candidate {
	return withFirst([]candidate{{text: text, cost: -100, how: "override", override: true}}, l)
}

// withFirst puts first before l, without the candidates of l that first has.
func withFirst(first, l []candidate) []candidate {
	out := make([]candidate, 0, len(first)+len(l))
	seen := map[string]bool{}
	for _, c := range append(append([]candidate(nil), first...), l...) {
		if k := key(c.text); !seen[k] {
			seen[k] = true
			out = append(out, c)
		}
	}
	return out
}

// usable keeps the candidates of 2 to 10 characters that the module may have (allowed); a
// title that gives none gets its first letters, as many as it takes.
func (d *deriver) usable(l []candidate, p *parsed) []candidate {
	out := l[:0:0]
	for _, c := range l {
		if n := runes(c.text); n >= 2 && n <= 10 && d.allowed(c, p) {
			out = append(out, c)
		}
	}
	if len(out) == 0 {
		var letters []rune
		for _, r := range p.title {
			if isLetterOrDigit(r) {
				letters = append(letters, r)
			}
		}
		for n := 3; ; n++ {
			s := capitalize(string(letters[:min(n, len(letters))]))
			for runes(s) < 2 {
				s += "x"
			}
			c := candidate{text: s, cost: 9900, how: "fallback"}
			if d.allowed(c, p) || n >= min(len(letters), 10) {
				out = append(out, c)
				break
			}
		}
	}
	return out
}

// allowed: an override line may name any form; a derived candidate must not be blocked
// (blocked.tsv, the building tokens) and not reserved by an owner's or common line for a title
// of another head.
func (d *deriver) allowed(c candidate, p *parsed) bool {
	if c.override {
		return true
	}
	if _, b := Blocked(c.text, p.title); b {
		return false
	}
	return !d.reservedElsewhere(c.text, p.seriesKey)
}

// reservedElsewhere: an owner's or common line reserves the form, or its stem, for a title of
// another head than seriesKey.
func (d *deriver) reservedElsewhere(form, seriesKey string) bool {
	if ts, ok := d.reserved[key(form)]; ok && !ts[seriesKey] {
		return true
	}
	if ts, ok := d.reservedStem[stemKey(form)]; ok && !ts[seriesKey] {
		return true
	}
	return false
}

func isLetterOrDigit(r rune) bool {
	return r >= '0' && r <= '9' || r >= 'A' && r <= 'Z' || r >= 'a' && r <= 'z' || strings.ContainsRune("ÄÖÜäöüß", r)
}

// resolve gives every module of one program an abbreviation that is unique in it.
func (d *deriver) resolve(program string, entries []entry) map[string]Choice {
	// Siblings: the same head and designator, different full titles. Their subtitle tells them
	// apart: „Dynamik der Kraftfahrzeuge - Längs-/Querdynamik“ → DKL / DKQ.
	byHead := map[string]map[string]bool{}
	for _, e := range entries {
		p := d.parse(e.module)
		if byHead[p.headKey] == nil {
			byHead[p.headKey] = map[string]bool{}
		}
		byHead[p.headKey][titleKey(p.title)] = true
	}
	extra := d.sharedOpenings(entries)

	lists := make([][]candidate, len(entries))
	for i, e := range entries {
		p := d.parse(e.module)
		l := d.list(e.module, len(byHead[p.headKey]) > 1 && len(p.subs) > 0, program)
		if x, ok := extra[e.module]; ok {
			var keep []candidate
			for _, c := range x {
				if d.allowed(c, p) {
					keep = append(keep, c)
				}
			}
			l = merge(l, keep)
		}
		lists[i] = l
	}
	out, _ := d.assign(entries, lists)
	return out
}

func merge(base, extra []candidate) []candidate {
	byKey := map[string]int{}
	var out []candidate
	for _, c := range append(append([]candidate(nil), base...), extra...) {
		k := key(c.text)
		if i, ok := byKey[k]; ok {
			if out[i].cost > c.cost {
				out[i] = c
			}
			continue
		}
		byKey[k] = len(out)
		out = append(out, c)
	}
	sortCandidates(out)
	return out
}

// sharedOpenings: titles of one program that open with the same words („Vertiefendes
// Integrationsmodul …“) also get candidates from what follows the shared words, at +0.8.
// It needs three different titles and two shared content words.
func (d *deriver) sharedOpenings(entries []entry) map[string][]candidate {
	type group struct {
		first string
		ids   []string
	}
	var order []string
	groups := map[string]*group{}
	for _, e := range entries {
		p := d.parse(e.module)
		var cw []string
		for _, t := range p.head {
			if t.kind.content() {
				cw = append(cw, strings.ToLower(t.text))
			}
		}
		if len(cw) >= 3 {
			k := cw[0] + "\x00" + cw[1]
			if groups[k] == nil {
				groups[k] = &group{}
				order = append(order, k)
			}
			groups[k].ids = append(groups[k].ids, e.module)
		}
	}
	extra := map[string][]candidate{}
	for _, k := range order {
		ids := groups[k].ids
		heads := map[string][]string{}
		distinct := map[string]bool{}
		for _, id := range ids {
			var ws []string
			for _, t := range d.parse(id).head {
				ws = append(ws, strings.ToLower(t.text))
			}
			heads[id] = ws
			distinct[strings.Join(ws, "\x00")] = true
		}
		if len(distinct) < 3 {
			continue
		}
		for _, id := range ids {
			p := d.parse(id)
			mine := heads[id]
			others := map[string][]string{}
			for _, o := range ids {
				if j := strings.Join(heads[o], "\x00"); j != strings.Join(mine, "\x00") {
					others[j] = heads[o]
				}
			}
			// the longest opening this title shares with at least two other titles
			n := 0
			for k := len(mine) - 1; k > 1; k-- {
				shared := 0
				for _, o := range others {
					if commonPrefix(mine, o) >= k {
						shared++
					}
				}
				if shared >= 2 {
					n = k
					break
				}
			}
			if countContent(p.head[:n]) < 2 {
				continue
			}
			rest := p.head[n:]
			for len(rest) > 0 && (rest[0].kind == kFunc || rest[0].kind == kAnd) {
				rest = rest[1:]
			}
			if !hasContent(rest) {
				continue
			}
			ck := id + "\x00" + strconv.Itoa(len(p.head)-len(rest))
			l, ok := d.prefix[ck]
			if !ok {
				q := *p
				q.head, q.subs, q.parenAcr = rest, nil, nil
				all := d.candidates(&q, false)
				if len(all) > 10 {
					all = all[:10]
				}
				for _, c := range all {
					if n := runes(c.text); n >= 2 && n <= 10 {
						l = append(l, candidate{text: c.text, cost: c.cost + 80, how: "prefix"})
					}
				}
				d.prefix[ck] = l
			}
			extra[id] = l
		}
	}
	return extra
}

func commonPrefix(a, b []string) int {
	n := 0
	for n < len(a) && n < len(b) && a[n] == b[n] {
		n++
	}
	return n
}
