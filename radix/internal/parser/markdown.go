package parser

import (
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// Markdown reads a free text of a module description (learning outcomes, contents, assessment,
// prerequisites, remarks) as CommonMark, the form the catalog keeps these texts in
// (docs/radix/schema-v2.md §3, „Module texts"; owner, 2026-10-01: „dass in der Datenbank markdown
// liegt"). Folia sets them from it: paragraphs as paragraphs, lists as lists.
//
// What the page marks up is taken as it is: <p> and blank lines (<br><br>, a line of &nbsp;)
// end paragraphs, <ul> and <ol> are lists, <b>, <strong> and underlining are strong (the pages
// underline their headings), <i> and <em> emphasis. What the page only types is read from its
// lines the way a reader reads them:
//
//   - Lines that begin with a bullet („-", „•", „·", „o" …) are items of a list, and so are lines
//     numbered in sequence from the start („1." „2.", „a)" „b)", „(1)" „(2)", „I." „II."). A number
//     that starts no sequence stays text („1. Semester" alone).
//   - A list that follows a numbered item, or an item that ends with a colon, belongs to it; what
//     stands between two numbered items belongs to the first.
//   - A line that only goes on with the sentence before it — the source broke it, mostly a copy
//     from a PDF: the line before ends in „und" or a comma, or a long one goes on in lower case —
//     is joined to it. A new sentence after a long line begins a paragraph. Every other line
//     break of the page is kept.
//
// CommonMark numbers lists with digits only, so a list labelled with letters, Roman numerals or
// numbers in brackets is a bullet list whose items begin with their label („- (a) Absorption");
// Folia shows those labels as the markers of the list. Anything CommonMark would read as markup
// is escaped, so the text says what the page says and nothing more.
func Markdown(n *html.Node) string {
	if n == nil {
		return ""
	}
	c := &collector{}
	c.children(n)
	c.endLine()
	return render(structure(c.entries))
}

// style is how a piece of a line is set. One at a time: what is both strong and emphasized is
// strong, which keeps the delimiters of the Markdown in the one order CommonMark reads safely.
type style uint8

const (
	plain style = iota
	emphasis
	strong
)

// span is a piece of a line in one style.
type span struct {
	text  string
	style style
}

// textLine is a line of a text as the page breaks it: by <br> or by the end of a block.
type textLine []span

func (l textLine) text() string {
	var b strings.Builder
	for _, s := range l {
		b.WriteString(s.text)
	}
	return b.String()
}

// The parts of a flow: what a cell or an item of a list holds, in order.
type entryKind uint8

const (
	flowLine entryKind = iota // a line of text
	flowGap                   // a blank line: the end of a paragraph
	flowList                  // a list the page marks up
)

type entry struct {
	kind entryKind
	line textLine
	list *pageList
}

// pageList is a list the page marks up (<ul>, <ol>): each item a flow of its own.
type pageList struct {
	ordered bool
	start   int
	items   [][]entry
}

// collector reads the nodes of a cell into a flow.
type collector struct {
	entries  []entry
	line     textLine
	strong   int // the open elements that set their text strong
	emphasis int // … and emphasized
}

func (c *collector) style() style {
	switch {
	case c.strong > 0:
		return strong
	case c.emphasis > 0:
		return emphasis
	}
	return plain
}

func (c *collector) children(n *html.Node) {
	for child := n.FirstChild; child != nil; child = child.NextSibling {
		c.node(child)
	}
}

func (c *collector) node(n *html.Node) {
	switch n.Type {
	case html.TextNode:
		c.add(n.Data)
		return
	case html.ElementNode:
	default:
		return
	}
	switch n.DataAtom {
	case atom.Br:
		c.lineBreak()
	case atom.Ul, atom.Ol, atom.Menu:
		c.list(n)
	case atom.Li:
		// An item outside a list reads like a typed one.
		c.endLine()
		c.add("• ")
		c.inline(n)
		c.endLine()
	case atom.P, atom.H1, atom.H2, atom.H3, atom.H4, atom.H5, atom.H6, atom.Blockquote, atom.Pre, atom.Table,
		atom.Hr, atom.Dl, atom.Figure, atom.Address, atom.Section, atom.Article, atom.Aside, atom.Header,
		atom.Footer, atom.Nav, atom.Main, atom.Fieldset, atom.Form:
		c.endLine()
		c.gap()
		c.inline(n)
		c.endLine()
		c.gap()
	case atom.Div, atom.Center, atom.Tr, atom.Dt, atom.Dd, atom.Caption, atom.Figcaption, atom.Legend,
		atom.Details, atom.Summary:
		c.endLine()
		c.inline(n)
		c.endLine()
	case atom.Td, atom.Th:
		c.add(" ")
		c.inline(n)
		c.add(" ")
	case atom.Script, atom.Style, atom.Noscript, atom.Template, atom.Head, atom.Title, atom.Iframe,
		atom.Object, atom.Embed, atom.Svg, atom.Math, atom.Select, atom.Textarea, atom.Button, atom.Input,
		atom.Img, atom.Video, atom.Audio, atom.Canvas:
		// Nothing of the text.
	default:
		c.inline(n)
	}
}

// inline reads the children of an element in the style it sets.
func (c *collector) inline(n *html.Node) {
	isStrong, isEmphasis := elementStyle(n)
	if isStrong {
		c.strong++
	}
	if isEmphasis {
		c.emphasis++
	}
	c.children(n)
	if isStrong {
		c.strong--
	}
	if isEmphasis {
		c.emphasis--
	}
}

// elementStyle is the style an element sets: <b>, <strong>, headings and underlining strong
// (the pages underline their headings: „<span style="text-decoration: underline;">Lehrpraktikum
// </span>"), <i> and <em> emphasis.
func elementStyle(n *html.Node) (isStrong, isEmphasis bool) {
	switch n.DataAtom {
	case atom.B, atom.Strong, atom.U, atom.Ins, atom.H1, atom.H2, atom.H3, atom.H4, atom.H5, atom.H6:
		isStrong = true
	case atom.I, atom.Em, atom.Cite, atom.Dfn, atom.Var:
		isEmphasis = true
	}
	for _, declaration := range strings.Split(strings.ToLower(GetAttr(n, "style")), ";") {
		property, value, ok := strings.Cut(declaration, ":")
		if !ok {
			continue
		}
		value = strings.TrimSpace(value)
		switch strings.TrimSpace(property) {
		case "font-weight":
			weight, err := strconv.Atoi(value)
			if value == "bold" || value == "bolder" || (err == nil && weight >= 600) {
				isStrong = true
			}
		case "text-decoration", "text-decoration-line":
			if strings.Contains(value, "underline") {
				isStrong = true
			}
		case "font-style":
			if value == "italic" || value == "oblique" {
				isEmphasis = true
			}
		}
	}
	return isStrong, isEmphasis
}

// list reads a list the page marks up. A list right inside a list (the pages nest them so)
// belongs to the item before it; text outside an item is an item of its own.
func (c *collector) list(n *html.Node) {
	c.endLine()
	l := &pageList{ordered: n.DataAtom == atom.Ol, start: 1}
	if start, err := strconv.Atoi(strings.TrimSpace(GetAttr(n, "start"))); l.ordered && err == nil && start >= 0 && start < 10000 {
		l.start = start
	}
	for child := n.FirstChild; child != nil; child = child.NextSibling {
		sub := &collector{strong: c.strong, emphasis: c.emphasis}
		element := child.Type == html.ElementNode
		switch {
		case element && child.DataAtom == atom.Li:
			sub.inline(child)
		case element && (child.DataAtom == atom.Ul || child.DataAtom == atom.Ol) && len(l.items) > 0:
			sub.list(child)
			l.items[len(l.items)-1] = append(l.items[len(l.items)-1], sub.entries...)
			continue
		default:
			sub.node(child)
		}
		sub.endLine()
		if len(sub.entries) > 0 {
			l.items = append(l.items, sub.entries)
		}
	}
	if len(l.items) > 0 {
		c.entries = append(c.entries, entry{kind: flowList, list: l})
	}
}

// add appends text in the current style. White space collapses as a browser collapses it.
func (c *collector) add(s string) {
	s = collapseSpace(s)
	if strings.HasPrefix(s, " ") && (len(c.line) == 0 || strings.HasSuffix(c.line[len(c.line)-1].text, " ")) {
		s = s[1:]
	}
	if s == "" {
		return
	}
	st := c.style()
	if n := len(c.line); n > 0 && c.line[n-1].style == st {
		c.line[n-1].text += s
		return
	}
	c.line = append(c.line, span{text: s, style: st})
}

// endLine ends the line where a block ends.
func (c *collector) endLine() {
	if line := trimLine(c.line); len(line) > 0 {
		c.entries = append(c.entries, entry{kind: flowLine, line: line})
	}
	c.line = nil
}

// lineBreak is a <br>. A line it ends with no text in it is a blank line: the end of a paragraph.
func (c *collector) lineBreak() {
	if len(trimLine(c.line)) == 0 {
		c.line = nil
		c.gap()
		return
	}
	c.endLine()
}

// gap ends a paragraph. A flow neither begins with one nor has two in a row.
func (c *collector) gap() {
	if n := len(c.entries); n > 0 && c.entries[n-1].kind != flowGap {
		c.entries = append(c.entries, entry{kind: flowGap})
	}
}

// collapseSpace turns every run of white space into one space, the no-break space included:
// the pages pad with it („Lecture:&nbsp;&nbsp; ").
func collapseSpace(s string) string {
	var b strings.Builder
	space := false
	for _, r := range s {
		switch {
		case r == '\u200b' || r == '\ufeff':
			continue
		case isSpace(r):
			space = true
			continue
		}
		if space {
			b.WriteByte(' ')
			space = false
		}
		b.WriteRune(r)
	}
	if space {
		b.WriteByte(' ')
	}
	return b.String()
}

func isSpace(r rune) bool {
	switch r {
	case ' ', '\t', '\n', '\r', '\f', '\v', '\u00a0', '\u1680', '\u2028', '\u2029', '\u202f', '\u205f', '\u3000':
		return true
	}
	return r >= '\u2000' && r <= '\u200a'
}

// trimLine drops the white space at both ends of a line and the pieces left empty.
func trimLine(line textLine) textLine {
	out := make(textLine, 0, len(line))
	for _, s := range line {
		if s.text != "" {
			out = append(out, s)
		}
	}
	for len(out) > 0 {
		if out[0].text = strings.TrimLeft(out[0].text, " "); out[0].text != "" {
			break
		}
		out = out[1:]
	}
	for len(out) > 0 {
		last := len(out) - 1
		if out[last].text = strings.TrimRight(out[last].text, " "); out[last].text != "" {
			break
		}
		out = out[:last]
	}
	return out
}

// cutLine drops the first n bytes of a line's text: the marker of a typed item.
func cutLine(line textLine, n int) textLine {
	var out textLine
	for _, s := range line {
		if n >= len(s.text) {
			n -= len(s.text)
			continue
		}
		s.text = s.text[n:]
		n = 0
		out = append(out, s)
	}
	return trimLine(out)
}

// marker is how a typed item of a list begins: a bullet („-", „•", „o") or a number of a
// sequence („1.", „b)", „(3)", „IV.").
type marker struct {
	seq   string // the kind of the sequence: "•" for every bullet, else the number's pattern: "1.", "(a)", "I)" …
	value int    // the number's place in its sequence, from 1
	label string // the number as the page writes it
	size  int    // the bytes the marker takes with the space after it
	rare  rune   // a bullet that only counts where a text has two of it
	tight bool   // a dash right before its word: „-Techniken", or a word it shares („und -analytische")
	end   int    // a number: the entry of the last item of its sequence
}

const (
	// bullets the pages type before an item, Word's among them (pasted: „·", „o", „§", and the
	// private-use characters of the Symbol and Wingdings fonts)
	bulletRunes = "•·◦▪▫■□●○►▶▸‣⁃➢➤➔→✓✔-–—*+>o\uf0b7\uf0a7\uf0a8\uf0d8\uf0fc\uf076"
	// bullets that may stand right before their word: „-Techniken", „•Klausur"
	tightBullets = "•·-–"
	// bullets that only count where a text has two of them: „o" begins words, „*" footnotes,
	// „+" sums, „→" conclusions, „>" a quote
	rareBullets = "o*+→>"
)

var (
	// reNumber is a number that may begin an item: „1." „1)" „(1)", „a)" „(a)" „A.", „iv." „(IV)".
	reNumber = regexp.MustCompile(`^(?:\((\d{1,2}|[a-zA-Z]|[ivx]{2,6}|[IVX]{2,6})\)|(\d{1,2}|[a-zA-Z]|[ivx]{2,6}|[IVX]{2,6})([.)])) `)
	// reSection is the number of a section under a numbered one: „3.1." „3.2.", „4.1 ".
	reSection = regexp.MustCompile(`^(\d{1,2})\.(\d{1,2})(\.?) `)
	// reTightNumber is a number right before its word: „3.Entwicklungspfade", „7.Nanotechnologie".
	reTightNumber = regexp.MustCompile(`^(\d{1,2})([.)])\pL`)
)

// candidates are the markers a line may begin with: a letter that is a Roman numeral too
// („i.", „V)") begins one of either sequence.
func candidates(text string) []marker {
	if match := reSection.FindStringSubmatch(text); match != nil && strings.TrimSpace(text[len(match[0]):]) != "" {
		value, _ := strconv.Atoi(match[2])
		return []marker{{seq: match[1] + ".1" + match[3], value: value, label: strings.TrimSpace(match[0]), size: len(match[0])}}
	}
	first, size := utf8.DecodeRuneInString(text)
	if strings.ContainsRune(bulletRunes, first) {
		rest := text[size:]
		switch {
		case rest == "" && !strings.ContainsRune(rareBullets, first):
			// a bullet alone: its text follows on the next line („<li><br>Text</li>")
			return []marker{{seq: "•", size: size}}
		case strings.HasPrefix(rest, " ") && strings.TrimSpace(rest) != "":
			m := marker{seq: "•", size: size + 1}
			if strings.ContainsRune(rareBullets, first) {
				m.rare = first
			}
			return []marker{m}
		case strings.ContainsRune(tightBullets, first) && beginsWord(rest):
			return []marker{{seq: "•", size: size, tight: first == '-' || first == '–'}}
		}
	}
	match := reNumber.FindStringSubmatch(text)
	if tight := reTightNumber.FindStringSubmatch(text); match == nil && tight != nil {
		// the word is the item's text: the match ends before it
		match = []string{tight[1] + tight[2], "", tight[1], tight[2]}
	}
	if match == nil || strings.TrimSpace(text[len(match[0]):]) == "" {
		return nil
	}
	token, delimiter := match[1], "()"
	if token == "" {
		token, delimiter = match[2], match[3]
	}
	pattern := func(first string) string {
		if delimiter == "()" {
			return "(" + first + ")"
		}
		return first + delimiter
	}
	m := marker{label: strings.TrimSpace(match[0]), size: len(match[0])}
	var out []marker
	if number, err := strconv.Atoi(token); err == nil {
		m.seq, m.value = pattern("1"), number
		return append(out, m)
	}
	if r := []rune(token); len(r) == 1 {
		alpha := m
		if unicode.IsUpper(r[0]) {
			alpha.seq, alpha.value = pattern("A"), int(r[0]-'A')+1
		} else {
			alpha.seq, alpha.value = pattern("a"), int(r[0]-'a')+1
		}
		out = append(out, alpha)
	}
	if value := roman(token); value > 0 {
		m.seq, m.value = pattern("i"), value
		if unicode.IsUpper(rune(token[0])) {
			m.seq = pattern("I")
		}
		out = append(out, m)
	}
	return out
}

// beginsWord reports whether text begins with a letter or what opens a word: a bracket, a quote.
func beginsWord(text string) bool {
	r, _ := utf8.DecodeRuneInString(text)
	return unicode.IsLetter(r) || strings.ContainsRune("(„“\"'‚", r)
}

// roman is the value of a Roman numeral written the one usual way (I to XXXIX), else 0.
func roman(s string) int {
	if s == "" || (strings.ToLower(s) != s && strings.ToUpper(s) != s) {
		return 0
	}
	values := map[byte]int{'i': 1, 'v': 5, 'x': 10}
	lower := strings.ToLower(s)
	total := 0
	for i := 0; i < len(lower); i++ {
		v := values[lower[i]]
		if i+1 < len(lower) && values[lower[i+1]] > v {
			total -= v
		} else {
			total += v
		}
	}
	if total <= 0 || total >= 40 || toRoman(total) != lower {
		return 0
	}
	return total
}

func toRoman(n int) string {
	return strings.Repeat("x", n/10) + []string{"", "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix"}[n%10]
}

// markers decides which lines of a flow begin typed items. A bullet does, unless it is one of
// those that also begin words or footnotes and stands alone, or a dash that only goes on with
// the line before („systemtheoretische und / –analytische"). A number does where it is one of a
// sequence that begins at the beginning (1, a, i) and has a second number.
func markers(entries []entry) []*marker {
	chosen := make([]*marker, len(entries))
	options := make([][]marker, len(entries))
	rare := map[rune]int{}
	for i, e := range entries {
		if e.kind != flowLine {
			continue
		}
		options[i] = candidates(e.line.text())
		for _, m := range options[i] {
			if m.rare != 0 {
				rare[m.rare]++
			}
		}
	}
	for i := range entries {
		for k := range options[i] {
			m := options[i][k]
			if m.seq != "•" || (m.rare != 0 && rare[m.rare] < 2) {
				continue
			}
			if m.tight && i > 0 && entries[i-1].kind == flowLine && endsInConjunction(entries[i-1].line.text()) {
				continue
			}
			chosen[i] = &m
		}
	}

	// The numbers of each kind of sequence, in order; a sequence goes on with the number after
	// its last one and begins again at 1.
	type place struct{ entry, option int }
	var kinds []string
	numbers := map[string][]place{}
	for i := range entries {
		if chosen[i] != nil {
			continue
		}
		for k, m := range options[i] {
			if m.seq == "•" {
				continue
			}
			if _, ok := numbers[m.seq]; !ok {
				kinds = append(kinds, m.seq)
			}
			numbers[m.seq] = append(numbers[m.seq], place{i, k})
		}
	}
	var runs [][]place
	for _, kind := range kinds {
		var run []place
		next := 1
		for _, p := range numbers[kind] {
			switch value := options[p.entry][p.option].value; {
			case value == next:
				run = append(run, p)
				next++
			case value == 1:
				if len(run) >= 2 {
					runs = append(runs, run)
				}
				run, next = []place{p}, 2
			}
		}
		if len(run) >= 2 {
			runs = append(runs, run)
		}
	}
	// A line two sequences claim („i." after „h." or before „ii.") goes to the longer one.
	sort.SliceStable(runs, func(a, b int) bool { return len(runs[a]) > len(runs[b]) })
	for _, run := range runs {
		taken := false
		for _, p := range run {
			taken = taken || chosen[p.entry] != nil
		}
		if taken {
			continue
		}
		end := run[len(run)-1].entry
		for _, p := range run {
			m := options[p.entry][p.option]
			m.end = end
			chosen[p.entry] = &m
		}
	}
	return chosen
}

// How a line follows the line before it.
type joint uint8

const (
	hardBreak      joint = iota // the page's line break, kept
	softBreak                   // a sentence the source broke: it reads on
	paragraphBreak              // a new sentence after a long line: a paragraph of its own
)

const (
	// paragraphLine is how long one of two lines is, at least, whose line break between two
	// sentences begins a paragraph. Between shorter ones („Die Dauer des Grundpraktikums beträgt
	// 6 Wochen." / „Die Dauer des Fachpraktikums …") the break stays a line break.
	paragraphLine = 80
	// wrappedLine is how long a line is, at least, that the line after it goes on in lower case
	// as a broken sentence. Shorter ones are items without a bullet („Practical project
	// presentation, 30 min (50%) in groups" / „oral examination, 20 min. (50%)").
	wrappedLine = 60
)

// join decides how a line follows the one before it in a paragraph.
func join(prev, next string) joint {
	first, _ := utf8.DecodeRuneInString(next)
	last, _ := utf8.DecodeLastRuneInString(prev)
	switch {
	case endsSentence(prev):
		if (unicode.IsUpper(first) || unicode.IsDigit(first) || strings.ContainsRune("„“\"'‚(", first)) &&
			max(utf8.RuneCountInString(prev), utf8.RuneCountInString(next)) >= paragraphLine {
			return paragraphBreak
		}
		return hardBreak
	case strings.ContainsRune(":;", last):
		return hardBreak
	case strings.ContainsRune(".!?", last), // an abbreviation, „z.B." / „Moodle"
		continuesSentence(prev),
		unicode.IsLower(first) && utf8.RuneCountInString(prev) >= wrappedLine:
		return softBreak
	}
	// „Bau-" / „und Umweltingenieurwesen": a word cut before „und".
	if words := strings.Fields(next); strings.HasSuffix(prev, "-") && len(words) > 0 && conjunctions[strings.ToLower(words[0])] {
		return softBreak
	}
	return hardBreak
}

// endsSentence reports whether a line ends with the end of a sentence; „z.B." does not.
func endsSentence(line string) bool {
	line = strings.TrimRight(line, "\"'“”‘’»)]")
	last, _ := utf8.DecodeLastRuneInString(line)
	if !strings.ContainsRune(".!?", last) {
		return false
	}
	fields := strings.Fields(line)
	return len(fields) == 0 || !abbreviations[strings.ToLower(fields[len(fields)-1])]
}

// continuesSentence reports whether a line stops inside a sentence: after a comma, a dash
// between words, or a word that ends none („und", „der", "of").
func continuesSentence(line string) bool {
	if strings.HasSuffix(line, ",") || strings.HasSuffix(line, " –") || strings.HasSuffix(line, " -") {
		return true
	}
	// An article in capitals is a label: „Teil A".
	fields := strings.Fields(line)
	return len(fields) > 0 && connectives[fields[len(fields)-1]] || endsInConjunction(line)
}

// endsInConjunction reports whether a line ends with „und", „oder" …: a dash right before the
// word on the next line („systemtheoretische und / –analytische") shares a word, it begins no item.
func endsInConjunction(line string) bool {
	fields := strings.Fields(line)
	return len(fields) > 0 && conjunctions[strings.ToLower(fields[len(fields)-1])]
}

var conjunctions = setOf("und", "oder", "sowie", "bzw.", "and", "or", "&")

// connectives are words a sentence does not end with: articles, prepositions.
var connectives = setOf("sowohl", "als", "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines",
	"einem", "einen", "von", "vom", "mit", "zu", "zur", "zum", "im", "in", "an", "am", "auf", "für", "bei", "aus",
	"nach", "über", "unter", "durch", "gegen", "ohne", "um", "the", "of", "to", "for", "with", "on", "by", "from",
	"a", "an", "as", "at", "into")

// abbreviations end with a full stop but not a sentence.
var abbreviations = setOf("z.b.", "d.h.", "u.a.", "bzw.", "ca.", "vgl.", "e.g.", "i.e.", "prof.", "dr.", "nr.",
	"inkl.", "ggf.", "evtl.", "sog.", "bspw.", "insb.", "incl.", "approx.", "resp.", "s.", "u.", "o.")

func setOf(words ...string) map[string]bool {
	set := make(map[string]bool, len(words))
	for _, w := range words {
		set[w] = true
	}
	return set
}

// block is a paragraph (its lines) or a list.
type block struct {
	lines []mdLine
	list  *mdList
}

// mdLine is a line of a paragraph and how it follows the line before it (a hard or a soft
// break; a paragraph break begins a block of its own).
type mdLine struct {
	spans textLine
	joint joint
}

type listKind uint8

const (
	bulletList listKind = iota // „-"
	numberList                 // „1.", the numbers CommonMark knows
	labelList                  // „a)", „(1)", „IV.": bullets whose items begin with their label
)

type mdList struct {
	kind  listKind
	start int
	items []*mdItem
}

type mdItem struct {
	label  string
	blocks []block
}

// open is a typed list that can still take items.
type open struct {
	list *mdList
	seq  string // the marker it goes on with
	next int    // a numbered list: the number of its next item
	end  int    // a numbered list: the entry of its last item; -1 for bullets
}

type builder struct {
	entries []entry
	marks   []*marker
	out     []block
	stack   []*open
	gap     bool // a blank line stands before the entry at hand
}

// structure turns a flow into blocks: the lists it types become lists, its lines paragraphs.
func structure(entries []entry) []block {
	b := &builder{entries: entries, marks: markers(entries)}
	for i, e := range entries {
		switch e.kind {
		case flowGap:
			b.gap = true
			continue
		case flowList:
			b.pageList(i, e.list)
		case flowLine:
			if m := b.marks[i]; m != nil {
				b.item(i, m, e.line)
			} else {
				b.text(i, e.line)
			}
		}
		b.gap = false
	}
	return b.out
}

// blocks is where what comes next goes: into the item the innermost open list is at, else
// into the flow itself.
func (b *builder) blocks() *[]block {
	if n := len(b.stack); n > 0 {
		items := b.stack[n-1].list.items
		return &items[len(items)-1].blocks
	}
	return &b.out
}

// resume closes the lists a blank line ends: all but the numbered ones that go on after entry
// i, since what stands between two numbered items belongs to the first.
func (b *builder) resume(i int) {
	for k := len(b.stack) - 1; k >= 0; k-- {
		if b.stack[k].end > i {
			b.stack = b.stack[:k+1]
			return
		}
	}
	b.stack = nil
}

// settle closes the open lists a list beginning at entry i does not belong into. After a blank
// line that is all but the numbered lists that go on after it; else the list belongs to the item
// before it where that item is numbered („2. Kennenlernen:" and what it lists) or announces it
// with a colon.
func (b *builder) settle(i int) {
	if b.gap {
		b.resume(i)
		return
	}
	for n := len(b.stack); n > 0; n = len(b.stack) {
		o := b.stack[n-1]
		if o.list.kind != bulletList || announces(o.list.items[len(o.list.items)-1]) {
			return
		}
		b.stack = b.stack[:n-1]
	}
}

// announces reports whether an item ends in a colon: what follows it is its own.
func announces(it *mdItem) bool {
	if n := len(it.blocks); n > 0 && it.blocks[n-1].list == nil {
		lines := it.blocks[n-1].lines
		return strings.HasSuffix(lines[len(lines)-1].spans.text(), ":")
	}
	return false
}

// item takes a typed item: into the open list it goes on with, else into a new list.
func (b *builder) item(i int, m *marker, line textLine) {
	for k := len(b.stack) - 1; k >= 0; k-- {
		o := b.stack[k]
		if o.seq != m.seq {
			continue
		}
		if o.list.kind == bulletList || o.next == m.value {
			b.stack = b.stack[:k+1]
			b.add(o, m, line)
			return
		}
		// A sequence that begins again is a list of its own, beside the one before.
		b.stack = b.stack[:k]
		break
	}
	b.settle(i)
	l := &mdList{kind: labelList, start: 1}
	switch m.seq {
	case "•":
		l.kind = bulletList
	case "1.", "1)":
		l.kind, l.start = numberList, m.value
	}
	if last := b.adjacentList(); l.kind == bulletList && last != nil && last.kind == bulletList {
		// typed items right after a list of the page: one list
		l = last
	} else {
		t := b.blocks()
		*t = append(*t, block{list: l})
	}
	o := &open{list: l, seq: m.seq, end: -1}
	if m.seq != "•" {
		o.end = m.end
	}
	b.stack = append(b.stack, o)
	b.add(o, m, line)
}

func (b *builder) add(o *open, m *marker, line textLine) {
	it := &mdItem{}
	if o.list.kind == labelList {
		it.label = m.label
	}
	if rest := cutLine(line, m.size); len(rest) > 0 {
		it.blocks = []block{{lines: []mdLine{{spans: rest}}}}
	}
	o.list.items = append(o.list.items, it)
	o.next = m.value + 1
}

// adjacentList is the list that what comes next directly follows, where it does: the last block
// of the place it goes to, with no blank line between.
func (b *builder) adjacentList() *mdList {
	if t := *b.blocks(); !b.gap && len(t) > 0 {
		return t[len(t)-1].list
	}
	return nil
}

// text takes a line that begins no item: it goes on with the paragraph before it, or begins one.
func (b *builder) text(i int, line textLine) {
	if b.gap {
		b.resume(i)
	} else {
		b.leave(i, line)
	}
	t := b.blocks()
	if n := len(*t); !b.gap && n > 0 && (*t)[n-1].list == nil {
		p := &(*t)[n-1]
		if j := join(p.lines[len(p.lines)-1].spans.text(), line.text()); j != paragraphBreak {
			p.lines = append(p.lines, mdLine{spans: line, joint: j})
			return
		}
	}
	*t = append(*t, block{lines: []mdLine{{spans: line}}})
}

// pageList takes a list the page marks up, its items read as flows of their own. A bullet or
// a number typed at the start of an item repeats the list's own and goes.
func (b *builder) pageList(i int, pl *pageList) {
	l := &mdList{kind: bulletList, start: 1}
	if pl.ordered {
		l.kind, l.start = numberList, pl.start
	}
	flows, labels := pl.items, []string(nil)
	if !pl.ordered {
		if kind, typed, cut := typedNumbers(flows); cut != nil {
			l.kind, labels, flows = kind, typed, cut
		}
	}
	for n, flow := range flows {
		if labels == nil {
			flow = dropMarker(flow, pl.ordered, l.start+len(l.items))
		}
		blocks := structure(flow)
		switch {
		case len(blocks) == 0:
			continue
		case len(blocks) == 1 && blocks[0].list != nil && len(l.items) > 0:
			// An item that holds nothing but a list: the page nests it under the item before.
			prev := l.items[len(l.items)-1]
			prev.blocks = append(prev.blocks, blocks...)
			continue
		}
		it := &mdItem{blocks: blocks}
		if l.kind == labelList {
			it.label = labels[n]
		}
		l.items = append(l.items, it)
	}
	if len(l.items) == 0 {
		return
	}
	if only := l.items[0]; len(l.items) == 1 && only.label == "" && len(only.blocks) == 1 && only.blocks[0].list != nil {
		// a list of one item that holds nothing but a list is that list
		l = only.blocks[0].list
	}
	b.settle(i)
	// A page that begins a list for every item (</ul><ul>) means one list.
	if last := b.adjacentList(); last != nil && last.kind == l.kind && (l.kind == bulletList || l.start == last.start+len(last.items)) {
		last.items = append(last.items, l.items...)
		return
	}
	t := b.blocks()
	*t = append(*t, block{list: l})
}

// leave closes the typed lists a line after one of their items does not belong to — none where
// the line only goes on with the sentence before it, and no numbered list that goes on after it:
// what stands between two of its items belongs to the first. A line that announces what follows
// (it ends with a colon, or it is all strong: a heading) belongs to a numbered item that has no
// list of its own yet — it begins one („c. Theorie und Praxis …" / „Grundlagen von …:") — and
// begins something new after any other („Teil 2:" after the list of „3. Gemeinwesenarbeit:").
// Any other line belongs to the item before it where a bullet follows it (a heading and what it
// says, between two items), or where the item before the last goes on below its first line too;
// else the list has ended, and the line follows it.
func (b *builder) leave(i int, line textLine) {
	t := *b.blocks()
	if len(b.stack) > 0 && len(t) == 0 {
		return // the text of an item whose bullet stands alone
	}
	if len(t) > 0 && t[len(t)-1].list == nil {
		lines := t[len(t)-1].lines
		if join(lines[len(lines)-1].spans.text(), line.text()) == softBreak {
			return
		}
	}
	heading := announcesLine(line)
	for n := len(b.stack); n > 0; n = len(b.stack) {
		o := b.stack[n-1]
		switch {
		case o.end > i:
			return
		case heading:
			if o.list.kind != bulletList && !hasList(o.list.items[len(o.list.items)-1]) {
				return
			}
		case o.list.kind == bulletList && b.bulletFollows(i), itemsGoOn(o.list):
			return
		}
		b.stack = b.stack[:n-1]
	}
}

// hasList reports whether an item holds a list.
func hasList(it *mdItem) bool {
	for _, bl := range it.blocks {
		if bl.list != nil {
			return true
		}
	}
	return false
}

// announcesLine reports whether a line announces what follows: it ends with a colon („Teil 2:"),
// or it is all strong, as a heading is.
func announcesLine(line textLine) bool {
	if strings.HasSuffix(line.text(), ":") {
		return true
	}
	for _, s := range line {
		if s.style != strong {
			return false
		}
	}
	return len(line) > 0
}

// bulletFollows reports whether the next typed item after entry i, before a blank line, has a bullet.
func (b *builder) bulletFollows(i int) bool {
	for j := i + 1; j < len(b.entries); j++ {
		switch {
		case b.entries[j].kind == flowGap:
			return false
		case b.marks[j] != nil:
			return b.marks[j].seq == "•"
		}
	}
	return false
}

// itemsGoOn reports whether the item before the last of a list goes on below its first line.
func itemsGoOn(l *mdList) bool {
	n := len(l.items)
	if n < 2 {
		return false
	}
	blocks := l.items[n-2].blocks
	return len(blocks) > 1 || (len(blocks) == 1 && blocks[0].list == nil && len(blocks[0].lines) > 1)
}

// typedNumbers reads the numbers typed at the start of every item of a bullet list of the page
// („<li>1) Continuous assessment …</li><li>2) final written exam …</li>"): they number the list,
// and the items lose them. Nothing where an item has none, or they are no sequence from its start.
func typedNumbers(flows [][]entry) (listKind, []string, [][]entry) {
	if len(flows) < 2 {
		return bulletList, nil, nil
	}
	options := make([][]marker, len(flows))
	for n, flow := range flows {
		if len(flow) == 0 || flow[0].kind != flowLine {
			return bulletList, nil, nil
		}
		options[n] = candidates(flow[0].line.text())
	}
	for _, first := range options[0] {
		if first.seq == "•" || first.value != 1 {
			continue
		}
		chosen := make([]marker, 0, len(flows))
		for n := range flows {
			for _, m := range options[n] {
				if m.seq == first.seq && m.value == n+1 {
					chosen = append(chosen, m)
					break
				}
			}
		}
		if len(chosen) < len(flows) {
			continue
		}
		labels := make([]string, len(flows))
		cut := make([][]entry, len(flows))
		for n, flow := range flows {
			labels[n] = chosen[n].label
			cut[n] = flow[1:]
			if rest := cutLine(flow[0].line, chosen[n].size); len(rest) > 0 {
				cut[n] = append([]entry{{kind: flowLine, line: rest}}, flow[1:]...)
			}
		}
		if first.seq == "1." || first.seq == "1)" {
			return numberList, labels, cut
		}
		return labelList, labels, cut
	}
	return bulletList, nil, nil
}

// dropMarker cuts a bullet, or in a numbered list the item's own number, from the start of the
// item: „<li>- Klausur</li>", „<li>3. Projekt</li>" in the third item of an <ol>.
func dropMarker(flow []entry, ordered bool, number int) []entry {
	if len(flow) == 0 || flow[0].kind != flowLine {
		return flow
	}
	text := flow[0].line.text()
	for _, m := range candidates(text) {
		if (m.seq == "•" && m.rare == 0) || (ordered && (m.seq == "1." || m.seq == "1)") && m.value == number) {
			out := append([]entry{{kind: flowLine, line: cutLine(flow[0].line, m.size)}}, flow[1:]...)
			if len(out[0].line) == 0 {
				out = out[1:]
			}
			return out
		}
	}
	return flow
}

// render writes blocks as CommonMark.
func render(blocks []block) string {
	return strings.Join(renderBlocks(blocks, false), "\n")
}

// renderBlocks writes blocks as lines of Markdown, a blank line between two. Inside an item a
// list follows the paragraph that leads it on the next line, so the item stays as tight as the
// page sets it.
func renderBlocks(blocks []block, inItem bool) []string {
	var out []string
	previous := ""
	for k, bl := range blocks {
		if k > 0 && !(inItem && bl.list != nil && blocks[k-1].list == nil && interrupts(bl.list)) {
			out = append(out, "")
		}
		if bl.list == nil {
			out = append(out, renderParagraph(bl.lines)...)
			previous = ""
			continue
		}
		// Two lists in a row stay two where their markers differ: CommonMark joins lists with
		// the same one.
		var lines []string
		lines, previous = renderList(bl.list, previous)
		out = append(out, lines...)
	}
	return out
}

// interrupts reports whether a list may follow a paragraph on its next line: CommonMark lets a
// numbered list do so only from 1.
func interrupts(l *mdList) bool {
	return l.kind != numberList || l.start == 1
}

// renderList writes a list with the marker it takes after a list that took `after`; it returns
// the lines and its marker.
func renderList(l *mdList, after string) ([]string, string) {
	bullet, delimiter := "-", "."
	if after == "-" {
		bullet = "*"
	}
	if after == "." {
		delimiter = ")"
	}
	used := bullet
	if l.kind == numberList {
		used = delimiter
	}
	var out []string
	for n, it := range l.items {
		marker := bullet + " "
		if l.kind == numberList {
			marker = strconv.Itoa(l.start+n) + delimiter + " "
		}
		body := renderBlocks(it.blocks, true)
		if it.label != "" {
			label := escapeInline(it.label, false)
			if len(body) == 0 {
				body = []string{label}
			} else {
				body[0] = label + " " + body[0]
			}
		}
		if len(body) == 0 {
			continue
		}
		indent := strings.Repeat(" ", len(marker))
		out = append(out, marker+body[0])
		for _, line := range body[1:] {
			if line != "" {
				line = indent + line
			}
			out = append(out, line)
		}
	}
	return out, used
}

// renderParagraph writes the lines of a paragraph: a hard break ends a line with a backslash, a
// soft one only with the line itself.
func renderParagraph(lines []mdLine) []string {
	out := make([]string, 0, len(lines))
	for n, l := range lines {
		if n > 0 && l.joint == hardBreak {
			out[n-1] += `\`
		}
		out = append(out, inlineMarkdown(l.spans))
	}
	return out
}

// inlineMarkdown writes a line: its strong pieces between „**", its emphasized ones between „*",
// and everything that would be markup escaped.
func inlineMarkdown(line textLine) string {
	spans := settleSpans(line)
	var b strings.Builder
	for k, s := range spans {
		text := escapeInline(s.text, k == 0 && s.style == plain)
		switch s.style {
		case strong:
			b.WriteString("**" + text + "**")
		case emphasis:
			b.WriteString("*" + text + "*")
		default:
			b.WriteString(text)
		}
	}
	return b.String()
}

// settleSpans arranges the pieces of a line so that CommonMark reads their delimiters as the
// page means them: the white space at the edges of a set piece goes outside it, pieces of one
// style join, two set pieces never touch (the emphasized one of the two is set plain), and a
// delimiter that could not open or close where it stands — „**Voraussetzung:**Text" — has the
// punctuation beside it moved out, or its piece is set plain.
func settleSpans(line textLine) textLine {
	var spans textLine
	push := func(s span) {
		if s.text == "" {
			return
		}
		if n := len(spans); n > 0 && spans[n-1].style == s.style {
			spans[n-1].text += s.text
			return
		}
		spans = append(spans, s)
	}
	for _, s := range line {
		if s.style == plain {
			push(s)
			continue
		}
		trimmed := strings.TrimLeft(s.text, " ")
		if len(trimmed) < len(s.text) {
			push(span{text: " "})
		}
		inner := strings.TrimRight(trimmed, " ")
		push(span{text: inner, style: s.style})
		if len(inner) < len(trimmed) {
			push(span{text: " "})
		}
	}
	for k := 0; k+1 < len(spans); k++ {
		if spans[k].style != plain && spans[k+1].style != plain {
			if spans[k].style == emphasis {
				spans[k].style = plain
			} else {
				spans[k+1].style = plain
			}
		}
	}
	spans = rejoin(spans)

	for k := 0; k < len(spans); k++ {
		s := &spans[k]
		if s.style == plain {
			continue
		}
		before, after := ' ', ' '
		if k > 0 {
			before, _ = utf8.DecodeLastRuneInString(spans[k-1].text)
		}
		if k+1 < len(spans) {
			after, _ = utf8.DecodeRuneInString(spans[k+1].text)
		}
		var lead, trail string
		for s.text != "" {
			first, size := utf8.DecodeRuneInString(s.text)
			if leftFlanking(before, first) || !isPunctuation(first) {
				break
			}
			lead += s.text[:size]
			s.text = s.text[size:]
			before = first
		}
		for s.text != "" {
			last, size := utf8.DecodeLastRuneInString(s.text)
			if rightFlanking(last, after) || !isPunctuation(last) {
				break
			}
			trail = s.text[len(s.text)-size:] + trail
			s.text = s.text[:len(s.text)-size]
			after = last
		}
		first, _ := utf8.DecodeRuneInString(s.text)
		last, _ := utf8.DecodeLastRuneInString(s.text)
		if s.text == "" || !leftFlanking(before, first) || !rightFlanking(last, after) {
			s.text, s.style = lead+s.text+trail, plain
			continue
		}
		if lead == "" && trail == "" {
			continue
		}
		var rest textLine
		if lead != "" {
			rest = append(rest, span{text: lead})
		}
		rest = append(rest, *s)
		if trail != "" {
			rest = append(rest, span{text: trail})
		}
		rest = append(rest, spans[k+1:]...)
		spans = append(spans[:k], rest...)
		if lead != "" {
			k++
		}
	}
	return rejoin(spans)
}

// rejoin drops empty pieces and joins neighbours of one style.
func rejoin(spans textLine) textLine {
	var out textLine
	for _, s := range spans {
		if s.text == "" {
			continue
		}
		if n := len(out); n > 0 && out[n-1].style == s.style {
			out[n-1].text += s.text
			continue
		}
		out = append(out, s)
	}
	return out
}

// leftFlanking and rightFlanking are CommonMark's conditions for a run of „*" that opens and
// one that closes: `before` and `after` are the characters around it, a space at a line's ends.
func leftFlanking(before, after rune) bool {
	return !isMarkdownSpace(after) && (!isPunctuation(after) || isMarkdownSpace(before) || isPunctuation(before))
}

func rightFlanking(before, after rune) bool {
	return !isMarkdownSpace(before) && (!isPunctuation(before) || isMarkdownSpace(after) || isPunctuation(after))
}

func isMarkdownSpace(r rune) bool {
	return r == '\t' || r == '\n' || r == '\f' || r == '\r' || unicode.Is(unicode.Zs, r)
}

// isPunctuation is CommonMark's punctuation: Unicode's punctuation and symbols.
func isPunctuation(r rune) bool {
	return unicode.IsPunct(r) || unicode.IsSymbol(r)
}

// reEntity is what CommonMark reads as a character reference: „&amp;", „&#228;".
var reEntity = regexp.MustCompile(`^&(?:#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[A-Za-z][A-Za-z0-9]{1,31});`)

// escapeInline escapes what CommonMark would read as markup in a piece of text; atStart for a
// piece that begins a line, where more is markup: a list item, a heading, a quote.
func escapeInline(s string, atStart bool) string {
	var b strings.Builder
	for i, r := range s {
		switch r {
		case '\\', '`', '*', '[', ']':
			b.WriteByte('\\')
		case '_':
			// „_" inside a word is text to CommonMark.
			before, _ := utf8.DecodeLastRuneInString(s[:i])
			after, _ := utf8.DecodeRuneInString(s[i+1:])
			if i == 0 || i+1 == len(s) || !isWordRune(before) || !isWordRune(after) {
				b.WriteByte('\\')
			}
		case '<':
			// a tag, a comment or an autolink
			if next, _ := utf8.DecodeRuneInString(s[i+1:]); unicode.IsLetter(next) || strings.ContainsRune("/!?", next) {
				b.WriteByte('\\')
			}
		case '&':
			if reEntity.MatchString(s[i:]) {
				b.WriteByte('\\')
			}
		}
		b.WriteRune(r)
	}
	if atStart {
		return escapeLineStart(b.String())
	}
	return b.String()
}

func isWordRune(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsDigit(r)
}

var (
	reOrderedStart = regexp.MustCompile(`^(\d{1,9})([.)])( |$)`)
	reRuleLine     = regexp.MustCompile(`^(?:=+|-+)$`)
)

// escapeLineStart escapes what begins a block at the start of a line: „# ", „> ", „- ", „+ ",
// „1. ", a line of „=" or „-" (a heading's underline), „~~~" (a code fence).
func escapeLineStart(line string) string {
	switch {
	case reRuleLine.MatchString(line):
		return `\` + line
	case strings.HasPrefix(line, "#"), strings.HasPrefix(line, ">"), strings.HasPrefix(line, "~~~"):
		return `\` + line
	case line == "-" || line == "+" || strings.HasPrefix(line, "- ") || strings.HasPrefix(line, "+ "):
		return `\` + line
	}
	if m := reOrderedStart.FindStringSubmatchIndex(line); m != nil {
		return line[:m[3]] + `\` + line[m[3]:]
	}
	return line
}
