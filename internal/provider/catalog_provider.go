package provider

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/jakob/btu-scraper/internal/cache"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/parser"
	"github.com/jakob/btu-scraper/internal/storage"
)

const (
	CatalogProviderName = "btu-catalog"
	DefaultCatalogURL   = "https://www.b-tu.de/modul"
	DefaultCatalogTTL   = 24 * time.Hour
)

// BTUModuleCatalogProvider scrapes the BTU module catalog list.
type BTUModuleCatalogProvider struct {
	targetURL string
	client    *http.Client
	cache     cache.Cache
	cacheTTL  time.Duration
	parser    *parser.CatalogParser
	storage   *storage.Storage
}

// NewBTUModuleCatalogProvider creates a new catalog provider.
func NewBTUModuleCatalogProvider(
	storage *storage.Storage,
	c cache.Cache,
	targetURL string,
	ttl time.Duration,
) *BTUModuleCatalogProvider {
	if targetURL == "" {
		targetURL = DefaultCatalogURL
	}
	if ttl <= 0 {
		ttl = DefaultCatalogTTL
	}
	return &BTUModuleCatalogProvider{
		targetURL: targetURL,
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
		cache:    c,
		cacheTTL: ttl,
		parser:   parser.NewCatalogParser("https://www.b-tu.de"),
		storage:  storage,
	}
}

func (p *BTUModuleCatalogProvider) Name() string {
	return CatalogProviderName
}

func (p *BTUModuleCatalogProvider) Description() string {
	return "Discovers all available course modules from b-tu.de/modul"
}

// ScrapeCatalog fetches the catalog page, extracts module summaries, and stores them in SQLite.
func (p *BTUModuleCatalogProvider) ScrapeCatalog(ctx context.Context, forceRefresh bool) (int, error) {
	cacheKey := "catalog:list"
	var htmlData []byte

	if !forceRefresh && p.cache != nil {
		if cached, ok, err := p.cache.Get(cacheKey); err == nil && ok {
			htmlData = cached
		}
	}

	if htmlData == nil {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, p.targetURL, nil)
		if err != nil {
			return 0, fmt.Errorf("failed to create request: %w", err)
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)")

		resp, err := p.client.Do(req)
		if err != nil {
			return 0, fmt.Errorf("failed to fetch catalog URL %s: %w", p.targetURL, err)
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			return 0, fmt.Errorf("unexpected status code from %s: %d", p.targetURL, resp.StatusCode)
		}

		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return 0, fmt.Errorf("failed to read response body: %w", err)
		}
		htmlData = body

		if p.cache != nil {
			_ = p.cache.Set(cacheKey, htmlData, p.cacheTTL)
		}
	}

	summaries, err := p.parser.Parse(bytes.NewReader(htmlData))
	if err != nil {
		return 0, fmt.Errorf("failed to parse catalog HTML: %w", err)
	}

	if p.storage != nil && len(summaries) > 0 {
		if err := p.storage.UpsertCatalog(summaries); err != nil {
			return len(summaries), fmt.Errorf("failed to store catalog in database: %w", err)
		}
	}

	return len(summaries), nil
}

// ParseOnly parses raw HTML without fetching or storing, useful for testing.
func (p *BTUModuleCatalogProvider) ParseOnly(r io.Reader) ([]model.ModuleSummary, error) {
	return p.parser.Parse(r)
}
