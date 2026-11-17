package fuescatalog

import (
	"bytes"
	"fmt"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"

	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// Link is a FÜS catalog a page links, and the semester it is for.
type Link struct {
	URL      string
	Semester string
}

// reFileSemester reads the semester of a catalog's file name: „FÜS_Katalog_WiSe_2026_27.pdf",
// „FÜS_Katalog_SoSe_2026.pdf".
var reFileSemester = regexp.MustCompile(`(?i)(WiSe|SoSe)_(\d{4})`)

// Links finds the FÜS catalogs a page links („FÜS-Modulkatalog zum Wintersemester 2026/2027"): a
// link to a PDF whose text or file name says FÜS and catalog, its semester from its text or else
// from its file name. base resolves relative links. A link whose semester neither says is left
// out.
func Links(page []byte, base string) []Link {
	doc, err := html.Parse(bytes.NewReader(page))
	if err != nil {
		return nil
	}
	baseURL, _ := url.Parse(base)
	var links []Link
	seen := map[string]bool{}
	var walk func(*html.Node)
	walk = func(n *html.Node) {
		if n.Type == html.ElementNode && n.DataAtom == atom.A {
			href := ""
			for _, a := range n.Attr {
				if a.Key == "href" {
					href = strings.TrimSpace(a.Val)
				}
			}
			text := strings.Join(strings.Fields(nodeText(n)), " ")
			file, _ := url.PathUnescape(href)
			lowerFile, lowerText := strings.ToLower(file), strings.ToLower(text)
			isPDF := strings.HasSuffix(strings.ToLower(strings.SplitN(href, "?", 2)[0]), ".pdf")
			namesFUES := strings.Contains(lowerText, "füs") || strings.Contains(lowerFile, "füs") || strings.Contains(lowerFile, "fues")
			namesCatalog := strings.Contains(lowerText, "katalog") || strings.Contains(lowerFile, "katalog")
			if href != "" && isPDF && namesFUES && namesCatalog {
				semester := SemesterOf(text)
				if semester == "" {
					semester = fileSemester(file)
				}
				target := href
				if u, err := url.Parse(href); err == nil && baseURL != nil {
					target = baseURL.ResolveReference(u).String()
				}
				if semester != "" && !seen[semester+" "+target] {
					seen[semester+" "+target] = true
					links = append(links, Link{URL: target, Semester: semester})
				}
			}
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}
	walk(doc)
	return links
}

func nodeText(n *html.Node) string {
	if n.Type == html.TextNode {
		return n.Data
	}
	var b strings.Builder
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		b.WriteString(nodeText(c))
		b.WriteByte(' ')
	}
	return b.String()
}

func fileSemester(file string) string {
	m := reFileSemester.FindStringSubmatch(file)
	if m == nil {
		return ""
	}
	if strings.EqualFold(m[1], "WiSe") {
		return m[2] + "W"
	}
	return m[2] + "S"
}

// CatalogURL is where the BTU puts the catalog of a semester, by the names of its files of 2026
// („F%C3%9CS_Katalog_WiSe_2026_27.pdf", „F%C3%9CS_Katalog_SoSe_2026.pdf") in the directory files.
// The page „Aktuelle Infos" links only the current one; this finds the one before it.
func CatalogURL(files, semester string) string {
	year, season, ok := split(semester)
	if !ok {
		return ""
	}
	if season == 'W' {
		return fmt.Sprintf("%sF%%C3%%9CS_Katalog_WiSe_%d_%02d.pdf", files, year, (year+1)%100)
	}
	return fmt.Sprintf("%sF%%C3%%9CS_Katalog_SoSe_%d.pdf", files, year)
}

// SemesterAt is the semester a day belongs to: April to September the summer, October to March
// the winter that began in October.
func SemesterAt(now time.Time) string {
	now = now.In(time.FixedZone("CET", 3600))
	switch m := now.Month(); {
	case m >= time.April && m <= time.September:
		return fmt.Sprintf("%dS", now.Year())
	case m >= time.October:
		return fmt.Sprintf("%dW", now.Year())
	default:
		return fmt.Sprintf("%dW", now.Year()-1)
	}
}

// Previous is the semester before: „2026W" → „2026S", „2026S" → „2025W".
func Previous(semester string) string {
	year, season, ok := split(semester)
	if !ok {
		return ""
	}
	if season == 'W' {
		return fmt.Sprintf("%dS", year)
	}
	return fmt.Sprintf("%dW", year-1)
}

// Next is the semester after: „2026W" → „2027S", „2026S" → „2026W".
func Next(semester string) string {
	year, season, ok := split(semester)
	if !ok {
		return ""
	}
	if season == 'W' {
		return fmt.Sprintf("%dS", year+1)
	}
	return fmt.Sprintf("%dW", year)
}

func split(semester string) (year int, season byte, ok bool) {
	if len(semester) != 5 || (semester[4] != 'S' && semester[4] != 'W') {
		return 0, 0, false
	}
	year, err := strconv.Atoi(semester[:4])
	if err != nil {
		return 0, 0, false
	}
	return year, semester[4], true
}
