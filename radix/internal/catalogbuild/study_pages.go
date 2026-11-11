package catalogbuild

import (
	"fmt"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/model"
	"github.com/leonieziechmann/betula/radix/internal/parser"
)

// studyPages is the BTU's page „Studienorganisation" in one language, as archived.
type studyPages struct {
	lang  string // "de" or "en", the key of its page in the archive
	url   string
	pages []model.StudyPage
}

// loadStudyPages reads the archived pages „Studienorganisation", German then English (the order of
// their keys). A page the server answered with anything but its content (a 404) yields nothing; so
// does a page whose layout changed, which validate then warns of.
func loadStudyPages(db *catalogdb.DB, src *sources) error {
	return db.EachPage(catalogdb.SourceBTUStudyOrganisation, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 || (p.Key != "de" && p.Key != "en") {
			return nil
		}
		pages, err := parser.ParseStudyPages(p.Body, p.URL)
		if err != nil {
			return fmt.Errorf("the BTU's page Studienorganisation (%s): %w", p.Key, err)
		}
		src.studyPages = append(src.studyPages, studyPages{lang: p.Key, url: p.URL, pages: pages})
		return nil
	})
}

// writeStudyPages writes every page of the pages „Studienorganisation" in the order of its page
// (study_page; docs/radix/schema-v2.md, „Study pages"): its group, its title and the sentence under
// it as the BTU writes them, its address.
func (b *builder) writeStudyPages() error {
	stmt, err := b.tx.Prepare(`INSERT INTO study_page (lang, ord, section, title, abstract, url, source_url) VALUES (?, ?, ?, ?, ?, ?, ?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()
	for _, page := range b.src.studyPages {
		for i, p := range page.pages {
			if _, err := stmt.Exec(page.lang, i+1, p.Section, p.Title, null(p.Abstract), p.URL, page.url); err != nil {
				return fmt.Errorf("study page %s %d (%s): %w", page.lang, i+1, p.URL, err)
			}
			b.report.StudyPages++
		}
	}
	return nil
}
