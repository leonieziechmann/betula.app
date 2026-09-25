package abbrev

import (
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

// candidate is one rendering of a title with its cost; lower is better.
type candidate struct {
	text     string
	cost     int
	how      string // "", "p3", "lead", "tail", "sub", "long", "paren", "initials", "override", "prefix"
	override bool
}

// key is what two abbreviations are compared by: without case, and without the & and - that
// a reader passes over (B&B reads as BB, L&AS as LaS).
func key(s string) string { return keyReplacer.Replace(strings.ToLower(s)) }

var keyReplacer = strings.NewReplacer("&", "", "-", "")

// stemKey compares the stems (Stem): ST and ST1 have one.
func stemKey(s string) string { return key(Stem(s)) }

// lenPenalty is the price of a length, on the letters: designator digits do not count, the
// letters of a code do (IT-1 → 2). Three is the sweet spot; with a designator two letters
// are fine too (BS1, AP1).
func lenPenalty(n int, desig bool) int {
	if desig {
		switch n {
		case 0:
			return 900
		case 1:
			return 200
		case 2:
			return 40
		}
	} else {
		switch n {
		case 0:
			return 900
		case 1:
			return 300
		case 2:
			return 90
		}
	}
	switch {
	case n == 3:
		return 0
	case n <= 7:
		return 100 * (n - 3)
	}
	return 400 + 100*(n-7)
}

type headVariant struct {
	toks    []token
	cost    int
	label   string
	dropped int
}

// headVariants are the head as it stands, without a generic opening („Grundlagen der“), and
// without a trailing phrase that starts with a preposition or article (EEG drops „der Informatik“).
func headVariants(toks []token) []headVariant {
	out := []headVariant{{toks, 0, "", 0}}
	if !hasContent(toks) {
		return out
	}
	i := 0
	for i < len(toks) && toks[i].kind == kWord && genericLead[strings.ToLower(toks[i].text)] {
		i++
	}
	if i > 0 {
		j := i
		for j < len(toks) && toks[j].kind == kFunc {
			j++
		}
		if rest := toks[j:]; hasContent(rest) {
			out = append(out, headVariant{rest, 60, "lead", i})
		}
	}
	base := append([]headVariant(nil), out...)
	for _, v := range base {
		for k, t := range v.toks {
			if t.kind != kFunc || k == 0 {
				continue
			}
			after := countContent(v.toks[k:])
			if hasContent(v.toks[:k]) && after > 0 {
				label := strings.Trim(v.label+"+tail", "+")
				out = append(out, headVariant{v.toks[:k], v.cost + 50 + 20*(after-1), label, v.dropped + after})
			}
		}
	}
	return out
}

func countContent(toks []token) int {
	n := 0
	for _, t := range toks {
		if t.kind.content() {
			n++
		}
	}
	return n
}

type rendering struct {
	s     string
	cost  int
	used  int
	only  *token // the one content token used, if exactly one
	xused bool   // a first-two-letters form is in it
}

// functionLetters are the lowercase letters a reader takes for a function word between two
// capitals: u for und (AuP), f for für (SfA), d for der (GdW), i for in (EiP) ….
var functionLetters = set("u", "f", "d", "i", "v", "a", "z", "m", "o", "t")

// render enumerates the renderings of a token list. A first-two-letters form whose second
// letter is a function letter and stands before another letter costs 0.5 more: NuM for
// „Numerische Mathematik“ reads as N und M (NMa).
func render(toks []token, maxLetters int) []rendering {
	canDrop := countContent(toks) >= 4
	var res []rendering
	var rec func(i int, s string, n, cost, used, drops int, xused bool, xpos int, last *token)
	rec = func(i int, s string, n, cost, used, drops int, xused bool, xpos int, last *token) {
		if n > maxLetters || cost > 700 {
			return
		}
		if i == len(toks) {
			if xpos >= 0 {
				if rs := []rune(s); xpos+1 < len(rs) && unicode.IsLetter(rs[xpos+1]) && functionLetters[string(rs[xpos])] {
					cost += 50
				}
			}
			r := rendering{s: s, cost: cost, used: used, xused: xused}
			if used == 1 {
				r.only = last
			}
			res = append(res, r)
			return
		}
		t := &toks[i]
		if t.kind == kFunc || t.kind == kAnd {
			rec(i+1, s, n, cost, used, drops, xused, xpos, last)
			// a function letter only between two content letters
			if s != "" && t.letter != "" && hasContent(toks[i+1:]) {
				rec(i+1, s+t.letter, n+runes(t.letter), cost+t.lcost, used, drops, xused, xpos, last)
			}
			return
		}
		first := used == 0
		add := func(x string, c int, x2 bool) { rec(i+1, s+x, n+runes(x), cost+c, used+1, drops, x2, xpos, t) }
		switch t.kind {
		case kSlash:
			add(t.parts, 0, xused)
			if runes(t.parts) > 1 {
				add(firstRune(t.parts), 30, xused)
			}
		case kAcr:
			a := strings.ReplaceAll(t.text, "-", "")
			if strings.ContainsAny(a, "abcdefghijklmnopqrstuvwxyzäöü") && runes(a) > 4 { // VisuNet, CampusTV: its capitals
				var b strings.Builder
				for _, r := range a {
					if unicode.IsUpper(r) || unicode.IsDigit(r) {
						b.WriteRune(r)
					}
				}
				a = b.String()
			}
			add(a, 0, xused)
			if runes(a) > 1 {
				add(firstUpper(a), 50, xused)
			}
		default:
			w := t.text
			if t.known != "" {
				add(t.known, 0, xused)
				add(firstUpper(w), 30, xused)
			} else {
				add(firstUpper(w), 0, xused)
				if t.parts != "" {
					add(t.parts, 15, xused)
				}
				if t.partsAlt != "" {
					add(t.partsAlt, 35, xused)
				}
			}
			if wr := []rune(w); !xused && t.known == "" && len(wr) > 2 && unicode.IsLetter(wr[1]) {
				c := 80
				if first {
					c = 70
				}
				x := string(unicode.ToUpper(wr[0])) + strings.ToLower(string(wr[1]))
				rec(i+1, s+x, n+2, cost+c, used+1, drops, true, runes(s)+1, t)
			}
		}
		if t.drop >= 0 {
			rec(i+1, s, n, cost+t.drop, used, drops, xused, xpos, last)
		} else if canDrop && drops < 2 && t.kind != kSlash {
			rec(i+1, s, n, cost+120, used, drops+1, xused, xpos, last)
		}
	}
	rec(0, "", 0, 0, 0, 0, false, -1, nil)
	return res
}

func firstRune(s string) string {
	_, n := utf8.DecodeRuneInString(s)
	return s[:n]
}

type candidateSet struct {
	best  map[string]*candidate
	order []string
}

func (cs *candidateSet) add(text string, cost int, how string) {
	if text == "" {
		return
	}
	k := key(text)
	if c, ok := cs.best[k]; ok {
		if c.cost > cost {
			*c = candidate{text: text, cost: cost, how: how}
		}
		return
	}
	cs.best[k] = &candidate{text: text, cost: cost, how: how}
	cs.order = append(cs.order, k)
}

// first makes text the cheapest candidate so far: one hundredth ahead of the best other one,
// unless it is cheaper already. It takes that place without a bonus of its own, so the gap to
// the fallbacks, which the resolution weighs („a far next choice“), stays what it was.
func (cs *candidateSet) first(text, how string) {
	k := key(text)
	cost, others := 0, false
	for _, o := range cs.order {
		if c := cs.best[o]; o != k && (!others || c.cost-1 < cost) {
			cost, others = c.cost-1, true
		}
	}
	c, ok := cs.best[k]
	if !ok {
		cs.order = append(cs.order, k)
	} else if !others || c.cost < cost {
		cost = c.cost
	}
	cs.best[k] = &candidate{text: text, cost: cost, how: how}
}

// allInitials is the head written as the initials of all its words, when that makes exactly three
// characters: a content word as its capital, a function word as the lowercase letter it leaves
// (von → v, und → u, der → d, of → o; English and as &, M5). A lowercase letter only ever stands
// for a function word, and only between two capitals, so the head is three words whose first and
// last are content words: EvS, AuP, GdW, DaF, and three content words give their initials. A
// hyphen part is a word of its own (Bau- und Stadtbaugeschichte 1 → BuS1), & and + are und. A head
// with an acronym or a slash group is left to the other forms: an acronym is a short form already.
func allInitials(toks []token) (string, bool) {
	if len(toks) != 3 || toks[0].kind != kWord || toks[2].kind != kWord {
		return "", false
	}
	var b strings.Builder
	for _, t := range toks {
		switch t.kind {
		case kWord:
			b.WriteString(firstUpper(t.text))
		case kFunc, kAnd:
			l := t.letter
			if l == "" { // the, a, an: never inserted elsewhere, but here every word counts
				l = strings.ToLower(firstRune(t.text))
			}
			b.WriteString(l)
		default:
			return "", false
		}
	}
	return b.String(), runes(b.String()) == 3
}

func sortCandidates(list []candidate) {
	sort.SliceStable(list, func(i, j int) bool { return candidateLess(list[i], list[j]) })
}

// candidateLess ranks by cost, then by distance from three characters, then by length, then by text.
func candidateLess(a, b candidate) bool {
	if a.cost != b.cost {
		return a.cost < b.cost
	}
	la, lb := runes(a.text), runes(b.text)
	if da, db := abs(la-3), abs(lb-3); da != db {
		return da < db
	}
	if la != lb {
		return la < lb
	}
	return a.text < b.text
}

func abs(n int) int {
	if n < 0 {
		return -n
	}
	return n
}

type headForm struct {
	s     string
	cost  int
	label string
}

// candidates returns the ranked candidates of a parsed title. sibling ranks the forms with a
// subtitle first: another module of the program has the same head.
func (d *deriver) candidates(p *parsed, sibling bool) []candidate {
	cs := &candidateSet{best: map[string]*candidate{}}
	ds, dl, hd := p.desigStr, p.desigLetters, p.desigStr != ""
	var heads []headForm
	for _, hv := range headVariants(p.head) {
		for _, r := range render(hv.toks, 8) {
			if r.s == "" || r.used == 0 {
				continue
			}
			// One word's first two letters alone are no abbreviation (Ps, Ko); a single word
			// gives its first three (Kom), unless a series number follows (An2).
			if r.used == 1 && r.xused && p.desigStr == "" {
				continue
			}
			letters := runes(r.s)
			onlyWord := r.only != nil && r.only.kind == kWord && runes(r.only.text) >= 3
			if letters+dl < 2 && !onlyWord {
				continue
			}
			if hv.dropped <= 1 && onlyWord && r.s == firstUpper(r.only.text) {
				three := prefix(r.only.text, 3)
				s3 := three
				if !isUpper(three) {
					s3 = firstUpper(three) + strings.ToLower(three[len(firstRune(three)):])
				}
				cc := hv.cost + r.cost + 110
				label := hv.label
				if label == "" {
					label = "p3"
				}
				heads = append(heads, headForm{s3, cc, label})
				cs.add(s3+ds, cc+lenPenalty(3+dl, hd), strings.Trim(hv.label+"+p3", "+"))
			}
			if letters+dl < 2 {
				continue // a lone initial is never an abbreviation
			}
			heads = append(heads, headForm{r.s, hv.cost + r.cost, hv.label})
			cs.add(r.s+ds, hv.cost+r.cost+lenPenalty(letters+dl, hd), hv.label)
		}
	}
	// The owner's rule (2026-09-25): where the initials of all words of the head make exactly
	// three characters, that form is the first choice, ahead of compound parts and every other
	// derived form: Entwicklung von Softwaresystemen → EvS, not ESS. An acronym the title states
	// for itself and the override lines still come before it; a series number is appended.
	if s, ok := allInitials(p.head); ok {
		cs.first(s+ds, "initials")
	}
	// An acronym the title states for itself („… Resource Investigation (ANRI)“) is the first
	// choice up to five letters, but only where its letters are, in order, initials of the
	// head's words or compound parts: „Laborpraktikum der Elektrotechnik (IMT)“ names a program.
	for _, acr := range p.parenAcr {
		var seq strings.Builder
		for _, t := range p.head {
			switch t.kind {
			case kWord:
				if t.parts != "" {
					seq.WriteString(t.parts)
				} else {
					seq.WriteString(firstRune(t.text))
				}
			case kAcr:
				seq.WriteString(t.text)
			case kSlash:
				seq.WriteString(t.parts)
			}
		}
		if isSubsequence(strings.ToUpper(acr), strings.ToUpper(seq.String())) {
			if runes(acr) <= 5 {
				cs.add(acr+ds, -50, "paren")
			} else {
				cs.add(acr+ds, 30+lenPenalty(runes(acr)+dl, hd), "paren")
			}
		}
	}
	// Subtitle forms: one of the six cheapest head forms and the first letters of a subtitle.
	sort.SliceStable(heads, func(i, j int) bool { return heads[i].cost < heads[j].cost })
	var top []headForm
	seen := map[string]bool{}
	for _, h := range heads {
		if seen[key(h.s)] {
			continue
		}
		seen[key(h.s)] = true
		top = append(top, h)
		if len(top) >= 6 {
			break
		}
	}
	subBonus := 0
	if sibling {
		subBonus = -160
	}
	for si, sub := range p.subs {
		picked, sds, sl := subPicks(sub)
		for _, h := range top {
			for _, sp := range picked {
				total := runes(h.s) + runes(sp.s) + dl + sl
				pen := lenPenalty(total, hd || sds != "")
				cs.add(h.s+sp.s+ds+sds, h.cost+sp.cost+100+20*si+subBonus+pen*7/10, "sub")
			}
		}
	}
	if sibling {
		// a form without the subtitle is what the other sibling gets too
		for _, c := range cs.best {
			if c.how != "sub" {
				c.cost += 200
			}
		}
	}
	// Long forms: the initials and the last word widened.
	var words []token
	for _, t := range p.head {
		if t.kind.content() {
			words = append(words, t)
		}
	}
	from, base := 1, 200
	if len(words) == 1 {
		from, base = 2, 160
	}
	for extra := from; extra < 4; extra++ {
		var b strings.Builder
		for i, t := range words {
			switch {
			case t.kind == kWord && i == len(words)-1:
				b.WriteString(capitalize(prefix(t.text, 1+extra)))
			case t.kind == kWord:
				b.WriteString(firstUpper(t.text))
			case t.kind == kSlash:
				b.WriteString(t.parts)
			default:
				b.WriteString(t.text)
			}
		}
		s := b.String()
		cs.add(s+ds, base+100*extra+lenPenalty(runes(s)+dl, hd), "long")
	}
	out := make([]candidate, 0, len(cs.order))
	for _, k := range cs.order {
		out = append(out, *cs.best[k])
	}
	sortCandidates(out)
	return out
}

type subPick struct {
	s    string
	cost int
}

// subPicks are the letters a subtitle adds to a form, cheapest first (at most six: its
// first letter, its first two, its initials …), with the subtitle's designator and the
// letters that designator counts.
func subPicks(sub subtitle) (picked []subPick, desig string, desigLetters int) {
	var sds strings.Builder
	for _, d := range sub.desig {
		sds.WriteString(d.text)
		desigLetters += d.letters
	}
	var rs []rendering
	for _, r := range render(sub.toks, 4) {
		if r.s != "" && r.used > 0 {
			rs = append(rs, r)
		}
	}
	sort.SliceStable(rs, func(i, j int) bool {
		if rs[i].cost != rs[j].cost {
			return rs[i].cost < rs[j].cost
		}
		return runes(rs[i].s) < runes(rs[j].s)
	})
	seen := map[string]bool{}
	for _, r := range rs {
		// shortest first: the cheaper forms of one rendering
		for _, cand := range []string{prefix(r.s, 1), prefix(r.s, 2), r.s} {
			if cand != "" && !seen[key(cand)] {
				seen[key(cand)] = true
				picked = append(picked, subPick{cand, r.cost + 20*(runes(cand)-1)})
			}
		}
	}
	if len(picked) > 6 {
		picked = picked[:6]
	}
	return picked, sds.String(), desigLetters
}

// subtitleForms are what the subtitles add to an override's form for a sibling, in order:
// „Allgemeine Betriebswirtschaftslehre III: Investition und Finanzierung“ → I, IF ….
func subtitleForms(p *parsed) []subPick {
	var out []subPick
	for si, sub := range p.subs {
		picked, sds, _ := subPicks(sub)
		for _, sp := range picked {
			out = append(out, subPick{sp.s + sds, sp.cost + 20*si})
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].cost < out[j].cost })
	return out
}

func isSubsequence(needle, hay string) bool {
	h := []rune(hay)
	i := 0
	for _, r := range needle {
		for i < len(h) && h[i] != r {
			i++
		}
		if i == len(h) {
			return false
		}
		i++
	}
	return true
}
