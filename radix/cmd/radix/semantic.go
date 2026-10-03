package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/embed"
	"github.com/leonieziechmann/betula/radix/internal/gemini"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/secrets"
	"github.com/leonieziechmann/betula/radix/internal/service"
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
	f := addEncoderFlags(fs)
	f.summaryModel = fs.String("summary-model", envOr("GEMINI_SUMMARY_MODEL", gemini.DefaultModel),
		"Gemini model of the modules' summaries (env GEMINI_SUMMARY_MODEL)")
	f.perMinute = fs.Int("gemini-rpm", envInt("RADIX_GEMINI_RPM", gemini.DefaultRequestsPerMinute),
		"Gemini requests a minute, at most: the free tier's limit or less (env RADIX_GEMINI_RPM)")
	f.perDay = fs.Int("gemini-rpd", envInt("RADIX_GEMINI_RPD", gemini.DefaultRequestsPerDay),
		"Gemini requests a day, at most (env RADIX_GEMINI_RPD)")
	return f
}

// addEncoderFlags are the flags of the vectors alone: an offline Radix (serve-snapshot) asks
// Gemini nothing and embeds the summaries its database has.
func addEncoderFlags(fs *flag.FlagSet) semanticFlags {
	return semanticFlags{
		model: fs.String("embed-model", envOr("RADIX_EMBED_MODEL", ""),
			"The packed e5 model of the semantic search's vectors (e5-de-en-server.bin, folia/crates/semantic/README.md); empty: no vectors (env RADIX_EMBED_MODEL)"),
		// GOMAXPROCS follows the container's CPU limit (at least 2), NumCPU the host's.
		workers: fs.Int("embed-workers", envInt("RADIX_EMBED_WORKERS", max(1, runtime.GOMAXPROCS(0)-1)),
			"Passages embedded at the same time, each by a process of its own holding the model, about 170 MB (env RADIX_EMBED_WORKERS)"),
		budget: fs.Duration("semantic-budget", envDuration("RADIX_SEMANTIC_BUDGET", 20*time.Minute),
			"Time a cycle may spend on summaries and vectors; the rest follows in the next cycles (env RADIX_SEMANTIC_BUDGET)"),
	}
}

// setup loads the model and, when there is a Gemini API key, the summarizer. Without a model
// the stage does not run; without a key the vectors are computed from the modules' texts
// alone; flags without Gemini's (addEncoderFlags, offline) never look for a key. The returned
// function releases the model.
//
// Log events: semantic.enabled, semantic.disabled, semantic.gemini_disabled (WARN).
func (f semanticFlags) setup(ctx context.Context) (service.Semantic, func(), error) {
	log := oplog.For("semantic")
	if *f.model == "" {
		log.Info("no embedding model; the snapshot carries no vectors for the semantic search",
			"event", "semantic.disabled", "how_to", "set RADIX_EMBED_MODEL (folia/crates/semantic/README.md)")
		return service.Semantic{}, func() {}, nil
	}
	start := time.Now()
	self, err := os.Executable()
	if err != nil {
		return service.Semantic{}, nil, err
	}
	// The encoders run in processes of their own: the WebAssembly's long calls, which Go cannot
	// preempt, would otherwise stall this process's HTTP server for seconds (embed.Processes).
	encoder, err := embed.StartProcesses(*f.model, *f.workers, func() *exec.Cmd {
		return exec.Command(self, "embed-worker", *f.model)
	})
	if err != nil {
		return service.Semantic{}, nil, err
	}
	cfg := service.Semantic{Encoder: encoder, Budget: *f.budget, Workers: *f.workers, Batch: 20}
	if f.summaryModel == nil {
		log.Info("semantic search enabled, offline: the vectors without asking Gemini", "event", "semantic.enabled",
			"model", *f.model, "model_id", encoder.ID(), "workers", *f.workers, "budget", f.budget.String(), "summaries", false,
			"load_ms", time.Since(start).Milliseconds())
		return cfg, encoder.Close, nil
	}

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
		encoder.Close()
		return service.Semantic{}, nil, err
	}
	log.Info("semantic search enabled", "event", "semantic.enabled", "model", *f.model, "model_id", encoder.ID(),
		"workers", *f.workers, "budget", f.budget.String(), "summaries", cfg.Summarizer != nil, "summary_model", *f.summaryModel,
		"key_source", string(source), "load_ms", time.Since(start).Milliseconds())
	return cfg, encoder.Close, nil
}

// runEmbedWorker is a process of embed.Processes: the passages on stdin, their vectors on
// stdout (embed.Serve). Its log goes to stderr, which is Radix's.
func runEmbedWorker(ctx context.Context, args []string) {
	if len(args) != 1 {
		fmt.Fprintln(os.Stderr, "usage: radix embed-worker MODEL (started by radix run)")
		os.Exit(2)
	}
	if err := embed.Serve(ctx, args[0], os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, "embed-worker:", err)
		os.Exit(1)
	}
}
