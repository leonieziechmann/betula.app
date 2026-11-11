package catalogbuild

import (
	"fmt"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/model"
	"github.com/leonieziechmann/betula/radix/internal/normalize"
	"github.com/leonieziechmann/betula/radix/internal/parser"
)

// generalPage is the BTU's page of Grundsatzordnungen in one language, as archived.
type generalPage struct {
	lang string // "de" or "en", the key of its page in the archive
	url  string
	docs []model.GeneralDocument
}

// loadGeneralPages reads the archived pages of Grundsatzordnungen, German then English (the order
// of their keys). A page the server answered with anything but its content (a 404) yields nothing;
// so does a page whose layout changed, which validate then warns of.
func loadGeneralPages(db *catalogdb.DB, src *sources) error {
	return db.EachPage(catalogdb.SourceBTUGeneralDocuments, func(p *catalogdb.RawPage) error {
		if p.HTTPStatus != 200 || len(p.Body) == 0 || (p.Key != "de" && p.Key != "en") {
			return nil
		}
		docs, err := parser.ParseGeneralDocuments(p.Body, p.URL)
		if err != nil {
			return fmt.Errorf("the BTU's page of Grundsatzordnungen (%s): %w", p.Key, err)
		}
		src.generalPages = append(src.generalPages, generalPage{lang: p.Key, url: p.URL, docs: docs})
		return nil
	})
}

// writeGeneralDocuments writes every document of the pages of Grundsatzordnungen in the order of
// its page (general_document; docs/radix/schema-v2.md, „General documents"), with what it is
// about (normalize.GeneralTopic), what it is in its section (normalize.GeneralKind) and its issue
// of the Amtsblatt (normalize.GazetteIssue). A document both pages list has the issue the German
// one tells, whose title is the BTU's German one.
func (b *builder) writeGeneralDocuments() error {
	issues := make(map[string]string)
	for _, page := range b.src.generalPages {
		for _, d := range page.docs {
			if _, ok := issues[d.URL]; !ok && page.lang == "de" {
				issues[d.URL] = normalize.GazetteIssue(d.URL, d.Title)
			}
		}
	}
	stmt, err := b.tx.Prepare(`INSERT INTO general_document (lang, ord, section, topic, kind, title, issue, url, source_url)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()
	for _, page := range b.src.generalPages {
		for i, d := range page.docs {
			issue, ok := issues[d.URL]
			if !ok {
				issue = normalize.GazetteIssue(d.URL, d.Title)
			}
			if _, err := stmt.Exec(page.lang, i+1, d.Section, null(normalize.GeneralTopic(d.Section, d.Title)), normalize.GeneralKind(d.Fold),
				d.Title, null(issue), d.URL, page.url); err != nil {
				return fmt.Errorf("general document %s %d (%s): %w", page.lang, i+1, d.URL, err)
			}
			b.report.GeneralDocuments++
		}
	}
	return nil
}
