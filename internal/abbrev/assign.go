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
// that lost its form moves on and may displace a weaker holder.
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
//
//  1. Claim: every module claims its best candidate. Where claims conflict, the higher matching
//     score keeps the form; a tie goes to the module first in priority order. This (score, then
//     priority) is a strict total order on claims, so every contest has one winner.
//  2. Cascade, at most three rounds: every module without a form moves on to its next candidate
//     it can win — one nobody holds, or one whose holders all have a lower score for theirs
//     (the tie again to priority) — and claims it. The claims of a round and the forms held are
//     settled together, best first: a claim stands unless a better one it conflicts with stood
//     before it. A holder that loses is displaced and moves on in the next round.
//  3. Rest: every module still without a form takes, best claim first, its best candidate that
//     conflicts with no form held. A module whose list has none left gets its first candidate
//     with a letter suffix (-b … -z, -bb …) whose form and stem nobody holds.
//
// It terminates, whatever the lists: the claim and the cascade are at most four passes, each
// over finitely many modules and candidates (a module's position in its list only moves
// forward, so no module tries a candidate twice and no displacement can repeat); the rest
// assigns one module per step; and the suffix search ends because a holder blocks at most two
// suffixes (its form and its stem), and there are 25 + 25² + 25³ suffixes against fewer holders
// than a program has modules. A naive cascade that let a displaced module start again from its
// best candidate, or let the newcomer win a tie, could loop (A takes X from B, B takes it back);
// here neither happens. The result depends only on the entries and their lists.
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
	better := func(a, b claim) bool {
		if sa, sb := score(lists[a.m][a.r]), score(lists[b.m][b.r]); sa != sb {
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
	pos := make([]int, n)  // the next rank a module without a form may claim
	for i := range held {
		held[i] = -1
	}
	rounds := 0
	for round := 0; round <= cascadeRounds; round++ {
		holders := newBoard()
		var all []claim
		for i := range held {
			if held[i] >= 0 {
				c := claim{i, held[i]}
				put(holders, c)
				all = append(all, c)
			}
		}
		claimed := false
		for i := range held {
			if held[i] >= 0 {
				continue
			}
			// the next candidate this module can win against the forms held
			for ; pos[i] < len(lists[i]); pos[i]++ {
				c, wins := claim{i, pos[i]}, true
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
		}
		if !claimed {
			break
		}
		rounds = round
		sort.Slice(all, func(a, b int) bool { return better(all[a], all[b]) })
		standing := newBoard()
		for i := range held {
			held[i] = -1
		}
		for _, c := range all {
			if len(rivals(standing, c.m, text(c))) == 0 {
				put(standing, c)
				held[c.m] = c.r
			} else {
				pos[c.m] = c.r + 1 // lost, or displaced: it moves on
			}
		}
	}

	// The rest: best claim first, each takes its best candidate that conflicts with nothing held.
	holders := newBoard()
	for i := range held {
		if held[i] >= 0 {
			put(holders, claim{i, held[i]})
		}
	}
	free := func(i int) (claim, bool) {
		for r := range lists[i] {
			if c := (claim{i, r}); len(rivals(holders, i, text(c))) == 0 {
				return c, true
			}
		}
		return claim{}, false
	}
	var exhausted []int
	for {
		best, found := claim{}, false
		for i := range held {
			if held[i] >= 0 {
				continue
			}
			if c, ok := free(i); ok && (!found || better(c, best)) {
				best, found = c, true
			}
		}
		if !found {
			break
		}
		held[best.m] = best.r
		put(holders, best)
	}
	for i := range held {
		if held[i] < 0 {
			exhausted = append(exhausted, i)
		}
	}

	// taken: every form and stem held, for the suffixes.
	takenKey, takenStem := map[string]bool{}, map[string]bool{}
	for i := range held {
		if held[i] >= 0 {
			t := lists[i][held[i]].text
			takenKey[key(t)], takenStem[stemKey(t)] = true, true
		}
	}
	isFree := func(t string) bool { return !takenKey[key(t)] && !takenStem[stemKey(t)] }
	take := func(t string) { takenKey[key(t)], takenStem[stemKey(t)] = true, true }

	out := make(map[string]Choice, n)
	for _, i := range exhausted {
		base := lists[i][0]
		t := suffixed(base.text, isFree, 0)
		take(t)
		out[entries[i].module] = Choice{Abbrev: t, Override: base.override, Choice: len(lists[i]) + 1}
	}
	// Identical titles in one program: the first in priority order keeps the form, the others
	// get -b, -c ….
	seen := map[string]int{}
	for i, e := range entries {
		if held[i] < 0 {
			continue
		}
		c := lists[i][held[i]]
		ch := Choice{Abbrev: c.text, Override: c.override, Choice: held[i] + 1}
		k := key(c.text)
		if count, ok := seen[k]; ok {
			seen[k] = count + 1
			ch.Abbrev, ch.Twin = suffixed(c.text, isFree, count), true
			take(ch.Abbrev)
		} else {
			seen[k] = 0
		}
		out[e.module] = ch
	}
	return out, rounds
}

// suffixed returns base-b, base-c … base-z, base-bb … (from the given suffix on), the first form
// free says nobody holds. Letters, never digits, which would read as a series number and share
// the base's stem; the whole stays within 10 characters. A letter suffix is its own stem, so
// every holder rules out at most two of them, and the search ends.
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
