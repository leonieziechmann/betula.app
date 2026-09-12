package parser

import (
	"strings"
	"testing"
)

func TestFUESParser(t *testing.T) {
	htmlSnippet := `
	<!DOCTYPE html>
	<html>
	<body>
		<table summary="Suchergebnis">
			<tr>
				<th>Nr.</th>
				<th>Modultitel</th>
				<th>Lehr- und Prüfungssprache</th>
				<th>Leistungspunkte</th>
				<th>FÜS-Modul</th>
				<th>Teilnehmerbeschränkung</th>
			</tr>
			<tr>
				<td>11101</td>
				<td><a href="https://qis.b-tu.de/modul11101">Lineare Algebra und analytische Geometrie I</a></td>
				<td>Deutsch</td>
				<td>8</td>
				<td>ja</td>
				<td>keine</td>
			</tr>
			<tr>
				<td>11152</td>
				<td><a href="https://qis.b-tu.de/modul11152">ERP - Integrierte betriebliche Systeme</a></td>
				<td>Deutsch</td>
				<td>6</td>
				<td>ja</td>
				<td></td>
			</tr>
		</table>
	</body>
	</html>
	`

	p := NewFUESParser()
	modules, err := p.Parse(strings.NewReader(htmlSnippet))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(modules) != 2 {
		t.Fatalf("expected 2 modules, got %d", len(modules))
	}

	if modules[0].ID != "11101" {
		t.Errorf("expected ID 11101, got %s", modules[0].ID)
	}
	if modules[0].Title != "Lineare Algebra und analytische Geometrie I" {
		t.Errorf("expected title, got %s", modules[0].Title)
	}
	if modules[0].Credits != 8.0 {
		t.Errorf("expected credits 8.0, got %f", modules[0].Credits)
	}
	if !modules[0].IsFUES {
		t.Errorf("expected IsFUES true")
	}

	if modules[1].ID != "11152" {
		t.Errorf("expected ID 11152, got %s", modules[1].ID)
	}
	if modules[1].Credits != 6.0 {
		t.Errorf("expected credits 6.0, got %f", modules[1].Credits)
	}
}
