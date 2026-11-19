package service

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/history"
)

// With a history, every cycle records the published snapshot when it is new: the first cycle's,
// none for a cycle that publishes nothing, the next one's with what changed.
func TestTheHistoryRecordsWhatIsPublished(t *testing.T) {
	site := newFakeBTU(t)
	svc, _ := newTestService(t, site)
	svc.cfg.History = filepath.Join(t.TempDir(), "history.db")
	ctx := context.Background()

	states := func() int {
		t.Helper()
		h, err := history.Open(svc.cfg.History)
		if err != nil {
			t.Fatal(err)
		}
		defer h.Close()
		st, err := h.Status(ctx)
		if err != nil {
			t.Fatal(err)
		}
		return st.Snapshots
	}
	first := svc.RunCycle(ctx)
	if first.Result != "ok" || stage(first, "history").Error != "" || stage(first, "history").Skipped != "" {
		t.Fatalf("first cycle: %+v, history stage %+v", first.Result, stage(first, "history"))
	}
	if n := states(); n != 1 {
		t.Fatalf("%d states after the first cycle", n)
	}
	if second := svc.RunCycle(ctx); second.Published || states() != 1 {
		t.Fatalf("a cycle that published nothing recorded a state: %d", states())
	}

	site.mu.Lock()
	site.credits = "9"
	site.mu.Unlock()
	if third := svc.RunCycle(ctx); !third.Published || stage(third, "history").Error != "" {
		t.Fatalf("third cycle: %+v", stage(third, "history"))
	}
	if n := states(); n != 2 {
		t.Fatalf("%d states after the change", n)
	}
	h, err := history.Open(svc.cfg.History)
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	versions, err := h.Log(ctx, "module", map[string]string{"id": "11101"})
	if err != nil {
		t.Fatal(err)
	}
	if len(versions) != 2 || !slices.Contains(versions[1].Changed, "credits") || versions[1].Values["credits"] != float64(9) && versions[1].Values["credits"] != int64(9) {
		t.Errorf("versions of module 11101: %+v", versions)
	}

	// The stage fails in the cycle that publishes the next snapshot; the cycle after records it
	// (its file is kept beside the current one) before the one it publishes itself.
	good := svc.cfg.History
	svc.cfg.History = filepath.Join(t.TempDir(), "not-a-directory")
	if err := os.WriteFile(svc.cfg.History, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	svc.cfg.History = filepath.Join(svc.cfg.History, "history.db")
	site.mu.Lock()
	site.credits = "10"
	site.mu.Unlock()
	if fourth := svc.RunCycle(ctx); !fourth.Published || stage(fourth, "history").Error == "" || fourth.Result != "degraded" {
		t.Fatalf("fourth cycle: %s, history stage %+v", fourth.Result, stage(fourth, "history"))
	}
	svc.cfg.History = good
	site.mu.Lock()
	site.credits = "11"
	site.mu.Unlock()
	if fifth := svc.RunCycle(ctx); !fifth.Published || stage(fifth, "history").Error != "" {
		t.Fatalf("fifth cycle: %+v", stage(fifth, "history"))
	}
	if n := states(); n != 4 {
		t.Fatalf("%d states: the snapshot the failed stage missed was not recorded", n)
	}
	versions, err = h.Log(ctx, "module", map[string]string{"id": "11101"})
	if err != nil {
		t.Fatal(err)
	}
	var credits []string
	for _, v := range versions {
		credits = append(credits, fmt.Sprint(v.Values["credits"]))
	}
	if strings.Join(credits, ",") != "8,9,10,11" {
		t.Errorf("credits of module 11101 through the states: %v", credits)
	}

	svc.cfg.History = ""
	if s := stage(svc.RunCycle(ctx), "history"); s.Skipped != "no history" {
		t.Errorf("without a history: %+v", s)
	}
}
