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
	generalPageDE = "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen"
	generalPageEN = "https://www.b-tu.de/en/study/during-studies/study-organization/examinations/regulations-and-forms/basic-regulations"
)

// putGeneralPages archives the BTU's page of Grundsatzordnungen as it was on 2026-11-10, German and
// English (parser/testdata), the German one changed by edit.
func putGeneralPages(t *testing.T, db *catalogdb.DB, edit func(string) string) {
	t.Helper()
	for _, page := range []struct{ key, file, url string }{
		{"de", "btu_grundsatzordnungen_de.html", generalPageDE},
		{"en", "btu_grundsatzordnungen_en.html", generalPageEN},
	} {
		body, err := os.ReadFile(filepath.Join("..", "parser", "testdata", page.file))
		if err != nil {
			t.Fatal(err)
		}
		if page.key == "de" && edit != nil {
			body = []byte(edit(string(body)))
		}
		if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceBTUGeneralDocuments, Key: page.key, URL: page.url, HTTPStatus: 200, Body: body,
			FetchedAt: time.Date(2026, 11, 10, 3, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatal(err)
		}
	}
}

func validated(t *testing.T, db *catalogdb.DB) map[string]catalogdb.Check {
	t.Helper()
	checks, err := db.Validate(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	byName := make(map[string]catalogdb.Check)
	for _, c := range checks {
		byName[c.Name] = c
	}
	return byName
}

// The build turns both pages into general_document: every document in the order of its page, the
// Rahmenordnungen and the Immatrikulationsordnung with their amendments, the FÜS and the law by
// their topic, each document of the Amtsblatt named by its issue (the German page's where both list
// it: the English title of the sixth amendment for Master's programmes has the wrong year), the
// BTU's translations without one. Validate is content.
func TestGeneralDocumentsFromTheBTUsPage(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	putGeneralPages(t, db, nil)
	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if report.GeneralDocuments != 100 {
		t.Errorf("%d general documents, want 100 (50 on each page)", report.GeneralDocuments)
	}

	want(t, db, `SELECT lang, COUNT(*), COUNT(DISTINCT section), COUNT(topic), COUNT(issue) FROM v_general_document GROUP BY lang ORDER BY lang`,
		"de|50|13|18|42", "en|50|13|18|39")
	want(t, db, `SELECT ord, kind, issue, title FROM v_general_document WHERE lang = 'de' AND topic = 'framework_bachelor' ORDER BY ord`,
		"21|statute|13/2016|Allgemeine Prüfungs- und Studienordnung für Bachelor-Studiengänge vom 12. September 2016",
		"22|amendment|01/2021|Erste Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom 26. Januar 2021",
		"23|amendment|17/2022|Zweite Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom 12. September 2016",
		"24|amendment|06/2023|Dritte Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom am 25. April 2023",
		"25|amendment|14/2024|Vierte Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom 24. Juni 2024",
		"26|amendment|29/2024|Fünfte Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom 29. August 2024",
		"27|amendment|35/2026|Sechste Änderungssatzung zur Allgemeinen Prüfungs- und Studienordnung für Bachelor-Studiengänge (RahmenO-BA) vom 29. Juli 2026")
	want(t, db, `SELECT kind, issue FROM v_general_document WHERE lang = 'de' AND topic = 'framework_master' ORDER BY ord`,
		"statute|14/2016", "amendment|02/2021", "amendment|18/2022", "amendment|07/2023", "amendment|15/2024", "amendment|30/2024", "amendment|38/2026")
	want(t, db, `SELECT lang, topic, kind, issue, url FROM v_general_document WHERE topic IN ('enrolment', 'fues', 'higher_education_act') ORDER BY topic, lang, ord`,
		"de|enrolment|statute|01/2020|https://opus4.kobv.de/opus4-btu/files/5109/AMbl-01_2020_ImmaO.pdf",
		"de|enrolment|amendment|10/2022|https://opus4.kobv.de/opus4-btu/files/6002/AMbl-10_2022_ImmaO_Aussetzung_Paragraph_16.pdf",
		"en|enrolment|statute|∅|https://www-docs.b-tu.de/studierende/public/ImmaO/ImmaO_28.01.2020_englih.pdf",
		"en|enrolment|amendment|10/2022|https://opus4.kobv.de/opus4-btu/files/6002/AMbl-10_2022_ImmaO_Aussetzung_Paragraph_16.pdf",
		"de|fues|statute|04/2010|http://opus.kobv.de/btu/volltexte/2010/1748/pdf/04_FAS_Rili_230410.pdf",
		"en|fues|statute|04/2010|http://opus.kobv.de/btu/volltexte/2010/1748/pdf/04_FAS_Rili_230410.pdf",
		"de|higher_education_act|statute|∅|https://bravors.brandenburg.de/gesetze/bbghg",
		"en|higher_education_act|statute|∅|https://bravors.brandenburg.de/gesetze/bbghg")
	want(t, db, `SELECT kind, issue, url FROM v_general_document WHERE lang = 'en' AND topic = 'framework_master' AND (kind = 'statute' OR ord = (SELECT MAX(ord) FROM general_document WHERE lang = 'en' AND topic = 'framework_master'))`,
		"statute|∅|https://www-docs.b-tu.de/studierende/public/rahmenordnungen/master-en/29-RahmnO_MA_5-Aenderungssatzung_EN.pdf",
		"amendment|38/2026|https://opus4.kobv.de/opus4-btu/files/7450/AMbl-38_2026.pdf")
	want(t, db, `SELECT DISTINCT source_url FROM general_document ORDER BY lang`, generalPageDE, generalPageEN)

	checks := validated(t, db)
	for _, name := range []string{"pages of the BTU's Grundsatzordnungen that yield no document", "topics Folia names that the BTU's German page of Grundsatzordnungen lacks",
		"no empty-string or '-' placeholders in text columns"} {
		if c := checks[name]; c.Status != catalogdb.StatusOK {
			t.Errorf("%s: %+v", name, c)
		}
	}
	if c := checks["documents of the BTU's pages of Grundsatzordnungen"]; c.Status != catalogdb.StatusInfo || c.Value != 100 {
		t.Errorf("the count: %+v", c)
	}
}

// What the page does not yield, validate warns of, and Folia has nothing instead of old documents:
// nothing archived, a German page whose heading of the Bachelor's Rahmenordnung the rules no longer
// recognise, an English page that answered 404.
func TestValidateWarnsWhenTheBTUsPageYieldsNothing(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	if _, err := Build(context.Background(), db); err != nil {
		t.Fatal(err)
	}
	checks := validated(t, db)
	if c := checks["pages of the BTU's Grundsatzordnungen that yield no document"]; c.Status != catalogdb.StatusWarn || c.Value != 2 || strings.Join(c.Samples, ",") != "de,en" {
		t.Errorf("nothing archived: %+v", c)
	}
	if c := checks["topics Folia names that the BTU's German page of Grundsatzordnungen lacks"]; c.Status != catalogdb.StatusOK {
		t.Errorf("nothing archived, nothing more to say: %+v", c)
	}

	putGeneralPages(t, db, func(page string) string {
		return strings.Replace(page, "Rahmenordnung für Bachelor-Studiengänge\n", "Allgemeine Ordnung für das erste Studium\n", 1)
	})
	if err := db.PutPage(catalogdb.RawPage{Source: catalogdb.SourceBTUGeneralDocuments, Key: "en", URL: generalPageEN, HTTPStatus: 404,
		FetchedAt: time.Date(2026, 11, 11, 3, 0, 0, 0, time.UTC)}); err != nil {
		t.Fatal(err)
	}
	if _, err := Build(context.Background(), db); err != nil {
		t.Fatal(err)
	}
	want(t, db, `SELECT lang, COUNT(*), COUNT(topic) FROM general_document GROUP BY lang`, "de|50|11")
	checks = validated(t, db)
	if c := checks["pages of the BTU's Grundsatzordnungen that yield no document"]; c.Status != catalogdb.StatusWarn || c.Value != 1 || strings.Join(c.Samples, ",") != "en" {
		t.Errorf("the English page answered 404: %+v", c)
	}
	if c := checks["topics Folia names that the BTU's German page of Grundsatzordnungen lacks"]; c.Status != catalogdb.StatusWarn || c.Value != 1 || strings.Join(c.Samples, ",") != "framework_bachelor" {
		t.Errorf("a heading renamed: %+v", c)
	}
	if catalogdb.HasFailures(mapValues(checks)) {
		t.Errorf("what the page does not yield fails nothing: %+v", checks)
	}
}

func mapValues(m map[string]catalogdb.Check) []catalogdb.Check {
	var all []catalogdb.Check
	for _, c := range m {
		all = append(all, c)
	}
	return all
}
