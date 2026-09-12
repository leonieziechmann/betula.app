package analytics

import (
	"os"
	"testing"
	"time"
)

func TestAnalyticsTracker(t *testing.T) {
	tmpDB := "test_analytics.db"
	defer os.Remove(tmpDB)

	tracker, err := NewTracker(tmpDB)
	if err != nil {
		t.Fatalf("failed to create tracker: %v", err)
	}
	defer tracker.Close()

	// Track page views
	tracker.TrackPageView()
	tracker.TrackPageView()

	// Track module clicks
	tracker.TrackModuleClick("11101")
	tracker.TrackModuleClick("11101")
	tracker.TrackModuleClick("22202")

	// Track program selections
	tracker.TrackProgramSelect("inf-bsc", "Informatik (B.Sc.)")
	tracker.TrackProgramSelect("inf-bsc", "Informatik (B.Sc.)")
	tracker.TrackProgramSelect("wi-msc", "Wirtschaftsingenieurwesen (M.Sc.)")

	// Flush to database synchronously
	tracker.Flush()

	summary, err := tracker.GetSummary()
	if err != nil {
		t.Fatalf("failed to get summary: %v", err)
	}

	if summary.TotalPageViews != 2 {
		t.Errorf("expected 2 page views, got %d", summary.TotalPageViews)
	}
	if summary.TotalModuleClicks != 3 {
		t.Errorf("expected 3 module clicks, got %d", summary.TotalModuleClicks)
	}
	if summary.TotalProgramSelect != 3 {
		t.Errorf("expected 3 program selections, got %d", summary.TotalProgramSelect)
	}

	if len(summary.TopModules) != 2 {
		t.Fatalf("expected 2 top modules, got %d", len(summary.TopModules))
	}
	if summary.TopModules[0].ModuleID != "11101" || summary.TopModules[0].ClickCount != 2 {
		t.Errorf("expected module 11101 with 2 clicks, got %+v", summary.TopModules[0])
	}

	if len(summary.TopPrograms) != 2 {
		t.Fatalf("expected 2 top programs, got %d", len(summary.TopPrograms))
	}
	if summary.TopPrograms[0].ProgramID != "inf-bsc" || summary.TopPrograms[0].SelectCount != 2 {
		t.Errorf("expected program inf-bsc with 2 selections, got %+v", summary.TopPrograms[0])
	}

	// Verify daily views
	today := time.Now().Format("2006-01-02")
	if len(summary.DailyViews) == 0 || summary.DailyViews[0].Date != today || summary.DailyViews[0].Count != 2 {
		t.Errorf("expected today's daily views to be 2, got %+v", summary.DailyViews)
	}
}
