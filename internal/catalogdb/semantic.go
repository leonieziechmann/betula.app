package catalogdb

import (
	"database/sql"
	"fmt"
	"strings"
	"time"
)

// SemanticModule is what the semantic stage needs of a module: its text and, for Gemini,
// its department.
type SemanticModule struct {
	ID                 string
	TitleDE, TitleEN   string // TitleDE: the German title, else the display title
	Department         string
	Contents, Outcomes string
}

// SemanticModules lists the modules of the last build, in id order.
func (db *DB) SemanticModules() ([]SemanticModule, error) {
	rows, err := db.sql.Query(`
		SELECT m.id, COALESCE(m.title_de, m.title), COALESCE(m.title_en, ''), COALESCE(d.label, ''),
		       COALESCE(m.contents, ''), COALESCE(m.learning_outcomes, '')
		FROM module m LEFT JOIN department d ON d.id = m.department_id
		ORDER BY m.id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []SemanticModule
	for rows.Next() {
		var m SemanticModule
		if err := rows.Scan(&m.ID, &m.TitleDE, &m.TitleEN, &m.Department, &m.Contents, &m.Outcomes); err != nil {
			return nil, err
		}
		out = append(out, m)
	}
	return out, rows.Err()
}

// ModuleSummary is a row of module_summary: what Gemini wrote about the text with this hash.
type ModuleSummary struct {
	TextHash  string
	SummaryDE string
	SummaryEN string
	Keywords  []string
	Model     string
	CreatedAt time.Time
}

// ModuleSummaries reads every summary, by the hash of its text.
func (db *DB) ModuleSummaries() (map[string]ModuleSummary, error) {
	return readModuleSummaries(db.sql)
}

func readModuleSummaries(q interface {
	Query(string, ...any) (*sql.Rows, error)
}) (map[string]ModuleSummary, error) {
	rows, err := q.Query("SELECT text_hash, summary_de, summary_en, keywords, model, created_at FROM module_summary")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[string]ModuleSummary{}
	for rows.Next() {
		var s ModuleSummary
		var keywords, created string
		if err := rows.Scan(&s.TextHash, &s.SummaryDE, &s.SummaryEN, &keywords, &s.Model, &created); err != nil {
			return nil, err
		}
		s.Keywords = strings.Split(keywords, "\n")
		s.CreatedAt, _ = time.Parse(time.RFC3339, created)
		out[s.TextHash] = s
	}
	return out, rows.Err()
}

// ReadModuleSummaries is ModuleSummaries within a transaction (the build's).
func ReadModuleSummaries(tx *sql.Tx) (map[string]ModuleSummary, error) {
	return readModuleSummaries(tx)
}

// SaveModuleSummaries stores summaries, replacing those of the same texts.
func (db *DB) SaveModuleSummaries(summaries []ModuleSummary) error {
	tx, err := db.sql.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, s := range summaries {
		keywords := make([]string, 0, len(s.Keywords))
		for _, k := range s.Keywords {
			if k = strings.Join(strings.Fields(k), " "); k != "" {
				keywords = append(keywords, k)
			}
		}
		if s.SummaryDE == "" || s.SummaryEN == "" || len(keywords) == 0 {
			return fmt.Errorf("summary of %s: empty", s.TextHash)
		}
		if _, err := tx.Exec(`
			INSERT INTO module_summary (text_hash, summary_de, summary_en, keywords, model, created_at)
			VALUES (?, ?, ?, ?, ?, ?)
			ON CONFLICT(text_hash) DO UPDATE SET
				summary_de = excluded.summary_de, summary_en = excluded.summary_en, keywords = excluded.keywords,
				model = excluded.model, created_at = excluded.created_at`,
			s.TextHash, s.SummaryDE, s.SummaryEN, strings.Join(keywords, "\n"), s.Model, s.CreatedAt.UTC().Format(time.RFC3339)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// PassageEmbedding is a row of passage_embedding: the vector of the passage with this hash,
// as int8 codes and their scale.
type PassageEmbedding struct {
	PassageHash string
	Scale       float32
	Vector      []int8
}

// PassageEmbeddingHashes lists the passages that have a vector under model.
func (db *DB) PassageEmbeddingHashes(model string) (map[string]bool, error) {
	rows, err := db.sql.Query("SELECT passage_hash FROM passage_embedding WHERE model = ?", model)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[string]bool{}
	for rows.Next() {
		var h string
		if err := rows.Scan(&h); err != nil {
			return nil, err
		}
		out[h] = true
	}
	return out, rows.Err()
}

// SavePassageEmbeddings stores vectors computed by model.
func (db *DB) SavePassageEmbeddings(model string, embeddings []PassageEmbedding) error {
	tx, err := db.sql.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	now := time.Now().UTC().Format(time.RFC3339)
	for _, e := range embeddings {
		if !(e.Scale > 0) || len(e.Vector) == 0 {
			return fmt.Errorf("vector of %s: scale %v, %d values", e.PassageHash, e.Scale, len(e.Vector))
		}
		if _, err := tx.Exec(`
			INSERT INTO passage_embedding (passage_hash, model, scale, vector, created_at) VALUES (?, ?, ?, ?, ?)
			ON CONFLICT(passage_hash) DO UPDATE SET
				model = excluded.model, scale = excluded.scale, vector = excluded.vector, created_at = excluded.created_at`,
			e.PassageHash, model, float64(e.Scale), int8Bytes(e.Vector), now); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// DropPassageEmbeddingsExcept removes the vectors of every other model than model: vectors of
// two models cannot be compared with one query. Returns how many it removed.
func (db *DB) DropPassageEmbeddingsExcept(model string) (int, error) {
	res, err := db.sql.Exec("DELETE FROM passage_embedding WHERE model <> ?", model)
	if err != nil {
		return 0, err
	}
	n, err := res.RowsAffected()
	return int(n), err
}

func int8Bytes(v []int8) []byte {
	out := make([]byte, len(v))
	for i, c := range v {
		out[i] = byte(c)
	}
	return out
}
