package provider

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"

	"github.com/jakob/btu-scraper/internal/cache"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/parser"
	"github.com/jakob/btu-scraper/internal/storage"
)

const (
	EventProviderName   = "btu-event"
	DefaultEventTTL     = 3 * 24 * time.Hour
	DefaultQISBaseURL   = "https://www.b-tu.de/qisserver3/rds?state=verpublish&status=init&vmfile=no&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstaltung.veranstid=%s"
)

// BTUEventProvider scrapes event and schedule information from the BTU QIS portal.
type BTUEventProvider struct {
	client   *http.Client
	cache    cache.Cache
	cacheTTL time.Duration
	parser   *parser.EventParser
	storage  *storage.Storage
}

// NewBTUEventProvider creates a new BTUEventProvider.
func NewBTUEventProvider(
	storage *storage.Storage,
	c cache.Cache,
	ttl time.Duration,
) *BTUEventProvider {
	if ttl <= 0 {
		ttl = DefaultEventTTL
	}
	return &BTUEventProvider{
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
		cache:    c,
		cacheTTL: ttl,
		parser:   parser.NewEventParser(),
		storage:  storage,
	}
}

func (p *BTUEventProvider) Name() string {
	return EventProviderName
}

func (p *BTUEventProvider) Description() string {
	return "Scrapes course event/lecture details and schedules from the BTU QIS portal"
}

// FetchAndParseEvent fetches, parses, caches, and returns an event without writing to storage.
func (p *BTUEventProvider) FetchAndParseEvent(ctx context.Context, eventID, pageURL string, forceRefresh bool) (*model.EventDetail, error) {
	if pageURL == "" && eventID != "" {
		pageURL = fmt.Sprintf(DefaultQISBaseURL, eventID)
	}

	if eventID == "" {
		eventID = extractIDFromURL(pageURL)
	}

	cacheKey := fmt.Sprintf("event:%s", eventID)
	var htmlData []byte

	if !forceRefresh && p.cache != nil && eventID != "" {
		if cached, ok, err := p.cache.Get(cacheKey); err == nil && ok {
			htmlData = cached
		}
	}

	if htmlData == nil {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, pageURL, nil)
		if err != nil {
			return nil, fmt.Errorf("failed to create request for event %s: %w", eventID, err)
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)")

		resp, err := p.client.Do(req)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch event %s: %w", eventID, err)
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("unexpected status code %d for event %s", resp.StatusCode, eventID)
		}

		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return nil, fmt.Errorf("failed to read response body for event %s: %w", eventID, err)
		}
		htmlData = body

		if p.cache != nil && eventID != "" {
			_ = p.cache.Set(cacheKey, htmlData, p.cacheTTL)
		}
	}

	detail, err := p.parser.Parse(bytes.NewReader(htmlData), eventID, pageURL)
	if err != nil {
		return nil, fmt.Errorf("failed to parse event %s: %w", eventID, err)
	}

	return detail, nil
}

// ScrapeEvent fetches, parses, and persists an event in SQLite.
func (p *BTUEventProvider) ScrapeEvent(ctx context.Context, eventID, pageURL string, forceRefresh bool) (*model.EventDetail, error) {
	detail, err := p.FetchAndParseEvent(ctx, eventID, pageURL, forceRefresh)
	if err != nil {
		return nil, err
	}

	if p.storage != nil {
		if err := p.storage.UpsertEvent(detail); err != nil {
			return detail, fmt.Errorf("failed to store event %s: %w", detail.ID, err)
		}
	}

	return detail, nil
}

// ScrapeEventsForModule scrapes all current semester events associated with a module.
func (p *BTUEventProvider) ScrapeEventsForModule(ctx context.Context, moduleID string, forceRefresh bool) (int, error) {
	if p.storage == nil {
		return 0, fmt.Errorf("storage not configured")
	}

	mod, err := p.storage.GetModule(moduleID)
	if err != nil {
		return 0, fmt.Errorf("failed to get module %s: %w", moduleID, err)
	}

	if len(mod.CurrentSemesterEvents) == 0 {
		return 0, nil
	}

	scrapedCount := 0
	for i, evt := range mod.CurrentSemesterEvents {
		if evt.URL == "" {
			continue
		}

		if i > 0 {
			time.Sleep(800 * time.Millisecond) // Polite delay for QIS
		}

		evtID := extractIDFromURL(evt.URL)
		eventDetail, err := p.ScrapeEvent(ctx, evtID, evt.URL, forceRefresh)
		if err != nil {
			continue
		}

		// Ensure link in module_events table
		_ = p.storage.LinkModuleEvent(moduleID, eventDetail.ID)
		scrapedCount++
	}

	return scrapedCount, nil
}

func extractIDFromURL(rawURL string) string {
	u, err := url.Parse(rawURL)
	if err != nil {
		return ""
	}
	if v := u.Query().Get("veranstaltung.veranstid"); v != "" {
		return v
	}
	return u.Query().Get("veranstid")
}
