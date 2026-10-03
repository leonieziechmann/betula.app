package abbrev

import (
	"regexp"
	"sort"
	"strings"
	"unicode/utf8"
)

// Costs are hundredths: 15 is the 0.15 of docs/radix/schema-v2.md („Short names“). Integers
// keep ties ties, where floats would break them by rounding noise.

// funcWord is a function word: the lowercase letter it may leave in an abbreviation
// („und“ → u in AuP, „als“ → a in DaF) and what inserting it costs. An empty letter is
// never inserted.
type funcWord struct {
	letter string
	cost   int
}

var funcWords = func() map[string]funcWord {
	m := map[string]funcWord{}
	for _, w := range []string{"der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines", "einem", "einen"} {
		l := "e"
		if w[0] == 'd' {
			l = "d"
		}
		m[w] = funcWord{l, 60}
	}
	for _, p := range [][2]string{
		{"in", "i"}, {"im", "i"}, {"ins", "i"}, {"für", "f"}, {"von", "v"}, {"vom", "v"},
		{"zu", "z"}, {"zur", "z"}, {"zum", "z"}, {"mit", "m"}, {"an", "a"}, {"am", "a"},
		{"auf", "a"}, {"aus", "a"}, {"bei", "b"}, {"über", "ü"}, {"unter", "u"},
		{"nach", "n"}, {"durch", "d"}, {"ohne", "o"}, {"gegen", "g"},
		{"zwischen", "z"}, {"vor", "v"}, {"um", "u"}, {"oder", "o"}, {"bzw", "b"},
		{"sowie", "u"}, {"inkl", "i"}, {"of", "o"}, {"for", "f"}, {"to", "t"}, {"on", "o"},
		{"with", "w"}, {"at", "a"}, {"by", "b"}, {"from", "f"}, {"into", "i"}, {"as", "a"},
		{"or", "o"}, {"de", "d"}, {"du", "d"}, {"la", "l"}, {"le", "l"}, {"del", "d"},
		{"y", "y"}, {"within", "w"}, {"towards", "t"}, {"via", "v"}, {"per", "p"},
	} {
		m[p[0]] = funcWord{p[1], 50}
	}
	// Articles and the English possessives that stand where an article would: no letter of their
	// own, and the phrase they open is a tail that may be left out, as „der Informatik“ is in EEG
	// („Industrial Heating Systems and their Defossilization“ → IHS, was IHSTD).
	for _, w := range []string{"the", "a", "an", "their", "its", "his", "her", "our", "your"} {
		m[w] = funcWord{"", 990}
	}
	m["als"] = funcWord{"a", 30} // DaF, DaZ
	return m
}()

// andWords join two content words: „und“ as u (AuP), English „and“ as & (A&M).
var andWords = map[string]funcWord{
	"und": {"u", 30}, "&": {"u", 30}, "and": {"&", 40}, "et": {"&", 40}, "+": {"u", 30},
}

// genericLead are opening words a title can do without („Grundlagen der Rechnernetze“).
var genericLead = set("grundlagen", "grundzüge", "einführung", "allgemeine", "ausgewählte", "kapitel",
	"spezielle", "themen", "introduction", "fundamentals", "foundations", "basics",
	"principles", "selected", "topics", "special", "aktuelle")

// knownForms beat initials, as a whole word and as a compound part.
var knownForms = map[string]string{
	"betriebswirtschaftslehre": "BWL", "volkswirtschaftslehre": "VWL",
	"software": "SW", "hardware": "HW",
}

// KnownFormsIn returns the known short forms of the words of title that are a word they stand
// for or a compound of it, sorted: BWL for „Allgemeine Betriebswirtschaftslehre“, SW for
// „Softwaresysteme“. The search finds such a module by its short form as well (docs/radix/schema-v2.md,
// „Search“).
func KnownFormsIn(title string) []string {
	seen := map[string]bool{}
	for _, w := range vocabularyWord.FindAllString(title, -1) {
		w = strings.ToLower(w)
		for long, short := range knownForms {
			if strings.Contains(w, long) {
				seen[short] = true
			}
		}
	}
	forms := make([]string, 0, len(seen))
	for form := range seen {
		forms = append(forms, form)
	}
	sort.Strings(forms)
	return forms
}

