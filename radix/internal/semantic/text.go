// Package semantic computes what Folia's semantic search compares a query with: one vector
// per module, of a passage made of the module's titles, a summary Gemini wrote of it, search
// terms, and its description.
//
// The texts are composed here, once, for the stage that writes summaries and vectors
// (internal/service, semantic.go) and for the build that looks the vectors up (catalogbuild),
// so the two always mean the same passage by the same hash.
package semantic

import (
	"crypto/sha256"
	"encoding/hex"
	"regexp"
	"strings"
)

// Text is what a module says about itself: the parts a summary is written from and a passage
// is made of. Every field is taken with its white space collapsed.
type Text struct {
	TitleDE, TitleEN   string
	Contents, Outcomes string
}

// Summary is what Gemini wrote about a Text.
type Summary struct {
	DE, EN   string
	Keywords []string
}

// Clean collapses the white space of every field, as Hash and Passage see them. Passage and
// Plain want the text as it is: a cleaned description has lost the lines that mark its lists.
func (t Text) Clean() Text {
	return Text{TitleDE: collapse(t.TitleDE), TitleEN: collapse(t.TitleEN), Contents: collapse(t.Contents), Outcomes: collapse(t.Outcomes)}
}

// Hash identifies the text a summary was written from: equal texts share one summary.
func (t Text) Hash() string {
	c := t.Clean()
	return hash("text\x1f" + c.TitleDE + "\x1f" + c.TitleEN + "\x1f" + c.Contents + "\x1f" + c.Outcomes)
}

// Titles is the German title and, where it differs, the English one: „Analysis III“,
// „Werkstofftechnik / Materials Engineering“.
func (t Text) Titles() string {
	c := t.Clean()
	switch {
	case c.TitleDE == "":
		return c.TitleEN
	case c.TitleEN == "" || c.TitleEN == c.TitleDE:
		return c.TitleDE
	}
	return c.TitleDE + " / " + c.TitleEN
}

// Passage is the text a module's vector is computed from (as an e5 passage, without the
// „passage: “ the encoder adds): the titles, then the summary in both languages and the
// search terms when there is a summary, then the description in plain words (Plain). The summary comes before the
// description because the model reads 512 tokens at most, and a long description would cut it
// off. Measured on the catalog (folia/crates/semantic/README.md, „Quality“), this passage finds a module by
// what students type better than the description alone.
func Passage(t Text, s *Summary) string {
	c := t.Clean()
	var b strings.Builder
	b.WriteString(c.Titles())
	b.WriteString(". ")
	if s != nil {
		b.WriteString(collapse(s.DE))
		b.WriteString(" ")
		b.WriteString(collapse(s.EN))
		b.WriteString(" ")
		keywords := make([]string, 0, len(s.Keywords))
		for _, k := range s.Keywords {
			if k = collapse(k); k != "" {
				keywords = append(keywords, k)
			}
		}
		b.WriteString(strings.Join(keywords, ", "))
		b.WriteString(". ")
	}
	body := Plain(t.Contents)
	if outcomes := Plain(t.Outcomes); outcomes != "" {
		if body != "" {
			body += " "
		}
		body += outcomes
	}
	b.WriteString(body)
	return b.String()
}

// PassageHash identifies a passage: its vector is looked up by it.
func PassageHash(passage string) string {
	return hash("passage\x1f" + passage)
}

func hash(s string) string {
	sum := sha256.Sum256([]byte(s))
	return hex.EncodeToString(sum[:])
}

func collapse(s string) string {
	return strings.Join(strings.Fields(s), " ")
}

// Plain is a module text (Markdown since schema 10, internal/parser.Markdown) as the words it
// says, for a passage and for Gemini: without the markers of its lists („- ", „1. "), the stars
// of strong and emphasized text, the backslash of a line break and those that escape a
// character; the labels of a list („(1)", „a)") stay, they are words of the text. One line.
func Plain(markdown string) string {
	var b strings.Builder
	for _, line := range strings.Split(markdown, "\n") {
		line = strings.TrimSpace(line)
		line = strings.TrimSuffix(line, "\\") // a line break
		line = listMarker.ReplaceAllString(line, "")
		b.WriteString(unescape(line))
		b.WriteByte(' ')
	}
	return collapse(b.String())
}

// listMarker is the marker of an item of a list, as Radix writes it: a bullet or a number.
var listMarker = regexp.MustCompile(`^(?:[-*+]|\d{1,9}[.)])\s+`)

// unescape drops the stars of strong and emphasized text and keeps a character a backslash
// escapes as it is.
func unescape(line string) string {
	var b strings.Builder
	escaped := false
	for _, r := range line {
		switch {
		case escaped:
			b.WriteRune(r)
			escaped = false
		case r == '\\':
			escaped = true
		case r == '*':
		default:
			b.WriteRune(r)
		}
	}
	if escaped {
		b.WriteByte('\\')
	}
	return b.String()
}
