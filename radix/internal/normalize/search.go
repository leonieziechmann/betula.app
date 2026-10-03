package normalize

import (
	"slices"
	"strings"
	"unicode"
	"unicode/utf8"
)

// Search terms (docs/radix/schema-v2.md, „Search“): what Folia's search compares a query with, folded
// once by the build instead of by every reader. Folia folds the query with folia_search::fold,
// so SearchFold folds as that does, character for character, and SearchWords parts a text where
// that parts it: testdata/search.tsv holds both to the same cases (Folia's tests read it too).

// SearchFillers are left out of a title's initials („Algorithmen und Datenstrukturen“ is „ad“),
// as Folia leaves them out of its initials and of a query (folia_search::FILLERS; the same
// line of testdata/search.tsv holds both to one list).
var SearchFillers = []string{"of", "and", "und", "der", "die", "das", "in", "im", "fur", "the", "zur", "zum", "von", "mit"}

// SearchFold lowercases and removes the diacritics of the common Latin letters: „Ökologie &
// Straße“ is „okologie & strasse“. Everything else stays as it is.
func SearchFold(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		// The one letter whose lower case is two characters (Rust's char::to_lowercase).
		if r == 'İ' {
			b.WriteString("i̇")
			continue
		}
		switch r = unicode.ToLower(r); r {
		case 'ä', 'à', 'á', 'â', 'ã', 'å':
			b.WriteByte('a')
		case 'ö', 'ò', 'ó', 'ô', 'õ', 'ø':
			b.WriteByte('o')
		case 'ü', 'ù', 'ú', 'û':
			b.WriteByte('u')
		case 'è', 'é', 'ê', 'ë':
			b.WriteByte('e')
		case 'ì', 'í', 'î', 'ï':
			b.WriteByte('i')
		case 'ç':
			b.WriteByte('c')
		case 'ñ':
			b.WriteByte('n')
		case 'ß':
			b.WriteString("ss")
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}

// isWordRune is Rust's char::is_alphanumeric: a letter or a number in the sense of Unicode.
func isWordRune(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsNumber(r) ||
		unicode.In(r, unicode.Other_Alphabetic, unicode.Other_Lowercase, unicode.Other_Uppercase)
}

// SearchWords are the words of s as the search sees them: folded, and parted at every character
// that is neither a letter nor a digit („Python-Programmierung“ is python and programmierung).
func SearchWords(s string) []string {
	return strings.FieldsFunc(SearchFold(s), func(r rune) bool { return !isWordRune(r) })
}

// SearchText is a title as the search compares it: its words, separated by one space, and after
// them once more as one word each part of it that is written in parts, so that it is found
// written either way („B.Sc.“ is also bsc, „E-Technik“ also etechnik). "" for a title without a
// letter or a digit.
func SearchText(title string) string {
	words := SearchWords(title)
	for _, chunk := range strings.Fields(title) {
		if parts := SearchWords(chunk); len(parts) > 1 {
			words = append(words, strings.Join(parts, ""))
		}
	}
	return strings.Join(words, " ")
}

// SearchInitials are the first letters of the words of a title, the fillers left out:
// „Theoretische Informatik“ is ti, „Algorithmen und Datenstrukturen“ ad. "" for fewer than two.
func SearchInitials(title string) string {
	var b strings.Builder
	n := 0
	for _, w := range SearchWords(title) {
		if slices.Contains(SearchFillers, w) {
			continue
		}
		r, _ := utf8.DecodeRuneInString(w)
		b.WriteRune(r)
		n++
	}
	if n < 2 {
		return ""
	}
	return b.String()
}

// SearchAbbrev is an abbreviation as the search compares it: folded, in one word („AuP-b“ is
// aupb, „A&M“ am, „AMÖ“ amo).
func SearchAbbrev(abbrev string) string {
	return strings.Join(SearchWords(abbrev), "")
}
