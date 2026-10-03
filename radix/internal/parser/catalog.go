package parser

import (
	"fmt"
	"io"
	"net/url"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

const defaultBaseURL = "https://www.b-tu.de"

// CatalogParser parses the main BTU module overview page (b-tu.de/modul).
type CatalogParser struct {
	BaseURL string
}

// NewCatalogParser creates a new CatalogParser.
func NewCatalogParser(baseURL string) *CatalogParser {
	if baseURL == "" {
		baseURL = defaultBaseURL
	}
	return &CatalogParser{BaseURL: strings.TrimRight(baseURL, "/")}
}

// Parse extracts module summaries from the HTML reader.
func (p *CatalogParser) Parse(r io.Reader) ([]model.ModuleSummary, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var summaries []model.ModuleSummary
	now := time.Now().UTC()

	// Find the module list table body
	// Typically: <tbody class="list"> within table
	tbodies := FindAllByTag(doc, atom.Tbody)
	var targetTbody *html.Node
	for _, tb := range tbodies {
		if HasClass(tb, "list") {
			targetTbody = tb
			break
		}
	}

	// Fallback to searching all trs if tbody.list is not explicitly found
	var rows []*html.Node
	if targetTbody != nil {
		rows = FindAllByTag(targetTbody, atom.Tr)
	} else {
		rows = FindAllByTag(doc, atom.Tr)
	}

	for _, tr := range rows {
		tdNumber := FindFirstByClass(tr, "moduleNumber")
		tdTitle := FindFirstByClass(tr, "title")

		if tdNumber == nil || tdTitle == nil {
			continue
		}

		// Extract ID and href from link inside tdNumber if present
		var moduleID, rawURL string
		aNode := FindFirstByTag(tdNumber, atom.A)
		if aNode != nil {
			moduleID = CleanSingleLine(NodeText(aNode))
			rawURL = GetAttr(aNode, "href")
		} else {
			moduleID = CleanSingleLine(NodeText(tdNumber))
		}

		moduleTitle := CleanSingleLine(NodeText(tdTitle))

		if moduleID == "" && moduleTitle == "" {
			continue
		}

		// Resolve relative URL to absolute URL
		var fullURL string
		if rawURL != "" {
			if strings.HasPrefix(rawURL, "http://") || strings.HasPrefix(rawURL, "https://") {
				fullURL = rawURL
			} else {
				parsedBase, err := url.Parse(p.BaseURL)
				if err == nil {
					relURL, err := url.Parse(rawURL)
					if err == nil {
						fullURL = parsedBase.ResolveReference(relURL).String()
					}
				}
				if fullURL == "" {
					fullURL = p.BaseURL + "/" + strings.TrimLeft(rawURL, "/")
				}
			}
		} else if moduleID != "" {
			fullURL = fmt.Sprintf("%s/modul/%s", p.BaseURL, moduleID)
		}

		summaries = append(summaries, model.ModuleSummary{
			ID:        moduleID,
			Code:      moduleID,
			Title:     moduleTitle,
			URL:       fullURL,
			ScrapedAt: now,
		})
	}

	return summaries, nil
}
