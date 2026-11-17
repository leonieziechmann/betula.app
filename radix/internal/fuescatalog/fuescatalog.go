// Package fuescatalog reads the BTU's FÜS catalog of a semester: the PDF „FÜS-Modulkatalog" its
// page „Aktuelle Infos" links (docs/radix/data-sources.md §19). It lists the modules admitted to the
// Fachübergreifendes Studium in that semester, by campus, and the „Liste der nicht zugelassenen
// Module je Studiengang": the negative lists of the Ordnung zum Fachübergreifenden Studium (§ 3
// Abs. 2), per program, some by degree, profile, Anwendungsfach, Schwerpunkt or Studienrichtung.
//
// Parse works on the text rows of the PDF (ReadRows), so that it is tested on rows without a PDF.
package fuescatalog

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// Text is a piece of text of a row, where it starts.
type Text struct {
	X float64
	S string
}

// Row is a line of a page as the PDF places it: its pieces left to right.
type Row struct {
	Page  int
	Y     float64
	Texts []Text
}

// String is the row's text, its pieces joined as they stand.
func (r Row) String() string {
	var b strings.Builder
	for _, t := range r.Texts {
		b.WriteString(t.S)
	}
	return strings.Join(strings.Fields(b.String()), " ")
}

// X is where the row's first piece that is not blank starts.
func (r Row) X() float64 {
	for _, t := range r.Texts {
		if strings.TrimSpace(t.S) != "" {
			return t.X
		}
	}
	return 0
}

// Module is a module of the catalog.
type Module struct {
	ID     string
	Title  string // as far as the module's row holds it; a title the PDF wraps may be cut
	Campus string // "sachsendorf", "senftenberg", "zentralcampus" as the catalog groups it; "" outside a group
}

// Exclusion is a line of a program's negative list.
type Exclusion struct {
	Program  string // the program's heading as printed („Informatik")
	Scope    string // the heading of the part it stands under („Anwendungsfach Physik"), "" for none
	ModuleID string
	Title    string
}

// Note is what a program's negative list says instead of modules („Bitte bei der
// Studiengangleitung erfragen!", „Keine weiteren Einschränkungen").
type Note struct {
	Program string
	Scope   string
	Text    string
}

// Catalog is a FÜS catalog as Parse reads it.
type Catalog struct {
	Semester   string // „2026W" for the winter 2026/27, „2026S" for the summer 2026
	Issued     string // the date it states, ISO („2026-10-14"); "" if it states none
	Modules    []Module
	Programs   []string // the headings of its list of programs, in its order
	Exclusions []Exclusion
	Notes      []Note
}

var (
	reSemester  = regexp.MustCompile(`(Winter|Sommer)semester\s+(\d{4})(?:\s*/\s*\d{2,4})?`)
	reIssued    = regexp.MustCompile(`^(\d{1,2})\.\s*(Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\s+(\d{4})$`)
	reModuleRow = regexp.MustCompile(`^(\d{5})\b\s*(.*)$`)
	reEntry     = regexp.MustCompile(`^[-–]\s*(\d{5})\b\s*(.*)$`)
	reBullet    = regexp.MustCompile(`^[-–]\s*(\S.*)$`)
	reIndex     = regexp.MustCompile(`^(\S.*?)\s*\.{3,}[.\s]*(\d+)$`)
	reFooter    = regexp.MustCompile(`^Seite\s+\d+\s+von\s+\d+$`)
	reScope     = regexp.MustCompile(`^(?i:unabhängig|für |nur |mit |anwendungs|schwerpunkt|studienrichtung|studium |bei |vertiefung|profil)`)
)

var months = map[string]int{"Januar": 1, "Februar": 2, "März": 3, "April": 4, "Mai": 5, "Juni": 6,
	"Juli": 7, "August": 8, "September": 9, "Oktober": 10, "November": 11, "Dezember": 12}

// SemesterOf reads the semester a text names: „Wintersemester 2026/2027" → „2026W", „Sommersemester
// 2026" → „2026S"; "" for none.
func SemesterOf(text string) string {
	m := reSemester.FindStringSubmatch(text)
	if m == nil {
		return ""
	}
	if m[1] == "Winter" {
		return m[2] + "W"
	}
	return m[2] + "S"
}

