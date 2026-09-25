// Package abbrev derives short names for modules: „AuP“ for „Algorithmieren und
// Programmieren“, „EEG“ for „Elektrische und elektronische Grundlagen der Informatik“.
//
// No source states them (docs/data-sources.md §5.14), so they are derived, and derived
// again by every build: an abbreviation is metadata that follows the catalog, never a
// fact to keep (owner, 2026-09-25: „dann machen wir die meta eben neu“).
//
// Every title gets a ranked list of candidates with a cost: word initials, compound
// initials (Betriebs|systeme → BS), function letters (AuP), dropped openings and tails
// (EEG), first letters, subtitle forms. Three characters are the sweet spot. Within a
// program, over all of its selectable modules (curriculum, electives and FÜS), every
// abbreviation is unique: a candidate that two modules want goes to neither, and both fall
// back („Grundzüge der Makro-/Mikroökonomik“ → GMa / GMi), unless a guard settles it by
// priority. The rules and their numbers: docs/schema-v2.md, „Short names“.
package abbrev

import (
	"sort"
	"strconv"
	"strings"
)

// Module is a module of the catalog with the title Folia shows (module.title).
type Module struct{ ID, Title string }

// Member is a module a program lets its students select. Tier orders the contest for a
// candidate: 0 compulsory, thesis and internship, 1 other curricular, 2 FÜS.
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
	// UnusedOverrides are module-number lines of the override file that name no module.
	UnusedOverrides []string
}

