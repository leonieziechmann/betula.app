package abbrev

import (
	"regexp"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

type tokKind uint8

const (
	kWord  tokKind = iota // a content word
	kAcr                  // an acronym, kept whole (CAD, IT, 3D)
	kSlash                // a slash group as one unit (Jazz/Rock/Pop)
	kFunc                 // a function word
	kAnd                  // und, &, and
)

func (k tokKind) content() bool { return k <= kSlash }

type token struct {
	kind     tokKind
	text     string
	parts    string // slash: its rendering; word: the initials of its compound parts
	partsAlt string // word: the compound's initials with a known form (Software|praktikum → SWP)
	known    string // word: a known form of the whole word (BWL)
	letter   string // func, and: the letter it may leave
	lcost    int
	drop     int // cost of leaving the word out, or -1
}

type designator struct {
	text    string
	letters int // letters that count toward the abbreviation's length (IT-1 → 2)
}

var roman = map[string]int{"I": 1, "II": 2, "III": 3, "IV": 4, "V": 5, "VI": 6, "VII": 7, "VIII": 8, "IX": 9,
	"X": 10, "XI": 11, "XII": 12}

var (
	reNum  = regexp.MustCompile(`^\d{1,2}(?:\.\d)?$`)
	reCEFR = regexp.MustCompile(`^[ABC][12](?:\.\d)?$`)
	reCode = regexp.MustCompile(`^([A-ZÄÖÜ]{1,3})-?(\d{1,2})$`) // IT-1, W-3, T1
	// an acronym: 2 to 7 letters or digits, starting with a capital or digit ...
	reAcrShape = regexp.MustCompile(`^[A-ZÄÖÜ0-9][A-Za-zÄÖÜäöü0-9]{1,6}$`)
	// ... with a capital followed somewhere by another capital or a digit
	reAcrCaps = regexp.MustCompile(`[A-ZÄÖÜ].*[A-ZÄÖÜ0-9]`)
	reLetters = regexp.MustCompile(`^[A-Za-zÄÖÜäöüß]{2,}$`)
	reAnyLtr  = regexp.MustCompile(`[A-Za-zÄÖÜäöüß]`)
	reOneCap  = regexp.MustCompile(`^[A-Z]$`)

	rePossessive  = regexp.MustCompile(`['’]s([^\p{L}\p{N}_]|$)`) // Bachelor's Thesis, not Grimm'sche
	rePunct       = regexp.MustCompile(`[„“”"',;!?]`)
	reInflection  = regexp.MustCompile(`([\p{L}\p{N}_])\((?:en|n|e|s|innen|in)\)`) // Bewertung(en), Student(inn)en
	reParen       = regexp.MustCompile(`\(([^)]*)\)`)
	reParenSpaces = regexp.MustCompile(`\s*\([^)]*\)\s*`)
	reVariant     = regexp.MustCompile(`\s+(Dual|Online|online|dual)$`)
	reParenAcr    = regexp.MustCompile(`^[A-ZÄÖÜ][A-ZÄÖÜ0-9]{1,5}$`)
	// segments: „Head: Sub“, „Head - Sub“, „Head -2“ (the digit stays)
	reSegment = regexp.MustCompile(`\s*:\s*|\s+[-–]\s+|\s+[-–]\d`)
	reUmlaut  = regexp.MustCompile(`[äöüßÄÖÜ]`)
	reLower   = regexp.MustCompile(`[a-zäöüß]+`)
)

func isAcronym(s string) bool { return reAcrShape.MatchString(s) && reAcrCaps.MatchString(s) }

// compactDesignator reads a series number, a language level or a code: II → 2, B1.1 → -B1.1, IT-1 → IT1.
func compactDesignator(tok string) (designator, bool) {
	if n, ok := roman[tok]; ok {
		return designator{strconv.Itoa(n), 0}, true
	}
	if reNum.MatchString(tok) {
		return designator{tok, 0}, true
	}
	if reCEFR.MatchString(tok) {
		return designator{"-" + tok, 0}, true // DaF-B1.1, not DaFB1.1
	}
	if m := reCode.FindStringSubmatch(tok); m != nil {
		return designator{m[1] + m[2], utf8.RuneCountInString(m[1])}, true
	}
	return designator{}, false
}

// isUpper is Python's str.isupper: at least one cased letter, and no lowercase one.
func isUpper(s string) bool {
	cased := false
	for _, r := range s {
		if unicode.IsLower(r) || unicode.IsTitle(r) {
			return false
		}
		if unicode.IsUpper(r) {
			cased = true
		}
	}
	return cased
}

func firstUpper(s string) string {
	r, _ := utf8.DecodeRuneInString(s)
	return string(unicode.ToUpper(r))
}

// capitalize is Python's str.capitalize: the first letter upper, the rest lower.
func capitalize(s string) string {
	if s == "" {
		return s
	}
	r, n := utf8.DecodeRuneInString(s)
	return string(unicode.ToUpper(r)) + strings.ToLower(s[n:])
}

func prefix(s string, n int) string {
	i := 0
	for j := range s {
		if i == n {
			return s[:j]
		}
		i++
	}
	return s
}

func runes(s string) int { return utf8.RuneCountInString(s) }

func (p *parser) tokenize(seg string, desig *[]designator) []token {
	var toks []token
	raw := strings.NewReplacer("–", "-", "—", "-", "’", "'").Replace(seg)
	raw = rePossessive.ReplaceAllString(raw, "$1") // Bachelor's → Bachelor
	raw = rePunct.ReplaceAllString(raw, " ")
	for _, t := range strings.Fields(raw) {
		t = strings.Trim(t, ".")
		if t == "" {
			continue
		}
		low := strings.ToLower(t)
		if t == "&" || t == "+" {
			a := andWords[t]
			if !p.german {
				a = funcWord{"&", 40}
			}
			toks = append(toks, token{kind: kAnd, text: t, letter: a.letter, lcost: a.cost, drop: -1})
			continue
		}
		if t == "/" || t == "-" {
			continue
		}
		if d, ok := compactDesignator(t); ok && len(toks) > 0 { // a designator never opens a title
			*desig = append(*desig, d)
			continue
		}
		if reOneCap.MatchString(t) && len(*desig) > 0 && len(toks) > 0 { // „Vorbereitungskurs 1 A“
			*desig = append(*desig, designator{t, 0})
			continue
		}
		if a, ok := andWords[low]; ok {
			toks = append(toks, token{kind: kAnd, text: t, letter: a.letter, lcost: a.cost, drop: -1})
			continue
		}
		if f, ok := funcWords[low]; ok {
			toks = append(toks, token{kind: kFunc, text: t, letter: f.letter, lcost: f.cost, drop: -1})
			continue
		}
		var pieces []string
		for _, x := range strings.Split(t, "/") {
			if strings.Trim(x, "-") != "" {
				pieces = append(pieces, x)
			}
		}
		if len(pieces) > 1 && p.slashUnit(pieces) {
			// Jazz/Rock/Pop, Sorbisch/Wendisch, CAD/CAE: one unit, all initials or the first only
			allAcr := true
			for _, x := range pieces {
				allAcr = allAcr && isAcronym(x)
			}
			parts := pieces[0]
			if !allAcr {
				var b strings.Builder
				for _, x := range pieces {
					b.WriteString(firstUpper(x))
				}
				parts = b.String()
			}
			toks = append(toks, token{kind: kSlash, text: t, parts: parts, drop: -1})
			continue
		}
		group := len(pieces) > 1
		for pi, piece := range pieces {
			var sub []string
			for _, x := range strings.Split(piece, "-") {
				if x != "" {
					sub = append(sub, x)
				}
			}
			if len(sub) > 1 {
				short := true
				for _, x := range sub {
					short = short && runes(x) <= 3 && isUpper(x)
				}
				if short { // B-TU
					sub = []string{strings.Join(sub, "")}
				}
			}
			for si, x := range sub {
				// A Roman numeral inside a hyphen group is a letter (Power-to-X, not Power-to-10).
				if _, isRoman := roman[x]; !isRoman {
					if d, ok := compactDesignator(x); ok && (len(toks) > 0 || si > 0) {
						*desig = append(*desig, d)
						continue
					}
				}
				xl := strings.ToLower(x)
				if f, ok := funcWords[xl]; ok {
					toks = append(toks, token{kind: kFunc, text: x, letter: f.letter, lcost: f.cost, drop: -1})
					continue
				}
				if a, ok := andWords[xl]; ok {
					toks = append(toks, token{kind: kAnd, text: x, letter: a.letter, lcost: a.cost, drop: -1})
					continue
				}
				drop := -1
				if group && pi > 0 {
					drop = 30
				}
				first, _ := utf8.DecodeRuneInString(x)
				if isAcronym(x) || (unicode.IsDigit(first) && runes(x) <= 4) {
					toks = append(toks, token{kind: kAcr, text: x, drop: drop})
					continue
				}
				if !reAnyLtr.MatchString(x) {
					continue
				}
				tk := token{kind: kWord, text: x, known: knownForms[xl], drop: drop}
				var parts []string
				if p.german {
					parts = p.split.split(x)
				}
				if parts != nil && tk.known == "" {
					// A compound part with a known short form (software → SW) gives both the plain
					// initials (Software|systeme → SS) and the known form (SWS): ESS for
					// „Entwicklung von Softwaresystemen“, SWP for „Softwarepraktikum“.
					var init, alt strings.Builder
					for _, part := range parts {
						init.WriteString(firstUpper(part))
						if k, ok := knownForms[part]; ok {
							alt.WriteString(strings.ToUpper(k))
						} else {
							alt.WriteString(firstUpper(part))
						}
					}
					tk.parts = init.String()
					if alt.String() != tk.parts {
						tk.partsAlt = alt.String()
					}
				}
				if tk.parts == "" && tk.known == "" {
					tk.parts = camelParts(x)
				}
				toks = append(toks, tk)
			}
		}
	}
	return toks
}

// camelParts are the initials of a word written in parts (CampusTV → CTV, eBusiness → EB,
// ProTrack → PT); a part in capitals stays whole. "" for a word of one part.
func camelParts(w string) string {
	rs := []rune(w)
	var b strings.Builder
	start, pieces := 0, 0
	flush := func(end int) {
		piece := string(rs[start:end])
		if isUpper(piece) {
			b.WriteString(piece)
		} else {
			b.WriteString(firstUpper(piece))
		}
		pieces++
	}
	for i := 1; i < len(rs); i++ {
		if unicode.IsLower(rs[i-1]) && unicode.IsUpper(rs[i]) {
			flush(i)
			start = i
		}
	}
	flush(len(rs))
	if pieces < 2 {
		return ""
	}
	return b.String()
}

func (p *parser) slashUnit(pieces []string) bool {
	for _, x := range pieces {
		if !reLetters.MatchString(x) {
			return false
		}
		xl := strings.ToLower(x)
		if _, ok := funcWords[xl]; ok {
			return false
		}
		if _, ok := andWords[xl]; ok {
			return false
		}
	}
	return true
}

func isGerman(title string) bool {
	if reUmlaut.MatchString(title) {
		return true
	}
	words := map[string]bool{}
	for _, w := range reLower.FindAllString(strings.ToLower(title), -1) {
		words[w] = true
	}
	de, en := 0, 0
	for _, w := range []string{"und", "der", "die", "das", "des", "für", "in", "im", "mit", "von", "zur", "zum", "als"} {
		if words[w] {
			de++
		}
	}
	for _, w := range []string{"and", "the", "of", "for", "to", "with", "in"} {
		if words[w] {
			en++
		}
	}
	return de >= en
}

type subtitle struct {
	toks  []token
	desig []designator
}

// parsed is a title taken apart: the head (before the first „:“ or „ - “, without
// parentheses), the subtitles (fallback material only) and the designator, collected
// wherever it stands and always appended at the end.
type parsed struct {
	title        string
	german       bool
	head         []token
	subs         []subtitle
	parenAcr     []string
	desigStr     string
	desigLetters int
	headKey      string // the head's words and the designator: siblings share it
	seriesKey    string // the head's words: Systemtheorie I and II share it, Steuerungstechnik not
}

type parser struct {
	split  *splitter
	german bool
}

func parseTitle(title string, sp *splitter) *parsed {
	t := strings.Join(strings.Fields(title), " ")
	p := &parsed{title: title, german: isGerman(t)}
	ps := &parser{split: sp, german: p.german}
	t = reInflection.ReplaceAllString(t, "$1")
	var parens []string
	for _, m := range reParen.FindAllStringSubmatch(t, -1) {
		parens = append(parens, m[1])
	}
	main := strings.TrimSpace(reParenSpaces.ReplaceAllString(t, " "))
	// variant markers at the end of the head only tell variants apart (Dual, Online)
	if m := reVariant.FindStringSubmatchIndex(main); m != nil {
		parens = append(parens, main[m[2]:m[3]])
		main = main[:m[0]]
	}
	for _, x := range parens {
		if x = strings.TrimSpace(x); reParenAcr.MatchString(x) {
			p.parenAcr = append(p.parenAcr, x)
		}
	}
	var segs []string
	prev := 0
	for _, m := range reSegment.FindAllStringIndex(main, -1) {
		end := m[1]
		if r, _ := utf8.DecodeLastRuneInString(main[m[0]:m[1]]); unicode.IsDigit(r) {
			end-- // „- 2“ splits before the digit
		}
		segs = append(segs, main[prev:m[0]])
		prev = end
	}
	segs = append(segs, main[prev:])
	var kept []string
	for _, s := range segs {
		if strings.TrimSpace(s) != "" {
			kept = append(kept, s)
		}
	}
	head := main
	if len(kept) > 0 {
		head = kept[0]
	}
	var desig []designator
	p.head = ps.tokenize(head, &desig)
	for i := 1; i < len(kept); i++ {
		s := strings.TrimSpace(kept[i])
		if d, ok := compactDesignator(strings.ReplaceAll(s, " ", "")); ok {
			desig = append(desig, d)
			continue
		}
		var dd []designator
		toks := ps.tokenize(kept[i], &dd)
		if len(toks) > 0 || len(dd) > 0 {
			p.subs = append(p.subs, subtitle{toks, dd})
		}
	}
	for _, x := range parens {
		x = strings.TrimSpace(x)
		if d, ok := compactDesignator(strings.ReplaceAll(x, " ", "")); ok {
			desig = append(desig, d)
			continue
		}
		var dd []designator
		toks := ps.tokenize(x, &dd)
		if len(toks) > 0 || len(dd) > 0 {
			p.subs = append(p.subs, subtitle{toks, dd})
		}
	}
	if !hasContent(p.head) && len(p.subs) > 0 {
		// a head of function words and designators only: the first subtitle is the head
		p.head = p.subs[0].toks
		desig = append(desig, p.subs[0].desig...)
		p.subs = p.subs[1:]
	}
	var b strings.Builder
	for _, d := range desig {
		b.WriteString(d.text)
		p.desigLetters += d.letters
	}
	p.desigStr = b.String()
	words := make([]string, len(p.head))
	for i, tk := range p.head {
		words[i] = strings.ToLower(tk.text)
	}
	p.seriesKey = strings.Join(words, " ")
	p.headKey = p.seriesKey + "\x00" + strings.ToLower(p.desigStr)
	return p
}

func hasContent(toks []token) bool {
	for _, t := range toks {
		if t.kind.content() {
			return true
		}
	}
	return false
}
