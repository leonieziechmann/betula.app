package parser

import (
	"os"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/model"
)

const (
	studyOrganisationDE = "https://www.b-tu.de/studium/im-studium/studienorganisation"
	studyOrganisationEN = "https://www.b-tu.de/en/study/during-studies/study-organization"
)

func readStudyPages(t *testing.T, file, pageURL string) []model.StudyPage {
	t.Helper()
	body, err := os.ReadFile("testdata/" + file)
	if err != nil {
		t.Fatal(err)
	}
	pages, err := ParseStudyPages(body, pageURL)
	if err != nil {
		t.Fatal(err)
	}
	return pages
}

// groups is the headings of pages in their order, each with how many pages it has.
func groups(pages []model.StudyPage) string {
	var out []string
	count := map[string]int{}
	for _, p := range pages {
		if count[p.Section] == 0 {
			out = append(out, p.Section)
		}
		count[p.Section]++
	}
	for i, s := range out {
		out[i] = s + " " + string(rune('0'+count[s]))
	}
	return strings.Join(out, ", ")
}

// The BTU's page „Studienorganisation" as it was on 2026-11-11: six groups of tiles, 31 pages, each
// with its title, its address and the sentence under it; not the site's menus, not the contacts at
// the page's end (no menu of teasers), not the pictures' links twice.
func TestParseStudyPagesGerman(t *testing.T) {
	pages := readStudyPages(t, "btu_studienorganisation_de.html", studyOrganisationDE)
	if got := groups(pages); got != "Quicklinks 5, Prüfungen 6, Formalia 8, Erfolgreich studieren 6, Besondere Lebenslagen 3, Gesundheit & Versicherung 3" {
		t.Errorf("groups %s", got)
	}
	for _, p := range pages {
		if !strings.HasPrefix(p.URL, "https://") || p.Title == "" || p.Abstract == "" {
			t.Errorf("%+v", p)
		}
	}
	want := map[string]model.StudyPage{
		"Termine & Fristen": {Section: "Quicklinks", Title: "Termine & Fristen", URL: studyOrganisationDE + "/quicklinks/termine-fristen",
			Abstract: "Vorlesungszeiten, Bewerbungsfristen, Rückmeldefristen und Akademische Ferien."},
		"Rückmeldung": {Section: "Formalia", Title: "Rückmeldung", URL: studyOrganisationDE + "/formalia/rueckmeldung"},
		// A page elsewhere on the BTU's site, and one of the student body's.
		"Semesterticket": {Section: "Formalia", Title: "Semesterticket", URL: "https://www.stura-btu.de/themen/semesterticket"},
		// A tile that points into a page keeps its fragment.
		"Weg zum Master-Studium": {Section: "Erfolgreich studieren", Title: "Weg zum Master-Studium",
			URL: "https://www.b-tu.de/studium/vor-dem-studium/beratung-hilfe/studien-berufswahl/fachstudienberatung#c403684"},
	}
	for _, p := range pages {
		w, ok := want[p.Title]
		if !ok {
			continue
		}
		delete(want, p.Title)
		if p.Section != w.Section || p.URL != w.URL || (w.Abstract != "" && p.Abstract != w.Abstract) {
			t.Errorf("%+v, want %+v", p, w)
		}
	}
	if len(want) > 0 {
		t.Errorf("not read: %v", want)
	}
}

// The English page is built alike, with its own titles, sentences and addresses.
func TestParseStudyPagesEnglish(t *testing.T) {
	pages := readStudyPages(t, "btu_studienorganisation_en.html", studyOrganisationEN)
	if got := groups(pages); got != "Quicklinks 5, Examinations 6, Formalities 8, Study successfully 6, Special life situations 3, Health & Insurance 3" {
		t.Errorf("groups %s", got)
	}
	first := pages[5]
	if first.Title != "Registering for Examinations" || first.URL != studyOrganisationEN+"/examinations/registering-for-examinations" || !strings.HasPrefix(first.Abstract, "Here you can find out how the registration works") {
		t.Errorf("%+v", first)
	}
}

// A page whose layout changed (no menu of teasers) yields nothing, and a tile without a link is
// left out.
func TestParseStudyPagesWithoutTiles(t *testing.T) {
	pages, err := ParseStudyPages([]byte(`<html><body><!--TYPO3SEARCH_begin--><h2>Prüfungen</h2><p><a href="/x">X</a></p>
		<section class="btu-menu-teaser_element"><h3 class="btu-menu-teaser_title">Ohne Link</h3></section><!--TYPO3SEARCH_end--></body></html>`), studyOrganisationDE)
	if err != nil || len(pages) != 0 {
		t.Errorf("%+v, %v", pages, err)
	}
}
