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
		{SourceCell{Row: "Master’s Thesis"}, "Abschlussarbeit"},
		{SourceCell{Row: "Doctoral Thesis – Dissertation"}, "Abschlussarbeit"},
		// A course about a thesis is not one.
		{SourceCell{Row: "PhD Thesis Writing Skills"}, "Pflicht"},
		{SourceCell{Row: "Status Seminar ERM: Progress Reports PhD Thesis"}, "Pflicht"},
		{SourceCell{Row: "Thesis-Entwicklung"}, "Pflicht"},
		{SourceCell{Row: "Ingenieurpraktikum"}, "Praktikum"},
		{SourceCell{Row: "Bachelor-Praktikum"}, "Praktikum"},
		{SourceCell{Row: "Berufspraktikum"}, "Praktikum"},
		{SourceCell{Row: "Berufsfeldpraktikum VI*"}, "Praktikum"},
		{SourceCell{Row: "Betriebspraktikum"}, "Praktikum"},
		{SourceCell{Row: "PRA Pflichtpraktikum"}, "Praktikum"},
		{SourceCell{Row: "Industriefachpraktikum"}, "Praktikum"},
		{SourceCell{Row: "25 Außeruniversitäres Praktikum"}, "Praktikum"},
		{SourceCell{Row: "Industrial Internship (siehe § 32 Abs. 5)"}, "Praktikum"},
		{SourceCell{Row: "Internship"}, "Praktikum"},
		{SourceCell{Row: "Praxismodul 1"}, "Praktikum"},
		{SourceCell{Row: "Praktikum / Praxisphase"}, "Praktikum"},
		{SourceCell{Row: "Praktikum"}, "Praktikum"},
		{SourceCell{Row: "Praktikum*"}, "Praktikum"},
		// Their regulations make these internships: 800 hours in an
		// administration or a company, 18 weeks at a research institution, a
		// placement in a company, the dual program's practice at its music school.
		{SourceCell{Row: "Integrationspraktikum**"}, "Praktikum"},
		{SourceCell{Row: "Forschungspraktikum"}, "Praktikum"},
		{SourceCell{Row: "14257 Wirtschaftspraktikum Wirtschaftsingenieurwesen"}, "Praktikum"},
		{SourceCell{Row: "13786 Praktikum Wirtschaftsingenieurwesen"}, "Praktikum"},
		{SourceCell{Row: "Praktikum Maschinenbau"}, "Praktikum"},
		{SourceCell{Row: "14627 Praxis Musikschule (Praktikum Dual) I"}, "Praktikum"},
		// A Lehramt module with a school practicum in it is no internship.
		{SourceCell{Row: "Bildungswissenschaften I (beinhaltet Integriertes Eingangspraktikum, iEP)"}, "Pflicht"},
		{SourceCell{Row: "Fachdidaktik Mathematik (beinhaltet fachdidaktisches Tagespraktikum, fTP)"}, "Pflicht"},
		{SourceCell{Row: "Bildungswissenschaften III (beinhaltet Praktikum in pädagogisch-psychologischen Handlungsfeldern, PpH)"}, "Pflicht"},
		// Nor is a lab course.
		{SourceCell{Row: "Programmierpraktikum"}, "Pflicht"},
		{SourceCell{Row: "Programmierpraktikum für Ingenieure"}, "Pflicht"},
		{SourceCell{Row: "Softwarepraktikum"}, "Pflicht"},
		{SourceCell{Row: "Laborpraktikum der Elektrotechnik"}, "Pflicht"},
		{SourceCell{Row: "Digitaltechnik-Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Prozess- und Fertigungsmesstechnik mit Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Werkstofftechnik 2 mit Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Konstruktionslehre 1 - Technische Darstellung/CAD mit Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Physikalisches Praktikum I"}, "Pflicht"},
		{SourceCell{Row: "Elektronikpraktikum"}, "Pflicht"},
		{SourceCell{Row: "Fortgeschrittenenpraktikum 1"}, "Pflicht"},
		{SourceCell{Row: "Organische Chemie Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Mikrobiologie Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Biochemie Praktikum"}, "Pflicht"},
		{SourceCell{Row: "Praktikum Maschinelles Lernen"}, "Pflicht"},
		{SourceCell{Row: "Modul des FÜS"}, "FÜS"},
		{SourceCell{Row: "Modul zum Fachübergreifenden Stu- / dium"}, "FÜS"},
		{SourceCell{Row: "Fächerübergreifendes Studium (gemäß BTU-FÜSModulangebot)"}, "FÜS"},
		// A module that crosses subjects is not the FÜS.
		{SourceCell{Row: "Fachübergreifende Projektarbeit"}, "Pflicht"},
		{SourceCell{Row: "Wahlbereich", Elective: true}, "Wahlpflicht"},
		{SourceCell{Row: "Alternative A", AltGroup: 1}, "Wahlpflicht"},
		{SourceCell{Row: "Mathematik 1"}, "Pflicht"},
	} {
		if got := ClassifyRequirement(tc.cell); got != tc.want {
			t.Errorf("ClassifyRequirement(%q) = %q, want %q", tc.cell.Row, got, tc.want)
		}
	}
}
