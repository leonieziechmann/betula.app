package provider

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/jakob/btu-scraper/internal/cache"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/parser"
	"github.com/jakob/btu-scraper/internal/storage"
)

const (
	DetailProviderName   = "btu-detail"
	DefaultDetailURLTmpl = "https://www.b-tu.de/modul/%s"
	DefaultDetailTTL     = 7 * 24 * time.Hour
)

var (
	ErrModuleNotFound = errors.New("module not found on BTU website")
)

// BTUModuleDetailProvider scrapes individual course module detail pages.
type BTUModuleDetailProvider struct {
	urlTemplate          string
	client               *http.Client
	cache                cache.Cache
	cacheTTL             time.Duration
	parser               *parser.DetailParser
	storage              *storage.Storage
	treeProvider         *BTUProgramTreeProvider
	unresolvablePrograms map[string]bool
	unresMu              sync.RWMutex
}

// NewBTUModuleDetailProvider creates a new detail provider.
func NewBTUModuleDetailProvider(
	storage *storage.Storage,
	c cache.Cache,
	urlTemplate string,
	ttl time.Duration,
) *BTUModuleDetailProvider {
	if urlTemplate == "" {
		urlTemplate = DefaultDetailURLTmpl
	}
	if ttl <= 0 {
		ttl = DefaultDetailTTL
	}
	return &BTUModuleDetailProvider{
		urlTemplate: urlTemplate,
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
		cache:                c,
		cacheTTL:             ttl,
		parser:               parser.NewDetailParser(),
		storage:              storage,
		unresolvablePrograms: make(map[string]bool),
	}
}

// SetProgramTreeProvider sets the optional study program tree provider for on-demand QIS lookups.
func (p *BTUModuleDetailProvider) SetProgramTreeProvider(tp *BTUProgramTreeProvider) {
	p.treeProvider = tp
}

func (p *BTUModuleDetailProvider) Name() string {
	return DetailProviderName
}

func (p *BTUModuleDetailProvider) Description() string {
	return "Scrapes comprehensive course details for a specific module ID from b-tu.de/modul/<id>"
}

// FetchAndParseModule fetches, parses, caches, and returns the module detail without necessarily storing it.
func (p *BTUModuleDetailProvider) FetchAndParseModule(ctx context.Context, moduleID string, forceRefresh bool) (*model.ModuleDetail, error) {
	cacheKey := fmt.Sprintf("module:%s", moduleID)
	pageURL := fmt.Sprintf(p.urlTemplate, moduleID)
	var htmlData []byte

	if !forceRefresh && p.cache != nil {
		if cached, ok, err := p.cache.Get(cacheKey); err == nil && ok {
			htmlData = cached
		}
	}

	if htmlData == nil {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, pageURL, nil)
		if err != nil {
			return nil, fmt.Errorf("failed to create request for module %s: %w", moduleID, err)
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; BTU-Student-Scraper/1.0)")

		resp, err := p.client.Do(req)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch module %s: %w", moduleID, err)
		}
		defer resp.Body.Close()

		if resp.StatusCode == http.StatusNotFound {
			return nil, fmt.Errorf("%w (%s)", ErrModuleNotFound, moduleID)
		}
		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("unexpected status %d for module %s", resp.StatusCode, moduleID)
		}

		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return nil, fmt.Errorf("failed to read module response body: %w", err)
		}
		htmlData = body

		if p.cache != nil {
			_ = p.cache.Set(cacheKey, htmlData, p.cacheTTL)
		}
	}

	detail, err := p.parser.Parse(bytes.NewReader(htmlData), moduleID, pageURL)
	if err != nil {
		return nil, fmt.Errorf("failed to parse module %s: %w", moduleID, err)
	}

	return detail, nil
}

// ScrapeModule fetches, parses, and upserts a module detail into storage.
func (p *BTUModuleDetailProvider) ScrapeModule(ctx context.Context, moduleID string, forceRefresh bool) error {
	detail, err := p.FetchAndParseModule(ctx, moduleID, forceRefresh)
	if err != nil {
		return err
	}

	if p.storage != nil {
		// Resolve study programs against official study programs, falling back to QIS on demand
		detail.StudyPrograms = p.resolveAndLinkStudyPrograms(ctx, detail.ID, detail.StudyPrograms)

		if err := p.storage.UpsertModuleDetail(detail); err != nil {
			return fmt.Errorf("failed to store module %s: %w", moduleID, err)
		}
	}

	return nil
}

// resolveAndLinkStudyPrograms matches module study programs with official study programs.
// If an official program is not in SQLite, it triggers a single gentle QIS fetch.
// If unresolvable, it gracefully leaves the entry unlinked.
func (p *BTUModuleDetailProvider) resolveAndLinkStudyPrograms(ctx context.Context, moduleID string, programs []model.StudyProgram) []model.StudyProgram {
	if p.storage == nil || len(programs) == 0 {
		return programs
	}

	for i := range programs {
		sp := &programs[i]
		progName := strings.TrimSpace(sp.Program)
		if progName == "" || strings.EqualFold(progName, "keine Zuordnung vorhanden") {
			continue
		}

		// 1. Try finding in SQLite first
		official, err := p.storage.FindOfficialProgram(progName, sp.Degree, sp.Regulation)
		if err != nil && p.treeProvider != nil {
			// Check if we already tried and failed to find this program in QIS
			p.unresMu.RLock()
			unres := p.unresolvablePrograms[progName]
			p.unresMu.RUnlock()

			if !unres {
				// 2. Not in DB: perform single gentle QIS fetch for this specific study program
				results, scrapeErr := p.treeProvider.ScrapeProgramTree(ctx, progName, false, 1, 1000, false)
				if scrapeErr == nil && len(results) > 0 {
					// Query SQLite again after QIS results were persisted
					official, err = p.storage.FindOfficialProgram(progName, sp.Degree, sp.Regulation)
				} else {
					// Mark as unresolvable so we do not query QIS again on subsequent modules
					p.unresMu.Lock()
					p.unresolvablePrograms[progName] = true
					p.unresMu.Unlock()
				}
			}
		}

		// 3. If found, link in SQLite and set OfficialProgramID
		if err == nil && official != nil {
			sp.OfficialProgramID = official.ID
			_ = p.storage.LinkModuleToStudyProgram(moduleID, official.ID, official.ProgramName, sp.Degree, sp.Regulation)
		}
		// 4. If not found in QIS: gracefully ignored per user requirement!
	}

	return programs
}
