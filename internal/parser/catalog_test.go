package parser

import (
	"strings"
	"testing"
)

func TestCatalogParser(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<div id="module-list-listjs" class="module-list">
			<table>
				<thead>
					<tr><th>Modulnummer</th><th>Bezeichnung</th></tr>
				</thead>
				<tbody class="list">
					<tr>
						<td class="moduleNumber"><a href="/modul/11101">11101</a></td>
						<td class="title">Lineare Algebra und analytische Geometrie I</td>
					</tr>
					<tr>
						<td class="moduleNumber"><a href="https://www.b-tu.de/modul/11102">11102</a></td>
						<td class="title">Lineare Algebra und analytische Geometrie II</td>
					</tr>
				</tbody>
			</table>
		</div>
	</body>
	</html>
	`

	p := NewCatalogParser("https://www.b-tu.de")
	summaries, err := p.Parse(strings.NewReader(htmlSnippet))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(summaries) != 2 {
		t.Fatalf("expected 2 summaries, got %d", len(summaries))
	}

	if summaries[0].ID != "11101" {
		t.Errorf("expected ID 11101, got %s", summaries[0].ID)
	}
	if summaries[0].Title != "Lineare Algebra und analytische Geometrie I" {
		t.Errorf("expected title Lineare Algebra und analytische Geometrie I, got %s", summaries[0].Title)
	}
	if summaries[0].URL != "https://www.b-tu.de/modul/11101" {
		t.Errorf("expected url https://www.b-tu.de/modul/11101, got %s", summaries[0].URL)
	}

	if summaries[1].ID != "11102" {
		t.Errorf("expected ID 11102, got %s", summaries[1].ID)
	}
	if summaries[1].URL != "https://www.b-tu.de/modul/11102" {
		t.Errorf("expected url https://www.b-tu.de/modul/11102, got %s", summaries[1].URL)
	}
}
