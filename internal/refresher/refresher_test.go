package refresher

import (
	"testing"
	"time"
)

func TestRefresherOffPeak(t *testing.T) {
	cfg := Config{
		OffPeakStartHour: 1,
		OffPeakEndHour:   6,
	}
	r := &Refresher{config: cfg}

	nowHour := time.Now().Hour()
	expected := nowHour >= 1 && nowHour < 6
	if r.IsOffPeak() != expected {
		t.Errorf("expected IsOffPeak to be %v at hour %d, got %v", expected, nowHour, r.IsOffPeak())
	}
}

func TestRefresherQueue(t *testing.T) {
	cfg := DefaultConfig()
	r := NewRefresher(cfg, nil, nil, nil, nil, nil)

	r.EnqueueCacheMiss("11101")
	r.EnqueueCacheMiss("11101") // duplicate should be ignored
	r.EnqueueCacheMiss("22202")

	status := r.GetStatus()
	if status.PriorityQueueLen != 2 {
		t.Errorf("expected 2 items in priority queue, got %d", status.PriorityQueueLen)
	}
}

func TestRefresherBackoff(t *testing.T) {
	cfg := DefaultConfig()
	r := NewRefresher(cfg, nil, nil, nil, nil, nil)

	if r.isBackingOff() {
		t.Errorf("expected initially not backing off")
	}

	r.triggerBackoff(2 * time.Second)
	if !r.isBackingOff() {
		t.Errorf("expected isBackingOff to be true")
	}

	status := r.GetStatus()
	if !status.BackoffUntil.After(time.Now()) {
		t.Errorf("expected BackoffUntil to be in future, got %v", status.BackoffUntil)
	}
}

func TestRefresherEventQueue(t *testing.T) {
	cfg := DefaultConfig()
	r := NewRefresher(cfg, nil, nil, nil, nil, nil)

	r.EnqueueEvent(EventScrapeJob{EventID: "101", PageURL: "https://example.com/101", Title: "Vorlesung A"})
	r.EnqueueEvent(EventScrapeJob{EventID: "101", PageURL: "https://example.com/101", Title: "Vorlesung A"}) // duplicate ignored
	r.EnqueueEvent(EventScrapeJob{EventID: "102", PageURL: "https://example.com/102", Title: "Übung B"})

	status := r.GetStatus()
	if status.EventQueueLen != 2 {
		t.Errorf("expected 2 items in event queue, got %d", status.EventQueueLen)
	}
}

