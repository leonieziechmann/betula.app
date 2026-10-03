package catalogbuild

import (
	"context"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/semantic"
)

// A module gets the vector of its passage of this build: with the summary of its text when
// there is one, else that of the text alone until the passage with the summary has one, and
// none when the semantic stage has computed neither. The vectors are content: a new one changes
// the digest, so the next export carries it.
func TestModuleVectorsFollowTheirPassage(t *testing.T) {
	db, report := buildFixture(t)
	if report.ModuleVectors != 0 {
		t.Fatalf("%d vectors before any was computed", report.ModuleVectors)
	}
	modules, err := db.SemanticModules()
	if err != nil {
		t.Fatal(err)
	}
	var m catalogdb.SemanticModule
	for _, x := range modules {
		if x.ID == "11881" {
			m = x
		}
	}
	if m.ID == "" {
		t.Fatal("module 11881 is not in the fixture")
	}
	text := semantic.Text{TitleDE: m.TitleDE, TitleEN: m.TitleEN, Contents: m.Contents, Outcomes: m.Outcomes}
	plain := semantic.PassageHash(semantic.Passage(text, nil))
	if err := db.SavePassageEmbeddings("model-a", []catalogdb.PassageEmbedding{{PassageHash: plain, Scale: 0.5, Vector: []byte{0x91, 0xa8}}}); err != nil {
		t.Fatal(err)
	}

	rebuild := func() *Report {
		t.Helper()
		r, err := Build(context.Background(), db)
		if err != nil {
			t.Fatal(err)
		}
		return r
	}
	r := rebuild()
	if r.ModuleVectors != 1 || !r.ContentChanged {
		t.Errorf("a computed vector: %d vectors, content changed %v; want 1, true", r.ModuleVectors, r.ContentChanged)
	}
	want(t, db, "SELECT module_id, scale, hex(vector) FROM v_module_vector", "11881|0.5|91A8")
	// The snapshot names the model of its vectors: the browser offers the semantic search only
	// with the query model made for it.
	want(t, db, "SELECT value FROM meta WHERE key = 'semantic_model'", "model-a")

	// A summary makes another passage, whose vector is not computed yet: the module keeps the one
	// of its text alone, and the snapshot does not change.
	summary := catalogdb.ModuleSummary{TextHash: text.Hash(), SummaryDE: "Daten auswerten.", SummaryEN: "Analysing data.",
		Keywords: []string{"data mining"}, Model: "gemini-test", CreatedAt: time.Now()}
	if err := db.SaveModuleSummaries([]catalogdb.ModuleSummary{summary}); err != nil {
		t.Fatal(err)
	}
	if r := rebuild(); r.ModuleVectors != 1 || r.ContentChanged {
		t.Errorf("a summary without its vector: %d vectors, content changed %v; want 1, false", r.ModuleVectors, r.ContentChanged)
	}
	want(t, db, "SELECT module_id, scale, hex(vector) FROM v_module_vector", "11881|0.5|91A8")
	withSummary := semantic.PassageHash(semantic.Passage(text, &semantic.Summary{DE: summary.SummaryDE, EN: summary.SummaryEN, Keywords: summary.Keywords}))
	if err := db.SavePassageEmbeddings("model-a", []catalogdb.PassageEmbedding{{PassageHash: withSummary, Scale: 0.25, Vector: []byte{0xc7, 0x8e}}}); err != nil {
		t.Fatal(err)
	}
	rebuild()
	want(t, db, "SELECT module_id, scale, hex(vector) FROM v_module_vector", "11881|0.25|C78E")

	// The caches stay through builds, and the next model's vectors replace the old ones.
	if dropped, err := db.DropPassageEmbeddingsExcept("model-b"); err != nil || dropped != 2 {
		t.Errorf("DropPassageEmbeddingsExcept: %d, %v; want 2 dropped", dropped, err)
	}
	if r := rebuild(); r.ModuleVectors != 0 || !r.ContentChanged {
		t.Errorf("after the model changed: %d vectors, content changed %v; want 0, true", r.ModuleVectors, r.ContentChanged)
	}
	want(t, db, "SELECT COUNT(*) FROM module_summary", "1")
	want(t, db, "SELECT COUNT(*) FROM meta WHERE key = 'semantic_model'", "0")
	if err := db.SavePassageEmbeddings("model-b", []catalogdb.PassageEmbedding{{PassageHash: plain, Scale: 0.5, Vector: []byte{0x91, 0xa8}}}); err != nil {
		t.Fatal(err)
	}
	rebuild()
	want(t, db, "SELECT value FROM meta WHERE key = 'semantic_model'", "model-b")
}
