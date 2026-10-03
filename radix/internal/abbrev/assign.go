package abbrev

import "sort"

// The assignment within a program (the owner, 2026-09-25): „Wenn das Kürzel schon existiert,
// dann darf das Modul das Kürzel behalten, das den höheren Matching-Score hat — muss
// kaskadieren, achte aber drauf, dass es nach 3 Mal garantiert terminiert.“

// Matching scores: how well a form fits its title, the same scale for every module, so that
// two modules that want one form can be compared. Higher is better.
const (
	scoreOverride = 10000 // a line of the override file (for a sibling, with its subtitle)
	scoreStated   = 9000  // an acronym the title states for itself, up to five letters („(GIS)“)
	scoreInitials = 8000  // the initials of all words of the title, exactly three (EvS)
	scoreDerived  = 5000  // every other form: 5000 minus its cost, between 1 and 7999
	scoreSuffixed = 0     // the fallback of an exhausted list: a letter suffix (-b)
)

// scoreCurriculum is added, in a program's contest, to every form of a module in that program's
// curriculum (compulsory, thesis, internship or elective; not one of its FÜS offers). The owner
// (2026-09-25): „Alle Module, die in einem Curriculum existieren und nicht ausschließlich FÜS sind,
// sollten da auch nochmal einen ordentlichen Boost bekommen.“ It is the width of the derived band:
// a curriculum module's derived form of ordinary cost (below 1,000) outranks everything a FÜS module
// derives, its initials (EiL of „Elektronik im Labor“) and an acronym its title states included;
// only an override line of a FÜS module, or a curriculum module's fallback material, can still lose
// to it. It is per program, not per module: 170 of the 171 modules any program offers as FÜS are in
// some other program's curriculum, so a bonus for „curricular anywhere“ would lift nearly every
// module and decide nothing. Within one module every form gets it, so a list's order never changes.
const scoreCurriculum = 5000

// score is the matching score of a candidate. Within one module it falls as the list goes on:
// the classes above stand at the top of every list (their costs are the lowest), and the other
// forms are ordered by cost, so a form of higher cost scores less, down to the fallback
// material (longer forms, first letters), which scores lowest.
func score(c candidate) int {
	switch {
	case c.override:
		return scoreOverride
	case c.how == "paren" && c.cost < 0:
		return scoreStated
	case c.how == "initials":
		return scoreInitials
	}
	return min(max(scoreDerived-c.cost, 1), scoreInitials-1)
}

// cascadeRounds bounds the cascade: after the claim, at most three rounds in which a module
// that lost its form, or whose better form came free again, claims anew and may displace a
// weaker holder.
const cascadeRounds = 3

// claim is a module (its index in the program's priority order) holding or wanting the
// candidate of the given rank in its list.
type claim struct{ m, r int }