// curatedHeads are compound parts the title vocabulary lacks as standalone words.
var curatedHeads = set("rechnung", "lehre", "banken", "bank", "kunde", "wesen", "wirtschaft", "technik", "systeme", "system",
	"theorie", "praktikum", "netze", "recht", "planung", "führung", "gestaltung", "entwicklung",
	"verarbeitung", "analyse", "mechanik", "dynamik", "statik", "chemie", "physik", "biologie",
	"elemente", "maschinen", "stoffe", "anlagen", "prozesse", "verfahren", "messung", "methoden",
	"modelle", "modellierung", "simulation", "steuerung", "regelung", "versorgung", "konstruktion",
	"geschichte", "pädagogik", "didaktik", "psychologie", "soziologie", "politik", "ökonomie",
	"ökonomik", "forschung", "projekt", "seminar", "labor", "arbeit", "sprache", "sprachen", "kurs",
	// frequent first parts
	"makro", "echtzeit", "fach", "ober", "haupt", "neben", "grund", "daten", "kriminal", "kodierung",
	// tails (Produktions|automatisierung → PA, Betriebs|festigkeit → BF, Material|auswahl)
	"automatisierung", "umwandlung", "übertragung", "pflege", "festigkeit", "wasserbau", "auswahl")

// openHeads end so many compounds that the part before them need not be a word the
// vocabulary knows: Deponie|technik → DT, Mehrgrößen|regelung → MR, Pflicht|praktikum → PP.
// Only where no other split exists, only after four letters or more with a vowel, and not
// after a prefix that belongs to the head (Umsatz|be|steuerung is Umsatz|besteuerung).
var openHeads = set("technik", "systeme", "system", "theorie", "praktikum", "netze", "planung", "führung",
	"gestaltung", "entwicklung", "verarbeitung", "analyse", "mechanik", "dynamik", "statik", "chemie", "physik",
	"biologie", "elemente", "maschinen", "anlagen", "prozesse", "verfahren", "messung", "methoden", "modelle",
	"modellierung", "simulation", "steuerung", "regelung", "versorgung", "konstruktion", "geschichte",
	"pädagogik", "didaktik", "psychologie", "soziologie", "politik", "ökonomie", "ökonomik", "forschung",
	"projekt", "seminar", "labor", "automatisierung", "umwandlung", "übertragung", "pflege", "festigkeit",
	"wasserbau", "auswahl")

// noPart is never a compound part on its own.
var noPart = set("trans", "inter", "multi", "poly", "mono", "para", "meta", "proto", "super", "ultra", "sozio",
	"ische", "ischen", "ungen", "heit", "keit", "lich", "liche", "lichen", "ation",
	"logisch", "logische", "logischer", "logischen", "logisches")

// boundBlock are prefixes that are never a mined bound head.
var boundBlock = set("über", "unter", "nach", "durch", "gegen", "wider", "hinter", "inter", "vorder", "zusammen",
	"weiter", "wieder", "nicht", "einführung", "trans", "integ", "gene", "wiss", "sich", "fest",
	"praxi", "inte", "kon", "kom", "pro", "infra")

// noSplit look like compounds but are names (Deutsch|land, Frank|reich).
var noSplit = set("deutschland", "frankreich", "russland", "finnland", "england", "griechenland", "lausitz",
	"rumänien", "österreich")

// shortParts are the only three-letter compound parts.
var shortParts = set("bau", "bio", "geo", "öko", "neu", "alt", "ton", "tag", "rat", "bad", "gas", "eis", "öl")

var onsetOK = set("bl", "br", "ch", "dr", "fl", "fr", "gl", "gr", "kl", "kn", "kr", "pf", "ph", "pl", "pr", "sc",
	"sh", "sk", "sl", "sm", "sn", "sp", "st", "sw", "sz", "th", "tr", "ts", "tw", "wr", "zw", "qu",
	"gn", "ps", "rh", "cl", "cr", "sy")

func set(words ...string) map[string]bool {
	m := make(map[string]bool, len(words))
	for _, w := range words {
		m[w] = true
	}
	return m
}

var vocabularyWord = regexp.MustCompile(`[A-Za-zÄÖÜäöüß]+`)

// Vocabulary returns the words the splitter knows: every word of three or more letters of
// the given titles, lowercased, without function words, plus the curated heads.
func Vocabulary(titles []string) map[string]bool {
	v := make(map[string]bool, len(curatedHeads)+8192)
	for _, t := range titles {
		for _, w := range vocabularyWord.FindAllString(t, -1) {
			w = strings.ToLower(w)
			if utf8.RuneCountInString(w) < 3 {
				continue
			}
			if _, ok := funcWords[w]; ok {
				continue
			}
			if _, ok := andWords[w]; ok {
				continue
			}
			v[w] = true
		}
	}
	for h := range curatedHeads {
		v[h] = true
	}
	return v
}

// splitter splits German compounds with the catalog's own title vocabulary.
type splitter struct {
	vocab map[string]bool
	bound map[string]bool
	cache map[string][]string
}

