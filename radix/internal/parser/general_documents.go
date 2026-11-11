package parser

import (
	"bytes"
	"fmt"
	"net/url"
	"regexp"
	"strings"

	"github.com/leonieziechmann/betula/radix/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// rePDFNote is the note „(PDF)" the BTU's page writes after a link.
var rePDFNote = regexp.MustCompile(`(?i)\(\s*PDF[^)]*\)`)

// ParseGeneralDocuments reads the BTU's page of Grundsatzordnungen (German or English, they are
// built alike): every link of its content in the order the page has them, each with the <h2>
// heading of its section and the fold it sits in. The content is what the CMS marks for its
// search (between the comments TYPO3SEARCH_begin and TYPO3SEARCH_end), without the menus; links
// before the first heading, anchors on the page itself and links into the page's head, menus and
// foot are left out. pageURL resolves relative links.
//
// A link's title is what the page writes for it: its text and the words after it up to the next
// link („Satzung über … vom 27. September 2016 (PDF)"), without „(PDF)"; where that is empty, its
// title attribute. The dates in it are the BTU's: some are wrong.
func ParseGeneralDocuments(body []byte, pageURL string) ([]model.GeneralDocument, error) {
	doc, err := html.Parse(bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("failed to parse the page of Grundsatzordnungen: %w", err)
	}
	base, err := url.Parse(pageURL)
	if err != nil {
		return nil, fmt.Errorf("invalid page address %q: %w", pageURL, err)
	}

	// Without the CMS's marks the whole page is content, but for its head, menus and foot.
	marked := containsComment(doc, "TYPO3SEARCH_begin")
	inContent := !marked

	var (
		docs    []model.GeneralDocument
		section string
	)
	var walk func(n *html.Node, fold string)
	walk = func(n *html.Node, fold string) {
		switch n.Type {
		case html.CommentNode:
			switch strings.TrimSpace(n.Data) {
			case "TYPO3SEARCH_begin":
				inContent = true
			case "TYPO3SEARCH_end":
				inContent = false
			}
			return
		case html.ElementNode:
			switch n.DataAtom {
			case atom.Nav, atom.Header, atom.Footer, atom.Script, atom.Style:
				// A section's own <header> holds its heading; the page's head and menus are the rest.
				if n.DataAtom != atom.Header || FindFirstByTag(n, atom.H2) == nil {
					return
				}
			case atom.H2:
				if inContent {
					section = CleanSingleLine(NodeText(n))
				}
				return
			case atom.A:
				if inContent && section != "" {
					if d, ok := generalDocument(n, base); ok {
						d.Section, d.Fold = section, fold
						docs = append(docs, d)
					}
				}
				return
			}
			if HasClass(n, "accordion-content") {
				fold = foldTitle(n)
			}
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c, fold)
		}
	}
	walk(doc, "")
	return docs, nil
}

// generalDocument is the link a as a document: its absolute address and its title. Not one for an
// anchor on the page, an address that is not http(s), or a link without any words.
func generalDocument(a *html.Node, base *url.URL) (model.GeneralDocument, bool) {
	href := strings.TrimSpace(GetAttr(a, "href"))
	if href == "" || strings.HasPrefix(href, "#") {
		return model.GeneralDocument{}, false
	}
	target, err := base.Parse(href)
	if err != nil || (target.Scheme != "http" && target.Scheme != "https") {
		return model.GeneralDocument{}, false
	}
	target.Fragment = ""

	words := NodeText(a)
	for s := a.NextSibling; s != nil; s = s.NextSibling {
		if s.Type == html.ElementNode && (s.DataAtom == atom.A || FindFirstByTag(s, atom.A) != nil) {
			break
		}
		words += " " + NodeText(s)
	}
	title := CleanSingleLine(rePDFNote.ReplaceAllString(words, " "))
	if title == "" {
		title = CleanSingleLine(GetAttr(a, "title"))
	}
	if title == "" {
		return model.GeneralDocument{}, false
	}
	return model.GeneralDocument{Title: title, URL: target.String()}, true
}

// foldTitle is the title of the fold whose content is n (an element of the class
// accordion-content): the accordion-title beside it, or the link that opens it by its id.
func foldTitle(content *html.Node) string {
	if item := content.Parent; item != nil {
		for c := item.FirstChild; c != nil; c = c.NextSibling {
			if c.Type == html.ElementNode && HasClass(c, "accordion-title") {
				return CleanSingleLine(NodeText(c))
			}
		}
	}
	return ""
}

func containsComment(n *html.Node, text string) bool {
	if n.Type == html.CommentNode && strings.TrimSpace(n.Data) == text {
		return true
	}
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		if containsComment(c, text) {
			return true
		}
	}
	return false
}
