package provider

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/cache"
	"github.com/leonieziechmann/btu-scraper/internal/model"
	"github.com/leonieziechmann/btu-scraper/internal/parser"
	"github.com/leonieziechmann/btu-scraper/internal/storage"
)

const (
	FUESProviderName = "btu-fues"
	DefaultFUESURL   = "https://www.b-tu.de/qisserver3/rds?state=change&type=3&moduleParameter=pordpos&nextdir=change&next=TableSelectModul.vm&subdir=pord&P_start=0&P_anzahl=9999&missing=FUES"
	DefaultFUESTTL   = 7 * 24 * time.Hour
)

// BTUFUESProvider scrapes the approved Fachübergreifendes Studium module list.
type BTUFUESProvider struct {
	targetURL string
	client    *http.Client
	cache     cache.Cache
	cacheTTL  time.Duration
	parser    *parser.FUESParser
	storage   *storage.Storage
}

// NewBTUFUESProvider creates a new BTUFUESProvider.
func NewBTUFUESProvider(
	storage *storage.Storage,
	c cache.Cache,
	targetURL string,
	ttl time.Duration,
) *BTUFUESProvider {
	if targetURL == "" {
		targetURL = DefaultFUESURL
	}
	if ttl <= 0 {
		ttl = DefaultFUESTTL
	}
	return &BTUFUESProvider{
		targetURL: targetURL,
		client: &http.Client{
			Timeout: 45 * time.Second,
		},
		cache:    c,
		cacheTTL: ttl,
		parser:   parser.NewFUESParser(),
		storage:  storage,
	}
}

func (p *BTUFUESProvider) Name() string {
	return FUESProviderName
}

func (p *BTUFUESProvider) Description() string {
	return "Scrapes official Fachübergreifendes Studium (FÜS) module catalog from the BTU QIS portal"
}

// ScrapeFUES fetches, caches, parses, and persists all approved FÜS modules.
func (p *BTUFUESProvider) ScrapeFUES(ctx context.Context, forceRefresh bool) ([]model.FUESModule, error) {
	cacheKey := "fues:catalog"
	var htmlData []byte

	if !forceRefresh && p.cache != nil {
		if cached, ok, err := p.cache.Get(cacheKey); err == nil && ok {
			htmlData = cached
		}
	}

	if htmlData == nil {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, p.targetURL, nil)
		if err != nil {
			return nil, fmt.Errorf("failed to create request for FÜS: %w", err)
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)")

		resp, err := p.client.Do(req)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch FÜS URL %s: %w", p.targetURL, err)
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("unexpected status code %d from %s", resp.StatusCode, p.targetURL)
		}

		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return nil, fmt.Errorf("failed to read response body: %w", err)
		}
		htmlData = body

		if p.cache != nil {
			_ = p.cache.Set(cacheKey, htmlData, p.cacheTTL)
		}
	}

	modules, err := p.parser.Parse(bytes.NewReader(htmlData))
	if err != nil {
		return nil, fmt.Errorf("failed to parse FÜS HTML: %w", err)
	}

	if p.storage != nil && len(modules) > 0 {
		if err := p.storage.UpsertFUESList(modules); err != nil {
			return modules, fmt.Errorf("failed to store FÜS modules in database: %w", err)
		}
	}

	return modules, nil
}