// Derive computes the abbreviation of every module (Defaults) and of every module of every
// program (Programs), unique within the program. titles are all title variants of the
// catalog (title, title_de, title_en): they are the vocabulary the compound splitter knows.
// The result depends only on its input, never on its order.
func Derive(modules []Module, members []Member, titles []string, overrides []Override) *Result {
	d := &deriver{
		split:  newSplitter(Vocabulary(titles)),
		titles: map[string]string{},
		parsed: map[string]*parsed{},
		base:   map[string][]candidate{},
		sib:    map[string][]candidate{},
		prefix: map[string][]candidate{},
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
	for _, o := range overrides {
		if _, ok := d.titles[o.ModuleID]; o.ModuleID != "" && !ok {
			res.UnusedOverrides = append(res.UnusedOverrides, "line "+strconv.Itoa(o.Line)+": "+o.ModuleID+" → "+o.Abbrev)
		}
	}
	ids := make([]string, 0, len(d.titles))
	for id := range d.titles {
		ids = append(ids, id)
	}
	sort.Strings(ids)
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
	return res
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
	l, ok := cache[id]
	if !ok {
		p := d.parse(id)
		l = d.candidates(p, sibling)
		l = d.override(l, id, p, "")
		l = usable(l, p)
		cache[id] = l
	}
	if program != "" {
		l = d.override(l, id, d.parse(id), program)
	}
	return l
}

// override puts the override lines for a module, in one program or in every program, on
// top of its candidates: first the first matching title pattern, then the module's number.
func (d *deriver) override(l []candidate, id string, p *parsed, program string) []candidate {
	for _, o := range d.patterns {
		if o.Program != program {
			continue
		}
		if text, ok := o.apply(p); ok {
			l = withOverride(l, text)
			break
		}
	}
	for _, o := range d.byModule[id] {
		if o.Program == program {
			text, _ := o.apply(p)
			l = withOverride(l, text)
		}
	}
	return l
}

func withOverride(l []candidate, text string) []candidate {
	out := make([]candidate, 0, len(l)+1)
	out = append(out, candidate{text: text, cost: -100, how: "override", override: true})
	for _, c := range l {
		if key(c.text) != key(text) {
			out = append(out, c)
		}
	}
	return out
}

// usable keeps the candidates of 2 to 10 characters; a title that gives none gets its first letters.
func usable(l []candidate, p *parsed) []candidate {
	out := l[:0:0]
	for _, c := range l {
		if n := runes(c.text); n >= 2 && n <= 10 {
			out = append(out, c)
		}
	}
	if len(out) == 0 {
		var b strings.Builder
		for _, r := range p.title {
			if isLetterOrDigit(r) && runes(b.String()) < 3 {
				b.WriteRune(r)
			}
		}
		s := capitalize(b.String())
		for runes(s) < 2 {
			s += "x"
		}
		out = append(out, candidate{text: s, cost: 9900, how: "fallback"})
	}
	return out
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
		byHead[p.headKey][key(p.title)] = true
	}
	extra := d.sharedOpenings(entries)

	lists := make([][]candidate, len(entries))
	for i, e := range entries {
		p := d.parse(e.module)
		l := d.list(e.module, len(byHead[p.headKey]) > 1 && len(p.subs) > 0, program)
		if x, ok := extra[e.module]; ok {
			l = merge(l, x)
		}
		lists[i] = l
	}
	return d.contrast(entries, lists)
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

// contrast resolves one program. Each module proposes its first candidate that is not
// banned for it. A candidate proposed by modules with different titles is nobody's (the
// owner's rule), with guards: a higher tier keeps it against a lower one, an earlier choice
// beats a fallback, and FÜS against FÜS, a far next choice or near-identical candidate lists
// are settled by priority. Then every module takes its first free candidate, and identical
// titles get -b, -c ….
func (d *deriver) contrast(entries []entry, lists [][]candidate) map[string]Choice {
	n := len(entries)
	title := make([]string, n)
	sig := make([]string, n)
	banned := make([]map[string]bool, n)
	for i, e := range entries {
		title[i] = key(d.parse(e.module).title)
		var ks []string
		for j, c := range lists[i] {
			if j == 8 {
				break
			}
			ks = append(ks, key(c.text))
		}
		sig[i] = strings.Join(ks, "\x00")
		banned[i] = map[string]bool{}
	}
	propose := func(i int) int {
		for r, c := range lists[i] {
			if !banned[i][key(c.text)] {
				return r
			}
		}
		return -1
	}
	for round := 0; round < 200; round++ {
		prop := make([]int, n)
		var keys []string
		byKey := map[string][]int{}
		for i := range entries {
			prop[i] = propose(i)
			if prop[i] < 0 {
				continue
			}
			k := key(lists[i][prop[i]].text)
			if _, ok := byKey[k]; !ok {
				keys = append(keys, k)
			}
			byKey[k] = append(byKey[k], i)
		}
		changed := false
		for _, k := range keys {
			ms := byKey[k]
			if distinctOf(ms, title) < 2 {
				continue
			}
			best := entries[ms[0]].tier
			for _, m := range ms {
				best = min(best, entries[m].tier)
			}
			rmin := -1
			for _, m := range ms {
				if entries[m].tier == best && (rmin < 0 || prop[m] < rmin) {
					rmin = prop[m]
				}
			}
			var top []int
			holders := map[string]bool{}
			for _, m := range ms {
				if entries[m].tier == best && prop[m] == rmin {
					top = append(top, m)
					holders[title[m]] = true
				}
			}
			for _, m := range ms {
				if entries[m].tier > best || prop[m] > rmin {
					if !holders[title[m]] {
						banned[m][k] = true
						changed = true
					}
				}
			}
			if distinctOf(top, title) < 2 {
				continue
			}
			next := func(m int) candidate {
				for _, c := range lists[m] {
					if kk := key(c.text); kk != k && !banned[m][kk] {
						return c
					}
				}
				return candidate{text: "?????????", cost: 9900}
			}
			tooFar := best >= 2
			for _, m := range top {
				c := next(m)
				if c.cost > lists[m][0].cost+100 || runes(c.text) > max(4, runes(lists[m][prop[m]].text)) {
					tooFar = true
				}
			}
			if tooFar || distinctOf(top, sig) == 1 {
				win := top[0] // entries are in priority order
				for _, m := range top {
					if m != win && title[m] != title[win] {
						banned[m][k] = true
						changed = true
					}
				}
			} else {
				for _, m := range top {
					banned[m][k] = true
				}
				changed = true
			}
		}
		if !changed {
			break
		}
	}

	// Anything still contested or exhausted: first come, first served over what is left.
	type pick struct {
		c    candidate
		rank int
	}
	picks := make([]pick, n)
	taken := map[string]string{}
	for i := range entries {
		found := false
		for r, c := range lists[i] {
			k := key(c.text)
			if banned[i][k] {
				continue
			}
			if t, ok := taken[k]; !ok || t == title[i] {
				if !ok {
					taken[k] = title[i]
				}
				picks[i] = pick{c, r}
				found = true
				break
			}
		}
		if !found {
			text := suffixed(lists[i][0].text, taken, 0)
			taken[key(text)] = title[i]
			picks[i] = pick{candidate{text: text, cost: 9900, how: "numbered", override: lists[i][0].override}, len(lists[i])}
		}
	}

	// Identical titles in one program: the first in priority order keeps the form.
	out := make(map[string]Choice, n)
	used := map[string]bool{}
	for _, p := range picks {
		used[key(p.c.text)] = true
	}
	seen := map[string]int{}
	for i, e := range entries {
		p := picks[i]
		c := Choice{Abbrev: p.c.text, Override: p.c.override, Choice: p.rank + 1}
		k := key(p.c.text)
		if count, ok := seen[k]; ok {
			seen[k] = count + 1
			text := suffixed(p.c.text, used, count)
			used[key(text)] = true
			c.Abbrev, c.Twin = text, true
		} else {
			seen[k] = 0
		}
		out[e.module] = c
	}
	return out
}

func distinctOf(ms []int, values []string) int {
	s := map[string]bool{}
	for _, m := range ms {
		s[values[m]] = true
	}
	return len(s)
}

const suffixLetters = "bcdefghijklmnopqrstuvwxyz"

// suffixed returns base-b, base-c … (from the given letter on), the first one not taken. A
// letter, not a digit that would read as a series number; the whole stays within 10 characters.
func suffixed[V any](base string, taken map[string]V, from int) string {
	if runes(base) > 8 {
		base = prefix(base, 8)
	}
	for i := from; ; i++ {
		text := base + "-" + string(suffixLetters[i%len(suffixLetters)])
		if i >= len(suffixLetters) {
			text = prefix(base, 7) + "-" + strconv.Itoa(i)
		}
		if _, ok := taken[key(text)]; !ok {
			return text
		}
	}
}
