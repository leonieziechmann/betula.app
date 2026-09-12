package parser

import (
	"strings"

	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// FindFirstByTag traverses the node tree depth-first and returns the first node with the specified tag atom.
func FindFirstByTag(n *html.Node, a atom.Atom) *html.Node {
	if n == nil {
		return nil
	}
	if n.Type == html.ElementNode && n.DataAtom == a {
		return n
	}
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		if res := FindFirstByTag(c, a); res != nil {
			return res
		}
	}
	return nil
}

// FindFirstByClass returns the first element node having the given class name in its class attribute.
func FindFirstByClass(n *html.Node, className string) *html.Node {
	if n == nil {
		return nil
	}
	if n.Type == html.ElementNode && HasClass(n, className) {
		return n
	}
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		if res := FindFirstByClass(c, className); res != nil {
			return res
		}
	}
	return nil
}

// FindAllByTag returns all descendant element nodes with the specified tag atom.
func FindAllByTag(n *html.Node, a atom.Atom) []*html.Node {
	var result []*html.Node
	var walk func(*html.Node)
	walk = func(curr *html.Node) {
		if curr == nil {
			return
		}
		if curr.Type == html.ElementNode && curr.DataAtom == a {
			result = append(result, curr)
		}
		for c := curr.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}
	walk(n)
	return result
}

// FindAllByClass returns all descendant element nodes with the given class.
func FindAllByClass(n *html.Node, className string) []*html.Node {
	var result []*html.Node
	var walk func(*html.Node)
	walk = func(curr *html.Node) {
		if curr == nil {
			return
		}
		if curr.Type == html.ElementNode && HasClass(curr, className) {
			result = append(result, curr)
		}
		for c := curr.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}
	walk(n)
	return result
}

// HasClass reports whether the element node has the specified class.
func HasClass(n *html.Node, className string) bool {
	for _, attr := range n.Attr {
		if attr.Key == "class" {
			fields := strings.Fields(attr.Val)
			for _, f := range fields {
				if f == className {
					return true
				}
			}
		}
	}
	return false
}

// GetAttr returns the value of the named attribute, or empty string if not found.
func GetAttr(n *html.Node, key string) string {
	if n == nil {
		return ""
	}
	for _, attr := range n.Attr {
		if attr.Key == key {
			return attr.Val
		}
	}
	return ""
}

// NodeText extracts and concatenates all text content from the node and its children.
func NodeText(n *html.Node) string {
	if n == nil {
		return ""
	}
	var sb strings.Builder
	var walk func(*html.Node)
	walk = func(curr *html.Node) {
		if curr == nil {
			return
		}
		if curr.Type == html.TextNode {
			sb.WriteString(curr.Data)
		} else if curr.Type == html.ElementNode {
			switch curr.DataAtom {
			case atom.Br:
				sb.WriteString("\n")
			case atom.Li:
				sb.WriteString("\n• ")
			case atom.P, atom.Div:
				sb.WriteString("\n")
			}
		}
		for c := curr.FirstChild; c != nil; c = c.NextSibling {
			walk(c)
		}
	}
	walk(n)
	return sb.String()
}

// CleanText extracts node text, converts non-breaking spaces, and trims leading/trailing whitespace.
func CleanText(n *html.Node) string {
	raw := NodeText(n)
	raw = strings.ReplaceAll(raw, "\u00a0", " ")
	lines := strings.Split(raw, "\n")
	var cleaned []string
	for _, l := range lines {
		trimmed := strings.TrimSpace(l)
		if trimmed != "" {
			cleaned = append(cleaned, trimmed)
		}
	}
	return strings.Join(cleaned, "\n")
}

// CleanSingleLine normalizes internal whitespace to single spaces.
func CleanSingleLine(s string) string {
	s = strings.ReplaceAll(s, "\u00a0", " ")
	return strings.Join(strings.Fields(s), " ")
}

// ExtractListItems extracts text from all <li> elements inside a node.
func ExtractListItems(n *html.Node) []string {
	var items []string
	lis := FindAllByTag(n, atom.Li)
	for _, li := range lis {
		txt := CleanSingleLine(NodeText(li))
		txt = strings.TrimPrefix(txt, "• ")
		if txt != "" {
			items = append(items, txt)
		}
	}
	return items
}
