package service

import (
	"context"
	"crypto/sha256"
	"os"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/internal/embed"
	"github.com/leonieziechmann/betula/internal/gemini"
)

// fakeEncoder makes a vector of a passage's hash: equal passages, equal vectors.
type fakeEncoder struct {
	id     string
	mu     sync.Mutex
	embeds []string
}

func (f *fakeEncoder) ID() string { return f.id }

func (f *fakeEncoder) EmbedPassage(_ context.Context, passage string) (float32, []byte, error) {
	f.mu.Lock()
	f.embeds = append(f.embeds, passage)
	f.mu.Unlock()
	sum := sha256.Sum256([]byte(passage))
	return 0.125, sum[:4], nil
}

// fakeSummarizer answers every module, until the day's requests are used up.
type fakeSummarizer struct {
	requests, limit int
}

func (f *fakeSummarizer) SummarizeModules(_ context.Context, modules []gemini.ModuleText) ([]gemini.ModuleSummary, error) {
	if f.requests == f.limit {
		return nil, gemini.ErrDailyLimit
	}
	f.requests++
	out := make([]gemini.ModuleSummary, len(modules))
	for i, m := range modules {
		out[i] = gemini.ModuleSummary{Key: m.Key, SummaryDE: "Über " + m.TitleDE + ".", SummaryEN: "About " + m.TitleDE + ".", Keywords: []string{"thema", "topic"}}
	}
	return out, nil
}

// The semantic stage runs after the export: what it computes, the next build publishes. With
// the day's Gemini requests used up it embeds the texts without a summary, and once the summary
// came, the passage with it; a vector is computed once per passage.
func TestTheSemanticStageFillsTheVectorsOfTheNextSnapshot(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	encoder := &fakeEncoder{id: "model-a"}
	summarizer := &fakeSummarizer{limit: 0}
	svc.cfg.Semantic = Semantic{Encoder: encoder, Summarizer: summarizer, SummaryModel: "gemini-test", Batch: 1, Budget: time.Minute, Workers: 2}
	ctx := context.Background()

	vectors := func() string { return column(t, svc.db, "SELECT COUNT(*) FROM module_vector") }
	modules := column(t, svc.db, "SELECT COUNT(*) FROM module")

	first := svc.RunCycle(ctx)
	if first.Result != "ok" || !first.Published || stage(first, "semantic").Error != "" {
		t.Fatalf("first cycle = %+v", first)
	}
	modules = column(t, svc.db, "SELECT COUNT(*) FROM module")
	if vectors() != "0" || len(encoder.embeds) == 0 {
		t.Fatalf("after the first cycle: %s vectors published, %d passages embedded; want 0 and some", vectors(), len(encoder.embeds))
	}
	for _, p := range encoder.embeds {
		if strings.Contains(p, "Über ") {
			t.Errorf("a summary was embedded although Gemini answered nothing: %q", p)
		}
	}

	// The next cycle publishes them, and computes nothing again.
	embedded := len(encoder.embeds)
	second := svc.RunCycle(ctx)
	if !second.Published || vectors() != modules {
		t.Errorf("second cycle: published %v, %s vectors for %s modules", second.Published, vectors(), modules)
	}
	if len(encoder.embeds) != embedded {
		t.Errorf("%d passages embedded again", len(encoder.embeds)-embedded)
	}

	// Gemini has requests again: the summaries come, then the passages with them, then the
	// snapshot.
	summarizer.limit = 100
	svc.RunCycle(ctx)
	if got := column(t, svc.db, "SELECT COUNT(*) FROM module_summary"); got == "0" {
		t.Fatal("no summary was stored")
	}
	withSummary := 0
	for _, p := range encoder.embeds[embedded:] {
		if strings.Contains(p, "Über ") {
			withSummary++
		}
	}
	if withSummary == 0 {
		t.Error("the passages with their summaries were not embedded")
	}
	fourth := svc.RunCycle(ctx)
	if !fourth.Published || vectors() != modules {
		t.Errorf("fourth cycle: published %v, %s vectors for %s modules", fourth.Published, vectors(), modules)
	}

	// A new model drops the old one's vectors: they are not comparable with its queries.
	svc.cfg.Semantic.Encoder = &fakeEncoder{id: "model-b"}
	svc.RunCycle(ctx)
	if got := column(t, svc.db, "SELECT COUNT(DISTINCT model) || ' ' || MIN(model) FROM passage_embedding"); got != "1 model-b" {
		t.Errorf("models of the stored vectors: %q, want only model-b", got)
	}
}

// Without an encoder the stage does not run; the cycle is not worse for it.
func TestTheSemanticStageNeedsAModel(t *testing.T) {
	svc, _ := newTestService(t, newFakeBTU(t))
	r := svc.RunCycle(context.Background())
	if r.Result != "ok" || stage(r, "semantic").Skipped == "" {
		t.Errorf("cycle = %+v", r)
	}
}

// With the real encoder (the crate as WebAssembly, internal/embed): the snapshot carries a vector
// of the model's length for every module. RADIX_TEST_EMBED_MODEL names the server's model.
func TestTheSemanticStageWithTheModel(t *testing.T) {
	path := os.Getenv("RADIX_TEST_EMBED_MODEL")
	if path == "" {
		t.Skip("RADIX_TEST_EMBED_MODEL is not set")
	}
	ctx := context.Background()
	encoder, err := embed.Load(ctx, path, 2)
	if err != nil {
		t.Fatal(err)
	}
	defer encoder.Close(ctx)
	svc, _ := newTestService(t, newFakeBTU(t))
	svc.cfg.Semantic = Semantic{Encoder: encoder, Budget: time.Minute, Workers: 2}
	svc.RunCycle(ctx)
	if r := svc.RunCycle(ctx); !r.Published {
		t.Fatalf("the second cycle published nothing: %+v", r)
	}
	modules := column(t, svc.db, "SELECT COUNT(*) FROM module")
	if got := column(t, svc.db, "SELECT COUNT(*) || ' ' || MIN(length(vector)) || ' ' || MAX(length(vector)) FROM v_module_vector"); got != modules+" 192 192" {
		t.Errorf("vectors: %q, want %s of 384 values in 192 bytes", got, modules)
	}
}
