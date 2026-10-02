package service

import (
	"context"
	"errors"
	"fmt"
	"sort"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/gemini"
	"github.com/leonieziechmann/betula/internal/oplog"
	"github.com/leonieziechmann/betula/internal/semantic"
)

// Semantic is the setup of the semantic stage, which computes what Folia's semantic search
// compares a query with: a vector for every module (docs/schema-v2.md, „Semantic search“).
// Without an Encoder the stage does not run; without a Summarizer the vectors are computed
// from the modules' texts alone.
type Semantic struct {
	Encoder Encoder

	Summarizer   Summarizer
	SummaryModel string // the Gemini model, recorded with each summary
	Batch        int    // modules per Gemini request

	// Budget is the time the stage may take in a cycle; what does not fit is done in the next
	// ones. The first cycles of a new model compute every module's vector: about an hour of
	// several processors.
	Budget  time.Duration
	Workers int // passages embedded at the same time: the encoder's instances
}

// Encoder computes the vector of a passage as the snapshot publishes it, packed values and
// their scale: embed.Encoder, the crate semantic/ as WebAssembly.
type Encoder interface {
	ID() string // the model; vectors of another model are dropped
	EmbedPassage(ctx context.Context, passage string) (scale float32, packed []byte, err error)
}

// Summarizer writes the summaries of module texts: gemini.Client.
type Summarizer interface {
	SummarizeModules(ctx context.Context, modules []gemini.ModuleText) ([]gemini.ModuleSummary, error)
}

// semanticStats is what one run of the stage did.
type semanticStats struct {
	Texts, SummariesWritten, SummariesMissing int
	VectorsWritten, VectorsMissing, Dropped   int
	SummaryError                              error
}

// semanticStage runs the stage at the end of a cycle: the summaries of the module texts Gemini
// has not summarised yet, then the vectors of the passages that have none. What it computes is
// published by the next build (catalogbuild, „module vectors“). A failure of the vectors (the
// database, the encoder) degrades the cycle; one of Gemini, which is optional, is a WARN and
// shows in the stage's error, and the vectors go on without the summaries it did not write.
// The snapshot keeps the vectors it has.
//
// Log events: semantic.finished, semantic.summaries_failed (WARN), semantic.gemini_daily_limit,
// stage.failed (ERROR).
func (s *Service) semanticStage(ctx context.Context, result *CycleResult) {
	cfg := s.cfg.Semantic
	if cfg.Encoder == nil {
		result.Stages = append(result.Stages, StageResult{Name: "semantic", Skipped: "no embedding model"})
		return
	}
	log := oplog.For("semantic")
	start := s.now()
	if s.summaryFailed == nil {
		s.summaryFailed = map[string]time.Time{}
	}
	stats, err := runSemantic(ctx, s.db, cfg, start.Add(cfg.Budget), s.now, s.summaryFailed)
	stage := StageResult{Name: "semantic", DurationMS: s.now().Sub(start).Milliseconds()}
	switch {
	case err != nil && ctx.Err() == nil:
		stage.Error = err.Error()
		if result.Result == "ok" {
			result.Result = "degraded"
		}
		log.Error("semantic stage failed", "event", "stage.failed", "stage", "semantic", oplog.Err(err))
	case stats.SummaryError != nil:
		stage.Error = stats.SummaryError.Error() // logged by summarize, as a WARN
	}
	result.Stages = append(result.Stages, stage)
	log.Info("semantic stage finished", "event", "semantic.finished", "duration_ms", stage.DurationMS,
		"texts", stats.Texts, "summaries_written", stats.SummariesWritten, "summaries_missing", stats.SummariesMissing,
		"vectors_written", stats.VectorsWritten, "vectors_missing", stats.VectorsMissing, "vectors_dropped", stats.Dropped)
}

type semanticText struct {
	text       semantic.Text
	hash       string
	department string
}

func runSemantic(ctx context.Context, db *catalogdb.DB, cfg Semantic, deadline time.Time, now func() time.Time, failed map[string]time.Time) (semanticStats, error) {
	var stats semanticStats
	modules, err := db.SemanticModules()
	if err != nil {
		return stats, err
	}
	// Modules that say the same thing share a summary (texts). The passages are each module's
	// own, made from the text as the build makes them (catalogbuild, „module vectors“), not from
	// a cleaned one: Plain needs the lines of a description to drop the markers of its lists, and
	// a passage made otherwise has a hash the build never looks up. Equal passages share a vector.
	var texts, all []semanticText
	seen := map[string]bool{}
	for _, m := range modules {
		t := semantic.Text{TitleDE: m.TitleDE, TitleEN: m.TitleEN, Contents: m.Contents, Outcomes: m.Outcomes}
		text := semanticText{text: t, hash: t.Hash(), department: m.Department}
		all = append(all, text)
		if !seen[text.hash] {
			seen[text.hash] = true
			texts = append(texts, text)
		}
	}
	stats.Texts = len(texts)

	if cfg.Summarizer != nil {
		if err := summarize(ctx, db, cfg, texts, deadline, now, failed, &stats); err != nil {
			return stats, err
		}
	}
	return stats, embedPassages(ctx, db, cfg, all, deadline, now, &stats)
}

// summaryRetry is how long a text Gemini did not summarise waits before it is asked again: an
// answer that failed at temperature 0 fails again, and every cycle would spend a request on it.
const summaryRetry = 24 * time.Hour

