package catalogbuild

import (
	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/semantic"
)

// writeModuleVectors gives each module the vector of its passage of this build, if the
// semantic stage has computed it (passage_embedding): the passage of the module's text with
// the summary of that text, when Gemini wrote one (module_summary). Until that passage has a
// vector, the module keeps the one of its text alone, if it has one: a summary that arrives
// before the stage has embedded it must not take a published vector away. A module with
// neither gets no row; the stage after this build computes it, and the next build publishes
// it.
func (b *builder) writeModuleVectors() error {
	summaries, err := catalogdb.ReadModuleSummaries(b.tx)
	if err != nil {
		return err
	}
	rows, err := b.tx.Query(`SELECT id, COALESCE(title_de, title), COALESCE(title_en, ''), COALESCE(contents, ''), COALESCE(learning_outcomes, '') FROM module ORDER BY id`)
	if err != nil {
		return err
	}
	type module struct {
		id   string
		text semantic.Text
	}
	var modules []module
	for rows.Next() {
		var m module
		if err := rows.Scan(&m.id, &m.text.TitleDE, &m.text.TitleEN, &m.text.Contents, &m.text.Outcomes); err != nil {
			rows.Close()
			return err
		}
		modules = append(modules, m)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	// The passage with the summary first, else the text alone.
	insert, err := b.tx.Prepare(`INSERT INTO module_vector (module_id, scale, vector)
		SELECT ?1, scale, vector FROM passage_embedding WHERE passage_hash IN (?2, ?3)
		ORDER BY passage_hash = ?2 DESC LIMIT 1`)
	if err != nil {
		return err
	}
	defer insert.Close()
	for _, m := range modules {
		alone := semantic.PassageHash(semantic.Passage(m.text, nil))
		best := alone
		if s, ok := summaries[m.text.Hash()]; ok {
			best = semantic.PassageHash(semantic.Passage(m.text, &semantic.Summary{DE: s.SummaryDE, EN: s.SummaryEN, Keywords: s.Keywords}))
		}
		res, err := insert.Exec(m.id, best, alone)
		if err != nil {
			return err
		}
		if n, _ := res.RowsAffected(); n > 0 {
			b.report.ModuleVectors++
		}
	}
	return nil
}
