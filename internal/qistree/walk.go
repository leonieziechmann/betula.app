// Package qistree walks the QIS module-description tree
// (program → degree → PO version → area nodes → module leaves).
package qistree

import (
	"bytes"
	"context"
	"errors"
	"strings"

	"github.com/jakob/btu-scraper/internal/parser"
)

const baseURL = "https://www.b-tu.de"

// Level is the depth of a fetched tree page.
type Level int

const (
	LevelRoot    Level = iota // lists programs
	LevelProgram              // lists degrees
	LevelDegree               // lists PO versions
	LevelPO                   // lists documents and the first area nodes
	LevelArea                 // lists area nodes and module leaves
)

// Page is one fetched tree page with its position in the tree.
type Page struct {
	URL     string
	Level   Level
	Program parser.ProgramNode
	Degree  parser.DegreeNode
	PO      parser.PONode
	Path    []string             // area labels from the PO down to this page
	Nodes   []parser.QISTreeNode // children of a PO or area page
	Body    []byte
}

// ErrPageMissing is returned by a Fetch that cannot provide a page. The walk
// skips that subtree and reports the URL in Result.Missing.
var ErrPageMissing = errors.New("page missing")

// Fetch returns the body of a tree page.
type Fetch func(ctx context.Context, pageURL string) ([]byte, error)

// Result summarises a walk.
type Result struct {
	Pages   int
	Missing []string
}

// Walk visits every page of the tree below rootURL, top-down. The same walk
// serves a live crawl, a cache import and the offline build: only fetch differs.
func Walk(ctx context.Context, rootURL string, fetch Fetch, visit func(Page) error) (Result, error) {
	w := &walker{ctx: ctx, fetch: fetch, visit: visit, parser: parser.NewProgramTreeParser(), visited: make(map[string]bool)}

	body, ok, err := w.get(rootURL)
	if err != nil || !ok {
		return w.result, err
	}
	if err := w.emit(Page{URL: rootURL, Level: LevelRoot, Body: body}); err != nil {
		return w.result, err
	}
	programs, err := w.parser.ParseRootPrograms(bytes.NewReader(body), baseURL)
	if err != nil {
		return w.result, err
	}

	for _, prog := range programs {
		body, ok, err := w.get(prog.URL)
		if err != nil {
			return w.result, err
		}
		if !ok {
			continue
		}
		if err := w.emit(Page{URL: prog.URL, Level: LevelProgram, Program: prog, Body: body}); err != nil {
			return w.result, err
		}
		degrees, err := w.parser.ParseDegrees(bytes.NewReader(body), baseURL)
		if err != nil {
			return w.result, err
		}

		for _, deg := range degrees {
			body, ok, err := w.get(deg.URL)
			if err != nil {
				return w.result, err
			}
			if !ok {
				continue
			}
			if err := w.emit(Page{URL: deg.URL, Level: LevelDegree, Program: prog, Degree: deg, Body: body}); err != nil {
				return w.result, err
			}
			versions, err := w.parser.ParsePOVersions(bytes.NewReader(body), baseURL)
			if err != nil {
				return w.result, err
			}

			for _, po := range versions {
				page := Page{URL: po.URL, Level: LevelPO, Program: prog, Degree: deg, PO: po}
				if err := w.walkBranch(page); err != nil {
					return w.result, err
				}
			}
		}
	}
	return w.result, nil
}

// WalkPO visits the PO page at page.URL and everything below it. page carries
// the program, degree and PO the caller already knows.
func WalkPO(ctx context.Context, page Page, fetch Fetch, visit func(Page) error) (Result, error) {
	w := &walker{ctx: ctx, fetch: fetch, visit: visit, parser: parser.NewProgramTreeParser(), visited: make(map[string]bool)}
	page.Level = LevelPO
	page.Path = nil
	err := w.walkBranch(page)
	return w.result, err
}

type walker struct {
	ctx     context.Context
	fetch   Fetch
	visit   func(Page) error
	parser  *parser.ProgramTreeParser
	visited map[string]bool
	result  Result
}

func (w *walker) get(pageURL string) ([]byte, bool, error) {
	if err := w.ctx.Err(); err != nil {
		return nil, false, err
	}
	body, err := w.fetch(w.ctx, pageURL)
	if errors.Is(err, ErrPageMissing) {
		w.result.Missing = append(w.result.Missing, pageURL)
		return nil, false, nil
	}
	if err != nil {
		return nil, false, err
	}
	return body, true, nil
}

func (w *walker) emit(p Page) error {
	w.result.Pages++
	return w.visit(p)
}

// walkBranch fetches a PO or area page and descends into its area nodes.
func (w *walker) walkBranch(page Page) error {
	if w.visited[page.URL] {
		return nil
	}
	w.visited[page.URL] = true

	body, ok, err := w.get(page.URL)
	if err != nil || !ok {
		return err
	}
	page.Body = body
	if page.Nodes, err = w.parser.ParsePOBranchNodes(bytes.NewReader(body), baseURL); err != nil {
		return err
	}
	if err := w.emit(page); err != nil {
		return err
	}

	for _, node := range page.Nodes {
		if node.IsModule || isAncestorLabel(node.Text) || contains(page.Path, node.Text) {
			continue
		}
		child := page
		child.URL = node.URL
		child.Level = LevelArea
		child.Path = append(append([]string{}, page.Path...), node.Text)
		child.Nodes = nil
		if err := w.walkBranch(child); err != nil {
			return err
		}
	}
	return nil
}

// isAncestorLabel matches the breadcrumb-like links back up the tree that QIS
// repeats inside the tree list.
func isAncestorLabel(text string) bool {
	return strings.HasPrefix(text, "PO-Version") ||
		strings.HasPrefix(text, "Studiengang") ||
		strings.HasPrefix(text, "Module für Abschluss")
}

func contains(slice []string, val string) bool {
	for _, s := range slice {
		if s == val {
			return true
		}
	}
	return false
}