// assign gives every module of one program an abbreviation that is unique in it, and returns
// the number of cascade rounds it used (0: the claim settled everything).
//
// The lists are sorted by matching score, best first; entries are in priority order (tier:
// compulsory, thesis and internship before other curricular modules before FÜS; then plan
// semester; then module number). Two claims conflict when their forms are one form (compared
// without case, & and -) for different titles, or have one stem (ST, ST1) for different heads.
// Identical titles do not conflict: they share the form and are told apart by -b, -c at the end.
// Of two claims the one with the higher matching score is better, a module of the program's
// curriculum counting scoreCurriculum more than a FÜS module; on equal scores the module first in
// priority order (within one module, the earlier candidate). This is a strict total order, and it
// does not ask who holds a form: every contest has one winner, the same in every round.
//
//  1. Claim: every module claims its best candidate. Where claims conflict, the better one keeps
//     the form.
//  2. Cascade, at most three rounds: every module claims the first candidate of its list it can
//     win against the forms held — one nobody holds, or one whose holders all have worse claims —
//     if that candidate comes before the one it holds: a module without a form, and a holder whose
//     better form has come free again because the module that took it was displaced itself. The
//     claims of a round and the forms held are settled together, best first: a claim stands unless
//     a better one it conflicts with stood before it. A holder that loses is displaced and claims
//     again in the next round.
//  3. Rest: while a module has a candidate before the one it holds (any candidate, without a form)
//     that conflicts with no form held, the best such claim takes it; nobody is displaced any more.
//     A module whose list has none left gets its first candidate with a letter suffix (-b … -z,
//     -bb …) whose form and stem nobody holds and that reads neither as a blocked form nor as a
//     form an override line reserves for another head (NP-d reads as NPD, Au-p as AuP).
//
// It terminates, whatever the lists: the claim and the cascade are at most four passes over the
// modules and their lists; the rest displaces nobody, so each of its steps gives a module without
// a form one or moves a holder to an earlier candidate of its list, which can happen only finitely
// often; and the suffix search ends because a holder rules out at most two suffixes (its form and
// its stem), a blocked or reserved form at most one of each length, and there are 25 + 25² + 25³
// suffixes against far fewer. As the order is strict and ignores who holds, no tie undoes a
// displacement (the loop of a naive cascade: A takes X from B, B takes it back), and going back
// to an earlier candidate cannot loop either; the round limit stops the cascade in any case. What
// the limit can leave is a module that would still win a form a worse claim holds: the rest takes
// only free forms. The result depends only on the entries and their lists.
func (d *deriver) assign(entries []entry, lists [][]candidate) (map[string]Choice, int) {
	n := len(entries)
	title := make([]string, n)
	series := make([]string, n)
	for i, e := range entries {
		p := d.parse(e.module)
		title[i] = titleKey(p.title)
		series[i] = p.seriesKey
	}
	text := func(c claim) string { return lists[c.m][c.r].text }
	// weigh is a claim's matching score in this program: its form's score, and the curriculum's bonus
	weigh := func(c claim) int {
		s := score(lists[c.m][c.r])
		if entries[c.m].tier < 2 {
			s += scoreCurriculum
		}
		return s
	}
	better := func(a, b claim) bool {
		if sa, sb := weigh(a), weigh(b); sa != sb {
			return sa > sb
		}
		if a.m != b.m {
			return a.m < b.m
		}
		return a.r < b.r
	}

	// board holds the forms taken so far, by key and by stem key.
	type board struct{ byKey, byStem map[string][]claim }
	newBoard := func() *board { return &board{map[string][]claim{}, map[string][]claim{}} }
	put := func(b *board, c claim) {
		t := text(c)
		b.byKey[key(t)] = append(b.byKey[key(t)], c)
		b.byStem[stemKey(t)] = append(b.byStem[stemKey(t)], c)
	}
	// rivals are the claims on the board a form of module m conflicts with.
	rivals := func(b *board, m int, t string) []claim {
		var out []claim
		for _, h := range b.byKey[key(t)] {
			if h.m != m && title[h.m] != title[m] {
				out = append(out, h)
			}
		}
		for _, h := range b.byStem[stemKey(t)] {
			if h.m != m && series[h.m] != series[m] {
				out = append(out, h)
			}
		}
		return out
	}

	held := make([]int, n) // rank of the form a module holds, -1 for none
	for i := range held {
		held[i] = -1
	}
	// holding is the board of the forms held; before is how far into its list a module may still
	// look: up to the form it holds, or to the end.
	holding := func() *board {
		b := newBoard()
		for i := range held {
			if held[i] >= 0 {
				put(b, claim{i, held[i]})
			}
		}
		return b
	}
	before := func(i int) int {
		if held[i] >= 0 {
			return held[i]
		}
		return len(lists[i])
	}
	rounds := 0
	for round := 0; round <= cascadeRounds; round++ {
		holders := holding()
		var all []claim
		claimed := false
		for i := range held {
			// the first candidate before the one it holds that this module can win
			for r := 0; r < before(i); r++ {
				c, wins := claim{i, r}, true
				for _, h := range rivals(holders, i, text(c)) {
					if !better(c, h) {
						wins = false
						break
					}
				}
				if wins {
					all = append(all, c)
					claimed = true
					break
				}
			}
			if held[i] >= 0 {
				all = append(all, claim{i, held[i]})
			}
		}
		if !claimed {
			break
		}
		rounds = round
		// A module's new claim is better than the form it holds and is settled first; where it
		// stands, the module lets the old form go.
		sort.Slice(all, func(a, b int) bool { return better(all[a], all[b]) })
		standing := newBoard()
		for i := range held {
			held[i] = -1
		}
		for _, c := range all {
			if held[c.m] < 0 && len(rivals(standing, c.m, text(c))) == 0 {
				put(standing, c)
				held[c.m] = c.r
			}
		}
	}

	// The rest: one at a time, the best claim on a form nobody holds that comes before the
	// module's own; nobody is displaced.
	for {
		holders := holding()
		best, found := claim{}, false
		for i := range held {
			for r := 0; r < before(i); r++ {
				if c := (claim{i, r}); len(rivals(holders, i, text(c))) == 0 {
					if !found || better(c, best) {
						best, found = c, true
					}
					break
				}
			}
		}
		if !found {
			break
		}
		held[best.m] = best.r
	}

	// taken: every form and stem held, for the suffixes. A suffix must not read as a blocked form
	// or a form reserved for another head either: a reader passes over the hyphen.
	takenKey, takenStem := map[string]bool{}, map[string]bool{}
	for i := range held {
		if held[i] >= 0 {
			t := lists[i][held[i]].text
			takenKey[key(t)], takenStem[stemKey(t)] = true, true
		}
	}
	free := func(i int) func(string) bool {
		p := d.parse(entries[i].module)
		return func(t string) bool {
			if takenKey[key(t)] || takenStem[stemKey(t)] || d.reservedElsewhere(t, p.seriesKey) {
				return false
			}
			_, blocked := Blocked(t, p.title)
			return !blocked
		}
	}
	take := func(t string) { takenKey[key(t)], takenStem[stemKey(t)] = true, true }

	out := make(map[string]Choice, n)
	for i := range held {
		if held[i] < 0 {
			base := lists[i][0]
			t := suffixed(base.text, free(i), 0)
			take(t)
			out[entries[i].module] = Choice{Abbrev: t, Override: base.override, Choice: len(lists[i]) + 1}
		}
	}
	// Identical titles in one program: the best claim keeps the form (with alike lists, the first
	// in priority order), the others get -b, -c … in that order.
	twins := map[string][]int{}
	for i := range held {
		if held[i] >= 0 {
			k := key(text(claim{i, held[i]}))
			twins[k] = append(twins[k], i)
		}
	}
	place := make([]int, n)
	for _, g := range twins {
		sort.Slice(g, func(a, b int) bool { return better(claim{g[a], held[g[a]]}, claim{g[b], held[g[b]]}) })
		for k, i := range g {
			place[i] = k
		}
	}
	for i, e := range entries {
		if held[i] < 0 {
			continue
		}
		c := lists[i][held[i]]
		ch := Choice{Abbrev: c.text, Override: c.override, Choice: held[i] + 1}
		if place[i] > 0 {
			ch.Abbrev, ch.Twin = suffixed(c.text, free(i), place[i]-1), true
			take(ch.Abbrev)
		}
		out[e.module] = ch
	}
	return out, rounds
}

// suffixed returns base-b, base-c … base-z, base-bb … (from the given suffix on), the first form
// free accepts. Letters, never digits, which would read as a series number and share the base's
// stem; the whole stays within 10 characters. A letter suffix is its own stem, so every holder
// rules out at most two of them and every other form free refuses at most one of each length, and
// the search ends.
func suffixed(base string, free func(string) bool, from int) string {
	for i := from; ; i++ {
		s := suffixLetters(i)
		b := base
		if runes(b)+1+len(s) > 10 {
			b = prefix(b, 10-1-len(s))
		}
		if t := b + "-" + s; free(t) {
			return t
		}
	}
}

// suffixLetters is the i-th suffix: b … z, then bb … zz, then bbb ….
func suffixLetters(i int) string {
	const letters = "bcdefghijklmnopqrstuvwxyz"
	width, count := 1, len(letters)
	for i >= count {
		i -= count
		width++
		count *= len(letters)
	}
	out := make([]byte, width)
	for k := width - 1; k >= 0; k-- {
		out[k] = letters[i%len(letters)]
		i /= len(letters)
	}
	return string(out)
}
