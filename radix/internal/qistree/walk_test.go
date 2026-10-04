package qistree

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"
)

const nodeBase = "https://www.b-tu.de/qisserver3/rds?state=modulBeschrGast&nodeID="

func treeList(links ...string) []byte {
	var b strings.Builder
	b.WriteString(`<html><body><a class="breadCrumb" href="/qisserver3/rds?nodeID=crumb">Oberste Ebene</a><ul class="treelist">`)
	for i := 0; i+1 < len(links); i += 2 {
		fmt.Fprintf(&b, `<li><a href="%s">%s</a></li>`, links[i], links[i+1])
	}
	b.WriteString(`</ul></body></html>`)
	return []byte(b.String())
}

func TestWalkVisitsAreasAndReportsMissingPages(t *testing.T) {
	root := nodeBase + "auswahlBaum"
	prog := nodeBase + "auswahlBaum|stg=079"
	deg := prog + "|abschl=82"
	po := deg + "|pversion=2008"
	fach := po + "|area=fach"
	praktische := fach + "|area=prakt"
	grund := po + "|area=grund"

	pages := map[string][]byte{
		root: treeList(prog, "Studiengang: Informatik"),
		prog: treeList(deg, "Module für Abschluss: Bachelor (universitär)"),
		deg:  treeList(po, "PO-Version: 2008 - 2. SÄ 2024"),
		po: treeList(
			deg, "Module für Abschluss: Bachelor (universitär)", // link back up, must not be followed
			fach, "Fachstudium",
			grund, "Grundstudium"),
		fach:       treeList(praktische, "Praktische Informatik"),
		praktische: treeList(praktische+"|pruefung:1", "11861 Operating Systems II"),
		// grund is missing on purpose
	}
	fetch := func(_ context.Context, u string) ([]byte, error) {
		if body, ok := pages[u]; ok {
			return body, nil
		}
		return nil, ErrPageMissing
	}

	var visited []string
	var leafPath []string
	var leafModule string
	result, err := Walk(context.Background(), root, fetch, func(p Page) error {
		visited = append(visited, p.URL)
		for _, n := range p.Nodes {
			if n.IsModule {
				leafPath, leafModule = p.Path, n.ModuleID
				if p.Program.Name != "Informatik" || p.Program.Code != "079" || p.Degree.Code != "82" || p.PO.POVersion != "2008 - 2. SÄ 2024" {
					t.Errorf("leaf page lost its ancestors: %+v", p)
				}
			}
		}
		return nil
	})
	if err != nil {
		t.Fatalf("Walk failed: %v", err)
	}

	if want := []string{root, prog, deg, po, fach, praktische}; !reflect.DeepEqual(visited, want) {
		t.Errorf("visited = %v\nwant %v", visited, want)
	}
	if !reflect.DeepEqual(leafPath, []string{"Fachstudium", "Praktische Informatik"}) || leafModule != "11861" {
		t.Errorf("leaf path = %v, module = %q", leafPath, leafModule)
	}
	if result.Pages != 6 || !reflect.DeepEqual(result.Missing, []string{grund}) {
		t.Errorf("result = %+v", result)
	}
}
