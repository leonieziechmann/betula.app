package catalogbuild

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

const (
	studyPageDE = "https://www.b-tu.de/studium/im-studium/studienorganisation"
	studyPageEN = "https://www.b-tu.de/en/study/during-studies/study-organization"
)

// putStudyPages archives the BTU's page „Studienorganisation" as it was on 2026-11-11, German and
// English (parser/testdata).
func putStudyPages(t *testing.T, db *catalogdb.DB) {
	t.Helper()
	for _, page := range []struct{ key, file, url string }{
		{"de", "btu_studienorganisation_de.html", studyPageDE},
		{"en", "btu_studienorganisation_en.html", studyPageEN},
	} {
		body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", page.file))
		if err != nil {
			t.Fatal(err)
		}
		if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceBTUStudyOrganisation, Key: page.key, URL: page.url, HTTPStatus: 200, Body: body,
			FetchedAt: time.Date(2026, 11, 11, 3, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatal(err)
		}
	}
}

// The build turns both pages into study_page: every page in the order of its page, with its group,
// its title and the sentence under it as the BTU writes them, and its address. Validate is content.
func TestStudyPagesFromTheBTUsPage(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	putStudyPages(t, db)
	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if report.StudyPages != 62 {
		t.Errorf("%d study pages, want 62 (31 on each page)", report.StudyPages)
	}
	want(t, db, `SELECT lang, COUNT(*), COUNT(DISTINCT section), COUNT(abstract) FROM v_study_page GROUP BY lang ORDER BY lang`,
		"de|31|6|31", "en|31|6|31")
	want(t, db, `SELECT ord, section, title, url FROM v_study_page WHERE lang = 'de' AND ord IN (1, 6, 15, 25) ORDER BY ord`,
		"1|Quicklinks|Portale|https://www.b-tu.de/studium/im-studium/studienorganisation/quicklinks/portale",
		"6|Prüfungen|An- & Abmeldung von Modulprüfungen|https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/an-abmeldung-von-modulpruefungen",
		"15|Formalia|Semesterticket|https://www.stura-btu.de/themen/semesterticket",
		"25|Erfolgreich studieren|Weg zum Master-Studium|https://www.b-tu.de/studium/vor-dem-studium/beratung-hilfe/studien-berufswahl/fachstudienberatung#c403684")
	want(t, db, `SELECT section, title FROM v_study_page WHERE lang = 'en' AND ord = 6`, "Examinations|Registering for Examinations")
	want(t, db, `SELECT DISTINCT source_url FROM study_page ORDER BY lang`, studyPageDE, studyPageEN)

	checks := validated(t, db)
	for _, name := range []string{"pages of the BTU's Studienorganisation that yield no page", "no empty-string or '-' placeholders in text columns"} {
		if c := checks[name]; c.Status != catalogdb.StatusOK {
			t.Errorf("%s: %+v", name, c)
		}
	}
	if c := checks["pages the BTU's pages of Studienorganisation list"]; c.Status != catalogdb.StatusInfo || c.Value != 62 {
		t.Errorf("the count: %+v", c)
	}
}

// What the page does not yield, validate warns of, and Folia has nothing instead of old pages:
// nothing archived, then an English page that answered 404.
func TestValidateWarnsWhenTheStudyOrganisationYieldsNothing(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	if _, err := Build(context.Background(), db); err != nil {
		t.Fatal(err)
	}
	if c := validated(t, db)["pages of the BTU's Studienorganisation that yield no page"]; c.Status != catalogdb.StatusWarn || c.Value != 2 || strings.Join(c.Samples, ",") != "de,en" {
		t.Errorf("nothing archived: %+v", c)
	}

	putStudyPages(t, db)
	if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceBTUStudyOrganisation, Key: "en", URL: studyPageEN, HTTPStatus: 404,
		FetchedAt: time.Date(2026, 11, 12, 3, 0, 0, 0, time.UTC)}); err != nil {
		t.Fatal(err)
	}
	if _, err := Build(context.Background(), db); err != nil {
		t.Fatal(err)
	}
	want(t, db, `SELECT lang, COUNT(*) FROM study_page GROUP BY lang`, "de|31")
	checks := validated(t, db)
	if c := checks["pages of the BTU's Studienorganisation that yield no page"]; c.Status != catalogdb.StatusWarn || c.Value != 1 || strings.Join(c.Samples, ",") != "en" {
		t.Errorf("the English page answered 404: %+v", c)
	}
	if catalogdb.HasFailures(mapValues(checks)) {
		t.Errorf("what the page does not yield fails nothing: %+v", checks)
	}
}
