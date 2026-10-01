package main

import (
	"context"
	"errors"
	"flag"
	"runtime"
	"time"

	"github.com/leonieziechmann/betula/internal/embed"
	"github.com/leonieziechmann/betula/internal/gemini"
	"github.com/leonieziechmann/betula/internal/oplog"
	"github.com/leonieziechmann/betula/internal/secrets"
	"github.com/leonieziechmann/betula/internal/service"
)

// semanticFlags configure the vectors of the semantic search (service.Semantic).
type semanticFlags struct {
	model        *string
	workers      *int
	budget       *time.Duration
	summaryModel *string
	perMinute    *int
	perDay       *int
}

func addSemanticFlags(fs *flag.FlagSet) semanticFlags {
	return semanticFlags{
		model: fs.String("embed-model", envOr("RADIX_EMBED_MODEL", ""),
			"The packed e5 model of the semantic search's vectors (e5-de-en-server.bin, semantic/README.md); empty: no vectors (env RADIX_EMBED_MODEL)"),
		workers: fs.Int("embed-workers", envInt("RADIX_EMBED_WORKERS", max(1, runtime.NumCPU()-1)),
			"Passages embedded at the same time, each by an instance of the model of about 60 MB (env RADIX_EMBED_WORKERS)"),
		budget: fs.Duration("semantic-budget", envDuration("RADIX_SEMANTIC_BUDGET", 20*time.Minute),
			"Time a cycle may spend on summaries and vectors; the rest follows in the next cycles (env RADIX_SEMANTIC_BUDGET)"),
		summaryModel: fs.String("summary-model", envOr("GEMINI_SUMMARY_MODEL", gemini.DefaultModel),
			"Gemini model of the modules' summaries (env GEMINI_SUMMARY_MODEL)"),
		perMinute: fs.Int("gemini-rpm", envInt("RADIX_GEMINI_RPM", gemini.DefaultRequestsPerMinute),
			"Gemini requests a minute, at most: the free tier's limit or less (env RADIX_GEMINI_RPM)"),
		perDay: fs.Int("gemini-rpd", envInt("RADIX_GEMINI_RPD", gemini.DefaultRequestsPerDay),
			"Gemini requests a day, at most (env RADIX_GEMINI_RPD)"),
	}
}

// setup loads the model and, when there is a Gemini API key, the summarizer. Without a model
// the stage does not run; without a key the vectors are computed from the modules' texts
// alone. The returned function releases the model.
//
// Log events: semantic.enabled, semantic.disabled, semantic.gemini_disabled (WARN).
func (f semanticFlags) setup(ctx context.Context) (service.Semantic, func(), error) {
	log := oplog.For("semantic")
	if *f.model == "" {
		log.Info("no embedding model; the snapshot carries no vectors for the semantic search",
			"event", "semantic.disabled", "how_to", "set RADIX_EMBED_MODEL (semantic/README.md)")
		return service.Semantic{}, func() {}, nil
	}
	start := time.Now()
	encoder, err := embed.Load(ctx, *f.model, *f.workers)
	if err != nil {
		return service.Semantic{}, nil, err
	}
	cfg := service.Semantic{Encoder: encoder, Budget: *f.budget, Workers: *f.workers, Batch: 20}

	// The key comes from the secret sources only, never from a flag or a config file.
	key, source, err := secrets.Resolve(secrets.GeminiAPIKey)
	switch {
	case err == nil:
		client := gemini.NewClient(key, *f.summaryModel)
		client.SetLimiter(gemini.NewLimiter(*f.perMinute, *f.perDay))
		cfg.Summarizer, cfg.SummaryModel = client, *f.summaryModel
	case errors.Is(err, secrets.ErrNotFound):
		log.Warn("no Gemini API key; the vectors are computed from the modules' texts without summaries",
			"event", "semantic.gemini_disabled", "how_to", secrets.HowTo(secrets.GeminiAPIKey))
	default:
		_ = encoder.Close(ctx)
		return service.Semantic{}, nil, err
	}
	log.Info("semantic search enabled", "event", "semantic.enabled", "model", *f.model, "model_id", encoder.ID(),
		"workers", *f.workers, "budget", f.budget.String(), "summaries", cfg.Summarizer != nil, "summary_model", *f.summaryModel,
		"key_source", string(source), "load_ms", time.Since(start).Milliseconds())
	return cfg, func() { _ = encoder.Close(context.Background()) }, nil
}
