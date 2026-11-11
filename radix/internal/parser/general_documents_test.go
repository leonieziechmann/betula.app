package parser

import (
	"os"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/model"
)

const (
	grundsatzDE = "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen"
	grundsatzEN = "https://www.b-tu.de/en/study/during-studies/study-organization/examinations/regulations-and-forms/basic-regulations"
)

func readGeneralDocuments(t *testing.T, file, pageURL string) []model.GeneralDocument {
	t.Helper()
	body, err := os.ReadFile("testdata/" + file)
	if err != nil {
		t.Fatal(err)
	}
	docs, err := ParseGeneralDocuments(body, pageURL)
	if err != nil {
		t.Fatal(err)
	}
	return docs
}

// The BTU's page of Grundsatzordnungen as it was on 2026-11-10: 13 sections, 50 links of
// documents (Soziale Arbeit's selection statute twice, once for each degree), none of the menus,
// of the foot or the folds' own links; the amendments in their fold.
func TestParseGeneralDocumentsGerman(t *testing.T) {
	docs := readGeneralDocuments(t, "btu_grundsatzordnungen_de.html", grundsatzDE)
	if len(docs) != 50 {
		t.Fatalf("%d documents, want 50", len(docs))
	}
	sections := map[string]int{}
	var order []string
	for _, d := range docs {
		if sections[d.Section] == 0 {
			order = append(order, d.Section)
		}
		sections[d.Section]++
		if !strings.HasPrefix(d.URL, "http") || strings.Contains(d.URL, "#") || strings.Contains(d.Title, "(PDF)") || d.Title == "" {
			t.Errorf("%+v", d)
		}
	}
	if len(order) != 13 || order[0] != "Landesrechtliche Grundlagen" || order[12] != "Sonstige Ordnungen und Richtlinien" {
		t.Errorf("sections %q", order)
	}

	var bachelor []model.GeneralDocument
	for _, d := range docs {
		if d.Section == "Rahmenordnung für Bachelor-Studiengänge" {
			bachelor = append(bachelor, d)
		}
	}
	if len(bachelor) != 7 {
		t.Fatalf("the Rahmenordnung for Bachelor-Studiengänge has %d documents, want 7: %+v", len(bachelor), bachelor)
	}
	want := model.GeneralDocument{Section: "Rahmenordnung für Bachelor-Studiengänge", Title: "Allgemeine Prüfungs- und Studienordnung für Bachelor-Studiengänge vom 12. September 2016",
		URL: "https://opus4.kobv.de/opus4-btu/files/3983/AMbl-13_2016-RahmenO-BA.pdf"}
	if bachelor[0] != want {
		t.Errorf("the statute: %+v", bachelor[0])
	}
	for _, d := range bachelor[1:] {
		if d.Fold != "Änderungssatzungen" {
			t.Errorf("an amendment outside the fold: %+v", d)
		}
	}
	if last := bachelor[6]; last.URL != "https://opus4.kobv.de/opus4-btu/files/7446/AMbl-35_2026.pdf" ||
		last.Title != "Sechste Änderungssatzung zur Allgemeinen Prüfungs- und Studienordnung für Bachelor-Studiengänge (RahmenO-BA) vom 29. Juli 2026" {
		t.Errorf("the sixth amendment: %+v", last)
	}

	// The words after a link belong to its title; the law is a page, not a PDF; a link with an
	// escaped path keeps it.
	byURL := map[string]model.GeneralDocument{}
	for _, d := range docs {
		byURL[d.URL] = d
	}
	if d := byURL["https://www-docs.b-tu.de/studierende/public/files/18-Satzung%20Fl%C3%BCchtlinge_27.09.16.pdf"]; d.Title != "Satzung über den Hochschulzugang bei fluchtbedingter Nichterbringung der Hochschulzugangsberechtigung vom 27. September 2016" {
		t.Errorf("the words after the link: %+v", d)
	}
	if d := byURL["https://bravors.brandenburg.de/gesetze/bbghg"]; d.Section != "Landesrechtliche Grundlagen" || d.Title != "Brandenburgisches Hochschulgesetz vom 23. September 2020" || d.Fold != "" {
		t.Errorf("the law: %+v", d)
	}
	if d := byURL["http://opus.kobv.de/btu/volltexte/2010/1748/pdf/04_FAS_Rili_230410.pdf"]; d.Section != "Sonstige Ordnungen und Richtlinien" || d.Title != "Ordnung zum Fachübergreifenden Studium" {
		t.Errorf("the FÜS: %+v", d)
	}
	if d := byURL["https://opus4.kobv.de/opus4-btu/files/6002/AMbl-10_2022_ImmaO_Aussetzung_Paragraph_16.pdf"]; d.Section != "Immatrikulationsordnung" || d.Fold != "Änderungssatzungen" {
		t.Errorf("the suspension of § 16: %+v", d)
	}
}

