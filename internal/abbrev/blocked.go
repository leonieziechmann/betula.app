package abbrev

import (
	_ "embed"
	"fmt"
	"regexp"
	"strings"
	"unicode"

	"github.com/leonieziechmann/betula/internal/normalize"
)

//go:embed blocked.tsv
var blockedTSV string

type blockedForm struct {
	spelling, reason string
}

// blocked maps the upper case of a blocked form to its spelling and reason: the lines of
// blocked.tsv and the building tokens of short room names.
var blocked = func() map[string]blockedForm {
	m, err := parseBlocked(blockedTSV)
	if err != nil {
		panic(err) // the embedded file; TestTheBlockedFile reads it first
	}
	for _, t := range normalize.BuildingTokens() {
		if _, ok := m[strings.ToUpper(t)]; !ok {
			m[strings.ToUpper(t)] = blockedForm{t, "a building in short room names"}
		}
	}
	return m
}()

func parseBlocked(text string) (map[string]blockedForm, error) {
	m := map[string]blockedForm{}
	for i, line := range strings.Split(text, "\n") {
		line = strings.TrimRight(line, "\r")
		if strings.TrimSpace(line) == "" || strings.HasPrefix(line, "#") {
			continue
		}
		cols := strings.Split(line, "\t")
		if len(cols) != 2 || cols[1] == "" {
			return nil, fmt.Errorf("blocked line %d: want form and reason separated by a tab", i+1)
		}
		form := cols[0]
		if n := runes(form); n < 2 || n > 10 || strings.IndexFunc(form, unicode.IsSpace) >= 0 || Stem(form) != form {
			return nil, fmt.Errorf("blocked line %d: %q is not a form of 2 to 10 characters without spaces and designator", i+1, form)
		}
		k := strings.ToUpper(form)
		if _, dup := m[k]; dup {
			return nil, fmt.Errorf("blocked line %d: %s is listed twice", i+1, form)
		}
		m[k] = blockedForm{form, cols[1]}
	}
	return m, nil
}

// A designator at the end of an abbreviation: a language level (-B1.1, -A1-A2) or a series
// number (1, 2.1).
var reTrailingDesignator = regexp.MustCompile(`(?:-[ABC][12](?:\.\d)?)+$|-?\d+(?:\.\d+)*$`)

// Stem is an abbreviation without the designator a reader sees at its end: ST for ST1, SS for
// SS-A1, DaF for DaF-B1.1. A form that is nothing but a designator is its own stem.
func Stem(form string) string {
	s := form
	for {
		t := reTrailingDesignator.ReplaceAllString(s, "")
		if t == s || t == "" {
			break
		}
		s = t
	}
	return s
}

// Blocked reports whether a derived abbreviation is one no module may show (blocked.tsv, the
// building tokens of short room names), and why. The designator does not help (SS1 is
// blocked), the case does not matter, and a title that has the form as a word of its own may
// use it.
func Blocked(form, title string) (reason string, ok bool) {
	b, found := blocked[strings.ToUpper(Stem(form))]
	if !found {
		return "", false
	}
	for _, w := range strings.FieldsFunc(title, func(r rune) bool { return !unicode.IsLetter(r) && !unicode.IsDigit(r) }) {
		if w == b.spelling {
			return "", false
		}
	}
	return b.spelling + ": " + b.reason, true
}
