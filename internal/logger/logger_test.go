package logger

import (
	"errors"
	"os"
	"testing"
	"time"
)

func TestLoggerRingBuffer(t *testing.T) {
	rb := NewRingBuffer(5)
	for i := 1; i <= 7; i++ {
		rb.Add(LogEntry{
			Timestamp: time.Now(),
			Level:     "INFO",
			Component: "TEST",
			Message:   "msg",
		})
	}

	entries := rb.GetAll(10, LevelInfo)
	if len(entries) != 5 {
		t.Fatalf("expected 5 entries, got %d", len(entries))
	}
}

func TestLoggerAlert(t *testing.T) {
	tmpFile := "test_logger.log"
	defer os.Remove(tmpFile)

	l, err := NewLogger(Options{
		MinLevel:   LevelDebug,
		FilePath:   tmpFile,
		BufferSize: 50,
	})
	if err != nil {
		t.Fatalf("failed to create logger: %v", err)
	}
	defer l.Close()

	alertFired := false
	l.SetAlertCallback(func(alert string) {
		alertFired = true
	})

	errDummy := errors.New("simulated network error")
	for i := 0; i < 5; i++ {
		l.RecordScrapeFailure("SCRAPER", "http://example.com", errDummy)
	}

	if !alertFired {
		t.Errorf("expected alert to fire after 5 consecutive failures")
	}

	stats := l.GetHealthStats()
	if stats["consecutive_scrape_errors"].(uint64) != 5 {
		t.Errorf("expected 5 consecutive scrape errors, got %v", stats["consecutive_scrape_errors"])
	}

	l.RecordScrapeSuccess()
	stats = l.GetHealthStats()
	if stats["consecutive_scrape_errors"].(uint64) != 0 {
		t.Errorf("expected consecutive errors reset to 0, got %v", stats["consecutive_scrape_errors"])
	}
}
