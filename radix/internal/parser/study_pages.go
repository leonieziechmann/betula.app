package parser

import (
	"bytes"
	"fmt"
	"net/url"
	"strings"

	"github.com/leonieziechmann/betula/radix/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// ParseStudyPages reads the BTU's page „Studienorganisation" (German or English, they are built
// alike): every tile of its menus of teasers (`btu-menu-teaser_element`) in the order the page has
// them, each with the <h2> heading of its group. The content is what the CMS marks for its search
// (between the comments TYPO3SEARCH_begin and TYPO3SEARCH_end), without the menus of the site; what
// is not a tile of such a menu (the contacts at the page's end) is left out. pageURL resolves
// relative links.
//
// A tile's title is the text of its heading (`btu-menu-teaser_title`), else the title of its link;
// its address the link of its heading, else the first link of the tile (its picture); its abstract
// the text of `btu-menu-teaser_abstract`. An address keeps its fragment: a tile may point into a
// page („…/fachstudienberatung#c403684").
func ParseStudyPages(body []byte, pageURL string) ([]model.StudyPage, error) {
	doc, err := html.Parse(bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("failed to parse the page Studienorganisation: %w", err)
	}
	base, err := url.Parse(pageURL)
	if err != nil {
		return nil, fmt.Errorf("invalid page address %q: %w", pageURL, err)
	}

	marked := containsComment(doc, "TYPO3SEARCH_begin")
	inContent := !marked

	var (
		pages   []model.StudyPage
		section string
	)
	var walk func(n *html.Node)
	walk = func(n *html.Node) {
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
			case atom.Nav, atom.Footer, atom.Script, atom.Style:
				return
			case atom.H2:
				if inContent {
					section = CleanSingleLine(NodeText(n))
				}
				return
			}
			if HasClass(n, "btu-menu-teaser_element") {
				if inContent && section != "" {
					if p, ok := studyPage(n, base); ok {
						p.Section = section
						pages = append(pages, p)
					}
				}
				return
			}
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}
	walk(doc)
	return pages, nil
}

// studyPage is the tile n as a page: its title, its absolute address and its abstract. Not one
// without an address that is http(s) or without a title.
func studyPage(tile *html.Node, base *url.URL) (model.StudyPage, bool) {
	heading := FindFirstByClass(tile, "btu-menu-teaser_title")
	link := (*html.Node)(nil)
	if heading != nil {
		link = FindFirstByTag(heading, atom.A)
	}
	if link == nil {
		link = FindFirstByTag(tile, atom.A)
	}
	if link == nil {
		return model.StudyPage{}, false
	}
	href := strings.TrimSpace(GetAttr(link, "href"))
	if href == "" || strings.HasPrefix(href, "#") {
		return model.StudyPage{}, false
	}
	target, err := base.Parse(href)
	if err != nil || (target.Scheme != "http" && target.Scheme != "https") {
		return model.StudyPage{}, false
	}
	title := ""
	if heading != nil {
		title = CleanSingleLine(NodeText(heading))
	}
	if title == "" {
		title = CleanSingleLine(GetAttr(link, "title"))
	}
	if title == "" {
		return model.StudyPage{}, false
	}
	abstract := ""
	if a := FindFirstByClass(tile, "btu-menu-teaser_abstract"); a != nil {
		abstract = CleanSingleLine(NodeText(a))
	}
	return model.StudyPage{Title: title, Abstract: abstract, URL: target.String()}, true
}
