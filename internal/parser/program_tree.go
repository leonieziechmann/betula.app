package parser

import (
	"fmt"
	"io"
	"net/url"
	"path"
	"regexp"
	"strings"

	"github.com/jakob/btu-scraper/internal/model"
	"golang.org/x/net/html"
)

var (
	stgRegex      = regexp.MustCompile(`stg=(\d+)`)
	abschlRegex   = regexp.MustCompile(`abschl=(\d+)`)
	pversionRegex = regexp.MustCompile(`pversion=([^&|#]+)`)
)

// ProgramNode represents a study program node in the QIS tree.
type ProgramNode struct {
	Name   string
	Code   string
	NodeID string
	URL    string
}

// DegreeNode represents an Abschluss node for a study program.
type DegreeNode struct {
	Degree string
	Code   string
	NodeID string
	URL    string
}

// PONode represents a PO-Version node under a degree.
type PONode struct {
	POVersion string
	NodeID    string
	URL       string
}

// ProgramTreeParser parses the various levels of the QIS study program tree.
type ProgramTreeParser struct{}

// NewProgramTreeParser returns a new ProgramTreeParser instance.
func NewProgramTreeParser() *ProgramTreeParser {
	return &ProgramTreeParser{}
}

// ParseRootPrograms extracts all study programs listed on the root tree page.
func (p *ProgramTreeParser) ParseRootPrograms(r io.Reader, baseURL string) ([]ProgramNode, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var results []ProgramNode
	seen := make(map[string]bool)

	var walk func(*html.Node)
	walk = func(n *html.Node) {
		if n.Type == html.ElementNode && n.Data == "a" {
			href := GetAttr(n, "href")
			text := CleanSingleLine(NodeText(n))

			if strings.HasPrefix(text, "Studiengang:") {
				name := strings.TrimSpace(strings.TrimPrefix(text, "Studiengang:"))
				fullURL := resolveURL(baseURL, href)
				parsedURL, _ := url.Parse(fullURL)
				nodeID := ""
				if parsedURL != nil {
					nodeID = parsedURL.Query().Get("nodeID")
				}

				code := ""
				if m := stgRegex.FindStringSubmatch(nodeID); len(m) > 1 {
					code = m[1]
				}

				key := name + "_" + code
				if !seen[key] && name != "" {
					seen[key] = true
					results = append(results, ProgramNode{
						Name:   name,
						Code:   code,
						NodeID: nodeID,
						URL:    fullURL,
					})
				}
			}
		}

		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}

	walk(doc)
	return results, nil
}

// ParseDegrees extracts all degrees (Module für Abschluss) for a study program.
func (p *ProgramTreeParser) ParseDegrees(r io.Reader, baseURL string) ([]DegreeNode, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var results []DegreeNode
	seen := make(map[string]bool)

	var walk func(*html.Node)
	walk = func(n *html.Node) {
		if n.Type == html.ElementNode && n.Data == "a" {
			href := GetAttr(n, "href")
			text := CleanSingleLine(NodeText(n))

			if strings.HasPrefix(text, "Module für Abschluss:") {
				degree := strings.TrimSpace(strings.TrimPrefix(text, "Module für Abschluss:"))
				fullURL := resolveURL(baseURL, href)
				parsedURL, _ := url.Parse(fullURL)
				nodeID := ""
				if parsedURL != nil {
					nodeID = parsedURL.Query().Get("nodeID")
				}

				code := ""
				if m := abschlRegex.FindStringSubmatch(nodeID); len(m) > 1 {
					code = m[1]
				}

				key := degree + "_" + code
				if !seen[key] && degree != "" {
					seen[key] = true
					results = append(results, DegreeNode{
						Degree: degree,
						Code:   code,
						NodeID: nodeID,
						URL:    fullURL,
					})
				}
			}
		}

		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}

	walk(doc)
	return results, nil
}