// summarize asks Gemini for the summaries of the texts that have none, a batch at a time,
// until the budget or the day's requests are used up. failed remembers when a text's summary
// failed last.
func summarize(ctx context.Context, db *catalogdb.DB, cfg Semantic, texts []semanticText, deadline time.Time, now func() time.Time, failed map[string]time.Time, stats *semanticStats) error {
	log := oplog.For("semantic")
	have, err := db.ModuleSummaries()
	if err != nil {
		return err
	}
	var missing []semanticText
	for _, t := range texts {
		if _, ok := have[t.hash]; !ok && now().Sub(failed[t.hash]) >= summaryRetry {
			missing = append(missing, t)
		}
	}
	batch := cfg.Batch
	if batch <= 0 {
		batch = 20
	}
	for len(missing) > 0 && now().Before(deadline) && ctx.Err() == nil {
		n := min(batch, len(missing))
		request := make([]gemini.ModuleText, n)
		for i, t := range missing[:n] {
			request[i] = gemini.ModuleText{Key: t.hash, TitleDE: t.text.TitleDE, TitleEN: t.text.TitleEN,
				Department: t.department, Contents: semantic.Plain(t.text.Contents), Outcomes: semantic.Plain(t.text.Outcomes)}
		}
		answers, err := cfg.Summarizer.SummarizeModules(ctx, request)
		if len(answers) > 0 {
			rows := make([]catalogdb.ModuleSummary, len(answers))
			for i, a := range answers {
				rows[i] = catalogdb.ModuleSummary{TextHash: a.Key, SummaryDE: a.SummaryDE, SummaryEN: a.SummaryEN,
					Keywords: a.Keywords, Model: cfg.SummaryModel, CreatedAt: now()}
			}
			if err := db.SaveModuleSummaries(rows); err != nil {
				return err
			}
			stats.SummariesWritten += len(rows)
		}
		missing = missing[n:]
		if err != nil {
			if errors.Is(err, gemini.ErrDailyLimit) {
				log.Info("Gemini's requests for today are used up; the other summaries follow tomorrow",
					"event", "semantic.gemini_daily_limit", "missing", len(missing)+n-len(answers))
				break
			}
			var incomplete *gemini.IncompleteError
			if errors.As(err, &incomplete) {
				// The modules Gemini did not answer well are asked again in a day.
				for _, k := range incomplete.Keys {
					failed[k] = now()
				}
				log.Warn("Gemini left some summaries out", "event", "semantic.summaries_failed", "missing", len(incomplete.Keys), oplog.Err(err))
				continue
			}
			stats.SummaryError = fmt.Errorf("summaries: %w", err)
			log.Warn("Gemini failed; the summaries wait for the next cycle", "event", "semantic.summaries_failed", oplog.Err(err))
			break
		}
	}
	have, err = db.ModuleSummaries()
	if err != nil {
		return err
	}
	for _, t := range texts {
		if _, ok := have[t.hash]; !ok {
			stats.SummariesMissing++
		}
	}
	return nil
}

// embedPassages computes the vectors of the passages that have none under the model, on
// several goroutines, until the budget is used up, and stores them as they come.
func embedPassages(ctx context.Context, db *catalogdb.DB, cfg Semantic, texts []semanticText, deadline time.Time, now func() time.Time, stats *semanticStats) error {
	model := cfg.Encoder.ID()
	dropped, err := db.DropPassageEmbeddingsExcept(model)
	if err != nil {
		return err
	}
	stats.Dropped = dropped
	have, err := db.PassageEmbeddingHashes(model)
	if err != nil {
		return err
	}
	summaries, err := db.ModuleSummaries()
	if err != nil {
		return err
	}
	type passage struct{ hash, text string }
	var todo []passage
	queued := map[string]bool{}
	for _, t := range texts {
		var summary *semantic.Summary
		if s, ok := summaries[t.hash]; ok {
			summary = &semantic.Summary{DE: s.SummaryDE, EN: s.SummaryEN, Keywords: s.Keywords}
		}
		p := semantic.Passage(t.text, summary)
		h := semantic.PassageHash(p)
		if !have[h] && !queued[h] {
			queued[h] = true
			todo = append(todo, passage{h, p})
		}
	}
	// The short ones first: more modules get a vector within the budget.
	sort.SliceStable(todo, func(i, j int) bool { return len(todo[i].text) < len(todo[j].text) })

	workers := max(1, cfg.Workers)
	jobs := make(chan passage)
	done := make(chan catalogdb.PassageEmbedding)
	var wg sync.WaitGroup
	var errMu sync.Mutex
	var embedErr error
	for range workers {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for p := range jobs {
				scale, packed, err := cfg.Encoder.EmbedPassage(ctx, p.text)
				if err != nil {
					errMu.Lock()
					embedErr = errors.Join(embedErr, fmt.Errorf("passage %s: %w", p.hash[:12], err))
					errMu.Unlock()
					continue
				}
				done <- catalogdb.PassageEmbedding{PassageHash: p.hash, Scale: scale, Vector: packed}
			}
		}()
	}
	go func() {
		defer close(jobs)
		for _, p := range todo {
			if ctx.Err() != nil || !now().Before(deadline) {
				return
			}
			jobs <- p
		}
	}()
	go func() { wg.Wait(); close(done) }()

	var pending []catalogdb.PassageEmbedding
	var saveErr error
	flush := func() {
		if len(pending) > 0 && saveErr == nil {
			if saveErr = db.SavePassageEmbeddings(model, pending); saveErr == nil {
				stats.VectorsWritten += len(pending)
			}
		}
		pending = pending[:0]
	}
	for e := range done {
		pending = append(pending, e)
		if len(pending) == 32 {
			flush()
		}
	}
	flush()
	stats.VectorsMissing = len(todo) - stats.VectorsWritten
	if saveErr != nil {
		return saveErr
	}
	if ctx.Err() != nil {
		return nil // stopping, not failing
	}
	return embedErr
}