// The English page is built as the German one: the same sections in the same order, the BTU's
// English translations where the German page has the regulation, the German amendments.
func TestParseGeneralDocumentsEnglish(t *testing.T) {
	docs := readGeneralDocuments(t, "btu_grundsatzordnungen_en.html", grundsatzEN)
	if len(docs) != 50 {
		t.Fatalf("%d documents, want 50", len(docs))
	}
	var bachelor []model.GeneralDocument
	for _, d := range docs {
		if d.Section == "Framework regulations for Bachelor´s degree programs" {
			bachelor = append(bachelor, d)
		}
	}
	if len(bachelor) != 7 || bachelor[0].URL != "https://www-docs.b-tu.de/studierende/public/rahmenordnungen/bachelor-en/29-RahmnO%20BA_5-Aenderungssatzung_EN.pdf" ||
		bachelor[0].Fold != "" || bachelor[1].Fold != "Amendment statutes" || bachelor[1].URL != "https://opus4.kobv.de/opus4-btu/files/5396/AMbl-1_2021_RahmenO_BA_1.AeS.pdf" {
		t.Errorf("the Rahmenordnung for Bachelor's programmes: %+v", bachelor)
	}
}

// Without the CMS's marks the page's head, menus and foot are still not documents; relative
// links are resolved, anchors and mail addresses are not documents, a link before the first
// heading belongs to no section.
func TestParseGeneralDocumentsWithoutMarks(t *testing.T) {
	page := `<html><body><header><nav><a href="/kontakt">Kontakt</a></nav></header>
		<h1>Allgemeine Satzungen</h1><p><a href="/intro.pdf">Einleitung</a></p>
		<div class="frame"><header><h2> Rahmenordnung für Master-Studiengänge </h2></header>
		<ul><li><a href="/files/AMbl-14_2016-RahmenO-MA.pdf" title="t">Allgemeine Prüfungs- und Studienordnung </a>vom 12. September 2016 (PDF)</li>
		<li><a href="#c1">Sprung</a> <a href="mailto:x@example.org">Post</a></li></ul>
		<div class="accordion-item"><a href="#c2" class="accordion-title">Änderungssatzungen</a><div class="accordion-content" id="c2">
		<ul><li><a href="https://opus4.kobv.de/opus4-btu/files/5397/AMbl-2_2021.pdf"></a>Erste Änderungssatzung (PDF)</li></ul></div></div></div>
		<footer><a href="/impressum">Impressum</a></footer></body></html>`
	docs, err := ParseGeneralDocuments([]byte(page), grundsatzDE)
	if err != nil {
		t.Fatal(err)
	}
	want := []model.GeneralDocument{
		{Section: "Rahmenordnung für Master-Studiengänge", Title: "Allgemeine Prüfungs- und Studienordnung vom 12. September 2016", URL: "https://www.b-tu.de/files/AMbl-14_2016-RahmenO-MA.pdf"},
		{Section: "Rahmenordnung für Master-Studiengänge", Fold: "Änderungssatzungen", Title: "Erste Änderungssatzung", URL: "https://opus4.kobv.de/opus4-btu/files/5397/AMbl-2_2021.pdf"},
	}
	if len(docs) != len(want) {
		t.Fatalf("%+v", docs)
	}
	for i := range want {
		if docs[i] != want[i] {
			t.Errorf("%d: %+v, want %+v", i, docs[i], want[i])
		}
	}
}
