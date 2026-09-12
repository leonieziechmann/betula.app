package provider

import (
	"bytes"
	"context"
	"crypto/sha1"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/jakob/btu-scraper/internal/cache"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/parser"
	"github.com/jakob/btu-scraper/internal/storage"
)

const (
	ProgramTreeProviderName = "btu-programs"
	DefaultProgramTreeURL   = "https://www.b-tu.de/qisserver3/rds?state=modulBeschrGast&moduleParameter=modDescr&next=tree.vm&nextdir=qispos/modulBeschr/gast&nodeID=auswahlBaum&navigationPosition=modules%2CmodulBeschrGast&breadcrumb=modDescrViewOnly2&topitem=modules&subitem=modulBeschrGast&asi="
	DefaultProgramTreeTTL   = 7 * 24 * time.Hour
)

// BTUProgramTreeProvider navigates the official BTU study programs tree in QIS
// and extracts all study programs, degrees, PO versions, statutes, and amendments.
type BTUProgramTreeProvider struct {
	targetURL   string
	client      *http.Client
	cache       cache.Cache
	cacheTTL    time.Duration
	parser      *parser.ProgramTreeParser
	storage     *storage.Storage
	downloadDir string
}

// NewBTUProgramTreeProvider creates a new BTUProgramTreeProvider.
func NewBTUProgramTreeProvider(
	storage *storage.Storage,
	c cache.Cache,
	targetURL string,
	ttl time.Duration,
	downloadDir string,
) *BTUProgramTreeProvider {
	if targetURL == "" {
		targetURL = DefaultProgramTreeURL
	}
	if ttl <= 0 {
		ttl = DefaultProgramTreeTTL
	}
	if downloadDir == "" {
		downloadDir = "statutes"
	}
	return &BTUProgramTreeProvider{
		targetURL: targetURL,
		client: &http.Client{
			Timeout: 45 * time.Second,
		},
		cache:       c,
		cacheTTL:    ttl,
		parser:      parser.NewProgramTreeParser(),
		storage:     storage,
		downloadDir: downloadDir,
	}
}

func (p *BTUProgramTreeProvider) Name() string {
	return ProgramTreeProviderName
}

func (p *BTUProgramTreeProvider) Description() string {
	return "Traverses official study programs tree and extracts degree regulations, statutes, and amendments from QIS and OPUS 4"
}

// fetchHTML fetches a page from cache or network.
func (p *BTUProgramTreeProvider) fetchHTML(ctx context.Context, pageURL string, forceRefresh bool) ([]byte, error) {
	h := sha1.Sum([]byte(pageURL))
	cacheKey := fmt.Sprintf("qis:tree:%s", hex.EncodeToString(h[:]))

	if !forceRefresh && p.cache != nil {
		if cached, ok, err := p.cache.Get(cacheKey); err == nil && ok {
			return cached, nil
		}
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, pageURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request for %s: %w", pageURL, err)
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")

	resp, err := p.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch %s: %w", pageURL, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status code %d from %s", resp.StatusCode, pageURL)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response body: %w", err)
	}

	if p.cache != nil {
		_ = p.cache.Set(cacheKey, body, p.cacheTTL)
	}

	return body, nil
}

// ScrapeProgramTree traverses the QIS study programs tree, parses degrees, PO versions, and document links.
func (p *BTUProgramTreeProvider) ScrapeProgramTree(
	ctx context.Context,
	programFilter string,
	downloadPDFs bool,
	workers int,
	delayMs int,
	forceRefresh bool,
) ([]model.OfficialStudyProgram, error) {
	if workers <= 0 {
		workers = 1 // Very gentle: single worker by default for sensitive QIS servers
	}
	if delayMs <= 0 {
		delayMs = 1000 // 1 second polite pause between requests by default
	}

	rootHTML, err := p.fetchHTML(ctx, p.targetURL, forceRefresh)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch root program tree: %w", err)
	}

	programs, err := p.parser.ParseRootPrograms(bytes.NewReader(rootHTML), "https://www.b-tu.de")
	if err != nil {
		return nil, fmt.Errorf("failed to parse root programs: %w", err)
	}

	var targetPrograms []parser.ProgramNode
	normFilter := strings.ToLower(strings.TrimSpace(programFilter))
	for _, prog := range programs {
		if normFilter == "" || strings.Contains(strings.ToLower(prog.Name), normFilter) {
			targetPrograms = append(targetPrograms, prog)
		}
	}

	total := len(targetPrograms)
	jobs := make(chan parser.ProgramNode, total)
	for _, prog := range targetPrograms {
		jobs <- prog
	}
	close(jobs)

	var mu sync.Mutex
	var allResults []model.OfficialStudyProgram
	var completed uint64

	var wg sync.WaitGroup
	for w := 1; w <= workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				select {
				case <-ctx.Done():
					return
				case prog, ok := <-jobs:
					if !ok {
						return
					}

					progs := p.processStudyProgram(ctx, prog, downloadPDFs, delayMs, forceRefresh)
					if len(progs) > 0 {
						mu.Lock()
						allResults = append(allResults, progs...)
						mu.Unlock()

						if p.storage != nil {
							_ = p.storage.UpsertOfficialPrograms(progs)
						}
					}

					curr := atomic.AddUint64(&completed, 1)
					if curr%5 == 0 || curr == uint64(total) {
						fmt.Printf("[Programs] %d/%d (%.1f%%) study programs processed\n",
							curr, total, float64(curr)/float64(total)*100)
					}

					if delayMs > 0 {
						time.Sleep(time.Duration(delayMs) * time.Millisecond)
					}
				}
			}
		}()
	}

	wg.Wait()
	return allResults, nil
}

