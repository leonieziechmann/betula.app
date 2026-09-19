package catalogbuild

import (
	"bytes"
	"context"
	"fmt"
	"sort"
	"time"

	"github.com/jakob/btu-scraper/internal/catalogdb"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/parser"
	"github.com/jakob/btu-scraper/internal/qistree"
)

// sources is the parsed content of the raw archive.
type sources struct {
	catalogTitles map[string]string           // module ID → title from the catalog list
	fues          map[string]model.FUESModule // modules on the FÜS list
	fuesFetched   bool                        // false: no FÜS list archived, flag unknown → treated as empty
	modulePages   map[string]*modulePage      // module ID → parsed module page
	moduleGone    map[string]bool             // module ID → page answered 404
	poTrees       []*poTree
	events        map[string]*eventPage
}

type modulePage struct {
	detail    *model.ModuleDetail
	english   bool
	url       string
	fetchedAt time.Time
}

// poTree is one PO version with everything below it.
type poTree struct {
	node      qistree.NodeID
	context   parser.POContext
	documents []model.ProgramRegulationDocument
	url       string
	fetchedAt time.Time
	pages     []qistree.Page // PO page first, then its area pages top-down
	missing   int
}

type eventPage struct {
	detail    *model.EventDetail
	url       string
	fetchedAt time.Time
}

func loadSources(ctx context.Context, db *catalogdb.DB, report *Report) (*sources, error) {
	src := &sources{
		catalogTitles: make(map[string]string),
		fues:          make(map[string]model.FUESModule),
		modulePages:   make(map[string]*modulePage),
		moduleGone:    make(map[string]bool),
		events:        make(map[string]*eventPage),
	}

	if list, err := db.GetPage(catalogdb.SourceModuleCatalog, "list"); err == nil {
		summaries, err := parser.NewCatalogParser("").Parse(bytes.NewReader(list.Body))
		if err != nil {
			return nil, fmt.Errorf("catalog list: %w", err)
		}
		for _, s := range summaries {
			src.catalogTitles[s.ID] = s.Title
		}
	} else if err != catalogdb.ErrNotFound {
		return nil, err
	}

	if list, err := db.GetPage(catalogdb.SourceQISFUESList, "list"); err == nil {
		modules, err := parser.NewFUESParser().Parse(bytes.NewReader(list.Body))
		if err != nil {
			return nil, fmt.Errorf("FÜS list: %w", err)
		}
		src.fuesFetched = true
		for _, m := range modules {
			src.fues[m.ID] = m
		}
	} else if err != catalogdb.ErrNotFound {
		return nil, err
	}

	detailParser := parser.NewDetailParser()
	err := db.EachPage(catalogdb.SourceModulePage, func(p *catalogdb.RawPage) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			src.moduleGone[p.Key] = true
			return nil
		}
		d, err := detailParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("module page %s: %w", p.Key, err)
		}
		d.ID = p.Key // the archive key is the identity; the page cannot rename a module
		src.modulePages[p.Key] = &modulePage{detail: d, english: isEnglishModulePage(p.Body), url: p.URL, fetchedAt: p.FetchedAt}
		return nil
	})
	if err != nil {
		return nil, err
	}

	if err := loadTrees(ctx, db, src); err != nil {
		return nil, err
	}
	for _, t := range src.poTrees {
		report.MissingTreePages += t.missing
	}

	eventParser := parser.NewEventParser()
	err = db.EachPage(catalogdb.SourceQISEvent, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		d, err := eventParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("event page %s: %w", p.Key, err)
		}
		src.events[p.Key] = &eventPage{detail: d, url: p.URL, fetchedAt: p.FetchedAt}
		return nil
	})
	if err != nil {
		return nil, err
	}

	return src, nil
}

// isEnglishModulePage: module pages are served in the module's teaching language,
// with different labels and with the two title rows swapped.
func isEnglishModulePage(body []byte) bool {
	return bytes.Contains(body, []byte("Module Number")) && !bytes.Contains(body, []byte("Modulnummer"))
}

// loadTrees finds every archived PO page and walks the archive below it. A PO page
// names its own program and degree in the breadcrumb, so the index pages above it
// are not needed.
func loadTrees(ctx context.Context, db *catalogdb.DB, src *sources) error {
	bodies := make(map[string]*catalogdb.RawPage)
	var poURLs []string
	err := db.EachPage(catalogdb.SourceQISTree, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		bodies[p.Key] = p
		if qistree.ParseNodeID(p.Key).IsPO() {
			poURLs = append(poURLs, p.Key)
		}
		return nil
	})
	if err != nil {
		return err
	}
	sort.Strings(poURLs)

	fetch := func(_ context.Context, pageURL string) ([]byte, error) {
		if p, ok := bodies[pageURL]; ok {
			return p.Body, nil
		}
		return nil, qistree.ErrPageMissing
	}

	treeParser := parser.NewProgramTreeParser()
	for _, poURL := range poURLs {
		page := bodies[poURL]
		t := &poTree{node: qistree.ParseNodeID(poURL), url: poURL, fetchedAt: page.FetchedAt}
		if t.context, err = treeParser.ParsePOContext(bytes.NewReader(page.Body)); err != nil {
			return fmt.Errorf("PO page %s: %w", poURL, err)
		}
		if t.documents, err = treeParser.ParsePODocuments(bytes.NewReader(page.Body), "https://www.b-tu.de"); err != nil {
			return fmt.Errorf("PO page %s: %w", poURL, err)
		}
		result, err := qistree.WalkPO(ctx, qistree.Page{URL: poURL}, fetch, func(p qistree.Page) error {
			p.Body = nil // parsed already; keep memory small
			t.pages = append(t.pages, p)
			return nil
		})
		if err != nil {
			return err
		}
		t.missing = len(result.Missing)
		src.poTrees = append(src.poTrees, t)
	}
	return nil
}