// ParsePOVersions extracts all PO versions available under an Abschluss.
func (p *ProgramTreeParser) ParsePOVersions(r io.Reader, baseURL string) ([]PONode, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var results []PONode
	seen := make(map[string]bool)

	// Search inside ul.treelist to avoid matching breadcrumbs
	var walk func(*html.Node, bool)
	walk = func(n *html.Node, inTreeList bool) {
		currentInTree := inTreeList
		if n.Type == html.ElementNode && n.Data == "ul" && strings.Contains(GetAttr(n, "class"), "treelist") {
			currentInTree = true
		}

		if currentInTree && n.Type == html.ElementNode && n.Data == "a" {
			href := GetAttr(n, "href")
			if strings.Contains(href, "pversion") {
				text := CleanSingleLine(NodeText(n))
				po := text
				if strings.HasPrefix(po, "PO-Version:") {
					po = strings.TrimSpace(strings.TrimPrefix(po, "PO-Version:"))
				}

				fullURL := resolveURL(baseURL, href)
				parsedURL, _ := url.Parse(fullURL)
				nodeID := ""
				if parsedURL != nil {
					nodeID = parsedURL.Query().Get("nodeID")
				}

				key := nodeID
				if key == "" {
					key = fullURL
				}

				if !seen[key] && po != "" {
					seen[key] = true
					results = append(results, PONode{
						POVersion: po,
						NodeID:    nodeID,
						URL:       fullURL,
					})
				}
			}
		}

		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c, currentInTree)
		}
	}

	walk(doc, false)
	return results, nil
}

// ParsePODocuments extracts all statute and amendment links from a PO version page.
func (p *ProgramTreeParser) ParsePODocuments(r io.Reader, baseURL string) ([]model.ProgramRegulationDocument, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var docs []model.ProgramRegulationDocument
	seen := make(map[string]bool)

	var walk func(*html.Node)
	walk = func(n *html.Node) {
		if n.Type == html.ElementNode && n.Data == "a" {
			href := GetAttr(n, "href")
			target := GetAttr(n, "target")
			title := GetAttr(n, "title")

			// Check if this link points to opus4, a PDF, or has target="_blank" with a statute icon
			isDocLink := strings.Contains(href, "opus4") || strings.HasSuffix(strings.ToLower(href), ".pdf") || target == "_blank"

			if isDocLink && href != "" {
				fullURL := resolveURL(baseURL, href)
				if !seen[fullURL] {
					// Check child img for icon and alt text if title is empty
					icon := ""
					for c := n.FirstChild; c != nil; c = c.NextSibling {
						if c.Type == html.ElementNode && c.Data == "img" {
							src := GetAttr(c, "src")
							icon = path.Base(src)
							if title == "" {
								title = GetAttr(c, "alt")
							}
							break
						}
					}

					// Verify this is indeed a statute or amendment
					isRegulation := strings.Contains(icon, "pruefung") ||
						strings.Contains(icon, "satzung") ||
						strings.Contains(strings.ToLower(title), "ordnung") ||
						strings.Contains(strings.ToLower(title), "satzung") ||
						strings.Contains(href, "opus4")

					if isRegulation {
						seen[fullURL] = true

						docType := "other"
						lowerTitle := strings.ToLower(title)
						lowerIcon := strings.ToLower(icon)

						if strings.Contains(lowerTitle, "änderung") || strings.Contains(lowerTitle, "aenderung") || strings.Contains(lowerIcon, "satzungsaenderung") {
							docType = "amendment"
						} else if strings.Contains(lowerTitle, "prüfungsordnung") || strings.Contains(lowerTitle, "pruefungsordnung") ||
							strings.Contains(lowerTitle, "studienordnung") || strings.Contains(lowerIcon, "pruefungsordnung") {
							docType = "statute"
						}

						if title == "" {
							title = path.Base(href)
						}

						docs = append(docs, model.ProgramRegulationDocument{
							Title:          title,
							DocType:        docType,
							URL:            fullURL,
							Icon:           icon,
							DownloadStatus: "not_attempted",
						})
					}
				}
			}
		}

		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}

	walk(doc)
	return docs, nil
}

func resolveURL(base, ref string) string {
	if strings.HasPrefix(ref, "http://") || strings.HasPrefix(ref, "https://") {
		return ref
	}
	if base == "" {
		return ref
	}
	parsedBase, err := url.Parse(base)
	if err != nil {
		return ref
	}
	parsedRef, err := url.Parse(ref)
	if err != nil {
		return ref
	}
	return parsedBase.ResolveReference(parsedRef).String()
}