// Parse reads a catalog from the rows of its pages, in page order and top to bottom.
func Parse(rows []Row) (*Catalog, error) {
	c := &Catalog{}
	const (
		modules = iota
		index
		negatives
	)
	part := modules
	campus := ""
	personX := 0.0 // where the column of whom a module is with starts
	program, scope := "", ""
	known := map[string]bool{}
	bulletX := 0.0
	seen := map[string]bool{}
	for _, row := range rows {
		text := row.String()
		if text == "" || reFooter.MatchString(text) {
			continue
		}
		if m := reIssued.FindStringSubmatch(text); m != nil {
			if c.Issued == "" {
				day, _ := strconv.Atoi(m[1])
				c.Issued = fmt.Sprintf("%s-%02d-%02d", m[3], months[m[2]], day)
			}
			continue
		}
		if c.Semester == "" && row.Page == 1 {
			c.Semester = SemesterOf(text)
		}
		switch {
		case strings.HasPrefix(text, "Liste der Studiengänge"):
			part = index
			continue
		case strings.HasPrefix(text, "Liste der nicht zugelassenen Module"):
			part = negatives
			continue
		case strings.HasPrefix(text, "List of "):
			continue
		}
		switch part {
		case modules:
			if strings.HasPrefix(text, "Standort ") {
				campus = campusOf(text)
				continue
			}
			for _, t := range row.Texts {
				if strings.HasPrefix(strings.TrimSpace(t.S), "Modulver") {
					personX = t.X
				}
			}
			if m := reModuleRow.FindStringSubmatch(text); m != nil && !seen[m[1]] {
				seen[m[1]] = true
				c.Modules = append(c.Modules, Module{ID: m[1], Title: moduleTitle(row, personX), Campus: campus})
			}
		case index:
			if m := reIndex.FindStringSubmatch(text); m != nil {
				name := strings.TrimSpace(m[1])
				c.Programs = append(c.Programs, name)
				known[name] = true
			}
		case negatives:
			if m := reEntry.FindStringSubmatch(text); m != nil {
				if bulletX == 0 {
					bulletX = row.X()
				}
				if program != "" {
					c.Exclusions = append(c.Exclusions, Exclusion{Program: program, Scope: scope, ModuleID: m[1], Title: strings.TrimSpace(m[2])})
				}
				continue
			}
			if m := reBullet.FindStringSubmatch(text); m != nil {
				if program != "" {
					c.Notes = append(c.Notes, Note{Program: program, Scope: scope, Text: strings.TrimSpace(m[1])})
				}
				continue
			}
			if bulletX > 0 && row.X() > bulletX+10 {
				// a title the line before wrapped, or a note under a scope
				if text == "Keine weiteren Einschränkungen" && program != "" {
					c.Notes = append(c.Notes, Note{Program: program, Scope: scope, Text: text})
				} else if n := len(c.Exclusions); n > 0 && c.Exclusions[n-1].Program == program {
					c.Exclusions[n-1].Title += " " + text
				}
				continue
			}
			if text == "Keine weiteren Einschränkungen" {
				if program != "" {
					c.Notes = append(c.Notes, Note{Program: program, Scope: scope, Text: text})
				}
				continue
			}
			if known[text] {
				program, scope = text, ""
				continue
			}
			if program != "" && reScope.MatchString(text) {
				scope = strings.TrimSpace(strings.TrimSuffix(text, ":"))
				continue
			}
			// A heading the list of programs does not name: a program of its own, which the build
			// reports where it matches none.
			program, scope = text, ""
		}
	}
	if c.Semester == "" {
		return nil, fmt.Errorf("the catalog names no semester on its first page")
	}
	if len(c.Modules) == 0 {
		return nil, fmt.Errorf("the catalog of %s lists no module", c.Semester)
	}
	return c, nil
}

// campusOf names the campus of a heading „Standort ZENTRALCAMPUS (COTTBUS)".
func campusOf(heading string) string {
	h := strings.ToLower(heading)
	switch {
	case strings.Contains(h, "zentralcampus"):
		return "zentralcampus"
	case strings.Contains(h, "sachsendorf"):
		return "sachsendorf"
	case strings.Contains(h, "senftenberg"):
		return "senftenberg"
	}
	return ""
}

// moduleTitle is the text of a module's row between its number and the column of whom it is
// with (personX; 0 where the page has no such heading), as far as the row holds it.
func moduleTitle(row Row, personX float64) string {
	pieces := append([]Text(nil), row.Texts...)
	sort.SliceStable(pieces, func(i, j int) bool { return pieces[i].X < pieces[j].X })
	var b strings.Builder
	number := true
	for _, p := range pieces {
		if number {
			if reModuleRow.MatchString(strings.TrimSpace(p.S)) {
				number = false
			}
			continue
		}
		if personX > 0 && p.X >= personX-5 {
			break
		}
		b.WriteString(p.S)
	}
	return strings.Join(strings.Fields(b.String()), " ")
}

// Scope kinds: for whom a part of a negative list holds.
const (
	ScopeAll      = "all"      // every student of the program („Unabhängig vom Anwendungsfach")
	ScopeBachelor = "bachelor" // „für B.Sc", „Nur für B.Sc"
	ScopeMaster   = "master"   // „für M.Sc", „Nur für M.Sc"
	ScopeProfile  = "profile"  // „mit anwendungsbezogenem Studienprofil" …
	ScopeChoice   = "choice"   // what a student chooses: an Anwendungsfach, a Schwerpunkt, a Studienrichtung
)

// ScopeKind tells what a part's heading restricts.
func ScopeKind(scope string) string {
	s := strings.ToLower(strings.TrimSpace(scope))
	switch {
	case s == "" || strings.HasPrefix(s, "unabhängig"):
		return ScopeAll
	case strings.Contains(s, "b.sc") || strings.Contains(s, "bachelor"):
		return ScopeBachelor
	case strings.Contains(s, "m.sc") || strings.Contains(s, "master"):
		return ScopeMaster
	case strings.HasPrefix(s, "mit ") && strings.Contains(s, "profil"):
		return ScopeProfile
	}
	return ScopeChoice
}

// ProfileHolds tells whether a profile heading („mit universitärem und dualem Studienprofil")
// holds for a program of the type („university", „applied", "" unknown) and variant (dual or not).
func ProfileHolds(scope, degreeType string, dual bool) bool {
	s := strings.ToLower(scope)
	applied := strings.Contains(s, "anwendungsbezogen")
	university := strings.Contains(s, "universitär")
	withDual := strings.Contains(s, "dual")
	isApplied := degreeType == "applied"
	switch {
	case applied && withDual:
		return isApplied
	case applied:
		return isApplied && !dual
	case university && withDual:
		return !isApplied || dual
	case university:
		return !isApplied && !dual
	}
	return false
}

// ChoiceLabel is how a choice's heading is named beside a module: „Bei Anwendungsfach Natural
// Sciences and Engineering:" → „Anwendungsfach Natural Sciences and Engineering".
func ChoiceLabel(scope string) string {
	s := strings.TrimSpace(strings.TrimSuffix(strings.TrimSpace(scope), ":"))
	s = strings.TrimPrefix(s, "Bei ")
	return s
}