// processStudyProgram processes a single study program node down to its PO versions.
func (p *BTUProgramTreeProvider) processStudyProgram(
	ctx context.Context,
	prog parser.ProgramNode,
	downloadPDFs bool,
	delayMs int,
	forceRefresh bool,
) []model.OfficialStudyProgram {
	stgHTML, err := p.fetchHTML(ctx, prog.URL, forceRefresh)
	if err != nil {
		return nil
	}

	degrees, err := p.parser.ParseDegrees(bytes.NewReader(stgHTML), "https://www.b-tu.de")
	if err != nil || len(degrees) == 0 {
		return nil
	}

	var results []model.OfficialStudyProgram

	for _, deg := range degrees {
		if delayMs > 0 {
			time.Sleep(time.Duration(delayMs/2) * time.Millisecond)
		}

		degHTML, err := p.fetchHTML(ctx, deg.URL, forceRefresh)
		if err != nil {
			continue
		}

		pos, err := p.parser.ParsePOVersions(bytes.NewReader(degHTML), "https://www.b-tu.de")
		if err != nil || len(pos) == 0 {
			continue
		}

		for _, po := range pos {
			if delayMs > 0 {
				time.Sleep(time.Duration(delayMs/2) * time.Millisecond)
			}

			poHTML, err := p.fetchHTML(ctx, po.URL, forceRefresh)
			if err != nil {
				continue
			}

			docs, err := p.parser.ParsePODocuments(bytes.NewReader(poHTML), "https://www.b-tu.de")
			if err != nil {
				docs = []model.ProgramRegulationDocument{}
			}

			// Clean and process documents
			for i := range docs {
				if downloadPDFs {
					p.processDocumentDownload(ctx, &docs[i], prog.Name)
				}
			}

			id := fmt.Sprintf("stg_%s_abschl_%s_po_%s",
				cleanSlug(prog.Code, prog.Name),
				cleanSlug(deg.Code, deg.Degree),
				cleanSlug(po.POVersion, po.POVersion),
			)

			results = append(results, model.OfficialStudyProgram{
				ID:          id,
				ProgramName: prog.Name,
				ProgramCode: prog.Code,
				Degree:      deg.Degree,
				DegreeCode:  deg.Code,
				POVersion:   po.POVersion,
				QISNodeID:   po.NodeID,
				QISURL:      po.URL,
				Documents:   docs,
				ScrapedAt:   time.Now().UTC(),
			})
		}
	}

	return results
}

// processDocumentDownload attempts to download a statute or amendment, checking for bot challenge.
func (p *BTUProgramTreeProvider) processDocumentDownload(ctx context.Context, doc *model.ProgramRegulationDocument, programName string) {
	if doc.URL == "" {
		return
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, doc.URL, nil)
	if err != nil {
		doc.DownloadStatus = "error"
		return
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")

	resp, err := p.client.Do(req)
	if err != nil {
		doc.DownloadStatus = "error"
		return
	}
	defer resp.Body.Close()

	contentType := strings.ToLower(resp.Header.Get("Content-Type"))

	// Read initial header bytes (up to 4KB) to inspect content
	preview := make([]byte, 4096)
	n, _ := io.ReadFull(resp.Body, preview)
	previewStr := strings.ToLower(string(preview[:n]))

	// Check if this is a bot challenge (e.g. OPUS 4 / Cloudflare "Making sure you're not a bot!")
	if strings.Contains(contentType, "text/html") ||
		strings.Contains(previewStr, "not a bot") ||
		strings.Contains(previewStr, "cloudflare") ||
		strings.Contains(previewStr, "security check") ||
		strings.Contains(previewStr, "challenge-platform") {
		// Strictly respect user constraint: do not attempt to bypass/mitigate
		doc.DownloadStatus = "blocked_bot_checker"
		return
	}

	// If it is genuinely a PDF, save to disk
	if strings.Contains(contentType, "application/pdf") || strings.HasPrefix(string(preview[:n]), "%PDF-") {
		progSlug := cleanSlug(programName, "program")
		dir := filepath.Join(p.downloadDir, progSlug)
		_ = os.MkdirAll(dir, 0755)

		fileName := filepath.Base(doc.URL)
		if !strings.HasSuffix(strings.ToLower(fileName), ".pdf") {
			fileName += ".pdf"
		}
		targetPath := filepath.Join(dir, fileName)

		outFile, err := os.Create(targetPath)
		if err == nil {
			defer outFile.Close()
			_, _ = outFile.Write(preview[:n])
			_, _ = io.Copy(outFile, resp.Body)
			doc.LocalPath = targetPath
			doc.DownloadStatus = "downloaded"
			return
		}
	}

	doc.DownloadStatus = "blocked_bot_checker"
}

func cleanSlug(s, fallback string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		s = fallback
	}
	s = strings.ReplaceAll(s, " ", "_")
	s = strings.ReplaceAll(s, "/", "_")
	s = strings.ReplaceAll(s, "\\", "_")
	s = strings.ReplaceAll(s, ":", "_")
	s = strings.ReplaceAll(s, "|", "_")
	return s
}
