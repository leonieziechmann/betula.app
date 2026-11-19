package privacy

import (
	"regexp"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

// folds maps the letters with diacritics of the Latin scripts to their base letters, after
// unicode.ToLower: „Müller", „Muller" and „MÜLLER" are one name to the patterns.
var folds = map[rune]string{
	'à': "a", 'á': "a", 'â': "a", 'ã': "a", 'ä': "a", 'å': "a", 'ā': "a", 'ă': "a", 'ą': "a",
	'æ': "ae", 'ç': "c", 'ć': "c", 'č': "c", 'ĉ': "c", 'ď': "d", 'đ': "d", 'ð': "d",
	'è': "e", 'é': "e", 'ê': "e", 'ë': "e", 'ē': "e", 'ė': "e", 'ę': "e", 'ě': "e",
	'ğ': "g", 'ģ': "g", 'ì': "i", 'í': "i", 'î': "i", 'ï': "i", 'ī': "i", 'į': "i", 'ı': "i",
	'ķ': "k", 'ł': "l", 'ľ': "l", 'ĺ': "l", 'ļ': "l", 'ñ': "n", 'ń': "n", 'ň': "n", 'ņ': "n",
	'ò': "o", 'ó': "o", 'ô': "o", 'õ': "o", 'ö': "o", 'ø': "o", 'ō': "o", 'ő': "o", 'œ': "oe",
	'ř': "r", 'ŕ': "r", 'ś': "s", 'š': "s", 'ş': "s", 'ș': "s", 'ß': "ss", 'ť': "t", 'ţ': "t", 'ț': "t",
	'ù': "u", 'ú': "u", 'û': "u", 'ü': "u", 'ū': "u", 'ů': "u", 'ű': "u", 'ų': "u",
	'ý': "y", 'ÿ': "y", 'ź': "z", 'ż': "z", 'ž': "z", 'þ': "th",
}

// fold returns s in lower case, without diacritics (ö → o, é → e, ß → ss) and with every kind
// of space a plain one, and for every byte of the result the offset in s of the rune it came
// from, ending with len(s): a match [i, j) of the result is s[at[i]:at[j]].
func fold(s string) (string, []int) {
	var b strings.Builder
	b.Grow(len(s))
	at := make([]int, 0, len(s)+1)
	for i, r := range s {
		if unicode.Is(unicode.Mn, r) { // a combining mark (a decomposed „ö"): its letter stands for it
			continue
		}
		r = unicode.ToLower(r)
		var out string
		switch f, ok := folds[r]; {
		case ok:
			out = f
		case unicode.IsSpace(r):
			out = " "
		default:
			out = string(r)
		}
		for range len(out) {
			at = append(at, i)
		}
		b.WriteString(out)
	}
	return b.String(), append(at, len(s))
}

// folded is fold without the offsets.
func folded(s string) string {
	f, _ := fold(s)
	return f
}

// translit writes the German umlauts as they are written without them („Müller" → „Mueller"),
// the way addresses and older texts spell a name.
var translit = strings.NewReplacer("ä", "ae", "ö", "oe", "ü", "ue", "Ä", "Ae", "Ö", "Oe", "Ü", "Ue", "ß", "ss")

// variants are the folded spellings of a name: as written, and with its umlauts transliterated.
func variants(name string) []string {
	seen := map[string]bool{}
	var out []string
	for _, v := range []string{folded(name), folded(translit.Replace(name))} {
		v = strings.Join(strings.Fields(v), " ")
		if v != "" && !seen[v] {
			seen[v] = true
			out = append(out, v)
		}
	}
	return out
}

// alternation is a group of the spellings, the longest first, each word of a spelling apart
// from the next by spaces or hyphens („Müller-Lüdenscheidt" and „Müller Lüdenscheidt" alike).
func alternation(spellings []string) string {
	seen := map[string]bool{}
	var alts []string
	for _, s := range spellings {
		words := strings.FieldsFunc(s, func(r rune) bool { return r == ' ' || r == '-' })
		if len(words) == 0 {
			continue
		}
		for i, w := range words {
			words[i] = regexp.QuoteMeta(w)
		}
		alt := strings.Join(words, `[ -]+`)
		if !seen[alt] {
			seen[alt] = true
			alts = append(alts, alt)
		}
	}
	sort.Slice(alts, func(i, j int) bool {
		if len(alts[i]) != len(alts[j]) {
			return len(alts[i]) > len(alts[j])
		}
		return alts[i] < alts[j]
	})
	return "(?:" + strings.Join(alts, "|") + ")"
}

// The academic titles and salutations that stand before a name. „prof" and „dr" without their
// dot too; the rest only with it, so that a word of a sentence is never taken for a title.
const (
	titleWord  = `(?:univ\.|prof\.?|dr\.?|habil\.|pd|apl\.|jun\.|juniorprof\.|hon\.|dipl\.|ing\.|rer\.|nat\.|pol\.|phil\.|oec\.|med\.|paed\.|jur\.|techn\.|inf\.|math\.|phys\.|kfm\.|wirt\.|h\. ?c\.|m\. ?sc\.|m\. ?a\.|m\. ?eng\.|b\. ?sc\.|b\. ?a\.|b\. ?eng\.|msc|mba|ph\. ?d\.)`
	titles     = `(?:` + titleWord + `[ -]*)+`
	salutation = `(?:herrn?|frau|hr\.|fr\.|mr\.?|mrs\.?|ms\.?)`
	// boundaries: a name is never a part of a word, and a hyphen joins it to another name
	// („Mustermann-Schulz" is somebody else). Go's regexp has no look-behind, so the character
	// before and after is matched and the name is group 1.
	before = `(?:^|[^\pL\pN-])(`
	after  = `)(?:[^\pL\pN-]|$)`
)

// emailPattern finds every address in folded text, also those written to keep harvesters off
// („max.mustermann(at)b-tu.de", „[at]", „(dot)"); whose it is, the local part says.
var (
	emailPattern = regexp.MustCompile(`[a-z0-9._%+-]+ ?(?:@|\(at\)|\[at\]|\{at\}) ?[a-z0-9-]+(?:(?:\.|\(dot\)|\[dot\])[a-z0-9-]+)+`)
	atPattern    = regexp.MustCompile(` ?(?:@|\(at\)|\[at\]|\{at\}) ?`)
)

// hasAt reports whether a folded text may hold an address.
func hasAt(f string) bool {
	return strings.Contains(f, "@") || strings.Contains(f, "(at)") || strings.Contains(f, "[at]") || strings.Contains(f, "{at}")
}

// codeValue is a value of an enumeration („winter", „block", „nord", „week_a"): never a name, even
// where it is a person's surname. Names are written with a capital.
var codeValue = regexp.MustCompile(`^[a-z0-9_]+$`)

// person is the compiled forms of one person of the register.
type person struct {
	// text finds the person in free text: given name (or initial) and surname in either order,
	// a title or salutation before the surname, and the further forms of the register.
	text *regexp.Regexp
	// strict finds the person in a list of names (module_person, event_person): surname and
	// given name, nothing less, so that a namesake keeps their row.
	strict *regexp.Regexp
	// bare is the surname alone, for the rows that belong to a module or an event of the person,
	// where it means them („Dozent: Mustermann"), and for the columns of names where no namesake
	// with another given name stands in the lists.
	bare *regexp.Regexp
	// surnameOnly is a row of a list of names that holds the surname alone, as the event search
	// writes them („Mustermann").
	surnameOnly *regexp.Regexp
	// middle finds given name, further given names and surname („Max Peter Mustermann"); a match
	// counts where the further names are written with a capital (group 2), not „Max und Mustermann".
	middle *regexp.Regexp
	// surnames and givens are the folded spellings, for telling a namesake.
	surnames, givens []string
	// locals are the local parts of the person's addresses (max.mustermann, m.mustermann, mustermann).
	locals map[string]bool
	// needles: a folded text that contains none of them cannot name the person.
	needles []string
}

func compile(p Person) *person {
	surnames := variants(p.Surname)
	var givens []string
	for _, g := range variants(p.Given) {
		givens = append(givens, g)
		if first := strings.FieldsFunc(g, func(r rune) bool { return r == ' ' || r == '-' }); len(first) > 1 {
			givens = append(givens, first[0]) // „Max" of „Max Peter"
		}
	}
	initials := map[string]bool{}
	for _, g := range givens {
		parts := strings.FieldsFunc(g, func(r rune) bool { return r == ' ' || r == '-' })
		var b strings.Builder
		for i, part := range parts {
			if i > 0 {
				b.WriteString(`(?:[ -]*`)
			}
			b.WriteString(regexp.QuoteMeta(string([]rune(part)[0])) + `\.`)
		}
		for i := 1; i < len(parts); i++ {
			b.WriteString(`)?`)
		}
		initials[b.String()] = true
	}
	var inits []string
	for i := range initials {
		inits = append(inits, i)
	}
	sort.Strings(inits)

	S := alternation(surnames)
	G := alternation(givens)
	I := "(?:" + strings.Join(inits, "|") + ")"
	middle := `(?: [a-z]\.)*` // „Max P. Mustermann"
	pre := `(?:` + salutation + ` +)?(?:` + titles + `)?`

	forms := []string{
		pre + G + middle + ` +` + S,                                      // Prof. Dr. Max Mustermann, Max P. Mustermann
		pre + I + `(?: ?[a-z]\.)* *` + S,                                 // M. Mustermann, M.Mustermann, M. P. Mustermann
		S + ` *, *(?:` + G + `|` + I + `)(?: *, *` + titles + `)?`,       // Mustermann, Max , Prof. Dr.
		S + ` +(?:` + G + `|` + I + `)`,                                  // Mustermann Max, Mustermann M.
		`(?:` + salutation + ` +(?:` + titles + `)?|` + titles + `)` + S, // Prof. Mustermann, Herr Mustermann
	}
	// The longest word of each spelling of the surname: „Müller Lüdenscheidt" is written with a
	// hyphen or without, „lüdenscheidt" stands in both.
	var needles []string
	for _, s := range surnames {
		longest := ""
		for _, w := range strings.FieldsFunc(s, func(r rune) bool { return r == ' ' || r == '-' }) {
			if len(w) > len(longest) {
				longest = w
			}
		}
		needles = append(needles, longest)
	}
	for _, f := range p.Forms {
		f = strings.TrimSpace(f)
		if f == "" || strings.Contains(f, "@") {
			continue
		}
		ff := strings.Join(strings.Fields(folded(f)), " ")
		forms = append(forms, regexp.QuoteMeta(ff))
		needles = append(needles, ff)
	}

	locals := map[string]bool{}
	for _, s := range surnames {
		for _, sv := range []string{strings.NewReplacer(" ", "", "-", "").Replace(s), strings.NewReplacer(" ", "-").Replace(s)} {
			locals[sv] = true
			for _, g := range givens {
				for _, gv := range []string{strings.NewReplacer(" ", "", "-", "").Replace(g), strings.NewReplacer(" ", "-").Replace(g)} {
					for _, sep := range []string{".", "_", "-", ""} {
						locals[gv+sep+sv] = true
						locals[sv+sep+gv] = true
					}
					locals[string([]rune(gv)[0])+"."+sv] = true
					locals[string([]rune(gv)[0])+sv] = true
				}
			}
		}
	}
	for _, f := range p.Forms {
		if at := strings.Index(f, "@"); at > 0 {
			locals[folded(strings.TrimSpace(f[:at]))] = true
		}
	}

	return &person{
		text:        regexp.MustCompile(before + strings.Join(forms, "|") + after),
		strict:      regexp.MustCompile(before + G + middle + ` +` + S + `|` + S + ` *, *` + G + after),
		bare:        regexp.MustCompile(before + S + after),
		surnameOnly: regexp.MustCompile(`^ *` + S + ` *(?:, *` + titles + `)? *$`),
		middle:      regexp.MustCompile(before + pre + G + `((?: [a-z][a-z'-]*){1,2}) +` + S + after),
		locals:      locals,
		needles:     needles,
		surnames:    surnames,
		givens:      givens,
	}
}

// span is a part [start, end) of a folded text.
type span struct{ start, end int }

// all returns every match of group 1 of re in f, again from each end: a boundary the match took
// cannot begin the next name („Mustermann,Musterfrau").
func all(re *regexp.Regexp, f string) []span {
	var out []span
	for pos := 0; pos < len(f); {
		m := re.FindStringSubmatchIndex(f[pos:])
		if m == nil {
			break
		}
		out = append(out, span{pos + m[2], pos + m[3]})
		pos += m[3]
	}
	return out
}

// mentions returns where the person stands in the text s, folded f with its offsets at; bare also
// finds the surname alone, but never in a value of an enumeration.
func (p *person) mentions(s, f string, at []int, bare bool) []span {
	found := false
	for _, n := range p.needles {
		if strings.Contains(f, n) {
			found = true
			break
		}
	}
	var out []span
	if found {
		out = append(out, all(p.text, f)...)
		out = append(out, p.middleNames(s, f, at)...)
		if bare && !codeValue.MatchString(s) {
			out = append(out, all(p.bare, f)...)
		}
	}
	if hasAt(f) {
		for _, m := range emailPattern.FindAllStringIndex(f, -1) {
			sep := atPattern.FindStringIndex(f[m[0]:m[1]])
			if sep != nil && p.locals[f[m[0]:m[0]+sep[0]]] {
				out = append(out, span{m[0], m[1]})
			}
		}
	}
	return out
}

// middleNames finds given name, further given names and surname where every further name begins
// with a capital in the text itself.
func (p *person) middleNames(s, f string, at []int) []span {
	var out []span
	for pos := 0; pos < len(f); {
		m := p.middle.FindStringSubmatchIndex(f[pos:])
		if m == nil {
			break
		}
		ok := true
		mid := f[pos+m[4] : pos+m[5]]
		offset := pos + m[4]
		for _, w := range strings.Fields(mid) {
			i := strings.Index(f[offset:], w) + offset
			offset = i + len(w)
			r, _ := utf8.DecodeRuneInString(s[at[i]:])
			if !unicode.IsUpper(r) {
				ok = false
				break
			}
		}
		if ok {
			out = append(out, span{pos + m[2], pos + m[3]})
		}
		pos += m[3]
	}
	return out
}

// merge sorts spans and joins those that overlap or touch.
func merge(spans []span) []span {
	if len(spans) < 2 {
		return spans
	}
	sort.Slice(spans, func(i, j int) bool { return spans[i].start < spans[j].start })
	out := spans[:1]
	for _, s := range spans[1:] {
		last := &out[len(out)-1]
		if s.start <= last.end {
			last.end = max(last.end, s.end)
			continue
		}
		out = append(out, s)
	}
	return out
}

// Mentions counts where the persons stand in a text, by the rules for free text (not the surname
// alone): for the raw archive, whose pages are no rows of a module or an event.
func Mentions(text string, people []Person) int {
	f, at := fold(text)
	n := 0
	for _, p := range people {
		n += len(merge(compile(p).mentions(text, f, at, false)))
	}
	return n
}
