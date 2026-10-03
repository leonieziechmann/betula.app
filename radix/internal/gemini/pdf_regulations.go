package gemini

import (
	"context"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"unicode"

	"github.com/ledongthuc/pdf"
)

// An issue of the Amtliches Mitteilungsblatt may print several regulations one
// after the other: 17/2018 holds the Bachelor's Prüfungs- und Studienordnung of
// Materialchemie on pages 2–5 and the Master's on pages 6–10. Each has its own
// Anlage 1 and its own Regelstudienplan, so a document read as a whole gave
// both programs both plans — the Bachelor a Master-Arbeit, the Master the first
// semesters of the Bachelor.
//
// The cover says where each regulation begins: its table of contents lists
// every one with the page it starts on. The running footers name the
// regulation as well, but not reliably — 07/2014 prints „Master-Studiengang"
// under two pages of the Bachelor's plans — so the pages come from the
// contents, and the page headers have to confirm that a page is the one the
// contents mean.
type regulation struct {
	title    string
	degrees  []string // "bachelor", "master"; none where the title names neither
	from, to int      // physical pages; from is 0 where the contents print none
}

var (
	// contentsEntry opens an entry of the table of contents: „1.  Fachspezifische
	// Prüfungs- und Studienordnung für den Bachelor-Studiengang  2".
	contentsEntry = regexp.MustCompile(`^(\d{1,2})\.\s+(.+)$`)
	// contentsPage is the page an entry starts on, printed at the end of one of
	// its lines — of the first, in every issue of the corpus.
	contentsPage = regexp.MustCompile(`\s(\d{1,3})$`)
	// studiengangDegree is the degree a regulation is for: „für den
	// Bachelor-Studiengang", „des Master-Studiengangs", „für den universitären
	// Master-Studiengang", or both in „Bachelor- und Master-Studiengang".
	studiengangDegree = regexp.MustCompile(`(?i)\b(bachelor|master)(?:-?\s+und\s+(bachelor|master))?\s*-?\s*studieng`)
	// printedPageNumber is the number a page's running header prints; some
	// issues set the S of „Seite" apart („S  eite 6").
	printedPageNumber = regexp.MustCompile(`\bS\s*eite\s+(\d{1,3})\b`)
	// programDegree is the degree a program hint names („Materialchemie /
	// Master (universitär) / PO 2018", „… / LA Bachelor Grundstufe/Primarstufe / …").
	programDegree = regexp.MustCompile(`(?i)\b(bachelor|master)\b`)
)

// tableOfContents reads the entries the cover lists, in the order it lists them.
func tableOfContents(cover string) []regulation {
	lines := strings.Split(cover, "\n")
	start := -1
	for i, line := range lines {
		if strings.EqualFold(strings.Join(strings.Fields(line), ""), "inhalt") {
			start = i + 1
			break
		}
	}
	if start < 0 {
		return nil
	}
	var out []regulation
	for _, line := range lines[start:] {
		line = strings.Join(strings.Fields(line), " ")
		if strings.HasPrefix(line, "Herausgeber") {
			break
		}
		// The entries are numbered one after the other; a continuation line that
		// happens to begin with a number („17. September 2018") opens none.
		if m := contentsEntry.FindStringSubmatch(line); m != nil && m[1] == strconv.Itoa(len(out)+1) {
			out = append(out, regulation{})
			line = m[2]
		} else if len(out) == 0 {
			continue // the „Seite" over the page column
		}
		r := &out[len(out)-1]
		if m := contentsPage.FindStringSubmatchIndex(line); m != nil && r.from == 0 {
			r.from, _ = strconv.Atoi(line[m[2]:m[3]])
			line = strings.TrimSpace(line[:m[0]])
		}
		r.title = joinWrapped(r.title, line)
	}
	for i := range out {
		r := &out[i]
		seen := map[string]bool{}
		for _, m := range studiengangDegree.FindAllStringSubmatch(r.title, -1) {
			for _, d := range m[1:] {
				if d = strings.ToLower(d); d != "" && !seen[d] {
					seen[d] = true
					r.degrees = append(r.degrees, d)
				}
			}
		}
	}
	return out
}

// joinWrapped continues a title with its next line. A word the line broke is
// joined again („Ba-" and „chelor-Studiengang", „Master-Stu-" and „diengang"),
// while „Prüfungs-" before „und Studienordnung" keeps its hyphen and its space.
func joinWrapped(title, line string) string {
	switch {
	case title == "":
		return line
	case line == "":
		return title
	}
	head := []rune(line)[0]
	next, _, _ := strings.Cut(line, " ")
	if strings.HasSuffix(title, "-") && len(title) > 1 && unicode.IsLetter([]rune(title)[len([]rune(title))-2]) {
		switch {
		case unicode.IsUpper(head):
			return title + line // „Master-" and „Studiengang"
		case unicode.IsLower(head) && next != "und" && next != "oder" && next != "bzw." && next != "sowie":
			return strings.TrimSuffix(title, "-") + line
		}
	}
	return title + " " + line
}

