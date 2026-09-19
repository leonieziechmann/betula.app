package gemini

import (
	"errors"
	"testing"
)

func TestIsLayoutError(t *testing.T) {
	for _, tc := range []struct {
		msg    string
		layout bool
	}{
		{"no supported ruled semester/ECTS table found", true},
		{"ambiguous PDF layout; manual review required: p1t1r2: x", true},
		{"incomplete table p9t1: no whole-plan total for semester 1", true},
		{"gemini api returned status 404: model not found", false},
		{"gemini api temporary error 429: quota", false},
		{"failed to parse structured curriculum JSON", false},
	} {
		if got := IsLayoutError(errors.New(tc.msg)); got != tc.layout {
			t.Errorf("IsLayoutError(%q) = %v, want %v", tc.msg, got, tc.layout)
		}
	}
	if IsLayoutError(nil) {
		t.Error("nil is not a layout error")
	}
}

func TestClassifyRequirementUsesSourceEvidence(t *testing.T) {
	for _, tc := range []struct {
		cell SourceCell
		want string
	}{
		{SourceCell{Row: "Bachelor-Arbeit"}, "Abschlussarbeit"},
		{SourceCell{Row: "Ingenieurpraktikum"}, "Praktikum"},
		{SourceCell{Row: "Modul des FÜS"}, "FÜS"},
		{SourceCell{Row: "Wahlbereich", Elective: true}, "Wahlpflicht"},
		{SourceCell{Row: "Alternative A", AltGroup: 1}, "Wahlpflicht"},
		{SourceCell{Row: "Mathematik 1"}, "Pflicht"},
	} {
		if got := ClassifyRequirement(tc.cell); got != tc.want {
			t.Errorf("ClassifyRequirement(%q) = %q, want %q", tc.cell.Row, got, tc.want)
		}
	}
}
