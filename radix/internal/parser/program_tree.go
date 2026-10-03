package parser

import (
	"fmt"
	"io"
	"net/url"
	"path"
	"regexp"
	"strings"

	"github.com/leonieziechmann/betula/radix/internal/model"
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

var (
	moduleLeafCodeRegex = regexp.MustCompile(`^(\d{5})\s+(.+)$`)
)

// QISTreeNode represents any category branch or module leaf node inside the QIS PO hierarchy.
type QISTreeNode struct {
	Text     string
	URL      string
	NodeID   string
	IsModule bool
	ModuleID string
	Title    string
}

// ParsePOBranchNodes extracts all child branch links and module leaves from a QIS PO node page.
func (p *ProgramTreeParser) ParsePOBranchNodes(r io.Reader, baseURL string) ([]QISTreeNode, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var nodes []QISTreeNode
	seen := make(map[string]bool)

	var walk func(*html.Node, bool)
	walk = func(n *html.Node, inTreelist bool) {
		currentInTree := inTreelist
		if n.Type == html.ElementNode && n.Data == "ul" && strings.Contains(GetAttr(n, "class"), "treelist") {
			currentInTree = true
		}

		if currentInTree && n.Type == html.ElementNode && n.Data == "a" {
			href := GetAttr(n, "href")
			class := GetAttr(n, "class")
			text := CleanSingleLine(NodeText(n))

			if strings.Contains(href, "nodeID") && !strings.Contains(class, "breadCrumb") && text != "" && text != "Oberste Ebene" {
				fullURL := resolveURL(baseURL, href)
				parsedURL, _ := url.Parse(fullURL)
				nodeID := ""
				if parsedURL != nil {
					nodeID = parsedURL.Query().Get("nodeID")
				}

				if nodeID != "" && !seen[nodeID] {
					seen[nodeID] = true
					isMod := false
					modID := ""
					title := text

					if m := moduleLeafCodeRegex.FindStringSubmatch(text); len(m) > 2 {
						isMod = true
						modID = m[1]
						title = m[2]
					} else if strings.Contains(nodeID, "pruefung:") {
						isMod = true
					}

					nodes = append(nodes, QISTreeNode{
						Text:     text,
						URL:      fullURL,
						NodeID:   nodeID,
						IsModule: isMod,
						ModuleID: modID,
						Title:    title,
					})
				}
			}
		}

		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c, currentInTree)
		}
	}

	walk(doc, false)
	return nodes, nil
}

// AnalyzeQISPath extracts StudySection, SubjectArea, ModuleType, and Specialization from a QIS hierarchy path.
func AnalyzeQISPath(path []string) (studySection, subjectArea, moduleType, specialization string) {
	// Clean path components: remove root headers
	var clean []string
	for _, p := range path {
		t := strings.TrimSpace(p)
		if t == "" || t == "Gesamtkonto" || t == "Oberste Ebene" ||
			strings.HasPrefix(t, "Studiengang:") ||
			strings.HasPrefix(t, "Module für Abschluss:") ||
			strings.HasPrefix(t, "PO-Version:") {
			continue
		}
		clean = append(clean, t)
	}

	// 1. Determine studySection
	for _, segment := range clean {
		low := strings.ToLower(segment)
		if strings.Contains(low, "grundstudium") || strings.Contains(low, "basisstudium") {
			studySection = "Grundstudium"
			break
		} else if strings.Contains(low, "fachstudium") || strings.Contains(low, "hauptstudium") {
			studySection = "Fachstudium"
			break
		} else if strings.Contains(low, "vertiefungsstudium") {
			studySection = "Vertiefungsstudium"
			break
		} else if strings.Contains(low, "kernstudium") {
			studySection = "Kernstudium"
			break
		}
	}

	// 2. Determine moduleType
	moduleType = "Pflicht"
	for _, segment := range clean {
		low := strings.ToLower(segment)
		if strings.Contains(low, "wahlpflicht") || strings.Contains(low, "wahlbereich") ||
			strings.Contains(low, "wahlmodul") || strings.Contains(low, "wpf") || strings.Contains(low, "wahl") {
			moduleType = "Wahlpflicht"
			break
		} else if strings.Contains(low, "bachelor-arbeit") || strings.Contains(low, "bachelorarbeit") ||
			strings.Contains(low, "master-arbeit") || strings.Contains(low, "masterarbeit") ||
			strings.Contains(low, "abschlussarbeit") || strings.Contains(low, "kolloquium") {
			moduleType = "Abschlussarbeit"
			break
		} else if strings.Contains(low, "füs") || strings.Contains(low, "fachübergreifend") {
			moduleType = "FÜS"
			break
		}
	}

	// 3. Determine subjectArea & specialization
	// Filter out the pure section name from subjectArea
	var meaningfulSegments []string
	for _, segment := range clean {
		if segment != studySection {
			meaningfulSegments = append(meaningfulSegments, segment)
		}
	}

	if len(meaningfulSegments) == 1 {
		subjectArea = meaningfulSegments[0]
	} else if len(meaningfulSegments) == 2 {
		subjectArea = meaningfulSegments[0]
		specialization = meaningfulSegments[1]
	} else if len(meaningfulSegments) > 2 {
		subjectArea = meaningfulSegments[0] + " / " + meaningfulSegments[1]
		specialization = meaningfulSegments[len(meaningfulSegments)-1]
	}

	if subjectArea == "" && studySection != "" {
		subjectArea = studySection
	}

	return studySection, subjectArea, moduleType, specialization
}

// POContext is what a PO page (or any page below it) says about its own position
// in the tree. The breadcrumb repeats the ancestors, so a PO page can be understood
// without the program and degree index pages above it.
type POContext struct {
	ProgramName string
	Degree      string
	POVersion   string
}

// ParsePOContext reads the breadcrumb („Krümelpfad") of a tree page.
func (p *ProgramTreeParser) ParsePOContext(r io.Reader) (POContext, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return POContext{}, fmt.Errorf("failed to parse HTML: %w", err)
	}

	var ctx POContext
	for _, entry := range FindAllByClass(doc, "KruemelpfadEintrag") {
		text := CleanSingleLine(NodeText(entry))
		switch {
		case strings.HasPrefix(text, "Studiengang:"):
			ctx.ProgramName = strings.TrimSpace(strings.TrimPrefix(text, "Studiengang:"))
		case strings.HasPrefix(text, "Module für Abschluss:"):
			ctx.Degree = strings.TrimSpace(strings.TrimPrefix(text, "Module für Abschluss:"))
		case strings.HasPrefix(text, "PO-Version:"):
			ctx.POVersion = strings.TrimSpace(strings.TrimPrefix(text, "PO-Version:"))
		}
	}
	return ctx, nil
}