// severalDegrees reports whether the regulations are for more than one degree.
// Only then does a program need its own part: an Änderungssatzung printed before
// the Lesefassung it amends is one program's regulation twice.
func severalDegrees(regs []regulation) bool {
	seen := map[string]bool{}
	for _, r := range regs {
		for _, d := range r.degrees {
			seen[d] = true
		}
	}
	return len(seen) > 1
}

func (r regulation) names(degree string) bool {
	for _, d := range r.degrees {
		if d == degree {
			return true
		}
	}
	return false
}

// regulationError marks every refusal of this file, so curriculumscan treats it
// as the document's own answer and not as a failed request.
const regulationError = "the document prints the regulations of several degrees"

// programPages names the pages of the regulations a program follows, where the
// document prints the regulations of several degrees, and says in a note what
// was left out. Any other document is read as a whole: it returns no pages.
func programPages(ctx context.Context, path, hint string) (pages []int, note string, err error) {
	defer func() {
		if r := recover(); r != nil {
			pages, note, err = nil, "", fmt.Errorf("read the table of contents: %v", r)
		}
	}()
	f, reader, err := pdf.Open(path)
	if err != nil {
		return nil, "", fmt.Errorf("open PDF: %w", err)
	}
	defer f.Close()
	text := func(page int) (string, error) {
		g, err := readPageGeometry(ctx, reader.Page(page))
		if err != nil {
			return "", fmt.Errorf("PDF page %d: %w", page, err)
		}
		return textInBox(g.glyphs, nil), nil
	}
	if reader.NumPage() < 2 {
		return nil, "", nil
	}
	cover, err := text(1)
	if err != nil {
		return nil, "", err
	}
	regs := tableOfContents(cover)
	if !severalDegrees(regs) {
		return nil, "", nil
	}
	// The contents and the pages must agree before either says which page
	// belongs to whom. Every regulation starts on a page that prints the number
	// the contents give it, and two of different degrees never share one: its
	// tables could belong to either.
	for i := range regs {
		r := &regs[i]
		if r.from < 2 || r.from > reader.NumPage() || (i > 0 && r.from < regs[i-1].from) {
			return nil, "", fmt.Errorf("%s, but its table of contents does not say on which page of this document %q begins; manual review required", regulationError, r.title)
		}
		if i > 0 && r.from == regs[i-1].from && strings.Join(r.degrees, " ") != strings.Join(regs[i-1].degrees, " ") {
			return nil, "", fmt.Errorf("%s, and two of them begin on page %d; manual review required", regulationError, r.from)
		}
		head, err := text(r.from)
		if err != nil {
			return nil, "", err
		}
		first, _, _ := strings.Cut(head, "\n")
		if m := printedPageNumber.FindStringSubmatch(first); m == nil || m[1] != strconv.Itoa(r.from) {
			return nil, "", fmt.Errorf("%s, but page %d does not print the page number the table of contents gives it; manual review required", regulationError, r.from)
		}
		// A regulation runs to the page before the next one that begins later.
		r.to = reader.NumPage()
		for _, next := range regs[i+1:] {
			if next.from > r.from {
				r.to = next.from - 1
				break
			}
		}
	}
	all := programDegree.FindAllString(hint, -1)
	if len(all) == 0 {
		return nil, "", fmt.Errorf("%s (%s), and the program names none; manual review required", regulationError, describeRegulations(regs))
	}
	// The name comes first in the hint and the degree after it.
	degree := strings.ToLower(all[len(all)-1])
	var own, other []regulation
	seen := map[int]bool{}
	for _, r := range regs {
		if !r.names(degree) {
			other = append(other, r)
			continue
		}
		own = append(own, r)
		for p := r.from; p <= r.to; p++ {
			if !seen[p] {
				seen[p] = true
				pages = append(pages, p)
			}
		}
	}
	switch {
	case len(own) == 0:
		return nil, "", fmt.Errorf("%s (%s), none of them for a %s program; manual review required", regulationError, describeRegulations(regs), degree)
	case len(other) == 0:
		return nil, "", nil // every one of them is this program's
	}
	return pages, fmt.Sprintf("The document prints the regulations of several degrees; only this program's were read, %s, not %s", describeRegulations(own), describeRegulations(other)), nil
}

// describeRegulations names regulations by their pages and titles.
func describeRegulations(regs []regulation) string {
	parts := make([]string, 0, len(regs))
	for _, r := range regs {
		pages := fmt.Sprintf("page %d", r.from)
		if r.to > r.from {
			pages = fmt.Sprintf("pages %d–%d", r.from, r.to)
		}
		parts = append(parts, fmt.Sprintf("%s %q", pages, r.title))
	}
	return strings.Join(parts, ", ")
}