func newSplitter(vocab map[string]bool) *splitter {
	s := &splitter{vocab: vocab, bound: map[string]bool{}, cache: map[string][]string{}}
	// Mined bound heads: a prefix of five or more letters that starts three or more title
	// words whose remainder is itself a word (nachrichten|technik, nachrichten|systeme,
	// nachrichten|übertragung) is a compound part, although it never stands alone.
	heads := map[string]map[string]bool{}
	for w := range vocab {
		r := []rune(w)
		if len(r) < 9 {
			continue
		}
		for i := 5; i < len(r)-3; i++ {
			h, rest := string(r[:i]), string(r[i:])
			if (vocab[rest] || curatedHeads[rest]) && len(r)-i >= 4 && !boundBlock[h] {
				if heads[h] == nil {
					heads[h] = map[string]bool{}
				}
				heads[h][w] = true
			}
		}
	}
	for h, ws := range heads {
		if len(ws) >= 3 {
			s.bound[h] = true
		}
	}
	return s
}

var vowels = set("a", "e", "i", "o", "u", "y", "ä", "ö", "ü")

func hasVowel(part []rune) bool {
	for _, r := range part {
		if vowels[string(r)] {
			return true
		}
	}
	return false
}

func (s *splitter) onsetOK(part []rune) bool {
	return vowels[string(part[0])] || vowels[string(part[1])] || onsetOK[string(part[:2])]
}

var fugenS = regexp.MustCompile(`(ung|heit|keit|schaft|ion|tät|ling|tum)s$`)

// reBoundPrefix: a part that ends in an unstressed verb prefix, which belongs to what follows.
var reBoundPrefix = regexp.MustCompile(`(be|ge|ver|ent|zer)$`)

// quality of a compound part, in tenths: 0 = not a part, 20 = a vocabulary word,
// 15 = a word with a linking element, 10 = by rule.
func (s *splitter) quality(part []rune, final bool) int {
	p := string(part)
	if len(part) < 3 || noPart[p] || !s.onsetOK(part) {
		return 0
	}
	if len(part) == 3 && !shortParts[p] {
		return 0
	}
	if s.vocab[p] {
		return 20
	}
	if !final && s.bound[p] {
		return 10
	}
	if final {
		// a final part with an inflection ending (the ending is not checked, only cut)
		for _, k := range []int{1, 2, 1, 1, 2} {
			if len(part)-k >= 4 && s.vocab[string(part[:len(part)-k])] {
				return 15
			}
		}
		return 0
	}
	for _, e := range []string{"s", "es", "n", "en", "e", "er"} {
		if strings.HasSuffix(p, e) {
			stem := []rune(strings.TrimSuffix(p, e))
			if len(stem) >= 4 && s.vocab[string(stem)] {
				return 15
			}
		}
	}
	// Fugen-s after a noun suffix is reliable (leistungs|, wahrscheinlichkeits|, informations|).
	if len(part) >= 6 && fugenS.MatchString(p) {
		return 10
	}
	// truncated stem (programmier|praktikum)
	for _, e := range []string{"e", "en", "n", "ung", "ie"} {
		if s.vocab[p+e] {
			return 10
		}
	}
	return 0
}

// split returns the parts of a compound, or nil. Every split into two or three parts is
// scored by the quality of its parts minus 1.6 per part; fewer, better parts win.
func (s *splitter) split(word string) []string {
	w := strings.ToLower(word)
	if noSplit[w] {
		return nil
	}
	if parts, ok := s.cache[w]; ok {
		return parts
	}
	r := []rune(w)
	n := len(r)
	q := func(i, j int) int { return s.quality(r[i:j], j == n) }

	var best []int // cut positions
	bestScore, bestLast := 0, 0
	consider := func(score, last int, cuts ...int) {
		if best == nil || score > bestScore || (score == bestScore && last > bestLast) {
			best = append([]int(nil), cuts...)
			bestScore, bestLast = score, last
		}
	}
	// The order is that of a depth-first walk over the first cut, then the second; the
	// first of equally good splits wins.
	for j1 := 3; j1 < n; j1++ {
		q1 := q(0, j1)
		if q1 == 0 {
			continue
		}
		for j2 := j1 + 3; j2 <= n; j2++ {
			q2 := q(j1, j2)
			if q2 == 0 {
				continue
			}
			if j2 == n {
				consider(q1+q2-32, n-j1, j1)
				continue
			}
			if n-j2 >= 3 {
				if q3 := q(j2, n); q3 != 0 {
					consider(q1+q2+q3-48, n-j2, j1, j2)
				}
			}
		}
	}
	if best == nil {
		// no split of known parts: an unknown part before an open head (Deponie|technik), at 0.5
		for j1 := 4; j1 <= n-4; j1++ {
			pre := r[:j1]
			if openHeads[string(r[j1:])] && s.onsetOK(pre) && hasVowel(pre) && !noPart[string(pre)] && !boundBlock[string(pre)] &&
				!reBoundPrefix.MatchString(string(pre)) {
				consider(5+20-32, n-j1, j1)
				break
			}
		}
	}
	var parts []string
	if best != nil {
		prev := 0
		for _, c := range append(best, n) {
			parts = append(parts, string(r[prev:c]))
			prev = c
		}
	}
	s.cache[w] = parts
	return parts
}
