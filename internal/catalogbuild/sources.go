package catalogbuild

import (
	"bytes"
	"context"
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/model"
	"github.com/leonieziechmann/betula/internal/parser"
	"github.com/leonieziechmann/betula/internal/qistree"
)

// sources is the parsed content of the raw archive.
type sources struct {
	catalogTitles map[string]string           // module ID → title from the catalog list
	qisTitles     map[string]string           // module ID → title from the QIS module table
	fues          map[string]model.FUESModule // modules on the FÜS list
	fuesFetched   bool                        // false: no FÜS list archived, flag unknown → treated as empty
	modulePages   map[string]*modulePage      // module ID → the description that wins
	moduleGone    map[string]bool             // module ID → page answered 404
	eventLinks    map[string][]model.ModuleEvent
	poTrees       []*poTree
	events        map[string]*eventPage
}

type modulePage struct {
	detail    *model.ModuleDetail
	english   bool
	fromQIS   bool   // read from the QIS description, not from the copy on b-tu.de
	url       string // the page the fields were read from, and what source_url states
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
}

type eventPage struct {
	detail    *model.EventDetail
	url       string
	fetchedAt time.Time
}

func loadSources(ctx context.Context, db *catalogdb.DB, report *Report) (*sources, error) {
	src := &sources{
		catalogTitles: make(map[string]string),
		qisTitles:     make(map[string]string),
		fues:          make(map[string]model.FUESModule),
		modulePages:   make(map[string]*modulePage),
		moduleGone:    make(map[string]bool),
		eventLinks:    make(map[string][]model.ModuleEvent),
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

	rowParser := parser.NewFUESParser()
	err := db.EachPage(catalogdb.SourceQISModuleList, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		rows, err := rowParser.Parse(bytes.NewReader(p.Body))
		if err != nil {
			return fmt.Errorf("QIS module table %s: %w", p.Key, err)
		}
		for _, r := range rows {
			src.qisTitles[r.ID] = r.Title
		}
		return nil
	})
	if err != nil {
		return nil, err
	}

	detailParser := parser.NewDetailParser()
	err = db.EachPage(catalogdb.SourceModulePage, func(p *catalogdb.RawPage) error {
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
		src.eventLinks[p.Key] = d.CurrentSemesterEvents
		return nil
	})
	if err != nil {
		return nil, err
	}

	// QIS is where the catalog is maintained; b-tu.de/modul renders a copy that can
	// be a semester behind. Where QIS has the description, it replaces the copy.
	// Its event links are added to those of the copy instead of replacing them: the
	// copy still names the exams of the semester that is ending, and an event carries
	// the semester it belongs to, so both can be shown until retention removes them.
	qisParser := parser.NewQISModuleParser()
	err = db.EachPage(catalogdb.SourceQISModulePage, func(p *catalogdb.RawPage) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		d, err := qisParser.Parse(bytes.NewReader(p.Body), p.Key, p.URL)
		if err != nil {
			return fmt.Errorf("QIS module page %s: %w", p.Key, err)
		}
		d.ID = p.Key // the archive key is the identity; the page cannot rename a module
		delete(src.moduleGone, p.Key)
		// The description is fetched in the language the module is taught in, so the
		// English view marks an English-taught module exactly as the copy did.
		src.modulePages[p.Key] = &modulePage{detail: d, fromQIS: true, english: isEnglishModulePage(p.Body), url: p.URL, fetchedAt: p.FetchedAt}
		src.eventLinks[p.Key] = mergeEventLinks(d.CurrentSemesterEvents, src.eventLinks[p.Key])
		return nil
	})
	if err != nil {
		return nil, err
	}

	// The lists decide which modules exist. The page of a module that left every list
	// is not part of the current dataset, however recently it was archived. QIS lists
	// what it still maintains; b-tu.de also lists the modules that are no longer offered.
	if len(src.catalogTitles)+len(src.qisTitles)+len(src.fues) > 0 {
		listed := func(id string) bool {
			if _, ok := src.catalogTitles[id]; ok {
				return true
			}
			if _, ok := src.qisTitles[id]; ok {
				return true
			}
			_, ok := src.fues[id]
			return ok
		}
		unusedQIS := make(map[string]bool)
		for id, page := range src.modulePages {
			if listed(id) {
				continue
			}
			if page.fromQIS {
				unusedQIS[id] = true
			}
			delete(src.modulePages, id)
			delete(src.eventLinks, id)
			report.Unused[catalogdb.SourceModulePage] = append(report.Unused[catalogdb.SourceModulePage], id)
		}
		for id := range src.moduleGone {
			if !listed(id) {
				report.Unused[catalogdb.SourceModulePage] = append(report.Unused[catalogdb.SourceModulePage], id)
			}
		}
		for id := range unusedQIS {
			report.Unused[catalogdb.SourceQISModulePage] = append(report.Unused[catalogdb.SourceQISModulePage], id)
		}
		sort.Strings(report.Unused[catalogdb.SourceQISModulePage])
		sort.Strings(report.Unused[catalogdb.SourceModulePage])
	}

	if err := loadTrees(ctx, db, src, report); err != nil {
		return nil, err
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

// mergeEventLinks keeps the links of both descriptions of a module, the current
// ones first, without naming an event twice.
func mergeEventLinks(first, second []model.ModuleEvent) []model.ModuleEvent {
	merged := make([]model.ModuleEvent, 0, len(first)+len(second))
	seen := make(map[string]bool, len(first)+len(second))
	for _, list := range [][]model.ModuleEvent{first, second} {
		for _, link := range list {
			id := eventIDFromURL(link.URL)
			if id == "" || seen[id] {
				continue
			}
			seen[id] = true
			merged = append(merged, link)
		}
	}
	return merged
}

// isEnglishModulePage: module pages are served in the module's teaching language,
// with different labels and with the two title rows swapped.
func isEnglishModulePage(body []byte) bool {
	return bytes.Contains(body, []byte("Module Number")) && !bytes.Contains(body, []byte("Modulnummer"))
}

// loadTrees reads the program tree from the archive. With an archived root page the
// tree is what a walk from that root reaches: a program or PO version that QIS no
// longer lists is not part of the current dataset, and its pages are reported as
// unused. Without a root (a partial archive) every archived PO page counts; a PO page
// names its own program and degree in the breadcrumb.
func loadTrees(ctx context.Context, db *catalogdb.DB, src *sources, report *Report) error {
	bodies := make(map[string]*catalogdb.RawPage)
	var poURLs []string
	var root *catalogdb.RawPage
	err := db.EachPage(catalogdb.SourceQISTree, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 {
			return nil
		}
		bodies[p.Key] = p
		node := qistree.ParseNodeID(p.Key)
		if node.IsPO() {
			poURLs = append(poURLs, p.Key)
		}
		if node.Stg == "" && strings.Contains(p.Key, "nodeID=auswahlBaum") && (root == nil || p.FetchedAt.After(root.FetchedAt)) {
			root = p
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
	trees := make(map[string]*poTree)
	collect := func(p qistree.Page) error {
		if p.Level < qistree.LevelPO {
			return nil
		}
		poURL := p.PO.URL
		if p.Level == qistree.LevelPO {
			poURL = p.URL
		}
		t := trees[poURL]
		if t == nil {
			page := bodies[poURL]
			t = &poTree{node: qistree.ParseNodeID(poURL), url: poURL, fetchedAt: page.FetchedAt}
			var err error
			if t.context, err = treeParser.ParsePOContext(bytes.NewReader(page.Body)); err != nil {
				return fmt.Errorf("PO page %s: %w", poURL, err)
			}
			if t.documents, err = treeParser.ParsePODocuments(bytes.NewReader(page.Body), "https://www.b-tu.de"); err != nil {
				return fmt.Errorf("PO page %s: %w", poURL, err)
			}
			trees[poURL] = t
			src.poTrees = append(src.poTrees, t)
		}
		p.Body = nil // parsed already; keep memory small
		t.pages = append(t.pages, p)
		return nil
	}

	if root != nil {
		visited := make(map[string]bool)
		result, err := qistree.Walk(ctx, root.Key, fetch, func(p qistree.Page) error {
			visited[p.URL] = true
			return collect(p)
		})
		if err != nil {
			return err
		}
		report.MissingTreePages += len(result.Missing)
		for key := range bodies {
			if !visited[key] {
				report.Unused[catalogdb.SourceQISTree] = append(report.Unused[catalogdb.SourceQISTree], key)
			}
		}
		sort.Strings(report.Unused[catalogdb.SourceQISTree])
	} else {
		for _, poURL := range poURLs {
			result, err := qistree.WalkPO(ctx, qistree.Page{URL: poURL, PO: parser.PONode{URL: poURL}}, fetch, collect)
			if err != nil {
				return err
			}
			report.MissingTreePages += len(result.Missing)
		}
	}
	sort.Slice(src.poTrees, func(i, j int) bool { return src.poTrees[i].url < src.poTrees[j].url })
	return nil
}

// descriptionSource names the page the fields of a module were read from.
func (p *modulePage) descriptionSource() string {
	if p.fromQIS {
		return "qis"
	}
	return "btu_cms"
}
